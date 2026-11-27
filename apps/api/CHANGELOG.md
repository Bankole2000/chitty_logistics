# Changelog — api

All notable changes to this app are recorded here. Format based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow [Semantic Versioning](https://semver.org/) and are tagged `api-vX.Y.Z`.

## [Unreleased]
### Added
- Server skeleton: health check at `GET /health` including the database, request IDs (`x-request-id`), the standard error body, CORS from `API_CORS_ORIGINS`, migrations with PostGIS enabled.
- OpenAPI 3.1 contract generated from the code with utoipa, committed as `packages/api-contract/openapi.json`, documenting `GET /health` and the standard error body (`ErrorBody`). New subcommand `api openapi --out <file>` writes it and `api openapi --check [<file>]` fails when it is out of date; neither needs a database. A Bruno collection in `packages/api-contract/bruno/` has a request per endpoint.
- Staff roles and permissions: staff accounts, roles (operations, admin, finance, HR with drafted permission sets), permission keys synced to the database on startup, a permission check that answers 403 `forbidden`, and append-only audit entries written in the same transaction as the change they record.
- Staff sign-in (FT-044, FT-062): `api create-admin --email … --name …` creates the first admin and prints a one-time enrolment token; `POST /staff/enrolment` sets the password (argon2id, at least `STAFF_PASSWORD_MIN_LENGTH` characters) and returns a TOTP secret and otpauth URI once; `POST /staff/sign-in` (email and password) and `POST /staff/sign-in/verify` (challenge and 6-digit code) return an opaque session token sent as `Authorization: Bearer`; `POST /staff/sign-out` ends the session; `GET /staff/me` returns the staff member, roles and permissions. Sessions end after `STAFF_SESSION_IDLE_TIMEOUT_MINUTES` without use; failed attempts are limited per account and per IP address (429 `rate_limited`). TOTP secrets are encrypted with `TOTP_ENCRYPTION_KEY`, which is required outside `API_ENV=development`. Sign-ins, failed sign-ins and sign-outs are audited.
### Changed
### Fixed
### Removed
