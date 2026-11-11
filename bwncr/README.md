# Bad Wither No Cookie - Reloaded for Horizon

A port of [Bad Wither No Cookie - Reloaded](https://gitlab.com/kreezxil/BadWitherNoCookie) by Kreezxil and its
contributors to [Horizon](https://github.com/horizon-rust/horizon), the Rust rewrite of Minecraft: Java Edition 26.3,
as a native mod (`.hnmod`). It behaves like the Fabric mod (3.20.4, branch 1.21.1) and has its config screen.

Ported by DispatchedToHeaven.

## Features

- **Silences the world-wide sounds** before they start, each with its option (all on by default):
  - **Silence Wither**: the wither's spawn and death (`entity.wither.spawn`, `entity.wither.death`);
  - **Silence Trader**: the wandering trader's and the llamas' ambient sounds;
  - **Silence Dragon**: the ender dragon's death (`entity.ender_dragon.death`);
  - **Silence Lightning**: the thunder of lightning (`entity.lightning_bolt.thunder`).
- **Silence Us**: any other sound events, one per line (`entity.sheep.step`, `entity.cow.ambient`, a mod's own).
- **`/bwncr listen`** turns on (and off) the name of every sound in chat ("Sound is entity.cow.ambient"), to find
  the ones to silence.
- **Debug Mode** writes each sound and what the mod did with it to the game's log.
- **The config screen** AutoConfig builds with Cloth Config: the collapsible General group, Yes/No buttons,
  the list with Insert New and Delete Selected, Reset, search, Save & Quit and "Changes Not Saved". It opens from
  Options > Mods > Bad Wither No Cookie Reloaded (Configure...) and from Mod Menu's Configure; the options are kept
  in `config/bwncr.toml`, in the Fabric mod's format.
- Mod Menu shows the mod's icon, credits and links. English and Russian.

Client only: it changes what you hear, on any server.

## Install

From the launcher: Mods, find **Bad Wither No Cookie Reloaded**, Install, then Native build > Build.

By hand: put `bwncr-<version>.hnmod` in `<game folder>/horizon-native-mods/`, add the line `mod.bwncr:true` to
`horizon-mods.txt`, then Native build > Build in the launcher.

Native mods are compiled into the game and are not sandboxed: build only code you trust.

## Source layout

```
horizon-native.json            bundle manifest (id, version, pinned Horizon and horizon-mixins revisions)
mixins/bwncr/
  horizon-mixin.toml           modules and injections
  runtime.rs                   the config and its file, the texts: no game types (unit tests)
  sounds.rs                    the check of each sound (SoundHandler), a module beside the game's sound hook
                               in the client library (unit tests)
  glue.rs                      the mod's row and page, /bwncr listen, each frame's work
  config.rs                    the config screen: opening, saving, Mod Menu's icon and metadata
  cloth.rs                     Cloth Config's screen, with its sub-category, boolean and string list entries
  cloth_sprites.rs             Cloth Config's texture as pixel runs (made by tools/sprites.mjs)
  h_*.rs                       one small helper per injection
  data/icon.png                the mod's logo
  LICENSE, NOTICE
tools/check.sh                 stages the mod on the pinned Horizon source and cargo-checks the game
tools/package.mjs              makes the .hnmod
tools/sprites.mjs              makes cloth_sprites.rs from Cloth Config's resources
Cargo.toml                     runs runtime.rs's and sounds.rs's tests
```

Tests: `cargo test` (Rust 1.99.1, edition 2024).

## Licence

MIT, as the original (droidicus, Kreezxil, Eleksploded; the port: DispatchedToHeaven): see
`LICENSE` and `NOTICE`. The config screen, `mixins/bwncr/cloth.rs` and `cloth_sprites.rs`, follows Cloth Config and is
under the GNU LGPL-3.0 (`LICENSE.LGPL-3.0`, with the GPL-3.0 it builds on in `LICENSE.GPL-3.0`). Not affiliated with Kreezxil, Mojang or Microsoft.
