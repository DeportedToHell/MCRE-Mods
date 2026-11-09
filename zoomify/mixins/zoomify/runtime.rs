// Zoomify for Horizon: the mod's logic, with no game types, so the same file
// builds and tests on its own (`cargo test` at the repository root) and is
// what the bundle adds to the game as `crate::game::zoomify`.
//
// A port of Zoomify by isXander (LGPL-3.0-or-later,
// https://github.com/isXander/Zoomify): `ZoomHelper`, its interpolators and
// transitions, the spyglass rules and the settings with their defaults. The
// original steps its interpolators once a tick and blends the last two
// ticks; this port steps them once a frame by the frame's time, which draws
// the same curves.

use std::sync::Mutex;

pub const ID: &str = "zoomify";
pub const NAME: &str = "Zoomify";
pub const VERSION: &str = "1.1.1";
pub const UNBOUND: &str = "key.keyboard.unknown";
/// The mod's keys: name and default (the original's C and F6; scroll keys
/// unbound).
pub const KEYS: [(&str, &str); 4] = [
    ("zoom", "key.keyboard.c"),
    ("secondary", "key.keyboard.f6"),
    ("zoom_in", UNBOUND),
    ("zoom_out", UNBOUND),
];

pub fn russian(language: &str) -> bool {
    language.starts_with("ru")
}

pub fn description(ru: bool) -> &'static str {
    if ru {
        "Зум с плавными переходами, зумом колесом, подзорной трубой и вторым кинематографичным зумом."
    } else {
        "A zoom mod with infinite customizability."
    }
}

pub fn key_label(name: &str, ru: bool) -> &'static str {
    match (name, ru) {
        ("zoom", true) => "Зум",
        ("secondary", true) => "Второй зум",
        ("zoom_in", true) => "Приблизить колесом",
        ("zoom_out", true) => "Отдалить колесом",
        ("zoom", false) => "Zoom",
        ("secondary", false) => "Secondary Zoom",
        ("zoom_in", false) => "Scroll Zoom In",
        _ => "Scroll Zoom Out",
    }
}

// ---- transitions -------------------------------------------------------------

/// Zoomify's `TransitionType`: the curve a zoom in or out follows.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Transition {
    Instant,
    Linear,
    EaseInSine,
    EaseOutSine,
    EaseInOutSine,
    EaseInQuad,
    EaseOutQuad,
    EaseInOutQuad,
    EaseInCubic,
    EaseOutCubic,
    EaseInOutCubic,
    EaseInExp,
    EaseOutExp,
    EaseInOutExp,
}

impl Transition {
    pub const ALL: [Transition; 14] = [
        Transition::Instant,
        Transition::Linear,
        Transition::EaseInSine,
        Transition::EaseOutSine,
        Transition::EaseInOutSine,
        Transition::EaseInQuad,
        Transition::EaseOutQuad,
        Transition::EaseInOutQuad,
        Transition::EaseInCubic,
        Transition::EaseOutCubic,
        Transition::EaseInOutCubic,
        Transition::EaseInExp,
        Transition::EaseOutExp,
        Transition::EaseInOutExp,
    ];

    pub fn stored(self) -> &'static str {
        match self {
            Transition::Instant => "instant",
            Transition::Linear => "linear",
            Transition::EaseInSine => "ease_in_sine",
            Transition::EaseOutSine => "ease_out_sine",
            Transition::EaseInOutSine => "ease_in_out_sine",
            Transition::EaseInQuad => "ease_in_quad",
            Transition::EaseOutQuad => "ease_out_quad",
            Transition::EaseInOutQuad => "ease_in_out_quad",
            Transition::EaseInCubic => "ease_in_cubic",
            Transition::EaseOutCubic => "ease_out_cubic",
            Transition::EaseInOutCubic => "ease_in_out_cubic",
            Transition::EaseInExp => "ease_in_exp",
            Transition::EaseOutExp => "ease_out_exp",
            Transition::EaseInOutExp => "ease_in_out_exp",
        }
    }

    pub fn label(self, ru: bool) -> &'static str {
        if !ru {
            return match self {
                Transition::Instant => "Instant",
                Transition::Linear => "Linear",
                Transition::EaseInSine => "Ease In Sine",
                Transition::EaseOutSine => "Ease Out Sine",
                Transition::EaseInOutSine => "Ease In Out Sine",
                Transition::EaseInQuad => "Ease In Quad",
                Transition::EaseOutQuad => "Ease Out Quad",
                Transition::EaseInOutQuad => "Ease In Out Quad",
                Transition::EaseInCubic => "Ease In Cubic",
                Transition::EaseOutCubic => "Ease Out Cubic",
                Transition::EaseInOutCubic => "Ease In Out Cubic",
                Transition::EaseInExp => "Ease In Exponential",
                Transition::EaseOutExp => "Ease Out Exponential",
                Transition::EaseInOutExp => "Ease In Out Exponential",
            };
        }
        match self {
            Transition::Instant => "мгновенно",
            Transition::Linear => "линейно",
            Transition::EaseInSine => "разгон (sine)",
            Transition::EaseOutSine => "торможение (sine)",
            Transition::EaseInOutSine => "разгон и торможение (sine)",
            Transition::EaseInQuad => "разгон (quad)",
            Transition::EaseOutQuad => "торможение (quad)",
            Transition::EaseInOutQuad => "разгон и торможение (quad)",
            Transition::EaseInCubic => "разгон (cubic)",
            Transition::EaseOutCubic => "торможение (cubic)",
            Transition::EaseInOutCubic => "разгон и торможение (cubic)",
            Transition::EaseInExp => "разгон (exp)",
            Transition::EaseOutExp => "торможение (exp)",
            Transition::EaseInOutExp => "разгон и торможение (exp)",
        }
    }

    /// The curve. The exponential ones are easings.net's, scaled to reach 0
    /// and 1 exactly as the original's comments intend (its `1/1023` is an
    /// integer division, a jump of a thousandth at the ends, and its
    /// ease-in-out jumped at the middle).
    pub fn apply(self, t: f64) -> f64 {
        use std::f64::consts::PI;
        match self {
            Transition::Instant | Transition::Linear => t,
            Transition::EaseInSine => 1.0 - (t * PI / 2.0).cos(),
            Transition::EaseOutSine => (t * PI / 2.0).sin(),
            Transition::EaseInOutSine => -((PI * t).cos() - 1.0) / 2.0,
            Transition::EaseInQuad => t * t,
            Transition::EaseOutQuad => 1.0 - (1.0 - t) * (1.0 - t),
            Transition::EaseInOutQuad => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
                }
            }
            Transition::EaseInCubic => t.powi(3),
            Transition::EaseOutCubic => 1.0 - (1.0 - t).powi(3),
            Transition::EaseInOutCubic => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
                }
            }
            Transition::EaseInExp => {
                if t <= 0.0 {
                    0.0
                } else if t >= 1.0 {
                    1.0
                } else {
                    ((10.0 * t).exp2() - 1.0) / 1023.0
                }
            }
            Transition::EaseOutExp => {
                if t <= 0.0 {
                    0.0
                } else if t >= 1.0 {
                    1.0
                } else {
                    1024.0 * (1.0 - (-10.0 * t).exp2()) / 1023.0
                }
            }
            Transition::EaseInOutExp => {
                if t <= 0.0 {
                    0.0
                } else if t >= 1.0 {
                    1.0
                } else if t < 0.5 {
                    ((20.0 * t).exp2() - 1.0) / 2046.0
                } else {
                    1.0 - ((20.0 - 20.0 * t).exp2() - 1.0) / 2046.0
                }
            }
        }
    }

    /// The original gives an inverse to the one-sided curves only.
    pub fn has_inverse(self) -> bool {
        matches!(
            self,
            Transition::EaseInSine
                | Transition::EaseOutSine
                | Transition::EaseInQuad
                | Transition::EaseOutQuad
                | Transition::EaseInCubic
                | Transition::EaseOutCubic
                | Transition::EaseInExp
                | Transition::EaseOutExp
        )
    }

    pub fn inverse(self, x: f64) -> f64 {
        use std::f64::consts::PI;
        let x = x.clamp(0.0, 1.0);
        match self {
            Transition::EaseInSine => (1.0 - x).acos() * 2.0 / PI,
            Transition::EaseOutSine => x.asin() * 2.0 / PI,
            Transition::EaseInQuad => x.sqrt(),
            Transition::EaseOutQuad => 1.0 - (1.0 - x).sqrt(),
            Transition::EaseInCubic => x.cbrt(),
            Transition::EaseOutCubic => 1.0 - (1.0 - x).cbrt(),
            Transition::EaseInExp => {
                if x <= 0.0 {
                    0.0
                } else if x >= 1.0 {
                    1.0
                } else {
                    (1023.0 * x + 1.0).log2() / 10.0
                }
            }
            Transition::EaseOutExp => {
                if x <= 0.0 {
                    0.0
                } else if x >= 1.0 {
                    1.0
                } else {
                    -((1024.0 - 1023.0 * x) / 1024.0).log2() / 10.0
                }
            }
            _ => x,
        }
    }

    /// The curve a zoom out uses for the chosen one: zooming out runs it
    /// backwards, so its mirror image shows the chosen shape.
    pub fn opposite(self) -> Transition {
        match self {
            Transition::EaseInSine => Transition::EaseOutSine,
            Transition::EaseOutSine => Transition::EaseInSine,
            Transition::EaseInQuad => Transition::EaseOutQuad,
            Transition::EaseOutQuad => Transition::EaseInQuad,
            Transition::EaseInCubic => Transition::EaseOutCubic,
            Transition::EaseOutCubic => Transition::EaseInCubic,
            Transition::EaseInExp => Transition::EaseOutExp,
            Transition::EaseOutExp => Transition::EaseInExp,
            other => other,
        }
    }
}

// ---- interpolators and the zoom helper ---------------------------------------

/// The zoom's main progress, 0 (none) to 1 (the initial zoom): the original's
/// `TransitionInterpolator` (eased) or `TimedInterpolator` (linear, the
/// secondary zoom), moving linearly in time.
#[derive(Clone, Debug)]
struct Progress {
    value: f64,
    active: Transition,
    prev_target: f64,
}

impl Progress {
    fn new(transition_in: Transition) -> Self {
        Progress {
            value: 0.0,
            active: transition_in,
            prev_target: 0.0,
        }
    }

    fn tick(&mut self, target: f64, dt: f64, c: &Config) {
        if c.eased {
            let (tin, tout) = (c.transition_in, c.transition_out.opposite());
            // A turn in the other direction keeps the eased value where it
            // is, through the new curve's inverse (the curve in use until now
            // is the original's inactive transition).
            if target > self.value {
                if self.prev_target < target && tin.has_inverse() {
                    self.value = tin.inverse(self.active.apply(self.value));
                }
                self.active = tin;
            } else if target < self.value {
                if self.prev_target > target && tout.has_inverse() {
                    self.value = tout.inverse(self.active.apply(self.value));
                }
                self.active = tout;
            }
            self.prev_target = target;
            if self.active == Transition::Instant {
                self.value = target;
                return;
            }
        }
        if target > self.value {
            self.value = (self.value + dt / c.time_in).min(target);
        } else if target < self.value {
            self.value = (self.value - dt / c.time_out).max(target);
        }
    }

    fn eased(&self, c: &Config) -> f64 {
        if c.eased { self.active.apply(self.value) } else { self.value }
    }
}

/// One `ZoomHelper`'s settings.
#[derive(Clone, Copy, Debug)]
pub struct Config {
    pub initial_zoom: f64,
    /// Percent per scroll step (150: 1.5x).
    pub zoom_per_step: f64,
    pub max_tiers: i32,
    pub time_in: f64,
    pub time_out: f64,
    pub transition_in: Transition,
    pub transition_out: Transition,
    /// Eased by the transitions (the main zoom), or linear (the secondary).
    pub eased: bool,
    /// The share of the way the scroll moves each tick (1: at once).
    pub scroll_rate: f64,
}

impl Config {
    /// Scrolling out stops at the normal field of view.
    pub fn min_tiers(&self) -> i32 {
        let step = self.zoom_per_step / 100.0;
        if step <= 1.0 {
            return 0;
        }
        let to_normal = (self.initial_zoom.ln() / step.ln()).ceil() as i32;
        -to_normal.clamp(0, self.max_tiers.max(0))
    }
}

/// Zoomify's `ZoomHelper`: the initial zoom and the scroll zoom on top.
#[derive(Clone, Debug)]
pub struct Helper {
    progress: Progress,
    scroll: f64,
    zooming_last: bool,
    last_tier: i32,
    resetting: bool,
    reset_multiplier: f64,
}

impl Helper {
    pub fn new(c: &Config) -> Self {
        Helper {
            progress: Progress::new(c.transition_in),
            scroll: 0.0,
            zooming_last: false,
            last_tier: 0,
            resetting: false,
            reset_multiplier: 1.0,
        }
    }

    pub fn tick(&mut self, zooming: bool, tiers: i32, dt: f64, c: &Config) {
        if zooming && !self.zooming_last {
            self.resetting = false;
        }
        self.progress.tick(if zooming { 1.0 } else { 0.0 }, dt, c);
        self.zooming_last = zooming;

        if tiers.abs() > self.last_tier.abs() {
            self.resetting = false;
        }
        let target = if c.max_tiers > 0 {
            f64::from(tiers) / f64::from(c.max_tiers)
        } else {
            0.0
        };
        // SmoothInterpolator: a share of the remaining way each tick.
        self.scroll = if c.scroll_rate >= 1.0 {
            target
        } else {
            let share = 1.0 - (1.0 - c.scroll_rate).powf(dt / 0.05);
            self.scroll + (target - self.scroll) * share
        };
        self.last_tier = tiers;
    }

    /// The zoom key is up: the scroll zoom is let go, and the zoom out
    /// starts from where it was.
    pub fn reset(&mut self) {
        if !self.resetting && self.scroll != 0.0 {
            self.resetting = true;
            self.scroll = 0.0;
        }
    }

    /// How many times the field of view is divided now.
    pub fn divisor(&mut self, c: &Config) -> f64 {
        let eased = self.progress.eased(c);
        let target = if self.resetting {
            self.reset_multiplier
        } else {
            1.0 / c.initial_zoom
        };
        let multiplier = 1.0 + eased * (target - 1.0);
        let base = 1.0 / multiplier;
        let scroll = if self.resetting { 0.0 } else { self.scroll };
        let step = c.zoom_per_step / 100.0;
        let raw = base * step.powf(scroll * f64::from(c.max_tiers));
        let divisor = if raw.is_finite() { raw.clamp(1.0, 500.0) } else { 1.0 };
        if self.progress.value == 0.0 && self.scroll == 0.0 {
            self.resetting = false;
        }
        if !self.resetting {
            self.reset_multiplier = 1.0 / divisor;
        }
        divisor
    }
}

// ---- settings ------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum KeyBehaviour {
    Hold,
    Toggle,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Spyglass {
    /// Zoomify and the spyglass each zoom on their own.
    Combine,
    /// The spyglass zooms with Zoomify's zoom.
    Override,
    OnlyHolding,
    OnlyCarrying,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Overlay {
    Never,
    Holding,
    Carrying,
    Always,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sounds {
    Never,
    Always,
    OnlySpyglass,
    WithOverlay,
}

const INITIAL_ZOOMS: [u32; 8] = [2, 3, 4, 5, 6, 8, 10, 15];
const ZOOM_TIMES: [f64; 7] = [0.0, 0.25, 0.5, 0.75, 1.0, 1.5, 2.0];
const SCROLL_STEPS: [u32; 5] = [3, 5, 10, 15, 20];
const PER_STEP: [u32; 6] = [110, 125, 150, 175, 200, 250];
const SMOOTHNESS: [u32; 6] = [0, 25, 50, 70, 85, 100];
const SENSITIVITY: [u32; 7] = [0, 25, 50, 75, 100, 125, 150];
const SECONDARY_ZOOMS: [u32; 6] = [2, 3, 4, 6, 8, 10];
const SECONDARY_IN: [f64; 5] = [2.0, 5.0, 10.0, 15.0, 20.0];
const SECONDARY_OUT: [f64; 5] = [0.0, 0.5, 1.0, 2.0, 3.0];

/// Zoomify's settings (`ZoomifySettings`), with its defaults; the values
/// step through the lists above on the settings page.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Settings {
    pub initial_zoom: u32,
    pub zoom_in_time: f64,
    pub zoom_out_time: f64,
    pub zoom_in_transition: Transition,
    pub zoom_out_transition: Transition,
    pub affect_hand_fov: bool,
    pub retain_zoom_steps: bool,
    pub scroll_zoom: bool,
    pub scroll_step_count: u32,
    pub zoom_per_step: u32,
    pub scroll_smoothness: u32,
    pub key_behaviour: KeyBehaviour,
    pub keybind_scrolling: bool,
    pub relative_sensitivity: u32,
    /// The cinematic camera while zooming: the original's percentage of
    /// vanilla's smoothing, 0 for off (any other amount glides as vanilla's
    /// cinematic camera does here).
    pub cinematic_camera: u32,
    pub spyglass: Spyglass,
    pub overlay: Overlay,
    pub sounds: Sounds,
    pub secondary_amount: u32,
    pub secondary_in_time: f64,
    pub secondary_out_time: f64,
    pub secondary_hide_hud: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            initial_zoom: 4,
            zoom_in_time: 1.0,
            zoom_out_time: 0.5,
            zoom_in_transition: Transition::EaseOutExp,
            zoom_out_transition: Transition::EaseOutExp,
            affect_hand_fov: true,
            retain_zoom_steps: false,
            scroll_zoom: true,
            scroll_step_count: 10,
            zoom_per_step: 150,
            scroll_smoothness: 70,
            key_behaviour: KeyBehaviour::Hold,
            keybind_scrolling: false,
            relative_sensitivity: 100,
            cinematic_camera: 0,
            spyglass: Spyglass::Combine,
            overlay: Overlay::Holding,
            sounds: Sounds::WithOverlay,
            secondary_amount: 4,
            secondary_in_time: 10.0,
            secondary_out_time: 1.0,
            secondary_hide_hud: true,
        }
    }
}

fn next<T: PartialEq + Copy>(list: &[T], now: T) -> T {
    let i = list.iter().position(|v| *v == now).map_or(0, |i| i + 1);
    list[i % list.len()]
}

/// The enums in the original's order (its config screen cycles them so).
pub const SPYGLASS: [Spyglass; 4] = [Spyglass::Combine, Spyglass::Override, Spyglass::OnlyHolding, Spyglass::OnlyCarrying];
pub const OVERLAY: [Overlay; 4] = [Overlay::Never, Overlay::Holding, Overlay::Carrying, Overlay::Always];
pub const SOUNDS: [Sounds; 4] = [Sounds::Never, Sounds::Always, Sounds::OnlySpyglass, Sounds::WithOverlay];

/// The settings page's preset buttons, after the settings.
pub const PRESETS: usize = 3;
/// How many settings come before the presets on the page.
pub const ROWS: usize = 22;

impl Settings {
    /// The original's presets (`Presets`): Default, OptiFine, OK Zoomer,
    /// each over the defaults.
    pub fn preset(i: usize) -> Settings {
        let mut s = Settings::default();
        s.click(ROWS + i.min(PRESETS - 1));
        s
    }

    pub fn regular(&self) -> Config {
        // Zoomify: lerp(smoothness / 100, 1.0, 0.1).
        let rate = 1.0 + f64::from(self.scroll_smoothness) / 100.0 * (0.1 - 1.0);
        Config {
            initial_zoom: f64::from(self.initial_zoom.max(1)),
            zoom_per_step: f64::from(self.zoom_per_step),
            max_tiers: self.scroll_step_count as i32,
            time_in: self.zoom_in_time,
            time_out: self.zoom_out_time,
            transition_in: self.zoom_in_transition,
            transition_out: self.zoom_out_transition,
            eased: true,
            scroll_rate: rate,
        }
    }

    pub fn secondary(&self) -> Config {
        Config {
            initial_zoom: f64::from(self.secondary_amount.max(1)),
            zoom_per_step: 100.0,
            max_tiers: 0,
            time_in: self.secondary_in_time,
            time_out: self.secondary_out_time,
            transition_in: Transition::Linear,
            transition_out: Transition::Linear,
            eased: false,
            scroll_rate: 1.0,
        }
    }

    pub fn read(get: impl Fn(&str) -> Option<String>) -> Settings {
        let mut s = Settings::default();
        let num = |k: &str| get(k).and_then(|v| v.trim().parse::<f64>().ok());
        let flag = |k: &str, d: bool| match get(k).as_deref().map(str::trim) {
            Some("true") => true,
            Some("false") => false,
            _ => d,
        };
        let int = |k: &str, d: u32, lo: u32, hi: u32| num(k).map_or(d, |v| (v as u32).clamp(lo, hi));
        let secs = |k: &str, d: f64| num(k).map_or(d, |v| v.clamp(0.0, 60.0));
        let transition = |k: &str, d: Transition| {
            get(k)
                .and_then(|v| Transition::ALL.into_iter().find(|t| t.stored() == v.trim()))
                .unwrap_or(d)
        };
        s.initial_zoom = int("initial_zoom", s.initial_zoom, 1, 50);
        s.zoom_in_time = secs("zoom_in_time", s.zoom_in_time);
        s.zoom_out_time = secs("zoom_out_time", s.zoom_out_time);
        s.zoom_in_transition = transition("zoom_in_transition", s.zoom_in_transition);
        s.zoom_out_transition = transition("zoom_out_transition", s.zoom_out_transition);
        s.affect_hand_fov = flag("affect_hand_fov", s.affect_hand_fov);
        s.retain_zoom_steps = flag("retain_zoom_steps", s.retain_zoom_steps);
        s.scroll_zoom = flag("scroll_zoom", s.scroll_zoom);
        s.scroll_step_count = int("scroll_step_count", s.scroll_step_count, 0, 50);
        s.zoom_per_step = int("zoom_per_step", s.zoom_per_step, 100, 500);
        s.scroll_smoothness = int("scroll_smoothness", s.scroll_smoothness, 0, 100);
        s.key_behaviour = match get("key_behaviour").as_deref().map(str::trim) {
            Some("toggle") => KeyBehaviour::Toggle,
            _ => KeyBehaviour::Hold,
        };
        s.keybind_scrolling = flag("keybind_scrolling", s.keybind_scrolling);
        s.relative_sensitivity = int("relative_sensitivity", s.relative_sensitivity, 0, 150);
        // A percentage now; 1.0's on and off read as 100 and 0.
        s.cinematic_camera = match get("cinematic_camera").as_deref().map(str::trim) {
            Some("true") => 100,
            Some("false") => 0,
            _ => int("cinematic_camera", s.cinematic_camera, 0, 250),
        };
        s.spyglass = match get("spyglass").as_deref().map(str::trim) {
            Some("override") => Spyglass::Override,
            Some("only_holding") => Spyglass::OnlyHolding,
            Some("only_carrying") => Spyglass::OnlyCarrying,
            _ => Spyglass::Combine,
        };
        s.overlay = match get("overlay").as_deref().map(str::trim) {
            Some("never") => Overlay::Never,
            Some("carrying") => Overlay::Carrying,
            Some("always") => Overlay::Always,
            _ => Overlay::Holding,
        };
        s.sounds = match get("sounds").as_deref().map(str::trim) {
            Some("never") => Sounds::Never,
            Some("always") => Sounds::Always,
            Some("only_spyglass") => Sounds::OnlySpyglass,
            _ => Sounds::WithOverlay,
        };
        s.secondary_amount = int("secondary_amount", s.secondary_amount, 1, 50);
        s.secondary_in_time = secs("secondary_in_time", s.secondary_in_time);
        s.secondary_out_time = secs("secondary_out_time", s.secondary_out_time);
        s.secondary_hide_hud = flag("secondary_hide_hud", s.secondary_hide_hud);
        s
    }

    pub fn stored(&self) -> Vec<(&'static str, String)> {
        let spyglass = match self.spyglass {
            Spyglass::Combine => "combine",
            Spyglass::Override => "override",
            Spyglass::OnlyHolding => "only_holding",
            Spyglass::OnlyCarrying => "only_carrying",
        };
        let overlay = match self.overlay {
            Overlay::Never => "never",
            Overlay::Holding => "holding",
            Overlay::Carrying => "carrying",
            Overlay::Always => "always",
        };
        let sounds = match self.sounds {
            Sounds::Never => "never",
            Sounds::Always => "always",
            Sounds::OnlySpyglass => "only_spyglass",
            Sounds::WithOverlay => "with_overlay",
        };
        let key = match self.key_behaviour {
            KeyBehaviour::Hold => "hold",
            KeyBehaviour::Toggle => "toggle",
        };
        vec![
            ("initial_zoom", self.initial_zoom.to_string()),
            ("zoom_in_time", self.zoom_in_time.to_string()),
            ("zoom_out_time", self.zoom_out_time.to_string()),
            ("zoom_in_transition", self.zoom_in_transition.stored().into()),
            ("zoom_out_transition", self.zoom_out_transition.stored().into()),
            ("affect_hand_fov", self.affect_hand_fov.to_string()),
            ("retain_zoom_steps", self.retain_zoom_steps.to_string()),
            ("scroll_zoom", self.scroll_zoom.to_string()),
            ("scroll_step_count", self.scroll_step_count.to_string()),
            ("zoom_per_step", self.zoom_per_step.to_string()),
            ("scroll_smoothness", self.scroll_smoothness.to_string()),
            ("key_behaviour", key.into()),
            ("keybind_scrolling", self.keybind_scrolling.to_string()),
            ("relative_sensitivity", self.relative_sensitivity.to_string()),
            ("cinematic_camera", self.cinematic_camera.to_string()),
            ("spyglass", spyglass.into()),
            ("overlay", overlay.into()),
            ("sounds", sounds.into()),
            ("secondary_amount", self.secondary_amount.to_string()),
            ("secondary_in_time", self.secondary_in_time.to_string()),
            ("secondary_out_time", self.secondary_out_time.to_string()),
            ("secondary_hide_hud", self.secondary_hide_hud.to_string()),
        ]
    }

    /// The settings page, in `click`'s order: the settings, then the presets.
    pub fn labels(&self, ru: bool) -> Vec<String> {
        let on = |b: bool| match (b, ru) {
            (true, true) => "вкл",
            (false, true) => "выкл",
            (true, false) => "On",
            (false, false) => "Off",
        };
        let secs = |t: f64| {
            if t <= 0.0 {
                if ru { "мгновенно".to_string() } else { "Instant".to_string() }
            } else if ru {
                format!("{t} с")
            } else {
                format!("{t} secs")
            }
        };
        let times = |x: u32| format!("{}x", f64::from(x) / 100.0);
        let pick = |r: &'static str, e: &'static str| if ru { r } else { e };
        let key = match self.key_behaviour {
            KeyBehaviour::Hold => pick("удерживать", "Hold"),
            KeyBehaviour::Toggle => pick("переключать", "Toggle"),
        };
        let spyglass = match self.spyglass {
            Spyglass::Combine => pick("вместе с зумом", "Combine"),
            Spyglass::Override => pick("заменять зумом", "Override"),
            Spyglass::OnlyHolding => pick("зум только с трубой в руке", "Allow While Holding"),
            Spyglass::OnlyCarrying => pick("зум только с трубой в инвентаре", "Allow While Carrying"),
        };
        let overlay = match self.overlay {
            Overlay::Never => pick("никогда", "Never"),
            Overlay::Holding => pick("с трубой в руке", "Holding"),
            Overlay::Carrying => pick("с трубой в инвентаре", "Carrying"),
            Overlay::Always => pick("всегда", "Always"),
        };
        let sounds = match self.sounds {
            Sounds::Never => pick("никогда", "Never"),
            Sounds::Always => pick("всегда", "Always"),
            Sounds::OnlySpyglass => pick("только с трубой", "Only Holding Spyglass"),
            Sounds::WithOverlay => pick("как оверлей", "Match Overlay"),
        };
        let t = |r: &str, e: &str| if ru { r.to_string() } else { e.to_string() };
        vec![
            format!("{}: {}x", t("Начальный зум", "Initial Zoom"), self.initial_zoom),
            format!("{}: {}", t("Время приближения", "Zoom In Time"), secs(self.zoom_in_time)),
            format!("{}: {}", t("Время отдаления", "Zoom Out Time"), secs(self.zoom_out_time)),
            format!("{}: {}", t("Переход приближения", "Zoom In Transition"), self.zoom_in_transition.label(ru)),
            format!("{}: {}", t("Переход отдаления", "Zoom Out Transition"), self.zoom_out_transition.label(ru)),
            format!("{}: {}", t("Зум затрагивает руку", "Affect Hand FOV"), on(self.affect_hand_fov)),
            format!("{}: {}", t("Клавиша зума", "Zoom Key Behavior"), key),
            format!("{}: {}", t("Зум колесом", "Scroll Zoom"), on(self.scroll_zoom)),
            format!("{}: {}", t("Шагов колеса", "Scroll Step Count"), self.scroll_step_count),
            format!("{}: {}", t("Зум за шаг", "Zoom Per Step"), times(self.zoom_per_step)),
            format!("{}: {}%", t("Плавность колеса", "Scroll Zoom Smoothness"), self.scroll_smoothness),
            format!("{}: {}", t("Запоминать шаги колеса", "Remember Zoom Steps"), on(self.retain_zoom_steps)),
            format!("{}: {}", t("Колесо клавишами", "Keybind Scrolling"), on(self.keybind_scrolling)),
            format!("{}: {}%", t("Относительная чувствительность", "Relative Sensitivity"), self.relative_sensitivity),
            format!("{}: {}", t("Кинокамера при зуме", "Cinematic Camera"), on(self.cinematic_camera > 0)),
            format!("{}: {}", t("Подзорная труба", "Spyglass"), spyglass),
            format!("{}: {}", t("Оверлей трубы", "Spyglass Overlay"), overlay),
            format!("{}: {}", t("Звуки трубы", "Spyglass Sounds"), sounds),
            format!("{}: {}x", t("Второй зум", "Secondary Zoom Amount"), self.secondary_amount),
            format!("{}: {}", t("Второй зум, приближение", "Secondary Zoom In Time"), secs(self.secondary_in_time)),
            format!("{}: {}", t("Второй зум, отдаление", "Secondary Zoom Out Time"), secs(self.secondary_out_time)),
            format!("{}: {}", t("Второй зум скрывает HUD", "Hide HUD On Secondary Zoom"), on(self.secondary_hide_hud)),
            t("Пресет: по умолчанию", "Preset: Default"),
            t("Пресет: OptiFine", "Preset: OptiFine"),
            t("Пресет: OK Zoomer", "Preset: OK Zoomer"),
        ]
    }

    pub fn click(&mut self, button: usize) {
        match button {
            0 => self.initial_zoom = next(&INITIAL_ZOOMS, self.initial_zoom),
            1 => self.zoom_in_time = next(&ZOOM_TIMES, self.zoom_in_time),
            2 => self.zoom_out_time = next(&ZOOM_TIMES, self.zoom_out_time),
            3 => self.zoom_in_transition = next(&Transition::ALL, self.zoom_in_transition),
            4 => self.zoom_out_transition = next(&Transition::ALL, self.zoom_out_transition),
            5 => self.affect_hand_fov = !self.affect_hand_fov,
            6 => {
                self.key_behaviour = match self.key_behaviour {
                    KeyBehaviour::Hold => KeyBehaviour::Toggle,
                    KeyBehaviour::Toggle => KeyBehaviour::Hold,
                }
            }
            7 => self.scroll_zoom = !self.scroll_zoom,
            8 => self.scroll_step_count = next(&SCROLL_STEPS, self.scroll_step_count),
            9 => self.zoom_per_step = next(&PER_STEP, self.zoom_per_step),
            10 => self.scroll_smoothness = next(&SMOOTHNESS, self.scroll_smoothness),
            11 => self.retain_zoom_steps = !self.retain_zoom_steps,
            12 => self.keybind_scrolling = !self.keybind_scrolling,
            13 => self.relative_sensitivity = next(&SENSITIVITY, self.relative_sensitivity),
            14 => self.cinematic_camera = if self.cinematic_camera > 0 { 0 } else { 100 },
            15 => self.spyglass = next(&SPYGLASS, self.spyglass),
            16 => self.overlay = next(&OVERLAY, self.overlay),
            17 => self.sounds = next(&SOUNDS, self.sounds),
            18 => self.secondary_amount = next(&SECONDARY_ZOOMS, self.secondary_amount),
            19 => self.secondary_in_time = next(&SECONDARY_IN, self.secondary_in_time),
            20 => self.secondary_out_time = next(&SECONDARY_OUT, self.secondary_out_time),
            21 => self.secondary_hide_hud = !self.secondary_hide_hud,
            22 => *self = Settings::default(),
            // Presets: Zoomify's OptiFine and OK Zoomer (their view bobbing
            // option has no counterpart here).
            23 => {
                *self = Settings {
                    zoom_in_transition: Transition::Instant,
                    zoom_out_transition: Transition::Instant,
                    scroll_zoom: false,
                    relative_sensitivity: 0,
                    cinematic_camera: 100,
                    ..Settings::default()
                }
            }
            24 => {
                *self = Settings {
                    zoom_in_time: 0.25,
                    zoom_out_time: 0.25,
                    relative_sensitivity: 50,
                    scroll_smoothness: 25,
                    ..Settings::default()
                }
            }
            _ => {}
        }
    }
}

static SETTINGS: Mutex<Option<Settings>> = Mutex::new(None);

pub fn settings(get: impl Fn(&str) -> Option<String>) -> Settings {
    let mut kept = SETTINGS.lock().unwrap_or_else(|e| e.into_inner());
    *kept.get_or_insert_with(|| Settings::read(get))
}

pub fn set_settings(settings: Settings) {
    *SETTINGS.lock().unwrap_or_else(|e| e.into_inner()) = Some(settings);
}

// ---- the zoom ------------------------------------------------------------------

/// What the frame brings: the keys and the player's spyglass.
#[derive(Clone, Copy, Default, Debug)]
pub struct Input {
    /// The zoom key is held (and the game has the keyboard).
    pub key_down: bool,
    /// Presses of the zoom key, the secondary zoom key and the scroll keys.
    pub presses: u32,
    pub secondary_presses: u32,
    pub zoom_in_presses: i32,
    pub zoom_out_presses: i32,
    /// The local player as the camera: none while spectating a mob.
    pub spyglass: Option<SpyglassState>,
}

#[derive(Clone, Copy, Default, Debug)]
pub struct SpyglassState {
    pub holding: bool,
    pub carrying: bool,
    pub scoping: bool,
    pub first_person: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sound {
    Use,
    Stop,
}

/// Zoomify's state (`Zoomify` object).
#[derive(Clone, Debug)]
pub struct Zoom {
    pub zooming: bool,
    pub secondary: bool,
    pub steps: i32,
    regular: Helper,
    second: Helper,
    /// The main zoom's divisor (the original's `previousZoomDivisor`), which
    /// the relative sensitivity follows.
    pub regular_divisor: f64,
    /// Both zooms together: what the field of view is divided by.
    pub divisor: f64,
    play_sound: bool,
}

impl Zoom {
    pub fn new(s: &Settings) -> Zoom {
        Zoom {
            zooming: false,
            secondary: false,
            steps: 0,
            regular: Helper::new(&s.regular()),
            second: Helper::new(&s.secondary()),
            regular_divisor: 1.0,
            divisor: 1.0,
            play_sound: false,
        }
    }

    /// One frame of `dt` seconds: the original's tick, then its
    /// `getZoomDivisor`.
    pub fn frame(&mut self, s: &Settings, input: Input, dt: f64) -> Vec<Sound> {
        let previous = self.zooming;
        match s.key_behaviour {
            KeyBehaviour::Hold => self.zooming = input.key_down,
            KeyBehaviour::Toggle => {
                if input.presses % 2 == 1 {
                    self.zooming = !self.zooming;
                }
            }
        }
        if input.secondary_presses % 2 == 1 {
            self.secondary = !self.secondary;
        }
        let regular = s.regular();
        if s.keybind_scrolling {
            self.steps += input.zoom_in_presses - input.zoom_out_presses;
            self.steps = self.steps.clamp(regular.min_tiers(), regular.max_tiers);
        }
        let sounds = match input.spyglass {
            Some(spy) => self.spyglass(s, spy, previous),
            None => Vec::new(),
        };
        let dt = dt.clamp(0.0, 0.25);
        self.regular.tick(self.zooming, self.steps, dt, &regular);
        let secondary = s.secondary();
        self.second.tick(self.secondary, 0, dt, &secondary);
        if !self.zooming {
            if !s.retain_zoom_steps {
                self.steps = 0;
            }
            self.regular.reset();
        }
        self.regular_divisor = self.regular.divisor(&regular);
        self.divisor = self.regular_divisor * self.second.divisor(&secondary);
        sounds
    }

    /// A wheel turn while zooming (`mouseZoom`): one step either way.
    pub fn mouse_zoom(&mut self, s: &Settings, delta: i32) {
        let c = s.regular();
        self.steps += delta.signum();
        self.steps = self.steps.clamp(c.min_tiers(), c.max_tiers);
    }

    fn spyglass(&mut self, s: &Settings, spy: SpyglassState, previous: bool) -> Vec<Sound> {
        match s.spyglass {
            Spyglass::OnlyHolding if !spy.holding => self.zooming = false,
            Spyglass::OnlyCarrying if !spy.carrying => self.zooming = false,
            Spyglass::Override if spy.scoping => self.zooming = self.zooming && spy.first_person,
            _ => {}
        }
        let requires = s.spyglass != Spyglass::Combine;
        if requires && spy.scoping {
            self.zooming = true;
        }
        let mut sounds = Vec::new();
        if self.play_sound && !self.zooming && previous {
            sounds.push(Sound::Stop);
        }
        self.play_sound = match s.sounds {
            Sounds::Never => false,
            Sounds::Always => true,
            Sounds::OnlySpyglass => spy.scoping || (requires && self.zooming && spy.holding),
            Sounds::WithOverlay => {
                self.overlay(s, spy.holding, spy.carrying, spy.first_person && spy.scoping) && requires
            }
        };
        if self.play_sound && self.zooming && !previous {
            sounds.push(Sound::Use);
        }
        sounds
    }

    /// Whether the spyglass overlay shows (`shouldRenderOverlay`).
    pub fn overlay(&self, s: &Settings, holding: bool, carrying: bool, using_spyglass: bool) -> bool {
        match s.overlay {
            Overlay::Never => false,
            Overlay::Always => self.zooming,
            Overlay::Holding => {
                using_spyglass || (self.zooming && holding && s.spyglass != Spyglass::Combine)
            }
            Overlay::Carrying => self.zooming && carrying,
        }
    }

    /// The mouse sensitivity option while zooming (`applyRelativeSensitivity`).
    pub fn sensitivity(&self, s: &Settings, value: f64) -> f64 {
        let t = f64::from(s.relative_sensitivity) / 100.0;
        value / (1.0 + t * (self.regular_divisor - 1.0))
    }

    /// The cinematic camera is on while zooming (`smoothCameraIfZoom`).
    pub fn smooth_camera(&self, s: &Settings, original: bool) -> bool {
        original || self.secondary || (self.zooming && s.cinematic_camera > 0)
    }
}

static ZOOM: Mutex<Option<Zoom>> = Mutex::new(None);

/// The zoom's state, made with the settings on first use.
pub fn with_zoom<R>(s: &Settings, f: impl FnOnce(&mut Zoom) -> R) -> R {
    let mut kept = ZOOM.lock().unwrap_or_else(|e| e.into_inner());
    f(kept.get_or_insert_with(|| Zoom::new(s)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(z: &mut Zoom, s: &Settings, input: Input, seconds: f64) -> Vec<Sound> {
        let mut sounds = Vec::new();
        let frames = (seconds / 0.016).round() as usize;
        for _ in 0..frames {
            sounds.extend(z.frame(s, input, 0.016));
        }
        sounds
    }

    fn held() -> Input {
        Input { key_down: true, ..Input::default() }
    }

    #[test]
    fn transitions_reach_their_ends_and_invert() {
        for t in Transition::ALL {
            assert!(t.apply(0.0).abs() < 1e-9, "{t:?}");
            assert!((t.apply(1.0) - 1.0).abs() < 1e-9, "{t:?}");
            let mut last = 0.0;
            for i in 0..=100 {
                let y = t.apply(f64::from(i) / 100.0);
                assert!(y >= last - 1e-9, "{t:?} at {i}");
                last = y;
            }
            if t.has_inverse() {
                for i in 1..100 {
                    let x = f64::from(i) / 100.0;
                    assert!((t.inverse(t.apply(x)) - x).abs() < 1e-9, "{t:?} {x}");
                }
            }
            assert_eq!(t.opposite().opposite(), t);
        }
        // The default zoom in eases out: half way in time is nearly all the way.
        assert!(Transition::EaseOutExp.apply(0.5) > 0.96);
    }

    #[test]
    fn holding_the_key_zooms_to_the_initial_zoom_in_its_time() {
        let s = Settings::default();
        let mut z = Zoom::new(&s);
        run(&mut z, &s, held(), 0.5);
        assert!(z.divisor > 1.5 && z.divisor < 4.0, "{}", z.divisor);
        run(&mut z, &s, held(), 0.6);
        assert!((z.divisor - 4.0).abs() < 1e-9, "{}", z.divisor);
        // Let go: out in half a second.
        run(&mut z, &s, Input::default(), 0.25);
        assert!(z.divisor > 1.0 && z.divisor < 4.0);
        run(&mut z, &s, Input::default(), 0.3);
        assert!((z.divisor - 1.0).abs() < 1e-9, "{}", z.divisor);
    }

    #[test]
    fn letting_go_halfway_turns_back_without_a_jump() {
        let s = Settings::default();
        let mut z = Zoom::new(&s);
        run(&mut z, &s, held(), 0.3);
        let before = z.divisor;
        z.frame(&s, Input::default(), 0.0);
        assert!((z.divisor - before).abs() < 0.02, "{before} -> {}", z.divisor);
        run(&mut z, &s, Input::default(), 0.2);
        let mid = z.divisor;
        z.frame(&s, held(), 0.0);
        assert!((z.divisor - mid).abs() < 0.02, "{mid} -> {}", z.divisor);
    }

    #[test]
    fn the_wheel_zooms_by_steps_and_lets_go_smoothly() {
        let s = Settings::default();
        let mut z = Zoom::new(&s);
        run(&mut z, &s, held(), 1.2);
        for _ in 0..3 {
            z.mouse_zoom(&s, 1);
        }
        run(&mut z, &s, held(), 2.0);
        assert!((z.divisor - 4.0 * 1.5f64.powi(3)).abs() < 0.01, "{}", z.divisor);
        // At most 10 steps in; out no further than the normal view.
        for _ in 0..30 {
            z.mouse_zoom(&s, 1);
        }
        assert_eq!(z.steps, 10);
        for _ in 0..30 {
            z.mouse_zoom(&s, -1);
        }
        assert_eq!(z.steps, -4);
        run(&mut z, &s, held(), 3.0);
        assert!((z.divisor - 1.0).abs() < 0.02, "{}", z.divisor);
        // Zoomed in by the wheel, then let go: no jump, back to 1.
        for _ in 0..8 {
            z.mouse_zoom(&s, 1);
        }
        run(&mut z, &s, held(), 3.0);
        let before = z.divisor;
        z.frame(&s, Input::default(), 0.0);
        assert!((z.divisor - before).abs() / before < 0.01, "{before} -> {}", z.divisor);
        assert_eq!(z.steps, 0);
        run(&mut z, &s, Input::default(), 0.6);
        assert!((z.divisor - 1.0).abs() < 1e-9, "{}", z.divisor);
    }

    #[test]
    fn toggle_and_secondary_zoom() {
        let mut s = Settings { key_behaviour: KeyBehaviour::Toggle, ..Settings::default() };
        let mut z = Zoom::new(&s);
        z.frame(&s, Input { presses: 1, ..Input::default() }, 0.016);
        run(&mut z, &s, Input::default(), 1.2);
        assert!((z.divisor - 4.0).abs() < 1e-9);
        z.frame(&s, Input { presses: 1, ..Input::default() }, 0.016);
        run(&mut z, &s, Input::default(), 0.6);
        assert!((z.divisor - 1.0).abs() < 1e-9);
        // The secondary zoom: linear over ten seconds, then multiplied in.
        s.key_behaviour = KeyBehaviour::Hold;
        z.frame(&s, Input { secondary_presses: 1, ..Input::default() }, 0.0);
        assert!(z.secondary);
        run(&mut z, &s, Input::default(), 5.0);
        assert!((z.divisor - 1.0 / (1.0 + 0.5 * (0.25 - 1.0))).abs() < 0.02, "{}", z.divisor);
        run(&mut z, &s, Input::default(), 5.1);
        assert!((z.divisor - 4.0).abs() < 1e-9);
        run(&mut z, &s, held(), 1.2);
        assert!((z.divisor - 16.0).abs() < 1e-6);
        assert!(z.smooth_camera(&s, false));
    }

    #[test]
    fn spyglass_rules() {
        let s = Settings { spyglass: Spyglass::OnlyHolding, ..Settings::default() };
        let mut z = Zoom::new(&s);
        let empty = SpyglassState { first_person: true, ..SpyglassState::default() };
        run(&mut z, &s, Input { key_down: true, spyglass: Some(empty), ..Input::default() }, 1.2);
        assert!((z.divisor - 1.0).abs() < 1e-9, "no spyglass, no zoom");
        let holding = SpyglassState { holding: true, carrying: true, ..empty };
        let sounds = run(&mut z, &s, Input { key_down: true, spyglass: Some(holding), ..Input::default() }, 1.2);
        assert!((z.divisor - 4.0).abs() < 1e-9);
        // Default sounds match the overlay, which shows holding one.
        assert_eq!(sounds, vec![Sound::Use]);
        assert!(z.overlay(&s, true, true, false));
        let sounds = run(&mut z, &s, Input { spyglass: Some(holding), ..Input::default() }, 0.1);
        assert_eq!(sounds, vec![Sound::Stop]);
        // Using the spyglass itself zooms with Zoomify when it is required.
        let scoping = SpyglassState { scoping: true, ..holding };
        run(&mut z, &s, Input { spyglass: Some(scoping), ..Input::default() }, 1.2);
        assert!((z.divisor - 4.0).abs() < 1e-9);
        // Combine (the default) leaves the spyglass alone and plays nothing.
        let d = Settings::default();
        let mut c = Zoom::new(&d);
        let sounds = run(&mut c, &d, Input { key_down: true, spyglass: Some(holding), ..Input::default() }, 1.2);
        assert!(sounds.is_empty());
        assert!(!c.overlay(&d, true, true, false));
        assert!(c.overlay(&d, true, true, true));
    }

    #[test]
    fn sensitivity_follows_the_zoom() {
        let s = Settings::default();
        let mut z = Zoom::new(&s);
        run(&mut z, &s, held(), 1.2);
        assert!((z.sensitivity(&s, 0.5) - 0.125).abs() < 1e-9);
        let half = Settings { relative_sensitivity: 50, ..s };
        assert!((z.sensitivity(&half, 0.5) - 0.2).abs() < 1e-9);
        let off = Settings { relative_sensitivity: 0, ..s };
        assert!((z.sensitivity(&off, 0.5) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn instant_transitions_and_presets() {
        let mut s = Settings::default();
        s.click(23);
        assert_eq!(s.zoom_in_transition, Transition::Instant);
        assert!(!s.scroll_zoom && s.cinematic_camera == 100);
        assert_eq!(Settings::preset(1), s);
        let mut z = Zoom::new(&s);
        z.frame(&s, held(), 0.016);
        assert!((z.divisor - 4.0).abs() < 1e-9);
        z.frame(&s, Input::default(), 0.016);
        assert!((z.divisor - 1.0).abs() < 1e-9);
        s.click(24);
        assert_eq!((s.zoom_in_time, s.relative_sensitivity, s.scroll_smoothness), (0.25, 50, 25));
        s.click(22);
        assert_eq!(s, Settings::default());
        let labels = Settings::default().labels(true);
        assert_eq!(labels.len(), ROWS + PRESETS);
        let mut t = Settings::default();
        for i in 0..ROWS {
            t.click(i);
        }
        let stored: std::collections::HashMap<_, _> = t.stored().into_iter().collect();
        assert_eq!(Settings::read(|k| stored.get(k).cloned()), t);
    }
}
