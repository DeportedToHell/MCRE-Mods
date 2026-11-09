// ShulkerBoxTooltip for Horizon: its config screen (`impl/config/gui`:
// `ConfigScreen`, `ConfigCategoryTab`, `ConfigEntryList` and the entries),
// over the Options screen while the mod's page is open. The page's first
// button opens it, and so does Mod Menu's Configure. The tabs of the
// categories, a list of options per tab (Yes/No, enum and key buttons, number
// and colour fields, each with Reset "R" and Undo "U"), the tooltips, and the
// footer's Cancel / Discard Changes and Save & Quit; the "Changes Not Saved"
// and "Restart Required" confirmations. Also writes the icon and Mod Menu's
// metadata into the mod's data folder.
//
// The screen hooks only read the game: what needs to change it (a click's
// sound, the clipboard, closing, quitting) waits for the next frame.
use super::shulkerboxtooltip as sbt;
use super::shulkerboxtooltip_glue as glue;
use super::{Game, Screen};
use horizon_client::clipboard::Clipboard as _;
use horizon_client::hud::HudBatch;
use horizon_client::mod_api::NativeScreenView;
use horizon_client::options_ui::Page;
use horizon_client::text::{self, Style, Text};
use horizon_client::ui::{self, EditBox};
use sbt::{Category, ColorGroup, Config, Kind, Node, Value};
use std::cell::RefCell;

/// Mod Menu's Configure: `native_settings_click(id, MODMENU_CONFIGURE)` after it opened the mod's page.
pub const MODMENU_CONFIGURE: usize = 0x4D4D_4346;

/// GLFW's modifier bits.
const SHIFT: u32 = 1;
const CONTROL: u32 = 2;

/// `TabNavigationBar`'s height, `HeaderAndFooterLayout`'s footer, the list's rows.
const TAB_H: f32 = 24.0;
const FOOTER: f32 = 33.0;
const ITEM: f32 = 24.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Page,
    ModMenu,
}

/// A `ConfirmScreen` over the config screen.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Confirm {
    /// "Changes Not Saved".
    Quit,
    /// "Restart Required", after saving.
    Restart,
}

/// A row of a tab's list.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Row {
    /// `PrefixTextConfigEntry`: the text above node `n`.
    Prefix(usize),
    /// `CategoryTitleConfigEntry`: a colour group's heading.
    Title(ColorGroup),
    /// A `ValueConfigEntry` of node `n`.
    Value(usize),
}

/// The screen's widgets.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum W {
    Tab(usize),
    /// The value's button (Yes/No, an enum, a key).
    Value(usize),
    /// The number or colour field.
    Field(usize),
    /// The colour swatch beside a colour field (inactive).
    Swatch(usize),
    Reset(usize),
    Undo(usize),
    Cancel,
    Save,
    ConfirmYes,
    ConfirmNo,
}

#[derive(Clone, Copy)]
struct Placed {
    w: W,
    rect: [f32; 4],
    /// In the scrolling list (clipped to it).
    list: bool,
}

/// Something the next frame does with the game.
enum Request {
    Click,
    Paste,
    Copy(String),
}

struct State {
    open: Option<Mode>,
    /// The screen goes at the next frame.
    close: bool,
    /// Save & Quit: the new configuration, kept at the next frame.
    save: Option<Config>,
    /// "Exit Minecraft".
    quit_game: bool,
    nodes: Vec<Node>,
    /// The configuration in effect when the editing started (`savedConfig`).
    active: Config,
    /// The values being edited (`ValueConfigNode.editingValue`).
    editing: Vec<Value>,
    tab: usize,
    scroll: [ui::Scroll; 5],
    /// The key button waiting for a key (`selectedKeyNode`).
    selected_key: Option<usize>,
    /// The number and colour fields, by node.
    boxes: Vec<Option<EditBox>>,
    /// The focused field's node.
    focus: Option<usize>,
    confirm: Option<Confirm>,
    /// Mouse buttons whose press the screen took.
    taken: u8,
    requests: Vec<Request>,
    /// The icon and the metadata were written this run.
    written: bool,
    /// The field under a press keeps the drag.
    dragging_field: bool,
}

impl Default for State {
    fn default() -> Self {
        State {
            open: None,
            close: false,
            save: None,
            quit_game: false,
            nodes: Vec::new(),
            active: Config::default(),
            editing: Vec::new(),
            tab: 0,
            scroll: [ui::Scroll::default(); 5],
            selected_key: None,
            boxes: Vec::new(),
            focus: None,
            confirm: None,
            taken: 0,
            requests: Vec::new(),
            written: false,
            dragging_field: false,
        }
    }
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|s| f(&mut s.borrow_mut()))
}

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
        Page::NativeMod(i) if game.settings.mods.installed.get(i).is_some_and(|m| m.id == sbt::ID) => Some(i),
        _ => None,
    }
}

pub(super) fn covers(game: &Game<'_>) -> bool {
    with(|s| s.open.is_some()) && page(game).is_some()
}

fn over(game: &Game<'_>, screen: &NativeScreenView<'_>) -> bool {
    screen.id == "options" && covers(game)
}

// ---- editing ------------------------------------------------------------------------------

impl State {
    /// `ConfigScreen.init`: every value back to what is in effect
    /// (`root.resetToActive`), the fields with their values.
    fn reset_to_active(&mut self) {
        self.editing = self.active.values.clone();
        self.selected_key = None;
        self.focus = None;
        self.boxes = self.nodes.iter().map(|_| None).collect();
        for i in 0..self.nodes.len() {
            if matches!(self.nodes[i].kind, Kind::PositiveInt | Kind::Color) {
                let mut b = EditBox::new(0.0, 0.0, 100.0, 18.0);
                b.max_length = 32;
                b.focused = false;
                b.set_value(&self.field_text(i));
                b.set_cursor(0, false);
                self.boxes[i] = Some(b);
            }
        }
    }

    /// What field `i` shows for its value (`displayValue`: `#` and the
    /// colour in hexadecimal, unpadded).
    fn field_text(&self, i: usize) -> String {
        match (&self.nodes[i].kind, &self.editing[i]) {
            (Kind::Color, Value::Color(c)) => format!("#{:x}", *c as u32),
            (_, v) => v.as_int().to_string(),
        }
    }

    fn changed(&self, i: usize) -> bool {
        self.editing[i] != self.active.values[i]
    }
    fn is_default(&self, i: usize) -> bool {
        self.editing[i] == self.nodes[i].default
    }
    fn error(&self, i: usize) -> Option<&'static str> {
        self.nodes[i].validate(&self.editing[i])
    }
    fn any_changed(&self) -> bool {
        (0..self.nodes.len()).any(|i| self.changed(i))
    }
    fn editing_config(&self) -> Config {
        Config { values: self.editing.clone() }
    }
    fn valid(&self) -> bool {
        self.editing_config().validate().is_none()
    }
    fn tab_changed(&self, cat: Category) -> bool {
        (0..self.nodes.len()).any(|i| self.nodes[i].category == cat && self.changed(i))
    }
    fn tab_error(&self, cat: Category) -> bool {
        (0..self.nodes.len()).any(|i| self.nodes[i].category == cat && self.error(i).is_some())
    }

    /// `ValueConfigEntry.setValue` and `ConfigScreen.refresh`: a field
    /// that holds a valid value shows it as the entry writes it.
    fn set(&mut self, i: usize, v: Value) {
        self.editing[i] = v;
        self.refresh_field(i);
    }

    fn refresh_field(&mut self, i: usize) {
        if self.error(i).is_none() {
            let text = self.field_text(i);
            if let Some(b) = self.boxes[i].as_mut()
                && b.value != text
            {
                b.set_value(&text);
            }
        }
    }

    /// The field's text changed (`onInputChange`).
    fn field_changed(&mut self, i: usize) {
        let Some(text) = self.boxes[i].as_ref().map(|b| b.value.clone()) else { return };
        match self.nodes[i].kind {
            // `Integer.parseInt`; what it can't read leaves the value.
            Kind::PositiveInt => {
                if let Ok(v) = text.parse::<i32>() {
                    self.set(i, Value::Int(v));
                }
            }
            Kind::Color => {
                let parsed = match text.strip_prefix('#') {
                    Some(_) if text.len() > 7 => None,
                    Some(hex) => i64::from_str_radix(hex, 16).ok(),
                    None => i64::from_str_radix(&text, 16).ok(),
                };
                let v = parsed.map_or(-1, |v| v as i32);
                self.set(i, Value::Color(v));
            }
            _ => {}
        }
    }

    /// The rows of tab `tab`'s list (`ConfigCategoryTab`'s entries).
    fn rows(&self, tab: usize) -> Vec<Row> {
        let cat = Category::ALL[tab];
        let mut rows = Vec::new();
        let mut group = None;
        for (i, n) in self.nodes.iter().enumerate().filter(|(_, n)| n.category == cat) {
            if n.group.is_some() && n.group != group {
                group = n.group;
                rows.push(Row::Title(n.group.unwrap_or(ColorGroup::Default)));
            }
            if n.prefix_key().is_some_and(|k| glue::has_text(&k)) {
                rows.push(Row::Prefix(i));
            }
            rows.push(Row::Value(i));
        }
        rows
    }

    /// `ConfigScreen.onClose`: straight back when nothing changed, else
    /// "Changes Not Saved".
    fn quit(&mut self) {
        if self.any_changed() {
            self.confirm = Some(Confirm::Quit);
            self.focus = None;
        } else {
            self.close = true;
        }
    }

    /// `ConfigScreen.saveAndQuit`.
    fn save_and_quit(&mut self) {
        let restart = (0..self.nodes.len()).any(|i| self.nodes[i].restart && self.changed(i));
        let config = self.editing_config();
        self.active = config.clone();
        self.save = Some(config);
        if restart {
            self.confirm = Some(Confirm::Restart);
            self.focus = None;
        } else {
            self.close = true;
        }
    }
}

/// Opens the screen over the mod's page: the page's first button, or Mod
/// Menu's Configure.
pub(super) fn open(game: &Game<'_>, from_mod_menu: bool) {
    if page(game).is_none() {
        return;
    }
    let active = glue::config();
    with(|s| {
        *s = State {
            open: Some(if from_mod_menu { Mode::ModMenu } else { Mode::Page }),
            written: s.written,
            nodes: sbt::nodes(),
            active,
            ..State::default()
        };
        s.reset_to_active();
    });
}

/// Once a frame: the data files, clicks, the clipboard, saving, closing.
pub(super) fn frame(game: &mut Game<'_>) {
    if !with(|s| std::mem::replace(&mut s.written, true)) {
        write_data(game);
    }
    for r in with(|s| std::mem::take(&mut s.requests)) {
        match r {
            Request::Click => game.audio.play_button_click(),
            Request::Paste => {
                let text = game.clipboard.get();
                with(|s| {
                    if let Some(i) = s.focus
                        && let Some(b) = s.boxes[i].as_mut()
                    {
                        let clean: String = text.chars().filter(|c| !c.is_control()).collect();
                        b.insert_text(&clean);
                        s.field_changed(i);
                    }
                });
            }
            Request::Copy(text) => {
                if !text.is_empty() {
                    game.clipboard.set(&text);
                }
            }
        }
    }
    if let Some(config) = with(|s| s.save.take()) {
        glue::store(config);
    }
    if with(|s| std::mem::take(&mut s.quit_game)) {
        let _ = game.begin_closing_save(true);
    }
    let Some(mode) = with(|s| s.open) else {
        return;
    };
    match page(game) {
        None => with(|s| s.open = None),
        Some(i) if with(|s| s.close) => {
            with(|s| {
                s.open = None;
                s.close = false;
                s.confirm = None;
            });
            match mode {
                Mode::Page => game.native_settings_refresh(i),
                Mode::ModMenu => {
                    let events = game.ui.options.back(&mut game.settings);
                    let _ = game.handle_option_events(events);
                }
            }
        }
        Some(_) => {}
    }
}

// ---- layout -------------------------------------------------------------------------------

/// `Mth.roundToward`: up to a multiple of `step`.
fn round_toward(v: f32, step: f32) -> f32 {
    (v / step).ceil() * step
}

struct Layout {
    widgets: Vec<Placed>,
    /// Each row of the list and its top.
    rows: Vec<(Row, f32)>,
    /// The list's rows' left and width (`getRowLeft`, `getRowWidth`).
    row_left: f32,
    row_width: f32,
    top: f32,
    bottom: f32,
    content: f32,
    /// The confirmation's title, message lines and their top.
    confirm: Option<(Text, Vec<Text>, f32)>,
}

fn button_width(atlas: &horizon_client::hud::HudAtlas, key: &str) -> f32 {
    (atlas.font.width_text(&glue::tr(key)) + 6.0).max(20.0)
}

/// The tabs: the first's x and each one's width (`MenuTabBar.arrangeElements`).
fn tabs(w: f32) -> (f32, f32) {
    let tw = round_toward(((w.min(400.0) - 28.0) / 5.0).trunc(), 2.0);
    (round_toward(((w - tw * 5.0) / 2.0).trunc(), 2.0), tw)
}

fn layout(game: &Game<'_>, s: &State) -> Layout {
    let (w, h) = gui_size(game);
    let atlas = &game.gui.atlas;
    let mut widgets = Vec::new();
    if let Some(c) = s.confirm {
        // `ConfirmScreen`: the title, the message (width - 50 wide, centred),
        // the two buttons 150 wide, 8 apart, the buttons 16 lower still.
        let (title, message, yes, no) = match c {
            Confirm::Quit => (
                "shulkerboxtooltip.config.quit.confirm.title",
                "shulkerboxtooltip.config.quit.confirm.warning",
                glue::tr("shulkerboxtooltip.config.quit.confirm"),
                Text::translate("gui.cancel", &[]),
            ),
            Confirm::Restart => (
                "shulkerboxtooltip.config.restart_required.title",
                "shulkerboxtooltip.config.restart_required",
                glue::tr("shulkerboxtooltip.config.exit_minecraft"),
                glue::tr("shulkerboxtooltip.config.ignore_restart"),
            ),
        };
        let _ = (yes, no);
        let lines: Vec<Text> = atlas.font.split(&glue::tr(message), (w - 50.0).floor()).into_iter().take(15).collect();
        let total = 9.0 + 8.0 + lines.len() as f32 * 9.0 + 8.0 + 16.0 + 20.0;
        let y0 = ((h - total) / 2.0).trunc();
        let by = y0 + 17.0 + lines.len() as f32 * 9.0 + 8.0 + 16.0;
        let bx = ((w - 304.0) / 2.0).trunc();
        widgets.push(Placed { w: W::ConfirmYes, rect: [bx, by, 150.0, 20.0], list: false });
        widgets.push(Placed { w: W::ConfirmNo, rect: [bx + 154.0, by, 150.0, 20.0], list: false });
        return Layout {
            widgets,
            rows: Vec::new(),
            row_left: 0.0,
            row_width: 0.0,
            top: 0.0,
            bottom: 0.0,
            content: 0.0,
            confirm: Some((glue::tr(title), lines, y0)),
        };
    }
    let (x0, tw) = tabs(w);
    for i in 0..5 {
        widgets.push(Placed { w: W::Tab(i), rect: [x0 + tw * i as f32, 0.0, tw, TAB_H], list: false });
    }
    let (top, bottom) = (TAB_H, h - FOOTER);
    let row_width = w - 80.0;
    let row_left = (w / 2.0).trunc() - (row_width / 2.0).trunc();
    let rows = s.rows(s.tab);
    let content = rows.len() as f32 * ITEM + 4.0;
    let scroll = s.scroll[s.tab].amount;
    let r_w = button_width(atlas, "shulkerboxtooltip.config.reset_to_default.small");
    let u_w = button_width(atlas, "shulkerboxtooltip.config.undo.small");
    let mut placed_rows = Vec::new();
    for (n, row) in rows.iter().enumerate() {
        let y = top + 2.0 - scroll + n as f32 * ITEM;
        placed_rows.push((*row, y));
        let Row::Value(i) = *row else { continue };
        let cy = y + 2.0;
        let right = row_left + 2.0 + row_width - 4.0;
        let undo = [right - u_w, cy, u_w, 20.0];
        let reset = [undo[0] - r_w - 2.0, cy, r_w, 20.0];
        match s.nodes[i].kind {
            Kind::PositiveInt => {
                let fw = 158.0 - r_w - 2.0 - u_w - 2.0;
                widgets.push(Placed { w: W::Field(i), rect: [reset[0] - fw - 3.0, cy + 1.0, fw, 18.0], list: true });
            }
            Kind::Color => {
                let fw = 138.0 - r_w - 2.0 - u_w - 2.0;
                let fx = reset[0] - fw - 3.0;
                widgets.push(Placed { w: W::Field(i), rect: [fx, cy + 1.0, fw, 18.0], list: true });
                widgets.push(Placed { w: W::Swatch(i), rect: [fx - 18.0 - 2.0, cy + 1.0, 18.0, 18.0], list: true });
            }
            _ => {
                let vw = 160.0 - r_w - 2.0 - u_w - 2.0;
                widgets.push(Placed { w: W::Value(i), rect: [reset[0] - vw - 2.0, cy, vw, 20.0], list: true });
            }
        }
        widgets.push(Placed { w: W::Reset(i), rect: reset, list: true });
        widgets.push(Placed { w: W::Undo(i), rect: undo, list: true });
    }
    let fx = ((w - 408.0) / 2.0).trunc();
    let fy = h - FOOTER + 6.0;
    widgets.push(Placed { w: W::Cancel, rect: [fx, fy, 200.0, 20.0], list: false });
    widgets.push(Placed { w: W::Save, rect: [fx + 208.0, fy, 200.0, 20.0], list: false });
    Layout { widgets, rows: placed_rows, row_left, row_width, top, bottom, content, confirm: None }
}

/// The layout with the tab's scroll kept inside its list.
fn current(game: &Game<'_>) -> Layout {
    with(|s| {
        let l = layout(game, s);
        if l.confirm.is_some() {
            return l;
        }
        let tab = s.tab;
        let before = s.scroll[tab].amount;
        s.scroll[tab].clamp(l.top, l.bottom, l.content);
        if before != s.scroll[tab].amount { layout(game, s) } else { l }
    })
}

fn under(l: &Layout, p: (f32, f32)) -> Option<Placed> {
    l.widgets.iter().copied().find(|pl| inside(p, pl.rect) && (!pl.list || (p.1 >= l.top && p.1 < l.bottom)))
}

/// Whether a widget can be pressed (`active`).
fn active(s: &State, w: W) -> bool {
    match w {
        W::Reset(i) => !s.is_default(i),
        W::Undo(i) => s.changed(i),
        W::Save => s.any_changed() && s.valid(),
        W::Swatch(_) => false,
        _ => true,
    }
}

// ---- drawing ------------------------------------------------------------------------------

fn styled(t: Text, color: u32, italic: bool) -> Text {
    let t = t.with_color(color);
    if italic { t.italic() } else { t }
}

/// The value button's message.
fn value_message(s: &State, i: usize) -> Text {
    match (&s.nodes[i].kind, &s.editing[i]) {
        (Kind::Bool, v) => {
            if v.as_bool() {
                Text::translate("gui.yes", &[]).with_color(text::GREEN)
            } else {
                Text::translate("gui.no", &[]).with_color(text::RED)
            }
        }
        (Kind::Enum(k), v) => glue::tr(&k.value_key(v.as_enum())),
        (Kind::Key, v) => {
            let name = glue::key_name(v.as_key());
            if s.selected_key == Some(i) {
                Text::literal("> ")
                    .with_color(text::YELLOW)
                    .append(name.with_style(&Style { color: Some(text::WHITE), underlined: Some(true), ..Style::default() }))
                    .append(Text::literal(" <").with_color(text::YELLOW))
            } else {
                name
            }
        }
        _ => Text::empty(),
    }
}

/// An entry's tooltip lines: its own (`font.split(tooltip, 350)`) and its error, red.
fn tooltip_lines(game: &Game<'_>, s: &State, i: usize) -> Vec<Text> {
    let atlas = &game.gui.atlas;
    let mut lines = Vec::new();
    if let Some(key) = s.nodes[i].tooltip_key() {
        let text = glue::tr(&key).plain();
        for part in text.split('\n') {
            lines.extend(atlas.font.split(&Text::literal(part.to_owned()), 350.0));
        }
    }
    if let Some(e) = s.error(i) {
        lines.push(glue::tr(e).with_color(text::RED));
    }
    lines
}

pub(super) fn render(game: &Game<'_>, batch: &mut HudBatch, screen: &NativeScreenView<'_>) {
    if !over(game, screen) {
        return;
    }
    let l = current(game);
    let atlas = &game.gui.atlas;
    let (w, h) = (batch.width, batch.height);
    let in_world = game.ui.options.in_world;
    let mouse = (screen.mouse[0], screen.mouse[1]);
    game.native_apply_screen_blur();
    ui::menu_background(batch, atlas, 0.0, 0.0, w, h, in_world);
    let hovered = under(&l, mouse);
    with(|s| {
        if let Some((title, lines, y0)) = &l.confirm {
            let tw = atlas.font.width_text(title);
            batch.text_styled(atlas, title, ((w - tw) / 2.0).trunc(), *y0, u32::MAX, true);
            for (n, line) in lines.iter().enumerate() {
                let lw = atlas.font.width_text(line);
                batch.text_styled(atlas, line, ((w - lw) / 2.0).trunc(), y0 + 17.0 + n as f32 * 9.0, u32::MAX, true);
            }
            for pl in &l.widgets {
                let [x, y, bw, bh] = pl.rect;
                let label = match (s.confirm, pl.w) {
                    (Some(Confirm::Quit), W::ConfirmYes) => glue::tr("shulkerboxtooltip.config.quit.confirm"),
                    (Some(Confirm::Quit), _) => Text::translate("gui.cancel", &[]),
                    (_, W::ConfirmYes) => glue::tr("shulkerboxtooltip.config.exit_minecraft"),
                    _ => glue::tr("shulkerboxtooltip.config.ignore_restart"),
                };
                ui::button(batch, atlas, x, y, bw, bh, &label, true, hovered.is_some_and(|hv| hv.w == pl.w));
            }
            return;
        }

        // The list (`ConfigEntryList`): its background, its rows, no separators.
        ui::list_background(batch, atlas, l.top, l.bottom, in_world);
        let label_color = |i: usize| -> (u32, bool) {
            match (s.error(i).is_some(), s.changed(i)) {
                (true, ch) => (text::RED, ch),
                (false, true) => (text::WHITE, true),
                (false, false) => (text::GRAY, false),
            }
        };
        for (row, y) in &l.rows {
            if *y + ITEM <= l.top || *y >= l.bottom {
                continue;
            }
            let start = batch.vertices.len();
            let (cx, cy) = (l.row_left + 2.0, y + 2.0);
            let cw = l.row_width - 4.0;
            match *row {
                Row::Prefix(i) => {
                    let key = s.nodes[i].prefix_key().unwrap_or_default();
                    for (n, line) in atlas.font.split(&glue::tr(&key), cw).iter().enumerate() {
                        batch.text_styled(atlas, line, cx, cy + n as f32 * 9.0, u32::MAX, true);
                    }
                }
                Row::Title(g) => {
                    let t = glue::tr(&g.title_key());
                    let tw = atlas.font.width_text(&t);
                    batch.text_styled(atlas, &t, cx + (cw / 2.0).trunc() - (tw / 2.0).trunc(), cy + 5.0, u32::MAX, true);
                }
                Row::Value(i) => {
                    let (color, italic) = label_color(i);
                    let label = styled(glue::tr(&s.nodes[i].title_key), color, italic);
                    batch.text_styled(atlas, &label, cx, cy + 5.0, u32::MAX, false);
                }
            }
            if *y < l.top || y + ITEM > l.bottom {
                batch.clip_since(start, 0.0, l.top, w, l.bottom);
            }
        }
        for pl in l.widgets.iter().filter(|pl| pl.list) {
            let [x, y, bw, bh] = pl.rect;
            if y >= l.bottom || y + bh <= l.top {
                continue;
            }
            let start = batch.vertices.len();
            let hover = hovered.is_some_and(|hv| hv.w == pl.w);
            match pl.w {
                W::Value(i) => ui::button(batch, atlas, x, y, bw, bh, &value_message(s, i), true, hover),
                W::Reset(_) => ui::button(batch, atlas, x, y, bw, bh, &glue::tr("shulkerboxtooltip.config.reset_to_default.small"), active(s, pl.w), hover),
                W::Undo(_) => ui::button(batch, atlas, x, y, bw, bh, &glue::tr("shulkerboxtooltip.config.undo.small"), active(s, pl.w), hover),
                W::Field(i) => {
                    let invalid = s.error(i).is_some();
                    let focused = s.focus == Some(i);
                    if let Some(b) = s.boxes[i].as_mut() {
                        [b.x, b.y, b.width, b.height] = pl.rect;
                        b.focused = focused;
                        b.text_color = if invalid { 0xFFFF_5555 } else { 0xFFE0_E0E0 };
                        b.draw(batch, atlas, None);
                    }
                }
                W::Swatch(i) => {
                    // `ColorWidget`: the field's frame (lit while the field has
                    // the focus), the colour inside.
                    let field_focused = s.focus == Some(i);
                    let sprite = if field_focused { "gui/sprites/widget/text_field_highlighted" } else { "gui/sprites/widget/text_field" };
                    ui::blit_sprite(batch, atlas, sprite, x, y, bw, bh, u32::MAX);
                    let c = s.editing[i].as_int() as u32 | 0xFF00_0000;
                    let [a, r, g, b] = c.to_be_bytes();
                    batch.fill(atlas, x + 1.0, y + 1.0, bw - 2.0, bh - 2.0, horizon_client::hud::rgba(r, g, b, a));
                }
                _ => {}
            }
            if y < l.top || y + bh > l.bottom {
                batch.clip_since(start, 0.0, l.top, w, l.bottom);
            }
        }
        let bar_x = l.row_left + l.row_width + 8.0;
        ui::scrollbar(batch, atlas, bar_x, l.top, l.bottom, l.content, s.scroll[s.tab].amount);

        // The tab bar (`MenuTabBar`): the header separator either side, the tabs.
        let (x0, tw) = tabs(w);
        let right = x0 + tw * 5.0;
        for (x, width) in [(0.0, x0), (right, w - right)] {
            ui::tiled_sized(batch, atlas, "gui/header_separator", x, TAB_H - 2.0, width, 2.0, [32.0, 2.0], u32::MAX);
        }
        for (i, cat) in Category::ALL.iter().enumerate() {
            let x = x0 + tw * i as f32;
            let selected = s.tab == i;
            let hover = hovered.is_some_and(|hv| hv.w == W::Tab(i));
            let mut label = glue::tr(&cat.title_key());
            let (changed, error) = (s.tab_changed(*cat), s.tab_error(*cat));
            if error {
                label = label.with_color(text::RED);
            }
            if changed {
                label = label.italic();
            }
            let sprite = match (selected, hover) {
                (true, true) => "gui/sprites/widget/tab_selected_highlighted",
                (true, false) => "gui/sprites/widget/tab_selected",
                (false, true) => "gui/sprites/widget/tab_highlighted",
                (false, false) => "gui/sprites/widget/tab",
            };
            ui::nine_slice(batch, atlas, sprite, x, 0.0, tw, TAB_H, u32::MAX);
            if selected {
                ui::menu_background(batch, atlas, x + 2.0, 2.0, tw - 4.0, TAB_H - 2.0, false);
                let uw = atlas.font.width_text(&label).floor().min(tw - 4.0);
                batch.fill(atlas, x + ((tw - uw) / 2.0).trunc(), TAB_H - 2.0, uw, 1.0, u32::MAX);
            }
            ui::menu_tab_label(batch, atlas, &label, (x, 0.0, tw, TAB_H), selected, true, horizon_client::hud_overlays::now_ms());
        }

        // The footer: its separator (`Screen.FOOTER_SEPARATOR`), Cancel and Save & Quit.
        ui::tiled_sized(batch, atlas, "gui/footer_separator", 0.0, h - FOOTER - 2.0, w, 2.0, [32.0, 2.0], u32::MAX);
        for pl in l.widgets.iter().filter(|pl| matches!(pl.w, W::Cancel | W::Save)) {
            let [x, y, bw, bh] = pl.rect;
            let label = match pl.w {
                W::Cancel if s.any_changed() => glue::tr("shulkerboxtooltip.config.quit.unsaved"),
                W::Cancel => Text::translate("gui.cancel", &[]),
                _ if s.valid() => glue::tr("shulkerboxtooltip.config.save"),
                _ => glue::tr("shulkerboxtooltip.config.cannot_save"),
            };
            ui::button(batch, atlas, x, y, bw, bh, &label, active(s, pl.w), hovered.is_some_and(|hv| hv.w == pl.w));
        }

        // The hovered entry's tooltip, or its Reset / Undo button's.
        if mouse.1 >= l.top && mouse.1 < l.bottom && mouse.0 >= l.row_left && mouse.0 < l.row_left + l.row_width {
            let entry = l.rows.iter().find(|(_, y)| mouse.1 >= *y && mouse.1 < y + ITEM).map(|(r, _)| *r);
            if let Some(Row::Value(i)) = entry {
                let lines = match hovered.map(|hv| hv.w) {
                    Some(W::Reset(_)) => vec![glue::tr("shulkerboxtooltip.config.reset_to_default.full")],
                    Some(W::Undo(_)) => vec![glue::tr("shulkerboxtooltip.config.undo.full")],
                    _ => tooltip_lines(game, s, i),
                };
                ui::tooltip(batch, atlas, &lines, mouse);
            }
        }
    });
}

// ---- input --------------------------------------------------------------------------------

fn bit(button: i32) -> u8 {
    1 << button.clamp(0, 7)
}

/// Widget `w` pressed with the left button.
fn press(s: &mut State, w: W, shift: bool) {
    if !active(s, w) {
        return;
    }
    match w {
        W::Tab(i) => {
            if s.tab != i {
                s.tab = i;
                s.selected_key = None;
                s.focus = None;
            }
            s.requests.push(Request::Click);
        }
        W::Value(i) => {
            s.requests.push(Request::Click);
            match s.nodes[i].kind {
                Kind::Bool => {
                    let v = !s.editing[i].as_bool();
                    s.set(i, Value::Bool(v));
                }
                // `CycleButton`: Shift goes back.
                Kind::Enum(k) => {
                    let n = k.values().len();
                    let cur = s.editing[i].as_enum();
                    let next = if shift { (cur + n - 1) % n } else { (cur + 1) % n };
                    s.set(i, Value::Enum(next));
                }
                Kind::Key => s.selected_key = Some(i),
                _ => {}
            }
        }
        W::Reset(i) => {
            s.requests.push(Request::Click);
            let d = s.nodes[i].default.clone();
            s.set(i, d);
            if s.nodes[i].kind == Kind::Key {
                s.selected_key = None;
            }
        }
        W::Undo(i) => {
            s.requests.push(Request::Click);
            let a = s.active.values[i].clone();
            s.set(i, a);
        }
        W::Cancel => {
            s.requests.push(Request::Click);
            s.quit();
        }
        W::Save => {
            s.requests.push(Request::Click);
            s.save_and_quit();
        }
        W::ConfirmYes => {
            s.requests.push(Request::Click);
            match s.confirm {
                Some(Confirm::Quit) => s.close = true,
                _ => s.quit_game = true,
            }
        }
        W::ConfirmNo => {
            s.requests.push(Request::Click);
            match s.confirm {
                // Back to the config screen, which `init`s again: the values
                // go back to those in effect.
                Some(Confirm::Quit) => {
                    s.confirm = None;
                    s.reset_to_active();
                }
                _ => s.close = true,
            }
        }
        W::Field(_) | W::Swatch(_) => {}
    }
}

pub(super) fn pressed(game: &Game<'_>, screen: &NativeScreenView<'_>, button: i32) -> bool {
    if !over(game, screen) {
        return false;
    }
    with(|s| s.taken |= bit(button));
    let p = (screen.mouse[0], screen.mouse[1]);
    let l = current(game);
    let shift = game.native_has_shift_down();
    with(|s| {
        if button != 0 {
            return;
        }
        if l.confirm.is_none() {
            let bar_x = l.row_left + l.row_width + 8.0;
            let tab = s.tab;
            if s.scroll[tab].press(p, bar_x, l.top, l.bottom, l.content) {
                return;
            }
        }
        let hit = under(&l, p);
        // A press anywhere else takes the focus off a field.
        s.focus = match hit {
            Some(Placed { w: W::Field(i), .. }) => Some(i),
            _ => None,
        };
        if let Some(Placed { w: W::Field(i), .. }) = hit
            && let Some(b) = s.boxes[i].as_mut()
        {
            b.click(&game.gui.atlas, p.0, shift, false);
            b.reset_blink();
            s.dragging_field = true;
        }
        if let Some(pl) = hit {
            press(s, pl.w, shift);
        }
    });
    true
}

pub(super) fn released(button: i32) -> bool {
    with(|s| {
        let was = s.taken & bit(button) != 0;
        s.taken &= !bit(button);
        if button == 0 {
            for sc in &mut s.scroll {
                sc.release();
            }
            s.dragging_field = false;
        }
        was
    })
}

pub(super) fn dragged(game: &Game<'_>, screen: &NativeScreenView<'_>) -> bool {
    if !over(game, screen) {
        return false;
    }
    let p = (screen.mouse[0], screen.mouse[1]);
    let l = current(game);
    with(|s| {
        let tab = s.tab;
        if s.scroll[tab].held() {
            s.scroll[tab].drag(p.1, l.top, l.bottom, l.content);
        } else if s.dragging_field
            && let Some(i) = s.focus
            && let Some(b) = s.boxes[i].as_mut()
        {
            b.drag_to(&game.gui.atlas, p.0);
        }
    });
    true
}

pub(super) fn scrolled(game: &Game<'_>, screen: &NativeScreenView<'_>, scroll: [f64; 2]) -> bool {
    if !over(game, screen) {
        return false;
    }
    let l = current(game);
    if l.confirm.is_none() && screen.mouse[1] >= l.top && screen.mouse[1] < l.bottom {
        with(|s| {
            let tab = s.tab;
            s.scroll[tab].wheel(scroll[1] as f32, ITEM / 2.0, l.top, l.bottom, l.content);
        });
    }
    true
}

/// A key that types a character (the char hook inserts it).
fn writes(key: &str) -> bool {
    let Some(name) = key.strip_prefix("key.keyboard.") else {
        return false;
    };
    name.chars().count() == 1 || name == "space" || name.starts_with("keypad.") && name.len() == 8
}

/// `ConfigScreen.keyPressed`: a key button waiting takes the key (Escape:
/// none); Ctrl+Tab and Ctrl+1..5 change tabs; Escape closes; a focused field
/// edits. Every key is the screen's.
pub(super) fn key_pressed(game: &Game<'_>, screen: &NativeScreenView<'_>, key: &str, modifiers: u32) -> bool {
    if !over(game, screen) {
        return false;
    }
    let escape = key == "key.keyboard.escape";
    let control = modifiers & CONTROL != 0;
    let shift = modifiers & SHIFT != 0;
    with(|s| {
        if s.confirm.is_some() {
            // `ConfirmScreen.keyPressed`: Escape answers no.
            if escape {
                press(s, W::ConfirmNo, false);
            }
            return;
        }
        if let Some(i) = s.selected_key.take() {
            let v = if escape { sbt::UNBOUND.to_owned() } else { key.to_owned() };
            s.set(i, Value::Key(v));
            return;
        }
        if control {
            let next = match key {
                "key.keyboard.tab" if shift => Some((s.tab + 4) % 5),
                "key.keyboard.tab" => Some((s.tab + 1) % 5),
                _ => key.strip_prefix("key.keyboard.").and_then(|d| d.parse::<usize>().ok()).filter(|d| (1..=9).contains(d)).map(|d| (d - 1).min(4)),
            };
            if let Some(t) = next {
                s.tab = t;
                s.focus = None;
                s.requests.push(Request::Click);
                return;
            }
        }
        if escape {
            s.quit();
            return;
        }
        let Some(i) = s.focus else { return };
        let request = match key {
            "key.keyboard.v" if control => Some(Request::Paste),
            "key.keyboard.c" if control => s.boxes[i].as_ref().map(|b| Request::Copy(b.selected_text())),
            "key.keyboard.x" if control => {
                let r = s.boxes[i].as_mut().map(|b| {
                    let t = b.selected_text();
                    b.delete_chars(0);
                    Request::Copy(t)
                });
                s.field_changed(i);
                r
            }
            _ if !control && writes(key) => None,
            _ => {
                if let Some(code) = horizon_client::keybinds::key_code(key)
                    && let Some(b) = s.boxes[i].as_mut()
                {
                    let before = b.value.clone();
                    b.key(code, control, shift);
                    b.reset_blink();
                    if b.value != before {
                        s.field_changed(i);
                    }
                }
                None
            }
        };
        s.requests.extend(request);
    });
    true
}

/// A character typed: into the focused field.
pub(super) fn char_typed(game: &Game<'_>, screen: &NativeScreenView<'_>, c: char) -> bool {
    if !over(game, screen) {
        return false;
    }
    with(|s| {
        if s.confirm.is_some() || s.selected_key.is_some() || c.is_control() {
            return;
        }
        if let Some(i) = s.focus
            && let Some(b) = s.boxes[i].as_mut()
        {
            b.insert_text(&c.to_string());
            b.reset_blink();
            s.field_changed(i);
        }
    });
    true
}

pub(super) fn swallowed(game: &Game<'_>, screen: &NativeScreenView<'_>) -> bool {
    over(game, screen)
}

// ---- the icon and Mod Menu's metadata -----------------------------------------------------

/// ShulkerBoxTooltip's icon (`assets/shulkerboxtooltip/icon.png`).
const ICON: &[u8] = horizon_client::native_data!("shulkerboxtooltip", "icon.png");

const CONTRIBUTORS: [&str; 21] = [
    "Samekichi",
    "disguys",
    "Felix14-v2",
    "egeesin",
    "gbl",
    "gyular",
    "notlin4",
    "JustEmo",
    "ReneMuetti",
    "yichifauzi",
    "Alexander317",
    "TheLegendofSaram",
    "yumunet",
    "Texaliuz",
    "pilahito",
    "mpustovoi",
    "camdencove",
    "WyattMarks",
    "lishhhhmm",
    "Fhilips613",
    "DeportedToHell",
];

/// What Mod Menu shows of the mod (`fabric.mod.json`).
fn modmenu_json() -> String {
    let contributors: Vec<String> = CONTRIBUTORS.iter().map(|c| format!("\"{c}\"")).collect();
    format!(
        r#"{{
  "schema": 1,
  "name": "{name}",
  "version": "{version}",
  "description": "{en}",
  "authors": ["MisterPeModder"],
  "contributors": [{contributors}],
  "license": "MIT",
  "links": {{
    "homepage": "https://minecraft.curseforge.com/projects/shulkerboxtooltip",
    "sources": "https://github.com/DeportedToHell/MCRE-Mods/tree/main/shulkerboxtooltip",
    "issues": "https://github.com/DeportedToHell/MCRE-Mods/issues"
  }},
  "badges": [],
  "config_screen": true
}}
"#,
        name = sbt::NAME,
        version = sbt::VERSION,
        en = glue::description(false),
        contributors = contributors.join(", "),
    )
}

/// `icon.png` and `modmenu.json` in the mod's data folder, each written only
/// when it differs from what is there.
fn write_data(game: &Game<'_>) {
    let dir = game.native_data_dir(sbt::ID);
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
