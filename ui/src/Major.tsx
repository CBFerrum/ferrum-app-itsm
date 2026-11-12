// A major incident: declare it with a communication plan, send updates, end it. Before each message goes, the screen shows who will
// be told (the subscribers of the incident's business service) and how the message reads; a message that would reach no one is
// said so. The updates already sent are the comments the caller sees on the incident.

import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { Link, useParams } from "react-router";
import { isolate, t } from "@ferrum/client";
import { Alert, Badge, Button, FormSkeleton, PageHead, PageSplit, PageStack, StatusBadge, toast } from "@ferrum/kit";
import { useMe } from "@ferrum/records";
import { record, rows, run, type Row, text, titleOf } from "./data";
import { priorityTone, updateOverdue, when } from "./desk";
import { isOpen, message, told, updatesOf, type Kind, type Told } from "./comms";
import { Ask, Await, Nav, useRights, type Param } from "./ui";

/** Who will be told, or why no one will. */
function Audience({ who }: { who: Told }) {
  if (who.why)
    return <Alert tone="warning">{who.why === "no_service" ? t("No one will be told: the incident names no business service, and a message goes to the subscribers of the service.") : t("No one will be told: the service has no subscribers. Add them on the service's configuration item.")}</Alert>;
  return (
    <div className="stack">
      <strong>{t("{n} people will be told", { n: who.who.length })}</strong>
      <ul className="stack">
        {who.who.map((w) => (
          <li key={w.address}>
            {isolate(w.address)} {w.note && <span className="muted">{isolate(w.note)}</span>}
          </li>
        ))}
      </ul>
    </div>
  );
}

/** The message as it will read, under the words being typed. */
function Preview({ kind, incident, who, v }: { kind: Kind; incident: Row; who: Told; v: { plan?: string; update?: string; resolution?: string } }) {
  const m = message(kind, incident, v);
  return (
    <div className="stack" aria-live="polite">
      <Audience who={who} />
      <div className="stack">
        <strong>{t("The message")}</strong>
        <p className="muted">{t("As the package words it; a workspace may have changed the wording under Messages.")}</p>
        <p>
          <strong>{isolate(m.subject)}</strong>
        </p>
        <p>{isolate(m.text)}</p>
      </div>
      {kind === "update" && <p className="muted">{t("The caller sees the update on the incident as a comment.")}</p>}
    </div>
  );
}

export function MajorRoute() {
  const { id = "" } = useParams();
  const qc = useQueryClient();
  const me = useMe();
  const rights = useRights("Incident");
  const write = !!rights && rights.write !== "none";
  const [ask, setAsk] = useState<Kind | null>(null);
  const q = useQuery({
    queryKey: ["itsm", "major", id],
    queryFn: async () => {
      const incident = await record("Incident", id);
      const service = incident.service ? await record("ConfigurationItem", text(incident.service)).catch(() => null) : null;
      const notes = await rows("ItNote", [["incident", "=", id]], { sort: "posted_at asc" }).catch(() => [] as Row[]);
      return { incident, service, notes };
    },
  });
  const refresh = () => void qc.invalidateQueries({ queryKey: ["itsm"] });
  return (
    <PageStack>
      <Nav />
      <Await q={q} skeleton={<FormSkeleton fields={5} />}>
        {({ incident, service, notes }) => {
          const who = told(incident, service);
          const open = isOpen(incident);
          const declared = incident.major_declared === true;
          const now = Date.now();
          const plan: Param = { name: "plan", label: t("Communication plan"), kind: "text", required: true, hint: t("Who is told what, and how often, while it is major.") };
          const next: Param = { name: "next_update_by", label: t("Next update by"), kind: "moment" };
          const forms: Record<Kind, { title: string; params: Param[]; confirm: string; action: string; danger?: boolean }> = {
            declared: { title: t("Declare a major incident"), params: [plan, { name: "manager", label: t("Major incident manager"), kind: "email", hint: me.data ? t("Left empty it is you: {who}", { who: me.data.user.email }) : undefined }, next], confirm: t("Declare it"), action: "declare_major", danger: true },
            update: { title: t("Send an update"), params: [{ name: "update", label: t("Update"), kind: "text", required: true }, next], confirm: t("Send the update"), action: "major_update" },
            resolved: { title: t("Resolve the major incident"), params: [{ name: "resolution", label: t("How it was resolved"), kind: "text", required: true }], confirm: t("Resolve it"), action: "end_major" },
          };
          const form = ask ? forms[ask] : null;
          return (
            <>
              <PageHead
                title={`${text(incident.number)} ${text(incident.title)}`.trim()}
                back={{ to: "/problems", label: t("Problems") }}
                description={
                  <>
                    <StatusBadge value={incident.status} /> <Badge tone={priorityTone(incident.priority)}>{isolate(text(incident.priority))}</Badge> {declared ? <Badge tone="red">{t("Major incident declared")}</Badge> : incident.major === true ? <Badge tone="amber">{t("Major by priority, not yet declared")}</Badge> : null}
                  </>
                }
                actions={
                  write && open ? (
                    declared ? (
                      <>
                        <Button variant="primary" onClick={() => setAsk("update")}>
                          {t("Send an update")}
                        </Button>
                        <Button onClick={() => setAsk("resolved")}>{t("Resolve it")}</Button>
                      </>
                    ) : (
                      <Button variant="primary" onClick={() => setAsk("declared")}>
                        {t("Declare a major incident")}
                      </Button>
                    )
                  ) : undefined
                }
              />
              {declared && open && updateOverdue(incident, now) && <Alert tone="error">{t("The next update was due {due}.", { due: when(incident.next_update_by) })}</Alert>}
              {!open && <Alert tone="info">{t("This incident is resolved: nothing more is sent.")}</Alert>}
              <PageSplit
                main={
                  <section className="card card-pad stack" aria-labelledby="itsm-mj-updates">
                    <h2 id="itsm-mj-updates" className="section-title">
                      {t("Updates sent")}
                    </h2>
                    {declared && incident.comms_plan ? (
                      <div>
                        <strong>{t("Communication plan")}</strong>
                        <p className="muted">{isolate(text(incident.comms_plan))}</p>
                      </div>
                    ) : null}
                    {updatesOf(notes).length === 0 ? (
                      <p className="muted">{t("No update has been sent.")}</p>
                    ) : (
                      <ol className="stack">
                        {updatesOf(notes).map((n) => (
                          <li key={n.id}>
                            <p>{isolate(text(n.body))}</p>
                            <p className="muted">
                              {isolate(text(n.author))} · {when(n.posted_at)}
                            </p>
                          </li>
                        ))}
                      </ol>
                    )}
                  </section>
                }
                aside={
                  <>
                    <section className="card card-pad stack" aria-labelledby="itsm-mj-facts">
                      <h2 id="itsm-mj-facts" className="section-title">
                        {t("The incident")}
                      </h2>
                      <dl className="stack">
                        <div>
                          <dt className="muted">{t("Business service")}</dt>
                          <dd>{incident.service ? <Link to={`/ci/${text(incident.service)}`}>{isolate(titleOf(incident, "service"))}</Link> : "—"}</dd>
                        </div>
                        {declared && (
                          <div>
                            <dt className="muted">{t("Major incident manager")}</dt>
                            <dd>{isolate(text(incident.major_manager))}</dd>
                          </div>
                        )}
                        {incident.next_update_by ? (
                          <div>
                            <dt className="muted">{t("Next update by")}</dt>
                            <dd>{when(incident.next_update_by)}</dd>
                          </div>
                        ) : null}
                      </dl>
                      <Link to={`/e/Incident/${id}`}>{t("Open the whole record")}</Link>
                    </section>
                    <section className="card card-pad stack" aria-labelledby="itsm-mj-told">
                      <h2 id="itsm-mj-told" className="section-title">
                        {t("Who is told")}
                      </h2>
                      <Audience who={who} />
                    </section>
                  </>
                }
              />
              {ask && form && (
                <Ask
                  title={form.title}
                  params={form.params}
                  confirm={form.confirm}
                  danger={form.danger}
                  onSubmit={(b) => run("Incident", id, form.action, b)}
                  onClose={() => setAsk(null)}
                  onDone={() => (setAsk(null), refresh(), toast(t("Done")))}
                  preview={(v) => <Preview kind={ask} incident={incident} who={who} v={{ plan: String(v.plan ?? ""), update: String(v.update ?? ""), resolution: String(v.resolution ?? "") }} />}
                />
              )}
            </>
          );
        }}
      </Await>
    </PageStack>
  );
}
