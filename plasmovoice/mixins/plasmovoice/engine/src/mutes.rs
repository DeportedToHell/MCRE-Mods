//! Server mutes as Plasmo Voice keeps them: `ServerMuteInfo` in `voice_mutes.json` of the server's
//! Plasmo Voice folder (`JsonMuteStorage`, Gson's map of UUID to mute), expired mutes dropped on
//! load and save (`VoiceMuteManager.isMuteValid`), and the `/vmute` duration argument
//! (`MuteDurationType`, `MuteDurationUnit`).
//
// This Source Code Form is a port of Plasmo Voice (https://github.com/plasmoapp/plasmo-voice), LGPL-3.0.

use crate::server_config::parse_uuid;
use crate::wire::Uuid;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::PathBuf;

/// `ServerMuteInfo`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MuteInfo {
    pub player: Uuid,
    pub muted_by: Option<Uuid>,
    pub muted_at: i64,
    /// 0: permanent.
    pub muted_to: i64,
    pub reason: Option<String>,
    /// None for mutes stored before the field existed.
    pub silent: Option<bool>,
}

impl MuteInfo {
    /// `VoiceMuteManager.isMuteValid`.
    pub fn valid(&self, now: i64) -> bool {
        self.muted_to == 0 || self.muted_to > now
    }
}

/// `JsonMuteStorage`: in memory, and in its file when it has one.
#[derive(Debug, Default)]
pub struct MuteStorage {
    path: Option<PathBuf>,
    mutes: BTreeMap<Uuid, MuteInfo>,
}

impl MuteStorage {
    /// `init`: the file's mutes still valid; the file written again when some had expired.
    pub fn load(path: Option<PathBuf>, now: i64) -> Result<MuteStorage, String> {
        let mut s = MuteStorage { path, mutes: BTreeMap::new() };
        let Some(path) = s.path.clone() else { return Ok(s) };
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(s),
            Err(e) => return Err(format!("{}: {e}", path.display())),
        };
        let all = parse(&text)?;
        let total = all.len();
        s.mutes = all.into_iter().filter(|(_, m)| m.valid(now)).collect();
        if s.mutes.len() != total {
            s.save(now)?;
        }
        Ok(s)
    }

    pub fn get(&self, player: Uuid) -> Option<&MuteInfo> {
        self.mutes.get(&player)
    }

    pub fn all(&self) -> impl Iterator<Item = &MuteInfo> {
        self.mutes.values()
    }

    /// `putPlayerMute`: kept (and saved) only while valid.
    pub fn put(&mut self, info: MuteInfo, now: i64) -> Result<(), String> {
        if info.valid(now) {
            self.mutes.insert(info.player, info);
            self.save(now)?;
        }
        Ok(())
    }

    /// `removeMuteByPlayerId`.
    pub fn remove(&mut self, player: Uuid, now: i64) -> Result<Option<MuteInfo>, String> {
        let gone = self.mutes.remove(&player);
        if gone.is_some() {
            self.save(now)?;
        }
        Ok(gone)
    }

    /// `save`: expired mutes dropped, the rest written as Gson writes the map.
    pub fn save(&mut self, now: i64) -> Result<(), String> {
        self.mutes.retain(|_, m| m.valid(now));
        let Some(path) = &self.path else { return Ok(()) };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        std::fs::write(path, write(&self.mutes)).map_err(|e| format!("{}: {e}", path.display()))
    }
}

/// Gson's `toJson(Map<UUID, ServerMuteInfo>)`: compact, fields in declaration order, nulls left
/// out, HTML characters escaped.
fn write(mutes: &BTreeMap<Uuid, MuteInfo>) -> String {
    let mut o = String::from("{");
    for (i, (id, m)) in mutes.iter().enumerate() {
        if i > 0 {
            o.push(',');
        }
        let _ = write!(o, "{}:{{\"playerUUID\":{}", gson_string(&id.to_string()), gson_string(&m.player.to_string()));
        if let Some(by) = m.muted_by {
            let _ = write!(o, ",\"mutedByPlayerUUID\":{}", gson_string(&by.to_string()));
        }
        let _ = write!(o, ",\"mutedAtTime\":{},\"mutedToTime\":{}", m.muted_at, m.muted_to);
        if let Some(r) = &m.reason {
            let _ = write!(o, ",\"reason\":{}", gson_string(r));
        }
        if let Some(s) = m.silent {
            let _ = write!(o, ",\"silent\":{s}");
        }
        o.push('}');
    }
    o.push('}');
    o
}

/// Gson's `JsonWriter.string` with `htmlSafe` on (its default).
fn gson_string(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            '\u{8}' => o.push_str("\\b"),
            '\u{c}' => o.push_str("\\f"),
            '<' | '>' | '&' | '=' | '\'' | '\u{2028}' | '\u{2029}' => {
                let _ = write!(o, "\\u{:04x}", c as u32);
            }
            c if (c as u32) < 0x20 => {
                let _ = write!(o, "\\u{:04x}", c as u32);
            }
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

fn parse(text: &str) -> Result<BTreeMap<Uuid, MuteInfo>, String> {
    let v: serde_json::Value = serde_json::from_str(text).map_err(|e| format!("voice_mutes.json: {e}"))?;
    let mut out = BTreeMap::new();
    // Gson reads `null` as no map.
    let Some(map) = v.as_object() else { return Ok(out) };
    for (key, m) in map {
        let Some(id) = parse_uuid(key) else { continue };
        let uuid = |k: &str| m.get(k).and_then(|x| x.as_str()).and_then(parse_uuid);
        let info = MuteInfo {
            player: uuid("playerUUID").unwrap_or(id),
            muted_by: uuid("mutedByPlayerUUID"),
            muted_at: m.get("mutedAtTime").and_then(serde_json::Value::as_i64).unwrap_or(0),
            muted_to: m.get("mutedToTime").and_then(serde_json::Value::as_i64).unwrap_or(0),
            reason: m.get("reason").and_then(|x| x.as_str()).map(str::to_owned),
            silent: m.get("silent").and_then(serde_json::Value::as_bool),
        };
        out.insert(id, info);
    }
    Ok(out)
}

/// `MuteDurationUnit`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DurationUnit {
    Second,
    Minute,
    Hour,
    Day,
    Week,
    /// A Unix time in seconds (`u`).
    Timestamp,
}

impl DurationUnit {
    pub fn millis(self) -> i64 {
        match self {
            DurationUnit::Second => 1_000,
            DurationUnit::Minute => 60_000,
            DurationUnit::Hour => 3_600_000,
            DurationUnit::Day => 86_400_000,
            DurationUnit::Week => 604_800_000,
            DurationUnit::Timestamp => 1,
        }
    }
    /// The text's key (`pv.mutes.durations.*`).
    pub fn key(self) -> &'static str {
        match self {
            DurationUnit::Second | DurationUnit::Timestamp => "pv.mutes.durations.seconds",
            DurationUnit::Minute => "pv.mutes.durations.minutes",
            DurationUnit::Hour => "pv.mutes.durations.hours",
            DurationUnit::Day => "pv.mutes.durations.days",
            DurationUnit::Week => "pv.mutes.durations.weeks",
        }
    }
}

/// `MuteDuration`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MuteDuration {
    Permanent,
    Time { amount: i64, unit: DurationUnit },
}

/// Why a duration was refused (`pv.command.mute.invalid_duration`, `timestamp_in_past`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DurationError {
    Invalid,
    InPast,
}

impl DurationError {
    pub fn key(self) -> &'static str {
        match self {
            DurationError::Invalid => "pv.command.mute.invalid_duration",
            DurationError::InPast => "pv.command.mute.timestamp_in_past",
        }
    }
}

/// `MuteDurationType.parse`: `permanent`, or `^([0-9]+)([mhdwsu])?$` above 0 (no unit: seconds;
/// `u`: a Unix time in seconds, which must be in the future).
pub fn parse_duration(input: &str, now: i64) -> Result<MuteDuration, DurationError> {
    if input == "permanent" {
        return Ok(MuteDuration::Permanent);
    }
    let digits = input.trim_end_matches(|c: char| c.is_ascii_alphabetic());
    let suffix = &input[digits.len()..];
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) || suffix.len() > 1 {
        return Err(DurationError::Invalid);
    }
    let unit = match suffix {
        "m" => DurationUnit::Minute,
        "h" => DurationUnit::Hour,
        "d" => DurationUnit::Day,
        "w" => DurationUnit::Week,
        "u" => DurationUnit::Timestamp,
        "" | "s" => DurationUnit::Second,
        _ => return Err(DurationError::Invalid),
    };
    let amount: i64 = digits.parse().map_err(|_| DurationError::Invalid)?;
    if amount <= 0 {
        return Err(DurationError::Invalid);
    }
    let muted_to = if unit == DurationUnit::Timestamp {
        amount.checked_mul(1_000)
    } else {
        amount.checked_mul(unit.millis()).and_then(|ms| ms.checked_add(now))
    }
    .ok_or(DurationError::Invalid)?;
    if unit == DurationUnit::Timestamp && muted_to <= now {
        return Err(DurationError::InPast);
    }
    Ok(MuteDuration::Time { amount, unit })
}

/// `MuteDurationType.listSuggestions`.
pub fn duration_suggestions(input: &str) -> Vec<String> {
    let mut out = Vec::new();
    if "permanent".starts_with(&input.to_lowercase()) {
        out.push("permanent".to_owned());
    }
    if input.starts_with(|c: char| ('1'..='9').contains(&c)) && input.bytes().all(|b| b.is_ascii_digit()) {
        out.extend(["s", "m", "h", "d", "w"].iter().map(|u| format!("{input}{u}")));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_762_790_400_000;

    #[test]
    fn durations() {
        assert_eq!(parse_duration("permanent", NOW), Ok(MuteDuration::Permanent));
        assert_eq!(parse_duration("30", NOW), Ok(MuteDuration::Time { amount: 30, unit: DurationUnit::Second }));
        assert_eq!(parse_duration("15m", NOW), Ok(MuteDuration::Time { amount: 15, unit: DurationUnit::Minute }));
        assert_eq!(parse_duration("2w", NOW), Ok(MuteDuration::Time { amount: 2, unit: DurationUnit::Week }));
        for bad in ["0", "0m", "-5m", "5x", "m", "", "1.5h", "5mm", "Permanent", "99999999999999999999"] {
            assert_eq!(parse_duration(bad, NOW), Err(DurationError::Invalid), "{bad}");
        }
        assert_eq!(parse_duration("1000u", NOW), Err(DurationError::InPast));
        assert_eq!(parse_duration("9999999999u", NOW), Ok(MuteDuration::Time { amount: 9_999_999_999, unit: DurationUnit::Timestamp }));
        assert_eq!(duration_suggestions(""), vec!["permanent"]);
        assert_eq!(duration_suggestions("PER"), vec!["permanent"]);
        assert_eq!(duration_suggestions("15"), vec!["15s", "15m", "15h", "15d", "15w"]);
        assert!(duration_suggestions("05").is_empty());
    }

    #[test]
    fn storage_as_gson_writes_it() {
        let dir = std::env::temp_dir().join(format!("pv-mutes-{}", std::process::id()));
        let path = dir.join("voice_mutes.json");
        let _ = std::fs::remove_dir_all(&dir);
        let mut s = MuteStorage::load(Some(path.clone()), NOW).unwrap();
        let a = MuteInfo {
            player: Uuid(0x1fb15b74_2e5f_48a1_a610_4891d834a25c),
            muted_by: None,
            muted_at: NOW,
            muted_to: 0,
            reason: Some("<spam> & 'flood'".into()),
            silent: Some(false),
        };
        let b = MuteInfo { player: Uuid(2), muted_by: Some(Uuid(1)), muted_at: NOW, muted_to: NOW + 60_000, reason: None, silent: None };
        s.put(a.clone(), NOW).unwrap();
        s.put(b.clone(), NOW).unwrap();
        // Already over: not kept.
        s.put(MuteInfo { player: Uuid(3), muted_to: NOW - 1, ..b.clone() }, NOW).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            text,
            "{\"00000000-0000-0000-0000-000000000002\":{\"playerUUID\":\"00000000-0000-0000-0000-000000000002\",\
             \"mutedByPlayerUUID\":\"00000000-0000-0000-0000-000000000001\",\"mutedAtTime\":1762790400000,\
             \"mutedToTime\":1762790460000},\
             \"1fb15b74-2e5f-48a1-a610-4891d834a25c\":{\"playerUUID\":\"1fb15b74-2e5f-48a1-a610-4891d834a25c\",\
             \"mutedAtTime\":1762790400000,\"mutedToTime\":0,\"reason\":\"\\u003cspam\\u003e \\u0026 \\u0027flood\\u0027\",\"silent\":false}}"
        );
        // A restart: both back; a minute later, the temporary one is gone and the file rewritten.
        let again = MuteStorage::load(Some(path.clone()), NOW).unwrap();
        assert_eq!(again.get(a.player), Some(&a));
        assert_eq!(again.get(b.player), Some(&b));
        let later = MuteStorage::load(Some(path.clone()), NOW + 61_000).unwrap();
        assert!(later.get(b.player).is_none() && later.get(a.player).is_some());
        assert!(!std::fs::read_to_string(&path).unwrap().contains("00000000-0000-0000-0000-000000000002"));
        let mut s = later;
        assert_eq!(s.remove(a.player, NOW).unwrap(), Some(a.clone()));
        assert_eq!(s.remove(a.player, NOW).unwrap(), None);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
