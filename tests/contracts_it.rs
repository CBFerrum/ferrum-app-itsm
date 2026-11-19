//! Every action says what done means, and a caller's data is declared, against a real PostgreSQL
//! (`FERRUM_ITSM_TEST_DATABASE_URL`, default `postgres://ferrum:ferrum@localhost:5432/ferrum_itsm_test`; reset first):
//! each of the 21 actions is a capability with its own completion and result shape, the three that tell a service's
//! subscribers are outside effects, a repeat under one key does it once, a change without a key or without the right
//! is refused, and an erasure blanks a caller and a subscriber and no one else.

use ferrum_kernel::capability::Invocation;
use ferrum_kernel::tenant::NewTenant;
use ferrum_kernel::{Ctx, Db, DbConfig, Doc, HookRegistry, Kernel, KernelSettings};
use ferrum_meta::Catalog;
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;

const IDS: [&str; 21] = [
    "itsm.incident.confirm_resolution",
    "itsm.incident.reject_resolution",
    "itsm.incident.declare_major",
    "itsm.incident.major_update",
    "itsm.incident.end_major",
    "itsm.problem.resolve_linked_incidents",
    "itsm.change_template.approve_template",
    "itsm.change_template.withdraw_template",
    "itsm.change_template.raise_change",
    "itsm.catalog_request.approve_request",
    "itsm.catalog_request.reject_request",
    "itsm.catalog_request.fulfil",
    "itsm.catalog_request.cancel_request",
    "itsm.request_task.complete_task",
    "itsm.it_change.submit_change",
    "itsm.it_change.approve_change",
    "itsm.it_change.reject_change",
    "itsm.it_change.implement",
    "itsm.it_change.close_change",
    "itsm.it_change.fail_change",
    "itsm.it_change.review_change",
];

/// The actions that tell the subscribers of the incident's business service.
const OUTSIDE: [&str; 3] = ["itsm.incident.declare_major", "itsm.incident.major_update", "itsm.incident.end_major"];

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
        let s = self.k.login("itcontracts", email, "Correct-Horse-9").await.expect("login");
        self.k.load_ctx(self.tenant, id, Some(s.token_version), "it".into()).await.expect("ctx")
    }
    /// A request for an offering of one step, made by `desk`: its one task, Open.
    async fn open_task(&self, desk: &Ctx, name: &str) -> Uuid {
        let offer = uid(&self.insert(&self.admin, "ServiceOffering", json!({"offering_name": name, "steps": [{"step_name": "Do it"}]})).await["id"]);
        let req = uid(&self.insert(desk, "CatalogRequest", json!({"offering": offer.to_string()})).await["id"]);
        let mut tx = self.k.db.tenant_tx(self.tenant).await.unwrap();
        let tasks: Vec<Uuid> = sqlx::query_scalar("select id from e_request_task where tenant_id = $1 and request = $2")
            .bind(self.tenant)
            .bind(req)
            .fetch_all(&mut *tx)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        assert_eq!(tasks.len(), 1, "the offering's one step is one task");
        tasks[0]
    }
}

#[test]
fn every_action_states_its_contract() {
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().thread_stack_size(16 << 20).build().unwrap();
    rt.block_on(async { tokio::spawn(scenario()).await.unwrap() });
}

/// What opens the tests' vault: the database password of `url`, with Argon2id kept cheap for tests.
fn opener(url: &str) -> ferrum_kernel::vault::Opener {
    ferrum_kernel::vault::Opener {
        database_url: url.to_string(),
        file: "/nonexistent/root.key".into(),
        kdf: ferrum_kernel::vault::Kdf { m_kib: 64, t: 1, p: 1 },
        ..Default::default()
    }
}

async fn scenario() {
    let url = std::env::var("FERRUM_ITSM_TEST_DATABASE_URL").unwrap_or_else(|_| "postgres://ferrum:ferrum@localhost:5432/ferrum_itsm_test".into());
    let db = Db::connect(DbConfig { url: url.clone(), max_connections: 16, ..Default::default() }).await.expect("connect");
    sqlx::raw_sql("drop schema if exists vault cascade; drop schema if exists public cascade; create schema public;").execute(&db.pool).await.expect("reset");
    ferrum_kernel::vault::open(&db, opener(&url), false).await.expect("the vault");
    let mut packages = ferrum_domain::packages().expect("platform packages");
    packages.extend(ferrum_app_itsm::packages().expect("app packages").into_iter().filter(|p| p.name() != ferrum_app_itsm::CONNECTOR_PACKAGE));
    let catalog = Catalog::from_packages(packages).expect("catalog");
    let mut hooks = HookRegistry::default();
    ferrum_domain::register(&mut hooks);
    ferrum_app_itsm::register(&mut hooks);
    let k = Kernel::boot(db, catalog, hooks, KernelSettings::default(), None).await.expect("boot");
    ferrum_domain::maintenance(&k).await.expect("partitions");
    let tenant = k
        .create_tenant(NewTenant {
            slug: "itcontracts".into(),
            name: "Acme IT".into(),
            admin_email: "admin@acme.test".into(),
            admin_password: "Correct-Horse-9".into(),
            admin_name: None,
            packages: vec!["itsm".into()],
            settings: Some(json!({"timezone": "UTC"})),
        })
        .await
        .expect("tenant");
    let session = k.login("itcontracts", "admin@acme.test", "Correct-Horse-9").await.expect("login");
    let admin = k.load_ctx(tenant, session.user, Some(session.token_version), "it".into()).await.expect("ctx");
    let t = T { k: k.clone(), tenant, admin };
    let desk = t.user("desk@acme.test", &["Service Desk"]).await;

    every_action_says_what_done_means(&t).await;
    major_incident_messages_are_outside_effects(&t).await;
    complete_task_repeats_under_one_key(&t, &desk).await;
    a_change_without_a_key_is_refused(&t, &desk).await;
    someone_without_the_right_is_refused(&t).await;
    a_callers_data_is_found_and_erased(&t, &desk).await;
}

async fn every_action_says_what_done_means(t: &T) {
    for id in IDS {
        let c = t.k.capability(&t.admin, id).unwrap_or_else(|x| panic!("{id}: {x:?}"));
        assert!(!c.completion.ends_with("is done, committed with this receipt") && !c.completion.starts_with("What "), "{id}: {}", c.completion);
        assert!(c.output["type"] == "object" && c.output["properties"].is_object(), "{id}: {}", c.output);
    }
}

async fn major_incident_messages_are_outside_effects(t: &T) {
    for id in IDS {
        let c = t.k.capability(&t.admin, id).unwrap();
        let (effect, unknown) = (serde_json::to_value(c.effect).unwrap(), serde_json::to_value(c.unknown_outcome).unwrap());
        if OUTSIDE.contains(&id) {
            assert_eq!((effect, unknown), (json!("external_effect"), json!("manual")), "{id}");
        } else {
            assert_eq!(effect, json!("transactional_action"), "{id}");
        }
    }
}

async fn complete_task_repeats_under_one_key(t: &T, desk: &Ctx) {
    let task = t.open_task(desk, "Badge").await;
    let call = || Invocation::new("itsm.request_task.complete_task", json!({"id": task})).key("t1");
    let first = t.k.invoke(desk, call()).await.expect("done");
    assert!(!first.repeated);
    let done_at = t.get("RequestTask", task).await["done_at"].clone();
    let second = t.k.invoke(desk, call()).await.expect("answered again");
    assert!(second.repeated, "the second call under the key is the first's answer");
    assert_eq!(second.result, first.result);
    assert_eq!(t.get("RequestTask", task).await["done_at"], done_at, "done once");
}

async fn a_change_without_a_key_is_refused(t: &T, desk: &Ctx) {
    let task = t.open_task(desk, "Desk phone").await;
    let r = t.k.invoke(desk, Invocation::new("itsm.request_task.complete_task", json!({"id": task}))).await;
    assert!(r.is_err(), "{r:?}");
    assert_eq!(t.get("RequestTask", task).await["status"], "Open");
}

async fn someone_without_the_right_is_refused(t: &T) {
    let asker = t.user("asker@acme.test", &["Requester"]).await;
    let ci = uid(&t.insert(&t.admin, "ConfigurationItem", json!({"ci_name": "Mail relay", "ci_type": "Server"})).await["id"]);
    let change = uid(&t
        .insert(
            &t.admin,
            "ItChange",
            json!({"title": "Patch the relay", "ci": ci.to_string(), "kind": "Normal", "planned_start": "2031-03-02T10:00:00Z",
                   "planned_end": "2031-03-02T11:00:00Z", "backout_plan": "Roll back the package"}),
        )
        .await["id"]);
    let r = t.k.invoke(&asker, Invocation::new("itsm.it_change.implement", json!({"id": change})).key("i1")).await;
    assert!(r.is_err(), "{r:?}");
    assert_eq!(t.get("ItChange", change).await["status"], "Draft");
}

/// The fault itsm-1 closes: an erasure found nothing in IT Services. And it reaches no one else.
async fn a_callers_data_is_found_and_erased(t: &T, desk: &Ctx) {
    let svc = uid(&t
        .insert(
            &t.admin,
            "ConfigurationItem",
            json!({"ci_name": "Payroll", "ci_type": "Business Service", "subscribers": [{"subscriber": "eve@x.test"}, {"subscriber": "sam@x.test"}]}),
        )
        .await["id"]);
    let caller = json!({"caller_email": "eve@x.test", "caller_name": "Eve Example", "caller_phone": "+44 20 7946 0000"});
    let mut inc = caller.clone();
    inc["title"] = json!("Payslip missing");
    let inc = uid(&t.insert(desk, "Incident", inc).await["id"]);
    let offer = uid(&t.insert(&t.admin, "ServiceOffering", json!({"offering_name": "Payslip copy"})).await["id"]);
    let mut req = caller.clone();
    req["offering"] = json!(offer.to_string());
    let req = uid(&t.insert(desk, "CatalogRequest", req).await["id"]);
    let other = uid(&t.insert(desk, "Incident", json!({"title": "Laptop slow", "caller_email": "sam@x.test", "caller_name": "Sam"})).await["id"]);

    t.k.erase_personal_data(&t.admin, "eve@x.test", "asked").await.expect("erased");
    for (entity, id) in [("Incident", inc), ("CatalogRequest", req)] {
        let d = t.get(entity, id).await;
        for f in ["caller_email", "caller_name", "caller_phone"] {
            assert!(d[f].is_null() || d[f] == "", "{entity}.{f} is blank: {}", d[f]);
        }
    }
    let subs: Vec<String> =
        t.get("ConfigurationItem", svc).await["subscribers"].as_array().unwrap().iter().map(|r| r["subscriber"].as_str().unwrap().to_string()).collect();
    assert!(!subs.contains(&"eve@x.test".to_string()), "her subscription is gone: {subs:?}");
    an_erasure_leaves_other_callers_alone(t, other, &subs).await;
}

async fn an_erasure_leaves_other_callers_alone(t: &T, other: Uuid, subs: &[String]) {
    let d = t.get("Incident", other).await;
    assert_eq!((d["caller_email"].as_str(), d["caller_name"].as_str()), (Some("sam@x.test"), Some("Sam")));
    assert!(subs.contains(&"sam@x.test".to_string()), "another subscriber stays: {subs:?}");
}
