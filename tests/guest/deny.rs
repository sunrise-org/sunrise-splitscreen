fn deny() -> std::ops::ControlFlow<Result<()>> {
    if crate::game::loading_screen::splitscreen_view::GuestTurn::active() {
        std::ops::ControlFlow::Break(Err("the local client cannot open or publish a world".into()))
    } else {
        std::ops::ControlFlow::Continue(())
    }
}
