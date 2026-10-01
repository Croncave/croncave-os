//! Test helpers: a fresh Postgres database per test, and a full control plane on random
//! ports with computers run by the local driver.

#![allow(dead_code)]

use std::net::SocketAddr;
use std::time::Duration;

use croncave_server::AppState;
use croncave_server::config::Config;
use serde_json::{Value, json};
use sqlx::Connection;
use uuid::Uuid;

/// A new, empty database (migrated by the app when it starts).
pub async fn fresh_database() -> String {
    let base = std::env::var("TEST_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .expect("set TEST_DATABASE_URL (scripts/check.sh does) to run database tests");
    let name = format!("cc_test_{}", Uuid::new_v4().simple());
    let admin = replace_db(&base, "postgres");
    let mut conn = sqlx::PgConnection::connect(&admin).await.expect("connect to Postgres");
    sqlx::query(sqlx::AssertSqlSafe(format!("create database {name}")))
        .execute(&mut conn)
        .await
        .expect("create test database");
    replace_db(&base, &name)
}

fn replace_db(url: &str, db: &str) -> String {
    let (head, _) = url.rsplit_once('/').expect("database url has a path");
    format!("{head}/{db}")
}

pub struct Stack {
    pub app: AppState,
    pub base: String,
    pub client: reqwest::Client,
    pub data: tempfile::TempDir,
}

pub fn agent_bin() -> std::path::PathBuf {
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/croncave-agent");
    assert!(p.exists(), "build the agent first: cargo build -p croncave-agent");
    p
}

/// A whole control plane (API, relay, workers) on a random port.
pub async fn stack() -> Stack {
    let db = fresh_database().await;
    let data = tempfile::tempdir().expect("temp dir");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr: SocketAddr = listener.local_addr().expect("addr");
    let mut cfg = Config::for_tests(&db, data.path().to_path_buf());
    cfg.relay_url = format!("http://{addr}");
    cfg.agent_bin = agent_bin();
    let app = croncave_server::build(cfg).await.expect("build app");
    croncave_server::spawn_workers(&app);
    let router = croncave_server::api::router(app.clone());
    tokio::spawn(async move { axum::serve(listener, router).await.expect("serve") });
    let client = reqwest::Client::builder().cookie_store(true).no_proxy().build().expect("client");
    Stack { app, base: format!("http://{addr}"), client, data }
}

impl Stack {
    pub async fn post(&self, path: &str, body: Value) -> Value {
        let r = self.client.post(format!("{}/api{path}", self.base)).json(&body).send().await.expect("request");
        let status = r.status();
        let v: Value = r.json().await.unwrap_or(Value::Null);
        assert!(status.is_success(), "POST {path} -> {status}: {v}");
        v
    }

    pub async fn post_err(&self, path: &str, body: Value) -> (u16, Value) {
        let r = self.client.post(format!("{}/api{path}", self.base)).json(&body).send().await.expect("request");
        (r.status().as_u16(), r.json().await.unwrap_or(Value::Null))
    }

    pub async fn get(&self, path: &str) -> Value {
        let r = self.client.get(format!("{}/api{path}", self.base)).send().await.expect("request");
        let status = r.status();
        let v: Value = r.json().await.unwrap_or(Value::Null);
        assert!(status.is_success(), "GET {path} -> {status}: {v}");
        v
    }

    pub async fn put_bytes(&self, path: &str, body: Vec<u8>) -> Value {
        let r = self.client.put(format!("{}/api{path}", self.base)).body(body).send().await.expect("request");
        let status = r.status();
        let v: Value = r.json().await.unwrap_or(Value::Null);
        assert!(status.is_success(), "PUT {path} -> {status}: {v}");
        v
    }

    async fn latest_outbox(&self, channel: &str) -> Value {
        let o = self.get("/dev/outbox").await;
        o["messages"].as_array().unwrap().iter().find(|m| m["channel"] == channel).cloned().expect("a message")
    }

    /// Sign up through the real flow (link, phone code, plan, trial answer).
    pub async fn sign_up(&self, email: &str, phone: &str, plan: &str, card: Option<&str>, trial: bool) -> Value {
        self.post("/auth/email", json!({ "email": email })).await;
        let link = self.latest_outbox("email").await["link"].as_str().unwrap().to_string();
        let token = link.rsplit_once("token=").unwrap().1.to_string();
        self.post("/auth/verify", json!({ "token": token })).await;
        self.post("/auth/phone", json!({ "phone": phone })).await;
        let body = self.latest_outbox("sms").await["body"].as_str().unwrap().to_string();
        let code: String = body.chars().filter(|c| c.is_ascii_digit()).take(6).collect();
        self.post("/auth/phone/verify", json!({ "code": code })).await;
        let mut choose = json!({ "plan": plan });
        if let Some(n) = card {
            choose["card"] = json!({ "number": n, "exp_month": 12, "exp_year": 2099, "cvc": "123", "zip": "94110" });
        }
        let chosen = self.post("/signup/plan", choose).await;
        self.post("/signup/trial", json!({ "accept": trial })).await;
        chosen
    }

    pub async fn wait_for<F: Fn(&Value) -> bool>(&self, path: &str, what: &str, f: F) -> Value {
        for _ in 0..200 {
            let v = self.get(path).await;
            if f(&v) {
                return v;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        panic!("timed out waiting for {what} at {path}: {}", self.get(path).await);
    }

    pub async fn wait_run(&self, run: &str) -> Value {
        self.wait_for(&format!("/runs/{run}"), "the run to end", |v| {
            v["ended_at"].is_string() && v["status"] != "running"
        })
        .await
    }
}

impl Drop for Stack {
    fn drop(&mut self) {
        // Stop any agent processes this test started.
        let dir = self.data.path().join("computers");
        if let Ok(rd) = std::fs::read_dir(dir) {
            for e in rd.flatten() {
                if let Ok(pid) = std::fs::read_to_string(e.path().join("agent.pid")) {
                    let _ = std::process::Command::new("kill")
                        .args(["-s", "KILL", "--", &format!("-{}", pid.trim())])
                        .status();
                }
            }
        }
    }
}
