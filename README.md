# IT Services

IT service management for [Ferrum](https://github.com/CBFerrum/Ferrum):
- configuration items and what they depend on or run on;
- a service catalog, and requests approved by someone other than who asked;
- incidents prioritised by impact and urgency, with their targets and breaches;
- problems, and changes with approval, conflicts, freezes and impact.

| | |
|---|---|
| 🛠️ App | `itsm`, native, in the server |
| 🧩 Package | `itsm` (needs only the platform's core) |
| 🖥️ Interface | its own, at `/itsm/`: open incidents by priority, requests waiting and changes coming up, and the records |
| 🔐 Roles | Service Desk, Change Manager, Configuration Manager, Requester |
| 📜 Licence | AGPL-3.0-only |

## Highlights

- **Configuration items.** An item depends on, runs on or connects to others. A dependency never
  comes back round to the item itself, however far; connections may.
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
- who may do what.

## Not yet

- Discovery of configuration items from the network, cloud accounts or agents.
- Helpdesk tickets raised as incidents, and incidents told to their reporters by mail.
- Business hours, holidays and pauses in the targets (they run on the clock).
- A change advisory board of several approvers, and change calendars.
- Asset records in Assets linked to items (an item has its serial number).

## Changelog

| Version | Highlights |
|---|---|
| **0.1.0** | Configuration items, a service catalog, incidents by priority with their targets, problems, and changes with approval, conflicts, freezes and impact (FERRUM-ECOSYSTEM-002, -003). |

## License

AGPL-3.0-only. See [LICENSE](LICENSE).
