//! Isolated development harness for a second Game and a shared compositor. Not packaged.
use super::{App, Game, Resources, split::Split, splitscreen_view::View};
use crate::game::integrated::splitscreen_admission::WorldSeats;
use std::{cell::OnceCell, sync::Arc};
use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, DeviceId, WindowEvent},
    event_loop::ActiveEventLoop,
    window::WindowId,
};

const GUEST: &str = "ViewFixture";

pub(crate) trait PrimaryApp: ApplicationHandler {
    fn with_primary(&mut self, step: impl FnOnce(&mut Game<'_>));
}

impl PrimaryApp for Split<'_> {
    fn with_primary(&mut self, step: impl FnOnce(&mut Game<'_>)) {
        step(&mut self.host);
    }
}

impl PrimaryApp for App {
    fn with_primary(&mut self, step: impl FnOnce(&mut Game<'_>)) {
        if let Self::Ready(game) = self {
            game.with_primary(step);
        }
    }
}

#[derive(Default)]
enum Phase<'res> {
    #[default]
    Waiting,
    Joining,
    Playing(Box<View<'res>>),
    Disconnected,
    Closing,
}

pub(crate) struct GuestFixture<'app, 'res, A> {
    app: &'app mut A,
    seats: WorldSeats,
    resources: &'res OnceCell<Arc<Resources>>,
    phase: Phase<'res>,
}

impl<'app, 'res, A> GuestFixture<'app, 'res, A> {
    pub(crate) fn new(app: &'app mut A, resources: &'res OnceCell<Arc<Resources>>) -> Self {
        Self {
            app,
            seats: WorldSeats::default(),
            resources,
            phase: Phase::Waiting,
        }
    }
}

impl<A: PrimaryApp> ApplicationHandler for GuestFixture<'_, '_, A> {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        self.app.resumed(el);
    }

    fn window_event(&mut self, el: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if !matches!(self.phase, Phase::Playing(_))
            || !matches!(
                event,
                WindowEvent::RedrawRequested | WindowEvent::Resized(_)
            )
        {
            self.app.window_event(el, id, event);
        }
    }

    fn device_event(&mut self, el: &ActiveEventLoop, id: DeviceId, event: DeviceEvent) {
        self.app.device_event(el, id, event);
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        if !matches!(self.phase, Phase::Playing(_)) {
            self.app.about_to_wait(el);
        }
        self.app.with_primary(|host| {
            let phase = std::mem::take(&mut self.phase);
            let same_world = self.seats.track(host);
            self.phase = match phase {
                Phase::Waiting if host.player.is_some() && host.integrated.is_some() => {
                    assert!(self.seats.request(host, GUEST.into()));
                    Phase::Joining
                }
                Phase::Joining => match self.seats.next_ready() {
                    Some(Some(pipe)) => {
                        let view =
                            View::start(host, self.resources, pipe).expect("start guest Game");
                        Phase::Playing(Box::new(view))
                    }
                    Some(None) => panic!("guest view reservation refused"),
                    None => Phase::Joining,
                },
                Phase::Playing(mut view) => {
                    assert!(
                        same_world && !el.exiting(),
                        "the primary world/loop ended early"
                    );
                    let drawn = view.frame(host, el).expect("draw both client views");
                    if drawn {
                        assert_eq!(
                            roster(host),
                            (true, true),
                            "both clients need distinct roster entries"
                        );
                        view.check_guest_isolation();
                        view.restore(host, el).expect("restore primary surface");
                        assert!(!el.exiting(), "guest quit ended the primary loop");
                        Phase::Disconnected
                    } else {
                        Phase::Playing(view)
                    }
                }
                Phase::Disconnected => match roster(host) {
                    (true, false) => {
                        eprintln!("guest fixture: PASS (guest quit, P1 still playing)");
                        host.begin_closing_save(true).expect("save primary world");
                        Phase::Closing
                    }
                    (true, true) => Phase::Disconnected,
                    _ => panic!("the primary disappeared during guest teardown"),
                },
                phase => phase,
            };
        });
    }

    fn exiting(&mut self, el: &ActiveEventLoop) {
        self.phase = Phase::Closing;
        self.app.exiting(el);
    }
}

fn roster(host: &Game<'_>) -> (bool, bool) {
    host.integrated
        .as_ref()
        .expect("P1's integrated world closed")
        .call(|game| {
            let lan = game.lan.as_ref()?;
            let owner = lan.owner.as_ref()?;
            let guest = sunrise_protocol::crypto::offline_uuid(GUEST);
            assert_ne!(owner.uuid, guest);
            assert!(!lan.hub.ops.contains_key(&guest));
            Some((
                lan.hub.guests.iter().any(|p| p.uuid == owner.uuid),
                lan.hub
                    .guests
                    .iter()
                    .any(|p| p.uuid == guest && p.name == GUEST),
            ))
        })
        .flatten()
        .expect("read the playing roster")
}
