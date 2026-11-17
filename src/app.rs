use super::{App, split::Split};
use crate::game::integrated::splitscreen_admission::WorldSeats;
use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, DeviceId, StartCause, WindowEvent},
    event_loop::ActiveEventLoop,
    window::WindowId,
};

pub(crate) trait PrimaryApp: ApplicationHandler {
    fn track_world(&self, seats: &mut WorldSeats);
}

impl PrimaryApp for Split<'_> {
    fn track_world(&self, seats: &mut WorldSeats) {
        seats.track(&self.host);
    }
}

impl PrimaryApp for App {
    fn track_world(&self, seats: &mut WorldSeats) {
        match self {
            Self::Ready(game) => game.track_world(seats),
            Self::Loading(_) => seats.clear(),
        }
    }
}

pub(crate) struct WindowApp<'a, A> {
    app: &'a mut A,
    seats: WorldSeats,
}

impl<'a, A> WindowApp<'a, A> {
    pub(crate) fn new(app: &'a mut A) -> Self {
        Self {
            app,
            seats: WorldSeats::default(),
        }
    }
}

impl<A: PrimaryApp> ApplicationHandler for WindowApp<'_, A> {
    fn new_events(&mut self, el: &ActiveEventLoop, cause: StartCause) {
        self.app.new_events(el, cause);
    }

    fn resumed(&mut self, el: &ActiveEventLoop) {
        self.app.resumed(el);
    }

    fn suspended(&mut self, el: &ActiveEventLoop) {
        self.app.suspended(el);
    }

    fn window_event(&mut self, el: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        self.app.window_event(el, id, event);
    }

    fn device_event(&mut self, el: &ActiveEventLoop, id: DeviceId, event: DeviceEvent) {
        self.app.device_event(el, id, event);
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        self.app.about_to_wait(el);
        self.app.track_world(&mut self.seats);
    }

    fn exiting(&mut self, el: &ActiveEventLoop) {
        self.seats.clear();
        self.app.exiting(el);
    }

    fn memory_warning(&mut self, el: &ActiveEventLoop) {
        self.app.memory_warning(el);
    }
}
