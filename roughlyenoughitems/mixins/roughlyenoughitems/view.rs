// Roughly Enough Items for Horizon: drawing. The entry list right of the
// container screen, the favorites left of it, the search field under it and
// the recipe screen over it, laid out with REI's own numbers
// (`ScreenOverlayImpl`, `PaginatedEntryListWidget`, `FavoritesListWidget`,
// `DefaultDisplayViewingScreen`, the default plugin's categories) in REI's
// textures (`roughlyenoughitems_sprites`, drawn as fills) and the game's own
// buttons. Every part drawn is recorded in `State::hits` for the mouse and
// keys of the next frame.
use super::roughlyenoughitems::{self as rei, Category, Lookup, Mark, Rect, Recipe, Shortcut, Stack, inside};
use super::roughlyenoughitems_glue::{self as glue, Data, Hit, ListCell, Menu, MenuItem, OWN_TOOLTIP, State, Status};
use super::roughlyenoughitems_sprites::{self as sprites, Sprite};
use super::{Game, Screen};
use horizon_client::hud::{HudBatch, rgba};
use horizon_client::items::ItemStack;
use horizon_client::text::Text;
use std::time::Instant;

/// An entry's cell.
const E: f32 = 18.0;
/// The recipe window's width (`guiWidth`).
const GUI_W: f32 = 190.0;
/// A display's width.
const DISPLAY_W: f32 = 150.0;

const WHITE: u32 = 0xffff_ffff;
const HOVER: u32 = 0x80ff_ffff;
const LABEL_HOVER: u32 = 0xff66_ffcc;
const STRIPE: u32 = 0xff9e_9e9e;
const DARK_TEXT: u32 = 0xff40_4040;
const YELLOW: u32 = 0xffff_ff55;
const RED: u32 = 0xffff_5555;
const GRAY: u32 = 0xffaa_aaaa;
const DARK_GRAY: u32 = 0xff55_5555;
const BLUE: u32 = 0xff55_55ff;

// ---- the theme -----------------------------------------------------------------------

thread_local! {
    /// REI's dark theme is on this frame.
    static DARK: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn dark() -> bool {
    DARK.with(std::cell::Cell::get)
}

/// The theme the buttons are drawn in (the config screen over the Options
/// screen, where no overlay frame sets it).
pub(super) fn set_dark(on: bool) {
    DARK.with(|d| d.set(on));
}

/// The light sprite, or its dark twin in the dark theme.
fn th(light: &'static Sprite, dark_twin: &'static Sprite) -> &'static Sprite {
    if dark() { dark_twin } else { light }
}

/// A sprite kept whole, stretched over `r` as the game does a nine-slice:
/// its corners as they are, its edges and middle tiled.
fn nine_tiled(game: &Game<'_>, batch: &mut HudBatch, s: &Sprite, r: Rect) {
    let b = s.border.map_or(0, |x| x[0]);
    let (sw, sh) = (s.width, s.height);
    let (tw, th) = (r[2].round().max(0.0) as u16, r[3].round().max(0.0) as u16);
    let (bw, bh) = (b.min(tw / 2), b.min(th / 2));
    let map = |t: u16, len: u16, src: u16, edge: u16| -> u16 {
        if t < edge {
            t
        } else if t >= len - edge {
            src - (len - t)
        } else {
            b + (t - edge) % (src - 2 * b).max(1)
        }
    };
    let atlas = &game.gui.atlas;
    for ty in 0..th {
        let sy = usize::from(map(ty, th, sh, bh));
        let at = |tx: u16| s.pixels[sy * usize::from(sw) + usize::from(map(tx, tw, sw, bw))];
        let mut tx = 0;
        while tx < tw {
            let c = at(tx);
            let mut len = 1;
            while tx + len < tw && at(tx + len) == c {
                len += 1;
            }
            if c >> 24 != 0 {
                batch.fill(atlas, r[0] + f32::from(tx), r[1] + f32::from(ty), f32::from(len), 1.0, c);
            }
            tx += len;
        }
    }
}

// ---- drawing primitives -------------------------------------------------------------

/// The game's colour from 0xAARRGGBB.
pub(super) fn argb(c: u32) -> u32 {
    let [a, r, g, b] = c.to_be_bytes();
    rgba(r, g, b, a)
}

pub(super) fn fill(game: &Game<'_>, batch: &mut HudBatch, r: Rect, color: u32) {
    if r[2] > 0.0 && r[3] > 0.0 {
        batch.fill(&game.gui.atlas, r[0], r[1], r[2], r[3], argb(color));
    }
}

/// One of REI's sprites at its size, `scale` times.
pub(super) fn blit(game: &Game<'_>, batch: &mut HudBatch, s: &Sprite, x: f32, y: f32, scale: f32) {
    let atlas = &game.gui.atlas;
    for &(px, py, len, c) in s.runs {
        batch.fill(atlas, x + f32::from(px) * scale, y + f32::from(py) * scale, f32::from(len) * scale, scale, c);
    }
}

/// Columns `cols` and rows `rows` of a sprite, at `at`.
fn blit_part(game: &Game<'_>, batch: &mut HudBatch, s: &Sprite, at: (f32, f32), cols: (u16, u16), rows: (u16, u16)) {
    let atlas = &game.gui.atlas;
    let w = usize::from(s.width);
    for py in rows.0..rows.1.min(s.height) {
        let mut px = cols.0;
        while px < cols.1.min(s.width) {
            let c = s.pixels[usize::from(py) * w + usize::from(px)];
            let mut len = 1;
            while px + len < cols.1.min(s.width) && s.pixels[usize::from(py) * w + usize::from(px + len)] == c {
                len += 1;
            }
            if c >> 24 != 0 {
                batch.fill(atlas, at.0 + f32::from(px - cols.0), at.1 + f32::from(py - rows.0), f32::from(len), 1.0, c);
            }
            px += len;
        }
    }
}

/// A nine-slice sprite stretched over `r`: its corners as they are, its
/// edges and middle (plain colours in REI's panels) stretched.
pub(super) fn nine(game: &Game<'_>, batch: &mut HudBatch, s: &Sprite, r: Rect) {
    let Some([l, t, rt, b]) = s.border else {
        blit(game, batch, s, r[0], r[1], 1.0);
        return;
    };
    let (w, h) = (s.width, s.height);
    let atlas = &game.gui.atlas;
    let (x, y) = (r[0], r[1]);
    let rw = r[2].max(f32::from(l + rt));
    let rh = r[3].max(f32::from(t + b));
    let px = |cx: u16, cy: u16| s.pixels[usize::from(cy) * usize::from(w) + usize::from(cx)];
    let (mid_w, mid_h) = (rw - f32::from(l + rt), rh - f32::from(t + b));
    blit_part(game, batch, s, (x, y), (0, l), (0, t));
    blit_part(game, batch, s, (x + rw - f32::from(rt), y), (w - rt, w), (0, t));
    blit_part(game, batch, s, (x, y + rh - f32::from(b)), (0, l), (h - b, h));
    blit_part(game, batch, s, (x + rw - f32::from(rt), y + rh - f32::from(b)), (w - rt, w), (h - b, h));
    for row in 0..t {
        put(batch, atlas, [x + f32::from(l), y + f32::from(row), mid_w, 1.0], px(l, row));
    }
    for row in h - b..h {
        put(batch, atlas, [x + f32::from(l), y + rh - f32::from(h - row), mid_w, 1.0], px(l, row));
    }
    for col in 0..l {
        put(batch, atlas, [x + f32::from(col), y + f32::from(t), 1.0, mid_h], px(col, t));
    }
    for col in w - rt..w {
        put(batch, atlas, [x + rw - f32::from(w - col), y + f32::from(t), 1.0, mid_h], px(col, t));
    }
    put(batch, atlas, [x + f32::from(l), y + f32::from(t), mid_w, mid_h], px(l, t));
}

fn put(batch: &mut HudBatch, atlas: &horizon_client::hud::HudAtlas, r: Rect, c: u32) {
    if c >> 24 != 0 && r[2] > 0.0 && r[3] > 0.0 {
        batch.fill(atlas, r[0], r[1], r[2], r[3], c);
    }
}

pub(super) fn text(game: &Game<'_>, batch: &mut HudBatch, s: &str, x: f32, y: f32, color: u32, shadow: bool) {
    batch.text(&game.gui.atlas, s, x.floor(), y.floor(), argb(color), shadow);
}

pub(super) fn width(game: &Game<'_>, s: &str) -> f32 {
    HudBatch::text_width(&game.gui.atlas, s)
}

/// The game's own button (`minecraft:widget/button`), as REI draws every one
/// in its light theme, with an optional centred label.
pub(super) fn button(game: &Game<'_>, batch: &mut HudBatch, r: Rect, enabled: bool, label: &str) {
    let hovered = enabled && inside(game.cursor, r);
    if dark() {
        let s = match (enabled, hovered) {
            (false, _) => &sprites::BUTTON_DISABLED_DARK,
            (true, true) => &sprites::BUTTON_HIGHLIGHTED_DARK,
            (true, false) => &sprites::BUTTON_DARK,
        };
        nine_tiled(game, batch, s, r);
    } else {
        let sprite = match (enabled, hovered) {
            (false, _) => "gui/sprites/widget/button_disabled",
            (true, true) => "gui/sprites/widget/button_highlighted",
            (true, false) => "gui/sprites/widget/button",
        };
        horizon_client::ui::blit_sprite(batch, &game.gui.atlas, sprite, r[0], r[1], r[2], r[3], u32::MAX);
    }
    if !label.is_empty() {
        let color = match (enabled, hovered) {
            (false, _) => 0xffa0_a0a0,
            (true, true) => 0xffff_ffa0,
            (true, false) => 0xffe0_e0e0,
        };
        let w = width(game, label);
        text(game, batch, label, r[0] + (r[2] / 2.0 - w / 2.0).floor(), r[1] + ((r[3] - 8.0) / 2.0).floor(), color, true);
    }
}

/// A 16x16 page button with REI's arrow.
fn arrow_button(game: &Game<'_>, batch: &mut HudBatch, r: Rect, back: bool, enabled: bool) {
    button(game, batch, r, enabled, "");
    let (s, dx, dy) = match (back, r[2] < 16.0) {
        (true, false) => (&sprites::ARROW_LEFT, 4.0, 4.0),
        (false, false) => (&sprites::ARROW_RIGHT, 4.0, 4.0),
        (true, true) => (&sprites::ARROW_LEFT, 2.5, 2.5),
        (false, true) => (&sprites::ARROW_RIGHT, 2.0, 2.5),
    };
    blit(game, batch, s, r[0] + dx, r[1] + dy, 1.0);
}

/// A centred `LabelWidget`: white with a shadow, `LABEL_HOVER` under the
/// pointer when it can be clicked; returns its bounds.
fn label(game: &Game<'_>, batch: &mut HudBatch, s: &str, anchor: (f32, f32), clickable: bool) -> Rect {
    let w = width(game, s);
    let bounds = [anchor.0 - w / 2.0 - 1.0, anchor.1 - 5.0, w + 2.0, 14.0];
    let lit = clickable && inside(game.cursor, bounds);
    let color = match (dark(), lit) {
        (false, false) => WHITE,
        (false, true) => LABEL_HOVER,
        (true, false) => 0xffbb_bbbb,
        (true, true) => WHITE,
    };
    text(game, batch, s, (anchor.0 - w / 2.0).floor(), anchor.1, color, true);
    bounds
}

pub(super) fn item(game: &Game<'_>, batch: &mut HudBatch, stack: &ItemStack, x: f32, y: f32) {
    game.draw_item(batch, stack, x, y);
}

/// An item drawn `scale` times its size from its top left corner.
pub(super) fn item_scaled(game: &Game<'_>, batch: &mut HudBatch, stack: &ItemStack, x: f32, y: f32, scale: f32) {
    let start = batch.vertices.len();
    game.draw_item(batch, stack, x, y);
    batch.scale_since(start, x, y, scale, scale);
}

/// REI's hover: half-white over the 16x16 icon.
fn hover(game: &Game<'_>, batch: &mut HudBatch, x: f32, y: f32) {
    hover_sized(game, batch, x, y, 16.0);
}

fn hover_sized(game: &Game<'_>, batch: &mut HudBatch, x: f32, y: f32, size: f32) {
    fill(game, batch, [x, y, size, size], HOVER);
    if dark() {
        fill(game, batch, [x, y, size, size], 0x5a11_1111);
    }
}

/// An entry's cell (`entrySize`): `ceil(18 * zoom)` pixels.
fn entry_size(settings: rei::Settings) -> f32 {
    (18.0 * settings.zoom).ceil()
}

/// An item in a cell `e` wide whose icon starts at (x, y) (`getInnerBounds`).
fn cell_item(game: &Game<'_>, batch: &mut HudBatch, stack: &ItemStack, x: f32, y: f32, e: f32) {
    if e == E {
        item(game, batch, stack, x, y);
    } else {
        item_scaled(game, batch, stack, x, y, (e - 2.0) / 16.0);
    }
}

/// A recipe's background by `appearance.recipe_border` (`createRecipeBase`).
fn recipe_base(game: &Game<'_>, batch: &mut HudBatch, border: u8, r: Rect) {
    match border {
        1 => nine(game, batch, th(&sprites::PANEL_LIGHTER, &sprites::PANEL_LIGHTER_DARK), r),
        2 => {}
        _ => nine(game, batch, th(&sprites::PANEL, &sprites::PANEL_DARK), r),
    }
}

/// The recipe screen's tab size and tabs a page (`TabContainerWidget`):
/// 24 or, with Larger Tabs, 28; as many as fit, an odd number.
fn tab_metrics(s: &State) -> (f32, usize) {
    let larger = s.settings.is_some_and(|x| x.larger_tabs);
    (if larger { 28.0 } else { 24.0 }, rei::tabs_per_page(larger))
}

/// A label in REI's rainbow (`TextTransformations.applyRainbow`).
fn rainbow_label(game: &Game<'_>, batch: &mut HudBatch, s: &str, anchor: (f32, f32)) -> Rect {
    let w = width(game, s);
    let x0 = (anchor.0 - w / 2.0).floor();
    let millis = START.with(|t| t.elapsed().as_millis() as i64);
    let mut cx = 0.0;
    for ch in s.chars() {
        let one = ch.to_string();
        let hue = (millis - (cx * 10.0) as i64 - (anchor.1 * 10.0) as i64).rem_euclid(2000) as f32 / 2000.0;
        batch.text(&game.gui.atlas, &one, x0 + cx, anchor.1, argb(hsb(hue, 0.8, 0.95)), true);
        cx += width(game, &one);
    }
    [anchor.0 - w / 2.0 - 1.0, anchor.1 - 5.0, w + 2.0, 14.0]
}

thread_local! {
    static START: Instant = Instant::now();
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

/// REI's vanilla-screen click areas (`DefaultClientPlugin.registerScreens`):
/// the open screen's arrow, relative to its window, and the category it shows.
fn click_area(game: &Game<'_>) -> Option<(Rect, Category)> {
    if game.screen != Screen::Inventory || game.station.is_some() {
        return None;
    }
    match game.container {
        None if game.craft_size == 2 => Some(([137.0, 29.0, 10.0, 13.0], Category::Crafting)),
        None => Some(([88.0, 32.0, 28.0, 23.0], Category::Crafting)),
        Some((x, y, z)) => {
            let name = game.res.facts().get(game.world.block(x, y, z)).name.clone();
            match &*name {
                "minecraft:brewing_stand" => Some(([97.0, 16.0, 14.0, 30.0], Category::Brewing)),
                "minecraft:furnace" => Some(([78.0, 32.0, 28.0, 23.0], Category::Smelting)),
                "minecraft:smoker" => Some(([78.0, 32.0, 28.0, 23.0], Category::Smoking)),
                "minecraft:blast_furnace" => Some(([78.0, 32.0, 28.0, 23.0], Category::Blasting)),
                _ => None,
            }
        }
    }
}

fn intersects(a: Rect, b: Rect) -> bool {
    a[0] < b[0] + b[2] && b[0] < a[0] + a[2] && a[1] < b[1] + b[3] && b[1] < a[1] + a[3]
}

/// Whether a cell at slot point (x, y) steps on an exclusion zone: any of
/// its corners inside one (`EntryListWidget.notSteppingOnExclusionZones`).
fn steps_on(x: f32, y: f32, e: f32, zones: &[Rect]) -> bool {
    let (x0, y0) = (x - 1.0, y - 1.0);
    let corners = [(x0, y0), (x0 + e, y0), (x0, y0 + e), (x0 + e, y0 + e)];
    zones.iter().any(|z| corners.iter().any(|c| inside(*c, *z)))
}

// ---- where things go ----------------------------------------------------------------

/// The container screen's window.
fn container_rect(game: &Game<'_>, batch: &HudBatch) -> Rect {
    let image = game.container_image(batch);
    [image.left as f32, image.top as f32, image.width as f32, image.height as f32]
}

/// The default plugin's exclusion zones: the recipe book left of the
/// container and the inventory's effects right of it.
fn exclusions(game: &Game<'_>, batch: &HudBatch, s: Rect) -> Vec<Rect> {
    let mut out = Vec::new();
    if game.recipe_book_shift(batch.width) > 0.0 {
        out.push([s[0] - 149.0, s[1], 179.0, s[3]]);
    }
    let shows = match game.screen {
        Screen::Inventory => game.container.is_none() && game.craft_size == 2 && game.station.is_none(),
        Screen::Creative => true,
        _ => false,
    };
    let n = game.potions.effects.list.len();
    let x = s[0] + s[2] + 2.0;
    let avail = batch.width - x;
    if shows && n > 0 && avail >= 32.0 {
        let step = if n > 5 { 132.0 / (n - 1) as f32 } else { 33.0 };
        let w = if avail >= 120.0 { 120.0 } else { 32.0 };
        for i in 0..n {
            out.push([x, s[1] + i as f32 * step, w, 32.0]);
        }
    }
    out
}

/// What to show over everything at the end of the frame.
struct Tip {
    stack: Option<ItemStack>,
    /// Lines under the stack's name, or the whole tooltip without a stack.
    lines: Vec<(String, u32)>,
    /// A picture under the lines (REI's tooltip components).
    component: Option<Component>,
    /// Already drawn this frame.
    drawn: bool,
}

/// REI's tooltip components: a grid of entries with a title and maybe a tag
/// line ("Accepts following:", `MissingStacksTooltip`), or a whole display
/// (`DisplayTooltipComponent`).
enum Component {
    Grid { title: String, cells: Vec<ItemStack>, tag: Option<String> },
    Display(usize),
}

impl Tip {
    fn text(lines: Vec<(String, u32)>) -> Tip {
        Tip { stack: None, lines, component: None, drawn: false }
    }
    fn stack(stack: ItemStack, lines: Vec<(String, u32)>) -> Tip {
        Tip { stack: Some(stack), lines, component: None, drawn: false }
    }
    fn with(mut self, component: Component) -> Tip {
        self.component = Some(component);
        self
    }
}

// ---- the whole frame --------------------------------------------------------------------

/// Draws the overlay over the container screen in place of the game's item
/// panel (`draw_mod_browser`): true when the mod is on.
pub(super) fn draw(game: &mut Game<'_>, batch: &mut HudBatch) -> bool {
    if !glue::on(game) {
        return false;
    }
    let tip = glue::with(|s| frame(game, batch, s)).flatten();
    let cursor = game.cursor;
    let covered = glue::COVERED.with(std::cell::Cell::get);
    if game.carried.is_none() {
        match tip {
            Some(Tip { drawn: true, .. }) => {}
            Some(Tip { stack: Some(stack), lines, .. }) => {
                let extra: Vec<(Text, u32)> = lines.into_iter().map(|(l, c)| (Text::legacy(&l), argb(c))).collect();
                OWN_TOOLTIP.with(|c| c.set(true));
                game.draw_stack_tooltip_with(batch, &stack, false, cursor.0, cursor.1, extra);
                OWN_TOOLTIP.with(|c| c.set(false));
            }
            Some(Tip { stack: None, lines, .. }) if !lines.is_empty() => {
                let lines: Vec<(String, u32)> = lines.into_iter().map(|(l, c)| (l, argb(c))).collect();
                game.draw_tooltip_lines(batch, &lines, cursor.0, cursor.1);
            }
            _ if !covered => game.draw_slot_tooltip(batch),
            _ => {}
        }
    } else {
        game.draw_carried(batch);
    }
    true
}

/// The recipe window's place (`DefaultDisplayViewingScreen.init`).
struct Window {
    bounds: Rect,
    category: Category,
    per_page: usize,
    pages: usize,
    /// The workstations' panel left of the window, when it has any.
    panel: Option<Rect>,
}

fn window(game: &Game<'_>, batch: &HudBatch, s: &State) -> Option<Window> {
    let category = s.recipes.category()?;
    let data = s.data.as_ref()?;
    let (w, h) = (batch.width, batch.height);
    // `OUTER_PADDING_TOP + tabSize - 2`, 16 more for the tab pages' buttons.
    let (ts, tpp) = tab_metrics(s);
    let paged = s.recipes.tabs.len() > tpp;
    let top = ts + if paged { 16.0 } else { 0.0 };
    let bottom = 24.0;
    let counts: Vec<(Category, usize)> = s.recipes.tabs.iter().map(|(c, l)| (*c, l.len())).collect();
    let height = rei::window_height(h - top - bottom, &counts).floor();
    let x = (w / 2.0).floor() - GUI_W / 2.0;
    let y = top + ((h - top - bottom) / 2.0).floor() - (height / 2.0).floor();
    let bounds = [x, y, GUI_W, height];
    let per_page = rei::per_page(height, category);
    let pages = rei::pages(s.recipes.recipes().len(), per_page);
    let n = data.index.workstations(category).len();
    let panel = (n > 0).then(|| {
        let hh = (((height - 16.0) / 18.0).floor() as usize).max(1);
        let rows = hh.min(n);
        let cols = n.div_ceil(hh);
        let xx = x - (8.0 + cols as f32 * 16.0) + 6.0;
        let yy = y + 16.0;
        [xx - 5.0, yy - 5.0, 15.0 + cols as f32 * 16.0, 10.0 + rows as f32 * 16.0]
    });
    let _ = game;
    Some(Window { bounds, category, per_page, pages, panel })
}

fn frame(game: &mut Game<'_>, batch: &mut HudBatch, s: &mut State) -> Option<Tip> {
    let settings = glue::settings(game, s);
    DARK.with(|d| d.set(settings.dark));
    // REI's config screen stands in for the container screen while it is open.
    if s.config.is_some() {
        // The container stays out of sight under it.
        let atlas = &game.gui.atlas;
        batch.fill(atlas, 0.0, 0.0, batch.width, batch.height, argb(0x8000_0000));
        s.hits.clear();
        let game: &Game<'_> = game;
        return super::roughlyenoughitems_config::draw(game, batch, s).map(Tip::text);
    }
    s.hits.clear();
    s.favorites_grid = None;
    s.list_area = None;
    let millis = s.started.get_or_insert_with(Instant::now).elapsed().as_millis() as u64;
    s.data.as_ref()?;
    let game: &Game<'_> = game;
    let cursor = game.cursor;
    glue::drag_moved(s, cursor);
    let mut tip = None;
    let container = container_rect(game, batch);
    let mut zones = exclusions(game, batch, container);
    let mut screen = container;
    let win = if s.recipes.is_open() { window(game, batch, s) } else { None };
    if s.recipes.is_open() {
        let atlas = &game.gui.atlas;
        batch.fill_gradient(atlas, (0.0, 0.0, batch.width, batch.height), argb(0xc010_1010), argb(0xd010_1010));
        zones.clear();
        if let Some(w) = &win {
            screen = w.bounds;
            zones.extend(w.panel);
            s.per_page = w.per_page;
            s.recipes.page = s.recipes.page.min(w.pages - 1);
        }
    }
    let mut menu = None;
    if settings.overlay {
        let (t, m) = overlay(game, batch, s, screen, &zones, settings, win.is_some());
        tip = t;
        menu = m;
    }
    if let Some(w) = &win
        && let Some(t) = recipe_screen(game, batch, s, w, millis)
    {
        tip = Some(t);
    }
    if let Some((kind, owner)) = menu {
        draw_menu(game, batch, s, kind, owner, settings);
        tip = None;
    }
    if s.choose.is_some() {
        choose_popup(game, batch, s);
        tip = None;
    }
    // The dragged entry follows the pointer.
    if let Some(press) = s.press
        && press.dragging
    {
        tip = None;
        dragged(game, batch, s, press.hit, cursor);
    }
    if let Some(t) = &tip
        && t.component.is_some()
        && game.carried.is_none()
    {
        component_tooltip(game, batch, s, t, millis);
        tip = Some(Tip { stack: None, lines: Vec::new(), component: None, drawn: true });
    }
    if let Some((note, at)) = &s.note
        && at.elapsed().as_secs() < 3
    {
        let w = width(game, note);
        let x = ((batch.width - w) / 2.0).floor();
        fill(game, batch, [x - 3.0, 3.0, w + 6.0, 12.0], 0xc000_0000);
        text(game, batch, note, x, 5.0, RED, true);
    }
    tip
}

/// The choose-page popup (`DefaultDisplayChoosePageWidget`) in the middle of
/// the darkened screen: "Choose Page", the number field with " /N", Done.
fn choose_popup(game: &Game<'_>, batch: &mut HudBatch, s: &mut State) {
    let ru = s.ru;
    let Some(c) = s.choose.as_mut() else {
        return;
    };
    let atlas = &game.gui.atlas;
    batch.fill_gradient(atlas, (0.0, 0.0, batch.width, batch.height), argb(0xc010_1010), argb(0xd010_1010));
    let (mx, my) = ((batch.width / 2.0).floor(), (batch.height / 2.0).floor());
    let top = [mx - 50.0, my - 20.0, 100.0, 40.0];
    let foot = [mx, my + 14.0, 50.0, 36.0];
    nine(game, batch, th(&sprites::PANEL, &sprites::PANEL_DARK), top);
    nine(game, batch, th(&sprites::PANEL, &sprites::PANEL_DARK), foot);
    text(game, batch, rei::tr(ru, "choose_page"), top[0] + 5.0, top[1] + 5.0, if dark() { 0xffbb_bbbb } else { DARK_TEXT }, false);
    let of = format!(" /{}", c.pages);
    let ow = width(game, &of);
    text(game, batch, &of, top[0] + top[2] - 5.0 - ow, top[1] + 22.0, if dark() { 0xffbb_bbbb } else { DARK_TEXT }, false);
    // The number field (`TextFieldWidget`).
    let f = [top[0] + 7.0, top[1] + 16.0, 100.0 - ow - 12.0, 18.0];
    let lit = c.field.focused || inside(game.cursor, f);
    fill(game, batch, [f[0] - 1.0, f[1] - 1.0, f[2] + 2.0, f[3] + 2.0], 0xff00_0000);
    fill(game, batch, f, if lit { WHITE } else { 0xffa0_a0a0 });
    fill(game, batch, [f[0] + 1.0, f[1] + 1.0, f[2] - 2.0, f[3] - 2.0], 0xff00_0000);
    let value = c.field.value.clone();
    text(game, batch, &value, f[0] + 4.0, f[1] + 5.0, 0xffe0_e0e0, true);
    if c.field.focused && c.field.cursor_visible() {
        let before: String = value.chars().take(c.field.cursor).collect();
        let cx = f[0] + 4.0 + width(game, &before);
        fill(game, batch, [cx, f[1] + 4.0, 1.0, 9.0], 0xffe0_e0e0);
    }
    let done = [top[0] + 55.0, top[1] + 43.0, 40.0, 20.0];
    let label = game.translate("gui.done", "Done");
    button(game, batch, done, true, &label);
    s.hits.push((top, Hit::ChooseArea));
    s.hits.push((foot, Hit::ChooseArea));
    s.hits.push((f, Hit::ChooseField));
    s.hits.push((done, Hit::ChooseDone));
}

/// The first item tag holding exactly these entries ("Tag: #minecraft:planks").
fn exact_tag(data: &Data, entries: &[Stack], ru: bool) -> Option<String> {
    let ids: std::collections::HashSet<u32> = entries.iter().map(|s| s.id).collect();
    data.tags[..data.item_tags.min(data.tags.len())]
        .iter()
        .find(|t| t.all.len() == ids.len() && t.all.iter().all(|e| ids.contains(e)))
        .map(|t| format!("{}{}", rei::tr(ru, "tag_accept"), t.id))
}

/// A component's size (`ClientTooltipComponent.getWidth/getHeight`).
fn component_size(game: &Game<'_>, data: &Data, c: &Component) -> (f32, f32) {
    match c {
        Component::Grid { title, cells, tag } => {
            let n = cells.len();
            let cols = n.clamp(1, 11);
            let rows = n.div_ceil(11).clamp(1, 6);
            let mut w = (cols as f32 * E).max(width(game, title));
            if let Some(t) = tag {
                w = w.max(width(game, t));
            }
            (w, rows as f32 * E + 2.0 + 12.0 + if tag.is_some() { 12.0 } else { 0.0 })
        }
        Component::Display(r) => {
            let h = data.index.recipes.get(*r).map_or(66.0, |r| r.category().height());
            (DISPLAY_W + 4.0, h + 4.0)
        }
    }
}

/// A tooltip with a component under its lines, as the game draws item
/// tooltips: the box, the lines, then the component.
fn component_tooltip(game: &Game<'_>, batch: &mut HudBatch, s: &State, tip: &Tip, millis: u64) {
    let Some(data) = s.data.as_ref() else {
        return;
    };
    let Some(component) = &tip.component else {
        return;
    };
    let atlas = &game.gui.atlas;
    let mut lines: Vec<(Text, u32)> = Vec::new();
    if let Some(stack) = &tip.stack {
        OWN_TOOLTIP.with(|c| c.set(true));
        lines = game.stack_tooltip(stack);
        OWN_TOOLTIP.with(|c| c.set(false));
    }
    lines.extend(tip.lines.iter().map(|(l, c)| (Text::legacy(l), argb(*c))));
    if lines.is_empty() {
        return;
    }
    let (cw, ch) = component_size(game, data, component);
    let (x, y, w, h) = Game::tooltip_geometry(atlas, &lines, game.cursor, (batch.width, batch.height), [cw, ch + 2.0]);
    game.tooltip_box(batch, tip.stack.as_ref(), x, y, w, h);
    Game::tooltip_text(batch, atlas, &lines, x, y);
    let cy = y + h - ch;
    match component {
        Component::Grid { title, cells, tag } => {
            text(game, batch, title, x, cy + 2.0, GRAY, true);
            let n = cells.len();
            let rows = n.div_ceil(11).clamp(1, 6);
            for (k, st) in cells.iter().enumerate() {
                let (gx, gy) = (x + (k % 11) as f32 * E, cy + 13.0 + (k / 11) as f32 * E);
                if k >= 11 * 6 - 1 {
                    let more = format!("+{}", n - 11 * 6 + 1);
                    let mw = width(game, &more);
                    text(game, batch, &more, gx + 9.0 - (mw / 2.0).floor(), gy + 8.0, GRAY, true);
                    break;
                }
                item_scaled(game, batch, st, gx, gy, E / 16.0);
            }
            if let Some(t) = tag {
                text(game, batch, t, x, cy + 16.0 + rows as f32 * E, GRAY, true);
            }
        }
        Component::Display(r) => {
            if let Some(recipe) = data.index.recipes.get(*r) {
                let ctx = Context { game, data, ru: s.ru, millis, focus: None, cursor: (-1000.0, -1000.0), reduced_motion: true, border: s.settings.map_or(0, |x| x.recipe_border) };
                let _ = display(&ctx, batch, recipe.category(), recipe, x + 2.0, cy + 2.0);
            }
        }
    }
}

/// The entry being dragged, under the pointer.
fn dragged(game: &Game<'_>, batch: &mut HudBatch, s: &mut State, hit: Hit, cursor: (f32, f32)) {
    let (x, y) = (cursor.0 - 8.0, cursor.1 - 8.0);
    let ru = s.ru;
    if let Hit::Shortcut(sc) = hit {
        shortcut(game, batch, sc, x - 1.0, y - 1.0, ru);
        return;
    }
    if let Hit::Favorite(i) = hit
        && let Some(sc) = glue::favorites(s).keys.get(i).and_then(|k| Shortcut::from_key(k))
    {
        shortcut(game, batch, sc, x - 1.0, y - 1.0, ru);
        return;
    }
    let id = match hit {
        Hit::Entry(id) | Hit::Slot(id) | Hit::Workstation(id) => Some(id),
        Hit::Favorite(i) => {
            let key = glue::favorites(s).keys.get(i).cloned();
            key.and_then(|k| s.data.as_ref()?.ingredient_by_key(&k))
        }
        Hit::Collapsed(g) => {
            let ids = glue::filtered(game, s);
            glue::cells(s, &ids).into_iter().find_map(|c| match c {
                ListCell::Group(x, list) if x == g => list.first().copied(),
                _ => None,
            })
        }
        _ => None,
    };
    if let Some(stack) = id.and_then(|id| s.data.as_ref()?.ingredient(id)).map(|i| i.stack.clone()) {
        item(game, batch, &stack, x, y);
    }
}

// ---- the overlay (`ScreenOverlayImpl`) ------------------------------------------------

/// The entry list, the search row and the favorites; returns the tooltip
/// and the hover menu to draw last.
fn overlay(
    game: &Game<'_>,
    batch: &mut HudBatch,
    s: &mut State,
    screen: Rect,
    zones: &[Rect],
    settings: rei::Settings,
    covered: bool,
) -> (Option<Tip>, Option<(Menu, Rect)>) {
    let (w, h) = (batch.width, batch.height);
    let mut tip = None;
    // The entry list's region beside the screen (`getOverlayBounds`), right
    // or, with "Left Side Panel", left; at most 50 entries wide.
    let left = settings.left_panel;
    let e = entry_size(settings);
    let o: Option<Rect> = side_region(screen, w, h, left).map(|mut o| {
        let max_w = (e * 50.0 + e * 0.75).ceil();
        if o[2] > max_w {
            if !left {
                o[0] += o[2] - max_w;
            }
            o[2] = max_w;
        }
        o
    });
    // The search field: where it is set, but under the entry list when the
    // screen reaches the bottom (`REIRuntimeImpl.getContextualSearchFieldLocation`).
    let position = match settings.search_position {
        0 if h - 20.0 <= screen[1] + screen[3] => 1,
        p => p,
    };
    if let Some(o) = o
        && let Some(t) = entry_list(game, batch, s, o, position, settings, zones, covered)
    {
        tip = Some(t);
    }
    // The field makes room for the buttons beside it (`REIRuntimeImpl`).
    let buttons = 1.0 + if settings.craftable_button { 22.0 } else { 0.0 } + if settings.config_upper { 0.0 } else { 22.0 };
    let field: Option<Rect> = match position {
        1 => o.map(|o| [o[0] + 2.0, h - 22.0, o[2] - 6.0 - buttons, 18.0]),
        2 => o.map(|o| [o[0] + 2.0, 4.0, o[2] - 6.0 - buttons, 18.0]),
        _ => Some([screen[0], h - 22.0, screen[2] - buttons, 18.0]),
    };
    let mut menu = None;
    if let Some(f) = field.filter(|f| f[2] > 8.0) {
        if let Some(t) = search_field(game, batch, s, f, settings) {
            tip = Some(t);
        }
        let mut x = f[0] + f[2] + 4.0;
        if settings.craftable_button {
            let r = [x, f[1] - 1.0, 20.0, 20.0];
            button(game, batch, r, true, "");
            let tint = if s.craftable { 0x3800_d907 } else { 0x38ff_0000 };
            fill(game, batch, [r[0] + 1.0, r[1] + 1.0, 18.0, 18.0], tint);
            s.hits.push((r, Hit::Craftable));
            if inside(game.cursor, r) {
                let key = if s.craftable { "showing_craftable" } else { "showing_all" };
                tip = Some(Tip::text(vec![(rei::tr(s.ru, key).to_owned(), WHITE)]));
            }
            menu = hover_menu(s, Menu::Craftable, r, game.cursor, menu);
            x += 22.0;
        }
        if !settings.config_upper {
            let r = [x, f[1] - 1.0, 20.0, 20.0];
            config_button(game, batch, s, r, settings, covered);
            menu = hover_menu(s, Menu::Config, r, game.cursor, menu);
        }
    }
    // The config button in the top corner (`ConfigButtonPosition.UPPER`).
    if settings.config_upper {
        let r = [if left { w - 30.0 } else { 10.0 }, 10.0, 20.0, 20.0];
        config_button(game, batch, s, r, settings, covered);
        menu = hover_menu(s, Menu::Config, r, game.cursor, menu);
    }
    if settings.favorites
        && let Some(t) = favorites(game, batch, s, screen, zones, covered, settings)
    {
        tip = Some(t);
    }
    // The screen's own recipe arrow (`accessibility.clickable_recipe_arrows`).
    if settings.clickable_arrows
        && !covered
        && let Some((area, category)) = click_area(game)
    {
        let r = [screen[0] + area[0], screen[1] + area[1], area[2], area[3]];
        s.hits.push((r, Hit::ClickArea(category)));
        if inside(game.cursor, r) && game.carried.is_none() {
            let name = title(game, s.ru, category);
            tip = Some(Tip::text(vec![(rei::tr(s.ru, "view_recipes_for").replace("%s", &name), WHITE)]));
        }
    }
    if menu.is_none() {
        s.menu = None;
        s.menu_rects.clear();
    }
    (tip, menu)
}

/// The gear (`ConfigButtonWidget`): tinted while cheating.
fn config_button(game: &Game<'_>, batch: &mut HudBatch, s: &mut State, r: Rect, settings: rei::Settings, covered: bool) {
    button(game, batch, r, true, "");
    if settings.cheating(game.creative) && !covered {
        let tint = if game.creative { 0x2aff_0000 } else { 0x5800_afff };
        fill(game, batch, [r[0] + 1.0, r[1] + 1.0, 18.0, 18.0], tint);
    }
    blit(game, batch, &sprites::GEAR, r[0] + 3.0, r[1] + 3.0, 1.0);
    s.hits.push((r, Hit::Config));
}

/// The hover menu of `kind` over button `owner`: open while the pointer is
/// on the button or on the menu (`MenuAccess.openOrClose`).
fn hover_menu(s: &mut State, kind: Menu, owner: Rect, cursor: (f32, f32), current: Option<(Menu, Rect)>) -> Option<(Menu, Rect)> {
    if current.is_some() {
        return current;
    }
    let over_menu = s.menu == Some(kind) && s.menu_rects.iter().any(|(_, m)| inside(cursor, *m));
    if inside(cursor, owner) || over_menu {
        if s.menu != Some(kind) {
            s.menu_at = Some(Instant::now());
            s.menu_focus = [None; 3];
            s.sub_at = None;
        }
        s.menu = Some(kind);
        return Some((kind, owner));
    }
    None
}

// ---- the entry list (`PaginatedEntryListWidget`) ----------------------------------------

#[allow(clippy::too_many_arguments)]
fn entry_list(
    game: &Game<'_>,
    batch: &mut HudBatch,
    s: &mut State,
    o: Rect,
    position: u8,
    settings: rei::Settings,
    zones: &[Rect],
    covered: bool,
) -> Option<Tip> {
    let ru = s.ru;
    let cursor = game.cursor;
    let paged = !settings.scrolling;
    let e = entry_size(settings);
    let inner_size = e - 2.0;
    // The list's area (`REIRuntimeImpl.calculateEntryListArea`): under the
    // page row, and the search field when it sits over or under the list.
    let top = if position == 2 { 24.0 } else { 0.0 };
    let y_off = 2.0 + top + if paged { 22.0 } else { 0.0 };
    let h_off = if position == 1 { 24.0 } else { 0.0 };
    let a = [o[0], o[1] + y_off, o[2], o[3] - 1.0 - y_off - h_off];
    let cx = (a[0] + (a[2] / 2.0).floor()).floor();
    let cy = (a[1] + (a[3] / 2.0).floor()).floor();
    s.list_area = Some(o);
    let ids = glue::filtered(game, s);
    if s.search_changed() {
        s.list_page = 0;
        s.list_scroll = 0.0;
    }
    let cells = glue::cells(s, &ids);
    s.list_len = cells.len();
    let mut tip = None;
    let (grid, points, skip) = if paged {
        // `PaginatedEntryListWidget`: as many rows and columns as fit, centred.
        let cols = (((a[2] - 2.0) / e).floor() as usize).max(1);
        let rows = (((a[3] - 2.0) / e).floor() as usize).max(1);
        let grid = [(cx - cols as f32 * e / 2.0).floor(), (cy - rows as f32 * e / 2.0).floor(), cols as f32 * e, rows as f32 * e];
        let mut points = Vec::new();
        for r in 0..rows {
            for c in 0..cols {
                let (x, y) = (grid[0] + c as f32 * e, grid[1] + r as f32 * e);
                if !steps_on(x, y, e, zones) {
                    points.push((x, y));
                }
            }
        }
        let per = points.len().max(1);
        let pages = rei::pages(cells.len(), per);
        s.list_per = per;
        s.list_page = s.list_page.min(pages - 1);
        (grid, points, s.list_page * per)
    } else {
        // `ScrollingEntryListWidget`: the full height, 3 px aside for the bar,
        // every entry in rows that scroll.
        let cols = (((a[2] - 8.0) / e).floor() as usize).max(1);
        let shift = if settings.left_panel { 3.0 } else { -3.0 };
        let grid = [(cx - cols as f32 * e / 2.0 + shift).floor(), a[1], cols as f32 * e, a[3]];
        let mut points = Vec::new();
        let mut r = 0;
        while points.len() < cells.len() && r < cells.len() + 64 {
            for c in 0..cols {
                let (x, y) = (grid[0] + c as f32 * e, grid[1] + r as f32 * e);
                if points.len() < cells.len() && !steps_on(x, y - s.list_scroll, e, zones) {
                    points.push((x, y));
                }
            }
            r += 1;
        }
        let content = points.last().map_or(0.0, |p| p.1 + e - grid[1]);
        s.list_scroll = s.list_scroll.clamp(0.0, (content - grid[3]).max(0.0));
        let scroll = s.list_scroll;
        let points: Vec<(f32, f32)> = points.into_iter().map(|(x, y)| (x, y - scroll)).collect();
        s.list_per = cells.len().max(1);
        s.list_page = 0;
        // The scroll bar (cloth `ScrollingContainer.renderScrollBar`).
        if content > grid[3] {
            let bar_x = if settings.left_panel { grid[0] - 7.0 } else { grid[0] + grid[2] + 1.0 };
            let thumb_h = (grid[3] * grid[3] / content).clamp(32.0, grid[3]);
            let thumb_y = grid[1] + scroll * (grid[3] - thumb_h) / (content - grid[3]);
            let lit = inside(cursor, [bar_x, grid[1], 6.0, grid[3]]);
            fill(game, batch, [bar_x, thumb_y, 6.0, thumb_h], if lit { 0xffaa_aaaa } else { 0xff7f_7f7f });
            fill(game, batch, [bar_x, thumb_y, 5.0, thumb_h - 1.0], if lit { 0xffdd_dddd } else { 0xffaa_aaaa });
        }
        (grid, points, 0)
    };
    let per = s.list_per;
    let mut hits: Vec<(Rect, Hit)> = vec![(grid, Hit::Area)];

    if paged {
        // The page row.
        let pages = rei::pages(cells.len(), per);
        let back = [o[0], o[1] + top + 5.0, 16.0, 16.0];
        let next = [o[0] + o[2] - 18.0, o[1] + top + 5.0, 16.0, 16.0];
        arrow_button(game, batch, back, true, pages > 1);
        arrow_button(game, batch, next, false, pages > 1);
        let page_text = format!("{}/{}", s.list_page + 1, pages);
        let anchor = ((o[0] + (o[2] / 2.0).floor()).floor(), o[1] + top + 10.0);
        let bounds = if settings.rainbow { rainbow_label(game, batch, &page_text, anchor) } else { label(game, batch, &page_text, anchor, pages > 1) };
        hits.push((back, Hit::ListPage(-1)));
        hits.push((next, Hit::ListPage(1)));
        hits.push((bounds, Hit::ListLabel));
        if inside(cursor, back) || inside(cursor, next) {
            let key = if inside(cursor, back) { "previous_page" } else { "next_page" };
            tip = Some(Tip::text(vec![(rei::tr(ru, key).to_owned(), WHITE)]));
        } else if inside(cursor, bounds) {
            tip = Some(Tip::text(vec![
                (rei::tr(ru, "go_back_first_page").to_owned(), WHITE),
                (" ".to_owned(), WHITE),
                (rei::tr(ru, "shift_click_choose").to_owned(), GRAY),
            ]));
        }
    }
    let clip_start = batch.vertices.len();

    // The entry under the pointer, and how long it has been there.
    let under = points
        .iter()
        .zip(cells.iter().skip(skip))
        .find(|(p, _)| inside(cursor, [p.0, p.1, e - 1.0, e - 1.0]) && inside(cursor, grid))
        .and_then(|(_, c)| match c {
            ListCell::One(id) => Some(*id),
            ListCell::Group(..) => None,
        });
    match (under, s.hover_since) {
        (Some(id), Some((was, _))) if id == was => {}
        (Some(id), _) => s.hover_since = Some((id, Instant::now())),
        (None, _) => s.hover_since = None,
    }
    let waited = s.hover_since.is_some_and(|(_, at)| at.elapsed().as_millis() > 200);
    // Focus mode Zoomed: the entry under the pointer grows to 1.5 times over 200 ms.
    let focus_scale = if settings.focus_zoomed {
        let t = if settings.reduced_motion { 1.0 } else { s.hover_since.map_or(0.0, |(_, at)| (at.elapsed().as_millis() as f32 / 200.0).min(1.0)) };
        1.0 + 0.5 * t
    } else {
        1.0
    };
    let favorites_hint = settings.favorites_hint && settings.favorites;
    let favorite_key = glue::favorite_key_name(game);
    let expanded = s.expanded;
    let data = s.data.as_ref()?;
    let craftable: Option<&std::collections::HashSet<u32>> = s.inventory.as_ref().map(|(_, i)| &i.craftable);
    let shown: Vec<(&(f32, f32), &ListCell)> = points.iter().zip(cells.iter().skip(skip)).filter(|(p, _)| p.1 + e > grid[1] && p.1 < grid[1] + grid[3]).collect();
    // Expanded groups: each member's cell, for their outline.
    let mut members: Vec<(f32, f32, u8)> = Vec::new();
    for &(&(x, y), cell) in &shown {
        let hovered = inside(cursor, [x, y, e - 1.0, e - 1.0]) && inside(cursor, grid);
        match cell {
            ListCell::One(id) => {
                let Some(ingredient) = data.ingredient(*id) else {
                    continue;
                };
                // A group's only entry stays a collapsed cell; an unfolded
                // group's entries are lighter and outlined.
                match ingredient.group {
                    Some(g) if expanded[g as usize] => {
                        fill(game, batch, [x - 1.0, y - 1.0, e, e], 0x34ff_ffff);
                        members.push((x, y, g));
                    }
                    Some(_) => fill(game, batch, [x - 1.0, y - 1.0, e, e], 0x53ff_ffff),
                    None => {}
                }
                if hovered && settings.focus_zoomed {
                    let size = inner_size * focus_scale;
                    let (ix, iy) = (x + inner_size / 2.0 - size / 2.0, y + inner_size / 2.0 - size / 2.0);
                    item_scaled(game, batch, &ingredient.stack, ix, iy, size / 16.0);
                } else {
                    cell_item(game, batch, &ingredient.stack, x, y, e);
                }
                if hovered {
                    if !settings.focus_zoomed {
                        hover_sized(game, batch, x, y, inner_size);
                    }
                    let mut lines = Vec::new();
                    // A recipe this screen can make now: "Ctrl-Click to Quick
                    // Craft" and the recipe itself (`DisplayTooltipComponent`).
                    let preview = (waited && !covered && craftable.is_some_and(|c| c.contains(id)))
                        .then(|| {
                            data.index
                                .lookup(Lookup::Recipes(*id))
                                .into_iter()
                                .find(|(c, _)| *c == Category::Crafting)?
                                .1
                                .into_iter()
                                .find(|&i| glue::status_now(s, &data.index.recipes[i]) == Status::Ok)
                        })
                        .flatten();
                    if preview.is_some() {
                        lines.push((rei::tr(ru, "move_items_tooltip").to_owned(), YELLOW));
                    }
                    if let Some(g) = ingredient.group {
                        let n = ids.iter().filter(|i| data.ingredient(**i).and_then(|x| x.group) == Some(g)).count();
                        let name = &data.group_names[g as usize];
                        let hint = if expanded[g as usize] { "collapsed_collapse" } else { "collapsed_expand" };
                        lines.push((format!("§7§o{}{name} ({n} {})", rei::tr(ru, hint), rei::tr(ru, "entries")), GRAY));
                    }
                    if favorites_hint {
                        lines.push((" ".to_owned(), GRAY));
                        lines.push((format!("§7{}", rei::tr(ru, "favorites_tooltip").replace("%s", &favorite_key)), GRAY));
                    }
                    let t = Tip::stack(ingredient.stack.clone(), lines);
                    tip = Some(match preview {
                        Some(r) => t.with(Component::Display(r)),
                        None => t,
                    });
                }
                hits.push(([x - 1.0, y - 1.0, e, e], Hit::Entry(*id)));
            }
            ListCell::Group(g, list) => {
                fill(game, batch, [x - 1.0, y - 1.0, e, e], 0x53ff_ffff);
                let stacks: Vec<ItemStack> = list.iter().take(2).filter_map(|id| data.ingredient(*id)).map(|i| i.stack.clone()).collect();
                // REI's two stacked icons, the second behind.
                let k = inner_size / 16.0;
                if let Some(second) = stacks.get(1) {
                    item_scaled(game, batch, second, x + 0.98 * k, y - 1.18 * k, 1.0125 * k);
                }
                if let Some(first) = stacks.first() {
                    item_scaled(game, batch, first, x - 1.18 * k, y + 0.98 * k, 1.0125 * k);
                }
                if hovered {
                    hover_sized(game, batch, x, y, inner_size);
                    let name = &data.group_names[*g as usize];
                    tip = Some(Tip::text(vec![
                        (format!("{}{name}", rei::tr(ru, "collapsed")), WHITE),
                        (format!("§7§o{}{name} ({} {})", rei::tr(ru, "collapsed_expand"), list.len(), rei::tr(ru, "entries")), GRAY),
                    ]));
                }
                hits.push(([x - 1.0, y - 1.0, e, e], Hit::Collapsed(*g)));
            }
        }
    }
    // An expanded group's outline: the edges its cells share with others.
    for &(x, y, g) in &members {
        let has = |dx: f32, dy: f32| members.iter().any(|&(mx, my, mg)| mg == g && mx == x + dx && my == y + dy);
        let (x0, y0) = (x - 1.0, y - 1.0);
        if !has(0.0, -e) {
            fill(game, batch, [x0, y0, e, 1.0], 0x67ff_ffff);
        }
        if !has(0.0, e) {
            fill(game, batch, [x0, y0 + e - 1.0, e, 1.0], 0x67ff_ffff);
        }
        if !has(-e, 0.0) {
            fill(game, batch, [x0, y0, 1.0, e], 0x67ff_ffff);
        }
        if !has(e, 0.0) {
            fill(game, batch, [x0 + e - 1.0, y0, 1.0, e], 0x67ff_ffff);
        }
    }
    if !paged {
        batch.clip_since(clip_start, grid[0], grid[1], grid[0] + grid[2], grid[1] + grid[3]);
    }
    // The zoom's percentage after Ctrl+wheel, fading after a few seconds.
    if let Some(at) = s.zoom_at {
        let k = (10.0 * (1.0 - at.elapsed().as_millis() as f32 / 4000.0)).clamp(0.0, 1.0);
        if k > 0.04 {
            let t = format!("{}%", (settings.zoom * 100.0).round());
            let tw = width(game, &t);
            let (zx, zy) = ((a[0] + a[2] / 2.0).floor(), (a[1] + a[3] / 2.0).floor());
            let bg = ((0xa0 as f32 * k).round() as u32) << 24;
            let fg = (((0xdd as f32 * k).round() as u32) << 24) | 0x00ff_ffff;
            let half = (tw / 2.0).floor();
            fill(game, batch, [zx - half - 2.0, zy - 6.0, tw + 4.0, 12.0], bg);
            text(game, batch, &t, zx - half, zy - 4.0, fg, false);
        } else {
            s.zoom_at = None;
        }
    }
    s.hits.extend(hits);
    tip
}

// ---- the search field (`OverlaySearchField`) --------------------------------------------

fn search_field(game: &Game<'_>, batch: &mut HudBatch, s: &mut State, f: Rect, settings: rei::Settings) -> Option<Tip> {
    let ru = s.ru;
    let cursor = game.cursor;
    let hovered = inside(cursor, f);
    let empty_list = s.list_len == 0;
    let atlas = &game.gui.atlas;
    let search = glue::search_box(s);
    [search.x, search.y, search.width, search.height] = f;
    let focused = search.focused;
    let value = search.value.clone();
    let (caret, anchor) = (search.cursor, search.highlight);
    let caret_on = focused && search.cursor_visible();
    let no_results = empty_list && !value.is_empty();
    let border = if no_results {
        0xffff_5555
    } else if hovered || focused {
        WHITE
    } else {
        0xffa0_a0a0
    };
    fill(game, batch, [f[0] - 1.0, f[1] - 1.0, f[2] + 2.0, f[3] + 2.0], 0xff00_0000);
    fill(game, batch, f, border);
    fill(game, batch, [f[0] + 1.0, f[1] + 1.0, f[2] - 2.0, f[3] - 2.0], 0xff00_0000);
    s.hits.push((f, Hit::Search));
    let (tx, ty) = (f[0] + 4.0, f[1] + 5.0);
    let inner = f[2] - 8.0;
    if value.is_empty() && !focused {
        let color = if hovered { 0xddea_eaea } else { 0xffa0_a0a0 };
        text(game, batch, rei::tr(ru, "search"), tx, ty, color, true);
        return None;
    }
    let chars: Vec<char> = value.chars().collect();
    // The first character shown keeps the caret in view.
    let mut scroll = s.search_scroll.min(chars.len());
    if caret < scroll {
        scroll = caret;
    }
    let span = |a: usize, b: usize| HudBatch::text_width(atlas, &chars[a..b].iter().collect::<String>());
    while scroll < caret && span(scroll, caret.min(chars.len())) > inner {
        scroll += 1;
    }
    s.search_scroll = scroll;
    let mut end = scroll;
    while end < chars.len() && span(scroll, end + 1) <= inner {
        end += 1;
    }
    // Each character's colour (`OverlaySearchFieldSyntaxHighlighter`).
    let marks = rei::marks(&value);
    let base = if focused || hovered { 0xffe0_e0e0 } else { WHITE };
    // `*_UNDERSCORED`: the bodies of the prefixed terms underlined.
    let underlined = |i: usize| settings.underline && matches!(marks.get(i), Some(Mark::Body(k)) if *k != rei::Kind::Text);
    let colour = |i: usize| {
        let mut c = if no_results { RED } else { base };
        if settings.syntax {
            match marks.get(i) {
                Some(Mark::Prefix(k) | Mark::Body(k)) => {
                    if let Some(kc) = k.colour() {
                        c = kc;
                    }
                }
                Some(Mark::Bar) => c = GRAY,
                Some(Mark::Quote) => c = 0xffff_aa00,
                _ => {}
            }
        }
        if !(focused || hovered) {
            // `brighter(0.75)`.
            let [a, r, g, b] = c.to_be_bytes();
            let up = |v: u8| ((f32::from(v) / 0.75) as u32).min(255) as u8;
            c = u32::from_be_bytes([a, up(r), up(g), up(b)]);
        }
        c
    };
    // The selection behind the text.
    let (lo, hi) = (caret.min(anchor).max(scroll), caret.max(anchor).min(end));
    if focused && lo < hi {
        let x0 = tx + span(scroll, lo);
        let x1 = tx + span(scroll, hi);
        fill(game, batch, [x0, ty - 1.0, x1 - x0, 10.0], 0x78e0_e0e0);
    }
    let mut x = tx;
    let mut i = scroll;
    while i < end {
        let c = colour(i);
        let ul = underlined(i);
        let mut j = i + 1;
        while j < end && colour(j) == c && underlined(j) == ul {
            j += 1;
        }
        let part: String = chars[i..j].iter().collect();
        let shown = if ul { format!("§n{part}") } else { part.clone() };
        text(game, batch, &shown, x, ty, c, true);
        x += width(game, &part);
        i = j;
    }
    if caret_on && caret >= scroll && caret <= end {
        let cx = tx + span(scroll, caret);
        fill(game, batch, [cx + 1.0, ty, 1.0, 9.0], 0xff38_3838);
        fill(game, batch, [cx, ty - 1.0, 1.0, 9.0], 0xffe0_e0e0);
    }
    None
}

// ---- the hover menus (`Menu`) --------------------------------------------------------

/// A row of a hover menu (`ToggleMenuEntry`, `SubMenuEntry`, `TextMenuEntry`,
/// `EmptyMenuEntry`, `SeparatorMenuEntry`).
enum Row {
    /// An inactive toggle is grey, never lit, and takes no click.
    Toggle { item: MenuItem, label: String, on: bool, active: bool },
    Sub { label: String, rows: Vec<Row> },
    Text(String),
    Gap(f32),
    Separator,
}

fn toggle(item: MenuItem, label: &str, on: bool) -> Row {
    Row::Toggle { item, label: label.to_owned(), on, active: true }
}

/// The gear's menu and the craftable filter's (`ConfigButtonWidget`,
/// `CraftableFilterButtonWidget`), in REI's order.
fn menu_rows(game: &Game<'_>, s: &State, kind: Menu, settings: rei::Settings) -> Vec<Row> {
    let ru = s.ru;
    let t = |k: &str| rei::tr(ru, k);
    match kind {
        Menu::Config => {
            let status = if !settings.cheating(game.creative) {
                "cheating_disabled"
            } else if game.creative {
                "cheating_creative"
            } else {
                "cheating_commands"
            };
            vec![
                toggle(MenuItem::Cheating, t("cheating"), settings.cheating(game.creative)),
                Row::Gap(4.0),
                Row::Text(t(status).to_owned()),
                Row::Separator,
                toggle(MenuItem::DarkTheme, t("dark"), settings.dark),
                toggle(MenuItem::ReducedMotion, t("reduced_motion"), settings.reduced_motion),
                toggle(MenuItem::CraftableFilter, t("menu_craftable_filter"), settings.craftable_button),
                Row::Sub {
                    label: t("menu_display").to_owned(),
                    rows: vec![
                        // The game's recipe book and effects are its own: these
                        // two stay grey here.
                        Row::Toggle { item: MenuItem::Cheating, label: t("menu_remove_recipe_book").to_owned(), on: false, active: false },
                        Row::Toggle { item: MenuItem::Cheating, label: t("menu_left_effects").to_owned(), on: false, active: false },
                        toggle(MenuItem::LeftPanel, t("left_panel"), settings.left_panel),
                        toggle(MenuItem::ScrollingPanel, t("scrolling"), settings.scrolling),
                        Row::Separator,
                        toggle(MenuItem::Syntax, t("menu_syntax"), settings.syntax),
                    ],
                },
                Row::Separator,
                toggle(MenuItem::MoreOptions, t("menu_config"), false),
            ]
        }
        Menu::Craftable => vec![
            Row::Sub {
                label: t("menu_search_position").to_owned(),
                rows: ["CENTER", "BOTTOM_SIDE", "TOP_SIDE"]
                    .iter()
                    .enumerate()
                    .map(|(i, name)| {
                        let current = settings.search_position == i as u8;
                        Row::Toggle { item: MenuItem::SearchPosition(i as u8), label: (*name).to_owned(), on: current, active: !current }
                    })
                    .collect(),
            },
            toggle(MenuItem::HideIdle, t("menu_hide_idle"), settings.hide_idle),
        ],
    }
}

fn check_mark(game: &Game<'_>) -> &'static str {
    if game.gui.atlas.glyph('✔').is_some() { "✔" } else { "✓" }
}

fn row_size(game: &Game<'_>, r: &Row) -> (f32, f32) {
    match r {
        Row::Toggle { label, .. } => (width(game, label) + 12.0, 12.0),
        Row::Sub { label, .. } => (width(game, label) + 16.0, 12.0),
        Row::Text(label) => (width(game, label) + 4.0, 12.0),
        Row::Gap(h) => (0.0, *h),
        Row::Separator => (0.0, 5.0),
    }
}

/// A menu's row width and content height.
fn menu_size(game: &Game<'_>, rows: &[Row]) -> (f32, f32) {
    let w = rows.iter().map(|r| row_size(game, r).0).fold(10.0, f32::max);
    (w, rows.iter().map(|r| row_size(game, r).1).sum())
}

/// The open hover menu over button `owner` (`Menu`), placed below or above
/// it and growing out of its corner over 300 ms.
fn draw_menu(game: &Game<'_>, batch: &mut HudBatch, s: &mut State, kind: Menu, owner: Rect, settings: rei::Settings) {
    let rows = menu_rows(game, s, kind, settings);
    let (row_w, content_h) = menu_size(game, &rows);
    let w = row_w + 2.0;
    let (sw, sh) = (batch.width, batch.height);
    let down = sh - (owner[1] + owner[3]) > owner[1];
    let y = if down { owner[1] + owner[3] } else { owner[1] - (content_h + 2.0) };
    let right = sw - (owner[0] + owner[2]) >= w + 10.0;
    let x = if right { owner[0] } else { owner[0] + owner[2] - w };
    let h = content_h.min(sh - 20.0 - y) + 2.0;
    let corner = (if right { x } else { x + w }, if down { y } else { y + h });
    let opened = s.menu_at.unwrap_or_else(Instant::now);
    let previous = std::mem::take(&mut s.menu_rects);
    let mut hits = Vec::new();
    menu_level(game, batch, s, &rows, [x, y, w, h], corner, opened, 0, right, &previous, &mut hits);
    s.hits.extend(hits);
}

/// One menu level and, when a submenu row is selected, its child beside it
/// (`SubMenuEntry`): rows lit under the pointer and on the focused row.
#[allow(clippy::too_many_arguments)]
fn menu_level(
    game: &Game<'_>,
    batch: &mut HudBatch,
    s: &mut State,
    rows: &[Row],
    full: Rect,
    corner: (f32, f32),
    opened: Instant,
    depth: usize,
    right: bool,
    previous: &[(usize, Rect)],
    hits: &mut Vec<(Rect, Hit)>,
) {
    let cursor = game.cursor;
    let reduced = s.settings.is_some_and(|x| x.reduced_motion);
    let t = if reduced { 1.0 } else { (opened.elapsed().as_millis() as f32 / 300.0).min(1.0) };
    let e = 1.0 - (1.0 - t).powi(3);
    let anim = [
        corner.0 + (full[0] - corner.0) * e,
        corner.1 + (full[1] - corner.1) * e,
        (full[2] * e).max(0.1),
        (full[3] * e).max(0.1),
    ];
    s.menu_rects.push((depth, full));
    // White while the pointer is in this menu or one it opened.
    let lit = previous.iter().any(|(d, r)| *d >= depth && inside(cursor, *r));
    let border = match (lit, dark()) {
        (true, false) => WHITE,
        (true, true) => 0xffff_bb4d,
        (false, _) => 0xffa0_a0a0,
    };
    fill(game, batch, anim, border);
    let inner = [anim[0] + 1.0, anim[1] + 1.0, anim[2] - 2.0, anim[3] - 2.0];
    fill(game, batch, inner, 0xff00_0000);
    hits.push((full, Hit::Area));
    let start = batch.vertices.len();
    let row_w = full[2] - 2.0;
    let check = check_mark(game);
    let mut child = None;
    let mut y = full[1] + 1.0;
    for (i, row) in rows.iter().enumerate() {
        let h = row_size(game, row).1;
        let r = [full[0] + 1.0, y, row_w, h];
        let under = inside(cursor, r) && inside(cursor, inner);
        if under {
            if s.menu_focus[depth] != Some(i) {
                s.menu_focus[depth] = Some(i);
                s.menu_focus[(depth + 1).min(2)] = None;
            }
        }
        let selected = s.menu_focus[depth] == Some(i) || under;
        match row {
            Row::Toggle { item, label, on, active } => {
                let shown = selected && *active;
                if shown {
                    fill(game, batch, r, 0xff45_4545);
                }
                let color = if shown { WHITE } else { 0xff88_8888 };
                text(game, batch, label, r[0] + 2.0, y + 2.0, color, false);
                if *on {
                    let cw = width(game, check);
                    text(game, batch, check, r[0] + row_w - 2.0 - cw, y + 2.0, color, false);
                }
                if *active {
                    hits.push((r, Hit::Menu(*item)));
                }
            }
            Row::Sub { label, rows: children } => {
                if selected {
                    fill(game, batch, r, 0xff45_4545);
                    child = Some((i, y, children));
                }
                text(game, batch, label, r[0] + 2.0, y + 2.0, if selected { WHITE } else { 0xff88_8888 }, false);
                blit(game, batch, &sprites::SUBMENU, r[0] + row_w - 15.0, y - 2.0, 1.0);
            }
            Row::Text(label) => text(game, batch, label, r[0] + 2.0, y + 2.0, 0xff88_8888, false),
            Row::Separator => fill(game, batch, [r[0] + 3.0, y + 2.0, row_w - 6.0, 1.0], 0xff88_8888),
            Row::Gap(_) => {}
        }
        y += h;
    }
    batch.clip_since(start, inner[0], inner[1], inner[0] + inner[2], inner[1] + inner[3]);
    // The child menu beside its row, facing the side with room.
    let Some((row, row_y, children)) = child else {
        if s.sub_at.is_some_and(|(k, _)| k / 100 == depth) {
            s.sub_at = None;
        }
        return;
    };
    let (sw, sh) = (batch.width, batch.height);
    let (cw, ch) = menu_size(game, children);
    let cw = cw + 2.0;
    let start_rect = [full[0], row_y, full[2], 12.0];
    let mut facing = right;
    if facing && sw - (start_rect[0] + start_rect[2]) < cw + 10.0 {
        facing = false;
    } else if !facing && start_rect[0] < cw + 10.0 {
        facing = true;
    }
    let down = sh - (start_rect[1] + start_rect[3]) > start_rect[1];
    let cy = if down { row_y - 1.0 } else { row_y + 12.0 - (ch + 1.0) };
    let cx = if facing { full[0] + full[2] } else { full[0] - cw };
    let rect = [cx, cy, cw, ch.min(sh - 20.0 - cy) + 2.0];
    let key = depth * 100 + row;
    let at = match s.sub_at {
        Some((k, at)) if k == key => at,
        _ => {
            let now = Instant::now();
            s.sub_at = Some((key, now));
            now
        }
    };
    let corner = (if facing { cx } else { cx + cw }, if down { cy } else { cy + rect[3] });
    menu_level(game, batch, s, children, rect, corner, at, depth + 1, facing, previous, hits);
}

// ---- the favorites (`FavoritesListWidget`, `FavoritesPanel`) ----------------------------

/// A shortcut's look in an 18x18 cell at (x, y).
fn shortcut(game: &Game<'_>, batch: &mut HudBatch, sc: Shortcut, x: f32, y: f32, ru: bool) {
    let hovered = inside(game.cursor, [x, y, E, E]);
    let (cx, cy) = (x + 9.0, y + 9.0);
    match sc {
        Shortcut::GameMode(i) => {
            let t = rei::tr(ru, ["short_survival", "short_creative", "short_adventure", "short_spectator"][usize::from(i) % 4]);
            let w = width(game, t);
            text(game, batch, t, cx - w / 2.0 + 0.5, cy - 3.5, if hovered { 0xffee_eeee } else { 0xffaa_aaaa }, false);
        }
        Shortcut::Weather(i) => {
            let s = [&sprites::WEATHER_CLEAR, &sprites::WEATHER_RAIN, &sprites::WEATHER_THUNDER][usize::from(i) % 3];
            blit(game, batch, s, cx - 7.0, cy - 7.0, 1.0);
        }
        Shortcut::Time(i) => {
            let s = [&sprites::TIME_MORNING, &sprites::TIME_NOON, &sprites::TIME_EVENING, &sprites::TIME_NIGHT][usize::from(i) % 4];
            blit(game, batch, s, cx - 7.0, cy - 7.0, 1.0);
        }
    }
}

fn shortcut_name(ru: bool, sc: Shortcut, game: &Game<'_>) -> String {
    match sc {
        Shortcut::GameMode(i) => {
            let name = Shortcut::GAME_MODES[usize::from(i) % 4];
            game.translate(&format!("gameMode.{name}"), name)
        }
        Shortcut::Weather(i) => rei::tr(ru, ["weather_clear", "weather_rain", "weather_thunder"][usize::from(i) % 3]).to_owned(),
        Shortcut::Time(i) => rei::tr(ru, ["time_morn", "time_noon", "time_evening", "time_night"][usize::from(i) % 4]).to_owned(),
    }
}

/// The region beside the screen on one side (ScreenRegistryImpl.getOverlayBounds):
/// none when less than 10 px are left.
fn side_region(screen: Rect, w: f32, h: f32, left: bool) -> Option<Rect> {
    if left {
        (screen[0] >= 10.0).then(|| [2.0, 0.0, screen[0] - 2.0, h])
    } else {
        let right = screen[0] + screen[2];
        (w - right >= 10.0).then(|| [right + 2.0, 0.0, w - right - 4.0, h])
    }
}

#[allow(clippy::too_many_arguments)]
fn favorites(game: &Game<'_>, batch: &mut HudBatch, s: &mut State, screen: Rect, zones: &[Rect], covered: bool, settings: rei::Settings) -> Option<Tip> {
    let ru = s.ru;
    let cursor = game.cursor;
    let entries_left = settings.left_panel;
    let e = entry_size(settings);
    // The side opposite the entry list, 25 px lower under a top config button.
    let region = side_region(screen, batch.width, batch.height, !entries_left)?;
    let y_off = 8.0 + if settings.config_upper { 25.0 } else { 0.0 };
    let mut f = [region[0], y_off, region[2], batch.height - 3.0 - y_off];
    let max_w = (e * 50.0 + e * 0.75).ceil() + 8.0;
    if f[2] > max_w {
        if entries_left {
            f[0] += f[2] - max_w;
        }
        f[2] = max_w;
    }
    // The display history over the favorites (`DisplayHistoryWidget`).
    let mut history_hits = Vec::new();
    let mut history_tip = None;
    s.history_area = None;
    if !s.display_history.is_empty() {
        let hs = [f[0] + 2.0, f[1], f[2] - 4.0, f[3].min(80.0)];
        s.history_area = Some(hs);
        history(game, batch, s, hs, &mut history_hits, &mut history_tip);
        f = [f[0], hs[1] + hs[3], f[2], f[1] + f[3] - (hs[1] + hs[3])];
    }
    let cols = (((f[2] - 8.0) / e).floor() as usize).max(1);
    let shift = if entries_left { 3.0 } else { -3.0 };
    let gx = (f[0] + (f[2] / 2.0).floor() - cols as f32 * e / 2.0 + shift).floor();
    let gy = f[1];
    let add = [f[0] + 4.0, f[1] + f[3] - 20.0, 16.0, 16.0];
    let add_mode = settings.favorites_button;
    let mut zones: Vec<Rect> = zones.to_vec();
    if add_mode != 0 {
        zones.push(add);
    }
    let keys = glue::favorites(s).keys.clone();
    // The cells in order, skipping those on a zone.
    let max_rows = keys.len() + 1;
    let mut spots = Vec::new();
    'grid: for r in 0..max_rows * 2 {
        for c in 0..cols {
            if spots.len() >= keys.len() {
                break 'grid;
            }
            let (x, y) = (gx + c as f32 * e, gy + r as f32 * e);
            let cell = [x, y - s.favorites_scroll, e, e];
            if zones.iter().any(|z| intersects(cell, *z)) {
                continue;
            }
            spots.push((x, y));
        }
    }
    let content = spots.last().map_or(0.0, |&(_, y)| y + e - gy);
    s.favorites_scroll = s.favorites_scroll.min((content - f[3]).max(0.0));
    let scroll = s.favorites_scroll;
    s.favorites_grid = Some((f, gx, gy, cols));
    let mut hits: Vec<(Rect, Hit)> = history_hits;
    let mut tip = history_tip;
    let data = s.data.as_ref()?;
    let start = batch.vertices.len();
    for (i, (key, &(x, y))) in keys.iter().zip(spots.iter()).enumerate() {
        let y = y - scroll;
        if y + e < f[1] || y > f[1] + f[3] {
            continue;
        }
        let cell = [x, y, e, e];
        let hovered = inside(cursor, cell) && inside(cursor, f);
        if let Some(sc) = Shortcut::from_key(key) {
            shortcut(game, batch, sc, x, y, ru);
            if hovered {
                tip = Some(Tip::text(vec![(format!("{}{}", rei::tr(ru, "switch_to"), shortcut_name(ru, sc, game)), WHITE)]));
            }
        } else if let Some(ingredient) = data.ingredient_by_key(key).and_then(|id| data.ingredient(id)) {
            cell_item(game, batch, &ingredient.stack, x + 1.0, y + 1.0, e);
            if hovered {
                hover_sized(game, batch, x + 1.0, y + 1.0, e - 2.0);
                let mut lines = Vec::new();
                if settings.favorites_hint {
                    lines.push((" ".to_owned(), GRAY));
                    lines.push((format!("§7{}", rei::tr(ru, "remove_favorites_tooltip").replace("%s", &glue::favorite_key_name(game))), GRAY));
                }
                tip = Some(Tip::stack(ingredient.stack.clone(), lines));
            }
        } else {
            continue;
        }
        // Only the part inside the region takes clicks.
        let top = y.max(f[1]);
        let bottom = (y + e).min(f[1] + f[3]);
        if bottom > top {
            hits.push(([x, top, e, bottom - top], Hit::Favorite(i)));
        }
    }
    batch.clip_since(start, f[0], f[1], f[0] + f[2], f[1] + f[3]);

    // The "Add..." button and its panel of game mode, weather and time.
    let p = panel_progress(s);
    s.panel_area = None;
    if p > 0.0 && !covered {
        let pw = 16.0 + (p.min(1.0) * (f[2] - 24.0)).round();
        let ph = 16.0 + (p * (f[3] * 0.4 - 20.0)).round();
        let panel = [f[0] + 4.0, f[1] + f[3] - ph - 4.0, pw, ph];
        let alpha = (f32::from(0x34u8) * (2.0 * p).min(1.0)) as u32;
        fill(game, batch, panel, (alpha << 24) | 0x00ff_ffff);
        hits.push((panel, Hit::Area));
        if p >= 1.0 {
            let e = E;
            let inner = [panel[0] + 4.0, panel[1] + 4.0, panel[2] - 8.0, panel[3] - 20.0];
            let start = batch.vertices.len();
            let row_cols = ((inner[2] / e).floor() as usize).max(1);
            // Its rows scroll when they don't fit.
            let content = 3.0 * 11.0 + 2.0 * 5.0 + e * (4usize.div_ceil(row_cols) * 2 + 3usize.div_ceil(row_cols)) as f32;
            s.panel_scroll = s.panel_scroll.min((content - inner[3]).max(0.0));
            s.panel_area = Some(inner);
            let mut y = inner[1] - s.panel_scroll;
            let sections: [(&str, Vec<Shortcut>); 3] = [
                ("section_gamemode", (0..4).map(Shortcut::GameMode).collect()),
                ("section_weather", (0..3).map(Shortcut::Weather).collect()),
                ("section_time", (0..4).map(Shortcut::Time).collect()),
            ];
            for (n, (title, list)) in sections.iter().enumerate() {
                let t = rei::tr(ru, title);
                text(game, batch, t, inner[0], y + 1.0, WHITE, false);
                y += 11.0;
                for (k, sc) in list.iter().enumerate() {
                    let (cx, cy) = (inner[0] + (k % row_cols) as f32 * e, y + (k / row_cols) as f32 * e);
                    shortcut(game, batch, *sc, cx, cy, ru);
                    let cell = [cx, cy, e, e];
                    if inside(cursor, cell) && inside(cursor, inner) {
                        tip = Some(Tip::text(vec![
                            (format!("{}{}", rei::tr(ru, "switch_to"), shortcut_name(ru, *sc, game)), WHITE),
                            (rei::tr(ru, "drag_to_add").to_owned(), GRAY),
                        ]));
                    }
                    let top = cy.max(inner[1]);
                    let bottom = (cy + e).min(inner[1] + inner[3]);
                    if bottom > top {
                        hits.push(([cx, top, e, bottom - top], Hit::Shortcut(*sc)));
                    }
                }
                y += list.len().div_ceil(row_cols) as f32 * e;
                if n < 2 {
                    fill(game, batch, [inner[0], y + 2.0, inner[2], 1.0], 0xddea_eaea);
                    y += 5.0;
                }
            }
            batch.clip_since(start, inner[0], inner[1], inner[0] + inner[2], inner[1] + inner[3]);
        }
    }
    // `favorites.new_favorites_button_visibility`: never, when active (lit
    // under the pointer, half over the favorites, fading over 260 ms), always.
    let target = match add_mode {
        0 => 0.0,
        1 if inside(cursor, add) => 1.0,
        1 if inside(cursor, f) || s.panel => 0.5,
        1 => 0.0,
        _ if inside(cursor, add) => 1.0,
        _ => 0.5,
    };
    let step = if settings.reduced_motion { 1.0 } else { 1.0 / 0.26 / 60.0 };
    s.add_alpha = if (s.add_alpha - target).abs() <= step { target } else if s.add_alpha < target { s.add_alpha + step } else { s.add_alpha - step };
    if add_mode == 2 {
        s.add_alpha = target;
    }
    let a = s.add_alpha;
    if !covered && a > 0.0 {
        let fill_alpha = (f32::from(0x74u8) * a) as u32;
        fill(game, batch, add, (fill_alpha << 24) | 0x00ff_ffff);
        let glyph_alpha = (255.0 * a) as u32;
        text(game, batch, "+", add[0] + 5.5, add[1] + 5.0, (glyph_alpha << 24) | 0x00ff_ffff, false);
        hits.push((add, Hit::AddButton));
        if inside(cursor, add) {
            tip = Some(Tip::text(vec![(rei::tr(ru, "add_favorite_widget").to_owned(), WHITE)]));
        }
    }
    s.hits.extend(hits);
    tip
}

/// The display shown in the history strip `hs`, scaled to fit
/// (`DisplayEntry`), with its own "+" button when this screen could move it.
fn history(game: &Game<'_>, batch: &mut HudBatch, s: &mut State, hs: Rect, hits: &mut Vec<(Rect, Hit)>, tip: &mut Option<Tip>) {
    let cursor = game.cursor;
    let ru = s.ru;
    let millis = s.started.map_or(0, |at| at.elapsed().as_millis() as u64);
    s.history_page = s.history_page.min(s.display_history.len().saturating_sub(1));
    let Some(&ri) = s.display_history.get(s.history_page) else {
        return;
    };
    let status = s.data.as_ref().and_then(|d| d.index.recipes.get(ri)).map_or(Status::NotSupported, |r| glue::status_now(s, r));
    let reduced_motion = s.settings.is_some_and(|x| x.reduced_motion);
    let Some(data) = s.data.as_ref() else {
        return;
    };
    let Some(recipe) = data.index.recipes.get(ri) else {
        return;
    };
    let category = recipe.category();
    let (dw, dh) = (DISPLAY_W, category.height());
    let mut sc: f32 = 1.0;
    if hs[2] * sc < dw {
        sc = sc.min(hs[2] * sc / dw);
    }
    if hs[3] * sc < dh {
        sc = sc.min(hs[3] * sc / dh);
    }
    let t = [
        hs[0] + (hs[2] / 2.0).floor() - (dw / 2.0).floor() * sc,
        hs[1] + (hs[3] / 2.0).floor() - (dh / 2.0).floor() * sc,
        dw * sc,
        dh * sc,
    ];
    let local = if inside(cursor, hs) { ((cursor.0 - t[0]) / sc, (cursor.1 - t[1]) / sc) } else { (-1000.0, -1000.0) };
    let ctx = Context { game, data, ru, millis, focus: None, cursor: local, reduced_motion, border: s.settings.map_or(0, |x| x.recipe_border) };
    let start = batch.vertices.len();
    let (slot_hits, slot_tip) = display(&ctx, batch, category, recipe, 0.0, 0.0);
    // The slots nothing here fills stay red (an upstream quirk).
    if let Status::Missing(cells) = &status {
        for &c in cells {
            let (col, row) = ((c % 3) as f32, (c / 3) as f32);
            fill(game, batch, [18.0 + 18.0 * col, 7.0 + 18.0 * row, 16.0, 16.0], 0x40ff_0000);
        }
    }
    batch.scale_since(start, 0.0, 0.0, sc, sc);
    for vert in &mut batch.vertices[start..] {
        vert.pos[0] += t[0];
        vert.pos[1] += t[1];
    }
    batch.clip_since(start, hs[0], hs[1], hs[0] + hs[2], hs[1] + hs[3]);
    hits.push((t, Hit::History(ri)));
    for (r, h) in slot_hits {
        hits.push(([t[0] + r[0] * sc, t[1] + r[1] * sc, r[2] * sc, r[3] * sc], h));
    }
    if slot_tip.is_some() {
        *tip = slot_tip;
    }
    // Its "+": only where a handler applies.
    if status != Status::NotSupported {
        let r = [(t[0] + t[2] - 14.0).floor(), (t[1] + t[3] - 14.0).floor(), 10.0, 10.0];
        let enabled = status == Status::Ok;
        button(game, batch, r, enabled, "+");
        if !enabled {
            fill(game, batch, [r[0] + 1.0, r[1] + 1.0, 8.0, 8.0], 0x67ff_0000);
        }
        hits.push((r, if enabled { Hit::HistoryTransfer(ri) } else { Hit::Area }));
        if inside(cursor, r) {
            let line = match status {
                Status::Ok => (rei::tr(ru, "move_items").to_owned(), WHITE),
                Status::TooSmall => (rei::tr(ru, "too_small").to_owned(), RED),
                _ => (rei::tr(ru, "not_enough").to_owned(), RED),
            };
            *tip = Some(Tip::text(vec![line]));
        }
    }
}

/// How far the favorites panel is open, 0 to 1 (it grows over 300 ms).
fn panel_progress(s: &State) -> f32 {
    let reduced = s.settings.is_some_and(|x| x.reduced_motion);
    let t = s.panel_at.filter(|_| !reduced).map_or(1.0, |at| (at.elapsed().as_millis() as f32 / 300.0).min(1.0));
    if s.panel { t } else { 1.0 - t }
}

// ---- the recipe screen (`DefaultDisplayViewingScreen`) ----------------------------------

/// A category's title: REI's, or the game's own block name for the anvil.
fn title(game: &Game<'_>, ru: bool, c: Category) -> String {
    match c {
        Category::Anvil => game.translate("block.minecraft.anvil", rei::category_title(ru, c)),
        _ => rei::category_title(ru, c).to_owned(),
    }
}

fn recipe_screen(game: &Game<'_>, batch: &mut HudBatch, s: &mut State, w: &Window, millis: u64) -> Option<Tip> {
    let ru = s.ru;
    let mod_name = s.settings.is_none_or(|x| x.mod_name);
    let cursor = game.cursor;
    let b = w.bounds;
    let (bx, by, bw, bh) = (b[0], b[1], b[2], b[3]);
    let cx = bx + (bw / 2.0).floor();
    let cy = by + (bh / 2.0).floor();
    let category = w.category;
    let tabs: Vec<Category> = s.recipes.tabs.iter().map(|(c, _)| *c).collect();
    let recipes: Vec<usize> = s.recipes.recipes().to_vec();
    let page = s.recipes.page;
    let selected = s.recipes.tab;
    // Status of each shown display, worked out before the data is borrowed.
    let shown: Vec<usize> = recipes.iter().copied().skip(page * w.per_page).take(w.per_page).collect();
    let statuses: Vec<Status> = shown
        .iter()
        .map(|&i| s.data.as_ref().map_or(Status::NotSupported, |d| glue::status_now(s, &d.index.recipes[i])))
        .collect();
    let all_categories = s.data.as_ref().map_or(0, |d| Category::ALL.iter().filter(|c| !d.index.of_category(**c).is_empty()).count());
    let advanced = game.settings.file.bool("advancedItemTooltips");
    let favorites_on = s.settings.is_none_or(|x| x.favorites);
    let border = s.settings.map_or(0, |x| x.recipe_border);
    let (ts, tpp) = tab_metrics(s);
    let larger_tabs = ts > 24.0;
    let larger_arrows = s.settings.is_none_or(|x| x.larger_arrows);
    // Tag views start afresh on each page (REI makes the widgets again), and
    // one held by the pointer follows it.
    let page_key = s.recipes.lookup.map(|l| (l, selected, page));
    if s.tag_views_for != page_key {
        s.tag_views.clear();
        s.tag_views_for = page_key;
    }
    if let Some(press) = s.press
        && let Hit::TagView(ri) = press.hit
    {
        let (dx, dy) = (cursor.0 - s.pan_last.0, cursor.1 - s.pan_last.1);
        let view = s.tag_views.entry(ri).or_default();
        view.offset = (view.offset.0 + dx, view.offset.1 + dy);
        s.pan_last = cursor;
    }
    let mut views = std::mem::take(&mut s.tag_views);
    let data = s.data.as_ref()?;
    let mut hits: Vec<(Rect, Hit)> = Vec::new();
    let mut tip: Option<Tip> = None;

    // The tabs: unselected ones behind the window, the selected over it.
    let n = tabs.len();
    let first = if n > tpp { selected.saturating_sub(tpp / 2).min(n - tpp) } else { 0 };
    let count = n.min(tpp);
    let left = cx - (count as f32 * ts / 2.0).floor();
    let tab_rect = |i: usize| [left + (i - first) as f32 * ts, by - ts, ts, ts];
    let draw_tab = |batch: &mut HudBatch, i: usize, sel: bool| {
        let r = tab_rect(i);
        let sprite = match (larger_tabs, sel) {
            (false, true) => th(&sprites::TAB_SELECTED, &sprites::TAB_SELECTED_DARK),
            (false, false) => th(&sprites::TAB_UNSELECTED, &sprites::TAB_UNSELECTED_DARK),
            (true, true) => th(&sprites::TAB_LARGE_SELECTED, &sprites::TAB_LARGE_SELECTED_DARK),
            (true, false) => th(&sprites::TAB_LARGE_UNSELECTED, &sprites::TAB_LARGE_UNSELECTED_DARK),
        };
        blit(game, batch, sprite, r[0], r[1] + 2.0, 1.0);
        // The icon at the tab's centre, 5 px up (`TabWidget`).
        if let Some(icon) = data.icons.get(&tabs[i]).and_then(|&id| data.ingredient(id)) {
            item(game, batch, &icon.stack, r[0] + (ts / 2.0).floor() - 8.0, by - (ts / 2.0).floor() - 5.0);
        }
    };
    for i in first..first + count {
        if i != selected {
            draw_tab(batch, i, false);
        }
    }

    // The workstations, in a panel under the window's left edge.
    if let Some(panel) = w.panel {
        let ids = data.index.workstations(category);
        let hh = (((bh - 16.0) / 18.0).floor() as usize).max(1);
        let cols = ids.len().div_ceil(hh);
        let rows = hh.min(ids.len());
        let xx = bx - (8.0 + cols as f32 * 16.0) + 6.0;
        let yy = by + 16.0;
        nine(game, batch, th(&sprites::PANEL, &sprites::PANEL_DARK), panel);
        nine(game, batch, th(&sprites::PANEL_SLOT, &sprites::PANEL_SLOT_DARK), [xx - 1.0, yy - 1.0, cols as f32 * 16.0 + 2.0, rows as f32 * 16.0 + 2.0]);
        hits.push((panel, Hit::Area));
        for (k, &id) in ids.iter().enumerate() {
            let (col, row) = (k / hh, k % hh);
            let x = xx + (cols - 1 - col) as f32 * 16.0;
            let y = yy + row as f32 * 16.0;
            let Some(ingredient) = data.ingredient(id) else {
                continue;
            };
            item(game, batch, &ingredient.stack, x, y);
            if inside(cursor, [x, y, 16.0, 16.0]) {
                hover(game, batch, x, y);
                tip = Some(Tip::stack(ingredient.stack.clone(), Vec::new()));
            }
            hits.push(([x, y, 16.0, 16.0], Hit::Workstation(id)));
        }
    }

    // The window and its two stripes.
    nine(game, batch, th(&sprites::PANEL, &sprites::PANEL_DARK), b);
    hits.push((b, Hit::Area));
    fill(game, batch, [bx + 17.0, by + 5.0, bw - 34.0, 12.0], if dark() { 0xff40_4040 } else { STRIPE });
    fill(game, batch, [bx + 17.0, by + 19.0, bw - 34.0, 12.0], if dark() { 0xff40_4040 } else { STRIPE });
    let buttons = [
        ([bx + 5.0, by + 5.0, 12.0, 12.0], true, n > 1, Hit::CategoryTurn(-1), "previous_category"),
        ([bx + bw - 17.0, by + 5.0, 12.0, 12.0], false, n > 1, Hit::CategoryTurn(1), "next_category"),
        ([bx + 5.0, by + 19.0, 12.0, 12.0], true, w.pages > 1, Hit::RecipePage(-1), "previous_page"),
        ([bx + bw - 17.0, by + 19.0, 12.0, 12.0], false, w.pages > 1, Hit::RecipePage(1), "next_page"),
    ];
    for (r, back, enabled, hit, key) in buttons {
        arrow_button(game, batch, r, back, enabled);
        if enabled {
            hits.push((r, hit));
        }
        if inside(cursor, r) {
            tip = Some(Tip::text(vec![(rei::tr(ru, key).to_owned(), WHITE)]));
        }
    }
    let t = title(game, ru, category);
    let title_bounds = label(game, batch, &t, (cx, by + 7.0), true);
    hits.push((title_bounds, Hit::Title));
    if inside(cursor, title_bounds) {
        tip = Some(Tip::text(vec![
            (rei::tr(ru, "view_all_categories").to_owned(), WHITE),
            (format!("{all_categories} {}", rei::tr(ru, "categories")), DARK_GRAY),
        ]));
    }
    let page_text = format!("{}/{}", page + 1, w.pages);
    let page_bounds = label(game, batch, &page_text, (cx, by + 21.0), w.pages > 1);
    if w.pages > 1 {
        hits.push((page_bounds, Hit::PageLabel));
        if inside(cursor, page_bounds) {
            tip = Some(Tip::text(vec![
            (rei::tr(ru, "go_back_first_page").to_owned(), WHITE),
            (" ".to_owned(), WHITE),
            (rei::tr(ru, "shift_click_choose").to_owned(), GRAY),
        ]));
        }
    }

    // The displays.
    let h = category.height();
    let per = w.per_page as f32;
    let focus = s.recipes.lookup.and_then(Lookup::focus);
    let reduced_motion = s.settings.is_some_and(|x| x.reduced_motion);
    let ctx = Context { game, data, ru, millis, focus, cursor, reduced_motion, border };
    for (k, (&ri, status)) in shown.iter().zip(statuses.iter()).enumerate() {
        let Some(recipe) = data.index.recipes.get(ri) else {
            continue;
        };
        let x = cx - DISPLAY_W / 2.0;
        let y = cy + 16.0 - ((h * per) / 2.0).floor() - 2.0 * per + (h + 4.0) * k as f32;
        if category == Category::Tag {
            let view = views.entry(ri).or_default();
            let (slot_hits, slot_tip, clear) = tag_display(&ctx, batch, recipe, ri, x, y, view);
            hits.extend(slot_hits);
            if clear {
                tip = None;
            }
            if slot_tip.is_some() {
                tip = slot_tip;
            }
        } else {
            let (slot_hits, slot_tip) = display(&ctx, batch, category, recipe, x, y);
            hits.extend(slot_hits);
            if slot_tip.is_some() {
                tip = slot_tip;
            }
        }
        // The "+" button right of the display, at its foot.
        let r = [x + DISPLAY_W + 2.0, y + h - 16.0, 10.0, 10.0];
        let (glyph, enabled, tint) = match status {
            Status::NotSupported => ("!", false, 0),
            Status::Ok => ("+", true, 0),
            Status::Missing(_) | Status::TooSmall => ("+", false, 0x67ff_0000),
        };
        button(game, batch, r, enabled, glyph);
        if tint != 0 {
            fill(game, batch, [r[0] + 1.0, r[1] + 1.0, 8.0, 8.0], tint);
        }
        hits.push((r, if enabled { Hit::Transfer(ri) } else { Hit::TransferOff(ri) }));
        if inside(cursor, r) {
            let mut missing_grid: Option<Vec<ItemStack>> = None;
            let mut lines = match status {
                Status::NotSupported => vec![(rei::tr(ru, "not_supported").to_owned(), RED)],
                Status::TooSmall => vec![(rei::tr(ru, "too_small").to_owned(), RED)],
                Status::Missing(cells) => {
                    // The slots nothing here fills, in red.
                    for &c in cells {
                        let (col, row) = ((c % 3) as f32, (c / 3) as f32);
                        fill(game, batch, [x + 18.0 + 18.0 * col, y + 7.0 + 18.0 * row, 16.0, 16.0], 0x40ff_0000);
                    }
                    // Cells needing the same entries merge, their counts added.
                    let mut merged: Vec<(Vec<Stack>, u32)> = Vec::new();
                    for &c in cells {
                        let entries: Vec<Stack> = recipe.inputs.get(c).map(|cell| cell.iter().copied().filter(|x| !x.is_empty()).collect()).unwrap_or_default();
                        let Some(first) = entries.first().copied() else {
                            continue;
                        };
                        match merged.iter_mut().find(|(e, _)| e.iter().map(|s| s.id).eq(entries.iter().map(|s| s.id))) {
                            Some((_, n)) => *n += first.count.max(1),
                            None => merged.push((entries, first.count.max(1))),
                        }
                    }
                    let second = (millis_now(game) / 1000) as usize;
                    missing_grid = Some(
                        merged
                            .iter()
                            .filter_map(|(e, n)| {
                                let mut st = data.stack(e[second % e.len()])?;
                                st.count = *n;
                                Some(st)
                            })
                            .collect::<Vec<_>>(),
                    );
                    vec![(rei::tr(ru, "not_enough").to_owned(), RED)]
                }
                Status::Ok => vec![(rei::tr(ru, "move_items").to_owned(), WHITE)],
            };
            if favorites_on {
                let key = glue::favorite_key_name(game);
                lines.push((" ".to_owned(), GRAY));
                lines.push((rei::tr(ru, "save_recipes").replace("%s", &format!("§l{key}§r§7")), GRAY));
            }
            if advanced && !recipe.id.is_empty() {
                lines.push((" ".to_owned(), GRAY));
                lines.push((format!("{}{}", rei::tr(ru, "recipe_id"), recipe.id), GRAY));
            }
            let t = Tip::text(lines);
            tip = Some(match missing_grid.take() {
                Some(cells) => t.with(Component::Grid { title: rei::tr(ru, "missing").to_owned(), cells, tag: None }),
                None => t,
            });
        }
    }

    // The selected tab over the window's edge, then the tabs' tooltips.
    if selected < n {
        draw_tab(batch, selected, true);
    }
    for i in first..first + count {
        let r = tab_rect(i);
        hits.push((r, Hit::Tab(i)));
        if inside(cursor, r) {
            let mut lines = vec![(title(game, ru, tabs[i]), WHITE)];
            if mod_name {
                lines.push(("Minecraft".to_owned(), BLUE));
            }
            tip = Some(Tip::text(lines));
        }
    }
    if n > tpp {
        // `getCategoryButtons`: 16 px, or 10 px with small arrows.
        let b = if larger_arrows { 16.0 } else { 10.0 };
        let y = by - ts + 1.0 - b;
        let back = [bx, y, b, b];
        let next = [bx + bw - b - if larger_arrows { 1.0 } else { 0.0 }, y, b, b];
        for (r, is_back) in [(back, true), (next, false)] {
            button(game, batch, r, true, "");
            if larger_arrows {
                blit(game, batch, if is_back { &sprites::ARROW_LEFT } else { &sprites::ARROW_RIGHT }, r[0] + 4.0, r[1] + 4.0, 1.0);
            } else {
                blit(game, batch, if is_back { &sprites::ARROW_LEFT_SMALL } else { &sprites::ARROW_RIGHT_SMALL }, r[0] + 2.0, r[1] + 2.5, 1.0);
            }
            if inside(cursor, r) {
                tip = Some(Tip::text(vec![(rei::tr(ru, if is_back { "previous_page" } else { "next_page" }).to_owned(), WHITE)]));
            }
        }
        hits.push((back, Hit::TabPage(-1)));
        hits.push((next, Hit::TabPage(1)));
    }
    s.tag_views = views;
    s.hits.extend(hits);
    tip
}

/// A tag display (`DefaultTagCategory`): the tag's tree in a view that drags
/// and zooms, the expand button that grows it over most of the screen, the
/// copy button. Returns its parts, its tooltip, and whether the pointer is in
/// the view (REI drops the tooltips of what is under it).
#[allow(clippy::too_many_arguments)]
fn tag_display(
    ctx: &Context<'_, '_>,
    batch: &mut HudBatch,
    r: &Recipe,
    ri: usize,
    x: f32,
    y: f32,
    view: &mut glue::TagView,
) -> (Vec<(Rect, Hit)>, Option<Tip>, bool) {
    let game = ctx.game;
    let ru = ctx.ru;
    let cursor = ctx.cursor;
    let mut hits = Vec::new();
    let mut tip = None;
    let Some(tree) = r.tag.and_then(|t| ctx.data.tags.get(t)) else {
        return (hits, tip, false);
    };
    // The display's place: grown towards 90% of the screen's width and 80%
    // of its height over 1.4 s when expanded.
    let small = [x, y, DISPLAY_W, Category::Tag.height()];
    let big = [(batch.width * 0.05).floor(), (batch.height * 0.1).floor(), (batch.width * 0.9).floor(), (batch.height * 0.8).floor()];
    let reduced = ctx.reduced_motion;
    let t = view.toggled.filter(|_| !reduced).map_or(1.0, |at| (at.elapsed().as_millis() as f32 / 1400.0).min(1.0));
    let eased = 1.0 - (1.0 - t).powi(3);
    let p = if view.expanded { eased } else if view.toggled.is_some() { 1.0 - eased } else { 0.0 };
    let rb: Rect = std::array::from_fn(|i| (small[i] + (big[i] - small[i]) * p).round());
    recipe_base(game, batch, ctx.border, rb);
    hits.push((rb, Hit::Area));
    let inner = [rb[0] + 20.0, rb[1] + 6.0, rb[2] - 26.0, rb[3] - 12.0];
    nine(game, batch, th(&sprites::PANEL_SLOT, &sprites::PANEL_SLOT_DARK), inner);
    let v = [inner[0] + 1.0, inner[1] + 1.0, inner[2] - 2.0, inner[3] - 2.0];

    // The tree, padded 16 px, drawn 1 / scale times inside the view.
    let root = rei::TagNode::Reference(tree);
    let (tw, th) = root.tree_size();
    let content = (tw + 32.0, th + 32.0);
    let (lo, hi) = rei::tag_scale_range(content, (v[2], v[3]));
    view.scale = view.scale.clamp(lo, hi);
    let s = 1.0 / view.scale;
    if !view.placed {
        view.offset = ((v[2] - content.0 * s) / 2.0, 0.0);
        view.drawn = s;
        view.placed = true;
    }
    if s != view.drawn {
        // Zoom about the view's middle.
        let mid = ((v[2] / 2.0 - view.offset.0) / view.drawn, (v[3] / 2.0 - view.offset.1) / view.drawn);
        view.offset = (v[2] / 2.0 - mid.0 * s, v[3] / 2.0 - mid.1 * s);
        view.drawn = s;
    }
    view.offset = (rei::tag_offset(view.offset.0, content.0 * s, v[2]), rei::tag_offset(view.offset.1, content.1 * s, v[3]));
    let origin = (v[0] + view.offset.0, v[1] + view.offset.1);
    let in_view = inside(cursor, v);
    // The pointer in the tree's own pixels.
    let local = ((cursor.0 - origin.0) / s, (cursor.1 - origin.1) / s);
    let to_screen = |r: Rect| -> Rect { [origin.0 + r[0] * s, origin.1 + r[1] * s, r[2] * s, r[3] * s] };
    hits.push((v, Hit::TagView(ri)));
    let start = batch.vertices.len();
    let mut slot_hits: Vec<(Rect, Hit)> = Vec::new();
    draw_tag_tree(ctx, batch, root, 16.0, 16.0, in_view.then_some(local), &mut |r, h| slot_hits.push((r, h)), &mut tip);
    // The tree's own pixels were laid out from (0, 0): scale them, then move
    // them into the view.
    batch.scale_since(start, 0.0, 0.0, s, s);
    for vert in &mut batch.vertices[start..] {
        vert.pos[0] += origin.0;
        vert.pos[1] += origin.1;
    }
    batch.clip_since(start, v[0], v[1], v[0] + v[2], v[1] + v[3]);
    for (r, h) in slot_hits {
        let r = to_screen(r);
        // Only the part inside the view takes clicks.
        let x0 = r[0].max(v[0]);
        let y0 = r[1].max(v[1]);
        let x1 = (r[0] + r[2]).min(v[0] + v[2]);
        let y1 = (r[1] + r[3]).min(v[1] + v[3]);
        if x1 > x0 && y1 > y0 {
            hits.push(([x0, y0, x1 - x0, y1 - y0], h));
        }
    }

    // The expand and copy buttons.
    let expand = [rb[0] + 5.0, rb[1] + 6.0, 13.0, 13.0];
    let copy = [rb[0] + 5.0, rb[1] + rb[3] - 6.0 - 13.0, 13.0, 13.0];
    button(game, batch, expand, true, "");
    button(game, batch, copy, true, "");
    let icon = if view.expanded { &sprites::SHRINK } else { &sprites::EXPAND };
    blit(game, batch, icon, expand[0] + 2.0, expand[1] + 2.0, 1.0);
    blit(game, batch, &sprites::CLIPBOARD, copy[0] + 2.0, copy[1] + 2.0, 1.0);
    hits.push((expand, Hit::TagExpand(ri)));
    hits.push((copy, Hit::TagCopy(ri)));
    if inside(cursor, expand) {
        let key = if view.expanded { "expand_view_close" } else { "expand_view" };
        tip = Some(Tip::text(vec![(rei::tr(ru, key).to_owned(), WHITE)]));
    } else if inside(cursor, copy) {
        tip = Some(Tip::text(vec![(rei::tr(ru, "tag_copy").to_owned(), WHITE)]));
    }
    (hits, tip, in_view)
}

/// One subtree of a tag at (x, y) in the tree's own pixels
/// (`TagTreeWidget.render`): its box centred over its children, the white
/// lines between them.
#[allow(clippy::too_many_arguments)]
fn draw_tag_tree(
    ctx: &Context<'_, '_>,
    batch: &mut HudBatch,
    node: rei::TagNode<'_>,
    x: f32,
    y: f32,
    pointer: Option<(f32, f32)>,
    hit: &mut dyn FnMut(Rect, Hit),
    tip: &mut Option<Tip>,
) {
    let game = ctx.game;
    let (tw, _) = node.tree_size();
    let (rw, rh) = node.size();
    let cx = x + (tw / 2.0).floor();
    let rx = cx - (rw / 2.0).floor();
    let over = |r: Rect| pointer.is_some_and(|p| inside(p, r));
    match node {
        rei::TagNode::Reference(t) => {
            horizon_client::ui::blit_sprite(batch, &game.gui.atlas, "gui/sprites/advancements/task_frame_unobtained", rx - 1.0, y - 1.0, 26.0, 26.0, u32::MAX);
            let shown = t.all.get(rei::cycle(t.all.len(), ctx.millis)).and_then(|&id| ctx.data.ingredient(id));
            if let Some(i) = shown {
                item(game, batch, &i.stack, rx + 12.0 - 8.0, y + 4.0);
            }
            if over([rx, y, rw, rh]) {
                *tip = Some(Tip::text(vec![(format!("#{}", t.id), WHITE)]));
            }
        }
        rei::TagNode::Values(values) => {
            let cols = values.len().clamp(1, 4);
            let rows = values.len().div_ceil(cols).max(1);
            nine(game, batch, th(&sprites::PANEL, &sprites::PANEL_DARK), [rx, y, rw, rh]);
            nine(game, batch, th(&sprites::PANEL_SLOT, &sprites::PANEL_SLOT_DARK), [rx + 5.0, y + 5.0, 16.0 * cols as f32 + 2.0, 16.0 * rows as f32 + 2.0]);
            for (k, v) in values.iter().enumerate() {
                let sx = rx + 6.0 + 16.0 * (k % cols) as f32;
                let sy = y + 6.0 + 16.0 * (k / cols) as f32;
                let cell = [sx - 1.0, sy - 1.0, E, E];
                match v {
                    Ok(id) => {
                        let Some(i) = ctx.data.ingredient(*id) else {
                            continue;
                        };
                        item(game, batch, &i.stack, sx, sy);
                        if over(cell) {
                            hover(game, batch, sx, sy);
                            *tip = Some(Tip::stack(i.stack.clone(), Vec::new()));
                        }
                        hit(cell, Hit::Slot(*id));
                    }
                    Err(name) => {
                        let w = width(game, "?");
                        text(game, batch, "?", sx + 8.0 - w / 2.0 + 0.2, sy + 8.0 - 4.5 + 1.0, if dark() { 0xffbb_bbbb } else { DARK_TEXT }, false);
                        if over(cell) {
                            *tip = Some(Tip::text(vec![(name.clone(), WHITE)]));
                        }
                    }
                }
            }
        }
    }
    let children = node.children();
    if children.is_empty() {
        return;
    }
    let sizes: Vec<(f32, f32)> = children.iter().map(|c| c.tree_size()).collect();
    let total = sizes.iter().map(|s| s.0 + 6.0).sum::<f32>() - 6.0;
    let bottom = y + rh;
    let line = 0xffff_ffff;
    let first = cx - (total / 2.0).floor();
    fill(game, batch, [cx, bottom + 1.0, 1.0, 5.0], line);
    let left = first + (sizes[0].0 / 2.0).floor();
    let right = first + total - (sizes[sizes.len() - 1].0 / 2.0).floor();
    fill(game, batch, [left, bottom + 6.0, right - left + 1.0, 1.0], line);
    let mut at = first;
    for (child, size) in children.into_iter().zip(sizes) {
        fill(game, batch, [at + (size.0 / 2.0).floor(), bottom + 7.0, 1.0, 9.0], line);
        draw_tag_tree(ctx, batch, child, at, bottom + 16.0, pointer, hit, tip);
        at += size.0 + 6.0;
    }
}

struct Context<'a, 'g> {
    reduced_motion: bool,
    /// `appearance.recipe_border`: 0 default, 1 lighter, 2 none.
    border: u8,
    game: &'a Game<'g>,
    data: &'a Data,
    ru: bool,
    millis: u64,
    focus: Option<u32>,
    cursor: (f32, f32),
}

impl Context<'_, '_> {
    /// What a slot shows now: the focus when it holds it, else one of its
    /// entries a second, every slot in step.
    fn shown(&self, stacks: &[Stack]) -> Option<Stack> {
        if let Some(f) = self.focus
            && let Some(s) = stacks.iter().find(|s| s.id == f)
        {
            return Some(*s);
        }
        stacks.get(rei::cycle(stacks.len(), self.millis)).copied()
    }
}

/// One slot of a display (`EntryWidget`): its background, the entry, the
/// hover and the tooltip.
#[allow(clippy::too_many_arguments)]
fn slot(ctx: &Context<'_, '_>, batch: &mut HudBatch, hits: &mut Vec<(Rect, Hit)>, input: bool, stacks: &[Stack], p: (f32, f32), background: bool, lines: Vec<(String, u32)>) -> Option<Tip> {
    let game = ctx.game;
    if background {
        blit(game, batch, th(&sprites::SLOT, &sprites::SLOT_DARK), p.0 - 1.0, p.1 - 1.0, 1.0);
    }
    let shown = ctx.shown(stacks)?;
    let stack = ctx.data.stack(shown)?;
    item(game, batch, &stack, p.0, p.1);
    let bounds = [p.0 - 1.0, p.1 - 1.0, E, E];
    hits.push((bounds, Hit::Slot(shown.id)));
    if inside(ctx.cursor, bounds) {
        hover(game, batch, p.0, p.1);
        let tip = Tip::stack(stack, lines);
        // An input with several entries lists them ("Accepts following:").
        let entries: Vec<Stack> = stacks.iter().copied().filter(|s| !s.is_empty()).collect();
        if input && entries.len() > 1 {
            let cells = entries.iter().filter_map(|s| ctx.data.stack(*s)).collect();
            return Some(tip.with(Component::Grid { title: rei::tr(ctx.ru, "accepts").to_owned(), cells, tag: exact_tag(ctx.data, &entries, ctx.ru) }));
        }
        return Some(tip);
    }
    None
}

/// The animated arrow (`ArrowWidget`): fills from the left over `ms`.
fn arrow(game: &Game<'_>, batch: &mut HudBatch, x: f32, y: f32, ms: u64) {
    if ms == 0 {
        blit(game, batch, th(&sprites::ARROW_EMPTY, &sprites::ARROW_EMPTY_DARK), x, y, 1.0);
        return;
    }
    let w = ((millis_now(game) as f64 / (ms as f64 / 24.0)) % 24.0).ceil() as u16;
    blit_part(game, batch, th(&sprites::ARROW_EMPTY, &sprites::ARROW_EMPTY_DARK), (x + f32::from(w), y), (w, 24), (0, 17));
    blit_part(game, batch, th(&sprites::ARROW_FULL, &sprites::ARROW_FULL_DARK), (x, y), (0, w), (0, 17));
}

/// The burning fire (`BurningFireWidget`): burns down over `ms`.
fn fire(game: &Game<'_>, batch: &mut HudBatch, x: f32, y: f32, ms: u64) {
    let h = 14 - ((millis_now(game) as f64 / (ms as f64 / 14.0)) % 14.0).ceil() as u16;
    blit_part(game, batch, th(&sprites::FIRE_UNLIT, &sprites::FIRE_UNLIT_DARK), (x, y), (0, 14), (0, 14 - h));
    blit_part(game, batch, th(&sprites::FIRE_LIT, &sprites::FIRE_LIT_DARK), (x, y + f32::from(14 - h)), (0, 14), (14 - h, 14));
}

/// The clock REI's animations run on.
fn millis_now(_game: &Game<'_>) -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

fn display(ctx: &Context<'_, '_>, batch: &mut HudBatch, category: Category, r: &Recipe, x: f32, y: f32) -> (Vec<(Rect, Hit)>, Option<Tip>) {
    let game = ctx.game;
    let ru = ctx.ru;
    let mut hits = Vec::new();
    let mut tip = None;
    let empty: Vec<Stack> = Vec::new();
    let input = |i: usize| r.inputs.get(i).unwrap_or(&empty);
    let output = r.outputs.first().unwrap_or(&empty);
    let mut add = |batch: &mut HudBatch, hits: &mut Vec<(Rect, Hit)>, input: bool, stacks: &[Stack], px: f32, py: f32, bg: bool, lines: Vec<(String, u32)>| {
        let t = slot(ctx, batch, hits, input, stacks, (x + px, y + py), bg, lines);
        if t.is_some() {
            tip = t;
        }
    };
    let base = |batch: &mut HudBatch| recipe_base(game, batch, ctx.border, [x, y, DISPLAY_W, category.height()]);
    let result_bg = |batch: &mut HudBatch, rx: f32, ry: f32| nine(game, batch, th(&sprites::PANEL_SLOT, &sprites::PANEL_SLOT_DARK), [x + rx, y + ry, 26.0, 26.0]);
    match category {
        Category::Crafting => {
            base(batch);
            blit(game, batch, th(&sprites::ARROW_EMPTY, &sprites::ARROW_EMPTY_DARK), x + 77.0, y + 24.0, 1.0);
            result_bg(batch, 107.0, 20.0);
            for i in 0..9 {
                let (c, row) = ((i % 3) as f32, (i / 3) as f32);
                add(batch, &mut hits, true, input(i), 18.0 + 18.0 * c, 7.0 + 18.0 * row, true, Vec::new());
            }
            add(batch, &mut hits, false, output, 112.0, 25.0, false, Vec::new());
            if r.shapeless {
                let icon = [x + 137.0, y + 5.0, 8.0, 8.0];
                blit(game, batch, &sprites::SHAPELESS, icon[0], icon[1], 0.25);
                if inside(ctx.cursor, icon) {
                    tip = Some(Tip::text(vec![(rei::tr(ru, "shapeless").to_owned(), WHITE)]));
                }
            }
        }
        Category::Smelting | Category::Smoking | Category::Blasting | Category::Campfire => {
            base(batch);
            result_bg(batch, 90.0, 14.0);
            fire(game, batch, x + 35.0, y + 30.0, 10_000);
            let default = match category {
                Category::Smelting => 200,
                Category::Campfire => 600,
                _ => 100,
            };
            let ticks = if r.ticks > 0 { r.ticks } else { default };
            if r.ticks > 0 {
                let secs = rei::decimal(f64::from(r.ticks) / 20.0, ru);
                let t = if category == Category::Campfire {
                    rei::tr(ru, "campfire_time").replacen("%s", &secs, 1)
                } else {
                    let xp = rei::decimal(f64::from(r.experience), ru);
                    rei::tr(ru, "cooking_time_xp").replacen("%s", &xp, 1).replacen("%s", &secs, 1)
                };
                let tw = width(game, &t);
                text(game, batch, &t, x + 145.0 - tw, y + 5.0, if dark() { 0xffbb_bbbb } else { DARK_TEXT }, false);
            }
            arrow(game, batch, x + 58.0, y + 18.0, u64::from(ticks) * 50);
            add(batch, &mut hits, false, output, 95.0, 19.0, false, Vec::new());
            add(batch, &mut hits, true, input(0), 35.0, 11.0, true, Vec::new());
        }
        Category::StoneCutting
        | Category::Stripping
        | Category::Tilling
        | Category::Pathing
        | Category::Waxing
        | Category::WaxScraping
        | Category::Oxidizing
        | Category::OxidationScraping => {
            base(batch);
            blit(game, batch, th(&sprites::ARROW_EMPTY, &sprites::ARROW_EMPTY_DARK), x + 61.0, y + 9.0, 1.0);
            result_bg(batch, 90.0, 5.0);
            add(batch, &mut hits, true, input(0), 38.0, 10.0, true, Vec::new());
            add(batch, &mut hits, false, output, 95.0, 10.0, false, Vec::new());
        }
        Category::Brewing => {
            base(batch);
            blit(game, batch, th(&sprites::BREWING, &sprites::BREWING_DARK), x + 23.0, y + 4.0, 1.0);
            let w = ((millis_now(game) / 250) % 18 + 1).min(18) as u16;
            blit_part(game, batch, th(&sprites::BREWING_FUEL, &sprites::BREWING_FUEL_DARK), (x + 67.0, y + 32.0), (0, w), (0, 4));
            if let Some(blaze) = ctx.data.blaze_powder {
                add(batch, &mut hits, false, &[Stack::one(blaze)], 24.0, 5.0, false, Vec::new());
            }
            let yellow = |key: &str| vec![(rei::tr(ru, key).to_owned(), YELLOW)];
            add(batch, &mut hits, true, input(0), 63.0, 5.0, false, yellow("brewing_input"));
            add(batch, &mut hits, true, input(1), 86.0, 5.0, false, yellow("brewing_reactant"));
            // The bottles fill one after another over six seconds.
            let step = (ctx.millis / 1000) % 6;
            add(batch, &mut hits, false, output, 63.0, 39.0, false, yellow("brewing_result"));
            if step >= 2 {
                add(batch, &mut hits, false, output, 86.0, 46.0, false, yellow("brewing_result"));
            }
            if step >= 4 {
                add(batch, &mut hits, false, output, 109.0, 39.0, false, yellow("brewing_result"));
            }
        }
        Category::Composting => {
            base(batch);
            for i in 0..35 {
                let (c, row) = ((i % 7) as f32, (i / 7) as f32);
                let lines = r
                    .chances
                    .get(i)
                    .map(|ch| {
                        let percent = ((ch * 100.0).floor() as i32).clamp(0, 100);
                        vec![(rei::tr(ru, "composting_chance").replace("%d", &percent.to_string()).replace("%%", "%"), YELLOW)]
                    })
                    .unwrap_or_default();
                add(batch, &mut hits, true, input(i), 12.0 + 18.0 * c, 12.0 + 18.0 * row, true, lines);
            }
            blit(game, batch, th(&sprites::ARROW_EMPTY, &sprites::ARROW_EMPTY_DARK), x + 89.0, y + 112.0, 1.0);
            result_bg(batch, 118.0, 108.0);
            add(batch, &mut hits, false, output, 123.0, 113.0, false, Vec::new());
        }
        Category::Smithing => {
            base(batch);
            blit(game, batch, th(&sprites::ARROW_EMPTY, &sprites::ARROW_EMPTY_DARK), x + 76.0, y + 9.0, 1.0);
            result_bg(batch, 105.0, 5.0);
            add(batch, &mut hits, true, input(0), 17.0, 10.0, true, Vec::new());
            add(batch, &mut hits, true, input(1), 35.0, 10.0, true, Vec::new());
            add(batch, &mut hits, true, input(2), 53.0, 10.0, true, Vec::new());
            add(batch, &mut hits, false, output, 110.0, 10.0, false, Vec::new());
        }
        Category::Anvil => {
            base(batch);
            let sy = 4.0;
            blit(game, batch, th(&sprites::ARROW_EMPTY, &sprites::ARROW_EMPTY_DARK), x + 71.0, y + sy + 4.0, 1.0);
            result_bg(batch, 100.0, sy);
            add(batch, &mut hits, true, input(0), 26.0, sy + 5.0, true, Vec::new());
            add(batch, &mut hits, true, input(1), 48.0, sy + 5.0, true, Vec::new());
            add(batch, &mut hits, false, output, 105.0, sy + 5.0, false, Vec::new());
            if r.cost > 0 {
                let template = game.translate("container.repair.cost", "Enchantment Cost: %1$s");
                let t = template.replace("%1$s", &r.cost.to_string()).replace("%s", &r.cost.to_string());
                let tw = width(game, &t);
                let tx = x + 146.0 - tw - 2.0;
                fill(game, batch, [tx - 2.0, y + sy + 28.0, x + 146.0 - (tx - 2.0), 12.0], 0x4f00_0000);
                text(game, batch, &t, tx, y + sy + 30.0, 0xff80_ff20, true);
            }
        }
        // Drawn by `tag_display`.
        Category::Tag => {}
        Category::BeaconBase | Category::BeaconPayment => {
            if let Some(beacon) = ctx.data.icons.get(&category) {
                add(batch, &mut hits, false, &[Stack::one(*beacon)], 67.0, 3.0, true, Vec::new());
            }
            nine(game, batch, th(&sprites::PANEL_SLOT, &sprites::PANEL_SLOT_DARK), [x - 1.0, y + 23.0, 152.0, 112.0]);
            for (i, cell) in r.inputs.iter().enumerate().take(48) {
                let (c, row) = ((i % 8) as f32, (i / 8) as f32);
                add(batch, &mut hits, true, cell, 1.0 + 18.0 * c, 25.0 + 18.0 * row, false, Vec::new());
            }
        }
    }
    (hits, tip)
}
