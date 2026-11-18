fn no_auth() -> std::ops::ControlFlow<Option<std::sync::Arc<sunrise_client::auth::AuthSession>>> {
    if crate::game::loading_screen::splitscreen_view::GuestTurn::active() {
        std::ops::ControlFlow::Break(None)
    } else {
        std::ops::ControlFlow::Continue(())
    }
}
