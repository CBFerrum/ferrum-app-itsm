// What a failure of a configuration item reaches (src/cmdb.rs, `impact`): the items that depend on it or run on it (a relation of a
// type whose failure flows To the Item, listed by the dependent and naming the failing item) and what it is a part of (its own
// relation of a type that flows To the Target), then the same from each of those, to the depth the settings allow and never
// twice. The server gives this only as the names it writes into a change; the tree is worked out here from the relations the
// items list, with the same rules. A relation with no type is read by its kind, as the server reads it.

import { type Row, text } from "./data";

export type Flows = "To the Item" | "To the Target" | "None";

export interface RelationType {
  id: string;
  name: string;
  inverse: string;
  flows: Flows;
}

export const typesOf = (list: Row[]): Map<string, RelationType> =>
  new Map(list.map((r) => [r.id, { id: r.id, name: text(r.relation_name), inverse: text(r.inverse_name), flows: (["To the Item", "To the Target"].includes(text(r.impact_flows)) ? text(r.impact_flows) : "None") as Flows }]));

export interface Relation {
  /** The relation's type's id, when it has one. */
  type: string;
  kind: string;
  target: string;
}

export interface Item {
  id: string;
  name: string;
  criticality: string;
  status: string;
  relations: Relation[];
}

/** An item record as the walk reads it. */
export const itemOf = (r: Row): Item => ({
  id: r.id,
  name: text(r.ci_name),
  criticality: text(r.criticality),
  status: text(r.status),
  relations: (Array.isArray(r.relations) ? (r.relations as Row[]) : []).map((x) => ({ type: text(x.relation_type), kind: text(x.kind), target: text(x.target) })),
});

/** Where a relation's failure flows, and the name it goes by. */
export function flowsOf(r: Relation, types: Map<string, RelationType>): { flows: Flows; name: string } {
  const t = r.type ? types.get(r.type) : undefined;
  if (t) return { flows: t.flows, name: t.name };
  return { flows: r.kind === "Depends On" || r.kind === "Runs On" ? "To the Item" : "None", name: r.kind };
}

export interface Node {
  item: Item;
  depth: number;
  /** The item it was reached from (none for the first ring). */
  from: string;
  /** "Orders Runs On Server 1": how the failure got here, read from the one that has the relation to the one it names. */
  edge: { from: string; relation: string; to: string };
}

export interface Source {
  item(id: string): Promise<Item>;
  /** The ids of the items that list a relation to this one. */
  dependents(id: string): Promise<string[]>;
}

/** Every item a failure of `root` reaches, breadth first, up to `depth` relations out and `cap` items; `cut` when the cap stopped it. */
export async function walk(root: string, depth: number, src: Source, types: Map<string, RelationType>, cap = 200): Promise<{ nodes: Node[]; cut: boolean; root: Item }> {
  const start = await src.item(root);
  const seen = new Set([root]);
  const nodes: Node[] = [];
  let frontier = [start];
  let cut = false;
  for (let level = 1; level <= depth && frontier.length && !cut; level++) {
    const next: Item[] = [];
    for (const x of frontier) {
      const found: { item: Item; edge: Node["edge"] }[] = [];
      // What depends on it, or runs on it.
      for (const id of await src.dependents(x.id)) {
        if (seen.has(id)) continue;
        const d = await src.item(id);
        const rel = d.relations.find((r) => r.target === x.id && flowsOf(r, types).flows === "To the Item");
        if (rel) found.push({ item: d, edge: { from: d.name, relation: flowsOf(rel, types).name, to: x.name } });
      }
      // What it is a part of.
      for (const r of x.relations) {
        const { flows, name } = flowsOf(r, types);
        if (flows !== "To the Target" || !r.target || seen.has(r.target)) continue;
        const up = await src.item(r.target);
        found.push({ item: up, edge: { from: x.name, relation: name, to: up.name } });
      }
      for (const f of found) {
        if (seen.has(f.item.id)) continue;
        if (nodes.length >= cap) {
          cut = true;
          break;
        }
        seen.add(f.item.id);
        nodes.push({ item: f.item, depth: level, from: x.id, edge: f.edge });
        next.push(f.item);
      }
      if (cut) break;
    }
    frontier = next;
  }
  return { nodes, cut, root: start };
}

/** The nodes as a tree: each item under the one it was reached from. */
export function childrenOf(nodes: Node[], parent: string): Node[] {
  return nodes.filter((n) => n.from === parent);
}

/** How many items each ring has, nearest first. */
export function rings(nodes: Node[]): number[] {
  const out: number[] = [];
  for (const n of nodes) out[n.depth - 1] = (out[n.depth - 1] ?? 0) + 1;
  return Array.from(out, (n) => n ?? 0);
}
