//! What a workspace starts with that is not a package's own record: the working calendar its targets
//! count on, and the notifications to callers and assignees (`automations.json`: automations like any
//! other, with their wording as templates). Both are the workspace's to edit (Admin, Calendars and
//! Automations); the calendar is made only when absent and the automations once, so no edit is undone.

use crate::obj;
use ferrum_domain::util::sys;
use ferrum_kernel::{Ctx, Kernel, Result, Tx};
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

/// Makes the automations once for the workspace (the settings record notes that they were made), as
/// the system; a workspace that edits or switches one off, or deletes it, is not undone.
pub(crate) async fn ensure_automations(k: &Kernel, ctx: &Ctx) -> Result<()> {
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
    match settings.first().and_then(|s| s.get("id")).and_then(Value::as_str).and_then(|i| i.parse().ok()) {
        Some(id) => k.set_values_tx(&system, &mut tx, "ItsmSettings", id, obj(json!({"defaults_made": true}))).await?,
        None => {
            k.insert_tx(&system, &mut tx, "ItsmSettings", obj(json!({"defaults_made": true}))).await?;
        }
    }
    tx.commit().await?;
    Ok(())
}
