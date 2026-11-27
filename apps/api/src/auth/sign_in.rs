//! Staff sign-in (FT-044, FT-062; FR-019): email and password, then a TOTP code.
//!
//! - `POST /staff/enrolment` `{token, password}`: with the one-time token from
//!   `api create-admin` or a staff invitation (`POST /staff`), sets the password and returns
//!   a new TOTP secret and otpauth URI (shown once) with a challenge of purpose `enrol`.
//! - `POST /staff/sign-in` `{email, password}`: returns a challenge of purpose `sign_in`.
//! - `POST /staff/sign-in/verify` `{challenge, code}`: completes either challenge with a
//!   TOTP code (confirming the authenticator for `enrol`, which also makes an invited
//!   account active) and returns a session token, once.
//! - `POST /staff/sign-out`: revokes the current session.
//! - `GET /staff/me`: the signed-in staff member, their roles and permissions.
//!
//! A wrong email and a wrong password give the same `invalid_credentials` error after the
//! same work (an unknown email is checked against a dummy hash). Failed passwords and codes
//! count towards the per-account and per-IP limits (`rate_limit`).

use std::net::SocketAddr;

use axum::{
    Extension, Json, Router,
    extract::{ConnectInfo, FromRequestParts, State},
    http::{HeaderMap, StatusCode, request::Parts},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

use super::{
    Permission, StaffIdentity,
    rate_limit::{self, Outcome},
    secrets,
    session::{self, CurrentSession},
};
use crate::{
    audit::{self, AuditEntry},
    error::{ApiError, ErrorBody},
    request_id::REQUEST_ID_HEADER,
    state::AppState,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/staff/enrolment", post(enrol))
        .route("/staff/sign-in", post(sign_in))
        .route("/staff/sign-in/verify", post(verify))
        .route("/staff/sign-out", post(sign_out))
        .route("/staff/me", get(me))
}

// --- Requests and responses ---

// No Debug: holds a secret.
#[derive(Deserialize, ToSchema)]
pub struct EnrolRequest {
    /// The one-time enrolment token printed by `api create-admin` or sent in a staff
    /// invitation.
    pub token: String,
    /// The new password: at least `STAFF_PASSWORD_MIN_LENGTH` characters (default 12).
    #[schema(format = Password)]
    pub password: String,
}

// No Debug: holds a secret.
#[derive(Deserialize, ToSchema)]
pub struct SignInRequest {
    #[schema(example = "admin@example.com")]
    pub email: String,
    #[schema(format = Password)]
    pub password: String,
}

// No Debug: holds a secret.
#[derive(Deserialize, ToSchema)]
pub struct VerifyRequest {
    /// From `POST /staff/sign-in` or `POST /staff/enrolment`.
    pub challenge: String,
    /// The 6-digit code the authenticator app shows now.
    #[schema(example = "123456")]
    pub code: String,
}

// No Debug: holds a secret.
#[derive(Serialize, ToSchema)]
pub struct ChallengeResponse {
    /// Sent back with the code to `POST /staff/sign-in/verify`.
    pub challenge: String,
    pub expires_at: DateTime<Utc>,
}

// No Debug: holds a secret.
#[derive(Serialize, ToSchema)]
pub struct EnrolResponse {
    /// Sent back with the first code to `POST /staff/sign-in/verify`.
    pub challenge: String,
    pub expires_at: DateTime<Utc>,
    pub totp: TotpSetup,
}

/// What an authenticator app needs. Returned once; the server keeps only an encrypted copy.
// No Debug: holds a secret.
#[derive(Serialize, ToSchema)]
pub struct TotpSetup {
    /// Base32, for typing into an authenticator app.
    pub secret: String,
    /// `otpauth://totp/…`, for a QR code.
    pub otpauth_uri: String,
    #[schema(example = "SHA1")]
    pub algorithm: &'static str,
    #[schema(example = 6)]
    pub digits: u8,
    /// Seconds each code lasts.
    #[schema(example = 30)]
    pub period: u64,
}

// No Debug: holds a secret.
#[derive(Serialize, ToSchema)]
pub struct SessionResponse {
    /// Send as `Authorization: Bearer <token>`. Returned once.
    pub token: String,
    #[schema(example = "Bearer")]
    pub token_type: &'static str,
    /// When the session ends if unused; each use moves it forward.
    pub expires_at: DateTime<Utc>,
    pub staff: Me,
}

/// The signed-in staff member.
#[derive(Debug, Serialize, ToSchema)]
pub struct Me {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    /// `active` (only active staff can be signed in).
    #[schema(example = "active")]
    pub status: String,
    /// Role keys, e.g. `admin`, `operations`.
    #[schema(example = json!(["admin"]))]
    pub roles: Vec<String>,
    /// Permission keys the roles give, e.g. `driver.approve`.
    #[schema(example = json!(["driver.view", "driver.approve"]))]
    pub permissions: Vec<&'static str>,
}

// --- Errors ---

fn invalid_credentials() -> ApiError {
    ApiError::new(
        StatusCode::UNAUTHORIZED,
        "invalid_credentials",
        "The email or password is incorrect.",
    )
}

fn invalid_code() -> ApiError {
    ApiError::new(
        StatusCode::UNAUTHORIZED,
        "invalid_code",
        "The code is incorrect or has expired. Enter the current code from your authenticator app.",
    )
}

fn challenge_expired() -> ApiError {
    ApiError::new(
        StatusCode::UNAUTHORIZED,
        "challenge_expired",
        "This sign-in has expired or was already completed. Please sign in again.",
    )
}

fn invalid_enrolment_token() -> ApiError {
    ApiError::new(
        StatusCode::UNAUTHORIZED,
        "invalid_enrolment_token",
        "This enrolment link has expired or was already used. Ask an admin for a new one.",
    )
}

fn totp_enrolment_required() -> ApiError {
    ApiError::new(
        StatusCode::FORBIDDEN,
        "totp_enrolment_required",
        "Finish setting up your authenticator app with your enrolment link first.",
    )
}

// --- Request context ---

/// The client's IP address: from `API_CLIENT_IP_HEADER` when set (a header the proxy in
/// front of the api overwrites), otherwise the connection's peer address.
pub struct ClientIp(pub String);

impl FromRequestParts<AppState> for ClientIp {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, ApiError> {
        if let Some(name) = &state.config.client_ip_header {
            let from_header = parts
                .headers
                .get(name.as_str())
                .and_then(|v| v.to_str().ok())
                // X-Forwarded-For style lists: the proxy appends the address it saw last.
                .and_then(|v| v.rsplit(',').next())
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty());
            return Ok(ClientIp(from_header.unwrap_or_else(|| "unknown".into())));
        }
        let peer = parts
            .extensions
            .get::<ConnectInfo<SocketAddr>>()
            .map(|c| c.0.ip().to_string());
        Ok(ClientIp(peer.unwrap_or_else(|| "unknown".into())))
    }
}

pub(crate) fn request_id(headers: &HeaderMap) -> Option<String> {
    headers
        .get(REQUEST_ID_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned)
}

/// Writes an audit entry on its own (for events that change nothing else).
async fn audit_event(
    db: &PgPool,
    actor: Option<Uuid>,
    action: &str,
    staff_id: Uuid,
    after: Value,
    request_id: Option<String>,
) -> Result<(), ApiError> {
    let mut tx = db.begin().await?;
    audit::record(
        &mut tx,
        &AuditEntry {
            staff_id: actor,
            action,
            record_type: "staff",
            record_id: staff_id.to_string(),
            before: None,
            after: Some(after),
            reason: None,
            request_id,
        },
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

/// Records a failed sign-in for an existing account. The actor is unknown (it may not be
/// the staff member), so `staff_id` is empty and the account is the record.
async fn audit_failure(
    db: &PgPool,
    staff_id: Uuid,
    failure: &str,
    ip: &str,
    request_id: Option<String>,
) -> Result<(), ApiError> {
    audit_event(
        db,
        None,
        "staff.sign_in_failed",
        staff_id,
        json!({ "failure": failure, "ip": ip }),
        request_id,
    )
    .await
}

async fn create_challenge(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    state: &AppState,
    staff_id: Uuid,
    enrolment_token_hash: Option<&[u8]>,
) -> Result<ChallengeResponse, sqlx::Error> {
    let (challenge, hash) = secrets::new_token();
    let purpose = if enrolment_token_hash.is_some() {
        "enrol"
    } else {
        "sign_in"
    };
    let expires_at: DateTime<Utc> = sqlx::query_scalar(
        "INSERT INTO staff_sign_in_challenges (token_hash, staff_id, purpose, enrolment_token_hash, expires_at)
         VALUES ($1, $2, $3, $4, now() + make_interval(secs => $5))
         RETURNING expires_at",
    )
    .bind(&hash[..])
    .bind(staff_id)
    .bind(purpose)
    .bind(enrolment_token_hash)
    .bind(state.config.staff_auth.challenge_ttl.as_secs_f64())
    .fetch_one(&mut **tx)
    .await?;
    Ok(ChallengeResponse {
        challenge,
        expires_at,
    })
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> T + Send + 'static,
) -> Result<T, ApiError> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(ApiError::internal)
}

// --- Handlers ---

/// `POST /staff/enrolment`
#[utoipa::path(
    post,
    path = "/staff/enrolment",
    tag = "staff-auth",
    operation_id = "staff_enrolment",
    summary = "Set a password and get a new authenticator secret",
    description = "With the one-time enrolment token from `api create-admin` or a staff invitation (`POST /staff`), sets the staff member's password and returns a new TOTP secret and otpauth URI (shown once) with a challenge. Complete enrolment with `POST /staff/sign-in/verify` and the first code. The token stays usable until enrolment completes or it expires; using it again replaces the secret. Needs no authentication.",
    request_body = EnrolRequest,
    responses(
        (status = 200, description = "Password set; the authenticator secret, shown once.", body = EnrolResponse),
        (status = 401, description = "`invalid_enrolment_token`: unknown, expired or used.", body = ErrorBody),
        (status = 422, description = "`invalid_request`: the password is too short or too long.", body = ErrorBody),
    )
)]
pub async fn enrol(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<EnrolRequest>,
) -> Result<Json<EnrolResponse>, ApiError> {
    let Some(token_hash) = secrets::token_hash(&body.token) else {
        return Err(invalid_enrolment_token());
    };
    let row: Option<(Uuid, String)> = sqlx::query_as(
        "SELECT t.staff_id, s.email
         FROM staff_enrolment_tokens t JOIN staff s ON s.id = t.staff_id
         WHERE t.token_hash = $1 AND t.used_at IS NULL AND t.expires_at > now()
           AND s.status IN ('active', 'invited')",
    )
    .bind(&token_hash[..])
    .fetch_optional(&state.db)
    .await?;
    let Some((staff_id, email)) = row else {
        return Err(invalid_enrolment_token());
    };

    let min = state.config.staff_auth.password_min_length;
    let length = body.password.chars().count();
    if length < min || body.password.len() > secrets::PASSWORD_MAX_LENGTH {
        return Err(ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_request",
            format!(
                "The password must have at least {min} characters and at most {} bytes.",
                secrets::PASSWORD_MAX_LENGTH
            ),
        )
        .with_details(json!({
            "fields": {"password": "length"},
            "min_length": min,
            "max_length": secrets::PASSWORD_MAX_LENGTH,
        })));
    }

    let password = body.password;
    let password_hash = blocking(move || secrets::hash_password(&password))
        .await?
        .map_err(ApiError::internal)?;
    let secret: [u8; 20] = secrets::random_bytes();
    let totp = secrets::totp(secret.to_vec(), &email).map_err(ApiError::internal)?;
    let setup = TotpSetup {
        secret: totp.secret().to_base32(),
        otpauth_uri: totp.to_url().map_err(ApiError::internal)?,
        algorithm: "SHA1",
        digits: secrets::TOTP_DIGITS,
        period: secrets::TOTP_STEP_SECONDS,
    };
    let encrypted = secrets::encrypt_totp_secret(
        &state.config.staff_auth.totp_encryption_key,
        staff_id,
        &secret,
    );

    let mut tx = state.db.begin().await?;
    // A new secret replaces any earlier one; it counts only once confirmed with a code.
    sqlx::query(
        "UPDATE staff SET password_hash = $2, totp_secret_nonce = $3, totp_secret_ciphertext = $4,
                          totp_confirmed_at = NULL, totp_last_used_step = NULL
         WHERE id = $1",
    )
    .bind(staff_id)
    .bind(&password_hash)
    .bind(&encrypted.nonce[..])
    .bind(&encrypted.ciphertext)
    .execute(&mut *tx)
    .await?;
    // Challenges issued before this one belong to the old secret.
    sqlx::query(
        "UPDATE staff_sign_in_challenges SET used_at = now() WHERE staff_id = $1 AND used_at IS NULL",
    )
    .bind(staff_id)
    .execute(&mut *tx)
    .await?;
    let challenge = create_challenge(&mut tx, &state, staff_id, Some(&token_hash[..])).await?;
    audit::record(
        &mut tx,
        &AuditEntry {
            staff_id: Some(staff_id),
            action: "staff.enrolment_start",
            record_type: "staff",
            record_id: staff_id.to_string(),
            before: None,
            after: Some(json!({"password_set": true, "totp_confirmed": false})),
            reason: None,
            request_id: request_id(&headers),
        },
    )
    .await?;
    tx.commit().await?;

    Ok(Json(EnrolResponse {
        challenge: challenge.challenge,
        expires_at: challenge.expires_at,
        totp: setup,
    }))
}

/// `POST /staff/sign-in`
#[utoipa::path(
    post,
    path = "/staff/sign-in",
    tag = "staff-auth",
    operation_id = "staff_sign_in",
    summary = "First step of staff sign-in: email and password",
    description = "Returns a short-lived challenge to complete with a code at `POST /staff/sign-in/verify`. A wrong email and a wrong password give the same `invalid_credentials` error. Failed attempts are limited per account and per IP address (`STAFF_SIGNIN_MAX_ATTEMPTS` per `STAFF_SIGNIN_WINDOW_MINUTES`). Needs no authentication.",
    request_body = SignInRequest,
    responses(
        (status = 200, description = "Password accepted; enter a code next.", body = ChallengeResponse),
        (status = 401, description = "`invalid_credentials`: wrong email or password, or the account cannot sign in.", body = ErrorBody),
        (status = 403, description = "`totp_enrolment_required`: the password is right but enrolment was not completed.", body = ErrorBody),
        (status = 429, description = "`rate_limited`: too many failed attempts; see the `Retry-After` header.", body = ErrorBody, headers(("Retry-After" = u64, description = "Seconds until another attempt is allowed."))),
    )
)]
pub async fn sign_in(
    State(state): State<AppState>,
    headers: HeaderMap,
    ClientIp(ip): ClientIp,
    Json(body): Json<SignInRequest>,
) -> Result<Response, ApiError> {
    let account_key = body.email.trim().to_lowercase();
    let attempt =
        match rate_limit::begin(&state.db, &state.config.staff_auth, &account_key, &ip).await? {
            Outcome::Allowed(a) => a,
            Outcome::Limited(r) => return Ok(r.into_response()),
        };

    let staff: Option<(Uuid, String, Option<String>, bool)> = sqlx::query_as(
        "SELECT id, status, password_hash, totp_confirmed_at IS NOT NULL
         FROM staff WHERE lower(email) = $1",
    )
    .bind(&account_key)
    .fetch_optional(&state.db)
    .await?;

    // The same hashing work whether or not the account exists or has a password.
    let phc = staff.as_ref().and_then(|s| s.2.clone());
    let password = body.password;
    let password_ok = if password.len() > secrets::PASSWORD_MAX_LENGTH {
        false
    } else {
        blocking(move || secrets::verify_password(&password, phc.as_deref())).await?
    };

    let Some((staff_id, status, _, totp_confirmed)) = staff else {
        attempt.failed();
        return Err(invalid_credentials());
    };
    let request_id = request_id(&headers);
    if !password_ok {
        attempt.failed();
        audit_failure(&state.db, staff_id, "wrong_password", &ip, request_id).await?;
        return Err(invalid_credentials());
    }
    if status == "invited" {
        // The password was set by a started but unfinished enrolment.
        attempt.succeeded(&state.db).await?;
        return Err(totp_enrolment_required());
    }
    if status != "active" {
        attempt.failed();
        audit_failure(&state.db, staff_id, "account_not_active", &ip, request_id).await?;
        return Err(invalid_credentials());
    }
    attempt.succeeded(&state.db).await?;
    if !totp_confirmed {
        return Err(totp_enrolment_required());
    }

    let mut tx = state.db.begin().await?;
    let challenge = create_challenge(&mut tx, &state, staff_id, None).await?;
    tx.commit().await?;
    Ok(Json(challenge).into_response())
}

/// `POST /staff/sign-in/verify`
#[utoipa::path(
    post,
    path = "/staff/sign-in/verify",
    tag = "staff-auth",
    operation_id = "staff_sign_in_verify",
    summary = "Second step of staff sign-in: the authenticator code",
    description = "Completes a challenge (from sign-in or enrolment) with the current 6-digit TOTP code and returns a session token, once. Send it as `Authorization: Bearer <token>`. Each code is accepted once; each challenge completes once. Wrong codes count towards the sign-in limit. Needs no authentication.",
    request_body = VerifyRequest,
    responses(
        (status = 200, description = "Signed in.", body = SessionResponse),
        (status = 401, description = "`invalid_code`: wrong, old or already used code. `challenge_expired`: the challenge is unknown, expired or already completed. `invalid_enrolment_token`: the enrolment token expired meanwhile.", body = ErrorBody),
        (status = 429, description = "`rate_limited`: too many failed attempts; see the `Retry-After` header.", body = ErrorBody, headers(("Retry-After" = u64, description = "Seconds until another attempt is allowed."))),
    )
)]
pub async fn verify(
    State(state): State<AppState>,
    headers: HeaderMap,
    ClientIp(ip): ClientIp,
    Json(body): Json<VerifyRequest>,
) -> Result<Response, ApiError> {
    let Some(challenge_hash) = secrets::token_hash(&body.challenge) else {
        return Err(challenge_expired());
    };
    #[allow(clippy::type_complexity)]
    let row: Option<(
        Uuid,
        String,
        Option<Vec<u8>>,
        String,
        String,
        Option<Vec<u8>>,
        Option<Vec<u8>>,
        bool,
        Option<i64>,
    )> = sqlx::query_as(
        "SELECT c.staff_id, c.purpose, c.enrolment_token_hash, s.email, s.status,
                s.totp_secret_nonce, s.totp_secret_ciphertext, s.totp_confirmed_at IS NOT NULL,
                s.totp_last_used_step
         FROM staff_sign_in_challenges c JOIN staff s ON s.id = c.staff_id
         WHERE c.token_hash = $1 AND c.used_at IS NULL AND c.expires_at > now()",
    )
    .bind(&challenge_hash[..])
    .fetch_optional(&state.db)
    .await?;
    let Some((
        staff_id,
        purpose,
        enrolment_token_hash,
        email,
        status,
        nonce,
        ciphertext,
        totp_confirmed,
        last_step,
    )) = row
    else {
        return Err(challenge_expired());
    };
    let request_id = request_id(&headers);
    let enrolling = purpose == "enrol";
    // Invited staff may only complete enrolment; archived staff may do nothing.
    let may_continue = status == "active" || (enrolling && status == "invited");
    if !may_continue || (!enrolling && !totp_confirmed) {
        return Err(challenge_expired());
    }
    let (Some(nonce), Some(ciphertext)) = (nonce, ciphertext) else {
        return Err(challenge_expired());
    };

    let account_key = email.to_lowercase();
    let attempt =
        match rate_limit::begin(&state.db, &state.config.staff_auth, &account_key, &ip).await? {
            Outcome::Allowed(a) => a,
            Outcome::Limited(r) => return Ok(r.into_response()),
        };

    let Some(secret) = secrets::decrypt_totp_secret(
        &state.config.staff_auth.totp_encryption_key,
        staff_id,
        &nonce,
        &ciphertext,
    ) else {
        attempt.succeeded(&state.db).await?;
        return Err(ApiError::internal(format!(
            "TOTP secret of staff {staff_id} does not decrypt: is TOTP_ENCRYPTION_KEY the key it was stored with?"
        )));
    };
    let totp = secrets::totp(secret, &email).map_err(ApiError::internal)?;
    let step = secrets::check_totp(&totp, &body.code, secrets::unix_now())
        // A code is accepted once: never a step at or before the last one used.
        .filter(|s| last_step.is_none_or(|last| i64::try_from(*s).is_ok_and(|s| s > last)));
    let Some(step) = step else {
        attempt.failed();
        audit_failure(&state.db, staff_id, "wrong_code", &ip, request_id).await?;
        return Err(invalid_code());
    };
    let step = i64::try_from(step).map_err(ApiError::internal)?;

    let mut tx = state.db.begin().await?;
    let used = sqlx::query(
        "UPDATE staff_sign_in_challenges SET used_at = now()
         WHERE token_hash = $1 AND used_at IS NULL AND expires_at > now()",
    )
    .bind(&challenge_hash[..])
    .execute(&mut *tx)
    .await?;
    if used.rows_affected() != 1 {
        // Completed by a concurrent request.
        attempt.failed();
        return Err(challenge_expired());
    }
    let updated = sqlx::query(
        "UPDATE staff SET totp_last_used_step = $2,
                          totp_confirmed_at = COALESCE(totp_confirmed_at, now()),
                          status = 'active'
         WHERE id = $1 AND (status = 'active' OR ($3 AND status = 'invited'))
           AND (totp_last_used_step IS NULL OR totp_last_used_step < $2)",
    )
    .bind(staff_id)
    .bind(step)
    .bind(enrolling)
    .execute(&mut *tx)
    .await?;
    if updated.rows_affected() != 1 {
        // The same code was used by a concurrent request.
        attempt.failed();
        return Err(invalid_code());
    }
    if let Some(token_hash) = &enrolment_token_hash {
        let consumed = sqlx::query(
            "UPDATE staff_enrolment_tokens SET used_at = now()
             WHERE token_hash = $1 AND used_at IS NULL AND expires_at > now()",
        )
        .bind(token_hash)
        .execute(&mut *tx)
        .await?;
        if consumed.rows_affected() != 1 {
            attempt.failed();
            return Err(invalid_enrolment_token());
        }
        audit::record(
            &mut tx,
            &AuditEntry {
                staff_id: Some(staff_id),
                action: "staff.enrolment_complete",
                record_type: "staff",
                record_id: staff_id.to_string(),
                before: Some(json!({"totp_confirmed": false, "status": status})),
                after: Some(json!({"totp_confirmed": true, "status": "active"})),
                reason: None,
                request_id: request_id.clone(),
            },
        )
        .await?;
    }
    let new_session = session::create(
        &mut tx,
        staff_id,
        state.config.staff_auth.session_idle_timeout,
    )
    .await?;
    audit::record(
        &mut tx,
        &AuditEntry {
            staff_id: Some(staff_id),
            action: "staff.sign_in",
            record_type: "staff",
            record_id: staff_id.to_string(),
            before: None,
            after: Some(json!({"session_id": new_session.id, "ip": ip, "enrolment": enrolling})),
            reason: None,
            request_id,
        },
    )
    .await?;
    tx.commit().await?;
    attempt.succeeded(&state.db).await?;
    rate_limit::clear_account(&state.db, &account_key).await?;

    let identity = StaffIdentity::load(&state.db, staff_id)
        .await?
        .ok_or_else(|| ApiError::internal("staff member vanished during sign-in"))?;
    Ok(Json(SessionResponse {
        token: new_session.token,
        token_type: "Bearer",
        expires_at: new_session.expires_at,
        staff: load_me(&state.db, &identity).await?,
    })
    .into_response())
}

/// `POST /staff/sign-out`: any signed-in staff member may end their own session.
#[utoipa::path(
    post,
    path = "/staff/sign-out",
    tag = "staff-auth",
    operation_id = "staff_sign_out",
    summary = "End the current staff session",
    description = "Revokes the session whose token the request carries; the token is refused from then on. Other sessions of the same staff member are not affected.",
    security(("staff_session" = [])),
    responses(
        (status = 204, description = "Signed out."),
        (status = 401, description = "`unauthenticated`: no valid session.", body = ErrorBody),
    )
)]
pub async fn sign_out(
    State(state): State<AppState>,
    staff: StaffIdentity,
    Extension(current): Extension<CurrentSession>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let mut tx = state.db.begin().await?;
    sqlx::query(
        "UPDATE staff_sessions SET revoked_at = now() WHERE id = $1 AND revoked_at IS NULL",
    )
    .bind(current.id)
    .execute(&mut *tx)
    .await?;
    audit::record(
        &mut tx,
        &AuditEntry {
            staff_id: Some(staff.staff_id),
            action: "staff.sign_out",
            record_type: "staff",
            record_id: staff.staff_id.to_string(),
            before: None,
            after: Some(json!({"session_id": current.id})),
            reason: None,
            request_id: request_id(&headers),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `GET /staff/me`: any signed-in staff member may see their own account.
#[utoipa::path(
    get,
    path = "/staff/me",
    tag = "staff-auth",
    operation_id = "staff_me",
    summary = "The signed-in staff member, their roles and permissions",
    security(("staff_session" = [])),
    responses(
        (status = 200, description = "The signed-in staff member.", body = Me),
        (status = 401, description = "`unauthenticated`: no valid session.", body = ErrorBody),
    )
)]
pub async fn me(State(state): State<AppState>, staff: StaffIdentity) -> Result<Json<Me>, ApiError> {
    Ok(Json(load_me(&state.db, &staff).await?))
}

async fn load_me(db: &PgPool, staff: &StaffIdentity) -> Result<Me, ApiError> {
    let (email, name, status): (String, String, String) =
        sqlx::query_as("SELECT email, name, status FROM staff WHERE id = $1")
            .bind(staff.staff_id)
            .fetch_one(db)
            .await?;
    let roles: Vec<String> = sqlx::query_scalar(
        "SELECT role_key FROM staff_roles WHERE staff_id = $1 ORDER BY role_key",
    )
    .bind(staff.staff_id)
    .fetch_all(db)
    .await?;
    Ok(Me {
        id: staff.staff_id,
        email,
        name,
        status,
        roles,
        permissions: staff.permissions().map(Permission::key).collect(),
    })
}
