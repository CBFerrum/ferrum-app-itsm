//! The service desk: catalog requests, incidents and problems.
//!
//! * A catalog request is due its offering's hours after it is asked; one that needs approval is
//!   fulfilled only once approved, by a Change Manager who did not ask for it.
//! * An incident's impact and urgency give its priority (the usual matrix), its priority the hours
//!   it is to be resolved in, and a P1 on a critical item is major. Resolved, it says how; one
//!   resolved late, or still open past its time, has breached, and stays breached.
//! * A known error has its workaround; a problem resolved says its root cause and resolves the
//!   incidents still open under it.

use crate::{at, obj};
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use ferrum_domain::util::{elevated, id, s};
use ferrum_kernel::{Ctx, Doc, DocHook, Error, HookCtx, Kernel, Result, Tx};
use serde_json::{Value, json};
use uuid::Uuid;

/// The priority of an impact and an urgency.
pub(crate) fn priority(impact: &str, urgency: &str) -> &'static str {
    let rank = |v: &str| match v {
        "High" => 0,
        "Medium" => 1,
        _ => 2,
    };
    match rank(impact) + rank(urgency) {
        0 => "P1",
        1 => "P2",
        2 => "P3",
        _ => "P4",
    }
}

/// The hours a priority is to be resolved in.
pub(crate) fn hours(priority: &str) -> i64 {
    match priority {
        "P1" => 4,
        "P2" => 8,
        "P3" => 24,
        _ => 72,
    }
}

fn decides(ctx: &Ctx) -> bool {
    ctx.is_admin || ctx.roles.iter().any(|r| r == "Change Manager")
}

pub struct RequestHook;

#[async_trait]
impl DocHook for RequestHook {
    fn name(&self) -> &'static str {
        "itsm.request"
    }

    fn fills(&self) -> &'static [&'static str] {
        &["requested_by", "due_at"]
    }

    async fn before_validate(&self, hc: &mut HookCtx<'_>, doc: &mut Doc) -> Result<()> {
        if s(doc, "requested_by").is_none() && !hc.ctx.email.is_empty() {
            doc.insert("requested_by".into(), json!(hc.ctx.email.to_lowercase()));
        }
        if s(doc, "due_at").is_none() {
            let h: Option<i64> = sqlx::query_scalar("select fulfil_hours from e_service_offering where tenant_id = $1 and id = $2")
                .bind(hc.ctx.tenant)
                .bind(id(doc, "offering"))
                .fetch_optional(&mut **hc.tx)
                .await?
                .flatten();
            doc.insert("due_at".into(), json!((Utc::now() + Duration::hours(h.unwrap_or(24))).to_rfc3339()));
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
                if !decides(hc.ctx) {
                    return Err(Error::Forbidden("a Change Manager decides a request".into()));
                }
                if mine {
                    return Err(Error::Forbidden("a request is decided by someone other than who asked".into()));
                }
                if name == "approve_request" {
                    json!({"status": "Approved", "decided_by": hc.ctx.email})
                } else {
                    let why = note.ok_or_else(|| Error::field("note", "a rejection says why"))?;
                    json!({"status": "Rejected", "decided_by": hc.ctx.email, "decision_note": why})
                }
            }
            "fulfil" => {
                if needs && s(doc, "status") != Some("Approved") {
                    return Err(Error::invalid("this request waits for its approval"));
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
        &["priority", "major", "opened_at", "resolve_by", "resolved_at", "breached"]
    }

    async fn before_validate(&self, hc: &mut HookCtx<'_>, doc: &mut Doc) -> Result<()> {
        let p = priority(s(doc, "impact").unwrap_or("Medium"), s(doc, "urgency").unwrap_or("Medium"));
        let opened = at(doc, "opened_at").unwrap_or_else(Utc::now);
        let due = opened + Duration::hours(hours(p));
        let critical: Option<String> = sqlx::query_scalar("select criticality from e_configuration_item where tenant_id = $1 and id = $2")
            .bind(hc.ctx.tenant)
            .bind(id(doc, "ci"))
            .fetch_optional(&mut **hc.tx)
            .await?
            .flatten();
        let done = matches!(s(doc, "status"), Some("Resolved" | "Closed"));
        let resolved = if done { Some(at(doc, "resolved_at").unwrap_or_else(Utc::now)) } else { None };
        doc.insert("priority".into(), json!(p));
        doc.insert("major".into(), json!(p == "P1" && critical.as_deref() == Some("Critical")));
        doc.insert("opened_at".into(), json!(opened.to_rfc3339()));
        doc.insert("resolve_by".into(), json!(due.to_rfc3339()));
        doc.insert("resolved_at".into(), resolved.map_or(Value::Null, |r| json!(r.to_rfc3339())));
        let late = resolved.map_or(Utc::now() > due, |r| r > due);
        let was = doc.get("breached").and_then(Value::as_bool).unwrap_or(false);
        // A breach, once recorded, stays.
        doc.insert("breached".into(), json!(late || was));
        Ok(())
    }

    async fn validate(&self, _hc: &mut HookCtx<'_>, doc: &Doc) -> Result<()> {
        if matches!(s(doc, "status"), Some("Resolved" | "Closed")) && s(doc, "resolution").is_none() {
            return Err(Error::field("resolution", "a resolved incident says how"));
        }
        Ok(())
    }
}

/// Incidents still open past their time, as of `now`, marked breached.
pub(crate) async fn mark_breached(k: &Kernel, ctx: &Ctx, tx: &mut Tx, now: DateTime<Utc>) -> Result<()> {
    let late: Vec<Uuid> = sqlx::query_scalar(
        "select id from e_incident where tenant_id = $1 and status in ('New', 'In Progress', 'On Hold') and resolve_by < $2 and not coalesce(breached, false)",
    )
    .bind(ctx.tenant)
    .bind(now)
    .fetch_all(&mut **tx)
    .await?;
    for incident in late {
        k.set_values_tx(ctx, tx, "Incident", incident, obj(json!({"breached": true}))).await?;
    }
    Ok(())
}

pub struct ProblemHook;

#[async_trait]
impl DocHook for ProblemHook {
    fn name(&self) -> &'static str {
        "itsm.problem"
    }

    async fn validate(&self, _hc: &mut HookCtx<'_>, doc: &Doc) -> Result<()> {
        match s(doc, "status") {
            Some("Known Error") if s(doc, "workaround").is_none() => Err(Error::field("workaround", "a known error has its workaround")),
            Some("Resolved" | "Closed") if s(doc, "root_cause").is_none() => Err(Error::field("root_cause", "a resolved problem says its root cause")),
            _ => Ok(()),
        }
    }

    async fn after_save(&self, hc: &mut HookCtx<'_>, doc: &Doc, old: Option<&Doc>) -> Result<()> {
        let resolved = matches!(s(doc, "status"), Some("Resolved" | "Closed"));
        let was = old.is_some_and(|o| matches!(s(o, "status"), Some("Resolved" | "Closed")));
        if !resolved || was {
            return Ok(());
        }
        let problem = id(doc, "id").ok_or_else(|| Error::internal("a problem without an id"))?;
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
