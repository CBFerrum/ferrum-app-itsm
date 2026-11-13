<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/assets/banner-dark.svg">
  <img alt="Ferrum IT Services: incidents, requests, problems, changes and the configuration database" src="docs/assets/banner-light.svg" width="100%">
</picture>

<br>

[![Version](https://img.shields.io/badge/version-0.4.2-0369a1?style=flat-square)](CHANGELOG.md)
[![Licence: AGPL-3.0](https://img.shields.io/badge/licence-AGPL--3.0-blue?style=flat-square)](LICENSE)
[![Ferrum platform](https://img.shields.io/badge/runs%20on-Ferrum-2f5bea?style=flat-square)](https://github.com/CBFerrum/Ferrum)
![Rust](https://img.shields.io/badge/Rust-server-b7410e?style=flat-square&logo=rust)
![PostgreSQL](https://img.shields.io/badge/PostgreSQL-data-336791?style=flat-square&logo=postgresql&logoColor=white)
![React](https://img.shields.io/badge/React%20%2B%20TypeScript-interface-149eca?style=flat-square&logo=react&logoColor=white)
![Tests](https://img.shields.io/badge/tests-7%20Rust%20%2B%2014%20UI%20files-16a34a?style=flat-square)

### Run IT the way ITIL describes it: know what you run, fix what breaks, change it safely.

Ferrum IT Services is for IT teams that handle incidents in a shared mailbox, requests in a spreadsheet and changes in
meetings. It joins a configuration database, a service catalog, incident, problem and change management and a
self-service portal in one open-source app, on the same data as the rest of your business.

[💡 Why](#-why-it-services) · [📸 Screenshots](#-screenshots) · [✨ Features](#-features) · [📊 Compare](#-how-it-compares) · [📍 Roadmap](#-roadmap) · [🚀 Install](#-install) · [🔧 Develop](#-develop)

</div>

## 💡 Why IT Services

- 🗺️ **Know what depends on what.** Configuration items with classes, typed relations and an impact route that walks
  a failure to everything it reaches, never looping back to the item itself.
- 🎚️ **Priority by matrix, targets by clock.** Impact and urgency give the priority through records you edit, and the
  priority gives response and resolution times counted in working hours, paused while you wait for the caller.
- 🚦 **Changes with a real board.** N of M approvals, a smaller group for emergencies, one more look for high risk,
  nobody for a standard change from an approved template. Calendars, collisions and freezes are checked for you.
- 🛍️ **A catalog that asks the right questions.** Each item has its own form, approvers and fulfilment tasks.
- 🔌 **Discovery in, nothing overwritten.** Scanners and cloud inventories report to an API; what people keep is left alone.
- 🏠 **Self-hosted and open source.** AGPL-3.0, on your own server and database, with no per-agent fee.
- 🌍 **Arabic and Hindi** words and right-to-left layout, beside English.

## 🎯 At a glance

| | |
|---|---|
| **Version** | 0.4.2 (platform `>=0.3.2, <0.4`) |
| **Kind** | Own interface at `/itsm/` (the desk, change flow and calendar, problems, major incidents, catalog, configuration items), plus generic records screens; a portal for employees |
| **What it adds** | 29 entities (19 main, 10 child tables), 6 roles, 6 reports, 3 message templates, 7 notification automations, 1 approval policy per change and request, 1 menu module |
| **Depends on** | Ferrum core only |
| **Works with** | Assets (connector `itsm-assets`), the platform's calendars, targets and approvals, discovery tools |
| **Languages** | English, Arabic (right to left), Hindi; a workspace can change any interface word |
| **Tests** | 7 Rust tests (five walk a whole IT department) and 14 interface test files |
| **Licence** | AGPL-3.0-only |

## 📸 Screenshots

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/assets/screens/itsm-home-dark.webp">
  <img alt="IT Services desk" src="docs/assets/screens/itsm-home-light.webp" width="100%">
</picture>

**🛠️ The desk**<br><sub>See your queue and your team's, what waits for your decision, targets in trouble and any major incident.</sub>

<table>
<tr>
<td width="50%" valign="top">
<picture><source media="(prefers-color-scheme: dark)" srcset="docs/assets/screens/itsm-incident-list-dark.webp"><img alt="Incidents" src="docs/assets/screens/itsm-incident-list-light.webp"></picture>
<p><b>🚨 Incidents</b><br><sub>Browse, search and filter incidents by priority, group and state.</sub></p>
</td>
<td width="50%" valign="top">
<picture><source media="(prefers-color-scheme: dark)" srcset="docs/assets/screens/itsm-incident-detail-dark.webp"><img alt="Incident record" src="docs/assets/screens/itsm-incident-detail-light.webp"></picture>
<p><b>🔎 Incident record</b><br><sub>Work one incident: caller, priority, targets, notes and the item it hit.</sub></p>
</td>
</tr>
<tr>
<td width="50%" valign="top">
<picture><source media="(prefers-color-scheme: dark)" srcset="docs/assets/screens/itsm-request-list-dark.webp"><img alt="Requests" src="docs/assets/screens/itsm-request-list-light.webp"></picture>
<p><b>📨 Requests</b><br><sub>See requests from the service catalog and who each waits for.</sub></p>
</td>
<td width="50%" valign="top">
<picture><source media="(prefers-color-scheme: dark)" srcset="docs/assets/screens/itsm-service-offering-list-dark.webp"><img alt="Service catalog" src="docs/assets/screens/itsm-service-offering-list-light.webp"></picture>
<p><b>🛍️ Service catalog</b><br><sub>Keep the things people can ask IT for, with their approvals and fulfilment times.</sub></p>
</td>
</tr>
<tr>
<td width="50%" valign="top">
<picture><source media="(prefers-color-scheme: dark)" srcset="docs/assets/screens/itsm-change-list-dark.webp"><img alt="Changes" src="docs/assets/screens/itsm-change-list-light.webp"></picture>
<p><b>🚦 Changes</b><br><sub>See planned changes and what is waiting for approval.</sub></p>
</td>
<td width="50%" valign="top">
<picture><source media="(prefers-color-scheme: dark)" srcset="docs/assets/screens/itsm-configuration-item-list-dark.webp"><img alt="Configuration items" src="docs/assets/screens/itsm-configuration-item-list-light.webp"></picture>
<p><b>🗺️ Configuration items</b><br><sub>Browse what IT runs, its class, environment and criticality.</sub></p>
</td>
</tr>
</table>

<table>
<tr>
<td align="center" valign="top">
<picture><source media="(prefers-color-scheme: dark)" srcset="docs/assets/screens/itsm-home-phone-dark.webp"><img alt="Desk on a phone" src="docs/assets/screens/itsm-home-phone-light.webp" width="260"></picture>
<p><b>📱 Desk on a phone</b></p>
</td>
<td align="center" valign="top">
<picture><source media="(prefers-color-scheme: dark)" srcset="docs/assets/screens/itsm-incident-list-phone-dark.webp"><img alt="Incidents on a phone" src="docs/assets/screens/itsm-incident-list-phone-light.webp" width="260"></picture>
<p><b>📱 Incidents on a phone</b></p>
</td>
</tr>
</table>

## ✨ Features

### 🗺️ Configuration items

- **Configuration Item.** Something IT runs: name, class, type (Business Service, Application, Server, Database,
  Network Device, Laptop, Other), status (Planned, Live, Retired), environment (Production, Staging, Test,
  Development), criticality (Low to Critical), owner, serial number, location and notes.
- **Classes.** A class declares the attributes its items carry (text, number, date, yes or no, or a choice, each
  required or not). Values are checked when the item is saved. Seven classes ship.
- **Relations.** An item depends on, runs on or connects to others, by a relation type. Four types ship (Depends On,
  Runs On, Part Of, Connects To), each with its inverse and where a failure flows: to the item, to the target or nowhere.
- **No loops.** A relation that carries a failure never comes round to the item itself, however far.
- **Impact route.** What a failure of an item reaches, as a tree by ring, to a depth the settings give.
- **Subscribers.** Who is told when a major incident hits the service.
- **Where it stands.** The item's page shows its attributes, relations both ways, the impact tree, and the incidents and changes on it.

### 🔌 Discovery

- **Intake.** Scanners, cloud inventories and agents report to `POST /api/app/itsm/discovery` with the API key of a
  Discovery Agent: `{"source", "items": [{"key", "name", "type", "serial_number", "location", "environment", "details", "runs_on"}]}`,
  up to 500 at a time.
- **Matching.** By the key the source used before, else the serial number, else the name unless another source
  reports that one. Otherwise the item is new and Live.
- **What it never touches.** The name, owner, criticality, status, notes and relations people set.
- **`runs_on`.** Adds that relation when missing, never one that loops. Each item refused says why.
- **Not seen.** An item its source has not reported for 7 days is marked not seen since its last day, and its owner is
  told (a notification kind). It is never retired by itself.
- **Outside inventory.** A connection described as `itsm_inventory` pulls an outside inventory hourly.

### 🧾 Assets (with the Assets app)

- **Connector app `itsm-assets`.** An item names the fixed asset it is. It is found by serial number when none is named,
  and shows who holds it, where, and when its warranty ends, kept as Assets changes them. An asset is one item's.

### 🚨 Incidents

- **Incident** (`INC-2026-00001`). Title, description, caller (the person raising it when left empty, or the name and
  contact of someone outside), who reported it if another, category and subcategory, business service and the item affected.
- **Priority by matrix.** Impact and urgency (High, Medium, Low) give P1 to P4 through the Priority Matrix records, seeded
  with the usual values: P1 15 minutes to respond and 4 hours to resolve, P2 1 and 8 hours, P3 4 and 24, P4 8 and 72.
- **Targets on the platform's clock.** Response and resolution are service targets on the working calendar `itsm_hours`,
  paused while the incident is Awaiting Caller, with each breach recorded once. The calendar is made on first use as every
  hour of every day in UTC; an administrator edits it under Admin, Calendars.
- **States.** New, In Progress, On Hold, Awaiting Caller, Resolved, Closed.
- **Resolve.** A resolved incident says how. The caller may confirm it or, for the days the settings give, reopen it.
- **Assignment.** An assignment group and an assignee who must be one of its members.
- **Links.** To a problem and to a change.
- **Major incidents.** A P1 on a critical item is major. A manager declares one with a communication plan and sends
  updates; the screen shows who will be told (the subscribers of the incident's business service) and how the message reads, before it goes.
  A next-update time and a manager are kept; ending it sends a resolved message. The wording is a message template a workspace edits.
- **Suggestions.** Known errors that fit what someone is typing appear beside the title, the catalog's search and a request's free text.

### 🛍️ Service catalog and requests

- **Service Offering.** What people may ask IT for: name, description, business service, response and fulfilment
  hours, a fulfilment group and whether it needs approval.
- **Questions of its own.** Each offering asks questions (label, answer kind of text, number, date, yes or no or a choice,
  required or not) and can show a question only when an earlier one has a given answer.
- **Catalog Request** (`REQ-2026-00001`). Carries the answers, checked on the server against its item and shown with
  their labels. States: Submitted, Approved, Awaiting Caller, Rejected, Fulfilled, Cancelled.
- **Approval.** An offering can name an approver group and how many of its members must approve (N of M), by the
  platform's approvals with reminders. Never the person who asked. A rejection says why.
- **Fulfilment tasks.** An offering lists steps for groups, run in order or at once. A request gets a task per step once
  approved and is fulfilled when they are done.
- **Targets.** Response and fulfilment times are the item's hours, working time on `itsm_hours`. Tasks have targets too.

### 🧯 Problems and known errors

- **Problem** (`PRB-2026-00001`). The cause behind incidents, which link to it. States: Open, Known Error, Resolved, Closed.
- **Known error.** A workaround is kept on the problem. Marked visible, it is published to the desk's search and to
  the portal's suggestions.
- **Resolving.** A resolved problem says its root cause and offers to resolve the incidents still open under it, or
  does so automatically, as the settings say.

### 🚦 Changes

- **Change** (`CHG-2026-00001`). Title, type, item and affected services and items, kind (Standard, Normal,
  Emergency), planned start and end, change plan, back-out plan and test plan.
- **Change types.** Each type has a path: Pre-approved, Board or Emergency. Three ship: Standard, Normal and Emergency.
- **Risk questionnaire.** Five seeded questions, each answer scoring. Risk and impact add up against thresholds in the
  settings. The score is shown while the form is filled, so what is shown is what saving keeps.
- **Approval.** By the platform's approval policy: N of the members of a board for a normal change (up to five), a
  smaller group for an emergency (reviewed afterwards), one more look for a high-risk change, and nobody for a standard
  change from an approved template. Reminders after 24 hours and escalation. A setting turns it on (new workspaces) or keeps
  the older rule of one Change Manager other than who raised it (upgraded ones).
- **Templates.** A change done the same way each time, with its plans. Once a Change Manager approves a template, a change
  raised from it on a Pre-approved type needs no further approval. A template's approval can be withdrawn.
- **Change calendar.** Scheduled changes by window as a month or a list, flagging collisions with another change on one of the same
  items or services. Filter by a service or item.
- **Freezes.** A time no normal change is scheduled in; emergencies only. The settings refuse or warn.
- **Statuses.** Draft, Submitted, Approved, Implemented, Closed, Rejected, Failed.
- **Review.** After implementation, a review with an outcome (Successful, Successful with Issues, Failed). A failed change can open an incident.

### 📝 Notes and conversations

- **Work notes** (staff only) and **comments** (the caller sees them) on an incident, request, problem or change, with author
  and time, in order.
- **Correct, never rewrite.** An author may correct a note for the minutes the settings give; then it is final and only an
  administrator deletes it.

### 🙋 Self-service

- **IT Self Service role.** For employees or people outside, given by an administrator, for the portal. They report
  incidents, ask from the catalog, follow their own records with the comments meant for them, comment, and confirm or reopen a resolution.
  Records are theirs by the caller field.
- **Requester role.** Asks for things in the catalog and follows them.
- **What a portal person sees.** Offerings, categories, published known errors and their own records. What a role may not
  read is left out and said, not failed.

### 👥 Groups, categories and settings

- **Group.** A team that works incidents and requests, with its lead and members. Two boards ship: Change Board and Emergency Change Board.
- **Category.** Hardware, Software, Network and Access with subcategories such as Laptop, Printer, VPN and Account and Password.
- **Settings.** Minutes a note can be corrected, days a caller may reopen, whether changes use the board or a single manager,
  whether a freeze refuses or warns, the scores at which risk is medium and high, the depth of the impact route, and whether a resolved problem resolves its incidents or offers to.

### 🔔 Notifications

Seven automations are made once for a workspace: callers told of an incident or request received, an incident resolved, a
request done and a comment meant for them, and an assignee told of an assignment. Edit their wording and conditions under
Automations. Item owners are told when discovery no longer finds their item.

### 📊 Reports

| Report | What it shows |
|---|---|
| **Open Incidents** | By priority, assignment group, category, assignee or age |
| **Target Compliance** | Response and resolution met or missed, by priority, group or category |
| **Mean Time to Resolve** | Elapsed hours from opened to resolved |
| **Requests by Catalogue Item** | What people ask for most |
| **Change Success** | Changes by type and outcome |
| **Change Calendar** | Scheduled changes with their collisions and freezes |

### 🔐 Roles

- **Service Desk.** Works incidents, requests and tasks, and records problems and known errors.
- **Change Manager.** Approves or rejects changes and requests, and sets freezes, types, templates and questions.
- **Configuration Manager.** Keeps the items, classes, relation types, catalog, groups, categories, matrix and settings.
- **Requester, IT Self Service.** See above.
- **Discovery Agent.** A tool's account for the intake.

## 🧭 How it works

```mermaid
flowchart LR
    CALLER["Caller"] --> INC["Incident"]
    INC -->|"impact and urgency"| PRI["Priority P1 to P4"]
    PRI --> TGT["Response and resolution targets"]
    INC -->|"P1 on a critical item"| MAJ["Major incident"]
    INC --> PRB["Problem"]
    PRB -->|"workaround"| KE["Known error, in search"]
    PRB -->|"root cause fixed"| CHG["Change"]
    CHG -->|"board, freeze, calendar"| IMP["Implemented and reviewed"]
```

An incident gets its priority from the matrix and its clocks from the priority. Incidents that share a cause link to a
problem, whose workaround is offered to the next person who types something similar. A fix goes through a change.

```mermaid
flowchart TD
    DISC["Discovery tools and cloud inventories"] -->|"POST discovery"| CI["Configuration items"]
    ASSET["Assets app"] -->|"serial number"| CI
    CI -->|"impact route"| INC["Incidents and changes"]
    CAT["Service catalog"] --> REQ["Requests, approvals, tasks"]
    REQ --> CI
    PORTAL["Employee portal"] --> INC
    PORTAL --> REQ
```

The configuration items are what everything else points at: an incident names the item it hit, a change lists the items
it affects, and the impact route shows who is hit.

## 🧱 Data model

| Entity | What it is | Key fields or states |
|---|---|---|
| **Incident** | Something broken | Number, caller, category, item, impact, urgency, priority, major, status, targets, group, assignee |
| **Catalog Request** | A request from the catalog | Offering, caller, answers, status, group, targets |
| **Request Task** | One step of fulfilling a request | Step, group, status (Waiting, Open, Done), target |
| **Problem** | The cause behind incidents | Status, workaround, root cause |
| **Known Error** | A workaround as the desk finds it | Problem, service, symptoms, workaround |
| **Change** | A change to something IT runs | Type, kind, risk, impact, window, plans, status, route, review |
| **Change Type, Change Template, Risk Question, Change Freeze** | How changes are classified and checked | Path, quorum, scores, window |
| **Configuration Item** | Something IT runs | Class, type, status, environment, criticality, relations, discovery source |
| **CI Class, Relation Type** | What items are and how they relate | Attributes, inverse, where a failure flows |
| **Service Offering** | A catalog item | Questions, approval, fulfilment steps |
| **IT Note** | A work note or comment | Visibility, author, posted at |
| **Group, Category, Priority Matrix** | Who and what | Members, parent, priority and times |
| **IT Services Settings** | Workspace settings | See Settings above |
| Child tables | Attributes, relations, subscribers, questions, answers, steps, choices, change answers, change items, group members | |

| Role | What it can do |
|---|---|
| **Service Desk** | Incidents, requests, tasks, problems, known errors, notes |
| **Change Manager** | Changes, types, templates, freezes, request approvals |
| **Configuration Manager** | Items, classes, relations, catalog, groups, categories, matrix and settings |
| **Requester** | Asks from the catalog and follows their requests |
| **IT Self Service** | Portal: own incidents and requests, comments, confirm or reopen |
| **Discovery Agent** | Reports to the discovery intake |

Reports: six, listed under Features. Print formats: none ship. Messages: three major-incident messages.

## 🔌 Integrations

| With | How | Needs |
|---|---|---|
| **Discovery tools** | Network scanners, cloud inventories and agents post what they find to the intake | A Discovery Agent API key |
| **Outside inventory** | A described connection pulls items hourly | The inventory's API and a key |
| **Assets** (`itsm-assets`) | An item is a fixed asset: custodian, location and warranty kept as Assets changes them | Assets and ERP installed |
| **Platform calendars and service targets** | `itsm_hours` and the targets of incidents, requests and tasks | Platform |
| **Platform approvals** | Change board and request approver groups, reminders and escalation | Platform |
| **Notifications and automations** | Seven shipped automations, editable | Platform; mail for email |
| **Portal** | IT Self Service people use the catalog and follow requests | Portal switched on |
| **MCP** | AI agents work with the records under each person's permissions | Platform's MCP server |

## 📊 How it compares

Compared with ServiceNow ITSM, Jira Service Management, Freshservice, ManageEngine ServiceDesk Plus and GLPI (open source).

| Capability | Ferrum IT Services | ServiceNow | Jira Service Mgmt | Freshservice | ServiceDesk Plus | GLPI |
|---|:-:|:-:|:-:|:-:|:-:|:-:|
| Incidents with caller, group, category and service | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Editable impact and urgency priority matrix | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Targets in working hours, paused while waiting | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Work notes and customer-visible comments | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Email and chat intake | ❌ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Employee self-service portal | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Service catalog with its own forms | ✅ | ✅ | ✅ | ✅ | ✅ | 🟡 |
| Catalog approvals by group, with tasks | ✅ | ✅ | ✅ | ✅ | ✅ | 🟡 |
| Known errors suggested while typing | ✅ | ✅ | ✅ | ✅ | ✅ | 🟡 |
| Problem management with workaround and root cause | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Major incident with communication plan | 🟡¹ | ✅ | ✅ | ✅ | 🟡 | ❌ |
| Change approval by board, N of M | ✅ | ✅ | ✅ | ✅ | ✅ | 🟡 |
| Emergency path and review afterwards | ✅ | ✅ | ✅ | ✅ | ✅ | 🟡 |
| Risk questionnaire sets change risk | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ |
| Change calendar with collisions and freezes | ✅ | ✅ | 🟡 | ✅ | 🟡 | 🟡 |
| Change templates and standard changes | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Release management | ❌ | ✅ | 🟡 | ✅ | ✅ | ❌ |
| Configuration database with classes and relations | ✅ | ✅ | ✅ | ✅ | ✅ | 🟡 |
| Impact route through relations | ✅ | ✅ | ✅ | ✅ | ✅ | 🟡 |
| Discovery of devices | 🟡² | ✅ | 🟡 | ✅ | ✅ | ✅ |
| On-call schedules and paging | ❌ | ✅ | ✅ | 🟡 | ❌ | ❌ |
| Monitoring alerts open incidents | ❌ | ✅ | ✅ | ✅ | 🟡 | ❌ |
| Customer satisfaction survey | ❌ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Virtual agent and AI summaries | ❌ | ✅ | ✅ | ✅ | ✅ | ❌ |
| Chat-ops in Slack or Teams | ❌ | ✅ | ✅ | ✅ | ✅ | ❌ |
| Native mobile app | ❌ | ✅ | ✅ | ✅ | ✅ | ❌ |
| Reports and dashboards | 🟡³ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Custom fields and workflows without code | 🟡⁴ | ✅ | ✅ | ✅ | ✅ | 🟡 |
| Same data set as assets, HR, accounting and ERP | 🟡⁵ | ✅ | 🟡 | 🟡 | 🟡 | 🟡 |
| Self-hosted | ✅ | ❌ | ✅⁶ | ❌ | ✅ | ✅ |
| Open source | ✅ | ❌ | ❌ | ❌ | ❌ | ✅ |
| No per-agent subscription | ✅ | ❌ | ❌ | ❌ | ❌ | ✅ |

¹ A manager declares it, sends updates to the service's subscribers and ends it. There is no bridge and no post-incident review yet.
² Tools report to an intake; the app does not scan networks or read cloud accounts itself.
³ Six reports; no shipped dashboard.
⁴ Fields, rules and automations are added in Studio; the lifecycle states are fixed.
⁵ Items can be linked to fixed assets with the Assets app; nothing else is linked yet.
⁶ Jira Service Management Data Center is self-managed.

### 🏆 Where IT Services stands out

- 🔌 **Discovery that never overwrites** what people keep, with matching by key, serial number and name.
- 🚦 **A change board built from the platform's approvals**: N of M, emergency path, high-risk second look, reminders and escalation.
- 🗺️ **Loop-free impact route** through typed relations that say where a failure flows.
- 🛍️ **Catalog items that ask their own questions** with conditional questions, approvals and tasks.
- ⏱️ **Targets on working calendars** you edit, with breach recorded once.
- 🏠 **Self-hosted, open source and no per-agent fee**, with MCP access for AI agents under each person's permissions.

### 🚧 Where the leaders are still ahead

- 📬 **Email and chat intake** into incidents and requests.
- 🛰️ **Discovery, monitoring and event management** that find things and open incidents by themselves.
- 📟 **On-call schedules and paging.**
- 🤖 **AI**: virtual agents, summaries and suggested resolutions.
- 📱 **Native apps, chat-ops and satisfaction surveys.**
- 📦 **Release management** and years of marketplace integrations.

> [!NOTE]
> Leader columns are from public product information as of November 2026 and may change.

## 📍 Roadmap

### ✅ Recently shipped

- [x] **0.4.2.** Fixes from the first browser walk: a refused save moves to the first field in error with a linked summary; a board with too few members is said plainly.
- [x] **0.4.1.** The interface's own screens: the desk, the change flow and calendar, problems, the major incident's messages, the catalog and configuration items with the impact walk.
- [x] **0.4.0.** The change board, known errors published to search and the portal, major incident communication, catalog approver groups and fulfilment tasks, item classes, relation types and the impact route.
- [x] **0.3.0.** Callers, groups and categories, the priority matrix, work notes and comments, targets on the platform's clock, employee self-service, catalog questions, notifications and reports.
- [x] **0.2.0.** Discovery intake and the `itsm-assets` connector.
- [x] **0.2.15 to 0.2.13.** The tuned design system, the redesigned interface, names in Arabic and Hindi.

### 🔜 Next up

- [ ] **Email and chat intake** into incidents and requests, with mail replies becoming comments.
- [ ] **Helpdesk tickets raised as incidents**, so one customer problem is not typed twice.
- [ ] **Satisfaction survey** after an incident or request.
- [ ] **Alert and monitoring intake** that opens or updates one incident per alert.
- [ ] **On-call schedules** with escalation and paging.
- [ ] **Tell an assignment group's members** when an incident is assigned to the group.

### 🔭 Later

**Incidents**
- [ ] Major incident bridge, stakeholder roles and a post-incident review.
- [ ] A major incident's message shown as the workspace edited it, not as the package ships it.
- [ ] Child and duplicate incidents, with a parent major incident closing its children.
- [ ] Auto-close of resolved incidents after some days.

**Requests**
- [ ] Approval chains on requests beyond one group.
- [ ] Tasks closed when a request is rejected.
- [ ] The employee as the caller by link rather than by address.

**Changes**
- [ ] Release management: bundle changes and deployment windows.
- [ ] Conflicts across dependent items and services, not only the same one.

**Configuration**
- [ ] Scanning networks or reading cloud accounts directly.

**Platform**
- [ ] AI virtual agent and incident summaries.
- [ ] Chat-ops: raise and approve from Slack or Teams.
- [ ] Public status page and subscriber notices.
- [ ] Native mobile app.
- [ ] Dashboards.

## 🚀 Install

IT Services is a native Ferrum app, compiled into the Ferrum server. Workspace administrators install it from **Apps**
(`/store/`). It needs platform `>=0.3.2, <0.4` and nothing else. The connector `itsm-assets` is offered where Assets is
installed too.

1. Install IT Services.
2. Give people the roles: Service Desk, Change Manager and Configuration Manager. Give IT Self Service to employees for the portal.
3. Create groups (with leads and members) and check the categories and the Priority Matrix that ship. Edit the working calendar `itsm_hours` under Admin, Calendars.
4. Add members to the Change Board and Emergency Change Board groups, and choose the change approval setting.
5. Add configuration items, or create a Discovery Agent account, give it an API key and point your discovery tool at `POST /api/app/itsm/discovery`.
6. Add service offerings with their questions, approvers and steps.
7. Edit the seven notification automations to your wording.

## 🔧 Develop

```bash
# server (needs a PostgreSQL role that is not a superuser, so row-level security applies)
export FERRUM_ITSM_TEST_DATABASE_URL=postgres://ferrum:ferrum@localhost:5432/ferrum_itsm_test
export FERRUM_ITSM_ASSETS_TEST_DATABASE_URL=postgres://ferrum:ferrum@localhost:5432/ferrum_itsm_assets_test
export FERRUM_ITSM_CHANGE_TEST_DATABASE_URL=postgres://ferrum:ferrum@localhost:5432/ferrum_itsm_change_test
export FERRUM_ITSM_SERVICE_TEST_DATABASE_URL=postgres://ferrum:ferrum@localhost:5432/ferrum_itsm_service_test
cargo fmt -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

The interface is built with Ferrum-Frontend, checked out next to this repository as `../ferrum/web`:

```bash
cd ui
npm run build       # writes ui/dist, which is committed and embedded by build.rs
npm run check
npm run typecheck
npm test
```

What the integration tests walk:

- `tests/itsm_it.rs`: an IT department: items and a loop refused, requests approved and fulfilled, incidents prioritised,
  resolved and breached, a problem resolving its incidents, changes through freezes and conflicts, who may do what, and discovery.
- `tests/desk_it.rs`: callers, groups, categories and the matrix, notes read by permission, targets on a calendar a
  workspace edits, the portal role, catalog questions, notifications and reports.
- `tests/change_it.rs`: the change board and a workspace upgraded from 0.3.
- `tests/service_it.rs`: problems, major incidents, catalog tasks and classes.
- `tests/assets_it.rs`: the connector with ERP and Assets.

## 📂 Project layout

```text
app.yaml                      the manifest (roles, shell entries, notification, assistant notes)
connectors/assets/            the connector app that links items to fixed assets
packages/itsm/                entities (itsm.yaml, desk.yaml), roles, approvals, seeds, messages, connection
packages/itsm_assets/         the link from an item to an asset
src/
  desk.rs                     catalog requests, incidents, problems, major incidents
  changes.rs                  routes, risk score, calendar, freezes, review
  cmdb.rs                     classes, relations and the impact route
  discovery.rs, assets.rs     the intake and the Assets link
  notes.rs, reports.rs, defaults.rs, automations.json
ui/src/                       the desk, change, calendar, problems, major, catalog, items; locales ar and hi
tests/                        integration tests against a real PostgreSQL
docs/PROVENANCE.md            where the design comes from
```

## 📜 Changelog

Every release is described in [CHANGELOG.md](CHANGELOG.md).

## 🤝 Contributing

Issues and pull requests are welcome at [github.com/CBFerrum/ferrum-app-itsm/issues](https://github.com/CBFerrum/ferrum-app-itsm/issues).
Run the checks in [Develop](#-develop) first.

## 📄 License

AGPL-3.0-only. See [LICENSE](LICENSE).

<div align="center">

<sub>Part of <a href="https://github.com/CBFerrum/Ferrum">Ferrum</a>, the open business platform · Made with 🦀 Rust and 🐘 PostgreSQL</sub>

</div>
