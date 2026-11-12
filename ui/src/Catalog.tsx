// The service catalog as a requester sees it (the Requester and the portal's IT Self Service read only the offerings, the known
// errors a problem has published and their own requests): the items, a search that shows the known problems that fit as it is typed,
// an item's form made of its questions, and a request with its approval, who decides, its answers and its fulfilment tasks, each
// task done by its group. What a role may not read (the tasks, to a portal person) is left out and said, not failed.

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { type FormEvent, useState } from "react";
import { Link, useNavigate, useParams } from "react-router";
import { isolate, t } from "@ferrum/client";
import { Alert, Badge, Button, Field, FormSkeleton, PageHead, PageSplit, PageStack, Queue, QueueItem, ScrollRegion, StatePanel, StatusBadge, TableSkeleton, Tile, TileGrid } from "@ferrum/kit";
import { make, record, rows, run, type Row, text, titleOf } from "./data";
import { when } from "./desk";
import { answerRows, asked, problems, questionsOf, search, stepsOf, taskOrder } from "./offering";
import { Ask, Await, Gaps, Nav, Refused, TargetBadge, useRights } from "./ui";
import { ApprovalState, useAsked } from "./Approval";
import { Suggest } from "./Suggest";

const status = (e: unknown) => (e as { status?: number } | null)?.status;

// ── the catalog ──

export function CatalogRoute() {
  const [q, setQ] = useState("");
  const items = useQuery({ queryKey: ["itsm", "offerings"], queryFn: () => rows("ServiceOffering", [], { titles: true, sort: "offering_name asc" }) });
  const mine = useQuery({ queryKey: ["itsm", "my-requests"], queryFn: () => rows("CatalogRequest", [], { titles: true, sort: "created_at desc", limit: 6 }) });
  return (
    <PageStack>
      <Nav />
      <PageHead title={t("Service catalog")} description={t("Ask IT for something. What you ask for is followed here.")} />
      <Field label={t("What do you need?")} htmlFor="itsm-cat-q">
        <input id="itsm-cat-q" className="input" type="search" value={q} onChange={(e) => setQ(e.target.value)} autoComplete="off" />
      </Field>
      <Suggest typed={q} />
      <Await q={items} skeleton={<TableSkeleton columns={3} rows={4} />}>
        {(all) => {
          const shown = search(all, q);
          return shown.length ? (
            <TileGrid min="md" label={t("Catalog items")}>
              {shown.map((o) => (
                <Tile
                  key={o.id}
                  to={`/catalog/${o.id}`}
                  title={text(o.offering_name)}
                  meta={
                    <>
                      {o.description ? <span>{isolate(text(o.description))}</span> : null}
                      <span>{t("Done within {n} working hours", { n: Number(o.fulfil_hours ?? 0) })}</span>
                    </>
                  }
                  status={o.needs_approval === true ? <Badge tone="amber">{t("Needs approval")}</Badge> : undefined}
                />
              ))}
            </TileGrid>
          ) : q.trim() ? (
            <StatePanel kind="no-matches" onClear={() => setQ("")} clearLabel={t("Clear the search")}>
              {t("Nothing in the catalog matches “{q}”.", { q: isolate(q.trim()) })}
            </StatePanel>
          ) : (
            <StatePanel kind="empty" title={t("The catalog is empty")}>
              {t("Items appear here once IT adds them.")}
            </StatePanel>
          );
        }}
      </Await>
      {mine.isError ? (
        status(mine.error) === 403 ? null : <Refused error={mine.error} />
      ) : mine.data ? (
        <Queue title={t("Your requests")} count={mine.data.length} more={{ to: "/e/CatalogRequest", count: mine.data.length - 5 }}>
          {mine.data.slice(0, 5).map((r) => (
            <QueueItem key={r.id} to={`/request/${r.id}`} title={isolate(titleOf(r, "offering") || text(r.number))} meta={isolate([text(r.number), when(r.due_at) && t("due {due}", { due: when(r.due_at) })].filter(Boolean).join(" · "))} trailing={<StatusBadge value={r.status} />} />
          ))}
        </Queue>
      ) : null}
    </PageStack>
  );
}

// ── an item ──

function Form({ offering }: { offering: Row }) {
  const go = useNavigate();
  const rights = useRights("CatalogRequest");
  const questions = questionsOf(offering);
  const [answers, setAnswers] = useState<Record<string, string>>({});
  const [onBehalf, setOnBehalf] = useState("");
  const [wrong, setWrong] = useState<Record<string, string>>({});
  const [turn, setTurn] = useState(0);
  const send = useMutation({
    mutationFn: () => make("CatalogRequest", { offering: offering.id, answers: answerRows(questions, answers), ...(onBehalf.trim() ? { caller_email: onBehalf.trim() } : {}) }),
    onSuccess: (made) => go(`/request/${made.id}`),
  });
  const set = (label: string, v: string) => (setAnswers((cur) => ({ ...cur, [label]: v })), setWrong({}));
  const submit = (e: FormEvent) => {
    e.preventDefault();
    if (send.isPending) return;
    const bad = problems(questions, answers);
    if (Object.keys(bad).length) return (setWrong(bad), setTurn((n) => n + 1));
    send.mutate();
  };
  const shown = questions.filter((q) => asked(q, answers));
  const gaps = shown.flatMap((q, i) => (wrong[q.label] ? [{ id: `itsm-of-${i}`, label: q.label, message: wrong[q.label] }] : []));
  const steps = stepsOf(offering);
  const staff = !!rights && rights.write !== "none";
  return (
    <PageSplit
      main={
        <form className="card card-pad stack" onSubmit={submit} noValidate aria-labelledby="itsm-of-form">
          <h2 id="itsm-of-form" className="section-title">
            {t("Your request")}
          </h2>
          <Gaps gaps={gaps} turn={turn} />
          {questions.length === 0 && <p className="muted">{t("Nothing more is asked: press Ask for it.")}</p>}
          {shown.map((q, i) => {
            const id = `itsm-of-${i}`;
            const v = answers[q.label] ?? "";
            return (
              <div key={q.label} className="stack">
                <Field label={isolate(q.label)} htmlFor={id} required={q.required} error={wrong[q.label]}>
                  {q.qtype === "Choice" || q.qtype === "Yes/No" ? (
                    <select id={id} className="select" value={v} onChange={(e) => set(q.label, e.target.value)}>
                      <option value="">{t("Choose…")}</option>
                      {(q.qtype === "Yes/No" ? ([["Yes", t("Yes")], ["No", t("No")]] as const) : q.options.map((o) => [o, o] as const)).map(([o, word]) => (
                        <option key={o} value={o}>
                          {isolate(word)}
                        </option>
                      ))}
                    </select>
                  ) : q.qtype === "Date" ? (
                    <input id={id} className="input" type="date" value={v} onChange={(e) => set(q.label, e.target.value)} />
                  ) : q.qtype === "Number" ? (
                    <input id={id} className="input" type="number" value={v} onChange={(e) => set(q.label, e.target.value)} />
                  ) : (
                    <textarea id={id} className="textarea" rows={2} value={v} onChange={(e) => set(q.label, e.target.value)} />
                  )}
                </Field>
                {q.qtype === "Text" && <Suggest typed={v} />}
              </div>
            );
          })}
          {staff && (
            <Field label={t("On behalf of")} htmlFor="itsm-of-who" hint={t("An address; left empty, the request is yours.")}>
              <input id="itsm-of-who" className="input" type="email" value={onBehalf} onChange={(e) => setOnBehalf(e.target.value)} />
            </Field>
          )}
          <Refused error={send.error} />
          <div className="row">
            <Button variant="primary" type="submit" loading={send.isPending}>
              {t("Ask for it")}
            </Button>
          </div>
        </form>
      }
      aside={
        <section className="card card-pad stack" aria-labelledby="itsm-of-next">
          <h2 id="itsm-of-next" className="section-title">
            {t("What happens next")}
          </h2>
          <ol className="stack">
            {offering.needs_approval === true && (
              <li>
                {t("It is approved first")}
                {Number(offering.approvals_needed ?? 1) > 1 || offering.approver_group ? <span className="muted"> · {t("{n} of the approving group", { n: Number(offering.approvals_needed ?? 1) })}</span> : null}
              </li>
            )}
            {steps.map((s) => (
              <li key={s.name}>
                {isolate(s.name)} <span className="muted">{t("within {n} working hours", { n: s.hours })}</span>
              </li>
            ))}
            {!steps.length && <li>{t("IT works on it, within {n} working hours.", { n: Number(offering.fulfil_hours ?? 0) })}</li>}
          </ol>
        </section>
      }
    />
  );
}

export function OfferingRoute() {
  const { id = "" } = useParams();
  const q = useQuery({ queryKey: ["itsm", "offering", id], queryFn: () => record("ServiceOffering", id) });
  return (
    <PageStack>
      <Nav />
      <Await q={q} skeleton={<FormSkeleton fields={4} />}>
        {(offering) => (
          <>
            <PageHead title={text(offering.offering_name)} back={{ to: "/catalog", label: t("Service catalog") }} description={offering.description ? isolate(text(offering.description)) : undefined} />
            {offering.active === false ? <Alert tone="warning">{t("This item is not offered any more.")}</Alert> : <Form offering={offering} />}
          </>
        )}
      </Await>
    </PageStack>
  );
}

// ── a request ──

function Tasks({ request, tasks }: { request: Row; tasks: Row[] | null }) {
  const qc = useQueryClient();
  const rights = useRights("RequestTask");
  const write = !!rights && rights.write !== "none";
  const done = useMutation({ mutationFn: (id: string) => run("RequestTask", id, "complete_task"), onSuccess: () => void qc.invalidateQueries({ queryKey: ["itsm"] }) });
  return (
    <section className="card card-pad stack" aria-labelledby="itsm-rq-tasks">
      <h2 id="itsm-rq-tasks" className="section-title">
        {t("Fulfilment tasks")}
      </h2>
      <Refused error={done.error} />
      {tasks === null ? (
        <p className="muted">{t("The tasks are IT's to see: you are told when the request is done.")}</p>
      ) : tasks.length === 0 ? (
        <p className="muted">{["Submitted", "Awaiting Caller"].includes(text(request.status)) ? t("Its tasks are made once it is approved.") : t("It has no tasks.")}</p>
      ) : (
        <ScrollRegion label={t("Fulfilment tasks")}>
          <table className="table">
            <thead>
              <tr>
                <th>{t("Step")}</th>
                <th>{t("Group")}</th>
                <th>{t("Status")}</th>
                <th>{t("Due")}</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {taskOrder(tasks).map((k) => (
                <tr key={k.id}>
                  <td>{isolate(text(k.step_name))}</td>
                  <td>{isolate(titleOf(k, "assignment_group"))}</td>
                  <td>
                    <StatusBadge value={k.status} /> {text(k.status) === "Waiting" && <span className="muted">{t("waits for the step before")}</span>}
                  </td>
                  <td>
                    {k.due_by ? when(k.due_by) : ""} <TargetBadge state={k.task_state} />
                  </td>
                  <td>
                    {write && text(k.status) === "Open" && (
                      <Button size="sm" variant="primary" loading={done.isPending && done.variables === k.id} disabled={done.isPending} onClick={() => done.mutate(k.id)}>
                        {t("Done")}
                      </Button>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </ScrollRegion>
      )}
    </section>
  );
}

export function RequestRoute() {
  const { id = "" } = useParams();
  const qc = useQueryClient();
  const rights = useRights("CatalogRequest");
  const write = !!rights && rights.write !== "none";
  const [reject, setReject] = useState(false);
  const q = useQuery({
    queryKey: ["itsm", "request", id],
    queryFn: async () => {
      const request = await record("CatalogRequest", id);
      const [offering, tasks] = await Promise.all([
        record("ServiceOffering", text(request.offering)).catch(() => null),
        // A person who may not read the tasks (a portal person) is told so, and the rest of the page is shown.
        rows("RequestTask", [["request", "=", id]], { titles: true, sort: "sequence asc" }).catch((e: unknown) => (status(e) === 403 ? null : Promise.reject(e))),
      ]);
      return { request, offering, tasks };
    },
  });
  const group = text(q.data?.request.route) === "Group";
  const asked = useAsked("CatalogRequest", id);
  const refresh = () => void qc.invalidateQueries({ queryKey: ["itsm"] }).then(() => qc.invalidateQueries({ queryKey: ["approvals"] }));
  const act = useMutation({ mutationFn: (action: string) => run("CatalogRequest", id, action), onSuccess: refresh });
  return (
    <PageStack>
      <Nav />
      <Await q={q} skeleton={<FormSkeleton fields={5} />}>
        {({ request, offering, tasks }) => {
          const st = text(request.status);
          const needs = offering?.needs_approval === true;
          const answers = Array.isArray(request.answers) ? (request.answers as Row[]) : [];
          const pending = asked.data?.status === "pending";
          return (
            <>
              <PageHead
                title={`${text(request.number)} ${titleOf(request, "offering")}`.trim()}
                back={{ to: "/catalog", label: t("Service catalog") }}
                description={
                  <>
                    <StatusBadge value={st} /> <TargetBadge state={request.fulfil_state} label={t("Fulfilment")} /> {request.due_at ? t("due {due}", { due: when(request.due_at) }) : null}
                  </>
                }
                actions={
                  write ? (
                    <>
                      {st === "Submitted" && needs && !group && (
                        <>
                          <Button variant="primary" loading={act.isPending} disabled={act.isPending} onClick={() => act.mutate("approve_request")}>
                            {t("Approve")}
                          </Button>
                          <Button onClick={() => setReject(true)}>{t("Reject")}</Button>
                        </>
                      )}
                      {st === "Submitted" && needs && group && !pending && asked.data?.status !== "approved" && (
                        <Button variant="primary" loading={act.isPending} disabled={act.isPending} onClick={() => act.mutate("approve_request")}>
                          {t("Ask the approvers")}
                        </Button>
                      )}
                      {st === "Submitted" && needs && group && asked.data?.status === "rejected" && <Button onClick={() => setReject(true)}>{t("Reject it")}</Button>}
                      {["Submitted", "Approved"].includes(st) && (!needs || st === "Approved") && (
                        <Button variant="primary" loading={act.isPending} disabled={act.isPending} onClick={() => act.mutate("fulfil")}>
                          {t("Fulfil it")}
                        </Button>
                      )}
                      {["Submitted", "Approved"].includes(st) && (
                        <Button disabled={act.isPending} onClick={() => act.mutate("cancel_request")}>
                          {t("Cancel it")}
                        </Button>
                      )}
                    </>
                  ) : undefined
                }
              />
              <Refused error={act.error} />
              <PageSplit
                main={
                  <>
                    {needs && st === "Submitted" && (
                      <Alert tone="info">{group ? t("It waits for {n} of the approving group.", { n: Number(request.quorum ?? 1) }) : t("It waits for a Change Manager other than who asked.")}</Alert>
                    )}
                    {st === "Rejected" && <Alert tone="error">{t("Rejected: {why}", { why: isolate(text(request.decision_note)) })}</Alert>}
                    {group && asked.isError && status(asked.error) !== 403 && status(asked.error) !== 404 ? <Refused error={asked.error} /> : null}
                    {group && asked.data && <ApprovalState asked={asked.data} onChanged={refresh} />}
                    <Tasks request={request} tasks={tasks} />
                  </>
                }
                aside={
                  <section className="card card-pad stack" aria-labelledby="itsm-rq-facts">
                    <h2 id="itsm-rq-facts" className="section-title">
                      {t("What was asked")}
                    </h2>
                    <dl className="stack">
                      <div>
                        <dt className="muted">{t("Item")}</dt>
                        <dd>
                          <Link to={`/catalog/${text(request.offering)}`}>{isolate(titleOf(request, "offering"))}</Link>
                        </dd>
                      </div>
                      <div>
                        <dt className="muted">{t("Asked by")}</dt>
                        <dd>{isolate(text(request.requested_by))}</dd>
                      </div>
                      {answers.map((a) => (
                        <div key={text(a.question)}>
                          <dt className="muted">{isolate(text(a.question))}</dt>
                          <dd>{isolate(text(a.answer))}</dd>
                        </div>
                      ))}
                      {request.decided_by ? (
                        <div>
                          <dt className="muted">{t("Decided by")}</dt>
                          <dd>{isolate(text(request.decided_by))}</dd>
                        </div>
                      ) : null}
                    </dl>
                  </section>
                }
              />
              {reject && (
                <Ask
                  title={t("Reject the request")}
                  params={[{ name: "note", label: t("Why"), kind: "text", required: true }]}
                  confirm={t("Reject it")}
                  danger
                  onSubmit={(b) => run("CatalogRequest", id, "reject_request", b)}
                  onClose={() => setReject(false)}
                  onDone={() => (setReject(false), refresh())}
                />
              )}
            </>
          );
        }}
      </Await>
    </PageStack>
  );
}
