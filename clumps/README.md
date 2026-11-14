<h1 align="left">
  <img src="../.github/assets/stickers/clumps.gif" height="64" alt="">
  <img src="../.github/assets/stickers/clumps.png" height="64" alt="">
</h1>


## <img src="../.github/assets/stickers/info.png" height="20" alt=""> This is an independent port of the original project.

> It is not affiliated with, endorsed by, or officially associated with the original author, Jaredlll08.
>
> Original mod: https://github.com/jaredlll08/Clumps
>
> Special thanks to [Jaredlll08](https://github.com/jaredlll08) for creating the original mod.


## ✨Features

**Experience orbs.** Nearby orbs merge into a single orb containing their combined XP. New XP drops also merge into existing nearby orbs, reducing entity count and lag.

**Pickup and Mending.** Players collect the entire orb at once instead of picking up a stream of smaller orbs. Mending still takes priority, with remaining XP added to the player's experience bar.

**No configuration needed.** Clumps has no settings. Mod Menu integration includes the icon, credits and links. English and Russian translations.

**Server-side.** Works wherever the world runs: singleplayer, hosted LAN worlds and servers with the mod installed. On other servers, the server determines the behaviour. Changes made in Options → Mods take effect when the world is loaded again.

## 📦Installation

Find **Clumps** in the launcher's Mods section, select Install, then run **Native build → Build**.

For manual installation, place `clumps-<version>.hnmod` in `<game folder>/horizon-native-mods/`, add `mod.clumps:true` to `horizon-mods.txt`, then run **Native build → Build** in the launcher.

> Native mods are compiled into the game and are not sandboxed. Only build code you trust.

## 🔧Differences from the Java mod

Horizon represents experience orbs using a value and a count, so the port tracks their combined XP rather than preserving the individual values of merged orbs. The result is effectively the same, although Mending may select a slightly different item to repair.

The original Clumps removes the player's pickup delay, allowing an orb to be collected every tick. Horizon currently retains the vanilla two-tick delay and provides no way for mods to change it. In practice, nearby orbs usually merge before reaching the player.

Clumps' API events (`ValueEvent`, `RepairEvent`) have no equivalent in Horizon yet.

## 🗂️Source layout

```text id="clumps"
horizon-native.json              Native bundle manifest
mixins/clumps/
  horizon-mixin.toml             Modules and injections
  runtime.rs                     Names and XP calculations
  glue.rs                        Merging, pickup and Mod Menu integration
  h_*.rs                          Injection helpers
  data/icon.png                  Original Clumps icon
tools/
  check.sh                       Checks and builds against pinned Horizon source
  package.mjs                    Packages the .hnmod
Cargo.toml                       Rust project and tests
```

Tests: `cargo test` (Rust 1.99.1, edition 2024).

## ⚖️License

Licensed under **MIT**, same as the original project.

**Original author:** [Jaredlll08](https://github.com/jaredlll08)  
**Port author:** [DispatchedToHeaven](https://github.com/DispatchedToHeaven)

See `LICENSE` and `NOTICE` for details.

This project is an independent port and is not affiliated with or endorsed by Jaredlll08, Mojang or Microsoft.
