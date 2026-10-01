//! The time everything in the control plane reads.
//!
//! In development the clock can be moved forward (Dev tools), so trials can end and
//! months can roll over without waiting. Durations that are billed (awake time, disk)
//! are measured with a monotonic clock instead, so moving the clock never bills time
//! that didn't pass.

use std::sync::atomic::{AtomicI64, Ordering};

use chrono::{DateTime, Duration, Utc};

#[derive(Debug, Default)]
pub struct Clock {
    adjustable: bool,
    offset_secs: AtomicI64,
}

impl Clock {
    pub fn new(adjustable: bool) -> Self {
        Self { adjustable, offset_secs: AtomicI64::new(0) }
    }

    pub fn now(&self) -> DateTime<Utc> {
        Utc::now() + Duration::seconds(self.offset_secs.load(Ordering::Relaxed))
    }

    pub fn offset_secs(&self) -> i64 {
        self.offset_secs.load(Ordering::Relaxed)
    }

    /// Move time forward. Only an adjustable clock moves.
    pub fn advance(&self, secs: i64) -> Result<(), &'static str> {
        if !self.adjustable {
            return Err("The clock isn't adjustable (set CLOCK=adjustable).");
        }
        if secs < 0 {
            return Err("Time only moves forward.");
        }
        self.offset_secs.fetch_add(secs, Ordering::Relaxed);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_adjustable_clocks_move_and_only_forward() {
        let fixed = Clock::new(false);
        assert!(fixed.advance(10).is_err());
        let c = Clock::new(true);
        let before = c.now();
        c.advance(86_400).unwrap();
        assert!(c.now() - before >= Duration::seconds(86_399));
        assert!(c.advance(-1).is_err());
    }
}
