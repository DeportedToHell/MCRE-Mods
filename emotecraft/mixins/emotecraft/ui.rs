// Emotecraft's screens (`crate::game::emotecraft_ui`): the emote wheel (`FastMenuScreen`,
// `PreviewFastChooseWidget`, the square tiles and the old circle), All Emotes (`FullMenuScreen`,
// `EmoteListWidget` with its folders, search and preview), Emotes config (`EmoteMenu`: the key of
// an emote, the wheel's editor), Key Binds (`KeyBindsMenu`), the Mod Options (`ConfigScreen`), the
// Export Menu, the confirmations and Emotecraft's toasts, in the vanilla widgets' look. In a world
// they are the mod's own screen; from Options > Mods (and Mod Menu) they cover the options.
//
// This Source Code Form is a port of Emotecraft (https://github.com/KosmX/emotes), GPL-3.0.

use super::emotecraft_glue::{self as glue, Holder, ID, lock};
use super::emotecraft_icons as icons;
use super::emotecraft_lang::{self as lang, t, tf};
use super::emotecraft_preview::{self as preview, BASE_ID, Mannequin};
use super::{Game, Screen as GameScreen};
use emotecraft_engine::anim::{Animation, LoopType};
use emotecraft_engine::config::{self as cfg, Kind, V};
use emotecraft_engine::easing::Easing;
use horizon_client::hud::{HudBatch, rgba};
use horizon_client::mod_api::NativeScreenView;
use horizon_client::options_ui::Page;
use horizon_client::text::Text;
use horizon_client::ui::{self, EditBox};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

type Rect = [f32; 4];

const SPACING: f32 = 8.0;
const SMALL_WIDTH: f32 = 120.0;
const DEFAULT_WIDTH: f32 = 150.0;
const BIG_WIDTH: f32 = 200.0;
const HEADER: f32 = 33.0;
const UNBOUND: &str = "key.keyboard.unknown";

/// Preview model ids: the wheel's tiles, the editor's tiles, the lists' mannequin.
const WHEEL_IDS: u32 = BASE_ID;
const EDITOR_IDS: u32 = BASE_ID + 8;
const LIST_ID: u32 = BASE_ID + 16;

fn argb(c: u32) -> u32 {
    let [a, r, g, b] = c.to_be_bytes();
    rgba(r, g, b, a)
}

fn inside(p: (f32, f32), r: Rect) -> bool {
    p.0 >= r[0] && p.0 < r[0] + r[2] && p.1 >= r[1] && p.1 < r[1] + r[3]
}

fn fill(game: &Game<'_>, batch: &mut HudBatch, r: Rect, c: u32) {
    if r[2] > 0.0 && r[3] > 0.0 {
        batch.fill(&game.gui.atlas, r[0], r[1], r[2], r[3], argb(c));
    }
}

fn width(game: &Game<'_>, t: &Text) -> f32 {
    game.gui.atlas.font.width_text(t)
}

fn lit(s: impl Into<String>) -> Text {
    Text::literal(s.into())
}

fn button(game: &Game<'_>, batch: &mut HudBatch, cursor: (f32, f32), r: Rect, label: &Text, active: bool) {
    let hover = active && inside(cursor, r);
    ui::button(batch, &game.gui.atlas, r[0], r[1], r[2], r[3], label, active, hover);
}

/// A row of footer buttons (`HeaderAndFooterLayout`'s footer, a horizontal layout 8 apart).
fn footer(w: f32, h: f32, widths: &[f32]) -> Vec<Rect> {
    let total: f32 = widths.iter().sum::<f32>() + SPACING * (widths.len().max(1) - 1) as f32;
    let mut x = ((w - total) / 2.0).floor();
    let y = h - HEADER + ((HEADER - 20.0) / 2.0).floor();
    widths
        .iter()
        .map(|bw| {
            let r = [x, y, *bw, 20.0];
            x += bw + SPACING;
            r
        })
        .collect()
}

/// The title in the header (`addTitleHeader`).
fn title(game: &Game<'_>, batch: &mut HudBatch, w: f32, text: &Text) {
    let tw = width(game, text);
    batch.text_styled(&game.gui.atlas, text, ((w - tw) / 2.0).floor(), ((HEADER - 9.0) / 2.0).floor(), u32::MAX, true);
}

/// A texture tiled with an offset into it (`blit(…, u, v, w, h, 32, 32)`).
fn tile(game: &Game<'_>, batch: &mut HudBatch, sprite: &str, r: Rect, uv: (f32, f32), size: (f32, f32), color: u32) {
    let Some(s) = game.gui.atlas.sprites.get(sprite).copied() else { return };
    let (sw, sh) = size;
    let mut y = r[1];
    let mut v = uv.1.rem_euclid(sh);
    while y < r[1] + r[3] {
        let hh = (sh - v).min(r[1] + r[3] - y);
        let mut x = r[0];
        let mut u = uv.0.rem_euclid(sw);
        while x < r[0] + r[2] {
            let ww = (sw - u).min(r[0] + r[2] - x);
            let sub = horizon_client::hud::Rect {
                u0: s.u0 + (s.u1 - s.u0) * u / sw,
                v0: s.v0 + (s.v1 - s.v0) * v / sh,
                u1: s.u0 + (s.u1 - s.u0) * (u + ww) / sw,
                v1: s.v0 + (s.v1 - s.v0) * (v + hh) / sh,
                ..s
            };
            batch.rect_uv(x, y, ww, hh, sub, color);
            x += ww;
            u = 0.0;
        }
        y += hh;
        v = 0.0;
    }
}

fn in_world(game: &Game<'_>) -> bool {
    game.player.is_some()
}

fn list_bg_sprite(game: &Game<'_>) -> &'static str {
    if in_world(game) { "gui/inworld_menu_list_background" } else { "gui/menu_list_background" }
}

/// `WidgetOutliner.extractOutline`: the header and footer separators around a box.
fn outline(game: &Game<'_>, batch: &mut HudBatch, r: Rect, color: u32) {
    let world = in_world(game);
    let (head, foot) = if world { ("gui/inworld_header_separator", "gui/inworld_footer_separator") } else { ("gui/header_separator", "gui/footer_separator") };
    let c = argb(color);
    tile(game, batch, head, [r[0], r[1] - 1.0, r[2], 2.0], (0.0, 0.0), (32.0, 2.0), c);
    tile(game, batch, foot, [r[0], r[1] + r[3] - 1.0, r[2], 2.0], (0.0, 0.0), (32.0, 2.0), c);
    // The sides: the header separator turned a quarter.
    if let Some(s) = game.gui.atlas.sprites.get(head).copied() {
        let len = r[3];
        let v_end = s.v0 + (s.v1 - s.v0);
        let u = |k: f32| s.u0 + (s.u1 - s.u0) * (k / 32.0).min(1.0);
        let mut done = 0.0;
        while done < len {
            let seg = (32.0f32).min(len - done);
            // Left: rotated -90 (runs upward), at x - 1 .. x + 1.
            let (x, y) = (r[0] - 1.0, r[1] + done);
            batch.quad([[x, y + seg], [x, y], [x + 2.0, y], [x + 2.0, y + seg]], [[u(0.0), s.v0], [u(seg), s.v0], [u(seg), v_end], [u(0.0), v_end]], c);
            let x = r[0] + r[2] - 1.0;
            batch.quad([[x + 2.0, y], [x + 2.0, y + seg], [x, y + seg], [x, y]], [[u(0.0), s.v0], [u(seg), s.v0], [u(seg), v_end], [u(0.0), v_end]], c);
            done += seg;
        }
    }
}

/// A page arrow (`PageButton` with the recipe book's sprites), with a button behind it when
/// `background`.
fn page_button(game: &Game<'_>, batch: &mut HudBatch, cursor: (f32, f32), r: Rect, forward: bool, active: bool, background: bool) {
    let hover = active && inside(cursor, r);
    if background {
        ui::button(batch, &game.gui.atlas, r[0], r[1], r[2], r[3], &Text::empty(), active, hover);
    }
    let name = match (forward, hover) {
        (true, true) => "gui/sprites/recipe_book/page_forward_highlighted",
        (true, false) => "gui/sprites/recipe_book/page_forward",
        (false, true) => "gui/sprites/recipe_book/page_backward_highlighted",
        (false, false) => "gui/sprites/recipe_book/page_backward",
    };
    let (x, y) = if background { (r[0] + ((r[2] - 12.0) / 2.0).floor(), r[1] + ((r[3] - 17.0) / 2.0).floor()) } else { (r[0], r[1]) };
    let color = if active { u32::MAX } else { argb(0xFFA0A0A0) };
    ui::blit_sprite(batch, &game.gui.atlas, name, x, y, 12.0, 17.0, color);
}

// ---- toasts ---------------------------------------------------------------------------------

struct Toast {
    title: Text,
    message: Option<Text>,
    at: Instant,
}

static TOASTS: Mutex<Vec<Toast>> = Mutex::new(Vec::new());

/// `PlatformTools.addToast` (`EmotecraftToast`, 5 s).
pub(super) fn toast(title: Text, message: Text) {
    let message = (!message.is_empty()).then_some(message);
    lock(&TOASTS).push(Toast { title, message, at: Instant::now() });
}

/// Hook `gui_render_post`: the toasts, top right, sliding in and out as vanilla's do.
pub(super) fn draw_toasts(game: &Game<'_>, batch: &mut HudBatch) {
    let mut list = lock(&TOASTS);
    list.retain(|t| t.at.elapsed().as_millis() < 5600);
    let atlas = &game.gui.atlas;
    let mut y = f32::from(u16::try_from(game.toast_slots_used()).unwrap_or(0)) * 32.0;
    for t in list.iter() {
        let font = &atlas.font;
        let mut title: Vec<Text> = font.split_lines(&t.title, 200.0).into_iter().map(|(l, _)| l).collect();
        let mut message: Vec<Text> = t.message.as_ref().map(|m| font.split_lines(m, 200.0).into_iter().map(|(l, _)| l).collect()).unwrap_or_default();
        title.truncate(if message.is_empty() { 12 } else { 6 });
        message.truncate(12 - title.len());
        let lines = title.len() + message.len();
        let text_h = lines as f32 * 12.0;
        let h = text_h.max(32.0 - 4.0 - 4.0) + 4.0;
        let icon = h - 4.0;
        let text_x = 2.0 + icon + 2.0;
        let tw = title.iter().chain(&message).map(|l| font.width_text(l)).fold(0.0, f32::max);
        let w = (text_x + tw + 2.0).max(160.0);
        // `ToastInstance`: 600 ms in and out.
        let ms = t.at.elapsed().as_millis() as f32;
        let shown = if ms < 600.0 { ms / 600.0 } else if ms > 5000.0 { 1.0 - (ms - 5000.0) / 600.0 } else { 1.0 };
        let shown = shown.clamp(0.0, 1.0);
        let x = batch.width - w * (1.0 - (1.0 - shown).powi(3)).max(shown * shown);
        tile(game, batch, list_bg_sprite(game), [x, y, w, h], (0.0, 0.0), (32.0, 32.0), u32::MAX);
        outline(game, batch, [x, y, w, h], 0xFFFF_FFFF);
        let logo = Arc::new(glue::ICON.to_vec());
        icons::draw(game, batch, icons::LOGO, &logo, [x + 2.0, y + 2.0, icon, icon], u32::MAX);
        let mut ty = y + ((h - text_h) / 2.0).floor() + 1.0;
        for l in &title {
            batch.text_styled(atlas, l, x + text_x, ty, u32::MAX, false);
            ty += 12.0;
        }
        for l in &message {
            batch.text_styled(atlas, l, x + text_x, ty, argb(0xFFAAAAAA), false);
            ty += 12.0;
        }
        y += (((h + 4.0) / 32.0).ceil()) * 32.0;
    }
}

// ---- the emote list (`EmoteListWidget`) -----------------------------------------------------

/// An entry of the list.
#[derive(Clone)]
enum Entry {
    /// The folder path line on top (`HeaderEntry`).
    Header,
    Folder { path: Vec<String>, count: usize },
    Emote(Arc<Animation>),
}

/// `EmoteListWidget.LAST_OPENED_PATH`.
static LAST_PATH: Mutex<Vec<String>> = Mutex::new(Vec::new());

struct EmoteList {
    compact: bool,
    /// The emotes it lists (with the bound ones not in the list, for the editor).
    emotes: Vec<Arc<Animation>>,
    /// The open folder, from the main one down.
    open: Vec<String>,
    entries: Vec<Entry>,
    selected: Option<usize>,
    scroll: f32,
    last_scroll: Instant,
    dragging: bool,
}

impl EmoteList {
    fn new(compact: bool) -> EmoteList {
        EmoteList { compact, emotes: Vec::new(), open: Vec::new(), entries: Vec::new(), selected: None, scroll: 0.0, last_scroll: Instant::now() - std::time::Duration::from_secs(1), dragging: false }
    }

    fn folder_of(a: &Animation) -> Vec<String> {
        a.str("folderpath").filter(|f| !f.trim().is_empty()).map(|f| f.split('/').map(str::to_owned).collect()).unwrap_or_default()
    }

    /// `setEmotes`.
    fn set_emotes(&mut self, holders: &[Holder], show_invalid: bool, bound: &[Arc<Animation>]) {
        self.emotes = holders.iter().map(|h| h.anim.clone()).collect();
        if show_invalid {
            for b in bound {
                if !self.emotes.iter().any(|e| e.uuid == b.uuid) {
                    self.emotes.push(b.clone());
                }
            }
        }
        // The last opened folders, while they still exist.
        let path = lock(&LAST_PATH).clone();
        self.open.clear();
        for f in path {
            let mut p = self.open.clone();
            p.push(f);
            if self.emotes.iter().any(|e| Self::folder_of(e).starts_with(&p)) {
                self.open = p;
            } else {
                break;
            }
        }
        self.filter("");
    }

    fn name_of(e: &Entry) -> String {
        match e {
            Entry::Header => String::new(),
            Entry::Folder { path, .. } => path.last().cloned().unwrap_or_default(),
            Entry::Emote(a) => lang::plain(a.str("name").unwrap_or_default()),
        }
    }

    /// `filter`: the open folder's folders (then its emotes), by name; or, searching, every emote
    /// under it that matches.
    fn filter(&mut self, search: &str) {
        self.entries = vec![Entry::Header];
        self.selected = None;
        let open = self.open.clone();
        let under: Vec<&Arc<Animation>> = self.emotes.iter().filter(|e| Self::folder_of(e).starts_with(&open)).collect();
        if search.trim().is_empty() {
            let mut folders: Vec<Entry> = Vec::new();
            let mut emotes: Vec<Entry> = Vec::new();
            for e in &under {
                let f = Self::folder_of(e);
                if f.len() == open.len() {
                    emotes.push(Entry::Emote((*e).clone()));
                } else {
                    let mut path = open.clone();
                    path.push(f[open.len()].clone());
                    match folders.iter_mut().find(|x| matches!(x, Entry::Folder { path: p, .. } if *p == path)) {
                        Some(Entry::Folder { count, .. }) => *count += 1,
                        _ => folders.push(Entry::Folder { path, count: 1 }),
                    }
                }
            }
            let key = |e: &Entry| Self::name_of(e);
            folders.sort_by_key(key);
            emotes.sort_by_key(key);
            self.entries.extend(folders);
            self.entries.extend(emotes);
        } else {
            let s = search.to_lowercase();
            for e in under {
                let name = lang::plain(e.str("name").unwrap_or_default()).to_lowercase();
                let desc = lang::plain(e.str("description").unwrap_or_default()).to_lowercase();
                let author = emotecraft_engine::text::strip_codes(&lang::plain(e.str("author").unwrap_or_default()));
                if name.contains(&s) || desc.contains(&s) || author.trim().eq_ignore_ascii_case(search.trim()) {
                    self.entries.push(Entry::Emote(e.clone()));
                }
            }
        }
        self.scroll = 0.0;
    }

    fn open_folder(&mut self, path: Vec<String>) {
        self.open = path;
        *lock(&LAST_PATH) = self.open.clone();
        self.filter("");
    }

    fn back(&mut self) {
        self.open.pop();
        *lock(&LAST_PATH) = self.open.clone();
        self.filter("");
    }

    fn height_of(&self, i: usize) -> f32 {
        if matches!(self.entries[i], Entry::Header) { 13.0 } else { 36.0 }
    }

    fn content(&self) -> f32 {
        (0..self.entries.len()).map(|i| self.height_of(i)).sum::<f32>() + 4.0
    }

    fn scrolling(&self) -> bool {
        self.last_scroll.elapsed().as_millis() < 400
    }

    fn set_scroll(&mut self, v: f32, area: Rect) {
        let max = (self.content() - area[3]).max(0.0);
        let v = v.clamp(0.0, max);
        if v != self.scroll {
            self.last_scroll = Instant::now();
        }
        self.scroll = v;
    }

    /// The list's area, its rows' left and width.
    fn layout(&self, w: f32, top: f32, bottom: f32, x: f32, list_w: f32) -> (Rect, f32, f32) {
        let row_w = if self.compact { list_w } else { (list_w / 2.0).floor() };
        let left = x + (list_w / 2.0).floor() - (row_w / 2.0).floor();
        let _ = w;
        ([x, top, list_w, bottom - top], left, row_w)
    }

    /// Each entry's row: index, y, height.
    fn rows(&self, area: Rect) -> Vec<(usize, f32, f32)> {
        let mut y = area[1] + 4.0 - self.scroll.floor();
        (0..self.entries.len())
            .map(|i| {
                let h = self.height_of(i);
                let r = (i, y, h);
                y += h;
                r
            })
            .collect()
    }

    fn hovered(&self, area: Rect, left: f32, row_w: f32, cursor: (f32, f32)) -> Option<usize> {
        if !inside(cursor, area) {
            return None;
        }
        self.rows(area).into_iter().find(|(_, y, h)| cursor.0 >= left && cursor.0 < left + row_w && cursor.1 >= *y && cursor.1 < y + h).map(|r| r.0).filter(|i| *i != 0)
    }

    fn path_text(&self) -> Text {
        let mut t = lit("/ ");
        let main = t_text("emotecraft.folder.main");
        if self.open.is_empty() {
            return t.append(main.bold());
        }
        t = t.append(main).append(lit(" "));
        for (i, f) in self.open.iter().enumerate() {
            t = t.append(lit("/ "));
            if i + 1 == self.open.len() {
                t = t.append(lit(f.clone()).bold());
            } else {
                t = t.append(lit(format!("{f} ")));
            }
        }
        t
    }

    #[allow(clippy::too_many_arguments)]
    fn draw(&mut self, game: &Game<'_>, batch: &mut HudBatch, area: Rect, left: f32, row_w: f32, cursor: (f32, f32), focused: bool) {
        let atlas = &game.gui.atlas;
        let world = in_world(game);
        tile(game, batch, list_bg_sprite(game), area, (area[0], area[1]), (32.0, 32.0), u32::MAX);
        let start = batch.vertices.len();
        let hovered = self.hovered(area, left, row_w, cursor);
        let scrollable = self.content() > area[3];
        let max_x = left + row_w - if self.compact && scrollable { 7.0 } else { 0.0 };
        for (i, y, h) in self.rows(area) {
            if y + h < area[1] || y > area[1] + area[3] {
                continue;
            }
            let (cx, cy, ch) = (left + 2.0, y + 2.0, h - 4.0);
            if self.selected == Some(i) {
                let outline_c = if focused { 0xFFFF_FFFF } else { 0xFF80_8080 };
                let r = [left, y, if self.compact && scrollable { row_w - 8.0 } else { row_w }, h];
                fill(game, batch, r, outline_c);
                fill(game, batch, [r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0], 0xFF00_0000);
            }
            match &self.entries[i] {
                Entry::Header => {
                    let t = self.path_text();
                    let tw = width(game, &t);
                    let x = if self.compact { cx } else { cx + ((row_w - 4.0 - tw) / 2.0).floor().max(0.0) };
                    batch.text_styled(atlas, &t, x, cy, u32::MAX, true);
                }
                entry => {
                    let hover = hovered == Some(i);
                    if hover {
                        fill(game, batch, [cx - 1.0, cy - 1.0, max_x - cx + 1.0, ch + 2.0], 0x8042_4242);
                    }
                    let (name, desc, author, badges) = entry_texts(entry);
                    let line = 9.0;
                    let has_sub = !author.plain().is_empty();
                    let desc_blank = desc.plain().trim().is_empty();
                    let sub_y = if desc_blank { 12.0 } else { 23.0 };
                    let bottom = if has_sub { sub_y + line } else if !desc_blank { 12.0 + line } else { line };
                    let top = cy + ((ch - bottom) / 2.0).floor().max(0.0);
                    let text_x = cx + 33.0;
                    // Badges, right aligned (`BageUtils.extractBadges`).
                    let name_w = width(game, &name);
                    let max_badges = (max_x - name_w).max(max_x / 3.0) - text_x;
                    let mut bx = 3.0;
                    for b in &badges {
                        let tw = width(game, b) + 6.0;
                        if bx + tw + 3.0 < max_badges {
                            badge(game, batch, b, max_x - tw - bx, top, tw);
                            bx += tw + 3.0;
                        }
                    }
                    let name_right = max_x - bx;
                    let clip = batch.vertices.len();
                    batch.text_styled(atlas, &name, text_x, top, u32::MAX, true);
                    batch.clip_since(clip, text_x, top - 1.0, name_right, top + line + 1.0);
                    batch.text_styled(atlas, &desc, text_x, top + 12.0, argb(0xFF80_8080), true);
                    if has_sub {
                        let t = t_text("emotecraft.emote.author").with_color(0xFFAA00).append(author.clone());
                        batch.text_styled(atlas, &t, text_x, top + sub_y, argb(0xFF80_8080), true);
                    }
                    match entry {
                        Entry::Folder { .. } => {
                            let (key, bytes) = if hover { (icons::FOLDER_OPEN, glue::FOLDER_OPEN_ICON) } else { (icons::FOLDER, glue::FOLDER_ICON) };
                            icons::draw(game, batch, key, &Arc::new(bytes.to_vec()), [left, cy, 32.0, 32.0], u32::MAX);
                        }
                        Entry::Emote(a) => {
                            if let Some(icon) = &a.icon {
                                icons::draw(game, batch, a.uuid, icon, [cx, cy, 32.0, 32.0], u32::MAX);
                            }
                        }
                        Entry::Header => {}
                    }
                }
            }
        }
        batch.clip_since(start, area[0], area[1], area[0] + area[2], area[1] + area[3]);
        // The separators over and under the list.
        let (head, foot) = if world { ("gui/inworld_header_separator", "gui/inworld_footer_separator") } else { ("gui/header_separator", "gui/footer_separator") };
        tile(game, batch, head, [area[0], area[1] - 2.0, area[2], 2.0], (0.0, 0.0), (32.0, 2.0), u32::MAX);
        tile(game, batch, foot, [area[0], area[1] + area[3], area[2], 2.0], (0.0, 0.0), (32.0, 2.0), u32::MAX);
        if scrollable {
            let x = if self.compact { left + row_w - 6.0 } else { left + row_w + 6.0 };
            let content = self.content();
            let bar = ((area[3] * area[3] / content).floor()).clamp(32.0, area[3] - 8.0);
            let y = area[1] + (self.scroll / (content - area[3]) * (area[3] - bar)).floor();
            ui::blit_sprite(batch, atlas, "gui/sprites/widget/scroller_background", x, area[1], 6.0, area[3], u32::MAX);
            ui::blit_sprite(batch, atlas, "gui/sprites/widget/scroller", x, y, 6.0, bar, u32::MAX);
        }
    }
}

fn t_text(key: &str) -> Text {
    lit(t(key))
}

/// An entry's name, description, author and badges.
fn entry_texts(e: &Entry) -> (Text, Text, Text, Vec<Text>) {
    match e {
        Entry::Header => (Text::empty(), Text::empty(), Text::empty(), Vec::new()),
        Entry::Folder { path, count } => (
            lit(path.last().cloned().unwrap_or_default()),
            t_text("emotecraft.folder"),
            Text::empty(),
            vec![lit(tf("emotecraft.folder.emotes", &[&count.to_string()])).with_color(0xAAAAAA)],
        ),
        Entry::Emote(a) => {
            let badges = match a.extra.get("bages") {
                Some(emotecraft_engine::anim::Extra::List(l)) => l.iter().filter_map(|b| b.as_str()).map(lang::component).collect(),
                _ => Vec::new(),
            };
            (lang::component(a.str("name").unwrap_or_default()), lang::component(a.str("description").unwrap_or_default()), lang::component(a.str("author").unwrap_or_default()), badges)
        }
    }
}

/// `BageUtils.extractBadge`.
fn badge(game: &Game<'_>, batch: &mut HudBatch, text: &Text, x: f32, y: f32, w: f32) {
    let color = text.spans.first().and_then(|s| s.style.color).map_or(0xFFFF_FFFF, |c| 0xFF00_0000 | c);
    let dark = {
        let [a, r, g, b] = color.to_be_bytes();
        u32::from_be_bytes([a, r / 4, g / 4, b / 4])
    };
    let lh = 9.0;
    fill(game, batch, [x + 1.0, y - 1.0, w - 1.0, 1.0], color);
    fill(game, batch, [x, y, 1.0, lh], color);
    fill(game, batch, [x + 1.0, y + lh, w - 1.0, 1.0], color);
    fill(game, batch, [x + w, y, 1.0, lh], color);
    fill(game, batch, [x + 1.0, y, w - 1.0, lh], dark);
    let tw = width(game, text);
    batch.text_styled(&game.gui.atlas, text, (x + 1.0 + (w - tw) / 2.0).floor(), y + 1.0, u32::MAX, false);
}

// ---- the wheel ------------------------------------------------------------------------------

/// The current page of every wheel (`AbstractFastChooseWidget.currentPage`).
static PAGE: Mutex<usize> = Mutex::new(0);

fn page() -> usize {
    *lock(&PAGE) % cfg::PAGES
}

/// A wheel (`PreviewFastChooseWidget`) on a screen.
struct Wheel {
    animated: bool,
    anim_time: f32,
    scroll_acc: f64,
    /// Its tiles' preview ids.
    ids: u32,
}

impl Wheel {
    fn new(animated: bool, ids: u32) -> Wheel {
        Wheel { animated, anim_time: 1.0, scroll_acc: 0.0, ids }
    }
    fn progress(&self) -> f32 {
        let p = if self.animated { self.anim_time } else { 0.0 };
        1.0 - Easing::InCirc.apply_simple(0.0, 1.0, p)
    }
    /// The tiles' rectangles and their direction (`updateRectangle`, `getDirectionVector`).
    fn tiles(&self, r: Rect, old: bool) -> Vec<(Rect, (f32, f32))> {
        let size = r[2];
        let s = (size / 8.0).floor();
        let eased = self.progress();
        let (cx, cy) = (r[0] + (size / 2.0), r[1] + (size / 2.0));
        if old {
            (0..8)
                .map(|i| {
                    let angle = (i as f32 * 45.0).to_radians();
                    let x = ((r[0] + (size / 2.0).floor()) + size * 0.36 * (i as f32 * 45.0 * 0.0174533).sin() * eased).trunc() - s;
                    let y = ((r[1] + (size / 2.0).floor()) + size * 0.36 * (i as f32 * 45.0 * 0.0174533).cos() * eased).trunc() - s;
                    ([x, y, s * 2.0, s * 2.0], (angle.sin(), angle.cos()))
                })
                .collect()
        } else {
            let dirs = [(-1.0, -1.0), (0.0, -1.0), (1.0, -1.0), (-1.0, 0.0), (1.0, 0.0), (-1.0, 1.0), (0.0, 1.0), (1.0, 1.0)];
            let distance = s * 2.5 * eased;
            dirs.iter()
                .map(|&(dx, dy)| {
                    let x = (cx + dx * distance).round() - s;
                    let y = (cy + dy * distance).round() - s;
                    ([x, y, s * 2.0, s * 2.0], (dx, dy))
                })
                .collect()
        }
    }
    fn page_buttons(&self, game: &Game<'_>, r: Rect) -> (Rect, Rect) {
        let (cx, cy) = (r[0] + (r[2] / 2.0).floor(), r[1] + (r[3] / 2.0).floor());
        let text = lit((page() + 1).to_string());
        let tw = width(game, &text);
        let pad = (((12.0 + tw) / 2.0).floor() + 2.0).max((r[2] / 8.0).floor());
        ([cx - 6.0 + pad, cy - 8.0, 12.0, 17.0], [cx - 6.0 - pad, cy - 8.0, 12.0, 17.0])
    }

    /// `PlayerChooseElement.tick` for every tile.
    fn tick(&mut self, mannequins: &mut HashMap<u32, Mannequin>, hovered: Option<usize>, slots: &[Option<Arc<Animation>>; 8]) {
        if self.animated {
            self.anim_time = (self.anim_time - 0.15).max(0.0);
        }
        for (i, slot) in slots.iter().enumerate() {
            let m = mannequins.entry(self.ids + i as u32).or_default();
            let (tick, again) = match slot {
                Some(a) => (a.preview_tick(), a.loop_type.plays_again()),
                None => (1.0, true),
            };
            let updated = m.play(slot.as_ref(), if again { None } else { Some(LoopType::Loop) }, true, tick);
            let hover = hovered == Some(i);
            m.pause(!updated && !hover);
            if updated || hover {
                m.tick(true);
            }
        }
    }

    /// Draws the wheel: the page, the arrows, the tiles with their previews, numbers and tooltip.
    #[allow(clippy::too_many_arguments)]
    fn draw(&self, game: &Game<'_>, batch: &mut HudBatch, r: Rect, cursor: (f32, f32), slots: &[Option<Arc<Animation>>; 8], numbers: bool, hover_parts: bool, mannequins: &mut HashMap<u32, Mannequin>, tip: &mut Option<Vec<Text>>) {
        let atlas = &game.gui.atlas;
        let old = glue::config(|c| c.b("oldChooseWheel"));
        let icons_only = glue::config(|c| c.b("showIconsIfPossible"));
        let (cx, cy) = (r[0] + (r[2] / 2.0).floor(), r[1] + (r[3] / 2.0).floor());
        let text = lit((page() + 1).to_string());
        let tw = width(game, &text);
        batch.text_styled(atlas, &text, cx - (tw / 2.0).floor(), cy - 4.0, u32::MAX, false);
        let (fwd, back) = self.page_buttons(game, r);
        page_button(game, batch, cursor, fwd, true, true, false);
        page_button(game, batch, cursor, back, false, true, false);
        let eased = self.progress();
        for (i, (tr, dir)) in self.tiles(r, old).into_iter().enumerate() {
            let emote = slots[i].as_ref();
            let hover = inside(cursor, tr);
            // `extractBackground`: the list background and an outline.
            tile(game, batch, list_bg_sprite(game), [tr[0] + 1.0, tr[1] + 1.0, tr[2] - 2.0, tr[3] - 2.0], (tr[0] + tr[2], tr[1] + tr[3]), (32.0, 32.0), u32::MAX);
            outline(game, batch, tr, 0xFFFF_FFFF);
            if hover && hover_parts {
                fill(game, batch, tr, 0x8042_4242);
            }
            if numbers && emote.is_some() {
                // `extractTileId`.
                let len = (dir.0 * dir.0 + dir.1 * dir.1).sqrt();
                let (nx, ny) = (dir.0 / len, dir.1 / len);
                let t = 1.0 - eased;
                let (c0x, c0y) = (tr[0] + tr[2] * 0.5, tr[1] + tr[3] * 0.5);
                let (c1x, c1y) = (r[0] + r[2] * 0.5, r[1] + r[3] * 0.5);
                let (px, py) = (c0x + (c1x - c0x) * t, c0y + (c1y - c0y) * t);
                let (hw, hh) = (tr[2] * 0.5, tr[3] * 0.5);
                let edge = (hw / nx.abs()).min(hh / ny.abs());
                let offset = edge + 14.0 + tr[2].min(tr[3]) * 0.25 * t;
                let s = (i + 1).to_string();
                let sw = HudBatch::text_width(atlas, &s);
                batch.text(atlas, &s, (px + nx * offset - sw / 2.0).round(), (py + ny * offset - 4.5).round(), u32::MAX, false);
            }
            let id = self.ids + i as u32;
            if let Some(a) = emote
                && icons_only
                && let Some(icon) = &a.icon
                && icons::readable(a.uuid)
            {
                icons::draw(game, batch, a.uuid, icon, tr, u32::MAX);
            } else if emote.is_some() || !icons_only {
                let m = mannequins.entry(id).or_default();
                let size = (m.alpha * (tr[3] / 2.0).floor()).round();
                // The mannequin faces out (the tile's middle stands for the pointer).
                let center = (tr[0] + (tr[2] / 2.0).floor(), tr[1] + (tr[3] / 2.0).floor());
                preview::draw(game, batch, id, tr, size, 0.0625, center);
                m.after_draw();
            }
            if hover && let Some(a) = emote {
                *tip = Some(vec![lang::component(a.str("name").unwrap_or_default())]);
            }
        }
    }

    /// The tile under the cursor.
    fn tile_at(&self, r: Rect, cursor: (f32, f32)) -> Option<usize> {
        let old = glue::config(|c| c.b("oldChooseWheel"));
        self.tiles(r, old).iter().position(|(tr, _)| inside(cursor, *tr))
    }

    fn turn(&mut self, forward: bool) {
        let mut p = lock(&PAGE);
        *p = if forward { (*p + 1) % cfg::PAGES } else { (*p + cfg::PAGES - 1) % cfg::PAGES };
    }

    /// `mouseScrolled`: two units of horizontal scroll a page.
    fn scroll(&mut self, x: f64) -> bool {
        self.scroll_acc += x;
        let mut turned = false;
        while self.scroll_acc >= 2.0 {
            self.scroll_acc -= 2.0;
            self.turn(true);
            turned = true;
        }
        while self.scroll_acc <= -2.0 {
            self.scroll_acc += 2.0;
            self.turn(false);
            turned = true;
        }
        turned
    }
}

fn wheel_slots() -> [Option<Arc<Animation>>; 8] {
    glue::config(|c| c.fast_menu[page()].clone())
}

// ---- the screens ----------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum ListKind {
    /// All Emotes (`FullMenuScreen`).
    Full,
    /// Emotes config (`EmoteMenu`).
    Menu,
}

struct ListScreen {
    kind: ListKind,
    list: EmoteList,
    search: EditBox,
    search_focused: bool,
    /// `activeKeyTime`: ticks left waiting for a key.
    key_wait: i64,
    wheel: Wheel,
    /// The emote the preview plays (`previewed`).
    previewed: Option<u128>,
    /// After playing from All Emotes opened from the wheel, closing goes back to the game.
    close_all: bool,
}

struct KeysScreen {
    selected: Option<String>,
    scroll: f32,
    previewed: Option<u128>,
}

struct OptionsScreen {
    scroll: f32,
    sliding: Option<&'static str>,
}

#[derive(Clone)]
enum ConfirmKind {
    ResetConfig,
    ResetKeys,
    /// Bind this emote to this key over the one bound to it.
    Overwrite(Arc<Animation>, String),
}

enum Scr {
    Wheel(Wheel),
    List(Box<ListScreen>),
    Keys(KeysScreen),
    Options(OptionsScreen),
    Export,
    Confirm(ConfirmKind),
}

enum Host {
    World,
    /// Over Options > Mods (the mod's page row), opened by Mod Menu or not.
    Options { row: usize, modmenu: bool },
}

/// What needs the game, done in `frame`.
enum Req {
    Click,
    CloseAll,
    OpenFolder,
    Export,
    Reload,
}

struct Ui {
    stack: Vec<Scr>,
    host: Host,
    reqs: Vec<Req>,
    mannequins: HashMap<u32, Mannequin>,
    /// The wheel key went down to open the wheel (HOLD closes on its release).
    opened_with: Option<String>,
}

thread_local! {
    static UI: RefCell<Ui> = RefCell::new(Ui { stack: Vec::new(), host: Host::World, reqs: Vec::new(), mannequins: HashMap::new(), opened_with: None });
}

fn with<R>(f: impl FnOnce(&mut Ui) -> R) -> Option<R> {
    UI.with(|u| u.try_borrow_mut().ok().map(|mut u| f(&mut u)))
}

fn new_list(kind: ListKind) -> Box<ListScreen> {
    let mut search = EditBox::new(0.0, 0.0, 200.0, 20.0);
    search.focused = false;
    search.max_length = 256;
    let mut s = Box::new(ListScreen { kind, list: EmoteList::new(kind == ListKind::Menu), search, search_focused: false, key_wait: 0, wheel: Wheel::new(false, EDITOR_IDS), previewed: None, close_all: false });
    refill(&mut s);
    s
}

/// `addOptions`: the emotes (and, for the editor, the bound ones not in the list).
fn refill(s: &mut ListScreen) {
    let holders = glue::list();
    let bound: Vec<Arc<Animation>> = glue::config(|c| c.keys.values().cloned().collect());
    s.list.set_emotes(&holders, s.kind == ListKind::Menu, &bound);
    let text = s.search.value.clone();
    if !text.trim().is_empty() {
        s.list.filter(&text);
    }
}

/// The emotes were loaded again: the lists show them.
pub(super) fn emotes_reloaded() {
    with(|u| {
        for s in u.stack.iter_mut() {
            if let Scr::List(l) = s {
                refill(l);
            }
        }
    });
}

fn is_open() -> bool {
    with(|u| !u.stack.is_empty()).unwrap_or(false)
}

/// Opens the wheel from play (`new FastMenuScreen(null)`).
pub(super) fn open_wheel(game: &mut Game<'_>) {
    let key = game.native_mod_key(ID, "fastchoose", "key.keyboard.b");
    if !game.native_mod_screen_open(ID) {
        return;
    }
    with(|u| {
        u.stack = vec![Scr::Wheel(Wheel::new(true, WHEEL_IDS))];
        u.host = Host::World;
        u.opened_with = Some(key);
    });
    tick_screens();
}

/// Configure... on the mod's page, or Mod Menu's: the Emotes config screen over the options.
pub(super) fn open_over_options(game: &mut Game<'_>, modmenu: bool) {
    let Some(row) = game.settings.mods.installed.iter().position(|m| m.id == ID && m.native.is_some()) else { return };
    with(|u| {
        u.stack = vec![Scr::List(new_list(ListKind::Menu))];
        u.host = Host::Options { row, modmenu };
        // `EmoteMenu` reloads the emotes when it opens.
        u.reqs.push(Req::Reload);
    });
}

/// The screens are up over the options page.
fn covers(game: &Game<'_>) -> bool {
    let row = with(|u| match u.host {
        Host::Options { row, .. } if !u.stack.is_empty() => Some(row),
        _ => None,
    })
    .flatten();
    row.is_some_and(|row| game.screen == GameScreen::Options && game.ui.options.page() == Page::NativeMod(row))
}

/// HEAD of `draw_options`: the options stay hidden under the screens.
pub(super) fn options_covered(game: &Game<'_>) -> bool {
    covers(game)
}

fn mine(game: &Game<'_>, screen: &NativeScreenView<'_>) -> bool {
    if !is_open() {
        return false;
    }
    let world = with(|u| matches!(u.host, Host::World)).unwrap_or(false);
    if world { screen.id == "mod" && game.native_mod_screen() == Some(ID) } else { screen.id == "options" && covers(game) }
}

fn pop(u: &mut Ui) {
    // `EmoteSubScreen.onClose`: not while the emotes reload.
    if matches!(u.stack.last(), Some(Scr::List(_))) && glue::loading() {
        toast(lit(glue::NAME), t_text("emotecraft.reloading.wait"));
        return;
    }
    u.stack.pop();
    if u.stack.is_empty() {
        u.reqs.push(Req::CloseAll);
    }
}

// ---- once a tick and a frame ----------------------------------------------------------------

fn tick_screens() {
    with(|u| {
        let Ui { stack, mannequins, .. } = u;
        let Some(top) = stack.last_mut() else {
            mannequins.clear();
            return;
        };
        let slots = wheel_slots();
        match top {
            Scr::Wheel(w) => {
                let hovered = HOVER.with(|h| h.get());
                w.tick(mannequins, hovered, &slots);
            }
            Scr::List(l) => {
                if l.key_wait == 1 {
                    l.key_wait = 0;
                } else if l.key_wait != 0 {
                    l.key_wait -= 1;
                }
                if l.kind == ListKind::Menu {
                    let hovered = HOVER.with(|h| h.get());
                    l.wheel.tick(mannequins, hovered, &slots);
                }
                let m = mannequins.entry(LIST_ID).or_default();
                m.tick(true);
            }
            Scr::Keys(_) => {
                let m = mannequins.entry(LIST_ID).or_default();
                m.tick(true);
            }
            _ => {}
        }
    });
}

thread_local! {
    /// The wheel tile under the cursor at the last draw.
    static HOVER: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
    /// The list entry under the cursor at the last draw (for the preview).
    static LIST_HOVER: RefCell<Option<Entry>> = const { RefCell::new(None) };
}

/// What the emotes' folder holds (its files' names, sizes and times), to see it change.
fn folder_signature() -> u64 {
    let dir = glue::client(|c| c.emotes_dir.clone());
    let mut h: u64 = 1469598103934665603;
    let mut stack = vec![(dir, 0)];
    while let Some((d, depth)) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            let Ok(meta) = e.metadata() else { continue };
            if meta.is_dir() {
                if depth < 16 {
                    stack.push((p, depth + 1));
                }
                continue;
            }
            let modified = meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_millis() as u64);
            for b in p.to_string_lossy().bytes().chain(meta.len().to_le_bytes()).chain(modified.to_le_bytes()) {
                h = (h ^ u64::from(b)).wrapping_mul(1099511628211);
            }
        }
    }
    h
}

thread_local! {
    /// The folder's last signature and the ticks since it was read (`EmoteListener`).
    static WATCH: std::cell::Cell<(Option<u64>, u32)> = const { std::cell::Cell::new((None, 0)) };
}

/// Hook `tick` (the window's): the screens' animations and previews, and the emotes'
/// folder watched while a list is up.
pub(super) fn tick(_game: &mut Game<'_>) {
    tick_screens();
    let listing = with(|u| matches!(u.stack.last(), Some(Scr::List(_)))).unwrap_or(false);
    if listing {
        let (last, n) = WATCH.with(std::cell::Cell::get);
        if n >= 20 {
            let now = folder_signature();
            WATCH.with(|w| w.set((Some(now), 0)));
            if last.is_some_and(|l| l != now) && !glue::loading() {
                with(|u| u.reqs.push(Req::Reload));
            }
        } else {
            WATCH.with(|w| w.set((last, n + 1)));
        }
    } else {
        WATCH.with(|w| w.set((None, 20)));
    }
    // The list's preview plays what is hovered (`EmoteSubScreen.tick`).
    let hovered = LIST_HOVER.with(|h| h.borrow().clone());
    with(|u| {
        let Ui { stack, mannequins, .. } = u;
        let m = mannequins.entry(LIST_ID).or_default();
        match stack.last_mut() {
            Some(Scr::List(l)) => {
                if l.list.scrolling() {
                    l.previewed = None;
                    m.ctrl.stop();
                } else {
                    match hovered {
                        Some(Entry::Emote(a)) => {
                            if l.previewed != Some(a.uuid) {
                                l.previewed = Some(a.uuid);
                                let check = m.playing();
                                m.play(Some(&a), None, check, 0.0);
                            }
                        }
                        Some(Entry::Folder { .. }) => {
                            l.previewed = None;
                            m.ctrl.stop();
                        }
                        _ => l.previewed = None,
                    }
                }
            }
            Some(Scr::Keys(k)) => match hovered {
                Some(Entry::Emote(a)) => {
                    if k.previewed != Some(a.uuid) {
                        k.previewed = Some(a.uuid);
                        let check = m.playing();
                        m.play(Some(&a), None, check, 0.0);
                    }
                }
                _ => k.previewed = None,
            },
            _ => {}
        }
    });
}

/// The previews' poses this frame, beside the avatars'.
pub(super) fn preview_frames(frames: &mut glue::Frames) {
    with(|u| {
        for (id, m) in u.mannequins.iter_mut() {
            if m.ctrl.is_active() && m.ctrl.process(0.0)
                && let Some(f) = m.ctrl.frame()
            {
                frames.insert(*id, Arc::new(glue::FrameAdapter(f)));
            }
        }
    });
}

/// Hook `frame`: what the screens asked for, and the screens gone when their game screen is.
pub(super) fn frame(game: &mut Game<'_>) {
    super::emotecraft_lang::set_language(&game.settings.language);
    icons::frame(game);
    let world = with(|u| matches!(u.host, Host::World)).unwrap_or(true);
    if is_open() {
        let gone = if world { game.native_mod_screen() != Some(ID) } else { !covers(game) };
        if gone {
            with(|u| u.stack.clear());
            glue::save_config();
        }
    }
    let reqs = with(|u| std::mem::take(&mut u.reqs)).unwrap_or_default();
    for r in reqs {
        match r {
            Req::Click => game.audio.play_button_click(),
            Req::CloseAll => {
                glue::save_config();
                let host = with(|u| std::mem::replace(&mut u.host, Host::World));
                match host {
                    Some(Host::Options { row, modmenu }) => {
                        if modmenu {
                            let events = game.ui.options.back(&mut game.settings);
                            let _ = game.handle_option_events(events);
                        } else {
                            game.native_settings_refresh(row);
                        }
                    }
                    _ => game.native_mod_screen_close(ID),
                }
            }
            Req::OpenFolder => {
                let dir = glue::client(|c| c.emotes_dir.clone());
                let _ = std::fs::create_dir_all(&dir);
                super::packs::open_folder(&dir);
            }
            Req::Export => export(game),
            Req::Reload => glue::reload_emotes(true),
        }
    }
}

/// `ExportMenu.exportEmotesInFormat` with the binary writer.
fn export(game: &Game<'_>) {
    let dir = glue::client(|c| c.emotes_dir.join("emotecraft_export"));
    let _ = std::fs::create_dir_all(&dir);
    for h in glue::list() {
        if h.anim.is_builtin() {
            continue;
        }
        if emotecraft_engine::emotes::export(&h.anim, &dir).is_err() {
            toast(lit(tf("emotecraft.export.error", &["emotecraft"])), lang::component(h.anim.str("name").unwrap_or_default()));
        }
    }
    let rel = dir.strip_prefix(game.native_instance_dir()).map(|p| p.to_string_lossy().replace('\\', "/")).unwrap_or_else(|_| dir.to_string_lossy().into_owned());
    toast(lit(tf("emotecraft.export.done", &["emotecraft"])), lit(rel));
}

/// Any key while the screens are up (the `key` hook): the wheel closes on the wheel key's
/// release when it closes on HOLD.
pub(super) fn key_anywhere(game: &mut Game<'_>, key: &str, pressed: bool) {
    if pressed || !is_open() {
        return;
    }
    let hold = glue::config(|c| c.s("closeWheelType")) == "HOLD";
    let wheel_key = game.native_mod_key(ID, "fastchoose", "key.keyboard.b");
    if hold && key == wheel_key {
        with(|u| {
            if matches!(u.stack.last(), Some(Scr::Wheel(_))) && u.stack.len() == 1 {
                pop(u);
            }
        });
    }
}

// ---- drawing ------------------------------------------------------------------------------

/// Hook `screen_render_post`: the top screen.
pub(super) fn render(game: &Game<'_>, batch: &mut HudBatch, screen: &NativeScreenView<'_>) {
    if !mine(game, screen) {
        return;
    }
    let (w, h) = (screen.rect[2].floor(), screen.rect[3].floor());
    let cursor = (screen.mouse[0], screen.mouse[1]);
    with(|u| {
        let world = in_world(game);
        let mut tip: Option<Vec<Text>> = None;
        let Ui { stack, mannequins, .. } = u;
        let n = stack.len();
        // A confirmation is drawn over the screen under it.
        let base = if matches!(stack.last(), Some(Scr::Confirm(_))) && n >= 2 { n - 2 } else { n - 1 };
        let over = n - 1 != base;
        let cur = if over { (-1.0, -1.0) } else { cursor };
        match &mut stack[base] {
            Scr::Wheel(wh) => draw_wheel_screen(game, batch, w, h, cur, wh, mannequins, &mut tip),
            Scr::List(l) => draw_list_screen(game, batch, w, h, cur, l, mannequins, world, &mut tip),
            Scr::Keys(k) => draw_keys(game, batch, w, h, cur, k, mannequins, world, &mut tip),
            Scr::Options(o) => draw_options(game, batch, w, h, cur, o, world, &mut tip),
            Scr::Export => draw_export(game, batch, w, h, cur, world),
            Scr::Confirm(_) => {}
        }
        if over && let Some(Scr::Confirm(c)) = stack.last() {
            draw_confirm(game, batch, w, h, cursor, c, world);
        } else if let Some(lines) = tip {
            ui::tooltip(batch, &game.gui.atlas, &lines, cursor);
        }
    });
}

fn background(game: &Game<'_>, batch: &mut HudBatch, w: f32, h: f32, world: bool) {
    // `Screen.extractBackground`: the blur, then the menu background.
    game.native_apply_screen_blur();
    ui::menu_background(batch, &game.gui.atlas, 0.0, 0.0, w, h, world);
}

/// The wheel screen's layout: header height and the wheel's box.
fn wheel_layout(game: &Game<'_>, w: f32, h: f32) -> (f32, Option<Vec<Text>>, Rect) {
    let (header, lines) = match glue::server_state() {
        glue::ServerState::Fine => (0.0, None),
        glue::ServerState::Different => {
            let t = lit(t("emotecraft.different_server")).with_color(0xFF5555);
            let lines: Vec<Text> = game.gui.atlas.font.split_lines(&t, (w / 1.2).ceil()).into_iter().map(|(l, _)| l).take(3).collect();
            ((lines.len() as f32 * 9.0).max(18.0), Some(lines))
        }
        glue::ServerState::None => (18.0, Some(vec![lit(t("emotecraft.no_server")).with_color(0xFF5555)])),
    };
    let content = h - header - HEADER;
    let size = (w * 0.8).min(content * 0.8).round().min(512.0);
    let rect = [((w - size) / 2.0).floor(), header + ((content - size) / 2.0).floor(), size, size];
    (header, lines, rect)
}

#[allow(clippy::too_many_arguments)]
fn draw_wheel_screen(game: &Game<'_>, batch: &mut HudBatch, w: f32, h: f32, cursor: (f32, f32), wh: &mut Wheel, mannequins: &mut HashMap<u32, Mannequin>, tip: &mut Option<Vec<Text>>) {
    let atlas = &game.gui.atlas;
    // No blur, no background (`extractBlurredBackground` is a no-op).
    let (header, lines, rect) = wheel_layout(game, w, h);
    if let Some(lines) = lines {
        let mut y = ((header - lines.len() as f32 * 9.0) / 2.0).floor();
        for l in &lines {
            let tw = width(game, l);
            batch.text_styled(atlas, l, ((w - tw) / 2.0).floor(), y, u32::MAX, true);
            y += 9.0;
        }
    }
    let slots = wheel_slots();
    let nav = glue::config(|c| c.b("enableWheelKeyboardNav"));
    wh.draw(game, batch, rect, cursor, &slots, nav, true, mannequins, tip);
    HOVER.with(|c| c.set(wh.tile_at(rect, cursor)));
    for (i, r) in wheel_footer(w, h).into_iter().enumerate() {
        let (label, active) = wheel_footer_label(i);
        button(game, batch, cursor, r, &label, active);
    }
}

fn online_debug() -> bool {
    glue::config(|c| c.b("onlineDebug"))
}

fn wheel_footer(w: f32, h: f32) -> Vec<Rect> {
    if online_debug() { footer(w, h, &[SMALL_WIDTH, SMALL_WIDTH, SMALL_WIDTH]) } else { footer(w, h, &[SMALL_WIDTH, SMALL_WIDTH]) }
}

fn wheel_footer_label(i: usize) -> (Text, bool) {
    let i = if online_debug() { i } else { i + 1 };
    match i {
        // Online Emotes' service isn't part of this port: its button can never reconnect.
        0 => (t_text("emotecraft.online.reconnect"), false),
        1 => (Text::translate("gui.cancel", &[]), true),
        _ => (t_text("emotecraft.emotelist"), true),
    }
}

/// The list screens' layout: the list area, its rows' left and width, the preview's box.
fn list_layout(s: &ListScreen, w: f32, h: f32) -> (Rect, f32, f32, Rect, bool) {
    let top = HEADER;
    let bottom = h - HEADER;
    let content_h = bottom - top;
    let (area, left, row_w) = if s.kind == ListKind::Menu {
        let list_w = (w / 3.0).floor();
        let wheel = (w / 2.5).min(h / 2.3).round().min(256.0);
        let grid_w = (SMALL_WIDTH * 2.0 + SPACING / 3.0).max(wheel);
        let total = list_w + SPACING + grid_w + 8.0;
        let x = ((w - total) / 2.0).floor().max(0.0);
        s.list.layout(w, top, bottom, x, list_w)
    } else {
        s.list.layout(w, top, bottom, 0.0, w)
    };
    // `repositionPreview(list.getRowLeft())`.
    let preview_h = (h / 2.0).floor().min(256.0);
    let space = left - SPACING * 2.0;
    let preview_w = space.clamp(0.0, preview_h);
    let visible = space >= (preview_h / 3.0).floor();
    let preview = [SPACING + ((space - preview_w) / 2.0).floor(), top + ((content_h - preview_h) / 2.0).floor(), preview_w, preview_h];
    (area, left, row_w, preview, visible)
}

/// The editor's right side: key text, key button, Key Binds, the help, the wheel.
fn menu_layout(game: &Game<'_>, s: &ListScreen, w: f32, h: f32) -> (f32, Vec<Text>, Rect, Rect, Vec<Text>, f32, Rect) {
    let (area, ..) = list_layout(s, w, h);
    let x = area[0] + area[2] + SPACING + 4.0;
    let font = &game.gui.atlas.font;
    let key_lines: Vec<Text> = font.split_lines(&t_text("emotecraft.options.keybind"), SMALL_WIDTH * 2.0).into_iter().map(|(l, _)| l).collect();
    let help = lit(format!("{} {} {}", t("emotecraft.options.fastmenu"), t("emotecraft.options.fastmenu2"), t("emotecraft.options.fastmenu3")));
    let help_lines: Vec<Text> = font.split_lines(&help, SMALL_WIDTH * 2.0).into_iter().map(|(l, _)| l).collect();
    let wheel = (w / 2.5).min(h / 2.3).round().min(256.0);
    let total = 4.0 + key_lines.len() as f32 * 9.0 + 4.0 + 20.0 + SPACING + help_lines.len() as f32 * 9.0 + SPACING / 2.0 + wheel;
    let mut y = (HEADER + ((h - 2.0 * HEADER - total) / 2.0).floor()).max(HEADER);
    y += 4.0;
    let text_y = y;
    y += key_lines.len() as f32 * 9.0 + 4.0;
    let key_btn = [x, y, SMALL_WIDTH, 20.0];
    let binds_btn = [x + SMALL_WIDTH + (SPACING / 3.0).floor(), y, SMALL_WIDTH, 20.0];
    y += 20.0 + SPACING;
    let help_y = y;
    y += help_lines.len() as f32 * 9.0 + (SPACING / 2.0).floor();
    let grid_w = SMALL_WIDTH * 2.0 + (SPACING / 3.0).floor();
    let wheel_rect = [x + ((grid_w - wheel) / 2.0).floor(), y, wheel, wheel];
    (text_y, key_lines, key_btn, binds_btn, help_lines, help_y, wheel_rect)
}

fn list_footer(s: &ListScreen, w: f32, h: f32) -> Vec<Rect> {
    match s.kind {
        ListKind::Full => footer(w, h, &[20.0, DEFAULT_WIDTH, DEFAULT_WIDTH]),
        ListKind::Menu => footer(w, h, &[20.0, SMALL_WIDTH, SMALL_WIDTH, SMALL_WIDTH]),
    }
}

/// The key a bound emote has (`KeyBindUtils.getKey`).
fn key_of(uuid: u128) -> String {
    glue::config(|c| c.keys.iter().find(|(_, a)| a.uuid == uuid).map(|(k, _)| k.clone())).unwrap_or_else(|| UNBOUND.into())
}

/// `KeyBindUtils.getKeyMessage`.
fn key_message(key: &str, waiting: bool) -> Text {
    let name = horizon_client::keybinds::display_name(key);
    if !waiting {
        return name;
    }
    lit("> ").append(name.with_color(0xFFFFFF).underlined()).append(lit(" <")).with_color(0xFFFF55)
}

#[allow(clippy::too_many_arguments)]
fn draw_list_screen(game: &Game<'_>, batch: &mut HudBatch, w: f32, h: f32, cursor: (f32, f32), s: &mut ListScreen, mannequins: &mut HashMap<u32, Mannequin>, world: bool, tip: &mut Option<Vec<Text>>) {
    let atlas = &game.gui.atlas;
    // No blur while the player's own emote plays (All Emotes).
    let playing = glue::own_emote().is_some();
    if !(s.kind == ListKind::Full && playing) {
        background(game, batch, w, h, world);
    } else {
        ui::menu_background(batch, atlas, 0.0, 0.0, w, h, world);
    }
    let (area, left, row_w, prev, visible) = list_layout(s, w, h);
    // The search box in the header.
    s.search.x = ((w - 200.0) / 2.0).floor();
    s.search.y = ((HEADER - 20.0) / 2.0).floor();
    s.search.focused = s.search_focused;
    s.search.suggestion = s.search.value.is_empty().then(|| Text::translate("gui.recipebook.search_hint", &[]).plain());
    s.search.draw(batch, atlas, None);
    let focused = true;
    s.list.draw(game, batch, area, left, row_w, cursor, focused);
    let hovered = s.list.hovered(area, left, row_w, cursor).filter(|i| Some(*i) != s.list.selected).map(|i| s.list.entries[i].clone());
    LIST_HOVER.with(|h| *h.borrow_mut() = hovered);
    // The preview, with its background (`PlayerPreview` with `renderBackground`).
    if visible && prev[2] > 0.0 {
        let m = mannequins.entry(LIST_ID).or_default();
        let a = m.alpha * 0.5;
        fill(game, batch, [prev[0] + 1.0, prev[1] + 1.0, prev[2] - 2.0, prev[3] - 2.0], u32::from_be_bytes([(a * 255.0) as u8, 0, 0, 0]));
        outline(game, batch, prev, u32::from_be_bytes([(a * 255.0) as u8, 255, 255, 255]));
        let size = (m.alpha * (prev[3] / 3.0).floor()).round();
        preview::draw(game, batch, LIST_ID, prev, size, 0.0625, cursor);
        m.after_draw();
    }
    if s.kind == ListKind::Menu {
        let (text_y, key_lines, key_btn, binds_btn, help_lines, help_y, wheel_rect) = menu_layout(game, s, w, h);
        let mut y = text_y;
        for l in &key_lines {
            batch.text_styled(atlas, l, key_btn[0], y, u32::MAX, true);
            y += 9.0;
        }
        let selected = s.list.selected.and_then(|i| match &s.list.entries[i] {
            Entry::Emote(a) => Some(a.clone()),
            _ => None,
        });
        let key_label = match &selected {
            Some(a) => key_message(&key_of(a.uuid), s.key_wait != 0),
            None => horizon_client::keybinds::display_name(UNBOUND),
        };
        button(game, batch, cursor, key_btn, &key_label, selected.is_some());
        let any_keys = glue::config(|c| !c.keys.is_empty());
        button(game, batch, cursor, binds_btn, &Text::translate("controls.keybinds", &[]), any_keys);
        let mut y = help_y;
        for l in &help_lines {
            batch.text_styled(atlas, l, key_btn[0], y, u32::MAX, true);
            y += 9.0;
        }
        let slots = wheel_slots();
        s.wheel.draw(game, batch, wheel_rect, cursor, &slots, false, s.key_wait == 0, mannequins, tip);
        HOVER.with(|c| c.set(s.wheel.tile_at(wheel_rect, cursor)));
    }
    let foot = list_footer(s, w, h);
    page_button(game, batch, cursor, foot[0], false, !s.list.open.is_empty(), true);
    match s.kind {
        ListKind::Full => {
            button(game, batch, cursor, foot[1], &Text::translate("gui.cancel", &[]), true);
            button(game, batch, cursor, foot[2], &t_text("emotecraft.config"), true);
        }
        ListKind::Menu => {
            button(game, batch, cursor, foot[1], &t_text("emotecraft.openFolder"), true);
            button(game, batch, cursor, foot[2], &Text::translate("gui.done", &[]), true);
            button(game, batch, cursor, foot[3], &t_text("emotecraft.options.options"), true);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_keys(game: &Game<'_>, batch: &mut HudBatch, w: f32, h: f32, cursor: (f32, f32), k: &mut KeysScreen, mannequins: &mut HashMap<u32, Mannequin>, world: bool, tip: &mut Option<Vec<Text>>) {
    let _ = tip;
    let atlas = &game.gui.atlas;
    background(game, batch, w, h, world);
    title(game, batch, w, &Text::translate("controls.keybinds.title", &[]));
    let binds = sorted_binds();
    let area = [0.0, HEADER, w, h - 2.0 * HEADER];
    let row_w = (w / 2.0).floor();
    let left = (w / 2.0).floor() - (row_w / 2.0).floor();
    tile(game, batch, list_bg_sprite(game), area, (0.0, HEADER), (32.0, 32.0), u32::MAX);
    let start = batch.vertices.len();
    let mut hovered = None;
    for (i, (key, a)) in binds.iter().enumerate() {
        let y = area[1] + 4.0 + i as f32 * 24.0 - k.scroll;
        let (cx, cy, cw, ch) = (left + 2.0, y + 2.0, row_w - 4.0, 20.0);
        let hover = inside(cursor, [left, y, row_w, 24.0]) && inside(cursor, area);
        if hover {
            hovered = Some(Entry::Emote(a.clone()));
            fill(game, batch, [cx - 1.0, cy - 1.0, cw + 2.0, ch + 2.0], 0x8042_4242);
        }
        let reset = [cx + cw - 50.0, cy + ch / 2.0 - 10.0, 50.0, 20.0];
        let change = [reset[0] - 4.0 - 75.0, reset[1], 75.0, 20.0];
        button(game, batch, cursor, reset, &Text::translate("controls.reset", &[]), true);
        button(game, batch, cursor, change, &key_message(key, k.selected.as_deref() == Some(key.as_str())), true);
        if let Some(icon) = glue::find(a.uuid).and_then(|l| l.icon.clone()).or_else(|| a.icon.clone()) {
            icons::draw(game, batch, a.uuid, &icon, [cx, cy + ch / 2.0 - 10.0, 20.0, 20.0], u32::MAX);
        }
        let name_x = cx + 20.0 + 4.0;
        let clip = batch.vertices.len();
        batch.text_styled(atlas, &lang::component(a.str("name").unwrap_or_default()), name_x, cy + ch / 2.0 - 4.0, u32::MAX, true);
        batch.clip_since(clip, name_x, cy, change[0] - 8.0, cy + ch);
    }
    batch.clip_since(start, area[0], area[1], area[0] + area[2], area[1] + area[3]);
    let content = binds.len() as f32 * 24.0 + 4.0;
    if content > area[3] {
        let sx = left + row_w + 6.0;
        let bar = ((area[3] * area[3] / content).floor()).clamp(32.0, area[3] - 8.0);
        let sy = area[1] + (k.scroll.clamp(0.0, content - area[3]) / (content - area[3]) * (area[3] - bar)).floor();
        ui::blit_sprite(batch, atlas, "gui/sprites/widget/scroller_background", sx, area[1], 6.0, area[3], u32::MAX);
        ui::blit_sprite(batch, atlas, "gui/sprites/widget/scroller", sx, sy, 6.0, bar, u32::MAX);
    }
    ui::separators(batch, atlas, HEADER, h - HEADER, world);
    LIST_HOVER.with(|h| *h.borrow_mut() = hovered);
    // The preview on the left.
    let preview_h = (h / 2.0).floor().min(256.0);
    let space = left - SPACING * 2.0;
    let pw = space.clamp(0.0, preview_h);
    if space >= (preview_h / 3.0).floor() && pw > 0.0 {
        let prev = [SPACING + ((space - pw) / 2.0).floor(), HEADER + ((h - 2.0 * HEADER - preview_h) / 2.0).floor(), pw, preview_h];
        let m = mannequins.entry(LIST_ID).or_default();
        let a = m.alpha * 0.5;
        fill(game, batch, [prev[0] + 1.0, prev[1] + 1.0, prev[2] - 2.0, prev[3] - 2.0], u32::from_be_bytes([(a * 255.0) as u8, 0, 0, 0]));
        outline(game, batch, prev, u32::from_be_bytes([(a * 255.0) as u8, 255, 255, 255]));
        preview::draw(game, batch, LIST_ID, prev, (m.alpha * (prev[3] / 3.0).floor()).round(), 0.0625, cursor);
        m.after_draw();
    }
    let n = binds.len();
    let foot = footer(w, h, &[SMALL_WIDTH, SMALL_WIDTH]);
    let reset_label = if n > 0 { Text::translate("controls.resetAll", &[]).append(lit(format!(" ({n})"))) } else { Text::translate("controls.resetAll", &[]) };
    button(game, batch, cursor, foot[0], &reset_label, n > 0);
    button(game, batch, cursor, foot[1], &Text::translate("gui.done", &[]), true);
}

fn sorted_binds() -> Vec<(String, Arc<Animation>)> {
    let mut b: Vec<(String, Arc<Animation>)> = glue::config(|c| c.keys.iter().map(|(k, a)| (k.clone(), a.clone())).collect());
    b.sort_by_key(|(_, a)| lang::plain(a.str("name").unwrap_or_default()));
    b
}

/// The options shown (hidden ones only with "Show Hidden Options"), by category: a header row
/// then a row per option.
fn option_rows() -> Vec<Option<&'static cfg::Entry>> {
    let show_hidden = glue::config(|c| c.b("showHiddenConfig"));
    let mut rows = Vec::new();
    for cat in cfg::CATEGORIES {
        rows.push(None);
        let _ = cat;
        for e in cfg::ENTRIES.iter().filter(|e| e.category == *cat && (!e.hidden || show_hidden)) {
            rows.push(Some(e));
        }
    }
    rows
}

fn category_of(i: usize, rows: &[Option<&'static cfg::Entry>]) -> &'static str {
    let mut cat = cfg::CATEGORIES[0];
    let mut k = 0;
    for (j, r) in rows.iter().enumerate() {
        if r.is_none() {
            cat = cfg::CATEGORIES[k.min(cfg::CATEGORIES.len() - 1)];
            k += 1;
        }
        if j == i {
            break;
        }
    }
    cat
}

/// Options the port can't make do anything: the cloud library and Online Emotes are services of
/// Emotecraft's authors that Horizon can't reach.
fn unavailable(name: &str) -> bool {
    matches!(name, "cloudLibraryStatus" | "libraryNotifications" | "onlineReconnectDelay" | "onlineDebug")
}

fn option_label(e: &cfg::Entry) -> Text {
    let name = t_text(&format!("emotecraft.otherconfig.{}", e.name));
    let value = glue::config(|c| c.get(e.name));
    let v = match (&e.kind, value) {
        (Kind::Bool, V::Bool(b)) => Text::translate(if b { "options.on" } else { "options.off" }, &[]),
        (_, V::Float(f)) => lit(emotecraft_engine::config::java_float(f)),
        (_, V::Long(l)) => lit(l.to_string()),
        (_, V::Enum(s) | V::Str(s)) => lit(s),
        (_, V::Bool(b)) => lit(b.to_string()),
    };
    Text::translate("options.generic_value", &[name, v])
}

fn option_tip(game: &Game<'_>, e: &cfg::Entry) -> Option<Vec<Text>> {
    let mut lines: Vec<Text> = Vec::new();
    if e.tooltip {
        let t = t_text(&format!("emotecraft.otherconfig.{}.tooltip", e.name));
        lines.extend(game.gui.atlas.font.split_lines(&t, 200.0).into_iter().map(|(l, _)| l));
    }
    if unavailable(e.name) {
        let note = if lang::russian() { "Недоступно в порте для Horizon: облачная библиотека и Online Emotes — закрытые сервисы авторов Emotecraft." } else { "Not available in the Horizon port: the cloud library and Online Emotes are closed services of Emotecraft's authors." };
        lines.extend(game.gui.atlas.font.split_lines(&lit(note).with_color(0xAAAAAA), 200.0).into_iter().map(|(l, _)| l));
    }
    (!lines.is_empty()).then_some(lines)
}

fn slider_value(e: &cfg::Entry) -> f32 {
    let v = glue::config(|c| c.f(e.name));
    match e.kind {
        Kind::Float(lo, hi) => ((v - lo) / (hi - lo)).clamp(0.0, 1.0),
        Kind::Long(lo, hi) => ((v - lo as f32) / (hi - lo) as f32).clamp(0.0, 1.0),
        _ => 0.0,
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_options(game: &Game<'_>, batch: &mut HudBatch, w: f32, h: f32, cursor: (f32, f32), o: &mut OptionsScreen, world: bool, tip: &mut Option<Vec<Text>>) {
    let atlas = &game.gui.atlas;
    background(game, batch, w, h, world);
    title(game, batch, w, &t_text("emotecraft.otherconfig"));
    let rows = option_rows();
    let area = [0.0, HEADER, w, h - 2.0 * HEADER];
    let content = rows.len() as f32 * 25.0 + 4.0;
    o.scroll = o.scroll.clamp(0.0, (content - area[3]).max(0.0));
    tile(game, batch, list_bg_sprite(game), area, (0.0, HEADER), (32.0, 32.0), u32::MAX);
    let start = batch.vertices.len();
    let x = (w / 2.0).floor() - 155.0;
    for (i, r) in rows.iter().enumerate() {
        let y = area[1] + 4.0 + i as f32 * 25.0 - o.scroll;
        if y + 25.0 < area[1] || y > area[1] + area[3] {
            continue;
        }
        match r {
            None => {
                let cat = category_of(i, &rows);
                let t = t_text(&format!("emotecraft.otherconfig.category.{cat}"));
                let tw = width(game, &t);
                batch.text_styled(atlas, &t, ((w - tw) / 2.0).floor(), y + 12.0 - 4.0, u32::MAX, true);
            }
            Some(e) => {
                let rr = [x, y + 2.0, 310.0, 20.0];
                let active = !unavailable(e.name);
                let hover = inside(cursor, rr) && inside(cursor, area);
                match e.kind {
                    Kind::Float(..) | Kind::Long(..) => {
                        ui::slider(batch, atlas, rr[0], rr[1], rr[2], &option_label(e), slider_value(e), hover, active, o.sliding == Some(e.name));
                    }
                    _ => button(game, batch, cursor, rr, &option_label(e), active),
                }
                if hover {
                    *tip = option_tip(game, e);
                }
            }
        }
    }
    batch.clip_since(start, area[0], area[1], area[0] + area[2], area[1] + area[3]);
    if content > area[3] {
        // `AbstractSelectionList`'s scrollbar beside the rows.
        let sx = (w / 2.0).floor() + 155.0 + 10.0;
        let bar = ((area[3] * area[3] / content).floor()).clamp(32.0, area[3] - 8.0);
        let sy = area[1] + (o.scroll / (content - area[3]) * (area[3] - bar)).floor();
        ui::blit_sprite(batch, atlas, "gui/sprites/widget/scroller_background", sx, area[1], 6.0, area[3], u32::MAX);
        ui::blit_sprite(batch, atlas, "gui/sprites/widget/scroller", sx, sy, 6.0, bar, u32::MAX);
    }
    ui::separators(batch, atlas, HEADER, h - HEADER, world);
    let foot = footer(w, h, &[SMALL_WIDTH, DEFAULT_WIDTH, SMALL_WIDTH]);
    button(game, batch, cursor, foot[0], &Text::translate("controls.reset", &[]), true);
    button(game, batch, cursor, foot[1], &Text::translate("gui.done", &[]), true);
    button(game, batch, cursor, foot[2], &t_text("emotecraft.options.export"), true);
}

fn export_button(w: f32, h: f32) -> Rect {
    [((w - BIG_WIDTH) / 2.0).floor() - SPACING, HEADER + ((h - 2.0 * HEADER - 24.0) / 2.0).floor(), BIG_WIDTH, 20.0]
}

fn draw_export(game: &Game<'_>, batch: &mut HudBatch, w: f32, h: f32, cursor: (f32, f32), world: bool) {
    background(game, batch, w, h, world);
    title(game, batch, w, &t_text("emotecraft.options.export"));
    let mut b = export_button(w, h);
    b[0] += SPACING;
    button(game, batch, cursor, b, &lit(tf("emotecraft.export", &["emotecraft"])), true);
    let foot = footer(w, h, &[DEFAULT_WIDTH, DEFAULT_WIDTH]);
    button(game, batch, cursor, foot[0], &Text::translate("gui.done", &[]), true);
    button(game, batch, cursor, foot[1], &t_text("emotecraft.openFolder"), true);
}

fn confirm_texts(c: &ConfirmKind) -> (Text, Text, [Text; 2]) {
    let yes_no = [Text::translate("gui.yes", &[]), Text::translate("gui.no", &[])];
    match c {
        ConfirmKind::ResetConfig => (t_text("emotecraft.resetConfig.title"), t_text("emotecraft.resetConfig.message"), yes_no),
        ConfirmKind::ResetKeys => {
            let n = glue::config(|c| c.keys.len());
            (t_text("emotecraft.resetAllKeys.title"), lit(format!("{} ({n})", t("emotecraft.resetAllKeys.message"))), yes_no)
        }
        ConfirmKind::Overwrite(..) => (t_text("emotecraft.sure").bold(), t_text("emotecraft.sure2"), yes_no),
    }
}

fn draw_confirm(game: &Game<'_>, batch: &mut HudBatch, w: f32, h: f32, cursor: (f32, f32), c: &ConfirmKind, world: bool) {
    // A modal over the screen under it (`BaseModalScreen`), or vanilla's confirmation.
    fill(game, batch, [0.0, 0.0, w, h], if world { 0xC010_1010 } else { 0xE010_1010 });
    let (title, message, labels) = confirm_texts(c);
    let conf = horizon_client::confirm_screen::Confirm::new(&game.gui.atlas.font, (w, h), &title, &message);
    conf.draw(batch, &game.gui.atlas, cursor, [&labels[0], &labels[1]]);
}

// ---- input --------------------------------------------------------------------------------

fn click(u: &mut Ui) {
    u.reqs.push(Req::Click);
}

/// Binds `key` to the selected emote (`KeyBindUtils.bind`): a key another emote has asks first.
fn bind(u: &mut Ui, a: Arc<Animation>, key: String) {
    let current = (key != UNBOUND).then(|| glue::config(|c| c.keys.get(&key).cloned())).flatten();
    match current {
        Some(c) if c.uuid != a.uuid => u.stack.push(Scr::Confirm(ConfirmKind::Overwrite(a, key))),
        _ => force_bind(a, key),
    }
}

/// `forceBind`: one key per emote, one emote per key.
fn force_bind(a: Arc<Animation>, key: String) {
    glue::config(|c| {
        c.keys.retain(|_, b| b.uuid != a.uuid);
        if key != UNBOUND {
            c.keys.insert(key, a);
        }
        c.dirty = true;
    });
}

fn mouse_key(button: i32) -> String {
    match button {
        0 => "key.mouse.left".into(),
        1 => "key.mouse.right".into(),
        2 => "key.mouse.middle".into(),
        n => format!("key.mouse.{}", n + 1),
    }
}

/// `onClick` of a wheel tile on the wheel screen: plays it and closes.
fn wheel_play(u: &mut Ui, slot: usize) -> bool {
    let slots = wheel_slots();
    let Some(a) = slots[slot].clone() else { return false };
    glue::act(glue::Action::Play(a));
    // `playEmote` closing the screen when it started: the action decides in the next frame.
    u.stack.clear();
    u.reqs.push(Req::CloseAll);
    true
}

pub(super) fn pressed(game: &Game<'_>, screen: &NativeScreenView<'_>, button: i32) -> bool {
    if !mine(game, screen) {
        return false;
    }
    let (w, h) = (screen.rect[2].floor(), screen.rect[3].floor());
    let p = (screen.mouse[0], screen.mouse[1]);
    let wheel_key = game.native_mod_key(ID, "fastchoose", "key.keyboard.b");
    with(|u| {
        let n = u.stack.len();
        let Some(top) = u.stack.last_mut() else { return };
        match top {
            Scr::Confirm(c) => {
                let (title, message, _) = confirm_texts(c);
                let conf = horizon_client::confirm_screen::Confirm::new(&game.gui.atlas.font, (w, h), &title, &message);
                let Some(yes) = conf.answer_at(p) else { return };
                let c = c.clone();
                u.stack.pop();
                click(u);
                if yes {
                    match c {
                        ConfirmKind::ResetConfig => glue::config(|c| c.reset()),
                        ConfirmKind::ResetKeys => glue::config(|c| {
                            c.keys.clear();
                            c.dirty = true;
                        }),
                        ConfirmKind::Overwrite(a, key) => force_bind(a, key),
                    }
                }
            }
            Scr::Wheel(wh) => {
                let (_, _, rect) = wheel_layout(game, w, h);
                if glue::config(|c| c.s("closeWheelType")) == "PRESS" && mouse_key(button) == wheel_key {
                    pop(u);
                    return;
                }
                if button == 0 {
                    let (fwd, back) = wh.page_buttons(game, rect);
                    if inside(p, fwd) {
                        wh.turn(true);
                        click(u);
                        return;
                    }
                    if inside(p, back) {
                        wh.turn(false);
                        click(u);
                        return;
                    }
                    if let Some(slot) = wh.tile_at(rect, p) {
                        if wheel_slots()[slot].is_some() {
                            click(u);
                        }
                        wheel_play(u, slot);
                        return;
                    }
                    for (i, r) in wheel_footer(w, h).into_iter().enumerate() {
                        if inside(p, r) {
                            let (_, active) = wheel_footer_label(i);
                            if !active {
                                return;
                            }
                            click(u);
                            let idx = if online_debug() { i } else { i + 1 };
                            if idx == 1 {
                                pop(u);
                            } else {
                                u.stack.push(Scr::List(new_list(ListKind::Full)));
                            }
                            return;
                        }
                    }
                }
            }
            Scr::List(s) => {
                // Waiting for a key: a mouse button binds.
                if s.key_wait != 0 {
                    s.key_wait = 0;
                    if let Some(Entry::Emote(a)) = s.list.selected.map(|i| s.list.entries[i].clone()) {
                        bind(u, a, mouse_key(button));
                    }
                    return;
                }
                let (area, left, row_w, ..) = list_layout(s, w, h);
                let sb = [s.search.x, s.search.y, s.search.width, s.search.height];
                s.search_focused = inside(p, sb);
                if s.search_focused {
                    s.search.click(&game.gui.atlas, p.0, false, false);
                    return;
                }
                if s.kind == ListKind::Menu {
                    let (_, _, key_btn, binds_btn, _, _, wheel_rect) = menu_layout(game, s, w, h);
                    if button == 0 && inside(p, key_btn) {
                        if s.list.selected.is_some_and(|i| matches!(s.list.entries[i], Entry::Emote(_))) {
                            click(u);
                            if let Some(Scr::List(s)) = u.stack.last_mut() {
                                s.key_wait = 200;
                            }
                        }
                        return;
                    }
                    if button == 0 && inside(p, binds_btn) && glue::config(|c| !c.keys.is_empty()) {
                        click(u);
                        u.stack.push(Scr::Keys(KeysScreen { selected: None, scroll: 0.0, previewed: None }));
                        return;
                    }
                    let (fwd, back) = s.wheel.page_buttons(game, wheel_rect);
                    if button == 0 && inside(p, fwd) {
                        s.wheel.turn(true);
                        click(u);
                        return;
                    }
                    if button == 0 && inside(p, back) {
                        s.wheel.turn(false);
                        click(u);
                        return;
                    }
                    if let Some(slot) = s.wheel.tile_at(wheel_rect, p) {
                        // Right click clears the slot; left puts the selected emote in it.
                        let pg = page();
                        if button == 1 {
                            glue::config(|c| {
                                c.fast_menu[pg][slot] = None;
                                c.dirty = true;
                            });
                            click(u);
                        } else if button == 0
                            && let Some(Entry::Emote(a)) = s.list.selected.map(|i| s.list.entries[i].clone())
                        {
                            glue::config(|c| {
                                c.fast_menu[pg][slot] = Some(a);
                                c.dirty = true;
                            });
                            click(u);
                        }
                        return;
                    }
                }
                let foot = list_footer(s, w, h);
                if button == 0 {
                    for (i, r) in foot.iter().enumerate() {
                        if !inside(p, *r) {
                            continue;
                        }
                        match (s.kind, i) {
                            (_, 0) => {
                                if !s.list.open.is_empty() {
                                    click(u);
                                    if let Some(Scr::List(s)) = u.stack.last_mut() {
                                        s.list.back();
                                        s.search.set_value("");
                                    }
                                }
                            }
                            (ListKind::Full, 1) | (ListKind::Menu, 2) => {
                                let close_all = s.close_all;
                                click(u);
                                if close_all {
                                    u.stack.clear();
                                    u.reqs.push(Req::CloseAll);
                                } else {
                                    pop(u);
                                }
                            }
                            (ListKind::Full, 2) => {
                                click(u);
                                u.stack.push(Scr::List(new_list(ListKind::Menu)));
                                u.reqs.push(Req::Reload);
                            }
                            (ListKind::Menu, 1) => {
                                click(u);
                                u.reqs.push(Req::OpenFolder);
                            }
                            (ListKind::Menu, 3) => {
                                click(u);
                                u.stack.push(Scr::Options(OptionsScreen { scroll: 0.0, sliding: None }));
                            }
                            _ => {}
                        }
                        return;
                    }
                }
                // The scrollbar.
                let content = s.list.content();
                let bar_x = if s.list.compact { left + row_w - 6.0 } else { left + row_w + 6.0 };
                if button == 0 && content > area[3] && inside(p, [bar_x, area[1], 6.0, area[3]]) {
                    s.list.dragging = true;
                    return;
                }
                if button == 0
                    && let Some(i) = s.list.hovered(area, left, row_w, p)
                {
                    click(u);
                    let Some(Scr::List(s)) = u.stack.last_mut() else { return };
                    match s.list.entries[i].clone() {
                        Entry::Folder { path, .. } => {
                            s.list.open_folder(path);
                            s.search.set_value("");
                            s.previewed = None;
                        }
                        Entry::Emote(a) => {
                            s.list.selected = Some(i);
                            if s.kind == ListKind::Full {
                                glue::act(glue::Action::Play(a));
                                // `lastScreen` becomes the wheel's parent: closing leaves to the game.
                                if n >= 2 && matches!(u.stack[n - 2], Scr::Wheel(_)) {
                                    if let Some(Scr::List(s)) = u.stack.last_mut() {
                                        s.close_all = true;
                                    }
                                }
                            }
                        }
                        Entry::Header => {}
                    }
                }
            }
            Scr::Keys(k) => {
                if let Some(key) = k.selected.take() {
                    let binds = sorted_binds();
                    if let Some((_, a)) = binds.into_iter().find(|(kk, _)| *kk == key) {
                        bind(u, a, mouse_key(button));
                    }
                    return;
                }
                if button != 0 {
                    return;
                }
                let foot = footer(w, h, &[SMALL_WIDTH, SMALL_WIDTH]);
                if inside(p, foot[0]) && glue::config(|c| !c.keys.is_empty()) {
                    click(u);
                    u.stack.push(Scr::Confirm(ConfirmKind::ResetKeys));
                    return;
                }
                if inside(p, foot[1]) {
                    click(u);
                    pop(u);
                    return;
                }
                let row_w = (w / 2.0).floor();
                let left = (w / 2.0).floor() - (row_w / 2.0).floor();
                for (i, (key, a)) in sorted_binds().into_iter().enumerate() {
                    let y = HEADER + 4.0 + i as f32 * 24.0 - k.scroll + 2.0;
                    let cx = left + 2.0;
                    let cw = row_w - 4.0;
                    let reset = [cx + cw - 50.0, y, 50.0, 20.0];
                    let change = [reset[0] - 4.0 - 75.0, y, 75.0, 20.0];
                    if inside(p, reset) {
                        click(u);
                        glue::config(|c| {
                            c.keys.retain(|_, b| b.uuid != a.uuid);
                            c.dirty = true;
                        });
                        return;
                    }
                    if inside(p, change) {
                        click(u);
                        if let Some(Scr::Keys(k)) = u.stack.last_mut() {
                            k.selected = Some(key);
                        }
                        return;
                    }
                }
            }
            Scr::Options(o) => {
                if button != 0 {
                    return;
                }
                let foot = footer(w, h, &[SMALL_WIDTH, DEFAULT_WIDTH, SMALL_WIDTH]);
                if inside(p, foot[0]) {
                    click(u);
                    u.stack.push(Scr::Confirm(ConfirmKind::ResetConfig));
                    return;
                }
                if inside(p, foot[1]) {
                    click(u);
                    glue::save_config();
                    pop(u);
                    return;
                }
                if inside(p, foot[2]) {
                    click(u);
                    u.stack.push(Scr::Export);
                    return;
                }
                let rows = option_rows();
                let x = (w / 2.0).floor() - 155.0;
                for (i, r) in rows.iter().enumerate() {
                    let Some(e) = r else { continue };
                    let y = HEADER + 4.0 + i as f32 * 25.0 - o.scroll + 2.0;
                    if !inside(p, [x, y, 310.0, 20.0]) || p.1 < HEADER || p.1 > h - HEADER || unavailable(e.name) {
                        continue;
                    }
                    match e.kind {
                        Kind::Bool => {
                            click(u);
                            glue::config(|c| {
                                let b = c.b(e.name);
                                c.set(e.name, V::Bool(!b));
                            });
                        }
                        Kind::Enum(values) => {
                            click(u);
                            glue::config(|c| {
                                let now = c.s(e.name);
                                let i = values.iter().position(|v| *v == now).unwrap_or(0);
                                c.set(e.name, V::Enum(values[(i + 1) % values.len()].to_owned()));
                            });
                        }
                        Kind::Float(..) | Kind::Long(..) => {
                            o.sliding = Some(e.name);
                            slide(e, x, p.0);
                        }
                        Kind::Str => {}
                    }
                    return;
                }
            }
            Scr::Export => {
                if button != 0 {
                    return;
                }
                let mut b = export_button(w, h);
                b[0] += SPACING;
                if inside(p, b) {
                    click(u);
                    u.reqs.push(Req::Export);
                    return;
                }
                let foot = footer(w, h, &[DEFAULT_WIDTH, DEFAULT_WIDTH]);
                if inside(p, foot[0]) {
                    click(u);
                    pop(u);
                } else if inside(p, foot[1]) {
                    click(u);
                    u.reqs.push(Req::OpenFolder);
                }
            }
        }
    });
    true
}

/// A slider moved to the pointer (`NumberRange.fromSliderValue`: two decimals).
fn slide(e: &cfg::Entry, x: f32, mx: f32) {
    let v = ui::slider_value(x, 310.0, mx);
    glue::config(|c| match e.kind {
        Kind::Float(lo, hi) => {
            let raw = f64::from(lo) + f64::from(v) * f64::from(hi - lo);
            let val = ((raw * 100.0).round() / 100.0).clamp(f64::from(lo), f64::from(hi));
            c.set(e.name, V::Float(val as f32));
        }
        Kind::Long(lo, hi) => {
            let raw = lo as f64 + f64::from(v) * (hi - lo) as f64;
            let val = ((raw * 100.0).round() / 100.0).clamp(lo as f64, hi as f64);
            c.set(e.name, V::Long(val as i64));
        }
        _ => {}
    });
}

pub(super) fn released(game: &Game<'_>, screen: &NativeScreenView<'_>, button: i32) -> bool {
    if !mine(game, screen) {
        return false;
    }
    let wheel_key = game.native_mod_key(ID, "fastchoose", "key.keyboard.b");
    with(|u| {
        if glue::config(|c| c.s("closeWheelType")) == "HOLD" && mouse_key(button) == wheel_key && matches!(u.stack.last(), Some(Scr::Wheel(_))) {
            pop(u);
        }
        match u.stack.last_mut() {
            Some(Scr::Options(o)) => o.sliding = None,
            Some(Scr::List(s)) => s.list.dragging = false,
            _ => {}
        }
    });
    true
}

pub(super) fn dragged(game: &Game<'_>, screen: &NativeScreenView<'_>) -> bool {
    if !mine(game, screen) {
        return false;
    }
    let (w, h) = (screen.rect[2].floor(), screen.rect[3].floor());
    let p = (screen.mouse[0], screen.mouse[1]);
    with(|u| match u.stack.last_mut() {
        Some(Scr::Options(o)) => {
            if let Some(name) = o.sliding
                && let Some(e) = cfg::entry(name)
            {
                slide(e, (w / 2.0).floor() - 155.0, p.0);
            }
        }
        Some(Scr::List(s)) if s.list.dragging => {
            let (area, ..) = list_layout(s, w, h);
            let content = s.list.content();
            let bar = ((area[3] * area[3] / content).floor()).clamp(32.0, area[3] - 8.0);
            let frac = ((p.1 - area[1] - bar / 2.0) / (area[3] - bar)).clamp(0.0, 1.0);
            s.list.set_scroll(frac * (content - area[3]), area);
        }
        _ => {}
    });
    true
}

pub(super) fn scrolled(game: &Game<'_>, screen: &NativeScreenView<'_>, scroll: [f64; 2]) -> bool {
    if !mine(game, screen) {
        return false;
    }
    let (w, h) = (screen.rect[2].floor(), screen.rect[3].floor());
    let p = (screen.mouse[0], screen.mouse[1]);
    with(|u| match u.stack.last_mut() {
        Some(Scr::Wheel(wh)) => {
            wh.scroll(scroll[0]);
        }
        Some(Scr::List(s)) => {
            let (area, ..) = list_layout(s, w, h);
            if s.kind == ListKind::Menu {
                let (.., wheel_rect) = menu_layout(game, s, w, h);
                if inside(p, wheel_rect) {
                    s.wheel.scroll(scroll[0]);
                    return;
                }
            }
            if inside(p, area) {
                let v = s.list.scroll - scroll[1] as f32 * 36.0 / 2.0;
                s.list.set_scroll(v, area);
            }
        }
        Some(Scr::Keys(k)) => {
            let content = sorted_binds().len() as f32 * 24.0 + 4.0;
            k.scroll = (k.scroll - scroll[1] as f32 * 12.0).clamp(0.0, (content - (h - 2.0 * HEADER)).max(0.0));
        }
        Some(Scr::Options(o)) => {
            let content = option_rows().len() as f32 * 25.0 + 4.0;
            o.scroll = (o.scroll - scroll[1] as f32 * 12.5).clamp(0.0, (content - (h - 2.0 * HEADER)).max(0.0));
        }
        _ => {}
    });
    true
}

fn digit(key: &str) -> Option<usize> {
    let d = key.strip_prefix("key.keyboard.")?;
    let d = d.strip_prefix("keypad.").unwrap_or(d);
    d.parse::<usize>().ok()
}

pub(super) fn key_pressed(game: &Game<'_>, screen: &NativeScreenView<'_>, key: &str, modifiers: u32) -> bool {
    if !mine(game, screen) {
        return false;
    }
    let wheel_key = game.native_mod_key(ID, "fastchoose", "key.keyboard.b");
    let escape = key == "key.keyboard.escape";
    with(|u| {
        let Some(top) = u.stack.last_mut() else { return };
        match top {
            Scr::Confirm(_) => {
                if escape {
                    u.stack.pop();
                }
            }
            Scr::Wheel(wh) => {
                let nav = glue::config(|c| c.b("enableWheelKeyboardNav"));
                if nav && let Some(d) = digit(key).filter(|d| (1..=8).contains(d)) {
                    if wheel_slots()[d - 1].is_some() {
                        wheel_play(u, d - 1);
                    }
                    return;
                }
                if glue::config(|c| c.s("closeWheelType")) == "PRESS" && key == wheel_key {
                    pop(u);
                    return;
                }
                if nav && key == "key.keyboard.left" {
                    wh.turn(false);
                } else if nav && key == "key.keyboard.right" {
                    wh.turn(true);
                } else if escape {
                    pop(u);
                }
            }
            Scr::List(s) => {
                if s.key_wait != 0
                    && let Some(Entry::Emote(a)) = s.list.selected.map(|i| s.list.entries[i].clone())
                {
                    s.key_wait = 0;
                    bind(u, a, if escape { UNBOUND.into() } else { key.to_owned() });
                    return;
                }
                if s.search_focused && !escape {
                    let before = s.search.value.clone();
                    edit(&mut s.search, key, modifiers);
                    if s.search.value != before {
                        let v = s.search.value.clone();
                        s.list.filter(&v);
                    }
                    return;
                }
                if escape {
                    if s.close_all {
                        u.stack.clear();
                        u.reqs.push(Req::CloseAll);
                    } else {
                        pop(u);
                    }
                }
            }
            Scr::Keys(k) => {
                if let Some(sel) = k.selected.take() {
                    if let Some((_, a)) = sorted_binds().into_iter().find(|(kk, _)| *kk == sel) {
                        bind(u, a, if escape { UNBOUND.into() } else { key.to_owned() });
                    }
                    return;
                }
                if escape {
                    pop(u);
                }
            }
            Scr::Options(_) => {
                if escape {
                    glue::save_config();
                    pop(u);
                }
            }
            Scr::Export => {
                if escape {
                    pop(u);
                }
            }
        }
    });
    true
}

/// The search field's editing keys.
fn edit(b: &mut EditBox, key: &str, modifiers: u32) {
    let ctrl = modifiers & 2 != 0;
    let shift = modifiers & 1 != 0;
    match key {
        "key.keyboard.backspace" => b.delete_chars(-1),
        "key.keyboard.delete" => b.delete_chars(1),
        "key.keyboard.left" => {
            let c = b.cursor.saturating_sub(1);
            b.set_cursor(c, shift);
        }
        "key.keyboard.right" => {
            let c = (b.cursor + 1).min(ui::char_len(&b.value));
            b.set_cursor(c, shift);
        }
        "key.keyboard.home" => b.set_cursor(0, shift),
        "key.keyboard.end" => b.move_cursor_to_end(shift),
        "key.keyboard.a" if ctrl => {
            b.move_cursor_to_end(false);
            b.set_cursor(0, true);
        }
        _ => {}
    }
}

pub(super) fn key_released(game: &Game<'_>, screen: &NativeScreenView<'_>, key: &str) -> bool {
    if !mine(game, screen) {
        return false;
    }
    let wheel_key = game.native_mod_key(ID, "fastchoose", "key.keyboard.b");
    with(|u| {
        if glue::config(|c| c.s("closeWheelType")) == "HOLD" && key == wheel_key && matches!(u.stack.last(), Some(Scr::Wheel(_))) {
            pop(u);
        }
    });
    true
}

pub(super) fn char_typed(game: &Game<'_>, screen: &NativeScreenView<'_>, c: char) -> bool {
    if !mine(game, screen) {
        return false;
    }
    with(|u| {
        if let Some(Scr::List(s)) = u.stack.last_mut()
            && s.search_focused
            && s.key_wait == 0
        {
            s.search.insert_text(&c.to_string());
            let v = s.search.value.clone();
            s.list.filter(&v);
        }
    });
    true
}
