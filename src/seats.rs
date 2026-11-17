//! Ticket policy shared by the native patch and its tests. The patch attaches a ticket to the
//! server end of its own in-process pipe; the login name and UUID alone don't prove possession.
use std::{
    collections::HashMap,
    sync::{
        Arc, Weak,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

pub struct Ticket {
    pub name: String,
    pub uuid: u128,
    pub deadline: Instant,
    used: AtomicBool,
}

impl Ticket {
    pub fn new(name: String, uuid: u128, deadline: Instant) -> Self {
        Self {
            name,
            uuid,
            deadline,
            used: AtomicBool::new(false),
        }
    }
}

pub enum Connection<'a> {
    Network,
    Memory(Option<&'a Ticket>),
}

#[derive(Debug, PartialEq, Eq)]
pub enum Decision {
    Stock,
    Allow,
    Deny,
}

pub struct Seats {
    owner_name: String,
    owner: u128,
    max: usize,
    tickets: Option<HashMap<u128, Weak<Ticket>>>,
}

impl Seats {
    pub fn new(owner_name: String, owner: u128, max: usize) -> Self {
        Self {
            owner_name,
            owner,
            max,
            tickets: Some(HashMap::new()),
        }
    }

    pub fn reserve<'a>(
        &mut self,
        ticket: &Arc<Ticket>,
        stock_seat: bool,
        online: impl IntoIterator<Item = (u128, &'a str)>,
    ) -> bool {
        let clashes =
            |uuid, name: &str| uuid == ticket.uuid || name.eq_ignore_ascii_case(&ticket.name);
        if stock_seat
            || clashes(self.owner, &self.owner_name)
            || online.into_iter().any(|(uuid, name)| clashes(uuid, name))
        {
            return false;
        }
        let Some(tickets) = &mut self.tickets else {
            return false;
        };
        tickets.retain(|_, ticket| ticket.strong_count() > 0);
        if tickets.len() >= self.max
            || tickets
                .values()
                .filter_map(Weak::upgrade)
                .any(|other| clashes(other.uuid, &other.name))
        {
            return false;
        }
        tickets.insert(ticket.uuid, Arc::downgrade(ticket));
        true
    }

    pub fn decide(
        &self,
        connection: Connection<'_>,
        uuid: u128,
        name: &str,
        stock_seat: bool,
        now: Instant,
    ) -> Decision {
        if stock_seat || uuid == self.owner {
            return match connection {
                Connection::Memory(Some(_)) => Decision::Deny,
                _ => Decision::Stock,
            };
        }
        let reserved = self
            .tickets
            .as_ref()
            .and_then(|tickets| tickets.get(&uuid)?.upgrade());
        match (connection, reserved) {
            (Connection::Network, None) => Decision::Stock,
            (Connection::Memory(Some(presented)), Some(ticket))
                if std::ptr::eq(presented, ticket.as_ref())
                    && ticket.name == name
                    && now < ticket.deadline
                    && !ticket.used.swap(true, Ordering::Relaxed) =>
            {
                Decision::Allow
            }
            _ => Decision::Deny,
        }
    }

    pub fn close(&mut self) {
        self.tickets = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn guest(now: Instant, name: &str, id: u128) -> Arc<Ticket> {
        Arc::new(Ticket::new(name.into(), id, now + Duration::from_secs(1)))
    }

    #[test]
    fn only_the_issued_pipe_can_consume_a_ticket_once() {
        let now = Instant::now();
        let mut seats = Seats::new("Owner".into(), 1, 15);
        let reserved = guest(now, "Guest", 2);
        let other = guest(now, "Guest", 2);
        assert!(seats.reserve(&reserved, false, []));
        assert_eq!(
            seats.decide(Connection::Memory(None), 2, "Guest", false, now),
            Decision::Deny
        );
        assert_eq!(
            seats.decide(Connection::Memory(Some(&other)), 2, "Guest", false, now),
            Decision::Deny
        );
        assert_eq!(
            seats.decide(Connection::Memory(Some(&reserved)), 2, "Other", false, now),
            Decision::Deny
        );
        assert_eq!(
            seats.decide(Connection::Memory(Some(&reserved)), 3, "Guest", false, now),
            Decision::Deny
        );
        assert_eq!(
            seats.decide(Connection::Network, 2, "Guest", false, now),
            Decision::Deny
        );
        assert_eq!(
            seats.decide(Connection::Memory(Some(&reserved)), 2, "Guest", false, now),
            Decision::Allow
        );
        assert_eq!(
            seats.decide(Connection::Memory(Some(&reserved)), 2, "Guest", false, now),
            Decision::Deny
        );
    }

    #[test]
    fn expired_dropped_and_closed_reservations_cannot_admit() {
        let now = Instant::now();
        let mut seats = Seats::new("Owner".into(), 1, 1);
        let reserved = guest(now, "Guest", 2);
        assert!(seats.reserve(&reserved, false, []));
        assert_eq!(
            seats.decide(
                Connection::Memory(Some(&reserved)),
                2,
                "Guest",
                false,
                reserved.deadline
            ),
            Decision::Deny
        );
        drop(reserved);
        assert_eq!(
            seats.decide(Connection::Memory(None), 2, "Guest", false, now),
            Decision::Deny
        );
        let next = guest(now, "Guest", 2);
        assert!(seats.reserve(&next, false, []));
        seats.close();
        assert_eq!(
            seats.decide(Connection::Memory(Some(&next)), 2, "Guest", false, now),
            Decision::Deny
        );
        assert!(!seats.reserve(&next, false, []));
    }

    #[test]
    fn owner_lan_and_stock_seats_are_left_to_the_existing_policy() {
        let now = Instant::now();
        let mut seats = Seats::new("Owner".into(), 1, 1);
        let reserved = guest(now, "Guest", 2);
        assert!(seats.reserve(&reserved, false, []));
        assert_eq!(
            seats.decide(Connection::Memory(None), 1, "Owner", false, now),
            Decision::Stock
        );
        assert_eq!(
            seats.decide(Connection::Network, 3, "OrdinaryLan", false, now),
            Decision::Stock
        );
        assert_eq!(
            seats.decide(Connection::Memory(None), 2, "Guest", true, now),
            Decision::Stock
        );
        assert_eq!(
            seats.decide(Connection::Memory(Some(&reserved)), 2, "Guest", true, now),
            Decision::Deny
        );
    }

    #[test]
    fn owner_stock_duplicates_online_players_and_capacity_cannot_be_reserved() {
        let now = Instant::now();
        let mut seats = Seats::new("Owner".into(), 1, 2);
        assert!(!seats.reserve(&guest(now, "NotOwner", 1), false, []));
        assert!(!seats.reserve(&guest(now, "oWNER", 2), false, []));
        assert!(!seats.reserve(&guest(now, "Unrelated", 3), true, []));
        assert!(!seats.reserve(&guest(now, "Guest", 4), false, [(5, "gUeSt")]));
        assert!(!seats.reserve(&guest(now, "Guest", 4), false, [(4, "OtherGuest")]));
        let first = guest(now, "Guest", 4);
        assert!(seats.reserve(&first, false, []));
        assert!(!seats.reserve(&guest(now, "guest", 5), false, []));
        assert!(!seats.reserve(&guest(now, "OtherGuest", 4), false, []));
        let second = guest(now, "OtherGuest", 5);
        assert!(seats.reserve(&second, false, []));
        let third = guest(now, "NextGuest", 6);
        assert!(!seats.reserve(&third, false, []));
        drop(first);
        assert!(seats.reserve(&third, false, []));
    }
}
