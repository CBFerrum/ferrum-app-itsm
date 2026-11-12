import { describe, expect, it } from "vitest";
import { byDay, daysCovered, entriesOf, freezeDays, itemsOf, monthDays, parseWindow, shiftMonth, touching } from "./month";

const report = {
  rows: [
    { window: "2026-11-30 10:00 to 2026-11-30 12:00", number: "CHG-1", title: "Shop release", type: "Normal", status: "Submitted", items: "Orders DB, Shop", collides: "CHG-2", freeze: "" },
    { window: "2026-11-30 11:00 to 2026-12-01 01:00", number: "CHG-2", title: "Banner", type: "Normal", status: "Approved", items: "Shop", collides: "CHG-1", freeze: "Year end" },
    { window: "soon", number: "CHG-3", title: "Odd", type: "", status: "", items: "", collides: "", freeze: "" },
  ],
};

describe("the change calendar", () => {
  it("reads a window of the report as two instants, and leaves out one it cannot read", () => {
    expect(parseWindow("2026-11-30 10:00 to 2026-11-30 12:00")).toEqual({ start: "2026-11-30T10:00:00Z", end: "2026-11-30T12:00:00Z" });
    expect(parseWindow("soon")).toBeNull();
    expect(entriesOf(report).map((e) => e.number)).toEqual(["CHG-1", "CHG-2"]);
  });

  it("marks what collides and what is frozen, and filters by the item or service touched", () => {
    const entries = entriesOf(report);
    expect(entries[0].collides).toEqual(["CHG-2"]);
    expect(entries[1].freeze).toEqual(["Year end"]);
    expect(itemsOf(entries)).toEqual(["Orders DB", "Shop"]);
    expect(touching(entries, "Orders DB").map((e) => e.number)).toEqual(["CHG-1"]);
    expect(touching(entries, "")).toHaveLength(2);
  });

  it("lays a change that runs over midnight on both days", () => {
    const days = byDay(entriesOf(report));
    expect(days.get("2026-11-30")?.map((e) => e.number)).toEqual(["CHG-1", "CHG-2"]);
    expect(days.get("2026-12-01")?.map((e) => e.number)).toEqual(["CHG-2"]);
    expect(daysCovered("2026-11-30T10:00:00Z", "2026-11-30T12:00:00Z")).toEqual(["2026-11-30"]);
  });

  it("marks the days of a freeze", () => {
    const f = freezeDays([{ id: "f", reason: "Year end", starts_at: "2026-12-24T00:00:00Z", ends_at: "2026-12-26T12:00:00Z" }]);
    expect([...f.keys()]).toEqual(["2026-12-24", "2026-12-25", "2026-12-26"]);
  });

  it("draws whole weeks from the day a week begins on, and moves between months", () => {
    const monday = monthDays("2026-11", 1);
    expect(monday[0]).toBe("2026-10-26");
    expect(monday).toHaveLength(42);
    expect(monthDays("2026-11", 7)[0]).toBe("2026-11-01");
    expect(monthDays("2026-11", 7)).toHaveLength(35);
    expect(shiftMonth("2026-12", 1)).toBe("2027-01");
    expect(shiftMonth("2026-01", -1)).toBe("2025-12");
  });
});
