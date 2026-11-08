// Pick Up Notifier's game side (`game::pickupnotifier_glue`): what the hooks
// call. It reads the pickups the server announces (`ClientboundTakeItemEntity`,
// as the original's `ClientPacketListenerMixin`), ticks the list with the
// client and draws it after the effects layer of the HUD. The logic is in
// `runtime.rs` (`game::pickupnotifier`).
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use super::Game;
use super::pickupnotifier as pun;
use horizon_client::hud::{HudBatch, rgba};
use horizon_client::items::ItemStack;
use horizon_client::mod_api::{ClientboundPlayPacket, NativeMod};
use horizon_client::text::{Style, Text};
use std::cell::RefCell;

/// What an entry draws: the stack and its name, or the orb's name.
#[derive(Clone, Debug)]
enum Shown {
    Item(Box<ItemStack>, Text),
    Experience(Text),
}

/// One player's list. Splitscreen players are `Game`s of their own on the
/// window's thread, so each keeps its own, by its entity id.
struct Player {
    log: pun::Log<Shown>,
    /// The GUI's height when the list was last drawn (for its max size).
    gui_height: f32,
    /// The partial tick of the last unpaused frame (it stops while paused).
    partial: f32,
}

impl Default for Player {
    fn default() -> Self {
        Player {
            log: pun::Log::default(),
            gui_height: 240.0,
            partial: 0.0,
        }
    }
}

#[derive(Default)]
struct State {
    /// The options, read once from the mod's settings folder.
    config: Option<pun::Config>,
    players: Vec<(i32, Player)>,
}

thread_local! {
    // The window's thread runs every hook this mod uses.
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// This game's player, by the entity id its server gave it.
fn me(game: &Game<'_>) -> Option<i32> {
    game.remote.as_ref().map(|r| r.entity_id)
}

/// `f` on player `id`'s list, made when it has none yet.
fn with_player<R>(id: i32, f: impl FnOnce(&mut Player) -> R) -> R {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let i = match s.players.iter().position(|(p, _)| *p == id) {
            Some(i) => i,
            None => {
                s.players.push((id, Player::default()));
                s.players.len() - 1
            }
        };
        f(&mut s.players[i].1)
    })
}

/// Empties player `id`'s list.
fn forget(id: i32) {
    STATE.with(|s| s.borrow_mut().players.retain(|(p, _)| *p != id));
}

/// The options, read the first time they are needed.
pub(super) fn config(game: &Game<'_>) -> pun::Config {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.config
            .get_or_insert_with(|| pun::Config::load(|key| game.native_setting(pun::ID, key)))
            .clone()
    })
}

/// Keeps `config` and writes the settings that changed to their files.
pub(super) fn store(game: &Game<'_>, config: pun::Config, changed: &[pun::Setting]) {
    for s in changed {
        if let Some(key) = s.key() {
            game.native_set_setting(pun::ID, key, &config.value(*s));
        }
    }
    STATE.with(|st| st.borrow_mut().config = Some(config));
}

/// Russian labels when the game is in Russian.
pub(super) fn ru() -> bool {
    horizon_client::lang::code().starts_with("ru")
}

// ---------------------------------------------------------------------------
// Pickups
// ---------------------------------------------------------------------------

/// A play packet from the server, before the game handles it: the taken
/// entity is still in the world.
pub(super) fn packet(game: &mut Game<'_>, packet: &ClientboundPlayPacket) {
    match packet {
        // A new player (joining, respawning, another dimension): a new list,
        // as `onPlayerLeave` and `onCopy` clear it.
        ClientboundPlayPacket::Login(login) => {
            forget(login.player_id);
            if let Some(id) = me(game) {
                forget(id);
            }
        }
        ClientboundPlayPacket::Respawn(_) => {
            if let Some(id) = me(game) {
                forget(id);
            }
        }
        ClientboundPlayPacket::TakeItemEntity(p) => taken(game, p.item, p.player, p.amount),
        _ => {}
    }
}

/// `AddEntriesHandler.onEntityPickup`: `player` took `amount` of entity
/// `item`; only this client's player's pickups are listed.
fn taken(game: &Game<'_>, item: i32, player: i32, amount: i32) {
    if !game.native_on(pun::ID) || !game.remote.as_ref().is_some_and(|r| r.entity_id == player) {
        return;
    }
    let cfg = config(game);
    // `addEntry`: nothing logged in creative mode when the option says so.
    if cfg.disable_in_creative && game.creative {
        return;
    }
    let id = item as u32;
    let amount = amount.max(0) as u32;
    let entry = if let Some(d) = game.dropped.iter().find(|d| d.id == id) {
        if !cfg.include_items {
            return;
        }
        item_entry(game, &cfg, &d.stack, amount)
    } else if let Some(shot) = game.creatures.projectiles.iter().find(|s| s.id == id) {
        // `AbstractArrow.getPickupItemStackOrigin`.
        let Some(arrow) = shot.arrow.as_ref() else {
            return;
        };
        if !cfg.include_arrows {
            return;
        }
        item_entry(game, &cfg, &arrow.pickup_item, amount)
    } else if let Some(t) = game.tridents.iter().find(|t| t.id == id) {
        // A trident is an `AbstractArrow` too.
        if !cfg.include_arrows {
            return;
        }
        item_entry(game, &cfg, &t.stack, amount)
    } else if let Some(orb) = game.orbs.iter().find(|o| o.id == id) {
        // `addExperienceEntry`: its points, or the number of orbs.
        if !cfg.include_experience || orb.value == 0 {
            return;
        }
        let amount = if cfg.experience_value {
            orb.value
        } else {
            amount
        };
        let name = Text::translate("entity.minecraft.experience_orb", &[]);
        Some(pun::Entry::experience(
            amount,
            orb.ticks,
            &cfg,
            Shown::Experience(name),
        ))
    } else {
        None
    };
    let Some(entry) = entry else {
        return;
    };
    with_player(player, |p| {
        let max = pun::max_size(p.gui_height, &cfg);
        p.log.add(entry, cfg.combine, max);
    });
}

/// `addItemEntry` and the `ItemDisplayEntry` it makes, unless the stack is
/// empty or hidden.
fn item_entry(
    game: &Game<'_>,
    cfg: &pun::Config,
    stack: &ItemStack,
    amount: u32,
) -> Option<pun::Entry<Shown>> {
    if &*stack.item == "minecraft:air" || stack.count == 0 {
        return None;
    }
    let in_tag = |tag: &str, item: &str| {
        game.res
            .items()
            .item_tags
            .get(tag)
            .is_some_and(|t| t.contains(item))
    };
    if pun::hidden(&cfg.hidden_items, &stack.item, in_tag) {
        return None;
    }
    let default_rarity = game.item_rarity(&stack.item);
    let rarity = pun::Rarity::parse(horizon_client::item_tooltip::rarity(stack, &default_rarity));
    let hover = hover_name(game, stack);
    let key = pun::Key::Item {
        id: stack.item.to_string(),
        rarity,
        enchanted: !stack.enchantments().is_empty(),
        name: hover.plain(),
    };
    // `getEntryName`: the item's own name when entries combine always.
    let name = if cfg.combine == pun::Combine::Always {
        item_name(game, stack)
    } else {
        hover
    };
    Some(pun::Entry::item(
        key,
        amount,
        cfg,
        Shown::Item(Box::new(stack.clone()), name),
    ))
}

/// `ItemStack.getHoverName`: the custom name (not italic, as the original
/// shows it), else the item's name.
fn hover_name(game: &Game<'_>, stack: &ItemStack) -> Text {
    if let Some(name) = stack.data().and_then(|d| d.custom_name.as_ref()) {
        return horizon_client::component::name_text(name);
    }
    item_name(game, stack)
}

/// `Item.getName(stack)`: the item name component, else the item's
/// translated name (potions, banners, heads and templates included).
fn item_name(game: &Game<'_>, stack: &ItemStack) -> Text {
    if let Some(name) = stack.data().and_then(|d| d.other("minecraft:item_name")) {
        return horizon_client::component::name_text(name);
    }
    let mut plain = stack.clone();
    if let Some(data) = plain.extra.as_mut() {
        data.custom_name = None;
    }
    let line =
        horizon_client::item_tooltip::name_line(&plain, &|key| game.pack.names.display_get(key));
    Text::legacy(&line)
}

/// `ContainerHelper.clearOrCountMatchingItems` over the player's inventory:
/// the main slots, armor and off hand.
fn inventory_count(game: &Game<'_>, item: &str) -> u32 {
    let count = |s: &Option<ItemStack>| {
        s.as_ref()
            .filter(|s| &*s.item == item)
            .map_or(0, |s| s.count)
    };
    game.inventory.iter().map(count).sum::<u32>()
        + game.armor.iter().map(count).sum::<u32>()
        + count(&game.offhand)
}

// ---------------------------------------------------------------------------
// Ticking
// ---------------------------------------------------------------------------

/// The end of a client tick (`ClientTickEvent.Post`): time runs unless the
/// game is paused; nothing is kept out of a world.
pub(super) fn tick(game: &mut Game<'_>) {
    let Some(id) = me(game) else {
        return;
    };
    if game.title.is_some() || game.player.is_none() || !game.native_on(pun::ID) {
        forget(id);
        return;
    }
    if game.pause {
        return;
    }
    let display_time = config(game).display_time;
    with_player(id, |p| p.log.tick(display_time));
}

// ---------------------------------------------------------------------------
// Drawing
// ---------------------------------------------------------------------------

/// After a HUD layer: the list draws after `minecraft:effects`, as the
/// original's layer follows the status effects (F1 hides both).
pub(super) fn draw(game: &Game<'_>, layer: &str, batch: &mut HudBatch) {
    if layer != "minecraft:effects" || !game.native_on(pun::ID) {
        return;
    }
    let Some(id) = me(game) else {
        return;
    };
    let cfg = config(game);
    with_player(id, |s| {
        s.gui_height = batch.height;
        if s.log.is_empty() || game.player.is_none() {
            return;
        }
        // `getGameTimeDeltaPartialTick(false)`: frozen while paused.
        let partial = if game.pause {
            s.partial
        } else {
            let p = game.clock.fraction() as f32;
            s.partial = p;
            p
        };
        let order = s.log.drawing_order(cfg.corner);
        let font = &game.gui.atlas.font;
        let mut lines = Vec::with_capacity(order.len());
        for &i in &order {
            let e = &s.log.entries[i];
            let text = entry_text(game, &cfg, e);
            let text_width = font.width_text(&text) as i32;
            lines.push((i, text, text_width));
        }
        let widths: Vec<i32> = lines
            .iter()
            .map(|(_, _, w)| pun::entry_width(&cfg, *w))
            .collect();
        let relative: Vec<f32> = order
            .iter()
            .map(|&i| s.log.entries[i].relative(partial, &cfg))
            .collect();
        let places = pun::layout(&cfg, &widths, &relative, (batch.width, batch.height));
        let start = batch.vertices.len();
        for (((i, text, text_width), width), place) in lines.iter().zip(&widths).zip(&places) {
            let e = &s.log.entries[*i];
            draw_entry(
                game,
                batch,
                &cfg,
                e,
                text,
                *text_width,
                *width,
                *place,
                partial,
            );
        }
        // `pose().scale(scale)`: the list was laid out in its own scale.
        let scale = cfg.scale();
        batch.scale_since(start, 0.0, 0.0, scale, scale);
    });
}

/// `createTextComponent`: the entry's pieces joined with spaces, in the
/// rarity's colour (else the text colour). The inventory count is read as it
/// is drawn.
fn entry_text(game: &Game<'_>, cfg: &pun::Config, e: &pun::Entry<Shown>) -> Text {
    let (name, inventory) = match &e.payload {
        Shown::Item(stack, name) => {
            let n = if cfg.inventory_count {
                inventory_count(game, &stack.item)
            } else {
                0
            };
            (name, n)
        }
        Shown::Experience(name) => (name, 0),
    };
    let mut text = Text::empty();
    for (i, part) in pun::parts(cfg, e.is_item(), e.amount, inventory)
        .iter()
        .enumerate()
    {
        if i > 0 {
            text = text.append(" ");
        }
        text = match part {
            pun::Part::Name => text.append(name.clone()),
            pun::Part::Literal(s) => text.append(Text::literal(s.clone())),
        };
    }
    text.with_style(&Style::color(cfg.name_rgb(e.key.rarity())))
}

/// `DisplayEntry.render`: the background, the text, then the sprite.
#[allow(clippy::too_many_arguments)]
fn draw_entry(
    game: &Game<'_>,
    batch: &mut HudBatch,
    cfg: &pun::Config,
    e: &pun::Entry<Shown>,
    text: &Text,
    text_width: i32,
    width: i32,
    place: pun::Placement,
    partial: f32,
) {
    let alpha8 = pun::channel(place.alpha);
    if alpha8 == 0 {
        return;
    }
    let atlas = &game.gui.atlas;
    let (x, y) = (place.x, place.y);
    match cfg.background {
        pun::Background::None => {}
        pun::Background::Chat => {
            // The text background option, as the chat's.
            let opacity = (game.settings.text_background_opacity * place.alpha).clamp(0.0, 1.0);
            let bottom = y + 16 + i32::from(cfg.amount.is_sprite());
            batch.fill(
                atlas,
                (x - 3) as f32,
                y as f32,
                (width + 8) as f32,
                (bottom - y) as f32,
                rgba(0, 0, 0, pun::channel(opacity)),
            );
        }
        pun::Background::Tooltip => {
            // `extractTooltipBackground(x, y + 3, width, 9)`.
            for sprite in [
                "gui/sprites/tooltip/background",
                "gui/sprites/tooltip/frame",
            ] {
                horizon_client::ui::nine_slice(
                    batch,
                    atlas,
                    sprite,
                    (x - 12) as f32,
                    (y + 3 - 12) as f32,
                    (width + 24) as f32,
                    33.0,
                    rgba(255, 255, 255, alpha8),
                );
            }
        }
    }
    let (text_x, sprite_x) = pun::text_and_sprite_x(cfg, x, text_width);
    batch.text_styled(
        atlas,
        text,
        text_x as f32,
        (y + 4) as f32,
        rgba(255, 255, 255, alpha8),
        true,
    );
    if !cfg.draw_sprite {
        return;
    }
    match &e.payload {
        Shown::Item(stack, _) => {
            let start = batch.vertices.len();
            game.draw_item_model(batch, stack, sprite_x as f32, y as f32);
            if let Some((sx, sy)) = pun::pop_scale(e.pop, partial) {
                batch.scale_since(start, (sprite_x + 8) as f32, (y + 12) as f32, sx, sy);
            }
            fade(batch, start, place.alpha);
            if pun::sprite_count_shown(cfg, e.amount) {
                draw_count(game, batch, e.amount, sprite_x, y, alpha8);
            }
        }
        Shown::Experience(_) => {
            // The orb's frame of `textures/entity/experience/experience_orb.png`.
            let Some(r) = atlas
                .sprites
                .get("entity/experience/experience_orb")
                .copied()
            else {
                return;
            };
            let icon = pun::xp_icon(e.amount);
            let (u, v) = ((icon % 4) as f32 / 4.0, (icon / 4) as f32 / 4.0);
            let (du, dv) = (r.u1 - r.u0, r.v1 - r.v0);
            let mut part = r;
            part.u0 = r.u0 + u * du;
            part.v0 = r.v0 + v * dv;
            part.u1 = part.u0 + du / 4.0;
            part.v1 = part.v0 + dv / 4.0;
            let (red, green, blue) = pun::xp_color(e.ticks);
            batch.rect_uv(
                sprite_x as f32,
                y as f32,
                16.0,
                16.0,
                part,
                rgba(red, green, blue, alpha8),
            );
        }
    }
}

/// The item's translucency (the original's `TransparentGuiItemRenderState`).
fn fade(batch: &mut HudBatch, start: usize, alpha: f32) {
    if alpha >= 1.0 {
        return;
    }
    for v in &mut batch.vertices[start..] {
        let a = ((v.color >> 24) as f32 * alpha).round() as u32;
        v.color = (v.color & 0x00FF_FFFF) | (a.min(255) << 24);
    }
}

/// `renderGuiItemDecorations`: the amount, shortened, in the sprite's
/// bottom right corner, shrunk to fit 16 pixels.
fn draw_count(game: &Game<'_>, batch: &mut HudBatch, amount: u32, x: i32, y: i32, alpha8: u8) {
    let atlas = &game.gui.atlas;
    let label = pun::shorten(amount);
    let width = atlas.font.width(&label);
    let scale = (16.0 / width).min(1.0);
    let px = ((x + 17) as f32 / scale - width) as i32;
    let py = ((y + pun::LINE_HEIGHT * 2) as f32 / scale - pun::LINE_HEIGHT as f32) as i32;
    let start = batch.vertices.len();
    batch.text(
        atlas,
        &label,
        px as f32,
        py as f32,
        rgba(255, 255, 255, alpha8),
        true,
    );
    batch.scale_since(start, 0.0, 0.0, scale, scale);
}

// ---------------------------------------------------------------------------
// Options > Mods, the settings page and the command
// ---------------------------------------------------------------------------

/// The mod's row in Options > Mods.
pub(super) fn listed(mods: &mut Vec<NativeMod>) {
    mods.push(NativeMod {
        id: pun::ID.into(),
        title: format!("{} {}", pun::NAME, pun::VERSION),
        description: pun::DESCRIPTION.into(),
        keys: Vec::new(),
    });
}

/// The settings page: one button per option.
pub(super) fn page(game: &mut Game<'_>, mod_id: &str, buttons: &mut Vec<String>) {
    if mod_id != pun::ID {
        return;
    }
    let cfg = config(game);
    let ru = ru();
    // Mod Menu's "Configure..." first: the original's config screen.
    buttons.push(if ru { "Настройки..." } else { "Configure..." }.to_owned());
    buttons.extend(pun::PAGE.iter().map(|s| cfg.label(*s, ru)));
}

/// A click on the page: the option's next value (Shift: the previous one).
pub(super) fn click(game: &mut Game<'_>, mod_id: &str, button: usize) {
    if mod_id != pun::ID {
        return;
    }
    // The first button and Mod Menu's Configure: the config screen.
    if button == 0 || button == super::pickupnotifier_config::MODMENU_CONFIGURE {
        super::pickupnotifier_config::open(game, button != 0);
        return;
    }
    let Some(&setting) = pun::PAGE.get(button - 1) else {
        return;
    };
    let mut cfg = config(game);
    let changed = cfg.click(setting, game.has_shift_down());
    store(game, cfg, &changed);
}

/// The command's name, for the chat box's completion.
pub(super) fn commands(commands: &mut Vec<&'static str>) {
    commands.push(pun::COMMAND);
}

/// A line in chat, in the game's language.
fn say(game: &mut Game<'_>, en: impl AsRef<str>, rus: impl AsRef<str>) {
    let line = if ru() { rus.as_ref() } else { en.as_ref() };
    game.native_chat(line);
}

/// `/pickupnotifier ...`.
pub(super) fn command(game: &mut Game<'_>, line: &str, ran: &mut bool) {
    let Some(command) = pun::parse_command(line) else {
        return;
    };
    if !game.native_on(pun::ID) {
        return;
    }
    *ran = true;
    match command {
        pun::Command::Open => {
            game.list_native_mods();
            let index = game
                .settings
                .mods
                .installed
                .iter()
                .position(|m| m.id == pun::ID && m.native.is_some());
            if let Some(index) = index {
                game.open_options_at(false, horizon_client::options_ui::Page::NativeMod(index));
            }
        }
        pun::Command::Hide(entry) => {
            let entry = entry.or_else(|| {
                game.inventory
                    .get(game.selected)
                    .and_then(|s| s.as_ref())
                    .map(|s| s.item.to_string())
            });
            let Some(entry) = entry else {
                say(
                    game,
                    "Hold an item or name one: /pickupnotifier hide minecraft:dirt",
                    "Возьмите предмет в руку или укажите его: /pickupnotifier hide minecraft:dirt",
                );
                return;
            };
            let mut cfg = config(game);
            if pun::hide(&mut cfg, &entry) {
                store(game, cfg, &[pun::Setting::HiddenItems]);
                say(game, format!("Hidden: {entry}"), format!("Скрыто: {entry}"));
            } else {
                say(
                    game,
                    format!("Already hidden: {entry}"),
                    format!("Уже скрыто: {entry}"),
                );
            }
        }
        pun::Command::Show(entry) => {
            let entry = entry.or_else(|| {
                game.inventory
                    .get(game.selected)
                    .and_then(|s| s.as_ref())
                    .map(|s| s.item.to_string())
            });
            let Some(entry) = entry else {
                say(game, pun::help(false), pun::help(true));
                return;
            };
            let mut cfg = config(game);
            if pun::show(&mut cfg, &entry) {
                store(game, cfg, &[pun::Setting::HiddenItems]);
                say(
                    game,
                    format!("Shown again: {entry}"),
                    format!("Снова показывается: {entry}"),
                );
            } else {
                say(
                    game,
                    format!("Not hidden: {entry}"),
                    format!("Не было скрыто: {entry}"),
                );
            }
        }
        pun::Command::List => {
            let list = config(game).hidden_items.join(", ");
            if list.is_empty() {
                say(game, "No hidden items", "Скрытых предметов нет");
            } else {
                say(game, format!("Hidden: {list}"), format!("Скрыты: {list}"));
            }
        }
        pun::Command::Clear => {
            if let Some(id) = me(game) {
                forget(id);
            }
        }
        pun::Command::Help => say(game, pun::help(false), pun::help(true)),
    }
}
