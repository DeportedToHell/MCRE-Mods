// Mod Menu for Horizon: the title screen's side. The title lives in the
// client library, which can't reach the game's modules, so the game writes
// what the button needs here once a frame (`modmenu_glue::frame`) and reads
// the presses back.
use super::{HudAtlas, HudBatch};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Mutex, OnceLock};

/// The game is up: the startup screen shows the same title before it is, from
/// what the game kept last time (`load_startup`).
pub static READY: AtomicBool = AtomicBool::new(false);
/// The mods screen is over the title: the title isn't drawn.
pub static COVER: AtomicBool = AtomicBool::new(false);
/// The Mods button was pressed; the game opens the screen at its next frame.
pub static OPEN_REQUEST: AtomicBool = AtomicBool::new(false);
/// `mods_button_style`: 0 Below Realms, 1 Replace Realms, 2 Adjacent, 3 Icon;
/// `NONE` when `modify_title_screen` is off.
pub static STYLE: AtomicU8 = AtomicU8::new(3);
pub const NONE: u8 = 255;
/// The Mods button's id among the title's buttons.
pub const BUTTON_ID: u8 = 200;
/// Its text for the text styles ("Mods", "Mods (25 Loaded)").
pub static LABEL: Mutex<String> = Mutex::new(String::new());
/// What follows the title's version (" (25 Mods)"), or nothing.
pub static COUNT: Mutex<String> = Mutex::new(String::new());

/// `textures/gui/sprites/mods/enabled.png` (Mod Menu, MIT), 20x20: runs of
/// pixels (x, y, length, colour as the game packs it).
const MODS_ICON: &[(u8, u8, u8, u32)] = &[(4,4,10,0xffffffff),(15,4,1,0xffffffff),(5,5,10,0xff3f3f3f),(16,5,1,0xff3f3f3f),(4,6,2,0xffffffff),(7,6,9,0xffffffff),(4,7,2,0xffffffff),(6,7,1,0xff3f3f3f),(7,7,9,0xffffffff),(16,7,1,0xff3f3f3f),(5,8,2,0xff3f3f3f),(8,8,9,0xff3f3f3f),(4,9,2,0xffffffff),(7,9,9,0xffffffff),(4,10,2,0xffffffff),(6,10,1,0xff3f3f3f),(7,10,9,0xffffffff),(16,10,1,0xff3f3f3f),(5,11,2,0xff3f3f3f),(8,11,9,0xff3f3f3f),(4,12,2,0xffffffff),(7,12,9,0xffffffff),(4,13,2,0xffffffff),(6,13,1,0xff3f3f3f),(7,13,9,0xffffffff),(16,13,1,0xff3f3f3f),(5,14,2,0xff3f3f3f),(8,14,9,0xff3f3f3f),(4,15,5,0xffffffff),(11,15,5,0xffffffff),(5,16,5,0xff3f3f3f),(12,16,5,0xff3f3f3f)];

/// The Mods icon, 20 px wide.
pub fn draw_icon(batch: &mut HudBatch, atlas: &HudAtlas, x: f32, y: f32) {
    for &(px, py, len, c) in MODS_ICON {
        batch.fill(atlas, x + f32::from(px), y + f32::from(py), f32::from(len), 1.0, c);
    }
}

/// Where the game keeps what the title last showed (the style, the button's
/// text and the count, a line each) for the startup screen, which shows the
/// title before the game is up: Mod Menu's data folder.
pub fn startup_path() -> std::path::PathBuf {
    let dir = std::env::current_dir().unwrap_or_default().join(crate::mod_list::DIR);
    dir.join(".horizon-mod-data").join("modmenu").join("startup.txt")
}

/// Before the game is up: what the title showed last time.
fn load_startup() {
    static LOADED: OnceLock<()> = OnceLock::new();
    LOADED.get_or_init(|| {
        let Ok(text) = std::fs::read_to_string(startup_path()) else {
            return;
        };
        let mut lines = text.lines();
        if let Some(style) = lines.next().and_then(|l| l.trim().parse::<u8>().ok()) {
            STYLE.store(style, Ordering::Relaxed);
        }
        for slot in [&LABEL, &COUNT] {
            if let (Some(line), Ok(mut slot)) = (lines.next(), slot.lock())
                && slot.is_empty()
            {
                *slot = line.to_owned();
            }
        }
    });
}

/// A title button drawn, the startup screen's too: on the Mods button, its
/// icon (`SmallModMenuButtonWidget`) and the mod count after the version
/// (`MixinTitleScreen`).
pub(crate) fn drawn(b: &super::Button, batch: &mut HudBatch, atlas: &HudAtlas) {
    let (w, h) = (batch.width.ceil(), batch.height.ceil());
    if b.id != BUTTON_ID || button_rect(w, h) != Some((b.x, b.y, b.w)) {
        return;
    }
    if b.label.is_empty() {
        draw_icon(batch, atlas, b.x, b.y);
    }
    let count = COUNT.lock().map(|c| c.clone()).unwrap_or_default();
    if !count.is_empty() {
        let version = format!("Horizon {} (Rust)", horizon_core::VANILLA_VERSION);
        let x = (2.0 + HudBatch::text_width(atlas, &version)).floor();
        batch.text(atlas, &count, x, h - 10.0, u32::MAX, true);
    }
}

/// The Mods button's rectangle on the main title screen (x, y, w), for the
/// game's drawing of its icon and tooltip.
pub fn button_rect(w: f32, h: f32) -> Option<(f32, f32, f32)> {
    let style = STYLE.load(Ordering::Relaxed);
    let (cx, y0) = ((w.ceil() / 2.0).floor(), (h.ceil() / 4.0).floor() + 48.0);
    match style {
        0 => Some((cx - 100.0, y0 + 48.0 - 12.0 + 24.0, 200.0)),
        1 => Some((cx - 100.0, y0 + 48.0, 200.0)),
        2 => Some((cx + 2.0, y0 + 48.0, 98.0)),
        3 => Some((cx - 46.0 + 3.0 * 24.0, y0 + 72.0, 20.0)),
        _ => None,
    }
}

/// The main title screen's buttons with the Mods button
/// (`MixinTitleScreen`, `ModMenuEventHandler.afterTitleScreenInit`); None on
/// the title's other screens.
pub(crate) fn buttons(t: &super::Title, w: f32, h: f32) -> Option<Vec<super::Button>> {
    if !matches!(t.mode, super::Mode::Main) || t.rules.borrow().is_some() {
        return None;
    }
    if !READY.load(Ordering::Relaxed) {
        load_startup();
    }
    let style = STYLE.load(Ordering::Relaxed);
    if style == NONE {
        return None;
    }
    let (w, h) = (w.ceil(), h.ceil());
    let b = |x: f32, y: f32, w: f32, label: &str, enabled: bool, id: u8| super::Button {
        x,
        y,
        w,
        label: label.into(),
        enabled,
        id,
        icon: None,
        trailing: None,
        action: None,
    };
    let lang = |key: &str, plain: &str| crate::lang::get_or(key, plain);
    let cx = (w / 2.0).floor();
    let y = (h / 4.0).floor() + 48.0;
    // Pressed before the game is up, it opens the screen once the game is.
    let ready = true;
    let label = LABEL.lock().map(|l| l.clone()).unwrap_or_default();
    let label = if !label.is_empty() {
        label
    } else if crate::lang::code().starts_with("ru") {
        "Моды".to_owned()
    } else {
        "Mods".to_owned()
    };
    // Below Realms: what is up to Realms moves up 12, the rest down 12.
    let (up, down) = if style == 0 { (-12.0, 12.0) } else { (0.0, 0.0) };
    let mut out = vec![
        b(cx - 100.0, y + up, 200.0, &lang("menu.singleplayer", "Singleplayer"), true, 1),
        b(cx - 100.0, y + 24.0 + up, 200.0, &lang("menu.multiplayer", "Multiplayer"), t.allows_multiplayer, 4),
    ];
    match style {
        1 => out.push(b(cx - 100.0, y + 48.0, 200.0, &label, ready, BUTTON_ID)),
        2 => {
            out.push(b(cx - 100.0, y + 48.0, 98.0, &lang("menu.online", "Minecraft Realms"), t.allows_multiplayer, 8));
            out.push(b(cx + 2.0, y + 48.0, 98.0, &label, ready, BUTTON_ID));
        }
        0 => {
            out.push(b(cx - 100.0, y + 48.0 + up, 200.0, &lang("menu.online", "Minecraft Realms"), t.allows_multiplayer, 8));
            out.push(b(cx - 100.0, y + 48.0 + up + 24.0, 200.0, &label, ready, BUTTON_ID));
        }
        _ => out.push(b(cx - 100.0, y + 48.0, 200.0, &lang("menu.online", "Minecraft Realms"), t.allows_multiplayer, 8)),
    }
    out.push(b(cx - 100.0, y + 96.0 + down, 98.0, &lang("menu.options", "Options..."), true, 3).does(crate::widgets::Action::Options));
    out.push(b(cx + 2.0, y + 96.0 + down, 98.0, &lang("menu.quit", "Quit Game"), true, 2).does(crate::widgets::Action::Exit));
    // The icon row (`TitleScreen.getHorizontalPosition`): four icons when the
    // Mods icon joins it, each of the three moving 12 px left.
    let icons = if style == 3 { 4.0 } else { 3.0 };
    let left = cx - (icons * 20.0 + (icons - 1.0) * 4.0) / 2.0;
    for (i, (key, plain, id, icon)) in [
        ("gui.friends.open", "Friends", 5, "friends/friends"),
        ("options.language", "Language...", 6, "icon/language"),
        ("options.accessibility", "Accessibility Settings...", 7, "icon/accessibility"),
    ]
    .into_iter()
    .enumerate()
    {
        let mut button = b(left + i as f32 * 24.0, y + 72.0 + down, 20.0, &lang(key, plain), true, id);
        button.icon = Some((icon, 15.0));
        out.push(button);
    }
    if style == 3 {
        // `SmallModMenuButtonWidget`: no text; the game draws its icon.
        out.push(b(left + 3.0 * 24.0, y + 72.0, 20.0, "", ready, BUTTON_ID));
    }
    Some(out)
}

/// A click on the main title screen: true when it was on the Mods button.
pub(crate) fn clicked(t: &mut super::Title, x: f32, y: f32, size: (f32, f32)) -> bool {
    if !matches!(t.mode, super::Mode::Main) || t.rules.get_mut().is_some() {
        return false;
    }
    let Some((bx, by, bw)) = button_rect(size.0, size.1) else {
        return false;
    };
    if !(x >= bx && x < bx + bw && y >= by && y < by + 20.0) {
        return false;
    }
    open(t)
}

/// Enter or Space on the focused Mods button.
pub(crate) fn confirmed(t: &mut super::Title) -> bool {
    if !matches!(t.mode, super::Mode::Main) || t.focused_button != Some(BUTTON_ID) {
        return false;
    }
    open(t)
}

fn open(t: &mut super::Title) -> bool {
    t.focused_button = None;
    OPEN_REQUEST.store(true, Ordering::Relaxed);
    true
}
