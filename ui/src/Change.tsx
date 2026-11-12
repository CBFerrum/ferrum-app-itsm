// A change as a flow: what it is and where, the risk questionnaire with the score it adds up to and the route that gives, the
// plans, the window with its collisions and freezes, then, once submitted, who was asked and who has answered, and after it is
// implemented its review. What the server works out (risk, impact, route, flags) is asked of it while the form is filled
// (POST /api/v1/ItChange/_calc), so what is shown is what saving will keep; its refusals are shown as it words them.

import { keepPreviousData, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { type FormEvent, useEffect, useMemo, useState } from "react";
import { Link, useNavigate, useParams } from "react-router";
import { ApiError, isolate, t } from "@ferrum/client";
import { Alert, Badge, Button, Field, FormSkeleton, PageHead, PageSplit, PageStack, StatusBadge, toast, useDebounced } from "@ferrum/kit";
import { LinkInput, useLinkTitle } from "@ferrum/records";
import { ApprovalState, useAsked } from "./Approval";
import { derive, change, make, record, rows, run, type Row, text, titleOf } from "./data";
import { emptyDraft, flagLines, fromRow, inFreeze, levelOf, missing, routeLines, toBody, windowWords, type Draft, type Level } from "./draft";
import { Ask, Await, Nav, Refused, useRights, type Param } from "./ui";
import { when } from "./desk";

const LEVELS: Level[] = ["Low", "Medium", "High"];

/** The workspace's change settings (who approves, what a freeze does, where the scores turn); a person who may not read them
 *  gets the defaults the server uses. */
function useSettings() {
  return useQuery({
    queryKey: ["itsm", "settings"],
    queryFn: () => rows("ItsmSettings").then((r) => r[0] ?? null).catch(() => null),
    staleTime: 60_000,
  });
}

const useTypes = () => useQuery({ queryKey: ["itsm", "types"], queryFn: () => rows("ChangeType", [], { titles: true, sort: "type_name asc" }).then((r) => r.filter((x) => x.active !== false)), staleTime: 60_000 });

/** The questionnaire with each question's choices (a list gives no table rows, so each question is read). */
const useQuestions = () =>
  useQuery({
    queryKey: ["itsm", "questions"],
    staleTime: 60_000,
    queryFn: async () => {
      const list = (await rows("RiskQuestion", [], { sort: "position asc" })).filter((q) => q.active !== false);
      return Promise.all(list.map((q) => record("RiskQuestion", q.id)));
    },
  });

/** A configuration item by its name, as a link to its page. */
function ItemLink({ id }: { id: string }) {
  const name = useLinkTitle("ConfigurationItem", id);
  return <Link to={`/ci/${id}`}>{isolate(name ?? "…")}</Link>;
}

function Chip({ id, onRemove }: { id: string; onRemove: () => void }) {
  const name = useLinkTitle("ConfigurationItem", id);
  return (
    <li className="row">
      <span>{isolate(name ?? "…")}</span>
      <Button size="sm" variant="ghost" icon="x" aria-label={t("Remove {name}", { name: name ?? "" })} onClick={onRemove} />
    </li>
  );
}

// ── the flow ──

/** A raised-from-template start: an approved template, the title and the window, and the change it makes opens as a draft. */
function FromTemplate() {
  const go = useNavigate();
  const templates = useQuery({ queryKey: ["itsm", "templates"], queryFn: () => rows("ChangeTemplate", [["approved", "=", true]], { titles: true, sort: "template_name asc" }).then((r) => r.filter((x) => x.active !== false)) });
  const [asking, setAsking] = useState<Row | null>(null);
  const params: Param[] = [
    { name: "title", label: t("Title"), kind: "line", required: true },
    { name: "planned_start", label: t("Starts"), kind: "moment", required: true },
    { name: "planned_end", label: t("Ends"), kind: "moment", required: true },
  ];
  if (templates.isPending) return null;
  // A person who may not read templates is not offered them; any other failure is said.
  if (templates.isError) return (templates.error as { status?: number }).status === 403 ? null : <Refused error={templates.error} />;
  if (!templates.data.length) return null;
  return (
    <section className="card card-pad stack" aria-labelledby="itsm-tpl">
      <h2 id="itsm-tpl" className="section-title">
        {t("Raise it from an approved template")}
      </h2>
      <p className="muted">{t("A change from an approved template on a pre-approved type needs no one's decision.")}</p>
      <ul className="stack">
        {templates.data.map((tpl) => (
          <li key={tpl.id} className="row">
            <span>{isolate(text(tpl.template_name))}</span>
            <span className="muted">{isolate(titleOf(tpl, "ci"))}</span>
            <Button size="sm" onClick={() => setAsking(tpl)}>
              {t("Raise a change")}
            </Button>
          </li>
        ))}
      </ul>
      {asking && (
        <Ask
          title={t("Raise a change from {name}", { name: text(asking.template_name) })}
          params={params}
          confirm={t("Raise it")}
          onClose={() => setAsking(null)}
          onSubmit={async (body) => {
            await run("ChangeTemplate", asking.id, "raise_change", body);
            // The action does not say which change it made: the newest from this template is it.
            const made = await rows("ItChange", [["template", "=", asking.id]], { sort: "created_at desc", limit: 1 });
            return made[0]?.id;
          }}
          onDone={(id) => go(typeof id === "string" ? `/change/${id}` : "/e/ItChange")}
        />
      )}
    </section>
  );
}

function Editor({ row }: { row?: Row }) {
  const go = useNavigate();
  const qc = useQueryClient();
  const [d, setD] = useState<Draft>(() => (row ? fromRow(row) : emptyDraft()));
  const [said, setSaid] = useState<Record<string, string>>({});
  const types = useTypes();
  const questions = useQuestions();
  const settings = useSettings();
  const set = <K extends keyof Draft>(k: K, v: Draft[K]) => (setD((cur) => ({ ...cur, [k]: v })), setSaid({}));

  // A new change starts on the type named Normal, the one a change without a type takes.
  useEffect(() => {
    const normal = types.data?.find((x) => x.type_name === "Normal");
    if (!row && !d.change_type && normal) setD((cur) => (cur.change_type ? cur : { ...cur, change_type: normal.id }));
  }, [types.data, row, d.change_type]);

  const type = types.data?.find((x) => x.id === d.change_type);
  const body = useMemo(() => JSON.stringify({ ...toBody(d), ...(row ? { id: row.id } : {}) }), [d, row]);
  const asked = useDebounced(body, 400);
  const calc = useQuery({
    queryKey: ["itsm", "derive", asked],
    queryFn: () => derive("ItChange", JSON.parse(asked) as Record<string, unknown>),
    enabled: !!d.ci,
    placeholderData: keepPreviousData,
  });
  const derived = calc.data;

  const save = useMutation({
    mutationFn: async (andSubmit: boolean) => {
      const b = toBody(d);
      const saved = row ? await change("ItChange", row.id, b, row.version) : await make("ItChange", b);
      if (andSubmit) await run("ItChange", saved.id, "submit_change");
      return saved;
    },
    onSuccess: (saved) => {
      void qc.invalidateQueries({ queryKey: ["itsm"] });
      toast(t("Saved"));
      go(`/change/${saved.id}`, { replace: !row });
    },
    onError: (e) => {
      if (e instanceof ApiError) setSaid(Object.fromEntries(e.fields.map((f) => [f.field, f.message])));
    },
  });
  const press = (andSubmit: boolean) => (e?: FormEvent) => {
    e?.preventDefault();
    if (save.isPending) return;
    const gaps = missing(d, !!type && type.path !== "Pre-approved");
    if (gaps.length) return setSaid(Object.fromEntries(gaps.map((g) => [g.field, g.word])));
    save.mutate(andSubmit);
  };
  // What the server said that no field here carries.
  const loose = save.error instanceof ApiError && save.error.fields.length && save.error.fields.every((f) => f.field in d || f.field === "risk_answers") ? null : save.error;

  const answered = Object.values(d.answers).some(Boolean);
  const lines = flagLines(derived?.window_flags);
  const refuses = settings.data?.freeze_action !== "Warn" && type?.path !== "Emergency";
  const medium = Number(settings.data?.score_medium_from ?? 4);
  const high = Number(settings.data?.score_high_from ?? 8);

  return (
    <PageStack>
      <Nav />
      <PageHead title={row ? `${text(row.number)} ${text(row.title)}`.trim() : t("Plan a change")} back={{ to: "/e/ItChange", label: t("Changes") }} description={row ? t("A draft: nothing is asked of anyone until it is submitted.") : undefined} />
      {!row && <FromTemplate />}
      <PageSplit
        main={
          <form className="stack" onSubmit={press(false)} noValidate>
            <section className="card card-pad stack" aria-labelledby="itsm-ch-what">
              <h2 id="itsm-ch-what" className="section-title">
                {t("1. What changes")}
              </h2>
              <Field label={t("Title")} htmlFor="itsm-ch-title" required error={said.title}>
                <input id="itsm-ch-title" className="input" value={d.title} onChange={(e) => set("title", e.target.value)} />
              </Field>
              <Field label={t("Type")} htmlFor="itsm-ch-type" error={said.change_type} hint={type ? isolate(text(type.description)) : undefined}>
                <select id="itsm-ch-type" className="select" value={d.change_type ?? ""} onChange={(e) => set("change_type", e.target.value || null)}>
                  <option value="">{t("Chosen from its class")}</option>
                  {(types.data ?? []).map((x) => (
                    <option key={x.id} value={x.id}>
                      {isolate(`${text(x.type_name)} (${text(x.path)})`)}
                    </option>
                  ))}
                </select>
              </Field>
              <Field label={t("Item it changes")} htmlFor="itsm-ch-ci" required error={said.ci}>
                <LinkInput id="itsm-ch-ci" to="ConfigurationItem" value={d.ci} title={row ? titleOf(row, "ci") : undefined} onChange={(v) => set("ci", v)} />
              </Field>
              <Field label={t("Also affects")} htmlFor="itsm-ch-also" hint={t("Other services and items the change touches: a change on any of them in the same window is flagged.")}>
                <div className="stack">
                  {d.affected.length > 0 && (
                    <ul className="stack">
                      {d.affected.map((id) => (
                        <Chip key={id} id={id} onRemove={() => set("affected", d.affected.filter((x) => x !== id))} />
                      ))}
                    </ul>
                  )}
                  <LinkInput id="itsm-ch-also" to="ConfigurationItem" value={null} placeholder={t("Add a service or item")} onChange={(v) => v && !d.affected.includes(v) && v !== d.ci && set("affected", [...d.affected, v])} />
                </div>
              </Field>
            </section>

            <section className="card card-pad stack" aria-labelledby="itsm-ch-risk">
              <h2 id="itsm-ch-risk" className="section-title">
                {t("2. Risk and impact")}
              </h2>
              <Refused error={questions.error} />
              {questions.isPending && <FormSkeleton fields={3} />}
              {questions.data && !questions.data.length && <p className="muted">{t("The workspace has no risk questions: pick the risk by hand.")}</p>}
              {(questions.data ?? []).map((q) => (
                <fieldset key={q.id} className="stack">
                  <legend>{isolate(text(q.question_text))}</legend>
                  {(Array.isArray(q.choices) ? (q.choices as Row[]) : []).map((c) => (
                    <label key={text(c.choice_label)} className="check-field">
                      <input type="radio" name={`q-${q.id}`} checked={d.answers[text(q.question_text)] === text(c.choice_label)} onChange={() => set("answers", { ...d.answers, [text(q.question_text)]: text(c.choice_label) })} />
                      {isolate(text(c.choice_label))} <span className="muted">{t("score {n}", { n: Number(c.score ?? 0) })}</span>
                    </label>
                  ))}
                  <span className="muted">{text(q.measures) === "Impact" ? t("Measures impact") : t("Measures risk")}</span>
                </fieldset>
              ))}
              {said.risk_answers && <p className="field-error">{isolate(said.risk_answers)}</p>}
              {!answered && (
                <Field label={t("Risk, by hand")} htmlFor="itsm-ch-risklevel" hint={t("Answer the questions and it is worked out from them.")}>
                  <select id="itsm-ch-risklevel" className="select" value={d.risk} onChange={(e) => set("risk", e.target.value as Level)}>
                    {LEVELS.map((l) => (
                      <option key={l}>{l}</option>
                    ))}
                  </select>
                </Field>
              )}
            </section>

            <section className="card card-pad stack" aria-labelledby="itsm-ch-plans">
              <h2 id="itsm-ch-plans" className="section-title">
                {t("3. Plans")}
              </h2>
              <Field label={t("Plan")} htmlFor="itsm-ch-plan" error={said.change_plan}>
                <textarea id="itsm-ch-plan" className="textarea" rows={4} value={d.change_plan} onChange={(e) => set("change_plan", e.target.value)} />
              </Field>
              <Field label={t("Back-out plan")} htmlFor="itsm-ch-back" required={!!type && type.path !== "Pre-approved"} error={said.backout_plan}>
                <textarea id="itsm-ch-back" className="textarea" rows={3} value={d.backout_plan} onChange={(e) => set("backout_plan", e.target.value)} />
              </Field>
              <Field label={t("Test plan")} htmlFor="itsm-ch-test" error={said.test_plan}>
                <textarea id="itsm-ch-test" className="textarea" rows={3} value={d.test_plan} onChange={(e) => set("test_plan", e.target.value)} />
              </Field>
            </section>

            <section className="card card-pad stack" aria-labelledby="itsm-ch-window">
              <h2 id="itsm-ch-window" className="section-title">
                {t("4. Window")}
              </h2>
              <div className="form-grid">
                <Field label={t("Starts")} htmlFor="itsm-ch-start" required error={said.planned_start}>
                  <input id="itsm-ch-start" className="input" type="datetime-local" value={d.planned_start} onChange={(e) => set("planned_start", e.target.value)} />
                </Field>
                <Field label={t("Ends")} htmlFor="itsm-ch-end" required error={said.planned_end}>
                  <input id="itsm-ch-end" className="input" type="datetime-local" value={d.planned_end} onChange={(e) => set("planned_end", e.target.value)} />
                </Field>
              </div>
              {lines.map((l) => (
                <Alert key={l} tone={inFreeze(l) && refuses ? "error" : "warning"}>
                  {isolate(l)}
                  {inFreeze(l) && refuses ? <> · {t("only an emergency change may be planned in a freeze; saving will be refused")}</> : null}
                </Alert>
              ))}
            </section>

            <Refused error={loose} />
            <div className="row">
              <Button variant="primary" type="submit" loading={save.isPending && !save.variables} disabled={save.isPending}>
                {t("Save draft")}
              </Button>
              <Button onClick={() => press(true)()} loading={save.isPending && !!save.variables} disabled={save.isPending}>
                {t("Save and submit")}
              </Button>
            </div>
          </form>
        }
        aside={
          <section className="card card-pad stack" aria-labelledby="itsm-ch-gives" aria-live="polite">
            <h2 id="itsm-ch-gives" className="section-title">
              {t("What this gives")}
            </h2>
            {!d.ci && <p className="muted">{t("Name the item it changes, and the risk, the route and the collisions are worked out here.")}</p>}
            {calc.isError && <Refused error={calc.error} />}
            {derived && (
              <>
                <dl className="stack">
                  <div>
                    <dt className="muted">{t("Risk")}</dt>
                    <dd>
                      <Badge tone={derived.risk === "High" ? "red" : derived.risk === "Medium" ? "amber" : "green"}>{isolate(text(derived.risk))}</Badge>{" "}
                      {derived.risk_score != null && <span className="muted">{t("score {n}", { n: Number(derived.risk_score) })}</span>}
                    </dd>
                  </div>
                  {derived.impact_level != null && (
                    <div>
                      <dt className="muted">{t("Impact")}</dt>
                      <dd>
                        <Badge tone={derived.impact_level === "High" ? "red" : derived.impact_level === "Medium" ? "amber" : "green"}>{isolate(text(derived.impact_level))}</Badge>{" "}
                        <span className="muted">{t("score {n}", { n: Number(derived.impact_score ?? 0) })}</span>
                      </dd>
                    </div>
                  )}
                </dl>
                {answered && <p className="muted">{t("Medium from {medium}, High from {high} (Settings).", { medium, high })}</p>}
                {answered && derived.risk_score != null && <p className="faint">{t("A score of {n} is {level}.", { n: Number(derived.risk_score), level: levelOf(Number(derived.risk_score), medium, high) })}</p>}
                <div className="stack">
                  <strong>{t("Who approves it")}</strong>
                  <p className="muted">
                    <Badge tone="blue">{isolate(text(derived.route))}</Badge>
                  </p>
                  {routeLines(derived, type).map((l) => (
                    <p key={l}>{isolate(l)}</p>
                  ))}
                </div>
                {derived.impacted ? (
                  <div className="stack">
                    <strong>{t("What depends on the item")}</strong>
                    <p className="muted">{isolate(text(derived.impacted))}</p>
                  </div>
                ) : null}
              </>
            )}
          </section>
        }
      />
    </PageStack>
  );
}

// ── a change submitted ──

/** The change's approval and what a person may do with it next. */
function View({ row }: { row: Row }) {
  const qc = useQueryClient();
  const rights = useRights("ItChange");
  const write = !!rights && rights.write !== "none";
  const id = row.id;
  const status = text(row.status);
  const route = text(row.route);
  const asked = useAsked("ItChange", id);
  const [ask, setAsk] = useState<"reject" | "review" | "close" | "fail" | null>(null);
  const refresh = () => void qc.invalidateQueries({ queryKey: ["itsm"] }).then(() => qc.invalidateQueries({ queryKey: ["approvals"] }));
  const act = useMutation({ mutationFn: (action: string) => run("ItChange", id, action), onSuccess: refresh });
  const board = route === "Board" || route === "Emergency";
  const pending = asked.data?.status === "pending";
  const reviewed = text(row.review_outcome);
  const mustReview = row.review_required === true && !reviewed;
  const also = Array.isArray(row.affected) ? (row.affected as Row[]) : [];
  const lines = flagLines(row.window_flags);

  const asking: Record<string, { title: string; params: Param[]; confirm: string; action: string; danger?: boolean }> = {
    reject: { title: t("Reject the change"), params: [{ name: "note", label: t("Why"), kind: "text", required: true }], confirm: t("Reject it"), action: "reject_change", danger: true },
    fail: { title: t("The change failed"), params: [{ name: "note", label: t("What happened"), kind: "text", required: true }], confirm: t("It failed"), action: "fail_change", danger: true },
    close: { title: t("Close the change"), params: [{ name: "note", label: t("Review"), kind: "text" }], confirm: t("Close it"), action: "close_change" },
    review: {
      title: t("Review the change"),
      params: [
        { name: "outcome", label: t("Outcome"), kind: "select", options: ["Successful", "Successful with Issues", "Failed"], required: true },
        { name: "note", label: t("What we learned"), kind: "text", hint: t("A failed change says what happened.") },
        { name: "open_incident", label: t("Open an incident for it"), kind: "check" },
      ],
      confirm: t("Save the review"),
      action: "review_change",
    },
  };
  const modal = ask ? asking[ask] : null;

  return (
    <PageStack>
      <Nav />
      <PageHead
        title={`${text(row.number)} ${text(row.title)}`.trim()}
        back={{ to: "/e/ItChange", label: t("Changes") }}
        description={<><StatusBadge value={status} /> {isolate([titleOf(row, "change_type"), route && t("route {route}", { route })].filter(Boolean).join(" · "))}</>}
        actions={
          write ? (
            <>
              {status === "Submitted" && route === "Manager" && (
                <>
                  <Button variant="primary" loading={act.isPending} disabled={act.isPending} onClick={() => act.mutate("approve_change")}>
                    {t("Approve")}
                  </Button>
                  <Button onClick={() => setAsk("reject")}>{t("Reject")}</Button>
                </>
              )}
              {status === "Submitted" && board && !pending && asked.data?.status !== "approved" && (
                <Button variant="primary" loading={act.isPending} disabled={act.isPending} onClick={() => act.mutate("approve_change")}>
                  {route === "Emergency" ? t("Ask the emergency group") : t("Ask the board")}
                </Button>
              )}
              {status === "Submitted" && board && asked.data?.status === "rejected" && <Button onClick={() => setAsk("reject")}>{t("Reject it")}</Button>}
              {status === "Approved" && (
                <Button variant="primary" loading={act.isPending} disabled={act.isPending} onClick={() => act.mutate("implement")}>
                  {t("Mark as done")}
                </Button>
              )}
              {status === "Implemented" && (
                <>
                  <Button variant="primary" onClick={() => setAsk("review")}>
                    {t("Review it")}
                  </Button>
                  {!mustReview && <Button onClick={() => setAsk("close")}>{t("Close it")}</Button>}
                  <Button onClick={() => setAsk("fail")}>{t("It failed")}</Button>
                </>
              )}
            </>
          ) : undefined
        }
      />
      <Refused error={act.error} />
      {status === "Implemented" && mustReview && <Alert tone="info">{t("A change of this type is reviewed before it is closed.")}</Alert>}
      {lines.map((l) => (
        <Alert key={l} tone="warning">
          {isolate(l)}
        </Alert>
      ))}
      <PageSplit
        main={
          <>
            {asked.isError && <Refused error={asked.error} />}
            {asked.data && <ApprovalState asked={asked.data} onChanged={refresh} />}
            {status === "Submitted" && !asked.data && !asked.isPending && (
              <Alert tone="info">
                {route === "Manager" ? t("One Change Manager, other than who raised it, approves it.") : route === "Pre-approved" ? t("It is approved in advance.") : t("No one has been asked yet: press Ask the board.")}
              </Alert>
            )}
            <section className="card card-pad stack" aria-labelledby="itsm-v-plans">
              <h2 id="itsm-v-plans" className="section-title">
                {t("Plans")}
              </h2>
              {(
                [
                  [t("Plan"), row.change_plan],
                  [t("Back-out plan"), row.backout_plan],
                  [t("Test plan"), row.test_plan],
                ] as const
              ).map(([label, v]) => (
                <div key={label}>
                  <strong>{label}</strong>
                  <p className="muted">{v ? isolate(text(v)) : t("None written")}</p>
                </div>
              ))}
            </section>
            {(reviewed || row.implemented_at) && (
              <section className="card card-pad stack" aria-labelledby="itsm-v-review">
                <h2 id="itsm-v-review" className="section-title">
                  {t("After it was done")}
                </h2>
                {row.implemented_at ? <p>{t("Implemented {at}", { at: when(row.implemented_at) })}</p> : null}
                {reviewed ? (
                  <p>
                    <StatusBadge value={reviewed} /> {isolate(text(row.review_notes))}
                    <span className="muted"> · {isolate(text(row.reviewed_by))} {when(row.reviewed_at)}</span>
                  </p>
                ) : null}
              </section>
            )}
          </>
        }
        aside={
          <section className="card card-pad stack" aria-labelledby="itsm-v-facts">
            <h2 id="itsm-v-facts" className="section-title">
              {t("The change")}
            </h2>
            <dl className="stack">
              <div>
                <dt className="muted">{t("Item it changes")}</dt>
                <dd>{row.ci ? <ItemLink id={text(row.ci)} /> : "—"}</dd>
              </div>
              {also.length > 0 && (
                <div>
                  <dt className="muted">{t("Also affects")}</dt>
                  <dd className="stack">
                    {also.map((a) => (
                      <ItemLink key={text(a.item)} id={text(a.item)} />
                    ))}
                  </dd>
                </div>
              )}
              <div>
                <dt className="muted">{t("Window")}</dt>
                <dd>{windowWords(row.planned_start, row.planned_end)}</dd>
              </div>
              <div>
                <dt className="muted">{t("Risk")}</dt>
                <dd>
                  {isolate(text(row.risk))} {row.risk_score != null && <span className="muted">{t("score {n}", { n: Number(row.risk_score) })}</span>}
                </dd>
              </div>
              {row.impact_level ? (
                <div>
                  <dt className="muted">{t("Impact")}</dt>
                  <dd>
                    {isolate(text(row.impact_level))} <span className="muted">{t("score {n}", { n: Number(row.impact_score ?? 0) })}</span>
                  </dd>
                </div>
              ) : null}
              {row.impacted ? (
                <div>
                  <dt className="muted">{t("What depends on the item")}</dt>
                  <dd>{isolate(text(row.impacted))}</dd>
                </div>
              ) : null}
              {row.decision_note ? (
                <div>
                  <dt className="muted">{t("Decision")}</dt>
                  <dd>{isolate(text(row.decision_note))}</dd>
                </div>
              ) : null}
            </dl>
            <Link to={`/e/ItChange/${id}`}>{t("Open the whole record")}</Link>
          </section>
        }
      />
      {modal && (
        <Ask
          title={modal.title}
          params={modal.params}
          confirm={modal.confirm}
          danger={modal.danger}
          onSubmit={(b) => run("ItChange", id, modal.action, b)}
          onClose={() => setAsk(null)}
          onDone={() => (setAsk(null), refresh())}
        />
      )}
    </PageStack>
  );
}

export function ChangeRoute() {
  const { id = "new" } = useParams();
  const one = useQuery({ queryKey: ["itsm", "change", id], queryFn: () => record("ItChange", id), enabled: id !== "new" });
  if (id === "new") return <Editor />;
  return (
    <Await
      q={one}
      skeleton={
        <PageStack>
          <Nav />
          <FormSkeleton fields={6} />
        </PageStack>
      }
    >
      {(row) => (text(row.status) === "Draft" ? <Editor key={`${row.id}-${row.version}`} row={row} /> : <View row={row} />)}
    </Await>
  );
}

