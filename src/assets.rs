//! The connector with Assets (package `itsm_assets`, where both apps are installed): a
//! configuration item names the fixed asset it is, found by its serial number when none is named,
//! and shows who holds it, where, and until when its warranty runs, kept as they change in Assets.
//! An asset is one item's.

use crate::obj;
use async_trait::async_trait;
use chrono::NaiveDate;
use ferrum_domain::util::{id, s, sys};
use ferrum_kernel::{Ctx, Doc, DocHook, Error, Filter, HookCtx, Kernel, Result, Tx};
use serde_json::{Value, json};
use uuid::Uuid;

pub struct AssetLinkHook;

/// The fields the connector shows of the asset: (the item's, the asset's).
const SHOWN: [(&str, &str); 3] = [("ita_custodian", "custodian"), ("ita_location", "location"), ("ita_warranty_ends", "warranty_expiry_date")];

#[async_trait]
impl DocHook for AssetLinkHook {
    fn name(&self) -> &'static str {
        "itsm_assets.ci"
    }

    fn fills(&self) -> &'static [&'static str] {
        &["ita_asset", "ita_custodian", "ita_location", "ita_warranty_ends", "serial_number"]
    }

    async fn before_validate(&self, hc: &mut HookCtx<'_>, doc: &mut Doc) -> Result<()> {
        let system = sys(hc.ctx);
        // Named by its serial number: the one asset, not cancelled, that carries it.
        if id(doc, "ita_asset").is_none()
            && let Some(serial) = s(doc, "serial_number").map(str::trim).filter(|v| !v.is_empty()).map(str::to_string)
        {
            let found = hc.kernel.find_tx(&system, hc.tx, "Asset", &[Filter::eq("serial_number", serial)], None, 3).await?;
            let live: Vec<&Doc> = found.iter().filter(|a| a.get("docstatus").and_then(Value::as_i64) != Some(2)).collect();
            if let [only] = live.as_slice() {
                doc.insert("ita_asset".into(), json!(s(only, "id")));
            }
        }
        match id(doc, "ita_asset") {
            Some(asset) => {
                let a = hc.kernel.get_tx(&system, hc.tx, "Asset", asset, false).await?;
                for (mine, theirs) in SHOWN {
                    doc.insert(mine.into(), a.get(theirs).cloned().unwrap_or(Value::Null));
                }
                if s(doc, "serial_number").is_none() {
                    doc.insert("serial_number".into(), a.get("serial_number").cloned().unwrap_or(Value::Null));
                }
            }
            None => {
                for (mine, _) in SHOWN {
                    doc.insert(mine.into(), Value::Null);
                }
            }
        }
        Ok(())
    }

    async fn validate(&self, hc: &mut HookCtx<'_>, doc: &Doc) -> Result<()> {
        let Some(asset) = id(doc, "ita_asset") else { return Ok(()) };
        let others = hc.kernel.find_tx(&sys(hc.ctx), hc.tx, "ConfigurationItem", &[Filter::eq("ita_asset", asset.to_string())], None, 2).await?;
        if let Some(other) = others.iter().find(|o| id(o, "id") != id(doc, "id")) {
            return Err(Error::field("ita_asset", format!("is already {}", s(other, "ci_name").unwrap_or("another item"))));
        }
        Ok(())
    }
}

/// Upkeep where the connector is installed: what items show of their asset kept as it is in
/// Assets (custodians and locations change with movements). How many changed.
pub(crate) async fn refresh(k: &Kernel, ctx: &Ctx, tx: &mut Tx) -> Result<usize> {
    let stale: Vec<(Uuid, Option<String>, Option<Uuid>, Option<NaiveDate>)> = sqlx::query_as(
        "select ci.id, a.custodian::text, a.location, a.warranty_expiry_date from e_configuration_item ci \
         join e_asset a on a.tenant_id = ci.tenant_id and a.id = nullif(ci.ext->>'ita_asset', '')::uuid \
         where ci.tenant_id = $1 and ((ci.ext->>'ita_custodian') is distinct from a.custodian::text \
           or nullif(ci.ext->>'ita_location', '')::uuid is distinct from a.location \
           or (ci.ext->>'ita_warranty_ends')::date is distinct from a.warranty_expiry_date) order by ci.id",
    )
    .bind(ctx.tenant)
    .fetch_all(&mut **tx)
    .await?;
    for (ci, custodian, location, warranty) in &stale {
        let v = json!({"ita_custodian": custodian, "ita_location": location.map(|l| l.to_string()), "ita_warranty_ends": warranty.map(|w| w.to_string())});
        k.set_values_tx(ctx, tx, "ConfigurationItem", *ci, obj(v)).await?;
    }
    Ok(stale.len())
}
