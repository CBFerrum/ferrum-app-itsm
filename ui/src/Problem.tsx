// Problems and known errors: the list by what a problem is (open, known error, resolved) with its incidents counted and the major
// incidents running; and a problem's page: the root cause and workaround it is kept with, who sees the workaround once it is
// published, the incidents linked to it (linked and unlinked here, and resolved with it), each in the server's own refusals.

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { type FormEvent, useState } from "react";
import { Link, useParams, useSearchParams } from "react-router";
import { ApiError, isolate, t } from "@ferrum/client";
import { Alert, Badge, Button, ButtonLink, Field, FormSkeleton, PageHead, PageSplit, PageStack, Queue, QueueGrid, QueueItem, ScrollRegion, StatePanel, StatusBadge, TableSkeleton, toast } from "@ferrum/kit";
import { LinkInput } from "@ferrum/records";
import { change, record, rows, run, type Row, text, titleOf } from "./data";
import { priorityTone } from "./desk";
import { bodyOfProblem, draftOf, FILTERS, offersResolve, openByProblem, OPEN_INCIDENT, showOf, stillOpen, type ProblemDraft, type Show } from "./problems";
import { Ask, Await, Nav, Refused, useRights, useSettings } from "./ui";

// ── the list ──

export function ProblemsRoute() {
  const [params, setParams] = useSearchParams();
  const show = showOf(params.get("show"));
  const rights = useRights("Problem");
  const list = useQuery({
    queryKey: ["itsm", "problems", show],
    queryFn: async () => {
      const [problems, linked, major] = await Promise.all([
        rows("Problem", FILTERS[show], { titles: true, sort: "created_at desc" }),
        rows("Incident", [["problem", "is", "set"], ["status", "in", OPEN_INCIDENT]], { fields: "problem" }).catch(() => [] as Row[]),
        rows("Incident", [["major", "=", true], ["status", "in", [...OPEN_INCIDENT, "Awaiting Caller"]]]).catch(() => [] as Row[]),
      ]);
      return { problems, open: openByProblem(linked), major };
    },
  });
  const tabs: [Show, string][] = [["open", t("Open")], ["known", t("Known errors")], ["resolved", t("Resolved")]];
  return (
    <PageStack>
      <Nav />
      <PageHead
        title={t("Problems and known errors")}
        description={t("The cause behind incidents, and the workaround people use until it is fixed.")}
        actions={rights && rights.create !== "none" ? <ButtonLink to="/e/Problem/new" variant="primary" icon="plus">{t("New problem")}</ButtonLink> : undefined}
      />
      <Await q={list} skeleton={<TableSkeleton columns={5} rows={5} />}>
        {({ problems, open, major }) => (
          <>
            {major.length > 0 && (
              <QueueGrid columns={1}>
                <Queue title={t("Major incidents now")} count={major.length} tone="attention">
                  {major.map((m) => (
                    <QueueItem key={m.id} to={`/major/${m.id}`} title={isolate(text(m.title))} meta={isolate(text(m.number))} trailing={<Badge tone={priorityTone(m.priority)}>{isolate(text(m.priority))}</Badge>} />
                  ))}
                </Queue>
              </QueueGrid>
            )}
            <div className="segmented" role="group" aria-label={t("Show")}>
              {tabs.map(([id, label]) => (
                <button key={id} type="button" aria-pressed={show === id} onClick={() => setParams({ show: id }, { replace: true })}>
                  {label}
                </button>
              ))}
            </div>
            {problems.length === 0 ? (
              <StatePanel kind="empty" title={show === "known" ? t("No known errors") : show === "resolved" ? t("No resolved problems") : t("No open problems")}>
                {t("A problem is raised when incidents share a cause; a known error is one whose workaround is known.")}
              </StatePanel>
            ) : (
              <ScrollRegion label={t("Problems")}>
                <table className="table">
                  <thead>
                    <tr>
                      <th>{t("Problem")}</th>
                      <th>{t("Item")}</th>
                      <th>{t("Status")}</th>
                      <th>{t("Open incidents")}</th>
                    </tr>
                  </thead>
                  <tbody>
                    {problems.map((p) => (
                      <tr key={p.id}>
                        <td>
                          <Link to={`/problem/${p.id}`}>{isolate(`${text(p.number)} ${text(p.title)}`.trim())}</Link>
                        </td>
                        <td>{isolate(titleOf(p, "ci"))}</td>
                        <td>
                          <StatusBadge value={p.status} /> {p.known_error === true && <Badge tone="purple">{t("Known error")}</Badge>}
                        </td>
                        <td className="num">{open.get(p.id) ?? 0}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </ScrollRegion>
            )}
          </>
        )}
      </Await>
    </PageStack>
  );
}

// ── a problem ──

function Form({ problem }: { problem: Row }) {
  const qc = useQueryClient();
  const [d, setD] = useState<ProblemDraft>(() => draftOf(problem));
  const set = <K extends keyof ProblemDraft>(k: K, v: ProblemDraft[K]) => setD((cur) => ({ ...cur, [k]: v }));
  const save = useMutation({
    mutationFn: () => change("Problem", problem.id, bodyOfProblem(d), problem.version),
    onSuccess: () => {
      toast(t("Saved"));
      void qc.invalidateQueries({ queryKey: ["itsm"] });
    },
  });
  // The server names the field it refused: its words go under it.
  const said = (field: string) => (save.error instanceof ApiError ? save.error.fields.find((f) => f.field === field)?.message : undefined);
  const loose = save.error instanceof ApiError && save.error.fields.length && save.error.fields.every((f) => f.field in d) ? null : save.error;
  const known = d.known_error || d.status === "Known Error";
  const submit = (e: FormEvent) => {
    e.preventDefault();
    if (!save.isPending) save.mutate();
  };
  return (
    <form className="card card-pad stack" onSubmit={submit} noValidate aria-labelledby="itsm-pb-form">
      <h2 id="itsm-pb-form" className="section-title">
        {t("The problem")}
      </h2>
      <Field label={t("Title")} htmlFor="itsm-pb-title" required error={said("title")}>
        <input id="itsm-pb-title" className="input" value={d.title} onChange={(e) => set("title", e.target.value)} />
      </Field>
      <Field label={t("Status")} htmlFor="itsm-pb-status">
        <select id="itsm-pb-status" className="select" value={d.status} onChange={(e) => set("status", e.target.value)}>
          {["Open", "Known Error", "Resolved", "Closed"].map((s) => (
            <option key={s}>{s}</option>
          ))}
        </select>
      </Field>
      <Field label={t("Symptoms")} htmlFor="itsm-pb-symptoms" hint={t("What people see: the words they would search for. Published with the workaround.")} error={said("symptoms")}>
        <textarea id="itsm-pb-symptoms" className="textarea" rows={2} value={d.symptoms} onChange={(e) => set("symptoms", e.target.value)} />
      </Field>
      <Field label={t("Root cause")} htmlFor="itsm-pb-cause" required={["Resolved", "Closed"].includes(d.status)} error={said("root_cause")}>
        <textarea id="itsm-pb-cause" className="textarea" rows={3} value={d.root_cause} onChange={(e) => set("root_cause", e.target.value)} />
      </Field>
      <label className="check-field">
        <input type="checkbox" checked={d.known_error} onChange={(e) => set("known_error", e.target.checked)} />
        {t("This is a known error: the cause is known and there is a workaround")}
      </label>
      <Field label={t("Workaround")} htmlFor="itsm-pb-work" required={known} error={said("workaround")}>
        <textarea id="itsm-pb-work" className="textarea" rows={3} value={d.workaround} onChange={(e) => set("workaround", e.target.value)} />
      </Field>
      <div className="stack">
        <label className="check-field">
          <input type="checkbox" checked={d.workaround_visible} onChange={(e) => set("workaround_visible", e.target.checked)} />
          {t("Publish the workaround")}
        </label>
        {said("workaround_visible") && <p className="field-error">{isolate(said("workaround_visible") ?? "")}</p>}
      </div>
      <Refused error={loose} />
      <div className="row">
        <Button variant="primary" type="submit" loading={save.isPending}>
          {t("Save")}
        </Button>
      </div>
    </form>
  );
}

/** Who finds the workaround, and as what: the known error record the problem keeps in step. */
function Published({ problem, known }: { problem: Row; known: Row | null }) {
  const will = problem.workaround_visible === true && (problem.known_error === true || problem.status === "Known Error");
  return (
    <section className="card card-pad stack" aria-labelledby="itsm-pb-pub">
      <h2 id="itsm-pb-pub" className="section-title">
        {t("Published workaround")}
      </h2>
      {known?.published === true ? (
        <>
          <Badge tone="green">{t("Published")}</Badge>
          <p className="muted">{t("The service desk finds it by search, and people reporting an incident or asking from the catalog are shown it as they type.")}</p>
          <p>
            <strong>{isolate(text(known.title))}</strong>
          </p>
          {known.symptoms ? <p className="muted">{isolate(text(known.symptoms))}</p> : null}
          <p>{isolate(text(known.workaround))}</p>
        </>
      ) : (
        <>
          <Badge tone="neutral">{known ? t("Withdrawn") : t("Not published")}</Badge>
          <p className="muted">{will ? t("It is published once you save.") : t("Mark a known error's workaround as published and people see it as they type their problem.")}</p>
        </>
      )}
    </section>
  );
}

function Linked({ problem, incidents }: { problem: Row; incidents: Row[] }) {
  const qc = useQueryClient();
  const rights = useRights("Incident");
  const write = !!rights && rights.write !== "none";
  const settings = useSettings();
  const [picked, setPicked] = useState<string | null>(null);
  const [resolving, setResolving] = useState(false);
  const refresh = () => void qc.invalidateQueries({ queryKey: ["itsm"] });
  const link = useMutation({ mutationFn: ({ id, to }: { id: string; to: string | null }) => change("Incident", id, { problem: to }), onSuccess: () => (setPicked(null), refresh()) });
  const open = stillOpen(incidents);
  const auto = settings.data && settings.data.problem_closes_incidents !== "Offer";
  return (
    <section className="card card-pad stack" aria-labelledby="itsm-pb-inc">
      <h2 id="itsm-pb-inc" className="section-title">
        {t("Incidents linked to it")}
      </h2>
      <Refused error={link.error} />
      {incidents.length === 0 ? (
        <p className="muted">{t("No incident is linked to this problem yet.")}</p>
      ) : (
        <ScrollRegion label={t("Incidents linked to it")}>
          <table className="table">
            <thead>
              <tr>
                <th>{t("Incident")}</th>
                <th>{t("Priority")}</th>
                <th>{t("Status")}</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {incidents.map((i) => (
                <tr key={i.id}>
                  <td>
                    <Link to={`/e/Incident/${i.id}`}>{isolate(`${text(i.number)} ${text(i.title)}`.trim())}</Link>
                  </td>
                  <td>
                    <Badge tone={priorityTone(i.priority)}>{isolate(text(i.priority))}</Badge>
                  </td>
                  <td>
                    <StatusBadge value={i.status} />
                  </td>
                  <td>
                    {write && (
                      <Button size="sm" variant="ghost" disabled={link.isPending} onClick={() => link.mutate({ id: i.id, to: null })}>
                        {t("Unlink")}
                      </Button>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </ScrollRegion>
      )}
      {write && (
        <div className="row">
          <Field label={t("Link an incident")} htmlFor="itsm-pb-link">
            <LinkInput id="itsm-pb-link" to="Incident" value={picked} onChange={setPicked} />
          </Field>
          <Button disabled={!picked || link.isPending} loading={link.isPending} onClick={() => picked && link.mutate({ id: picked, to: text(problem.id) })}>
            {t("Link it")}
          </Button>
        </div>
      )}
      {["Resolved", "Closed"].includes(text(problem.status)) && (
        <div className="stack">
          {auto && <p className="muted">{t("Resolving a problem resolves its open incidents with its root cause (Settings, Closing a problem).")}</p>}
          {offersResolve(problem, incidents) && write && (
            <div className="row">
              <Button variant="primary" onClick={() => setResolving(true)}>
                {t("Resolve its incidents")}
              </Button>
              <span className="muted">{t("{n} still open", { n: open.length })}</span>
            </div>
          )}
        </div>
      )}
      {resolving && (
        <Ask
          title={t("Resolve the incidents of this problem")}
          description={t("The {n} incidents still open under it are resolved with its root cause: {cause}", { n: open.length, cause: text(problem.root_cause) })}
          params={[]}
          confirm={t("Resolve them")}
          onSubmit={() => run("Problem", problem.id, "resolve_linked_incidents")}
          onClose={() => setResolving(false)}
          onDone={() => (setResolving(false), refresh(), toast(t("Resolved")))}
        />
      )}
    </section>
  );
}

export function ProblemRoute() {
  const { id = "" } = useParams();
  const q = useQuery({
    queryKey: ["itsm", "problem", id],
    queryFn: async () => {
      const [problem, incidents, known] = await Promise.all([
        record("Problem", id),
        rows("Incident", [["problem", "=", id]], { titles: true, sort: "created_at desc" }),
        rows("KnownError", [["problem", "=", id]]).catch(() => [] as Row[]),
      ]);
      return { problem, incidents, known: known[0] ?? null };
    },
  });
  return (
    <PageStack>
      <Nav />
      <Await q={q} skeleton={<FormSkeleton fields={6} />}>
        {({ problem, incidents, known }) => (
          <>
            <PageHead
              title={`${text(problem.number)} ${text(problem.title)}`.trim()}
              back={{ to: "/problems", label: t("Problems") }}
              description={
                <>
                  <StatusBadge value={problem.status} /> {problem.ci ? isolate(titleOf(problem, "ci")) : null}
                </>
              }
            />
            {text(problem.status) === "Known Error" && !problem.workaround && <Alert tone="warning">{t("A known error has its workaround: write it below.")}</Alert>}
            <PageSplit
              main={<Form key={`${problem.id}-${problem.version}`} problem={problem} />}
              aside={
                <>
                  <Published problem={problem} known={known} />
                  <Linked problem={problem} incidents={incidents} />
                </>
              }
            />
          </>
        )}
      </Await>
    </PageStack>
  );
}
