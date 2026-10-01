//! Delivers notification-worthy events on the channels each person turned on, once per
//! channel, holding non-urgent ones during quiet hours and grouping repeats.

use std::collections::BTreeMap;
use std::time::Duration;

use chrono::Timelike;
use serde_json::Value;
use uuid::Uuid;

use crate::providers::notifier::{Channel, Message};
use crate::state::AppState;

pub fn kick(app: &AppState) {
    app.kick.notify_waiters();
}

#[derive(sqlx::FromRow)]
struct Pending {
    id: i64,
    account_id: Uuid,
    app: String,
    kind: String,
    title: String,
    body: String,
    urgent: bool,
}

pub async fn run_loop(app: AppState) {
    loop {
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(2)) => {}
            _ = app.kick.notified() => {}
        }
        if let Err(e) = deliver(&app).await {
            tracing::error!(error = %e, "notification delivery failed");
        }
    }
}

/// Whether `hour` (0-23, local) falls in quiet hours `start..end` (which may wrap midnight).
pub fn in_quiet_hours(prefs: &Value, now_utc: chrono::DateTime<chrono::Utc>) -> bool {
    let q = &prefs["quiet_hours"];
    if q["enabled"].as_bool() != Some(true) {
        return false;
    }
    let offset = q["utc_offset_minutes"].as_i64().unwrap_or(0);
    let local = now_utc + chrono::Duration::minutes(offset);
    let (start, end) = (q["start_hour"].as_u64().unwrap_or(22) as u32, q["end_hour"].as_u64().unwrap_or(7) as u32);
    let h = local.hour();
    if start <= end { (start..end).contains(&h) } else { h >= start || h < end }
}

pub async fn deliver(app: &AppState) -> anyhow::Result<()> {
    let pending: Vec<Pending> = sqlx::query_as(
        "select id, account_id, app, kind, title, body, urgent from events e
         where notify and created_at > now() - interval '2 days'
           and not exists (select 1 from notification_deliveries d where d.event_id = e.id)
         order by id limit 200",
    )
    .fetch_all(&app.db)
    .await?;
    let mut by_account: BTreeMap<Uuid, Vec<Pending>> = BTreeMap::new();
    for p in pending {
        by_account.entry(p.account_id).or_default().push(p);
    }
    for (account, events) in by_account {
        let user: Option<(String, Option<String>, Value)> = sqlx::query_as(
            "select u.email, u.phone, u.prefs from users u join account_members m on m.user_id = u.id where m.account_id = $1 limit 1",
        )
        .bind(account)
        .fetch_optional(&app.db)
        .await?;
        let Some((email, phone, prefs)) = user else { continue };
        let quiet = in_quiet_hours(&prefs, app.now());
        let (now_events, _held): (Vec<Pending>, Vec<Pending>) = events.into_iter().partition(|e| e.urgent || !quiet);
        // Group repeats: three or more of the same kind become one message.
        let mut groups: BTreeMap<(String, String), Vec<Pending>> = BTreeMap::new();
        for e in now_events {
            groups.entry((e.app.clone(), e.kind.clone())).or_default().push(e);
        }
        for ((app_id, _), group) in groups {
            let chunks: Vec<Vec<Pending>> =
                if group.len() >= 3 { vec![group] } else { group.into_iter().map(|e| vec![e]).collect() };
            for chunk in chunks {
                let (subject, body) = if chunk.len() == 1 {
                    (chunk[0].title.clone(), chunk[0].body.clone())
                } else {
                    let app_name = match app_id.as_str() {
                        "watcher" => "Watcher",
                        "scripts" => "Scripts",
                        "code" => "Code",
                        "billing" => "Billing",
                        _ => "Croncave",
                    };
                    (
                        format!("{} updates from {app_name}", chunk.len()),
                        chunk.iter().map(|e| format!("• {}", e.title)).collect::<Vec<_>>().join("\n"),
                    )
                };
                let link = format!("{}/home", app.cfg.web_url);
                for channel in [Channel::Email, Channel::Sms, Channel::Push] {
                    let on = prefs["channels"][channel.as_str()].as_bool().unwrap_or(channel == Channel::Email);
                    let to = match channel {
                        Channel::Email => Some(email.clone()),
                        Channel::Sms => phone.clone(),
                        Channel::Push => Some(format!("browser:{account}")),
                    };
                    let (status, outbox_id) = match (on, to) {
                        (true, Some(to)) => {
                            let msg = Message {
                                channel,
                                to,
                                subject: subject.clone(),
                                body: if channel == Channel::Sms {
                                    format!("Croncave: {subject}")
                                } else {
                                    format!("{body}\n\n{link}")
                                },
                                link: Some(link.clone()),
                            };
                            match app.providers.notifier.send(msg).await {
                                Ok(id) => ("sent", Some(id)),
                                Err(e) => {
                                    tracing::warn!(error = %e, "sending a notification failed");
                                    ("failed", None)
                                }
                            }
                        }
                        _ => ("off", None),
                    };
                    for e in &chunk {
                        sqlx::query("insert into notification_deliveries (event_id, channel, status, outbox_id) values ($1, $2, $3, $4) on conflict do nothing")
                            .bind(e.id)
                            .bind(channel.as_str())
                            .bind(status)
                            .bind(outbox_id)
                            .execute(&app.db)
                            .await?;
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use serde_json::json;

    #[test]
    fn quiet_hours_wrap_midnight_and_respect_offset() {
        let prefs =
            json!({ "quiet_hours": { "enabled": true, "start_hour": 22, "end_hour": 7, "utc_offset_minutes": -420 } });
        // 06:00 UTC is 23:00 at UTC-7: quiet.
        assert!(in_quiet_hours(&prefs, chrono::Utc.with_ymd_and_hms(2026, 1, 1, 6, 0, 0).unwrap()));
        // 18:00 UTC is 11:00 at UTC-7: not quiet.
        assert!(!in_quiet_hours(&prefs, chrono::Utc.with_ymd_and_hms(2026, 1, 1, 18, 0, 0).unwrap()));
        assert!(!in_quiet_hours(&json!({}), chrono::Utc::now()));
    }
}
