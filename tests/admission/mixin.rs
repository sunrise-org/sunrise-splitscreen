fn drive(game: &mut Game<'_>, event_loop: &dyn EventLoopCtl) {
    if game.options.is_client_start() && crate::game::integrated::splitscreen_fixture::tick(game) {
        event_loop.exit();
    }
}
