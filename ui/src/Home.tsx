// The desk: open incidents by priority (breached ones marked), catalog requests waiting with what
// was asked for, by whom and when it is due, the changes coming up with when they start, and the
// items discovery no longer finds. Each opens its record.

import { useQuery } from "@tanstack/react-query";
import { api, docPath, formatMoment, t, isolate } from "@ferrum/client";
import { Badge, ButtonLink, HomeSkeleton, LoadFailed, PageHead, PageStack, Queue, QueueGrid, QueueItem, StatusBadge, type Tone, useShellTitle } from "@ferrum/kit";

export interface Row {
  id: string;
  [k: string]: unknown;
}

const rows = (entity: string, filters: unknown[], extra: Record<string, string | number | boolean> = {}) =>
  api.get<{ rows: Row[] }>(docPath(entity), { filters: JSON.stringify(filters), limit: 500, ...extra }).then((r) => r.rows);

/** Open incidents, P1 first, then the soonest due. */
export function byPriority(list: Row[]): Row[] {
  return [...list].sort((a, b) => String(a.priority ?? "P9").localeCompare(String(b.priority ?? "P9")) || String(a.resolve_by ?? "").localeCompare(String(b.resolve_by ?? "")));
}

const TONE: Record<string, Tone> = { P1: "red", P2: "amber", P3: "blue", P4: "neutral" };

/** The tone of a priority mark, from its stored id (P1..P4); any other value is neutral. */
export const priorityTone = (p: unknown) => (typeof p === "string" && Object.prototype.hasOwnProperty.call(TONE, p) ? TONE[p] : "neutral");

/** The queues show this many rows each; the rest is "and N more", linking to the list. */
const LIMIT = 5;

/** A moment on the shown clock: "8 Nov, 20:21". */
const when = (at: unknown) => (typeof at === "string" && at ? formatMoment(at, { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" }) : "");

/** A request as the desk reads it: what was asked for, by whom, and when it is due. */
export function requestLine(r: Row): string {
  const asked = String((r._titles as Record<string, string> | undefined)?.offering ?? "");
  const due = when(r.due_at);
  return [String(r.number ?? ""), asked, r.requested_by ? t("by {value}", { value: String(r.requested_by) }) : "", due ? t("due {due}", { due }) : "", String(r.status ?? "")].filter(Boolean).join(" · ");
}

/** A change as the desk reads it: what it is, when it starts, and where it stands. */
export function changeLine(c: Row): string {
  const starts = when(c.planned_start);
  return [String(c.number ?? ""), String(c.title ?? ""), starts ? t("from {starts}", { starts }) : "", String(c.status ?? "")].filter(Boolean).join(" · ");
}

export function Home() {
  useShellTitle(t("IT Services"));
  const q = useQuery({
    queryKey: ["itsm", "desk"],
    queryFn: async () => {
      const [incidents, requests, changes, missing] = await Promise.all([
        rows("Incident", [["status", "in", ["New", "In Progress", "On Hold"]]]),
        rows("CatalogRequest", [["status", "in", ["Submitted", "Approved"]]], { titles: true, sort: "due_at asc" }),
        rows("ItChange", [["status", "in", ["Submitted", "Approved"]]], { sort: "planned_start asc" }),
        // Only those who may read configuration items see them.
        rows("ConfigurationItem", [["missing_since", "is", "set"]], { sort: "missing_since asc" }).catch(() => [] as Row[]),
      ]);
      return { incidents: byPriority(incidents), requests, changes, missing };
    },
    refetchInterval: 30_000,
  });
  const create = (
    <ButtonLink to="/e/Incident/new" variant="primary" icon="plus">
      {t("New incident")}
    </ButtonLink>
  );
  if (q.isPending) return (
    <PageStack>
      <PageHead title={t("The desk")} />
      <HomeSkeleton queues={3} />
    </PageStack>
  );
  if (q.isError) return (
    <PageStack>
      <PageHead title={t("The desk")} actions={create} />
      <LoadFailed error={q.error} onRetry={() => void q.refetch()} />
    </PageStack>
  );
  const { incidents, requests, changes, missing } = q.data;
  const summary = [incidents.length && t("{n} open incidents", { n: incidents.length }), requests.length && t("{n} requests waiting", { n: requests.length })].filter(Boolean).join(" · ");
  const parts = (...p: unknown[]) => isolate(p.filter(Boolean).join(" · "));
  return (
    <PageStack>
      <PageHead title={t("The desk")} description={summary ? isolate(summary) : undefined} actions={create} />
      <QueueGrid>
        <Queue title={t("Open incidents")} count={incidents.length} tone={incidents.some((i) => i.breached) ? "attention" : "default"} more={{ to: "/e/Incident", count: incidents.length - LIMIT }}>
          {incidents.slice(0, LIMIT).map((i) => (
            <QueueItem
              key={i.id}
              to={`/e/Incident/${i.id}`}
              title={isolate(String(i.title))}
              meta={parts(i.number, when(i.resolve_by) && t("due {due}", { due: when(i.resolve_by) }))}
              trailing={
                <>
                  {i.breached ? <Badge tone="red">{t("Breached")}</Badge> : null}
                  <Badge tone={priorityTone(i.priority)}>{isolate(String(i.priority ?? ""))}</Badge>
                </>
              }
            />
          ))}
        </Queue>
        <Queue title={t("Requests waiting")} count={requests.length} more={{ to: "/e/CatalogRequest", count: requests.length - LIMIT }}>
          {requests.slice(0, LIMIT).map((r) => {
            const asked = String((r._titles as Record<string, string> | undefined)?.offering ?? "");
            return (
              <QueueItem
                key={r.id}
                to={`/e/CatalogRequest/${r.id}`}
                title={isolate(asked || String(r.number ?? ""))}
                meta={parts(asked && r.number, when(r.due_at) && t("due {due}", { due: when(r.due_at) }), r.requested_by && t("by {value}", { value: String(r.requested_by) }))}
                trailing={<StatusBadge value={r.status} />}
              />
            );
          })}
        </Queue>
        <Queue title={t("Changes coming up")} count={changes.length} more={{ to: "/e/ItChange", count: changes.length - LIMIT }}>
          {changes.slice(0, LIMIT).map((c) => (
            <QueueItem key={c.id} to={`/e/ItChange/${c.id}`} title={isolate(String(c.title ?? c.number ?? ""))} meta={parts(c.number, when(c.planned_start) && t("from {starts}", { starts: when(c.planned_start) }))} trailing={<StatusBadge value={c.status} />} />
          ))}
        </Queue>
        {missing.length > 0 && (
          <Queue title={t("Not seen by discovery")} count={missing.length} more={{ to: "/e/ConfigurationItem", count: missing.length - LIMIT }}>
            {missing.slice(0, LIMIT).map((c) => (
              <QueueItem key={c.id} to={`/e/ConfigurationItem/${c.id}`} title={isolate(String(c.ci_name))} meta={isolate(`${String(c.discovered_by ?? "")}${t(", last")} ${String(c.missing_since ?? "")}`)} />
            ))}
          </Queue>
        )}
      </QueueGrid>
    </PageStack>
  );
}
