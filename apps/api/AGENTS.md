# AGENTS.md — api

These instructions add to the repository root `AGENTS.md`; they never relax it. Read the root file first.

## What this app is
- Purpose: the one server every client calls (customers, drivers, staff; later the MCP server).
- Kind: server/API
- Stack: Rust (pinned in rust-toolchain.toml), axum on tokio, sqlx on Postgres 17 with PostGIS (ADR-001). Details in docs/TECHNICAL.md, Components.
- Depends on: Postgres (docker-compose.yml locally). Its contract is packages/api-contract.

## Commands
Use this app's block in the README [Commands](../../README.md#commands) section. Run them from the repository root with `-p api`.

## Rules for this app
- Configuration: secrets come from the secret store named in docs/DEPLOYMENT.md and are never logged. Every variable is read in `src/config.rs`, has a default that works locally, and is documented in `.env.example`.
- Errors: handlers return `error::ApiError`; never build error JSON by hand. The body format is in docs/TECHNICAL.md (Error responses). Internal causes are logged with `ApiError::internal`, never sent to clients.
- Database: schema changes only through new files in `migrations/`; never edit a migration that has been merged. Use `sqlx::query` with bound parameters, never string-formatted SQL.
- Staff authorization: every staff endpoint takes `auth::StaffIdentity` and calls `staff.require(Permission::…)` before doing anything; never check role names in code. New permissions are added only in `src/auth/permissions.rs` (synced on startup); which roles hold them is data, changed by migration. Never add a way to set a `StaffIdentity` other than the sign-in layer (no test-only headers or bypasses in non-test code).
- Audit: every staff change to a driver, customer, ride, payment, fare rule, payout or staff account (FR-022) writes `audit::record` in the same transaction as the change, before commit.
- Tests: `tests/` holds HTTP tests that drive the router with `tower::ServiceExt::oneshot`; tests needing the database say so, use `#[sqlx::test]` (a throwaway database per test, never the shared `chitty` one) and expect the docker-compose database (or `DATABASE_URL`, defaulted in `.cargo/config.toml`).
- Directories not to edit by hand: `target/`.

## Release
Versioned separately; tags are `api-vX.Y.Z`. Record user-visible changes in this folder's CHANGELOG.md. Release steps are in docs/DEPLOYMENT.md; signed builds for mobile and desktop apps run in `.github/workflows/release.yml`, never on a developer machine or in an agent session.
