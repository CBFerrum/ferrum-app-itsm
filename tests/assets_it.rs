//! IT Services' connector with Assets (`itsm_assets`) against a real PostgreSQL
//! (`FERRUM_ITSM_ASSETS_TEST_DATABASE_URL`, default
//! `postgres://ferrum:ferrum@localhost:5432/ferrum_itsm_assets_test`; reset first): a configuration
//! item finds the fixed asset it is by its serial number, or names it; shows who holds it, where,
//! and its warranty, kept as Assets changes them; and an asset is one item's.

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
    admin: Ctx,
}

impl T {
    async fn insert(&self, ctx: &Ctx, entity: &str, v: Value) -> Doc {
        self.k.insert(ctx, entity, obj(v)).await.unwrap_or_else(|e| panic!("insert {entity}: {e:?}"))
    }
    async fn get(&self, entity: &str, id: Uuid) -> Doc {
        self.k.get(&self.admin, entity, id, false).await.unwrap_or_else(|e| panic!("get {entity}: {e:?}"))
    }
    async fn find_one(&self, entity: &str, filters: Value) -> Doc {
        let q = ListQuery { filters: ferrum_kernel::query::parse_filters(&filters).unwrap(), ..Default::default() };
        let rows = self.k.list(&self.admin, entity, &q).await.unwrap().rows;
        assert_eq!(rows.len(), 1, "{entity} {filters}");
        rows[0].clone()
    }
}

#[test]
fn items_and_the_assets_they_are() {
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().thread_stack_size(16 << 20).build().unwrap();
    rt.block_on(async { tokio::spawn(scenario()).await.unwrap() });
}

async fn scenario() {
    let url =
        std::env::var("FERRUM_ITSM_ASSETS_TEST_DATABASE_URL").unwrap_or_else(|_| "postgres://ferrum:ferrum@localhost:5432/ferrum_itsm_assets_test".into());
    let db = Db::connect(DbConfig { url, max_connections: 16, ..Default::default() }).await.expect("connect");
    sqlx::raw_sql("drop schema if exists public cascade; create schema public;").execute(&db.pool).await.expect("reset");
    let mut packages = ferrum_domain::packages().expect("platform packages");
    packages.extend(ferrum_ops::packages().expect("Operations' packages"));
    packages.extend(ferrum_app_erp::packages().expect("ERP's packages"));
    packages.extend(ferrum_app_assets::packages().expect("Assets' packages"));
    packages.extend(ferrum_app_itsm::packages().expect("app packages"));
    let catalog = Catalog::from_packages(packages).expect("catalog");
    assert_eq!(catalog.links_out_of_reach(ferrum_app_itsm::CONNECTOR_PACKAGE), Vec::<String>::new(), "whoever links an asset can look it up");
    let mut hooks = HookRegistry::default();
    ferrum_domain::register(&mut hooks);
    ferrum_app_erp::register(&mut hooks);
    ferrum_app_assets::register(&mut hooks);
    ferrum_app_itsm::register(&mut hooks);
    let settings = KernelSettings { secret_key: Some(SecretKey::new([7u8; 32])), ..Default::default() };
    let k = Kernel::boot(db, catalog, hooks, settings, None).await.expect("boot");
    ferrum_domain::maintenance(&k).await.expect("partitions");
    ferrum_app_erp::maintenance(&k).await.expect("partitions");
    let tenant = k
        .create_tenant(NewTenant {
            slug: "itassets".into(),
            name: "Acme IT".into(),
            admin_email: "admin@acme.test".into(),
            admin_password: "Correct-Horse-9".into(),
            admin_name: None,
            packages: vec!["itsm".into(), ferrum_app_itsm::CONNECTOR_PACKAGE.into()],
            settings: Some(json!({"timezone": "UTC"})),
        })
        .await
        .expect("tenant");
    let session = k.login("itassets", "admin@acme.test", "Correct-Horse-9").await.expect("login");
    let admin = k.load_ctx(tenant, session.user, Some(session.token_version), "it".into()).await.expect("ctx");
    let t = T { k: k.clone(), admin };
    let a = t.admin.clone();
    let cfg = {
        let user = k.create_user(&a, "cmdb@acme.test", "CMDB", Some("Correct-Horse-9"), &["Configuration Manager".to_string()], false).await.expect("user");
        let s = k.login("itassets", "cmdb@acme.test", "Correct-Horse-9").await.expect("login");
        k.load_ctx(tenant, user, Some(s.token_version), "it".into()).await.expect("ctx")
    };

    // ───────────── a laptop in Assets ─────────────
    let inr = t.find_one("Currency", json!({"code": "INR"})).await;
    let company = uid(&t.insert(&a, "Company", json!({"company_name": "Acme IT", "abbr": "AIT", "default_currency": inr["id"]})).await["id"]);
    let computers = t.find_one("Account", json!({"account_name": "Computers", "company": company.to_string()})).await;
    let category = t
        .insert(
            &a,
            "AssetCategory",
            json!({"category_name": "Laptops", "depreciation_method": "Straight Line", "total_number_of_depreciations": 36, "frequency_of_depreciation": 1,
                                         "accounts": [{"company": company.to_string(), "fixed_asset_account": computers["id"]}]}),
        )
        .await;
    let products = t.find_one("ItemGroup", json!({"group_name": "Products"})).await;
    let nos = t.find_one("Uom", json!({"uom_name": "Nos"})).await;
    let item = t
        .insert(
            &a,
            "Item",
            json!({"item_code": "LAPTOP", "item_name": "laptop", "item_group": products["id"], "stock_uom": nos["id"], "is_stock_item": false,
                                 "fa_is_fixed_asset": true, "fa_asset_category": category["id"]}),
        )
        .await;
    let office = uid(&t.insert(&a, "Location", json!({"location_name": "Office"})).await["id"]);
    let home = uid(&t.insert(&a, "Location", json!({"location_name": "Home"})).await["id"]);
    let laptop = |name: &str, serial: &str| {
        json!({"asset_name": name, "item": item["id"], "company": company.to_string(), "location": office.to_string(), "is_existing_asset": true,
               "calculate_depreciation": false, "purchase_date": "2026-01-10", "gross_purchase_amount": "90000", "serial_number": serial,
               "custodian": "ann@acme.test", "warranty_expiry_date": "2029-01-09"})
    };
    let anns = uid(&t.insert(&a, "Asset", laptop("Ann's laptop", "SN-LAP-1")).await["id"]);
    let spare = uid(&t.insert(&a, "Asset", laptop("Spare laptop", "SN-LAP-2")).await["id"]);

    // ───────────── items and the assets they are ─────────────
    let ci = |name: &str, extra: Value| {
        let mut v = json!({"ci_name": name, "ci_type": "Laptop"});
        v.as_object_mut().unwrap().extend(extra.as_object().cloned().unwrap_or_default());
        v
    };
    let item1 = t.insert(&cfg, "ConfigurationItem", ci("ann-laptop", json!({"serial_number": "SN-LAP-1"}))).await;
    assert_eq!(
        (item1["ita_asset"].as_str(), item1["ita_custodian"].as_str(), item1["ita_location"].as_str(), item1["ita_warranty_ends"].as_str()),
        (Some(anns.to_string().as_str()), Some("ann@acme.test"), Some(office.to_string().as_str()), Some("2029-01-09")),
        "found by its serial number, with who holds it, where, and its warranty"
    );
    let item1 = uid(&item1["id"]);
    let twice = t.k.insert(&cfg, "ConfigurationItem", obj(ci("ann-laptop-2", json!({"ita_asset": anns.to_string()})))).await;
    assert!(matches!(&twice, Err(Error::Validation(e)) if e.iter().any(|f| f.field == "ita_asset" && f.message.contains("ann-laptop"))), "{twice:?}");
    let named = t.insert(&cfg, "ConfigurationItem", ci("spare-laptop", json!({"ita_asset": spare.to_string()}))).await;
    assert_eq!(named["serial_number"], "SN-LAP-2", "named, it takes the asset's serial number");
    let unknown = t.insert(&cfg, "ConfigurationItem", ci("odd-laptop", json!({"serial_number": "SN-NONE"}))).await;
    assert!(unknown["ita_asset"].is_null() && unknown["ita_custodian"].is_null(), "no asset carries that serial number");

    // ───────────── kept as Assets changes them ─────────────
    t.k.update(&a, "Asset", anns, obj(json!({"custodian": "bob@acme.test", "location": home.to_string()})), None).await.expect("moved before it is submitted");
    ferrum_app_itsm::maintenance(&k).await.unwrap();
    let now = t.get("ConfigurationItem", item1).await;
    assert_eq!((now["ita_custodian"].as_str(), now["ita_location"].as_str()), (Some("bob@acme.test"), Some(home.to_string().as_str())));
    // Unlinked, it shows nothing of an asset.
    t.k.update(&cfg, "ConfigurationItem", item1, obj(json!({"ita_asset": null, "serial_number": null})), None).await.expect("unlinked");
    let now = t.get("ConfigurationItem", item1).await;
    assert!(now["ita_custodian"].is_null() && now["ita_warranty_ends"].is_null(), "{now:?}");
}
