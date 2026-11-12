//! Configuration items. An item has a class (records, with the attributes it declares, checked when the item is
//! saved) and relates to others by a relation type (records). A relation type says where a failure flows: To the
//! Item (a dependency, a thing run on another), To the Target (a part of something) or None. The relations that carry
//! a failure never come back round to the item itself, however far; an item's impact is every item a failure of it
//! reaches, to the depth the settings allow.

use async_trait::async_trait;
use ferrum_domain::util::{id, rows, s};
use ferrum_kernel::{Doc, DocHook, Error, HookCtx, Result, Tx};
use serde_json::{Value, json};
use std::collections::{HashSet, VecDeque};
use uuid::Uuid;

pub struct CiHook;

/// Where a failure flows, in SQL, for a relation `r` and its type `t`: a relation with no type (written before relation
/// types) is read by its kind, as it was.
macro_rules! flows {
    () => {
        "coalesce(t.impact_flows, case when r.kind in ('Depends On', 'Runs On') then 'To the Item' else 'None' end)"
    };
}

/// The items a failure of `from` reaches directly: those that depend on it (their relation, To the Item, names it) and
/// what it is a part of (its own relation, To the Target). `not_by` leaves out the relations an item being saved lists,
/// which the database still holds as they were.
async fn reaches(tx: &mut Tx, tenant: Uuid, from: Uuid, not_by: Option<Uuid>) -> Result<Vec<Uuid>> {
    Ok(sqlx::query_scalar(concat!(
        "select r.parent_id from e_configuration_item__relations r left join e_relation_type t on t.tenant_id = r.tenant_id and t.id = r.relation_type ",
        "where r.tenant_id = $1 and r.target = $2 and r.parent_id <> $3 and ",
        flows!(),
        " = 'To the Item' ",
        "union select r.target from e_configuration_item__relations r left join e_relation_type t on t.tenant_id = r.tenant_id and t.id = r.relation_type ",
        "where r.tenant_id = $1 and r.parent_id = $2 and $2 <> $3 and r.target is not null and ",
        flows!(),
        " = 'To the Target'"
    ))
    .bind(tenant)
    .bind(from)
    .bind(not_by.unwrap_or_else(Uuid::nil))
    .fetch_all(&mut **tx)
    .await?)
}

/// Every item a failure of `ci` reaches, up to `depth` relations out and never twice, by name.
pub(crate) async fn impact(tx: &mut Tx, tenant: Uuid, ci: Uuid, depth: i64) -> Result<Vec<String>> {
    let mut seen = HashSet::from([ci]);
    let mut frontier = vec![ci];
    let mut out = vec![];
    for _ in 0..depth {
        let mut next = vec![];
        for n in frontier {
            for up in reaches(tx, tenant, n, None).await? {
                if seen.insert(up) {
                    next.push(up);
                    out.push(up);
                }
            }
        }
        if next.is_empty() {
            break;
        }
        frontier = next;
    }
    let mut names: Vec<String> = sqlx::query_scalar("select ci_name from e_configuration_item where tenant_id = $1 and id = any($2)")
        .bind(tenant)
        .bind(&out)
        .fetch_all(&mut **tx)
        .await?;
    names.sort();
    Ok(names)
}

/// A relation type by name, with where its failures flow.
async fn type_named(tx: &mut Tx, tenant: Uuid, name: &str) -> Result<Option<(Uuid, String)>> {
    Ok(sqlx::query_as("select id, relation_name from e_relation_type where tenant_id = $1 and lower(relation_name) = lower($2)")
        .bind(tenant)
        .bind(name)
        .fetch_optional(&mut **tx)
        .await?)
}

/// Where a relation row's failure flows.
async fn flows_of(tx: &mut Tx, tenant: Uuid, row: &Doc) -> Result<&'static str> {
    let by_type: Option<String> = match id(row, "relation_type") {
        Some(t) => {
            sqlx::query_scalar("select impact_flows from e_relation_type where tenant_id = $1 and id = $2")
                .bind(tenant)
                .bind(t)
                .fetch_optional(&mut **tx)
                .await?
        }
        None => None,
    };
    Ok(match by_type.as_deref() {
        Some("To the Item") => "To the Item",
        Some("To the Target") => "To the Target",
        Some(_) => "None",
        None if matches!(s(row, "kind"), Some("Depends On" | "Runs On")) => "To the Item",
        None => "None",
    })
}

/// The attributes the class declares are what an item of it holds, each of its kind.
async fn attributes_fit(hc: &mut HookCtx<'_>, doc: &Doc) -> Result<()> {
    let given = doc.get("attributes").and_then(Value::as_object).cloned().unwrap_or_default();
    let declared: Vec<(String, String, Option<bool>, Option<String>)> = match id(doc, "ci_class") {
        Some(c) => {
            sqlx::query_as("select attr_name, attr_type, required, choices from e_ci_class__attributes where tenant_id = $1 and parent_id = $2 order by idx")
                .bind(hc.ctx.tenant)
                .bind(c)
                .fetch_all(&mut **hc.tx)
                .await?
        }
        None => vec![],
    };
    let bad = |m: String| Error::field("attributes", m);
    for key in given.keys() {
        if !declared.iter().any(|d| d.0.eq_ignore_ascii_case(key)) {
            return Err(bad(format!("{key} is not an attribute of its class")));
        }
    }
    for (name, kind, required, choices) in &declared {
        let v = given.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v);
        let text = match v {
            None | Some(Value::Null) => String::new(),
            Some(Value::String(t)) => t.trim().to_string(),
            Some(other) => other.to_string(),
        };
        if text.is_empty() {
            if *required == Some(true) {
                return Err(bad(format!("{name} needs a value")));
            }
            continue;
        }
        let fits = match kind.as_str() {
            "Number" => text.parse::<f64>().is_ok(),
            "Date" => chrono::NaiveDate::parse_from_str(&text, "%Y-%m-%d").is_ok(),
            "Yes/No" => matches!(text.as_str(), "Yes" | "No" | "true" | "false"),
            "Choice" => choices.as_deref().unwrap_or_default().split(',').map(str::trim).any(|c| c == text),
            _ => true,
        };
        if !fits {
            let want = match kind.as_str() {
                "Number" => "a number".to_string(),
                "Date" => "a date (YYYY-MM-DD)".to_string(),
                "Yes/No" => "Yes or No".to_string(),
                _ => format!("one of {}", choices.as_deref().unwrap_or_default()),
            };
            return Err(bad(format!("{name} is {want}")));
        }
    }
    Ok(())
}

#[async_trait]
impl DocHook for CiHook {
    fn name(&self) -> &'static str {
        "itsm.ci"
    }

    fn fills(&self) -> &'static [&'static str] {
        &["ci_class", "ci_type", "relations"]
    }

    /// Class and type follow each other (a class named like one of the types sets it, else Other; a type with a class of
    /// its name sets the class), and a relation's type and kind do: all so what was written before classes and relation
    /// types, and what a workspace upgraded has not been given them yet, keeps working.
    async fn before_validate(&self, hc: &mut HookCtx<'_>, doc: &mut Doc) -> Result<()> {
        const TYPES: [&str; 7] = ["Business Service", "Application", "Server", "Database", "Network Device", "Laptop", "Other"];
        let tenant = hc.ctx.tenant;
        let stored: Option<(Option<String>, Option<Uuid>)> = match id(doc, "id") {
            Some(me) => {
                sqlx::query_as("select ci_type, ci_class from e_configuration_item where tenant_id = $1 and id = $2")
                    .bind(tenant)
                    .bind(me)
                    .fetch_optional(&mut **hc.tx)
                    .await?
            }
            None => None,
        };
        let (was_type, was_class) = stored.unwrap_or_default();
        let (ty, class) = (s(doc, "ci_type").map(str::to_string), id(doc, "ci_class"));
        if class.is_none() || (class == was_class && ty != was_type) {
            let found: Option<Uuid> = sqlx::query_scalar("select id from e_ci_class where tenant_id = $1 and lower(class_name) = lower($2)")
                .bind(tenant)
                .bind(ty.as_deref().unwrap_or("Application"))
                .fetch_optional(&mut **hc.tx)
                .await?;
            doc.insert("ci_class".into(), found.map_or(Value::Null, |c| json!(c)));
        } else if let Some(c) = class {
            let name: Option<String> = sqlx::query_scalar("select class_name from e_ci_class where tenant_id = $1 and id = $2")
                .bind(tenant)
                .bind(c)
                .fetch_optional(&mut **hc.tx)
                .await?;
            let name = name.ok_or_else(|| Error::field("ci_class", "is not a class"))?;
            doc.insert("ci_type".into(), json!(TYPES.iter().find(|t| t.eq_ignore_ascii_case(&name)).copied().unwrap_or("Other")));
        }
        if let Some(Value::Array(list)) = doc.get("relations").cloned() {
            let mut out = Vec::new();
            for r in list.iter().filter_map(Value::as_object) {
                let mut r = r.clone();
                if let Some(t) = id(&r, "relation_type") {
                    let name: Option<String> = sqlx::query_scalar("select relation_name from e_relation_type where tenant_id = $1 and id = $2")
                        .bind(tenant)
                        .bind(t)
                        .fetch_optional(&mut **hc.tx)
                        .await?;
                    let name = name.ok_or_else(|| Error::field("relations", "a relation is of a type that does not exist"))?;
                    r.insert(
                        "kind".into(),
                        json!(["Depends On", "Runs On", "Connects To"].iter().find(|k| k.eq_ignore_ascii_case(&name)).copied().unwrap_or("Connects To")),
                    );
                } else if let Some((t, _)) = type_named(hc.tx, tenant, s(&r, "kind").unwrap_or("Depends On")).await? {
                    r.insert("relation_type".into(), json!(t));
                }
                out.push(Value::Object(r));
            }
            doc.insert("relations".into(), Value::Array(out));
        }
        Ok(())
    }

    async fn validate(&self, hc: &mut HookCtx<'_>, doc: &Doc) -> Result<()> {
        attributes_fit(hc, doc).await?;
        let me = id(doc, "id");
        let mut targets = HashSet::new();
        let (mut in_set, mut out_seed) = (HashSet::new(), VecDeque::new());
        for r in rows(doc, "relations") {
            let target = id(&r, "target").ok_or_else(|| Error::field("relations", "each relation names an item"))?;
            if Some(target) == me {
                return Err(Error::field("relations", "an item does not relate to itself"));
            }
            if !targets.insert((id(&r, "relation_type").map(|t| t.to_string()).or_else(|| s(&r, "kind").map(str::to_string)).unwrap_or_default(), target)) {
                return Err(Error::field("relations", "a relation is listed twice"));
            }
            match flows_of(hc.tx, hc.ctx.tenant, &r).await? {
                "To the Item" => {
                    in_set.insert(target);
                }
                "To the Target" => out_seed.push_back(target),
                _ => {}
            }
        }
        // A failure that leads back to this item would be a loop: from what this item's failure reaches, can any
        // item whose failure reaches this one be got to?
        let Some(me) = me else { return Ok(()) };
        out_seed.extend(reaches(hc.tx, hc.ctx.tenant, me, Some(me)).await?);
        let mut seen = HashSet::new();
        while let Some(next) = out_seed.pop_front() {
            if next == me || in_set.contains(&next) {
                return Err(Error::field("relations", "these relations come back round to this item"));
            }
            if seen.insert(next) {
                out_seed.extend(reaches(hc.tx, hc.ctx.tenant, next, Some(me)).await?);
            }
        }
        Ok(())
    }
}
