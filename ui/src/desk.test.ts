import { describe, expect, it } from "vitest";
import { byPriority, changeLine, incidentFlags, priorityTone, requestLine, targetFlag } from "./Home";

describe("the desk", () => {
  it("puts P1 first, then the soonest due", () => {
    const l = byPriority([
      { id: "c", priority: "P3", resolve_by: "2026-11-05T10:00:00Z" },
      { id: "b", priority: "P1", resolve_by: "2026-11-04T12:00:00Z" },
      { id: "a", priority: "P1", resolve_by: "2026-11-04T09:00:00Z" },
    ]);
    expect(l.map((x) => x.id)).toEqual(["a", "b", "c"]);
  });

  it("says what a request asks for, who asked and when it is due, and when a change starts", () => {
    const r = requestLine({ id: "r", number: "REQ-1", _titles: { offering: "New laptop" }, requested_by: "ana@x.test", due_at: "2026-11-08T20:21:48Z", status: "Submitted" });
    expect(r.startsWith("REQ-1 · New laptop · by ana@x.test · due ")).toBe(true);
    expect(r.endsWith(" · Submitted")).toBe(true);
    expect(requestLine({ id: "r", number: "REQ-2", status: "Approved" })).toBe("REQ-2 · Approved");
    expect(changeLine({ id: "c", number: "CHG-1", title: "Patch", planned_start: "2026-11-10T22:00:00Z", status: "Approved" })).toMatch(/^CHG-1 · Patch · from .+ · Approved$/);
  });

  it("keeps a priority's tone from its stored id", () => {
    expect(["P1", "P2", "P3", "P4", "P9", "", null].map(priorityTone)).toEqual(["red", "amber", "blue", "neutral", "neutral", "neutral", "neutral"]);
    expect(priorityTone("toString")).toBe("neutral");
  });

  it("flags a target breached as the platform recorded it, and one running that has used three quarters of its time", () => {
    const [opened, due] = ["2026-11-04T08:00:00Z", "2026-11-04T12:00:00Z"];
    const at = (h: number) => Date.parse(`2026-11-04T${String(h).padStart(2, "0")}:00:00Z`);
    expect(targetFlag("breached", opened, due, at(9))).toBe("breached");
    expect(targetFlag("running", opened, due, at(9))).toBeNull();
    expect(targetFlag("running", opened, due, at(11))).toBe("at_risk");
    for (const state of ["met", "met_late", "paused", null, undefined]) expect(targetFlag(state, opened, due, at(11))).toBeNull();
    expect(targetFlag("running", opened, null, at(11))).toBeNull();
  });

  it("reads response and resolution apart, from the fields this release has", () => {
    const i = { id: "i", opened_at: "2026-11-04T08:00:00Z", respond_state: "breached", respond_by: "2026-11-04T08:15:00Z", resolve_state: "running", resolve_by: "2026-11-04T12:00:00Z" };
    expect(incidentFlags(i, Date.parse("2026-11-04T11:30:00Z"))).toEqual({ response: "breached", resolution: "at_risk" });
    expect(incidentFlags({ id: "j", breached: true }, 0)).toEqual({ response: null, resolution: null });
  });
});
