// Roughly Enough Items for Horizon: REI's config screen over the Options
// screen, as Java's Mod Menu opens it (`REIModMenuEntryPoint`) and as the
// mod's page's first button does. While it is open on REI's page it hides the
// options (`draw_options`), draws itself in `screen_render_post` and takes all
// of the screen's input; Done, Cancel and Escape go back to the page, or out of
// the options (to Mod Menu) when Mod Menu opened it. Also writes the icon and
// Mod Menu's metadata into the mod's data folder.
use super::roughlyenoughitems as rei;
use super::roughlyenoughitems_config::{self as config, ConfigScreen};
use super::roughlyenoughitems_glue as glue;
use super::roughlyenoughitems_view::argb;
use super::{Game, Screen};
use horizon_client::hud::HudBatch;
use horizon_client::mod_api::NativeScreenView;
use horizon_client::options_ui::Page;
use std::cell::Cell;

/// Mod Menu's Configure: `native_settings_click(id, MODMENU_CONFIGURE)` after it opened the mod's page.
pub const MODMENU_CONFIGURE: usize = 0x4D4D_4346;

/// Who opened the screen: the page's button, or Mod Menu's Configure.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Page,
    ModMenu,
}

thread_local! {
    static OPEN: Cell<Option<Mode>> = const { Cell::new(None) };
    /// Mouse buttons whose press the screen took, so their release is taken too.
    static TAKEN: Cell<u8> = const { Cell::new(0) };
    /// A container screen has been open since the game started
    /// (`REIRuntime.getPreviousContainerScreen`): with a world, a "level".
    static CONTAINER_SEEN: Cell<bool> = const { Cell::new(false) };
    /// The icon and the metadata were written this run.
    static WRITTEN: Cell<bool> = const { Cell::new(false) };
}

/// REI's page is the one the Options screen shows: its index in Options > Mods.
fn page(game: &Game<'_>) -> Option<usize> {
    if game.screen != Screen::Options {
        return None;
    }
    match game.ui.options.page() {
        Page::NativeMod(i) if game.settings.mods.installed.get(i).is_some_and(|m| m.id == rei::ID) => Some(i),
        _ => None,
    }
}

/// The config screen is up over REI's page: the page stays hidden and the
/// screen's input is the config screen's.
pub(super) fn covers(game: &Game<'_>) -> bool {
    OPEN.with(Cell::get).is_some() && page(game).is_some()
}

/// Opens the config screen over REI's page (`settings_click`): button 0 of
/// the page, or Mod Menu's Configure. It opens with REI switched off too, or
/// while Just Enough Items keeps REI's overlay away: they are REI's settings.
pub(super) fn open(game: &mut Game<'_>, from_mod_menu: bool) {
    if page(game).is_none() {
        return;
    }
    let in_world = game.title.is_none() && game.player.is_some();
    let level = in_world && CONTAINER_SEEN.with(Cell::get);
    let ru = rei::russian(&horizon_client::lang::code());
    let screen = ConfigScreen::over_options(game, in_world, level);
    let opened = glue::with(|s| {
        s.ru = ru;
        s.config = Some(screen);
    });
    if opened.is_some() {
        OPEN.with(|o| o.set(Some(if from_mod_menu { Mode::ModMenu } else { Mode::Page })));
        TAKEN.with(|t| t.set(0));
    }
}

fn shown_on_options() -> bool {
    glue::with(|s| s.config.as_ref().is_some_and(ConfigScreen::on_options)).unwrap_or(true)
}

/// Once a frame (after `glue::frame`): the data files, what the screen asked
/// for, and the screen's closing.
pub(super) fn frame(game: &mut Game<'_>) {
    if !WRITTEN.with(Cell::get) {
        WRITTEN.with(|w| w.set(true));
        write_data(game);
    }
    if matches!(game.screen, Screen::Inventory | Screen::Creative) && game.title.is_none() {
        CONTAINER_SEEN.with(|c| c.set(true));
    }
    // Keybinds kept by Done, a paste: `glue::frame` skips them while REI is off.
    glue::run_pending(game);
    let Some(mode) = OPEN.with(Cell::get) else {
        return;
    };
    match page(game) {
        // The options went elsewhere some other way: the screen goes, unsaved.
        None => {
            OPEN.with(|o| o.set(None));
            glue::with(|s| {
                if s.config.as_ref().is_some_and(ConfigScreen::on_options) {
                    s.config = None;
                }
            });
        }
        // Done, Cancel or Escape closed it.
        Some(i) if !shown_on_options() => {
            OPEN.with(|o| o.set(None));
            match mode {
                // REI's page again, its labels following the new values.
                Mode::Page => game.native_settings_refresh(i),
                // Out of the options the way Escape goes: back to Mod Menu.
                Mode::ModMenu => {
                    let events = game.ui.options.back(&mut game.settings);
                    let _ = game.handle_option_events(events);
                }
            }
        }
        Some(_) => {}
    }
}

fn over(game: &Game<'_>, screen: &NativeScreenView<'_>) -> bool {
    screen.id == "options" && covers(game)
}

/// Draws the screen over the Options screen (`screen_render_post`), its own
/// background first, then its tooltip.
pub(super) fn render(game: &Game<'_>, batch: &mut HudBatch, screen: &NativeScreenView<'_>) {
    if !over(game, screen) {
        return;
    }
    let tip = glue::with(|s| if s.config.as_ref().is_some_and(ConfigScreen::on_options) { config::draw(game, batch, s) } else { None }).flatten();
    if let Some(lines) = tip
        && !lines.is_empty()
    {
        let lines: Vec<(String, u32)> = lines.into_iter().map(|(l, c)| (l, argb(c))).collect();
        game.draw_tooltip_lines(batch, &lines, game.cursor.0, game.cursor.1);
    }
}

fn bit(button: i32) -> u8 {
    1 << button.clamp(0, 7)
}

/// A mouse button pressed over the screen: true when it took it.
pub(super) fn pressed(game: &Game<'_>, screen: &NativeScreenView<'_>, button: i32) -> bool {
    if !over(game, screen) {
        return false;
    }
    TAKEN.with(|t| t.set(t.get() | bit(button)));
    let p = (screen.mouse[0], screen.mouse[1]);
    glue::with(|s| config::press(game, s, p, button));
    true
}

/// A mouse button let go: taken when its press was.
pub(super) fn released(game: &Game<'_>, screen: &NativeScreenView<'_>, button: i32) -> bool {
    let taken = TAKEN.with(|t| {
        let was = t.get() & bit(button) != 0;
        t.set(t.get() & !bit(button));
        was
    });
    if !over(game, screen) {
        return taken;
    }
    glue::with(config::release);
    true
}

/// The pointer dragged: nothing under the screen hears it.
pub(super) fn dragged(game: &Game<'_>, screen: &NativeScreenView<'_>) -> bool {
    over(game, screen)
}

/// The wheel: the panel under the pointer, or the open menu.
pub(super) fn scrolled(game: &Game<'_>, screen: &NativeScreenView<'_>, scroll: [f64; 2]) -> bool {
    if !over(game, screen) {
        return false;
    }
    let delta = if scroll[1] > 0.0 {
        -1
    } else if scroll[1] < 0.0 {
        1
    } else {
        return true;
    };
    let p = (screen.mouse[0], screen.mouse[1]);
    glue::with(|s| config::scroll(s, p, delta));
    true
}

/// A key: the screen takes every one.
pub(super) fn key_pressed(game: &Game<'_>, screen: &NativeScreenView<'_>, key: &str, modifiers: u32) -> bool {
    if !over(game, screen) {
        return false;
    }
    glue::with(|s| config::key(s, key, modifiers));
    true
}

/// A key let go: nothing under the screen hears it.
pub(super) fn key_released(game: &Game<'_>, screen: &NativeScreenView<'_>) -> bool {
    over(game, screen)
}

/// A character: the options search field's when it has focus.
pub(super) fn char_typed(game: &Game<'_>, screen: &NativeScreenView<'_>, c: char) -> bool {
    if !over(game, screen) {
        return false;
    }
    glue::with(|s| config::char_typed(s, c));
    true
}

// ---- the icon and Mod Menu's metadata ---------------------------------------------------

/// REI's icon (its `fabric.mod.json`'s `icon.png`).
const ICON: &[u8] = horizon_client::native_data!("roughlyenoughitems", "icon.png");

/// What Mod Menu shows of the mod, from REI's `fabric.mod.json` (its
/// environment is "*", so no Client badge).
fn modmenu_json() -> String {
    format!(
        r#"{{
  "schema": 1,
  "name": "{name}",
  "version": "{version}",
  "description": "Clean and Customizable.",
  "authors": ["shedaniel"],
  "contributors": ["DeportedToHell"],
  "license": "MIT",
  "links": {{
    "homepage": "https://www.curseforge.com/minecraft/mc-mods/roughly-enough-items",
    "sources": "https://github.com/DeportedToHell/MCRE-Mods/tree/main/roughlyenoughitems",
    "issues": "https://github.com/DeportedToHell/MCRE-Mods/issues"
  }},
  "badges": [],
  "config_screen": true
}}
"#,
        name = rei::NAME,
        version = rei::VERSION,
    )
}

/// `icon.png` and `modmenu.json` in the mod's data folder, each written only
/// when it differs from what is there.
fn write_data(game: &Game<'_>) {
    let dir = game.native_data_dir(rei::ID);
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let meta = modmenu_json();
    for (name, bytes) in [("icon.png", ICON), ("modmenu.json", meta.as_bytes())] {
        let path = dir.join(name);
        if std::fs::read(&path).ok().as_deref() != Some(bytes) {
            let _ = std::fs::write(&path, bytes);
        }
    }
}
