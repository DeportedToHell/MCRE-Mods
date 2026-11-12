// A backpack's menu (in a hand or placed, no block's menu): the mod's screen shows it.
fn travelersbackpack_screen_open(
    game: &mut crate::game::Game<'_>,
    key: horizon_client::block_entities::Pos,
    target: Option<horizon_client::block_entities::Pos>,
    window: i32,
    menu: &str,
    extra: &Option<horizon_protocol::menu_extra::Message>,
) -> core::ops::ControlFlow<()> {
    // `target` is the last block clicked, which outlives its use: a
    // backpack's menu is told by its type alone.
    let _ = target;
    if crate::game::travelersbackpack::tier_of_menu(menu).is_none() {
        return core::ops::ControlFlow::Continue(());
    }
    crate::game::travelersbackpack_glue::screen_extra(menu, extra.as_ref().map(|m| &m.data[..]));
    let mut screen = None;
    game.native_screen(menu, &mut screen);
    let Some(screen) = screen else {
        return core::ops::ControlFlow::Continue(());
    };
    game.mod_screen = Some(OpenNativeScreen { key, window, block: menu.to_owned(), screen, data: Vec::new(), extra: extra.clone() });
    core::ops::ControlFlow::Break(())
}
