# Testing and Acceptance

Status: Draft | Owner: @bankole2000 | Version: 0.3 | Last updated: 2026-11-26
References: FEATURES.md, PRD.md, TECHNICAL.md

## Strategy
Test layers per app:

| App | Unit | Integration | End-to-end | Tools |
|---|---|---|---|---|
| customer-web | Components, form validation, state | Api client against the contract | J-01, J-02, J-03 in supported browsers | {{tools}} |
| customer-mobile-app | Components, state, map and location handling | Api client against the contract | J-01, J-02, J-03 on Android and iOS devices or simulators | {{tools}} |
| driver-web | Components, form validation, document upload | Api client against the contract | J-04 and earnings and payouts in supported browsers | {{tools}} |
| driver-mobile-app | Components, state, location reporting, offer timeouts | Api client against the contract | J-04, J-05, J-06 on Android and iOS devices or simulators | {{tools}} |
| admin-web | Components, role-based screens | Api client against the contract | J-07, J-08, J-09, J-10, J-11 in supported browsers | {{tools}} |
| admin-app | Components, role-based screens | Api client against the contract | J-07, J-08, J-09, J-10, J-11 on each desktop operating system and on Android and iOS | {{tools}} |
| api | Fare and earnings calculation, matching rules, permissions per role | Endpoints against a real test database; payment callbacks with recorded provider payloads | — | {{tools}} |
| worker | Job logic: offer timeouts, scheduled-ride assignment, reminders, payout runs | Jobs against a real test database and queue | — | {{tools}} |

Contract tests: one per provider–consumer pair in the API contracts table (TECHNICAL.md), including server-to-server pairs. {{How each pair is checked against its contract package, e.g. schema validation of responses in provider tests and generated client types in consumers}}. Providers with installed-client consumers (mobile, desktop) also test against the oldest supported contract version.

Test data: isolated per environment; synthetic only.

## Acceptance tests
| ID | Requirement | Features | Apps | Scenario | Expected result |
|---|---|---|---|---|---|
| TC-000 | M0 | — | all | Fresh clone: follow the README setup, run the "All apps" checks and start every app | Every check passes; each client shows live data from the server. A human checks each client in a browser or the app, including the browser console |
| TC-001 | FR-001, FR-002 | J-01 | customer-web, customer-mobile-app, api | A new customer signs up with a phone number and SMS code, edits their name, signs out, signs in again and closes the account; also a wrong and an expired code | Account created, profile updated, sessions end on sign-out, closed account can't sign in; bad codes refused |
| TC-002 | FR-003, FR-004, FR-005, FR-006, FR-007, FR-008, FR-009, FR-011 | J-02 | customer-web, customer-mobile-app, driver-mobile-app, api, worker | A customer sees an estimate, requests a ride now, the first driver lets the offer time out, the second accepts; the customer tracks the driver, the trip completes, the customer pays by card and rates the driver; also a customer cancellation and a cash payment | Ride matched to the second driver, both notified at each step, receipt shown, payment paid only after the provider callback, rating saved; fees as the fare rules say |
| TC-003 | FR-007, FR-012, FR-013, FR-014 | J-03 | customer-web, customer-mobile-app, api, worker | A customer schedules a ride within a state and one on an inter-state route, views them, cancels one before the cut-off; a booking outside a served area is refused | Rides show as "Scheduled", driver confirmed or ride handed to operations by the lead time, reminders sent, free cancellation before the cut-off |
| TC-004 | FR-015 | J-04 | driver-web, driver-mobile-app, api | A new driver applies, uploads every document, adds a vehicle and a bank account, and checks their status | Application "Pending documents", each document "Under review", status visible to the driver |
| TC-005 | FR-004, FR-007, FR-010, FR-011, FR-016, FR-017, FR-018 | J-05 | driver-mobile-app, api, worker | An approved driver goes online, declines one offer, accepts the next, starts and completes the trip, records cash, rates the customer and checks earnings; an unapproved driver tries to go online | Ride passes to the next driver on decline; trip steps shown to both parties; earnings correct; unapproved driver refused |
| TC-006 | FR-012 | J-06 | driver-web, driver-mobile-app, api | An online driver sees open scheduled and inter-state rides and accepts one | The ride is in the driver's upcoming trips and the customer sees the driver confirmed |
| TC-007 | FR-019, FR-020, FR-021, FR-022 | J-07 | admin-web, admin-app, api | An operations agent signs in, reviews an application, declines a document, approves the driver, then suspends and reinstates them and a customer; a staff member without the operations role tries the same | Driver sees the decline reason, can go online once approved, goes offline when suspended; every change in the audit log; wrong role refused |
| TC-008 | FR-024 | J-08 | admin-web, admin-app, api | An agent watches the live map, finds a ride by plate number and filters by status, assigns a driver to an unconfirmed scheduled ride and cancels another ride | Live positions update; search and filters return the right rides; assigned driver and customer notified; cancellation reaches both parties |
| TC-009 | FR-025, FR-026, FR-027 | J-09 | customer-web, customer-mobile-app, driver-web, driver-mobile-app, admin-web, admin-app, api | A customer and a driver raise tickets; an agent assigns, replies and resolves them and refunds part of a payment; a refund above the agent's limit | Replies and statuses reach both; refund received and notified; over-limit refund refused |
| TC-010 | FR-019, FR-023, FR-028, FR-029 | J-10 | admin-web, admin-app, api | An admin signs in, creates and archives a staff account, changes a fare rule and the served areas and routes, views a report and searches the audit log | New prices apply to new bookings only; archived staff can't sign in; bookings outside served areas refused; report figures and audit results correct |
| TC-011 | FR-018 | J-11 | admin-web, admin-app, driver-web, driver-mobile-app, api, worker | A payout period ends with card and cash trips; finance views and approves the payouts | Payout amounts equal earnings less commission and cash collected; paid status after provider confirmation; driver sees the payout |
| TC-012 | FR-030, FR-031, FR-032 | J-12 | customer-web, customer-mobile-app, driver-mobile-app, admin-web, admin-app, api | A customer shares a ride and raises an SOS; a driver raises an SOS; an agent resolves both; the driver starts a trip with a wrong and then the right PIN | Contact follows the trip until it ends; SOS alerts reach operations within seconds with live location; wrong PIN refused |
| TC-013 | FR-033, FR-034 | J-13 | api, worker, driver-mobile-app | Documents that don't match official records; a failed selfie check; a ride that leaves its route | Documents flagged; driver kept offline; route alert raised for operations |
| TC-014 | FR-035 | J-14 | customer-web, customer-mobile-app, driver-mobile-app, api | A customer sends a package that a driver accepts and delivers with proof | Customer tracks the package and sees it delivered |
| TC-015 | FR-036, FR-037 | J-15 | mcp, api, customer-mobile-app | A customer connects an assistant with view and book permissions; the assistant gets an estimate and requests a ride; the customer confirms in the app; the assistant tries to cancel without cancel permission; the customer revokes the connection | Ride booked only after confirmation; cancel refused; every assistant action in the audit log; after revoking, every request is refused |
| TC-016 | FR-038 | J-16 | mcp, api, admin-web | An operations agent connects a read-only assistant, which searches tomorrow's scheduled rides with no driver and summarises open tickets; it tries to reply to a ticket and to read a report the role may not see; an admin revokes the connection | Results match the admin apps; reply and report refused; access ends when revoked |

A feature's outcome (FEATURES.md) is the starting point for its expected result. Each journey (J-) gets at least one end-to-end test that walks its features in order. Cover failure cases, validation, authorization and critical user journeys. Use synthetic data; never production secrets or personal data. A journey that crosses apps is tested end to end at least once.

## Validation commands
See the README [Commands](../README.md#commands) section. Record here only how and when each check runs (locally, in CI, before release).

## Definition of done
- [ ] Acceptance criteria met and linked to tests
- [ ] For UI changes: a human checked the change in a browser or the app, including that the browser console shows no errors or warnings (such as hydration mismatches); agents report what to check rather than checking it themselves
- [ ] Automated checks passed for every changed app and every app depending on a changed package
- [ ] No unrelated changes or exposed secrets (including in client app configuration)
- [ ] Documentation and the affected apps' CHANGELOG.md updated where user-visible
- [ ] PR reviewed and merged by authorized human

## Release acceptance
{{Per app: critical journeys, security checks, performance targets, supported browsers/devices/OS versions, known limitations, human sign-off}}

## Approval
{{Human approver, revision, date}}
