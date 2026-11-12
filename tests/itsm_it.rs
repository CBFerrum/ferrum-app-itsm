//! An IT department (FERRUM-ECOSYSTEM-002, -003) against a real PostgreSQL
//! (`FERRUM_ITSM_TEST_DATABASE_URL`, default `postgres://ferrum:ferrum@localhost:5432/ferrum_itsm_test`;
//! reset first): configuration items whose dependencies never loop; catalog requests approved by a
//! Change Manager other than who asked, and seen only by who asked; incidents prioritised by impact
//! and urgency with their targets, major on a critical item, and breached when late; a problem
//! that resolves its incidents; changes kept out of freezes and off each other's items, decided by
//! someone other than who raised them, with their impact; who may do what; and discovery reports
//! matched to the items they are, kept apart from what people keep, and items no longer reported
//! marked for a person. The connector with Assets has its own test (tests/assets_it.rs).

use chrono::{Duration, Utc};
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
        let s = self.k.login("itdept", email, "Correct-Horse-9").await.expect("login");
        self.k.load_ctx(self.tenant, id, Some(s.token_version), "it".into()).await.expect("ctx")
    }
    async fn act(&self, ctx: &Ctx, entity: &str, id: Uuid, action: &str, params: Value) -> Result<Value, Error> {
        self.k.run_action(ctx, entity, id, action, params).await
    }
    async fn set(&self, ctx: &Ctx, entity: &str, id: Uuid, v: Value) -> Result<Doc, Error> {
        self.k.update(ctx, entity, id, obj(v), None).await
    }
}

/// A discovery report, as a tool posts it to `/api/app/itsm/discovery`, from `who`.
async fn report(k: &Kernel, tenant: Uuid, who: Option<&Ctx>, body: Value) -> Result<Value, Error> {
    let site = k.hooks.site("itsm").cloned().expect("IT Services' endpoints");
    let req = SiteRequest {
        tenant,
        user: who.cloned(),
        method: "POST".into(),
        path: "/discovery".into(),
        query: Default::default(),
        headers: Default::default(),
        body: serde_json::to_vec(&body).unwrap(),
        client_ip: "10.0.0.2".into(),
    };
    let r = site.handle(k, req).await?;
    Ok(serde_json::from_slice(&r.body).unwrap())
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
fn an_it_department() {
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().thread_stack_size(16 << 20).build().unwrap();
    rt.block_on(async { tokio::spawn(scenario()).await.unwrap() });
}

async fn scenario() {
    let url = std::env::var("FERRUM_ITSM_TEST_DATABASE_URL").unwrap_or_else(|_| "postgres://ferrum:ferrum@localhost:5432/ferrum_itsm_test".into());
    let db = Db::connect(DbConfig { url, max_connections: 16, ..Default::default() }).await.expect("connect");
    sqlx::raw_sql("drop schema if exists public cascade; create schema public;").execute(&db.pool).await.expect("reset");
    let mut packages = ferrum_domain::packages().expect("platform packages");
    // IT Services alone: its connector with Assets needs Assets' packages (tests/assets_it.rs).
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
            slug: "itdept".into(),
            name: "Acme IT".into(),
            admin_email: "admin@acme.test".into(),
            admin_password: "Correct-Horse-9".into(),
            admin_name: None,
            packages: vec!["itsm".into()],
            settings: Some(json!({"timezone": "UTC"})),
        })
        .await
        .expect("tenant");
    let session = k.login("itdept", "admin@acme.test", "Correct-Horse-9").await.expect("login");
    let admin = k.load_ctx(tenant, session.user, Some(session.token_version), "it".into()).await.expect("ctx");
    let t = T { k: k.clone(), tenant, admin };
    let desk = t.user("desk@acme.test", &["Service Desk"]).await;
    let cab = t.user("cab@acme.test", &["Change Manager"]).await;
    let cab2 = t.user("cab2@acme.test", &["Change Manager", "Requester"]).await;
    let cfg = t.user("cmdb@acme.test", &["Configuration Manager"]).await;
    let req = t.user("ann@acme.test", &["Requester"]).await;
    let req2 = t.user("bob@acme.test", &["Requester"]).await;

    // ───────────── configuration items ─────────────
    let ci = |name: &str, kind: &str, criticality: &str, relations: Value| json!({"ci_name": name, "ci_type": kind, "criticality": criticality, "relations": relations});
    let rel = |kind: &str, target: Uuid| json!({"kind": kind, "target": target.to_string()});
    let db_server = uid(&t.insert(&cfg, "ConfigurationItem", ci("db-server-1", "Server", "High", json!([]))).await["id"]);
    let app_server = uid(&t.insert(&cfg, "ConfigurationItem", ci("app-server-1", "Server", "High", json!([]))).await["id"]);
    let db = uid(&t.insert(&cfg, "ConfigurationItem", ci("payroll-db", "Database", "High", json!([rel("Runs On", db_server)]))).await["id"]);
    let app = uid(&t
        .insert(&cfg, "ConfigurationItem", ci("Payroll App", "Application", "High", json!([rel("Runs On", app_server), rel("Depends On", db)])))
        .await["id"]);
    let payroll = uid(&t.insert(&cfg, "ConfigurationItem", ci("Payroll", "Business Service", "Critical", json!([rel("Depends On", app)]))).await["id"]);
    refused(t.set(&cfg, "ConfigurationItem", db_server, json!({"relations": [rel("Depends On", db_server)]})).await, "invalid", "does not relate to itself");
    refused(t.set(&cfg, "ConfigurationItem", app, json!({"relations": [rel("Depends On", db), rel("Depends On", db)]})).await, "invalid", "listed twice");
    refused(t.set(&cfg, "ConfigurationItem", db_server, json!({"relations": [rel("Depends On", payroll)]})).await, "invalid", "come back round to this item");
    t.set(&cfg, "ConfigurationItem", db_server, json!({"relations": [rel("Connects To", payroll)]})).await.expect("a connection is not a dependency");

    // ───────────── the catalog ─────────────
    let laptop = uid(&t.insert(&cfg, "ServiceOffering", json!({"offering_name": "New laptop", "needs_approval": true, "fulfil_hours": 48})).await["id"]);
    let reset = uid(&t.insert(&cfg, "ServiceOffering", json!({"offering_name": "Password reset", "fulfil_hours": 1})).await["id"]);
    let gone = uid(&t.insert(&cfg, "ServiceOffering", json!({"offering_name": "Fax line", "active": false})).await["id"]);
    let before = Utc::now();
    let ask = t.insert(&req, "CatalogRequest", json!({"offering": laptop.to_string()})).await;
    let ask_id = uid(&ask["id"]);
    assert_eq!(ask["requested_by"], "ann@acme.test");
    let due = chrono::DateTime::parse_from_rfc3339(ask["due_at"].as_str().unwrap()).unwrap().with_timezone(&Utc);
    assert!(due >= before + Duration::hours(48) && due <= Utc::now() + Duration::hours(48), "due its offering's 48 hours later: {due}");
    let bobs = uid(&t.insert(&req2, "CatalogRequest", json!({"offering": reset.to_string()})).await["id"]);
    let mine = t.k.list(&req, "CatalogRequest", &ListQuery::default()).await.unwrap().rows;
    assert_eq!(mine.iter().map(|r| r["id"].as_str().unwrap()).collect::<Vec<_>>(), [ask_id.to_string()], "only her own");
    refused(t.k.insert(&req, "CatalogRequest", obj(json!({"offering": gone.to_string()}))).await, "invalid", "not offered any more");
    refused(t.act(&desk, "CatalogRequest", ask_id, "fulfil", Value::Null).await, "invalid", "waits for its approval");
    refused(t.act(&desk, "CatalogRequest", ask_id, "approve_request", Value::Null).await, "forbidden", "a Change Manager decides");
    refused(t.act(&cab, "CatalogRequest", ask_id, "reject_request", Value::Null).await, "invalid", "says why");
    t.act(&cab, "CatalogRequest", ask_id, "approve_request", Value::Null).await.expect("approved");
    t.act(&desk, "CatalogRequest", ask_id, "fulfil", Value::Null).await.expect("fulfilled");
    assert!(t.get("CatalogRequest", ask_id).await["fulfilled_at"].is_string());
    refused(t.act(&cab, "CatalogRequest", bobs, "approve_request", Value::Null).await, "invalid", "needs no approval");
    t.act(&desk, "CatalogRequest", bobs, "fulfil", Value::Null).await.expect("no approval needed");
    let own = uid(&t.insert(&cab2, "CatalogRequest", json!({"offering": laptop.to_string()})).await["id"]);
    refused(t.act(&cab2, "CatalogRequest", own, "approve_request", Value::Null).await, "forbidden", "someone other than who asked");

    // ───────────── incidents ─────────────
    let incident = |title: &str, ci: Uuid, impact: &str, urgency: &str| json!({"title": title, "ci": ci.to_string(), "impact": impact, "urgency": urgency});
    let down = t.insert(&desk, "Incident", incident("Payroll is down", payroll, "High", "High")).await;
    let down_id = uid(&down["id"]);
    assert_eq!((down["priority"].as_str(), down["major"].as_bool()), (Some("P1"), Some(true)), "P1 on a critical item");
    let opened = chrono::DateTime::parse_from_rfc3339(down["opened_at"].as_str().unwrap()).unwrap();
    let by = chrono::DateTime::parse_from_rfc3339(down["resolve_by"].as_str().unwrap()).unwrap();
    assert!((by - opened - Duration::hours(4)).abs() < Duration::minutes(1), "a P1 is to be resolved in four hours: {opened} to {by}");
    let slow = t.insert(&desk, "Incident", incident("Report is slow", app, "Medium", "Low")).await;
    assert_eq!((slow["priority"].as_str(), slow["major"].as_bool()), (Some("P4"), Some(false)));
    let typo = uid(&t.insert(&desk, "Incident", incident("Typo on payslip", app, "Low", "Medium")).await["id"]);
    refused(t.set(&desk, "Incident", typo, json!({"status": "Resolved"})).await, "invalid", "says how");
    t.set(&desk, "Incident", typo, json!({"status": "Resolved", "resolution": "Fixed the template"})).await.unwrap();
    let fixed = t.get("Incident", typo).await;
    assert!(fixed["resolved_at"].is_string() && fixed["resolve_state"] == "met", "resolved in time: {fixed:?}");
    t.set(&desk, "Incident", down_id, json!({"impact": "High", "urgency": "Medium"})).await.unwrap();
    assert_eq!(t.get("Incident", down_id).await["priority"], "P2", "its priority follows its impact and urgency");
    t.set(&desk, "Incident", down_id, json!({"urgency": "High"})).await.unwrap();
    k.sweep_targets_at(Utc::now() + Duration::hours(5)).await.unwrap();
    assert_eq!(t.get("Incident", down_id).await["resolve_state"], "breached", "a P1 open five hours on");
    assert_eq!(t.get("Incident", uid(&slow["id"])).await["resolve_state"], "running", "a P4 has three days");

    // ───────────── problems ─────────────
    let problem = uid(&t.insert(&desk, "Problem", json!({"title": "Payroll database locks", "ci": db.to_string()})).await["id"]);
    refused(t.set(&desk, "Problem", problem, json!({"status": "Known Error"})).await, "invalid", "its workaround");
    t.set(&desk, "Problem", problem, json!({"status": "Known Error", "workaround": "Restart the app nightly"})).await.unwrap();
    t.set(&desk, "Incident", down_id, json!({"problem": problem.to_string()})).await.unwrap();
    t.set(&desk, "Incident", uid(&slow["id"]), json!({"problem": problem.to_string()})).await.unwrap();
    refused(t.set(&desk, "Problem", problem, json!({"status": "Resolved"})).await, "invalid", "its root cause");
    t.set(&desk, "Problem", problem, json!({"status": "Resolved", "root_cause": "A missing index"})).await.unwrap();
    let after = t.get("Incident", down_id).await;
    assert_eq!(after["status"], "Resolved", "resolved with its problem");
    assert!(after["resolution"].as_str().unwrap().contains("A missing index"), "{after:?}");
    assert_eq!(after["resolve_state"], "met_late", "late is late");
    assert_eq!(t.get("Incident", uid(&slow["id"])).await["status"], "Resolved");

    // ───────────── changes ─────────────
    t.insert(&cab, "ChangeFreeze", json!({"reason": "Year end", "starts_at": "2026-12-20T00:00:00Z", "ends_at": "2027-01-05T00:00:00Z"})).await;
    refused(
        t.k.insert(&cab, "ChangeFreeze", obj(json!({"reason": "Backwards", "starts_at": "2026-12-20T00:00:00Z", "ends_at": "2026-12-19T00:00:00Z"}))).await,
        "invalid",
        "after the freeze starts",
    );
    let change = |title: &str, ci: Uuid, kind: &str, from: &str, to: &str, backout: Option<&str>| json!({"title": title, "ci": ci.to_string(), "kind": kind, "planned_start": from, "planned_end": to, "backout_plan": backout});
    refused(
        t.k.insert(&desk, "ItChange", obj(change("Index", db, "Normal", "2026-12-22T10:00:00Z", "2026-12-22T12:00:00Z", Some("Drop the index")))).await,
        "invalid",
        "falls in the freeze for Year end",
    );
    t.k.insert(&desk, "ItChange", obj(change("Hotfix", db, "Emergency", "2026-12-22T10:00:00Z", "2026-12-22T11:00:00Z", Some("Revert"))))
        .await
        .expect("emergencies may");
    refused(
        t.k.insert(&desk, "ItChange", obj(change("No way back", db, "Normal", "2026-11-20T10:00:00Z", "2026-11-20T12:00:00Z", None))).await,
        "invalid",
        "how it is undone",
    );
    refused(
        t.k.insert(&desk, "ItChange", obj(change("Backwards", db, "Normal", "2026-11-20T12:00:00Z", "2026-11-20T10:00:00Z", Some("x")))).await,
        "invalid",
        "after the change starts",
    );
    let index = t.insert(&desk, "ItChange", change("Add an index", db, "Normal", "2026-11-20T10:00:00Z", "2026-11-20T12:00:00Z", Some("Drop the index"))).await;
    assert_eq!(index["impacted"], "Payroll, Payroll App", "what depends on the database, however far round");
    let index_id = uid(&index["id"]);
    t.act(&desk, "ItChange", index_id, "submit_change", Value::Null).await.unwrap();
    refused(t.set(&desk, "ItChange", index_id, json!({"planned_end": "2026-11-20T13:00:00Z"})).await, "invalid", "keeps its item, kind, window");
    refused(t.act(&desk, "ItChange", index_id, "approve_change", Value::Null).await, "forbidden", "a Change Manager decides");
    t.act(&cab, "ItChange", index_id, "approve_change", json!({"note": "Low risk"})).await.expect("approved");
    let clash = uid(&t.insert(&cab, "ItChange", change("Vacuum", db, "Normal", "2026-11-20T11:00:00Z", "2026-11-20T13:00:00Z", Some("Stop it"))).await["id"]);
    t.act(&cab, "ItChange", clash, "submit_change", Value::Null).await.unwrap();
    refused(t.act(&cab, "ItChange", clash, "approve_change", Value::Null).await, "forbidden", "someone other than who raised it");
    refused(t.act(&cab2, "ItChange", clash, "approve_change", Value::Null).await, "invalid", "changes the same item then");
    refused(t.act(&cab2, "ItChange", clash, "reject_change", Value::Null).await, "invalid", "says why");
    t.act(&cab2, "ItChange", clash, "reject_change", json!({"note": "Clashes with the index"})).await.unwrap();
    let later =
        uid(&t.insert(&cab, "ItChange", change("Vacuum later", db, "Normal", "2026-11-20T14:00:00Z", "2026-11-20T15:00:00Z", Some("Stop it"))).await["id"]);
    t.act(&cab, "ItChange", later, "submit_change", Value::Null).await.unwrap();
    t.act(&cab2, "ItChange", later, "approve_change", Value::Null).await.expect("after the index");
    let patch = uid(&t.insert(&desk, "ItChange", change("OS patch", app_server, "Standard", "2026-11-21T02:00:00Z", "2026-11-21T03:00:00Z", None)).await["id"]);
    t.act(&desk, "ItChange", patch, "submit_change", Value::Null).await.unwrap();
    assert_eq!(t.get("ItChange", patch).await["status"], "Approved", "a standard change is approved in advance");
    t.act(&desk, "ItChange", index_id, "implement", Value::Null).await.unwrap();
    t.act(&desk, "ItChange", index_id, "close_change", json!({"note": "Queries 10x faster"})).await.unwrap();
    assert_eq!(t.get("ItChange", index_id).await["status"], "Closed");
    t.act(&desk, "ItChange", later, "implement", Value::Null).await.unwrap();
    refused(t.act(&desk, "ItChange", later, "fail_change", Value::Null).await, "invalid", "says what happened");
    t.act(&desk, "ItChange", later, "fail_change", json!({"note": "Locked the table; backed out"})).await.unwrap();
    assert_eq!(t.get("ItChange", later).await["status"], "Failed");

    // ───────────── who may do what ─────────────
    refused(t.k.insert(&req, "Incident", obj(incident("Mine", app, "Low", "Low"))).await, "forbidden", "");
    refused(t.act(&cfg, "ItChange", patch, "implement", Value::Null).await, "forbidden", "");
    refused(t.k.insert(&desk, "ConfigurationItem", obj(ci("rogue", "Server", "Low", json!([])))).await, "forbidden", "");

    // ───────────── discovery ─────────────
    let agent = t.user("scanner@acme.test", &["Discovery Agent"]).await;
    let found = |key: &str, name: &str, extra: Value| {
        let mut v = json!({"key": key, "name": name});
        v.as_object_mut().unwrap().extend(extra.as_object().cloned().unwrap_or_default());
        v
    };
    let host = found("host-1", "esx-host-1", json!({"type": "Server", "serial_number": "SN-H1", "location": "DC1", "details": {"os": "ESXi 8"}}));
    let vm = found("vm-7", "db-server-1", json!({"serial_number": "SN-7", "details": {"os": "Debian 13", "ip": "10.0.0.7"}, "runs_on": "host-1"}));
    let toaster = found("t-1", "toaster", json!({"type": "Toaster"}));
    let first = report(&k, tenant, Some(&agent), json!({"source": "scanner-dc1", "items": [host, vm, toaster]})).await.expect("taken in");
    assert_eq!((&first["created"], &first["updated"]), (&json!(1), &json!(1)), "{first}");
    assert!(first["refused"][0]["key"] == "t-1" && first["refused"][0]["error"].as_str().unwrap().contains("not one of"), "{first}");
    // An item people keep, matched by its name: what the source says is added, what people keep stays.
    let d = t.get("ConfigurationItem", db_server).await;
    assert_eq!(
        (d["discovered_by"].as_str(), d["discovery_key"].as_str(), d["serial_number"].as_str(), d["discovered"]["os"].as_str(), d["criticality"].as_str()),
        (Some("scanner-dc1"), Some("vm-7"), Some("SN-7"), Some("Debian 13"), Some("High"))
    );
    let esx = uid(&t
        .k
        .list(
            &t.admin,
            "ConfigurationItem",
            &ListQuery { filters: ferrum_kernel::query::parse_filters(&json!({"ci_name": "esx-host-1"})).unwrap(), ..Default::default() },
        )
        .await
        .unwrap()
        .rows[0]["id"]);
    let kinds: Vec<(String, String)> =
        d["relations"].as_array().unwrap().iter().map(|r| (r["kind"].as_str().unwrap().to_string(), r["target"].as_str().unwrap().to_string())).collect();
    assert!(
        kinds.contains(&("Runs On".into(), esx.to_string())) && kinds.contains(&("Connects To".into(), payroll.to_string())),
        "the source's relation added to people's: {kinds:?}"
    );
    assert_eq!(t.get("ConfigurationItem", esx).await["status"], "Live", "found, so it runs");
    // Reported again under its key: matched by it, its name people's; a relation that loops is refused.
    let again = report(
        &k,
        tenant,
        Some(&agent),
        json!({"source": "scanner-dc1", "items": [
        found("vm-7", "db-renamed", json!({"details": {"os": "Debian 13.1"}})), found("host-1", "esx-host-1", json!({"runs_on": "vm-7"}))]}),
    )
    .await
    .unwrap();
    assert_eq!((&again["created"], &again["updated"]), (&json!(0), &json!(2)), "{again}");
    assert!(again["refused"][0]["error"].as_str().unwrap().contains("come back round"), "{again}");
    let d = t.get("ConfigurationItem", db_server).await;
    assert_eq!((d["ci_name"].as_str(), d["discovered"]["os"].as_str()), (Some("db-server-1"), Some("Debian 13.1")));
    // One source's item is not another's; reports come from agents and the configuration's managers.
    let other = report(&k, tenant, Some(&agent), json!({"source": "cloud-inventory", "items": [found("i-123", "db-server-1", json!({}))]})).await.unwrap();
    assert!(other["refused"][0]["error"].as_str().unwrap().contains("reported by scanner-dc1"), "{other}");
    refused(report(&k, tenant, Some(&req), json!({"source": "s", "items": [found("k", "n", json!({}))]})).await, "forbidden", "Discovery Agent");
    refused(report(&k, tenant, Some(&agent), json!({"source": "s", "items": []})).await, "invalid", "1 to 500");
    assert!(matches!(report(&k, tenant, None, json!({})).await, Err(Error::Unauthorized(_))));
    // A week unreported: marked, its owner told once; reported again, cleared.
    t.set(&cfg, "ConfigurationItem", db_server, json!({"owner": "cmdb@acme.test"})).await.expect("its owner");
    ferrum_app_itsm::maintenance_at(&k, Utc::now() + Duration::days(8)).await.unwrap();
    ferrum_app_itsm::maintenance_at(&k, Utc::now() + Duration::days(9)).await.unwrap();
    let today = Utc::now().date_naive().to_string();
    assert_eq!(t.get("ConfigurationItem", db_server).await["missing_since"].as_str(), Some(today.as_str()));
    assert_eq!(t.get("ConfigurationItem", esx).await["status"], "Live", "never retired by itself");
    let mut tx = k.db.tenant_tx(tenant).await.unwrap();
    let told: i64 = sqlx::query_scalar("select count(*) from notification where tenant_id = $1 and record_id = $2 and kind = 'itsm.missing'")
        .bind(tenant)
        .bind(db_server)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(told, 1, "its owner, once");
    report(&k, tenant, Some(&agent), json!({"source": "scanner-dc1", "items": [found("vm-7", "db-server-1", json!({}))]})).await.unwrap();
    assert!(t.get("ConfigurationItem", db_server).await["missing_since"].is_null(), "seen again");
}
