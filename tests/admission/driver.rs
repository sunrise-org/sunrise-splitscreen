//! Opt-in smoke driver, compiled in a scratch copy of the package, never included in the release.
use super::{
    Game,
    splitscreen_admission::{SeatPipe, WorldSeats},
};
use std::{cell::RefCell, sync::Arc};
use sunrise_client::net::{Event, Session};
use sunrise_protocol::crypto::offline_uuid;

const NAME: &str = "SeatFixture";

#[derive(Default)]
struct Probe {
    seats: WorldSeats,
    phase: Phase,
}

#[derive(Default)]
enum Phase {
    #[default]
    Waiting,
    Requested,
    Duplicate(SeatPipe),
    Impostor {
        reserved: SeatPipe,
        session: Session,
    },
    Joining {
        lease: Arc<mod_splitscreen::seats::Ticket>,
        session: Session,
    },
    Playing {
        lease: Arc<mod_splitscreen::seats::Ticket>,
        session: Session,
    },
    PlayingConflict(Session),
    Dropped,
    Replay(Session),
    Passed,
}

thread_local! {
    static PROBE: RefCell<Probe> = RefCell::default();
}

pub(crate) fn tick(game: &Game<'_>) -> bool {
    PROBE.with_borrow_mut(|probe| probe.tick(game))
}

impl Probe {
    fn tick(&mut self, game: &Game<'_>) -> bool {
        let Some(server) = game.integrated.as_ref() else {
            return false;
        };
        self.seats.track(game);
        let uuid = offline_uuid(NAME);
        let local = |pipe| {
            Session::connect_local(
                pipe,
                NAME,
                None,
                game.settings.client_information(2),
                Arc::new(sunrise_client::net::load_language(&game.options.resources)),
                Arc::clone(&game.res.core.item_data),
            )
        };
        let roster = || {
            server
                .call(move |game| {
                    let host = game.lan.as_ref()?;
                    let owner = host.owner.as_ref()?;
                    assert!(!host.hub.ops.contains_key(&uuid));
                    Some((
                        host.hub.guests.iter().any(|guest| guest.uuid == owner.uuid),
                        host.hub.guests.iter().any(|guest| guest.uuid == uuid),
                    ))
                })
                .flatten()
        };
        let unreserved = || {
            local(
                server
                    .call(|game| game.lan.as_ref()?.hub.lan.connect_local().ok())
                    .flatten()
                    .unwrap(),
            )
        };
        self.phase = match std::mem::take(&mut self.phase) {
            Phase::Waiting if roster() == Some((true, false)) => {
                assert!(!self.seats.request(game, "Invalid name".into()));
                assert!(self.seats.request(game, NAME.into()));
                Phase::Requested
            }
            Phase::Requested => match self.seats.next_ready() {
                Some(Some(reserved)) => {
                    assert!(self.seats.request(game, NAME.to_ascii_lowercase()));
                    Phase::Duplicate(reserved)
                }
                Some(None) => panic!("first request was refused"),
                None => Phase::Requested,
            },
            Phase::Duplicate(reserved) => match self.seats.next_ready() {
                Some(None) => Phase::Impostor {
                    reserved,
                    session: unreserved(),
                },
                Some(Some(_)) => panic!("the same name in another case reserved a second seat"),
                None => Phase::Duplicate(reserved),
            },
            Phase::Impostor { reserved, session } => {
                if refused(&session) {
                    eprintln!("admission fixture: wrong pipe and duplicate name refused");
                    Phase::Joining {
                        lease: reserved.lease,
                        session: local(reserved.pipe),
                    }
                } else {
                    Phase::Impostor { reserved, session }
                }
            }
            Phase::Joining { lease, session } => {
                let mut joined = false;
                for receipt in session.poll() {
                    match receipt.into_parts().0 {
                        Event::Joined {
                            uuid: admitted,
                            name,
                            ..
                        } => {
                            assert_eq!((admitted, name.as_str()), (uuid, NAME));
                            joined = true;
                        }
                        Event::Disconnected { reason, .. } => {
                            panic!("reserved login failed: {reason}")
                        }
                        _ => {}
                    }
                }
                if joined {
                    Phase::Playing { lease, session }
                } else {
                    Phase::Joining { lease, session }
                }
            }
            Phase::Playing { lease, session } if roster() == Some((true, true)) => {
                eprintln!("admission fixture: both players online, no guest operator grant");
                // Keep the player online but remove the lease so only the roster can reject
                // the new reservation.
                drop(lease);
                assert!(self.seats.request(game, NAME.into()));
                Phase::PlayingConflict(session)
            }
            Phase::PlayingConflict(session) => match self.seats.next_ready() {
                Some(None) => Phase::Dropped,
                Some(Some(_)) => panic!("the playing guest's UUID reserved another seat"),
                None => Phase::PlayingConflict(session),
            },
            Phase::Dropped if roster() == Some((true, false)) => Phase::Replay(unreserved()),
            Phase::Replay(session) => {
                if refused(&session) {
                    eprintln!("admission fixture: PASS (owner stayed; unreserved replay refused)");
                    Phase::Passed
                } else {
                    Phase::Replay(session)
                }
            }
            phase => phase,
        };
        matches!(self.phase, Phase::Passed)
    }
}

fn refused(session: &Session) -> bool {
    for receipt in session.poll() {
        match receipt.into_parts().0 {
            Event::Disconnected {
                key, from_server, ..
            } => {
                assert!(from_server);
                assert_eq!(
                    key.as_deref(),
                    Some("multiplayer.disconnect.not_whitelisted")
                );
                return true;
            }
            Event::Joined { .. } => panic!("an unbound pipe used the ticket"),
            _ => {}
        }
    }
    false
}
