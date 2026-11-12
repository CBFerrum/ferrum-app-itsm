-- A request's free-text details are replaced by the answers to its offering's questions. What was written
-- there stays with the request, as an internal note by whoever asked.
select set_config('ferrum.system', 'on', true);

do $$
begin
  if exists (select 1 from information_schema.columns where table_name = 'e_catalog_request' and column_name = 'details') then
    insert into e_it_note (tenant_id, id, request, visibility, body, author, posted_at, caller_email)
    select r.tenant_id, gen_random_uuid(), r.id, 'Internal', r.details, coalesce(r.requested_by, ''), r.created_at, r.caller_email
    from e_catalog_request r
    where r.details is not null and btrim(r.details) <> '';
  end if;
end $$;
