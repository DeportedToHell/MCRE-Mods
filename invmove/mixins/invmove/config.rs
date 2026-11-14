// InvMove's settings (`InvMoveConfig`, `ModuleConfig`, the vanilla module's
// options) and its files, as the original keeps them in the game's `config/`
// folder: `invmove.json` (General, UI Movement and UI Background, and the
// Allowed Keys that differ from their defaults), `invmove/vanilla.json` (the
// vanilla module's per-screen options) and `invmove/unrecognized.json` (the
// screens no module knows, as they were met). Each file keeps what it held
// that the mod does not know, as Gson's read-modify-write does.
//
// Ported from InvMove by PieKing1215 (LGPL-3.0-only,
// https://github.com/PieKing1215/InvMove).
use serde_json::{Map, Value};
use std::cell::RefCell;
use std::path::PathBuf;

pub(super) const ID: &str = "invmove";
pub(super) const NAME: &str = "InvMove";
/// This port's version; the original it follows is InvMove 0.9.6.
pub(super) const VERSION: &str = "1.0.1";
/// The toggle key's name among the mod's keys (`keybind.invmove.toggleMove`),
/// unbound at first.
pub(super) const TOGGLE_KEY: &str = "toggleMove";
pub(super) const UNBOUND: &str = "key.keyboard.unknown";

/// `InvMoveConfig.Movement.SneakMode`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum SneakMode {
    Off,
    Maintain,
    Pressed,
    MaintainWhilePressed,
}

impl SneakMode {
    pub(super) const ALL: [SneakMode; 4] = [Self::Off, Self::Maintain, Self::Pressed, Self::MaintainWhilePressed];
    /// The enum constant's name, as the file and the selector show it.
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Maintain => "Maintain",
            Self::Pressed => "Pressed",
            Self::MaintainWhilePressed => "MaintainWhilePressed",
        }
    }
    fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.name() == s)
    }
}

/// `InvMoveConfig.Background.PauseScreenMode`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum PauseMode {
    Show,
    ShowSp,
    AllowHide,
}

impl PauseMode {
    pub(super) const ALL: [PauseMode; 3] = [Self::Show, Self::ShowSp, Self::AllowHide];
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Show => "Show",
            Self::ShowSp => "ShowSP",
            Self::AllowHide => "AllowHide",
        }
    }
    fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.name() == s)
    }
}

/// The vanilla module's per-screen options (`VanillaModule`), in its order:
/// the option's id and the translation key it is shown with (`display`),
/// none for those shown by their own key
/// (`config.invmove.vanilla.movement.<id>`). Every one is also a background
/// option, the recipe book excepted.
pub(super) const VANILLA: [(&str, Option<&str>); 23] = [
    ("inventory", Some("container.inventory")),
    ("horseInventory", None),
    ("creative", Some("key.category.minecraft.creative")),
    ("crafting", Some("block.minecraft.crafting_table")),
    ("chest", Some("container.chest")),
    ("shulker", Some("container.shulkerBox")),
    ("dispenser", Some("container.dispenser")),
    ("hopper", Some("container.hopper")),
    ("enchantment", Some("block.minecraft.enchanting_table")),
    ("anvil", Some("block.minecraft.anvil")),
    ("beacon", Some("container.beacon")),
    ("brewing", Some("container.brewing")),
    ("furnace", Some("container.furnace")),
    ("blastFurnace", Some("container.blast_furnace")),
    ("smoker", Some("container.smoker")),
    ("loom", Some("container.loom")),
    ("cartography", Some("container.cartography_table")),
    ("grindstone", Some("block.minecraft.grindstone")),
    ("stonecutter", Some("container.stonecutter")),
    ("smithing", Some("block.minecraft.smithing_table")),
    ("villager", Some("entity.minecraft.villager")),
    ("book", Some("item.minecraft.book")),
    ("advancements", Some("gui.advancements")),
];

/// The recipe book's movement option: shown as "Recipe Book".
pub(super) const RECIPE_BOOK: &str = "recipeBook";

#[derive(Clone, Debug)]
pub(super) struct Config {
    // General.
    pub(super) enabled: bool,
    pub(super) debug_display: bool,
    // UI Movement.
    pub(super) move_enabled: bool,
    /// The old "Allow Jumping" (hidden; moved into the Allowed Keys).
    pub(super) jump: bool,
    pub(super) sneak: SneakMode,
    pub(super) sneak_disallowed: SneakMode,
    pub(super) sneak_flying: SneakMode,
    pub(super) dismount: bool,
    pub(super) text_field_disables: bool,
    pub(super) move_unrecognized_default: bool,
    /// `allowedKeys`: every key asked about, with its value (the default
    /// until the player changes it), by mapping name.
    pub(super) allowed_keys: Vec<(String, bool)>,
    // UI Background.
    pub(super) bg_enabled: bool,
    pub(super) hide_on_pause: PauseMode,
    pub(super) bg_unrecognized_default: bool,
    // The vanilla module.
    pub(super) recipe_book: bool,
    pub(super) vanilla_move: Vec<bool>,
    pub(super) vanilla_bg: Vec<bool>,
    /// The unrecognized screens met, by their class (the screen's name for
    /// Horizon), in the order they were met: whether one allows movement,
    /// whether one hides its background.
    pub(super) unrecognized_move: Vec<(String, bool)>,
    pub(super) unrecognized_bg: Vec<(String, bool)>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            enabled: true,
            debug_display: false,
            move_enabled: true,
            jump: true,
            sneak: SneakMode::Maintain,
            sneak_disallowed: SneakMode::Maintain,
            sneak_flying: SneakMode::Pressed,
            dismount: false,
            text_field_disables: true,
            move_unrecognized_default: true,
            allowed_keys: Vec::new(),
            bg_enabled: true,
            hide_on_pause: PauseMode::Show,
            bg_unrecognized_default: true,
            recipe_book: true,
            vanilla_move: vec![true; VANILLA.len()],
            vanilla_bg: vec![true; VANILLA.len()],
            unrecognized_move: Vec::new(),
            unrecognized_bg: Vec::new(),
        }
    }
}

/// `VanillaModule.allowKeyDefault`: the movement keys are handled, nothing
/// else unless the player says so.
pub(super) fn allow_key_default(name: &str) -> bool {
    matches!(
        name,
        "key.forward" | "key.back" | "key.left" | "key.right" | "key.jump" | "key.sneak" | "key.sprint"
    )
}

impl Config {
    pub(super) fn vanilla_index(id: &str) -> Option<usize> {
        VANILLA.iter().position(|(v, _)| *v == id)
    }
    pub(super) fn vanilla_move(&self, id: &str) -> bool {
        Self::vanilla_index(id).and_then(|i| self.vanilla_move.get(i).copied()).unwrap_or(true)
    }
    pub(super) fn vanilla_bg(&self, id: &str) -> bool {
        Self::vanilla_index(id).and_then(|i| self.vanilla_bg.get(i).copied()).unwrap_or(true)
    }
    /// `InvMove.allowKey`: the key's value, its default kept from the first
    /// time it is asked about.
    pub(super) fn allow_key(&mut self, name: &str) -> bool {
        if let Some((_, v)) = self.allowed_keys.iter().find(|(k, _)| k == name) {
            return *v;
        }
        let v = allow_key_default(name);
        self.allowed_keys.push((name.to_owned(), v));
        v
    }
    /// The key's value without remembering it (the config screen).
    pub(super) fn allowed_key(&self, name: &str) -> bool {
        self.allowed_keys
            .iter()
            .find(|(k, _)| k == name)
            .map_or_else(|| allow_key_default(name), |(_, v)| *v)
    }
    pub(super) fn set_allowed_key(&mut self, name: &str, v: bool) {
        match self.allowed_keys.iter_mut().find(|(k, _)| k == name) {
            Some(e) => e.1 = v,
            None => self.allowed_keys.push((name.to_owned(), v)),
        }
    }
}

// ---- the files -------------------------------------------------------------------------------

fn config_dir() -> Option<PathBuf> {
    super::Game::native_config_dir().ok()
}

/// A file's JSON object: an empty one when there is no file, it is empty or
/// it isn't an object.
fn read_object(path: &PathBuf) -> Map<String, Value> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .and_then(|v| match v {
            Value::Object(m) => Some(m),
            _ => None,
        })
        .unwrap_or_default()
}

/// Gson's two-space pretty printing.
fn write_object(path: &PathBuf, json: Map<String, Value>) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut text) = serde_json::to_string_pretty(&Value::Object(json)) {
        text.push('\n');
        if std::fs::read_to_string(path).ok().as_deref() != Some(text.as_str()) {
            let _ = std::fs::write(path, text);
        }
    }
}

/// `getOrPutJsonObject`.
fn object<'a>(parent: &'a mut Map<String, Value>, id: &str) -> &'a mut Map<String, Value> {
    if !parent.get(id).is_some_and(Value::is_object) {
        parent.insert(id.to_owned(), Value::Object(Map::new()));
    }
    match parent.get_mut(id) {
        Some(Value::Object(m)) => m,
        _ => unreachable!("just inserted"),
    }
}

fn read_bool(obj: Option<&Map<String, Value>>, id: &str, into: &mut bool) {
    if let Some(Value::Bool(b)) = obj.and_then(|o| o.get(id)) {
        *into = *b;
    }
}

fn read_sneak(obj: Option<&Map<String, Value>>, id: &str, into: &mut SneakMode, migrate: bool) {
    match obj.and_then(|o| o.get(id)) {
        // The old boolean sneak option: on was "Pressed", off "Maintain".
        Some(Value::Bool(b)) if migrate => *into = if *b { SneakMode::Pressed } else { SneakMode::Maintain },
        Some(Value::String(s)) => {
            if let Some(m) = SneakMode::parse(s) {
                *into = m;
            }
        }
        _ => {}
    }
}

fn as_object<'a>(json: &'a Map<String, Value>, id: &str) -> Option<&'a Map<String, Value>> {
    json.get(id).and_then(Value::as_object)
}

impl Config {
    fn read_main(&mut self, json: &Map<String, Value>) {
        let general = as_object(json, "general");
        read_bool(general, "enable", &mut self.enabled);
        read_bool(general, "debugDisplay", &mut self.debug_display);
        let movement = as_object(json, "movement");
        read_bool(movement, "enable", &mut self.move_enabled);
        read_bool(movement, "jump", &mut self.jump);
        read_sneak(movement, "sneak", &mut self.sneak, true);
        read_sneak(movement, "sneak_disallowed", &mut self.sneak_disallowed, false);
        read_sneak(movement, "sneak_flying", &mut self.sneak_flying, false);
        read_bool(movement, "dismount", &mut self.dismount);
        read_bool(movement, "textFieldDisables", &mut self.text_field_disables);
        read_bool(movement, "unrecognizedScreenDefault", &mut self.move_unrecognized_default);
        let background = as_object(json, "background");
        read_bool(background, "enable", &mut self.bg_enabled);
        if let Some(Value::String(s)) = background.and_then(|o| o.get("hideOnPause"))
            && let Some(m) = PauseMode::parse(s)
        {
            self.hide_on_pause = m;
        }
        read_bool(background, "unrecognizedScreenDefault", &mut self.bg_unrecognized_default);
        if let Some(keys) = as_object(json, "allowedKeysOverrides") {
            for (k, v) in keys {
                if let Some(b) = v.as_bool() {
                    self.set_allowed_key(k, b);
                }
            }
        }
        // The old "Allow Jumping" (which did nothing) becomes the jump key's
        // value, once.
        if !self.allowed_keys.iter().any(|(k, _)| k == "key.jump") && !self.jump {
            self.set_allowed_key("key.jump", false);
            self.jump = true;
        }
    }

    fn write_main(&self, json: &mut Map<String, Value>) {
        let general = object(json, "general");
        general.insert("enable".into(), self.enabled.into());
        general.insert("debugDisplay".into(), self.debug_display.into());
        let movement = object(json, "movement");
        movement.insert("enable".into(), self.move_enabled.into());
        movement.insert("jump".into(), self.jump.into());
        movement.insert("sneak".into(), self.sneak.name().into());
        movement.insert("sneak_disallowed".into(), self.sneak_disallowed.name().into());
        movement.insert("sneak_flying".into(), self.sneak_flying.name().into());
        movement.insert("dismount".into(), self.dismount.into());
        movement.insert("textFieldDisables".into(), self.text_field_disables.into());
        movement.insert("unrecognizedScreenDefault".into(), self.move_unrecognized_default.into());
        let background = object(json, "background");
        background.insert("enable".into(), self.bg_enabled.into());
        background.insert("hideOnPause".into(), self.hide_on_pause.name().into());
        background.insert("unrecognizedScreenDefault".into(), self.bg_unrecognized_default.into());
        // Only the keys that differ from their defaults; a key the game
        // doesn't have (a mod that is off) keeps its value.
        let mut keys = Map::new();
        for (k, v) in &self.allowed_keys {
            if !super::invmove_glue::is_mapping(k) || *v != allow_key_default(k) {
                keys.insert(k.clone(), (*v).into());
            }
        }
        json.insert("allowedKeysOverrides".into(), Value::Object(keys));
    }

    fn read_vanilla(&mut self, json: &Map<String, Value>) {
        let movement = as_object(json, "movement");
        read_bool(movement, RECIPE_BOOK, &mut self.recipe_book);
        let background = as_object(json, "backgroundHide");
        for (i, (id, _)) in VANILLA.iter().enumerate() {
            read_bool(movement, id, &mut self.vanilla_move[i]);
            read_bool(background, id, &mut self.vanilla_bg[i]);
        }
    }

    fn write_vanilla(&self, json: &mut Map<String, Value>) {
        let movement = object(json, "movement");
        movement.insert(RECIPE_BOOK.into(), self.recipe_book.into());
        for (i, (id, _)) in VANILLA.iter().enumerate() {
            movement.insert((*id).into(), self.vanilla_move[i].into());
        }
        let background = object(json, "backgroundHide");
        for (i, (id, _)) in VANILLA.iter().enumerate() {
            background.insert((*id).into(), self.vanilla_bg[i].into());
        }
    }

    fn read_unrecognized(&mut self, json: &Map<String, Value>) {
        for (key, list) in [("allowMovement", &mut self.unrecognized_move), ("hideBackground", &mut self.unrecognized_bg)] {
            if let Some(obj) = as_object(json, key) {
                for (screen, v) in obj {
                    // Only the screens this game has (`Class.forName`).
                    if let Some(b) = v.as_bool()
                        && super::invmove_glue::known_screen(screen)
                        && !list.iter().any(|(s, _)| s == screen)
                    {
                        list.push((screen.clone(), b));
                    }
                }
            }
        }
    }

    fn write_unrecognized(&self, json: &mut Map<String, Value>) {
        for (key, list) in [("allowMovement", &self.unrecognized_move), ("hideBackground", &self.unrecognized_bg)] {
            let obj = object(json, key);
            for (screen, v) in list {
                obj.insert(screen.clone(), (*v).into());
            }
        }
    }
}

/// The three files' paths.
fn paths() -> Option<[PathBuf; 3]> {
    let dir = config_dir()?;
    Some([
        dir.join("invmove.json"),
        dir.join("invmove").join("vanilla.json"),
        dir.join("invmove").join("unrecognized.json"),
    ])
}

/// `InvMoveConfig.load`: each file read (written first when there is none).
fn load() -> Config {
    let mut c = Config::default();
    let Some([main, vanilla, unrecognized]) = paths() else {
        return c;
    };
    if main.exists() {
        c.read_main(&read_object(&main));
    } else {
        let mut json = Map::new();
        c.write_main(&mut json);
        write_object(&main, json);
    }
    if vanilla.exists() {
        c.read_vanilla(&read_object(&vanilla));
    } else {
        let mut json = Map::new();
        c.write_vanilla(&mut json);
        write_object(&vanilla, json);
    }
    if unrecognized.exists() {
        c.read_unrecognized(&read_object(&unrecognized));
    } else {
        let mut json = Map::new();
        c.write_unrecognized(&mut json);
        write_object(&unrecognized, json);
    }
    c
}

/// `InvMoveConfig.save`: every file, over what each holds.
pub(super) fn save(c: &Config) {
    let Some([main, vanilla, unrecognized]) = paths() else {
        return;
    };
    let mut json = read_object(&main);
    c.write_main(&mut json);
    write_object(&main, json);
    let mut json = read_object(&vanilla);
    c.write_vanilla(&mut json);
    write_object(&vanilla, json);
    let mut json = read_object(&unrecognized);
    c.write_unrecognized(&mut json);
    write_object(&unrecognized, json);
}

thread_local! {
    static CONFIG: RefCell<Option<Config>> = const { RefCell::new(None) };
}

/// The settings, read from the files the first time.
pub(super) fn with<R>(f: impl FnOnce(&mut Config) -> R) -> R {
    CONFIG.with(|c| {
        let mut c = c.borrow_mut();
        let c = c.get_or_insert_with(load);
        f(c)
    })
}

pub(super) fn get() -> Config {
    with(|c| c.clone())
}

/// The settings changed (the config screen saved): kept and written.
pub(super) fn replace(new: Config) {
    save(&new);
    with(|c| *c = new);
}

// ---- texts -----------------------------------------------------------------------------------

pub(super) fn russian() -> bool {
    horizon_client::lang::code().starts_with("ru")
}

/// InvMove's `en_us.json`.
fn english(key: &str) -> Option<&'static str> {
    Some(match key {
        "config.invmove.title" => "InvMove Config",
        "key.invmove.category.general" => "General",
        "key.invmove.category.movement" => "UI Movement",
        "key.invmove.category.background" => "UI Background",
        "config.invmove.general.enable" => "Enable Mod",
        "tooltip.config.invmove.general.enable" => "Enable the entire mod",
        "config.invmove.general.debugDisplay" => "Debug Display",
        "tooltip.config.invmove.general.debugDisplay" => "Enable debug display.\n(not supported on 26.x yet)",
        "config.invmove.movement.enable" => "Move In Inventories",
        "tooltip.config.invmove.movement.enable" => {
            "Enable movement while inside inventories.\nYou can manually turn off certain inventories below.\nThere is a keybind you can set to toggle this."
        }
        "config.invmove.movement.sneak" => "Sneak Mode",
        "tooltip.config.invmove.movement.sneak" => {
            "How to handle sneaking while inside inventories:\nOff = No sneaking\nMaintain = Keep sneaking if you were when the inventory opened\nPressed = Sneak only while holding the sneak button\nMaintainWhilePressed = Maintain until the sneak button is released\n(Pressed can be distracting when shift-clicking)\n(If using toggle sneak, Pressed allows toggling and both other modes maintain)"
        }
        "config.invmove.movement.sneak_disallowed" => "Sneak Mode When Disallowed",
        "tooltip.config.invmove.movement.sneak_disallowed" => {
            "Determines sneaking behavior inside inventories that have movement disabled.\nOff = Automatically unsneak (Vanilla behavior)\nMaintain = Keep sneaking if you were when you opened the inventory\nPressed = Sneak only while holding the sneak button\nMaintainWhilePressed = Maintain until the sneak button is released\n(Pressed can be distracting when shift-clicking)\n(If using toggle sneak, Pressed allows toggling and both other modes maintain)"
        }
        "config.invmove.movement.sneak_flying" => "Sneak Mode When Creative Flying",
        "tooltip.config.invmove.movement.sneak_flying" => {
            "Determines sneaking behavior inside inventories that have movement enabled while creative flying.\nOff = Automatically unsneak (Vanilla behavior)\nMaintain = Keep sneaking if you were when you opened the inventory\nPressed = Sneak only while holding the sneak button\nMaintainWhilePressed = Maintain until the sneak button is released\n(Pressed can be distracting when shift-clicking)\n(If using toggle sneak, Pressed allows toggling and both other modes maintain)"
        }
        "config.invmove.movement.jump" => "Allow Jumping",
        "tooltip.config.invmove.movement.jump" => "Allow jumping while inside inventories.",
        "config.invmove.movement.dismount" => "Allow Dismounting",
        "tooltip.config.invmove.movement.dismount" => "Allow dismounting while inside inventories.",
        "config.invmove.movement.textFieldDisables" => "Text Field Disables Movement",
        "tooltip.config.invmove.movement.textFieldDisables" => "Disable movement in inventories when a text field is focused.",
        "config.invmove.movement.unrecognizedScreenDefault" => "Unrecognized Screens Default",
        "tooltip.config.invmove.movement.unrecognizedScreenDefault" => "Default state for new unrecognized UIs.",
        "config.invmove.background.enable" => "Hide Inventory Backgrounds",
        "tooltip.config.invmove.background.enable" => {
            "Hides the background tint while inside inventories.\nYou can manually turn off certain inventories below."
        }
        "config.invmove.background.hideOnPause" => "Pause Screens Background",
        "tooltip.config.invmove.background.hideOnPause" => {
            "How to handle screens that pause the game:\nShow = Always show background\nShowSP = Show background in singleplayer, otherwise allow hide\nAllowHide = Allow background to be hidden"
        }
        "config.invmove.background.unrecognizedScreenDefault" => "Unrecognized Screens Default",
        "tooltip.config.invmove.background.unrecognizedScreenDefault" => "Default state for new unrecognized UIs.",
        "key.invmove.category.movement.keys" => "Allowed Keys",
        "key.invmove.category.movement.keys.desc" => "Only keys marked \"Allow\" will be handled by the mod.",
        "key.invmove.module.nooptions" => "This module doesn't have any options.",
        "key.invmove.unrecognized" => "Unrecognized UI Types",
        "key.invmove.unrecognized.desc" => "Unrecognized GUIs show up here once they have been interacted with.",
        "key.invmove.unrecognized.nomod" => "Unknown",
        "key.category.invmove.main" => "InvMove",
        "keybind.invmove.toggleMove" => "Toggle UI Movement",
        "key.invmove.modules" => "Modules",
        "key.invmove.module.vanilla" => "Vanilla",
        "config.invmove.vanilla.movement.horseInventory" => "Horse Inventory",
        "config.invmove.vanilla.backgroundHide.horseInventory" => "Horse Inventory",
        "tooltip.gui.advancements" => "This menu is a pause screen so this option may be overridden.",
        "tooltip.item.minecraft.book" => "This menu is a pause screen so this option may be overridden.",
        // `InvMoveConfig`'s Yes/No texts (literal in the original).
        "invmove.movement.yes" => "§aAllow Movement",
        "invmove.movement.no" => "§cDisallow Movement",
        "invmove.key.yes" => "§aAllow",
        "invmove.key.no" => "§cIgnore",
        "invmove.background.yes" => "§aHide Background",
        "invmove.background.no" => "§cShow Background",
        // The recipe book option is shown by this name (a literal in the original).
        "Recipe Book" => "Recipe Book",
        _ => return None,
    })
}

/// The Russian texts (this port's own: the original has English only).
fn russian_text(key: &str) -> Option<&'static str> {
    Some(match key {
        "config.invmove.title" => "Настройки InvMove",
        "key.invmove.category.general" => "Общие",
        "key.invmove.category.movement" => "Движение в интерфейсе",
        "key.invmove.category.background" => "Фон интерфейса",
        "config.invmove.general.enable" => "Включить мод",
        "tooltip.config.invmove.general.enable" => "Включить весь мод",
        "config.invmove.general.debugDisplay" => "Отладочный экран",
        "tooltip.config.invmove.general.debugDisplay" => "Включить отладочный экран.\n(пока не поддерживается на 26.x)",
        "config.invmove.movement.enable" => "Движение в инвентарях",
        "tooltip.config.invmove.movement.enable" => {
            "Разрешить движение, пока открыт инвентарь.\nОтдельные инвентари можно отключить ниже.\nДля переключения можно назначить клавишу."
        }
        "config.invmove.movement.sneak" => "Режим приседания",
        "tooltip.config.invmove.movement.sneak" => {
            "Как приседать, пока открыт инвентарь:\nOff = Не приседать\nMaintain = Остаться присевшим, если вы приседали, когда открыли инвентарь\nPressed = Приседать, только пока зажата клавиша приседания\nMaintainWhilePressed = Оставаться присевшим, пока не отпущена клавиша приседания\n(Pressed может мешать при Shift-клике)\n(При переключаемом приседании Pressed позволяет переключать, а оба других режима сохраняют)"
        }
        "config.invmove.movement.sneak_disallowed" => "Приседание, где движение запрещено",
        "tooltip.config.invmove.movement.sneak_disallowed" => {
            "Как приседать в инвентарях, где движение отключено.\nOff = Встать автоматически (как в ванилле)\nMaintain = Остаться присевшим, если вы приседали, когда открыли инвентарь\nPressed = Приседать, только пока зажата клавиша приседания\nMaintainWhilePressed = Оставаться присевшим, пока не отпущена клавиша приседания\n(Pressed может мешать при Shift-клике)\n(При переключаемом приседании Pressed позволяет переключать, а оба других режима сохраняют)"
        }
        "config.invmove.movement.sneak_flying" => "Приседание в творческом полёте",
        "tooltip.config.invmove.movement.sneak_flying" => {
            "Как приседать в инвентарях с разрешённым движением во время творческого полёта.\nOff = Встать автоматически (как в ванилле)\nMaintain = Остаться присевшим, если вы приседали, когда открыли инвентарь\nPressed = Приседать, только пока зажата клавиша приседания\nMaintainWhilePressed = Оставаться присевшим, пока не отпущена клавиша приседания\n(Pressed может мешать при Shift-клике)\n(При переключаемом приседании Pressed позволяет переключать, а оба других режима сохраняют)"
        }
        "config.invmove.movement.jump" => "Разрешить прыжки",
        "tooltip.config.invmove.movement.jump" => "Разрешить прыгать, пока открыт инвентарь.",
        "config.invmove.movement.dismount" => "Разрешить спешиваться",
        "tooltip.config.invmove.movement.dismount" => "Разрешить спешиваться, пока открыт инвентарь.",
        "config.invmove.movement.textFieldDisables" => "Текстовое поле отключает движение",
        "tooltip.config.invmove.movement.textFieldDisables" => "Отключать движение в инвентарях, когда активно текстовое поле.",
        "config.invmove.movement.unrecognizedScreenDefault" => "Неизвестные экраны по умолчанию",
        "tooltip.config.invmove.movement.unrecognizedScreenDefault" => "Начальное значение для новых неизвестных экранов.",
        "config.invmove.background.enable" => "Скрывать фон инвентарей",
        "tooltip.config.invmove.background.enable" => {
            "Убирает затемнение фона, пока открыт инвентарь.\nОтдельные инвентари можно отключить ниже."
        }
        "config.invmove.background.hideOnPause" => "Фон экранов паузы",
        "tooltip.config.invmove.background.hideOnPause" => {
            "Как быть с экранами, которые ставят игру на паузу:\nShow = Всегда показывать фон\nShowSP = Показывать фон в одиночной игре, иначе можно скрыть\nAllowHide = Фон можно скрыть"
        }
        "config.invmove.background.unrecognizedScreenDefault" => "Неизвестные экраны по умолчанию",
        "tooltip.config.invmove.background.unrecognizedScreenDefault" => "Начальное значение для новых неизвестных экранов.",
        "key.invmove.category.movement.keys" => "Разрешённые клавиши",
        "key.invmove.category.movement.keys.desc" => "Мод обрабатывает только клавиши с отметкой «Разрешить».",
        "key.invmove.module.nooptions" => "У этого модуля нет настроек.",
        "key.invmove.unrecognized" => "Неизвестные экраны",
        "key.invmove.unrecognized.desc" => "Неизвестные экраны появятся здесь, когда вы их откроете.",
        "key.invmove.unrecognized.nomod" => "Неизвестно",
        "key.category.invmove.main" => "InvMove",
        "keybind.invmove.toggleMove" => "Переключить движение в интерфейсе",
        "key.invmove.modules" => "Модули",
        "key.invmove.module.vanilla" => "Ванилла",
        "config.invmove.vanilla.movement.horseInventory" => "Инвентарь лошади",
        "config.invmove.vanilla.backgroundHide.horseInventory" => "Инвентарь лошади",
        "tooltip.gui.advancements" => "Это меню ставит игру на паузу, поэтому настройка может не действовать.",
        "tooltip.item.minecraft.book" => "Это меню ставит игру на паузу, поэтому настройка может не действовать.",
        "invmove.movement.yes" => "§aДвижение: да",
        "invmove.movement.no" => "§cДвижение: нет",
        "invmove.key.yes" => "§aРазрешить",
        "invmove.key.no" => "§cИгнорировать",
        "invmove.background.yes" => "§aСкрывать фон",
        "invmove.background.no" => "§cПоказывать фон",
        "Recipe Book" => "Книга рецептов",
        _ => return None,
    })
}

/// Whether a translation exists (`Language.has`): the mod's, or the game's.
pub(super) fn has(key: &str) -> bool {
    english(key).is_some() || horizon_client::lang::has(key)
}

/// `Component.translatable(key)`: the mod's text in the game's language (the
/// English one where the Russian is missing), else the game's own, else the
/// key itself.
pub(super) fn t(key: &str) -> String {
    let own = if russian() { russian_text(key).or_else(|| english(key)) } else { english(key) };
    match own {
        Some(s) => s.to_owned(),
        None if horizon_client::lang::has(key) => horizon_client::lang::get(key),
        None => key.to_owned(),
    }
}
