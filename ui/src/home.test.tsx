// @vitest-environment jsdom
// The desk as a person at the service desk, and as a person with no desk, meets it: their queue and their team's, what waits for
// their decision, the targets in trouble, the banner of a major incident, the quick report with the known problems that fit, and
// what a refusal looks like.

import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { act } from "react";
import { Home } from "./Home";
import { byText, draw, fakeServer, settle, until } from "./testkit";

afterEach(() => {
  vi.unstubAllGlobals();
  document.body.innerHTML = "";
});

const all = { read: "all", write: "all", create: "all" };
const meta = (perms: Record<string, Record<string, string>>) => ["GET", "/api/meta", { entities: Object.entries(perms).map(([name, permissions]) => ({ name, permissions })) }] as [string, string, unknown];
const me = (roles: string[]) => ["GET", "/api/auth/me", { tenant: "t", user: { id: "u1", email: "ana@x.test", full_name: "Ana Rao", is_admin: false, roles } }] as [string, string, unknown];

const incidents = [
  { id: "i1", number: "INC-1", title: "Mail is down", priority: "P1", status: "In Progress", assigned_to: "ana@x.test", assignment_group: "g1", opened_at: "2026-11-04T08:00:00Z", respond_state: "met", resolve_state: "breached", resolve_by: "2026-11-04T12:00:00Z" },
  { id: "i2", number: "INC-2", title: "Printer jam", priority: "P3", status: "New", assigned_to: "bo@x.test", assignment_group: "g1" },
  { id: "i3", number: "INC-3", title: "Payments are failing", priority: "P1", status: "In Progress", major: true, next_update_by: "2020-01-01T00:00:00Z", assignment_group: "g9" },
];

const staff = [
  meta({ Incident: all, CatalogRequest: all, RequestTask: all, ItChange: all, ItGroup: all, ConfigurationItem: all, ServiceOffering: all, Problem: all }),
  me(["Service Desk", "Change Manager"]),
  ["GET", "/api/approvals", { to_decide: [{ request: "ap1", entity: "ItChange", record: "ch1", title: "Core switch", action: "approve_change", requester: "u9", requested_at: "2026-12-01T09:00:00Z" }], awaiting_others: [] }],
  ["GET", "/api/app/itsm/suggest", { suggestions: [{ id: "k1", title: "Printer queue stuck", workaround: "Restart the spooler" }] }],
  ["GET", "/api/v1/Incident", { rows: incidents }],
  ["GET", "/api/v1/CatalogRequest", { rows: [{ id: "r1", number: "REQ-1", status: "Submitted", assigned_to: "ana@x.test", _titles: { offering: "New laptop" } }] }],
  ["GET", "/api/v1/RequestTask", { rows: [] }],
  ["GET", "/api/v1/ItChange", { rows: [{ id: "ch2", number: "CHG-2", title: "Patch", status: "Submitted", route: "Manager", created_by: "u7", planned_start: "2026-12-02T10:00:00Z" }] }],
  ["GET", "/api/v1/ItGroup", { rows: [{ id: "g1" }] }],
  ["GET", "/api/v1/ConfigurationItem", { rows: [] }],
  ["GET", "/api/v1/ServiceOffering", { rows: [] }],
] as [string, string, unknown, number?][];

describe("the desk", () => {
  it("separates what is mine from what my team has, and names the targets in trouble, what waits for my decision and the major incident", async () => {
    fakeServer(staff);
    const host = await draw(<Home />, "/", "/");
    await settle(300);
    const text = host.textContent ?? "";
    const queue = (name: string) => [...host.querySelectorAll("section.queue")].find((s) => s.querySelector("h2")?.textContent === name)?.textContent ?? "";
    expect(queue("Your incidents")).toContain("Mail is down");
    expect(queue("Your incidents")).not.toContain("Printer jam");
    expect(queue("Your team's incidents")).toContain("Printer jam");
    expect(queue("Targets breached or at risk")).toContain("Mail is down");
    expect(queue("Waiting for your decision")).toContain("Core switch");
    expect(queue("Waiting for your decision")).toContain("Patch");
    expect(queue("Your requests and tasks")).toContain("New laptop");
    expect(text).toContain("Major incident");
    expect(text).toContain("INC-3 Payments are failing");
    expect(text).toContain("next update was due");
    expect(host.querySelector('a[href="/major/i3"]')).toBeTruthy();
    expect(["/calendar", "/problems", "/catalog", "/items"].every((to) => host.querySelector(`nav a[href="${to}"]`))).toBe(true);
  });

  it("reports an incident from the desk, with the known problems that fit as the title is typed", async () => {
    const server = fakeServer([["POST", "/api/v1/Incident", { id: "i9", title: "Printer queue" }, 201], ...staff]);
    const host = await draw(<Home />, "/", "/");
    await settle(300);
    const user = userEvent.setup();
    await act(async () => user.type(host.querySelector<HTMLInputElement>("#itsm-quick-title")!, "Printer queue"));
    await until(() => (host.textContent ?? "").includes("Restart the spooler"));
    expect(server.seen.some((s) => s.line === "GET /api/app/itsm/suggest?q=Printer+queue")).toBe(true);
    expect(host.textContent).toContain("Restart the spooler");
    await act(async () => byText(host, "button", "Report it")?.click());
    await settle();
    expect(server.seen.find((s) => s.line === "POST /api/v1/Incident")?.body).toEqual({ title: "Printer queue", impact: "Medium", urgency: "Medium" });
  });

  it("says the server's refusal of a report as it is said", async () => {
    fakeServer([["POST", "/api/v1/Incident", { error: "validation", message: "Please check your input.", fields: [{ field: "title", message: "is too long" }] }, 422], ...staff]);
    const host = await draw(<Home />, "/", "/");
    await settle(300);
    await act(async () => userEvent.setup().type(host.querySelector<HTMLInputElement>("#itsm-quick-title")!, "x"));
    await act(async () => byText(host, "button", "Report it")?.click());
    await settle();
    expect([...host.querySelectorAll('[role="alert"]')].map((a) => a.textContent).join(" ")).toContain("is too long");
  });

  it("shows a person with no desk what is theirs, with no team queues and no quick report", async () => {
    fakeServer([
      meta({ Incident: { read: "own", write: "none", create: "all" }, CatalogRequest: { read: "own", write: "none", create: "all" } }),
      me(["IT Self Service"]),
      ["GET", "/api/v1/Incident", { rows: [incidents[1]] }],
      ["GET", "/api/v1/CatalogRequest", { rows: [] }],
    ]);
    const host = await draw(<Home />, "/", "/");
    await settle(300);
    const text = host.textContent ?? "";
    expect(text).toContain("Your incidents");
    expect(text).toContain("Printer jam");
    expect(text).not.toContain("Your team's incidents");
    expect(text).not.toContain("Report an incident");
    expect(host.querySelector('a[href="/e/Incident/new"]')).toBeTruthy();
    // Only the screens they may read are offered.
    expect(host.querySelector('nav a[href="/catalog"]')).toBeTruthy();
    expect(["/problems", "/items", "/calendar"].some((to) => host.querySelector(`nav a[href="${to}"]`))).toBe(false);
  });

  it("says it is not theirs to see when nothing may be read, and tries again when it failed", async () => {
    fakeServer([meta({}), me(["Requester"])]);
    const denied = await draw(<Home />, "/", "/");
    await settle(300);
    expect(denied.textContent).toContain("You don't have access");
    vi.unstubAllGlobals();
    fakeServer([meta({ Incident: all }), me(["Service Desk"]), ["GET", "/api/v1/Incident", { error: "server_error", message: "The database is busy." }, 500], ["GET", "/api/approvals", { error: "server_error", message: "The database is busy." }, 500]]);
    const failed = await draw(<Home />, "/", "/");
    await settle(300);
    expect(failed.textContent).toContain("The database is busy.");
    expect(byText(failed, "button", "Try again")).toBeTruthy();
  });
});
