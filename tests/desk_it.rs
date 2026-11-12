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

    // ───────────── 2. work notes and comments ─────────────
    let other = t.insert(&desk, "Incident", json!({"title": "Printer jam", "caller_email": "eve@acme.test"})).await;
    let other_id = uid(&other["id"]);
    let ann = t.user("ann@acme.test", &["IT Self Service"]).await;
    let on_mine = t.insert(&desk, "Incident", json!({"title": "Laptop will not boot", "caller_email": "ann@acme.test"})).await;
    let mine_id = uid(&on_mine["id"]);
    let note = |on: Uuid, kind: &str, body: &str| json!({"incident": on.to_string(), "visibility": kind, "body": body});
    let wn = uid(&t.insert(&desk, "ItNote", note(mine_id, "Internal", "Looks like the SSD")).await["id"]);
    let cm = t.insert(&desk, "ItNote", note(mine_id, "Visible to Caller", "We are on it")).await;
    assert_eq!((cm["author"].as_str(), cm["caller_email"].as_str()), (Some("desk@acme.test"), Some("ann@acme.test")));
    t.insert(&desk, "ItNote", note(other_id, "Visible to Caller", "For Eve only")).await;
    refused(t.k.insert(&desk, "ItNote", obj(json!({"body": "Floating"}))).await, "invalid", "on one incident");
    refused(t.k.insert(&desk, "ItNote", obj(json!({"problem": Uuid::nil().to_string(), "visibility": "Visible to Caller", "body": "x"}))).await, "invalid", "");
    // The caller, as a portal person, reads only the visible ones on her own records.
    let seen = t.k.list(&ann, "ItNote", &Default::default()).await.unwrap().rows;
    assert_eq!(seen.iter().map(|r| r["body"].as_str().unwrap()).collect::<Vec<_>>(), ["We are on it"], "no work note, nothing of Eve's");
    refused(t.k.get(&ann, "ItNote", wn, false).await, "not found", "");
    let mine = t.k.list(&ann, "Incident", &Default::default()).await.unwrap().rows;
    assert_eq!(mine.iter().map(|r| r["title"].as_str().unwrap()).collect::<Vec<_>>(), ["Laptop will not boot"]);
    // She may comment on her own incident, visibly, and on no other, nor leave a work note.
    let reply = t.insert(&ann, "ItNote", note(mine_id, "Visible to Caller", "Thanks, it is the black one")).await;
    assert_eq!(reply["author"], "ann@acme.test");
    refused(t.k.insert(&ann, "ItNote", obj(note(other_id, "Visible to Caller", "Snooping"))).await, "forbidden", "");
    refused(t.k.insert(&ann, "ItNote", obj(note(mine_id, "Internal", "A secret note"))).await, "forbidden", "");
    assert_eq!(t.k.list(&desk, "ItNote", &Default::default()).await.unwrap().rows.len(), 4, "staff read all, in order");
    // A note is corrected by its author within the grace period, and then it is final; nobody but an administrator deletes.
    let reply_id = uid(&reply["id"]);
    t.set(&ann, "ItNote", reply_id, json!({"body": "Thanks, it is the black one (T14)"})).await.expect("within the grace period");
    refused(t.set(&desk, "ItNote", reply_id, json!({"body": "Rewritten by staff"})).await, "invalid", "only by its author");
    refused(t.set(&desk, "ItNote", uid(&cm["id"]), json!({"incident": other_id.to_string()})).await, "invalid", "stays on its record");
    t.insert(&cfg, "ItsmSettings", json!({"note_edit_minutes": 0})).await;
    refused(t.set(&ann, "ItNote", reply_id, json!({"body": "Too late"})).await, "invalid", "within 0 minutes");
    refused(t.k.delete(&desk, "ItNote", wn, None).await, "forbidden", "");
    t.k.delete(&t.admin, "ItNote", wn, None).await.expect("an administrator deletes");
}
