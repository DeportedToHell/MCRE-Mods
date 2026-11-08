// Zoomify's demo pictures in the config screen's description panel
// (`config.demo`: `ZoomDemoImageRenderer`, `FirstPersonDemo`,
// `ThirdPersonDemo`, `ControlEmulation`): the original's own pictures of the
// world, the hand, a third-person view and the HUD, zoomed in and out by a
// `ZoomHelper` of their own that a pretend key drives, twenty ticks a second.
//
// The pictures are Zoomify's WebPs, decoded once on a thread of their own and
// put into the game's atlas slot for a mod's image (`native_mod_image`), one
// pair at a time: the first-person pair or the third-person one.

use super::zoomify::{Settings, Transition};
use super::Game;
use horizon_client::hud::HudBatch;
use std::sync::{Arc, Mutex};

// ---- the original's ZoomHelper, its interpolators and the pretend key ---------------

/// What a demo's `ZoomHelper` reads of its settings (live in the original).
#[derive(Clone, Copy, Debug)]
pub(super) struct Cfg {
    initial_zoom: f64,
    zoom_per_step: f64,
    max_tiers: i32,
    time_in: f64,
    time_out: f64,
    transition_in: Transition,
    transition_out: Transition,
    /// `SmoothInterpolator`'s share: `lerp(smoothness / 100, 1.0, 0.1)`.
    smoothness: f64,
}

impl Cfg {
    /// `RegularZoomHelper(settings)`.
    pub(super) fn regular(s: &Settings) -> Cfg {
        Cfg {
            initial_zoom: f64::from(s.initial_zoom),
            zoom_per_step: f64::from(s.zoom_per_step),
            max_tiers: s.scroll_step_count as i32,
            time_in: s.zoom_in_time,
            time_out: s.zoom_out_time,
            transition_in: s.zoom_in_transition,
            transition_out: s.zoom_out_transition,
            smoothness: 1.0 + f64::from(s.scroll_smoothness) / 100.0 * (0.1 - 1.0),
        }
    }

    /// `SecondaryZoomHelper(settings)`.
    pub(super) fn secondary(s: &Settings) -> Cfg {
        Cfg {
            initial_zoom: f64::from(s.secondary_amount),
            zoom_per_step: 100.0,
            max_tiers: 0,
            time_in: s.secondary_in_time,
            time_out: s.secondary_out_time,
            transition_in: Transition::Linear,
            transition_out: Transition::Linear,
            smoothness: 1.0,
        }
    }

    /// `minScrollTiers`: scrolling out stops at the normal field of view.
    fn min_tiers(&self) -> i32 {
        let step = self.zoom_per_step / 100.0;
        if step <= 1.0 {
            return 0;
        }
        let to_normal = (self.initial_zoom.ln() / step.ln()).ceil() as i32;
        -to_normal.clamp(0, self.max_tiers.max(0))
    }
}

fn lerp(t: f64, a: f64, b: f64) -> f64 {
    a + t * (b - a)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    /// `TransitionInterpolator`: timed, eased by the transitions.
    Transition,
    /// `TimedInterpolator`: timed, linear.
    Timed,
    /// `SmoothInterpolator`: a share of the way each tick.
    Smooth,
    /// `InstantInterpolator`.
    Instant,
}

#[derive(Clone, Debug)]
struct Interp {
    kind: Kind,
    going_in: bool,
    active: Transition,
    inactive: Transition,
    prev_target: f64,
    just_swapped: bool,
}

impl Interp {
    fn new(kind: Kind, c: &Cfg) -> Interp {
        Interp {
            kind,
            going_in: true,
            active: c.transition_in,
            // The constructor's own `transitionOut`, not yet its opposite.
            inactive: c.transition_out,
            prev_target: 0.0,
            just_swapped: false,
        }
    }

    fn timed_smooth(&self, c: &Cfg) -> bool {
        (self.going_in && c.time_in > 0.0) || (!self.going_in && c.time_out > 0.0)
    }

    /// `isSmooth`.
    fn smooth(&self, c: &Cfg) -> bool {
        match self.kind {
            Kind::Instant => false,
            Kind::Timed => self.timed_smooth(c),
            Kind::Transition => !self.just_swapped && self.timed_smooth(c) && self.active != Transition::Instant,
            Kind::Smooth => c.smoothness != 1.0,
        }
    }

    fn increment(&self, out: bool, dt: f64, target: f64, current: f64, c: &Cfg) -> f64 {
        match self.kind {
            Kind::Smooth => {
                let diff = if out { current - target } else { target - current };
                diff * c.smoothness / 0.05 * dt
            }
            _ => dt / if out { c.time_out } else { c.time_in },
        }
    }

    /// `LinearInterpolator.tickInterpolation`.
    fn linear(&mut self, target: f64, current: f64, dt: f64, c: &Cfg) -> f64 {
        if target > current {
            self.going_in = true;
            (current + self.increment(false, dt, target, current, c)).min(target)
        } else if target < current {
            self.going_in = false;
            (current - self.increment(true, dt, target, current, c)).max(target)
        } else {
            self.going_in = true;
            target
        }
    }

    fn tick(&mut self, target: f64, current: f64, dt: f64, c: &Cfg) -> f64 {
        match self.kind {
            Kind::Instant => target,
            Kind::Timed => self.linear(target, current, dt, c),
            Kind::Smooth if !self.smooth(c) => target,
            Kind::Smooth => self.linear(target, current, dt, c),
            Kind::Transition => {
                let (tin, tout) = (c.transition_in, c.transition_out.opposite());
                let mut cur = current;
                if target > current {
                    self.active = tin;
                    self.inactive = tout;
                    if self.prev_target < target && self.active.has_inverse() {
                        self.just_swapped = true;
                        cur = self.active.inverse(self.inactive.apply(cur));
                    }
                } else if target < current {
                    self.active = tout;
                    self.inactive = tin;
                    if self.prev_target > target && self.active.has_inverse() {
                        self.just_swapped = true;
                        cur = self.active.inverse(self.inactive.apply(cur));
                    }
                }
                self.prev_target = target;
                if self.active == Transition::Instant {
                    return target;
                }
                self.linear(target, cur, dt, c)
            }
        }
    }

    fn modify(&self, x: f64) -> f64 {
        if self.kind == Kind::Transition { self.active.apply(x) } else { x }
    }

    fn modify_prev(&mut self, x: f64) -> f64 {
        if self.kind == Kind::Transition && self.just_swapped {
            self.just_swapped = false;
            return self.active.inverse(self.inactive.apply(x));
        }
        x
    }
}

/// The original's `ZoomHelper`, ticked twenty times a second.
#[derive(Clone, Debug)]
struct Helper {
    initial: Interp,
    scroll: Interp,
    prev_initial: f64,
    initial_at: f64,
    zooming_last: bool,
    prev_scroll: f64,
    scroll_at: f64,
    last_tier: i32,
    resetting: bool,
    reset_multiplier: f64,
}

impl Helper {
    fn new(secondary: bool, c: &Cfg) -> Helper {
        let (initial, scroll) = if secondary { (Kind::Timed, Kind::Instant) } else { (Kind::Transition, Kind::Smooth) };
        Helper {
            initial: Interp::new(initial, c),
            scroll: Interp::new(scroll, c),
            prev_initial: 0.0,
            initial_at: 0.0,
            zooming_last: false,
            prev_scroll: 0.0,
            scroll_at: 0.0,
            last_tier: 0,
            resetting: false,
            reset_multiplier: 0.0,
        }
    }

    fn tick(&mut self, zooming: bool, tiers: i32, c: &Cfg) {
        let dt = 0.05;
        if zooming && !self.zooming_last {
            self.resetting = false;
        }
        self.prev_initial = self.initial_at;
        self.initial_at = self.initial.tick(if zooming { 1.0 } else { 0.0 }, self.initial_at, dt, c);
        self.prev_initial = self.initial.modify_prev(self.prev_initial);
        if !self.initial.smooth(c) {
            self.prev_initial = self.initial_at;
        }
        self.zooming_last = zooming;

        if tiers.abs() > self.last_tier.abs() {
            self.resetting = false;
        }
        let target = if c.max_tiers > 0 { f64::from(tiers) / f64::from(c.max_tiers) } else { 0.0 };
        self.prev_scroll = self.scroll_at;
        self.scroll_at = self.scroll.tick(target, self.scroll_at, dt, c);
        self.prev_scroll = self.scroll.modify_prev(self.prev_scroll);
        // As the original: the initial interpolator's smoothness again.
        if !self.initial.smooth(c) {
            self.prev_initial = self.initial_at;
        }
        self.last_tier = tiers;
    }

    /// `getZoomDivisor(tickDelta)`.
    fn divisor(&mut self, delta: f64, c: &Cfg) -> f64 {
        let t = if self.initial.smooth(c) {
            self.initial.modify(lerp(delta, self.prev_initial, self.initial_at))
        } else {
            self.initial_at
        };
        let end = if self.resetting { self.reset_multiplier } else { 1.0 / c.initial_zoom };
        let base = 1.0 / lerp(t, 1.0, end);
        let scroll = if self.resetting {
            0.0
        } else if self.scroll.smooth(c) {
            self.scroll.modify(lerp(delta, self.prev_scroll, self.scroll_at))
        } else {
            self.scroll_at
        };
        let raw = base * (c.zoom_per_step / 100.0).powf(scroll * f64::from(c.max_tiers));
        let divisor = if raw.is_finite() { raw.clamp(1.0, 500.0) } else { 1.0 };
        if self.initial_at == 0.0 && self.scroll_at == 0.0 {
            self.resetting = false;
        }
        if !self.resetting {
            self.reset_multiplier = 1.0 / divisor;
        }
        divisor
    }

    fn set_to_zero(&mut self, initial: bool, scroll: bool) {
        if initial {
            self.initial_at = 0.0;
            self.prev_initial = 0.0;
            self.zooming_last = false;
        }
        if scroll {
            self.scroll_at = 0.0;
            self.prev_scroll = 0.0;
            self.last_tier = 0;
        }
        self.resetting = false;
    }

    fn skip_initial(&mut self) {
        self.initial_at = 1.0;
        self.prev_initial = 1.0;
    }
}

/// The pretend key (`ControlEmulation`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Control {
    /// Holds the key until the zoom settles, waits a second, lets go.
    InitialOnly,
    /// Zoomed in already, scrolls in step by step and back out.
    ScrollOnly,
}

/// The two `ControlEmulation` objects' own state, which every demo using one
/// shares (they are Kotlin objects).
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Shared {
    switch_pause: i32,
    scroll_pause: i32,
    reverse: bool,
}

/// One `ZoomDemoImageRenderer`.
#[derive(Clone, Debug)]
pub(super) struct Demo {
    helper: Helper,
    key_down: bool,
    scroll_tiers: i32,
    control: Control,
    /// `ThirdPersonDemo` (the secondary zoom's), else `FirstPersonDemo`.
    pub(super) third_person: bool,
    /// `FirstPersonDemo.keepHandFov`: the hand stays as it is.
    pub(super) keep_hand_fov: bool,
    /// `ThirdPersonDemo.renderHud`.
    pub(super) render_hud: bool,
}

impl Demo {
    pub(super) fn new(control: Control, third_person: bool, c: &Cfg) -> Demo {
        let mut demo = Demo {
            helper: Helper::new(third_person, c),
            key_down: false,
            scroll_tiers: 0,
            control,
            third_person,
            keep_hand_fov: false,
            render_hud: true,
        };
        // `zoomControl.setup(this)`.
        demo.key_down = true;
        if control == Control::ScrollOnly {
            demo.helper.skip_initial();
        }
        demo
    }

    /// `tick`: the helper, then the pretend key.
    pub(super) fn tick(&mut self, c: &Cfg, shared: &mut Shared) {
        self.helper.tick(self.key_down, self.scroll_tiers, c);
        match self.control {
            Control::InitialOnly => {
                if shared.switch_pause > 0 {
                    shared.switch_pause -= 1;
                    if shared.switch_pause == 0 {
                        self.key_down = !self.key_down;
                    }
                } else if self.helper.divisor(1.0, c) == self.helper.divisor(0.0, c) {
                    shared.switch_pause = 20;
                }
            }
            Control::ScrollOnly => {
                if c.max_tiers == 0 {
                    self.helper.set_to_zero(false, true);
                    self.scroll_tiers = 0;
                    return;
                }
                if shared.scroll_pause > 0 {
                    shared.scroll_pause -= 1;
                    return;
                }
                shared.scroll_pause = 3;
                if !shared.reverse {
                    self.scroll_tiers += 1;
                    if self.scroll_tiers >= c.max_tiers {
                        shared.reverse = true;
                        shared.scroll_pause = 20;
                    }
                } else {
                    self.scroll_tiers -= 1;
                    if self.scroll_tiers <= c.min_tiers() {
                        shared.reverse = false;
                        shared.scroll_pause = 20;
                    }
                }
            }
        }
    }

    /// `pause`: back to no zoom, the key let go for a second.
    pub(super) fn pause(&mut self, shared: &mut Shared) {
        self.helper.set_to_zero(true, true);
        match self.control {
            Control::InitialOnly => {
                self.key_down = false;
                shared.switch_pause = 20;
            }
            Control::ScrollOnly => {
                shared.scroll_pause = 20;
                shared.reverse = false;
                self.scroll_tiers = 0;
                self.helper.skip_initial();
            }
        }
    }

    /// The picture's width in the original's pixels (`TEX_WIDTH`).
    fn tex_width(&self) -> f64 {
        if self.third_person { 1915.0 } else { 1916.0 }
    }

    /// `render`'s height for `width`: what the picture takes, drawn or not.
    pub(super) fn height(&self, width: i32) -> i32 {
        (910.0 * (f64::from(width) / self.tex_width())) as i32
    }

    /// `render` at `(x, y)`, `width` wide, inside `clip` (the description's
    /// scissor); `ready` when the pair is in the atlas. Returns its height.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw(
        &mut self,
        game: &Game<'_>,
        batch: &mut HudBatch,
        (x, y, width): (i32, i32, i32),
        delta: f64,
        c: &Cfg,
        ready: bool,
        clip: [f32; 4],
    ) -> i32 {
        let height = self.height(width);
        if !ready {
            return height;
        }
        let ratio = f64::from(width) / self.tex_width();
        let zoom = self.helper.divisor(delta, c) as f32;
        let (x, y) = (x as f32, y as f32);
        let drawn = [x, y, (self.tex_width() * ratio) as f32, (910.0 * ratio) as f32];
        // Both pictures scale about `FirstPersonDemo`'s centre.
        let (cx, cy) = (x + (958.0 * ratio) as f32, y + (455.0 * ratio) as f32);
        let rows = (f64::from(SLOT_WIDTH) * 910.0 / self.tex_width()) as f32;
        let start = batch.vertices.len();
        game.native_sprite(batch, horizon_client::hud::MOD_IMAGE_SPRITE, [0.0, 0.0, SLOT_WIDTH as f32, rows], drawn);
        let overlay = [0.0, OVERLAY_ROW as f32, SLOT_WIDTH as f32, rows];
        let overlay_zooms = if self.third_person { false } else { !self.keep_hand_fov };
        if overlay_zooms {
            game.native_sprite(batch, horizon_client::hud::MOD_IMAGE_SPRITE, overlay, drawn);
        }
        batch.scale_since(start, cx, cy, zoom, zoom);
        let shows_overlay = if self.third_person { self.render_hud } else { self.keep_hand_fov };
        if shows_overlay {
            game.native_sprite(batch, horizon_client::hud::MOD_IMAGE_SPRITE, overlay, drawn);
        }
        let x1 = (x + width as f32).min(clip[2]);
        let y1 = (y + height as f32).min(clip[3]);
        batch.clip_since(start, x.max(clip[0]), y.max(clip[1]), x1, y1);
        height
    }
}

// ---- the pictures --------------------------------------------------------------------

/// Which pair is in the atlas slot.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Pair {
    /// `zoom-world` under `zoom-hand`.
    FirstPerson,
    /// `third-person-view` under `third-person-hud`.
    ThirdPerson,
}

/// The slot's picture: the base picture's rows from the top, the overlay's
/// from `OVERLAY_ROW`, each 1024 wide.
pub(super) const SLOT_WIDTH: u32 = 1024;
const OVERLAY_ROW: u32 = 488;
pub(super) const SLOT_HEIGHT: u32 = OVERLAY_ROW * 2;

const WORLD: &[u8] = horizon_client::native_data!("zoomify", "demo/zoom-world.webp");
const HAND: &[u8] = horizon_client::native_data!("zoomify", "demo/zoom-hand.webp");
const VIEW: &[u8] = horizon_client::native_data!("zoomify", "demo/third-person-view.webp");
const HUD: &[u8] = horizon_client::native_data!("zoomify", "demo/third-person-hud.webp");

/// Both pairs, `SLOT_WIDTH` by `SLOT_HEIGHT` RGBA each, once decoded.
type Pictures = Arc<[Vec<u8>; 2]>;

enum Load {
    Idle,
    Decoding,
    Done(Option<Pictures>),
}

static PICTURES: Mutex<Load> = Mutex::new(Load::Idle);

/// The pictures, decoding them on a thread of their own the first time
/// (`ImageRendererManager.registerOrGetImage`): None until they are ready,
/// or when they don't decode.
pub(super) fn pictures() -> Option<Pictures> {
    let mut load = PICTURES.lock().unwrap_or_else(|e| e.into_inner());
    match &*load {
        Load::Done(p) => p.clone(),
        Load::Decoding => None,
        Load::Idle => {
            *load = Load::Decoding;
            let started = std::thread::Builder::new().name("zoomify-demo".into()).spawn(|| {
                let pictures = (|| Some(Arc::new([pair(WORLD, HAND)?, pair(VIEW, HUD)?])))();
                *PICTURES.lock().unwrap_or_else(|e| e.into_inner()) = Load::Done(pictures);
            });
            if started.is_err() {
                *load = Load::Done(None);
            }
            None
        }
    }
}

impl Pair {
    pub(super) fn index(self) -> usize {
        match self {
            Pair::FirstPerson => 0,
            Pair::ThirdPerson => 1,
        }
    }
}

/// A base picture and its overlay, scaled down into the slot's layout.
fn pair(base: &[u8], overlay: &[u8]) -> Option<Vec<u8>> {
    let mut out = vec![0u8; (SLOT_WIDTH * SLOT_HEIGHT * 4) as usize];
    for (bytes, row) in [(base, 0), (overlay, OVERLAY_ROW)] {
        let (w, h, rgba) = horizon_client::hud::decode_image_bytes(bytes).ok()?;
        scale_into(&mut out, row, (w, h, &rgba));
    }
    Some(out)
}

/// The picture averaged down to `SLOT_WIDTH` wide (its height in proportion)
/// at `row` of the slot, its colours weighted by their alpha so the hand's
/// and the HUD's edges keep no dark fringe.
fn scale_into(out: &mut [u8], row: u32, (w, h, rgba): (u32, u32, &[u8])) {
    if w == 0 || h == 0 {
        return;
    }
    let nw = SLOT_WIDTH as usize;
    let nh = ((u64::from(SLOT_WIDTH) * u64::from(h)).div_ceil(u64::from(w)) as usize).min(OVERLAY_ROW as usize);
    let (w, h) = (w as usize, h as usize);
    for oy in 0..nh {
        let (y0, y1) = (oy * h / nh, ((oy + 1) * h / nh).max(oy * h / nh + 1).min(h));
        for ox in 0..nw {
            let (x0, x1) = (ox * w / nw, ((ox + 1) * w / nw).max(ox * w / nw + 1).min(w));
            let mut sum = [0u64; 4];
            for sy in y0..y1 {
                for sx in x0..x1 {
                    let i = (sy * w + sx) * 4;
                    let a = u64::from(rgba[i + 3]);
                    for (c, s) in sum.iter_mut().take(3).enumerate() {
                        *s += u64::from(rgba[i + c]) * a;
                    }
                    sum[3] += a;
                }
            }
            let n = ((y1 - y0) * (x1 - x0)) as u64;
            let o = ((row as usize + oy) * nw + ox) * 4;
            if sum[3] > 0 {
                for c in 0..3 {
                    out[o + c] = (sum[c] / sum[3]) as u8;
                }
            }
            out[o + 3] = (sum[3] / n.max(1)) as u8;
        }
    }
}
