//! The storage structures' fill gauges and status lamps (the Capacitor Bank, the
//! Materials Vault): presentation only. [`StoreWatch`] reads each side's stores as the
//! render mirror is written and packs a word into the storage instances' `status[2]`;
//! nothing here goes back into `State`. The shaders read the word by
//! `mc_models::gpu_consts::store`'s copy of these bits (a test holds them equal).

use std::collections::VecDeque;

use mc_core::{Fx, TICKS_PER_SECOND};

use crate::mirror::{RenderFrame, KIND_GHOST, KIND_PROP, KIND_WRECK};
use crate::tables::{flag, Player};
use crate::world::World;
use crate::Handle;

/// The store word's fill: how full the side's store is, 0 empty to 255 full.
pub const STORE_FILL_MASK: u32 = 0xFF;
/// The store word's [`StoreState`], two bits from here.
pub const STORE_STATE_SHIFT: u32 = 8;
pub const STORE_STATE_MASK: u32 = 0x3;
/// Set on a word the mirror wrote; without it the model shows its authored look.
pub const STORE_MARK: u32 = 1 << 12;

/// What a storage structure's lamps say about its side's store.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u32)]
pub enum StoreState {
    /// Filling or holding: the lamps are dark.
    #[default]
    Neutral = 0,
    /// The store has been falling for [`WINDOW_TICKS`]: amber.
    Draining = 1,
    /// Dry: red.
    Empty = 2,
    /// Full: green.
    Full = 3,
}

/// How far back a store is compared to judge it draining: a second.
const WINDOW_TICKS: u32 = TICKS_PER_SECOND;
/// Below this the store is dry (as the shields judge it, `shields::DRY`).
const DRY: Fx = Fx::ONE;
/// Full from 99%, and it stays full until it falls under 97%.
const FULL_ON: Fx = Fx::ratio(99, 100);
const FULL_OFF: Fx = Fx::ratio(97, 100);
/// A store counts as draining once it has fallen this share of its capacity in the
/// window (and at least half a unit), and stops once it no longer falls.
const DRAIN_SHARE: i32 = 500;

/// Each side's two stores, watched from one mirror write to the next.
#[derive(Clone, Debug, Default)]
pub struct StoreWatch {
    /// Per player: materials, then energy.
    players: Vec<[Gauge; 2]>,
}

/// One store: what it held over the last window, and what its lamps say.
#[derive(Clone, Debug, Default)]
struct Gauge {
    samples: VecDeque<(u32, Fx)>,
    state: StoreState,
}

impl Gauge {
    fn update(&mut self, tick: u32, stock: Fx, capacity: Fx) {
        if self.samples.back().is_some_and(|&(t, _)| t > tick) {
            // A seek or a restart: the history is of another timeline.
            self.samples.clear();
            self.state = StoreState::Neutral;
        }
        if self.samples.back().is_none_or(|&(t, _)| t < tick) {
            self.samples.push_back((tick, stock));
        }
        // Keep one sample at or before the window's start, none older.
        while self.samples.len() > 1 && self.samples[1].0 + WINDOW_TICKS <= tick {
            self.samples.pop_front();
        }
        let old = self
            .samples
            .front()
            .filter(|&&(t, _)| t + WINDOW_TICKS <= tick)
            .map(|&(_, v)| v);
        self.state = classify(self.state, stock, capacity, old);
    }

    fn word(&self, stock: Fx, capacity: Fx) -> u32 {
        let fill = if capacity > Fx::ZERO {
            (stock.clamp(Fx::ZERO, capacity) * 255 / capacity).round_int() as u32
        } else {
            0
        };
        STORE_MARK | fill.min(STORE_FILL_MASK) | (self.state as u32) << STORE_STATE_SHIFT
    }
}

/// The lamps' state for a store holding `stock` of `capacity`, which held `old` a
/// window ago (`None` while there is no window of history yet), given what they said
/// last time. Dry wins, then full, then draining.
pub fn classify(prev: StoreState, stock: Fx, capacity: Fx, old: Option<Fx>) -> StoreState {
    if capacity <= Fx::ZERO {
        return StoreState::Neutral;
    }
    if stock < DRY {
        return StoreState::Empty;
    }
    let full_at = if prev == StoreState::Full {
        FULL_OFF
    } else {
        FULL_ON
    };
    if stock >= capacity * full_at {
        return StoreState::Full;
    }
    let fell = old.map_or(Fx::ZERO, |o| o - stock);
    let enough = if prev == StoreState::Draining {
        Fx::ZERO
    } else {
        (capacity / DRAIN_SHARE).max(Fx::ONE / 2)
    };
    if fell > enough {
        StoreState::Draining
    } else {
        StoreState::Neutral
    }
}

fn stores(p: &Player) -> [(Fx, Fx); 2] {
    [(p.mass, p.mass_capacity), (p.energy, p.energy_capacity)]
}

impl StoreWatch {
    /// Watches every side's stores at `tick`.
    fn update(&mut self, players: &[Player], tick: u32) {
        self.players.resize_with(players.len(), Default::default);
        for (gauges, p) in self.players.iter_mut().zip(players) {
            for (g, (stock, capacity)) in gauges.iter_mut().zip(stores(p)) {
                g.update(tick, stock, capacity);
            }
        }
    }
}

impl World {
    /// Writes the store word into every finished storage structure in `frame` whose side
    /// the viewer may read (its own team's; everyone's with no viewer), after the units
    /// are in it. Energy storage shows the energy store, materials storage the materials.
    pub(crate) fn write_store_lights(&self, viewer: Option<u8>, frame: &mut RenderFrame) {
        let s = &self.state;
        let mut watch = std::mem::take(&mut frame.stores);
        watch.update(&s.players, s.tick);
        let team = |p: u8| s.players.get(p as usize).map(|p| p.team);
        for u in frame.units.iter_mut() {
            if u.owner_flags & (KIND_WRECK | KIND_PROP | KIND_GHOST) != 0 || u.status[2] != 0 {
                continue;
            }
            let Some(row) = s.units.row(Handle(u.unit_id)) else {
                continue;
            };
            let bp = self.bp(row);
            let owner = s.units.owner[row];
            let which = if bp.economy.energy_storage > Fx::ZERO {
                1
            } else if bp.economy.mass_storage > Fx::ZERO {
                0
            } else {
                continue;
            };
            if bp.footprint == (0, 0)
                || s.units.has_flag(row, flag::UNDER_CONSTRUCTION)
                || viewer.is_some_and(|v| team(v) != team(owner))
            {
                continue;
            }
            let (Some(gauges), Some(p)) = (
                watch.players.get(owner as usize),
                s.players.get(owner as usize),
            ) else {
                continue;
            };
            let (stock, capacity) = stores(p)[which];
            u.status[2] = gauges[which].word(stock, capacity);
        }
        frame.stores = watch;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CAP: Fx = Fx::from_int(5000);

    #[test]
    fn dry_is_empty_and_wins() {
        let s = classify(StoreState::Full, Fx::ZERO, CAP, Some(CAP));
        assert_eq!(s, StoreState::Empty);
        assert_eq!(
            classify(StoreState::Neutral, Fx::ratio(1, 2), CAP, None),
            StoreState::Empty
        );
    }

    #[test]
    fn full_from_99_percent_until_under_97() {
        let at = |pct: i32| CAP * Fx::ratio(pct as i64, 100);
        assert_eq!(
            classify(StoreState::Neutral, at(98), CAP, None),
            StoreState::Neutral
        );
        assert_eq!(
            classify(StoreState::Neutral, at(99), CAP, None),
            StoreState::Full
        );
        assert_eq!(
            classify(StoreState::Full, at(98), CAP, None),
            StoreState::Full
        );
        assert_eq!(
            classify(StoreState::Full, at(96), CAP, None),
            StoreState::Neutral
        );
    }

    #[test]
    fn draining_needs_a_real_fall_and_holds_while_falling() {
        let half = CAP / 2;
        // No window of history yet: not draining.
        assert_eq!(
            classify(StoreState::Neutral, half, CAP, None),
            StoreState::Neutral
        );
        // A wobble of a unit is not a drain.
        assert_eq!(
            classify(StoreState::Neutral, half, CAP, Some(half + Fx::ONE)),
            StoreState::Neutral
        );
        // A fall of more than 0.2% of the store in the window is.
        let fell = half + CAP / 400;
        assert_eq!(
            classify(StoreState::Neutral, half, CAP, Some(fell)),
            StoreState::Draining
        );
        // Once draining, any fall keeps it so; holding or rising ends it.
        let tiny = half + Fx::ONE / 10;
        assert_eq!(
            classify(StoreState::Draining, half, CAP, Some(tiny)),
            StoreState::Draining
        );
        assert_eq!(
            classify(StoreState::Draining, half, CAP, Some(half)),
            StoreState::Neutral
        );
    }

    #[test]
    fn no_store_is_neutral() {
        assert_eq!(
            classify(StoreState::Full, Fx::ZERO, Fx::ZERO, None),
            StoreState::Neutral
        );
    }

    #[test]
    fn the_gauge_judges_a_second_of_history() {
        let mut g = Gauge::default();
        let mut stock = Fx::from_int(3000);
        // Falling 10 a tick (100 a second, 2% of the store): draining once a second has passed.
        for tick in 0..WINDOW_TICKS {
            g.update(tick, stock, CAP);
            assert_eq!(g.state, StoreState::Neutral, "tick {tick}");
            stock -= Fx::from_int(10);
        }
        g.update(WINDOW_TICKS, stock, CAP);
        assert_eq!(g.state, StoreState::Draining);
        // Every other tick, as a double-buffered mirror sees it, it holds.
        for tick in (WINDOW_TICKS + 2..WINDOW_TICKS * 3).step_by(2) {
            stock -= Fx::from_int(20);
            g.update(tick, stock, CAP);
            assert_eq!(g.state, StoreState::Draining, "tick {tick}");
        }
        // Holding steady for a second ends it.
        for tick in WINDOW_TICKS * 3..WINDOW_TICKS * 5 {
            g.update(tick, stock, CAP);
        }
        assert_eq!(g.state, StoreState::Neutral);
        // The word packs fill and state.
        let w = g.word(CAP / 2, CAP);
        assert_eq!(w & STORE_FILL_MASK, 128);
        assert_eq!(w >> STORE_STATE_SHIFT & STORE_STATE_MASK, 0);
        assert_ne!(w & STORE_MARK, 0);
        // A seek back in time starts over.
        g.update(3, CAP, CAP);
        assert_eq!(g.state, StoreState::Full);
        assert_eq!(g.samples.len(), 1);
    }
}
