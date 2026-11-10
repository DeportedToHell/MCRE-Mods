// Emotecraft's texts in the game's language (`crate::game::emotecraft_lang`): the original's
// en_US and ru_RU (engine/src/lang.rs), and an emote's name, description or author as the game
// draws text components, its `emotecraft.*` keys put in.
//
// This Source Code Form is a port of Emotecraft (https://github.com/KosmX/emotes), GPL-3.0.

use horizon_client::text::Text;
use serde_json::Value;
use std::sync::Mutex;

static LANGUAGE: Mutex<String> = Mutex::new(String::new());

/// The game's language now (set once a frame).
pub(super) fn set_language(lang: &str) {
    let mut l = super::emotecraft_glue::lock(&LANGUAGE);
    if *l != lang {
        *l = lang.to_owned();
    }
}

fn language() -> String {
    super::emotecraft_glue::lock(&LANGUAGE).clone()
}

pub(super) fn russian() -> bool {
    language().eq_ignore_ascii_case("ru_ru")
}

/// A key's text, if Emotecraft has it.
pub(super) fn text(key: &str) -> Option<String> {
    emotecraft_engine::lang::get(&language(), key).map(str::to_owned)
}

/// A key's text (the key itself if unknown).
pub(super) fn t(key: &str) -> String {
    text(key).unwrap_or_else(|| key.to_owned())
}

pub(super) fn get_en(key: &str) -> &'static str {
    emotecraft_engine::lang::get("en_us", key).unwrap_or_default()
}

/// `String.format` for `%s` and `%1$s`.
pub(super) fn tf(key: &str, args: &[&str]) -> String {
    let mut s = t(key);
    for (i, a) in args.iter().enumerate() {
        s = s.replace(&format!("%{}$s", i + 1), a);
    }
    for a in args {
        if let Some(at) = s.find("%s") {
            s.replace_range(at..at + 2, a);
        }
    }
    s
}

/// `{"translate": "emotecraft..."}` replaced by its text (with its style kept), so the game's
/// text drawing, which knows only its own keys, shows it.
fn resolve(v: &mut Value) {
    match v {
        Value::Object(o) => {
            if let Some(key) = o.get("translate").and_then(Value::as_str).map(str::to_owned)
                && let Some(text) = text(&key)
            {
                o.remove("translate");
                o.remove("with");
                o.insert("text".into(), Value::String(text));
            }
            for (_, x) in o.iter_mut() {
                resolve(x);
            }
        }
        Value::Array(a) => a.iter_mut().for_each(resolve),
        _ => {}
    }
}

/// `McUtils.fromJson`: a JSON text component, or a plain string.
pub(super) fn component(raw: &str) -> Text {
    if raw.trim().is_empty() {
        return Text::empty();
    }
    match serde_json::from_str::<Value>(raw) {
        Ok(mut v) => {
            resolve(&mut v);
            horizon_client::component::to_text(&v, &mut |s| Text::literal(s))
        }
        Err(_) => Text::legacy(raw),
    }
}

/// A component's plain text.
pub(super) fn plain(raw: &str) -> String {
    emotecraft_engine::text::plain(raw, &|k| text(k))
}
