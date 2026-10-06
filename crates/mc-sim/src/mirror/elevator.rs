//! What the interface sees of a space elevator (`crate::elevator`): where its cycle
//! stands, and what of its queue rides the next platform down.

use super::*;
use crate::elevator::{phase, Phase};
use crate::tables::OrderKind;

/// Where a space elevator's cycle stands, for its panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ElevatorPhase {
    /// The cable is coming down out of orbit and being clamped.
    Tethering,
    /// A platform is on its way down the cable.
    Inbound,
    /// The platform stands docked; what it brought is driving off.
    Docked,
    /// The platform is going back up.
    Ascending,
    /// The platform is away in orbit, loading the next payload.
    Away,
}

/// A space elevator as the interface shows it (`UnitOrders::elevator`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ElevatorView {
    pub phase: ElevatorPhase,
    /// Seconds left in this phase.
    pub left: f32,
    /// How far through this phase, zero to one.
    pub through: f32,
    /// Seconds before the next platform touches down and lets its cargo off.
    pub to_touchdown: f32,
    /// How far along the wait for that touchdown is, zero to one: from the last
    /// touchdown (or the start of the tether) to the next.
    pub approach: f32,
    /// How far through the platform's cycle (descent, dock, ascent, away), zero to one;
    /// how far the cable is down while it is tethering.
    pub cycle: f32,
    /// Seconds from one platform to the next, and the mass each brings down.
    pub period: f32,
    pub payload: f32,
    /// Seconds of the platform's descent, its stay docked and its ascent.
    pub trip: [f32; 3],
    /// The front of the queue that rides the next platform (`platform_load`): how many
    /// units, and their mass. The rest of the payload comes down as materials.
    pub riding: u16,
    pub riding_mass: f32,
}

/// What of a queue rides one platform of `payload` mass, the elevator's own rule
/// (`World::unload_platform`): units in queue order while the next still fits in what
/// is left, stopping at the first that does not. `costs` is the mass of each unit at the
/// front of the queue, in order. Returns how many ride and their mass.
pub fn platform_load(costs: impl IntoIterator<Item = Fx>, payload: Fx) -> (u16, Fx) {
    let mut left = payload;
    let mut riding: u16 = 0;
    for cost in costs {
        if cost > left {
            break;
        }
        left -= cost;
        riding = riding.saturating_add(1);
    }
    (riding, payload - left)
}

impl World {
    pub(super) fn elevator_view(&self, row: usize) -> Option<ElevatorView> {
        let e = self.elevator_spec(row)?;
        let age = self.elevator_age(row).unwrap_or(0);
        let s = &self.state;
        let tps = TICKS_PER_SECOND as f32;
        let secs = |t: u32| t as f32 / tps;
        let share = |t: u32, of: u32| (t as f32 / of.max(1) as f32).clamp(0.0, 1.0);
        let touch = e.touchdown();
        let (phase, left, through, to_touchdown, approach, cycle) = match phase(&e, age) {
            Phase::Tethering { ticks } => {
                let wait = e.tether_ticks + touch;
                (
                    ElevatorPhase::Tethering,
                    secs(e.tether_ticks - ticks),
                    share(ticks, e.tether_ticks),
                    secs(wait - ticks),
                    share(ticks, wait),
                    share(ticks, e.tether_ticks),
                )
            }
            Phase::Cycle { ticks } => {
                let to_touch = if ticks < touch {
                    touch - ticks
                } else {
                    e.period_ticks - ticks + touch
                };
                // Before the first touchdown the wait runs from the start of the tether.
                let wait = if age < e.tether_ticks + touch {
                    e.tether_ticks + touch
                } else {
                    e.period_ticks
                };
                let (docked, up) = (touch + e.dock_ticks, touch + e.dock_ticks + e.ascent_ticks);
                let (phase, from, to) = if ticks < touch {
                    (ElevatorPhase::Inbound, 0, touch)
                } else if ticks < docked {
                    (ElevatorPhase::Docked, touch, docked)
                } else if ticks < up {
                    (ElevatorPhase::Ascending, docked, up)
                } else {
                    (ElevatorPhase::Away, up, e.period_ticks)
                };
                (
                    phase,
                    secs(to - ticks),
                    share(ticks - from, to - from),
                    secs(to_touch),
                    share(wait - to_touch, wait),
                    share(ticks, e.period_ticks),
                )
            }
        };
        let costs = s
            .orders
            .iter(&s.units, row)
            .take_while(|o| o.kind == OrderKind::Produce && !self.work_paused(row))
            .map(|o| self.blueprints.unit(o.blueprint).cost_mass);
        let (riding, riding_mass) = platform_load(costs, e.payload);
        Some(ElevatorView {
            phase,
            left,
            through,
            to_touchdown,
            approach,
            cycle,
            period: secs(e.period_ticks),
            payload: e.payload.to_f32(),
            trip: [
                secs(e.descent_ticks),
                secs(e.dock_ticks),
                secs(e.ascent_ticks),
            ],
            riding,
            riding_mass: riding_mass.to_f32(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::platform_load;
    use mc_core::Fx;

    fn load(costs: &[i32], payload: i32) -> (u16, i32) {
        let (n, m) = platform_load(
            costs.iter().map(|&c| Fx::from_int(c)),
            Fx::from_int(payload),
        );
        (n, m.round_int())
    }

    /// Units ride in queue order while they fit; the first that does not fit stops the
    /// load, even when something smaller waits behind it (the sim's rule).
    #[test]
    fn the_next_platform_takes_the_front_while_it_fits() {
        assert_eq!(load(&[], 20_000), (0, 0));
        assert_eq!(load(&[5_000, 5_000, 4_400], 20_000), (3, 14_400));
        assert_eq!(load(&[12_000, 9_000, 100], 20_000), (1, 12_000));
        assert_eq!(load(&[10_000, 10_000, 1], 20_000), (2, 20_000));
        assert_eq!(load(&[25_000, 100], 20_000), (0, 0));
    }
}
