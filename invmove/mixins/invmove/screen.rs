// InvMove's config screen: the Cloth Config screen `InvMoveConfig.setupCloth`
// builds (cloth.rs draws it), with its General, UI Movement and UI Background
// tabs: the options, the Allowed Keys, the vanilla module's per-screen
// options and the unrecognized screens grouped by mod. Opened by Mod Menu's
// Configure (`MODMENU_CONFIGURE`) and by Configure... on the mod's page in
// Options > Mods; while it is up it covers the options. Save & Quit sets the
// options and saves the files (`InvMoveConfig.save`).
//
// Ported from InvMove by PieKing1215 (LGPL-3.0-only,
// https://github.com/PieKing1215/InvMove).
use super::invmove_cloth::{Category, Entry, Request, Screen as Cloth, Value};
use super::invmove_config::{self as cfg, Config, PauseMode, SneakMode, VANILLA, RECIPE_BOOK, t};
use super::{Game, Screen};
use horizon_client::clipboard::Clipboard as _;
use horizon_client::hud::HudBatch;
use horizon_client::mod_api::NativeScreenView;
use horizon_client::options_ui::Page;
use std::cell::RefCell;

/// Mod Menu's Configure: `native_settings_click(id, MODMENU_CONFIGURE)` after
/// it opened the mod's page.
pub(super) const MODMENU_CONFIGURE: usize = 0x4D4D_4346;

struct State {
    screen: Option<Cloth>,
    /// Opened by Mod Menu: closing leaves the options too.
    modmenu: bool,
    /// The mod's row in Options > Mods.
    row: usize,
}

thread_local! {
    static STATE: RefCell<State> = const { RefCell::new(State { screen: None, modmenu: false, row: 0 }) };
}

fn with<R>(f: impl FnOnce(&mut State) -> R) -> Option<R> {
    STATE.with(|s| s.try_borrow_mut().ok().map(|mut s| f(&mut s)))
}

// ---- building --------------------------------------------------------------------------------

/// A tooltip's lines: one per `\n`.
fn lines(key: &str) -> Vec<String> {
    t(key).split('\n').map(str::to_owned).collect()
}

fn with_tip(e: Entry, lines: &[String]) -> Entry {
    e.tooltip(&lines.iter().map(String::as_str).collect::<Vec<_>>())
}

/// `ConfigBool.addTo`: its name `id`, a tooltip when `tooltip.<id>` exists.
fn boolean(entry_id: &str, key: &str, v: bool, d: bool) -> Entry {
    let e = Entry::boolean(entry_id, &t(key), v, d);
    let tip = format!("tooltip.{key}");
    if cfg::has(&tip) { with_tip(e, &lines(&tip)) } else { e }
}

/// `ConfigEnum.addTo`: an enum selector with the constants' names.
fn select(entry_id: &str, key: &str, names: Vec<String>, v: usize, d: usize) -> Entry {
    with_tip(Entry::select(entry_id, &t(key), names, v, d), &lines(&format!("tooltip.{key}")))
}

fn sneak(entry_id: &str, key: &str, v: SneakMode, d: SneakMode) -> Entry {
    let at = |m: SneakMode| SneakMode::ALL.iter().position(|x| *x == m).unwrap_or(0);
    let names = SneakMode::ALL.iter().map(|m| m.name().to_owned()).collect();
    select(entry_id, key, names, at(v), at(d))
}

fn movement_text(e: Entry) -> Entry {
    e.yes_no(&t("invmove.movement.yes"), &t("invmove.movement.no"))
}

fn background_text(e: Entry) -> Entry {
    e.yes_no(&t("invmove.background.yes"), &t("invmove.background.no"))
}

/// A text description (`startTextDescription`), with its style.
fn label(text: &str) -> Entry {
    Entry::text(text, 0xff_ffff)
}

/// A vanilla module option's name key: its `display`, else its own key.
fn vanilla_key(i: usize, kind: &str) -> String {
    let (id, display) = VANILLA[i];
    display.map_or_else(|| format!("config.invmove.vanilla.{kind}.{id}"), str::to_owned)
}

/// The unrecognized screens of `list` grouped by mod, a sub-category each
/// (`setupCloth`'s "unrecognized" part).
fn unrecognized(game: &Game<'_>, list: &[(String, bool)], prefix: &str, background: bool) -> Vec<Entry> {
    let mut out = vec![label(&format!("§n{}", t("key.invmove.unrecognized")))];
    if list.is_empty() {
        out.push(label(&format!("§7{}", t("key.invmove.unrecognized.desc"))));
    }
    let mut mods: Vec<&str> = Vec::new();
    for (id, _) in list {
        let m = super::invmove_glue::modid(id);
        if !mods.contains(&m) {
            mods.push(m);
        }
    }
    for m in mods {
        let entries = list
            .iter()
            .filter(|(id, _)| super::invmove_glue::modid(id) == m)
            .map(|(id, v)| {
                let e = Entry::boolean(&format!("{prefix}|{id}"), super::invmove_glue::simple_name(id), *v, *v).no_default();
                let e = with_tip(e, &[format!("§7{id}")]);
                if background { background_text(e) } else { movement_text(e) }
            })
            .collect();
        let name = if m == "?unknown" { t("key.invmove.unrecognized.nomod") } else { super::invmove_glue::mod_name(game, m) };
        out.push(with_tip(Entry::sub(&name, entries, false), &[format!("§7modid: {m}")]));
    }
    out
}

/// `InvMoveConfig.setupCloth` for the settings now.
fn build(game: &Game<'_>, c: &Config, in_world: bool) -> Cloth {
    let d = Config::default();

    let general = vec![
        boolean("general.enable", "config.invmove.general.enable", c.enabled, d.enabled),
        // `setRequirement(() -> false)`: the debug display isn't drawn on 26.x.
        boolean("general.debugDisplay", "config.invmove.general.debugDisplay", c.debug_display, d.debug_display).disabled(),
    ];

    let mut movement = vec![
        boolean("movement.enable", "config.invmove.movement.enable", c.move_enabled, d.move_enabled),
        sneak("movement.sneak", "config.invmove.movement.sneak", c.sneak, d.sneak),
        sneak("movement.sneak_disallowed", "config.invmove.movement.sneak_disallowed", c.sneak_disallowed, d.sneak_disallowed),
        sneak("movement.sneak_flying", "config.invmove.movement.sneak_flying", c.sneak_flying, d.sneak_flying),
        boolean("movement.dismount", "config.invmove.movement.dismount", c.dismount, d.dismount),
        boolean("movement.textFieldDisables", "config.invmove.movement.textFieldDisables", c.text_field_disables, d.text_field_disables),
        movement_text(boolean(
            "movement.unrecognizedScreenDefault",
            "config.invmove.movement.unrecognizedScreenDefault",
            c.move_unrecognized_default,
            d.move_unrecognized_default,
        )),
    ];
    // Allowed Keys: every key mapping, Allow or Ignore.
    let keys = super::invmove_glue::mappings()
        .into_iter()
        .map(|name| {
            let def = cfg::allow_key_default(name);
            let e = Entry::boolean(&format!("key|{name}"), &t(name), c.allowed_key(name), def);
            with_tip(e, &[format!("§7{name}")]).yes_no(&t("invmove.key.yes"), &t("invmove.key.no"))
        })
        .collect();
    movement.push(with_tip(
        Entry::sub(&t("key.invmove.category.movement.keys"), keys, false),
        &lines("key.invmove.category.movement.keys.desc"),
    ));
    // The modules: the vanilla one.
    movement.push(label(&format!("§n{}", t("key.invmove.modules"))));
    let mut vanilla = vec![movement_text(boolean(&format!("vmove|{RECIPE_BOOK}"), "Recipe Book", c.recipe_book, d.recipe_book))];
    for (i, (id, _)) in VANILLA.iter().enumerate() {
        vanilla.push(movement_text(boolean(&format!("vmove|{id}"), &vanilla_key(i, "movement"), c.vanilla_move[i], true)));
    }
    movement.push(Entry::sub(&t("key.invmove.module.vanilla"), vanilla, false));
    movement.extend(unrecognized(game, &c.unrecognized_move, "umove", false));

    let at = |m: PauseMode| PauseMode::ALL.iter().position(|x| *x == m).unwrap_or(0);
    let mut background = vec![
        boolean("background.enable", "config.invmove.background.enable", c.bg_enabled, d.bg_enabled),
        select(
            "background.hideOnPause",
            "config.invmove.background.hideOnPause",
            PauseMode::ALL.iter().map(|m| m.name().to_owned()).collect(),
            at(c.hide_on_pause),
            at(d.hide_on_pause),
        ),
        background_text(boolean(
            "background.unrecognizedScreenDefault",
            "config.invmove.background.unrecognizedScreenDefault",
            c.bg_unrecognized_default,
            d.bg_unrecognized_default,
        )),
    ];
    background.push(label(&format!("§n{}", t("key.invmove.modules"))));
    let vanilla = VANILLA
        .iter()
        .enumerate()
        .map(|(i, (id, _))| background_text(boolean(&format!("vbg|{id}"), &vanilla_key(i, "backgroundHide"), c.vanilla_bg[i], true)))
        .collect();
    background.push(Entry::sub(&t("key.invmove.module.vanilla"), vanilla, false));
    background.extend(unrecognized(game, &c.unrecognized_bg, "ubg", true));

    let category = |key: &str, entries: Vec<Entry>| Category { name: t(key), entries };
    Cloth::new(
        &t("config.invmove.title"),
        vec![
            category("key.invmove.category.general", general),
            category("key.invmove.category.movement", movement),
            category("key.invmove.category.background", background),
        ],
        in_world,
    )
}

/// Save & Quit: every entry's save consumer, then `InvMoveConfig.save`.
fn save(values: Vec<(String, Value)>) {
    let mut c = cfg::get();
    let sneak = |v: &Value| SneakMode::ALL.get(v.as_choice()).copied().unwrap_or(SneakMode::Maintain);
    for (id, v) in &values {
        let b = v.as_bool();
        if let Some((kind, key)) = id.split_once('|') {
            match kind {
                "key" => c.set_allowed_key(key, b),
                "vmove" if key == RECIPE_BOOK => c.recipe_book = b,
                "vmove" => {
                    if let Some(i) = Config::vanilla_index(key) {
                        c.vanilla_move[i] = b;
                    }
                }
                "vbg" => {
                    if let Some(i) = Config::vanilla_index(key) {
                        c.vanilla_bg[i] = b;
                    }
                }
                "umove" => {
                    if let Some(e) = c.unrecognized_move.iter_mut().find(|(s, _)| s == key) {
                        e.1 = b;
                    }
                }
                "ubg" => {
                    if let Some(e) = c.unrecognized_bg.iter_mut().find(|(s, _)| s == key) {
                        e.1 = b;
                    }
                }
                _ => {}
            }
            continue;
        }
        match id.as_str() {
            "general.enable" => c.enabled = b,
            "general.debugDisplay" => c.debug_display = b,
            "movement.enable" => c.move_enabled = b,
            "movement.sneak" => c.sneak = sneak(v),
            "movement.sneak_disallowed" => c.sneak_disallowed = sneak(v),
            "movement.sneak_flying" => c.sneak_flying = sneak(v),
            "movement.dismount" => c.dismount = b,
            "movement.textFieldDisables" => c.text_field_disables = b,
            "movement.unrecognizedScreenDefault" => c.move_unrecognized_default = b,
            "background.enable" => c.bg_enabled = b,
            "background.hideOnPause" => c.hide_on_pause = PauseMode::ALL.get(v.as_choice()).copied().unwrap_or(PauseMode::Show),
            "background.unrecognizedScreenDefault" => c.bg_unrecognized_default = b,
            _ => {}
        }
    }
    cfg::replace(c);
}

// ---- opening, covering, closing -----------------------------------------------------------

/// The mod's row in Options > Mods.
fn row(game: &Game<'_>) -> Option<usize> {
    game.settings.mods.installed.iter().position(|m| m.id == cfg::ID && m.native.is_some())
}

/// Opens the screen over the mod's page: `from_modmenu` when Mod Menu's
/// Configure asked.
pub(super) fn open(game: &Game<'_>, from_modmenu: bool) {
    let Some(row) = row(game) else {
        return;
    };
    let screen = build(game, &cfg::get(), game.ui.options.in_world);
    with(|s| {
        s.screen = Some(screen);
        s.modmenu = from_modmenu;
        s.row = row;
    });
}

/// The screen is up and the options show the mod's page under it.
pub(super) fn covers(game: &Game<'_>) -> bool {
    let (open, row) = with(|s| (s.screen.is_some(), s.row)).unwrap_or((false, 0));
    open && game.screen == Screen::Options
        && game.ui.options.page() == Page::NativeMod(row)
        && game.settings.mods.installed.get(row).is_some_and(|m| m.id == cfg::ID)
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

/// Once a frame: what the screen asked for, and the screen gone when its
/// page is.
pub(super) fn frame(game: &mut Game<'_>) {
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
                    save(values);
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
    input(game, screen, |c, _, _, _| c.release(button))
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
