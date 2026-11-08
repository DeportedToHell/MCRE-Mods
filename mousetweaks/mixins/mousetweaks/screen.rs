// Mouse Tweaks for Horizon: the mod's own options screen (`ConfigScreen`,
// "Mouse Tweaks Options"), laid out with its numbers in the game's widgets,
// over the Options screen while the mod's page is open. Mod Menu's Configure
// opens it (`ModMenuApiImpl`), and so does the page's first button. Done and
// Escape go back to the page, or out of the options (to Mod Menu) when Mod
// Menu opened it. Each change is kept at once, as `ConfigScreen.removed`
// saves whatever way the screen closes. Also writes the icon and Mod Menu's
// metadata into the mod's data folder.
//
// The screen hooks only read the game: what needs to change it (a click's
// sound, closing) waits for the next frame.
use super::mousetweaks as mt;
use super::{Game, Screen};
use horizon_client::gamepad;
use horizon_client::hud::HudBatch;
use horizon_client::mod_api::NativeScreenView;
use horizon_client::options_ui::Page;
use horizon_client::text::Text;
use horizon_client::ui;
use std::cell::RefCell;

/// Mod Menu's Configure: `native_settings_click(id, MODMENU_CONFIGURE)` after it opened the mod's page.
pub const MODMENU_CONFIGURE: usize = 0x4D4D_4346;

/// GLFW's Shift bit.
const SHIFT: u32 = 1;

/// Who opened the screen: the page's button, or Mod Menu's Configure.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Page,
    ModMenu,
}

/// The screen's widgets, in `init`'s order (Tab's order too).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum W {
    Rmb,
    Wheel,
    LmbWith,
    LmbWithout,
    Order,
    Direction,
    Scaling,
    Done,
}

const WIDGETS: [W; 8] = [W::Rmb, W::Wheel, W::LmbWith, W::LmbWithout, W::Order, W::Direction, W::Scaling, W::Done];

impl W {
    /// The settings row the button changes (`Settings::cycle`); None for Done.
    fn row(self) -> Option<usize> {
        match self {
            W::Rmb => Some(0),
            W::LmbWith => Some(1),
            W::LmbWithout => Some(2),
            W::Wheel => Some(3),
            W::Order => Some(4),
            W::Direction => Some(5),
            W::Scaling => Some(6),
            W::Done => None,
        }
    }

    /// `ConfigScreen.init`'s bounds on a `w` × `h` screen (Java's int halves
    /// and sixths). Config.debug's Debug Mode row (`height / 6 + 120`) is not
    /// here: the port has no debug log.
    fn rect(self, (w, h): (f32, f32)) -> [f32; 4] {
        let cx = (w as i32 / 2) as f32;
        let top = (h as i32 / 6) as f32;
        match self {
            W::Rmb => [cx - 155.0, top, 150.0, 20.0],
            W::Wheel => [cx - 155.0, top + 24.0, 150.0, 20.0],
            W::LmbWith => [cx + 5.0, top, 150.0, 20.0],
            W::LmbWithout => [cx + 5.0, top + 24.0, 150.0, 20.0],
            W::Order => [cx - 155.0, top + 48.0, 310.0, 20.0],
            W::Direction => [cx - 155.0, top + 72.0, 310.0, 20.0],
            W::Scaling => [cx - 155.0, top + 96.0, 310.0, 20.0],
            W::Done => [cx - 100.0, h - 27.0, 200.0, 20.0],
        }
    }
}

#[derive(Default)]
struct State {
    open: Option<Mode>,
    /// Done or Escape: the screen goes at the next frame.
    close: bool,
    /// The focused widget (a click's or the keyboard's): drawn highlighted.
    focus: Option<W>,
    /// Mouse buttons whose press the screen took, so their release is taken too.
    taken: u8,
    /// Button clicks to sound at the next frame.
    clicks: u32,
    /// The icon and the metadata were written this run.
    written: bool,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|s| f(&mut s.borrow_mut()))
}

fn russian() -> bool {
    mt::russian(&horizon_client::lang::code())
}

fn settings(game: &Game<'_>) -> mt::Settings {
    mt::settings(|key| game.native_setting(mt::ID, key))
}

/// The values kept: for the tweaks at once, and in the mod's settings.
fn store(game: &Game<'_>, s: mt::Settings) {
    mt::set_settings(s);
    for (key, value) in s.stored() {
        game.native_set_setting(mt::ID, key, &value);
    }
}

/// The GUI's size in pixels, as the screen is drawn.
fn gui_size(game: &Game<'_>) -> (f32, f32) {
    let size = game.renderer.as_ref().map_or([1280, 720], |r| [r.config.width, r.config.height]);
    let b = HudBatch::new(size);
    (b.width, b.height)
}

fn inside(p: (f32, f32), r: [f32; 4]) -> bool {
    ui::hovered(p, r[0], r[1], r[2], r[3])
}

/// Mouse Tweaks' page is the one the Options screen shows: its index in Options > Mods.
fn page(game: &Game<'_>) -> Option<usize> {
    if game.screen != Screen::Options {
        return None;
    }
    match game.ui.options.page() {
        Page::NativeMod(i) if game.settings.mods.installed.get(i).is_some_and(|m| m.id == mt::ID) => Some(i),
        _ => None,
    }
}

/// The screen is up over the mod's page: the page stays hidden and the
/// screen's input is the screen's.
pub(super) fn covers(game: &Game<'_>) -> bool {
    with(|s| s.open.is_some()) && page(game).is_some()
}

fn over(game: &Game<'_>, screen: &NativeScreenView<'_>) -> bool {
    screen.id == "options" && covers(game)
}

/// Opens the screen over the mod's page (`settings_click`): the page's
/// first button, or Mod Menu's Configure.
pub(super) fn open(game: &Game<'_>, from_mod_menu: bool) {
    if page(game).is_none() {
        return;
    }
    with(|s| {
        s.open = Some(if from_mod_menu { Mode::ModMenu } else { Mode::Page });
        s.close = false;
        s.focus = None;
        s.taken = 0;
    });
}

/// Once a frame: the data files, the clicks' sound and the screen's closing.
pub(super) fn frame(game: &mut Game<'_>) {
    if !with(|s| std::mem::replace(&mut s.written, true)) {
        write_data(game);
    }
    for _ in 0..with(|s| std::mem::take(&mut s.clicks)) {
        game.audio.play_button_click();
    }
    let Some(mode) = with(|s| s.open) else {
        return;
    };
    match page(game) {
        // The options went elsewhere some other way: the screen goes.
        None => with(|s| {
            s.open = None;
            s.close = false;
        }),
        Some(i) if with(|s| s.close) => {
            with(|s| {
                s.open = None;
                s.close = false;
            });
            match mode {
                // The mod's page again, its labels following the new values.
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

/// A widget's label: `CycleButton`'s "Caption: value" (the game's ON and
/// OFF for a switch), or Done.
fn label(w: W, s: &mt::Settings, ru: bool) -> Text {
    let Some(row) = w.row() else {
        return Text::translate("gui.done", &[]);
    };
    let value = match s.value(row, ru) {
        Ok(on) => Text::translate(if on { "options.on" } else { "options.off" }, &[]),
        Err(text) => Text::literal(text),
    };
    Text::option_name_value(Text::literal(mt::Settings::caption(row, ru)), value)
}

/// Draws the screen over the Options screen (`screen_render_post`): its
/// background, the title (`StringWidget` at y 15) and the buttons.
pub(super) fn render(game: &Game<'_>, batch: &mut HudBatch, screen: &NativeScreenView<'_>) {
    if !over(game, screen) {
        return;
    }
    let atlas = &game.gui.atlas;
    let (w, h) = (batch.width, batch.height);
    game.native_apply_screen_blur();
    ui::menu_background(batch, atlas, 0.0, 0.0, w, h, game.ui.options.in_world);
    let ru = russian();
    let title = Text::literal(if ru { "Настройки Mouse Tweaks" } else { "Mouse Tweaks Options" });
    let tw = atlas.font.width_text(&title);
    batch.text_styled(atlas, &title, (w as i32 / 2 - tw as i32 / 2) as f32, 15.0, u32::MAX, true);
    let s = settings(game);
    let focus = with(|st| st.focus);
    let mouse = (screen.mouse[0], screen.mouse[1]);
    for wd in WIDGETS {
        let r = wd.rect((w, h));
        let hover = inside(mouse, r) || focus == Some(wd);
        let text = label(wd, &s, ru);
        if wd == W::Done {
            ui::action_button(batch, atlas, horizon_client::widgets::Action::Done, r[0], r[1], r[2], r[3], &text, true, hover);
        } else {
            ui::button(batch, atlas, r[0], r[1], r[2], r[3], &text, true, hover);
        }
    }
}

/// Widget `w` pressed (a click, Enter or Space): a click's sound, then the
/// button's action.
fn press(game: &Game<'_>, w: W, shift: bool) {
    with(|s| {
        s.clicks += 1;
        if w == W::Done {
            s.close = true;
        }
    });
    if let Some(row) = w.row() {
        let mut s = settings(game);
        s.cycle(row, shift);
        store(game, s);
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
    with(|s| s.taken |= bit(button));
    // `AbstractWidget` accepts only the primary button.
    if button != 0 {
        return true;
    }
    let p = (screen.mouse[0], screen.mouse[1]);
    let size = gui_size(game);
    if let Some(w) = WIDGETS.into_iter().find(|w| inside(p, w.rect(size))) {
        with(|s| s.focus = Some(w));
        press(game, w, modifiers & SHIFT != 0);
    }
    true
}

/// A mouse button let go: taken when its press was.
pub(super) fn released(button: i32) -> bool {
    with(|s| {
        let was = s.taken & bit(button) != 0;
        s.taken &= !bit(button);
        was
    })
}

/// The pointer dragged: nothing under the screen hears it.
pub(super) fn dragged(game: &Game<'_>, screen: &NativeScreenView<'_>) -> bool {
    over(game, screen)
}

/// The wheel: a cycle button under the pointer steps (`CycleButton.mouseScrolled`:
/// up goes back), with no sound.
pub(super) fn scrolled(game: &Game<'_>, screen: &NativeScreenView<'_>, scroll: [f64; 2]) -> bool {
    if !over(game, screen) {
        return false;
    }
    let p = (screen.mouse[0], screen.mouse[1]);
    let size = gui_size(game);
    if scroll[1] != 0.0
        && let Some(row) = WIDGETS.into_iter().find(|w| inside(p, w.rect(size))).and_then(W::row)
    {
        let mut s = settings(game);
        s.cycle(row, scroll[1] > 0.0);
        store(game, s);
    }
    true
}

/// A key: Escape closes (`onClose`), Tab and the arrows move the focus,
/// Enter and Space press the focused button. Every key is the screen's.
pub(super) fn key_pressed(game: &Game<'_>, screen: &NativeScreenView<'_>, key: &str, modifiers: u32) -> bool {
    if !over(game, screen) {
        return false;
    }
    let shift = modifiers & SHIFT != 0;
    let size = gui_size(game);
    let focus = with(|s| s.focus);
    let index = focus.and_then(|f| WIDGETS.iter().position(|w| *w == f));
    let arrow = match key {
        "key.keyboard.left" => Some(gamepad::Button::DPadLeft),
        "key.keyboard.right" => Some(gamepad::Button::DPadRight),
        "key.keyboard.up" => Some(gamepad::Button::DPadUp),
        "key.keyboard.down" => Some(gamepad::Button::DPadDown),
        _ => None,
    };
    match key {
        "key.keyboard.escape" => with(|s| s.close = true),
        "key.keyboard.tab" => {
            let n = WIDGETS.len();
            let next = match index {
                None if shift => n - 1,
                None => 0,
                Some(i) if shift => (i + n - 1) % n,
                Some(i) => (i + 1) % n,
            };
            with(|s| s.focus = Some(WIDGETS[next]));
        }
        "key.keyboard.enter" | "key.keyboard.keypad.enter" | "key.keyboard.space" => {
            if let Some(w) = focus {
                press(game, w, shift);
            }
        }
        _ => {
            if let Some(dir) = arrow {
                let next = match index {
                    None => Some(0),
                    Some(i) => {
                        let rects: Vec<[f32; 4]> = WIDGETS.iter().map(|w| w.rect(size)).collect();
                        gamepad::focus_step_rect(rects[i], dir, &rects)
                    }
                };
                if let Some(next) = next {
                    with(|s| s.focus = Some(WIDGETS[next]));
                }
            }
        }
    }
    true
}

/// A key let go, or a character typed: nothing under the screen hears it.
pub(super) fn swallowed(game: &Game<'_>, screen: &NativeScreenView<'_>) -> bool {
    over(game, screen)
}

// ---- the icon and Mod Menu's metadata ---------------------------------------------------

/// Mouse Tweaks' icon (the catalog's).
const ICON: &[u8] = horizon_client::native_data!("mousetweaks", "icon.png");

/// What Mod Menu shows of the mod, from Mouse Tweaks' `fabric.mod.json`
/// (its description is `gradle.properties`' `modDescription`; environment
/// "client": the Client badge).
fn modmenu_json() -> String {
    format!(
        r#"{{
  "schema": 1,
  "name": "{name}",
  "version": "{version}",
  "description": "A mod that enhances the inventory management by adding various additional functions to the usual mouse buttons.",
  "authors": ["Ivan Molodetskikh (YaLTeR)"],
  "contributors": ["mezz", "juliand665", "panoskj", "FabiClawZ", "ErrorMikey", "livInFear", "Abelpro678", "DeportedToHell"],
  "license": "BSD-3-Clause",
  "links": {{
    "homepage": "https://www.curseforge.com/minecraft/mc-mods/mouse-tweaks",
    "sources": "https://github.com/DeportedToHell/MCRE-Mods/tree/main/mousetweaks",
    "issues": "https://github.com/DeportedToHell/MCRE-Mods/issues"
  }},
  "badges": ["client"],
  "config_screen": true
}}
"#,
        name = mt::NAME,
        version = mt::VERSION,
    )
}

/// `icon.png` and `modmenu.json` in the mod's data folder, each written only
/// when it differs from what is there.
fn write_data(game: &Game<'_>) {
    let dir = game.native_data_dir(mt::ID);
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
