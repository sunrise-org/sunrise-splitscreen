fn no_skin() -> std::ops::ControlFlow<Option<String>> {
    if crate::game::loading_screen::splitscreen_view::GuestTurn::active() {
        std::ops::ControlFlow::Break(None)
    } else {
        std::ops::ControlFlow::Continue(())
    }
}
