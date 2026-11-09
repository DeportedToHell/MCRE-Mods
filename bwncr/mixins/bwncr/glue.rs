// Bad Wither No Cookie in the game: its row and page in Options > Mods, the
// config it keeps in `config/bwncr.toml` (loaded once, as AutoConfig loads it
// when the mod starts), the client command `/bwncr listen`
// (`FabricListenCommand`), and each frame the settings handed to the sound
// check (`bwncr_sounds`, beside the game's sound hook) and what it heard
// written out: the debug lines to the log, the names to the chat.
// Ported from Bad Wither No Cookie - Reloaded by Kreezxil and its
// contributors (MIT, https://gitlab.com/kreezxil/BadWitherNoCookie).
use super::Game;
use super::bwncr::{self as b, Config};
use super::bwncr_config as config;
use horizon_client::mod_api::NativeMod;
use horizon_client::sound_playback::bwncr_sounds as sounds;
use horizon_client::text::Text;
use std::cell::RefCell;

/// `Constants.LOGGER`'s name, before each line in the log (Horizon's log file
/// takes INFO lines from the game's own modules only, so they keep the
/// module's target).
const LOGGER: &str = "Bad Wither No Cookie Reloaded";

struct State {
    /// The config, once read.
    config: Option<Config>,
    /// `CommonClass.whatWasThat`: `/bwncr listen` is on.
    listen: bool,
}

thread_local! {
    static STATE: RefCell<State> = const { RefCell::new(State { config: None, listen: false }) };
}

fn with<R>(f: impl FnOnce(&mut State) -> R) -> Option<R> {
    STATE.with(|s| s.try_borrow_mut().ok().map(|mut s| f(&mut s)))
}

pub(super) fn russian() -> bool {
    horizon_client::lang::code().starts_with("ru")
}

fn config_path() -> Option<std::path::PathBuf> {
    Game::native_config_dir().ok().map(|d| d.join(b::CONFIG_FILE))
}

/// The config: read from `bwncr.toml` the first time (the defaults when there
/// is none, as `Toml4jConfigSerializer.deserialize` gives them).
pub(super) fn config() -> Config {
    if let Some(c) = with(|s| s.config.clone()).flatten() {
        return c;
    }
    let text = config_path().and_then(|p| std::fs::read_to_string(p).ok()).unwrap_or_default();
    let c = b::parse(&text);
    with(|s| s.config = Some(c.clone()));
    c
}

/// The config from now on, written to `bwncr.toml` (`ConfigManager.save`).
pub(super) fn set_config(c: Config) {
    if let Some(path) = config_path()
        && let Err(e) = std::fs::write(&path, b::write(&c))
    {
        log::error!("[{LOGGER}] Failed to save config '{}': {e}", path.display());
    }
    with(|s| s.config = Some(c));
}

// ---- Options > Mods -----------------------------------------------------------------------

/// The mod's row in Options > Mods.
pub(super) fn list(mods: &mut Vec<NativeMod>) {
    mods.push(NativeMod {
        id: b::ID.into(),
        title: format!("{} {}", b::NAME, b::VERSION),
        description: b::description(russian()).into(),
        keys: Vec::new(),
    });
}

/// The mod's page: Configure... first.
pub(super) fn page(mod_id: &str, labels: &mut Vec<String>) {
    if mod_id == b::ID {
        labels.insert(0, config::configure_label().into());
    }
}

/// Configure... (button 0) or Mod Menu's Configure opens the config screen.
pub(super) fn page_click(game: &mut Game<'_>, mod_id: &str, button: usize) {
    if mod_id != b::ID {
        return;
    }
    if button == config::MODMENU_CONFIGURE || button == 0 {
        config::open(game, button == config::MODMENU_CONFIGURE);
    }
}

// ---- /bwncr listen --------------------------------------------------------------------------

/// The command's name, so the chat box completes and colours it.
pub(super) fn commands(commands: &mut Vec<&'static str>) {
    commands.push(b::COMMAND);
}

/// `/bwncr listen`: the names of the sounds heard go to the chat, or stop.
pub(super) fn command(game: &mut Game<'_>, line: &str, ran: &mut bool) {
    let mut words = line.split_whitespace();
    if words.next() != Some(b::COMMAND) || words.next() != Some("listen") || words.next().is_some() {
        return;
    }
    if !game.native_on(b::ID) {
        return;
    }
    *ran = true;
    let on = with(|s| {
        s.listen = !s.listen;
        s.listen
    })
    .unwrap_or(false);
    game.native_chat(&format!("Event Listening is now {}", if on { "on" } else { "off" }));
}

// ---- each frame -----------------------------------------------------------------------------

/// Once a frame: the config screen's requests, the settings for the sound
/// check, then the lines and names it kept.
pub(super) fn frame(game: &mut Game<'_>) {
    config::frame(game);
    let c = config();
    let listen = with(|s| s.listen).unwrap_or(false);
    sounds::set(sounds::Filter {
        on: game.native_on(b::ID),
        wither: c.silence_wither,
        trader: c.silence_trader,
        dragon: c.silence_dragon,
        lightning: c.silence_lightning,
        sounds: c.silence_us,
        debug: c.debug_mode,
        listen,
    });
    let (lines, heard) = sounds::take();
    for line in lines {
        log::info!("[{LOGGER}] {line}");
    }
    for name in heard {
        // "Sound is <name>", aqua then red, to the player (the log without one).
        let message = format!("§bSound is §c{name}");
        if game.player.is_some() {
            game.system_message(Text::legacy(&message));
        } else {
            log::info!("[{LOGGER}] {message}");
        }
    }
}
