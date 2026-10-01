//! Interfaces to everything that needs the founder's accounts, each with the
//! implementation chosen in `.env`.

pub mod ai;
pub mod compute;
pub mod market;
pub mod notifier;
pub mod payments;

use std::sync::Arc;

use sqlx::PgPool;

use crate::config::Config;

pub struct Providers {
    pub compute: Arc<dyn compute::ComputeDriver>,
    pub payments: Arc<dyn payments::PaymentProvider>,
    pub notifier: Arc<dyn notifier::Notifier>,
    pub ai: Arc<dyn ai::AiModel>,
    pub market: Arc<dyn market::MarketData>,
    /// Present when the mock market is in use, so Dev tools can nudge prices.
    pub mock_market: Option<Arc<market::MockMarket>>,
}

impl Providers {
    pub fn from_config(cfg: &Config, db: PgPool) -> anyhow::Result<Self> {
        let unknown = |what: &str, v: &str| anyhow::anyhow!("{what}={v} has no implementation yet (use mock)");
        let payments: Arc<dyn payments::PaymentProvider> = match cfg.payments.as_str() {
            "mock" => Arc::new(payments::MockPayments::default()),
            v => return Err(unknown("PAYMENTS", v)),
        };
        let notifier: Arc<dyn notifier::Notifier> = match cfg.notifier.as_str() {
            "outbox" => Arc::new(notifier::OutboxNotifier::new(db)),
            v => return Err(unknown("NOTIFIER", v)),
        };
        let ai: Arc<dyn ai::AiModel> = match cfg.ai.as_str() {
            "mock" => Arc::new(ai::MockModel),
            v => return Err(unknown("AI", v)),
        };
        let (market, mock_market): (Arc<dyn market::MarketData>, _) = match cfg.market_data.as_str() {
            "mock" => {
                let m = Arc::new(market::MockMarket::default());
                (m.clone(), Some(m))
            }
            v => return Err(unknown("MARKET_DATA", v)),
        };
        Ok(Self { compute: compute::from_config(cfg)?, payments, notifier, ai, market, mock_market })
    }
}
