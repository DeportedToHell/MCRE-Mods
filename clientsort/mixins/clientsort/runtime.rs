// ClientSort for Horizon: the mod's logic, with no game types, so the same
// file builds and tests on its own (`cargo test` at the repository root) and
// is what the bundle adds to the game as `crate::game::clientsort`.
//
// A port of ClientSort by TerminalMC (Apache-2.0,
// https://github.com/TerminalMC/ClientSort), which uses code from Mouse
// Wheelie by Siphalor (Apache-2.0) and Inventory Management by Roundaround
// (MIT). The button sprites are ClientSort's own.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::Mutex;

pub const ID: &str = "clientsort";
pub const NAME: &str = "ClientSort";
pub const VERSION: &str = "1.1.0";

/// No key: Options > Mods shows it unbound.
pub const UNBOUND: &str = "key.keyboard.unknown";

/// Russian labels when the game's language is Russian, English otherwise.
pub fn russian(language: &str) -> bool {
    language.starts_with("ru")
}

pub fn description(ru: bool) -> &'static str {
    if ru {
        "Универсальная и простая сортировка инвентаря."
    } else {
        "Versatile and easy inventory sorting."
    }
}

// ---- operations ----------------------------------------------------------

/// What a button or a key does to the inventory under it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Op {
    /// Sorts the inventory.
    Sort,
    /// Tops up the other inventory's partial stacks from this one.
    StackFill,
    /// Moves this inventory's items that the other one already holds
    /// (ClientSort's "Restock" or match transfer).
    MatchTransfer,
    /// Moves as much as fits into the other inventory.
    Transfer,
}

impl Op {
    pub const ALL: [Op; 4] = [Op::Sort, Op::StackFill, Op::MatchTransfer, Op::Transfer];

    /// Its key's name in Options > Mods (`key.<id>.<name>`).
    pub fn key(self) -> &'static str {
        match self {
            Op::Sort => "sort",
            Op::StackFill => "stack_fill",
            Op::MatchTransfer => "restock",
            Op::Transfer => "transfer",
        }
    }

    /// ClientSort's default keys: the middle mouse button sorts, the rest are
    /// unbound.
    pub fn default_key(self) -> &'static str {
        match self {
            Op::Sort => "key.mouse.middle",
            _ => UNBOUND,
        }
    }

    pub fn label(self, ru: bool) -> &'static str {
        match (self, ru) {
            (Op::Sort, true) => "Сортировать",
            (Op::StackFill, true) => "Заполнить стаки",
            (Op::MatchTransfer, true) => "Пополнить",
            (Op::Transfer, true) => "Переместить",
            (Op::Sort, false) => "Sort",
            (Op::StackFill, false) => "Fill Stacks",
            (Op::MatchTransfer, false) => "Restock",
            (Op::Transfer, false) => "Transfer",
        }
    }

    /// The operation needs another inventory to move items to.
    pub fn directional(self) -> bool {
        self != Op::Sort
    }
}

/// The slot groups ClientSort works on (its scopes): the open container, the
/// player's main inventory and the hotbar, each on its own.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Group {
    Container,
    Main,
    Hotbar,
}

// ---- settings ------------------------------------------------------------

/// The default sort order. Shift, Control and Alt pick ClientSort's others.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Order {
    /// The creative inventory's search tab order.
    Creative,
    /// By name.
    Alphabet,
    /// The most of an item first.
    Quantity,
    /// The item registry's order.
    RawId,
}

impl Order {
    pub const ALL: [Order; 4] = [Order::Creative, Order::Alphabet, Order::Quantity, Order::RawId];

    fn stored(self) -> &'static str {
        match self {
            Order::Creative => "creative",
            Order::Alphabet => "alphabet",
            Order::Quantity => "quantity",
            Order::RawId => "rawId",
        }
    }

    fn label(self, ru: bool) -> &'static str {
        match (self, ru) {
            (Order::Creative, true) => "как в творческом",
            (Order::Alphabet, true) => "по алфавиту",
            (Order::Quantity, true) => "по количеству",
            (Order::RawId, true) => "по ID предмета",
            (Order::Creative, false) => "Creative",
            (Order::Alphabet, false) => "Alphabetical",
            (Order::Quantity, false) => "Quantity",
            (Order::RawId, false) => "Item ID",
        }
    }

    /// ClientSort's modifier keys: Shift for quantity, Control for name, Alt
    /// for the item ID; none for the chosen default.
    pub fn with_modifiers(self, shift: bool, control: bool, alt: bool) -> Order {
        if shift {
            Order::Quantity
        } else if control {
            Order::Alphabet
        } else if alt {
            Order::RawId
        } else {
            self
        }
    }
}

/// The pause between two clicks, in ms; None for automatic: none in single
/// player, ClientSort's 10 ms on a server.
pub const SPEEDS: [Option<u32>; 5] = [None, Some(0), Some(10), Some(25), Some(50)];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Settings {
    /// The trigger buttons beside the inventories.
    pub buttons: bool,
    pub order: Order,
    /// An index into `SPEEDS`.
    pub speed: usize,
    /// The hotbar is a group of its own (ClientSort's default); off, the
    /// keys leave it alone.
    pub hotbar: bool,
    /// Restock matches tools, weapons and armour by item, ignoring damage and
    /// enchantments (ClientSort's type matching tags).
    pub match_type: bool,
    /// A button's name under the cursor (off, as ClientSort's default).
    pub tooltips: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            buttons: true,
            order: Order::Creative,
            speed: 0,
            hotbar: true,
            match_type: true,
            tooltips: false,
        }
    }
}

impl Settings {
    /// The settings kept in the mod's folder (`get` reads one key).
    pub fn read(get: impl Fn(&str) -> Option<String>) -> Settings {
        let mut s = Settings::default();
        let flag = |key: &str, default: bool| match get(key).as_deref() {
            Some("true") => true,
            Some("false") => false,
            _ => default,
        };
        s.buttons = flag("buttons", s.buttons);
        s.hotbar = flag("hotbar", s.hotbar);
        s.match_type = flag("match_type", s.match_type);
        s.tooltips = flag("tooltips", s.tooltips);
        if let Some(order) = get("order") {
            if let Some(o) = Order::ALL.iter().find(|o| o.stored() == order) {
                s.order = *o;
            }
        }
        if let Some(speed) = get("speed").and_then(|v| v.parse::<usize>().ok()) {
            if speed < SPEEDS.len() {
                s.speed = speed;
            }
        }
        s
    }

    /// What `read` reads back, key by key.
    pub fn stored(&self) -> Vec<(&'static str, String)> {
        vec![
            ("buttons", self.buttons.to_string()),
            ("order", self.order.stored().to_string()),
            ("speed", self.speed.to_string()),
            ("hotbar", self.hotbar.to_string()),
            ("match_type", self.match_type.to_string()),
            ("tooltips", self.tooltips.to_string()),
        ]
    }

    /// The settings page's buttons, in `click`'s order.
    pub fn labels(&self, ru: bool) -> Vec<String> {
        let on = |b: bool| match (b, ru) {
            (true, true) => "вкл",
            (false, true) => "выкл",
            (true, false) => "On",
            (false, false) => "Off",
        };
        let speed = match (SPEEDS[self.speed], ru) {
            (None, true) => "авто".to_string(),
            (None, false) => "Auto".to_string(),
            (Some(0), true) => "мгновенно".to_string(),
            (Some(0), false) => "Instant".to_string(),
            (Some(ms), true) => format!("{ms} мс"),
            (Some(ms), false) => format!("{ms} ms"),
        };
        if ru {
            vec![
                format!("Кнопки у инвентаря: {}", on(self.buttons)),
                format!("Порядок сортировки: {}", self.order.label(true)),
                format!("Пауза между кликами: {speed}"),
                format!("Хотбар отдельно: {}", on(self.hotbar)),
                format!("Пополнять инструменты и броню по типу: {}", on(self.match_type)),
                format!("Подсказки у кнопок: {}", on(self.tooltips)),
            ]
        } else {
            vec![
                format!("Buttons: {}", on(self.buttons)),
                format!("Sort order: {}", self.order.label(false)),
                format!("Click interval: {speed}"),
                format!("Hotbar as its own group: {}", on(self.hotbar)),
                format!("Restock tools and armour by type: {}", on(self.match_type)),
                format!("Button tooltips: {}", on(self.tooltips)),
            ]
        }
    }

    /// Button `button` of the settings page was clicked.
    pub fn click(&mut self, button: usize) {
        match button {
            0 => self.buttons = !self.buttons,
            1 => {
                let i = Order::ALL.iter().position(|o| *o == self.order).unwrap_or(0);
                self.order = Order::ALL[(i + 1) % Order::ALL.len()];
            }
            2 => self.speed = (self.speed + 1) % SPEEDS.len(),
            3 => self.hotbar = !self.hotbar,
            4 => self.match_type = !self.match_type,
            5 => self.tooltips = !self.tooltips,
            _ => {}
        }
    }

    /// The pause between clicks for this game, in ms.
    pub fn interval_ms(&self, singleplayer: bool) -> u32 {
        match SPEEDS[self.speed] {
            Some(ms) => ms,
            None if singleplayer => 0,
            None => 10,
        }
    }
}

static SETTINGS: Mutex<Option<Settings>> = Mutex::new(None);

/// The settings, read once with `get` and kept.
pub fn settings(get: impl Fn(&str) -> Option<String>) -> Settings {
    let mut kept = SETTINGS.lock().unwrap_or_else(|e| e.into_inner());
    *kept.get_or_insert_with(|| Settings::read(get))
}

pub fn set_settings(settings: Settings) {
    *SETTINGS.lock().unwrap_or_else(|e| e.into_inner()) = Some(settings);
}

// ---- the click planner -----------------------------------------------------

/// A stack as the planner sees it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Stack {
    /// Equal for stacks of the same item with the same components
    /// (`ItemStack.isSameItemSameComponents`).
    pub kind: u32,
    /// Equal for stacks of the same item.
    pub item: u32,
    pub count: u32,
    /// `ItemStack.getMaxStackSize`.
    pub max: u32,
    /// A bundle: a plain left click with or on it would open its contents.
    pub bundle: bool,
    /// A shulker box, which a shulker box's slots refuse.
    pub boxed: bool,
    /// Matched by item when restocking (tools, weapons and armour).
    pub by_type: bool,
}

/// One planned click, with the slot and the cursor as they must be before it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Step {
    pub slot: usize,
    pub button: i32,
    pub before: Option<Stack>,
    pub carried: Option<Stack>,
}

/// The slots of one operation and the clicks that change them, each click
/// played on the slots as vanilla's `AbstractContainerMenu.doClick` plays a
/// PICKUP click.
pub struct Plan {
    pub slots: Vec<Option<Stack>>,
    /// Slots that refuse shulker boxes (a shulker box's own).
    pub refuse_boxes: Vec<bool>,
    pub carried: Option<Stack>,
    pub steps: Vec<Step>,
    /// A click the planner should never make: the plan is not used.
    pub broken: bool,
}

impl Plan {
    pub fn new(slots: Vec<Option<Stack>>, refuse_boxes: Vec<bool>) -> Plan {
        Plan {
            slots,
            refuse_boxes,
            carried: None,
            steps: Vec::new(),
            broken: false,
        }
    }

    fn may_place(&self, s: usize, stack: &Stack) -> bool {
        !(stack.boxed && self.refuse_boxes.get(s).copied().unwrap_or(false))
    }

    /// A PICKUP click on slot `s` (button 0 left, 1 right), recorded and
    /// played on the slots.
    pub fn click(&mut self, s: usize, button: i32) {
        let before = self.slots[s];
        let carried = self.carried;
        self.steps.push(Step {
            slot: s,
            button,
            before,
            carried,
        });
        let primary = button == 0;
        // BundleItem's overrides: a left click with a bundle on an item, or
        // with an item on a bundle, moves items in or out of the bundle; a
        // right click of a bundle on nothing (or of nothing on a bundle)
        // takes one out. The planner never asks for those.
        let bundle_click = match (before, carried) {
            (Some(b), Some(c)) => primary && (b.bundle || c.bundle),
            (None, Some(c)) => !primary && c.bundle,
            (Some(b), None) => !primary && b.bundle,
            (None, None) => false,
        };
        if bundle_click {
            self.broken = true;
            return;
        }
        match (before, carried) {
            (None, None) => {}
            (None, Some(mut c)) => {
                if !self.may_place(s, &c) {
                    return;
                }
                let amount = if primary { c.count } else { 1 };
                let put = amount.min(c.max);
                if put == 0 {
                    return;
                }
                let mut placed = c;
                placed.count = put;
                self.slots[s] = Some(placed);
                c.count -= put;
                self.carried = (c.count > 0).then_some(c);
            }
            (Some(mut k), None) => {
                let take = if primary { k.count } else { k.count.div_ceil(2) };
                let mut taken = k;
                taken.count = take;
                k.count -= take;
                self.carried = Some(taken);
                self.slots[s] = (k.count > 0).then_some(k);
            }
            (Some(mut k), Some(mut c)) => {
                if self.may_place(s, &c) {
                    if k.kind == c.kind {
                        let amount = if primary { c.count } else { 1 };
                        let put = amount.min(k.max.saturating_sub(k.count));
                        k.count += put;
                        c.count -= put;
                        self.slots[s] = Some(k);
                        self.carried = (c.count > 0).then_some(c);
                    } else if c.count <= c.max {
                        self.slots[s] = Some(c);
                        self.carried = Some(k);
                    }
                } else if k.kind == c.kind {
                    let take = k.count.min(c.max.saturating_sub(c.count));
                    c.count += take;
                    k.count -= take;
                    self.carried = Some(c);
                    self.slots[s] = (k.count > 0).then_some(k);
                }
            }
        }
    }

    fn partial(&self, s: usize) -> Option<Stack> {
        self.slots[s].filter(|k| k.count < k.max)
    }

    /// ClientSort's collect: partial stacks of `o` merged into as few slots
    /// as they fit, the later ones into the earlier.
    pub fn collect(&mut self, o: &[usize]) {
        for i in (0..o.len()).rev() {
            let Some(src) = self.partial(o[i]) else {
                continue;
            };
            let found = o[..i]
                .iter()
                .any(|&d| self.partial(d).is_some_and(|k| k.kind == src.kind));
            if !found {
                continue;
            }
            self.click(o[i], 0);
            for &d in &o[..i] {
                if self.carried.is_none() || self.broken {
                    break;
                }
                if self.partial(d).is_some_and(|k| k.kind == src.kind) {
                    self.click(d, 0);
                }
            }
            if self.carried.is_some() {
                self.click(o[i], 0);
            }
        }
    }

    /// Moves the stacks of `o` so that position `i` holds the stack that was
    /// at position `order[i]`: each stack is picked up once and carried round
    /// its cycle, swapping as it goes (ClientSort's sort). Collect first, so
    /// that two stacks of a kind differ in count only when one is full.
    pub fn sort(&mut self, o: &[usize], order: &[usize]) {
        let n = o.len();
        if order.len() != n {
            self.broken = true;
            return;
        }
        let mut dst = vec![usize::MAX; n];
        for (pos, &src) in order.iter().enumerate() {
            if src >= n || dst[src] != usize::MAX {
                self.broken = true;
                return;
            }
            dst[src] = pos;
        }
        // The original position of the stack each position holds.
        let mut ident: Vec<Option<usize>> =
            (0..n).map(|p| self.slots[o[p]].map(|_| p)).collect();
        for p in 0..n {
            let Some(start) = ident[p] else {
                continue;
            };
            if dst[start] == p {
                continue;
            }
            self.click(o[p], 0);
            ident[p] = None;
            let work = p;
            let mut carried = start;
            let mut guard = 0;
            loop {
                guard += 1;
                if self.broken || guard > 4 * n + 4 {
                    self.broken = true;
                    return;
                }
                let d = dst[carried];
                let Some(other) = ident[d] else {
                    self.click(o[d], 0);
                    ident[d] = Some(carried);
                    break;
                };
                let (Some(held), Some(there)) = (self.carried, self.slots[o[d]]) else {
                    self.broken = true;
                    return;
                };
                if held.kind == there.kind && held.count == there.count {
                    // The same stack is already there: carry on with it.
                } else if held.kind != there.kind {
                    // A swap; a right click keeps a bundle closed.
                    let button = if held.bundle || there.bundle { 1 } else { 0 };
                    self.click(o[d], button);
                } else if held.count > there.count {
                    // A full stack on a partial one fills it, and the partial
                    // one's count stays in the cursor: a swap.
                    self.click(o[d], 0);
                } else {
                    // A partial stack on a full one does nothing: park it in
                    // the empty start slot, take the full one, fill the
                    // parked one up from it, put the rest down, take the
                    // full one back.
                    self.click(o[work], 0);
                    self.click(o[d], 0);
                    self.click(o[work], 0);
                    self.click(o[d], 0);
                    self.click(o[work], 0);
                }
                ident[d] = Some(carried);
                carried = other;
            }
        }
    }

    /// ClientSort's stack fill: the partial stacks of `other` topped up from
    /// `o`, taking from its last slots first.
    pub fn fill(&mut self, o: &[usize], other: &[usize]) {
        for i in (0..o.len()).rev() {
            let Some(src) = self.slots[o[i]] else {
                continue;
            };
            let found = other
                .iter()
                .any(|&d| self.partial(d).is_some_and(|k| k.kind == src.kind));
            if !found {
                continue;
            }
            self.click(o[i], 0);
            for &d in other {
                if self.carried.is_none() || self.broken {
                    break;
                }
                if self.partial(d).is_some_and(|k| k.kind == src.kind) {
                    self.click(d, 0);
                }
            }
            if self.carried.is_some() {
                self.click(o[i], 0);
            }
        }
    }

    /// ClientSort's transfer: each stack of `from` (a subset of the origin)
    /// tops up the partial stacks of `other`, then goes to its first empty
    /// slot, the last stacks first when `reversed`.
    pub fn transfer(&mut self, from: &[usize], other: &[usize], reversed: bool) {
        let order: Vec<usize> = if reversed {
            from.iter().rev().copied().collect()
        } else {
            from.to_vec()
        };
        for s in order {
            if self.broken {
                return;
            }
            let Some(src) = self.slots[s] else {
                continue;
            };
            let partial = other
                .iter()
                .any(|&d| self.partial(d).is_some_and(|k| k.kind == src.kind));
            let empty = other
                .iter()
                .any(|&d| self.slots[d].is_none() && self.may_place(d, &src));
            if !partial && !empty {
                continue;
            }
            self.click(s, 0);
            for &d in other {
                if self.carried.is_none() || self.broken {
                    break;
                }
                if self.partial(d).is_some_and(|k| k.kind == src.kind) {
                    self.click(d, 0);
                }
            }
            if let Some(held) = self.carried {
                if let Some(&d) = other
                    .iter()
                    .find(|&&d| self.slots[d].is_none() && self.may_place(d, &held))
                {
                    self.click(d, 0);
                }
            }
            if self.carried.is_some() {
                self.click(s, 0);
            }
        }
    }

    /// The slots of `o` whose stack `other` already has (ClientSort's match
    /// transfer): the same kind, or the same item for `by_type` items when
    /// `by_type` matching is on.
    pub fn matching(&self, o: &[usize], other: &[usize], by_type: bool) -> Vec<usize> {
        o.iter()
            .copied()
            .filter(|&s| {
                let Some(a) = self.slots[s] else {
                    return false;
                };
                other.iter().any(|&d| {
                    self.slots[d].is_some_and(|b| {
                        b.kind == a.kind || (by_type && a.by_type && b.item == a.item)
                    })
                })
            })
            .collect()
    }
}

// ---- sort orders -----------------------------------------------------------

/// What the sort orders compare of a kind of stack.
#[derive(Clone, Debug, Default)]
pub struct Key {
    /// Its place in the creative search tab (`u64::MAX`: not there).
    pub creative: u64,
    /// Its item's place in the item registry.
    pub registry: u64,
    /// Its name, lower case.
    pub name: String,
    /// It was renamed.
    pub custom: bool,
    pub damage: u32,
    /// What tells two kinds apart when everything else is equal.
    pub tie: String,
}

/// ClientSort's sort orders over the stacks of `o`: for each position, the
/// position whose stack goes there. Empty slots last; equal stacks keep their
/// order.
pub fn sort_order(slots: &[Option<Stack>], o: &[usize], keys: &[Key], order: Order) -> Vec<usize> {
    let mut totals: HashMap<u32, u64> = HashMap::new();
    for &s in o {
        if let Some(k) = slots[s] {
            *totals.entry(k.item).or_insert(0) += u64::from(k.count);
        }
    }
    let key = |k: &Stack| keys.get(k.kind as usize);
    let equal_items = |a: &Stack, b: &Stack, counts: bool| -> Ordering {
        let (ka, kb) = (key(a), key(b));
        let mut c = if counts {
            b.count.cmp(&a.count)
        } else {
            Ordering::Equal
        };
        if let (Some(ka), Some(kb)) = (ka, kb) {
            c = c
                .then_with(|| kb.custom.cmp(&ka.custom))
                .then_with(|| ka.name.cmp(&kb.name))
                .then_with(|| ka.damage.cmp(&kb.damage))
                .then_with(|| ka.tie.cmp(&kb.tie));
        }
        c.then_with(|| b.count.cmp(&a.count))
    };
    let mut positions: Vec<usize> = (0..o.len()).collect();
    positions.sort_by(|&pa, &pb| {
        match (slots[o[pa]], slots[o[pb]]) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Greater,
            (Some(_), None) => Ordering::Less,
            (Some(a), Some(b)) => {
                let (ka, kb) = (key(&a), key(&b));
                let first = match (order, ka, kb) {
                    (Order::Creative, Some(ka), Some(kb)) => ka.creative.cmp(&kb.creative),
                    (Order::RawId, Some(ka), Some(kb)) => ka.registry.cmp(&kb.registry),
                    (Order::Alphabet, Some(ka), Some(kb)) => ka.name.cmp(&kb.name),
                    (Order::Quantity, _, _) => totals
                        .get(&b.item)
                        .copied()
                        .unwrap_or(0)
                        .cmp(&totals.get(&a.item).copied().unwrap_or(0)),
                    _ => Ordering::Equal,
                };
                let same = a.kind == b.kind;
                first.then_with(|| equal_items(&a, &b, order != Order::Quantity || same))
            }
        }
    });
    positions
}

// ---- the trigger buttons ---------------------------------------------------

/// ClientSort's trigger buttons: 13 x 13, one under the other with a pixel
/// between them, 4 pixels over the panel's right edge, from the group's first
/// row of slots down (its default layout).
pub const BUTTON: f32 = 13.0;
pub const BUTTON_STEP: f32 = 14.0;
pub const BUTTON_OFFSET: f32 = -4.0;

/// Where button `index` of a group goes: `right` is the panel's right edge,
/// `top` the top of the group's first slot, both in GUI pixels.
pub fn button_rect(right: f32, top: f32, index: usize) -> [f32; 4] {
    [right + BUTTON_OFFSET, top + index as f32 * BUTTON_STEP, BUTTON, BUTTON]
}

pub fn inside(rect: [f32; 4], at: [f32; 2]) -> bool {
    at[0] >= rect[0] && at[0] < rect[0] + rect[2] && at[1] >= rect[1] && at[1] < rect[1] + rect[3]
}

/// The buttons a group shows: sort alone with no other inventory; sort,
/// restock and transfer beside a container. ClientSort shows the fill
/// button only where both sides show it, and hides it on the player's
/// inventory by default, so it shows nowhere (its key still works).
pub fn group_ops(other: bool) -> &'static [Op] {
    if other {
        &[Op::Sort, Op::MatchTransfer, Op::Transfer]
    } else {
        &[Op::Sort]
    }
}

/// The sprite of `op`'s button: its arrows point down on the container's
/// buttons and up on the player's.
pub fn sprite(op: Op, down: bool, hover: bool) -> &'static [(u8, u8, u8, u32)] {
    use sprites::*;
    match (op, down, hover) {
        (Op::Sort, _, false) => SORT,
        (Op::Sort, _, true) => SORT_HOVER,
        (Op::StackFill, true, false) => STACK_FILL_DOWN,
        (Op::StackFill, true, true) => STACK_FILL_DOWN_HOVER,
        (Op::StackFill, false, false) => STACK_FILL_UP,
        (Op::StackFill, false, true) => STACK_FILL_UP_HOVER,
        (Op::MatchTransfer, true, false) => MATCH_TRANSFER_DOWN,
        (Op::MatchTransfer, true, true) => MATCH_TRANSFER_DOWN_HOVER,
        (Op::MatchTransfer, false, false) => MATCH_TRANSFER_UP,
        (Op::MatchTransfer, false, true) => MATCH_TRANSFER_UP_HOVER,
        (Op::Transfer, true, false) => TRANSFER_DOWN,
        (Op::Transfer, true, true) => TRANSFER_DOWN_HOVER,
        (Op::Transfer, false, false) => TRANSFER_UP,
        (Op::Transfer, false, true) => TRANSFER_UP_HOVER,
    }
}

/// How many clicks to make now: with `interval` ms between clicks and
/// `budget` ms saved up (kept short of a burst after a slow frame).
pub fn clicks_due(budget: &mut f32, elapsed_ms: f32, interval: u32) -> usize {
    if interval == 0 {
        return usize::MAX;
    }
    let interval = interval as f32;
    *budget = (*budget + elapsed_ms).min(interval * 4.0);
    let n = (*budget / interval).floor();
    *budget -= n * interval;
    n as usize
}

/// The GLFW mouse button's key name, as Options > Mods keeps keys.
pub fn mouse_key(button: i32) -> String {
    match button {
        0 => "key.mouse.left".into(),
        1 => "key.mouse.right".into(),
        2 => "key.mouse.middle".into(),
        n => format!("key.mouse.{}", n + 1),
    }
}

pub mod sprites {
    // The 13 x 13 button sprites of ClientSort (Apache-2.0, TerminalMC), as
    // runs of one colour: (x, y, width, argb). Normal, then highlighted.
    pub const SORT: &[(u8, u8, u8, u32)] = &[(2, 0, 9, 0xFF000000), (1, 1, 1, 0xFF000000), (2, 1, 9, 0xFFFFFFFF), (11, 1, 1, 0xFF000000), (0, 2, 1, 0xFF000000), (1, 2, 1, 0xFFFFFFFF), (2, 2, 3, 0xFFC6C6C6), (5, 2, 1, 0xFF3F3F3F), (6, 2, 5, 0xFFC6C6C6), (11, 2, 1, 0xFF555555), (12, 2, 1, 0xFF000000), (0, 3, 1, 0xFF000000), (1, 3, 1, 0xFFFFFFFF), (2, 3, 2, 0xFFC6C6C6), (4, 3, 2, 0xFF3F3F3F), (6, 3, 5, 0xFFC6C6C6), (11, 3, 1, 0xFF555555), (12, 3, 1, 0xFF000000), (0, 4, 1, 0xFF000000), (1, 4, 1, 0xFFFFFFFF), (2, 4, 1, 0xFFC6C6C6), (3, 4, 7, 0xFF3F3F3F), (10, 4, 1, 0xFFC6C6C6), (11, 4, 1, 0xFF555555), (12, 4, 1, 0xFF000000), (0, 5, 1, 0xFF000000), (1, 5, 1, 0xFFFFFFFF), (2, 5, 2, 0xFFC6C6C6), (4, 5, 2, 0xFF3F3F3F), (6, 5, 5, 0xFFC6C6C6), (11, 5, 1, 0xFF555555), (12, 5, 1, 0xFF000000), (0, 6, 1, 0xFF000000), (1, 6, 1, 0xFFFFFFFF), (2, 6, 3, 0xFFC6C6C6), (5, 6, 1, 0xFF3F3F3F), (6, 6, 1, 0xFFC6C6C6), (7, 6, 1, 0xFF262626), (8, 6, 3, 0xFFC6C6C6), (11, 6, 1, 0xFF555555), (12, 6, 1, 0xFF000000), (0, 7, 1, 0xFF000000), (1, 7, 1, 0xFFFFFFFF), (2, 7, 5, 0xFFC6C6C6), (7, 7, 2, 0xFF262626), (9, 7, 2, 0xFFC6C6C6), (11, 7, 1, 0xFF555555), (12, 7, 1, 0xFF000000), (0, 8, 1, 0xFF000000), (1, 8, 1, 0xFFFFFFFF), (2, 8, 1, 0xFFC6C6C6), (3, 8, 7, 0xFF262626), (10, 8, 1, 0xFFC6C6C6), (11, 8, 1, 0xFF555555), (12, 8, 1, 0xFF000000), (0, 9, 1, 0xFF000000), (1, 9, 1, 0xFFFFFFFF), (2, 9, 5, 0xFFC6C6C6), (7, 9, 2, 0xFF262626), (9, 9, 2, 0xFFC6C6C6), (11, 9, 1, 0xFF555555), (12, 9, 1, 0xFF000000), (0, 10, 1, 0xFF000000), (1, 10, 1, 0xFFFFFFFF), (2, 10, 5, 0xFFC6C6C6), (7, 10, 1, 0xFF262626), (8, 10, 3, 0xFFC6C6C6), (11, 10, 1, 0xFF555555), (12, 10, 1, 0xFF000000), (1, 11, 1, 0xFF000000), (2, 11, 9, 0xFF555555), (11, 11, 1, 0xFF000000), (2, 12, 9, 0xFF000000)];
    pub const SORT_HOVER: &[(u8, u8, u8, u32)] = &[(2, 0, 9, 0xFF00073E), (1, 1, 1, 0xFF00073E), (2, 1, 9, 0xFFFFFFFF), (11, 1, 1, 0xFF00073E), (0, 2, 1, 0xFF00073E), (1, 2, 1, 0xFFFFFFFF), (2, 2, 3, 0xFF8892C9), (5, 2, 1, 0xFF3F3F3F), (6, 2, 5, 0xFF8892C9), (11, 2, 1, 0xFF343E75), (12, 2, 1, 0xFF00073E), (0, 3, 1, 0xFF00073E), (1, 3, 1, 0xFFFFFFFF), (2, 3, 2, 0xFF8892C9), (4, 3, 2, 0xFF3F3F3F), (6, 3, 5, 0xFF8892C9), (11, 3, 1, 0xFF343E75), (12, 3, 1, 0xFF00073E), (0, 4, 1, 0xFF00073E), (1, 4, 1, 0xFFFFFFFF), (2, 4, 1, 0xFF8892C9), (3, 4, 7, 0xFF3F3F3F), (10, 4, 1, 0xFF8892C9), (11, 4, 1, 0xFF343E75), (12, 4, 1, 0xFF00073E), (0, 5, 1, 0xFF00073E), (1, 5, 1, 0xFFFFFFFF), (2, 5, 2, 0xFF8892C9), (4, 5, 2, 0xFF3F3F3F), (6, 5, 5, 0xFF8892C9), (11, 5, 1, 0xFF343E75), (12, 5, 1, 0xFF00073E), (0, 6, 1, 0xFF00073E), (1, 6, 1, 0xFFFFFFFF), (2, 6, 3, 0xFF8892C9), (5, 6, 1, 0xFF3F3F3F), (6, 6, 1, 0xFF8892C9), (7, 6, 1, 0xFF262626), (8, 6, 3, 0xFF8892C9), (11, 6, 1, 0xFF343E75), (12, 6, 1, 0xFF00073E), (0, 7, 1, 0xFF00073E), (1, 7, 1, 0xFFFFFFFF), (2, 7, 5, 0xFF8892C9), (7, 7, 2, 0xFF262626), (9, 7, 2, 0xFF8892C9), (11, 7, 1, 0xFF343E75), (12, 7, 1, 0xFF00073E), (0, 8, 1, 0xFF00073E), (1, 8, 1, 0xFFFFFFFF), (2, 8, 1, 0xFF8892C9), (3, 8, 7, 0xFF262626), (10, 8, 1, 0xFF8892C9), (11, 8, 1, 0xFF343E75), (12, 8, 1, 0xFF00073E), (0, 9, 1, 0xFF00073E), (1, 9, 1, 0xFFFFFFFF), (2, 9, 5, 0xFF8892C9), (7, 9, 2, 0xFF262626), (9, 9, 2, 0xFF8892C9), (11, 9, 1, 0xFF343E75), (12, 9, 1, 0xFF00073E), (0, 10, 1, 0xFF00073E), (1, 10, 1, 0xFFFFFFFF), (2, 10, 5, 0xFF8892C9), (7, 10, 1, 0xFF262626), (8, 10, 3, 0xFF8892C9), (11, 10, 1, 0xFF343E75), (12, 10, 1, 0xFF00073E), (1, 11, 1, 0xFF00073E), (2, 11, 9, 0xFF343E75), (11, 11, 1, 0xFF00073E), (2, 12, 9, 0xFF00073E)];
    pub const STACK_FILL_DOWN: &[(u8, u8, u8, u32)] = &[(2, 0, 9, 0xFF000000), (1, 1, 1, 0xFF000000), (2, 1, 9, 0xFFFFFFFF), (11, 1, 1, 0xFF000000), (0, 2, 1, 0xFF000000), (1, 2, 1, 0xFFFFFFFF), (2, 2, 4, 0xFFC6C6C6), (6, 2, 1, 0xFF3F3F3F), (7, 2, 4, 0xFFC6C6C6), (11, 2, 1, 0xFF555555), (12, 2, 1, 0xFF000000), (0, 3, 1, 0xFF000000), (1, 3, 1, 0xFFFFFFFF), (2, 3, 4, 0xFFC6C6C6), (6, 3, 1, 0xFF3F3F3F), (7, 3, 4, 0xFFC6C6C6), (11, 3, 1, 0xFF555555), (12, 3, 1, 0xFF000000), (0, 4, 1, 0xFF000000), (1, 4, 1, 0xFFFFFFFF), (2, 4, 4, 0xFFC6C6C6), (6, 4, 1, 0xFF3F3F3F), (7, 4, 4, 0xFFC6C6C6), (11, 4, 1, 0xFF555555), (12, 4, 1, 0xFF000000), (0, 5, 1, 0xFF000000), (1, 5, 1, 0xFFFFFFFF), (2, 5, 4, 0xFFC6C6C6), (6, 5, 1, 0xFF3F3F3F), (7, 5, 4, 0xFFC6C6C6), (11, 5, 1, 0xFF555555), (12, 5, 1, 0xFF000000), (0, 6, 1, 0xFF000000), (1, 6, 1, 0xFFFFFFFF), (2, 6, 2, 0xFFC6C6C6), (4, 6, 5, 0xFF3F3F3F), (9, 6, 2, 0xFFC6C6C6), (11, 6, 1, 0xFF555555), (12, 6, 1, 0xFF000000), (0, 7, 1, 0xFF000000), (1, 7, 1, 0xFFFFFFFF), (2, 7, 3, 0xFFC6C6C6), (5, 7, 3, 0xFF3F3F3F), (8, 7, 3, 0xFFC6C6C6), (11, 7, 1, 0xFF555555), (12, 7, 1, 0xFF000000), (0, 8, 1, 0xFF000000), (1, 8, 1, 0xFFFFFFFF), (2, 8, 4, 0xFFC6C6C6), (6, 8, 1, 0xFF3F3F3F), (7, 8, 4, 0xFFC6C6C6), (11, 8, 1, 0xFF555555), (12, 8, 1, 0xFF000000), (0, 9, 1, 0xFF000000), (1, 9, 1, 0xFFFFFFFF), (2, 9, 2, 0xFFC6C6C6), (4, 9, 1, 0xFF262626), (5, 9, 3, 0xFFC6C6C6), (8, 9, 1, 0xFF262626), (9, 9, 2, 0xFFC6C6C6), (11, 9, 1, 0xFF555555), (12, 9, 1, 0xFF000000), (0, 10, 1, 0xFF000000), (1, 10, 1, 0xFFFFFFFF), (2, 10, 2, 0xFFC6C6C6), (4, 10, 5, 0xFF262626), (9, 10, 2, 0xFFC6C6C6), (11, 10, 1, 0xFF555555), (12, 10, 1, 0xFF000000), (1, 11, 1, 0xFF000000), (2, 11, 9, 0xFF555555), (11, 11, 1, 0xFF000000), (2, 12, 9, 0xFF000000)];
    pub const STACK_FILL_DOWN_HOVER: &[(u8, u8, u8, u32)] = &[(2, 0, 9, 0xFF00073E), (1, 1, 1, 0xFF00073E), (2, 1, 9, 0xFFFFFFFF), (11, 1, 1, 0xFF00073E), (0, 2, 1, 0xFF00073E), (1, 2, 1, 0xFFFFFFFF), (2, 2, 4, 0xFF8892C9), (6, 2, 1, 0xFF3F3F3F), (7, 2, 4, 0xFF8892C9), (11, 2, 1, 0xFF343E75), (12, 2, 1, 0xFF00073E), (0, 3, 1, 0xFF00073E), (1, 3, 1, 0xFFFFFFFF), (2, 3, 4, 0xFF8892C9), (6, 3, 1, 0xFF3F3F3F), (7, 3, 4, 0xFF8892C9), (11, 3, 1, 0xFF343E75), (12, 3, 1, 0xFF00073E), (0, 4, 1, 0xFF00073E), (1, 4, 1, 0xFFFFFFFF), (2, 4, 4, 0xFF8892C9), (6, 4, 1, 0xFF3F3F3F), (7, 4, 4, 0xFF8892C9), (11, 4, 1, 0xFF343E75), (12, 4, 1, 0xFF00073E), (0, 5, 1, 0xFF00073E), (1, 5, 1, 0xFFFFFFFF), (2, 5, 4, 0xFF8892C9), (6, 5, 1, 0xFF3F3F3F), (7, 5, 4, 0xFF8892C9), (11, 5, 1, 0xFF343E75), (12, 5, 1, 0xFF00073E), (0, 6, 1, 0xFF00073E), (1, 6, 1, 0xFFFFFFFF), (2, 6, 2, 0xFF8892C9), (4, 6, 5, 0xFF3F3F3F), (9, 6, 2, 0xFF8892C9), (11, 6, 1, 0xFF343E75), (12, 6, 1, 0xFF00073E), (0, 7, 1, 0xFF00073E), (1, 7, 1, 0xFFFFFFFF), (2, 7, 3, 0xFF8892C9), (5, 7, 3, 0xFF3F3F3F), (8, 7, 3, 0xFF8892C9), (11, 7, 1, 0xFF343E75), (12, 7, 1, 0xFF00073E), (0, 8, 1, 0xFF00073E), (1, 8, 1, 0xFFEDEDED), (2, 8, 4, 0xFF8892C9), (6, 8, 1, 0xFF3F3F3F), (7, 8, 4, 0xFF8892C9), (11, 8, 1, 0xFF343E75), (12, 8, 1, 0xFF00073E), (0, 9, 1, 0xFF00073E), (1, 9, 1, 0xFFFDFDFD), (2, 9, 2, 0xFF8892C9), (4, 9, 1, 0xFF262626), (5, 9, 3, 0xFF8892C9), (8, 9, 1, 0xFF262626), (9, 9, 2, 0xFF8892C9), (11, 9, 1, 0xFF343E75), (12, 9, 1, 0xFF00073E), (0, 10, 1, 0xFF00073E), (1, 10, 1, 0xFFFFFFFF), (2, 10, 2, 0xFF8892C9), (4, 10, 5, 0xFF262626), (9, 10, 2, 0xFF8892C9), (11, 10, 1, 0xFF343E75), (12, 10, 1, 0xFF00073E), (1, 11, 1, 0xFF00073E), (2, 11, 9, 0xFF343E75), (11, 11, 1, 0xFF00073E), (2, 12, 9, 0xFF00073E)];
    pub const STACK_FILL_UP: &[(u8, u8, u8, u32)] = &[(2, 0, 9, 0xFF000000), (1, 1, 1, 0xFF000000), (2, 1, 9, 0xFFFFFFFF), (11, 1, 1, 0xFF000000), (0, 2, 1, 0xFF000000), (1, 2, 1, 0xFFFFFFFF), (2, 2, 2, 0xFFC6C6C6), (4, 2, 5, 0xFF3F3F3F), (9, 2, 2, 0xFFC6C6C6), (11, 2, 1, 0xFF555555), (12, 2, 1, 0xFF000000), (0, 3, 1, 0xFF000000), (1, 3, 1, 0xFFFFFFFF), (2, 3, 2, 0xFFC6C6C6), (4, 3, 1, 0xFF3F3F3F), (5, 3, 3, 0xFFC6C6C6), (8, 3, 1, 0xFF3F3F3F), (9, 3, 2, 0xFFC6C6C6), (11, 3, 1, 0xFF555555), (12, 3, 1, 0xFF000000), (0, 4, 1, 0xFF000000), (1, 4, 1, 0xFFFFFFFF), (2, 4, 4, 0xFFC6C6C6), (6, 4, 1, 0xFF262626), (7, 4, 4, 0xFFC6C6C6), (11, 4, 1, 0xFF555555), (12, 4, 1, 0xFF000000), (0, 5, 1, 0xFF000000), (1, 5, 1, 0xFFFFFFFF), (2, 5, 3, 0xFFC6C6C6), (5, 5, 3, 0xFF262626), (8, 5, 3, 0xFFC6C6C6), (11, 5, 1, 0xFF555555), (12, 5, 1, 0xFF000000), (0, 6, 1, 0xFF000000), (1, 6, 1, 0xFFFFFFFF), (2, 6, 2, 0xFFC6C6C6), (4, 6, 5, 0xFF262626), (9, 6, 2, 0xFFC6C6C6), (11, 6, 1, 0xFF555555), (12, 6, 1, 0xFF000000), (0, 7, 1, 0xFF000000), (1, 7, 1, 0xFFFFFFFF), (2, 7, 4, 0xFFC6C6C6), (6, 7, 1, 0xFF262626), (7, 7, 4, 0xFFC6C6C6), (11, 7, 1, 0xFF555555), (12, 7, 1, 0xFF000000), (0, 8, 1, 0xFF000000), (1, 8, 1, 0xFFFFFFFF), (2, 8, 4, 0xFFC6C6C6), (6, 8, 1, 0xFF262626), (7, 8, 4, 0xFFC6C6C6), (11, 8, 1, 0xFF555555), (12, 8, 1, 0xFF000000), (0, 9, 1, 0xFF000000), (1, 9, 1, 0xFFFFFFFF), (2, 9, 4, 0xFFC6C6C6), (6, 9, 1, 0xFF262626), (7, 9, 4, 0xFFC6C6C6), (11, 9, 1, 0xFF555555), (12, 9, 1, 0xFF000000), (0, 10, 1, 0xFF000000), (1, 10, 1, 0xFFFFFFFF), (2, 10, 4, 0xFFC6C6C6), (6, 10, 1, 0xFF262626), (7, 10, 4, 0xFFC6C6C6), (11, 10, 1, 0xFF555555), (12, 10, 1, 0xFF000000), (1, 11, 1, 0xFF000000), (2, 11, 9, 0xFF555555), (11, 11, 1, 0xFF000000), (2, 12, 9, 0xFF000000)];
    pub const STACK_FILL_UP_HOVER: &[(u8, u8, u8, u32)] = &[(2, 0, 9, 0xFF00073E), (1, 1, 1, 0xFF00073E), (2, 1, 9, 0xFFFFFFFF), (11, 1, 1, 0xFF00073E), (0, 2, 1, 0xFF00073E), (1, 2, 1, 0xFFFFFFFF), (2, 2, 2, 0xFF8892C9), (4, 2, 5, 0xFF3F3F3F), (9, 2, 2, 0xFF8892C9), (11, 2, 1, 0xFF343E75), (12, 2, 1, 0xFF00073E), (0, 3, 1, 0xFF00073E), (1, 3, 1, 0xFFFFFFFF), (2, 3, 2, 0xFF8892C9), (4, 3, 1, 0xFF3F3F3F), (5, 3, 3, 0xFF8892C9), (8, 3, 1, 0xFF3F3F3F), (9, 3, 2, 0xFF8892C9), (11, 3, 1, 0xFF343E75), (12, 3, 1, 0xFF00073E), (0, 4, 1, 0xFF00073E), (1, 4, 1, 0xFFFFFFFF), (2, 4, 4, 0xFF8892C9), (6, 4, 1, 0xFF262626), (7, 4, 4, 0xFF8892C9), (11, 4, 1, 0xFF343E75), (12, 4, 1, 0xFF00073E), (0, 5, 1, 0xFF00073E), (1, 5, 1, 0xFFFFFFFF), (2, 5, 3, 0xFF8892C9), (5, 5, 3, 0xFF262626), (8, 5, 3, 0xFF8892C9), (11, 5, 1, 0xFF343E75), (12, 5, 1, 0xFF00073E), (0, 6, 1, 0xFF00073E), (1, 6, 1, 0xFFFFFFFF), (2, 6, 2, 0xFF8892C9), (4, 6, 5, 0xFF262626), (9, 6, 2, 0xFF8892C9), (11, 6, 1, 0xFF343E75), (12, 6, 1, 0xFF00073E), (0, 7, 1, 0xFF00073E), (1, 7, 1, 0xFFFFFFFF), (2, 7, 4, 0xFF8892C9), (6, 7, 1, 0xFF262626), (7, 7, 4, 0xFF8892C9), (11, 7, 1, 0xFF343E75), (12, 7, 1, 0xFF00073E), (0, 8, 1, 0xFF00073E), (1, 8, 1, 0xFFEDEDED), (2, 8, 4, 0xFF8892C9), (6, 8, 1, 0xFF262626), (7, 8, 4, 0xFF8892C9), (11, 8, 1, 0xFF343E75), (12, 8, 1, 0xFF00073E), (0, 9, 1, 0xFF00073E), (1, 9, 1, 0xFFFDFDFD), (2, 9, 4, 0xFF8892C9), (6, 9, 1, 0xFF262626), (7, 9, 4, 0xFF8892C9), (11, 9, 1, 0xFF343E75), (12, 9, 1, 0xFF00073E), (0, 10, 1, 0xFF00073E), (1, 10, 1, 0xFFFFFFFF), (2, 10, 4, 0xFF8892C9), (6, 10, 1, 0xFF262626), (7, 10, 4, 0xFF8892C9), (11, 10, 1, 0xFF343E75), (12, 10, 1, 0xFF00073E), (1, 11, 1, 0xFF00073E), (2, 11, 9, 0xFF343E75), (11, 11, 1, 0xFF00073E), (2, 12, 9, 0xFF00073E)];
    pub const MATCH_TRANSFER_DOWN: &[(u8, u8, u8, u32)] = &[(2, 0, 9, 0xFF000000), (1, 1, 1, 0xFF000000), (2, 1, 9, 0xFFFFFFFF), (11, 1, 1, 0xFF000000), (0, 2, 1, 0xFF000000), (1, 2, 1, 0xFFFFFFFF), (2, 2, 9, 0xFFC6C6C6), (11, 2, 1, 0xFF555555), (12, 2, 1, 0xFF000000), (0, 3, 1, 0xFF000000), (1, 3, 1, 0xFFFFFFFF), (2, 3, 5, 0xFFC6C6C6), (7, 3, 1, 0xFF262626), (8, 3, 3, 0xFFC6C6C6), (11, 3, 1, 0xFF555555), (12, 3, 1, 0xFF000000), (0, 4, 1, 0xFF000000), (1, 4, 1, 0xFFFFFFFF), (2, 4, 5, 0xFFC6C6C6), (7, 4, 1, 0xFF262626), (8, 4, 3, 0xFFC6C6C6), (11, 4, 1, 0xFF555555), (12, 4, 1, 0xFF000000), (0, 5, 1, 0xFF000000), (1, 5, 1, 0xFFFFFFFF), (2, 5, 1, 0xFFC6C6C6), (3, 5, 3, 0xFF3F3F3F), (6, 5, 1, 0xFFC6C6C6), (7, 5, 1, 0xFF262626), (8, 5, 3, 0xFFC6C6C6), (11, 5, 1, 0xFF555555), (12, 5, 1, 0xFF000000), (0, 6, 1, 0xFF000000), (1, 6, 1, 0xFFFFFFFF), (2, 6, 5, 0xFFC6C6C6), (7, 6, 1, 0xFF262626), (8, 6, 3, 0xFFC6C6C6), (11, 6, 1, 0xFF555555), (12, 6, 1, 0xFF000000), (0, 7, 1, 0xFF000000), (1, 7, 1, 0xFFFFFFFF), (2, 7, 1, 0xFFC6C6C6), (3, 7, 3, 0xFF3F3F3F), (6, 7, 1, 0xFFC6C6C6), (7, 7, 3, 0xFF262626), (10, 7, 1, 0xFFC6C6C6), (11, 7, 1, 0xFF555555), (12, 7, 1, 0xFF000000), (0, 8, 1, 0xFF000000), (1, 8, 1, 0xFFFFFFFF), (2, 8, 5, 0xFFC6C6C6), (7, 8, 2, 0xFF262626), (9, 8, 2, 0xFFC6C6C6), (11, 8, 1, 0xFF555555), (12, 8, 1, 0xFF000000), (0, 9, 1, 0xFF000000), (1, 9, 1, 0xFFFFFFFF), (2, 9, 5, 0xFFC6C6C6), (7, 9, 1, 0xFF262626), (8, 9, 3, 0xFFC6C6C6), (11, 9, 1, 0xFF555555), (12, 9, 1, 0xFF000000), (0, 10, 1, 0xFF000000), (1, 10, 1, 0xFFFFFFFF), (2, 10, 9, 0xFFC6C6C6), (11, 10, 1, 0xFF555555), (12, 10, 1, 0xFF000000), (1, 11, 1, 0xFF000000), (2, 11, 9, 0xFF555555), (11, 11, 1, 0xFF000000), (2, 12, 9, 0xFF000000)];
    pub const MATCH_TRANSFER_DOWN_HOVER: &[(u8, u8, u8, u32)] = &[(2, 0, 9, 0xFF00073E), (1, 1, 1, 0xFF00073E), (2, 1, 9, 0xFFFFFFFF), (11, 1, 1, 0xFF00073E), (0, 2, 1, 0xFF00073E), (1, 2, 1, 0xFFFFFFFF), (2, 2, 9, 0xFF8892C9), (11, 2, 1, 0xFF343E75), (12, 2, 1, 0xFF00073E), (0, 3, 1, 0xFF00073E), (1, 3, 1, 0xFFFFFFFF), (2, 3, 5, 0xFF8892C9), (7, 3, 1, 0xFF262626), (8, 3, 3, 0xFF8892C9), (11, 3, 1, 0xFF343E75), (12, 3, 1, 0xFF00073E), (0, 4, 1, 0xFF00073E), (1, 4, 1, 0xFFFFFFFF), (2, 4, 5, 0xFF8892C9), (7, 4, 1, 0xFF262626), (8, 4, 3, 0xFF8892C9), (11, 4, 1, 0xFF343E75), (12, 4, 1, 0xFF00073E), (0, 5, 1, 0xFF00073E), (1, 5, 1, 0xFFFFFFFF), (2, 5, 1, 0xFF8892C9), (3, 5, 3, 0xFF3F3F3F), (6, 5, 1, 0xFF8892C9), (7, 5, 1, 0xFF262626), (8, 5, 3, 0xFF8892C9), (11, 5, 1, 0xFF343E75), (12, 5, 1, 0xFF00073E), (0, 6, 1, 0xFF00073E), (1, 6, 1, 0xFFFFFFFF), (2, 6, 5, 0xFF8892C9), (7, 6, 1, 0xFF262626), (8, 6, 3, 0xFF8892C9), (11, 6, 1, 0xFF343E75), (12, 6, 1, 0xFF00073E), (0, 7, 1, 0xFF00073E), (1, 7, 1, 0xFFFFFFFF), (2, 7, 1, 0xFF8892C9), (3, 7, 3, 0xFF3F3F3F), (6, 7, 1, 0xFF8892C9), (7, 7, 3, 0xFF262626), (10, 7, 1, 0xFF8892C9), (11, 7, 1, 0xFF343E75), (12, 7, 1, 0xFF00073E), (0, 8, 1, 0xFF00073E), (1, 8, 1, 0xFFFFFFFF), (2, 8, 5, 0xFF8892C9), (7, 8, 2, 0xFF262626), (9, 8, 2, 0xFF8892C9), (11, 8, 1, 0xFF343E75), (12, 8, 1, 0xFF00073E), (0, 9, 1, 0xFF00073E), (1, 9, 1, 0xFFFFFFFF), (2, 9, 5, 0xFF8892C9), (7, 9, 1, 0xFF262626), (8, 9, 3, 0xFF8892C9), (11, 9, 1, 0xFF343E75), (12, 9, 1, 0xFF00073E), (0, 10, 1, 0xFF00073E), (1, 10, 1, 0xFFFFFFFF), (2, 10, 9, 0xFF8892C9), (11, 10, 1, 0xFF343E75), (12, 10, 1, 0xFF00073E), (1, 11, 1, 0xFF00073E), (2, 11, 9, 0xFF343E75), (11, 11, 1, 0xFF00073E), (2, 12, 9, 0xFF00073E)];
    pub const MATCH_TRANSFER_UP: &[(u8, u8, u8, u32)] = &[(2, 0, 9, 0xFF000000), (1, 1, 1, 0xFF000000), (2, 1, 9, 0xFFFFFFFF), (11, 1, 1, 0xFF000000), (0, 2, 1, 0xFF000000), (1, 2, 1, 0xFFFFFFFF), (2, 2, 9, 0xFFC6C6C6), (11, 2, 1, 0xFF555555), (12, 2, 1, 0xFF000000), (0, 3, 1, 0xFF000000), (1, 3, 1, 0xFFFFFFFF), (2, 3, 5, 0xFFC6C6C6), (7, 3, 1, 0xFF262626), (8, 3, 3, 0xFFC6C6C6), (11, 3, 1, 0xFF555555), (12, 3, 1, 0xFF000000), (0, 4, 1, 0xFF000000), (1, 4, 1, 0xFFFFFFFF), (2, 4, 5, 0xFFC6C6C6), (7, 4, 2, 0xFF262626), (9, 4, 2, 0xFFC6C6C6), (11, 4, 1, 0xFF555555), (12, 4, 1, 0xFF000000), (0, 5, 1, 0xFF000000), (1, 5, 1, 0xFFFFFFFF), (2, 5, 1, 0xFFC6C6C6), (3, 5, 3, 0xFF3F3F3F), (6, 5, 1, 0xFFC6C6C6), (7, 5, 3, 0xFF262626), (10, 5, 1, 0xFFC6C6C6), (11, 5, 1, 0xFF555555), (12, 5, 1, 0xFF000000), (0, 6, 1, 0xFF000000), (1, 6, 1, 0xFFFFFFFF), (2, 6, 5, 0xFFC6C6C6), (7, 6, 1, 0xFF262626), (8, 6, 3, 0xFFC6C6C6), (11, 6, 1, 0xFF555555), (12, 6, 1, 0xFF000000), (0, 7, 1, 0xFF000000), (1, 7, 1, 0xFFFFFFFF), (2, 7, 1, 0xFFC6C6C6), (3, 7, 3, 0xFF3F3F3F), (6, 7, 1, 0xFFC6C6C6), (7, 7, 1, 0xFF262626), (8, 7, 3, 0xFFC6C6C6), (11, 7, 1, 0xFF555555), (12, 7, 1, 0xFF000000), (0, 8, 1, 0xFF000000), (1, 8, 1, 0xFFFFFFFF), (2, 8, 5, 0xFFC6C6C6), (7, 8, 1, 0xFF262626), (8, 8, 3, 0xFFC6C6C6), (11, 8, 1, 0xFF555555), (12, 8, 1, 0xFF000000), (0, 9, 1, 0xFF000000), (1, 9, 1, 0xFFFFFFFF), (2, 9, 5, 0xFFC6C6C6), (7, 9, 1, 0xFF262626), (8, 9, 3, 0xFFC6C6C6), (11, 9, 1, 0xFF555555), (12, 9, 1, 0xFF000000), (0, 10, 1, 0xFF000000), (1, 10, 1, 0xFFFFFFFF), (2, 10, 9, 0xFFC6C6C6), (11, 10, 1, 0xFF555555), (12, 10, 1, 0xFF000000), (1, 11, 1, 0xFF000000), (2, 11, 9, 0xFF555555), (11, 11, 1, 0xFF000000), (2, 12, 9, 0xFF000000)];
    pub const MATCH_TRANSFER_UP_HOVER: &[(u8, u8, u8, u32)] = &[(2, 0, 9, 0xFF00073E), (1, 1, 1, 0xFF00073E), (2, 1, 9, 0xFFFFFFFF), (11, 1, 1, 0xFF00073E), (0, 2, 1, 0xFF00073E), (1, 2, 1, 0xFFFFFFFF), (2, 2, 9, 0xFF8892C9), (11, 2, 1, 0xFF343E75), (12, 2, 1, 0xFF00073E), (0, 3, 1, 0xFF00073E), (1, 3, 1, 0xFFFFFFFF), (2, 3, 5, 0xFF8892C9), (7, 3, 1, 0xFF262626), (8, 3, 3, 0xFF8892C9), (11, 3, 1, 0xFF343E75), (12, 3, 1, 0xFF00073E), (0, 4, 1, 0xFF00073E), (1, 4, 1, 0xFFFFFFFF), (2, 4, 5, 0xFF8892C9), (7, 4, 2, 0xFF262626), (9, 4, 2, 0xFF8892C9), (11, 4, 1, 0xFF343E75), (12, 4, 1, 0xFF00073E), (0, 5, 1, 0xFF00073E), (1, 5, 1, 0xFFFFFFFF), (2, 5, 1, 0xFF8892C9), (3, 5, 3, 0xFF3F3F3F), (6, 5, 1, 0xFF8892C9), (7, 5, 3, 0xFF262626), (10, 5, 1, 0xFF8892C9), (11, 5, 1, 0xFF343E75), (12, 5, 1, 0xFF00073E), (0, 6, 1, 0xFF00073E), (1, 6, 1, 0xFFFFFFFF), (2, 6, 5, 0xFF8892C9), (7, 6, 1, 0xFF262626), (8, 6, 3, 0xFF8892C9), (11, 6, 1, 0xFF343E75), (12, 6, 1, 0xFF00073E), (0, 7, 1, 0xFF00073E), (1, 7, 1, 0xFFFFFFFF), (2, 7, 1, 0xFF8892C9), (3, 7, 3, 0xFF3F3F3F), (6, 7, 1, 0xFF8892C9), (7, 7, 1, 0xFF262626), (8, 7, 3, 0xFF8892C9), (11, 7, 1, 0xFF343E75), (12, 7, 1, 0xFF00073E), (0, 8, 1, 0xFF00073E), (1, 8, 1, 0xFFFFFFFF), (2, 8, 5, 0xFF8892C9), (7, 8, 1, 0xFF262626), (8, 8, 3, 0xFF8892C9), (11, 8, 1, 0xFF343E75), (12, 8, 1, 0xFF00073E), (0, 9, 1, 0xFF00073E), (1, 9, 1, 0xFFFFFFFF), (2, 9, 5, 0xFF8892C9), (7, 9, 1, 0xFF262626), (8, 9, 3, 0xFF8892C9), (11, 9, 1, 0xFF343E75), (12, 9, 1, 0xFF00073E), (0, 10, 1, 0xFF00073E), (1, 10, 1, 0xFFFFFFFF), (2, 10, 9, 0xFF8892C9), (11, 10, 1, 0xFF343E75), (12, 10, 1, 0xFF00073E), (1, 11, 1, 0xFF00073E), (2, 11, 9, 0xFF343E75), (11, 11, 1, 0xFF00073E), (2, 12, 9, 0xFF00073E)];
    pub const TRANSFER_DOWN: &[(u8, u8, u8, u32)] = &[(2, 0, 9, 0xFF000000), (1, 1, 1, 0xFF000000), (2, 1, 9, 0xFFFFFFFF), (11, 1, 1, 0xFF000000), (0, 2, 1, 0xFF000000), (1, 2, 1, 0xFFFFFFFF), (2, 2, 9, 0xFFC6C6C6), (11, 2, 1, 0xFF555555), (12, 2, 1, 0xFF000000), (0, 3, 1, 0xFF000000), (1, 3, 1, 0xFFFFFFFF), (2, 3, 1, 0xFFC6C6C6), (3, 3, 1, 0xFF3F3F3F), (4, 3, 3, 0xFFC6C6C6), (7, 3, 1, 0xFF262626), (8, 3, 3, 0xFFC6C6C6), (11, 3, 1, 0xFF555555), (12, 3, 1, 0xFF000000), (0, 4, 1, 0xFF000000), (1, 4, 1, 0xFFFFFFFF), (2, 4, 1, 0xFFC6C6C6), (3, 4, 1, 0xFF3F3F3F), (4, 4, 3, 0xFFC6C6C6), (7, 4, 1, 0xFF262626), (8, 4, 3, 0xFFC6C6C6), (11, 4, 1, 0xFF555555), (12, 4, 1, 0xFF000000), (0, 5, 1, 0xFF000000), (1, 5, 1, 0xFFFFFFFF), (2, 5, 1, 0xFFC6C6C6), (3, 5, 1, 0xFF3F3F3F), (4, 5, 3, 0xFFC6C6C6), (7, 5, 1, 0xFF262626), (8, 5, 3, 0xFFC6C6C6), (11, 5, 1, 0xFF555555), (12, 5, 1, 0xFF000000), (0, 6, 1, 0xFF000000), (1, 6, 1, 0xFFFFFFFF), (2, 6, 1, 0xFFC6C6C6), (3, 6, 1, 0xFF3F3F3F), (4, 6, 3, 0xFFC6C6C6), (7, 6, 1, 0xFF262626), (8, 6, 3, 0xFFC6C6C6), (11, 6, 1, 0xFF555555), (12, 6, 1, 0xFF000000), (0, 7, 1, 0xFF000000), (1, 7, 1, 0xFFFFFFFF), (2, 7, 1, 0xFFC6C6C6), (3, 7, 3, 0xFF3F3F3F), (6, 7, 1, 0xFFC6C6C6), (7, 7, 3, 0xFF262626), (10, 7, 1, 0xFFC6C6C6), (11, 7, 1, 0xFF555555), (12, 7, 1, 0xFF000000), (0, 8, 1, 0xFF000000), (1, 8, 1, 0xFFFFFFFF), (2, 8, 1, 0xFFC6C6C6), (3, 8, 2, 0xFF3F3F3F), (5, 8, 2, 0xFFC6C6C6), (7, 8, 2, 0xFF262626), (9, 8, 2, 0xFFC6C6C6), (11, 8, 1, 0xFF555555), (12, 8, 1, 0xFF000000), (0, 9, 1, 0xFF000000), (1, 9, 1, 0xFFFFFFFF), (2, 9, 1, 0xFFC6C6C6), (3, 9, 1, 0xFF3F3F3F), (4, 9, 3, 0xFFC6C6C6), (7, 9, 1, 0xFF262626), (8, 9, 3, 0xFFC6C6C6), (11, 9, 1, 0xFF555555), (12, 9, 1, 0xFF000000), (0, 10, 1, 0xFF000000), (1, 10, 1, 0xFFFFFFFF), (2, 10, 9, 0xFFC6C6C6), (11, 10, 1, 0xFF555555), (12, 10, 1, 0xFF000000), (1, 11, 1, 0xFF000000), (2, 11, 9, 0xFF555555), (11, 11, 1, 0xFF000000), (2, 12, 9, 0xFF000000)];
    pub const TRANSFER_DOWN_HOVER: &[(u8, u8, u8, u32)] = &[(2, 0, 9, 0xFF00073E), (1, 1, 1, 0xFF00073E), (2, 1, 9, 0xFFFFFFFF), (11, 1, 1, 0xFF00073E), (0, 2, 1, 0xFF00073E), (1, 2, 1, 0xFFFFFFFF), (2, 2, 9, 0xFF8892C9), (11, 2, 1, 0xFF343E75), (12, 2, 1, 0xFF00073E), (0, 3, 1, 0xFF00073E), (1, 3, 1, 0xFFFFFFFF), (2, 3, 1, 0xFF8892C9), (3, 3, 1, 0xFF3F3F3F), (4, 3, 3, 0xFF8892C9), (7, 3, 1, 0xFF262626), (8, 3, 3, 0xFF8892C9), (11, 3, 1, 0xFF343E75), (12, 3, 1, 0xFF00073E), (0, 4, 1, 0xFF00073E), (1, 4, 1, 0xFFFFFFFF), (2, 4, 1, 0xFF8892C9), (3, 4, 1, 0xFF3F3F3F), (4, 4, 3, 0xFF8892C9), (7, 4, 1, 0xFF262626), (8, 4, 3, 0xFF8892C9), (11, 4, 1, 0xFF343E75), (12, 4, 1, 0xFF00073E), (0, 5, 1, 0xFF00073E), (1, 5, 1, 0xFFFFFFFF), (2, 5, 1, 0xFF8892C9), (3, 5, 1, 0xFF3F3F3F), (4, 5, 3, 0xFF8892C9), (7, 5, 1, 0xFF262626), (8, 5, 3, 0xFF8892C9), (11, 5, 1, 0xFF343E75), (12, 5, 1, 0xFF00073E), (0, 6, 1, 0xFF00073E), (1, 6, 1, 0xFFFFFFFF), (2, 6, 1, 0xFF8892C9), (3, 6, 1, 0xFF3F3F3F), (4, 6, 3, 0xFF8892C9), (7, 6, 1, 0xFF262626), (8, 6, 3, 0xFF8892C9), (11, 6, 1, 0xFF343E75), (12, 6, 1, 0xFF00073E), (0, 7, 1, 0xFF00073E), (1, 7, 1, 0xFFFFFFFF), (2, 7, 1, 0xFF8892C9), (3, 7, 3, 0xFF3F3F3F), (6, 7, 1, 0xFF8892C9), (7, 7, 3, 0xFF262626), (10, 7, 1, 0xFF8892C9), (11, 7, 1, 0xFF343E75), (12, 7, 1, 0xFF00073E), (0, 8, 1, 0xFF00073E), (1, 8, 1, 0xFFFFFFFF), (2, 8, 1, 0xFF8892C9), (3, 8, 2, 0xFF3F3F3F), (5, 8, 2, 0xFF8892C9), (7, 8, 2, 0xFF262626), (9, 8, 2, 0xFF8892C9), (11, 8, 1, 0xFF343E75), (12, 8, 1, 0xFF00073E), (0, 9, 1, 0xFF00073E), (1, 9, 1, 0xFFFFFFFF), (2, 9, 1, 0xFF8892C9), (3, 9, 1, 0xFF3F3F3F), (4, 9, 3, 0xFF8892C9), (7, 9, 1, 0xFF262626), (8, 9, 3, 0xFF8892C9), (11, 9, 1, 0xFF343E75), (12, 9, 1, 0xFF00073E), (0, 10, 1, 0xFF00073E), (1, 10, 1, 0xFFFFFFFF), (2, 10, 9, 0xFF8892C9), (11, 10, 1, 0xFF343E75), (12, 10, 1, 0xFF00073E), (1, 11, 1, 0xFF00073E), (2, 11, 9, 0xFF343E75), (11, 11, 1, 0xFF00073E), (2, 12, 9, 0xFF00073E)];
    pub const TRANSFER_UP: &[(u8, u8, u8, u32)] = &[(2, 0, 9, 0xFF000000), (1, 1, 1, 0xFF000000), (2, 1, 9, 0xFFFFFFFF), (11, 1, 1, 0xFF000000), (0, 2, 1, 0xFF000000), (1, 2, 1, 0xFFFFFFFF), (2, 2, 9, 0xFFC6C6C6), (11, 2, 1, 0xFF555555), (12, 2, 1, 0xFF000000), (0, 3, 1, 0xFF000000), (1, 3, 1, 0xFFFFFFFF), (2, 3, 1, 0xFFC6C6C6), (3, 3, 1, 0xFF3F3F3F), (4, 3, 3, 0xFFC6C6C6), (7, 3, 1, 0xFF262626), (8, 3, 3, 0xFFC6C6C6), (11, 3, 1, 0xFF555555), (12, 3, 1, 0xFF000000), (0, 4, 1, 0xFF000000), (1, 4, 1, 0xFFFFFFFF), (2, 4, 1, 0xFFC6C6C6), (3, 4, 2, 0xFF3F3F3F), (5, 4, 2, 0xFFC6C6C6), (7, 4, 2, 0xFF262626), (9, 4, 2, 0xFFC6C6C6), (11, 4, 1, 0xFF555555), (12, 4, 1, 0xFF000000), (0, 5, 1, 0xFF000000), (1, 5, 1, 0xFFFFFFFF), (2, 5, 1, 0xFFC6C6C6), (3, 5, 3, 0xFF3F3F3F), (6, 5, 1, 0xFFC6C6C6), (7, 5, 3, 0xFF262626), (10, 5, 1, 0xFFC6C6C6), (11, 5, 1, 0xFF555555), (12, 5, 1, 0xFF000000), (0, 6, 1, 0xFF000000), (1, 6, 1, 0xFFFFFFFF), (2, 6, 1, 0xFFC6C6C6), (3, 6, 1, 0xFF3F3F3F), (4, 6, 3, 0xFFC6C6C6), (7, 6, 1, 0xFF262626), (8, 6, 3, 0xFFC6C6C6), (11, 6, 1, 0xFF555555), (12, 6, 1, 0xFF000000), (0, 7, 1, 0xFF000000), (1, 7, 1, 0xFFFFFFFF), (2, 7, 1, 0xFFC6C6C6), (3, 7, 1, 0xFF3F3F3F), (4, 7, 3, 0xFFC6C6C6), (7, 7, 1, 0xFF262626), (8, 7, 3, 0xFFC6C6C6), (11, 7, 1, 0xFF555555), (12, 7, 1, 0xFF000000), (0, 8, 1, 0xFF000000), (1, 8, 1, 0xFFFFFFFF), (2, 8, 1, 0xFFC6C6C6), (3, 8, 1, 0xFF3F3F3F), (4, 8, 3, 0xFFC6C6C6), (7, 8, 1, 0xFF262626), (8, 8, 3, 0xFFC6C6C6), (11, 8, 1, 0xFF555555), (12, 8, 1, 0xFF000000), (0, 9, 1, 0xFF000000), (1, 9, 1, 0xFFFFFFFF), (2, 9, 1, 0xFFC6C6C6), (3, 9, 1, 0xFF3F3F3F), (4, 9, 3, 0xFFC6C6C6), (7, 9, 1, 0xFF262626), (8, 9, 3, 0xFFC6C6C6), (11, 9, 1, 0xFF555555), (12, 9, 1, 0xFF000000), (0, 10, 1, 0xFF000000), (1, 10, 1, 0xFFFFFFFF), (2, 10, 9, 0xFFC6C6C6), (11, 10, 1, 0xFF555555), (12, 10, 1, 0xFF000000), (1, 11, 1, 0xFF000000), (2, 11, 9, 0xFF555555), (11, 11, 1, 0xFF000000), (2, 12, 9, 0xFF000000)];
    pub const TRANSFER_UP_HOVER: &[(u8, u8, u8, u32)] = &[(2, 0, 9, 0xFF00073E), (1, 1, 1, 0xFF00073E), (2, 1, 9, 0xFFFFFFFF), (11, 1, 1, 0xFF00073E), (0, 2, 1, 0xFF00073E), (1, 2, 1, 0xFFFFFFFF), (2, 2, 9, 0xFF8892C9), (11, 2, 1, 0xFF343E75), (12, 2, 1, 0xFF00073E), (0, 3, 1, 0xFF00073E), (1, 3, 1, 0xFFFFFFFF), (2, 3, 1, 0xFF8892C9), (3, 3, 1, 0xFF3F3F3F), (4, 3, 3, 0xFF8892C9), (7, 3, 1, 0xFF262626), (8, 3, 3, 0xFF8892C9), (11, 3, 1, 0xFF343E75), (12, 3, 1, 0xFF00073E), (0, 4, 1, 0xFF00073E), (1, 4, 1, 0xFFFFFFFF), (2, 4, 1, 0xFF8892C9), (3, 4, 2, 0xFF3F3F3F), (5, 4, 2, 0xFF8892C9), (7, 4, 2, 0xFF262626), (9, 4, 2, 0xFF8892C9), (11, 4, 1, 0xFF343E75), (12, 4, 1, 0xFF00073E), (0, 5, 1, 0xFF00073E), (1, 5, 1, 0xFFFFFFFF), (2, 5, 1, 0xFF8892C9), (3, 5, 3, 0xFF3F3F3F), (6, 5, 1, 0xFF8892C9), (7, 5, 3, 0xFF262626), (10, 5, 1, 0xFF8892C9), (11, 5, 1, 0xFF343E75), (12, 5, 1, 0xFF00073E), (0, 6, 1, 0xFF00073E), (1, 6, 1, 0xFFFFFFFF), (2, 6, 1, 0xFF8892C9), (3, 6, 1, 0xFF3F3F3F), (4, 6, 3, 0xFF8892C9), (7, 6, 1, 0xFF262626), (8, 6, 3, 0xFF8892C9), (11, 6, 1, 0xFF343E75), (12, 6, 1, 0xFF00073E), (0, 7, 1, 0xFF00073E), (1, 7, 1, 0xFFFFFFFF), (2, 7, 1, 0xFF8892C9), (3, 7, 1, 0xFF3F3F3F), (4, 7, 3, 0xFF8892C9), (7, 7, 1, 0xFF262626), (8, 7, 3, 0xFF8892C9), (11, 7, 1, 0xFF343E75), (12, 7, 1, 0xFF00073E), (0, 8, 1, 0xFF00073E), (1, 8, 1, 0xFFFFFFFF), (2, 8, 1, 0xFF8892C9), (3, 8, 1, 0xFF3F3F3F), (4, 8, 3, 0xFF8892C9), (7, 8, 1, 0xFF262626), (8, 8, 3, 0xFF8892C9), (11, 8, 1, 0xFF343E75), (12, 8, 1, 0xFF00073E), (0, 9, 1, 0xFF00073E), (1, 9, 1, 0xFFFFFFFF), (2, 9, 1, 0xFF8892C9), (3, 9, 1, 0xFF3F3F3F), (4, 9, 3, 0xFF8892C9), (7, 9, 1, 0xFF262626), (8, 9, 3, 0xFF8892C9), (11, 9, 1, 0xFF343E75), (12, 9, 1, 0xFF00073E), (0, 10, 1, 0xFF00073E), (1, 10, 1, 0xFFFFFFFF), (2, 10, 9, 0xFF8892C9), (11, 10, 1, 0xFF343E75), (12, 10, 1, 0xFF00073E), (1, 11, 1, 0xFF00073E), (2, 11, 9, 0xFF343E75), (11, 11, 1, 0xFF00073E), (2, 12, 9, 0xFF00073E)];
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
        fn below(&mut self, n: u64) -> u64 {
            self.next() % n
        }
    }

    // Kinds 0..8: stackable to 64, 16 or 1, a bundle (kind 6) and a shulker
    // box (kind 7); kinds 4 and 5 are the same item (a sword, matched by type).
    fn kind(k: u32) -> Stack {
        let max = match k {
            0..=2 => 64,
            3 => 16,
            _ => 1,
        };
        Stack {
            kind: k,
            item: if k == 5 { 4 } else { k },
            count: 1,
            max,
            bundle: k == 6,
            boxed: k == 7,
            by_type: k == 4 || k == 5,
        }
    }

    fn random_slots(rng: &mut Rng, n: usize, fill: u64) -> Vec<Option<Stack>> {
        (0..n)
            .map(|_| {
                if rng.below(100) >= fill {
                    return None;
                }
                let mut s = kind(rng.below(8) as u32);
                s.count = 1 + rng.below(u64::from(s.max)) as u32;
                Some(s)
            })
            .collect()
    }

    fn keys() -> Vec<Key> {
        (0..8)
            .map(|k| Key {
                creative: 7 - k as u64,
                registry: k as u64,
                name: format!("item{k}"),
                custom: false,
                damage: 0,
                tie: String::new(),
            })
            .collect()
    }

    fn totals(slots: &[Option<Stack>], positions: &[usize]) -> HashMap<u32, u32> {
        let mut t = HashMap::new();
        for &p in positions {
            if let Some(s) = slots[p] {
                *t.entry(s.kind).or_insert(0) += s.count;
            }
        }
        t
    }

    /// Replays the steps on a fresh plan: each recorded state is the one the
    /// clicks before it left, as the game's checks expect.
    fn replays(start: &[Option<Stack>], refuse: &[bool], steps: &[Step]) {
        let mut p = Plan::new(start.to_vec(), refuse.to_vec());
        for step in steps {
            assert_eq!(p.slots[step.slot], step.before);
            assert_eq!(p.carried, step.carried);
            p.click(step.slot, step.button);
        }
        assert!(p.carried.is_none());
    }

    #[test]
    fn sorting_reaches_the_order_and_keeps_every_item() {
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
        for round in 0..4000 {
            let n = [9, 27, 54][round % 3];
            let fill = [30, 70, 100][(round / 3) % 3];
            let start = random_slots(&mut rng, n, fill);
            let o: Vec<usize> = (0..n).collect();
            let order = Order::ALL[round % 4];
            let mut plan = Plan::new(start.clone(), vec![false; n]);
            plan.collect(&o);
            let collected = plan.slots.clone();
            let target = sort_order(&collected, &o, &keys(), order);
            plan.sort(&o, &target);
            assert!(!plan.broken, "round {round}");
            assert!(plan.carried.is_none());
            for (i, &src) in target.iter().enumerate() {
                let (a, b) = (plan.slots[i], collected[src]);
                assert_eq!(a.map(|s| (s.kind, s.count)), b.map(|s| (s.kind, s.count)), "round {round} slot {i}");
            }
            assert_eq!(totals(&start, &o), totals(&plan.slots, &o));
            replays(&start, &vec![false; n], &plan.steps);
            // Sorting again moves nothing.
            let again = sort_order(&plan.slots, &o, &keys(), order);
            let mut second = Plan::new(plan.slots.clone(), vec![false; n]);
            second.collect(&o);
            second.sort(&o, &again);
            assert!(second.steps.is_empty(), "round {round}: {:?}", second.steps);
        }
    }

    #[test]
    fn collecting_leaves_one_partial_stack_per_kind() {
        let mut rng = Rng(42);
        for _ in 0..2000 {
            let start = random_slots(&mut rng, 27, 80);
            let o: Vec<usize> = (0..27).collect();
            let mut plan = Plan::new(start.clone(), vec![false; 27]);
            plan.collect(&o);
            assert!(!plan.broken && plan.carried.is_none());
            let mut partial = HashMap::new();
            for s in plan.slots.iter().flatten() {
                if s.count < s.max {
                    *partial.entry(s.kind).or_insert(0) += 1;
                }
            }
            assert!(partial.values().all(|&c| c <= 1), "{partial:?}");
            assert_eq!(totals(&start, &o), totals(&plan.slots, &o));
        }
    }

    #[test]
    fn moving_between_inventories_keeps_every_item() {
        let mut rng = Rng(7);
        for round in 0..3000 {
            let mut start = random_slots(&mut rng, 27, 60);
            start.extend(random_slots(&mut rng, 27, 60));
            let all: Vec<usize> = (0..54).collect();
            let (a, b): (Vec<usize>, Vec<usize>) = (all[..27].to_vec(), all[27..].to_vec());
            // The second half is a shulker box half the time.
            let boxed = round % 2 == 0;
            let refuse: Vec<bool> = (0..54).map(|i| boxed && i >= 27).collect();
            for op in 0..3 {
                let mut plan = Plan::new(start.clone(), refuse.clone());
                match op {
                    0 => plan.fill(&a, &b),
                    1 => {
                        let from = plan.matching(&a, &b, true);
                        plan.transfer(&from, &b, true)
                    }
                    _ => plan.transfer(&a, &b, true),
                }
                assert!(!plan.broken, "round {round} op {op}");
                assert!(plan.carried.is_none());
                assert_eq!(totals(&start, &all), totals(&plan.slots, &all));
                if boxed {
                    assert!(plan.slots[27..].iter().flatten().filter(|s| s.boxed).count()
                        <= start[27..].iter().flatten().filter(|s| s.boxed).count());
                }
                if op == 2 {
                    // Whatever stayed behind found no room.
                    for &s in &a {
                        if let Some(left) = plan.slots[s] {
                            assert!(b.iter().all(|&d| match plan.slots[d] {
                                None => boxed && left.boxed,
                                Some(k) => k.kind != left.kind || k.count == k.max,
                            }));
                        }
                    }
                }
                replays(&start, &refuse, &plan.steps);
            }
        }
    }

    #[test]
    fn restock_matches_kinds_and_tools_by_type() {
        let mut sword = kind(4);
        sword.count = 1;
        let mut worn = kind(5);
        worn.count = 1;
        let mut dirt = kind(0);
        dirt.count = 10;
        let slots = vec![Some(sword), Some(dirt), None, Some(worn), None];
        let plan = Plan::new(slots, vec![false; 5]);
        assert_eq!(plan.matching(&[0, 1], &[3, 4], true), vec![0]);
        assert!(plan.matching(&[0, 1], &[3, 4], false).is_empty());
    }

    #[test]
    fn swaps_with_bundles_use_the_right_button() {
        let mut bundle = kind(6);
        bundle.count = 1;
        let mut dirt = kind(0);
        dirt.count = 5;
        let slots = vec![Some(bundle), Some(dirt)];
        let mut plan = Plan::new(slots.clone(), vec![false; 2]);
        plan.sort(&[0, 1], &[1, 0]);
        assert!(!plan.broken);
        assert_eq!(plan.slots, vec![Some(dirt), Some(bundle)]);
        assert!(plan.steps.iter().any(|s| s.button == 1));
    }

    #[test]
    fn settings_round_trip() {
        let mut s = Settings::default();
        s.click(1);
        s.click(2);
        s.click(3);
        s.click(0);
        let stored: HashMap<_, _> = s.stored().into_iter().collect();
        let back = Settings::read(|k| stored.get(k).cloned());
        assert_eq!(back, s);
        assert_eq!(back.order, Order::Alphabet);
        assert_eq!(back.interval_ms(false), 0);
        assert_eq!(Settings::default().interval_ms(true), 0);
        assert_eq!(Settings::default().interval_ms(false), 10);
        assert_eq!(s.labels(true).len(), 6);
        assert!(!Settings::default().tooltips);
    }

    #[test]
    fn clicks_follow_the_interval() {
        let mut budget = 0.0;
        assert_eq!(clicks_due(&mut budget, 16.0, 10), 1);
        assert_eq!(clicks_due(&mut budget, 16.0, 10), 2);
        assert_eq!(clicks_due(&mut budget, 1000.0, 10), 4);
        assert_eq!(clicks_due(&mut budget, 5.0, 0), usize::MAX);
    }

    #[test]
    fn buttons_sit_on_the_panel_edge() {
        assert_eq!(button_rect(176.0, 18.0, 0), [172.0, 18.0, 13.0, 13.0]);
        assert_eq!(button_rect(176.0, 18.0, 3), [172.0, 60.0, 13.0, 13.0]);
        assert!(inside(button_rect(0.0, 0.0, 1), [-3.0, 15.0]));
        assert_eq!(sprite(Op::Sort, true, false).len(), sprite(Op::Sort, false, false).len());
        assert_eq!(mouse_key(2), "key.mouse.middle");
        assert_eq!(mouse_key(3), "key.mouse.4");
    }

    #[test]
    fn no_fill_button_as_in_clientsort() {
        assert_eq!(group_ops(true), &[Op::Sort, Op::MatchTransfer, Op::Transfer]);
        assert_eq!(group_ops(false), &[Op::Sort]);
    }
}
