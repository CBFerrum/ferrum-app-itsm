// The change calendar as the report gives it (GET /api/app/itsm/calendar?from_date=&to_date=, src/reports.rs): one row to a scheduled
// change with its window as text in UTC ("2026-11-30 10:00 to 2026-11-30 12:00"), the items and services it touches, the other
// scheduled changes that collide with it and the freezes it falls in. Here the rows become entries with real moments, and the
// entries are laid on the days of a month on the clock the page shows.

import { toZonedInput } from "@ferrum/client";
import type { Row } from "./data";
import { text } from "./data";

export interface Entry {
  number: string;
  title: string;
  type: string;
  status: string;
  /** The names of the items and services it touches. */
  items: string[];
  collides: string[];
  freeze: string[];
  start: string;
  end: string;
}

/** "2026-11-30 10:00 to 2026-11-30 12:00" (UTC) as two instants; `null` when it is not that. */
export function parseWindow(w: string): { start: string; end: string } | null {
  const m = /^(\d{4}-\d{2}-\d{2}) (\d{2}:\d{2}) to (\d{4}-\d{2}-\d{2}) (\d{2}:\d{2})$/.exec(w.trim());
  return m ? { start: `${m[1]}T${m[2]}:00Z`, end: `${m[3]}T${m[4]}:00Z` } : null;
}

const list = (v: unknown) => text(v).split(", ").map((s) => s.trim()).filter(Boolean);

/** The report's rows as entries, those whose window it could not read left out. */
export function entriesOf(report: { rows: Record<string, unknown>[] }): Entry[] {
  return report.rows.flatMap((r) => {
    const w = parseWindow(text(r.window));
    return w ? [{ number: text(r.number), title: text(r.title), type: text(r.type), status: text(r.status), items: list(r.items), collides: list(r.collides), freeze: list(r.freeze), ...w }] : [];
  });
}

/** The names of every item and service the entries touch, for the filter. */
export const itemsOf = (entries: Entry[]) => [...new Set(entries.flatMap((e) => e.items))].sort((a, b) => a.localeCompare(b));

/** Only what touches the item or service (all of them when none is named). */
export const touching = (entries: Entry[], item: string) => (item ? entries.filter((e) => e.items.includes(item)) : entries);

// ── days ──

const DAY = 86_400_000;
const utc = (day: string) => Date.parse(`${day}T00:00:00Z`);
const dayString = (ms: number) => new Date(ms).toISOString().slice(0, 10);

/** The first day shown in the first week, given the weekday a week begins on (1 Monday to 7 Sunday). */
export function monthDays(month: string, firstDay = 1): string[] {
  const first = utc(`${month}-01`);
  const weekday = new Date(first).getUTCDay() || 7;
  const start = first - ((weekday - firstDay + 7) % 7) * DAY;
  const next = new Date(first);
  const last = Date.UTC(next.getUTCFullYear(), next.getUTCMonth() + 1, 0);
  const weeks = Math.ceil((last - start) / DAY / 7 + 1 / 7);
  return Array.from({ length: weeks * 7 }, (_, i) => dayString(start + i * DAY));
}

/** The month a day is in (`YYYY-MM`) and the one before and after it. */
export const shiftMonth = (month: string, by: number) => {
  const [y, m] = month.split("-").map(Number);
  const d = new Date(Date.UTC(y, m - 1 + by, 1));
  return `${d.getUTCFullYear()}-${String(d.getUTCMonth() + 1).padStart(2, "0")}`;
};

/** The days of the shown clock an instant range covers: the day it starts on to the day it ends on. */
export function daysCovered(start: string, end: string): string[] {
  const [a, b] = [toZonedInput(start).slice(0, 10), toZonedInput(end).slice(0, 10)];
  if (!a || !b) return [];
  const out: string[] = [];
  for (let d = utc(a); d <= utc(b) && out.length < 400; d += DAY) out.push(dayString(d));
  return out;
}

/** Entries by the days they cover on the shown clock; a change over midnight is on both days. */
export function byDay(entries: Entry[]): Map<string, Entry[]> {
  const out = new Map<string, Entry[]>();
  for (const e of entries) for (const d of daysCovered(e.start, e.end)) out.set(d, [...(out.get(d) ?? []), e]);
  for (const list of out.values()) list.sort((a, b) => a.start.localeCompare(b.start) || a.number.localeCompare(b.number));
  return out;
}

/** The days each freeze covers, by day, with its reason. */
export function freezeDays(freezes: Row[]): Map<string, string[]> {
  const out = new Map<string, string[]>();
  for (const f of freezes)
    for (const d of daysCovered(text(f.starts_at), text(f.ends_at))) out.set(d, [...(out.get(d) ?? []), text(f.reason)]);
  return out;
}

/** Whether any day the entry covers is in the month (`YYYY-MM`). */
export const inMonth = (e: Entry, month: string) => daysCovered(e.start, e.end).some((d) => d.startsWith(month));
