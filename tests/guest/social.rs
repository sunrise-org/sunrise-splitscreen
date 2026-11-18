fn social(tick: impl FnOnce() -> ()) {
    if !crate::game::loading_screen::splitscreen_view::GuestTurn::active() {
        tick();
    }
}
