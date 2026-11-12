-- Incidents and requests now keep their targets on the platform's clock (record_target). Those still open
-- when this runs keep the clock they had: it started when they were opened, and runs to the moment they
-- were due (the old four, eight, 24 or 72 hours, or the offering's hours), and one already breached stays
-- breached. A response target is not started for them. Nothing here runs on a fresh cluster.
select set_config('ferrum.system', 'on', true);

do $$
begin
  if exists (select 1 from information_schema.columns where table_name = 'e_incident' and column_name = 'breached') then
    insert into record_target (tenant_id, entity, record_id, target, round, started_at, resumed_at, within_secs, due_at, breached_at)
    select i.tenant_id, 'Incident', i.id, 'resolve', 1, coalesce(i.opened_at, i.created_at), coalesce(i.opened_at, i.created_at),
           (case i.priority when 'P1' then 4 when 'P2' then 8 when 'P3' then 24 else 72 end) * 3600, i.resolve_by,
           case when i.breached then now() end
    from e_incident i
    where i.status in ('New', 'In Progress', 'On Hold') and i.resolve_by is not null
    on conflict do nothing;
    update e_incident set resolve_state = case when breached then 'breached' else 'running' end
    where status in ('New', 'In Progress', 'On Hold') and resolve_by is not null and resolve_state is null;
    update e_incident set resolve_state = case when breached then 'met_late' else 'met' end
    where status in ('Resolved', 'Closed') and resolve_state is null;
  end if;
end $$;

insert into record_target (tenant_id, entity, record_id, target, round, started_at, resumed_at, within_secs, due_at)
select r.tenant_id, 'CatalogRequest', r.id, 'fulfil', 1, r.created_at, r.created_at, greatest(extract(epoch from r.due_at - r.created_at)::bigint, 1), r.due_at
from e_catalog_request r
where r.status in ('Submitted', 'Approved') and r.due_at is not null and r.fulfil_state is null
on conflict do nothing;
update e_catalog_request set fulfil_state = 'running' where status in ('Submitted', 'Approved') and due_at is not null and fulfil_state is null;
