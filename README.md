# IT Services

IT service management for [Ferrum](https://github.com/CBFerrum/Ferrum):
- configuration items and what they depend on or run on, reported by discovery tools or kept by
  hand, and linked to the fixed assets they are where Assets is installed;
- a service catalog whose items ask their own questions, and requests approved by someone other than who asked;
- incidents with a caller, an assignment group and a category, prioritised by a matrix kept as records, with response and resolution targets on a working calendar;
- work notes and comments the caller sees, an employee self-service role, notifications and reports;
- problems, and changes with approval, conflicts, freezes and impact.

| | |
|---|---|
| 🛠️ App | `itsm`, native, in the server |
| 🧩 Packages | `itsm` (needs only the platform's core); `itsm_assets`, the connector app `itsm-assets` with Assets |
| 🖥️ Interface | its own, at `/itsm/`: the desk (your queue and your team's, what waits for your decision, targets in trouble, major incidents), the change flow and calendar, problems and known errors, the major incident's messages, the catalog and its requests, configuration items and the impact walk, and the records |
| 🔐 Roles | Service Desk, Change Manager, Configuration Manager, Requester, IT Self Service (for the portal), Discovery Agent (a tool's account) |
| 📜 Licence | AGPL-3.0-only |

## Highlights

- **Configuration items.** An item depends on, runs on or connects to others. A dependency never
  comes back round to the item itself, however far; connections may.
- **Discovery.** Network scanners, cloud inventories and agents report what they find to
  `POST /api/app/itsm/discovery` with an API key of a Discovery Agent:
  `{"source", "items": [{"key", "name", "type", "serial_number", "location", "environment", "details", "runs_on"}]}`,
  up to 500 at a time. An item is matched in this order:
  1. the one its source reported under its key before;
  2. else the one with its serial number;
  3. else the one with its name, unless another source reports that one.

  Otherwise it is new, and Live. The source's type, serial number, location, environment and
  details are kept. The name, owner, criticality, status, notes and relations people set are not
  touched. `runs_on` adds that relation when missing, never one that loops. Each item refused says
  why. An item its source has not reported for 7 days is marked not seen since its last day, and
  its owner is told. It is never retired by itself.
- **Assets.** With the connector app `itsm-assets` (where Assets is installed too), an item names
  the fixed asset it is. It is found by serial number when none is named, and shows who holds it,
  where, and when its warranty ends, kept as Assets changes them. An asset is one item's.
- **The catalog.** An item asks its own questions (a label, an answer kind of text, number, date,
  yes or no, or a choice, whether it is required, and an "only when" that names an earlier
  question and the answer it wants). A request carries the answers, checked on the server against
  its item and shown with their labels, and is assigned to the item's fulfilment group. Its
  response and fulfilment times are the item's hours, working time on the calendar `itsm_hours`.
  One that needs approval is fulfilled only once a Change Manager approves it, and not the one
  who asked. A rejection says why.
- **Incidents.** A caller (the person raising it when left empty, or the name and contact of
  someone outside), who reported it if another, an assignment group and an assignee from it, a
  category and subcategory, and the business service. Impact and urgency give the priority
  through the Priority Matrix records (P1 to P4, seeded with the usual values), and the priority
  its response and resolution times, as working time on `itsm_hours`: the platform's service
  targets, paused while the incident is Awaiting Caller, the breach recorded once. The calendar is
  made on first use as every hour of every day in UTC, which counts as the wall clock did; an
  administrator edits it under Admin, Calendars. A P1 on a critical item is major. A resolved
  incident says how, and its caller may confirm it or, for the days the settings give, reopen it.
- **Notes.** Work notes (staff only) and comments (the caller sees them) on an incident, request,
  problem or change, with author and time, in order. An author may correct one for the minutes
  the settings give; then it is final, and only an administrator deletes one.
- **Self-service.** The role IT Self Service, which an administrator gives to employees or people
  outside, reports incidents, asks from the catalog, follows their own with the comments meant for
  them, comments, and confirms or reopens a resolution. Records are theirs by the caller field.
- **Notifications.** Seven automations are made once for a workspace (callers told of an incident
  or request received, an incident resolved, a request done and a comment meant for them; an
  assignee told of an assignment); edit their wording and conditions under Automations.
- **Reports.** Open incidents (by priority, group, category, assignee or age), target compliance,
  mean time to resolve, requests by catalogue item, and change success by type.
- **Problems.** Incidents link to a problem. A known error has its workaround; marked visible, it is
  published to the desk's search and the portal's suggestions. A resolved problem says its root
  cause, and offers to resolve the incidents still open under it (or does, as the settings say).
  An incident may be declared major, with a communication plan told to the service's subscribers
  from message templates a workspace edits.
- **Changes.** A change has a type (records, each with its path), a risk and impact from a
  questionnaire kept as records, plans, back-out and test plans, and the services and items it
  affects. Its approval is the platform's, a policy the package ships on the action: N of the
  members of a group for a normal change, a smaller group for an emergency (reviewed afterwards),
  one more for high risk, nobody for a standard change from an approved template. The setting
  `change_approval` turns it on (new workspaces) or leaves the one-Change-Manager rule (upgraded
  ones). The change calendar (report and route) flags collisions on a service or item and freezes,
  which refuse or warn as the settings say. After implementation, a review with an outcome; a
  failed change can open an incident.
- **Catalog tasks.** An offering may name an approver group, and list steps for groups, run in
  order or at once; a request gets a task per step and is fulfilled when they are done.
- **Classes and relations.** Items have a class (with declared attributes) and relations a type
  (records that say where a failure flows); the impact route follows them to a depth the settings
  give. An outside inventory is described, pulled hourly, and shipped off.

## Develop

```bash
cargo test                       # needs FERRUM_ITSM_TEST_DATABASE_URL's database
cd ui && npm run build && npm run check
```

`tests/desk_it.rs` walks the desk's people and records: callers, groups, categories and the matrix,
notes read by permission, targets on a calendar a workspace edits, the portal role, catalogue
questions, notifications and reports. `tests/itsm_it.rs` walks an IT department:
- items and a loop refused;
- requests approved and fulfilled;
- incidents prioritised, resolved and breached;
- a problem resolving its incidents;
- changes through freezes, conflicts and their decisions;
- who may do what;
- discovery reports matched by name and key, a refused type, a loop and another source's item
  refused, and an item no longer reported marked and its owner told once.

`tests/change_it.rs` (`FERRUM_ITSM_CHANGE_TEST_DATABASE_URL`) walks the change board, a workspace
upgraded from 0.3, and `tests/service_it.rs` (`FERRUM_ITSM_SERVICE_TEST_DATABASE_URL`) problems,
major incidents, catalogue tasks and classes.

`tests/assets_it.rs` (`FERRUM_ITSM_ASSETS_TEST_DATABASE_URL`) runs the connector with ERP and
Assets: items finding their laptop by serial number or naming it, an asset refused to a second
item, and custodian and location kept as Assets moves it.

## Not yet

- Scanning networks or reading cloud accounts itself: tools do, and report to the intake.
- Helpdesk tickets raised as incidents, and mail replies becoming comments.
- Approval chains on requests beyond one group, and tasks closed for a rejected request.
- Telling an assignment group's members when an incident is assigned to the group.
- An employee as the caller by link (the caller is an address).
- A major incident's message shown as the workspace edited it: the screen shows the wording the
  package ships, because no route gives a screen a workspace's own template.

## Changelog

| Version | Highlights |
|---|---|
| **0.4.2** | Fixes from the first browser walk: a refused save moves to the first field in error with a linked summary and the step marked (change, catalog request, problem, major incident), and a board with too few members is said plainly with a link to its group instead of "2 of 0". |
| **0.4.1** | The interface's own screens for 0.3 and 0.4: the desk with your queue and your team's, the change as a flow with its approval, the change calendar, problems and known errors, the major incident's messages, the catalog and its requests, configuration items and the impact walk. |
| **0.4.0** | The change board (change types, a risk questionnaire, N-of-M approval as a package policy, templates, calendar, freezes, review); known errors published to search and the portal; major incident communication; catalogue approver groups and fulfilment tasks; item classes, relation types and a bounded impact route. |
| **0.3.0** | Callers, assignment groups and categories; the priority matrix as records; work notes and comments; targets on the platform's clock and a working calendar; employee self-service; catalogue items with their own questions; notifications; reports. |
| **0.2.15** | Rebuilt on the tuned design system: better menu icons, a lighter first page, a visible scroll cue, cleaner print. |
| **0.2.14** | The redesigned interface: the upgraded design system, shaped loading, told-apart states. |
| **0.2.13** | Its names in Arabic and Hindi. |
| **0.2.12** | Sentences built in code are words too, in Arabic and Hindi. |
| **0.2.11** | The assistant knows its records. |
| **0.2.10** | Rebuilt with Ferrum-Frontend 5b22e7b: its own words in Arabic and Hindi. |
| **0.2.9** | Rebuilt with Ferrum-Frontend 6537d65: once-shown answers kept; English isolated right to left; its own words translatable by a workspace. |
| **0.2.8** | Rebuilt with Ferrum-Frontend d5d98f2: a workspace's own interface words; pictures in attachment fields; a duplicate value said so. |
| **0.2.7** | Rebuilt with Ferrum-Frontend 63280d3: right to left, pages mirror; the records pages' words in Arabic and Hindi. |
| **0.2.6** | Rebuilt with Ferrum-Frontend a64416a: a box whose default is 1 shows ticked. |
| **0.2.5** | Rebuilt with Ferrum-Frontend 922fcfb: action dialogs say every refusal and ask once. |
| **0.2.4** | The desk names what each request asks for, who asked and when it is due, and when each change starts. |
| **0.2.3** | Rebuilt with Ferrum-Frontend a76295e: times on a person's own clock when they chose one. |
| **0.2.2** | Rebuilt with Ferrum-Frontend 24c3951: times on the workspace's clock; drafts headed as drafts. |
| **0.2.1** | Rebuilt with Ferrum-Frontend bb60115: empty numbers read as the server reads them; settings open as their record. |
| **0.2.0** | Discovery reports from scanners, cloud inventories and agents; items no longer reported marked; the connector app `itsm-assets` linking items to the fixed assets they are. |
| **0.1.3** | Rebuilt with Ferrum-Frontend 00dea07: a workflow state's kept fields are read-only, its needed ones required. |
| **0.1.2** | Rebuilt with Ferrum-Frontend daeadda: actions say what they did. |
| **0.1.1** | Rebuilt with Ferrum-Frontend 80ff179: labels name their fields, and linked titles show their titles. |
| **0.1.0** | Configuration items, a service catalog, incidents by priority with their targets, problems, and changes with approval, conflicts, freezes and impact (FERRUM-ECOSYSTEM-002, -003). |

## License

AGPL-3.0-only. See [LICENSE](LICENSE).
