// Zoomify's edges in the game (not in the Cargo library): its keys, the
// wheel, the field of view, the mouse, the hand, the spyglass and the HUD,
// each where the original hooks the Java game.
use super::zoomify as zf;
use super::Game;
use horizon_client::camera::{FovPlayer, UseItem};
use horizon_client::hud::HudBatch;
use horizon_client::items::ItemStack;
use horizon_client::mod_api::NativeMod;
use horizon_client::mouse::LookOptions;
use std::sync::Mutex;
use std::time::Duration;

/// The keys between frames: the zoom key held, the presses not yet acted on,
/// and `/zoomify` asking for the settings.
struct Keys {
    down: bool,
    presses: u32,
    secondary: u32,
    zoom_in: i32,
    zoom_out: i32,
    open_settings: bool,
}

static KEYS: Mutex<Keys> = Mutex::new(Keys {
    down: false,
    presses: 0,
    secondary: 0,
    zoom_in: 0,
    zoom_out: 0,
    open_settings: false,
});

fn keys() -> std::sync::MutexGuard<'static, Keys> {
    KEYS.lock().unwrap_or_else(|e| e.into_inner())
}

fn settings(game: &Game<'_>) -> zf::Settings {
    zf::settings(|key| game.native_setting(zf::ID, key))
}

fn russian(game: &Game<'_>) -> bool {
    zf::russian(&game.settings.language)
}

/// The zoom's divisor now, 1 with the mod off.
fn divisor(game: &Game<'_>) -> f64 {
    if !game.native_on(zf::ID) {
        return 1.0;
    }
    let s = settings(game);
    zf::with_zoom(&s, |z| z.divisor)
}

fn is_spyglass(stack: &Option<ItemStack>) -> bool {
    stack.as_ref().is_some_and(|s| &*s.item == "minecraft:spyglass")
}

/// The local player's spyglass: in a hand, anywhere on them. None while the
/// camera is another entity (the original checks the camera entity).
fn spyglass_state(game: &Game<'_>) -> Option<zf::SpyglassState> {
    if game.player.is_none() || game.camera_mob().is_some() {
        return None;
    }
    let main = game.inventory.get(game.selected).is_some_and(is_spyglass);
    let holding = main || is_spyglass(&game.offhand);
    let carrying = holding || game.inventory.iter().any(is_spyglass);
    Some(zf::SpyglassState {
        holding,
        carrying,
        scoping: game.scoping(),
        first_person: game.camera_type.first_person(),
    })
}

/// `/zoomify`: the config screen over Options > Mods > Zoomify.
fn open_settings(game: &mut Game<'_>) {
    let Some(i) = game
        .settings
        .mods
        .installed
        .iter()
        .position(|m| m.id == zf::ID)
    else {
        return;
    };
    game.open_options_at(false, horizon_client::options_ui::Page::NativeMod(i));
    game.native_settings_refresh(i);
    // As the original's command, the config screen itself; closing it goes
    // back to the game.
    super::zoomify_screen::open(game, true);
}

// ---- the hooks -------------------------------------------------------------

pub(super) fn list(game: &Game<'_>, mods: &mut Vec<NativeMod>) {
    let ru = russian(game);
    mods.push(NativeMod {
        id: zf::ID.into(),
        title: format!("{} {}", zf::NAME, zf::VERSION),
        description: zf::description(ru).into(),
        keys: zf::KEYS
            .iter()
            .map(|(name, default)| [name.to_string(), zf::key_label(name, ru).to_string(), default.to_string()])
            .collect(),
    });
}

/// The settings page: first the config screen's button (Mod Menu's
/// "Configure..."), then the settings and the presets.
pub(super) fn page(game: &Game<'_>, mod_id: &str, labels: &mut Vec<String>) {
    if mod_id == zf::ID {
        labels.push(super::zoomify_screen::configure_label().to_string());
        labels.extend(settings(game).labels(russian(game)));
    }
}

pub(super) fn page_click(game: &Game<'_>, mod_id: &str, button: usize) {
    if mod_id != zf::ID {
        return;
    }
    // Mod Menu's Configure, or the page's first button: the config screen.
    if button == super::zoomify_screen::MODMENU_CONFIGURE || button == 0 {
        super::zoomify_screen::open(game, button != 0);
        return;
    }
    let button = button - 1;
    if button >= zf::ROWS + zf::PRESETS {
        return;
    }
    let mut s = settings(game);
    s.click(button);
    zf::set_settings(s);
    for (key, value) in s.stored() {
        game.native_set_setting(zf::ID, key, &value);
    }
}

/// A key went down or up (any screen): the zoom key's state and the presses
/// the game counts while playing, as a `KeyMapping` does.
pub(super) fn key(game: &Game<'_>, key: &str, pressed: bool) {
    if !game.native_on(zf::ID) || key == zf::UNBOUND {
        return;
    }
    let bound = |name: &str, default: &str| game.native_mod_key(zf::ID, name, default) == key;
    let playing = game.native_playing();
    let [zoom, secondary, zoom_in, zoom_out] = zf::KEYS.map(|(name, default)| bound(name, default));
    let mut k = keys();
    if zoom {
        if !pressed {
            k.down = false;
        } else if playing {
            k.down = true;
            k.presses += 1;
        }
    }
    if pressed && playing {
        k.secondary += u32::from(secondary);
        k.zoom_in += i32::from(zoom_in);
        k.zoom_out += i32::from(zoom_out);
    }
}

/// Each frame: the original's tick (keys, spyglass, sounds), then its zoom
/// divisor; `/zoomify`'s settings once the chat has closed.
pub(super) fn frame(game: &mut Game<'_>, elapsed: Duration) {
    super::zoomify_screen::frame(game);
    let s = settings(game);
    if !game.native_on(zf::ID) {
        zf::with_zoom(&s, |z| *z = zf::Zoom::new(&s));
        return;
    }
    let playing = game.native_playing() && game.player.is_some();
    let spyglass = spyglass_state(game);
    let (input, open) = {
        let mut k = keys();
        if !playing {
            // `KeyMapping.releaseAll` as a screen opens.
            k.down = false;
        }
        let input = zf::Input {
            key_down: k.down && playing,
            presses: std::mem::take(&mut k.presses),
            secondary_presses: std::mem::take(&mut k.secondary),
            zoom_in_presses: std::mem::take(&mut k.zoom_in),
            zoom_out_presses: std::mem::take(&mut k.zoom_out),
            spyglass,
        };
        let open = playing && std::mem::take(&mut k.open_settings);
        (input, open)
    };
    let sounds = zf::with_zoom(&s, |z| z.frame(&s, input, elapsed.as_secs_f64()));
    for sound in sounds {
        // `Player.playSound` as `SpyglassItem` plays them.
        match sound {
            zf::Sound::Use => {
                let at = game
                    .player
                    .as_ref()
                    .map(|p| [p.position.x, p.position.y, p.position.z]);
                game.audio.play("minecraft:item.spyglass.use", at, 1.0, 1.0);
            }
            zf::Sound::Stop => {
                let at = game.eye_position();
                game.audio.play("minecraft:item.spyglass.stop_using", at, 1.0, 1.0);
            }
        }
    }
    if open {
        open_settings(game);
    }
}

/// The wheel while playing: while zooming it zooms, and the hotbar stays.
pub(super) fn scrolling(game: &Game<'_>, accumulated: [i32; 2], canceled: &mut bool) {
    if !game.native_on(zf::ID) {
        return;
    }
    let s = settings(game);
    if !s.scroll_zoom || s.keybind_scrolling {
        return;
    }
    let delta = accumulated[0] + accumulated[1];
    if delta == 0 {
        return;
    }
    let took = zf::with_zoom(&s, |z| {
        if z.zooming {
            z.mouse_zoom(&s, delta);
        }
        z.zooming
    });
    if took {
        *canceled = true;
    }
}

/// The narrowest field of view drawn: the original divides by up to 500
/// with no floor (`CameraMixin`), a hundredth of a degree keeps the
/// projection sound.
const MIN_FOV: f32 = 0.01;

/// The world's field of view (`Camera.calculateFov`'s result), divided.
pub(super) fn fov(game: &Game<'_>, value: f32) -> f32 {
    let d = divisor(game);
    if d > 1.0 && value.is_finite() && value > 0.0 {
        (value / d as f32).max(MIN_FOV)
    } else {
        value
    }
}

/// The hand's field of view (`Camera.calculateHudFov`), divided too unless
/// the setting is off.
pub(super) fn hand_fov(game: &Game<'_>, value: f32) -> f32 {
    if !game.native_on(zf::ID) || !settings(game).affect_hand_fov {
        return value;
    }
    let d = divisor(game);
    if d > 1.0 && value.is_finite() && value > 0.0 {
        (value / d as f32).max(MIN_FOV)
    } else {
        value
    }
}

/// The spyglass's own zoom (`getFieldOfViewModifier`'s 0.1) is 1 when
/// Zoomify zooms for it.
pub(super) fn fov_modifier(game: &Game<'_>, player: &FovPlayer, modifier: &mut f32) {
    if !game.native_on(zf::ID) || settings(game).spyglass == zf::Spyglass::Combine {
        return;
    }
    if game.camera_type.first_person() && matches!(player.using, Some((UseItem::Spyglass, _))) {
        *modifier = 1.0;
    }
}

/// What the mouse reads (`MouseHandler.turnPlayer`): the sensitivity over
/// the zoom, the cinematic camera while zooming, and no spyglass slowdown
/// when Zoomify zooms for the spyglass.
pub(super) fn look(game: &Game<'_>, mut look: LookOptions) -> LookOptions {
    if !game.native_on(zf::ID) {
        return look;
    }
    let s = settings(game);
    zf::with_zoom(&s, |z| {
        look.sensitivity = z.sensitivity(&s, look.sensitivity);
        look.smooth_camera = z.smooth_camera(&s, look.smooth_camera);
    });
    if s.spyglass != zf::Spyglass::Combine {
        look.scoping = false;
    }
    look
}

/// The HUD while the secondary zoom is on: hidden, as with F1.
pub(super) fn gui_layer(game: &Game<'_>, canceled: &mut bool) {
    if !game.native_on(zf::ID) {
        return;
    }
    let s = settings(game);
    if s.secondary_hide_hud && zf::with_zoom(&s, |z| z.secondary) {
        *canceled = true;
    }
}

pub(super) fn commands(commands: &mut Vec<&'static str>) {
    commands.push(zf::ID);
}

pub(super) fn command(command: &str, ran: &mut bool) {
    if command.split_whitespace().next() == Some(zf::ID) {
        keys().open_settings = true;
        *ran = true;
    }
}

/// The spyglass overlay (`Hud.extractCameraOverlays`' `isScoping`), shown
/// when Zoomify says: true when this drew it or hid it.
pub(super) fn spyglass(game: &mut Game<'_>, batch: &mut HudBatch) -> bool {
    if !game.native_on(zf::ID) || !game.camera_type.first_person() {
        return false;
    }
    let Some(spy) = spyglass_state(game) else {
        return false;
    };
    let s = settings(game);
    let want = zf::with_zoom(&s, |z| z.overlay(&s, spy.holding, spy.carrying, spy.scoping));
    if want == spy.scoping {
        return false;
    }
    // As the game's own: the scope's size eases toward 1.125, back to 0.5
    // while it is not shown.
    game.scope_scale += 0.5 * game.frame_ticks * (1.125 - game.scope_scale);
    if !want {
        game.scope_scale = 0.5;
        return true;
    }
    let draws =
        horizon_client::hud_overlays::spyglass(batch.width as i32, batch.height as i32, game.scope_scale);
    horizon_client::hud_layout::paint(batch, &game.gui.atlas, &draws);
    true
}
