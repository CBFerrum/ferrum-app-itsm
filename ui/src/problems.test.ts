import { describe, expect, it } from "vitest";
import { bodyOfProblem, draftOf, offersResolve, openByProblem, showOf, stillOpen } from "./problems";

describe("problems", () => {
  it("reads the tab from the address, and counts open incidents by problem", () => {
    expect(["known", "resolved", "open", "x", null].map(showOf)).toEqual(["known", "resolved", "open", "open", "open"]);
    expect([...openByProblem([{ id: "a", problem: "p1" }, { id: "b", problem: "p1" }, { id: "c", problem: null }, { id: "d", problem: "p2" }])]).toEqual([["p1", 2], ["p2", 1]]);
  });

  it("offers to resolve the incidents of a resolved problem that still has open ones, and not those waiting for the caller", () => {
    const incidents = [{ id: "a", status: "In Progress" }, { id: "b", status: "Awaiting Caller" }, { id: "c", status: "Resolved" }];
    expect(stillOpen(incidents).map((i) => i.id)).toEqual(["a"]);
    expect(offersResolve({ id: "p", status: "Resolved" }, incidents)).toBe(true);
    expect(offersResolve({ id: "p", status: "Open" }, incidents)).toBe(false);
    expect(offersResolve({ id: "p", status: "Closed" }, [{ id: "c", status: "Resolved" }])).toBe(false);
  });

  it("is saved with its words trimmed and its boxes as they are", () => {
    const d = { ...draftOf({ id: "p", title: " VPN drops ", status: "Known Error", known_error: true, workaround: "Reconnect", workaround_visible: true }), root_cause: "  " };
    expect(bodyOfProblem(d)).toEqual({ title: "VPN drops", symptoms: null, status: "Known Error", known_error: true, workaround: "Reconnect", workaround_visible: true, root_cause: null });
  });
});
