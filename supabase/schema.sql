-- Liquor POS cloud mirror. Run this once in your Supabase project: SQL Editor > New query > paste > Run.
--
-- Model: one Supabase Auth user = one store. The till logs in as that user and every row it
-- pushes carries store_id = auth.uid(). Row Level Security keeps each store's data private.
-- Money columns are integer cents (KES). Timestamps are the till's local time (Africa/Nairobi).

create table if not exists public.users (
  uid         text primary key,
  store_id    uuid not null default auth.uid(),
  device_id   text,
  local_id    bigint,
  username    text not null,
  role        text not null,
  active      boolean not null default true,
  created_at  timestamp
);

create table if not exists public.products (
  uid            text primary key,
  store_id       uuid not null default auth.uid(),
  device_id      text,
  local_id       bigint,
  barcode        text,
  name           text not null,
  category       text,
  cost_price     bigint not null default 0,
  sell_price     bigint not null default 0,
  stock_qty      bigint not null default 0,
  reorder_level  bigint not null default 5,
  active         boolean not null default true,
  created_at     timestamp,
  updated_at     timestamp not null default (now() at time zone 'Africa/Nairobi')
);

create table if not exists public.sales (
  uid             text primary key,
  store_id        uuid not null default auth.uid(),
  device_id       text,
  local_id        bigint,
  user_uid        text,
  cashier         text,
  subtotal        bigint not null,
  discount        bigint not null default 0,
  total           bigint not null,
  payment_method  text not null,
  mpesa_code      text,
  cash_tendered   bigint,
  change_given    bigint,
  status          text not null default 'completed',
  voided_by_uid   text,
  voided_at       timestamp,
  void_reason     text,
  created_at      timestamp
);

create table if not exists public.sale_items (
  uid           text primary key,
  store_id      uuid not null default auth.uid(),
  device_id     text,
  local_id      bigint,
  sale_uid      text not null,
  product_uid   text,
  product_name  text not null,
  qty           bigint not null,
  unit_price    bigint not null,
  unit_cost     bigint not null default 0,
  line_total    bigint not null
);

create table if not exists public.stock_movements (
  uid          text primary key,
  store_id     uuid not null default auth.uid(),
  device_id    text,
  local_id     bigint,
  product_uid  text,
  qty_delta    bigint not null,
  reason       text not null,
  sale_uid     text,
  note         text,
  user_uid     text,
  created_at   timestamp
);

create table if not exists public.audit_log (
  uid         text primary key,
  store_id    uuid not null default auth.uid(),
  device_id   text,
  local_id    bigint,
  user_uid    text,
  action      text not null,
  entity      text not null,
  entity_id   bigint,
  details     jsonb,
  created_at  timestamp
);

-- Indexes for the owner dashboard queries.
create index if not exists sales_store_created   on public.sales (store_id, created_at desc);
create index if not exists sale_items_sale       on public.sale_items (sale_uid);
create index if not exists products_store_upd    on public.products (store_id, updated_at);
create index if not exists movements_store_prod  on public.stock_movements (store_id, product_uid, created_at);
create index if not exists audit_store_created   on public.audit_log (store_id, created_at desc);

-- Editing a product in the cloud (Table Editor or a dashboard) bumps updated_at,
-- which is what the till watches to pull the change down.
create or replace function public.touch_updated_at() returns trigger language plpgsql as $$
begin
  if tg_op = 'UPDATE' and new.updated_at is not distinct from old.updated_at then
    new.updated_at := now() at time zone 'Africa/Nairobi';
  end if;
  return new;
end $$;
drop trigger if exists products_touch on public.products;
create trigger products_touch before update on public.products
  for each row execute function public.touch_updated_at();

-- Row Level Security: a store sees and writes only its own rows.
do $$
declare t text;
begin
  foreach t in array array['users','products','sales','sale_items','stock_movements','audit_log'] loop
    execute format('alter table public.%I enable row level security', t);
    execute format('drop policy if exists store_rw on public.%I', t);
    execute format('create policy store_rw on public.%I for all to authenticated using (store_id = auth.uid()) with check (store_id = auth.uid())', t);
  end loop;
end $$;

-- Handy view for a weekly summary per store.
create or replace view public.daily_sales as
select store_id, date(created_at) as day, count(*) as sales_count,
       sum(total) as net_cents,
       sum(total) filter (where payment_method = 'cash') as cash_cents,
       sum(total) filter (where payment_method = 'mpesa') as mpesa_cents
from public.sales
where status = 'completed'
group by store_id, date(created_at);

-- One row per till. Updated on every sync pass, whether or not the push succeeded,
-- so the owner can see a till that is online but stuck, or one that went dark.
create table if not exists public.devices (
  device_id           text primary key,
  store_id            uuid not null default auth.uid(),
  last_seen           timestamp,
  pending             bigint not null default 0,
  push_ok             boolean,
  last_sale_local_id  bigint,
  last_user           text,
  app_version         text
);
alter table public.devices enable row level security;
drop policy if exists store_rw on public.devices;
create policy store_rw on public.devices for all to authenticated
  using (store_id = auth.uid()) with check (store_id = auth.uid());
