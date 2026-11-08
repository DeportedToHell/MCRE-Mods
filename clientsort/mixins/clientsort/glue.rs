// ClientSort's edges in the game (not in the Cargo library): the open
// screen's slot groups read from Horizon's own layout, the planner's clicks
// played through the menu engine (predicted here and sent to the server as
// vanilla's ContainerClick, as a player's own clicks are), and the trigger
// buttons drawn over the screen.
use super::clientsort as cs;
use super::{Game, Screen};
use horizon_client::hud::HudBatch;
use horizon_client::items::{ItemStack, Items};
use horizon_client::menu::Input;
use horizon_client::mod_api::{NativeMod, NativeScreenView};
use horizon_client::sound_engine::{Attenuation, MASTER, SoundEngine, SoundInstance, Sounds as _};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// An operation under way: its clicks, played as the interval allows, each
/// checked against the slot and the cursor it expects.
struct Job {
    window: i32,
    /// The game slot of each of the plan's slots.
    ids: Vec<usize>,
    /// One stack of each kind the plan names.
    protos: Vec<ItemStack>,
    steps: Vec<cs::Step>,
    next: usize,
    interval: u32,
    budget: f32,
    /// The clicks' sound (ClientSort's interaction sound), when it plays.
    sound: Option<cs::SoundPitch>,
}

static JOB: Mutex<Option<Job>> = Mutex::new(None);
/// `SoundManager`: when the next sound may start, and the last one, held to
/// be stopped when sounds may not overlap.
static SOUND: Mutex<(Option<Instant>, Option<u64>)> = Mutex::new((None, None));
/// A mouse button whose press ClientSort took: its release is taken too.
static SWALLOWED: Mutex<Option<i32>> = Mutex::new(None);

/// The open screen's slot groups, as Horizon lays them out.
struct View {
    window: i32,
    container: Vec<usize>,
    main: Vec<usize>,
    hotbar: Vec<usize>,
    shulker: bool,
    /// The panel's right edge, the tops of the container's and the main
    /// inventory's first rows, in GUI pixels.
    right: f32,
    container_top: f32,
    main_top: f32,
}

struct Button {
    group: cs::Group,
    op: cs::Op,
    rect: [f32; 4],
    down: bool,
}

/// Chests, barrels, ender chests, shulker boxes, hoppers and dispensers: the
/// containers whose slots ClientSort's operations may fill with anything.
fn storage_menu(menu: &str) -> bool {
    menu.starts_with("minecraft:generic_9x")
        || matches!(
            menu,
            "minecraft:generic_3x3" | "minecraft:shulker_box" | "minecraft:hopper"
        )
}

pub(super) fn settings(game: &Game<'_>) -> cs::Settings {
    cs::settings(|key| game.native_setting(cs::ID, key))
}

fn russian(game: &Game<'_>) -> bool {
    cs::russian(&game.settings.language)
}

fn batch(game: &Game<'_>) -> Option<HudBatch> {
    let r = game.renderer.as_ref()?;
    Some(HudBatch::new([r.config.width, r.config.height]))
}

fn view(game: &Game<'_>, batch: &HudBatch) -> Option<View> {
    if game.screen != Screen::Inventory
        || game.remote.is_none()
        || game.station.is_some()
        || game.mount_active()
        || game.native_screen_shown()
    {
        return None;
    }
    let (window, menu) = game.remote_menu();
    // The menus Horizon's own inventory tweaks click in: the player's
    // inventory, chest-like containers, furnaces and crafting tables.
    if !super::tweaks::tweak_menu(&menu) {
        return None;
    }
    let slots = game.slots();
    let known = |i: usize| slots.contains(&i) && Game::menu_slot_index(&menu, i).is_some();
    let mut main: Vec<usize> = (9..36).collect();
    let hotbar: Vec<usize> = (0..9).collect();
    if !main.iter().chain(&hotbar).all(|&i| known(i)) {
        return None;
    }
    // The hotbar as part of the main inventory, after it as in the menu.
    if settings(game).hotbar == cs::HotbarScope::Inventory {
        main.extend_from_slice(&hotbar);
    }
    let mut container: Vec<usize> = if storage_menu(&menu) {
        slots.iter().copied().filter(|i| (300..500).contains(i)).collect()
    } else {
        Vec::new()
    };
    container.sort_unstable();
    if !container.iter().all(|&i| known(i)) {
        container.clear();
    }
    let (ox, oy) = game.inventory_origin(batch);
    let width = game.screen_rect(batch, Screen::Inventory)[2];
    let container_top = container.first().map_or(0.0, |&i| game.slot_position(i).1);
    Some(View {
        window,
        container,
        main,
        hotbar,
        shulker: menu == "minecraft:shulker_box",
        right: ox + width,
        container_top: oy + container_top,
        main_top: oy + game.slot_position(9).1,
    })
}

/// ClientSort's default layout: a column on the panel's right edge by each
/// inventory, the container's arrows pointing down and the player's up. A
/// short container (a hopper) shows as many of its buttons as fit above the
/// player's.
fn buttons(view: &View, ops: &[cs::Op; 4]) -> Vec<Button> {
    let mut out = Vec::new();
    let other = !view.container.is_empty();
    if other {
        let room = ((view.main_top - view.container_top + 1.0) / cs::BUTTON_STEP).floor();
        let fit = room.max(1.0) as usize;
        for (i, &op) in cs::group_ops(true, ops).iter().enumerate().take(fit) {
            out.push(Button {
                group: cs::Group::Container,
                op,
                rect: cs::button_rect(view.right, view.container_top, i),
                down: true,
            });
        }
    }
    for (i, &op) in cs::group_ops(other, ops).iter().enumerate() {
        out.push(Button {
            group: cs::Group::Main,
            op,
            rect: cs::button_rect(view.right, view.main_top, i),
            down: false,
        });
    }
    out
}

/// The group of the slot under the cursor.
fn hovered(game: &Game<'_>, batch: &HudBatch, view: &View, settings: &cs::Settings) -> Option<cs::Group> {
    let slot = game.slot_at(batch, game.cursor.0, game.cursor.1)?;
    if view.container.contains(&slot) {
        Some(cs::Group::Container)
    } else if view.main.contains(&slot) {
        Some(cs::Group::Main)
    } else if settings.hotbar == cs::HotbarScope::Hotbar && view.hotbar.contains(&slot) {
        Some(cs::Group::Hotbar)
    } else {
        None
    }
}

/// `ItemStack.getMaxStackSize`: the stack's component, else the item's (as
/// the menu engine reads it).
fn max_stack(items: &Items, stack: &ItemStack) -> u32 {
    let patched = stack.data().and_then(|d| {
        if d.other("!minecraft:max_stack_size").is_some() {
            return None;
        }
        match d.other("minecraft:max_stack_size") {
            Some(horizon_world::nbt::Tag::Int(n)) if (1..=99).contains(n) => Some(*n as u32),
            _ => None,
        }
    });
    patched.unwrap_or_else(|| items.get(&stack.item).map_or(64, |i| i.max_stack))
}

fn plan_stack(
    items: &Items,
    settings: &cs::Settings,
    stack: &ItemStack,
    protos: &mut Vec<ItemStack>,
    item_ids: &mut HashMap<String, u32>,
) -> cs::Stack {
    let kind = match protos.iter().position(|p| p.same_item_same_components(stack)) {
        Some(k) => k,
        None => {
            protos.push(stack.clone());
            protos.len() - 1
        }
    };
    let next = item_ids.len() as u32;
    let item = *item_ids.entry(stack.item.to_string()).or_insert(next);
    let id: &str = &stack.item;
    cs::Stack {
        kind: kind as u32,
        item,
        count: stack.count,
        max: max_stack(items, stack),
        bundle: id == "minecraft:bundle" || (id.starts_with("minecraft:") && id.ends_with("_bundle")),
        boxed: id == "minecraft:shulker_box"
            || (id.starts_with("minecraft:") && id.ends_with("_shulker_box")),
        by_type: settings.always_match_by_type
            || settings.type_match_tags.iter().any(|tag| items.item_in_tag(&cs::full_id(tag), id)),
    }
}

/// What the sort orders compare of each kind of stack.
fn keys(game: &Game<'_>, settings: &cs::Settings, protos: &[ItemStack]) -> Vec<cs::Key> {
    let items = game.items();
    // The sort order overrides: an item's first place in each list.
    let place = |on: bool, list: &[String], item: &str| {
        on.then(|| list.iter().position(|id| cs::full_id(id.trim()) == item)).flatten()
    };
    let registry: HashMap<&str, usize> = items
        .registry_order
        .iter()
        .enumerate()
        .map(|(i, id)| (&**id, i))
        .collect();
    protos
        .iter()
        .map(|p| cs::Key {
            creative: game.creative_screen.order_of(p).map_or(u64::MAX, |i| i as u64),
            registry: registry.get(&*p.item).map_or(u64::MAX, |&i| i as u64),
            name: game.stack_display_name(p).plain().to_lowercase(),
            custom: p.custom_name().is_some(),
            damage: p.damage,
            tie: format!("{}{:?}", p.item, p.extra),
            start: place(settings.use_start_overrides, &settings.start_items, &p.item),
            end: place(settings.use_end_overrides, &settings.end_items, &p.item),
        })
        .collect()
}

/// Plans `op` on `group` and starts it. True when ClientSort took the input
/// (an operation started, had nothing to do, or one is still running).
fn start(game: &mut Game<'_>, op: cs::Op, group: cs::Group, order: cs::Order) -> bool {
    // ClientSort's "none" order: no sort, and the input goes on.
    if op == cs::Op::Sort && order == cs::Order::None {
        return false;
    }
    if JOB.lock().unwrap_or_else(|e| e.into_inner()).is_some() {
        return true;
    }
    let Some(batch) = batch(game) else {
        return false;
    };
    let Some(view) = view(game, &batch) else {
        return false;
    };
    if game.carried.is_some() {
        return false;
    }
    let settings = settings(game);
    let (origin, other): (&[usize], &[usize]) = match group {
        cs::Group::Container => (&view.container, &view.main),
        cs::Group::Main => (&view.main, &view.container),
        cs::Group::Hotbar => (&view.hotbar, &view.container),
    };
    if origin.is_empty() || (op.directional() && other.is_empty()) {
        return false;
    }
    let mut ids: Vec<usize> = origin.to_vec();
    if op.directional() {
        ids.extend_from_slice(other);
    }
    let found: Vec<Option<ItemStack>> = ids.iter().map(|&i| game.slot_view(i)).collect();
    let mut protos = Vec::new();
    let mut item_ids = HashMap::new();
    let stacks: Vec<Option<cs::Stack>> = {
        let items = game.items();
        found
            .iter()
            .map(|s| s.as_ref().map(|s| plan_stack(items, &settings, s, &mut protos, &mut item_ids)))
            .collect()
    };
    let refuse = ids.iter().map(|&i| view.shulker && i >= 300).collect();
    let mut plan = cs::Plan::new(stacks, refuse);
    let o: Vec<usize> = (0..origin.len()).collect();
    let rest: Vec<usize> = (origin.len()..ids.len()).collect();
    match op {
        cs::Op::Sort => {
            plan.collect(&o);
            let keys = keys(game, &settings, &protos);
            let target = cs::sort_order(&plan.slots, &o, &keys, order);
            plan.sort(&o, &target);
        }
        cs::Op::StackFill => plan.fill(&o, &rest),
        cs::Op::MatchTransfer => {
            let from = plan.matching(&o, &rest, true);
            plan.transfer(&from, &rest, settings.transfer_reverse);
        }
        cs::Op::Transfer => plan.transfer(&o, &rest, settings.transfer_reverse),
    }
    if plan.broken || plan.steps.is_empty() {
        return true;
    }
    let interval = settings.interval_ms(game.has_singleplayer_server());
    // Sounds without server acceleration, the pitch climbing over the clicks.
    let wanted = if op == cs::Op::Sort { settings.play_sound_sort } else { settings.play_sound_other };
    let sound = (wanted && interval > 0 && settings.sound_volume > 0.0)
        .then(|| cs::SoundPitch::new(&settings, plan.steps.len()));
    *JOB.lock().unwrap_or_else(|e| e.into_inner()) = Some(Job {
        window: view.window,
        ids,
        protos,
        steps: plan.steps,
        next: 0,
        interval,
        budget: 0.0,
        sound,
    });
    if interval == 0 {
        run(game, usize::MAX);
    }
    true
}

fn same(found: &Option<ItemStack>, expected: Option<cs::Stack>, protos: &[ItemStack]) -> bool {
    match (found, expected) {
        (None, None) => true,
        (Some(f), Some(e)) => {
            f.count == e.count && protos.get(e.kind as usize).is_some_and(|p| f.same_item_same_components(p))
        }
        _ => false,
    }
}

/// Plays up to `clicks` of the running operation's clicks. A slot or a cursor
/// that isn't what the plan expects (the server moved something, the player
/// did) ends it, with the cursor's stack put down in an empty slot.
fn run(game: &mut Game<'_>, mut clicks: usize) {
    let mut lock = JOB.lock().unwrap_or_else(|e| e.into_inner());
    let Some(mut job) = lock.take() else {
        return;
    };
    while clicks > 0 && job.next < job.steps.len() {
        if game.screen != Screen::Inventory || game.remote_menu().0 != job.window {
            // The screen closed: the server gives back what the cursor held.
            return;
        }
        let step = job.steps[job.next];
        let id = job.ids[step.slot];
        let here = game.slot_view(id);
        let ok = same(&here, step.before, &job.protos)
            && same(&game.carried, step.carried, &job.protos)
            && game.remote_menu_ops(&[(Some(id), step.button, Input::Pickup)]);
        if !ok {
            put_back(game, &job.ids);
            return;
        }
        job.next += 1;
        clicks -= 1;
        if let Some(pitch) = job.sound.as_mut() {
            let settings = settings(game);
            let p = pitch.next(settings.sound_pitch_max);
            play_sound(game, &settings, p);
        }
    }
    if job.next < job.steps.len() {
        *lock = Some(job);
    }
}

/// `SoundManager.play`: the interaction sound, at most once a sound
/// interval, the last one stopped unless sounds may overlap.
fn play_sound(game: &mut Game<'_>, settings: &cs::Settings, pitch: f32) {
    let mut state = SOUND.lock().unwrap_or_else(|e| e.into_inner());
    let now = Instant::now();
    if state.0.is_some_and(|next| now < next) {
        return;
    }
    state.0 = Some(now + Duration::from_millis(u64::from(settings.sound_interval)));
    if let Some(last) = state.1.take() {
        game.audio.stop_and_release(last);
    }
    let instance = SoundInstance::new(
        &cs::full_id(&settings.interaction_sound),
        MASTER,
        settings.sound_volume,
        pitch,
        SoundEngine::unseeded(),
        false,
        0,
        Attenuation::None,
        [0.0; 3],
        true,
    );
    let hold = !settings.allow_sound_overlap;
    let (id, _) = game.audio.play_instance(instance, hold);
    if hold {
        state.1 = Some(id);
    }
}

fn put_back(game: &mut Game<'_>, ids: &[usize]) {
    for &id in ids {
        if game.carried.is_none() {
            return;
        }
        if game.slot_view(id).is_none() {
            game.remote_menu_ops(&[(Some(id), 0, Input::Pickup)]);
        }
    }
}

// ---- the hooks -------------------------------------------------------------

/// The mod's row in Options > Mods, with its keys.
pub(super) fn list(game: &Game<'_>, mods: &mut Vec<NativeMod>) {
    let ru = russian(game);
    mods.push(NativeMod {
        id: cs::ID.into(),
        title: format!("{} {}", cs::NAME, cs::VERSION),
        description: cs::description(ru).into(),
        keys: cs::Op::ALL
            .iter()
            .map(|op| [op.key().to_string(), op.label(ru).to_string(), op.default_key().to_string()])
            .collect(),
    });
}

/// The mod's settings page: Configure... (ClientSort's own screen), then
/// the first port's buttons.
pub(super) fn page(game: &Game<'_>, mod_id: &str, labels: &mut Vec<String>) {
    if mod_id == cs::ID {
        labels.push(super::clientsort_config::configure_label().into());
        labels.extend(settings(game).labels(russian(game)));
    }
}

/// A click on the settings page: Configure... opens ClientSort's screen (and
/// Mod Menu's Configure, `MODMENU_CONFIGURE`, does too); the others change
/// a setting and keep it.
pub(super) fn page_click(game: &mut Game<'_>, mod_id: &str, button: usize) {
    if mod_id != cs::ID {
        return;
    }
    if button == super::clientsort_config::MODMENU_CONFIGURE {
        super::clientsort_config::open(game, true);
        return;
    }
    if button == 0 {
        super::clientsort_config::open(game, false);
        return;
    }
    let mut s = settings(game);
    s.click(button - 1);
    save_settings(game, s);
}

/// Keeps `s` in the mod's folder and in memory.
pub(super) fn save_settings(game: &Game<'_>, s: cs::Settings) {
    for (key, value) in s.stored() {
        if game.native_setting(cs::ID, key).as_deref() != Some(value.trim()) {
            game.native_set_setting(cs::ID, key, &value);
        }
    }
    cs::set_settings(s);
}

/// Each frame: the running operation's next clicks.
pub(super) fn frame(game: &mut Game<'_>, elapsed: Duration) {
    let due = {
        let mut lock = JOB.lock().unwrap_or_else(|e| e.into_inner());
        let Some(job) = lock.as_mut() else {
            return;
        };
        cs::clicks_due(&mut job.budget, elapsed.as_secs_f32() * 1000.0, job.interval)
    };
    if due > 0 {
        run(game, due);
    }
}

/// A mouse button over a screen, before the screen sees it: a left press on
/// a button, or an operation's key (the middle button sorts) over a slot.
pub(super) fn mouse(game: &mut Game<'_>, button: i32, pressed: bool, modifiers: u32, canceled: &mut bool) {
    if !pressed {
        let mut swallowed = SWALLOWED.lock().unwrap_or_else(|e| e.into_inner());
        if *swallowed == Some(button) {
            *swallowed = None;
            *canceled = true;
        }
        return;
    }
    if game.screen != Screen::Inventory || !game.screen_hooks_open() {
        return;
    }
    let Some(batch) = batch(game) else {
        return;
    };
    let Some(view) = view(game, &batch) else {
        return;
    };
    let settings = settings(game);
    let order = settings.order_for(modifiers & 1 != 0, modifiers & 2 != 0, modifiers & 4 != 0);
    let at = [game.cursor.0, game.cursor.1];
    let pressed_button = (button == 0 && settings.buttons)
        .then(|| buttons(&view, &settings.button_ops).into_iter().find(|b| cs::inside(b.rect, at)))
        .flatten();
    let took = if let Some(b) = pressed_button {
        start(game, b.op, b.group, order);
        game.audio.play_button_click();
        true
    } else {
        let key = cs::mouse_key(button);
        let op = cs::Op::ALL
            .into_iter()
            .find(|op| game.native_mod_key(cs::ID, op.key(), op.default_key()) == key);
        match (op, hovered(game, &batch, &view, &settings)) {
            (Some(op), Some(group)) => start(game, op, group, order),
            _ => false,
        }
    };
    if took {
        *canceled = true;
        *SWALLOWED.lock().unwrap_or_else(|e| e.into_inner()) = Some(button);
    }
}

/// A key on any screen: an operation's key over a slot.
pub(super) fn key(game: &mut Game<'_>, key: &str, pressed: bool) {
    if !pressed || key == cs::UNBOUND || game.screen != Screen::Inventory {
        return;
    }
    let Some(op) = cs::Op::ALL
        .into_iter()
        .find(|op| game.native_mod_key(cs::ID, op.key(), op.default_key()) == key)
    else {
        return;
    };
    let Some(batch) = batch(game) else {
        return;
    };
    let Some(view) = view(game, &batch) else {
        return;
    };
    let settings = settings(game);
    let Some(group) = hovered(game, &batch, &view, &settings) else {
        return;
    };
    let order = settings.order_for(game.has_shift_down(), game.has_control_down(), game.has_alt_down());
    start(game, op, group, order);
}

/// ClientSort's buttons show on the open screen.
fn shown(game: &Game<'_>) -> bool {
    game.native_on(cs::ID)
        && settings(game).buttons
        && batch(game).is_some_and(|b| view(game, &b).is_some())
}

/// Horizon's own sort buttons (Extras > Sorting) give way to ClientSort's
/// where those show, as the Java game has only ClientSort's.
pub(super) fn hides_horizon_sort(game: &Game<'_>) -> bool {
    shown(game)
}

/// The buttons, drawn where Horizon draws its own sort buttons: after the
/// slots, before the held stack and the tooltips, as a screen's widgets
/// are in the Java game (an item's name covers them).
pub(super) fn draw(game: &Game<'_>, batch: &mut HudBatch) {
    if !game.native_on(cs::ID) || !settings(game).buttons {
        return;
    }
    let Some(view) = view(game, batch) else {
        return;
    };
    let at = [game.cursor.0, game.cursor.1];
    for b in buttons(&view, &settings(game).button_ops) {
        let hover = cs::inside(b.rect, at);
        for &(x, y, w, argb) in cs::sprite(b.op, b.down, hover) {
            let rect = [b.rect[0] + f32::from(x), b.rect[1] + f32::from(y), f32::from(w), 1.0];
            game.native_fill(batch, rect, argb);
        }
    }
}

/// Over the finished screen: the hovered button's name, with the option on
/// (off by default, as in ClientSort).
pub(super) fn render(game: &Game<'_>, batch: &mut HudBatch, screen: &NativeScreenView<'_>) {
    let settings = settings(game);
    if screen.id != "inventory" || !settings.buttons || !settings.tooltips || game.carried.is_some() {
        return;
    }
    let Some(view) = view(game, batch) else {
        return;
    };
    let Some(b) = buttons(&view, &settings.button_ops).into_iter().find(|b| cs::inside(b.rect, screen.mouse)) else {
        return;
    };
    let line = horizon_client::text::Text::literal(b.op.label(russian(game)));
    game.draw_tooltip_lines(batch, &[(line, u32::MAX)], screen.mouse[0], screen.mouse[1]);
}
