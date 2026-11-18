fn direct(mut game: Game<'_>) -> Result<()> {
    if game.options.headless {
        // No display to open winit's loop on: the game's own loop.
        game.run_headless();
        // narrator::start ran for it too (options.txt's narrator setting).
        narrator::stop();
        let result = write_report(&game).and_then(|()| game.error.take().map_or(Ok(()), Err));
        if game.options.is_client_start() {
            game.native_client_stopped();
            game.native_game_shutting_down();
        }
        return result;
    }
    let mut split = split::Split::new(game);
    let resources = std::cell::OnceCell::new();
    let result = EventLoop::new()?.run_app(
        &mut crate::game::loading_screen::splitscreen_app::GuestFixture::new(
            &mut split, &resources,
        ),
    );
    game = split.host;
    narrator::stop();
    leave_fullscreen_after(&result, &mut game.settings);
    let result = result
        .map_err(Into::into)
        .and_then(|()| write_report(&game))
        .and_then(|()| game.error.take().map_or(Ok(()), Err));
    if game.options.is_client_start() {
        game.native_client_stopped();
        game.native_game_shutting_down();
    }
    result
}
