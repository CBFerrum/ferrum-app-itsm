import { describe, expect, it } from "vitest";
import { byPriority, changeLine, incidentFlags, inTrouble, majors, priorityTone, requestLine, split, targetFlag, updateOverdue } from "./desk";

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

  it("splits what is mine from what my team has, and leaves out what another team has", () => {
    const l = [
      { id: "a", assigned_to: "Ana@x.test", assignment_group: "g1" },
      { id: "b", assigned_to: "bo@x.test", assignment_group: "g1" },
      { id: "c", assigned_to: null, assignment_group: "g1" },
      { id: "d", assigned_to: "bo@x.test", assignment_group: "g2" },
    ];
    const { mine, team } = split(l, "ana@x.test", new Set(["g1"]));
    expect(mine.map((r) => r.id)).toEqual(["a"]);
    expect(team.map((r) => r.id)).toEqual(["b", "c"]);
    expect(split(l, "zed@x.test", new Set()).team).toEqual([]);
  });

  it("lists the targets in trouble across incidents, requests and tasks, breached first", () => {
    const now = Date.parse("2026-11-04T11:30:00Z");
    const inc = { id: "i", opened_at: "2026-11-04T08:00:00Z", resolve_state: "running", resolve_by: "2026-11-04T12:00:00Z", respond_state: "met" };
    const req = { id: "r", created_at: "2026-11-01T08:00:00Z", fulfil_state: "breached", due_at: "2026-11-03T08:00:00Z" };
    const task = { id: "k", created_at: "2026-11-04T08:00:00Z", task_state: "paused", due_by: "2026-11-04T12:00:00Z" };
    const hits = inTrouble([inc], [req], [task], now);
    expect(hits.map((h) => [h.entity, h.flag])).toEqual([["CatalogRequest", "breached"], ["Incident", "at_risk"]]);
  });

  it("finds the major incidents and the ones whose next update is overdue", () => {
    expect(majors([{ id: "a", major: true }, { id: "b", major: false }, { id: "c" }]).map((r) => r.id)).toEqual(["a"]);
    const now = Date.parse("2026-11-04T10:00:00Z");
    expect(updateOverdue({ id: "a", next_update_by: "2026-11-04T09:00:00Z" }, now)).toBe(true);
    expect(updateOverdue({ id: "a", next_update_by: "2026-11-04T11:00:00Z" }, now)).toBe(false);
    expect(updateOverdue({ id: "a" }, now)).toBe(false);
  });
});
