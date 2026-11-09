# Plasmo Voice for Horizon

A port of [Plasmo Voice](https://github.com/plasmoapp/plasmo-voice) 2.2.0 by Plasmo to
[Horizon](https://github.com/horizon-rust/horizon), the Rust rewrite of Minecraft: Java Edition 26.3, as a native mod
(`.hnmod`). Proximity voice chat: you hear the players near you, from where they stand, and they hear you.

## Features

- **Plasmo Voice's protocol** (`plasmo:voice/v2`, UDP, Opus, AES): a Horizon player talks with Java players on any
  server running Plasmo Voice (Paper, Fabric, NeoForge…), and on Horizon worlds.
- **Voice server built in** for a world you host: single player opened to LAN, so friends with the mod talk with you
  without installing anything else.
- **Push to talk** (Left Alt) or **voice activation** with its threshold, per activation, as in the original; **Mute
  microphone** (M), **Disable voice chat**, the activation and distance keys, the **Action** button + wheel for a
  player's volume.
- **Plasmo Voice's settings screen** on **V**: the Devices, Volume, Activation, Overlay, Advanced and Hotkeys tabs with
  every row of the original — microphone and speakers, the level meter with the threshold marker and **Test
  microphone**, volumes up to 200%, per-player volume and mute, overlay styles and positions, key bindings.
- **On screen as in Plasmo Voice:** the microphone icon while you talk (and its muted / disabled variants), the list
  of who is talking, and the icons above players' heads.
- Settings kept where Plasmo Voice keeps them: `config/plasmovoice/client.toml`.
- Mod Menu's Configure and Options > Mods > Plasmo Voice > Configure... open Plasmo Voice's Mod Menu screen (Cloth
  Config, as in Java). English and Russian (Plasmo Voice's own translations).

Additions of this port: a microphone before the name of players with voice chat in the player list (Tab) — white
while they talk, crossed out in red otherwise — with its option in the Overlay tab; and the list of who is talking
is on by default (Plasmo Voice: off for proximity).

Differences from the original, where Horizon has no counterpart yet: noise suppression, stereo capture, sound
occlusion, directional sources, HRTF and the Javax input device are in the settings but change nothing; the About
tab and addons are not ported; head icons are drawn over the world, not lit or cut by blocks; single player isn't
paused while the settings are open.

## Install

From the launcher: Mods, find **Plasmo Voice**, Install, then Native build > Build.

By hand: put `plasmovoice-<version>.hnmod` in `<game folder>/horizon-native-mods/`, add the line
`mod.plasmovoice:true` to `horizon-mods.txt`, then Native build > Build in the launcher.

Native mods are compiled into the game and are not sandboxed: build only code you trust.

## Source layout

- `mixins/plasmovoice/engine/` — the voice chat itself, a crate of its own: the protocol (`tcp.rs`, `udp.rs`, `wire.rs`),
  encryption and ids (`crypto.rs`, `md5.rs`), the microphone filters (`dsp.rs`), Opus and the audio devices
  (`audio.rs`), the client (`client.rs`) and the server (`server.rs`). `cargo test` runs its tests.
- `mixins/plasmovoice/glue.rs` — what joins it to the game: packets, keys, positions, the built-in server.
- `gui.rs`, `hud.rs`, `tab.rs`, `menu.rs`, `cloth.rs`, `config.rs`, `lang.rs`, `icons.rs` — the screens, the HUD, the
  player list, the settings file, the texts and the icons. `tools/lang.mjs` regenerates `lang.rs`.

## Licence

LGPL-3.0, as Plasmo Voice: see `LICENSE` and `NOTICE`. Not affiliated with Plasmo, Mojang or Microsoft.
