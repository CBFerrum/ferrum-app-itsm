//! The service desk's people and records, against a real PostgreSQL (`FERRUM_ITSM_TEST_DATABASE_URL`,
//! default `postgres://ferrum:ferrum@localhost:5432/ferrum_itsm_test`; reset first): who an incident
//! is about and who works it, categories, the priority matrix kept as records, work notes and
//! comments, targets on the platform's clock, the employee portal, catalogue forms, change boards,
//! and the reports. The older scenarios are in tests/itsm_it.rs.

use chrono::{DateTime, Duration, Utc};
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
    /// A portal person holding `roles`: an employee, or someone outside, who signs in by a mailed code.
    async fn portal(&self, email: &str, roles: &[&str]) -> Ctx {
        let roles: Vec<String> = roles.iter().map(|r| r.to_string()).collect();
        let id = self.k.create_user(&self.admin, email, email, None, &roles, false).await.expect("user");
        self.k.set_user_kind(&self.admin, id, "portal").await.expect("a portal person");
        let version = self.k.token_version(self.tenant, id).await.expect("token version");
        self.k.load_ctx(self.tenant, id, Some(version), "it".into()).await.expect("ctx")
    }
    /// The id of the record whose `field` holds `value`.
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
    let ann = t.portal("ann@acme.test", &["IT Self Service"]).await;
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

    // ───────────── 3. targets on the platform's clock ─────────────
    let when = |d: &Doc, f: &str| d[f].as_str().map(|v| DateTime::parse_from_rfc3339(v).unwrap().with_timezone(&Utc));
    let p1 = t.insert(&desk, "Incident", json!({"title": "Everything is down", "impact": "High", "urgency": "High"})).await;
    let p1_id = uid(&p1["id"]);
    let started = Utc::now();
    let near = |a: DateTime<Utc>, b: DateTime<Utc>| (a - b).abs() < Duration::minutes(2);
    assert!(near(when(&p1, "respond_by").unwrap(), started + Duration::minutes(15)), "P1 response: {p1:?}");
    assert!(near(when(&p1, "resolve_by").unwrap(), started + Duration::hours(4)), "P1 resolution: {p1:?}");
    assert_eq!((p1["respond_state"].as_str(), p1["resolve_state"].as_str()), (Some("running"), Some("running")));
    let calendars: i64 = {
        let mut tx = k.db.tenant_tx(tenant).await.unwrap();
        let n = sqlx::query_scalar("select count(*) from logic_calendar where tenant_id = $1 and name = 'itsm_hours'")
            .bind(tenant)
            .fetch_one(&mut *tx)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        n
    };
    assert_eq!(calendars, 1, "the working calendar the targets count on is made once, and is the workspace's to edit");
    // Taken up: the response target is met; waiting on the caller stops the resolution clock, and resuming restarts it.
    t.set(&desk, "Incident", p1_id, json!({"status": "In Progress"})).await.unwrap();
    assert_eq!(t.get("Incident", p1_id).await["respond_state"], "met");
    t.set(&desk, "Incident", p1_id, json!({"status": "Awaiting Caller"})).await.unwrap();
    let waiting = t.get("Incident", p1_id).await;
    assert_eq!((waiting["resolve_state"].as_str(), waiting["resolve_by"].is_null()), (Some("paused"), true), "paused while waiting on the caller");
    t.set(&desk, "Incident", p1_id, json!({"status": "In Progress"})).await.unwrap();
    assert_eq!(t.get("Incident", p1_id).await["resolve_state"], "running");
    // The breach is recorded once, by the platform.
    let later = Utc::now() + Duration::hours(6);
    assert!(k.sweep_targets_at(later).await.unwrap() >= 1, "the P1 open past its four hours, and the unanswered ones past theirs");
    assert_eq!(k.sweep_targets_at(later).await.unwrap(), 0, "once");
    assert_eq!(t.get("Incident", p1_id).await["resolve_state"], "breached");
    t.set(&desk, "Incident", p1_id, json!({"status": "Resolved", "resolution": "Rebooted"})).await.unwrap();
    assert_eq!(t.get("Incident", p1_id).await["resolve_state"], "met_late");
    // A workspace's own hours: Monday to Friday, 09:00 to 17:00 UTC, and a Friday 16:00 P3 is due Monday.
    let hours = json!([["09:00", "17:00"]]);
    let cal: ferrum_kernel::Calendar =
        serde_json::from_value(json!({"name": "itsm_hours", "zone": "UTC", "weekends": ["sat", "sun"], "holidays": [], "revision": 0,
        "hours": {"mon": hours, "tue": hours, "wed": hours, "thu": hours, "fri": hours}}))
        .unwrap();
    t.k.set_logic_calendar(&t.admin, &cal).await.expect("an administrator edits the calendar");
    t.k.pin_clock(Some("2026-11-13T16:00:00Z".parse().unwrap()));
    let p3 = t.insert(&desk, "Incident", json!({"title": "Slow wifi", "impact": "High", "urgency": "Low"})).await;
    t.k.pin_clock(None);
    assert_eq!(when(&p3, "respond_by").unwrap().to_rfc3339(), "2026-11-16T12:00:00+00:00", "one working hour on Friday, three on Monday");
    assert_eq!(
        when(&p3, "resolve_by").unwrap().to_rfc3339(),
        "2026-11-18T16:00:00+00:00",
        "24 working hours: one on Friday, eight each on Monday and Tuesday, seven on Wednesday"
    );
    // A request has its offering's targets.
    let offer = uid(&t.insert(&cfg, "ServiceOffering", json!({"offering_name": "VPN access", "response_hours": 2, "fulfil_hours": 3})).await["id"]);
    t.k.pin_clock(Some("2026-11-16T10:00:00Z".parse().unwrap()));
    let ask = t.insert(&desk, "CatalogRequest", json!({"offering": offer.to_string()})).await;
    t.k.pin_clock(None);
    assert_eq!(
        (when(&ask, "respond_by").unwrap().to_rfc3339(), when(&ask, "due_at").unwrap().to_rfc3339()),
        ("2026-11-16T12:00:00+00:00".to_string(), "2026-11-16T13:00:00+00:00".to_string())
    );
    assert_eq!((ask["respond_state"].as_str(), ask["fulfil_state"].as_str()), (Some("running"), Some("running")));
    // Incidents open when this release came keep the clock they had.
    let mut tx = k.db.tenant_tx(tenant).await.unwrap();
    sqlx::query("select set_config('ferrum.system', 'on', true)").execute(&mut *tx).await.unwrap();
    sqlx::raw_sql("alter table e_incident add column breached boolean").execute(&mut *tx).await.unwrap();
    let old: Uuid = Uuid::now_v7();
    sqlx::query(
        "insert into e_incident (tenant_id, id, title, impact, urgency, status, priority, opened_at, resolve_by, breached) \
         values ($1, $2, 'Left from before', 'High', 'High', 'In Progress', 'P1', now() - interval '5 hours', now() - interval '1 hour', true)",
    )
    .bind(tenant)
    .bind(old)
    .execute(&mut *tx)
    .await
    .unwrap();
    sqlx::raw_sql(include_str!("../packages/itsm/sql/001_targets.sql")).execute(&mut *tx).await.unwrap();
    let kept: (String, Option<DateTime<Utc>>, i64) = sqlx::query_as(
        "select i.resolve_state, t.breached_at, t.within_secs from e_incident i join record_target t on t.record_id = i.id and t.target = 'resolve' where i.id = $1",
    )
    .bind(old)
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    tx.rollback().await.unwrap();
    assert_eq!((kept.0.as_str(), kept.1.is_some(), kept.2), ("breached", true, 4 * 3600), "an open, late P1 stays breached on its own four hours");

    // ───────────── 4. employee self-service ─────────────
    // She reports an incident (herself the caller), and cannot report one for someone else nor work it.
    let mine = t.insert(&ann, "Incident", json!({"title": "Cannot print", "impact": "Low", "urgency": "Medium"})).await;
    let mine_id = uid(&mine["id"]);
    assert_eq!((mine["caller_email"].as_str(), mine["priority"].as_str()), (Some("ann@acme.test"), Some("P4")));
    refused(t.k.insert(&ann, "Incident", obj(json!({"title": "For Eve", "caller_email": "eve@acme.test"}))).await, "forbidden", "");
    refused(t.set(&ann, "Incident", mine_id, json!({"status": "Closed"})).await, "forbidden", "");
    refused(t.k.get(&ann, "Incident", other_id, false).await, "not found", "");
    refused(t.k.insert(&ann, "ItGroup", obj(json!({"group_name": "Mine"}))).await, "forbidden", "");
    // She asks for something from the catalog, and follows it.
    let offerings = t.k.list(&ann, "ServiceOffering", &Default::default()).await.unwrap().rows;
    assert!(offerings.iter().any(|o| o["offering_name"] == "VPN access"), "the catalog is hers to read");
    let wanted = t.insert(&ann, "CatalogRequest", json!({"offering": offer.to_string()})).await;
    assert_eq!(wanted["caller_email"], "ann@acme.test");
    let followed = t.k.list(&ann, "CatalogRequest", &Default::default()).await.unwrap().rows;
    assert_eq!(followed.len(), 1, "only her own request");
    // Resolved, she confirms it, or finds it is not.
    t.set(&desk, "Incident", mine_id, json!({"status": "Resolved", "resolution": "Reconnected the printer"})).await.unwrap();
    t.act(&ann, "Incident", mine_id, "confirm_resolution", Value::Null).await.expect("the caller confirms");
    assert_eq!(t.get("Incident", mine_id).await["status"], "Closed");
    let again = uid(&t.insert(&ann, "Incident", json!({"title": "Still cannot print", "impact": "Low", "urgency": "Medium"})).await["id"]);
    t.set(&desk, "Incident", again, json!({"status": "Resolved", "resolution": "Rebooted"})).await.unwrap();
    refused(t.act(&ann, "Incident", other_id, "reject_resolution", json!({"note": "x"})).await, "not found", "");
    refused(t.act(&ann, "Incident", again, "reject_resolution", Value::Null).await, "invalid", "note");
    let kk = k.clone();
    let long_ago = |days: i64| {
        let kk = kk.clone();
        async move {
            let mut tx = kk.db.tenant_tx(tenant).await.unwrap();
            sqlx::query("select set_config('ferrum.system', 'on', true)").execute(&mut *tx).await.unwrap();
            sqlx::query("update e_incident set resolved_at = now() - make_interval(days => $2) where id = $1")
                .bind(again)
                .bind(days as i32)
                .execute(&mut *tx)
                .await
                .unwrap();
            tx.commit().await.unwrap();
        }
    };
    long_ago(9).await;
    refused(t.act(&ann, "Incident", again, "reject_resolution", json!({"note": "It broke again"})).await, "forbidden", "too long ago");
    long_ago(2).await;
    t.act(&ann, "Incident", again, "reject_resolution", json!({"note": "It broke again"})).await.expect("within the days it may be reopened");
    let reopened = t.get("Incident", again).await;
    assert_eq!((reopened["status"].as_str(), reopened["resolved_at"].is_null()), (Some("In Progress"), true));
    let thread = t.k.list(&ann, "ItNote", &Default::default()).await.unwrap().rows;
    assert!(
        thread.iter().any(|n| n["body"].as_str().is_some_and(|b| b.contains("It broke again")) && n["visibility"] == "Visible to Caller"),
        "her reason is in the thread"
    );

    // ───────────── 5. catalogue items with their own form ─────────────
    let laptop_item = uid(&t
        .insert(
            &cfg,
            "ServiceOffering",
            json!({"offering_name": "New laptop", "fulfilment_group": group.to_string(), "questions": [
                {"label": "Model", "qtype": "Choice", "required": true, "options": "T14, X1"},
                {"label": "Needs a dock", "qtype": "Yes/No"},
                {"label": "Dock type", "qtype": "Choice", "options": "USB-C, Thunderbolt", "only_when": "Needs a dock", "only_when_answer": "Yes", "required": true},
                {"label": "Needed by", "qtype": "Date"},
                {"label": "Monitors", "qtype": "Number"}]}),
        )
        .await["id"]);
    refused(
        t.k.insert(&cfg, "ServiceOffering", obj(json!({"offering_name": "Bad", "questions": [{"label": "A", "only_when": "B", "only_when_answer": "x"}]})))
            .await,
        "invalid",
        "earlier question",
    );
    refused(
        t.k.insert(
            &cfg,
            "ServiceOffering",
            obj(json!({"offering_name": "Bad2", "questions": [{"label": "A", "qtype": "Choice", "options": "x"}, {"label": "a"}]})),
        )
        .await,
        "invalid",
        "same label",
    );
    let order = |answers: Value| json!({"offering": laptop_item.to_string(), "answers": answers});
    let ok = t.insert(&ann, "CatalogRequest", order(json!([{"question": "Model", "answer": "T14"}, {"question": "Needs a dock", "answer": "Yes"}, {"question": "Dock type", "answer": "USB-C"}, {"question": "Monitors", "answer": "2"}]))).await;
    assert_eq!(ok["assignment_group"], group.to_string(), "to the item's fulfilment group");
    assert_eq!(ok["answers"][0]["question"], "Model", "shown with their labels");
    refused(t.k.insert(&ann, "CatalogRequest", obj(order(json!([])))).await, "invalid", "Model needs an answer");
    refused(
        t.k.insert(&ann, "CatalogRequest", obj(order(json!([{"question": "Model", "answer": "T14"}, {"question": "Needs a dock", "answer": "Yes"}])))).await,
        "invalid",
        "Dock type needs an answer",
    );
    refused(t.k.insert(&ann, "CatalogRequest", obj(order(json!([{"question": "Model", "answer": "Z9"}])))).await, "invalid", "Model is one of T14, X1");
    refused(
        t.k.insert(&ann, "CatalogRequest", obj(order(json!([{"question": "Model", "answer": "X1"}, {"question": "Monitors", "answer": "two"}])))).await,
        "invalid",
        "a number",
    );
    refused(
        t.k.insert(&ann, "CatalogRequest", obj(order(json!([{"question": "Model", "answer": "X1"}, {"question": "Needed by", "answer": "soon"}])))).await,
        "invalid",
        "a date",
    );
    refused(
        t.k.insert(&ann, "CatalogRequest", obj(order(json!([{"question": "Model", "answer": "X1"}, {"question": "Dock type", "answer": "USB-C"}])))).await,
        "invalid",
        "only when Needs a dock is Yes",
    );
    refused(
        t.k.insert(&ann, "CatalogRequest", obj(order(json!([{"question": "Model", "answer": "X1"}, {"question": "Colour", "answer": "red"}])))).await,
        "invalid",
        "not a question of this offering",
    );
    // What requests carried before the offerings had questions stays with them, as a note.
    let mut tx = k.db.tenant_tx(tenant).await.unwrap();
    sqlx::query("select set_config('ferrum.system', 'on', true)").execute(&mut *tx).await.unwrap();
    sqlx::raw_sql("alter table e_catalog_request add column details text").execute(&mut *tx).await.unwrap();
    sqlx::query("update e_catalog_request set details = 'A lighter one' where id = $1").bind(uid(&ok["id"])).execute(&mut *tx).await.unwrap();
    sqlx::raw_sql(include_str!("../packages/itsm/sql/002_request_details.sql")).execute(&mut *tx).await.unwrap();
    let kept: (String, String) =
        sqlx::query_as("select body, visibility from e_it_note where request = $1").bind(uid(&ok["id"])).fetch_one(&mut *tx).await.unwrap();
    tx.rollback().await.unwrap();
    assert_eq!((kept.0.as_str(), kept.1.as_str()), ("A lighter one", "Internal"));

    // ───────────── 8. notifications ─────────────
    t.user("net@acme.test", &["Service Desk"]).await;
    ferrum_app_itsm::maintenance(&k).await.expect("upkeep makes the notifications the workspace starts with");
    ferrum_app_itsm::maintenance(&k).await.expect("and only once");
    let count = |sql: &'static str| {
        let kk = k.clone();
        async move {
            let mut tx = kk.db.tenant_tx(tenant).await.unwrap();
            let n: i64 = sqlx::query_scalar(sql).bind(tenant).fetch_one(&mut *tx).await.unwrap();
            tx.commit().await.unwrap();
            n
        }
    };
    assert_eq!(count("select count(*) from automation where tenant_id = $1 and name like 'IT Services:%'").await, 7);
    t.k.dispatch_outbox_once().await.ok();
    let inc = t
        .insert(
            &desk,
            "Incident",
            json!({"title": "Mouse is dead", "caller_email": "ann@acme.test", "assigned_to": "net@acme.test", "assignment_group": group.to_string()}),
        )
        .await;
    let inc_id = uid(&inc["id"]);
    while k.dispatch_outbox_once().await.unwrap_or(0) > 0 {}
    let told = |kind: &'static str| {
        let kk = k.clone();
        async move {
            let mut tx = kk.db.tenant_tx(tenant).await.unwrap();
            let n: i64 = sqlx::query_scalar("select count(*) from notification where tenant_id = $1 and record_id = $2 and title like $3")
                .bind(tenant)
                .bind(inc_id)
                .bind(format!("{kind}%"))
                .fetch_one(&mut *tx)
                .await
                .unwrap();
            tx.commit().await.unwrap();
            n
        }
    };
    assert_eq!(told("We received your incident").await, 1, "the caller is told it was received");
    assert_eq!(
        count("select count(*) from outbox where tenant_id = $1 and topic = 'ferrum.mail' and payload->>'subject' like '%received: Mouse is dead'").await,
        1,
        "and by mail"
    );
    t.insert(&desk, "ItNote", json!({"incident": inc_id.to_string(), "visibility": "Visible to Caller", "body": "Try another port"})).await;
    t.insert(&desk, "ItNote", json!({"incident": inc_id.to_string(), "body": "Probably the cable"})).await;
    t.set(&desk, "Incident", inc_id, json!({"status": "Resolved", "resolution": "New mouse"})).await.unwrap();
    while k.dispatch_outbox_once().await.unwrap_or(0) > 0 {}
    assert_eq!(
        count("select count(*) from notification where tenant_id = $1 and title like 'New comment from IT: Try another port'").await,
        1,
        "a visible comment, not a work note"
    );

    assert_eq!(told("Incident INC").await, 2, "the assignee is told, and the caller that it is resolved");

    // ───────────── 7. reports ─────────────
    for source in ferrum_app_itsm::reports() {
        ferrum_domain::reports::add_source(source);
    }
    let report = |name: &'static str, ctx: Ctx, extra: &[(&str, &str)]| {
        let kk = k.clone();
        let params: std::collections::HashMap<String, String> = extra.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect();
        async move { ferrum_domain::reports::run(&kk, &ctx, name, &params).await.unwrap_or_else(|e| panic!("{name}: {e:?}")) }
    };
    let cell = |r: &Value, group: &str, field: &str| {
        r["rows"].as_array().unwrap().iter().find(|x| x["group"] == group).map(|x| x[field].clone()).unwrap_or(Value::Null)
    };
    let open = report("itsm_open_incidents", desk.clone(), &[]).await;
    let open_now =
        t.k.list(&t.admin, "Incident", &Default::default())
            .await
            .unwrap()
            .rows
            .iter()
            .filter(|r| ["New", "In Progress", "On Hold", "Awaiting Caller"].contains(&r["status"].as_str().unwrap()))
            .count();
    assert_eq!(open["totals"]["incidents"], json!(open_now), "every open incident, once");
    assert!(cell(&open, "P3", "incidents").as_i64().unwrap() >= 1);
    let by_age = report("itsm_open_incidents", desk.clone(), &[("group_by", "Age")]).await;
    assert_eq!(cell(&by_age, "under a day", "incidents"), json!(open_now), "all opened today");
    let comp = report("itsm_target_compliance", desk.clone(), &[]).await;
    assert!(cell(&comp, "P1", "resolution_breached").as_i64().unwrap() >= 1, "the P1 resolved late is a miss: {comp}");
    assert!(!cell(&comp, "P1", "resolution_pct").is_null());
    let mttr = report("itsm_time_to_resolve", desk.clone(), &[("group_by", "Month")]).await;
    assert!(mttr["rows"][0]["resolved"].as_i64().unwrap() >= 1 && !mttr["rows"][0]["mean"].is_null(), "{mttr}");
    let asked = report("itsm_requests_by_item", desk.clone(), &[]).await;
    assert_eq!(cell(&asked, "VPN access", "requests"), json!(2), "Ann's and the desk's");
    assert_eq!(cell(&asked, "New laptop", "requests"), json!(1));
    let ci = uid(&t.insert(&cfg, "ConfigurationItem", json!({"ci_name": "Edge router", "ci_type": "Network Device"})).await["id"]);
    let ci2 = uid(&t.insert(&cfg, "ConfigurationItem", json!({"ci_name": "Core switch", "ci_type": "Network Device"})).await["id"]);
    for (item, day, ends) in [(ci, "2026-11-02", true), (ci2, "2026-11-03", false)] {
        let c = uid(&t.insert(&desk, "ItChange", json!({"title": "Firmware", "ci": item.to_string(), "kind": "Standard", "planned_start": format!("{day}T02:00:00Z"), "planned_end": format!("{day}T03:00:00Z")})).await["id"]);
        t.act(&desk, "ItChange", c, "submit_change", Value::Null).await.unwrap();
        t.act(&desk, "ItChange", c, "implement", Value::Null).await.unwrap();
        if ends {
            t.act(&desk, "ItChange", c, "close_change", Value::Null).await.unwrap();
        } else {
            t.act(&desk, "ItChange", c, "fail_change", json!({"note": "Backed out"})).await.unwrap();
        }
    }
    let changes = report("itsm_change_success", desk.clone(), &[]).await;
    assert_eq!((cell(&changes, "Standard", "successful"), cell(&changes, "Standard", "failed")), (json!(1), json!(1)));
    // Reports carry the caller's row scope: Ann counts her own incidents only.
    let hers = report("itsm_open_incidents", ann.clone(), &[]).await;
    assert_eq!(hers["totals"]["incidents"], json!(2), "her two open incidents, nobody else's");

    // ───────────── 9. a workspace upgraded from 0.2.x ─────────────
    // 0.2.x kept the matrix in code: its workspace has no matrix rows, categories, calendar or settings, and its package
    // stamped as it is still (the package's own version stayed 0.1.0). The platform seeds rows only when the package is installed (an administrator's update), so the
    // upkeep, which runs for every workspace with the package, must make what this release needs.
    let old = k
        .create_tenant(NewTenant {
            slug: "olddesk".into(),
            name: "Old IT".into(),
            admin_email: "admin@old.test".into(),
            admin_password: "Correct-Horse-9".into(),
            admin_name: None,
            packages: vec!["itsm".into()],
            settings: Some(json!({"timezone": "UTC"})),
        })
        .await
        .expect("tenant");
    let session = k.login("olddesk", "admin@old.test", "Correct-Horse-9").await.expect("login");
    let o = T { k: k.clone(), tenant: old, admin: k.load_ctx(old, session.user, Some(session.token_version), "it".into()).await.expect("ctx") };
    let sql = |q: &'static str| {
        let kk = k.clone();
        async move {
            let mut tx = kk.db.tenant_tx(old).await.unwrap();
            sqlx::query("select set_config('ferrum.system', 'on', true)").execute(&mut *tx).await.unwrap();
            let n: Option<i64> = sqlx::query_scalar(q).bind(old).fetch_optional(&mut *tx).await.unwrap().flatten();
            tx.commit().await.unwrap();
            n.unwrap_or(0)
        }
    };
    for q in [
        "update e_it_category set parent = null where tenant_id = $1 and $1 is not null returning 1::bigint",
        "delete from e_it_category where tenant_id = $1 returning 1::bigint",
        "delete from e_priority_matrix where tenant_id = $1 returning 1::bigint",
        "delete from e_itsm_settings where tenant_id = $1 returning 1::bigint",
        "delete from automation where tenant_id = $1 returning 1::bigint",
        "delete from logic_calendar where tenant_id = $1 returning 1::bigint",
        "update installed_package set version = '0.1.0' where tenant_id = $1 and name = 'itsm' returning 1::bigint",
    ] {
        sql(q).await;
    }
    assert_eq!(sql("select count(*) from e_priority_matrix where tenant_id = $1").await, 0, "the 0.2.x workspace has no matrix");
    // The workspace's own High / High row, made before the upkeep, is its own.
    o.insert(&o.admin, "PriorityMatrix", json!({"impact": "High", "urgency": "High", "priority": "P2", "response_time": "PT30M", "resolution_time": "PT2H"}))
        .await;
    // With the matrix incomplete an incident is still saved: P3 and its four and 24 hours, and it says the matrix had no row.
    let near = |a: &Doc, f: &str, hours: i64| {
        let at = DateTime::parse_from_rfc3339(a[f].as_str().unwrap()).unwrap().with_timezone(&Utc);
        (at - Utc::now() - Duration::hours(hours)).abs() < Duration::minutes(2)
    };
    let gap = o.insert(&o.admin, "Incident", json!({"title": "No row for me", "impact": "High", "urgency": "Medium"})).await;
    assert_eq!((gap["priority"].as_str(), gap["matrix_missing"].as_bool(), gap["matrix"].is_null()), (Some("P3"), Some(true), true), "{gap:?}");
    assert!(near(&gap, "respond_by", 4) && near(&gap, "resolve_by", 24), "the fallback's targets: {gap:?}");
    o.set(&o.admin, "Incident", uid(&gap["id"]), json!({"status": "Resolved", "resolution": "Fixed"})).await.expect("resolved");
    assert_eq!(o.get("Incident", uid(&gap["id"])).await["resolve_state"], "met");
    let mine = o.insert(&o.admin, "Incident", json!({"title": "Own row", "impact": "High", "urgency": "High"})).await;
    assert_eq!((mine["priority"].as_str(), mine["matrix_missing"].as_bool()), (Some("P2"), Some(false)), "the row it has is used");
    // The upkeep, with no administrator, makes the other eight rows, the categories, the calendar and the notices.
    ferrum_app_itsm::maintenance(&k).await.expect("upkeep");
    assert_eq!(sql("select count(*) from e_priority_matrix where tenant_id = $1").await, 9);
    let kept = o.get("PriorityMatrix", o.named("PriorityMatrix", "matrix_name", "High / High").await).await;
    assert_eq!((kept["priority"].as_str(), kept["response_time"].as_str()), (Some("P2"), Some("PT30M")), "a row the workspace changed is not overwritten");
    assert_eq!(sql("select count(*) from e_it_category where tenant_id = $1").await, 12);
    assert_eq!(sql("select count(*) from e_it_category c join e_it_category p on p.id = c.parent where c.tenant_id = $1 and c.category_name = 'VPN' and p.category_name = 'Network'").await, 1);
    assert_eq!(sql("select count(*) from logic_calendar where tenant_id = $1 and name = 'itsm_hours'").await, 1);
    assert_eq!(sql("select count(*) from automation where tenant_id = $1 and name like 'IT Services:%'").await, 7);
    // A row it deletes afterwards stays deleted, and a second upkeep changes nothing.
    sql("delete from e_priority_matrix where tenant_id = $1 and impact = 'Low' and urgency = 'Low' returning 1::bigint").await;
    ferrum_app_itsm::maintenance(&k).await.expect("upkeep again");
    assert_eq!(sql("select count(*) from e_priority_matrix where tenant_id = $1").await, 8);
    let after = o.insert(&o.admin, "Incident", json!({"title": "Now a row", "impact": "High", "urgency": "Medium"})).await;
    assert_eq!((after["priority"].as_str(), after["matrix_missing"].as_bool()), (Some("P2"), Some(false)));
    o.set(&o.admin, "Incident", uid(&after["id"]), json!({"status": "Resolved", "resolution": "Done"})).await.expect("resolved");
    // The administrator's own update (the platform's install) adds what is missing and overwrites nothing.
    o.set(&o.admin, "PriorityMatrix", o.named("PriorityMatrix", "matrix_name", "Low / Medium").await, json!({"priority": "P1"})).await.unwrap();
    k.install_packages(old, &["itsm".to_string()]).await.expect("update");
    assert_eq!(sql("select count(*) from e_priority_matrix where tenant_id = $1").await, 9, "the deleted row is back, as an install does");
    assert_eq!(o.get("PriorityMatrix", o.named("PriorityMatrix", "matrix_name", "Low / Medium").await).await["priority"], "P1");
}
