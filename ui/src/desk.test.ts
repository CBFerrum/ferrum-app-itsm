import { describe, expect, it } from "vitest";
import { byPriority } from "./Home";

describe("the desk", () => {
  it("puts P1 first, then the soonest due", () => {
    const l = byPriority([
      { id: "c", priority: "P3", resolve_by: "2026-11-05T10:00:00Z" },
      { id: "b", priority: "P1", resolve_by: "2026-11-04T12:00:00Z" },
      { id: "a", priority: "P1", resolve_by: "2026-11-04T09:00:00Z" },
    ]);
    expect(l.map((x) => x.id)).toEqual(["a", "b", "c"]);
  });
});
