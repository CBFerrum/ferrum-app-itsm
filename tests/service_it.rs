//! Problems and known errors, major incidents, catalogue tasks and the configuration database's classes and
//! relation types, against a real PostgreSQL (`FERRUM_ITSM_SERVICE_TEST_DATABASE_URL`, default
//! `postgres://ferrum:ferrum@localhost:5432/ferrum_itsm_service_test`; reset first).

use ferrum_kernel::secret::SecretKey;
use ferrum_kernel::tenant::NewTenant;
use ferrum_kernel::{Ctx, Db, DbConfig, Doc, Error, HookRegistry, Kernel, KernelSettings, ListQuery, SiteRequest};
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
    slug: &'static str,
    tenant: Uuid,
    admin: Ctx,
}

#[allow(dead_code)]
impl T {
    async fn new(k: &Arc<Kernel>, slug: &'static str) -> T {
        let tenant = k
            .create_tenant(NewTenant {
                slug: slug.into(),
                name: "Acme IT".into(),
                admin_email: "admin@acme.test".into(),
                admin_password: "Correct-Horse-9".into(),
                admin_name: None,
                packages: vec!["itsm".into()],
                settings: Some(json!({"timezone": "UTC"})),
            })
            .await
            .expect("tenant");
        let session = k.login(slug, "admin@acme.test", "Correct-Horse-9").await.expect("login");
        let admin = k.load_ctx(tenant, session.user, Some(session.token_version), "it".into()).await.expect("ctx");
        T { k: k.clone(), slug, tenant, admin }
    }
    async fn insert(&self, ctx: &Ctx, entity: &str, v: Value) -> Doc {
        self.k.insert(ctx, entity, obj(v)).await.unwrap_or_else(|e| panic!("insert {entity}: {e:?}"))
    }
    async fn get(&self, entity: &str, id: Uuid) -> Doc {
        self.k.get(&self.admin, entity, id, false).await.unwrap_or_else(|e| panic!("get {entity}: {e:?}"))
    }
    async fn user(&self, email: &str, roles: &[&str]) -> Ctx {
        let roles: Vec<String> = roles.iter().map(|r| r.to_string()).collect();
        let id = self.k.create_user(&self.admin, email, email, Some("Correct-Horse-9"), &roles, false).await.expect("user");
        let s = self.k.login(self.slug, email, "Correct-Horse-9").await.expect("login");
        self.k.load_ctx(self.tenant, id, Some(s.token_version), "it".into()).await.expect("ctx")
    }
    async fn named(&self, entity: &str, field: &str, value: &str) -> Uuid {
        let q = ListQuery { filters: ferrum_kernel::query::parse_filters(&json!({field: value})).unwrap(), ..Default::default() };
        uid(&self.k.list(&self.admin, entity, &q).await.unwrap().rows[0]["id"])
    }
    async fn act(&self, ctx: &Ctx, entity: &str, id: Uuid, action: &str, params: Value) -> Result<Value, Error> {
        self.k.run_action(ctx, entity, id, action, params).await
    }
    async fn set(&self, ctx: &Ctx, entity: &str, id: Uuid, v: Value) -> Result<Doc, Error> {
        self.k.update(ctx, entity, id, obj(v), None).await
    }
    async fn settings(&self) -> Uuid {
        uid(&self.k.list(&self.admin, "ItsmSettings", &Default::default()).await.unwrap().rows[0]["id"])
    }
    async fn decide(&self, who: &Ctx, entity: &str, id: Uuid, approve: bool) -> Result<Value, Error> {
        self.k.decide_approval(who, entity, id, approve, if approve { None } else { Some("no") }).await
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
fn service_management() {
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().thread_stack_size(16 << 20).build().unwrap();
    rt.block_on(async { tokio::spawn(scenario()).await.unwrap() });
}

async fn boot() -> Arc<Kernel> {
    let url =
        std::env::var("FERRUM_ITSM_SERVICE_TEST_DATABASE_URL").unwrap_or_else(|_| "postgres://ferrum:ferrum@localhost:5432/ferrum_itsm_service_test".into());
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
    k
}

async fn scenario() {
    let k = boot().await;
    let t = T::new(&k, "svc").await;
    let desk = t.user("desk@acme.test", &["Service Desk"]).await;
    let cfg = t.user("cmdb@acme.test", &["Configuration Manager"]).await;
    let ann = t.user("ann@acme.test", &["IT Self Service"]).await;
    ferrum_app_itsm::maintenance(&k).await.unwrap();

    // ───────────── 2. problems, known errors, major incidents ─────────────
    let mail = uid(&t
        .insert(
            &cfg,
            "ConfigurationItem",
            json!({"ci_name": "Mail", "ci_type": "Business Service",
        "subscribers": [{"subscriber": "ops@acme.test"}, {"subscriber": "boss@acme.test"}]}),
        )
        .await["id"]);
    let i1 = uid(&t.insert(&desk, "Incident", json!({"title": "Mail is slow", "service": mail.to_string()})).await["id"]);
    let i2 = uid(&t.insert(&desk, "Incident", json!({"title": "Mail will not send", "service": mail.to_string()})).await["id"]);
    let prb =
        uid(&t.insert(&desk, "Problem", json!({"title": "Mail queue stalls", "ci": mail.to_string(), "symptoms": "slow mail, outbox stuck"})).await["id"]);
    t.set(&desk, "Incident", i1, json!({"problem": prb.to_string()})).await.unwrap();
    t.set(&desk, "Incident", i2, json!({"problem": prb.to_string()})).await.unwrap();
    // A known error needs its workaround, and only a known error's workaround is published.
    refused(t.set(&desk, "Problem", prb, json!({"known_error": true})).await, "invalid", "has its workaround");
    refused(t.set(&desk, "Problem", prb, json!({"workaround_visible": true, "workaround": "Restart the queue"})).await, "invalid", "only a known error");
    assert!(t.k.list(&ann, "KnownError", &Default::default()).await.unwrap().rows.is_empty());
    t.set(&desk, "Problem", prb, json!({"known_error": true, "workaround": "Restart the queue", "workaround_visible": true})).await.unwrap();
    let suggest = |who: &Ctx, q: &str| SiteRequest {
        tenant: t.tenant,
        user: Some(who.clone()),
        method: "GET".into(),
        path: "/suggest".into(),
        query: json!({"q": q}).as_object().cloned().unwrap(),
        headers: Default::default(),
        body: vec![],
        client_ip: "10.0.0.2".into(),
    };
    let site = k.hooks.site("itsm").cloned().unwrap();
    let said: Value = serde_json::from_slice(&site.handle(&k, suggest(&ann, "outbox")).await.unwrap().body).unwrap();
    assert_eq!(said["suggestions"][0]["workaround"], "Restart the queue", "the portal's suggestions find it by its symptoms: {said}");
    let desk_found = t.k.list(&desk, "KnownError", &ListQuery { search: Some("queue".into()), ..Default::default() }).await.unwrap();
    assert_eq!(desk_found.rows.len(), 1, "and the desk's search");
    // Unmarked, it is no longer shown to the portal.
    t.set(&desk, "Problem", prb, json!({"workaround_visible": false})).await.unwrap();
    assert!(t.k.list(&ann, "KnownError", &Default::default()).await.unwrap().rows.is_empty());
    t.set(&desk, "Problem", prb, json!({"workaround_visible": true})).await.unwrap();
    assert_eq!(t.k.list(&ann, "KnownError", &Default::default()).await.unwrap().rows.len(), 1);

    // Closing the problem offers: the incidents stay open until the button is pressed (a new workspace's setting).
    t.set(&desk, "Problem", prb, json!({"status": "Resolved", "root_cause": "A full disk"})).await.unwrap();
    assert_eq!(t.get("Incident", i1).await["status"], "New");
    t.act(&desk, "Problem", prb, "resolve_linked_incidents", Value::Null).await.unwrap();
    for i in [i1, i2] {
        let d = t.get("Incident", i).await;
        assert_eq!(d["status"], "Resolved");
        assert!(d["resolution"].as_str().unwrap().contains("A full disk"));
    }
    // Set to Automatically, as an upgraded workspace has it, closing resolves them as it did before.
    t.set(&t.admin, "ItsmSettings", t.settings().await, json!({"problem_closes_incidents": "Automatically"})).await.unwrap();
    let p2 = uid(&t.insert(&desk, "Problem", json!({"title": "Disk fills"})).await["id"]);
    let i3 = uid(&t.insert(&desk, "Incident", json!({"title": "Disk full", "problem": p2.to_string()})).await["id"]);
    t.set(&desk, "Problem", p2, json!({"status": "Resolved", "root_cause": "Logs"})).await.unwrap();
    assert_eq!(t.get("Incident", i3).await["status"], "Resolved");

    // A major incident: declared with its plan, told to the service's subscribers, updated, resolved.
    let m = uid(&t.insert(&desk, "Incident", json!({"title": "Mail is down", "service": mail.to_string(), "impact": "High", "urgency": "High"})).await["id"]);
    refused(t.act(&desk, "Incident", m, "major_update", json!({"update": "x"})).await, "invalid", "not available");
    t.act(&desk, "Incident", m, "declare_major", json!({"plan": "Updates every 30 minutes by mail"})).await.unwrap();
    let d = t.get("Incident", m).await;
    assert_eq!((d["major"].as_bool(), d["major_declared"].as_bool(), d["major_manager"].as_str()), (Some(true), Some(true), Some("desk@acme.test")));
    t.act(&desk, "Incident", m, "major_update", json!({"update": "The queue is draining", "next_update_by": "2026-11-13T12:00:00Z"})).await.unwrap();
    assert!(t.get("Incident", m).await["next_update_by"].as_str().unwrap().starts_with("2026-11-13T12:00"));
    t.act(&desk, "Incident", m, "end_major", json!({"resolution": "Queue cleared"})).await.unwrap();
    assert_eq!(t.get("Incident", m).await["status"], "Resolved");
    let sent = t.k.message_log(&t.admin, Some(("Incident", m)), None, 50).await.unwrap();
    for template in ["itsm.major_declared", "itsm.major_update", "itsm.major_resolved"] {
        let to: Vec<&str> = sent.iter().filter(|s| s["template"] == template).filter_map(|s| s["recipient"].as_str()).collect();
        assert_eq!(to.len(), 2, "{template} went to both subscribers: {sent:?}");
        assert!(to.contains(&"ops@acme.test") && to.contains(&"boss@acme.test"));
    }

    // ───────────── 3. catalog items: an approver group, then tasks for groups in order ─────────────
    let mgr = t.user("mgr@acme.test", &["Change Manager"]).await;
    let (m1, _m2, m3) =
        (t.user("m1@acme.test", &["Requester"]).await, t.user("m2@acme.test", &["Requester"]).await, t.user("m3@acme.test", &["Requester"]).await);
    let managers = uid(&t
        .insert(
            &cfg,
            "ItGroup",
            json!({"group_name": "Managers", "members": [{"member": "m1@acme.test"}, {"member": "m2@acme.test"}, {"member": "m3@acme.test"}]}),
        )
        .await["id"]);
    let accounts = uid(&t.insert(&cfg, "ItGroup", json!({"group_name": "Accounts", "members": [{"member": "acc@acme.test"}]})).await["id"]);
    let kit = uid(&t.insert(&cfg, "ItGroup", json!({"group_name": "Kit", "members": [{"member": "kit@acme.test"}]})).await["id"]);
    let hire = uid(&t
        .insert(
            &cfg,
            "ServiceOffering",
            json!({"offering_name": "New hire", "needs_approval": true, "approver_group": managers.to_string(), "approvals_needed": 2,
        "steps": [{"step_name": "Create the accounts", "sequence": 10, "assignment_group": accounts.to_string(), "target_hours": 4},
                  {"step_name": "Prepare the laptop", "sequence": 20, "assignment_group": kit.to_string(), "target_hours": 24},
                  {"step_name": "Prepare the desk", "sequence": 20, "assignment_group": kit.to_string(), "target_hours": 24}]}),
        )
        .await["id"]);
    let r = uid(&t.insert(&desk, "CatalogRequest", json!({"offering": hire.to_string()})).await["id"]);
    assert_eq!(t.get("CatalogRequest", r).await["route"], "Group");
    assert!(t.k.list(&t.admin, "RequestTask", &Default::default()).await.unwrap().rows.is_empty(), "no task before approval");
    refused(t.act(&m1, "CatalogRequest", r, "approve_request", Value::Null).await, "forbidden", "not permitted");
    let asked = t.act(&desk, "CatalogRequest", r, "approve_request", Value::Null).await.unwrap();
    assert!(asked["_approval"].is_object(), "{asked}");
    t.decide(&m1, "CatalogRequest", r, true).await.unwrap();
    assert_eq!(t.get("CatalogRequest", r).await["status"], "Submitted", "one of two");
    t.decide(&m3, "CatalogRequest", r, true).await.unwrap();
    assert_eq!(t.get("CatalogRequest", r).await["status"], "Approved");
    let tasks = t.k.list(&t.admin, "RequestTask", &Default::default()).await.unwrap().rows;
    let by = |n: &str| tasks.iter().find(|x| x["step_name"] == n).map(|x| uid(&x["id"])).unwrap();
    let state = |n: &str| tasks.iter().find(|x| x["step_name"] == n).map(|x| x["status"].as_str().unwrap().to_string()).unwrap();
    assert_eq!((state("Create the accounts").as_str(), state("Prepare the laptop").as_str()), ("Open", "Waiting"));
    let first = t.get("RequestTask", by("Create the accounts")).await;
    assert!(!first["due_by"].is_null() && first["task_state"] == "running", "an open task's clock runs: {first:?}");
    refused(t.act(&desk, "CatalogRequest", r, "fulfil", Value::Null).await, "invalid", "tasks are not all done");
    t.act(&desk, "RequestTask", by("Create the accounts"), "complete_task", Value::Null).await.unwrap();
    for n in ["Prepare the laptop", "Prepare the desk"] {
        assert_eq!(t.get("RequestTask", by(n)).await["status"], "Open", "{n} opens when the step before is done");
        let g = t.get("RequestTask", by(n)).await;
        assert!(!g["due_by"].is_null(), "and has its target: {:?} {:?}", g["task_state"], g["due_by"]);
    }
    t.act(&desk, "RequestTask", by("Prepare the laptop"), "complete_task", Value::Null).await.unwrap();
    assert_eq!(t.get("CatalogRequest", r).await["status"], "Approved");
    t.act(&desk, "RequestTask", by("Prepare the desk"), "complete_task", Value::Null).await.unwrap();
    assert_eq!(t.get("CatalogRequest", r).await["status"], "Fulfilled", "the request closes when its tasks do");
    assert_eq!(t.get("RequestTask", by("Prepare the desk")).await["task_state"], "met");
    assert_eq!(t.get("CatalogRequest", r).await["fulfil_state"], "met", "and its own target is met");
    // An offering with no approver group asks one Change Manager, as before; one with no approval and steps run at once.
    let old = uid(&t.insert(&cfg, "ServiceOffering", json!({"offering_name": "Software", "needs_approval": true})).await["id"]);
    let ro = uid(&t.insert(&desk, "CatalogRequest", json!({"offering": old.to_string()})).await["id"]);
    assert_eq!(t.get("CatalogRequest", ro).await["route"], "Manager");
    refused(t.act(&desk, "CatalogRequest", ro, "approve_request", Value::Null).await, "forbidden", "a Change Manager decides");
    t.act(&mgr, "CatalogRequest", ro, "approve_request", Value::Null).await.expect("one Change Manager, as before");
    assert_eq!(t.get("CatalogRequest", ro).await["status"], "Approved");
    t.set(&cfg, "ServiceOffering", old, json!({"needs_approval": false, "steps_run": "At Once",
        "steps": [{"step_name": "A", "sequence": 10, "assignment_group": accounts.to_string()}, {"step_name": "B", "sequence": 20, "assignment_group": kit.to_string()}]})).await.unwrap();
    let rn = uid(&t.insert(&desk, "CatalogRequest", json!({"offering": old.to_string()})).await["id"]);
    let ts =
        t.k.list(
            &t.admin,
            "RequestTask",
            &ListQuery { filters: ferrum_kernel::query::parse_filters(&json!({"request": rn.to_string()})).unwrap(), ..Default::default() },
        )
        .await
        .unwrap()
        .rows;
    assert!(ts.len() == 2 && ts.iter().all(|x| x["status"] == "Open"), "all open at once: {ts:?}");
}
