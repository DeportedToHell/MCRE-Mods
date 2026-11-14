// Plasmo Voice's settings screen (`VoiceSettingsScreen`, `VoiceSettingsNavigation`, the tabs in
// `gui/settings/tab` and the widgets in `gui/settings/widget` and `su.plo.lib.mod.client.gui`),
// `VoiceNotAvailableScreen` and the position screens (`ActivationIconPositionScreen`,
// `OverlayPositionScreen`), laid out with Plasmo Voice's own numbers in the game's widget sprites
// and Plasmo Voice's icons. Opened by the menu key (V) as a screen of the mod's own over the
// world (`native_mod_screen_open`), drawn in `screen_render_post` and fed by the screen hooks.
//
// Each frame the open tab's rows are built again from the config and what the voice connection
// knows (`init` on every change, as the tabs do on their events), so the screen keeps only what
// the player is doing: the tab, the scroll, a slider held, a list open, a field focused, a hotkey
// being bound, the meter and the microphone test.
//
// This Source Code Form is a port of Plasmo Voice (https://github.com/plasmoapp/plasmo-voice), LGPL-3.0.

use super::Game;
use super::plasmovoice_config::{Config, V};
use super::plasmovoice_icons as icons;
use super::plasmovoice_lang::{t, tf};
use horizon_client::hud::{HudAtlas, HudBatch, rgba};
use horizon_client::text::Text;
use horizon_client::ui::{self, EditBox};
use plasmovoice_engine::tcp::{Activation, PlayerInfo, SourceLine};
use std::cell::RefCell;
use std::collections::HashMap;
use std::time::Instant;

type Rect = [f32; 4];

/// `TabWidget`'s numbers.
const ELEMENT_WIDTH: f32 = 124.0;
const CATEGORY_GAP: f32 = 8.0;
const ELEMENT_GAP: f32 = 4.0;
const PAGE_PADDING: f32 = 4.0;
const CONTENT_WIDTH: f32 = 303.0;
const SCROLL_STEP: f32 = 24.0;
/// `DropDownWidgetList`.
const DROP_ITEM: f32 = 17.0;
const DROP_VISIBLE: usize = 5;

/// `Colors`.
const WHITE: u32 = 0xFFFF_FFFF;
const GRAY: u32 = 0xFFA0_A0A0;
const TEXT: u32 = 0xFFE0_E0E0;
const TEXT_DISABLED: u32 = 0xFF70_7070;
const TEXT_MUTED: u32 = 0xFF80_8080;
const WARNING: u32 = 0xFFFA_C653;
const BLACK: u32 = 0xFF00_0000;

/// The port's version, shown in the settings title as the original shows its own.
pub(super) const VERSION: &str = "1.0.3";

// ---- what the screen shows of the voice connection ------------------------------------------

/// Filled by the glue each frame from the client.
#[derive(Clone, Default)]
pub(super) struct Info {
    /// `udpClientManager.isConnected && serverInfo.isPresent`.
    pub connected: bool,
    /// A UDP client exists (connecting) and whether it gave up.
    pub has_client: bool,
    pub closed: bool,
    pub server_id: String,
    /// `getActivations` (heaviest first, then by name) and `getLines`.
    pub activations: Vec<Activation>,
    pub lines: Vec<SourceLine>,
    pub players: Vec<PlayerInfo>,
    /// Senders of the direct sources heard (`DirectSourceInfo.sender`).
    pub direct_senders: Vec<(u128, String)>,
    pub me: u128,
    /// Player list skins for faces.
    pub skins: HashMap<u128, String>,
    pub input_device: Option<String>,
    pub input_error: Option<String>,
    pub speakers: Option<String>,
    pub level: f64,
    pub frames: u64,
    /// When each line was last heard (`ClientSourceLine.lastActivationTime`).
    pub line_used: HashMap<String, Instant>,
}

/// The audio devices, read when the screen opens (`DeviceFactory.getDeviceNames`).
#[derive(Clone, Default)]
struct Devices {
    inputs: Vec<String>,
    outputs: Vec<String>,
    default_input: String,
    default_output: String,
}

fn devices() -> Devices {
    use plasmovoice_engine::audio;
    Devices {
        inputs: audio::input_devices(),
        outputs: audio::output_devices(),
        default_input: audio::default_input_device().unwrap_or_default(),
        default_output: audio::default_output_device().unwrap_or_default(),
    }
}

// ---- the screen's state ---------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Page {
    Settings,
    NotAvailable,
    /// `ActivationIconPositionScreen` (true) or `OverlayPositionScreen`.
    Position(bool),
}

#[derive(Clone, Debug, PartialEq)]
enum Focus {
    Search,
    Number(String),
}

/// What needs `&mut Game`, done in `frame`.
pub(super) enum Request {
    Click,
    Close,
}

pub(super) struct Screen {
    pub page: Page,
    tab: usize,
    scroll: f32,
    /// The scrollbar held: the pointer's y where it was.
    scrollbar: Option<f32>,
    /// The slider held, by row id.
    dragging: Option<String>,
    /// The open dropdown, by row id, and its list's scroll.
    dropdown: Option<(String, f32)>,
    focus: Option<Focus>,
    search: EditBox,
    number: EditBox,
    /// The hotkey being bound and the keys pressed so far (`HotKeyWidget`).
    capture: Option<(String, Vec<String>)>,
    /// `MicrophoneTestController.microphoneValue` and the frames counted.
    meter: f64,
    frames: u64,
    last_frame: Instant,
    pub mic_test: bool,
    devices: Devices,
    pub requests: Vec<Request>,
    opened: Instant,
}

thread_local! {
    static SCREEN: RefCell<Option<Screen>> = const { RefCell::new(None) };
    static INFO: RefCell<Info> = RefCell::new(Info::default());
}

pub(super) fn set_info(info: Info) {
    INFO.with(|i| *i.borrow_mut() = info);
}

fn info<R>(f: impl FnOnce(&Info) -> R) -> R {
    INFO.with(|i| f(&i.borrow()))
}

pub(super) fn with<R>(f: impl FnOnce(&mut Screen) -> R) -> Option<R> {
    SCREEN.with(|s| s.try_borrow_mut().ok().and_then(|mut s| s.as_mut().map(f)))
}

pub(super) fn is_open() -> bool {
    SCREEN.with(|s| s.try_borrow().map(|s| s.is_some()).unwrap_or(false))
}

/// `VoiceScreens.openSettings`: the settings when connected, else the not-available screen.
pub(super) fn open() {
    let connected = info(|i| i.connected);
    let mut search = EditBox::new(0.0, 0.0, CONTENT_WIDTH, 20.0);
    search.focused = false;
    let mut number = EditBox::new(0.0, 0.0, ELEMENT_WIDTH, 20.0);
    number.focused = false;
    let screen = Screen {
        page: if connected { Page::Settings } else { Page::NotAvailable },
        tab: SCREEN.with(|s| s.borrow().as_ref().map(|s| s.tab)).unwrap_or(0),
        scroll: 0.0,
        scrollbar: None,
        dragging: None,
        dropdown: None,
        focus: None,
        search,
        number,
        capture: None,
        meter: 0.0,
        frames: info(|i| i.frames),
        last_frame: Instant::now(),
        mic_test: false,
        devices: if connected { devices() } else { Devices::default() },
        requests: Vec::new(),
        opened: Instant::now(),
    };
    SCREEN.with(|s| *s.borrow_mut() = Some(screen));
    LAST_TAB.with(|t| {
        if let Some(tab) = t.get() {
            with(|s| s.tab = tab);
        }
    });
    with(Screen::init_tab);
}

thread_local! {
    /// The settings screen is made once and kept (`VoiceScreens.settingsScreen`): its tab too.
    static LAST_TAB: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
}

/// The screen went away (Escape, the menu key, another screen): `removed`.
pub(super) fn closed() {
    if let Some(tab) = with(|s| s.tab) {
        LAST_TAB.with(|t| t.set(Some(tab)));
    }
    SCREEN.with(|s| *s.borrow_mut() = None);
}

// ---- drawing helpers ------------------------------------------------------------------------

fn c(argb: u32) -> u32 {
    let [a, r, g, b] = argb.to_be_bytes();
    rgba(r, g, b, a)
}

fn fill(batch: &mut HudBatch, atlas: &HudAtlas, x0: f32, y0: f32, x1: f32, y1: f32, argb: u32) {
    if x1 > x0 && y1 > y0 {
        batch.fill(atlas, x0, y0, x1 - x0, y1 - y0, c(argb));
    }
}

fn styled(s: &str) -> Text {
    if s.contains('§') { Text::legacy(s) } else { Text::literal(s) }
}

fn width(atlas: &HudAtlas, s: &str) -> f32 {
    atlas.font.width_text(&styled(s))
}

fn text(batch: &mut HudBatch, atlas: &HudAtlas, s: &str, x: f32, y: f32, argb: u32, shadow: bool) {
    batch.text_styled(atlas, &styled(s), x, y, c(argb), shadow);
}

fn centered(batch: &mut HudBatch, atlas: &HudAtlas, s: &str, cx: f32, y: f32, argb: u32) {
    let w = width(atlas, s);
    text(batch, atlas, s, (cx - (w / 2.0).floor()).floor(), y, argb, true);
}

/// `RenderUtil.getTruncatedText`: cut to `w` with "..." when it doesn't fit.
fn truncated(atlas: &HudAtlas, s: &str, w: f32) -> String {
    if width(atlas, s) <= w {
        return s.to_owned();
    }
    let room = w - width(atlas, "...");
    let mut out = String::new();
    for ch in s.chars() {
        out.push(ch);
        if width(atlas, &out) > room {
            out.pop();
            break;
        }
    }
    out + "..."
}

fn inside(p: (f32, f32), r: Rect) -> bool {
    p.0 >= r[0] && p.1 >= r[1] && p.0 < r[0] + r[2] && p.1 < r[1] + r[3]
}

fn sprite(batch: &mut HudBatch, atlas: &HudAtlas, name: &str, r: Rect) {
    ui::nine_slice(batch, atlas, &format!("gui/sprites/widget/{name}"), r[0], r[1], r[2], r[3], u32::MAX);
}

/// `GuiAbstractWidget.renderButton` of a plain button: its sprite and centred label.
fn button(batch: &mut HudBatch, atlas: &HudAtlas, r: Rect, label: &str, active: bool, hover: bool) {
    let name = if !active {
        "button_disabled"
    } else if hover {
        "button_highlighted"
    } else {
        "button"
    };
    sprite(batch, atlas, name, r);
    if !label.is_empty() {
        centered(batch, atlas, label, r[0] + (r[2] / 2.0).floor(), r[1] + ((r[3] - 8.0) / 2.0).floor(), if active { WHITE } else { GRAY });
    }
}

/// `IconButton`: a button with a 16 px icon and its shadow.
fn icon_button(batch: &mut HudBatch, atlas: &HudAtlas, r: Rect, b: &Btn, hover: bool) {
    button(batch, atlas, r, "", b.active, hover);
    let color = b.color.unwrap_or(if b.active { WHITE } else { GRAY });
    let shadow = b.shadow.then_some(if b.active { WHITE } else { GRAY });
    icons::draw_shadowed(batch, atlas, &b.icon, r[0] + 2.0, r[1] + 2.0, 16.0, color, shadow);
}

/// `AbstractSlider`: the track, the handle at `value` and the centred text.
fn slider(batch: &mut HudBatch, atlas: &HudAtlas, r: Rect, label: &str, value: f64, hover: bool, active: bool) {
    sprite(batch, atlas, "slider", r);
    let x0 = r[0] + (value * f64::from(r[2] - 8.0)) as i32 as f32;
    sprite(batch, atlas, if hover && active { "slider_handle_highlighted" } else { "slider_handle" }, [x0, r[1], 8.0, 20.0]);
    centered(batch, atlas, label, r[0] + (r[2] / 2.0).floor(), r[1] + 10.0 - 4.0, if active { WHITE } else { GRAY });
}

/// Java's `Math.round`.
fn round(v: f64) -> i64 {
    (v + 0.5).floor() as i64
}

/// `AudioUtil.audioLevelToDoubleRange` and back.
fn level_to_range(db: f64) -> f64 {
    1.0 - (db.max(-60.0) / -60.0)
}
fn range_to_level(v: f64) -> f64 {
    -60.0 * (1.0 - v)
}

// ---- rows -----------------------------------------------------------------------------------

/// What a click does to the config.
#[derive(Clone, Debug)]
enum Act {
    None,
    Invert(String),
    Set(String, V),
    Reset(Vec<String>),
    ResetHotkey(String),
    MicTest,
    Position(bool),
    /// The Volume tab's eye (`showAllSourceLines`).
    ShowAll,
}

/// What choosing a dropdown's item `i` does.
#[derive(Clone, Debug)]
enum Choose {
    /// A device by name: the default one is kept as "" (`Strings.nullToEmpty`).
    Device(String, Vec<String>, String),
    /// A field set to the item's value.
    Value(String, Vec<V>),
}

#[derive(Clone, Debug)]
struct Btn {
    icon: String,
    active: bool,
    act: Act,
    tip: Option<String>,
    /// `setIconColor`, else white (gray inactive).
    color: Option<u32>,
    shadow: bool,
}

impl Btn {
    fn new(icon: &str, act: Act) -> Btn {
        Btn { icon: icon.to_owned(), active: true, act, tip: None, color: None, shadow: true }
    }
}

#[derive(Clone, Debug)]
enum Label {
    Text(String),
    /// `iconLabel`: a 16 px icon, then the text.
    Icon(String, String),
    /// A player's 24 px face and name (`PlayerHeadWidget`).
    Head(u128, String),
}

#[derive(Clone, Debug)]
enum El {
    Toggle(String),
    /// A toggle over a value of another kind: on, and what clicking does.
    ToggleWith(bool, Act),
    Volume(String),
    IntSlider(String, i64, i64, String),
    Threshold,
    Drop { items: Vec<String>, selected: Option<usize>, text: String, overflow: bool, tips: Vec<Option<String>>, active: bool, choose: Choose },
    Button(String, Act),
    Hotkey(String),
    Distance(String, Vec<i32>),
    Number(String),
}

#[derive(Clone, Debug)]
struct Opt {
    id: String,
    label: Label,
    tip: Option<String>,
    el: El,
    el_w: f32,
    buttons: Vec<Btn>,
    /// The reset button: active when not at the default, and what it does.
    reset: (bool, Act),
    height: f32,
}

impl Opt {
    fn new(id: &str, label: &str, el: El, path_default: (bool, Act)) -> Opt {
        Opt { id: id.to_owned(), label: Label::Text(label.to_owned()), tip: None, el, el_w: ELEMENT_WIDTH, buttons: Vec::new(), reset: path_default, height: 20.0 }
    }
    fn tip(mut self, tip: Option<String>) -> Opt {
        self.tip = tip;
        self
    }
}

#[derive(Clone, Debug)]
enum Item {
    Opt(Opt),
    Search,
    /// The players' column (6 px apart).
    Players(Vec<Opt>),
}

struct Cat {
    title: String,
    trailing: Vec<Btn>,
    items: Vec<Item>,
}

fn cat(title: String, items: Vec<Item>) -> Cat {
    Cat { title, trailing: Vec::new(), items }
}

fn path_reset(cfg: &Config, path: &str) -> (bool, Act) {
    (!cfg.is_default(path), Act::Reset(vec![path.to_owned()]))
}

fn toggle(cfg: &Config, path: &str, label: &str, tip: Option<&str>) -> Item {
    Item::Opt(Opt::new(path, &t(label), El::Toggle(path.to_owned()), path_reset(cfg, path)).tip(tip.map(t)))
}

fn volume(cfg: &Config, path: &str, label: &str, tip: Option<&str>) -> Item {
    Item::Opt(Opt::new(path, &t(label), El::Volume(path.to_owned()), path_reset(cfg, path)).tip(tip.map(t)))
}

fn on_off(b: bool) -> String {
    t(if b { "message.plasmovoice.on" } else { "message.plasmovoice.off" })
}

/// "Currently: ENABLED" (green) or "DISABLED" (red), gray.
fn currently(enabled: bool) -> String {
    let state = if enabled { format!("§a{}", t("gui.plasmovoice.toggle.enabled")) } else { format!("§c{}", t("gui.plasmovoice.toggle.disabled")) };
    format!("§7{}", tf("gui.plasmovoice.toggle.currently", &[&format!("{state}§7")]))
}

/// A key list as `HotKeyWidget.formatKeys` writes it.
fn key_names(keys: &[String]) -> String {
    if keys.is_empty() {
        return horizon_client::lang::get("gui.none");
    }
    keys.iter().map(|k| horizon_client::keybinds::display_name(k).plain()).collect::<Vec<_>>().join(" + ")
}

/// `PlayerActivationDistances`' path for an activation on this server.
pub(super) fn distance_path(server: &str, a: &Activation) -> String {
    format!("servers.{server}.distances.{}", a.id())
}

/// `Activation.calculateAllowedDistance`.
pub(super) fn allowed_distance(a: &Activation, d: i64) -> i64 {
    let (min, max) = (a.distances.first().copied().unwrap_or(-1), a.distances.last().copied().unwrap_or(-1));
    if min == -1 {
        if d < 1 || d > i64::from(max) { i64::from(a.default_distance) } else { d }
    } else if a.distances.contains(&(d as i32)) {
        d
    } else {
        i64::from(a.default_distance)
    }
}

impl Screen {
    fn tabs() -> [(&'static str, &'static str); 6] {
        [
            ("gui.plasmovoice.devices", "tab_devices"),
            ("gui.plasmovoice.volume", "tab_volume"),
            ("gui.plasmovoice.activation", "tab_activation"),
            ("gui.plasmovoice.overlay", "tab_overlay"),
            ("gui.plasmovoice.advanced", "tab_advanced"),
            ("gui.plasmovoice.hotkeys", "tab_hotkeys"),
        ]
    }

    /// `TabWidget.init` of the tab opened.
    fn init_tab(&mut self) {
        self.scroll = 0.0;
        self.dragging = None;
        self.dropdown = None;
        self.capture = None;
        self.focus = None;
        self.search.focused = false;
        self.number.focused = false;
        // The Volume tab focuses its search field.
        if self.tab == 1 {
            self.focus = Some(Focus::Search);
            self.search.focused = true;
        }
    }

    fn open_tab(&mut self, tab: usize) {
        // `openTab`: the microphone test stops.
        self.mic_test = false;
        self.tab = tab;
        self.init_tab();
    }

    fn build(&self, cfg: &Config, inf: &Info) -> Vec<Cat> {
        match self.tab {
            0 => self.devices_tab(cfg, inf),
            1 => self.volume_tab(cfg, inf),
            2 => self.activation_tab(cfg, inf),
            3 => self.overlay_tab(cfg, inf),
            4 => self.advanced_tab(cfg),
            _ => self.hotkeys_tab(cfg, inf),
        }
    }

    /// `DevicesTabWidget`.
    fn devices_tab(&self, cfg: &Config, inf: &Info) -> Vec<Cat> {
        let d = &self.devices;
        // The activation threshold and the microphone test button.
        let test_active = inf.input_device.is_some() && inf.speakers.is_some();
        let mut test = Btn::new(if self.mic_test { "speaker_menu" } else { "speaker_menu_disabled" }, Act::MicTest);
        test.active = test_active;
        if !test_active {
            test.tip = Some(t("gui.plasmovoice.devices.not_available"));
        }
        let mut threshold = Opt::new(
            "voice.activation_threshold",
            &t("gui.plasmovoice.devices.activation_threshold"),
            El::Threshold,
            path_reset(cfg, "voice.activation_threshold"),
        )
        .tip(Some(t("gui.plasmovoice.devices.activation_threshold.tooltip")));
        threshold.el_w = ELEMENT_WIDTH - 24.0;
        threshold.buttons.push(test);

        // The microphone list (`createMicrophoneEntry`).
        let wanted = cfg.s("voice.input_device");
        let current = inf.input_device.clone();
        let input_text = current.clone().unwrap_or_else(|| t("gui.plasmovoice.devices.not_available"));
        let selected = current.as_ref().and_then(|n| d.inputs.iter().position(|x| x == n));
        let mut mic = Opt::new(
            "voice.input_device",
            &t("gui.plasmovoice.devices.microphone"),
            El::Drop {
                items: d.inputs.clone(),
                selected,
                text: input_text,
                overflow: true,
                tips: Vec::new(),
                active: !d.inputs.is_empty() && !cfg.b("voice.disable_input_device"),
                choose: Choose::Device("voice.input_device".into(), d.inputs.clone(), d.default_input.clone()),
            },
            path_reset(cfg, "voice.input_device"),
        );
        if inf.input_error.is_some() && !cfg.b("voice.disable_input_device") {
            let name = if wanted.is_empty() { d.default_input.clone() } else { wanted };
            let mut warn = Btn::new("warning", Act::None);
            warn.color = Some(WARNING);
            warn.shadow = false;
            warn.tip = Some(tf("gui.plasmovoice.devices.failed_to_initialize_microphone.tooltip", &[&name]));
            mic.el_w = ELEMENT_WIDTH - 24.0;
            mic.buttons.push(warn);
        }

        let microphone = cat(
            t("gui.plasmovoice.devices.microphone"),
            vec![
                Item::Opt(threshold),
                Item::Opt(mic),
                volume(cfg, "voice.microphone_volume", "gui.plasmovoice.devices.microphone_volume", Some("gui.plasmovoice.devices.volume.tooltip")),
                toggle(cfg, "voice.noise_suppression", "gui.plasmovoice.devices.noise_suppression", Some("gui.plasmovoice.devices.noise_suppression.tooltip")),
                toggle(cfg, "voice.stereo_capture", "gui.plasmovoice.devices.stereo_capture", Some("gui.plasmovoice.devices.stereo_capture.tooltip")),
                toggle(cfg, "voice.disable_input_device", "gui.plasmovoice.devices.disable_input_device", Some("gui.plasmovoice.devices.disable_input_device.tooltip")),
            ],
        );

        let out_text = inf.speakers.clone().unwrap_or_else(|| t("gui.plasmovoice.devices.not_available"));
        let out_selected = inf.speakers.as_ref().and_then(|n| d.outputs.iter().position(|x| x == n));
        let output_device = Opt::new(
            "voice.output_device",
            &t("gui.plasmovoice.devices.output_device"),
            El::Drop {
                items: d.outputs.clone(),
                selected: out_selected,
                text: out_text,
                overflow: true,
                tips: Vec::new(),
                active: !d.outputs.is_empty(),
                choose: Choose::Device("voice.output_device".into(), d.outputs.clone(), d.default_output.clone()),
            },
            path_reset(cfg, "voice.output_device"),
        );
        let output = cat(
            t("gui.plasmovoice.devices.output"),
            vec![
                Item::Opt(output_device),
                volume(cfg, "voice.volume", "gui.plasmovoice.devices.volume", Some("gui.plasmovoice.devices.volume.tooltip")),
                toggle(cfg, "voice.sound_occlusion", "gui.plasmovoice.devices.occlusion", Some("gui.plasmovoice.devices.occlusion.tooltip")),
                toggle(cfg, "voice.directional_sources", "gui.plasmovoice.devices.directional_sources", Some("gui.plasmovoice.devices.directional_sources.tooltip")),
                toggle(cfg, "voice.hrtf", "gui.plasmovoice.devices.hrtf", Some("gui.plasmovoice.devices.hrtf.tooltip")),
            ],
        );
        vec![microphone, output]
    }

    /// `VolumeTabWidget`.
    fn volume_tab(&self, cfg: &Config, inf: &Info) -> Vec<Cat> {
        let mut lines: Vec<&SourceLine> = inf.lines.iter().collect();
        lines.reverse();
        let filterable = lines.len() > 3;
        let show_all = !filterable || cfg.b("voice.show_all_source_lines");
        let mut sources = cat(t("gui.plasmovoice.volume.sources"), Vec::new());
        if filterable {
            let all = cfg.b("voice.show_all_source_lines");
            let mut eye = Btn::new(if all { "eye" } else { "eye_disabled" }, Act::ShowAll);
            eye.tip = Some(t(if all { "gui.plasmovoice.volume.sources.show_recent" } else { "gui.plasmovoice.volume.sources.show_all" }));
            sources.trailing.push(eye);
        }
        let recent = |l: &SourceLine| inf.line_used.get(&l.name).is_some_and(|t| t.elapsed().as_secs() <= 5 * 60);
        for l in lines.into_iter().filter(|l| show_all || l.name == plasmovoice_engine::tcp::PROXIMITY || recent(l)) {
            let vol = format!("voice.volumes.{}.volume", l.name);
            let mute = format!("voice.volumes.{}.muted", l.name);
            sources.items.push(Item::Opt(volume_row(cfg, &vol, &mute, Label::Icon(l.icon.clone(), t(&l.translation)))));
        }

        // `createPlayersSearch` and `refreshPlayerEntries`: the voice chat's players, the source
        // lines' and the direct sources' senders, merged by UUID, then searched. (Plasmo Voice
        // searches only the first list, so the others' players stay whatever is typed.)
        let search = self.search.value.to_lowercase();
        let mut players: Vec<(u128, String)> = Vec::new();
        let mut join = |id: u128, name: &str| {
            if let Some(p) = players.iter_mut().find(|p| p.0 == id) {
                p.1 = name.to_owned();
            } else {
                players.push((id, name.to_owned()));
            }
        };
        for p in &inf.players {
            join(p.id.0, &p.nick);
        }
        for l in &inf.lines {
            for p in l.players.iter().flatten() {
                join(p.id.0, &p.name);
            }
        }
        for (id, name) in &inf.direct_senders {
            join(*id, name);
        }
        players.retain(|p| p.0 != inf.me && p.1.to_lowercase().contains(&search));
        players.sort_by(|a, b| a.1.encode_utf16().cmp(b.1.encode_utf16()));
        let rows = players
            .into_iter()
            .map(|(id, name)| {
                let uuid = plasmovoice_engine::wire::Uuid(id);
                let vol = format!("voice.volumes.source_{uuid}.volume");
                let mute = format!("voice.volumes.source_{uuid}.muted");
                let mut o = volume_row(cfg, &vol, &mute, Label::Head(id, name));
                o.height = 24.0;
                o
            })
            .collect();
        let players = cat(t("gui.plasmovoice.volume.players"), vec![Item::Search, Item::Players(rows)]);
        vec![sources, players]
    }

    /// `ActivationTabWidget`.
    fn activation_tab(&self, cfg: &Config, inf: &Info) -> Vec<Cat> {
        let conflicts = conflicts(cfg, &inf.activations);
        let mut out = Vec::new();
        let parent = inf.activations.iter().find(|a| a.name == plasmovoice_engine::tcp::PROXIMITY);
        if let Some(p) = parent {
            out.push(self.activation(cfg, inf, p, false, &conflicts));
        }
        for a in inf.activations.iter().rev().filter(|a| a.name != plasmovoice_engine::tcp::PROXIMITY) {
            out.push(self.activation(cfg, inf, a, true, &conflicts));
        }
        out
    }

    fn activation(&self, cfg: &Config, inf: &Info, a: &Activation, can_inherit: bool, conflicts: &HashMap<String, Vec<String>>) -> Cat {
        let name = t(&a.translation);
        let type_path = format!("activations.{}.type", a.name);
        let toggle_path = format!("activations.{}.toggle", a.name);
        let kind = cfg.s(&type_path);
        let types = ["PUSH_TO_TALK", "VOICE", "INHERIT"];
        let ptt = kind == "PUSH_TO_TALK";
        let index = types.iter().position(|x| *x == kind).unwrap_or(0);
        let mut names = vec![t("gui.plasmovoice.activation.type_ptt"), t("gui.plasmovoice.activation.type_voice")];
        if can_inherit {
            names.push(t("gui.plasmovoice.activation.type_inherit"));
        }
        // The Inherit item's tooltip: what the parent does now.
        let parent_tip = inf.activations.iter().find(|p| p.name == plasmovoice_engine::tcp::PROXIMITY).map(|p| {
            let pkind = cfg.s(&format!("activations.{}.type", p.name));
            let type_name = match pkind.as_str() {
                "VOICE" => t("gui.plasmovoice.activation.type_voice"),
                "INHERIT" => t("gui.plasmovoice.activation.type_inherit"),
                _ => {
                    let keys = cfg.hotkey(&format!("key.plasmovoice.{}.ptt", p.name)).map(|h| h.keys.clone()).unwrap_or_default();
                    tf("gui.plasmovoice.activation.type_ptt_key", &[&key_names(&keys)])
                }
            };
            tf("gui.plasmovoice.activation.type_inherit.tooltip", &[&t(&p.translation), &format!("§7{}", tf("gui.plasmovoice.toggle.currently", &[&type_name]))])
        });
        let tips = vec![None, None, parent_tip];
        let mut row = Opt::new(
            &type_path,
            &t("gui.plasmovoice.activation.type"),
            El::Drop {
                items: names.clone(),
                selected: Some(index),
                text: names.get(index).cloned().unwrap_or_default(),
                overflow: false,
                tips,
                active: true,
                choose: Choose::Value(type_path.clone(), types.iter().map(|s| V::S((*s).into())).collect()),
            },
            path_reset(cfg, &type_path),
        );
        row.el_w = if ptt { ELEMENT_WIDTH } else { ELEMENT_WIDTH - 24.0 };
        if let Some(higher) = conflicts.get(&a.name) {
            row.el_w -= 24.0;
            let joined = higher.iter().map(|n| t(n)).collect::<Vec<_>>().join("\", \"");
            let mut warn = Btn::new("warning", Act::None);
            warn.color = Some(WARNING);
            warn.shadow = false;
            warn.tip = Some(tf("gui.plasmovoice.activation.conflict", &[&name, &joined]));
            row.buttons.push(warn);
        }
        if !ptt {
            // `createToggleStateButtons`: the microphone when on, crossed out when toggled off.
            let off = cfg.b(&toggle_path);
            let mut b = Btn::new(if off { "microphone_menu_disabled" } else { "microphone_menu" }, Act::Invert(toggle_path.clone()));
            b.tip = Some(tf("gui.plasmovoice.activation.toggle", &[&name, &currently(!off)]));
            row.buttons.push(b);
        }
        let mut items = vec![Item::Opt(row)];
        // The push-to-talk or toggle key.
        let (label, key) = if ptt {
            ("gui.plasmovoice.activation.ptt_button", format!("key.plasmovoice.{}.ptt", a.name))
        } else {
            ("gui.plasmovoice.activation.toggle_button", format!("key.plasmovoice.{}.toggle", a.name))
        };
        items.push(hotkey_row(cfg, &key, &t(label)));
        if !a.distances.is_empty() {
            let path = distance_path(&inf.server_id, a);
            let label = tf("gui.plasmovoice.activation.distance", &[&name]);
            let el = if a.distances[0] == -1 {
                El::Number(path.clone())
            } else {
                El::Distance(path.clone(), a.distances.clone())
            };
            items.push(Item::Opt(Opt::new(&path, &label, el, path_reset(cfg, &path))));
        }
        cat(name, items)
    }

    /// `OverlayTabWidget`.
    fn overlay_tab(&self, cfg: &Config, inf: &Info) -> Vec<Cat> {
        let position = |path: &str, label: &str, icon: bool| {
            let key = format!("gui.plasmovoice.overlay.hud_position.{}", cfg.s(path).to_lowercase());
            Item::Opt(Opt::new(path, &t(label), El::Button(t(&key), Act::Position(icon)), path_reset(cfg, path)))
        };
        let icon = cat(
            t("gui.plasmovoice.overlay.activation_icon"),
            vec![
                toggle(cfg, "overlay.show_activation_icon", "gui.plasmovoice.overlay.activation_icon_show", None),
                position("overlay.activation_icon_position", "gui.plasmovoice.overlay.activation_icon_position", true),
            ],
        );
        let show_icons: Vec<String> = ["hud", "always", "hidden"].iter().map(|k| t(&format!("gui.plasmovoice.overlay.show_source_icons.{k}"))).collect();
        let si = cfg.i("overlay.show_source_icons").clamp(0, 2) as usize;
        let sources = cat(
            t("gui.plasmovoice.overlay.source_icons"),
            vec![
                Item::Opt(Opt::new(
                    "overlay.show_source_icons",
                    &t("gui.plasmovoice.overlay.show_source_icons"),
                    El::Drop {
                        items: show_icons.clone(),
                        selected: Some(si),
                        text: show_icons[si].clone(),
                        overflow: true,
                        tips: Vec::new(),
                        active: true,
                        choose: Choose::Value("overlay.show_source_icons".into(), vec![V::I(0), V::I(1), V::I(2)]),
                    },
                    path_reset(cfg, "overlay.show_source_icons"),
                )),
                toggle(cfg, "overlay.show_static_source_icons", "gui.plasmovoice.overlay.show_static_source_icons", None),
                // Horizon's own: the icons in the player list (plasmovoice_tab).
                toggle(cfg, "horizon.tab_list_icons", "gui.plasmovoice.overlay.tab_list_icons", Some("gui.plasmovoice.overlay.tab_list_icons.tooltip")),
            ],
        );
        let styles = ["NAME_SKIN", "SKIN", "NAME"];
        let style_names: Vec<String> = ["name_skin", "skin", "name"].iter().map(|k| t(&format!("gui.plasmovoice.overlay.style.{k}"))).collect();
        let st = styles.iter().position(|s| *s == cfg.s("overlay.overlay_style")).unwrap_or(0);
        let overlay = cat(
            t("gui.plasmovoice.overlay"),
            vec![
                toggle(cfg, "overlay.overlay_enabled", "gui.plasmovoice.overlay.enable", None),
                position("overlay.overlay_position", "gui.plasmovoice.overlay.position", false),
                Item::Opt(Opt::new(
                    "overlay.overlay_style",
                    &t("gui.plasmovoice.overlay.style"),
                    El::Drop {
                        items: style_names.clone(),
                        selected: Some(st),
                        text: style_names[st].clone(),
                        overflow: false,
                        tips: Vec::new(),
                        active: true,
                        choose: Choose::Value("overlay.overlay_style".into(), styles.iter().map(|s| V::S((*s).into())).collect()),
                    },
                    path_reset(cfg, "overlay.overlay_style"),
                )),
            ],
        );
        let mut lines = cat(t("gui.plasmovoice.overlay.sources"), Vec::new());
        let displays: Vec<String> = ["when_talking", "always", "never"].iter().map(|k| t(&format!("gui.plasmovoice.overlay.sources.{k}"))).collect();
        for l in inf.lines.iter().rev() {
            let path = format!("overlay.source_states.{}", l.name);
            let state = cfg.s(&path);
            let label = Label::Icon(l.icon.clone(), t(&l.translation));
            let o = if l.players.is_none() {
                // A line without players: a toggle over ON/OFF (Horizon's default: When Talking, on).
                let on = state != "OFF" && state != "NEVER";
                let mut o = Opt::new(&path, "", El::ToggleWith(on, Act::Set(path.clone(), V::S(if on { "OFF" } else { "ON" }.into()))), path_reset(cfg, &path));
                o.label = label;
                o
            } else {
                let i = match state.as_str() {
                    "ALWAYS" => 1,
                    "NEVER" | "OFF" => 2,
                    _ => 0,
                };
                let mut o = Opt::new(
                    &path,
                    "",
                    El::Drop {
                        items: displays.clone(),
                        selected: Some(i),
                        text: displays[i].clone(),
                        overflow: true,
                        tips: Vec::new(),
                        active: true,
                        choose: Choose::Value(path.clone(), ["WHEN_TALKING", "ALWAYS", "NEVER"].iter().map(|s| V::S((*s).into())).collect()),
                    },
                    (!cfg.is_default(&path), Act::Set(path.clone(), V::S("WHEN_TALKING".into()))),
                );
                o.label = label;
                o
            };
            lines.items.push(Item::Opt(o));
        }
        vec![icon, sources, overlay, lines]
    }

    /// `AdvancedTabWidget`.
    fn advanced_tab(&self, cfg: &Config) -> Vec<Cat> {
        let visual = cat(
            t("gui.plasmovoice.advanced.visual"),
            vec![
                toggle(cfg, "advanced.visualize_voice_distance", "gui.plasmovoice.advanced.visualize_voice_distance", None),
                toggle(cfg, "advanced.visualize_voice_distance_on_join", "gui.plasmovoice.advanced.visualize_voice_distance_on_join", None),
            ],
        );
        let overlaps = ["MUTE_DIRECT", "MUTE_PROXIMITY", "OFF"];
        let overlap_names: Vec<String> =
            ["mute_direct", "mute_proximity", "off"].iter().map(|k| t(&format!("gui.plasmovoice.advanced.source_types_overlap.{k}"))).collect();
        let oi = overlaps.iter().position(|s| *s == cfg.s("advanced.source_types_overlap")).unwrap_or(1);
        let engine = cat(
            t("gui.plasmovoice.advanced.audio_engine"),
            vec![
                Item::Opt(
                    Opt::new(
                        "advanced.directional_sources_angle",
                        &t("gui.plasmovoice.advanced.directional_sources_angle"),
                        El::IntSlider("advanced.directional_sources_angle".into(), 100, 360, String::new()),
                        path_reset(cfg, "advanced.directional_sources_angle"),
                    )
                    .tip(Some(t("gui.plasmovoice.advanced.directional_sources_angle.tooltip"))),
                ),
                toggle(cfg, "advanced.stereo_sources_to_mono", "gui.plasmovoice.advanced.stereo_sources_to_mono", Some("gui.plasmovoice.advanced.stereo_sources_to_mono.tooltip")),
                toggle(cfg, "advanced.panning", "gui.plasmovoice.advanced.panning", None),
                Item::Opt(
                    Opt::new(
                        "advanced.source_types_overlap",
                        &t("gui.plasmovoice.advanced.source_types_overlap"),
                        El::Drop {
                            items: overlap_names.clone(),
                            selected: Some(oi),
                            text: overlap_names[oi].clone(),
                            overflow: false,
                            tips: Vec::new(),
                            active: true,
                            choose: Choose::Value("advanced.source_types_overlap".into(), overlaps.iter().map(|s| V::S((*s).into())).collect()),
                        },
                        path_reset(cfg, "advanced.source_types_overlap"),
                    )
                    .tip(Some(t("gui.plasmovoice.advanced.source_types_overlap.tooltip"))),
                ),
            ],
        );
        let exponential = cat(
            t("gui.plasmovoice.advanced.exponential_volume"),
            vec![
                toggle(cfg, "advanced.exponential_volume_slider", "gui.plasmovoice.advanced.exponential_volume.volume_slider", None),
                toggle(cfg, "advanced.exponential_distance_gain", "gui.plasmovoice.advanced.exponential_volume.distance_gain", None),
            ],
        );
        vec![visual, engine, exponential]
    }

    /// `HotKeysTabWidget`: the categories of `ConfigHotkeys` but "hidden" (in its HashMap's
    /// order: General, then Sound Occlusion), then each activation's distance keys.
    fn hotkeys_tab(&self, cfg: &Config, inf: &Info) -> Vec<Cat> {
        let mut out = Vec::new();
        for category in ["key.plasmovoice.general", "key.plasmovoice.occlusion"] {
            let items = cfg.hotkeys.iter().filter(|h| h.category == category).map(|h| hotkey_row(cfg, &h.name, &t(&h.name))).collect();
            out.push(cat(t(category), items));
        }
        for a in inf.activations.iter().rev() {
            if a.distances.is_empty() || a.distances[0] == -1 {
                continue;
            }
            let title = tf("key.plasmovoice.distance", &[&t(&a.translation)]);
            out.push(cat(
                title,
                vec![
                    hotkey_row(cfg, &format!("key.plasmovoice.{}.distance_increase", a.name), &t("key.plasmovoice.distance.increase")),
                    hotkey_row(cfg, &format!("key.plasmovoice.{}.distance_decrease", a.name), &t("key.plasmovoice.distance.decrease")),
                ],
            ));
        }
        out
    }
}

/// A volume row of the Volume tab (`VolumeOption`): slider, mute button, reset of both.
fn volume_row(cfg: &Config, vol: &str, mute: &str, label: Label) -> Opt {
    let muted = cfg.b(mute);
    let mut o = Opt::new(vol, "", El::Volume(vol.to_owned()), (!(cfg.is_default(vol) && cfg.is_default(mute)), Act::Reset(vec![vol.to_owned(), mute.to_owned()])));
    o.label = label;
    o.el_w = ELEMENT_WIDTH - 24.0;
    o.buttons.push(Btn::new(if muted { "speaker_menu_disabled" } else { "speaker_menu" }, Act::Invert(mute.to_owned())));
    o
}

fn hotkey_row(cfg: &Config, name: &str, label: &str) -> Item {
    let default = cfg.hotkey(name).is_none_or(|h| h.keys == h.default);
    Item::Opt(Opt::new(name, label, El::Hotkey(name.to_owned()), (!default, Act::ResetHotkey(name.to_owned()))))
}

/// `detectActivationConflicts`, for proximity and other activations: an activation that a
/// heavier non-transitive one may block (both push to talk on the same keys, or both by voice).
fn conflicts(cfg: &Config, activations: &[Activation]) -> HashMap<String, Vec<String>> {
    let mut out: HashMap<String, Vec<String>> = HashMap::new();
    for proximity in [true, false] {
        let mut list: Vec<&Activation> = activations.iter().filter(|a| a.proximity == proximity).collect();
        list.sort_by_key(|a| a.weight);
        if list.len() < 2 {
            continue;
        }
        let kind = |a: &Activation| cfg.s(&format!("activations.{}.type", a.name));
        let keys = |a: &Activation| cfg.hotkey(&format!("key.plasmovoice.{}.ptt", a.name)).map(|h| h.keys.clone()).unwrap_or_default();
        for i in 0..list.len() {
            let cur = list[i];
            for higher in &list[i + 1..] {
                if higher.transitive {
                    continue;
                }
                let (hk, ck) = (kind(higher), kind(cur));
                let conflict = if hk == "PUSH_TO_TALK" && ck == "PUSH_TO_TALK" {
                    let k = keys(higher);
                    !k.is_empty() && {
                        let mut a = k.clone();
                        let mut b = keys(cur);
                        a.sort();
                        b.sort();
                        a == b
                    }
                } else {
                    hk != "PUSH_TO_TALK" && ck != "PUSH_TO_TALK"
                };
                if conflict {
                    out.entry(cur.name.clone()).or_default().push(higher.translation.clone());
                }
            }
        }
    }
    out
}

// ---- layout ---------------------------------------------------------------------------------

/// A row's parts on the screen.
struct Parts {
    label: Rect,
    el: Rect,
    buttons: Vec<Rect>,
    reset: Rect,
}

/// `OptionWidget`'s row: the label takes what the fixed widgets leave (gap 4).
fn parts(o: &Opt, x: f32, y: f32) -> Parts {
    let fixed = o.el_w + 20.0 * o.buttons.len() as f32 + 20.0;
    let count = 3 + o.buttons.len();
    let label_w = CONTENT_WIDTH - fixed - ELEMENT_GAP * (count - 1) as f32;
    let cy = |h: f32| y + ((o.height - h) / 2.0).floor();
    let mut bx = x + label_w + ELEMENT_GAP;
    let el = [bx, cy(20.0), o.el_w, 20.0];
    bx += o.el_w + ELEMENT_GAP;
    let mut buttons = Vec::new();
    for _ in &o.buttons {
        buttons.push([bx, cy(20.0), 20.0, 20.0]);
        bx += 20.0 + ELEMENT_GAP;
    }
    Parts { label: [x, y, label_w, o.height], el, buttons, reset: [bx, cy(20.0), 20.0, 20.0] }
}

/// Where each thing of the page is, in screen pixels.
enum Spot<'a> {
    Title(&'a Cat, f32),
    Opt(&'a Opt, f32),
    Search(f32),
}

struct Layout {
    h: f32,
    nav: f32,
    x: f32,
    content: f32,
}

impl Layout {
    fn max_scroll(&self) -> f32 {
        (self.content - (self.h - self.nav)).max(0.0)
    }
}

fn page_height(cats: &[Cat]) -> f32 {
    let mut h = PAGE_PADDING * 2.0;
    for (i, c) in cats.iter().enumerate() {
        if i > 0 {
            h += CATEGORY_GAP;
        }
        h += 20.0;
        for item in &c.items {
            h += ELEMENT_GAP + item_height(item);
        }
    }
    h
}

fn item_height(item: &Item) -> f32 {
    match item {
        Item::Opt(o) => o.height,
        Item::Search => 20.0,
        Item::Players(rows) => rows.iter().map(|r| r.height).sum::<f32>() + 6.0 * rows.len().saturating_sub(1) as f32,
    }
}

fn spots<'a>(cats: &'a [Cat], top: f32) -> Vec<Spot<'a>> {
    let mut out = Vec::new();
    let mut y = top + PAGE_PADDING;
    for (i, c) in cats.iter().enumerate() {
        if i > 0 {
            y += CATEGORY_GAP;
        }
        out.push(Spot::Title(c, y));
        y += 20.0;
        for item in &c.items {
            y += ELEMENT_GAP;
            match item {
                Item::Opt(o) => out.push(Spot::Opt(o, y)),
                Item::Search => out.push(Spot::Search(y)),
                Item::Players(rows) => {
                    let mut py = y;
                    for r in rows {
                        out.push(Spot::Opt(r, py));
                        py += r.height + 6.0;
                    }
                }
            }
            y += item_height(item);
        }
    }
    out
}

impl Screen {
    fn title(&self) -> String {
        tf("gui.plasmovoice.title", &["Plasmo Voice", VERSION])
    }

    /// The tab buttons' widths (`textWidth + 24`).
    fn tab_widths(atlas: &HudAtlas) -> Vec<f32> {
        Self::tabs().iter().map(|(k, _)| width(atlas, &t(k)) + 24.0).collect()
    }

    /// `VoiceSettingsNavigation.isMinimized`.
    fn minimized(&self, atlas: &HudAtlas, w: f32) -> bool {
        let title_w = 14.0 + width(atlas, &self.title()) + 4.0;
        let widths = Self::tab_widths(atlas);
        let buttons_w = widths.iter().sum::<f32>() + 4.0 * (widths.len() - 1) as f32;
        let bx = (w / 2.0).floor() - (buttons_w / 2.0).floor();
        bx < title_w || title_w + buttons_w + 62.0 > w
    }

    /// The tab buttons' rectangles (`renderButtons`).
    fn tab_rects(&self, atlas: &HudAtlas, w: f32) -> Vec<Rect> {
        let widths = Self::tab_widths(atlas);
        let (mut x, mut y) = (14.0, 36.0);
        if !self.minimized(atlas, w) {
            let buttons_w = widths.iter().sum::<f32>() + 4.0 * (widths.len() - 1) as f32;
            x = (w / 2.0).floor() - (buttons_w / 2.0).floor();
            y = 8.0;
        }
        widths
            .iter()
            .map(|&bw| {
                if x + bw > w - 8.0 {
                    x = 14.0;
                    y += 26.0;
                }
                let r = [x, y, bw, 20.0];
                x += bw + 4.0;
                r
            })
            .collect()
    }

    /// `getHeight`: 36, or 36 + 28 a line of tab buttons when minimized.
    fn nav_height(&self, atlas: &HudAtlas, w: f32) -> f32 {
        if !self.minimized(atlas, w) {
            return 36.0;
        }
        let widths = Self::tab_widths(atlas);
        let (mut x, mut lines) = (14.0, 1.0);
        for bw in widths {
            if x + bw > w - 8.0 {
                x = 14.0;
                lines += 1.0;
            }
            x += bw + 4.0;
        }
        36.0 + lines * 28.0
    }

    fn layout(&self, atlas: &HudAtlas, w: f32, h: f32, cats: &[Cat]) -> Layout {
        let nav = self.nav_height(atlas, w);
        Layout { h, nav, x: ((w - CONTENT_WIDTH) / 2.0).floor(), content: page_height(cats) }
    }
}

// ---- drawing --------------------------------------------------------------------------------

pub(super) fn render(game: &Game<'_>, batch: &mut HudBatch, cfg: &Config) {
    with(|s| s.draw(game, batch, cfg));
}

impl Screen {
    fn draw(&mut self, game: &Game<'_>, batch: &mut HudBatch, cfg: &Config) {
        let atlas = &game.gui.atlas;
        let (w, h) = (batch.width.floor(), batch.height.floor());
        // `Screen.renderBackground` in a world: the blur and the dark menu background.
        game.native_apply_screen_blur();
        ui::menu_background(batch, atlas, 0.0, 0.0, w, h, true);
        match self.page {
            Page::NotAvailable => self.draw_not_available(game, batch, w, h),
            Page::Position(icon) => self.draw_position(game, batch, cfg, w, h, icon),
            Page::Settings => self.draw_settings(game, batch, cfg, w, h),
        }
    }

    fn not_available_message(&self) -> String {
        let (has_client, closed) = info(|i| (i.has_client, i.closed));
        if has_client && closed {
            tf("gui.plasmovoice.cannot_connect_to_udp", &["§ehttps://plasmovoice.com/docs/server/installing§r"])
        } else if has_client {
            t("gui.plasmovoice.connecting")
        } else {
            t("gui.plasmovoice.not_available")
        }
    }

    /// `VoiceNotAvailableScreen`: the message centred in a 248 × 50 box, Close under it.
    fn draw_not_available(&mut self, game: &Game<'_>, batch: &mut HudBatch, w: f32, h: f32) {
        let atlas = &game.gui.atlas;
        let y = ((h - 50.0) / 2.0).floor();
        let x = ((w - 248.0) / 2.0).floor();
        let lines = wrap(atlas, &self.not_available_message(), w);
        for (i, l) in lines.iter().enumerate() {
            centered(batch, atlas, l, (w / 2.0).floor(), y + 9.0 * i as f32, WHITE);
        }
        let by = y + 9.0 * lines.len() as f32 + 20.0;
        let r = [x + 10.0, by, 228.0, 20.0];
        button(batch, atlas, r, &t("message.plasmovoice.close"), true, inside(game.cursor, r));
    }

    fn position_buttons(w: f32, h: f32, icon: bool) -> Vec<(Rect, &'static str)> {
        let (left, center, right) = (25.0, (w / 2.0).floor() - 50.0, w - 25.0 - 100.0);
        let (top, bottom) = (25.0, h - 20.0 - 25.0);
        if icon {
            vec![
                ([left, top, 100.0, 20.0], "TOP_LEFT"),
                ([center, top, 100.0, 20.0], "TOP_CENTER"),
                ([right, top, 100.0, 20.0], "TOP_RIGHT"),
                ([left, bottom, 100.0, 20.0], "BOTTOM_LEFT"),
                ([center, bottom, 100.0, 20.0], "BOTTOM_CENTER"),
                ([right, bottom, 100.0, 20.0], "BOTTOM_RIGHT"),
            ]
        } else {
            vec![
                ([left, top, 100.0, 20.0], "TOP_LEFT"),
                ([right, top, 100.0, 20.0], "TOP_RIGHT"),
                ([left, bottom, 100.0, 20.0], "BOTTOM_LEFT"),
                ([right, bottom, 100.0, 20.0], "BOTTOM_RIGHT"),
            ]
        }
    }

    /// `HudPositionScreen`: "Choose ... Position" in the middle, a button per corner; the other
    /// HUD element's corner is off.
    fn draw_position(&mut self, game: &Game<'_>, batch: &mut HudBatch, cfg: &Config, w: f32, h: f32, icon: bool) {
        let atlas = &game.gui.atlas;
        let disabled = cfg.s(if icon { "overlay.overlay_position" } else { "overlay.activation_icon_position" });
        for (r, pos) in Self::position_buttons(w, h, icon) {
            let active = pos != disabled;
            let label = t(&format!("gui.plasmovoice.overlay.hud_position.{}", pos.to_lowercase()));
            button(batch, atlas, r, &label, active, active && inside(game.cursor, r));
        }
        let choose = t(if icon { "gui.plasmovoice.overlay.activation_icon_position.choose" } else { "gui.plasmovoice.overlay.position.choose" });
        let tw = width(atlas, &choose);
        text(batch, atlas, &choose, (w / 2.0).floor() - (tw / 2.0).floor(), (h / 2.0).floor() - 9.0, WHITE, true);
    }

    fn draw_settings(&mut self, game: &Game<'_>, batch: &mut HudBatch, cfg: &Config, w: f32, h: f32) {
        let atlas = &game.gui.atlas;
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f64();
        self.last_frame = now;
        // `MicrophoneTestController`: a louder frame lifts the meter, which falls back slowly.
        let (frames, level) = info(|i| (i.frames, i.level));
        if frames != self.frames {
            self.frames = frames;
            let value = level_to_range(level);
            if level > -60.0 && value > self.meter {
                self.meter = value;
            }
        }
        self.meter = (self.meter - 1.2 * dt).max(0.0);

        let inf = INFO.with(|i| i.borrow().clone());
        let cats = self.build(cfg, &inf);
        let l = self.layout(atlas, w, h, &cats);
        self.scroll = self.scroll.clamp(0.0, l.max_scroll());
        let cursor = game.cursor;
        // An open list, a held slider or the held scrollbar take the hover away (`render`).
        let blocked = self.dropdown.is_some() || self.dragging.is_some() || self.scrollbar.is_some();
        let mouse = if blocked { (-1.0, -1.0) } else { cursor };
        let in_view = mouse.1 >= l.nav && mouse.1 < h && mouse.0 >= 0.0 && mouse.0 <= w;
        let content_mouse = if in_view { mouse } else { (-1.0, -1.0) };
        let mut tip: Option<String> = None;
        let mut deferred: Option<(Rect, Vec<String>, Option<usize>, Vec<Option<String>>, bool)> = None;

        // The tab (`renderTab`), clipped to its area.
        let start = batch.vertices.len();
        let top = l.nav - self.scroll.floor();
        for spot in spots(&cats, top) {
            match spot {
                Spot::Title(c, y) => {
                    if y + 20.0 < l.nav || y > h {
                        continue;
                    }
                    let tw = width(atlas, &c.title);
                    let s = truncated(atlas, &c.title, CONTENT_WIDTH);
                    let _ = tw;
                    centered(batch, atlas, &s, l.x + (CONTENT_WIDTH / 2.0).floor(), y + 6.0, WHITE);
                    let mut bx = l.x + CONTENT_WIDTH;
                    for b in c.trailing.iter().rev() {
                        bx -= 20.0;
                        let r = [bx, y, 20.0, 20.0];
                        let hover = inside(content_mouse, r);
                        icon_button(batch, atlas, r, b, hover);
                        if hover && let Some(t) = &b.tip {
                            tip = Some(t.clone());
                        }
                        bx -= ELEMENT_GAP;
                    }
                }
                Spot::Search(y) => {
                    self.search.x = l.x;
                    self.search.y = y;
                    self.search.width = CONTENT_WIDTH;
                    self.search.height = 20.0;
                    self.search.focused = self.focus == Some(Focus::Search);
                    self.search.draw(batch, atlas, None);
                    if self.search.value.is_empty() && !self.search.focused {
                        text(batch, atlas, &format!("§7{}", t("gui.plasmovoice.volume.players_search")), l.x + 4.0, y + 6.0, BLACK, true);
                    }
                }
                Spot::Opt(o, y) => {
                    if y + o.height < l.nav || y > h {
                        continue;
                    }
                    let p = parts(o, l.x, y);
                    if let Some(t) = self.draw_row(game, batch, cfg, &inf, o, &p, content_mouse, &mut deferred) {
                        tip = Some(t);
                    }
                }
            }
        }
        batch.clip_since(start, l.x, l.nav, l.x + CONTENT_WIDTH, h);
        // The scrollbar (`ScrollContainer.renderScrollbar`).
        let max = l.max_scroll();
        if max > 0.0 {
            let x0 = l.x + CONTENT_WIDTH + 13.0;
            let (t0, t1) = (l.nav + 4.0, h - 4.0);
            let track = t1 - t0;
            let thumb = (track * track / l.content.max(1.0)).floor().clamp(32.0, (track - 8.0).max(32.0));
            let thumb_top = (t0 + (self.scroll.floor() * (track - thumb) / max).floor()).max(t0);
            fill(batch, atlas, x0, t0, x0 + 6.0, t1, BLACK);
            fill(batch, atlas, x0, thumb_top, x0 + 6.0, thumb_top + thumb, TEXT_MUTED);
            fill(batch, atlas, x0, thumb_top, x0 + 5.0, thumb_top + thumb - 1.0, 0xFFC0_C0C0);
        }

        // The header (`renderBackground`): the list background and its separator.
        ui::tiled(batch, atlas, "gui/inworld_menu_list_background", 0.0, 0.0, w, l.nav, 32.0, u32::MAX);
        ui::tiled_sized(batch, atlas, "gui/inworld_footer_separator", 0.0, l.nav, w, 2.0, [32.0, 2.0], u32::MAX);
        let title = self.title();
        text(batch, atlas, &title, 14.0, 15.0, WHITE, true);

        // The tab buttons (`TabButton`) and the microphone and voice toggles.
        for (i, r) in self.tab_rects(atlas, w).into_iter().enumerate() {
            let (key, icon) = Self::tabs()[i];
            let active = i != self.tab;
            let hover = active && inside(mouse, r);
            button(batch, atlas, r, "", active, hover);
            let icon_name = if active { icon.to_owned() } else { format!("{icon}_disabled") };
            icons::draw_shadowed(batch, atlas, &icon_name, r[0] + 6.0, r[1] + 6.0, 8.0, WHITE, Some(if active { WHITE } else { GRAY }));
            text(batch, atlas, &t(key), r[0] + 16.0, r[1] + 6.0, if active { WHITE } else { GRAY }, true);
        }
        let mic_off = cfg.b("voice.microphone_disabled");
        let voice_off = cfg.b("voice.disabled");
        for (r, icon, key, off) in [
            ([w - 52.0, 8.0, 20.0, 20.0], if mic_off { "microphone_menu_disabled" } else { "microphone_menu" }, "gui.plasmovoice.toggle.microphone", mic_off),
            ([w - 28.0, 8.0, 20.0, 20.0], if voice_off { "speaker_menu_disabled" } else { "speaker_menu" }, "gui.plasmovoice.toggle.voice", voice_off),
        ] {
            let hover = inside(mouse, r);
            icon_button(batch, atlas, r, &Btn::new(icon, Act::None), hover);
            if hover {
                tip = Some(tf(key, &[&currently(!off)]));
            }
        }

        // An open dropdown's list over everything (`deferredRender`).
        if let Some((r, items, selected, tips, overflow)) = deferred {
            let _ = selected;
            if let Some(t) = self.draw_list(game, batch, &l, r, &items, &tips, overflow, cursor) {
                tip = Some(t);
            }
        }
        if let Some(lines) = tip {
            let lines: Vec<Text> = wrap(atlas, &lines, 180.0).iter().map(|l| styled(l)).collect();
            ui::tooltip(batch, atlas, &lines, cursor);
        }
    }

    /// One row: its label, its widget, its buttons and reset; its tooltip when hovered.
    #[allow(clippy::too_many_arguments)]
    fn draw_row(
        &mut self,
        game: &Game<'_>,
        batch: &mut HudBatch,
        cfg: &Config,
        inf: &Info,
        o: &Opt,
        p: &Parts,
        mouse: (f32, f32),
        deferred: &mut Option<(Rect, Vec<String>, Option<usize>, Vec<Option<String>>, bool)>,
    ) -> Option<String> {
        let atlas = &game.gui.atlas;
        let mut tip = None;
        // The label (`TextWidget`, an icon label, a player's face).
        let ly = p.label[1] + ((o.height - 8.0) / 2.0).floor();
        match &o.label {
            Label::Text(s) => {
                let s = truncated(atlas, s, p.label[2]);
                text(batch, atlas, &s, p.label[0], ly, WHITE, true);
            }
            Label::Icon(icon, s) => {
                icons::draw(batch, atlas, icon, p.label[0], p.label[1] + 2.0, 16.0, 16.0, WHITE);
                let s = truncated(atlas, s, p.label[2] - 20.0);
                text(batch, atlas, &s, p.label[0] + 20.0, ly, WHITE, true);
            }
            Label::Head(id, name) => {
                let skin = inf.skins.get(id).cloned().unwrap_or_else(|| horizon_client::tab_list::default_skin(*id).to_owned());
                let mut draws = Vec::new();
                horizon_client::tab_list::face(&mut draws, &skin, p.label[0] as i32, p.label[1] as i32, 24, true, false, horizon_client::hud_layout::WHITE);
                game.native_gui_paint(batch, &draws);
                let s = truncated(atlas, name, p.label[2] - 30.0);
                text(batch, atlas, &s, p.label[0] + 30.0, ly, WHITE, true);
            }
        }
        if inside(mouse, p.label) && o.tip.is_some() {
            tip.clone_from(&o.tip);
        }

        // The widget.
        let r = p.el;
        let hover = inside(mouse, r);
        let held = self.dragging.as_deref() == Some(o.id.as_str());
        match &o.el {
            El::Toggle(_) | El::ToggleWith(..) => {
                // `ToggleButton`: the slider track, the handle left (off) or right (on).
                let on = match &o.el {
                    El::Toggle(path) => cfg.b(path),
                    El::ToggleWith(on, _) => *on,
                    _ => false,
                };
                sprite(batch, atlas, "slider", r);
                let x0 = if on { r[0] + r[2] - 8.0 } else { r[0] };
                sprite(batch, atlas, if hover { "button_highlighted" } else { "button" }, [x0, r[1], 8.0, 20.0]);
                centered(batch, atlas, &on_off(on), r[0] + (r[2] / 2.0).floor(), r[1] + 6.0, WHITE);
            }
            El::Volume(path) => {
                let max = 2.0;
                let v = (cfg.f(path) / max).clamp(0.0, 1.0);
                let label = format!("{}%", round(v * max * 100.0));
                slider(batch, atlas, r, &label, v, hover || held, true);
            }
            El::IntSlider(path, min, max, suffix) => {
                let n = cfg.i(path);
                let v = (n - min) as f64 / (max - min) as f64;
                let label = if suffix.is_empty() { format!("{n} ") } else { format!("{n} {suffix}") };
                slider(batch, atlas, r, &label, v, hover || held, true);
            }
            El::Threshold => {
                // `ActivationThresholdWidget`: the track, the level in green, yellow or red, the
                // handle and the threshold in dB.
                let v = level_to_range(cfg.f("voice.activation_threshold"));
                sprite(batch, atlas, "slider", r);
                let color = if self.meter > 0.95 {
                    0xFFFF_0000
                } else if self.meter > 0.7 {
                    0xFFFF_FF00
                } else {
                    0xFF00_FF00
                };
                let mw = ((r[2] - 2.0) as f64 * self.meter) as i32 as f32;
                if mw > 0.0
                    && let Some(mut s) = atlas.sprites.get("gui/sprites/widget/slider").copied()
                {
                    let (du, dv) = ((s.u1 - s.u0) / 200.0, (s.v1 - s.v0) / 20.0);
                    s.u0 += du;
                    s.u1 = s.u0 + du * mw;
                    s.v0 += dv;
                    s.v1 = s.v0 + dv * 18.0;
                    batch.rect_uv(r[0] + 1.0, r[1] + 1.0, mw, 18.0, s, c(color));
                }
                let x0 = r[0] + (v * f64::from(r[2] - 8.0)) as i32 as f32;
                sprite(batch, atlas, if hover || held { "slider_handle_highlighted" } else { "slider_handle" }, [x0, r[1], 8.0, 20.0]);
                let label = format!("{:.0} dB", range_to_level(v));
                centered(batch, atlas, &label, r[0] + (r[2] / 2.0).floor(), r[1] + 6.0, WHITE);
            }
            El::Drop { items, selected, text: label, overflow, tips, active, .. } => {
                // `DropDownWidget`: a text field with the choice and an arrow.
                let open = self.dropdown.as_ref().is_some_and(|(id, _)| *id == o.id);
                sprite(batch, atlas, if open { "text_field_highlighted" } else { "text_field" }, r);
                if *active {
                    for i in 0..5 {
                        let i = i as f32;
                        let (x0, y0, x1, y1) = if open {
                            (r[0] + r[2] - (9.0 + i), r[1] + ((r[3] - 5.0) / 2.0).floor() + i, r[0] + r[2] - (8.0 - i), r[1] + ((r[3] - 5.0) / 2.0).floor() + 2.0 + i)
                        } else {
                            (
                                r[0] + r[2] - (13.0 - i),
                                r[1] + ((r[3] - 5.0) / 2.0).floor() + if i > 0.0 { 1.0 + i } else { 0.0 },
                                r[0] + r[2] - (4.0 + i),
                                r[1] + ((r[3] - 5.0) / 2.0).floor() + 2.0 + i,
                            )
                        };
                        fill(batch, atlas, x0, y0, x1, y1, GRAY);
                    }
                }
                let text_w = if *active { r[2] - 23.0 } else { r[2] - 5.0 };
                let shown = truncated(atlas, label, text_w);
                text(batch, atlas, &shown, r[0] + 5.0, r[1] + 6.0, if *active { TEXT } else { TEXT_DISABLED }, true);
                if open {
                    *deferred = Some((r, items.clone(), *selected, tips.clone(), *overflow));
                } else if hover {
                    let i = selected.unwrap_or(usize::MAX);
                    let element_tip = tips.get(i).cloned().flatten();
                    let over = (*overflow && width(atlas, label) > text_w).then(|| label.clone());
                    if let Some(t) = element_tip.or(over) {
                        tip = Some(t);
                    }
                }
            }
            El::Button(label, _) => button(batch, atlas, r, label, true, hover),
            El::Hotkey(name) => {
                let keys = cfg.hotkey(name).map(|h| h.keys.clone()).unwrap_or_default();
                let capturing = self.capture.as_ref().filter(|(n, _)| n == name);
                let label = match capturing {
                    Some((_, pressed)) => {
                        let shown = if pressed.is_empty() { key_names(&keys) } else { key_names(pressed) };
                        format!("§e> {shown} <")
                    }
                    None => key_names(&keys),
                };
                button(batch, atlas, r, "", true, hover);
                let s = if capturing.is_some() { label.clone() } else { truncated(atlas, &label, r[2] - 16.0) };
                centered(batch, atlas, &s, r[0] + (r[2] / 2.0).floor(), r[1] + 6.0, WHITE);
                if hover && capturing.is_none() && width(atlas, &label) > r[2] - 16.0 {
                    tip = Some(label);
                }
            }
            El::Distance(path, distances) => {
                let d = cfg.i(path) as i32;
                let v = if distances.len() > 1 {
                    distances.iter().position(|x| *x == d).map_or(-1.0, |i| i as f64) / (distances.len() - 1) as f64
                } else {
                    0.0
                };
                slider(batch, atlas, r, &d.to_string(), v.max(0.0), hover || held, true);
            }
            El::Number(path) => {
                if self.focus == Some(Focus::Number(o.id.clone())) {
                    let n = &mut self.number;
                    (n.x, n.y, n.width, n.height, n.focused) = (r[0], r[1], r[2], r[3], true);
                    n.draw(batch, atlas, None);
                } else {
                    let mut n = EditBox::new(r[0], r[1], r[2], r[3]);
                    n.focused = false;
                    n.set_value(&cfg.i(path).to_string());
                    n.draw(batch, atlas, None);
                }
            }
        }

        // Its buttons and the reset button (`reset.png`, active when not the default).
        for (b, br) in o.buttons.iter().zip(&p.buttons) {
            let hover = inside(mouse, *br);
            icon_button(batch, atlas, *br, b, hover);
            if hover && let Some(t) = &b.tip {
                tip = Some(t.clone());
            }
        }
        let mut reset = Btn::new("reset", Act::None);
        reset.active = o.reset.0;
        icon_button(batch, atlas, p.reset, &reset, reset.active && inside(mouse, p.reset));
        tip
    }

    /// `DropDownWidgetList`: under the dropdown (over it when there is no room below), 17 px
    /// items, five at most, a scrollbar when longer.
    fn list_rect(&self, l: &Layout, r: Rect, count: usize) -> (Rect, bool) {
        let max_h = DROP_ITEM * count.min(DROP_VISIBLE) as f32;
        let down = r[1] + r[3] + 1.0 + max_h;
        let up = r[1] - 1.0 - max_h;
        let to_top = down > l.h && up > l.nav;
        if to_top {
            ([r[0], r[1] - max_h, r[2], max_h], true)
        } else {
            let y = r[1] + r[3];
            ([r[0], y, r[2], max_h.min(l.h - y)], false)
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_list(&mut self, game: &Game<'_>, batch: &mut HudBatch, l: &Layout, r: Rect, items: &[String], tips: &[Option<String>], overflow: bool, cursor: (f32, f32)) -> Option<String> {
        let atlas = &game.gui.atlas;
        let (lr, to_top) = self.list_rect(l, r, items.len());
        let content = DROP_ITEM * items.len() as f32;
        let max = (content - lr[3]).max(0.0);
        let scroll = self.dropdown.as_mut().map(|d| {
            d.1 = d.1.clamp(0.0, max);
            d.1
        });
        let scroll = scroll.unwrap_or(0.0).floor();
        let frame = 0xFF46_4646;
        let hover_c = 0xFF32_3232;
        let y_off = if to_top { 1.0 } else { 0.0 };
        let pad = if max > 0.0 { 23.0 } else { 10.0 };
        let mut tip = None;
        let start = batch.vertices.len();
        for (i, item) in items.iter().enumerate() {
            let y = lr[1] + DROP_ITEM * i as f32 - scroll;
            if y + DROP_ITEM < lr[1] || y > lr[1] + lr[3] {
                continue;
            }
            let ir = [lr[0], y, lr[2], DROP_ITEM];
            fill(batch, atlas, ir[0], ir[1], ir[0] + ir[2], ir[1] + ir[3], frame);
            fill(batch, atlas, ir[0] + 1.0, y + y_off, ir[0] + ir[2] - 1.0, y + DROP_ITEM - 1.0 + y_off, BLACK);
            let over_bar = max > 0.0 && cursor.0 >= lr[0] + lr[2] - 11.0;
            if inside(cursor, ir) && inside(cursor, lr) {
                let t = tips.get(i).cloned().flatten().or_else(|| (overflow && width(atlas, item) > lr[2] - pad).then(|| item.clone()));
                if t.is_some() && !over_bar {
                    tip = t;
                }
                fill(batch, atlas, ir[0] + 1.0, y + y_off, ir[0] + ir[2] - 1.0, y + DROP_ITEM - 1.0 + y_off, hover_c);
            }
            let s = truncated(atlas, item, lr[2] - pad);
            text(batch, atlas, &s, ir[0] + 5.0, y + (DROP_ITEM / 2.0).floor() - 4.0 + y_off, TEXT, true);
        }
        batch.clip_since(start, lr[0], lr[1], lr[0] + lr[2], lr[1] + lr[3]);
        if max > 0.0 {
            let x0 = lr[0] + lr[2] - 11.0;
            let (t0, t1) = (lr[1] + 4.0, lr[1] + lr[3] - 4.0);
            let track = t1 - t0;
            let thumb = (track * track / content).floor().clamp(32.0_f32.min(track), (track - 8.0).max(1.0));
            let thumb_top = (t0 + (scroll * (track - thumb) / max).floor()).max(t0);
            fill(batch, atlas, x0, thumb_top, x0 + 6.0, thumb_top + thumb, TEXT_MUTED);
            fill(batch, atlas, x0, thumb_top, x0 + 5.0, thumb_top + thumb - 1.0, 0xFFC0_C0C0);
            let line_y = if to_top { lr[1] } else { lr[1] + lr[3] };
            fill(batch, atlas, lr[0], line_y - 1.0, lr[0] + lr[2], line_y, frame);
        }
        tip
    }
}

/// `font.split` at `w`, and at each `\n`.
fn wrap(atlas: &HudAtlas, s: &str, w: f32) -> Vec<String> {
    let mut out = Vec::new();
    for line in s.split('\n') {
        if line.is_empty() {
            out.push(String::new());
            continue;
        }
        let mut cur = String::new();
        let mut color = String::new();
        for word in line.split(' ') {
            let candidate = if cur.is_empty() { format!("{color}{word}") } else { format!("{cur} {word}") };
            if !cur.is_empty() && width(atlas, &candidate) > w {
                // Carry the colour on to the next line, as the font's split keeps styles.
                if let Some(i) = cur.rfind('§') {
                    color = cur[i..].chars().take(2).collect();
                }
                out.push(std::mem::take(&mut cur));
                cur = format!("{color}{word}");
            } else {
                cur = candidate;
            }
        }
        out.push(cur);
    }
    out
}

// ---- input ----------------------------------------------------------------------------------

/// What the pointer is over.
enum Hit {
    Tab(usize),
    MicToggle,
    VoiceToggle,
    Trailing(Btn),
    El(Opt, Rect),
    Button(Btn),
    Reset(Opt),
    Search,
    Nothing,
}

impl Screen {
    fn hit(&self, atlas: &HudAtlas, cfg: &Config, w: f32, h: f32, p: (f32, f32)) -> (Hit, Layout) {
        let inf = INFO.with(|i| i.borrow().clone());
        let cats = self.build(cfg, &inf);
        let l = self.layout(atlas, w, h, &cats);
        for (i, r) in self.tab_rects(atlas, w).into_iter().enumerate() {
            if inside(p, r) && i != self.tab {
                return (Hit::Tab(i), l);
            }
        }
        if inside(p, [w - 52.0, 8.0, 20.0, 20.0]) {
            return (Hit::MicToggle, l);
        }
        if inside(p, [w - 28.0, 8.0, 20.0, 20.0]) {
            return (Hit::VoiceToggle, l);
        }
        if p.1 < l.nav {
            return (Hit::Nothing, l);
        }
        let top = l.nav - self.scroll.floor();
        for spot in spots(&cats, top) {
            match spot {
                Spot::Title(c, y) => {
                    let mut bx = l.x + CONTENT_WIDTH;
                    for b in c.trailing.iter().rev() {
                        bx -= 20.0;
                        if inside(p, [bx, y, 20.0, 20.0]) {
                            return (Hit::Trailing(b.clone()), l);
                        }
                        bx -= ELEMENT_GAP;
                    }
                }
                Spot::Search(y) => {
                    if inside(p, [l.x, y, CONTENT_WIDTH, 20.0]) {
                        return (Hit::Search, l);
                    }
                }
                Spot::Opt(o, y) => {
                    let parts = parts(o, l.x, y);
                    if inside(p, parts.el) {
                        return (Hit::El(o.clone(), parts.el), l);
                    }
                    for (b, r) in o.buttons.iter().zip(&parts.buttons) {
                        if inside(p, *r) {
                            return (Hit::Button(b.clone()), l);
                        }
                    }
                    if inside(p, parts.reset) {
                        return (Hit::Reset(o.clone()), l);
                    }
                }
            }
        }
        (Hit::Nothing, l)
    }

    fn act(&mut self, cfg: &mut Config, act: &Act) {
        match act {
            Act::None => {}
            Act::Invert(path) => cfg.invert(path),
            Act::Set(path, v) => cfg.set(path, v.clone()),
            Act::Reset(paths) => {
                for p in paths {
                    cfg.reset(p);
                }
            }
            Act::ResetHotkey(name) => {
                if let Some(d) = cfg.hotkey(name).map(|h| h.default.clone()) {
                    cfg.set_hotkey(name, d);
                }
            }
            Act::MicTest => self.mic_test = !self.mic_test,
            Act::Position(icon) => {
                self.dropdown = None;
                self.page = Page::Position(*icon);
            }
            Act::ShowAll => cfg.invert("voice.show_all_source_lines"),
        }
    }

    /// Sets a slider's value from the pointer (`setValueFromMouse` and `applyValue`).
    fn slide(&mut self, cfg: &mut Config, o: &Opt, r: Rect, mx: f32) {
        let v = ((f64::from(mx) - f64::from(r[0] + 4.0)) / f64::from(r[2] - 8.0)).clamp(0.0, 1.0);
        match &o.el {
            El::Volume(path) => {
                let max = 2.0;
                // Steps of 5% unless Left Shift is held.
                let shift = super::plasmovoice_glue::held("key.keyboard.left.shift");
                let v = if shift { v } else { (round(v * max * 100.0 / 5.0) * 5) as f64 / (max * 100.0) };
                cfg.set(path, V::F(v * max));
            }
            El::IntSlider(path, min, max, _) => cfg.set(path, V::I((v * (max - min) as f64) as i64 + min)),
            El::Threshold => cfg.set("voice.activation_threshold", V::F(range_to_level(v))),
            El::Distance(path, distances) if !distances.is_empty() => {
                let index = round(v * (distances.len() - 1) as f64).clamp(0, distances.len() as i64 - 1) as usize;
                cfg.set(path, V::I(i64::from(distances[index])));
            }
            _ => {}
        }
    }

    fn choose(&mut self, cfg: &mut Config, choose: &Choose, i: usize) {
        match choose {
            Choose::Device(path, names, default) => {
                if let Some(name) = names.get(i) {
                    cfg.set(path, V::S(if name == default { String::new() } else { name.clone() }));
                }
            }
            Choose::Value(path, values) => {
                if let Some(v) = values.get(i) {
                    cfg.set(path, v.clone());
                }
            }
        }
    }

    pub(super) fn press(&mut self, atlas: &HudAtlas, cfg: &mut Config, w: f32, h: f32, p: (f32, f32), button: i32) {
        match self.page {
            Page::NotAvailable => {
                let y = ((h - 50.0) / 2.0).floor();
                let x = ((w - 248.0) / 2.0).floor();
                let lines = wrap(atlas, &self.not_available_message(), w);
                let by = y + 9.0 * lines.len() as f32 + 20.0;
                if button == 0 && inside(p, [x + 10.0, by, 228.0, 20.0]) {
                    self.requests.push(Request::Click);
                    self.requests.push(Request::Close);
                }
                return;
            }
            Page::Position(icon) => {
                let disabled = cfg.s(if icon { "overlay.overlay_position" } else { "overlay.activation_icon_position" });
                for (r, pos) in Self::position_buttons(w, h, icon) {
                    if button == 0 && inside(p, r) && pos != disabled {
                        self.requests.push(Request::Click);
                        cfg.set(if icon { "overlay.activation_icon_position" } else { "overlay.overlay_position" }, V::S(pos.into()));
                        self.page = Page::Settings;
                    }
                }
                return;
            }
            Page::Settings => {}
        }
        // A hotkey being bound takes the mouse buttons (`HotKeyWidget.mouseClicked`).
        if let Some((_, pressed)) = &mut self.capture {
            let key = mouse_name(button);
            if pressed.len() < 3 && !pressed.contains(&key) {
                pressed.push(key);
            }
            return;
        }
        let inf = INFO.with(|i| i.borrow().clone());
        let cats = self.build(cfg, &inf);
        let l = self.layout(atlas, w, h, &cats);
        // An open list takes the click: an item, or closing (`DropDownWidget.mouseClicked`).
        if let Some((id, scroll)) = self.dropdown.clone() {
            let found = spots(&cats, l.nav - self.scroll.floor()).into_iter().find_map(|s| match s {
                Spot::Opt(o, y) if o.id == id => Some((o.clone(), parts(o, l.x, y).el)),
                _ => None,
            });
            self.dropdown = None;
            self.requests.push(Request::Click);
            if let Some((o, r)) = found
                && let El::Drop { items, choose, .. } = &o.el
            {
                let (lr, _) = self.list_rect(&l, r, items.len());
                let content = DROP_ITEM * items.len() as f32;
                let over_bar = content > lr[3] && p.0 >= lr[0] + lr[2] - 11.0;
                if inside(p, lr) && !over_bar && button == 0 {
                    let i = ((p.1 - lr[1] + scroll.floor()) / DROP_ITEM) as usize;
                    if i < items.len() {
                        self.choose(cfg, choose, i);
                    }
                } else if inside(p, lr) {
                    self.dropdown = Some((id, scroll));
                    self.requests.pop();
                }
            }
            return;
        }
        // The scrollbar.
        let max = l.max_scroll();
        let bar_x = l.x + CONTENT_WIDTH + 13.0;
        if button == 0 && max > 0.0 && p.0 >= bar_x && p.0 < bar_x + 6.0 && p.1 >= l.nav && p.1 <= h {
            self.scrollbar = Some(p.1);
            return;
        }
        if button != 0 {
            return;
        }
        let (hit, l) = self.hit(atlas, cfg, w, h, p);
        // A click elsewhere takes the focus from a field.
        let keep_focus = matches!(&hit, Hit::Search) || matches!(&hit, Hit::El(o, _) if matches!(o.el, El::Number(..)));
        if !keep_focus {
            self.focus = None;
        }
        match hit {
            Hit::Tab(i) => {
                self.requests.push(Request::Click);
                self.open_tab(i);
            }
            Hit::MicToggle => {
                self.requests.push(Request::Click);
                cfg.invert("voice.microphone_disabled");
            }
            Hit::VoiceToggle => {
                self.requests.push(Request::Click);
                cfg.invert("voice.disabled");
            }
            Hit::Trailing(b) | Hit::Button(b) => {
                if b.active {
                    self.requests.push(Request::Click);
                    self.act(cfg, &b.act);
                }
            }
            Hit::Reset(o) => {
                if o.reset.0 {
                    self.requests.push(Request::Click);
                    self.act(cfg, &o.reset.1);
                }
            }
            Hit::Search => {
                self.focus = Some(Focus::Search);
                self.search.focused = true;
                self.search.click(atlas, p.0, false, false);
            }
            Hit::El(o, r) => match &o.el {
                El::Toggle(path) => {
                    self.requests.push(Request::Click);
                    cfg.invert(path);
                }
                El::ToggleWith(_, act) => {
                    self.requests.push(Request::Click);
                    self.act(cfg, act);
                }
                El::Volume(_) | El::IntSlider(..) | El::Threshold | El::Distance(..) => {
                    // `AbstractSlider.isHovered` leaves out its last pixel.
                    if p.0 < r[0] + r[2] - 1.0 {
                        self.dragging = Some(o.id.clone());
                        self.slide(cfg, &o, r, p.0);
                    }
                }
                El::Drop { active, .. } => {
                    if *active {
                        self.requests.push(Request::Click);
                        self.dropdown = Some((o.id.clone(), 0.0));
                    }
                }
                El::Button(_, act) => {
                    self.requests.push(Request::Click);
                    self.act(cfg, act);
                }
                El::Hotkey(name) => {
                    self.requests.push(Request::Click);
                    self.capture = Some((name.clone(), Vec::new()));
                    super::plasmovoice_glue::reset_hotkeys();
                }
                El::Number(path) => {
                    if self.focus != Some(Focus::Number(o.id.clone())) {
                        self.number.set_value(&cfg.i(path).to_string());
                    }
                    self.focus = Some(Focus::Number(o.id.clone()));
                    self.number.focused = true;
                    self.number.click(atlas, p.0, false, false);
                }
            },
            Hit::Nothing => {}
        }
        if self.focus != Some(Focus::Search) {
            self.search.focused = false;
        }
        let _ = l;
    }

    pub(super) fn release(&mut self, atlas: &HudAtlas, cfg: &mut Config, w: f32, h: f32, button: i32) {
        let _ = (atlas, w, h);
        // A mouse button of the hotkey being bound let go: the keys are its keys.
        if let Some((name, pressed)) = &self.capture {
            let key = mouse_name(button);
            if !(button == 0 && pressed.is_empty()) && pressed.contains(&key) {
                let (name, pressed) = (name.clone(), pressed.clone());
                cfg.set_hotkey(&name, pressed);
                self.capture = None;
            }
            return;
        }
        self.scrollbar = None;
        if self.dragging.take().is_some() {
            self.requests.push(Request::Click);
        }
    }

    pub(super) fn dragged(&mut self, atlas: &HudAtlas, cfg: &mut Config, w: f32, h: f32, p: (f32, f32)) {
        if let Some(from) = self.scrollbar {
            let inf = INFO.with(|i| i.borrow().clone());
            let cats = self.build(cfg, &inf);
            let l = self.layout(atlas, w, h, &cats);
            let max = l.max_scroll().max(1.0);
            let view = h - l.nav;
            let thumb = (view * view / l.content.max(1.0)).clamp(32.0, view);
            let multiplier = (max / (view - thumb).max(1.0)).max(1.0);
            if p.1 < l.nav {
                self.scroll = 0.0;
            } else if p.1 > h {
                self.scroll = max;
            } else {
                self.scroll += (p.1 - from) * multiplier;
            }
            self.scrollbar = Some(p.1);
            return;
        }
        let Some(id) = self.dragging.clone() else { return };
        let inf = INFO.with(|i| i.borrow().clone());
        let cats = self.build(cfg, &inf);
        let l = self.layout(atlas, w, h, &cats);
        let found = spots(&cats, l.nav - self.scroll.floor()).into_iter().find_map(|s| match s {
            Spot::Opt(o, y) if o.id == id => Some((o.clone(), parts(o, l.x, y).el)),
            _ => None,
        });
        if let Some((o, r)) = found {
            self.slide(cfg, &o, r, p.0);
        }
    }

    pub(super) fn scrolled(&mut self, delta: f32) {
        if self.page != Page::Settings {
            return;
        }
        if let Some((_, s)) = &mut self.dropdown {
            *s -= delta * DROP_ITEM;
            return;
        }
        self.scroll -= delta * SCROLL_STEP;
    }

    /// A key: a hotkey being bound, a field, a slider's arrows, the menu key, Escape. True when
    /// the screen took it (it goes no further).
    pub(super) fn key_pressed(&mut self, atlas: &HudAtlas, cfg: &mut Config, key: &str, mods: u32, menu_key: &str) -> (bool, Option<super::plasmovoice_glue::Edit>) {
        let _ = atlas;
        let escape = key == "key.keyboard.escape";
        match self.page {
            Page::Position(_) if escape => {
                self.page = Page::Settings;
                return (true, None);
            }
            Page::Position(_) | Page::NotAvailable => {
                if key == menu_key && self.page == Page::NotAvailable {
                    self.requests.push(Request::Close);
                    return (true, None);
                }
                return (false, None);
            }
            Page::Settings => {}
        }
        if let Some((name, pressed)) = &mut self.capture {
            if escape {
                // Escape: the keys pressed are the keys, or none clears it.
                let (name, pressed) = (name.clone(), pressed.clone());
                cfg.set_hotkey(&name, pressed);
                self.capture = None;
            } else if pressed.len() < 3 && !pressed.iter().any(|k| k == key) {
                pressed.push(key.to_owned());
            }
            return (true, None);
        }
        if let Some((id, _)) = &self.dropdown
            && escape
        {
            let _ = id;
            self.dropdown = None;
            return (true, None);
        }
        let control = mods & 2 != 0;
        let shift = mods & 1 != 0;
        if let Some(focus) = self.focus.clone() {
            if escape {
                self.focus = None;
                self.search.focused = false;
                return (false, None);
            }
            let edit = match key {
                "key.keyboard.v" if control => Some(super::plasmovoice_glue::Edit::Paste),
                "key.keyboard.c" if control => Some(super::plasmovoice_glue::Edit::Copy(self.focused_box(&focus).selected_text())),
                "key.keyboard.x" if control => {
                    let b = self.focused_box(&focus);
                    let t = b.selected_text();
                    b.delete_chars(0);
                    Some(super::plasmovoice_glue::Edit::Copy(t))
                }
                _ => {
                    if let Some(code) = horizon_client::keybinds::key_code(key) {
                        let b = self.focused_box(&focus);
                        b.key(code, control, shift);
                        b.reset_blink();
                    }
                    None
                }
            };
            self.after_edit(cfg, &focus);
            return (true, edit);
        }
        if key == menu_key {
            self.requests.push(Request::Close);
            return (true, None);
        }
        (false, None)
    }

    /// A slider or the scrollbar is held: the config isn't saved mid-drag.
    pub(super) fn busy(&self) -> bool {
        self.dragging.is_some() || self.scrollbar.is_some()
    }

    fn focused_box(&mut self, focus: &Focus) -> &mut EditBox {
        match focus {
            Focus::Search => &mut self.search,
            Focus::Number(_) => &mut self.number,
        }
    }

    /// A field changed: the search filters again; a distance field sets its distance
    /// (`NumberTextFieldWidget`'s responder: empty resets, else at least min + 1).
    fn after_edit(&mut self, cfg: &mut Config, focus: &Focus) {
        if let Focus::Number(path) = focus {
            let s = self.number.value.clone();
            if s.is_empty() {
                cfg.reset(path);
            } else if let Ok(n) = s.parse::<i64>() {
                cfg.set(path, V::I(n.max(0)));
            }
        }
    }

    pub(super) fn key_released(&mut self, cfg: &mut Config, key: &str) -> bool {
        if let Some((name, pressed)) = &self.capture {
            if pressed.iter().any(|k| k == key) {
                let (name, pressed) = (name.clone(), pressed.clone());
                cfg.set_hotkey(&name, pressed);
                self.capture = None;
            }
            return true;
        }
        false
    }

    /// Typed characters into the focused field (not in the first frames: the key that opened
    /// the screen types too, `renderCount < 5`).
    pub(super) fn typed(&mut self, cfg: &mut Config, s: &str, max: Option<i64>) -> bool {
        if self.opened.elapsed().as_millis() < 100 || self.capture.is_some() {
            return self.capture.is_some();
        }
        let Some(focus) = self.focus.clone() else { return false };
        let text: String = s.chars().filter(|c| !c.is_control()).collect();
        if let Focus::Number(_) = focus {
            // `setFilter`: digits only, within 1..max.
            let before = self.number.value.clone();
            self.number.insert_text(&text);
            let v = &self.number.value;
            let ok = v.is_empty() || (v.chars().all(|c| c.is_ascii_digit()) && v.parse::<i64>().is_ok_and(|n| n > 0 && max.is_none_or(|m| n <= m)));
            if !ok {
                self.number.set_value(&before);
            }
        } else {
            self.search.insert_text(&text);
        }
        self.focused_box(&focus).reset_blink();
        self.after_edit(cfg, &focus);
        true
    }

    /// The distance field's maximum, for its filter.
    pub(super) fn number_max(&self) -> Option<i64> {
        let Some(Focus::Number(path)) = &self.focus else { return None };
        info(|i| {
            i.activations.iter().find(|a| distance_path(&i.server_id, a) == *path).and_then(|a| a.distances.last().map(|d| i64::from(*d)))
        })
    }

    pub(super) fn paste(&mut self, cfg: &mut Config, s: &str) {
        let max = self.number_max();
        self.typed(cfg, s, max);
    }
}

/// A GLFW mouse button's key name.
fn mouse_name(button: i32) -> String {
    match button {
        0 => "key.mouse.left".into(),
        1 => "key.mouse.right".into(),
        2 => "key.mouse.middle".into(),
        n => format!("key.mouse.{}", n + 1),
    }
}

