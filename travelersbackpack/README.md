# 🎒Traveler's Backpack for Horizon

An independent port of [Traveler's Backpack](https://github.com/Tiviacz1337/Travelers-Backpack) by Tiviacz1337 to [Horizon](https://github.com/horizon-rust/horizon), the Rust rewrite of Minecraft: Java Edition 26.3.

**Version 0.9.5** — nearly all original features implemented. Remaining work is listed below.

Ported by [DispatchedToHeaven](https://github.com/DispatchedToHeaven).

> This is an independent port of the original project.
> It is not affiliated with, endorsed by, or officially associated with the original author, Tiviacz1337.
>
> Original mod: https://github.com/Tiviacz1337/Travelers-Backpack
> License: GNU LGPL-3.0.


Special thanks to [Tiviacz1337](https://github.com/Tiviacz1337) for creating the original mod.

## ✨Features

**Backpacks.** All 44 original designs and five tiers, from leather to netherite. Backpacks can be opened, placed as blocks or worn on the player's back. Other players can see them, and dying while wearing one places it as a block.

**Inventory and upgrades.** The original backpack menu, sorting, quick stacking, tool slots and settings. Upgrades include crafting, furnaces, pickup, magnet, void, feeding, refill, tanks, jukebox and lantern, with configurable filters and toggles.

**Fluids and abilities.** Tanks support water, lava, milk and potions. The hose can suck, spill and drink fluids. Original design-specific abilities are implemented, alongside the sleeping bag, dyeing, cauldron washing and backpack restoration.

**Other features.** Dynamic lighting, tool radial menu, advancements, wandering trader trade, creative tab and keybinds. English and Russian translations.

## 🚧Remaining work

Original config settings and Mod Menu metadata are not yet implemented. Compatibility with other Java mods supported by the original, including Accessories / Trinkets, JEI / EMI / REI, Comforts and grave mods, is also missing.

Minor differences include 16-step fluid levels in item tanks, no icon growth animation in the tool radial menu, and tool swap sounds audible only to the player.

Some abilities and interactions still need in-game verification, including several design-specific abilities, upgrade keybinds, the hose radial menu, jukebox audio and sleeping or dying while wearing a backpack.

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
