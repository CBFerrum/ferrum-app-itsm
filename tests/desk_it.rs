//! The service desk's people and records, against a real PostgreSQL (`FERRUM_ITSM_TEST_DATABASE_URL`,
//! default `postgres://ferrum:ferrum@localhost:5432/ferrum_itsm_test`; reset first): who an incident
//! is about and who works it, categories, the priority matrix kept as records, work notes and
//! comments, targets on the platform's clock, the employee portal, catalogue forms, change boards,
//! and the reports. The older scenarios are in tests/itsm_it.rs.

use ferrum_kernel::secret::SecretKey;
use ferrum_kernel::tenant::NewTenant;
use ferrum_kernel::{Ctx, Db, DbConfig, Doc, Error, HookRegistry, Kernel, KernelSettings, ListQuery};
use ferrum_meta::Catalog;
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;

fn obj(v: Value) -> Doc {
    v.as_object().cloned().unwrap()
}

fn uid(v: &Value) -> Uuid {
    v.as_str().unwrap_or_else(|| panic!("not an id: {v}")).parse().unwrap()
}

struct T {
    k: Arc<Kernel>,
    tenant: Uuid,
    admin: Ctx,
}

impl T {
    async fn insert(&self, ctx: &Ctx, entity: &str, v: Value) -> Doc {
        self.k.insert(ctx, entity, obj(v)).await.unwrap_or_else(|e| panic!("insert {entity}: {e:?}"))
    }
    async fn get(&self, entity: &str, id: Uuid) -> Doc {
        self.k.get(&self.admin, entity, id, false).await.unwrap_or_else(|e| panic!("get {entity}: {e:?}"))
    }
    async fn user(&self, email: &str, roles: &[&str]) -> Ctx {
        let roles: Vec<String> = roles.iter().map(|r| r.to_string()).collect();
        let id = self.k.create_user(&self.admin, email, email, Some("Correct-Horse-9"), &roles, false).await.expect("user");
        let s = self.k.login("itdesk", email, "Correct-Horse-9").await.expect("login");
        self.k.load_ctx(self.tenant, id, Some(s.token_version), "it".into()).await.expect("ctx")
    }
    /// The id of the record whose `field` holds `value`.
    async fn named(&self, entity: &str, field: &str, value: &str) -> Uuid {
        let q = ListQuery { filters: ferrum_kernel::query::parse_filters(&json!({field: value})).unwrap(), ..Default::default() };
        uid(&self.k.list(&self.admin, entity, &q).await.unwrap().rows[0]["id"])
    }
    #[allow(dead_code)]
    async fn act(&self, ctx: &Ctx, entity: &str, id: Uuid, action: &str, params: Value) -> Result<Value, Error> {
        self.k.run_action(ctx, entity, id, action, params).await
    }
    async fn set(&self, ctx: &Ctx, entity: &str, id: Uuid, v: Value) -> Result<Doc, Error> {
        self.k.update(ctx, entity, id, obj(v), None).await
    }
}

fn refused<V: std::fmt::Debug>(r: Result<V, Error>, kind: &str, says: &str) {
    let msg = format!("{r:?}");
    let ok = matches!(
        (&r, kind),
        (Err(Error::Invalid(_) | Error::Validation(_)), "invalid") | (Err(Error::Forbidden(_)), "forbidden") | (Err(Error::NotFound(_)), "not found")
    );
    assert!(ok && msg.contains(says), "expected {kind} saying {says:?}, got {msg}");
}

#[test]
fn a_service_desk() {
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().thread_stack_size(16 << 20).build().unwrap();
    rt.block_on(async { tokio::spawn(scenario()).await.unwrap() });
}

async fn scenario() {
    let url = std::env::var("FERRUM_ITSM_TEST_DATABASE_URL").unwrap_or_else(|_| "postgres://ferrum:ferrum@localhost:5432/ferrum_itsm_test".into());
    let db = Db::connect(DbConfig { url, max_connections: 16, ..Default::default() }).await.expect("connect");
    sqlx::raw_sql("drop schema if exists public cascade; create schema public;").execute(&db.pool).await.expect("reset");
    let mut packages = ferrum_domain::packages().expect("platform packages");
    packages.extend(ferrum_app_itsm::packages().expect("app packages").into_iter().filter(|p| p.name() != ferrum_app_itsm::CONNECTOR_PACKAGE));
    let catalog = Catalog::from_packages(packages).expect("catalog");
    let mut hooks = HookRegistry::default();
    ferrum_domain::register(&mut hooks);
    ferrum_app_itsm::register(&mut hooks);
    let settings = KernelSettings { secret_key: Some(SecretKey::new([7u8; 32])), ..Default::default() };
    let k = Kernel::boot(db, catalog, hooks, settings, None).await.expect("boot");
    ferrum_domain::maintenance(&k).await.expect("partitions");
    let tenant = k
        .create_tenant(NewTenant {
            slug: "itdesk".into(),
            name: "Acme IT".into(),
            admin_email: "admin@acme.test".into(),
            admin_password: "Correct-Horse-9".into(),
            admin_name: None,
            packages: vec!["itsm".into()],
            settings: Some(json!({"timezone": "UTC"})),
        })
        .await
        .expect("tenant");
    let session = k.login("itdesk", "admin@acme.test", "Correct-Horse-9").await.expect("login");
    let admin = k.load_ctx(tenant, session.user, Some(session.token_version), "it".into()).await.expect("ctx");
    let t = T { k: k.clone(), tenant, admin };
    let desk = t.user("desk@acme.test", &["Service Desk"]).await;
    let cfg = t.user("cmdb@acme.test", &["Configuration Manager"]).await;

    // ───────────── 1. who and what ─────────────
    let group =
        uid(&t.insert(&cfg, "ItGroup", json!({"group_name": "Network Ops", "lead": "lead@acme.test", "members": [{"member": "Net@acme.test"}]})).await["id"]);
    let hardware = t.named("ItCategory", "category_name", "Hardware").await;
    let laptop = t.named("ItCategory", "category_name", "Laptop").await;
    let vpn = t.named("ItCategory", "category_name", "VPN").await;
    let svc = uid(&t.insert(&cfg, "ConfigurationItem", json!({"ci_name": "Email", "ci_type": "Business Service"})).await["id"]);
    let inc = t
        .insert(
            &desk,
            "Incident",
            json!({"title": "VPN drops", "impact": "Medium", "urgency": "Medium", "category": hardware.to_string(), "subcategory": laptop.to_string(),
                   "service": svc.to_string(), "assignment_group": group.to_string(), "assigned_to": "net@acme.test", "reported_by": "boss@acme.test",
                   "caller_name": "Dana (visitor)", "caller_email": "dana@outside.test"}),
        )
        .await;
    let inc_id = uid(&inc["id"]);
    assert_eq!(
        (inc["priority"].as_str(), inc["caller_email"].as_str(), inc["reported_by"].as_str()),
        (Some("P3"), Some("dana@outside.test"), Some("boss@acme.test"))
    );
    refused(t.set(&desk, "Incident", inc_id, json!({"subcategory": vpn.to_string()})).await, "invalid", "subcategory of the category");
    refused(t.set(&desk, "Incident", inc_id, json!({"assigned_to": "someone@acme.test"})).await, "invalid", "not a member of the assignment group");
    let plain = t.insert(&desk, "Incident", json!({"title": "No caller given"})).await;
    assert_eq!(plain["caller_email"], "desk@acme.test", "a caller left empty is the person raising it");
    // The matrix is records: today's values are seeded, and a change is seen by the next save.
    let rows = t.k.list(&t.admin, "PriorityMatrix", &Default::default()).await.unwrap().rows;
    assert_eq!(rows.len(), 9);
    let mm = rows.iter().find(|r| r["impact"] == "Medium" && r["urgency"] == "Medium").unwrap();
    t.set(&cfg, "PriorityMatrix", uid(&mm["id"]), json!({"priority": "P2"})).await.unwrap();
    t.set(&desk, "Incident", inc_id, json!({"title": "VPN drops again"})).await.unwrap();
    assert_eq!(t.get("Incident", inc_id).await["priority"], "P2", "its priority follows the matrix");
    refused(t.set(&desk, "PriorityMatrix", uid(&mm["id"]), json!({"priority": "P1"})).await, "forbidden", "");
    refused(t.set(&cfg, "PriorityMatrix", uid(&mm["id"]), json!({"resolution_time": "four hours"})).await, "invalid", "ISO 8601");
}
