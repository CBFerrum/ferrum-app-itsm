//! IT service management (FERRUM-ECOSYSTEM-002, -003): configuration items and their
//! dependencies, a service catalog, incidents by priority with their targets, problems, and changes
//! with approval, conflicts, freezes and impact.
//!
//! * `cmdb`: configuration items whose dependencies never loop, and what a change to one affects.
//! * `desk`: catalog requests approved by someone else, incidents by the impact and urgency matrix
//!   with their targets and breaches, and problems that resolve their incidents.
//! * `changes`: standard, normal and emergency changes, decided by a Change Manager who did not
//!   raise them, kept out of freezes and off each other's items.
//!
//! This app's own design, after the practices ITIL describes; no code was copied
//! (docs/PROVENANCE.md). Hooks are registered per entity, so a workspace without the `itsm`
//! package never runs them.

mod changes;
mod cmdb;
mod desk;

use chrono::{DateTime, Utc};
use ferrum_domain::reports::ReportSource;
use ferrum_domain::util::s;
use ferrum_kernel::{Ctx, Doc, Error, HookRegistry, Kernel, Result};
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

include!(concat!(env!("OUT_DIR"), "/packages.rs"));
include!(concat!(env!("OUT_DIR"), "/ui.rs"));

/// This app's manifest (`app.yaml`).
pub const MANIFEST: &str = include_str!("../app.yaml");

/// This app's metadata packages, embedded at build time.
pub fn packages() -> std::result::Result<Vec<ferrum_meta::Package>, ferrum_meta::MetaError> {
    ferrum_meta::catalog::load_embedded(EMBEDDED)
}

/// Registers this app's document hooks.
pub fn register(hooks: &mut HookRegistry) {
    hooks
        .register("ConfigurationItem", Arc::new(cmdb::CiHook))
        .register("CatalogRequest", Arc::new(desk::RequestHook))
        .register("Incident", Arc::new(desk::IncidentHook))
        .register("Problem", Arc::new(desk::ProblemHook))
        .register("ItChange", Arc::new(changes::ChangeHook))
        .register("ChangeFreeze", Arc::new(changes::FreezeHook));
}

/// This app's reports: none of its own yet.
pub fn reports() -> Vec<Arc<dyn ReportSource>> {
    vec![]
}

/// Upkeep: incidents still open past their time marked breached.
pub async fn maintenance(k: &Kernel) -> Result<()> {
    maintenance_at(k, Utc::now()).await
}

/// [`maintenance`] as of `now`.
pub async fn maintenance_at(k: &Kernel, now: DateTime<Utc>) -> Result<()> {
    let mut tx = k.db.system_tx().await?;
    let tenants: Vec<Uuid> = sqlx::query_scalar("select id from tenant where status = 'active' order by id").fetch_all(&mut *tx).await?;
    tx.commit().await?;
    let mut first_error: Option<Error> = None;
    for tenant in tenants {
        let run = async {
            let schema = k.registry.schema(tenant).await?;
            if !schema.packages.iter().any(|p| p == "itsm") {
                return Ok(());
            }
            let ctx = Ctx::system(tenant, schema);
            let mut tx = k.db.tenant_tx(tenant).await?;
            desk::mark_breached(k, &ctx, &mut tx, now).await?;
            tx.commit().await?;
            Ok::<(), Error>(())
        };
        if let Err(e) = run.await {
            first_error.get_or_insert(e);
        }
    }
    first_error.map_or(Ok(()), Err)
}

/// A document from JSON.
pub(crate) fn obj(v: Value) -> Doc {
    v.as_object().cloned().unwrap_or_default()
}

/// A date and time field.
pub(crate) fn at(doc: &Doc, k: &str) -> Option<DateTime<Utc>> {
    s(doc, k).and_then(|v| DateTime::parse_from_rfc3339(v).ok()).map(|t| t.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_priority_matrix() {
        use desk::{hours, priority};
        assert_eq!(priority("High", "High"), "P1");
        assert_eq!((priority("High", "Medium"), priority("Medium", "High")), ("P2", "P2"));
        assert_eq!((priority("High", "Low"), priority("Medium", "Medium"), priority("Low", "High")), ("P3", "P3", "P3"));
        assert_eq!((priority("Medium", "Low"), priority("Low", "Low")), ("P4", "P4"));
        assert_eq!((hours("P1"), hours("P2"), hours("P3"), hours("P4")), (4, 8, 24, 72));
    }

    #[test]
    fn the_interface_is_a_release_the_platform_takes() {
        let m = ferrum_apps::AppManifest::parse(MANIFEST).expect("app.yaml");
        assert_eq!(m.routes(), vec![("itsm".to_string(), vec![])]);
        let files = UI.iter().map(|(p, b)| (p.to_string(), b.to_vec())).collect();
        let ui = ferrum_apps::ui::prepare("itsm", files).expect("ui/dist is an interface release");
        assert!(UI.iter().any(|(p, _)| *p == "index.html") && !UI.iter().any(|(p, _)| p.starts_with('.')), "{}", ui.digest);
    }
}
