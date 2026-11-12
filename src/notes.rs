//! Notes on incidents, requests, problems and changes: internal work notes, or comments their caller
//! sees (which only an incident or a request has). A note is on exactly one record, carries its
//! author and time, and is corrected only by its author within the grace period the settings give
//! (`ItsmSettings.note_edit_minutes`); after that it is final. Who reads which note is permission
//! (the self-service role reads only visible ones on its own records), not code.

use async_trait::async_trait;
use chrono::{Duration, Utc};
use ferrum_domain::util::{id, s};
use ferrum_kernel::{Doc, DocHook, Error, HookCtx, Result};
use serde_json::json;

pub struct NoteHook;

/// The links a note may be on.
const PARENTS: [&str; 4] = ["incident", "request", "problem", "change"];

#[async_trait]
impl DocHook for NoteHook {
    fn name(&self) -> &'static str {
        "itsm.note"
    }

    fn fills(&self) -> &'static [&'static str] {
        &["author", "posted_at", "caller_email"]
    }

    async fn before_validate(&self, hc: &mut HookCtx<'_>, doc: &mut Doc) -> Result<()> {
        let on: Vec<_> = PARENTS.iter().filter_map(|f| id(doc, f).map(|i| (*f, i))).collect();
        let [(on, parent)] = on.as_slice() else {
            return Err(Error::invalid("a note is on one incident, request, problem or change"));
        };
        if s(doc, "author").is_none() {
            doc.insert("author".into(), json!(hc.ctx.email.to_lowercase()));
            doc.insert("posted_at".into(), json!(Utc::now().to_rfc3339()));
        }
        let caller = match *on {
            "incident" => sqlx::query_scalar::<_, Option<String>>("select caller_email from e_incident where tenant_id = $1 and id = $2"),
            "request" => sqlx::query_scalar::<_, Option<String>>("select caller_email from e_catalog_request where tenant_id = $1 and id = $2"),
            _ => return Ok(()),
        }
        .bind(hc.ctx.tenant)
        .bind(parent)
        .fetch_optional(&mut **hc.tx)
        .await?
        .flatten();
        doc.insert("caller_email".into(), caller.map_or(serde_json::Value::Null, |c| json!(c)));
        Ok(())
    }

    async fn validate(&self, _hc: &mut HookCtx<'_>, doc: &Doc) -> Result<()> {
        if s(doc, "visibility") == Some("Visible to Caller") && id(doc, "incident").is_none() && id(doc, "request").is_none() {
            return Err(Error::field("visibility", "only an incident or a request has a caller to see a comment"));
        }
        Ok(())
    }

    async fn validate_change(&self, hc: &mut HookCtx<'_>, old: Option<&Doc>, doc: &Doc) -> Result<()> {
        let Some(old) = old.filter(|_| !hc.ctx.is_system) else { return Ok(()) };
        if PARENTS.iter().any(|f| id(old, f) != id(doc, f)) || s(old, "author") != s(doc, "author") {
            return Err(Error::invalid("a note stays on its record, with its author"));
        }
        if s(old, "body") == s(doc, "body") && s(old, "visibility") == s(doc, "visibility") {
            return Ok(());
        }
        let minutes: Option<i64> = sqlx::query_scalar("select note_edit_minutes::bigint from e_itsm_settings where tenant_id = $1")
            .bind(hc.ctx.tenant)
            .fetch_optional(&mut **hc.tx)
            .await?
            .flatten();
        let minutes = minutes.unwrap_or(5);
        let mine = s(old, "author").is_some_and(|a| a.eq_ignore_ascii_case(&hc.ctx.email));
        let posted = crate::at(old, "posted_at").unwrap_or_else(Utc::now);
        if !mine || Utc::now() > posted + Duration::minutes(minutes) {
            return Err(Error::invalid(format!("a note is corrected only by its author, within {minutes} minutes of posting it")));
        }
        Ok(())
    }
}
