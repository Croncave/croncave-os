//! The watcher engine: runs any watcher type as fetch, extract, compare, report.

use std::collections::HashSet;

use croncave_proto::watcher::{Condition, Direction, Extract, FilterOp, Source, WatcherType, fill};
use croncave_proto::{CheckSetup, OutputStream, RunOutcome, RunResult, SdkCall, Summary, SummaryValue};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::runs::RunCtx;

#[derive(Debug, Clone, PartialEq)]
pub enum Observation {
    Text(String),
    Items(Vec<Item>),
    Value(f64),
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Item {
    pub id: String,
    pub fields: Map<String, Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct State {
    pub last_text: Option<String>,
    #[serde(default)]
    pub seen: Vec<String>,
    pub last_value: Option<f64>,
    #[serde(default)]
    pub in_condition: bool,
    /// (unix ms, value) for charts.
    #[serde(default)]
    pub history: Vec<(i64, f64)>,
    #[serde(default)]
    pub checks: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Evaluation {
    pub matched: bool,
    pub headline: String,
    pub values: Vec<SummaryValue>,
    pub data: Value,
    pub state: State,
}

pub async fn run(ctx: RunCtx) -> RunResult {
    let test = ctx.spec.setup.get("test").and_then(Value::as_bool).unwrap_or(false);
    let setup: CheckSetup = match serde_json::from_value(ctx.spec.setup.clone()) {
        Ok(s) => s,
        Err(e) => return couldnt_check(&ctx, format!("The watch's setup couldn't be read: {e}")),
    };
    let state_dir = ctx.agent.disk.system("apps/watcher").join(ctx.spec.job_id.to_string());
    let state_path = state_dir.join("state.json");
    let state: State =
        std::fs::read(&state_path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();

    ctx.output(
        OutputStream::System,
        format!("Checking with the {} type (version {})", setup.watch_type.name, setup.watch_type.version),
    );
    let obs = match observe(&ctx, &setup.watch_type, &setup.inputs).await {
        Ok(o) => o,
        Err(e) => return couldnt_check(&ctx, e),
    };
    let eval = match evaluate(&setup.watch_type, &setup.inputs, &state, obs, chrono::Utc::now().timestamp_millis()) {
        Ok(e) => e,
        Err(e) => return couldnt_check(&ctx, e),
    };
    ctx.output(OutputStream::System, &eval.headline);
    if test {
        ctx.output(OutputStream::System, "This was a test: nothing was saved and no one was told.");
    } else {
        let _ = std::fs::create_dir_all(&state_dir);
        let _ = std::fs::write(&state_path, serde_json::to_vec(&eval.state).expect("state serializes"));
    }
    let mut r = ctx.result(RunOutcome::Succeeded);
    r.summary = Some(Summary { headline: eval.headline.clone(), values: eval.values });
    r.output_tail = eval.headline;
    r.data = json!({ "matched": eval.matched, "test": test, "detail": eval.data });
    r
}

fn couldnt_check(ctx: &RunCtx, why: String) -> RunResult {
    ctx.output(OutputStream::System, format!("Couldn't check: {why}"));
    let mut r = ctx.result(RunOutcome::Failed);
    r.summary = Some(Summary { headline: "Couldn't check".into(), values: vec![] });
    r.output_tail = why.clone();
    r.error = Some(why);
    r.data = json!({ "matched": false, "couldnt_check": true });
    r
}

async fn observe(ctx: &RunCtx, t: &WatcherType, inputs: &Map<String, Value>) -> Result<Observation, String> {
    match &t.source {
        Source::Http { url } => {
            let url = fill(url, inputs);
            let body = fetch(&url).await?;
            extract(&t.extract, inputs, &body)
        }
        Source::Platform { call, symbol } => {
            let symbol = fill(symbol, inputs).trim().to_uppercase();
            if call != "market.quote" {
                return Err(format!("This computer doesn't know the \"{call}\" source."));
            }
            let v = ctx.agent.sdk(SdkCall::MarketQuote { symbol: symbol.clone() }).await?;
            match &t.extract {
                Extract::Value { pointer, .. } => v
                    .pointer(pointer)
                    .and_then(Value::as_f64)
                    .map(Observation::Value)
                    .ok_or_else(|| format!("No price came back for {symbol}.")),
                _ => Err("Platform sources give a single value.".into()),
            }
        }
    }
}

async fn fetch(url: &str) -> Result<String, String> {
    let parsed = reqwest::Url::parse(url).map_err(|_| format!("\"{url}\" isn't a web address."))?;
    let host = parsed.host_str().unwrap_or("the site").to_string();
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .user_agent("Croncave-Watcher/0.1 (+https://croncave.com)")
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client.get(parsed).send().await.map_err(|e| {
        if e.is_timeout() {
            format!("{host} took too long to answer.")
        } else {
            format!("Couldn't reach {host}. The site may be down or the address may be wrong.")
        }
    })?;
    let status = resp.status();
    if !status.is_success() {
        return Err(format!("{host} answered {} ({}).", status.as_u16(), status.canonical_reason().unwrap_or("error")));
    }
    resp.text().await.map_err(|e| e.to_string())
}

pub fn extract(how: &Extract, inputs: &Map<String, Value>, body: &str) -> Result<Observation, String> {
    let html = scraper::Html::parse_document(body);
    match how {
        Extract::Text { selector } => {
            let sel_src = selector.as_deref().map(|s| fill(s, inputs)).filter(|s| !s.trim().is_empty());
            let text = match &sel_src {
                Some(s) => {
                    let sel =
                        scraper::Selector::parse(s).map_err(|_| format!("\"{s}\" isn't a valid part of a page."))?;
                    let parts: Vec<String> =
                        html.select(&sel).map(|e| e.text().collect::<Vec<_>>().join(" ")).collect();
                    if parts.is_empty() {
                        return Err(format!(
                            "The part of the page you picked (\"{s}\") wasn't found. The page may have changed."
                        ));
                    }
                    parts.join("\n")
                }
                None => {
                    let body_sel = scraper::Selector::parse("body").expect("valid selector");
                    match html.select(&body_sel).next() {
                        Some(b) => visible_text(b),
                        None => body.to_string(),
                    }
                }
            };
            Ok(Observation::Text(normalize(&text)))
        }
        Extract::Items { item, id_field, fields } => {
            let item_sel = scraper::Selector::parse(item).map_err(|_| "The item selector isn't valid.".to_string())?;
            let mut items = Vec::new();
            for el in html.select(&item_sel) {
                let mut map = Map::new();
                for f in fields {
                    let Ok(sel) = scraper::Selector::parse(&f.selector) else { continue };
                    let Some(node) = el.select(&sel).next() else { continue };
                    let raw = match &f.attr {
                        Some(a) => node.value().attr(a).unwrap_or_default().to_string(),
                        None => normalize(&node.text().collect::<Vec<_>>().join(" ")),
                    };
                    let v =
                        if f.number { parse_number(&raw).map(|n| json!(n)).unwrap_or(Value::Null) } else { json!(raw) };
                    map.insert(f.key.clone(), v);
                }
                let id = match map.get(id_field) {
                    Some(Value::String(s)) if !s.is_empty() => s.clone(),
                    Some(Value::Number(n)) => n.to_string(),
                    _ => continue,
                };
                items.push(Item { id, fields: map });
            }
            if items.is_empty() {
                return Err("No items were found on the page. The page may have changed.".into());
            }
            Ok(Observation::Items(items))
        }
        Extract::Value { .. } => Err("This type reads a single value, which a web page doesn't give.".into()),
    }
}

fn visible_text(el: scraper::ElementRef) -> String {
    let mut out = String::new();
    for node in el.descendants() {
        if let Some(t) = node.value().as_text() {
            let parent_is_code = node
                .parent()
                .and_then(|p| p.value().as_element().map(|e| matches!(e.name(), "script" | "style" | "noscript")))
                .unwrap_or(false);
            if !parent_is_code {
                out.push_str(t);
                out.push('\n');
            }
        }
    }
    out
}

fn normalize(s: &str) -> String {
    s.lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// "$1,950 / mo" is 1950; "-2.5%" is -2.5.
pub fn parse_number(s: &str) -> Option<f64> {
    let re = regex::Regex::new(r"-?\d[\d,]*(?:\.\d+)?").expect("valid regex");
    re.find(s).and_then(|m| m.as_str().replace(',', "").parse().ok())
}

pub fn evaluate(
    t: &WatcherType,
    inputs: &Map<String, Value>,
    prev: &State,
    obs: Observation,
    now_ms: i64,
) -> Result<Evaluation, String> {
    let mut state = prev.clone();
    state.checks += 1;
    let first = prev.checks == 0;
    let (matched, headline, values, data) = match (&t.condition, obs) {
        (Condition::Changed, Observation::Text(text)) => {
            let r = match &prev.last_text {
                None => (false, "First check: saved the page to compare against".to_string(), json!({})),
                Some(old) if *old != text => {
                    let (added, removed) = line_diff(old, &text);
                    (true, "The page changed".to_string(), json!({ "added": added, "removed": removed }))
                }
                Some(_) => (false, "No change since the last check".to_string(), json!({})),
            };
            let excerpt: String = text.chars().take(600).collect();
            state.last_text = Some(text);
            (r.0, r.1, vec![], json!({ "diff": r.2, "excerpt": excerpt }))
        }
        (Condition::Contains { text: needle }, Observation::Text(text)) => {
            let needle = fill(needle, inputs);
            let present = text.to_lowercase().contains(&needle.to_lowercase());
            let matched = present && !prev.in_condition;
            state.in_condition = present;
            state.last_text = Some(text.clone());
            let headline = if present {
                format!("Found \"{needle}\" on the page")
            } else {
                format!("\"{needle}\" isn't on the page yet")
            };
            (
                matched,
                headline,
                vec![],
                json!({ "present": present, "excerpt": text.chars().take(600).collect::<String>() }),
            )
        }
        (Condition::NewItems { filters }, Observation::Items(items)) => {
            let seen: HashSet<&str> = prev.seen.iter().map(String::as_str).collect();
            let passing: Vec<&Item> = items.iter().filter(|i| passes(i, filters, inputs)).collect();
            let new: Vec<&Item> = passing.iter().copied().filter(|i| !seen.contains(i.id.as_str())).collect();
            let mut all_seen = prev.seen.clone();
            for i in &items {
                if !seen.contains(i.id.as_str()) {
                    all_seen.push(i.id.clone());
                }
            }
            let excess = all_seen.len().saturating_sub(2000);
            all_seen.drain(..excess);
            state.seen = all_seen;
            let n = new.len();
            let headline = match (n, first) {
                (0, _) => format!("No new matches ({} on the page, {} match your filters)", items.len(), passing.len()),
                (1, true) => "1 match on the first check".to_string(),
                (n, true) => format!("{n} matches on the first check"),
                (1, false) => "1 new match".to_string(),
                (n, false) => format!("{n} new matches"),
            };
            let values = vec![
                SummaryValue { label: "On the page".into(), value: items.len().to_string() },
                SummaryValue { label: "Match your filters".into(), value: passing.len().to_string() },
                SummaryValue { label: "New".into(), value: n.to_string() },
            ];
            (n > 0, headline, values, json!({ "new_items": new, "matching": passing }))
        }
        (Condition::Threshold { direction, value }, Observation::Value(v)) => {
            let label = match &t.extract {
                Extract::Value { label, .. } => fill(label, inputs),
                _ => "The value".into(),
            };
            let limit_src = fill(value, inputs);
            let limit =
                parse_number(&limit_src).ok_or_else(|| format!("\"{limit_src}\" isn't a number to compare with."))?;
            let direction = Direction::parse(&fill(direction, inputs)).ok_or("Choose above or below.")?;
            let inside = match direction {
                Direction::Above => v > limit,
                Direction::Below => v < limit,
            };
            let matched = inside && !prev.in_condition;
            state.in_condition = inside;
            let change = prev.last_value.map(|p| v - p);
            state.last_value = Some(v);
            state.history.push((now_ms, v));
            let excess = state.history.len().saturating_sub(200);
            state.history.drain(..excess);
            let word = match direction {
                Direction::Above => "above",
                Direction::Below => "below",
            };
            let headline = if inside {
                format!("{label} is {v:.2}, {word} your {limit:.2}")
            } else {
                format!("{label} is {v:.2}")
            };
            let mut values = vec![SummaryValue { label: label.clone(), value: format!("{v:.2}") }];
            if let Some(c) = change {
                values.push(SummaryValue { label: "Since last check".into(), value: format!("{c:+.2}") });
            }
            (matched, headline, values, json!({ "value": v, "limit": limit, "history": state.history }))
        }
        (_, _) => return Err("This type's condition doesn't fit what it reads.".into()),
    };
    Ok(Evaluation { matched, headline, values, data, state })
}

fn passes(item: &Item, filters: &[croncave_proto::watcher::Filter], inputs: &Map<String, Value>) -> bool {
    filters.iter().all(|f| {
        let want = fill(&f.value, inputs);
        if want.trim().is_empty() {
            return true; // An empty input means "any".
        }
        let have = item.fields.get(&f.field);
        match f.op {
            FilterOp::Contains => {
                have.and_then(Value::as_str).is_some_and(|s| s.to_lowercase().contains(&want.to_lowercase()))
            }
            FilterOp::AtMost | FilterOp::AtLeast => {
                let (Some(h), Some(w)) = (have.and_then(Value::as_f64), parse_number(&want)) else { return false };
                if f.op == FilterOp::AtMost { h <= w } else { h >= w }
            }
        }
    })
}

fn line_diff(old: &str, new: &str) -> (Vec<String>, Vec<String>) {
    let o: HashSet<&str> = old.lines().collect();
    let n: HashSet<&str> = new.lines().collect();
    let added = new.lines().filter(|l| !o.contains(l)).take(20).map(String::from).collect();
    let removed = old.lines().filter(|l| !n.contains(l)).take(20).map(String::from).collect();
    (added, removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listings_type() -> WatcherType {
        serde_json::from_value(json!({
            "id": "demo-listings", "version": 1, "name": "Listings", "description": "",
            "inputs": [
                {"key": "max_price", "label": "Most rent", "kind": {"type": "number"}},
                {"key": "min_beds", "label": "Fewest bedrooms", "kind": {"type": "number"}}
            ],
            "source": {"type": "http", "url": "http://x"},
            "extract": {"type": "items", "item": ".listing", "id_field": "id", "fields": [
                {"key": "id", "label": "Id", "selector": "a", "attr": "data-id"},
                {"key": "title", "label": "Title", "selector": "h3"},
                {"key": "price", "label": "Rent", "selector": ".price", "number": true},
                {"key": "beds", "label": "Beds", "selector": ".beds", "number": true}
            ]},
            "condition": {"type": "new_items", "filters": [
                {"field": "price", "op": "at_most", "value": "{max_price}"},
                {"field": "beds", "op": "at_least", "value": "{min_beds}"}
            ]},
            "rule": "r", "view": "items"
        }))
        .unwrap()
    }

    fn page(listings: &[(&str, &str, u32, u32)]) -> String {
        let items: String = listings
            .iter()
            .map(|(id, t, p, b)| {
                format!(r#"<div class="listing"><a data-id="{id}"><h3>{t}</h3></a><span class="price">${p} / mo</span><span class="beds">{b} bd</span></div>"#)
            })
            .collect();
        format!("<html><body>{items}</body></html>")
    }

    fn inputs(v: Value) -> Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    #[test]
    fn new_listings_matching_filters_are_found_once() {
        let t = listings_type();
        let inp = inputs(json!({"max_price": "2000", "min_beds": ""}));
        let obs = extract(&t.extract, &inp, &page(&[("a", "Loft", 1900, 1), ("b", "House", 3000, 3)])).unwrap();
        let e1 = evaluate(&t, &inp, &State::default(), obs, 0).unwrap();
        assert!(e1.matched);
        assert_eq!(e1.headline, "1 match on the first check");

        // Same page again: nothing new.
        let obs = extract(&t.extract, &inp, &page(&[("a", "Loft", 1900, 1), ("b", "House", 3000, 3)])).unwrap();
        let e2 = evaluate(&t, &inp, &e1.state, obs, 0).unwrap();
        assert!(!e2.matched);

        // A planted listing under the price appears.
        let obs = extract(&t.extract, &inp, &page(&[("a", "Loft", 1900, 1), ("c", "Studio", 1500, 0)])).unwrap();
        let e3 = evaluate(&t, &inp, &e2.state, obs, 0).unwrap();
        assert!(e3.matched);
        assert_eq!(e3.headline, "1 new match");
    }

    #[test]
    fn page_change_is_detected_after_a_baseline() {
        let mut t = listings_type();
        t.extract = Extract::Text { selector: Some("#price".into()) };
        t.condition = Condition::Changed;
        let none = Map::new();
        let first = extract(&t.extract, &none, "<p id=price>$10</p>").unwrap();
        let e1 = evaluate(&t, &none, &State::default(), first, 0).unwrap();
        assert!(!e1.matched);
        let same = extract(&t.extract, &none, "<p id=price>$10</p><p>ad</p>").unwrap();
        assert!(!evaluate(&t, &none, &e1.state, same, 0).unwrap().matched, "changes outside the selector don't count");
        let changed = extract(&t.extract, &none, "<p id=price>$12</p>").unwrap();
        let e3 = evaluate(&t, &none, &e1.state, changed, 0).unwrap();
        assert!(e3.matched);
        assert_eq!(e3.data["diff"]["added"][0], "$12");
    }

    #[test]
    fn missing_part_of_page_is_couldnt_check() {
        let err = extract(&Extract::Text { selector: Some("#gone".into()) }, &Map::new(), "<p>hi</p>").unwrap_err();
        assert!(err.contains("wasn't found"));
    }

    #[test]
    fn threshold_alerts_on_crossing_only() {
        let mut t = listings_type();
        t.extract = Extract::Value { pointer: "/price".into(), label: "{symbol} price".into() };
        t.condition = Condition::Threshold { direction: "{direction}".into(), value: "{target}".into() };
        let inp = inputs(json!({"symbol": "ACME", "target": "100", "direction": "below"}));
        let e1 = evaluate(&t, &inp, &State::default(), Observation::Value(105.0), 1).unwrap();
        assert!(!e1.matched);
        let e2 = evaluate(&t, &inp, &e1.state, Observation::Value(99.5), 2).unwrap();
        assert!(e2.matched);
        assert_eq!(e2.headline, "ACME price is 99.50, below your 100.00");
        let e3 = evaluate(&t, &inp, &e2.state, Observation::Value(98.0), 3).unwrap();
        assert!(!e3.matched, "staying below doesn't alert again");
        assert_eq!(e3.state.history.len(), 3);
    }

    #[test]
    fn numbers_parse_from_display_text() {
        assert_eq!(parse_number("$1,950 / mo"), Some(1950.0));
        assert_eq!(parse_number("down -2.5%"), Some(-2.5));
        assert_eq!(parse_number("none"), None);
    }
}
