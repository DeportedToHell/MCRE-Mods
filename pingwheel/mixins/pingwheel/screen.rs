// Ping Wheel's settings screen (`SettingsScreen`, an `OptionsSubScreen`): the title "Ping Wheel
// Settings", the options list of two-column rows (`OptionsList.addSmall`: Ping Volume and Ping
// Duration, Ping Distance and Correction Period, Item Icons and Direction Indicator, Player Info
// and Team Color, Ping Size), the "Ping Channel" field under the list at `50 + 25 * rows`, and
// Done in the footer, drawn with the game's own widgets. Opened in the world by the Open Settings
// key and `/pingwheel config` (a mod screen), and over Options > Mods > Ping Wheel by its
// Configure... button and by Mod Menu's Configure, which goes back to Mod Menu. Every change is
// kept at once; closing saves the config (`ClientConfig.HANDLER.save`). Spliced into game.rs as
// `pingwheel_screen`.
//
// Ping Wheel by Luken (https://github.com/LukenSkyne/Minecraft-Ping-Wheel), MIT.

use super::pingwheel_glue as glue;
use super::pingwheel_lang::{t, tf};
use super::{Game, Screen as GameScreen};
use horizon_client::hud::{HudAtlas, HudBatch, rgba};
use horizon_client::mod_api::NativeScreenView;
use horizon_client::options_ui::Page;
use horizon_client::text::Text;
use horizon_client::ui::{self, EditBox};
use pingwheel_core::config::{ClientConfig, MAX_CORRECTION_PERIOD, MAX_PING_DISTANCE, MAX_PING_DURATION, PlayerInfoMode};
use pingwheel_core::MAX_CHANNEL_LENGTH;
use std::cell::RefCell;
use std::time::Instant;

/// Mod Menu's Configure: `native_settings_click(id, MODMENU_CONFIGURE)` after it opened the page.
pub(super) const MODMENU_CONFIGURE: usize = 0x4D4D_4346;

const GRAY: u32 = 0xFFA0_A0A0;
const SHIFT: u32 = 1;
const CONTROL: u32 = 2;

/// Where the screen is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    /// In the world, a screen of the mod's own (no parent: closing goes back to the game).
    World,
    /// Over the mod's page in Options > Mods (row `row`); `modmenu`: Mod Menu opened it and
    /// closing goes back to it.
    Page { row: usize, modmenu: bool },
}

/// The list's options, in `addOptions`' order.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Opt {
    Volume,
    Duration,
    Distance,
    Correction,
    ItemIcons,
    Direction,
    PlayerInfo,
    TeamColor,
    Size,
}

const ROWS: [(Opt, Option<Opt>); 5] = [
    (Opt::Volume, Some(Opt::Duration)),
    (Opt::Distance, Some(Opt::Correction)),
    (Opt::ItemIcons, Some(Opt::Direction)),
    (Opt::PlayerInfo, Some(Opt::TeamColor)),
    (Opt::Size, None),
];

/// What can have the focus (Tab's order: the list, Done, then the field added last).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum W {
    Opt(Opt),
    Done,
    Field,
}

fn widgets() -> Vec<W> {
    let mut v: Vec<W> = ROWS.iter().flat_map(|(a, b)| std::iter::once(W::Opt(*a)).chain(b.map(W::Opt))).collect();
    v.push(W::Done);
    v.push(W::Field);
    v
}

impl Opt {
    /// A slider's whole steps (`OptionInstance.IntRange(min / step, max / step)`).
    fn range(self) -> Option<(i32, i32)> {
        Some(match self {
            Opt::Volume => (0, 100),
            Opt::Duration => (1, MAX_PING_DURATION),
            Opt::Distance => (0, MAX_PING_DISTANCE / 16),
            Opt::Correction => (1, (MAX_CORRECTION_PERIOD / 0.1f32) as i32),
            Opt::Size => (4, 30),
            _ => return None,
        })
    }
    fn get(self, c: &ClientConfig) -> i32 {
        match self {
            Opt::Volume => c.ping_volume,
            Opt::Duration => c.ping_duration,
            Opt::Distance => c.ping_distance / 16,
            Opt::Correction => (c.correction_period / 0.1f32) as i32,
            Opt::Size => c.ping_size / 10,
            _ => 0,
        }
    }
    fn set(self, c: &mut ClientConfig, v: i32) {
        match self {
            Opt::Volume => c.ping_volume = v,
            Opt::Duration => c.ping_duration = v,
            Opt::Distance => c.ping_distance = v * 16,
            Opt::Correction => c.correction_period = v as f32 * 0.1f32,
            Opt::Size => c.ping_size = v * 10,
            _ => {}
        }
    }
    /// A cycle button pressed (`back`: Shift, or the wheel up).
    fn cycle(self, c: &mut ClientConfig, back: bool) {
        match self {
            Opt::ItemIcons => c.item_icon_visible = !c.item_icon_visible,
            Opt::Direction => c.direction_indicator_visible = !c.direction_indicator_visible,
            Opt::PlayerInfo => c.player_info_mode = c.player_info_mode.cycle(back),
            Opt::TeamColor => c.team_color_mode = c.team_color_mode.cycle(back),
            _ => {}
        }
    }
}

/// `CommonComponents.OPTION_OFF` and the game's other texts.
fn vanilla(key: &str) -> String {
    horizon_client::lang::get(key)
}

fn decimal() -> &'static str {
    if super::pingwheel_lang::russian() { "," } else { "." }
}

/// The widget's message: a slider's formatter, or "Caption: value" for a cycle button.
fn message(o: Opt, c: &ClientConfig) -> Text {
    let unit = |key: &str, v: String| tf(key, &[&v]);
    let s = match o {
        Opt::Volume => {
            let v = if c.ping_volume == 0 { vanilla("options.off") } else { unit("unit.pingwheel.percent", c.ping_volume.to_string()) };
            tf("settings.pingwheel.ping_volume", &[&v])
        }
        Opt::Duration => {
            let v = if c.ping_duration >= MAX_PING_DURATION { t("value.pingwheel.infinite") } else { unit("unit.pingwheel.seconds", c.ping_duration.to_string()) };
            tf("settings.pingwheel.ping_duration", &[&v])
        }
        Opt::Distance => {
            let v = if c.ping_distance == 0 {
                t("value.pingwheel.hidden")
            } else if c.ping_distance >= MAX_PING_DISTANCE {
                t("value.pingwheel.infinite")
            } else {
                unit("unit.pingwheel.meters", c.ping_distance.to_string())
            };
            tf("settings.pingwheel.ping_distance", &[&v])
        }
        Opt::Correction => {
            let v = if c.correction_period >= MAX_CORRECTION_PERIOD {
                t("value.pingwheel.infinite")
            } else {
                unit("unit.pingwheel.seconds", format!("{:.1}", c.correction_period).replace('.', decimal()))
            };
            tf("settings.pingwheel.correction_period", &[&v])
        }
        Opt::Size => tf("settings.pingwheel.ping_size", &[&unit("unit.pingwheel.percent", c.ping_size.to_string())]),
        Opt::ItemIcons | Opt::Direction => {
            let (key, on) = if o == Opt::ItemIcons {
                ("settings.pingwheel.item_icon_visible", c.item_icon_visible)
            } else {
                ("settings.pingwheel.direction_indicator_visible", c.direction_indicator_visible)
            };
            return Text::option_name_value(Text::literal(t(key)), Text::translate(if on { "options.on" } else { "options.off" }, &[]));
        }
        Opt::PlayerInfo => {
            return Text::option_name_value(Text::literal(t("settings.pingwheel.player_info_mode")), Text::literal(t(&format!("value.pingwheel.{}", c.player_info_mode.lower()))));
        }
        Opt::TeamColor => {
            return Text::option_name_value(Text::literal(t("settings.pingwheel.team_color_mode")), Text::literal(t(&format!("value.pingwheel.{}", c.team_color_mode.lower()))));
        }
    };
    Text::literal(s)
}

/// `IntRangeBase.toSliderValue`.
fn to_slider(v: i32, (lo, hi): (i32, i32)) -> f32 {
    if v <= lo {
        return 0.0;
    }
    if v >= hi {
        return 1.0;
    }
    ((f64::from(v) + 0.5 - f64::from(lo)) / (f64::from(hi) + 1.0 - f64::from(lo))) as f32
}

/// `IntRangeBase.fromSliderValue`.
fn from_slider(s: f32, (lo, hi): (i32, i32)) -> i32 {
    let s = if s >= 1.0 { 0.99999f32 } else { s.max(0.0) };
    (f64::from(lo) + f64::from(s) * (f64::from(hi) + 1.0 - f64::from(lo))).floor() as i32
}

// ---- the screen ---------------------------------------------------------------------------------

/// What the screen asks of the next frame, which has the game to change.
pub(super) enum Request {
    Click,
    Paste,
    Copy(String),
}

struct State {
    mode: Mode,
    focus: Option<W>,
    /// The slider held by the pointer.
    dragging: Option<Opt>,
    field: EditBox,
    scroll: f32,
    /// Mouse buttons whose press the screen took, so their release is taken too.
    taken: u8,
    close: bool,
    requests: Vec<Request>,
    opened: Instant,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

fn with<R>(f: impl FnOnce(&mut State) -> R) -> Option<R> {
    STATE.with(|s| s.try_borrow_mut().ok().and_then(|mut s| s.as_mut().map(f)))
}

pub(super) fn is_open() -> bool {
    STATE.with(|s| s.try_borrow().map(|s| s.is_some()).unwrap_or(true))
}

fn new_state(mode: Mode) -> State {
    let mut field = EditBox::new(0.0, 0.0, 200.0, 20.0);
    field.max_length = MAX_CHANNEL_LENGTH;
    field.set_value(&glue::channel());
    field.focused = false;
    State { mode, focus: None, dragging: None, field, scroll: 0.0, taken: 0, close: false, requests: Vec::new(), opened: Instant::now() }
}

/// The mod's row in Options > Mods, while the options show it.
fn page(game: &Game<'_>) -> Option<usize> {
    if game.screen != GameScreen::Options {
        return None;
    }
    match game.ui.options.page() {
        Page::NativeMod(i) if game.settings.mods.installed.get(i).is_some_and(|m| m.id == glue::ID) => Some(i),
        _ => None,
    }
}

/// The key or `/pingwheel config`: the screen in the world.
pub(super) fn open_in_world(game: &mut Game<'_>) {
    if game.native_mod_screen_open(glue::ID) {
        STATE.with(|s| *s.borrow_mut() = Some(new_state(Mode::World)));
    }
}

/// Configure... on the mod's page, or Mod Menu's Configure.
pub(super) fn open_over_options(game: &Game<'_>, modmenu: bool) {
    let Some(row) = page(game) else { return };
    STATE.with(|s| *s.borrow_mut() = Some(new_state(Mode::Page { row, modmenu })));
}

/// The screen is up over the mod's page: the options stay hidden.
pub(super) fn covers(game: &Game<'_>) -> bool {
    with(|s| matches!(s.mode, Mode::Page { .. })).unwrap_or(false) && page(game).is_some()
}

/// The screen is the one under `screen` now.
fn shown(game: &Game<'_>, screen: &NativeScreenView<'_>) -> bool {
    match with(|s| s.mode) {
        Some(Mode::World) => screen.id == "mod" && game.native_mod_screen() == Some(glue::ID),
        Some(Mode::Page { row, .. }) => screen.id == "options" && page(game) == Some(row),
        None => false,
    }
}

/// Once a frame: what the screen asked for, and its closing (Done, Escape, or the game taking
/// its place), which saves the config as `onClose` does.
pub(super) fn frame(game: &mut Game<'_>) {
    let Some(mode) = with(|s| s.mode) else { return };
    for r in with(|s| std::mem::take(&mut s.requests)).unwrap_or_default() {
        match r {
            Request::Click => game.audio.play_button_click(),
            Request::Paste => {
                let text = horizon_client::clipboard::Clipboard::get(&mut game.clipboard);
                with(|s| insert(s, &text));
            }
            Request::Copy(text) => {
                if !text.is_empty() {
                    horizon_client::clipboard::Clipboard::set(&mut game.clipboard, &text);
                }
            }
        }
    }
    let close = with(|s| s.close).unwrap_or(false);
    let gone = match mode {
        Mode::World => game.native_mod_screen() != Some(glue::ID),
        Mode::Page { row, .. } => page(game) != Some(row),
    };
    if !close && !gone {
        return;
    }
    STATE.with(|s| *s.borrow_mut() = None);
    glue::save(game);
    if gone {
        return;
    }
    match mode {
        Mode::World => game.native_mod_screen_close(glue::ID),
        Mode::Page { row, modmenu: false } => game.native_settings_refresh(row),
        // Back to the parent, Mod Menu, the way Escape leaves the options.
        Mode::Page { modmenu: true, .. } => {
            let events = game.ui.options.back(&mut game.settings);
            let _ = game.handle_option_events(events);
        }
    }
}

// ---- layout -------------------------------------------------------------------------------------

/// `HeaderAndFooterLayout`'s list: between the 33-pixel header and footer.
fn list_bounds(h: f32) -> (f32, f32) {
    (33.0, h - 33.0)
}

/// Each option's widget, scrolled: x, y, width.
fn layout(w: f32, scroll: f32) -> Vec<(Opt, [f32; 4])> {
    let cx = (w as i32 / 2) as f32;
    let mut out = Vec::new();
    for (i, (a, b)) in ROWS.iter().enumerate() {
        let y = 33.0 + 4.0 + 25.0 * i as f32 - scroll;
        out.push((*a, [cx - 155.0, y, 150.0, 20.0]));
        if let Some(b) = b {
            out.push((*b, [cx + 5.0, y, 150.0, 20.0]));
        }
    }
    out
}

fn content_height() -> f32 {
    ROWS.len() as f32 * 25.0 + 4.0
}

fn done_rect(w: f32, h: f32) -> [f32; 4] {
    [(w as i32 / 2) as f32 - 100.0, h - 27.0, 200.0, 20.0]
}

/// `extractRenderState`: the field's top, `50 + 25 * children` (the list's rows).
fn field_rect(w: f32) -> [f32; 4] {
    [(w as i32 / 2) as f32 - 100.0, 50.0 + 25.0 * ROWS.len() as f32, 200.0, 20.0]
}

fn inside(p: (f32, f32), r: [f32; 4]) -> bool {
    ui::hovered(p, r[0], r[1], r[2], r[3])
}

/// The option under the pointer, inside the list.
fn option_at(p: (f32, f32), w: f32, h: f32, scroll: f32) -> Option<(Opt, [f32; 4])> {
    let (top, bottom) = list_bounds(h);
    if p.1 < top || p.1 >= bottom {
        return None;
    }
    layout(w, scroll).into_iter().find(|(_, r)| inside(p, *r))
}

// ---- drawing ------------------------------------------------------------------------------------

fn in_world(game: &Game<'_>, mode: Mode) -> bool {
    match mode {
        Mode::World => true,
        Mode::Page { .. } => game.ui.options.in_world,
    }
}

/// `getChannelPlaceholder`: "Global", or the team context it follows, in dark grey italics.
fn placeholder(game: &Game<'_>) -> Option<Text> {
    game.native_player()?;
    let text = if game.player_team().is_some() {
        tf("settings.pingwheel.channel.placeholder", &[&t("value.pingwheel.vanilla_team")])
    } else {
        t("value.pingwheel.global")
    };
    Some(Text::colored(text, 0x55_5555).italic())
}

/// Lines of a tooltip as `Tooltip.create` splits them (170 pixels).
fn tooltip_lines(atlas: &HudAtlas, text: &str) -> Vec<Text> {
    atlas.font.split_lines(&Text::literal(text), 170.0).into_iter().map(|(l, _)| l).collect()
}

pub(super) fn render(game: &Game<'_>, batch: &mut HudBatch, screen: &NativeScreenView<'_>) {
    if !shown(game, screen) {
        return;
    }
    let atlas = &game.gui.atlas;
    let (w, h) = (batch.width, batch.height);
    let mouse = (screen.mouse[0], screen.mouse[1]);
    let cfg = glue::config(|c| c.clone());
    let Some((mode, focus, dragging, scroll)) = with(|s| (s.mode, s.focus, s.dragging, s.scroll)) else { return };
    let world = in_world(game, mode);
    let (top, bottom) = list_bounds(h);
    // `Screen.renderBackground`, the header and footer and the list's background.
    if world {
        game.native_apply_screen_blur();
    }
    ui::menu_background(batch, atlas, 0.0, 0.0, w, h, world);
    ui::list_background(batch, atlas, top, bottom, world);
    ui::separators(batch, atlas, top, bottom, world);
    let title = Text::literal(t("settings.pingwheel.title"));
    let tw = atlas.font.width_text(&title);
    batch.text_styled(atlas, &title, ((w as i32 - tw as i32) / 2) as f32, 12.0, u32::MAX, true);

    // The list, cut to its area.
    let mut tooltip: Option<(Vec<Text>, [f32; 4])> = None;
    let start = batch.vertices.len();
    let over_list = mouse.1 >= top && mouse.1 < bottom;
    for (o, r) in layout(w, scroll) {
        if r[1] + r[3] < top || r[1] > bottom {
            continue;
        }
        let hover = over_list && inside(mouse, r);
        let focused = focus == Some(W::Opt(o));
        let label = message(o, &cfg);
        if let Some(range) = o.range() {
            let value = to_slider(o.get(&cfg), range);
            ui::slider_ext(batch, atlas, r[0], r[1], r[2], &label, value, hover, focused, false, true, dragging == Some(o));
        } else {
            ui::button(batch, atlas, r[0], r[1], r[2], r[3], &label, true, hover || focused);
        }
        // Player Info's tooltip in its Hold mode.
        if o == Opt::PlayerInfo && cfg.player_info_mode == PlayerInfoMode::Hold && (hover || focused) {
            let name = vanilla("key.playerlist");
            let key = horizon_client::keybinds::display_name(game.settings.keys.get("key.playerlist")).plain();
            tooltip = Some((tooltip_lines(atlas, &tf("settings.pingwheel.player_info_mode.hold.tooltip", &[&name, &key])), r));
        }
    }
    batch.clip_since(start, 0.0, top, w, bottom);
    if content_height() > bottom - top {
        ui::scrollbar(batch, atlas, (w as i32 / 2 + 155 + 6) as f32, top, bottom, content_height(), scroll);
    }
    let done = done_rect(w, h);
    ui::action_button(batch, atlas, horizon_client::widgets::Action::Done, done[0], done[1], done[2], done[3], &Text::translate("gui.done", &[]), true, inside(mouse, done) || focus == Some(W::Done));

    // The channel field and its label, over everything else.
    let fr = field_rect(w);
    let label = Text::literal(t("settings.pingwheel.channel"));
    batch.text_styled(atlas, &label, fr[0], fr[1] - 12.0, c(GRAY), true);
    let field_focused = focus == Some(W::Field);
    let empty = with(|s| {
        s.field.x = fr[0];
        s.field.y = fr[1];
        s.field.width = fr[2];
        s.field.height = fr[3];
        s.field.focused = field_focused;
        s.field.draw(batch, atlas, None);
        s.field.value.is_empty()
    })
    .unwrap_or(true);
    if empty && !field_focused
        && let Some(p) = placeholder(game)
    {
        batch.text_styled(atlas, &p, fr[0] + 4.0, fr[1] + 6.0, u32::MAX, true);
    }
    if inside(mouse, fr) && !field_focused {
        ui::tooltip(batch, atlas, &tooltip_lines(atlas, &t("settings.pingwheel.channel.tooltip")), mouse);
    } else if let Some((lines, r)) = tooltip {
        ui::tooltip_menu(batch, atlas, &lines, mouse, r);
    }
}

fn c(argb: u32) -> u32 {
    rgba((argb >> 16) as u8, (argb >> 8) as u8, argb as u8, (argb >> 24) as u8)
}

// ---- input --------------------------------------------------------------------------------------

fn size(screen: &NativeScreenView<'_>) -> (f32, f32) {
    (screen.rect[2].floor(), screen.rect[3].floor())
}

fn bit(button: i32) -> u8 {
    1 << button.clamp(0, 7)
}

/// The field's text changed: the channel follows it (`setResponder(config::setChannel)`).
fn changed(s: &State) {
    let v = s.field.value.clone();
    glue::config(|c| glue::set_channel(c, &v));
}

fn insert(s: &mut State, text: &str) {
    if s.focus != Some(W::Field) {
        return;
    }
    let text: String = text.chars().filter(|c| !c.is_control() && *c != '§').collect();
    s.field.insert_text(&text);
    s.field.reset_blink();
    changed(s);
}

fn set_slider(o: Opt, p: (f32, f32), r: [f32; 4]) {
    let Some(range) = o.range() else { return };
    let v = from_slider(ui::slider_value(r[0], r[2], p.0), range);
    glue::config(|c| o.set(c, v));
}

pub(super) fn pressed(game: &Game<'_>, screen: &NativeScreenView<'_>, button: i32, modifiers: u32) -> bool {
    if !shown(game, screen) {
        return false;
    }
    let (w, h) = size(screen);
    let p = (screen.mouse[0], screen.mouse[1]);
    with(|s| {
        s.taken |= bit(button);
        if button != 0 {
            return;
        }
        if inside(p, field_rect(w)) {
            s.focus = Some(W::Field);
            let atlas = &game.gui.atlas;
            s.field.focused = true;
            s.field.click(atlas, p.0, modifiers & SHIFT != 0, false);
            return;
        }
        if inside(p, done_rect(w, h)) {
            s.focus = Some(W::Done);
            s.requests.push(Request::Click);
            s.close = true;
            return;
        }
        if let Some((o, r)) = option_at(p, w, h, s.scroll) {
            s.focus = Some(W::Opt(o));
            if o.range().is_some() {
                s.dragging = Some(o);
                set_slider(o, p, r);
            } else {
                s.requests.push(Request::Click);
                glue::config(|c| o.cycle(c, modifiers & SHIFT != 0));
            }
            return;
        }
        s.focus = None;
    });
    true
}

pub(super) fn released(game: &Game<'_>, screen: &NativeScreenView<'_>, button: i32) -> bool {
    let was = with(|s| {
        let was = s.taken & bit(button) != 0;
        s.taken &= !bit(button);
        // `AbstractSliderButton.onRelease`: the click sound once the handle is let go.
        if button == 0 && s.dragging.take().is_some() {
            s.requests.push(Request::Click);
        }
        was
    })
    .unwrap_or(false);
    was || shown(game, screen)
}

pub(super) fn dragged(game: &Game<'_>, screen: &NativeScreenView<'_>) -> bool {
    if !shown(game, screen) {
        return false;
    }
    let (w, _) = size(screen);
    let p = (screen.mouse[0], screen.mouse[1]);
    with(|s| {
        if let Some(o) = s.dragging
            && let Some((_, r)) = layout(w, s.scroll).into_iter().find(|(x, _)| *x == o)
        {
            set_slider(o, p, r);
        }
        if s.focus == Some(W::Field) {
            s.field.drag_to(&game.gui.atlas, p.0);
        }
    });
    true
}

/// The wheel: a cycle button under the pointer steps (`CycleButton.mouseScrolled`, up goes
/// back); elsewhere the list scrolls.
pub(super) fn scrolled(game: &Game<'_>, screen: &NativeScreenView<'_>, scroll: [f64; 2]) -> bool {
    if !shown(game, screen) {
        return false;
    }
    let (w, h) = size(screen);
    let p = (screen.mouse[0], screen.mouse[1]);
    with(|s| {
        if scroll[1] == 0.0 {
            return;
        }
        if let Some((o, _)) = option_at(p, w, h, s.scroll)
            && o.range().is_none()
        {
            glue::config(|c| o.cycle(c, scroll[1] > 0.0));
            return;
        }
        let (top, bottom) = list_bounds(h);
        let max = (content_height() - (bottom - top)).max(0.0);
        s.scroll = (s.scroll - scroll[1] as f32 * 10.0).clamp(0.0, max);
    });
    true
}

pub(super) fn key_pressed(game: &Game<'_>, screen: &NativeScreenView<'_>, key: &str, modifiers: u32) -> bool {
    if !shown(game, screen) {
        return false;
    }
    let (control, shift) = (modifiers & CONTROL != 0, modifiers & SHIFT != 0);
    with(|s| {
        if key == "key.keyboard.escape" {
            s.close = true;
            return;
        }
        if key == "key.keyboard.tab" {
            let all = widgets();
            let i = s.focus.and_then(|f| all.iter().position(|w| *w == f));
            let n = all.len();
            let next = match i {
                None if shift => n - 1,
                None => 0,
                Some(i) if shift => (i + n - 1) % n,
                Some(i) => (i + 1) % n,
            };
            s.focus = Some(all[next]);
            return;
        }
        match s.focus {
            Some(W::Field) => {
                let edit = match key {
                    "key.keyboard.v" if control => Some(Request::Paste),
                    "key.keyboard.c" if control => Some(Request::Copy(s.field.selected_text())),
                    "key.keyboard.x" if control => {
                        let text = s.field.selected_text();
                        s.field.delete_chars(0);
                        changed(s);
                        Some(Request::Copy(text))
                    }
                    _ => {
                        if let Some(code) = horizon_client::keybinds::key_code(key) {
                            s.field.key(code, control, shift);
                            s.field.reset_blink();
                            changed(s);
                        }
                        None
                    }
                };
                if let Some(e) = edit {
                    s.requests.push(e);
                }
            }
            Some(W::Done) if matches!(key, "key.keyboard.enter" | "key.keyboard.space" | "key.keyboard.keypad.enter") => {
                s.requests.push(Request::Click);
                s.close = true;
            }
            Some(W::Opt(o)) => {
                if let Some(range) = o.range() {
                    // A focused slider steps with the arrows.
                    let step = match key {
                        "key.keyboard.left" => -1,
                        "key.keyboard.right" => 1,
                        _ => 0,
                    };
                    if step != 0 {
                        glue::config(|c| {
                            let v = (o.get(c) + step).clamp(range.0, range.1);
                            o.set(c, v);
                        });
                    }
                } else if matches!(key, "key.keyboard.enter" | "key.keyboard.space" | "key.keyboard.keypad.enter") {
                    s.requests.push(Request::Click);
                    glue::config(|c| o.cycle(c, shift));
                }
            }
            _ => {}
        }
    });
    true
}

pub(super) fn key_released(game: &Game<'_>, screen: &NativeScreenView<'_>) -> bool {
    shown(game, screen)
}

pub(super) fn char_typed(game: &Game<'_>, screen: &NativeScreenView<'_>, ch: char) -> bool {
    if !shown(game, screen) {
        return false;
    }
    with(|s| {
        // The key that opened the screen types too: not in its first moment.
        if s.opened.elapsed().as_millis() >= 100 {
            insert(s, &ch.to_string());
        }
    });
    true
}

