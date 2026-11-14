
  <img src="../.github/assets/stickers/travelersbackpack.gif" height="64" alt=""> <img src="../.github/assets/stickers/travelersbackpack.png" height="64" alt="">


## <img src="../.github/assets/stickers/info.png" height="20" alt=""> This is an independent port of the original project.

> It is not affiliated with, endorsed by, or officially associated with the original author, Tiviacz1337.
>
> Original mod: https://github.com/Tiviacz1337/Travelers-Backpack
>
> Special thanks to [Tiviacz1337](https://github.com/Tiviacz1337) for creating the original mod.


## Current State

1.0.0. Fully playable.

BOTH FILES ARE REQUIRED FOR THE MOD TO WORK

`travelersbackpack.hmod`

`travelersbackpack_native.hnmod`

I'll release an updated mod to the launcher as soon as Horizon rolls out an update that merges `.hnmod` into `.hmod`.


## ✨Features

**Backpacks.** All 44 designs and five tiers, from leather to netherite. Open one from your hand, place it as a block or wear it on your back. Other players see it, and if you die wearing one, it's placed where you died.

**Upgrades.** Crafting, furnaces, pickup, magnet, void, feeding, refill, tanks, jukebox and lantern, with filters and on/off switches. Sorting, quick stacking and tool slots.

**Tanks and abilities.** Tanks hold water, lava, milk and potions, and the hose sucks, spills and drinks them. Each design has its own ability. Sleeping bags, dyeing and cleaning in a cauldron.

**In the world.** Backpacks in chest loot, and mobs that sometimes spawn wearing one.

**Back slot.** With [Accessories](../accessories) installed, the backpack goes in its Back slot. Turn it off with `backSlotIntegration` in the config.

**Other features.** Dynamic lighting, a radial tool menu, advancements, a wandering trader trade, a creative tab and keybinds. The original's config files, `/tb` commands with backups made on death, Mod Menu info. English and Russian.


## 🚧Remaining work

1.1.0 - Universal Graves integration.

Not implemented compared to the original:

- **Other mods' integration.** Moving JEI / EMI / REI recipes into the crafting upgrade, Crafting Tweaks, Polymorph, Comforts, grave mods, Trash Slot, VinURL and Entity Model Features.
- **Supporter badge.** It needs the supporters' list from the internet, and the port makes no network requests.

Differences: a Horizon menu holds at most 256 slots, so with the most upgrades and tools the config allows (10 and 8), or filters of more than 10 slots, a tier's storage is cut to what still fits (138 instead of 154 with the defaults).


## Changelog:

### 1.0.0

- Back slot with [Accessories](../accessories).

### 0.9.9

- Filter sizes from the config.

### 0.9.8

- Tier sizes from the config.
- The jukebox song follows the player.
- Tool swaps, abilities and upgrades are heard by players nearby.
- JEI and REI show the smithing upgrades.

### 0.9.7

- The creeper design saves you only from a blow that would really kill you.
- A placed backpack's refill upgrade works for players within 3 blocks.
- Sleeping bags from a worn backpack are cleaned up after a restart.
- Hoppers can't put a backpack into a shulker box.

### 0.9.6

- Fixed item duplication with placed backpacks in multiplayer, and by dropping or swapping a backpack with its menu open.
- Abilities, the smithing table, bottles, feeding and furnaces work like the original.
- Added the config files, Mod Menu info, `/tb` commands, chest loot, mobs with backpacks, tools on the back and the worn backpack's icon in the inventory.


## 📦Installation

Build the content mod with `cargo horizon-mod build` from `content/` and place the resulting `.hmod` in the game's mods folder.

Package the native mod using `tools/package.mjs` and place the resulting `.hnmod` in `<game folder>/horizon-native-mods/`. Add `mod.travelersbackpack_native:true` to `horizon-mods.txt`, then select **Native build → Build** in the launcher.

> Native mods are compiled into the game and are not sandboxed. Only build code you trust.


## 🗂️Source layout

```text
content/                         Content mod, generated from the original checkout
horizon-native.json              Native bundle manifest
mixins/travelersbackpack_native/
  horizon-mixin.toml             Modules and injections
  runtime.rs                     Tiers, designs and names
  glue.rs                        Main mod implementation
  config.rs                      The original's config files
  menu.rs, spider.rs, screen_relayout.rs
                                 Additional modules
  h_*.rs                         Injection helpers
tools/
  content.mjs                    Generates content/ from the original checkout
  check.sh                       Stages the mod on pinned Horizon source and checks/builds the game
  package.mjs                    Packages the .hnmod
```


## ⚖️License

Licensed under **GNU LGPL-3.0**, same as the original project.

**Original author:** [Tiviacz1337](https://github.com/Tiviacz1337)  
**Port author:** [DispatchedToHeaven](https://github.com/DispatchedToHeaven)

See `LICENSE`, `LICENSE.LGPL-3.0`, `LICENSE.GPL-3.0`, and `NOTICE` for details.

This project is an independent port and is not affiliated with or endorsed by Tiviacz1337, Mojang, or Microsoft.
