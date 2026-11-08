// Roughly Enough Items for Horizon: the game's side. The game's items and
// recipes read into `roughlyenoughitems`'s model (the default plugin's
// displays), the player's mouse and keys on container screens, and what they
// change in the game (giving items, moving a recipe into the crafting grid,
// the favorites' commands). `roughlyenoughitems_view` draws it.
//
// The screen hooks only read the game (`&Game`): what needs to change it waits
// in `State::pending` for the next frame. Hooks the game fires while this mod
// is drawing (item tooltips) read only the `Cell`s below, never `STATE`.
use super::roughlyenoughitems::{
    self as rei, Category, Favorites, Index, Lookup, Query, Recipe, Settings, Shortcut, Stack,
};
use super::roughlyenoughitems_config::{self as config, ConfigScreen};
use super::{Game, Screen};
use horizon_client::clipboard::Clipboard as _;
use horizon_client::items::ItemStack;
use horizon_client::mod_api::NativeScreenView;
use horizon_client::ui::EditBox;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

/// GLFW's modifier bits.
const SHIFT: u32 = 1;
const CONTROL: u32 = 2;
const ALT: u32 = 4;
/// The search field's most characters (REI's 10000).
const SEARCH_MAX: usize = 10_000;
/// The pointer moves this far with a button held before an entry drags.
const DRAG_START: f32 = 4.0;

/// REI's collapsible groups (`DefaultClientPlugin.registerCollapsibleEntries`):
/// item id test and name key.
pub(super) const GROUPS: [(&str, &str); 7] = [
    ("minecraft:enchanted_book", "item.minecraft.enchanted_book"),
    ("minecraft:potion", "item.minecraft.potion"),
    ("minecraft:splash_potion", "item.minecraft.splash_potion"),
    ("minecraft:lingering_potion", "item.minecraft.lingering_potion"),
    ("_spawn_egg", "spawn_egg"),
    ("minecraft:tipped_arrow", "item.minecraft.tipped_arrow"),
    ("minecraft:music_disc_", "music_disc"),
];

fn group_of(item: &str) -> Option<u8> {
    GROUPS.iter().position(|(test, _)| match *test {
        "_spawn_egg" => item.ends_with("_spawn_egg"),
        "minecraft:music_disc_" => item.starts_with("minecraft:music_disc_"),
        id => item == id,
    }).map(|i| i as u8)
}

/// One entry: an item with its components (count and damage aside).
pub(super) struct Ingredient {
    pub(super) stack: ItemStack,
    /// The item as `/give` writes it: `minecraft:potion[...]`.
    pub(super) key: String,
    namespace: String,
    name: String,
    lower_name: String,
    path: String,
    lower_mod: String,
    tags: Vec<String>,
    pub(super) group: Option<u8>,
}

impl Ingredient {
    pub(super) fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }
}

/// The game's items and recipes as REI shows them, made once a world (again
/// when the recipes or the language change).
pub(super) struct Data {
    language: String,
    recipes_at: usize,
    pub(super) ingredients: Vec<Ingredient>,
    by_key: HashMap<String, u32>,
    /// The entry list's order: the creative inventory's search tab, then the
    /// items no tab shows.
    pub(super) list: Vec<u32>,
    pub(super) index: Index,
    /// Each entry's whole tooltip, lowercased; made the first time a search
    /// reads tooltips.
    tooltips: Option<Vec<String>>,
    /// Each collapsible group's name.
    pub(super) group_names: Vec<String>,
    /// Each category's tab icon.
    pub(super) icons: HashMap<Category, u32>,
    /// The blaze powder brewing shows.
    pub(super) blaze_powder: Option<u32>,
    /// Every item and block tag's tree, for the tag category.
    pub(super) tags: Vec<rei::TagTree>,
    /// The item tags come first in `tags`: this many.
    pub(super) item_tags: usize,
}

impl Data {
    pub(super) fn ingredient(&self, id: u32) -> Option<&Ingredient> {
        self.ingredients.get(id as usize)
    }
    pub(super) fn id_of(&self, stack: &ItemStack) -> Option<u32> {
        self.by_key.get(&stack_key(stack)).copied()
    }
    pub(super) fn ingredient_by_key(&self, key: &str) -> Option<u32> {
        self.by_key.get(key).copied()
    }
    /// A display's entry as the game draws it.
    pub(super) fn stack(&self, s: Stack) -> Option<ItemStack> {
        if s.is_empty() {
            return None;
        }
        let mut stack = self.ingredient(s.id)?.stack.clone();
        stack.count = s.count.max(1);
        stack.damage = s.damage;
        Some(stack)
    }
}

/// What a part of the overlay is, for the clicks and keys over it.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Hit {
    /// The screen's own recipe arrow (`accessibility.clickable_recipe_arrows`).
    ClickArea(Category),
    /// An entry of the list; a collapsed group; the favorite at an index.
    Entry(u32),
    Collapsed(u8),
    Favorite(usize),
    /// A display's slot or a workstation.
    Slot(u32),
    Workstation(u32),
    /// A shortcut of the favorites panel.
    Shortcut(Shortcut),
    AddButton,
    ListPage(i32),
    ListLabel,
    Search,
    Craftable,
    Config,
    Menu(MenuItem),
    /// The recipe screen: a tab, the tab arrows, the category title and its
    /// arrows, the page arrows and label, a display's "+" button.
    Tab(usize),
    TabPage(i32),
    Title,
    CategoryTurn(i32),
    RecipePage(i32),
    PageLabel,
    Transfer(usize),
    /// A tag display's tree view, its expand and its copy buttons.
    TagView(usize),
    TagExpand(usize),
    TagCopy(usize),
    /// A display's "+" button that can't move it (A saves it all the same).
    TransferOff(usize),
    /// The display history strip: the recipe shown, its "+" button.
    History(usize),
    HistoryTransfer(usize),
    /// The choose-page popup: its panels, its number field, its Done button.
    ChooseArea,
    ChooseField,
    ChooseDone,
    /// Somewhere else on the overlay: the click goes nowhere.
    Area,
}

/// The hover menus' entries (`ConfigButtonWidget`, `CraftableFilterButtonWidget`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum MenuItem {
    Cheating,
    DarkTheme,
    ReducedMotion,
    LeftPanel,
    ScrollingPanel,
    /// Search Field Position: CENTER, BOTTOM_SIDE, TOP_SIDE.
    SearchPosition(u8),
    CraftableFilter,
    Syntax,
    HideIdle,
    MoreOptions,
}

/// Which hover menu is open.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Menu {
    Config,
    Craftable,
}

/// What changes the game, done at the next frame.
#[derive(Clone, Debug)]
pub(super) enum Action {
    Give { id: u32, count: u32 },
    Transfer { recipe: usize, all: bool },
    /// Ctrl+click on an entry: its first recipe that can be made, moved in.
    QuickCraft { id: u32, all: bool },
    /// R, U or A over a slot of the screen's.
    SlotKey { key: SlotKey, at: (f32, f32) },
    Command(String),
    /// A tag tree to the clipboard.
    CopyText(String),
    Paste,
    Copy,
    /// The config screen's Done: a keybind's key (the game keeps mod keys).
    SetKey { name: String, key: String },
    /// "Reload Plugins" (the recipes read again) and "Reload Search".
    Reload { search_only: bool },
    /// Ctrl+V in the config screen's search field.
    ConfigPaste,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum SlotKey {
    Recipes,
    Usages,
    Favorite,
}

/// A press on the overlay, until its release.
#[derive(Clone, Copy, Debug)]
pub(super) struct Press {
    pub(super) hit: Hit,
    button: i32,
    pub(super) at: (f32, f32),
    shift: bool,
    control: bool,
    alt: bool,
    /// The entry follows the pointer.
    pub(super) dragging: bool,
}

/// The choose-page popup (`DefaultDisplayChoosePageWidget`): which pages,
/// how many, and the number typed.
pub(super) struct ChoosePage {
    /// The entry list's pages, or the recipe screen's.
    pub(super) list: bool,
    pub(super) pages: usize,
    pub(super) field: EditBox,
}

/// A tag display's tree view (`OverflowWidget`) and its expanded state.
#[derive(Clone, Copy, Debug)]
pub(super) struct TagView {
    /// Grown to most of the screen, and when that last changed.
    pub(super) expanded: bool,
    pub(super) toggled: Option<Instant>,
    /// REI's scale: the tree is drawn `1 / scale` times.
    pub(super) scale: f32,
    /// The tree's corner in the view, and the scale it was drawn at.
    pub(super) offset: (f32, f32),
    pub(super) drawn: f32,
    pub(super) placed: bool,
}

impl Default for TagView {
    fn default() -> Self {
        TagView { expanded: false, toggled: None, scale: 1.0, offset: (0.0, 0.0), drawn: 1.0, placed: false }
    }
}

/// What the open crafting grid and the inventory hold, refreshed a few times
/// a second (REI's craftable filter reads it every five ticks).
#[derive(Default)]
pub(super) struct Inventory {
    have: HashMap<String, u32>,
    max_stack: HashMap<String, u32>,
    /// A crafting grid is open, and its size.
    crafting: bool,
    size: usize,
    /// The entries some crafting recipe makes from what is here.
    pub(super) craftable: HashSet<u32>,
}

/// Whether the "+" button can move a display in (`AutoCraftingEvaluator`).
#[derive(Clone, PartialEq, Debug)]
pub(super) enum Status {
    /// "!": no handler for this display and screen.
    NotSupported,
    /// A 3x3 recipe and the inventory's 2x2 grid.
    TooSmall,
    /// The input slots nothing here fills.
    Missing(Vec<usize>),
    Ok,
}

#[derive(Default)]
pub(super) struct State {
    pub(super) data: Option<Data>,
    pub(super) settings: Option<Settings>,
    pub(super) favorites: Option<Favorites>,
    pub(super) search: Option<EditBox>,
    /// The first character the search field shows.
    pub(super) search_scroll: usize,
    history: Vec<String>,
    history_at: Option<usize>,
    /// The list as the search leaves it; None when it needs working out.
    filtered: Option<Vec<u32>>,
    filtered_for: (String, bool, Option<Instant>),
    pub(super) list_page: usize,
    /// The cells of a page of the list the last frame.
    pub(super) list_per: usize,
    pub(super) list_len: usize,
    /// How far the scrolling entry list is scrolled.
    pub(super) list_scroll: f32,
    /// The entry under the pointer and since when (REI waits 200 ms before
    /// looking for a recipe to preview).
    pub(super) hover_since: Option<(u32, Instant)>,
    /// Only what can be crafted now (not saved, as in REI).
    pub(super) craftable: bool,
    /// The collapsible groups shown expanded.
    pub(super) expanded: [bool; GROUPS.len()],
    pub(super) favorites_scroll: f32,
    /// The favorites panel ("Add...") and when it last opened or closed.
    pub(super) panel: bool,
    pub(super) panel_at: Option<Instant>,
    /// The panel's rows scrolled, and where they show.
    pub(super) panel_scroll: f32,
    pub(super) panel_area: Option<rei::Rect>,
    /// Where the favorites go: their region, the grid's corner and columns.
    pub(super) favorites_grid: Option<(rei::Rect, f32, f32, usize)>,
    pub(super) menu: Option<Menu>,
    /// The open menus (the hover menu, its submenu) the last frame, by depth.
    pub(super) menu_rects: Vec<(usize, rei::Rect)>,
    /// Each menu level's focused row (the one last under the pointer).
    pub(super) menu_focus: [Option<usize>; 3],
    /// When the menu and its submenu (with its row) opened, for their growth.
    pub(super) menu_at: Option<Instant>,
    pub(super) sub_at: Option<(usize, Instant)>,
    /// The display history (`DisplayHistoryManager`): recipes put aside with
    /// A over their "+", newest first, nine at most; the one shown.
    pub(super) display_history: Vec<usize>,
    pub(super) history_page: usize,
    /// Where the strip was the last frame (the wheel pages it).
    pub(super) history_area: Option<rei::Rect>,
    /// The choose-page popup (Shift+click on "1/N").
    pub(super) choose: Option<ChoosePage>,
    /// REI's config screen, over everything while it is open.
    pub(super) config: Option<ConfigScreen>,
    /// The last Ctrl+wheel zoom, for its percentage.
    pub(super) zoom_at: Option<Instant>,
    /// The favorites' "+" button's fade.
    pub(super) add_alpha: f32,
    pub(super) recipes: rei::Screen,
    /// The displays shown a page the last frame, for the page keys.
    pub(super) per_page: usize,
    /// The entry list's region the last frame (the wheel turns its pages).
    pub(super) list_area: Option<rei::Rect>,
    /// The tag displays' views on the page shown, and the page they are for.
    pub(super) tag_views: HashMap<usize, TagView>,
    pub(super) tag_views_for: Option<(Lookup, usize, usize)>,
    /// Where the pointer was the last frame, while a tag view is dragged.
    pub(super) pan_last: (f32, f32),
    /// The parts drawn the last frame, for the mouse.
    pub(super) hits: Vec<(rei::Rect, Hit)>,
    pub(super) press: Option<Press>,
    pub(super) pending: Vec<Action>,
    pub(super) started: Option<Instant>,
    pub(super) inventory: Option<(Instant, Inventory)>,
    /// Buttons whose press the overlay took, so their release is taken too.
    pressed: [bool; 8],
    /// A line shown over the overlay for a moment (a move that failed).
    pub(super) note: Option<(String, Instant)>,
    /// The language the labels are in.
    pub(super) ru: bool,
    /// The search text the list's page was last kept for.
    paged_for: String,
}

impl State {
    /// True once each time the search text changes (the list goes back to
    /// its first page).
    pub(super) fn search_changed(&mut self) -> bool {
        let text = self.search.as_ref().map_or("", |s| s.value.as_str());
        if text == self.paged_for {
            return false;
        }
        self.paged_for = text.to_owned();
        true
    }
    pub(super) fn search_text(&self) -> &str {
        self.search.as_ref().map_or("", |s| s.value.as_str())
    }
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
    /// The mod name in tooltips; read by the tooltip hook, which the game
    /// fires while this mod draws.
    static MOD_NAME: Cell<bool> = const { Cell::new(true) };
    /// This mod is drawing a tooltip of its own.
    pub(super) static OWN_TOOLTIP: Cell<bool> = const { Cell::new(false) };
    /// The recipe screen is open: the game's own tooltips under it stay hidden.
    pub(super) static COVERED: Cell<bool> = const { Cell::new(false) };
}

/// Runs `f` on the state; None while it is in use (a hook fired from inside
/// this mod's own call into the game).
pub(super) fn with<R>(f: impl FnOnce(&mut State) -> R) -> Option<R> {
    STATE.with(|s| s.try_borrow_mut().ok().map(|mut s| f(&mut s)))
}

/// On, and not in Just Enough Items's way: both draw in the same place, so
/// REI steps aside while JEI is on.
pub(super) fn on(game: &Game<'_>) -> bool {
    game.native_on(rei::ID) && !jei_running(game)
}

/// Just Enough Items is built into this game and switched on. A mod that is
/// installed but not built has no line in `horizon-mods.txt` and so counts as
/// "on" for `native_on`: only a built one (`native` set) can be in the way.
fn jei_running(game: &Game<'_>) -> bool {
    game.native_on("justenoughitems") && game.settings.mods.installed.iter().any(|m| m.id == "justenoughitems" && m.native.is_some())
}

/// The item as `/give` writes it, damage and count aside.
pub(super) fn stack_key(stack: &ItemStack) -> String {
    let mut plain = stack.clone();
    plain.count = 1;
    plain.damage = 0;
    format!("{}{}", plain.item, super::native_recipes::components(&plain))
}

/// The stack `/give` syntax names: `minecraft:potion[...]`.
fn stack_of(key: &str) -> Option<ItemStack> {
    let (item, components) = match key.find('[') {
        Some(i) => (&key[..i], &key[i..]),
        None => (key, ""),
    };
    if item.is_empty() {
        return None;
    }
    let (damage, extra) = if components.is_empty() {
        (0, None)
    } else {
        horizon_client::item_data::command_components(components).map_or((0, None), |tag| {
            horizon_client::item_data::decode_components(Some(&tag))
        })
    };
    Some(ItemStack {
        item: item.into(),
        count: 1,
        damage,
        extra,
    })
}

// ---- settings and favorites --------------------------------------------------------

pub(super) fn settings(game: &Game<'_>, state: &mut State) -> Settings {
    if state.settings.is_none() {
        let s = Settings::read(|k| game.native_setting(rei::ID, k));
        MOD_NAME.with(|c| c.set(s.mod_name));
        state.settings = Some(s);
    }
    state.settings.unwrap_or_default()
}

/// One of the menu's switches, kept as the option it stands for.
pub(super) fn set_setting(game: &Game<'_>, state: &mut State, key: &str, on: bool) {
    let (id, token) = Settings::token(key, on);
    set_option(game, state, id, token);
}

/// An option kept by its REI id (`appearance.theme`) and value.
pub(super) fn set_option(game: &Game<'_>, state: &mut State, id: &str, token: &str) {
    game.native_set_setting(rei::ID, id, token);
    reload_settings(game, state);
}

/// The settings read again (after the config screen's Done).
pub(super) fn reload_settings(game: &Game<'_>, state: &mut State) {
    state.settings = None;
    let _ = settings(game, state);
    state.filtered = None;
}

/// A keybind's modifiers ("input.hide" keeps "ctrl").
pub(super) fn key_mods(game: &Game<'_>, name: &str) -> u8 {
    let id = format!("input.{name}");
    let value = game.native_setting(rei::ID, &id).unwrap_or_else(|| rei::option_default(&id).to_owned());
    config::parse_mods(&value)
}

/// A plain stack of an item, for the config screen's previews.
pub(super) fn plain_stack(item: &str) -> Option<ItemStack> {
    stack_of(item)
}

fn favorites_file() -> Option<std::path::PathBuf> {
    Some(Game::native_config_dir().ok()?.join(rei::ID).join("favorites.txt"))
}

pub(super) fn favorites(state: &mut State) -> &mut Favorites {
    state.favorites.get_or_insert_with(|| {
        favorites_file()
            .and_then(|f| std::fs::read_to_string(f).ok())
            .map(|t| Favorites::parse(&t))
            .unwrap_or_default()
    })
}

fn save_favorites(state: &mut State) {
    let text = favorites(state).write();
    if let Some(file) = favorites_file() {
        if let Some(dir) = file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(file, text);
    }
}

/// A key the favorites keep for entry `id`.
fn key_of(state: &State, id: u32) -> Option<String> {
    state.data.as_ref()?.ingredient(id).map(|i| i.key.clone())
}

fn add_favorite(state: &mut State, key: String, at: Option<usize>) {
    favorites(state).add(&key, at);
    save_favorites(state);
}

fn remove_favorite(state: &mut State, key: &str) {
    if favorites(state).remove(key) {
        save_favorites(state);
    }
}

// ---- the game's items and recipes ---------------------------------------------------

/// Makes the model once a world is open, and again when the game's recipe
/// tables or the language change; drops it outside a world.
fn sync(game: &mut Game<'_>) {
    if game.player.is_none() {
        with(|s| {
            s.data = None;
            s.recipes.close();
            s.filtered = None;
        });
        return;
    }
    let Some(recipes) = game.native_recipes() else {
        return;
    };
    let language = horizon_client::lang::code();
    let at = Arc::as_ptr(&recipes) as usize;
    let fresh = with(|s| s.data.as_ref().is_some_and(|d| d.recipes_at == at && d.language == language)).unwrap_or(true);
    if fresh {
        return;
    }
    let ru = rei::russian(&language);
    let data = build(game, &recipes, language, at, ru);
    with(|s| {
        s.data = Some(data);
        s.recipes.close();
        s.filtered = None;
        s.ru = ru;
        s.display_history.clear();
        s.history_page = 0;
    });
}

struct Builder<'a, 'g> {
    game: &'a Game<'g>,
    ingredients: Vec<Ingredient>,
    by_key: HashMap<String, u32>,
    tags: HashMap<String, Vec<String>>,
}

impl Builder<'_, '_> {
    /// The entry of `stack`, made when new.
    fn id(&mut self, stack: &ItemStack) -> u32 {
        let key = stack_key(stack);
        if let Some(&id) = self.by_key.get(&key) {
            return id;
        }
        let mut plain = stack.clone();
        plain.count = 1;
        plain.damage = 0;
        let names = &self.game.pack.names;
        let name = horizon_client::item_tooltip::name_line(&plain, &|k| names.display_get(k));
        let id_text = plain.item.to_string();
        let (namespace, path) = id_text.split_once(':').unwrap_or(("minecraft", &id_text));
        let ingredient = Ingredient {
            lower_name: name.to_lowercase(),
            name,
            path: path.to_owned(),
            lower_mod: rei::mod_display_name(namespace).to_lowercase(),
            namespace: namespace.to_owned(),
            tags: self.tags.get(&id_text).cloned().unwrap_or_default(),
            group: group_of(&id_text),
            stack: plain,
            key: key.clone(),
        };
        let id = self.ingredients.len() as u32;
        self.ingredients.push(ingredient);
        self.by_key.insert(key, id);
        id
    }
    fn item(&mut self, id: &str) -> Option<u32> {
        self.game.res.items().get(id)?;
        Some(self.id(&ItemStack::new(id, 1)))
    }
}

fn build(game: &Game<'_>, recipes: &super::native_hooks::NativeRecipes, language: String, at: usize, ru: bool) -> Data {
    let mut tags = HashMap::new();
    for item in &recipes.items {
        tags.insert(item.id.clone(), item.tags.clone());
    }
    let mut b = Builder {
        game,
        ingredients: Vec::new(),
        by_key: HashMap::new(),
        tags,
    };
    let mut list = Vec::new();
    let mut listed = HashSet::new();
    let mut push = |b: &mut Builder<'_, '_>, stack: &ItemStack, list: &mut Vec<u32>| {
        if &*stack.item == "minecraft:air" {
            return;
        }
        let id = b.id(stack);
        if listed.insert(id) {
            list.push(id);
        }
    };
    let search: Vec<ItemStack> = game.creative_screen.search_items().to_vec();
    for stack in &search {
        push(&mut b, stack, &mut list);
    }
    for item in &recipes.items {
        if game.res.items().get(&item.id).is_some() {
            push(&mut b, &ItemStack::new(&item.id, 1), &mut list);
        }
    }

    let mut out: Vec<Recipe> = Vec::new();
    let native_stack = |b: &mut Builder<'_, '_>, s: &super::native_hooks::NativeRecipeStack| {
        let id = recipes.items.get(s.item as usize)?.id.clone();
        let mut key = id;
        key.push_str(&s.components);
        let stack = stack_of(&key)?;
        Some(Stack { id: b.id(&stack), count: s.count.max(1), damage: stack.damage })
    };
    use super::native_hooks::NativeRecipeKind as K;
    for r in &recipes.recipes {
        if r.slots.iter().all(Vec::is_empty) || r.results.is_empty() {
            continue;
        }
        let category = match r.kind {
            K::CraftingShaped | K::CraftingShapeless => Category::Crafting,
            K::Smelting => Category::Smelting,
            K::Blasting => Category::Blasting,
            K::Smoking => Category::Smoking,
            K::Campfire => Category::Campfire,
            K::Stonecutting => Category::StoneCutting,
            K::Smithing => Category::Smithing,
            K::Brewing => Category::Brewing,
        };
        let mut inputs: Vec<Vec<Stack>> = r
            .slots
            .iter()
            .map(|slot| slot.iter().filter_map(|s| native_stack(&mut b, s)).collect())
            .collect();
        let shapeless = r.kind == K::CraftingShapeless;
        let (mut width, mut height) = (r.width, r.height);
        if category == Category::Crafting {
            (inputs, width, height) = rei::crafting_grid(inputs, width, shapeless);
        }
        let outputs = vec![r.results.iter().filter_map(|s| native_stack(&mut b, s)).collect()];
        out.push(Recipe {
            category: Some(category),
            id: r.id.clone(),
            inputs,
            outputs,
            width,
            height,
            shapeless,
            ticks: r.cooking_time,
            experience: r.experience,
            ..Recipe::default()
        });
    }
    let listed: Vec<ItemStack> = list.iter().map(|&id| b.ingredients[id as usize].stack.clone()).collect();
    special_crafting(game, &mut b, &listed, &mut out);
    composting(game, &mut b, &listed, &mut out);
    block_displays(game, &mut b, &mut out);
    beacon(game, &mut b, &list, &mut out);
    anvil(game, &mut b, &listed, &mut out);
    let (tags, item_tags) = tag_displays(game, &mut b, &mut out);

    let mut workstations: HashMap<Category, Vec<u32>> = HashMap::new();
    let mut icons = HashMap::new();
    for c in Category::ALL {
        let mut ids: Vec<u32> = c.workstations().iter().filter_map(|id| b.item(id)).collect();
        if let Some(tag) = c.tool_tag() {
            ids.extend(list.iter().copied().filter(|&id| b.ingredients[id as usize].has_tag(tag)));
        }
        workstations.insert(c, ids);
        if let Some(icon) = b.item(c.icon()) {
            icons.insert(c, icon);
        }
    }
    let blaze_powder = b.item("minecraft:blaze_powder");
    let names = &game.pack.names;
    let group_names = GROUPS
        .iter()
        .map(|(_, key)| match *key {
            "spawn_egg" | "music_disc" => rei::tr(ru, key).to_owned(),
            k => names.display_get(k).map_or_else(|| k.rsplit('.').next().unwrap_or(k).replace('_', " "), |s| s.to_string()),
        })
        .collect();
    let index = Index::new(out, workstations);
    Data {
        language,
        recipes_at: at,
        ingredients: b.ingredients,
        by_key: b.by_key,
        list,
        index,
        tooltips: None,
        group_names,
        icons,
        blaze_powder,
        tags,
        item_tags,
    }
}

/// A stack `/give` syntax names, kept only when its components were read
/// (a key the game can't read would make a plain item of it).
fn made(key: &str) -> Option<ItemStack> {
    let stack = stack_of(key)?;
    (key.contains('[') == stack.extra.is_some()).then_some(stack)
}

/// The crafting fillers of REI's default plugin for vanilla's special
/// recipes: shield decoration, firework rockets and book cloning.
fn special_crafting(game: &Game<'_>, b: &mut Builder<'_, '_>, listed: &[ItemStack], out: &mut Vec<Recipe>) {
    let has = |id: &str| game.res.items().get(id).is_some();
    let shapeless = |b: &mut Builder<'_, '_>, slots: Vec<Vec<Stack>>, outputs: Vec<Stack>, id: &str| {
        let (inputs, width, height) = rei::crafting_grid(slots, 0, true);
        let _ = b;
        Recipe {
            category: Some(Category::Crafting),
            id: id.to_owned(),
            inputs,
            outputs: vec![outputs],
            width,
            height,
            shapeless: true,
            ..Recipe::default()
        }
    };
    // Shields: every banner, the shield taking its colour.
    if has("minecraft:shield") {
        let shield = Stack::one(b.id(&ItemStack::new("minecraft:shield", 1)));
        let (mut banners, mut shields) = (Vec::new(), Vec::new());
        for banner in listed.iter().filter(|s| s.item.ends_with("_banner") && s.extra.is_none()) {
            let colour = banner.item.trim_start_matches("minecraft:").trim_end_matches("_banner");
            let Some(decorated) = made(&format!("minecraft:shield[minecraft:base_color=\"{colour}\"]")) else {
                continue;
            };
            banners.push(Stack::one(b.id(banner)));
            shields.push(Stack::one(b.id(&decorated)));
        }
        if !banners.is_empty() {
            let r = shapeless(b, vec![banners, vec![shield]], shields, "minecraft:shield_decoration");
            out.push(r);
        }
    }
    // Firework rockets: gunpowder, paper and up to two more gunpowder.
    if has("minecraft:paper") && has("minecraft:gunpowder") {
        let paper = Stack::one(b.id(&ItemStack::new("minecraft:paper", 1)));
        let powder = Stack::one(b.id(&ItemStack::new("minecraft:gunpowder", 1)));
        let empty = Stack { id: Stack::EMPTY, count: 1, damage: 0 };
        let rockets: Vec<Stack> = (1..=3)
            .filter_map(|flight| made(&format!("minecraft:firework_rocket[minecraft:fireworks={{flight_duration:{flight}}}]")))
            .map(|rocket| Stack { id: b.id(&rocket), count: 3, damage: 0 })
            .collect();
        if !rockets.is_empty() {
            let slots = vec![vec![powder], vec![paper], vec![empty, powder, powder], vec![empty, empty, powder]];
            let r = shapeless(b, slots, rockets, "minecraft:firework_rocket");
            out.push(r);
        }
    }
    // Book cloning: a written book and one to eight book and quills.
    if has("minecraft:written_book") && has("minecraft:writable_book") {
        let written = b.id(&ItemStack::new("minecraft:written_book", 1));
        let writable = Stack::one(b.id(&ItemStack::new("minecraft:writable_book", 1)));
        for copies in 1..=8u32 {
            let mut slots = vec![vec![Stack::one(written)]];
            slots.extend(std::iter::repeat_n(vec![writable], copies as usize));
            let r = shapeless(b, slots, vec![Stack { id: written, count: copies, damage: 0 }], "minecraft:book_cloning");
            out.push(r);
        }
    }
}

/// Composting: every compostable item, the least likely first, 35 a display.
fn composting(game: &Game<'_>, b: &mut Builder<'_, '_>, listed: &[ItemStack], out: &mut Vec<Recipe>) {
    let items = game.res.items();
    let mut compost: Vec<(f32, u32)> = Vec::new();
    for stack in listed.iter().filter(|s| s.extra.is_none()) {
        let Some(layers) = items.get(&stack.item).and_then(|i| i.compostable.as_ref()) else {
            continue;
        };
        let chance = compost_chance(layers);
        if chance > 0.0 {
            compost.push((chance, b.id(stack)));
        }
    }
    compost.sort_by(|a, b| a.0.total_cmp(&b.0));
    let Some(bone_meal) = b.item("minecraft:bone_meal") else {
        return;
    };
    for chunk in compost.chunks(35) {
        out.push(Recipe {
            category: Some(Category::Composting),
            inputs: chunk.iter().map(|&(_, id)| vec![Stack::one(id)]).collect(),
            chances: chunk.iter().map(|&(c, _)| c).collect(),
            outputs: vec![vec![Stack::one(bone_meal)]],
            ..Recipe::default()
        });
    }
}

/// The chance a compostable item adds a layer: its `layers` provider, a named
/// one of vanilla's, a number, or a weighted list (inside a dispatcher).
fn compost_chance(layers: &serde_json::Value) -> f32 {
    use serde_json::Value;
    match layers {
        Value::String(name) => rei::compost_chance_named(name).unwrap_or(0.0),
        Value::Number(n) => {
            if n.as_f64().unwrap_or(0.0) > 0.0 { 1.0 } else { 0.0 }
        }
        Value::Object(o) => match o.get("type").and_then(Value::as_str) {
            Some("minecraft:number_dispatcher") => o.get("default").map_or(0.0, compost_chance),
            Some("minecraft:weighted_list") => {
                let list = o.get("distribution").and_then(Value::as_array);
                let (mut hit, mut all) = (0.0, 0.0);
                for e in list.into_iter().flatten() {
                    let w = e.get("weight").and_then(Value::as_f64).unwrap_or(0.0);
                    let d = e.get("data").and_then(Value::as_f64).unwrap_or(0.0);
                    all += w;
                    if d >= 1.0 {
                        hit += w;
                    }
                }
                if all > 0.0 { (hit / all) as f32 } else { 0.0 }
            }
            _ => 0.0,
        },
        _ => 0.0,
    }
}

/// What a tool does to a block (`data/minecraft/block_transformer/*.json`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Change {
    Strip,
    Scrape,
    WaxOff,
    Till,
    Path,
}

/// The tools' block changes as (change, from block, to block), in file order.
fn block_changes(game: &Game<'_>) -> Vec<(Change, String, String)> {
    use serde_json::Value;
    let dir = game.options.resources.join("data/minecraft/block_transformer");
    let mut out = Vec::new();
    for (file, tool) in [("axe", 0), ("hoe", 1), ("shovel", 2)] {
        let Ok(bytes) = std::fs::read(dir.join(format!("{file}.json"))) else {
            continue;
        };
        let Ok(Value::Array(transforms)) = serde_json::from_slice::<Value>(&bytes) else {
            continue;
        };
        for t in &transforms {
            let particle = t["particle"].as_str().unwrap_or("");
            let change = match (tool, particle) {
                (0, p) if p.ends_with("scrape") => Change::Scrape,
                (0, p) if p.ends_with("wax_off") => Change::WaxOff,
                (0, _) => Change::Strip,
                (1, _) => Change::Till,
                _ => Change::Path,
            };
            let rules = t["block_state_provider"]["rules"].as_array().cloned().unwrap_or_default();
            for rule in &rules {
                let Some(target) = target_block(&rule["then"]) else {
                    continue;
                };
                for from in matching_blocks(&rule["if_true"]) {
                    if from != target {
                        out.push((change, from, target.clone()));
                    }
                }
            }
        }
    }
    out
}

/// The blocks a rule's predicate names (inside `all_of`/`any_of` too).
fn matching_blocks(p: &serde_json::Value) -> Vec<String> {
    use serde_json::Value;
    match p["type"].as_str() {
        Some("minecraft:matching_blocks") => match &p["blocks"] {
            Value::String(s) => vec![s.clone()],
            Value::Array(a) => a.iter().filter_map(|b| b.as_str().map(str::to_owned)).collect(),
            _ => Vec::new(),
        },
        Some("minecraft:all_of" | "minecraft:any_of") => {
            p["predicates"].as_array().into_iter().flatten().flat_map(matching_blocks).collect()
        }
        _ => Vec::new(),
    }
}

/// The block a rule turns it into (a state, or a `copy_properties` source).
fn target_block(then: &serde_json::Value) -> Option<String> {
    match then["type"].as_str() {
        Some("minecraft:copy_properties") => target_block(&then["source"]),
        Some("minecraft:rule_based") => None,
        _ => then["id"].as_str().or_else(|| then["Name"].as_str()).map(str::to_owned),
    }
}

/// Stripping, tilling, pathing, waxing, wax and oxidation scraping, and
/// oxidizing: one display a block, sorted by the block's id.
fn block_displays(game: &Game<'_>, b: &mut Builder<'_, '_>, out: &mut Vec<Recipe>) {
    let mut pairs: Vec<(Category, String, String)> = Vec::new();
    for (change, from, to) in block_changes(game) {
        let (c, from, to) = match change {
            Change::Strip => (Category::Stripping, from, to),
            Change::Till => (Category::Tilling, from, to),
            Change::Path => (Category::Pathing, from, to),
            Change::WaxOff => {
                pairs.push((Category::Waxing, to.clone(), from.clone()));
                (Category::WaxScraping, from, to)
            }
            Change::Scrape => {
                pairs.push((Category::Oxidizing, to.clone(), from.clone()));
                (Category::OxidationScraping, from, to)
            }
        };
        pairs.push((c, from, to));
    }
    pairs.sort();
    pairs.dedup();
    for (c, from, to) in pairs {
        let (Some(input), Some(output)) = (b.item(&from), b.item(&to)) else {
            continue;
        };
        out.push(Recipe {
            category: Some(c),
            inputs: vec![vec![Stack::one(input)]],
            outputs: vec![vec![Stack::one(output)]],
            ..Recipe::default()
        });
    }
}

/// The beacon's base blocks and payment items: one display each.
fn beacon(game: &Game<'_>, b: &mut Builder<'_, '_>, list: &[u32], out: &mut Vec<Recipe>) {
    use serde_json::Value;
    let file = game.options.resources.join("data/minecraft/tags/block/beacon_base_blocks.json");
    let base: Vec<String> = std::fs::read(file)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .and_then(|v| v["values"].as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|v| v.as_str().or_else(|| v["id"].as_str()).map(str::to_owned))
        .collect();
    let base: Vec<Stack> = base.iter().filter_map(|id| b.item(id)).map(Stack::one).collect();
    let payment: Vec<Stack> = list
        .iter()
        .copied()
        .filter(|&id| b.ingredients[id as usize].has_tag("minecraft:beacon_payment_items"))
        .map(Stack::one)
        .collect();
    for (c, stacks) in [(Category::BeaconBase, base), (Category::BeaconPayment, payment)] {
        if !stacks.is_empty() {
            out.push(Recipe {
                category: Some(c),
                inputs: stacks.into_iter().map(|s| vec![s]).collect(),
                ..Recipe::default()
            });
        }
    }
}

/// The anvil: each worn item mended with one to four of its material, and
/// each enchantable item with each enchanted book it takes, every level.
fn anvil(game: &Game<'_>, b: &mut Builder<'_, '_>, listed: &[ItemStack], out: &mut Vec<Recipe>) {
    let items = game.res.items();
    let e = game.enchantments();
    for stack in listed.iter().filter(|s| s.extra.is_none()) {
        let max = items.get(&stack.item).map_or(0, |i| i.max_damage);
        if max == 0 {
            continue;
        }
        let materials: Vec<u32> = listed
            .iter()
            .filter(|m| m.extra.is_none() && horizon_client::stations::is_valid_repair_item(e, items, stack, m))
            .map(|m| b.id(m))
            .collect();
        if materials.is_empty() {
            continue;
        }
        let id = b.id(stack);
        for count in 1..=4 {
            let damage = if count == 4 { max } else { max / 4 * count };
            let Some((left, cost)) = rei::anvil_repair(max, damage, count) else {
                continue;
            };
            out.push(Recipe {
                category: Some(Category::Anvil),
                inputs: vec![
                    vec![Stack { id, count: 1, damage }],
                    materials.iter().map(|&m| Stack { id: m, count, damage: 0 }).collect(),
                ],
                outputs: vec![vec![Stack { id, count: 1, damage: left }]],
                cost,
                ..Recipe::default()
            });
        }
    }
    let enchantable: Vec<&ItemStack> = listed
        .iter()
        .filter(|s| s.extra.is_none() && e.enchantable(&s.item) > 0 && !matches!(&*s.item, "minecraft:book" | "minecraft:enchanted_book"))
        .collect();
    for ench in &e.list {
        let levels: Vec<u32> = if ench.max_level >= 11 { vec![1, ench.max_level] } else { (1..=ench.max_level).collect() };
        for level in levels {
            let id = &ench.id;
            let Some(book) = made(&format!("minecraft:enchanted_book[minecraft:stored_enchantments={{\"{id}\":{level}}}]")) else {
                continue;
            };
            let book = Stack::one(b.id(&book));
            let cost = (ench.anvil_cost / 2).max(1) * level;
            for stack in &enchantable {
                if !e.can_enchant(items, id, &stack.item) {
                    continue;
                }
                let Some(result) = made(&format!("{}[minecraft:enchantments={{\"{id}\":{level}}}]", stack.item)) else {
                    continue;
                };
                out.push(Recipe {
                    category: Some(Category::Anvil),
                    inputs: vec![vec![Stack::one(b.id(stack))], vec![book]],
                    outputs: vec![vec![Stack::one(b.id(&result))]],
                    cost,
                    ..Recipe::default()
                });
            }
        }
    }
}

/// Every item tag, then every block tag (as their items), as REI's tag
/// category shows them: one display a tag, its entries its inputs and outputs.
fn tag_displays(game: &Game<'_>, b: &mut Builder<'_, '_>, out: &mut Vec<Recipe>) -> (Vec<rei::TagTree>, usize) {
    let root = game.options.resources.join("data/minecraft/tags");
    let mut trees = Vec::new();
    let mut item_tags = 0;
    for kind in ["item", "block"] {
        if kind == "block" {
            item_tags = trees.len();
        }
        let mut raw: std::collections::BTreeMap<String, Vec<String>> = std::collections::BTreeMap::new();
        read_tags(&root.join(kind), "", &mut raw);
        for id in raw.keys() {
            if let Some(tree) = tag_tree(id, &raw, b, 0) {
                trees.push(tree);
            }
        }
    }
    for (i, t) in trees.iter().enumerate() {
        let cells: Vec<Vec<Stack>> = t.all.iter().map(|&e| vec![Stack::one(e)]).collect();
        out.push(Recipe {
            category: Some(Category::Tag),
            id: t.id.clone(),
            inputs: cells.clone(),
            outputs: cells,
            tag: Some(i),
            ..Recipe::default()
        });
    }
    (trees, item_tags)
}

/// The tag files under `dir` by id (`minecraft:enchantable/mining`), each
/// with its values (`#` for an included tag).
fn read_tags(dir: &std::path::Path, prefix: &str, out: &mut std::collections::BTreeMap<String, Vec<String>>) {
    use serde_json::Value;
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            read_tags(&path, &format!("{prefix}{name}/"), out);
            continue;
        }
        let Some(stem) = name.strip_suffix(".json") else {
            continue;
        };
        let Ok(Value::Object(o)) = std::fs::read(&path).map_err(|_| ()).and_then(|b| serde_json::from_slice::<Value>(&b).map_err(|_| ())) else {
            continue;
        };
        let values = o
            .get("values")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_str().or_else(|| v.get("id").and_then(Value::as_str)).map(str::to_owned))
            .collect();
        out.insert(format!("minecraft:{prefix}{stem}"), values);
    }
}

/// A tag's tree (`TagNodes.resolveTag`): its own entries, then the tags it
/// includes, each with theirs.
fn tag_tree(id: &str, raw: &std::collections::BTreeMap<String, Vec<String>>, b: &mut Builder<'_, '_>, depth: u32) -> Option<rei::TagTree> {
    let values = raw.get(id)?;
    let mut tree = rei::TagTree { id: id.to_owned(), ..rei::TagTree::default() };
    for v in values {
        if let Some(child) = v.strip_prefix('#') {
            if depth < 16
                && let Some(t) = tag_tree(child, raw, b, depth + 1)
            {
                tree.children.push(t);
            }
        } else {
            tree.values.push(b.item(v).ok_or_else(|| v.clone()));
        }
    }
    let mut all: Vec<u32> = tree.values.iter().filter_map(|v| v.as_ref().ok().copied()).collect();
    for c in &tree.children {
        all.extend(c.all.iter().copied());
    }
    let mut seen = HashSet::new();
    all.retain(|e| seen.insert(*e));
    tree.all = all;
    Some(tree)
}

/// Each entry's whole tooltip, lowercased.
fn tooltips(game: &Game<'_>, data: &Data) -> Vec<String> {
    let mut listed = vec![false; data.ingredients.len()];
    for &id in &data.list {
        listed[id as usize] = true;
    }
    data.ingredients
        .iter()
        .zip(listed)
        .map(|(i, listed)| {
            if !listed {
                return String::new();
            }
            game.stack_tooltip(&i.stack)
                .iter()
                .map(|(t, _)| t.plain().to_lowercase())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .collect()
}

/// The list as the search field and the craftable filter leave it.
pub(super) fn filtered(game: &Game<'_>, state: &mut State) -> Vec<u32> {
    let text = state.search_text().to_owned();
    // The craftable set changes with the inventory.
    let read_at = state.craftable.then(|| state.inventory.as_ref().map(|(at, _)| *at)).flatten();
    let key = (text.clone(), state.craftable, read_at);
    if let Some(f) = &state.filtered
        && state.filtered_for == key
    {
        return f.clone();
    }
    let st = settings(game, state);
    let hide_idle = st.hide_idle;
    rei::set_search_modes(st.search_modes);
    let query = Query::parse(&text);
    let craftable: Option<HashSet<u32>> = state.craftable.then(|| state.inventory.as_ref().map(|(_, i)| i.craftable.clone()).unwrap_or_default());
    let Some(data) = state.data.as_mut() else {
        return Vec::new();
    };
    if query.needs_tooltips() && data.tooltips.is_none() {
        // Tooltips call back into the tooltip hook, which reads only cells.
        data.tooltips = Some(tooltips(game, data));
    }
    let mut out: Vec<u32> = if hide_idle && text.is_empty() {
        Vec::new()
    } else {
        data.list
            .iter()
            .copied()
            .filter(|id| craftable.as_ref().is_none_or(|c| c.contains(id)))
            .filter(|&id| {
                let i = &data.ingredients[id as usize];
                let s = rei::Searchable {
                    name: &i.lower_name,
                    name_case: &i.name,
                    mod_id: &i.namespace,
                    mod_name: &i.lower_mod,
                    tooltip: data.tooltips.as_ref().and_then(|t| t.get(id as usize)).map(String::as_str),
                    tags: &i.tags,
                    path: &i.path,
                };
                query.matches(&s)
            })
            .collect()
    };
    // `list.ordering`: by registry, name or groups (the creative tabs' order,
    // which the registry order here already is), maybe descending.
    if matches!(st.ordering, 1 | 4) {
        out.sort_by(|a, b| data.ingredients[*a as usize].name.cmp(&data.ingredients[*b as usize].name));
    }
    if st.ordering >= 3 {
        out.reverse();
    }
    state.filtered_for = key;
    state.filtered = Some(out.clone());
    out
}

/// A cell of the entry list: one entry, or a collapsed group's entries.
#[derive(Clone, PartialEq, Debug)]
pub(super) enum ListCell {
    One(u32),
    Group(u8, Vec<u32>),
}

/// The list's cells: each collapsed group where its first entry would be.
pub(super) fn cells(state: &State, ids: &[u32]) -> Vec<ListCell> {
    let Some(data) = state.data.as_ref() else {
        return Vec::new();
    };
    let mut out: Vec<ListCell> = Vec::new();
    let mut at: [Option<usize>; GROUPS.len()] = [None; GROUPS.len()];
    for &id in ids {
        match data.ingredient(id).and_then(|i| i.group) {
            Some(g) if !state.expanded[g as usize] => match at[g as usize] {
                Some(i) => {
                    if let ListCell::Group(_, list) = &mut out[i] {
                        list.push(id);
                    }
                }
                None => {
                    at[g as usize] = Some(out.len());
                    out.push(ListCell::Group(g, vec![id]));
                }
            },
            _ => out.push(ListCell::One(id)),
        }
    }
    // A group of one is that entry.
    for c in &mut out {
        if let ListCell::Group(_, list) = c
            && list.len() == 1
        {
            *c = ListCell::One(list[0]);
        }
    }
    out
}

// ---- the inventory, the "+" button and quick crafting ----------------------------------

/// What the inventory and the open crafting grid hold.
fn read_inventory(game: &mut Game<'_>) -> Inventory {
    let crafting = game.mod_recipe_screen();
    let size = game.craft_size.clamp(2, 3);
    let mut inv = Inventory { crafting, size, ..Inventory::default() };
    let grid = if crafting { 100..100 + size * size } else { 0..0 };
    for slot in (0..36).chain(grid) {
        if let Some(stack) = game.slot_view(slot) {
            let key = stack_key(&stack);
            let max = game.res.items().get(&stack.item).map_or(64, |i| i.max_stack.max(1));
            inv.max_stack.insert(key.clone(), max);
            *inv.have.entry(key).or_default() += stack.count;
        }
    }
    inv
}

/// The entries crafting recipes can make from what `inv` holds.
fn craftable(inv: &Inventory, data: &Data) -> HashSet<u32> {
    let mut out = HashSet::new();
    if !inv.crafting {
        return out;
    }
    for &i in data.index.of_category(Category::Crafting) {
        let r = &data.index.recipes[i];
        if status(inv, data, r) == Status::Ok {
            out.extend(r.outputs.iter().flatten().filter(|s| !s.is_empty()).map(|s| s.id));
        }
    }
    out
}

/// Whether the "+" button can move `r` in, with what the inventory holds.
pub(super) fn status(inv: &Inventory, data: &Data, r: &Recipe) -> Status {
    if r.category() != Category::Crafting || !inv.crafting {
        return Status::NotSupported;
    }
    if inv.size < 3 && (r.width > 2 || r.height > 2 || r.inputs.iter().enumerate().any(|(i, s)| !s.is_empty() && (i / 3 >= 2 || i % 3 >= 2))) {
        return Status::TooSmall;
    }
    let mut uses: HashMap<&str, u32> = HashMap::new();
    let mut missing = Vec::new();
    for (i, cell) in r.inputs.iter().enumerate() {
        if cell.iter().all(|s| s.is_empty()) {
            continue;
        }
        let found = cell
            .iter()
            .filter(|s| !s.is_empty())
            .filter_map(|s| data.ingredient(s.id).map(|i| i.key.as_str()))
            .find(|k| inv.have.get(*k).copied().unwrap_or(0) > uses.get(k).copied().unwrap_or(0));
        match found {
            Some(k) => *uses.entry(k).or_default() += 1,
            None => missing.push(i),
        }
    }
    if missing.is_empty() { Status::Ok } else { Status::Missing(missing) }
}

/// The status of display `r` now (the last inventory read).
pub(super) fn status_now(state: &State, r: &Recipe) -> Status {
    let Some((_, inv)) = state.inventory.as_ref() else {
        return Status::NotSupported;
    };
    let Some(data) = state.data.as_ref() else {
        return Status::NotSupported;
    };
    status(inv, data, r)
}

/// The "+" button: the recipe's items moved into the open crafting grid by
/// ordinary slot clicks, as a player would move them, so it works on any
/// server; `all` (Shift) fills it as many times as the items allow. The grid's
/// old contents go back to the inventory first, and REI goes back to the
/// container once the items are in.
fn transfer(game: &mut Game<'_>, recipe: usize, all: bool) {
    use horizon_client::menu::Input;
    let Some(cells) = with(|s| {
        let data = s.data.as_ref()?;
        let r = data.index.recipes.get(recipe)?;
        Some(
            r.inputs
                .iter()
                .map(|slot| {
                    slot.iter()
                        .filter(|st| !st.is_empty())
                        .filter_map(|st| data.ingredient(st.id).map(|i| i.key.clone()))
                        .collect::<Vec<String>>()
                })
                .collect::<Vec<_>>(),
        )
    })
    .flatten() else {
        return;
    };
    let note = |key: &str| {
        with(|s| s.note = Some((rei::tr(s.ru, key).to_owned(), Instant::now())));
    };
    if game.carried.is_some() || !game.mod_recipe_screen() {
        note("not_supported");
        return;
    }
    let size = game.craft_size.clamp(2, 3);
    let grid_slot = |i: usize| {
        let (row, col) = (i / 3, i % 3);
        (row < size && col < size).then_some(100 + row * size + col)
    };
    let mut plan: Vec<(usize, &Vec<String>)> = Vec::new();
    for (i, keys) in cells.iter().enumerate() {
        if keys.is_empty() {
            continue;
        }
        let Some(slot) = grid_slot(i) else {
            note("too_small");
            return;
        };
        plan.push((slot, keys));
    }
    // The grid back into the inventory.
    for slot in 100..100 + size * size {
        if game.slot_view(slot).is_some() {
            game.menu_click(Some(slot), 0, Input::QuickMove);
        }
    }
    if (100..100 + size * size).any(|slot| game.slot_view(slot).is_some()) {
        note("no_room");
        return;
    }
    let mut have: HashMap<String, u32> = HashMap::new();
    let mut max_stack: HashMap<String, u32> = HashMap::new();
    for slot in 0..36 {
        if let Some(stack) = game.slot_view(slot) {
            let key = stack_key(&stack);
            let max = game.res.items().get(&stack.item).map_or(64, |i| i.max_stack.max(1));
            max_stack.insert(key.clone(), max);
            *have.entry(key).or_default() += stack.count;
        }
    }
    let mut uses: HashMap<String, u32> = HashMap::new();
    let mut chosen: Vec<(usize, String)> = Vec::new();
    for (slot, keys) in &plan {
        let Some(key) = keys.iter().find(|k| have.get(*k).copied().unwrap_or(0) > uses.get(*k).copied().unwrap_or(0)) else {
            note("not_enough");
            return;
        };
        *uses.entry(key.clone()).or_default() += 1;
        chosen.push((*slot, key.clone()));
    }
    let times = if all {
        uses.iter()
            .map(|(k, n)| (have[k] / n).min(max_stack.get(k).copied().unwrap_or(64)))
            .min()
            .unwrap_or(1)
            .max(1)
    } else {
        1
    };
    for (cell, key) in chosen {
        let mut left = times;
        for source in 0..36 {
            if left == 0 {
                break;
            }
            let Some(stack) = game.slot_view(source) else {
                continue;
            };
            if stack_key(&stack) != key {
                continue;
            }
            game.menu_click(Some(source), 0, Input::Pickup);
            let mut placed = 0;
            while placed < left {
                let Some(carried) = game.carried.as_ref().map(|c| c.count) else {
                    break;
                };
                if carried <= left - placed {
                    game.menu_click(Some(cell), 0, Input::Pickup);
                    placed += carried;
                } else {
                    game.menu_click(Some(cell), 1, Input::Pickup);
                    placed += 1;
                }
            }
            if game.carried.is_some() {
                game.menu_click(Some(source), 0, Input::Pickup);
            }
            left -= placed;
        }
    }
    with(|s| {
        s.recipes.close();
        s.inventory = None;
    });
}

/// Ctrl+click on an entry: the first recipe for it that can be made here.
fn quick_craft(game: &mut Game<'_>, id: u32, all: bool) {
    let recipe = with(|s| {
        let data = s.data.as_ref()?;
        data.index
            .lookup(Lookup::Recipes(id))
            .into_iter()
            .find(|(c, _)| *c == Category::Crafting)?
            .1
            .into_iter()
            .find(|&i| status_now(s, &data.index.recipes[i]) == Status::Ok)
    })
    .flatten();
    if let Some(r) = recipe {
        transfer(game, r, all);
    }
}

// ---- hooks ---------------------------------------------------------------------------

/// Once a frame (`frame`): keeps the model and does what the screen hooks asked.
pub(super) fn frame(game: &mut Game<'_>) {
    if !on(game) {
        COVERED.with(|c| c.set(false));
        return;
    }
    sync(game);
    let open = matches!(game.screen, Screen::Inventory | Screen::Creative) && game.title.is_none();
    if !open {
        with(|s| {
            s.recipes.close();
            // The config screen over the Options screen stays.
            if s.config.as_ref().is_some_and(|c| !c.on_options()) {
                s.config = None;
            }
            s.menu = None;
            s.press = None;
            s.choose = None;
            s.inventory = None;
            if let Some(search) = s.search.as_mut() {
                search.focused = false;
            }
        });
    } else if with(|s| s.inventory.as_ref().is_none_or(|(at, _)| at.elapsed().as_millis() >= 250)).unwrap_or(false) {
        // The inventory, as the craftable filter and the "+" buttons read it.
        let mut inv = read_inventory(game);
        with(|s| {
            if let Some(data) = s.data.as_ref() {
                inv.craftable = craftable(&inv, data);
            }
            s.inventory = Some((Instant::now(), inv));
        });
    }
    COVERED.with(|c| c.set(open && with(|s| s.recipes.is_open() || s.config.is_some()).unwrap_or(false)));
    run_pending(game);
}

/// Does what the screen hooks and the config screen asked for.
pub(super) fn run_pending(game: &mut Game<'_>) {
    let actions = with(|s| std::mem::take(&mut s.pending)).unwrap_or_default();
    for action in actions {
        act(game, action);
    }
}

fn act(game: &mut Game<'_>, action: Action) {
    match action {
        Action::Give { id, count } => give(game, id, count),
        Action::Transfer { recipe, all } => transfer(game, recipe, all),
        Action::QuickCraft { id, all } => quick_craft(game, id, all),
        Action::SlotKey { key, at } => slot_key(game, key, at),
        Action::Command(command) => game.send_command(&command),
        Action::CopyText(text) => game.clipboard.set(&text),
        Action::Paste => {
            let text = game.clipboard.get();
            with(|s| {
                if let Some(search) = s.search.as_mut() {
                    let text: String = text.chars().filter(|c| !c.is_control()).collect();
                    search.insert_text(&text);
                }
            });
        }
        Action::Copy => {
            if let Some(text) = with(|s| s.search.as_ref().map(EditBox::selected_text)).flatten()
                && !text.is_empty()
            {
                game.clipboard.set(&text);
            }
        }
        Action::SetKey { name, key } => {
            game.native_set_mod_key(rei::ID, &name, Some(&key));
        }
        Action::Reload { search_only } => {
            with(|s| {
                s.filtered = None;
                if !search_only {
                    s.data = None;
                    s.recipes.close();
                }
            });
        }
        Action::ConfigPaste => {
            let text = game.clipboard.get();
            with(|s| {
                if let Some(c) = s.config.as_mut() {
                    let text: String = text.chars().filter(|c| !c.is_control()).collect();
                    c.paste(&text);
                }
            });
        }
    }
}

/// Cheat mode: a creative player takes the stack as from the creative
/// inventory; elsewhere it is `/give`, which the server allows to operators.
fn give(game: &mut Game<'_>, id: u32, count: u32) {
    let Some((stack, key)) = with(|s| s.data.as_ref().and_then(|d| d.ingredient(id)).map(|i| (i.stack.clone(), i.key.clone()))).flatten() else {
        return;
    };
    let max = game.res.items().get(&stack.item).map_or(64, |i| i.max_stack.max(1));
    let count = count.clamp(1, max);
    if game.creative {
        let mut stack = stack;
        stack.count = count;
        let _ = game.give(stack);
    } else {
        game.send_command(&format!("give @s {key} {count}"));
    }
}

/// R, U or A over one of the screen's own slots.
fn slot_key(game: &mut Game<'_>, key: SlotKey, at: (f32, f32)) {
    let Some(size) = game.renderer.as_ref().map(|r| [r.config.width, r.config.height]) else {
        return;
    };
    let batch = horizon_client::hud::HudBatch::new(size);
    let Some(slot) = game.slot_at(&batch, at.0, at.1) else {
        return;
    };
    let Some(stack) = game.slot_view(slot) else {
        return;
    };
    with(|s| {
        let Some(id) = s.data.as_ref().and_then(|d| d.id_of(&stack)) else {
            return;
        };
        match key {
            SlotKey::Recipes => open(s, Lookup::Recipes(id)),
            SlotKey::Usages => open(s, Lookup::Usages(id)),
            SlotKey::Favorite => {
                if let Some(k) = key_of(s, id) {
                    add_favorite(s, k, None);
                }
            }
        }
    });
}

/// Opens the recipe screen on `lookup`; nothing when it has no displays.
/// Crafting shows what can be made now first, then what this screen could
/// make with more items (`ViewsImpl`).
pub(super) fn open(state: &mut State, lookup: Lookup) {
    let Some(data) = state.data.as_ref() else {
        return;
    };
    if !state.recipes.open(&data.index, lookup) {
        return;
    }
    if let Some(search) = state.search.as_mut() {
        search.focused = false;
    }
    let rank = |s: &State, i: usize| match status_now(s, &data.index.recipes[i]) {
        Status::Ok => 0,
        Status::Missing(_) => 1,
        _ => 2,
    };
    if let Some(t) = state.recipes.tabs.iter().position(|(c, _)| *c == Category::Crafting) {
        let mut list = state.recipes.tabs[t].1.clone();
        list.sort_by_key(|&i| rank(state, i));
        state.recipes.tabs[t].1 = list;
    }
}

fn hit_at(state: &State, p: (f32, f32)) -> Option<Hit> {
    state.hits.iter().rev().find(|(r, _)| rei::inside(p, *r)).map(|(_, h)| *h)
}

fn screen_shown(game: &Game<'_>, screen: &NativeScreenView<'_>) -> bool {
    on(game) && matches!(screen.id, "inventory" | "creative") && game.title.is_none()
}

/// The entry under a hit, for R, U and A.
fn entry_of(state: &mut State, hit: Hit) -> Option<u32> {
    match hit {
        Hit::Entry(id) | Hit::Slot(id) | Hit::Workstation(id) => Some(id),
        Hit::Collapsed(g) => {
            let ids = state.filtered.clone().unwrap_or_default();
            cells(state, &ids).into_iter().find_map(|c| match c {
                ListCell::Group(x, list) if x == g => list.first().copied(),
                _ => None,
            })
        }
        Hit::Favorite(i) => {
            let key = favorites(state).keys.get(i).cloned()?;
            state.data.as_ref()?.ingredient_by_key(&key)
        }
        _ => None,
    }
}

/// A mouse button over a container screen
/// (`screen_mouse_button_pressed_pre`): true when the overlay took it.
pub(super) fn pressed(game: &Game<'_>, screen: &NativeScreenView<'_>, button: i32, modifiers: u32) -> bool {
    if !screen_shown(game, screen) {
        return false;
    }
    let p = (screen.mouse[0], screen.mouse[1]);
    let shift = modifiers & SHIFT != 0;
    let control = modifiers & CONTROL != 0;
    let alt = modifiers & ALT != 0;
    with(|s| {
        // The config screen takes every click.
        if s.config.is_some() {
            take_press(s, button);
            config::press(game, s, p, button);
            return true;
        }
        let hit = hit_at(s, p);
        // The choose-page popup takes every click; one outside it closes it.
        if s.choose.is_some() {
            take_press(s, button);
            match hit {
                Some(Hit::ChooseDone) if button == 0 => confirm_page(s),
                Some(Hit::ChooseField) => {
                    if let Some(c) = s.choose.as_mut() {
                        c.field.focused = true;
                        c.field.reset_blink();
                    }
                }
                Some(Hit::ChooseArea | Hit::ChooseDone) => {}
                _ => s.choose = None,
            }
            return true;
        }
        if hit != Some(Hit::Search)
            && let Some(search) = s.search.as_mut()
        {
            search.focused = false;
        }
        let covered = s.recipes.is_open();
        let Some(hit) = hit else {
            // The recipe screen covers the container: nothing under it is clicked.
            if covered {
                take_press(s, button);
                return true;
            }
            return false;
        };
        take_press(s, button);
        s.press = Some(Press { hit, button, at: p, shift, control, alt, dragging: false });
        let left = button == 0;
        match hit {
            Hit::ListPage(d) if left => {
                let pages = rei::pages(s.list_len, s.list_per.max(1));
                s.list_page = rei::turn(s.list_page, d, pages);
            }
            Hit::ListLabel if left && shift => {
                let pages = rei::pages(s.list_len, s.list_per.max(1));
                open_choose(s, true, pages, s.list_page);
            }
            Hit::ListLabel if left => s.list_page = 0,
            Hit::Search => {
                let search = search_box(s);
                search.focused = true;
                search.reset_blink();
                if button == 1 {
                    search.set_value("");
                } else if left {
                    place_cursor(game, s, p.0, shift);
                }
            }
            Hit::Craftable if left => {
                s.craftable = !s.craftable;
                s.filtered = None;
                s.list_page = 0;
            }
            Hit::Config if left => {
                if shift || control {
                    let cheat = !settings(game, s).cheating(game.creative);
                    set_setting(game, s, "cheat", cheat);
                } else {
                    open_config(game, s);
                }
            }
            Hit::Menu(item) if left => menu_click(game, s, item),
            Hit::AddButton if left => {
                s.panel = !s.panel;
                s.panel_at = Some(Instant::now());
            }
            Hit::Tab(t) if left => s.recipes.select_tab(t),
            Hit::TabPage(d) if left => {
                // `TabContainerWidget`'s page buttons: the next page's middle tab.
                let n = s.recipes.tabs.len() as i32;
                let tpp = rei::tabs_per_page(settings(game, s).larger_tabs) as i32;
                let page = (s.recipes.tab as i32 / tpp + d).rem_euclid(n / tpp + 1);
                let hi = (n - (tpp + 1) / 2).max(tpp / 2);
                s.recipes.select_tab((page * tpp + tpp / 2).clamp(tpp / 2, hi).clamp(0, (n - 1).max(0)) as usize);
            }
            Hit::ClickArea(c) if left => open(s, Lookup::Category(c)),
            Hit::Title if left => {
                if let Some(c) = s.recipes.category() {
                    open(s, Lookup::All(c));
                }
            }
            Hit::CategoryTurn(d) if left => s.recipes.turn_tab(d),
            Hit::RecipePage(d) if left => {
                let per = s.per_page.max(1);
                s.recipes.turn_page(d, per);
            }
            Hit::PageLabel if left && shift => {
                let pages = rei::pages(s.recipes.recipes().len(), s.per_page.max(1));
                open_choose(s, false, pages, s.recipes.page);
            }
            Hit::PageLabel if left => s.recipes.page = 0,
            Hit::Transfer(recipe) | Hit::HistoryTransfer(recipe) if left => s.pending.push(Action::Transfer { recipe, all: shift }),
            Hit::TagView(_) if left => s.pan_last = p,
            Hit::TagExpand(recipe) if left => {
                let view = s.tag_views.entry(recipe).or_default();
                view.expanded = !view.expanded;
                view.toggled = Some(Instant::now());
            }
            Hit::TagCopy(recipe) if left => {
                let text = s.data.as_ref().and_then(|d| {
                    let tree = d.tags.get(d.index.recipes.get(recipe)?.tag?)?;
                    Some(tree.text(&|id| d.ingredient(id).map_or_else(String::new, |i| i.stack.item.to_string())))
                });
                if let Some(text) = text {
                    s.pending.push(Action::CopyText(text));
                }
            }
            _ => {}
        }
        true
    })
    .unwrap_or(false)
}

/// Opens the choose-page popup on page `page` of `pages`.
fn open_choose(s: &mut State, list: bool, pages: usize, page: usize) {
    if pages <= 1 {
        return;
    }
    let mut field = EditBox::new(0.0, 0.0, 0.0, 0.0);
    field.max_length = pages.to_string().len().max(1);
    field.set_value(&(page + 1).to_string());
    field.focused = true;
    field.reset_blink();
    s.choose = Some(ChoosePage { list, pages, field });
    if let Some(search) = s.search.as_mut() {
        search.focused = false;
    }
}

/// The popup's Done (or Enter): the page typed, kept within the pages.
fn confirm_page(s: &mut State) {
    let Some(c) = s.choose.take() else {
        return;
    };
    let Ok(n) = c.field.value.trim().parse::<usize>() else {
        return;
    };
    let page = n.clamp(1, c.pages) - 1;
    if c.list {
        s.list_page = page;
    } else {
        s.recipes.page = page;
    }
}

/// The cursor of the search field at GUI `x`.
fn place_cursor(game: &Game<'_>, s: &mut State, x: f32, shift: bool) {
    let scroll = s.search_scroll;
    let Some(search) = s.search.as_mut() else {
        return;
    };
    let chars: Vec<char> = search.value.chars().collect();
    let mut at = scroll.min(chars.len());
    let mut px = search.x + 4.0;
    while at < chars.len() {
        let w = horizon_client::hud::HudBatch::text_width(&game.gui.atlas, &chars[at].to_string());
        if px + w / 2.0 > x {
            break;
        }
        px += w;
        at += 1;
    }
    search.cursor = at;
    if !shift {
        search.highlight = at;
    }
    search.reset_blink();
}

/// The config screen opens (the gear, "More Options...").
fn open_config(game: &Game<'_>, s: &mut State) {
    s.menu = None;
    s.press = None;
    s.choose = None;
    if let Some(search) = s.search.as_mut() {
        search.focused = false;
    }
    s.config = Some(ConfigScreen::new(game));
}

fn menu_click(game: &Game<'_>, s: &mut State, item: MenuItem) {
    match item {
        MenuItem::Cheating => {
            let cheat = !settings(game, s).cheating(game.creative);
            set_setting(game, s, "cheat", cheat);
        }
        MenuItem::CraftableFilter => {
            let on = !settings(game, s).craftable_button;
            set_setting(game, s, "craftable_button", on);
            if !on {
                s.craftable = false;
            }
        }
        MenuItem::LeftPanel => {
            let on = !settings(game, s).left_panel;
            set_setting(game, s, "left_panel", on);
        }
        MenuItem::ScrollingPanel => {
            let on = !settings(game, s).scrolling;
            set_setting(game, s, "scrolling", on);
        }
        MenuItem::SearchPosition(p) => {
            set_option(game, s, "layout.search_field_location", ["center", "bottom_side", "top_side"][usize::from(p.min(2))]);
        }
        MenuItem::DarkTheme => {
            let on = !settings(game, s).dark;
            set_setting(game, s, "dark", on);
        }
        MenuItem::ReducedMotion => {
            let on = !settings(game, s).reduced_motion;
            set_setting(game, s, "reduced_motion", on);
        }
        MenuItem::Syntax => {
            let on = !settings(game, s).syntax;
            set_setting(game, s, "syntax", on);
        }
        MenuItem::HideIdle => {
            let on = !settings(game, s).hide_idle;
            set_setting(game, s, "hide_idle", on);
        }
        MenuItem::MoreOptions => open_config(game, s),
    }
}

fn take_press(state: &mut State, button: i32) {
    if let Some(p) = state.pressed.get_mut(button.clamp(0, 7) as usize) {
        *p = true;
    }
}

/// A mouse button let go (`screen_mouse_button_released_pre`): REI acts on
/// entries here, on the cell the press was on, or drops a dragged entry.
pub(super) fn released(game: &Game<'_>, screen: &NativeScreenView<'_>, button: i32) -> bool {
    if !screen_shown(game, screen) {
        return false;
    }
    let p = (screen.mouse[0], screen.mouse[1]);
    with(|s| {
        if s.config.is_some() {
            config::release(s);
            if let Some(pressed) = s.pressed.get_mut(button.clamp(0, 7) as usize) {
                *pressed = false;
            }
            return true;
        }
        if let Some(press) = s.press
            && press.button == button
        {
            s.press = None;
            // A release away from the press is a drag even when no frame saw
            // the pointer move.
            let far = (p.0 - press.at.0).abs() + (p.1 - press.at.1).abs() > DRAG_START;
            let dragged = press.dragging || (far && draggable(press.hit) && s.settings.is_none_or(|x| x.favorites));
            if dragged {
                drop_entry(s, press, p);
            } else if hit_at(s, p) == Some(press.hit) {
                click_entry(game, s, press);
            }
        }
        s.pressed
            .get_mut(button.clamp(0, 7) as usize)
            .is_some_and(|p| std::mem::replace(p, false))
    })
    .unwrap_or(false)
}

/// A click on an entry, the favorites or a display's slot.
fn click_entry(game: &Game<'_>, s: &mut State, press: Press) {
    let (left, right) = (press.button == 0, press.button == 1);
    let covered = s.recipes.is_open();
    // Alt+click folds or unfolds a collapsible group.
    if left && press.alt {
        let group = match press.hit {
            Hit::Collapsed(g) => Some(g),
            Hit::Entry(id) => s.data.as_ref().and_then(|d| d.ingredient(id)).and_then(|i| i.group),
            _ => None,
        };
        if let Some(g) = group {
            s.expanded[g as usize] = !s.expanded[g as usize];
            return;
        }
    }
    if let Hit::Favorite(i) = press.hit
        && let Some(shortcut) = favorites(s).keys.get(i).and_then(|k| Shortcut::from_key(k))
    {
        if left {
            s.pending.push(Action::Command(shortcut.command()));
        }
        return;
    }
    if let Hit::History(recipe) = press.hit {
        open(s, Lookup::Display(recipe));
        return;
    }
    let Some(id) = entry_of(s, press.hit) else {
        return;
    };
    let listed = matches!(press.hit, Hit::Entry(_) | Hit::Collapsed(_) | Hit::Favorite(_));
    if listed && !covered {
        let st = settings(game, s);
        if st.cheating(game.creative) && !press.control {
            let max = s
                .data
                .as_ref()
                .and_then(|d| d.ingredient(id))
                .and_then(|i| game.res.items().get(&i.stack.item).map(|x| x.max_stack))
                .unwrap_or(64);
            // `cheats.amount`: REI's (right or Shift for a stack) or reversed.
            let all = if st.cheat_reversed { !right || press.shift } else { right || press.shift };
            let count = if all { max } else { 1 };
            if left || right {
                s.pending.push(Action::Give { id, count });
            }
            return;
        }
        if press.control && (left || right) {
            s.pending.push(Action::QuickCraft { id, all: right || press.shift });
            return;
        }
    }
    if left {
        open(s, Lookup::Recipes(id));
    } else if right {
        open(s, Lookup::Usages(id));
    }
}

/// The key a dragged hit stands for in the favorites.
fn drag_key(s: &mut State, hit: Hit) -> Option<String> {
    match hit {
        Hit::Shortcut(sc) => Some(sc.key()),
        Hit::Favorite(i) => favorites(s).keys.get(i).cloned(),
        h => {
            let id = entry_of(s, h)?;
            key_of(s, id)
        }
    }
}

/// Whether a press on `hit` can drag.
pub(super) fn draggable(hit: Hit) -> bool {
    matches!(hit, Hit::Entry(_) | Hit::Collapsed(_) | Hit::Favorite(_) | Hit::Slot(_) | Hit::Workstation(_) | Hit::Shortcut(_))
}

/// The pointer moved with a button held: an entry starts dragging.
pub(super) fn drag_moved(state: &mut State, cursor: (f32, f32)) {
    if let Some(press) = state.press.as_mut()
        && !press.dragging
        && draggable(press.hit)
        && (cursor.0 - press.at.0).abs() + (cursor.1 - press.at.1).abs() > DRAG_START
        && state.settings.is_none_or(|s| s.favorites)
    {
        press.dragging = true;
    }
}

/// A dragged entry let go: into the favorites where it lands; a favorite
/// dragged out of them goes away.
fn drop_entry(s: &mut State, press: Press, p: (f32, f32)) {
    let Some(key) = drag_key(s, press.hit) else {
        return;
    };
    let over = s.favorites_grid.filter(|(area, ..)| rei::inside(p, *area));
    match over {
        Some((_, x, y, cols)) => {
            let col = (((p.0 - x) / 18.0).floor().max(0.0) as usize).min(cols.saturating_sub(1));
            let row = ((p.1 - y + s.favorites_scroll) / 18.0).floor().max(0.0) as usize;
            let at = row * cols + col;
            add_favorite(s, key, Some(at));
        }
        None => {
            if matches!(press.hit, Hit::Favorite(_)) {
                remove_favorite(s, &key);
            }
        }
    }
}

/// The wheel (`screen_mouse_scrolled_pre`): pages of the list and the recipe
/// screen, the favorites' scroll.
pub(super) fn scrolled(game: &Game<'_>, screen: &NativeScreenView<'_>, scroll: [f64; 2]) -> bool {
    if !screen_shown(game, screen) {
        return false;
    }
    let p = (screen.mouse[0], screen.mouse[1]);
    let delta = if scroll[1] > 0.0 {
        -1
    } else if scroll[1] < 0.0 {
        1
    } else {
        return false;
    };
    let control = game.has_control_down();
    with(|s| {
        if s.config.is_some() {
            config::scroll(s, p, delta);
            return true;
        }
        // Ctrl+wheel zooms the entries (`EntryListWidget`, `FavoritesListWidget`).
        let over_list = s.list_area.is_some_and(|a| rei::inside(p, a));
        let over_favorites = s.favorites_grid.is_some_and(|(f, ..)| rei::inside(p, f));
        if control && (over_list || over_favorites) && !s.recipes.is_open() {
            let step = if over_list { 0.05 } else { 0.075 };
            let zoom = settings(game, s).zoom;
            let new = (zoom - step * delta as f32).clamp(0.25, 4.0);
            s.zoom_at = Some(Instant::now());
            if (new - zoom).abs() > f32::EPSILON {
                let token = format!("{}", (new * 1000.0).round() / 1000.0);
                set_option(game, s, "list.zoom", &token);
            }
            return true;
        }
        let hit = hit_at(s, p);
        if let Some(area) = s.history_area
            && rei::inside(p, area)
            && !s.display_history.is_empty()
        {
            let n = s.display_history.len() as i64;
            s.history_page = (s.history_page as i64 + i64::from(delta)).clamp(0, n - 1) as usize;
            return true;
        }
        if let Some(area) = s.panel_area
            && rei::inside(p, area)
        {
            s.panel_scroll = (s.panel_scroll + delta as f32 * 19.0).max(0.0);
            return true;
        }
        if let Some((area, ..)) = s.favorites_grid
            && rei::inside(p, area)
        {
            s.favorites_scroll = (s.favorites_scroll + delta as f32 * 19.0).max(0.0);
            return true;
        }
        // A tag view zooms (`OverflowWidget.mouseScrolled`): up draws it larger.
        if let Some(recipe) = s.hits.iter().rev().find_map(|(r, h)| match h {
            Hit::TagView(i) if rei::inside(p, *r) => Some(*i),
            _ => None,
        }) {
            let view = s.tag_views.entry(recipe).or_default();
            view.scale += 0.2 * delta as f32;
            return true;
        }
        match hit {
            Some(Hit::Menu(_)) => return true,
            Some(Hit::Tab(_) | Hit::TabPage(_)) => {
                s.recipes.turn_tab(delta);
                return true;
            }
            _ => {}
        }
        if s.list_area.is_some_and(|a| rei::inside(p, a)) {
            if s.settings.is_some_and(|x| x.scrolling) {
                s.list_scroll = (s.list_scroll + delta as f32 * 19.0).max(0.0);
                return true;
            }
            let pages = rei::pages(s.list_len, s.list_per.max(1));
            if pages > 1 {
                s.list_page = rei::turn(s.list_page, delta, pages);
            }
            return true;
        }
        if s.recipes.is_open() {
            let per = s.per_page.max(1);
            s.recipes.turn_page(delta, per);
            return true;
        }
        hit.is_some()
    })
    .unwrap_or(false)
}

/// The favorite key's name as REI shows it in tooltips, upper case: "A".
pub(super) fn favorite_key_name(game: &Game<'_>) -> String {
    let key = bound(game, "favorite");
    key.rsplit('.').next().unwrap_or("a").to_uppercase()
}

/// The key a mod key is bound to (`key.keyboard.r`).
pub(super) fn bound(game: &Game<'_>, name: &str) -> String {
    let default = rei::KEYS.iter().find(|k| k.0 == name).map_or("key.keyboard.unknown", |k| k.2);
    game.native_mod_key(rei::ID, name, default)
}

/// A key over a container screen (`screen_key_pressed_pre`): true when the
/// mod took it.
pub(super) fn key_pressed(game: &Game<'_>, screen: &NativeScreenView<'_>, key: &str, modifiers: u32) -> bool {
    if !screen_shown(game, screen) {
        return false;
    }
    let control = modifiers & CONTROL != 0;
    let shift = modifiers & SHIFT != 0;
    let p = (screen.mouse[0], screen.mouse[1]);
    // REI's keybinds match with their modifiers exactly (`ModifierKeyCode.matchesKey`).
    let held = (if shift { config::SHIFT } else { 0 }) | (if control { config::CTRL } else { 0 }) | (if modifiers & ALT != 0 { config::ALT } else { 0 });
    let is = |name: &str| {
        let b = bound(game, name);
        b != "key.keyboard.unknown" && b == key && key_mods(game, name) == held
    };
    with(|s| {
        // The config screen takes every key.
        if s.config.is_some() {
            config::key(s, key, modifiers);
            return true;
        }
        let covered = s.recipes.is_open();
        // The choose-page popup: Enter confirms, Escape closes it.
        if let Some(c) = s.choose.as_mut() {
            match key {
                "key.keyboard.enter" | "key.keyboard.keypad.enter" => confirm_page(s),
                "key.keyboard.escape" => s.choose = None,
                _ => {
                    if c.field.focused
                        && let Some(code) = horizon_client::keybinds::key_code(key)
                    {
                        c.field.key(code, control, shift);
                        c.field.reset_blink();
                    }
                }
            }
            return true;
        }
        // The search field takes every key but Escape.
        if s.search.as_ref().is_some_and(|b| b.focused) {
            let search = s.search.as_mut().expect("focused");
            match key {
                "key.keyboard.escape" => {
                    search.focused = false;
                    if covered {
                        s.recipes.close();
                        return true;
                    }
                    return false;
                }
                "key.keyboard.enter" | "key.keyboard.keypad.enter" => {
                    let text = search.value.clone();
                    search.focused = false;
                    if !text.is_empty() {
                        s.history.retain(|h| !h.eq_ignore_ascii_case(&text));
                        s.history.push(text);
                        if s.history.len() > 100 {
                            s.history.remove(0);
                        }
                    }
                    s.history_at = None;
                }
                "key.keyboard.up" | "key.keyboard.down" => {
                    let n = s.history.len();
                    let at = match (key, s.history_at) {
                        ("key.keyboard.up", None) => n.checked_sub(1),
                        ("key.keyboard.up", Some(i)) => Some(i.saturating_sub(1)),
                        (_, Some(i)) if i + 1 < n => Some(i + 1),
                        _ => None,
                    };
                    s.history_at = at;
                    let text = at.and_then(|i| s.history.get(i).cloned()).unwrap_or_default();
                    if let Some(search) = s.search.as_mut() {
                        search.set_value(&text);
                    }
                }
                "key.keyboard.v" if control => s.pending.push(Action::Paste),
                "key.keyboard.c" if control => s.pending.push(Action::Copy),
                "key.keyboard.x" if control => {
                    s.pending.push(Action::Copy);
                    search.delete_chars(0);
                }
                _ => {
                    if let Some(code) = horizon_client::keybinds::key_code(key) {
                        search.key(code, control, shift);
                    }
                }
            }
            if let Some(search) = s.search.as_mut() {
                search.reset_blink();
            }
            return true;
        }
        if is("hide") {
            let shown = !settings(game, s).overlay;
            set_setting(game, s, "overlay", shown);
            return true;
        }
        if is("focus_search") {
            if settings(game, s).overlay {
                let search = search_box(s);
                search.focused = true;
                search.reset_blink();
            }
            return true;
        }
        // A text box of the game's has the keys (creative search, recipe
        // book, anvil name): REI's keys wait.
        let game_typing = (game.screen == Screen::Creative && game.creative_screen.tab().search && game.creative_screen.search.focused)
            || game.recipe_search_focused()
            || game.station_typing();
        if game_typing && !covered {
            return false;
        }
        let hit = hit_at(s, p);
        // The recipe's identifier over its "+" button.
        if is("copy_recipe_id")
            && let Some(Hit::Transfer(ri) | Hit::TransferOff(ri) | Hit::HistoryTransfer(ri)) = hit
        {
            if let Some(id) = s.data.as_ref().and_then(|d| d.index.recipes.get(ri)).map(|r| r.id.clone())
                && !id.is_empty()
            {
                s.pending.push(Action::CopyText(id));
            }
            return true;
        }
        let lookup_key = if is("recipe") {
            Some(SlotKey::Recipes)
        } else if is("usage") {
            Some(SlotKey::Usages)
        } else if is("favorite") {
            Some(SlotKey::Favorite)
        } else {
            None
        };
        if let (Some(SlotKey::Favorite), Some(Hit::Transfer(ri) | Hit::TransferOff(ri) | Hit::HistoryTransfer(ri))) = (lookup_key, hit) {
            if settings(game, s).favorites {
                s.display_history.retain(|h| *h != ri);
                s.display_history.insert(0, ri);
                s.display_history.truncate(9);
                s.history_page = 0;
            }
            return true;
        }
        if let (Some(SlotKey::Favorite), Some(Hit::History(ri))) = (lookup_key, hit) {
            s.display_history.retain(|h| *h != ri);
            s.history_page = s.history_page.min(s.display_history.len().saturating_sub(1));
            return true;
        }
        if let Some(k) = lookup_key {
            // A over a favorite takes it away.
            if let (SlotKey::Favorite, Some(Hit::Favorite(i))) = (k, hit) {
                if let Some(key) = favorites(s).keys.get(i).cloned() {
                    remove_favorite(s, &key);
                }
                return true;
            }
            let over = hit.and_then(|h| entry_of(s, h));
            match over {
                Some(id) => match k {
                    SlotKey::Recipes => open(s, Lookup::Recipes(id)),
                    SlotKey::Usages => open(s, Lookup::Usages(id)),
                    SlotKey::Favorite => {
                        if settings(game, s).favorites
                            && let Some(key) = key_of(s, id)
                        {
                            add_favorite(s, key, None);
                        }
                    }
                },
                None if !covered && hit.is_none() => {
                    s.pending.push(Action::SlotKey { key: k, at: p });
                    // The key's own use (A, R or U on a slot) is none: let it go on.
                    return false;
                }
                None => {}
            }
            return true;
        }
        if covered {
            let inventory = game.settings.keys.get("key.inventory") == key;
            if key == "key.keyboard.escape" || inventory {
                s.recipes.close();
            } else if is("back") {
                if let Some(data) = s.data.as_ref()
                    && !s.recipes.back(&data.index)
                {
                    s.recipes.close();
                }
            } else if is("previous_page") || is("next_page") {
                let d = if is("previous_page") { -1 } else { 1 };
                let per = s.per_page.max(1);
                s.recipes.turn_page(d, per);
            }
            // Nothing under the recipe screen hears a key.
            return true;
        }
        false
    })
    .unwrap_or(false)
}

/// A character typed (`screen_char_typed_pre`): the search field's, when it
/// has focus.
pub(super) fn char_typed(game: &Game<'_>, screen: &NativeScreenView<'_>, c: char) -> bool {
    if !screen_shown(game, screen) {
        return false;
    }
    with(|s| {
        if s.config.is_some() {
            config::char_typed(s, c);
            return true;
        }
        let covered = s.recipes.is_open();
        // The popup's field takes digits only.
        if let Some(choose) = s.choose.as_mut() {
            if choose.field.focused && c.is_ascii_digit() {
                choose.field.insert_text(&c.to_string());
                choose.field.reset_blink();
            }
            return true;
        }
        match s.search.as_mut() {
            Some(search) if search.focused => {
                search.insert_text(&c.to_string());
                search.reset_blink();
                true
            }
            _ => covered,
        }
    })
    .unwrap_or(false)
}

/// The container screen closed (`mod_browser_closed`).
pub(super) fn closed() {
    with(|s| {
        s.recipes.close();
        if s.config.as_ref().is_some_and(|c| !c.on_options()) {
            s.config = None;
        }
        s.menu = None;
        s.press = None;
        s.choose = None;
        if let Some(search) = s.search.as_mut() {
            search.focused = false;
        }
    });
    COVERED.with(|c| c.set(false));
}

/// The item tooltip (`tooltip`): the mod's name at its foot, blue and
/// italic, as REI adds it.
pub(super) fn tooltip(stack: &ItemStack, lines: &mut Vec<(horizon_client::text::Text, u32)>) {
    if !MOD_NAME.with(Cell::get) || !OWN_TOOLTIP.with(Cell::get) || lines.is_empty() {
        return;
    }
    let namespace = stack.item.split(':').next().unwrap_or("minecraft");
    let name = rei::mod_display_name(namespace);
    lines.push((horizon_client::text::Text::legacy(&format!("§9§o{name}")), u32::MAX));
}

/// The game's tooltips under the recipe screen stay hidden
/// (`native_tooltip_render_pre`); this mod's own show.
pub(super) fn hide_tooltip() -> bool {
    COVERED.with(Cell::get) && !OWN_TOOLTIP.with(Cell::get)
}

// ---- Options > Mods ------------------------------------------------------------

/// The row on Options > Mods, with the mod's keys.
pub(super) fn mods(game: &Game<'_>, mods: &mut Vec<super::native_hooks::NativeMod>) {
    let ru = rei::russian(&horizon_client::lang::code());
    let _ = game;
    mods.push(super::native_hooks::NativeMod {
        id: rei::ID.into(),
        title: format!("{} {}", rei::NAME, rei::VERSION),
        description: rei::tr(ru, "description").into(),
        keys: rei::KEYS
            .iter()
            .map(|(name, label, default)| [name.to_string(), rei::tr(ru, label).to_string(), default.to_string()])
            .collect(),
    });
}

/// The settings page's buttons: Mod Menu's "Configure..." first (REI's config
/// screen), then the switches and the favorites.
pub(super) fn settings_buttons(game: &mut Game<'_>, mod_id: &str, buttons: &mut Vec<String>) {
    if mod_id != rei::ID {
        return;
    }
    let ru = rei::russian(&horizon_client::lang::code());
    buttons.push(rei::tr(ru, "modmenu_configure").to_owned());
    let game: &Game<'_> = game;
    with(|st| {
        let s = settings(game, st);
        for key in Settings::KEYS {
            let state = rei::tr(ru, if s.get(key) { "on" } else { "off" });
            buttons.push(format!("{}: {state}", rei::tr(ru, key)));
        }
        let count = favorites(st).keys.len();
        buttons.push(format!("{} ({count})", rei::tr(ru, "clear_favorites")));
    });
}

pub(super) fn settings_click(game: &mut Game<'_>, mod_id: &str, button: usize) {
    if mod_id != rei::ID {
        return;
    }
    // The first button and Mod Menu's Configure: REI's config screen.
    if button == 0 || button == super::roughlyenoughitems_options::MODMENU_CONFIGURE {
        super::roughlyenoughitems_options::open(game, button != 0);
        return;
    }
    let button = button - 1;
    let game: &Game<'_> = game;
    with(|st| {
        if let Some(key) = Settings::KEYS.get(button) {
            let on = !settings(game, st).get(key);
            set_setting(game, st, key, on);
        } else if button == Settings::KEYS.len() {
            favorites(st).keys.clear();
            save_favorites(st);
        }
    });
}

/// The search field, made on first use.
pub(super) fn search_box(state: &mut State) -> &mut EditBox {
    state.search.get_or_insert_with(|| {
        let mut edit = EditBox::new(0.0, 0.0, 0.0, 0.0);
        edit.max_length = SEARCH_MAX;
        edit.focused = false;
        edit
    })
}
