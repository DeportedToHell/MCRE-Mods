//! An emote's name, description or author as plain text (`Component.getString`): a JSON text
//! component (`translate`, `text`, `extra`, `fallback`, a player object) or a plain string, as
//! `McUtils.fromJson` reads it. The game draws them as components itself; this is for the
//! commands, searching and sorting.

use serde_json::Value;

/// The plain text of a component; `translate` keys through `resolve`.
pub fn plain(raw: &str, resolve: &dyn Fn(&str) -> Option<String>) -> String {
    if raw.trim().is_empty() {
        return String::new();
    }
    match serde_json::from_str::<Value>(raw) {
        Ok(v) => plain_value(&v, resolve),
        Err(_) => raw.to_owned(),
    }
}

pub fn plain_value(v: &Value, resolve: &dyn Fn(&str) -> Option<String>) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Array(list) => list.iter().map(|x| plain_value(x, resolve)).collect(),
        Value::Object(o) => {
            let mut s = if let Some(t) = o.get("text").and_then(Value::as_str) {
                t.to_owned()
            } else if let Some(k) = o.get("translate").and_then(Value::as_str) {
                resolve(k).or_else(|| o.get("fallback").and_then(Value::as_str).map(str::to_owned)).unwrap_or_else(|| k.to_owned())
            } else if o.contains_key("player") {
                String::new()
            } else {
                String::new()
            };
            if let Some(Value::Array(extra)) = o.get("extra") {
                for x in extra {
                    s.push_str(&plain_value(x, resolve));
                }
            }
            s
        }
        Value::Null => String::new(),
    }
}

/// `StringUtil.filterText`: the characters a chat line allows (no `§`, no control characters).
pub fn filter(s: &str) -> String {
    s.chars().filter(|c| *c != '§' && *c >= ' ' && *c != '\u{7f}').collect()
}

/// Legacy formatting codes (`§7`) removed.
pub fn strip_codes(s: &str) -> String {
    let mut out = String::new();
    let mut skip = false;
    for c in s.chars() {
        if skip {
            skip = false;
            continue;
        }
        if c == '§' {
            skip = true;
            continue;
        }
        out.push(c);
    }
    out
}
