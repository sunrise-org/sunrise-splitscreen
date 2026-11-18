fn local_only() -> std::ops::ControlFlow<()> {
    if crate::game::loading_screen::splitscreen_view::GuestTurn::active() {
        std::ops::ControlFlow::Break(())
    } else {
        std::ops::ControlFlow::Continue(())
    }
}
