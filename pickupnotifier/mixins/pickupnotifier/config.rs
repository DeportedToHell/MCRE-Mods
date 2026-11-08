// Pick Up Notifier for Horizon: the original's config screen, as Mod Menu's
// Configure button opens it on Fabric 26.3. Puzzles Lib's
// `FabricConfigurationScreen` (github.com/Fuzss/puzzleslib, MPL-2.0) over
// Forge Config API Port's `ConfigurationSectionScreen` and
// `ConfigurationListScreen` (github.com/Fuzss/forgeconfigapiport, LGPL-2.1,
// NeoForge's): vanilla's options list screen with a label and a 150-wide
// widget per row, "Edit..." buttons into the sections, and Undo, Reset and
// Done at the foot. The client config only: the server config holds just
// "Partial Pick Ups", which this client-side port has not got, and with one
// config Puzzles Lib goes straight to it. The labels are the config keys in
// words and the tooltips their comments, in English as on Java (no
// translations), but for vanilla's ON/OFF and Done.
//
// The screen covers the Options screen while it is open on the mod's page,
// opened by the page's first button or Mod Menu's Configure; Escape and Done
// go back up, and out of the top screen to the page (or out of the options,
// to Mod Menu). Also writes Mod Menu's metadata.
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.
use super::pickupnotifier::{self as pun, Setting};
use super::pickupnotifier_glue as glue;
use super::{Game, Screen};
use horizon_client::hud::{HudBatch, rgba};
use horizon_client::mod_api::NativeScreenView;
use horizon_client::options_ui::Page;
use horizon_client::text::Text;
use horizon_client::ui::{self, EditBox};
use std::cell::{Cell, RefCell};

/// Mod Menu's Configure: `native_settings_click(id, MODMENU_CONFIGURE)` after it opened the mod's page.
pub const MODMENU_CONFIGURE: usize = 0x4D4D_4346;

const HEADER: f32 = 33.0;
const ROW: f32 = 25.0;
const WHITE: u32 = 0xFFFF_FFFF;
const YELLOW: u32 = 0xFFFF_FF55;
const GOLD: u32 = 0xFFFF_AA00;
const RED: u32 = 0xFFFF_0000;
const EDIT_TEXT: u32 = 0xFFE0_E0E0;
/// `Tooltip.MAX_WIDTH`.
const TOOLTIP_WIDTH: f32 = 170.0;

fn argb(c: u32) -> u32 {
    let [a, r, g, b] = c.to_be_bytes();
    rgba(r, g, b, a)
}

// ---- the options ----------------------------------------------------------------------------

/// A section of the client config, in the screen's order (Puzzles Lib's
/// `HashMultimap` order: Display, General, Behavior).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Section {
    Display,
    General,
    Behavior,
}

const SECTIONS: [Section; 3] = [Section::Display, Section::General, Section::Behavior];

impl Section {
    fn name(self) -> &'static str {
        match self {
            Section::Display => "Display",
            Section::General => "General",
            Section::Behavior => "Behavior",
        }
    }
    /// Its values, in the screen's order (`client_only` is left out: the port
    /// always runs on the client alone).
    fn rows(self) -> &'static [Setting] {
        match self {
            Section::General => &[
                Setting::ExperienceValue,
                Setting::IncludeItems,
                Setting::DisableInCreative,
                Setting::IncludeExperience,
                Setting::HiddenItems,
                Setting::IncludeArrows,
            ],
            Section::Behavior => &[Setting::FadeOut, Setting::MoveOut, Setting::DisplayTime, Setting::MoveTime, Setting::Combine],
            Section::Display => &[
                Setting::MaxHeight,
                Setting::SingleCount,
                Setting::OffsetY,
                Setting::OffsetX,
                Setting::ItemName,
                Setting::Amount,
                Setting::DisplayScale,
                Setting::Corner,
                Setting::IgnoreRarity,
                Setting::DrawSprite,
                Setting::InventoryCount,
                Setting::Background,
                Setting::TextColor,
            ],
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Bool,
    Enum,
    Int,
    Double,
    Slider,
    List,
}

fn kind(s: Setting) -> Kind {
    match s {
        Setting::DisplayTime | Setting::MoveTime | Setting::OffsetX | Setting::OffsetY => Kind::Int,
        Setting::MaxHeight => Kind::Double,
        Setting::DisplayScale => Kind::Slider,
        Setting::HiddenItems => Kind::List,
        _ if !s.choices().is_empty() => Kind::Enum,
        _ => Kind::Bool,
    }
}

/// The config key in words (`ModContainer.getCapitalizedString`).
fn label(s: Setting) -> String {
    s.key()
        .unwrap_or_default()
        .split('_')
        .map(|w| {
            let mut c = w.chars();
            c.next().map(|f| f.to_uppercase().chain(c).collect::<String>()).unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The config's comment lines (`@Config` descriptions, then what the spec
/// adds), as Puzzles Lib's tooltip shows them.
fn comment(s: Setting) -> Vec<String> {
    let d = |lines: &[&str]| lines.iter().map(|l| (*l).to_owned()).collect::<Vec<_>>();
    let allowed = |s: Setting| {
        format!("Allowed Values: {}", s.choices().iter().map(|c| c.to_uppercase()).collect::<Vec<_>>().join(", "))
    };
    let default = pun::Config::default();
    let value = |s: Setting| {
        let v = default.value(s);
        match kind(s) {
            Kind::Enum => v.to_uppercase(),
            Kind::List => "[]".to_owned(),
            _ => v,
        }
    };
    let mut lines = match s {
        Setting::IncludeItems => d(&["Show item entities the player has collected in the pick-up notifications."]),
        Setting::IncludeExperience => d(&["Show experience orbs the player has collected in the pick-up notifications."]),
        Setting::IncludeArrows => d(&["Show shot arrows the player has collected in the pick-up notifications."]),
        Setting::ExperienceValue => d(&["Show the value of experience points collected instead of the amount of individual orbs."]),
        Setting::DisableInCreative => d(&["Prevent items from being added to the pick-up log when in creative mode."]),
        Setting::HiddenItems => d(&[
            "Disable specific items or content from whole mods from showing.",
            "Format for every entry is \"<namespace>:<path>\". Tags are supported, must be in the format of \"#<namespace>:<path>\". Namespace may be omitted to use \"minecraft\" by default. May use asterisk as wildcard parameter via pattern matching, e.g. \"minecraft:*_shulker_box\" to match all shulker boxes no matter of color. Begin an entry with \"!\" to make sure it is excluded, useful e.g. when it has already been matched by another pattern.",
        ]),
        Setting::Combine => d(&["Combine entries of the same type instead of showing each one individually."]),
        Setting::DisplayTime => {
            d(&["Amount of ticks each entry will be shown for. Set to zero to only remove entries when space for new ones is needed."])
        }
        Setting::MoveOut => d(&["Make outdated entries slowly move out of the screen.", "Not necessarily supported by all position presets."]),
        Setting::MoveTime => d(&["Amount of ticks it takes for an entry to move out of the screen."]),
        Setting::FadeOut => d(&["Make outdated entry names slowly fade away instead of simply vanishing."]),
        Setting::DrawSprite => d(&["Show a small sprite next to the name of each entry showing its contents."]),
        Setting::IgnoreRarity => d(&["Ignore rarity when determining item name color."]),
        Setting::Corner => d(&["Screen corner for entry list to be drawn in."]),
        Setting::OffsetX => d(&["Offset on x-axis from screen border."]),
        Setting::OffsetY => d(&["Offset on y-axis from screen border."]),
        Setting::MaxHeight => d(&["Percentage of relative screen height entries are allowed to fill at max."]),
        Setting::DisplayScale => d(&[
            "Scale of entries. A lower scale will make room for more rows to show. Works together with \"GUI Scale\" option in \"Video Settings\".",
        ]),
        Setting::Amount => d(&[
            "Where and if to display the amount of items picked up. 'SPRITE' will render the amount on the item sprite like in inventories, 'TEXT' will add a dedicated text including the amount to the item name display.",
        ]),
        Setting::InventoryCount => d(&["Add the total amount of an item in your inventory to the entry."]),
        Setting::SingleCount => d(&["Should the picked up amount be shown when it's just a single item."]),
        Setting::Background => d(&[
            "Mode for drawing a background behind entries for better visibility. 'CHAT' is similar to the chat background, 'TOOLTIP' uses the tooltip background rendering instead.",
        ]),
        Setting::ItemName => d(&["Add the name of the item to the entry."]),
        // `addToBuilder`: its comment, then only the allowed values.
        Setting::TextColor => return vec!["Color for the entry name text.".to_owned(), allowed(s)],
        Setting::Reset => Vec::new(),
    };
    lines.push(format!("Default Value: {}", value(s)));
    match s {
        Setting::DisplayTime | Setting::MoveTime | Setting::OffsetX | Setting::OffsetY => lines.push("Value Range: > 0".to_owned()),
        Setting::MaxHeight => lines.push("Value Range: 0.0 ~ 1.0".to_owned()),
        Setting::DisplayScale => lines.push("Value Range: 1 ~ 24".to_owned()),
        _ if kind(s) == Kind::Enum => lines.push(allowed(s)),
        _ => {}
    }
    lines
}

/// The tooltip: the name, an empty line, the comment lines in yellow and gold.
fn tooltip(s: Setting) -> Vec<(String, u32)> {
    let mut out = vec![(label(s), WHITE), (String::new(), WHITE)];
    for (i, line) in comment(s).into_iter().enumerate() {
        out.push((line, if i % 2 == 0 { YELLOW } else { GOLD }));
    }
    out
}

/// A stored value as the widget shows it.
fn shown(cfg: &pun::Config, s: Setting, ru: bool) -> String {
    let v = cfg.value(s);
    match kind(s) {
        Kind::Bool => match (v == "true", ru) {
            (true, false) => "ON",
            (false, false) => "OFF",
            (true, true) => "Вкл",
            (false, true) => "Выкл",
        }
        .to_owned(),
        Kind::Enum => v.to_uppercase(),
        _ => v,
    }
}

// ---- the screens ----------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum View {
    Top,
    Section(Section),
    List,
}

/// One undo step: the settings it changed, with their values before.
type Step = Vec<(Setting, String)>;

struct ConfigScreen {
    ru: bool,
    /// The screens open, the top one last.
    views: Vec<View>,
    scroll: f32,
    /// The section screen's undo steps (FCAP's `UndoManager`).
    undo: Vec<Step>,
    /// The number boxes of the section screen, by setting.
    boxes: Vec<(Setting, EditBox)>,
    /// The slider being dragged.
    sliding: Option<Setting>,
    /// The list screen's working copy, its undo steps and its boxes.
    list: Vec<String>,
    list_undo: Vec<Vec<String>>,
    list_boxes: Vec<EditBox>,
    list_scroll: f32,
    /// The scrollbar being dragged, with the grab offset.
    bar: Option<f32>,
    closed: bool,
    clicks: u32,
}

thread_local! {
    static SCREEN: RefCell<Option<ConfigScreen>> = const { RefCell::new(None) };
    /// Opened by Mod Menu (else by the page's button).
    static MOD_MENU: Cell<bool> = const { Cell::new(false) };
    static TAKEN: Cell<u8> = const { Cell::new(0) };
    static WRITTEN: Cell<bool> = const { Cell::new(false) };
    static SIZE: Cell<(f32, f32)> = const { Cell::new((0.0, 0.0)) };
}

fn with<T>(f: impl FnOnce(&mut ConfigScreen) -> T) -> Option<T> {
    SCREEN.with(|s| s.try_borrow_mut().ok().and_then(|mut s| s.as_mut().map(f)))
}

/// The mod's page is the one the Options screen shows.
fn page(game: &Game<'_>) -> Option<usize> {
    if game.screen != Screen::Options {
        return None;
    }
    match game.ui.options.page() {
        Page::NativeMod(i) if game.settings.mods.installed.get(i).is_some_and(|m| m.id == pun::ID) => Some(i),
        _ => None,
    }
}

pub(super) fn covers(game: &Game<'_>) -> bool {
    SCREEN.with(|s| s.try_borrow().is_ok_and(|s| s.is_some())) && page(game).is_some()
}

pub(super) fn open(game: &mut Game<'_>, from_mod_menu: bool) {
    if page(game).is_none() {
        return;
    }
    let screen = ConfigScreen {
        ru: glue::ru(),
        views: vec![View::Top],
        scroll: 0.0,
        undo: Vec::new(),
        boxes: Vec::new(),
        sliding: None,
        list: Vec::new(),
        list_undo: Vec::new(),
        list_boxes: Vec::new(),
        list_scroll: 0.0,
        bar: None,
        closed: false,
        clicks: 0,
    };
    SCREEN.with(|s| *s.borrow_mut() = Some(screen));
    MOD_MENU.with(|m| m.set(from_mod_menu));
    TAKEN.with(|t| t.set(0));
}

/// Once a frame: the data files, the clicks' sound, and the screen's closing.
pub(super) fn frame(game: &mut Game<'_>) {
    if !WRITTEN.with(Cell::get) {
        WRITTEN.with(|w| w.set(true));
        write_data(game);
    }
    let Some((clicks, closed)) = with(|c| (std::mem::take(&mut c.clicks), c.closed)) else {
        return;
    };
    for _ in 0..clicks {
        game.audio.play_button_click();
    }
    match page(game) {
        None => SCREEN.with(|s| *s.borrow_mut() = None),
        Some(i) if closed => {
            SCREEN.with(|s| *s.borrow_mut() = None);
            if MOD_MENU.with(Cell::get) {
                let events = game.ui.options.back(&mut game.settings);
                let _ = game.handle_option_events(events);
            } else {
                game.native_settings_refresh(i);
            }
        }
        Some(_) => {}
    }
}

fn number_box(text: &str) -> EditBox {
    let mut b = EditBox::new(0.0, 0.0, 150.0, 20.0);
    b.max_length = 32;
    b.focused = false;
    b.set_value(text);
    b.text_color = argb(EDIT_TEXT);
    b
}

/// The list's rows: its elements, then the "+" row.
fn list_rows(len: usize) -> usize {
    len + 1
}

impl ConfigScreen {
    fn view(&self) -> View {
        *self.views.last().unwrap_or(&View::Top)
    }

    fn rows(&self) -> usize {
        match self.view() {
            View::Top => SECTIONS.len(),
            View::Section(s) => s.rows().len(),
            View::List => list_rows(self.list.len()),
        }
    }

    fn scroll_mut(&mut self) -> &mut f32 {
        if self.view() == View::List { &mut self.list_scroll } else { &mut self.scroll }
    }

    fn max_scroll(&self, h: f32) -> f32 {
        (ROW * self.rows() as f32 + 4.0 - (h - 2.0 * HEADER)).max(0.0)
    }

    /// The number boxes as the values are now (FCAP's `rebuild`).
    fn rebuild(&mut self, cfg: &pun::Config) {
        self.boxes.clear();
        if let View::Section(sec) = self.view() {
            for &s in sec.rows() {
                if matches!(kind(s), Kind::Int | Kind::Double) {
                    self.boxes.push((s, number_box(&cfg.value(s))));
                }
            }
        }
        self.list_boxes = self.list.iter().map(|v| number_box(v)).collect();
        for b in &mut self.list_boxes {
            b.max_length = (b.value.chars().count() + 5).clamp(128, 192);
        }
    }

    fn enter(&mut self, game: &Game<'_>, view: View) {
        self.views.push(view);
        let cfg = glue::config(game);
        if view == View::List {
            self.list = cfg.hidden_items.clone();
            self.list_undo.clear();
            self.list_scroll = 0.0;
        } else {
            self.scroll = 0.0;
            self.undo.clear();
        }
        self.rebuild(&cfg);
    }

    /// Done or Escape: the list keeps its working copy; the top screen closes.
    fn back(&mut self, game: &Game<'_>) {
        match self.views.pop() {
            Some(View::List) => {
                let mut cfg = glue::config(game);
                if cfg.hidden_items != self.list {
                    cfg.hidden_items = self.list.clone();
                    glue::store(game, cfg.clone(), &[Setting::HiddenItems]);
                }
                self.rebuild(&cfg);
            }
            Some(View::Section(_)) => {
                self.scroll = 0.0;
                self.undo.clear();
                self.boxes.clear();
            }
            _ => self.closed = true,
        }
        if self.views.is_empty() {
            self.closed = true;
        }
    }

    fn set(&mut self, game: &Game<'_>, changes: &[(Setting, String)]) {
        let mut cfg = glue::config(game);
        let mut step = Vec::new();
        let mut changed = Vec::new();
        for (s, v) in changes {
            let old = cfg.value(*s);
            if &old != v && cfg.set(*s, v) {
                step.push((*s, old));
                changed.push(*s);
            }
        }
        if !step.is_empty() {
            glue::store(game, cfg, &changed);
            self.undo.push(step);
        }
    }

    // ---- layout ----

    /// The widget rectangles of row `i`: label (or the list row's left part)
    /// and the right column's widget.
    fn row_rects(&self, w: f32, i: usize) -> ([f32; 4], [f32; 4]) {
        let scroll = if self.view() == View::List { self.list_scroll } else { self.scroll };
        let y = 37.0 + ROW * i as f32 - scroll.floor();
        let l = (w / 2.0).floor() - 155.0;
        ([l, y, 150.0, 20.0], [l + 160.0, y, 150.0, 20.0])
    }

    fn footer(&self, w: f32, h: f32) -> Vec<(&'static str, [f32; 4])> {
        let y = h - 27.0;
        if self.view() == View::Top {
            vec![("done", [((w - 200.0) / 2.0).floor(), y, 200.0, 20.0])]
        } else {
            let x0 = ((w - 376.0) / 2.0).floor();
            vec![("undo", [x0, y, 120.0, 20.0]), ("reset", [x0 + 128.0, y, 120.0, 20.0]), ("done", [x0 + 256.0, y, 120.0, 20.0])]
        }
    }

    fn title(&self) -> Text {
        let mut t = String::from("Pick Up Notifier Settings");
        for v in &self.views[1..] {
            let name = match v {
                View::Section(s) => s.name(),
                View::List => "Hidden Items",
                View::Top => continue,
            };
            t.push_str(&format!(" §6§l>§r {name}"));
        }
        Text::legacy(&t)
    }

    fn can_undo(&self) -> bool {
        if self.view() == View::List { !self.list_undo.is_empty() } else { !self.undo.is_empty() }
    }

    fn can_reset(&self, cfg: &pun::Config) -> bool {
        let d = pun::Config::default();
        match self.view() {
            View::Section(sec) => sec.rows().iter().any(|&s| kind(s) != Kind::List && cfg.value(s) != d.value(s)),
            View::List => !self.list.is_empty(),
            View::Top => false,
        }
    }

    // ---- drawing ----

    fn draw(&mut self, game: &Game<'_>, batch: &mut HudBatch) -> Option<Vec<(String, u32)>> {
        let (w, h) = (batch.width, batch.height);
        let atlas = &game.gui.atlas;
        let in_world = game.title.is_none() && game.player.is_some();
        let p = game.cursor;
        let cfg = glue::config(game);
        let ru = self.ru;
        let max = self.max_scroll(h);
        let s = self.scroll_mut();
        *s = s.clamp(0.0, max);
        // `Screen.extractTransparentBackground`, then the list's background.
        if in_world {
            game.native_apply_screen_blur();
        }
        ui::menu_background(batch, atlas, 0.0, 0.0, w, h, in_world);
        ui::list_background(batch, atlas, HEADER, h - HEADER, in_world);
        // The title.
        let title = self.title();
        let tw = atlas.font.width_text(&title);
        batch.text_styled(atlas, &title, ((w - tw) * 0.5).floor(), 12.0, argb(WHITE), true);

        let mut tip: Option<Vec<(String, u32)>> = None;
        let in_list = p.1 >= HEADER && p.1 < h - HEADER;
        let start = batch.vertices.len();
        for i in 0..self.rows() {
            let (left, right) = self.row_rects(w, i);
            if left[1] + 20.0 < HEADER || left[1] > h - HEADER {
                continue;
            }
            let over = |r: [f32; 4]| in_list && ui::hovered(p, r[0], r[1], r[2], r[3]);
            match self.view() {
                View::Top => {
                    let sec = SECTIONS[i];
                    label_text(game, batch, &format!("{}...", sec.name()), left);
                    ui::button(batch, atlas, right[0], right[1], 150.0, 20.0, &Text::literal("Edit..."), true, over(right));
                }
                View::Section(sec) => {
                    let s = sec.rows()[i];
                    let name = if kind(s) == Kind::List { format!("{}...", label(s)) } else { label(s) };
                    label_text(game, batch, &name, left);
                    match kind(s) {
                        Kind::Bool | Kind::Enum => {
                            ui::button(batch, atlas, right[0], right[1], 150.0, 20.0, &Text::literal(shown(&cfg, s, ru)), true, over(right));
                        }
                        Kind::List => {
                            ui::button(batch, atlas, right[0], right[1], 150.0, 20.0, &Text::literal("Edit..."), true, over(right));
                        }
                        Kind::Slider => {
                            let v: i32 = cfg.value(s).parse().unwrap_or(4);
                            let value = if v <= 1 {
                                0.0
                            } else if v >= 24 {
                                1.0
                            } else {
                                (v as f32 + 0.5 - 1.0) / 24.0
                            };
                            let dragging = self.sliding == Some(s);
                            ui::slider(batch, atlas, right[0], right[1], 150.0, &Text::literal(v.to_string()), value, over(right), true, dragging);
                        }
                        Kind::Int | Kind::Double => {
                            if let Some((_, b)) = self.boxes.iter_mut().find(|(x, _)| *x == s) {
                                b.x = right[0];
                                b.y = right[1];
                                b.draw(batch, atlas, None);
                            }
                        }
                    }
                    if over(left) || over(right) {
                        tip = Some(tooltip(s));
                    }
                }
                View::List => {
                    if i < self.list.len() {
                        // `ListLabelWidget`: ⏶ ⏷, "0:", ❌.
                        let up = [left[0], left[1], 20.0, 20.0];
                        let down = [left[0] + 22.0, left[1], 20.0, 20.0];
                        let del = [left[0] + 130.0, left[1], 20.0, 20.0];
                        if i > 0 {
                            ui::button(batch, atlas, up[0], up[1], 20.0, 20.0, &Text::literal("\u{23f6}"), true, over(up));
                        }
                        if i + 1 < self.list.len() {
                            ui::button(batch, atlas, down[0], down[1], 20.0, 20.0, &Text::literal("\u{23f7}"), true, over(down));
                        }
                        batch.text(atlas, &format!("{i}:"), left[0] + 44.0, left[1] + 5.0, argb(WHITE), true);
                        ui::button(batch, atlas, del[0], del[1], 20.0, 20.0, &Text::literal("\u{274c}"), true, over(del));
                        if let Some(b) = self.list_boxes.get_mut(i) {
                            b.x = right[0];
                            b.y = right[1];
                            b.draw(batch, atlas, None);
                        }
                    } else {
                        ui::button(batch, atlas, right[0], right[1], 150.0, 20.0, &Text::literal("+"), true, over(right));
                    }
                }
            }
        }
        batch.clip_since(start, 0.0, HEADER, w, h - HEADER);
        let content = ROW * self.rows() as f32 + 4.0;
        let scroll = if self.view() == View::List { self.list_scroll } else { self.scroll };
        ui::scrollbar_sprites(batch, atlas, (w / 2.0).floor() + 163.0, HEADER, h - HEADER, content, scroll);
        ui::separators(batch, atlas, HEADER, h - HEADER, in_world);

        // The footer.
        let undo = self.can_undo();
        let reset = self.can_reset(&cfg);
        for (id, r) in self.footer(w, h) {
            let (text, active) = match id {
                "undo" => ("Undo", undo),
                "reset" => ("Reset", reset),
                _ => (if ru { "Готово" } else { "Done" }, true),
            };
            let hover = ui::hovered(p, r[0], r[1], r[2], r[3]);
            ui::button(batch, atlas, r[0], r[1], r[2], r[3], &Text::literal(text), active, hover);
            if hover && id == "undo" {
                tip = Some(vec![("Reverts changes on this screen only.".to_owned(), WHITE)]);
            } else if hover && id == "reset" {
                tip = Some(vec![("Reverts everything on this screen to its default value.".to_owned(), WHITE)]);
            }
        }
        tip
    }

    // ---- input ----

    fn press(&mut self, game: &Game<'_>, p: (f32, f32), button: i32) {
        let (w, h) = SIZE.with(Cell::get);
        for b in &mut self.boxes {
            b.1.focused = false;
        }
        for b in &mut self.list_boxes {
            b.focused = false;
        }
        if button != 0 {
            return;
        }
        // The footer.
        for (id, r) in self.footer(w, h) {
            if ui::hovered(p, r[0], r[1], r[2], r[3]) {
                self.footer_click(game, id);
                return;
            }
        }
        // The scrollbar.
        let bar_x = (w / 2.0).floor() + 163.0;
        if self.max_scroll(h) > 0.0 && p.0 >= bar_x && p.0 < bar_x + 6.0 && p.1 >= HEADER && p.1 < h - HEADER {
            self.bar = Some(p.1);
            return;
        }
        if p.1 < HEADER || p.1 >= h - HEADER {
            return;
        }
        let shift = game.has_shift_down();
        for i in 0..self.rows() {
            let (left, right) = self.row_rects(w, i);
            let on = |r: [f32; 4]| ui::hovered(p, r[0], r[1], r[2], r[3]);
            match self.view() {
                View::Top => {
                    if on(right) {
                        self.clicks += 1;
                        self.enter(game, View::Section(SECTIONS[i]));
                        return;
                    }
                }
                View::Section(sec) => {
                    let s = sec.rows()[i];
                    if !on(right) {
                        continue;
                    }
                    let cfg = glue::config(game);
                    match kind(s) {
                        Kind::Bool => {
                            self.clicks += 1;
                            let v = if cfg.value(s) == "true" { "false" } else { "true" };
                            self.set(game, &[(s, v.to_owned())]);
                        }
                        Kind::Enum => {
                            self.clicks += 1;
                            let all = s.choices();
                            let at = all.iter().position(|c| *c == cfg.value(s)).unwrap_or(0);
                            let next = if shift { (at + all.len() - 1) % all.len() } else { (at + 1) % all.len() };
                            self.set(game, &[(s, all[next].to_owned())]);
                        }
                        Kind::List => {
                            self.clicks += 1;
                            self.enter(game, View::List);
                        }
                        Kind::Slider => {
                            self.clicks += 1;
                            self.sliding = Some(s);
                            self.slide(game, p.0, right);
                        }
                        Kind::Int | Kind::Double => {
                            if let Some((_, b)) = self.boxes.iter_mut().find(|(x, _)| *x == s) {
                                b.focused = true;
                                b.click(&game.gui.atlas, p.0, shift, false);
                                b.reset_blink();
                            }
                        }
                    }
                    return;
                }
                View::List => {
                    if i < self.list.len() {
                        let up = [left[0], left[1], 20.0, 20.0];
                        let down = [left[0] + 22.0, left[1], 20.0, 20.0];
                        let del = [left[0] + 130.0, left[1], 20.0, 20.0];
                        let before = self.list.clone();
                        if i > 0 && on(up) {
                            self.list.swap(i, i - 1);
                        } else if i + 1 < self.list.len() && on(down) {
                            self.list.swap(i, i + 1);
                        } else if on(del) {
                            self.list.remove(i);
                        } else if on(right) {
                            if let Some(b) = self.list_boxes.get_mut(i) {
                                b.focused = true;
                                b.click(&game.gui.atlas, p.0, shift, false);
                                b.reset_blink();
                            }
                            return;
                        } else {
                            continue;
                        }
                        self.clicks += 1;
                        self.list_undo.push(before);
                        let cfg = glue::config(game);
                        self.rebuild(&cfg);
                        return;
                    } else if on(right) {
                        self.clicks += 1;
                        self.list_undo.push(self.list.clone());
                        self.list.push(String::new());
                        let cfg = glue::config(game);
                        self.rebuild(&cfg);
                        if let Some(b) = self.list_boxes.last_mut() {
                            b.focused = true;
                        }
                        return;
                    }
                }
            }
        }
    }

    fn footer_click(&mut self, game: &Game<'_>, id: &str) {
        let cfg = glue::config(game);
        match id {
            "undo" if self.can_undo() => {
                self.clicks += 1;
                if self.view() == View::List {
                    if let Some(prev) = self.list_undo.pop() {
                        self.list = prev;
                    }
                } else if let Some(step) = self.undo.pop() {
                    let mut cfg = cfg.clone();
                    let changed: Vec<Setting> = step.iter().map(|(s, _)| *s).collect();
                    for (s, v) in step.into_iter().rev() {
                        cfg.set(s, &v);
                    }
                    glue::store(game, cfg, &changed);
                }
                let cfg = glue::config(game);
                self.rebuild(&cfg);
            }
            "reset" if self.can_reset(&cfg) => {
                self.clicks += 1;
                if self.view() == View::List {
                    self.list_undo.push(std::mem::take(&mut self.list));
                } else if let View::Section(sec) = self.view() {
                    let d = pun::Config::default();
                    let changes: Vec<(Setting, String)> =
                        sec.rows().iter().filter(|&&s| kind(s) != Kind::List).map(|&s| (s, d.value(s))).collect();
                    self.set(game, &changes);
                }
                let cfg = glue::config(game);
                self.rebuild(&cfg);
            }
            "done" => {
                self.clicks += 1;
                self.back(game);
            }
            _ => {}
        }
    }

    /// `AbstractSliderButton.setValueFromMouse`, then `IntRange.fromSliderValue`.
    fn slide(&mut self, game: &Game<'_>, x: f32, r: [f32; 4]) {
        let Some(s) = self.sliding else {
            return;
        };
        let d = ui::slider_value(r[0], r[2], x);
        let v = (1.0 + d.min(0.99999) * 24.0).floor() as i32;
        if glue::config(game).value(s) != v.to_string() {
            self.set(game, &[(s, v.to_string())]);
        }
    }

    fn release(&mut self) {
        self.sliding = None;
        self.bar = None;
    }

    fn drag(&mut self, game: &Game<'_>, p: (f32, f32)) {
        let (w, h) = SIZE.with(Cell::get);
        if let Some(s) = self.sliding
            && let View::Section(sec) = self.view()
            && let Some(i) = sec.rows().iter().position(|x| *x == s)
        {
            let (_, right) = self.row_rects(w, i);
            self.slide(game, p.0, right);
            return;
        }
        if let Some(last) = self.bar {
            let content = ROW * self.rows() as f32 + 4.0;
            let track = h - 2.0 * HEADER;
            let thumb = ((track * track / content) as i32).clamp(32, (track - 8.0) as i32) as f32;
            let max = self.max_scroll(h);
            let per = max / (track - thumb).max(1.0);
            *self.scroll_mut() = (*self.scroll_mut() + (p.1 - last) * per).clamp(0.0, max);
            self.bar = Some(p.1);
        }
    }

    fn scroll(&mut self, dy: f64) {
        let (_, h) = SIZE.with(Cell::get);
        let max = self.max_scroll(h);
        let s = self.scroll_mut();
        *s = (*s - dy as f32 * ROW / 2.0).clamp(0.0, max);
    }

    /// The focused box, if any: the section's number box or a list element's.
    fn focused_box(&mut self) -> Option<(Option<Setting>, &mut EditBox)> {
        if let Some((s, b)) = self.boxes.iter_mut().find(|(_, b)| b.focused) {
            return Some((Some(*s), b));
        }
        self.list_boxes.iter_mut().find(|b| b.focused).map(|b| (None, b))
    }

    /// A box's text changed: FCAP's responder.
    fn edited(&mut self, game: &Game<'_>) {
        let Some((setting, text)) = self.focused_box().map(|(s, b)| (s, b.value.clone())) else {
            return;
        };
        match setting {
            Some(s) => {
                let valid = match kind(s) {
                    Kind::Double => text.trim().parse::<f64>().ok().filter(|v| (0.0..=1.0).contains(v)).map(pun::java_double),
                    _ => decode(&text).filter(|v| *v >= 0).map(|v| v.to_string()),
                };
                if let Some(v) = &valid
                    && glue::config(game).value(s) != *v
                {
                    self.set(game, &[(s, v.clone())]);
                }
                if let Some((_, b)) = self.focused_box() {
                    b.text_color = argb(if valid.is_some() { EDIT_TEXT } else { RED });
                }
            }
            None => {
                if let Some(i) = self.list_boxes.iter().position(|b| b.focused)
                    && self.list.get(i) != Some(&text)
                {
                    self.list_undo.push(self.list.clone());
                    self.list[i] = text;
                }
            }
        }
    }

    fn key(&mut self, game: &Game<'_>, key: &str, modifiers: u32) {
        let control = modifiers & 2 != 0;
        let shift = modifiers & 1 != 0;
        if key == "key.keyboard.escape" {
            self.back(game);
            return;
        }
        if let Some((_, b)) = self.focused_box() {
            let before = b.value.clone();
            if let Some(code) = horizon_client::keybinds::key_code(key) {
                b.key(code, control, shift);
            }
            b.reset_blink();
            if b.value != before {
                self.edited(game);
            }
        }
    }

    fn char_typed(&mut self, game: &Game<'_>, c: char) {
        if c.is_control() {
            return;
        }
        if let Some((_, b)) = self.focused_box() {
            b.insert_text(&c.to_string());
            b.reset_blink();
            self.edited(game);
        }
    }
}

/// `Integer.decode`: decimal, `0x`/`#` hexadecimal, a leading `0` octal.
fn decode(s: &str) -> Option<i32> {
    let (neg, s) = match s.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, s.strip_prefix('+').unwrap_or(s)),
    };
    let v = if let Some(h) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")).or_else(|| s.strip_prefix('#')) {
        i64::from_str_radix(h, 16).ok()?
    } else if s.len() > 1 && s.starts_with('0') {
        i64::from_str_radix(&s[1..], 8).ok()?
    } else {
        s.parse::<i64>().ok()?
    };
    i32::try_from(if neg { -v } else { v }).ok()
}

/// A row's label (`StringWidget` 150x20): left, white, cut with "..." past 150.
fn label_text(game: &Game<'_>, batch: &mut HudBatch, s: &str, r: [f32; 4]) {
    let atlas = &game.gui.atlas;
    let mut text = s.to_owned();
    if HudBatch::text_width(atlas, &text) > r[2] {
        let mut cut = String::new();
        for c in s.chars() {
            if HudBatch::text_width(atlas, &format!("{cut}{c}...")) > r[2] {
                break;
            }
            cut.push(c);
        }
        text = format!("{cut}...");
    }
    batch.text(atlas, &text, r[0], r[1] + 5.0, argb(WHITE), true);
}

/// The tooltip's lines wrapped at 170 pixels, each keeping its colour.
fn wrap_tooltip(game: &Game<'_>, lines: Vec<(String, u32)>) -> Vec<(String, u32)> {
    let atlas = &game.gui.atlas;
    let mut out = Vec::new();
    for (line, color) in lines {
        if line.is_empty() {
            out.push((line, color));
            continue;
        }
        let mut cur = String::new();
        for word in line.split(' ') {
            let next = if cur.is_empty() { word.to_owned() } else { format!("{cur} {word}") };
            if HudBatch::text_width(atlas, &next) <= TOOLTIP_WIDTH || cur.is_empty() {
                cur = next;
            } else {
                out.push((std::mem::take(&mut cur), color));
                cur = word.to_owned();
            }
        }
        out.push((cur, color));
    }
    out
}

// ---- the screen's hooks -----------------------------------------------------------------------

fn over(game: &Game<'_>, screen: &NativeScreenView<'_>) -> bool {
    screen.id == "options" && covers(game)
}

pub(super) fn render(game: &Game<'_>, batch: &mut HudBatch, screen: &NativeScreenView<'_>) {
    if !over(game, screen) {
        return;
    }
    SIZE.with(|s| s.set((batch.width, batch.height)));
    if let Some(tip) = with(|c| c.draw(game, batch)).flatten() {
        let lines: Vec<(String, u32)> = wrap_tooltip(game, tip).into_iter().map(|(l, c)| (l, argb(c))).collect();
        game.draw_tooltip_lines(batch, &lines, game.cursor.0, game.cursor.1);
    }
}

fn bit(button: i32) -> u8 {
    1 << button.clamp(0, 7)
}

pub(super) fn pressed(game: &Game<'_>, screen: &NativeScreenView<'_>, button: i32) -> bool {
    if !over(game, screen) {
        return false;
    }
    TAKEN.with(|t| t.set(t.get() | bit(button)));
    with(|c| c.press(game, (screen.mouse[0], screen.mouse[1]), button));
    true
}

pub(super) fn released(game: &Game<'_>, screen: &NativeScreenView<'_>, button: i32) -> bool {
    let taken = TAKEN.with(|t| {
        let was = t.get() & bit(button) != 0;
        t.set(t.get() & !bit(button));
        was
    });
    if !over(game, screen) {
        return taken;
    }
    with(ConfigScreen::release);
    true
}

pub(super) fn dragged(game: &Game<'_>, screen: &NativeScreenView<'_>) -> bool {
    if !over(game, screen) {
        return false;
    }
    with(|c| c.drag(game, (screen.mouse[0], screen.mouse[1])));
    true
}

pub(super) fn scrolled(game: &Game<'_>, screen: &NativeScreenView<'_>, scroll: [f64; 2]) -> bool {
    if !over(game, screen) {
        return false;
    }
    with(|c| c.scroll(scroll[1]));
    true
}

pub(super) fn key_pressed(game: &Game<'_>, screen: &NativeScreenView<'_>, key: &str, modifiers: u32) -> bool {
    if !over(game, screen) {
        return false;
    }
    with(|c| c.key(game, key, modifiers));
    true
}

pub(super) fn key_released(game: &Game<'_>, screen: &NativeScreenView<'_>) -> bool {
    over(game, screen)
}

pub(super) fn char_typed(game: &Game<'_>, screen: &NativeScreenView<'_>, c: char) -> bool {
    if !over(game, screen) {
        return false;
    }
    with(|s| s.char_typed(game, c));
    true
}

// ---- the icon and Mod Menu's metadata ---------------------------------------------------

/// What Mod Menu shows of the mod, from the original's `fabric.mod.json`
/// (environment "client": the Client badge).
fn modmenu_json() -> String {
    format!(
        r#"{{
  "schema": 1,
  "name": "{name}",
  "version": "{version}",
  "description": "Be notified about all the things you've just collected.",
  "authors": ["Fuzs"],
  "contributors": ["DeportedToHell"],
  "license": "MPL-2.0",
  "links": {{
    "homepage": "https://www.curseforge.com/minecraft/mc-mods/pick-up-notifier",
    "sources": "https://github.com/DeportedToHell/MCRE-Mods/tree/main/pickupnotifier",
    "issues": "https://github.com/DeportedToHell/MCRE-Mods/issues"
  }},
  "badges": ["client"],
  "config_screen": true
}}
"#,
        name = pun::NAME,
        version = pun::VERSION,
    )
}

/// The port's own icon, drawn by tools/icon.mjs.
const ICON: &[u8] = horizon_client::native_data!("pickupnotifier", "icon.png");

fn write_data(game: &Game<'_>) {
    let dir = game.native_data_dir(pun::ID);
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    // The port's own icon (tools/icon.mjs): the original's assets are All
    // Rights Reserved (LICENSE-ASSETS.md), as NOTICE says.
    let meta = modmenu_json();
    for (name, bytes) in [("icon.png", ICON), ("modmenu.json", meta.as_bytes())] {
        let path = dir.join(name);
        if std::fs::read(&path).ok().as_deref() != Some(bytes) {
            let _ = std::fs::write(&path, bytes);
        }
    }
}
