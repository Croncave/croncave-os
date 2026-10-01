//! Watcher types: data the watcher engine reads, not code.
//!
//! A type says what a person fills in ([`InputDef`]), where to read from ([`Source`]),
//! what to pull out ([`Extract`]) and when to tell them ([`Condition`]). Templates use
//! `{input_key}` placeholders. The control plane validates a type before it can be
//! approved, and the agent's engine runs it.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WatcherType {
    pub id: String,
    pub version: u32,
    pub name: String,
    pub description: String,
    pub inputs: Vec<InputDef>,
    pub source: Source,
    pub extract: Extract,
    pub condition: Condition,
    /// The plain-words rule, e.g. "Check {url} and tell me when the page changes".
    pub rule: String,
    pub view: View,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InputDef {
    pub key: String,
    pub label: String,
    pub kind: InputKind,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub default: Option<Value>,
    #[serde(default)]
    pub help: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InputKind {
    Text,
    Url,
    Number,
    Select { options: Vec<String> },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Source {
    /// Fetch a URL from the computer.
    Http { url: String },
    /// Ask the platform (keys stay in the control plane), e.g. `market.quote`.
    Platform { call: String, symbol: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Extract {
    /// The visible text of the page, or of the elements matching a CSS selector.
    Text {
        #[serde(default)]
        selector: Option<String>,
    },
    /// A list of items: one CSS selector per item, one per field inside it.
    Items { item: String, id_field: String, fields: Vec<FieldDef> },
    /// A single number from the platform's JSON answer (a JSON pointer such as `/price`).
    Value { pointer: String, label: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FieldDef {
    pub key: String,
    pub label: String,
    pub selector: String,
    /// Read this attribute instead of the text.
    #[serde(default)]
    pub attr: Option<String>,
    /// Parse as a number ("$1,950 / mo" becomes 1950).
    #[serde(default)]
    pub number: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Condition {
    /// Anything in the extracted text changed since the last check.
    Changed,
    /// The text contains this (template).
    Contains { text: String },
    /// Items not seen before that pass every filter. Filters with an empty input are skipped.
    NewItems { filters: Vec<Filter> },
    /// A value crossed a threshold. Both are templates; the direction fills to
    /// `above` or `below`.
    Threshold { direction: String, value: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Filter {
    pub field: String,
    pub op: FilterOp,
    pub value: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FilterOp {
    AtMost,
    AtLeast,
    Contains,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Above,
    Below,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum View {
    Text,
    Items,
    Chart,
}

impl Direction {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "above" => Some(Self::Above),
            "below" => Some(Self::Below),
            _ => None,
        }
    }
}

/// Platform calls a type may use.
pub const PLATFORM_CALLS: &[&str] = &["market.quote"];

impl WatcherType {
    /// Every problem with the type, in plain words. Empty means valid.
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        let keys: Vec<&str> = self.inputs.iter().map(|i| i.key.as_str()).collect();
        if self.id.trim().is_empty() || !self.id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            out.push("The type's id must be letters, digits and dashes.".into());
        }
        if self.name.trim().is_empty() {
            out.push("The type needs a name.".into());
        }
        for (i, input) in self.inputs.iter().enumerate() {
            if keys[..i].contains(&input.key.as_str()) {
                out.push(format!("The input \"{}\" is listed twice.", input.key));
            }
        }
        let mut templates: Vec<(&str, &str)> = vec![("rule", &self.rule)];
        match &self.source {
            Source::Http { url } => templates.push(("source URL", url)),
            Source::Platform { call, symbol } => {
                if !PLATFORM_CALLS.contains(&call.as_str()) {
                    out.push(format!("The platform has no \"{call}\" source."));
                }
                templates.push(("symbol", symbol));
            }
        }
        match &self.extract {
            Extract::Text { selector } => {
                if selector.as_deref().is_some_and(|s| s.trim().is_empty()) {
                    out.push("The text selector is empty.".into());
                }
            }
            Extract::Items { item, id_field, fields } => {
                if item.trim().is_empty() {
                    out.push("The item selector is empty.".into());
                }
                if !fields.iter().any(|f| &f.key == id_field) {
                    out.push(format!("The id field \"{id_field}\" isn't one of the fields."));
                }
            }
            Extract::Value { pointer, .. } => {
                if !pointer.starts_with('/') {
                    out.push("The value pointer must start with /.".into());
                }
            }
        }
        match &self.condition {
            Condition::Contains { text } => templates.push(("condition", text)),
            Condition::Threshold { value, direction } => {
                templates.push(("threshold", value));
                templates.push(("direction", direction));
                if placeholders(direction).is_empty() && Direction::parse(direction).is_none() {
                    out.push("The direction must be \"above\" or \"below\".".into());
                }
                if !matches!(self.extract, Extract::Value { .. }) {
                    out.push("A threshold needs a single value to compare.".into());
                }
            }
            Condition::NewItems { filters } => {
                let Extract::Items { fields, .. } = &self.extract else {
                    out.push("New-item conditions need a list of items.".into());
                    return out;
                };
                for f in filters {
                    if !fields.iter().any(|d| d.key == f.field) {
                        out.push(format!("The filter reads \"{}\", which isn't a field.", f.field));
                    }
                    templates.push(("filter", &f.value));
                }
            }
            Condition::Changed => {}
        }
        for (what, t) in templates {
            for key in placeholders(t) {
                if !keys.contains(&key.as_str()) {
                    out.push(format!("The {what} uses {{{key}}}, which isn't an input."));
                }
            }
        }
        out
    }

    /// Fill in defaults and check required inputs. Returns the inputs to store.
    pub fn resolve_inputs(&self, given: &Map<String, Value>) -> Result<Map<String, Value>, Vec<String>> {
        let mut out = Map::new();
        let mut missing = Vec::new();
        for def in &self.inputs {
            let v = given.get(&def.key).filter(|v| !is_blank(v)).cloned().or_else(|| def.default.clone());
            match v {
                Some(v) => {
                    if let (InputKind::Number, Some(s)) = (&def.kind, v.as_str())
                        && s.trim().parse::<f64>().is_err()
                    {
                        missing.push(format!("{} must be a number.", def.label));
                    }
                    if let InputKind::Select { options } = &def.kind
                        && !options.iter().any(|o| Some(o.as_str()) == v.as_str())
                    {
                        missing.push(format!("{} must be one of: {}.", def.label, options.join(", ")));
                    }
                    out.insert(def.key.clone(), v);
                }
                None if def.required => missing.push(format!("{} is needed.", def.label)),
                None => {}
            }
        }
        if missing.is_empty() { Ok(out) } else { Err(missing) }
    }

    /// The setup in plain words, for the review before saving.
    pub fn plain_rule(&self, inputs: &Map<String, Value>) -> String {
        fill(&self.rule, inputs)
    }
}

fn is_blank(v: &Value) -> bool {
    v.is_null() || v.as_str().is_some_and(|s| s.trim().is_empty())
}

/// The `{key}` placeholders in a template.
pub fn placeholders(template: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        let after = &rest[start + 1..];
        let Some(end) = after.find('}') else { break };
        let key = &after[..end];
        if !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            out.push(key.to_string());
        }
        rest = &after[end + 1..];
    }
    out
}

/// Replace `{key}` with the input's value; missing values become empty.
pub fn fill(template: &str, inputs: &Map<String, Value>) -> String {
    let mut out = template.to_string();
    for key in placeholders(template) {
        let value = match inputs.get(&key) {
            Some(Value::String(s)) => s.clone(),
            Some(Value::Null) | None => String::new(),
            Some(other) => other.to_string(),
        };
        out = out.replace(&format!("{{{key}}}"), &value);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn page_type() -> WatcherType {
        serde_json::from_value(json!({
            "id": "web-page", "version": 1, "name": "Web page", "description": "d",
            "inputs": [
                {"key": "url", "label": "Page address", "kind": {"type": "url"}, "required": true},
                {"key": "selector", "label": "Part of the page", "kind": {"type": "text"}}
            ],
            "source": {"type": "http", "url": "{url}"},
            "extract": {"type": "text"},
            "condition": {"type": "changed"},
            "rule": "Check {url} and tell me when it changes",
            "view": "text"
        }))
        .unwrap()
    }

    #[test]
    fn valid_type_has_no_problems() {
        assert!(page_type().problems().is_empty(), "{:?}", page_type().problems());
    }

    #[test]
    fn unknown_placeholder_is_a_problem() {
        let mut t = page_type();
        t.rule = "Check {address}".into();
        let p = t.problems();
        assert_eq!(p.len(), 1);
        assert!(p[0].contains("{address}"));
    }

    #[test]
    fn threshold_without_a_value_is_a_problem() {
        let mut t = page_type();
        t.condition = Condition::Threshold { direction: "below".into(), value: "5".into() };
        assert!(t.problems().iter().any(|p| p.contains("threshold")));
    }

    #[test]
    fn required_inputs_are_checked_and_defaults_applied() {
        let mut t = page_type();
        t.inputs[1].default = Some(json!("main"));
        assert_eq!(t.resolve_inputs(&Map::new()).unwrap_err(), vec!["Page address is needed.".to_string()]);
        let mut given = Map::new();
        given.insert("url".into(), json!("http://x"));
        let r = t.resolve_inputs(&given).unwrap();
        assert_eq!(r["selector"], "main");
        assert_eq!(t.plain_rule(&r), "Check http://x and tell me when it changes");
    }

    #[test]
    fn placeholders_ignore_json_like_braces() {
        assert_eq!(placeholders("a {b} {c d} {} {e_1}"), vec!["b", "e_1"]);
    }
}
