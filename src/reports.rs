//! IT Services' reports, read from the records and the platform's service-target states they carry
//! (no second bookkeeping): open incidents by priority, group and age; target compliance; mean time to
//! resolve; requests by catalogue item and state; change success by type. Every query carries the
//! caller's row scope on the entity it reads.

use async_trait::async_trait;
use chrono::NaiveDate;
use ferrum_domain::reports::{Params, ReportCompany, ReportSource, col, param_def, period, read_tx};
use ferrum_domain::util::money;
use ferrum_kernel::{Action, Ctx, Error, Kernel, Result, Tx, row_scope};
use rust_decimal::Decimal;
use serde_json::{Map, Value, json};
use sqlx::{Postgres, QueryBuilder, Row};
use std::collections::HashMap;
use uuid::Uuid;

pub struct ItsmReports;

/// Incident groupings: label, SQL over the incident `a` and its group `g`.
const GROUPS: [(&str, &str); 5] = [
    ("Priority", "coalesce(a.priority, 'None')"),
    ("Assignment Group", "coalesce(g.group_name, 'Unassigned')"),
    ("Category", "coalesce((select category_name from e_it_category c where c.tenant_id = a.tenant_id and c.id = a.category), 'None')"),
    ("Assignee", "coalesce(a.assigned_to, 'Unassigned')"),
    ("Month", "to_char(a.opened_at, 'YYYY-MM')"),
];

const AGE: &str = "case when now() - a.opened_at < interval '1 day' then '1 under a day' when now() - a.opened_at < interval '3 days' then '2 one to three days' \
                   when now() - a.opened_at < interval '7 days' then '3 three to seven days' else '4 over a week' end";

fn grouping(p: &Params<'_>, default: &str) -> Result<(&'static str, &'static str)> {
    let want = p.str("group_by").unwrap_or(default);
    GROUPS
        .iter()
        .copied()
        .find(|(label, _)| label.eq_ignore_ascii_case(want))
        .ok_or_else(|| Error::field("group_by", format!("use one of: {}", GROUPS.map(|(l, _)| l).join(", "))))
}

fn group_param(default: &str) -> Value {
    json!({"name": "group_by", "label": "Group By", "type": "select", "options": GROUPS.map(|(l, _)| l), "default": default})
}

fn span(ctx: &Ctx, p: &Params<'_>) -> Result<(NaiveDate, NaiveDate)> {
    period(p, &ReportCompany { id: Uuid::nil(), name: String::new(), currency: String::new(), fy_month: ctx.fy_start_month, precision: 2 })
}

fn within(qb: &mut QueryBuilder<Postgres>, column: &str, (from, to): (NaiveDate, NaiveDate)) {
    qb.push(format!(" and {column} >= ")).push_bind(from).push(format!(" and {column} < ")).push_bind(to).push(" + 1");
}

/// `select <columns> from <table> a ...` with the caller's row scope on entity `entity`.
fn scoped(ctx: &Ctx, entity: &str, columns: &str, from: &str) -> Result<QueryBuilder<Postgres>> {
    let ent = ctx.schema.entity(entity).ok_or_else(|| Error::NotFound(entity.into()))?;
    let scope = row_scope(ctx, ent, Action::Read)?;
    let mut qb = QueryBuilder::new(format!("select {columns} from {from} where a.tenant_id = "));
    qb.push_bind(ctx.tenant);
    scope.push_sql(&mut qb, "a");
    Ok(qb)
}

/// The rows a query gives as a report: `cols` says each column's name, label and kind (`data`, `int`, `decimal`); the
/// totals line adds up the whole numbers. The first column's labels may begin with a sort digit and a space, dropped.
async fn table(tx: &mut Tx, qb: QueryBuilder<Postgres>, cols: &[(&str, &str, &str)], meta: Value) -> Result<Value> {
    let mut qb = qb;
    let data = qb.build().fetch_all(&mut **tx).await?;
    let mut totals = Map::new();
    let rows: Vec<Value> = data
        .iter()
        .map(|r| {
            let mut row = Map::new();
            for (i, (name, _, ty)) in cols.iter().enumerate() {
                let v = match *ty {
                    "int" => {
                        let n: i64 = r.try_get(i).unwrap_or(0);
                        let t = totals.entry(name.to_string()).or_insert(json!(0));
                        *t = json!(t.as_i64().unwrap_or(0) + n);
                        json!(n)
                    }
                    "decimal" => r.try_get::<Option<Decimal>, _>(i).ok().flatten().map_or(Value::Null, money),
                    _ => {
                        let text: String = r.try_get(i).unwrap_or_default();
                        json!(if i == 0 && text.chars().next().is_some_and(|c| c.is_ascii_digit()) && text.chars().nth(1) == Some(' ') {
                            text[2..].to_string()
                        } else {
                            text
                        })
                    }
                };
                row.insert(name.to_string(), v);
            }
            Value::Object(row)
        })
        .collect();
    Ok(json!({"columns": cols.iter().map(|(n, l, t)| col(n, l, t)).collect::<Vec<_>>(), "rows": rows, "totals": totals, "meta": meta}))
}

/// Percent met of those met or missed, as a decimal column from two count expressions.
fn pct(met: &str, missed: &str) -> String {
    format!("round(100.0 * ({met})::numeric / nullif(({met}) + ({missed}), 0), 2)")
}

async fn open_incidents(ctx: &Ctx, p: &Params<'_>, tx: &mut Tx) -> Result<Value> {
    let by = p.str("group_by").unwrap_or("Priority");
    let (label, expr) = if by.eq_ignore_ascii_case("Age") { ("Age", AGE) } else { grouping(p, "Priority")? };
    let mut qb = scoped(
        ctx,
        "Incident",
        &format!(
            "{expr}, count(*)::bigint, count(*) filter (where a.resolve_state = 'breached')::bigint, \
             round(avg(extract(epoch from now() - a.opened_at) / 3600)::numeric, 1)"
        ),
        "e_incident a left join e_it_group g on g.tenant_id = a.tenant_id and g.id = a.assignment_group",
    )?;
    qb.push(" and a.status in ('New', 'In Progress', 'On Hold', 'Awaiting Caller') group by 1 order by 1");
    table(
        tx,
        qb,
        &[
            ("group", label, "data"),
            ("incidents", "Open Incidents", "int"),
            ("breached", "Past Their Resolution Target", "int"),
            ("age_hours", "Average Age (h)", "decimal"),
        ],
        json!({"group_by": label}),
    )
    .await
}

async fn compliance(ctx: &Ctx, p: &Params<'_>, tx: &mut Tx) -> Result<Value> {
    let span = span(ctx, p)?;
    let (label, expr) = grouping(p, "Priority")?;
    let n = |f: &str, states: &str| format!("count(*) filter (where a.{f} in ({states}))::bigint");
    let (rm, rb, zm, zb) =
        (n("respond_state", "'met'"), n("respond_state", "'met_late', 'breached'"), n("resolve_state", "'met'"), n("resolve_state", "'met_late', 'breached'"));
    let mut qb = scoped(
        ctx,
        "Incident",
        &format!("{expr}, {rm}, {rb}, {}, {zm}, {zb}, {}", pct(&rm, &rb), pct(&zm, &zb)),
        "e_incident a left join e_it_group g on g.tenant_id = a.tenant_id and g.id = a.assignment_group",
    )?;
    within(&mut qb, "a.opened_at", span);
    qb.push(" group by 1 order by 1");
    table(
        tx,
        qb,
        &[
            ("group", label, "data"),
            ("response_met", "Response Met", "int"),
            ("response_breached", "Response Missed", "int"),
            ("response_pct", "Response Compliance %", "decimal"),
            ("resolution_met", "Resolution Met", "int"),
            ("resolution_breached", "Resolution Missed", "int"),
            ("resolution_pct", "Resolution Compliance %", "decimal"),
        ],
        json!({"from_date": span.0, "to_date": span.1, "group_by": label, "note": "met: inside the target; missed: breached, then met late or still open"}),
    )
    .await
}

async fn time_to_resolve(ctx: &Ctx, p: &Params<'_>, tx: &mut Tx) -> Result<Value> {
    let span = span(ctx, p)?;
    let (label, expr) = grouping(p, "Priority")?;
    let hours = "extract(epoch from a.resolved_at - a.opened_at) / 3600";
    let mut qb = scoped(
        ctx,
        "Incident",
        &format!(
            "{expr}, count(*)::bigint, round(avg({hours})::numeric, 2), round((percentile_cont(0.5) within group (order by {hours}))::numeric, 2), \
             round((percentile_cont(0.9) within group (order by {hours}))::numeric, 2)"
        ),
        "e_incident a left join e_it_group g on g.tenant_id = a.tenant_id and g.id = a.assignment_group",
    )?;
    within(&mut qb, "a.resolved_at", span);
    qb.push(" and a.resolved_at is not null and a.opened_at is not null group by 1 order by 1");
    table(
        tx,
        qb,
        &[
            ("group", label, "data"),
            ("resolved", "Resolved", "int"),
            ("mean", "Mean Time to Resolve (h)", "decimal"),
            ("median", "Median (h)", "decimal"),
            ("p90", "90th Percentile (h)", "decimal"),
        ],
        json!({"from_date": span.0, "to_date": span.1, "group_by": label, "unit": "elapsed hours from opened to resolved"}),
    )
    .await
}

async fn requests(ctx: &Ctx, p: &Params<'_>, tx: &mut Tx) -> Result<Value> {
    let span = span(ctx, p)?;
    let n = |s: &str| format!("count(*) filter (where a.status = '{s}')::bigint");
    let mut qb = scoped(
        ctx,
        "CatalogRequest",
        &format!(
            "coalesce(o.offering_name, 'None'), count(*)::bigint, {}, {}, {}, {}, {}, {}",
            n("Submitted"),
            n("Approved"),
            n("Awaiting Caller"),
            n("Fulfilled"),
            n("Rejected"),
            n("Cancelled")
        ),
        "e_catalog_request a left join e_service_offering o on o.tenant_id = a.tenant_id and o.id = a.offering",
    )?;
    within(&mut qb, "a.created_at::date", span);
    qb.push(" group by 1 order by 2 desc, 1");
    table(
        tx,
        qb,
        &[
            ("group", "Catalogue Item", "data"),
            ("requests", "Requests", "int"),
            ("submitted", "Submitted", "int"),
            ("approved", "Approved", "int"),
            ("awaiting", "Awaiting Caller", "int"),
            ("fulfilled", "Fulfilled", "int"),
            ("rejected", "Rejected", "int"),
            ("cancelled", "Cancelled", "int"),
        ],
        json!({"from_date": span.0, "to_date": span.1}),
    )
    .await
}

async fn change_success(ctx: &Ctx, p: &Params<'_>, tx: &mut Tx) -> Result<Value> {
    let span = span(ctx, p)?;
    let (done, bad) = ("count(*) filter (where a.status in ('Implemented', 'Closed'))", "count(*) filter (where a.status = 'Failed')");
    let mut qb = scoped(ctx, "ItChange", &format!("a.kind, {done}::bigint, {bad}::bigint, {}", pct(done, bad)), "e_it_change a")?;
    within(&mut qb, "a.planned_start::date", span);
    qb.push(" group by 1 order by 1");
    table(
        tx,
        qb,
        &[("group", "Type", "data"), ("successful", "Successful", "int"), ("failed", "Failed or Backed Out", "int"), ("success_pct", "Success %", "decimal")],
        json!({"from_date": span.0, "to_date": span.1, "note": "a change that failed is one backed out: there is no separate rolled-back state"}),
    )
    .await
}

/// Scheduled changes (submitted, approved or being implemented) by window, each with the items it touches, the other
/// scheduled changes on one of them in the same window, and the freeze it falls in.
pub(crate) async fn change_calendar(ctx: &Ctx, p: &Params<'_>, tx: &mut Tx) -> Result<Value> {
    let span = span(ctx, p)?;
    const MINE: &str = "(select x.item from e_it_change__affected x where x.tenant_id = a.tenant_id and x.parent_id = a.id)";
    let mut qb = scoped(
        ctx,
        "ItChange",
        &format!(
            "to_char(a.planned_start at time zone 'UTC', 'YYYY-MM-DD HH24:MI') || ' to ' || to_char(a.planned_end at time zone 'UTC', 'YYYY-MM-DD HH24:MI'),              a.number, a.title, coalesce(t.type_name, a.kind), a.status,              coalesce((select string_agg(i.ci_name, ', ' order by i.ci_name) from e_configuration_item i where i.tenant_id = a.tenant_id and (i.id = a.ci or i.id in {MINE})), ''),              coalesce((select string_agg(o.number, ', ' order by o.number) from e_it_change o where o.tenant_id = a.tenant_id and o.id <> a.id and o.status in ('Submitted', 'Approved')                and o.planned_start < a.planned_end and o.planned_end > a.planned_start and (o.ci = a.ci or o.ci in {MINE}                or exists (select 1 from e_it_change__affected y where y.tenant_id = o.tenant_id and y.parent_id = o.id and (y.item = a.ci or y.item in {MINE})))), ''),              coalesce((select string_agg(f.reason, ', ') from e_change_freeze f where f.tenant_id = a.tenant_id and f.starts_at < a.planned_end and f.ends_at > a.planned_start), '')"
        ),
        "e_it_change a left join e_change_type t on t.tenant_id = a.tenant_id and t.id = a.change_type",
    )?;
    qb.push(" and a.status in ('Submitted', 'Approved', 'Implemented') and a.planned_start < ")
        .push_bind(span.1)
        .push(" + 1 and a.planned_end >= ")
        .push_bind(span.0);
    qb.push(" order by a.planned_start, a.number");
    table(
        tx,
        qb,
        &[
            ("window", "Window (UTC)", "data"),
            ("number", "Change", "data"),
            ("title", "Title", "data"),
            ("type", "Type", "data"),
            ("status", "Status", "data"),
            ("items", "Items and Services", "data"),
            ("collides", "Collides With", "data"),
            ("freeze", "In Freeze For", "data"),
        ],
        json!({"from_date": span.0, "to_date": span.1, "note": "collides: another submitted or approved change on one of the same items or services in an overlapping window"}),
    )
    .await
}

#[async_trait]
impl ReportSource for ItsmReports {
    fn catalog(&self) -> Vec<Value> {
        let entry = |name: &str, label: &str, entity: &str, params: Vec<Value>| json!({"name": name, "label": label, "module": "itsm", "entity": entity, "params": params});
        let dates = || vec![param_def("from_date"), param_def("to_date")];
        let open_group = json!({"name": "group_by", "label": "Group By", "type": "select", "options": ["Priority", "Assignment Group", "Category", "Assignee", "Age"], "default": "Priority"});
        vec![
            entry("itsm_open_incidents", "Open Incidents", "Incident", vec![open_group]),
            entry("itsm_target_compliance", "Target Compliance", "Incident", [dates(), vec![group_param("Priority")]].concat()),
            entry("itsm_time_to_resolve", "Mean Time to Resolve", "Incident", [dates(), vec![group_param("Priority")]].concat()),
            entry("itsm_requests_by_item", "Requests by Catalogue Item", "CatalogRequest", dates()),
            entry("itsm_change_success", "Change Success", "ItChange", dates()),
            entry("itsm_change_calendar", "Change Calendar", "ItChange", dates()),
        ]
    }

    async fn run(&self, k: &Kernel, ctx: &Ctx, name: &str, params: &HashMap<String, String>) -> Result<Value> {
        let p = Params { raw: params };
        let mut tx = read_tx(k, ctx).await?;
        let out = match name {
            "itsm_open_incidents" => open_incidents(ctx, &p, &mut tx).await,
            "itsm_target_compliance" => compliance(ctx, &p, &mut tx).await,
            "itsm_time_to_resolve" => time_to_resolve(ctx, &p, &mut tx).await,
            "itsm_requests_by_item" => requests(ctx, &p, &mut tx).await,
            "itsm_change_success" => change_success(ctx, &p, &mut tx).await,
            "itsm_change_calendar" => change_calendar(ctx, &p, &mut tx).await,
            _ => Err(Error::NotFound(format!("report {name}"))),
        };
        tx.rollback().await?;
        out
    }
}
