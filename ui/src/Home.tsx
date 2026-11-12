// The desk: what is the person's to do (their own queue and their team's: incidents, requests and their tasks, and what waits for
// their decision), the targets in trouble, a banner for each major incident, and a way to report one at once. Each opens its
// record or its screen. A person who is not staff (a portal person, a requester) sees what the server lets them read: their own.

import { useMutation, useQuery } from "@tanstack/react-query";
import { type FormEvent, useState } from "react";
import { useNavigate } from "react-router";
import { api, ApiError, isolate, refusal, t } from "@ferrum/client";
import { Alert, Badge, Button, ButtonLink, Field, HomeSkeleton, LoadFailed, PageHead, PageStack, Queue, QueueGrid, QueueItem, StatePanel, StatusBadge } from "@ferrum/kit";
import { can, useMe, useMetaIndex, type Me } from "@ferrum/records";
import { make, rows, type Row, text, titleOf } from "./data";
import { byPriority, incidentFlags, inTrouble, majors, parts, priorityTone, split, updateOverdue, when, type Flag } from "./desk";
import { Suggest } from "./Suggest";
import { Nav, Refused, TargetBadge } from "./ui";
import { awaitingMe, type MyApprovals } from "./waiting";

export type { Row } from "./data";

/** The queues show this many rows each; the rest is "and N more", linking to the list. */
const LIMIT = 5;
const OPEN = ["New", "In Progress", "On Hold", "Awaiting Caller"];

const FLAG_TONE: Record<Flag, "red" | "amber"> = { breached: "red", at_risk: "amber" };

function useDesk(me: Me) {
  const meta = useMetaIndex();
  const read = (entity: string) => can(meta.data?.entities.find((e) => e.name === entity)?.permissions, "read");
  const staff = can(meta.data?.entities.find((e) => e.name === "Incident")?.permissions, "write");
  const decider = me.user.is_admin || me.user.roles.includes("Change Manager");
  const email = me.user.email.toLowerCase();
  return useQuery({
    queryKey: ["itsm", "desk", email, staff, decider],
    enabled: !!meta.data,
    refetchInterval: 30_000,
    queryFn: async () => {
      // Each part is asked on its own, and only where the person may read it: what is refused is left out, what fails is said.
      const attempt = async <T,>(ran: boolean, go: () => Promise<T>, none: T) =>
        ran ? go().then((v) => ({ v, ran, error: undefined as unknown }), (error: unknown) => ({ v: none, ran, error })) : { v: none, ran, error: undefined as unknown };
      const none: Row[] = [];
      const [incidents, requests, tasks, changes, groups, missing, offerings, approvals] = await Promise.all([
        attempt(read("Incident"), () => rows("Incident", [["status", "in", OPEN]], { titles: true }), none),
        attempt(read("CatalogRequest"), () => rows("CatalogRequest", [["status", "in", ["Submitted", "Approved", "Awaiting Caller"]]], { titles: true, sort: "due_at asc" }), none),
        attempt(staff && read("RequestTask"), () => rows("RequestTask", [["status", "=", "Open"]], { titles: true, sort: "due_by asc" }), none),
        attempt(read("ItChange"), () => rows("ItChange", [["status", "in", ["Submitted", "Approved"]]], { sort: "planned_start asc" }), none),
        attempt(staff && read("ItGroup"), () => rows("ItGroup", [["members.member", "=", email]]), none),
        // Only those who may read configuration items see them.
        attempt(staff && read("ConfigurationItem"), () => rows("ConfigurationItem", [["missing_since", "is", "set"]], { sort: "missing_since asc" }), none),
        attempt(decider && read("ServiceOffering"), () => rows("ServiceOffering", [["needs_approval", "=", true]], { fields: "id" }), none),
        attempt(staff || decider, () => api.get<MyApprovals>("/api/approvals"), null as MyApprovals | null),
      ]);
      const ran = [incidents, requests, tasks, changes, groups, missing, offerings, approvals].filter((a) => a.ran);
      if (!ran.length) throw new ApiError(403, "forbidden", t("You do not have permission to do that."));
      if (ran.every((a) => a.error !== undefined)) throw ran[0].error;
      return {
        staff,
        incidents: byPriority(incidents.v),
        requests: requests.v,
        tasks: tasks.v,
        changes: changes.v,
        groups: new Set(groups.v.map((g) => g.id)),
        missing: missing.v,
        waiting: awaitingMe(approvals.v, changes.v, requests.v, new Set(offerings.v.map((o) => o.id)), { id: me.user.id, email, decider }),
        problems: ran.filter((a) => a.error !== undefined && (a.error as { status?: number }).status !== 403).map((a) => a.error),
      };
    },
  });
}

/** Report an incident without leaving the desk: its title, how bad and how urgent; the known problems that fit are shown as it is typed. */
function Quick() {
  const go = useNavigate();
  const [title, setTitle] = useState("");
  const [impact, setImpact] = useState("Medium");
  const [urgency, setUrgency] = useState("Medium");
  const report = useMutation({
    mutationFn: () => make("Incident", { title: title.trim(), impact, urgency }),
    onSuccess: (made) => go(`/e/Incident/${made.id}`),
  });
  const submit = (e: FormEvent) => {
    e.preventDefault();
    if (title.trim() && !report.isPending) report.mutate();
  };
  const level = (id: string, label: string, value: string, set: (v: string) => void) => (
    <Field label={label} htmlFor={id}>
      <select id={id} className="select" value={value} onChange={(e) => set(e.target.value)}>
        {([["High", t("High")], ["Medium", t("Medium")], ["Low", t("Low")]] as const).map(([l, word]) => (
          <option key={l} value={l}>
            {word}
          </option>
        ))}
      </select>
    </Field>
  );
  return (
    <section className="card card-pad" aria-labelledby="itsm-quick">
      <form className="stack" onSubmit={submit}>
        <h2 id="itsm-quick" className="section-title">
          {t("Report an incident")}
        </h2>
        <Refused error={report.error} />
        <Field label={t("What is wrong")} htmlFor="itsm-quick-title" required>
          <input id="itsm-quick-title" className="input" value={title} onChange={(e) => setTitle(e.target.value)} autoComplete="off" />
        </Field>
        <Suggest typed={title} />
        <div className="row">
          {level("itsm-quick-impact", t("Impact"), impact, setImpact)}
          {level("itsm-quick-urgency", t("Urgency"), urgency, setUrgency)}
          <Button variant="primary" type="submit" icon="plus" loading={report.isPending} disabled={!title.trim()}>
            {t("Report it")}
          </Button>
        </div>
        <div className="row">
          <ButtonLink to="/catalog" size="sm">
            {t("Ask for something")}
          </ButtonLink>
          <ButtonLink to="/change/new" size="sm">
            {t("Plan a change")}
          </ButtonLink>
        </div>
      </form>
    </section>
  );
}

export function Home() {
  const me = useMe();
  if (me.isPending) return <Frame><HomeSkeleton queues={3} /></Frame>;
  if (me.isError) return <Frame><LoadFailed error={me.error} onRetry={() => void me.refetch()} /></Frame>;
  return <Desk me={me.data} />;
}

function Frame({ children, actions }: { children: React.ReactNode; actions?: React.ReactNode }) {
  return (
    <PageStack>
      <Nav />
      <PageHead title={t("The desk")} actions={actions} />
      {children}
    </PageStack>
  );
}

function Desk({ me }: { me: Me }) {
  const q = useDesk(me);
  if (q.isPending) return <Frame><HomeSkeleton queues={3} /></Frame>;
  if (q.isError) return <Frame><LoadFailed error={q.error} onRetry={() => void q.refetch()} /></Frame>;
  const { staff, incidents, requests, tasks, changes, groups, missing, waiting, problems } = q.data;
  const email = me.user.email;
  const now = Date.now();
  const inc = split(incidents, email, groups);
  const req = split(requests, email, groups);
  const task = split(tasks, email, groups);
  const major = majors(incidents);
  const trouble = inTrouble(incidents, requests, tasks, now);
  const summary = [incidents.length && t("{n} open incidents", { n: incidents.length }), requests.length && t("{n} requests waiting", { n: requests.length })].filter(Boolean).join(" · ");
  const empty = !incidents.length && !requests.length && !changes.length && !waiting.length && !tasks.length;

  const incident = (i: Row) => {
    const f = incidentFlags(i, now);
    const words = {
      response: { breached: t("Response breached"), at_risk: t("Response at risk") },
      resolution: { breached: t("Resolution breached"), at_risk: t("Resolution at risk") },
    };
    return (
      <QueueItem
        key={i.id}
        to={`/e/Incident/${i.id}`}
        title={isolate(text(i.title))}
        meta={parts(i.number, when(i.resolve_by) && t("due {due}", { due: when(i.resolve_by) }))}
        trailing={
          <>
            {f.response ? <Badge tone={FLAG_TONE[f.response]}>{words.response[f.response]}</Badge> : null}
            {f.resolution ? <Badge tone={FLAG_TONE[f.resolution]}>{words.resolution[f.resolution]}</Badge> : null}
            <Badge tone={priorityTone(i.priority)}>{isolate(text(i.priority))}</Badge>
          </>
        }
      />
    );
  };
  const request = (r: Row) => {
    const asked = titleOf(r, "offering");
    return (
      <QueueItem
        key={r.id}
        to={`/request/${r.id}`}
        title={isolate(asked || text(r.number))}
        meta={parts(asked && r.number, when(r.due_at) && t("due {due}", { due: when(r.due_at) }), r.requested_by && t("by {value}", { value: text(r.requested_by) }))}
        trailing={<><TargetBadge state={r.fulfil_state} /><StatusBadge value={r.status} /></>}
      />
    );
  };
  const taskRow = (k: Row) => (
    <QueueItem key={k.id} to={`/request/${text(k.request)}`} title={isolate(text(k.step_name))} meta={parts(titleOf(k, "request"), when(k.due_by) && t("due {due}", { due: when(k.due_by) }))} trailing={<TargetBadge state={k.task_state} />} />
  );
  const more = (entity: string, list: Row[]) => ({ to: `/e/${entity}`, count: list.length - LIMIT });

  return (
    <PageStack>
      <Nav />
      <PageHead
        title={t("The desk")}
        description={summary ? isolate(summary) : undefined}
        actions={
          staff ? null : (
            <ButtonLink to="/e/Incident/new" variant="primary" icon="plus">
              {t("New incident")}
            </ButtonLink>
          )
        }
      />
      {problems.length > 0 && (
        <Alert tone="warning" action={<Button size="sm" icon="refresh" onClick={() => void q.refetch()}>{t("Try again")}</Button>}>
          {t("Some of this could not be read: {why}", { why: isolate(refusal(problems[0])) })}
        </Alert>
      )}
      {major.map((m) => (
        <Alert key={m.id} tone="error" action={<ButtonLink to={`/major/${m.id}`} size="sm">{t("Open the major incident")}</ButtonLink>}>
          <strong>{t("Major incident")}</strong> {isolate(`${text(m.number)} ${text(m.title)}`)}
          {updateOverdue(m, now) ? <> · {t("next update was due {due}", { due: when(m.next_update_by) })}</> : m.next_update_by ? <> · {t("next update {due}", { due: when(m.next_update_by) })}</> : null}
        </Alert>
      ))}
      {staff && <Quick />}
      {empty && !staff ? (
        <StatePanel kind="empty" title={t("Nothing of yours is open")} action={<ButtonLink to="/catalog" variant="primary">{t("Ask for something")}</ButtonLink>}>
          {t("What you report or ask for appears here, with where it stands.")}
        </StatePanel>
      ) : (
        <QueueGrid>
          {trouble.length > 0 && (
            <Queue title={t("Targets breached or at risk")} count={trouble.length} tone="attention">
              {trouble.slice(0, LIMIT).map((h) => (
                <QueueItem
                  key={`${h.entity}/${h.row.id}/${h.target}`}
                  to={h.entity === "Incident" ? `/e/Incident/${h.row.id}` : h.entity === "CatalogRequest" ? `/request/${h.row.id}` : `/request/${text(h.row.request)}`}
                  title={isolate(text(h.row.title) || text(h.row.step_name) || titleOf(h.row, "offering") || text(h.row.number))}
                  meta={parts(h.row.number, h.target, when(h.due) && t("due {due}", { due: when(h.due) }))}
                  trailing={<Badge tone={FLAG_TONE[h.flag]}>{h.flag === "breached" ? t("Breached") : t("At risk")}</Badge>}
                />
              ))}
            </Queue>
          )}
          {waiting.length > 0 && (
            <Queue title={t("Waiting for your decision")} count={waiting.length} tone="attention">
              {waiting.slice(0, LIMIT).map((w) => (
                <QueueItem
                  key={`${w.entity}/${w.id}`}
                  to={w.entity === "ItChange" ? `/change/${w.id}` : `/request/${w.id}`}
                  title={isolate(w.title)}
                  meta={parts(w.entity === "ItChange" ? t("Change") : t("Request"), when(w.from) && t("asked {at}", { at: when(w.from) }))}
                  trailing={<Badge tone="amber">{w.by === "approval" ? t("Asked of you") : t("A Change Manager decides")}</Badge>}
                />
              ))}
            </Queue>
          )}
          {staff ? (
            <>
              <Queue title={t("Your incidents")} count={inc.mine.length} more={more("Incident", inc.mine)}>
                {inc.mine.slice(0, LIMIT).map(incident)}
              </Queue>
              {groups.size > 0 && (
                <Queue title={t("Your team's incidents")} count={inc.team.length} more={more("Incident", inc.team)}>
                  {inc.team.slice(0, LIMIT).map(incident)}
                </Queue>
              )}
              <Queue title={t("Your requests and tasks")} count={req.mine.length + task.mine.length} more={more("CatalogRequest", req.mine)}>
                {task.mine.slice(0, LIMIT).map(taskRow)}
                {req.mine.slice(0, LIMIT).map(request)}
              </Queue>
              {groups.size > 0 && (
                <Queue title={t("Your team's requests and tasks")} count={req.team.length + task.team.length} more={more("CatalogRequest", req.team)}>
                  {task.team.slice(0, LIMIT).map(taskRow)}
                  {req.team.slice(0, LIMIT).map(request)}
                </Queue>
              )}
            </>
          ) : (
            <>
              <Queue title={t("Your incidents")} count={incidents.length} more={more("Incident", incidents)}>
                {incidents.slice(0, LIMIT).map(incident)}
              </Queue>
              <Queue title={t("Your requests")} count={requests.length} more={more("CatalogRequest", requests)}>
                {requests.slice(0, LIMIT).map(request)}
              </Queue>
            </>
          )}
          {changes.length > 0 && (
            <Queue title={t("Changes coming up")} count={changes.length} more={{ to: "/calendar", count: changes.length - LIMIT }}>
              {changes.slice(0, LIMIT).map((c) => (
                <QueueItem key={c.id} to={`/change/${c.id}`} title={isolate(text(c.title) || text(c.number))} meta={parts(c.number, when(c.planned_start) && t("from {starts}", { starts: when(c.planned_start) }))} trailing={<StatusBadge value={c.status} />} />
              ))}
            </Queue>
          )}
          {missing.length > 0 && (
            <Queue title={t("Not seen by discovery")} count={missing.length} more={{ to: "/items", count: missing.length - LIMIT }}>
              {missing.slice(0, LIMIT).map((c) => (
                <QueueItem key={c.id} to={`/ci/${c.id}`} title={isolate(text(c.ci_name))} meta={isolate(`${text(c.discovered_by)}${t(", last")} ${text(c.missing_since)}`)} />
              ))}
            </Queue>
          )}
        </QueueGrid>
      )}
    </PageStack>
  );
}

