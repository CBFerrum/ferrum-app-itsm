//! Configuration items. An item depends on, runs on or connects to others. Dependencies (depends
//! on, runs on) never come back round to the item itself, however far; an item's impact is every
//! item that depends on or runs on it, however far round.

use async_trait::async_trait;
use ferrum_domain::util::{id, rows, s};
use ferrum_kernel::{Doc, DocHook, Error, HookCtx, Result, Tx};
use std::collections::{HashSet, VecDeque};
use uuid::Uuid;

pub struct CiHook;

/// What `from` depends on or runs on, directly.
async fn below(tx: &mut Tx, tenant: Uuid, from: Uuid) -> Result<Vec<Uuid>> {
    Ok(sqlx::query_scalar(
        "select r.target from e_configuration_item__relations r where r.tenant_id = $1 and r.parent_id = $2 and r.kind in ('Depends On', 'Runs On') and r.target is not null",
    )
    .bind(tenant)
    .bind(from)
    .fetch_all(&mut **tx)
    .await?)
}

/// What depends on or runs on `on`, directly.
async fn above(tx: &mut Tx, tenant: Uuid, on: Uuid) -> Result<Vec<Uuid>> {
    Ok(sqlx::query_scalar(
        "select r.parent_id from e_configuration_item__relations r where r.tenant_id = $1 and r.target = $2 and r.kind in ('Depends On', 'Runs On')",
    )
    .bind(tenant)
    .bind(on)
    .fetch_all(&mut **tx)
    .await?)
}

/// Every item that depends on or runs on `ci`, up to `depth` relations out and never twice, by name.
pub(crate) async fn impact(tx: &mut Tx, tenant: Uuid, ci: Uuid, depth: i64) -> Result<Vec<String>> {
    let mut seen = HashSet::from([ci]);
    let mut frontier = vec![ci];
    let mut out = vec![];
    for _ in 0..depth {
        let mut next = vec![];
        for n in frontier {
            for up in above(tx, tenant, n).await? {
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

#[async_trait]
impl DocHook for CiHook {
    fn name(&self) -> &'static str {
        "itsm.ci"
    }

    async fn validate(&self, hc: &mut HookCtx<'_>, doc: &Doc) -> Result<()> {
        let me = id(doc, "id");
        let mut targets = HashSet::new();
        for r in rows(doc, "relations") {
            let target = id(&r, "target").ok_or_else(|| Error::field("relations", "each relation names an item"))?;
            if Some(target) == me {
                return Err(Error::field("relations", "an item does not relate to itself"));
            }
            if !targets.insert((s(&r, "kind").unwrap_or_default().to_string(), target)) {
                return Err(Error::field("relations", "a relation is listed twice"));
            }
        }
        // A dependency that leads back to this item would be a loop.
        let Some(me) = me else { return Ok(()) };
        let mut seen = HashSet::new();
        let mut queue: VecDeque<Uuid> =
            rows(doc, "relations").iter().filter(|r| matches!(s(r, "kind"), Some("Depends On" | "Runs On"))).filter_map(|r| id(r, "target")).collect();
        while let Some(next) = queue.pop_front() {
            if next == me {
                return Err(Error::field("relations", "these dependencies come back round to this item"));
            }
            if seen.insert(next) {
                queue.extend(below(hc.tx, hc.ctx.tenant, next).await?);
            }
        }
        Ok(())
    }
}
