//! Schedules follow the computer: its time zone sets the clock "every day at 9 AM" is read
//! on, and a computer told not to wake for scheduled work holds the run instead.

mod common;

use chrono::{DateTime, Timelike, Utc};
use chrono_tz::America::Los_Angeles;
use serde_json::json;

#[tokio::test]
async fn changing_a_computers_time_zone_moves_its_jobs_to_that_clock() {
    let s = common::stack().await;
    s.sign_up("tz@example.com", "4155550131", "free", None, false).await;
    let c = s.post("/computers", json!({ "name": "Clock", "size": "small" })).await;
    let c = c["id"].as_str().unwrap().to_string();
    assert_eq!(s.get(&format!("/computers/{c}")).await["time_zone"], "America/New_York");
    s.post(&format!("/computers/{c}/scripts/template"), json!({ "template": "csv-report" })).await;
    let made = s
        .post(
            &format!("/computers/{c}/scripts"),
            json!({ "name": "Morning", "path": "Scripts/csv-report/report.py", "trigger": "schedule", "schedule": "0 9 * * *" }),
        )
        .await;
    let job = made["job"]["id"].as_str().unwrap().to_string();
    assert_eq!(made["job"]["schedule_words"], "Every day at 9:00 am ET");

    s.patch(&format!("/computers/{c}"), json!({ "time_zone": "America/Los_Angeles" })).await;
    let j = s.get(&format!("/jobs/{job}")).await;
    assert_eq!(j["schedule_words"], "Every day at 9:00 am PT");
    let next: DateTime<Utc> = j["next_due_at"].as_str().unwrap().parse().unwrap();
    let there = next.with_timezone(&Los_Angeles);
    assert_eq!((there.hour(), there.minute()), (9, 0), "next run is 9 AM in Los Angeles, got {there}");
}

#[tokio::test]
async fn a_computer_that_doesnt_wake_for_schedules_holds_the_run() {
    let s = common::stack().await;
    s.sign_up("nowake@example.com", "4155550132", "free", None, false).await;
    let c = s.post("/computers", json!({ "name": "Lazy", "size": "small", "wake_for_schedule": false })).await;
    let id: uuid::Uuid = c["id"].as_str().unwrap().parse().unwrap();
    s.post(&format!("/computers/{id}/scripts/template"), json!({ "template": "backup" })).await;
    let made = s
        .post(
            &format!("/computers/{id}/scripts"),
            json!({ "name": "Hourly", "path": "Scripts/backup/backup.sh", "trigger": "schedule", "schedule": "0 * * * *" }),
        )
        .await;
    let job: uuid::Uuid = made["job"]["id"].as_str().unwrap().parse().unwrap();
    // Let the computer fall asleep, then make the slot due.
    sqlx::query("update computers set state = 'asleep', connected = false, wake_requested_at = null where id = $1")
        .bind(id)
        .execute(&s.app.db)
        .await
        .unwrap();
    sqlx::query("update jobs set next_due_at = now() - interval '1 second' where id = $1")
        .bind(job)
        .execute(&s.app.db)
        .await
        .unwrap();
    croncave_server::jobs::schedule_due(&s.app).await.unwrap();
    croncave_server::jobs::dispatch(&s.app).await.unwrap();

    let (status, note): (String, Option<String>) =
        sqlx::query_as("select status, status_note from runs where job_id = $1 and trigger = 'schedule'")
            .bind(job)
            .fetch_one(&s.app.db)
            .await
            .unwrap();
    assert_eq!(status, "waiting");
    assert!(note.unwrap_or_default().contains("doesn't wake for scheduled work"));
    let (wake,): (Option<DateTime<Utc>>,) = sqlx::query_as("select wake_requested_at from computers where id = $1")
        .bind(id)
        .fetch_one(&s.app.db)
        .await
        .unwrap();
    assert!(wake.is_none(), "the computer was not asked to wake");
}

#[tokio::test]
async fn a_window_keeps_scheduled_runs_inside_part_of_the_day() {
    let s = common::stack().await;
    s.sign_up("window@example.com", "4155550133", "free", None, false).await;
    let c = s.post("/computers", json!({ "name": "Hours", "size": "small" })).await;
    let c = c["id"].as_str().unwrap().to_string();
    s.post(&format!("/computers/{c}/scripts/template"), json!({ "template": "csv-report" })).await;
    let made = s
        .post(
            &format!("/computers/{c}/scripts"),
            json!({ "name": "Daytime", "path": "Scripts/csv-report/report.py", "trigger": "schedule",
                    "schedule": "0 */3 * * *", "window": { "start_min": 360, "end_min": 1440 } }),
        )
        .await;
    assert_eq!(made["job"]["schedule_words"], "Every 3 hours, from 6 AM to midnight ET");
    let job = made["job"]["id"].as_str().unwrap().to_string();
    let next: DateTime<Utc> = made["job"]["next_due_at"].as_str().unwrap().parse().unwrap();
    let there = next.with_timezone(&chrono_tz::America::New_York);
    assert!(there.hour() >= 6, "the next run is inside the window, got {there}");

    // Clearing the window brings the night slots back.
    let j = s.patch(&format!("/jobs/{job}"), json!({ "window": {} })).await;
    assert_eq!(j["schedule_words"], "Every 3 hours");
}

#[tokio::test]
async fn a_draft_can_be_test_run_and_asked_to_tell_me() {
    let s = common::stack().await;
    s.sign_up("draft@example.com", "4155550134", "free", None, false).await;
    let c = s.post("/computers", json!({ "name": "Drafts", "size": "small" })).await;
    let c = c["id"].as_str().unwrap().to_string();
    s.post(&format!("/computers/{c}/scripts/template"), json!({ "template": "csv-report" })).await;
    let made = s
        .post(
            &format!("/computers/{c}/scripts"),
            json!({ "name": "Try", "path": "Scripts/csv-report/report.py", "status": "draft", "test_now": true }),
        )
        .await;
    assert_eq!(made["job"]["status"], "draft");
    let run = made["run_id"].as_str().expect("a test run started").to_string();
    let r = s.get(&format!("/runs/{run}")).await;
    assert_eq!(r["trigger"], "test");
    s.post(&format!("/runs/{run}/tell-me"), json!({ "on": true })).await;
    let (tell,): (bool,) = sqlx::query_as("select tell_me from runs where id = $1")
        .bind(run.parse::<uuid::Uuid>().unwrap())
        .fetch_one(&s.app.db)
        .await
        .unwrap();
    assert!(tell);
}
