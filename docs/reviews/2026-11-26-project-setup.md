# Review: project setup

- Reviewer: Bankole
- Date: 2026-11-26
- Completeness: 9 of 9 required facts
- Review status: Partially applied

> For agents: apply these facts to every file under "Where to apply", replacing the matching placeholders. Expand per-app tables, columns, jobs and lines to exactly the apps listed (remove the template's example apps that are not listed). Replace each "Resolution: pending" with what you changed. Set "Review status" to Applied when all entries are resolved, or Partially applied otherwise. Follow AGENTS.md "Applying doc reviews". Facts marked _Not set_ stay as placeholders.

## Facts

| Fact | Value |
|---|---|
| Project name | Chitty Logistics |
| License | MIT |
| One-line description | Allows Users to book instant or scheduled intra-city or inter-city safe and secure rides in Nigeria |
| Repository | Bankole2000/chitty_logistics |
| Default branch | main |
| GitHub Project number | _Not set_ |
| Document owner | @bankole2000 |
| Release owner | @bankole2000 |
| Default code owner | @bankole2000 |
| Branch naming | 123-short-title |
| Commit convention | Conventional Commits, scoped by app |

## Apps (8)

| App | Path | Kind | Code owner |
|---|---|---|---|
| customer-web | apps/customer-web | Web client | @bankole2000 |
| customer-mobile-app | apps/customer-mobile-app | Mobile client | @bankole2000 |
| driver-web | apps/driver-web | Web client | @bankole2000 |
| driver-mobile-app | apps/driver-mobile-app | Mobile client | @bankole2000 |
| admin-web | apps/admin-web | Web client | @bankole2000 |
| admin-app | apps/admin-app | Mobile client, Desktop client | @bankole2000 |
| api | apps/api | Server/API | @bankole2000 |
| worker | apps/worker | Server/API, Worker | @bankole2000 |

## Shared packages (0)

None listed yet.

## Where to apply (13)

### README.md

Title (project name), one-line description and License. Commands: one block per app (plus blocks for packages with their own checks), named after the app; delete the unused example blocks.

Resolution: Applied 2026-11-26. Title, one-line description and license (MIT, with a new LICENSE file naming "Chitty Logistics" as copyright holder) set; Commands now has one block per app (customer-web, customer-mobile-app, driver-web, driver-mobile-app, admin-web, admin-app, api, worker) with commands left as placeholders until the stack is chosen; the template example blocks were removed. The "Using this template" section stays until setup is finished.

### AGENTS.md

Project name in Purpose; branch naming and commit convention under "Branches and pull requests".

Resolution: Applied 2026-11-26. Project name in Purpose; branch naming `<issue number>-short-title`; commit convention Conventional Commits, scoped by app.

### Document headers in docs/*.md

Set Owner to the document owner and Last updated to the date you apply this review.

Resolution: Applied 2026-11-26. Owner @bankole2000 and Last updated 2026-11-26 in every docs/*.md header; versions bumped.

### docs/GITHUB.md

Repository, Project and Default branch under Configuration; the App labels list (one `app:<name>` per app, plus `app:packages` and `app:repo`).

Resolution: Applied 2026-11-26. Repository Bankole2000/chitty_logistics, default branch main, no Project yet (label fallback), 0 approvals; app labels for the 8 apps plus app:packages, app:infra (once ADR-002 uses it) and app:repo.

### docs/FEATURES.md

Surfaces table: one row per app (S-01, S-02 …: `apps/customer-web`, `apps/customer-mobile-app`, `apps/driver-web`, `apps/driver-mobile-app`, `apps/admin-web`, `apps/admin-app`, `apps/api`, `apps/worker`): name, `apps/<app>` path, a short description and priority Must; add SDKs or other packages people use directly as further surfaces. Leave the users, actions, resources, journeys and features for the feature review.

Resolution: Applied 2026-11-26. Surfaces now match the 8 apps: S-01 customer-web, S-02 customer-mobile-app, S-03 driver-web, S-04 driver-mobile-app, S-08 admin-web, S-09 admin-app (repurposed from "Admin desktop app" to the one desktop and mobile staff app), S-11 api, and a new S-12 worker. S-05, S-06, S-07 and S-10 are retired (IDs kept, never reused) and their features moved to S-08 and S-09. Priority is Must for each, except S-03 driver-web, which keeps the Should accepted in the FEATURES review; change it there if you want Must. Mobile app names follow this setup (customer-mobile-app, driver-mobile-app), not the "customer-app" and "driver-app" written in the FEATURES review.

### docs/TECHNICAL.md

Components table: one row per app and package (and infra if used) with name, path, kind and owner. API contracts table: one row per server app and its consumers. Leave stack and hosting placeholders for the team to decide.

Resolution: Applied 2026-11-26. Components: one row per app plus packages/api-contract (the contract AGENTS.md requires for every server app; created in M0), with owner and requirement IDs; stack and hosting left as placeholders. API contracts: api to the customer, driver and staff clients; the worker has no api of its own.

### docs/DEPLOYMENT.md

"Where each app runs": one row per app. Approval: release owner.

Resolution: Applied 2026-11-26. "Where each app runs" has one row per app with tag-based release triggers and `release-<app>` environments, hosts left as placeholders; Approval names @bankole2000 as release owner for every app.

### docs/TESTING.md

Strategy table: one row per app (and infra if used).

Resolution: Applied 2026-11-26. Strategy table has one row per app; tools left as placeholders until the stack is chosen. No infra row, as infra/ is not used yet.

### .github/CODEOWNERS

Default owner on the `*` line; one line per app and package with its code owner; owners for /infra/, docs/, AGENTS.md and .github/ (default code owner unless told otherwise); the release owner on .github/workflows/release.yml.

Resolution: Applied 2026-11-26. Default owner, one line per app, /packages/, /infra/, docs, AGENTS.md and .github/ all @bankole2000, release owner on release.yml.

### .github/workflows/ci.yml.example

One path filter and one job per app, named after the app; remove example jobs for apps not listed (keep the infra job only if infra/ is used); add every job to `ci-ok` needs; set the push branch to the default branch.

Resolution: Applied 2026-11-26. One path filter and one job per app, all in ci-ok needs; infra job removed (no infra/ yet); push branch main. Job steps stay "Fail until configured" until the README Commands exist. YAML validated.

### .github/workflows/release.yml.example

One release job per app (one per platform for mobile, one per operating system for desktop), named after the app, each with environment `release-<app>`; remove example jobs for apps not listed. Creating the environments, their secrets and required reviewers (the release owner) is a human step in GitHub settings.

Resolution: Applied 2026-11-26. Release jobs: customer-web, driver-web, admin-web, api, worker; Android and iOS jobs for customer-mobile-app, driver-mobile-app and admin-app; a desktop matrix job for admin-app; each uses environment release-<app>. Creating the environments, secrets and required reviewers is left for the human. YAML validated.

### .github/dependabot.yml.example

Keep only the ecosystems the listed apps use and point directories at their apps.

Resolution: Partially applied 2026-11-26. Kept github-actions active; the other ecosystems depend on the stack (ADR-001), so they are commented out with directories pointing at this project's apps. Uncomment the matching ones once the stack is chosen.

### GitHub labels, milestones and ruleset (not a file)

A human (or an agent explicitly authorized to change GitHub settings) runs: `bash scripts/setup-github.sh --repo Bankole2000/chitty_logistics --apps customer-web,customer-mobile-app,driver-web,driver-mobile-app,admin-web,admin-app,api,worker --ruleset --approvals 0` (0 approvals: one person owns everything, and GitHub does not let anyone approve their own pull requests). After CI runs on pull requests, re-run with `--required-check ci-ok`. Milestones come from the docs/ROADMAP.md Milestones table; fill in its slice names first. Record here who ran it.

Resolution: Not done: changing GitHub needs explicit authorization, so this is left for the human. Milestone names are now filled in docs/ROADMAP.md (M0–M6). Command (no Project, label fallback): `bash scripts/setup-github.sh --repo Bankole2000/chitty_logistics --apps customer-web,customer-mobile-app,driver-web,driver-mobile-app,admin-web,admin-app,api,worker --ruleset --approvals 0`, run first with `--dry-run`. Record here who ran it.
