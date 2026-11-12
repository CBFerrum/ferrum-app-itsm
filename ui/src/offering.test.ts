import { describe, expect, it } from "vitest";
import { answerRows, asked, problems, questionsOf, search, stepsOf } from "./offering";

const offering = {
  id: "o",
  questions: [
    { label: "Which laptop?", qtype: "Choice", required: true, options: "Standard, Developer" },
    { label: "Why a developer laptop?", qtype: "Text", required: true, only_when: "Which laptop?", only_when_answer: "Developer" },
    { label: "How many?", qtype: "Number" },
    { label: "Needed by", qtype: "Date" },
    { label: "Urgent?", qtype: "Yes/No" },
  ],
  steps: [{ step_name: "Ship", sequence: 20, target_hours: 24 }, { step_name: "Image", sequence: 10, target_hours: 4 }],
};
const qs = questionsOf(offering);

describe("a catalog item's form", () => {
  it("asks a question only when the earlier one has the answer it names", () => {
    expect(asked(qs[1], {})).toBe(false);
    expect(asked(qs[1], { "Which laptop?": "developer" })).toBe(true);
    expect(asked(qs[0], {})).toBe(true);
  });

  it("holds answers to their kind, and asks for the required ones that are asked", () => {
    expect(problems(qs, {})).toEqual({ "Which laptop?": "Which laptop? needs an answer" });
    expect(problems(qs, { "Which laptop?": "Developer" })).toEqual({ "Why a developer laptop?": "Why a developer laptop? needs an answer" });
    expect(problems(qs, { "Which laptop?": "Standard", "How many?": "two", "Needed by": "tomorrow", "Urgent?": "Maybe" })).toEqual({
      "How many?": "How many? is a number",
      "Needed by": "Needed by is a date",
      "Urgent?": "Urgent? is Yes or No",
    });
    expect(problems(qs, { "Which laptop?": "Gaming" })).toEqual({ "Which laptop?": "Which laptop? is one of the choices" });
    expect(problems(qs, { "Which laptop?": "Standard", "How many?": "2", "Needed by": "2026-12-01", "Urgent?": "Yes" })).toEqual({});
  });

  it("sends only the answers of the questions asked", () => {
    expect(answerRows(qs, { "Which laptop?": "Standard", "Why a developer laptop?": "stale", "How many?": " 2 " })).toEqual([
      { question: "Which laptop?", answer: "Standard" },
      { question: "How many?", answer: "2" },
    ]);
  });

  it("lists its steps in order and searches by name or description", () => {
    expect(stepsOf(offering).map((s) => s.name)).toEqual(["Image", "Ship"]);
    const list = [{ id: "a", offering_name: "New laptop", description: "A machine" }, { id: "b", offering_name: "VPN access", description: "Remote work" }, { id: "c", offering_name: "Old", active: false }];
    expect(search(list, "work").map((o) => o.id)).toEqual(["b"]);
    expect(search(list, "").map((o) => o.id)).toEqual(["a", "b"]);
  });
});
