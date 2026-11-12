// The platform's approval of an action (GET /api/v1/{E}/{id}/approval): who was asked, who has answered, how many of how many
// each step needs, who stood in for whom, what was reminded or escalated, and, for a person it is asked of, Approve and Reject
// (a rejection says why). The change board and a catalog item's approver group both draw it.

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { api, docPath, formatMoment, isolate, t } from "@ferrum/client";
import { Badge, Button, Field, placed, type Tone } from "@ferrum/kit";
import { useLinkTitle } from "@ferrum/records";
import type { MyApprovals } from "./waiting";
import { Refused } from "./ui";

export interface Step {
  label?: string;
  state: "waiting" | "open" | "skipped" | "approved" | "rejected";
  /** A number, `all` or `any`. */
  need: number | string;
  approvers: string[];
  escalated?: string[];
  reminded_at?: string | null;
  escalated_at?: string | null;
  remind_at?: string | null;
  escalate_at?: string | null;
}

export interface Decision {
  step: number;
  decider: string;
  on_behalf_of?: string | null;
  outcome: "approve" | "reject";
  comment?: string | null;
  at: string;
  void?: boolean;
}

export interface ApprovalAsked {
  id: string;
  entity: string;
  record: string;
  action: string;
  requester: string;
  requested_at: string;
  status: "pending" | "approved" | "rejected" | "recalled" | "cancelled" | "failed";
  failure?: string | null;
  steps: Step[];
  decisions: Decision[];
}

const STATUS: Record<ApprovalAsked["status"], { tone: Tone; word: () => string }> = {
  pending: { tone: "amber", word: () => t("Waiting for approval") },
  approved: { tone: "green", word: () => t("Approved") },
  rejected: { tone: "red", word: () => t("Rejected") },
  recalled: { tone: "neutral", word: () => t("Recalled") },
  cancelled: { tone: "neutral", word: () => t("Cancelled") },
  failed: { tone: "red", word: () => t("Not done") },
};

const STEP: Record<Step["state"], { tone: Tone; word: () => string }> = {
  waiting: { tone: "neutral", word: () => t("Not yet") },
  open: { tone: "amber", word: () => t("Waiting") },
  skipped: { tone: "neutral", word: () => t("Skipped") },
  approved: { tone: "green", word: () => t("Approved") },
  rejected: { tone: "red", word: () => t("Rejected") },
};

/** How many of the step's approvers it needs: a number, all of them, or any one. */
export function needed(step: Pick<Step, "need" | "approvers">): number {
  if (step.need === "all") return step.approvers.length;
  return typeof step.need === "number" ? step.need : 1;
}

/** "1 of 3 approved, 2 needed": what a step has and what it still wants, counting each person once (a decision cancelled by a
 *  change to a field that matters, `void`, no longer counts). */
export function tally(step: Step, index: number, decisions: Decision[]): { approved: number; need: number; asked: number } {
  const who = new Set(decisions.filter((d) => d.step === index && !d.void && d.outcome === "approve").map((d) => d.on_behalf_of || d.decider));
  return { approved: who.size, need: needed(step), asked: step.approvers.length };
}

/** A person by their user id, as a name. */
export function Who({ id }: { id: string }) {
  const name = useLinkTitle("User", id);
  return <>{isolate(name ?? "…")}</>;
}

const People = ({ ids }: { ids: string[] }) => (
  <>
    {ids.map((id, i) => (
      <span key={id}>
        {i > 0 && ", "}
        <Who id={id} />
      </span>
    ))}
  </>
);

const at = (v: string) => formatMoment(v, { dateStyle: "medium", timeStyle: "short" });

/** The request's approval of `action` on the record: `null` while no one has been asked. */
export const useAsked = (entity: string, id: string) =>
  useQuery({ queryKey: ["itsm", "approval", entity, id], queryFn: ({ signal }) => api.get<ApprovalAsked | null>(docPath(entity, id, "approval"), undefined, signal) });

/** What the person has to decide: the platform's own list. */
export const useMine = (enabled: boolean) => useQuery({ queryKey: ["approvals"], enabled, queryFn: ({ signal }) => api.get<MyApprovals>("/api/approvals", undefined, signal) });

function Decide({ asked, onDone }: { asked: ApprovalAsked; onDone: () => void }) {
  const [comment, setComment] = useState("");
  const [need, setNeed] = useState(false);
  const decide = useMutation({
    mutationFn: (outcome: "approve" | "reject") => api.post(docPath(asked.entity, asked.record, "approval/decide"), { outcome, comment: comment.trim() || undefined }),
    onSuccess: onDone,
  });
  const go = (outcome: "approve" | "reject") => {
    if (decide.isPending) return;
    if (outcome === "reject" && !comment.trim()) return setNeed(true);
    setNeed(false);
    decide.mutate(outcome);
  };
  return (
    <div className="stack">
      <Refused error={decide.error} />
      <Field label={t("Comment")} htmlFor="itsm-approval-comment" error={need ? t("Say why when you reject") : undefined}>
        <textarea id="itsm-approval-comment" className="textarea" rows={2} value={comment} onChange={(e) => setComment(e.target.value)} />
      </Field>
      <div className="row">
        <Button variant="primary" loading={decide.isPending && decide.variables === "approve"} disabled={decide.isPending} onClick={() => go("approve")}>
          {t("Approve")}
        </Button>
        <Button variant="danger" loading={decide.isPending && decide.variables === "reject"} disabled={decide.isPending} onClick={() => go("reject")}>
          {t("Reject")}
        </Button>
      </div>
    </div>
  );
}

export function ApprovalState({ asked, onChanged }: { asked: ApprovalAsked; onChanged?: () => void }) {
  const qc = useQueryClient();
  const pending = asked.status === "pending";
  const mine = useMine(pending);
  // Whether the person decides, themselves or for someone who is away, is the platform's to say.
  const mayDecide = pending && !!mine.data?.to_decide.some((i) => i.request === asked.id);
  const done = () => {
    for (const key of [["itsm", "approval", asked.entity, asked.record], ["approvals"], ["itsm"]]) void qc.invalidateQueries({ queryKey: key });
    onChanged?.();
  };
  const status = STATUS[asked.status];
  const live = asked.decisions.filter((d) => !d.void);
  const voided = asked.decisions.length - live.length;
  return (
    <section className="card card-pad stack" aria-labelledby="itsm-approval">
      <div className="row">
        <h2 id="itsm-approval" className="section-title">
          {t("Approval")}
        </h2>
        <Badge tone={status.tone}>{status.word()}</Badge>
      </div>
      <p className="muted">{placed(t("Asked by {person} on {at}."), { person: <Who id={asked.requester} />, at: isolate(at(asked.requested_at)) })}</p>
      {asked.status === "failed" && <p role="alert">{t("Everyone agreed, but it could not be done: {reason}", { reason: isolate(asked.failure || t("no reason was given")) })}</p>}
      {voided > 0 && <p className="muted">{t("{n} earlier decisions were cancelled when a field that matters changed, and the steps opened again.", { n: voided })}</p>}
      <ol className="stack" aria-label={t("Steps")}>
        {asked.steps.map((s, n) => {
          const made = live.filter((d) => d.step === n);
          const c = tally(s, n, asked.decisions);
          return (
            <li key={n} className="stack">
              <div className="row">
                <strong>{isolate(s.label || t("Step {n}", { n: n + 1 }))}</strong>
                <Badge tone={STEP[s.state].tone}>{STEP[s.state].word()}</Badge>
                {s.state !== "skipped" && <span className="muted">{t("{approved} of {need} needed, {asked} asked", { approved: c.approved, need: c.need, asked: c.asked })}</span>}
              </div>
              {s.state !== "skipped" && (
                <p className="muted">
                  {t("Asked")}: <People ids={s.approvers} />
                </p>
              )}
              {made.map((d, i) => (
                <p key={i} className="muted">
                  {d.on_behalf_of ? placed(t("{person} for {other}"), { person: <Who id={d.decider} />, other: <Who id={d.on_behalf_of} /> }) : <Who id={d.decider} />} {d.outcome === "approve" ? t("approved") : t("rejected")}{" "}
                  <time dateTime={d.at}>{isolate(at(d.at))}</time>
                  {d.comment ? <>: {isolate(d.comment)}</> : null}
                </p>
              ))}
              {!!s.escalated?.length && (
                <p className="muted">
                  {t("Escalated to")} <People ids={s.escalated} />
                </p>
              )}
              {s.reminded_at && <p className="muted">{t("Reminded {at}", { at: at(s.reminded_at) })}</p>}
              {s.state === "open" && s.remind_at && !s.reminded_at && <p className="faint">{t("Reminder due {at}", { at: at(s.remind_at) })}</p>}
              {s.state === "open" && s.escalate_at && !s.escalated_at && !s.escalated?.length && <p className="faint">{t("Escalates {at}", { at: at(s.escalate_at) })}</p>}
            </li>
          );
        })}
      </ol>
      {pending && mine.isError && <Refused error={mine.error} />}
      {mayDecide && <Decide asked={asked} onDone={done} />}
    </section>
  );
}
