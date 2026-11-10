//! `config/emotecraft.json`: Emotecraft's options (`CommonConfig`, `ClientConfig`), in the
//! order, with the defaults, ranges and categories the original has, and the client's emote
//! wheel (`fastmenu`, 15 pages of 8) and emote key binds (`keys`), each holding the emote
//! itself as a stored emote in Base64 (config version 5; older versions' UUIDs are read and
//! resolved once the emotes are loaded). Written as Gson's pretty printer writes it.
//!
//! This Source Code Form is a port of Emotecraft (https://github.com/KosmX/emotes), GPL-3.0.

use crate::anim::Animation;
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::Arc;

pub const CONFIG_VERSION: i64 = 5;
pub const PAGES: usize = 15;
pub const SLOTS: usize = 8;

/// An option's value.
#[derive(Clone, Debug, PartialEq)]
pub enum V {
    Bool(bool),
    Float(f32),
    Long(i64),
    Str(String),
    Enum(String),
}

/// What kind of option it is, and its range or values.
#[derive(Clone, Copy, Debug)]
pub enum Kind {
    Bool,
    Float(f32, f32),
    Long(i64, i64),
    Str,
    Enum(&'static [&'static str]),
}

/// One option (`ConfigEntry`).
#[derive(Clone, Copy, Debug)]
pub struct Entry {
    pub name: &'static str,
    pub old: Option<&'static str>,
    pub category: &'static str,
    pub kind: Kind,
    pub tooltip: bool,
    pub hidden: bool,
    /// Only on the client (`ClientConfig`'s own).
    pub client: bool,
}

pub const CLOSE_WHEEL: &[&str] = &["HOLD", "PRESS"];
pub const CAMERA_TYPES: &[&str] = &["FIRST_PERSON", "THIRD_PERSON_BACK", "THIRD_PERSON_FRONT"];
pub const LIBRARY_STATUS: &[&str] = &["DISABLED", "SHOW_FOLDER", "ENABLED"];

const fn e(name: &'static str, old: Option<&'static str>, category: &'static str, kind: Kind, tooltip: bool, hidden: bool, client: bool) -> Entry {
    Entry { name, old, category, kind, tooltip, hidden, client }
}

/// Every option, in the order the categories and fields are declared.
pub const ENTRIES: &[Entry] = &[
    e("validate", None, "expert", Kind::Bool, true, false, false),
    e("emotesFolderOnLogicalServer", None, "expert", Kind::Bool, true, true, false),
    e("emotesDirectory", None, "expert", Kind::Str, false, true, false),
    e("checkPose", None, "expert", Kind::Bool, true, false, true),
    e("normalizeSoundVolume", None, "expert", Kind::Bool, false, false, true),
    e("otherEmoteVolume", None, "expert", Kind::Float(0.0, 1.0), true, false, true),
    e("alwaysValidateEmote", None, "expert", Kind::Bool, true, false, true),
    e("playersafety", None, "expert", Kind::Bool, true, false, true),
    e("stopthreshold", Some("stopThreshold"), "expert", Kind::Float(-3.912, 8.0), true, false, true),
    e("yratio", Some("yRatio"), "expert", Kind::Float(0.0, 100.0), true, false, true),
    e("showHiddenConfig", None, "expert", Kind::Bool, true, false, true),
    e("onlineReconnectDelay", None, "expert", Kind::Long(1, 300), true, false, true),
    e("onlineDebug", None, "expert", Kind::Bool, true, false, true),
    e("loadbuiltin", Some("loadBuiltin"), "general", Kind::Bool, true, false, false),
    e("quark", Some("enablequark"), "general", Kind::Bool, true, false, false),
    e("closeWheelType", None, "general", Kind::Enum(CLOSE_WHEEL), true, false, true),
    e("enableWheelKeyboardNav", None, "general", Kind::Bool, false, false, true),
    e("cameraType", None, "general", Kind::Enum(CAMERA_TYPES), true, false, true),
    e("alwaysOpenScreen", None, "general", Kind::Bool, true, false, true),
    e("oldChooseWheel", None, "legacy", Kind::Bool, false, false, true),
    e("showIconsIfPossible", None, "legacy", Kind::Bool, false, false, true),
    e("cloudLibraryStatus", None, "library", Kind::Enum(LIBRARY_STATUS), false, false, true),
    e("libraryNotifications", None, "library", Kind::Bool, true, false, true),
];

pub const CATEGORIES: &[&str] = &["expert", "general", "legacy", "library"];

/// An option's default; `emotesFolderOnLogicalServer` is on for a server and off for a client.
pub fn default_of(name: &str, client: bool) -> V {
    match name {
        "validate" | "quark" | "alwaysValidateEmote" | "showHiddenConfig" | "onlineDebug" | "oldChooseWheel" | "showIconsIfPossible" | "alwaysOpenScreen" => V::Bool(false),
        "emotesFolderOnLogicalServer" => V::Bool(!client),
        "emotesDirectory" => V::Str("emotes".into()),
        "checkPose" | "normalizeSoundVolume" | "playersafety" | "loadbuiltin" | "enableWheelKeyboardNav" | "libraryNotifications" => V::Bool(true),
        "otherEmoteVolume" => V::Float(1.0),
        "stopthreshold" => V::Float(0.04),
        "yratio" => V::Float(0.75),
        "onlineReconnectDelay" => V::Long(15),
        "closeWheelType" => V::Enum("PRESS".into()),
        "cameraType" => V::Enum("THIRD_PERSON_FRONT".into()),
        "cloudLibraryStatus" => V::Enum("SHOW_FOLDER".into()),
        _ => V::Bool(false),
    }
}

pub fn entry(name: &str) -> Option<&'static Entry> {
    ENTRIES.iter().find(|e| e.name == name)
}

/// The options, the wheel and the key binds.
#[derive(Clone)]
pub struct Config {
    pub client: bool,
    pub version: i64,
    values: BTreeMap<&'static str, V>,
    pub fast_menu: Vec<[Option<Arc<Animation>>; SLOTS]>,
    /// Key name (`key.keyboard.k`, `key.mouse.4`) → the emote.
    pub keys: BTreeMap<String, Arc<Animation>>,
    /// An old config's bindings by emote id, until the emotes are loaded.
    pub legacy_keys: Option<BTreeMap<String, u128>>,
    pub legacy_fast_menu: Option<Vec<[Option<u128>; SLOTS]>>,
    /// Changed since read or saved.
    pub dirty: bool,
}

impl Config {
    pub fn new(client: bool) -> Config {
        Config {
            client,
            version: CONFIG_VERSION,
            values: ENTRIES.iter().filter(|e| client || !e.client).map(|e| (e.name, default_of(e.name, client))).collect(),
            fast_menu: (0..PAGES).map(|_| Default::default()).collect(),
            keys: BTreeMap::new(),
            legacy_keys: None,
            legacy_fast_menu: None,
            dirty: false,
        }
    }

    /// `NumberConfigEntry.get`: kept in range.
    pub fn get(&self, name: &str) -> V {
        let v = self.values.get(name).cloned().unwrap_or_else(|| default_of(name, self.client));
        match (entry(name).map(|e| e.kind), v) {
            (Some(Kind::Float(lo, hi)), V::Float(f)) => V::Float(f.clamp(lo, hi)),
            (Some(Kind::Long(lo, hi)), V::Long(l)) => V::Long(l.clamp(lo, hi)),
            (_, v) => v,
        }
    }
    pub fn b(&self, name: &str) -> bool {
        matches!(self.get(name), V::Bool(true))
    }
    pub fn f(&self, name: &str) -> f32 {
        match self.get(name) {
            V::Float(f) => f,
            V::Long(l) => l as f32,
            _ => 0.0,
        }
    }
    pub fn s(&self, name: &str) -> String {
        match self.get(name) {
            V::Str(s) | V::Enum(s) => s,
            V::Bool(b) => b.to_string(),
            V::Float(f) => java_float(f),
            V::Long(l) => l.to_string(),
        }
    }
    pub fn set(&mut self, name: &str, v: V) {
        if let Some(e) = entry(name) {
            self.values.insert(e.name, v);
            self.dirty = true;
        }
    }
    /// `iterate(resetToDefault)`: every option back to its default (the wheel and keys stay).
    pub fn reset(&mut self) {
        for e in ENTRIES {
            if self.client || !e.client {
                self.values.insert(e.name, default_of(e.name, self.client));
            }
        }
        self.dirty = true;
    }

    /// `ConfigSerializer.deserialize` and `ClientConfigSerializer.clientDeserialize`.
    pub fn parse(text: &str, client: bool) -> Result<Config, String> {
        let json: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        let Value::Object(node) = json else { return Err("Json is empty".into()) };
        let mut c = Config::new(client);
        c.version = node.get("config_version").and_then(Value::as_i64).unwrap_or(CONFIG_VERSION);
        if c.version > CONFIG_VERSION {
            log::warn!("[Emotecraft] You are trying to load version {} config. The mod can only load correctly up to {CONFIG_VERSION}.", c.version);
        }
        for e in ENTRIES.iter().filter(|e| client || !e.client) {
            let Some(v) = node.get(e.name).or_else(|| e.old.and_then(|o| node.get(o))) else { continue };
            let parsed = match e.kind {
                Kind::Bool => v.as_bool().or_else(|| v.as_str().map(|s| s.eq_ignore_ascii_case("true"))).map(V::Bool),
                Kind::Float(..) => v.as_f64().or_else(|| v.as_str().and_then(|s| s.parse().ok())).map(|f| V::Float(f as f32)),
                Kind::Long(..) => v.as_f64().or_else(|| v.as_str().and_then(|s| s.parse().ok())).map(|f| V::Long(f as i64)),
                Kind::Str => v.as_str().map(|s| V::Str(s.to_owned())),
                Kind::Enum(names) => v.as_str().filter(|s| names.contains(s)).map(|s| V::Enum(s.to_owned())),
            };
            if let Some(p) = parsed {
                c.values.insert(e.name, p);
            }
        }
        if client {
            if let Some(Value::Object(fm)) = node.get("fastmenu") {
                let legacy = c.version < 5;
                let mut legacy_menu: Vec<[Option<u128>; SLOTS]> = (0..PAGES).map(|_| Default::default()).collect();
                for j in 0..PAGES {
                    let Some(sub) = fm.get(&j.to_string()) else { continue };
                    match sub {
                        Value::Object(page) => {
                            for i in 0..SLOTS {
                                let Some(slot) = page.get(&i.to_string()) else { continue };
                                if legacy {
                                    legacy_menu[j][i] = slot.as_str().and_then(crate::buf::parse_uuid);
                                } else {
                                    c.fast_menu[j][i] = slot.as_str().and_then(read_emote);
                                }
                            }
                        }
                        other if legacy => legacy_menu[0][j.min(SLOTS - 1)] = other.as_str().and_then(crate::buf::parse_uuid),
                        _ => {}
                    }
                }
                if legacy {
                    c.legacy_fast_menu = Some(legacy_menu);
                }
            }
            if let Some(keys) = node.get("keys") {
                if c.version < 5 {
                    let mut map = BTreeMap::new();
                    match keys {
                        Value::Array(list) if c.version < 4 => {
                            for n in list {
                                if let (Some(k), Some(id)) = (n.get("key").and_then(Value::as_str), n.get("id").and_then(Value::as_str)) {
                                    map.insert(k.to_owned(), crate::buf::parse_uuid(id).unwrap_or(0));
                                }
                            }
                        }
                        Value::Object(o) => {
                            for (id, k) in o {
                                if let (Some(k), Some(u)) = (k.as_str(), crate::buf::parse_uuid(id)) {
                                    map.insert(k.to_owned(), u);
                                }
                            }
                        }
                        _ => {}
                    }
                    c.legacy_keys = Some(map);
                } else if let Value::Object(o) = keys {
                    for (k, v) in o {
                        if let Some(a) = v.as_str().and_then(read_emote) {
                            c.keys.insert(k.clone(), a);
                        }
                    }
                }
            }
        }
        Ok(c)
    }

    /// `migrateLegacyBinds`: an old config's ids, to the emotes loaded. True if it changed.
    pub fn migrate(&mut self, find: impl Fn(u128) -> Option<Arc<Animation>>) -> bool {
        let mut changed = false;
        if let Some(keys) = self.legacy_keys.take() {
            for (k, id) in keys {
                if let Some(a) = find(id) {
                    self.keys.insert(k, a);
                }
            }
            changed = true;
        }
        if let Some(menu) = self.legacy_fast_menu.take() {
            for (j, page) in menu.iter().enumerate().take(PAGES) {
                for (i, id) in page.iter().enumerate() {
                    if let Some(a) = id.and_then(&find) {
                        self.fast_menu[j][i] = Some(a);
                    }
                }
            }
            changed = true;
        }
        if changed {
            self.dirty = true;
        }
        changed
    }

    /// `ConfigSerializer.serialize` (and the client's wheel and keys), as Gson writes it.
    pub fn write(&self) -> String {
        let mut out = String::from("{\n");
        out.push_str(&format!("  \"config_version\": {CONFIG_VERSION}"));
        for e in ENTRIES.iter().filter(|e| self.client || !e.client) {
            let v = self.values.get(e.name).cloned().unwrap_or_else(|| default_of(e.name, self.client));
            if e.hidden && v == default_of(e.name, self.client) {
                continue;
            }
            let text = match v {
                V::Bool(b) => b.to_string(),
                V::Float(f) => java_float(f),
                V::Long(l) => l.to_string(),
                V::Str(s) | V::Enum(s) => json_string(&s),
            };
            out.push_str(&format!(",\n  {}: {text}", json_string(e.name)));
        }
        if self.client {
            out.push_str(",\n  \"fastmenu\": {");
            let mut first_page = true;
            for (j, page) in self.fast_menu.iter().enumerate() {
                let slots: Vec<(usize, &Arc<Animation>)> = page.iter().enumerate().filter_map(|(i, s)| s.as_ref().map(|a| (i, a))).collect();
                if slots.is_empty() {
                    continue;
                }
                out.push_str(if first_page { "\n" } else { ",\n" });
                first_page = false;
                out.push_str(&format!("    \"{j}\": {{"));
                for (n, (i, a)) in slots.iter().enumerate() {
                    out.push_str(if n == 0 { "\n" } else { ",\n" });
                    out.push_str(&format!("      \"{i}\": \"{}\"", crate::buf::base64_encode(&crate::packet::emote_to_bytes(a))));
                }
                out.push_str("\n    }");
            }
            out.push_str(if first_page { "}" } else { "\n  }" });
            out.push_str(",\n  \"keys\": {");
            for (n, (k, a)) in self.keys.iter().enumerate() {
                out.push_str(if n == 0 { "\n" } else { ",\n" });
                out.push_str(&format!("    {}: \"{}\"", json_string(k), crate::buf::base64_encode(&crate::packet::emote_to_bytes(a))));
            }
            out.push_str(if self.keys.is_empty() { "}" } else { "\n  }" });
        }
        out.push_str("\n}");
        out
    }
}

fn read_emote(s: &str) -> Option<Arc<Animation>> {
    let bytes = crate::buf::base64_decode(s)?;
    let a = crate::packet::emote_from_bytes(&bytes);
    if a.is_none() {
        log::warn!("[Emotecraft] Failed to read a bound emote!");
    }
    a
}

/// `Float.toString`.
pub fn java_float(f: f32) -> String {
    if f.fract() == 0.0 && f.abs() < 1e7 { format!("{f:.1}") } else { format!("{f}") }
}

/// A JSON string as Gson writes it (`disableHtmlEscaping`).
pub fn json_string(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_round_trip() {
        let c = Config::new(true);
        let text = c.write();
        assert!(text.contains("\"stopthreshold\": 0.04"), "{text}");
        assert!(text.contains("\"cameraType\": \"THIRD_PERSON_FRONT\""));
        assert!(!text.contains("emotesDirectory"));
        let back = Config::parse(&text, true).unwrap();
        assert_eq!(back.f("yratio"), 0.75);
        assert_eq!(back.s("closeWheelType"), "PRESS");
        let server = Config::new(false);
        assert!(!server.write().contains("cameraType"));
        assert!(server.b("emotesFolderOnLogicalServer"));
    }

    #[test]
    fn keys_and_wheel_round_trip() {
        let a = Arc::new(crate::loader::load_json(crate::tests::BUILT_IN_JSON[1]).unwrap().remove(0));
        let mut c = Config::new(true);
        c.fast_menu[2][5] = Some(a.clone());
        c.keys.insert("key.keyboard.k".into(), a.clone());
        let back = Config::parse(&c.write(), true).unwrap();
        assert_eq!(back.fast_menu[2][5].as_ref().unwrap().uuid, a.uuid);
        assert_eq!(back.keys["key.keyboard.k"].uuid, a.uuid);
    }

    #[test]
    fn old_names_and_ranges() {
        let c = Config::parse(r#"{"config_version":3,"stopThreshold":20,"loadBuiltin":false}"#, true).unwrap();
        assert_eq!(c.f("stopthreshold"), 8.0);
        assert!(!c.b("loadbuiltin"));
    }
}
