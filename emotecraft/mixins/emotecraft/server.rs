// Emotecraft's server half in the game (`crate::game::emotecraft_server`): single player's server
// thread, a LAN host and a dedicated server (`horizon-engine`, no window) run what Emotecraft's
// Fabric mod and Paper plugin run (`McServerEmotePlay`, `ServerCommands`): the server emotes, the
// play-phase configuration, the relay to the players who track the emoter, `/emotes`. The host's
// own player, who has no connection to it, talks to it through `BRIDGE`.
//
// This Source Code Form is a port of Emotecraft (https://github.com/KosmX/emotes), GPL-3.0.

use super::Game;
use super::emotecraft_glue::{self as glue, BRIDGE, ID, NAME, lock};
use emotecraft_engine::config::Config;
use emotecraft_engine::emotes::{self, Loaded};
use emotecraft_engine::packet;
use emotecraft_engine::server::{self, Out, Reply, Sender, Server};
use horizon_client::mod_api::{NativeCommand, NativeServerPlayer, ServerTickPhase, Suggestion, WorldPhase};
use horizon_client::text::Text;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;

struct ServerSide {
    server: Server,
    dedicated: bool,
    joined: HashSet<u128>,
    host: Option<u128>,
    /// What loads the emotes again (`/emotes reload`).
    emotes_dir: PathBuf,
    load_built_in: bool,
    folder_on_server: bool,
}

static SERVER: Mutex<Option<ServerSide>> = Mutex::new(None);

fn dedicated(game: &Game<'_>) -> Option<PathBuf> {
    (game.options.execution_role == super::execution_role::ExecutionRole::DedicatedServer).then(|| game.native_server_dir()).flatten()
}

/// The server's config (`config/emotecraft.json`, `CommonConfig`): a dedicated server's own,
/// read and written back; single player's is the client's.
fn server_config() -> Config {
    let path = Game::native_config_dir().ok().map(|d| d.join("emotecraft.json"));
    let config = match path.as_ref().map(std::fs::read_to_string) {
        Some(Ok(text)) => Config::parse(&text, false).unwrap_or_else(|_| Config::new(false)),
        _ => Config::new(false),
    };
    if let Some(p) = path {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let text = config.write();
        if std::fs::read_to_string(&p).ok().as_deref() != Some(text.as_str()) {
            let _ = std::fs::write(&p, text);
        }
    }
    config
}

fn load(side_dir: &std::path::Path, built_in: bool, folder_on_server: bool) -> Loaded {
    emotes::load(&glue::built_ins(), built_in, side_dir, folder_on_server)
}

/// Hook `world_lifecycle`: the server half runs while the world does.
pub(super) fn world(game: &mut Game<'_>, phase: WorldPhase) {
    if game.window.is_some() {
        return;
    }
    match phase {
        WorldPhase::WorldOpenReady => {
            if !game.native_on(ID) {
                return;
            }
            let dedicated_dir = dedicated(game);
            let (dir, built_in, folder_on_server) = match &dedicated_dir {
                Some(d) => {
                    let c = server_config();
                    (d.join(c.s("emotesDirectory")), c.b("loadbuiltin"), c.b("emotesFolderOnLogicalServer"))
                }
                None => glue::client(|c| (c.emotes_dir.clone(), c.config.b("loadbuiltin"), c.config.b("emotesFolderOnLogicalServer"))),
            };
            let loaded = load(&dir, built_in, folder_on_server);
            log::info!("[{NAME}] Server emotes: {} streamed, {} built in or hidden", loaded.server.len(), loaded.hidden.len());
            *lock(&SERVER) = Some(ServerSide {
                server: Server::new(loaded),
                dedicated: dedicated_dir.is_some(),
                joined: HashSet::new(),
                host: None,
                emotes_dir: dir,
                load_built_in: built_in,
                folder_on_server,
            });
        }
        WorldPhase::WorldCloseFinal | WorldPhase::WorldCloseFailed => {
            *lock(&SERVER) = None;
            let mut b = lock(&BRIDGE);
            b.to_client.clear();
            b.to_server.clear();
        }
        _ => {}
    }
}

/// Hook `player_logged_in`: the channels registered with the player (`minecraft:register`, as a
/// Paper server registers them), so its client configures in play.
pub(super) fn logged_in(game: &mut Game<'_>, player: u128, name: &str) {
    if game.window.is_some() || !game.native_on(ID) {
        return;
    }
    let mut guard = lock(&SERVER);
    let Some(side) = guard.as_mut() else { return };
    if side.host == Some(player) {
        return;
    }
    side.joined.insert(player);
    side.server.join(player, name);
    drop(guard);
    let channels = format!("{}\0{}", packet::CHANNEL, packet::STREAM_CHANNEL).into_bytes();
    let _ = game.native_send_custom_payload_to_player(player, "minecraft:register", &channels);
}

pub(super) fn logged_out(game: &mut Game<'_>, player: u128) {
    if game.window.is_some() {
        return;
    }
    let mut guard = lock(&SERVER);
    let Some(side) = guard.as_mut() else { return };
    if side.joined.remove(&player) {
        side.server.quit(player);
    }
}

/// A guest's plugin message (`native_custom_payload_from_client`).
pub(super) fn payload(game: &mut Game<'_>, player: u128, channel: &str, data: &[u8]) {
    if game.window.is_some() {
        return;
    }
    let mut guard = lock(&SERVER);
    let Some(side) = guard.as_mut() else { return };
    match channel {
        packet::CHANNEL => side.server.receive(player, data),
        packet::STREAM_CHANNEL => side.server.stream_channel(player),
        _ => {}
    }
}

/// Hooks `player_start_tracking` / `player_stop_tracking`: who sees whom.
pub(super) fn tracking(game: &mut Game<'_>, player: u128, uuid: u128, kind: &str, start: bool) {
    if game.window.is_some() || !matches!(kind, "minecraft:player" | "minecraft:mannequin") || uuid == 0 {
        return;
    }
    let mut guard = lock(&SERVER);
    let Some(side) = guard.as_mut() else { return };
    if start {
        side.server.start_tracking(player, uuid);
    } else {
        side.server.stop_tracking(player, uuid);
    }
}

/// Hook `server_tick`, at its end: the host's messages in, everything queued out.
pub(super) fn server_tick(game: &mut Game<'_>, phase: ServerTickPhase) {
    if game.window.is_some() || !matches!(phase, ServerTickPhase::ServerTickEnd { .. }) {
        return;
    }
    let mut guard = lock(&SERVER);
    let Some(side) = guard.as_mut() else { return };
    let (host, from_host) = {
        let mut b = lock(&BRIDGE);
        (b.host.clone(), std::mem::take(&mut b.to_server))
    };
    match (&host, side.host) {
        (Some((uuid, name)), None) => {
            side.host = Some(*uuid);
            side.joined.remove(uuid);
            side.server.join(*uuid, name);
        }
        (None, Some(uuid)) => {
            side.host = None;
            side.server.quit(uuid);
        }
        _ => {}
    }
    if let Some(uuid) = side.host {
        for data in from_host {
            side.server.receive(uuid, &data);
        }
    }
    let host = side.host;
    let out = side.server.take_out();
    drop(guard);
    for o in out {
        match o {
            Out::Send(to, data) => {
                if Some(to) == host {
                    lock(&BRIDGE).to_client.push(data);
                } else if let Err(e) = game.native_send_custom_payload_to_player(to, packet::CHANNEL, &data) {
                    log::debug!("[{NAME}] can't send: {e}");
                }
            }
            Out::Kick(to, reason) => {
                game.native_kick(to, &Text::literal(reason));
            }
        }
    }
}

/// Hook `register_commands`: `/emotes play|stop|reload` (`ServerCommands`), each usage at the
/// level its permission defaults to.
pub(super) fn register_commands(game: &mut Game<'_>, commands: &mut Vec<NativeCommand>) {
    if game.window.is_some() || !game.native_on(ID) {
        return;
    }
    commands.push(NativeCommand::new("emotes play <emote:text?>"));
    commands.push(NativeCommand::new("emotes stop"));
    commands.push(NativeCommand::op("emotes stop <avatar:player>", server::MODERATORS));
    if dedicated(game).is_some() {
        commands.push(NativeCommand::op("emotes reload", server::ADMINS));
    }
}

fn ours(command: &str) -> Option<&str> {
    let line = command.trim_start_matches('/');
    let rest = line.strip_prefix("emotes")?;
    (rest.is_empty() || rest.starts_with(' ')).then(|| rest.trim())
}

/// Runs `/emotes …` for a sender.
fn run(game: &mut Game<'_>, sender: &Sender, command: &str) -> bool {
    let Some(rest) = ours(command) else { return false };
    let args = glue::split_args(rest);
    let players = game.native_server_players();
    let avatar = |who: &str| -> Option<u128> {
        if let Some(u) = emotecraft_engine::buf::parse_uuid(who) {
            return players.iter().find(|p| p.uuid == u).map(|p| p.uuid);
        }
        if who == "@s" {
            return sender.uuid;
        }
        players.iter().find(|p| p.name.eq_ignore_ascii_case(who)).map(|p| p.uuid)
    };
    let reply = {
        let mut guard = lock(&SERVER);
        let Some(side) = guard.as_mut() else { return false };
        side.server.command(sender, &args, &glue::plain_name, &avatar)
    };
    match reply {
        Reply::Ok | Reply::NotOurs => {}
        Reply::Error(e) => game.native_feedback(Text::literal(e).with_color(0xFF5555)),
        Reply::Reload => {
            let (dir, built_in, on_server) = {
                let g = lock(&SERVER);
                let Some(s) = g.as_ref() else { return true };
                (s.emotes_dir.clone(), s.load_built_in, s.folder_on_server)
            };
            let loaded = load(&dir, built_in, on_server);
            if let Some(s) = lock(&SERVER).as_mut() {
                s.server.reloaded(loaded);
            }
        }
    }
    true
}

/// Hook `perform_command`: a player's `/emotes`.
pub(super) fn command(game: &mut Game<'_>, player: &NativeServerPlayer, command: &str, ran: &mut bool) {
    if game.window.is_some() {
        return;
    }
    let dedicated = lock(&SERVER).as_ref().is_some_and(|s| s.dedicated);
    let sender = Sender { uuid: Some(player.uuid), level: player.permission, dedicated };
    if run(game, &sender, command) {
        *ran = true;
    }
}

/// Hook `perform_console_command`: the dedicated server's console (level 4, no player).
pub(super) fn console_command(game: &mut Game<'_>, command: &str, ran: &mut bool) {
    if game.window.is_some() {
        return;
    }
    let dedicated = lock(&SERVER).as_ref().is_some_and(|s| s.dedicated);
    if run(game, &Sender { uuid: None, level: 4, dedicated }, command) {
        *ran = true;
    }
}

/// `native_tab_complete_v2`: the emotes for `/emotes play` (`EmoteArgumentProvider`; the hidden
/// built-in ones only with `emotes.play.showhidden`, level 1).
pub(super) fn complete(game: &mut Game<'_>, player: u128, label: &str, arguments: &[String], range: [usize; 2], suggestions: &mut Vec<Suggestion>) {
    if game.window.is_some() || ours(label).is_none() || arguments.first().map(String::as_str) != Some("play") || arguments.len() != 2 {
        return;
    }
    let level = game.native_server_players().into_iter().find(|p| p.uuid == player).map_or(0, |p| p.permission);
    let guard = lock(&SERVER);
    let Some(side) = guard.as_ref() else { return };
    let typed = arguments[1].to_lowercase();
    let list = side.server.suggestions(level, &glue::plain_name);
    suggestions.clear();
    suggestions.extend(
        list.into_iter()
            .filter(|s| s.to_lowercase().trim_start_matches('"').starts_with(typed.trim_start_matches('"')))
            .map(|text| Suggestion { range: (range[0], range[1]), text, tooltip: None, int_value: None }),
    );
}
