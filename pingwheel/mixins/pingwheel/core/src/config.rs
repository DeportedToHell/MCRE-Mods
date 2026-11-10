//! `ClientConfig` (`config/pingwheel.json`) and `ServerConfig` (`config/pingwheel.server.json`),
//! read and written as Ping Wheel's `ConfigHandler` does with Gson (`setPrettyPrinting`): the
//! fields in their declaration order, two-space indents, enums by name, a field the file lacks
//! keeps its default, a file that doesn't parse is reset to the defaults. `Display` is Lombok's
//! `@ToString`, which the "Loaded"/"Saved" log lines print.

use crate::MAX_CHANNEL_LENGTH;
use serde_json::Value;
use std::fmt;

/// `ChannelMode`: what the default channel (`""`) does on a server.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ChannelMode {
    #[default]
    Auto,
    Disabled,
    Global,
    TeamOnly,
}

/// `PlayerInfoMode`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlayerInfoMode {
    #[default]
    Hold,
    Disabled,
    Always,
    Compact,
}

/// `TeamColorMode`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TeamColorMode {
    #[default]
    Full,
    Disabled,
    PingOnly,
    LabelsOnly,
}

macro_rules! java_enum {
    ($t:ty, $($v:ident = $name:literal),*) => {
        impl $t {
            pub const ALL: &'static [$t] = &[$(<$t>::$v),*];
            /// `name()`: as Gson writes it.
            pub fn name(self) -> &'static str {
                match self { $(<$t>::$v => $name),* }
            }
            /// `toString()`: lower case, the lang keys' `value.pingwheel.<this>`.
            pub fn lower(self) -> String {
                self.name().to_lowercase()
            }
            /// `get(name)` / Gson: by name, ignoring case for `get`.
            pub fn parse(name: &str) -> Option<Self> {
                Self::ALL.iter().copied().find(|m| m.name().eq_ignore_ascii_case(name))
            }
            /// The next one (`CycleButton`), or the previous.
            pub fn cycle(self, back: bool) -> Self {
                let i = Self::ALL.iter().position(|m| *m == self).unwrap_or(0);
                let n = Self::ALL.len();
                Self::ALL[if back { (i + n - 1) % n } else { (i + 1) % n }]
            }
        }
    };
}
java_enum!(ChannelMode, Auto = "AUTO", Disabled = "DISABLED", Global = "GLOBAL", TeamOnly = "TEAM_ONLY");
java_enum!(PlayerInfoMode, Hold = "HOLD", Disabled = "DISABLED", Always = "ALWAYS", Compact = "COMPACT");
java_enum!(TeamColorMode, Full = "FULL", Disabled = "DISABLED", PingOnly = "PING_ONLY", LabelsOnly = "LABELS_ONLY");

// ---- the server's -------------------------------------------------------------------------------

/// `ServerConfig`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerConfig {
    pub default_channel_mode: ChannelMode,
    pub player_tracking_enabled: bool,
    pub ms_to_regenerate: i32,
    pub rate_limit: i32,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self { default_channel_mode: ChannelMode::Auto, player_tracking_enabled: true, ms_to_regenerate: 1000, rate_limit: 5 }
    }
}

impl ServerConfig {
    pub fn rates(&self) -> crate::server::Rates {
        crate::server::Rates::new(i64::from(self.ms_to_regenerate), i64::from(self.rate_limit))
    }

    /// `validate`.
    pub fn validate(&mut self) {
        if self.ms_to_regenerate < 0 {
            self.ms_to_regenerate = 1000;
        }
        if self.rate_limit < 0 {
            self.rate_limit = 0;
        }
    }

    /// Gson's `fromJson`: None when the file isn't a JSON object (the handler then resets it).
    pub fn parse(text: &str) -> Option<Self> {
        let Value::Object(o) = serde_json::from_str::<Value>(text).ok()? else { return None };
        let mut c = Self::default();
        if let Some(v) = o.get("defaultChannelMode").and_then(Value::as_str).and_then(ChannelMode::parse_exact) {
            c.default_channel_mode = v;
        }
        if let Some(v) = o.get("playerTrackingEnabled").and_then(Value::as_bool) {
            c.player_tracking_enabled = v;
        }
        if let Some(v) = o.get("msToRegenerate").and_then(int) {
            c.ms_to_regenerate = v;
        }
        if let Some(v) = o.get("rateLimit").and_then(int) {
            c.rate_limit = v;
        }
        Some(c)
    }

    /// `gson.toJson(config)`.
    pub fn write(&self) -> String {
        let mut w = Gson::default();
        w.string("defaultChannelMode", self.default_channel_mode.name());
        w.raw("playerTrackingEnabled", &self.player_tracking_enabled.to_string());
        w.raw("msToRegenerate", &self.ms_to_regenerate.to_string());
        w.raw("rateLimit", &self.rate_limit.to_string());
        w.finish()
    }
}

impl fmt::Display for ServerConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ServerConfig(defaultChannelMode={}, playerTrackingEnabled={}, msToRegenerate={}, rateLimit={})",
            self.default_channel_mode.lower(),
            self.player_tracking_enabled,
            self.ms_to_regenerate,
            self.rate_limit
        )
    }
}

// ---- the client's -------------------------------------------------------------------------------

/// `ClientConfig`.
#[derive(Clone, Debug, PartialEq)]
pub struct ClientConfig {
    pub ping_volume: i32,
    pub ping_duration: i32,
    pub ping_distance: i32,
    pub correction_period: f32,
    pub item_icon_visible: bool,
    pub direction_indicator_visible: bool,
    pub player_info_mode: PlayerInfoMode,
    pub team_color_mode: TeamColorMode,
    pub ping_size: i32,
    /// Single player's channel (and a LAN host's).
    pub channel: String,
    /// Each server's channel, by the address as joined (`ServerData.ip`), in file order.
    pub server_channels: Vec<(String, String)>,
    // Hidden from the settings screen.
    pub remove_radius: i32,
    pub raycast_distance: i32,
    pub safe_zone_left: i32,
    pub safe_zone_right: i32,
    pub safe_zone_top: i32,
    pub safe_zone_bottom: i32,
}

pub const TPS: i32 = 20;
pub const MAX_PING_DURATION: i32 = 60;
pub const MAX_PING_DISTANCE: i32 = 2048;
pub const MAX_CORRECTION_PERIOD: f32 = 5.0;

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            ping_volume: 100,
            ping_duration: 7,
            ping_distance: 2048,
            correction_period: 1.0,
            item_icon_visible: true,
            direction_indicator_visible: true,
            player_info_mode: PlayerInfoMode::Hold,
            team_color_mode: TeamColorMode::Full,
            ping_size: 100,
            channel: String::new(),
            server_channels: Vec::new(),
            remove_radius: 10,
            raycast_distance: 1024,
            safe_zone_left: 5,
            safe_zone_right: 5,
            safe_zone_top: 5,
            safe_zone_bottom: 60,
        }
    }
}

impl ClientConfig {
    /// `getChannel`: the server's (by its address), or single player's.
    pub fn channel_for(&self, server: Option<&str>) -> String {
        match server {
            Some(ip) => self.server_channels.iter().find(|(k, _)| k == ip).map(|(_, v)| v.clone()).unwrap_or_default(),
            None => self.channel.clone(),
        }
    }

    /// `setChannel`.
    pub fn set_channel_for(&mut self, server: Option<&str>, channel: &str) {
        match server {
            Some(ip) => match self.server_channels.iter_mut().find(|(k, _)| k == ip) {
                Some(entry) => entry.1 = channel.to_owned(),
                None => self.server_channels.push((ip.to_owned(), channel.to_owned())),
            },
            None => self.channel = channel.to_owned(),
        }
    }

    /// `validate`: channels cut to 128 characters.
    pub fn validate(&mut self) {
        let cut = |s: &mut String| {
            if s.chars().count() > MAX_CHANNEL_LENGTH {
                *s = s.chars().take(MAX_CHANNEL_LENGTH).collect();
            }
        };
        cut(&mut self.channel);
        for (_, v) in &mut self.server_channels {
            cut(v);
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        let Value::Object(o) = serde_json::from_str::<Value>(text).ok()? else { return None };
        let mut c = Self::default();
        let ints: [(&str, &mut i32); 10] = [
            ("pingVolume", &mut c.ping_volume),
            ("pingDuration", &mut c.ping_duration),
            ("pingDistance", &mut c.ping_distance),
            ("pingSize", &mut c.ping_size),
            ("removeRadius", &mut c.remove_radius),
            ("raycastDistance", &mut c.raycast_distance),
            ("safeZoneLeft", &mut c.safe_zone_left),
            ("safeZoneRight", &mut c.safe_zone_right),
            ("safeZoneTop", &mut c.safe_zone_top),
            ("safeZoneBottom", &mut c.safe_zone_bottom),
        ];
        for (key, field) in ints {
            if let Some(v) = o.get(key).and_then(int) {
                *field = v;
            }
        }
        if let Some(v) = o.get("correctionPeriod").and_then(Value::as_f64) {
            c.correction_period = v as f32;
        }
        if let Some(v) = o.get("itemIconVisible").and_then(Value::as_bool) {
            c.item_icon_visible = v;
        }
        if let Some(v) = o.get("directionIndicatorVisible").and_then(Value::as_bool) {
            c.direction_indicator_visible = v;
        }
        if let Some(v) = o.get("playerInfoMode").and_then(Value::as_str).and_then(PlayerInfoMode::parse_exact) {
            c.player_info_mode = v;
        }
        if let Some(v) = o.get("teamColorMode").and_then(Value::as_str).and_then(TeamColorMode::parse_exact) {
            c.team_color_mode = v;
        }
        if let Some(v) = o.get("channel").and_then(Value::as_str) {
            c.channel = v.to_owned();
        }
        if let Some(Value::Object(m)) = o.get("serverChannels") {
            c.server_channels = m.iter().filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_owned()))).collect();
        }
        Some(c)
    }

    pub fn write(&self) -> String {
        let mut w = Gson::default();
        w.raw("pingVolume", &self.ping_volume.to_string());
        w.raw("pingDuration", &self.ping_duration.to_string());
        w.raw("pingDistance", &self.ping_distance.to_string());
        w.raw("correctionPeriod", &java_float(self.correction_period));
        w.raw("itemIconVisible", &self.item_icon_visible.to_string());
        w.raw("directionIndicatorVisible", &self.direction_indicator_visible.to_string());
        w.string("playerInfoMode", self.player_info_mode.name());
        w.string("teamColorMode", self.team_color_mode.name());
        w.raw("pingSize", &self.ping_size.to_string());
        w.string("channel", &self.channel);
        if self.server_channels.is_empty() {
            w.raw("serverChannels", "{}");
        } else {
            let entries: Vec<String> = self.server_channels.iter().map(|(k, v)| format!("    {}: {}", gson_string(k), gson_string(v))).collect();
            w.raw("serverChannels", &format!("{{\n{}\n  }}", entries.join(",\n")));
        }
        w.raw("removeRadius", &self.remove_radius.to_string());
        w.raw("raycastDistance", &self.raycast_distance.to_string());
        w.raw("safeZoneLeft", &self.safe_zone_left.to_string());
        w.raw("safeZoneRight", &self.safe_zone_right.to_string());
        w.raw("safeZoneTop", &self.safe_zone_top.to_string());
        w.raw("safeZoneBottom", &self.safe_zone_bottom.to_string());
        w.finish()
    }
}

impl fmt::Display for ClientConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let map: Vec<String> = self.server_channels.iter().map(|(k, v)| format!("{k}={v}")).collect();
        write!(
            f,
            "ClientConfig(pingVolume={}, pingDuration={}, pingDistance={}, correctionPeriod={}, itemIconVisible={}, \
             directionIndicatorVisible={}, playerInfoMode={}, teamColorMode={}, pingSize={}, channel={}, serverChannels={{{}}}, \
             removeRadius={}, raycastDistance={}, safeZoneLeft={}, safeZoneRight={}, safeZoneTop={}, safeZoneBottom={})",
            self.ping_volume,
            self.ping_duration,
            self.ping_distance,
            java_float(self.correction_period),
            self.item_icon_visible,
            self.direction_indicator_visible,
            self.player_info_mode.lower(),
            self.team_color_mode.lower(),
            self.ping_size,
            self.channel,
            map.join(", "),
            self.remove_radius,
            self.raycast_distance,
            self.safe_zone_left,
            self.safe_zone_right,
            self.safe_zone_top,
            self.safe_zone_bottom
        )
    }
}

// ---- Gson's ways ---------------------------------------------------------------------------------

impl ChannelMode {
    /// Gson reads an enum by its exact name; anything else is null (the default here).
    fn parse_exact(s: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|m| m.name() == s)
    }
}
impl PlayerInfoMode {
    fn parse_exact(s: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|m| m.name() == s)
    }
}
impl TeamColorMode {
    fn parse_exact(s: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|m| m.name() == s)
    }
}

/// A JSON number as Gson's int adapter takes it (a whole number in range).
fn int(v: &Value) -> Option<i32> {
    if let Some(i) = v.as_i64() {
        return i32::try_from(i).ok();
    }
    v.as_f64().filter(|f| f.fract() == 0.0).and_then(|f| i32::try_from(f as i64).ok())
}

/// `Float.toString`: always a fraction (`1.0`, `0.3`).
pub fn java_float(v: f32) -> String {
    let s = format!("{v:?}");
    if s.contains('.') || s.contains('e') || !v.is_finite() { s } else { format!("{s}.0") }
}

/// `JsonWriter.string` with Gson's default HTML-safe escaping.
pub fn gson_string(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '<' | '>' | '&' | '=' | '\'' | '\u{2028}' | '\u{2029}' => out.push_str(&format!("\\u{:04x}", c as u32)),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[derive(Default)]
struct Gson {
    fields: Vec<String>,
}

impl Gson {
    fn raw(&mut self, name: &str, value: &str) {
        self.fields.push(format!("  {}: {value}", gson_string(name)));
    }
    fn string(&mut self, name: &str, value: &str) {
        self.raw(name, &gson_string(value));
    }
    fn finish(self) -> String {
        format!("{{\n{}\n}}", self.fields.join(",\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLIENT_DEFAULT: &str = r#"{
  "pingVolume": 100,
  "pingDuration": 7,
  "pingDistance": 2048,
  "correctionPeriod": 1.0,
  "itemIconVisible": true,
  "directionIndicatorVisible": true,
  "playerInfoMode": "HOLD",
  "teamColorMode": "FULL",
  "pingSize": 100,
  "channel": "",
  "serverChannels": {},
  "removeRadius": 10,
  "raycastDistance": 1024,
  "safeZoneLeft": 5,
  "safeZoneRight": 5,
  "safeZoneTop": 5,
  "safeZoneBottom": 60
}"#;

    const SERVER_DEFAULT: &str = r#"{
  "defaultChannelMode": "AUTO",
  "playerTrackingEnabled": true,
  "msToRegenerate": 1000,
  "rateLimit": 5
}"#;

    #[test]
    fn defaults_as_gson_writes_them() {
        assert_eq!(ClientConfig::default().write(), CLIENT_DEFAULT);
        assert_eq!(ServerConfig::default().write(), SERVER_DEFAULT);
        assert_eq!(ClientConfig::parse(CLIENT_DEFAULT), Some(ClientConfig::default()));
        assert_eq!(ServerConfig::parse(SERVER_DEFAULT), Some(ServerConfig::default()));
    }

    #[test]
    fn server_channels_round_trip() {
        let mut c = ClientConfig::default();
        c.set_channel_for(Some("mc.example.com"), "red");
        c.set_channel_for(Some("localhost:25565"), "a<b");
        c.set_channel_for(None, "solo");
        c.correction_period = 0.3;
        let text = c.write();
        assert!(text.contains("  \"correctionPeriod\": 0.3,\n"));
        assert!(text.contains("  \"serverChannels\": {\n    \"mc.example.com\": \"red\",\n    \"localhost:25565\": \"a\\u003cb\"\n  },\n"));
        let back = ClientConfig::parse(&text).unwrap();
        assert_eq!(back.channel_for(Some("localhost:25565")), "a<b");
        assert_eq!(back.channel_for(Some("other")), "");
        assert_eq!(back.channel_for(None), "solo");
    }

    #[test]
    fn missing_fields_keep_defaults_and_broken_files_fail() {
        let c = ClientConfig::parse(r#"{"pingSize": 250, "playerInfoMode": "ALWAYS"}"#).unwrap();
        assert_eq!(c.ping_size, 250);
        assert_eq!(c.player_info_mode, PlayerInfoMode::Always);
        assert_eq!(c.ping_duration, 7);
        assert_eq!(ClientConfig::parse("{"), None);
        assert_eq!(ServerConfig::parse("[]"), None);
        let mut s = ServerConfig::parse(r#"{"defaultChannelMode": "TEAM_ONLY", "msToRegenerate": -5, "rateLimit": -1}"#).unwrap();
        s.validate();
        assert_eq!(s, ServerConfig { default_channel_mode: ChannelMode::TeamOnly, player_tracking_enabled: true, ms_to_regenerate: 1000, rate_limit: 0 });
    }

    #[test]
    fn channels_are_cut_to_128() {
        let mut c = ClientConfig { channel: "x".repeat(200), ..ClientConfig::default() };
        c.validate();
        assert_eq!(c.channel.len(), 128);
    }

    #[test]
    fn lombok_strings() {
        assert_eq!(ServerConfig::default().to_string(), "ServerConfig(defaultChannelMode=auto, playerTrackingEnabled=true, msToRegenerate=1000, rateLimit=5)");
        assert!(ClientConfig::default().to_string().starts_with("ClientConfig(pingVolume=100, pingDuration=7, pingDistance=2048, correctionPeriod=1.0,"));
    }

    #[test]
    fn enums() {
        assert_eq!(ChannelMode::parse("team_only"), Some(ChannelMode::TeamOnly));
        assert_eq!(ChannelMode::TeamOnly.lower(), "team_only");
        assert_eq!(PlayerInfoMode::Compact.cycle(false), PlayerInfoMode::Hold);
        assert_eq!(TeamColorMode::Full.cycle(true), TeamColorMode::LabelsOnly);
    }
}
