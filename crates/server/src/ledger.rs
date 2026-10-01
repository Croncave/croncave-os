//! One append-only balance ledger per account.
//!
//! Grants (award, trial allowance, monthly allowance, credits) are positive entries in a
//! bucket; usage is negative entries drawn from buckets in order. Usage draws from the
//! award, then credits, then a trial allowance, then the monthly allowance, then overage
//! if the person opted in. The spending cap counts what came from the allowance and
//! overage this period; when nothing is left to draw, the rest is recorded as `unbilled`
//! and work pauses.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Bucket {
    Award,
    Credit,
    Trial,
    Allowance,
    Overage,
    Unbilled,
}

impl Bucket {
    pub fn as_str(self) -> &'static str {
        match self {
            Bucket::Award => "award",
            Bucket::Credit => "credit",
            Bucket::Trial => "trial",
            Bucket::Allowance => "allowance",
            Bucket::Overage => "overage",
            Bucket::Unbilled => "unbilled",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "award" => Bucket::Award,
            "credit" => Bucket::Credit,
            "trial" => Bucket::Trial,
            "allowance" => Bucket::Allowance,
            "overage" => Bucket::Overage,
            "unbilled" => Bucket::Unbilled,
            _ => return None,
        })
    }
}

/// What is left to draw, and what counts toward the cap this period.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Balances {
    pub award: i64,
    pub credit: i64,
    pub trial: i64,
    pub allowance: i64,
    /// Drawn from the allowance and overage this period (positive).
    pub cap_spend: i64,
    /// Overage drawn this period (positive), billed at the period's end.
    pub overage: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapRules {
    /// The monthly spending cap in micros.
    pub cap: i64,
    pub overage_enabled: bool,
}

/// Split a cost across buckets. Never draws past the cap; whatever can't be covered is
/// `Unbilled` (the account pauses).
pub fn allocate(cost: i64, b: &Balances, rules: CapRules) -> Vec<(Bucket, i64)> {
    let mut left = cost.max(0);
    let mut out = Vec::new();
    let mut take = |bucket: Bucket, available: i64, left: &mut i64| {
        let n = (*left).min(available.max(0));
        if n > 0 {
            out.push((bucket, n));
            *left -= n;
        }
    };
    take(Bucket::Award, b.award, &mut left);
    take(Bucket::Credit, b.credit, &mut left);
    take(Bucket::Trial, b.trial, &mut left);
    let cap_room = (rules.cap - b.cap_spend).max(0);
    let from_allowance = b.allowance.min(cap_room).max(0);
    let before = left;
    take(Bucket::Allowance, from_allowance, &mut left);
    if rules.overage_enabled {
        take(Bucket::Overage, cap_room - (before - left), &mut left);
    }
    if left > 0 {
        out.push((Bucket::Unbilled, left));
    }
    out
}

/// True when nothing more can be drawn: work must pause.
pub fn exhausted(b: &Balances, rules: CapRules) -> bool {
    allocate(1, b, rules).first().map(|(k, _)| *k) == Some(Bucket::Unbilled)
}

/// Which alert thresholds (50, 80, 100 percent of the cap) have been reached.
pub fn alert_level(b: &Balances, rules: CapRules) -> i32 {
    if rules.cap <= 0 {
        return 100;
    }
    let pct = b.cap_spend as f64 * 100.0 / rules.cap as f64;
    [100, 80, 50].into_iter().find(|t| pct >= *t as f64).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const D: i64 = 1_000_000;

    #[test]
    fn award_then_allowance_then_overage() {
        let rules = CapRules { cap: 3 * D, overage_enabled: true };
        let b = Balances { award: D, allowance: 2 * D, ..Default::default() };
        assert_eq!(allocate(D / 2, &b, rules), vec![(Bucket::Award, D / 2)]);
        assert_eq!(allocate(2 * D, &b, rules), vec![(Bucket::Award, D), (Bucket::Allowance, D)]);
        // Award gone, allowance gone: overage up to the cap.
        let b = Balances { allowance: 0, cap_spend: 2 * D, ..Default::default() };
        assert_eq!(allocate(2 * D, &b, rules), vec![(Bucket::Overage, D), (Bucket::Unbilled, D)]);
    }

    #[test]
    fn without_overage_work_pauses_when_the_allowance_is_spent() {
        let rules = CapRules { cap: D, overage_enabled: false };
        let b = Balances { allowance: D / 4, cap_spend: 3 * D / 4, ..Default::default() };
        assert_eq!(allocate(D, &b, rules), vec![(Bucket::Allowance, D / 4), (Bucket::Unbilled, 3 * D / 4)]);
        assert!(!exhausted(&b, rules));
        let b = Balances { allowance: 0, cap_spend: D, ..Default::default() };
        assert!(exhausted(&b, rules));
    }

    #[test]
    fn credits_and_trials_come_before_the_allowance() {
        let rules = CapRules { cap: D, overage_enabled: false };
        let b = Balances { credit: 10, trial: 10, allowance: D, ..Default::default() };
        assert_eq!(allocate(25, &b, rules), vec![(Bucket::Credit, 10), (Bucket::Trial, 10), (Bucket::Allowance, 5)]);
    }

    #[test]
    fn a_cap_below_the_allowance_pauses_early() {
        let rules = CapRules { cap: D / 2, overage_enabled: false };
        let b = Balances { allowance: D, cap_spend: D / 2, ..Default::default() };
        assert!(exhausted(&b, rules));
    }

    #[test]
    fn alerts_at_half_four_fifths_and_the_cap() {
        let rules = CapRules { cap: 10 * D, overage_enabled: true };
        let at = |spend| alert_level(&Balances { cap_spend: spend, ..Default::default() }, rules);
        assert_eq!(at(4 * D), 0);
        assert_eq!(at(5 * D), 50);
        assert_eq!(at(8 * D), 80);
        assert_eq!(at(10 * D), 100);
    }
}
