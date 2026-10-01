//! The US time zones a person can choose (Croncave is US only), and schedule times in them.

use chrono::{DateTime, Offset, Utc};
use chrono_tz::Tz;
use serde_json::{Value, json};

pub struct Zone {
    pub id: &'static str,
    pub name: &'static str,
    pub places: &'static str,
    /// Short label used after a time, as in "9:00 am ET".
    pub short: &'static str,
}

pub const ZONES: &[Zone] = &[
    Zone { id: "America/New_York", name: "Eastern", places: "New York, Miami, Atlanta", short: "ET" },
    Zone { id: "America/Chicago", name: "Central", places: "Chicago, Dallas, Houston", short: "CT" },
    Zone { id: "America/Denver", name: "Mountain", places: "Denver, Salt Lake City", short: "MT" },
    Zone {
        id: "America/Phoenix",
        name: "Mountain, no daylight saving",
        places: "Phoenix, most of Arizona",
        short: "Arizona time",
    },
    Zone { id: "America/Los_Angeles", name: "Pacific", places: "Los Angeles, Seattle, Las Vegas", short: "PT" },
    Zone { id: "America/Anchorage", name: "Alaska", places: "Anchorage, Juneau", short: "AKT" },
    Zone { id: "America/Adak", name: "Hawaii–Aleutian", places: "Adak, Aleutian Islands", short: "HAT" },
    Zone { id: "Pacific/Honolulu", name: "Hawaii", places: "Honolulu", short: "HT" },
    Zone { id: "America/Puerto_Rico", name: "Atlantic", places: "Puerto Rico, US Virgin Islands", short: "AT" },
    Zone { id: "Pacific/Pago_Pago", name: "Samoa", places: "American Samoa", short: "SST" },
    Zone { id: "Pacific/Guam", name: "Chamorro", places: "Guam, Northern Mariana Islands", short: "ChT" },
];

pub const DEFAULT: &str = "America/New_York";

pub fn find(id: &str) -> Option<&'static Zone> {
    ZONES.iter().find(|z| z.id == id)
}

/// The zone to compute in; anything unknown falls back to Eastern.
pub fn tz(id: &str) -> Tz {
    find(id).and_then(|z| z.id.parse().ok()).unwrap_or(chrono_tz::America::New_York)
}

pub fn short(id: &str) -> &'static str {
    find(id).map(|z| z.short).unwrap_or("ET")
}

/// "UTC−4": the offset right now, with a real minus sign.
pub fn offset_label(id: &str, at: DateTime<Utc>) -> String {
    let secs = at.with_timezone(&tz(id)).offset().fix().local_minus_utc();
    let h = secs / 3600;
    let m = (secs.abs() % 3600) / 60;
    let sign = if secs < 0 { "−" } else { "+" };
    if m == 0 { format!("UTC{sign}{}", h.abs()) } else { format!("UTC{sign}{}:{m:02}", h.abs()) }
}

/// Every zone for the picker, with its offset now.
pub fn list(at: DateTime<Utc>) -> Value {
    json!(
        ZONES
            .iter()
            .map(|z| json!({ "id": z.id, "name": z.name, "places": z.places, "short": z.short, "offset": offset_label(z.id, at) }))
            .collect::<Vec<_>>()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn offsets_follow_daylight_saving() {
        let summer = Utc.with_ymd_and_hms(2026, 7, 1, 12, 0, 0).unwrap();
        let winter = Utc.with_ymd_and_hms(2026, 1, 15, 12, 0, 0).unwrap();
        assert_eq!(offset_label("America/New_York", summer), "UTC−4");
        assert_eq!(offset_label("America/New_York", winter), "UTC−5");
        assert_eq!(offset_label("America/Phoenix", summer), "UTC−7");
        assert_eq!(offset_label("Pacific/Guam", summer), "UTC+10");
    }

    #[test]
    fn every_zone_is_a_real_zone() {
        for z in ZONES {
            assert!(z.id.parse::<Tz>().is_ok(), "{} isn't a time zone", z.id);
        }
        assert_eq!(tz("Mars/Olympus"), chrono_tz::America::New_York);
    }
}
