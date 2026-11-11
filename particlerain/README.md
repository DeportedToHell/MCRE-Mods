# Particle Rain for Horizon

A port of [Particle Rain](https://github.com/PigCart/particle-rain) by PigCart to
[Horizon](https://github.com/horizon-rust/horizon), the Rust rewrite of Minecraft: Java Edition 26.3, as a native
mod (`.hnmod`). The game's flat rain and snow curtains give way to particles: raindrops, snowflakes, desert dust,
fog and more, blown by the wind. Ported by DeportedToHell.

## Features

As Particle Rain 4.0.3 (branch `main`, a9f1de9) on Minecraft 26.3:

- **Rain** and, in thunderstorms, **heavy rain**: drops that fall at an angle with their speed and the wind, cut
  from the game's own rain texture (a resource pack's too), greyed and tinted with the biome's water colour mixed
  with the sky's. They end on what they hit.
- **Snow** and **heavy snow** in cold biomes, spinning flakes cut from the game's snow texture.
- **Rain ripples** on water, **rain splashes** (the game's splash particle, tinted), **rain streaks** running down
  stone and glass walls (a drip where the wall ends), **rain steam** over magma and netherrack.
- **Dust**, **skittering dust**, **dust haze** and rolling **shrubs** (dead bushes) in deserts and badlands while it
  rains elsewhere, with a **sandstorm sound**; a **snow sound** where it snows.
- **Fog** in wet biomes (swamps, jungles, dark forests) and **fog after rain** for a while after a rain stops,
  flat and upright banks that blend together.
- **Wind**: gusts that grow with height and drop to nothing underground, stronger in storms; the game's own
  particles Particle Rain spawns drift with it too.
- Water drips and splashes take the water's colour; the game's rain curtains are hidden (each can be turned back
  on), its own rain splashes too, and the rain sound plays at the mod's volume.
- Each built-in particle can be turned off or edited, and new ones added: when they spawn (weather, precipitation,
  biomes and blocks lists with tags), where (sky, world surface, block sides, top or bottom), their motion,
  sprites, size, fade, tint, render type and rotation. Weather passes through fences, signs and barriers.
- **Particle Rain's config screens**, as in Java: `/particlerain`, Configure... on Options > Mods > Particle Rain,
  or Configure in [Mod Menu](../modmenu). Performance, Sound, Wind and Compatibility options, each particle's
  ON/OFF and Edit, Add new, the particle editor with its lists, Reset and Done.
- `/particlerain debug` and `/particlerain biometags` print the spawner's state and the biome's tags in chat.
- Kept as Particle Rain keeps them: `config/particlerain/config.json` and `config/particlerain/particles.json`; a
  resource pack's `assets/particlerain/particles.json` changes the built-in particles.
- English and Russian (the original's translations; the lines its Russian file lacks are translated by the port).
  Mod Menu shows Particle Rain's icon and details.

### Port addition

- **Changing Wind Direction** (Wind Options, on by default): the original's wind always blows north-west and only
  its strength gusts; with this option the wind turns slowly with the game time (every direction over a few tens
  of minutes, the same for every player of a world). Strength, gusts, height and storms are the original's. Off
  is exactly the original. It is kept in `config.json` as `horizonChangingWindDirection`.

### Differences from the original

- Fabric's `c:` convention tags don't exist in Horizon: the port defines those the built-in particles name.
- "Sync registries" is kept and shown but changes nothing (Horizon has no registry sync).
- WindLink and Iris compatibility are not ported (neither exists for Horizon); "Don't tint water particles when
  using shaderpack" follows Horizon's own shader packs.
- The F3 entries `particlerain:debug` and `particlerain:biome_tags` (off by default in Java) are not added; the
  same lines come from `/particlerain debug` and `/particlerain biometags`.
- "Blended" particles draw on the game's translucent particle layer.
- A built-in particle's Blocks or Biomes list switched between whitelist and blacklist, its entries unchanged, is
  saved to `particles.json` (the whole list, as the original writes a changed one). The original compares only
  the entries and loses the switch on the next load.

## Install

From the launcher: Mods, find **Particle Rain**, Install, then Native build > Build.

By hand: put `particlerain-<version>.hnmod` in `<game folder>/horizon-native-mods/`, add the line
`mod.particlerain:true` to `horizon-mods.txt`, then Native build > Build in the launcher.

Native mods are compiled into the game and are not sandboxed: build only code you trust.

## Source layout

```
horizon-native.json            bundle manifest (id, version, pinned Horizon and horizon-mixins revisions)
mixins/particlerain/
  horizon-mixin.toml           modules and injections
  atlas.rs                     the sprites made while the game's atlas is packed (rain, snow, ripples, splashes)
  textures.rs                  the original's particle textures (generated)
  particle.rs                  inside the particle engine: the frame's quads, wind and tint on the game's particles
  sound.rs                     the snow and sandstorm sounds, beside the sound cache
  core.rs                      the settings, the particles' data, their files, the tags
  defaults.rs                  the original's built-in particles (generated)
  weather.rs                   spawning, the particles' tick, the wind, tints, weather sounds, the frame's quads
  glue.rs                      files, the tick, /particlerain, Options > Mods, Mod Menu, the game's rain columns
  screen.rs                    the config screens
  lang.rs                      English and Russian (generated)
  h_*.rs                       one small helper per injection
  data/                        the icon and the weather sounds (the original's)
  LICENSE, NOTICE
tools/                         generate.mjs (lang.rs, defaults.rs, textures.rs from tools/assets/), package.mjs
```

The `.hnmod` file is a zip of `horizon-native.json` and `mixins/`.

Built for Horizon `fdc87aecdbd116bf1e08adcd062fcbaea8c2e2ca` (Nightly 2026-11-08) and horizon-mixins
`5a64af938ec86b00d53d16a408aabc0038fb3e57`.

## Licence and credits

MIT, as the original ([LICENSE](LICENSE), [NOTICE](NOTICE)).

- Particle Rain: Copyright (c) 2025 PigCart, https://github.com/PigCart/particle-rain. Contributors: plunderpixels,
  HigorFr, LilleboHaugen, CH4ACKO3; translations: megatocha, Sasaki-Akari, good7777865.
- The textures, sounds, icon and texts are Particle Rain's own.

Not affiliated with PigCart, Mojang or Microsoft.

---

**По-русски.** Порт мода Particle Rain для Horizon: вместо плоских «занавесок» дождя и снега — частицы: капли,
снежинки, пыль и перекати-поле в пустынях, туман, рябь на воде, струйки по стенам, всё сносит ветер. Экран настроек
как в Java: команда `/particlerain`, кнопка «Настройки...» в Mod Menu или Настройки > Моды > Particle Rain.
Добавлено в порте: «Меняющееся направление ветра» (включено; выключите — будет ровно как в оригинале).
