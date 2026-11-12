// A menu of one of Traveler's Backpack's own menu types whose slots are no
// block's: the backpack's contents, held by the server's native menu
// (`Slots::Native`), taking and giving items by vanilla's click rules (the
// whole menu is its input store). A Horizon client shows it through the mod's
// screen; Java's `openMenu` of a backpack item. Spliced into the hub's
// container code (`lan/containers.rs`), whose own `open_native_menu` opens only
// a chest's `generic_9xN`.
use super::*;

/// Opens menu type `menu_kind` (`travelersbackpack:leather`) for guest `conn`
/// over their inventory, holding `items` (as many as the menu type's slots),
/// titled `title`: the window's id, or None for an unknown menu type or guest.
/// Its first `input` slots (the storage) take items by vanilla's rules; a click
/// on the others (upgrades, tools) is the mod's (`native_menu_clicked`).
/// `layout` goes ahead as the menu's extra data (`horizon:menu_extra`), for its screen.
pub fn open(
    hub: &mut Hub,
    conn: u32,
    menu_kind: &str,
    title: Tag,
    items: Vec<Option<ItemStack>>,
    input: usize,
    layout: &[u8],
) -> Option<i32> {
    let kind = hub.sync.builtin.get("minecraft:menu").and_then(|m| m.id(menu_kind))?;
    let i = hub.guests.iter().position(|g| g.conn == conn)?;
    hub.close_container_at(i);
    let size = items.len();
    let g = &mut hub.guests[i];
    g.native_menu = items.clone();
    g.next_window = g.next_window % 100 + 1;
    let window = g.next_window;
    let message = horizon_protocol::menu_extra::Message { window, pos: [0; 3], data: layout.to_vec() };
    if let Ok(data) = horizon_protocol::menu_extra::encode(&message) {
        g.send(&C::CustomPayload(CustomPayload { channel: horizon_protocol::menu_extra::CHANNEL.into(), data }));
    }
    g.send(&C::OpenScreen(play::OpenScreen { container: window, menu: kind, title }));
    g.menu = Some(Menu {
        window,
        kind,
        pos: None,
        slots: Slots::Native,
        size,
        state: 0,
        sent: Vec::new(),
        drag: None,
        sent_data: Vec::new(),
        hidden: Vec::new(),
        mod_type: None,
    });
    hub.send_menu_content(i, items);
    hub.set_native_store(conn, NativeStore::Input(0..input.min(size)));
    remember(conn, window);
    Some(window)
}

/// Closes `conn`'s menu `window` from the server, as `ServerPlayer.closeContainer`
/// does: its slots go to `native_menu_input_closed`. False when that isn't
/// the menu open.
pub fn close(hub: &mut Hub, conn: u32, window: i32) -> bool {
    let Some(i) = hub.guests.iter().position(|g| g.conn == conn) else {
        return false;
    };
    if hub.guests[i].menu.as_ref().is_none_or(|m| m.window != window) {
        return false;
    }
    hub.close_container_at(i);
    true
}

/// What guest `conn`'s open native menu holds, and their cursor.
pub fn items(hub: &Hub, conn: u32) -> Option<(Vec<Option<ItemStack>>, Option<ItemStack>)> {
    let g = hub.guests.iter().find(|g| g.conn == conn)?;
    Some((g.native_menu.clone(), g.carried.clone()))
}

/// Guest `conn`'s open native menu holds `items` and their cursor `carried`
/// from now on, sent to them at once (`broadcastChanges`).
pub fn set_items(hub: &mut Hub, conn: u32, items: Vec<Option<ItemStack>>, carried: Option<ItemStack>) {
    let Some(i) = hub.guests.iter().position(|g| g.conn == conn) else {
        return;
    };
    let g = &mut hub.guests[i];
    g.native_menu = items.clone();
    g.carried = carried;
    hub.send_menu_content(i, items);
}

/// Sends guest `conn`'s menu `window` its data slots (`ContainerSetData`,
/// 16 bits each on the wire): what its screen draws beside the slots.
pub fn send_data(hub: &mut Hub, conn: u32, window: i32, data: &[i32]) {
    let Some(g) = hub.guests.iter_mut().find(|g| g.conn == conn) else {
        return;
    };
    for (k, &value) in data.iter().enumerate() {
        g.send(&C::ContainerSetData(play::ContainerSetData { container: window, id: k as i16, value: value as i16 }));
    }
}

/// Stops sound event `sound` of the records for every guest (`ClientboundStopSoundPacket`).
pub fn stop_sound(hub: &mut Hub, sound: &str) {
    for g in &mut hub.guests {
        g.send(&C::StopSound(play::StopSound { source: Some(2), sound: Some(sound.to_owned()) }));
    }
}

/// The backpack menus open, a window a guest, for the storage's rule.
static OPEN: std::sync::Mutex<Vec<(u32, i32)>> = std::sync::Mutex::new(Vec::new());

/// Guest `conn`'s window `window` is a backpack's menu.
fn remember(conn: u32, window: i32) {
    if let Ok(mut open) = OPEN.lock() {
        open.retain(|&(c, _)| c != conn);
        open.push((conn, window));
    }
}

/// Whether guest `conn`'s window `window` is a backpack's menu.
pub fn is_open(conn: u32, window: i32) -> bool {
    OPEN.lock().is_ok_and(|open| open.contains(&(conn, window)))
}

/// What a shift-click into a backpack's storage respects
/// (`BackpackBaseMenu.moveItemStackTo`): its storage's size, the slots sorting
/// leaves alone, and the slots kept for an item (whether components match).
#[derive(Clone, Debug, Default)]
pub struct MoveRules {
    pub storage: usize,
    pub unsortable: Vec<usize>,
    pub memory: Vec<(usize, ItemStack, bool)>,
}

static RULES: std::sync::Mutex<Vec<(u32, i32, MoveRules)>> = std::sync::Mutex::new(Vec::new());

/// Guest `conn`'s backpack menu `window` moves by `rules` from now on.
pub fn set_rules(conn: u32, window: i32, rules: MoveRules) {
    if let Ok(mut all) = RULES.lock() {
        all.retain(|r| r.0 != conn);
        all.push((conn, window, rules));
    }
}

/// The rules of guest `conn`'s backpack menu `window`, if it is one.
pub fn move_rules(conn: u32, window: i32) -> Option<MoveRules> {
    let all = RULES.lock().ok()?;
    all.iter().find(|r| r.0 == conn && r.1 == window).map(|r| r.2.clone())
}
