// The desk: open incidents by priority (breached ones marked), catalog requests waiting, the
// changes approved and coming up, and the items discovery no longer finds. Each opens its record.

import { useQuery } from "@tanstack/react-query";
import { Link } from "react-router";
import { api, docPath } from "@ferrum/client";
import { Badge, ErrorState, PageLoading, useShellTitle } from "@ferrum/kit";

export interface Row {
  id: string;
  [k: string]: unknown;
}

const rows = (entity: string, filters: unknown[], extra: Record<string, string | number> = {}) =>
  api.get<{ rows: Row[] }>(docPath(entity), { filters: JSON.stringify(filters), limit: 500, ...extra }).then((r) => r.rows);

/** Open incidents, P1 first, then the soonest due. */
export function byPriority(list: Row[]): Row[] {
  return [...list].sort((a, b) => String(a.priority ?? "P9").localeCompare(String(b.priority ?? "P9")) || String(a.resolve_by ?? "").localeCompare(String(b.resolve_by ?? "")));
}

const TONE: Record<string, "red" | "amber" | "blue" | "neutral"> = { P1: "red", P2: "amber", P3: "blue", P4: "neutral" };

export function Home() {
  useShellTitle("IT Services");
  const q = useQuery({
    queryKey: ["itsm", "desk"],
    queryFn: async () => {
      const [incidents, requests, changes, missing] = await Promise.all([
        rows("Incident", [["status", "in", ["New", "In Progress", "On Hold"]]]),
        rows("CatalogRequest", [["status", "in", ["Submitted", "Approved"]]]),
        rows("ItChange", [["status", "in", ["Submitted", "Approved"]]], { sort: "planned_start asc" }),
        // Only those who may read configuration items see them.
        rows("ConfigurationItem", [["missing_since", "is", "set"]], { sort: "missing_since asc" }).catch(() => [] as Row[]),
      ]);
      return { incidents: byPriority(incidents), requests, changes, missing };
    },
    refetchInterval: 30_000,
  });
  if (q.isPending) return <PageLoading />;
  if (q.isError) return <ErrorState error={q.error} onRetry={() => void q.refetch()} />;
  return (
    <div className="np-page">
      <header className="row" style={{ justifyContent: "space-between" }}>
        <h2>The desk</h2>
        <Link to="/e/Incident/new">New incident</Link>
      </header>
      <div className="np-columns">
        <section className="card card-pad stack" aria-label="Open incidents">
          <h3>Open incidents ({q.data.incidents.length})</h3>
          <ul className="stack">
            {q.data.incidents.map((i) => (
              <li key={i.id}>
                <Badge tone={TONE[String(i.priority)] ?? "neutral"}>{String(i.priority ?? "")}</Badge> <Link to={`/e/Incident/${i.id}`}>{String(i.title)}</Link>
                {i.breached ? <Badge tone="red">Breached</Badge> : null}
              </li>
            ))}
          </ul>
        </section>
        <section className="card card-pad stack" aria-label="Requests waiting">
          <h3>Requests waiting ({q.data.requests.length})</h3>
          <ul className="stack">
            {q.data.requests.map((r) => (
              <li key={r.id}>
                <Link to={`/e/CatalogRequest/${r.id}`}>{`${String(r.number ?? "")} · ${String(r.status)}`}</Link>
              </li>
            ))}
          </ul>
        </section>
        <section className="card card-pad stack" aria-label="Changes coming up">
          <h3>Changes coming up ({q.data.changes.length})</h3>
          <ul className="stack">
            {q.data.changes.map((c) => (
              <li key={c.id}>
                <Link to={`/e/ItChange/${c.id}`}>{`${String(c.number ?? "")} · ${String(c.title)} · ${String(c.status)}`}</Link>
              </li>
            ))}
          </ul>
        </section>
        {q.data.missing.length > 0 && (
          <section className="card card-pad stack" aria-label="Not seen by discovery">
            <h3>Not seen by discovery ({q.data.missing.length})</h3>
            <ul className="stack">
              {q.data.missing.map((c) => (
                <li key={c.id}>
                  <Link to={`/e/ConfigurationItem/${c.id}`}>{String(c.ci_name)}</Link>{" "}
                  <span className="faint">
                    · {String(c.discovered_by ?? "")}, last {String(c.missing_since ?? "")}
                  </span>
                </li>
              ))}
            </ul>
          </section>
        )}
      </div>
    </div>
  );
}
