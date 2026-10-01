//! Rows the control plane reads often.

use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub phone: Option<String>,
    pub phone_verified_at: Option<DateTime<Utc>>,
    pub is_admin: bool,
    pub prefs: Value,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub previous_seen_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub time_zone: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Account {
    pub id: Uuid,
    pub name: String,
    pub catalog_version: i32,
    pub plan: Option<String>,
    pub trial_plan: Option<String>,
    pub trial_started_at: Option<DateTime<Utc>>,
    pub trial_ends_at: Option<DateTime<Utc>>,
    pub period_start: DateTime<Utc>,
    pub spending_cap_micros: Option<i64>,
    pub overage_enabled: bool,
    pub ai_enabled: bool,
    pub paused_at: Option<DateTime<Utc>>,
    pub cap_alert_level: i32,
    pub card_brand: Option<String>,
    pub card_last4: Option<String>,
    #[serde(skip)]
    pub card_fingerprint: Option<String>,
    #[serde(skip)]
    pub payment_customer: Option<String>,
    pub signup_completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

impl Account {
    /// The plan whose limits apply now: the trial plan during a trial.
    pub fn effective_plan(&self) -> &str {
        match (&self.trial_plan, &self.plan) {
            (Some(t), _) => t,
            (None, Some(p)) => p,
            (None, None) => "free",
        }
    }
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Computer {
    pub id: Uuid,
    pub account_id: Uuid,
    pub name: String,
    pub size: String,
    pub cpu: i32,
    pub memory_gb: i32,
    pub disk_gb: i32,
    pub state: String,
    pub state_changed_at: DateTime<Utc>,
    pub sleep_delay_secs: i32,
    pub keep_awake: bool,
    #[serde(skip)]
    pub compute_ref: Option<String>,
    pub last_active_at: DateTime<Utc>,
    pub wake_requested_at: Option<DateTime<Utc>>,
    pub wake_cause: Option<String>,
    pub connected: bool,
    pub health: Value,
    pub agent_version: Option<String>,
    pub note: Option<String>,
    pub created_at: DateTime<Utc>,
    pub time_zone: String,
    pub wake_for_schedule: bool,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Job {
    pub id: Uuid,
    pub account_id: Uuid,
    pub computer_id: Uuid,
    pub app: String,
    pub kind: String,
    pub name: String,
    pub setup: Value,
    pub trigger: String,
    pub schedule: Option<String>,
    pub watch_path: Option<String>,
    pub overlap: String,
    pub max_runtime_secs: i32,
    pub retries: i32,
    pub max_spend_micros: Option<i64>,
    pub notify: Value,
    pub status: String,
    pub next_due_at: Option<DateTime<Utc>>,
    pub created_by_kind: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub time_zone: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct Run {
    pub id: Uuid,
    pub job_id: Uuid,
    pub account_id: Uuid,
    pub computer_id: Uuid,
    pub app: String,
    pub kind: String,
    pub trigger: String,
    pub slot_at: Option<DateTime<Utc>>,
    pub started_by_kind: String,
    pub parent_run_id: Option<Uuid>,
    pub attempt: i32,
    pub status: String,
    pub status_note: Option<String>,
    pub queued_at: DateTime<Utc>,
    pub dispatched_at: Option<DateTime<Utc>>,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub exit_code: Option<i32>,
    pub headline: Option<String>,
    pub summary: Option<Value>,
    pub output_tail: Option<String>,
    pub error_plain: Option<String>,
    pub error_fix: Option<String>,
    pub data: Option<Value>,
    pub changes: Option<Value>,
    pub progress: Option<Value>,
    pub awake_seconds: i32,
    pub ai_cost_micros: i64,
}

pub const ACTIVE_RUN_STATES: &[&str] = &["queued", "waiting", "starting", "running"];
