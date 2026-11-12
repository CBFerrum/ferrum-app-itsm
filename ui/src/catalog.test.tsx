// @vitest-environment jsdom
// The catalog as a requester meets it (a person who reads only the offerings and their own requests), an item's form made of its
// questions, the server's refusals of a request, and a request with its approval and its tasks: done by those who may, and left
// unsaid-but-explained to a portal person who may not read them.

import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { act } from "react";
import { CatalogRoute, OfferingRoute, RequestRoute } from "./Catalog";
import { byText, draw, fakeServer, until } from "./testkit";

afterEach(() => {
  vi.unstubAllGlobals();
  document.body.innerHTML = "";
});

const perms = (p: Record<string, Record<string, string>>) => ["GET", "/api/meta", { entities: Object.entries(p).map(([name, permissions]) => ({ name, permissions })) }] as [string, string, unknown];
const requester = perms({ ServiceOffering: { read: "all", write: "none", create: "none" }, CatalogRequest: { read: "own", write: "none", create: "all" } });
const laptop = {
  id: "o1",
  offering_name: "New laptop",
  description: "A machine for work",
  needs_approval: true,
  fulfil_hours: 24,
  approvals_needed: 2,
  questions: [
    { label: "Which laptop?", qtype: "Choice", required: true, options: "Standard, Developer" },
    { label: "Why a developer laptop?", qtype: "Text", required: true, only_when: "Which laptop?", only_when_answer: "Developer" },
  ],
  steps: [{ step_name: "Image it", sequence: 10, target_hours: 4 }, { step_name: "Ship it", sequence: 20, target_hours: 24 }],
};
const vpn = { id: "o2", offering_name: "VPN access", description: "Remote work", needs_approval: false, fulfil_hours: 8 };

describe("the catalog", () => {
  it("shows a requester the items, narrows them by what is typed with the known problems that fit, and their own requests", async () => {
    fakeServer([
      requester,
      ["GET", "/api/app/itsm/suggest", { suggestions: [{ id: "k", title: "VPN drops often", workaround: "Reconnect" }] }],
      ["GET", "/api/v1/ServiceOffering", { rows: [laptop, vpn, { id: "o3", offering_name: "Old thing", active: false }] }],
      ["GET", "/api/v1/CatalogRequest", { rows: [{ id: "r1", number: "REQ-1", status: "Submitted", _titles: { offering: "New laptop" } }] }],
    ]);
    const host = await draw(<CatalogRoute />, "/catalog", "/catalog");
    await until(() => (host.textContent ?? "").includes("VPN access"));
    expect(host.textContent).toContain("New laptop");
    expect(host.textContent).toContain("Needs approval");
    expect(host.textContent).not.toContain("Old thing");
    expect(host.textContent).toContain("Done within 24 working hours");
    expect(host.querySelector('a[href="/request/r1"]')).toBeTruthy();
    await act(async () => userEvent.setup().type(host.querySelector<HTMLInputElement>("#itsm-cat-q")!, "vpn"));
    await until(() => (host.textContent ?? "").includes("Reconnect"));
    expect(host.textContent).toContain("VPN drops often");
    expect(host.textContent).not.toContain("A machine for work");
  });

  it("says nothing matches a search, and that an empty catalog is empty", async () => {
    fakeServer([requester, ["GET", "/api/v1/ServiceOffering", { rows: [vpn] }], ["GET", "/api/v1/CatalogRequest", { rows: [] }], ["GET", "/api/app/itsm/suggest", { suggestions: [] }]]);
    const host = await draw(<CatalogRoute />, "/catalog", "/catalog");
    await until(() => (host.textContent ?? "").includes("VPN access"));
    await act(async () => userEvent.setup().type(host.querySelector<HTMLInputElement>("#itsm-cat-q")!, "zebra"));
    await until(() => (host.textContent ?? "").includes("Nothing in the catalog matches"));
    vi.unstubAllGlobals();
    fakeServer([requester, ["GET", "/api/v1/ServiceOffering", { rows: [] }], ["GET", "/api/v1/CatalogRequest", { rows: [] }]]);
    const empty = await draw(<CatalogRoute />, "/catalog", "/catalog");
    await until(() => (empty.textContent ?? "").includes("The catalog is empty"));
  });

  it("says a refused read is refused, with no way to retry", async () => {
    fakeServer([requester, ["GET", "/api/v1/ServiceOffering", { error: "forbidden", message: "no" }, 403], ["GET", "/api/v1/CatalogRequest", { error: "forbidden", message: "no" }, 403]]);
    const host = await draw(<CatalogRoute />, "/catalog", "/catalog");
    await until(() => (host.textContent ?? "").includes("You don't have access"));
    expect(byText(host, "button", "Try again")).toBeUndefined();
  });
});

describe("an item's form", () => {
  const routes: [string, string, unknown, number?][] = [requester, ["GET", "/api/v1/ServiceOffering/o1", laptop], ["GET", "/api/app/itsm/suggest", { suggestions: [] }]];

  it("asks a conditional question only when its answer is given, and holds the required ones before anything is sent", async () => {
    const server = fakeServer(routes);
    const host = await draw(<OfferingRoute />, "/catalog/:id", "/catalog/o1");
    await until(() => !!host.querySelector("#itsm-of-0"));
    expect(host.textContent).toContain("Image it");
    expect(host.textContent).not.toContain("Why a developer laptop?");
    expect(host.textContent).toContain("It is approved first");
    await act(async () => byText(host, "button", "Ask for it")?.click());
    await until(() => (host.textContent ?? "").includes("Which laptop? needs an answer"));
    expect(server.seen.some((s) => s.line.startsWith("POST"))).toBe(false);
    // The refusal is listed at the top and the cursor is in the first field in error.
    expect(host.textContent).toContain("Not saved. These need attention:");
    expect(document.activeElement?.id).toBe("itsm-of-0");
    const user = userEvent.setup();
    await act(async () => user.selectOptions(host.querySelector<HTMLSelectElement>("#itsm-of-0")!, "Developer"));
    await until(() => (host.textContent ?? "").includes("Why a developer laptop?"));
  });

  it("sends the answers asked and shows the server's refusal as it words it", async () => {
    const server = fakeServer([["POST", "/api/v1/CatalogRequest", { error: "validation", message: "Please check your input.", fields: [{ field: "offering", message: "is not offered any more" }] }, 422], ...routes]);
    const host = await draw(<OfferingRoute />, "/catalog/:id", "/catalog/o1");
    await until(() => !!host.querySelector("#itsm-of-0"));
    await act(async () => userEvent.setup().selectOptions(host.querySelector<HTMLSelectElement>("#itsm-of-0")!, "Standard"));
    await act(async () => byText(host, "button", "Ask for it")?.click());
    await until(() => (host.textContent ?? "").includes("is not offered any more"));
    expect(server.seen.find((s) => s.line === "POST /api/v1/CatalogRequest")?.body).toEqual({ offering: "o1", answers: [{ question: "Which laptop?", answer: "Standard" }] });
  });
});

describe("a request", () => {
  const request = { id: "r1", version: 1, number: "REQ-1", status: "Approved", offering: "o1", route: "Group", quorum: 2, requested_by: "ana@x.test", fulfil_state: "running", due_at: "2026-12-05T10:00:00Z", answers: [{ question: "Which laptop?", answer: "Standard" }], _titles: { offering: "New laptop" } };
  const tasks = [
    { id: "k1", step_name: "Image it", sequence: 10, status: "Open", task_state: "running", due_by: "2026-12-02T10:00:00Z", _titles: { assignment_group: "Desktop team" } },
    { id: "k2", step_name: "Ship it", sequence: 20, status: "Waiting", _titles: {} },
  ];
  const staff = perms({ CatalogRequest: { read: "all", write: "all", create: "all" }, RequestTask: { read: "all", write: "all", create: "all" } });

  it("shows its answers, its approval and its tasks, and a task is done by pressing Done", async () => {
    const server = fakeServer([
      staff,
      ["POST", "/api/v1/RequestTask/k1/actions/complete_task", { status: "Done" }],
      ["GET", "/api/v1/RequestTask", { rows: tasks }],
      ["GET", "/api/v1/CatalogRequest/r1/approval", { id: "ap", entity: "CatalogRequest", record: "r1", action: "approve_request", requester: "u1", requested_at: "2026-12-01T09:00:00Z", status: "approved", steps: [{ label: "Approver group (2 of its members)", state: "approved", need: 2, approvers: ["u2", "u3"] }], decisions: [{ step: 0, decider: "u2", outcome: "approve", at: "2026-12-01T10:00:00Z" }, { step: 0, decider: "u3", outcome: "approve", at: "2026-12-01T11:00:00Z" }] }],
      ["GET", "/api/approvals", { to_decide: [], awaiting_others: [] }],
      ["GET", "/api/meta/User", { name: "User", title_field: "full_name", fields: [], permissions: { read: "all" } }],
      ["GET", "/api/v1/User/", { id: "u", full_name: "Someone" }],
      ["GET", "/api/v1/ServiceOffering/o1", laptop],
      ["GET", "/api/v1/CatalogRequest/r1", request],
    ]);
    const host = await draw(<RequestRoute />, "/request/:id", "/request/r1");
    await until(() => !!byText(host, "button", "Done"));
    const text = host.textContent ?? "";
    expect(text).toContain("Which laptop?");
    expect(text).toContain("Standard");
    expect(text).toContain("Desktop team");
    expect(text).toContain("waits for the step before");
    expect(text).toContain("2 of 2 needed");
    await act(async () => byText(host, "button", "Done")?.click());
    await until(() => server.seen.some((s) => s.line.endsWith("complete_task")));
  });

  it("tells a portal person the tasks are IT's, and shows the rest", async () => {
    fakeServer([
      perms({ CatalogRequest: { read: "own", write: "none", create: "all" } }),
      ["GET", "/api/v1/RequestTask", { error: "forbidden", message: "no" }, 403],
      ["GET", "/api/v1/CatalogRequest/r1/approval", { error: "forbidden", message: "no" }, 403],
      ["GET", "/api/v1/ServiceOffering/o1", laptop],
      ["GET", "/api/v1/CatalogRequest/r1", { ...request, status: "Submitted" }],
    ]);
    const host = await draw(<RequestRoute />, "/request/:id", "/request/r1");
    await until(() => (host.textContent ?? "").includes("The tasks are IT's to see"));
    expect(host.textContent).toContain("It waits for 2 of the approving group.");
    expect(byText(host, "button", "Approve")).toBeUndefined();
    expect(byText(host, "button", "Cancel it")).toBeUndefined();
  });

  it("asks a Change Manager to approve one-by-one, and a rejection says why", async () => {
    const server = fakeServer([
      staff,
      ["GET", "/api/v1/RequestTask", { rows: [] }],
      ["GET", "/api/v1/ServiceOffering/o1", laptop],
      ["GET", "/api/v1/CatalogRequest/r1", { ...request, status: "Submitted", route: "Manager" }],
      ["POST", "/api/v1/CatalogRequest/r1/actions/reject_request", { error: "forbidden", message: "a request is decided by someone other than who asked" }, 403],
    ]);
    const host = await draw(<RequestRoute />, "/request/:id", "/request/r1");
    await until(() => !!byText(host, "button", "Reject"));
    expect(host.textContent).toContain("It waits for a Change Manager other than who asked.");
    await act(async () => byText(host, "button", "Reject")?.click());
    await until(() => !!byText(host, "button", "Reject it"));
    await act(async () => byText(host, "button", "Reject it")?.click());
    await until(() => (host.textContent ?? "").includes("is needed"));
    expect(server.seen.some((s) => s.line.endsWith("reject_request"))).toBe(false);
    await act(async () => userEvent.setup().type(host.querySelector<HTMLTextAreaElement>("#ask-note")!, "Not this quarter"));
    await act(async () => byText(host, "button", "Reject it")?.click());
    await until(() => (host.textContent ?? "").includes("someone other than who asked"));
    expect(server.seen.find((s) => s.line.endsWith("reject_request"))?.body).toEqual({ note: "Not this quarter" });
  });
});
