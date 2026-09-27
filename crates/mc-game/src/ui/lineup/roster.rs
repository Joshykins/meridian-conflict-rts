//! The seats of a match being set up: who plays each (a person, an AI, or
//! nobody), on which team, from which landing zone, in which colour and race.
//!
//! Seats in play always come first and closed seats after them. A network
//! lobby's seat `i` is the relay's seat `i` and the match's player `i`, so
//! closing or opening a seat must never move a seat somebody sits in: a seat
//! that closes goes to the back, one that opens joins the end of those in play,
//! and a change that would move an occupied seat is refused.

use crate::ui::faction::Pick;
use crate::ui::teams;
use glam::Vec2;
use mc_sim::AiConfig;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Control {
    /// A person plays it: yours in skirmish; in a lobby, whoever sits there
    /// (an AI plays it if nobody does).
    Person,
    Ai,
    Closed,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Seat {
    /// The row's identity on screen: its animations follow it when seats reorder.
    pub key: u8,
    pub control: Control,
    pub team: u8,
    /// Skirmish: the map's start position. Survival: an index into the theatre's spawns.
    pub start: u8,
    pub color: u8,
    pub race: Pick,
    pub ai: AiConfig,
}

impl Seat {
    pub fn open(&self) -> bool {
        self.control != Control::Closed
    }
}

/// Why a seat change was refused, said to the player.
pub type Refusal = String;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Roster {
    pub seats: Vec<Seat>,
}

impl Roster {
    /// `zones` seats: the first `people` for people, then `ai` AI commanders, the
    /// rest closed. Everyone on a team of their own (all on one for `allied`).
    pub fn new(zones: usize, people: usize, ai: usize, allied: bool) -> Roster {
        let zones = zones.min(8);
        Roster {
            seats: (0..zones)
                .map(|i| Seat {
                    key: i as u8,
                    control: if i < people {
                        Control::Person
                    } else if i < people + ai {
                        Control::Ai
                    } else {
                        Control::Closed
                    },
                    team: if allied { 0 } else { i as u8 },
                    start: i as u8,
                    color: i as u8,
                    race: Pick::default(),
                    ai: AiConfig::default(),
                })
                .collect(),
        }
    }

    /// Seats in play, from the first.
    pub fn in_play(&self) -> usize {
        self.seats.iter().take_while(|s| s.open()).count()
    }

    pub fn index_of(&self, key: u8) -> Option<usize> {
        self.seats.iter().position(|s| s.key == key)
    }

    /// The seated commanders' teams, in seat order.
    pub fn seated_teams(&self) -> Vec<u8> {
        self.seats
            .iter()
            .filter(|s| s.open())
            .map(|s| s.team)
            .collect()
    }

    /// Some commanders share a team: the list and the chart show sides.
    pub fn allied(&self) -> bool {
        teams::sizes(&self.seated_teams())
            .iter()
            .any(|&(_, n)| n > 1)
    }

    /// Another seat in play has this value of `field`.
    pub fn taken<T: PartialEq>(&self, except: usize, field: impl Fn(&Seat) -> T, value: T) -> bool {
        self.seats
            .iter()
            .enumerate()
            .any(|(i, s)| i != except && s.open() && field(s) == value)
    }

    /// Steps seat `i`'s value of a field to the next one (of `count`) nobody else uses.
    pub fn step_unique(
        &mut self,
        i: usize,
        by: i32,
        count: usize,
        get: impl Fn(&Seat) -> u8,
        set: impl Fn(&mut Seat, u8),
    ) {
        let mut v = get(&self.seats[i]) as i32;
        for _ in 0..count {
            v = (v + by).rem_euclid(count.max(1) as i32);
            if !self.taken(i, &get, v as u8) {
                set(&mut self.seats[i], v as u8);
                return;
            }
        }
    }

    /// Changes who plays seat `i`, keeping the seats in play first. `occupied`
    /// says which seats somebody sits in (a lobby); they never move. Returns
    /// where the seat is now.
    pub fn set_control(
        &mut self,
        i: usize,
        to: Control,
        zones: usize,
        occupied: impl Fn(usize) -> bool,
    ) -> Result<usize, Refusal> {
        let from = self.seats[i].control;
        if from == to {
            return Ok(i);
        }
        if occupied(i) && to != Control::Person {
            return Err("Someone sits there: remove them first".into());
        }
        let play = self.in_play();
        match (from == Control::Closed, to == Control::Closed) {
            // Closing: to the back, past the seats in play after it.
            (false, true) => {
                if let Some(j) = (i + 1..play).find(|&j| occupied(j)) {
                    return Err(format!(
                        "Seat {} is taken: close a seat after it instead",
                        j + 1
                    ));
                }
                let mut seat = self.seats.remove(i);
                seat.control = to;
                self.seats.push(seat);
                Ok(self.seats.len() - 1)
            }
            // Opening: to the end of the seats in play, on a free zone and colour.
            (true, false) => {
                let mut seat = self.seats.remove(i);
                seat.control = to;
                self.seats.insert(play, seat);
                if self.taken(play, |s| s.start, seat.start) {
                    self.step_unique(play, 1, zones, |s| s.start, |s, v| s.start = v);
                }
                if self.taken(play, |s| s.color, seat.color) {
                    let colours = crate::setup::TEAM_COLORS.len();
                    self.step_unique(play, 1, colours, |s| s.color, |s, v| s.color = v);
                }
                Ok(play)
            }
            _ => {
                self.seats[i].control = to;
                Ok(i)
            }
        }
    }

    /// Seat `i` deploys at zone `n`; whoever held it takes seat `i`'s.
    pub fn take_zone(&mut self, i: usize, n: u8) {
        let mine = self.seats[i].start;
        if let Some(j) = (0..self.seats.len())
            .find(|&j| j != i && self.seats[j].open() && self.seats[j].start == n)
        {
            self.seats[j].start = mine;
        }
        self.seats[i].start = n;
    }

    /// Seat `i` wears colour `n`; whoever wore it takes seat `i`'s.
    pub fn take_color(&mut self, i: usize, n: u8) {
        let mine = self.seats[i].color;
        if let Some(j) = (0..self.seats.len())
            .find(|&j| j != i && self.seats[j].open() && self.seats[j].color == n)
        {
            self.seats[j].color = mine;
        }
        self.seats[i].color = n;
    }

    /// Re-teams the seats in play into `groups` sides by where their zones
    /// (`at`, by start) lie, so allies start next to each other. `keep`'s side
    /// (yours) stays Team 1.
    pub fn teams_by_ground(&mut self, at: &[Vec2], groups: usize, keep: usize) {
        let seated: Vec<usize> = (0..self.seats.len())
            .filter(|&i| self.seats[i].open())
            .collect();
        let points: Vec<Vec2> = seated
            .iter()
            .map(|&i| {
                at.get(self.seats[i].start as usize)
                    .copied()
                    .unwrap_or(Vec2::ZERO)
            })
            .collect();
        let split = teams::by_ground(&points, groups);
        let first = seated
            .iter()
            .position(|&i| i == keep)
            .map_or(0, |k| split[k]);
        for (k, &i) in seated.iter().enumerate() {
            let t = split[k];
            self.seats[i].team = if t == first {
                0
            } else if t == 0 {
                first
            } else {
                t
            };
        }
    }

    /// Keeps the roster possible on a map with `zones` zones: seats beyond it
    /// go, zones and colours stay unique. Fails when a seat that would go is occupied.
    pub fn fit(&mut self, zones: usize, occupied: impl Fn(usize) -> bool) -> Result<(), Refusal> {
        let zones = zones.clamp(1, 8);
        if let Some(i) = (zones..self.seats.len()).find(|&i| self.seats[i].open() && occupied(i)) {
            return Err(format!(
                "Seat {} is taken, and the map has {zones} landing zones",
                i + 1
            ));
        }
        // Drop the surplus from the back, closed seats first.
        while self.seats.len() > zones {
            let at = self
                .seats
                .iter()
                .rposition(|s| !s.open())
                .unwrap_or(self.seats.len() - 1);
            self.seats.remove(at);
        }
        while self.seats.len() < zones {
            let key = (0..8u8).find(|k| self.index_of(*k).is_none()).unwrap_or(0);
            self.seats.push(Seat {
                key,
                control: Control::Closed,
                team: key,
                start: key,
                color: key,
                race: Pick::default(),
                ai: AiConfig::default(),
            });
        }
        // Zones and colours unique among the seats in play.
        for i in 0..self.seats.len() {
            if !self.seats[i].open() {
                continue;
            }
            if self.seats[i].start as usize >= zones
                || self.taken(i, |s| s.start, self.seats[i].start)
            {
                self.seats[i].start = (zones - 1) as u8;
                self.step_unique(i, 1, zones, |s| s.start, |s, v| s.start = v);
            }
            if self.taken(i, |s| s.color, self.seats[i].color) {
                let colours = crate::setup::TEAM_COLORS.len();
                self.step_unique(i, 1, colours, |s| s.color, |s, v| s.color = v);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(r: &Roster) -> Vec<u8> {
        r.seats.iter().map(|s| s.key).collect()
    }

    #[test]
    fn seats_in_play_stay_first_and_occupied_seats_never_move() {
        // Four zones: you, a friend, an AI, one closed.
        let mut r = Roster::new(4, 2, 1, false);
        assert_eq!(r.in_play(), 3);
        let nobody = |_: usize| false;
        // Opening the closed seat puts it at the end of those in play.
        assert_eq!(r.set_control(3, Control::Ai, 4, nobody), Ok(3));
        assert_eq!(r.in_play(), 4);
        // Closing seat 1 sends it to the back; the seats after it move up.
        assert_eq!(r.set_control(1, Control::Closed, 4, nobody), Ok(3));
        assert_eq!(keys(&r), vec![0, 2, 3, 1]);
        assert_eq!(r.in_play(), 3);
        // With someone in seat 2, closing seat 1 would move them: refused.
        let sat = |i: usize| i == 0 || i == 2;
        assert!(r.set_control(1, Control::Closed, 4, sat).is_err());
        assert_eq!(keys(&r), vec![0, 2, 3, 1], "nothing moved");
        // An occupied seat cannot be made an AI either; the last seat in play can close.
        assert!(r.set_control(0, Control::Ai, 4, sat).is_err());
        let only_first = |i: usize| i == 0;
        assert_eq!(r.set_control(2, Control::Closed, 4, only_first), Ok(3));
        assert_eq!(r.in_play(), 2);
    }

    #[test]
    fn opening_a_seat_finds_a_free_zone_and_colour() {
        let mut r = Roster::new(3, 1, 1, false);
        // The closed seat's zone and colour were taken while it was closed.
        r.seats[2].start = 0;
        r.seats[2].color = 1;
        let at = r.set_control(2, Control::Ai, 3, |_| false).unwrap();
        let s = r.seats[at];
        assert!(!r.taken(at, |x| x.start, s.start));
        assert!(!r.taken(at, |x| x.color, s.color));
    }

    #[test]
    fn taking_a_zone_or_colour_swaps_with_its_holder() {
        let mut r = Roster::new(3, 1, 2, false);
        r.take_zone(0, 2);
        assert_eq!((r.seats[0].start, r.seats[2].start), (2, 0));
        r.take_color(1, 0);
        assert_eq!((r.seats[1].color, r.seats[0].color), (0, 1));
    }

    #[test]
    fn fitting_a_smaller_map_drops_closed_seats_and_refuses_an_occupied_one() {
        let mut r = Roster::new(8, 2, 2, false);
        assert!(r.fit(4, |_| false).is_ok());
        assert_eq!(r.seats.len(), 4);
        assert_eq!(r.in_play(), 4);
        let mut r = Roster::new(8, 3, 0, false);
        assert!(r.fit(2, |i| i == 2).is_err(), "seat 3 is taken");
        assert!(r.fit(4, |i| i == 2).is_ok());
        assert!(r.seats.iter().all(|s| (s.start as usize) < 4));
        // Growing adds closed seats with fresh keys.
        assert!(r.fit(6, |_| false).is_ok());
        assert_eq!(r.seats.len(), 6);
        let mut k = keys(&r);
        k.sort_unstable();
        k.dedup();
        assert_eq!(k.len(), 6);
    }
}
