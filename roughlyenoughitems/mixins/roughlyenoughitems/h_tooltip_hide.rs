// A tooltip about to be drawn: hidden under the recipes screen.
fn roughlyenoughitems_tooltip_hide(cancel: &mut bool) {
    if crate::game::roughlyenoughitems_glue::hide_tooltip() {
        *cancel = true;
    }
}
