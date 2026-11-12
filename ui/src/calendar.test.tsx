// @vitest-environment jsdom
// The change calendar as it is drawn: a month with its changes on the days they cover, a collision and a freeze said in words,
// the list, the filter by an item, an empty month and a refused read.

import { afterEach, describe, expect, it, vi } from "vitest";
import { act } from "react";
import { CalendarRoute } from "./Calendar";
import { byText, draw, fakeServer, settle } from "./testkit";

afterEach(() => {
  vi.unstubAllGlobals();
  document.body.innerHTML = "";
});

const report = {
  columns: [],
  totals: {},
  meta: {},
  rows: [
    { window: "2026-11-30 10:00 to 2026-11-30 12:00", number: "CHG-1", title: "Shop release", type: "Normal", status: "Submitted", items: "Orders DB, Shop", collides: "CHG-2", freeze: "" },
    { window: "2026-11-30 11:00 to 2026-11-30 13:00", number: "CHG-2", title: "Banner", type: "Normal", status: "Approved", items: "Shop", collides: "CHG-1", freeze: "Year end" },
  ],
};
const routes: [string, string, unknown, number?][] = [
  ["GET", "/api/app/itsm/calendar", report],
  ["GET", "/api/v1/ChangeFreeze", { rows: [{ id: "f", reason: "Year end", starts_at: "2026-11-30T00:00:00Z", ends_at: "2026-11-30T23:00:00Z" }] }],
  ["GET", "/api/v1/ItChange", { rows: [{ id: "a", number: "CHG-1" }, { id: "b", number: "CHG-2" }] }],
];

describe("the change calendar", () => {
  it("asks for the days of the month shown and lays the changes on their days, with the collision and the freeze in words", async () => {
    const server = fakeServer(routes);
    const host = await draw(<CalendarRoute />, "/calendar", "/calendar?m=2026-11");
    await settle(200);
    expect(server.seen.some((s) => /^GET \/api\/app\/itsm\/calendar\?from_date=2026-(10|11)-\d\d&to_date=2026-12-\d\d$/.test(s.line))).toBe(true);
    const text = host.textContent ?? "";
    expect(text).toContain("CHG-1 Shop release");
    expect(text).toContain("Collides with CHG-2");
    expect(text).toContain("In the freeze for Year end");
    expect(text).toContain("Freeze: Year end");
    expect(host.querySelector('a[href="/change/a"]')).toBeTruthy();
  });

  it("lists the changes by window and filters by the item or service they touch", async () => {
    fakeServer(routes);
    const host = await draw(<CalendarRoute />, "/calendar", "/calendar?m=2026-11&v=list");
    await settle(200);
    expect(host.querySelectorAll("tbody tr")).toHaveLength(2);
    const select = host.querySelector<HTMLSelectElement>("#itsm-cal-item")!;
    expect([...select.options].map((o) => o.value)).toEqual(["", "Orders DB", "Shop"]);
    const host2 = await draw(<CalendarRoute />, "/calendar", "/calendar?m=2026-11&v=list&item=Orders%20DB");
    await settle(200);
    expect(host2.querySelectorAll("tbody tr")).toHaveLength(1);
  });

  it("says a month with nothing scheduled is empty, and a refused read as refused", async () => {
    fakeServer([["GET", "/api/app/itsm/calendar", { columns: [], rows: [], totals: {}, meta: {} }], ["GET", "/api/v1/ChangeFreeze", { rows: [] }]]);
    const host = await draw(<CalendarRoute />, "/calendar", "/calendar?m=2026-11&v=list");
    await settle(200);
    expect(host.textContent).toContain("Nothing is scheduled this month");
    vi.unstubAllGlobals();
    fakeServer([["GET", "/api/app/itsm/calendar", { error: "forbidden", message: "no" }, 403]]);
    const denied = await draw(<CalendarRoute />, "/calendar", "/calendar?m=2026-11");
    await settle(200);
    expect(denied.textContent).toContain("You don't have access");
    expect(byText(denied, "button", "Try again")).toBeUndefined();
    await act(async () => undefined);
  });
});
