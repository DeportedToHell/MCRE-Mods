# ClientSort for Horizon

A port of [ClientSort](https://github.com/TerminalMC/ClientSort) by TerminalMC to
[Horizon](https://github.com/horizon-rust/horizon), the Rust rewrite of Minecraft: Java Edition 26.3,
as a native mod (`.hnmod`).

## Features

- **Trigger buttons** beside each inventory, as in the original: **Sort**, **Restock** (moves the items the
  other inventory already holds) and **Transfer** (moves as much as fits).
- **Middle click** over an inventory sorts it.
- Sort order: the creative inventory's order by default; hold **Shift** for quantity, **Ctrl** for name
  (A-Z), **Alt** for item ID.
- The container, the main inventory and the hotbar are sorted on their own.
- Keys for every operation, including **Fill Stacks**, in Options > Mods > ClientSort (unbound except
  Sort).
- Settings: buttons on/off, default order, click speed, hotbar as its own group, restock by item type
  (ignoring damage and enchantments), button tooltips. Russian labels when the game is in Russian.
- Horizon's own "≡" sort buttons are hidden while ClientSort's are shown.

Every action is an ordinary container click, so it works on servers too.

## Install

From the launcher: Mods, find **ClientSort**, Install, then Native build > Build.

By hand: put `clientsort-<version>.hnmod` in `<game folder>/horizon-native-mods/`, add the line
`mod.clientsort:true` to `horizon-mods.txt`, then Native build > Build in the launcher.

Native mods are compiled into the game and are not sandboxed: build only code you trust.

## Source layout

```
horizon-native.json            bundle manifest (id, version, pinned Horizon and horizon-mixins revisions)
mixins/clientsort/
  horizon-mixin.toml           modules and injections
  runtime.rs                   the mod's logic, no game types (unit tests)
  glue.rs                      reads and clicks the game's inventory
  *.rs                         one small helper per injection
  LICENSE, NOTICE
Cargo.toml                     runs runtime.rs's tests
```

The `.hnmod` file is a zip of `horizon-native.json` and `mixins/`.

Tests: `cargo test` (Rust 1.99.1, edition 2024).

Built for Horizon `2ebf00a0281c592f79191e76fce0e7d522c6f425` (Nightly 2026-11-07) and horizon-mixins
`5a64af938ec86b00d53d16a408aabc0038fb3e57`.

## Licence and credits

Apache License 2.0, as the original ([LICENSE](LICENSE), [NOTICE](NOTICE)).

- ClientSort: Copyright 2024-2026 TerminalMC, https://github.com/TerminalMC/ClientSort
- ClientSort uses code from Mouse Wheelie by Siphalor (Apache-2.0) and Inventory Management by
  Roundaround (MIT).
- The button sprites are ClientSort's own textures.

This port rewrites ClientSort's logic in Rust for Horizon's native mod API. Changes from the original:
the Fill Stacks button is a key only; settings live on Horizon's mod settings page. Not affiliated with
TerminalMC, Mojang or Microsoft.

---

**По-русски.** Порт мода ClientSort для Horizon: кнопки сортировки, пополнения и переноса у каждого
инвентаря, сортировка средней кнопкой мыши (Shift — по количеству, Ctrl — по алфавиту, Alt — по ID).
Настройки: Настройки > Моды > ClientSort.
