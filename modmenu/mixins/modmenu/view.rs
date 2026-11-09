// Mod Menu for Horizon: drawing. The mods screen (`ModsScreen`: the list,
// the selected mod's details, the search box, the filters and the buttons),
// Mod Menu's options (`ModMenuOptionsScreen`) and the link confirmation
// (`ConfirmLinkScreen`), laid out with Mod Menu's own numbers in the game's
// widgets and Mod Menu's textures; and, while it is closed, the title
// screen's mod count and the Mods icon buttons. Every part drawn is recorded
// in `State::hits` for the mouse of the next event.
use super::Game;
use super::modmenu::{self as mm, Badge, Icon, Mod};
use super::modmenu_glue::{self as glue, Hit, IconState, Image, Layout, Link, Origin, Page, State};
use super::modmenu_sprites::{self as sprites, Sprite};
use horizon_client::hud::{HudBatch, rgba};
use horizon_client::text::Text;
use horizon_client::title::modmenu_title as title_side;
use horizon_client::ui;

type Rect = [f32; 4];

const WHITE: u32 = 0xffff_ffff;
const GRAY_TEXT: u32 = 0xffaa_aaaa;
const SUMMARY: u32 = 0xff80_8080;
const TREND: &str = "gui/sprites/icon/trial_available";

// ---- primitives ------------------------------------------------------------------------

/// The game's colour from 0xAARRGGBB.
fn argb(c: u32) -> u32 {
    let [a, r, g, b] = c.to_be_bytes();
    rgba(r, g, b, a)
}

fn rect(game: &Game<'_>, batch: &mut HudBatch, r: Rect, color: u32) {
    if r[2] > 0.0 && r[3] > 0.0 {
        batch.fill(&game.gui.atlas, r[0], r[1], r[2], r[3], argb(color));
    }
}

/// Vanilla's `fill(x0, y0, x1, y1)`.
fn fill(game: &Game<'_>, batch: &mut HudBatch, x0: f32, y0: f32, x1: f32, y1: f32, color: u32) {
    rect(game, batch, [x0, y0, x1 - x0, y1 - y0], color);
}

fn text(game: &Game<'_>, batch: &mut HudBatch, s: &str, x: f32, y: f32, color: u32, shadow: bool) {
    batch.text(&game.gui.atlas, s, x.floor(), y.floor(), argb(color), shadow);
}

fn width(game: &Game<'_>, s: &str) -> f32 {
    HudBatch::text_width(&game.gui.atlas, s)
}

fn inside(p: (f32, f32), r: Rect) -> bool {
    p.0 >= r[0] && p.0 < r[0] + r[2] && p.1 >= r[1] && p.1 < r[1] + r[3]
}

fn intersect(a: Rect, b: Rect) -> Option<Rect> {
    let x0 = a[0].max(b[0]);
    let y0 = a[1].max(b[1]);
    let x1 = (a[0] + a[2]).min(b[0] + b[2]);
    let y1 = (a[1] + a[3]).min(b[1] + b[3]);
    (x1 > x0 && y1 > y0).then_some([x0, y0, x1 - x0, y1 - y0])
}

/// One of Mod Menu's sprites drawn `size` wide.
fn blit(game: &Game<'_>, batch: &mut HudBatch, s: &Sprite, x: f32, y: f32, size: f32) {
    let k = size / f32::from(s.width);
    let atlas = &game.gui.atlas;
    for &(px, py, len, c) in s.runs {
        batch.fill(atlas, x + f32::from(px) * k, y + f32::from(py) * k, f32::from(len) * k, k, c);
    }
}

/// A decoded icon drawn `size` wide, nearest-sampled.
fn image(game: &Game<'_>, batch: &mut HudBatch, img: &Image, x: f32, y: f32, size: f32) {
    let k = size / img.w.max(img.h) as f32;
    let atlas = &game.gui.atlas;
    for py in 0..img.h {
        let row = &img.px[(py * img.w) as usize..((py + 1) * img.w) as usize];
        let mut px = 0;
        while px < img.w as usize {
            let c = row[px];
            let mut len = 1;
            while px + len < row.len() && row[px + len] == c {
                len += 1;
            }
            if c >> 24 != 0 {
                batch.fill(atlas, x + px as f32 * k, y + py as f32 * k, len as f32 * k, k, argb(c));
            }
            px += len;
        }
    }
}

/// A mod's icon (`Mod.getIcon`): the catalog's, Mod Menu's own, Minecraft's
/// or the unknown one while it loads or when there is none.
fn icon(game: &Game<'_>, batch: &mut HudBatch, s: &State, m: &Mod, x: f32, y: f32, size: f32) {
    match &m.icon {
        Icon::Minecraft => blit(game, batch, &sprites::MINECRAFT, x, y, size),
        Icon::ModMenu => blit(game, batch, &sprites::ICON, x, y, size),
        Icon::Horizon => match &s.horizon_icon {
            Some(img) => image(game, batch, img, x, y, size),
            None => blit(game, batch, &sprites::UNKNOWN, x, y, size),
        },
        Icon::Url(_) | Icon::File(_) => match s.icons.get(&m.id) {
            Some(IconState::Ready(img)) => image(game, batch, img, x, y, size),
            _ => blit(game, batch, &sprites::UNKNOWN, x, y, size),
        },
        Icon::Unknown => blit(game, batch, &sprites::UNKNOWN, x, y, size),
    }
}

/// `font.substrByWidth(text, max)` with "..." when it is cut.
fn trim(game: &Game<'_>, s: &str, max: f32) -> String {
    if width(game, s) <= max {
        return s.to_owned();
    }
    let limit = max - width(game, "...");
    let mut out = String::new();
    for c in s.chars() {
        out.push(c);
        if width(game, &out) > limit {
            out.pop();
            break;
        }
    }
    format!("{out}...")
}

fn wrap(game: &Game<'_>, s: &str, w: f32) -> Vec<Text> {
    let s = s.trim_end_matches('\n');
    if s.is_empty() {
        return Vec::new();
    }
    game.gui.atlas.font.split_lines(&Text::legacy(s), w.max(1.0)).into_iter().map(|(t, _)| t).collect()
}

/// `DrawingUtil.drawWrappedString`: at most `lines` lines, the last with
/// "..." when more were cut.
#[allow(clippy::too_many_arguments)]
fn wrapped(game: &Game<'_>, batch: &mut HudBatch, s: &str, x: f32, y: f32, w: f32, lines: usize, color: u32) {
    let all = wrap(game, s, w);
    let atlas = &game.gui.atlas;
    for (i, line) in all.iter().take(lines).enumerate() {
        let mut t = line.clone();
        if i + 1 == lines && all.len() > lines {
            t = t.append(Text::literal("..."));
        }
        batch.text_styled(atlas, &t, x.floor(), (y + 9.0 * i as f32).floor(), argb(color), true);
    }
}

/// `ModBadgeRenderer`: each badge's box with its corners cut, while it fits
/// before `end`.
fn badges(game: &Game<'_>, batch: &mut HudBatch, ru: bool, list: &[Badge], start: f32, y: f32, end: f32) {
    let mut bx = start;
    for &badge in list {
        let label = badge.text(ru);
        let tw = width(game, label);
        let w = tw + 6.0;
        if bx + w >= end {
            break;
        }
        let (outline, inner) = badge.colours();
        fill(game, batch, bx + 1.0, y - 1.0, bx + w, y, outline);
        fill(game, batch, bx, y, bx + 1.0, y + 9.0, outline);
        fill(game, batch, bx + 1.0, y + 9.0, bx + w, y + 10.0, outline);
        fill(game, batch, bx + w, y, bx + w + 1.0, y + 9.0, outline);
        fill(game, batch, bx + 1.0, y, bx + w, y + 9.0, inner);
        text(game, batch, label, (bx + 1.0 + (w - tw) / 2.0).floor(), y + 1.0, 0xffca_caca, false);
        bx += w + 3.0;
    }
}

/// A list's background (`menu_list_background`) and its separators.
fn list_background(game: &Game<'_>, batch: &mut HudBatch, r: Rect) {
    rect(game, batch, r, 0x7000_0000);
    rect(game, batch, [r[0], r[1] - 2.0, r[2], 1.0], 0x33ff_ffff);
    rect(game, batch, [r[0], r[1] - 1.0, r[2], 1.0], 0xbf00_0000);
    rect(game, batch, [r[0], r[1] + r[3], r[2], 1.0], 0xbf00_0000);
    rect(game, batch, [r[0], r[1] + r[3] + 1.0, r[2], 1.0], 0x33ff_ffff);
}

fn button(game: &Game<'_>, batch: &mut HudBatch, r: Rect, label: &Text, active: bool) {
    let hover = active && inside(game.cursor, r);
    ui::button(batch, &game.gui.atlas, r[0], r[1], r[2], r[3], label, active, hover);
}

/// An icon button (`SpriteIconButton`): the game's button with Mod Menu's
/// 20x20 sprite over it.
fn icon_button(game: &Game<'_>, batch: &mut HudBatch, r: Rect, sprite: &Sprite, active: bool) {
    button(game, batch, r, &Text::empty(), active);
    blit(game, batch, sprite, r[0], r[1], 20.0);
}

fn tooltip(game: &Game<'_>, batch: &mut HudBatch, lines: &[String]) {
    let lines: Vec<Text> = lines.iter().map(|l| Text::legacy(l)).collect();
    ui::tooltip(batch, &game.gui.atlas, &lines, game.cursor);
}

/// `UpdateAvailableBadge`: `icon/trial_available`, 8 px, its frames turning as
/// its mcmeta says. The game's GUI atlas turns them only in a world that isn't
/// paused, where vanilla ticks GUI textures 20 times a second everywhere, so
/// the frame for the time is drawn from the sprite's own pixels.
fn update_badge(game: &Game<'_>, batch: &mut HudBatch, x: f32, y: f32) {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    let atlas = &game.gui.atlas;
    let frame = atlas.sprites.get(TREND).and_then(|rect| {
        let (_, animation) = atlas.sprite_animations.iter().find(|(r, _)| r.u0 == rect.u0 && r.v0 == rect.v0)?;
        let (w, h) = rect.texels((atlas.width, atlas.height));
        let tick = START.get_or_init(std::time::Instant::now).elapsed().as_millis() as u64 / 50;
        let px = animation.pixels(animation.frame(tick));
        (w > 0 && h > 0 && px.len() == (w * h * 4) as usize).then_some((w, h, px))
    });
    let Some((w, h, px)) = frame else {
        ui::blit_sprite(batch, atlas, TREND, x, y, 8.0, 8.0, u32::MAX);
        return;
    };
    let k = 8.0 / w.max(h) as f32;
    for ty in 0..h {
        for tx in 0..w {
            let i = ((ty * w + tx) * 4) as usize;
            if px[i + 3] != 0 {
                batch.fill(atlas, x + tx as f32 * k, y + ty as f32 * k, k, k, rgba(px[i], px[i + 1], px[i + 2], px[i + 3]));
            }
        }
    }
}

// ---- the hook --------------------------------------------------------------------------

/// After a screen's own drawing (`screen_render_post`).
pub(super) fn render(game: &Game<'_>, batch: &mut HudBatch, screen: &horizon_client::mod_api::NativeScreenView<'_>) {
    if !glue::on(game) {
        return;
    }
    let covers = glue::covers(game, screen);
    glue::with(|s| {
        if covers {
            s.hits.clear();
            s.links.clear();
            match s.page.clone() {
                Page::List => list_page(game, batch, s),
                Page::Options => options_page(game, batch, s),
                Page::Link(url) => link_page(game, batch, s, &url),
            }
        } else if screen.id == "title" {
            title_extras(game, batch, s);
        } else if screen.id == "paused" {
            pause_extras(game, batch, s);
        }
    });
}

// ---- the title screen and the pause menu ------------------------------------------------

/// The title's " (N Mods)" after its version, and the Mods icon button.
fn title_extras(game: &Game<'_>, batch: &mut HudBatch, s: &State) {
    let c = &s.config;
    if !c.modify_title_screen {
        return;
    }
    // The count after the version and the Mods icon are the title's own
    // drawing (`modmenu_title::drawn`), on the startup screen too.
    let (w, h) = (batch.width.ceil(), batch.height.ceil());
    let (count, updates) = s.count.map_or((0, false), |c| (c.1, c.2));
    let Some((x, y, bw)) = title_side::button_rect(w, h) else {
        return;
    };
    let badge = c.button_update_badge && c.update_checker && updates;
    if c.title_style == 3 {
        if badge {
            update_badge(game, batch, x + bw - 5.0, y - 3.0);
        }
        if inside(game.cursor, [x, y, bw, 20.0]) {
            tooltip(game, batch, &[mm::button_text(s.ru, count, c)]);
        }
    } else if badge {
        update_badge(game, batch, x + bw - 13.0, y + 10.0 - 5.0);
    }
}

/// The pause menu's Mods icon and its tooltip.
fn pause_extras(game: &Game<'_>, batch: &mut HudBatch, s: &State) {
    let c = &s.config;
    if !c.modify_game_menu || c.game_menu_style != 1 {
        return;
    }
    let Some((x, y)) = mm::pause_icon() else {
        return;
    };
    title_side::draw_icon(batch, &game.gui.atlas, x, y);
    let (count, updates) = s.count.map_or((0, false), |c| (c.1, c.2));
    if c.button_update_badge && c.update_checker && updates {
        update_badge(game, batch, x + 15.0, y - 3.0);
    }
    if inside(game.cursor, [x, y, 20.0, 20.0]) {
        tooltip(game, batch, &[mm::button_text(s.ru, count, c)]);
        return;
    }
    // The menu drew its tooltips before this icon: one under the pointer is
    // drawn again over it, as Java draws tooltips last. The server links
    // (or server dialog) button's, which can reach up over the icon...
    if let Some((bx, by, bw)) = mm::pause_links_button()
        && inside(game.cursor, [bx, by, bw, 20.0])
    {
        let label = Text::translate("menu.custom_options.tooltip", &[]);
        let atlas = &game.gui.atlas;
        let lines: Vec<Text> = atlas.font.split_lines(&label, batch.width).into_iter().map(|(l, _)| l).collect();
        ui::tooltip(batch, atlas, &lines, game.cursor);
        return;
    }
    // ...and the icons' labels (`PauseScreen`'s icon row).
    for i in 0..4 {
        let ix = x - 96.0 + 24.0 * i as f32;
        if !inside(game.cursor, [ix, y, 20.0, 20.0]) {
            continue;
        }
        let label = match i {
            0 => Text::translate("menu.reportBugs", &[]),
            1 => Text::translate("menu.sendFeedback", &[]),
            2 if game.friends_view().shown => Text::literal(horizon_client::friends_screen::title()),
            3 => Text::translate("menu.playerReporting", &[]),
            _ => return,
        };
        let atlas = &game.gui.atlas;
        let lines: Vec<Text> = atlas.font.split_lines(&label, batch.width).into_iter().map(|(l, _)| l).collect();
        ui::tooltip(batch, atlas, &lines, game.cursor);
    }
}

// ---- the mods screen (`ModsScreen`) ------------------------------------------------------

/// "Sort: A-Z", "Libraries: With Config" (`options.generic_value`).
fn option_label(game: &Game<'_>, ru: bool, key: &str) -> String {
    let Some(o) = mm::option(key) else {
        return String::new();
    };
    let token = glue::option_token(game, key);
    let value = o.values.iter().find(|v| v.0 == token).map_or("", |v| if ru { v.2 } else { v.1 });
    format!("{}: {value}", if ru { o.caption.1 } else { o.caption.0 })
}

/// The counts' texts now: everything, mods alone, libraries alone.
fn count_texts(s: &State) -> (String, String, String) {
    let hidden = |m: &Mod| s.config.hidden_mods.contains(&m.id);
    let mods_total = s.mods.iter().filter(|m| !hidden(m) && !m.has(Badge::Library)).count();
    let libs_total = s.mods.iter().filter(|m| !hidden(m) && m.has(Badge::Library) && !mm::library_hidden(m, s.config.show_libraries)).count();
    let shown = |lib: bool| s.rows.iter().filter(|&&i| s.mods[i].has(Badge::Library) == lib).count();
    let (mods, libs) = ((shown(false), mods_total), (shown(true), libs_total));
    (mm::showing(s.ru, mods, libs), mm::showing(s.ru, mods, (0, 0)), if libs.1 > 0 { mm::showing_libraries(s.ru, libs) } else { String::new() })
}

fn list_page(game: &Game<'_>, batch: &mut HudBatch, s: &mut State) {
    let ru = s.ru;
    let cursor = game.cursor;
    let atlas = &game.gui.atlas;
    let (w, h) = (batch.width.floor(), batch.height.floor());
    let in_world = s.open != Some(Origin::Title);
    if !in_world {
        game.native_apply_screen_blur();
    }
    ui::menu_background(batch, atlas, 0.0, 0.0, w, h, in_world);

    // `ModsScreen.init`'s numbers.
    let pane_y = 67.0;
    let pane_w = (w / 2.0).floor() - 8.0;
    let right_x = w - pane_w;
    let compact = s.config.compact_list;
    let item_h = if compact { 23.0 } else { 36.0 };
    let list = [0.0, pane_y, pane_w, h - pane_y - 36.0];
    let fbs = 22.0;
    let search_w = pane_w - 32.0 - fbs;
    let search_x = (pane_w / 2.0).floor() - (search_w / 2.0).floor() - fbs / 2.0;
    let filters = [(pane_w / 2.0).floor() + (search_w / 2.0).floor() - 10.0 + 2.0, 22.0, 20.0, 20.0];
    let search_row_w = search_x + search_w + 22.0;
    let layout = match s.layout {
        Some(l) if l.size == (w, h) => l,
        _ => {
            let sort_w = width(game, &option_label(game, ru, "sorting")) + 28.0;
            let lib_w = width(game, &option_label(game, ru, "show_libraries")) + 20.0;
            let filters_w = sort_w + lib_w + 2.0;
            let n = s.mods.iter().filter(|m| !m.has(Badge::Library) && !s.config.hidden_mods.contains(&m.id)).count();
            let centred = filters_w + width(game, &mm::showing(ru, (n, n), (0, 0))) + 20.0 >= search_row_w;
            let filters_x = if centred { (pane_w / 2.0).floor() - (filters_w / 2.0).floor() } else { search_row_w - filters_w + 1.0 };
            let l = Layout { size: (w, h), filters_x, sort_w, lib_w };
            s.layout = Some(l);
            l
        }
    };
    let mut tip: Option<Vec<String>> = None;
    let selected = s.selected_mod().cloned();

    // The buttons, in the order the screen adds them.
    icon_button(game, batch, filters, &sprites::FILTERS, true);
    s.hits.push((filters, Hit::Filters));
    if inside(cursor, filters) {
        tip = Some(vec![mm::tr(ru, "toggle_filters").to_owned()]);
    }
    if s.filters_shown {
        let sort = [layout.filters_x, 45.0, layout.sort_w, 20.0];
        let libs = [layout.filters_x + layout.sort_w + 2.0, 45.0, layout.lib_w, 20.0];
        button(game, batch, sort, &Text::literal(option_label(game, ru, "sorting")), true);
        button(game, batch, libs, &Text::literal(option_label(game, ru, "show_libraries")), true);
        s.hits.push((sort, Hit::Sort));
        s.hits.push((libs, Hit::Libraries));
    }
    if let Some(m) = &selected {
        if m.configurable() && !s.config.hide_config_buttons {
            let r = [w - 24.0, 48.0, 20.0, 20.0];
            icon_button(game, batch, r, &sprites::CONFIGURE, true);
            s.hits.push((r, Hit::Configure));
            if inside(cursor, r) {
                tip = Some(vec![mm::tr(ru, "configure").to_owned()]);
            }
        }
        let url_w = (pane_w / 2.0).floor() - 2.0;
        let cap_w = url_w.min(200.0);
        let website = [right_x + (url_w / 2.0).floor() - (cap_w / 2.0).floor(), 84.0, cap_w, 20.0];
        let issues = [right_x + url_w + 4.0 + (url_w / 2.0).floor() - (cap_w / 2.0).floor(), 84.0, cap_w, 20.0];
        let (wl, il) = if m.id == "minecraft" {
            (Text::translate("menu.sendFeedback", &[]), Text::translate("menu.reportBugs", &[]))
        } else {
            (Text::literal(mm::tr(ru, "website")), Text::literal(mm::tr(ru, "issues")))
        };
        button(game, batch, website, &wl, m.website.is_some());
        button(game, batch, issues, &il, m.issues.is_some());
        s.hits.push((website, Hit::Website));
        s.hits.push((issues, Hit::Issues));
    }
    let folder = [(w / 2.0).floor() - 154.0, h - 28.0, 150.0, 20.0];
    let done = [(w / 2.0).floor() + 4.0, h - 28.0, 150.0, 20.0];
    button(game, batch, folder, &Text::literal(mm::tr(ru, "mods_folder")), true);
    button(game, batch, done, &Text::translate("gui.done", &[]), true);
    s.hits.push((folder, Hit::ModsFolder));
    s.hits.push((done, Hit::Done));

    // The selected mod's description list (`DescriptionListWidget`).
    s.bars[1] = None;
    if let Some(m) = &selected {
        let r = [right_x, 108.0, pane_w, h - 144.0];
        let lines = description(game, s, m, pane_w - 15.0);
        let content = 10.0 * lines.len() as f32 + 4.0;
        let max = (content - r[3]).max(0.0);
        s.desc_scroll = s.desc_scroll.clamp(0.0, max);
        s.bars[1] = Some((r, content));
        list_background(game, batch, r);
        let start = batch.vertices.len();
        let row_left = right_x + 5.0;
        for (i, line) in lines.iter().enumerate() {
            let y = r[1] + 2.0 + 10.0 * i as f32 - s.desc_scroll.floor();
            if y + 10.0 < r[1] || y > r[1] + r[3] {
                continue;
            }
            let mut x = row_left + line.indent;
            if line.badge {
                update_badge(game, batch, x, y + 2.0);
                x += 11.0;
            }
            batch.text_styled(atlas, &line.text, x.floor(), (y + 2.0).floor(), argb(line.color), true);
            if let Some(link) = &line.link {
                let tw = atlas.font.width_text(&line.text);
                if let Some(hr) = intersect([x, y, tw, 10.0], r) {
                    s.hits.push((hr, Hit::DescLink(s.links.len())));
                }
                s.links.push(link.clone());
            }
        }
        batch.clip_since(start, r[0], r[1], r[0] + r[2], r[1] + r[3]);
        if max > 0.0 {
            ui::scrollbar_sprites(batch, atlas, r[0] + r[2] - 6.0, r[1], r[1] + r[3], content, s.desc_scroll);
            s.hits.push(([r[0] + r[2] - 6.0, r[1], 6.0, r[3]], Hit::Bar(1)));
        }
    }

    // The mod list (`ModListWidget`).
    let n = s.rows.len();
    let total = item_h * n as f32;
    let row_w = pane_w - if total + 12.0 > list[3] { 18.0 } else { 12.0 };
    let content = total + 8.0;
    let max = (content - list[3]).max(0.0);
    s.list_scroll = s.list_scroll.clamp(0.0, max);
    s.bars[0] = Some((list, content));
    list_background(game, batch, list);
    let start = batch.vertices.len();
    let scroll = s.list_scroll.floor();
    let size = if compact { 19.0 } else { 32.0 };
    // The row under the pointer (`getEntryAtPos`, 2 px under the rows).
    let hovered = {
        let t = (cursor.1 - pane_y).floor() + scroll - 4.0;
        let index = (t / item_h).floor();
        (inside(cursor, list) && cursor.0 < pane_w - 6.0 && cursor.0 >= 6.0 && cursor.0 <= 6.0 + row_w && t >= 0.0 && (index as usize) < n).then_some(index as usize)
    };
    for (i, &mi) in s.rows.iter().enumerate() {
        let ey = pane_y + 2.0 - scroll + item_h * i as f32;
        if ey + item_h < pane_y || ey + 2.0 > list[1] + list[3] {
            continue;
        }
        let m = &s.mods[mi];
        if s.selected.as_deref() == Some(&m.id) {
            let bottom = ey + if compact { 25.0 } else { 38.0 };
            fill(game, batch, 4.0, ey + 2.0, 6.0 + row_w + 2.0, bottom, if s.list_focused { WHITE } else { 0xff80_8080 });
            fill(game, batch, 5.0, ey + 3.0, 6.0 + row_w + 1.0, bottom - 1.0, 0xff00_0000);
        }
        let (x, y) = (6.0, ey + 4.0);
        let cw = row_w - 4.0;
        icon(game, batch, s, m, x, y, size);
        let full_w = width(game, &m.name);
        let name = if full_w > cw - size - 3.0 { trim(game, &m.name, cw - size - 3.0 - 6.0 + width(game, "...")) } else { m.name.clone() };
        text(game, batch, &name, x + size + 3.0, y + 1.0, WHITE, true);
        let mut shift = 0.0;
        if s.config.update_checker && m.update.is_some() {
            update_badge(game, batch, x + size + 3.0 + full_w + 2.0, y);
            shift = 11.0;
        }
        if !s.config.hide_badges {
            badges(game, batch, ru, &m.badges, x + size + 3.0 + full_w + 2.0 + shift, y, x + cw);
        }
        if compact {
            wrapped(game, batch, &mm::version_text(&m.version), x + size + 3.0, y + 11.0, cw - size - 7.0, 2, SUMMARY);
        } else {
            wrapped(game, batch, &m.summary, x + size + 7.0, y + 11.0, cw - size - 7.0, 2, SUMMARY);
        }
        // Quick configure over the hovered row's icon.
        if hovered == Some(i) && s.config.quick_configure && m.configurable() {
            rect(game, batch, [x, y, size, size], 0xa090_9090);
            let over = cursor.0 - x < size;
            blit(game, batch, if over { &sprites::QUICK_CONFIGURE_HOVER } else { &sprites::QUICK_CONFIGURE }, x, y, size);
        }
        if let Some(hr) = intersect([6.0, ey, row_w, item_h], list) {
            s.hits.push((hr, Hit::Row(i)));
        }
    }
    batch.clip_since(start, list[0], list[1], list[0] + list[2], list[1] + list[3]);
    if max > 0.0 {
        ui::scrollbar_sprites(batch, atlas, pane_w - 6.0, pane_y, list[1] + list[3], content, s.list_scroll);
        s.hits.push(([pane_w - 6.0, pane_y, 6.0, list[3]], Hit::Bar(0)));
    }

    // The search box (no placeholder: Mod Menu sets none).
    let field = [search_x, 22.0, search_w, 20.0];
    s.search.x = field[0];
    s.search.y = field[1];
    s.search.width = field[2];
    s.search.height = field[3];
    s.search.draw(batch, atlas, None);
    s.hits.push((field, Hit::Search));

    // The title and the mod count.
    let title = mm::tr(ru, "title");
    text(game, batch, title, (pane_w / 2.0 - width(game, title) / 2.0).floor(), 8.0, WHITE, true);
    let (full, mods_only, libs_only) = count_texts(s);
    let a = layout.sort_w + layout.lib_w + 2.0 + width(game, &full) + 20.0 >= search_row_w;
    let b = layout.sort_w + layout.lib_w + 2.0 + width(game, &mods_only) + 20.0 >= search_row_w;
    let c = layout.sort_w + layout.lib_w + 2.0 + width(game, &libs_only) + 20.0 >= search_row_w;
    let show_count = if a && (b || c) { !s.filters_shown } else { true };
    if show_count {
        let limit = if s.filters_shown { layout.filters_x - 5.0 } else { list[2] - 5.0 };
        if width(game, &full) <= limit {
            text(game, batch, &full, search_x, 52.0, WHITE, true);
        } else {
            text(game, batch, &mods_only, search_x, 46.0, WHITE, true);
            text(game, batch, &libs_only, search_x, 57.0, WHITE, true);
        }
    }

    // The selected mod's header.
    if let Some(m) = &selected {
        let x = right_x;
        icon(game, batch, s, m, x, 48.0, 32.0);
        let name = trim(game, &m.name, w - x - 36.0);
        text(game, batch, &name, x + 36.0, 49.0, WHITE, true);
        let nw = width(game, &name);
        if cursor.0 > x + 36.0 && cursor.0 < x + 36.0 + nw && cursor.1 > 49.0 && cursor.1 < 58.0 {
            tip = Some(vec![mm::tr(ru, "mod_id").replace("%s", &m.id)]);
        }
        if !s.config.hide_badges {
            badges(game, batch, ru, &m.badges, x + 36.0 + nw + 2.0, 48.0, w - 28.0);
        }
        if !m.version.is_empty() {
            text(game, batch, &mm::version_text(&m.version), x + 36.0, 60.0, GRAY_TEXT, true);
        }
        if !m.authors.is_empty() {
            let by = mm::tr(ru, "by").replace("%s", &m.authors.join(", "));
            wrapped(game, batch, &by, x + 36.0, 70.0, pane_w - 40.0, 1, GRAY_TEXT);
        }
    }
    if let Some(lines) = tip {
        tooltip(game, batch, &lines);
    }
}

/// A line of the description list.
struct Line {
    text: Text,
    indent: f32,
    color: u32,
    link: Option<Link>,
    /// The update badge before it.
    badge: bool,
}

/// `DescriptionListWidget.rebuildUI`: the description, the update, the
/// links, the licence and the credits, wrapped to `ww`.
fn description(game: &Game<'_>, s: &State, m: &Mod, ww: f32) -> Vec<Line> {
    let ru = s.ru;
    let c = &s.config;
    let mut out = Vec::new();
    let blank = |out: &mut Vec<Line>| out.push(Line { text: Text::empty(), indent: 0.0, color: GRAY_TEXT, link: None, badge: false });
    let add = |out: &mut Vec<Line>, s: &str, first: f32, rest: f32, w: f32, link: Option<Link>, badge: bool| {
        for (i, t) in wrap(game, s, w).into_iter().enumerate() {
            out.push(Line { text: t, indent: if i == 0 { first } else { rest }, color: GRAY_TEXT, link: link.clone(), badge: badge && i == 0 });
        }
    };
    if !m.description.is_empty() {
        add(&mut out, &m.description, 0.0, 0.0, ww, None, false);
    }
    if c.update_checker
        && let Some(update) = &m.update
    {
        blank(&mut out);
        add(&mut out, mm::tr(ru, "has_update"), 0.0, 0.0, ww - 11.0, None, true);
        add(&mut out, &format!("§6{}", mm::tr(ru, "experimental")), 8.0, 8.0, ww - 16.0, None, false);
        let message = mm::tr(ru, "update_text").replacen("%s", update.version.trim_start_matches('v'), 1).replacen("%s", mm::tr(ru, "catalog"), 1);
        add(&mut out, &format!("§9§n{message}"), 8.0, 8.0, ww - 16.0, Some(Link::Url(update.url.clone())), false);
    }
    if !c.hide_mod_links
        && let Some(source) = &m.source
    {
        blank(&mut out);
        add(&mut out, mm::tr(ru, "links"), 0.0, 0.0, ww, None, false);
        add(&mut out, &format!("§9§n{}", mm::tr(ru, "source")), 8.0, 16.0, ww - 8.0, Some(Link::Url(source.clone())), false);
    }
    if !c.hide_mod_license && !m.licenses.is_empty() {
        blank(&mut out);
        add(&mut out, mm::tr(ru, "license"), 0.0, 0.0, ww, None, false);
        for l in &m.licenses {
            add(&mut out, l, 8.0, 16.0, ww - 8.0, None, false);
        }
    }
    if !c.hide_mod_credits {
        if m.id == "minecraft" {
            blank(&mut out);
            add(&mut out, &format!("§9§n{}", mm::tr(ru, "view_credits")), 0.0, 0.0, ww, Some(Link::Credits), false);
        } else if !m.authors.is_empty() || !m.contributors.is_empty() || !m.maintainers.is_empty() {
            // `Mod.getCredits`: by role, the roles in name order (Author,
            // Contributor, Maintainer).
            blank(&mut out);
            add(&mut out, mm::tr(ru, "credits"), 0.0, 0.0, ww, None, false);
            for (role, names) in [("role_authors", &m.authors), ("role_contributors", &m.contributors), ("role_maintainers", &m.maintainers)] {
                if names.is_empty() {
                    continue;
                }
                add(&mut out, mm::tr(ru, role), 8.0, 16.0, ww - 8.0, None, false);
                for a in names {
                    add(&mut out, a, 16.0, 24.0, ww - 16.0, None, false);
                }
            }
        }
    }
    out
}

// ---- Mod Menu's options (`ModMenuOptionsScreen`) -----------------------------------------

fn options_page(game: &Game<'_>, batch: &mut HudBatch, s: &mut State) {
    let ru = s.ru;
    let atlas = &game.gui.atlas;
    let (w, h) = (batch.width.floor(), batch.height.floor());
    let in_world = s.open != Some(Origin::Title);
    if !in_world {
        game.native_apply_screen_blur();
    }
    ui::menu_background(batch, atlas, 0.0, 0.0, w, h, in_world);
    let title = mm::tr(ru, "options_title");
    text(game, batch, title, (w / 2.0 - width(game, title) / 2.0).floor(), 12.0, WHITE, true);
    ui::list_background(batch, atlas, 33.0, h - 33.0, in_world);
    ui::separators(batch, atlas, 33.0, h - 33.0, in_world);
    let rows = mm::OPTIONS.len().div_ceil(2);
    let content = 25.0 * rows as f32 + 4.0;
    let area = [0.0, 33.0, w, h - 66.0];
    let max = (content - area[3]).max(0.0);
    s.options_scroll = s.options_scroll.clamp(0.0, max);
    s.bars[2] = Some((area, content));
    let start = batch.vertices.len();
    let cx = (w / 2.0).floor();
    for (i, o) in mm::OPTIONS.iter().enumerate() {
        let x = cx - 155.0 + if i % 2 == 1 { 160.0 } else { 0.0 };
        let y = 37.0 + 25.0 * (i / 2) as f32 - s.options_scroll.floor();
        if y + 20.0 < area[1] || y > area[1] + area[3] {
            continue;
        }
        let r = [x, y, 150.0, 20.0];
        let hover = inside(game.cursor, r) && inside(game.cursor, area);
        ui::button(batch, atlas, x, y, 150.0, 20.0, &Text::literal(option_label(game, ru, o.key)), true, hover);
        if let Some(hr) = intersect(r, area) {
            s.hits.push((hr, Hit::Option(i)));
        }
    }
    batch.clip_since(start, area[0], area[1], area[0] + area[2], area[1] + area[3]);
    if max > 0.0 {
        ui::scrollbar_sprites(batch, atlas, cx + 163.0, area[1], area[1] + area[3], content, s.options_scroll);
        s.hits.push(([cx + 163.0, area[1], 6.0, area[3]], Hit::Bar(2)));
    }
    let done = [cx - 100.0, h - 27.0, 200.0, 20.0];
    button(game, batch, done, &Text::translate("gui.done", &[]), true);
    s.hits.push((done, Hit::OptionsDone));
}

// ---- links (`ConfirmLinkScreen`) ---------------------------------------------------------

fn link_page(game: &Game<'_>, batch: &mut HudBatch, s: &mut State, url: &str) {
    let atlas = &game.gui.atlas;
    let (w, h) = (batch.width, batch.height);
    let in_world = s.open != Some(Origin::Title);
    if !in_world {
        game.native_apply_screen_blur();
    }
    ui::menu_background(batch, atlas, 0.0, 0.0, w, h, in_world);
    let warning = Text::translate("chat.link.warning", &[]).with_color(0xFF_CCCC);
    let message = Text::literal(url).append(Text::literal("\n")).append(warning);
    let choice = horizon_client::confirm_screen::Choice::new(&atlas.font, (w, h), &Text::translate("chat.link.confirm", &[]), &message, 3, 100.0);
    let labels = [Text::translate("gui.yes", &[]), Text::translate("chat.copy", &[]), Text::translate("gui.no", &[])];
    choice.draw(batch, atlas, game.cursor, &labels);
    for (i, &(x, y)) in choice.buttons.iter().enumerate() {
        s.hits.push(([x, y, choice.width, 20.0], Hit::LinkButton(i)));
    }
}
