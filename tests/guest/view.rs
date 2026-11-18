//! Development fixture: each Game owns its camera and memory session; one compositor owns the surface.
use super::{Audio, Game, Options, Renderer, Resources, Result, UiResources, build_renderer};
use crate::game::{
    event_loop::EventLoopCtl, input_trace::InputOrigin, integrated::splitscreen_admission::SeatPipe,
};
use std::{
    cell::{Cell, OnceCell},
    sync::Arc,
};
use sunrise_client::{
    gfx::compositor::Compositor,
    window::{FramerateLimiter, SystemClock, end_frame},
};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow},
    window::Window,
};

pub(crate) struct View<'res> {
    guest: Box<Game<'res>>,
    _lease: Arc<mod_splitscreen::seats::Ticket>,
    compositor: Compositor,
    size: PhysicalSize<u32>,
    limiter: FramerateLimiter,
    clock: SystemClock,
}

impl<'res> View<'res> {
    #[inline(never)] // Keep the Game constructor's large stack frame out of the app callback.
    pub(crate) fn start(
        host: &mut Game<'_>,
        resources: &'res OnceCell<Arc<Resources>>,
        SeatPipe { pipe, lease }: SeatPipe,
    ) -> Result<Self> {
        if host.options.offscreen || !host.options.developer_single_player || host.auth().is_some()
        {
            return Err("the fixture requires an offline developer window".into());
        }
        let window = host
            .window
            .clone()
            .ok_or("the primary window is not open")?;
        let primary = host
            .renderer
            .as_ref()
            .ok_or("the primary renderer is not ready")?;
        let mut settings = host.settings.clone();
        settings.controller.enabled = false;
        settings.extras.agent_control = false;
        settings.sunrise.lod_distance = 0;
        let options = Options {
            player_name: Some(lease.name.clone()),
            offscreen: true,
            paced_by_split: true,
            // Exercise the mixins rather than borrowing the stock guest's guards.
            split_guest: false,
            split_in_main_window: false,
            world: None,
            quick_play: None,
            server: None,
            session_stdin: false,
            session_file: None,
            offline_reason: None,
            control: None,
            game_dir: None,
            frames: None,
            world_frames: None,
            screenshot_after: None,
            timeout: None,
            report: None,
            benchmark: None,
            benchmark_set: None,
            benchmark_profile: false,
            look: None,
            time: None,
            weather: None,
            dimension: None,
            ..host.options.clone()
        };
        let gui = UiResources::load(&host.pack.source)?;
        let shared = resources.get_or_init(|| Arc::clone(&host.res));
        let guest = GuestTurn::run(|| {
            let audio = Audio::silent(&host.pack.source);
            let mut guest = Box::new(Game::new(
                shared,
                host.pack.clone(),
                gui,
                options,
                settings,
                audio,
                None,
            )?);
            guest.ui.vanilla_root = host.vanilla_root();
            guest.refresh_debug_renderers();
            guest.refresh_loop_context()?;
            guest.preload_registries();
            guest.window = Some(window.clone());
            guest.renderer = Some(renderer(&guest, window.clone(), primary, true)?);
            guest.world_renderer_ready = true;
            guest.keyboard_mouse = false;
            assert!(guest.auth().is_none() && guest.title.is_none());
            guest.connect_local(pipe, &lease.name)?;
            Ok::<_, Box<dyn std::error::Error>>(guest)
        })?;
        let offscreen = renderer(host, window.clone(), primary, true)?;
        let primary = host.renderer.as_mut().unwrap();
        primary.release_surface();
        let compositor = match primary.compositor(window) {
            Ok(compositor) => compositor,
            Err(error) => {
                primary.recreate_surface()?;
                return Err(error);
            }
        };
        host.options.offscreen = true;
        host.options.paced_by_split = true;
        host.split_window = true;
        install_primary(host, offscreen);
        Ok(Self {
            guest,
            _lease: lease,
            compositor,
            size: PhysicalSize::default(),
            limiter: FramerateLimiter::new(0),
            clock: SystemClock::default(),
        })
    }

    pub(crate) fn frame(&mut self, host: &mut Game<'_>, el: &ActiveEventLoop) -> Result<bool> {
        let window = host.window.clone().ok_or("the primary window closed")?;
        let size = window.inner_size();
        if size.width < 2 || size.height == 0 {
            return Ok(false);
        }
        let mid = size.width / 2;
        let guest = &mut self.guest;
        let guest_loop = GuestLoop::default();
        if size != self.size {
            for renderer in host.renderer.iter_mut().chain(guest.renderer.iter_mut()) {
                renderer.wait_submissions();
            }
            self.compositor.resize(size.width, size.height);
            host.window_event(
                el,
                window.id(),
                WindowEvent::Resized(PhysicalSize::new(mid, size.height)),
            );
            GuestTurn::run(|| {
                guest.view_at = [mid, 0];
                guest.handle_window_event(
                    &guest_loop,
                    window.id(),
                    WindowEvent::Resized(PhysicalSize::new(size.width - mid, size.height)),
                    InputOrigin::Game,
                );
            });
            self.size = size;
        }
        host.about_to_wait(el);
        if el.exiting() || host.integrated.is_none() {
            return Err("primary exited during guest frame".into());
        }
        GuestTurn::run(|| guest.idle(&guest_loop));
        if let Some(error) = guest.error.take() {
            return Err(error);
        }
        if guest_loop.0.get() || guest.remote.is_none() || guest.title.is_some() {
            return Err("guest left the memory session before drawing".into());
        }
        let limit = host.framerate_limit();
        let primary = host.renderer.as_mut().ok_or("lost primary renderer")?;
        let other = guest.renderer.as_mut().ok_or("lost guest renderer")?;
        if primary.device_lost() || other.device_lost() {
            return Err("graphics device was lost".into());
        }
        primary.wait_submissions();
        other.wait_submissions();
        self.compositor.set_vsync(host.settings.vsync);
        let presented = match (primary.offscreen_frame(), other.offscreen_frame()) {
            (Some(p1), Some(p2)) => {
                assert_eq!((p1.width(), p1.height()), (mid, size.height));
                assert_eq!((p2.width(), p2.height()), (size.width - mid, size.height));
                self.compositor.present(&[(p1, [0, 0]), (p2, [mid, 0])])
            }
            _ => false,
        };
        primary.wait_in_flight();
        other.wait_in_flight();
        let refresh = window
            .current_monitor()
            .and_then(|m| m.refresh_rate_millihertz())
            .map_or(60, |hz| hz.div_ceil(1000) as i32);
        if !(host.settings.vsync && limit >= refresh) {
            end_frame(&mut self.limiter, limit, &mut self.clock);
        }
        Ok(presented && host.player.is_some() && guest.player.is_some())
    }

    pub(crate) fn check_guest_isolation(&mut self) {
        let guest = &mut self.guest;
        GuestTurn::run(|| {
            let window = guest.window.as_ref().unwrap().clone();
            let before = window.fullscreen();
            let saved_fullscreen = guest.settings.fullscreen;
            let key = winit::keyboard::KeyCode::F11;
            let keyboard = sunrise_client::held::Source::Keyboard;
            guest.mapped_key(key, true, false, key, keyboard).unwrap();
            guest.mapped_key(key, false, false, key, keyboard).unwrap();
            assert_eq!(
                window.fullscreen(),
                before,
                "guest F11 changed shared fullscreen"
            );
            assert_eq!(
                guest.settings.fullscreen, saved_fullscreen,
                "guest F11 changed stored fullscreen"
            );
            assert!(guest.auth().is_none() && guest.title.is_none());
            guest.show_title(guest.new_title("unused".into()));
            assert!(guest.title.is_none(), "guest installed a stock title");
            for (lan, realm) in [(false, false), (true, false), (false, true)] {
                assert!(
                    guest
                        .connect_remote_session("127.0.0.1:1", "Fixture", None, None, lan, realm)
                        .is_err()
                );
            }
            assert!(guest.open_world("unused", None, None, false, None).is_err());
            assert!(
                guest
                    .lan_open(crate::game::lan::LanSettings {
                        port: 0,
                        guest_commands: false,
                        force_game_mode: false
                    })
                    .is_err()
            );
            guest.begin_closing_save(true).expect("execute guest quit");
            assert!(
                guest.remote.is_none() && guest.pending_save.is_none() && guest.title.is_none()
            );
        });
        eprintln!(
            "guest fixture: both views presented; no guest auth, title, TCP or shared fullscreen"
        );
    }

    pub(crate) fn restore(self, host: &mut Game<'_>, el: &ActiveEventLoop) -> Result<()> {
        drop(self.guest);
        let window = host.window.clone().ok_or("the primary window closed")?;
        let previous = host
            .renderer
            .as_ref()
            .ok_or("the primary renderer was lost")?;
        previous.take_back(self.compositor);
        let ordinary = renderer(host, window.clone(), previous, false)?;
        host.options.offscreen = false;
        host.options.paced_by_split = false;
        host.split_window = false;
        install_primary(host, ordinary);
        host.window_event(el, window.id(), WindowEvent::Resized(window.inner_size()));
        Ok(())
    }
}

// Not the engine's seat scope. This only suppresses process work in the fixture's guest callbacks.
thread_local! {
    static GUEST_TURN: Cell<bool> = const { Cell::new(false) };
}

pub(crate) struct GuestTurn;
impl GuestTurn {
    fn run<T>(f: impl FnOnce() -> T) -> T {
        assert!(!GUEST_TURN.replace(true));
        let _turn = Self;
        f()
    }

    pub(crate) fn active() -> bool {
        GUEST_TURN.get()
    }
}
impl Drop for GuestTurn {
    fn drop(&mut self) {
        GUEST_TURN.set(false);
    }
}

#[derive(Default)]
struct GuestLoop(Cell<bool>);
impl EventLoopCtl for GuestLoop {
    fn exit(&self) {
        self.0.set(true);
    }
    fn set_control_flow(&self, _: ControlFlow) {}
}

fn renderer(
    game: &Game<'_>,
    window: Arc<Window>,
    previous: &Renderer,
    offscreen: bool,
) -> Result<Renderer> {
    let settings = &game.settings;
    build_renderer(
        window,
        offscreen,
        &game.pack,
        &game.gui,
        Some(previous),
        None,
        settings.sunrise.api,
        settings.sunrise.translucent_batching,
        settings.vsync,
        &settings.sunrise,
        crate::game::shader_packs::picked_pack_shape(settings),
        previous.display_reading(),
        (
            settings.file.int("textureFiltering"),
            settings.file.int("maxAnisotropyBit"),
        ),
    )
}

fn install_primary(host: &mut Game<'_>, mut renderer: Renderer) {
    renderer.paced_elsewhere = host.options.offscreen;
    host.renderer = Some(renderer);
    host.forget_atlas_slots();
    host.apply_settings();
    host.clear_lod();
    host.all_sections_changed();
}
