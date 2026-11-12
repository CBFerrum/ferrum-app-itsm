//! Changes. A change has a type, and the type a path: Pre-approved (from an approved template, no one decides),
//! Board (N of the members of a group record) or Emergency (a smaller group, reviewed afterwards). Which of these
//! asks whom is the change's `route`, worked out when it is saved while a draft; the approval itself is the
//! platform's, a policy the package ships on the action `approve_change` (package.yaml, `approvals`), so this file
//! holds the facts the policy reads (the route, the quorum, the members of the group as they were when the change
//! was raised) and what the platform cannot know: the questionnaire's score, the calendar, the freeze, a clash
//! with another change on the same item or service, and the review.
//!
//! With the setting Single Manager (what a workspace upgraded from 0.3 keeps) the route is Manager, no step of the
//! policy applies and one Change Manager other than who raised the change approves it, as before. A change is never
//! approved here without either: Manager's check, a platform approval that has completed, or a Pre-approved route.

use crate::cmdb::impact;
use crate::{at, obj};
use async_trait::async_trait;
use chrono::Utc;
use ferrum_domain::util::{elevated, id, rows, s};
use ferrum_kernel::{Doc, DocHook, Error, HookCtx, Result, Tx};
use serde_json::{Value, json};
use uuid::Uuid;

pub struct ChangeHook;

/// What the settings say about changes (an unsaved settings record says the defaults).
struct Cfg {
    board: bool,
    freeze_warns: bool,
    medium: i64,
    high: i64,
}

async fn cfg(tx: &mut Tx, tenant: Uuid) -> Result<Cfg> {
    let row: Option<(Option<String>, Option<String>, Option<i64>, Option<i64>)> =
        sqlx::query_as("select change_approval, freeze_action, score_medium_from, score_high_from from e_itsm_settings where tenant_id = $1 limit 1")
            .bind(tenant)
            .fetch_optional(&mut **tx)
            .await?;
    let (a, f, m, h) = row.unwrap_or_default();
    Ok(Cfg { board: a.as_deref() == Some("Change Board"), freeze_warns: f.as_deref() == Some("Warn"), medium: m.unwrap_or(4), high: h.unwrap_or(8) })
}

/// The path a class (`kind`) takes when no change type of the workspace says otherwise.
fn path_of(kind: &str) -> &'static str {
    match kind {
        "Standard" => "Pre-approved",
        "Emergency" => "Emergency",
        _ => "Board",
    }
}

fn kind_of(path: &str) -> &'static str {
    match path {
        "Pre-approved" => "Standard",
        "Emergency" => "Emergency",
        _ => "Normal",
    }
}

/// A change type as a change reads it.
struct Ty {
    id: Option<Uuid>,
    path: String,
    board: Option<Uuid>,
    quorum: i64,
    high_group: Option<Uuid>,
    review: bool,
}

type TyRow = (Uuid, String, Option<Uuid>, Option<i64>, Option<Uuid>, Option<bool>);
macro_rules! ty_sql {
    ($rest:literal) => {
        concat!("select id, path, board, quorum, high_risk_group, review_after from e_change_type where tenant_id = $1", $rest)
    };
}

fn ty(r: TyRow) -> Ty {
    Ty { id: Some(r.0), path: r.1, board: r.2, quorum: r.3.unwrap_or(1), high_group: r.4, review: r.5.unwrap_or(false) }
}

/// The change's type: the one it names; else the one named like its class; else the first of the class's path; else,
/// for a workspace that has none yet (not upgraded so far), the class's own path with no group.
async fn resolve_type(tx: &mut Tx, tenant: Uuid, doc: &Doc) -> Result<Ty> {
    if let Some(t) = id(doc, "change_type") {
        let r: Option<TyRow> = sqlx::query_as(ty_sql!(" and id = $2")).bind(tenant).bind(t).fetch_optional(&mut **tx).await?;
        return r.map(ty).ok_or_else(|| Error::field("change_type", "is not a change type"));
    }
    let kind = s(doc, "kind").unwrap_or("Normal");
    let by_name: Option<TyRow> = sqlx::query_as(ty_sql!(" and type_name = $2")).bind(tenant).bind(kind).fetch_optional(&mut **tx).await?;
    if let Some(r) = by_name {
        return Ok(ty(r));
    }
    let by_path: Option<TyRow> = sqlx::query_as(ty_sql!(" and path = $2 and coalesce(active, true) order by created_at limit 1"))
        .bind(tenant)
        .bind(path_of(kind))
        .fetch_optional(&mut **tx)
        .await?;
    Ok(by_path.map(ty).unwrap_or(Ty { id: None, path: path_of(kind).into(), board: None, quorum: 1, high_group: None, review: false }))
}

/// The e-mail addresses of a group's members.
async fn members(tx: &mut Tx, tenant: Uuid, group: Option<Uuid>) -> Result<Vec<String>> {
    let Some(group) = group else { return Ok(vec![]) };
    Ok(sqlx::query_scalar("select lower(member) from e_it_group__members where tenant_id = $1 and parent_id = $2 order by idx")
        .bind(tenant)
        .bind(group)
        .fetch_all(&mut **tx)
        .await?)
}

/// Works the risk and impact out of the answered questions: each answer's score from the question's choices, the
/// sums against the settings' thresholds. Nothing changes when no question is answered (risk is then picked by hand).
async fn score(tx: &mut Tx, tenant: Uuid, c: &Cfg, doc: &mut Doc) -> Result<()> {
    let Some(answers) = doc.get("risk_answers").and_then(Value::as_array).cloned().filter(|a| !a.is_empty()) else { return Ok(()) };
    let (mut risk, mut impact, mut has_risk, mut has_impact) = (0, 0, false, false);
    let mut graded = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for a in answers.iter().filter_map(Value::as_object) {
        let (q, choice) = (s(a, "question").unwrap_or_default().trim(), s(a, "answer").unwrap_or_default().trim());
        if seen.iter().any(|x| x.eq_ignore_ascii_case(q)) {
            return Err(Error::field("risk_answers", format!("{q} is answered twice")));
        }
        seen.push(q.to_string());
        let found: Option<(String, i64)> = sqlx::query_as(
            "select q.measures, c.score from e_risk_question q join e_risk_question__choices c on c.tenant_id = q.tenant_id and c.parent_id = q.id \
             where q.tenant_id = $1 and lower(q.question_text) = lower($2) and lower(c.choice_label) = lower($3) and coalesce(q.active, true)",
        )
        .bind(tenant)
        .bind(q)
        .bind(choice)
        .fetch_optional(&mut **tx)
        .await?;
        let (measures, n) = found.ok_or_else(|| Error::field("risk_answers", format!("{q}: {choice} is not an answer to a question of the questionnaire")))?;
        if measures == "Impact" {
            (impact, has_impact) = (impact + n, true);
        } else {
            (risk, has_risk) = (risk + n, true);
        }
        let mut row = a.clone();
        row.insert("score".into(), json!(n));
        graded.push(Value::Object(row));
    }
    let level = |n: i64| {
        if n >= c.high {
            "High"
        } else if n >= c.medium {
            "Medium"
        } else {
            "Low"
        }
    };
    doc.insert("risk_answers".into(), Value::Array(graded));
    if has_risk {
        doc.insert("risk_score".into(), json!(risk));
        doc.insert("risk".into(), json!(level(risk)));
    }
    if has_impact {
        doc.insert("impact_score".into(), json!(impact));
        doc.insert("impact_level".into(), json!(level(impact)));
    }
    Ok(())
}

/// The items a change touches: the one it changes and those it also affects.
fn touched(doc: &Doc) -> Vec<Uuid> {
    let mut v: Vec<Uuid> = id(doc, "ci").into_iter().collect();
    v.extend(rows(doc, "affected").iter().filter_map(|r| id(r, "item")));
    v
}

/// What the calendar says of the change's window: the other scheduled changes (submitted or approved) in it on one of
/// the same items or services, and the freezes it falls in.
async fn flags(tx: &mut Tx, tenant: Uuid, doc: &Doc) -> Result<Option<String>> {
    let (Some(start), Some(end)) = (at(doc, "planned_start"), at(doc, "planned_end")) else { return Ok(None) };
    let items = touched(doc);
    let me = id(doc, "id").unwrap_or_else(Uuid::nil);
    let clash: Vec<String> = sqlx::query_scalar(
        "select c.number from e_it_change c where c.tenant_id = $1 and c.id <> $2 and c.status in ('Submitted', 'Approved') and c.planned_start < $4 and c.planned_end > $3 \
         and (c.ci = any($5) or exists (select 1 from e_it_change__affected x where x.tenant_id = c.tenant_id and x.parent_id = c.id and x.item = any($5))) order by c.number",
    )
    .bind(tenant)
    .bind(me)
    .bind(start)
    .bind(end)
    .bind(&items)
    .fetch_all(&mut **tx)
    .await?;
    let frozen: Vec<String> =
        sqlx::query_scalar("select reason from e_change_freeze where tenant_id = $1 and starts_at < $3 and ends_at > $2 order by starts_at")
            .bind(tenant)
            .bind(start)
            .bind(end)
            .fetch_all(&mut **tx)
            .await?;
    let mut out = Vec::new();
    if !clash.is_empty() {
        out.push(format!("Same window, same service or item: {}", clash.join(", ")));
    }
    if !frozen.is_empty() {
        out.push(format!("In the freeze for {}", frozen.join(", ")));
    }
    Ok((!out.is_empty()).then(|| out.join(". ")))
}

/// Refused when the change's window clashes: inside a freeze (unless an emergency, and not at all when the settings
/// only warn), or, once it is to be approved (`approving`), over another approved change of the same item.
async fn clear(hc: &mut HookCtx<'_>, doc: &Doc, approving: bool) -> Result<()> {
    let (start, end) = (at(doc, "planned_start"), at(doc, "planned_end"));
    let (Some(start), Some(end)) = (start, end) else { return Ok(()) };
    if s(doc, "path").map_or(s(doc, "kind") != Some("Emergency"), |p| p != "Emergency") && !cfg(hc.tx, hc.ctx.tenant).await?.freeze_warns {
        let freeze: Option<String> = sqlx::query_scalar("select reason from e_change_freeze where tenant_id = $1 and starts_at < $3 and ends_at > $2 limit 1")
            .bind(hc.ctx.tenant)
            .bind(start)
            .bind(end)
            .fetch_optional(&mut **hc.tx)
            .await?;
        if let Some(reason) = freeze {
            return Err(Error::field("planned_start", format!("falls in the freeze for {reason}: only emergencies then")));
        }
    }
    if !approving {
        return Ok(());
    }
    let other: Option<String> = sqlx::query_scalar(
        "select number from e_it_change where tenant_id = $1 and ci = $2 and id <> $3 and status = 'Approved' and planned_start < $5 and planned_end > $4 limit 1",
    )
    .bind(hc.ctx.tenant)
    .bind(id(doc, "ci"))
    .bind(id(doc, "id").unwrap_or_else(Uuid::nil))
    .bind(start)
    .bind(end)
    .fetch_optional(&mut **hc.tx)
    .await?;
    if let Some(n) = other {
        return Err(Error::field("planned_start", format!("{n} changes the same item then")));
    }
    Ok(())
}

#[async_trait]
impl DocHook for ChangeHook {
    fn name(&self) -> &'static str {
        "itsm.change"
    }

    fn fills(&self) -> &'static [&'static str] {
        &[
            "impacted",
            "window_flags",
            "change_type",
            "kind",
            "path",
            "route",
            "quorum",
            "board_members",
            "escalation_members",
            "second_look",
            "review_required",
            "risk",
            "risk_score",
            "impact_score",
            "impact_level",
            "risk_answers",
            "change_plan",
            "backout_plan",
            "test_plan",
        ]
    }

    async fn before_validate(&self, hc: &mut HookCtx<'_>, doc: &mut Doc) -> Result<()> {
        let tenant = hc.ctx.tenant;
        if let Some(ci) = id(doc, "ci") {
            let d = depth(hc.tx, tenant).await?;
            let names = impact(hc.tx, tenant, ci, d).await?;
            doc.insert("impacted".into(), if names.is_empty() { Value::Null } else { json!(names.join(", ")) });
        }
        let status = s(doc, "status").unwrap_or("Draft").to_string();
        let status = status.as_str();
        if status == "Draft" {
            let c = cfg(hc.tx, tenant).await?;
            let t = resolve_type(hc.tx, tenant, doc).await?;
            doc.insert("change_type".into(), t.id.map_or(Value::Null, |i| json!(i)));
            doc.insert("path".into(), json!(t.path));
            doc.insert("kind".into(), json!(kind_of(&t.path)));
            // A template fills the plans a person left empty.
            if let Some(tpl) = id(doc, "template") {
                let r: Option<(bool, Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
                    "select coalesce(approved, false), change_plan, backout_plan, test_plan from e_change_template where tenant_id = $1 and id = $2",
                )
                .bind(tenant)
                .bind(tpl)
                .fetch_optional(&mut **hc.tx)
                .await?;
                let (_, plan, back, test) = r.ok_or_else(|| Error::field("template", "is not a change template"))?;
                for (k, v) in [("change_plan", plan), ("backout_plan", back), ("test_plan", test)] {
                    if s(doc, k).is_none()
                        && let Some(v) = v
                    {
                        doc.insert(k.into(), json!(v));
                    }
                }
            }
            score(hc.tx, tenant, &c, doc).await?;
            let approved_template = match id(doc, "template") {
                Some(tpl) => {
                    sqlx::query_scalar("select coalesce(approved, false) and coalesce(active, true) from e_change_template where tenant_id = $1 and id = $2")
                        .bind(tenant)
                        .bind(tpl)
                        .fetch_optional(&mut **hc.tx)
                        .await?
                        .unwrap_or(false)
                }
                None => false,
            };
            let route = match (c.board, t.path.as_str(), approved_template) {
                (false, "Pre-approved", _) => "Pre-approved",
                (false, _, _) => "Manager",
                (true, "Pre-approved", true) => "Pre-approved",
                (true, "Pre-approved", false) => "Board",
                (true, p, _) => {
                    if p == "Emergency" {
                        "Emergency"
                    } else {
                        "Board"
                    }
                }
            };
            let asks = matches!(route, "Board" | "Emergency");
            let second = route == "Board" && s(doc, "risk") == Some("High") && t.high_group.is_some();
            doc.insert("route".into(), json!(route));
            doc.insert("quorum".into(), if asks { json!(t.quorum) } else { Value::Null });
            doc.insert("board_members".into(), if asks { json!(members(hc.tx, tenant, t.board).await?) } else { Value::Null });
            doc.insert("escalation_members".into(), if second { json!(members(hc.tx, tenant, t.high_group).await?) } else { Value::Null });
            doc.insert("second_look".into(), json!(second));
            doc.insert("review_required".into(), json!(t.review));
        }
        if !matches!(status, "Closed" | "Rejected" | "Failed") {
            let f = flags(hc.tx, tenant, doc).await?;
            doc.insert("window_flags".into(), f.map_or(Value::Null, |f| json!(f)));
        }
        Ok(())
    }

    async fn validate(&self, hc: &mut HookCtx<'_>, doc: &Doc) -> Result<()> {
        if let (Some(start), Some(end)) = (at(doc, "planned_start"), at(doc, "planned_end"))
            && end <= start
        {
            return Err(Error::field("planned_end", "is after the change starts"));
        }
        if s(doc, "path").map_or(s(doc, "kind") != Some("Standard"), |p| p != "Pre-approved") && s(doc, "backout_plan").is_none() {
            return Err(Error::field("backout_plan", "a normal or emergency change says how it is undone"));
        }
        if matches!(s(doc, "status"), Some("Draft" | "Submitted") | None) {
            clear(hc, doc, false).await?;
        }
        Ok(())
    }

    async fn validate_change(&self, hc: &mut HookCtx<'_>, old: Option<&Doc>, doc: &Doc) -> Result<()> {
        if hc.ctx.is_system {
            return Ok(());
        }
        if let Some(o) = old
            && !matches!(s(o, "status"), Some("Draft") | None)
            && ["ci", "kind", "change_type", "planned_start", "planned_end", "backout_plan"].iter().any(|k| s(o, k) != s(doc, k))
        {
            return Err(Error::invalid("a submitted change keeps its item, kind, window and back-out plan"));
        }
        Ok(())
    }

    async fn action(&self, hc: &mut HookCtx<'_>, name: &str, doc: &Doc, params: &Value) -> Result<Option<Value>> {
        let change = id(doc, "id").ok_or_else(|| Error::internal("a change without an id"))?;
        let note = params.get("note").and_then(Value::as_str).map(str::trim).filter(|n| !n.is_empty());
        let decider = hc.ctx.is_admin || hc.ctx.roles.iter().any(|r| r == "Change Manager");
        let raised_it = s(doc, "created_by").and_then(|c| c.parse::<Uuid>().ok()) == Some(hc.ctx.user);
        let route = s(doc, "route").unwrap_or(if s(doc, "kind") == Some("Standard") { "Pre-approved" } else { "Manager" });
        let mut answer = json!({});
        let values = match name {
            "submit_change" if route == "Pre-approved" => {
                clear(hc, doc, true).await?;
                let why =
                    if id(doc, "template").is_some() { "From an approved template, approved in advance" } else { "A standard change, approved in advance" };
                json!({"status": "Approved", "decision_note": why})
            }
            "submit_change" => {
                if matches!(route, "Board" | "Emergency") {
                    let (have, need) =
                        (doc.get("board_members").and_then(Value::as_array).map_or(0, Vec::len) as i64, doc.get("quorum").and_then(Value::as_i64).unwrap_or(1));
                    if have < need {
                        return Err(Error::field(
                            "change_type",
                            format!("its approving group has {have} members and the type asks {need} of them to approve: add members to the group"),
                        ));
                    }
                    if doc.get("second_look").and_then(Value::as_bool) == Some(true)
                        && doc.get("escalation_members").and_then(Value::as_array).is_none_or(Vec::is_empty)
                    {
                        return Err(Error::field(
                            "change_type",
                            "a high-risk change is also approved by a member of the type's high-risk group, which has none",
                        ));
                    }
                }
                json!({"status": "Submitted"})
            }
            "approve_change" | "reject_change" => {
                // With the board or the emergency group the platform has asked and been answered before this runs.
                let by_policy = name == "approve_change" && matches!(route, "Board" | "Emergency");
                if !by_policy {
                    if !decider {
                        return Err(Error::Forbidden("a Change Manager decides a change".into()));
                    }
                    if raised_it {
                        return Err(Error::Forbidden("a change is decided by someone other than who raised it".into()));
                    }
                }
                if name == "approve_change" {
                    clear(hc, doc, true).await?;
                    if by_policy {
                        let who: Option<String> = sqlx::query_scalar(
                            "select string_agg(distinct u.email, ', ') from approval_decision d join approval_request r on r.tenant_id = d.tenant_id and r.id = d.request_id \
                             join app_user u on u.tenant_id = d.tenant_id and u.id = d.decider where r.tenant_id = $1 and r.entity = 'ItChange' and r.record_id = $2 \
                             and r.action = 'approve_change' and not d.void and d.outcome = 'approve'",
                        )
                        .bind(hc.ctx.tenant)
                        .bind(change)
                        .fetch_one(&mut **hc.tx)
                        .await?;
                        json!({"status": "Approved", "decision_note": who.map(|w| format!("Approved by {w}"))})
                    } else {
                        json!({"status": "Approved", "decided_by": hc.ctx.email, "decision_note": note})
                    }
                } else {
                    json!({"status": "Rejected", "decided_by": hc.ctx.email, "decision_note": note.ok_or_else(|| Error::field("note", "a rejection says why"))?})
                }
            }
            "implement" => json!({"status": "Implemented", "implemented_at": Utc::now().to_rfc3339()}),
            "close_change" => {
                if doc.get("review_required").and_then(Value::as_bool) == Some(true) && s(doc, "review_outcome").is_none() {
                    return Err(Error::invalid("a change of this type is reviewed before it is closed: use Review It"));
                }
                json!({"status": "Closed", "decision_note": note.or(s(doc, "decision_note"))})
            }
            "fail_change" => json!({"status": "Failed", "decision_note": note.ok_or_else(|| Error::field("note", "a failed change says what happened"))?}),
            "review_change" => {
                let outcome = params.get("outcome").and_then(Value::as_str).unwrap_or_default();
                if !["Successful", "Successful with Issues", "Failed"].contains(&outcome) {
                    return Err(Error::field("outcome", "is one of Successful, Successful with Issues, Failed"));
                }
                if outcome == "Failed" && note.is_none() {
                    return Err(Error::field("note", "a failed change says what happened"));
                }
                if outcome == "Failed" && params.get("open_incident").and_then(Value::as_bool) == Some(true) {
                    let incident = hc
                        .kernel
                        .insert_tx(
                            &elevated(hc.ctx),
                            hc.tx,
                            "Incident",
                            obj(json!({
                                "title": format!("{} {}: {}", s(doc, "number").unwrap_or("Change"), "failed", s(doc, "title").unwrap_or_default()),
                                "description": note, "ci": id(doc, "ci"), "change": change, "impact": "High", "urgency": "Medium",
                                "caller_email": (!hc.ctx.email.is_empty()).then(|| hc.ctx.email.to_lowercase()),
                            })),
                        )
                        .await?;
                    answer["incident"] = incident.get("id").cloned().unwrap_or(Value::Null);
                }
                json!({"status": if outcome == "Failed" { "Failed" } else { "Closed" }, "review_outcome": outcome, "review_notes": note,
                       "reviewed_at": Utc::now().to_rfc3339(), "reviewed_by": hc.ctx.email})
            }
            _ => return Ok(None),
        };
        answer["status"] = values["status"].clone();
        hc.kernel.set_values_tx(&elevated(hc.ctx), hc.tx, "ItChange", change, obj(values)).await?;
        Ok(Some(answer))
    }
}

/// How far out from an item the impact route goes (the settings' `impact_depth`).
pub(crate) async fn depth(tx: &mut Tx, tenant: Uuid) -> Result<i64> {
    let d: Option<Option<i64>> =
        sqlx::query_scalar("select impact_depth from e_itsm_settings where tenant_id = $1 limit 1").bind(tenant).fetch_optional(&mut **tx).await?;
    Ok(d.flatten().unwrap_or(10))
}

pub struct FreezeHook;

#[async_trait]
impl DocHook for FreezeHook {
    fn name(&self) -> &'static str {
        "itsm.freeze"
    }

    async fn validate(&self, _hc: &mut HookCtx<'_>, doc: &Doc) -> Result<()> {
        if let (Some(start), Some(end)) = (at(doc, "starts_at"), at(doc, "ends_at"))
            && end <= start
        {
            return Err(Error::field("ends_at", "is after the freeze starts"));
        }
        Ok(())
    }
}
