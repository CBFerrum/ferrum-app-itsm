# Changelog

## 0.4.2

Fixes from the first browser walk (interface only; the server is unchanged).

- **A refused save is seen.** "Save draft" with a field missing looked like a dead button: the messages were far down the page. On a refused save (the form's own check or the server's refusal) the cursor now goes to the first field in error and the page scrolls to it, a summary at the top lists every field as a link to it, and the step that holds one is marked "Needs attention". The same in the catalog request, the problem form and the dialogs of the major incident (declare, update, resolve). Pressing again with the same refusal takes the focus again.
- **A board with too few members.** The route panel said "2 of 0 members of Change Board approve". The seeded groups have no members, and the server refuses `submit_change` ("its approving group has N members and the type asks M of them to approve: add members to the group"; the same for a high-risk group with none). The panel now says the group has no members (or how many it has against how many are asked), who adds them (a link to the group's record, `/e/ItGroup/{id}`, where Configuration Manager and administrators may write), and "Save and submit" is not offered until then; the draft can still be saved.

## 0.4.1

The interface's own screens for what 0.3 and 0.4 added (generic record forms still show every field). Rebuilt with Ferrum-Frontend as it is on its main.

- **The desk** (`/`): your queue and your team's (incidents, requests and their tasks; a team is the assignment groups you are a member of), what waits for your decision (the platform's approvals asked of you, and, for a Change Manager, what one Change Manager decides), the targets breached or at risk across incidents, requests and tasks, a banner for each major incident with its next update, and a quick report whose title is matched to known errors as it is typed (`GET /suggest`). A person with no desk (the portal's IT Self Service, a Requester) sees only their own, and a part they may not read is left out.
- **Change** (`/change/new`, `/change/{id}`): the change as a flow (type, item and what it also affects, the risk questionnaire, plans, window). The risk, impact, route and who approves are asked of the server as the form is filled (`POST /ItChange/_calc`), so the score and the route shown are the ones saving keeps; collisions and a freeze are said in the server's words, and a freeze that will refuse the save is said before it. Submitted, it shows the platform's approval: who was asked, who answered (a stand-in named), N of M each step needs, reminders and escalations, and Approve and Reject for an approver. Then Done, the review, Close and It failed. A change is raised from an approved template, or the window and title are asked of the template's own action.
- **Change calendar** (`/calendar`): the month as a grid and as a list from `GET /calendar`, a collision and a freeze marked in words, the days of a freeze shaded, filtered by a service or item.
- **Problems** (`/problems`, `/problem/{id}`) and **major incidents** (`/major/{id}`): the list by open, known error and resolved with open incidents counted; a problem's root cause, workaround and what the portal will show, its incidents linked and unlinked, and resolved with it (`resolve_linked_incidents`); a major incident declared, updated and ended with who will be told and the message as it will read.
- **Catalog** (`/catalog`, `/catalog/{id}`, `/request/{id}`): the items as a requester sees them (also a portal person, who reads only the offerings, the published known errors and their own requests), an item's form from its questions (a question asked only when an earlier one has its answer), a request with its approval, who decides, its answers and its fulfilment tasks, each task done by pressing Done. The tasks are left out, and said to be, for one who may not read them.
- **Configuration items** (`/items`, `/ci/{id}`): the items, the classes with their attributes and the relation types with where a failure flows; an item with its attributes, relations and what relates to it, what a failure of it reaches as a tree by ring (to the depth the settings allow, worked out from the relations with the server's rules), and the incidents and changes on it.
- The shell's Create menu opens the change flow and the catalog.
- Not done: the major incident's message is shown as the package ships it, not as a workspace edited it (no route gives a screen a template); the impact tree is worked out in the page from the relations the person may read (a route that gave it would save the requests); the calendar's items filter is by name; new words are in English in the Arabic and Hindi catalogs.

## 0.4.0

The change board the audit found missing, problems and known errors, catalogue approvals and tasks, and classes and relation types for the configuration database. Needs a platform with approval before an action (`on: {action}`, BD-34).

- **Change types and the questionnaire.** `ChangeType` records (seeded Standard, Normal, Emergency; a workspace adds its own), each with a path (Pre-approved, Board, Emergency), the group record whose members approve and how many of them (N of M, 1 to 5), a group for high risk, and whether the change is reviewed afterwards. `RiskQuestion` records, each with choices that score, measure risk or impact; a change answers them and its risk and impact are worked out against two thresholds in the settings. A change keeps its plan, back-out plan, test plan, the items and services it also affects, and a `window_flags` text.
- **Approval is the platform's.** The package ships a policy on the action `approve_change` (`package.yaml`, `approvals`): the group's members, N of them, then, for a high-risk change, one member of the type's high-risk group; reminders after a day and escalation to the Change Managers after two (no calendar, so elapsed time). Which steps apply is the change's `route`, made when the draft is saved: Pre-approved (an approved `ChangeTemplate` on a Pre-approved type: no one decides, and `raise_change` on the template makes the change), Board, Emergency (its own smaller group, reviewed afterwards), or Manager. The members are taken when the change is raised. A Pre-approved type without an approved template goes to the board its type names.
- **What an existing workspace keeps.** The setting `change_approval` is empty until chosen, which reads as Single Manager: route Manager, no step applies, the action runs at once and the rule of 0.3 holds (a Change Manager other than who raised it; a standard change approved on submit). A new workspace starts on Change Board, and on `problem_closes_incidents` Offer, by the upkeep's first round; one upgraded from 0.2 or 0.3 (it has made its notifications, or it has changes) is left empty. Chosen so a release changes nothing for a workspace until an administrator says so, and so that, between the update and the upkeep, a new workspace is on the old rule, never on a board with no members. A workspace that set its own policy on the action keeps it (the platform lets it replace ours). The upkeep also adds, once, the change types, their two groups (with no members: add them before the first change is submitted, which says so otherwise), the risk questions, the class and relation-type seeds, and gives existing changes the type named like their class, items the class named like their type and relations the type named like their kind.
- **Calendar, freezes, collisions.** The report `itsm_change_calendar` and `GET /api/app/itsm/calendar?from_date=&to_date=` list scheduled changes by window with their items and services, the other scheduled changes on one of them in the same window and the freeze they fall in. The setting `freeze_action` (Refuse, the default and what 0.3 did, or Warn) says whether a change in a freeze is refused or flagged; an emergency may always fall in one. Approving over another approved change of the same item is still refused.
- **Review.** `review_change` after implementation: an outcome (Successful, Successful with Issues, Failed), notes, who and when; a failed one may open an incident linked to the change (`Incident.change`). A type that reviews afterwards cannot be closed without it.
- **Problems.** `known_error` and `workaround_visible` on a problem; a visible workaround is kept in step as a `KnownError` record that the service desk finds by search and the portal reads (IT Self Service, published ones only). `GET /api/app/itsm/suggest?q=` gives the five best matches. Closing a problem offers `resolve_linked_incidents` (a configured action) when the setting says Offer; Automatically does as 0.3 did.
- **Major incidents.** `declare_major`, `major_update` and `end_major` on an incident, with a communication plan, a manager and the next update due; each tells the subscribers of the affected service (a table on the configuration item) with the message templates `itsm.major_declared`, `itsm.major_update` and `itsm.major_resolved`, which a workspace edits under Messages.
- **Catalog.** An offering names an approver group and how many of its members approve; one that names none is decided by one Change Manager as before. It also lists fulfilment steps (group, hours, sequence) run in order or at once; a request approved (or made, when it needs none) gets a `RequestTask` per step, each on a target on `itsm_hours` that runs only while it is open, and is fulfilled when the last task is done. The offering's questions and the request's checked answers are what the app already had for a form.
- **Configuration database.** `CiClass` records with declared attributes (kept as the item's `attributes`, checked when it is saved), `RelationType` records that say where a failure flows (To the Item, To the Target, None), seeded with Depends On, Runs On, Part Of and Connects To. The impact route follows them to the depth `impact_depth` (10; 0.3 followed without a bound), never twice; a loop through them is refused. The package describes an outside inventory (`itsm_inventory`, generic, an hourly pull mapped onto items by name), offered turned off. Items come in through the platform's import.
- Not done: the interface's own screens for any of this (generic record forms show the new fields), tasks for a rejected or cancelled request, a pull mapped past name, serial number and location, and interface words for the new names.

## 0.3.0

The service desk closes the gaps an audit against ServiceNow, Jira Service Management, Freshservice, Zendesk and Freshdesk found.

- **Who and what.** An incident and a request have a caller (an address, or a name and contact for someone outside), an assignment group (records: members and a lead) and an assignee who is one of its members, and a category and subcategory (a tree). An incident names the business service. The priority matrix is records (`PriorityMatrix`), seeded with the old values; the app's own matrix is gone. The platform seeds a package's rows only when an administrator installs or updates the app, so the upkeep also adds, once per workspace, any matrix row or category the workspace lacks by its key (never overwriting a row, never bringing back one deleted afterwards): a workspace upgraded from 0.2.x has them within the upkeep's first round, with the calendar and the notifications. Until then, and whenever the matrix has no row for an incident's impact and urgency, the incident is saved as P3 with the four and 24 working hours of the shipped P3 row, and its `matrix_missing` check says so. `assigned_to` is the assignee, label changed, data untouched.
- **Notes.** `ItNote`: work notes (staff only) and comments the caller sees, on an incident, request, problem or change, corrected only by their author within `ItsmSettings.note_edit_minutes` (5), never deleted but by an administrator.
- **Targets.** Response and resolution targets on incidents and requests are the platform's service targets on the working calendar `itsm_hours` (made on first use, every hour of every day in UTC, which is the wall clock the app counted before), paused while Awaiting Caller; the breach is recorded once by the platform. The fixed 4, 8, 24 and 72 hours and `mark_breached` are gone: the hours are the matrix's, an offering's `response_hours` and `fulfil_hours`. The incident's `breached` check is replaced by `resolve_state` and `respond_state`. Incidents and requests open at upgrade keep the clock they had (sql/001_targets.sql): started when opened, running to the moment they were due; one already breached stays breached; no response target is started for them.
- **Self-service.** The portal role IT Self Service: report an incident, ask from the catalog, follow their own with the visible comments, comment, confirm a resolution or reopen it within `ItsmSettings.reopen_days` (5).
- **Catalogue forms.** An offering has questions (kind, required, choices, only-when) and a fulfilment group; a request carries `answers`, checked on the server. The generic `details` text is gone; what existing requests held is kept as an internal note (sql/002_request_details.sql).
- **Notifications.** Seven automations are made once per workspace (`ensure_defaults`), for the caller (received, resolved, done, a comment meant for them) and the assignee; the workspace edits them under Automations.
- **The home.** It marks an open incident whose response or resolution is breached (as the platform recorded it) or at risk (running, with three quarters of the time from opening to its due moment gone), in place of the removed `breached` check; Awaiting Caller counts as open. Arabic and Hindi names for the new words. Rebuilt with Ferrum-Frontend 5f178dd.
- **Reports.** Open incidents, target compliance, mean time to resolve, requests by catalogue item, change success.
- Not done: a change board of several approvers (item 6 of the audit), approval chains on requests, notifying a group's members, and the interface's own screens beyond the home.

## 0.2.15

Rebuilt with Ferrum-Frontend 45dad7f, the design system as tuned after the landed build was reviewed. The left menu's icons are chosen so that none repeats on more than three rows and each fits its row; their drawings load beside the first page instead of inside it, so the page starts lighter; a row that scrolls sideways fades at the edge where there is more, in every browser and with reduced motion; a printed record leaves out empty fields and keeps its totals with the table; menus and popovers sit inside the page's landmarks for assistive technology; a selection bar folds what does not fit into More on a narrow screen. Nothing else changes.

## 0.2.14

The interface follows the upgraded design system, rebuilt with Ferrum-Frontend 0a47157. Text uses the kit's type roles (nothing under 12px) and controls its height tokens; loading shows skeletons shaped like what is coming; empty, no matches, denied, offline and failed are told apart; status colours come from stored state ids. The interface check now runs the UI lint against an empty baseline. The home is rebuilt on the kit's work-home pattern: one page title, the main action as a button, queues with their counts, status at its natural width; its other own pages follow the same anatomy.

## 0.2.13

Rebuilt with Ferrum-Frontend c59c9a2: the names of its models, fields and manifest in Arabic and Hindi (162 each in ui/src/locales/names, which the platform serves to every page of the workspace, so a record's labels, the launcher and the bar read in the person's language). Translated by the model, to be reviewed by native speakers.

## 0.2.12

Rebuilt with Ferrum-Frontend 155518c: the sentences its pages built in code (counts, dates, what a toast or a screen reader says) go through t() too, as whole sentences where code chose a word, and carry built-in Arabic and Hindi (17 words each in ui/src/locales, translated by the model and to be reviewed by native speakers).

## 0.2.11

The assistant knows what its records mean in its pages (contributes.shell.assistant): their states, what counts and where figures come from, with questions it suggests asking there.

## 0.2.10

Rebuilt with Ferrum-Frontend 5b22e7b: its own words carry built-in Arabic and Hindi (14 words each, ui/src/locales, translated by the model and to be reviewed by native speakers), fetched for the page's language only. The labels it keeps in objects, its dialogs, its page titles and the words it keeps in variables go through t() as well, and the names and sizes it shows as they are keep their order right to left. Its pages are drawn once their words are in, and its entry carries only the parts of the kit it uses.

## 0.2.9

Rebuilt with Ferrum-Frontend 6537d65: an action's link, code or token shown once is kept in a dialog to copy, and on a page that reads right to left, English left untranslated keeps its own order. Its own words go through the kit's t() (14 places), so a workspace's own words for its language reach them, and Admin, Interface words lists them under it (dist/words.json).

## 0.2.8

Rebuilt with Ferrum-Frontend d5d98f2: a workspace's own words for its interface (Admin, Interface words) reach its pages as they load; an attachment field that holds a picture shows it; and a value another record already has is said so, where the form said the record had been changed by someone else.

## 0.2.7

Rebuilt with Ferrum-Frontend 63280d3: right to left, its pages mirror as they read (icons that point along the line, margins and alignment by the line's start and end, arrow keys along it), and the records pages' words are in Arabic and Hindi.

## 0.2.6

Rebuilt with Ferrum-Frontend a64416a: a new record's box whose default is 1 shows ticked, as it is saved.

## 0.2.5

Rebuilt with Ferrum-Frontend 922fcfb: an action's dialog says every refusal, the errors on what the action made (another record's rows) among them, and an action with parameters and a confirmation asks once.

## 0.2.4

What walking the desk found: a waiting request was listed as its number and status alone, and a change coming up without its date. The desk now names what each request asks for, who asked and when it is due, soonest first, and when each change starts.

## 0.2.3

Rebuilt with Ferrum-Frontend a76295e: its pages show times on a person's own clock when they chose one on their Account page, while today and due dates stay on the workspace's day.

## 0.2.2

Rebuilt with Ferrum-Frontend 24c3951: its pages show times on the workspace's clock, naming the zone when it is not the browser's, a draft numbered on submit is headed as a draft, and a record of an app that is not installed says so.

## 0.2.1

Rebuilt with Ferrum-Frontend bb60115: a record page reads an empty number or check as the server does, so an action whose condition needs one shows, and a workspace's settings open as their one record.

## 0.2.0

Discovery, and the connector with Assets (FERRUM-ECOSYSTEM-003).

- `POST /api/app/itsm/discovery` takes what scanners, cloud inventories and agents find, from a
  Discovery Agent (a new role for a tool's account, which otherwise only reads items) or a
  Configuration Manager. A report has up to 500 items. Each item is matched in this order:
  1. the one its source reported under its key before;
  2. else the one with its serial number;
  3. else the one with its name, unless another source reports that one.

  Otherwise it is new, and Live. The source's type, serial number, location, environment and
  details are kept. The name, owner, criticality, status, notes and relations people set are left
  alone. `runs_on` adds that relation when missing, never one that loops. The answer counts what
  was created and updated, and says why each refused item was refused.
- Items their source has not reported for 7 days are marked not seen since their last day, and
  their owner is told (`itsm.missing`). They are never retired by themselves, and are cleared when
  reported again. The desk lists them.
- The connector app `itsm-assets` (package `itsm_assets`, shipped with IT Services, offered where
  Assets is installed too): an item names the fixed asset it is. It is found by serial number when
  none is named. The item shows who holds the asset, where, and when its warranty ends, kept as
  Assets changes them. An asset is one item's.
- Rebuilt with Ferrum-Frontend 4a3681c.

## 0.1.3

Rebuilt with Ferrum-Frontend 00dea07: in a workflow state that names what may change, the other fields show read-only, and the fields the state needs show required.

## 0.1.2

Rebuilt with Ferrum-Frontend daeadda: after an action runs, a record page shows what the action said it did rather than only that it is done.

## 0.1.1

Rebuilt with Ferrum-Frontend 80ff179: a field's label names its control for assistive technology, and a record whose title is a link shows that record's title, not its id.

## 0.1.0

IT service management (FERRUM-ECOSYSTEM-002, FERRUM-ECOSYSTEM-003).

- Configuration items with their relations; dependencies that never loop.
- A service catalog: requests due their offering's hours, approved by a Change Manager other than
  who asked when the offering needs it; requesters see their own.
- Incidents by the impact and urgency matrix, their targets, major incidents and breaches.
- Problems with workarounds and root causes, resolving their incidents.
- Changes: standard, normal and emergency; impact; freezes; no two approved on one item at once;
  decided by someone other than who raised them.
- Its own interface at `/itsm/`: the desk.
