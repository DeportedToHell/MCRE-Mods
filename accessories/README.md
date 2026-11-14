<h1 align="left">
  <img src="../.github/assets/stickers/accessories.png" height="64" alt="">
  <img src="../.github/assets/stickers/accessoriesn.png" height="64" alt="">
</h1>

**Version 0.2.1** — the work is still ongoing. Slots, the screen and equipping already work, but few items can be worn without other mods. Remaining work is listed below.

**!!! Two files are needed: the content mod `accessories.hmod` and the native mod `accessories_native.hnmod`. One alone does not work.**

I'll release an updated mod to the launcher as soon as Horizon rolls out an update that merges `.hnmod` into `.hmod`.


## <img src="../.github/assets/stickers/info.png" height="20" alt=""> This is an independent port of the original project.

> It is not affiliated with, endorsed by, or officially associated with the original authors, Wisp Forest.
>
> Original mod: https://github.com/wisp-forest/accessories
>
> Special thanks to [blodhgarm](https://github.com/Dragon-Seeker), [chyzman](https://github.com/chyzman), Dr.Zeal and all contributors for creating the original mod.


## ✨Features

**Accessory slots.** The original's 12 slots in 7 groups: hat, face, necklace, cape, back, two hand slots, two rings, wrist, belt, anklet, shoes and charm. Slots, groups and the player's set are loaded from data packs in the original format, so a data pack can add slots or change their amount, order, icon, rules and what happens to them on death.

**Accessories screen.** Opened with **H** or the small button in the inventory. The player in the middle with armour and cosmetic armour, the accessory grid, a group filter, the inventory and the off hand. Cosmetic slots, a render toggle on every slot, and slots that won't take the carried item darken.

**Screen settings.** Columns, scrolling or pages, unused slots, cosmetics instead of or beside the accessories, one or two player views, where the grid and the filter go, centring, light or dark theme. Everything is saved per player.

**Equipping.** Drag, shift-click, or right-click an accessory in the air. Banners go on the head and the back as cape, gliders and totems only when the config allows it, as in the original. A curse of binding keeps an accessory on, a curse of vanishing destroys it on death, keepInventory keeps them.

**Accessory items.** The original's item components: valid slots, attribute modifiers while worn, slot size bonuses, stack size in a slot, accessories inside accessories and mob effects. Item tooltips show the slots and what an accessory gives. A totem in an accessory slot saves from death.

**On the player.** Accessories are drawn on the player's model where the original puts them, and every player sees the others' accessories.

**Other features.** The `/accessories` commands (`feature`, `edit`, `dump`, `slot`, `enchant`, `components`, `item replace`), the original's config file (`config/accessories.json5`), Mod Menu info and keybind. Works with [Traveler's Backpack](../travelersbackpack): its backpack is worn in the Back slot. English and Russian translations.


## 🚧Remaining work

Not implemented compared to the original:

- **Needs Horizon hooks.** Cosmetic armour drawn instead of the worn one, elytra gliding from an accessory slot, the `keepAccessoryInventory` game rule, piglin, freezing and enderman protection, enchantments of worn accessories, accessories on the screen's player view.
- **Not done yet.** The crafting grid on the screen, the equip reasons in slot tooltips, the config screen in Mod Menu, dispensers, accessories on armour stands and horses, advancement criteria and data-driven renderers.

Differences: the screen's player view always faces the cursor, so the second view isn't turned around.


## Changelog:

### 0.2.1

- [Traveler's Backpack](../travelersbackpack) is worn in the Back slot (its `backSlotIntegration`).
- An item can have its own renderer and refuse a right-click equip, as in the original.
- The screen saves only the slots that were changed, so a backpack worn in the Back slot keeps what goes into it meanwhile.

### 0.2.0

- Item components: valid slots, attributes, stack size, nested accessories and mob effects.
- Slot sizes change with worn items and with `/accessories slot modifier`, by the original's formula.
- Item tooltips (`Slot`, `When Equipped`), the config file, the screen settings panel, pages, cosmetics beside the accessories, dark theme and slot darkening.
- The `/accessories` commands and the totem in an accessory slot.

### 0.1.0

- Slots from data, the accessories screen, equipping, drops on death and accessories on the player's model.


## 📦Installation

Build the content mod with `cargo horizon-mod build` from `content/` and place the resulting `.hmod` in the game's mods folder.

Package the native mod using `tools/package.mjs` and place the resulting `.hnmod` in `<game folder>/horizon-native-mods/`. Add `mod.accessories_native:true` to `horizon-mods.txt`, then select **Native build → Build** in the launcher.

> Native mods are compiled into the game and are not sandboxed. Only build code you trust.


## 🗂️Source layout

```text
content/                         Content mod, generated from the original checkout
horizon-native.json              Native bundle manifest
mixins/accessories_native/
  horizon-mixin.toml             Modules and injections
  glue.rs                        Main mod implementation
  menu.rs                        The accessories menu, slot rules and item components
  screen_relayout.rs             The screen's layout
  h_*.rs                         Injection helpers
  data/icon.png                  Original Accessories icon
tools/
  content.mjs                    Generates content/ from the original checkout
  check.sh                       Stages the mod on pinned Horizon source and checks/builds the game
  package.mjs                    Packages the .hnmod
```


## ⚖️License

Licensed under **MIT**, same as the original project.

**Original authors:** [blodhgarm](https://github.com/Dragon-Seeker), [chyzman](https://github.com/chyzman), Dr.Zeal ([Wisp Forest](https://github.com/wisp-forest))  
**Port author:** [DispatchedToHeaven](https://github.com/DispatchedToHeaven)

See `LICENSE` and `NOTICE` for details.

This project is an independent port and is not affiliated with or endorsed by Wisp Forest, Mojang, or Microsoft.
