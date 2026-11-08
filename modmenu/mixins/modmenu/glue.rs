// Mod Menu for Horizon: the game's side. The mods read from the game (the
// mods built in, the content mods turned on, Minecraft and Horizon themselves)
// and from the catalog's copy beside the game (names, authors, licences,
// links, icons, newer versions); the mods screen opened over the title screen
// or the pause menu, and its mouse and keys. `modmenu_view` draws it.
//
// The screen hooks only read the game (`&Game`): what needs to change it waits
// in `State::pending` for the next frame.
use super::modmenu::{self as mm, Badge, Config, Configure, Icon, Mod, Update};
use super::{Game, Screen};
use horizon_client::clipboard::Clipboard as _;
use horizon_client::title::modmenu_title as title_side;
use horizon_client::ui::EditBox;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::time::Instant;

/// GLFW's modifier bits.
const SHIFT: u32 = 1;
const CONTROL: u32 = 2;

/// Where the mods screen was opened from: it goes back there.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Origin {
    Title,
    Pause,
    /// From the game itself (Open Mod Menu): the game pauses under it.
    Playing,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub(super) enum Page {
    List,
    /// Mod Menu's own options (`ModMenuOptionsScreen`).
    Options,
    /// `ConfirmLinkScreen` for a link, back to the list after.
    Link(String),
}

/// What a click on the description list opens.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(super) enum Link {
    Url(String),
    /// Minecraft's View Credits.
    Credits,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Hit {
    Search,
    Filters,
    Sort,
    Libraries,
    Configure,
    Website,
    Issues,
    ModsFolder,
    Done,
    /// A row of the list, by its place in `rows`.
    Row(usize),
    /// A link of the description list, by its place in `links`.
    DescLink(usize),
    Option(usize),
    OptionsDone,
    LinkButton(usize),
    /// A scroll bar: the list, the description, the options.
    Bar(u8),
}

#[derive(Clone, Debug)]
pub(super) enum Action {
    Open(Origin),
    Close,
    Configure(Configure),
    OpenFolder,
    Uri(String),
    Click,
    Paste,
    Copy(String),
    Credits,
}

/// What `ModsScreen.init` fixes until the next init (a resize or reopening):
/// the filter row's place and its buttons' widths.
#[derive(Clone, Copy, Debug)]
pub(super) struct Layout {
    pub(super) size: (f32, f32),
    pub(super) filters_x: f32,
    pub(super) sort_w: f32,
    pub(super) lib_w: f32,
}

/// A decoded icon: width, height, pixels as 0xAARRGGBB.
pub(super) struct Image {
    pub(super) w: u32,
    pub(super) h: u32,
    pub(super) px: Vec<u32>,
}

pub(super) enum IconState {
    Loading,
    Ready(Image),
    Failed,
}

/// A scroll bar being dragged: which, and the pointer's last height.
#[derive(Clone, Copy, Debug)]
pub(super) struct Drag {
    pub(super) bar: u8,
    pub(super) last_y: f32,
}

pub(super) struct State {
    pub(super) open: Option<Origin>,
    pub(super) page: Page,
    pub(super) mods: Vec<Mod>,
    /// The mods in the sort's order, and the list's rows (into `mods`).
    pub(super) order: Vec<usize>,
    pub(super) rows: Vec<usize>,
    pub(super) selected: Option<String>,
    pub(super) search: EditBox,
    /// The list has the keyboard's focus (its selection's outline is white).
    pub(super) list_focused: bool,
    pub(super) filters_shown: bool,
    pub(super) list_scroll: f32,
    pub(super) desc_scroll: f32,
    pub(super) options_scroll: f32,
    /// Each scroll bar's most scroll and its place the last frame:
    /// (rect, content height) for the list, the description, the options.
    pub(super) bars: [Option<([f32; 4], f32)>; 3],
    pub(super) drag: Option<Drag>,
    pub(super) last_click: Option<(Instant, String)>,
    pub(super) layout: Option<Layout>,
    pub(super) hits: Vec<([f32; 4], Hit)>,
    /// The description list's links the last frame (`Hit::DescLink`).
    pub(super) links: Vec<Link>,
    pub(super) pending: Vec<Action>,
    pub(super) config: Config,
    pub(super) config_read: bool,
    pub(super) ru: bool,
    pub(super) icons: HashMap<String, IconState>,
    icon_rx: Option<mpsc::Receiver<(String, Option<Image>)>>,
    pub(super) horizon_icon: Option<Image>,
    /// The title's mod count and whether any mod has an update, read again
    /// now and then.
    pub(super) count: Option<(Instant, usize, bool)>,
    /// What the startup screen was last told (`modmenu_title::startup_path`).
    startup: Option<String>,
}

impl Default for State {
    fn default() -> Self {
        let mut search = EditBox::new(0.0, 0.0, 0.0, 20.0);
        search.max_length = 32;
        search.bordered = true;
        State {
            open: None,
            page: Page::List,
            mods: Vec::new(),
            order: Vec::new(),
            rows: Vec::new(),
            selected: None,
            search,
            list_focused: false,
            filters_shown: false,
            list_scroll: 0.0,
            desc_scroll: 0.0,
            options_scroll: 0.0,
            bars: [None; 3],
            drag: None,
            last_click: None,
            layout: None,
            hits: Vec::new(),
            links: Vec::new(),
            pending: Vec::new(),
            config: Config::default(),
            config_read: false,
            ru: false,
            icons: HashMap::new(),
            icon_rx: None,
            horizon_icon: None,
            count: None,
            startup: None,
        }
    }
}

impl State {
    pub(super) fn selected_mod(&self) -> Option<&Mod> {
        let id = self.selected.as_deref()?;
        self.mods.iter().find(|m| m.id == id)
    }
    /// The rows again (`ModListWidget.filter`): the sort's order, the search
    /// and the filters; the selection kept, or the first row.
    pub(super) fn refilter(&mut self) {
        self.rows = mm::rows(&self.mods, &self.order, &self.search.value, &self.config, self.ru);
        if self.selected.is_none() {
            self.selected = self.rows.first().map(|&i| self.mods[i].id.clone());
        }
    }
    pub(super) fn resort(&mut self) {
        self.order = mm::sorted(&self.mods, self.config.sorting);
        self.refilter();
    }
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// Runs `f` on the state; None while it is in use.
pub(super) fn with<R>(f: impl FnOnce(&mut State) -> R) -> Option<R> {
    STATE.with(|s| s.try_borrow_mut().ok().map(|mut s| f(&mut s)))
}

pub(super) fn on(game: &Game<'_>) -> bool {
    game.native_on(mm::ID)
}

fn language_ru() -> bool {
    mm::russian(&horizon_client::lang::code())
}

// ---- settings -----------------------------------------------------------------------

pub(super) fn read_config(game: &Game<'_>, s: &mut State) {
    s.config = Config::read(|k| game.native_setting(mm::ID, k));
    s.config_read = true;
}

pub(super) fn set_option(game: &Game<'_>, s: &mut State, key: &str, token: &str) {
    game.native_set_setting(mm::ID, key, token);
    read_config(game, s);
    s.count = None;
}

/// The value an option has now, as stored.
pub(super) fn option_token(game: &Game<'_>, key: &str) -> String {
    let stored = game.native_setting(mm::ID, key).map(|v| v.trim().to_owned());
    let o = mm::option(key);
    match (stored, o) {
        (Some(v), Some(o)) if o.values.iter().any(|x| x.0 == v) => v,
        (_, Some(o)) => o.default.to_owned(),
        (v, None) => v.unwrap_or_default(),
    }
}

fn bound(game: &Game<'_>) -> String {
    game.native_mod_key(mm::ID, "open_menu", "key.keyboard.unknown")
}

// ---- the mods -----------------------------------------------------------------------

/// "Name 1.0.1" as the game lists a mod: the name and the version.
fn split_title(title: &str) -> (String, String) {
    match title.rsplit_once(' ') {
        Some((name, v)) if v.chars().next().is_some_and(|c| c.is_ascii_digit()) => (name.to_owned(), v.to_owned()),
        _ => (title.to_owned(), String::new()),
    }
}

/// The repository of a GitHub source link and its issues page.
fn issues_of(source: &str) -> Option<String> {
    let rest = source.strip_prefix("https://github.com/")?;
    let mut parts = rest.split('/');
    let (owner, repo) = (parts.next()?, parts.next()?);
    (!owner.is_empty() && !repo.is_empty()).then(|| format!("https://github.com/{owner}/{repo}/issues"))
}

/// What a mod says of itself: `modmenu.json` in its data folder, which the
/// mod writes (Java's `fabric.mod.json`), and its `icon.png` beside it.
#[derive(Default)]
struct Meta {
    name: Option<String>,
    description: Option<String>,
    authors: Vec<String>,
    contributors: Vec<String>,
    maintainers: Vec<String>,
    license: Option<String>,
    homepage: Option<String>,
    sources: Option<String>,
    issues: Option<String>,
    badges: Vec<Badge>,
    config_screen: bool,
    icon: Option<String>,
}

fn meta(game: &Game<'_>, id: &str, ru: bool) -> Meta {
    let dir = game.native_data_dir(id);
    let Some(v) = std::fs::read_to_string(dir.join("modmenu.json")).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()) else {
        return Meta::default();
    };
    let text = |k: &str| v.get(k).and_then(|x| x.as_str()).map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned);
    let list = |k: &str| -> Vec<String> {
        v.get(k).and_then(|x| x.as_array()).map(|a| a.iter().filter_map(|x| x.as_str()).map(str::to_owned).collect()).unwrap_or_default()
    };
    let link = |k: &str| v.get("links").and_then(|l| l.get(k)).and_then(|x| x.as_str()).filter(|s| s.starts_with("https://")).map(str::to_owned);
    let icon = dir.join("icon.png");
    Meta {
        name: text("name"),
        description: if ru { text("description_ru").or_else(|| text("description")) } else { text("description") },
        authors: list("authors"),
        contributors: list("contributors"),
        maintainers: list("maintainers"),
        license: text("license"),
        homepage: link("homepage"),
        sources: link("sources"),
        issues: link("issues"),
        badges: list("badges")
            .iter()
            .filter_map(|b| match b.as_str() {
                "client" => Some(Badge::Client),
                "library" => Some(Badge::Library),
                "deprecated" => Some(Badge::Deprecated),
                _ => None,
            })
            .collect(),
        config_screen: v.get("config_screen").and_then(|x| x.as_bool()).unwrap_or(false),
        icon: icon.is_file().then(|| icon.to_string_lossy().into_owned()),
    }
}

/// The mods Mod Menu lists: Minecraft and Horizon, every native mod built
/// into this game and every content mod turned on, each with what the
/// catalog knows of it.
pub(super) fn load_mods(game: &Game<'_>, ru: bool) -> Vec<Mod> {
    let catalog: HashMap<String, horizon_mods::index::Project> = std::fs::read_to_string(Game::game_dir().join(horizon_client::mod_registry::CACHE))
        .ok()
        .and_then(|text| horizon_mods::index::Index::parse(&text).ok())
        .map(|index| index.projects.into_iter().map(|p| (p.id.clone(), p)).collect())
        .unwrap_or_default();
    // The build's receipt: the native mods compiled into this game.
    let built: Vec<(String, String)> = std::env::current_exe()
        .ok()
        .and_then(|exe| std::fs::read_to_string(exe.with_file_name("native-mods.json")).ok())
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|v| v.get("mods").and_then(|m| m.as_array()).cloned())
        .map(|list| {
            list.iter()
                .filter_map(|m| Some((m.get("id")?.as_str()?.to_owned(), m.get("version").and_then(|v| v.as_str()).unwrap_or("").to_owned())))
                .collect()
        })
        .unwrap_or_default();
    let installed = &game.settings.mods.installed;
    let mut ids: Vec<(String, String, bool)> = built.iter().map(|(id, v)| (id.clone(), v.clone(), true)).collect();
    for (i, row) in installed.iter().enumerate() {
        if ids.iter().any(|(id, _, _)| *id == row.id) || row.problem.is_some() {
            continue;
        }
        let native = row.native.is_some();
        if native || game.settings.mods.is_on(i) {
            let (_, version) = split_title(&row.title);
            ids.push((row.id.clone(), version, native));
        }
    }
    let mut out = vec![
        Mod {
            id: "minecraft".into(),
            name: "Minecraft".into(),
            version: horizon_core::VANILLA_VERSION.into(),
            summary: mm::tr(ru, "minecraft_description").into(),
            description: mm::tr(ru, "minecraft_description").into(),
            authors: vec!["Mojang Studios".into()],
            contributors: Vec::new(),
            maintainers: Vec::new(),
            licenses: vec!["Minecraft EULA".into()],
            website: Some("https://aka.ms/javafeedback?ref=game".into()),
            issues: Some("https://aka.ms/snapshotbugs?ref=game".into()),
            source: None,
            badges: vec![Badge::Minecraft],
            icon: Icon::Minecraft,
            configure: Configure::Options,
            update: None,
        },
        Mod {
            id: "horizon".into(),
            name: "Horizon".into(),
            version: horizon_build(),
            summary: mm::tr(ru, "horizon_description").into(),
            description: mm::tr(ru, "horizon_description").into(),
            authors: vec!["Ludovic Marie".into()],
            contributors: Vec::new(),
            maintainers: Vec::new(),
            licenses: vec!["MIT".into()],
            website: Some("https://playhorizon.dev".into()),
            issues: Some("https://github.com/horizon-rust/horizon/issues".into()),
            source: Some("https://github.com/horizon-rust/horizon".into()),
            badges: vec![Badge::Library],
            icon: Icon::Horizon,
            configure: Configure::Horizon,
            update: None,
        },
    ];
    for (id, version, native) in ids {
        let row = installed.iter().find(|r| r.id == id);
        let project = catalog.get(&id);
        // The mod's own word first, as Java's Mod Menu reads the mod's jar,
        // then the catalog's, then the game's line on Options > Mods.
        let meta = if native { meta(game, &id, ru) } else { Meta::default() };
        let (row_name, row_version) = row.map(|r| split_title(&r.title)).unwrap_or_default();
        let version = if version.is_empty() { row_version } else { version };
        let name = meta
            .name
            .clone()
            .or_else(|| project.map(|p| p.name.clone()).filter(|n| !n.is_empty()))
            .or_else(|| (!row_name.is_empty()).then(|| row_name.clone()))
            .unwrap_or_else(|| id.clone());
        let row_description = row.map(|r| r.description.clone()).unwrap_or_default();
        let summary = meta.description.clone().or_else(|| project.map(|p| p.summary.clone()).filter(|s| !s.is_empty())).unwrap_or_else(|| row_description.clone());
        let description = meta
            .description
            .clone()
            .or_else(|| project.map(|p| mm::plain_markdown(&p.description)).filter(|s| !s.is_empty()))
            .unwrap_or_else(|| row_description.clone());
        let source = meta.sources.clone().or_else(|| project.map(|p| p.source.clone()).filter(|s| !s.is_empty()));
        // Mod Menu itself, before the catalog lists it: the port's own source.
        let source = source.or_else(|| (id == mm::ID).then(|| "https://github.com/DeportedToHell/MCRE-Mods/tree/main/modmenu".to_owned()));
        let configure = if id == mm::ID {
            Configure::ModMenu
        } else if native && meta.config_screen {
            Configure::Screen(id.clone())
        } else if row.and_then(|r| r.native.as_ref()).is_some_and(|p| !p.keys.is_empty() || !p.buttons.is_empty()) {
            Configure::Native(id.clone())
        } else {
            Configure::None
        };
        let mut badges = meta.badges.clone();
        if badges.is_empty() && project.is_some_and(|p| p.categories.iter().any(|c| c == "library")) {
            badges.push(Badge::Library);
        }
        if id == mm::ID {
            badges.push(Badge::Client);
        }
        let update = project.and_then(|p| {
            let latest = p.versions.iter().map(|v| v.version.clone()).max_by(|a, b| if mm::newer(a, b) { std::cmp::Ordering::Greater } else if mm::newer(b, a) { std::cmp::Ordering::Less } else { std::cmp::Ordering::Equal })?;
            (!version.is_empty() && mm::newer(&latest, &version)).then(|| Update { version: latest, url: format!("{}/{}", horizon_client::mod_registry::SITE, id) })
        });
        let (summary, description) = if id == mm::ID {
            (mm::tr(ru, "modmenu_description").to_owned(), mm::tr(ru, "modmenu_description").to_owned())
        } else {
            (summary, description)
        };
        out.push(Mod {
            name: if id == mm::ID { mm::tr(ru, "modmenu_name").to_owned() } else { name },
            version,
            summary,
            description,
            authors: if id == mm::ID {
                mm::AUTHORS.iter().map(|a| (*a).to_owned()).collect()
            } else if !meta.authors.is_empty() {
                meta.authors.clone()
            } else {
                project.map(|p| p.authors.clone()).unwrap_or_default()
            },
            contributors: if id == mm::ID { mm::CONTRIBUTORS.iter().map(|a| (*a).to_owned()).collect() } else { meta.contributors.clone() },
            maintainers: meta.maintainers.clone(),
            licenses: meta
                .license
                .clone()
                .or_else(|| project.map(|p| p.license.clone()).filter(|l| !l.is_empty()))
                .or_else(|| (id == mm::ID).then(|| "MIT".to_owned()))
                .map(|l| vec![l])
                .unwrap_or_default(),
            website: meta
                .homepage
                .clone()
                .or_else(|| (project.is_some() || id == mm::ID).then(|| format!("{}/{}", horizon_client::mod_registry::SITE, id))),
            issues: meta.issues.clone().or_else(|| source.as_deref().and_then(issues_of)),
            source,
            badges,
            icon: match (meta.icon.clone(), project.and_then(|p| p.icon.clone())) {
                _ if id == mm::ID => Icon::ModMenu,
                (Some(path), _) => Icon::File(path),
                (None, Some(url)) if url.starts_with("https://") => Icon::Url(url),
                _ => Icon::Unknown,
            },
            configure,
            update,
            id,
        });
    }
    out
}

/// Horizon's build: the Nightly it was built on, from the build's receipt.
fn horizon_build() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|exe| std::fs::read_to_string(exe.with_file_name("native-mods.json")).ok())
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|v| v.get("base").and_then(|b| b.as_str()).map(str::to_owned))
        .unwrap_or_else(|| horizon_core::release::VERSION.to_owned())
}

// ---- icons --------------------------------------------------------------------------

/// An image decoded from PNG (or WebP, JPEG) bytes, at most 64 px a side.
fn decode(bytes: &[u8]) -> Option<Image> {
    let (w, h, rgba) = horizon_client::hud::decode_image_bytes(bytes).ok()?;
    if w == 0 || h == 0 {
        return None;
    }
    let step = (w.max(h)).div_ceil(64).max(1);
    let (nw, nh) = (w / step, h / step);
    let mut px = Vec::with_capacity((nw * nh) as usize);
    for y in 0..nh {
        for x in 0..nw {
            let i = (((y * step) * w + x * step) * 4) as usize;
            let [r, g, b, a] = [rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]];
            px.push(u32::from_be_bytes([a, r, g, b]));
        }
    }
    Some(Image { w: nw, h: nh, px })
}

/// Fetches the catalog's icons not yet known, on a thread of their own, kept
/// in the mod's data folder for next time.
fn fetch_icons(game: &Game<'_>, s: &mut State) {
    // The mods' own icons, read at once.
    let files: Vec<(String, String)> = s
        .mods
        .iter()
        .filter_map(|m| match &m.icon {
            Icon::File(path) if !s.icons.contains_key(&m.id) => Some((m.id.clone(), path.clone())),
            _ => None,
        })
        .collect();
    for (id, path) in files {
        let image = std::fs::read(&path).ok().as_deref().and_then(decode);
        s.icons.insert(id, image.map_or(IconState::Failed, IconState::Ready));
    }
    let jobs: Vec<(String, String)> = s
        .mods
        .iter()
        .filter_map(|m| match &m.icon {
            Icon::Url(url) if !s.icons.contains_key(&m.id) => Some((m.id.clone(), url.clone())),
            _ => None,
        })
        .collect();
    if jobs.is_empty() {
        return;
    }
    for (id, _) in &jobs {
        s.icons.insert(id.clone(), IconState::Loading);
    }
    let dir = game.native_data_dir(mm::ID).join("icons");
    let (tx, rx) = mpsc::channel();
    s.icon_rx = Some(rx);
    std::thread::spawn(move || {
        let _ = std::fs::create_dir_all(&dir);
        for (id, url) in jobs {
            let path = dir.join(format!("{id}.img"));
            let bytes = std::fs::read(&path).ok().or_else(|| {
                let bytes = horizon_client::mod_registry::download(&url, 1 << 20).ok()?;
                let _ = std::fs::write(&path, &bytes);
                Some(bytes)
            });
            if tx.send((id, bytes.as_deref().and_then(decode))).is_err() {
                return;
            }
        }
    });
}

fn take_icons(s: &mut State) {
    let Some(rx) = s.icon_rx.as_ref() else {
        return;
    };
    let mut done = Vec::new();
    while let Ok(item) = rx.try_recv() {
        done.push(item);
    }
    for (id, image) in done {
        s.icons.insert(id, image.map_or(IconState::Failed, IconState::Ready));
    }
}

// ---- opening and closing ------------------------------------------------------------

fn open(game: &mut Game<'_>, origin: Origin) {
    let ru = language_ru();
    if origin == Origin::Playing {
        // The game pauses under the screen, as under the pause menu.
        game.pause_menu = true;
        game.pause_focus = None;
        game.screen = Screen::Paused;
        game.capture(false);
    }
    if origin == Origin::Title
        && let Some(t) = game.title.as_mut()
    {
        t.focused_button = None;
    }
    let game: &Game<'_> = game;
    let mods = load_mods(game, ru);
    let horizon = decode(horizon_theme::ICON_PNG);
    with(|s| {
        read_config(game, s);
        s.ru = ru;
        s.open = Some(origin);
        s.page = Page::List;
        s.mods = mods;
        s.selected = None;
        s.search.set_value("");
        s.search.focused = true;
        s.search.reset_blink();
        s.list_focused = false;
        s.filters_shown = false;
        s.list_scroll = 0.0;
        s.desc_scroll = 0.0;
        s.options_scroll = 0.0;
        s.drag = None;
        s.last_click = None;
        s.layout = None;
        s.links.clear();
        s.hits.clear();
        if s.horizon_icon.is_none() {
            s.horizon_icon = horizon;
        }
        s.resort();
        s.count = Some((Instant::now(), mm::displayed_count(&s.mods, &s.config), s.mods.iter().any(|m| m.update.is_some())));
        fetch_icons(game, s);
    });
}

fn close(game: &mut Game<'_>) {
    let origin = with(|s| {
        let o = s.open.take();
        s.page = Page::List;
        s.search.focused = false;
        o
    })
    .flatten();
    if origin == Some(Origin::Playing) && game.screen == Screen::Paused {
        game.screen = Screen::Playing;
        game.capture(true);
    }
    title_side::COVER.store(false, Ordering::Relaxed);
    mm::PAUSE_COVER.store(false, Ordering::Relaxed);
}

/// The pause menu's Mods button.
pub(super) fn open_from_pause(game: &mut Game<'_>) {
    open(game, Origin::Pause);
}

/// The title screen shows its main screen.
fn title_main(game: &Game<'_>) -> bool {
    game.title.as_ref().is_some_and(|t| matches!(t.mode, horizon_client::title::Mode::Main))
}

// ---- hooks ---------------------------------------------------------------------------

/// Options > Mods: the mod's row and its key.
pub(super) fn mods(_game: &mut Game<'_>, mods: &mut Vec<super::native_hooks::NativeMod>) {
    let ru = language_ru();
    mods.push(super::native_hooks::NativeMod {
        id: mm::ID.into(),
        title: format!("{} {}", mm::tr(ru, "modmenu_name"), mm::VERSION),
        description: mm::tr(ru, "description").into(),
        keys: vec![["open_menu".into(), mm::tr(ru, "key_open").into(), "key.keyboard.unknown".into()]],
    });
}

/// Once a frame: the title's and the pause menu's button, opening and
/// closing, and what the hooks asked for.
pub(super) fn frame(game: &mut Game<'_>) {
    if !on(game) {
        title_side::READY.store(false, Ordering::Relaxed);
        title_side::STYLE.store(title_side::NONE, Ordering::Relaxed);
        mm::PAUSE_STYLE.store(mm::NONE, Ordering::Relaxed);
        if let Ok(mut count) = title_side::COUNT.lock() {
            count.clear();
        }
        with(|s| keep_startup(s, format!("{}\n\n\n", title_side::NONE)));
        if with(|s| s.open.is_some()).unwrap_or(false) {
            close(game);
        }
        return;
    }
    title_side::READY.store(true, Ordering::Relaxed);
    {
        let g: &Game<'_> = game;
        let ru = language_ru();
        // The count on the title and the Mods button, read again every few seconds.
        let stale = with(|s| s.count.is_none_or(|(at, _, _)| at.elapsed().as_secs() >= 5)).unwrap_or(false);
        let fresh = stale.then(|| load_mods(g, ru));
        with(|s| {
            if !s.config_read || stale {
                read_config(g, s);
            }
            if let Some(mods) = fresh {
                s.count = Some((Instant::now(), mm::displayed_count(&mods, &s.config), mods.iter().any(|m| m.update.is_some())));
            }
            let count = s.count.map_or(0, |c| c.1);
            let c = &s.config;
            let style = if c.modify_title_screen { c.title_style } else { title_side::NONE };
            title_side::STYLE.store(style, Ordering::Relaxed);
            mm::PAUSE_STYLE.store(if c.modify_game_menu { c.game_menu_style } else { mm::NONE }, Ordering::Relaxed);
            let text = mm::button_text(ru, count, c);
            if let Ok(mut label) = title_side::LABEL.lock() {
                label.clone_from(&text);
            }
            // " (25 Mods)" after the title's version (`MixinTitleScreen`).
            let after = if c.modify_title_screen && c.count_at.title() { mm::title_count(ru, count, c.easter_eggs) } else { String::new() };
            if let Ok(mut slot) = title_side::COUNT.lock()
                && *slot != after
            {
                slot.clone_from(&after);
            }
            keep_startup(s, format!("{style}\n{text}\n{after}\n"));
            take_icons(s);
        });
    }
    if title_side::OPEN_REQUEST.swap(false, Ordering::Relaxed) && title_main(game) {
        open(game, Origin::Title);
    }
    let actions = with(|s| std::mem::take(&mut s.pending)).unwrap_or_default();
    for action in actions {
        act(game, action);
    }
    // The screen goes when what it was opened over does.
    let origin = with(|s| s.open).flatten();
    let gone = match origin {
        Some(Origin::Title) => game.title.is_none(),
        Some(Origin::Pause | Origin::Playing) => game.title.is_some() || game.screen == Screen::Playing,
        None => false,
    };
    if gone {
        close(game);
    }
    let origin = with(|s| s.open).flatten();
    title_side::COVER.store(origin == Some(Origin::Title) && game.screen == Screen::Playing && title_main(game), Ordering::Relaxed);
    mm::PAUSE_COVER.store(matches!(origin, Some(Origin::Pause | Origin::Playing)) && game.screen == Screen::Paused, Ordering::Relaxed);
}

/// What the title shows, kept for the startup screen of the next start.
fn keep_startup(s: &mut State, text: String) {
    if s.startup.as_deref() == Some(text.as_str()) {
        return;
    }
    let path = title_side::startup_path();
    if std::fs::read_to_string(&path).ok().as_deref() != Some(text.as_str()) {
        let _ = path.parent().map(std::fs::create_dir_all);
        let _ = std::fs::write(&path, &text);
    }
    s.startup = Some(text);
}

fn act(game: &mut Game<'_>, action: Action) {
    match action {
        Action::Open(origin) => {
            if with(|s| s.open.is_none()).unwrap_or(false) {
                open(game, origin);
            }
        }
        Action::Close => close(game),
        Action::Click => game.audio.play_button_click(),
        Action::OpenFolder => super::packs::open_folder(&Game::game_dir().join(horizon_client::mod_list::NATIVE_DIR)),
        Action::Uri(url) => super::packs::open_uri(&url),
        Action::Copy(text) => {
            if !text.is_empty() {
                game.clipboard.set(&text);
            }
        }
        Action::Paste => {
            let text = game.clipboard.get();
            with(|s| {
                let text: String = text.chars().filter(|c| !c.is_control()).collect();
                s.search.insert_text(&text);
                s.refilter();
            });
        }
        Action::Credits => {
            let from_title = with(|s| s.open == Some(Origin::Title)).unwrap_or(false);
            game.open_options_at(from_title, horizon_client::options_ui::Page::Credits);
        }
        Action::Configure(target) => {
            let from_title = with(|s| s.open == Some(Origin::Title)).unwrap_or(false);
            match target {
                Configure::Native(id) => {
                    game.list_native_mods();
                    if let Some(i) = game.settings.mods.installed.iter().position(|m| m.id == id && m.native.is_some()) {
                        game.open_options_at(from_title, horizon_client::options_ui::Page::NativeMod(i));
                        game.native_settings_refresh(i);
                    }
                }
                Configure::Screen(id) => {
                    game.list_native_mods();
                    if let Some(i) = game.settings.mods.installed.iter().position(|m| m.id == id && m.native.is_some()) {
                        game.open_options_at(from_title, horizon_client::options_ui::Page::NativeMod(i));
                        game.native_settings_refresh(i);
                        game.native_settings_click(&id, mm::MODMENU_CONFIGURE);
                    }
                }
                Configure::Options => game.open_options_at(from_title, horizon_client::options_ui::Page::Main),
                Configure::Horizon => game.open_options_at(from_title, horizon_client::options_ui::Page::Horizon),
                Configure::ModMenu => {
                    with(|s| {
                        s.page = Page::Options;
                        s.options_scroll = 0.0;
                    });
                }
                Configure::None => {}
            }
        }
    }
}

/// The mods screen is up over `screen` (its hooks take everything there).
pub(super) fn covers(game: &Game<'_>, screen: &horizon_client::mod_api::NativeScreenView<'_>) -> bool {
    if !on(game) {
        return false;
    }
    with(|s| match s.open {
        Some(Origin::Title) => screen.id == "title",
        Some(Origin::Pause | Origin::Playing) => screen.id == "paused",
        None => false,
    })
    .unwrap_or(false)
}

/// Open Mod Menu: from the title, the pause menu or the game.
pub(super) fn key(game: &Game<'_>, key: &str, pressed: bool) {
    if !pressed || !on(game) {
        return;
    }
    let b = bound(game);
    if b == "key.keyboard.unknown" || b != key {
        return;
    }
    let origin = if game.title.is_some() {
        (title_main(game) && game.screen == Screen::Playing).then_some(Origin::Title)
    } else if game.screen == Screen::Paused {
        Some(Origin::Pause)
    } else if game.screen == Screen::Playing {
        Some(Origin::Playing)
    } else {
        None
    };
    if let Some(origin) = origin {
        with(|s| {
            if s.open.is_none() {
                s.pending.push(Action::Open(origin));
            }
        });
    }
}

fn hit_at(s: &State, p: (f32, f32)) -> Option<Hit> {
    s.hits.iter().rev().find(|(r, _)| p.0 >= r[0] && p.0 < r[0] + r[2] && p.1 >= r[1] && p.1 < r[1] + r[3]).map(|(_, h)| *h)
}

pub(super) fn pressed(game: &Game<'_>, screen: &horizon_client::mod_api::NativeScreenView<'_>, button: i32, modifiers: u32) -> bool {
    if !covers(game, screen) {
        return false;
    }
    let p = (screen.mouse[0], screen.mouse[1]);
    let shift = modifiers & SHIFT != 0;
    let left = button == 0;
    let atlas = &game.gui.atlas;
    with(|s| {
        let hit = hit_at(s, p);
        match s.page.clone() {
            Page::Link(url) => {
                if let Some(Hit::LinkButton(i)) = hit
                    && left
                {
                    s.pending.push(Action::Click);
                    match i {
                        0 => s.pending.push(Action::Uri(url)),
                        1 => s.pending.push(Action::Copy(url)),
                        _ => {}
                    }
                    s.page = Page::List;
                }
                return true;
            }
            Page::Options => {
                match hit {
                    Some(Hit::Option(i)) if left => {
                        if let Some(o) = mm::OPTIONS.get(i) {
                            let next = mm::cycle(o.key, &option_token(game, o.key), shift);
                            set_option(game, s, o.key, &next);
                            s.pending.push(Action::Click);
                        }
                    }
                    Some(Hit::OptionsDone) if left => {
                        s.pending.push(Action::Click);
                        leave_options(s);
                    }
                    Some(Hit::Bar(2)) => s.drag = Some(Drag { bar: 2, last_y: p.1 }),
                    _ => {}
                }
                return true;
            }
            Page::List => {}
        }
        if hit != Some(Hit::Search) {
            s.search.focused = false;
        }
        match hit {
            Some(Hit::Search) => {
                s.search.focused = true;
                s.list_focused = false;
                s.search.click(atlas, p.0, shift, false);
                s.search.reset_blink();
            }
            Some(Hit::Filters) if left => {
                s.pending.push(Action::Click);
                s.filters_shown = !s.filters_shown;
            }
            Some(Hit::Sort) if left => {
                s.pending.push(Action::Click);
                let next = mm::cycle("sorting", &option_token(game, "sorting"), shift);
                set_option(game, s, "sorting", &next);
                s.resort();
            }
            Some(Hit::Libraries) if left => {
                s.pending.push(Action::Click);
                let next = mm::cycle("show_libraries", &option_token(game, "show_libraries"), shift);
                set_option(game, s, "show_libraries", &next);
                s.refilter();
            }
            Some(Hit::Configure) if left => {
                if let Some(m) = s.selected_mod() {
                    let target = m.configure.clone();
                    s.pending.push(Action::Click);
                    s.pending.push(Action::Configure(target));
                }
            }
            Some(Hit::Website) if left => {
                if let Some(url) = s.selected_mod().and_then(|m| m.website.clone()) {
                    s.pending.push(Action::Click);
                    s.page = Page::Link(url);
                }
            }
            Some(Hit::Issues) if left => {
                if let Some(url) = s.selected_mod().and_then(|m| m.issues.clone()) {
                    s.pending.push(Action::Click);
                    s.page = Page::Link(url);
                }
            }
            Some(Hit::ModsFolder) if left => {
                s.pending.push(Action::Click);
                s.pending.push(Action::OpenFolder);
            }
            Some(Hit::Done) if left => {
                s.pending.push(Action::Click);
                s.pending.push(Action::Close);
            }
            Some(Hit::Row(i)) => {
                let Some(&mi) = s.rows.get(i) else {
                    return true;
                };
                s.list_focused = true;
                let id = s.mods[mi].id.clone();
                let changed = s.selected.as_deref() != Some(&id);
                s.selected = Some(id.clone());
                if changed {
                    s.desc_scroll = 0.0;
                }
                // Quick configure: a click on the icon, or a double click.
                let m = &s.mods[mi];
                let size = if s.config.compact_list { 19.0 } else { 32.0 };
                let on_icon = p.0 - 6.0 <= size;
                let double = s.last_click.as_ref().is_some_and(|(at, last)| *last == id && at.elapsed().as_millis() < 250);
                if s.config.quick_configure && m.configurable() && (on_icon || double) {
                    let target = m.configure.clone();
                    s.pending.push(Action::Configure(target));
                }
                s.last_click = Some((Instant::now(), id));
            }
            Some(Hit::DescLink(i)) if left => {
                if let Some(link) = s.links.get(i).cloned() {
                    s.pending.push(Action::Click);
                    match link {
                        Link::Url(url) => s.page = Page::Link(url),
                        Link::Credits => s.pending.push(Action::Credits),
                    }
                }
            }
            Some(Hit::Bar(bar)) => s.drag = Some(Drag { bar, last_y: p.1 }),
            _ => {}
        }
        true
    })
    .unwrap_or(true)
}

/// Back from the options to the list, which takes them in (`init` again).
fn leave_options(s: &mut State) {
    s.page = Page::List;
    s.layout = None;
    s.resort();
}

pub(super) fn released(game: &Game<'_>, screen: &horizon_client::mod_api::NativeScreenView<'_>, _button: i32) -> bool {
    if !covers(game, screen) {
        return false;
    }
    with(|s| s.drag = None);
    true
}

/// A scroll bar follows the pointer (`AbstractScrollArea`'s drag): the
/// content moves `max scroll / (bar - thumb)` per pixel.
pub(super) fn dragged(game: &Game<'_>, screen: &horizon_client::mod_api::NativeScreenView<'_>) -> bool {
    if !covers(game, screen) {
        return false;
    }
    let y = screen.mouse[1];
    with(|s| {
        let Some(drag) = s.drag.as_mut() else {
            return;
        };
        let dy = y - drag.last_y;
        drag.last_y = y;
        let bar = drag.bar;
        let Some((r, content)) = s.bars[usize::from(bar)] else {
            return;
        };
        let h = r[3];
        let max = (content - h).max(0.0);
        if max <= 0.0 {
            return;
        }
        let th = (h * h / content).floor().clamp(32.0, (h - 8.0).max(32.0));
        let rate = (max / (h - th).max(1.0)).max(1.0);
        let scroll = match bar {
            0 => &mut s.list_scroll,
            1 => &mut s.desc_scroll,
            _ => &mut s.options_scroll,
        };
        *scroll = (*scroll + dy * rate).clamp(0.0, max);
    });
    true
}

/// The wheel: the list 18 px a notch (11 compact), the description 5, the
/// options 12 (half a row each, `AbstractSelectionList`).
pub(super) fn scrolled(game: &Game<'_>, screen: &horizon_client::mod_api::NativeScreenView<'_>, scroll: [f64; 2]) -> bool {
    if !covers(game, screen) {
        return false;
    }
    let p = (screen.mouse[0], screen.mouse[1]);
    let delta = scroll[1] as f32;
    with(|s| {
        let inside = |r: [f32; 4]| p.0 >= r[0] && p.0 < r[0] + r[2] && p.1 >= r[1] && p.1 < r[1] + r[3];
        for bar in 0..3u8 {
            let Some((r, content)) = s.bars[usize::from(bar)] else {
                continue;
            };
            if !inside(r) {
                continue;
            }
            let rate = match bar {
                0 => {
                    if s.config.compact_list {
                        11.0
                    } else {
                        18.0
                    }
                }
                1 => 5.0,
                _ => 12.0,
            };
            let max = (content - r[3]).max(0.0);
            let target = match bar {
                0 => &mut s.list_scroll,
                1 => &mut s.desc_scroll,
                _ => &mut s.options_scroll,
            };
            *target = (*target - delta * rate).clamp(0.0, max);
            return;
        }
    });
    true
}

/// A key that writes text when typed (a letter, a digit, a sign, Space).
fn writes(key: &str) -> bool {
    let Some(name) = key.strip_prefix("key.keyboard.") else {
        return false;
    };
    name.chars().count() == 1 || name == "space" || name.starts_with("keypad.") && name.len() == 8
}

pub(super) fn key_pressed(game: &Game<'_>, screen: &horizon_client::mod_api::NativeScreenView<'_>, key: &str, modifiers: u32) -> bool {
    if !covers(game, screen) {
        return false;
    }
    let shift = modifiers & SHIFT != 0;
    let control = modifiers & CONTROL != 0;
    with(|s| {
        if key == "key.keyboard.escape" {
            match s.page {
                Page::Link(_) => s.page = Page::List,
                Page::Options => leave_options(s),
                Page::List => s.pending.push(Action::Close),
            }
            return true;
        }
        if s.page != Page::List {
            return true;
        }
        if key == "key.keyboard.tab" {
            s.search.focused = !s.search.focused;
            s.list_focused = !s.search.focused;
            return true;
        }
        if s.search.focused {
            match key {
                "key.keyboard.v" if control => s.pending.push(Action::Paste),
                "key.keyboard.c" if control => s.pending.push(Action::Copy(s.search.selected_text())),
                "key.keyboard.x" if control => {
                    s.pending.push(Action::Copy(s.search.selected_text()));
                    s.search.delete_chars(0);
                    s.refilter();
                }
                // Over the title, typed text comes through `title_typed`: let it pass.
                _ if !control && writes(key) && s.open == Some(Origin::Title) => return false,
                _ if !control && writes(key) => {}
                _ => {
                    if let Some(code) = horizon_client::keybinds::key_code(key) {
                        let before = s.search.value.clone();
                        s.search.key(code, control, shift);
                        if s.search.value != before {
                            s.refilter();
                        }
                    }
                }
            }
            s.search.reset_blink();
            return true;
        }
        // The list: Up and Down move the selection (vanilla's focus moves).
        if s.list_focused && (key == "key.keyboard.up" || key == "key.keyboard.down") {
            let at = s.selected.as_deref().and_then(|id| s.rows.iter().position(|&i| s.mods[i].id == id));
            let n = s.rows.len();
            if n > 0 {
                let next = match (at, key) {
                    (Some(i), "key.keyboard.up") => i.saturating_sub(1),
                    (Some(i), _) => (i + 1).min(n - 1),
                    (None, _) => 0,
                };
                s.selected = Some(s.mods[s.rows[next]].id.clone());
                s.desc_scroll = 0.0;
                // Into view.
                let item = if s.config.compact_list { 23.0 } else { 36.0 };
                if let Some((r, _)) = s.bars[0] {
                    let top = next as f32 * item;
                    if top < s.list_scroll {
                        s.list_scroll = top;
                    } else if top + item + 4.0 > s.list_scroll + r[3] {
                        s.list_scroll = top + item + 4.0 - r[3];
                    }
                }
            }
        }
        true
    })
    .unwrap_or(true)
}

/// A character typed over the pause menu (the title's come through `title_typed`).
pub(super) fn char_typed(game: &Game<'_>, screen: &horizon_client::mod_api::NativeScreenView<'_>, c: char) -> bool {
    if !covers(game, screen) {
        return false;
    }
    with(|s| {
        if s.page == Page::List && s.search.focused && !c.is_control() {
            s.search.insert_text(&c.to_string());
            s.search.reset_blink();
            s.refilter();
        }
    });
    true
}

/// Text typed over the title screen while the mods screen is up: the search
/// box's when it has the focus, nobody's otherwise.
pub(super) fn title_typed(game: &mut Game<'_>, typed: Option<&str>, backspace: bool) -> bool {
    let open = with(|s| s.open == Some(Origin::Title)).unwrap_or(false);
    if !open || !on(game) || game.screen != Screen::Playing || !title_main(game) {
        return false;
    }
    with(|s| {
        if s.page != Page::List || !s.search.focused {
            return;
        }
        if backspace {
            s.search.delete_chars(-1);
        }
        if let Some(text) = typed {
            let text: String = text.chars().filter(|c| !c.is_control()).collect();
            s.search.insert_text(&text);
        }
        s.search.reset_blink();
        s.refilter();
    });
    true
}
