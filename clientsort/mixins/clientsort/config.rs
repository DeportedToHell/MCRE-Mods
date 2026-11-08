// ClientSort's config screen (`ClothScreenProvider`): Cloth Config's screen
// with ClientSort's tabs and the options this port has, opened by Mod Menu's
// Configure (`MODMENU_CONFIGURE`) and by Configure... on the mod's page in
// Options > Mods. While it is up it covers the options; Save & Quit keeps the
// values where the mod keeps them (its settings, its keys).
use super::clientsort as cs;
use super::clientsort_cloth::{Category, Entry, Request, Screen as Cloth, Value};
use super::{Game, Screen};
use horizon_client::clipboard::Clipboard as _;
use horizon_client::hud::HudBatch;
use horizon_client::mod_api::NativeScreenView;
use horizon_client::options_ui::Page;
use std::cell::RefCell;

/// Mod Menu's Configure: `native_settings_click(id, MODMENU_CONFIGURE)` after
/// it opened the mod's page.
pub(super) const MODMENU_CONFIGURE: usize = 0x4D4D_4346;

/// The original's icon (`assets/clientsort/icon.png`), for Mod Menu.
const ICON: &[u8] = horizon_client::native_data!("clientsort", "icon.png");

struct State {
    screen: Option<Cloth>,
    /// Opened by Mod Menu: closing leaves the options too.
    modmenu: bool,
    /// The mod's row in Options > Mods.
    row: usize,
    data_written: bool,
}

thread_local! {
    static STATE: RefCell<State> = const {
        RefCell::new(State { screen: None, modmenu: false, row: 0, data_written: false })
    };
}

fn with<R>(f: impl FnOnce(&mut State) -> R) -> Option<R> {
    STATE.with(|s| s.try_borrow_mut().ok().map(|mut s| f(&mut s)))
}

fn russian() -> bool {
    horizon_client::lang::code().starts_with("ru")
}

/// Mod Menu's `modmenu.configure`, the page's first button.
pub(super) fn configure_label() -> &'static str {
    if russian() { "Настройки..." } else { "Configure..." }
}

/// ClientSort's `en_us.json` and `ru_ru.json`.
fn t(ru: bool, key: &str) -> &'static str {
    let (en, rus) = match key {
        "name" => ("ClientSort", "ClientSort"),
        "general" => ("General", "Общие"),
        "interactionInterval" => ("Interaction Interval", "Интервал взаимодействия"),
        "interactionInterval.tooltip" => (
            "The minimum time, in milliseconds, between interaction events when server acceleration is not in use.",
            "Минимальное время между событиями взаимодействия без ускорения сервера, в миллисекундах.",
        ),
        "useServerAcceleration" => ("Use Server Acceleration", "Использовать ускорение сервера"),
        "useServerAcceleration.tooltip" => (
            "Whether to perform operations instantly in singleplayer and when the mod is installed on the server.",
            "Выполнять операции мгновенно в одиночной игре или если мод установлен на сервере.",
        ),
        "hotbarScope" => ("Hotbar Scope", "Область панели быстрого доступа"),
        "transferReverseOrder" => ("Transfer Reverse Order", "Обратный порядок переноса"),
        "transferReverseOrder.tooltip" => (
            "Whether to scan the source inventory backwards in transfer operations.",
            "Сканировать исходный инвентарь в обратном порядке при операциях переноса.",
        ),
        "sorting" => ("Sorting", "Сортировка"),
        "sortOrder" => ("Default Sort Order", "Режим сортировки"),
        "shiftSortOrder" => ("Shift Sort Order", "Режим сортировки по Shift"),
        "ctrlSortOrder" => ("Ctrl Sort Order", "Режим сортировки по Ctrl"),
        "altSortOrder" => ("Alt Sort Order", "Режим сортировки по Alt"),
        "orderOverrides" => ("Sort Order Overrides", "Переопределение порядка сортировки"),
        "useStartOverrides" => ("Use Start Overrides", "Закрепить начало списка"),
        "useEndOverrides" => ("Use End Overrides", "Закрепить конец списка"),
        "useOverrides.tooltip" => (
            "If disabled, the list below will be ignored.",
            "Если отключено, приведённый ниже список будет проигнорирован.",
        ),
        "startOverrideItems" => ("Start Items", "Начальные предметы"),
        "startOverrideItems.tooltip" => (
            "When sorting, items in this list will always be sorted to the start.",
            "При сортировке предметы из этого списка всегда будут помещены в начало.",
        ),
        "endOverrideItems" => ("End Items", "Конечные предметы"),
        "endOverrideItems.tooltip" => (
            "When sorting, items in this list will always be sorted to the end.",
            "При сортировке предметы из этого списка всегда будут помещены в конец.",
        ),
        "overrideItems.tooltip.2" => (
            "Accepts item identifiers such as '%s'.",
            "Принимает идентификаторы элементов, например '%s'.",
        ),
        "matching" => ("Matching", "Сопоставление"),
        "alwaysMatchByType" => ("Always Match by Type", "Всегда сопоставлять по типу"),
        "alwaysMatchByType.tooltip" => (
            "Item-matching operations will always match items with the same type.",
            "Операции сопоставления всегда будут сопоставлять элементы одного и того же типа.",
        ),
        "typeMatchTags" => ("Type Match Tags", "Теги сопоставления по типу"),
        "typeMatchTags.tooltip.1" => (
            "Items with any of the tags in this list will be matched by type only.",
            "Элементы с любым из тегов в этом списке будут сопоставляться только по типу.",
        ),
        "typeMatchTags.tooltip.2" => (
            "Only used if 'Always Match by Type' is disabled.",
            "Используется только в том случае, если опция 'Всегда сопоставлять по типу' отключена.",
        ),
        "typeMatchTags.tooltip.3" => ("See %s for a list of tags.", "Список тегов см. в %s."),
        "sounds" => ("Sounds", "Звук"),
        "playSoundSort" => ("Play Sorting Sounds", "Воспроизводить звуки сортировки"),
        "playSoundSort.tooltip" => (
            "Whether to play sounds when sorting without server acceleration.",
            "Воспроизводить ли звуки при сортировке без ускорения сервера.",
        ),
        "playSoundOther" => ("Play Other Sounds", "Воспроизводить другие звуки"),
        "playSoundOther.tooltip" => (
            "Whether to play sounds when performing other operations without server acceleration.",
            "Воспроизводить ли звуки при выполнении других операций без ускорения сервера.",
        ),
        "interactionSound" => ("Interaction Sound", "Звук взаимодействия"),
        "soundInterval" => ("Sound Interval", "Интервал звука"),
        "soundInterval.tooltip" => (
            "The minimum time, in milliseconds, between starting interaction sounds.",
            "Минимальное время между звуками взаимодействия, выраженное в миллисекундах.",
        ),
        "soundPitchMin" => ("Minimum Pitch", "Минимальная высота звука"),
        "soundPitchMin.tooltip" => ("Initial pitch of the interaction sound.", "Начальная высота звука взаимодействия"),
        "soundPitchMax" => ("Maximum Pitch", "Максимальная высота звука"),
        "soundPitchMax.tooltip" => ("Final pitch of the interaction sound.", "Конечная высота звука взаимодействия"),
        "soundVolume" => ("Volume", "Громкость"),
        "allowSoundOverlap" => ("Allow Overlap", "Разрешить наложение звуков"),
        "allowSoundOverlap.tooltip" => (
            "Whether to allow the previous sound to continue when starting a new sound.",
            "Продолжать воспроизведение предыдущего звука при запуске нового",
        ),
        "keybinds" => ("Keybinds", "Назначение клавиш"),
        "buttons" => ("Buttons", "Кнопки"),
        "showButtons" => ("Show Buttons", "Показывать кнопки"),
        "showButtons.tooltip" => (
            "Whether to show extra buttons on the GUI for inventory operations.",
            "Показывать ли дополнительные кнопки в интерфейсе для операций с инвентарём.",
        ),
        "showButtonTooltips" => ("Show Button Tooltips", "Всплывающие подсказки кнопок"),
        "showButtonTooltips.tooltip" => (
            "Whether to show tooltips when hovering over buttons.",
            "Показывать ли всплывающие подсказки при наведении курсора мыши на кнопки.",
        ),
        "firstButtonOp" => ("First Button Operation", "Операция первой кнопки"),
        "secondButtonOp" => ("Second Button Operation", "Операция второй кнопки"),
        "thirdButtonOp" => ("Third Button Operation", "Операция третьей кнопки"),
        "fourthButtonOp" => ("Fourth Button Operation", "Операция четвёртой кнопки"),
        "HOTBAR" => ("Hotbar", "Панель быстрого доступа"),
        "HOTBAR.tooltip" => (
            "Hotbar will be processed as a separate inventory.",
            "Панель быстрого доступа будет сортироваться как отдельный инвентарь",
        ),
        "INVENTORY" => ("Inventory", "Инвентарь"),
        "INVENTORY.tooltip" => (
            "Hotbar will be processed as part of the main inventory.",
            "Панель быстрого доступа будет сортироваться как часть основного инвентаря",
        ),
        "NONE" => ("None", "Нет"),
        "NONE.tooltip" => ("Hotbar will not be processed.", "Панель быстрого доступа никак не будет сортироваться"),
        "none" => ("None", "Без сортировки"),
        "alphabet" => ("Alphabet", "По алфавиту"),
        "creative" => ("Creative", "Творческий"),
        "quantity" => ("Quantity", "По количеству"),
        "rawId" => ("Raw ID", "По необработанному ID"),
        "SORT" => ("Sort", "Сортировать"),
        "STACK_FILL" => ("Fill Stacks", "Заполнить стаки"),
        "MATCH_TRANSFER" => ("Restock", "Пополнить"),
        "TRANSFER" => ("Transfer", "Переместить"),
        "low" => ("Too low", "Слишком низкое значение"),
        "high" => ("Too high", "Слишком высокое значение"),
        "duplicate" => ("Each button can only appear once", "Каждая кнопка может появляться только один раз"),
        _ => ("", ""),
    };
    if ru { rus } else { en }
}

/// The keys' entries, in the Keybinds tab's order (its editor and Cancel
/// Auto Operation keys are left out: this port has neither).
const KEY_IDS: [&str; 4] = ["key_sort", "key_stack_fill", "key_restock", "key_transfer"];
const OP_IDS: [&str; 4] = ["first_button_op", "second_button_op", "third_button_op", "fourth_button_op"];

/// The low/high error supplier of a number between `min` and `max`.
fn range(ru: bool, min: f32, max: f32, float: bool) -> impl Fn(&Value, &super::clientsort_cloth::Values<'_>) -> Option<String> {
    let (low, high) = (t(ru, "low").to_owned(), t(ru, "high").to_owned());
    move |v, _| {
        let n = if float { v.as_float() } else { v.as_int() as f32 };
        if n < min {
            Some(low.clone())
        } else if n > max {
            Some(high.clone())
        } else {
            None
        }
    }
}

/// `ClothScreenProvider.getConfigScreen`, with the options this port has.
fn build(game: &Game<'_>, ru: bool, in_world: bool) -> Cloth {
    let s = super::clientsort_glue::settings(game);
    let d = cs::Settings::default();
    let tip = |key: &str| t(ru, key);

    let general = vec![
        Entry::int("interval", t(ru, "interactionInterval"), i64::from(s.interval), i64::from(d.interval))
            .tooltip(&[tip("interactionInterval.tooltip")])
            .check(range(ru, cs::INTERVAL_MIN as f32, cs::INTERVAL_MAX as f32, false)),
        Entry::boolean("server_acceleration", t(ru, "useServerAcceleration"), s.server_acceleration, d.server_acceleration)
            .tooltip(&[tip("useServerAcceleration.tooltip")]),
        Entry::select(
            "hotbar_scope",
            t(ru, "hotbarScope"),
            cs::HotbarScope::ALL.iter().map(|h| t(ru, h.stored()).to_owned()).collect(),
            cs::HotbarScope::ALL.iter().position(|h| *h == s.hotbar).unwrap_or(0),
            0,
        )
        .choice_tooltips(
            cs::HotbarScope::ALL.iter().map(|h| vec![t(ru, &format!("{}.tooltip", h.stored())).to_owned()]).collect(),
        ),
        Entry::boolean("transfer_reverse", t(ru, "transferReverseOrder"), s.transfer_reverse, d.transfer_reverse)
            .tooltip(&[tip("transferReverseOrder.tooltip")]),
    ];

    let orders: Vec<String> = cs::Order::ALL.iter().map(|o| t(ru, o.stored()).to_owned()).collect();
    let order = |id: &'static str, key: &str, value: cs::Order, default: cs::Order| {
        let at = |o: cs::Order| cs::Order::ALL.iter().position(|x| *x == o).unwrap_or(0);
        Entry::select(id, t(ru, key), orders.clone(), at(value), at(default))
    };
    let item = "§6minecraft:red_shulker_box§r";
    let accepts = t(ru, "overrideItems.tooltip.2").replace("%s", item);
    let sorting = vec![
        order("order", "sortOrder", s.order, d.order),
        order("shift_order", "shiftSortOrder", s.shift_order, d.shift_order),
        order("ctrl_order", "ctrlSortOrder", s.ctrl_order, d.ctrl_order),
        order("alt_order", "altSortOrder", s.alt_order, d.alt_order),
        Entry::text(t(ru, "orderOverrides"), 0xFF_FFFF),
        Entry::boolean("use_start_overrides", t(ru, "useStartOverrides"), s.use_start_overrides, d.use_start_overrides)
            .tooltip(&[tip("useOverrides.tooltip")]),
        Entry::list("start_items", t(ru, "startOverrideItems"), s.start_items.clone(), d.start_items.clone(), s.use_start_overrides, true)
            .tooltip(&[tip("startOverrideItems.tooltip"), &accepts]),
        Entry::boolean("use_end_overrides", t(ru, "useEndOverrides"), s.use_end_overrides, d.use_end_overrides)
            .tooltip(&[tip("useOverrides.tooltip")]),
        Entry::list("end_items", t(ru, "endOverrideItems"), s.end_items.clone(), d.end_items.clone(), s.use_end_overrides, true)
            .tooltip(&[tip("endOverrideItems.tooltip"), &accepts]),
    ];

    let wiki = t(ru, "typeMatchTags.tooltip.3").replace("%s", "§6https://minecraft.wiki/w/Item_tag§r");
    let matching = vec![
        Entry::boolean("always_match_by_type", t(ru, "alwaysMatchByType"), s.always_match_by_type, d.always_match_by_type)
            .tooltip(&[tip("alwaysMatchByType.tooltip")]),
        Entry::list("type_match_tags", t(ru, "typeMatchTags"), s.type_match_tags.clone(), d.type_match_tags.clone(), !s.always_match_by_type, true)
            .tooltip(&[tip("typeMatchTags.tooltip.1"), tip("typeMatchTags.tooltip.2"), &wiki]),
    ];

    // The pitches check against the other's saved value, as ClientSort does.
    let (low, high) = (t(ru, "low").to_owned(), t(ru, "high").to_owned());
    let (saved_min, saved_max) = (s.sound_pitch_min, s.sound_pitch_max);
    let (l1, h1, l2, h2) = (low.clone(), high.clone(), low, high);
    let sounds = vec![
        Entry::boolean("play_sound_sort", t(ru, "playSoundSort"), s.play_sound_sort, d.play_sound_sort)
            .tooltip(&[tip("playSoundSort.tooltip")]),
        Entry::boolean("play_sound_other", t(ru, "playSoundOther"), s.play_sound_other, d.play_sound_other)
            .tooltip(&[tip("playSoundOther.tooltip")]),
        // ClientSort names an error key its language files don't have, so
        // the key itself shows.
        Entry::string("interaction_sound", t(ru, "interactionSound"), &s.interaction_sound, &d.interaction_sound)
            .check(|v, _| (!cs::valid_identifier(v.as_text())).then(|| "error.clientsort.Identifier.parse".to_owned())),
        Entry::int("sound_interval", t(ru, "soundInterval"), i64::from(s.sound_interval), i64::from(d.sound_interval))
            .tooltip(&[tip("soundInterval.tooltip")])
            .check(range(ru, cs::SOUND_INTERVAL_MIN as f32, cs::SOUND_INTERVAL_MAX as f32, false)),
        Entry::float("sound_pitch_min", t(ru, "soundPitchMin"), &cs::java_float(s.sound_pitch_min), &cs::java_float(d.sound_pitch_min))
            .tooltip(&[tip("soundPitchMin.tooltip")])
            .check(move |v, _| {
                let f = v.as_float();
                if f < cs::PITCH_MIN { Some(l1.clone()) } else if f > saved_max { Some(h1.clone()) } else { None }
            }),
        Entry::float("sound_pitch_max", t(ru, "soundPitchMax"), &cs::java_float(s.sound_pitch_max), &cs::java_float(d.sound_pitch_max))
            .tooltip(&[tip("soundPitchMax.tooltip")])
            .check(move |v, _| {
                let f = v.as_float();
                if f < saved_min { Some(l2.clone()) } else if f > cs::PITCH_MAX { Some(h2.clone()) } else { None }
            }),
        Entry::float("sound_volume", t(ru, "soundVolume"), &cs::java_float(s.sound_volume), &cs::java_float(d.sound_volume))
            .check(range(ru, cs::VOLUME_MIN, cs::VOLUME_MAX, true)),
        Entry::boolean("allow_sound_overlap", t(ru, "allowSoundOverlap"), s.allow_sound_overlap, d.allow_sound_overlap)
            .tooltip(&[tip("allowSoundOverlap.tooltip")]),
    ];

    let keybinds = cs::Op::ALL
        .iter()
        .zip(KEY_IDS)
        .map(|(op, id)| {
            let now = game.native_mod_key(cs::ID, op.key(), op.default_key());
            Entry::key(id, op.label(ru), &now, op.default_key())
        })
        .collect();

    let ops: Vec<String> = cs::Op::ALL.iter().map(|o| t(ru, o.stored()).to_owned()).collect();
    let mut buttons = vec![
        Entry::boolean("buttons", t(ru, "showButtons"), s.buttons, d.buttons).tooltip(&[tip("showButtons.tooltip")]),
        Entry::boolean("tooltips", t(ru, "showButtonTooltips"), s.tooltips, d.tooltips)
            .tooltip(&[tip("showButtonTooltips.tooltip")]),
    ];
    for (n, (id, key)) in OP_IDS.iter().zip(["firstButtonOp", "secondButtonOp", "thirdButtonOp", "fourthButtonOp"]).enumerate() {
        let at = |o: cs::Op| cs::Op::ALL.iter().position(|x| *x == o).unwrap_or(0);
        let duplicate = t(ru, "duplicate").to_owned();
        let others: Vec<&'static str> = OP_IDS.iter().copied().filter(|o| o != id).collect();
        buttons.push(
            Entry::select(id, t(ru, key), ops.clone(), at(s.button_ops[n]), at(d.button_ops[n])).check(move |v, values| {
                others
                    .iter()
                    .any(|o| values.get(o).is_some_and(|x| x.as_choice() == v.as_choice()))
                    .then(|| duplicate.clone())
            }),
        );
    }

    let category = |key: &str, entries: Vec<Entry>| Category { name: t(ru, key).to_owned(), entries };
    Cloth::new(
        t(ru, "name"),
        vec![
            category("general", general),
            category("sorting", sorting),
            category("matching", matching),
            category("sounds", sounds),
            category("keybinds", keybinds),
            category("buttons", buttons),
        ],
        in_world,
    )
}

/// Save & Quit: every entry's save consumer, then `Config.getAndSave`.
fn save(game: &mut Game<'_>, values: Vec<(&'static str, Value)>) {
    let mut s = super::clientsort_glue::settings(game);
    // A list's items stripped, the blank ones dropped.
    let list = |v: &Value| -> Vec<String> {
        v.as_list().iter().map(|i| i.trim().to_owned()).filter(|i| !i.is_empty()).collect()
    };
    let order = |v: &Value| cs::Order::ALL.get(v.as_choice()).copied().unwrap_or(cs::Order::Creative);
    for (id, v) in &values {
        match *id {
            "interval" => s.interval = v.as_int().clamp(cs::INTERVAL_MIN, cs::INTERVAL_MAX) as u32,
            "server_acceleration" => s.server_acceleration = v.as_bool(),
            "hotbar_scope" => s.hotbar = cs::HotbarScope::ALL.get(v.as_choice()).copied().unwrap_or(cs::HotbarScope::Hotbar),
            "transfer_reverse" => s.transfer_reverse = v.as_bool(),
            "order" => s.order = order(v),
            "shift_order" => s.shift_order = order(v),
            "ctrl_order" => s.ctrl_order = order(v),
            "alt_order" => s.alt_order = order(v),
            "use_start_overrides" => s.use_start_overrides = v.as_bool(),
            "start_items" => s.start_items = list(v),
            "use_end_overrides" => s.use_end_overrides = v.as_bool(),
            "end_items" => s.end_items = list(v),
            "always_match_by_type" => s.always_match_by_type = v.as_bool(),
            "type_match_tags" => s.type_match_tags = list(v),
            "play_sound_sort" => s.play_sound_sort = v.as_bool(),
            "play_sound_other" => s.play_sound_other = v.as_bool(),
            "interaction_sound" => s.interaction_sound = v.as_text().to_owned(),
            "sound_interval" => s.sound_interval = v.as_int().clamp(cs::SOUND_INTERVAL_MIN, cs::SOUND_INTERVAL_MAX) as u32,
            "sound_pitch_min" => s.sound_pitch_min = v.as_float(),
            "sound_pitch_max" => s.sound_pitch_max = v.as_float(),
            "sound_volume" => s.sound_volume = v.as_float(),
            "allow_sound_overlap" => s.allow_sound_overlap = v.as_bool(),
            "buttons" => s.buttons = v.as_bool(),
            "tooltips" => s.tooltips = v.as_bool(),
            _ => {}
        }
        if let Some(n) = OP_IDS.iter().position(|o| o == id) {
            s.button_ops[n] = cs::Op::ALL.get(v.as_choice()).copied().unwrap_or(cs::Op::Sort);
        }
        // `KeybindManager.bindKey`: the key alone, without the modifiers.
        if let Some(n) = KEY_IDS.iter().position(|k| k == id) {
            let op = cs::Op::ALL[n];
            let key = v.as_key();
            let chosen = (key != op.default_key()).then_some(key);
            game.native_set_mod_key(cs::ID, op.key(), chosen);
        }
    }
    super::clientsort_glue::save_settings(game, s);
}

// ---- opening, covering, closing -----------------------------------------------------------

/// The mod's row in Options > Mods.
fn row(game: &Game<'_>) -> Option<usize> {
    game.settings.mods.installed.iter().position(|m| m.id == cs::ID && m.native.is_some())
}

/// Opens the screen over the mod's page: `modmenu` when Mod Menu's Configure
/// asked.
pub(super) fn open(game: &mut Game<'_>, modmenu: bool) {
    let Some(row) = row(game) else {
        return;
    };
    let ru = russian();
    let screen = build(game, ru, game.ui.options.in_world);
    with(|s| {
        s.screen = Some(screen);
        s.modmenu = modmenu;
        s.row = row;
    });
}

/// The screen is up and the options show the mod's page under it.
pub(super) fn covers(game: &Game<'_>) -> bool {
    let (open, row) = with(|s| (s.screen.is_some(), s.row)).unwrap_or((false, 0));
    open && game.screen == Screen::Options
        && game.ui.options.page() == Page::NativeMod(row)
        && game.settings.mods.installed.get(row).is_some_and(|m| m.id == cs::ID)
}

fn close(game: &mut Game<'_>) {
    let Some((modmenu, row)) = with(|s| {
        s.screen = None;
        (s.modmenu, s.row)
    }) else {
        return;
    };
    if modmenu {
        // As Escape on the page Mod Menu opened alone: back to Mod Menu.
        let events = game.ui.options.back(&mut game.settings);
        let _ = game.handle_option_events(events);
    } else {
        game.native_settings_refresh(row);
    }
}

/// Mod Menu's icon and metadata in the mod's folder, each written when it
/// differs from what is there.
fn write_data(game: &Game<'_>) {
    let dir = game.native_data_dir(cs::ID);
    let _ = std::fs::create_dir_all(&dir);
    let json = format!(
        r#"{{
  "schema": 1,
  "name": "ClientSort",
  "version": "{}",
  "description": "Versatile and easy inventory sorting.",
  "description_ru": "Универсальная и простая сортировка инвентаря.",
  "authors": ["NotRyken"],
  "contributors": ["Siphalor (Mouse Wheelie)", "Roundaround (Inventory Management)", "TauCu (#49, #81)", "octoturt (#14)", "Xinyang-Gao (zh_cn)", "rfin0 (ru_ru)", "DeportedToHell"],
  "license": "Apache-2.0",
  "links": {{
    "homepage": "https://modrinth.com/project/K0AkAin6",
    "sources": "https://github.com/DeportedToHell/MCRE-Mods/tree/main/clientsort",
    "issues": "https://github.com/DeportedToHell/MCRE-Mods/issues"
  }},
  "badges": [],
  "config_screen": true
}}
"#,
        cs::VERSION
    );
    for (name, bytes) in [("icon.png", ICON), ("modmenu.json", json.as_bytes())] {
        let path = dir.join(name);
        if std::fs::read(&path).ok().as_deref() != Some(bytes) {
            let _ = std::fs::write(&path, bytes);
        }
    }
}

/// Once a frame: the data files at first, what the screen asked for, and the
/// screen gone when its page is.
pub(super) fn frame(game: &mut Game<'_>) {
    if with(|s| !std::mem::replace(&mut s.data_written, true)).unwrap_or(false) {
        write_data(game);
    }
    if !with(|s| s.screen.is_some()).unwrap_or(false) {
        return;
    }
    if !covers(game) {
        // The page changed some other way: closed, nothing saved.
        with(|s| s.screen = None);
        return;
    }
    let requests = with(|s| s.screen.as_mut().map(Cloth::take_requests).unwrap_or_default()).unwrap_or_default();
    for r in requests {
        match r {
            Request::Click => game.audio.play_button_click(),
            Request::Paste => {
                let text = game.clipboard.get();
                with(|s| {
                    if let Some(screen) = s.screen.as_mut() {
                        screen.typed(&text);
                    }
                });
            }
            Request::Copy(text) => {
                if !text.is_empty() {
                    game.clipboard.set(&text);
                }
            }
            Request::Close(saved) => {
                if saved {
                    let values = with(|s| s.screen.as_ref().map(Cloth::values)).flatten().unwrap_or_default();
                    save(game, values);
                }
                close(game);
                return;
            }
        }
    }
}

// ---- the screen hooks -----------------------------------------------------------------------

/// The screen over the options, after they drew (nothing: `draw_options` is
/// held back while the screen covers them).
pub(super) fn render(game: &Game<'_>, batch: &mut HudBatch, screen: &NativeScreenView<'_>) {
    if screen.id != "options" || !covers(game) {
        return;
    }
    with(|s| {
        if let Some(c) = s.screen.as_mut() {
            c.draw(game, batch);
        }
    });
}

/// Runs `f` on the open screen when it covers `screen`: true when it did
/// (the event goes no further).
fn input(game: &Game<'_>, screen: &NativeScreenView<'_>, f: impl FnOnce(&mut Cloth, f32, f32, (f32, f32))) -> bool {
    if screen.id != "options" || !covers(game) {
        return false;
    }
    let (w, h) = (screen.rect[2].floor(), screen.rect[3].floor());
    let p = (screen.mouse[0], screen.mouse[1]);
    with(|s| {
        if let Some(c) = s.screen.as_mut() {
            f(c, w, h, p);
        }
    });
    true
}

pub(super) fn pressed(game: &Game<'_>, screen: &NativeScreenView<'_>, button: i32, modifiers: u32) -> bool {
    input(game, screen, |c, w, h, p| c.press(game, w, h, p, button, modifiers))
}

pub(super) fn released(game: &Game<'_>, screen: &NativeScreenView<'_>, button: i32) -> bool {
    let _ = button;
    input(game, screen, |c, _, _, _| c.release())
}

pub(super) fn dragged(game: &Game<'_>, screen: &NativeScreenView<'_>) -> bool {
    input(game, screen, |c, w, h, p| c.dragged(game, w, h, p))
}

pub(super) fn scrolled(game: &Game<'_>, screen: &NativeScreenView<'_>, scroll: [f64; 2]) -> bool {
    input(game, screen, |c, w, h, p| c.scrolled(game, w, h, p, scroll[1] as f32))
}

pub(super) fn key_pressed(game: &Game<'_>, screen: &NativeScreenView<'_>, key: &str, modifiers: u32) -> bool {
    input(game, screen, |c, w, h, _| c.key_pressed(game, w, h, key, modifiers))
}

pub(super) fn key_released(game: &Game<'_>, screen: &NativeScreenView<'_>) -> bool {
    input(game, screen, |c, _, _, _| c.key_released())
}

pub(super) fn char_typed(game: &Game<'_>, screen: &NativeScreenView<'_>, c: char) -> bool {
    input(game, screen, |s, _, _, _| s.typed(&c.to_string()))
}
