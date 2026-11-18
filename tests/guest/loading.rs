fn loading(
    options: Options,
    settings: Settings,
    vanilla_root: PathBuf,
    pack_error: Option<String>,
    started: Instant,
) -> Result<()> {
    let progress = Progress::new();
    let (receiver, loader) = startup_menu::spawn_loader(
        &options,
        &settings,
        mods::item_data_load::LoadRole::startup(options.execution_role),
        &vanilla_root,
        started,
        &progress,
    )?;
    let menu = startup_menu::StartupMenu::new(&options, pack_error.as_deref())?;
    let loading = Loading {
        window: None,
        renderer: None,
        boot: sunrise_client::hud::HudAtlas::boot(),
        gui: None,
        settings,
        fullscreen: sunrise_client::window::FullscreenState::new(false),
        video_modes: Default::default(),
        options: Some(options),
        vanilla_root,
        pack_error,
        started,
        panorama_rotation: Default::default(),
        progress,
        receiver,
        loaded: None,
        delivered: false,
        handle: Some(loader),
        shown: false,
        ui_at: None,
        title_drawn: false,
        menu,
        error: None,
    };
    let mut app = App::Loading(Box::new(loading));
    let resources = std::cell::OnceCell::new();
    let result = EventLoop::new()?.run_app(
        &mut crate::game::loading_screen::splitscreen_app::GuestFixture::new(&mut app, &resources),
    );
    narrator::stop();
    match &mut app {
        // Ready: splitscreen around P1's game.
        App::Ready(split) => leave_fullscreen_after(&result, &mut split.host.settings),
        App::Loading(loading) => leave_fullscreen_after(&result, &mut loading.settings),
    }
    result?;
    match app {
        App::Ready(mut split) => {
            write_report(&split.host)?;
            if let Some(e) = split.host.error.take() {
                return Err(e);
            }
        }
        App::Loading(loading) => {
            if let Some(e) = loading.error {
                return Err(e);
            }
        }
    }
    Ok(())
}
