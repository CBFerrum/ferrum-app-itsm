// What every screen asks the platform: records by filter, one record, a create, a change, an action, and a record's
// derived values before it is saved. Shapes are the platform's (docs/api.md); nothing here knows a screen.

import { api, docPath, newKey } from "@ferrum/client";

export interface Row {
  id: string;
  version?: number;
  [k: string]: unknown;
}

/** `[field, op, value]`; a field may reach one level into a table (`relations.target`). */
export type Filter = [string, string, unknown];

/** A value as text: a string as it is, nothing as empty. */
export const text = (v: unknown): string => (typeof v === "string" ? v : v == null ? "" : String(v));

/** The title the server gave a link field (`titles: true`). */
export const titleOf = (r: Row, field: string): string => text((r._titles as Record<string, string> | undefined)?.[field]);

/** The rows of an entity the person may read, by filter (500 at most). */
export const rows = (entity: string, filters: Filter[] = [], extra: Record<string, string | number | boolean> = {}) =>
  api.get<{ rows: Row[] }>(docPath(entity), { filters: JSON.stringify(filters), limit: 500, ...extra }).then((r) => r.rows);

/** One record with the titles of what it links to. */
export const record = (entity: string, id: string) => api.get<Row>(docPath(entity, id), { titles: true });

/** Makes a record; pressed twice it is made once. */
export const make = (entity: string, body: Record<string, unknown>) => api.post<Row>(docPath(entity), body, { idempotencyKey: newKey() });

/** Changes a record as it was read (its version), so a change made meanwhile is refused rather than overwritten. */
export const change = (entity: string, id: string, body: Record<string, unknown>, version?: number) =>
  api.patch<Row>(docPath(entity, id), body, { ifMatch: version, idempotencyKey: newKey() });

/** What an action answered: the new state, and `_approval` when the platform asked someone before it ran. */
export type Done = Row & { status?: string; _approval?: { status?: string } };

/** Runs a record's action with its parameters. */
export const run = (entity: string, id: string, action: string, body: Record<string, unknown> = {}) =>
  api.post<Done>(docPath(entity, id, "actions", action), body, { idempotencyKey: newKey() });

/** The values the server works out for a record not yet saved (nothing is written). */
export const derive = (entity: string, body: Record<string, unknown>) => api.post<Row>(docPath(entity, "_calc"), body);
