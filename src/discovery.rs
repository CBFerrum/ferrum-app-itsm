//! Discovery: what network scanners, cloud inventories and agents find, reported to
//! `POST /api/app/itsm/discovery` by an account with the Discovery Agent or Configuration Manager
//! role (an API key, for a tool):
//!
//! ```json
//! {"source": "scanner-dc1", "items": [{"key": "srv-7", "name": "db-server-1", "type": "Server",
//!   "serial_number": "SN-1", "location": "DC1", "environment": "Production",
//!   "details": {"os": "Debian 13", "ip": "10.0.0.7"}, "runs_on": "host-2"}]}
//! ```
//!
//! An item is the one its source reported under its key before, else the one with its serial
//! number, else the one with its name, unless another source reports that one; otherwise it is
//! new, and Live. What a source says (type, serial number, location, environment, details) is
//! kept; what people keep (its name, owner, criticality, status, notes and the relations they set)
//! is not touched. `runs_on` names another item of the same source, and adds that relation when it
//! is missing, never one that would loop. An item its source has not reported for 7 days is marked
//! not seen since its last day and its owner told: to look into, never retired by itself.

use crate::obj;
use chrono::{DateTime, Duration, Utc};
use ferrum_domain::reports::ReportSource;
use ferrum_domain::util::{elevated, id, rows, s};
use ferrum_kernel::{Ctx, Doc, Error, Filter, Kernel, Result, Site, SiteRequest, SiteResponse, Tx, notify};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

/// Items in one report, at most.
const MAX_ITEMS: usize = 500;
/// Days without a report before an item is marked not seen.
const MISSING_DAYS: i64 = 7;
const TYPES: &[&str] = &["Business Service", "Application", "Server", "Database", "Network Device", "Laptop", "Other"];
const ENVIRONMENTS: &[&str] = &["Production", "Staging", "Test", "Development"];

#[derive(Debug, Deserialize)]
pub(crate) struct Report {
    pub source: String,
    pub items: Vec<Found>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Found {
    pub key: String,
    pub name: String,
    #[serde(default, rename = "type")]
    pub ci_type: Option<String>,
    #[serde(default)]
    pub serial_number: Option<String>,
    #[serde(default)]
    pub location: Option<String>,
    #[serde(default)]
    pub environment: Option<String>,
    #[serde(default)]
    pub details: Option<Value>,
    #[serde(default)]
    pub runs_on: Option<String>,
}

/// Why a report is refused as a whole, if it is.
pub(crate) fn check(r: &Report) -> std::result::Result<(), String> {
    let source = r.source.trim();
    if source.is_empty() || source.len() > 60 || !source.chars().all(|c| c.is_ascii_alphanumeric() || " ._-".contains(c)) {
        return Err("source is 1 to 60 letters, digits, spaces, dots, dashes or underscores".into());
    }
    if r.items.is_empty() || r.items.len() > MAX_ITEMS {
        return Err(format!("a report has 1 to {MAX_ITEMS} items"));
    }
    Ok(())
}

/// Why an item is refused, if it is.
pub(crate) fn check_item(f: &Found) -> std::result::Result<(), String> {
    if f.key.trim().is_empty() || f.key.len() > 200 {
        return Err("key is 1 to 200 characters".into());
    }
    if f.name.trim().is_empty() || f.name.len() > 140 {
        return Err("name is 1 to 140 characters".into());
    }
    if let Some(t) = f.ci_type.as_deref().filter(|t| !TYPES.contains(t)) {
        return Err(format!("type {t} is not one of {}", TYPES.join(", ")));
    }
    if let Some(e) = f.environment.as_deref().filter(|e| !ENVIRONMENTS.contains(e)) {
        return Err(format!("environment {e} is not one of {}", ENVIRONMENTS.join(", ")));
    }
    if f.details.as_ref().is_some_and(|d| !d.is_object()) {
        return Err("details are an object".into());
    }
    Ok(())
}

pub struct DiscoverySite;

#[async_trait::async_trait]
impl Site for DiscoverySite {
    async fn handle(&self, k: &Kernel, req: SiteRequest) -> Result<SiteResponse> {
        let user = req.user.as_ref().ok_or_else(|| Error::Unauthorized("sign in first".into()))?;
        // The change calendar, as the report gives it: `GET /calendar?from_date=&to_date=`.
        if (req.method.as_str(), req.path.trim_matches('/')) == ("GET", "calendar") {
            let params = req.query.iter().filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string()))).collect();
            let out = crate::reports::ItsmReports.run(k, user, "itsm_change_calendar", &params).await?;
            return Ok(SiteResponse::json(200, &out));
        }
        // Known errors that fit what someone typed, as the person may read them (a portal person: the published ones):
        // `GET /suggest?q=`, for the incident form and the portal.
        if (req.method.as_str(), req.path.trim_matches('/')) == ("GET", "suggest") {
            let q = ferrum_kernel::ListQuery { search: req.query.get("q").and_then(Value::as_str).map(str::to_string), limit: Some(5), ..Default::default() };
            let found = k.list(user, "KnownError", &q).await?;
            let rows: Vec<Value> = found.rows.iter().map(|r| json!({"id": r["id"], "title": r["title"], "workaround": r["workaround"]})).collect();
            return Ok(SiteResponse::json(200, &json!({"suggestions": rows})));
        }
        if (req.method.as_str(), req.path.trim_matches('/')) != ("POST", "discovery") {
            return Err(Error::NotFound(format!("IT Services endpoint {} {}", req.method, req.path)));
        }
        if !(user.is_admin || user.roles.iter().any(|r| r == "Discovery Agent" || r == "Configuration Manager")) {
            return Err(Error::Forbidden("discovery reports come from a Discovery Agent or a Configuration Manager".into()));
        }
        let report: Report = serde_json::from_slice(&req.body).map_err(|e| Error::invalid(format!("not a discovery report: {e}")))?;
        check(&report).map_err(Error::invalid)?;
        let out = take(k, user, &report).await?;
        Ok(SiteResponse::json(200, &out))
    }
}

/// Takes a report in, an item at a time: `{created, updated, refused: [{key, error}]}`.
pub(crate) async fn take(k: &Kernel, user: &Ctx, report: &Report) -> Result<Value> {
    let ctx = elevated(user);
    let source = report.source.trim();
    let now = Utc::now();
    let (mut created, mut updated, mut refused) = (0, 0, Vec::new());
    let mut placed: Vec<(Uuid, &Found)> = Vec::new();
    for f in &report.items {
        let done = match check_item(f) {
            Err(why) => Err(Error::invalid(why)),
            Ok(()) => upsert(k, &ctx, source, f, now).await,
        };
        match done {
            Ok((ci, new)) => {
                if new {
                    created += 1;
                } else {
                    updated += 1;
                }
                placed.push((ci, f));
            }
            Err(e) => refused.push(json!({"key": f.key, "error": said(&e)})),
        }
    }
    // What each runs on, once every item of the report is in.
    for (ci, f) in &placed {
        let Some(host) = f.runs_on.as_deref() else { continue };
        if let Err(e) = runs_on(k, &ctx, source, *ci, host).await {
            refused.push(json!({"key": f.key, "error": format!("runs_on: {}", said(&e))}));
        }
    }
    Ok(json!({"created": created, "updated": updated, "refused": refused}))
}

/// An error as a tool reads it, a refused field's message included.
fn said(e: &Error) -> String {
    match e {
        Error::Validation(fields) => fields.iter().map(|f| format!("{}: {}", f.field, f.message)).collect::<Vec<_>>().join("; "),
        other => other.to_string(),
    }
}

async fn one(k: &Kernel, ctx: &Ctx, filters: &[Filter]) -> Result<Vec<Doc>> {
    let mut tx = k.db.tenant_tx(ctx.tenant).await?;
    let found = k.find_tx(ctx, &mut tx, "ConfigurationItem", filters, None, 2).await?;
    tx.commit().await?;
    Ok(found)
}

/// The item a source reports, taken in: (its id, whether it is new).
async fn upsert(k: &Kernel, ctx: &Ctx, source: &str, f: &Found, now: DateTime<Utc>) -> Result<(Uuid, bool)> {
    let key = f.key.trim();
    let serial = f.serial_number.as_deref().map(str::trim).filter(|v| !v.is_empty());
    let free = |d: &Doc| s(d, "discovered_by").is_none_or(|by| by == source);
    let mut found = one(k, ctx, &[Filter::eq("discovered_by", source), Filter::eq("discovery_key", key)]).await?.into_iter().next();
    if found.is_none()
        && let Some(sn) = serial
    {
        let by_serial = one(k, ctx, &[Filter::eq("serial_number", sn)]).await?;
        if let [only] = by_serial.as_slice()
            && free(only)
        {
            found = Some(only.clone());
        }
    }
    if found.is_none() {
        let by_name = one(k, ctx, &[Filter::eq("ci_name", f.name.trim())]).await?;
        if let Some(named) = by_name.into_iter().next() {
            if !free(&named) {
                return Err(Error::invalid(format!("{} is reported by {}, another source", f.name.trim(), s(&named, "discovered_by").unwrap_or_default())));
            }
            found = Some(named);
        }
    }
    let mut v = obj(json!({"discovered_by": source, "discovery_key": key, "last_seen_at": now.to_rfc3339(), "missing_since": null}));
    for (field, given) in
        [("ci_type", f.ci_type.as_deref()), ("serial_number", serial), ("location", f.location.as_deref()), ("environment", f.environment.as_deref())]
    {
        if let Some(given) = given.map(str::trim).filter(|g| !g.is_empty()) {
            v.insert(field.into(), json!(given));
        }
    }
    if let Some(details) = &f.details {
        v.insert("discovered".into(), details.clone());
    }
    match found {
        Some(ci) => {
            let me = id(&ci, "id").ok_or_else(|| Error::internal("an item without an id"))?;
            k.update(ctx, "ConfigurationItem", me, v, None).await?;
            Ok((me, false))
        }
        None => {
            v.insert("ci_name".into(), json!(f.name.trim()));
            v.insert("status".into(), json!("Live"));
            let made = k.insert(ctx, "ConfigurationItem", v).await?;
            Ok((id(&made, "id").ok_or_else(|| Error::internal("an item without an id"))?, true))
        }
    }
}

/// `ci` runs on the item its source calls `host`: the relation added when it is missing.
async fn runs_on(k: &Kernel, ctx: &Ctx, source: &str, ci: Uuid, host: &str) -> Result<()> {
    let target = one(k, ctx, &[Filter::eq("discovered_by", source), Filter::eq("discovery_key", host.trim())])
        .await?
        .into_iter()
        .next()
        .and_then(|d| id(&d, "id"))
        .ok_or_else(|| Error::invalid(format!("{} reports no item {host}", source)))?;
    let me = k.get(ctx, "ConfigurationItem", ci, false).await?;
    let mut relations = rows(&me, "relations");
    if relations.iter().any(|r| s(r, "kind") == Some("Runs On") && id(r, "target") == Some(target)) {
        return Ok(());
    }
    relations.push(obj(json!({"kind": "Runs On", "target": target.to_string()})));
    k.update(ctx, "ConfigurationItem", ci, obj(json!({"relations": relations})), None).await?;
    Ok(())
}

/// Items their source has not reported for 7 days, as of `now`: marked not seen since the day it
/// last did, and their owner told, once.
pub(crate) async fn mark_missing(k: &Kernel, ctx: &Ctx, tx: &mut Tx, now: DateTime<Utc>) -> Result<usize> {
    let gone: Vec<(Uuid, String, Option<String>, DateTime<Utc>)> = sqlx::query_as(
        "select id, ci_name, owner::text, last_seen_at from e_configuration_item where tenant_id = $1 and discovered_by is not null \
         and missing_since is null and status <> 'Retired' and last_seen_at < $2 order by id",
    )
    .bind(ctx.tenant)
    .bind(now - Duration::days(MISSING_DAYS))
    .fetch_all(&mut **tx)
    .await?;
    for (ci, name, owner, seen) in &gone {
        k.set_values_tx(ctx, tx, "ConfigurationItem", *ci, obj(json!({"missing_since": seen.date_naive().to_string()}))).await?;
        let user: Option<Uuid> = sqlx::query_scalar("select id from app_user where tenant_id = $1 and is_active and lower(email::text) = lower($2)")
            .bind(ctx.tenant)
            .bind(owner.as_deref().unwrap_or_default())
            .fetch_optional(&mut **tx)
            .await?;
        if let Some(user) = user {
            let title = format!("Not seen by discovery since {}: {name}", seen.date_naive());
            notify::send_kind(tx, ctx.tenant, user, "itsm.missing", "inapp", &title, None, Some("ConfigurationItem"), Some(*ci)).await?;
        }
    }
    Ok(gone.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(v: Value) -> Found {
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn what_a_report_may_say() {
        let item = json!({"key": "k", "name": "n"});
        assert!(check(&Report { source: "scanner-dc1".into(), items: vec![found(item.clone())] }).is_ok());
        assert!(check(&Report { source: "bad/source".into(), items: vec![found(item.clone())] }).is_err());
        assert!(check(&Report { source: "s".into(), items: vec![] }).is_err());
        assert!(check(&Report { source: "s".into(), items: (0..=MAX_ITEMS).map(|_| found(item.clone())).collect() }).is_err());
        assert!(check_item(&found(json!({"key": "k", "name": "db", "type": "Server", "environment": "Test", "details": {"os": "x"}}))).is_ok());
        assert!(check_item(&found(json!({"key": " ", "name": "db"}))).is_err());
        assert!(check_item(&found(json!({"key": "k", "name": ""}))).is_err());
        assert!(check_item(&found(json!({"key": "k", "name": "db", "type": "Toaster"}))).unwrap_err().contains("not one of"));
        assert!(check_item(&found(json!({"key": "k", "name": "db", "environment": "Prod"}))).is_err());
        assert!(check_item(&found(json!({"key": "k", "name": "db", "details": [1]}))).is_err());
    }
}
