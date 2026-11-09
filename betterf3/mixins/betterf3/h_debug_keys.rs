// F3+M and F3+F are BetterF3's (`KeyboardMixin.processF3`).
fn betterf3_debug_keys(key: &str) -> core::ops::ControlFlow<bool> {
    if crate::debug_keys::betterf3_keys::handle(key) {
        return core::ops::ControlFlow::Break(true);
    }
    core::ops::ControlFlow::Continue(())
}
