//! Plasmo Voice's server commands (`VoiceListCommand`, `VoiceMuteCommand`, `VoiceUnmuteCommand`,
//! `VoiceMuteListCommand`, `VoiceReloadCommand`, `VoiceReconnectCommand`), their permission nodes
//! and defaults (`Permission`), and their completions (`MuteDurationType`, `UnmuteTargetsType`).
//! The game checks who may run them; these parse the line, act, and answer in the sender's
//! language.
//
// This Source Code Form is a port of Plasmo Voice (https://github.com/plasmoapp/plasmo-voice), LGPL-3.0.

use crate::mutes::{self, DurationError, MuteDuration};
use crate::server::{Server, now_millis};
use crate::server_config::parse_uuid;
use crate::wire::Uuid;

/// `PermissionDefault`: everyone, or the server's operators.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Default {
    Everyone,
    Op,
}

/// One command as the game registers it: its usage (`name <arg:kind> [opt:kind]`), the
/// permission node that decides who runs it, and the node's default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommandSpec {
    pub usage: &'static str,
    pub permission: &'static str,
    pub default: Default,
}

/// The commands, in Horizon's usage syntax: a player argument completes the online players, a
/// `?` kind asks the server for completions (the mute durations, the muted players).
pub const COMMANDS: [CommandSpec; 6] = [
    CommandSpec { usage: "vlist", permission: "pv.list", default: Default::Everyone },
    CommandSpec { usage: "vmute <targets:player> [duration:word?] [reason:text]", permission: "pv.mute", default: Default::Op },
    CommandSpec { usage: "vunmute <targets:player_name>", permission: "pv.unmute", default: Default::Op },
    CommandSpec { usage: "vmutelist", permission: "pv.mutelist", default: Default::Op },
    CommandSpec { usage: "vreload", permission: "pv.reload", default: Default::Op },
    CommandSpec { usage: "vrc", permission: "pv.reconnect", default: Default::Everyone },
];

/// Who runs a command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Sender {
    /// The server's console.
    Console,
    /// A player on the server.
    Player { id: Uuid, name: String },
}

impl Sender {
    fn id(&self) -> Option<Uuid> {
        match self {
            Sender::Console => None,
            Sender::Player { id, .. } => Some(*id),
        }
    }
}

/// The words of a command line, the last one the rest of the line (`greedyString`).
fn words(line: &str) -> (&str, Vec<&str>) {
    let line = line.trim_start_matches('/');
    let (label, rest) = line.split_once(' ').unwrap_or((line, ""));
    (label, rest.split(' ').filter(|w| !w.is_empty()).collect())
}

/// `/vmute`'s arguments: the target, the duration (`permanent` when left out) and the reason
/// (the rest of the line, none when blank).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MuteArgs {
    pub target: String,
    pub duration: MuteDuration,
    pub reason: Option<String>,
}

/// Parses `/vmute`'s arguments (after the command's name): None without a target.
pub fn parse_mute(args: &str, now: i64) -> Option<Result<MuteArgs, DurationError>> {
    let args = args.trim_start();
    let (target, rest) = args.split_once(' ').unwrap_or((args, ""));
    if target.is_empty() {
        return None;
    }
    let rest = rest.trim_start_matches(' ');
    let (duration, reason) = rest.split_once(' ').unwrap_or((rest, ""));
    let duration = if duration.is_empty() {
        MuteDuration::Permanent
    } else {
        match mutes::parse_duration(duration, now) {
            Ok(d) => d,
            Err(e) => return Some(Err(e)),
        }
    };
    let reason = Some(reason.to_owned()).filter(|r| !r.trim().is_empty());
    Some(Ok(MuteArgs { target: target.to_owned(), duration, reason }))
}

impl Server {
    /// The command names the server answers (`vlist`, `vmute`...).
    pub fn command_names() -> Vec<&'static str> {
        COMMANDS.iter().map(|c| c.usage.split(' ').next().unwrap_or_default()).collect()
    }

    /// Runs one of the commands (`line` without its slash) for `sender`: the lines to answer
    /// with, or None when it isn't one of them.
    pub fn command(&mut self, sender: &Sender, line: &str) -> Option<Vec<String>> {
        let line = line.trim_start_matches('/');
        let (label, _) = words(line);
        let args = line.split_once(' ').map_or("", |(_, a)| a);
        let lang = {
            let s = self.shared.lock().unwrap_or_else(|e| e.into_inner());
            sender.id().and_then(|id| s.players.get(&id)).and_then(|p| p.text_language().map(str::to_owned))
        };
        let lang = lang.as_deref();
        let out = match label {
            "vlist" => self.list(lang),
            "vmute" => self.mute_command(sender, args, lang),
            "vunmute" => self.unmute_command(sender, args, lang),
            "vmutelist" => self.mute_list(lang),
            "vreload" => match self.reload() {
                Ok(()) => vec![self.tr(lang, "pv.command.reload.message", &[])],
                Err(e) => {
                    log::error!(target: crate::LOG, "[Plasmo Voice] {e}");
                    vec![format!("An unexpected error occurred trying to execute that command: {e}")]
                }
            },
            "vrc" => self.reconnect(sender, lang),
            _ => return None,
        };
        Some(out)
    }

    fn tr(&self, lang: Option<&str>, key: &str, args: &[&str]) -> String {
        self.shared.lock().unwrap_or_else(|e| e.into_inner()).tr(lang, key, args)
    }

    /// `/vlist`: who has voice chat of everyone on.
    fn list(&self, lang: Option<&str>) -> Vec<String> {
        let s = self.shared.lock().unwrap_or_else(|e| e.into_inner());
        let mut with_voice: Vec<String> = s.players.values().filter(|p| p.connection.is_some()).map(|p| p.name.clone()).collect();
        with_voice.sort();
        let names = if with_voice.is_empty() { s.tr(lang, "pv.command.list.empty", &[]) } else { with_voice.join(", ") };
        vec![s.tr(lang, "pv.command.list.message", &[&with_voice.len().to_string(), &s.players.len().to_string(), &names])]
    }

    /// The players a target argument names (`McArgumentTypes.players()`): a name or UUID of a
    /// player on, or `@a`, `@s`, `@p`, `@r`.
    fn resolve_online(&self, sender: &Sender, target: &str) -> Vec<(Uuid, String)> {
        let s = self.shared.lock().unwrap_or_else(|e| e.into_inner());
        let mut all: Vec<(Uuid, String)> = s.players.iter().map(|(id, p)| (*id, p.name.clone())).collect();
        all.sort_by(|a, b| a.1.cmp(&b.1));
        match target {
            "@a" => all,
            "@s" => sender.id().and_then(|id| all.into_iter().find(|p| p.0 == id)).into_iter().collect(),
            "@p" => {
                // The nearest to the sender (a player is their own nearest), or to the world's origin.
                if let Some(me) = sender.id().and_then(|id| all.iter().find(|p| p.0 == id).cloned()) {
                    return vec![me];
                }
                let dist = |id: &Uuid| s.players.get(id).and_then(|p| p.position.as_ref()).map_or(f64::MAX, |p| p.pos.iter().map(|x| x * x).sum());
                all.into_iter().min_by(|a, b| dist(&a.0).total_cmp(&dist(&b.0))).into_iter().collect()
            }
            "@r" => {
                if all.is_empty() {
                    return all;
                }
                let mut r = [0u8; 4];
                let _ = getrandom::fill(&mut r);
                let i = u32::from_le_bytes(r) as usize % all.len();
                vec![all.swap_remove(i)]
            }
            _ => match parse_uuid(target) {
                Some(id) => all.into_iter().filter(|p| p.0 == id).collect(),
                None => all.into_iter().filter(|p| p.1.eq_ignore_ascii_case(target)).collect(),
            },
        }
    }

    /// `/vmute <targets> [duration] [reason]`.
    fn mute_command(&mut self, sender: &Sender, args: &str, lang: Option<&str>) -> Vec<String> {
        let now = now_millis();
        let parsed = match parse_mute(args, now) {
            None => {
                let usage = self.shared.lock().unwrap_or_else(|e| e.into_inner()).langs.raw(lang, "pv.command.mute.usage", "Usage: /vmute <player> [duration] [reason]");
                return vec![usage];
            }
            Some(Err(e)) => return vec![self.tr(lang, e.key(), &[])],
            Some(Ok(a)) => a,
        };
        let targets = self.resolve_online(sender, &parsed.target);
        if targets.is_empty() {
            return vec![self.tr(lang, "pv.error.player_not_found", &[])];
        }
        let mut s = self.shared.lock().unwrap_or_else(|e| e.into_inner());
        let silent = !s.config.notify_muted;
        let mut out = Vec::new();
        for (id, name) in targets {
            if s.mutes.get(id).is_some() {
                out.push(s.tr(lang, "pv.command.mute.already_muted", &[&name]));
                continue;
            }
            let reason = s.mute_reason(lang, parsed.reason.as_deref());
            let feedback = match parsed.duration {
                MuteDuration::Permanent => s.tr(lang, "pv.command.mute.permanently_muted", &[&name, &reason]),
                MuteDuration::Time { amount, unit } => {
                    let d = s.duration_text(lang, amount, unit, now);
                    s.tr(lang, "pv.command.mute.temporarily_muted", &[&name, &d, &reason])
                }
            };
            s.mute(id, sender.id(), parsed.duration, parsed.reason.clone(), silent);
            log::info!(target: crate::LOG, "[Plasmo Voice] {feedback}");
            out.push(feedback);
        }
        out
    }

    /// `/vunmute <targets>`: a selector, a UUID, or a name the server knows (`UnmuteTargetsType`).
    fn unmute_command(&mut self, sender: &Sender, args: &str, lang: Option<&str>) -> Vec<String> {
        let target = args.split(' ').find(|w| !w.is_empty()).unwrap_or("");
        if target.is_empty() {
            let usage = self.shared.lock().unwrap_or_else(|e| e.into_inner()).langs.raw(lang, "pv.command.unmute.usage", "Usage: /vunmute <player>");
            return vec![usage];
        }
        let profiles: Vec<(Uuid, String)> = if target.starts_with('@') {
            self.resolve_online(sender, target)
        } else {
            let s = self.shared.lock().unwrap_or_else(|e| e.into_inner());
            match parse_uuid(target) {
                Some(id) => vec![(id, s.profiles.get(&id).cloned().unwrap_or_else(|| "unknown".into()))],
                None => {
                    let mut found: Vec<(Uuid, String)> = s.profiles.iter().filter(|(_, n)| n.eq_ignore_ascii_case(target)).map(|(id, n)| (*id, n.clone())).collect();
                    // The one on the server first, else the muted one.
                    found.sort_by_key(|(id, _)| (!s.players.contains_key(id), s.mutes.get(*id).is_none()));
                    found.truncate(1);
                    found
                }
            }
        };
        if profiles.is_empty() {
            return vec![self.tr(lang, "pv.error.player_not_found", &[])];
        }
        let mut s = self.shared.lock().unwrap_or_else(|e| e.into_inner());
        let silent = !s.config.notify_unmuted;
        let mut out = Vec::new();
        for (id, name) in profiles {
            let line = if s.unmute(id, silent).is_some() {
                s.tr(lang, "pv.command.unmute.unmuted", &[&name])
            } else {
                s.tr(lang, "pv.command.unmute.not_muted", &[&name])
            };
            log::info!(target: crate::LOG, "[Plasmo Voice] {line}");
            out.push(line);
        }
        out
    }

    /// `/vmutelist`.
    fn mute_list(&self, lang: Option<&str>) -> Vec<String> {
        let s = self.shared.lock().unwrap_or_else(|e| e.into_inner());
        let mut out = vec![s.tr(lang, "pv.command.mute_list.header", &[])];
        let mut mutes: Vec<_> = s.mutes.all().cloned().collect();
        if mutes.is_empty() {
            out.push(s.tr(lang, "pv.command.mute_list.empty", &[]));
            return out;
        }
        mutes.sort_by_key(|m| m.muted_at);
        for m in mutes {
            // `getGameProfile`: a player the server doesn't know is left out.
            let Some(name) = s.profiles.get(&m.player) else { continue };
            let expires = if m.muted_to > 0 {
                let date = crate::server_lang::format_date(&s.langs.raw(lang, "pv.command.mute_list.expiration_date", "yyyy.MM.dd"), m.muted_to);
                let time = crate::server_lang::format_date(&s.langs.raw(lang, "pv.command.mute_list.expiration_time", "HH:mm:ss"), m.muted_to);
                s.tr(lang, "pv.command.mute_list.expire_at", &[&date, &time])
            } else {
                s.tr(lang, "pv.command.mute_list.never_expires", &[])
            };
            let reason = s.mute_reason(lang, m.reason.as_deref());
            match m.muted_by.and_then(|by| s.profiles.get(&by)) {
                Some(by) => out.push(s.tr(lang, "pv.command.mute_list.entry_muted_by", &[name, by, &expires, &reason])),
                None => out.push(s.tr(lang, "pv.command.mute_list.entry", &[name, &expires, &reason])),
            }
        }
        out
    }

    /// `/vrc`: the player's voice connection dropped and asked for again.
    fn reconnect(&mut self, sender: &Sender, lang: Option<&str>) -> Vec<String> {
        let Sender::Player { id, .. } = sender else {
            return vec![self.tr(lang, "pv.error.player_only_command", &[])];
        };
        let mut s = self.shared.lock().unwrap_or_else(|e| e.into_inner());
        if !s.players.contains_key(id) {
            return vec![s.tr(lang, "pv.error.player_not_found", &[])];
        }
        let line = s.tr(lang, "pv.command.reconnect.message", &[]);
        s.disconnect(*id);
        s.request_info(*id);
        vec![line]
    }

    /// Completions for an argument the client asks the server for: `/vmute`'s duration
    /// (`MuteDurationType`) and `/vunmute`'s target (the muted players, else the selectors).
    /// `arguments` are the words after the command's name, the last the one being typed.
    pub fn complete(&self, label: &str, arguments: &[String]) -> Option<Vec<String>> {
        let typing = arguments.last().map_or("", String::as_str);
        match (label, arguments.len()) {
            ("vmute", 2) => Some(mutes::duration_suggestions(typing)),
            ("vunmute", 1) => {
                let s = self.shared.lock().unwrap_or_else(|e| e.into_inner());
                let lower = typing.to_lowercase();
                let mut names: Vec<String> = s
                    .mutes
                    .all()
                    .map(|m| s.profiles.get(&m.player).cloned().unwrap_or_else(|| m.player.to_string()))
                    .filter(|n| n.to_lowercase().starts_with(&lower))
                    .collect();
                names.sort();
                if names.is_empty() {
                    names = ["@a", "@p", "@r", "@s"].iter().filter(|x| x.starts_with(typing)).map(|x| (*x).to_owned()).collect();
                }
                Some(names)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::tests::*;
    use crate::tcp::Packet;
    use std::time::Duration;

    #[test]
    fn mute_arguments() {
        let now = 1_000_000;
        assert_eq!(parse_mute("", now), None);
        assert_eq!(parse_mute("Bob", now), Some(Ok(MuteArgs { target: "Bob".into(), duration: MuteDuration::Permanent, reason: None })));
        assert_eq!(
            parse_mute("Bob 15m spamming in  chat ", now),
            Some(Ok(MuteArgs {
                target: "Bob".into(),
                duration: MuteDuration::Time { amount: 15, unit: mutes::DurationUnit::Minute },
                reason: Some("spamming in  chat ".into())
            }))
        );
        assert_eq!(parse_mute("Bob permanent   ", now), Some(Ok(MuteArgs { target: "Bob".into(), duration: MuteDuration::Permanent, reason: None })));
        assert_eq!(parse_mute("Bob spamming", now), Some(Err(DurationError::Invalid)));
        assert_eq!(parse_mute("Bob 5u", now), Some(Err(DurationError::InPast)));
        assert_eq!(Server::command_names(), vec!["vlist", "vmute", "vunmute", "vmutelist", "vreload", "vrc"]);
    }

    #[test]
    fn mute_list_unmute_and_reconnect() {
        let mut server = local();
        let console = Sender::Console;
        let a = connect(&mut server, Uuid(1), "Alice");
        let b = connect(&mut server, Uuid(2), "Bob");
        server.join(Uuid(3), "Carol");
        server.update(&[(a.id, at("overworld", 0.0)), (b.id, at("overworld", 1.0))]);
        server.take_tcp();
        let alice = Sender::Player { id: a.id, name: "Alice".into() };

        assert_eq!(server.command(&console, "vlist").unwrap(), vec!["Clients (2/3): Alice, Bob"]);
        assert_eq!(server.command(&console, "vmute Nobody").unwrap(), vec!["Player not found"]);
        assert_eq!(server.command(&console, "vmute bob 1h flood").unwrap(), vec!["Muted Bob for 1 h. Reason: flood"]);
        assert_eq!(server.command(&console, "vmute Bob").unwrap(), vec!["Bob already muted"]);
        assert!(server.muted(b.id));
        // Everyone with voice chat is told Bob is muted, and Bob is told why.
        let to_a = tcp_to(&mut server, a.id);
        assert!(to_a.iter().any(|p| matches!(p, Packet::PlayerInfoUpdate { player } if player.id == b.id && player.muted)));
        assert_eq!(server.take_tells(), vec![(b.id, "You've been muted for 1 h. Reason: flood".to_owned())]);
        // Bob can't talk.
        a.sock.set_read_timeout(Some(Duration::from_millis(200))).unwrap();
        speak(&b, 1);
        assert!(hears(&a).is_none(), "a muted player was heard");

        let list = server.command(&alice, "vmutelist").unwrap();
        assert_eq!(list[0], "Muted players:");
        assert!(list[1].starts_with("Bob, expires: ") && list[1].ends_with(". Reason: flood"), "{list:?}");
        assert_eq!(server.complete("vunmute", &["b".into()]), Some(vec!["Bob".to_owned()]));
        assert_eq!(server.complete("vmute", &["Bob".into(), "1".into()]), Some(vec!["1s", "1m", "1h", "1d", "1w"].into_iter().map(String::from).collect()));

        assert_eq!(server.command(&alice, "vunmute BOB").unwrap(), vec!["Unmuted Bob"]);
        assert_eq!(server.command(&alice, "vunmute Bob").unwrap(), vec!["Bob not muted"]);
        assert_eq!(server.take_tells(), vec![(b.id, "You've been unmuted".to_owned())]);
        speak(&b, 2);
        assert!(hears(&a).is_some(), "unmuted, heard again");
        assert_eq!(server.command(&console, "vmutelist").unwrap(), vec!["Muted players:", "No players are muted"]);

        // A permanent mute muted by a player, then Russian for a Russian client.
        server.set_player(b.id, Some("ru_ru".into()), crate::server::Perms::default());
        assert_eq!(server.command(&alice, "vmute @a").unwrap(), vec!["Alice is permanently muted. Reason: not specified", "Bob is permanently muted. Reason: not specified", "Carol is permanently muted. Reason: not specified"]);
        let tells = server.take_tells();
        assert!(tells.contains(&(b.id, "Вы были замучены навсегда. Причина: не указана".to_owned())), "{tells:?}");
        let bob = Sender::Player { id: b.id, name: "Bob".into() };
        assert_eq!(server.command(&bob, "vmutelist").unwrap()[1], "Alice замучен игроком Alice, до: никогда. Причина: не указана");

        // `/vrc`: from the console refused; for a player, the connection asked for again.
        assert_eq!(server.command(&console, "vrc").unwrap(), vec!["Only player can execute this command"]);
        server.take_tcp();
        assert_eq!(server.command(&alice, "vrc").unwrap(), vec!["Reconnect packet sent"]);
        let out = tcp_to(&mut server, a.id);
        assert!(matches!(out[..], [Packet::PlayerInfoRequest]), "{out:?}");
        assert!(server.command(&console, "gamemode creative").is_none());
    }

    #[test]
    fn mutes_survive_a_restart_and_reload_sends_distances() {
        let dir = std::env::temp_dir().join(format!("pv-cmd-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let start = || Server::start(crate::server::Options { folder: Some(dir.clone()), minecraft_port: 0 }, std::sync::Arc::new(Plain));
        let mut server = start();
        let a = connect(&mut server, Uuid(1), "Alice");
        assert_eq!(server.command(&Sender::Console, "vmute Alice 7d").unwrap(), vec!["Muted Alice for 7 d. Reason: not specified"]);
        drop(a);
        drop(server);
        let json = std::fs::read_to_string(dir.join("voice_mutes.json")).unwrap();
        assert!(json.contains("\"playerUUID\":\"00000000-0000-0000-0000-000000000001\""), "{json}");

        let mut server = start();
        let a = connect(&mut server, Uuid(1), "Alice");
        assert!(server.muted(a.id));
        // New distances in the file, then /vreload: the activation again, and a Config with them.
        let text = std::fs::read_to_string(dir.join("config.toml")).unwrap().replace("distances = [8, 16, 32]", "distances = [4, 48, 12]").replace("default_distance = 16", "default_distance = 12");
        std::fs::write(dir.join("config.toml"), text).unwrap();
        server.take_tcp();
        assert_eq!(server.command(&Sender::Console, "vreload").unwrap(), vec!["Config reloaded"]);
        let out = tcp_to(&mut server, a.id);
        assert!(matches!(&out[0], Packet::ActivationUnregister { .. }), "{out:?}");
        assert!(matches!(&out[1], Packet::SourceLineUnregister { .. }));
        assert!(matches!(&out[2], Packet::ActivationRegister(act) if act.distances == vec![4, 12, 48] && act.default_distance == 12));
        assert!(out.iter().any(|p| matches!(p, Packet::Config { activations, .. } if activations[0].distances == vec![4, 12, 48])));
        assert!(std::fs::read_to_string(dir.join("config.toml")).unwrap().contains("distances = [4, 12, 48]"), "written back sorted");
        drop(a);
        drop(server);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
