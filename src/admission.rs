//! An integrated world's in-process seats. The stock owner and LAN policy still governs every
//! connection except an explicitly reserved local seat.
use super::{Flags, Game};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak, mpsc};
use std::time::{Duration, Instant};
use sunrise_client::lan::{
    NetConfig,
    conn::{MEMORY_ADDRESS, valid_name},
};
use sunrise_protocol::{connection::Pipe, crypto::offline_uuid, text::translatable};

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_GUESTS: usize = 15;

struct Ticket {
    name: String,
    uuid: u128,
    deadline: Instant,
    used: AtomicBool,
}

pub(crate) struct Gate {
    owner_name: String,
    owner: u128,
    tickets: Mutex<Option<HashMap<u128, Weak<Ticket>>>>,
}

thread_local! {
    // A dispatch pointer for work already queued to this server thread, not a world owner.
    // The world's network gate owns the only lasting strong reference.
    static SERVER_GATE: RefCell<Weak<Gate>> = const { RefCell::new(Weak::new()) };
}

impl Gate {
    pub(crate) fn configure(net: &mut NetConfig) -> Option<Arc<Self>> {
        let (owner_name, owner) = net.singleplayer_profile.clone()?;
        if net.listen {
            return None;
        }
        let original = net.gate.take()?;
        let stock_seat = net.bypasses_limit.clone();
        let gate = Arc::new(Self {
            owner_name,
            owner,
            tickets: Mutex::new(Some(HashMap::new())),
        });
        let admission = Arc::clone(&gate);
        net.gate = Some(Arc::new(move |address, uuid, name| {
            let memory = address == MEMORY_ADDRESS;
            let ticket = admission
                .tickets
                .lock()
                .ok()
                .and_then(|tickets| tickets.as_ref()?.get(&uuid)?.upgrade());
            if let Some(ticket) = ticket {
                if memory
                    && ticket.name == name
                    && Instant::now() < ticket.deadline
                    && !ticket.used.swap(true, Ordering::Relaxed)
                {
                    return None;
                }
                return Some(translatable("multiplayer.disconnect.not_whitelisted", &[]));
            }
            // In a published world the stock gate accepts every unreserved memory connection.
            // Only P1, an existing built-in seat on this SDK, or a one-shot ticket can use memory.
            if memory && uuid != owner && !stock_seat.as_ref().is_some_and(|seat| seat(uuid)) {
                return Some(translatable("multiplayer.disconnect.not_whitelisted", &[]));
            }
            original(address, uuid, name)
        }));
        Some(gate)
    }

    pub(crate) fn install(gate: Option<Arc<Self>>) {
        SERVER_GATE
            .with(|slot| *slot.borrow_mut() = gate.as_ref().map_or_else(Weak::new, Arc::downgrade));
    }

    pub(crate) fn close() {
        let gate = SERVER_GATE.with(|slot| std::mem::take(&mut *slot.borrow_mut()).upgrade());
        if let Some(gate) = gate
            && let Ok(mut tickets) = gate.tickets.lock()
        {
            *tickets = None;
        }
    }

    fn open_pipe(&self, game: &Game<'_>, ticket: &Arc<Ticket>) -> Option<Pipe> {
        let host = game.lan.as_ref()?;
        let owner = host.owner.as_ref()?;
        if owner.uuid != self.owner
            || !host
                .hub
                .guests
                .iter()
                .any(|player| player.uuid == self.owner)
            || ticket.uuid == self.owner
            || ticket.name.eq_ignore_ascii_case(&self.owner_name)
            || host.hub.guests.iter().any(|player| {
                player.uuid == ticket.uuid || player.name.eq_ignore_ascii_case(&ticket.name)
            })
        {
            return None;
        }
        let mut table = self.tickets.lock().ok()?;
        let tickets = table.as_mut()?;
        tickets.retain(|_, ticket| ticket.strong_count() > 0);
        if tickets.len() >= MAX_GUESTS
            || tickets.values().filter_map(Weak::upgrade).any(|other| {
                other.uuid == ticket.uuid || other.name.eq_ignore_ascii_case(&ticket.name)
            })
        {
            return None;
        }
        tickets.insert(ticket.uuid, Arc::downgrade(ticket));
        // The connection thread cannot present a hello before the pipe is returned to its caller.
        host.hub
            .lan
            .connect_local()
            .inspect_err(|_| {
                tickets.remove(&ticket.uuid);
            })
            .ok()
    }
}

/// The pending pipes owned by the window, keyed to the exact integrated server rather than to
/// a world name or save path that might be reopened. A guest view keeps its ticket while it plays.
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
    // Dropping a pending join or guest revokes the ticket. The gate only holds its Weak pointer.
    _ticket: Arc<Ticket>,
}

impl WorldSeats {
    pub(crate) fn track(&mut self, game: &Game<'_>) {
        let world = game.integrated.as_ref().filter(|world| !world.stopped());
        if !world.is_some_and(|world| std::ptr::eq(Arc::as_ptr(&world.flags), self.world.as_ptr()))
        {
            self.clear();
            if let Some(world) = world {
                self.world = Arc::downgrade(&world.flags);
            }
        }
        self.pending
            .retain(|pending| Instant::now() < pending.ticket.deadline);
    }

    pub(crate) fn clear(&mut self) {
        self.pending.clear();
        self.world = Weak::new();
    }

    // The guest renderer/profile flow will request a pipe, then retain the returned seat lease for
    // its session. Nothing requests a seat automatically on startup.
    pub(crate) fn request(&mut self, game: &Game<'_>, name: String) -> bool {
        self.track(game);
        let Some(server) = game.integrated.as_ref().filter(|server| !server.stopped()) else {
            return false;
        };
        if !valid_name(&name) || self.pending.len() >= MAX_GUESTS {
            return false;
        }
        let ticket = Arc::new(Ticket {
            uuid: offline_uuid(&name),
            name,
            deadline: Instant::now() + HANDSHAKE_TIMEOUT,
            used: AtomicBool::new(false),
        });
        let (tx, pipe) = mpsc::sync_channel(1);
        let request = Arc::downgrade(&ticket);
        if !server.post(move |game| {
            let gate = SERVER_GATE.with(|slot| slot.borrow().upgrade());
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
                _ticket: ticket,
            }));
        }
        None
    }
}
