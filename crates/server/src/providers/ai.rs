//! AI models behind one interface. The gateway (`crate::assistant`) routes features to a
//! model, meters tokens, checks the spending cap and honours kill switches. The `mock`
//! model returns canned but plausible answers and reports token counts like a real one.

use async_trait::async_trait;
use serde_json::{Value, json};

#[derive(Debug, Clone)]
pub struct AiRequest {
    pub feature: String,
    pub system: String,
    /// (role, text), oldest first.
    pub messages: Vec<(String, String)>,
    /// What the person is looking at (app, computer, job), for context.
    pub context: Value,
}

#[derive(Debug, Clone)]
pub struct AiResponse {
    pub model: String,
    pub text: String,
    pub tokens_in: u32,
    pub tokens_out: u32,
}

#[async_trait]
pub trait AiModel: Send + Sync {
    fn id(&self) -> &str;
    fn provider(&self) -> &str;
    async fn complete(&self, req: &AiRequest) -> anyhow::Result<AiResponse>;
}

pub struct MockModel;

fn tokens(s: &str) -> u32 {
    (s.chars().count() as u32).div_ceil(4).max(1)
}

#[async_trait]
impl AiModel for MockModel {
    fn id(&self) -> &str {
        "mock-assistant"
    }
    fn provider(&self) -> &str {
        "mock"
    }

    async fn complete(&self, req: &AiRequest) -> anyhow::Result<AiResponse> {
        let last = req.messages.last().map(|(_, t)| t.clone()).unwrap_or_default();
        let answer = canned(&last, &req.context);
        let text = answer.to_string();
        let input: String =
            std::iter::once(req.system.as_str()).chain(req.messages.iter().map(|(_, t)| t.as_str())).collect();
        Ok(AiResponse { model: self.id().into(), tokens_in: tokens(&input) + 40, tokens_out: tokens(&text), text })
    }
}

/// The mock assistant's answers: a reply and, where it fits, proposals the person can
/// apply. Proposals are the same setups the forms make.
fn canned(message: &str, context: &Value) -> Value {
    let m = message.to_lowercase();
    let number = |after: &str| -> Option<String> {
        let idx = m.find(after)? + after.len();
        let digits: String = m[idx..]
            .chars()
            .skip_while(|c| !c.is_ascii_digit())
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .collect();
        (!digits.is_empty()).then_some(digits)
    };
    if m.contains("apartment") || m.contains("listing") || m.contains("rent") {
        let price = number("under").or_else(|| number("$")).unwrap_or_else(|| "2200".into());
        let words: Vec<&str> = m.split_whitespace().collect();
        let beds = words
            .windows(2)
            .find(|w| w[1].starts_with("bed"))
            .map(|w| w[0].chars().filter(|c| c.is_ascii_digit()).collect::<String>())
            .filter(|b| !b.is_empty())
            .unwrap_or_else(|| "1".into());
        return json!({
            "reply": format!("I can watch the listings site for new apartments up to ${price} a month with at least {beds} bedroom(s), every hour. Here's the setup — nothing starts until you apply it."),
            "proposals": [{
                "kind": "create_watch",
                "title": format!("Watch apartments up to ${price}"),
                "type_id": "demo-listings",
                "name": format!("Apartments up to ${price}"),
                "inputs": { "max_price": price, "min_beds": beds },
                "schedule": "0 0 * * * *"
            }]
        });
    }
    if m.contains("stock") || m.contains("price of") || m.contains("share") {
        let symbol = message
            .split_whitespace()
            .find(|w| w.len() >= 2 && w.len() <= 5 && w.chars().all(|c| c.is_ascii_uppercase()))
            .unwrap_or("ACME")
            .to_string();
        let target =
            number("below").or_else(|| number("under")).or_else(|| number("above")).unwrap_or_else(|| "100".into());
        let direction = if m.contains("above") || m.contains("over") { "above" } else { "below" };
        return json!({
            "reply": format!("I'll check {symbol} every hour and tell you when it goes {direction} ${target}."),
            "proposals": [{
                "kind": "create_watch",
                "title": format!("Watch {symbol} {direction} ${target}"),
                "type_id": "stock-price",
                "name": format!("{symbol} {direction} ${target}"),
                "inputs": { "symbol": symbol, "direction": direction, "target": target },
                "schedule": "0 0 * * * *"
            }]
        });
    }
    if m.contains("why") && (m.contains("fail") || m.contains("error")) {
        return json!({
            "reply": "Runs usually fail for one of three reasons: a missing package (add it to requirements.txt), a file that isn't where the script expects, or a time limit. Open the failed run: the reason is shown in plain words with the fix beside it.",
            "proposals": []
        });
    }
    let app = context.get("app").and_then(Value::as_str).unwrap_or("home");
    json!({
        "reply": format!("I'm the assistant (a mock in this prototype). From {app} I can set up watches for you: try \"watch apartments under $2000 with 2 beds\" or \"tell me when ACME goes below $90\". I'll show the setup first; nothing runs until you apply it."),
        "proposals": []
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_proposes_watches_and_meters_tokens() {
        let req = AiRequest {
            feature: "assistant".into(),
            system: "You help.".into(),
            messages: vec![("user".into(), "Watch apartments under $1800 with 2 beds".into())],
            context: json!({}),
        };
        let r = MockModel.complete(&req).await.unwrap();
        let v: Value = serde_json::from_str(&r.text).unwrap();
        assert_eq!(v["proposals"][0]["type_id"], "demo-listings");
        assert_eq!(v["proposals"][0]["inputs"]["max_price"], "1800");
        assert_eq!(v["proposals"][0]["inputs"]["min_beds"], "2");
        assert!(r.tokens_in > 0 && r.tokens_out > 0);

        let req = AiRequest { messages: vec![("user".into(), "tell me when ACME stock goes below $90".into())], ..req };
        let v: Value = serde_json::from_str(&MockModel.complete(&req).await.unwrap().text).unwrap();
        assert_eq!(v["proposals"][0]["inputs"]["symbol"], "ACME");
        assert_eq!(v["proposals"][0]["inputs"]["target"], "90");
    }
}
