//! Build order step 10's check: usage draws from the award, then the allowance, then
//! overage; work pauses at the cap and resumes when it's raised; a trial ends with no
//! charge; months roll over with an invoice.

mod common;

use serde_json::json;

const D: i64 = 1_000_000;

async fn balances(s: &common::Stack) -> serde_json::Value {
    s.get("/billing").await
}

#[tokio::test]
async fn award_then_allowance_then_overage_and_the_cap() {
    let s = common::stack().await;
    s.sign_up("pat@example.com", "4155550101", "plus", Some("4242424242424242"), false).await;
    let b = balances(&s).await;
    assert_eq!(b["balances"]["award"], 5 * D, "Plus comes with a $5 award");
    assert_eq!(b["balances"]["allowance"], 3 * D);
    assert_eq!(b["invoices"].as_array().unwrap().len(), 1, "the first month is charged at sign-up");
    assert_eq!(b["invoices"][0]["total_micros"], 10 * D);

    // $6 of usage: $5 from the award, $1 from the allowance.
    s.post("/dev/usage", json!({ "dollars": 6.0 })).await;
    let b = balances(&s).await;
    assert_eq!(b["balances"]["award"], 0);
    assert_eq!(b["balances"]["allowance"], 2 * D);
    assert!(b["paused"].is_null());

    // $3 more: $2 left in the allowance, the rest can't be drawn without overage: pause.
    s.post("/dev/usage", json!({ "dollars": 3.0 })).await;
    let b = balances(&s).await;
    assert_eq!(b["balances"]["allowance"], 0);
    assert!(b["paused"].is_string(), "work pauses at the cap");
    let events = s.get("/events").await;
    assert!(events["events"].as_array().unwrap().iter().any(|e| e["kind"] == "cap.reached"));

    // Opt in to overage with a $10 cap: work resumes, and more usage draws overage.
    let r = s.post("/billing/cap", json!({ "cap_dollars": 10.0, "overage_enabled": true })).await;
    assert_eq!(r["resumed"], true);
    s.post("/dev/usage", json!({ "dollars": 2.0 })).await;
    let b = balances(&s).await;
    assert!(b["paused"].is_null());
    assert_eq!(b["balances"]["overage"], 2 * D, "usage past a cap is never billed later; only the new $2 is overage");
}

#[tokio::test]
async fn free_trial_ends_with_no_charge_and_awards_are_once_per_phone() {
    let s = common::stack().await;
    let chosen = s.sign_up("lee@example.com", "4155550102", "free", None, true).await;
    assert_eq!(chosen["award_micros"], 2 * D);
    assert_eq!(chosen["trial"]["plan"], "plus");
    let me = s.get("/me").await;
    assert_eq!(me["account"]["trial_plan"], "plus");
    assert_eq!(me["usage"]["plan_name"], "Plus", "trial limits apply during the trial");

    // Fifteen days later the trial is over: back on Free, nothing charged.
    s.post("/dev/clock", json!({ "secs": 15 * 86400 })).await;
    s.wait_for("/me", "the trial to end", |v| v["account"]["trial_plan"].is_null()).await;
    let b = balances(&s).await;
    assert_eq!(b["invoices"].as_array().unwrap().len(), 0, "a trial never charges by itself");
    assert_eq!(b["balances"]["trial"], 0, "the trial allowance expires");
    let events = s.get("/events").await;
    assert!(events["events"].as_array().unwrap().iter().any(|e| e["kind"] == "trial.ended"));

    // The same phone signing up again gets no award and no trial.
    s.post("/auth/signout", json!({})).await;
    let again = s.sign_up("lee2@example.com", "(415) 555-0102", "free", None, false).await;
    assert_eq!(again["award_micros"], 0);
    assert!(again["trial"].is_null());
}

#[tokio::test]
async fn a_month_rolls_over_with_an_invoice_and_a_new_allowance() {
    let s = common::stack().await;
    s.sign_up("max@example.com", "4155550103", "plus", Some("5555555555554444"), false).await;
    s.post("/billing/cap", json!({ "cap_dollars": 20.0, "overage_enabled": true })).await;
    s.post("/dev/usage", json!({ "dollars": 9.0 })).await; // $5 award, $3 allowance, $1 overage
    s.post("/dev/clock", json!({ "secs": 32 * 86400 })).await;
    let b = s.wait_for("/billing", "the month to close", |v| v["invoices"].as_array().unwrap().len() == 2).await;
    let inv = &b["invoices"][0];
    assert_eq!(inv["total_micros"], 11 * D, "next month's $10 plus $1 of overage");
    assert_eq!(b["balances"]["allowance"], 3 * D, "a fresh allowance");
    assert_eq!(b["balances"]["overage"], 0);
}

#[tokio::test]
async fn declined_cards_and_free_overage_are_refused_plainly() {
    let s = common::stack().await;
    s.post("/auth/email", json!({ "email": "dee@example.com" })).await;
    let (code, _) = s.post_err("/signup/plan", json!({ "plan": "plus" })).await;
    assert_eq!(code, 401, "signed out");
    s.sign_up("dee@example.com", "4155550104", "free", None, false).await;
    let (code, err) = s.post_err("/billing/cap", json!({ "cap_dollars": 5.0, "overage_enabled": true })).await;
    assert_eq!(code, 402);
    assert!(err["error"].as_str().unwrap().contains("never becomes a bill"));
    let card = json!({ "number": "4000000000000002", "exp_month": 12, "exp_year": 2099, "cvc": "123" });
    let (code, err) = s.post_err("/billing/plan", json!({ "plan": "plus", "card": card })).await;
    assert_eq!(code, 400);
    assert_eq!(err["error"], "Your card was declined.");
}
