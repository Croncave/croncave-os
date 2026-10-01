//! Market data for the Watcher's stock type. Keys for a real provider stay in the control
//! plane; computers ask through the platform SDK.

use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Quote {
    pub symbol: String,
    pub price: f64,
    pub currency: &'static str,
    pub at: chrono::DateTime<chrono::Utc>,
}

#[async_trait]
pub trait MarketData: Send + Sync {
    async fn quote(&self, symbol: &str, at: chrono::DateTime<chrono::Utc>) -> Result<Quote, String>;
}

/// A deterministic made-up price per symbol that drifts over time, plus nudges from
/// Dev tools so a demo can push a price across a threshold.
#[derive(Default)]
pub struct MockMarket {
    nudges: Mutex<HashMap<String, f64>>,
}

impl MockMarket {
    /// Move a symbol's price by a percentage from now on.
    pub fn nudge(&self, symbol: &str, percent: f64) {
        let mut n = self.nudges.lock().expect("nudges");
        let f = n.entry(symbol.to_uppercase()).or_insert(1.0);
        *f *= 1.0 + percent / 100.0;
    }

    pub fn base_price(symbol: &str, minute: i64) -> f64 {
        let h = symbol.bytes().fold(7u64, |a, b| a.wrapping_mul(31).wrapping_add(b as u64));
        let base = 40.0 + (h % 260) as f64;
        let phase = (h % 100) as f64;
        let t = minute as f64;
        base * (1.0 + 0.06 * ((t / 90.0) + phase).sin() + 0.02 * ((t / 13.0) + 2.0 * phase).sin())
    }
}

#[async_trait]
impl MarketData for MockMarket {
    async fn quote(&self, symbol: &str, at: chrono::DateTime<chrono::Utc>) -> Result<Quote, String> {
        let symbol = symbol.trim().to_uppercase();
        if symbol.is_empty() || symbol.len() > 6 || !symbol.chars().all(|c| c.is_ascii_alphabetic() || c == '.') {
            return Err(format!("\"{symbol}\" isn't a stock symbol."));
        }
        let nudge = self.nudges.lock().expect("nudges").get(&symbol).copied().unwrap_or(1.0);
        let price = (Self::base_price(&symbol, at.timestamp() / 60) * nudge * 100.0).round() / 100.0;
        Ok(Quote { symbol, price, currency: "USD", at })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn prices_are_stable_within_a_minute_and_nudges_move_them() {
        let m = MockMarket::default();
        let t = chrono::Utc::now();
        let a = m.quote("acme", t).await.unwrap();
        assert_eq!(a.symbol, "ACME");
        assert_eq!(a.price, m.quote("ACME", t).await.unwrap().price);
        m.nudge("ACME", -50.0);
        let b = m.quote("ACME", t).await.unwrap();
        assert!((b.price - a.price / 2.0).abs() < 0.02);
        assert!(m.quote("NOT A SYMBOL", t).await.is_err());
    }
}
