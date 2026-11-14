// `TravelersBackpackConfig`: the mod's three config files in the game's
// `config/` folder, in the Java mod's own format (TOML, the same sections
// and names): `travelersbackpack-server.toml`, `-common.toml` and
// `-client.toml`. A missing file is written with the defaults; a missing or
// mistyped value takes its default. Read once, at first use.
use std::path::Path;
use std::sync::OnceLock;

pub(crate) struct Config {
    // server.backpackSettings
    pub tank_capacity: [i32; 5],
    /// Each tier's storage, upgrade and tool slots (`inventorySlotCount`,
    /// `upgradeSlotCount`, `toolSlotCount`): a new backpack's.
    pub tier_sizes: [[usize; 3]; 5],
    /// The filter upgrades' `filterSlotCount` and `slotsInRow`: void,
    /// pickup, magnet, feeding, refill.
    pub filter_layouts: [(usize, usize); 5],
    pub right_click_equip: bool,
    pub right_click_unequip: bool,
    pub allow_only_equipped_backpack: bool,
    pub allow_opening_from_slot: bool,
    pub allow_tool_swapping: bool,
    pub never_despawn: bool,
    pub fire_resistant: bool,
    pub explosion_resistant: bool,
    pub invulnerable_backpack: bool,
    pub tool_slots_accept_everything: bool,
    pub tool_slots_acceptable_items: Vec<String>,
    pub blacklisted_items: Vec<String>,
    pub allow_shulker_boxes: bool,
    pub void_protection: bool,
    pub backpack_death_place: bool,
    pub backpack_force_death_place: bool,
    pub quick_sleeping_bag: bool,
    pub enable_sleeping_bag_spawn_point: bool,
    /// `backSlotIntegration`: with Accessories installed, the backpack is worn in its Back slot.
    pub back_slot_integration: bool,
    // server.backpackUpgrades
    pub enable_sleeping_bag: bool,
    /// Upgrades turned off, by item path (`tanks_upgrade`).
    pub disabled_upgrades: Vec<&'static str>,
    pub feeding_tick_rate: u64,
    pub magnet_pull_range: i32,
    pub magnet_tick_rate: u64,
    pub refill_tick_rate: u64,
    // server.world
    pub spawn_entities_with_backpack: bool,
    pub spawn_chance: f64,
    pub overworld_entity_types: Vec<String>,
    pub nether_entity_types: Vec<String>,
    pub overworld_backpacks: Vec<String>,
    pub nether_backpacks: Vec<String>,
    // server.backpackAbilities
    pub enable_backpack_abilities: bool,
    pub force_ability_enabled: bool,
    pub allowed_abilities: Vec<String>,
    /// `backpack, effect, min duration, max duration, amplifier`.
    pub backpack_effects: Vec<(String, String, i32, i32, i32)>,
    /// `backpack, min seconds, max seconds`.
    pub cooldowns: Vec<(String, i32, i32)>,
    // server.slownessDebuff
    pub too_many_backpacks_slowness: bool,
    pub max_number_of_backpacks: i32,
    pub slowness_per_excessed_backpack: f64,
    // common (`enableLoot`, `enableVillagerTrade`): read where the loot tables and trades load.
    // client
    pub show_backpack_icon_in_inventory: bool,
    pub send_backpack_coordinates_message: bool,
    pub obtain_tips: bool,
    pub render_tools: bool,
    pub tools_swap_on_close: bool,
    pub tools_show_tooltip: bool,
    pub tools_display_empty_slots: bool,
    pub tools_allow_hotbar_scrolling: bool,
    pub tools_render_backpack_icon_in_center: bool,
    pub tools_opacity: f32,
    pub tools_offset: [i32; 2],
    pub overlay_enabled: bool,
    pub overlay_offset: [i32; 2],
}

const SERVER: &str = r#"
[server]

	[server.backpackSettings]
		#Enables equipping the backpack on right-click from the ground
		rightClickEquip = true
		#Enables unequipping the backpack on right-click on the ground with empty hand
		rightClickUnequip = false
		#Allows to use only equipped backpack
		allowOnlyEquippedBackpack = false
		#Allows opening a backpack from an inventory slot with the backpack key
		allowOpeningFromSlot = false
		#Prevents more than one player from accessing the same placed backpack
		preventMultiplePlayersAccess = false
		#Allows swapping tools with the tool slots (the tools menu)
		allowToolSwapping = true
		#Backpack immune to despawn
		neverDespawn = true
		#Backpack immune to fire
		fireResistant = true
		#Backpack immune to explosions
		explosionResistant = true
		#Backpack immune to any damage source (lava, fire), can't be destroyed, never disappears as floating item
		invulnerableBackpack = true
		#Tool slots accept any item
		toolSlotsAcceptEverything = false
		#List of items that can be put in tool slots (Use registry names, for example: minecraft:apple)
		toolSlotsAcceptableItems = []
		#List of items that can't be put in backpack inventory (Use registry names, for example: minecraft:apple)
		blacklistedItems = []
		allowShulkerBoxes = false
		#Prevents backpack disappearing in void, spawns floating backpack above minimum Y when player dies in void
		voidProtection = true
		#Places backpack at place where player died
		backpackDeathPlace = true
		#Places backpack at place where player died, replacing all blocks that are breakable and do not have inventory (backpackDeathPlace must be true in order to work)
		backpackForceDeathPlace = false
		#Enables quick sleeping bag (sleeping without unequipping the backpack)
		quickSleepingBag = true
		enableSleepingBagSpawnPoint = false
		#Backpacks can only be equipped in the Curios/Accessories 'Back' slot, provided those mods are installed. If set to false, backpacks can only be equipped by clicking the button in the Backpack GUI. This setting can be changed without unequipping the backpack. An already equipped backpack will not disappear and can be retrieved from the player's inventory.
		backSlotIntegration = true

		#Leather Tier Backpack Settings
		[server.backpackSettings.leatherTierBackpack]
			#Number of inventory slots for the tier
			# Default: 27
			# Range: 1 ~ 154
			inventorySlotCount = 27
			#Number of upgrade slots for the tier
			# Default: 2
			# Range: 0 ~ 10
			upgradeSlotCount = 2
			#Number of tool slots for the tier
			# Default: 2
			# Range: 0 ~ 8
			toolSlotCount = 2
			#Tank capacity per row of backpack storage, 81000 equals 1 Bucket on Fabric, 1000 on Forge and Horizon
			tankCapacity = 1000

		#Iron Tier Backpack Settings
		[server.backpackSettings.ironTierBackpack]
			#Number of inventory slots for the tier
			# Default: 45
			# Range: 1 ~ 154
			inventorySlotCount = 45
			#Number of upgrade slots for the tier
			# Default: 3
			# Range: 0 ~ 10
			upgradeSlotCount = 3
			#Number of tool slots for the tier
			# Default: 3
			# Range: 0 ~ 8
			toolSlotCount = 3
			tankCapacity = 1000

		#Gold Tier Backpack Settings
		[server.backpackSettings.goldTierBackpack]
			#Number of inventory slots for the tier
			# Default: 63
			# Range: 1 ~ 154
			inventorySlotCount = 63
			#Number of upgrade slots for the tier
			# Default: 4
			# Range: 0 ~ 10
			upgradeSlotCount = 4
			#Number of tool slots for the tier
			# Default: 4
			# Range: 0 ~ 8
			toolSlotCount = 4
			tankCapacity = 1000

		#Diamond Tier Backpack Settings
		[server.backpackSettings.diamondTierBackpack]
			#Number of inventory slots for the tier
			# Default: 81
			# Range: 1 ~ 154
			inventorySlotCount = 81
			#Number of upgrade slots for the tier
			# Default: 5
			# Range: 0 ~ 10
			upgradeSlotCount = 5
			#Number of tool slots for the tier
			# Default: 5
			# Range: 0 ~ 8
			toolSlotCount = 5
			tankCapacity = 1000

		#Netherite Tier Backpack Settings
		[server.backpackSettings.netheriteTierBackpack]
			#Number of inventory slots for the tier
			# Default: 99
			# Range: 1 ~ 154
			inventorySlotCount = 99
			#Number of upgrade slots for the tier
			# Default: 6
			# Range: 0 ~ 10
			upgradeSlotCount = 6
			#Number of tool slots for the tier
			# Default: 6
			# Range: 0 ~ 8
			toolSlotCount = 6
			tankCapacity = 1000

	[server.backpackUpgrades]
		enableSleepingBag = true
		enableTanksUpgrade = true
		enableCraftingUpgrade = true
		enableFurnaceUpgrade = true
		enableSmokerUpgrade = true
		enableBlastFurnaceUpgrade = true
		enableJukeboxUpgrade = true
		enableLanternUpgrade = true

		[server.backpackUpgrades.pickupUpgradeSettings]
			enablePickupUpgrade = true
			#Range 1 ~ 20
			filterSlotCount = 9
			#Range 1 ~ 5
			slotsInRow = 3

		[server.backpackUpgrades.magnetUpgradeSettings]
			enableMagnetUpgrade = true
			#Range 1 ~ 20
			filterSlotCount = 9
			#Range 1 ~ 5
			slotsInRow = 3
			#Range 1 ~ 20
			pullRange = 5
			#Range 1 ~ 1000
			tickRate = 10

		[server.backpackUpgrades.feedingUpgradeSettings]
			enableFeedingUpgrade = true
			#Range 1 ~ 20
			filterSlotCount = 9
			#Range 1 ~ 5
			slotsInRow = 3
			#Range 1 ~ 1000
			tickRate = 100

		[server.backpackUpgrades.refillUpgradeSettings]
			enableRefillUpgrade = true
			#Range 1 ~ 20
			filterSlotCount = 9
			#Range 1 ~ 5
			slotsInRow = 3
			#Range 1 ~ 1000
			tickRate = 5

		[server.backpackUpgrades.voidUpgradeSettings]
			enableVoidUpgrade = true
			#Range 1 ~ 20
			filterSlotCount = 9
			#Range 1 ~ 5
			slotsInRow = 3

	[server.world]
		#Enables chance to spawn Zombie, Skeleton, Wither Skeleton, Piglin or Enderman with random backpack equipped
		spawnEntitiesWithBackpack = true
		#Defines spawn chance of entity with a backpack
		#Range 0.0 ~ 1.0
		chance = 0.005
		#List of overworld entity types that can spawn with equipped backpack. DO NOT ADD anything to this list, because the game will crash, remove entries if mob should not spawn with backpack
		possibleOverworldEntityTypes = ["minecraft:zombie", "minecraft:skeleton", "minecraft:enderman"]
		#List of nether entity types that can spawn with equipped backpack. DO NOT ADD anything to this list, because the game will crash, remove entries if mob should not spawn with backpack
		possibleNetherEntityTypes = ["minecraft:wither_skeleton", "minecraft:piglin"]
		#List of backpacks that can spawn on overworld mobs
		overworldBackpacks = ["travelersbackpack:standard", "travelersbackpack:diamond", "travelersbackpack:gold", "travelersbackpack:emerald", "travelersbackpack:iron", "travelersbackpack:lapis", "travelersbackpack:redstone", "travelersbackpack:coal", "travelersbackpack:bookshelf", "travelersbackpack:sandstone", "travelersbackpack:snow", "travelersbackpack:sponge", "travelersbackpack:cake", "travelersbackpack:cactus", "travelersbackpack:hay", "travelersbackpack:melon", "travelersbackpack:pumpkin", "travelersbackpack:creeper", "travelersbackpack:enderman", "travelersbackpack:skeleton", "travelersbackpack:spider", "travelersbackpack:bee", "travelersbackpack:wolf", "travelersbackpack:fox", "travelersbackpack:ocelot", "travelersbackpack:horse", "travelersbackpack:cow", "travelersbackpack:pig", "travelersbackpack:sheep", "travelersbackpack:chicken", "travelersbackpack:squid"]
		#List of backpacks that can spawn on nether mobs
		netherBackpacks = ["travelersbackpack:quartz", "travelersbackpack:nether", "travelersbackpack:blaze", "travelersbackpack:ghast", "travelersbackpack:magma_cube", "travelersbackpack:wither"]

	[server.backpackAbilities]
		enableBackpackAbilities = true
		#Newly crafted backpacks will have ability enabled by default
		forceAbilityEnabled = true
		#List of backpacks that are allowed to have an ability. DO NOT ADD anything to this list, because the game will crash, remove entries if backpack should not have ability
		allowedAbilities = ["travelersbackpack:netherite", "travelersbackpack:diamond", "travelersbackpack:gold", "travelersbackpack:emerald", "travelersbackpack:iron", "travelersbackpack:lapis", "travelersbackpack:redstone", "travelersbackpack:bookshelf", "travelersbackpack:sponge", "travelersbackpack:cake", "travelersbackpack:cactus", "travelersbackpack:melon", "travelersbackpack:pumpkin", "travelersbackpack:creeper", "travelersbackpack:dragon", "travelersbackpack:enderman", "travelersbackpack:blaze", "travelersbackpack:ghast", "travelersbackpack:magma_cube", "travelersbackpack:spider", "travelersbackpack:wither", "travelersbackpack:warden", "travelersbackpack:bat", "travelersbackpack:bee", "travelersbackpack:ocelot", "travelersbackpack:cow", "travelersbackpack:chicken", "travelersbackpack:squid", "travelersbackpack:hay", "travelersbackpack:fox"]
		#List of effect abilities associated with backpacks, you can modify this list as you wish. Formatting: "<backpack_registry_name>, <status_effect_registry_name>, <min_duration_ticks>, <max_duration_ticks>, <amplifier>"
		backpackEffects = ["travelersbackpack:bat, minecraft:night_vision, 260, 300, 0", "travelersbackpack:magma_cube, minecraft:fire_resistance, 260, 300, 0", "travelersbackpack:squid, minecraft:water_breathing, 260, 300, 0", "travelersbackpack:squid, minecraft:night_vision, 260, 300, 0", "travelersbackpack:dragon, minecraft:regeneration, 260, 300, 0", "travelersbackpack:dragon, minecraft:strength, 250, 290, 0", "travelersbackpack:quartz, minecraft:haste, 260, 300, 0", "travelersbackpack:fox, minecraft:jump_boost, 260, 300, 0"]
		#List of cooldowns that are being applied after ability usage, the backpacks on the list are all that currently have cooldowns, adding additional backpack will not give it cooldown. Formatting: "<backpack_registry_name>, <min_possible_cooldown_seconds>, <max_possible_cooldown_seconds>"
		cooldowns = ["travelersbackpack:creeper, 1200, 1800", "travelersbackpack:cow, 480, 540", "travelersbackpack:chicken, 360, 600", "travelersbackpack:cake, 360, 480", "travelersbackpack:melon, 120, 480"]

	[server.slownessDebuff]
		#Player gets slowness effect, if carries too many backpacks in inventory
		tooManyBackpacksSlowness = false
		#Maximum number of backpacks, which can be carried in inventory, without slowness effect
		#Range 1 ~ 37
		maxNumberOfBackpacks = 3
		#Range 0.1 ~ 5.0
		slownessPerExcessedBackpack = 1.0
"#;

const COMMON: &str = r#"
[common]
	#Enables backpacks spawning in loot chests
	enableLoot = true
	#Enables trade for Villager Backpack in Wandering Trader
	enableVillagerTrade = true
"#;

const CLIENT: &str = r#"
[client]
	#Whether the backpack icon should be visible in player's inventory
	showBackpackIconInInventory = true
	#Whether to send the backpack coordinates message
	sendBackpackCoordinatesMessage = true
	#Enables tip, how to obtain a backpack, if there's no crafting recipe for it
	obtainTips = true
	#Render tools in tool slots on the backpack, while worn
	renderTools = true

	#The position of the Tools Overlay on the screen
	[client.toolsOverlay]
		#If true, the currently held item will be swapped with the selected tool belt slot when closing the tool belt GUI
		swapOnClose = true
		#Displays the full tooltip when hovering over an item in the tool belt. If false, only the item name is shown
		showTooltip = false
		#The tool belt GUI will always display all the slots, even when empty
		displayEmptySlots = false
		#Allows to scroll through hotbar slots while the tool belt GUI is open
		allowHotbarScrolling = false
		#Render backpack icon in the center of the tools overlay
		renderBackpackIconInCenter = true
		#Overlay opacity
		#Range 0.0 ~ 1.0
		opacity = 0.75
		#X offset
		offsetX = 0
		#Y offset
		offsetY = 0

	#The position of the Overlay on the screen
	[client.overlay]
		#Enables tanks and tool slots overlay, while backpack is worn
		enableOverlay = true
		#Offsets to left side
		offsetX = 20
		#Offsets to up
		offsetY = 30
"#;

/// A config file's table: the file's when it reads, else the defaults'
/// (written out when the file is missing).
fn table(dir: Option<&Path>, name: &str, defaults: &str) -> (toml::Table, toml::Table) {
    let own: toml::Table = defaults.parse().unwrap_or_default();
    let Some(dir) = dir else {
        return (toml::Table::new(), own);
    };
    let path = dir.join(name);
    match std::fs::read_to_string(&path) {
        Ok(text) => (text.parse().unwrap_or_default(), own),
        Err(_) => {
            let _ = std::fs::write(&path, defaults.trim_start());
            (toml::Table::new(), own)
        }
    }
}

/// The value at `path` (`a.b.c`) in the file, else in the defaults.
struct Values(toml::Table, toml::Table);

impl Values {
    fn at(&self, path: &str) -> Option<&toml::Value> {
        Self::get(&self.0, path).or_else(|| Self::get(&self.1, path))
    }
    fn get<'a>(t: &'a toml::Table, path: &str) -> Option<&'a toml::Value> {
        let mut parts = path.split('.');
        let mut v = t.get(parts.next()?)?;
        for p in parts {
            v = v.as_table()?.get(p)?;
        }
        Some(v)
    }
    fn flag(&self, path: &str) -> bool {
        Self::get(&self.0, path).and_then(toml::Value::as_bool).or_else(|| Self::get(&self.1, path).and_then(toml::Value::as_bool)).unwrap_or(false)
    }
    fn int(&self, path: &str, min: i64, max: i64) -> i64 {
        let own = Self::get(&self.1, path).and_then(toml::Value::as_integer).unwrap_or(min);
        Self::get(&self.0, path).and_then(toml::Value::as_integer).filter(|n| (min..=max).contains(n)).unwrap_or(own)
    }
    fn float(&self, path: &str, min: f64, max: f64) -> f64 {
        let number = |v: &toml::Value| v.as_float().or_else(|| v.as_integer().map(|n| n as f64));
        let own = Self::get(&self.1, path).and_then(number).unwrap_or(min);
        Self::get(&self.0, path).and_then(number).filter(|n| (min..=max).contains(n)).unwrap_or(own)
    }
    fn list(&self, path: &str) -> Vec<String> {
        self.at(path)
            .and_then(toml::Value::as_array)
            .map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.trim().to_owned())).collect())
            .unwrap_or_default()
    }
}

fn load(dir: Option<&Path>) -> Config {
    let (file, own) = table(dir, "travelersbackpack-server.toml", SERVER);
    let s = Values(file, own);
    // Written with its defaults when missing; the loot and the trade read it themselves.
    let _ = table(dir, "travelersbackpack-common.toml", COMMON);
    let (file, own) = table(dir, "travelersbackpack-client.toml", CLIENT);
    let k = Values(file, own);
    let b = |name: &str| format!("server.backpackSettings.{name}");
    let u = |name: &str| format!("server.backpackUpgrades.{name}");
    let tank = |tier: &str| s.int(&b(&format!("{tier}TierBackpack.tankCapacity")), 1, 100_000) as i32;
    let size = |tier: &str| {
        let n = |k: &str, min: i64, max: i64| s.int(&b(&format!("{tier}TierBackpack.{k}")), min, max) as usize;
        [n("inventorySlotCount", 1, 154), n("upgradeSlotCount", 0, 10), n("toolSlotCount", 0, 8)]
    };
    let numbers = |entry: &str| -> Vec<String> { entry.split(',').map(|p| p.trim().to_owned()).collect() };
    let upgrades: [(&'static str, String); 11] = [
        ("tanks_upgrade", u("enableTanksUpgrade")),
        ("crafting_upgrade", u("enableCraftingUpgrade")),
        ("furnace_upgrade", u("enableFurnaceUpgrade")),
        ("smoker_upgrade", u("enableSmokerUpgrade")),
        ("blast_furnace_upgrade", u("enableBlastFurnaceUpgrade")),
        ("jukebox_upgrade", u("enableJukeboxUpgrade")),
        ("lantern_upgrade", u("enableLanternUpgrade")),
        ("pickup_upgrade", u("pickupUpgradeSettings.enablePickupUpgrade")),
        ("magnet_upgrade", u("magnetUpgradeSettings.enableMagnetUpgrade")),
        ("feeding_upgrade", u("feedingUpgradeSettings.enableFeedingUpgrade")),
        ("refill_upgrade", u("refillUpgradeSettings.enableRefillUpgrade")),
    ];
    let mut disabled_upgrades: Vec<&'static str> = upgrades.iter().filter(|(_, key)| !s.flag(key)).map(|(name, _)| *name).collect();
    if !s.flag(&u("voidUpgradeSettings.enableVoidUpgrade")) {
        disabled_upgrades.push("void_upgrade");
    }
    Config {
        tank_capacity: [tank("leather"), tank("iron"), tank("gold"), tank("diamond"), tank("netherite")],
        tier_sizes: [size("leather"), size("iron"), size("gold"), size("diamond"), size("netherite")],
        filter_layouts: ["void", "pickup", "magnet", "feeding", "refill"].map(|n| {
            let key = |k: &str| u(&format!("{n}UpgradeSettings.{k}"));
            (s.int(&key("filterSlotCount"), 1, 20) as usize, s.int(&key("slotsInRow"), 1, 5) as usize)
        }),
        right_click_equip: s.flag(&b("rightClickEquip")),
        right_click_unequip: s.flag(&b("rightClickUnequip")),
        allow_only_equipped_backpack: s.flag(&b("allowOnlyEquippedBackpack")),
        allow_opening_from_slot: s.flag(&b("allowOpeningFromSlot")),
        allow_tool_swapping: s.flag(&b("allowToolSwapping")),
        never_despawn: s.flag(&b("neverDespawn")),
        fire_resistant: s.flag(&b("fireResistant")),
        explosion_resistant: s.flag(&b("explosionResistant")),
        invulnerable_backpack: s.flag(&b("invulnerableBackpack")),
        tool_slots_accept_everything: s.flag(&b("toolSlotsAcceptEverything")),
        tool_slots_acceptable_items: s.list(&b("toolSlotsAcceptableItems")),
        blacklisted_items: s.list(&b("blacklistedItems")),
        allow_shulker_boxes: s.flag(&b("allowShulkerBoxes")),
        void_protection: s.flag(&b("voidProtection")),
        backpack_death_place: s.flag(&b("backpackDeathPlace")),
        backpack_force_death_place: s.flag(&b("backpackForceDeathPlace")),
        quick_sleeping_bag: s.flag(&b("quickSleepingBag")),
        enable_sleeping_bag_spawn_point: s.flag(&b("enableSleepingBagSpawnPoint")),
        back_slot_integration: s.flag(&b("backSlotIntegration")),
        enable_sleeping_bag: s.flag(&u("enableSleepingBag")),
        disabled_upgrades,
        feeding_tick_rate: s.int(&u("feedingUpgradeSettings.tickRate"), 1, 1000) as u64,
        magnet_pull_range: s.int(&u("magnetUpgradeSettings.pullRange"), 1, 20) as i32,
        magnet_tick_rate: s.int(&u("magnetUpgradeSettings.tickRate"), 1, 1000) as u64,
        refill_tick_rate: s.int(&u("refillUpgradeSettings.tickRate"), 1, 1000) as u64,
        spawn_entities_with_backpack: s.flag("server.world.spawnEntitiesWithBackpack"),
        spawn_chance: s.float("server.world.chance", 0.0, 1.0),
        overworld_entity_types: s.list("server.world.possibleOverworldEntityTypes"),
        nether_entity_types: s.list("server.world.possibleNetherEntityTypes"),
        overworld_backpacks: s.list("server.world.overworldBackpacks"),
        nether_backpacks: s.list("server.world.netherBackpacks"),
        enable_backpack_abilities: s.flag("server.backpackAbilities.enableBackpackAbilities"),
        force_ability_enabled: s.flag("server.backpackAbilities.forceAbilityEnabled"),
        allowed_abilities: s.list("server.backpackAbilities.allowedAbilities"),
        backpack_effects: s
            .list("server.backpackAbilities.backpackEffects")
            .iter()
            .filter_map(|e| {
                let p = numbers(e);
                Some((p.first()?.clone(), p.get(1)?.clone(), p.get(2)?.parse().ok()?, p.get(3)?.parse().ok()?, p.get(4)?.parse().ok()?))
            })
            .collect(),
        cooldowns: s
            .list("server.backpackAbilities.cooldowns")
            .iter()
            .filter_map(|e| {
                let p = numbers(e);
                Some((p.first()?.clone(), p.get(1)?.parse().ok()?, p.get(2)?.parse().ok()?))
            })
            .collect(),
        too_many_backpacks_slowness: s.flag("server.slownessDebuff.tooManyBackpacksSlowness"),
        max_number_of_backpacks: s.int("server.slownessDebuff.maxNumberOfBackpacks", 1, 37) as i32,
        slowness_per_excessed_backpack: s.float("server.slownessDebuff.slownessPerExcessedBackpack", 0.1, 5.0),
        show_backpack_icon_in_inventory: k.flag("client.showBackpackIconInInventory"),
        send_backpack_coordinates_message: k.flag("client.sendBackpackCoordinatesMessage"),
        obtain_tips: k.flag("client.obtainTips"),
        render_tools: k.flag("client.renderTools"),
        tools_swap_on_close: k.flag("client.toolsOverlay.swapOnClose"),
        tools_show_tooltip: k.flag("client.toolsOverlay.showTooltip"),
        tools_display_empty_slots: k.flag("client.toolsOverlay.displayEmptySlots"),
        tools_allow_hotbar_scrolling: k.flag("client.toolsOverlay.allowHotbarScrolling"),
        tools_render_backpack_icon_in_center: k.flag("client.toolsOverlay.renderBackpackIconInCenter"),
        tools_opacity: k.float("client.toolsOverlay.opacity", 0.0, 1.0) as f32,
        tools_offset: [
            k.int("client.toolsOverlay.offsetX", i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
            k.int("client.toolsOverlay.offsetY", i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
        ],
        overlay_enabled: k.flag("client.overlay.enableOverlay"),
        overlay_offset: [
            k.int("client.overlay.offsetX", i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
            k.int("client.overlay.offsetY", i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
        ],
    }
}

static CONFIG: OnceLock<Config> = OnceLock::new();

/// The mod's config, read from `dir` the first time.
pub(crate) fn config() -> &'static Config {
    CONFIG.get_or_init(|| {
        let dir = super::Game::native_config_dir().ok();
        load(dir.as_deref())
    })
}
