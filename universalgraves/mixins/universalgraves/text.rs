// The texts of Universal Graves: the config's Simplified Text Format
// (`<red>`, `<#ffd257>`, `<lang:'key':'arg'>`, `</>`, `${placeholder}`), as
// Placeholder API's `NodeParser` reads it, turned into vanilla text components;
// and the mod's translations, which the original serves from the server
// (Server Translations API): a `translate` component carries the player's own
// language as its fallback, so a client without the keys reads it too.
// Ported from Universal Graves by Patbox (LGPL-3.0).
use serde_json::{Map, Value, json};
use std::collections::HashMap;
use std::sync::OnceLock;

/// The mod's languages, as the original ships them (`data/universal_graves/lang`).
const LANGS: &[(&str, &[u8])] = &[
    ("ca_es", horizon_client::native_data!("universalgraves", "lang/ca_es.json")),
    ("cs_cz", horizon_client::native_data!("universalgraves", "lang/cs_cz.json")),
    ("de_de", horizon_client::native_data!("universalgraves", "lang/de_de.json")),
    ("en_us", horizon_client::native_data!("universalgraves", "lang/en_us.json")),
    ("eo_eo", horizon_client::native_data!("universalgraves", "lang/eo_eo.json")),
    ("es_ar", horizon_client::native_data!("universalgraves", "lang/es_ar.json")),
    ("es_cl", horizon_client::native_data!("universalgraves", "lang/es_cl.json")),
    ("es_ec", horizon_client::native_data!("universalgraves", "lang/es_ec.json")),
    ("es_es", horizon_client::native_data!("universalgraves", "lang/es_es.json")),
    ("es_mx", horizon_client::native_data!("universalgraves", "lang/es_mx.json")),
    ("es_uy", horizon_client::native_data!("universalgraves", "lang/es_uy.json")),
    ("es_ve", horizon_client::native_data!("universalgraves", "lang/es_ve.json")),
    ("fr_fr", horizon_client::native_data!("universalgraves", "lang/fr_fr.json")),
    ("it_it", horizon_client::native_data!("universalgraves", "lang/it_it.json")),
    ("ja_jp", horizon_client::native_data!("universalgraves", "lang/ja_jp.json")),
    ("ko_kr", horizon_client::native_data!("universalgraves", "lang/ko_kr.json")),
    ("pl_pl", horizon_client::native_data!("universalgraves", "lang/pl_pl.json")),
    ("pt_br", horizon_client::native_data!("universalgraves", "lang/pt_br.json")),
    ("pt_pt", horizon_client::native_data!("universalgraves", "lang/pt_pt.json")),
    ("ru_ru", horizon_client::native_data!("universalgraves", "lang/ru_ru.json")),
    ("uk_ua", horizon_client::native_data!("universalgraves", "lang/uk_ua.json")),
    ("zh_cn", horizon_client::native_data!("universalgraves", "lang/zh_cn.json")),
    ("zh_hk", horizon_client::native_data!("universalgraves", "lang/zh_hk.json")),
    ("zh_tw", horizon_client::native_data!("universalgraves", "lang/zh_tw.json")),
];

fn tables() -> &'static HashMap<&'static str, HashMap<String, String>> {
    static TABLES: OnceLock<HashMap<&'static str, HashMap<String, String>>> = OnceLock::new();
    TABLES.get_or_init(|| {
        LANGS
            .iter()
            .map(|(code, bytes)| {
                let map: HashMap<String, String> = serde_json::from_slice(bytes).unwrap_or_default();
                (*code, map)
            })
            .collect()
    })
}

/// `key` in `lang` (`ru_ru`), else in English (Server Translations' fallback).
pub(super) fn translation(lang: &str, key: &str) -> Option<String> {
    let t = tables();
    let lang = lang.to_ascii_lowercase();
    t.get(lang.as_str())
        .and_then(|m| m.get(key))
        .or_else(|| t.get("en_us").and_then(|m| m.get(key)))
        .cloned()
}

/// A translated component: the key, its arguments, and the template in `lang`
/// as the fallback a client without the key shows.
pub(super) fn translatable(lang: &str, key: &str, args: Vec<Value>) -> Value {
    let mut o = Map::new();
    o.insert("translate".into(), key.into());
    if let Some(t) = translation(lang, key) {
        o.insert("fallback".into(), t.into());
    }
    if !args.is_empty() {
        o.insert("with".into(), Value::Array(args));
    }
    Value::Object(o)
}

pub(super) fn literal(s: &str) -> Value {
    json!({ "text": s })
}

/// A parsed text: what `WrappedText.of` keeps.
#[derive(Clone, Debug)]
enum Node {
    Text(String),
    Placeholder(String),
    Styled(Map<String, Value>, Vec<Node>),
    Lang(String, Vec<Vec<Node>>),
}

/// The colour names Simplified Text Format knows (`TextColor`'s).
const COLORS: &[&str] = &[
    "black", "dark_blue", "dark_green", "dark_aqua", "dark_red", "dark_purple", "gold", "gray", "dark_gray", "blue",
    "green", "aqua", "red", "light_purple", "yellow", "white",
];

/// The style a tag opens, or `None` for a tag the format doesn't know.
fn tag_style(name: &str, arg: Option<&str>) -> Option<Map<String, Value>> {
    let mut m = Map::new();
    let color = |c: &str| -> Option<String> {
        let c = c.trim();
        if COLORS.contains(&c) {
            Some(c.to_owned())
        } else if c == "grey" {
            Some("gray".into())
        } else if c == "dark_grey" {
            Some("dark_gray".into())
        } else if let Some(hex) = c.strip_prefix('#') {
            (hex.len() == 6 && hex.chars().all(|ch| ch.is_ascii_hexdigit())).then(|| format!("#{}", hex.to_ascii_lowercase()))
        } else {
            None
        }
    };
    match name {
        "bold" | "b" => {
            m.insert("bold".into(), true.into());
        }
        "italic" | "i" | "em" => {
            m.insert("italic".into(), true.into());
        }
        "underline" | "underlined" | "u" => {
            m.insert("underlined".into(), true.into());
        }
        "strikethrough" | "st" => {
            m.insert("strikethrough".into(), true.into());
        }
        "obfuscated" | "obf" => {
            m.insert("obfuscated".into(), true.into());
        }
        "color" | "colour" | "c" => {
            m.insert("color".into(), color(arg?)?.into());
        }
        _ => {
            m.insert("color".into(), color(name)?.into());
        }
    }
    Some(m)
}

/// Splits `'a':'b':c` into its arguments, minding quotes and nested tags.
fn split_args(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut depth = 0i32;
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match (quote, c) {
            (Some(_), '\\') => {
                if let Some(n) = chars.next() {
                    cur.push(n);
                }
            }
            (Some(q), c) if c == q && depth == 0 => quote = None,
            (Some(_), '<') => {
                depth += 1;
                cur.push(c);
            }
            (Some(_), '>') => {
                depth -= 1;
                cur.push(c);
            }
            (Some(_), c) => cur.push(c),
            (None, '\'' | '"' | '`') => quote = Some(c),
            (None, ':') => out.push(std::mem::take(&mut cur)),
            (None, c) => cur.push(c),
        }
    }
    out.push(cur);
    out
}

/// The end of the tag that opens at `start` (`<`), minding quoted arguments.
fn tag_end(s: &[char], start: usize) -> Option<usize> {
    let mut quote: Option<char> = None;
    let mut depth = 0;
    let mut i = start + 1;
    while i < s.len() {
        let c = s[i];
        match quote {
            Some(q) => {
                if c == '\\' {
                    i += 1;
                } else if c == '<' {
                    depth += 1;
                } else if c == '>' && depth > 0 {
                    depth -= 1;
                } else if c == q && depth == 0 {
                    quote = None;
                }
            }
            None => match c {
                '\'' | '"' | '`' => quote = Some(c),
                '>' => return Some(i),
                '<' => return None,
                _ => {}
            },
        }
        i += 1;
    }
    None
}

fn parse(input: &str) -> Vec<Node> {
    let chars: Vec<char> = input.chars().collect();
    // Open groups: (tag name, style, children).
    let mut stack: Vec<(String, Map<String, Value>, Vec<Node>)> = vec![(String::new(), Map::new(), Vec::new())];
    let mut text = String::new();
    let flush = |stack: &mut Vec<(String, Map<String, Value>, Vec<Node>)>, text: &mut String| {
        if !text.is_empty() {
            stack.last_mut().unwrap().2.push(Node::Text(std::mem::take(text)));
        }
    };
    let close = |stack: &mut Vec<(String, Map<String, Value>, Vec<Node>)>| {
        if stack.len() > 1 {
            let (_, style, children) = stack.pop().unwrap();
            stack.last_mut().unwrap().2.push(Node::Styled(style, children));
        }
    };
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' && i + 1 < chars.len() {
            text.push(chars[i + 1]);
            i += 2;
            continue;
        }
        if c == '$' && chars.get(i + 1) == Some(&'{') {
            if let Some(end) = chars[i + 2..].iter().position(|&ch| ch == '}') {
                flush(&mut stack, &mut text);
                let name: String = chars[i + 2..i + 2 + end].iter().collect();
                stack.last_mut().unwrap().2.push(Node::Placeholder(name));
                i += end + 3;
                continue;
            }
        }
        if c == '<'
            && let Some(end) = tag_end(&chars, i)
        {
            let inner: String = chars[i + 1..end].iter().collect();
            if let Some(name) = inner.strip_prefix('/') {
                flush(&mut stack, &mut text);
                let name = name.trim();
                if name.is_empty() {
                    close(&mut stack);
                } else if let Some(at) = stack.iter().rposition(|g| g.0 == name) {
                    while stack.len() > at {
                        close(&mut stack);
                    }
                }
                i = end + 1;
                continue;
            }
            let (name, rest) = match inner.find(':') {
                Some(k) => (inner[..k].trim().to_ascii_lowercase(), Some(inner[k + 1..].to_owned())),
                None => (inner.trim().to_ascii_lowercase(), None),
            };
            if name == "r" || name == "reset" {
                flush(&mut stack, &mut text);
                while stack.len() > 1 {
                    close(&mut stack);
                }
                i = end + 1;
                continue;
            }
            if matches!(name.as_str(), "lang" | "translate" | "tr") {
                if let Some(rest) = rest {
                    flush(&mut stack, &mut text);
                    let mut args = split_args(&rest).into_iter();
                    let key = args.next().unwrap_or_default();
                    let args = args.map(|a| parse(&a)).collect();
                    stack.last_mut().unwrap().2.push(Node::Lang(key, args));
                    i = end + 1;
                    continue;
                }
            }
            let arg = rest.as_deref().map(|r| split_args(r).into_iter().next().unwrap_or_default());
            if let Some(style) = tag_style(&name, arg.as_deref()) {
                flush(&mut stack, &mut text);
                stack.push((name, style, Vec::new()));
                i = end + 1;
                continue;
            }
        }
        text.push(c);
        i += 1;
    }
    flush(&mut stack, &mut text);
    while stack.len() > 1 {
        close(&mut stack);
    }
    stack.pop().map(|g| g.2).unwrap_or_default()
}

/// A config text (`WrappedText`): its input and parsed form.
#[derive(Clone, Debug)]
pub(super) struct Wrapped {
    pub input: String,
    nodes: Vec<Node>,
}

impl Wrapped {
    pub fn of(input: &str) -> Self {
        Self { input: input.to_owned(), nodes: parse(input) }
    }
    pub fn is_empty(&self) -> bool {
        self.input.is_empty()
    }
    /// `WrappedText.with(placeholders)` in `lang`, as a component.
    pub fn with(&self, lang: &str, placeholders: &dyn Fn(&str) -> Option<Value>) -> Value {
        root(render(&self.nodes, lang, placeholders))
    }
    pub fn text(&self, lang: &str) -> Value {
        self.with(lang, &|_| None)
    }
}

fn render(nodes: &[Node], lang: &str, ph: &dyn Fn(&str) -> Option<Value>) -> Vec<Value> {
    nodes
        .iter()
        .map(|n| match n {
            Node::Text(s) => literal(s),
            Node::Placeholder(name) => ph(name).unwrap_or_else(|| literal("")),
            Node::Styled(style, children) => {
                let mut o = style.clone();
                o.insert("text".into(), "".into());
                let extra = render(children, lang, ph);
                if !extra.is_empty() {
                    o.insert("extra".into(), Value::Array(extra));
                }
                Value::Object(o)
            }
            Node::Lang(key, args) => translatable(lang, key, args.iter().map(|a| root(render(a, lang, ph))).collect()),
        })
        .collect()
}

/// One component for several: an empty text holding them, unstyled and not
/// italic (as `Placeholders` leaves item names and lore).
fn root(parts: Vec<Value>) -> Value {
    match parts.len() {
        0 => literal(""),
        1 => parts.into_iter().next().unwrap(),
        _ => json!({ "text": "", "extra": parts }),
    }
}

/// A component for a lore line or item name: not italic unless it says so.
pub(super) fn not_italic(v: Value) -> Value {
    json!({ "text": "", "italic": false, "color": "white", "extra": [v] })
}

/// The plain text of a component, for the tests.
#[cfg(test)]
pub(super) fn plain(v: &Value, lang: &str) -> String {
    let mut out = String::new();
    fn walk(v: &Value, lang: &str, out: &mut String) {
        match v {
            Value::String(s) => out.push_str(s),
            Value::Array(a) => a.iter().for_each(|x| walk(x, lang, out)),
            Value::Object(o) => {
                if let Some(Value::String(t)) = o.get("text") {
                    out.push_str(t);
                }
                if let Some(Value::String(key)) = o.get("translate") {
                    let template = o
                        .get("fallback")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                        .or_else(|| translation(lang, key))
                        .unwrap_or_else(|| key.clone());
                    let args: Vec<String> = o
                        .get("with")
                        .and_then(Value::as_array)
                        .map(|a| {
                            a.iter()
                                .map(|x| {
                                    let mut s = String::new();
                                    walk(x, lang, &mut s);
                                    s
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    let mut k = 0;
                    let mut rest = template.as_str();
                    while let Some(p) = rest.find("%s") {
                        out.push_str(&rest[..p]);
                        out.push_str(args.get(k).map_or("", String::as_str));
                        k += 1;
                        rest = &rest[p + 2..];
                    }
                    out.push_str(rest);
                }
                if let Some(e) = o.get("extra") {
                    walk(e, lang, out);
                }
            }
            _ => {}
        }
    }
    walk(v, lang, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_lang_and_placeholders() {
        let w = Wrapped::of("<red><lang:'text.graves.creation_failed':'<gold>${position}':'<yellow>${world}'>");
        let v = w.with("en_us", &|n| Some(literal(match n {
            "position" => "1, 2, 3",
            _ => "Overworld",
        })));
        assert_eq!(v["color"], "red");
        let tr = &v["extra"][0];
        assert_eq!(tr["translate"], "text.graves.creation_failed");
        assert_eq!(plain(&v, "en_us"), "Couldn't create grave! Your items were dropped at 1, 2, 3 (Overworld)");
    }

    #[test]
    fn closing_tags() {
        let w = Wrapped::of("<dark_gray>[<white>${player}</>]</> ${position} <gray>(${world})");
        let v = w.with("en_us", &|n| Some(literal(n)));
        assert_eq!(plain(&v, "en_us"), "[player] position (world)");
    }
}
