// Traveler's Backpack for Horizon: the backpacks' tiers and what their menus
// hold. No game types here, so `cargo test` runs it alone.
//
// Ported from Traveler's Backpack by Tiviacz1337 (LGPL-3.0,
// https://github.com/Tiviacz1337/Travelers-Backpack).

/// The content mod's namespace: the items, blocks and menu types.
pub const NS: &str = "travelersbackpack";

/// `Tiers.Tier`: storage, upgrade and tool slots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tier {
    pub name: &'static str,
    pub storage: usize,
    pub upgrades: usize,
    pub tools: usize,
}

impl Tier {
    /// Every slot of the backpack's menu: storage, then upgrades, then tools.
    pub const fn slots(self) -> usize {
        self.storage + self.upgrades + self.tools
    }
    /// This tier with its own sizes (`STORAGE_SLOTS`, `UPGRADE_SLOTS`,
    /// `TOOL_SLOTS`), in the config's ranges.
    /// Horizon's menus hold at most 256 slots: with the most upgrades and
    /// tools the config allows, the storage is cut to what still fits.
    pub fn sized(self, storage: usize, upgrades: usize, tools: usize) -> Tier {
        let (upgrades, tools) = (upgrades.min(MAX_UPGRADES), tools.min(MAX_TOOLS));
        let room = MENU_SLOTS - tools - upgrades * (1 + TAB_SLOTS);
        Tier { storage: storage.clamp(1, MAX_STORAGE.min(room)), upgrades, tools, ..self }
    }
    /// Its ordinal (`ModDataComponents.TIER`).
    pub fn index(self) -> i32 {
        TIERS.iter().position(|t| t.name == self.name).unwrap_or(0) as i32
    }
}

/// The config's ranges (`inventorySlotCount` 1–154, `upgradeSlotCount`
/// 0–10, `toolSlotCount` 0–8).
pub const MAX_STORAGE: usize = 154;
pub const MAX_UPGRADES: usize = 10;
pub const MAX_TOOLS: usize = 8;
/// Every menu type's slots, the most a Horizon menu has: a backpack's and
/// its tabs', the rest hidden.
pub const MENU_SLOTS: usize = 256;
/// A backpack's own sizes (`ModDataComponents`).
pub const STORAGE_COMPONENT: &str = "travelersbackpack:storage_slots";
pub const UPGRADES_COMPONENT: &str = "travelersbackpack:upgrade_slots";
pub const TOOLS_COMPONENT: &str = "travelersbackpack:tool_slots";

pub const TIERS: [Tier; 5] = [
    Tier { name: "leather", storage: 27, upgrades: 2, tools: 2 },
    Tier { name: "iron", storage: 45, upgrades: 3, tools: 3 },
    Tier { name: "gold", storage: 63, upgrades: 4, tools: 4 },
    Tier { name: "diamond", storage: 81, upgrades: 5, tools: 5 },
    Tier { name: "netherite", storage: 99, upgrades: 6, tools: 6 },
];

/// Where a backpack's menu was opened from, which its menu type tells the
/// client: placed (no button), in a hand (the equip button) or worn on the
/// back (the unequip button), as `BackpackScreen` picks its buttons.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Opened {
    Placed,
    Held,
    Worn,
}

impl Opened {
    const fn suffix(self) -> &'static str {
        match self {
            Opened::Placed => "",
            Opened::Held => "_held",
            Opened::Worn => "_worn",
        }
    }
}

/// The slots each upgrade slot's tab may show (`getUpgradeSlots`): after the
/// backpack's own (storage, upgrades, tools), one block of these per upgrade
/// slot, in order; a tab uses the first of its block.
pub const TAB_SLOTS: usize = 10;

impl Tier {
    /// The menu type of this tier opened from `opened` (`travelersbackpack:iron_worn`).
    pub fn menu_for(self, opened: Opened) -> String {
        format!("{NS}:{}{}", self.name, opened.suffix())
    }
    /// Its menu's slots: the backpack's, then each upgrade slot's tab block.
    pub const fn menu_slots(self) -> usize {
        self.slots() + self.upgrades * TAB_SLOTS
    }
    /// `wrapper.isExtended()`: past 81 slots the rows are eleven wide.
    pub const fn extended(self) -> bool {
        self.storage > 81
    }
    /// The storage's rows.
    pub const fn rows(self) -> usize {
        self.storage.div_ceil(if self.extended() { 11 } else { 9 })
    }
}

/// The tier and the opening of menu type `menu`.
pub fn menu_kind(menu: &str) -> Option<(Tier, Opened)> {
    let name = menu.strip_prefix(NS)?.strip_prefix(':')?;
    [Opened::Held, Opened::Worn, Opened::Placed].into_iter().find_map(|o| {
        let tier = name.strip_suffix(o.suffix())?;
        TIERS.into_iter().find(|t| t.name == tier).map(|t| (t, o))
    })
}

/// The tier whose menu type is `menu`, however it was opened.
pub fn tier_of_menu(menu: &str) -> Option<Tier> {
    menu_kind(menu).map(|(t, _)| t)
}

/// What an upgrade slot shows (`UpgradeBase.getTabSize`, `getUpgradeSlotsPosition`):
/// a plain slot, or the tab of the upgrade in it, open, with its own slots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Plain,
    Tanks,
    Crafting,
    Furnace,
    Smoker,
    BlastFurnace,
    Void,
    Refill,
    Jukebox,
    Pickup,
    Magnet,
    Feeding,
}

impl Tab {
    const ALL: [Tab; 12] = [
        Tab::Plain,
        Tab::Tanks,
        Tab::Crafting,
        Tab::Furnace,
        Tab::Smoker,
        Tab::BlastFurnace,
        Tab::Void,
        Tab::Refill,
        Tab::Jukebox,
        Tab::Pickup,
        Tab::Magnet,
        Tab::Feeding,
    ];
    /// The tab of upgrade `item` in a slot.
    pub fn of(item: Option<&str>) -> Tab {
        match item.and_then(|i| i.strip_prefix("travelersbackpack:")) {
            Some("tanks_upgrade") => Tab::Tanks,
            Some("crafting_upgrade") => Tab::Crafting,
            Some("furnace_upgrade") => Tab::Furnace,
            Some("smoker_upgrade") => Tab::Smoker,
            Some("blast_furnace_upgrade") => Tab::BlastFurnace,
            Some("void_upgrade") => Tab::Void,
            Some("refill_upgrade") => Tab::Refill,
            Some("jukebox_upgrade") => Tab::Jukebox,
            Some("pickup_upgrade") => Tab::Pickup,
            Some("magnet_upgrade") => Tab::Magnet,
            Some("feeding_upgrade") => Tab::Feeding,
            _ => Tab::Plain,
        }
    }
    /// Its byte in the menu's extra data.
    pub fn code(self) -> u8 {
        Self::ALL.iter().position(|&t| t == self).unwrap_or(0) as u8
    }
    pub fn from_code(code: u8) -> Tab {
        Self::ALL.get(usize::from(code)).copied().unwrap_or(Tab::Plain)
    }
    /// The smelting tabs' recipe type.
    pub fn cooking(self) -> Option<&'static str> {
        match self {
            Tab::Furnace => Some("minecraft:smelting"),
            Tab::Smoker => Some("minecraft:smoking"),
            Tab::BlastFurnace => Some("minecraft:blasting"),
            _ => None,
        }
    }
    /// The open tab's width and height (a plain slot's is 24 square).
    pub fn size(self) -> [f32; 2] {
        match self {
            Tab::Plain => [24.0, 24.0],
            Tab::Tanks => [51.0, 72.0],
            Tab::Crafting => [66.0, 112.0],
            Tab::Furnace | Tab::Smoker | Tab::BlastFurnace => [66.0, 82.0],
            // `FilterUpgradeBase`: 66 by 49, a row of 18 for each three filter slots.
            Tab::Void | Tab::Pickup | Tab::Magnet | Tab::Feeding => [66.0, 103.0],
            Tab::Refill => [66.0, 82.0],
            Tab::Jukebox => [66.0, 46.0],
        }
    }
    /// Its slots, from the tab's top left: tanks' inputs and outputs; the
    /// crafting grid and its result; a furnace's input, fuel and output; a
    /// filter's nine; the jukebox's disc.
    pub fn slots(self) -> Vec<[f32; 2]> {
        let grid = || (0..9).map(|k| [7.0 + 18.0 * (k % 3) as f32, 23.0 + 18.0 * (k / 3) as f32]);
        match self {
            Tab::Plain => Vec::new(),
            Tab::Tanks => vec![[7.0, 23.0], [7.0, 49.0], [28.0, 23.0], [28.0, 49.0]],
            Tab::Crafting => grid().chain([[25.0, 89.0]]).collect(),
            Tab::Furnace | Tab::Smoker | Tab::BlastFurnace => vec![[7.0, 23.0], [7.0, 59.0], [43.0, 41.0]],
            // Under the filter buttons (`getUpgradeSlotsPosition`).
            Tab::Void | Tab::Pickup | Tab::Magnet | Tab::Feeding => {
                (0..9).map(|k| [7.0 + 18.0 * (k % 3) as f32, 44.0 + 18.0 * (k / 3) as f32]).collect()
            }
            Tab::Refill => grid().collect(),
            Tab::Jukebox => vec![[7.0, 23.0]],
        }
    }
    /// Whether the slots' items stay in the upgrade (a crafting grid, a
    /// furnace, a filter, a disc); a tank's buckets go back to the player.
    /// A filter tab with buttons (`IFilter`): how many states each of its three has.
    pub fn filter_states(self) -> Option<[i32; 3]> {
        match self {
            Tab::Void | Tab::Pickup => Some([3, 3, 2]),
            Tab::Magnet => Some([2, 3, 2]),
            Tab::Feeding => Some([2, 3, 2]),
            _ => None,
        }
    }
    /// Its buttons as they start (`FILTER_SETTINGS`).
    pub fn filter_default(self) -> [i32; 3] {
        match self {
            Tab::Void => [0, 0, 1],
            Tab::Feeding => [1, 1, 0],
            _ => [1, 0, 1],
        }
    }
    pub fn keeps_items(self) -> bool {
        !matches!(self, Tab::Plain | Tab::Tanks)
    }
}

/// Each upgrade slot's top (`addBackpackUpgradeSlots`): a tab's height, or 24,
/// plus one apart.
pub fn upgrade_slot_ys(tabs: &[Tab]) -> Vec<f32> {
    let mut y = 15.0 + 18.0;
    tabs.iter()
        .map(|t| {
            let at = y;
            y += t.size()[1] + 1.0;
            at
        })
        .collect()
}

/// The backpack designs (`ModBlocks`), each a block and its item.
pub const DESIGNS: [&str; 44] = [
    "standard", "netherite", "diamond", "gold", "emerald", "iron", "lapis", "redstone", "coal", "quartz",
    "bookshelf", "end", "nether", "sandstone", "snow", "sponge", "cake", "cactus", "hay", "melon", "pumpkin",
    "creeper", "dragon", "enderman", "blaze", "ghast", "magma_cube", "skeleton", "spider", "wither", "warden",
    "bat", "bee", "wolf", "fox", "ocelot", "horse", "cow", "pig", "sheep", "squid", "chicken", "villager",
    "iron_golem",
];

/// A backpack (`travelersbackpack:fox`): its item, or its block.
pub fn is_backpack(id: &str) -> bool {
    id.strip_prefix(NS).and_then(|r| r.strip_prefix(':')).is_some_and(|d| DESIGNS.contains(&d))
}

/// The custom component holding a backpack's tier (`ModDataComponents.TIER`).
pub const TIER_COMPONENT: &str = "travelersbackpack:tier";
/// A placed backpack's block entity, and the most slots any tier has.
pub const BLOCK_ENTITY: &str = "travelersbackpack:backpack";
pub const MAX_SLOTS: usize = MAX_STORAGE + MAX_UPGRADES + MAX_TOOLS;

/// The tier of ordinal `n` (`Tiers.of`), leather for any other.
pub fn tier(n: i32) -> Tier {
    usize::try_from(n).ok().and_then(|n| TIERS.get(n).copied()).unwrap_or(TIERS[0])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiers_hold_storage_upgrades_and_tools() {
        assert_eq!(TIERS[0].slots(), 31);
        assert_eq!(TIERS[4].slots(), 111);
        assert!(TIERS.iter().all(|t| t.slots() <= 256));
    }

    #[test]
    fn menus_name_their_tier() {
        assert_eq!(TIERS[1].menu_for(Opened::Placed), "travelersbackpack:iron");
        assert_eq!(tier_of_menu("travelersbackpack:gold"), Some(TIERS[2]));
        assert_eq!(tier_of_menu("minecraft:generic_9x3"), None);
        assert_eq!(menu_kind("travelersbackpack:iron_worn"), Some((TIERS[1], Opened::Worn)));
        assert_eq!(menu_kind("travelersbackpack:netherite_held"), Some((TIERS[4], Opened::Held)));
        assert_eq!(menu_kind("travelersbackpack:leather"), Some((TIERS[0], Opened::Placed)));
        assert_eq!(TIERS[3].menu_for(Opened::Worn), "travelersbackpack:diamond_worn");
        assert_eq!(TIERS[0].menu_slots(), 31 + 20);
        assert_eq!(Tab::of(Some("travelersbackpack:crafting_upgrade")), Tab::Crafting);
        assert_eq!(Tab::from_code(Tab::Smoker.code()), Tab::Smoker);
        assert_eq!(Tab::Crafting.slots().len(), 10);
        assert!(Tab::Crafting.slots().len() <= TAB_SLOTS && Tab::Void.slots().len() <= TAB_SLOTS);
        assert_eq!(upgrade_slot_ys(&[Tab::Plain, Tab::Tanks, Tab::Plain]), vec![33.0, 58.0, 131.0]);
        assert_eq!(menu_kind("travelersbackpack:iron_x"), None);
        assert!(is_backpack("travelersbackpack:standard"));
        assert!(is_backpack("travelersbackpack:iron_golem"));
        assert!(!is_backpack("travelersbackpack:hose"));
        assert!(!is_backpack("minecraft:chest"));
        assert_eq!(tier(4), TIERS[4]);
        assert_eq!(tier(9), TIERS[0]);
        assert!(TIERS.iter().all(|t| t.slots() <= MAX_SLOTS));
        assert_eq!(TIERS[2].sized(200, 3, 1).storage, MAX_STORAGE);
        assert!(TIERS.iter().all(|t| t.sized(154, 10, 8).menu_slots() <= MENU_SLOTS));
        assert!(TIERS.iter().all(|t| t.menu_slots() <= MENU_SLOTS));
    }
}
