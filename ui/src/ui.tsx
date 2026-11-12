// What the screens share: the way between them, a person's rights, a dialog that asks for an action's words, the
// states a request goes through, and the marks of a service target.

import type { UseQueryResult } from "@tanstack/react-query";
import { type FormEvent, type ReactNode, useEffect, useState } from "react";
import { useLocation } from "react-router";
import { ApiError, fromZonedInput, isolate, refusal, t } from "@ferrum/client";
import { Alert, Badge, Button, ButtonLink, Dialog, Field, FormSkeleton, LoadFailed, type Tone } from "@ferrum/kit";
import { useMetaIndex, type Perms } from "@ferrum/records";

/** The screens of the app, one press apart on every page. */
export function Nav() {
  const here = useLocation().pathname;
  const items: [string, string, string][] = [
    ["/", t("Desk"), "home"],
    ["/e/ItChange", t("Changes"), "list"],
    ["/calendar", t("Change calendar"), "calendar"],
    ["/problems", t("Problems"), "alert"],
    ["/catalog", t("Catalog"), "store"],
    ["/items", t("Configuration items"), "apps"],
  ];
  return (
    <nav className="row" aria-label={t("IT Services")}>
      {items.map(([to, label, icon]) => (
        <ButtonLink key={to} to={to} size="sm" icon={icon} variant={(to === "/" ? here === "/" : here.startsWith(to)) ? "default" : "ghost"}>
          {label}
        </ButtonLink>
      ))}
    </nav>
  );
}

/** What the person may do with an entity (`undefined` until the metadata is in): the server decides; this only keeps buttons
 *  from being offered that it will refuse. */
export function useRights(entity: string): Perms | undefined {
  return useMetaIndex().data?.entities.find((e) => e.name === entity)?.permissions;
}

/** A request's three states, told apart: waiting (a skeleton the shape of the content), refused or failed (with its words and a
 *  way to try again), and the data. */
export function Await<T>({ q, skeleton, children }: { q: UseQueryResult<T>; skeleton?: ReactNode; children: (data: T) => ReactNode }) {
  if (q.isPending) return <>{skeleton ?? <FormSkeleton />}</>;
  if (q.isError) return <LoadFailed error={q.error} onRetry={() => void q.refetch()} />;
  return <>{children(q.data)}</>;
}

/** The server's plain refusal, as it said it. */
export function Refused({ error }: { error: unknown }) {
  return error ? <Alert tone="error">{isolate(refusal(error))}</Alert> : null;
}

// ── a dialog that asks for an action's words ──

export interface Param {
  name: string;
  label: string;
  kind?: "text" | "line" | "select" | "check" | "moment" | "email";
  required?: boolean;
  options?: string[];
  hint?: string;
}

type Values = Record<string, string | boolean>;

/** What a dialog hands the action: a moment as an instant, a box as true or false, empty answers left out. */
export function bodyOf(params: Param[], values: Values): Record<string, unknown> {
  const body: Record<string, unknown> = {};
  for (const p of params) {
    const v = values[p.name];
    if (p.kind === "check") body[p.name] = v === true;
    else if (typeof v === "string" && v.trim()) body[p.name] = p.kind === "moment" ? (fromZonedInput(v) ?? v) : v.trim();
  }
  return body;
}

/** Which of the parameters a person must still answer. */
export const unanswered = (params: Param[], values: Values) => params.filter((p) => p.required && p.kind !== "check" && !String(values[p.name] ?? "").trim());

/** The words an action needs, asked in one dialog; the server's refusal is shown in it as it is said, on the field it names. */
export function Ask({
  title,
  description,
  params,
  confirm,
  danger,
  onSubmit,
  onClose,
  onDone,
  preview,
}: {
  title: string;
  description?: string;
  params: Param[];
  confirm: string;
  danger?: boolean;
  onSubmit: (body: Record<string, unknown>) => Promise<unknown>;
  onClose: () => void;
  onDone: (answer: unknown) => void;
  /** Shown under the fields and kept in step with them (what will be sent, to whom). */
  preview?: (values: Values) => ReactNode;
}) {
  const [values, setValues] = useState<Values>({});
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [error, setError] = useState<unknown>(null);
  const [busy, setBusy] = useState(false);
  useEffect(() => setErrors({}), [values]);
  const set = (name: string, v: string | boolean) => setValues((cur) => ({ ...cur, [name]: v }));
  const submit = async (e: FormEvent) => {
    e.preventDefault();
    if (busy) return;
    const missing = unanswered(params, values);
    if (missing.length) {
      setErrors(Object.fromEntries(missing.map((p) => [p.name, t("is needed")])));
      return;
    }
    setBusy(true);
    setError(null);
    try {
      onDone(await onSubmit(bodyOf(params, values)));
    } catch (err) {
      const own = err instanceof ApiError ? err.fields.filter((f) => params.some((p) => p.name === f.field)) : [];
      setErrors(Object.fromEntries(own.map((f) => [f.field, f.message])));
      if (!own.length || (err instanceof ApiError && own.length < err.fields.length)) setError(err);
      setBusy(false);
    }
  };
  const control = (p: Param) => {
    const id = `ask-${p.name}`;
    const v = values[p.name];
    switch (p.kind) {
      case "text":
        return <textarea id={id} className="textarea" rows={4} value={String(v ?? "")} onChange={(e) => set(p.name, e.target.value)} />;
      case "select":
        return (
          <select id={id} className="select" value={String(v ?? "")} onChange={(e) => set(p.name, e.target.value)}>
            <option value="">{p.required ? t("Choose…") : "—"}</option>
            {(p.options ?? []).map((o) => (
              <option key={o} value={o}>
                {isolate(o)}
              </option>
            ))}
          </select>
        );
      case "moment":
        return <input id={id} className="input" type="datetime-local" value={String(v ?? "")} onChange={(e) => set(p.name, e.target.value)} />;
      case "email":
        return <input id={id} className="input" type="email" value={String(v ?? "")} onChange={(e) => set(p.name, e.target.value)} />;
      default:
        return <input id={id} className="input" value={String(v ?? "")} onChange={(e) => set(p.name, e.target.value)} />;
    }
  };
  return (
    <Dialog
      open
      onClose={onClose}
      title={title}
      description={description}
      footer={
        <>
          <Button onClick={onClose}>{t("Cancel")}</Button>
          <Button variant={danger ? "danger" : "primary"} type="submit" form="itsm-ask" loading={busy}>
            {confirm}
          </Button>
        </>
      }
    >
      <form id="itsm-ask" className="stack" onSubmit={submit} noValidate>
        <Refused error={error} />
        {params.map((p) =>
          p.kind === "check" ? (
            <label key={p.name} className="check-field">
              <input type="checkbox" checked={values[p.name] === true} onChange={(e) => set(p.name, e.target.checked)} />
              {p.label}
            </label>
          ) : (
            <Field key={p.name} label={p.label} htmlFor={`ask-${p.name}`} required={p.required} error={errors[p.name]} hint={p.hint}>
              {control(p)}
            </Field>
          ),
        )}
        {preview?.(values)}
      </form>
    </Dialog>
  );
}

// ── service targets ──

/** The platform's target states, each with its tone: the tone follows the stored id, never the words. */
const TARGET: Record<string, { tone: Tone; word: () => string }> = {
  running: { tone: "blue", word: () => t("Running") },
  paused: { tone: "neutral", word: () => t("Paused") },
  met: { tone: "green", word: () => t("Met") },
  met_late: { tone: "amber", word: () => t("Met late") },
  breached: { tone: "red", word: () => t("Breached") },
};

export function TargetBadge({ state, label }: { state: unknown; label?: string }) {
  const known = typeof state === "string" && Object.prototype.hasOwnProperty.call(TARGET, state) ? TARGET[state] : null;
  if (!known) return null;
  return <Badge tone={known.tone}>{label ? `${label}: ${known.word()}` : known.word()}</Badge>;
}
