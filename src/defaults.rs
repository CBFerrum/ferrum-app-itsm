//! What a workspace starts with that is not a package's own record: the working calendar its targets
//! count on. It is the workspace's to edit (Admin, Calendars); it is made only when absent, so an
//! edit is never undone.

use ferrum_kernel::{Result, Tx};
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
