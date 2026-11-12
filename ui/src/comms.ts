// A major incident's communication (packages/itsm/package.yaml, `messages`; entities/itsm.yaml, the actions declare_major,
// major_update and end_major): who will be told, and how each message reads. The wording is the one the package ships under
// the keys itsm.major_declared, itsm.major_update and itsm.major_resolved; a workspace may have edited it under Messages, and no
// route gives a screen a workspace's own wording, so the preview says it is the shipped one.

import { type Row, text } from "./data";

export type Kind = "declared" | "update" | "resolved";

export interface Told {
  /** The addresses the message goes to. */
  who: { address: string; note: string }[];
  /** Why no one is told, when no one is. */
  why: "no_service" | "no_subscribers" | null;
}

/** Who a message is sent to: the subscribers of the incident's business service (`links:service.subscribers.subscriber`). */
export function told(incident: Row, service: Row | null): Told {
  if (!text(incident.service)) return { who: [], why: "no_service" };
  const rows = Array.isArray(service?.subscribers) ? (service.subscribers as Row[]) : [];
  const who = rows.filter((r) => text(r.subscriber)).map((r) => ({ address: text(r.subscriber), note: text(r.note) }));
  return { who, why: who.length ? null : "no_subscribers" };
}

/** The message as the shipped template makes it, with what was typed in. */
export function message(kind: Kind, incident: Row, v: { plan?: string; update?: string; resolution?: string }): { subject: string; text: string } {
  const [n, title] = [text(incident.number), text(incident.title)];
  switch (kind) {
    case "declared":
      return { subject: `Major incident ${n}: ${title}`, text: `Major incident ${n}: ${title}. ${v.plan ?? ""}`.trim() };
    case "update":
      return { subject: `Update on ${n}: ${title}`, text: `${n}: ${v.update ?? ""}`.trim() };
    default:
      return { subject: `Resolved: ${n} ${title}`, text: `Resolved: ${n} ${title}. ${v.resolution ?? ""}`.trim() };
  }
}

/** The updates sent so far: the comments the caller sees on the incident, oldest first. */
export const updatesOf = (notes: Row[]) => notes.filter((n) => text(n.visibility) === "Visible to Caller").sort((a, b) => text(a.posted_at).localeCompare(text(b.posted_at)));

/** Whether the incident can still be declared on, updated or ended. */
export const isOpen = (i: Row) => !["Resolved", "Closed"].includes(text(i.status));
