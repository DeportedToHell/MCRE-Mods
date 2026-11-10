//! The voice server's texts (`VoiceServerLanguages`): Plasmo Voice's `languages/en_us.toml` and
//! Crowdin's Russian `server.toml`, written into the server's `languages/` folder when missing and
//! read from it, so a server owner can change them; a key a file lacks comes from the bundled
//! text, then from the default language, then from English. `[server]` texts are the commands'
//! and messages', `[client]` ones go to clients in the `LanguagePacket`.
//
// This Source Code Form is a port of Plasmo Voice (https://github.com/plasmoapp/plasmo-voice), LGPL-3.0.

use std::collections::HashMap;
use std::path::Path;

const FALLBACK: &str = "en_us";

/// The languages the port carries: Plasmo Voice's English and Crowdin's Russian.
const BUNDLED: [(&str, &str); 2] = [("en_us", include_str!("lang/en_us.toml")), ("ru_ru", include_str!("lang/ru_ru.toml"))];

/// One language's texts, flattened (`pv.command.list.message`).
#[derive(Clone, Debug, Default)]
struct Language {
    server: HashMap<String, String>,
    client: HashMap<String, String>,
}

fn flatten(t: &toml::Table, prefix: &str, out: &mut HashMap<String, String>) {
    for (k, v) in t {
        let key = if prefix.is_empty() { k.clone() } else { format!("{prefix}.{k}") };
        match v {
            toml::Value::Table(t) => flatten(t, &key, out),
            toml::Value::String(s) => {
                out.insert(key, s.clone());
            }
            other => {
                out.insert(key, other.to_string());
            }
        }
    }
}

fn parse(text: &str) -> Result<Language, String> {
    let t: toml::Table = text.parse().map_err(|e: toml::de::Error| e.to_string())?;
    let mut l = Language::default();
    if let Some(s) = t.get("server").and_then(|v| v.as_table()) {
        flatten(s, "", &mut l.server);
    }
    if let Some(c) = t.get("client").and_then(|v| v.as_table()) {
        flatten(c, "", &mut l.client);
    }
    Ok(l)
}

/// The server's languages and which one each reader gets.
#[derive(Clone, Debug)]
pub struct Languages {
    languages: HashMap<String, Language>,
    default: String,
    forced: Option<String>,
}

impl Default for Languages {
    fn default() -> Self {
        Languages::load(None, FALLBACK, None).0
    }
}

impl Languages {
    /// The bundled languages, each file of `folder` (`languages/`) over them, the missing files
    /// written; with the problems met, for the log.
    pub fn load(folder: Option<&Path>, default: &str, forced: Option<&str>) -> (Languages, Vec<String>) {
        let mut problems = Vec::new();
        let mut languages = HashMap::new();
        for (name, text) in BUNDLED {
            let mut lang = parse(text).unwrap_or_default();
            if let Some(dir) = folder {
                let path = dir.join(format!("{name}.toml"));
                match std::fs::read_to_string(&path) {
                    Ok(own) => match parse(&own) {
                        Ok(own) => {
                            lang.server.extend(own.server);
                            lang.client.extend(own.client);
                        }
                        Err(e) => problems.push(format!("Failed to load language {name}.toml: {e}")),
                    },
                    Err(_) => {
                        let _ = std::fs::create_dir_all(dir);
                        if let Err(e) = std::fs::write(&path, text) {
                            problems.push(format!("Failed to save language {}: {e}", path.display()));
                        }
                    }
                }
            }
            languages.insert(name.to_owned(), lang);
        }
        // Other languages a server owner put in the folder, filled from the default.
        if let Some(dir) = folder
            && let Ok(entries) = std::fs::read_dir(dir)
        {
            for e in entries.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                let Some(lang) = name.strip_suffix(".toml") else { continue };
                if languages.contains_key(lang) {
                    continue;
                }
                match std::fs::read_to_string(e.path()).map_err(|e| e.to_string()).and_then(|t| parse(&t)) {
                    Ok(l) => {
                        languages.insert(lang.to_owned(), l);
                    }
                    Err(e) => problems.push(format!("Failed to load language {name}: {e}")),
                }
            }
        }
        let default = default.to_lowercase();
        (Languages { languages, default, forced: forced.map(str::to_lowercase) }, problems)
    }

    /// `getLanguage`: the forced language, else the reader's when the server has it, else the
    /// default one.
    fn pick(&self, language: Option<&str>) -> &str {
        if let Some(f) = &self.forced {
            return f;
        }
        match language.map(str::to_lowercase) {
            Some(l) if self.languages.contains_key(&l) => self.languages.get_key_value(&l).map_or(FALLBACK, |(k, _)| k.as_str()),
            _ => &self.default,
        }
    }

    /// The language a reader gets (`None`: the console, which reads the default one).
    pub fn resolve(&self, language: Option<&str>) -> String {
        self.pick(language).to_owned()
    }

    fn lookup(&self, language: Option<&str>, key: &str, client: bool) -> Option<&String> {
        let chain = [self.pick(language), self.default.as_str(), FALLBACK];
        chain.iter().find_map(|l| {
            let l = self.languages.get(*l)?;
            if client { l.client.get(key) } else { l.server.get(key) }
        })
    }

    /// A `[server]` text, its `%s` arguments filled (`McTextComponent.translatable`); the key
    /// itself when no language has it, as Minecraft shows a missing translation.
    pub fn tr(&self, language: Option<&str>, key: &str, args: &[&str]) -> String {
        match self.lookup(language, key, false) {
            Some(t) => java_format(t, args),
            None => key.to_owned(),
        }
    }

    /// A `[server]` text as is (a date pattern), or `default`.
    pub fn raw(&self, language: Option<&str>, key: &str, default: &str) -> String {
        self.lookup(language, key, false).cloned().unwrap_or_else(|| default.to_owned())
    }

    /// `getClientLanguage`: the `[client]` texts for the `LanguagePacket`.
    pub fn client(&self, language: &str) -> Vec<(String, String)> {
        let mut out: HashMap<String, String> = HashMap::new();
        for l in [FALLBACK, self.default.as_str(), self.pick(Some(language))] {
            if let Some(l) = self.languages.get(l) {
                out.extend(l.client.clone());
            }
        }
        let mut v: Vec<(String, String)> = out.into_iter().collect();
        v.sort();
        v
    }

    pub fn has(&self, language: &str) -> bool {
        self.languages.contains_key(&language.to_lowercase())
    }
}

/// Java's `String.format` for the conversions the texts use: `%s`, `%1$s`, `%%`.
pub fn java_format(template: &str, args: &[&str]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut next = 0usize;
    let mut rest = template;
    while let Some(i) = rest.find('%') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        if let Some(r) = after.strip_prefix('%') {
            out.push('%');
            rest = r;
            continue;
        }
        if let Some(r) = after.strip_prefix('s') {
            out.push_str(args.get(next).copied().unwrap_or("null"));
            next += 1;
            rest = r;
            continue;
        }
        let digits = after.bytes().take_while(u8::is_ascii_digit).count();
        if digits > 0 && after[digits..].starts_with("$s") {
            let n: usize = after[..digits].parse().unwrap_or(0);
            out.push_str(n.checked_sub(1).and_then(|n| args.get(n)).copied().unwrap_or("null"));
            rest = &after[digits + 2..];
            continue;
        }
        out.push('%');
        rest = after;
    }
    out.push_str(rest);
    out
}

/// `SimpleDateFormat` for the letters the mute list's patterns use (`yyyy.MM.dd`, `HH:mm:ss`),
/// in the server's time zone.
pub fn format_date(pattern: &str, millis: i64) -> String {
    use chrono::{Datelike, Local, TimeZone, Timelike};
    let Some(t) = Local.timestamp_millis_opt(millis).single() else { return String::new() };
    let mut out = String::new();
    let chars: Vec<char> = pattern.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\'' {
            let end = chars[i + 1..].iter().position(|x| *x == '\'').map_or(chars.len(), |p| i + 1 + p);
            if end == i + 1 {
                out.push('\'');
            } else {
                out.extend(&chars[i + 1..end.min(chars.len())]);
            }
            i = end + 1;
            continue;
        }
        if !c.is_ascii_alphabetic() {
            out.push(c);
            i += 1;
            continue;
        }
        let n = chars[i..].iter().take_while(|x| **x == c).count();
        let pad = |v: i64| format!("{v:0n$}");
        match c {
            'y' if n == 2 => out.push_str(&pad(i64::from(t.year() % 100))),
            'y' => out.push_str(&pad(i64::from(t.year()))),
            'M' => out.push_str(&pad(i64::from(t.month()))),
            'd' => out.push_str(&pad(i64::from(t.day()))),
            'H' => out.push_str(&pad(i64::from(t.hour()))),
            'm' => out.push_str(&pad(i64::from(t.minute()))),
            's' => out.push_str(&pad(i64::from(t.second()))),
            'S' => out.push_str(&pad(i64::from(t.timestamp_subsec_millis()))),
            _ => out.extend(std::iter::repeat_n(c, n)),
        }
        i += n;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_as_java() {
        assert_eq!(java_format("Clients (%1$s/%2$s): %3$s", &["1", "2", "a, b"]), "Clients (1/2): a, b");
        assert_eq!(java_format("%s already muted", &["Bob"]), "Bob already muted");
        assert_eq!(java_format("100%% %s %s", &["x"]), "100% x null");
    }

    #[test]
    fn english_russian_and_fallbacks() {
        let l = Languages::default();
        assert_eq!(l.tr(None, "pv.command.reload.message", &[]), "Config reloaded");
        assert_eq!(l.tr(Some("ru_RU"), "pv.command.reload.message", &[]), "Конфиг перезагружен");
        assert_eq!(l.tr(Some("ru_ru"), "pv.mutes.durations.minutes", &["15"]), "на 15 мин.");
        // Crowdin's Russian lacks it: English.
        assert_eq!(l.tr(Some("ru_ru"), "pv.command.mute.invalid_duration", &[]), "Invalid duration. Examples: 30s, 15m, 1h, 7d, 2w, permanent");
        assert_eq!(l.tr(Some("ja_jp"), "pv.command.unmute.unmuted", &["Bob"]), "Unmuted Bob");
        assert_eq!(l.tr(None, "pv.no.such", &[]), "pv.no.such");
        assert!(l.client("ru_ru").contains(&("pv.activation.proximity".to_owned(), "Локальный".to_owned())));
        let (forced, _) = Languages::load(None, "en_us", Some("ru_ru"));
        assert_eq!(forced.tr(Some("en_us"), "pv.command.list.empty", &[]), "нет игроков с установленным Plasmo Voice");
    }

    #[test]
    fn folder_files_override_and_are_written() {
        let dir = std::env::temp_dir().join(format!("pv-lang-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (_, problems) = Languages::load(Some(&dir), "en_us", None);
        assert!(problems.is_empty(), "{problems:?}");
        assert!(dir.join("en_us.toml").is_file() && dir.join("ru_ru.toml").is_file());
        std::fs::write(dir.join("en_us.toml"), "[server.pv.command.reload]\nmessage = \"Reloaded!\"\n").unwrap();
        let (l, _) = Languages::load(Some(&dir), "en_us", None);
        assert_eq!(l.tr(None, "pv.command.reload.message", &[]), "Reloaded!");
        assert_eq!(l.tr(None, "pv.command.unmute.unmuted", &["A"]), "Unmuted A");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dates() {
        let s = format_date("yyyy.MM.dd 'at' HH:mm:ss", 0);
        assert_eq!(s.len(), "1970.01.01 at 00:00:00".len());
        assert!(s.contains(" at "));
    }
}
