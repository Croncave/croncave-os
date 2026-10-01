//! Plans, awards, trials, usage, caps and invoices, all driven by the plan catalog and
//! one append-only ledger per account.

use std::sync::Arc;
use std::time::Duration;

use axum::Json;
use axum::extract::{Path, State};
use chrono::{DateTime, Months, Utc};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::auth::Auth;
use crate::catalog::{Catalog, Plan, micros};
use crate::error::{ApiError, ApiResult};
use crate::events::{self, NewEvent};
use crate::ledger::{self, Balances, Bucket, CapRules};
use crate::measure::funnel;
use crate::model::Account;
use crate::providers::payments::{CardInput, PaymentError};
use crate::state::AppState;

/// The catalog and the plan whose limits apply to this account now.
pub async fn plan_of(app: &AppState, account: &Account) -> ApiResult<(Arc<Catalog>, Plan)> {
    let catalog = app.catalog(account.catalog_version).await?;
    let plan = catalog
        .plan(account.effective_plan())
        .cloned()
        .ok_or_else(|| ApiError::bad("Your plan isn't in the catalog any more. Contact support."))?;
    Ok((catalog, plan))
}

pub async fn balances(conn: &mut PgConnection, account: Uuid, period_start: DateTime<Utc>) -> sqlx::Result<Balances> {
    let rows: Vec<(String, i64, i64, i64)> = sqlx::query_as(
        "select bucket,
                coalesce(sum(amount_micros), 0)::bigint,
                coalesce(sum(amount_micros) filter (where kind = 'usage' and period_start = $2), 0)::bigint,
                0::bigint
         from ledger_entries where account_id = $1 group by bucket",
    )
    .bind(account)
    .bind(period_start)
    .fetch_all(&mut *conn)
    .await?;
    let mut b = Balances::default();
    for (bucket, total, used_this_period, _) in rows {
        match Bucket::parse(&bucket) {
            Some(Bucket::Award) => b.award = total,
            Some(Bucket::Credit) => b.credit = total,
            Some(Bucket::Trial) => b.trial = total,
            Some(Bucket::Allowance) => {
                b.allowance = total;
                b.cap_spend += -used_this_period;
            }
            Some(Bucket::Overage) => {
                b.overage = -used_this_period;
                b.cap_spend += -used_this_period;
            }
            _ => {}
        }
    }
    Ok(b)
}

pub fn cap_rules(account: &Account, plan: &Plan) -> CapRules {
    CapRules {
        cap: account.spending_cap_micros.unwrap_or_else(|| micros(plan.allowance)),
        overage_enabled: account.overage_enabled && plan.overage,
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn grant(
    conn: &mut PgConnection,
    account: Uuid,
    period_start: DateTime<Utc>,
    at: DateTime<Utc>,
    kind: &str,
    bucket: Bucket,
    amount: i64,
    reason: &str,
    actor: (&str, Option<Uuid>),
) -> sqlx::Result<()> {
    if amount == 0 {
        return Ok(());
    }
    sqlx::query(
        "insert into ledger_entries (account_id, at, period_start, kind, bucket, amount_micros, reason, actor_kind, actor_id)
         values ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
    )
    .bind(account)
    .bind(at)
    .bind(period_start)
    .bind(kind)
    .bind(bucket.as_str())
    .bind(amount)
    .bind(reason)
    .bind(actor.0)
    .bind(actor.1)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// Draw a cost from the account's buckets in order, then check alerts and the cap.
pub async fn record_usage(
    app: &AppState,
    account_id: Uuid,
    cost: i64,
    computer: Option<Uuid>,
    meter: &str,
    detail: Value,
) -> anyhow::Result<()> {
    if cost <= 0 {
        return Ok(());
    }
    let mut tx = app.db.begin().await?;
    let account: Account =
        sqlx::query_as("select * from accounts where id = $1 for update").bind(account_id).fetch_one(&mut *tx).await?;
    let (_, plan) = plan_of(app, &account).await.map_err(|e| anyhow::anyhow!(e.message))?;
    let rules = cap_rules(&account, &plan);
    let b = balances(&mut tx, account.id, account.period_start).await?;
    let now = app.now();
    for (bucket, amount) in ledger::allocate(cost, &b, rules) {
        sqlx::query(
            "insert into ledger_entries (account_id, at, period_start, kind, bucket, amount_micros, computer_id, meter, detail)
             values ($1, $2, $3, 'usage', $4, $5, $6, $7, $8)",
        )
        .bind(account.id)
        .bind(now)
        .bind(account.period_start)
        .bind(bucket.as_str())
        .bind(-amount)
        .bind(computer)
        .bind(meter)
        .bind(&detail)
        .execute(&mut *tx)
        .await?;
    }
    after_usage(app, &mut tx, &account, rules).await?;
    tx.commit().await?;
    Ok(())
}

/// Alerts at 50%, 80% and 100% of the cap, and pausing when nothing is left.
async fn after_usage(app: &AppState, tx: &mut PgConnection, account: &Account, rules: CapRules) -> anyhow::Result<()> {
    let b = balances(tx, account.id, account.period_start).await?;
    let level = ledger::alert_level(&b, rules);
    let cap = dollars(rules.cap);
    if level > account.cap_alert_level && b.cap_spend > 0 {
        sqlx::query("update accounts set cap_alert_level = $2 where id = $1")
            .bind(account.id)
            .bind(level)
            .execute(&mut *tx)
            .await?;
        let ev = NewEvent::new(
            account.id,
            "billing",
            "cap.alert",
            if level >= 100 { "needs_you" } else { "warning" },
            format!("You've used {level}% of your {cap} spending cap"),
        )
        .body("Raise the cap or turn on overage in Plans and billing to keep work running past it.")
        .data(json!({ "level": level }));
        events::emit(&mut *tx, if level >= 100 { ev.urgent() } else { ev.notify() }).await?;
    }
    if account.paused_at.is_none() && ledger::exhausted(&b, rules) {
        sqlx::query("update accounts set paused_at = $2 where id = $1")
            .bind(account.id)
            .bind(app.now())
            .execute(&mut *tx)
            .await?;
        let why = if rules.overage_enabled || b.award + b.credit + b.trial > 0 {
            "your spending cap"
        } else {
            "your monthly allowance"
        };
        events::emit(
            &mut *tx,
            NewEvent::new(account.id, "billing", "cap.reached", "needs_you", format!("Work paused at {why}"))
                .body("Your computers finish what they're doing and go to sleep. Nothing is deleted. Raise the cap or turn on overage to resume.")
                .urgent(),
        )
        .await?;
        funnel(&app.db, Some(account.id), "cap_reached", json!({ "cap": rules.cap })).await;
        events::live(&mut *tx, account.id, "account", &account.id.to_string(), json!({})).await?;
    }
    Ok(())
}

/// Resume work if the account is paused but can draw again.
pub async fn maybe_resume(app: &AppState, tx: &mut PgConnection, account_id: Uuid) -> anyhow::Result<bool> {
    let account: Account =
        sqlx::query_as("select * from accounts where id = $1 for update").bind(account_id).fetch_one(&mut *tx).await?;
    if account.paused_at.is_none() {
        return Ok(false);
    }
    let (_, plan) = plan_of(app, &account).await.map_err(|e| anyhow::anyhow!(e.message))?;
    let rules = cap_rules(&account, &plan);
    let b = balances(tx, account.id, account.period_start).await?;
    if ledger::exhausted(&b, rules) {
        return Ok(false);
    }
    sqlx::query("update accounts set paused_at = null, cap_alert_level = $2 where id = $1")
        .bind(account.id)
        .bind(ledger::alert_level(&b, rules))
        .execute(&mut *tx)
        .await?;
    sqlx::query("update runs set status = 'queued', status_note = null where account_id = $1 and status = 'held'")
        .bind(account.id)
        .execute(&mut *tx)
        .await?;
    events::emit(
        &mut *tx,
        NewEvent::new(account.id, "billing", "cap.resumed", "success", "Work resumed")
            .body("Held runs are starting again."),
    )
    .await?;
    events::live(&mut *tx, account.id, "account", &account.id.to_string(), json!({})).await?;
    app.kick.notify_waiters();
    Ok(true)
}

pub fn dollars(m: i64) -> String {
    let d = m as f64 / 1_000_000.0;
    if d.abs() >= 1.0 || d == 0.0 { format!("${d:.2}") } else { format!("${d:.4}") }
}

// ---------------------------------------------------------------------------------------
// Background work: metering, months, trials.
// ---------------------------------------------------------------------------------------

pub async fn run_loop(app: AppState) {
    let mut last_meter = tokio::time::Instant::now();
    loop {
        tokio::time::sleep(Duration::from_secs(2)).await;
        if last_meter.elapsed() >= Duration::from_secs(15) {
            last_meter = tokio::time::Instant::now();
            if let Err(e) = meter(&app).await {
                tracing::error!(error = %e, "metering failed");
            }
        }
        if let Err(e) = roll_periods(&app).await {
            tracing::error!(error = %e, "closing billing periods failed");
        }
        if let Err(e) = trials(&app).await {
            tracing::error!(error = %e, "trial housekeeping failed");
        }
    }
}

/// Turn awake seconds and stored GB-seconds into usage and ledger draws.
pub async fn meter(app: &AppState) -> anyhow::Result<()> {
    let rows: Vec<(Uuid, Uuid, String, f64, f64, Value, i32)> = sqlx::query_as(
        "with old as (
            select id, unmetered_awake_secs a, unmetered_disk_secs d from computers
            where unmetered_awake_secs > 0 or unmetered_disk_secs > 0 for update
         )
         update computers c set unmetered_awake_secs = 0, unmetered_disk_secs = 0
         from old where c.id = old.id
         returning c.id, c.account_id, c.size, old.a, old.d, c.health, (select catalog_version from accounts where id = c.account_id)",
    )
    .fetch_all(&app.db)
    .await?;
    let now = app.now();
    let hour = now.date_naive().and_hms_opt(chrono::Timelike::hour(&now), 0, 0).expect("valid hour").and_utc();
    let mut per_account: std::collections::HashMap<Uuid, (i64, Value)> = Default::default();
    for (computer, account, size, awake_secs, disk_secs, health, version) in rows {
        let catalog = app.catalog(version).await?;
        let awake_cost = (awake_secs * catalog.awake_hourly_micros(&size) as f64 / 3600.0).round() as i64;
        // Storage is billed by what is stored (files and Trash), not the disk's size.
        let stored_gb =
            (health["disk_bytes"].as_f64().unwrap_or(0.0) + health["trash_bytes"].as_f64().unwrap_or(0.0)) / 1e9;
        let disk_rate = catalog.disk_gb_month * (1.0 + catalog.markup) * 1e6 / (30.0 * 86400.0);
        let disk_cost = (stored_gb * disk_secs * disk_rate).round() as i64;
        for (meter, qty, cost) in
            [("compute", awake_secs, awake_cost), ("storage", stored_gb * disk_secs / 3600.0, disk_cost)]
        {
            if qty <= 0.0 {
                continue;
            }
            sqlx::query(
                "insert into usage_hourly (account_id, computer_id, hour, meter, quantity, cost_micros) values ($1, $2, $3, $4, $5, $6)
                 on conflict (computer_id, hour, meter) do update set quantity = usage_hourly.quantity + excluded.quantity,
                                                                     cost_micros = usage_hourly.cost_micros + excluded.cost_micros",
            )
            .bind(account)
            .bind(computer)
            .bind(hour)
            .bind(meter)
            .bind(qty)
            .bind(cost)
            .execute(&app.db)
            .await?;
        }
        let e = per_account.entry(account).or_insert((0, json!({})));
        e.0 += awake_cost + disk_cost;
        e.1[computer.to_string()] = json!({ "awake_secs": awake_secs, "compute": awake_cost, "storage": disk_cost });
    }
    for (account, (cost, detail)) in per_account {
        record_usage(app, account, cost, None, "computers", detail).await?;
    }
    Ok(())
}

async fn roll_periods(app: &AppState) -> anyhow::Result<()> {
    let now = app.now();
    let due: Vec<Uuid> = sqlx::query_scalar(
        "select id from accounts where plan is not null and period_start + interval '1 month' <= $1",
    )
    .bind(now)
    .fetch_all(&app.db)
    .await?;
    for id in due {
        close_period(app, id).await?;
    }
    Ok(())
}

/// End a month: invoice the next month's plan and this month's overage, expire what is
/// left of the allowance, grant the new one.
pub async fn close_period(app: &AppState, account_id: Uuid) -> anyhow::Result<()> {
    let mut tx = app.db.begin().await?;
    let account: Account =
        sqlx::query_as("select * from accounts where id = $1 for update").bind(account_id).fetch_one(&mut *tx).await?;
    let start = account.period_start;
    let end = start.checked_add_months(Months::new(1)).expect("date in range");
    if end > app.now() {
        return Ok(());
    }
    let catalog = app.catalog(account.catalog_version).await?;
    let base = catalog
        .plan(account.plan.as_deref().unwrap_or("free"))
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("plan missing"))?;
    let b = balances(&mut tx, account.id, start).await?;
    let mut lines = vec![];
    if b.overage > 0 {
        lines.push(json!({ "label": format!("Usage past your allowance, {} to {}", start.format("%b %-d"), end.format("%b %-d")), "amount_micros": b.overage }));
    }
    if base.price_monthly > 0.0 {
        lines.push(json!({ "label": format!("{} plan, {} to {}", base.name, end.format("%b %-d"), end.checked_add_months(Months::new(1)).expect("in range").format("%b %-d")), "amount_micros": micros(base.price_monthly) }));
    }
    if !lines.is_empty() {
        charge_invoice(app, &mut tx, &account, start, end, lines).await?;
    }
    grant(
        &mut tx,
        account.id,
        start,
        end,
        "expire",
        Bucket::Allowance,
        -b.allowance.max(0),
        "Unused allowance at the end of the month",
        ("system", None),
    )
    .await?;
    grant(
        &mut tx,
        account.id,
        end,
        end,
        "allowance",
        Bucket::Allowance,
        micros(base.allowance),
        &format!("{} monthly allowance", base.name),
        ("system", None),
    )
    .await?;
    sqlx::query("update accounts set period_start = $2, cap_alert_level = 0 where id = $1")
        .bind(account.id)
        .bind(end)
        .execute(&mut *tx)
        .await?;
    maybe_resume(app, &mut tx, account.id).await?;
    tx.commit().await?;
    Ok(())
}

async fn charge_invoice(
    app: &AppState,
    tx: &mut PgConnection,
    account: &Account,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    lines: Vec<Value>,
) -> anyhow::Result<(Uuid, bool)> {
    let total: i64 = lines.iter().filter_map(|l| l["amount_micros"].as_i64()).sum();
    let customer = account.payment_customer.clone().unwrap_or_default();
    let result = if total > 0 && !customer.is_empty() {
        app.providers.payments.charge(&customer, total, "Croncave").await.map(|c| Some(c.id))
    } else {
        Ok(None)
    };
    let (status, reference) = match &result {
        Ok(r) => ("paid", r.clone()),
        Err(_) => ("failed", None),
    };
    let id = Uuid::new_v4();
    sqlx::query("insert into invoices (id, account_id, period_start, period_end, lines, total_micros, status, provider_ref) values ($1, $2, $3, $4, $5, $6, $7, $8)")
        .bind(id)
        .bind(account.id)
        .bind(start)
        .bind(end)
        .bind(json!(lines))
        .bind(total)
        .bind(status)
        .bind(reference)
        .execute(&mut *tx)
        .await?;
    if let Err(e) = result {
        events::emit(
            &mut *tx,
            NewEvent::new(
                account.id,
                "billing",
                "invoice.failed",
                "needs_you",
                format!("A payment of {} didn't go through", dollars(total)),
            )
            .body(e.to_string())
            .urgent(),
        )
        .await?;
    }
    Ok((id, status == "paid"))
}

async fn trials(app: &AppState) -> anyhow::Result<()> {
    let now = app.now();
    let ending: Vec<Account> =
        sqlx::query_as("select * from accounts where trial_plan is not null and trial_ends_at <= $1")
            .bind(now)
            .fetch_all(&app.db)
            .await?;
    for a in ending {
        end_trial(app, a.id, false).await?;
    }
    // A reminder two days before a trial ends.
    let soon: Vec<Account> = sqlx::query_as(
        "select a.* from accounts a where trial_plan is not null and trial_ends_at <= $1 + interval '2 days'
         and not exists (select 1 from events e where e.account_id = a.id and e.kind = 'trial.ending' and e.created_at >= a.trial_started_at)",
    )
    .bind(now)
    .fetch_all(&app.db)
    .await?;
    for a in soon {
        let catalog = app.catalog(a.catalog_version).await?;
        let trial =
            catalog.plan(a.trial_plan.as_deref().unwrap_or_default()).map(|p| p.name.clone()).unwrap_or_default();
        let base = catalog.plan(a.plan.as_deref().unwrap_or("free")).map(|p| p.name.clone()).unwrap_or_default();
        events::emit(
            &app.db,
            NewEvent::new(a.id, "billing", "trial.ending", "needs_you", format!("Your {trial} trial ends on {}", a.trial_ends_at.unwrap_or(now).format("%B %-d")))
                .body(format!("You'll go back to {base} and nothing is charged. Keep {trial} from Plans and billing if you'd like to stay."))
                .notify(),
        )
        .await?;
    }
    Ok(())
}

/// End a trial: back to the original plan, no charge. `kept` is true when the person
/// chose to stay on the trial plan (they paid for it separately).
pub async fn end_trial(app: &AppState, account_id: Uuid, kept: bool) -> anyhow::Result<()> {
    let mut tx = app.db.begin().await?;
    let a: Account =
        sqlx::query_as("select * from accounts where id = $1 for update").bind(account_id).fetch_one(&mut *tx).await?;
    let Some(trial_plan) = a.trial_plan.clone() else { return Ok(()) };
    let catalog = app.catalog(a.catalog_version).await?;
    let b = balances(&mut tx, a.id, a.period_start).await?;
    let now = app.now();
    grant(
        &mut tx,
        a.id,
        a.period_start,
        now,
        "expire",
        Bucket::Trial,
        -b.trial.max(0),
        "Trial ended",
        ("system", None),
    )
    .await?;
    sqlx::query("update accounts set trial_plan = null, trial_ends_at = null where id = $1")
        .bind(a.id)
        .execute(&mut *tx)
        .await?;
    if !kept {
        let trial = catalog.plan(&trial_plan).map(|p| p.name.clone()).unwrap_or_default();
        let base = catalog.plan(a.plan.as_deref().unwrap_or("free")).map(|p| p.name.clone()).unwrap_or_default();
        events::emit(
            &mut *tx,
            NewEvent::new(a.id, "billing", "trial.ended", "info", format!("Your {trial} trial ended"))
                .body(format!("You're back on {base}. Nothing was charged."))
                .notify(),
        )
        .await?;
        funnel(&app.db, Some(a.id), "trial_ended", json!({ "trial_plan": trial_plan })).await;
    }
    events::live(&mut *tx, a.id, "account", &a.id.to_string(), json!({})).await?;
    tx.commit().await?;
    Ok(())
}

// ---------------------------------------------------------------------------------------
// Sign-up: choose a plan, pay if needed, the award, the trial offer.
// ---------------------------------------------------------------------------------------

pub async fn plans(State(app): State<AppState>, auth: Auth) -> ApiResult<Json<Value>> {
    let catalog = app.catalog(auth.account.catalog_version).await?;
    let mut plans: Vec<(String, Plan)> = catalog.plans.clone().into_iter().collect();
    plans.sort_by_key(|(_, p)| p.order);
    let plans: Vec<Value> = plans.into_iter().map(|(id, p)| json!({ "id": id, "plan": p })).collect();
    Ok(Json(
        json!({ "plans": plans, "sizes": catalog.sizes, "catalog_version": auth.account.catalog_version, "markup": catalog.markup, "disk_gb_month": catalog.disk_gb_month }),
    ))
}

#[derive(Deserialize)]
pub struct ChoosePlan {
    pub plan: String,
    pub card: Option<CardInput>,
}

pub async fn choose_plan(State(app): State<AppState>, auth: Auth, Json(b): Json<ChoosePlan>) -> ApiResult<Json<Value>> {
    if auth.user.phone_verified_at.is_none() {
        return Err(ApiError::forbidden("Verify your phone number first.").with_code("needs_phone"));
    }
    if auth.account.plan.is_some() {
        return Err(ApiError::conflict("You've already chosen a plan. Change it in Plans and billing."));
    }
    let catalog = app.catalog(auth.account.catalog_version).await?;
    let plan = catalog.plan(&b.plan).cloned().ok_or_else(|| ApiError::bad("That plan doesn't exist."))?;
    funnel(&app.db, Some(auth.account.id), "plan_chosen", json!({ "plan": b.plan })).await;
    let now = app.now();
    let mut tx = app.db.begin().await?;
    let card_fp = if plan.requires_card {
        let card =
            b.card.as_ref().ok_or_else(|| ApiError::bad("Add a card to start this plan.").with_code("needs_card"))?;
        let fp = attach_card(&app, &mut tx, &auth, card).await?;
        let account: Account =
            sqlx::query_as("select * from accounts where id = $1").bind(auth.account.id).fetch_one(&mut *tx).await?;
        let (_, paid) = charge_invoice(&app, &mut tx, &account, now, now.checked_add_months(Months::new(1)).expect("in range"),
            vec![json!({ "label": format!("{} plan, {} to {}", plan.name, now.format("%b %-d"), now.checked_add_months(Months::new(1)).expect("in range").format("%b %-d")), "amount_micros": micros(plan.price_monthly) })]).await?;
        if !paid {
            return Err(ApiError::bad("Your card was charged but the payment didn't go through. Try another card.")
                .with_code("payment_failed"));
        }
        funnel(&app.db, Some(auth.account.id), "checkout_succeeded", json!({ "plan": b.plan })).await;
        Some(fp)
    } else {
        None
    };
    sqlx::query("update accounts set plan = $2, period_start = $3 where id = $1")
        .bind(auth.account.id)
        .bind(&b.plan)
        .bind(now)
        .execute(&mut *tx)
        .await?;
    grant(
        &mut tx,
        auth.account.id,
        now,
        now,
        "allowance",
        Bucket::Allowance,
        micros(plan.allowance),
        &format!("{} monthly allowance", plan.name),
        ("system", None),
    )
    .await?;

    // The award: once per account, tied to the verified phone and the card, not the email.
    let phone = auth.user.phone.clone().unwrap_or_default();
    let mut award_note = None;
    let mut awarded = 0;
    let phone_free = claim(&mut tx, "award_phone", &phone, auth.account.id).await?;
    let card_free = match &card_fp {
        Some(fp) => claim(&mut tx, "award_card", fp, auth.account.id).await?,
        None => true,
    };
    if phone_free && card_free {
        awarded = micros(plan.award);
        grant(
            &mut tx,
            auth.account.id,
            now,
            now,
            "award",
            Bucket::Award,
            awarded,
            &format!("{} sign-up award", plan.name),
            ("system", None),
        )
        .await?;
    } else {
        award_note =
            Some("This phone number or card already received a sign-up award, so there isn't another one.".to_string());
    }
    tx.commit().await?;
    if awarded > 0 {
        funnel(&app.db, Some(auth.account.id), "award_granted", json!({ "plan": b.plan, "amount_micros": awarded }))
            .await;
    }
    Ok(Json(
        json!({ "ok": true, "award_micros": awarded, "award_note": award_note, "trial": trial_offer(&app, &auth.account.id, &catalog, &b.plan, &phone).await? }),
    ))
}

async fn attach_card(app: &AppState, tx: &mut PgConnection, auth: &Auth, card: &CardInput) -> ApiResult<String> {
    let customer = match &auth.account.payment_customer {
        Some(c) => c.clone(),
        None => app.providers.payments.create_customer(auth.account.id, &auth.user.email).await.map_err(pay_err)?,
    };
    let c = app.providers.payments.attach_card(&customer, card).await.map_err(|e| {
        let app = app.clone();
        let id = auth.account.id;
        tokio::spawn(async move { funnel(&app.db, Some(id), "checkout_failed", json!({})).await });
        pay_err(e)
    })?;
    sqlx::query("update accounts set payment_customer = $2, card_brand = $3, card_last4 = $4, card_fingerprint = $5 where id = $1")
        .bind(auth.account.id)
        .bind(&customer)
        .bind(&c.brand)
        .bind(&c.last4)
        .bind(&c.fingerprint)
        .execute(&mut *tx)
        .await?;
    Ok(c.fingerprint)
}

fn pay_err(e: PaymentError) -> ApiError {
    match e {
        PaymentError::Declined(m) => ApiError::bad(m).with_code("card_declined"),
        PaymentError::NotConfigured(m) => ApiError::unavailable(m),
    }
}

/// Take a one-time claim (an award or trial for a phone or card). False if already taken.
async fn claim(tx: &mut PgConnection, kind: &str, key: &str, account: Uuid) -> sqlx::Result<bool> {
    if key.is_empty() {
        return Ok(false);
    }
    let r = sqlx::query("insert into claims (kind, key_hash, account_id) values ($1, $2, $3) on conflict do nothing")
        .bind(kind)
        .bind(crate::crypto::hash(key))
        .bind(account)
        .execute(&mut *tx)
        .await?;
    Ok(r.rows_affected() == 1)
}

async fn trial_offer(app: &AppState, account: &Uuid, catalog: &Catalog, plan: &str, phone: &str) -> ApiResult<Value> {
    let Some(t) = catalog.plan(plan).and_then(|p| p.trial.clone()) else { return Ok(Value::Null) };
    let (used,): (bool,) = sqlx::query_as(
        "select exists(select 1 from claims where kind = 'trial_phone' and key_hash = $1 and account_id <> $2)",
    )
    .bind(crate::crypto::hash(phone))
    .bind(account)
    .fetch_one(&app.db)
    .await?;
    if used {
        return Ok(Value::Null);
    }
    let tp = catalog.plan(&t.plan).cloned().ok_or_else(|| ApiError::bad("trial plan missing"))?;
    Ok(
        json!({ "plan": t.plan, "name": tp.name, "days": t.days, "allowance_micros": prorated(&tp, t.days), "details": tp }),
    )
}

fn prorated(plan: &Plan, days: i64) -> i64 {
    micros(plan.allowance * days as f64 / 30.0)
}

pub async fn get_trial_offer(State(app): State<AppState>, auth: Auth) -> ApiResult<Json<Value>> {
    let catalog = app.catalog(auth.account.catalog_version).await?;
    let plan = auth.account.plan.clone().ok_or_else(|| ApiError::bad("Choose a plan first."))?;
    let offer = if auth.account.signup_completed_at.is_none() {
        trial_offer(&app, &auth.account.id, &catalog, &plan, auth.user.phone.as_deref().unwrap_or_default()).await?
    } else {
        Value::Null
    };
    Ok(Json(json!({ "offer": offer })))
}

#[derive(Deserialize)]
pub struct TrialAnswer {
    pub accept: bool,
}

pub async fn answer_trial(
    State(app): State<AppState>,
    auth: Auth,
    Json(b): Json<TrialAnswer>,
) -> ApiResult<Json<Value>> {
    let plan = auth.account.plan.clone().ok_or_else(|| ApiError::bad("Choose a plan first."))?;
    if auth.account.signup_completed_at.is_some() {
        return Err(ApiError::conflict("Trials are offered once, at sign-up."));
    }
    let catalog = app.catalog(auth.account.catalog_version).await?;
    let phone = auth.user.phone.clone().unwrap_or_default();
    let now = app.now();
    let mut tx = app.db.begin().await?;
    let mut started = Value::Null;
    if b.accept {
        let offer = trial_offer(&app, &auth.account.id, &catalog, &plan, &phone).await?;
        if offer.is_null() {
            return Err(ApiError::bad("There's no trial to start for this plan or phone number."));
        }
        claim(&mut tx, "trial_phone", &phone, auth.account.id).await?;
        let trial_plan = offer["plan"].as_str().unwrap_or_default().to_string();
        let days = offer["days"].as_i64().unwrap_or(7);
        let ends = now + chrono::Duration::days(days);
        sqlx::query("update accounts set trial_plan = $2, trial_started_at = $3, trial_ends_at = $4 where id = $1")
            .bind(auth.account.id)
            .bind(&trial_plan)
            .bind(now)
            .bind(ends)
            .execute(&mut *tx)
            .await?;
        // Trials come with a pro-rated allowance and never with an award.
        let amount = offer["allowance_micros"].as_i64().unwrap_or(0);
        grant(
            &mut tx,
            auth.account.id,
            auth.account.period_start,
            now,
            "trial_allowance",
            Bucket::Trial,
            amount,
            &format!("{}-day {} trial allowance", days, offer["name"].as_str().unwrap_or("")),
            ("system", None),
        )
        .await?;
        started = json!({ "plan": trial_plan, "ends_at": ends });
    }
    sqlx::query("update accounts set signup_completed_at = $2 where id = $1")
        .bind(auth.account.id)
        .bind(now)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    funnel(
        &app.db,
        Some(auth.account.id),
        if b.accept { "trial_started" } else { "trial_skipped" },
        json!({ "plan": plan }),
    )
    .await;
    Ok(Json(json!({ "ok": true, "trial": started })))
}

// ---------------------------------------------------------------------------------------
// Plans and billing pages.
// ---------------------------------------------------------------------------------------

pub async fn summary(State(app): State<AppState>, auth: Auth) -> ApiResult<Json<Value>> {
    let a = &auth.account;
    let (catalog, plan) = plan_of(&app, a).await?;
    let mut conn = app.db.acquire().await?;
    let b = balances(&mut conn, a.id, a.period_start).await?;
    let rules = cap_rules(a, &plan);
    let base = catalog.plan(a.plan.as_deref().unwrap_or("free")).cloned();
    let period_end = a.period_start.checked_add_months(Months::new(1)).expect("in range");
    let by_day: Vec<(chrono::NaiveDate, String, i64, f64)> = sqlx::query_as(
        "select (hour at time zone 'utc')::date, meter, sum(cost_micros)::bigint, sum(quantity) from usage_hourly
         where account_id = $1 and hour >= $2 group by 1, 2 order by 1",
    )
    .bind(a.id)
    .bind(a.period_start - chrono::Duration::days(1))
    .fetch_all(&mut *conn)
    .await?;
    let by_computer: Vec<(Uuid, String, String, i64, f64)> = sqlx::query_as(
        "select u.computer_id, coalesce(c.name, 'Deleted computer'), u.meter, sum(u.cost_micros)::bigint, sum(u.quantity)
         from usage_hourly u left join computers c on c.id = u.computer_id
         where u.account_id = $1 and u.hour >= $2 group by 1, 2, 3 order by 2",
    )
    .bind(a.id)
    .bind(a.period_start)
    .fetch_all(&mut *conn)
    .await?;
    let by_app: Vec<(String, i64, i64)> = sqlx::query_as(
        "select app, count(*)::bigint, coalesce(sum(awake_seconds), 0)::bigint from runs where account_id = $1 and queued_at >= $2 group by app",
    )
    .bind(a.id)
    .bind(a.period_start)
    .fetch_all(&mut *conn)
    .await?;
    let ai: (i64,) = sqlx::query_as("select coalesce(sum(cost_micros), 0)::bigint from assistant_messages where account_id = $1 and created_at >= $2")
        .bind(a.id)
        .bind(a.period_start)
        .fetch_one(&mut *conn)
        .await?;
    let invoices: Vec<(Uuid, DateTime<Utc>, DateTime<Utc>, i64, String, Value)> = sqlx::query_as(
        "select id, period_start, period_end, total_micros, status, lines from invoices where account_id = $1 order by created_at desc limit 24",
    )
    .bind(a.id)
    .fetch_all(&mut *conn)
    .await?;
    let ledger: Vec<(DateTime<Utc>, String, String, i64, String)> = sqlx::query_as(
        "select at, kind, bucket, amount_micros, reason from ledger_entries where account_id = $1 and kind <> 'usage' order by id desc limit 20",
    )
    .bind(a.id)
    .fetch_all(&mut *conn)
    .await?;
    let used: (i64,) = sqlx::query_as("select coalesce(-sum(amount_micros), 0)::bigint from ledger_entries where account_id = $1 and kind = 'usage' and period_start = $2")
        .bind(a.id)
        .bind(a.period_start)
        .fetch_one(&mut *conn)
        .await?;
    Ok(Json(json!({
        "plan_id": a.plan, "plan": base, "effective_plan_id": a.effective_plan(), "effective_plan": plan,
        "trial": a.trial_plan.as_ref().map(|t| json!({ "plan": t, "name": catalog.plan(t).map(|p| p.name.clone()), "ends_at": a.trial_ends_at, "started_at": a.trial_started_at })),
        "period": { "start": a.period_start, "end": period_end },
        "balances": b,
        "left_micros": b.award + b.credit + b.trial + b.allowance.max(0),
        "used_this_period_micros": used.0,
        "cap": { "micros": rules.cap, "overage_enabled": rules.overage_enabled, "overage_allowed": plan.overage, "default_micros": micros(plan.allowance), "custom": a.spending_cap_micros.is_some() },
        "alert_level": ledger::alert_level(&b, rules),
        "paused": a.paused_at,
        "card": a.card_last4.as_ref().map(|l| json!({ "brand": a.card_brand, "last4": l })),
        "usage_by_day": by_day.into_iter().map(|(d, m, c, q)| json!({ "day": d, "meter": m, "cost_micros": c, "quantity": q })).collect::<Vec<_>>(),
        "usage_by_computer": by_computer.into_iter().map(|(id, n, m, c, q)| json!({ "computer_id": id, "name": n, "meter": m, "cost_micros": c, "quantity": q })).collect::<Vec<_>>(),
        "usage_by_app": by_app.into_iter().map(|(app, runs, secs)| json!({ "app": app, "runs": runs, "awake_seconds": secs })).collect::<Vec<_>>(),
        "ai_cost_micros": ai.0,
        "invoices": invoices.into_iter().map(|(id, s, e, t, st, lines)| json!({ "id": id, "period_start": s, "period_end": e, "total_micros": t, "status": st, "lines": lines })).collect::<Vec<_>>(),
        "grants": ledger.into_iter().map(|(at, k, b, m, r)| json!({ "at": at, "kind": k, "bucket": b, "amount_micros": m, "reason": r })).collect::<Vec<_>>(),
        "sizes": catalog.sizes, "markup": catalog.markup,
    })))
}

#[derive(Deserialize)]
pub struct CapBody {
    pub cap_dollars: Option<f64>,
    pub overage_enabled: bool,
}

pub async fn set_cap(State(app): State<AppState>, auth: Auth, Json(b): Json<CapBody>) -> ApiResult<Json<Value>> {
    let (_, plan) = plan_of(&app, &auth.account).await?;
    if b.overage_enabled && !plan.overage {
        return Err(ApiError::limit(format!(
            "{} doesn't include overage: work pauses at the allowance so it never becomes a bill. Choose Plus or above to opt in.",
            plan.name
        )));
    }
    let cap = match b.cap_dollars {
        Some(d) if !(0.0..=10_000.0).contains(&d) => return Err(ApiError::bad("Choose a cap between $0 and $10,000.")),
        Some(d) => Some(micros(d)),
        None => None,
    };
    let mut tx = app.db.begin().await?;
    sqlx::query("update accounts set spending_cap_micros = $2, overage_enabled = $3 where id = $1")
        .bind(auth.account.id)
        .bind(cap)
        .bind(b.overage_enabled)
        .execute(&mut *tx)
        .await?;
    crate::measure::audit(
        &mut *tx,
        auth.account.id,
        auth.actor_kind,
        Some(auth.user.id),
        "cap.changed",
        "",
        json!({ "cap_micros": cap, "overage": b.overage_enabled }),
    )
    .await?;
    let resumed = maybe_resume(&app, &mut tx, auth.account.id).await?;
    tx.commit().await?;
    if b.overage_enabled && !auth.account.overage_enabled {
        funnel(&app.db, Some(auth.account.id), "overage_enabled", json!({})).await;
    }
    if cap.unwrap_or(0) > auth.account.spending_cap_micros.unwrap_or(0) {
        funnel(&app.db, Some(auth.account.id), "cap_raised", json!({ "cap_micros": cap })).await;
    }
    Ok(Json(json!({ "ok": true, "resumed": resumed })))
}

#[derive(Deserialize)]
pub struct ChangePlan {
    pub plan: String,
    pub card: Option<CardInput>,
}

/// Change plan (or keep the trial plan). No award: awards are for sign-up only.
pub async fn change_plan(State(app): State<AppState>, auth: Auth, Json(b): Json<ChangePlan>) -> ApiResult<Json<Value>> {
    auth.require_ready()?;
    let catalog = app.catalog(auth.account.catalog_version).await?;
    let new = catalog.plan(&b.plan).cloned().ok_or_else(|| ApiError::bad("That plan doesn't exist."))?;
    let old_id = auth.account.plan.clone().unwrap_or_else(|| "free".into());
    if old_id == b.plan && auth.account.trial_plan.is_none() {
        return Err(ApiError::conflict(format!("You're already on {}.", new.name)));
    }
    let old = catalog.plan(&old_id).cloned().ok_or_else(|| ApiError::bad("Your plan is missing."))?;
    let now = app.now();
    let keeping_trial = auth.account.trial_plan.as_deref() == Some(b.plan.as_str());
    let mut tx = app.db.begin().await?;
    if new.requires_card {
        if let Some(card) = &b.card {
            attach_card(&app, &mut tx, &auth, card).await?;
        } else if auth.account.payment_customer.is_none() {
            return Err(ApiError::bad("Add a card to choose this plan.").with_code("needs_card"));
        }
        let account: Account =
            sqlx::query_as("select * from accounts where id = $1").bind(auth.account.id).fetch_one(&mut *tx).await?;
        let (_, paid) = charge_invoice(&app, &mut tx, &account, now, account.period_start.checked_add_months(Months::new(1)).expect("in range"),
            vec![json!({ "label": format!("{} plan from {}", new.name, now.format("%b %-d")), "amount_micros": micros(new.price_monthly) })]).await?;
        if !paid {
            return Err(ApiError::bad("The payment didn't go through. Try another card.").with_code("payment_failed"));
        }
    }
    sqlx::query("update accounts set plan = $2 where id = $1")
        .bind(auth.account.id)
        .bind(&b.plan)
        .execute(&mut *tx)
        .await?;
    if !new.overage {
        sqlx::query("update accounts set overage_enabled = false where id = $1")
            .bind(auth.account.id)
            .execute(&mut *tx)
            .await?;
    }
    // Top up the allowance if the new plan's is larger.
    let extra = micros(new.allowance - old.allowance);
    if extra > 0 {
        grant(
            &mut tx,
            auth.account.id,
            auth.account.period_start,
            now,
            "allowance",
            Bucket::Allowance,
            extra,
            &format!("{} allowance (changed plan)", new.name),
            (auth.actor_kind, Some(auth.user.id)),
        )
        .await?;
    }
    crate::measure::audit(
        &mut *tx,
        auth.account.id,
        auth.actor_kind,
        Some(auth.user.id),
        "plan.changed",
        &b.plan,
        json!({ "from": old_id }),
    )
    .await?;
    maybe_resume(&app, &mut tx, auth.account.id).await?;
    tx.commit().await?;
    if keeping_trial {
        end_trial(&app, auth.account.id, true).await?;
        funnel(&app.db, Some(auth.account.id), "trial_kept", json!({ "plan": b.plan })).await;
    } else {
        let kind = if new.order > old.order { "upgraded" } else { "downgraded" };
        funnel(&app.db, Some(auth.account.id), kind, json!({ "from": old_id, "to": b.plan })).await;
    }
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct PromoBody {
    pub code: String,
}

pub async fn redeem_promo(State(app): State<AppState>, auth: Auth, Json(b): Json<PromoBody>) -> ApiResult<Json<Value>> {
    let catalog = app.catalog(auth.account.catalog_version).await?;
    let code = b.code.trim().to_uppercase();
    let promo = catalog
        .promos
        .iter()
        .find(|p| p.code == code && p.ends_at.is_none_or(|e| e > app.now()))
        .cloned()
        .ok_or_else(|| ApiError::bad("That code isn't valid."))?;
    let mut tx = app.db.begin().await?;
    if !claim(&mut tx, &format!("promo:{code}"), &auth.account.id.to_string(), auth.account.id).await? {
        return Err(ApiError::conflict("You've already used this code."));
    }
    let now = app.now();
    grant(
        &mut tx,
        auth.account.id,
        auth.account.period_start,
        now,
        "credit",
        Bucket::Credit,
        micros(promo.credit),
        &format!("Promo code {code}"),
        (auth.actor_kind, Some(auth.user.id)),
    )
    .await?;
    maybe_resume(&app, &mut tx, auth.account.id).await?;
    tx.commit().await?;
    funnel(&app.db, Some(auth.account.id), "promo_redeemed", json!({ "code": code })).await;
    Ok(Json(json!({ "ok": true, "credit_micros": micros(promo.credit) })))
}

pub async fn invoice(State(app): State<AppState>, auth: Auth, Path(id): Path<Uuid>) -> ApiResult<Json<Value>> {
    let row: Option<(DateTime<Utc>, DateTime<Utc>, i64, String, Value, Option<String>, DateTime<Utc>)> = sqlx::query_as(
        "select period_start, period_end, total_micros, status, lines, provider_ref, created_at from invoices where id = $1 and account_id = $2",
    )
    .bind(id)
    .bind(auth.account.id)
    .fetch_optional(&app.db)
    .await?;
    let (s, e, total, status, lines, reference, created) = row.ok_or_else(|| ApiError::not_found("That invoice"))?;
    Ok(Json(json!({
        "id": id, "number": format!("CC-{}", &id.simple().to_string()[..8].to_uppercase()), "period_start": s, "period_end": e,
        "total_micros": total, "status": status, "lines": lines, "reference": reference, "created_at": created,
        "account": auth.account.name, "email": auth.user.email,
        "card": auth.account.card_last4.as_ref().map(|l| format!("{} ending {l}", auth.account.card_brand.clone().unwrap_or_default())),
    })))
}

/// Whether the account may start new work now.
pub async fn is_paused(db: &PgPool, account: Uuid) -> sqlx::Result<bool> {
    let (p,): (bool,) =
        sqlx::query_as("select paused_at is not null from accounts where id = $1").bind(account).fetch_one(db).await?;
    Ok(p)
}
