// Roughly Enough Items for Horizon: REI's config screen (`REIConfigScreen`).
// The categories on the left, the chosen one's options on the right, Cancel
// and Done under them, the options search, the selection menus and the
// keybind capture, laid out with REI's own numbers (`ConfigCategoriesListWidget`,
// `ConfigEntriesListWidget`, `ConfigGroupWidget`, `ConfigOptionWidget`,
// `ConfigOptionValueWidget`) and the groups' previews. It edits a copy of the
// settings; Done keeps it, Cancel and Escape drop it. Opened by the gear button
// and the gear menu's "More Options..." over the container screen, and by Mod
// Menu's Configure and the mod's page over the Options screen
// (`roughlyenoughitems_options`).
use super::Game;
use super::roughlyenoughitems::{self as rei, CATEGORIES, Group, Opt, OptKind, Preview, Rect, inside};
use super::roughlyenoughitems_glue::{self as glue, Action, State};
use super::roughlyenoughitems_sprites::{self as sprites, Sprite};
use super::roughlyenoughitems_view::{argb, blit, button, fill, item, item_scaled, nine, text, width};
use horizon_client::hud::HudBatch;
use horizon_client::text::Text;
use horizon_client::ui::EditBox;
use std::collections::HashMap;
use std::time::Instant;

/// A keybind's modifiers.
pub(super) const SHIFT: u8 = 1;
pub(super) const CTRL: u8 = 2;
pub(super) const ALT: u8 = 4;

const NAME: u32 = 0xffc0_c0c0;
const DESC: u32 = 0xff75_7575;
const VALUE: u32 = 0xffe0_e0e0;
const WHITE: u32 = 0xffff_ffff;

/// A scrolled panel (cloth's `ScrollingContainer`): 16 pixels a notch,
/// eased to its target, never past its ends.
#[derive(Clone, Copy, Default)]
struct Scroll {
    value: f32,
    target: f32,
}

impl Scroll {
    fn step(&mut self, max: f32, dt: f32) {
        let max = max.max(0.0);
        self.target = self.target.clamp(0.0, max);
        let k = 1.0 - (-dt * 14.0).exp();
        self.value += (self.target - self.value) * k;
        if (self.target - self.value).abs() < 0.5 {
            self.value = self.target;
        }
        self.value = self.value.clamp(0.0, max);
    }
    fn at(self) -> f32 {
        self.value.round()
    }
}

/// A group of options as the list shows it: a category's group, or search
/// results under their group's title.
#[derive(Clone)]
struct GroupView {
    group: &'static Group,
    opts: Vec<&'static Opt>,
    /// The group's preview is shown (a category's list, not the results).
    preview: bool,
    /// The search term, lit on the title or on each option's name or text.
    query: String,
    title_lit: bool,
    lit: HashMap<&'static str, bool>,
}

impl GroupView {
    fn whole(group: &'static Group) -> GroupView {
        GroupView { group, opts: group.opts.iter().collect(), preview: true, query: String::new(), title_lit: false, lit: HashMap::new() }
    }
}

/// Where a group's preview goes (`ConfigGroupWidget`).
#[derive(Clone, Copy, PartialEq)]
enum Place {
    None,
    Top(f32),
    Right(f32),
    Bottom(f32),
}

/// An option's menu of values (`ConfigOptionValueWidget`, three values or more).
struct SelMenu {
    opt: &'static Opt,
    anchor: Rect,
    at: Instant,
    scroll: f32,
}

/// A keybind being set: the key pressed so far and its modifiers.
struct Capture {
    opt: &'static Opt,
    name: &'static str,
    key: Option<String>,
    mods: u8,
}

#[derive(Clone, Copy)]
enum CHit {
    SearchButton,
    Category(usize),
    Value(&'static Opt, Rect),
    Back,
    Field,
    Cancel,
    Done,
    MenuEntry(usize),
    MenuArea,
    PreviewTab(usize),
}

/// What the screen is drawn over.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Host {
    /// A container screen (the gear, "More Options..."), dimmed under it.
    Container,
    /// The Options screen (Mod Menu's Configure, the mod's page): vanilla's
    /// menu background over the world (`in_world`) or the title's panorama.
    Options { in_world: bool },
}

pub(super) struct ConfigScreen {
    host: Host,
    /// A world is open and a container screen was (`cleanRequiresLevel`):
    /// the previews show and the options that need a level work.
    level: bool,
    /// The values being edited, by option id; a keybind's is its modifiers.
    pending: HashMap<&'static str, String>,
    /// The keybinds' keys being edited, by key name.
    keys: HashMap<&'static str, String>,
    category: usize,
    searching: bool,
    field: EditBox,
    results: Option<(String, Vec<GroupView>)>,
    /// The categories, the options and the search results.
    scrolls: [Scroll; 3],
    areas: [Option<Rect>; 3],
    menu: Option<SelMenu>,
    capture: Option<Capture>,
    hits: Vec<(Rect, CHit)>,
    /// The accessibility preview's selected tab.
    preview_tab: usize,
    last: Instant,
}

impl ConfigScreen {
    /// Over a container screen: always in a level.
    pub(super) fn new(game: &Game<'_>) -> ConfigScreen {
        ConfigScreen::hosted(game, Host::Container, true)
    }

    /// Over the Options screen, from the world (`in_world`) or the title.
    pub(super) fn over_options(game: &Game<'_>, in_world: bool, level: bool) -> ConfigScreen {
        ConfigScreen::hosted(game, Host::Options { in_world }, level)
    }

    /// Drawn over the Options screen.
    pub(super) fn on_options(&self) -> bool {
        matches!(self.host, Host::Options { .. })
    }

    fn hosted(game: &Game<'_>, host: Host, level: bool) -> ConfigScreen {
        let mut pending = HashMap::new();
        let mut keys = HashMap::new();
        for o in all_options() {
            let stored = if o.fixed { None } else { game.native_setting(rei::ID, o.id) };
            let value = match o.kind {
                OptKind::Key(name) => {
                    keys.insert(name, glue::bound(game, name));
                    stored.unwrap_or_else(|| o.default.to_owned())
                }
                _ => stored.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty()).unwrap_or_else(|| o.default.to_owned()),
            };
            pending.insert(o.id, value);
        }
        let mut field = EditBox::new(0.0, 0.0, 0.0, 0.0);
        field.max_length = 9000;
        field.focused = false;
        ConfigScreen {
            host,
            level,
            pending,
            keys,
            category: 0,
            searching: false,
            field,
            results: None,
            scrolls: [Scroll::default(); 3],
            areas: [None; 3],
            menu: None,
            capture: None,
            hits: Vec::new(),
            preview_tab: 0,
            last: Instant::now(),
        }
    }

    fn value(&self, id: &str) -> &str {
        self.pending.get(id).map_or_else(|| rei::option_default(id), String::as_str)
    }
    fn is(&self, id: &str, token: &str) -> bool {
        self.value(id) == token
    }
    /// The option needs a level and there is none: "Load Level First!".
    fn locked(&self, o: &Opt) -> bool {
        o.level && !self.level
    }
}

fn all_options() -> impl Iterator<Item = &'static Opt> {
    CATEGORIES.iter().flat_map(|c| c.groups.iter()).flat_map(|g| g.opts.iter())
}

/// A modifiers value ("ctrl", "alt+shift") as bits.
pub(super) fn parse_mods(s: &str) -> u8 {
    s.split(['+', ',']).fold(0, |m, part| match part.trim() {
        "shift" => m | SHIFT,
        "ctrl" | "control" => m | CTRL,
        "alt" => m | ALT,
        _ => m,
    })
}

fn mods_text(m: u8) -> String {
    let mut parts = Vec::new();
    if m & ALT != 0 {
        parts.push("alt");
    }
    if m & CTRL != 0 {
        parts.push("ctrl");
    }
    if m & SHIFT != 0 {
        parts.push("shift");
    }
    parts.join("+")
}

/// A modifier key's own bit.
fn modifier_of(key: &str) -> Option<u8> {
    match key {
        "key.keyboard.left.shift" | "key.keyboard.right.shift" => Some(SHIFT),
        "key.keyboard.left.control" | "key.keyboard.right.control" => Some(CTRL),
        "key.keyboard.left.alt" | "key.keyboard.right.alt" => Some(ALT),
        _ => None,
    }
}

/// A keybind as cloth shows it: "Alt + Ctrl + Shift + K".
pub(super) fn key_text(key: &str, mods: u8) -> String {
    let mut s = horizon_client::keybinds::display_name(key).plain();
    if mods & SHIFT != 0 {
        s = format!("Shift + {s}");
    }
    if mods & CTRL != 0 {
        s = format!("Ctrl + {s}");
    }
    if mods & ALT != 0 {
        s = format!("Alt + {s}");
    }
    s
}

/// A mouse button's key name.
pub(super) fn mouse_key(button: i32) -> String {
    match button {
        0 => "key.mouse.left".to_owned(),
        1 => "key.mouse.right".to_owned(),
        2 => "key.mouse.middle".to_owned(),
        n => format!("key.mouse.{}", n + 1),
    }
}

fn pick(light: &'static Sprite, dark_twin: &'static Sprite, dark: bool) -> &'static Sprite {
    if dark { dark_twin } else { light }
}

fn intersect(a: Rect, b: Rect) -> Option<Rect> {
    let x0 = a[0].max(b[0]);
    let y0 = a[1].max(b[1]);
    let x1 = (a[0] + a[2]).min(b[0] + b[2]);
    let y1 = (a[1] + a[3]).min(b[1] + b[3]);
    (x1 > x0 && y1 > y0).then_some([x0, y0, x1 - x0, y1 - y0])
}

/// `font.substrByWidth`.
fn cut(game: &Game<'_>, s: &str, max: f32) -> String {
    let mut out = String::new();
    for ch in s.chars() {
        out.push(ch);
        if width(game, &out) > max {
            out.pop();
            break;
        }
    }
    out
}

fn wrap(game: &Game<'_>, s: &str, w: f32) -> Vec<Text> {
    if s.is_empty() {
        return Vec::new();
    }
    game.gui.atlas.font.split_lines(&Text::legacy(s), w).into_iter().map(|(t, _)| t).collect()
}

/// `text` with every case-blind match of `query` in yellow, `base` (legacy
/// codes such as "§n") on the rest.
fn lit(s: &str, query: &str, base: &str) -> String {
    let q: Vec<char> = query.chars().flat_map(char::to_lowercase).collect();
    let chars: Vec<char> = s.chars().collect();
    let low: Vec<char> = chars.iter().map(|c| c.to_lowercase().next().unwrap_or(*c)).collect();
    let mut out = String::from(base);
    let mut i = 0;
    while i < chars.len() {
        if !q.is_empty() && i + q.len() <= low.len() && low[i..i + q.len()] == q[..] {
            out.push_str("§e");
            out.push_str(base);
            out.extend(&chars[i..i + q.len()]);
            out.push_str("§r");
            out.push_str(base);
            i += q.len();
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

/// An underline under text drawn at (x, y) `w` wide (vanilla's, with its shadow).
fn underline(game: &Game<'_>, batch: &mut HudBatch, x: f32, y: f32, w: f32, color: u32, shadow: bool) {
    if shadow {
        let [a, r, g, b] = color.to_be_bytes();
        let dim = u32::from_be_bytes([a, r / 4, g / 4, b / 4]);
        fill(game, batch, [x, y + 9.0, w + 1.0, 1.0], dim);
    }
    fill(game, batch, [x - 1.0, y + 8.0, w + 1.0, 1.0], color);
}

/// One option's label in the language shown.
fn tr2(ru: bool, pair: (&'static str, &'static str)) -> &'static str {
    if ru { pair.1 } else { pair.0 }
}

// ---- search (`ConfigSearchWidget.matchResult`) ---------------------------------------

fn levenshtein(a: &[char], b: &[char]) -> usize {
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur[j + 1] = (prev[j + 1] + 1).min(cur[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

fn similarity(a: &str, b: &str) -> f32 {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let (longer, shorter) = if a.len() >= b.len() { (&a, &b) } else { (&b, &a) };
    let l = longer.len();
    if l == 0 {
        return 1.0;
    }
    (l - levenshtein(longer, shorter)) as f32 / l as f32
}

fn score(candidate: &str, q: &str, mult: f32) -> f32 {
    let c = candidate.to_lowercase();
    if c.contains(q) {
        similarity(&c, q) * mult
    } else {
        let s = similarity(&c, q).powf(1.5) * mult * 0.9;
        if s > 0.5 { s } else { 0.0 }
    }
}

fn search(term: &str, ru: bool) -> Vec<GroupView> {
    let q = term.to_lowercase();
    if q.trim().is_empty() {
        return Vec::new();
    }
    enum Found {
        Group(&'static Group),
        Opt(&'static Opt, &'static Group, bool),
    }
    let mut found: Vec<(f32, Found)> = Vec::new();
    for cat in CATEGORIES {
        for g in cat.groups {
            // Ids score too little to show (`KEY`, 0.0005), as in REI.
            let s = score(tr2(ru, g.name), &q, 0.98).max(score(g.id, &q, 0.0005));
            if s > 0.0 {
                found.push((s, Found::Group(g)));
            }
            for o in g.opts {
                let s = score(tr2(ru, o.name), &q, 0.98).max(score(o.id, &q, 0.0005));
                if s > 0.0 {
                    found.push((s, Found::Opt(o, g, false)));
                }
                if let Some(d) = o.desc {
                    let s = score(tr2(ru, d), &q, 0.8);
                    if s > 0.0 {
                        found.push((s, Found::Opt(o, g, true)));
                    }
                }
            }
        }
    }
    found.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    found.retain(|(s, _)| *s >= 0.001);
    // Once each; groups go by their name.
    let mut seen_opts: Vec<&str> = Vec::new();
    let mut seen_groups: Vec<&str> = Vec::new();
    let mut kept: Vec<Found> = Vec::new();
    for (_, f) in found {
        match f {
            Found::Group(g) => {
                let name = tr2(ru, g.name);
                if !seen_groups.contains(&name) {
                    seen_groups.push(name);
                    kept.push(Found::Group(g));
                }
            }
            Found::Opt(o, g, d) => {
                if !seen_opts.contains(&o.id) {
                    seen_opts.push(o.id);
                    kept.push(Found::Opt(o, g, d));
                }
            }
        }
    }
    kept.retain(|f| match f {
        Found::Opt(_, g, _) => !seen_groups.contains(&tr2(ru, g.name)),
        Found::Group(_) => true,
    });
    // Options of one group's name go together, under its title.
    let mut out: Vec<GroupView> = Vec::new();
    for f in kept {
        match f {
            Found::Group(g) => {
                let mut v = GroupView::whole(g);
                v.preview = false;
                v.query = term.to_owned();
                v.title_lit = true;
                out.push(v);
            }
            Found::Opt(o, g, d) => {
                let name = tr2(ru, g.name);
                let at = out.iter().rposition(|v| !v.title_lit && tr2(ru, v.group.name) == name);
                match at {
                    Some(i) => {
                        out[i].opts.push(o);
                        out[i].lit.insert(o.id, d);
                    }
                    None => {
                        let mut lit = HashMap::new();
                        lit.insert(o.id, d);
                        out.push(GroupView { group: g, opts: vec![o], preview: false, query: term.to_owned(), title_lit: false, lit });
                    }
                }
            }
        }
    }
    out
}

// ---- measuring --------------------------------------------------------------------------

/// The lines of the tooltip preview, each wrapped to `w - 16`.
fn tooltip_lines(game: &Game<'_>, c: &ConfigScreen, ru: bool, w: f32) -> Vec<Text> {
    let mut lines = vec![game.translate("block.minecraft.oak_planks", rei::tr(ru, "oak_planks"))];
    if c.is("appearance.append_mod_names", "true") {
        lines.push("§9§oMinecraft".to_owned());
    }
    if c.is("appearance.append_favorites_hint", "true") {
        let key = glue::favorite_key_name(game);
        lines.push(" ".to_owned());
        lines.push(format!("§7{}", rei::tr(ru, "favorites_tooltip").replace("%s", &key)));
    }
    lines.iter().flat_map(|l| wrap(game, l, w - 16.0)).collect()
}

fn tooltip_size(game: &Game<'_>, lines: &[Text]) -> (f32, f32) {
    let min_w = lines.iter().map(|t| game.gui.atlas.font.width_text(t)).fold(0.0, f32::max) + 4.0;
    let n = lines.len();
    let min_h = if n == 0 { 4.0 } else { 10.0 * n as f32 + if n >= 2 { 2.0 } else { 0.0 } + 4.0 };
    (min_w, min_h)
}

fn place(game: &Game<'_>, c: &ConfigScreen, ru: bool, v: &GroupView, gw: f32) -> Place {
    // Only in a level (`ConfigGroupWidget`).
    if !v.preview || !c.level {
        return Place::None;
    }
    let half = (gw * 6.0 / 10.0).floor() - 2.0;
    match v.group.preview {
        Preview::None => Place::None,
        Preview::Interface if half > 200.0 => Place::Right(half),
        Preview::Interface => Place::Top(80.0),
        Preview::Tooltip if half > 200.0 => Place::Right(half),
        Preview::Tooltip => {
            let (_, h) = tooltip_size(game, &tooltip_lines(game, c, ru, gw));
            Place::Top(28.0 + h + 12.0)
        }
        Preview::Accessibility => Place::Bottom(53.0),
    }
}

fn row_height(game: &Game<'_>, ru: bool, o: &Opt, rw: f32) -> f32 {
    let lines = o.desc.map_or(0, |d| wrap(game, tr2(ru, d), rw).len());
    12.0 + 12.0 * lines as f32
}

fn opts_height(game: &Game<'_>, ru: bool, v: &GroupView, rw: f32) -> f32 {
    let n = v.opts.len();
    if n == 0 {
        return 0.0;
    }
    v.opts.iter().map(|o| row_height(game, ru, o, rw)).sum::<f32>() + 7.0 * (n - 1) as f32
}

fn group_height(game: &Game<'_>, c: &ConfigScreen, ru: bool, v: &GroupView, gw: f32) -> f32 {
    let p = place(game, c, ru, v, gw);
    let rw = if let Place::Right(half) = p { half } else { gw };
    let oh = opts_height(game, ru, v, rw);
    18.0 + match p {
        Place::None | Place::Right(_) => oh,
        Place::Top(ph) => ph + 4.0 + oh,
        Place::Bottom(ph) => oh + 2.0 + ph,
    }
}

// ---- the values -------------------------------------------------------------------------

/// A value's text, before " (Default)".
fn choice_label(c: &ConfigScreen, ru: bool, o: &Opt, token: &str) -> String {
    let OptKind::Choice(values) = o.kind else {
        return token.to_owned();
    };
    let label = values.iter().find(|v| v.0 == token).map_or_else(
        || match token.parse::<f32>() {
            Ok(v) => format!("{}%", (v * 100.0).round()),
            Err(_) => token.to_owned(),
        },
        |v| (if ru { v.2 } else { v.1 }).to_owned(),
    );
    // The search field's places name the side the panel is on.
    if o.id == "layout.search_field_location" && c.is("layout.location", "left") {
        return label.replace("Right", "Left");
    }
    label
}

/// What an option's value shows (`ConfigOptionValueWidget`), at most `max` wide.
fn value_text(game: &Game<'_>, c: &ConfigScreen, ru: bool, o: &Opt, max: f32) -> String {
    // The requires-level dummy: red, no "(Default)".
    if c.locked(o) {
        return format!("§c{}", rei::tr(ru, "requires_level"));
    }
    let with_default = |value: String, is_default: bool| -> String {
        let dots = width(game, "...");
        if is_default {
            let full = rei::tr(ru, "value_default").replace("%s", &value);
            let fw = width(game, &full);
            if fw > max {
                let extra = fw - width(game, &value);
                let inner = cut(game, &value, max - dots - extra);
                return rei::tr(ru, "value_default").replace("%s", &format!("{inner}..."));
            }
            full
        } else if width(game, &value) > max {
            format!("{}...", cut(game, &value, max - dots))
        } else {
            value
        }
    };
    match o.kind {
        OptKind::Choice(_) => {
            let token = c.value(o.id);
            with_default(choice_label(c, ru, o, token), token == o.default)
        }
        OptKind::Key(name) => {
            if let Some(cap) = &c.capture
                && cap.opt.id == o.id
            {
                let shown = cap.key.as_deref().map_or_else(|| key_text("key.keyboard.unknown", 0), |k| key_text(k, cap.mods));
                return format!("§e> {shown} <");
            }
            let key = c.keys.get(name).map_or("key.keyboard.unknown", String::as_str);
            let mods = parse_mods(c.value(o.id));
            let default_key = rei::KEYS.iter().find(|k| k.0 == name).map_or("key.keyboard.unknown", |k| k.2);
            with_default(key_text(key, mods), key == default_key && mods == parse_mods(o.default))
        }
        OptKind::Link(t) => rei::tr(ru, t).to_owned(),
        OptKind::Text => with_default(c.value(o.id).to_owned(), true),
    }
}

/// The option can be changed here.
fn clickable(o: &Opt) -> bool {
    if o.fixed {
        return false;
    }
    !matches!(o.kind, OptKind::Text)
}

// ---- drawing ------------------------------------------------------------------------------

/// A panel (`HoleWidget`): the list background and its separators.
fn panel(game: &Game<'_>, batch: &mut HudBatch, r: Rect) {
    fill(game, batch, r, 0x7000_0000);
    fill(game, batch, [r[0], r[1] - 2.0, r[2], 1.0], 0x33ff_ffff);
    fill(game, batch, [r[0], r[1] - 1.0, r[2], 1.0], 0xbf00_0000);
    fill(game, batch, [r[0], r[1] + r[3], r[2], 1.0], 0xbf00_0000);
    fill(game, batch, [r[0], r[1] + r[3] + 1.0, r[2], 1.0], 0x33ff_ffff);
}

/// The scrollbar's thumb at the panel's right edge (its track stays unseen).
fn scrollbar(game: &Game<'_>, batch: &mut HudBatch, r: Rect, content: f32, scroll: f32) {
    if content <= r[3] {
        return;
    }
    let h = r[3];
    let max = content - h;
    let th = ((h * h / content).floor()).clamp(32.0, h);
    let y = (scroll.floor() * (h - th) / max + r[1]).floor().clamp(r[1], r[1] + h - th);
    horizon_client::ui::blit_sprite(batch, &game.gui.atlas, "gui/sprites/widget/scroller", r[0] + r[2] - 6.0, y, 6.0, th, u32::MAX);
}

/// A category's icon (`textures/gui/config/<key>.png`).
fn icon(i: usize) -> &'static Sprite {
    match CATEGORIES[i].key {
        "appearance" => &sprites::CFG_APPEARANCE,
        "input" => &sprites::CFG_INPUT,
        "layout" => &sprites::CFG_LAYOUT,
        "accessibility" => &sprites::CFG_ACCESSIBILITY,
        "filtering" => &sprites::CFG_FILTERING,
        "list" => &sprites::CFG_LIST,
        "favorites" => &sprites::CFG_FAVORITES,
        "search" => &sprites::CFG_SEARCH,
        "cheats" => &sprites::CFG_CHEATS,
        "performance" => &sprites::CFG_PERFORMANCE,
        "debug" => &sprites::CFG_DEBUG,
        _ => &sprites::CFG_RESET,
    }
}

/// Draws the screen; returns the tooltip.
pub(super) fn draw(game: &Game<'_>, batch: &mut HudBatch, s: &mut State) -> Option<Vec<(String, u32)>> {
    let mut c = s.config.take()?;
    let saved = glue::settings(game, s);
    // The buttons follow the saved theme, wherever the screen is drawn.
    super::roughlyenoughitems_view::set_dark(saved.dark);
    let tip = c.draw(game, batch, s.ru, saved);
    s.config = Some(c);
    tip
}

impl ConfigScreen {
    fn draw(&mut self, game: &Game<'_>, batch: &mut HudBatch, ru: bool, saved: rei::Settings) -> Option<Vec<(String, u32)>> {
        let now = Instant::now();
        let dt = now.duration_since(self.last).as_secs_f32().min(0.1);
        self.last = now;
        // A modifier key alone is kept once it is let go.
        if let Some(cap) = &self.capture
            && let Some(k) = cap.key.as_deref()
            && let Some(bit) = modifier_of(k)
        {
            let down = match bit {
                SHIFT => game.has_shift_down(),
                CTRL => game.has_control_down(),
                _ => game.has_alt_down(),
            };
            if !down {
                self.commit_capture();
            }
        }
        self.hits.clear();
        let cursor = game.cursor;
        let (w, h) = (batch.width, batch.height);
        let mut tip = None;
        match self.host {
            // The game's in-world menu background.
            Host::Container => {
                batch.fill_gradient(&game.gui.atlas, (0.0, 0.0, w, h), argb(0xc010_1010), argb(0xd010_1010));
                fill(game, batch, [0.0, 0.0, w, h], 0x4000_0000);
            }
            // `Screen.extractBackground`: the blur over the world or the
            // panorama, then the menu background.
            Host::Options { in_world } => {
                game.native_apply_screen_blur();
                horizon_client::ui::menu_background(batch, &game.gui.atlas, 0.0, 0.0, w, h, in_world);
            }
        }
        // The title.
        let title = rei::tr(ru, "config_title");
        let tw = width(game, title);
        text(game, batch, title, (w / 2.0 - tw / 2.0).floor(), 12.0, if saved.dark { 0xffbb_bbbb } else { WHITE }, true);
        let side = (w / 4.2).round();
        let two = w - 20.0 - side > 330.0;
        let millis = glue_millis();
        if self.searching {
            self.areas[0] = None;
            self.areas[1] = None;
            let back = [8.0, 32.0, side, 20.0];
            button(game, batch, back, true, rei::tr(ru, "back_arrow"));
            self.hits.push((back, CHit::Back));
            let hole = [12.0 + side, 32.0, w - 20.0 - side, 20.0];
            fill(game, batch, hole, 0x7000_0000);
            let f = [18.0 + side, 38.0, w - 30.0 - side, 12.0];
            self.draw_field(game, batch, ru, f);
            self.hits.push((hole, CHit::Field));
            let r = [8.0, 56.0, w - 16.0, h - 88.0];
            let term = self.field.value.clone();
            if self.results.as_ref().is_none_or(|(t, _)| *t != term) {
                self.results = Some((term.clone(), search(&term, ru)));
            }
            let views = self.results.as_ref().map(|(_, v)| v.clone()).unwrap_or_default();
            if let Some(t) = self.options_panel(game, batch, ru, saved, r, &views, 2, dt, millis) {
                tip = Some(t);
            }
        } else {
            let (cr, er) = if two {
                ([8.0, 32.0, side, h - 64.0], [12.0 + side, 32.0, w - 20.0 - side, h - 64.0])
            } else {
                ([8.0, 32.0, 38.0, h - 64.0], [50.0, 32.0, w - 58.0, h - 64.0])
            };
            if let Some(t) = self.categories(game, batch, ru, cr, two, dt, millis) {
                tip = Some(t);
            }
            self.areas[2] = None;
            let views: Vec<GroupView> = CATEGORIES[self.category].groups.iter().map(GroupView::whole).collect();
            if let Some(t) = self.options_panel(game, batch, ru, saved, er, &views, 1, dt, millis) {
                tip = Some(t);
            }
        }
        // Cancel and Done.
        let cancel = [(w / 2.0).floor() - 160.0, h - 26.0, 150.0, 20.0];
        let done = [(w / 2.0).floor() + 10.0, h - 26.0, 150.0, 20.0];
        button(game, batch, cancel, true, &game.translate("gui.cancel", rei::tr(ru, "cancel")));
        button(game, batch, done, true, &game.translate("gui.done", "Done"));
        self.hits.push((cancel, CHit::Cancel));
        self.hits.push((done, CHit::Done));
        // The open selection menu, over everything.
        if self.menu.is_some() {
            self.draw_menu(game, batch, ru, saved);
            tip = None;
        }
        let _ = cursor;
        tip
    }

    fn draw_field(&mut self, game: &Game<'_>, batch: &mut HudBatch, ru: bool, f: Rect) {
        let hovered = inside(game.cursor, f);
        let value = self.field.value.clone();
        if value.is_empty() && !self.field.focused {
            let color = if hovered { 0xddea_eaea } else { 0xffa0_a0a0 };
            text(game, batch, rei::tr(ru, "search_options"), f[0], f[1], color, true);
        } else {
            // What fits, from the start.
            let shown = cut(game, &value, f[2]);
            let (a, b) = (self.field.cursor.min(self.field.highlight), self.field.cursor.max(self.field.highlight));
            if a != b {
                let x0 = f[0] + width(game, &shown.chars().take(a).collect::<String>());
                let x1 = f[0] + width(game, &shown.chars().take(b).collect::<String>());
                fill(game, batch, [x0, f[1] - 1.0, (x1 - x0).max(0.0), 11.0], 0xff00_00ff);
            }
            text(game, batch, &shown, f[0], f[1], VALUE, true);
            if self.field.focused {
                let before: String = shown.chars().take(self.field.cursor).collect();
                let cx = f[0] + width(game, &before);
                if self.field.cursor >= shown.chars().count() {
                    text(game, batch, "_", cx, f[1], VALUE, true);
                } else {
                    fill(game, batch, [cx, f[1] - 1.0, 1.0, 12.0], 0xffd0_d0d0);
                }
            }
        }
        if self.field.focused {
            let (x0, x1) = (f[0] - 6.0, f[0] + f[2] + 4.0);
            fill(game, batch, [x0, 32.0, x1 - x0, 1.0], VALUE);
            fill(game, batch, [x0, 51.0, x1 - x0, 1.0], VALUE);
            fill(game, batch, [f[0] - 7.0, 32.0, 1.0, 20.0], VALUE);
            fill(game, batch, [f[0] + f[2] + 3.0, 32.0, 1.0, 20.0], VALUE);
        }
    }

    /// The categories' panel: "Search options..." and the twelve categories,
    /// or their icons alone when the screen is narrow.
    #[allow(clippy::too_many_arguments)]
    fn categories(&mut self, game: &Game<'_>, batch: &mut HudBatch, ru: bool, cr: Rect, two: bool, dt: f32, millis: u64) -> Option<Vec<(String, u32)>> {
        let cursor = game.cursor;
        let mut tip = None;
        panel(game, batch, cr);
        self.areas[0] = Some(cr);
        let (l, gap, pad) = if two {
            ([cr[0] + 3.0, cr[1] + 5.0, cr[2] - 6.0, cr[3] - 10.0], 3.0, 10.0)
        } else {
            let inset = ((cr[2] - 6.0 - 16.0) / 2.0).floor();
            ([cr[0] + inset, cr[1] + 9.0, cr[2] - 2.0 * inset, cr[3] - 18.0], 7.0, 18.0)
        };
        let heights: Vec<f32> = std::iter::once(if two { 21.0 } else { 16.0 })
            .chain(CATEGORIES.iter().map(|c| if !two { 16.0 } else if c.desc.is_some() { 24.0 } else { 21.0 }))
            .collect();
        let list_h = heights.iter().sum::<f32>() + gap * (heights.len() - 1) as f32;
        let inner_w = if list_h > l[3] { l[2] - 6.0 } else { l[2] };
        let content = list_h + pad;
        self.scrolls[0].step(content - cr[3], dt);
        let scroll = self.scrolls[0].at();
        let clip = if content > cr[3] { [cr[0], cr[1], cr[2] - 6.0, cr[3]] } else { cr };
        let start = batch.vertices.len();
        let mut y = l[1] - scroll;
        let reduced = self.is("appearance.reduced_motion", "true");
        for (i, &ih) in heights.iter().enumerate() {
            let x = l[0];
            if i == self.category + 1 {
                let outer = [x - 1.0, y - 1.0, inner_w + 2.0, ih + 2.0];
                let c = if inside(cursor, outer) { 0xffd0_d0d0 } else { 0xff8f_8f8f };
                fill(game, batch, outer, c);
                fill(game, batch, [x, y, inner_w, ih], 0xff00_0000);
            }
            if i == 0 {
                if two {
                    let sw = if list_h + 6.0 > cr[3] { cr[2] - 12.0 } else { cr[2] - 6.0 };
                    let hit = [x - 1.0, y - 1.0, sw + 2.0, 21.0];
                    let lit = inside(cursor, hit) && inside(cursor, clip);
                    fill(game, batch, [x + 1.0, y + 1.0, sw - 2.0, 18.0], if lit { 0x50ff_ffff } else { 0x25ff_ffff });
                    let line = if lit { 0x90ff_ffff } else { 0x45ff_ffff };
                    fill(game, batch, [x - 1.0, y - 1.0, sw + 2.0, 1.0], line);
                    fill(game, batch, [x - 1.0, y + 20.0, sw + 2.0, 1.0], line);
                    fill(game, batch, [x - 1.0, y, 1.0, 20.0], line);
                    fill(game, batch, [x + sw, y, 1.0, 20.0], line);
                    blit(game, batch, &sprites::CFG_SEARCH_OPTIONS, x + 3.0, y + 3.0, 1.0);
                    let label_color = if lit { 0xffe1_e1e1 } else { NAME };
                    batch.text(&game.gui.atlas, rei::tr(ru, "search_options"), x + 21.0, y + 6.5, argb(label_color), true);
                    if let Some(r) = intersect(hit, clip) {
                        self.hits.push((r, CHit::SearchButton));
                    }
                } else {
                    let hit = [x - 1.0, y - 1.0, 18.0, 18.0];
                    let lit = inside(cursor, hit) && inside(cursor, clip);
                    let (ox, oy) = (x - 0.5, y - 0.5);
                    fill(game, batch, [ox - 1.0, oy - 1.0, 18.0, 18.0], if lit { 0x50ff_ffff } else { 0x25ff_ffff });
                    let line = if lit { 0x90ff_ffff } else { 0x45ff_ffff };
                    fill(game, batch, [ox - 3.0, oy - 3.0, 22.0, 1.0], line);
                    fill(game, batch, [ox - 3.0, oy + 18.0, 22.0, 1.0], line);
                    fill(game, batch, [ox - 3.0, oy - 2.0, 1.0, 20.0], line);
                    fill(game, batch, [ox + 18.0, oy - 2.0, 1.0, 20.0], line);
                    blit(game, batch, &sprites::CFG_SEARCH_OPTIONS, x, y, 1.0);
                    if let Some(r) = intersect(hit, clip) {
                        self.hits.push((r, CHit::SearchButton));
                    }
                    if lit {
                        tip = Some(vec![(rei::tr(ru, "search_options").to_owned(), WHITE)]);
                    }
                }
            } else {
                let k = i - 1;
                let cat = &CATEGORIES[k];
                let name = tr2(ru, cat.name);
                if two {
                    let has_desc = cat.desc.is_some();
                    blit(game, batch, icon(k), x + 3.0, y + if has_desc { 5.0 } else { 3.0 }, 1.0);
                    text(game, batch, name, x + 21.0, y + if has_desc { 5.0 } else { 7.0 }, NAME, true);
                    if let Some(d) = cat.desc {
                        let desc = tr2(ru, d);
                        let ew = cr[2] - 6.0;
                        let aw = ((ew - 27.0) / 0.75).floor();
                        let tw = width(game, desc);
                        let (ox, oy) = (x + 21.0, y + 15.0);
                        let s0 = batch.vertices.len();
                        if tw > aw && !reduced {
                            let tw2 = (tw / 0.75).round();
                            let aw2 = (aw / 0.75).round();
                            let period = (((tw2 + 10.0) * tw2 / 3.0) as u64).max(1);
                            let off = (millis % period) as f32 / tw2 * 3.0;
                            batch.text(&game.gui.atlas, desc, ox + aw2 - tw2 - 10.0 - off, oy, argb(0xffb0_b0b0), true);
                            batch.text(&game.gui.atlas, desc, ox + aw2 - off, oy, argb(0xffb0_b0b0), true);
                        } else {
                            batch.text(&game.gui.atlas, desc, ox, oy, argb(0xffb0_b0b0), true);
                        }
                        batch.scale_since(s0, ox, oy, 0.75, 0.75);
                        batch.clip_since(s0, ox, oy, ox + aw * 0.75, oy + 9.0 * 0.75);
                    }
                } else {
                    blit(game, batch, icon(k), x, y, 1.0);
                    let r = [x, y, 16.0, 16.0];
                    if inside(cursor, r) && inside(cursor, clip) {
                        let mut lines = vec![(format!("§n{name}"), WHITE)];
                        if let Some(d) = cat.desc {
                            lines.push((tr2(ru, d).to_owned(), WHITE));
                        }
                        tip = Some(lines);
                    }
                }
                if let Some(r) = intersect([x, y, inner_w, ih], clip) {
                    self.hits.push((r, CHit::Category(k)));
                }
            }
            y += ih + gap;
        }
        batch.clip_since(start, clip[0], clip[1], clip[0] + clip[2], clip[1] + clip[3]);
        scrollbar(game, batch, cr, content, scroll);
        tip
    }

    /// An options panel: the groups of a category, or the search results.
    #[allow(clippy::too_many_arguments)]
    fn options_panel(
        &mut self,
        game: &Game<'_>,
        batch: &mut HudBatch,
        ru: bool,
        saved: rei::Settings,
        er: Rect,
        views: &[GroupView],
        which: usize,
        dt: f32,
        millis: u64,
    ) -> Option<Vec<(String, u32)>> {
        let mut tip = None;
        panel(game, batch, er);
        self.areas[which] = Some(er);
        let list = [er[0] + 6.0, er[1] + 6.0, er[2] - 12.0, er[3] - 12.0];
        let gw = er[2] - 18.0;
        let heights: Vec<f32> = views.iter().map(|v| group_height(game, self, ru, v, gw)).collect();
        let list_h = if heights.is_empty() { 0.0 } else { heights.iter().sum::<f32>() + 7.0 * (heights.len() - 1) as f32 };
        let content = list_h + 10.0;
        self.scrolls[which].step(content - er[3], dt);
        let scroll = self.scrolls[which].at();
        let clip = if content > er[3] { [er[0], er[1], er[2] - 6.0, er[3]] } else { er };
        let start = batch.vertices.len();
        let mut y = list[1] - scroll;
        for (v, gh) in views.iter().zip(heights) {
            if y + gh >= clip[1] && y <= clip[1] + clip[3]
                && let Some(t) = self.group(game, batch, ru, saved, v, list[0], y, gw, clip, millis)
            {
                tip = Some(t);
            }
            y += gh + 7.0;
        }
        batch.clip_since(start, clip[0], clip[1], clip[0] + clip[2], clip[1] + clip[3]);
        scrollbar(game, batch, er, content, scroll);
        tip
    }

    #[allow(clippy::too_many_arguments)]
    fn group(
        &mut self,
        game: &Game<'_>,
        batch: &mut HudBatch,
        ru: bool,
        saved: rei::Settings,
        v: &GroupView,
        gx: f32,
        gy: f32,
        gw: f32,
        clip: Rect,
        millis: u64,
    ) -> Option<Vec<(String, u32)>> {
        let mut tip = None;
        let title = tr2(ru, v.group.name);
        let shown = if v.title_lit { lit(title, &v.query, "§n") } else { format!("§n{title}") };
        text(game, batch, &shown, gx, gy + 3.0, NAME, true);
        let base = gy + 18.0;
        let p = place(game, self, ru, v, gw);
        let rw = if let Place::Right(half) = p { half } else { gw };
        let oh = opts_height(game, ru, v, rw);
        let slot_bg = |batch: &mut HudBatch, r: Rect| {
            fill(game, batch, r, 0xff33_3333);
            fill(game, batch, [r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0], 0xff00_0000);
        };
        let opts_y = match p {
            Place::Top(ph) => {
                let r = [gx, base, gw, ph];
                slot_bg(batch, r);
                self.preview(game, batch, ru, saved, v.group.preview, r, false);
                base + ph + 4.0
            }
            Place::Right(half) => {
                let r = [gx + half + 2.0, base, gw - half - 4.0, oh];
                slot_bg(batch, r);
                self.preview(game, batch, ru, saved, v.group.preview, r, true);
                base
            }
            Place::Bottom(ph) => {
                let r = [gx, base + oh + 4.0, gw, ph];
                slot_bg(batch, r);
                self.preview(game, batch, ru, saved, v.group.preview, r, false);
                base
            }
            Place::None => base,
        };
        let mut y = opts_y;
        let n = v.opts.len();
        for (i, o) in v.opts.iter().enumerate() {
            let rh = row_height(game, ru, o, rw);
            if let Some(t) = self.option(game, batch, ru, v, o, gx, y, rw, clip, millis) {
                tip = Some(t);
            }
            y += rh;
            if i + 1 < n {
                let mut x = 0.0;
                while x <= rw {
                    let x0 = x.min(rw);
                    let x1 = (x + 2.0).min(rw);
                    fill(game, batch, [gx + x0, y + 1.0, x1 - x0, 1.0], DESC);
                    x += 4.0;
                }
                y += 7.0;
            }
        }
        tip
    }

    #[allow(clippy::too_many_arguments)]
    fn option(
        &mut self,
        game: &Game<'_>,
        batch: &mut HudBatch,
        ru: bool,
        v: &GroupView,
        o: &'static Opt,
        x: f32,
        y: f32,
        rw: f32,
        clip: Rect,
        millis: u64,
    ) -> Option<Vec<(String, u32)>> {
        let cursor = game.cursor;
        let name = tr2(ru, o.name);
        let lit_desc = v.lit.get(o.id).copied();
        if o.id == "appearance.rainbow" {
            // Each glyph its own hue (`TextTransformations.applyRainbow`).
            let mut cx = 0.0;
            for ch in name.chars() {
                let s = ch.to_string();
                let hue = ((millis as i64 - (cx * 10.0) as i64).rem_euclid(2000)) as f32 / 2000.0;
                text_at(game, batch, &s, x + cx, y, hsb(hue, 0.8, 0.95), true);
                cx += width(game, &s);
            }
        } else {
            let shown = if lit_desc == Some(false) { lit(name, &v.query, "") } else { name.to_owned() };
            text(game, batch, &shown, x, y, NAME, true);
        }
        if let Some(d) = o.desc {
            let desc = tr2(ru, d);
            let shown = if lit_desc == Some(true) { lit(desc, &v.query, "") } else { desc.to_owned() };
            for (i, line) in wrap(game, &shown, rw).iter().enumerate() {
                batch.text_styled(&game.gui.atlas, line, x, (y + 12.0 + 12.0 * i as f32).floor(), argb(DESC), false);
            }
        }
        // The value, right-aligned, with its arrow.
        let max = rw - 12.0 - width(game, name);
        let value = value_text(game, self, ru, o, max);
        let vw = width(game, &value);
        let tx = x + rw - 8.0 - vw;
        let label = [tx - 1.0, y - 5.0, vw + 2.0, 14.0];
        let hovered = inside(cursor, label) && inside(cursor, clip);
        let locked = self.locked(o);
        text(game, batch, &value, tx, y, VALUE, true);
        if hovered {
            underline(game, batch, tx, y, vw, if locked { 0xffff_5555 } else { VALUE }, true);
        }
        blit(game, batch, &sprites::CFG_SELECTOR, x + rw - 7.0, y + 1.5, 0.5);
        if clickable(o)
            && !locked
            && let Some(r) = intersect(label, clip)
        {
            self.hits.push((r, CHit::Value(o, label)));
        }
        if hovered && o.fixed && !locked {
            return Some(vec![(rei::tr(ru, "not_ported").to_owned(), 0xffaa_aaaa)]);
        }
        None
    }

    /// A group's preview in `r` (`InterfacePreviewer`, `TooltipPreviewer`,
    /// `AccessibilityDisplayPreviewer`), with the values being edited.
    #[allow(clippy::too_many_arguments)]
    fn preview(&mut self, game: &Game<'_>, batch: &mut HudBatch, ru: bool, saved: rei::Settings, kind: Preview, r: Rect, right: bool) {
        let dark = self.is("appearance.theme", "dark");
        let (x, y, w, h) = (r[0], r[1], r[2], r[3]);
        match kind {
            Preview::None => {}
            Preview::Interface => {
                let base_dark = if right { dark } else { saved.dark };
                nine(game, batch, pick(&sprites::PANEL, &sprites::PANEL_DARK, base_dark), [x + 2.0, y + 2.0, w - 4.0, h - 4.0]);
                let oy = if right { ((h - 80.0) / 2.0).floor() } else { 0.0 };
                let d = (x + ((w - 124.0) / 2.0).floor(), y + oy + 7.0);
                match self.value("appearance.recipe_border") {
                    "lighter" => nine(game, batch, pick(&sprites::PANEL_LIGHTER, &sprites::PANEL_LIGHTER_DARK, dark), [d.0, d.1, 124.0, 66.0]),
                    "none" => {}
                    _ => nine(game, batch, pick(&sprites::PANEL, &sprites::PANEL_DARK, dark), [d.0, d.1, 124.0, 66.0]),
                }
                let s = (d.0 + 6.0, d.1 + 6.0);
                blit(game, batch, pick(&sprites::ARROW_EMPTY, &sprites::ARROW_EMPTY_DARK, dark), s.0 + 58.0, s.1 + 18.0, 1.0);
                nine(game, batch, pick(&sprites::PANEL_SLOT, &sprites::PANEL_SLOT_DARK, dark), [s.0 + 86.0, s.1 + 14.0, 26.0, 26.0]);
                let cobble = glue::plain_stack("minecraft:cobblestone");
                for row in 0..3 {
                    for col in 0..3 {
                        let (sx, sy) = (s.0 + 18.0 * col as f32, s.1 + 18.0 * row as f32);
                        blit(game, batch, pick(&sprites::SLOT, &sprites::SLOT_DARK, dark), sx, sy, 1.0);
                        if !(row == 1 && col == 1)
                            && let Some(st) = &cobble
                        {
                            item(game, batch, st, sx + 1.0, sy + 1.0);
                        }
                    }
                }
                if let Some(st) = glue::plain_stack("minecraft:furnace") {
                    item(game, batch, &st, s.0 + 91.0, s.1 + 19.0);
                }
            }
            Preview::Tooltip => {
                let lines = tooltip_lines(game, self, ru, w);
                let (min_w, min_h) = tooltip_size(game, &lines);
                let tx = x + (6.0f32).max(((w - min_w) / 2.0).floor());
                let tw = (w - 12.0).min(min_w);
                let th = if right { min_h.min(h - 32.0) } else { min_h };
                let off = if right { ((h - (28.0 + th)) / 2.0).floor().max(0.0) } else { 4.0 };
                let ty = y + 28.0 + off;
                if let Some(st) = glue::plain_stack("minecraft:oak_planks") {
                    item_scaled(game, batch, &st, x + (w / 2.0).floor() - 12.0, y + off, 1.5);
                }
                let bg = 0xf010_0010;
                let fr = |batch: &mut HudBatch, x0: f32, y0: f32, x1: f32, y1: f32, c: u32| fill(game, batch, [x0, y0, x1 - x0, y1 - y0], c);
                fr(batch, tx - 3.0, ty - 4.0, tx + tw + 3.0, ty - 3.0, bg);
                fr(batch, tx - 3.0, ty + th + 3.0, tx + tw + 3.0, ty + th + 4.0, bg);
                fr(batch, tx - 3.0, ty - 3.0, tx + tw + 3.0, ty + th + 3.0, bg);
                fr(batch, tx - 4.0, ty - 3.0, tx - 3.0, ty + th + 3.0, bg);
                fr(batch, tx + tw + 3.0, ty - 3.0, tx + tw + 4.0, ty + th + 3.0, bg);
                let atlas = &game.gui.atlas;
                batch.fill_gradient(atlas, (tx - 3.0, ty - 2.0, 1.0, th + 4.0), argb(0x5050_00ff), argb(0x5028_007f));
                batch.fill_gradient(atlas, (tx + tw + 2.0, ty - 2.0, 1.0, th + 4.0), argb(0x5050_00ff), argb(0x5028_007f));
                fr(batch, tx - 3.0, ty - 3.0, tx + tw + 3.0, ty - 2.0, 0x5050_00ff);
                fr(batch, tx - 3.0, ty + th + 2.0, tx + tw + 3.0, ty + th + 3.0, 0x5028_007f);
                let start = batch.vertices.len();
                let mut ly = ty + 2.0;
                for (i, line) in lines.iter().enumerate() {
                    batch.text_styled(atlas, line, tx + 2.0, ly, argb(WHITE), false);
                    ly += if i == 0 { 12.0 } else { 10.0 };
                }
                batch.clip_since(start, x, y, x + w, y + h);
            }
            Preview::Accessibility => {
                let larger = self.is("accessibility.larger_tabs", "true");
                let arrows = self.is("accessibility.larger_arrow_buttons", "true");
                let ts = if larger { 28.0 } else { 24.0 };
                let n = if larger { 3 } else { 4 };
                if self.preview_tab >= n {
                    self.preview_tab = 0;
                }
                let cx = x + (w / 2.0).floor();
                let icons = ["minecraft:crafting_table", "minecraft:furnace", "minecraft:smoker", "minecraft:blast_furnace"];
                let tab = |i: usize| [cx - ts * n as f32 / 2.0 + i as f32 * ts, y + 47.0 - ts, ts, ts];
                let draw_tab = |batch: &mut HudBatch, i: usize, sel: bool| {
                    let r = tab(i);
                    let sprite = match (larger, sel) {
                        (false, false) => pick(&sprites::TAB_UNSELECTED, &sprites::TAB_UNSELECTED_DARK, saved.dark),
                        (false, true) => pick(&sprites::TAB_SELECTED, &sprites::TAB_SELECTED_DARK, saved.dark),
                        (true, false) => pick(&sprites::TAB_LARGE_UNSELECTED, &sprites::TAB_LARGE_UNSELECTED_DARK, saved.dark),
                        (true, true) => pick(&sprites::TAB_LARGE_SELECTED, &sprites::TAB_LARGE_SELECTED_DARK, saved.dark),
                    };
                    blit(game, batch, sprite, r[0], r[1] + 2.0, 1.0);
                    if let Some(st) = glue::plain_stack(icons[i]) {
                        item(game, batch, &st, r[0] + (ts / 2.0) - 8.0, r[1] + (ts / 2.0) - 5.0);
                    }
                };
                for i in 0..n {
                    if i != self.preview_tab {
                        draw_tab(batch, i, false);
                    }
                }
                let start = batch.vertices.len();
                nine(game, batch, pick(&sprites::PANEL, &sprites::PANEL_DARK, saved.dark), [cx - 52.0, y + 47.0, 104.0, 28.0]);
                batch.clip_since(start, x + 1.0, y + 1.0, x + w - 1.0, y + 52.0);
                draw_tab(batch, self.preview_tab, true);
                for i in 0..n {
                    self.hits.push((tab(i), CHit::PreviewTab(i)));
                }
                let b = if arrows { 16.0 } else { 10.0 };
                let by = y + 46.0 - ts + 1.0 - b;
                let left = [cx - 52.0, by, b, b];
                let right_b = [cx - 52.0 + 104.0 - b - if arrows { 1.0 } else { 0.0 }, by, b, b];
                for (r, back) in [(left, true), (right_b, false)] {
                    button(game, batch, r, true, "");
                    if arrows {
                        blit(game, batch, if back { &sprites::ARROW_LEFT } else { &sprites::ARROW_RIGHT }, r[0] + 4.0, r[1] + 4.0, 1.0);
                    } else {
                        blit(game, batch, if back { &sprites::ARROW_LEFT_SMALL } else { &sprites::ARROW_RIGHT_SMALL }, r[0] + 2.0, r[1] + 2.5, 1.0);
                    }
                }
            }
        }
    }

    /// The selection menu under (or over) its value.
    fn draw_menu(&mut self, game: &Game<'_>, batch: &mut HudBatch, ru: bool, saved: rei::Settings) {
        let Some(m) = &self.menu else {
            return;
        };
        let OptKind::Choice(values) = m.opt.kind else {
            return;
        };
        let cursor = game.cursor;
        let (sw, sh) = (batch.width, batch.height);
        let a = m.anchor;
        let labels: Vec<String> = values
            .iter()
            .map(|v| {
                let l = choice_label(self, ru, m.opt, v.0);
                if v.0 == m.opt.default { rei::tr(ru, "value_default").replace("%s", &l) } else { l }
            })
            .collect();
        let row_w = labels.iter().map(|l| width(game, l) + 12.0).fold(10.0, f32::max);
        let content_h = 12.0 * labels.len() as f32;
        let down = sh - (a[1] + a[3]) > a[1];
        let y = if down { a[1] + a[3] } else { a[1] - (content_h + 2.0) };
        let inner_h = content_h.min(sh - 20.0 - y);
        let scrolls = inner_h < content_h;
        let mw = row_w + 2.0 + if scrolls { 6.0 } else { 0.0 };
        let right = sw - (a[0] + a[2]) >= mw + 10.0;
        let x = if right { a[0] } else { a[0] + a[2] - mw };
        let full = [x, y, mw, inner_h + 2.0];
        let reduced = saved.reduced_motion;
        let t = if reduced { 1.0 } else { (m.at.elapsed().as_millis() as f32 / 300.0).min(1.0) };
        let e = 1.0 - (1.0 - t).powi(3);
        let corner = (if right { x } else { x + mw }, if down { y } else { y + full[3] });
        let anim = [corner.0 + (full[0] - corner.0) * e, corner.1 + (full[1] - corner.1) * e, (full[2] * e).max(0.1), (full[3] * e).max(0.1)];
        let border = match (inside(cursor, full), saved.dark) {
            (true, false) => WHITE,
            (true, true) => 0xffff_bb4d,
            (false, _) => 0xffa0_a0a0,
        };
        fill(game, batch, anim, border);
        let inner = [anim[0] + 1.0, anim[1] + 1.0, anim[2] - 2.0, anim[3] - 2.0];
        fill(game, batch, inner, 0xff00_0000);
        self.hits.push((full, CHit::MenuArea));
        let max_scroll = (content_h - inner_h).max(0.0);
        let scroll = m.scroll.clamp(0.0, max_scroll);
        let start = batch.vertices.len();
        for (i, l) in labels.iter().enumerate() {
            let r = [x + 1.0, y + 1.0 + 12.0 * i as f32 - scroll, row_w, 12.0];
            let lit = inside(cursor, r) && inside(cursor, inner);
            if lit {
                fill(game, batch, r, 0xff45_4545);
            }
            text(game, batch, l, r[0] + 2.0, r[1] + 2.0, if lit { WHITE } else { 0xff88_8888 }, false);
            if let Some(hr) = intersect(r, inner) {
                self.hits.push((hr, CHit::MenuEntry(i)));
            }
        }
        batch.clip_since(start, inner[0], inner[1], inner[0] + inner[2], inner[1] + inner[3]);
        if scrolls {
            let bar = [inner[0], inner[1], inner[2], inner[3]];
            scrollbar(game, batch, bar, content_h, scroll);
        }
    }
}

/// Text at a point that need not be whole (the rainbow's glyphs).
fn text_at(game: &Game<'_>, batch: &mut HudBatch, s: &str, x: f32, y: f32, color: u32, shadow: bool) {
    batch.text(&game.gui.atlas, s, x, y, argb(color), shadow);
}

/// Java's `Color.HSBtoRGB`, opaque.
fn hsb(h: f32, s: f32, v: f32) -> u32 {
    let h6 = (h - h.floor()) * 6.0;
    let i = h6.floor();
    let f = h6 - i;
    let (p, q, t) = (v * (1.0 - s), v * (1.0 - s * f), v * (1.0 - s * (1.0 - f)));
    let (r, g, b) = match i as i32 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    let c = |x: f32| (x * 255.0 + 0.5) as u32;
    0xff00_0000 | (c(r) << 16) | (c(g) << 8) | c(b)
}

/// Milliseconds the mod has run, for the marquee and the rainbow.
fn glue_millis() -> u64 {
    thread_local! {
        static START: Instant = Instant::now();
    }
    START.with(|s| s.elapsed().as_millis() as u64)
}

// ---- input ------------------------------------------------------------------------------

impl ConfigScreen {
    fn hit_at(&self, p: (f32, f32)) -> Option<CHit> {
        self.hits.iter().rev().find(|(r, _)| inside(p, *r)).map(|(_, h)| *h)
    }

    fn commit_capture(&mut self) {
        if let Some(cap) = self.capture.take() {
            let key = cap.key.unwrap_or_else(|| "key.keyboard.unknown".to_owned());
            self.keys.insert(cap.name, key);
            self.pending.insert(cap.opt.id, mods_text(cap.mods));
        }
    }

    /// Text pasted into the search field.
    pub(super) fn paste(&mut self, text: &str) {
        if self.searching && self.field.focused {
            self.field.insert_text(text);
        }
    }

    fn set_searching(&mut self, on: bool) {
        self.searching = on;
        self.field.set_value("");
        self.field.focused = false;
        self.results = None;
        self.scrolls = [Scroll::default(); 3];
        self.menu = None;
    }

    /// A click on a value (`ConfigOptionValueWidget`).
    fn click_value(&mut self, s_pending: &mut Vec<Action>, opt: &'static Opt, anchor: Rect) {
        match opt.kind {
            OptKind::Choice(values) if values.len() == 2 => {
                let cur = self.value(opt.id).to_owned();
                let next = match values.iter().position(|v| v.0 == cur) {
                    Some(i) => values[(i + 1) % 2].0,
                    None => values[0].0,
                };
                self.pending.insert(opt.id, next.to_owned());
            }
            OptKind::Choice(_) => {
                self.menu = Some(SelMenu { opt, anchor, at: Instant::now(), scroll: 0.0 });
            }
            OptKind::Key(name) => {
                self.menu = None;
                self.capture = Some(Capture { opt, name, key: None, mods: 0 });
            }
            OptKind::Link(_) => match opt.id {
                "reset.reload_plugins" => s_pending.push(Action::Reload { search_only: false }),
                "reset.reload_search" => s_pending.push(Action::Reload { search_only: true }),
                _ => {}
            },
            OptKind::Text => {}
        }
    }

    /// Done: every value kept (`REIConfigScreen`'s save).
    fn save(&self, game: &Game<'_>, pending: &mut Vec<Action>) {
        for o in all_options() {
            if o.fixed {
                continue;
            }
            match o.kind {
                OptKind::Choice(_) => game.native_set_setting(rei::ID, o.id, self.value(o.id)),
                OptKind::Key(name) => {
                    game.native_set_setting(rei::ID, o.id, self.value(o.id));
                    if let Some(key) = self.keys.get(name) {
                        pending.push(Action::SetKey { name: name.to_owned(), key: key.clone() });
                    }
                }
                _ => {}
            }
        }
    }
}

/// A mouse button pressed while the screen is open.
pub(super) fn press(game: &Game<'_>, s: &mut State, p: (f32, f32), button: i32) {
    let Some(mut c) = s.config.take() else {
        return;
    };
    let mut close = None;
    // A keybind being set takes the button.
    if let Some(cap) = c.capture.as_mut() {
        match cap.key.as_deref().and_then(modifier_of) {
            Some(bit) => {
                cap.mods |= bit;
                cap.key = Some(mouse_key(button));
            }
            None if cap.key.is_none() => cap.key = Some(mouse_key(button)),
            None => {}
        }
        s.config = Some(c);
        return;
    }
    let hit = c.hit_at(p);
    if c.menu.is_some() {
        match hit {
            Some(CHit::MenuEntry(i)) => {
                if let Some(m) = c.menu.take()
                    && let OptKind::Choice(values) = m.opt.kind
                    && let Some(v) = values.get(i)
                {
                    c.pending.insert(m.opt.id, v.0.to_owned());
                }
            }
            Some(CHit::MenuArea) => {}
            _ => c.menu = None,
        }
        s.config = Some(c);
        return;
    }
    if !matches!(hit, Some(CHit::Field)) {
        c.field.focused = false;
    }
    match hit {
        Some(CHit::SearchButton) => c.set_searching(true),
        Some(CHit::Category(k)) => {
            if c.category != k {
                c.category = k;
                c.scrolls[1] = Scroll::default();
            }
        }
        Some(CHit::Value(opt, anchor)) if button == 0 => {
            let mut pending = std::mem::take(&mut s.pending);
            c.click_value(&mut pending, opt, anchor);
            s.pending = pending;
        }
        Some(CHit::Back) if button == 0 => c.set_searching(false),
        Some(CHit::Field) => {
            c.field.focused = true;
            c.field.reset_blink();
        }
        Some(CHit::Cancel) if button == 0 => close = Some(false),
        Some(CHit::Done) if button == 0 => close = Some(true),
        Some(CHit::PreviewTab(i)) if button == 0 => c.preview_tab = i,
        _ => {}
    }
    match close {
        Some(true) => {
            let mut pending = std::mem::take(&mut s.pending);
            c.save(game, &mut pending);
            s.pending = pending;
            glue::reload_settings(game, s);
        }
        Some(false) => {}
        None => s.config = Some(c),
    }
}

/// A mouse button let go: it ends a keybind's capture.
pub(super) fn release(s: &mut State) {
    if let Some(c) = s.config.as_mut()
        && c.capture.as_ref().is_some_and(|cap| cap.key.is_some())
    {
        c.commit_capture();
    }
}

pub(super) fn scroll(s: &mut State, p: (f32, f32), delta: i32) {
    let Some(c) = s.config.as_mut() else {
        return;
    };
    if c.capture.is_some() {
        return;
    }
    if let Some(m) = c.menu.as_mut() {
        m.scroll = (m.scroll + 12.0 * delta as f32).max(0.0);
        if let OptKind::Choice(values) = m.opt.kind {
            m.scroll = m.scroll.min(12.0 * values.len() as f32);
        }
        return;
    }
    for (i, area) in c.areas.iter().enumerate() {
        if let Some(a) = area
            && inside(p, *a)
        {
            c.scrolls[i].target += 16.0 * delta as f32;
            return;
        }
    }
}

/// A key pressed while the screen is open: it takes every key.
pub(super) fn key(s: &mut State, key: &str, mods: u32) {
    let Some(c) = s.config.as_mut() else {
        return;
    };
    let shift = mods & 1 != 0;
    let control = mods & 2 != 0;
    let alt = mods & 4 != 0;
    if let Some(cap) = c.capture.as_mut() {
        if key == "key.keyboard.escape" {
            cap.key = Some("key.keyboard.unknown".to_owned());
            cap.mods = 0;
            c.commit_capture();
            return;
        }
        let event_mods = (if shift { SHIFT } else { 0 }) | (if control { CTRL } else { 0 }) | (if alt { ALT } else { 0 });
        match cap.key.clone() {
            None => {
                cap.key = Some(key.to_owned());
                if modifier_of(key).is_none() {
                    cap.mods |= event_mods;
                }
            }
            Some(k) => {
                if let Some(bit) = modifier_of(&k) {
                    cap.mods |= bit;
                    cap.key = Some(key.to_owned());
                } else {
                    cap.mods |= event_mods;
                }
            }
        }
        // A key that is no modifier is kept at once (REI waits for its release).
        if cap.key.as_deref().and_then(modifier_of).is_none() {
            c.commit_capture();
        }
        return;
    }
    if key == "key.keyboard.escape" {
        if c.menu.is_some() {
            c.menu = None;
        } else if c.searching {
            c.set_searching(false);
        } else {
            s.config = None;
        }
        return;
    }
    if c.field.focused && c.searching {
        match key {
            "key.keyboard.v" if control => s.pending.push(Action::ConfigPaste),
            "key.keyboard.c" if control => s.pending.push(Action::CopyText(c.field.selected_text())),
            "key.keyboard.x" if control => {
                s.pending.push(Action::CopyText(c.field.selected_text()));
                c.field.delete_chars(0);
            }
            _ => {
                if let Some(code) = horizon_client::keybinds::key_code(key) {
                    c.field.key(code, control, shift);
                }
            }
        }
        c.field.reset_blink();
    }
}

pub(super) fn char_typed(s: &mut State, ch: char) {
    if let Some(c) = s.config.as_mut()
        && c.searching
        && c.field.focused
        && c.capture.is_none()
        && !ch.is_control()
    {
        c.field.insert_text(&ch.to_string());
        c.field.reset_blink();
    }
}
