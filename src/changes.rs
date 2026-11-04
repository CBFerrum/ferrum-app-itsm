//! Changes. A change ends after it starts, and a normal or emergency one has its back-out plan. Its
//! impact is what depends on the item it changes. A standard change is approved when submitted; a
//! normal one by a Change Manager who did not raise it, and never inside a freeze; an emergency one
//! may fall in a freeze. No two approved changes touch one item at once.

use crate::cmdb::impact;
use crate::{at, obj};
use async_trait::async_trait;
use chrono::Utc;
use ferrum_domain::util::{elevated, id, s};
use ferrum_kernel::{Doc, DocHook, Error, HookCtx, Result};
use serde_json::{Value, json};
use uuid::Uuid;

pub struct ChangeHook;

/// Refused when the change's window clashes: inside a freeze (unless an emergency), or, once it is
/// to be approved (`approving`), over another approved change of the same item.
async fn clear(hc: &mut HookCtx<'_>, doc: &Doc, approving: bool) -> Result<()> {
    let (start, end) = (at(doc, "planned_start"), at(doc, "planned_end"));
    let (Some(start), Some(end)) = (start, end) else { return Ok(()) };
    if s(doc, "kind") != Some("Emergency") {
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
        &["impacted"]
    }

    async fn before_validate(&self, hc: &mut HookCtx<'_>, doc: &mut Doc) -> Result<()> {
        if let Some(ci) = id(doc, "ci") {
            let names = impact(hc.tx, hc.ctx.tenant, ci).await?;
            doc.insert("impacted".into(), if names.is_empty() { Value::Null } else { json!(names.join(", ")) });
        }
        Ok(())
    }

    async fn validate(&self, hc: &mut HookCtx<'_>, doc: &Doc) -> Result<()> {
        if let (Some(start), Some(end)) = (at(doc, "planned_start"), at(doc, "planned_end"))
            && end <= start
        {
            return Err(Error::field("planned_end", "is after the change starts"));
        }
        if s(doc, "kind") != Some("Standard") && s(doc, "backout_plan").is_none() {
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
            && ["ci", "kind", "planned_start", "planned_end", "backout_plan"].iter().any(|k| s(o, k) != s(doc, k))
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
        let values = match name {
            "submit_change" if s(doc, "kind") == Some("Standard") => {
                clear(hc, doc, true).await?;
                json!({"status": "Approved", "decision_note": "A standard change, approved in advance"})
            }
            "submit_change" => json!({"status": "Submitted"}),
            "approve_change" | "reject_change" => {
                if !decider {
                    return Err(Error::Forbidden("a Change Manager decides a change".into()));
                }
                if raised_it {
                    return Err(Error::Forbidden("a change is decided by someone other than who raised it".into()));
                }
                if name == "approve_change" {
                    clear(hc, doc, true).await?;
                    json!({"status": "Approved", "decided_by": hc.ctx.email, "decision_note": note})
                } else {
                    json!({"status": "Rejected", "decided_by": hc.ctx.email, "decision_note": note.ok_or_else(|| Error::field("note", "a rejection says why"))?})
                }
            }
            "implement" => json!({"status": "Implemented", "implemented_at": Utc::now().to_rfc3339()}),
            "close_change" => json!({"status": "Closed", "decision_note": note.or(s(doc, "decision_note"))}),
            "fail_change" => json!({"status": "Failed", "decision_note": note.ok_or_else(|| Error::field("note", "a failed change says what happened"))?}),
            _ => return Ok(None),
        };
        let status = values["status"].clone();
        hc.kernel.set_values_tx(&elevated(hc.ctx), hc.tx, "ItChange", change, obj(values)).await?;
        Ok(Some(json!({"status": status})))
    }
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
