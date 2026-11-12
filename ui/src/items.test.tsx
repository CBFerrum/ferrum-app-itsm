// @vitest-environment jsdom
// Configuration items as a person at the desk meets them: the list, the classes with their attributes, the relation types, and an
// item with its attributes and relations, the tree of what a failure of it reaches, and the incidents and changes on it, with what
// a role may not read said to be so.

import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { act } from "react";
import { ItemRoute, ItemsRoute } from "./Items";
import { byText, draw, fakeServer, until } from "./testkit";

afterEach(() => {
  vi.unstubAllGlobals();
  document.body.innerHTML = "";
});

const perms = ["GET", "/api/meta", { entities: [{ name: "ConfigurationItem", permissions: { read: "all", write: "all", create: "all" } }] }] as [string, string, unknown];
const relationTypes = {
  rows: [
    { id: "runs", relation_name: "Runs On", inverse_name: "Hosts", impact_flows: "To the Item" },
    { id: "dep", relation_name: "Depends On", inverse_name: "Is Depended On By", impact_flows: "To the Item" },
    { id: "conn", relation_name: "Connects To", inverse_name: "Connects To", impact_flows: "None" },
  ],
};
const graph: Record<string, Record<string, unknown>> = {
  s1: { id: "s1", ci_name: "Server 1", status: "Live", environment: "Production", criticality: "High", ci_class: "cl1", owner: "ops@x.test", attributes: { firmware: "2.1" }, relations: [{ relation_type: "conn", kind: "Connects To", target: "sw", _titles: { target: "Switch" } }], subscribers: [{ subscriber: "noc@x.test" }], _titles: { ci_class: "Server" } },
  app: { id: "app", ci_name: "Orders", criticality: "Critical", relations: [{ relation_type: "runs", kind: "Runs On", target: "s1" }] },
  shop: { id: "shop", ci_name: "Shop", criticality: "Medium", relations: [{ relation_type: "dep", kind: "Depends On", target: "app" }] },
  sw: { id: "sw", ci_name: "Switch", criticality: "Low", relations: [] },
};
const itemApi = (u: URL) => {
  const id = u.pathname.split("/")[4];
  if (id) return graph[id];
  const filters = JSON.parse(u.searchParams.get("filters") ?? "[]") as [string, string, string][];
  const target = filters.find((f) => f[0] === "relations.target")?.[2];
  return { rows: Object.values(graph).filter((i) => (i.relations as { target: string }[]).some((r) => r.target === target)).map((i) => ({ id: i.id, ci_name: i.ci_name })) };
};

describe("an item", () => {
  const base: [string, string, unknown, number?][] = [
    perms,
    ["GET", "/api/v1/ItsmSettings", { rows: [{ id: "s", impact_depth: 3 }] }],
    ["GET", "/api/v1/RelationType", relationTypes],
    ["GET", "/api/v1/CiClass/cl1", { id: "cl1", class_name: "Server", attributes: [{ attr_name: "firmware", attr_type: "Text" }, { attr_name: "rack", attr_type: "Text" }] }],
    ["GET", "/api/v1/ConfigurationItem", itemApi],
  ];

  it("shows its attributes and relations, the tree of what a failure of it reaches, and the incidents on it", async () => {
    fakeServer([
      ["GET", "/api/v1/Incident", (u: URL) => ({ rows: u.searchParams.get("filters")?.includes('"ci"') ? [{ id: "i1", number: "INC-1", title: "Disk full", status: "In Progress" }] : [] })],
      ["GET", "/api/v1/ItChange", { error: "forbidden", message: "no" }, 403],
      ...base,
    ]);
    const host = await draw(<ItemRoute />, "/ci/:id", "/ci/s1");
    await until(() => (host.textContent ?? "").includes("Disk full") && (host.textContent ?? "").includes("These are not shown to you."));
    const text = host.textContent ?? "";
    expect(text).toContain("Server 1");
    expect(text).toContain("firmware");
    expect(text).toContain("2.1");
    expect(text).toContain("Connects To");
    expect(text).toContain("No failure flows along it");
    expect(text).toContain("noc@x.test");
    // The tree: Orders runs on Server 1, and the Shop depends on Orders.
    const tree = host.querySelector(".walk")!;
    expect(tree.textContent).toContain("Orders");
    expect(tree.textContent).toContain("Orders Runs On Server 1");
    expect(tree.textContent).toContain("Shop Depends On Orders");
    expect(tree.querySelector("li li")?.textContent).toContain("Shop");
    expect(text).toContain("2 items, followed up to 3 relations out: 1 at 1, 1 at 2");
    expect(text).toContain("Hosts");
  });

  it("stops the tree at the depth the settings allow", async () => {
    fakeServer([["GET", "/api/v1/ItsmSettings", { rows: [{ id: "s", impact_depth: 1 }] }], ["GET", "/api/v1/Incident", { rows: [] }], ["GET", "/api/v1/ItChange", { rows: [] }], ...base.filter((r) => r[1] !== "/api/v1/ItsmSettings")]);
    const host = await draw(<ItemRoute />, "/ci/:id", "/ci/s1");
    await until(() => !!host.querySelector(".walk"));
    expect(host.querySelector(".walk")?.textContent).not.toContain("Shop");
  });

  it("says why when the item cannot be read, and that nothing depends on one that nothing reaches", async () => {
    fakeServer([["GET", "/api/v1/ConfigurationItem/s1", { error: "not_found", message: "Configuration item not found." }, 404]]);
    const gone = await draw(<ItemRoute />, "/ci/:id", "/ci/s1");
    await until(() => (gone.textContent ?? "").includes("Configuration item not found."));
    expect(byText(gone, "button", "Try again")).toBeTruthy();
    vi.unstubAllGlobals();
    fakeServer([["GET", "/api/v1/Incident", { rows: [] }], ["GET", "/api/v1/ItChange", { rows: [] }], ...base]);
    const alone = await draw(<ItemRoute />, "/ci/:id", "/ci/sw");
    await until(() => (alone.textContent ?? "").includes("Nothing depends on it"));
  });
});

describe("the items", () => {
  const list = { rows: [{ id: "s1", ci_name: "Server 1", ci_class: "cl1", status: "Live", environment: "Production", criticality: "High", owner: "ops@x.test", _titles: { ci_class: "Server" } }, { id: "sw", ci_name: "Switch", status: "Live", criticality: "Low", missing_since: "2026-11-01", _titles: {} }] };

  it("lists the items, finds one by what is typed, and marks one discovery no longer sees", async () => {
    fakeServer([perms, ["GET", "/api/v1/ConfigurationItem", list]]);
    const host = await draw(<ItemsRoute />, "/items", "/items");
    await until(() => (host.textContent ?? "").includes("Switch"));
    expect(host.textContent).toContain("Not seen since 2026-11-01");
    await act(async () => userEvent.setup().type(host.querySelector<HTMLInputElement>("#itsm-ci-q")!, "ops@"));
    await until(() => !(host.textContent ?? "").includes("Switch"));
    expect(host.textContent).toContain("Server 1");
    await act(async () => userEvent.setup().type(host.querySelector<HTMLInputElement>("#itsm-ci-q")!, "zzz"));
    await until(() => (host.textContent ?? "").includes("No item matches"));
  });

  it("shows the classes with the attributes they declare, and the relation types with where a failure flows", async () => {
    fakeServer([
      perms,
      ["GET", "/api/v1/ConfigurationItem", list],
      ["GET", "/api/v1/CiClass/cl1", { id: "cl1", class_name: "Server", attributes: [{ attr_name: "firmware", attr_type: "Choice", required: true, choices: "1.0, 2.0" }] }],
      ["GET", "/api/v1/CiClass", { rows: [{ id: "cl1", class_name: "Server" }] }],
      ["GET", "/api/v1/RelationType", relationTypes],
    ]);
    const classes = await draw(<ItemsRoute />, "/items", "/items?view=classes");
    await until(() => (classes.textContent ?? "").includes("firmware"));
    expect(classes.textContent).toContain("1.0, 2.0");
    expect(classes.textContent).toContain("1 items");
    const types = await draw(<ItemsRoute />, "/items", "/items?view=types");
    await until(() => (types.textContent ?? "").includes("Is Depended On By"));
    expect(types.textContent).toContain("What depends on or runs on the target is hit when it fails");
  });

  it("says a refused list is refused, and an empty one empty", async () => {
    fakeServer([perms, ["GET", "/api/v1/ConfigurationItem", { error: "forbidden", message: "no" }, 403]]);
    const denied = await draw(<ItemsRoute />, "/items", "/items");
    await until(() => (denied.textContent ?? "").includes("You don't have access"));
    vi.unstubAllGlobals();
    fakeServer([perms, ["GET", "/api/v1/ConfigurationItem", { rows: [] }]]);
    const empty = await draw(<ItemsRoute />, "/items", "/items");
    await until(() => (empty.textContent ?? "").includes("No configuration items yet"));
  });
});
