# Dynamic Lights for Horizon

An original native mod (`.hnmod`) for [Horizon](https://github.com/horizon-rust/horizon), the Rust rewrite of
Minecraft: Java Edition 26.3: torches, lanterns and other glowing things light up the world as they move.

## Features

- **Held items**: a glowing item in your main hand or off hand lights the blocks and mobs around you. So do
  glowing items held by other players, mobs (a zombie with a torch) and armor stands.
- **Dropped items**: a torch or a lantern lying on the ground keeps shining.
- **Burning entities**: players, mobs, items, arrows and tridents on fire shine as brightly as fire.
- **Glowing creatures**: blazes, magma cubes (brighter the bigger they are), glow squids (dark for a while after
  being hurt, as in vanilla), allays and vexes.
- **Projectiles**: fireballs, dragon fireballs, wither skulls, shulker bullets, spectral arrows and firework rockets.
- **TNT and minecarts**: primed TNT, a lit minecart with TNT and a running minecart with furnace.
- **Water puts out fire** (option): torches, campfires, lava buckets, fireballs and blazes give no light
  underwater; lanterns keep their flame behind glass.

An item glows as brightly as its block does when placed (torch 14, lantern and glowstone 15, soul torch 10…),
glow berries as cave vines with berries (14).

### Settings

Options > Mods > Dynamic Lights > **Configure...** (or Mod Menu's Configure button) opens the settings screen,
an options screen in the game's own look, in English or Russian with the game's language:

| Option | Default | |
|---|---|---|
| Dynamic Lights | ON | the main switch (also on the mod's page) |
| Your Hands, Other Players, Mobs, Dropped Items, Burning Entities, Glowing Creatures, Projectiles, TNT and Minecarts | ON | each kind of light source |
| Water Puts Out Fire | ON | |
| Brightness | 100% | 25–100% of the full light level |
| Max Light Sources | 16 | how many of the nearest sources shine at once; fewer is faster on a weak graphics card |
| Range | 64 blocks | 16–96 blocks from the camera |
| Toggle Dynamic Lights | not bound | a key that turns the lights on and off while playing |

Every change is kept at once (one file per setting in `horizon-mods/.horizon-mod-data/dynamiclights/`).

The lights are Horizon's own dynamic lights: the mod only chooses the sources, and Horizon's terrain and entity
shaders light the world with them (client side only; the light engine, chunks and saves never see them). While
the mod is on, it takes the place of Options > Horizon Extras > Dynamic lights; turned off in Options > Mods,
Horizon's extra works as before.

## Install

From the launcher: Mods, find **Dynamic Lights**, Install, then Native build > Build.

By hand: put `dynamiclights-<version>.hnmod` in `<game folder>/horizon-native-mods/`, add the line
`mod.dynamiclights:true` to `horizon-mods.txt`, then Native build > Build in the launcher.

Native mods are compiled into the game and are not sandboxed: build only code you trust.

## Source layout

```
horizon-native.json            bundle manifest (id, version, pinned Horizon and horizon-mixins revisions)
mixins/dynamiclights/
  horizon-mixin.toml           modules and injections
  runtime.rs                   settings, texts and light levels, no game types (unit tests)
  glue.rs                      the light sources from the game's entity lists; the mod's page and key
  screen.rs                    the settings screen; the icon and Mod Menu's metadata
  *.rs                         one small helper per injection
  data/icon.png                the icon (drawn by tools/icon.mjs)
  LICENSE
tools/                         check.sh (build into the pinned Horizon), package.mjs, sync.mjs, icon.mjs
Cargo.toml                     runs runtime.rs's tests
```

The `.hnmod` file is a zip of `horizon-native.json` and `mixins/`.

Tests: `cargo test` (Rust 1.99.1, edition 2024).

Built for Horizon `2ebf00a0281c592f79191e76fce0e7d522c6f425` (Nightly 2026-11-07) and horizon-mixins
`5a64af938ec86b00d53d16a408aabc0038fb3e57`.

## Licence

MIT License, Copyright (c) 2026 DeportedToHell ([LICENSE](LICENSE)). Written from scratch for Horizon. Not
affiliated with Mojang or Microsoft.

---

**По-русски.** Динамический свет для Horizon: факел, фонарь или другой светящийся предмет в руке освещает всё
вокруг. Светятся и предметы на земле, горящие мобы, всполохи, магмовые кубы, светящиеся спруты, файерболы,
активированный динамит и другое. Под водой огонь гаснет (настраивается). Настройки: Настройки > Моды >
Dynamic Lights > «Настройки...» (или кнопка «Настроить» в Mod Menu); там же клавиша для включения и выключения.
