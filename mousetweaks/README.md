# Mouse Tweaks for Horizon

A port of [Mouse Tweaks](https://github.com/YaLTeR/MouseTweaks) by Ivan Molodetskikh (YaLTeR) to
[Horizon](https://github.com/horizon-rust/horizon), the Rust rewrite of Minecraft: Java Edition 26.3,
as a native mod (`.hnmod`).

## Features

The original's tweaks and all seven of its options:

- **RMB drag**: hold the right button with a stack and drag over slots to drop one item into each.
- **LMB drag**: with an item on the cursor, drag to gather the same items; with **Shift**, quick-move
  every slot you pass.
- **LMB drag with an empty hand + Shift**: quick-move every slot you pass.
- **Wheel tweak**: the wheel over a slot moves items one at a time between your inventory and the
  container (crafting results included, even with another item on the cursor). Search order, scroll
  direction (normal, inverted, position-aware) and item scaling as in the original.
- Works on every container screen and the creative inventory (the player's slots). The wheel is left to
  the merchant, stonecutter and loom lists.
- While it is on, it replaces Horizon's built-in Extras > Mouse Tweaks.

Settings: Options > Mods > Mouse Tweaks. Russian labels when the game is in Russian.

Every action is an ordinary slot click, so it works on servers too.

## Install

From the launcher: Mods, find **Mouse Tweaks**, Install, then Native build > Build.

By hand: put `mousetweaks-<version>.hnmod` in `<game folder>/horizon-native-mods/`, add the line
`mod.mousetweaks:true` to `horizon-mods.txt`, then Native build > Build in the launcher.

Native mods are compiled into the game and are not sandboxed: build only code you trust.

## Source layout

```
horizon-native.json            bundle manifest (id, version, pinned Horizon and horizon-mixins revisions)
mixins/mousetweaks/
  horizon-mixin.toml           modules and injections
  runtime.rs                   the tweaks, no game types (unit tests)
  glue.rs                      the game's menu as the tweaks see it
  *.rs                         one small helper per injection
  LICENSE, NOTICE
Cargo.toml                     runs runtime.rs's tests
```

The `.hnmod` file is a zip of `horizon-native.json` and `mixins/`.

Tests: `cargo test` (Rust 1.99.1, edition 2024).

Built for Horizon `2ebf00a0281c592f79191e76fce0e7d522c6f425` (Nightly 2026-11-07) and horizon-mixins
`5a64af938ec86b00d53d16a408aabc0038fb3e57`.

## Licence and credits

BSD 3-Clause License, as the original ([LICENSE](LICENSE), [NOTICE](NOTICE)).

- Mouse Tweaks: Copyright (c) 2023, Ivan Molodetskikh, https://github.com/YaLTeR/MouseTweaks

This port rewrites Mouse Tweaks' logic (`Main.java`, `Config.java`, original commit
`e8ecd08ed94192f6f19d3cbb630707c8670a4d66`) in Rust for Horizon's native mod API. Its settings live on a
Horizon settings page instead of `MouseTweaks.cfg`. Not affiliated with Ivan Molodetskikh, Mojang or
Microsoft.

---

**По-русски.** Порт Mouse Tweaks для Horizon: протяжка правой кнопкой (по одному предмету в слот),
протяжка левой (собрать одинаковые, с Shift — быстро переложить), колесо над слотом перекладывает
предметы по одному. Настройки: Настройки > Моды > Mouse Tweaks.
