# Chitty Logistics

Allows users to book instant or scheduled intra-city or inter-city safe and secure rides in Nigeria.

## Using this template (delete this section after setup)
This repository was created from an idea-to-MVP starter kit for solo developers and teams of 2–5. It assumes a monorepo with one or more apps (e.g. web, API, mobile); a single-app project simply has one app. The kit does not provision GitHub configuration or deployment; the CI and Dependabot files ship disabled (`.example`) until real commands are configured. Here, `.github/workflows/ci.yml` runs the api and api-contract checks and `.github/dependabot.yml` updates GitHub Actions and Cargo; the jobs for apps that don't exist yet stay in `ci.yml.example`.

1. Fill in **Project setup** in the doc reviewer (see [Reviewing documents](#reviewing-documents)): name, repository, owners and apps.
2. Draft `docs/FEATURES.md` first: the users, actions, resources and surfaces, then the features built from them, grouped into journeys. An agent can draft it from a short description of the idea; a human reviews it, adding features with the reviewer's "+ Add". The approved features feed every other document.
3. Draft the rest only as deep as the first milestone needs: the PRD's vision, scope, requirements (each listing its features) and open decisions; TECHNICAL's stack and the decisions it forces (which apps exist, ADR-001 monorepo tooling); the M0 and first slice of `docs/ROADMAP.md`. Everything else stays a short stub that is filled in when its milestone is planned. Customize `AGENTS.md`, `docs/GITHUB.md` and `.github/CODEOWNERS`.
4. Review decisions, not prose: the reviewer shows the items that need a decision first (placeholders, TBDs, open decisions, proposed ADRs), and "Approve as drafted" accepts the rest of a document in one step. Each later milestone's planning reviews the document sections it needs.
5. Create the repository. For the GitHub Project, pass `--project-from <number of your template Project>` below to copy it (fields, Status options and views included), `--project <number>` to use one you made, or neither for the label fallback in `docs/GITHUB.md`. Run `bash scripts/setup-github.sh --all --apps <app1,app2,…> --approvals <0 solo, 1 team> [--project <number>] --dry-run` to see what it will do, then again without `--dry-run`. It checks your `gh` login first, then creates labels, the milestones from ROADMAP.md, the branch ruleset and the Project fields (or labels in their place), removes GitHub's unused default labels, and ends with a summary of anything left for you. The project setup review prints the exact command. Once CI runs on pull requests, re-run it with `--required-check ci-ok`.
6. In milestone M0, work through the M0 checklist in `docs/ROADMAP.md`: create each app from `templates/app/` and each shared package from `templates/package/` (see [Repository layout](#repository-layout)), set up `infra/` if the project uses infrastructure-as-code, and fill in the [Commands](#commands). Then rename `.github/workflows/ci.yml.example` to `ci.yml`, `.github/workflows/release.yml.example` to `release.yml` and `.github/dependabot.yml.example` to `dependabot.yml`, adjusting each to your apps.
7. Ask a planning agent to generate issues for the next milestone from its journeys' approved features. Review and approve each issue before execution. Tasks that share an interface can run in parallel once the interface is written down (ROADMAP.md, planning rules).
8. Implement, test, review, merge and deploy incrementally.

Tip: keep a pristine copy of the kit as a GitHub template repository (Settings → Template repository) so new projects start from "Use this template".

## Repository layout
```
apps/<app>/          Deployable apps, e.g. apps/web, apps/api, apps/android, apps/desktop
  AGENTS.md          App-specific agent instructions (stack, commands, boundaries)
  CLAUDE.md          Imports AGENTS.md for Claude Code
  CHANGELOG.md       The app's release history; each app is versioned separately
  .env.example       The app's configuration variables
packages/<name>/     Shared code, e.g. packages/api-contract, packages/design-tokens
  AGENTS.md          Purpose, consumers and change rules
  CLAUDE.md          Imports AGENTS.md for Claude Code
infra/               Infrastructure-as-code for all environments (optional)
  AGENTS.md          Infrastructure rules: plan, never apply without approval
docs/                Project-wide documents; one set for all apps
templates/app/       Starting files for a new app; copy to apps/<app>/
templates/package/   Starting files for a new package; copy to packages/<name>/
templates/infra/     Starting files for infra/
.github/workflows/   ci.yml (every PR) and release.yml (tagged releases, signed native builds)
tools/, scripts/     Repository tooling
```

App, package and infra folders are created by M0 tasks, not shipped empty. The workspace tool (pnpm/npm workspaces, Turborepo, Nx, uv, Gradle, …) is chosen in `docs/TECHNICAL.md` (ADR-001), and the infrastructure tool in ADR-002.

## Conventions
- **Placeholders:** text wrapped in double curly braces marks something to fill in. Run `bash scripts/check-placeholders.sh` to list any that remain (`templates/` and `docs/reviews/` are skipped). Write `TBD` for genuine unknowns; TBD is never an approved decision. A value that could be a setting (a threshold, interval, limit or period) isn't a TBD: write it as a configurable setting with a default (AGENTS.md).
- **Document status:** every document in `docs/` has a header with Status, Owner, Version and Last updated. Status moves Draft → In review → Approved → Superseded. Material changes to an Approved document return it to In review and bump the version.
- **IDs:** feature IDs (`U-` users, `A-` actions, `R-` resources, `S-` surfaces, `J-` journeys, `FT-` features) and requirement and artifact IDs (`OBJ-`, `FR-`, `NFR-`, `BR-`, `FLOW-`, `UI-`, `API-`, `ADR-`, `TC-`, `TASK-`) are stable. Never renumber or reuse an ID; mark removed items as deprecated instead.
- **Apps:** app names (`web`, `api`, `android`, `desktop`, …) are used consistently in folder names, `app:` labels, commit scopes (`feat(web): …`), CI job names and release tags (`web-v1.2.0`). Packages and `infra` use the same names as commit scopes.
- **Configuration:** anything in a web, mobile or desktop app's configuration ships to users and is public. Only server-side apps may hold secrets.
- **Scripts:** run scripts as `bash scripts/<name>.sh`. This works on Windows (Git Bash), macOS and Linux without needing the executable bit, which Git on Windows does not set.
- **Single sources of truth:** what the product does lives in `docs/FEATURES.md`; commands in [Commands](#commands) below; milestones in `docs/ROADMAP.md`; task status in the GitHub Project (or its labels).
- **Checks:** `bash scripts/check-placeholders.sh` and `node scripts/check-features.mjs` (FEATURES.md references, PRD links and its generated parts; `--write` regenerates them) run in CI once the `repo` job from `ci.yml.example` is added to `ci.yml` (it is left out while template placeholders remain).

## Reviewing documents
Documents drafted or changed by an agent are reviewed by a human with `tools/doc-reviewer.html`:

1. Open `tools/doc-reviewer.html` in a browser (Chrome or Edge can save files directly; other browsers download them), choose **Open project folder** and select the repository root.
2. Fill in **Project setup** first: project name, repository, owners, apps and conventions. These facts are used across many files, including CODEOWNERS, CI and the GitHub setup script; the repository and default branch are detected from `.git` where possible.
3. Go through each document. Only what changed is shown: items you accepted in an earlier review and that are unchanged since are carried forward (the review files in `docs/reviews/` record them), and boilerplate (the document header, introduction, "How this file works", "Approval") is approved with the document. **Review everything again** on a document's card shows every item. Items that need a decision, and items the agent asks you to look at, come first ("Decisions first"; switch to "Document order" in the review header). Flag or modify what needs to change, fill in short placeholders (commands, names, URLs), add missing items, and give the document a verdict. You don't have to open every item: **✓ Accept rest** (or **✓ All** next to a section heading, or **✓ Accept all** on a document's card) accepts everything you haven't decided on, and the review file notes how many items were accepted that way. **Approve as drafted** on a document's card accepts everything undecided and sets the verdict to Approved in one step; the agent then records that approval in the document. In `FEATURES.md`, **+ Add** builds a feature from the lists with dropdowns ("+ New" adds a user, action, resource, surface or journey) or takes one described in words. Open placeholders are highlighted red; those answered by setup or filled in turn green. Progress is saved in the browser, so you can stop and come back.
4. Choose **Export reviews**. The setup and each reviewed document get one file in `docs/reviews/` (e.g. `2026-11-25-project-setup.md`, `2026-11-25-PRD.md`); a document whose review hasn't changed since its last file is not saved again. Commit them.
5. Ask the agent to apply open reviews. It edits the documents and records a resolution for each entry (see AGENTS.md, "Applying doc reviews").
6. Repeat until the verdict is Approved. A verdict of Approved with changes counts as approval once the agent has applied every requested change as written; the agent then sets Status to Approved.

## Local development
- Prerequisites: {{runtimes and versions, package manager(s), database, mobile and desktop SDKs/simulators, infrastructure CLI if any}}
- Configure: for each app, copy `apps/<app>/.env.example` to `apps/<app>/.env` and supply local values. The root `.env.example` covers repository-wide tooling only.
- Local services: `docker compose up -d` starts the databases and services in `docker-compose.yml`. Put your own extra services in `docker-compose.override.yml` (gitignored; Compose loads it automatically). Defaults for ports, hosts and CORS are in `docs/TECHNICAL.md`, Local development.

## Commands
This section is the single source of truth for project commands. Other documents and agents refer here. Use "All apps" to check everything before a PR; use an app's block when working inside that app. There is one block per app, package with its own checks, and `infra`; add or delete blocks to match. Write N/A where a command doesn't apply. Keep the blocks separate (blank lines between them), so changes to different apps don't conflict.

### All apps (repo root)
Only the Rust apps exist so far; client commands are added with the first client.
- Install: `cargo fetch` (Rust toolchain pinned in rust-toolchain.toml; Docker for the database)
- Test: `docker compose up -d --wait` then `cargo test --workspace`
- E2E: N/A
- Lint: `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings`
- Typecheck: `cargo check --workspace --all-targets`
- Build: `cargo build --workspace --release`

### customer-web
- Run locally: {{command}}
- Test: {{command}}
- E2E: {{command or N/A}}
- Lint: {{command}}
- Typecheck: {{command or N/A}}
- Build: {{command}}

### customer-mobile-app
- Run locally: {{command}}
- Test: {{command}}
- E2E: {{command or N/A}}
- Lint: {{command}}
- Typecheck: {{command or N/A}}
- Build: {{command}}

### driver-web
- Run locally: {{command}}
- Test: {{command}}
- E2E: {{command or N/A}}
- Lint: {{command}}
- Typecheck: {{command or N/A}}
- Build: {{command}}

### driver-mobile-app
- Run locally: {{command}}
- Test: {{command}}
- E2E: {{command or N/A}}
- Lint: {{command}}
- Typecheck: {{command or N/A}}
- Build: {{command}}

### admin-web
- Run locally: {{command}}
- Test: {{command}}
- E2E: {{command or N/A}}
- Lint: {{command}}
- Typecheck: {{command or N/A}}
- Build: {{command}}

### admin-app
- Run locally: {{command}}
- Test: {{command}}
- E2E: {{command or N/A}}
- Lint: {{command}}
- Typecheck: {{command or N/A}}
- Build: {{command}}

### api
- Run locally: `docker compose up -d --wait` then `cargo run -p api` (listens on http://127.0.0.1:18081; configuration in apps/api/.env.example)
- Test: `cargo test -p api` (needs the docker-compose database: `docker compose up -d --wait`. `DATABASE_URL` defaults to it through `.cargo/config.toml`; database tests use `#[sqlx::test]`, which creates a throwaway database per test, so they never migrate the shared `chitty` database)
- Lint: `cargo fmt -p api --check` and `cargo clippy -p api --all-targets -- -D warnings`
- Typecheck: `cargo check -p api --all-targets`
- Build: `cargo build -p api --release`
- First admin: `cargo run -p api -- create-admin --email <email> --name <name>` prints a one-time enrolment token (no default credentials exist); the admin sends it with a new password to `POST /staff/enrolment` and completes sign-in with a code from their authenticator app
- Migrations: `cargo run -p api -- migrate` (also run on start unless `API_MIGRATE_ON_START=false`); add one as `apps/api/migrations/<YYYYMMDDHHMMSS>_<name>.sql`
- Contract: after changing a route or schema, regenerate packages/api-contract (see the api-contract block); `cargo test -p api` fails while it is stale

### api-contract
The OpenAPI 3.1 spec and Bruno collection for the api, in packages/api-contract. Run from the repository root; none of these need the database.
- Generate: `cargo run -p api -- openapi --out packages/api-contract/openapi.json` (writes the file itself; never redirect the output)
- Check: `cargo run -p api -- openapi --check` (exits non-zero when the committed spec differs from the code); `cargo test -p api --test contract` also checks that every path and method in the spec has a Bruno request
- Bruno collection: open `packages/api-contract/bruno/` in Bruno and pick the `local` environment (http://127.0.0.1:18081), or run it with the Bruno CLI: `cd packages/api-contract/bruno` then `npx @usebruno/cli run --env local` (with the api running)
- Lint, typecheck, build: covered by the api block

### worker
- Run locally: {{command}}
- Test: {{command}}
- Lint: {{command}}
- Typecheck: {{command or N/A}}
- Build: {{command}}

Infrastructure (if `infra/` exists): plan {{command}} · validate/lint {{command}}. Applying is a release step; see `docs/DEPLOYMENT.md`.

## Documentation
- [Features](docs/FEATURES.md)
- [Product requirements](docs/PRD.md)
- [Design](docs/DESIGN.md)
- [Technical specification](docs/TECHNICAL.md)
- [Roadmap](docs/ROADMAP.md)
- [Testing](docs/TESTING.md)
- [Deployment and operations](docs/DEPLOYMENT.md)
- [GitHub protocol](docs/GITHUB.md)
- [Agent instructions](AGENTS.md) (each app, package and `infra/` also has its own AGENTS.md)
- Changelogs: `apps/<app>/CHANGELOG.md`
- [Document reviews](docs/reviews/)

## License
MIT. See [LICENSE](LICENSE).
