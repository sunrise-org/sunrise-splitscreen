fn configure(
    game: &mut Game<'_>,
    settings: LanSettings,
    mut net: NetConfig,
    hub: HubConfig,
    start: impl FnOnce(&mut Game<'_>, LanSettings, NetConfig, HubConfig) -> Result<()>,
) -> Result<()> {
    use crate::game::integrated::splitscreen_admission::Gate;
    let gate = Gate::configure(&mut net);
    start(game, settings, net, hub)?;
    Gate::install(gate);
    Ok(())
}
