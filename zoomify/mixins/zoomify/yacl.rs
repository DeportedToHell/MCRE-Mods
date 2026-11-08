// Zoomify's config screen as YetAnotherConfigLib 3.9.7 draws it (`YACLScreen`,
// which Zoomify's `SettingsGuiFactory` fills): the categories' tabs along the
// top (`ScrollableNavigationBar` over vanilla's `MenuTabBar`), the options'
// list in the left two thirds (`OptionListWidget`: collapsible groups, tick
// boxes, cycling enums, sliders, labels and buttons, each with its reset
// button), and on the right the hovered option's description with Zoomify's
// demo picture (`OptionDescriptionWidget`), the search field and the Undo,
// Reset/Cancel and Done/Save buttons. Changes stay pending until Save, as
// YACL keeps them (`StateManager`).

use super::zoomify::{self as zf, KeyBehaviour, Settings, Transition};
use super::zoomify_demo::{Cfg, Control, Demo, Pair, Shared};
use super::Game;
use horizon_client::hud::{rgba, HudAtlas, HudBatch};
use horizon_client::text::{Style, Text};
use horizon_client::ui::{self, EditBox, Scroll};
use std::time::{Duration, Instant};

// ---- texts ---------------------------------------------------------------------------

/// English and Russian, from Zoomify's and YACL's `en_us.json` and `ru_ru.json`
/// (Zoomify's Russian file keeps most of its texts in English).
type Tr = (&'static str, &'static str);

const fn same(s: &'static str) -> Tr {
    (s, s)
}

fn pick(t: Tr, ru: bool) -> &'static str {
    if ru { t.1 } else { t.0 }
}

const SAVE: Tr = ("Save Changes", "Сохранить");
const SAVE_TIP: Tr = ("Makes the changes made permanent.", "Сохранить изменения до следующего редактирования.");
const FINISHED_TIP: Tr = ("Closes the GUI.", "Закрыть меню.");
const CANCEL_TIP: Tr = ("Forgets pending changes and closes the GUI.", "Отменить изменения и закрыть настройки.");
const RESET_TIP: Tr = (
    "Resets all options to default. (This is reversible!)",
    "Сбросить все настройки до значений по умолчанию (их можно восстановить).",
);
const UNDO: Tr = ("Undo", "Отменить");
const UNDO_TIP: Tr = (
    "Reverts all options back to what they were before editing.",
    "Вернуть все настройки к состоянию, в котором они были до изменений.",
);
const SAVE_BEFORE_EXIT: Tr = ("Save before exiting!", "Сохраните перед закрытием");
const SAVE_BEFORE_EXIT_TIP: Tr = ("Save or cancel to exit the GUI.", "Сохраните или отмените изменения, чтобы закрыть настройки.");

const INSTANT: Tr = same("Instant");
const SECONDS: Tr = same("%s secs");
const PRESET_DESC: Tr = same("Makes Zoomify behave like %s.");
const APPLY_PRESET: Tr = same("Apply Preset");
const PRESET_TOAST: Tr = same("Preset Applied");
const PRESET_TOAST_DESC: Tr = same("Zoomify applied the %s preset.");
/// A label option's name (`LabelOptionImpl`), never translated.
const LABEL_NAME: Tr = same("Label Option");

const TRANSITIONS: [Tr; 14] = [
    same("Instant"),
    same("Linear"),
    same("Ease In Sine"),
    same("Ease Out Sine"),
    same("Ease In Out Sine"),
    same("Ease In Quad"),
    same("Ease Out Quad"),
    same("Ease In Out Quad"),
    same("Ease In Cubic"),
    same("Ease Out Cubic"),
    same("Ease In Out Cubic"),
    same("Ease In Exponential"),
    same("Ease Out Exponential"),
    same("Ease In Out Exponential"),
];
const KEY_BEHAVIOURS: [Tr; 2] = [("Hold", "Удерживать"), ("Toggle", "Переключать")];
const SPYGLASS_BEHAVIOURS: [Tr; 4] =
    [same("Combine"), same("Override"), same("Allow While Holding"), same("Allow While Carrying")];
const OVERLAY_VISIBILITIES: [Tr; 4] = [same("Never"), same("Holding"), same("Carrying"), same("Always")];
const SOUND_BEHAVIOURS: [Tr; 4] = [same("Never"), same("Always"), same("Only Holding Spyglass"), same("Match Overlay")];
const PRESET_NAMES: [Tr; 3] = [same("Default"), same("OptiFine"), same("OK Zoomer")];

// ---- the options -----------------------------------------------------------------------

/// Every option of the screen, in the order YACL walks them; the first
/// `VALUES` hold a value.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Opt {
    InitialZoom,
    ZoomInTime,
    ZoomOutTime,
    ZoomInTransition,
    ZoomOutTransition,
    AffectHandFov,
    ScrollZoom,
    ScrollStepCount,
    ZoomPerStep,
    ScrollSmoothness,
    RetainZoomSteps,
    SpyglassBehaviour,
    OverlayVisibility,
    SoundBehaviour,
    ZoomKeyBehaviour,
    KeybindScrolling,
    RelativeSensitivity,
    CinematicCamera,
    SecondaryAmount,
    SecondaryInTime,
    SecondaryOutTime,
    SecondaryHideHud,
    InfoLabel,
    ApplyWarning,
    PresetDefault,
    PresetOptifine,
    PresetOkZoomer,
}

const VALUES: usize = 22;
const VALUE_OPTS: [Opt; VALUES] = [
    Opt::InitialZoom,
    Opt::ZoomInTime,
    Opt::ZoomOutTime,
    Opt::ZoomInTransition,
    Opt::ZoomOutTransition,
    Opt::AffectHandFov,
    Opt::ScrollZoom,
    Opt::ScrollStepCount,
    Opt::ZoomPerStep,
    Opt::ScrollSmoothness,
    Opt::RetainZoomSteps,
    Opt::SpyglassBehaviour,
    Opt::OverlayVisibility,
    Opt::SoundBehaviour,
    Opt::ZoomKeyBehaviour,
    Opt::KeybindScrolling,
    Opt::RelativeSensitivity,
    Opt::CinematicCamera,
    Opt::SecondaryAmount,
    Opt::SecondaryInTime,
    Opt::SecondaryOutTime,
    Opt::SecondaryHideHud,
];
/// The scrolling group's options that follow "Enable Scroll Zoom".
const SCROLL_INNER: [Opt; 4] = [Opt::ScrollStepCount, Opt::ZoomPerStep, Opt::ScrollSmoothness, Opt::RetainZoomSteps];

/// How a slider shows its value (Zoomify's formatters).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Fmt {
    /// `"%dx"`.
    Times,
    /// YACL's own `"%,d"`.
    Count,
    /// `"%.1fx"` of the percentage.
    PerStep,
    /// `"%d%%"`, "Instant" at 0.
    PercentInstant,
    /// `"%d%%"`, vanilla's "OFF" at 0.
    PercentOff,
    /// `"%.1f secs"`.
    Seconds,
    /// `"%.2f secs"`, "Instant" at 0.
    SecondsInstant,
}

#[derive(Clone, Copy, Debug)]
enum Kind {
    /// `tickBox()`.
    Tick,
    /// `enumSwitch()`.
    Cycle(&'static [Tr]),
    /// `slider(range, step, formatter)`.
    Slider { min: f64, max: f64, step: f64, int: bool, fmt: Fmt },
    /// `registeringLabel`.
    Label(Tr),
    /// `registerButton`: a preset.
    Button(usize),
}

/// Which demo picture an option's description shows, and whose
/// `updateDemo` listener it has.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum DemoId {
    Initial,
    Scroll,
    Secondary,
    Preset(usize),
}

struct Def {
    opt: Opt,
    name: Tr,
    desc: Tr,
    kind: Kind,
    demo: Option<DemoId>,
}

struct GroupDef {
    /// None: the category's root options, with no separator.
    name: Option<Tr>,
    opts: &'static [Def],
}

struct CatDef {
    name: Tr,
    groups: &'static [GroupDef],
}

const fn slider(min: f64, max: f64, step: f64, int: bool, fmt: Fmt) -> Kind {
    Kind::Slider { min, max, step, int, fmt }
}

const BASIC: [Def; 6] = [
    Def {
        opt: Opt::InitialZoom,
        name: same("Initial Zoom"),
        desc: same("The zoom level when you first press the keybind."),
        kind: slider(1.0, 10.0, 1.0, true, Fmt::Times),
        demo: Some(DemoId::Initial),
    },
    Def {
        opt: Opt::ZoomInTime,
        name: same("Zoom In Time"),
        desc: same("How many seconds to complete zooming in."),
        kind: slider(0.1, 5.0, 0.1, false, Fmt::Seconds),
        demo: Some(DemoId::Initial),
    },
    Def {
        opt: Opt::ZoomOutTime,
        name: same("Zoom Out Time"),
        desc: same("How many seconds to complete zooming out."),
        kind: slider(0.1, 5.0, 0.1, false, Fmt::Seconds),
        demo: Some(DemoId::Initial),
    },
    Def {
        opt: Opt::ZoomInTransition,
        name: same("Zoom In Transition"),
        desc: same("The transition type used when zooming in."),
        kind: Kind::Cycle(&TRANSITIONS),
        demo: Some(DemoId::Initial),
    },
    Def {
        opt: Opt::ZoomOutTransition,
        name: same("Zoom Out Transition"),
        desc: same("The transition type used when zooming out."),
        kind: Kind::Cycle(&TRANSITIONS),
        demo: Some(DemoId::Initial),
    },
    Def {
        opt: Opt::AffectHandFov,
        name: same("Affect Hand FOV"),
        desc: same("When zooming, keep the FOV of the hand normal while the world view zooms in."),
        kind: Kind::Tick,
        demo: Some(DemoId::Initial),
    },
];

const SCROLLING: [Def; 5] = [
    Def {
        opt: Opt::ScrollZoom,
        name: same("Enable Scroll Zoom"),
        desc: same("Allow you to zoom in and out more with scroll wheel."),
        kind: Kind::Tick,
        demo: Some(DemoId::Scroll),
    },
    Def {
        opt: Opt::ScrollStepCount,
        name: same("Scroll Step Count"),
        desc: same("How many scroll steps in each direction. You can scroll to zoom in or out from the initial zoom."),
        kind: slider(3.0, 20.0, 1.0, true, Fmt::Count),
        demo: Some(DemoId::Scroll),
    },
    Def {
        opt: Opt::ZoomPerStep,
        name: same("Zoom Per Step"),
        desc: same("The zoom multiplier applied per scroll step. 1.5x means each step zooms in 50% more."),
        kind: slider(110.0, 200.0, 10.0, true, Fmt::PerStep),
        demo: Some(DemoId::Scroll),
    },
    Def {
        opt: Opt::ScrollSmoothness,
        name: same("Scroll Zoom Smoothness"),
        desc: same("How smooth/how long it takes to scroll-zoom."),
        kind: slider(0.0, 100.0, 1.0, true, Fmt::PercentInstant),
        demo: Some(DemoId::Scroll),
    },
    Def {
        opt: Opt::RetainZoomSteps,
        name: same("Remember Zoom Steps"),
        desc: same("Keep scroll zoom amount between zooming in and out."),
        kind: Kind::Tick,
        demo: Some(DemoId::Scroll),
    },
];

const SPYGLASS: [Def; 3] = [
    Def {
        opt: Opt::SpyglassBehaviour,
        name: same("Zoom Behavior"),
        desc: same("How Zoomify interacts with spyglass zoom."),
        kind: Kind::Cycle(&SPYGLASS_BEHAVIOURS),
        demo: None,
    },
    Def {
        opt: Opt::OverlayVisibility,
        name: same("Overlay Visibility"),
        desc: same("When to show the spyglass overlay."),
        kind: Kind::Cycle(&OVERLAY_VISIBILITIES),
        demo: None,
    },
    Def {
        opt: Opt::SoundBehaviour,
        name: same("Sound Behavior"),
        desc: same("When to play the zoom in and out sounds from the spyglass."),
        kind: Kind::Cycle(&SOUND_BEHAVIOURS),
        demo: None,
    },
];

// "Relative View Bobbing" is left out: Zoomify 2.17 shows it but reads it
// nowhere, and this port has nothing for it to change either.
const CONTROLS: [Def; 4] = [
    Def {
        opt: Opt::ZoomKeyBehaviour,
        name: same("Zoom Key Behavior"),
        desc: same("The behavior of the zoom key."),
        kind: Kind::Cycle(&KEY_BEHAVIOURS),
        demo: None,
    },
    Def {
        opt: Opt::KeybindScrolling,
        name: same("Keybind Scrolling"),
        desc: same("Use Minecraft keybinds to control scroll zoom instead of the scroll wheel."),
        kind: Kind::Tick,
        demo: None,
    },
    Def {
        opt: Opt::RelativeSensitivity,
        name: same("Relative Sensitivity"),
        desc: same("Controls the sensitivity based on the zoom level."),
        kind: slider(0.0, 150.0, 10.0, true, Fmt::PercentOff),
        demo: None,
    },
    Def {
        opt: Opt::CinematicCamera,
        name: same("Cinematic Camera"),
        desc: same("How much the camera glides once moved."),
        kind: slider(0.0, 250.0, 10.0, true, Fmt::PercentOff),
        demo: None,
    },
];

const INFO: Tr = same(
    "Completely separate from all other settings of Zoomify. Secondary zoom is toggle-only with a separate keybinding in controls.",
);
const SECONDARY: [Def; 5] = [
    Def { opt: Opt::InfoLabel, name: LABEL_NAME, desc: INFO, kind: Kind::Label(INFO), demo: None },
    Def {
        opt: Opt::SecondaryAmount,
        name: same("Zoom Amount"),
        desc: same("How many times to divide the current FOV once transition completes."),
        kind: slider(2.0, 10.0, 1.0, true, Fmt::Times),
        demo: Some(DemoId::Secondary),
    },
    Def {
        opt: Opt::SecondaryInTime,
        name: same("Zoom In Time"),
        desc: same("How many seconds to complete zooming in."),
        kind: slider(6.0, 30.0, 2.0, false, Fmt::Seconds),
        demo: Some(DemoId::Secondary),
    },
    Def {
        opt: Opt::SecondaryOutTime,
        name: same("Zoom Out Time"),
        desc: same("How many seconds to complete zooming out."),
        kind: slider(0.0, 5.0, 0.25, false, Fmt::SecondsInstant),
        demo: Some(DemoId::Secondary),
    },
    Def {
        opt: Opt::SecondaryHideHud,
        name: same("Hide HUD On Zoom"),
        desc: same("Stops rendering the HUD whilst zooming in, much like pressing F1."),
        kind: Kind::Tick,
        demo: Some(DemoId::Secondary),
    },
];

const WARNING: Tr = same("Applying a preset will immediately apply changes and undo any pending changes.");
const PRESETS: [Def; 4] = [
    Def { opt: Opt::ApplyWarning, name: LABEL_NAME, desc: WARNING, kind: Kind::Label(WARNING), demo: None },
    Def {
        opt: Opt::PresetDefault,
        name: PRESET_NAMES[0],
        desc: PRESET_DESC,
        kind: Kind::Button(0),
        demo: Some(DemoId::Preset(0)),
    },
    Def {
        opt: Opt::PresetOptifine,
        name: PRESET_NAMES[1],
        desc: PRESET_DESC,
        kind: Kind::Button(1),
        demo: Some(DemoId::Preset(1)),
    },
    Def {
        opt: Opt::PresetOkZoomer,
        name: PRESET_NAMES[2],
        desc: PRESET_DESC,
        kind: Kind::Button(2),
        demo: Some(DemoId::Preset(2)),
    },
];

const CATEGORIES: [CatDef; 4] = [
    CatDef {
        name: same("Behavior"),
        groups: &[
            GroupDef { name: Some(("Basic", "Основные")), opts: &BASIC },
            GroupDef { name: Some(same("Scrolling")), opts: &SCROLLING },
            GroupDef { name: Some(same("Spyglass")), opts: &SPYGLASS },
        ],
    },
    CatDef { name: ("Controls", "Управление"), groups: &[GroupDef { name: None, opts: &CONTROLS }] },
    CatDef { name: same("Secondary Zoom"), groups: &[GroupDef { name: None, opts: &SECONDARY }] },
    CatDef { name: ("Misc", "Прочее"), groups: &[GroupDef { name: Some(same("Presets")), opts: &PRESETS }] },
];

fn def(o: Opt) -> &'static Def {
    CATEGORIES
        .iter()
        .flat_map(|c| c.groups.iter())
        .flat_map(|g| g.opts.iter())
        .find(|d| d.opt == o)
        .unwrap_or(&BASIC[0])
}

fn index_of<T: PartialEq>(list: &[T], v: &T) -> f64 {
    list.iter().position(|x| x == v).unwrap_or(0) as f64
}

fn flag(b: bool) -> f64 {
    if b { 1.0 } else { 0.0 }
}

/// An option's value in `s` (YACL's binding getter).
fn get(s: &Settings, o: Opt) -> f64 {
    match o {
        Opt::InitialZoom => f64::from(s.initial_zoom),
        Opt::ZoomInTime => s.zoom_in_time,
        Opt::ZoomOutTime => s.zoom_out_time,
        Opt::ZoomInTransition => index_of(&Transition::ALL, &s.zoom_in_transition),
        Opt::ZoomOutTransition => index_of(&Transition::ALL, &s.zoom_out_transition),
        Opt::AffectHandFov => flag(s.affect_hand_fov),
        Opt::ScrollZoom => flag(s.scroll_zoom),
        Opt::ScrollStepCount => f64::from(s.scroll_step_count),
        Opt::ZoomPerStep => f64::from(s.zoom_per_step),
        Opt::ScrollSmoothness => f64::from(s.scroll_smoothness),
        Opt::RetainZoomSteps => flag(s.retain_zoom_steps),
        Opt::SpyglassBehaviour => index_of(&zf::SPYGLASS, &s.spyglass),
        Opt::OverlayVisibility => index_of(&zf::OVERLAY, &s.overlay),
        Opt::SoundBehaviour => index_of(&zf::SOUNDS, &s.sounds),
        Opt::ZoomKeyBehaviour => flag(s.key_behaviour == KeyBehaviour::Toggle),
        Opt::KeybindScrolling => flag(s.keybind_scrolling),
        Opt::RelativeSensitivity => f64::from(s.relative_sensitivity),
        Opt::CinematicCamera => f64::from(s.cinematic_camera),
        Opt::SecondaryAmount => f64::from(s.secondary_amount),
        Opt::SecondaryInTime => s.secondary_in_time,
        Opt::SecondaryOutTime => s.secondary_out_time,
        Opt::SecondaryHideHud => flag(s.secondary_hide_hud),
        _ => 0.0,
    }
}

/// Sets an option's value in `s` (the binding's setter).
fn put(s: &mut Settings, o: Opt, v: f64) {
    let int = v.round().max(0.0) as u32;
    let at = |len: usize| (v.round().max(0.0) as usize).min(len - 1);
    match o {
        Opt::InitialZoom => s.initial_zoom = int,
        Opt::ZoomInTime => s.zoom_in_time = v,
        Opt::ZoomOutTime => s.zoom_out_time = v,
        Opt::ZoomInTransition => s.zoom_in_transition = Transition::ALL[at(Transition::ALL.len())],
        Opt::ZoomOutTransition => s.zoom_out_transition = Transition::ALL[at(Transition::ALL.len())],
        Opt::AffectHandFov => s.affect_hand_fov = v != 0.0,
        Opt::ScrollZoom => s.scroll_zoom = v != 0.0,
        Opt::ScrollStepCount => s.scroll_step_count = int,
        Opt::ZoomPerStep => s.zoom_per_step = int,
        Opt::ScrollSmoothness => s.scroll_smoothness = int,
        Opt::RetainZoomSteps => s.retain_zoom_steps = v != 0.0,
        Opt::SpyglassBehaviour => s.spyglass = zf::SPYGLASS[at(zf::SPYGLASS.len())],
        Opt::OverlayVisibility => s.overlay = zf::OVERLAY[at(zf::OVERLAY.len())],
        Opt::SoundBehaviour => s.sounds = zf::SOUNDS[at(zf::SOUNDS.len())],
        Opt::ZoomKeyBehaviour => {
            s.key_behaviour = if v != 0.0 { KeyBehaviour::Toggle } else { KeyBehaviour::Hold }
        }
        Opt::KeybindScrolling => s.keybind_scrolling = v != 0.0,
        Opt::RelativeSensitivity => s.relative_sensitivity = int,
        Opt::CinematicCamera => s.cinematic_camera = int,
        Opt::SecondaryAmount => s.secondary_amount = int,
        Opt::SecondaryInTime => s.secondary_in_time = v,
        Opt::SecondaryOutTime => s.secondary_out_time = v,
        Opt::SecondaryHideHud => s.secondary_hide_hud = v != 0.0,
        _ => {}
    }
}

/// `String.format` with `HALF_UP` rounding, in the locale's decimal mark
/// (the Russian game as on a Russian system: a comma).
fn fixed(v: f64, places: usize, ru: bool) -> String {
    let m = 10f64.powi(places as i32);
    let s = format!("{:.*}", places, (v * m).round() / m);
    if ru { s.replace('.', ",") } else { s }
}

fn format_value(fmt: Fmt, v: f64, ru: bool) -> Text {
    let int = v as i64;
    let percent = |off: Text| if int == 0 { off } else { Text::literal(format!("{int}%")) };
    match fmt {
        Fmt::Times => Text::literal(format!("{int}x")),
        Fmt::Count => Text::literal(int.to_string()),
        Fmt::PerStep => Text::literal(format!("{}x", fixed(v / 100.0, 1, ru))),
        Fmt::PercentInstant => percent(Text::literal(pick(INSTANT, ru))),
        Fmt::PercentOff => percent(Text::translate("options.off", &[])),
        Fmt::Seconds => Text::literal(pick(SECONDS, ru).replace("%s", &fixed(v, 1, ru))),
        Fmt::SecondsInstant if v == 0.0 => Text::literal(pick(INSTANT, ru)),
        Fmt::SecondsInstant => Text::literal(pick(SECONDS, ru).replace("%s", &fixed(v, 2, ru))),
    }
}

// ---- drawing helpers -------------------------------------------------------------------

fn color(argb: u32) -> u32 {
    let [a, r, g, b] = argb.to_be_bytes();
    rgba(r, g, b, a)
}

const WHITE: u32 = 0xFFFF_FFFF;
const INACTIVE: u32 = 0xFFA0_A0A0;

/// `GuiGraphicsExtractor.fill(x0, y0, x1, y1, color)`.
fn fill(batch: &mut HudBatch, atlas: &HudAtlas, (x0, y0, x1, y1): (i32, i32, i32, i32), argb: u32) {
    let (x0, x1) = (x0.min(x1), x0.max(x1));
    let (y0, y1) = (y0.min(y1), y0.max(y1));
    batch.fill(atlas, x0 as f32, y0 as f32, (x1 - x0) as f32, (y1 - y0) as f32, color(argb));
}

/// `GuiGraphicsExtractor.outline`.
fn outline(batch: &mut HudBatch, atlas: &HudAtlas, (x, y, w, h): (i32, i32, i32, i32), argb: u32) {
    fill(batch, atlas, (x, y, x + w, y + 1), argb);
    fill(batch, atlas, (x, y + h - 1, x + w, y + h), argb);
    fill(batch, atlas, (x, y + 1, x + 1, y + h - 1), argb);
    fill(batch, atlas, (x + w - 1, y + 1, x + w, y + h - 1), argb);
}

fn text(batch: &mut HudBatch, atlas: &HudAtlas, t: &Text, x: i32, y: i32, argb: u32) {
    batch.text_styled(atlas, t, x as f32, y as f32, color(argb), true);
}

fn width(atlas: &HudAtlas, t: &Text) -> i32 {
    atlas.font.width_text(&t.resolved()) as i32
}

fn split(atlas: &HudAtlas, t: &Text, max: i32) -> Vec<Text> {
    atlas.font.split(&t.resolved(), max as f32)
}

/// `AbstractWidget.multiplyColor`: the RGB scaled, the alpha kept.
fn multiply(argb: u32, k: f32) -> u32 {
    let [a, r, g, b] = argb.to_be_bytes();
    let m = |c: u8| (f32::from(c) * k) as u8;
    u32::from_be_bytes([a, m(r), m(g), m(b)])
}

/// YACL's `Dimension.isPointInside`: both edges count.
fn within(r: [i32; 4], (x, y): (i32, i32)) -> bool {
    x >= r[0] && x <= r[0] + r[2] && y >= r[1] && y <= r[1] + r[3]
}

/// Vanilla's widget hover: the right and bottom edges are outside.
fn over(r: [i32; 4], (x, y): (i32, i32)) -> bool {
    x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3]
}

fn floor_point(p: (f32, f32)) -> (i32, i32) {
    (p.0.floor() as i32, p.1.floor() as i32)
}

/// YACL's reset glyph (`textures/font/reset.png`, "⟲" in its font): 8 by 8,
/// drawn with the font's shadow.
const RESET_GLYPH: [u8; 8] = [0b1011_1100, 0b1100_0010, 0b1110_0001, 0b0000_0001, 0b0000_0001, 0b1000_0001, 0b0100_0010, 0b0011_1100];

fn reset_glyph(batch: &mut HudBatch, atlas: &HudAtlas, x: i32, y: i32, argb: u32) {
    for (dx, dy, c) in [(1, 1, multiply(argb, 0.25)), (0, 0, argb)] {
        for (row, bits) in RESET_GLYPH.iter().enumerate() {
            for col in 0..8 {
                if bits & (0x80 >> col) != 0 {
                    let (px, py) = (x + col + dx, y + row as i32 + dy);
                    fill(batch, atlas, (px, py, px + 1, py + 1), c);
                }
            }
        }
    }
}

/// `GuiUtils.shortenString`.
fn shorten(atlas: &HudAtlas, name: &Text, max: i32) -> Text {
    let style = name.spans.first().map(|s| s.style.clone()).unwrap_or_default();
    let mut s = name.plain();
    if s.is_empty() {
        return name.clone();
    }
    let suffix = "...";
    let mut first = true;
    while width(atlas, &Text::styled(s.clone(), style.clone())) > max {
        let len = s.chars().count();
        let keep = len.saturating_sub(1 + if first { 1 } else { suffix.len() + 1 });
        s = s.chars().take(keep).collect::<String>().trim().to_string();
        s.push_str(suffix);
        if s == suffix {
            break;
        }
        first = false;
    }
    Text::styled(s, style)
}

/// A vanilla button, label and all (`Button`), with `label` centred.
#[allow(clippy::too_many_arguments)]
fn button(batch: &mut HudBatch, atlas: &HudAtlas, r: [i32; 4], label: &Text, active: bool, hovered: bool) {
    ui::button(batch, atlas, r[0] as f32, r[1] as f32, r[2] as f32, r[3] as f32, label, active, hovered);
}

// ---- layout ---------------------------------------------------------------------------

/// The list's top (`tabArea.top()`), under the 24-pixel tab bar.
const LIST_TOP: i32 = 23;
const TAB_BAR: i32 = 24;
const NAVBAR_MARGIN: i32 = 28;

/// Where everything is for a `w` by `h` screen (`CategoryTab`'s constructor
/// and `doLayout`).
#[derive(Clone, Copy, Debug)]
struct Layout {
    w: i32,
    h: i32,
    list_w: i32,
    /// The entries' row (`getRowLeft`, `getRowWidth`) once laid out.
    row_left: i32,
    row_width: i32,
    /// The widgets', made while the list was still one pixel wider.
    widget_left: i32,
    widget_width: i32,
    bar_x: i32,
    pane: [i32; 4],
    save: [i32; 4],
    cancel: [i32; 4],
    undo: [i32; 4],
    search: [i32; 4],
    desc: [i32; 4],
}

impl Layout {
    fn new(w: i32, h: i32) -> Layout {
        let list_w = w / 3 * 2;
        let first_w = w / 3 * 2 + 1;
        let widget_width = first_w - 6 - 20;
        let widget_left = first_w / 2 - widget_width / 2 - 6;
        let row_width = list_w - 6 - 20;
        let row_left = list_w / 2 - row_width / 2 - 6;
        let column = w / 3;
        let padding = column / 20;
        let column = column.min(400);
        let padded = column - padding * 2;
        let (cx, cy) = (w / 3 * 2 + w / 6, h - padding - 20);
        let save = [cx - padded / 2, cy, padded, 20];
        let half = padded - padded / 2 - 2;
        let (cx2, cy2) = (cx - half / 2 - 2, cy - 22);
        let cancel = [cx2 - half / 2, cy2, half, 20];
        let cx3 = cx2 + half + 4;
        let undo = [cx3 - half / 2, cy2, half, 20];
        let search = [w / 3 * 2 + w / 6 - padded / 2 + 1, cy2 - 22, padded - 2, 18];
        let desc = [w / 3 * 2 + padding, LIST_TOP + padding, padded, search[1] - 1 - LIST_TOP - padding * 2];
        Layout {
            w,
            h,
            list_w,
            row_left,
            row_width,
            widget_left,
            widget_width,
            bar_x: row_left + row_width + 6 + 2,
            pane: [w / 3 * 2, LIST_TOP + 1, w / 3, h - LIST_TOP],
            save,
            cancel,
            undo,
            search,
            desc,
        }
    }
}

// ---- the screen's state -----------------------------------------------------------------

/// What the screen asks `frame` for (it has `&mut Game`).
pub(super) enum Action {
    Click,
    Toast(String, String),
}

/// The screen's focused child (`Screen.getFocused`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Focus {
    None,
    List,
    Save,
    Cancel,
    Undo,
    Search,
    Tabs,
}

/// What a description shows: a group's name or an option's.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Target {
    Group(usize, usize),
    Opt(Opt),
}

/// One `CategoryTab`: its list, its description and its own buttons' labels.
struct Tab {
    scroll: Scroll,
    expanded: Vec<bool>,
    /// The focused widget in the list (`ControllerWidget.focused`).
    focused: Option<Opt>,
    desc: Option<Target>,
    desc_target: f32,
    desc_current: f32,
    desc_max: i32,
    desc_touched: Instant,
    desc_backward: bool,
    /// `updateButtons` last saw pending changes.
    shown_pending: bool,
    /// The Save button's message and tooltip set by the tick, kept until the
    /// next `updateButtons`.
    save_label: Option<(Text, Tr)>,
}

impl Tab {
    fn new(cat: usize, pending: bool) -> Tab {
        Tab {
            scroll: Scroll::default(),
            expanded: vec![true; CATEGORIES[cat].groups.len()],
            focused: None,
            desc: None,
            desc_target: 0.0,
            desc_current: 0.0,
            desc_max: 0,
            desc_touched: Instant::now(),
            desc_backward: false,
            shown_pending: pending,
            save_label: None,
        }
    }
}

/// `SettingsGuiFactory`'s demos and the copy of the settings they read.
struct Demos {
    settings: Settings,
    initial: Demo,
    scroll: Demo,
    secondary: Demo,
    presets: Vec<(Settings, Demo)>,
    shared: Shared,
}

impl Demos {
    fn new(saved: &Settings) -> Demos {
        let s = *saved;
        let mut initial = Demo::new(Control::InitialOnly, false, &Cfg::regular(&s));
        initial.keep_hand_fov = !s.affect_hand_fov;
        let mut scroll = Demo::new(Control::ScrollOnly, false, &Cfg::regular(&s));
        scroll.keep_hand_fov = !s.affect_hand_fov;
        let mut secondary = Demo::new(Control::InitialOnly, true, &Cfg::secondary(&s));
        secondary.render_hud = !s.secondary_hide_hud;
        let mut demos = Demos { settings: s, initial, scroll, secondary, presets: Vec::new(), shared: Shared::default() };
        // The options' first events pause the three demos made before them.
        demos.pause();
        // The presets' demos come after, each with its own settings.
        for i in 0..PRESET_NAMES.len() {
            let p = Settings::preset(i);
            demos.presets.push((p, Demo::new(Control::InitialOnly, false, &Cfg::regular(&p))));
        }
        demos
    }

    fn pause(&mut self) {
        self.initial.pause(&mut self.shared);
        self.scroll.pause(&mut self.shared);
        self.secondary.pause(&mut self.shared);
    }

    fn get(&mut self, id: DemoId) -> (&mut Demo, Cfg, &mut Shared) {
        let regular = Cfg::regular(&self.settings);
        match id {
            DemoId::Initial => (&mut self.initial, regular, &mut self.shared),
            DemoId::Scroll => (&mut self.scroll, regular, &mut self.shared),
            DemoId::Secondary => (&mut self.secondary, Cfg::secondary(&self.settings), &mut self.shared),
            DemoId::Preset(i) => {
                let (s, d) = &mut self.presets[i.min(PRESET_NAMES.len() - 1)];
                (d, Cfg::regular(s), &mut self.shared)
            }
        }
    }
}

/// One row of a category's list.
#[derive(Clone, Copy)]
enum Row {
    Sep(usize),
    Opt(usize, &'static Def),
}

pub(super) struct Yacl {
    ru: bool,
    in_world: bool,
    saved: Settings,
    pending: [f64; VALUES],
    /// The scrolling group's options are available ("Enable Scroll Zoom").
    available: bool,
    tab: usize,
    tabs: Vec<Tab>,
    search: EditBox,
    search_held: bool,
    preferred: Option<usize>,
    focus: Focus,
    nav_scroll: i32,
    /// `saveButtonMessage`'s ticks, while it shows.
    save_message: Option<u32>,
    demos: Demos,
    /// The slider whose thumb is held (`mouseDown`).
    held: Option<Opt>,
    last: Instant,
    ticks: f64,
    frame_ticks: f64,
    size: (i32, i32),
    pub(super) actions: Vec<Action>,
    /// Done, Cancel or Escape closed it.
    pub(super) closed: bool,
    /// The demo pair its description wants in the atlas, and the one there.
    pub(super) wanted: Option<Pair>,
    pub(super) uploaded: Option<Pair>,
    /// A preset built the screen again during this press.
    rebuilt: bool,
}

/// Keeps `s` as the port stores its settings.
fn store(game: &Game<'_>, s: Settings) {
    zf::set_settings(s);
    for (key, value) in s.stored() {
        game.native_set_setting(zf::ID, key, &value);
    }
}

impl Yacl {
    pub(super) fn new(saved: Settings, ru: bool, in_world: bool) -> Yacl {
        let pending = VALUE_OPTS.map(|o| get(&saved, o));
        Yacl {
            ru,
            in_world,
            saved,
            pending,
            available: saved.scroll_zoom,
            tab: 0,
            tabs: (0..CATEGORIES.len()).map(|c| Tab::new(c, false)).collect(),
            search: Self::new_search(),
            search_held: false,
            preferred: None,
            focus: Focus::None,
            nav_scroll: 0,
            save_message: None,
            demos: Demos::new(&saved),
            held: None,
            last: Instant::now(),
            ticks: 0.0,
            frame_ticks: 0.0,
            size: (0, 0),
            actions: Vec::new(),
            closed: false,
            wanted: None,
            uploaded: None,
            rebuilt: false,
        }
    }

    fn new_search() -> EditBox {
        let mut search = EditBox::new(0.0, 0.0, 0.0, 0.0);
        search.focused = false;
        search
    }

    // ---- values ----

    fn value(&self, o: Opt) -> f64 {
        self.pending.get(o as usize).copied().unwrap_or(0.0)
    }

    fn saved_value(&self, o: Opt) -> f64 {
        get(&self.saved, o)
    }

    fn changed(&self, o: Opt) -> bool {
        (o as usize) < VALUES && self.value(o) != self.saved_value(o)
    }

    fn pending_changes(&self) -> bool {
        VALUE_OPTS.iter().any(|&o| self.changed(o))
    }

    fn is_available(&self, o: Opt) -> bool {
        self.available || !SCROLL_INNER.contains(&o)
    }

    /// `requestSet`: a new pending value, and the option's listeners when it
    /// changed.
    fn set(&mut self, o: Opt, v: f64) {
        let i = o as usize;
        if i >= VALUES || self.pending[i] == v {
            return;
        }
        self.pending[i] = v;
        self.on_change(o);
    }

    fn on_change(&mut self, o: Opt) {
        let v = self.value(o);
        // `updateDemo`: the demos' settings follow, and they start over.
        match def(o).demo {
            Some(DemoId::Initial | DemoId::Scroll | DemoId::Secondary) => {
                match o {
                    Opt::AffectHandFov => {
                        self.demos.initial.keep_hand_fov = v == 0.0;
                        self.demos.scroll.keep_hand_fov = v == 0.0;
                    }
                    Opt::SecondaryHideHud => self.demos.secondary.render_hud = v == 0.0,
                    _ => put(&mut self.demos.settings, o, v),
                }
                self.demos.pause();
            }
            _ => {}
        }
        if o == Opt::ScrollZoom {
            self.set_available(v != 0.0);
        }
        // `YACLScreen.onOptionChanged`.
        self.update_buttons();
    }

    /// `setAvailable` on the scrolling group's options: one made unavailable
    /// forgets its pending value.
    fn set_available(&mut self, available: bool) {
        if self.available == available {
            return;
        }
        self.available = available;
        if !available {
            for o in SCROLL_INNER {
                let saved = self.saved_value(o);
                self.set(o, saved);
            }
        }
    }

    /// `CategoryTab.updateButtons`, on the open tab only.
    fn update_buttons(&mut self) {
        let pending = self.pending_changes();
        let tab = &mut self.tabs[self.tab];
        tab.shown_pending = pending;
        tab.save_label = None;
    }

    // ---- the buttons' jobs ----

    /// `finishOrSave`: Save keeps the screen open; Done closes it.
    fn finish_or_save(&mut self, game: &Game<'_>) {
        self.save_message = None;
        if self.pending_changes() {
            let mut s = self.saved;
            for o in VALUE_OPTS {
                put(&mut s, o, self.value(o));
            }
            store(game, s);
            self.saved = s;
            for o in VALUE_OPTS {
                self.pending[o as usize] = get(&s, o);
            }
            self.update_buttons();
        } else {
            self.closed = true;
        }
    }

    /// `cancelOrReset`: Cancel forgets and closes; Reset sets the defaults,
    /// pending.
    fn cancel_or_reset(&mut self) {
        if self.pending_changes() {
            self.undo();
            self.closed = true;
        } else {
            let d = Settings::default();
            for o in VALUE_OPTS {
                self.set(o, get(&d, o));
            }
        }
    }

    fn undo(&mut self) {
        for o in VALUE_OPTS {
            let v = self.saved_value(o);
            self.set(o, v);
        }
    }

    /// A preset's button: it is applied and kept at once, the pending
    /// changes are forgotten and the screen is built again (`screen.init`).
    fn apply_preset(&mut self, game: &Game<'_>, i: usize) {
        let s = Settings::preset(i);
        store(game, s);
        self.saved = s;
        let name = pick(PRESET_NAMES[i.min(PRESET_NAMES.len() - 1)], self.ru);
        self.actions.push(Action::Toast(
            pick(PRESET_TOAST, self.ru).to_string(),
            pick(PRESET_TOAST_DESC, self.ru).replace("%s", name),
        ));
        self.undo();
        let pending = self.pending_changes();
        self.tabs = (0..CATEGORIES.len()).map(|c| Tab::new(c, pending)).collect();
        self.search = Self::new_search();
        self.preferred = None;
        self.focus = Focus::None;
        self.nav_scroll = 0;
        self.held = None;
        self.rebuilt = true;
    }

    /// Escape (`shouldCloseOnEsc`): with pending changes it only says so.
    fn escape(&mut self) {
        if self.pending_changes() {
            self.save_message = Some(0);
        } else {
            self.closed = true;
        }
    }

    // ---- search ----

    fn query(&self) -> &str {
        &self.search.value
    }

    /// `OptionEntry.updateSearchQuery`.
    fn matches(&self, g: &GroupDef, d: &Def) -> bool {
        let q = self.query();
        if q.is_empty() {
            return true;
        }
        let group = g.name.map_or(String::new(), |n| pick(n, self.ru).to_lowercase());
        if group.contains(q) {
            return true;
        }
        let lower = q.to_lowercase();
        match d.kind {
            Kind::Label(t) => pick(t, self.ru).to_lowercase().contains(&lower),
            Kind::Button(_) => {
                pick(d.name, self.ru).to_lowercase().contains(&lower)
                    || pick(APPLY_PRESET, self.ru).to_lowercase().contains(q)
            }
            _ => pick(d.name, self.ru).to_lowercase().contains(&lower),
        }
    }

    /// `CategoryTab.hasSearch`.
    fn has_search(&self, cat: usize) -> bool {
        self.query().is_empty()
            || CATEGORIES[cat].groups.iter().any(|g| g.opts.iter().any(|d| self.matches(g, d)))
    }

    /// `YACLScreen.updateGlobalSearch`, from a change typed in the open tab.
    fn search_changed(&mut self) {
        if self.preferred.is_none() {
            self.preferred = Some(self.tab);
        }
        for tab in &mut self.tabs {
            tab.expanded.iter_mut().for_each(|e| *e = true);
        }
        let mut next = self.preferred.filter(|&p| self.has_search(p));
        for i in 0..CATEGORIES.len() {
            if Some(i) != self.preferred && next.is_none() && self.has_search(i) {
                next = Some(i);
            }
        }
        if let Some(n) = next
            && n != self.tab
            && (Some(n) == self.preferred || !self.has_search(self.tab))
        {
            self.tab = n;
            self.set_focus(Focus::Search);
        }
    }

    /// `Screen.setFocused`.
    fn set_focus(&mut self, f: Focus) {
        if f != Focus::Search {
            self.preferred = None;
        }
        if f != Focus::List {
            self.tabs[self.tab].focused = None;
        }
        let was = self.search.focused;
        self.search.focused = f == Focus::Search;
        if self.search.focused && !was {
            self.search.reset_blink();
        }
        self.focus = f;
    }

    fn edit_search(&mut self, f: impl FnOnce(&mut EditBox)) {
        let before = self.search.value.clone();
        f(&mut self.search);
        self.search.reset_blink();
        // `SearchFieldWidget.update`: nothing when it was and is empty.
        if self.search.value != before && !(before.is_empty() && self.search.value.is_empty()) {
            self.search_changed();
        }
    }

    // ---- the list's rows ----

    fn rows(&self, cat: usize) -> Vec<Row> {
        let mut rows = Vec::new();
        for (g, group) in CATEGORIES[cat].groups.iter().enumerate() {
            if group.name.is_some() {
                rows.push(Row::Sep(g));
            }
            for d in group.opts {
                rows.push(Row::Opt(g, d));
            }
        }
        rows
    }

    /// A row's height (0 while hidden), and its widget's for an option.
    fn row_height(&self, atlas: &HudAtlas, l: &Layout, cat: usize, row: Row) -> i32 {
        let groups = CATEGORIES[cat].groups;
        match row {
            Row::Sep(g) => {
                let group = &groups[g];
                if !group.opts.iter().any(|d| self.matches(group, d)) {
                    return 0;
                }
                let name = Text::literal(pick(group.name.unwrap_or(same("")), self.ru));
                split(atlas, &name, l.widget_width - 45).len().max(1) as i32 * 9 + 12
            }
            Row::Opt(g, d) => {
                let group = &groups[g];
                let expanded = group.name.is_none() || self.tabs[cat].expanded[g];
                if !expanded || !self.matches(group, d) {
                    return 0;
                }
                self.widget_height(atlas, l, d) + 2
            }
        }
    }

    fn widget_height(&self, atlas: &HudAtlas, l: &Layout, d: &Def) -> i32 {
        match d.kind {
            Kind::Label(t) => split(atlas, &Text::literal(pick(t, self.ru)), l.widget_width - 8).len() as i32 * 9 + 6,
            _ => 20,
        }
    }

    /// The option has a reset button (`canResetToDefault && widget.canReset`).
    fn resettable(d: &Def) -> bool {
        !matches!(d.kind, Kind::Label(_) | Kind::Button(_))
    }

    /// Every row with its y (`repositionEntries`) and height, and the list's
    /// content height.
    fn placed(&self, atlas: &HudAtlas, l: &Layout) -> (Vec<(Row, i32, i32)>, i32) {
        let scroll = self.tabs[self.tab].scroll.amount as i32;
        let mut y = LIST_TOP + 2 - scroll;
        let mut out = Vec::new();
        let mut content = 4;
        for row in self.rows(self.tab) {
            let h = self.row_height(atlas, l, self.tab, row);
            out.push((row, y, h));
            y += h;
            content += h;
        }
        (out, content)
    }

    fn widget_rect(l: &Layout, d: &Def, y: i32, h: i32) -> [i32; 4] {
        let w = if Self::resettable(d) { l.widget_width - 20 } else { l.widget_width };
        [l.widget_left, y, w, h]
    }

    /// The slider's track (`sliderBounds`).
    fn slider_bounds(r: [i32; 4]) -> [i32; 4] {
        let track = r[2] / 3;
        [r[0] + r[2] - 5 - 2 - track, r[1] + r[3] / 2 - 5, track, 10]
    }

    // ---- ticks ----

    fn advance(&mut self) {
        let now = Instant::now();
        let dt = now.duration_since(self.last).as_secs_f64().min(1.0);
        self.last = now;
        self.frame_ticks = (dt * 20.0).min(10.0);
        self.ticks += dt * 20.0;
        let mut n = 0;
        while self.ticks >= 1.0 && n < 20 {
            self.ticks -= 1.0;
            self.tick();
            n += 1;
        }
        self.ticks = self.ticks.min(1.0);
    }

    /// `YACLScreen.tick`: the open tab's description, then Save's message.
    fn tick(&mut self) {
        let tab = &mut self.tabs[self.tab];
        if let Some(Target::Opt(o)) = tab.desc
            && let Some(id) = def(o).demo
        {
            let (d, c, shared) = self.demos.get(id);
            d.tick(&c, shared);
        }
        let px = 1.0 / 20.0 * 9.0;
        if tab.desc_max > 0 && tab.desc_touched.elapsed() > Duration::from_millis(1500) {
            let mut step = px;
            if tab.desc_backward {
                step = -px;
                if tab.desc_target + step < 0.0 {
                    tab.desc_backward = false;
                    tab.desc_touched = Instant::now();
                }
            } else if tab.desc_target + step > tab.desc_max as f32 {
                tab.desc_backward = true;
                tab.desc_touched = Instant::now();
            }
            tab.desc_target = (tab.desc_target + step).clamp(0.0, tab.desc_max as f32);
        }
        if let Some(t) = self.save_message {
            if t > 140 {
                self.save_message = None;
            } else {
                self.save_message = Some(t + 1);
                let red = Text::literal(pick(SAVE_BEFORE_EXIT, self.ru)).with_color(0xFF5555);
                tab.save_label = Some((red, SAVE_BEFORE_EXIT_TIP));
            }
        }
    }

    // ---- drawing ----

    /// The whole screen over the Options screen, then its tooltip.
    pub(super) fn draw(&mut self, game: &Game<'_>, batch: &mut HudBatch) {
        self.advance();
        let atlas = &game.gui.atlas;
        let (w, h) = (batch.width.ceil() as i32, batch.height.ceil() as i32);
        self.size = (w, h);
        let l = Layout::new(w, h);
        let mouse = floor_point(game.cursor);
        // `Screen.extractBackground`: the blur over the world or the
        // panorama, the menu background, then the tab's right pane.
        game.native_apply_screen_blur();
        ui::menu_background(batch, atlas, 0.0, 0.0, w as f32, h as f32, self.in_world);
        self.right_pane(game, batch, &l);
        self.list(game, batch, &l, mouse);
        let mut tip = None;
        self.buttons(game, batch, &l, mouse, &mut tip);
        self.search.x = l.search[0] as f32;
        self.search.y = l.search[1] as f32;
        self.search.width = l.search[2] as f32;
        self.search.height = l.search[3] as f32;
        // No hint shows: YACL draws "Search..." in a colour with no alpha.
        self.search.draw(batch, atlas, None);
        self.description(game, batch, &l, mouse);
        self.tab_bar(game, batch, &l, mouse);
        if let Some(t) = tip {
            let lines = split(atlas, &Text::literal(pick(t, self.ru)), 170);
            ui::tooltip(batch, atlas, &lines, game.cursor);
        }
    }

    /// `CategoryTab.renderBackground`: the darker right pane and its top and
    /// left separators.
    fn right_pane(&self, game: &Game<'_>, batch: &mut HudBatch, l: &Layout) {
        let [left, top, pw, ph] = l.pane;
        // `menu_list_background` from texel (right + 2, bottom + 2), tiled.
        let sprite = "gui/menu_list_background";
        let size = game.native_sprite_size(sprite).unwrap_or([32.0, 32.0]);
        let k = [size[0] / 32.0, size[1] / 32.0];
        let (u0, v0) = ((left + pw + 2).rem_euclid(32), (top + ph + 2).rem_euclid(32));
        let (x_end, y_end) = (left + pw + 2, top + ph + 2);
        let mut y = top;
        let mut v = v0;
        while y < y_end {
            let th = (32 - v).min(y_end - y);
            let mut x = left;
            let mut u = u0;
            while x < x_end {
                let tw = (32 - u).min(x_end - x);
                let src = [u as f32 * k[0], v as f32 * k[1], tw as f32 * k[0], th as f32 * k[1]];
                game.native_sprite(batch, sprite, src, [x as f32, y as f32, tw as f32, th as f32]);
                x += tw;
                u = 0;
            }
            y += th;
            v = 0;
        }
        let atlas = &game.gui.atlas;
        ui::tiled_sized(
            batch,
            atlas,
            "gui/header_separator",
            (left - 1) as f32,
            (top - 2) as f32,
            (pw + 1) as f32,
            2.0,
            [32.0, 2.0],
            u32::MAX,
        );
        // The footer separator turned a quarter: its first row on the right.
        let sep = "gui/footer_separator";
        let sw = game.native_sprite_size(sep).unwrap_or([32.0, 2.0]);
        let row = sw[1] / 2.0;
        for (i, x) in [(0.0, left - 1), (1.0, left - 2)] {
            game.native_sprite(batch, sep, [0.0, i * row, 1.0, row], [x as f32, (top - 1) as f32, 1.0, (ph + 1) as f32]);
        }
    }

    /// `OptionListWidget`: the rows, clipped to the list, its separators and
    /// its scrollbar; the row under the pointer gives the description.
    fn list(&mut self, game: &Game<'_>, batch: &mut HudBatch, l: &Layout, mouse: (i32, i32)) {
        let atlas = &game.gui.atlas;
        let (_, content) = self.placed(atlas, l);
        let (top, bottom) = (LIST_TOP as f32, l.h as f32);
        self.tabs[self.tab].scroll.clamp(top, bottom, content as f32);
        let (rows, content) = self.placed(atlas, l);
        let start = batch.vertices.len();
        let mut hovered = None;
        for &(row, y, h) in &rows {
            if h == 0 || y + h < LIST_TOP || y > l.h {
                continue;
            }
            match row {
                Row::Sep(g) => self.separator(game, batch, l, (g, y, h), mouse),
                Row::Opt(_, d) => self.option(game, batch, l, (d, y, h), mouse),
            }
            if over([l.row_left, y, l.row_width, h], mouse) {
                hovered = Some(match row {
                    Row::Sep(g) => Target::Group(self.tab, g),
                    Row::Opt(_, d) => Target::Opt(d.opt),
                });
            }
        }
        batch.clip_since(start, 0.0, top, l.list_w as f32, bottom);
        // `extractListSeparators`: the header's above the list.
        let header = if self.in_world { "gui/inworld_header_separator" } else { "gui/header_separator" };
        ui::tiled_sized(batch, atlas, header, 0.0, top - 2.0, l.list_w as f32, 2.0, [32.0, 2.0], u32::MAX);
        let tab = &self.tabs[self.tab];
        ui::scrollbar_sprites(batch, atlas, l.bar_x as f32, top, bottom, content as f32, tab.scroll.amount);
        if let Some(t) = hovered {
            self.describe(t);
        }
    }

    /// `setHoverDescription`: a new description starts at its top.
    fn describe(&mut self, t: Target) {
        let tab = &mut self.tabs[self.tab];
        if tab.desc != Some(t) {
            tab.desc = Some(t);
            tab.desc_target = 0.0;
            tab.desc_current = 0.0;
            tab.desc_touched = Instant::now();
        }
    }

    fn in_list(l: &Layout, mouse: (i32, i32)) -> bool {
        mouse.1 >= LIST_TOP && mouse.1 < l.h && mouse.0 >= 0 && mouse.0 < l.list_w
    }

    /// `GroupSeparatorEntry`: the expand button and the centred name.
    fn separator(&self, game: &Game<'_>, batch: &mut HudBatch, l: &Layout, (g, y, h): (usize, i32, i32), mouse: (i32, i32)) {
        let atlas = &game.gui.atlas;
        let group = &CATEGORIES[self.tab].groups[g];
        let b = [l.row_left, y + h / 2 - 10 + 1, 20, 20];
        let hovered = over(b, mouse) && Self::in_list(l, mouse);
        // `LowProfileButtonWidget`: the button shows only while hovered.
        if hovered {
            ui::nine_slice(batch, atlas, "gui/sprites/widget/button_highlighted", b[0] as f32, b[1] as f32, 20.0, 20.0, u32::MAX);
        }
        horizon_client::widgets::widget_cursor(hovered, true);
        let arrow = Text::literal(if self.tabs[self.tab].expanded[g] { "▼" } else { "▶" });
        ui::scrolling_text_around_at(
            batch,
            atlas,
            &arrow,
            (b[0] + 10) as f32,
            (b[0] + 2) as f32,
            (b[0] + 18) as f32,
            b[1] as f32,
            (b[1] + 20) as f32,
            (b[1] + 6) as f32,
            horizon_client::hud_overlays::now_ms(),
            u32::MAX,
        );
        let name = Text::literal(pick(group.name.unwrap_or(same("")), self.ru));
        let cx = l.row_left + l.row_width / 2;
        for (i, line) in split(atlas, &name, l.widget_width - 45).iter().enumerate() {
            text(batch, atlas, line, cx - width(atlas, line) / 2, y + 6 + i as i32 * 9, WHITE);
        }
    }

    /// `OptionEntry`: the option's widget and its reset button.
    fn option(&self, game: &Game<'_>, batch: &mut HudBatch, l: &Layout, (d, y, h): (&Def, i32, i32), mouse: (i32, i32)) {
        let atlas = &game.gui.atlas;
        let r = Self::widget_rect(l, d, y, h - 2);
        match d.kind {
            Kind::Label(t) => {
                let lines = split(atlas, &Text::literal(pick(t, self.ru)), r[2] - 8);
                for (i, line) in lines.iter().enumerate() {
                    text(batch, atlas, line, r[0] + 4, r[1] + 3 + i as i32 * 9, WHITE);
                }
            }
            _ => self.controller(game, batch, d, r, mouse),
        }
        if Self::resettable(d) {
            let b = [r[0] + r[2], y, 20, 20];
            let active = self.value(d.opt) != get(&Settings::default(), d.opt) && self.is_available(d.opt);
            let hovered = over(b, mouse) && Self::in_list(l, mouse);
            button(batch, atlas, b, &Text::empty(), active, hovered);
            reset_glyph(batch, atlas, b[0] + 6, b[1] + 6, if active { WHITE } else { INACTIVE });
        }
    }

    /// `ControllerWidget.extractRenderState` with each controller's own part.
    fn controller(&self, game: &Game<'_>, batch: &mut HudBatch, d: &Def, r: [i32; 4], mouse: (i32, i32)) {
        let atlas = &game.gui.atlas;
        let available = self.is_available(d.opt);
        let focused = self.tabs[self.tab].focused == Some(d.opt);
        let slider = matches!(d.kind, Kind::Slider { .. });
        let hovered = within(r, mouse) || (slider && self.held == Some(d.opt));
        let shows_hover = available && (hovered || focused);
        let highlighted = (hovered && available) || focused;
        // `requestCursor`: a hand over what clicks, "not allowed" over what
        // is unavailable (the sliders' resize cursor has no Horizon twin).
        if matches!(d.kind, Kind::Tick | Kind::Cycle(_) | Kind::Button(_)) {
            horizon_client::widgets::widget_cursor(within(r, mouse), available);
        }
        let sprite = match (available, highlighted) {
            (true, false) => "gui/sprites/widget/button",
            (false, false) => "gui/sprites/widget/button_disabled",
            (true, true) => "gui/sprites/widget/button_highlighted",
            (false, true) => "gui/sprites/widget/slider_highlighted",
        };
        ui::nine_slice(batch, atlas, sprite, r[0] as f32, r[1] as f32, r[2] as f32, r[3] as f32, u32::MAX);
        let v = self.value(d.opt);
        let value = match d.kind {
            Kind::Cycle(names) => Text::literal(pick(names[(v.max(0.0) as usize).min(names.len() - 1)], self.ru)),
            Kind::Slider { fmt, .. } => format_value(fmt, v, self.ru),
            Kind::Button(_) => Text::literal(pick(APPLY_PRESET, self.ru)),
            _ => Text::empty(),
        };
        let value_w = width(atlas, &value);
        let track = Self::slider_bounds(r)[2];
        let control_w = match d.kind {
            Kind::Tick => 10,
            Kind::Slider { .. } if shows_hover => track + value_w + 6 + 2,
            _ => value_w,
        };
        let mut name = Text::literal(pick(d.name, self.ru));
        if self.changed(d.opt) {
            name = name.italic();
        }
        let name = shorten(atlas, &name, r[2] - control_w - 5 - 7);
        let c = if available { WHITE } else { INACTIVE };
        let ty = r[1] + 5;
        text(batch, atlas, &name, r[0] + 5, ty, c);
        match d.kind {
            Kind::Tick => {
                // `TickBoxControllerElement.extractValueText`.
                let (x1, y1) = (r[0] + r[2] - 5 - 10, r[1] + r[3] / 2 - 5);
                let (x2, y2) = (r[0] + r[2] - 5, r[1] + r[3] / 2 + 5);
                let shadow = multiply(c, 0.25);
                outline(batch, atlas, (x1 + 1, y1 + 1, x2 - x1, y2 - y1), shadow);
                outline(batch, atlas, (x1, y1, x2 - x1, y2 - y1), c);
                if v != 0.0 {
                    fill(batch, atlas, (x1 + 3, y1 + 3, x2 - 1, y2 - 1), shadow);
                    fill(batch, atlas, (x1 + 2, y1 + 2, x2 - 2, y2 - 2), c);
                }
            }
            Kind::Slider { min, max, .. } => {
                let shift = if shows_hover { track + 6 + 2 } else { 0 };
                text(batch, atlas, &value, r[0] + r[2] - value_w - 5 - shift, ty, c);
                if shows_hover {
                    // `extractHoveredControl`: the track and the thumb.
                    let b = Self::slider_bounds(r);
                    let cy = b[1] + b[3] / 2;
                    fill(batch, atlas, (b[0], cy - 1, b[0] + b[2], cy), WHITE);
                    fill(batch, atlas, (b[0] + 1, cy, b[0] + b[2] + 1, cy + 1), 0xFF40_4040);
                    let t = ((v - min) / (max - min)).clamp(0.0, 1.0) as f32;
                    let tx = (b[0] as f32 + b[2] as f32 * t) as i32;
                    fill(batch, atlas, (tx - 2 + 1, b[1] + 1, tx + 2 + 1, b[1] + b[3] + 1), 0xFF40_4040);
                    fill(batch, atlas, (tx - 2, b[1], tx + 2, b[1] + b[3]), WHITE);
                }
            }
            _ => text(batch, atlas, &value, r[0] + r[2] - value_w - 5, ty, c),
        }
    }

    /// The open tab's Done/Save, Reset/Cancel and Undo, with their labels as
    /// `updateButtons` last set them.
    fn buttons(&self, game: &Game<'_>, batch: &mut HudBatch, l: &Layout, mouse: (i32, i32), tip: &mut Option<Tr>) {
        let atlas = &game.gui.atlas;
        let tab = &self.tabs[self.tab];
        let pending = tab.shown_pending;
        let (save, save_tip) = match &tab.save_label {
            Some((label, t)) => (label.clone(), *t),
            None if pending => (Text::literal(pick(SAVE, self.ru)), SAVE_TIP),
            None => (Text::translate("gui.done", &[]), FINISHED_TIP),
        };
        let (cancel, cancel_tip) = if pending {
            (Text::translate("gui.cancel", &[]), CANCEL_TIP)
        } else {
            (Text::translate("controls.reset", &[]), RESET_TIP)
        };
        let undo = Text::literal(pick(UNDO, self.ru));
        for (r, label, active, t) in [
            (l.save, &save, true, save_tip),
            (l.cancel, &cancel, true, cancel_tip),
            (l.undo, &undo, pending, UNDO_TIP),
        ] {
            let hovered = over(r, mouse);
            button(batch, atlas, r, label, active, hovered);
            if hovered {
                *tip = Some(t);
            }
        }
    }

    /// `OptionDescriptionWidget`: the name, then the demo picture and the
    /// text, scrolled.
    fn description(&mut self, game: &Game<'_>, batch: &mut HudBatch, l: &Layout, mouse: (i32, i32)) {
        let atlas = &game.gui.atlas;
        let ru = self.ru;
        let a = self.frame_ticks as f32;
        let partial = self.frame_ticks;
        let cur = self.tab;
        let Some(target) = self.tabs[cur].desc else {
            return;
        };
        {
            let tab = &mut self.tabs[cur];
            tab.desc_current += (a * 0.5) * (tab.desc_target - tab.desc_current);
        }
        let [x, top, w, h] = l.desc;
        let (name, body, demo) = match target {
            Target::Group(c, g) => (pick(CATEGORIES[c].groups[g].name.unwrap_or(same("")), ru).to_string(), String::new(), None),
            Target::Opt(o) => {
                let d = def(o);
                let body = match d.kind {
                    Kind::Button(i) => pick(d.desc, ru).replace("%s", pick(PRESET_NAMES[i], ru)),
                    _ => pick(d.desc, ru).to_string(),
                };
                (pick(d.name, ru).to_string(), body, d.demo)
            }
        };
        let name = Text::literal(name).bold();
        if width(atlas, &name) > w {
            ui::scrolling_text(batch, atlas, &name, x as f32, (x + w) as f32, top as f32, (top + 9) as f32, (top + 1) as f32, u32::MAX);
        } else {
            text(batch, atlas, &name, x, top, WHITE);
        }
        let mut y = top + 5 + 9;
        let clip = [x as f32, y as f32, (x + w) as f32, (top + h) as f32];
        let start = batch.vertices.len();
        let current = self.tabs[cur].desc_current;
        y -= current as i32;
        if let Some(id) = demo {
            let pair = if id == DemoId::Secondary { Pair::ThirdPerson } else { Pair::FirstPerson };
            self.wanted = Some(pair);
            let ready = self.uploaded == Some(pair);
            let (d, c, _) = self.demos.get(id);
            y += d.draw(game, batch, (x, y, w), partial, &c, ready, clip) + 5;
        }
        for line in split(atlas, &Text::literal(body), w) {
            text(batch, atlas, &line, x, y, WHITE);
            y += 9;
        }
        batch.clip_since(start, clip[0], clip[1], clip[2], clip[3]);
        let tab = &mut self.tabs[cur];
        tab.desc_max = (y + current as i32 - top - h).max(0);
        if over(l.desc, mouse) {
            tab.desc_touched = Instant::now();
        }
    }

    /// The tabs' widths and the first one's x (`ScrollableNavigationBar.arrangeElements`).
    fn tab_layout(&self, atlas: &HudAtlas, w: i32) -> (i32, Vec<i32>, i32) {
        let no_scroll = w - NAVBAR_MARGIN * 2;
        let mut widths: Vec<i32> =
            CATEGORIES.iter().map(|c| width(atlas, &Text::literal(pick(c.name, self.ru))) + 20).collect();
        let mut all: i32 = widths.iter().sum();
        if all < no_scroll {
            let equal = no_scroll / widths.len() as i32;
            let big: i32 = widths.iter().filter(|&&b| b >= equal).sum();
            let small = widths.iter().filter(|&&b| b < equal).count() as i32;
            let each = if small > 0 { (no_scroll - big) / small } else { 0 };
            for b in &mut widths {
                if *b < equal {
                    *b = each;
                }
            }
            all = no_scroll;
        }
        let x0 = ((w - all) / 2).max(NAVBAR_MARGIN);
        (x0, widths, (all - no_scroll).max(0))
    }

    fn tab_title(&self, cat: usize) -> Text {
        let name = pick(CATEGORIES[cat].name, self.ru);
        if self.has_search(cat) {
            Text::literal(name)
        } else {
            Text::styled(name, Style { color: Some(0x555555), strikethrough: Some(true), ..Style::default() })
        }
    }

    /// `ScrollableNavigationBar` over `MenuTabBar`: the separators beside the
    /// tabs, the tabs, and the fades when they don't all fit.
    fn tab_bar(&self, game: &Game<'_>, batch: &mut HudBatch, l: &Layout, mouse: (i32, i32)) {
        let atlas = &game.gui.atlas;
        let (x0, widths, max) = self.tab_layout(atlas, l.w);
        let first = x0 - self.nav_scroll;
        let right = first + widths.iter().sum::<i32>();
        for (x, w) in [(0, first), (right, l.w)] {
            ui::tiled_sized(batch, atlas, "gui/header_separator", x as f32, (TAB_BAR - 2) as f32, w as f32, 2.0, [32.0, 2.0], u32::MAX);
        }
        let mut x = first;
        for (i, &tw) in widths.iter().enumerate() {
            let r = [x, 0, tw, TAB_BAR];
            let selected = i == self.tab;
            let hovered = over(r, mouse);
            let sprite = match (selected, hovered) {
                (true, true) => "gui/sprites/widget/tab_selected_highlighted",
                (true, false) => "gui/sprites/widget/tab_selected",
                (false, true) => "gui/sprites/widget/tab_highlighted",
                (false, false) => "gui/sprites/widget/tab",
            };
            ui::nine_slice(batch, atlas, sprite, x as f32, 0.0, tw as f32, TAB_BAR as f32, u32::MAX);
            let title = self.tab_title(i);
            if selected {
                ui::menu_background(batch, atlas, (x + 2) as f32, 2.0, (tw - 4) as f32, (TAB_BAR - 2) as f32, false);
                let uw = width(atlas, &title).min(tw - 4);
                let ux = x + (tw - uw) / 2;
                fill(batch, atlas, (ux, TAB_BAR - 2, ux + uw, TAB_BAR - 1), WHITE);
            }
            ui::menu_tab_label(
                batch,
                atlas,
                &title,
                (x as f32, 0.0, tw as f32, TAB_BAR as f32),
                selected,
                true,
                horizon_client::hud_overlays::now_ms(),
            );
            x += tw;
        }
        // The fades and arrows of a bar that scrolls.
        let fade = |batch: &mut HudBatch, from: i32, left_dark: bool| {
            for i in 0..40 {
                let k = if left_dark { 1.0 - i as f32 / 40.0 } else { i as f32 / 40.0 };
                let alpha = (255.0 * k) as u32;
                fill(batch, atlas, (from + i, 0, from + i + 1, TAB_BAR), alpha << 24);
            }
        };
        let label_y = (TAB_BAR - 9) / 2;
        if self.nav_scroll < max - NAVBAR_MARGIN {
            fade(batch, l.w - 40, false);
            text(batch, atlas, &Text::literal("→"), l.w - 10, label_y, WHITE);
        }
        if self.nav_scroll > NAVBAR_MARGIN {
            fade(batch, 0, true);
            text(batch, atlas, &Text::literal("←"), 5, label_y, WHITE);
        }
    }

    // ---- input ----

    fn layout(&self) -> Layout {
        Layout::new(self.size.0.max(1), self.size.1.max(1))
    }

    /// The row under the pointer (`getEntryAtPosition`).
    fn row_at(&self, atlas: &HudAtlas, l: &Layout, p: (i32, i32)) -> Option<(Row, i32, i32)> {
        let (rows, _) = self.placed(atlas, l);
        rows.into_iter().find(|&(_, y, h)| h > 0 && over([l.row_left, y, l.row_width, h], p))
    }

    /// A mouse button pressed: the screen's children in YACL's order (the
    /// list, the buttons, the search field, the description, the tabs).
    pub(super) fn press(&mut self, game: &Game<'_>, p: (f32, f32), button: i32, modifiers: u32) {
        let atlas = &game.gui.atlas;
        let l = self.layout();
        let m = floor_point(p);
        // `OptionListWidget.mouseClicked`: every other row's widget lets go.
        let under = self.row_at(atlas, &l, m);
        let keep = match under {
            Some((Row::Opt(_, d), _, _)) => Some(d.opt),
            _ => None,
        };
        if self.tabs[self.tab].focused != keep {
            self.tabs[self.tab].focused = None;
        }
        self.rebuilt = false;
        if self.list_press(game, &l, p, button, modifiers, under) {
            // A rebuilt screen's focus stays cleared.
            if !self.rebuilt {
                self.set_focus(Focus::List);
            }
            return;
        }
        if button == 0 {
            let pending = self.tabs[self.tab].shown_pending;
            if over(l.save, m) {
                self.actions.push(Action::Click);
                self.set_focus(Focus::Save);
                self.finish_or_save(game);
                return;
            }
            if over(l.cancel, m) {
                self.actions.push(Action::Click);
                self.set_focus(Focus::Cancel);
                self.cancel_or_reset();
                return;
            }
            if over(l.undo, m) && pending {
                self.actions.push(Action::Click);
                self.set_focus(Focus::Undo);
                self.undo();
                return;
            }
            if over(l.search, m) {
                self.set_focus(Focus::Search);
                let shift = modifiers & 1 != 0;
                self.search.click(atlas, p.0, shift, false);
                self.search_held = true;
                return;
            }
        }
        // The tab bar (`isMouseOver`: the top 24 pixels).
        if p.1 <= TAB_BAR as f32 && button == 0 {
            let (x0, widths, _) = self.tab_layout(atlas, l.w);
            let mut x = x0 - self.nav_scroll;
            for (i, tw) in widths.into_iter().enumerate() {
                if over([x, 0, tw, TAB_BAR], m) {
                    if i != self.tab {
                        self.actions.push(Action::Click);
                        self.tab = i;
                    }
                    self.set_focus(Focus::Tabs);
                    return;
                }
                x += tw;
            }
        }
    }

    /// The list's part of a press: its scrollbar, then the row's widgets.
    fn list_press(
        &mut self,
        game: &Game<'_>,
        l: &Layout,
        p: (f32, f32),
        button: i32,
        modifiers: u32,
        under: Option<(Row, i32, i32)>,
    ) -> bool {
        let atlas = &game.gui.atlas;
        let (_, content) = self.placed(atlas, l);
        let (top, bottom) = (LIST_TOP as f32, l.h as f32);
        let scrolling = button == 0 && self.tabs[self.tab].scroll.press(p, l.bar_x as f32, top, bottom, content as f32);
        let m = floor_point(p);
        if !Self::in_list(l, m) {
            return scrolling;
        }
        let Some((row, y, h)) = under else {
            return scrolling;
        };
        match row {
            Row::Sep(g) => {
                let b = [l.row_left, y + h / 2 - 10 + 1, 20, 20];
                if button == 0 && over(b, m) {
                    self.actions.push(Action::Click);
                    let e = &mut self.tabs[self.tab].expanded[g];
                    *e = !*e;
                    return true;
                }
            }
            Row::Opt(_, d) => {
                let r = Self::widget_rect(l, d, y, h - 2);
                if self.widget_press(game, d, r, p, button, modifiers) {
                    if !self.rebuilt {
                        self.tabs[self.tab].focused = Some(d.opt);
                    }
                    return true;
                }
                if Self::resettable(d) {
                    let b = [r[0] + r[2], y, 20, 20];
                    let default = get(&Settings::default(), d.opt);
                    let active = self.value(d.opt) != default && self.is_available(d.opt);
                    if button == 0 && active && over(b, m) {
                        self.actions.push(Action::Click);
                        self.tabs[self.tab].focused = None;
                        self.set(d.opt, default);
                        return true;
                    }
                }
            }
        }
        scrolling
    }

    /// A controller's `mouseClicked`.
    fn widget_press(&mut self, game: &Game<'_>, d: &Def, r: [i32; 4], p: (f32, f32), button: i32, modifiers: u32) -> bool {
        let m = floor_point(p);
        let available = self.is_available(d.opt);
        let v = self.value(d.opt);
        match d.kind {
            Kind::Tick => {
                if !within(r, m) || !available {
                    return false;
                }
                self.set(d.opt, if v != 0.0 { 0.0 } else { 1.0 });
                self.actions.push(Action::Click);
                true
            }
            Kind::Cycle(names) => {
                if !within(r, m) || (button != 0 && button != 1) || !available {
                    return false;
                }
                self.actions.push(Action::Click);
                let back = button == 1 || modifiers & 3 != 0;
                self.cycle(d.opt, names.len(), if back { -1 } else { 1 });
                true
            }
            Kind::Slider { .. } => {
                if !available || button != 0 || !within(Self::slider_bounds(r), m) {
                    return false;
                }
                self.held = Some(d.opt);
                self.slide_to(d, r, p.0);
                true
            }
            Kind::Button(i) => {
                if !within(r, m) || !available {
                    return false;
                }
                self.actions.push(Action::Click);
                self.apply_preset(game, i);
                true
            }
            Kind::Label(_) => false,
        }
    }

    fn cycle(&mut self, o: Opt, len: usize, step: i32) {
        let len = len as i32;
        let mut i = self.value(o) as i32 + step;
        if i >= len {
            i -= len;
        } else if i < 0 {
            i += len;
        }
        self.set(o, f64::from(i));
    }

    /// `setValueFromMouse`, rounded to the step (`roundToInterval`).
    fn slide_to(&mut self, d: &Def, r: [i32; 4], x: f32) {
        let Kind::Slider { min, max, step, int, .. } = d.kind else {
            return;
        };
        let b = Self::slider_bounds(r);
        let value = (f64::from(x) - f64::from(b[0])) / f64::from(b[2]) * (max - min);
        let mut v = (min + step * (value / step).round()).clamp(min, max);
        if int {
            v = v.trunc();
        }
        self.set(d.opt, v);
    }

    /// `incrementValue`: the arrows and the wheel with Shift or Control.
    fn nudge(&mut self, d: &Def, amount: f64) {
        let Kind::Slider { min, max, step, int, .. } = d.kind else {
            return;
        };
        let mut v = (self.value(d.opt) + step * amount).clamp(min, max);
        if int {
            v = v.trunc();
        }
        self.set(d.opt, v);
    }

    /// The row and widget of a focused or held option in the open tab.
    fn widget_of(&self, atlas: &HudAtlas, l: &Layout, o: Opt) -> Option<(&'static Def, [i32; 4])> {
        let (rows, _) = self.placed(atlas, l);
        rows.into_iter().find_map(|(row, y, h)| match row {
            Row::Opt(_, d) if d.opt == o && h > 0 => Some((d, Self::widget_rect(l, d, y, h - 2))),
            _ => None,
        })
    }

    pub(super) fn release(&mut self, _button: i32) {
        self.tabs[self.tab].scroll.release();
        self.search_held = false;
        if let Some(o) = self.held.take()
            && self.is_available(o)
        {
            self.actions.push(Action::Click);
        }
    }

    pub(super) fn drag(&mut self, game: &Game<'_>, p: (f32, f32)) {
        let atlas = &game.gui.atlas;
        let l = self.layout();
        if let Some(o) = self.held {
            if self.is_available(o)
                && let Some((d, r)) = self.widget_of(atlas, &l, o)
            {
                self.slide_to(d, r, p.0);
            }
            return;
        }
        if self.tabs[self.tab].scroll.held() {
            let (_, content) = self.placed(atlas, &l);
            self.tabs[self.tab].scroll.drag(p.1, LIST_TOP as f32, l.h as f32, content as f32);
            return;
        }
        if self.search_held && self.focus == Focus::Search {
            self.search.drag_to(atlas, p.0);
        }
    }

    /// The wheel: the child under the pointer takes it (`getChildAt`).
    pub(super) fn scroll(&mut self, game: &Game<'_>, p: (f32, f32), (horizontal, vertical): (f64, f64)) {
        let atlas = &game.gui.atlas;
        let l = self.layout();
        let m = floor_point(p);
        if Self::in_list(&l, m) {
            let (_, content) = self.placed(atlas, &l);
            self.tabs[self.tab].scroll.wheel(vertical as f32, 10.0, LIST_TOP as f32, l.h as f32, content as f32);
            // Then each row's widget: a slider moves with Shift or Control.
            if game.has_shift_down() || game.has_control_down() {
                let (rows, _) = self.placed(atlas, &l);
                for (row, y, h) in rows {
                    if let Row::Opt(_, d) = row
                        && h > 0
                        && matches!(d.kind, Kind::Slider { .. })
                        && self.is_available(d.opt)
                        && within(Self::widget_rect(&l, d, y, h - 2), m)
                    {
                        self.nudge(d, vertical);
                        break;
                    }
                }
            }
            return;
        }
        if [l.save, l.cancel, l.undo, l.search].iter().any(|&r| over(r, m)) {
            return;
        }
        if over(l.desc, m) {
            let tab = &mut self.tabs[self.tab];
            if tab.desc.is_some() {
                tab.desc_target = (tab.desc_target - (vertical as i32 * 10) as f32).clamp(0.0, tab.desc_max as f32);
                tab.desc_touched = Instant::now();
            }
            return;
        }
        if p.1 <= TAB_BAR as f32 {
            let (_, _, max) = self.tab_layout(atlas, l.w);
            let offset = self.nav_scroll - (vertical * 15.0) as i32 - (horizontal * 15.0) as i32;
            self.nav_scroll = offset.clamp(0, max);
        }
    }

    /// A key: Escape, the search field's editing, a focused widget's keys.
    /// True when the clipboard is wanted (`paste`).
    pub(super) fn key(&mut self, game: &Game<'_>, key: &str, modifiers: u32) -> KeyResult {
        let shift = modifiers & 1 != 0;
        let control = modifiers & 2 != 0;
        if key == "key.keyboard.escape" {
            self.escape();
            return KeyResult::None;
        }
        if self.focus == Focus::Search {
            match key {
                "key.keyboard.v" if control => return KeyResult::Paste,
                "key.keyboard.c" if control => return KeyResult::Copy(self.search.selected_text()),
                "key.keyboard.x" if control => {
                    let cut = self.search.selected_text();
                    self.edit_search(|s| s.delete_chars(0));
                    return KeyResult::Copy(cut);
                }
                _ => {
                    if let Some(code) = horizon_client::keybinds::key_code(key) {
                        self.edit_search(|s| {
                            s.key(code, control, shift);
                        });
                    }
                }
            }
            return KeyResult::None;
        }
        if self.focus != Focus::List {
            return KeyResult::None;
        }
        let Some(o) = self.tabs[self.tab].focused else {
            return KeyResult::None;
        };
        let atlas = &game.gui.atlas;
        let l = self.layout();
        let Some((d, _)) = self.widget_of(atlas, &l, o) else {
            return KeyResult::None;
        };
        let enter = matches!(key, "key.keyboard.enter" | "key.keyboard.space" | "key.keyboard.keypad.enter");
        match d.kind {
            Kind::Tick if enter => {
                let v = self.value(o);
                self.set(o, if v != 0.0 { 0.0 } else { 1.0 });
                self.actions.push(Action::Click);
            }
            Kind::Cycle(names) => match key {
                "key.keyboard.left" => self.cycle(o, names.len(), -1),
                "key.keyboard.right" => self.cycle(o, names.len(), 1),
                _ if enter => self.cycle(o, names.len(), if control || shift { -1 } else { 1 }),
                _ => {}
            },
            Kind::Slider { .. } => match key {
                "key.keyboard.left" => self.nudge(d, -1.0),
                "key.keyboard.right" => self.nudge(d, 1.0),
                _ => {}
            },
            Kind::Button(i) if enter => {
                self.actions.push(Action::Click);
                self.apply_preset(game, i);
            }
            _ => {}
        }
        KeyResult::None
    }

    /// A character typed: into the search field when it has the focus.
    pub(super) fn char_typed(&mut self, c: char) {
        if self.focus == Focus::Search && !c.is_control() {
            self.edit_search(|s| s.insert_text(&c.to_string()));
        }
    }

    /// The clipboard's text, pasted into the search field.
    pub(super) fn paste(&mut self, text: &str) {
        if self.focus == Focus::Search {
            let text: String = text.chars().filter(|c| !c.is_control()).collect();
            self.edit_search(|s| s.insert_text(&text));
        }
    }
}

/// What a key asks of `frame`.
pub(super) enum KeyResult {
    None,
    Paste,
    Copy(String),
}
