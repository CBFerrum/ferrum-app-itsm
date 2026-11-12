// What the desk is made of, kept apart from how it is drawn: whose a record is, which targets are in trouble, and how a
// record reads in a queue. Target states are the platform's (running, paused, met, met_late, breached).

import { formatMoment, isolate, t } from "@ferrum/client";
import type { Tone } from "@ferrum/kit";
import { type Row, text, titleOf } from "./data";

/** Open incidents, P1 first, then the soonest due. */
export function byPriority(list: Row[]): Row[] {
  return [...list].sort((a, b) => String(a.priority ?? "P9").localeCompare(String(b.priority ?? "P9")) || String(a.resolve_by ?? "").localeCompare(String(b.resolve_by ?? "")));
}

const TONE: Record<string, Tone> = { P1: "red", P2: "amber", P3: "blue", P4: "neutral" };

/** The tone of a priority mark, from its stored id (P1..P4); any other value is neutral. */
export const priorityTone = (p: unknown) => (typeof p === "string" && Object.prototype.hasOwnProperty.call(TONE, p) ? TONE[p] : "neutral");

/** A running target is at risk once this share of the time from opening to its due moment has gone (the due moment counts
 * working hours, so it is a guide, not the platform's word: that one is `breached`). */
const AT_RISK = 0.75;

export type Flag = "breached" | "at_risk";

/** What a target's state word, with when the record opened and when the target is due, says about it: breached as the
 * platform recorded it, at risk while it runs and most of its time has gone; nothing for one met, paused or not started. */
export function targetFlag(state: unknown, opened: unknown, due: unknown, now = Date.now()): Flag | null {
  if (state === "breached") return "breached";
  if (state !== "running" || typeof opened !== "string" || typeof due !== "string") return null;
  const [from, to] = [Date.parse(opened), Date.parse(due)];
  return to > from && (now - from) / (to - from) >= AT_RISK ? "at_risk" : null;
}

/** An incident's response and resolution, each flagged or not. */
export const incidentFlags = (i: Row, now = Date.now()) => ({
  response: targetFlag(i.respond_state, i.opened_at, i.respond_by, now),
  resolution: targetFlag(i.resolve_state, i.opened_at, i.resolve_by, now),
});

/** A moment on the shown clock: "8 Nov, 20:21". */
export const when = (at: unknown) => (typeof at === "string" && at ? formatMoment(at, { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" }) : "");

/** The parts of a line that are there, joined. */
export const parts = (...p: unknown[]) => isolate(p.filter(Boolean).join(" · "));

/** A request as the desk reads it: what was asked for, by whom, and when it is due. */
export function requestLine(r: Row): string {
  const asked = titleOf(r, "offering");
  const due = when(r.due_at);
  return [text(r.number), asked, r.requested_by ? t("by {value}", { value: text(r.requested_by) }) : "", due ? t("due {due}", { due }) : "", text(r.status)].filter(Boolean).join(" · ");
}

/** A change as the desk reads it: what it is, when it starts, and where it stands. */
export function changeLine(c: Row): string {
  const starts = when(c.planned_start);
  return [text(c.number), text(c.title), starts ? t("from {starts}", { starts }) : "", text(c.status)].filter(Boolean).join(" · ");
}

// ── whose ──

const same = (a: unknown, b: string) => text(a).toLowerCase() === b.toLowerCase();

/** Records the person has (assigned to them) and those their teams have (assigned to a group they belong to, to someone else or to
 *  no one yet). A person with no team has no team's queue. */
export function split(list: Row[], email: string, groups: ReadonlySet<string>): { mine: Row[]; team: Row[] } {
  const mine = list.filter((r) => same(r.assigned_to, email));
  const team = list.filter((r) => !same(r.assigned_to, email) && groups.has(text(r.assignment_group)));
  return { mine, team };
}

// ── targets in trouble ──

export interface Hit {
  entity: "Incident" | "CatalogRequest" | "RequestTask";
  row: Row;
  /** Which target: the words a person reads. */
  target: string;
  flag: Flag;
  due: string;
}

/** Every target of an open incident, request or task that is breached or at risk, breached first and then the soonest due. */
export function inTrouble(incidents: Row[], requests: Row[], tasks: Row[], now = Date.now()): Hit[] {
  const hits: Hit[] = [];
  const look = (entity: Hit["entity"], row: Row, target: string, state: unknown, opened: unknown, due: unknown) => {
    const flag = targetFlag(state, opened, due, now);
    if (flag) hits.push({ entity, row, target, flag, due: text(due) });
  };
  for (const i of incidents) {
    look("Incident", i, t("Response"), i.respond_state, i.opened_at, i.respond_by);
    look("Incident", i, t("Resolution"), i.resolve_state, i.opened_at, i.resolve_by);
  }
  for (const r of requests) {
    look("CatalogRequest", r, t("Response"), r.respond_state, r.created_at, r.respond_by);
    look("CatalogRequest", r, t("Fulfilment"), r.fulfil_state, r.created_at, r.due_at);
  }
  for (const k of tasks) look("RequestTask", k, t("Completion"), k.task_state, k.created_at, k.due_by);
  return hits.sort((a, b) => Number(b.flag === "breached") - Number(a.flag === "breached") || a.due.localeCompare(b.due));
}

/** Open incidents that are major: declared, or P1 on a critical item (the server's `major`). */
export const majors = (incidents: Row[]) => incidents.filter((i) => i.major === true);

/** Whether a major incident's next update is overdue. */
export const updateOverdue = (i: Row, now = Date.now()) => typeof i.next_update_by === "string" && Date.parse(i.next_update_by) < now;
