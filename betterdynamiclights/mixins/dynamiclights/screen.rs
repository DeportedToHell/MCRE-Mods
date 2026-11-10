// Dynamic Lights for Horizon: the settings screen, an options screen in the
// game's own look (the title, a scrolling list of buttons and sliders two to
// a row under headings, tooltips, Done), over the Options screen while the
// mod's page is open. The page's first button opens it, and so does Mod
// Menu's Configure. Done and Escape go back to the page, or out of the
// options (to Mod Menu) when Mod Menu opened it. Every change is kept at once,
// as the game's options are. Also writes the icon and Mod Menu's metadata
// into the mod's data folder.
//
// The screen hooks only read the game: what needs to change it (a click's
// sound, a key binding, closing) waits for the next frame.
use super::dynamiclights as dl;
use super::dynamiclights_glue as glue;
use super::{Game, Screen};
use dl::{Heading, Kind, Opt};
use horizon_client::gamepad;
use horizon_client::hud::HudBatch;
use horizon_client::mod_api::NativeScreenView;
use horizon_client::options_ui::Page;
use horizon_client::text::{self, Style, Text};
use horizon_client::ui;
use std::cell::RefCell;

/// Mod Menu's Configure: `native_settings_click(id, MODMENU_CONFIGURE)` after it opened the mod's page.
pub const MODMENU_CONFIGURE: usize = 0x4D4D_4346;

/// GLFW's Shift bit.
const SHIFT: u32 = 1;

/// The list's rows, top to bottom.
#[derive(Clone, Copy)]
enum Row {
    Wide(Opt),
    Pair(Opt, Opt),
    Header(Heading),
    Key,
}

const ROWS: [Row; 11] = [
    Row::Wide(Opt::Enabled),
    Row::Header(Heading::Sources),
    Row::Pair(Opt::Kind(Kind::OwnHands), Opt::Kind(Kind::Players)),
    Row::Pair(Opt::Kind(Kind::Mobs), Opt::Kind(Kind::Dropped)),
    Row::Pair(Opt::Kind(Kind::Burning), Opt::Kind(Kind::Creatures)),
    Row::Pair(Opt::Kind(Kind::Projectiles), Opt::Kind(Kind::Explosives)),
    Row::Header(Heading::Lighting),
    Row::Pair(Opt::Water, Opt::Brightness),
    Row::Pair(Opt::MaxSources, Opt::Range),
    Row::Header(Heading::Controls),
    Row::Key,
];

/// The screen's widgets.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum W {
    Opt(Opt),
    Key,
    KeyReset,
    Done,
}

/// A widget where it is drawn this frame.
#[derive(Clone, Copy)]
struct Placed {
    w: W,
    rect: [f32; 4],
    /// In the scrolling list (clipped to it), not the footer.
    list: bool,
}

/// Text in the list: a heading (centred) or the key row's label.
struct Label {
    text: Text,
    x: f32,
    y: f32,
    centred: bool,
}

/// List geometry: as the game's options screens (header and footer 33 high,
/// rows 25 apart, 310 wide).
const TOP: f32 = 33.0;
const ROW: f32 = 25.0;

struct Layout {
    widgets: Vec<Placed>,
    labels: Vec<Label>,
    top: f32,
    bottom: f32,
    content: f32,
}

/// Who opened the screen: the page's button, or Mod Menu's Configure.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Page,
    ModMenu,
}

#[derive(Default)]
struct State {
    open: Option<Mode>,
    /// Done or Escape: the screen goes at the next frame.
    close: bool,
    /// The focused widget (a click's or the keyboard's).
    focus: Option<W>,
    /// The focus came from the keyboard: its tooltip shows beside it.
    keyboard: bool,
    scroll: ui::Scroll,
    /// The slider held by the mouse.
    dragging: Option<Opt>,
    /// The key button waits for a key or a mouse button.
    binding: bool,
    /// A key chosen, to keep at the next frame: None restores the default.
    new_key: Option<Option<String>>,
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

/// The GUI's size in pixels, as the screen is drawn.
fn gui_size(game: &Game<'_>) -> (f32, f32) {
    let size = game.renderer.as_ref().map_or([1280, 720], |r| [r.config.width, r.config.height]);
    let b = HudBatch::new(size);
    (b.width, b.height)
}

fn inside(p: (f32, f32), r: [f32; 4]) -> bool {
    ui::hovered(p, r[0], r[1], r[2], r[3])
}

/// The mod's page is the one the Options screen shows: its index in Options > Mods.
fn page(game: &Game<'_>) -> Option<usize> {
    if game.screen != Screen::Options {
        return None;
    }
    match game.ui.options.page() {
        Page::NativeMod(i) if game.settings.mods.installed.get(i).is_some_and(|m| m.id == dl::ID) => Some(i),
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

/// Opens the screen over the mod's page: the page's first button, or Mod
/// Menu's Configure.
pub(super) fn open(game: &Game<'_>, from_mod_menu: bool) {
    if page(game).is_none() {
        return;
    }
    with(|s| {
        *s = State {
            open: Some(if from_mod_menu { Mode::ModMenu } else { Mode::Page }),
            written: s.written,
            clicks: s.clicks,
            ..State::default()
        };
    });
}

/// Once a frame: the data files, the clicks' sound, a key binding and the
/// screen's closing.
pub(super) fn frame(game: &mut Game<'_>) {
    if !with(|s| std::mem::replace(&mut s.written, true)) {
        write_data(game);
    }
    for _ in 0..with(|s| std::mem::take(&mut s.clicks)) {
        game.audio.play_button_click();
    }
    if let Some(key) = with(|s| s.new_key.take()) {
        game.native_set_mod_key(dl::ID, dl::TOGGLE_KEY, key.as_deref());
    }
    let Some(mode) = with(|s| s.open) else {
        return;
    };
    match page(game) {
        // The options went elsewhere some other way: the screen goes.
        None => with(|s| s.open = None),
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

// ---- layout -----------------------------------------------------------------------------

fn heading(text: &str) -> Text {
    Text::literal(text).with_style(&Style { bold: Some(true), underlined: Some(true), ..Style::default() })
}

/// Every widget and label where the list, scrolled by `scroll`, puts them.
fn layout((w, h): (f32, f32), scroll: f32, ru: bool) -> Layout {
    let cx = (w / 2.0).floor();
    let (top, bottom) = (TOP, h - TOP);
    let mut widgets = Vec::new();
    let mut labels = Vec::new();
    let mut y = top + 2.0 - scroll;
    for (n, row) in ROWS.iter().enumerate() {
        match *row {
            Row::Wide(opt) => {
                widgets.push(Placed { w: W::Opt(opt), rect: [cx - 155.0, y + 2.0, 310.0, 20.0], list: true });
                y += ROW;
            }
            Row::Pair(a, b) => {
                widgets.push(Placed { w: W::Opt(a), rect: [cx - 155.0, y + 2.0, 150.0, 20.0], list: true });
                widgets.push(Placed { w: W::Opt(b), rect: [cx + 5.0, y + 2.0, 150.0, 20.0], list: true });
                y += ROW;
            }
            Row::Header(hd) => {
                // A heading row: room above it unless it starts the list.
                let padding = if n == 0 { 0.0 } else { 18.0 };
                labels.push(Label { text: heading(hd.text(ru)), x: cx, y: y + 2.0 + padding + 4.0, centred: true });
                y += padding + 18.0;
            }
            Row::Key => {
                // As a row of the game's Key Binds list: the name, the key, Reset.
                labels.push(Label { text: Text::literal(dl::key_label(ru)), x: cx - 155.0, y: y + 8.0, centred: false });
                widgets.push(Placed { w: W::Key, rect: [cx + 25.0, y + 2.0, 75.0, 20.0], list: true });
                widgets.push(Placed { w: W::KeyReset, rect: [cx + 105.0, y + 2.0, 50.0, 20.0], list: true });
                y += ROW;
            }
        }
    }
    let content = y + scroll - top + 2.0;
    widgets.push(Placed { w: W::Done, rect: [cx - 100.0, h - 27.0, 200.0, 20.0], list: false });
    Layout { widgets, labels, top, bottom, content }
}

/// The layout with the scroll kept inside the list.
fn current(game: &Game<'_>) -> Layout {
    let size = gui_size(game);
    let ru = glue::russian();
    let first = layout(size, with(|s| s.scroll.amount), ru);
    let changed = with(|s| {
        let before = s.scroll.amount;
        s.scroll.clamp(first.top, first.bottom, first.content);
        before != s.scroll.amount
    });
    if changed { layout(size, with(|s| s.scroll.amount), ru) } else { first }
}

/// The widget under the pointer: a list widget only inside the list.
fn under(l: &Layout, p: (f32, f32)) -> Option<Placed> {
    l.widgets
        .iter()
        .copied()
        .find(|pl| inside(p, pl.rect) && (!pl.list || (p.1 >= l.top && p.1 < l.bottom)))
}

// ---- drawing ----------------------------------------------------------------------------

/// The key the toggle is bound to, with a choice not yet kept.
fn bound_key(game: &Game<'_>) -> String {
    match with(|s| s.new_key.clone()) {
        Some(Some(k)) => k,
        Some(None) => dl::UNBOUND.to_owned(),
        None => glue::toggle_key(game),
    }
}

/// The game's own binding on the same key, if any.
fn clash(game: &Game<'_>, key: &str) -> Option<&'static str> {
    (key != dl::UNBOUND).then(|| game.settings.keys.mapping_for(key)).flatten()
}

fn on_off(on: bool) -> Text {
    Text::translate(if on { "options.on" } else { "options.off" }, &[])
}

/// Options other than the main switch do nothing while it is off.
fn active(w: W, s: &dl::Settings) -> bool {
    match w {
        W::Opt(Opt::Enabled) | W::Done | W::Key => true,
        W::Opt(_) => s.enabled,
        W::KeyReset => true,
    }
}

/// Draws the screen over the Options screen (`screen_render_post`): its
/// background, the title, the list and Done, then a tooltip.
pub(super) fn render(game: &Game<'_>, batch: &mut HudBatch, screen: &NativeScreenView<'_>) {
    if !over(game, screen) {
        return;
    }
    let atlas = &game.gui.atlas;
    let (w, h) = (batch.width, batch.height);
    let in_world = game.ui.options.in_world;
    game.native_apply_screen_blur();
    ui::menu_background(batch, atlas, 0.0, 0.0, w, h, in_world);
    let ru = glue::russian();
    let l = current(game);
    ui::list_background(batch, atlas, l.top, l.bottom, in_world);

    let title = Text::literal(dl::title(ru));
    let tw = atlas.font.width_text(&title);
    batch.text_styled(atlas, &title, ((w - tw) / 2.0).floor(), 12.0, u32::MAX, true);

    let s = glue::settings(game);
    let (focus, keyboard, dragging, binding) = with(|st| (st.focus, st.keyboard, st.dragging, st.binding));
    let mouse = (screen.mouse[0], screen.mouse[1]);
    let hovered = under(&l, mouse);
    let key = bound_key(game);
    let clashing = clash(game, &key);

    for label in &l.labels {
        if label.y + 9.0 <= l.top || label.y >= l.bottom {
            continue;
        }
        let start = batch.vertices.len();
        let x = if label.centred { (label.x - atlas.font.width_text(&label.text) / 2.0).floor() } else { label.x };
        batch.text_styled(atlas, &label.text, x, label.y, u32::MAX, true);
        if label.y < l.top || label.y + 9.0 > l.bottom {
            batch.clip_since(start, 0.0, l.top, w, l.bottom);
        }
    }

    for pl in &l.widgets {
        let [x, y, ww, wh] = pl.rect;
        if pl.list && (y >= l.bottom || y + wh <= l.top) {
            continue;
        }
        let start = batch.vertices.len();
        let on = active(pl.w, &s);
        let hover = hovered.is_some_and(|h| h.w == pl.w) || focus == Some(pl.w);
        match pl.w {
            W::Opt(opt) if opt.slider() => {
                let label = Text::option_name_value(Text::literal(opt.caption(ru)), Text::literal(s.number_text(opt, ru)));
                let held = dragging == Some(opt);
                ui::slider_ext(batch, atlas, x, y, ww, &label, s.fraction(opt), hover || held, focus == Some(pl.w), keyboard && focus == Some(pl.w), on, held);
            }
            W::Opt(opt) => {
                let label = Text::option_name_value(Text::literal(opt.caption(ru)), on_off(s.switch(opt).unwrap_or(false)));
                ui::button(batch, atlas, x, y, ww, wh, &label, on, hover);
            }
            W::Key => {
                let mut message = horizon_client::keybinds::display_name(&key);
                if clashing.is_some() {
                    message = Text::literal("[ ").append(message.with_color(text::WHITE)).append(" ]").with_color(text::YELLOW);
                }
                if binding {
                    message = Text::literal("> ")
                        .append(message.with_style(&Style { color: Some(text::WHITE), underlined: Some(true), ..Style::default() }))
                        .append(" <")
                        .with_color(text::YELLOW);
                }
                ui::button(batch, atlas, x, y, ww, wh, &message, true, hover);
            }
            W::KeyReset => {
                let label = Text::translate("controls.reset", &[]);
                ui::button(batch, atlas, x, y, ww, wh, &label, key != dl::UNBOUND, hover);
            }
            W::Done => {
                let label = Text::translate("gui.done", &[]);
                ui::action_button(batch, atlas, horizon_client::widgets::Action::Done, x, y, ww, wh, &label, true, hover);
            }
        }
        if pl.list && (y < l.top || y + wh > l.bottom) {
            batch.clip_since(start, 0.0, l.top, w, l.bottom);
        }
    }

    ui::separators(batch, atlas, l.top, l.bottom, in_world);
    ui::scrollbar(batch, atlas, (w / 2.0).floor() + 163.0, l.top, l.bottom, l.content, with(|st| st.scroll.amount));

    // The tooltip: the pointer's widget, else the keyboard's.
    let tip_of = |w: W| -> Option<Text> {
        match w {
            W::Opt(opt) => Some(Text::literal(opt.tooltip(ru))),
            W::Key => clashing.map(|m| Text::translate("controls.keybinds.duplicateKeybinds", &[Text::translate(m, &[])])),
            _ => None,
        }
    };
    if dragging.is_none() {
        if let Some(pl) = hovered
            && let Some(tip) = tip_of(pl.w)
        {
            let lines: Vec<Text> = tip.plain().split('\n').flat_map(|line| atlas.font.split(&Text::literal(line.to_owned()), 170.0)).collect();
            ui::tooltip(batch, atlas, &lines, mouse);
        } else if keyboard
            && let Some(f) = focus
            && let Some(pl) = l.widgets.iter().find(|pl| pl.w == f)
            && let Some(tip) = tip_of(f)
        {
            let lines: Vec<Text> = tip.plain().split('\n').flat_map(|line| atlas.font.split(&Text::literal(line.to_owned()), 170.0)).collect();
            ui::tooltip_menu(batch, atlas, &lines, mouse, pl.rect);
        }
    }
}

// ---- input ------------------------------------------------------------------------------

/// Widget `w` pressed (a click, Enter or Space).
fn press(game: &Game<'_>, w: W) {
    let s = glue::settings(game);
    if !active(w, &s) {
        return;
    }
    with(|st| st.clicks += 1);
    match w {
        W::Opt(opt) if !opt.slider() => {
            let mut s = s;
            s.flip(opt);
            glue::store(game, s);
        }
        W::Opt(_) => {}
        W::Key => with(|st| st.binding = true),
        W::KeyReset => with(|st| {
            st.new_key = Some(None);
            st.binding = false;
        }),
        W::Done => with(|st| st.close = true),
    }
}

/// A slider follows the pointer's x.
fn slide(game: &Game<'_>, opt: Opt, rect: [f32; 4], x: f32) {
    let mut s = glue::settings(game);
    if s.set_fraction(opt, ui::slider_value(rect[0], rect[2], x)) {
        glue::store(game, s);
    }
}

fn bit(button: i32) -> u8 {
    1 << button.clamp(0, 7)
}

/// A mouse button's key name, as the game names it.
fn mouse_key(button: i32) -> String {
    match button {
        0 => "key.mouse.left".to_owned(),
        1 => "key.mouse.right".to_owned(),
        2 => "key.mouse.middle".to_owned(),
        n => format!("key.mouse.{}", n + 1),
    }
}

/// A mouse button pressed over the screen: true when it took it.
pub(super) fn pressed(game: &Game<'_>, screen: &NativeScreenView<'_>, button: i32) -> bool {
    if !over(game, screen) {
        return false;
    }
    with(|s| s.taken |= bit(button));
    // The key button waits: any mouse button is the new key.
    if with(|s| s.binding) {
        with(|s| {
            s.new_key = Some(Some(mouse_key(button)));
            s.binding = false;
        });
        return true;
    }
    if button != 0 {
        return true;
    }
    let p = (screen.mouse[0], screen.mouse[1]);
    let l = current(game);
    let bar_x = (gui_size(game).0 / 2.0).floor() + 163.0;
    if with(|s| s.scroll.press(p, bar_x, l.top, l.bottom, l.content)) {
        return true;
    }
    match under(&l, p) {
        Some(pl) => {
            with(|s| {
                s.focus = Some(pl.w);
                s.keyboard = false;
            });
            if let W::Opt(opt) = pl.w
                && opt.slider()
            {
                if active(pl.w, &glue::settings(game)) {
                    with(|s| s.dragging = Some(opt));
                    slide(game, opt, pl.rect, p.0);
                }
            } else {
                press(game, pl.w);
            }
        }
        None => with(|s| s.focus = None),
    }
    true
}

/// A mouse button let go: taken when its press was. A slider let go clicks.
pub(super) fn released(button: i32) -> bool {
    with(|s| {
        let was = s.taken & bit(button) != 0;
        s.taken &= !bit(button);
        if button == 0 {
            s.scroll.release();
            if s.dragging.take().is_some() {
                s.clicks += 1;
            }
        }
        was
    })
}

/// The pointer dragged: the held slider or scrollbar follows it.
pub(super) fn dragged(game: &Game<'_>, screen: &NativeScreenView<'_>) -> bool {
    if !over(game, screen) {
        return false;
    }
    let p = (screen.mouse[0], screen.mouse[1]);
    let l = current(game);
    if with(|s| s.scroll.held()) {
        with(|s| s.scroll.drag(p.1, l.top, l.bottom, l.content));
    } else if let Some(opt) = with(|s| s.dragging)
        && let Some(pl) = l.widgets.iter().find(|pl| pl.w == W::Opt(opt))
    {
        slide(game, opt, pl.rect, p.0);
    }
    true
}

/// The wheel scrolls the list (half a row a step).
pub(super) fn scrolled(game: &Game<'_>, screen: &NativeScreenView<'_>, scroll: [f64; 2]) -> bool {
    if !over(game, screen) {
        return false;
    }
    let l = current(game);
    with(|s| s.scroll.wheel(scroll[1] as f32, ROW / 2.0, l.top, l.bottom, l.content));
    true
}

/// Scrolls the list so that the focused widget shows.
fn show_focus(game: &Game<'_>) {
    let l = current(game);
    let Some(f) = with(|s| s.focus) else { return };
    let Some(pl) = l.widgets.iter().find(|pl| pl.w == f && pl.list) else { return };
    let [_, y, _, h] = pl.rect;
    with(|s| {
        if y < l.top + 2.0 {
            s.scroll.amount -= l.top + 2.0 - y;
        } else if y + h > l.bottom - 2.0 {
            s.scroll.amount += y + h - (l.bottom - 2.0);
        }
        s.scroll.clamp(l.top, l.bottom, l.content);
    });
}

/// A key: while the key button waits, the new key (Escape: none); else
/// Escape closes, Tab and the arrows move the focus, Enter and Space press,
/// left and right move a focused slider. Every key is the screen's.
pub(super) fn key_pressed(game: &Game<'_>, screen: &NativeScreenView<'_>, key: &str, modifiers: u32) -> bool {
    if !over(game, screen) {
        return false;
    }
    if with(|s| s.binding) {
        with(|s| {
            s.new_key = Some((key != "key.keyboard.escape").then(|| key.to_owned()));
            s.binding = false;
        });
        return true;
    }
    let shift = modifiers & SHIFT != 0;
    let l = current(game);
    let order: Vec<W> = l.widgets.iter().map(|pl| pl.w).collect();
    let focus = with(|s| s.focus);
    let index = focus.and_then(|f| order.iter().position(|w| *w == f));
    let set_focus = |i: usize| {
        with(|s| {
            s.focus = Some(order[i]);
            s.keyboard = true;
        });
        show_focus(game);
    };
    match key {
        "key.keyboard.escape" => with(|s| s.close = true),
        "key.keyboard.tab" => {
            let n = order.len();
            set_focus(match index {
                None if shift => n - 1,
                None => 0,
                Some(i) if shift => (i + n - 1) % n,
                Some(i) => (i + 1) % n,
            });
        }
        "key.keyboard.enter" | "key.keyboard.keypad.enter" | "key.keyboard.space" => {
            if let Some(w) = focus {
                press(game, w);
            }
        }
        "key.keyboard.left" | "key.keyboard.right"
            if matches!(focus, Some(W::Opt(o)) if o.slider()) && with(|s| s.keyboard) =>
        {
            if let Some(W::Opt(opt)) = focus
                && active(W::Opt(opt), &glue::settings(game))
            {
                let mut s = glue::settings(game);
                if s.step(opt, key == "key.keyboard.right") {
                    glue::store(game, s);
                }
            }
        }
        _ => {
            let dir = match key {
                "key.keyboard.left" => Some(gamepad::Button::DPadLeft),
                "key.keyboard.right" => Some(gamepad::Button::DPadRight),
                "key.keyboard.up" => Some(gamepad::Button::DPadUp),
                "key.keyboard.down" => Some(gamepad::Button::DPadDown),
                _ => None,
            };
            if let Some(dir) = dir {
                let next = match index {
                    None => Some(0),
                    Some(i) => {
                        let rects: Vec<[f32; 4]> = l.widgets.iter().map(|pl| pl.rect).collect();
                        gamepad::focus_step_rect(rects[i], dir, &rects)
                    }
                };
                if let Some(next) = next {
                    set_focus(next);
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

/// The mod's icon: a lantern and its glow (tools/icon.mjs draws it).
const ICON: &[u8] = horizon_client::native_data!("dynamiclights", "icon.png");

/// What Mod Menu shows of the mod.
fn modmenu_json() -> String {
    format!(
        r#"{{
  "schema": 1,
  "name": "{name}",
  "version": "{version}",
  "description": "{en}",
  "description_ru": "{ru}",
  "authors": ["DeportedToHell"],
  "license": "MIT",
  "links": {{
    "sources": "https://github.com/DeportedToHell/MCRE-Mods/tree/main/betterdynamiclights",
    "issues": "https://github.com/DeportedToHell/MCRE-Mods/issues"
  }},
  "badges": ["client"],
  "config_screen": true
}}
"#,
        name = dl::NAME,
        version = dl::VERSION,
        en = dl::description(false),
        ru = dl::description(true),
    )
}

/// `icon.png` and `modmenu.json` in the mod's data folder, each written only
/// when it differs from what is there.
fn write_data(game: &Game<'_>) {
    let dir = game.native_data_dir(dl::ID);
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
