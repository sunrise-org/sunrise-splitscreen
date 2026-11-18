fn join(
    local: &Option<sunrise_protocol::connection::Pipe>,
    lan: bool,
    realm: bool,
) -> std::ops::ControlFlow<Result<()>> {
    if crate::game::loading_screen::splitscreen_view::GuestTurn::active()
        && (local.is_none() || lan || realm)
    {
        std::ops::ControlFlow::Break(Err(
            "the local client requires the host's memory connection".into(),
        ))
    } else {
        std::ops::ControlFlow::Continue(())
    }
}
