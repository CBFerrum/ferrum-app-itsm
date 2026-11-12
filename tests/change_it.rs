//! Change enablement, against a real PostgreSQL (`FERRUM_ITSM_CHANGE_TEST_DATABASE_URL`, default
//! `postgres://ferrum:ferrum@localhost:5432/ferrum_itsm_change_test`; reset first): the change board's approval
//! (N of M, a high-risk second look, standard from an approved template, emergency and its review), the questionnaire,
//! the calendar and freezes, and a workspace upgraded from 0.3 that keeps what it ran.

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
fn change_enablement() {
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().thread_stack_size(16 << 20).build().unwrap();
    rt.block_on(async { tokio::spawn(scenario()).await.unwrap() });
}

async fn boot() -> Arc<Kernel> {
    let url =
        std::env::var("FERRUM_ITSM_CHANGE_TEST_DATABASE_URL").unwrap_or_else(|_| "postgres://ferrum:ferrum@localhost:5432/ferrum_itsm_change_test".into());
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

fn change(title: &str, ci: Uuid, ty: Uuid, start: &str, end: &str) -> Value {
    json!({"title": title, "ci": ci.to_string(), "change_type": ty.to_string(), "planned_start": start, "planned_end": end, "backout_plan": "Restore the snapshot"})
}

async fn scenario() {
    let k = boot().await;

    // ───────────── a new workspace starts on the change board ─────────────
    let t = T::new(&k, "newco").await;
    let desk = t.user("desk@acme.test", &["Service Desk"]).await;
    let mgr = t.user("mgr@acme.test", &["Change Manager"]).await;
    let cfg = t.user("cmdb@acme.test", &["Configuration Manager"]).await;
    let (c1, c2, c3) =
        (t.user("cab1@acme.test", &["Requester"]).await, t.user("cab2@acme.test", &["Requester"]).await, t.user("cab3@acme.test", &["Requester"]).await);
    let exec = t.user("exec@acme.test", &["Requester"]).await;
    let emg = t.user("emg@acme.test", &["Requester"]).await;
    let outsider = t.user("other@acme.test", &["Requester"]).await;
    ferrum_app_itsm::maintenance(&k).await.unwrap();
    let st = t.get("ItsmSettings", t.settings().await).await;
    assert_eq!((st["change_approval"].as_str(), st["problem_closes_incidents"].as_str()), (Some("Change Board"), Some("Offer")));
    let standard = t.named("ChangeType", "type_name", "Standard").await;
    let normal = t.named("ChangeType", "type_name", "Normal").await;
    let emergency = t.named("ChangeType", "type_name", "Emergency").await;
    let cab = t.named("ItGroup", "group_name", "Change Board").await;
    let emerg_board = t.named("ItGroup", "group_name", "Emergency Change Board").await;
    let db = uid(&t.insert(&cfg, "ConfigurationItem", json!({"ci_name": "Orders DB", "ci_type": "Database"})).await["id"]);
    let shop = uid(&t.insert(&cfg, "ConfigurationItem", json!({"ci_name": "Shop", "ci_type": "Business Service"})).await["id"]);

    // A board with no members cannot be asked.
    let c = uid(&t.insert(&desk, "ItChange", change("Index", db, normal, "2026-11-20T10:00:00Z", "2026-11-20T12:00:00Z")).await["id"]);
    refused(t.act(&desk, "ItChange", c, "submit_change", Value::Null).await, "invalid", "has 0 members");
    t.set(&cfg, "ItGroup", cab, json!({"members": [{"member": "CAB1@acme.test"}, {"member": "cab2@acme.test"}, {"member": "cab3@acme.test"}]})).await.unwrap();
    t.set(&cfg, "ItGroup", emerg_board, json!({"members": [{"member": "emg@acme.test"}]})).await.unwrap();
    let execs = uid(&t.insert(&cfg, "ItGroup", json!({"group_name": "Executives", "members": [{"member": "exec@acme.test"}]})).await["id"]);
    // The members are taken when the change is raised: saving the draft again takes the group as it now is.
    t.set(&desk, "ItChange", c, json!({"title": "Index"})).await.unwrap();

    // ───────────── the questionnaire sets risk and impact ─────────────
    let answers = json!([
        {"question": "Has this change been done before?", "answer": "Never"},
        {"question": "Can it be backed out?", "answer": "Not or only partly"},
        {"question": "Is it tested outside production?", "answer": "no"},
        {"question": "Who is affected if it goes wrong?", "answer": "A team or department"}]);
    let d = t.set(&desk, "ItChange", c, json!({"risk_answers": answers})).await.unwrap();
    assert_eq!(
        (d["risk"].as_str(), d["risk_score"].as_i64(), d["impact_level"].as_str(), d["impact_score"].as_i64()),
        (Some("High"), Some(12), Some("Medium"), Some(4))
    );
    assert_eq!(d["risk_answers"][2]["score"], 4, "an answer is matched without regard to case and scored");
    refused(
        t.set(&desk, "ItChange", c, json!({"risk_answers": [{"question": "Has this change been done before?", "answer": "Sometimes"}]})).await,
        "invalid",
        "is not an answer",
    );
    // The type's high-risk group makes a High-risk change ask one of them as well; here the type has none, so a score is only a score.
    assert_eq!((d["route"].as_str(), d["quorum"].as_i64(), d["second_look"].as_bool()), (Some("Board"), Some(2), Some(false)));
    // A type of the workspace's own, with its own board, its own N and a second look.
    let net = uid(&t
        .insert(
            &mgr,
            "ChangeType",
            json!({"type_name": "Network", "path": "Board", "board": cab.to_string(), "quorum": 2, "high_risk_group": execs.to_string()}),
        )
        .await["id"]);
    refused(
        t.k.insert(&mgr, "ChangeType", obj(json!({"type_name": "Too many", "path": "Board", "board": cab.to_string(), "quorum": 6}))).await,
        "invalid",
        "quorum",
    );
    let n = t
        .insert(
            &desk,
            "ItChange",
            json!({"title": "Core switch", "ci": db.to_string(), "change_type": net.to_string(), "planned_start": "2026-11-21T10:00:00Z",
        "planned_end": "2026-11-21T12:00:00Z", "backout_plan": "Roll back", "risk_answers": answers, "affected": [{"item": shop.to_string()}]}),
        )
        .await;
    let n_id = uid(&n["id"]);
    assert_eq!((n["kind"].as_str(), n["route"].as_str(), n["second_look"].as_bool()), (Some("Normal"), Some("Board"), Some(true)));
    t.act(&desk, "ItChange", n_id, "submit_change", Value::Null).await.unwrap();
    let asked = t.act(&desk, "ItChange", n_id, "approve_change", Value::Null).await.unwrap();
    assert!(asked["_approval"].is_object(), "pressing Approve asks the board: {asked}");
    assert_eq!(t.get("ItChange", n_id).await["status"], "Submitted");
    refused(t.decide(&outsider, "ItChange", n_id, true).await, "forbidden", "");
    refused(t.decide(&desk, "ItChange", n_id, true).await, "forbidden", "");
    t.decide(&c1, "ItChange", n_id, true).await.unwrap();
    assert!(matches!(t.decide(&c1, "ItChange", n_id, true).await, Err(Error::Conflict(_))));
    assert_eq!(t.get("ItChange", n_id).await["status"], "Submitted", "one of two is not enough, and one person twice is one");
    t.decide(&c3, "ItChange", n_id, true).await.unwrap();
    assert_eq!(t.get("ItChange", n_id).await["status"], "Submitted", "the high-risk group is asked as well");
    refused(t.decide(&c2, "ItChange", n_id, true).await, "forbidden", "");
    t.decide(&exec, "ItChange", n_id, true).await.unwrap();
    let done = t.get("ItChange", n_id).await;
    assert_eq!(done["status"], "Approved");
    let note = done["decision_note"].as_str().unwrap();
    assert!(note.contains("cab1@acme.test") && note.contains("cab3@acme.test") && note.contains("exec@acme.test"), "{note}");

    // Stand-ins: a member who is away is stood in for by the substitute (the platform's own, at its defaults).
    let m = t.insert(&desk, "ItChange", change("Patch", db, normal, "2026-11-25T10:00:00Z", "2026-11-25T11:00:00Z")).await;
    let m_id = uid(&m["id"]);
    t.act(&desk, "ItChange", m_id, "submit_change", Value::Null).await.unwrap();
    t.act(&desk, "ItChange", m_id, "approve_change", Value::Null).await.unwrap();
    t.decide(&c1, "ItChange", m_id, false).await.expect("a rejection by a member is the platform's");
    // The request ended rejected; the change stays Submitted and a Change Manager may still reject it in the app's own words.
    t.act(&mgr, "ItChange", m_id, "reject_change", json!({"note": "Not now"})).await.unwrap();
    assert_eq!(t.get("ItChange", m_id).await["status"], "Rejected");

    // ───────────── a standard change from an approved template goes straight through ─────────────
    let tpl = uid(&t
        .insert(
            &mgr,
            "ChangeTemplate",
            json!({"template_name": "OS patch", "change_type": standard.to_string(), "ci": db.to_string(),
        "change_plan": "Apply the patch", "backout_plan": "Uninstall it", "test_plan": "Smoke test"}),
        )
        .await["id"]);
    refused(
        t.act(
            &desk,
            "ChangeTemplate",
            tpl,
            "raise_change",
            json!({"title": "x", "planned_start": "2026-12-01T01:00:00Z", "planned_end": "2026-12-01T02:00:00Z"}),
        )
        .await,
        "invalid",
        "not available",
    );
    refused(t.act(&desk, "ChangeTemplate", tpl, "approve_template", Value::Null).await, "forbidden", "not permitted");
    t.act(&mgr, "ChangeTemplate", tpl, "approve_template", Value::Null).await.unwrap();
    t.act(
        &desk,
        "ChangeTemplate",
        tpl,
        "raise_change",
        json!({"title": "Patch Orders DB", "planned_start": "2026-12-01T01:00:00Z", "planned_end": "2026-12-01T02:00:00Z"}),
    )
    .await
    .unwrap();
    let p_id = t.named("ItChange", "title", "Patch Orders DB").await;
    let p = t.get("ItChange", p_id).await;
    assert_eq!(
        (p["route"].as_str(), p["test_plan"].as_str(), p["template"].as_str().map(str::to_string)),
        (Some("Pre-approved"), Some("Smoke test"), Some(tpl.to_string()))
    );
    t.act(&desk, "ItChange", p_id, "submit_change", Value::Null).await.unwrap();
    assert_eq!(t.get("ItChange", p_id).await["status"], "Approved", "no one decides it");
    // Without an approved template a standard change goes to the board its type names.
    let q = uid(&t
        .insert(
            &desk,
            "ItChange",
            json!({"title": "Loose patch", "ci": db.to_string(), "change_type": standard.to_string(),
        "planned_start": "2026-12-02T01:00:00Z", "planned_end": "2026-12-02T02:00:00Z"}),
        )
        .await["id"]);
    assert_eq!(t.get("ItChange", q).await["route"], "Board");

    // ───────────── an emergency change: the smaller group, then a review ─────────────
    let e = uid(&t.insert(&desk, "ItChange", change("Hotfix", db, emergency, "2026-12-22T10:00:00Z", "2026-12-22T11:00:00Z")).await["id"]);
    // A freeze refuses a normal change and lets an emergency through.
    t.insert(&mgr, "ChangeFreeze", json!({"reason": "Year end", "starts_at": "2026-12-20T00:00:00Z", "ends_at": "2027-01-05T00:00:00Z"})).await;
    refused(
        t.k.insert(&desk, "ItChange", obj(change("Late", db, normal, "2026-12-22T12:00:00Z", "2026-12-22T13:00:00Z"))).await,
        "invalid",
        "falls in the freeze for Year end",
    );
    t.act(&desk, "ItChange", e, "submit_change", Value::Null).await.unwrap();
    t.act(&desk, "ItChange", e, "approve_change", Value::Null).await.unwrap();
    refused(t.decide(&c1, "ItChange", e, true).await, "forbidden", "");
    t.decide(&emg, "ItChange", e, true).await.unwrap();
    assert_eq!(t.get("ItChange", e).await["status"], "Approved", "one member of the emergency group is enough");
    t.act(&desk, "ItChange", e, "implement", Value::Null).await.unwrap();
    refused(t.act(&desk, "ItChange", e, "close_change", Value::Null).await, "invalid", "reviewed before it is closed");
    refused(t.act(&desk, "ItChange", e, "review_change", json!({"outcome": "Failed"})).await, "invalid", "says what happened");
    let r = t.act(&desk, "ItChange", e, "review_change", json!({"outcome": "Failed", "note": "Locked the table", "open_incident": true})).await.unwrap();
    let after = t.get("ItChange", e).await;
    assert_eq!(
        (after["status"].as_str(), after["review_outcome"].as_str(), after["reviewed_by"].as_str()),
        (Some("Failed"), Some("Failed"), Some("desk@acme.test"))
    );
    let inc = t.get("Incident", uid(&r["incident"])).await;
    assert_eq!((inc["change"].as_str().map(str::to_string), inc["ci"].as_str().map(str::to_string)), (Some(e.to_string()), Some(db.to_string())));
    // A reviewed change that went well closes.
    let ok = uid(&t.insert(&desk, "ItChange", change("Hotfix 2", shop, emergency, "2026-12-23T10:00:00Z", "2026-12-23T11:00:00Z")).await["id"]);
    t.act(&desk, "ItChange", ok, "submit_change", Value::Null).await.unwrap();
    t.act(&desk, "ItChange", ok, "approve_change", Value::Null).await.unwrap();
    t.decide(&emg, "ItChange", ok, true).await.unwrap();
    t.act(&desk, "ItChange", ok, "implement", Value::Null).await.unwrap();
    t.act(&desk, "ItChange", ok, "review_change", json!({"outcome": "Successful with Issues", "note": "Slow at first"})).await.unwrap();
    assert_eq!(t.get("ItChange", ok).await["status"], "Closed");

    // ───────────── the calendar: collisions flagged, freezes refuse or warn per the setting ─────────────
    let a = t
        .insert(
            &desk,
            "ItChange",
            json!({"title": "Shop release", "ci": db.to_string(), "change_type": normal.to_string(), "planned_start": "2026-11-30T10:00:00Z",
        "planned_end": "2026-11-30T12:00:00Z", "backout_plan": "x", "affected": [{"item": shop.to_string()}]}),
        )
        .await;
    let a_id = uid(&a["id"]);
    t.act(&desk, "ItChange", a_id, "submit_change", Value::Null).await.unwrap();
    let b = t
        .insert(
            &desk,
            "ItChange",
            json!({"title": "Shop banner", "ci": shop.to_string(), "change_type": normal.to_string(), "planned_start": "2026-11-30T11:00:00Z",
        "planned_end": "2026-11-30T13:00:00Z", "backout_plan": "x"}),
        )
        .await;
    let flagged = b["window_flags"].as_str().unwrap();
    assert!(flagged.contains(a["number"].as_str().unwrap()), "{flagged}");
    let site = k.hooks.site("itsm").cloned().unwrap();
    let get = |q: Value| SiteRequest {
        tenant: t.tenant,
        user: Some(desk.clone()),
        method: "GET".into(),
        path: "/calendar".into(),
        query: q.as_object().cloned().unwrap(),
        headers: Default::default(),
        body: vec![],
        client_ip: "10.0.0.2".into(),
    };
    t.act(&desk, "ItChange", uid(&b["id"]), "submit_change", Value::Null).await.unwrap();
    let cal: Value = serde_json::from_slice(&site.handle(&k, get(json!({"from_date": "2026-11-30", "to_date": "2026-11-30"}))).await.unwrap().body).unwrap();
    let rows = cal["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 2, "{cal}");
    assert!(rows.iter().all(|r| !r["collides"].as_str().unwrap().is_empty()), "each names the other: {cal}");
    let rep = t.k.hooks.site("itsm").is_some();
    assert!(rep);
    t.set(&t.admin, "ItsmSettings", t.settings().await, json!({"freeze_action": "Warn"})).await.unwrap();
    let w = t.insert(&desk, "ItChange", change("In the freeze", shop, normal, "2026-12-28T10:00:00Z", "2026-12-28T12:00:00Z")).await;
    assert!(w["window_flags"].as_str().unwrap().contains("freeze for Year end"), "warned, not refused: {w:?}");

    // ───────────── a workspace upgraded from 0.3 keeps what it ran ─────────────
    let o = T::new(&k, "oldco").await;
    let odesk = o.user("desk@acme.test", &["Service Desk"]).await;
    let (cm1, cm2) = (o.user("cm1@acme.test", &["Change Manager"]).await, o.user("cm2@acme.test", &["Change Manager"]).await);
    // What 0.3 left: its package at the earlier version, its notifications made, a change with only a class.
    sqlx::query("update installed_package set version = '0.1.0' where tenant_id = $1 and name = 'itsm'").bind(o.tenant).execute(&k.db.pool).await.ok();
    for entity in ["ChangeType", "RiskQuestion", "ItGroup"] {
        for r in o.k.list(&o.admin, entity, &Default::default()).await.unwrap().rows {
            o.k.delete(&o.admin, entity, uid(&r["id"]), None).await.unwrap();
        }
    }
    o.insert(&o.admin, "ItsmSettings", json!({"defaults_made": true})).await;
    let item = uid(&o.insert(&o.admin, "ConfigurationItem", json!({"ci_name": "Mail", "ci_type": "Application"})).await["id"]);
    let old = uid(&o
        .insert(
            &odesk,
            "ItChange",
            json!({"title": "Old change", "ci": item.to_string(), "kind": "Normal", "planned_start": "2026-11-20T10:00:00Z",
        "planned_end": "2026-11-20T12:00:00Z", "backout_plan": "x"}),
        )
        .await["id"]);
    assert!(o.get("ItChange", old).await["change_type"].is_null(), "no type before the upkeep");
    ferrum_app_itsm::maintenance(&k).await.unwrap();
    ferrum_app_itsm::maintenance(&k).await.unwrap();
    let st = o.get("ItsmSettings", o.settings().await).await;
    assert!(
        st["change_approval"].is_null() && st["problem_closes_incidents"].is_null(),
        "an upgraded workspace is left on Single Manager and Automatically: {st:?}"
    );
    assert_eq!(st["board_defaults_made"], true);
    let normal_o = o.named("ChangeType", "type_name", "Normal").await;
    assert_eq!(o.get("ItChange", old).await["change_type"], normal_o.to_string(), "its change has its type");
    o.act(&odesk, "ItChange", old, "submit_change", Value::Null).await.unwrap();
    assert_eq!(o.get("ItChange", old).await["route"], "Manager");
    let asked = o.act(&odesk, "ItChange", old, "approve_change", Value::Null).await;
    refused(asked, "forbidden", "a Change Manager decides");
    o.act(&cm1, "ItChange", old, "approve_change", Value::Null).await.expect("one Change Manager decides, no board asked");
    assert_eq!(o.get("ItChange", old).await["status"], "Approved");
    let _ = cm2;
    // The administrator turns the board on; a change made then asks it.
    o.set(&o.admin, "ItsmSettings", o.settings().await, json!({"change_approval": "Change Board"})).await.unwrap();
    let fresh = o
        .insert(
            &odesk,
            "ItChange",
            json!({"title": "New one", "ci": item.to_string(), "kind": "Normal", "planned_start": "2026-11-22T10:00:00Z",
        "planned_end": "2026-11-22T12:00:00Z", "backout_plan": "x"}),
        )
        .await;
    assert_eq!(fresh["route"], "Board");

    // A workspace on 0.2: no settings record at all and a change already there is upgraded too, not new.
    let z = T::new(&k, "zeroco").await;
    let zd = z.user("desk@acme.test", &["Service Desk"]).await;
    let zi = uid(&z.insert(&z.admin, "ConfigurationItem", json!({"ci_name": "Wiki", "ci_type": "Application"})).await["id"]);
    z.insert(
        &zd,
        "ItChange",
        json!({"title": "Earlier", "ci": zi.to_string(), "kind": "Standard", "planned_start": "2026-11-20T10:00:00Z", "planned_end": "2026-11-20T12:00:00Z"}),
    )
    .await;
    ferrum_app_itsm::maintenance(&k).await.unwrap();
    assert!(z.get("ItsmSettings", z.settings().await).await["change_approval"].is_null(), "a workspace with changes is not new");
}
