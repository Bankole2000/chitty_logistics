# Features

Status: Draft | Owner: @bankole2000 | Version: 0.5 | Last updated: 2026-11-26
References: PRD.md (requirements FR-), ROADMAP.md (milestones)
<!-- review:ignore-columns Requirements, Milestone, Status -->

## How this file works
This file is the list of everything the product lets someone do. It is drafted first and feeds the other documents: when it changes, the PRD requirements, DESIGN, TECHNICAL, TESTING and ROADMAP are updated to match, and they refer back to these IDs.

1. **Lists first.** Users, actions, resources and surfaces are short lists with stable IDs. Every feature is built from them, so the same thing always has the same name. Their details feed the other documents: users become roles and permissions, resources the data model and glossary, actions the api operations and audit events, surfaces the apps.
2. **A feature is one sentence:** *user* + *action* + *resource*, with a *scope* where it matters ("own posts only"), on one or more *surfaces*. Its *outcome* is what the user sees when it worked; it becomes the acceptance criterion.
3. **Journeys** group features in the order a user meets them. Milestones are cut from journeys, so each milestone delivers whole journeys end to end (ROADMAP.md).
4. **Status** records the decision only: `proposed`, `approved`, `deferred` or `out-of-scope`. Whether a feature is built is tracked in GitHub through its tasks, not here.
5. **Effective priority** is the lower of the feature's priority and its surface's priority (a Must feature on a Should surface is a Should there).
6. **IDs are stable:** never renumber or reuse them. Mark removed items `out-of-scope` instead of deleting them.
7. **Only humans approve.** Agents add new ideas as `proposed` and never change a status.
8. **Adding features:** open this file in `tools/doc-reviewer.html` and use "+ Add". "Build from lists" picks the user, action, resource, surfaces and journey from dropdowns ("+ New" adds a list item on the spot); "Describe in words" takes a sentence that an agent turns into a row.
9. **Generated columns:** the Requirements column (from the PRD), the journeys' Milestone column (from ROADMAP.md) and the coverage matrix are written by `node scripts/check-features.mjs --write`. Don't edit them by hand; CI fails when they are out of date.

## Users
Kinds: `person`, `app` (another program using the product), `agent` (a script or AI agent acting for someone) or `system` (the product itself: checks, calculations, scheduled jobs).

| ID | Name | Kind | Description | Priority |
|---|---|---|---|---|
| U-01 | Customer | person | Books rides in Nigeria, now or in advance, within a state or between states; pays, tracks the driver and rates the trip. | Must |
| U-02 | Driver | person | Applies with their documents and vehicle, takes ride offers, completes trips and is paid their earnings. | Must |
| U-03 | Operations agent | person | Chitty staff handling customer and driver support, driver and vehicle approvals, and live tracking of rides and vehicles. | Must |
| U-04 | Admin | person | Chitty management, HR and finance staff: staff accounts, pricing, service areas, reports and driver payouts. | Must |
| U-05 | System | system | The product itself: checks on every request, fare and earnings calculations, ride matching, notifications and scheduled jobs. | Must |
| U-06 | Payment provider | app | The payment gateway (vendor to be chosen) that takes card and transfer payments, confirms them by callback and sends payouts. | Must |
| U-07 | Trusted contact | person | Someone a customer shares a live trip with; has no Chitty account. | Should |
| U-08 | AI assistant | agent | An AI assistant (e.g. Claude) a customer or staff member has connected to Chitty through the MCP server; acts only for that person, within the permissions they granted. | Could |

## Actions
Common verbs are listed; add the ones this product needs (e.g. `requests`, `accepts`, `pays`, `invites`) and remove those it doesn't.

| ID | Verb | Meaning | Writes data |
|---|---|---|---|
| A-01 | creates | Adds a new record. | yes |
| A-02 | views | Lists records or opens one. | no |
| A-03 | edits | Changes fields of an existing record, including its status. | yes |
| A-04 | archives | Hides a record without deleting it; can be undone. | yes |
| A-05 | removes | Deletes a record permanently. | yes |
| A-06 | searches | Finds records by text. | no |
| A-07 | filters | Narrows a list to records matching a field. | no |
| A-08 | signs in | Starts a session. | yes |
| A-09 | signs out | Ends a session. | yes |
| A-10 | rejects | Refuses a request that breaks a rule, with a stable error code. | no |
| A-11 | records | Stores a record as a side effect of another action (e.g. an audit entry). | yes |
| A-12 | requests | Asks for something to happen now (e.g. a ride). | yes |
| A-13 | schedules | Books something for a later date and time. | yes |
| A-14 | cancels | Stops a booking or trip before it is completed, with a reason. | yes |
| A-15 | accepts | Takes on something offered (a ride offer). | yes |
| A-16 | declines | Turns down an offer or an application, with a reason where one is needed. | yes |
| A-17 | starts | Marks something as begun (a trip at pickup). | yes |
| A-18 | completes | Marks something as finished (a trip at drop-off). | yes |
| A-19 | pays | Settles an amount owed. | yes |
| A-20 | rates | Gives a score with an optional comment. | yes |
| A-21 | approves | Accepts a submission after review (a driver application, a payout). | yes |
| A-22 | suspends | Stops an account from being used until it is reinstated, with a reason. | yes |
| A-23 | tracks | Follows a live location on a map. | no |
| A-24 | assigns | Gives a record to a person (a ride to a driver, a ticket to an agent). | yes |
| A-25 | calculates | Works out a derived value (a fare, earnings). | no |
| A-26 | matches | Finds suitable drivers for a ride and offers it to them in turn. | yes |
| A-27 | notifies | Sends a message by push notification, SMS or email. | yes |
| A-28 | shares | Gives someone outside the product a view of a record (a link). | yes |
| A-29 | verifies | Confirms a claim is true (a phone number by code, an identity, a trip PIN). | yes |
| A-30 | refunds | Returns all or part of a payment to the payer. | yes |
| A-31 | tops up | Adds money to a balance. | yes |

## Resources
Kinds: `stored` (kept in the database) or `derived` (calculated from stored data, e.g. a report or a total).

| ID | Name | Kind | Belongs to | Description |
|---|---|---|---|---|
| R-01 | Customer account | stored | — | Name, phone number (verified), email, photo, sign-in sessions and account status. |
| R-02 | Driver profile | stored | — | A driver's account and application: name, phone, photo, application status (pending, approved, declined, suspended), availability (online or offline), rating and payout bank account. |
| R-03 | Driver document | stored | R-02 | An uploaded document under review: driver's licence, NIN, vehicle registration, insurance, roadworthiness; with expiry date and review status. |
| R-04 | Vehicle | stored | R-02 | Plate number, make, model, colour, year, seats, vehicle class and inspection status. |
| R-05 | Ride | stored | R-01 | A booking: pickup and drop-off, type (immediate or scheduled; intra-state or inter-state), pickup time, vehicle class, assigned driver and vehicle, status, fare and cancellation details. |
| R-06 | Fare estimate | derived | R-05 | The price, distance and duration shown before booking, worked out from the active fare rules. |
| R-07 | Fare rule | stored | — | Pricing settings per vehicle class and area or route: base fare, per kilometre, per minute, minimum fare, cancellation fee, inter-state route price. |
| R-08 | Payment | stored | R-05 | What the customer paid for a ride: amount, method (card, transfer, cash), status, provider reference and refunds. |
| R-09 | Driver location | stored | R-02 | The live GPS position of an online driver, kept for active rides and tracking. |
| R-10 | Rating | stored | R-05 | A 1–5 score and optional comment, given by the customer about the driver or by the driver about the customer. |
| R-11 | Support ticket | stored | — | An issue raised by a customer or driver, optionally linked to a ride: messages, status and assigned agent. |
| R-12 | Payout | stored | R-02 | A driver's settlement for a payout period: earnings, commission, cash already collected, amount, bank account and status. |
| R-13 | Driver earnings | derived | R-02 | Per trip and per period: fares less Chitty's commission, with cash collected shown separately. |
| R-14 | Staff account | stored | — | An operations or admin user: name, email, role (operations, admin, finance, HR) and status. |
| R-15 | Notification | stored | — | A push, SMS or email message sent to a customer, driver or staff member, with delivery status. |
| R-16 | Audit entry | stored | — | Who did what to which record and when, for every staff action and every change to money. |
| R-17 | Business report | derived | — | Rides, revenue, cancellations and active drivers by day and by state. |
| R-18 | Service area | stored | — | The states and cities Chitty serves, and the inter-state routes it offers. |
| R-19 | SOS alert | stored | R-05 | An emergency alert raised during a ride: who raised it, live location, status and resolution notes. |
| R-20 | Delivery order | stored | R-01 | A dispatch or delivery job: pickup, drop-off, package details, recipient and proof of delivery. |
| R-21 | Assistant connection | stored | — | Permission a customer or staff member gives an AI assistant: who granted it, what it may do (e.g. view rides, book, cancel; read-only for staff), when it was created and last used, and whether it has been revoked. |
| R-22 | Shared departure | stored | R-02 | A driver's offer of seats between states: route, departure date and time window, vehicle, seats offered and left, price per seat and status. |
| R-23 | Seat booking | stored | R-22 | A customer's booking of one or more seats on a shared departure: seats, pickup point, status and payment (R-08). |
| R-24 | Wallet | stored | R-01 | A customer's Chitty balance: top-ups, ride payments and refunds, with the current balance. |

## Surfaces
Where a feature is offered: one row per app in `apps/` (from the project setup), plus SDKs or other packages people use directly.

| ID | Name | App or package | Description | Priority |
|---|---|---|---|---|
| S-01 | Customer website | apps/customer-web | Customers book and manage rides in a browser; also hosts shared trip links. | Must |
| S-02 | Customer mobile app | apps/customer-mobile-app | Android and iOS app for customers. | Must |
| S-03 | Driver website | apps/driver-web | Drivers apply, upload documents and see earnings and payouts in a browser. | Should |
| S-04 | Driver mobile app | apps/driver-mobile-app | Android and iOS app drivers use to go online and run trips. | Must |
| S-05 | Operations website (retired) | — | Retired 2026-11-26: merged into S-08, the one website for all staff. | Won't |
| S-06 | Operations desktop app (retired) | — | Retired 2026-11-26: merged into S-09, the one desktop and mobile app for all staff. | Won't |
| S-07 | Operations mobile app (retired) | — | Retired 2026-11-26: merged into S-09, the one desktop and mobile app for all staff. | Won't |
| S-08 | Admin website | apps/admin-web | The website for all staff, with screens by role: operations (support, driver approvals, live tracking) and admin, HR, management and finance (staff, pricing, reports, payouts). | Must |
| S-09 | Admin app | apps/admin-app | One desktop and mobile app for all staff, with the same screens by role as S-08. | Must |
| S-10 | Admin mobile app (retired) | — | Retired 2026-11-26: merged into S-09, the one desktop and mobile app for all staff. | Won't |
| S-11 | Platform api | apps/api | The server every app uses: accounts, rides, matching, pricing, payments and live updates. | Must |
| S-12 | Background worker | apps/worker | Runs work no request waits for: ride-offer timeouts, scheduled-ride assignment, reminders, notification sending and payout runs. | Must |
| S-13 | MCP server | apps/mcp | Lets AI assistants act for a connected customer or staff member; calls the api through the contract like any other client. Proposed: not yet in the project setup. | Could |

## Journeys
A journey is what one user does to reach a goal, as features in order. The Milestone column is generated from ROADMAP.md, where each milestone lists the journeys it delivers.

| ID | Name | Main user | Goal | Features, in order | Milestone |
|---|---|---|---|---|---|
| J-01 | Get started as a customer | U-01 | Have a verified account and be signed in | FT-001, FT-002, FT-003, FT-004, FT-005, FT-006 | M2 |
| J-02 | Ride now | U-01 | Get a car to the destination right away and pay for it | FT-007, FT-008, FT-009, FT-010, FT-011, FT-012, FT-013, FT-014, FT-015, FT-016, FT-017, FT-018 | M2 |
| J-03 | Book a ride in advance | U-01 | Have a driver confirmed for a future trip, within a state or between states | FT-019, FT-020, FT-021, FT-022, FT-023, FT-024, FT-114 | M3 |
| J-04 | Become a driver | U-02 | Apply with documents and a vehicle, ready for approval | FT-025, FT-026, FT-027, FT-028, FT-029, FT-030 | M1 |
| J-05 | Drive and complete trips | U-02 | Take ride offers, complete trips and see earnings | FT-031, FT-032, FT-033, FT-034, FT-035, FT-036, FT-037, FT-038, FT-039, FT-040, FT-041 | M2 |
| J-06 | Take scheduled and inter-state trips | U-02 | Commit to rides booked in advance | FT-042, FT-043 | M3 |
| J-07 | Approve drivers and vehicles | U-03 | Only checked drivers and vehicles can take rides | FT-044, FT-045, FT-046, FT-047, FT-048, FT-049, FT-050 | M1 |
| J-08 | Watch live operations | U-03 | See every active ride and vehicle and step in when needed | FT-051, FT-052, FT-053, FT-054, FT-055 | M3 |
| J-09 | Support customers and drivers | U-03 | Resolve issues raised about rides and accounts | FT-056, FT-057, FT-058, FT-059, FT-060, FT-061 | M3 |
| J-10 | Run the business | U-04 | Staff, prices and service areas are set up and the business can be monitored | FT-062, FT-063, FT-064, FT-065, FT-066, FT-067, FT-068, FT-069, FT-113 | M1 |
| J-11 | Pay drivers | U-04 | Drivers are paid their earnings on time and correctly | FT-070, FT-071, FT-072, FT-073, FT-074 | M2 |
| J-12 | Ride safely | U-01 | Customers and drivers can get help and share their trip during a ride | FT-075, FT-076, FT-077, FT-078, FT-079, FT-080, FT-081 | M5 |
| J-13 | Verify drivers and watch trips automatically | U-05 | Identity and trip risks are caught without waiting for a report | FT-082, FT-083, FT-084 | M6 |
| J-14 | Send a package | U-01 | Have a package picked up and delivered (dispatch and delivery, stretch) | FT-085, FT-086, FT-087, FT-088 | M6 |
| J-15 | Ride through an AI assistant | U-01 | Check prices and book, follow and cancel rides by asking an AI assistant | FT-089, FT-090, FT-091, FT-092, FT-093, FT-094, FT-095, FT-096, FT-097 | M6 |
| J-16 | Ask operations questions through an AI assistant | U-03 | Look up rides, tickets and reports by asking an AI assistant, without it changing anything | FT-098, FT-099, FT-100, FT-101, FT-102 | M6 |
| J-17 | Book a seat between states | U-01 | Travel between states on a seat in a driver's shared departure | FT-104, FT-105, FT-111, FT-112, FT-106 | — |
| J-18 | Offer seats on an inter-state departure | U-02 | Fill a scheduled inter-state trip with passengers and complete it | FT-103, FT-107, FT-108, FT-109, FT-110 | — |
| J-19 | Pay with a Chitty wallet | U-01 | Keep a balance and pay for rides from it | FT-115, FT-116, FT-117 | — |

## Features
| ID | User | Action | Resource | Scope | Surfaces | Outcome | Priority | Depends on | Journey | Requirements | Status |
|---|---|---|---|---|---|---|---|---|---|---|---|
| FT-001 | U-01 Customer | A-01 creates | R-01 Customer account | Own, with name and Nigerian phone number | S-01, S-02 | The account is created once the phone number is verified, and the customer lands on the booking screen | Must | FT-002 | J-01 | FR-001 | approved |
| FT-002 | U-05 System | A-29 verifies | R-01 Customer account | Phone number, by one-time code sent by SMS | S-11 | The customer receives a code by SMS; a wrong or expired code is refused with a clear message | Must | — | J-01 | FR-001 | approved |
| FT-003 | U-01 Customer | A-08 signs in | R-01 Customer account | Own, by phone number and one-time code | S-01, S-02 | The customer is signed in and stays signed in on that device until they sign out | Must | FT-001 | J-01 | FR-001 | approved |
| FT-004 | U-01 Customer | A-03 edits | R-01 Customer account | Own name, email and photo | S-01, S-02 | The updated details show on the profile and to the driver on the next ride | Must | FT-003 | J-01 | FR-001 | approved |
| FT-005 | U-01 Customer | A-09 signs out | R-01 Customer account | Own session on this device | S-01, S-02 | The customer is signed out and must sign in again to book | Must | FT-003 | J-01 | FR-001 | approved |
| FT-006 | U-01 Customer | A-05 removes | R-01 Customer account | Own account | S-01, S-02 | The account is closed and personal data deleted or anonymised; ride and payment records are kept as the retention policy requires | Should | FT-003 | J-01 | FR-002 | approved |
| FT-007 | U-01 Customer | A-02 views | R-06 Fare estimate | For a chosen pickup, drop-off and vehicle class | S-01, S-02 | The customer sees the estimated fare, distance and arrival time before booking | Must | FT-008 | J-02 | FR-003 | approved |
| FT-008 | U-05 System | A-25 calculates | R-06 Fare estimate | From the active fare rules for the area, vehicle class, distance and duration | S-11 | The estimate matches the fare rules in force at booking time | Must | — | J-02 | FR-003 | approved |
| FT-009 | U-01 Customer | A-12 requests | R-05 Ride | Immediate ride within one state | S-01, S-02 | The customer sees "Finding a driver", then the assigned driver and vehicle | Must | FT-003, FT-007 | J-02 | FR-004 | approved |
| FT-010 | U-05 System | A-26 matches | R-05 Ride | Approved, online drivers of the right vehicle class nearest the pickup, one offer at a time with a configurable offer timeout and search time | S-11, S-12 | The ride goes to the nearest eligible driver who accepts; if no one accepts within the search time the customer is told no driver was found | Must | FT-009, FT-031 | J-02 | FR-004 | approved |
| FT-011 | U-01 Customer | A-02 views | R-05 Ride | Own active ride: driver name, photo and rating; vehicle plate, make, model and colour | S-01, S-02 | The customer can check the driver and car before getting in | Must | FT-010 | J-02 | FR-005 | approved |
| FT-012 | U-01 Customer | A-23 tracks | R-09 Driver location | The driver assigned to own active ride | S-01, S-02 | The customer sees the driver approaching on a map with an updated arrival time, and the route during the trip | Must | FT-010, FT-032 | J-02 | FR-005 | approved |
| FT-013 | U-01 Customer | A-14 cancels | R-05 Ride | Own ride, before the trip starts | S-01, S-02 | The ride is cancelled, the driver is told, and any cancellation fee from the fare rules is shown | Must | FT-009 | J-02 | FR-007 | approved |
| FT-014 | U-05 System | A-27 notifies | R-15 Notification | Customer and driver, on ride status changes (accepted, driver arriving, started, completed, cancelled); push first, SMS when push fails | S-11, S-12 | Both parties hear about each status change within seconds | Must | FT-009 | J-02 | FR-006 | approved |
| FT-015 | U-01 Customer | A-19 pays | R-08 Payment | Own ride, by card or bank transfer through the payment provider, or cash to the driver | S-01, S-02 | The customer sees a receipt with the fare breakdown | Must | FT-036 | J-02 | FR-008 | approved |
| FT-016 | U-06 Payment provider | A-03 edits | R-08 Payment | Payment status, by signed callback | S-11 | A payment is marked paid only after a verified callback; duplicate callbacks change nothing | Must | FT-015 | J-02 | FR-008 | approved |
| FT-017 | U-01 Customer | A-20 rates | R-10 Rating | The driver of own completed ride | S-01, S-02 | The rating is saved and counts towards the driver's average | Must | FT-036 | J-02 | FR-009 | approved |
| FT-018 | U-01 Customer | A-02 views | R-05 Ride | Own past rides and receipts | S-01, S-02 | The customer sees each past ride with route, driver, fare and payment status | Must | FT-009 | J-02 | FR-011 | approved |
| FT-019 | U-01 Customer | A-13 schedules | R-05 Ride | Within one state, for a date and time at least the configurable minimum lead time ahead | S-01, S-02 | The booking shows as "Scheduled" with its pickup time and estimated fare | Must | FT-003, FT-007 | J-03 | FR-012 | approved |
| FT-020 | U-01 Customer | A-13 schedules | R-05 Ride | Between states, on an inter-state route Chitty offers | S-01, S-02 | The booking shows as "Scheduled" with the route price | Must | FT-019, FT-068 | J-03 | FR-013 | approved |
| FT-021 | U-01 Customer | A-02 views | R-05 Ride | Own upcoming scheduled rides | S-01, S-02 | The customer sees each upcoming ride, its pickup time and whether a driver is confirmed | Must | FT-019 | J-03 | FR-012 | approved |
| FT-022 | U-01 Customer | A-14 cancels | R-05 Ride | Own scheduled ride; free before the configurable cut-off, fee after | S-01, S-02 | The ride is cancelled and the customer sees whether a fee applied | Must | FT-019 | J-03 | FR-007 | approved |
| FT-023 | U-05 System | A-24 assigns | R-05 Ride | Scheduled ride, to a driver who accepted it; hands it to operations if none has by the configurable lead time | S-11, S-12 | Every scheduled ride has a confirmed driver before pickup, or operations is alerted in time | Must | FT-043 | J-03 | FR-012 | approved |
| FT-024 | U-05 System | A-27 notifies | R-15 Notification | Customer and driver, a configurable time before a scheduled pickup | S-11, S-12 | Both receive a reminder with the pickup time, place, driver and vehicle | Should | FT-023 | J-03 | FR-014 | approved |
| FT-025 | U-02 Driver | A-01 creates | R-02 Driver profile | Own, with verified phone number | S-03, S-04 | The application is created with status "Pending documents" | Must | — | J-04 | FR-015 | approved |
| FT-026 | U-02 Driver | A-01 creates | R-03 Driver document | Own: driver's licence, NIN, photo, vehicle registration, insurance | S-03, S-04 | Each uploaded document shows "Under review" | Must | FT-025 | J-04 | FR-015 | approved |
| FT-027 | U-02 Driver | A-01 creates | R-04 Vehicle | Own: plate, make, model, colour, year, seats, class | S-03, S-04 | The vehicle is added to the application for review | Must | FT-025 | J-04 | FR-015 | approved |
| FT-028 | U-02 Driver | A-02 views | R-02 Driver profile | Own application status and the reasons for any declined document | S-03, S-04 | The driver sees what is approved, pending or needs re-uploading | Must | FT-026 | J-04 | FR-015 | approved |
| FT-029 | U-02 Driver | A-08 signs in | R-02 Driver profile | Own, by phone number and one-time code | S-03, S-04 | The driver is signed in and sees their application or the driving screen | Must | FT-025 | J-04 | FR-015 | approved |
| FT-030 | U-02 Driver | A-03 edits | R-02 Driver profile | Own payout bank account | S-03, S-04 | The bank account is saved after the account name is checked | Must | FT-029 | J-04 | FR-015 | approved |
| FT-031 | U-02 Driver | A-03 edits | R-02 Driver profile | Own availability (online or offline); only when approved | S-04 | The driver goes online and can receive ride offers, or goes offline and stops receiving them | Must | FT-046 | J-05 | FR-016 | approved |
| FT-032 | U-05 System | A-11 records | R-09 Driver location | Online drivers, at a configurable interval | S-11 | The latest position of every online driver is available for matching and tracking | Must | FT-031 | J-05 | FR-016 | approved |
| FT-033 | U-02 Driver | A-15 accepts | R-05 Ride | A ride offered to them, within the offer timeout | S-04 | The driver sees the pickup and navigation; the customer sees the driver assigned | Must | FT-010 | J-05 | FR-004 | approved |
| FT-034 | U-02 Driver | A-16 declines | R-05 Ride | A ride offered to them | S-04 | The offer closes and the ride is offered to the next driver | Must | FT-010 | J-05 | FR-004 | approved |
| FT-035 | U-02 Driver | A-17 starts | R-05 Ride | Own assigned ride, at the pickup | S-04 | The ride shows "In progress" to both parties and the trip is measured | Must | FT-033 | J-05 | FR-017 | approved |
| FT-036 | U-02 Driver | A-18 completes | R-05 Ride | Own ride in progress, at the drop-off | S-04 | The final fare is shown to both parties and payment is requested | Must | FT-035 | J-05 | FR-017 | approved |
| FT-037 | U-02 Driver | A-03 edits | R-08 Payment | Own completed ride paid in cash: marks the cash as received | S-04 | The ride is marked paid in cash and counts against the driver's next payout | Must | FT-036 | J-05 | FR-008 | approved |
| FT-038 | U-02 Driver | A-20 rates | R-10 Rating | The customer of own completed ride | S-04 | The rating is saved and counts towards the customer's average | Should | FT-036 | J-05 | FR-010 | approved |
| FT-039 | U-02 Driver | A-14 cancels | R-05 Ride | Own assigned ride, before it starts, with a reason | S-04 | The customer is told and the ride goes back to matching | Must | FT-033 | J-05 | FR-007 | approved |
| FT-040 | U-02 Driver | A-02 views | R-13 Driver earnings | Own, per trip, day and week | S-03, S-04 | The driver sees fares, commission and cash collected for the period | Must | FT-070 | J-05 | FR-018 | approved |
| FT-041 | U-02 Driver | A-02 views | R-05 Ride | Own past trips | S-03, S-04 | The driver sees each past trip with route, fare and rating | Must | FT-036 | J-05 | FR-011 | approved |
| FT-042 | U-02 Driver | A-02 views | R-05 Ride | Scheduled and inter-state rides open in their state or on their routes | S-03, S-04 | The driver sees open bookings with pickup time, route and fare | Must | FT-019, FT-031 | J-06 | FR-012 | approved |
| FT-043 | U-02 Driver | A-15 accepts | R-05 Ride | An open scheduled ride | S-03, S-04 | The ride appears in the driver's upcoming trips and the customer sees the driver confirmed | Must | FT-042 | J-06 | FR-012 | approved |
| FT-044 | U-03 Operations agent | A-08 signs in | R-14 Staff account | Own, with email, password and second factor | S-08, S-09 | The agent sees the operations screens their role allows | Must | FT-063 | J-07 | FR-019 | approved |
| FT-045 | U-03 Operations agent | A-02 views | R-02 Driver profile | Pending applications with their documents and vehicle | S-08, S-09 | The agent sees each application with documents ready to check | Must | FT-026, FT-027, FT-044 | J-07 | FR-020 | approved |
| FT-046 | U-03 Operations agent | A-21 approves | R-02 Driver profile | After checking every document and the vehicle | S-08, S-09 | The driver is told they are approved and can go online | Must | FT-045 | J-07 | FR-020 | approved |
| FT-047 | U-03 Operations agent | A-16 declines | R-02 Driver profile | An application or a single document, with a reason the driver sees | S-08, S-09 | The driver sees the reason and can re-upload | Must | FT-045 | J-07 | FR-020 | approved |
| FT-048 | U-03 Operations agent | A-22 suspends | R-02 Driver profile | Any driver, with a reason; can be reinstated | S-08, S-09 | The driver goes offline and cannot go online until reinstated | Must | FT-046 | J-07 | FR-020 | approved |
| FT-049 | U-05 System | A-11 records | R-16 Audit entry | Every staff action that changes a driver, customer, ride, payment, fare rule, payout or staff account | S-11 | Each such change has an entry naming who, what, when and the reason | Must | — | J-07 | FR-022 | approved |
| FT-050 | U-03 Operations agent | A-22 suspends | R-01 Customer account | Any customer, with a reason; can be reinstated | S-08, S-09 | The customer cannot book until reinstated | Should | FT-044 | J-07 | FR-021 | approved |
| FT-051 | U-03 Operations agent | A-23 tracks | R-09 Driver location | All online drivers and active rides, on one map | S-08, S-09 | The agent sees every online vehicle and active ride live, and can open any of them | Must | FT-032, FT-044 | J-08 | FR-024 | approved |
| FT-052 | U-03 Operations agent | A-06 searches | R-05 Ride | By ride ID, customer or driver phone number, or plate number | S-08, S-09 | The agent finds the ride and sees its full history | Must | FT-044 | J-08 | FR-024 | approved |
| FT-053 | U-03 Operations agent | A-07 filters | R-05 Ride | By status, type (immediate, scheduled, inter-state), state and date | S-08, S-09 | The ride list shows only matching rides | Must | FT-044 | J-08 | FR-024 | approved |
| FT-054 | U-03 Operations agent | A-24 assigns | R-05 Ride | A scheduled ride with no confirmed driver, to an approved driver | S-08, S-09 | The driver and customer are told the driver is confirmed | Must | FT-023 | J-08 | FR-024 | approved |
| FT-055 | U-03 Operations agent | A-14 cancels | R-05 Ride | Any ride not yet completed, on behalf of customer or driver, with a reason | S-08, S-09 | Both parties are told and no cancellation fee is charged unless the agent chooses one | Must | FT-052 | J-08 | FR-024 | approved |
| FT-056 | U-01 Customer | A-01 creates | R-11 Support ticket | About own ride, payment or account | S-01, S-02 | The customer sees the ticket as "Open" and is told when it is answered | Must | FT-003 | J-09 | FR-025 | approved |
| FT-057 | U-02 Driver | A-01 creates | R-11 Support ticket | About own trip, earnings, payout or account | S-03, S-04 | The driver sees the ticket as "Open" and is told when it is answered | Must | FT-029 | J-09 | FR-025 | approved |
| FT-058 | U-03 Operations agent | A-02 views | R-11 Support ticket | Open tickets, with the linked ride and account | S-08, S-09 | The agent sees the queue, oldest first, with full context | Must | FT-044 | J-09 | FR-025 | approved |
| FT-059 | U-03 Operations agent | A-03 edits | R-11 Support ticket | Replies and status (open, waiting, resolved) | S-08, S-09 | The customer or driver sees the reply and the new status | Must | FT-058 | J-09 | FR-025 | approved |
| FT-060 | U-03 Operations agent | A-24 assigns | R-11 Support ticket | To an operations agent | S-08, S-09 | The ticket shows in that agent's queue | Should | FT-058 | J-09 | FR-026 | approved |
| FT-061 | U-03 Operations agent | A-30 refunds | R-08 Payment | All or part of a card or transfer payment, with a reason, up to a configurable limit per agent | S-08, S-09 | The customer receives the refund and a notification | Should | FT-016 | J-09 | FR-027 | approved |
| FT-062 | U-04 Admin | A-08 signs in | R-14 Staff account | Own, with email, password and second factor | S-08, S-09 | The admin sees the screens their role allows | Must | — | J-10 | FR-019 | approved |
| FT-063 | U-04 Admin | A-01 creates | R-14 Staff account | Operations and admin staff, with a role (operations, admin, finance, HR) | S-08, S-09 | The new staff member receives an invitation and can only use what their role allows | Must | FT-062 | J-10 | FR-019 | approved |
| FT-064 | U-04 Admin | A-04 archives | R-14 Staff account | Staff who have left; can be restored | S-08, S-09 | The person can no longer sign in; their past actions stay in the audit log | Must | FT-063 | J-10 | FR-019 | approved |
| FT-065 | U-04 Admin | A-03 edits | R-07 Fare rule | Base fare, per kilometre, per minute, minimum fare, cancellation fee, by vehicle class and area; inter-state route prices | S-08, S-09 | New bookings use the new prices; rides already booked keep their price | Must | FT-062 | J-10 | FR-028 | approved |
| FT-066 | U-04 Admin | A-02 views | R-17 Business report | Rides, revenue, cancellations and active drivers by day and state | S-08, S-09 | The admin sees the figures for a chosen period | Should | FT-062 | J-10 | FR-029 | approved |
| FT-067 | U-04 Admin | A-06 searches | R-16 Audit entry | By staff member, record and date | S-08, S-09 | The admin sees who changed what and when | Should | FT-049 | J-10 | FR-023 | approved |
| FT-068 | U-04 Admin | A-03 edits | R-18 Service area | Which states and cities are served and which inter-state routes are offered | S-08, S-09 | Customers can book only in served areas and on offered routes | Must | FT-062 | J-10 | FR-028 | approved |
| FT-069 | U-05 System | A-10 rejects | R-05 Ride | Bookings outside a served area or offered route | S-11 | The customer is told the area or route is not served yet | Must | FT-068 | J-10 | FR-028 | approved |
| FT-070 | U-05 System | A-25 calculates | R-13 Driver earnings | Per trip: fare less Chitty's configurable commission, with cash collected recorded separately | S-11 | Each completed trip adds its earnings to the driver's current period | Must | FT-036 | J-11 | FR-018 | approved |
| FT-071 | U-05 System | A-01 creates | R-12 Payout | Per driver, at the end of each configurable payout period (default weekly) | S-11, S-12 | A pending payout exists for each driver with earnings in the period | Must | FT-070 | J-11 | FR-018 | approved |
| FT-072 | U-04 Admin | A-02 views | R-12 Payout | Pending payouts with their trip breakdown; finance role | S-08, S-09 | The admin sees each payout amount and how it was worked out | Must | FT-071 | J-11 | FR-018 | approved |
| FT-073 | U-04 Admin | A-21 approves | R-12 Payout | Pending payouts; finance role | S-08, S-09 | The money is sent through the payment provider and the payout is marked paid when confirmed | Must | FT-072, FT-030 | J-11 | FR-018 | approved |
| FT-074 | U-02 Driver | A-02 views | R-12 Payout | Own payouts and their status | S-03, S-04 | The driver sees each payout, its amount and when it was paid | Must | FT-071 | J-11 | FR-018 | approved |
| FT-075 | U-01 Customer | A-28 shares | R-05 Ride | Own active ride, as a link sent to a trusted contact | S-01, S-02 | The contact receives a link that works until the trip ends | Should | FT-011 | J-12 | FR-030 | approved |
| FT-076 | U-07 Trusted contact | A-02 views | R-05 Ride | A ride shared with them: driver, vehicle and live location, until the trip ends | S-01 | The contact follows the trip live without an account | Should | FT-075 | J-12 | FR-030 | approved |
| FT-077 | U-01 Customer | A-01 creates | R-19 SOS alert | During own active ride | S-01, S-02 | Operations sees the alert with the live location at once, and the customer sees that help is on the way | Should | FT-011 | J-12 | FR-031 | approved |
| FT-078 | U-02 Driver | A-01 creates | R-19 SOS alert | During own active trip | S-04 | Operations sees the alert with the live location at once, and the driver sees that help is on the way | Should | FT-035 | J-12 | FR-031 | approved |
| FT-079 | U-03 Operations agent | A-02 views | R-19 SOS alert | Open alerts with live location, ride, driver and customer details | S-08, S-09 | The agent sees each alert as soon as it is raised and can call both parties | Should | FT-077 | J-12 | FR-031 | approved |
| FT-080 | U-05 System | A-29 verifies | R-05 Ride | A trip PIN: the driver can start a trip only after entering the customer's PIN | S-11 | A trip cannot start with the wrong customer or the wrong driver | Should | FT-035 | J-12 | FR-032 | approved |
| FT-081 | U-03 Operations agent | A-03 edits | R-19 SOS alert | Resolves an alert with notes | S-08, S-09 | The alert is closed with a record of what was done | Should | FT-079 | J-12 | FR-031 | approved |
| FT-082 | U-05 System | A-29 verifies | R-03 Driver document | NIN and driver's licence, with a government verification service | S-11 | Documents that don't match official records are flagged before an agent reviews them | Could | FT-026 | J-13 | FR-033 | approved |
| FT-083 | U-05 System | A-29 verifies | R-02 Driver profile | The driver's identity by selfie against their profile photo before going online, at a configurable interval | S-11 | Someone other than the approved driver cannot go online | Could | FT-031 | J-13 | FR-033 | approved |
| FT-084 | U-05 System | A-01 creates | R-19 SOS alert | When a ride leaves its expected route or stops for longer than a configurable time | S-11, S-12 | Operations is alerted without anyone pressing SOS | Could | FT-079 | J-13 | FR-034 | approved |
| FT-085 | U-01 Customer | A-01 creates | R-20 Delivery order | Own: package details, pickup, drop-off and recipient | S-01, S-02 | The customer sees the delivery price and the order as "Finding a driver" | Won't | FT-003 | J-14 | FR-035 | approved |
| FT-086 | U-02 Driver | A-15 accepts | R-20 Delivery order | A delivery offered to them | S-04 | The driver sees the pickup and package details | Won't | FT-085 | J-14 | FR-035 | approved |
| FT-087 | U-02 Driver | A-18 completes | R-20 Delivery order | Own delivery, with proof of delivery (photo or recipient code) | S-04 | The customer and recipient see the package as delivered | Won't | FT-086 | J-14 | FR-035 | approved |
| FT-088 | U-01 Customer | A-23 tracks | R-09 Driver location | The driver carrying own delivery | S-01, S-02 | The customer sees the package on its way on a map | Won't | FT-086 | J-14 | FR-035 | approved |
| FT-089 | U-01 Customer | A-01 creates | R-21 Assistant connection | Own; the customer chooses what the assistant may do (view rides, get prices, book, cancel) | S-01, S-02 | The assistant is connected with only the chosen permissions, and the connection shows in the customer's settings | Could | FT-003 | J-15 | FR-036 | approved |
| FT-090 | U-08 AI assistant | A-02 views | R-06 Fare estimate | For the connected customer, for a pickup, drop-off and vehicle class | S-13 | The assistant gets the same estimate the customer would see in the app | Could | FT-089, FT-008 | J-15 | FR-037 | approved |
| FT-091 | U-08 AI assistant | A-12 requests | R-05 Ride | Immediate ride for the connected customer; sent only after the customer confirms the pickup, drop-off and price in the Chitty app | S-13 | The ride is requested after the customer confirms; without confirmation nothing is booked | Could | FT-090, FT-009 | J-15 | FR-037 | approved |
| FT-092 | U-08 AI assistant | A-13 schedules | R-05 Ride | Scheduled ride for the connected customer, with the same confirmation in the Chitty app | S-13 | The ride shows as "Scheduled" after the customer confirms | Could | FT-090, FT-019 | J-15 | FR-037 | approved |
| FT-093 | U-08 AI assistant | A-02 views | R-05 Ride | The connected customer's active and upcoming rides, with driver and vehicle details and status | S-13 | The assistant can tell the customer who is coming, in which car, and when | Could | FT-089, FT-011 | J-15 | FR-037 | approved |
| FT-094 | U-08 AI assistant | A-14 cancels | R-05 Ride | The connected customer's ride, after the customer confirms in the Chitty app with any cancellation fee shown | S-13 | The ride is cancelled only after confirmation, and the customer saw the fee first | Could | FT-093, FT-013 | J-15 | FR-037 | approved |
| FT-095 | U-01 Customer | A-05 removes | R-21 Assistant connection | Own connection | S-01, S-02 | The assistant loses access at once | Could | FT-089 | J-15 | FR-036 | approved |
| FT-096 | U-05 System | A-11 records | R-16 Audit entry | Every action an assistant takes, tagged with its connection | S-11, S-13 | Each assistant action can be traced to the connection and the person who granted it | Could | FT-089 | J-15 | FR-036 | approved |
| FT-097 | U-05 System | A-10 rejects | R-21 Assistant connection | Assistant requests outside the granted permissions, from a revoked connection, or above a configurable rate limit | S-11, S-13 | The assistant gets a clear error and nothing changes | Could | FT-089 | J-15 | FR-036 | approved |
| FT-098 | U-03 Operations agent | A-01 creates | R-21 Assistant connection | Own; read-only, limited to what the staff member's role may see | S-08, S-09 | The assistant is connected read-only, and the connection shows in the staff member's settings | Could | FT-044 | J-16 | FR-038 | approved |
| FT-099 | U-08 AI assistant | A-06 searches | R-05 Ride | For the connected staff member, within their role (e.g. scheduled rides tomorrow with no driver) | S-13 | The assistant returns the same rides the staff member would find in the admin apps | Could | FT-098, FT-052 | J-16 | FR-038 | approved |
| FT-100 | U-08 AI assistant | A-02 views | R-11 Support ticket | Open tickets, for the connected staff member, within their role | S-13 | The assistant can summarise the ticket queue without replying or changing status | Could | FT-098, FT-058 | J-16 | FR-038 | approved |
| FT-101 | U-08 AI assistant | A-02 views | R-17 Business report | For connected staff whose role may see reports | S-13 | The assistant answers questions about rides, revenue and drivers with the report figures | Could | FT-098, FT-066 | J-16 | FR-038 | approved |
| FT-102 | U-03 Operations agent | A-05 removes | R-21 Assistant connection | Own connection; admins can also revoke any staff connection | S-08, S-09 | The assistant loses access at once | Could | FT-098 | J-16 | FR-038 | approved |
| FT-103 | U-02 Driver | A-01 creates | R-22 Shared departure | Own, on an offered inter-state route: departure date and time window, vehicle, seats offered; price per seat from the route's fare rule | S-03, S-04 | The departure is open for booking and shows the seats left | Must | FT-046, FT-068 | J-18 | — (new) | proposed |
| FT-104 | U-01 Customer | A-02 views | R-22 Shared departure | Open departures on a route and date: time window, seats left, price per seat, driver and vehicle | S-01, S-02 | The customer sees which departures still have seats | Must | FT-103 | J-17 | — (new) | proposed |
| FT-105 | U-01 Customer | A-01 creates | R-23 Seat booking | One or more seats on an open departure, with a pickup point | S-01, S-02 | The seats are reserved, the seats left go down, and the booking shows as "Booked" | Must | FT-003, FT-104 | J-17 | — (new) | proposed |
| FT-106 | U-01 Customer | A-14 cancels | R-23 Seat booking | Own booking; free before the configurable cut-off, fee after | S-01, S-02 | The seats are released and the customer sees whether a fee applied | Must | FT-105 | J-17 | — (new) | proposed |
| FT-107 | U-02 Driver | A-02 views | R-23 Seat booking | Bookings on own departure: passengers, seats and pickup points | S-03, S-04 | The driver sees who to pick up and where | Must | FT-105 | J-18 | — (new) | proposed |
| FT-108 | U-02 Driver | A-17 starts | R-22 Shared departure | Own departure, within its time window | S-04 | Booked passengers see the departure "In progress" and can track it | Must | FT-107 | J-18 | — (new) | proposed |
| FT-109 | U-02 Driver | A-18 completes | R-22 Shared departure | Own departure in progress, at the destination | S-04 | Every booking on the departure is completed and payment is requested where still due | Must | FT-108 | J-18 | — (new) | proposed |
| FT-110 | U-02 Driver | A-14 cancels | R-22 Shared departure | Own departure before it starts, with a reason | S-03, S-04 | Every booked passenger is told, offered other departures and refunded any amount paid | Must | FT-103 | J-18 | — (new) | proposed |
| FT-111 | U-05 System | A-10 rejects | R-23 Seat booking | Bookings for more seats than are left, or after the configurable booking cut-off | S-11 | The customer is told the departure is full or closed | Must | FT-105 | J-17 | — (new) | proposed |
| FT-112 | U-01 Customer | A-19 pays | R-08 Payment | Own seat booking, by any supported method (card, transfer, wallet or cash) | S-01, S-02 | The customer sees a receipt for the seats | Must | FT-105, FT-016 | J-17 | — (new) | proposed |
| FT-113 | U-04 Admin | A-03 edits | R-07 Fare rule | Payment at booking for inter-state rides and seat bookings: none, a deposit (percentage) or the full fare; default none | S-08, S-09 | New inter-state bookings ask for the configured amount at booking | Must | FT-062 | J-10 | — (new) | proposed |
| FT-114 | U-01 Customer | A-19 pays | R-08 Payment | A deposit or the full fare at booking, for an inter-state ride or seat booking, when the setting requires it | S-01, S-02 | The booking is confirmed only after the payment succeeds | Must | FT-113, FT-020, FT-016 | J-03 | — (new) | proposed |
| FT-115 | U-01 Customer | A-31 tops up | R-24 Wallet | Own wallet, by card or bank transfer through the payment provider | S-01, S-02 | The new balance shows once the provider confirms the payment | Must | FT-003, FT-016 | J-19 | — (new) | proposed |
| FT-116 | U-01 Customer | A-02 views | R-24 Wallet | Own balance and transactions | S-01, S-02 | The customer sees the balance and every top-up, payment and refund | Must | FT-115 | J-19 | — (new) | proposed |
| FT-117 | U-01 Customer | A-19 pays | R-08 Payment | Own ride or seat booking, from the wallet balance | S-01, S-02 | The fare is taken from the balance and a receipt is shown; a balance that is too low is refused with the amount missing | Must | FT-115, FT-036 | J-19 | — (new) | proposed |

## Coverage matrix
Generated from the Features table (approved and proposed features only); don't edit it by hand. Each cell lists the actions a user can take on a resource. An empty cell is not an error, but it is worth a look: who can change or remove this resource?

<!-- matrix:start -->
| User \ Resource | Customer account | Driver profile | Driver document | Vehicle | Ride | Fare estimate | Fare rule | Payment | Driver location | Rating | Support ticket | Payout | Driver earnings | Staff account | Notification | Audit entry | Business report | Service area | SOS alert | Delivery order | Assistant connection | Shared departure | Seat booking | Wallet |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| U-01 Customer | creates, signs in, edits, signs out, removes |  |  |  | requests, views, cancels, schedules, shares | views |  | pays | tracks | rates | creates |  |  |  |  |  |  |  | creates | creates | creates, removes | views | creates, cancels | tops up, views |
| U-02 Driver |  | creates, views, signs in, edits | creates | creates | accepts, declines, starts, completes, cancels, views |  |  | edits |  | rates | creates | views | views |  |  |  |  |  | creates | accepts, completes |  | creates, starts, completes, cancels | views |  |
| U-03 Operations agent | suspends | views, approves, declines, suspends |  |  | searches, filters, assigns, cancels |  |  | refunds | tracks |  | views, edits, assigns |  |  | signs in |  |  |  |  | views, edits |  | creates, removes |  |  |  |
| U-04 Admin |  |  |  |  |  |  | edits |  |  |  |  | views, approves |  | signs in, creates, archives |  | searches | views | edits |  |  |  |  |  |  |
| U-05 System | verifies | verifies | verifies |  | matches, assigns, rejects, verifies | calculates |  |  | records |  |  | creates | calculates |  | notifies | records |  |  | creates |  | rejects |  | rejects |  |
| U-06 Payment provider |  |  |  |  |  |  |  | edits |  |  |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| U-07 Trusted contact |  |  |  |  | views |  |  |  |  |  |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| U-08 AI assistant |  |  |  |  | requests, schedules, views, cancels, searches | views |  |  |  |  | views |  |  |  |  |  | views |  |  |  |  |  |  |  |
<!-- matrix:end -->

## Gaps
Questions found while building the lists that the features don't answer yet. Each needs a decision: add a feature (as `proposed`), or note here that it is intentionally missing.

- **Number of apps.** Decided 2026-11-26 (docs/reviews/2026-11-26-FEATURES.md): 8 apps. Customer website and mobile app, driver website and mobile app, one admin website (S-08) and one admin desktop and mobile app (S-09) that serve both operations and admin staff by role, the api (S-11) and a background worker (S-12). The separate operations apps and the admin mobile app (S-05, S-06, S-07, S-10) are retired.
- **How inter-state trips work.** Decided 2026-11-26 (docs/reviews/2026-11-26-FEATURES-2.md): customers can book both a private car (FT-020) and seats on a driver's shared departure (J-17, J-18, proposed). Inter-state trips are scheduled only, with pickup within a time window (configurable). Still open: reimbursing drivers' return trips, for management to discuss; and whether drivers may set their own seat price (drafted as fixed by the route's fare rule).
- **Payment methods and timing.** Decided 2026-11-26 (docs/reviews/2026-11-26-FEATURES-2.md): the MVP supports every method: cash, card, bank transfer and a Chitty wallet (J-19, proposed). Whether inter-state bookings need a deposit or full payment at booking is an admin setting (FT-113, FT-114, proposed; default none). Payment providers: Paystack and OPay. Still open: which provider handles which method, and which sends payouts.
- **Cash and commission.** On cash trips the driver holds Chitty's commission. Drafted as deducted from the next payout (FT-037, FT-070). What happens when a driver's cash collected exceeds what Chitty owes them? Deferred 2026-11-26: for management to answer.
- **Contact between customer and driver.** In-app calling or chat with masked phone numbers is basic safety and isn't drafted yet. Add it as a feature?
- **Staff roles.** Drafted roles: operations, admin, finance, HR. Answered 2026-11-26: HR in the product does not keep employee records. Deferred for management: what HR does in the product beyond that, and whether drivers count as staff.
- **Vehicle owners and fleets.** Some drivers drive vehicles owned by someone else (fleet partners). Is a fleet owner a user who adds vehicles and drivers and receives payouts?
- **Booking for someone else, saved places, promotions, referrals and tips.** None are drafted. Add any as features?
- **Ratings with consequences.** Should a low average rating put a driver or customer up for review automatically?
- **Data protection.** The Nigeria Data Protection Act 2023 applies to customers' and drivers' personal data, location history and NIN. Retention periods and consent belong in the PRD (NFR-) and need `review:privacy`.
- **AI assistants (MCP server, J-15, J-16).** Drafted 2026-11-26 as proposed, priority Could. Open: whether to offer the customer server, the staff server or both (they could also be two separate servers); how a customer confirms an assistant's booking or cancellation in the Chitty app (push prompt or in-app screen); the sign-in flow assistants use to connect (OAuth); and which milestone, likely after M4 (launch). Payments, refunds and payouts are deliberately left out for assistants. Adding `apps/mcp` also needs a project setup review entry (README Commands, CI, CODEOWNERS, `app:mcp` label).

## Not features
Requirements that are not user + action + resource stay in the PRD: quality targets (NFR-), constraints, and "works on every surface" requirements, which the Surfaces list covers.

## Approval
{{Human approver, revision, date}}
