// Smooth Swapping for Horizon: the mod's config screen (`ConfigScreen` of its
// 26.3 branch) over the Options screen while the mod's page is open: Toggle
// Mod, the speed slider, the curve editor (`CatmullRomWidget`), the test
// inventory (`InventoryWidget`, whose stacks fly along the curve being
// edited), Reset, Save & Exit and Exit, with the original's numbers in the
// game's widgets. Mod Menu's Configure opens it (`ModmenuIntegration`), and
// so does the page's first button. Save & Exit keeps everything; Exit and
// Escape put the speed and the curve back (`onClose`), while Toggle Mod
// stays as clicked, unsaved, until the game closes, as in the original. Also
// writes the icon and Mod Menu's metadata into the mod's data folder.
//
// The screen hooks only read the game: what needs to change it (a click's
// sound, closing) waits for the next frame.
use super::smoothswapping as ss;
use super::{Game, Screen};
use horizon_client::gamepad;
use horizon_client::hud::{HudBatch, rgba};
use horizon_client::items::ItemStack;
use horizon_client::mod_api::NativeScreenView;
use horizon_client::options_ui::Page;
use horizon_client::text::{self, Text};
use horizon_client::ui;
use std::cell::RefCell;
use std::time::Instant;

/// Mod Menu's Configure: `native_settings_click(id, MODMENU_CONFIGURE)` after it opened the mod's page.
pub const MODMENU_CONFIGURE: usize = 0x4D4D_4346;

/// GLFW's Shift bit.
const SHIFT: u32 = 1;

/// `new CatmullRomWidget(x, y, 64, 64, 12, 4, 4, points)`: the grid, the
/// border round it and the grid's lines each way.
const GRID: i32 = 64;
const BORDER: i32 = 12;
const LINES: i32 = 4;
const CURVE_SIZE: f32 = (GRID + 2 * BORDER) as f32;

/// `new InventoryWidget(x, y, 3, 4, title)` on `generic_54.png`: its size is
/// `columns * 18 + 2 * 7` by `rows * 18 + 17 + 7 + 14`.
const COLUMNS: usize = 3;
const ROWS: usize = 4;
const SLOTS: usize = COLUMNS * ROWS;
const INV_W: f32 = (COLUMNS * 18 + 14) as f32;
const INV_H: f32 = (ROWS * 18 + 17 + 7 + 14) as f32;
const TEXTURE: &str = "gui/container/generic_54";

/// `CMRTooltipState.setDelay(Duration.ofMillis(1000))`.
const TOOLTIP_DELAY_MS: u128 = 1000;

/// Who opened the screen: the page's button, or Mod Menu's Configure.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Page,
    ModMenu,
}

/// The screen's widgets, in `init`'s order (clicks and Tab go in this order).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum W {
    Toggle,
    Speed,
    Curve,
    Inventory,
    Reset,
    Save,
    Exit,
}

const WIDGETS: [W; 7] = [W::Toggle, W::Speed, W::Curve, W::Inventory, W::Reset, W::Save, W::Exit];

impl W {
    /// `ConfigScreen.init`'s bounds on a `w` × `h` screen (Java's int halves,
    /// fifths and thirds).
    fn rect(self, (w, h): (f32, f32)) -> [f32; 4] {
        let cx = (w as i32 / 2) as f32;
        let fifth = (h as i32 / 5) as f32;
        let third = (h as i32 / 3) as f32;
        match self {
            W::Toggle => [cx - 94.0, fifth - 20.0, 188.0, 20.0],
            W::Speed => [cx - 94.0, fifth + 5.0, 188.0, 20.0],
            W::Curve => [cx - 84.0 - 10.0, third, CURVE_SIZE, CURVE_SIZE],
            W::Inventory => [cx + 10.0, third, INV_W, INV_H],
            W::Reset => [cx - 84.0 - 10.0, third + 86.0 + 4.0, 88.0, 20.0],
            W::Save => [cx + 10.0, h - 30.0, 88.0, 20.0],
            W::Exit => [cx - 84.0 - 10.0, h - 30.0, 88.0, 20.0],
        }
    }
}

/// A stack flying in the test inventory (`ItemToItemInventorySwap`).
struct Flight {
    from: usize,
    to: usize,
    /// The stack, with the count that flies.
    stack: ItemStack,
    progress: f64,
}

/// The open screen.
struct Open {
    mode: Mode,
    /// `oldAnimationSpeed`, `oldPoints`: what Exit puts back.
    old_speed: u32,
    old_points: Vec<ss::Point>,
    /// The config as the screen changes it (`ConfigManager.getConfig()`,
    /// changed live by the widgets).
    work: ss::Settings,
    /// Save & Exit (true), Exit or Escape (false): the screen goes at the next frame.
    close: Option<bool>,
    /// The focused widget (`setFocused`), and whether the keyboard focused it.
    focus: Option<W>,
    keyboard: bool,
    /// The slider's own value (`AbstractSliderButton.value`), its
    /// `canChangeValue`, and whether the left button holds it.
    slider: f64,
    can_change: bool,
    slider_held: bool,
    /// The left button went down on the curve widget (focused and dragging).
    curve_held: bool,
    /// `hoveredPointIndex` at the last drawing: an index into `work.points`.
    hovered: Option<usize>,
    /// When the pointer came over the curve widget (`renderCheckTime`).
    tip_since: Option<Instant>,
    /// The test inventory: its slots, the stack on the pointer, the slot
    /// under the pointer at the last drawing (`focusedSlot`) and the flights.
    slots: Vec<Option<ItemStack>>,
    carried: Option<ItemStack>,
    focused_slot: Option<usize>,
    flights: Vec<Flight>,
    last_draw: Instant,
}

#[derive(Default)]
struct State {
    open: Option<Open>,
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

/// `f` on the open screen; None when it is closed.
fn with_open<R>(f: impl FnOnce(&mut Open) -> R) -> Option<R> {
    with(|s| s.open.as_mut().map(f))
}

fn russian() -> bool {
    ss::russian(&horizon_client::lang::code())
}

fn settings(game: &Game<'_>) -> ss::Settings {
    ss::settings(|key| game.native_setting(ss::ID, key))
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

/// The game's colour from 0xAARRGGBB.
fn argb(c: u32) -> u32 {
    let [a, r, g, b] = c.to_be_bytes();
    rgba(r, g, b, a)
}

/// Vanilla's `fill(x0, y0, x1, y1, colour)`.
fn fill(game: &Game<'_>, batch: &mut HudBatch, (x0, y0): (i32, i32), (x1, y1): (i32, i32), color: u32) {
    if x1 > x0 && y1 > y0 {
        batch.fill(&game.gui.atlas, x0 as f32, y0 as f32, (x1 - x0) as f32, (y1 - y0) as f32, argb(color));
    }
}

/// A `u, v, w, h` region of `generic_54.png` (256 × 256) drawn at `x, y`
/// (`blit(..., 256, 256)`).
#[allow(clippy::too_many_arguments)]
fn blit(game: &Game<'_>, batch: &mut HudBatch, x: f32, y: f32, u: f32, v: f32, w: f32, h: f32) {
    if let Some(mut r) = game.gui.atlas.sprites.get(TEXTURE).copied() {
        let (du, dv) = ((r.u1 - r.u0) / r.w as f32, (r.v1 - r.v0) / r.h as f32);
        let (u0, v0) = (r.u0, r.v0);
        r.u0 = u0 + u * du;
        r.u1 = u0 + (u + w) * du;
        r.v0 = v0 + v * dv;
        r.v1 = v0 + (v + h) * dv;
        batch.rect_uv(x, y, w, h, r, u32::MAX);
    }
}

// ---- opening and closing ---------------------------------------------------------------

/// Smooth Swapping's page is the one the Options screen shows: its index in Options > Mods.
fn page(game: &Game<'_>) -> Option<usize> {
    if game.screen != Screen::Options {
        return None;
    }
    match game.ui.options.page() {
        Page::NativeMod(i) if game.settings.mods.installed.get(i).is_some_and(|m| m.id == ss::ID) => Some(i),
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

/// `IntRange(1, 50).xmap(v * 10, v / 10)`: the slider's value for a speed
/// (`toSliderValue`).
fn to_slider(speed: u32) -> f64 {
    let v = f64::from(speed / 10);
    if v <= 1.0 {
        0.0
    } else if v >= 50.0 {
        1.0
    } else {
        (v + 0.5 - 1.0) / 50.0
    }
}

/// The speed at slider value `p` (`fromSliderValue`).
fn from_slider(p: f64) -> u32 {
    let p = if p >= 1.0 { f64::from(0.99999_f32) } else { p.max(0.0) };
    ((1.0 + p * 50.0).floor() as u32).clamp(1, 50) * 10
}

/// Opens the screen over the mod's page (`settings_click`): the page's
/// first button, or Mod Menu's Configure. A new screen: the test inventory
/// starts with its 32 cobblestone again.
pub(super) fn open(game: &Game<'_>, from_mod_menu: bool) {
    if page(game).is_none() {
        return;
    }
    let work = settings(game);
    let mut slots = vec![None; SLOTS];
    slots[0] = Some(ItemStack::new("minecraft:cobblestone", 32));
    let open = Open {
        mode: if from_mod_menu { Mode::ModMenu } else { Mode::Page },
        old_speed: work.speed,
        old_points: work.points.clone(),
        slider: to_slider(work.speed),
        work,
        close: None,
        focus: None,
        keyboard: false,
        can_change: false,
        slider_held: false,
        curve_held: false,
        hovered: None,
        tip_since: None,
        slots,
        carried: None,
        focused_slot: None,
        flights: Vec::new(),
        last_draw: Instant::now(),
    };
    with(|s| {
        s.open = Some(open);
        s.taken = 0;
    });
}

/// The screen goes: Save & Exit keeps the config (`ConfigManager.save`);
/// otherwise (`onClose`) the speed and the curve go back, and Toggle Mod
/// stays as it was clicked for this run only.
fn finish(game: &Game<'_>, save: bool) {
    let Some(o) = with(|s| s.open.take()) else {
        return;
    };
    let mut kept = settings(game);
    kept.enabled = o.work.enabled;
    if save {
        kept.speed = o.work.speed;
        kept.points = o.work.points;
        for (key, value) in kept.stored() {
            game.native_set_setting(ss::ID, key, &value);
        }
    } else {
        kept.speed = o.old_speed;
        kept.points = o.old_points;
    }
    ss::set_settings(kept);
}

/// Once a frame: the data files, the clicks' sound and the screen's closing.
pub(super) fn frame(game: &mut Game<'_>) {
    if !with(|s| std::mem::replace(&mut s.written, true)) {
        write_data(game);
    }
    for _ in 0..with(|s| std::mem::take(&mut s.clicks)) {
        game.audio.play_button_click();
    }
    let Some((mode, close)) = with_open(|o| (o.mode, o.close)) else {
        return;
    };
    match page(game) {
        // The options went elsewhere some other way: the screen goes, unsaved.
        None => finish(game, false),
        Some(i) if close.is_some() => {
            finish(game, close == Some(true));
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

// ---- drawing ---------------------------------------------------------------------------

/// Toggle Mod's label: `CycleButton`'s "Toggle Mod: Enabled", the value
/// green or red.
fn toggle_label(on: bool, ru: bool) -> Text {
    let (caption, value, color) = match (on, ru) {
        (true, true) => ("Мод", "Включён", text::GREEN),
        (false, true) => ("Мод", "Выключен", text::RED),
        (true, false) => ("Toggle Mod", "Enabled", text::GREEN),
        (false, false) => ("Toggle Mod", "Disabled", text::RED),
    };
    Text::option_name_value(Text::literal(caption), Text::colored(value, color))
}

/// The slider's label: "Speed: 100%".
fn speed_label(speed: u32, ru: bool) -> Text {
    Text::literal(format!("{}: {speed}%", if ru { "Скорость" } else { "Speed" }))
}

fn button_text(w: W, ru: bool) -> &'static str {
    match (w, ru) {
        (W::Reset, true) => "Сброс",
        (W::Reset, false) => "Reset",
        (W::Save, true) => "Готово",
        (W::Save, false) => "Save & Exit",
        (W::Exit, true) => "Отмена",
        _ => "Exit",
    }
}

/// `smoothswapping.config.option.animationspeed.tooltip`.
fn tooltip_text(ru: bool) -> &'static str {
    if ru {
        "-ПКМ по пустому месту — новая точка кривой\n-Зажмите ЛКМ на точке и тащите, чтобы сдвинуть её\n-ПКМ по точке — удалить её"
    } else {
        "-Right click on empty space to create new curve point\n-Left click and drag a point to move it\n-Right click one to delete it"
    }
}

/// Draws the screen over the Options screen (`screen_render_post`): its
/// background, the widgets in order, the title, then the curve's tooltip.
pub(super) fn render(game: &Game<'_>, batch: &mut HudBatch, screen: &NativeScreenView<'_>) {
    if !over(game, screen) {
        return;
    }
    let (w, h) = (batch.width, batch.height);
    game.native_apply_screen_blur();
    ui::menu_background(batch, &game.gui.atlas, 0.0, 0.0, w, h, game.ui.options.in_world);
    let ru = russian();
    let mouse = (screen.mouse[0], screen.mouse[1]);
    with_open(|o| draw(game, batch, o, (w, h), mouse, ru));
}

fn draw(game: &Game<'_>, batch: &mut HudBatch, o: &mut Open, size: (f32, f32), mouse: (f32, f32), ru: bool) {
    let atlas = &game.gui.atlas;
    // The flights move on (`renderSwap`: a tenth of the way a tick at 100%).
    let now = Instant::now();
    let dt = (now.duration_since(o.last_draw).as_secs_f64() * 1000.0).min(100.0);
    o.last_draw = now;
    let rate = o.work.rate_per_ms();
    for f in &mut o.flights {
        f.progress += dt * rate;
    }
    o.flights.retain(|f| f.progress < 1.0);

    let r = W::Toggle.rect(size);
    let hover = inside(mouse, r) || o.focus == Some(W::Toggle);
    ui::button(batch, atlas, r[0], r[1], r[2], r[3], &toggle_label(o.work.enabled, ru), true, hover);

    let r = W::Speed.rect(size);
    let focused = o.focus == Some(W::Speed);
    ui::slider_ext(
        batch,
        atlas,
        r[0],
        r[1],
        r[2],
        &speed_label(o.work.speed, ru),
        o.slider as f32,
        inside(mouse, r),
        focused,
        o.can_change && focused,
        true,
        o.slider_held,
    );

    draw_curve(game, batch, o, W::Curve.rect(size), mouse);
    draw_inventory(game, batch, o, W::Inventory.rect(size), mouse);

    for wd in [W::Reset, W::Save, W::Exit] {
        let r = wd.rect(size);
        let hover = inside(mouse, r) || o.focus == Some(wd);
        let label = Text::literal(button_text(wd, ru));
        let action = match wd {
            W::Save => Some(horizon_client::widgets::Action::Done),
            W::Exit => Some(horizon_client::widgets::Action::Cancel),
            _ => None,
        };
        match action {
            Some(a) => ui::action_button(batch, atlas, a, r[0], r[1], r[2], r[3], &label, true, hover),
            None => ui::button(batch, atlas, r[0], r[1], r[2], r[3], &label, true, hover),
        }
    }

    // `context.centeredText(font, title, width / 2, 10, 0xFFFFFFFF)`.
    let title = Text::literal(if ru { "Настройки Smooth Swapping" } else { "Smooth Swapping Config Menu" });
    let tw = atlas.font.width_text(&title);
    batch.text_styled(atlas, &title, (size.0 as i32 / 2 - tw as i32 / 2) as f32, 10.0, u32::MAX, true);

    // `CMRTooltipState`: a second after the pointer came over the curve
    // widget (or the keyboard focused it), its tooltip, placed by
    // `CMRTooltipPosition`.
    let curve = W::Curve.rect(size);
    let shown = inside(mouse, curve) || (o.focus == Some(W::Curve) && o.keyboard);
    match (shown, o.tip_since) {
        (true, None) => o.tip_since = Some(now),
        (false, _) => o.tip_since = None,
        _ => {}
    }
    if o.tip_since.is_some_and(|t| now.duration_since(t).as_millis() > TOOLTIP_DELAY_MS) {
        let lines = atlas.font.split(&Text::literal(tooltip_text(ru)), 170.0);
        let tw = lines.iter().map(|l| atlas.font.width_text(l)).fold(0.0, f32::max) as i32;
        let th = if lines.len() == 1 { 8 } else { 10 * lines.len() as i32 };
        let (mx, my) = (mouse.0.floor() as i32, mouse.1.floor() as i32);
        let (mut x, mut y) = (mx + 10, my - th);
        if y + th > size.1 as i32 {
            y = curve[1] as i32 - th - 1;
        }
        if x + tw > size.0 as i32 {
            x = (curve[0] as i32 + curve[2] as i32 - tw - 10).max(4);
        }
        // `ui::tooltip` puts the lines 12 right of and 12 above its point.
        ui::tooltip(batch, atlas, &lines, ((x - 12) as f32, (y + 12) as f32));
    }
}

/// A point of the curve on the screen, as the widget draws it (`(int)` of
/// the double, then one right and one up).
fn curve_pixel(r: [f32; 4], (px, py): ss::Point) -> (i32, i32) {
    let (x, y) = (f64::from(r[0] as i32), f64::from(r[1] as i32));
    let xc = (x + f64::from(BORDER) + px * f64::from(GRID)) as i32 + 1;
    let yc = (y + f64::from(BORDER + GRID) - py * f64::from(GRID)) as i32 - 1;
    (xc, yc)
}

/// `CatmullRomWidget.extractWidgetRenderState`: the black box, the grid's
/// faint lines and its two white axes, the spline a red pixel at every
/// 0.005 of each segment, then the points (yellow under the pointer).
fn draw_curve(game: &Game<'_>, batch: &mut HudBatch, o: &mut Open, r: [f32; 4], mouse: (f32, f32)) {
    o.work.points.sort_by(|a, b| a.0.total_cmp(&b.0));
    let (x, y) = (r[0] as i32, r[1] as i32);
    let size = CURVE_SIZE as i32;
    fill(game, batch, (x, y), (x + size, y + size), 0xFA00_0000);
    // `verticalLine(x, y1, y2)` fills y1 + 1 to y2; `horizontalLine(x1, x2, y)` x1 to x2 + 1.
    let step = GRID / LINES;
    for i in 0..LINES {
        let lx = x + BORDER + step + i * step;
        fill(game, batch, (lx, y + BORDER + 1), (lx + 1, y + BORDER + GRID), 0x10FF_FFFF);
    }
    for i in 0..LINES {
        let ly = y + BORDER + 1 + i * step;
        fill(game, batch, (x + BORDER, ly), (x + BORDER + GRID + 1, ly + 1), 0x10FF_FFFF);
    }
    fill(game, batch, (x + BORDER, y + BORDER + 1), (x + BORDER + 1, y + BORDER + GRID), 0xFFFF_FFFF);
    fill(game, batch, (x + BORDER, y + BORDER + GRID), (x + BORDER + GRID + 1, y + BORDER + GRID + 1), 0xFFFF_FFFF);

    let p = ss::with_ends(&o.work.points);
    for i in 1..p.len() - 2 {
        let seg = [p[i - 1], p[i], p[i + 1], p[i + 2]];
        let mut t = 0.0_f32;
        while t < 1.0 {
            let (xc, yc) = curve_pixel(r, ss::spline_point(seg, f64::from(t)));
            fill(game, batch, (xc, yc), (xc + 1, yc + 1), 0xFFFF_0000);
            t += 0.005;
        }
    }

    o.hovered = hovered_point(o, r, (mouse.0.floor(), mouse.1.floor()));
    for (i, &point) in p.iter().enumerate().take(p.len() - 1).skip(1) {
        let (xc, yc) = curve_pixel(r, point);
        let hovered = i >= 2 && o.hovered == Some(i - 2);
        let color = if hovered { 0xFFFF_FF00 } else { 0xFFC9_08FF };
        fill(game, batch, (xc - 2, yc - 2), (xc + 2, yc + 2), color);
    }
}

/// `hoveredPointIndex`: the first of the curve's own points whose 4 × 4
/// box holds the pointer, while the pointer is over the widget.
fn hovered_point(o: &Open, r: [f32; 4], m: (f32, f32)) -> Option<usize> {
    if !inside(m, r) {
        return None;
    }
    let (x, y) = (f64::from(r[0] as i32), f64::from(r[1] as i32));
    o.work.points.iter().position(|&(px, py)| {
        let xc = (x + f64::from(BORDER) + px * f64::from(GRID)) as i32 - 1;
        let yc = (y + f64::from(BORDER + GRID) - py * f64::from(GRID)) as i32 - 4;
        let (mx, my) = (m.0 as i32, m.1 as i32);
        mx >= xc && mx < xc + 4 && my >= yc && my < yc + 4
    })
}

/// The slot `k`'s top left (`Slot.x`, `Slot.y`): the last row 14 lower,
/// under the splitter.
fn slot_pos(r: [f32; 4], k: usize) -> (f32, f32) {
    let (row, col) = (k / COLUMNS, k % COLUMNS);
    let gap = if row == ROWS - 1 { 14.0 } else { 0.0 };
    (r[0] + 8.0 + col as f32 * 18.0, r[1] + 18.0 + row as f32 * 18.0 + gap)
}

/// `isPointOverSlot`: 18 × 18 from the slot's corner.
fn over_slot(r: [f32; 4], k: usize, m: (f32, f32)) -> bool {
    let (x, y) = slot_pos(r, k);
    m.0 >= x && m.0 < x + 18.0 && m.1 >= y && m.1 < y + 18.0
}

/// `InventoryWidget.extractWidgetRenderState`: the frame and the slots cut
/// from `generic_54.png`, then each slot (the one under the pointer over
/// `slot_highlight_back`), a flying stack where it is headed, and the stack
/// on the pointer.
fn draw_inventory(game: &Game<'_>, batch: &mut HudBatch, o: &mut Open, r: [f32; 4], mouse: (f32, f32)) {
    let (x, y) = (r[0], r[1]);
    blit(game, batch, x, y, 0.0, 0.0, 7.0, INV_H - 7.0);
    blit(game, batch, x, y + INV_H - 7.0, 0.0, 222.0 - 7.0, 7.0, 7.0);
    blit(game, batch, x + 7.0, y, 7.0, 0.0, INV_W - 14.0, 17.0);
    blit(game, batch, x + INV_W - 7.0, y, 176.0 - 7.0, 0.0, 7.0, INV_H - 7.0);
    blit(game, batch, x + INV_W - 7.0, y + INV_H - 7.0, 176.0 - 7.0, 222.0 - 7.0, 7.0, 7.0);
    blit(game, batch, x + 7.0, y + INV_H - 7.0, 7.0, 222.0 - 7.0, INV_W - 14.0, 7.0);
    blit(game, batch, x + 7.0, y + 17.0 + (ROWS - 1) as f32 * 18.0, 7.0, 125.0, INV_W - 14.0, 14.0);
    for k in 0..SLOTS {
        let (sx, sy) = slot_pos(r, k);
        blit(game, batch, sx - 1.0, sy - 1.0, 7.0, 17.0, 18.0, 18.0);
    }
    // Its title ("Test Inventory", cut to fit) is drawn with
    // `text(..., 4210752, false)`: a colour with no alpha, which 26.3 draws
    // nothing for.

    let m = (mouse.0.floor(), mouse.1.floor());
    for k in 0..SLOTS {
        let (sx, sy) = slot_pos(r, k);
        if over_slot(r, k, m) {
            o.focused_slot = Some(k);
            ui::blit_sprite(batch, &game.gui.atlas, "gui/sprites/container/slot_highlight_back", sx - 4.0, sy - 4.0, 24.0, 24.0, u32::MAX);
        }
        // The destination of a flight shows the flying stack instead
        // (`doSwap`, `doOverlayRender`), where the curve has it now.
        if let Some(f) = o.flights.iter().find(|f| f.to == k) {
            let from = slot_pos(r, f.from);
            let [fx, fy] = ss::position([from.0, from.1], [sx, sy], f.progress, &o.work.points);
            game.draw_item(batch, &f.stack, fx, fy);
        } else if let Some(stack) = &o.slots[k] {
            game.draw_item(batch, stack, sx, sy);
        }
    }
    if let Some(stack) = &o.carried {
        game.draw_item(batch, stack, m.0 - 8.0, m.1 - 8.0);
    }
}

// ---- input -----------------------------------------------------------------------------

/// The curve widget's point for the pointer (`getPointX`, `getPointY`).
fn curve_point(r: [f32; 4], m: (f32, f32)) -> ss::Point {
    let (x, y) = (f64::from(r[0] as i32), f64::from(r[1] as i32));
    let (mx, my) = (f64::from(m.0), f64::from(m.1));
    (
        (-f64::from(BORDER) + mx - x - 1.0) / f64::from(GRID),
        (f64::from(BORDER) - my + f64::from(GRID) + y - 1.0) / f64::from(GRID),
    )
}

/// `isMouseInGrid`, or with `extended` `isMouseInGridYExtended` (the
/// whole widget's height).
fn in_grid(r: [f32; 4], m: (f32, f32), extended: bool) -> bool {
    let (x, y) = (r[0] as i32 as f32, r[1] as i32 as f32);
    let (b, g) = (BORDER as f32, GRID as f32);
    let (top, bottom) = if extended { (y + 1.0, y + CURVE_SIZE - 1.0) } else { (y + b + 1.0, y + b + g - 1.0) };
    m.0 > x + b + 1.0 && m.0 < x + b + g - 1.0 && m.1 > top && m.1 < bottom
}

/// `InventoryWidget.mouseClicked` over the slot the pointer was last drawn
/// over: with Shift its stack flies to the first empty slot of the other
/// part (`moveItems`), else it is taken up or put down. True when something
/// happened.
fn inventory_click(o: &mut Open, r: [f32; 4], m: (f32, f32), shift: bool) -> bool {
    let Some(k) = o.focused_slot.filter(|&k| over_slot(r, k, m)) else {
        return false;
    };
    if shift {
        let targets: Vec<usize> = if k >= SLOTS - COLUMNS {
            (0..SLOTS - COLUMNS).collect()
        } else {
            (SLOTS - COLUMNS..SLOTS).rev().collect()
        };
        for to in targets {
            if o.slots[to].is_none() {
                o.flights.retain(|f| f.to != k);
                if let Some(stack) = o.slots[k].take() {
                    o.slots[to] = Some(stack.clone());
                    o.flights.push(Flight { from: k, to, stack, progress: 0.0 });
                }
                return true;
            }
        }
        false
    } else if o.carried.is_none() && o.slots[k].is_some() {
        o.flights.retain(|f| f.to != k);
        o.carried = o.slots[k].take();
        true
    } else if o.carried.is_some() && o.slots[k].is_none() {
        o.slots[k] = o.carried.take();
        true
    } else {
        false
    }
}

/// The focus moves to `w` (`setFocused`): a slider focused by the mouse or
/// Tab can change its value at once, by the arrows not.
fn set_focus(o: &mut Open, w: W, keyboard: bool, can_change: bool) {
    o.focus = Some(w);
    o.keyboard = keyboard;
    o.can_change = w == W::Speed && can_change;
}

/// A button pressed (a click, Enter or Space): its sound, then its action.
fn press_button(o: &mut Open, w: W, clicks: &mut u32) {
    *clicks += 1;
    match w {
        // `CycleButton` of two values: either way flips it.
        W::Toggle => o.work.enabled = !o.work.enabled,
        // `catmullRomWidget.reset()`.
        W::Reset => {
            o.work.points.clear();
            o.hovered = None;
        }
        W::Save => o.close = Some(true),
        W::Exit => o.close = Some(false),
        W::Speed | W::Curve | W::Inventory => {}
    }
}

/// The slider's value set (`setValue`): the speed follows when it moved.
fn set_slider(o: &mut Open, v: f64) {
    let v = v.clamp(0.0, 1.0);
    if v != o.slider {
        o.slider = v;
        o.work.speed = from_slider(v);
    }
}

fn bit(button: i32) -> u8 {
    1 << button.clamp(0, 7)
}

/// A mouse button pressed over the screen: the first widget, in `init`'s
/// order, that takes it (`ContainerEventHandler.mouseClicked`). True when
/// the screen took it.
pub(super) fn pressed(game: &Game<'_>, screen: &NativeScreenView<'_>, button: i32, modifiers: u32) -> bool {
    if !over(game, screen) {
        return false;
    }
    let m = (screen.mouse[0], screen.mouse[1]);
    let size = gui_size(game);
    let shift = modifiers & SHIFT != 0;
    with(|s| {
        s.taken |= bit(button);
        let clicks = &mut s.clicks;
        let Some(o) = s.open.as_mut() else {
            return;
        };
        for w in WIDGETS {
            let r = w.rect(size);
            let took = match w {
                // Buttons and the slider take the left button alone.
                W::Toggle | W::Speed | W::Reset | W::Save | W::Exit => button == 0 && inside(m, r),
                // Any button over it (`isMouseOver`); the right one (the
                // tooltip's, 26.3 asks for button 3, the back button: both
                // work) removes the point under the pointer or adds one.
                W::Curve => {
                    // The point under the pointer as the last drawing found
                    // it, found again for where the press is (the same for
                    // a pointer that moved there over frames).
                    o.hovered = hovered_point(o, r, (m.0.floor(), m.1.floor()));
                    if (button == 1 || button == 3) && inside(m, r) {
                        if let Some(k) = o.hovered.take() {
                            if k < o.work.points.len() {
                                o.work.points.remove(k);
                            }
                        } else if in_grid(r, m, false) {
                            o.work.points.push(curve_point(r, m));
                        }
                    }
                    inside(m, r)
                }
                // 26.3 reacts to the right button (1.21 to the left): both work.
                W::Inventory => {
                    let p = (m.0.floor(), m.1.floor());
                    if let Some(k) = (0..SLOTS).find(|&k| over_slot(r, k, p)) {
                        o.focused_slot = Some(k);
                    }
                    (button == 0 || button == 1) && inventory_click(o, r, m, shift)
                }
            };
            if !took {
                continue;
            }
            set_focus(o, w, false, true);
            match w {
                W::Speed => {
                    // No sound on the press: the release has it.
                    o.slider_held = true;
                    set_slider(o, f64::from(ui::slider_value(r[0], r[2], m.0)));
                }
                W::Curve => o.curve_held = button == 0,
                W::Inventory => {}
                _ => press_button(o, w, clicks),
            }
            return;
        }
    });
    true
}

/// A mouse button let go: the held slider sounds and settles on its value
/// (`onRelease`, `resetValue`). Taken when its press was.
pub(super) fn released(button: i32) -> bool {
    with(|s| {
        let was = s.taken & bit(button) != 0;
        s.taken &= !bit(button);
        if button == 0
            && let Some(o) = s.open.as_mut()
        {
            if o.slider_held {
                o.slider_held = false;
                o.slider = to_slider(o.work.speed);
                s.clicks += 1;
            }
            o.curve_held = false;
        }
        was
    })
}

/// The pointer dragged with the left button: the held slider follows it, or
/// the curve's point under it (`onDrag`). Nothing under the screen hears it.
pub(super) fn dragged(game: &Game<'_>, screen: &NativeScreenView<'_>, button: i32) -> bool {
    if !over(game, screen) {
        return false;
    }
    if button == 0 {
        let m = (screen.mouse[0], screen.mouse[1]);
        let size = gui_size(game);
        with_open(|o| {
            if o.slider_held {
                let r = W::Speed.rect(size);
                set_slider(o, f64::from(ui::slider_value(r[0], r[2], m.0)));
            } else if o.curve_held
                && let Some(k) = o.hovered.filter(|&k| k < o.work.points.len())
            {
                let r = W::Curve.rect(size);
                if in_grid(r, m, true) {
                    o.work.points[k] = curve_point(r, m);
                }
            }
        });
    }
    true
}

/// The wheel: Toggle Mod under the pointer flips (`CycleButton.mouseScrolled`),
/// with no sound.
pub(super) fn scrolled(game: &Game<'_>, screen: &NativeScreenView<'_>, scroll: [f64; 2]) -> bool {
    if !over(game, screen) {
        return false;
    }
    let m = (screen.mouse[0], screen.mouse[1]);
    let size = gui_size(game);
    if scroll[1] != 0.0 && inside(m, W::Toggle.rect(size)) {
        with_open(|o| o.work.enabled = !o.work.enabled);
    }
    true
}

/// A key: Escape closes (`onClose`), Tab and the arrows move the focus (the
/// arrows step a slider that can change), Enter and Space press the focused
/// button or let the slider change. Every key is the screen's.
pub(super) fn key_pressed(game: &Game<'_>, screen: &NativeScreenView<'_>, key: &str, modifiers: u32) -> bool {
    if !over(game, screen) {
        return false;
    }
    let shift = modifiers & SHIFT != 0;
    let size = gui_size(game);
    let arrow = match key {
        "key.keyboard.left" => Some(gamepad::Button::DPadLeft),
        "key.keyboard.right" => Some(gamepad::Button::DPadRight),
        "key.keyboard.up" => Some(gamepad::Button::DPadUp),
        "key.keyboard.down" => Some(gamepad::Button::DPadDown),
        _ => None,
    };
    with(|s| {
        let clicks = &mut s.clicks;
        let Some(o) = s.open.as_mut() else {
            return;
        };
        let index = o.focus.and_then(|f| WIDGETS.iter().position(|w| *w == f));
        match key {
            "key.keyboard.escape" => o.close = Some(false),
            "key.keyboard.tab" => {
                let n = WIDGETS.len();
                let next = match index {
                    None if shift => n - 1,
                    None => 0,
                    Some(i) if shift => (i + n - 1) % n,
                    Some(i) => (i + 1) % n,
                };
                set_focus(o, WIDGETS[next], true, true);
            }
            "key.keyboard.enter" | "key.keyboard.keypad.enter" | "key.keyboard.space" => match o.focus {
                Some(W::Speed) => o.can_change = !o.can_change,
                Some(w @ (W::Toggle | W::Reset | W::Save | W::Exit)) => press_button(o, w, clicks),
                _ => {}
            },
            _ => {
                let Some(dir) = arrow else {
                    return;
                };
                let sideways = matches!(dir, gamepad::Button::DPadLeft | gamepad::Button::DPadRight);
                if o.focus == Some(W::Speed) && o.can_change && sideways {
                    let r = W::Speed.rect(size);
                    let step = if dir == gamepad::Button::DPadLeft { -1.0 } else { 1.0 };
                    set_slider(o, o.slider + step / f64::from(r[2] - 8.0));
                    return;
                }
                let next = match index {
                    None => Some(0),
                    Some(i) => {
                        let rects: Vec<[f32; 4]> = WIDGETS.iter().map(|w| w.rect(size)).collect();
                        gamepad::focus_step_rect(rects[i], dir, &rects)
                    }
                };
                if let Some(next) = next {
                    set_focus(o, WIDGETS[next], true, false);
                }
            }
        }
    });
    true
}

/// A key let go, or a character typed: nothing under the screen hears it.
pub(super) fn swallowed(game: &Game<'_>, screen: &NativeScreenView<'_>) -> bool {
    over(game, screen)
}

// ---- the icon and Mod Menu's metadata ---------------------------------------------------

/// Smooth Swapping's icon (`assets/smoothswapping/icon.png`, its
/// `fabric.mod.json`'s).
const ICON: &[u8] = horizon_client::native_data!("smoothswapping", "icon.png");

/// What Mod Menu shows of the mod, from Smooth Swapping's `fabric.mod.json`
/// (26.3 branch; environment "client": the Client badge) and its ru_ru's
/// `modmenu.descriptionTranslation.smoothswapping`.
fn modmenu_json() -> String {
    format!(
        r#"{{
  "schema": 1,
  "name": "{name}",
  "version": "{version}",
  "description": "Moves the items in an inventory smoothly instead of instantly",
  "description_ru": "На смену мгновенному перемещению предметов по инвентарям этот мод добавляет плавную анимацию их перелёта при удерживании Shift или использовании клавиш панели инструментов.",
  "authors": ["Schauweg", "Riflusso"],
  "contributors": ["DeportedToHell"],
  "license": "GNU LGPL 3.0",
  "links": {{
    "homepage": "https://modrinth.com/mod/smooth-swapping",
    "sources": "https://github.com/DeportedToHell/MCRE-Mods/tree/main/smoothswapping",
    "issues": "https://github.com/DeportedToHell/MCRE-Mods/issues"
  }},
  "badges": ["client"],
  "config_screen": true
}}
"#,
        name = ss::NAME,
        version = ss::VERSION,
    )
}

/// `icon.png` and `modmenu.json` in the mod's data folder, each written only
/// when it differs from what is there.
fn write_data(game: &Game<'_>) {
    let dir = game.native_data_dir(ss::ID);
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
