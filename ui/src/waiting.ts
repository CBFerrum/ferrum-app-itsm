// What waits for the person's decision: the platform's approvals asked of them (the change board, a catalog item's approver
// group, with stand-ins as the platform works them out) and, for a Change Manager, what one Change Manager decides (the route
// Manager: a change, or a request for an offering that needs approval, that someone else raised).

import { type Row, text } from "./data";

/** `GET /api/approvals`: what the person has to decide and what they wait on. */
export interface MyApprovals {
  to_decide: { request: string; entity: string; record: string; title: string; action: string; requester: string; requested_at: string }[];
  awaiting_others: { request: string; entity: string; record: string; title: string; action: string }[];
}

export interface Waiting {
  entity: "ItChange" | "CatalogRequest";
  id: string;
  title: string;
  /** Why it is the person's: asked of them by the platform, or decided by any Change Manager. */
  by: "approval" | "manager";
  from: string;
}

export function awaitingMe(
  approvals: MyApprovals | null,
  changes: Row[],
  requests: Row[],
  needing: ReadonlySet<string>,
  me: { id: string; email: string; decider: boolean },
): Waiting[] {
  const out: Waiting[] = [];
  const seen = new Set<string>();
  const add = (w: Waiting) => {
    const key = `${w.entity}/${w.id}`;
    if (!seen.has(key)) (seen.add(key), out.push(w));
  };
  for (const a of approvals?.to_decide ?? []) {
    if (a.entity === "ItChange" || a.entity === "CatalogRequest") add({ entity: a.entity, id: a.record, title: a.title, by: "approval", from: a.requested_at });
  }
  if (me.decider) {
    for (const c of changes)
      if (c.status === "Submitted" && c.route === "Manager" && text(c.created_by) !== me.id) add({ entity: "ItChange", id: c.id, title: text(c.title) || text(c.number), by: "manager", from: text(c.created_at) });
    for (const r of requests)
      if (r.status === "Submitted" && r.route === "Manager" && needing.has(text(r.offering)) && text(r.requested_by).toLowerCase() !== me.email.toLowerCase())
        add({ entity: "CatalogRequest", id: r.id, title: text(r.number), by: "manager", from: text(r.created_at) });
  }
  return out;
}
