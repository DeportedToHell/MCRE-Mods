// BetterF3's debug screen in place of the game's F3 text (`DebugMixin`,
// `FabricDebugMixin`, `DebugRenderer`): the modules' lines in two columns
// on their backgrounds, at the font scale, sliding in and out
// (`renderAnimation`), with the game's charts kept; the boss bar and the
// sidebar hidden while it shows; Auto Start. Ported from BetterF3 by
// cominixo and TreyRuffy (MIT, https://github.com/TreyRuffy/BetterF3).
use super::betterf3::{self as b, General, Kind, ModuleRef};
use super::betterf3_lines as lines;
use super::{Game, Screen};
use horizon_client::debug_overlay::{self, PaintCache};
use horizon_client::hud::HudBatch;
use horizon_client::hud_layout::{self, GuiDraw, PoseOp};
use horizon_client::text::Text;
use std::cell::RefCell;
use std::time::{Duration, Instant};

/// `Utils.START_X_POS`.
const START_X: i32 = 200;
/// `DebugRenderer.STATIC_REFRESH_INTERVAL_MS`.
const STATIC_REFRESH: Duration = Duration::from_millis(250);
/// The text's colour where a line has none (`0xFFE0E0E0`).
const TEXT: u32 = 0xFFE0_E0E0;

struct State {
    /// `Utils.xPos`, `closingAnimation`, `lastAnimationUpdate`.
    x: i32,
    closing: bool,
    last_step: Instant,
    /// The game's overlay switch as last seen, and whether a world was.
    was_open: bool,
    was_in_world: bool,
    paint: PaintCache,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State {
        x: START_X,
        closing: false,
        last_step: Instant::now(),
        was_open: false,
        was_in_world: false,
        paint: PaintCache::default(),
    });
}

fn with<R>(f: impl FnOnce(&mut State) -> R) -> Option<R> {
    STATE.with(|s| s.try_borrow_mut().ok().map(|mut s| f(&mut s)))
}

/// The mod is installed, switched on in Options > Mods and not disabled in
/// its own settings.
pub(super) fn active(g: &Game<'_>) -> bool {
    g.native_on(b::ID) && !b::general().disable_mod
}

/// `debugEntries.isOverlayVisible()` as BetterF3 keeps it: open, or still
/// sliding out.
pub(super) fn visible(g: &Game<'_>) -> bool {
    g.debug || with(|s| s.closing).unwrap_or(false)
}

/// Once a frame: Auto Start when a world opens, the opening and closing
/// animation (`DebugScreenEntryListMixin.onSetOverlayVisible` and
/// `renderAnimation`).
pub(super) fn frame(g: &mut Game<'_>) {
    let opt = b::general();
    let on = g.native_on(b::ID) && !opt.disable_mod;
    let in_world = g.player.is_some();
    let joined = with(|s| {
        let joined = in_world && !s.was_in_world;
        s.was_in_world = in_world;
        joined
    })
    .unwrap_or(false);
    // `DebugOptionMixin`: the overlay (and the charts asked for) on a new level.
    if on && joined && opt.auto_f3 {
        g.debug = true;
        if opt.always_tps {
            g.debug_charts.fps = true;
        }
        if opt.always_ping {
            g.debug_charts.toggle_network(&mut g.debug);
        }
    }
    let open = g.debug;
    with(|s| {
        if on && opt.animations {
            if open && !s.was_open {
                s.closing = false;
                s.x = START_X;
            } else if !open && s.was_open {
                s.closing = true;
            }
        } else {
            s.closing = false;
            s.x = 0;
        }
        s.was_open = open;
        step(s, &opt);
    });
}

/// `renderAnimation`: every 10 ms the columns move toward their place, or
/// away until 300 pixels out.
fn step(s: &mut State, opt: &General) {
    if !opt.animations || s.last_step.elapsed() < Duration::from_millis(10) || (s.x == 0 && !s.closing) {
        return;
    }
    let mut i = ((START_X / 2 + s.x) / 10) - 9;
    if s.x != 0 && !s.closing {
        s.x = (f64::from(s.x) / opt.animation_speed) as i32;
        s.x -= i;
    }
    if i == 0 {
        i = 1;
    }
    if s.closing {
        s.x += i;
        s.x = (f64::from(s.x) * opt.animation_speed) as i32;
        if s.x >= 300 {
            s.closing = false;
        }
    }
    s.last_step = Instant::now();
}

/// The boss bar and the sidebar while the overlay shows (`BossbarMixin`,
/// `ScoreboardMixin`).
pub(super) fn hides_layer(g: &Game<'_>, layer: &str) -> bool {
    if !g.native_on(b::ID) {
        return false;
    }
    let opt = b::general();
    if opt.disable_mod || !visible(g) {
        return false;
    }
    (layer == "minecraft:boss_overlay" && opt.hide_bossbar) || (layer == "minecraft:scoreboard_sidebar" && opt.hide_sidebar)
}

/// `draw_debug_overlay` in BetterF3's way while it shows: true when it drew
/// (the game's own overlay is held back).
pub(super) fn draw(g: &Game<'_>, batch: &mut HudBatch, size: [u32; 2]) -> bool {
    if !active(g) || g.screen == Screen::DebugOptions || !visible(g) {
        return false;
    }
    let screen_open = g.screen != Screen::Playing;
    if g.title.is_some() || (g.ui.hide_gui && !screen_open) {
        lines::stop_server_debug(g);
        return true;
    }
    // `DebugRenderer.newText`: the lines over no screen, the chat, or the
    // modules screen (which draws them itself, under it).
    let lines_here = g.title.is_none() && matches!(g.screen, Screen::Playing | Screen::Chat);
    let animating = with(|s| s.closing || s.x != 0).unwrap_or(false) && b::general().animations;
    let layout = (size, batch.scale.to_bits(), g.gui.atlas.font.epoch());
    let now = Instant::now();
    if !animating
        && let Some(done) = with(|s| {
            s.paint.fresh(now, layout).map(|v| {
                batch.vertices.extend_from_slice(v);
            })
        })
        && done.is_some()
    {
        return true;
    }
    let first = batch.vertices.len();
    if lines_here {
        paint_lines(g, batch, size);
    } else {
        lines::stop_server_debug(g);
    }
    if g.debug {
        paint_charts(g, batch, size);
    }
    with(|s| s.paint.store(now, layout, &batch.vertices[first..]));
    true
}

/// The overlay's scale against the GUI's and the width it lays out in
/// (`debugGuiScale`).
fn overlay_scale(g: &Game<'_>, batch: &HudBatch, size: [u32; 2]) -> (f32, [i32; 2]) {
    debug_overlay::scale(
        size.map(|v| v as i32),
        batch.scale as i32,
        g.settings.file.int("debugGuiScale"),
        g.settings.force_unicode,
        [batch.width as i32, batch.height as i32],
    )
}

/// The game's charts while the overlay is open (`renderFpsCharts`,
/// `renderNetworkCharts`, the lightmap).
fn paint_charts(g: &Game<'_>, batch: &mut HudBatch, size: [u32; 2]) {
    let (scale, [width, height]) = overlay_scale(g, batch, size);
    let atlas = &g.gui.atlas;
    let mut draws = vec![GuiDraw::Push(vec![PoseOp::Scale(scale, scale)])];
    draws.extend(g.debug_charts.draw(
        &atlas.font,
        width,
        height,
        g.settings.max_fps,
        1000.0 / g.clock.rate(),
        !g.has_singleplayer_server(),
    ));
    if g.debug_charts.lightmap
        && let Some(r) = &g.renderer
    {
        draws.extend(horizon_client::debug_chart::lightmap(&r.lightmap_pixels, width, height));
    }
    draws.push(GuiDraw::Pop);
    hud_layout::paint(batch, atlas, &draws);
}

/// `DebugRenderer.newText` for one column: each enabled module's lines,
/// updated as often as `shouldUpdateModuleNow` says, and a blank line after
/// each with Space Modules.
fn column(g: &Game<'_>, modules: &[ModuleRef], opt: &General, shared: &lines::Shared, reduced: bool) -> Vec<Text> {
    let mut out = Vec::new();
    let now = Instant::now();
    for m in modules {
        let Ok(mut m) = m.try_borrow_mut() else {
            continue;
        };
        if !m.enabled {
            continue;
        }
        let due = m.kind.every_frame()
            || !opt.performance
            || m.dirty
            || m.memory.updated.is_none_or(|at| now.duration_since(at) >= STATIC_REFRESH);
        if due {
            lines::update(&mut m, g, shared);
            m.dirty = false;
            m.memory.updated = Some(now);
        }
        lines::format_lines(&m, reduced, &mut out);
        if opt.space_modules {
            out.push(Text::default());
        }
    }
    out
}

/// Both columns, their backgrounds first (`drawBackground`) and then their
/// text (`drawLeftText`, `drawRightText`), at the overlay's scale times the
/// font scale and moved by the animation.
pub(super) fn paint_lines(g: &Game<'_>, batch: &mut HudBatch, size: [u32; 2]) {
    if g.player.is_none() {
        return;
    }
    let opt = b::general();
    let (left_modules, right_modules) = b::with(|c| (c.left.clone(), c.right.clone())).unwrap_or_default();
    let wanted = left_modules.iter().chain(&right_modules).any(|m| {
        m.try_borrow().is_ok_and(|m| m.enabled && matches!(m.kind, Kind::Chunks | Kind::Location | Kind::Entity))
    });
    let shared = lines::Shared { server: lines::server_debug(g, wanted), framebuffer: size.map(|v| v as i32) };
    let reduced = g.reduced_debug_info();
    let left = column(g, &left_modules, &opt, &shared, reduced);
    let right = column(g, &right_modules, &opt, &shared, reduced);
    let (scale, [width, _]) = overlay_scale(g, batch, size);
    let font_scale = opt.font_scale as f32;
    let x = if opt.animations { with(|s| s.x).unwrap_or(0) } else { 0 };
    let atlas = &g.gui.atlas;
    let mut draws = vec![
        GuiDraw::Push(vec![PoseOp::Scale(scale, scale)]),
        GuiDraw::Push(vec![PoseOp::Scale(font_scale, font_scale)]),
    ];
    let right_edge = (f64::from(width) / opt.font_scale) as i32 - 2;
    for (texts, on_left) in [(&left, true), (&right, false)] {
        let placed: Vec<(Text, i32, i32, i32)> = texts
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let t = t.resolve_legacy();
                let w = atlas.font.width_text(&t) as i32;
                let left = if on_left { 2 - x } else { right_edge - w + x };
                (t, left, 2 + 9 * i as i32, w)
            })
            .collect();
        for (_, left, y, w) in &placed {
            if *w == 0 {
                continue;
            }
            draws.push(hud_layout::fill(left - 1, y - 1, left + w + 1, y + 8, opt.background_color));
        }
        for (t, left, y, _) in placed {
            if t.is_empty() {
                continue;
            }
            draws.push(GuiDraw::Text { text: t, x: left, y, color: TEXT, shadow: opt.shadow_text });
        }
    }
    draws.push(GuiDraw::Pop);
    draws.push(GuiDraw::Pop);
    hud_layout::paint(batch, atlas, &draws);
}

