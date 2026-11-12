//! The service desk: catalog requests, incidents and problems.
//!
//! * A catalog request one that needs approval is fulfilled only once approved, by a Change Manager
//!   who did not ask for it.
//! * An incident's impact and urgency give its priority through the Priority Matrix records, and a P1
//!   on a critical item is major. Resolved, it says how. Its response and resolution times, and a
//!   request's, are service targets of the platform (the entities' `targets`), not arithmetic here.
//! * A known error has its workaround; a problem resolved says its root cause and resolves the
//!   incidents still open under it.

use crate::{at, obj};
use async_trait::async_trait;
use chrono::Utc;
use ferrum_domain::util::{elevated, id, s};
use ferrum_kernel::{Ctx, Doc, DocHook, Error, HookCtx, Result};
use serde_json::{Value, json};
use uuid::Uuid;

fn decides(ctx: &Ctx) -> bool {
    ctx.is_admin || ctx.roles.iter().any(|r| r == "Change Manager")
}

/// Who a record is about, and who works it: a caller left empty is the person raising it, and an
/// assignee is one of the assignment group's members.
async fn people(hc: &mut HookCtx<'_>, doc: &mut Doc) -> Result<()> {
    if s(doc, "caller_email").is_none() && s(doc, "caller_name").is_none() && !hc.ctx.email.is_empty() {
        doc.insert("caller_email".into(), json!(hc.ctx.email.to_lowercase()));
    }
    let (Some(group), Some(who)) = (id(doc, "assignment_group"), s(doc, "assigned_to")) else { return Ok(()) };
    let member: bool =
        sqlx::query_scalar("select exists (select 1 from e_it_group__members m where m.tenant_id = $1 and m.parent_id = $2 and lower(m.member) = lower($3))")
            .bind(hc.ctx.tenant)
            .bind(group)
            .bind(who)
            .fetch_one(&mut **hc.tx)
            .await?;
    if member { Ok(()) } else { Err(Error::field("assigned_to", "is not a member of the assignment group")) }
}

/// An offering's questions each have a label of their own, choices where they are a choice, and an
/// "only when" that names an earlier question.
pub struct OfferingHook;

#[async_trait]
impl DocHook for OfferingHook {
    fn name(&self) -> &'static str {
        "itsm.offering"
    }

    async fn validate(&self, _hc: &mut HookCtx<'_>, doc: &Doc) -> Result<()> {
        let mut earlier: Vec<String> = vec![];
        for (i, q) in rows(doc, "questions").iter().enumerate() {
            let at = |m: &str| Error::field("questions", format!("question {}: {m}", i + 1));
            let label = s(q, "label").unwrap_or_default().to_lowercase();
            if earlier.contains(&label) {
                return Err(at("two questions have the same label"));
            }
            if s(q, "qtype") == Some("Choice") && choices(q).is_empty() {
                return Err(at("a choice question lists its choices"));
            }
            if let Some(w) = s(q, "only_when")
                && !earlier.contains(&w.to_lowercase())
            {
                return Err(at("is asked only when an earlier question has an answer: name one"));
            }
            earlier.push(label);
        }
        Ok(())
    }
}

/// A table field's rows.
fn rows<'a>(doc: &'a Doc, table: &str) -> Vec<&'a Doc> {
    doc.get(table).and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_object).collect()).unwrap_or_default()
}

/// The choices of a choice question.
fn choices(q: &Doc) -> Vec<&str> {
    s(q, "options").map(|o| o.split(',').map(str::trim).filter(|c| !c.is_empty()).collect()).unwrap_or_default()
}

/// Whether the answers fit the offering's questions: each one asked is answered when it is required and
/// answered as its kind says, one whose "only when" does not hold is not answered, and nothing is
/// answered that the offering does not ask.
async fn answers_fit(hc: &mut HookCtx<'_>, doc: &Doc) -> Result<()> {
    let asked: Vec<Doc> = sqlx::query_scalar::<_, Value>(
        "select jsonb_build_object('label', label, 'qtype', qtype, 'required', coalesce(required, false), 'options', options, 'only_when', only_when, \
         'only_when_answer', only_when_answer) from e_service_offering__questions where tenant_id = $1 and parent_id = $2 order by idx",
    )
    .bind(hc.ctx.tenant)
    .bind(id(doc, "offering"))
    .fetch_all(&mut **hc.tx)
    .await?
    .into_iter()
    .filter_map(|v| v.as_object().cloned())
    .collect();
    let given: Vec<(String, String)> = rows(doc, "answers")
        .iter()
        .map(|a| (s(a, "question").unwrap_or_default().trim().to_lowercase(), s(a, "answer").unwrap_or_default().trim().to_string()))
        .collect();
    let answer = |label: &str| given.iter().find(|(q, _)| q == &label.trim().to_lowercase()).map(|(_, a)| a.as_str()).unwrap_or_default();
    let bad = |m: String| Error::field("answers", m);
    for (q, _) in &given {
        if !asked.iter().any(|a| s(a, "label").is_some_and(|l| l.trim().eq_ignore_ascii_case(q))) {
            return Err(bad(format!("{q} is not a question of this offering")));
        }
    }
    for q in &asked {
        let label = s(q, "label").unwrap_or_default();
        let a = answer(label);
        let applies = s(q, "only_when").is_none_or(|w| answer(w).eq_ignore_ascii_case(s(q, "only_when_answer").unwrap_or_default().trim()));
        if !applies {
            if !a.is_empty() {
                return Err(bad(format!(
                    "{label} is asked only when {} is {}",
                    s(q, "only_when").unwrap_or_default(),
                    s(q, "only_when_answer").unwrap_or_default()
                )));
            }
            continue;
        }
        if a.is_empty() {
            if q.get("required").and_then(Value::as_bool) == Some(true) {
                return Err(bad(format!("{label} needs an answer")));
            }
            continue;
        }
        let fits = match s(q, "qtype") {
            Some("Number") => a.parse::<f64>().is_ok(),
            Some("Date") => chrono::NaiveDate::parse_from_str(a, "%Y-%m-%d").is_ok(),
            Some("Yes/No") => a == "Yes" || a == "No",
            Some("Choice") => choices(q).contains(&a),
            _ => true,
        };
        if !fits {
            let kind = match s(q, "qtype") {
                Some("Number") => "a number".to_string(),
                Some("Date") => "a date (YYYY-MM-DD)".to_string(),
                Some("Yes/No") => "Yes or No".to_string(),
                _ => format!("one of {}", choices(q).join(", ")),
            };
            return Err(bad(format!("{label} is {kind}")));
        }
    }
    Ok(())
}

pub struct RequestHook;

#[async_trait]
impl DocHook for RequestHook {
    fn name(&self) -> &'static str {
        "itsm.request"
    }

    fn fills(&self) -> &'static [&'static str] {
        &["requested_by", "caller_email", "assignment_group", "route", "quorum", "approver_members"]
    }

    async fn after_save(&self, hc: &mut HookCtx<'_>, doc: &Doc, old: Option<&Doc>) -> Result<()> {
        // A request that needs no approval has its tasks as it is made; one that does, when it is approved.
        let needs: Option<bool> = sqlx::query_scalar("select coalesce(needs_approval, false) from e_service_offering where tenant_id = $1 and id = $2")
            .bind(hc.ctx.tenant)
            .bind(id(doc, "offering"))
            .fetch_optional(&mut **hc.tx)
            .await?;
        if old.is_none() && needs != Some(true) && s(doc, "status") == Some("Submitted") {
            make_tasks(hc, doc).await?;
        }
        Ok(())
    }

    async fn before_validate(&self, hc: &mut HookCtx<'_>, doc: &mut Doc) -> Result<()> {
        people(hc, doc).await?;
        crate::defaults::ensure_calendar(hc.tx, hc.ctx.tenant).await?;
        if id(doc, "assignment_group").is_none() {
            let group: Option<Uuid> = sqlx::query_scalar("select fulfilment_group from e_service_offering where tenant_id = $1 and id = $2")
                .bind(hc.ctx.tenant)
                .bind(id(doc, "offering"))
                .fetch_optional(&mut **hc.tx)
                .await?
                .flatten();
            if let Some(g) = group {
                doc.insert("assignment_group".into(), json!(g));
            }
        }
        if s(doc, "requested_by").is_none() && !hc.ctx.email.is_empty() {
            doc.insert("requested_by".into(), json!(hc.ctx.email.to_lowercase()));
        }
        // Who approves it, as the offering says when this request is made: its approver group (N of M) or, left
        // as it was before release 0.4, one Change Manager. Taken once, while it waits for approval to begin.
        if matches!(s(doc, "status"), Some("Submitted") | None) {
            let off: Option<(Option<bool>, Option<Uuid>, Option<i64>)> =
                sqlx::query_as("select needs_approval, approver_group, approvals_needed from e_service_offering where tenant_id = $1 and id = $2")
                    .bind(hc.ctx.tenant)
                    .bind(id(doc, "offering"))
                    .fetch_optional(&mut **hc.tx)
                    .await?;
            let (needs, group, n) = off.unwrap_or_default();
            let group = group.filter(|_| needs == Some(true));
            let who: Vec<String> = match group {
                Some(g) => {
                    sqlx::query_scalar("select lower(member) from e_it_group__members where tenant_id = $1 and parent_id = $2 order by idx")
                        .bind(hc.ctx.tenant)
                        .bind(g)
                        .fetch_all(&mut **hc.tx)
                        .await?
                }
                None => vec![],
            };
            doc.insert("route".into(), json!(if group.is_some() { "Group" } else { "Manager" }));
            doc.insert("quorum".into(), group.map_or(Value::Null, |_| json!(n.unwrap_or(1))));
            doc.insert("approver_members".into(), group.map_or(Value::Null, |_| json!(who)));
        }
        Ok(())
    }

    async fn validate_change(&self, hc: &mut HookCtx<'_>, old: Option<&Doc>, doc: &Doc) -> Result<()> {
        // Answers are held to the offering's questions as they are given, not again at each decision.
        if old.is_none_or(|o| o.get("answers") != doc.get("answers") || id(o, "offering") != id(doc, "offering")) {
            answers_fit(hc, doc).await?;
        }
        Ok(())
    }

    async fn validate(&self, hc: &mut HookCtx<'_>, doc: &Doc) -> Result<()> {
        let active: Option<bool> = sqlx::query_scalar("select coalesce(active, true) from e_service_offering where tenant_id = $1 and id = $2")
            .bind(hc.ctx.tenant)
            .bind(id(doc, "offering"))
            .fetch_optional(&mut **hc.tx)
            .await?;
        if active == Some(false) && s(doc, "status") == Some("Submitted") {
            return Err(Error::field("offering", "is not offered any more"));
        }
        Ok(())
    }

    async fn action(&self, hc: &mut HookCtx<'_>, name: &str, doc: &Doc, params: &Value) -> Result<Option<Value>> {
        let request = id(doc, "id").ok_or_else(|| Error::internal("a request without an id"))?;
        let needs: Option<bool> = sqlx::query_scalar("select coalesce(needs_approval, false) from e_service_offering where tenant_id = $1 and id = $2")
            .bind(hc.ctx.tenant)
            .bind(id(doc, "offering"))
            .fetch_optional(&mut **hc.tx)
            .await?;
        let needs = needs.unwrap_or(false);
        let mine = s(doc, "requested_by").is_some_and(|r| r.eq_ignore_ascii_case(&hc.ctx.email));
        let note = params.get("note").and_then(Value::as_str).map(str::trim).filter(|n| !n.is_empty());
        let values = match name {
            "approve_request" | "reject_request" => {
                if !needs {
                    return Err(Error::invalid("this offering needs no approval"));
                }
                // With the offering's approver group the platform has asked and been answered before this runs.
                let by_policy = name == "approve_request" && s(doc, "route") == Some("Group");
                if !by_policy && !decides(hc.ctx) {
                    return Err(Error::Forbidden("a Change Manager decides a request".into()));
                }
                if !by_policy && mine {
                    return Err(Error::Forbidden("a request is decided by someone other than who asked".into()));
                }
                if name == "approve_request" {
                    hc.kernel
                        .set_values_tx(&elevated(hc.ctx), hc.tx, "CatalogRequest", request, obj(json!({"status": "Approved", "decided_by": hc.ctx.email})))
                        .await?;
                    make_tasks(hc, doc).await?;
                    return Ok(Some(json!({"status": "Approved"})));
                } else {
                    let why = note.ok_or_else(|| Error::field("note", "a rejection says why"))?;
                    json!({"status": "Rejected", "decided_by": hc.ctx.email, "decision_note": why})
                }
            }
            "fulfil" => {
                if needs && s(doc, "status") != Some("Approved") {
                    return Err(Error::invalid("this request waits for its approval"));
                }
                let undone: i64 = sqlx::query_scalar("select count(*) from e_request_task where tenant_id = $1 and request = $2 and status <> 'Done'")
                    .bind(hc.ctx.tenant)
                    .bind(request)
                    .fetch_one(&mut **hc.tx)
                    .await?;
                if undone > 0 {
                    return Err(Error::invalid("its tasks are not all done: the request is fulfilled when they are"));
                }
                json!({"status": "Fulfilled", "fulfilled_at": Utc::now().to_rfc3339()})
            }
            "cancel_request" => json!({"status": "Cancelled"}),
            _ => return Ok(None),
        };
        let status = values["status"].clone();
        hc.kernel.set_values_tx(&elevated(hc.ctx), hc.tx, "CatalogRequest", request, obj(values)).await?;
        Ok(Some(json!({"status": status})))
    }
}

pub struct IncidentHook;

#[async_trait]
impl DocHook for IncidentHook {
    fn name(&self) -> &'static str {
        "itsm.incident"
    }

    fn fills(&self) -> &'static [&'static str] {
        &["matrix", "matrix_missing", "priority", "major", "caller_email", "opened_at", "resolved_at"]
    }

    async fn before_validate(&self, hc: &mut HookCtx<'_>, doc: &mut Doc) -> Result<()> {
        people(hc, doc).await?;
        crate::defaults::ensure_calendar(hc.tx, hc.ctx.tenant).await?;
        let row: Option<(Uuid, String)> = sqlx::query_as("select id, priority from e_priority_matrix where tenant_id = $1 and impact = $2 and urgency = $3")
            .bind(hc.ctx.tenant)
            .bind(s(doc, "impact").unwrap_or("Medium"))
            .bind(s(doc, "urgency").unwrap_or("Medium"))
            .fetch_optional(&mut **hc.tx)
            .await?;
        // A matrix with no row for this pair (a workspace deleted it, or has not been given its rows yet) never
        // refuses the incident: it takes P3, what the default impact and urgency (Medium, Medium) give in the shipped
        // matrix, with that row's targets (the entity's `within` expressions), and says so in `matrix_missing`.
        let missing = row.is_none();
        let (matrix, p) = row.map_or((Value::Null, "P3".to_string()), |(m, p)| (json!(m), p));
        let p = p.as_str();
        let opened = at(doc, "opened_at").unwrap_or_else(Utc::now);
        let critical: Option<String> = sqlx::query_scalar("select criticality from e_configuration_item where tenant_id = $1 and id = $2")
            .bind(hc.ctx.tenant)
            .bind(id(doc, "ci"))
            .fetch_optional(&mut **hc.tx)
            .await?
            .flatten();
        let done = matches!(s(doc, "status"), Some("Resolved" | "Closed"));
        let resolved = if done { Some(at(doc, "resolved_at").unwrap_or_else(Utc::now)) } else { None };
        doc.insert("matrix".into(), matrix);
        doc.insert("matrix_missing".into(), json!(missing));
        doc.insert("priority".into(), json!(p));
        doc.insert(
            "major".into(),
            json!((p == "P1" && critical.as_deref() == Some("Critical")) || doc.get("major_declared").and_then(Value::as_bool) == Some(true)),
        );
        doc.insert("opened_at".into(), json!(opened.to_rfc3339()));
        doc.insert("resolved_at".into(), resolved.map_or(Value::Null, |r| json!(r.to_rfc3339())));
        Ok(())
    }

    async fn validate(&self, _hc: &mut HookCtx<'_>, doc: &Doc) -> Result<()> {
        if matches!(s(doc, "status"), Some("Resolved" | "Closed")) && s(doc, "resolution").is_none() {
            return Err(Error::field("resolution", "a resolved incident says how"));
        }
        Ok(())
    }
}

pub struct ProblemHook;

#[async_trait]
impl DocHook for ProblemHook {
    fn name(&self) -> &'static str {
        "itsm.problem"
    }

    async fn validate(&self, _hc: &mut HookCtx<'_>, doc: &Doc) -> Result<()> {
        let known = doc.get("known_error").and_then(Value::as_bool) == Some(true) || s(doc, "status") == Some("Known Error");
        if known && s(doc, "workaround").is_none() {
            return Err(Error::field("workaround", "a known error has its workaround"));
        }
        match s(doc, "status") {
            Some("Resolved" | "Closed") if s(doc, "root_cause").is_none() => Err(Error::field("root_cause", "a resolved problem says its root cause")),
            _ => Ok(()),
        }
    }

    async fn after_save(&self, hc: &mut HookCtx<'_>, doc: &Doc, old: Option<&Doc>) -> Result<()> {
        let problem = id(doc, "id").ok_or_else(|| Error::internal("a problem without an id"))?;
        publish(hc, doc, problem).await?;
        let resolved = matches!(s(doc, "status"), Some("Resolved" | "Closed"));
        let was = old.is_some_and(|o| matches!(s(o, "status"), Some("Resolved" | "Closed")));
        if !resolved || was {
            return Ok(());
        }
        // Which a workspace chooses (settings, Closing a Problem): the incidents still open under it are resolved with
        // it (what a workspace upgraded from 0.3 keeps), or left for the button the problem offers.
        let offer: Option<Option<String>> = sqlx::query_scalar("select problem_closes_incidents from e_itsm_settings where tenant_id = $1 limit 1")
            .bind(hc.ctx.tenant)
            .fetch_optional(&mut **hc.tx)
            .await?;
        if offer.flatten().as_deref() == Some("Offer") {
            return Ok(());
        }
        let open: Vec<Uuid> =
            sqlx::query_scalar("select id from e_incident where tenant_id = $1 and problem = $2 and status in ('New', 'In Progress', 'On Hold')")
                .bind(hc.ctx.tenant)
                .bind(problem)
                .fetch_all(&mut **hc.tx)
                .await?;
        let system = elevated(hc.ctx);
        let note = format!("Resolved with {}: {}", s(doc, "number").unwrap_or("its problem"), s(doc, "root_cause").unwrap_or_default());
        for incident in open {
            hc.kernel.update_tx(&system, hc.tx, "Incident", incident, obj(json!({"status": "Resolved", "resolution": note})), None).await?;
        }
        Ok(())
    }
}

/// Keeps the problem's published workaround in step: a known error whose workaround is marked visible has a
/// `KnownError` row, published, that the service desk's search and the portal's suggestions find; unmarked (or no
/// longer a known error) the row stays but is unpublished, so nothing the portal showed is lost to the desk.
async fn publish(hc: &mut HookCtx<'_>, doc: &Doc, problem: Uuid) -> Result<()> {
    let known = doc.get("known_error").and_then(Value::as_bool) == Some(true) || s(doc, "status") == Some("Known Error");
    let show = known && doc.get("workaround_visible").and_then(Value::as_bool) == Some(true) && s(doc, "workaround").is_some();
    let there: Option<Uuid> = sqlx::query_scalar("select id from e_known_error where tenant_id = $1 and problem = $2")
        .bind(hc.ctx.tenant)
        .bind(problem)
        .fetch_optional(&mut **hc.tx)
        .await?;
    let system = elevated(hc.ctx);
    let values =
        json!({"title": s(doc, "title"), "service": id(doc, "ci"), "symptoms": s(doc, "symptoms"), "workaround": s(doc, "workaround"), "published": show});
    match (there, show) {
        (Some(row), _) => {
            let mut v = obj(values);
            if !show {
                v.remove("workaround");
                v.remove("title");
            }
            hc.kernel.update_tx(&system, hc.tx, "KnownError", row, v, None).await?;
        }
        (None, true) => {
            let mut v = obj(values);
            v.insert("problem".into(), json!(problem));
            hc.kernel.insert_tx(&system, hc.tx, "KnownError", v).await?;
        }
        (None, false) => {}
    }
    Ok(())
}

/// The tasks of a request, made once from its offering's steps: the first sequence number opens (all of them, when the
/// offering runs its steps at once), the rest wait. Nothing is made twice (a request that has tasks is left as it is).
async fn make_tasks(hc: &mut HookCtx<'_>, request: &Doc) -> Result<()> {
    let Some(rid) = id(request, "id") else { return Ok(()) };
    let made: bool = sqlx::query_scalar("select exists (select 1 from e_request_task where tenant_id = $1 and request = $2)")
        .bind(hc.ctx.tenant)
        .bind(rid)
        .fetch_one(&mut **hc.tx)
        .await?;
    if made {
        return Ok(());
    }
    let at_once: Option<Option<String>> = sqlx::query_scalar("select steps_run from e_service_offering where tenant_id = $1 and id = $2")
        .bind(hc.ctx.tenant)
        .bind(id(request, "offering"))
        .fetch_optional(&mut **hc.tx)
        .await?;
    let at_once = at_once.flatten().as_deref() == Some("At Once");
    let steps: Vec<(String, Option<i64>, Option<Uuid>, Option<i64>, Option<String>)> = sqlx::query_as(
        "select step_name, sequence, assignment_group, target_hours, instructions from e_service_offering__steps where tenant_id = $1 and parent_id = $2 order by sequence, idx",
    )
    .bind(hc.ctx.tenant)
    .bind(id(request, "offering"))
    .fetch_all(&mut **hc.tx)
    .await?;
    let first = steps.first().and_then(|s| s.1);
    let system = elevated(hc.ctx);
    for (name, seq, group, hours, how) in steps {
        let status = if at_once || seq == first { "Open" } else { "Waiting" };
        hc.kernel
            .insert_tx(
                &system,
                hc.tx,
                "RequestTask",
                obj(json!({"request": rid, "step_name": name, "sequence": seq.unwrap_or(10), "assignment_group": group, "target_hours": hours.unwrap_or(8), "instructions": how, "status": status})),
            )
            .await?;
    }
    Ok(())
}

/// A task is done by its group; when none of the request's tasks is left, the request is fulfilled, and when the
/// offering runs its steps in order the next sequence number opens.
pub struct TaskHook;

#[async_trait]
impl DocHook for TaskHook {
    fn name(&self) -> &'static str {
        "itsm.task"
    }

    async fn action(&self, hc: &mut HookCtx<'_>, name: &str, doc: &Doc, _params: &Value) -> Result<Option<Value>> {
        if name != "complete_task" {
            return Ok(None);
        }
        let (task, request) = (
            id(doc, "id").ok_or_else(|| Error::internal("a task without an id"))?,
            id(doc, "request").ok_or_else(|| Error::internal("a task without a request"))?,
        );
        let system = elevated(hc.ctx);
        hc.kernel.update_tx(&system, hc.tx, "RequestTask", task, obj(json!({"status": "Done", "done_at": Utc::now().to_rfc3339()})), None).await?;
        let left: Vec<(Uuid, String, Option<i64>)> =
            sqlx::query_as("select id, status, sequence from e_request_task where tenant_id = $1 and request = $2 and status <> 'Done' order by sequence")
                .bind(hc.ctx.tenant)
                .bind(request)
                .fetch_all(&mut **hc.tx)
                .await?;
        if left.is_empty() {
            hc.kernel
                .update_tx(&system, hc.tx, "CatalogRequest", request, obj(json!({"status": "Fulfilled", "fulfilled_at": Utc::now().to_rfc3339()})), None)
                .await?;
            return Ok(Some(json!({"status": "Done", "request": "Fulfilled"})));
        }
        if !left.iter().any(|t| t.1 == "Open") {
            let next = left.first().and_then(|t| t.2);
            for (t, _, _) in left.iter().filter(|t| t.2 == next) {
                hc.kernel.update_tx(&system, hc.tx, "RequestTask", *t, obj(json!({"status": "Open"})), None).await?;
            }
        }
        Ok(Some(json!({"status": "Done"})))
    }
}
