//! Build order step 5's check: a job runs exactly once per slot, across two control
//! planes racing and across a restart, and missed slots run once.

mod common;

use chrono::{Duration, Utc};
use croncave_server::config::Config;
use uuid::Uuid;

async fn seed_job(db: &sqlx::PgPool, schedule: &str, due: chrono::DateTime<Utc>) -> Uuid {
    let (version,): (i32,) = sqlx::query_as("select id from catalog_versions limit 1").fetch_one(db).await.unwrap();
    let account = Uuid::new_v4();
    let computer = Uuid::new_v4();
    let job = Uuid::new_v4();
    sqlx::query("insert into accounts (id, name, catalog_version, plan, period_start) values ($1, 'a', $2, 'pro', now())")
        .bind(account)
        .bind(version)
        .execute(db)
        .await
        .unwrap();
    sqlx::query("insert into computers (id, account_id, name, size, cpu, memory_gb, disk_gb, state) values ($1, $2, 'c', 'small', 1, 1, 10, 'asleep')")
        .bind(computer)
        .bind(account)
        .execute(db)
        .await
        .unwrap();
    sqlx::query(
        "insert into jobs (id, account_id, computer_id, app, kind, name, setup, trigger, schedule, status, next_due_at, created_by_kind)
         values ($1, $2, $3, 'scripts', 'script', 'j', '{}', 'schedule', $4, 'active', $5, 'user')",
    )
    .bind(job)
    .bind(account)
    .bind(computer)
    .bind(schedule)
    .bind(due)
    .execute(db)
    .await
    .unwrap();
    job
}

async fn runs_for(db: &sqlx::PgPool, job: Uuid) -> Vec<Option<chrono::DateTime<Utc>>> {
    sqlx::query_scalar("select slot_at from runs where job_id = $1 order by slot_at").bind(job).fetch_all(db).await.unwrap()
}

#[tokio::test]
async fn each_slot_runs_once_across_racing_control_planes_and_a_restart() {
    let url = common::fresh_database().await;
    let dir = tempfile::tempdir().unwrap();
    // Two control planes on one database, as during a rolling deploy.
    let a = croncave_server::build(Config::for_tests(&url, dir.path().into())).await.unwrap();
    let b = croncave_server::build(Config::for_tests(&url, dir.path().into())).await.unwrap();
    // Postgres keeps microseconds.
    let slot = chrono::DateTime::from_timestamp_micros((Utc::now() - Duration::seconds(5)).timestamp_micros()).unwrap();
    let job = seed_job(&a.db, "0 * * * * *", slot).await;

    let (ra, rb) = tokio::join!(croncave_server::jobs::schedule_due(&a), croncave_server::jobs::schedule_due(&b));
    assert_eq!(ra.unwrap() + rb.unwrap(), 1, "exactly one of the two queued the slot");
    assert_eq!(runs_for(&a.db, job).await, vec![Some(slot)]);

    // Pretend the scheduler crashed after queuing but before moving next_due_at.
    sqlx::query("update jobs set next_due_at = $2 where id = $1").bind(job).bind(slot).execute(&a.db).await.unwrap();
    drop((a, b));
    let restarted = croncave_server::build(Config::for_tests(&url, dir.path().into())).await.unwrap();
    croncave_server::jobs::schedule_due(&restarted).await.unwrap();
    assert_eq!(runs_for(&restarted.db, job).await.len(), 1, "a restart doesn't run the same slot again");

    // Ten minutes of missed slots run once, not ten times.
    sqlx::query("update jobs set next_due_at = $2 where id = $1").bind(job).bind(Utc::now() - Duration::minutes(10)).execute(&restarted.db).await.unwrap();
    croncave_server::jobs::schedule_due(&restarted).await.unwrap();
    assert_eq!(runs_for(&restarted.db, job).await.len(), 2);
    let (next,): (chrono::DateTime<Utc>,) = sqlx::query_as("select next_due_at from jobs where id = $1").bind(job).fetch_one(&restarted.db).await.unwrap();
    assert!(next > Utc::now(), "the next slot is in the future");
}
