//! The plan catalog: every price, limit, award, trial and promo, as versioned data.
//! Nothing else in the code holds a price.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sqlx::PgPool;

pub const MICROS: f64 = 1_000_000.0;

pub fn micros(dollars: f64) -> i64 {
    (dollars * MICROS).round() as i64
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Catalog {
    #[serde(default)]
    pub note: String,
    pub currency: String,
    /// Compute, disk and outbound data are billed at provider cost plus this.
    pub markup: f64,
    pub disk_gb_month: f64,
    pub sizes: BTreeMap<String, Size>,
    pub ai_models: BTreeMap<String, AiRate>,
    pub plans: BTreeMap<String, Plan>,
    #[serde(default)]
    pub promos: Vec<Promo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Size {
    pub label: String,
    pub cpu: i32,
    pub memory_gb: i32,
    pub disk_gb: i32,
    pub provider_hourly: f64,
    #[serde(default)]
    pub suits: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AiRate {
    pub provider: String,
    pub input_per_mtok: f64,
    pub output_per_mtok: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Plan {
    pub name: String,
    pub order: i32,
    pub price_monthly: f64,
    pub requires_card: bool,
    pub award: f64,
    pub allowance: f64,
    pub computers: i64,
    pub awake_at_once: i64,
    pub largest_size: String,
    pub min_schedule_secs: i64,
    pub keep_awake: i64,
    pub overage: bool,
    pub history_days: i64,
    pub parallel_agents: i64,
    pub support: String,
    pub trial: Option<Trial>,
    /// One line on who the plan is for, shown on its card.
    #[serde(default)]
    pub tagline: String,
    /// The plan the sign-up highlights.
    #[serde(default)]
    pub recommended: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Trial {
    pub plan: String,
    pub days: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Promo {
    pub code: String,
    pub credit: f64,
    pub ends_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    pub note: String,
}

impl Catalog {
    pub fn plan(&self, id: &str) -> Option<&Plan> {
        self.plans.get(id)
    }

    pub fn size(&self, id: &str) -> Option<&Size> {
        self.sizes.get(id)
    }

    /// Sizes from smallest to largest by hourly cost.
    pub fn size_order(&self) -> Vec<&str> {
        let mut v: Vec<(&str, f64)> = self.sizes.iter().map(|(k, s)| (k.as_str(), s.provider_hourly)).collect();
        v.sort_by(|a, b| a.1.total_cmp(&b.1));
        v.into_iter().map(|(k, _)| k).collect()
    }

    pub fn size_allowed(&self, plan: &Plan, size: &str) -> bool {
        let order = self.size_order();
        match (order.iter().position(|s| *s == size), order.iter().position(|s| *s == plan.largest_size)) {
            (Some(a), Some(b)) => a <= b,
            _ => false,
        }
    }

    /// What a computer of this size costs per awake hour, in micros (with markup).
    pub fn awake_hourly_micros(&self, size: &str) -> i64 {
        self.size(size).map(|s| micros(s.provider_hourly * (1.0 + self.markup))).unwrap_or(0)
    }

    /// What a disk of this size costs per month, in micros (with markup).
    pub fn disk_monthly_micros(&self, gb: i32) -> i64 {
        micros(gb as f64 * self.disk_gb_month * (1.0 + self.markup))
    }

    /// Exact cost of AI tokens (no markup).
    pub fn ai_cost_micros(&self, model: &str, tokens_in: u32, tokens_out: u32) -> i64 {
        self.ai_models
            .get(model)
            .map(|r| {
                micros((tokens_in as f64 * r.input_per_mtok + tokens_out as f64 * r.output_per_mtok) / 1_000_000.0)
            })
            .unwrap_or(0)
    }

    /// Every problem with the catalog, in plain words. Empty means it can be saved.
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        if !(0.0..=5.0).contains(&self.markup) {
            out.push("The markup should be between 0 and 5 (0.25 is 25%).".into());
        }
        if self.sizes.is_empty() {
            out.push("There must be at least one size.".into());
        }
        for (id, s) in &self.sizes {
            if s.provider_hourly < 0.0 || s.cpu < 1 || s.memory_gb < 1 || s.disk_gb < 1 {
                out.push(format!("Size {id} needs positive CPU, memory, disk and cost."));
            }
        }
        for (id, p) in &self.plans {
            for (what, v) in [("price", p.price_monthly), ("award", p.award), ("allowance", p.allowance)] {
                if v < 0.0 {
                    out.push(format!("{}'s {what} can't be negative.", p.name));
                }
            }
            if !self.sizes.contains_key(&p.largest_size) {
                out.push(format!("{}'s largest size \"{}\" isn't a size.", p.name, p.largest_size));
            }
            if p.computers < 1 || p.awake_at_once < 1 || p.awake_at_once > p.computers {
                out.push(format!("{} needs at least one computer, and no more awake at once than it has.", p.name));
            }
            if p.min_schedule_secs < 60 {
                out.push(format!("{}'s most frequent schedule must be at least a minute.", p.name));
            }
            if p.price_monthly > 0.0 && !p.requires_card {
                out.push(format!("{} costs money, so it needs a card.", p.name));
            }
            if let Some(t) = &p.trial {
                if !self.plans.contains_key(&t.plan) || &t.plan == id {
                    out.push(format!("{}'s trial must be of another plan in the catalog.", p.name));
                }
                if !(1..=60).contains(&t.days) {
                    out.push(format!("{}'s trial must last 1 to 60 days.", p.name));
                }
            }
        }
        if !self.plans.contains_key("free") {
            out.push("There must be a free plan with the id \"free\".".into());
        }
        out
    }
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct CatalogVersion {
    pub id: i32,
    pub starts_at: chrono::DateTime<chrono::Utc>,
    pub note: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub fn seed() -> Catalog {
    serde_json::from_str(include_str!("../catalog/plans.json")).expect("the seed catalog is valid")
}

/// The newest catalog version that has started (what new accounts sign up on).
pub async fn current(db: &PgPool, now: chrono::DateTime<chrono::Utc>) -> anyhow::Result<(i32, Catalog)> {
    let (id, data): (i32, serde_json::Value) =
        sqlx::query_as("select id, data from catalog_versions where starts_at <= $1 order by id desc limit 1")
            .bind(now)
            .fetch_one(db)
            .await?;
    Ok((id, serde_json::from_value(data)?))
}

pub async fn version(db: &PgPool, id: i32) -> anyhow::Result<Catalog> {
    let (data,): (serde_json::Value,) =
        sqlx::query_as("select data from catalog_versions where id = $1").bind(id).fetch_one(db).await?;
    Ok(serde_json::from_value(data)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_catalog_is_valid_and_matches_the_product_doc() {
        let c = seed();
        assert!(c.problems().is_empty(), "{:?}", c.problems());
        let free = c.plan("free").unwrap();
        assert_eq!((free.price_monthly, free.award, free.allowance), (0.0, 2.0, 1.0));
        assert_eq!(free.trial.as_ref().unwrap().plan, "plus");
        assert_eq!(c.plan("max").unwrap().price_monthly, 60.0);
        assert!(c.plan("max").unwrap().trial.is_none());
    }

    #[test]
    fn sizes_are_ordered_and_limited_by_plan() {
        let c = seed();
        assert_eq!(c.size_order(), vec!["small", "medium", "large"]);
        assert!(c.size_allowed(c.plan("free").unwrap(), "small"));
        assert!(!c.size_allowed(c.plan("free").unwrap(), "medium"));
        assert!(c.size_allowed(c.plan("plus").unwrap(), "medium"));
        assert!(!c.size_allowed(c.plan("plus").unwrap(), "large"));
    }

    #[test]
    fn costs_include_the_markup_except_ai() {
        let c = seed();
        assert_eq!(c.awake_hourly_micros("small"), 7_500);
        assert_eq!(c.disk_monthly_micros(10), 1_875_000);
        assert_eq!(c.ai_cost_micros("mock-assistant", 1_000_000, 0), 3_000_000);
    }

    #[test]
    fn bad_catalogs_are_refused_in_plain_words() {
        let mut c = seed();
        c.plans.get_mut("plus").unwrap().largest_size = "huge".into();
        c.plans.get_mut("pro").unwrap().awake_at_once = 99;
        let p = c.problems();
        assert!(p.iter().any(|m| m.contains("huge")));
        assert!(p.iter().any(|m| m.contains("awake at once")));
    }
}
