// ShulkerBoxTooltip in the game (not in the Cargo library): the preview
// providers, the tooltip's lines and key hints, the preview window drawn in
// the item's tooltip, locking the tooltip, the ender chest's contents (the
// host's own, a server's through the mod's messages, or the cache filled when
// the player opens it), the bundle wheel, the mod's row and page in
// Options > Mods and its data files.
//
// ShulkerBoxTooltip hooks `ItemStack.getTooltipLines` (the lines and hints),
// `ItemStack.getTooltipImage` (its preview component), `AbstractContainerScreen`
// (locking) and `BundleMouseActions` (the wheel). Here: the `tooltip` hook,
// `draw_stack_tooltip_with` (the whole tooltip is drawn here while a preview
// shows, so the window can sit inside or outside the box), `draw_slot_tooltip`
// and `slot_at` (locking), and `bundle_scroll` (the wheel).
use super::shulkerboxtooltip as sbt;
use super::shulkerboxtooltip_lang::LANG;
use super::shulkerboxtooltip_sprites as sprites;
use super::{Game, Screen};
use horizon_client::hud::HudBatch;
use horizon_client::items::ItemStack;
use horizon_client::mod_api::NativeMod;
use horizon_client::text::{self, Text};
use sbt::{Config, Opt, PreviewType};
use std::cell::{Cell, RefCell};
use std::sync::atomic::Ordering;

// ---- texts --------------------------------------------------------------------------------

pub(super) fn russian() -> bool {
    horizon_client::lang::code().starts_with("ru")
}

/// One of ShulkerBoxTooltip's texts in the game's language (English for any
/// other than Russian), else the game's own translation of `key`.
pub(super) fn tr(key: &str) -> Text {
    match LANG.iter().find(|(k, _, _)| *k == key) {
        Some((_, en, ru)) => Text::literal(if russian() { *ru } else { *en }),
        None => Text::translate(key, &[]),
    }
}

/// Whether ShulkerBoxTooltip has text `key` (`Language.has`).
pub(super) fn has_text(key: &str) -> bool {
    LANG.iter().any(|(k, _, _)| *k == key)
}

/// A key's name as the game shows it (`InputConstants.Key.getDisplayName`).
pub(super) fn key_name(key: &str) -> Text {
    horizon_client::keybinds::display_name(key)
}

// ---- the configuration --------------------------------------------------------------------

thread_local! {
    /// The configuration in effect, read from the file at first use.
    static CONFIG: RefCell<Option<Config>> = const { RefCell::new(None) };
}

fn config_path() -> Option<std::path::PathBuf> {
    Game::native_config_dir().ok().map(|d| d.join(sbt::CONFIG_FILE))
}

/// `ConfigurationHandler.register`: the file's configuration (the defaults
/// when it is missing or can't be read), written back at once.
fn load() -> Config {
    let read = config_path().and_then(|p| std::fs::read_to_string(p).ok());
    let config = read.as_deref().and_then(Config::from_json5).unwrap_or_default();
    save_file(&config);
    config
}

fn save_file(config: &Config) {
    if let Some(path) = config_path() {
        let text = config.to_json5();
        if std::fs::read_to_string(&path).ok().as_deref() != Some(text.as_str()) {
            let _ = std::fs::write(path, text);
        }
    }
}

/// The saved configuration (`ShulkerBoxTooltip.savedConfig`).
pub(super) fn config() -> Config {
    CONFIG.with(|c| c.borrow_mut().get_or_insert_with(load).clone())
}

/// `ConfigurationHandler.saveToFile`: the new configuration, in effect and in the file.
pub(super) fn store(config: Config) {
    save_file(&config);
    CONFIG.with(|c| *c.borrow_mut() = Some(config));
}

/// The server's `@Synchronize`d values in effect: the configuration's own
/// in single player, what the server said otherwise (none until it answers:
/// `reinitClientSideSyncedValues`).
fn server_values(game: &Game<'_>, c: &Config) -> (bool, usize) {
    if game.remote.is_none() {
        (c.on(Opt::ClientIntegration), c.choice(Opt::EnderSync))
    } else {
        NET.with(|n| n.borrow().server).unwrap_or((false, 0))
    }
}

// ---- keys ---------------------------------------------------------------------------------

/// Whether key `name` is held (`InputConstants.isKeyDown`).
fn held(game: &Game<'_>, name: &str) -> bool {
    if name == sbt::UNBOUND {
        return false;
    }
    horizon_client::keybinds::key_code(name).is_some_and(|code| game.native_is_key_down(code))
}

/// The preview keys as `updatePreviewKeys` reads them: preview, full, lock.
fn keys(game: &Game<'_>, c: &Config) -> (bool, bool, bool) {
    (held(game, c.key(Opt::PreviewKey)), held(game, c.key(Opt::FullPreviewKey)), held(game, c.key(Opt::LockKey)))
}

/// `getCurrentPreviewType`.
fn current_type(game: &Game<'_>, c: &Config, has_full: bool) -> PreviewType {
    let (preview, full, _) = keys(game, c);
    sbt::preview_type(c.on(Opt::AlwaysOn) || preview, full, c.on(Opt::SwapModes), has_full)
}

// ---- providers ----------------------------------------------------------------------------

/// What `ShulkerBoxTooltip.registerProviders` registers for an item, and
/// `GenericContainerPreviewProvider`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    /// `ShulkerBoxPreviewProvider`: the dye, if any.
    Shulker(Option<usize>),
    /// `InventoryAwarePreviewProvider`: the container's size, the row size,
    /// and whether it is a `RandomizableContainer` (can use loot tables).
    Block { size: usize, row: i32, loot: bool },
    /// `LecternPreviewProvider`: the book from the block entity data.
    Lectern,
    /// `EnderChestPreviewProvider`.
    Ender,
    /// `BundlePreviewProvider`: the dye, if any.
    Bundle(Option<usize>),
    /// `GenericContainerPreviewProvider`.
    Generic,
}

const WOODS: [&str; 12] = ["acacia", "bamboo", "birch", "cherry", "crimson", "dark_oak", "jungle", "mangrove", "oak", "pale_oak", "spruce", "warped"];
const COPPER: [&str; 8] = ["", "exposed_", "weathered_", "oxidized_", "waxed_", "waxed_exposed_", "waxed_weathered_", "waxed_oxidized_"];

fn dye_of(name: &str, suffix: &str) -> Option<Option<usize>> {
    if name == suffix {
        return Some(None);
    }
    let dye = name.strip_suffix(suffix)?.strip_suffix('_')?;
    sbt::DYES.iter().position(|(d, _)| *d == dye).map(Some)
}

/// The registered provider of an item.
fn registered(item: &str) -> Option<Kind> {
    let name = item.strip_prefix("minecraft:")?;
    if let Some(dye) = dye_of(name, "shulker_box") {
        return Some(Kind::Shulker(dye));
    }
    if let Some(dye) = dye_of(name, "bundle") {
        return Some(Kind::Bundle(dye));
    }
    let block = |size, row, loot| Some(Kind::Block { size, row, loot });
    match name {
        "chest" | "trapped_chest" | "barrel" => block(27, 9, true),
        "furnace" | "blast_furnace" | "smoker" => block(3, 3, false),
        "dropper" | "dispenser" => block(9, 3, true),
        "hopper" => block(5, 5, true),
        "brewing_stand" => block(5, 5, false),
        "chiseled_bookshelf" => block(6, 3, false),
        "decorated_pot" => block(1, 1, true),
        "lectern" => Some(Kind::Lectern),
        "ender_chest" => Some(Kind::Ender),
        _ if COPPER.iter().any(|p| name.strip_prefix(p) == Some("copper_chest")) => block(27, 9, true),
        _ if WOODS.iter().any(|w| name.strip_suffix("_shelf") == Some(*w)) => block(3, 3, false),
        _ => None,
    }
}

/// A preview provider for a stack (`PreviewContext` + `PreviewProvider`).
struct Provider<'s> {
    kind: Kind,
    stack: &'s ItemStack,
}

fn has_container(stack: &ItemStack) -> bool {
    stack.data().is_some_and(|d| !d.container.is_empty())
}

fn component<'a>(stack: &'a ItemStack, id: &str) -> Option<&'a horizon_world::nbt::Tag> {
    stack.data().and_then(|d| d.other(id))
}

/// `PreviewProviderRegistry.get(stack)`.
fn provider<'s>(c: &Config, stack: &'s ItemStack) -> Option<Provider<'s>> {
    let kind = registered(&stack.item).or_else(|| (c.on(Opt::GenericPreview) && has_container(stack)).then_some(Kind::Generic))?;
    Some(Provider { kind, stack })
}

/// The ender chest's slots in effect for the client.
fn ender_items(game: &Game<'_>, c: &Config) -> Option<Vec<Option<ItemStack>>> {
    if ender_integration(game, c) {
        if game.remote.is_none() {
            if let Some(key) = game.ender_open
                && let Some(horizon_client::containers::BlockEntity::Chest(items)) = game.world.block_entities.get(&key)
            {
                return Some(items.clone());
            }
            return Some(game.ender_items.clone());
        }
        return Some(NET.with(|n| n.borrow().ender.clone()).unwrap_or_else(|| vec![None; 27]));
    }
    CACHE.with(|c| c.borrow().items.clone())
}

/// `EnderChestPreviewProvider.isServerIntegrationActive`.
fn ender_integration(game: &Game<'_>, c: &Config) -> bool {
    let (client, sync) = server_values(game, c);
    c.on(Opt::ServerIntegration) && client && sync != 0
}

impl Provider<'_> {
    fn can_use_loot(&self) -> bool {
        match self.kind {
            Kind::Shulker(_) | Kind::Generic => true,
            Kind::Block { loot, .. } => loot,
            _ => false,
        }
    }
    fn has_loot(&self) -> bool {
        component(self.stack, "minecraft:container_loot").is_some()
    }

    /// `getInventoryMaxSize`.
    fn max_size(&self, game: &Game<'_>) -> usize {
        match self.kind {
            Kind::Shulker(_) | Kind::Generic => 27,
            Kind::Block { size, .. } => size,
            Kind::Lectern => 1,
            Kind::Ender => {
                if game.player.is_some() {
                    27
                } else {
                    0
                }
            }
            Kind::Bundle(_) => 64,
        }
    }

    fn bundle_items(&self) -> &[ItemStack] {
        self.stack.data().map_or(&[][..], |d| &d.bundle_contents[..])
    }

    /// `getInventory`.
    fn inventory(&self, game: &Game<'_>, c: &Config) -> Vec<Option<ItemStack>> {
        let size = self.max_size(game);
        let mut inv = vec![None; size];
        match self.kind {
            Kind::Shulker(_) | Kind::Generic | Kind::Block { .. } => {
                for (slot, s) in self.stack.data().map_or(&[][..], |d| &d.container[..]) {
                    if let Some(cell) = inv.get_mut(usize::from(*slot)) {
                        *cell = Some(s.clone()).filter(|s| s.count > 0);
                    }
                }
            }
            Kind::Lectern => {
                if let Some(book) = component(self.stack, "minecraft:block_entity_data").and_then(|t| t.get("Book"))
                    && let Some(cell) = inv.get_mut(0)
                {
                    *cell = horizon_items::item_nbt::item_from_nbt(book);
                }
            }
            Kind::Ender => {
                if game.player.is_none() {
                    return Vec::new();
                }
                return ender_items(game, c).unwrap_or_default();
            }
            Kind::Bundle(_) => {
                let items = self.bundle_items();
                let active = items.len();
                let start = active.saturating_sub(items.len());
                for (i, s) in items.iter().enumerate() {
                    if let Some(cell) = inv.get_mut(start + i)
                        && start + i < active
                    {
                        *cell = Some(s.clone());
                    }
                }
            }
        }
        inv
    }

    /// `getActiveSlotCount`.
    fn active_slots(&self, game: &Game<'_>) -> usize {
        match self.kind {
            Kind::Bundle(_) => self.bundle_items().len(),
            _ => self.max_size(game),
        }
    }

    /// `getSelectedSlot`.
    fn selected_slot(&self) -> Option<usize> {
        match self.kind {
            Kind::Bundle(_) => {
                let selected = self.stack.data().map_or(-1, |d| d.bundle_selected.0);
                usize::try_from(selected).ok()
            }
            _ => None,
        }
    }

    /// `getMaxRowSize` (0: the configuration's).
    fn max_row(&self, c: &Config) -> i32 {
        match self.kind {
            Kind::Shulker(_) | Kind::Generic => 9,
            Kind::Block { row, .. } => row,
            Kind::Lectern => 1,
            Kind::Ender => 0,
            Kind::Bundle(_) => {
                let global = c.max_row_size();
                if global == 9 { 8 } else { global }
            }
        }
    }

    /// `getCompactMaxRowSize` (0: the configuration's).
    fn compact_row(&self, c: &Config) -> i32 {
        match self.kind {
            Kind::Ender => 0,
            _ => self.max_row(c),
        }
    }

    /// `shouldDisplay`.
    fn should_display(&self, game: &Game<'_>, c: &Config) -> bool {
        match self.kind {
            Kind::Ender => {
                if game.player.is_none() {
                    return false;
                }
                if ender_integration(game, c) {
                    return ender_items(game, c).is_some_and(|i| i.iter().any(Option::is_some));
                }
                CACHE.with(|c| c.borrow().items.as_ref().is_some_and(|i| i.iter().any(Option::is_some)))
            }
            Kind::Bundle(_) => c.choice(Opt::BundleTheme) != 1 && !self.bundle_items().is_empty(),
            _ => {
                if self.can_use_loot() && self.has_loot() {
                    return false;
                }
                self.inventory(game, c).iter().any(Option::is_some)
            }
        }
    }

    /// `showTooltipHints`.
    fn show_hints(&self, game: &Game<'_>, c: &Config) -> bool {
        match self.kind {
            Kind::Shulker(_) | Kind::Ender | Kind::Bundle(_) => true,
            Kind::Block { .. } => self.should_display(game, c),
            Kind::Lectern => component(self.stack, "minecraft:block_entity_data").is_some(),
            Kind::Generic => has_container(self.stack),
        }
    }

    fn compact_hint(&self) -> &'static str {
        match self.kind {
            Kind::Bundle(_) => "shulkerboxtooltip.hint.sorted",
            _ => "shulkerboxtooltip.hint.compact",
        }
    }

    /// `getWindowColorKey`: the colour's index in `color_keys()`.
    fn color_index(&self) -> usize {
        match self.kind {
            Kind::Shulker(None) => 3,
            Kind::Shulker(Some(d)) => 4 + d,
            Kind::Bundle(None) => 20,
            Kind::Bundle(Some(d)) => 21 + d,
            Kind::Ender => 1,
            Kind::Generic => 2,
            Kind::Block { .. } | Kind::Lectern => 0,
        }
    }

    /// `getTheme`: the vanilla look.
    fn vanilla_theme(&self, c: &Config) -> bool {
        match self.kind {
            Kind::Bundle(_) => c.choice(Opt::BundleTheme) != 0,
            _ => c.choice(Opt::Theme) == 1,
        }
    }

    /// `addTooltip`.
    fn tooltip(&self, game: &Game<'_>, c: &Config) -> Vec<Text> {
        let gray = |t: Text| t.with_color(text::GRAY);
        let count_line = |items: &[Option<ItemStack>]| {
            let n = items.iter().filter(|s| s.is_some()).count();
            if n > 0 {
                let template = tr("container.shulkerbox.contains").plain();
                gray(Text::literal(template.replacen("%s", &n.to_string(), 1)))
            } else {
                gray(tr("container.shulkerbox.empty"))
            }
        };
        match self.kind {
            Kind::Ender => {
                let cached = CACHE.with(|c| c.borrow().items.is_some());
                if ender_integration(game, c) || cached {
                    if current_type(game, c, true) == PreviewType::Full {
                        return Vec::new();
                    }
                    return vec![count_line(&self.inventory(game, c))];
                }
                vec![gray(tr("container.shulkerbox.no_cache"))]
            }
            Kind::Bundle(_) => Vec::new(),
            _ => {
                if self.can_use_loot() && self.has_loot() {
                    let loot = c.choice(Opt::LootInfo);
                    if matches!(self.kind, Kind::Shulker(_)) && loot == 0 {
                        return vec![gray(Text::literal("???????"))];
                    }
                    return match loot {
                        0 => Vec::new(),
                        1 => vec![gray(tr("shulkerboxtooltip.hint.loot_table"))],
                        _ => {
                            let table = component(self.stack, "minecraft:container_loot")
                                .and_then(|t| t.get("loot_table"))
                                .map(|t| match t {
                                    horizon_world::nbt::Tag::String(s) => s.to_string_lossy(),
                                    _ => String::new(),
                                })
                                .unwrap_or_default();
                            vec![tr("shulkerboxtooltip.hint.loot_table.advanced").append(": "), gray(Text::literal(format!(" {table}")))]
                        }
                    };
                }
                if current_type(game, c, true) == PreviewType::Full {
                    return Vec::new();
                }
                vec![count_line(&self.inventory(game, c))]
            }
        }
    }
}

// ---- the tooltip's lines ------------------------------------------------------------------

thread_local! {
    /// The container screen's tooltip is being drawn: the lock key's hint
    /// shows (`setLockKeyHintsEnabled`).
    static LOCK_HINTS: Cell<bool> = const { Cell::new(false) };
    /// The last stack whose tooltip asked for lines, for `onInventoryAccessStart`.
    static PREVIOUS: RefCell<Option<ItemStack>> = const { RefCell::new(None) };
    /// An ender chest preview opened while the server syncs passively: ask it
    /// at the next frame.
    static ENDER_REQUEST: Cell<bool> = const { Cell::new(false) };
}

/// `getTooltipHints`.
fn hints(game: &Game<'_>, c: &Config, p: &Provider<'_>) -> Vec<Text> {
    if !c.on(Opt::Enable) || !p.should_display(game, c) {
        return Vec::new();
    }
    let (preview, full, lock) = keys(game, c);
    let requested = c.on(Opt::AlwaysOn) || preview;
    let gold = |t: Text| t.with_color(text::GOLD);
    let mut out = Vec::new();
    // The preview key's hint, unless the full preview is already open.
    // (Every provider here has a full preview: `isFullPreviewAvailable`.)
    if !(requested && full) {
        let full_available = true;
        let mut line = Text::literal("");
        if requested {
            line = line.append(gold(key_name(c.key(Opt::FullPreviewKey))));
            if !c.on(Opt::AlwaysOn) {
                line = line.append(gold(Text::literal("+"))).append(gold(key_name(c.key(Opt::PreviewKey))));
            }
        } else {
            line = line.append(gold(key_name(c.key(Opt::PreviewKey))));
        }
        line = line.append(gold(Text::literal(": ")));
        let swap = c.on(Opt::SwapModes);
        let hint = if current_type(game, c, full_available) == PreviewType::NoPreview {
            if swap { "shulkerboxtooltip.hint.full" } else { p.compact_hint() }
        } else if swap {
            p.compact_hint()
        } else {
            "shulkerboxtooltip.hint.full"
        };
        out.push(line.append(tr(hint).with_color(text::WHITE)));
    }
    // The lock key's hint, while a preview shows in a container screen.
    if requested && !lock && LOCK_HINTS.with(Cell::get) {
        let line = gold(key_name(c.key(Opt::LockKey))).append(gold(Text::literal(": "))).append(tr("shulkerboxtooltip.hint.lock").with_color(text::WHITE));
        out.push(line);
    }
    out
}

/// The `tooltip` hook (`ShulkerBoxTooltipClient.modifyStackTooltip`, after
/// `getTooltipLines`): the provider's lines and the key hints.
pub(super) fn tooltip(game: &Game<'_>, stack: &ItemStack, lines: &mut Vec<(Text, u32)>) {
    if !game.native_on(sbt::ID) {
        return;
    }
    let c = config();
    let Some(p) = provider(&c, stack) else { return };
    let fresh = PREVIOUS.with(|prev| {
        let mut prev = prev.borrow_mut();
        let fresh = prev.as_ref() != Some(stack);
        if fresh {
            *prev = Some(stack.clone());
        }
        fresh
    });
    if fresh && p.kind == Kind::Ender && ender_integration(game, &c) && server_values(game, &c).1 == 2 {
        ENDER_REQUEST.with(|r| r.set(true));
    }
    if !p.show_hints(game, &c) {
        return;
    }
    if c.choice(Opt::TooltipType) == 1 {
        lines.extend(p.tooltip(game, &c).into_iter().map(|t| (t, u32::MAX)));
    }
    if c.on(Opt::ShowKeyHints) {
        lines.extend(hints(game, &c, &p).into_iter().map(|t| (t, u32::MAX)));
    }
}

// ---- the preview window -------------------------------------------------------------------

/// A preview to draw (`PreviewClientTooltipComponent` with its renderer set up).
pub(super) struct Preview {
    vanilla: bool,
    kind: PreviewType,
    /// Every slot (`fullItems`).
    full: Vec<Option<ItemStack>>,
    /// The compact cells (`compactItems`): the merged stack and the slots it merges.
    compact: Vec<(ItemStack, Vec<usize>)>,
    max_size: usize,
    active: usize,
    row: usize,
    compact_row: usize,
    selected: Option<usize>,
    color: u32,
    short_counts: bool,
    position: usize,
}

/// `getProviderIfPreviewAvailable` and the renderer's `setPreview`: the
/// preview this stack's tooltip shows now, if any.
pub(super) fn preview_for(game: &Game<'_>, stack: &ItemStack) -> Option<Preview> {
    if !game.native_on(sbt::ID) {
        return None;
    }
    let c = config();
    if !c.on(Opt::Enable) || !horizon_client::item_tooltip::TooltipDisplay::for_stack(stack).name_visible(false) {
        return None;
    }
    let p = provider(&c, stack)?;
    if !p.should_display(game, &c) {
        return None;
    }
    let kind = current_type(game, &c, true);
    if kind == PreviewType::NoPreview {
        return None;
    }
    let max_size = p.max_size(game);
    let mut compact_row = p.compact_row(&c);
    if compact_row <= 0 {
        compact_row = c.max_row_size();
    }
    if compact_row <= 0 {
        compact_row = 9;
    }
    let mut row = p.max_row(&c);
    if row <= 0 {
        row = compact_row;
    }
    let full = p.inventory(game, &c);
    let merging = c.choice(Opt::Merging);
    let counts: Vec<Option<u32>> = full.iter().map(|s| s.as_ref().map(|s| s.count)).collect();
    let same = |a: usize, b: usize| match (&full[a], &full[b]) {
        (Some(x), Some(y)) => {
            if merging == 2 {
                x.same_item_same_components(y)
            } else {
                x.item == y.item
            }
        }
        _ => false,
    };
    let groups = sbt::merge(&counts, max_size, same, c.choice(Opt::Order));
    let compact = groups
        .into_iter()
        .filter_map(|g| {
            let first = full.get(*g.slots.first()?)?.as_ref()?;
            let mut merged = if merging == 0 { ItemStack::new(&first.item, 1) } else { first.clone() };
            merged.count = u32::try_from(g.count).unwrap_or(u32::MAX);
            Some((merged, g.slots))
        })
        .collect();
    let color = if c.on(Opt::ColoredPreview) { c.color(p.color_index()) } else { c.color(0) };
    Some(Preview {
        vanilla: p.vanilla_theme(&c),
        kind,
        full,
        compact,
        max_size,
        active: p.active_slots(game),
        row: row as usize,
        compact_row: compact_row as usize,
        selected: p.selected_slot(),
        color,
        short_counts: c.on(Opt::ShortCounts),
        position: c.choice(Opt::Position),
    })
}

impl Preview {
    fn compact(&self) -> bool {
        self.kind == PreviewType::Compact
    }
    fn last_non_empty(&self) -> Option<usize> {
        self.full.iter().rposition(Option::is_some)
    }
    /// The renderer's `getInvSize`.
    fn inv_size(&self) -> usize {
        if self.compact() {
            self.compact.len().max(1)
        } else if self.vanilla {
            self.last_non_empty().map_or(0, |i| i + 1)
        } else {
            self.max_size
        }
    }
    /// `getMaxRowSize`.
    fn max_row(&self) -> usize {
        let row = if self.compact() { self.compact_row } else { self.row }.max(1);
        if self.vanilla { row.min(self.inv_size()).max(1) } else { row }
    }
    fn rows(&self) -> usize {
        self.inv_size().div_ceil(self.max_row())
    }
    pub(super) fn width(&self) -> f32 {
        if self.vanilla {
            (self.max_row() * 24) as f32
        } else {
            (14 + self.max_row().min(self.inv_size()) * 18) as f32
        }
    }
    pub(super) fn height(&self) -> f32 {
        if self.vanilla { (self.rows() * 24) as f32 } else { (14 + self.rows() * 18) as f32 }
    }
    /// The component's size inside the tooltip, for the Inside position
    /// (`getWidth`, `getHeight` + 4); none for the others.
    pub(super) fn inside_size(&self) -> Option<(f32, f32)> {
        (self.position == 0).then(|| (self.width(), self.height() + 4.0))
    }
    fn slot_size(&self) -> (i32, i32) {
        if self.vanilla { (24, 0) } else { (18, 8) }
    }
    /// `getSlotAt` relative to the window's corner.
    fn slot_at(&self, x: i32, y: i32) -> Option<usize> {
        let (size, offset) = self.slot_size();
        let (x, y) = if self.vanilla { (x - 1, y - 1) } else { (x, y) };
        if x + 1 >= offset && y + 1 >= offset {
            let sx = (x + 1 - offset) / size;
            let sy = (y + 1 - offset) / size;
            if (sx as usize) < self.max_row() {
                return Some(sx as usize + sy as usize * self.max_row());
            }
        }
        None
    }
    fn stack_at(&self, x: i32, y: i32) -> Option<ItemStack> {
        let slot = self.slot_at(x, y)?;
        if self.compact() { self.compact.get(slot).map(|(s, _)| s.clone()) } else { self.full.get(slot).cloned().flatten() }
    }
    /// `findCompactSelectedSlot`.
    fn compact_selected(&self) -> Option<usize> {
        let sel = self.selected?;
        self.compact.iter().position(|(_, slots)| slots.contains(&sel))
    }
}

/// The game's integer mouse position (the screen's `mouseX`, `mouseY`).
fn mouse(game: &Game<'_>) -> (i32, i32) {
    (game.cursor.0.floor() as i32, game.cursor.1.floor() as i32)
}

/// Multiplies two packed colours channel by channel (`rgba` packs
/// little-endian: red in the low byte).
fn tint(c: [u8; 4], argb: u32) -> u32 {
    let mul = |a: u8, b: u32| (u32::from(a) * (b & 0xFF) / 255) & 0xFF;
    let r = mul(c[0], argb >> 16);
    let g = mul(c[1], argb >> 8);
    let b = mul(c[2], argb);
    let a = mul(c[3], argb >> 24);
    (a << 24) | (b << 16) | (g << 8) | r
}

/// Part `[u, v, w, h]` of the window texture at `(x, y)`, tinted.
fn window_part(game: &Game<'_>, batch: &mut HudBatch, src: [u8; 4], x: f32, y: f32, argb: u32) {
    let [u, v, w, h] = src;
    for &(rx, ry, len, c) in sprites::WINDOW {
        if ry < v || ry >= v + h {
            continue;
        }
        let start = rx.max(u);
        let end = (rx + len).min(u + w);
        if start >= end {
            continue;
        }
        let fx = x + f32::from(start - u);
        let fy = y + f32::from(ry - v);
        batch.fill(&game.gui.atlas, fx, fy, f32::from(end - start), 1.0, tint(c, argb));
    }
}

/// `blitSprite` of the nine-sliced window texture over `w` x `h`: corners as
/// they are, edges and middle tiled.
fn window(game: &Game<'_>, batch: &mut HudBatch, x: f32, y: f32, w: f32, h: f32, argb: u32) {
    let size = sprites::WINDOW_SIZE;
    let b = sprites::WINDOW_BORDER;
    let inner = size - 2 * b;
    let (w, h) = (w as i32, h as i32);
    let (bi, ii) = (i32::from(b), i32::from(inner));
    // Columns and rows: (source start, source width, destination offset).
    let spans = |total: i32| -> Vec<(u8, u8, i32)> {
        let mut v = vec![(0, b, 0)];
        let mut at = bi;
        while at < total - bi {
            let len = (total - bi - at).min(ii);
            v.push((b, len as u8, at));
            at += len;
        }
        v.push((size - b, b, total - bi));
        v
    };
    for (su, sw, dx) in spans(w) {
        for (sv, sh, dy) in spans(h) {
            window_part(game, batch, [su, sv, sw, sh], x + dx as f32, y + dy as f32, argb);
        }
    }
}

fn sprite(game: &Game<'_>, batch: &mut HudBatch, name: &str, x: f32, y: f32) {
    horizon_client::ui::blit_sprite(batch, &game.gui.atlas, name, x, y, 24.0, 24.0, u32::MAX);
}

/// `drawItem`: the item and its count (shortened in compact previews).
fn draw_item(game: &Game<'_>, batch: &mut HudBatch, stack: &ItemStack, x: f32, y: f32, short: bool) {
    let label = (stack.count != 1).then(|| if short { sbt::abbreviate(i64::from(stack.count)) } else { stack.count.to_string() });
    game.draw_item_model(batch, stack, x, y);
    game.draw_item_decorations(batch, stack, x, y, label);
}

/// The renderer's `draw`, the window's corner at `(x, y)` (the vanilla look
/// centres itself in `viewport`).
fn draw_window(game: &Game<'_>, batch: &mut HudBatch, p: &Preview, x: f32, y: f32, viewport: f32, outside: bool) -> (f32, f32) {
    let row = p.max_row();
    let (mx, my) = mouse(game);
    if p.vanilla {
        let x = x + ((viewport - p.width()) / 2.0).trunc();
        if outside {
            game.tooltip_box(batch, None, x, y, p.width(), p.height());
        }
        let hovered = p.slot_at(mx - x as i32, my - y as i32);
        let highlighted = if p.compact() { p.compact_selected().or(hovered) } else { p.selected.or(hovered) };
        let last = p.last_non_empty();
        let cell = |slot: usize| (x + (24 * (slot % row)) as f32, y + (24 * (slot / row)) as f32);
        let mut slots: Vec<(usize, Option<&ItemStack>)> = Vec::new();
        if p.compact() {
            slots.extend(p.compact.iter().enumerate().map(|(i, (s, _))| (i, Some(s))));
        } else {
            slots.extend(p.full.iter().enumerate().filter(|(i, _)| last.is_some_and(|l| *i <= l)).map(|(i, s)| (i, s.as_ref())));
        }
        for (slot, stack) in slots {
            let (sx, sy) = cell(slot);
            let hl = highlighted == Some(slot);
            sprite(game, batch, if hl { "gui/sprites/container/bundle/slot_highlight_back" } else { "gui/sprites/container/bundle/slot_background" }, sx, sy);
            if let Some(stack) = stack {
                draw_item(game, batch, stack, sx + 4.0, sy + 4.0, p.compact() && p.short_counts);
            }
            if hl {
                sprite(game, batch, "gui/sprites/container/bundle/slot_highlight_front", sx, sy);
            }
        }
        return (x, y);
    }
    // ShulkerBoxTooltip's look: the tinted window, the inactive slots, the items.
    let inv = p.inv_size();
    let columns = row.min(inv);
    let rows = inv.div_ceil(row);
    window(game, batch, x, y, (14 + columns * 18) as f32, (14 + rows * 18) as f32, 0xFF00_0000 | p.color);
    if !p.compact() {
        let display = columns * rows;
        if p.active < display {
            for slot in p.active..display {
                let left = x + 8.0 + ((slot % row) * 18) as f32;
                let top = y + 8.0 + ((slot / row) * 18) as f32;
                let color = if slot < inv { 0x4000_0000 } else { 0x7000_0000 };
                let [a, r, g, b] = u32::to_be_bytes(color);
                batch.fill(&game.gui.atlas, left, top, 16.0, 16.0, horizon_client::hud::rgba(r, g, b, a));
            }
        }
    }
    let hovered = p.slot_at(mx - x as i32, my - y as i32);
    let highlighted = if p.compact() { p.compact_selected().or(hovered) } else { p.selected.or(hovered) };
    let draw = |batch: &mut HudBatch, slot: usize, stack: Option<&ItemStack>, short: bool| {
        let sx = x + 8.0 + (18 * (slot % row)) as f32;
        let sy = y + 8.0 + (18 * (slot / row)) as f32;
        let hl = highlighted == Some(slot);
        if hl {
            horizon_client::ui::blit_sprite(batch, &game.gui.atlas, "gui/sprites/container/slot_highlight_back", sx - 4.0, sy - 4.0, 24.0, 24.0, u32::MAX);
        }
        if let Some(stack) = stack {
            draw_item(game, batch, stack, sx, sy, short);
        }
        if hl {
            horizon_client::ui::blit_sprite(batch, &game.gui.atlas, "gui/sprites/container/slot_highlight_front", sx - 4.0, sy - 4.0, 24.0, 24.0, u32::MAX);
        }
    };
    if p.compact() {
        for (slot, (stack, _)) in p.compact.iter().enumerate() {
            draw(batch, slot, Some(stack), p.short_counts);
        }
    } else {
        for (slot, stack) in p.full.iter().enumerate() {
            if slot < p.active {
                draw(batch, slot, stack.as_ref(), false);
            }
        }
    }
    (x, y)
}

/// Draws the preview of a tooltip whose box is `[tx, ty, w, h]` and whose
/// component starts at `comp_y` (`PreviewClientTooltipComponent.extractImage`
/// and the renderer's `draw`, its inner tooltip and the selected item's name).
pub(super) fn draw_preview(game: &mut Game<'_>, batch: &mut HudBatch, p: Preview, [tx, ty, w, h]: [f32; 4], comp_y: f32) {
    if p.compact.is_empty() {
        return;
    }
    let (mx, _) = mouse(game);
    let (screen_w, screen_h) = (batch.width, batch.height);
    let view_h = p.height();
    let (out_x, out_y) = if p.vanilla { (0.0, 8.0) } else { (-4.0, 4.0) };
    let mut position = p.position;
    let auto = position == 1;
    let (mut x, mut y) = (tx, comp_y);
    let (mut top_x, mut top_y, mut total_w) = (tx, ty, w);
    if position == 1 {
        position = if ty + h + view_h > screen_h { 2 } else { 3 };
    }
    if position == 2 {
        x += out_x;
        y = ty - view_h - out_y;
    } else if position == 3 {
        x += out_x;
        y = ty + h + out_y;
    }
    if (position == 2 || position == 3) && (y < 0.0 || y + view_h > screen_h) {
        let mut resolved = false;
        if auto {
            let flipped = if position == 3 { ty - view_h - out_y } else { ty + h + out_y };
            if flipped >= 0.0 && flipped + view_h <= screen_h {
                y = flipped;
                resolved = true;
            }
        }
        if !resolved {
            let width = p.width();
            let gap = 24.0;
            x = if tx + (w / 2.0).trunc() < mx as f32 { (tx + w + gap).min(screen_w - width) } else { (tx - width - gap).max(0.0) };
            y = ty + ((h - view_h) / 2.0).trunc();
            y = y.max(0.0).min(screen_h - view_h);
            total_w = width;
            top_x = x;
            top_y = y;
        }
    }
    let outside = position != 0;
    let (wx, wy) = draw_window(game, batch, &p, x, y, total_w, outside);
    // `drawInnerTooltip`: the item under the pointer inside a locked tooltip.
    let (mx, my) = mouse(game);
    if let Some(inner) = p.stack_at(mx - wx as i32, my - wy as i32) {
        let (cx, cy) = game.cursor;
        game.draw_stack_tooltip(batch, &inner, cx, cy);
    }
    // `drawSelectedItemTooltip`: a bundle's selected item's name above it.
    if let Some(ci) = p.compact_selected()
        && let Some(sel) = p.selected.and_then(|s| p.full.get(s).cloned().flatten()).or_else(|| p.compact.get(ci).map(|(s, _)| s.clone()))
    {
        let name = game.stack_tooltip(&sel).into_iter().take(1).collect::<Vec<_>>();
        if !name.is_empty() {
            let lines: Vec<(Text, u32)> = name.into_iter().map(|(t, c)| (t.resolve_legacy(), c)).collect();
            let atlas = &game.gui.atlas;
            let tw = atlas.font.width_text(&lines[0].0);
            let center = top_x + (total_w / 2.0).trunc() - 12.0;
            let (nx, ny, nw, nh) = Game::tooltip_geometry(atlas, &lines, (center - (tw / 2.0).trunc(), top_y - 3.0), (screen_w, screen_h), [0.0; 2]);
            game.tooltip_box(batch, Some(&sel), nx, ny, nw, nh);
            Game::tooltip_text(batch, &game.gui.atlas, &lines, nx, ny);
        }
    }
}

// ---- locking the tooltip ------------------------------------------------------------------

thread_local! {
    /// The locked slot and where its tooltip stays (`mouseLockSlot`, `mouseLockX/Y`).
    static LOCK: Cell<Option<(usize, (f32, f32))>> = const { Cell::new(None) };
}

/// The locked slot: `isHovering` is true for it alone while it lasts.
pub(super) fn locked_slot() -> Option<usize> {
    LOCK.with(Cell::get).map(|(s, _)| s)
}

/// `AbstractContainerScreenMixin`: the container screen's tooltip. While the
/// lock key is held over a slot whose item shows a preview, the tooltip stays
/// where it was, for that slot, and the pointer can move over its preview.
/// True when it drew the tooltip.
pub(super) fn slot_tooltip(game: &mut Game<'_>, batch: &mut HudBatch) -> bool {
    LOCK_HINTS.with(|h| h.set(true));
    if !game.native_on(sbt::ID) {
        LOCK.with(|l| l.set(None));
        return false;
    }
    let c = config();
    let (_, _, lock) = keys(game, &c);
    if !lock {
        LOCK.with(|l| l.set(None));
        return false;
    }
    if LOCK.with(Cell::get).is_none()
        && let Some(slot) = game.slot_at(batch, game.cursor.0, game.cursor.1)
    {
        LOCK.with(|l| l.set(Some((slot, game.cursor))));
    }
    let Some((slot, (x, y))) = LOCK.with(Cell::get) else { return false };
    let stack = game.slot_view(slot);
    let Some(stack) = stack.filter(|_| game.carried.is_none()).filter(|s| preview_for(game, s).is_some()) else {
        LOCK.with(|l| l.set(None));
        return false;
    };
    game.draw_stack_tooltip(batch, &stack, x, y);
    LOCK_HINTS.with(|h| h.set(false));
    true
}

/// The container screen's tooltip is drawn: the lock hint goes.
pub(super) fn slot_tooltip_done() {
    LOCK_HINTS.with(|h| h.set(false));
}

// ---- the bundle wheel ---------------------------------------------------------------------

/// `BundleMouseActionsMixin`: while a bundle's preview shows, the wheel goes
/// through all its items (no limit), in the compact preview's order when
/// that one shows. `(count, order)`: the items, and the real index of each
/// compact position.
pub(super) fn bundle_wheel(game: &Game<'_>, bundle: &ItemStack) -> Option<(usize, Option<Vec<usize>>)> {
    let p = preview_for(game, bundle)?;
    let count = bundle.data().map_or(0, |d| d.bundle_contents.len());
    if !p.compact() {
        return Some((count, None));
    }
    let mut order = Vec::new();
    for (_, slots) in &p.compact {
        let mut slots: Vec<usize> = slots.iter().copied().filter(|s| *s < count).collect();
        slots.sort_unstable();
        order.extend(slots);
    }
    Some((count, Some(order)))
}

// ---- the ender chest ----------------------------------------------------------------------

/// The client's copy of its ender chest (`EnderChestCache`): filled when an
/// ender chest is open, kept in the world's folder (single player) or in
/// `config/` by server address.
#[derive(Default)]
struct Cache {
    /// The world or server the cache belongs to.
    place: Option<String>,
    items: Option<Vec<Option<ItemStack>>>,
}

/// The mod's messages with the server.
#[derive(Default)]
struct Net {
    /// The handshake went out to this connection's server.
    sent: bool,
    /// The server's `clientIntegration` and `enderChestSyncType`, once it answered.
    server: Option<(bool, usize)>,
    /// The ender chest's contents the server last sent.
    ender: Option<Vec<Option<ItemStack>>>,
}

/// The host's side: the players whose ender chest it keeps in sync.
#[derive(Default)]
struct Host {
    /// Player, what it last sent them and the tick of that.
    players: Vec<(u128, String, u64)>,
    tick: u64,
}

thread_local! {
    static CACHE: RefCell<Cache> = RefCell::new(Cache::default());
    static NET: RefCell<Net> = RefCell::new(Net::default());
    static HOST: RefCell<Host> = RefCell::new(Host::default());
}

const CH_HANDSHAKE: &str = "shulkerboxtooltip:handshake";
const CH_CONFIG: &str = "shulkerboxtooltip:config";
const CH_ENDER: &str = "shulkerboxtooltip:ender_chest";
const CH_REQUEST: &str = "shulkerboxtooltip:ender_chest_request";
/// `ProtocolVersion`.
const PROTOCOL: u8 = 1;

/// Slots as text: one `slot<TAB>stack` line per item (the stack as
/// `ItemStack.CODEC` writes it, in SNBT).
fn encode(items: &[Option<ItemStack>]) -> String {
    let mut out = String::new();
    for (i, s) in items.iter().enumerate() {
        if let Some(s) = s {
            let mut line = String::new();
            snbt(&horizon_items::item_nbt::item_to_nbt(s), &mut line);
            out.push_str(&format!("{i}\t{line}\n"));
        }
    }
    out
}

/// A tag as SNBT that `parse_snbt` reads back: every key and string quoted.
fn snbt(tag: &horizon_world::nbt::Tag, out: &mut String) {
    use horizon_world::nbt::Tag;
    use std::fmt::Write as _;
    let quote = |s: &str, out: &mut String| {
        out.push('"');
        for c in s.chars() {
            match c {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                c => out.push(c),
            }
        }
        out.push('"');
    };
    let list = |prefix: &str, items: Vec<String>, out: &mut String| {
        out.push_str(prefix);
        out.push_str(&items.join(","));
        out.push(']');
    };
    match tag {
        Tag::End => {}
        Tag::Byte(v) => {
            let _ = write!(out, "{v}b");
        }
        Tag::Short(v) => {
            let _ = write!(out, "{v}s");
        }
        Tag::Int(v) => {
            let _ = write!(out, "{v}");
        }
        Tag::Long(v) => {
            let _ = write!(out, "{v}L");
        }
        Tag::Float(v) => {
            let _ = write!(out, "{:?}f", f32::from_bits(*v));
        }
        Tag::Double(v) => {
            let _ = write!(out, "{:?}d", f64::from_bits(*v));
        }
        Tag::String(s) => quote(&s.to_string_lossy(), out),
        Tag::ByteArray(v) => list("[B;", v.iter().map(|b| format!("{}b", *b as i8)).collect(), out),
        Tag::IntArray(v) => list("[I;", v.iter().map(i32::to_string).collect(), out),
        Tag::LongArray(v) => list("[L;", v.iter().map(|l| format!("{l}L")).collect(), out),
        Tag::List(items) => {
            out.push('[');
            for (i, t) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                snbt(t, out);
            }
            out.push(']');
        }
        Tag::Compound(map) => {
            out.push('{');
            for (i, (k, v)) in map.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                quote(&k.to_string_lossy(), out);
                out.push(':');
                snbt(v, out);
            }
            out.push('}');
        }
    }
}

fn decode(text: &str) -> Vec<Option<ItemStack>> {
    let mut items: Vec<Option<ItemStack>> = vec![None; sbt::ENDER_SLOTS];
    for line in text.lines() {
        let Some((slot, snbt)) = line.split_once('\t') else { continue };
        // A slot outside the ender chest is ignored (`sbt::ender_slot`).
        let (Some(slot), Ok(Some(stack))) = (sbt::ender_slot(slot), horizon_client::item_data::parse_stack(snbt)) else { continue };
        items[slot] = Some(stack);
    }
    items
}

/// The world or server this window plays on, and the cache's file for it.
fn cache_place(game: &Game<'_>) -> Option<(String, std::path::PathBuf, Option<String>)> {
    if game.player.is_none() {
        return None;
    }
    // A world of this window's own: its folder (`saves/<folder>` when its
    // server runs beside the window); a server: its address.
    let world = match game.remote.as_ref() {
        Some(r) => match r.save_folder.as_deref() {
            Some(folder) => game.native_instance_dir().join("saves").join(folder),
            None => {
                let address = r.session.address.clone();
                let file = Game::native_config_dir().ok()?.join("shulkerboxtooltip_mp_ender_chest_cache.txt");
                return Some((format!("server:{address}"), file, Some(address)));
            }
        },
        None => game.world_root.clone(),
    };
    if world.as_os_str().is_empty() {
        return None;
    }
    Some((format!("world:{}", world.display()), world.join("data").join("shulkerboxtooltip_ender_chest_cache.txt"), None))
}

/// `loadFromDisk`: the cache of the world or server now played on.
fn load_cache(file: &std::path::Path, server: Option<&str>) -> Option<Vec<Option<ItemStack>>> {
    let text = std::fs::read_to_string(file).ok()?;
    match server {
        None => Some(decode(&text)),
        Some(address) => {
            let mut section = String::new();
            let mut found = false;
            let mut inside = false;
            for line in text.lines() {
                if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                    inside = name == address;
                    found |= inside;
                } else if inside {
                    section.push_str(line);
                    section.push('\n');
                }
            }
            found.then(|| decode(&section))
        }
    }
}

/// `putItems`: the cache, in memory and on disk.
fn save_cache(file: &std::path::Path, server: Option<&str>, items: &[Option<ItemStack>]) {
    let text = match server {
        None => encode(items),
        Some(address) => {
            let old = std::fs::read_to_string(file).unwrap_or_default();
            let mut out = String::new();
            let mut skip = false;
            for line in old.lines() {
                if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                    skip = name == address;
                }
                if !skip {
                    out.push_str(line);
                    out.push('\n');
                }
            }
            out.push_str(&format!("[{address}]\n{}", encode(items)));
            out
        }
    };
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(file, text);
}

/// The open ender chest's slots, if one is open (`EnderChestScreenDetector`:
/// a 9x3 chest menu titled "Ender Chest").
fn open_ender_chest(game: &mut Game<'_>) -> Option<Vec<Option<ItemStack>>> {
    if game.screen != Screen::Inventory {
        return None;
    }
    let ender = if game.remote.is_none() {
        game.ender_open.is_some()
    } else {
        let title = game.remote.as_ref().and_then(|r| r.menu_title.as_ref()).map(Text::plain);
        game.remote_menu_type() == Some("minecraft:generic_9x3") && title.is_some_and(|t| t == Text::translate("container.enderchest", &[]).plain())
    };
    if !ender {
        return None;
    }
    Some((0..27).map(|i| game.slot_view(300 + i)).collect())
}

/// Once a frame on the client: the cache follows the world, and an open
/// ender chest fills it; the handshake, and a passive server's request.
fn ender_frame(game: &mut Game<'_>, c: &Config) {
    let place = cache_place(game);
    let key = place.as_ref().map(|p| p.0.clone());
    if CACHE.with(|cache| cache.borrow().place != key) {
        let items = place.as_ref().and_then(|(_, file, server)| load_cache(file, server.as_deref()));
        CACHE.with(|cache| *cache.borrow_mut() = Cache { place: key, items });
        NET.with(|n| *n.borrow_mut() = Net::default());
    }
    if let Some((_, file, server)) = place
        && let Some(items) = open_ender_chest(game)
        && CACHE.with(|cache| cache.borrow().items.as_ref() != Some(&items))
    {
        save_cache(&file, server.as_deref(), &items);
        CACHE.with(|cache| cache.borrow_mut().items = Some(items));
    }
    // On a server: say hello once (`C2SMessages.attemptHandshake`).
    if game.remote.is_some() && game.player.is_some() && c.on(Opt::ServerIntegration) && !NET.with(|n| n.borrow().sent) {
        match game.native_net_send_to_server(CH_HANDSHAKE, &[PROTOCOL]) {
            Ok(()) => NET.with(|n| n.borrow_mut().sent = true),
            Err(e) if e.contains("carries no mod messages") && game.remote.as_ref().is_some_and(|r| r.joined) => {
                NET.with(|n| n.borrow_mut().sent = true);
            }
            Err(_) => {}
        }
    }
    if ENDER_REQUEST.with(|r| r.replace(false)) && game.remote.is_some() {
        let _ = game.native_net_send_to_server(CH_REQUEST, &[]);
    }
}

/// A message from the server.
pub(super) fn from_server(channel: &str, data: &[u8]) {
    match channel {
        CH_CONFIG if data.len() >= 2 => NET.with(|n| n.borrow_mut().server = Some((data[0] != 0, usize::from(data[1]).min(2)))),
        CH_ENDER => {
            let items = decode(&String::from_utf8_lossy(data));
            NET.with(|n| n.borrow_mut().ender = Some(items));
        }
        _ => {}
    }
}

/// The host's player `uuid`'s ender chest, as text.
fn ender_text(game: &Game<'_>, uuid: u128) -> Option<String> {
    let inv = game.native_inventory(uuid, super::NativeInventoryKind::Ender).ok()?;
    Some(encode(&inv.slots))
}

/// A message from a player (the host's side, `ServerNetworking`).
pub(super) fn from_client(game: &mut Game<'_>, player: u128, channel: &str, data: &[u8]) {
    if !game.native_on(sbt::ID) {
        return;
    }
    let c = config();
    let integration = c.on(Opt::ClientIntegration);
    let sync = c.choice(Opt::EnderSync);
    match channel {
        CH_HANDSHAKE if data.first() == Some(&PROTOCOL) => {
            let _ = game.native_net_send_to_player(player, CH_CONFIG, &[u8::from(integration), sync as u8]);
            if integration && sync != 0 {
                // What the player's ender chest holds now; while they are
                // still logging in, the first tick that can read it sends it.
                let text = ender_text(game, player);
                if let Some(text) = &text {
                    let _ = game.native_net_send_to_player(player, CH_ENDER, text.as_bytes());
                }
                let tick = HOST.with(|h| h.borrow().tick);
                HOST.with(|h| {
                    let mut h = h.borrow_mut();
                    h.players.retain(|(p, _, _)| *p != player);
                    // An unread chest waits no second for its first sending.
                    let at = if text.is_some() { tick } else { tick.saturating_sub(20) };
                    h.players.push((player, text.unwrap_or_else(|| "\u{0}".into()), at));
                });
            }
        }
        CH_REQUEST if integration && sync == 2 => {
            if let Some(text) = ender_text(game, player) {
                let _ = game.native_net_send_to_player(player, CH_ENDER, text.as_bytes());
            }
        }
        _ => {}
    }
}

/// Each server tick on the host: an active sync sends a player's ender chest
/// when it changed, at most once a second (`EnderChestListener`).
pub(super) fn server_tick(game: &mut Game<'_>) {
    let tick = HOST.with(|h| {
        let mut h = h.borrow_mut();
        h.tick += 1;
        h.tick
    });
    let c = config();
    if !game.native_on(sbt::ID) || !c.on(Opt::ClientIntegration) || c.choice(Opt::EnderSync) != 1 {
        return;
    }
    let players: Vec<(u128, String, u64)> = HOST.with(|h| h.borrow().players.clone());
    for (player, last, at) in players {
        if tick < at + 20 {
            continue;
        }
        let Some(text) = ender_text(game, player) else { continue };
        if text != last && game.native_net_send_to_player(player, CH_ENDER, text.as_bytes()).is_ok() {
            HOST.with(|h| {
                if let Some(e) = h.borrow_mut().players.iter_mut().find(|(p, _, _)| *p == player) {
                    *e = (player, text, tick);
                }
            });
        }
    }
}

// ---- the mod's row and page ---------------------------------------------------------------

pub(super) fn description(ru: bool) -> &'static str {
    if ru { "Что в моём шалкеровом ящике?" } else { "What's in my shulker box?" }
}

/// The mod's row in Options > Mods.
pub(super) fn list(mods: &mut Vec<NativeMod>) {
    mods.push(NativeMod {
        id: sbt::ID.into(),
        title: format!("{} {}", sbt::NAME, sbt::VERSION),
        description: description(russian()).into(),
        keys: Vec::new(),
    });
}

/// The mod's page: Configure... opens the config screen.
pub(super) fn page(mod_id: &str, labels: &mut Vec<String>) {
    if mod_id == sbt::ID {
        labels.push(if russian() { "Настройки...".into() } else { "Configure...".into() });
    }
}

/// A click on the page, or Mod Menu's Configure: the config screen opens.
pub(super) fn page_click(game: &Game<'_>, mod_id: &str, button: usize) {
    if mod_id == sbt::ID && (button == 0 || button == super::shulkerboxtooltip_screen::MODMENU_CONFIGURE) {
        super::shulkerboxtooltip_screen::open(game, button != 0);
    }
}

/// Once a frame: the library's flags follow the configuration, the ender
/// chest's cache and messages, and the lock ends off a container screen.
pub(super) fn frame(game: &mut Game<'_>) {
    let c = config();
    let on = game.native_on(sbt::ID);
    use horizon_client::item_tooltip::shulkerboxtooltip_flags as flags;
    flags::HIDE_CONTAINER_LINES.store(on && c.choice(Opt::TooltipType) != 0, Ordering::Relaxed);
    flags::HIDE_SHULKER_LORE.store(on && c.on(Opt::HideLore), Ordering::Relaxed);
    if game.screen != Screen::Inventory {
        LOCK.with(|l| l.set(None));
    }
    if on {
        ender_frame(game, &c);
    }
}
