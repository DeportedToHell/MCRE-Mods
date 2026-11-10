# Clumps for Horizon

A port of [Clumps](https://github.com/jaredlll08/Clumps) by Jaredlll08 to [Horizon](https://github.com/horizon-rust/horizon),
the Rust rewrite of Minecraft: Java Edition 26.3, as a native mod (`.hnmod`). It behaves like Clumps 26.3.

Ported by DispatchedToHeaven.

## Features

- **Experience orbs of any value clump together** into one orb worth all of them: when orbs come near each
  other, and when new experience drops by an orb that is already there. Less lag with many orbs about.
- **A player collects an orb whole**, all its points at once: no stream of orbs in your face. Mending repairs
  with it first, the rest goes to your experience bar, as with any orb.
- No settings (Clumps has none). Mod Menu shows its icon, credits and links. English and Russian.

Server side: it acts where your game runs the world (single player, a LAN world you host, a Horizon server
with the mod); joining someone else's server, their server decides. Turned on or off in Options > Mods, it
changes from the next time the world is loaded (the world's server reads it then).

## Install

From the launcher: Mods, find **Clumps**, Install, then Native build > Build.

By hand: put `clumps-<version>.hnmod` in `<game folder>/horizon-native-mods/`, add the line `mod.clumps:true` to
`horizon-mods.txt`, then Native build > Build in the launcher.

Native mods are compiled into the game and are not sandboxed: build only code you trust.

## Differences from the Java mod

- Clumps keeps how many orbs of each value an orb took in; the port keeps the orb's total (Horizon's orbs have
  one value and a count). The experience is the same; Mending repairs with the whole orb at once rather than
  value by value, which can pick the damaged item to repair a little differently.
- Clumps also lifts the player's pickup delay, so the player collects an orb every tick (the game touches one
  orb a tick). Horizon's server keeps vanilla's two ticks between two orbs and offers a mod no way to change it yet; orbs that reach
  the player mostly clump into one first anyway.
- Clumps' API events for other Java mods (`ValueEvent`, `RepairEvent`) have no Horizon counterpart.

## Source layout

```
horizon-native.json            bundle manifest (id, version, pinned Horizon and horizon-mixins revisions)
mixins/clumps/
  horizon-mixin.toml           modules and injections
  runtime.rs                   names and the clump's arithmetic: no game types (unit tests)
  glue.rs                      merging, the pickup, the mod's row and Mod Menu's files
  h_*.rs                       one small helper per injection
  data/icon.png                Clumps' icon
  LICENSE, NOTICE
tools/check.sh                 stages the mod on the pinned Horizon source and cargo-checks the game
tools/package.mjs              makes the .hnmod
Cargo.toml                     runs runtime.rs's tests
```

Tests: `cargo test` (Rust 1.99.1, edition 2024).

## Licence

MIT, as Clumps (Jaredlll08; the port: DispatchedToHeaven): see `LICENSE` and `NOTICE`. Not affiliated with Jaredlll08, Mojang or Microsoft.