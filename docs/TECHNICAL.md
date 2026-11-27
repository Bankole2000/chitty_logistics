# Technical Specification

Status: Draft | Owner: @bankole2000 | Version: 0.4 | Last updated: 2026-11-26
References: FEATURES.md, PRD.md, DESIGN.md

## Architecture overview
{{How the apps fit together: which clients call which services, data stores, external services. A component diagram helps.}}

## Components
One row per deployable app in `apps/`, per shared package in `packages/`, and for `infra/` if used. App names match folder names, `app:` labels and release tags. Kinds: web client, mobile client, desktop client, server/API, worker, shared package, infrastructure.

| Component | Path | Kind | Stack and version | Deploys to | Owner | Requirements |
|---|---|---|---|---|---|---|
| customer-web | apps/customer-web | Web client | {{framework, language and version}} | {{hosting}} | @bankole2000 | FR-001–FR-009, FR-011–FR-014, FR-025, FR-030, FR-031, FR-035 |
| customer-mobile-app | apps/customer-mobile-app | Mobile client | {{stack; minimum OS versions}} | App Store / Google Play | @bankole2000 | FR-001–FR-009, FR-011–FR-014, FR-025, FR-030, FR-031, FR-035 |
| driver-web | apps/driver-web | Web client | {{framework, language and version}} | {{hosting}} | @bankole2000 | FR-011, FR-012, FR-015, FR-018, FR-025 |
| driver-mobile-app | apps/driver-mobile-app | Mobile client | {{stack; minimum OS versions}} | App Store / Google Play | @bankole2000 | FR-004, FR-007, FR-010, FR-011, FR-012, FR-015–FR-018, FR-025, FR-031–FR-033, FR-035 |
| admin-web | apps/admin-web | Web client | {{framework, language and version}} | {{hosting}} | @bankole2000 | FR-018–FR-029, FR-031 |
| admin-app | apps/admin-app | Mobile client, desktop client | {{stack; minimum OS versions}} | App Store / Google Play; {{desktop distribution}} | @bankole2000 | FR-018–FR-029, FR-031 |
| api | apps/api | Server/API | Rust (stable, pinned in rust-toolchain.toml), axum on tokio, sqlx with migrations, Postgres 17 with PostGIS 3 (ADR-001) | {{hosting}} | @bankole2000 | FR-001–FR-035 |
| worker | apps/worker | Worker | Rust, same Cargo workspace and database layer as api (ADR-001) | {{hosting}} | @bankole2000 | FR-004, FR-006, FR-012, FR-014, FR-018, FR-034 |
| api-contract | packages/api-contract | Shared package | OpenAPI 3.1 spec generated from the api with utoipa, committed as openapi.json; typed clients generated from it when the first client is built | Not deployed | @bankole2000 | — |

## Local development
Defaults that avoid the usual clashes on a developer machine; change them only with a reason.
- **Ports:** uncommon ones, so the apps don't collide with other local tools (3000, 5173, 8000 and 8080 are often taken): api 18080, Postgres 15432; client ports are chosen when each client is created. Record each app's port in its `.env.example`.
- **Host:** use `127.0.0.1`, not `localhost`, in URLs and bind addresses. `localhost` can resolve to IPv6 first, which adds a delay to every database connection on some systems and makes some dev servers listen on IPv6 only.
- **Databases and other services** run in `docker-compose.yml` (committed) with trust authentication on `127.0.0.1` for local use and CI only, so no password sits in the repository. Each developer's extra services (caches, queues, other databases) go in a gitignored `docker-compose.override.yml`, which Compose loads automatically; commented-out services there cost nothing until needed. Turn off automatic restarts for local containers.
- **CORS:** each server reads the allowed client origins from configuration (e.g. `API_CORS_ORIGINS`), with defaults for every local client: the web dev server and the desktop and mobile shells.

## Repository and workspace
- Monorepo tooling: see ADR-001. A Cargo workspace at the repository root holds the Rust apps (api, later worker); the JavaScript and mobile workspace tooling is decided when the first client is created.
- Dependency rules: apps may depend on packages; packages never depend on apps; apps never import from another app's folder.
- {{Naming, code style, generated-code locations}}

Commands are listed in the README [Commands](../README.md#commands) section, not here.

## Modules
Significant modules within each component.

| Component | Module | Responsibility | Dependencies | Requirements |
|---|---|---|---|---|
| {{Component}} | {{Module}} | {{Purpose}} | {{Dependencies}} | FR-001 |
| api | `src/auth` (staff authorization) | Permission keys defined once in `src/auth/permissions.rs` and synced to the `permissions` table on startup; `StaffIdentity` extractor with `require(Permission)` returning 403 `forbidden`; only active staff hold permissions. Roles are data (`roles`, `role_permissions`, `staff_roles`; ADR-003). **Drafted default role sets, for the owner to change** (seeded in `migrations/20261127000000_staff_rbac.sql`): admin = every permission; operations = driver.view/approve/suspend, customer.view/suspend, ticket.view/reply/assign; finance = driver.view, payout.view/approve, refund.create, report.view; HR = staff.view only (PRD open decision: staff roles) | Postgres; sign-in (TASK-005) puts the identity on the request | FR-019 |
| api | `src/audit` (audit entries) | `audit::record` writes an `audit_entries` row (who, action, record type and id, before/after JSON, reason, request id, time) inside the transaction of the change, so both commit or neither does; the table is append-only (a trigger rejects updates and deletes) | Postgres | FR-022 |

## Domain and data model
Start from the resources (R-) in FEATURES.md: each stored resource becomes an entity, "Belongs to" becomes a relationship, and derived resources become queries or views. The users (U-) and the actions each may take (the coverage matrix) become the roles and permissions.

{{Entities, fields, relationships, constraints, indexes, migrations. Which component owns each data store.}}

## API and integrations
| ID | Operation | Input/output | Auth and errors | Requirement |
|---|---|---|---|---|
| API-001 | {{Method/path or operation}} | {{Contract}} | {{Policy/errors}} | FR-001 |

{{External integrations, timeouts, retries, idempotency, events/workflows where needed}}

### Error responses
Every error response from a server has the same JSON body, with `Content-Type: application/json`:

```json
{"error": {"code": "not_found", "message": "The requested resource was not found.", "request_id": "…", "details": null}}
```

- `code`: stable snake_case identifier clients branch on (e.g. `not_found`, `forbidden`, `unauthenticated`, `invalid_request`, `rate_limited`, `internal`). New codes may be added; existing ones never change meaning.
- `message`: human-readable, safe to show; never contains internal details (those are logged).
- `request_id`: the request's `x-request-id` (sent by the client or generated), also returned as a response header and present on every log line for the request.
- `details`: an object with code-specific data (e.g. which fields are invalid), or null.

### API contracts
Every server app has a contract package. List every consumer, including other servers.

| Provider | Contract | Consumers | Consumer auth | Compatibility rule |
|---|---|---|---|---|
| api | packages/api-contract | customer-web, customer-mobile-app | Customer sessions (phone and one-time code) | Installed-client window (below) |
| api | packages/api-contract | driver-web, driver-mobile-app | Driver sessions (phone and one-time code) | Installed-client window (below) |
| api | packages/api-contract | admin-web, admin-app | Staff sessions (email, password and second factor), checked per role | Installed-client window (below), for admin-app |
| api | packages/api-contract | mcp (server-to-server; planned for M6, added to the project setup when built) | Service credentials, acting for the person who granted the assistant connection; never that person's own session token | Rolling-deploy window |

The worker has no api of its own: it reads and writes the same data store as the api and takes jobs from a queue, so it deploys together with the api and follows the rolling-deploy rule below for shared data and job formats.

### API compatibility
- Clients and servers use generated or shared types from the contract package; they don't hand-write request shapes.
- Versioning: {{e.g. additive changes only within /v1; breaking changes need /v2}}.
- Installed clients (mobile, desktop) cannot be force-updated instantly. A provider with installed-client consumers supports every client version released in the last {{N months / N versions}}.
- Server-to-server: services deploy independently, so for the duration of a rollout both the old and new version of each service are live. Every change must work with the previous version of each consumer and provider.
- Breaking a field or endpoint: add the replacement → release consumers that use it → wait out the compatibility window → remove the old one. Record each step in an ADR.
- Forced upgrade: {{how an installed client learns its version is unsupported, e.g. a minimum-version endpoint checked at startup}}.

## Infrastructure
- Tool and layout: see ADR-002 and `infra/AGENTS.md`. {{e.g. modules/ plus environments/<env>/}}
- What it manages: {{networks, databases, hosting, DNS, queues, secret store entries; per environment}}
- What it does not manage: {{e.g. app stores, third-party SaaS configured by hand; list them so nothing is forgotten}}
- State: {{remote backend and locking}}. Applying changes is a release step (DEPLOYMENT.md).

## Third-party services and costs
| Service | Used by | Purpose | Plan / tier | Expected monthly cost | Limits and lock-in notes |
|---|---|---|---|---|---|
| Paystack | api, worker | Card and bank transfer payments, wallet top-ups, refunds; possibly driver payouts | {{Free / paid tier}} | {{Estimate}} | Which provider handles which method and payouts is an open decision (PRD) |
| OPay | api, worker | Card and bank transfer payments, wallet top-ups; possibly driver payouts | {{Free / paid tier}} | {{Estimate}} | As for Paystack; keep both behind one payment interface so either can be switched off |

## Error handling
{{How errors are classified, surfaced to users (messages, codes) and to developers; retry and fallback rules; offline behavior for mobile and desktop; what must never be shown to users}}

## Logging and observability
{{What each component logs and at which level, structured format, request/correlation IDs passed from client to server and between services, crash reporting for mobile and desktop, what must never be logged (secrets, personal data), metrics and tracing. Alerting and dashboards are in DEPLOYMENT.md.}}

## Security and privacy
- Client apps (web, mobile, desktop) are public: their code and configuration ship to users. They hold no secrets; anything they can access, an attacker can too. All authorization is enforced server-side.
- Internal apps (e.g. admin, ops) are still public on the internet unless the network says otherwise; they rely on server-side authorization like any other client.
- Server-to-server calls use service credentials scoped to what the caller needs, never a forwarded user token.
- {{Identity, permissions, input validation, secrets storage, sensitive data, threat mitigations, retention}}

## Technical decisions
| ID | Decision | Rationale | Status |
|---|---|---|---|
| ADR-001 | Server stack and workspace: Rust in a Cargo workspace (apps/api, apps/worker); axum on tokio; sqlx (compile-checked SQL, migrations) on Postgres 17 with PostGIS; OpenAPI generated with utoipa into packages/api-contract | SeaORM: less SQL but weaker PostGIS support. Separate services per client: rejected, one api serves every client by role. Client workspace tooling decided with the first client | Accepted (@bankole2000, 2026-11-26) |
| ADR-002 | Infrastructure-as-code tool: {{tool, or "none: managed hosting configured by hand"}} | {{Alternatives and tradeoffs; state storage}} | Proposed |
| ADR-003 | Sessions: opaque random tokens; only their hash is stored in Postgres, with expiry and last use. Authorization checks permissions; roles are data that map to permissions, so changing what a role may do needs no code change | JWT access and refresh tokens: no lookup per request, but sign-out, archiving staff (FT-064) and revoking assistants only take effect when the token expires | Accepted (@bankole2000, 2026-11-26) |

## Open decisions and approval
{{Human approver, revision, unresolved risks}}
