//! The voice server's settings file, `config.toml` in the server's Plasmo Voice folder
//! (`config/plasmovoice/server/`, `ModVoiceServer.getConfigFolder`), as Plasmo Voice reads and
//! writes it (`VoiceServerConfig` with plo-config's TOML: defaults for what it lacks, values its
//! validators refuse set back to their defaults, the distances sorted, and the file written back
//! at once with every field and its comment).
//
// This Source Code Form is a port of Plasmo Voice (https://github.com/plasmoapp/plasmo-voice), LGPL-3.0.

use crate::wire::Uuid;
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// `VoiceServerConfig.Host.Public`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostPublic {
    pub ip: String,
    pub port: u16,
}

/// `VoiceServerConfig.Host`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Host {
    pub ip: String,
    /// 0: the Minecraft server's port.
    pub port: u16,
    pub public: Option<HostPublic>,
}

/// `VoiceServerConfig.Voice.ReusePort` (kept in the file; Horizon binds one socket).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReusePort {
    pub enabled: bool,
    pub sockets: i64,
}

/// `VoiceServerConfig.Voice`.
#[derive(Clone, Debug, PartialEq)]
pub struct Voice {
    pub max_extra_audio_broadcast_distance: i32,
    pub sample_rate: i32,
    pub keep_alive_timeout_ms: i64,
    pub mtu_size: i32,
    pub client_mod_required: bool,
    pub client_mod_required_check_timeout_ms: i64,
    pub client_mod_min_version: String,
    pub reuse_port: ReusePort,
    pub distances: Vec<i32>,
    pub default_distance: i32,
    pub opus_mode: String,
    pub opus_bitrate: i32,
    /// `PlayerIconVisibility` names.
    pub icon_visibility: Vec<String>,
    pub icon_y_offset: f64,
    /// `[voice.weights.activations]` and `[voice.weights.source_lines]`.
    pub activation_weights: BTreeMap<String, i32>,
    pub source_line_weights: BTreeMap<String, i32>,
}

/// `VoiceServerConfig`.
#[derive(Clone, Debug, PartialEq)]
pub struct ServerConfig {
    pub server_id: String,
    pub default_language: String,
    pub forced_language: Option<String>,
    pub debug: bool,
    pub use_crowdin_translations: bool,
    pub check_for_updates: bool,
    pub host: Host,
    pub voice: Voice,
    pub notify_muted: bool,
    pub notify_unmuted: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        ServerConfig {
            server_id: random_uuid().to_string(),
            default_language: "en_us".into(),
            forced_language: None,
            debug: false,
            use_crowdin_translations: true,
            check_for_updates: true,
            host: Host { ip: "0.0.0.0".into(), port: 0, public: None },
            voice: Voice {
                max_extra_audio_broadcast_distance: 16,
                sample_rate: 48_000,
                keep_alive_timeout_ms: 15_000,
                mtu_size: 1024,
                client_mod_required: false,
                client_mod_required_check_timeout_ms: 3_000,
                client_mod_min_version: "2.0.0".into(),
                reuse_port: ReusePort { enabled: false, sockets: 4 },
                distances: vec![8, 16, 32],
                default_distance: 16,
                opus_mode: "VOIP".into(),
                opus_bitrate: -1000,
                icon_visibility: Vec::new(),
                icon_y_offset: 0.0,
                activation_weights: BTreeMap::new(),
                source_line_weights: BTreeMap::new(),
            },
            notify_muted: true,
            notify_unmuted: true,
        }
    }
}

pub(crate) fn random_uuid() -> Uuid {
    let mut b = [0u8; 16];
    let _ = getrandom::fill(&mut b);
    // Version 4, IETF variant, as `UUID.randomUUID`.
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    Uuid(u128::from_be_bytes(b))
}

/// `UUID.fromString`, for the forms Java accepts from a config (with its dashes).
pub fn parse_uuid(s: &str) -> Option<Uuid> {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 5 || parts.iter().any(|p| p.is_empty() || p.len() > 16) {
        return None;
    }
    let n: Vec<u64> = parts.iter().map(|p| u64::from_str_radix(p, 16).ok()).collect::<Option<_>>()?;
    let msb = (n[0] << 32) | ((n[1] & 0xffff) << 16) | (n[2] & 0xffff);
    let lsb = ((n[3] & 0xffff) << 48) | (n[4] & 0xffff_ffff_ffff);
    Some(Uuid((u128::from(msb) << 64) | u128::from(lsb)))
}

type Table = toml::Table;

fn table<'a>(t: &'a Table, key: &str) -> Option<&'a Table> {
    t.get(key)?.as_table()
}

fn int(t: Option<&Table>, key: &str) -> Option<i64> {
    t?.get(key)?.as_integer()
}

fn boolean(t: Option<&Table>, key: &str) -> Option<bool> {
    t?.get(key)?.as_bool()
}

fn string(t: Option<&Table>, key: &str) -> Option<String> {
    t?.get(key)?.as_str().map(str::to_owned)
}

fn port(v: Option<i64>, default: u16) -> u16 {
    // `PortValidator`: 0 to 65535.
    v.and_then(|p| u16::try_from(p).ok()).unwrap_or(default)
}

impl ServerConfig {
    /// `TOML.load(VoiceServerConfig.class, file, false)`: the file's values over the defaults;
    /// an invalid value (the validators') keeps its default. A file that isn't TOML is an error.
    pub fn parse(text: &str) -> Result<ServerConfig, String> {
        let root: Table = text.parse::<Table>().map_err(|e| e.to_string())?;
        let mut c = ServerConfig::default();
        let r = Some(&root);
        if let Some(id) = string(r, "server_id") {
            c.server_id = id;
        }
        // `UUID.fromString` or a new random id (`BaseVoiceServer.loadConfig`).
        if parse_uuid(&c.server_id).is_none() {
            c.server_id = random_uuid().to_string();
        }
        if let Some(v) = string(r, "default_language") {
            c.default_language = v;
        }
        c.forced_language = string(r, "forced_language");
        c.debug = boolean(r, "debug").unwrap_or(c.debug);
        c.use_crowdin_translations = boolean(r, "use_crowdin_translations").unwrap_or(c.use_crowdin_translations);
        c.check_for_updates = boolean(r, "check_for_updates").unwrap_or(c.check_for_updates);

        let host = table(&root, "host");
        if let Some(ip) = string(host, "ip") {
            c.host.ip = ip;
        }
        c.host.port = port(int(host, "port"), c.host.port);
        if let Some(public) = host.and_then(|h| table(h, "public")) {
            let p = Some(public);
            c.host.public = Some(HostPublic { ip: string(p, "ip").unwrap_or_else(|| "127.0.0.1".into()), port: port(int(p, "port"), 0) });
        }

        let voice = table(&root, "voice");
        let v = &mut c.voice;
        if let Some(n) = int(voice, "max_extra_audio_broadcast_distance").and_then(|n| i32::try_from(n).ok()) {
            v.max_extra_audio_broadcast_distance = n;
        }
        if let Some(n) = int(voice, "sample_rate").filter(|n| [8_000, 12_000, 24_000, 48_000].contains(n)) {
            v.sample_rate = n as i32;
        }
        if let Some(n) = int(voice, "keep_alive_timeout_ms").filter(|n| (1_000..=120_000).contains(n)) {
            v.keep_alive_timeout_ms = n;
        }
        if let Some(n) = int(voice, "mtu_size").filter(|n| (128..=5_000).contains(n)) {
            v.mtu_size = n as i32;
        }
        v.client_mod_required = boolean(voice, "client_mod_required").unwrap_or(v.client_mod_required);
        if let Some(n) = int(voice, "client_mod_required_check_timeout_ms") {
            v.client_mod_required_check_timeout_ms = n;
        }
        if let Some(s) = string(voice, "client_mod_min_version") {
            v.client_mod_min_version = s;
        }
        let reuse = voice.and_then(|t| table(t, "reuse_port"));
        v.reuse_port.enabled = boolean(reuse, "enabled").unwrap_or(v.reuse_port.enabled);
        v.reuse_port.sockets = int(reuse, "sockets").unwrap_or(v.reuse_port.sockets);

        let proximity = voice.and_then(|t| table(t, "proximity"));
        if let Some(list) = proximity.and_then(|t| t.get("distances")).and_then(|d| d.as_array()) {
            let mut d: Vec<i32> = list.iter().filter_map(|x| x.as_integer()).filter_map(|x| i32::try_from(x).ok()).collect();
            // `DistancesSorter`.
            d.sort_unstable();
            v.distances = d;
        }
        if let Some(n) = int(proximity, "default_distance").and_then(|n| i32::try_from(n).ok()) {
            v.default_distance = n;
        }

        let opus = voice.and_then(|t| table(t, "opus"));
        if let Some(m) = string(opus, "mode").filter(|m| ["VOIP", "AUDIO", "RESTRICTED_LOWDELAY"].contains(&m.as_str())) {
            v.opus_mode = m;
        }
        if let Some(b) = int(opus, "bitrate").filter(|b| *b == -1 || *b == -1000 || (500..=512_000).contains(b)) {
            v.opus_bitrate = b as i32;
        }

        let icon = voice.and_then(|t| table(t, "player_icon"));
        if let Some(list) = icon.and_then(|t| t.get("visibility")).and_then(|d| d.as_array()) {
            let names: Vec<String> = list.iter().filter_map(|x| x.as_str()).map(str::to_owned).collect();
            if names.iter().all(|n| crate::tcp::ICON_VISIBILITY.contains(&n.as_str())) {
                v.icon_visibility = names;
            }
        }
        if let Some(y) = icon.and_then(|t| t.get("y_offset")).and_then(|y| y.as_float().or_else(|| y.as_integer().map(|i| i as f64))) {
            v.icon_y_offset = y;
        }

        let weights = voice.and_then(|t| table(t, "weights"));
        let read_weights = |name: &str| -> BTreeMap<String, i32> {
            weights
                .and_then(|w| table(w, name))
                .map(|t| t.iter().filter_map(|(k, x)| Some((k.clone(), i32::try_from(x.as_integer()?).ok()?))).collect())
                .unwrap_or_default()
        };
        v.activation_weights = read_weights("activations");
        v.source_line_weights = read_weights("source_lines");

        let notifications = table(&root, "notifications");
        c.notify_muted = boolean(notifications, "muted").unwrap_or(c.notify_muted);
        c.notify_unmuted = boolean(notifications, "unmuted").unwrap_or(c.notify_unmuted);
        Ok(c)
    }

    /// `TOML.save(config, file)`: plo-config's output, every field with its comment.
    pub fn write(&self) -> String {
        let mut o = String::new();
        let v = &self.voice;
        let _ = write!(
            o,
            "# Used to store server-related config file on the client\n\
             # Set it to a single value on different servers if you want them to share config\n\
             server_id = {}\n\
             # Language used when client's language doesn't exist\n\
             # \n\
             # By default, client's language used for translations\n\
             # For example, if default_language is set to ja_jp but the client uses en_us, then en_us will be used\n\
             # If you want to use one specific language for all clients, uncomment and set forced_language below\n\
             default_language = {}\n\
             # Language used for all clients\n",
            quote(&self.server_id),
            quote(&self.default_language),
        );
        match &self.forced_language {
            Some(l) => {
                let _ = writeln!(o, "forced_language = {}", quote(l));
            }
            None => o.push_str("#forced_language = \"ja_jp\"\n"),
        }
        let _ = write!(
            o,
            "# Enables debug logging\n\
             debug = {}\n\
             # Enables translations from Crowdin\n\
             use_crowdin_translations = {}\n\
             check_for_updates = {}\n\
             \n\
             [host]\n\
             # IP address for the voice server to bind to\n\
             # 0.0.0.0 = bind to all available interfaces\n\
             ip = {}\n\
             # UDP port for the voice server\n\
             # 0 = same port as Minecraft server\n\
             port = {}\n\
             \n\
             # Public IP and port that clients will connect to\n\
             # If ip set to 0.0.0.0, ip from [host] will be used\n\
             # If port set to 0, port from [host] will be used\n\
             # Ignored when using BungeeCord/Velocity (proxy manages public address instead)\n",
            self.debug,
            self.use_crowdin_translations,
            self.check_for_updates,
            quote(&self.host.ip),
            self.host.port,
        );
        match &self.host.public {
            Some(p) => {
                let _ = write!(o, "[host.public]\nip = {}\nport = {}\n", quote(&p.ip), p.port);
            }
            None => o.push_str("#[host.public]\n#ip = \"127.0.0.1\"\n#port = 0\n"),
        }
        let _ = write!(
            o,
            "\n[voice]\n\
             # The maximum amount, in blocks, to broadcast audio packets past the audible proximity distance.\n\
             # The plugin will send audio packets to players within 2x the proximity distance, or the distance plus this number - whichever is smaller.\n\
             max_extra_audio_broadcast_distance = {}\n\
             # Supported sample rates: [8000, 12000, 24000, 48000]\n\
             # Don't change this to reduce bandwidth; adjust opus bitrate instead\n\
             sample_rate = {}\n\
             # Time before voice client connection is considered timed out\n\
             # Server will try to automatically reconnect the client after timeout\n\
             keep_alive_timeout_ms = {}\n\
             # Maximum packet size for voice data transmission\n\
             mtu_size = {}\n\
             # Requires players to have the Plasmo Voice mod installed\n\
             client_mod_required = {}\n\
             # Time before player is kicked if the mod is required but not installed\n\
             client_mod_required_check_timeout_ms = {}\n\
             # Minimum required version for a client with the mod to connect to the voice server\n\
             # This will not kick the player but will simply not connect them to the voice server and will suggest downloading the required version\n\
             client_mod_min_version = {}\n\
             \n\
             [voice.reuse_port]\n\
             # Enables SO_REUSEPORT\n\
             # Instead of using one single thread for all incoming connections,\n\
             # multiple sockets will be bound on the same port\n\
             # and distribute connections between multiple worker threads\n\
             # Requires Linux or macOS\n\
             enabled = {}\n\
             # Amount of sockets that'll be open with SO_REUSEPORT enabled\n\
             # When set to 0, amount of channel will match amount of available CPU cores * 2\n\
             sockets = {}\n\
             \n\
             [voice.proximity]\n\
             distances = [{}]\n\
             default_distance = {}\n\
             \n\
             [voice.opus]\n\
             # Opus application mode\n\
             # Supported values: VOIP, AUDIO, RESTRICTED_LOWDELAY\n\
             # Default is VOIP\n\
             mode = {}\n\
             # Opus bitrate\n\
             # Supported values: -1000 (auto), -1 (max), [500-512_000]\n\
             # Default is -1000\n\
             bitrate = {}\n\
             \n\
             [voice.player_icon]\n\
             # Controls which icons are hidden above player heads\n\
             # Leave empty to show all icons (default)\n\
             # To hide all icons, use: [\"HIDE_NOT_INSTALLED\", \"HIDE_VOICE_CHAT_DISABLED\", \"HIDE_SERVER_MUTED\", \"HIDE_CLIENT_MUTED\", \"HIDE_SOURCE_ICON\"]\n\
             # \n\
             # Available options:\n\
             # HIDE_NOT_INSTALLED - hides icon when player doesn't have Plasmo Voice installed\n\
             # HIDE_VOICE_CHAT_DISABLED - hides icon when player disables voice chat on the client\n\
             # HIDE_SERVER_MUTED - hides icon when player's voice chat is muted on the server\n\
             # HIDE_CLIENT_MUTED - hides icon when player is muted on the client using Volume tab\n\
             # HIDE_SOURCE_ICON - hides icon when player is talking\n\
             visibility = [{}]\n\
             # Controls the y offset of the icon above player heads\n\
             y_offset = {}\n\
             \n\
             # Set custom weights for specific activations and source lines\n\
             # Weight determines display order in client menu and overlay (lower = higher position)\n",
            v.max_extra_audio_broadcast_distance,
            v.sample_rate,
            v.keep_alive_timeout_ms,
            v.mtu_size,
            v.client_mod_required,
            v.client_mod_required_check_timeout_ms,
            quote(&v.client_mod_min_version),
            v.reuse_port.enabled,
            v.reuse_port.sockets,
            v.distances.iter().map(i32::to_string).collect::<Vec<_>>().join(", "),
            v.default_distance,
            quote(&v.opus_mode),
            v.opus_bitrate,
            v.icon_visibility.iter().map(|s| quote(s)).collect::<Vec<_>>().join(", "),
            java_double(v.icon_y_offset),
        );
        for (name, map) in [("activations", &v.activation_weights), ("source_lines", &v.source_line_weights)] {
            if map.is_empty() {
                let _ = write!(o, "#[voice.weights.{name}]\n#groups = 0\n\n");
            } else {
                let _ = writeln!(o, "[voice.weights.{name}]");
                for (k, w) in map {
                    let _ = writeln!(o, "{} = {w}", key(k));
                }
                o.push('\n');
            }
        }
        let _ = write!(
            o,
            "[notifications]\n\
             # Notify a player when their voice chat is muted (\"You've been muted ...\" message)\n\
             muted = {}\n\
             # Notify a player when their voice chat is unmuted (\"You've been unmuted\" message)\n\
             unmuted = {}\n",
            self.notify_muted, self.notify_unmuted,
        );
        o
    }
}

/// A TOML basic string.
fn quote(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 || c == '\u{7f}' => {
                let _ = write!(o, "\\u{:04X}", c as u32);
            }
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// A bare key when it can be one, else quoted.
fn key(k: &str) -> String {
    if !k.is_empty() && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') { k.to_owned() } else { quote(k) }
}

/// `Double.toString` for the plain values a config holds (`0.0`, `1.5`, `-0.25`).
fn java_double(d: f64) -> String {
    if d.is_finite() && d == d.trunc() && d.abs() < 1e7 { format!("{d:.1}") } else { format!("{d}") }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What PlasmoVoice-Paper 2.2.0-beta.3 wrote (its defaults, with this test's server id).
    const PLASMO: &str = include_str!("testdata/plasmo-config.toml");

    #[test]
    fn writes_what_plasmo_writes() {
        let mut c = ServerConfig::default();
        c.server_id = "1fb15b74-2e5f-48a1-a610-4891d834a25c".into();
        assert_eq!(c.write(), PLASMO.replace("\r\n", "\n"));
    }

    #[test]
    fn reads_back_what_it_writes() {
        let c = ServerConfig::parse(PLASMO).unwrap();
        assert_eq!(c.server_id, "1fb15b74-2e5f-48a1-a610-4891d834a25c");
        assert_eq!(c.voice.distances, vec![8, 16, 32]);
        assert_eq!(c.host, Host { ip: "0.0.0.0".into(), port: 0, public: None });
        assert_eq!(ServerConfig::parse(&c.write()).unwrap(), c);
    }

    #[test]
    fn defaults_for_what_is_missing_or_refused() {
        let c = ServerConfig::parse(
            "server_id = \"nope\"\n[host]\nport = 70000\n[host.public]\nip = \"voice.example.org\"\nport = 24454\n\
             [voice]\nsample_rate = 44100\nmtu_size = 64\n[voice.proximity]\ndistances = [32, 8, 64]\ndefault_distance = 64\n\
             [voice.opus]\nmode = \"LOUD\"\nbitrate = 64000\n[voice.player_icon]\nvisibility = [\"HIDE_SOURCE_ICON\"]\ny_offset = 0.5\n\
             [voice.weights.activations]\nproximity = 5\n[notifications]\nunmuted = false\n",
        )
        .unwrap();
        assert!(parse_uuid(&c.server_id).is_some(), "a new id for a bad one");
        assert_eq!(c.host.port, 0);
        assert_eq!(c.host.public, Some(HostPublic { ip: "voice.example.org".into(), port: 24454 }));
        assert_eq!((c.voice.sample_rate, c.voice.mtu_size), (48_000, 1024));
        assert_eq!((c.voice.distances.clone(), c.voice.default_distance), (vec![8, 32, 64], 64));
        assert_eq!((c.voice.opus_mode.as_str(), c.voice.opus_bitrate), ("VOIP", 64_000));
        assert_eq!((c.voice.icon_visibility.clone(), c.voice.icon_y_offset), (vec!["HIDE_SOURCE_ICON".to_owned()], 0.5));
        assert_eq!(c.voice.activation_weights.get("proximity"), Some(&5));
        assert!(c.notify_muted && !c.notify_unmuted);
        let text = c.write();
        assert!(text.contains("[host.public]\nip = \"voice.example.org\"\nport = 24454\n"));
        assert!(text.contains("[voice.weights.activations]\nproximity = 5\n\n#[voice.weights.source_lines]"));
        assert!(text.contains("y_offset = 0.5\n"));
        assert_eq!(ServerConfig::parse(&text).unwrap(), c);
        assert!(ServerConfig::parse("[host\nport = ").is_err());
    }

    #[test]
    fn uuids_as_java_reads_them() {
        assert_eq!(parse_uuid("1fb15b74-2e5f-48a1-a610-4891d834a25c").unwrap().to_string(), "1fb15b74-2e5f-48a1-a610-4891d834a25c");
        assert!(parse_uuid("1fb15b742e5f48a1a6104891d834a25c").is_none());
    }
}
