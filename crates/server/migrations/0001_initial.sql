-- Croncave control plane: records, run queue, events and the ledger.
-- Money is stored in micro-dollars (bigint): $1 = 1_000_000.

create table catalog_versions (
    id          serial primary key,
    starts_at   timestamptz not null default now(),
    data        jsonb not null,
    note        text not null default '',
    created_by  uuid,
    created_at  timestamptz not null default now()
);

create table color_schemes (
    id          text not null,
    version     int not null,
    data        jsonb not null,
    created_at  timestamptz not null default now(),
    primary key (id, version)
);

create table users (
    id                uuid primary key,
    email             text not null unique,
    name              text not null default '',
    phone             text,
    phone_verified_at timestamptz,
    is_admin          boolean not null default false,
    prefs             jsonb not null default '{}',
    last_seen_at      timestamptz,
    previous_seen_at  timestamptz,
    created_at        timestamptz not null default now()
);

-- An account is one person in R1 and can become a group later without a migration.
create table accounts (
    id                    uuid primary key,
    kind                  text not null default 'personal',
    name                  text not null,
    catalog_version       int not null references catalog_versions(id),
    plan                  text,
    trial_plan            text,
    trial_started_at      timestamptz,
    trial_ends_at         timestamptz,
    period_start          timestamptz not null,
    spending_cap_micros   bigint,
    overage_enabled       boolean not null default false,
    ai_enabled            boolean not null default false,
    paused_at             timestamptz,
    cap_alert_level       int not null default 0,
    card_brand            text,
    card_last4            text,
    card_fingerprint      text,
    payment_customer      text,
    signup_completed_at   timestamptz,
    created_at            timestamptz not null default now()
);

create table account_members (
    account_id  uuid not null references accounts(id),
    user_id     uuid not null references users(id),
    role        text not null default 'owner',
    primary key (account_id, user_id)
);

create table account_overrides (
    id          uuid primary key,
    account_id  uuid not null references accounts(id),
    kind        text not null,
    data        jsonb not null default '{}',
    reason      text not null,
    actor_kind  text not null,
    actor_id    uuid,
    ends_at     timestamptz,
    created_at  timestamptz not null default now()
);

create table sign_in_tokens (
    token_hash  text primary key,
    email       text not null,
    expires_at  timestamptz not null,
    used_at     timestamptz
);

create table phone_codes (
    id          uuid primary key,
    user_id     uuid not null references users(id),
    phone       text not null,
    code_hash   text not null,
    attempts    int not null default 0,
    expires_at  timestamptz not null,
    used_at     timestamptz
);

create table sessions (
    token_hash  text primary key,
    user_id     uuid not null references users(id),
    user_agent  text not null default '',
    created_at  timestamptz not null default now(),
    expires_at  timestamptz not null
);

-- Awards and trials are tied to a verified phone number and card, not the email.
create table claims (
    kind        text not null,
    key_hash    text not null,
    account_id  uuid not null references accounts(id),
    created_at  timestamptz not null default now(),
    primary key (kind, key_hash)
);

create table computers (
    id                  uuid primary key,
    account_id          uuid not null references accounts(id),
    name                text not null,
    size                text not null,
    cpu                 int not null,
    memory_gb           int not null,
    disk_gb             int not null,
    state               text not null,
    state_changed_at    timestamptz not null default now(),
    sleep_delay_secs    int not null default 30,
    keep_awake          boolean not null default false,
    compute_ref         text,
    last_active_at      timestamptz not null default now(),
    wake_requested_at   timestamptz,
    wake_cause          text,
    bootstrap_hash      text,
    bootstrap_expires_at timestamptz,
    agent_epoch         uuid,
    agent_acked_seq     bigint not null default 0,
    agent_version       text,
    connected           boolean not null default false,
    health              jsonb not null default '{}',
    unmetered_awake_secs double precision not null default 0,
    unmetered_disk_secs  double precision not null default 0,
    note                text,
    created_at          timestamptz not null default now(),
    deleted_at          timestamptz
);
create index computers_account on computers(account_id) where deleted_at is null;

create table installs (
    id          uuid primary key,
    computer_id uuid not null references computers(id),
    app_id      text not null,
    version     text not null,
    settings    jsonb not null default '{}',
    created_at  timestamptz not null default now(),
    unique (computer_id, app_id)
);

create table watcher_types (
    id          text not null,
    version     int not null,
    config      jsonb not null,
    origin      text not null,
    account_id  uuid references accounts(id),
    approved_at timestamptz,
    created_at  timestamptz not null default now(),
    primary key (id, version)
);

create table jobs (
    id                uuid primary key,
    account_id        uuid not null references accounts(id),
    computer_id       uuid not null references computers(id),
    app               text not null,
    kind              text not null,
    name              text not null,
    setup             jsonb not null,
    trigger           text not null default 'manual',
    schedule          text,
    watch_path        text,
    overlap           text not null default 'skip',
    max_runtime_secs  int not null default 600,
    retries           int not null default 0,
    max_spend_micros  bigint,
    notify            jsonb not null default '{}',
    status            text not null default 'active',
    next_due_at       timestamptz,
    created_by_kind   text not null,
    created_by        uuid,
    created_at        timestamptz not null default now(),
    updated_at        timestamptz not null default now()
);
create index jobs_due on jobs(next_due_at) where status = 'active' and schedule is not null;
create index jobs_computer on jobs(computer_id);

create table runs (
    id                uuid primary key,
    job_id            uuid not null references jobs(id),
    account_id        uuid not null references accounts(id),
    computer_id       uuid not null references computers(id),
    app               text not null,
    kind              text not null,
    trigger           text not null,
    slot_at           timestamptz,
    started_by_kind   text not null,
    started_by        uuid,
    parent_run_id     uuid,
    attempt           int not null default 1,
    status            text not null,
    status_note       text,
    setup             jsonb not null,
    queued_at         timestamptz not null default now(),
    dispatched_at     timestamptz,
    started_at        timestamptz,
    ended_at          timestamptz,
    exit_code         int,
    headline          text,
    summary           jsonb,
    output_tail       text,
    error_plain       text,
    error_fix         text,
    data              jsonb,
    changes           jsonb,
    progress          jsonb,
    awake_seconds     int not null default 0,
    ai_cost_micros    bigint not null default 0,
    unique (job_id, slot_at)
);
create index runs_job on runs(job_id, queued_at desc);
create index runs_active on runs(status) where status in ('queued', 'waiting', 'starting', 'running');
create index runs_computer on runs(computer_id, queued_at desc);

create table run_output (
    id      bigserial primary key,
    run_id  uuid not null references runs(id) on delete cascade,
    stream  text not null,
    text    text not null,
    at      timestamptz not null default now()
);
create index run_output_run on run_output(run_id, id);

create table approvals (
    id          uuid primary key,
    run_id      uuid not null references runs(id) on delete cascade,
    request_id  text not null,
    command     text not null,
    reason      text not null,
    status      text not null default 'pending',
    decided_by  uuid,
    created_at  timestamptz not null default now(),
    decided_at  timestamptz,
    unique (run_id, request_id)
);

create table file_records (
    id          bigserial primary key,
    computer_id uuid not null references computers(id),
    path        text not null,
    change      text not null,
    actor_kind  text not null,
    actor_id    uuid,
    run_id      uuid,
    size        bigint not null default 0,
    at          timestamptz not null default now()
);
create index file_records_path on file_records(computer_id, path, id desc);
create index file_records_run on file_records(run_id);

create table open_ports (
    computer_id uuid not null references computers(id),
    port        int not null,
    run_id      uuid,
    opened_at   timestamptz not null default now(),
    primary key (computer_id, port)
);

create table previews (
    id            text primary key,
    computer_id   uuid not null references computers(id),
    account_id    uuid not null references accounts(id),
    user_id       uuid not null references users(id),
    port          int not null,
    created_at    timestamptz not null default now(),
    last_used_at  timestamptz
);

create table preview_tokens (
    token_hash  text primary key,
    preview_id  text not null references previews(id),
    kind        text not null,
    expires_at  timestamptz not null,
    used_at     timestamptz
);

-- What is keeping a computer awake: an open app, a Files view, a preview.
create table presence (
    computer_id uuid not null references computers(id),
    user_id     uuid not null references users(id),
    cause       text not null,
    app         text,
    until_at    timestamptz not null,
    primary key (computer_id, user_id, cause)
);

create table events (
    id          bigserial primary key,
    account_id  uuid not null references accounts(id),
    computer_id uuid references computers(id),
    actor_kind  text not null,
    actor_id    uuid,
    app         text not null,
    kind        text not null,
    level       text not null,
    title       text not null,
    body        text not null default '',
    data        jsonb not null default '{}',
    run_id      uuid,
    job_id      uuid,
    notify      boolean not null default false,
    urgent      boolean not null default false,
    read_at     timestamptz,
    created_at  timestamptz not null default now()
);
create index events_account on events(account_id, id desc);

create table notification_deliveries (
    event_id    bigint not null references events(id),
    channel     text not null,
    status      text not null,
    outbox_id   uuid,
    decided_at  timestamptz not null default now(),
    primary key (event_id, channel)
);

-- Everything the Notifier "sent". Real providers send; the outbox implementation keeps it here.
create table outbox (
    id          uuid primary key,
    channel     text not null,
    recipient   text not null,
    subject     text not null,
    body        text not null,
    link        text,
    created_at  timestamptz not null default now()
);

create table secrets (
    id          uuid primary key,
    account_id  uuid not null references accounts(id),
    job_id      uuid references jobs(id),
    name        text not null,
    value_enc   text not null,
    created_at  timestamptz not null default now(),
    unique (job_id, name)
);

-- One append-only balance ledger per account.
create table ledger_entries (
    id            bigserial primary key,
    account_id    uuid not null references accounts(id),
    at            timestamptz not null,
    period_start  timestamptz not null,
    kind          text not null,
    bucket        text not null,
    amount_micros bigint not null,
    computer_id   uuid,
    meter         text,
    detail        jsonb not null default '{}',
    actor_kind    text not null default 'system',
    actor_id      uuid,
    reason        text not null default ''
);
create index ledger_account on ledger_entries(account_id, period_start);

create table usage_hourly (
    account_id    uuid not null references accounts(id),
    computer_id   uuid not null,
    hour          timestamptz not null,
    meter         text not null,
    quantity      double precision not null default 0,
    cost_micros   bigint not null default 0,
    primary key (computer_id, hour, meter)
);
create index usage_account on usage_hourly(account_id, hour);

create table invoices (
    id            uuid primary key,
    account_id    uuid not null references accounts(id),
    period_start  timestamptz not null,
    period_end    timestamptz not null,
    lines         jsonb not null,
    total_micros  bigint not null,
    status        text not null,
    provider_ref  text,
    created_at    timestamptz not null default now()
);

create table wake_measurements (
    id            bigserial primary key,
    computer_id   uuid not null,
    account_id    uuid not null,
    size          text not null,
    cause         text not null,
    requested_at  timestamptz not null,
    ready_at      timestamptz not null,
    ms            int not null
);

create table awake_by_cause (
    computer_id uuid not null,
    account_id  uuid not null,
    day         date not null,
    cause       text not null,
    seconds     double precision not null default 0,
    primary key (computer_id, day, cause)
);

create table funnel_events (
    id          bigserial primary key,
    account_id  uuid,
    kind        text not null,
    data        jsonb not null default '{}',
    at          timestamptz not null default now()
);

create table audit_log (
    id          bigserial primary key,
    account_id  uuid,
    actor_kind  text not null,
    actor_id    uuid,
    action      text not null,
    target      text not null default '',
    data        jsonb not null default '{}',
    at          timestamptz not null default now()
);

create table assistant_messages (
    id          uuid primary key,
    account_id  uuid not null references accounts(id),
    role        text not null,
    text        text not null,
    proposals   jsonb not null default '[]',
    tokens_in   int not null default 0,
    tokens_out  int not null default 0,
    cost_micros bigint not null default 0,
    created_at  timestamptz not null default now()
);
