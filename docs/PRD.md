# Product Requirements Document

Status: Draft | Owner: @bankole2000 | Version: 0.4 | Last updated: 2026-11-27
<!-- review:focus "Email provider for staff invitations" -->
References: FEATURES.md (users U-, resources R-, surfaces S-, journeys J-, features FT-)

FEATURES.md feeds this document. When features are approved or changed there, the requirements below are updated to match, and each requirement lists the features it covers.

## Vision and problem
Chitty Logistics lets people in Nigeria book safe and secure rides, either right away or in advance, within a state or between states. Customers book on the web or a mobile app. Drivers apply, are checked and approved, and take trips through their own website and mobile app. Chitty's staff support customers and drivers, approve drivers and vehicles, track rides live, set prices and pay drivers.

Safety for customers and drivers is Chitty's main value proposition. The MVP delivers the basic ride service with the safety basics built in: checked drivers and vehicles, driver and vehicle details shown before pickup, live tracking, ratings and an audit trail of staff actions. Trip sharing, SOS alerts and trip PINs follow in a later milestone, then automated verification and trip monitoring. Dispatch and delivery are a stretch goal.

## Users
The user list (U-) lives in FEATURES.md. Summarize here who they are and what each needs most.

- **Customers (U-01)** need to book a ride quickly, see the price before booking, check the driver and car before getting in, follow the trip and pay easily.
- **Drivers (U-02)** need a simple application, a steady flow of ride offers, clear trip steps and correct, on-time payouts.
- **Operations agents (U-03)** need to approve drivers and vehicles, see every active ride and vehicle live, step in on rides and answer support tickets.
- **Admins (U-04)**, including management, HR and finance, need to manage staff accounts, prices and service areas, see how the business is doing and approve driver payouts.
- **The system (U-05)** calculates fares and earnings, matches rides, sends notifications and runs scheduled jobs; **the payment provider (U-06)** confirms payments and sends payouts; **trusted contacts (U-07)** follow a shared trip.

## Platforms
The surfaces each feature is offered on (S-) are listed in FEATURES.md. Record here why the MVP ships on these platforms and which are deliberately deferred.

The MVP ships 8 apps:

| Audience | Apps |
|---|---|
| Customers | Website (`customer-web`) and an Android and iOS app (`customer-mobile-app`) |
| Drivers | Website (`driver-web`) for applying, documents, earnings and payouts, and an Android and iOS app (`driver-mobile-app`) for running trips |
| Staff | One website (`admin-web`) and one desktop and mobile app (`admin-app`) shared by operations and admin staff, with screens by role |
| Servers | The api (`api`) all apps use, and a background worker (`worker`) for timed and scheduled work |

After launch, an MCP server (`apps/mcp`, surface S-13) lets AI assistants act for a connected customer, and answer read-only questions for staff (FR-036 to FR-038). It is added to the project setup when it is built.

Separate operations apps were dropped on 2026-11-26 to reduce the number of apps to build and maintain. Chitty offers no public api for third parties in the MVP.

## Objectives and success criteria
- OBJ-001: {{objective and measurable success criterion}}

## MVP scope and functional requirements
Priority uses MoSCoW: Must, Should, Could, Won't. IDs are stable: never renumber or reuse them; mark removed requirements as deprecated.

The Features column lists the FEATURES.md features (FT-) each requirement covers; every approved feature appears in at least one requirement. A requirement that isn't a feature (e.g. "works on every supported platform") writes —. `node scripts/check-features.mjs` checks these links and copies them into FEATURES.md.

Values marked *configurable* are settings with a default (environment variable or admin setting), recorded in TECHNICAL.md, not open decisions.

| ID | Features | Requirement | Acceptance criterion | Priority |
|---|---|---|---|---|
| FR-001 | FT-001, FT-002, FT-003, FT-004, FT-005 | Customers create an account with a Nigerian phone number verified by SMS code, sign in and out, and edit their profile. | A new customer can sign up with phone and code, sign in on another device, change their name and sign out; a wrong or expired code is refused. | Must |
| FR-002 | FT-006 | Customers can close their account. | After closing, the customer can't sign in and their personal data is deleted or anonymised; ride and payment records are kept for the retention period. | Should |
| FR-003 | FT-007, FT-008 | Before booking, the customer sees a fare estimate calculated from the active fare rules. | For a pickup, drop-off and vehicle class, the estimate shown equals the price the fare rules give for that distance and duration. | Must |
| FR-004 | FT-009, FT-010, FT-033, FT-034 | Customers request a ride now; the system offers it to the nearest eligible online driver, one at a time, until one accepts or the search time runs out. | A request is accepted by the nearest eligible driver, or passed on when a driver declines or the offer times out (configurable); the customer is told when no driver is found. | Must |
| FR-005 | FT-011, FT-012 | Once a driver is assigned, the customer sees the driver's name, photo and rating and the car's plate, make, model and colour, and tracks the driver live. | The customer sees the assigned driver and vehicle details and the driver's position updating on a map until the trip ends. | Must |
| FR-006 | FT-014 | Customer and driver are notified of every ride status change, by push with SMS as fallback. | Each status change (accepted, arriving, started, completed, cancelled) reaches both parties; when push fails, an SMS is sent. | Must |
| FR-007 | FT-013, FT-022, FT-039 | Customers and drivers can cancel a ride before it starts, with fees as set in the fare rules and free cancellation of scheduled rides before a cut-off (configurable). | A cancelled ride stops for both parties, the correct fee (or none) is shown, and a driver's cancellation sends the ride back to matching. | Must |
| FR-008 | FT-015, FT-016, FT-037 | Customers pay for a completed ride by card or bank transfer through the payment provider, or in cash, which the driver records. | A card or transfer payment is marked paid only after a verified provider callback; a cash payment is marked paid by the driver; the customer gets a receipt either way. | Must |
| FR-009 | FT-017 | Customers rate the driver after a completed ride. | The rating is saved and the driver's average updates. | Must |
| FR-010 | FT-038 | Drivers rate the customer after a completed ride. | The rating is saved and the customer's average updates. | Should |
| FR-011 | FT-018, FT-041 | Customers and drivers see their past rides. | Each past ride shows route, other party, fare, payment status and rating. | Must |
| FR-012 | FT-019, FT-021, FT-023, FT-042, FT-043 | Customers schedule rides within a state at least the minimum lead time ahead (configurable); drivers see and accept open scheduled rides; the system confirms a driver or hands the ride to operations by the lead time (configurable). | A scheduled ride shows as "Scheduled", is accepted by a driver, and has a confirmed driver before pickup or reaches operations in time. | Must |
| FR-013 | FT-020 | Customers schedule private inter-state rides on the routes Chitty offers, with pickup within a time window (configurable). | A booking on an offered route, with a pickup time window, shows as "Scheduled" with the route price. | Must |
| FR-014 | FT-024 | Customer and driver are reminded before a scheduled pickup (configurable). | Both receive a reminder with time, place, driver and vehicle. | Should |
| FR-015 | FT-025, FT-026, FT-027, FT-028, FT-029, FT-030 | Drivers apply with a verified phone number, upload their documents, add their vehicle and payout bank account, sign in and follow their application status. | A new driver can submit an application with every required document and a vehicle, see each item's review status and save a bank account. | Must |
| FR-016 | FT-031, FT-032 | Approved drivers go online and offline; the positions of online drivers are recorded at an interval (configurable). | Only approved drivers can go online; an online driver's position is available for matching and tracking. | Must |
| FR-017 | FT-035, FT-036 | Drivers start the trip at pickup and complete it at drop-off; the final fare is shown to both parties. | A started ride shows "In progress" to both parties; a completed ride shows the final fare and requests payment. | Must |
| FR-018 | FT-040, FT-070, FT-071, FT-072, FT-073, FT-074 | Drivers' earnings are fares less Chitty's commission (configurable); payouts are created per period (configurable, default weekly), approved by finance staff and paid through the payment provider. | Each driver sees their earnings and payouts; a payout approved by finance is sent and marked paid when the provider confirms. | Must |
| FR-019 | FT-044, FT-062, FT-063, FT-064 | Staff sign in with email, password and a second factor; admins create and archive staff accounts with roles (operations, admin, finance, HR). | A staff member sees only what their role allows; an archived staff member can't sign in. | Must |
| FR-020 | FT-045, FT-046, FT-047, FT-048 | Operations agents review driver applications, approve or decline them with reasons, and suspend or reinstate drivers. | An approved driver can go online; a declined driver sees the reason; a suspended driver goes offline until reinstated. | Must |
| FR-021 | FT-050 | Operations agents suspend and reinstate customer accounts. | A suspended customer can't book until reinstated. | Should |
| FR-022 | FT-049 | Every staff action that changes a driver, customer, ride, payment, fare rule, payout or staff account is recorded in an audit log. | Each such change has an audit entry naming who, what, when and the reason. | Must |
| FR-023 | FT-067 | Admins search the audit log. | Searching by staff member, record or date returns the matching entries. | Should |
| FR-024 | FT-051, FT-052, FT-053, FT-054, FT-055 | Operations agents see all online drivers and active rides on a live map, find rides by search and filters, assign drivers to scheduled rides and cancel rides on behalf of either party. | An agent can find any ride, see it live, assign a driver to an unconfirmed scheduled ride and cancel a ride with a reason. | Must |
| FR-025 | FT-056, FT-057, FT-058, FT-059 | Customers and drivers raise support tickets; operations agents see them with the linked ride and reply. | A ticket raised in a customer or driver app appears in the operations queue, and the agent's reply and status reach the person who raised it. | Must |
| FR-026 | FT-060 | Operations agents assign tickets to an agent. | An assigned ticket appears in that agent's queue. | Should |
| FR-027 | FT-061 | Operations agents refund all or part of a card or transfer payment, up to a limit per agent (configurable). | The customer receives the refund and a notification; refunds above the limit are refused. | Should |
| FR-028 | FT-065, FT-068, FT-069 | Admins set fare rules by vehicle class and area, prices of inter-state routes, the areas served and the routes offered; bookings outside them are refused. | New bookings use the new prices while existing bookings keep theirs; a booking outside a served area or route is refused with a clear message. | Must |
| FR-029 | FT-066 | Admins see business reports: rides, revenue, cancellations and active drivers by day and state. | The figures for a chosen period match the underlying rides and payments. | Should |
| FR-030 | FT-075, FT-076 | Customers share an active ride with a trusted contact by link. | The contact follows the driver, vehicle and live location without an account until the trip ends; the link then stops working. | Should |
| FR-031 | FT-077, FT-078, FT-079, FT-081 | Customers and drivers raise an SOS alert during a ride; operations sees it at once with the live location and resolves it with notes. | An SOS raised in a customer or driver app appears for operations within seconds with live location, and is closed with a record of what was done. | Should |
| FR-032 | FT-080 | A trip starts only after the driver enters the customer's trip PIN. | A wrong PIN keeps the trip from starting. | Should |
| FR-033 | FT-082, FT-083 | Driver identity is checked automatically: NIN and licence against a government verification service, and a selfie against the profile photo before going online at an interval (configurable). | Mismatched documents are flagged before review; a failed selfie check keeps the driver offline. | Could |
| FR-034 | FT-084 | The system raises an alert when a ride leaves its expected route or stops for longer than a set time (configurable). | Operations receives an alert without anyone pressing SOS. | Could |
| FR-035 | FT-085, FT-086, FT-087, FT-088 | Customers send packages: create a delivery order, a driver accepts, delivers with proof, and the customer tracks it. | A delivery is completed with proof of delivery and tracked by the customer. Stretch goal, not in the MVP. | Won't |
| FR-036 | FT-089, FT-095, FT-096, FT-097 | Customers connect an AI assistant through the MCP server with the permissions they choose, and can revoke it; every assistant action is recorded, and requests outside the permissions, from a revoked connection or above a rate limit (configurable) are refused. | A connected assistant can do only what was granted, loses access at once when revoked, and each of its actions has an audit entry naming the connection. | Could |
| FR-037 | FT-090, FT-091, FT-092, FT-093, FT-094 | A connected assistant gets fare estimates, shows the customer's rides, and books, schedules or cancels rides only after the customer confirms in the Chitty app. | Without the customer's confirmation in the app nothing is booked or cancelled; a confirmed request behaves exactly as if made in the app, with any cancellation fee shown before confirming. | Could |
| FR-038 | FT-098, FT-099, FT-100, FT-101, FT-102 | Staff connect a read-only assistant that searches rides, summarises open tickets and answers report questions within their role; staff revoke their own connections and admins any staff connection. | The assistant returns only what the staff member's role may see and cannot change anything; a revoked connection loses access at once. | Could |

## Out of scope / stretch
- Dispatch and delivery (FR-035): a stretch goal after the ride service.
- Ride sharing between customers within a state, promotions, referrals, tips, saved places and booking for someone else: not drafted (see FEATURES.md, Gaps). Shared seats on inter-state departures and a wallet are proposed in FEATURES.md (J-17 to J-19).
- A public api for third parties.

## Business rules and glossary
Resources (R-) in FEATURES.md are the main glossary terms; define them here once, with their rules.

- BR-001: Rides can be booked only in served areas and on offered inter-state routes (R-18).
- BR-002: Only approved, non-suspended drivers can go online and receive ride offers; a suspended driver goes offline at once.
- BR-003: A ride's price is fixed by the fare rules in force when it is booked; later fare rule changes apply only to new bookings.
- BR-004: A card or transfer payment counts as paid only after a verified callback from the payment provider; repeated callbacks for the same payment change nothing.
- BR-005: Every staff change to a driver, customer, ride, payment, fare rule, payout or staff account is recorded in the audit log with a reason.
- BR-006: Inter-state trips are scheduled only, with pickup within a time window (configurable); they cannot be booked for now.
- BR-007: Assistants connected through the MCP server act only within the permissions the person granted; bookings and cancellations need the person's confirmation in the Chitty app, and assistants never pay, refund or approve payouts.
- Immediate ride: a ride requested for pickup now, within one state.
- Scheduled ride: a ride booked for a later date and time, at least the minimum lead time ahead.
- Inter-state ride: a scheduled ride between states on a route Chitty offers.
- Operations agent: a staff member with the operations role; admin, finance and HR are other staff roles.

## Non-functional requirements
- NFR-001: {{Performance target and measurement}}
- {{Accessibility, reliability, privacy, security, data retention targets}}

## Constraints and risks
{{Budget, time, dependencies, domain/legal requirements}}

## Open decisions
- Drivers' return trips on inter-state rides: whether and how Chitty reimburses them, and whether drivers may set their own seat price on shared departures (drafted as fixed by the route's fare rule). Owner: management. Needed before M3 is planned (J-03, J-06).
- Cash and commission: how Chitty collects its commission on cash trips when a driver's cash exceeds what Chitty owes them. Owner: management. Needed before M2 is planned (J-11).
- Staff roles: what HR does in the product (not employee records) and whether drivers count as staff. Owner: management. Needed before M1 is planned (J-10).
- Payment providers: Paystack and OPay are chosen; still open is which provider handles which method (card, transfer, wallet top-ups) and which sends driver payouts. Owner: @bankole2000. Needed before M2 is planned (J-02, J-11).
- Email provider for staff invitations (and later other email): not chosen. Until it is, invitations are delivered only in development (logged); other environments refuse to invite staff (TASK-006, `InvitationSender` in apps/api). Owner: @bankole2000. Needed before staff are invited in staging or production (J-10).
- Data protection under the Nigeria Data Protection Act 2023: retention periods and consent for personal data, location history and NIN (`review:privacy`). Owner: {{owner}}. Needed by {{date}}.

## Approval
{{Human approver, date, approved revision}}
