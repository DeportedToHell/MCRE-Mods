
  <img src="../.github/assets/stickers/bwncr.png" height="72" alt=""> <img src="../.github/assets/stickers/bwncrn.png" height="50" alt="">


## <img src="../.github/assets/stickers/info.png" height="20" alt=""> This is an independent port of the original project.

> It is not affiliated with, endorsed by, or officially associated with the original authors.
>
> Original mod: https://gitlab.com/kreezxil/BadWitherNoCookie
>
> Special thanks to [Kreezxil](https://gitlab.com/kreezxil) and all contributors for the original mod.


## Current State

1.0.3. Fully playable.


## ✨Features

**Quieter world.** Turns off the loud sounds: the Wither spawning and dying, the Ender Dragon dying, thunder, wandering traders and llamas. Each one has its own toggle, and you can mute any other sound by its ID.

**Find a sound's ID.** `/bwncr listen` shows the IDs of sounds in chat as they play.

**Config screen.** In Options → Mods or Mod Menu. Settings are saved to `config/bwncr.toml`, same format as the original.

**Client-side.** Only changes what you hear, on any server.

## Changelog:

### 1.0.1-1.0.3

- Nothing special

### 1.0.0

- First release: muted sounds, custom sound list, `/bwncr listen`, debug mode, config screen.


## 📦Installation

Find **Bad Wither No Cookie - Reloaded** in the launcher's Mods section, select Install, then run **Native build → Build**.

For manual installation, place `bwncr-<version>.hnmod` in `<game folder>/horizon-native-mods/`, add `mod.bwncr:true` to `horizon-mods.txt`, then run **Native build → Build** in the launcher.

> Native mods are compiled into the game and are not sandboxed. Only build code you trust.


## 🗂️Source layout

```text
horizon-native.json              Native bundle manifest
mixins/bwncr/
  horizon-mixin.toml             Modules and injections
  runtime.rs                     Config and sound handling
  sounds.rs                      Sound event processing
  glue.rs                        Main implementation
  config.rs, cloth.rs            Configuration screen
  cloth_sprites.rs               Cloth Config textures
  h_*.rs                         Injection helpers
  data/icon.png                  Mod icon
tools/
  check.sh                       Checks and builds against pinned Horizon source
  package.mjs                    Packages the .hnmod
  sprites.mjs                    Generates Cloth Config textures
Cargo.toml                       Rust project and tests
```

Tests: `cargo test` (Rust 1.99.1, edition 2024).


## ⚖️License

The original project is licensed under **MIT**.

**Original authors:** droidicus, [Kreezxil](https://gitlab.com/kreezxil), Eleksploded  
**Port author:** [DispatchedToHeaven](https://github.com/DispatchedToHeaven)

The configuration screen and Cloth Config-derived components are licensed under **GNU LGPL-3.0**. See `LICENSE`, `NOTICE`, `LICENSE.LGPL-3.0` and `LICENSE.GPL-3.0` for details.

This project is an independent port and is not affiliated with or endorsed by Kreezxil, Mojang or Microsoft.
