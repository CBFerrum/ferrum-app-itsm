# Changelog

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
