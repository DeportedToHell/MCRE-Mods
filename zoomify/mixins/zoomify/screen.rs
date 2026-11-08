// Zoomify for Horizon: its config screen (YACL's, `zoomify_yacl`) over the
// Options screen, as Java's Mod Menu opens it (`ModMenuIntegration`), as the
// mod's page's first button does and as `/zoomify` does. While it is open on
// Zoomify's page it hides the options (`draw_options`), draws itself in
// `screen_render_post` and takes all of the screen's input; Done, Cancel and
// Escape go back to the page, or out of the options when Mod Menu or the
// command opened it. Also writes the icon and Mod Menu's metadata into the
// mod's data folder.
use super::zoomify as zf;
use super::zoomify_demo as demo;
use super::zoomify_yacl::{Action, KeyResult, Yacl};
use super::{Game, Screen};
use horizon_client::clipboard::Clipboard as _;
use horizon_client::hud::HudBatch;
use horizon_client::mod_api::NativeScreenView;
use horizon_client::options_ui::Page;
use std::cell::{Cell, RefCell};

/// Mod Menu's Configure: `native_settings_click(id, MODMENU_CONFIGURE)` after it opened the mod's page.
pub const MODMENU_CONFIGURE: usize = 0x4D4D_4346;

/// Who opened the screen: the page's button, or Mod Menu's Configure (and
/// `/zoomify`), which close it out of the options.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Page,
    ModMenu,
}

/// What a key asked of `frame`, which has the clipboard.
enum Clip {
    Paste,
    Copy(String),
}

thread_local! {
    static OPEN: Cell<Option<Mode>> = const { Cell::new(None) };
    static SCREEN: RefCell<Option<Yacl>> = const { RefCell::new(None) };
    static CLIP: RefCell<Vec<Clip>> = const { RefCell::new(Vec::new()) };
    /// Mouse buttons whose press the screen took, so their release is taken too.
    static TAKEN: Cell<u8> = const { Cell::new(0) };
    /// The icon and the metadata were written this run.
    static WRITTEN: Cell<bool> = const { Cell::new(false) };
}

fn with<R>(f: impl FnOnce(&mut Yacl) -> R) -> Option<R> {
    SCREEN.with(|s| s.borrow_mut().as_mut().map(f))
}

/// The page's first button (Mod Menu's own `modmenu.configure`).
pub(super) fn configure_label() -> &'static str {
    if zf::russian(&horizon_client::lang::code()) { "Настройки..." } else { "Configure..." }
}

/// Zoomify's page is the one the Options screen shows: its index in Options > Mods.
fn page(game: &Game<'_>) -> Option<usize> {
    if game.screen != Screen::Options {
        return None;
    }
    match game.ui.options.page() {
        Page::NativeMod(i) if game.settings.mods.installed.get(i).is_some_and(|m| m.id == zf::ID) => Some(i),
        _ => None,
    }
}

/// The config screen is up over Zoomify's page: the page stays hidden and
/// the screen's input is the config screen's.
pub(super) fn covers(game: &Game<'_>) -> bool {
    OPEN.with(Cell::get).is_some() && page(game).is_some()
}

/// Opens the config screen over Zoomify's page (`settings_click`): button 0
/// of the page, or Mod Menu's Configure.
pub(super) fn open(game: &Game<'_>, from_mod_menu: bool) {
    if page(game).is_none() {
        return;
    }
    let saved = zf::settings(|key| game.native_setting(zf::ID, key));
    let ru = zf::russian(&horizon_client::lang::code());
    let in_world = game.title.is_none() && game.player.is_some();
    SCREEN.with(|s| *s.borrow_mut() = Some(Yacl::new(saved, ru, in_world)));
    OPEN.with(|o| o.set(Some(if from_mod_menu { Mode::ModMenu } else { Mode::Page })));
    TAKEN.with(|t| t.set(0));
}

fn close() {
    OPEN.with(|o| o.set(None));
    SCREEN.with(|s| *s.borrow_mut() = None);
    CLIP.with(|c| c.borrow_mut().clear());
}

/// Once a frame: the data files, what the screen asked for (sounds, a toast,
/// the clipboard, its demo pictures in the atlas), and its closing.
pub(super) fn frame(game: &mut Game<'_>) {
    if !WRITTEN.with(Cell::get) {
        WRITTEN.with(|w| w.set(true));
        write_data(game);
    }
    let Some(mode) = OPEN.with(Cell::get) else {
        return;
    };
    let Some(i) = page(game) else {
        // The options went elsewhere some other way: the screen goes, unsaved.
        close();
        return;
    };
    let actions = with(|y| std::mem::take(&mut y.actions)).unwrap_or_default();
    for action in actions {
        match action {
            Action::Click => game.audio.play_button_click(),
            Action::Toast(title, message) => toast(game, title, message),
        }
    }
    for clip in CLIP.with(|c| std::mem::take(&mut *c.borrow_mut())) {
        match clip {
            Clip::Paste => {
                let text = game.clipboard.get();
                with(|y| y.paste(&text));
            }
            Clip::Copy(text) => {
                if !text.is_empty() {
                    game.clipboard.set(&text);
                }
            }
        }
    }
    // The pair of demo pictures the description shows, once decoded.
    if let Some((pair, uploaded)) = with(|y| (y.wanted, y.uploaded)).and_then(|(w, u)| Some((w?, u)))
        && uploaded != Some(pair)
        && let Some(pictures) = demo::pictures()
        && game.native_mod_image(demo::SLOT_WIDTH, demo::SLOT_HEIGHT, &pictures[pair.index()])
    {
        with(|y| y.uploaded = Some(pair));
    }
    if with(|y| y.closed).unwrap_or(true) {
        close();
        match mode {
            // Zoomify's page again, its labels following the new values.
            Mode::Page => game.native_settings_refresh(i),
            // Out of the options the way Escape goes: back to Mod Menu or the game.
            Mode::ModMenu => {
                let events = game.ui.options.back(&mut game.settings);
                let _ = game.handle_option_events(events);
            }
        }
    }
}

/// `SystemToast.add` (`PERIODIC_NOTIFICATION`): the game's system toast, in
/// the slot the game keeps for one.
fn toast(game: &mut Game<'_>, title: String, message: String) {
    let new = horizon_client::toasts::SystemToast::new(title, Some(message));
    match game.server_packs.load_fail.as_mut() {
        Some(shown) => shown.reset_from(new),
        None => {
            if game.toast_allowed("system", "periodic_notification") {
                game.server_packs.load_fail = Some(new);
            }
        }
    }
}

fn over(game: &Game<'_>, screen: &NativeScreenView<'_>) -> bool {
    screen.id == "options" && covers(game)
}

/// Draws the screen over the Options screen (`screen_render_post`).
pub(super) fn render(game: &Game<'_>, batch: &mut HudBatch, screen: &NativeScreenView<'_>) {
    if over(game, screen) {
        with(|y| y.draw(game, batch));
    }
}

fn bit(button: i32) -> u8 {
    1 << button.clamp(0, 7)
}

/// A mouse button pressed over the screen: true when it took it.
pub(super) fn pressed(game: &Game<'_>, screen: &NativeScreenView<'_>, button: i32, modifiers: u32) -> bool {
    if !over(game, screen) {
        return false;
    }
    TAKEN.with(|t| t.set(t.get() | bit(button)));
    let p = (screen.mouse[0], screen.mouse[1]);
    with(|y| y.press(game, p, button, modifiers));
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
    with(|y| y.release(button));
    true
}

/// The pointer dragged with a button held.
pub(super) fn dragged(game: &Game<'_>, screen: &NativeScreenView<'_>) -> bool {
    if !over(game, screen) {
        return false;
    }
    let p = (screen.mouse[0], screen.mouse[1]);
    with(|y| y.drag(game, p));
    true
}

/// The wheel.
pub(super) fn scrolled(game: &Game<'_>, screen: &NativeScreenView<'_>, scroll: [f64; 2]) -> bool {
    if !over(game, screen) {
        return false;
    }
    let p = (screen.mouse[0], screen.mouse[1]);
    with(|y| y.scroll(game, p, (scroll[0], scroll[1])));
    true
}

/// A key: the screen takes every one.
pub(super) fn key_pressed(game: &Game<'_>, screen: &NativeScreenView<'_>, key: &str, modifiers: u32) -> bool {
    if !over(game, screen) {
        return false;
    }
    let clip = match with(|y| y.key(game, key, modifiers)) {
        Some(KeyResult::Paste) => Some(Clip::Paste),
        Some(KeyResult::Copy(text)) => Some(Clip::Copy(text)),
        _ => None,
    };
    if let Some(clip) = clip {
        CLIP.with(|c| c.borrow_mut().push(clip));
    }
    true
}

/// A key let go: nothing under the screen hears it.
pub(super) fn key_released(game: &Game<'_>, screen: &NativeScreenView<'_>) -> bool {
    over(game, screen)
}

/// A character: the search field's when it has focus.
pub(super) fn char_typed(game: &Game<'_>, screen: &NativeScreenView<'_>, c: char) -> bool {
    if !over(game, screen) {
        return false;
    }
    with(|y| y.char_typed(c));
    true
}

// ---- the icon and Mod Menu's metadata ---------------------------------------------------

/// Zoomify's icon (its `fabric.mod.json`'s `assets/zoomify/zoomify.png`).
const ICON: &[u8] = horizon_client::native_data!("zoomify", "icon.png");

/// What Mod Menu shows of the mod, from Zoomify's `fabric.mod.json` (it
/// names no environment, so no Client badge).
fn modmenu_json() -> String {
    format!(
        r#"{{
  "schema": 1,
  "name": "{name}",
  "version": "{version}",
  "description": "A zoom mod with inifinte customizability.",
  "authors": ["isXander"],
  "contributors": ["DeportedToHell"],
  "license": "LGPL-3.0-or-later",
  "links": {{
    "homepage": "https://github.com/isXander/Zoomify",
    "sources": "https://github.com/DeportedToHell/MCRE-Mods/tree/main/zoomify",
    "issues": "https://github.com/DeportedToHell/MCRE-Mods/issues"
  }},
  "badges": [],
  "config_screen": true
}}
"#,
        name = zf::NAME,
        version = zf::VERSION,
    )
}

/// `icon.png` and `modmenu.json` in the mod's data folder, each written only
/// when it differs from what is there.
fn write_data(game: &Game<'_>) {
    let dir = game.native_data_dir(zf::ID);
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
