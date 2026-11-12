// @vitest-environment jsdom
// The change screens as a person meets them, over a fake server answering as the server's code and tests do: a draft with the
// risk it adds up to and the route that gives; the server's refusal of a window in a freeze, said as it words it; a change
// asked of the board, with who was asked, who answered, N of M and a stand-in; and an approver's Approve and Reject.

import { afterEach, describe, expect, it, vi } from "vitest";
import { act } from "react";
import { ChangeRoute } from "./Change";
import { byText, draw, fakeServer, settle } from "./testkit";

afterEach(() => {
  vi.unstubAllGlobals();
  document.body.innerHTML = "";
});

const users: Record<string, string> = { u1: "Ana Rao", u2: "Bo Lee", u3: "Cy Kim", u4: "Di Park" };
const common: [string, string, unknown, number?][] = [
  ["GET", "/api/meta/User", { name: "User", title_field: "full_name", fields: [], permissions: { read: "all" } }],
  ["GET", "/api/v1/User/", (u: URL) => ({ id: u.pathname.split("/").pop(), full_name: users[u.pathname.split("/").pop() ?? ""] })],
  ["GET", "/api/meta/ConfigurationItem", { name: "ConfigurationItem", title_field: "ci_name", fields: [], permissions: { read: "all" } }],
  ["GET", "/api/meta", { entities: [{ name: "ItChange", permissions: { read: "all", write: "all", create: "all" } }] }],
  ["GET", "/api/v1/ConfigurationItem/", { id: "c1", ci_name: "Orders DB" }],
  ["GET", "/api/v1/ItsmSettings", { rows: [{ id: "s", freeze_action: "Refuse", score_medium_from: 4, score_high_from: 8 }] }],
  ["GET", "/api/v1/ChangeType", { rows: [{ id: "t-normal", type_name: "Normal", path: "Board", quorum: 2, _titles: { board: "Change Board" } }, { id: "t-emerg", type_name: "Emergency", path: "Emergency" }] }],
  ["GET", "/api/v1/RiskQuestion/q1", { id: "q1", question_text: "Has this change been done before?", measures: "Risk", choices: [{ choice_label: "Many times", score: 0 }, { choice_label: "Never", score: 4 }] }],
  ["GET", "/api/v1/RiskQuestion", { rows: [{ id: "q1", question_text: "Has this change been done before?", measures: "Risk", position: 10 }] }],
  ["GET", "/api/v1/ChangeTemplate", { rows: [] }],
];

const draft = { id: "ch1", version: 3, number: "CHG-2026-00001", title: "Core switch", status: "Draft", ci: "c1", change_type: "t-normal", risk: "Medium", planned_start: "2026-12-28T10:00:00Z", planned_end: "2026-12-28T12:00:00Z", backout_plan: "Roll back", risk_answers: [{ question: "Has this change been done before?", answer: "Never", score: 4 }] };

describe("a change as a draft", () => {
  it("shows the risk the answers add up to, the route it gives and the collision with another change", async () => {
    fakeServer([
      ["POST", "/api/v1/ItChange/_calc", { ...draft, risk: "Medium", risk_score: 4, route: "Board", quorum: 2, board_members: ["a@x.test", "b@x.test", "c@x.test"], second_look: false, review_required: false, window_flags: "Same window, same service or item: CHG-2026-00002" }],
      ["GET", "/api/v1/ItChange/ch1", draft],
      ...common,
    ]);
    const host = await draw(<ChangeRoute />, "/change/:id", "/change/ch1");
    await settle(500);
    const text = host.textContent ?? "";
    expect(text).toContain("1. What changes");
    expect(text).toContain("Has this change been done before?");
    expect(text).toContain("Never");
    expect(text).toContain("score 4");
    expect(text).toContain("2 of 3 members of Change Board approve: a@x.test, b@x.test, c@x.test");
    expect(text).toContain("Same window, same service or item: CHG-2026-00002");
  });

  it("says a freeze refused the window in the server's words, on the window", async () => {
    const server = fakeServer([
      ["PATCH", "/api/v1/ItChange/ch1", { error: "validation", message: "Please check your input.", fields: [{ field: "planned_start", message: "falls in the freeze for Year end: only emergencies then" }] }, 422],
      ["POST", "/api/v1/ItChange/_calc", { ...draft, route: "Board", quorum: 2, board_members: [], window_flags: "In the freeze for Year end" }],
      ["GET", "/api/v1/ItChange/ch1", draft],
      ...common,
    ]);
    const host = await draw(<ChangeRoute />, "/change/:id", "/change/ch1");
    await settle(500);
    expect(host.textContent).toContain("saving will be refused");
    await act(async () => byText(host, "button", "Save draft")?.click());
    await settle();
    expect(server.seen.some((s) => s.line.startsWith("PATCH /api/v1/ItChange/ch1"))).toBe(true);
    expect(host.textContent).toContain("falls in the freeze for Year end: only emergencies then");
  });

  it("does not send a draft that lacks its item, and says what it lacks", async () => {
    const server = fakeServer([["GET", "/api/v1/ItChange/ch1", { ...draft, ci: null }], ...common]);
    const host = await draw(<ChangeRoute />, "/change/:id", "/change/ch1");
    await act(async () => byText(host, "button", "Save draft")?.click());
    await settle();
    expect(host.textContent).toContain("Name the item it changes");
    expect(server.seen.some((s) => s.line.startsWith("PATCH"))).toBe(false);
  });
});

const submitted = { ...draft, status: "Submitted", route: "Board", quorum: 2 };
const asked = {
  id: "ap1",
  entity: "ItChange",
  record: "ch1",
  action: "approve_change",
  requester: "u1",
  requested_at: "2026-12-01T09:00:00Z",
  status: "pending",
  steps: [{ label: "Change board (2 of its members)", state: "open", need: 2, approvers: ["u2", "u3", "u4"], remind_at: "2026-12-02T09:00:00Z" }],
  decisions: [{ step: 0, decider: "u3", on_behalf_of: "u4", outcome: "approve", comment: "Fine by me", at: "2026-12-01T10:00:00Z" }],
};

describe("a change asked of the board", () => {
  it("shows who was asked, who answered for whom, and N of M, and offers Approve and Reject to an approver", async () => {
    const server = fakeServer([
      ["POST", "/api/v1/ItChange/ch1/approval/decide", {}],
      ["GET", "/api/v1/ItChange/ch1/approval", asked],
      ["GET", "/api/approvals", { to_decide: [{ request: "ap1", entity: "ItChange", record: "ch1", title: "Core switch", action: "approve_change", requester: "u1", requested_at: "2026-12-01T09:00:00Z" }], awaiting_others: [] }],
      ["GET", "/api/v1/ItChange/ch1", submitted],
      ...common,
    ]);
    const host = await draw(<ChangeRoute />, "/change/:id", "/change/ch1");
    await settle(300);
    const text = host.textContent ?? "";
    expect(text).toContain("Waiting for approval");
    expect(text).toContain("1 of 2 needed, 3 asked");
    expect(text).toContain("Bo Lee, Cy Kim, Di Park");
    expect(text).toContain("Cy Kim for Di Park");
    expect(text).toContain("Fine by me");
    expect(byText(host, "button", "Approve")).toBeTruthy();
    // A rejection says why: without it nothing is sent.
    await act(async () => byText(host, "button", "Reject")?.click());
    await settle();
    expect(host.textContent).toContain("Say why when you reject");
    expect(server.seen.some((s) => s.line.includes("approval/decide"))).toBe(false);
  });

  it("offers no decision to someone it was not asked of, and does not show a decision that was cancelled", async () => {
    fakeServer([
      ["GET", "/api/v1/ItChange/ch1/approval", { ...asked, decisions: [{ ...asked.decisions[0], void: true }] }],
      ["GET", "/api/approvals", { to_decide: [], awaiting_others: [] }],
      ["GET", "/api/v1/ItChange/ch1", submitted],
      ...common,
    ]);
    const host = await draw(<ChangeRoute />, "/change/:id", "/change/ch1");
    await settle(300);
    expect(byText(host, "button", "Approve")).toBeUndefined();
    expect(host.textContent).toContain("0 of 2 needed, 3 asked");
    expect(host.textContent).toContain("1 earlier decisions were cancelled");
  });

  it("says why when the change cannot be read", async () => {
    fakeServer([["GET", "/api/v1/ItChange/ch1", { error: "forbidden", message: "no" }, 403], ...common]);
    const host = await draw(<ChangeRoute />, "/change/:id", "/change/ch1");
    await settle();
    expect(host.textContent).toContain("You don't have access");
  });
});
