fn presence(
    status: sunrise_client::social::PresenceStatus,
    set: impl FnOnce(sunrise_client::social::PresenceStatus) -> (),
) {
    if !crate::game::loading_screen::splitscreen_view::GuestTurn::active() {
        set(status);
    }
}
