import { describe, expect, it } from "vitest";
import { emptyDraft, flagLines, fromRow, inFreeze, levelOf, missing, routeLines, shortages, toBody } from "./draft";

describe("a change as a draft", () => {
  it("is saved with its answers as rows, its moments as instants and empty plans left empty", () => {
    const d = { ...emptyDraft(), title: " Patch ", ci: "c1", affected: ["c2"], answers: { "Has this change been done before?": "Never", "Can it be backed out?": "" }, planned_start: "2026-11-30T10:00", planned_end: "2026-11-30T12:00", backout_plan: " Roll back " };
    const b = toBody(d);
    expect(b.title).toBe("Patch");
    expect(b.affected).toEqual([{ item: "c2" }]);
    expect(b.risk_answers).toEqual([{ question: "Has this change been done before?", answer: "Never" }]);
    expect(b.change_plan).toBeNull();
    expect(b.backout_plan).toBe("Roll back");
    expect(String(b.planned_start)).toMatch(/^2026-11-30T\d\d:\d\d:00\.000Z$/);
  });

  it("is read back from the saved record", () => {
    const d = fromRow({ id: "x", title: "Patch", ci: "c1", change_type: "t1", risk: "High", affected: [{ item: "c2" }], risk_answers: [{ question: "Q", answer: "A", score: 2 }], planned_start: "2026-11-30T10:00:00Z", backout_plan: "Undo" });
    expect([d.ci, d.change_type, d.risk, d.affected, d.answers, d.backout_plan]).toEqual(["c1", "t1", "High", ["c2"], { Q: "A" }, "Undo"]);
    expect(d.planned_start).toMatch(/^2026-11-30T\d\d:\d\d$/);
  });

  it("says what is missing before anything is sent, with the back-out plan only where the type asks for one", () => {
    const d = { ...emptyDraft(), title: "x" };
    expect(missing(d, true).map((m) => m.field)).toEqual(["ci", "backout_plan", "planned_start"]);
    expect(missing(d, false).map((m) => m.field)).toEqual(["ci", "planned_start"]);
    expect(missing({ ...d, ci: "c", planned_start: "2026-11-30T12:00", planned_end: "2026-11-30T10:00" }, false).map((m) => m.field)).toEqual(["planned_end"]);
  });
});

describe("what the server works out", () => {
  it("gives the level a score reaches against the settings' two thresholds", () => {
    expect([0, 3, 4, 7, 8, 20].map((n) => levelOf(n, 4, 8))).toEqual(["Low", "Low", "Medium", "Medium", "High", "High"]);
  });

  it("tells the route in words: the board with its members, a second look for high risk, and a review afterwards", () => {
    const type = { id: "t", path: "Board", _titles: { board: "Change Board", high_risk_group: "Executives" } };
    const lines = routeLines({ route: "Board", quorum: 2, board_members: ["a@x.test", "b@x.test", "c@x.test"], second_look: true, escalation_members: ["e@x.test"], review_required: true }, type);
    expect(lines).toEqual([
      "2 of 3 members of Change Board approve: a@x.test, b@x.test, c@x.test",
      "Then one member of Executives approves it as well (high risk): e@x.test",
      "It is reviewed after it is implemented, before it can be closed.",
    ]);
    expect(routeLines({ route: "Manager" }, undefined)).toEqual(["One Change Manager, other than who raised it, approves it."]);
    expect(routeLines({ route: "Pre-approved" }, undefined)[0]).toMatch(/^No one decides/);
    expect(routeLines({ route: "Board", quorum: 1, board_members: ["a@x.test"] }, { id: "t", path: "Pre-approved" })[1]).toMatch(/without an approved template/);
  });

  it("splits the calendar's flags into the collision and the freeze, in the server's words", () => {
    const lines = flagLines("Same window, same service or item: CHG-1, CHG-2. In the freeze for Year end");
    expect(lines).toEqual(["Same window, same service or item: CHG-1, CHG-2", "In the freeze for Year end"]);
    expect(lines.map(inFreeze)).toEqual([false, true]);
    expect(flagLines(null)).toEqual([]);
  });
});

describe("an approving group too small for its quorum", () => {
  const type = { id: "t", path: "Board", board: "g1", high_risk_group: "g2", _titles: { board: "Change Board", high_risk_group: "Executives" } };
  it("is a shortage, not '2 of 0'", () => {
    const d = { route: "Board", quorum: 2, board_members: [] as string[], second_look: true, escalation_members: [] as string[] };
    expect(shortages(d, type)).toEqual([
      { group: "Change Board", groupId: "g1", have: 0, need: 2 },
      { group: "Executives", groupId: "g2", have: 0, need: 1 },
    ]);
    expect(routeLines(d, type).join(" ")).not.toContain(" of 0 ");
  });
  it("is none when the members suffice or no one is asked", () => {
    expect(shortages({ route: "Board", quorum: 2, board_members: ["a", "b"] }, type)).toEqual([]);
    expect(shortages({ route: "Manager" }, type)).toEqual([]);
  });
});
