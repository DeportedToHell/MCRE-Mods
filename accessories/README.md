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

## ✨Features (0.2.1)

> Tested in the game (Horizon Nightly fdc87ae, single player): the screen from the button and from H, equipping by
> dragging, shift-click and right click, armour in the screen, the cosmetic switch, render toggles, death drops and
> keepInventory, banners on the head, the settings panel (columns, unused slots, double view, widget position, dark
> theme, scrolling), the item tooltips, `/accessories slot modifier` and `slot get`, `/accessories components` (slot
> validation and attributes on the held item), attribute modifiers while worn and taken off, a totem saving from death
> in the charm slot. Not tested yet: other players over LAN, data packs adding slots, the paginated grid's pages
> (it pages past 21 slots).

**Accessory slots from data.** Slot types, slot groups and entity bindings are read from
`data/<namespace>/accessories/{slot,group,entity}/*.json`, the same files and formats as the original: the 12 built-in
slots (hat, face, necklace, cape, back, hand ×2, ring ×2, wrist, belt, anklet, shoes, charm), their 7 groups and the
player's binding. A data pack can add slots, change their amount, order, icon, validators and drop rule. Item tags
`accessories:<slot>` and `accessories:any` make items fit a slot.

**Item components.** Items carry Accessories' components from content mods' data or from `/accessories components`:
`accessories:slot_validation` (valid and invalid slots), `accessories:attributes` (attribute modifiers while worn,
stackable or not, slot sizes `accessories:<slot>`, slot validation), `accessories:stack_settings` (how many go in a
slot), `accessories:nested_accessories` (accessories inside an accessory, worn too) and `accessories:mob_effects`
(constant effects while worn, delayed ones again and again).

**The accessories screen.** Opened with **H** or the small button in the inventory (creative's inventory tab too): the
player with their armour and cosmetic armour, the accessories grid (scrolled past 7 rows, or paged), the group filter,
the inventory and off hand. The cosmetic button switches the grid to the cosmetic slots or shows them beside; each slot
has its render toggle; carrying a stack darkens the slots that won't take it. The settings panel (the wrench) sets the
columns, scrollable or paginated, unused slots, the cosmetics' viewing mode, one or two views of the player, the grid's
and the filter's sides, centring, the light or dark theme, and the "Equipped in" line. H closes the screen, the back
button returns to the inventory.

**Item tooltips.** An item that fits says where (`Slot: Ring`, `Slots: Any except …`) and what it gives when worn
(`When Equipped:` or for each slot).

**Equipping.** Drag items in, shift-click from the inventory (armour to its slot, a shield to the off hand, an
accessory to its first free slot), or right-click an accessory in the air (`equipControl`: without crouching, with it,
or never), which takes the first free slot that fits or swaps with a worn one. The equip sound plays.

**Rules as in the original.** Banners go into the hat and cape slots; gliders and totems only where the config allows
it (off by default; the totem validator answers as shipped). A curse of binding keeps an accessory on a survival
player. On death each slot's drop rule applies: keepInventory keeps them, the curse of vanishing destroys, the rest
drop. A totem in an accessory slot saves from death as one in the hand does.

**On the player.** Accessories (or their cosmetics, `showCosmeticAccessories`) are drawn on every player's model where
the original's default renderer puts them; `disabledDefaultRenders` turns that off per slot. A banner in the hat slot
is worn on the head as a vanilla banner is; banners and gliders elsewhere are not drawn, as in the original.

**Commands** (level 2): `/accessories feature (enable|disable) (banner|glider|totem)`, `edit [player]` (another
player's accessories too), `dump (slots|groups|entity_bindings)`, `slot get <player> <slot> [scale]`,
`slot modifier clear <player> [slot]`, `slot modifier <player> <slot> <id> (add <amount> <operation> | remove | get)`,
`enchant <player> <enchantment> [level]`, `components slot (add|remove) (valid|invalid) <slot>`,
`components stack-sizing (useStackSize <bool> | <size>)`, `components attribute <attribute> <id> (add <amount>
<operation> <slot> <stackable> [validation] | remove | get)`, `components nest <item>`,
`item replace entity <player> <slot>/<index> with <item> [count]`.

**Other mods.** `accessories:custom_renderer` keeps the default renderer off an item (its own mod draws it), and
`stack_settings`' `can_equip_from_use` decides a right click, as in the original: [Traveler's Backpack](../travelersbackpack)
uses both for its Back slot. The screen writes back only the slots it changed, so another mod changing a worn item
meanwhile (a backpack's contents) keeps its change.

**Config:** `config/accessories.json5`, owo-config's file of the original (content, client and screen options, slot
amount modifiers).

**Saved with the player** (their `accessories:inventory_holder` data, slot size modifiers included) and given back
when a slot shrinks or goes away.

## Not done (compared with the original)

These need hooks Horizon doesn't have at this Nightly:

- Cosmetic armour drawn instead of the worn armour (the armour layers' items can't be swapped from a mod).
- Gliding with an elytra in an accessory slot (Horizon's gliding asks the chest slot only).
- The `accessories.keepAccessoryInventory` game rule (mods can't add game rules).
- Piglin, freezing and enderman protection, and enchantments of worn accessories acting on the wearer.
- The accessories drawn on the screen's player view, and its second view turned around (the view always faces the
  pointer).

Not done yet: the crafting grid on the screen, the equip reasoning lines in slot tooltips, the config screen in Mod Menu
(the file and the settings panel stand in), dispensers, other entities' accessories (armour stands, horses), the
advancement criteria, data-driven renderers, and `components effect` and the other `item` subcommands.

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
  glue.rs                          Data, config, holder, server tick, commands, screen, renderer, Mod Menu
  menu.rs                          The accessories menu on the server, the slots' rules and the item components
  screen_relayout.rs               The open screen laid out again
  h_*.rs                           Injection helpers
  data/icon.png                    The original's icon
tools/
  content.mjs                      Regenerates content/ from a checkout of the original
  check.sh                         Checks and builds against the pinned Horizon source
  package.mjs                      Packages the .hnmod
```
