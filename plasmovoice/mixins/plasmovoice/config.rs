// Plasmo Voice's client config (`VoiceClientConfig`, `ConfigHotkeys`), kept in
// `config/plasmovoice/client.toml` as Plasmo Voice keeps it: the Java fields in snake case and in
// their order, the maps (`SourceLineVolumes`, `Activations`, `SourceStates`, `Servers`) as
// tables sorted by key, the hotkeys a player changed as `[[key_bindings]]`.
//
// Values are kept by their path (`voice.microphone_volume`): `get` gives the default of one never
// set (`ConfigEntry.getDefault`), `set` keeps it, `created` lists the map entries Plasmo Voice
// would have made (`computeIfAbsent`) so they are written as it writes them.
//
// This Source Code Form is a port of Plasmo Voice (https://github.com/plasmoapp/plasmo-voice), LGPL-3.0.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;

#[derive(Clone, Debug, PartialEq)]
pub(super) enum V {
    B(bool),
    F(f64),
    I(i64),
    S(String),
}

impl V {
    pub(super) fn b(&self) -> bool {
        matches!(self, V::B(true))
    }
    pub(super) fn f(&self) -> f64 {
        match self {
            V::F(f) => *f,
            V::I(i) => *i as f64,
            _ => 0.0,
        }
    }
    pub(super) fn i(&self) -> i64 {
        match self {
            V::I(i) => *i,
            V::F(f) => *f as i64,
            _ => 0,
        }
    }
    pub(super) fn s(&self) -> &str {
        match self {
            V::S(s) => s,
            _ => "",
        }
    }
    fn toml(&self) -> String {
        match self {
            V::B(b) => b.to_string(),
            // Java's `Double.toString`: always a fraction.
            V::F(f) if f.fract() == 0.0 && f.abs() < 1e7 => format!("{f:.1}"),
            V::F(f) => f.to_string(),
            V::I(i) => i.to_string(),
            V::S(s) => quote(s),
        }
    }
}

fn quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04X}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A TOML key: bare when it can be.
fn key(s: &str) -> String {
    if !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') { s.to_owned() } else { quote(s) }
}

/// The fixed fields, by section, in `VoiceClientConfig`'s order, with their defaults and ranges.
const ROOT: [(&str, V); 3] = [("debug", V::B(false)), ("disable_crowdin", V::B(false)), ("check_for_updates", V::B(true))];

fn voice() -> Vec<(&'static str, V)> {
    vec![
        ("disabled", V::B(false)),
        ("microphone_disabled", V::B(false)),
        // This port's default (the user's choice): Plasmo Voice's is -30 dB.
        ("activation_threshold", V::F(-60.0)),
        ("input_device", V::S(String::new())),
        ("output_device", V::S(String::new())),
        ("disable_input_device", V::B(false)),
        ("use_javax_input", V::B(false)),
        ("input_backend", V::S("AUTO".into())),
        ("microphone_volume", V::F(1.0)),
        ("noise_suppression", V::B(false)),
        ("volume", V::F(1.0)),
        ("compressor_limiter", V::B(true)),
        ("sound_occlusion", V::B(false)),
        ("directional_sources", V::B(false)),
        ("hrtf", V::B(false)),
        ("stereo_capture", V::B(false)),
        ("show_all_source_lines", V::B(false)),
    ]
}

fn advanced() -> Vec<(&'static str, V)> {
    vec![
        ("visualize_voice_distance", V::B(true)),
        ("visualize_voice_distance_on_join", V::B(false)),
        ("compressor_threshold", V::I(-10)),
        ("limiter_threshold", V::I(-6)),
        ("directional_sources_angle", V::I(145)),
        ("stereo_sources_to_mono", V::B(false)),
        ("panning", V::B(true)),
        ("source_types_overlap", V::S("MUTE_PROXIMITY".into())),
        ("camera_sound_listener", V::B(true)),
        ("exponential_volume_slider", V::B(true)),
        ("exponential_distance_gain", V::B(true)),
        ("jitter_packet_delay", V::I(2)),
        ("al_playback_buffers", V::I(5)),
        ("adaptive_jitter_buffer", V::B(false)),
    ]
}

/// Horizon's own settings, not Plasmo Voice's: `[horizon]` after everything Plasmo Voice writes.
fn horizon() -> Vec<(&'static str, V)> {
    vec![("tab_list_icons", V::B(true))]
}

fn overlay() -> Vec<(&'static str, V)> {
    vec![
        ("show_activation_icon", V::B(true)),
        ("activation_icon_position", V::S("BOTTOM_CENTER".into())),
        ("show_source_icons", V::I(0)),
        ("show_static_source_icons", V::B(true)),
        ("overlay_enabled", V::B(true)),
        ("overlay_position", V::S("TOP_LEFT".into())),
        ("overlay_style", V::S("NAME_SKIN".into())),
    ]
}

/// `(min, max)` of a number field (`DoubleConfigEntry`/`IntConfigEntry` bounds): values are
/// clamped to them as `set` does.
fn range(path: &str) -> Option<(f64, f64)> {
    Some(match path {
        "voice.activation_threshold" => (-60.0, 0.0),
        "voice.microphone_volume" | "voice.volume" => (0.0, 2.0),
        "advanced.compressor_threshold" | "advanced.limiter_threshold" => (-60.0, 0.0),
        "advanced.directional_sources_angle" => (100.0, 360.0),
        "advanced.jitter_packet_delay" => (0.0, 16.0),
        "advanced.al_playback_buffers" => (1.0, 32.0),
        "overlay.show_source_icons" => (0.0, 2.0),
        p if p.starts_with("voice.volumes.") && p.ends_with(".volume") => (0.0, 2.0),
        _ => return None,
    })
}

/// A hotkey (`VoiceHotkey` in `ConfigHotkeys`): its keys by vanilla name (`key.keyboard.m`,
/// `key.mouse.right`), up to three, all held to press it.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Hotkey {
    pub name: String,
    pub category: String,
    pub keys: Vec<String>,
    pub default: Vec<String>,
    pub any_context: bool,
}

/// `ConfigHotkeys`' own: name, default keys, category, any context.
const HOTKEYS: [(&str, &[&str], &str, bool); 5] = [
    ("key.plasmovoice.proximity.ptt", &["key.keyboard.left.alt"], "hidden", true),
    ("key.plasmovoice.general.mute_microphone", &["key.keyboard.m"], "key.plasmovoice.general", false),
    ("key.plasmovoice.general.disable_voice", &[], "key.plasmovoice.general", false),
    ("key.plasmovoice.general.action", &["key.mouse.right"], "key.plasmovoice.general", false),
    ("key.plasmovoice.occlusion.toggle", &[], "key.plasmovoice.occlusion", false),
];

pub(super) struct Config {
    values: HashMap<String, V>,
    /// Defaults of map entries that depend on the server (a line's overlay state, an
    /// activation's distance).
    defaults: HashMap<String, V>,
    /// Map entries made so far (`computeIfAbsent`): written even when they hold their default.
    created: Vec<String>,
    pub hotkeys: Vec<Hotkey>,
    /// `[addons]`, kept as it was read.
    addons: Option<toml::Table>,
    /// Counts changes, so the game knows when to hand the engine new settings and to save.
    pub revision: u64,
}

impl Default for Config {
    fn default() -> Self {
        let hotkeys = HOTKEYS
            .iter()
            .map(|(name, keys, category, any)| Hotkey {
                name: (*name).into(),
                category: (*category).into(),
                keys: keys.iter().map(|k| (*k).to_owned()).collect(),
                default: keys.iter().map(|k| (*k).to_owned()).collect(),
                any_context: *any,
            })
            .collect();
        Config { values: HashMap::new(), defaults: HashMap::new(), created: Vec::new(), hotkeys, addons: None, revision: 1 }
    }
}

/// A fixed field's default, or a map entry's.
fn fixed_default(path: &str) -> Option<V> {
    let (section, field) = path.split_once('.').unwrap_or(("", path));
    let list = match section {
        "" => ROOT.to_vec(),
        "voice" => voice(),
        "advanced" => advanced(),
        "overlay" => overlay(),
        "horizon" => horizon(),
        _ => Vec::new(),
    };
    if let Some((_, v)) = list.into_iter().find(|(k, _)| *k == field) {
        return Some(v);
    }
    if let Some(rest) = path.strip_prefix("voice.volumes.") {
        if rest.ends_with(".volume") {
            return Some(V::F(1.0));
        }
        if rest.ends_with(".muted") {
            return Some(V::B(false));
        }
    }
    if let Some(rest) = path.strip_prefix("activations.") {
        if rest.ends_with(".type") {
            return Some(V::S("PUSH_TO_TALK".into()));
        }
        if rest.ends_with(".toggle") {
            return Some(V::B(false));
        }
    }
    // Horizon: every line's overlay shows who talks by default (Plasmo Voice has lines without
    // players off).
    if path.starts_with("overlay.source_states.") {
        return Some(V::S("WHEN_TALKING".into()));
    }
    None
}

impl Config {
    pub(super) fn default_of(&self, path: &str) -> V {
        self.defaults.get(path).cloned().or_else(|| fixed_default(path)).unwrap_or(V::I(0))
    }
    pub(super) fn get(&self, path: &str) -> V {
        self.values.get(path).cloned().unwrap_or_else(|| self.default_of(path))
    }
    pub(super) fn b(&self, path: &str) -> bool {
        self.get(path).b()
    }
    pub(super) fn f(&self, path: &str) -> f64 {
        self.get(path).f()
    }
    pub(super) fn i(&self, path: &str) -> i64 {
        self.get(path).i()
    }
    pub(super) fn s(&self, path: &str) -> String {
        self.get(path).s().to_owned()
    }
    /// The paths set under `prefix`.
    pub(super) fn paths_with_prefix(&self, prefix: &str) -> Vec<String> {
        let mut v: Vec<String> = self.values.keys().filter(|k| k.starts_with(prefix)).cloned().collect();
        v.sort();
        v
    }
    pub(super) fn is_default(&self, path: &str) -> bool {
        self.get(path) == self.default_of(path)
    }
    /// `ConfigEntry.set`: numbers clamped to the field's bounds.
    pub(super) fn set(&mut self, path: &str, v: V) {
        let v = match (range(path), v) {
            (Some((lo, hi)), V::F(f)) => V::F(f.clamp(lo, hi)),
            (Some((lo, hi)), V::I(i)) => V::I((i as f64).clamp(lo, hi) as i64),
            (_, v) => v,
        };
        if self.get(path) != v || !self.values.contains_key(path) {
            self.values.insert(path.to_owned(), v);
            self.revision += 1;
        }
        self.create(path);
    }
    pub(super) fn reset(&mut self, path: &str) {
        let d = self.default_of(path);
        self.set(path, d);
    }
    pub(super) fn invert(&mut self, path: &str) {
        let b = self.b(path);
        self.set(path, V::B(!b));
    }
    /// A map entry exists from now on (`computeIfAbsent`).
    pub(super) fn create(&mut self, path: &str) {
        let dynamic = ["voice.volumes.", "activations.", "overlay.source_states.", "servers."].iter().any(|p| path.starts_with(p));
        if dynamic && !self.created.iter().any(|c| c == path) {
            self.created.push(path.to_owned());
        }
    }
    /// A map entry's default, known once the server says (`setDefault`).
    pub(super) fn set_default(&mut self, path: &str, v: V) {
        if self.defaults.get(path) != Some(&v) {
            self.defaults.insert(path.to_owned(), v);
            self.revision += 1;
        }
        self.create(path);
    }

    // ---- hotkeys ----

    pub(super) fn hotkey(&self, name: &str) -> Option<&Hotkey> {
        self.hotkeys.iter().find(|h| h.name == name)
    }
    /// `ConfigHotkeys.register`, once per name.
    pub(super) fn register_hotkey(&mut self, name: &str, keys: &[&str], category: &str, any_context: bool) {
        if self.hotkey(name).is_none() {
            let keys: Vec<String> = keys.iter().map(|k| (*k).to_owned()).collect();
            self.hotkeys.push(Hotkey { name: name.into(), category: category.into(), keys: keys.clone(), default: keys, any_context });
        }
    }
    pub(super) fn set_hotkey(&mut self, name: &str, keys: Vec<String>) {
        if let Some(h) = self.hotkeys.iter_mut().find(|h| h.name == name)
            && h.keys != keys
        {
            h.keys = keys;
            self.revision += 1;
        }
    }

    // ---- the file ----

    /// `TomlConfiguration.load`: what the file has over the defaults; anything it can't read
    /// keeps its default.
    pub(super) fn parse(text: &str) -> Config {
        let mut c = Config::default();
        let Ok(table) = toml::from_str::<toml::Table>(text) else { return c };
        let value = |v: &toml::Value| -> Option<V> {
            Some(match v {
                toml::Value::Boolean(b) => V::B(*b),
                toml::Value::Float(f) => V::F(*f),
                toml::Value::Integer(i) => V::I(*i),
                toml::Value::String(s) => V::S(s.clone()),
                _ => return None,
            })
        };
        // A fixed field takes the file's value in the default's type.
        let fixed = |c: &mut Config, path: String, v: &toml::Value| {
            let Some(d) = fixed_default(&path) else { return };
            let Some(v) = value(v) else { return };
            let v = match (d, v) {
                (V::F(_), V::I(i)) => V::F(i as f64),
                (V::I(_), V::F(f)) => V::I(f as i64),
                (V::B(_), v @ V::B(_)) | (V::F(_), v @ V::F(_)) | (V::I(_), v @ V::I(_)) | (V::S(_), v @ V::S(_)) => v,
                _ => return,
            };
            c.values.insert(path, v);
        };
        for (k, v) in &table {
            match (k.as_str(), v) {
                ("voice" | "advanced" | "overlay" | "horizon", toml::Value::Table(t)) => {
                    for (f, v) in t {
                        match (k.as_str(), f.as_str(), v) {
                            ("voice", "volumes", toml::Value::Table(lines)) => {
                                for (line, entry) in lines {
                                    let toml::Value::Table(entry) = entry else { continue };
                                    for (field, v) in entry {
                                        let path = format!("voice.volumes.{line}.{field}");
                                        if matches!(field.as_str(), "volume" | "muted") {
                                            fixed(&mut c, path.clone(), v);
                                            c.create(&path);
                                        }
                                    }
                                }
                            }
                            ("overlay", "source_states", toml::Value::Table(states)) => {
                                for (line, state) in states {
                                    if let toml::Value::String(s) = state {
                                        let path = format!("overlay.source_states.{line}");
                                        c.values.insert(path.clone(), V::S(s.clone()));
                                        c.create(&path);
                                    }
                                }
                            }
                            _ => fixed(&mut c, format!("{k}.{f}"), v),
                        }
                    }
                }
                ("activations", toml::Value::Table(t)) => {
                    for (name, a) in t {
                        let toml::Value::Table(a) = a else { continue };
                        for field in ["type", "toggle"] {
                            let path = format!("activations.{name}.{field}");
                            if let Some(v) = a.get(field) {
                                fixed(&mut c, path.clone(), v);
                            }
                            c.create(&path);
                        }
                    }
                }
                ("servers", toml::Value::Table(t)) => {
                    for (server, s) in t {
                        let Some(toml::Value::Table(d)) = s.get("distances") else { continue };
                        for (activation, n) in d {
                            if let toml::Value::Integer(n) = n {
                                let path = format!("servers.{server}.distances.{activation}");
                                c.values.insert(path.clone(), V::I(*n));
                                c.create(&path);
                            }
                        }
                    }
                }
                ("key_bindings", toml::Value::Array(list)) => {
                    for entry in list {
                        let Some(name) = entry.get("name").and_then(toml::Value::as_str) else { continue };
                        let category = entry.get("category").and_then(toml::Value::as_str).unwrap_or("hidden");
                        let any = entry.get("any_context").and_then(toml::Value::as_bool).unwrap_or(false);
                        let keys: Vec<String> = entry
                            .get("keys")
                            .and_then(toml::Value::as_array)
                            .map(|a| a.iter().filter_map(|k| k.as_str().map(str::to_owned)).collect())
                            .unwrap_or_default();
                        c.register_hotkey(name, &[], category, any);
                        if let Some(h) = c.hotkeys.iter_mut().find(|h| h.name == name) {
                            h.keys = keys;
                            h.any_context = any;
                        }
                    }
                }
                ("addons", toml::Value::Table(t)) => c.addons = Some(t.clone()),
                _ => fixed(&mut c, k.clone(), v),
            }
        }
        c
    }

    /// `TomlConfiguration.save`.
    pub(super) fn write(&self) -> String {
        let mut out = String::new();
        let line = |out: &mut String, k: &str, v: &V| {
            let _ = writeln!(out, "{} = {}", key(k), v.toml());
        };
        for (k, _) in ROOT.iter() {
            line(&mut out, k, &self.get(k));
        }
        let created = |prefix: &str| -> Vec<&String> {
            let mut v: Vec<&String> = self.created.iter().filter(|p| p.starts_with(prefix)).collect();
            v.sort();
            v
        };
        let section = |out: &mut String, name: &str, fields: Vec<(&'static str, V)>| {
            let _ = writeln!(out, "\n[{name}]");
            for (f, _) in fields {
                line(out, f, &self.get(&format!("{name}.{f}")));
            }
        };
        section(&mut out, "voice", voice());
        // `SourceLineVolumes.serialize`: a line's volume always, a player's when changed, a
        // mute when on.
        let mut volumes: BTreeMap<String, Vec<(String, V)>> = BTreeMap::new();
        for p in created("voice.volumes.") {
            let rest = &p["voice.volumes.".len()..];
            let Some((name, field)) = rest.rsplit_once('.') else { continue };
            let v = self.get(p);
            let keep = match field {
                "volume" => !self.is_default(p) || !name.starts_with("source_"),
                "muted" => !self.is_default(p),
                _ => false,
            };
            if keep {
                volumes.entry(name.to_owned()).or_default().push((field.to_owned(), v));
            }
        }
        for (name, fields) in &mut volumes {
            fields.sort_by(|a, b| b.0.cmp(&a.0));
            let _ = writeln!(out, "\n[voice.volumes.{}]", key(name));
            for (f, v) in fields.iter().rev() {
                line(&mut out, f, v);
            }
        }
        section(&mut out, "advanced", advanced());
        let mut names: Vec<&str> = created("activations.").iter().filter_map(|p| p["activations.".len()..].rsplit_once('.').map(|(n, _)| n)).collect();
        names.dedup();
        if names.is_empty() {
            let _ = writeln!(out, "\n[activations]");
        }
        for name in names {
            let _ = writeln!(out, "\n[activations.{}]", key(name));
            line(&mut out, "type", &self.get(&format!("activations.{name}.type")));
            line(&mut out, "toggle", &self.get(&format!("activations.{name}.toggle")));
        }
        section(&mut out, "overlay", overlay());
        let _ = writeln!(out, "\n[overlay.source_states]");
        for p in created("overlay.source_states.") {
            line(&mut out, &p["overlay.source_states.".len()..], &self.get(p));
        }
        // `ConfigHotkeys.serialize`: the changed hotkeys only.
        for h in self.hotkeys.iter().filter(|h| h.keys != h.default) {
            let _ = writeln!(out, "\n[[key_bindings]]");
            line(&mut out, "name", &V::S(h.name.clone()));
            line(&mut out, "category", &V::S(h.category.clone()));
            let keys: Vec<String> = h.keys.iter().map(|k| quote(k)).collect();
            let _ = writeln!(out, "keys = [{}]", keys.join(", "));
            line(&mut out, "any_context", &V::B(h.any_context));
        }
        // `Servers.serialize`: each server's changed distances.
        let mut servers: BTreeMap<&str, Vec<(&str, V)>> = BTreeMap::new();
        for p in created("servers.") {
            let rest = &p["servers.".len()..];
            let Some((server, activation)) = rest.split_once(".distances.") else { continue };
            if !self.is_default(p) {
                servers.entry(server).or_default().push((activation, self.get(p)));
            }
        }
        if servers.is_empty() {
            let _ = writeln!(out, "\n[servers]");
        }
        for (server, distances) in servers {
            let _ = writeln!(out, "\n[servers.{}.distances]", key(server));
            for (a, v) in distances {
                line(&mut out, a, &v);
            }
        }
        match &self.addons {
            Some(t) if !t.is_empty() => {
                let mut wrapper = toml::Table::new();
                wrapper.insert("addons".into(), toml::Value::Table(t.clone()));
                let _ = write!(out, "\n{}", toml::to_string(&wrapper).unwrap_or_default());
            }
            _ => {
                let _ = writeln!(out, "\n[addons]");
            }
        }
        section(&mut out, "horizon", horizon());
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let mut c = Config::default();
        c.set("voice.volume", V::F(1.5));
        c.set("voice.volumes.proximity.volume", V::F(0.5));
        c.set("voice.volumes.source_00000000-0000-0000-0000-000000000001.muted", V::B(true));
        c.set("activations.proximity.type", V::S("VOICE".into()));
        c.set("overlay.source_states.proximity", V::S("ALWAYS".into()));
        c.set("servers.a-b.distances.c-d", V::I(8));
        c.set_hotkey("key.plasmovoice.general.mute_microphone", vec!["key.keyboard.n".into()]);
        let text = c.write();
        let d = Config::parse(&text);
        assert_eq!(d.f("voice.volume"), 1.5);
        assert_eq!(d.f("voice.volumes.proximity.volume"), 0.5);
        assert!(d.b("voice.volumes.source_00000000-0000-0000-0000-000000000001.muted"));
        assert_eq!(d.s("activations.proximity.type"), "VOICE");
        assert_eq!(d.s("overlay.source_states.proximity"), "ALWAYS");
        assert_eq!(d.i("servers.a-b.distances.c-d"), 8);
        assert_eq!(d.hotkey("key.plasmovoice.general.mute_microphone").unwrap().keys, vec!["key.keyboard.n"]);
        assert_eq!(d.write(), text);
        assert_eq!(d.f("voice.activation_threshold"), -60.0);
        // Clamped as `DoubleConfigEntry` does.
        let mut e = Config::default();
        e.set("voice.volume", V::F(5.0));
        assert_eq!(e.f("voice.volume"), 2.0);
    }
}
