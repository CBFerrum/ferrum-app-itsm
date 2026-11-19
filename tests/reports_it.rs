//! IT Services' reports group, filter and show only what the caller may read (S9 K0-7), against a real PostgreSQL
//! (`FERRUM_ITSM_TEST_DATABASE_URL`, default `postgres://ferrum:ferrum@localhost:5432/ferrum_itsm_test`; reset first).
//! A role of the test's own reads incidents but not their priority or resolution target, and changes but not their
//! class: a grouping by a field it cannot read is refused, a default it cannot read falls back to the next it can, and
//! a column of a field it cannot read is left out.

use ferrum_kernel::tenant::NewTenant;
use ferrum_kernel::{Ctx, Db, DbConfig, Doc, Error, HookRegistry, Kernel, KernelSettings};
use ferrum_meta::Catalog;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// A role that reads incidents without their priority and resolution target, and changes without their class.
const AUDIT: &str = "package: itsm_audit\nversion: 0.1.0\ndepends: [itsm]\nroles:\n  - name: Desk Auditor\n    permissions:\n      \
- {entity: Incident, read: true, hide: [priority, resolve_state]}\n      - {entity: ItGroup, read: true}\n      - {entity: ItCategory, read: true}\n      \
- {entity: ItChange, read: true, hide: [kind]}\n      - {entity: CatalogRequest, read: true}\n      - {entity: ServiceOffering, read: true}\n";

fn obj(v: Value) -> Doc {
    v.as_object().cloned().unwrap()
}

struct T {
    k: Arc<Kernel>,
    tenant: Uuid,
    admin: Ctx,
}

impl T {
    async fn user(&self, email: &str, roles: &[&str]) -> Ctx {
        let roles: Vec<String> = roles.iter().map(|r| r.to_string()).collect();
        let id = self.k.create_user(&self.admin, email, email, Some("Correct-Horse-9"), &roles, false).await.expect("user");
        let s = self.k.login("itreports", email, "Correct-Horse-9").await.expect("login");
        self.k.load_ctx(self.tenant, id, Some(s.token_version), "it".into()).await.expect("ctx")
    }
    async fn run(&self, ctx: &Ctx, name: &str, params: &[(&str, &str)]) -> Result<Value, Error> {
        let p: HashMap<String, String> = params.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect();
        ferrum_domain::reports::run(&self.k, ctx, name, &p).await
    }
}

fn columns(r: &Value) -> Vec<String> {
    r["columns"].as_array().unwrap().iter().map(|c| c["name"].as_str().unwrap().to_string()).collect()
}

#[test]
fn reports_by_what_the_caller_reads() {
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().thread_stack_size(16 << 20).build().unwrap();
    rt.block_on(async { tokio::spawn(scenario()).await.unwrap() });
}

async fn scenario() {
    let url = std::env::var("FERRUM_ITSM_TEST_DATABASE_URL").unwrap_or_else(|_| "postgres://ferrum:ferrum@localhost:5432/ferrum_itsm_test".into());
    let db = Db::connect(DbConfig { url, max_connections: 16, ..Default::default() }).await.expect("connect");
    sqlx::raw_sql("drop schema if exists public cascade; create schema public;").execute(&db.pool).await.expect("reset");
    let mut packages = ferrum_domain::packages().expect("platform packages");
    packages.extend(ferrum_app_itsm::packages().expect("app packages").into_iter().filter(|p| p.name() != ferrum_app_itsm::CONNECTOR_PACKAGE));
    let files = std::collections::BTreeMap::from([("package.yaml".to_string(), AUDIT.to_string())]);
    packages.push(ferrum_meta::catalog::load_package_files(&files).expect("the test's package"));
    let catalog = Catalog::from_packages(packages).expect("catalog");
    let mut hooks = HookRegistry::default();
    ferrum_domain::register(&mut hooks);
    ferrum_app_itsm::register(&mut hooks);
    let k = Kernel::boot(db, catalog, hooks, KernelSettings::default(), None).await.expect("boot");
    ferrum_domain::maintenance(&k).await.expect("partitions");
    for source in ferrum_app_itsm::reports() {
        ferrum_domain::reports::add_source(source);
    }
    let tenant = k
        .create_tenant(NewTenant {
            slug: "itreports".into(),
            name: "Acme IT".into(),
            admin_email: "admin@acme.test".into(),
            admin_password: "Correct-Horse-9".into(),
            admin_name: None,
            packages: vec!["itsm".into(), "itsm_audit".into()],
            settings: Some(json!({"timezone": "UTC"})),
        })
        .await
        .expect("tenant");
    let session = k.login("itreports", "admin@acme.test", "Correct-Horse-9").await.expect("login");
    let admin = k.load_ctx(tenant, session.user, Some(session.token_version), "it".into()).await.expect("ctx");
    let t = T { k: k.clone(), tenant, admin };
    let group = t.k.insert(&t.admin, "ItGroup", obj(json!({"group_name": "Network Ops"}))).await.unwrap();
    for (title, impact) in [("VPN down", "High"), ("Slow wifi", "Low")] {
        t.k.insert(&t.admin, "Incident", obj(json!({"title": title, "impact": impact, "urgency": "High", "assignment_group": group["id"]}))).await.unwrap();
    }
    let auditor = t.user("audit@acme.test", &["Desk Auditor"]).await;
    let desk = t.user("desk@acme.test", &["Service Desk"]).await;

    a_report_refuses_grouping_by_a_field_the_caller_cannot_read(&t, &auditor, &desk).await;
    a_report_s_default_falls_back_to_a_readable_grouping(&t, &auditor, &desk).await;
    a_report_leaves_out_columns_the_caller_cannot_read(&t, &auditor, &desk).await;
}

async fn a_report_refuses_grouping_by_a_field_the_caller_cannot_read(t: &T, auditor: &Ctx, desk: &Ctx) {
    for name in ["itsm_open_incidents", "itsm_target_compliance", "itsm_time_to_resolve"] {
        let r = t.run(auditor, name, &[("group_by", "Priority")]).await;
        assert!(matches!(&r, Err(Error::Forbidden(m)) if m.contains("priority")), "{name}: {r:?}");
        t.run(auditor, name, &[("group_by", "Assignment Group")]).await.unwrap_or_else(|e| panic!("{name} by group: {e:?}"));
        t.run(desk, name, &[("group_by", "Priority")]).await.unwrap_or_else(|e| panic!("{name} for the desk: {e:?}"));
    }
}

async fn a_report_s_default_falls_back_to_a_readable_grouping(t: &T, auditor: &Ctx, desk: &Ctx) {
    let mine = t.run(auditor, "itsm_open_incidents", &[]).await.expect("by the next grouping it reads");
    assert_eq!(mine["meta"]["group_by"], "Assignment Group", "{mine}");
    assert_eq!(mine["rows"][0]["group"], "Network Ops");
    assert_eq!(mine["totals"]["incidents"], json!(2));
    let theirs = t.run(desk, "itsm_open_incidents", &[]).await.unwrap();
    assert_eq!(theirs["meta"]["group_by"], "Priority", "the desk keeps the default");
}

async fn a_report_leaves_out_columns_the_caller_cannot_read(t: &T, auditor: &Ctx, desk: &Ctx) {
    let mine = t.run(auditor, "itsm_open_incidents", &[]).await.unwrap();
    assert!(!columns(&mine).contains(&"breached".to_string()), "the resolution target is not read: {:?}", columns(&mine));
    assert!(mine["rows"].as_array().unwrap().iter().all(|r| r.get("breached").is_none()) && mine["totals"].get("breached").is_none(), "{mine}");
    let comp = t.run(auditor, "itsm_target_compliance", &[("group_by", "Assignment Group")]).await.unwrap();
    let cols = columns(&comp);
    assert!(cols.contains(&"response_met".to_string()) && !cols.iter().any(|c| c.starts_with("resolution_")), "{cols:?}");
    // Change success is grouped by the class, which the auditor does not read: refused, not answered.
    let r = t.run(auditor, "itsm_change_success", &[]).await;
    assert!(matches!(&r, Err(Error::Forbidden(m)) if m.contains("kind")), "{r:?}");
    let theirs = t.run(desk, "itsm_open_incidents", &[]).await.unwrap();
    assert!(columns(&theirs).contains(&"breached".to_string()));
}
