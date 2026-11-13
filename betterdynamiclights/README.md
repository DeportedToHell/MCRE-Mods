<p align="center"><picture><source media="(max-width: 700px)" srcset="../.github/releases/dynamiclights/hero-phone.svg"><img src="../.github/releases/dynamiclights/hero.svg" alt="Better Dynamic Lights 1.0.3" width="640"></picture></p>

<p align="center"><a href="https://github.com/DeportedToHell/MCRE-Mods/releases/download/dynamiclights-1.0.3/dynamiclights-1.0.3.hnmod"><picture><source media="(max-width: 700px)" srcset="../.github/releases/dynamiclights/download-phone.svg"><img src="../.github/releases/dynamiclights/download.svg" alt="Download dynamiclights-1.0.3.hnmod" width="500"></picture></a><br>
<a href="https://playhorizon.dev/mods/dynamiclights"><picture><source media="(max-width: 700px)" srcset="../.github/releases/dynamiclights/catalog-phone.svg"><img src="../.github/releases/dynamiclights/catalog.svg" alt="Horizon catalog" width="500"></picture></a><br>
<a href="https://github.com/DeportedToHell/MCRE-Mods/releases/tag/dynamiclights-1.0.3"><picture><source media="(max-width: 700px)" srcset="../.github/releases/dynamiclights/github-phone.svg"><img src="../.github/releases/dynamiclights/github.svg" alt="Add from GitHub" width="500"></picture></a></p>

<p align="center"><img src="../.github/releases/common/title-features.svg" alt="Features" width="640"></p>

<p align="center">
<img src="../.github/releases/dynamiclights/feat1.svg" alt="Torch in hand lights up" width="315">
<img src="../.github/releases/dynamiclights/feat2.svg" alt="Dropped lights keep shining" width="315">
<img src="../.github/releases/dynamiclights/feat3.svg" alt="Burning and glowing mobs" width="315">
<img src="../.github/releases/dynamiclights/feat4.svg" alt="Choose what glows" width="315">
</p>

<p align="center"><img src="../.github/releases/common/title-install.svg" alt="How to install" width="640"></p>

<p align="center"><picture><source media="(max-width: 700px)" srcset="../.github/releases/common/steps-phone.svg"><img src="../.github/releases/common/steps.svg" alt="Install from the file: download, put it in horizon-native-mods, Native build > Build" width="640"></picture></p>

<details>
<summary><b>All features in detail · Все возможности подробно</b></summary>

An original native mod (`.hnmod`) for [Horizon](https://github.com/horizon-rust/horizon), the Rust rewrite of
Minecraft: Java Edition 26.3: torches, lanterns and other glowing things light up the world as they move.

### Better than the built-in lights

Horizon already has dynamic lights of its own (Options > Horizon Extras > Dynamic lights, off by default): items in
your hands, dropped items and burning mobs light up, with one on/off switch. Better Dynamic Lights uses the same light
renderer and takes the place of that switch while it is on, with much more:

| | Horizon Extras | Better Dynamic Lights |
|---|---|---|
| Held items (yours, other players', mobs') and dropped items | yes | yes, also armor stands |
| Burning mobs and items | yes | yes, also burning arrows and tridents |
| Glowing creatures: blazes, magma cubes, glow squids, allays, vexes | — | yes |
| Projectiles: fireballs, wither skulls, shulker bullets, spectral arrows, firework rockets | — | yes |
| Primed TNT, TNT and furnace minecarts | — | yes |
| Water puts out fire (lanterns keep shining) | — | yes |
| A switch for every kind of light | — | yes |
| Brightness, range and number of lights | — | yes |
| A key to turn the lights on and off | — | yes |
| Its own settings screen, Mod Menu, English and Russian | — | yes |
| On by default | off | on |

### Features

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

#### Settings

Options > Mods > Better Dynamic Lights > **Configure...** (or Mod Menu's Configure button) opens the settings screen,
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

</details>

<details>
<summary><b>Install by hand and source code · Установка вручную и исходный код</b></summary>

### Install

From the launcher: Mods, find **Better Dynamic Lights**, Install, then Native build > Build.

By hand: put `dynamiclights-<version>.hnmod` in `<game folder>/horizon-native-mods/`, add the line
`mod.dynamiclights:true` to `horizon-mods.txt`, then Native build > Build in the launcher.

Native mods are compiled into the game and are not sandboxed: build only code you trust.

### Source layout

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

Built for Horizon `fdc87aecdbd116bf1e08adcd062fcbaea8c2e2ca` (Nightly 2026-11-08) and horizon-mixins
`5a64af938ec86b00d53d16a408aabc0038fb3e57`.

</details>

<p align="center"><a href="https://github.com/DeportedToHell/MCRE-Mods/tree/main/betterdynamiclights/mixins/dynamiclights"><picture><source media="(max-width: 700px)" srcset="../.github/releases/common/code-phone.svg"><img src="../.github/releases/common/code.svg" alt="Source code" width="245"></picture></a> <a href="https://github.com/DeportedToHell/MCRE-Mods/issues/new?template=bug-better-dynamic-lights.yml"><picture><source media="(max-width: 700px)" srcset="../.github/releases/common/bug-phone.svg"><img src="../.github/releases/common/bug.svg" alt="Report a bug" width="245"></picture></a></p>

## Licence

MIT License, Copyright (c) 2026 DeportedToHell ([LICENSE](LICENSE)). Written from scratch for Horizon. Not
affiliated with Mojang or Microsoft.

---

**По-русски.** Better Dynamic Lights — улучшенный динамический свет для Horizon. В самой игре уже есть встроенный
динамический свет (Настройки > Дополнения Horizon, по умолчанию выключен): светятся предметы в руке и на земле и горящие
мобы. Мод заменяет его, пока включён, и умеет больше: светятся всполохи, магмовые кубы, светящиеся спруты, тихони и
вредины, файерболы, черепа визера, светящиеся стрелы и фейерверки, подожжённый динамит и вагонетки с динамитом и печкой;
под водой огонь гаснет, а фонари продолжают светить; у каждого вида света свой переключатель, можно настроить яркость,
дальность и число источников, есть клавиша включения и свой экран настроек. Настройки: Настройки > Моды >
Better Dynamic Lights > «Настройки...» (или кнопка «Настроить» в Mod Menu).
