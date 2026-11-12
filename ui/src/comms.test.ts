import { describe, expect, it } from "vitest";
import { isOpen, message, told, updatesOf } from "./comms";

describe("a major incident's message", () => {
  const incident = { id: "i", number: "INC-1", title: "Payments are down", service: "s1" };
  it("is told to the subscribers of the business service, and to no one when there is no service or none subscribes", () => {
    expect(told(incident, { id: "s1", subscribers: [{ subscriber: "ops@x.test", note: "on call" }, { subscriber: "" }] })).toEqual({ who: [{ address: "ops@x.test", note: "on call" }], why: null });
    expect(told({ id: "i" }, null).why).toBe("no_service");
    expect(told(incident, { id: "s1", subscribers: [] }).why).toBe("no_subscribers");
  });

  it("reads as the package words each message", () => {
    expect(message("declared", incident, { plan: "Hourly on chat" })).toEqual({ subject: "Major incident INC-1: Payments are down", text: "Major incident INC-1: Payments are down. Hourly on chat" });
    expect(message("update", incident, { update: "Fix is rolling out" })).toEqual({ subject: "Update on INC-1: Payments are down", text: "INC-1: Fix is rolling out" });
    expect(message("resolved", incident, { resolution: "Rolled back" }).subject).toBe("Resolved: INC-1 Payments are down");
  });

  it("lists the updates the caller sees, oldest first, and knows when it is over", () => {
    const notes = [{ id: "b", visibility: "Visible to Caller", posted_at: "2026-11-04T10:00:00Z" }, { id: "x", visibility: "Internal", posted_at: "2026-11-04T09:00:00Z" }, { id: "a", visibility: "Visible to Caller", posted_at: "2026-11-04T08:00:00Z" }];
    expect(updatesOf(notes).map((n) => n.id)).toEqual(["a", "b"]);
    expect([isOpen({ id: "i", status: "In Progress" }), isOpen({ id: "i", status: "Resolved" })]).toEqual([true, false]);
  });
});
