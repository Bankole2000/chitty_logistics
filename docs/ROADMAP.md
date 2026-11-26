# Development Roadmap

Status: Draft | Owner: @bankole2000 | Version: 0.4 | Last updated: 2026-11-26
References: FEATURES.md, PRD.md, DESIGN.md, TECHNICAL.md, TESTING.md

## Milestones
This table is the source of truth for milestone names; GitHub milestones must match exactly. `scripts/setup-github.sh` reads them from here.

M0 (foundation) comes first and the launch milestone last. The milestones in between are slices of the journeys in FEATURES.md: each one delivers whole journeys end to end, so it ends with something a user can try. Cut them from the approved features by priority (Must first) and by "Depends on", then name each one for what a user can do when it is done.

Managing milestones: a milestone is the set of journeys in its Journeys column. To add, split or move one, edit this table (a journey belongs to at most one milestone), then run `node scripts/check-features.mjs --write`. It updates the journeys' Milestone column in FEATURES.md, fails if a feature depends on one in a later milestone, and prints what each milestone delivers. Then `bash scripts/setup-github.sh --milestones` creates new milestones on GitHub and keeps their descriptions (outcome and journeys) in step. A journey too large for one milestone is split into two journeys in FEATURES.md first.

| Milestone | Outcome | Journeys | Exit criteria |
|---|---|---|---|
| M0 - Foundation | Runnable skeleton of every app | — | The M0 checklist below is done; TC-000 passes on a fresh clone |
| M1 - Drivers can apply and be approved | Staff accounts, prices and service areas are set up; drivers apply with their documents and vehicle, and operations approves or declines them | J-04, J-07, J-10 | Features of these journeys pass their acceptance tests on their surfaces |
| M2 - Customers can ride now | Customers sign up, book a ride now, track the driver, pay and rate; drivers complete trips and are paid | J-01, J-02, J-05, J-11 | Features of these journeys pass their acceptance tests on their surfaces |
| M3 - Book ahead, operations and support | Customers book rides in advance within a state and between states; operations watches rides live and answers support tickets | J-03, J-06, J-08, J-09 | Features of these journeys pass their acceptance tests on their surfaces |
| M4 - Hardening and launch | Reliable, deployed product | — | Acceptance and security checks pass; release checklist approved |
| M5 - Safety features | Customers and drivers share trips, raise SOS alerts that operations handles, and trips start only with the customer's PIN | J-12 | Features of these journeys pass their acceptance tests on their surfaces |
| M6 - Post-launch (stretch) | Automated driver verification and trip monitoring; dispatch and delivery; AI assistants through the MCP server | J-13, J-14, J-15, J-16 | Defined per release |

### M0 checklist
Every project with a client and a server needs these in M0; drop the ones that don't apply.
- [ ] Workspaces and toolchain versions pinned (ADR-001); apps and packages created from `templates/`.
- [ ] Each server answers a health check that includes its database, with request IDs and the standard error body.
- [ ] Local defaults (TECHNICAL.md, Local development): uncommon ports, `127.0.0.1`, trust-auth database in `docker-compose.yml`, a gitignored `docker-compose.override.yml` for personal services.
- [ ] Each app's `.env.example` documents every variable; clients get the server URL from public configuration.
- [ ] CORS on each server for every client origin (browser, desktop and mobile shells), from configuration.
- [ ] Generated contracts (API spec, typed clients) are committed, with checks that fail when they are out of date. Generators write their own output files.
- [ ] An api client collection (e.g. Bruno) for manual testing, with a check that it matches the spec.
- [ ] CI runs per app with path filters and one required `ci-ok` check; Dependabot configured.
- [ ] TC-000: on a fresh clone, every README step and command works, and each client shows live data from the server. A human checks each client in a browser or the app, including the browser console.

## Work breakdown
| Task key | Outcome | App | Features | Requirement IDs | Tests | Milestone | Depends on | Priority | Issue |
|---|---|---|---|---|---|---|---|---|---|
| TASK-001 | The api runs locally: Cargo workspace, Postgres with PostGIS in docker-compose, migrations, configuration from .env, health check including the database, request IDs, standard error body, CORS from configuration | api | — | — | TC-000 | M0 | None | High | TBD |
| TASK-002 | The OpenAPI spec is generated from the api into packages/api-contract, with a check that fails when it is out of date, and a Bruno collection that matches it | packages | — | — | TC-000 | M0 | TASK-001 | High | TBD |
| TASK-003 | CI runs the api checks (format, lint, tests against Postgres, contract up to date) and reports `ci-ok` | repo | — | — | TC-000 | M0 | TASK-001, TASK-002 | High | TBD |
| TASK-004 | Roles, permissions and audit entries: roles map to permissions (operations, admin, finance and HR seeded), a permission check every staff endpoint uses, and an audit entry for each staff change | api | FT-049, FT-063 | FR-019, FR-022 | TC-007, TC-010 | M1 | TASK-001 | High | TBD |
| TASK-005 | Staff sign in with email, password and a second factor (TOTP), and sign out; the first admin is created by a command | api | FT-044, FT-062 | FR-019 | TC-007, TC-010 | M1 | TASK-004 | High | TBD |
| TASK-006 | Admins invite staff with a role, and archive and restore them; an archived staff member is signed out and cannot sign in | api | FT-063, FT-064 | FR-019 | TC-010 | M1 | TASK-005 | High | TBD |

## Planning rules
- Plan only the next useful milestone in detail; split large tasks. FEATURES.md is the whole map; tasks are created only for the next slice.
- Plan from approved features (FEATURES.md, status `approved`) and their requirements, never from features that are only proposed.
- Each task belongs to one app (or `packages`/`infra`/`repo`). A feature spanning apps becomes one task per app, ordered infrastructure → contract → servers → clients (see GITHUB.md, Multi-app work).
- Tasks that build on a shared interface several tasks use (an api contract, a shared UI component): write the interface down in the milestone's parent issue first ("wave contract"), so the tasks can run in parallel.
- Mobile releases include store review time, and desktop releases need signing certificates and store accounts that can take days to weeks to obtain (see DEPLOYMENT.md). Start those early enough for the launch milestone.
- Set up the release workflow and each app's release environment before its first release, not during it.
- Map every implementation issue to feature IDs, requirement IDs and acceptance tests (TC IDs).
- Record dependencies and issue URLs; GitHub is the source of truth for task status.
- Reconcile scope changes with FEATURES.md and the approved PRD before generating new tasks.

## Approval
{{Human approver, revision, date}}
