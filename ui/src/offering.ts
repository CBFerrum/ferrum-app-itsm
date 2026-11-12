// A catalog item's form as its questions make it (src/desk.rs, `answers_fit`): a question is asked only when an earlier one has the
// answer it names, a required one is answered, and an answer is of its kind (a number, a date, Yes or No, one of the choices).
// The server checks again when the request is made and its words are shown as they are said; this keeps a person from sending what
// it will refuse.

import { t } from "@ferrum/client";
import { type Row, text } from "./data";

export interface Question {
  label: string;
  qtype: "Text" | "Number" | "Date" | "Yes/No" | "Choice";
  required: boolean;
  options: string[];
  onlyWhen: string;
  onlyWhenAnswer: string;
}

/** The questions of an offering, as the form asks them, in order. */
export function questionsOf(offering: Row): Question[] {
  const list = Array.isArray(offering.questions) ? (offering.questions as Row[]) : [];
  return list.map((q) => ({
    label: text(q.label),
    qtype: (["Number", "Date", "Yes/No", "Choice"].includes(text(q.qtype)) ? text(q.qtype) : "Text") as Question["qtype"],
    required: q.required === true,
    options: text(q.options).split(",").map((o) => o.trim()).filter(Boolean),
    onlyWhen: text(q.only_when),
    onlyWhenAnswer: text(q.only_when_answer).trim(),
  }));
}

const answerOf = (answers: Record<string, string>, label: string) => (answers[label] ?? "").trim();

/** Whether the question is asked, given what has been answered. */
export function asked(q: Question, answers: Record<string, string>): boolean {
  if (!q.onlyWhen) return true;
  const given = Object.entries(answers).find(([k]) => k.toLowerCase() === q.onlyWhen.toLowerCase())?.[1] ?? "";
  return given.trim().toLowerCase() === q.onlyWhenAnswer.toLowerCase();
}

/** What is wrong with the answers, by the question's label: what is needed and what is not of its kind. */
export function problems(questions: Question[], answers: Record<string, string>): Record<string, string> {
  const out: Record<string, string> = {};
  for (const q of questions) {
    if (!asked(q, answers)) continue;
    const a = answerOf(answers, q.label);
    if (!a) {
      if (q.required) out[q.label] = t("{label} needs an answer", { label: q.label });
      continue;
    }
    if (q.qtype === "Number" && !Number.isFinite(Number(a))) out[q.label] = t("{label} is a number", { label: q.label });
    else if (q.qtype === "Date" && !/^\d{4}-\d{2}-\d{2}$/.test(a)) out[q.label] = t("{label} is a date", { label: q.label });
    else if (q.qtype === "Yes/No" && a !== "Yes" && a !== "No") out[q.label] = t("{label} is Yes or No", { label: q.label });
    else if (q.qtype === "Choice" && !q.options.includes(a)) out[q.label] = t("{label} is one of the choices", { label: q.label });
  }
  return out;
}

/** The answers as the request carries them: only the questions asked, only those answered. */
export function answerRows(questions: Question[], answers: Record<string, string>): { question: string; answer: string }[] {
  return questions.filter((q) => asked(q, answers) && answerOf(answers, q.label)).map((q) => ({ question: q.label, answer: answerOf(answers, q.label) }));
}

/** An offering's fulfilment steps in the order they are done, by name. */
export function stepsOf(offering: Row): { name: string; hours: number; sequence: number }[] {
  const list = Array.isArray(offering.steps) ? (offering.steps as Row[]) : [];
  return list.map((s) => ({ name: text(s.step_name), hours: Number(s.target_hours ?? 8), sequence: Number(s.sequence ?? 10) })).sort((a, b) => a.sequence - b.sequence);
}

/** The catalog's items a search leaves: by name or description, all of them for no search. */
export function search(offerings: Row[], q: string): Row[] {
  const w = q.trim().toLowerCase();
  return offerings.filter((o) => o.active !== false && (!w || `${text(o.offering_name)} ${text(o.description)}`.toLowerCase().includes(w)));
}

/** The tasks of a request as a person reads them: those open or done before those waiting. */
export const taskOrder = (tasks: Row[]) => [...tasks].sort((a, b) => Number(a.sequence ?? 0) - Number(b.sequence ?? 0));
