//! The server half: `ServerCore` (the relay of pings to the players of a channel, the channels,
//! the default channel's modes) and `RateLimiter`, as Ping Wheel's server runs them. Voice chat
//! groups and FTB Teams don't exist in Horizon, so a player's "team context"
//! (`TeamContextHandler`) is their vanilla scoreboard team.

use crate::config::{ChannelMode, ServerConfig};
use crate::protocol::{self, PingC2S, PingS2C};
use crate::ORIGINAL_VERSION;
use std::collections::HashMap;

/// A player on the server, as the relay needs them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Player {
    pub uuid: u128,
    pub name: String,
    /// Their scoreboard team's name (`Player.getTeam`).
    pub team: Option<String>,
}

/// What the server does about a packet.
#[derive(Clone, Debug, PartialEq)]
pub enum Out {
    /// `ping-wheel-s2c:ping-location` to a player.
    Send { to: u128, data: Vec<u8> },
    /// `sendSystemMessage(Component.literal(...))`: a line with `§` colour codes.
    Tell { to: u128, legacy: String },
    /// A line for the server log (`[Ping-Wheel] ...`), warning or not.
    Log { warn: bool, line: String },
}

/// `RateLimiter`: a token bucket of `limit` pings refilling one per `regen` milliseconds, on the
/// wall clock (`Instant.now()`). Times are nanoseconds from any fixed start.
#[derive(Clone, Debug, Default)]
pub struct RateLimiter {
    start: Option<i128>,
}

/// `RateLimiter.setRates`: the time one ping takes to come back, and the window of all of them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rates {
    pub regen: i128,
    pub window: i128,
}

impl Rates {
    pub fn new(ms_to_regenerate: i64, limit: i64) -> Self {
        Self { regen: i128::from(ms_to_regenerate) * 1_000_000, window: i128::from(ms_to_regenerate) * i128::from(limit) * 1_000_000 }
    }
}

impl RateLimiter {
    /// `checkExceeded`: true when the player has no ping left.
    pub fn check_exceeded(&mut self, rates: Rates, now: i128) -> bool {
        let Some(start) = self.start else {
            self.start = Some(now - rates.window + rates.regen);
            return false;
        };
        let elapsed = (now - start).min(rates.window);
        let left_over = elapsed - rates.regen;
        if left_over < 0 {
            return true;
        }
        self.start = Some(now - left_over);
        false
    }
}

/// `ServerCore`: the players' channels and rate limiters.
#[derive(Debug, Default)]
pub struct ServerCore {
    channels: HashMap<u128, String>,
    rates: HashMap<u128, RateLimiter>,
}

const PREFIX: &str = "[Ping-Wheel] ";

impl ServerCore {
    pub fn new() -> Self {
        Self::default()
    }

    /// The player's channel (`""`: the default channel).
    pub fn channel(&self, player: u128) -> &str {
        self.channels.get(&player).map_or("", String::as_str)
    }

    /// `onPlayerDisconnect`.
    pub fn disconnect(&mut self, player: u128) {
        self.channels.remove(&player);
        self.rates.remove(&player);
    }

    /// `updatePlayerChannel`.
    fn set_channel(&mut self, player: &Player, channel: &str, out: &mut Vec<Out>) {
        if channel.is_empty() {
            self.channels.remove(&player.uuid);
            out.push(Out::Log { warn: false, line: format!("{PREFIX}Channel update: {} -> default", player.name) });
        } else {
            self.channels.insert(player.uuid, channel.to_owned());
            out.push(Out::Log { warn: false, line: format!("{PREFIX}Channel update: {} -> \"{channel}\"", player.name) });
        }
    }

    /// `onChannelUpdate`: a `ping-wheel-c2s:update-channel` payload.
    pub fn on_channel_update(&mut self, player: &Player, data: &[u8]) -> Vec<Out> {
        let mut out = Vec::new();
        match protocol::decode_update_channel(data) {
            Some(channel) => self.set_channel(player, &channel, &mut out),
            None => {
                out.push(Out::Log { warn: true, line: format!("{PREFIX}invalid channel update from {} ({})", player.name, uuid_string(player.uuid)) });
                out.push(Out::Tell {
                    to: player.uuid,
                    legacy: format!("§8[Ping-Wheel] §cChannel couldn't be updated\n§fMake sure your version matches the server's version: §d{ORIGINAL_VERSION}"),
                });
            }
        }
        out
    }

    /// `onPingLocation`: a `ping-wheel-c2s:ping-location` payload from `sender`, with the players
    /// online (the sender among them) and the time in nanoseconds.
    pub fn on_ping(&mut self, sender: &Player, data: &[u8], online: &[Player], config: &ServerConfig, now: i128) -> Vec<Out> {
        let mut out = Vec::new();
        let Some(packet) = PingC2S::decode(data) else {
            out.push(Out::Log { warn: true, line: format!("{PREFIX}invalid ping location from {} ({})", sender.name, uuid_string(sender.uuid)) });
            out.push(Out::Tell {
                to: sender.uuid,
                legacy: format!("§8[Ping-Wheel] §cUnable to send ping\n§fMake sure your version matches the server's version: §d{ORIGINAL_VERSION}"),
            });
            return out;
        };
        let limiter = self.rates.entry(sender.uuid).or_default();
        if config.rate_limit > 0 && limiter.check_exceeded(config.rates(), now) {
            return out;
        }
        let channel = packet.channel.clone();
        let mode = config.default_channel_mode;
        if channel.is_empty() && mode == ChannelMode::Disabled {
            out.push(Out::Tell { to: sender.uuid, legacy: "§8[Ping-Wheel] §eMust be in a channel to ping location\n§fUse §a/pingwheel channel§f to switch".into() });
            return out;
        }
        if channel.is_empty() && mode == ChannelMode::TeamOnly && sender.team.is_none() {
            out.push(Out::Tell {
                to: sender.uuid,
                legacy: "§8[Ping-Wheel] §eMust be in a team or channel to ping location\n§fUse §a/pingwheel channel§f to switch".into(),
            });
            return out;
        }
        if channel != self.channel(sender.uuid) {
            self.set_channel(sender, &channel, &mut out);
        }
        // Player tracking off: a ping on a player loses its entity (the marker stays where it was).
        let on_player = packet.entity.is_some_and(|e| online.iter().any(|p| p.uuid == e));
        let mut relayed = PingS2C::from_client(&packet, sender.uuid);
        if !config.player_tracking_enabled && on_player {
            relayed.entity = None;
        }
        let Ok(bytes) = relayed.encode() else { return out };
        for p in online {
            if channel != self.channel(p.uuid) {
                continue;
            }
            // `TeamContextHandler.inSameContext`: the same team, or both without one.
            if channel.is_empty() && mode != ChannelMode::Global && sender.team != p.team {
                continue;
            }
            out.push(Out::Send { to: p.uuid, data: bytes.clone() });
        }
        out
    }
}

/// `UUID.toString`.
pub fn uuid_string(u: u128) -> String {
    let h = format!("{u:032x}");
    format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::java_hash;

    const MS: i128 = 1_000_000;

    fn player(uuid: u128, team: Option<&str>) -> Player {
        Player { uuid, name: format!("p{uuid}"), team: team.map(str::to_owned) }
    }

    fn ping(channel: &str, entity: Option<u128>) -> Vec<u8> {
        PingC2S { channel: channel.into(), pos: [1.0, 2.0, 3.0], entity, sequence: 4, dimension: java_hash("minecraft:overworld") }.encode().unwrap()
    }

    fn sent(out: &[Out]) -> Vec<u128> {
        out.iter().filter_map(|o| if let Out::Send { to, .. } = o { Some(*to) } else { None }).collect()
    }

    fn told(out: &[Out]) -> Vec<String> {
        out.iter().filter_map(|o| if let Out::Tell { legacy, .. } = o { Some(legacy.clone()) } else { None }).collect()
    }

    #[test]
    fn rate_limiter_is_a_token_bucket() {
        let rates = Rates::new(1000, 5);
        let mut r = RateLimiter::default();
        // Five at once, then none until a second has passed.
        for _ in 0..5 {
            assert!(!r.check_exceeded(rates, 0));
        }
        assert!(r.check_exceeded(rates, 0));
        assert!(r.check_exceeded(rates, 999 * MS));
        assert!(!r.check_exceeded(rates, 1000 * MS));
        assert!(r.check_exceeded(rates, 1500 * MS));
        // A long pause fills the bucket to five again, no more.
        let later = 60_000 * MS;
        for _ in 0..5 {
            assert!(!r.check_exceeded(rates, later));
        }
        assert!(r.check_exceeded(rates, later));
    }

    #[test]
    fn default_channel_reaches_everyone_without_a_team() {
        let mut core = ServerCore::new();
        let (a, b, c) = (player(1, None), player(2, None), player(3, Some("red")));
        let online = [a.clone(), b.clone(), c.clone()];
        let out = core.on_ping(&a, &ping("", None), &online, &ServerConfig::default(), 0);
        // AUTO: the sender and b (both team-less), not c on a team.
        assert_eq!(sent(&out), vec![1, 2]);
        let data = out.iter().find_map(|o| if let Out::Send { data, .. } = o { Some(data.clone()) } else { None }).unwrap();
        let s2c = PingS2C::decode(&data).unwrap();
        assert_eq!(s2c.author, 1);
        assert_eq!(s2c.pos, [1.0, 2.0, 3.0]);
        assert_eq!(s2c.sequence, 4);
    }

    #[test]
    fn teams_and_global_mode() {
        let mut core = ServerCore::new();
        let (a, b, c) = (player(1, Some("red")), player(2, Some("red")), player(3, Some("blue")));
        let online = [a.clone(), b.clone(), c.clone()];
        assert_eq!(sent(&core.on_ping(&a, &ping("", None), &online, &ServerConfig::default(), 0)), vec![1, 2]);
        let global = ServerConfig { default_channel_mode: ChannelMode::Global, ..ServerConfig::default() };
        assert_eq!(sent(&core.on_ping(&a, &ping("", None), &online, &global, 0)), vec![1, 2, 3]);
    }

    #[test]
    fn named_channels() {
        let mut core = ServerCore::new();
        let (a, b, c) = (player(1, None), player(2, Some("x")), player(3, None));
        let online = [a.clone(), b.clone(), c.clone()];
        let out = core.on_channel_update(&b, &protocol::encode_update_channel("squad").unwrap());
        assert!(matches!(&out[0], Out::Log { line, .. } if line == "[Ping-Wheel] Channel update: p2 -> \"squad\""));
        // a pings on "squad": joins it on the way, reaches b whatever the teams.
        let out = core.on_ping(&a, &ping("squad", None), &online, &ServerConfig::default(), 0);
        assert_eq!(sent(&out), vec![1, 2]);
        assert_eq!(core.channel(1), "squad");
        // Back to the default channel.
        let out = core.on_channel_update(&a, &protocol::encode_update_channel("").unwrap());
        assert!(matches!(&out[0], Out::Log { line, .. } if line == "[Ping-Wheel] Channel update: p1 -> default"));
        assert_eq!(sent(&core.on_ping(&c, &ping("", None), &online, &ServerConfig::default(), 0)), vec![1, 3]);
        core.disconnect(2);
        assert_eq!(core.channel(2), "");
    }

    #[test]
    fn disabled_and_team_only_modes() {
        let mut core = ServerCore::new();
        let (a, b) = (player(1, None), player(2, Some("red")));
        let online = [a.clone(), b.clone()];
        let disabled = ServerConfig { default_channel_mode: ChannelMode::Disabled, ..ServerConfig::default() };
        let out = core.on_ping(&a, &ping("", None), &online, &disabled, 0);
        assert!(sent(&out).is_empty());
        assert_eq!(told(&out), vec!["§8[Ping-Wheel] §eMust be in a channel to ping location\n§fUse §a/pingwheel channel§f to switch".to_owned()]);
        let team_only = ServerConfig { default_channel_mode: ChannelMode::TeamOnly, ..ServerConfig::default() };
        let out = core.on_ping(&a, &ping("", None), &online, &team_only, 0);
        assert_eq!(told(&out), vec!["§8[Ping-Wheel] §eMust be in a team or channel to ping location\n§fUse §a/pingwheel channel§f to switch".to_owned()]);
        assert_eq!(sent(&core.on_ping(&b, &ping("", None), &online, &team_only, 0)), vec![2]);
    }

    #[test]
    fn rate_limit_drops_silently() {
        let mut core = ServerCore::new();
        let a = player(1, None);
        let online = [a.clone()];
        let config = ServerConfig { rate_limit: 2, ..ServerConfig::default() };
        assert_eq!(sent(&core.on_ping(&a, &ping("", None), &online, &config, 0)), vec![1]);
        assert_eq!(sent(&core.on_ping(&a, &ping("", None), &online, &config, 0)), vec![1]);
        let out = core.on_ping(&a, &ping("", None), &online, &config, 0);
        assert!(out.is_empty());
        // Limit 0: no limit.
        let off = ServerConfig { rate_limit: 0, ..ServerConfig::default() };
        for _ in 0..20 {
            assert_eq!(sent(&core.on_ping(&a, &ping("", None), &online, &off, 0)), vec![1]);
        }
    }

    #[test]
    fn player_tracking_strips_player_entities() {
        let mut core = ServerCore::new();
        let (a, b) = (player(1, None), player(2, None));
        let online = [a.clone(), b.clone()];
        let off = ServerConfig { player_tracking_enabled: false, ..ServerConfig::default() };
        let entity = |out: &[Out]| match &out[0] {
            Out::Send { data, .. } => PingS2C::decode(data).unwrap().entity,
            o => panic!("{o:?}"),
        };
        assert_eq!(entity(&core.on_ping(&a, &ping("", Some(2)), &online, &off, 0)), None);
        // Not a player (a mob, an item): kept.
        assert_eq!(entity(&core.on_ping(&a, &ping("", Some(99)), &online, &off, 0)), Some(99));
        assert_eq!(entity(&core.on_ping(&a, &ping("", Some(2)), &online, &ServerConfig::default(), 0)), Some(2));
    }

    #[test]
    fn corrupt_packets_answer_the_sender() {
        let mut core = ServerCore::new();
        let a = player(0x069a79f4_44e9_4726_a5be_fca90e38aaf5, None);
        let out = core.on_ping(&a, &[0x05, b'a'], std::slice::from_ref(&a), &ServerConfig::default(), 0);
        assert_eq!(told(&out), vec!["§8[Ping-Wheel] §cUnable to send ping\n§fMake sure your version matches the server's version: §d1.12.2".to_owned()]);
        assert!(matches!(&out[0], Out::Log { warn: true, line } if line.ends_with("(069a79f4-44e9-4726-a5be-fca90e38aaf5)")));
        let out = core.on_channel_update(&a, &[0x7F]);
        assert_eq!(told(&out), vec!["§8[Ping-Wheel] §cChannel couldn't be updated\n§fMake sure your version matches the server's version: §d1.12.2".to_owned()]);
    }
}
