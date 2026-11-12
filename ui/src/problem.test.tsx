// @vitest-environment jsdom
// Problems and major incidents as a person at the desk meets them: the list by what a problem is, a problem with its root cause, its
// published workaround and its incidents (resolved with it on a press), the server's refusal of a known error with no workaround
// shown on that field, and a major incident declared with who will be told and the message as it will read.

import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { act } from "react";
import { MajorRoute } from "./Major";
import { ProblemRoute, ProblemsRoute } from "./Problem";
import { byText, draw, fakeServer, settle, until } from "./testkit";

afterEach(() => {
  vi.unstubAllGlobals();
  document.body.innerHTML = "";
});

const meta = ["GET", "/api/meta", { entities: ["Incident", "Problem"].map((name) => ({ name, permissions: { read: "all", write: "all", create: "all" } })) }] as [string, string, unknown];
const problem = { id: "p1", version: 2, number: "PRB-1", title: "VPN drops", status: "Resolved", known_error: true, workaround: "Reconnect", workaround_visible: true, root_cause: "A bad firmware", symptoms: "VPN disconnects", _titles: { ci: "Gateway" } };
const incidents = [
  { id: "i1", number: "INC-1", title: "Cannot connect", status: "In Progress", priority: "P2" },
  { id: "i2", number: "INC-2", title: "VPN slow", status: "Resolved", priority: "P3" },
];

describe("the problems", () => {
  it("lists them by what they are, with their open incidents counted and the major incidents running", async () => {
    const server = fakeServer([
      ["GET", "/api/v1/Incident", (u: URL) => ({ rows: u.searchParams.get("fields") ? [{ id: "i1", problem: "p1" }, { id: "i3", problem: "p1" }] : [{ id: "m1", number: "INC-9", title: "Payments down", priority: "P1", major: true }] })],
      ["GET", "/api/v1/Problem", { rows: [problem, { id: "p2", number: "PRB-2", title: "Slow disk", status: "Open", _titles: {} }] }],
    ]);
    const host = await draw(<ProblemsRoute />, "/problems", "/problems");
    await until(() => (host.textContent ?? "").includes("PRB-2"));
    const text = host.textContent ?? "";
    expect(text).toContain("Major incidents now");
    expect(text).toContain("Payments down");
    expect(text).toContain("PRB-1 VPN drops");
    expect(text).toContain("Known error");
    expect([...host.querySelectorAll("tbody tr")].map((r) => r.querySelector("td.num")?.textContent)).toEqual(["2", "0"]);
    expect(server.seen.some((s) => s.line.includes("known_error"))).toBe(false);
    await act(async () => byText(host, "button", "Known errors")?.click());
    await settle();
    expect(decodeURIComponent(server.seen.at(-3)?.line ?? "") + decodeURIComponent(server.seen.at(-2)?.line ?? "") + decodeURIComponent(server.seen.at(-1)?.line ?? "")).toContain("known_error");
  });

  it("says why when it cannot be read", async () => {
    fakeServer([["GET", "/api/v1/Problem", { error: "server_error", message: "The database is busy." }, 500], ["GET", "/api/v1/Incident", { rows: [] }]]);
    const host = await draw(<ProblemsRoute />, "/problems", "/problems");
    await until(() => (host.textContent ?? "").includes("The database is busy."));
    expect(byText(host, "button", "Try again")).toBeTruthy();
  });
});

describe("a problem", () => {
  const routes = (extra: [string, string, unknown, number?][] = []) =>
    [
      ...extra,
      meta,
      ["GET", "/api/v1/ItsmSettings", { rows: [{ id: "s", problem_closes_incidents: "Offer" }] }],
      ["GET", "/api/v1/KnownError", { rows: [{ id: "k1", problem: "p1", title: "VPN drops", symptoms: "VPN disconnects", workaround: "Reconnect", published: true }] }],
      ["GET", "/api/v1/Incident", { rows: incidents }],
      ["GET", "/api/v1/Problem/p1", problem],
    ] as [string, string, unknown, number?][];

  it("shows the cause, the workaround the portal will show, and the incidents, and resolves the open ones on a press", async () => {
    const server = fakeServer(routes([["POST", "/api/v1/Problem/p1/actions/resolve_linked_incidents", { status: "Resolved" }]]));
    const host = await draw(<ProblemRoute />, "/problem/:id", "/problem/p1");
    await until(() => !!byText(host, "button", "Resolve its incidents"));
    const text = host.textContent ?? "";
    expect(host.querySelector<HTMLTextAreaElement>("#itsm-pb-cause")?.value).toBe("A bad firmware");
    expect(text).toContain("Published");
    expect(text).toContain("people reporting an incident or asking from the catalog are shown it");
    expect(text).toContain("INC-1 Cannot connect");
    expect(text).toContain("1 still open");
    await act(async () => byText(host, "button", "Resolve its incidents")?.click());
    await until(() => !!byText(host, "button", "Resolve them"));
    expect(host.textContent).toContain("The 1 incidents still open under it are resolved with its root cause: A bad firmware");
    await act(async () => byText(host, "button", "Resolve them")?.click());
    await until(() => server.seen.some((s) => s.line.endsWith("resolve_linked_incidents")));
    expect(server.seen.some((s) => s.line === "POST /api/v1/Problem/p1/actions/resolve_linked_incidents")).toBe(true);
  });

  it("shows the server's refusal of a known error with no workaround on the workaround", async () => {
    fakeServer(routes([["PATCH", "/api/v1/Problem/p1", { error: "validation", message: "Please check your input.", fields: [{ field: "workaround", message: "a known error has its workaround" }] }, 422]]));
    const host = await draw(<ProblemRoute />, "/problem/:id", "/problem/p1");
    await until(() => !!host.querySelector("#itsm-pb-work"));
    await act(async () => userEvent.setup().clear(host.querySelector<HTMLTextAreaElement>("#itsm-pb-work")!));
    await act(async () => byText(host, "button", "Save")?.click());
    await until(() => (host.textContent ?? "").includes("a known error has its workaround"));
    expect(host.querySelector("#itsm-pb-work-error")?.textContent).toBe("a known error has its workaround");
    expect(host.textContent).toContain("Not saved. These need attention:");
    expect(document.activeElement?.id).toBe("itsm-pb-work");
  });
});

describe("a major incident", () => {
  const incident = { id: "i1", number: "INC-1", title: "Payments are down", status: "In Progress", priority: "P1", major: true, service: "s1", _titles: { service: "Payments" } };
  const routes = (inc: Record<string, unknown>, subscribers: unknown[]): [string, string, unknown, number?][] => [
    ["GET", "/api/auth/me", { tenant: "t", user: { id: "u1", email: "ana@x.test", full_name: "Ana", is_admin: false, roles: ["Service Desk"] } }],
    ["GET", "/api/meta", { entities: [{ name: "Incident", permissions: { read: "all", write: "all", create: "all" } }] }],
    ["GET", "/api/v1/ItNote", { rows: [{ id: "n1", body: "Fix is rolling out", visibility: "Visible to Caller", author: "ana@x.test", posted_at: "2026-11-04T09:00:00Z" }, { id: "n2", body: "Internal chatter", visibility: "Internal", posted_at: "2026-11-04T09:30:00Z" }] }],
    ["GET", "/api/v1/ConfigurationItem/s1", { id: "s1", ci_name: "Payments", subscribers }],
    ["GET", "/api/v1/Incident/i1", inc],
  ];

  it("shows who will be told and the message as it will read before it is declared, and the server's refusal in the dialog", async () => {
    const server = fakeServer([["POST", "/api/v1/Incident/i1/actions/declare_major", { error: "forbidden", message: "only the Service Desk or a Change Manager declares" }, 403], ...routes(incident, [{ subscriber: "ops@x.test", note: "on call" }, { subscriber: "cfo@x.test" }])]);
    const host = await draw(<MajorRoute />, "/major/:id", "/major/i1");
    await until(() => !!byText(host, "button", "Declare a major incident"));
    expect(host.textContent).toContain("Major by priority, not yet declared");
    expect(host.textContent).toContain("2 people will be told");
    await act(async () => byText(host, "button", "Declare a major incident")?.click());
    await settle();
    await act(async () => userEvent.setup().type(host.querySelector<HTMLTextAreaElement>("#ask-plan")!, "Hourly on chat"));
    await until(() => (host.textContent ?? "").includes("Major incident INC-1: Payments are down. Hourly on chat"));
    expect(host.textContent).toContain("ops@x.test");
    await act(async () => byText(host, "button", "Declare it")?.click());
    await settle();
    expect(server.seen.find((s) => s.line.endsWith("declare_major"))?.body).toEqual({ plan: "Hourly on chat" });
    expect(host.textContent).toContain("only the Service Desk or a Change Manager declares");
  });

  it("asks for a plan before anything is sent, and warns that a service with no subscribers tells no one", async () => {
    const server = fakeServer(routes(incident, []));
    const host = await draw(<MajorRoute />, "/major/:id", "/major/i1");
    await until(() => !!byText(host, "button", "Declare a major incident"));
    await act(async () => byText(host, "button", "Declare a major incident")?.click());
    await settle();
    expect(host.textContent).toContain("the service has no subscribers");
    await act(async () => byText(host, "button", "Declare it")?.click());
    await settle();
    expect(host.textContent).toContain("is needed");
    expect(document.activeElement?.id).toBe("ask-plan");
    expect(host.textContent).toContain("Not saved. These need attention:");
    expect(server.seen.some((s) => s.line.endsWith("declare_major"))).toBe(false);
  });

  it("shows the updates the caller sees and an overdue next update once it is declared", async () => {
    fakeServer(routes({ ...incident, major_declared: true, comms_plan: "Hourly", next_update_by: "2020-01-01T00:00:00Z" }, [{ subscriber: "ops@x.test" }]));
    const host = await draw(<MajorRoute />, "/major/:id", "/major/i1");
    await until(() => !!byText(host, "button", "Send an update"));
    const text = host.textContent ?? "";
    expect(text).toContain("Fix is rolling out");
    expect(text).not.toContain("Internal chatter");
    expect(text).toContain("The next update was due");
    expect(byText(host, "button", "Resolve it")).toBeTruthy();
  });
});
