// Smooth Swapping's edges in the game (not in the Cargo library): the open
// inventory screen's slots compared frame by frame, as the original compares
// the menu's stacks in `HandledScreen.render`; the destination slots drawn
// with what has already landed; and the flying stacks drawn over the slots,
// under the held stack and the tooltips.
use super::smoothswapping as ss;
use super::{Game, Screen};
use horizon_client::containers::BlockEntity;
use horizon_client::hud::HudBatch;
use horizon_client::items::ItemStack;
use horizon_client::mod_api::NativeMod;
use std::sync::Mutex;
use std::time::Duration;

/// A stack on its way, its corners relative to the screen's top left (the
/// recipe book may move the screen while it flies).
struct Active {
    from: [f32; 2],
    to: [f32; 2],
    to_slot: Option<usize>,
    /// The stack, with the count that flies.
    stack: ItemStack,
    progress: f64,
}

/// What the last frame showed.
struct Seen {
    frame: u64,
    window: i32,
    menu: String,
    ids: Vec<usize>,
    slots: Vec<Option<ItemStack>>,
    carried: Option<ItemStack>,
}

struct State {
    /// Frames counted by the frame hook: a frame the screen was not drawn
    /// breaks the chain, and the next one starts afresh.
    frame: u64,
    pending_ms: f64,
    seen: Option<Seen>,
    flights: Vec<Active>,
}

static STATE: Mutex<State> = Mutex::new(State {
    frame: 0,
    pending_ms: 0.0,
    seen: None,
    flights: Vec::new(),
});

fn state() -> std::sync::MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

fn settings(game: &Game<'_>) -> ss::Settings {
    ss::settings(|key| game.native_setting(ss::ID, key))
}

fn russian(game: &Game<'_>) -> bool {
    ss::russian(&game.settings.language)
}

/// The screens whose slots `draw_inventory` draws: the player's inventory,
/// containers, furnaces, crafting tables, mounts and mod machines. Not the
/// creative screen (as the original) nor the stations, which draw their own.
/// Toggle Mod off (`toggleMod`), nothing flies.
fn tracked(game: &Game<'_>) -> bool {
    game.screen == Screen::Inventory
        && game.station.is_none()
        && game.native_on(ss::ID)
        && ss::with_settings(|key| game.native_setting(ss::ID, key), |s| s.enabled)
}

/// The slots whose stack the game computes: the crafting result, a
/// furnace's output, a crafter's result.
fn result_slot(game: &Game<'_>, slot: usize) -> bool {
    match slot {
        200 => true,
        302 => matches!(game.container_kind(), Some(BlockEntity::Furnace(_))),
        309 => matches!(game.container_kind(), Some(BlockEntity::Crafter(_))),
        _ => false,
    }
}

fn kind_of(protos: &mut Vec<ItemStack>, stack: &ItemStack) -> u32 {
    match protos.iter().position(|p| p.same_item_same_components(stack)) {
        Some(k) => k as u32,
        None => {
            protos.push(stack.clone());
            (protos.len() - 1) as u32
        }
    }
}

fn held(protos: &mut Vec<ItemStack>, stack: &Option<ItemStack>) -> ss::Held {
    stack
        .as_ref()
        .filter(|s| s.count > 0 && &*s.item != "minecraft:air")
        .map(|s| (kind_of(protos, s), s.count))
}

// ---- the hooks -------------------------------------------------------------

pub(super) fn list(game: &Game<'_>, mods: &mut Vec<NativeMod>) {
    mods.push(NativeMod {
        id: ss::ID.into(),
        title: format!("{} {}", ss::NAME, ss::VERSION),
        description: ss::description(russian(game)).into(),
        keys: Vec::new(),
    });
}

/// The mod's page: Configure... (Mod Menu's `modmenu.configure`) opens the
/// config screen, then the speed, the curve and Fly to the cursor.
pub(super) fn page(game: &Game<'_>, mod_id: &str, labels: &mut Vec<String>) {
    if mod_id == ss::ID {
        let ru = russian(game);
        labels.push(if ru { "Настройки..." } else { "Configure..." }.to_owned());
        labels.extend(settings(game).labels(ru));
    }
}

/// A click on the page: its first button, or Mod Menu's Configure, opens
/// the config screen; the others change their setting.
pub(super) fn page_click(game: &Game<'_>, mod_id: &str, button: usize) {
    if mod_id != ss::ID {
        return;
    }
    if button == 0 || button == super::smoothswapping_screen::MODMENU_CONFIGURE {
        super::smoothswapping_screen::open(game, button != 0);
        return;
    }
    let mut s = settings(game);
    s.click(button - 1);
    ss::set_settings(s.clone());
    for (key, value) in s.stored() {
        game.native_set_setting(ss::ID, key, &value);
    }
}

/// Each frame: its time, for the flights.
pub(super) fn frame(elapsed: Duration) {
    let mut st = state();
    st.frame += 1;
    // A long frame (a world loading) moves a flight no further than 100 ms.
    st.pending_ms += (elapsed.as_secs_f64() * 1000.0).min(100.0);
}

/// Before the slots are drawn: the flights move on, and what changed since
/// the last frame starts new ones.
pub(super) fn detect(game: &mut Game<'_>) {
    if !tracked(game) {
        let mut st = state();
        st.seen = None;
        st.flights.clear();
        return;
    }
    let settings = settings(game);
    let (window, menu) = game.remote_menu();
    let ids = game.slots();
    let slots: Vec<Option<ItemStack>> = ids.iter().map(|&i| game.slot_view(i)).collect();
    let carried = game.carried.clone();
    let results: Vec<bool> = ids.iter().map(|&i| result_slot(game, i)).collect();
    let cursor = [game.cursor.0, game.cursor.1];
    let origin = match game.renderer.as_ref() {
        Some(r) => game.inventory_origin(&HudBatch::new([r.config.width, r.config.height])),
        None => (0.0, 0.0),
    };
    let positions: Vec<(f32, f32)> = ids.iter().map(|&i| game.slot_position(i)).collect();

    let mut st = state();
    let dt = std::mem::take(&mut st.pending_ms);
    let rate = settings.rate_per_ms();
    for f in &mut st.flights {
        f.progress += dt * rate;
    }
    st.flights.retain(|f| f.progress < 1.0);
    let frame = st.frame;
    let previous = st.seen.replace(Seen {
        frame,
        window,
        menu: menu.clone(),
        ids: ids.clone(),
        slots: slots.clone(),
        carried: carried.clone(),
    });
    let Some(prev) = previous else {
        st.flights.clear();
        return;
    };
    if prev.window != window || prev.menu != menu || prev.ids != ids || frame > prev.frame + 1 {
        // Another screen, or this one after a pause: nothing flies into it.
        st.flights.clear();
        return;
    }
    let mut protos: Vec<ItemStack> = Vec::new();
    let mut changes = Vec::new();
    for (n, &id) in ids.iter().enumerate() {
        let old = held(&mut protos, &prev.slots[n]);
        let new = held(&mut protos, &slots[n]);
        if old != new {
            changes.push(ss::Change {
                slot: id,
                old,
                new,
                result: results[n],
            });
        }
    }
    let cursor_old = held(&mut protos, &prev.carried);
    let cursor_new = held(&mut protos, &carried);
    if changes.is_empty() {
        return;
    }
    // A slot that changed again lands what was flying to it (the original
    // drops a slot's swaps when it is clicked).
    st.flights
        .retain(|f| f.to_slot.is_none_or(|s| !changes.iter().any(|c| c.slot == s && !c.result)));
    let at = |slot: usize| -> Option<[f32; 2]> {
        let n = ids.iter().position(|&i| i == slot)?;
        Some([positions[n].0, positions[n].1])
    };
    for f in ss::flights(&changes, cursor_old, cursor_new, settings.to_cursor) {
        let Some(from) = at(f.from) else {
            continue;
        };
        let (to, to_slot) = match f.to {
            ss::To::Slot(s) => match at(s) {
                Some(p) => (p, Some(s)),
                None => continue,
            },
            ss::To::Cursor => ([cursor[0] - 8.0 - origin.0, cursor[1] - 8.0 - origin.1], None),
        };
        let Some(proto) = protos.get(f.kind as usize) else {
            continue;
        };
        let mut stack = proto.clone();
        stack.count = f.amount;
        st.flights.push(Active {
            from,
            to,
            to_slot,
            stack,
            progress: 0.0,
        });
    }
}

/// A slot's item, before the game draws it: a destination shows only what
/// has landed. True when this took the drawing (the caller then draws
/// nothing).
pub(super) fn slot(
    game: &Game<'_>,
    batch: &mut HudBatch,
    slot: usize,
    stack: &Option<ItemStack>,
    x: f32,
    y: f32,
) -> bool {
    if !tracked(game) {
        return false;
    }
    let incoming: u32 = state()
        .flights
        .iter()
        .filter(|f| f.to_slot == Some(slot))
        .map(|f| f.stack.count)
        .sum();
    let Some(stack) = stack.as_ref().filter(|_| incoming > 0) else {
        return false;
    };
    let shown = ss::shown_count(stack.count, incoming);
    if shown > 0 {
        let mut part = stack.clone();
        part.count = shown;
        game.draw_item_model(batch, &part, x, y);
        game.draw_item_decorations(batch, &part, x, y, None);
    }
    true
}

/// Over the slots, under the held stack and the tooltips: the flying stacks,
/// each with its count.
pub(super) fn draw(game: &Game<'_>, batch: &mut HudBatch) {
    if !tracked(game) {
        return;
    }
    let points = settings(game).points;
    let (ox, oy) = game.inventory_origin(batch);
    let flying: Vec<(ItemStack, [f32; 2])> = state()
        .flights
        .iter()
        .map(|f| (f.stack.clone(), ss::position(f.from, f.to, f.progress, &points)))
        .collect();
    for (stack, [x, y]) in flying {
        game.draw_item(batch, &stack, ox + x, oy + y);
    }
}
