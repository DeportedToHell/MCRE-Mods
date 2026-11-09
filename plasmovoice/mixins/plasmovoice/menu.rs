// Plasmo Voice's Mod Menu screen (`ModMenuIntegration`, `createClothConfigMenu`): Cloth Config's
// screen "Plasmo Voice" with Devices (Javax Input Device) and Advanced (Check For Updates, Debug
// Logger, Camera Sound Listener, Jitter Buffer Delay), in Cloth Config's look (cloth.rs). Opened
// by Mod Menu's Configure (`MODMENU_CONFIGURE`) and by Configure... on the mod's page in Options >
// Mods; while it is up it covers the options. Save & Quit sets the fields and saves.
//
// This Source Code Form is a port of Plasmo Voice (https://github.com/plasmoapp/plasmo-voice), LGPL-3.0.

use super::plasmovoice_cloth::{Category, Entry, Request, Screen as Cloth};
use super::plasmovoice_config::V;
use super::plasmovoice_lang::t;
use super::{Game, Screen};
use horizon_client::clipboard::Clipboard as _;
use horizon_client::hud::HudBatch;
use horizon_client::mod_api::NativeScreenView;
use horizon_client::options_ui::Page;
use std::cell::RefCell;

/// Mod Menu's Configure: `native_settings_click(id, MODMENU_CONFIGURE)` after it opened the
/// mod's page.
pub(super) const MODMENU_CONFIGURE: usize = 0x4D4D_4346;

struct State {
    screen: Option<Cloth>,
    /// Opened by Mod Menu: closing leaves the options too.
    modmenu: bool,
    /// The mod's row in Options > Mods.
    row: usize,
}

thread_local! {
    static STATE: RefCell<State> = const { RefCell::new(State { screen: None, modmenu: false, row: 0 }) };
}

fn with<R>(f: impl FnOnce(&mut State) -> R) -> Option<R> {
    STATE.with(|s| s.try_borrow_mut().ok().map(|mut s| f(&mut s)))
}

/// `BooleanToggleBuilder.tooltip`: the text split at 180 pixels.
fn tip(game: &Game<'_>, key: &str) -> Vec<String> {
    let text = horizon_client::text::Text::literal(t(key));
    game.gui.atlas.font.split_lines(&text, 180.0).into_iter().map(|(l, _)| l.plain()).collect()
}

/// `createClothConfigMenu`.
fn build(game: &Game<'_>) -> Cloth {
    let (javax, updates, debug, camera, jitter) = super::plasmovoice_glue::config(|c| {
        (c.b("voice.use_javax_input"), c.b("check_for_updates"), c.b("debug"), c.b("advanced.camera_sound_listener"), c.i("advanced.jitter_packet_delay"))
    });
    let with_tip = |e: Entry, key: &str| {
        let lines = tip(game, key);
        e.tooltip(&lines.iter().map(String::as_str).collect::<Vec<_>>())
    };
    let devices = Category {
        name: t("clothconfig.plasmovoice.devices"),
        entries: vec![with_tip(
            Entry::boolean("use_javax_input", &t("clothconfig.plasmovoice.devices.use_javax_input"), javax, false),
            "clothconfig.plasmovoice.devices.use_javax_input.tooltip",
        )],
    };
    let advanced = Category {
        name: t("clothconfig.plasmovoice.advanced"),
        entries: vec![
            Entry::boolean("check_for_updates", &t("clothconfig.plasmovoice.advanced.check_for_updates"), updates, true),
            Entry::boolean("debug", &t("clothconfig.plasmovoice.advanced.debug_logger"), debug, false),
            with_tip(
                Entry::boolean("camera_sound_listener", &t("clothconfig.plasmovoice.advanced.camera_sound_listener"), camera, true),
                "clothconfig.plasmovoice.advanced.camera_sound_listener.tooltip",
            ),
            Entry::int("jitter_packet_delay", &t("clothconfig.plasmovoice.advanced.jitter_buffer_packet_delay"), jitter, 2),
        ],
    };
    Cloth::new("Plasmo Voice", vec![devices, advanced], game.ui.options.in_world)
}

/// Save & Quit: each entry's save consumer, then `config.save`.
fn save(values: Vec<(String, super::plasmovoice_cloth::Value)>) {
    super::plasmovoice_glue::config(|c| {
        for (id, v) in values {
            match id.as_str() {
                "use_javax_input" => {
                    c.set("voice.use_javax_input", V::B(v.as_bool()));
                    c.set("voice.input_backend", V::S(if v.as_bool() { "JAVAX" } else { "AUTO" }.into()));
                }
                "check_for_updates" => c.set("check_for_updates", V::B(v.as_bool())),
                "debug" => c.set("debug", V::B(v.as_bool())),
                "camera_sound_listener" => c.set("advanced.camera_sound_listener", V::B(v.as_bool())),
                "jitter_packet_delay" => c.set("advanced.jitter_packet_delay", V::I(v.as_int())),
                _ => {}
            }
        }
    });
    super::plasmovoice_glue::save_now();
}

// ---- opening, covering, closing -----------------------------------------------------------

fn row(game: &Game<'_>) -> Option<usize> {
    game.settings.mods.installed.iter().position(|m| m.id == super::plasmovoice_glue::ID && m.native.is_some())
}

pub(super) fn open(game: &Game<'_>, from_modmenu: bool) {
    let Some(row) = row(game) else { return };
    let screen = build(game);
    with(|s| {
        s.screen = Some(screen);
        s.modmenu = from_modmenu;
        s.row = row;
    });
}

/// The screen is up and the options show the mod's page under it.
pub(super) fn covers(game: &Game<'_>) -> bool {
    let (open, row) = with(|s| (s.screen.is_some(), s.row)).unwrap_or((false, 0));
    open && game.screen == Screen::Options
        && game.ui.options.page() == Page::NativeMod(row)
        && game.settings.mods.installed.get(row).is_some_and(|m| m.id == super::plasmovoice_glue::ID)
}

fn close(game: &mut Game<'_>) {
    let Some((modmenu, row)) = with(|s| {
        s.screen = None;
        (s.modmenu, s.row)
    }) else {
        return;
    };
    if modmenu {
        let events = game.ui.options.back(&mut game.settings);
        let _ = game.handle_option_events(events);
    } else {
        game.native_settings_refresh(row);
    }
}

/// Once a frame: what the screen asked for, and the screen gone when its page is.
pub(super) fn frame(game: &mut Game<'_>) {
    if !with(|s| s.screen.is_some()).unwrap_or(false) {
        return;
    }
    if !covers(game) {
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

pub(super) fn released(game: &Game<'_>, screen: &NativeScreenView<'_>, button: i32) -> bool {
    input(game, screen, |c, _, _, _| c.release(button))
}

pub(super) fn dragged(game: &Game<'_>, screen: &NativeScreenView<'_>) -> bool {
    input(game, screen, |c, w, h, p| c.dragged(game, w, h, p))
}

pub(super) fn scrolled(game: &Game<'_>, screen: &NativeScreenView<'_>, scroll: [f64; 2]) -> bool {
    input(game, screen, |c, w, h, p| c.scrolled(game, w, h, p, scroll[1] as f32))
}

pub(super) fn key_pressed(game: &Game<'_>, screen: &NativeScreenView<'_>, key: &str, modifiers: u32) -> bool {
    input(game, screen, |c, w, h, _| c.key_pressed(game, w, h, key, modifiers))
}

pub(super) fn key_released(game: &Game<'_>, screen: &NativeScreenView<'_>) -> bool {
    input(game, screen, |c, _, _, _| c.key_released())
}

pub(super) fn char_typed(game: &Game<'_>, screen: &NativeScreenView<'_>, c: char) -> bool {
    input(game, screen, |s, _, _, _| s.typed(&c.to_string()))
}
