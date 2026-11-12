// The change calendar: scheduled changes by window, as a month of days or as a list, from the report the server keeps
// (GET /api/app/itsm/calendar). A change that collides with another on one of the same items or services, and a freeze,
// are marked with words as well as colour; a list filtered by a service or item shows only what touches it.

import { useQuery } from "@tanstack/react-query";
import { Link, useSearchParams } from "react-router";
import { api, currentLanguage, formatMoment, isolate, t, todayIso } from "@ferrum/client";
import { Badge, Button, ButtonLink, Field, PageHead, PageStack, ScrollRegion, StatePanel, StatusBadge, TableSkeleton } from "@ferrum/kit";
import { type Row, rows, text } from "./data";
import { byDay, entriesOf, freezeDays, inMonth, itemsOf, monthDays, shiftMonth, touching, type Entry } from "./month";
import { Await, Nav } from "./ui";

/** The weekday a week begins on in the page's language (1 Monday to 7 Sunday); Monday where the browser does not say. */
function weekStart(): number {
  try {
    return (new Intl.Locale(currentLanguage()) as Intl.Locale & { weekInfo?: { firstDay: number } }).weekInfo?.firstDay ?? 1;
  } catch {
    return 1;
  }
}

const time = (iso: string) => formatMoment(iso, { hour: "2-digit", minute: "2-digit" });
const dayTime = (iso: string) => formatMoment(iso, { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" });

function useCalendar(from: string, to: string) {
  return useQuery({
    queryKey: ["itsm", "calendar", from, to],
    queryFn: async () => {
      const [report, freezes] = await Promise.all([
        api.get<{ rows: Record<string, unknown>[] }>("/api/app/itsm/calendar", { from_date: from, to_date: to }),
        // Freezes shade their days; a person who may not read them still sees the freeze a change falls in, named on it.
        rows("ChangeFreeze", [["ends_at", ">", `${from}T00:00:00Z`], ["starts_at", "<", `${to}T23:59:59Z`]]).catch(() => [] as Row[]),
      ]);
      const entries = entriesOf(report);
      // The report names a change by its number; its page is found by it.
      const known = entries.length ? await rows("ItChange", [["number", "in", entries.map((e) => e.number)]], { fields: "number" }).catch(() => [] as Row[]) : [];
      return { entries, freezes, ids: new Map(known.map((r) => [text(r.number), r.id])) };
    },
  });
}

function Flags({ e }: { e: Entry }) {
  return (
    <>
      {e.collides.length > 0 && <Badge tone="red">{t("Collides with {who}", { who: e.collides.join(", ") })}</Badge>}
      {e.freeze.length > 0 && <Badge tone="amber">{t("In the freeze for {why}", { why: e.freeze.join(", ") })}</Badge>}
    </>
  );
}

function Name({ e, ids }: { e: Entry; ids: Map<string, string> }) {
  const label = isolate(`${e.number} ${e.title}`.trim());
  const id = ids.get(e.number);
  return id ? <Link to={`/change/${id}`}>{label}</Link> : <span>{label}</span>;
}

export function CalendarRoute() {
  const [params, setParams] = useSearchParams();
  const today = todayIso();
  const month = /^\d{4}-\d{2}$/.test(params.get("m") ?? "") ? params.get("m")! : today.slice(0, 7);
  const view = params.get("v") === "list" ? "list" : "month";
  const item = params.get("item") ?? "";
  const first = weekStart();
  const days = monthDays(month, first);
  const q = useCalendar(days[0], days[days.length - 1]);
  const put = (next: Record<string, string>) => setParams((cur) => ({ ...Object.fromEntries(cur), ...next }), { replace: true });
  const label = formatMoment(new Date(Date.UTC(Number(month.slice(0, 4)), Number(month.slice(5)) - 1, 15)), { month: "long", year: "numeric" });
  const weekday = new Intl.DateTimeFormat(currentLanguage(), { weekday: "short", timeZone: "UTC" });

  return (
    <PageStack>
      <Nav />
      <PageHead
        title={t("Change calendar")}
        description={t("Changes submitted, approved or being implemented, by the window they are planned for.")}
        actions={
          <ButtonLink to="/change/new" variant="primary" icon="plus">
            {t("Plan a change")}
          </ButtonLink>
        }
      />
      <div className="row">
        <Button size="sm" icon="left" aria-label={t("Previous month")} onClick={() => put({ m: shiftMonth(month, -1) })} />
        <h2 className="section-title" aria-live="polite">
          {label}
        </h2>
        <Button size="sm" icon="chevronRight" aria-label={t("Next month")} onClick={() => put({ m: shiftMonth(month, 1) })} />
        <Button size="sm" onClick={() => put({ m: today.slice(0, 7) })}>
          {t("Today")}
        </Button>
        <div className="segmented" role="group" aria-label={t("View")}>
          <button type="button" aria-pressed={view === "month"} onClick={() => put({ v: "month" })}>
            {t("Month")}
          </button>
          <button type="button" aria-pressed={view === "list"} onClick={() => put({ v: "list" })}>
            {t("List")}
          </button>
        </div>
        {q.data && (
          <Field label={t("Service or item")} htmlFor="itsm-cal-item">
            <select id="itsm-cal-item" className="select" value={item} onChange={(e) => put({ item: e.target.value })}>
              <option value="">{t("All")}</option>
              {itemsOf(q.data.entries).map((n) => (
                <option key={n} value={n}>
                  {isolate(n)}
                </option>
              ))}
            </select>
          </Field>
        )}
      </div>
      <Await q={q} skeleton={<TableSkeleton columns={5} rows={6} />}>
        {({ entries, freezes, ids }) => {
          const shown = touching(entries, item);
          const frozen = freezeDays(freezes);
          const perDay = byDay(shown);
          const listed = shown.filter((e) => inMonth(e, month));
          if (view === "list")
            return listed.length ? (
              <ScrollRegion label={t("Scheduled changes")}>
                <table className="table">
                  <thead>
                    <tr>
                      <th>{t("Window")}</th>
                      <th>{t("Change")}</th>
                      <th>{t("Type")}</th>
                      <th>{t("Status")}</th>
                      <th>{t("Items and services")}</th>
                      <th>{t("Flags")}</th>
                    </tr>
                  </thead>
                  <tbody>
                    {listed.map((e) => (
                      <tr key={e.number}>
                        <td>
                          {dayTime(e.start)} – {dayTime(e.end)}
                        </td>
                        <td>
                          <Name e={e} ids={ids} />
                        </td>
                        <td>{isolate(e.type)}</td>
                        <td>
                          <StatusBadge value={e.status} />
                        </td>
                        <td>{isolate(e.items.join(", "))}</td>
                        <td>
                          <Flags e={e} />
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </ScrollRegion>
            ) : (
              <StatePanel kind={item ? "no-matches" : "empty"} title={item ? undefined : t("Nothing is scheduled this month")} onClear={item ? () => put({ item: "" }) : undefined}>
                {item ? t("No change touches {item} this month.", { item: isolate(item) }) : t("Changes appear here once they are submitted with a window.")}
              </StatePanel>
            );
          return (
            <div className="cal-grid" role="grid" aria-label={label}>
              {days.slice(0, 7).map((d) => (
                <div key={d} className="cal-head" role="columnheader">
                  {weekday.format(new Date(`${d}T12:00:00Z`))}
                </div>
              ))}
              {days.map((d) => {
                const list = perDay.get(d) ?? [];
                const why = frozen.get(d);
                return (
                  <div key={d} className="cal-day" role="gridcell" data-outside={d.startsWith(month) ? undefined : ""} data-freeze={why ? "" : undefined} data-today={d === today ? "" : undefined} data-empty={list.length || why ? undefined : ""}>
                    <span className="cal-num">{Number(d.slice(8))}</span>
                    {why && <span className="cal-note">{t("Freeze: {why}", { why: isolate(why.join(", ")) })}</span>}
                    {list.map((e) => (
                      <div key={e.number} className="cal-entry">
                        <Name e={e} ids={ids} />
                        <span className="muted">{time(e.start)}</span>
                        <Flags e={e} />
                      </div>
                    ))}
                  </div>
                );
              })}
            </div>
          );
        }}
      </Await>
    </PageStack>
  );
}
