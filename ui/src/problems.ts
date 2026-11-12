// Problems and known errors (src/desk.rs): which problems a list shows, which of a problem's incidents are still open, and when
// the button that resolves them is offered.

import { type Filter, type Row, text } from "./data";

export type Show = "open" | "known" | "resolved";

/** The filters of each tab of the problem list. */
export const FILTERS: Record<Show, Filter[]> = {
  open: [["status", "in", ["Open", "Known Error"]]],
  known: [["known_error", "=", true]],
  resolved: [["status", "in", ["Resolved", "Closed"]]],
};

export const showOf = (v: string | null): Show => (v === "known" || v === "resolved" ? v : "open");

/** The statuses of an incident that is still being worked, which the problem's resolution reaches. */
export const OPEN_INCIDENT = ["New", "In Progress", "On Hold"];

/** The incidents of a problem still open (Awaiting Caller is the caller's to answer and is not resolved with it). */
export const stillOpen = (incidents: Row[]) => incidents.filter((i) => OPEN_INCIDENT.includes(text(i.status)));

/** Whether the problem offers to resolve its incidents: it is resolved or closed and some of them are open. */
export const offersResolve = (problem: Row, incidents: Row[]) => ["Resolved", "Closed"].includes(text(problem.status)) && stillOpen(incidents).length > 0;

/** How many open incidents each problem has, from the incidents that name one. */
export function openByProblem(incidents: Row[]): Map<string, number> {
  const out = new Map<string, number>();
  for (const i of incidents) {
    const p = text(i.problem);
    if (p) out.set(p, (out.get(p) ?? 0) + 1);
  }
  return out;
}

/** What a problem is saved with: the words and the two boxes, empty words as nothing. */
export interface ProblemDraft {
  title: string;
  symptoms: string;
  status: string;
  known_error: boolean;
  workaround: string;
  workaround_visible: boolean;
  root_cause: string;
}

export const draftOf = (p: Row): ProblemDraft => ({
  title: text(p.title),
  symptoms: text(p.symptoms),
  status: text(p.status) || "Open",
  known_error: p.known_error === true,
  workaround: text(p.workaround),
  workaround_visible: p.workaround_visible === true,
  root_cause: text(p.root_cause),
});

export const bodyOfProblem = (d: ProblemDraft): Record<string, unknown> => ({
  title: d.title.trim(),
  symptoms: d.symptoms.trim() || null,
  status: d.status,
  known_error: d.known_error,
  workaround: d.workaround.trim() || null,
  workaround_visible: d.workaround_visible,
  root_cause: d.root_cause.trim() || null,
});
