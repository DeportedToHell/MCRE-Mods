# Roughly Enough Items for Horizon

A port of [Roughly Enough Items (REI)](https://github.com/shedaniel/RoughlyEnoughItems) by shedaniel to
[Horizon](https://github.com/horizon-rust/horizon), the Rust rewrite of Minecraft: Java Edition 26.3,
as a native mod (`.hnmod`). It draws REI's own interface, with REI's textures and layout, over the
game's container screens.

## Features

- **Entry list** right of every container screen and the creative inventory, in pages: the arrows,
  the mouse wheel, or a click on "1/N" to go back to the first page. Enchanted books, potions, splash
  and lingering potions, tipped arrows, spawn eggs and music discs are folded into one cell each, as
  in REI; **Alt+click** unfolds or folds them.
- **Search** with REI's grammar, coloured as you type: words (the name, the tooltip and the id),
  `"quoted phrases"`, `-exclusions`, `a | b` alternatives, `@mod`, `#tooltip`, `$tag`, `*id`, `r/regex/`.
  The field turns red when nothing matches; ↑ and ↓ go through what you searched before.
  One difference from REI: `-word` leaves out every entry whose name, tooltip or id has the word. REI checks
  each of the three on its own, so an entry whose tooltip has the word still shows when its name lacks it.
- **Craftable filter** (the button right of the search field): only what you can craft from your
  inventory right now.
- **Recipes and usages**: left click (or **R**) for how an item is made, right click (or **U**) for what
  it is used for, on the list, the favorites, a recipe's slots or any slot of the screen.
- **The recipe screen** as REI's: category tabs, the workstations at its left, pages, a click on the
  category's name for all categories, **Backspace** back, **Esc** or **E** to close. Crafting (with REI's
  shield decoration, firework rocket and book cloning recipes; what you can craft now comes first),
  smelting, smoking, blasting, campfire, stone cutting, brewing, composting, stripping, smithing, anvil
  (repairs and enchanted books, with their cost), beacon base and payment, tilling, pathing, waxing, wax
  scraping, oxidizing, oxidation scraping.
- **"+" moves a recipe** into the crafting grid (Shift: as many as the items allow) with ordinary slot
  clicks, so it works on any server; red with the missing items when you lack some. **Ctrl+click** on an
  entry crafts it straight away.
- **Favorites** left of the screen: **A** over an item adds it, **A** over a favorite removes it, or drag
  entries in and out. The **+** button opens REI's panel of game mode, weather and time shortcuts to drag
  into the favorites. Kept in `config/roughlyenoughitems/favorites.txt`.
- **Cheat mode** (Shift+click on the gear, or its menu): a click on the list gives one item, a right
  click a stack; in creative as the creative inventory does, elsewhere with `/give`.
- **Tag category** with REI's tree view (drag, zoom, expand, copy), the **choose-page popup**
  (Shift+click on "1/N") and the **display history** (A over a recipe's "+" keeps it above the favorites).
- Hovering the gear or the craftable filter opens REI's menu. **Ctrl+O** hides or shows REI.
- **REI's config screen** (a click on the gear, or "More Options..." in its menu): the twelve
  categories, every option with its description, the live previews, the options search, keybinds with
  Ctrl/Shift/Alt, Cancel and Done. Light and dark theme, recipe borders, larger tabs and arrows, zoom
  (also Ctrl+wheel over the list), ordering, focus mode, search modes, cheats when creative, the
  favorites hint, clickable recipe arrows on the inventory, crafting table, furnaces and brewing stand.
- The same config screen from **Mod Menu** (Configure) and from "Configure..." on Options > Mods >
  Roughly Enough Items, over the title screen or the pause menu, as REI's Mod Menu entry opens it
  ("Load Level First!" and no previews outside a world, as in REI). Mod Menu shows REI's icon.
- The mod's name at the foot of item tooltips, blue and italic, as REI adds it.
- The main switches are also on Options > Mods > Roughly Enough Items, with the keys. Russian labels
  where REI has them when the game is in Russian.

Both this mod and [Just Enough Items for Horizon](../justenoughitems) draw beside the inventory: REI steps
aside while JEI is on, so turn one of them off.

## Install

From the launcher: Mods, find **Roughly Enough Items**, Install, then Native build > Build.

By hand: put `roughlyenoughitems-<version>.hnmod` in `<game folder>/horizon-native-mods/`, add the line
`mod.roughlyenoughitems:true` to `horizon-mods.txt`, then Native build > Build in the launcher.

Native mods are compiled into the game and are not sandboxed: build only code you trust.

## Source layout

```
horizon-native.json            bundle manifest (id, version, pinned Horizon and horizon-mixins revisions)
mixins/roughlyenoughitems/
  horizon-mixin.toml           modules and injections
  runtime.rs                   search, pages, favorites, recipe lookups, settings: no game types (unit tests)
  glue.rs                      the game's items and recipes, mouse and keys, giving and moving items
  view.rs                      drawing
  config.rs                    REI's config screen
  options.rs                   the config screen over the Options screen (Mod Menu), the icon and Mod Menu's metadata
  sprites.rs                   REI's textures as pixel runs (made by tools/sprites.mjs)
  h_*.rs                       one small helper per injection
  data/icon.png                REI's icon, for Mod Menu
  LICENSE, NOTICE
tools/check.sh                 stages the mod on the pinned Horizon source and cargo-checks the game
tools/package.mjs              makes the .hnmod
tools/sprites.mjs              makes sprites.rs from a REI checkout
Cargo.toml                     runs runtime.rs's tests
```

Tests: `cargo test` (Rust 1.99.1, edition 2024).

## Licence

MIT, as REI: see `mixins/roughlyenoughitems/LICENSE` and `NOTICE`. Not affiliated with shedaniel, Mojang or
Microsoft.
