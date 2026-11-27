# AGENTS.md — packages/api-contract

These instructions add to the repository root `AGENTS.md`; they never relax it. Read the root file first.

## What this package is
- Purpose: the API contract for apps/api: its OpenAPI 3.1 spec (`openapi.json`) and a Bruno collection (`bruno/`) with a request for every endpoint. Typed clients are generated from the spec when the first client app is built.
- Kind: API contract
- Used by: customer-web, customer-mobile-app, driver-web, driver-mobile-app, admin-web, admin-app, and the future mcp server (M6; server-to-server). Its source is apps/api.
- Stack: OpenAPI 3.1 JSON generated from the api's Rust code with utoipa; Bruno `.bru` request files.

## Rules for this package
- Packages are not deployed or versioned on their own; they ship inside the apps that use them.
- A change here affects every consumer. Run the checks of every app listed under "Used by" (README [Commands](../../README.md#commands) section), not just this package's. Until those apps exist, run the api checks.
- Breaking changes (renamed or removed exports, changed shapes) must update every consumer in the same PR, or be made additive first. API contracts also follow the compatibility rules in docs/TECHNICAL.md, because released mobile and desktop apps keep using old versions.
- Never import from an app. Never put secrets or environment-specific values here.
- Generated files: `openapi.json` is generated from the utoipa annotations in apps/api (assembled in `apps/api/src/openapi.rs`) with `cargo run -p api -- openapi --out packages/api-contract/openapi.json`, run from the repository root. Never edit it by hand; change the api code and regenerate. `cargo test -p api` (and `cargo run -p api -- openapi --check`) fails while it is stale.
- Bruno collection: one `.bru` request per path and method in the spec, its `url` written as the `baseUrl` environment variable followed by the path (see `bruno/health/Health.bru`), with path parameters as `:name`. `cargo test -p api` fails when an endpoint has no request. Environments in `bruno/environments/` hold only local, non-secret values; anything secret is read from the developer's environment with Bruno's `process.env.NAME` variables.
