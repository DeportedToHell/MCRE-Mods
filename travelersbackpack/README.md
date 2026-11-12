# Traveler's Backpack for Horizon

An independent port of [Traveler's Backpack](https://github.com/Tiviacz1337/Travelers-Backpack) by Tiviacz1337 to [Horizon](https://github.com/horizon-rust/horizon), the Rust rewrite of Minecraft: Java Edition 26.3.

**Version 0.9.5** — almost all original features implemented. Remaining work is listed below.

Ported by **DispatchedToHeaven**.

> This is an independent port of the original project. It is not affiliated with, endorsed by, or officially associated with the original author, Tiviacz1337.
>
> All original credits and attributions belong to their respective authors. This port is developed independently.
>
> Original mod: https://github.com/Tiviacz1337/Travelers-Backpack — licence: GNU LGPL-3.0.

The port consists of two required parts: `content/` — a content mod (`.hmod`) containing blocks, items, models, textures, recipes, loot, advancements and translations; and `mixins/travelersbackpack_native/` — a native mod (`.hnmod`) implementing backpack functionality and mechanics.

## Features

**Backpacks and inventory.** The port includes all 44 original designs, covering standard backpacks, ores, mobs, plants and more, with five tiers ranging from leather to netherite. Backpacks can be opened from the hand, placed as blocks (Shift + right-click), or worn on the player's back using the menu button, Shift + right-click on a placed backpack, or the B key. Other players can see worn backpacks, and dying while wearing one places it as a block. The inventory menu retains the original functionality, including sorting, quick stacking, item transfers, tool slots, configurable slot behaviour, scrolling and tooltips.

**Upgrades and fluids.** Available upgrades include tanks, crafting, furnace, smoker, blast furnace, pickup, magnet, void (with a trash bin), feeding, refill, jukebox and lantern. Filters support item, mod and tag modes, with a dedicated tag selector. Every upgrade can be toggled independently. Tanks store water, lava, milk and potions, while the hose can suck, spill and drink fluids. Tank contents are displayed on held, worn and placed backpacks, as well as in an on-screen bar.

**Abilities and customization.** All original design-specific abilities are implemented, including armour, effects, creeper death protection, spider wall climbing and more. Abilities have individual toggles and cooldowns. Standard backpacks can be dyed and washed in cauldrons, while shears can restore placed backpacks. A sleeping bag is available in every colour and can be deployed from a placed backpack.

**Lighting and tools.** The lantern provides dynamic lighting while worn, using either Horizon's Dynamic Lights or Better Dynamic Lights. The tool radial menu (hold Z) allows swapping held tools with those stored in backpack tool slots. When holding the hose, the same menu provides access to its modes and tanks.

**Other features.** The port also includes advancements, the wandering trader's trade, a creative tab and keybinds in Options → Mods. All original keybinds are present, including B, Z and six additional keys that are unbound by default. English and Russian translations are included.

## Remaining work

The original config file settings are not yet exposed, so the port currently uses their default values. Mod Menu metadata is also missing.

Compatibility with other Java mods supported by the original has not been implemented yet. This includes Accessories / Trinkets, JEI / EMI / REI, Comforts, grave mods and other integrations.

There are also a few minor differences: fluid levels in item tanks are displayed in 16 steps, the tool radial menu does not reproduce the original icon growth animation, and tool swap sounds are only audible to the player.

Some features still need in-game verification. These include the cake, chicken, bee, warden, lapis, pumpkin, ghast, hay, sponge, cactus, melon and bookshelf abilities; upgrade toggle keybinds; the hose section of the radial menu; jukebox audio; and death and quick sleeping while wearing a backpack.

## Installation

Build the content mod with `cargo horizon-mod build` from `content/` and place the resulting `.hmod` in the game's mods folder.

Package the native mod using `tools/package.mjs` and place the resulting `.hnmod` in `<game folder>/horizon-native-mods/`. Add `mod.travelersbackpack_native:true` to `horizon-mods.txt`, then select **Native build → Build** in the launcher.

> Native mods are compiled into the game and are not sandboxed. Only build code you trust.

## Source layout

```text
content/                         Content mod, generated from the original checkout
horizon-native.json              Native bundle manifest
mixins/travelersbackpack_native/
  horizon-mixin.toml             Modules and injections
  runtime.rs                     Tiers, designs and names (no game types)
  glue.rs                        Main mod implementation
  menu.rs, spider.rs, screen_relayout.rs
                                  Additional modules
  h_*.rs                         Small injection helpers
tools/
  content.mjs                    Generates content/ from the original checkout
  check.sh                       Stages the mod on pinned Horizon source and checks/builds the game
  package.mjs                    Packages the .hnmod
```

## Licence

GNU LGPL-3.0, same as the original (Tiviacz1337; port: DispatchedToHeaven). See `LICENSE`, `LICENSE.LGPL-3.0`, `LICENSE.GPL-3.0` and `NOTICE`.

Not affiliated with Tiviacz1337, Mojang or Microsoft.
