// Configuration items: the items with their classes, the classes with the attributes they declare, the relation types and where a
// failure flows along each; and an item with its attributes, the relations it has and those others have to it, what a failure of
// it reaches (a tree by ring, worked out from the relations with the server's own rules, to the depth the settings allow), and the
// incidents and changes on it. What a role may not read is left out and said, not failed.

import { useQuery } from "@tanstack/react-query";
import { type ReactNode, useMemo, useState } from "react";
import { Link, useParams, useSearchParams } from "react-router";
import { isolate, t } from "@ferrum/client";
import { Alert, Badge, ButtonLink, Field, FormSkeleton, ListSkeleton, PageHead, PageSplit, PageStack, ScrollRegion, StatePanel, StatusBadge, TableSkeleton, type Tone } from "@ferrum/kit";
import { record, rows, type Row, text, titleOf } from "./data";
import { when } from "./desk";
import { childrenOf, flowsOf, itemOf, rings, typesOf, walk, type Flows, type Node, type Source } from "./impact";
import { Await, Nav, Refused, useRights, useSettings } from "./ui";

const denied = (e: unknown) => (e as { status?: number } | null)?.status === 403;
/** A read that a role may not make is `null`, and the page says so; any other failure is the page's to show. */
const maybe = <T,>(p: Promise<T>): Promise<T | null> => p.catch((e: unknown) => (denied(e) ? null : Promise.reject(e)));

const CRITICAL: Record<string, Tone> = { Critical: "red", High: "amber", Medium: "blue", Low: "neutral" };
const tone = (c: string): Tone => (Object.prototype.hasOwnProperty.call(CRITICAL, c) ? CRITICAL[c] : "neutral");

const FLOWS = (): Record<Flows, string> => ({
  "To the Item": t("What depends on or runs on the target is hit when it fails"),
  "To the Target": t("A part: the thing it is part of fails with it"),
  None: t("No failure flows along it"),
});

// ── the lists ──

function ClassesView({ items }: { items: Row[] }) {
  const q = useQuery({
    queryKey: ["itsm", "classes"],
    // A list has no table rows: each class is read for the attributes it declares.
    queryFn: async () => Promise.all((await rows("CiClass", [], { sort: "class_name asc" })).map((c) => record("CiClass", c.id))),
  });
  return (
    <Await q={q} skeleton={<ListSkeleton rows={4} />}>
      {(classes) =>
        classes.length === 0 ? (
          <StatePanel kind="empty" title={t("No classes yet")}>
            {t("A class says what kind of item this is and which attributes it carries.")}
          </StatePanel>
        ) : (
          <div className="stack">
            {classes.map((c) => {
              const attrs = Array.isArray(c.attributes) ? (c.attributes as Row[]) : [];
              return (
                <section key={c.id} className="card card-pad stack" aria-label={text(c.class_name)}>
                  <div className="row">
                    <h2 className="section-title">{isolate(text(c.class_name))}</h2>
                    <span className="muted">{t("{n} items", { n: items.filter((i) => text(i.ci_class) === c.id).length })}</span>
                    {c.active === false && <Badge>{t("Not used")}</Badge>}
                  </div>
                  {c.description ? <p className="muted">{isolate(text(c.description))}</p> : null}
                  {attrs.length ? (
                    <ScrollRegion label={t("Attributes of {name}", { name: text(c.class_name) })}>
                      <table className="table">
                        <thead>
                          <tr>
                            <th>{t("Attribute")}</th>
                            <th>{t("Kind")}</th>
                            <th>{t("Required")}</th>
                            <th>{t("Choices")}</th>
                          </tr>
                        </thead>
                        <tbody>
                          {attrs.map((a) => (
                            <tr key={text(a.attr_name)}>
                              <td>{isolate(text(a.attr_name))}</td>
                              <td>{isolate(text(a.attr_type))}</td>
                              <td>{a.required === true ? t("Yes") : t("No")}</td>
                              <td>{isolate(text(a.choices))}</td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    </ScrollRegion>
                  ) : (
                    <p className="muted">{t("It declares no attributes.")}</p>
                  )}
                </section>
              );
            })}
          </div>
        )
      }
    </Await>
  );
}

function TypesView() {
  const q = useQuery({ queryKey: ["itsm", "relation-types"], queryFn: () => rows("RelationType", [], { sort: "relation_name asc" }) });
  const words = FLOWS();
  return (
    <Await q={q} skeleton={<TableSkeleton columns={3} rows={4} />}>
      {(list) => (
        <ScrollRegion label={t("Relation types")}>
          <table className="table">
            <thead>
              <tr>
                <th>{t("Relation")}</th>
                <th>{t("Read from the other end as")}</th>
                <th>{t("A failure flows")}</th>
              </tr>
            </thead>
            <tbody>
              {list.map((r) => (
                <tr key={r.id}>
                  <td>{isolate(text(r.relation_name))}</td>
                  <td>{isolate(text(r.inverse_name))}</td>
                  <td>
                    {isolate(text(r.impact_flows))} <span className="muted">{words[(text(r.impact_flows) || "None") as Flows] ?? ""}</span>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </ScrollRegion>
      )}
    </Await>
  );
}

export function ItemsRoute() {
  const [params, setParams] = useSearchParams();
  const view = ["classes", "types"].includes(params.get("view") ?? "") ? params.get("view")! : "items";
  const [q, setQ] = useState("");
  const rights = useRights("ConfigurationItem");
  const items = useQuery({ queryKey: ["itsm", "items"], queryFn: () => rows("ConfigurationItem", [], { titles: true, sort: "ci_name asc" }) });
  const tabs: [string, string][] = [["items", t("Items")], ["classes", t("Classes")], ["types", t("Relation types")]];
  return (
    <PageStack>
      <Nav />
      <PageHead
        title={t("Configuration items")}
        description={t("What IT runs, and what depends on what.")}
        actions={rights && rights.create !== "none" ? <ButtonLink to="/e/ConfigurationItem/new" variant="primary" icon="plus">{t("New item")}</ButtonLink> : undefined}
      />
      <div className="segmented" role="group" aria-label={t("Show")}>
        {tabs.map(([id, label]) => (
          <button key={id} type="button" aria-pressed={view === id} onClick={() => setParams({ view: id }, { replace: true })}>
            {label}
          </button>
        ))}
      </div>
      {view === "types" ? (
        <TypesView />
      ) : view === "classes" ? (
        <ClassesView items={items.data ?? []} />
      ) : (
        <>
          <Field label={t("Find an item")} htmlFor="itsm-ci-q">
            <input id="itsm-ci-q" className="input" type="search" value={q} onChange={(e) => setQ(e.target.value)} autoComplete="off" />
          </Field>
          <Await q={items} skeleton={<TableSkeleton columns={5} rows={6} />}>
            {(all) => {
              const w = q.trim().toLowerCase();
              const shown = all.filter((i) => !w || `${text(i.ci_name)} ${titleOf(i, "ci_class")} ${text(i.owner)} ${text(i.location)}`.toLowerCase().includes(w));
              return shown.length === 0 ? (
                <StatePanel kind={w ? "no-matches" : "empty"} title={w ? undefined : t("No configuration items yet")} onClear={w ? () => setQ("") : undefined} clearLabel={t("Clear the search")}>
                  {w ? t("No item matches “{q}”.", { q: isolate(q.trim()) }) : t("Items come in by discovery, by import or by hand.")}
                </StatePanel>
              ) : (
                <ScrollRegion label={t("Configuration items")}>
                  <table className="table">
                    <thead>
                      <tr>
                        <th>{t("Name")}</th>
                        <th>{t("Class")}</th>
                        <th>{t("Status")}</th>
                        <th>{t("Environment")}</th>
                        <th>{t("Criticality")}</th>
                        <th>{t("Owner")}</th>
                      </tr>
                    </thead>
                    <tbody>
                      {shown.map((i) => (
                        <tr key={i.id}>
                          <td>
                            <Link to={`/ci/${i.id}`}>{isolate(text(i.ci_name))}</Link> {i.missing_since ? <Badge tone="amber">{t("Not seen since {day}", { day: text(i.missing_since) })}</Badge> : null}
                          </td>
                          <td>{isolate(titleOf(i, "ci_class") || text(i.ci_type))}</td>
                          <td>
                            <StatusBadge value={i.status} />
                          </td>
                          <td>{isolate(text(i.environment))}</td>
                          <td>
                            <Badge tone={tone(text(i.criticality))}>{isolate(text(i.criticality))}</Badge>
                          </td>
                          <td>{isolate(text(i.owner))}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </ScrollRegion>
              );
            }}
          </Await>
        </>
      )}
    </PageStack>
  );
}

// ── an item ──

function Tree({ nodes, parent }: { nodes: Node[]; parent: string }) {
  const kids = childrenOf(nodes, parent);
  if (!kids.length) return null;
  return (
    <ul>
      {kids.map((n) => (
        <li key={n.item.id}>
          <Link to={`/ci/${n.item.id}`}>{isolate(n.item.name)}</Link> <Badge tone={tone(n.item.criticality)}>{isolate(n.item.criticality)}</Badge>{" "}
          <span className="muted">{t("{from} {relation} {to}", { from: isolate(n.edge.from), relation: isolate(n.edge.relation), to: isolate(n.edge.to) })}</span>
          <Tree nodes={nodes} parent={n.item.id} />
        </li>
      ))}
    </ul>
  );
}

function Impact({ id }: { id: string }) {
  const settings = useSettings();
  const depth = Math.max(1, Number(settings.data?.impact_depth ?? 10));
  const q = useQuery({
    queryKey: ["itsm", "impact", id, depth],
    enabled: !settings.isPending,
    queryFn: async () => {
      const types = typesOf(await rows("RelationType"));
      const cache = new Map<string, Promise<ReturnType<typeof itemOf>>>();
      const src: Source = {
        item: (x) => (cache.get(x) ?? cache.set(x, record("ConfigurationItem", x).then(itemOf)).get(x))!,
        dependents: (x) => rows("ConfigurationItem", [["relations.target", "=", x]], { fields: "ci_name" }).then((r) => r.map((y) => y.id)),
      };
      return walk(id, depth, src, types);
    },
  });
  return (
    <section className="card card-pad stack" aria-labelledby="itsm-ci-impact">
      <h2 id="itsm-ci-impact" className="section-title">
        {t("What a failure of it reaches")}
      </h2>
      <Await q={q} skeleton={<ListSkeleton rows={3} />}>
        {({ nodes, cut }) =>
          nodes.length === 0 ? (
            <p className="muted">{t("Nothing depends on it, and it is part of nothing that fails with it.")}</p>
          ) : (
            <>
              <p className="muted">{t("{n} items, followed up to {depth} relations out: {rings}", { n: nodes.length, depth, rings: rings(nodes).map((r, i) => t("{n} at {ring}", { n: r, ring: i + 1 })).join(", ") })}</p>
              <div className="walk" role="tree" aria-label={t("What a failure of it reaches")}>
                <Tree nodes={nodes} parent={id} />
              </div>
              {cut && <Alert tone="warning">{t("Only the nearest items are shown: there are more.")}</Alert>}
            </>
          )
        }
      </Await>
    </section>
  );
}

function Incoming({ id }: { id: string }) {
  const q = useQuery({
    queryKey: ["itsm", "incoming", id],
    queryFn: async () => {
      const [types, ids] = await Promise.all([rows("RelationType").then(typesOf), rows("ConfigurationItem", [["relations.target", "=", id]], { fields: "ci_name" })]);
      const items = await Promise.all(ids.slice(0, 50).map((r) => record("ConfigurationItem", r.id)));
      return items.flatMap((i) => (itemOf(i).relations.filter((r) => r.target === id).map((r) => ({ id: i.id, name: text(i.ci_name), as: (r.type ? types.get(r.type)?.inverse : "") || flowsOf(r, types).name }))));
    },
  });
  return (
    <Await q={q} skeleton={<ListSkeleton rows={2} />}>
      {(list) =>
        list.length === 0 ? (
          <p className="muted">{t("No other item relates to it.")}</p>
        ) : (
          <ul className="stack">
            {list.map((r) => (
              <li key={`${r.id}-${r.as}`}>
                <span className="muted">{isolate(r.as)}</span> <Link to={`/ci/${r.id}`}>{isolate(r.name)}</Link>
              </li>
            ))}
          </ul>
        )
      }
    </Await>
  );
}

/** Incidents or changes that name the item, open ones first; what a role may not read is said to be so. */
function Records({ id, entity }: { id: string; entity: "Incident" | "ItChange" }) {
  const q = useQuery({
    queryKey: ["itsm", "on-item", entity, id],
    queryFn: async () => {
      const [a, b] = await Promise.all(
        (entity === "Incident"
          ? [rows("Incident", [["ci", "=", id]], { sort: "created_at desc" }), rows("Incident", [["service", "=", id]], { sort: "created_at desc" })]
          : [rows("ItChange", [["ci", "=", id]], { sort: "planned_start desc" }), rows("ItChange", [["affected.item", "=", id]], { sort: "planned_start desc" })]
        ).map(maybe),
      );
      if (a === null && b === null) return null;
      const seen = new Set<string>();
      return [...(a ?? []), ...(b ?? [])].filter((r) => !seen.has(r.id) && seen.add(r.id));
    },
  });
  const done = entity === "Incident" ? ["Resolved", "Closed"] : ["Implemented", "Closed", "Rejected", "Failed"];
  return (
    <Await q={q} skeleton={<ListSkeleton rows={2} />}>
      {(list) => {
        if (list === null) return <p className="muted">{t("These are not shown to you.")}</p>;
        if (!list.length) return <p className="muted">{entity === "Incident" ? t("No incident names it.") : t("No change names it.")}</p>;
        const sorted = [...list].sort((x, y) => Number(done.includes(text(x.status))) - Number(done.includes(text(y.status))));
        return (
          <ul className="stack">
            {sorted.slice(0, 20).map((r) => (
              <li key={r.id} className="row">
                <Link to={entity === "Incident" ? `/e/Incident/${r.id}` : `/change/${r.id}`}>{isolate(`${text(r.number)} ${text(r.title)}`.trim())}</Link>
                <StatusBadge value={r.status} />
                {entity === "ItChange" && r.planned_start ? <span className="muted">{when(r.planned_start)}</span> : null}
              </li>
            ))}
          </ul>
        );
      }}
    </Await>
  );
}

function Section({ id, title, children }: { id: string; title: string; children: ReactNode }) {
  return (
    <section className="card card-pad stack" aria-labelledby={id}>
      <h2 id={id} className="section-title">
        {title}
      </h2>
      {children}
    </section>
  );
}

export function ItemRoute() {
  const { id = "" } = useParams();
  const q = useQuery({
    queryKey: ["itsm", "ci", id],
    queryFn: async () => {
      const item = await record("ConfigurationItem", id);
      const cls = item.ci_class ? await record("CiClass", text(item.ci_class)).catch(() => null) : null;
      return { item, cls };
    },
  });
  const rights = useRights("ConfigurationItem");
  const types = useQuery({ queryKey: ["itsm", "relation-types"], queryFn: () => rows("RelationType", [], { sort: "relation_name asc" }), staleTime: 60_000 });
  const typeMap = useMemo(() => typesOf(types.data ?? []), [types.data]);
  return (
    <PageStack>
      <Nav />
      <Await q={q} skeleton={<FormSkeleton fields={5} />}>
        {({ item, cls }) => {
          const declared = Array.isArray(cls?.attributes) ? (cls.attributes as Row[]) : [];
          const values = (item.attributes && typeof item.attributes === "object" ? item.attributes : {}) as Record<string, unknown>;
          const relations = Array.isArray(item.relations) ? (item.relations as Row[]) : [];
          const subscribers = Array.isArray(item.subscribers) ? (item.subscribers as Row[]) : [];
          return (
            <>
              <PageHead
                title={text(item.ci_name)}
                back={{ to: "/items", label: t("Configuration items") }}
                description={
                  <>
                    <StatusBadge value={item.status} /> <Badge tone={tone(text(item.criticality))}>{isolate(text(item.criticality))}</Badge> {isolate([titleOf(item, "ci_class"), text(item.environment)].filter(Boolean).join(" · "))}
                  </>
                }
                actions={rights && rights.write !== "none" ? <ButtonLink to={`/e/ConfigurationItem/${id}`}>{t("Edit the item")}</ButtonLink> : undefined}
              />
              {item.missing_since ? <Alert tone="warning">{t("Its discovery source has not reported it since {day}.", { day: text(item.missing_since) })}</Alert> : null}
              <PageSplit
                main={
                  <>
                    <Impact id={id} />
                    <Section id="itsm-ci-inc" title={t("Incidents on it")}>
                      <Records id={id} entity="Incident" />
                    </Section>
                    <Section id="itsm-ci-chg" title={t("Changes on it")}>
                      <Records id={id} entity="ItChange" />
                    </Section>
                  </>
                }
                aside={
                  <>
                    <Section id="itsm-ci-facts" title={t("The item")}>
                      <dl className="stack">
                        {[
                          [t("Owner"), text(item.owner)],
                          [t("Location"), text(item.location)],
                          [t("Serial number"), text(item.serial_number)],
                          [t("Discovered by"), text(item.discovered_by)],
                          [t("Last seen"), when(item.last_seen_at)],
                        ]
                          .filter(([, v]) => v)
                          .map(([k, v]) => (
                            <div key={k}>
                              <dt className="muted">{k}</dt>
                              <dd>{isolate(v)}</dd>
                            </div>
                          ))}
                        {declared.map((a) => (
                          <div key={text(a.attr_name)}>
                            <dt className="muted">{isolate(text(a.attr_name))}</dt>
                            <dd>{values[text(a.attr_name)] == null || values[text(a.attr_name)] === "" ? "—" : isolate(String(values[text(a.attr_name)]))}</dd>
                          </div>
                        ))}
                      </dl>
                    </Section>
                    <Section id="itsm-ci-rel" title={t("Relations")}>
                      <Refused error={types.error} />
                      {relations.length === 0 ? (
                        <p className="muted">{t("It lists no relation.")}</p>
                      ) : (
                        <ul className="stack">
                          {relations.map((r, n) => {
                            const f = flowsOf({ type: text(r.relation_type), kind: text(r.kind), target: text(r.target) }, typeMap);
                            return (
                              <li key={n}>
                                {isolate(f.name)} <Link to={`/ci/${text(r.target)}`}>{isolate(titleOf(r, "target"))}</Link> <span className="muted">{FLOWS()[f.flows]}</span>
                              </li>
                            );
                          })}
                        </ul>
                      )}
                      <strong>{t("Related to it")}</strong>
                      <Incoming id={id} />
                    </Section>
                    {subscribers.length > 0 && (
                      <Section id="itsm-ci-subs" title={t("Told of a major incident")}>
                        <ul className="stack">
                          {subscribers.map((s) => (
                            <li key={text(s.subscriber)}>{isolate(text(s.subscriber))}</li>
                          ))}
                        </ul>
                      </Section>
                    )}
                  </>
                }
              />
            </>
          );
        }}
      </Await>
    </PageStack>
  );
}
