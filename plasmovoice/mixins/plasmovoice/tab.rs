// A Horizon addition, not in Plasmo Voice: its icons in the player list (Tab), as servers add them
// with PlaceholderAPI's `%plasmovoice_...%` and the TAB plugin. The list is the game's own
// (`draw_tab_list`, laid out by `tab_list::layout`). A player with voice chat gets an 8 px
// microphone right before the name, as Horizon's mark is and one pixel from the name like it:
// white while they talk (the HUD's activation icon), crossed out in red otherwise. After the mark
// when they have it ([face][H][mic]Name, the name moved over), else in the mark's place
// ([face][mic]Name; [mic]Name when the list shows no faces); players without voice chat keep the
// game's row. Switched by "Tab List Icons" on the Overlay tab (`horizon.tab_list_icons`).

use super::Game;
use super::plasmovoice_icons as icons;
use horizon_client::actions::Action;
use horizon_client::hud::HudBatch;
use horizon_client::hud_layout::GuiDraw;
use horizon_client::scoreboard as sb;
use horizon_client::tab_list::{self, TabInputs, TabPlayer};

/// What an icon takes before a name: 8 px and a pixel, as Horizon's mark.
const COLUMN: i32 = 9;

/// The player list with the icons, in place of the game's (`draw_tab_list`, cancelled when this
/// drew it). False leaves it to the game: the icons off, or no voice connection.
pub(super) fn draw(game: &mut Game<'_>, batch: &mut HudBatch) -> bool {
    if game.window.is_none() || !game.native_on(super::plasmovoice_glue::ID) {
        return false;
    }
    let me = game.own_uuid();
    let Some(icons) = super::plasmovoice_glue::config(|c| super::plasmovoice_hud::tab_icons(c, me)) else { return false };

    // `Game::draw_tab_list`, as the game has it.
    game.flush_skin_slots();
    let key = game.screen == super::Screen::Playing && game.keys.held(Action::Playerlist);
    let count = game.remote.as_ref().map_or_else(
        || 1 + game.lan.as_ref().map_or(0, |h| h.hub.guests.iter().filter(|g| g.placed).count()),
        |r| r.listed_player_count(),
    );
    let has_objective = game.score.board.display_objective(sb::SLOT_LIST).is_some();
    let shown = tab_list::visible(!game.ui.hide_gui, key, game.has_singleplayer_server(), count, has_objective);
    let changed = game.tab_list.set_visible(shown);
    if !(shown || changed) {
        game.update_remote_skins();
        game.publish_tab_object_skins();
        return true;
    }
    let (mut players, online_mode, upside_down) = match game.remote_tab_players() {
        Some(remote) => remote,
        None => {
            let uuid = game.level.player_uuid.map_or(0, |u| u.iter().fold(0u128, |a, &w| (a << 32) | u128::from(w as u32)));
            let mut me = TabPlayer::new(uuid, super::chat::PLAYER_NAME);
            me.game_mode = game.mode_id();
            me.skin = "entity/player/wide/steve".into();
            let guests = game.lan.as_ref().map_or(&[][..], |h| &h.hub.guests);
            let players = super::scoreboard::local_tab_players(me, guests);
            let upside_down = players.iter().filter(|p| p.name == "Dinnerbone" || p.name == "Grumm").map(|p| p.uuid).collect();
            (players, true, upside_down)
        }
    };
    if changed {
        game.say_system_now(&tab_list::narration(&players, &game.score.board));
    }
    if !shown {
        return true;
    }
    let ids: Vec<u128> = game.tab_list.displayed_ids(&players, &game.score.board).to_vec();
    for p in players.iter_mut().filter(|p| ids.contains(&p.uuid)) {
        p.skin = game.face_skin(p.uuid, &p.skin);
    }
    let board = &game.score.board;
    let objective = board.display_objective(sb::SLOT_LIST);
    let up = |id: u128| upside_down.contains(&id);
    let horizon = game.horizon_players(players.iter().map(|p| p.uuid));
    let marked = |id: u128| horizon.contains(&id);
    let background = tab_list::row_background(true, f64::from(game.settings.text_background_opacity));
    let inputs = TabInputs {
        players: &players,
        board,
        objective,
        screen_width: batch.width as i32,
        online_mode,
        gui_ticks: game.visual_ticks() as i64,
        background,
        upside_down: &up,
        horizon: &marked,
    };
    let draws = game.tab_list.layout(&inputs, &game.gui.atlas.font);

    // The rows' icons, by the order the list shows them in.
    let row_icons: Vec<Option<&'static str>> = ids.iter().map(|id| icons.get(id).copied()).collect();
    if row_icons.iter().all(Option::is_none) {
        horizon_client::hud_layout::paint(batch, &game.gui.atlas, &draws);
        return true;
    }
    let row_marked: Vec<bool> = ids.iter().map(|id| horizon.contains(id)).collect();
    let (draws, places) = place(&draws, background, batch.width as i32, &game.gui.atlas.font, &row_icons, &row_marked);
    horizon_client::hud_layout::paint(batch, &game.gui.atlas, &draws);
    for ((x, y), icon) in places.into_iter().zip(row_icons) {
        if let Some(icon) = icon {
            icons::draw(batch, &game.gui.atlas, icon, x as f32, y as f32, 8.0, 8.0, 0xFFFF_FFFF);
        }
    }
    true
}

/// The laid out list with room for the icons: a row with an icon after a mark, or in a list
/// without marks, has its name moved `COLUMN` right; one without a mark in a list with marks puts
/// the icon in the mark's empty place. The columns grow by what the longest moved name needs (the
/// scores, hearts and ping bars moved with them), and the list is centred again. Rows without an
/// icon stay as the game lays them out. Returns the draws and, row by row, where an icon goes.
fn place(
    draws: &[GuiDraw],
    background: u32,
    sw: i32,
    font: &horizon_client::font::Font,
    icons: &[Option<&'static str>],
    marked: &[bool],
) -> (Vec<GuiDraw>, Vec<(i32, i32)>) {
    let is_row = |d: &GuiDraw| matches!(d, GuiDraw::Fill { y0, y1, color, .. } if *color == background && y1 - y0 == 8);
    // The rows (their backgrounds: 8 px fills in the row colour) and each row's name, the first
    // text after its background: where it is in `draws`, where it starts, how wide it is.
    let mut rows: Vec<(i32, i32, i32)> = Vec::new();
    let mut names: Vec<(usize, i32, i32)> = Vec::new();
    let mut want_name = false;
    for (i, d) in draws.iter().enumerate() {
        match d {
            GuiDraw::Fill { x0, y0, x1, .. } if is_row(d) => {
                rows.push((*x0, *y0, x1 - x0));
                want_name = true;
            }
            GuiDraw::Text { text, x, .. } if want_name => {
                names.push((i, *x, font.width_text(text) as i32));
                want_name = false;
            }
            _ => {}
        }
    }
    if rows.is_empty() || names.len() != rows.len() {
        return (draws.to_vec(), Vec::new());
    }
    let (first_y, slot) = (rows[0].1, rows[0].2);
    let mut starts: Vec<i32> = rows.iter().map(|r| r.0).collect();
    starts.sort_unstable();
    starts.dedup();
    let cols = starts.len() as i32;
    let last_y = rows.iter().map(|r| r.1).max().unwrap_or(first_y) + 8;
    // The names start `lead` into their column: after the face (when the list shows faces) and
    // the marks' column (when anyone listed has the mark).
    let lead = names[0].1 - rows[0].0;
    let mark_column = marked.iter().any(|m| *m);
    let shift: Vec<i32> = (0..rows.len())
        .map(|r| {
            let icon = icons.get(r).copied().flatten().is_some();
            if icon && (marked.get(r).copied().unwrap_or(false) || !mark_column) { COLUMN } else { 0 }
        })
        .collect();
    let max_name = names.iter().map(|n| n.2).max().unwrap_or(0);
    let extra = (names.iter().zip(&shift).map(|(n, s)| n.2 + s).max().unwrap_or(0) - max_name).max(0);
    let scores = lead + max_name + 1;
    let old_total = slot * cols + (cols - 1) * 5;
    let new_slot = slot + extra;
    let new_total = new_slot * cols + (cols - 1) * 5;
    let old_x = sw / 2 - old_total / 2;
    let new_x = sw / 2 - new_total / 2;
    let col_of = |x: i32| starts.iter().rposition(|s| x >= *s).unwrap_or(0) as i32;
    let moved = |x: i32, name_shift: Option<i32>| {
        let c = col_of(x);
        let from = old_x + c * (slot + 5);
        let to = new_x + c * (new_slot + 5);
        let rel = x - from;
        let add = match name_shift {
            Some(s) => s,
            None if rel >= scores => extra,
            None => 0,
        };
        to + rel + add
    };
    let in_rows = |y: i32| y >= first_y && y < last_y;
    let mut out = Vec::with_capacity(draws.len());
    for (i, d) in draws.iter().enumerate() {
        let name_shift = names.iter().position(|n| n.0 == i).map(|r| shift[r]);
        let d = match d.clone() {
            GuiDraw::Fill { x0, y0, y1, color, .. } if color == background && y1 - y0 == 8 => {
                let x0n = moved(x0, None);
                GuiDraw::Fill { x0: x0n, y0, x1: x0n + new_slot, y1, color }
            }
            // The band behind the rows, header and footer: as wide as the list when it grew.
            GuiDraw::Fill { x0, y0, x1, y1, color } if color == 0x8000_0000 => {
                let inner = x1 - x0 - 2;
                let half = if extra > 0 && inner < new_total { new_total / 2 } else { inner / 2 };
                GuiDraw::Fill { x0: sw / 2 - half - 1, y0, x1: sw / 2 + half + 1, y1, color }
            }
            GuiDraw::Fill { x0, y0, x1, y1, color } if in_rows(y0) => {
                let dx = moved(x0, None) - x0;
                GuiDraw::Fill { x0: x0 + dx, y0, x1: x1 + dx, y1, color }
            }
            GuiDraw::Text { text, x, y, color, shadow } if in_rows(y) => GuiDraw::Text { text, x: moved(x, name_shift), y, color, shadow },
            GuiDraw::Sprite { sprite, x, y, w, h, region, color, blend } if in_rows(y) => {
                GuiDraw::Sprite { sprite, x: moved(x, None), y, w, h, region, color, blend }
            }
            GuiDraw::Blit { texture, blend, x, y, w, h, uv, src, tex, color } if in_rows(y) => {
                GuiDraw::Blit { texture, blend, x: moved(x, None), y, w, h, uv, src, tex, color }
            }
            other => other,
        };
        out.push(d);
    }
    let places = rows
        .iter()
        .zip(&shift)
        .map(|(&(x0, y0, _), s)| {
            let c = col_of(x0);
            let at = new_x + c * (new_slot + 5) + lead;
            // A moved name leaves the icon where the name was; else it takes the mark's place.
            (if *s > 0 { at } else { at - COLUMN }, y0)
        })
        .collect();
    (out, places)
}
