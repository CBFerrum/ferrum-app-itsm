//! IT service management (FERRUM-ECOSYSTEM-002, -003): configuration items and their
//! dependencies, a service catalog, incidents by priority with their targets, problems, and changes
//! with approval, conflicts, freezes and impact.
//!
//! * `cmdb`: configuration items whose dependencies never loop, and what a change to one affects.
//! * `desk`: catalog requests approved by someone else, incidents by the impact and urgency matrix
//!   with their targets and breaches, and problems that resolve their incidents.
//! * `changes`: standard, normal and emergency changes, decided by a Change Manager who did not
//!   raise them, kept out of freezes and off each other's items.
//! * `discovery`: what scanners, cloud inventories and agents find, reported to an intake and
//!   matched to the items they are; items no longer reported marked for a person to look into.
//! * `assets`: the connector with Assets (`itsm_assets`): items linked to the fixed assets they
//!   are, with who holds them, where, and their warranty.
//!
//! This app's own design, after the practices ITIL describes; no code was copied
//! (docs/PROVENANCE.md). Hooks are registered per entity, so a workspace without the `itsm`
//! package never runs them.

mod assets;
mod changes;
mod cmdb;
mod desk;
mod discovery;

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

/// Registers this app's document hooks and its discovery intake. The connector's hook on
/// configuration items runs only where `itsm_assets` is installed.
pub fn register(hooks: &mut HookRegistry) {
    hooks
        .register("ConfigurationItem", Arc::new(cmdb::CiHook))
        .register("CatalogRequest", Arc::new(desk::RequestHook))
        .register("Incident", Arc::new(desk::IncidentHook))
        .register("Problem", Arc::new(desk::ProblemHook))
        .register("ItChange", Arc::new(changes::ChangeHook))
        .register("ChangeFreeze", Arc::new(changes::FreezeHook));
    hooks.scoped(&[CONNECTOR_PACKAGE.to_string()], |h| {
        h.register("ConfigurationItem", Arc::new(assets::AssetLinkHook));
    });
    hooks.add_site("itsm", Arc::new(discovery::DiscoverySite));
}

/// The connector with Assets' package.
pub const CONNECTOR_PACKAGE: &str = "itsm_assets";

/// The manifest of the connector app with Assets (`connectors/assets/app.yaml`): its package is
/// `itsm_assets`, its code this crate's. It ships with IT Services.
pub const CONNECTOR_MANIFEST: &str = include_str!("../connectors/assets/app.yaml");

/// This app's reports: none of its own yet.
pub fn reports() -> Vec<Arc<dyn ReportSource>> {
    vec![]
}

/// Upkeep: incidents still open past their time marked breached; items their discovery source no
/// longer reports marked not seen; and, with the connector, what items show of their asset kept.
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
            let connected = schema.packages.iter().any(|p| p == CONNECTOR_PACKAGE);
            let ctx = Ctx::system(tenant, schema);
            let mut tx = k.db.tenant_tx(tenant).await?;
            desk::mark_breached(k, &ctx, &mut tx, now).await?;
            discovery::mark_missing(k, &ctx, &mut tx, now).await?;
            if connected {
                assets::refresh(k, &ctx, &mut tx).await?;
            }
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
    fn the_interface_is_a_release_the_platform_takes() {
        let m = ferrum_apps::AppManifest::parse(MANIFEST).expect("app.yaml");
        assert_eq!(m.routes(), vec![("itsm".to_string(), vec![])]);
        let files = UI.iter().map(|(p, b)| (p.to_string(), b.to_vec())).collect();
        let ui = ferrum_apps::ui::prepare("itsm", files).expect("ui/dist is an interface release");
        assert!(UI.iter().any(|(p, _)| *p == "index.html") && !UI.iter().any(|(p, _)| p.starts_with('.')), "{}", ui.digest);
        let connector = ferrum_apps::AppManifest::parse(CONNECTOR_MANIFEST).expect("connectors/assets/app.yaml");
        assert_eq!(connector.packages, vec![CONNECTOR_PACKAGE.to_string()]);
    }
}
