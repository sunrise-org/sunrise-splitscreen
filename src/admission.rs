//! Connects the testable seat policy to Sunrise's integrated-server and memory-transport lifetimes.
use super::{Flags, Game};
use mod_splitscreen::seats::{Connection, Decision, Seats, Ticket};
use std::{
    cell::RefCell,
    sync::{Arc, Mutex, Weak, mpsc},
    time::{Duration, Instant},
};
use sunrise_client::lan::{
    NetConfig,
    conn::{MEMORY_ADDRESS, serve_local, valid_name},
};
use sunrise_protocol::{connection::Pipe, crypto::offline_uuid, text::translatable};

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_GUESTS: usize = sunrise_client::split_layout::MAX_PLAYERS - 1;
type StockSeat = Arc<dyn Fn(u128) -> bool + Send + Sync>;

pub(crate) struct Gate {
    owner: u128,
    seats: Mutex<Seats>,
    stock_seat: Option<StockSeat>,
}

thread_local! {
    // Weak dispatch pointer on the integrated owner thread. The world's gate owns the state.
    static SERVER_GATE: RefCell<Weak<Gate>> = const { RefCell::new(Weak::new()) };
    // Only the server thread for the specific pipe opened below carries this token. It is not a
    // login packet field and can't be sent from another memory connection or from the network.
    static PIPE_TICKET: RefCell<Weak<Ticket>> = const { RefCell::new(Weak::new()) };
}

impl Gate {
    pub(crate) fn configure(net: &mut NetConfig) -> Option<Arc<Self>> {
        let (owner_name, owner) = net.singleplayer_profile.clone()?;
        if net.listen {
            return None;
        }
        let original = net.gate.take()?;
        let gate = Arc::new(Self {
            owner,
            seats: Mutex::new(Seats::new(owner_name, owner, MAX_GUESTS)),
            stock_seat: net.bypasses_limit.clone(),
        });
        let admission = Arc::clone(&gate);
        net.gate = Some(Arc::new(move |address, uuid, name| {
            let known_stock_seat = admission.is_stock_seat(uuid);
            let ticket = PIPE_TICKET.with_borrow(Weak::upgrade);
            let connection = if address == MEMORY_ADDRESS {
                Connection::Memory(ticket.as_deref())
            } else {
                Connection::Network
            };
            let decision = admission.seats.lock().map_or(Decision::Deny, |seats| {
                seats.decide(connection, uuid, name, known_stock_seat, Instant::now())
            });
            match decision {
                Decision::Stock => original(address, uuid, name),
                Decision::Allow => None,
                Decision::Deny => Some(translatable("multiplayer.disconnect.not_whitelisted", &[])),
            }
        }));
        Some(gate)
    }

    pub(crate) fn install(gate: Option<Arc<Self>>) {
        SERVER_GATE
            .with_borrow_mut(|slot| *slot = gate.as_ref().map_or_else(Weak::new, Arc::downgrade));
    }

    pub(crate) fn close() {
        let gate = SERVER_GATE.with_borrow_mut(|slot| std::mem::take(slot).upgrade());
        if let Some(gate) = gate
            && let Ok(mut seats) = gate.seats.lock()
        {
            seats.close();
        }
    }

    fn is_stock_seat(&self, uuid: u128) -> bool {
        self.stock_seat.as_ref().is_some_and(|seat| seat(uuid))
    }

    fn open_pipe(&self, game: &Game<'_>, ticket: &Arc<Ticket>) -> Option<Pipe> {
        let host = game.lan.as_ref()?;
        if host.owner.as_ref()?.uuid != self.owner
            || !host
                .hub
                .guests
                .iter()
                .any(|player| player.uuid == self.owner)
        {
            return None;
        }
        let known_stock_seat = self.is_stock_seat(ticket.uuid);
        let online = host
            .hub
            .guests
            .iter()
            .map(|player| (player.uuid, player.name.as_str()));
        if !self
            .seats
            .lock()
            .ok()?
            .reserve(ticket, known_stock_seat, online)
        {
            return None;
        }
        // Same server-end transport and worker as Lan::connect_local, but bind this connection
        // to its own reservation before any login processing can run.
        let (client, pipe) = Pipe::pair();
        let shared = Arc::clone(&host.hub.lan.shared);
        let ticket = Arc::downgrade(ticket);
        sunrise_guest_stack::spawn("sunrise-local-conn".into(), move || {
            PIPE_TICKET.with_borrow_mut(|slot| *slot = ticket);
            if let Err(error) = serve_local(pipe, shared)
                && std::env::var_os("SUNRISE_NET_DEBUG").is_some()
            {
                log::warn!("[lan] local: {error}");
            }
        })
        .ok()?;
        Some(client)
    }
}

/// The pending pipes owned by a client handler, scoped to one particular integrated world.
/// The first UI caller lands with the guest view; the local smoke fixture drives it in this PR.
#[derive(Default)]
pub(crate) struct WorldSeats {
    world: Weak<Flags>,
    pending: Vec<Pending>,
}

struct Pending {
    ticket: Arc<Ticket>,
    pipe: mpsc::Receiver<Option<Pipe>>,
}

pub(crate) struct SeatPipe {
    pub(crate) pipe: Pipe,
    // Dropping the guest revokes the ticket; the network gate holds only a Weak reference.
    pub(crate) lease: Arc<Ticket>,
}

impl WorldSeats {
    pub(crate) fn track(&mut self, game: &Game<'_>) {
        let world = game.integrated.as_ref().filter(|world| !world.stopped());
        if !world.is_some_and(|world| std::ptr::eq(Arc::as_ptr(&world.flags), self.world.as_ptr()))
        {
            self.pending.clear();
            self.world = world.map_or_else(Weak::new, |world| Arc::downgrade(&world.flags));
        }
        self.pending
            .retain(|pending| Instant::now() < pending.ticket.deadline);
    }

    pub(crate) fn request(&mut self, game: &Game<'_>, name: String) -> bool {
        self.track(game);
        let Some(server) = game.integrated.as_ref().filter(|server| !server.stopped()) else {
            return false;
        };
        if !valid_name(&name) || self.pending.len() >= MAX_GUESTS {
            return false;
        }
        let uuid = offline_uuid(&name);
        let ticket = Arc::new(Ticket::new(name, uuid, Instant::now() + HANDSHAKE_TIMEOUT));
        let (tx, pipe) = mpsc::sync_channel(1);
        let request = Arc::downgrade(&ticket);
        if !server.post(move |game| {
            let gate = SERVER_GATE.with_borrow(Weak::upgrade);
            let opened = match (gate, request.upgrade()) {
                (Some(gate), Some(ticket)) if Instant::now() < ticket.deadline => {
                    gate.open_pipe(game, &ticket)
                }
                _ => None,
            };
            let _ = tx.send(opened);
        }) {
            return false;
        }
        self.pending.push(Pending { ticket, pipe });
        true
    }

    pub(crate) fn next_ready(&mut self) -> Option<Option<SeatPipe>> {
        for (index, pending) in self.pending.iter().enumerate() {
            let pipe = match pending.pipe.try_recv() {
                Ok(pipe) => pipe,
                Err(mpsc::TryRecvError::Disconnected) => None,
                Err(mpsc::TryRecvError::Empty) => continue,
            };
            let Pending { ticket, .. } = self.pending.remove(index);
            return Some(pipe.map(|pipe| SeatPipe {
                pipe,
                lease: ticket,
            }));
        }
        None
    }
}
