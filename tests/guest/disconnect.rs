fn disconnect(guest: &mut Game<'_>) -> std::ops::ControlFlow<Result<()>> {
    if crate::game::loading_screen::splitscreen_view::GuestTurn::active() {
        drop(guest.remote.take());
        std::ops::ControlFlow::Break(Ok(()))
    } else {
        std::ops::ControlFlow::Continue(())
    }
}
