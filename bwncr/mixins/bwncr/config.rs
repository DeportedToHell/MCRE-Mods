// Bad Wither No Cookie's config screen: the one AutoConfig builds for
// `BWNCR_Config` (`AutoConfig.getConfigScreen`, which Mod Menu opens through
// `BWNCRModMenuIntegration`): Cloth Config's screen with the `general`
// object as a collapsed sub-category. It opens from Mod Menu's Configure
// (`MODMENU_CONFIGURE`) and from Configure... on the mod's page in Options >
// Mods, covers the options while it is up, and Save & Quit writes
// `config/bwncr.toml` as AutoConfig does.
use super::bwncr::{self as b, Config};
use super::bwncr_cloth::{Entry, Request, Screen as Cloth, Value};
use super::bwncr_glue as glue;
use super::{Game, Screen};
use horizon_client::clipboard::Clipboard as _;
use horizon_client::hud::HudBatch;
use horizon_client::mod_api::NativeScreenView;
use horizon_client::options_ui::Page;
use std::cell::RefCell;

/// Mod Menu's Configure: `native_settings_click(id, MODMENU_CONFIGURE)` after
/// it opened the mod's page.
pub(super) const MODMENU_CONFIGURE: usize = 0x4D4D_4346;

/// The original's icon (`common/src/main/resources/logo.png`), for Mod Menu.
const ICON: &[u8] = horizon_client::native_data!("bwncr", "icon.png");

struct State {
    screen: Option<Cloth>,
    /// Opened by Mod Menu: closing leaves the options too.
    modmenu: bool,
    /// The mod's row in Options > Mods.
    row: usize,
    data_written: bool,
}

thread_local! {
    static STATE: RefCell<State> = const {
        RefCell::new(State { screen: None, modmenu: false, row: 0, data_written: false })
    };
}

fn with<R>(f: impl FnOnce(&mut State) -> R) -> Option<R> {
    STATE.with(|s| s.try_borrow_mut().ok().map(|mut s| f(&mut s)))
}

/// Mod Menu's `modmenu.configure`, the page's first button.
pub(super) fn configure_label() -> &'static str {
    if glue::russian() { "Настройки..." } else { "Configure..." }
}

/// AutoConfig's entries for `BWNCR_Config`: `@CollapsibleObject General`
/// (closed, `startExpanded` is false) with its fields in their order.
fn build(c: &Config, ru: bool, in_world: bool) -> Cloth {
    let d = Config::default();
    let general = vec![
        Entry::boolean("silenceWither", b::tr(ru, "silenceWither"), c.silence_wither, d.silence_wither),
        Entry::boolean("silenceTrader", b::tr(ru, "silenceTrader"), c.silence_trader, d.silence_trader),
        Entry::boolean("silenceDragon", b::tr(ru, "silenceDragon"), c.silence_dragon, d.silence_dragon),
        Entry::boolean("silenceLightning", b::tr(ru, "silenceLightning"), c.silence_lightning, d.silence_lightning),
        Entry::list("silenceUs", b::tr(ru, "silenceUs"), c.silence_us.clone(), d.silence_us.clone()),
        Entry::boolean("debugMode", b::tr(ru, "debugMode"), c.debug_mode, d.debug_mode),
    ];
    Cloth::new(b::tr(ru, "title"), vec![Entry::sub(b::tr(ru, "general"), general, false)], in_world)
}

/// Save & Quit: the screen's values into the config, kept on disk.
fn save(values: Vec<(&'static str, Value)>) {
    let mut c = glue::config();
    for (id, v) in values {
        match id {
            "silenceWither" => c.silence_wither = v.as_bool(),
            "silenceTrader" => c.silence_trader = v.as_bool(),
            "silenceDragon" => c.silence_dragon = v.as_bool(),
            "silenceLightning" => c.silence_lightning = v.as_bool(),
            "silenceUs" => c.silence_us = v.as_list(),
            "debugMode" => c.debug_mode = v.as_bool(),
            _ => {}
        }
    }
    glue::set_config(c);
}

// ---- opening, covering, closing -----------------------------------------------------------

/// The mod's row in Options > Mods.
fn row(game: &Game<'_>) -> Option<usize> {
    game.settings.mods.installed.iter().position(|m| m.id == b::ID && m.native.is_some())
}

/// Opens the screen over the mod's page: `modmenu` when Mod Menu's Configure
/// asked.
pub(super) fn open(game: &Game<'_>, modmenu: bool) {
    let Some(row) = row(game) else {
        return;
    };
    let screen = build(&glue::config(), glue::russian(), game.ui.options.in_world);
    with(|s| {
        s.screen = Some(screen);
        s.modmenu = modmenu;
        s.row = row;
    });
}

/// The screen is up and the options show the mod's page under it.
pub(super) fn covers(game: &Game<'_>) -> bool {
    let (open, row) = with(|s| (s.screen.is_some(), s.row)).unwrap_or((false, 0));
    open && game.screen == Screen::Options
        && game.ui.options.page() == Page::NativeMod(row)
        && game.settings.mods.installed.get(row).is_some_and(|m| m.id == b::ID)
}

fn close(game: &mut Game<'_>) {
    let Some((modmenu, row)) = with(|s| {
        s.screen = None;
        (s.modmenu, s.row)
    }) else {
        return;
    };
    if modmenu {
        // As Escape on the page Mod Menu opened alone: back to Mod Menu.
        let events = game.ui.options.back(&mut game.settings);
        let _ = game.handle_option_events(events);
    } else {
        game.native_settings_refresh(row);
    }
}

/// Mod Menu's icon and metadata in the mod's folder, each written when it
/// differs from what is there.
fn write_data(game: &Game<'_>) {
    let dir = game.native_data_dir(b::ID);
    let _ = std::fs::create_dir_all(&dir);
    let json = format!(
        r#"{{
  "schema": 1,
  "name": "{}",
  "version": "{}",
  "description": "{}",
  "description_ru": "{}",
  "authors": ["Kreezxil, Eleksploded"],
  "contributors": ["droidicus (original)", "Mrbysco", "PrincessStellar (pt_br)", "Valentín Alarcón (es_ar)", "DispatchedToHeaven (Horizon port)"],
  "license": "MIT AND LGPL-3.0-only",
  "links": {{
    "homepage": "https://www.curseforge.com/minecraft/mc-mods/bad-wither-no-cookie-reloaded",
    "sources": "https://github.com/DeportedToHell/MCRE-Mods/tree/main/bwncr",
    "issues": "https://github.com/DeportedToHell/MCRE-Mods/issues"
  }},
  "badges": ["client"],
  "config_screen": true
}}
"#,
        b::NAME,
        b::VERSION,
        b::description(false),
        b::description(true),
    );
    for (name, bytes) in [("icon.png", ICON), ("modmenu.json", json.as_bytes())] {
        let path = dir.join(name);
        if std::fs::read(&path).ok().as_deref() != Some(bytes) {
            let _ = std::fs::write(&path, bytes);
        }
    }
}

/// Once a frame: the data files at first, what the screen asked for, and the
/// screen gone when its page is.
pub(super) fn frame(game: &mut Game<'_>) {
    if with(|s| !std::mem::replace(&mut s.data_written, true)).unwrap_or(false) {
        write_data(game);
    }
    if !with(|s| s.screen.is_some()).unwrap_or(false) {
        return;
    }
    if !covers(game) {
        // The page changed some other way: closed, nothing saved.
        with(|s| s.screen = None);
        return;
    }
    let requests = with(|s| s.screen.as_mut().map(Cloth::take_requests).unwrap_or_default()).unwrap_or_default();
    for r in requests {
        match r {
            Request::Click => game.audio.play_button_click(),
            Request::Paste => {
                let text = game.clipboard.get();
                with(|s| {
                    if let Some(screen) = s.screen.as_mut() {
                        screen.typed(&text);
                    }
                });
            }
            Request::Copy(text) => {
                if !text.is_empty() {
                    game.clipboard.set(&text);
                }
            }
            Request::Close(saved) => {
                if saved {
                    let values = with(|s| s.screen.as_ref().map(Cloth::values)).flatten().unwrap_or_default();
                    save(values);
                }
                close(game);
                return;
            }
        }
    }
}

// ---- the screen hooks -----------------------------------------------------------------------

/// The screen over the options, after they drew (nothing: `draw_options` is
/// held back while the screen covers them).
pub(super) fn render(game: &Game<'_>, batch: &mut HudBatch, screen: &NativeScreenView<'_>) {
    if screen.id != "options" || !covers(game) {
        return;
    }
    with(|s| {
        if let Some(c) = s.screen.as_mut() {
            c.draw(game, batch);
        }
    });
}

/// Runs `f` on the open screen when it covers `screen`: true when it did
/// (the event goes no further).
fn input(game: &Game<'_>, screen: &NativeScreenView<'_>, f: impl FnOnce(&mut Cloth, f32, f32, (f32, f32))) -> bool {
    if screen.id != "options" || !covers(game) {
        return false;
    }
    let (w, h) = (screen.rect[2].floor(), screen.rect[3].floor());
    let p = (screen.mouse[0], screen.mouse[1]);
    with(|s| {
        if let Some(c) = s.screen.as_mut() {
            f(c, w, h, p);
        }
    });
    true
}

pub(super) fn pressed(game: &Game<'_>, screen: &NativeScreenView<'_>, button: i32, modifiers: u32) -> bool {
    input(game, screen, |c, w, h, p| c.press(game, w, h, p, button, modifiers))
}

pub(super) fn released(game: &Game<'_>, screen: &NativeScreenView<'_>) -> bool {
    input(game, screen, |c, _, _, _| c.release())
}

pub(super) fn dragged(game: &Game<'_>, screen: &NativeScreenView<'_>) -> bool {
    input(game, screen, |c, w, h, p| c.dragged(w, h, p))
}

pub(super) fn scrolled(game: &Game<'_>, screen: &NativeScreenView<'_>, scroll: [f64; 2]) -> bool {
    input(game, screen, |c, w, h, p| c.scrolled(w, h, p, scroll[1] as f32))
}

pub(super) fn key_pressed(game: &Game<'_>, screen: &NativeScreenView<'_>, key: &str, modifiers: u32) -> bool {
    input(game, screen, |c, _, _, _| c.key_pressed(key, modifiers))
}

/// A key let go: the screen has no use for it, but it goes no further.
pub(super) fn key_released(game: &Game<'_>, screen: &NativeScreenView<'_>) -> bool {
    input(game, screen, |_, _, _, _| {})
}

pub(super) fn char_typed(game: &Game<'_>, screen: &NativeScreenView<'_>, c: char) -> bool {
    input(game, screen, |s, _, _, _| s.typed(&c.to_string()))
}
