// Mouse Tweaks' edges in the game (not in the Cargo library): the open
// screen's menu as `Menu` (its slots in the menu's order, read from Horizon's
// own layout; its slot rules from the verified menu engine), and the mouse
// events of the container screens, where Horizon's own inventory tweaks
// (Options > Extras) take them. While Mouse Tweaks is on, those stay off.
use super::mousetweaks as mt;
use super::{Game, Screen};
use horizon_client::items::{ItemStack, Items};
use horizon_client::menu::Input;
use horizon_client::mod_api::NativeMod;
use std::collections::HashMap;
use std::sync::Mutex;

static TWEAKS: Mutex<mt::Tweaks> = Mutex::new(mt::Tweaks::new());

fn tweaks() -> std::sync::MutexGuard<'static, mt::Tweaks> {
    TWEAKS.lock().unwrap_or_else(|e| e.into_inner())
}

fn settings(game: &Game<'_>) -> mt::Settings {
    mt::settings(|key| game.native_setting(mt::ID, key))
}

fn russian(game: &Game<'_>) -> bool {
    mt::russian(&game.settings.language)
}

/// The open container screen the tweaks run in, named so that another one
/// (another window, another menu, another creative tab) starts afresh: any
/// `AbstractContainerScreen` whose menu the menu engine models, so each
/// click's outcome is known before the next one is chosen, the creative
/// screen included (`GuiContainerCreativeHandler`). Not a lectern or a
/// native mod's own screen.
fn screen_key(game: &mut Game<'_>) -> Option<String> {
    if game.remote.is_none() || game.native_screen_shown() {
        return None;
    }
    let key = match game.screen {
        Screen::Inventory => {
            let (window, menu) = game.remote_menu();
            if menu == "minecraft:lectern" {
                return None;
            }
            format!("{window} {menu}")
        }
        Screen::Creative if game.creative_inventory_open() => "creative inventory".to_owned(),
        Screen::Creative => "creative items".to_owned(),
        _ => return None,
    };
    game.menu_session(|_, _, _| Some(false)).then_some(key)
}

/// The screens whose own `mouseScrolled` takes every wheel event (their
/// lists scroll), so the wheel tweak never sees one.
fn wheel_taken(menu: &str) -> bool {
    matches!(menu, "minecraft:merchant" | "minecraft:stonecutter" | "minecraft:loom")
}

/// `ItemStack.getMaxStackSize`: the stack's component, else the item's.
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

/// The open menu, its slots in the menu's own order.
struct View<'g, 'a> {
    game: &'g mut Game<'a>,
    menu: String,
    ids: Vec<usize>,
    /// One stack of each kind seen, by `Stack::kind`.
    protos: Vec<ItemStack>,
    /// `Slot.mayPlace` and `Slot.getMaxStackSize` by slot and kind, asked of
    /// the engine once an event.
    places: HashMap<(usize, u32), bool>,
    maxes: HashMap<(usize, u32), u32>,
}

impl<'g, 'a> View<'g, 'a> {
    fn new(game: &'g mut Game<'a>) -> Self {
        let shown = game.slots();
        // The creative screen's slots that aren't the player's are ignored
        // (`GuiContainerCreativeHandler.isIgnored`): an item tab keeps its
        // hotbar row, the inventory tab the player's InventoryMenu slots.
        let (menu, ids) = if game.screen == Screen::Creative {
            let ids = if game.creative_inventory_open() {
                (0..)
                    .map_while(|s| Game::menu_slot("inventory", s))
                    .filter(|id| shown.contains(id) && (*id < 36 || (500..=504).contains(id)))
                    .collect()
            } else {
                (0..9).filter(|id| shown.contains(id)).collect()
            };
            ("creative".to_owned(), ids)
        } else {
            let (_, menu) = game.remote_menu();
            let ids = (0..)
                .map_while(|s| Game::menu_slot(&menu, s))
                .filter(|id| shown.contains(id))
                .collect();
            (menu, ids)
        };
        View {
            game,
            menu,
            ids,
            protos: Vec::new(),
            places: HashMap::new(),
            maxes: HashMap::new(),
        }
    }

    fn index(&self, id: usize) -> Option<usize> {
        self.ids.iter().position(|&i| i == id)
    }

    fn stack_of(&mut self, stack: Option<ItemStack>) -> Option<mt::Stack> {
        let stack = stack.filter(|s| s.count > 0 && &*s.item != "minecraft:air")?;
        let kind = match self.protos.iter().position(|p| p.same_item_same_components(&stack)) {
            Some(k) => k,
            None => {
                self.protos.push(stack.clone());
                self.protos.len() - 1
            }
        };
        let id: &str = &stack.item;
        Some(mt::Stack {
            kind: kind as u32,
            count: stack.count,
            max: max_stack(self.game.items(), &stack),
            bundle: id == "minecraft:bundle" || (id.starts_with("minecraft:") && id.ends_with("_bundle")),
        })
    }

    /// The stack of `kind`, `count` of it.
    fn proto(&self, stack: mt::Stack, count: u32) -> Option<ItemStack> {
        let mut p = self.protos.get(stack.kind as usize)?.clone();
        p.count = count;
        Some(p)
    }
}

impl mt::Menu for View<'_, '_> {
    fn len(&self) -> usize {
        self.ids.len()
    }
    fn stack(&mut self, slot: usize) -> Option<mt::Stack> {
        let found = self.game.slot_view(self.ids[slot]);
        self.stack_of(found)
    }
    fn carried(&mut self) -> Option<mt::Stack> {
        let found = self.game.carried.clone();
        self.stack_of(found)
    }
    fn player(&self, slot: usize) -> bool {
        // The hotbar and main inventory, the armour slots and the off hand.
        let id = self.ids[slot];
        id < 36 || (500..=504).contains(&id)
    }
    fn output(&self, slot: usize) -> bool {
        // ResultSlot, FurnaceResultSlot, MerchantResultSlot.
        match self.ids[slot] {
            200 => true,
            302 => matches!(
                self.menu.as_str(),
                "minecraft:furnace" | "minecraft:blast_furnace" | "minecraft:smoker"
            ),
            id => id == super::stations::RESULT && self.menu == "minecraft:merchant",
        }
    }
    fn y(&self, slot: usize) -> f32 {
        self.game.slot_position(self.ids[slot]).1
    }
    fn may_place(&mut self, slot: usize, stack: mt::Stack) -> bool {
        let id = self.ids[slot];
        if let Some(&takes) = self.places.get(&(id, stack.kind)) {
            return takes;
        }
        let Some(one) = self.proto(stack, 1) else {
            return false;
        };
        let mut takes = false;
        // On a copy of the menu, thrown away: the slot emptied takes one?
        self.game.menu_session(|m, env, to_menu| {
            let s = to_menu(id)?;
            m.set_slot(env, s, None);
            takes = m.takes_one(env, s, &one);
            Some(false)
        });
        self.places.insert((id, stack.kind), takes);
        takes
    }
    fn max_size(&mut self, slot: usize, stack: mt::Stack) -> u32 {
        let id = self.ids[slot];
        if let Some(&max) = self.maxes.get(&(id, stack.kind)) {
            return max;
        }
        let limit = stack.max.clamp(1, 99);
        let (Some(one), Some(proto)) = (self.proto(stack, 1), self.proto(stack, limit)) else {
            return 0;
        };
        let mut max = 0;
        // On a copy of the menu, thrown away: the slot holding `c` takes one
        // more while `c` is below its limit, so the limit is the first `c`
        // it refuses at.
        self.game.menu_session(|m, env, to_menu| {
            let s = to_menu(id)?;
            m.set_slot(env, s, None);
            if !m.takes_one(env, s, &one) {
                return Some(false);
            }
            let (mut lo, mut hi) = (1, limit);
            while lo < hi {
                let mid = (lo + hi) / 2;
                let mut held = proto.clone();
                held.count = mid;
                m.set_slot(env, s, Some(held));
                if m.takes_one(env, s, &one) {
                    lo = mid + 1;
                } else {
                    hi = mid;
                }
            }
            max = lo;
            Some(false)
        });
        self.maxes.insert((id, stack.kind), max);
        max
    }
    fn click(&mut self, slot: usize, right: bool, shift: bool) {
        let input = if shift { Input::QuickMove } else { Input::Pickup };
        self.game.menu_click(Some(self.ids[slot]), i32::from(right), input);
    }
    fn stop_rmb_drag(&mut self) {
        self.game.inv_drag = None;
        self.game.inv_clicks.skip_release();
    }
}

// ---- the hooks -------------------------------------------------------------

pub(super) fn list(game: &Game<'_>, mods: &mut Vec<NativeMod>) {
    mods.push(NativeMod {
        id: mt::ID.into(),
        title: format!("{} {}", mt::NAME, mt::VERSION),
        description: mt::description(russian(game)).into(),
        keys: Vec::new(),
    });
}

/// The mod's page: Configure... (Mod Menu's `modmenu.configure`) opens the
/// options screen, then a button per option.
pub(super) fn page(game: &Game<'_>, mod_id: &str, labels: &mut Vec<String>) {
    if mod_id == mt::ID {
        let ru = russian(game);
        labels.push(if ru { "Настройки..." } else { "Configure..." }.to_owned());
        labels.extend(settings(game).labels(ru));
    }
}

/// A click on the page: its first button, or Mod Menu's Configure, opens
/// the options screen; the others change their option.
pub(super) fn page_click(game: &Game<'_>, mod_id: &str, button: usize) {
    if mod_id != mt::ID {
        return;
    }
    if button == 0 || button == super::mousetweaks_screen::MODMENU_CONFIGURE {
        super::mousetweaks_screen::open(game, button != 0);
        return;
    }
    let mut s = settings(game);
    s.click(button - 1);
    mt::set_settings(s);
    for (key, value) in s.stored() {
        game.native_set_setting(mt::ID, key, &value);
    }
}

/// Each frame: with no container screen open the state is dropped.
pub(super) fn frame(game: &Game<'_>) {
    if !matches!(game.screen, Screen::Inventory | Screen::Creative) {
        tweaks().update_screen(None);
    }
}

/// A left or right press on a container screen, before the screen clicks
/// (`slot` is the slot under the cursor). True when Mouse Tweaks is on: the
/// press then goes on to the screen, and Horizon's own tweaks stay out.
pub(super) fn press(game: &mut Game<'_>, slot: Option<usize>, right: bool) -> bool {
    if !game.native_on(mt::ID) {
        return false;
    }
    let key = screen_key(game);
    let mut t = tweaks();
    if !t.update_screen(key.as_deref()) {
        return true;
    }
    let s = settings(game);
    let mut view = View::new(game);
    let hovered = slot.and_then(|id| view.index(id));
    t.press(&mut view, hovered, right, &s);
    true
}

/// The cursor moved over a container screen: a drag goes on while a button
/// is held.
pub(super) fn drag(game: &mut Game<'_>) {
    if !game.native_on(mt::ID) {
        return;
    }
    let mut t = tweaks();
    if !t.dragging() {
        return;
    }
    let key = screen_key(game);
    if !t.update_screen(key.as_deref()) {
        return;
    }
    let s = settings(game);
    let shift = game.has_shift_down();
    let slot = game.inventory_target().0;
    let mut view = View::new(game);
    let hovered = slot.and_then(|id| view.index(id));
    t.drag(&mut view, hovered, shift, &s);
}

/// A button let go over a container screen.
pub(super) fn release(game: &Game<'_>, right: bool) {
    if game.native_on(mt::ID) {
        tweaks().release(right);
    }
}

/// The wheel over a container screen, after a bundle under the cursor had
/// its turn. None when Mouse Tweaks is off (Horizon's own tweaks decide);
/// else whether it took the wheel.
pub(super) fn scroll(game: &mut Game<'_>, delta: f32, slot: Option<usize>) -> Option<bool> {
    if !game.native_on(mt::ID) {
        return None;
    }
    let key = screen_key(game);
    let mut t = tweaks();
    if !t.update_screen(key.as_deref()) || wheel_taken(&game.remote_menu().1) {
        return Some(false);
    }
    let s = settings(game);
    let mut view = View::new(game);
    let hovered = slot.and_then(|id| view.index(id));
    Some(t.scroll(&mut view, hovered, f64::from(delta), &s))
}
