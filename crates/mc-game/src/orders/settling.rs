//! Orders dropped a moment ago, shown where they were dropped until the sim has
//! them there too. In a network match a command reaches the sim only after the
//! relay's input delay and a turn or two more, so how long that takes is not
//! known up front: a drop is kept until the sim moves the order, or refuses it.

use super::Key;
use mc_core::FxVec2;
use mc_sim::tables::OrderKind;

/// Past the command delay, how many ticks a drop is still waited for when the sim
/// neither moves the order nor refuses it (its holders were gone, say). Then the
/// order shows where the sim has it.
const SETTLE_TICKS: u32 = 30;

struct Drop {
    key: Key,
    to: FxVec2,
    /// The last tick it is waited for.
    until: u32,
}

#[derive(Default)]
pub(super) struct Settling {
    drops: Vec<Drop>,
}

impl Settling {
    /// `key` was dropped at `to` on `tick`; commands take `delay` ticks to reach the sim.
    pub(super) fn dropped(&mut self, key: Key, to: FxVec2, tick: u32, delay: u32) {
        self.drops.retain(|d| d.key != key);
        self.drops.push(Drop {
            key,
            to,
            until: tick + delay + SETTLE_TICKS,
        });
    }

    /// Where the order `key` names was dropped, while the sim has yet to move it.
    pub(super) fn to(&self, key: Key) -> Option<FxVec2> {
        self.drops.iter().find(|d| d.key.names(key)).map(|d| d.to)
    }

    /// Once a frame: the drops the sim has caught up with go, and any waited on too long.
    pub(super) fn keep(&mut self, tick: u32, held: impl Fn(&Key) -> bool) {
        self.drops.retain(|d| tick <= d.until && held(&d.key));
    }

    /// The sim refused to put a structure down: the plans dropped stay where they were.
    pub(super) fn build_refused(&mut self) {
        self.drops.retain(|d| d.key.kind != OrderKind::Build);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mc_data::BlueprintId;

    fn plan(x: i32) -> Key {
        Key {
            kind: OrderKind::Build,
            at: FxVec2::from_ints(x, 100),
            blueprint: BlueprintId(3),
        }
    }

    /// A network match's command comes back after its input delay and a turn or
    /// two: the drop holds until then, not snapping back to the old site meanwhile.
    #[test]
    fn a_drop_waits_out_the_command_delay() {
        let mut s = Settling::default();
        let to = FxVec2::from_ints(400, 100);
        s.dropped(plan(100), to, 50, 8);
        s.keep(50 + 8 + 4, |_| true);
        assert_eq!(s.to(plan(100)), Some(to));
        // The sim moved it: the old site is no longer held.
        s.keep(50 + 8 + 5, |_| false);
        assert_eq!(s.to(plan(100)), None);
        // Never confirmed nor refused: given up after the fallback.
        s.dropped(plan(100), to, 50, 8);
        s.keep(50 + 8 + SETTLE_TICKS + 1, |_| true);
        assert_eq!(s.to(plan(100)), None);
    }

    #[test]
    fn a_refused_build_shows_where_it_still_is() {
        let mut s = Settling::default();
        let waypoint = Key {
            kind: OrderKind::Move,
            ..plan(200)
        };
        s.dropped(plan(100), FxVec2::from_ints(400, 100), 0, 2);
        s.dropped(waypoint, FxVec2::from_ints(500, 100), 0, 2);
        s.build_refused();
        assert_eq!(s.to(plan(100)), None);
        assert!(s.to(waypoint).is_some());
    }
}
