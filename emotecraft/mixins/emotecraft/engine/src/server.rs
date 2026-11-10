//! Emotecraft's server half: who plays which emote since when and whether it is forced
//! (`ServerNetworkInstance`), what a client's message does (`AbstractServerEmotePlay`: the
//! play-phase configuration and `presenceResponse`, streaming an emote to the players who
//! track the emoter, stopping it, a late tracker joining mid-emote), the server emotes sent to
//! each client, and the `/emotes` command (`ServerCommands`, `EmoteArgumentProvider`).
//!
//! The glue gives it the players and what they track, and sends what it queues.
//!
//! This Source Code Form is a port of Emotecraft (https://github.com/KosmX/emotes), GPL-3.0.

use crate::anim::Animation;
use crate::emotes::Loaded;
use crate::packet::{Bound, NetData, Task, Versions};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Instant;

/// Op levels (`PermissionLevel`).
pub const MODERATORS: u8 = 1;
pub const GAMEMASTERS: u8 = 2;
pub const ADMINS: u8 = 3;

struct Player {
    name: String,
    /// The versions its CONFIG gave; none until it configured (a client without the mod).
    versions: Option<Versions>,
    emote: Option<(Arc<Animation>, Instant, bool)>,
}

impl Player {
    /// `getPlayedEmote`: the emote and its tick, while it still plays.
    fn played(&mut self) -> Option<(Arc<Animation>, f32)> {
        let (a, start, _) = self.emote.as_ref()?;
        let tick = start.elapsed().as_millis() as f32 / 50.0;
        if !a.is_playing_at(tick) {
            self.emote = None;
            return None;
        }
        Some((a.clone(), tick))
    }
    fn forced(&mut self) -> bool {
        self.played().is_some() && self.emote.as_ref().is_some_and(|e| e.2)
    }
}

/// What the server half queues for the glue.
#[derive(Debug, PartialEq)]
pub enum Out {
    /// A message on `emotecraft:emote` to a player.
    Send(u128, Vec<u8>),
    /// A player to disconnect, with the reason.
    Kick(u128, String),
}

pub struct Server {
    pub loaded: Loaded,
    players: BTreeMap<u128, Player>,
    /// Tracked uuid → the players tracking it.
    tracked_by: BTreeMap<u128, BTreeSet<u128>>,
    out: Vec<Out>,
}

/// Who runs a command.
pub struct Sender {
    pub uuid: Option<u128>,
    pub level: u8,
    pub dedicated: bool,
}

/// A command's answer.
pub enum Reply {
    Ok,
    Error(String),
    /// `/emotes reload` asks the glue to load the emotes again.
    Reload,
    NotOurs,
}

impl Server {
    pub fn new(loaded: Loaded) -> Server {
        Server { loaded, players: BTreeMap::new(), tracked_by: BTreeMap::new(), out: Vec::new() }
    }

    pub fn take_out(&mut self) -> Vec<Out> {
        std::mem::take(&mut self.out)
    }

    pub fn join(&mut self, player: u128, name: &str) {
        self.players.entry(player).or_insert(Player { name: name.to_owned(), versions: None, emote: None });
    }

    pub fn quit(&mut self, player: u128) {
        self.players.remove(&player);
        self.tracked_by.remove(&player);
        for set in self.tracked_by.values_mut() {
            set.remove(&player);
        }
    }

    pub fn configured(&self, player: u128) -> bool {
        self.players.get(&player).is_some_and(|p| p.versions.is_some())
    }

    /// `sendMessage(data, true)`: the message in the player's versions.
    fn send(&mut self, to: u128, data: &NetData) {
        let Some(versions) = self.players.get(&to).and_then(|p| p.versions.clone()) else { return };
        let mut d = data.clone();
        d.versions = versions;
        match d.write(Bound::Client) {
            Ok(bytes) => self.out.push(Out::Send(to, bytes)),
            Err(e) => log::warn!("[Emotecraft] Failed to send packet to {}: {e}", crate::buf::uuid_string(to)),
        }
    }

    /// `sendForTrackedBy`: everyone tracking `player` who has the mod.
    fn send_tracked(&mut self, player: u128, data: &NetData) {
        let targets: Vec<u128> = self.tracked_by.get(&player).map(|s| s.iter().copied().filter(|t| *t != player).collect()).unwrap_or_default();
        for t in targets {
            self.send(t, data);
        }
    }

    /// A message on `emotecraft:stream`: `This server does not support streaming!`.
    pub fn stream_channel(&mut self, player: u128) {
        self.out.push(Out::Kick(player, "This server does not support streaming!".into()));
    }

    /// A player's message on `emotecraft:emote` (`receiveMessage`).
    pub fn receive(&mut self, player: u128, bytes: &[u8]) {
        let data = match NetData::read(bytes, Bound::Server, false) {
            Ok(d) => d,
            Err(e) => {
                log::warn!("[Emotecraft] Invalid message from {}: {e}", crate::buf::uuid_string(player));
                return;
            }
        };
        if !self.players.contains_key(&player) {
            self.join(player, "");
        }
        match data.purpose {
            Task::Stop => self.stop(player, true),
            Task::Config => {
                log::info!("[Emotecraft] The {} does not support the new configuration!", self.players.get(&player).map_or("player", |p| p.name.as_str()));
                if let Some(p) = self.players.get_mut(&player) {
                    p.versions = Some(data.versions.clone());
                }
                self.presence_response(player);
            }
            Task::Stream => {
                let forced = self.players.get_mut(&player).is_some_and(Player::forced);
                if forced {
                    log::warn!("[Emotecraft] Player {} is disobeying force play flag and tried to override it", crate::buf::uuid_string(player));
                    return;
                }
                if let Some(e) = data.emote.clone() {
                    self.stream(player, e, data.tick, false, true);
                }
            }
            _ => log::warn!("[Emotecraft] Unknown packet task from {}", crate::buf::uuid_string(player)),
        }
    }

    /// `presenceResponse`: the server's configuration, then every server emote as a file.
    fn presence_response(&mut self, player: u128) {
        let Some(versions) = self.players.get(&player).and_then(|p| p.versions.clone()) else { return };
        let mut config = NetData::config(true);
        if let Ok(bytes) = config.write(Bound::Client) {
            self.out.push(Out::Send(player, bytes));
        }
        if versions.contains_key(&crate::packet::HEADER_PACKET) || !versions.is_empty() {
            for e in self.loaded.server.clone() {
                self.send(player, &NetData::file(e));
            }
        }
    }

    /// `streamEmote`.
    fn stream(&mut self, player: u128, emote: Arc<Animation>, tick: f32, forced: bool, from_player: bool) {
        if let Some(p) = self.players.get_mut(&player) {
            p.emote = Some((emote.clone(), Instant::now(), forced));
        }
        let mut data = NetData::stream(emote, Some(player), tick);
        data.forced = forced;
        data.strict_size_limit = false;
        self.send_tracked(player, &data);
        if !from_player {
            self.send(player, &data);
        }
    }

    /// `stopEmote`: `from_player` false is the server stopping it (a command), which tells the
    /// emoter too, forced so it shows no "not allowed" toast.
    fn stop(&mut self, player: u128, from_player: bool) {
        let played = self.players.get_mut(&player).and_then(Player::played);
        if let Some(p) = self.players.get_mut(&player) {
            p.emote = None;
        }
        if let Some((a, _)) = played {
            let mut data = NetData::stop(a.uuid, Some(player));
            self.send_tracked(player, &data);
            if !from_player {
                data.forced = true;
                self.send(player, &data);
            }
        }
    }

    /// `playerStartTracking`: the tracked player's running emote, from where it is now.
    pub fn start_tracking(&mut self, tracker: u128, tracked: u128) {
        self.tracked_by.entry(tracked).or_default().insert(tracker);
        let played = self.players.get_mut(&tracked).and_then(Player::played);
        if let Some((a, tick)) = played {
            self.send(tracker, &NetData::stream(a, Some(tracked), tick));
        }
    }

    pub fn stop_tracking(&mut self, tracker: u128, tracked: u128) {
        if let Some(s) = self.tracked_by.get_mut(&tracked) {
            s.remove(&tracker);
        }
    }

    /// Every player's id and whether it plays an emote (for the glue's own bookkeeping).
    pub fn playing(&mut self, player: u128) -> Option<(Arc<Animation>, f32)> {
        self.players.get_mut(&player).and_then(Player::played)
    }

    /// `/emotes reload`: the emotes loaded again; the clients drop the server emotes gone.
    pub fn reloaded(&mut self, loaded: Loaded) {
        let old: BTreeSet<u128> = self.loaded.server.iter().map(|a| a.uuid).collect();
        let now: BTreeSet<u128> = loaded.server.iter().map(|a| a.uuid).collect();
        self.loaded = loaded;
        let removed: Vec<u128> = old.difference(&now).copied().collect();
        if removed.is_empty() {
            return;
        }
        let data = NetData::remove(removed);
        let everyone: Vec<u128> = self.players.keys().copied().collect();
        for p in everyone {
            self.send(p, &data);
        }
    }

    /// The emotes a sender may name: the loaded ones with `emotes.play.showhidden`, else the
    /// server's.
    fn emotes_for(&self, level: u8) -> Vec<Arc<Animation>> {
        if level >= MODERATORS { self.loaded.all() } else { self.loaded.server.clone() }
    }

    /// The command's suggestions for the emote argument (`EmoteArgumentProvider.getSuggestions`).
    pub fn suggestions(&self, level: u8, name: &dyn Fn(&Animation) -> Option<String>) -> Vec<String> {
        self.emotes_for(level)
            .iter()
            .map(|a| match name(a) {
                Some(n) if n.contains(' ') => format!("\"{n}\""),
                Some(n) => n,
                None => crate::buf::uuid_string(a.uuid),
            })
            .collect()
    }

    /// `EmoteArgumentProvider.getEmote`.
    fn find(&self, level: u8, id: &str, name: &dyn Fn(&Animation) -> Option<String>) -> Result<Arc<Animation>, String> {
        let list = self.emotes_for(level);
        if let Some(u) = crate::buf::parse_uuid(id) {
            return list.into_iter().find(|a| a.uuid == u).ok_or_else(|| format!("No emote with ID: {}", crate::buf::uuid_string(u)));
        }
        list.into_iter().find(|a| name(a).as_deref() == Some(id)).ok_or_else(|| format!("Not emote with name: {id}"))
    }

    /// `/emotes ...` (`ServerCommands`). `avatar` finds a player by a name or uuid argument.
    pub fn command(&mut self, sender: &Sender, args: &[String], name: &dyn Fn(&Animation) -> Option<String>, avatar: &dyn Fn(&str) -> Option<u128>) -> Reply {
        let force_error = || Reply::Error("Can't stop forced emote without admin rights".into());
        match args.first().map(String::as_str) {
            Some("play") => {
                let Some(id) = args.get(1) else { return Reply::Error("Missing emote".into()) };
                let emote = match self.find(sender.level, id, name) {
                    Ok(e) => e,
                    Err(e) => return Reply::Error(e),
                };
                match args.get(2) {
                    None => {
                        let Some(me) = sender.uuid else { return Reply::Error("A player is required to run this command here".into()) };
                        let admin = sender.level >= GAMEMASTERS;
                        if !admin && self.players.get_mut(&me).is_some_and(Player::forced) {
                            return force_error();
                        }
                        self.server_play(me, Some(emote), false);
                        Reply::Ok
                    }
                    Some(who) => {
                        if sender.level < MODERATORS {
                            return Reply::Error("Incorrect argument for command".into());
                        }
                        let Some(target) = avatar(who) else { return Reply::Error("The specified entity must be an avatar!".into()) };
                        let forced = args.get(3).is_some_and(|f| f.eq_ignore_ascii_case("true"));
                        self.server_play(target, Some(emote), forced);
                        Reply::Ok
                    }
                }
            }
            Some("stop") => match args.get(1) {
                None => {
                    let Some(me) = sender.uuid else { return Reply::Error("A player is required to run this command here".into()) };
                    let admin = sender.level >= GAMEMASTERS;
                    if admin || !self.players.get_mut(&me).is_some_and(Player::forced) {
                        self.server_play(me, None, false);
                        Reply::Ok
                    } else {
                        force_error()
                    }
                }
                Some(who) => {
                    if sender.level < MODERATORS {
                        return Reply::Error("Incorrect argument for command".into());
                    }
                    let Some(target) = avatar(who) else { return Reply::Error("The specified entity must be an avatar!".into()) };
                    self.server_play(target, None, false);
                    Reply::Ok
                }
            },
            Some("reload") if sender.dedicated && sender.level >= ADMINS => Reply::Reload,
            _ => Reply::Error("Unknown or incomplete command".into()),
        }
    }

    /// `ServerEmoteAPI.playEmote` (`setPlayerPlayingEmoteImpl`).
    pub fn server_play(&mut self, player: u128, emote: Option<Arc<Animation>>, forced: bool) {
        if !self.players.contains_key(&player) {
            return;
        }
        match emote {
            Some(e) => self.stream(player, e, 0.0, forced, false),
            None => self.stop(player, false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client_config() -> Vec<u8> {
        NetData::config(false).write(Bound::Server).unwrap()
    }

    #[test]
    fn relays_to_trackers_and_late_joiners() {
        let a = Arc::new(crate::loader::load_json(crate::tests::BUILT_IN_JSON[13]).unwrap().remove(0));
        let mut s = Server::new(Loaded::default());
        s.join(1, "a");
        s.join(2, "b");
        s.receive(1, &client_config());
        s.receive(2, &client_config());
        assert_eq!(s.take_out().len(), 2);
        s.start_tracking(2, 1);
        let mut d = NetData::stream(a.clone(), None, 0.0);
        s.receive(1, &d.write(Bound::Server).unwrap());
        let out = s.take_out();
        assert_eq!(out.len(), 1);
        let Out::Send(to, bytes) = &out[0] else { panic!() };
        assert_eq!(*to, 2);
        let got = NetData::read(bytes, Bound::Client, true).unwrap();
        assert_eq!(got.player, Some(1));
        assert_eq!(got.emote.unwrap().uuid, a.uuid);
        // A third player starts tracking: the emote comes mid-way.
        s.join(3, "c");
        s.receive(3, &client_config());
        s.take_out();
        s.start_tracking(3, 1);
        assert_eq!(s.take_out().len(), 1);
        // Stop from the player: the trackers hear it.
        let mut stop = NetData::stop(a.uuid, None);
        s.receive(1, &stop.write(Bound::Server).unwrap());
        let out = s.take_out();
        assert_eq!(out.len(), 2);
    }
}
