# Accessories

Based on Accessories 1.4.3-beta (Minecraft 1.21.11).

**!!! Two files are needed: the content mod `accessories.hmod` and the native mod `accessories_native.hnmod`. One alone does not work.**

## This is an independent port of the original project.

> It is not affiliated with, endorsed by, or officially associated with the original authors, Wisp Forest
> (blodhgarm, chyzman, Dr.Zeal).
>
> Original mod: https://github.com/wisp-forest/accessories
>
> Ported by DispatchedToHeaven, with DeportedToHell.

## ✨Features (0.1.0, in progress)

> 0.1.0 builds against Horizon Nightly fdc87ae; it has not been tested in the game yet.

**Accessory slots from data.** Slot types, slot groups and entity bindings are read from
`data/<namespace>/accessories/{slot,group,entity}/*.json`, the same files and formats as the original: the 12 built-in
slots (hat, face, necklace, cape, back, hand ×2, ring ×2, wrist, belt, anklet, shoes, charm), their 7 groups and the
player's binding. A data pack can add slots, change their amount, order, icon, validators and drop rule. Item tags
`accessories:<slot>` and `accessories:any` make items fit a slot.

**The accessories screen.** Opened with **H** or the small button in the inventory (also creative's inventory tab):
the player in the middle with their armour and cosmetic armour, the accessories grid on the left (2 columns, scrolled
past 7 rows), the group filter on the right, the inventory and off hand below. The cosmetic button switches the grid
to the cosmetic slots; each slot has its render toggle. Light theme; slot icons, tooltips and the original's textures.
H on the screen closes it, the back button returns to the inventory.

**Equipping.** Drag items in, shift-click from the inventory (armour to its slot, a shield to the off hand, an
accessory to its first free slot), or right-click an accessory in the air (without crouching), which takes the first
free slot that fits or swaps with a worn one. The equip sound plays.

**Rules as in the original.** Banners go into the hat and cape slots; gliders and totems only where the config allows
it (off by default). A curse of binding keeps an accessory on a survival player. On death each slot's drop rule
applies: keepInventory keeps them, the curse of vanishing destroys, the rest drop.

**On the player.** Accessories (or their cosmetics) are drawn on every player's model where the original's default
renderer puts them (on the head, face, back, neck, belt, arms, wrists, hands, ankles and feet); the charm slot has no
default place, as in the original. Every player sees the others' accessories.

**Saved with the player** (their `accessories:inventory_holder` data) and given back when a slot shrinks or goes away.

## Not done yet (compared with the original)

- Attribute modifiers, mob effects, enchantments and nesting from Accessories' own item components
  (`accessories:attributes`, `stack_settings`, `nested_accessories`, `mob_effects`).
- Gliding with an elytra, saving from death with a totem, piglin/freeze/enderman protection from accessory slots.
- The settings panel (columns, paginated grid, side by side cosmetics, double entity view, positions, dark theme), the
  crafting grid on the screen, slot darkening, equip reasoning tooltips, the item tooltip's "Slot:" lines.
- Cosmetic armour shown instead of the worn armour; banners and gliders through the vanilla layers; first-person arms.
- The `/accessories` commands, the config file and screen, the `accessories.keepAccessoryInventory` game rule,
  dispensers, other entities' accessories (armour stands, horses), the advancement criteria.

## 📦Installation

Build the content mod with `cargo horizon-mod build` from `content/` and place the resulting `.hmod` in
`<game folder>/horizon-mods/`. Place `accessories_native-<version>.hnmod` in `<game folder>/horizon-native-mods/`, add
`mod.accessories_native:true` to `horizon-mods.txt`, then run **Native build → Build** in the launcher.

> Native mods are compiled into the game and are not sandboxed. Only build code you trust.

## 🗂️Source layout

```text
horizon-native.json                Native bundle manifest
content/                           The content mod (horizon-mod.toml, assets/, data/), from tools/content.mjs
mixins/accessories_native/
  horizon-mixin.toml               Modules and injections
  glue.rs                          Data, holder, server tick, screen, renderer, Mod Menu
  menu.rs                          The accessories menu on the server and the slots' rules
  screen_relayout.rs               The open screen laid out again
  h_*.rs                           Injection helpers
  data/icon.png                    The original's icon
tools/
  content.mjs                      Regenerates content/ from a checkout of the original
  check.sh                         Checks and builds against the pinned Horizon source
  package.mjs                      Packages the .hnmod
```
