<h1 align="left">
  <img src="../.github/assets/stickers/bwncr.png" height="72" alt="">
  <img src="../.github/assets/stickers/bwncrn.png" height="62" alt="">
</h1>

An independent port of [Bad Wither No Cookie - Reloaded](https://gitlab.com/kreezxil/BadWitherNoCookie) by Kreezxil and contributors to [Horizon](https://github.com/horizon-rust/horizon), the Rust rewrite of Minecraft: Java Edition 26.3.

Based on Fabric version 3.20.4, branch 1.21.1, including its configuration screen.

Ported by [DispatchedToHeaven](https://github.com/DispatchedToHeaven).

> This is an independent port of the original project.
> It is not affiliated with, endorsed by, or officially associated with the original authors.
>
> Original mod: https://gitlab.com/kreezxil/BadWitherNoCookie

Special thanks to [Kreezxil](https://gitlab.com/kreezxil) and all contributors for the original mod.

## ✨Features

**Sound control.** Silences Wither spawn and death, wandering trader and llama ambient sounds, Ender Dragon death and lightning thunder. Each category has its own toggle. Custom sound events can also be muted individually through the config.

**Sound identification.** Use `/bwncr listen` to display sound event IDs in chat, making it easy to find sounds to mute. Debug Mode logs sound events and the mod's handling of them.

**Config screen.** Full configuration UI with toggles, editable sound lists, search, reset and save options. Accessible through Options → Mods or Mod Menu. Settings are stored in `config/bwncr.toml`, using the original Fabric format.

**Client-side only.** Changes what you hear, regardless of the server you're playing on.

## 📦Installation

Find **Bad Wither No Cookie Reloaded** in the launcher's Mods section, select Install, then run **Native build → Build**.

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
  h_*.rs                          Injection helpers
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
