// Mouse Tweaks for Horizon: the mod's logic, with no game types, so the same
// file builds and tests on its own (`cargo test` at the repository root) and
// is what the bundle adds to the game as `crate::game::mousetweaks`.
//
// A port of Mouse Tweaks by Ivan Molodetskikh (YaLTeR), BSD-3-Clause,
// https://github.com/YaLTeR/MouseTweaks: `Main.java`'s RMB, LMB and wheel
// tweaks and `Config.java`'s options. The tweaks act on the open menu
// through `Menu`, which the game implements over its own slots: every action
// is an ordinary slot click, read back before the next one decides.

use std::sync::Mutex;

pub const ID: &str = "mousetweaks";
pub const NAME: &str = "Mouse Tweaks";
pub const VERSION: &str = "1.1.0";

pub fn russian(language: &str) -> bool {
    language.starts_with("ru")
}

pub fn description(ru: bool) -> &'static str {
    if ru {
        "Протяжки мышью и колесо для перекладывания предметов в инвентаре."
    } else {
        "Enhances inventory management by adding various functions to the mouse buttons."
    }
}

// ---- the menu --------------------------------------------------------------

/// A slot's stack as the tweaks see it: `kind` is the same for stacks of the
/// same item with the same components, `max` the item's stack size.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Stack {
    pub kind: u32,
    pub count: u32,
    pub max: u32,
    pub bundle: bool,
}

/// The open menu, slots in the menu's order (`AbstractContainerMenu.slots`).
pub trait Menu {
    fn len(&self) -> usize;
    fn stack(&mut self, slot: usize) -> Option<Stack>;
    /// The stack on the cursor (`getCarried`).
    fn carried(&mut self) -> Option<Stack>;
    /// The slot is the player's inventory (`slot.container ==
    /// player.getInventory()`): hotbar, main, armour and off hand.
    fn player(&self, slot: usize) -> bool;
    /// A crafting, furnace or trading result (`isCraftingOutput`).
    fn output(&self, slot: usize) -> bool;
    /// `IGuiScreenHandler.isIgnored`: no slot of a vanilla screen is.
    fn ignored(&self, _slot: usize) -> bool {
        false
    }
    /// The slot's top on the screen.
    fn y(&self, slot: usize) -> f32;
    /// `Slot.mayPlace`.
    fn may_place(&mut self, slot: usize, stack: Stack) -> bool;
    /// `Slot.getMaxStackSize(stack)`: 0 for a slot that takes none of it.
    fn max_size(&mut self, slot: usize, stack: Stack) -> u32;
    /// `slotClicked` with PICKUP (left or right) or QUICK_MOVE (`shift`).
    fn click(&mut self, slot: usize, right: bool, shift: bool);
    /// `disableRMBDraggingFunctionality`: the screen's own right-drag stops,
    /// and the button's release clicks nothing.
    fn stop_rmb_drag(&mut self);
}

/// `areStacksCompatible`: an empty stack goes with anything.
pub fn compatible(a: Option<Stack>, b: Option<Stack>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a.kind == b.kind,
        _ => true,
    }
}

// ---- the settings ----------------------------------------------------------

/// `WheelSearchOrder`: where the wheel pulls from first.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SearchOrder {
    FirstToLast,
    LastToFirst,
}

/// `WheelScrollDirection`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Direction {
    Normal,
    Inverted,
    PositionAware,
    PositionAwareInverted,
}

impl Direction {
    const ALL: [Direction; 4] = [
        Direction::Normal,
        Direction::Inverted,
        Direction::PositionAware,
        Direction::PositionAwareInverted,
    ];
    fn inverted(self) -> bool {
        matches!(self, Direction::Inverted | Direction::PositionAwareInverted)
    }
    fn position_aware(self) -> bool {
        matches!(self, Direction::PositionAware | Direction::PositionAwareInverted)
    }
    fn stored(self) -> &'static str {
        match self {
            Direction::Normal => "normal",
            Direction::Inverted => "inverted",
            Direction::PositionAware => "position_aware",
            Direction::PositionAwareInverted => "position_aware_inverted",
        }
    }
}

/// `ScrollItemScaling`: how many items a wheel event moves.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Scaling {
    Proportional,
    AlwaysOne,
}

impl Scaling {
    fn scale(self, delta: f64) -> f64 {
        match self {
            Scaling::Proportional => delta,
            Scaling::AlwaysOne => {
                if delta == 0.0 {
                    0.0
                } else {
                    delta.signum()
                }
            }
        }
    }
}

/// `Config`, with its defaults.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Settings {
    pub rmb: bool,
    pub lmb_with_item: bool,
    pub lmb_without_item: bool,
    pub wheel: bool,
    pub order: SearchOrder,
    pub direction: Direction,
    pub scaling: Scaling,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            rmb: true,
            lmb_with_item: true,
            lmb_without_item: true,
            wheel: true,
            order: SearchOrder::LastToFirst,
            direction: Direction::Normal,
            scaling: Scaling::Proportional,
        }
    }
}

impl Settings {
    /// The settings page's rows (`ConfigScreen`), in order.
    pub const ROWS: usize = 7;

    pub fn read(get: impl Fn(&str) -> Option<String>) -> Settings {
        let d = Settings::default();
        let flag = |k: &str, d: bool| match get(k).as_deref().map(str::trim) {
            Some("true") => true,
            Some("false") => false,
            _ => d,
        };
        let value = |k: &str| get(k).map(|v| v.trim().to_owned());
        Settings {
            rmb: flag("rmb_tweak", d.rmb),
            lmb_with_item: flag("lmb_tweak_with_item", d.lmb_with_item),
            lmb_without_item: flag("lmb_tweak_without_item", d.lmb_without_item),
            wheel: flag("wheel_tweak", d.wheel),
            order: match value("wheel_search_order").as_deref() {
                Some("first_to_last") => SearchOrder::FirstToLast,
                Some("last_to_first") => SearchOrder::LastToFirst,
                _ => d.order,
            },
            direction: value("wheel_scroll_direction")
                .and_then(|v| Direction::ALL.into_iter().find(|x| x.stored() == v))
                .unwrap_or(d.direction),
            scaling: match value("scroll_item_scaling").as_deref() {
                Some("proportional") => Scaling::Proportional,
                Some("always_one") => Scaling::AlwaysOne,
                _ => d.scaling,
            },
        }
    }

    pub fn stored(&self) -> Vec<(&'static str, String)> {
        vec![
            ("rmb_tweak", self.rmb.to_string()),
            ("lmb_tweak_with_item", self.lmb_with_item.to_string()),
            ("lmb_tweak_without_item", self.lmb_without_item.to_string()),
            ("wheel_tweak", self.wheel.to_string()),
            (
                "wheel_search_order",
                match self.order {
                    SearchOrder::FirstToLast => "first_to_last",
                    SearchOrder::LastToFirst => "last_to_first",
                }
                .to_owned(),
            ),
            ("wheel_scroll_direction", self.direction.stored().to_owned()),
            (
                "scroll_item_scaling",
                match self.scaling {
                    Scaling::Proportional => "proportional",
                    Scaling::AlwaysOne => "always_one",
                }
                .to_owned(),
            ),
        ]
    }

    /// Row `row`'s name: `ConfigScreen`'s caption of its button (the page
    /// lists the rows in this order).
    pub fn caption(row: usize, ru: bool) -> &'static str {
        let t = |r: &'static str, e: &'static str| if ru { r } else { e };
        match row {
            0 => t("ПКМ-протяжка", "RMB Tweak"),
            1 => t("ЛКМ с предметом", "LMB Tweak With Item"),
            2 => t("ЛКМ без предмета", "LMB Tweak Without Item"),
            3 => t("Колесо мыши", "Wheel Tweak"),
            4 => t("Порядок поиска", "Wheel Tweak Search Order"),
            5 => t("Направление", "Scroll Direction"),
            _ => t("Шаг колеса", "Scroll Scaling"),
        }
    }

    /// Row `row`'s value: `Ok` of a switch (the game's ON and OFF), else
    /// the choice's text.
    pub fn value(&self, row: usize, ru: bool) -> Result<bool, &'static str> {
        let t = |r: &'static str, e: &'static str| if ru { r } else { e };
        match row {
            0 => Ok(self.rmb),
            1 => Ok(self.lmb_with_item),
            2 => Ok(self.lmb_without_item),
            3 => Ok(self.wheel),
            4 => Err(match self.order {
                SearchOrder::FirstToLast => t("С первого слота", "First to Last"),
                SearchOrder::LastToFirst => t("С последнего слота", "Last to First"),
            }),
            5 => Err(match self.direction {
                Direction::Normal => t("Вниз — отдать, вверх — забрать", "Down to Push, Up to Pull"),
                Direction::Inverted => t("Вверх — отдать, вниз — забрать", "Up to Push, Down to Pull"),
                Direction::PositionAware => t("По положению инвентаря", "Inventory Position Aware"),
                Direction::PositionAwareInverted => {
                    t("По положению инвентаря, наоборот", "Inventory Position Aware, Inverted")
                }
            }),
            _ => Err(match self.scaling {
                Scaling::Proportional => {
                    t("Сколько щелчков, столько предметов", "Multiple Wheel Clicks Move Multiple Items")
                }
                Scaling::AlwaysOne => t("Всегда один предмет (для macOS)", "Always Move One Item (macOS Compatibility)"),
            }),
        }
    }

    /// The page's buttons after its first: "RMB Tweak: ON", one per row.
    pub fn labels(&self, ru: bool) -> Vec<String> {
        (0..Self::ROWS)
            .map(|row| {
                let value = match self.value(row, ru) {
                    Ok(true) if ru => "Вкл",
                    Ok(true) => "ON",
                    Ok(false) if ru => "Выкл",
                    Ok(false) => "OFF",
                    Err(text) => text,
                };
                format!("{}: {value}", Self::caption(row, ru))
            })
            .collect()
    }

    /// Row `row`'s button pressed (`CycleButton.onPress`): a switch flips, a
    /// choice moves on, or back with `back` (Shift held, the wheel up).
    pub fn cycle(&mut self, row: usize, back: bool) {
        match row {
            0 => self.rmb = !self.rmb,
            1 => self.lmb_with_item = !self.lmb_with_item,
            2 => self.lmb_without_item = !self.lmb_without_item,
            3 => self.wheel = !self.wheel,
            4 => {
                self.order = match self.order {
                    SearchOrder::FirstToLast => SearchOrder::LastToFirst,
                    SearchOrder::LastToFirst => SearchOrder::FirstToLast,
                }
            }
            5 => {
                let n = Direction::ALL.len();
                let i = Direction::ALL.iter().position(|d| *d == self.direction).unwrap_or(0);
                self.direction = Direction::ALL[if back { (i + n - 1) % n } else { (i + 1) % n }];
            }
            6 => {
                self.scaling = match self.scaling {
                    Scaling::Proportional => Scaling::AlwaysOne,
                    Scaling::AlwaysOne => Scaling::Proportional,
                }
            }
            _ => {}
        }
    }

    /// A click on row `row` of the page: the toggle flips, a choice moves on.
    pub fn click(&mut self, row: usize) {
        self.cycle(row, false);
    }
}

static SETTINGS: Mutex<Option<Settings>> = Mutex::new(None);

/// The settings, read once from the game's store.
pub fn settings(get: impl Fn(&str) -> Option<String>) -> Settings {
    let mut kept = SETTINGS.lock().unwrap_or_else(|e| e.into_inner());
    *kept.get_or_insert_with(|| Settings::read(get))
}

pub fn set_settings(settings: Settings) {
    *SETTINGS.lock().unwrap_or_else(|e| e.into_inner()) = Some(settings);
}

// ---- the tweaks ------------------------------------------------------------

/// A guard on the loops whose clicks the game could refuse.
const MAX_ROUNDS: u32 = 256;

/// `Main`'s state for the open screen.
#[derive(Debug, Default)]
pub struct Tweaks {
    screen: Option<String>,
    old_selected: Option<usize>,
    accumulated: f64,
    can_lmb: bool,
    can_rmb: bool,
    rmb_left_original: bool,
    /// The button whose drag this is (`MouseHandler.activeButton`): right
    /// when true.
    button: Option<bool>,
}

impl Tweaks {
    pub const fn new() -> Self {
        Tweaks {
            screen: None,
            old_selected: None,
            accumulated: 0.0,
            can_lmb: false,
            can_rmb: false,
            rmb_left_original: false,
            button: None,
        }
    }

    /// `updateScreen`: another screen (or none the tweaks run in, `None`)
    /// starts afresh. True when the tweaks run in it.
    pub fn update_screen(&mut self, screen: Option<&str>) -> bool {
        if self.screen.as_deref() != screen {
            *self = Tweaks::new();
            self.screen = screen.map(str::to_owned);
        }
        screen.is_some()
    }

    /// A button is held over the screen.
    pub fn dragging(&self) -> bool {
        self.button.is_some()
    }

    /// `onMouseClicked`, before the screen takes the press (never taken).
    pub fn press(&mut self, m: &mut impl Menu, hovered: Option<usize>, right: bool, s: &Settings) {
        self.button = Some(right);
        self.old_selected = hovered;
        let carried = m.carried();
        if !right {
            // With a stack on the cursor the screen's own left-drag starts.
            if carried.is_none() {
                self.can_lmb = true;
            }
        } else {
            if carried.is_none() || !s.rmb {
                return;
            }
            self.can_rmb = true;
            self.rmb_left_original = false;
        }
    }

    /// `onMouseReleased`.
    pub fn release(&mut self, right: bool) {
        if right {
            self.can_rmb = false;
        } else {
            self.can_lmb = false;
        }
        if self.button == Some(right) {
            self.button = None;
        }
    }

    /// `rmbTweakMaybeClickSlot`.
    fn rmb_click(&mut self, m: &mut impl Menu, slot: Option<usize>) {
        let Some(slot) = slot else {
            return;
        };
        let Some(carried) = m.carried() else {
            return;
        };
        if m.ignored(slot) || m.output(slot) {
            return;
        }
        // A bundle goes over every slot: it takes their items in.
        if !carried.bundle {
            let here = m.stack(slot);
            if !compatible(here, Some(carried)) {
                return;
            }
            if let Some(h) = here
                && h.count >= m.max_size(slot, h)
            {
                return;
            }
        }
        m.click(slot, true, false);
    }

    /// `onMouseDrag`: the cursor moved with a button held.
    pub fn drag(&mut self, m: &mut impl Menu, hovered: Option<usize>, shift: bool, s: &Settings) {
        let Some(right) = self.button else {
            return;
        };
        if hovered == self.old_selected {
            return;
        }
        // Leaving the first slot: the screen's own drag stops, and the first
        // slot gets its item as the others will.
        if self.can_rmb && right && !self.rmb_left_original {
            self.rmb_left_original = true;
            m.stop_rmb_drag();
            let first = self.old_selected;
            self.rmb_click(m, first);
        }
        self.old_selected = hovered;
        let Some(slot) = hovered else {
            return;
        };
        if m.ignored(slot) {
            return;
        }
        if right {
            if self.can_rmb {
                self.rmb_click(m, Some(slot));
            }
            return;
        }
        if !self.can_lmb {
            return;
        }
        let Some(here) = m.stack(slot) else {
            return;
        };
        match m.carried() {
            None => {
                // Shift with an empty hand: every slot crossed is quick-moved.
                if s.lmb_without_item && shift {
                    m.click(slot, false, true);
                }
            }
            Some(held) => {
                if !s.lmb_with_item || !compatible(Some(here), Some(held)) {
                    return;
                }
                if shift {
                    m.click(slot, false, true);
                } else if held.count + here.count <= held.max {
                    // Put the hand's stack on the slot's, then take them both;
                    // a result slot adds to the hand at once.
                    m.click(slot, false, false);
                    if !m.output(slot) {
                        m.click(slot, false, false);
                    }
                }
            }
        }
    }

    /// `otherInventoryIsAbove`.
    fn other_above(m: &impl Menu, selected: usize) -> bool {
        let mine = m.player(selected);
        let y = m.y(selected);
        let (mut above, mut below) = (0, 0);
        for i in 0..m.len() {
            if m.player(i) != mine {
                if m.y(i) < y {
                    above += 1;
                } else {
                    below += 1;
                }
            }
        }
        above > below
    }

    /// `findPullSlot`: the first slot of the other side holding the
    /// selected slot's item.
    fn pull_slot(m: &mut impl Menu, selected: usize, s: &Settings) -> Option<usize> {
        let want = m.stack(selected);
        let in_player = !m.player(selected);
        let order: Box<dyn Iterator<Item = usize>> = match s.order {
            SearchOrder::FirstToLast => Box::new(0..m.len()),
            SearchOrder::LastToFirst => Box::new((0..m.len()).rev()),
        };
        for i in order {
            if m.ignored(i) || m.player(i) != in_player {
                continue;
            }
            let here = m.stack(i);
            if here.is_some() && compatible(want, here) {
                return Some(i);
            }
        }
        None
    }

    /// `findPushSlots`: where `count` of the selected slot's items go on the
    /// other side, partial stacks first, then empty slots. None when
    /// `all` and they don't all fit.
    fn push_slots(m: &mut impl Menu, selected: usize, count: u32, all: bool) -> Option<Vec<usize>> {
        let Some(want) = m.stack(selected) else {
            return Some(Vec::new());
        };
        let in_player = !m.player(selected);
        let mut left = count;
        let mut out = Vec::new();
        let mut empty = Vec::new();
        for i in 0..m.len() {
            if left == 0 {
                break;
            }
            if m.ignored(i) || m.player(i) != in_player || m.output(i) {
                continue;
            }
            match m.stack(i) {
                None => {
                    if m.may_place(i, want) {
                        empty.push(i);
                    }
                }
                Some(here) => {
                    let max = m.max_size(i, here);
                    if compatible(Some(want), Some(here)) && here.count < max {
                        out.push(i);
                        left -= left.min(max - here.count);
                    }
                }
            }
        }
        for i in empty {
            if left == 0 {
                break;
            }
            out.push(i);
            left -= left.min(m.max_size(i, want));
        }
        if all && left > 0 {
            return None;
        }
        Some(out)
    }

    /// `onMouseScrolled`: true when the wheel was over a slot (it is taken
    /// even when nothing moves).
    pub fn scroll(&mut self, m: &mut impl Menu, hovered: Option<usize>, delta: f64, s: &Settings) -> bool {
        if !s.wheel {
            return false;
        }
        let Some(selected) = hovered else {
            return false;
        };
        if m.ignored(selected) {
            return false;
        }
        let scaled = s.scaling.scale(delta);
        if self.accumulated != 0.0 && scaled.signum() != self.accumulated.signum() {
            self.accumulated = 0.0;
        }
        self.accumulated += scaled;
        let steps = self.accumulated.trunc();
        self.accumulated -= steps;
        if steps == 0.0 {
            return true;
        }
        let n = steps.abs() as u32;
        let mut push = steps < 0.0;
        if s.direction.position_aware() && Self::other_above(m, selected) {
            push = !push;
        }
        if s.direction.inverted() {
            push = !push;
        }
        let Some(here) = m.stack(selected) else {
            return true;
        };
        let mouse = m.carried();
        if m.output(selected) {
            Self::scroll_output(m, selected, here, mouse, n, push);
        } else if mouse.is_none() || !compatible(Some(here), mouse) {
            if push {
                Self::push(m, selected, here, mouse, n);
            } else {
                Self::pull(m, selected, here, mouse, n, s);
            }
        }
        true
    }

    /// The wheel over a result: down takes whole crafts into the other side
    /// (only where each fits), and with the result's item on the cursor any
    /// wheel adds crafts to it.
    fn scroll_output(m: &mut impl Menu, selected: usize, here: Stack, mouse: Option<Stack>, n: u32, push: bool) {
        if !compatible(Some(here), mouse) {
            return;
        }
        if mouse.is_some() {
            for _ in 0..n {
                m.click(selected, false, false);
            }
            return;
        }
        if !push {
            return;
        }
        for _ in 0..n {
            let Some(batch) = m.stack(selected) else {
                break;
            };
            let Some(targets) = Self::push_slots(m, selected, batch.count, true) else {
                break;
            };
            m.click(selected, false, false);
            let last = targets.len().saturating_sub(1);
            for (i, &t) in targets.iter().enumerate() {
                if i == last {
                    m.click(t, false, false);
                    continue;
                }
                let room = match m.stack(t) {
                    Some(h) => m.max_size(t, h).saturating_sub(h.count),
                    None => m.carried().map_or(0, |c| m.max_size(t, c)),
                };
                for _ in 0..room {
                    m.click(t, true, false);
                }
            }
        }
    }

    /// Wheel push: up to `n` items of the slot to the other side.
    fn push(m: &mut impl Menu, selected: usize, here: Stack, mouse: Option<Stack>, n: u32) {
        if let Some(c) = mouse
            && !m.may_place(selected, c)
        {
            return;
        }
        let mut n = n.min(here.count);
        let targets = Self::push_slots(m, selected, n, false).unwrap_or_default();
        if targets.is_empty() {
            return;
        }
        let had = mouse.is_some();
        // A right click takes half and keeps a furnace's progress; a left
        // click takes all when all go (a bundle is picked up, not opened).
        let left = mouse.is_none() && here.count <= n;
        m.click(selected, !left, false);
        let picked = match m.carried() {
            Some(p) if p.kind == here.kind => p,
            _ => return,
        };
        n = n.min(picked.count);
        for t in targets {
            let have = m.stack(t).map_or(0, |h| h.count);
            let clicks = m.max_size(t, picked).saturating_sub(have).min(n);
            n -= clicks;
            for _ in 0..clicks {
                m.click(t, true, false);
            }
        }
        // Put the rest back, and take back what the cursor held.
        let leftover = m.carried().is_some();
        if had || leftover {
            m.click(selected, had && leftover, false);
        }
    }

    /// Wheel pull: up to `n` items of the slot's kind from the other side.
    fn pull(m: &mut impl Menu, selected: usize, here: Stack, mouse: Option<Stack>, n: u32, s: &Settings) {
        let mut room = m.max_size(selected, here).saturating_sub(here.count);
        let mut n = n.min(room);
        let mut rounds = 0;
        while n > 0 && rounds < MAX_ROUNDS {
            rounds += 1;
            let Some(target) = Self::pull_slot(m, selected, s) else {
                break;
            };
            let Some(there) = m.stack(target) else {
                break;
            };
            if m.output(target) {
                // One craft a wheel step, and only one that fits whole.
                if room < there.count {
                    break;
                }
                room -= there.count;
                n = (n - 1).min(room);
                if let Some(c) = mouse
                    && !m.may_place(selected, c)
                {
                    break;
                }
                m.click(selected, false, false);
                m.click(target, false, false);
                m.click(selected, false, false);
                continue;
            }
            let had = mouse.is_some();
            if let Some(c) = mouse
                && !m.may_place(target, c)
            {
                break;
            }
            let left = mouse.is_none() && there.count == 1;
            m.click(target, !left, false);
            let picked = match m.carried() {
                Some(p) if p.kind == here.kind && p.count > 0 => p.count,
                _ => break,
            };
            let moved = picked.min(n);
            if moved == picked {
                m.click(selected, false, false);
            } else {
                for _ in 0..moved {
                    m.click(selected, true, false);
                }
            }
            room = room.saturating_sub(moved);
            n -= moved;
            let leftover = m.carried().is_some();
            if had || leftover {
                m.click(target, had && leftover, false);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: u32 = 1;
    const B: u32 = 2;
    const SWORD: u32 = 3;
    const BUNDLE: u32 = 4;

    fn st(kind: u32, count: u32) -> Option<Stack> {
        let max = if kind == SWORD || kind == BUNDLE { 1 } else { 64 };
        Some(Stack { kind, count, max, bundle: kind == BUNDLE })
    }

    #[derive(Clone)]
    struct Slot {
        stack: Option<Stack>,
        player: bool,
        output: bool,
        y: f32,
        /// What the slot takes (`mayPlace`): None for anything.
        only: Option<u32>,
        /// `Slot.getMaxStackSize()`.
        limit: u32,
    }

    /// A vanilla-like menu: `AbstractContainerMenu.doClick`'s PICKUP, a
    /// chest's `quickMoveStack`, and a crafting result that refills from
    /// `crafts` left.
    struct Sim {
        slots: Vec<Slot>,
        carried: Option<Stack>,
        crafts: u32,
        clicks: usize,
        stopped: usize,
    }

    impl Sim {
        /// A chest of `chest` slots above the 27 main slots and the hotbar.
        fn chest(chest: usize) -> Sim {
            let mut slots = Vec::new();
            for i in 0..chest {
                slots.push(Slot { stack: None, player: false, output: false, y: (i / 9) as f32 * 18.0, only: None, limit: 99 });
            }
            for i in 0..36 {
                let y = 100.0 + (i / 9) as f32 * 18.0 + if i >= 27 { 4.0 } else { 0.0 };
                slots.push(Slot { stack: None, player: true, output: false, y, only: None, limit: 99 });
            }
            Sim { slots, carried: None, crafts: 0, clicks: 0, stopped: 0 }
        }
        /// The player's inventory screen: result, 2x2 grid, then 36 slots.
        fn inventory(result: Option<Stack>, crafts: u32) -> Sim {
            let mut sim = Sim::chest(0);
            for i in 0..4 {
                sim.slots.insert(0, Slot { stack: None, player: false, output: false, y: 20.0 + (i / 2) as f32 * 18.0, only: None, limit: 99 });
            }
            sim.slots.insert(0, Slot { stack: result, player: false, output: true, y: 28.0, only: Some(0), limit: 99 });
            sim.crafts = crafts;
            sim
        }
        fn set(&mut self, i: usize, s: Option<Stack>) {
            self.slots[i].stack = s;
        }
        fn count(&self, i: usize) -> u32 {
            self.slots[i].stack.map_or(0, |s| s.count)
        }
        fn total(&self, kind: u32) -> u32 {
            self.slots.iter().filter(|s| !s.output).filter_map(|s| s.stack).filter(|s| s.kind == kind).map(|s| s.count).sum::<u32>()
                + self.carried.filter(|c| c.kind == kind).map_or(0, |c| c.count)
        }
        fn mays(&self, i: usize, s: Stack) -> bool {
            let slot = &self.slots[i];
            !slot.output && slot.only.is_none_or(|k| k == s.kind)
        }
        fn max_of(&self, i: usize, s: Stack) -> u32 {
            self.slots[i].limit.min(s.max)
        }
        fn insert(&mut self, i: usize, n: u32) {
            let Some(mut c) = self.carried else { return };
            if !self.mays(i, c) {
                return;
            }
            let have = self.slots[i].stack.map_or(0, |s| s.count);
            let k = n.min(c.count).min(self.max_of(i, c).saturating_sub(have));
            if k == 0 {
                return;
            }
            self.slots[i].stack = Some(Stack { count: have + k, ..c });
            c.count -= k;
            self.carried = (c.count > 0).then_some(c);
        }
        fn take_result(&mut self, i: usize) {
            let Some(r) = self.slots[i].stack else { return };
            match self.carried {
                None => self.carried = Some(r),
                Some(c) if c.kind == r.kind && c.count + r.count <= c.max => {
                    self.carried = Some(Stack { count: c.count + r.count, ..c })
                }
                _ => return,
            }
            self.crafts -= 1;
            if self.crafts == 0 {
                self.slots[i].stack = None;
            }
        }
    }

    impl Menu for Sim {
        fn len(&self) -> usize {
            self.slots.len()
        }
        fn stack(&mut self, slot: usize) -> Option<Stack> {
            self.slots[slot].stack
        }
        fn carried(&mut self) -> Option<Stack> {
            self.carried
        }
        fn player(&self, slot: usize) -> bool {
            self.slots[slot].player
        }
        fn output(&self, slot: usize) -> bool {
            self.slots[slot].output
        }
        fn y(&self, slot: usize) -> f32 {
            self.slots[slot].y
        }
        fn may_place(&mut self, slot: usize, stack: Stack) -> bool {
            self.mays(slot, stack)
        }
        fn max_size(&mut self, slot: usize, stack: Stack) -> u32 {
            if self.mays(slot, stack) { self.max_of(slot, stack) } else { 0 }
        }
        fn click(&mut self, i: usize, right: bool, shift: bool) {
            self.clicks += 1;
            if self.slots[i].output {
                self.take_result(i);
                return;
            }
            if shift {
                // quickMoveStack: to the other side, partial stacks first.
                let Some(mut s) = self.slots[i].stack else { return };
                let side = !self.slots[i].player;
                for pass in 0..2 {
                    for j in 0..self.slots.len() {
                        if s.count == 0 || self.slots[j].player != side || self.slots[j].output || !self.mays(j, s) {
                            continue;
                        }
                        let room = match self.slots[j].stack {
                            Some(h) if pass == 0 && h.kind == s.kind => self.max_of(j, s) - h.count,
                            None if pass == 1 => self.max_of(j, s),
                            _ => continue,
                        };
                        let k = room.min(s.count);
                        let have = self.slots[j].stack.map_or(0, |h| h.count);
                        self.slots[j].stack = Some(Stack { count: have + k, ..s });
                        s.count -= k;
                    }
                }
                self.slots[i].stack = (s.count > 0).then_some(s);
                return;
            }
            match (self.slots[i].stack, self.carried) {
                (None, Some(c)) => self.insert(i, if right { 1 } else { c.count }),
                (None, None) => {}
                (Some(h), None) => {
                    let n = if right { h.count.div_ceil(2) } else { h.count };
                    self.carried = Some(Stack { count: n, ..h });
                    self.slots[i].stack = (h.count > n).then_some(Stack { count: h.count - n, ..h });
                }
                (Some(h), Some(c)) => {
                    if c.bundle && !h.bundle {
                        // `BundleItem.overrideStackedOnOther`: the bundle takes the stack in.
                        self.slots[i].stack = None;
                        return;
                    }
                    if !self.mays(i, c) {
                        return;
                    }
                    if h.kind == c.kind {
                        self.insert(i, if right { 1 } else { c.count });
                    } else if c.count <= self.max_of(i, c) {
                        self.carried = Some(h);
                        self.slots[i].stack = Some(c);
                    }
                }
            }
        }
        fn stop_rmb_drag(&mut self) {
            self.stopped += 1;
        }
    }

    fn defaults() -> Settings {
        Settings::default()
    }

    #[test]
    fn right_drag_puts_one_in_each_slot_it_crosses() {
        let s = defaults();
        let mut m = Sim::chest(27);
        m.carried = st(A, 10);
        m.set(2, st(B, 3));
        m.set(3, st(A, 64));
        m.set(4, st(A, 5));
        let mut t = Tweaks::new();
        t.update_screen(Some("chest"));
        t.press(&mut m, Some(0), true, &s);
        // Over the first slot nothing happens yet (the screen's own drag).
        t.drag(&mut m, Some(0), false, &s);
        assert_eq!(m.clicks, 0);
        for slot in [1, 2, 3, 4, 1] {
            t.drag(&mut m, Some(slot), false, &s);
        }
        assert_eq!(m.stopped, 1, "the screen's right-drag stops once");
        assert_eq!((m.count(0), m.count(1), m.count(4)), (1, 2, 6), "re-entering a slot adds again");
        assert_eq!(m.slots[2].stack, st(B, 3), "another item is left alone");
        assert_eq!(m.count(3), 64, "a full stack is left alone");
        assert_eq!(m.carried, st(A, 6));
        t.release(true);
        t.drag(&mut m, Some(5), false, &s);
        assert_eq!(m.count(5), 0, "the drag ended with the button");
    }

    #[test]
    fn a_right_press_and_release_on_one_slot_is_the_screens_own() {
        let s = defaults();
        let mut m = Sim::chest(27);
        m.carried = st(A, 10);
        let mut t = Tweaks::new();
        t.update_screen(Some("chest"));
        t.press(&mut m, Some(0), true, &s);
        t.release(true);
        assert_eq!((m.clicks, m.stopped), (0, 0));
        // Off, the right-drag stays the screen's.
        let off = Settings { rmb: false, ..s };
        t.press(&mut m, Some(0), true, &off);
        t.drag(&mut m, Some(1), false, &off);
        assert_eq!((m.clicks, m.stopped), (0, 0));
    }

    #[test]
    fn left_drag_gathers_the_same_item_while_it_fits() {
        let s = defaults();
        let mut m = Sim::chest(27);
        m.set(0, st(A, 10));
        m.set(1, st(A, 20));
        m.set(2, st(B, 5));
        m.set(3, st(A, 50));
        m.set(4, st(A, 30));
        let mut t = Tweaks::new();
        t.update_screen(Some("chest"));
        // The press with an empty hand; the screen then picks the stack up.
        t.press(&mut m, Some(0), false, &s);
        m.click(0, false, false);
        for slot in [1, 2, 3, 4] {
            t.drag(&mut m, Some(slot), false, &s);
        }
        assert_eq!(m.carried, st(A, 60), "10 + 20 + 30; 50 would not fit");
        assert_eq!((m.count(1), m.count(3), m.count(4)), (0, 50, 0));
        assert_eq!(m.slots[2].stack, st(B, 5));
        assert_eq!(m.total(A), 110, "nothing is lost");
    }

    #[test]
    fn a_left_drag_that_started_holding_a_stack_is_the_screens_own() {
        let s = defaults();
        let mut m = Sim::chest(27);
        m.carried = st(A, 10);
        m.set(1, st(A, 20));
        let mut t = Tweaks::new();
        t.update_screen(Some("chest"));
        t.press(&mut m, Some(0), false, &s);
        t.drag(&mut m, Some(1), false, &s);
        assert_eq!(m.clicks, 0);
    }

    #[test]
    fn shift_left_drag_moves_every_slot_crossed() {
        let s = defaults();
        let mut m = Sim::chest(27);
        let p = 27;
        m.set(p, st(A, 10));
        m.set(p + 1, st(B, 7));
        m.set(p + 2, st(A, 5));
        let mut t = Tweaks::new();
        t.update_screen(Some("chest"));
        t.press(&mut m, Some(p), false, &s);
        m.click(p, false, true); // the screen's own Shift click
        t.drag(&mut m, Some(p + 1), true, &s);
        t.drag(&mut m, Some(p + 2), true, &s);
        assert_eq!(m.slots[0].stack, st(A, 15));
        assert_eq!(m.slots[1].stack, st(B, 7));
        assert!((p..p + 3).all(|i| m.slots[i].stack.is_none()));
        // Without Shift an empty-handed drag does nothing.
        m.set(p + 3, st(A, 1));
        let before = m.clicks;
        t.drag(&mut m, Some(p + 3), false, &s);
        assert_eq!(m.clicks, before);
    }

    #[test]
    fn shift_left_drag_holding_a_stack_moves_only_its_item() {
        let s = defaults();
        let mut m = Sim::chest(27);
        let p = 27;
        m.set(p, st(A, 10));
        m.set(p + 1, st(A, 4));
        m.set(p + 2, st(B, 4));
        let mut t = Tweaks::new();
        t.update_screen(Some("chest"));
        t.press(&mut m, Some(p), false, &s);
        m.click(p, false, false);
        t.drag(&mut m, Some(p + 1), true, &s);
        t.drag(&mut m, Some(p + 2), true, &s);
        assert_eq!(m.slots[0].stack, st(A, 4));
        assert_eq!(m.slots[p + 2].stack, st(B, 4));
        assert_eq!(m.carried, st(A, 10));
    }

    #[test]
    fn the_wheel_pushes_and_pulls_one_item_a_click() {
        let s = defaults();
        let mut m = Sim::chest(27);
        let p = 27;
        m.set(5, st(A, 3));
        m.set(p, st(A, 5));
        let mut t = Tweaks::new();
        t.update_screen(Some("chest"));
        // Down: one goes to the chest's stack of the same item.
        assert!(t.scroll(&mut m, Some(p), -1.0, &s));
        assert_eq!((m.count(p), m.count(5)), (4, 4));
        // Three clicks in one event: three items, but the right click that
        // picks them up takes half the stack (the original's FIXME): two.
        assert!(t.scroll(&mut m, Some(p), -3.0, &s));
        assert_eq!((m.count(p), m.count(5)), (2, 6));
        // Two clicks on two items: a left click takes them all.
        assert!(t.scroll(&mut m, Some(p), -2.0, &s));
        assert_eq!((m.count(p), m.count(5)), (0, 8));
        assert!(t.scroll(&mut m, Some(p), -1.0, &s), "an empty slot still takes the wheel");
        // Up over the chest's stack: nothing on the other side to pull.
        assert!(t.scroll(&mut m, Some(5), 1.0, &s));
        assert_eq!(m.count(5), 8);
        // Up over a player slot: one back from the chest.
        m.set(p + 1, st(A, 1));
        assert!(t.scroll(&mut m, Some(p + 1), 2.0, &s));
        assert_eq!((m.count(p + 1), m.count(5)), (3, 6));
        assert_eq!(m.carried, None);
        assert_eq!(m.total(A), 9);
        assert!(!t.scroll(&mut m, None, 1.0, &s), "off a slot the wheel is the screen's");
    }

    #[test]
    fn pushing_fills_partial_stacks_first_then_the_first_empty_slot() {
        let s = defaults();
        let mut m = Sim::chest(27);
        let p = 27;
        m.set(p, st(A, 64));
        m.set(10, st(A, 63));
        let mut t = Tweaks::new();
        t.update_screen(Some("chest"));
        t.scroll(&mut m, Some(p), -2.0, &s);
        assert_eq!((m.count(10), m.count(0), m.count(p)), (64, 1, 62));
    }

    #[test]
    fn pulling_searches_from_the_last_slot_by_default() {
        let mut m = Sim::chest(27);
        let p = 27;
        m.set(2, st(A, 10));
        m.set(20, st(A, 10));
        m.set(p, st(A, 1));
        let mut t = Tweaks::new();
        t.update_screen(Some("chest"));
        t.scroll(&mut m, Some(p), 1.0, &defaults());
        assert_eq!((m.count(2), m.count(20)), (10, 9));
        let first = Settings { order: SearchOrder::FirstToLast, ..defaults() };
        t.scroll(&mut m, Some(p), 1.0, &first);
        assert_eq!((m.count(2), m.count(20)), (9, 9));
        assert_eq!(m.count(p), 3);
    }

    #[test]
    fn fractional_wheel_steps_add_up_and_a_turn_back_starts_over() {
        let s = defaults();
        let mut m = Sim::chest(27);
        let p = 27;
        m.set(p, st(A, 10));
        let mut t = Tweaks::new();
        t.update_screen(Some("chest"));
        t.scroll(&mut m, Some(p), -0.5, &s);
        assert_eq!(m.count(p), 10);
        t.scroll(&mut m, Some(p), -0.5, &s);
        assert_eq!(m.count(p), 9);
        t.scroll(&mut m, Some(p), -0.6, &s);
        t.scroll(&mut m, Some(p), 0.6, &s);
        assert_eq!(m.count(p), 9, "the turn back dropped the half step");
        // Always one: a three-click event moves one.
        let one = Settings { scaling: Scaling::AlwaysOne, ..s };
        t.scroll(&mut m, Some(p), -3.0, &one);
        assert_eq!(m.count(p), 8);
    }

    #[test]
    fn the_directions() {
        let p = 27;
        let mut m = Sim::chest(27);
        m.set(p, st(A, 10));
        let mut t = Tweaks::new();
        t.update_screen(Some("chest"));
        let inverted = Settings { direction: Direction::Inverted, ..defaults() };
        t.scroll(&mut m, Some(p), 1.0, &inverted);
        assert_eq!(m.count(p), 9, "inverted: up pushes");
        // Position aware: the chest is above the inventory, so up pushes
        // from the inventory and down pushes from the chest.
        let aware = Settings { direction: Direction::PositionAware, ..defaults() };
        t.scroll(&mut m, Some(p), 1.0, &aware);
        assert_eq!(m.count(p), 8);
        t.scroll(&mut m, Some(0), -1.0, &aware);
        assert_eq!((m.count(0), m.count(p)), (1, 9), "down over the chest pulls");
        let both = Settings { direction: Direction::PositionAwareInverted, ..defaults() };
        t.scroll(&mut m, Some(p), 1.0, &both);
        assert_eq!(m.count(p), 10);
    }

    #[test]
    fn the_wheel_with_another_item_on_the_cursor_gives_it_back() {
        let s = defaults();
        let mut m = Sim::chest(27);
        let p = 27;
        m.set(p, st(A, 10));
        m.set(4, st(A, 2));
        m.carried = st(B, 3);
        let mut t = Tweaks::new();
        t.update_screen(Some("chest"));
        t.scroll(&mut m, Some(p), -1.0, &s);
        assert_eq!((m.count(p), m.count(4)), (9, 3));
        assert_eq!(m.carried, st(B, 3));
        t.scroll(&mut m, Some(p), 1.0, &s);
        assert_eq!((m.count(p), m.count(4)), (10, 2));
        assert_eq!(m.carried, st(B, 3));
        // The same item on the cursor: the wheel is taken, nothing moves.
        m.carried = st(A, 3);
        let before = m.clicks;
        assert!(t.scroll(&mut m, Some(p), -1.0, &s));
        assert_eq!(m.clicks, before);
    }

    #[test]
    fn the_wheel_down_over_a_result_takes_whole_crafts() {
        let s = defaults();
        // The inventory: 0 result, 1..5 grid, 5.. the player's slots.
        let mut m = Sim::inventory(st(A, 4), 3);
        m.set(5, st(A, 62));
        let mut t = Tweaks::new();
        t.update_screen(Some("inventory"));
        t.scroll(&mut m, Some(0), -2.0, &s);
        assert_eq!(m.total(A), 70, "two crafts of four");
        assert_eq!((m.count(5), m.count(6)), (64, 6), "two topped the stack up, six in the first empty slot");
        assert_eq!(m.carried, None);
        assert_eq!(m.crafts, 1);
        // Up over a result does nothing; a full inventory takes no craft.
        t.scroll(&mut m, Some(0), 1.0, &s);
        assert_eq!(m.crafts, 1);
        for i in 5..41 {
            m.set(i, st(B, 64));
        }
        t.scroll(&mut m, Some(0), -1.0, &s);
        assert_eq!(m.crafts, 1);
    }

    #[test]
    fn the_wheel_up_pulls_whole_crafts_from_a_result() {
        let s = defaults();
        let mut m = Sim::inventory(st(A, 4), 2);
        m.set(5, st(A, 1));
        let mut t = Tweaks::new();
        t.update_screen(Some("inventory"));
        t.scroll(&mut m, Some(5), 1.0, &s);
        assert_eq!(m.count(5), 5, "one wheel step, one craft");
        assert_eq!(m.crafts, 1);
        m.set(5, st(A, 62));
        t.scroll(&mut m, Some(5), 1.0, &s);
        assert_eq!(m.count(5), 62, "a craft that doesn't fit whole stays");
    }

    #[test]
    fn the_wheel_puts_nothing_where_the_slot_refuses_it() {
        let s = defaults();
        let mut m = Sim::chest(1);
        m.slots[0].only = Some(SWORD);
        m.set(1, st(A, 10));
        let mut t = Tweaks::new();
        t.update_screen(Some("one"));
        assert!(t.scroll(&mut m, Some(1), -1.0, &s));
        assert_eq!(m.count(1), 10);
        assert_eq!(m.clicks, 0, "no target, no click");
        // A slot holding at most one: one sword goes, the second doesn't.
        m.set(2, st(SWORD, 1));
        m.set(3, st(SWORD, 1));
        t.scroll(&mut m, Some(2), -1.0, &s);
        t.scroll(&mut m, Some(3), -1.0, &s);
        assert_eq!((m.count(0), m.count(2), m.count(3)), (1, 0, 1));
    }

    #[test]
    fn a_bundle_right_drags_over_any_slot() {
        let s = defaults();
        let mut m = Sim::chest(27);
        m.carried = st(BUNDLE, 1);
        m.set(0, st(B, 2));
        m.set(1, st(A, 5));
        let mut t = Tweaks::new();
        t.update_screen(Some("chest"));
        t.press(&mut m, Some(0), true, &s);
        t.drag(&mut m, Some(1), false, &s);
        assert_eq!(m.clicks, 2, "slots of other items are clicked too");
        assert_eq!((m.slots[0].stack, m.slots[1].stack), (None, None), "the bundle took them in");
        assert_eq!(m.carried, st(BUNDLE, 1));
    }

    #[test]
    fn another_screen_starts_afresh() {
        let s = defaults();
        let mut m = Sim::chest(27);
        m.carried = st(A, 10);
        let mut t = Tweaks::new();
        t.update_screen(Some("chest 1"));
        t.press(&mut m, Some(0), true, &s);
        assert!(t.dragging());
        assert!(t.update_screen(Some("chest 2")));
        assert!(!t.dragging());
        t.drag(&mut m, Some(1), false, &s);
        assert_eq!(m.clicks, 0);
        assert!(!t.update_screen(None));
    }

    /// Random menus and random mouse work: no item is made or lost, and a
    /// wheel event leaves the cursor as it found it.
    #[test]
    fn random_mouse_work_keeps_every_item() {
        let mut seed: u64 = 0x5eed;
        let mut rand = |n: u64| {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (seed >> 33) % n
        };
        for round in 0..300 {
            let mut m = Sim::chest(9 * (1 + rand(3) as usize));
            for i in 0..m.slots.len() {
                if rand(3) > 0 {
                    let kind = [A, B, SWORD][rand(3) as usize];
                    let max = if kind == SWORD { 1 } else { 64 };
                    m.set(i, st(kind, 1 + rand(max) as u32));
                }
            }
            if rand(3) == 0 {
                // A slot that takes swords only (as an armour slot takes armour).
                let i = rand(9) as usize;
                m.slots[i].only = Some(SWORD);
                if m.slots[i].stack.is_some_and(|h| h.kind != SWORD) {
                    m.set(i, None);
                }
            }
            if rand(2) == 0 {
                m.carried = st([A, B][rand(2) as usize], 1 + rand(64) as u32);
            }
            let totals = [A, B, SWORD].map(|k| m.total(k));
            let s = Settings {
                order: if rand(2) == 0 { SearchOrder::FirstToLast } else { SearchOrder::LastToFirst },
                direction: Direction::ALL[rand(4) as usize],
                ..defaults()
            };
            let mut t = Tweaks::new();
            t.update_screen(Some("chest"));
            for _ in 0..40 {
                let slot = Some(rand(m.slots.len() as u64) as usize);
                match rand(5) {
                    0 => t.press(&mut m, slot, rand(2) == 0, &s),
                    1 | 2 => t.drag(&mut m, slot, rand(2) == 0, &s),
                    3 => t.release(rand(2) == 0),
                    _ => {
                        let before = m.carried;
                        let delta = (rand(7) as f64 - 3.0) * 0.75;
                        t.scroll(&mut m, slot, delta, &s);
                        assert_eq!(m.carried, before, "round {round}: the wheel gave the cursor back");
                    }
                }
                assert_eq!([A, B, SWORD].map(|k| m.total(k)), totals, "round {round}");
                assert!(m.slots.iter().all(|x| x.stack.is_none_or(|h| h.count >= 1 && h.count <= x.limit.min(h.max))));
            }
        }
    }

    #[test]
    fn settings_round_trip_and_cycle() {
        let mut s = Settings::default();
        assert_eq!(s.labels(true).len(), Settings::ROWS);
        assert_eq!(s.labels(false).len(), Settings::ROWS);
        for row in 0..Settings::ROWS {
            s.click(row);
        }
        assert!(!s.rmb && !s.lmb_with_item && !s.lmb_without_item && !s.wheel);
        assert_eq!(
            (s.order, s.direction, s.scaling),
            (SearchOrder::FirstToLast, Direction::Inverted, Scaling::AlwaysOne)
        );
        let stored = s.stored();
        let back = Settings::read(|k| stored.iter().find(|(key, _)| *key == k).map(|(_, v)| v.clone()));
        assert_eq!(back, s);
        assert_eq!(Settings::read(|_| Some("junk".into())), Settings::default());
        for _ in 0..4 {
            s.click(5);
        }
        assert_eq!(s.direction, Direction::Inverted, "the direction cycles through four");
        s.cycle(5, true);
        assert_eq!(s.direction, Direction::Normal, "Shift or the wheel up goes back");
        s.cycle(5, true);
        assert_eq!(s.direction, Direction::PositionAwareInverted);
        let d = Settings::default();
        assert_eq!(d.labels(false)[0], "RMB Tweak: ON");
        assert_eq!(d.value(4, false), Err("Last to First"));
        assert_eq!(d.value(3, true), Ok(true));
    }
}
