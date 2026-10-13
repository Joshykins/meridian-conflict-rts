//! Decks: bridges' roads over the water (`mc_map::deck`). A deck's cells are land and
//! water at once to the pathfinder (`nav::cell_class`), so land units cross on top
//! while ships sail under; which of the two a unit is on decides where it stands
//! and what it can bump into.
//!
//! A land unit over a deck is on it. A hover or amphibious unit is on it only if
//! it came onto it from the land at its ends (it was already near the deck's
//! height), so one that sails or drives in under a bridge stays on the water or
//! the bed. Ships never are, nor aircraft, which fly over anyway. Units on a deck
//! and units under it pass each other; nothing is built on a deck
//! (`World::lot_fits`); a shot coming down onto a deck stops there unless it was
//! fired from on it (`impacts.rs`).

use crate::World;
use mc_core::{Fx, FxVec2, FxVec3};
use mc_data::MoveLayer;

/// How far under a deck's top a hover or amphibious unit may already be and
/// still go onto the deck: the step a ramp's last cell could leave.
const DECK_STEP: Fx = Fx::from_int(8);

impl World {
    /// The top of the deck `row` stands on at `pos`, if it is on one: always a
    /// land unit over a deck; a hover or amphibious unit only one that came up
    /// onto it (its height last tick near the deck's).
    pub(crate) fn deck_under(&self, row: usize, pos: FxVec2) -> Option<Fx> {
        let decks = self.terrain.decks();
        if decks.is_empty() {
            return None;
        }
        let layer = self.bp(row).motion?.layer;
        let top = decks.top_at(pos)?;
        match layer {
            MoveLayer::Land => Some(top),
            MoveLayer::Hover | MoveLayer::Amphibious => {
                (self.state.units.z[row] > top - DECK_STEP).then_some(top)
            }
            MoveLayer::Naval | MoveLayer::Air => None,
        }
    }

    /// Whether `row` is on a deck where it stands now.
    pub(crate) fn on_deck(&self, row: usize) -> bool {
        self.deck_under(row, self.state.units.pos[row]).is_some()
    }

    /// Whether two ground units pass each other where they stand: a ship and a
    /// dived submarine (`hulls_pass`), or one on a deck and the other under it.
    pub(crate) fn apart(&self, a: usize, b: usize) -> bool {
        self.hulls_pass(a, b)
            || (!self.terrain.decks().is_empty() && self.on_deck(a) != self.on_deck(b))
    }

    /// Whether a deck stands between a burst at `point` and `row`: the burst on
    /// a deck's top and the unit under that deck, or the burst under a deck and
    /// the unit on it. A deck takes the blast, whatever its reach.
    pub(crate) fn deck_shields(&self, point: FxVec3, row: usize) -> bool {
        let decks = self.terrain.decks();
        if decks.is_empty() {
            return false;
        }
        let units = &self.state.units;
        match self.deck_under(row, units.pos[row]) {
            Some(top) => {
                point.z < top - Fx::ONE && decks.top_at(point.xy()).is_some_and(|t| t >= top)
            }
            None => decks
                .top_at(units.pos[row])
                .is_some_and(|top| units.z[row] < top && point.z >= top - Fx::ONE),
        }
    }

    /// The surface a unit of `layer` is put down on at `pos` when it first
    /// appears: a deck's top for anything that would walk or float over it.
    pub(crate) fn spawn_surface(&self, layer: Option<MoveLayer>, pos: FxVec2, surface: Fx) -> Fx {
        match layer {
            Some(MoveLayer::Land | MoveLayer::Hover | MoveLayer::Amphibious) => self
                .terrain
                .decks()
                .top_at(pos)
                .map_or(surface, |top| surface.max(top)),
            _ => surface,
        }
    }
}
