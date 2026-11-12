# Traveler's Backpack for Horizon

A port of [Traveler's Backpack](https://github.com/Tiviacz1337/Travelers-Backpack) by Tiviacz1337 to
[Horizon](https://github.com/horizon-rust/horizon), the Rust rewrite of Minecraft: Java Edition 26.3.
Version 0.9.5: almost everything of the original works; what is left is listed below.

Ported by DispatchedToHeaven.

> This is an independent port of the original project. It is not affiliated with, endorsed by, or officially
> associated with the original author, Tiviacz1337.
>
> All original credits and attributions belong to their respective authors. This port is developed independently.
>
> Original mod: https://github.com/Tiviacz1337/Travelers-Backpack — licence: GNU LGPL-3.0.

The port has two parts, both needed:
- `content/` — a content mod (`.hmod`): blocks, items, models, textures, recipes, loot, advancements, languages;
- `mixins/travelersbackpack_native/` — a native mod (`.hnmod`): everything the backpacks do.

## What it does

- **Backpacks**: 44 designs (standard, ores, mobs, plants and more), five tiers from leather to netherite.
  Open from the hand, place as a block (Shift + right click), wear on the back (button in the menu, Shift + right
  click on a placed one, key B). Others see it on your back. Dying while wearing one places it as a block.
- **Menu** as in the original: sorting, quick stacking, moving items in and out, tool slots, settings (sort
  order, locked slots, reserved slots, hide from the back), scrolling on short windows, tooltips.
- **Upgrades**: tanks, crafting, furnace, smoker, blast furnace, pickup, magnet, void (with a trash bin),
  feeding, refill, jukebox, lantern; filters with item / mod / tag modes and a tag selector; each can be
  switched on and off.
- **Tanks and hose**: water, lava, milk and potions go into the tanks; the hose sucks, spills and drinks.
  Fluids are shown in the tanks of placed, worn and held backpacks, and in a bar on the screen.
- **Sleeping bag**: in any colour, deploys from a placed backpack.
- **Abilities** of every design, as in the original (armour, effects, creeper saving you from death, spider
  climbing walls, and so on), with a switch and cooldowns.
- **Dyeing** the standard backpack, washing it in a cauldron, shears to restore a placed one.
- **Lantern** gives dynamic light while worn: through Horizon's own Dynamic Lights or Better Dynamic Lights.
- **Tools radial menu** (hold Z): swap the tool in your hand with the backpack's tool slots; with the hose in
  hand, choose its mode and tank.
- Advancements, the wandering trader's trade, the creative tab, keys in Options → Mods (B, Z and six more,
  unbound at first, as in the original). English and Russian.

## What is left

- Settings of the original's config file (the port uses its default values) and Mod Menu metadata.
- Compatibility with other Java mods the original supports (Accessories / Trinkets, JEI / EMI / REI, Comforts,
  grave mods and others).
- Small differences: fluid height in an item's tanks goes in 16 steps; the tools menu opens without the
  original's growing animation of the icons; the tool swap sound is heard only by the player.
- Not checked in game yet: some abilities (cake, chicken, bee, warden, lapis, pumpkin, ghast, hay, sponge,
  cactus, melon, bookshelf), the upgrade toggle keys, the hose part of the radial menu, the jukebox sound,
  dying and quick sleeping while wearing a backpack.

## Install

Build the content mod with `cargo horizon-mod build` in `content/` and put the `.hmod` into the game's mods;
put the native mod (`tools/package.mjs` makes the `.hnmod`) into `<game folder>/horizon-native-mods/`, add
`mod.travelersbackpack_native:true` to `horizon-mods.txt`, then Native build > Build in the launcher.

Native mods are compiled into the game and are not sandboxed: build only code you trust.

## Source layout

```
content/                       the content mod (made by tools/content.mjs from the original's checkout)
horizon-native.json            native bundle manifest (id, version, pinned Horizon and horizon-mixins revisions)
mixins/travelersbackpack_native/
  horizon-mixin.toml           modules and injections
  runtime.rs                   tiers, designs, names (no game types)
  glue.rs                      the mod itself
  menu.rs, spider.rs, screen_relayout.rs   small modules
  h_*.rs                       one small helper per injection
tools/content.mjs              builds content/ from the original mod's checkout
tools/check.sh                 stages the mod on the pinned Horizon source and checks or builds the game
tools/package.mjs              makes the .hnmod
```

## Licence

GNU LGPL-3.0, as the original (Tiviacz1337; the port: DispatchedToHeaven): see `LICENSE`, `LICENSE.LGPL-3.0`,
`LICENSE.GPL-3.0` and `NOTICE`. Not affiliated with Tiviacz1337, Mojang or Microsoft.
