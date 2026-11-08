// A tooltip about to be drawn: hidden under the recipes screen.
fn justenoughitems_tooltip_hide(cancel: &mut bool) {
    if crate::game::justenoughitems_glue::hide_tooltip() {
        *cancel = true;
    }
}
