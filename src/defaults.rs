//! What a workspace starts with that is not a package's own record: the working calendar its targets
//! count on, the notifications to callers and assignees (`automations.json`: automations like any
//! other, with their wording as templates), and the package's seed rows (the priority matrix, the
//! categories). All are the workspace's to edit (Admin, Calendars, Automations, the records themselves);
//! the calendar is made only when absent, and the automations and any seed row missing once, so no edit is undone.
//!
//! The platform seeds a package's rows only when the package is installed or an administrator updates the
//! app (`Kernel::install_packages`), never when a server with a newer build starts: a workspace upgraded
//! from 0.2.x would otherwise run this release with an empty matrix until someone pressed Update. So the
//! upkeep makes them too, by the same rule as the platform's (a row is added when none has its key).

use crate::obj;
use ferrum_domain::util::sys;
use ferrum_kernel::query::Filter;
use ferrum_kernel::{Ctx, Doc, Kernel, Result, Tx};
use serde_json::{Value, json};
use uuid::Uuid;

/// The calendar the incident, request and approval targets name.
pub(crate) const CALENDAR: &str = "itsm_hours";

/// Makes the calendar when the workspace has none of that name: every hour of every day, in UTC, which
/// counts as the wall clock did before targets (so the hours an offering or a priority gives are the
/// hours that pass). A workspace sets its own hours, days off and zone.
pub(crate) async fn ensure_calendar(tx: &mut Tx, tenant: Uuid) -> Result<()> {
    sqlx::query("insert into logic_calendar (tenant_id, name, zone, weekends, holidays) values ($1, $2, 'UTC', '{}', '{}') on conflict do nothing")
        .bind(tenant)
        .bind(CALENDAR)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// The notifications as shipped: automations, each with its own id.
const AUTOMATIONS: &str = include_str!("automations.json");

/// Makes the automations and the package's seed rows once for the workspace (the settings record notes
/// that they were made), as the system; a workspace that edits or switches one off, or deletes it, or
/// deletes a seed row, is not undone, and a row it changed is never overwritten.
pub(crate) async fn ensure_defaults(k: &Kernel, ctx: &Ctx) -> Result<()> {
    let system = sys(ctx);
    let mut tx = k.db.tenant_tx(ctx.tenant).await?;
    let settings = k.find_tx(&system, &mut tx, "ItsmSettings", &[], None, 1).await?;
    tx.commit().await?;
    if settings.first().and_then(|s| s.get("defaults_made")).and_then(Value::as_bool) == Some(true) {
        return Ok(());
    }
    let list: Vec<Value> = serde_json::from_str(AUTOMATIONS).map_err(|e| ferrum_kernel::Error::internal(format!("automations.json: {e}")))?;
    for a in list {
        ferrum_kernel::automation::save(k, &system, &obj(a)).await?;
    }
    let mut tx = k.db.tenant_tx(ctx.tenant).await?;
    seed_rows(k, &system, &mut tx).await?;
    match settings.first().and_then(|s| s.get("id")).and_then(Value::as_str).and_then(|i| i.parse().ok()) {
        Some(id) => k.set_values_tx(&system, &mut tx, "ItsmSettings", id, obj(json!({"defaults_made": true}))).await?,
        None => {
            k.insert_tx(&system, &mut tx, "ItsmSettings", obj(json!({"defaults_made": true}))).await?;
        }
    }
    tx.commit().await?;
    Ok(())
}

/// The package's seed rows that the workspace has none of (by the seed's key), in the order the package lists
/// them (a parent before the rows that name it); `@Entity:field=value` is the id of that record.
async fn seed_rows(k: &Kernel, system: &Ctx, tx: &mut Tx) -> Result<()> {
    let pkgs = crate::packages().map_err(|e| ferrum_kernel::Error::internal(format!("package itsm: {e}")))?;
    for seed in pkgs.iter().filter(|p| p.name() == "itsm").flat_map(|p| p.manifest.seed.iter()) {
        for rec in &seed.records {
            let mut row = Doc::new();
            for (field, v) in rec {
                let named = v.as_str().and_then(|t| t.strip_prefix('@')).and_then(|t| t.split_once(':')).and_then(|(e, c)| Some((e, c.split_once('=')?)));
                let v = match named {
                    Some((entity, (f, value))) => {
                        k.find_tx(system, tx, entity, &[Filter::eq(f, value)], None, 1).await?.first().and_then(|d| d.get("id").cloned())
                    }
                    None => Some(v.clone()),
                };
                row.insert(field.clone(), v.ok_or_else(|| ferrum_kernel::Error::internal(format!("seed {}: {field} names nothing", seed.entity)))?);
            }
            let by: Vec<Filter> = seed.key.iter().map(|f| Filter::eq(f, row.get(f).cloned().unwrap_or(Value::Null))).collect();
            if k.find_tx(system, tx, &seed.entity, &by, None, 1).await?.is_empty() {
                k.insert_tx(system, tx, &seed.entity, row).await?;
            }
        }
    }
    Ok(())
}

/// Whether the workspace is one upgraded from an earlier release rather than new: it has made its notifications
/// already (release 0.3 did, once), or it has changes. Asked before `ensure_defaults` makes them for a new one.
pub(crate) async fn upgraded(k: &Kernel, ctx: &Ctx) -> Result<bool> {
    let system = sys(ctx);
    let mut tx = k.db.tenant_tx(ctx.tenant).await?;
    let settings = k.find_tx(&system, &mut tx, "ItsmSettings", &[], None, 1).await?;
    let changes: bool = sqlx::query_scalar("select exists (select 1 from e_it_change where tenant_id = $1)").bind(ctx.tenant).fetch_one(&mut *tx).await?;
    tx.commit().await?;
    Ok(changes || settings.first().and_then(|s| s.get("defaults_made")).and_then(Value::as_bool) == Some(true))
}

/// What release 0.4 adds, made once for the workspace (the settings record notes it): the package's seed rows it
/// lacks (change types, their groups, the risk questions), the type of each change it already holds, and the new
/// settings' starting values. A **new** workspace starts on the change board and on offering to resolve a problem's
/// incidents; one **upgraded** from 0.3 keeps what it ran (one Change Manager decides, a resolved problem resolves
/// its incidents) because both settings are left empty, which reads as that, until an administrator chooses.
/// A workspace that already set either itself is left as it set it.
pub(crate) async fn ensure_board(k: &Kernel, ctx: &Ctx, upgraded: bool) -> Result<()> {
    let system = sys(ctx);
    let mut tx = k.db.tenant_tx(ctx.tenant).await?;
    let settings = k.find_tx(&system, &mut tx, "ItsmSettings", &[], None, 1).await?;
    let Some(settings) = settings.first() else { return Ok(()) };
    if settings.get("board_defaults_made").and_then(Value::as_bool) == Some(true) {
        return Ok(());
    }
    seed_rows(k, &system, &mut tx).await?;
    sqlx::query(
        "update e_it_change c set change_type = t.id from e_change_type t where t.tenant_id = c.tenant_id and t.type_name = c.kind and c.change_type is null and c.tenant_id = $1",
    )
    .bind(ctx.tenant)
    .execute(&mut *tx)
    .await?;
    let mut set = json!({"board_defaults_made": true});
    if !upgraded {
        for (field, value) in [("change_approval", "Change Board"), ("problem_closes_incidents", "Offer")] {
            if settings.get(field).is_none_or(Value::is_null) {
                set[field] = json!(value);
            }
        }
    }
    let id = settings.get("id").and_then(Value::as_str).and_then(|i| i.parse().ok()).ok_or_else(|| ferrum_kernel::Error::internal("settings without an id"))?;
    k.set_values_tx(&system, &mut tx, "ItsmSettings", id, obj(set)).await?;
    tx.commit().await?;
    Ok(())
}
