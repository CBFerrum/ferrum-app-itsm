import { describe, expect, it } from "vitest";
import { childrenOf, flowsOf, itemOf, rings, typesOf, walk, type Item, type Source } from "./impact";

const types = typesOf([
  { id: "dep", relation_name: "Depends On", inverse_name: "Is Depended On By", impact_flows: "To the Item" },
  { id: "runs", relation_name: "Runs On", inverse_name: "Hosts", impact_flows: "To the Item" },
  { id: "part", relation_name: "Part Of", inverse_name: "Contains", impact_flows: "To the Target" },
  { id: "conn", relation_name: "Connects To", inverse_name: "Connects To", impact_flows: "None" },
]);

const item = (id: string, relations: Item["relations"] = []): Item => ({ id, name: id.toUpperCase(), criticality: "Medium", status: "Live", relations });
// srv hosts db; app runs on db; shop depends on app; shop is part of store; web connects to app (no flow); cache runs on srv.
const all: Record<string, Item> = {
  srv: item("srv"),
  db: item("db", [{ type: "runs", kind: "Runs On", target: "srv" }]),
  app: item("app", [{ type: "dep", kind: "Depends On", target: "db" }]),
  shop: item("shop", [{ type: "dep", kind: "Depends On", target: "app" }, { type: "part", kind: "", target: "store" }]),
  store: item("store"),
  web: item("web", [{ type: "conn", kind: "Connects To", target: "app" }]),
  cache: item("cache", [{ type: "", kind: "Runs On", target: "srv" }]),
};
const source = (calls: { n: number } = { n: 0 }): Source => ({
  item: async (id) => (calls.n++, all[id]),
  dependents: async (id) => Object.values(all).filter((i) => i.relations.some((r) => r.target === id)).map((i) => i.id),
});

describe("the impact walk", () => {
  it("follows what depends on an item and what it is a part of, ring by ring, and leaves out a relation that carries no failure", async () => {
    const { nodes } = await walk("srv", 10, source(), types);
    expect(nodes.map((n) => [n.item.id, n.depth])).toEqual([["db", 1], ["cache", 1], ["app", 2], ["shop", 3], ["store", 4]]);
    expect(nodes.find((n) => n.item.id === "app")?.edge).toEqual({ from: "APP", relation: "Depends On", to: "DB" });
    expect(nodes.find((n) => n.item.id === "store")?.edge).toEqual({ from: "SHOP", relation: "Part Of", to: "STORE" });
    expect(nodes.some((n) => n.item.id === "web")).toBe(false);
    expect(rings(nodes)).toEqual([2, 1, 1, 1]);
    expect(childrenOf(nodes, "db").map((n) => n.item.id)).toEqual(["app"]);
  });

  it("stops at the depth the settings allow, and at a cap with it said", async () => {
    expect((await walk("srv", 2, source(), types)).nodes.map((n) => n.item.id)).toEqual(["db", "cache", "app"]);
    const capped = await walk("srv", 10, source(), types, 2);
    expect(capped.nodes).toHaveLength(2);
    expect(capped.cut).toBe(true);
    expect((await walk("srv", 10, source(), types)).cut).toBe(false);
  });

  it("never reaches an item twice, even where relations come round", async () => {
    const loop: Record<string, Item> = { ...all, srv: item("srv", [{ type: "dep", kind: "", target: "shop" }]) };
    const src: Source = { item: async (id) => loop[id], dependents: async (id) => Object.values(loop).filter((i) => i.relations.some((r) => r.target === id)).map((i) => i.id) };
    const { nodes } = await walk("srv", 10, src, types);
    expect(new Set(nodes.map((n) => n.item.id)).size).toBe(nodes.length);
    expect(nodes.some((n) => n.item.id === "srv")).toBe(false);
  });

  it("reads a relation with no type by its kind", () => {
    expect(flowsOf({ type: "", kind: "Runs On", target: "x" }, types)).toEqual({ flows: "To the Item", name: "Runs On" });
    expect(flowsOf({ type: "", kind: "Connects To", target: "x" }, types).flows).toBe("None");
    expect(itemOf({ id: "a", ci_name: "A", relations: [{ relation_type: "dep", kind: "Depends On", target: "b" }] }).relations).toEqual([{ type: "dep", kind: "Depends On", target: "b" }]);
  });
});
