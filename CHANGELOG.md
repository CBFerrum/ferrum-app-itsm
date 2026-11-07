# Changelog

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
