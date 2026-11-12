// InvMove in the game (`InvMove`, `VanillaModule`, `ModuleImpl` and the
// mixins): which screen is open, whether it lets the player move
// (`allowMovementInScreen`) and whether its background tint is hidden
// (`shouldDisableScreenBackground`), the player's keys while a screen is open
// (`onInputUpdate`, on the local player's input tick), the toggle key, the
// jump key that must not press a focused button, and the mod's row in
// Options > Mods and its Mod Menu data.
//
// Ported from InvMove by PieKing1215 (LGPL-3.0-only,
// https://github.com/PieKing1215/InvMove).
//
// A Java screen is a class; Horizon's screens are its `Screen` variants, the
// overlays drawn over the world (books, signs, prompts, dialogs) and, on the
// inventory screen, the menu the server opened. `classify` names each as the
// original's class (`net.minecraft...`) so the vanilla module's rules and the
// "Unrecognized UI Types" lists read as in Java; Horizon's own screens are
// `horizon:<Name>`, a mod's menu or screen `<mod id>:<name>`.
use super::invmove_config::{self as cfg, Config, PauseMode, SneakMode};
use super::{Game, Screen};
use super::books::BookScreen;
use horizon_client::actions::Action;
use horizon_client::keymapping::{SNEAK, SPRINT};
use horizon_client::mod_api::NativeMod;
use horizon_client::sprint::Keys;
use std::cell::RefCell;
use std::sync::atomic::{AtomicU8, Ordering};

// ---- screens -----------------------------------------------------------------------------------

/// What the vanilla module says of a screen (`VanillaModule`'s `register`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Vanilla {
    /// Movement `SUGGEST_DISABLE`, background `SUGGEST_SHOW` (death, stats,
    /// credits, loading, chat, signs, command and structure block editors).
    Disable,
    /// `BookEditScreen`: movement `SUGGEST_DISABLE`, background as the book option.
    BookEdit,
    /// `GameModeSwitcherScreen`: movement `SUGGEST_ENABLE`; its background
    /// is the unrecognized screens'.
    GameModeSwitcher,
    /// An option of the module (`cfg(id)`), for movement and background.
    Option(&'static str),
}

/// The open screen as InvMove sees it.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct ScreenInfo {
    /// Its class (`getName`), the key of the unrecognized screens' lists.
    pub(super) id: String,
    pub(super) vanilla: Option<Vanilla>,
    /// `Screen.isPauseScreen`.
    pub(super) pause: bool,
    /// An `AbstractRecipeBookScreen` whose recipe book is visible.
    pub(super) recipe_book: bool,
}

fn info(id: &str, vanilla: Option<Vanilla>, pause: bool) -> Option<ScreenInfo> {
    Some(ScreenInfo { id: id.to_owned(), vanilla, pause, recipe_book: false })
}

const MC: &str = "net.minecraft.client.gui.screens.";

/// The vanilla screens `classify` names, for the unrecognized screens read
/// back from the file (`Class.forName`): the ones this game can show.
const VANILLA_SCREENS: [&str; 16] = [
    "net.minecraft.client.gui.screens.PauseScreen",
    "net.minecraft.client.gui.screens.options.OptionsScreen",
    "net.minecraft.client.gui.screens.ConfirmScreen",
    "net.minecraft.client.gui.screens.ConfirmLinkScreen",
    "net.minecraft.client.gui.screens.multiplayer.CodeOfConductScreen",
    "net.minecraft.client.multiplayer.ClientCommonPacketListenerImpl$PackConfirmScreen",
    "net.minecraft.client.gui.screens.dialog.DialogScreen",
    "net.minecraft.client.gui.screens.dialog.DialogScreen$WarningScreen",
    "net.minecraft.client.gui.screens.dialog.ServerLinksDialogScreen",
    "net.minecraft.client.gui.screens.social.SocialInteractionsScreen",
    "net.minecraft.client.gui.screens.FriendsScreen",
    "net.minecraft.client.gui.screens.WorldOptionsScreen",
    "net.minecraft.client.gui.screens.debug.DebugOptionsScreen",
    "net.minecraft.client.gui.screens.debug.GameModeSwitcherScreen",
    "net.minecraft.client.gui.screens.inventory.CrafterScreen",
    "net.minecraft.client.gui.screens.inventory.NautilusInventoryScreen",
];

/// An unrecognized screen the file names that this game can have: a vanilla
/// one `classify` gives, or Horizon's or a mod's (`<id>:<name>`).
pub(super) fn known_screen(id: &str) -> bool {
    VANILLA_SCREENS.contains(&id) || id.contains(':')
}

/// The class's simple name (`getSimpleName`).
pub(super) fn simple_name(id: &str) -> &str {
    id.rsplit(['.', '$', ':']).next().unwrap_or(id)
}

/// The mod a screen belongs to (`modidFromClass`).
pub(super) fn modid(id: &str) -> &str {
    if id.starts_with("net.minecraft.") {
        "minecraft"
    } else {
        id.split_once(':').map_or("?unknown", |(ns, _)| ns)
    }
}

/// `modNameFromModid`: the mod's name, or its id.
pub(super) fn mod_name(game: &Game<'_>, modid: &str) -> String {
    match modid {
        "minecraft" => "Minecraft".to_owned(),
        "horizon" => "Horizon".to_owned(),
        cfg::ID => cfg::NAME.to_owned(),
        _ => game
            .settings
            .mods
            .installed
            .iter()
            .find(|m| m.id == modid)
            .map(|m| {
                // The row's title is "Name 1.0": the name without its version.
                match m.title.rsplit_once(' ') {
                    Some((name, version)) if version.starts_with(|c: char| c.is_ascii_digit()) => name.to_owned(),
                    _ => m.title.clone(),
                }
            })
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| modid.to_owned()),
    }
}

/// The inventory screen's menu: the screen class it opens (`MenuScreens`).
fn menu_screen(game: &Game<'_>) -> Option<ScreenInfo> {
    let inv = |class: &str, option: &'static str| info(&format!("{MC}inventory.{class}"), Some(Vanilla::Option(option)), false);
    let menu = game.remote_menu_type().filter(|m| !m.is_empty());
    let Some(menu) = menu else {
        // The client's own inventory (`InventoryScreen`).
        return inv("InventoryScreen", "inventory").map(|mut i| {
            i.recipe_book = recipe_book_open(game);
            i
        });
    };
    if menu.starts_with("horizon:mount") {
        // `HorseInventoryScreen` (horses, donkeys, mules, llamas), or a
        // nautilus' (`NautilusInventoryScreen`, which InvMove doesn't know).
        let nautilus = game
            .mount_menu
            .opened_mob()
            .and_then(|id| game.mobs.iter().find(|m| m.id == id))
            .is_some_and(|m| horizon_client::mobs::nautilus::is_nautilus(m.species()));
        return if nautilus {
            info(&format!("{MC}inventory.NautilusInventoryScreen"), None, false)
        } else {
            inv("HorseInventoryScreen", "horseInventory")
        };
    }
    let (class, option, book) = match menu {
        m if m.starts_with("minecraft:generic_9x") => ("ContainerScreen", "chest", false),
        "minecraft:generic_3x3" => ("DispenserScreen", "dispenser", false),
        "minecraft:shulker_box" => ("ShulkerBoxScreen", "shulker", false),
        "minecraft:hopper" => ("HopperScreen", "hopper", false),
        "minecraft:crafting" => ("CraftingScreen", "crafting", true),
        "minecraft:enchantment" => ("EnchantmentScreen", "enchantment", false),
        "minecraft:anvil" => ("AnvilScreen", "anvil", false),
        "minecraft:beacon" => ("BeaconScreen", "beacon", false),
        "minecraft:brewing_stand" => ("BrewingStandScreen", "brewing", false),
        "minecraft:furnace" => ("FurnaceScreen", "furnace", true),
        "minecraft:blast_furnace" => ("BlastFurnaceScreen", "blastFurnace", true),
        "minecraft:smoker" => ("SmokerScreen", "smoker", true),
        "minecraft:loom" => ("LoomScreen", "loom", false),
        "minecraft:cartography_table" => ("CartographyTableScreen", "cartography", false),
        "minecraft:grindstone" => ("GrindstoneScreen", "grindstone", false),
        "minecraft:stonecutter" => ("StonecutterScreen", "stonecutter", false),
        "minecraft:smithing" => ("SmithingScreen", "smithing", false),
        "minecraft:merchant" => ("MerchantScreen", "villager", false),
        // `LecternScreen` is a `BookViewScreen`.
        "minecraft:lectern" => ("LecternScreen", "book", false),
        "minecraft:crafter_3x3" => return info(&format!("{MC}inventory.CrafterScreen"), None, false),
        // A menu InvMove has no rule for: a mod's, named by its type.
        other => return info(other, None, false),
    };
    inv(class, option).map(|mut i| {
        i.recipe_book = book && recipe_book_open(game);
        i
    })
}

/// The recipe book of the inventory, crafting table or furnaces is open.
fn recipe_book_open(game: &Game<'_>) -> bool {
    game.recipe_book_shift(f32::MAX) > 0.0
}

/// `Screen.isPauseScreen` for the screens drawn by the game's own `Screen`.
fn classify_screen(game: &Game<'_>, loading: bool) -> Option<ScreenInfo> {
    let s = |class: &str| format!("{MC}{class}");
    match game.screen {
        Screen::Playing if loading || game.remote_loading() => info(&s("LevelLoadingScreen"), Some(Vanilla::Disable), false),
        Screen::Playing => None,
        Screen::GameModeSwitcher => info(&s("debug.GameModeSwitcherScreen"), Some(Vanilla::GameModeSwitcher), false),
        Screen::EmoteWheel => info("horizon:EmoteWheelScreen", None, false),
        Screen::Paused => info(&s("PauseScreen"), None, true),
        Screen::Options => info(&s("options.OptionsScreen"), None, true),
        // `InBedChatScreen` is a `ChatScreen`.
        Screen::Chat => info(&s("ChatScreen"), Some(Vanilla::Disable), false),
        Screen::Creative => info(&s("inventory.CreativeModeInventoryScreen"), Some(Vanilla::Option("creative")), false),
        Screen::Inventory => menu_screen(game),
        Screen::Dead => info(&s("DeathScreen"), Some(Vanilla::Disable), false),
        Screen::Stats => info(&s("achievement.StatsScreen"), Some(Vanilla::Disable), true),
        Screen::Advancements => info(&s("advancements.AdvancementsScreen"), Some(Vanilla::Option("advancements")), true),
        Screen::WorldOptions => info(&s("WorldOptionsScreen"), None, true),
        Screen::Friends => info(&s("FriendsScreen"), None, true),
        Screen::Social => info(&s("social.SocialInteractionsScreen"), None, true),
        Screen::CommandBlock => info(&s("inventory.CommandBlockEditScreen"), Some(Vanilla::Disable), true),
        // The game master blocks' editors: structure and jigsaw blocks are
        // the module's (`StructureBlockEditScreen`, `JigsawBlockEditScreen`);
        // the test blocks' have a text field focused from the start.
        Screen::BlockEditor => info(&s("inventory.StructureBlockEditScreen"), Some(Vanilla::Disable), false),
        Screen::ServerLinks => info(&s("dialog.ServerLinksDialogScreen"), None, true),
        Screen::ConfirmLink => info(&s("ConfirmLinkScreen"), None, true),
        Screen::DialogWarning => info(&s("dialog.DialogScreen$WarningScreen"), None, true),
        Screen::Win => info(&s("WinScreen"), Some(Vanilla::Disable), true),
        Screen::DebugOptions => info(&s("debug.DebugOptionsScreen"), None, true),
        Screen::Mod => {
            let owner = game.native_mod_screen().unwrap_or("?unknown");
            info(&format!("{owner}:Screen"), None, true)
        }
        Screen::HudEditor => info("horizon:HudEditorScreen", None, true),
    }
}

/// The screen on top (`Minecraft.screen`), None while playing. `loading` is
/// the level loading screen up over the world.
pub(super) fn classify(game: &Game<'_>, loading: bool) -> Option<ScreenInfo> {
    if game.title.is_some() || game.player.is_none() {
        return None;
    }
    let s = |class: &str| format!("{MC}{class}");
    // The prompts first: they are over everything else.
    if game.command_prompt.is_some() {
        return info(&s("ConfirmScreen"), None, true);
    }
    if game.link_prompt.is_some() {
        return info(&s("ConfirmLinkScreen"), None, true);
    }
    if game.code_of_conduct.is_some() {
        return info(&s("multiplayer.CodeOfConductScreen"), None, true);
    }
    if game.server_packs.prompt.is_some() {
        return info("net.minecraft.client.multiplayer.ClientCommonPacketListenerImpl$PackConfirmScreen", None, true);
    }
    if game.dialog_open() {
        return info(&s("dialog.DialogScreen"), None, true);
    }
    // Signs (`AbstractSignEditScreen`: the module's rule, and its title
    // check) and books.
    if game.sign_edit.is_some() {
        return info(&s("inventory.SignEditScreen"), Some(Vanilla::Disable), false);
    }
    match &game.book {
        Some(BookScreen::Edit { .. }) => return info(&s("inventory.BookEditScreen"), Some(Vanilla::BookEdit), true),
        Some(BookScreen::View { lectern: Some(_), .. }) => {
            return info(&s("inventory.LecternScreen"), Some(Vanilla::Option("book")), true);
        }
        Some(BookScreen::View { .. }) => return info(&s("inventory.BookViewScreen"), Some(Vanilla::Option("book")), true),
        None => {}
    }
    classify_screen(game, loading)
}

// ---- the rules ---------------------------------------------------------------------------------

/// `Module.Movement` / `Module.Background` results folded: `Some(true)` to
/// allow movement or show the background.
type Suggestion = Option<bool>;

/// `VanillaModule.shouldAllowMovement` (`SUGGEST_*`; None is `PASS`).
fn vanilla_movement(c: &Config, screen: &ScreenInfo, text_field: bool) -> Suggestion {
    let sign = screen.id.ends_with("SignEditScreen");
    if sign {
        return Some(false);
    }
    if !c.recipe_book && screen.recipe_book {
        return Some(false);
    }
    // A focused text field. (The original asks only while the recipe book
    // option allows movement, so with it off typing in an anvil or the
    // creative search moved the player; here a text field always counts.)
    if c.text_field_disables && text_field {
        return Some(false);
    }
    match screen.vanilla? {
        Vanilla::Disable | Vanilla::BookEdit => Some(false),
        Vanilla::GameModeSwitcher => Some(true),
        Vanilla::Option(id) => Some(c.vanilla_move(id)),
    }
}

/// `VanillaModule.shouldHideBackground`: `Some(true)` shows it.
fn vanilla_background(c: &Config, screen: &ScreenInfo) -> Suggestion {
    if screen.id.ends_with("SignEditScreen") {
        return Some(true);
    }
    match screen.vanilla? {
        Vanilla::Disable => Some(true),
        Vanilla::BookEdit => Some(!c.vanilla_bg("book")),
        Vanilla::GameModeSwitcher => None,
        Vanilla::Option(id) => Some(!c.vanilla_bg(id)),
    }
}

/// An unrecognized screen's value, added with the default the first time it
/// is met (and the files saved).
fn unrecognized(c: &mut Config, id: &str, background: bool) -> bool {
    let (list, default) = if background {
        (&mut c.unrecognized_bg, c.bg_unrecognized_default)
    } else {
        (&mut c.unrecognized_move, c.move_unrecognized_default)
    };
    if let Some((_, v)) = list.iter().find(|(s, _)| s == id) {
        return *v;
    }
    list.push((id.to_owned(), default));
    cfg::save(c);
    default
}

/// `Minecraft.isPaused`: a single-player game paused by its screen. A
/// server's dialog pauses one too (`DialogScreen.isPauseScreen`, a dialog's
/// default), though Horizon keeps the world running under it.
fn paused(game: &Game<'_>) -> bool {
    game.pauses_game() || (game.dialog_open() && game.has_singleplayer_server() && !game.is_published())
}

/// `allowMovementInScreen`.
fn allow_movement(game: &Game<'_>, c: &mut Config, screen: &ScreenInfo, text_field: bool) -> bool {
    if game.player.is_none() || !c.enabled || !c.move_enabled || paused(game) {
        return false;
    }
    match vanilla_movement(c, screen, text_field) {
        Some(v) => v,
        None => unrecognized(c, &screen.id, false),
    }
}

/// `shouldDisableScreenBackground`.
fn hide_background(game: &Game<'_>, c: &mut Config, screen: &ScreenInfo) -> bool {
    if game.player.is_none() || !c.enabled || !c.bg_enabled {
        return false;
    }
    if screen.pause {
        match c.hide_on_pause {
            PauseMode::Show => return false,
            PauseMode::AllowHide => {}
            PauseMode::ShowSp => {
                if game.has_singleplayer_server() && !game.is_published() {
                    return false;
                }
            }
        }
    }
    match vanilla_background(c, screen) {
        Some(show) => !show,
        None => unrecognized(c, &screen.id, true),
    }
}

// ---- keys --------------------------------------------------------------------------------------

/// Every key mapping the game has (`KeyMapping.ALL`), in the Controls
/// screen's order: by category, then as the game lists them. InvMove's own
/// toggle key is last, in its category.
pub(super) fn mappings() -> Vec<&'static str> {
    let mut all: Vec<(usize, usize, &'static str)> = horizon_client::keybinds::all_mappings()
        .enumerate()
        .map(|(i, (name, _, category))| {
            let c = horizon_client::keybinds::CATEGORIES.iter().position(|x| x == category).unwrap_or(usize::MAX);
            (c, i, *name)
        })
        .collect();
    all.sort();
    let mut out: Vec<&'static str> = all.into_iter().map(|(_, _, n)| n).collect();
    out.push(TOGGLE_MAPPING);
    out
}

/// InvMove's toggle key as a mapping name (`keybind.invmove.toggleMove`).
pub(super) const TOGGLE_MAPPING: &str = "keybind.invmove.toggleMove";

/// A mapping the game has (the file keeps the others' values as they are).
pub(super) fn is_mapping(name: &str) -> bool {
    name == TOGGLE_MAPPING || horizon_client::keybinds::all_mappings().any(|(n, _, _)| *n == name)
}

/// `InputConstants.isKeyDown` for the mapping's key: a keyboard key held now.
/// Mouse buttons are not keys there, and an unfocused window holds none.
fn raw(game: &Game<'_>, mapping: &str) -> bool {
    horizon_client::keybinds::key_code(game.settings.keys.get(mapping)).is_some_and(|k| game.physical_keys.contains(&k))
}

/// The toggle key's key, held now.
fn toggle_key_raw(game: &Game<'_>) -> Option<bool> {
    let key = game.native_mod_key(cfg::ID, cfg::TOGGLE_KEY, cfg::UNBOUND);
    if key == cfg::UNBOUND {
        return None;
    }
    Some(horizon_client::keybinds::key_code(&key).is_some_and(|k| game.physical_keys.contains(&k)))
}

/// The movement mappings, in the order of `Keys`' fields.
const MOVEMENT: [&str; 7] = ["key.forward", "key.back", "key.left", "key.right", "key.jump", "key.sneak", "key.sprint"];

// ---- state -------------------------------------------------------------------------------------

#[derive(Default)]
struct State {
    /// `wasSneaking`: the input's sneak the last tick no screen was open.
    was_sneaking: bool,
    /// `wasToggleMovementPressed`.
    was_toggle_pressed: bool,
    /// `wasToggleKeyDown` for sneak and sprint (toggling): None until ticked.
    was_toggle_down: [Option<bool>; 2],
    /// Toggled sneak and sprint while a screen is open, from what they were
    /// when it opened: InvMove keeps them (`ToggleKeyMappingMixin`) instead of
    /// the game's release and restore.
    toggled: [bool; 2],
    /// Whether InvMove kept each of them this screen.
    kept: [bool; 2],
    /// A screen was open at the last tick.
    in_screen: bool,
    /// A mod's text field took typed characters (it cancelled them) on the
    /// open screen: it has the keyboard until a click or another screen.
    mod_typing: bool,
    /// The screen the last tick or frame saw.
    last_screen: Option<String>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

fn state<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|s| f(&mut s.borrow_mut()))
}

/// A character typed on the open screen was taken by a mod (cancelled).
pub(super) fn mod_typed() {
    state(|s| s.mod_typing = true);
}

/// A click, Escape, Enter or Tab: a mod's text field may have lost the focus.
pub(super) fn focus_moved() {
    state(|s| s.mod_typing = false);
}

/// The screen changed: no mod's text field has the focus yet.
fn screen_seen(screen: Option<&ScreenInfo>) {
    let id = screen.map(|s| s.id.clone());
    state(|s| {
        if s.last_screen != id {
            s.mod_typing = false;
            s.last_screen = id;
        }
    });
}

/// A text field has the keyboard: the game's own (`EditBox.canConsumeInput`
/// on the open screen), or a mod's on it.
fn text_field(game: &mut Game<'_>) -> bool {
    // Controls waiting for a key to bind takes the keyboard as a text field
    // does (in the original, pressing a key to bind it moved the player).
    let binding = game.screen == Screen::Options && game.ui.options.selected_key.is_some();
    game.text_field_focused() || binding || state(|s| s.mod_typing)
}

/// The mod applies: on, in the window's own game, played with the keyboard.
fn active(game: &Game<'_>) -> bool {
    game.window.is_some() && game.keyboard_mouse && game.native_on(cfg::ID)
}

/// `shouldSneak`.
fn should_sneak(st: &mut State, mode: SneakMode, shift_down: bool) -> bool {
    match mode {
        SneakMode::Off => false,
        SneakMode::MaintainWhilePressed => {
            if !shift_down {
                st.was_sneaking = false;
            }
            st.was_sneaking
        }
        SneakMode::Maintain => st.was_sneaking,
        SneakMode::Pressed => shift_down,
    }
}

/// `handleToggleMovementKey`: the toggle key flips "Move In Inventories",
/// and in a screen only when that changes whether the player can move there.
fn toggle_key(game: &Game<'_>, c: &mut Config, screen: Option<&ScreenInfo>, could_move: bool, text: bool) -> bool {
    let Some(down) = toggle_key_raw(game) else {
        return could_move;
    };
    let before = state(|s| std::mem::replace(&mut s.was_toggle_pressed, down));
    if !down || before {
        return could_move;
    }
    let Some(screen) = screen else {
        c.move_enabled = !c.move_enabled;
        return could_move;
    };
    if could_move && c.move_enabled {
        c.move_enabled = false;
        return false;
    }
    if !could_move && !c.move_enabled {
        c.move_enabled = true;
        if allow_movement(game, c, screen, text) {
            return true;
        }
        c.move_enabled = false;
        return false;
    }
    could_move
}

/// The local player's input tick (`onInputUpdate` on `LocalPlayer.aiStep`'s
/// input tick), at the start of the game's tick: the keys the player's input
/// reads this tick while a screen is open, none while playing (the game's own
/// then). While a screen lets the player move, the allowed movement keys are
/// as the keyboard holds them (`tickKeybinds`); in one that doesn't, only the
/// sneak key is kept, pressed or let go as the settings say.
pub(super) fn input_tick(game: &mut Game<'_>) {
    let keys = screen_keys(game);
    horizon_client::sprint::invmove_keys::set(keys);
}

fn screen_keys(game: &mut Game<'_>) -> Option<Keys> {
    if !active(game) || game.player.is_none() {
        state(|s| s.in_screen = false);
        return None;
    }
    let loading = game.remote.as_ref().is_some_and(|r| r.level_load.is_active());
    let screen = classify(game, loading);
    screen_seen(screen.as_ref());
    let toggles = [game.settings.toggle_crouch, game.settings.toggle_sprint];
    let Some(screen) = screen else {
        // Playing: the input as it is, and the toggled keys InvMove kept go
        // on as they are now, not as they were when the screen opened.
        let restore = state(|s| {
            let back = s.in_screen;
            s.in_screen = false;
            s.was_sneaking = game.keys.held(Action::Sneak);
            (back, s.kept, s.toggled)
        });
        if restore.0 {
            for slot in [SNEAK, SPRINT] {
                if restore.1[slot] && toggles[slot] && game.toggles.0[slot].is_down != restore.2[slot] {
                    game.toggles.0[slot].is_down = restore.2[slot];
                    game.sync_toggle(slot);
                }
            }
        }
        state(|s| s.toggled = [game.toggles.is_down(SNEAK), game.toggles.is_down(SPRINT)]);
        // The toggle key works while playing too.
        let mut c = cfg::get();
        toggle_key(game, &mut c, None, false, false);
        cfg::with(|now| now.move_enabled = c.move_enabled);
        return None;
    };
    state(|s| s.in_screen = true);
    let text = text_field(game);
    let mut c = cfg::get();
    let mut can_move = allow_movement(game, &mut c, &screen, text);
    can_move = toggle_key(game, &mut c, Some(&screen), can_move, text);
    let riding = game.riding.is_some();
    let flying = game.player.as_ref().is_some_and(|p| p.flying);
    // `ToggleKeyMappingMixin`: a toggled sneak or sprint stays while the
    // mod and movement are on and its key is allowed.
    let keep = |c: &mut Config, name: &str| c.enabled && c.move_enabled && c.allow_key(name);
    let kept = [toggles[SNEAK] && keep(&mut c, "key.sneak"), toggles[SPRINT] && keep(&mut c, "key.sprint")];
    let keys = if can_move {
        // `tickKeybinds`: each allowed movement key from the keyboard; a
        // toggling one flips on a press (sneak only in "Pressed" mode).
        let mut down = [false; 7];
        for (i, name) in MOVEMENT.iter().enumerate() {
            if !c.allow_key(name) {
                continue;
            }
            let held = raw(game, name);
            let slot = match *name {
                "key.sneak" => Some(SNEAK),
                "key.sprint" => Some(SPRINT),
                _ => None,
            };
            match slot {
                Some(slot) if toggles[slot] => {
                    let sneak_mode = c.sneak;
                    down[i] = state(|s| {
                        if let Some(was) = s.was_toggle_down[slot]
                            && !was
                            && held
                            && (slot == SPRINT || sneak_mode == SneakMode::Pressed)
                        {
                            s.toggled[slot] = !s.toggled[slot];
                        }
                        s.was_toggle_down[slot] = Some(held);
                        s.toggled[slot]
                    });
                }
                _ => down[i] = held,
            }
        }
        if !toggles[SNEAK] {
            let shift_down = down[5];
            down[5] = if riding {
                c.dismount && shift_down
            } else {
                let mode = if flying { c.sneak_flying } else { c.sneak };
                state(|s| should_sneak(s, mode, shift_down))
            };
        }
        Some(Keys {
            forward: down[0],
            backward: down[1],
            left: down[2],
            right: down[3],
            jump: down[4],
            shift: down[5],
            sprint: down[6],
            analog: None,
        })
    } else if c.enabled {
        // A screen that keeps the player still (the allowed keys let go):
        // only sneak (kept, pressed or let go) and a toggled sprint InvMove
        // kept.
        let shift = if toggles[SNEAK] {
            kept[SNEAK] && state(|s| s.toggled[SNEAK])
        } else if !riding {
            let shift_down = raw(game, "key.sneak");
            let mode = if flying { SneakMode::Off } else { c.sneak_disallowed };
            state(|s| should_sneak(s, mode, shift_down))
        } else {
            false
        };
        let sprint = kept[SPRINT] && state(|s| s.toggled[SPRINT]);
        Some(Keys { shift, sprint, ..Keys::default() })
    } else {
        None
    };
    state(|s| s.kept = kept);
    // What the tick changed: the toggle key's switch, the keys asked about,
    // the screens met.
    cfg::with(|now| {
        now.move_enabled = c.move_enabled;
        now.allowed_keys = c.allowed_keys;
        now.unrecognized_move = c.unrecognized_move;
    });
    keys
}

/// `ButtonJumpKeyFixMixin`: the jump key (Space) presses the focused button
/// of a screen (`AbstractButton.keyPressed`) only where the player can't
/// move; the merchant's trade offers still take it. True to keep the key
/// from the focused widget.
pub(super) fn jump_key_kept(game: &mut Game<'_>, key: winit::keyboard::KeyCode) -> bool {
    use winit::keyboard::KeyCode;
    if !active(game) || !matches!(key, KeyCode::Space | KeyCode::Enter | KeyCode::NumpadEnter) {
        return false;
    }
    let jump = horizon_client::keybinds::key_code(game.settings.keys.get("key.jump"));
    if jump != Some(key) {
        return false;
    }
    let mut c = cfg::get();
    if !c.enabled || !c.move_enabled || !c.allow_key("key.jump") {
        return false;
    }
    let Some(screen) = classify(game, false) else {
        return false;
    };
    if screen.id.ends_with("MerchantScreen") {
        return false;
    }
    let text = text_field(game);
    let allowed = allow_movement(game, &mut c, &screen, text);
    cfg::with(|now| now.unrecognized_move = c.unrecognized_move.clone());
    allowed
}

// ---- the background ----------------------------------------------------------------------------

thread_local! {
    /// This frame hides the open screen's background.
    static HIDE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Once a frame, before drawing: whether the open screen's background is
/// hidden (`BackgroundMixin`, asked as the screen draws its background).
fn background_frame(game: &Game<'_>) {
    let hide = if active(game) {
        let screen = classify(game, false);
        screen_seen(screen.as_ref());
        screen.is_some_and(|screen| {
            let mut c = cfg::get();
            let hide = hide_background(game, &mut c, &screen);
            cfg::with(|now| now.unrecognized_bg = c.unrecognized_bg.clone());
            hide
        })
    } else {
        false
    };
    HIDE.with(|h| h.set(hide));
    horizon_client::ui::invmove_background::set(hide);
}

/// The dark tint behind an in-game screen (`renderTransparentBackground`).
pub(super) fn transparent_background(canceled: &mut bool) {
    if HIDE.with(std::cell::Cell::get) {
        *canceled = true;
    }
}

/// The menu blur behind a screen (`extractBlurredBackground`, part of the
/// background InvMove hides).
pub(super) fn menu_blur(radius: &mut Option<f32>) {
    if HIDE.with(std::cell::Cell::get) {
        *radius = None;
    }
}

// ---- Options > Mods and Mod Menu ---------------------------------------------------------------

fn description(ru: bool) -> &'static str {
    if ru {
        "Позволяет двигаться, прыгать и приседать в открытых инвентарях и других экранах."
    } else {
        "Lets you move while in inventories."
    }
}

/// The mod's row in Options > Mods, with its toggle key (unbound at first).
pub(super) fn list(mods: &mut Vec<NativeMod>) {
    mods.push(NativeMod {
        id: cfg::ID.into(),
        title: format!("{} {}", cfg::NAME, cfg::VERSION),
        description: description(cfg::russian()).into(),
        keys: vec![[cfg::TOGGLE_KEY.into(), cfg::t(TOGGLE_MAPPING), cfg::UNBOUND.into()]],
    });
}

/// The mod's page: its first button opens the config screen.
pub(super) fn page(mod_id: &str, labels: &mut Vec<String>) {
    if mod_id == cfg::ID {
        labels.push(if cfg::russian() { "Настройки..." } else { "Configure..." }.into());
    }
}

/// A click on the page, or Mod Menu's Configure: the config screen.
pub(super) fn page_click(game: &Game<'_>, mod_id: &str, button: usize) {
    if mod_id == cfg::ID && (button == 0 || button == super::invmove_screen::MODMENU_CONFIGURE) {
        super::invmove_screen::open(game, button != 0);
    }
}

/// The original's icon, for Mod Menu.
const ICON: &[u8] = horizon_client::native_data!("invmove", "icon.png");

fn modmenu_json(ru: bool) -> String {
    let description = serde_json::to_string(description(ru)).unwrap_or_default();
    format!(
        r#"{{
  "schema": 1,
  "name": "{name}",
  "version": "{version}",
  "description": {description},
  "authors": ["PieKing1215"],
  "contributors": ["DeportedToHell"],
  "license": "LGPL-3.0-only",
  "links": {{
    "homepage": "https://modrinth.com/mod/invmove",
    "sources": "https://github.com/DeportedToHell/MCRE-Mods/tree/main/invmove",
    "issues": "https://github.com/DeportedToHell/MCRE-Mods/issues"
  }},
  "badges": ["client"],
  "config_screen": true
}}
"#,
        name = cfg::NAME,
        version = cfg::VERSION,
    )
}

/// The language Mod Menu's data was last written in (0 none, 1 English, 2 Russian).
static WROTE_DATA: AtomicU8 = AtomicU8::new(0);

/// Each frame: the background decision, and `icon.png` and `modmenu.json`
/// in the mod's data folder, once and again when the game's language turns
/// to or from Russian (each written only when it differs).
pub(super) fn frame(game: &mut Game<'_>) {
    background_frame(game);
    let ru = cfg::russian();
    if WROTE_DATA.swap(if ru { 2 } else { 1 }, Ordering::Relaxed) == if ru { 2 } else { 1 } {
        return;
    }
    let dir = game.native_data_dir(cfg::ID);
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let meta = modmenu_json(ru);
    for (name, bytes) in [("icon.png", ICON), ("modmenu.json", meta.as_bytes())] {
        let path = dir.join(name);
        if std::fs::read(&path).ok().as_deref() != Some(bytes) {
            let _ = std::fs::write(&path, bytes);
        }
    }
}
