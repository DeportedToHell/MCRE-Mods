<p align="center"><picture><source media="(max-width: 700px)" srcset="../.github/releases/emotecraft/hero-phone.svg"><img src="../.github/releases/emotecraft/hero.svg" alt="Emotecraft 1.0.1" width="640"></picture></p>

<p align="center"><a href="https://github.com/DeportedToHell/MCRE-Mods/releases/download/emotecraft-1.0.1/emotecraft-1.0.1.hnmod"><picture><source media="(max-width: 700px)" srcset="../.github/releases/emotecraft/download-phone.svg"><img src="../.github/releases/emotecraft/download.svg" alt="Download emotecraft-1.0.1.hnmod" width="500"></picture></a><br>
<a href="https://playhorizon.dev/mods"><picture><source media="(max-width: 700px)" srcset="../.github/releases/emotecraft/catalog-phone.svg"><img src="../.github/releases/emotecraft/catalog.svg" alt="Horizon catalog" width="500"></picture></a><br>
<a href="https://github.com/DeportedToHell/MCRE-Mods/releases/tag/emotecraft-1.0.1"><picture><source media="(max-width: 700px)" srcset="../.github/releases/emotecraft/github-phone.svg"><img src="../.github/releases/emotecraft/github.svg" alt="Add from GitHub" width="500"></picture></a></p>

<p align="center"><img src="../.github/releases/common/title-features.svg" alt="Features" width="640"></p>

<p align="center">
<img src="../.github/releases/emotecraft/feat1.svg" alt="Emote wheel on B" width="315">
<img src="../.github/releases/emotecraft/feat2.svg" alt="Dances, flips and more" width="315">
<img src="../.github/releases/emotecraft/feat3.svg" alt="Browse all emotes" width="315">
<img src="../.github/releases/emotecraft/feat4.svg" alt="Set up your wheel" width="315">
</p>

<p align="center"><img src="../.github/releases/common/title-install.svg" alt="How to install" width="640"></p>

<p align="center"><picture><source media="(max-width: 700px)" srcset="../.github/releases/common/steps-phone.svg"><img src="../.github/releases/common/steps.svg" alt="Install from the file: download, put it in horizon-native-mods, Native build > Build" width="640"></picture></p>

<details>
<summary><b>All features in detail · Все возможности подробно</b></summary>

A port of [Emotecraft](https://github.com/KosmX/emotes) 3.5.0 by KosmX and dima_dencep
to Horizon (Minecraft Java Edition 26.3 in Rust), as the native mod `emotecraft`.

- **Client**: the emote wheel (B), All Emotes, Emotes config (wheel pages, key binds),
  Key Binds, Mod Options, Export, toasts, the emote previews, songs, first person,
  English and Russian.
- **Server**: the same mod is Emotecraft's server on a dedicated Horizon server, in
  LAN and in single player (`/emotes play|stop|reload`, the server's `emotes` folder).
- **Java servers**: talks to Java servers running Emotecraft (Fabric/NeoForge mod or
  Paper plugin), over the play-phase handshake.

Not ported: Online Emotes and the cloud library (their buttons stay as Emotecraft shows
them when the service is unavailable), Geyser.

Custom emotes go in `emotes/` in the game folder (Emotecraft `.json`, Blockbench /
Bedrock `.json`, `.emotecraft` binary, with `.png` icons and `.ogg`/`.opus` songs).

</details>

<details>
<summary><b>Install by hand and source code · Установка вручную и исходный код</b></summary>

### Layout

- `horizon-native.json` — the catalog manifest.
- `mixins/emotecraft/` — the bundle: `horizon-mixin.toml`, the game-side modules
  (`glue.rs`, `server.rs`, `ui.rs`, `lang.rs`, `icons.rs`, `preview.rs`, `songs.rs`,
  `pose.rs`, `atlas.rs`), one-line hook helpers `h_*.rs`, `data/` and the `engine/`
  crate (animation player, formats, protocol, server logic, Opus).
- `tools/lang.mjs` regenerates `engine/src/lang.rs` from Emotecraft's lang files;
  `tools/package.mjs` builds `mods/emotecraft-<version>.hnmod`.

</details>

<p align="center"><a href="https://github.com/DeportedToHell/MCRE-Mods/tree/main/emotecraft/mixins/emotecraft"><picture><source media="(max-width: 700px)" srcset="../.github/releases/common/code-phone.svg"><img src="../.github/releases/common/code.svg" alt="Source code" width="245"></picture></a> <a href="https://github.com/DeportedToHell/MCRE-Mods/issues/new?template=bug-emotecraft.yml"><picture><source media="(max-width: 700px)" srcset="../.github/releases/common/bug-phone.svg"><img src="../.github/releases/common/bug.svg" alt="Report a bug" width="245"></picture></a></p>

## Licence

GPL-3.0-only (`LICENSE`). Emotecraft © KosmX, dima_dencep and contributors; Horizon
port © 2026 DeportedToHell. Third-party notices and the statement of changes: `NOTICE`.
Issues: https://github.com/DeportedToHell/MCRE-Mods/issues
