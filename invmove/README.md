# InvMove for Horizon

A port of [InvMove](https://github.com/PieKing1215/InvMove) by PieKing1215 to
[Horizon](https://github.com/horizon-rust/horizon), the Rust rewrite of Minecraft: Java Edition 26.3, as a native
mod (`.hnmod`). Walk, jump, sprint and sneak while an inventory or another screen is open, with rules per screen,
and the screen's dark background tint hidden.

## Features

- **Move in inventories**: the movement keys (forward, back, left, right, jump, sneak, sprint) keep working while
  the inventory, a chest, a furnace, the creative inventory, a villager's trades or another container is open, as
  they are on the keyboard (the original's `tickKeybinds` on the player's input tick). Double-tap sprinting, toggled
  sneak and sprint, riding and creative flight behave as without a screen.
- **Per-screen rules**, as InvMove's vanilla module has them: movement allowed in the containers, the creative
  inventory, horse inventories, books and the advancements screen (each can be switched off), never in the chat,
  sign and book editors, command and structure block editors, the death screen, the credits, statistics and level
  loading, and always in the F3+F4 game mode switcher. A paused game never moves.
- **Text fields stop movement** ("Text Field Disables Movement"): typing in the chat, a sign, a book, an anvil's
  name, the creative search, the recipe book's search or another of the game's text boxes never moves the player,
  nor typing in another mod's text field (JEI's and REI's search, for example).
- **Recipe Book** option: whether the player can move while a recipe book is open.
- **Sneak modes**: Off, Maintain (keep sneaking if you were when the screen opened; the default), Pressed and
  MaintainWhilePressed, separately for screens that allow movement, screens that don't, and creative flight.
  **Allow Dismounting** lets the sneak key dismount in a screen.
- **Unrecognized screens** (the pause menu in multiplayer, the options, a mod's own screen or menu): each one is
  added to the "Unrecognized UI Types" lists the first time it is met, grouped by mod, with "Unrecognized Screens
  Default" as its starting value.
- **Hide Inventory Backgrounds**: the dark tint behind the inventory and the containers is not drawn, per screen,
  and for pause screens as "Pause Screens Background" says (Show, ShowSP, AllowHide).
- **Toggle UI Movement** key (unbound at first): set it in Options > Mods > InvMove; it switches "Move In
  Inventories" while playing, and in a screen only when that changes whether you can move there.
- The jump key no longer presses a screen's focused button where you can move (the original's fix for its issue
  #2), except the merchant's trade offers.
- **Allowed Keys**: which keys InvMove handles in screens, every key mapping listed, the movement keys allowed.
- **InvMove's config screen**, as in Java (Cloth Config): Configure... on Options > Mods > InvMove, or Configure in
  [Mod Menu](../modmenu). The General, UI Movement and UI Background tabs, Reset beside each option, tooltips,
  search, the Allowed Keys, Modules > Vanilla and the unrecognized screens as expandable sections, Discard Changes /
  Save & Quit and the "Changes Not Saved" question. Mod Menu shows InvMove's icon and details.
- Kept as InvMove keeps it, in the game's `config/` folder: `invmove.json`, `invmove/vanilla.json` and
  `invmove/unrecognized.json`, in the original's format (an old boolean sneak option and the old "Allow Jumping"
  are carried over as the original does).
- English (InvMove's own texts) and Russian (this port's translation), as the game's language.

## Differences from the original

Horizon has no Java classes, so the screens are named after the original's classes (`net.minecraft...PauseScreen`)
where InvMove would see one, Horizon's own as `horizon:<Name>`, and a mod's menu or screen by its id; the vanilla
module's rules are the same.

- A text field always stops movement while "Text Field Disables Movement" is on. The original only checks it while
  the Recipe Book option allows movement, so with that option off, typing in an anvil or the creative search moved
  the player.
- Another mod's text field is recognised by the typed characters it takes: the player stops from the first
  character typed into it until a click, Escape, Enter, Tab or another screen (the original sees only the text
  fields a screen holds directly, and needs InvMoveCompats for JEI and REI).
- Controls waiting for a key to bind counts as a focused text field: pressing the key doesn't move the player (in
  the original, on a server, it did).
- A server's dialog stops movement in single player, where the original's dialogs pause the game (Horizon keeps the
  world running under them).
- The game's own test block screens count as structure block screens (they open with a text field focused, so the
  player can't move there in the original either).
- Allowed Keys: in Horizon only the movement keys are read while a screen is open, so allowing another key has no
  effect (in Java it only matters for the few keys that are read while held).
- "Debug Display" stays off and can't be changed, as in the original's 26.x versions ("not supported on 26.x yet").
- The toggle key is a keyboard key, as the original reads it (`InputConstants.isKeyDown`).

## Install

From the launcher: Mods, find **InvMove**, Install, then Native build > Build.

By hand: put `invmove-<version>.hnmod` in `<game folder>/horizon-native-mods/`, add the line `mod.invmove:true` to
`horizon-mods.txt`, then Native build > Build in the launcher.

Native mods are compiled into the game and are not sandboxed: build only code you trust. Like the original, this mod
only changes your own client; servers with an anticheat may not like moving while a screen is open.

## Source layout

```
horizon-native.json            bundle manifest (id, version, pinned Horizon and horizon-mixins revisions)
mixins/invmove/
  horizon-mixin.toml           modules and injections
  glue.rs                      the game side: which screen is open, the vanilla module's rules, the input tick,
                               the toggle key, the jump key fix, the background, Options > Mods and Mod Menu
  config.rs                    the settings and their files, InvMove's texts
  screen.rs                    InvMove's config screen; cloth.rs draws it as Cloth Config does
  cloth.rs, cloth_sprites.rs   Cloth Config's screen and its texture as pixels (tools/sprites.mjs makes it)
  background.rs                the in-world menu background switch, beside the function that draws it
  h_*.rs, list.rs, ...         one small helper per injection
  data/icon.png                InvMove's icon, for Mod Menu
  LICENSE, LICENSE.GPL-3.0, NOTICE
tools/                         package.mjs (writes the .hnmod), sprites.mjs
```

The `.hnmod` file is a zip of `horizon-native.json` and `mixins/`.

Built for Horizon `fdc87aecdbd116bf1e08adcd062fcbaea8c2e2ca` (Nightly 2026-11-08) and horizon-mixins
`5a64af938ec86b00d53d16a408aabc0038fb3e57`.

## Licence and credits

GNU LGPL-3.0-only, as the original ([LICENSE](LICENSE), with the GPL-3.0 it builds on in
[LICENSE.GPL-3.0](LICENSE.GPL-3.0); [NOTICE](NOTICE)).

- InvMove: PieKing1215, https://github.com/PieKing1215/InvMove (0.9.6, 26.3)
- The icon and the English texts are InvMove's own.
- The config screen follows Cloth Config by shedaniel (LGPL-3.0) and draws its texture.
- Port: DeportedToHell.
