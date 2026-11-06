# IT Services

IT service management for [Ferrum](https://github.com/CBFerrum/Ferrum):
- configuration items and what they depend on or run on, reported by discovery tools or kept by
  hand, and linked to the fixed assets they are where Assets is installed;
- a service catalog, and requests approved by someone other than who asked;
- incidents prioritised by impact and urgency, with their targets and breaches;
- problems, and changes with approval, conflicts, freezes and impact.

| | |
|---|---|
| 🛠️ App | `itsm`, native, in the server |
| 🧩 Packages | `itsm` (needs only the platform's core); `itsm_assets`, the connector app `itsm-assets` with Assets |
| 🖥️ Interface | its own, at `/itsm/`: open incidents by priority, requests waiting and changes coming up, and the records |
| 🔐 Roles | Service Desk, Change Manager, Configuration Manager, Requester, Discovery Agent (a tool's account) |
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
- **The catalog.** A request is due its offering's hours after it is asked. One that needs
  approval is fulfilled only once a Change Manager approves it, and not the one who asked. A
  rejection says why. Requesters see only their own requests.
- **Incidents.** Impact and urgency give the priority (the usual matrix, P1 to P4), and the
  priority the hours to resolve in: 4, 8, 24 or 72. A P1 on a critical item is major. A resolved
  incident says how. One resolved late, or still open past its time, has breached, and stays so.
- **Problems.** A known error has its workaround. A resolved problem says its root cause, and
  resolves the incidents still open under it.
- **Changes.** A change ends after it starts. A normal or emergency one has its back-out plan, and
  shows its impact: every item that depends on or runs on what it changes. A standard change is
  approved when submitted. A normal one needs a Change Manager who did not raise it, and is never
  in a freeze; an emergency may be. No two approved changes touch one item at once. A submitted
  change keeps its item, kind, window and back-out plan.

## Develop

```bash
cargo test                       # needs FERRUM_ITSM_TEST_DATABASE_URL's database
cd ui && npm run build && npm run check
```

`tests/itsm_it.rs` walks an IT department:
- items and a loop refused;
- requests approved and fulfilled;
- incidents prioritised, resolved and breached;
- a problem resolving its incidents;
- changes through freezes, conflicts and their decisions;
- who may do what;
- discovery reports matched by name and key, a refused type, a loop and another source's item
  refused, and an item no longer reported marked and its owner told once.

`tests/assets_it.rs` (`FERRUM_ITSM_ASSETS_TEST_DATABASE_URL`) runs the connector with ERP and
Assets: items finding their laptop by serial number or naming it, an asset refused to a second
item, and custodian and location kept as Assets moves it.

## Not yet

- Scanning networks or reading cloud accounts itself: tools do, and report to the intake.
- Helpdesk tickets raised as incidents, and incidents told to their reporters by mail.
- Business hours, holidays and pauses in the targets (they run on the clock).
- A change advisory board of several approvers, and change calendars.

## Changelog

| Version | Highlights |
|---|---|
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
