// BetterF3's screens: `ModConfigScreen` (General Settings, Left and Right
// Modules, Done), `ModulesScreen` with its `ModuleListWidget` (move a
// module up or down, Edit, Add, Delete), and the Cloth Config screens it
// builds (`GeneralOptionsScreen`, `EditModulesScreen`, `AddModuleScreen`).
// They open from Options > Mods > BetterF3 (Configure...), from Mod Menu's
// Configure (`MODMENU_CONFIGURE`), and with F3+M over the game
// (`KeyboardMixin`, which also has F3+F for the render distance). Ported from
// BetterF3 by cominixo and TreyRuffy (MIT, https://github.com/TreyRuffy/BetterF3).
use super::betterf3::{self as b, Kind, Module, ModuleRef, tr};
use super::betterf3_cloth::{self as cloth, Entry, Request, Screen as Cloth};
use super::betterf3_overlay as overlay;
use super::{Game, Screen};
use horizon_client::clipboard::Clipboard as _;
use horizon_client::hud::HudBatch;
use horizon_client::mod_api::NativeScreenView;
use horizon_client::options_ui::Page as OptionsPage;
use horizon_client::text::Text;
use horizon_client::ui;
use std::cell::RefCell;

/// Mod Menu's Configure: `native_settings_click(id, MODMENU_CONFIGURE)` after
/// it opened the mod's page.
pub(super) const MODMENU_CONFIGURE: usize = 0x4D4D_4346;

/// BetterF3's icon (`fabric/src/main/resources/icon.png`), for Mod Menu.
const ICON: &[u8] = horizon_client::native_data!("betterf3", "icon.png");

/// Where the screens are drawn.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Host {
    /// Over the mod's page in Options > Mods (row); `modmenu` when Mod
    /// Menu's Configure opened it.
    Page { row: usize, modmenu: bool },
    /// A screen of the mod's own over the game (F3+M).
    Own,
}

/// `ModulesScreen` and its list.
struct Modules {
    left: bool,
    entries: Vec<ModuleRef>,
    selected: Option<usize>,
    scroll: ui::Scroll,
    /// The list has the focus (after an arrow moved a module).
    focused: bool,
}

/// What a Cloth Config screen edits.
enum ClothKind {
    General,
    Edit(ModuleRef),
    Add,
}

enum Page {
    Config,
    Modules(Modules),
    Cloth(Box<Cloth>, ClothKind),
}

struct Ui {
    host: Host,
    in_world: bool,
    stack: Vec<Page>,
    /// `ModConfigScreen.init` is due (it opened, or a child closed).
    init: bool,
}

/// What the input hooks leave for `frame` (which has `&mut Game`).
enum Action {
    Click,
    /// Done or Escape on the first screen: back to its parent, or closed
    /// altogether (`setScreen(null)`).
    Leave { all: bool },
}

#[derive(Default)]
struct State {
    ui: Option<Ui>,
    actions: Vec<Action>,
    started: bool,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

fn with<R>(f: impl FnOnce(&mut State) -> R) -> Option<R> {
    STATE.with(|s| s.try_borrow_mut().ok().map(|mut s| f(&mut s)))
}

/// Mod Menu's `modmenu.configure`, the page's first button.
fn configure_label() -> &'static str {
    if b::russian() { "Настройки..." } else { "Configure..." }
}

fn data_dir(game: &Game<'_>) -> std::path::PathBuf {
    game.native_data_dir(b::ID)
}

// ---- the mod's page ------------------------------------------------------------------------

/// The page's buttons: Configure... only.
pub(super) fn page(mod_id: &str, labels: &mut Vec<String>) {
    if mod_id == b::ID {
        labels.insert(0, configure_label().into());
    }
}

/// Configure... (button 0) or Mod Menu's Configure opens `ModConfigScreen`.
pub(super) fn page_click(game: &mut Game<'_>, mod_id: &str, button: usize) {
    if mod_id != b::ID {
        return;
    }
    if button == MODMENU_CONFIGURE || button == 0 {
        let Some(row) = game.settings.mods.installed.iter().position(|m| m.id == b::ID && m.native.is_some()) else {
            return;
        };
        let in_world = game.ui.options.in_world;
        open(Host::Page { row, modmenu: button == MODMENU_CONFIGURE }, in_world);
    }
}

fn open(host: Host, in_world: bool) {
    with(|s| {
        s.ui = Some(Ui { host, in_world, stack: vec![Page::Config], init: true });
    });
}

/// The screens are up where they are drawn.
fn covers_page(game: &Game<'_>, host: Host) -> bool {
    match host {
        Host::Page { row, .. } => {
            game.screen == Screen::Options
                && game.ui.options.page() == OptionsPage::NativeMod(row)
                && game.settings.mods.installed.get(row).is_some_and(|m| m.id == b::ID)
        }
        Host::Own => game.native_mod_screen() == Some(b::ID),
    }
}

fn host() -> Option<Host> {
    with(|s| s.ui.as_ref().map(|u| u.host)).flatten()
}

/// The options are held back while the screens cover the mod's page.
pub(super) fn covers_options(game: &Game<'_>) -> bool {
    host().is_some_and(|h| matches!(h, Host::Page { .. }) && covers_page(game, h))
}

/// The screens are up over `screen`.
fn over(game: &Game<'_>, screen: &NativeScreenView<'_>) -> bool {
    match host() {
        Some(h @ Host::Page { .. }) => screen.id == "options" && covers_page(game, h),
        Some(Host::Own) => screen.id == "mod" && covers_page(game, Host::Own),
        None => false,
    }
}

// ---- the keys ------------------------------------------------------------------------------

/// F3+M or F3+F, used up by `handleDebugKeys`, runs at the next frame
/// with the modifiers still held.
fn debug_key(game: &mut Game<'_>) {
    use horizon_client::debug_keys::betterf3_keys as keys;
    use winit::keyboard::KeyCode;
    let held = |a: KeyCode, b: KeyCode| game.physical_keys.contains(&a) || game.physical_keys.contains(&b);
    let control = held(KeyCode::ControlLeft, KeyCode::ControlRight);
    let shift = held(KeyCode::ShiftLeft, KeyCode::ShiftRight);
    match keys::take() {
        keys::CONFIG => {
            // `setScreen(new ModConfigScreen(null))`.
            if game.native_mod_screen_open(b::ID) {
                open(Host::Own, game.player.is_some());
            }
        }
        keys::DISTANCE => cycle_distance(game, control, shift),
        _ => {}
    }
}

/// F3+F: the render distance one up (Shift: down), 2 to 32; with Ctrl the
/// simulation distance, 5 to 32. The new value goes to the chat.
fn cycle_distance(game: &mut Game<'_>, control: bool, shift: bool) {
    let step = if shift { -1 } else { 1 };
    let (key, value) = if control {
        let v = (game.settings.simulation_distance + step).clamp(5, 32);
        game.settings.simulation_distance = v;
        ("debug.betterf3.cycle_simulationdistance.message", v)
    } else {
        let v = (game.settings.render_distance + step).clamp(2, 32);
        game.settings.render_distance = v;
        ("debug.betterf3.cycle_renderdistance.message", v)
    };
    game.settings.save(&Game::options_path());
    game.apply_settings();
    // `debugFeedbackComponent`: "[Debug]:" in bold yellow, then the message.
    let message = Text::literal(b::trf(key, &[&value]));
    let line = Text::empty()
        .append(Text::translate("debug.prefix", &[]).with_color(horizon_client::text::YELLOW).bold())
        .append(Text::literal(" "))
        .append(message);
    game.say_system_queued(&line);
    game.system_message(line);
}

// ---- each frame ----------------------------------------------------------------------------

/// Mod Menu's icon and metadata in the mod's folder, each written when it
/// differs from what is there.
fn write_data(game: &Game<'_>) {
    let dir = data_dir(game);
    let _ = std::fs::create_dir_all(&dir);
    let json = format!(
        r#"{{
  "schema": 1,
  "name": "BetterF3",
  "version": "{}",
  "description": "Replaces Minecraft's original debug HUD with a highly customizable, more human-readable HUD.",
  "authors": ["cominixo", "TreyRuffy"],
  "contributors": ["DeportedToHell"],
  "license": "MIT",
  "links": {{
    "homepage": "https://modrinth.com/mod/betterf3",
    "sources": "https://github.com/DeportedToHell/MCRE-Mods/tree/main/betterf3",
    "issues": "https://github.com/DeportedToHell/MCRE-Mods/issues"
  }},
  "badges": ["client"],
  "config_screen": true
}}
"#,
        b::VERSION
    );
    for (name, bytes) in [("icon.png", ICON), ("modmenu.json", json.as_bytes())] {
        let path = dir.join(name);
        if std::fs::read(&path).ok().as_deref() != Some(bytes) {
            let _ = std::fs::write(&path, bytes);
        }
    }
}

/// Once a frame: the mod's files at first, the language, the keys' switch,
/// the overlay's animation, then what the screens asked for.
pub(super) fn frame(game: &mut Game<'_>) {
    b::set_language(&horizon_client::lang::code());
    if with(|s| !std::mem::replace(&mut s.started, true)).unwrap_or(false) {
        write_data(game);
        b::load(&data_dir(game));
    }
    horizon_client::debug_keys::betterf3_keys::ON.store(game.native_on(b::ID), std::sync::atomic::Ordering::Relaxed);
    debug_key(game);
    overlay::frame(game);
    let Some(h) = host() else {
        return;
    };
    if !covers_page(game, h) {
        // The page (or the own screen) went some other way: closed unsaved.
        with(|s| s.ui = None);
        return;
    }
    // `ModConfigScreen.init`: the overlay turned on in a world.
    let init = with(|s| {
        s.ui.as_mut().is_some_and(|u| matches!(u.stack.last(), Some(Page::Config)) && std::mem::take(&mut u.init))
    })
    .unwrap_or(false);
    if init && game.player.is_some() && !game.debug {
        game.debug = true;
    }
    let actions = with(|s| std::mem::take(&mut s.actions)).unwrap_or_default();
    for a in actions {
        match a {
            Action::Click => game.audio.play_button_click(),
            Action::Leave { all } => {
                leave(game, h, all);
                return;
            }
        }
    }
    cloth_requests(game);
}

/// The first screen closes: to its parent (Done), or all the way out
/// (Escape: `Screen.onClose` opens no screen).
fn leave(game: &mut Game<'_>, h: Host, all: bool) {
    with(|s| s.ui = None);
    match h {
        Host::Own => game.native_mod_screen_close(b::ID),
        Host::Page { modmenu: true, .. } => {
            // Back to Mod Menu, as Escape on the page it opened alone.
            let events = game.ui.options.back(&mut game.settings);
            let _ = game.handle_option_events(events);
        }
        Host::Page { row, modmenu: false } => {
            if all {
                if game.title.is_none() {
                    game.options_to_game = true;
                }
                let _ = game.handle_option_events(vec![horizon_client::options_ui::Event::Close]);
            } else {
                game.native_settings_refresh(row);
            }
        }
    }
}

/// What the open Cloth Config screen asked for.
fn cloth_requests(game: &mut Game<'_>) {
    let requests = with(|s| match s.ui.as_mut().and_then(|u| u.stack.last_mut()) {
        Some(Page::Cloth(c, _)) => c.take_requests(),
        _ => Vec::new(),
    })
    .unwrap_or_default();
    for r in requests {
        match r {
            Request::Click => game.audio.play_button_click(),
            Request::Paste => {
                let text = game.clipboard.get();
                with(|s| {
                    if let Some(Page::Cloth(c, _)) = s.ui.as_mut().and_then(|u| u.stack.last_mut()) {
                        c.typed(&text);
                    }
                });
            }
            Request::Copy(text) => {
                if !text.is_empty() {
                    game.clipboard.set(&text);
                }
            }
            Request::Close(saved) => {
                let page = with(|s| {
                    let u = s.ui.as_mut()?;
                    let page = u.stack.pop();
                    if matches!(u.stack.last(), Some(Page::Config)) {
                        u.init = true;
                    }
                    page
                })
                .flatten();
                if saved && let Some(Page::Cloth(c, kind)) = page {
                    save_cloth(game, &c, kind);
                }
                return;
            }
        }
    }
}

/// Save & Quit: every entry's save consumer, then `ModConfigFile.saveRunnable`.
fn save_cloth(game: &mut Game<'_>, c: &Cloth, kind: ClothKind) {
    let boolean = |id: &str| c.entry(id).is_some_and(|e| e.current().as_bool());
    match kind {
        ClothKind::General => {
            let mut g = b::general();
            let double = |id: &str, d: f64| c.entry(id).and_then(|e| e.current().as_text().trim().parse().ok()).unwrap_or(d);
            g.disable_mod = boolean("disable");
            g.auto_f3 = boolean("auto_start");
            g.space_modules = boolean("space_modules");
            g.performance = boolean("performance");
            g.shadow_text = boolean("shadow_text");
            g.animations = boolean("animations");
            g.animation_speed = double("animation_speed", 1.0);
            g.font_scale = double("font_scale", 1.0);
            if let Some(e) = c.entry("background") {
                g.background_color = e.color_value();
            }
            g.hide_debug_crosshair = boolean("debug_crosshair");
            // The 3D crosshair is switched off for good in a world.
            if game.player.is_some() && !g.disable_mod && g.hide_debug_crosshair {
                game.debug_entries.set_status("3d_crosshair", horizon_client::debug_entries::Status::Never);
                game.debug_entries.save();
            }
            g.hide_sidebar = boolean("sidebar");
            g.hide_bossbar = boolean("bossbar");
            g.always_profiler = boolean("profiler");
            // TPS and ping can't both be on: the one saved last wins.
            let tps = boolean("tps");
            if tps && g.always_ping {
                g.always_ping = false;
                g.always_tps = true;
            } else {
                g.always_tps = tps;
            }
            let ping = boolean("ping");
            if ping && g.always_tps {
                g.always_tps = false;
                g.always_ping = true;
            } else {
                g.always_ping = ping;
            }
            b::with(|cfg| cfg.general = g);
        }
        ClothKind::Edit(module) => {
            let mut m = module.borrow_mut();
            m.enabled = boolean("enabled");
            for i in 0..m.kind.extra_colors().len() {
                if let Some(e) = c.entry(&format!("extra{i}")) {
                    m.extra[i] = e.color_value() & 0xFF_FFFF;
                }
            }
            if let Some(e) = c.entry("empty_lines") {
                let n = e.current().as_text().parse::<i32>().unwrap_or(1);
                m.empty_lines = n.clamp(1, 20);
            }
            if c.entry("memory_color").is_some() {
                m.memory_color = boolean("memory_color");
            }
            if let Some(e) = c.entry("time_format") {
                m.time_format = e.current().as_text().to_owned();
            }
            if let Some(e) = c.entry("name_color") {
                m.name_color = e.color_value() & 0xFF_FFFF;
            }
            if let Some(e) = c.entry("value_color") {
                m.value_color = e.color_value() & 0xFF_FFFF;
            }
            for i in 0..m.lines.len() {
                if let Some(e) = c.entry(&format!("line{i}")) {
                    m.lines[i].enabled = e.current().as_bool();
                }
            }
            m.dirty = true;
        }
        ClothKind::Add => {
            if let Some(e) = c.entry("module") {
                let name = e.choice();
                if let Some(kind) = Kind::ALL.into_iter().find(|k| k.name() == name) {
                    with(|s| {
                        if let Some(Page::Modules(list)) = s.ui.as_mut().and_then(|u| u.stack.last_mut()) {
                            list.entries.push(Module::shared(kind));
                        }
                    });
                }
            }
        }
    }
    b::save(&data_dir(game));
}

// ---- the Cloth Config screens --------------------------------------------------------------

/// `GeneralOptionsScreen.configBuilder`.
fn general_screen(in_world: bool) -> Cloth {
    let g = b::general();
    let d = b::General::default();
    let tip = |key: &str| vec![tr(key)];
    let boolean = |id: &str, key: &str, value: bool, default: bool| {
        Entry::boolean(id, &tr(&format!("config.betterf3.{key}")), value, default)
            .tooltip(&tip(&format!("config.betterf3.{key}.tooltip")))
    };
    let entries = vec![
        boolean("disable", "disable", g.disable_mod, d.disable_mod),
        boolean("auto_start", "auto_start", g.auto_f3, d.auto_f3),
        boolean("space_modules", "space_modules", g.space_modules, d.space_modules),
        boolean("performance", "performance_optimizations", g.performance, d.performance),
        boolean("shadow_text", "shadow_text", g.shadow_text, d.shadow_text),
        boolean("animations", "animations", g.animations, d.animations),
        Entry::double("animation_speed", &tr("config.betterf3.animationSpeed"), g.animation_speed, 1.0, 1.0, 3.0)
            .tooltip(&tip("config.betterf3.animationSpeed.tooltip")),
        Entry::double("font_scale", &tr("config.betterf3.fontScale"), g.font_scale, 1.0, 0.1, 2.0)
            .tooltip(&tip("config.betterf3.fontScale.tooltip")),
        Entry::color("background", &tr("config.betterf3.color.background"), g.background_color, b::DEFAULT_BACKGROUND, true)
            .tooltip(&tip("config.betterf3.color.background.tooltip")),
        boolean("debug_crosshair", "debug_crosshair", g.hide_debug_crosshair, d.hide_debug_crosshair),
        boolean("sidebar", "sidebar", g.hide_sidebar, d.hide_sidebar),
        boolean("bossbar", "bossbar", g.hide_bossbar, d.hide_bossbar),
        boolean("profiler", "always_enable_profiler", g.always_profiler, d.always_profiler),
        boolean("tps", "always_enable_tps_graph", g.always_tps, d.always_tps),
        boolean("ping", "always_enable_ping_graph", g.always_ping, d.always_ping),
    ];
    Cloth::new(&tr("config.betterf3.title"), entries, in_world)
}

/// `EditModulesScreen.configBuilder`.
fn edit_screen(module: &Module, in_world: bool) -> Cloth {
    let tip = |key: &str| vec![tr(key)];
    let mut entries = vec![
        Entry::boolean("enabled", &tr("config.betterf3.module.enable"), module.enabled, true)
            .tooltip(&tip("config.betterf3.module.enable.tooltip")),
    ];
    let (default_name, default_value) = module.kind.colors();
    let colors: Vec<Entry> = module
        .kind
        .extra_colors()
        .iter()
        .enumerate()
        .map(|(i, e)| {
            Entry::color(&format!("extra{i}"), &tr(e.name), module.extra_color(i), e.default, false).tooltip(&tip(e.tooltip))
        })
        .collect();
    // Coordinates and FPS colours come before the separator's count, the
    // rest after it, as `EditModulesScreen` adds them.
    if matches!(module.kind, Kind::Coords | Kind::Fps) {
        entries.extend(colors);
    } else {
        if module.kind == Kind::Empty {
            entries.push(
                Entry::int("empty_lines", &tr("config.betterf3.empty_lines"), i64::from(module.empty_lines), 1)
                    .tooltip(&tip("config.betterf3.empty_lines.tooltip")),
            );
        }
        entries.extend(colors);
    }
    if module.kind == Kind::System {
        entries.push(
            Entry::boolean("memory_color", &tr("config.betterf3.memory_color_toggle"), module.memory_color, true)
                .tooltip(&tip("config.betterf3.memory_color_toggle.tooltip")),
        );
        entries.push(
            Entry::string("time_format", &tr("config.betterf3.time_format"), &module.time_format, b::DEFAULT_TIME_FORMAT)
                .tooltip(&tip("config.betterf3.time_format.tooltip")),
        );
    }
    entries.push(
        Entry::color("name_color", &tr("config.betterf3.color.name"), module.name_color, default_name, false)
            .tooltip(&tip("config.betterf3.color.name.tooltip")),
    );
    entries.push(
        Entry::color("value_color", &tr("config.betterf3.color.value"), module.value_color, default_value, false)
            .tooltip(&tip("config.betterf3.color.value.tooltip")),
    );
    if module.lines.len() > 1 {
        for (i, line) in module.lines.iter().enumerate() {
            let id = line.def.id;
            if id.is_empty() || id == "nothing" {
                continue;
            }
            let mut name = tr(&format!("text.betterf3.line.{id}"));
            if name.is_empty() {
                let words = id.replace('_', " ");
                name = words.get(..1).map_or(String::new(), |f| f.to_uppercase()) + words.get(1..).unwrap_or("");
            }
            entries.push(
                Entry::boolean(&format!("line{i}"), &name, line.enabled, true)
                    .tooltip(&tip("config.betterf3.disable_line.tooltip")),
            );
        }
    }
    Cloth::new(cloth::default_title(b::russian()), entries, in_world)
}

/// `AddModuleScreen.configBuilder`: a dropdown of every module, a separator
/// first.
fn add_screen(in_world: bool) -> Cloth {
    let names: Vec<String> = b::distinct_kinds().into_iter().map(Kind::name).collect();
    let empty = Kind::Empty.name();
    let entries = vec![Entry::dropdown("module", &tr("config.betterf3.add_button.module_name"), names, &empty, &empty)];
    Cloth::new(cloth::default_title(b::russian()), entries, in_world)
}

// ---- ModConfigScreen and ModulesScreen -----------------------------------------------------

type Rect = [f32; 4];

fn inside(p: (f32, f32), r: Rect) -> bool {
    p.0 >= r[0] && p.0 < r[0] + r[2] && p.1 >= r[1] && p.1 < r[1] + r[3]
}

/// `ModConfigScreen.init`'s buttons: General Settings, Left Modules, Right
/// Modules, Done.
fn config_buttons(w: f32, h: f32) -> [Rect; 4] {
    let (cx, q) = ((w / 2.0).floor(), (h / 4.0).floor());
    [
        [cx - 130.0, q - 24.0, 260.0, 20.0],
        [cx - 130.0, q, 120.0, 20.0],
        [cx + 10.0, q, 120.0, 20.0],
        [cx - 130.0, h - 50.0, 260.0, 20.0],
    ]
}

/// `ModulesScreen.init`'s buttons: Edit, Add, Delete, Done.
fn modules_buttons(w: f32, h: f32) -> [Rect; 4] {
    let cx = (w / 2.0).floor();
    [
        [cx - 50.0, h - 50.0, 100.0, 20.0],
        [cx + 4.0 + 50.0, h - 50.0, 100.0, 20.0],
        [cx - 154.0, h - 50.0, 100.0, 20.0],
        [cx - 154.0, h - 30.0 + 4.0, 308.0, 20.0],
    ]
}

/// The list: 32 from the top to 64 from the bottom, rows 220 wide and 36
/// high (`ObjectSelectionList`).
const ROW_W: f32 = 220.0;
const ROW_H: f32 = 36.0;

fn list_area(w: f32, h: f32) -> (f32, f32, f32) {
    (32.0, h - 64.0, (w / 2.0).floor() - (ROW_W / 2.0).floor())
}

fn content_height(n: usize) -> f32 {
    n as f32 * ROW_H + 4.0
}

fn background(game: &Game<'_>, batch: &mut HudBatch, in_world: bool) {
    let (w, h) = (batch.width.floor(), batch.height.floor());
    game.native_apply_screen_blur();
    ui::menu_background(batch, &game.gui.atlas, 0.0, 0.0, w, h, in_world);
}

fn button(game: &Game<'_>, batch: &mut HudBatch, r: Rect, label: &str, active: bool) {
    let hover = active && inside(game.cursor, r);
    ui::button(batch, &game.gui.atlas, r[0], r[1], r[2], r[3], &Text::literal(label), active, hover);
}

fn title(game: &Game<'_>, batch: &mut HudBatch, text: &str) {
    let w = batch.width.floor();
    let tw = HudBatch::text_width(&game.gui.atlas, text);
    batch.text(&game.gui.atlas, text, ((w - tw) / 2.0).floor(), 20.0, u32::MAX, true);
}

fn draw_config(game: &Game<'_>, batch: &mut HudBatch, in_world: bool) {
    background(game, batch, in_world);
    let (w, h) = (batch.width.floor(), batch.height.floor());
    let [general, left, right, done] = config_buttons(w, h);
    button(game, batch, left, &tr("config.betterf3.order_left_button"), true);
    button(game, batch, right, &tr("config.betterf3.order_right_button"), true);
    button(game, batch, general, &tr("config.betterf3.general_settings"), true);
    button(game, batch, done, &tr("config.betterf3.modules.done_button"), true);
    title(game, batch, &tr("config.betterf3.title.config"));
}

/// `ModuleEntry.extractContent`'s sample line.
fn sample(m: &Module) -> Text {
    let s = |t: &str, c: u32| b::styled(t, c);
    match m.kind {
        Kind::Coords => {
            let (x, y, z) = (m.extra_color(0), m.extra_color(1), m.extra_color(2));
            [s("X", x), s("Y", y), s("Z", z), s(": ", m.name_color), s("100 ", x), s("200 ", y), s("300", z)]
                .into_iter()
                .fold(Text::default(), b::append)
        }
        Kind::Fps => [s("60 fps  ", m.extra_color(0)), s("40 fps  ", m.extra_color(1)), s("10 fps", m.extra_color(2))]
            .into_iter()
            .fold(Text::default(), b::append),
        _ => b::append(s("Name: ", m.name_color), s("Value", m.value_color)),
    }
}

fn draw_modules(game: &Game<'_>, batch: &mut HudBatch, list: &mut Modules, in_world: bool, size: [u32; 2]) {
    // The debug overlay shows behind the modules (`DebugRenderer.newText`).
    if in_world && overlay::active(game) && overlay::visible(game) {
        overlay::paint_lines(game, batch, size);
    }
    background(game, batch, in_world);
    let atlas = &game.gui.atlas;
    let (w, h) = (batch.width.floor(), batch.height.floor());
    let (top, bottom, left) = list_area(w, h);
    let content = content_height(list.entries.len());
    list.scroll.clamp(top, bottom, content);
    ui::list_background(batch, atlas, top, bottom, in_world);
    let cursor = game.cursor;
    let first = batch.vertices.len();
    let over_list = cursor.1 >= top && cursor.1 < bottom && cursor.0 >= left && cursor.0 < left + ROW_W;
    let n = list.entries.len();
    for (i, m) in list.entries.iter().enumerate() {
        let y = top + 2.0 - list.scroll.amount.trunc() + i as f32 * ROW_H;
        if y + ROW_H < top || y > bottom {
            continue;
        }
        if list.selected == Some(i) {
            let border = if list.focused { u32::MAX } else { horizon_client::hud::rgba(128, 128, 128, 255) };
            batch.fill(atlas, left, y, ROW_W, ROW_H, border);
            batch.fill(atlas, left + 1.0, y + 1.0, ROW_W - 2.0, ROW_H - 2.0, horizon_client::hud::rgba(0, 0, 0, 255));
        }
        let (cx, cy) = (left + 2.0, y + 2.0);
        let Ok(m) = m.try_borrow() else {
            continue;
        };
        batch.text(atlas, &m.kind.name(), cx + 35.0, cy + 1.0, u32::MAX, true);
        batch.text_styled(atlas, &sample(&m), cx + 43.0, cy + 13.0, u32::MAX, true);
        let hovered = over_list && cursor.1 >= y && cursor.1 < y + ROW_H;
        if hovered {
            batch.fill(atlas, cx, cy, 32.0, 32.0, 0xA090_9090);
            let (v, w2) = (cursor.0 - cx, cursor.1 - cy);
            for (sprite, shown, lit) in
                [("move_up", i > 0, v < 16.0 && w2 < 16.0), ("move_down", i + 1 < n, v < 16.0 && w2 > 16.0)]
            {
                if shown {
                    let path = format!("gui/sprites/server_list/{sprite}{}", if lit { "_highlighted" } else { "" });
                    batch.sprite_scaled(atlas, &path, cx, cy, 32.0, 32.0, u32::MAX);
                }
            }
        }
    }
    batch.clip_since(first, 0.0, top, w, bottom);
    ui::separators(batch, atlas, top, bottom, in_world);
    ui::scrollbar(batch, atlas, left + ROW_W + 8.0, top, bottom, content, list.scroll.amount);
    let [edit, add, delete, done] = modules_buttons(w, h);
    let chosen = list.selected.is_some();
    button(game, batch, edit, &tr("config.betterf3.modules.edit_button"), chosen);
    button(game, batch, add, &tr("config.betterf3.modules.add_button"), true);
    button(game, batch, delete, &tr("config.betterf3.modules.delete_button"), chosen);
    button(game, batch, done, &tr("config.betterf3.modules.done_button"), true);
    title(game, batch, &tr("config.betterf3.title.modules"));
}

// ---- the screen hooks ----------------------------------------------------------------------

/// The open screen, drawn over the options (or as the mod's own screen).
pub(super) fn render(game: &Game<'_>, batch: &mut HudBatch, screen: &NativeScreenView<'_>) {
    if !over(game, screen) {
        return;
    }
    let size = game.renderer.as_ref().map_or([1280, 720], |r| [r.config.width, r.config.height]);
    with(|s| {
        let Some(u) = s.ui.as_mut() else {
            return;
        };
        let in_world = u.in_world;
        match u.stack.last_mut() {
            Some(Page::Config) => draw_config(game, batch, in_world),
            Some(Page::Modules(list)) => draw_modules(game, batch, list, in_world, size),
            Some(Page::Cloth(c, _)) => c.draw(game, batch),
            None => {}
        }
    });
}

/// `ModulesScreen.onClose`: the list becomes the column, then the config is saved.
fn close_modules(game: &Game<'_>, u: &mut Ui) {
    if let Some(Page::Modules(list)) = u.stack.pop() {
        b::with(|c| {
            if list.left {
                c.left = list.entries;
            } else {
                c.right = list.entries;
            }
        });
        b::save(&data_dir(game));
    }
    u.init = true;
}

pub(super) fn pressed(game: &Game<'_>, screen: &NativeScreenView<'_>, button: i32, modifiers: u32) -> bool {
    if !over(game, screen) {
        return false;
    }
    let (w, h) = (screen.rect[2].floor(), screen.rect[3].floor());
    let p = (screen.mouse[0], screen.mouse[1]);
    with(|s| {
        let State { ui, actions, .. } = s;
        let Some(u) = ui.as_mut() else {
            return;
        };
        let in_world = u.in_world;
        match u.stack.last_mut() {
            Some(Page::Config) => {
                if button != 0 {
                    return;
                }
                let [general, left, right, done] = config_buttons(w, h);
                if inside(p, general) {
                    actions.push(Action::Click);
                    u.stack.push(Page::Cloth(Box::new(general_screen(in_world)), ClothKind::General));
                } else if inside(p, left) || inside(p, right) {
                    actions.push(Action::Click);
                    let is_left = inside(p, left);
                    let entries = b::with(|c| if is_left { c.left.clone() } else { c.right.clone() }).unwrap_or_default();
                    u.stack.push(Page::Modules(Modules {
                        left: is_left,
                        entries,
                        selected: None,
                        scroll: ui::Scroll::default(),
                        focused: false,
                    }));
                } else if inside(p, done) {
                    actions.push(Action::Click);
                    actions.push(Action::Leave { all: false });
                }
            }
            Some(Page::Modules(list)) => {
                let (top, bottom, left) = list_area(w, h);
                let content = content_height(list.entries.len());
                let [edit, add, delete, done] = modules_buttons(w, h);
                let chosen = list.selected.is_some();
                if button == 0 {
                    if inside(p, edit) && chosen {
                        actions.push(Action::Click);
                        list.focused = false;
                        let module = list.selected.and_then(|i| list.entries.get(i).cloned());
                        if let Some(m) = module {
                            let screen = edit_screen(&m.borrow(), in_world);
                            u.stack.push(Page::Cloth(Box::new(screen), ClothKind::Edit(m)));
                        }
                        return;
                    }
                    if inside(p, add) {
                        actions.push(Action::Click);
                        list.focused = false;
                        u.stack.push(Page::Cloth(Box::new(add_screen(in_world)), ClothKind::Add));
                        return;
                    }
                    if inside(p, delete) && chosen {
                        actions.push(Action::Click);
                        list.focused = false;
                        if let Some(i) = list.selected.take()
                            && i < list.entries.len()
                        {
                            list.entries.remove(i);
                        }
                        list.scroll.clamp(top, bottom, content_height(list.entries.len()));
                        return;
                    }
                    if inside(p, done) {
                        actions.push(Action::Click);
                        close_modules(game, u);
                        return;
                    }
                }
                if list.scroll.press(p, left + ROW_W + 8.0, top, bottom, content) {
                    return;
                }
                if p.1 < top || p.1 >= bottom || p.0 < left || p.0 >= left + ROW_W {
                    return;
                }
                let index = ((p.1 - top - 2.0 + list.scroll.amount.trunc()) / ROW_H).floor();
                if index < 0.0 || index as usize >= list.entries.len() {
                    return;
                }
                let i = index as usize;
                let row_top = top + 2.0 - list.scroll.amount.trunc() + i as f32 * ROW_H;
                // `ModuleEntry.mouseClicked`: the arrows swap it with its
                // neighbour, anywhere else selects it.
                let (d, e) = (p.0 - left, p.1 - row_top);
                if d <= 32.0 && d < 16.0 {
                    if e < 16.0 && i > 0 {
                        list.entries.swap(i, i - 1);
                        list.selected = Some(i - 1);
                        list.focused = true;
                        return;
                    }
                    if e > 16.0 && i + 1 < list.entries.len() {
                        list.entries.swap(i, i + 1);
                        list.selected = Some(i + 1);
                        list.focused = true;
                        return;
                    }
                }
                list.selected = Some(i);
            }
            Some(Page::Cloth(c, _)) => c.press(game, w, h, p, button, modifiers),
            None => {}
        }
    });
    true
}

pub(super) fn released(game: &Game<'_>, screen: &NativeScreenView<'_>) -> bool {
    if !over(game, screen) {
        return false;
    }
    with(|s| match s.ui.as_mut().and_then(|u| u.stack.last_mut()) {
        Some(Page::Modules(list)) => list.scroll.release(),
        Some(Page::Cloth(c, _)) => c.release(),
        _ => {}
    });
    true
}

pub(super) fn dragged(game: &Game<'_>, screen: &NativeScreenView<'_>) -> bool {
    if !over(game, screen) {
        return false;
    }
    let (w, h) = (screen.rect[2].floor(), screen.rect[3].floor());
    let p = (screen.mouse[0], screen.mouse[1]);
    with(|s| match s.ui.as_mut().and_then(|u| u.stack.last_mut()) {
        Some(Page::Modules(list)) => {
            let (top, bottom, _) = list_area(w, h);
            list.scroll.drag(p.1, top, bottom, content_height(list.entries.len()));
        }
        Some(Page::Cloth(c, _)) => c.dragged(w, h, p),
        _ => {}
    });
    true
}

pub(super) fn scrolled(game: &Game<'_>, screen: &NativeScreenView<'_>, scroll: [f64; 2]) -> bool {
    if !over(game, screen) {
        return false;
    }
    let (w, h) = (screen.rect[2].floor(), screen.rect[3].floor());
    let p = (screen.mouse[0], screen.mouse[1]);
    with(|s| match s.ui.as_mut().and_then(|u| u.stack.last_mut()) {
        Some(Page::Modules(list)) => {
            let (top, bottom, _) = list_area(w, h);
            list.scroll.wheel(scroll[1] as f32, ROW_H / 2.0, top, bottom, content_height(list.entries.len()));
        }
        Some(Page::Cloth(c, _)) => c.scrolled(w, h, p, scroll[1] as f32),
        _ => {}
    });
    true
}

pub(super) fn key_pressed(game: &Game<'_>, screen: &NativeScreenView<'_>, key: &str, modifiers: u32) -> bool {
    if !over(game, screen) {
        return false;
    }
    let escape = key == "key.keyboard.escape";
    with(|s| {
        let State { ui, actions, .. } = s;
        let Some(u) = ui.as_mut() else {
            return;
        };
        match u.stack.last_mut() {
            Some(Page::Config) => {
                if escape {
                    actions.push(Action::Leave { all: true });
                }
            }
            Some(Page::Modules(list)) => {
                if escape {
                    close_modules(game, u);
                } else if list.focused && !list.entries.is_empty() {
                    // `AbstractSelectionList.keyPressed`: the arrows move the selection.
                    let last = list.entries.len() - 1;
                    match key {
                        "key.keyboard.up" => list.selected = Some(list.selected.map_or(0, |i| i.saturating_sub(1))),
                        "key.keyboard.down" => list.selected = Some(list.selected.map_or(0, |i| (i + 1).min(last))),
                        _ => {}
                    }
                }
            }
            Some(Page::Cloth(c, _)) => c.key_pressed(key, modifiers),
            None => {}
        }
    });
    true
}

pub(super) fn key_released(game: &Game<'_>, screen: &NativeScreenView<'_>) -> bool {
    over(game, screen)
}

pub(super) fn char_typed(game: &Game<'_>, screen: &NativeScreenView<'_>, c: char) -> bool {
    if !over(game, screen) {
        return false;
    }
    with(|s| {
        if let Some(Page::Cloth(cl, _)) = s.ui.as_mut().and_then(|u| u.stack.last_mut()) {
            cl.typed(&c.to_string());
        }
    });
    true
}


/// The mod's row in Options > Mods (BetterF3 binds no keys of its own: F3+M
/// and F3+F are fixed).
pub(super) fn list(mods: &mut Vec<super::native_hooks::NativeMod>) {
    let description = if horizon_client::lang::code().starts_with("ru") {
        "Заменяет экран отладки (F3) настраиваемым и более понятным."
    } else {
        "Replaces Minecraft's original debug HUD with a highly customizable, more human-readable HUD."
    };
    mods.push(super::native_hooks::NativeMod {
        id: b::ID.into(),
        title: format!("BetterF3 {}", b::VERSION),
        description: description.into(),
        keys: Vec::new(),
    });
}
