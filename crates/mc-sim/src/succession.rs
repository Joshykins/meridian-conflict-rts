//! What becomes of a defeated side's forces: lost with its commander
//! (assassination), or handed to a living ally to fight on with.

use crate::tables::Controller;
use crate::World;
use serde::{Deserialize, Serialize};

/// The match's rule for a side whose commander has fallen (or who resigned).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FallenArmy {
    /// Every unit the side has goes with its commander.
    #[default]
    Destroyed = 0,
    /// A living ally takes over every unit the side has, and what it had in store.
    /// With no ally left standing the army is destroyed.
    ToAlly = 1,
}

impl World {
    /// Sets the fallen army rule. Call once, right after `World::new`.
    pub fn set_fallen_army(&mut self, rule: FallenArmy) {
        self.state.fallen_army = rule;
    }

    /// The ally who takes over `player`'s army when it falls, if the rule hands
    /// it on and anyone on its team still stands: a person before an AI, and
    /// among those the lowest seat.
    pub(crate) fn heir(&self, player: u8) -> Option<u8> {
        if self.state.fallen_army != FallenArmy::ToAlly {
            return None;
        }
        let players = &self.state.players;
        let team = players[player as usize].team;
        let standing = |i: &usize| {
            let p = &players[*i];
            *i != player as usize && p.team == team && !p.defeated
        };
        let mut seats = (0..players.len()).filter(standing);
        let first_person = seats
            .clone()
            .find(|&i| players[i].controller == Controller::Human);
        first_person.or_else(|| seats.next()).map(|i| i as u8)
    }

    /// `heir` takes over everything `from` had: every unit (a resigned side's
    /// commander too, which fights on for the heir but no longer decides
    /// anyone's defeat), the work in hand that remembers its side, and the
    /// store. All of it goes to the one heir, so what must share an owner (a
    /// lift ship and its cargo, a carrier and its drones, a factory and what it
    /// is building) still does.
    pub(crate) fn hand_over(&mut self, from: u8, heir: u8) {
        let s = &mut self.state;
        let units = &mut s.units;
        for row in 0..units.slots.rows() {
            if units.slots.is_alive(row) && units.owner[row] == from {
                units.owner[row] = heir;
            }
        }
        let take = |owner: &mut u8| {
            if *owner == from {
                *owner = heir;
            }
        };
        s.batches.values_mut().for_each(|b| take(&mut b.owner));
        s.site_repeats
            .waiting
            .iter_mut()
            .for_each(|r| take(&mut r.owner));
        s.wells.iter_mut().for_each(|w| take(&mut w.owner));
        s.lenses.iter_mut().for_each(|t| take(&mut t.owner));
        // The heir's storage grows by what it took over; the economy holds the
        // store to it from the next tick.
        let (materials, energy) = {
            let p = &mut s.players[from as usize];
            (
                std::mem::replace(&mut p.materials, mc_core::Fx::ZERO),
                std::mem::replace(&mut p.energy, mc_core::Fx::ZERO),
            )
        };
        let p = &mut s.players[heir as usize];
        p.materials += materials;
        p.energy += energy;
    }
}
