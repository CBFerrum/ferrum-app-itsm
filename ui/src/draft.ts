// A change as a person fills it in and as the server reads it back (src/changes.rs): the draft behind the form, the body that
// is saved, what the questionnaire adds up to and where that sends the change, and the words of the calendar's flags.

import { formatMoment, fromZonedInput, t, toZonedInput } from "@ferrum/client";
import { type Row, text } from "./data";

export type Level = "Low" | "Medium" | "High";

export interface Draft {
  title: string;
  change_type: string | null;
  ci: string | null;
  affected: string[];
  /** The question's text, to the label of the choice picked. */
  answers: Record<string, string>;
  /** Picked by hand; worked out from the answers once any is given. */
  risk: Level;
  change_plan: string;
  backout_plan: string;
  test_plan: string;
  /** `datetime-local` values on the shown clock. */
  planned_start: string;
  planned_end: string;
}

export const emptyDraft = (): Draft => ({ title: "", change_type: null, ci: null, affected: [], answers: {}, risk: "Medium", change_plan: "", backout_plan: "", test_plan: "", planned_start: "", planned_end: "" });

/** What is saved, or asked of the server to work out: moments as instants, empty plans left empty, an answer for each question picked. */
export function toBody(d: Draft): Record<string, unknown> {
  const plan = (v: string) => (v.trim() ? v.trim() : null);
  return {
    title: d.title.trim(),
    change_type: d.change_type,
    ci: d.ci,
    affected: d.affected.map((item) => ({ item })),
    risk_answers: Object.entries(d.answers).filter(([, a]) => a).map(([question, answer]) => ({ question, answer })),
    risk: d.risk,
    change_plan: plan(d.change_plan),
    backout_plan: plan(d.backout_plan),
    test_plan: plan(d.test_plan),
    planned_start: d.planned_start ? fromZonedInput(d.planned_start) : null,
    planned_end: d.planned_end ? fromZonedInput(d.planned_end) : null,
  };
}

/** A saved draft as the form shows it. */
export function fromRow(r: Row): Draft {
  const rowsOf = (k: string) => (Array.isArray(r[k]) ? (r[k] as Row[]) : []);
  return {
    title: text(r.title),
    change_type: text(r.change_type) || null,
    ci: text(r.ci) || null,
    affected: rowsOf("affected").map((a) => text(a.item)).filter(Boolean),
    answers: Object.fromEntries(rowsOf("risk_answers").map((a) => [text(a.question), text(a.answer)])),
    risk: (["Low", "Medium", "High"].includes(text(r.risk)) ? text(r.risk) : "Medium") as Level,
    change_plan: text(r.change_plan),
    backout_plan: text(r.backout_plan),
    test_plan: text(r.test_plan),
    planned_start: toZonedInput(r.planned_start),
    planned_end: toZonedInput(r.planned_end),
  };
}

/** What the form can say before the server is asked: what is missing from a draft, in the form's own order. */
export function missing(d: Draft, backoutNeeded: boolean): { field: keyof Draft; word: string }[] {
  const out: { field: keyof Draft; word: string }[] = [];
  if (!d.title.trim()) out.push({ field: "title", word: t("A change has a title") });
  if (!d.ci) out.push({ field: "ci", word: t("Name the item it changes") });
  if (backoutNeeded && !d.backout_plan.trim()) out.push({ field: "backout_plan", word: t("A normal or emergency change says how it is undone") });
  if (!d.planned_start || !d.planned_end) out.push({ field: "planned_start", word: t("Give the window: when it starts and when it ends") });
  else if (d.planned_end <= d.planned_start) out.push({ field: "planned_end", word: t("The window ends after it starts") });
  return out;
}

/** The level a score gives against the two thresholds in the settings. */
export const levelOf = (score: number, medium: number, high: number): Level => (score >= high ? "High" : score >= medium ? "Medium" : "Low");

/** What the server worked out of a draft (risk, impact, route, who approves), as lines to read. `type` is the change type as
 *  listed (its group's title, its path). */
export function routeLines(d: Record<string, unknown>, type: Record<string, unknown> | undefined): string[] {
  const members = (k: string) => (Array.isArray(d[k]) ? (d[k] as unknown[]).map(String) : []);
  const group = text((type?._titles as Record<string, string> | undefined)?.board) || t("the approving group");
  const high = text((type?._titles as Record<string, string> | undefined)?.high_risk_group) || t("the high-risk group");
  const out: string[] = [];
  switch (d.route) {
    case "Pre-approved":
      out.push(t("No one decides: it is from an approved template, approved in advance."));
      break;
    case "Board":
    case "Emergency": {
      const m = members("board_members");
      // A group too small to give its quorum is said apart (see `shortages`), not as "2 of 0".
      if (m.length >= Number(d.quorum ?? 1))
        out.push(
          d.route === "Emergency"
            ? t("{n} of {m} members of {group} approve (the smaller emergency group): {who}", { n: String(d.quorum ?? 1), m: m.length, group, who: m.join(", ") })
            : t("{n} of {m} members of {group} approve: {who}", { n: String(d.quorum ?? 1), m: m.length, group, who: m.join(", ") }),
        );
      if (d.second_look === true && members("escalation_members").length)
        out.push(t("Then one member of {group} approves it as well (high risk): {who}", { group: high, who: members("escalation_members").join(", ") }));
      if (type && type.path === "Pre-approved") out.push(t("A standard change without an approved template goes to the board its type names."));
      break;
    }
    case "Manager":
      out.push(t("One Change Manager, other than who raised it, approves it."));
      break;
    default:
      break;
  }
  if (d.review_required === true) out.push(t("It is reviewed after it is implemented, before it can be closed."));
  return out;
}

/** An approving group with fewer members than the type asks of it. The server refuses to submit such a change
 *  (`submit_change`: "its approving group has N members and the type asks M of them"), so it is said before the button. */
export interface Shortage {
  group: string;
  /** The group record, to link to it. */
  groupId?: string;
  have: number;
  need: number;
}

export function shortages(d: Record<string, unknown>, type: Record<string, unknown> | undefined): Shortage[] {
  if (d.route !== "Board" && d.route !== "Emergency") return [];
  const titles = type?._titles as Record<string, string> | undefined;
  const count = (k: string) => (Array.isArray(d[k]) ? (d[k] as unknown[]).length : 0);
  const out: Shortage[] = [];
  const need = Number(d.quorum ?? 1);
  if (count("board_members") < need) out.push({ group: text(titles?.board) || t("the approving group"), groupId: text(type?.board) || undefined, have: count("board_members"), need });
  if (d.second_look === true && !count("escalation_members"))
    out.push({ group: text(titles?.high_risk_group) || t("the high-risk group"), groupId: text(type?.high_risk_group) || undefined, have: 0, need: 1 });
  return out;
}

/** The server's flags on a window, one to a line: the other scheduled changes on the same item or service, and a freeze. */
export const flagLines = (flags: unknown): string[] => (typeof flags === "string" && flags ? flags.split(". ").map((s) => s.trim()).filter(Boolean) : []);

/** Whether a flag says the window falls in a freeze. */
export const inFreeze = (line: string) => line.startsWith("In the freeze");

/** The shown moment of a window's end points: "8 Nov, 20:21". */
export const windowWords = (start: unknown, end: unknown) =>
  typeof start === "string" && typeof end === "string" ? `${formatMoment(start, { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" })} – ${formatMoment(end, { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" })}` : "";
