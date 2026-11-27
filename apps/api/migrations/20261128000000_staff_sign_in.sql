-- Staff sign-in: password, TOTP second factor, enrolment tokens, sign-in challenges,
-- sessions and failed-attempt counting (TASK-005; FR-019; ADR-003).
--
-- Nothing secret is stored in a usable form: passwords as argon2id PHC strings, TOTP
-- secrets encrypted (AES-256-GCM with TOTP_ENCRYPTION_KEY, bound to the staff id), and every
-- token (enrolment, challenge, session) only as the SHA-256 hash of 256 random bits.

ALTER TABLE staff
    ADD COLUMN password_hash          text CHECK (password_hash LIKE '$argon2id$%'),
    ADD COLUMN totp_secret_nonce      bytea CHECK (octet_length(totp_secret_nonce) = 12),
    ADD COLUMN totp_secret_ciphertext bytea,
    -- Set when the staff member proved, with a first code, that their authenticator works.
    ADD COLUMN totp_confirmed_at      timestamptz,
    -- The last 30-second step accepted, so a code cannot be used twice (RFC 6238, 5.2).
    ADD COLUMN totp_last_used_step    bigint,
    ADD CONSTRAINT staff_totp_secret_complete
        CHECK ((totp_secret_nonce IS NULL) = (totp_secret_ciphertext IS NULL)),
    ADD CONSTRAINT staff_totp_confirmed_has_secret
        CHECK (totp_confirmed_at IS NULL OR totp_secret_ciphertext IS NOT NULL);

-- One-time tokens that let a staff member set their password and enrol their authenticator
-- (printed by `api create-admin`). Used up when enrolment completes.
CREATE TABLE staff_enrolment_tokens (
    token_hash bytea PRIMARY KEY CHECK (octet_length(token_hash) = 32),
    staff_id   uuid NOT NULL REFERENCES staff (id) ON DELETE CASCADE,
    created_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL,
    used_at    timestamptz
);
CREATE INDEX staff_enrolment_tokens_staff ON staff_enrolment_tokens (staff_id);

-- The second step of sign-in: given after the password (purpose sign_in) or after
-- enrolment (purpose enrol), completed once with a TOTP code.
CREATE TABLE staff_sign_in_challenges (
    token_hash           bytea PRIMARY KEY CHECK (octet_length(token_hash) = 32),
    staff_id             uuid NOT NULL REFERENCES staff (id) ON DELETE CASCADE,
    purpose              text NOT NULL CHECK (purpose IN ('sign_in', 'enrol')),
    enrolment_token_hash bytea REFERENCES staff_enrolment_tokens (token_hash) ON DELETE CASCADE,
    created_at           timestamptz NOT NULL DEFAULT now(),
    expires_at           timestamptz NOT NULL,
    used_at              timestamptz,
    CHECK ((purpose = 'enrol') = (enrolment_token_hash IS NOT NULL))
);
CREATE INDEX staff_sign_in_challenges_staff ON staff_sign_in_challenges (staff_id);

-- Opaque sessions (ADR-003). expires_at moves forward on every use (idle timeout).
CREATE TABLE staff_sessions (
    id           uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    token_hash   bytea NOT NULL UNIQUE CHECK (octet_length(token_hash) = 32),
    staff_id     uuid NOT NULL REFERENCES staff (id) ON DELETE CASCADE,
    created_at   timestamptz NOT NULL DEFAULT now(),
    last_used_at timestamptz NOT NULL DEFAULT now(),
    expires_at   timestamptz NOT NULL,
    revoked_at   timestamptz
);
CREATE INDEX staff_sessions_staff ON staff_sessions (staff_id);

-- Failed sign-in attempts, counted per account (the lower-cased email as typed, so unknown
-- emails are limited the same way) and per client IP address within the window
-- (STAFF_SIGNIN_MAX_ATTEMPTS per STAFF_SIGNIN_WINDOW_MINUTES). Rows older than the window
-- are deleted as new attempts arrive.
CREATE TABLE staff_sign_in_failures (
    id          bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    account_key text NOT NULL,
    ip          text NOT NULL,
    created_at  timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX staff_sign_in_failures_account ON staff_sign_in_failures (account_key, created_at);
CREATE INDEX staff_sign_in_failures_ip ON staff_sign_in_failures (ip, created_at);
CREATE INDEX staff_sign_in_failures_created_at ON staff_sign_in_failures (created_at);
