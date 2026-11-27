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
- Tests: `tests/` holds HTTP tests that drive the router with `tower::ServiceExt::oneshot`; tests needing the database say so and expect the docker-compose database (or `DATABASE_URL`).
- Directories not to edit by hand: `target/`.

## Release
Versioned separately; tags are `api-vX.Y.Z`. Record user-visible changes in this folder's CHANGELOG.md. Release steps are in docs/DEPLOYMENT.md; signed builds for mobile and desktop apps run in `.github/workflows/release.yml`, never on a developer machine or in an agent session.
