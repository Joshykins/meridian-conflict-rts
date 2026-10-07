//! What the interface sees of a space elevator (`crate::elevator`): where its cycle
//! stands, and what of its queue rides the next crates down.

use super::*;
use crate::elevator::{phase, Phase};
use crate::tables::OrderKind;

/// Where a space elevator's cycle stands, for its panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ElevatorPhase {
    /// The cable is coming down out of orbit and being clamped.
    Tethering,
    /// The crates are on their way down the cable.
    Inbound,
    /// The crates stand landed, open; what they brought is driving out.
    Docked,
    /// The empty crates are sinking into the ground.
    Sinking,
    /// The crates are away in orbit, loading the next payload.
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
    /// Seconds before the next crates touch down and lets its cargo off.
    pub to_touchdown: f32,
    /// How far along the wait for that touchdown is, zero to one: from the last
    /// touchdown (or the start of the tether) to the next.
    pub approach: f32,
    /// How far through the crates' cycle (descent, dock, sink, away), zero to one;
    /// how far the cable is down while it is tethering.
    pub cycle: f32,
    /// Seconds from one fall to the next, and the mass each brings down.
    pub period: f32,
    pub payload: f32,
    /// Seconds of the crates' descent, their stay landed and their sink.
    pub trip: [f32; 3],
    /// The front of the queue that rides the next crates (`crate_load`): how many
    /// units, and their mass. The rest of the payload comes down as materials.
    pub riding: u16,
    pub riding_mass: f32,
}

/// What of a queue rides one fall of crates of `payload` mass, the elevator's own rule
/// (`elevator::seat_units`): units in queue order while the next still fits in what is
/// left and some crate has a seat for it, stopping at the first that does not. `queue` is
/// the radius and mass of each unit at the front of the queue, in order. Returns how many
/// ride and their mass.
pub fn crate_load(queue: impl IntoIterator<Item = (Fx, Fx)>, payload: Fx) -> (u16, Fx) {
    let queue: Vec<(Fx, Fx)> = queue.into_iter().collect();
    let seats = crate::elevator::seat_units(queue.iter().copied(), payload);
    let mass = queue
        .iter()
        .take(seats.len())
        .fold(Fx::ZERO, |m, &(_, cost)| m + cost);
    (u16::try_from(seats.len()).unwrap_or(u16::MAX), mass)
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
                let (docked, up) = (touch + e.dock_ticks, touch + e.dock_ticks + e.sink_ticks);
                let (phase, from, to) = if ticks < touch {
                    (ElevatorPhase::Inbound, 0, touch)
                } else if ticks < docked {
                    (ElevatorPhase::Docked, touch, docked)
                } else if ticks < up {
                    (ElevatorPhase::Sinking, docked, up)
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
            .map(|o| {
                let bp = self.blueprints.unit(o.blueprint);
                (bp.radius, bp.cost_mass)
            });
        let (riding, riding_mass) = crate_load(costs, e.payload);
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
                secs(e.sink_ticks),
            ],
            riding,
            riding_mass: riding_mass.to_f32(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::crate_load;
    use mc_core::Fx;

    fn load(units: &[(i32, i32)], payload: i32) -> (u16, i32) {
        let (n, m) = crate_load(
            units
                .iter()
                .map(|&(r, c)| (Fx::from_int(r), Fx::from_int(c))),
            Fx::from_int(payload),
        );
        (n, m.round_int())
    }

    /// Units ride in queue order while they fit; the first that does not fit stops the
    /// load, even when something smaller waits behind it (the sim's rule).
    #[test]
    fn the_next_crates_take_the_front_while_it_fits() {
        assert_eq!(load(&[], 20_000), (0, 0));
        assert_eq!(
            load(&[(8, 5_000), (8, 5_000), (8, 4_400)], 20_000),
            (3, 14_400)
        );
        assert_eq!(
            load(&[(8, 12_000), (8, 9_000), (8, 100)], 20_000),
            (1, 12_000)
        );
        assert_eq!(
            load(&[(8, 10_000), (8, 10_000), (8, 1)], 20_000),
            (2, 20_000)
        );
        assert_eq!(load(&[(8, 25_000), (8, 100)], 20_000), (0, 0));
    }

    /// The crates' seats run out before the payload does: scouts at 100 mass.
    #[test]
    fn the_crates_hold_only_so_many_units() {
        let scouts = vec![(4, 100); 400];
        let (n, _) = load(&scouts, 20_000);
        assert!(n > 30 && n < 200, "{n} scouts fit in three crates");
        // Three big walkers fill a crate each.
        let (n, _) = load(
            &[(31, 1_000), (31, 1_000), (31, 1_000), (31, 1_000)],
            20_000,
        );
        assert!(n >= 3, "a walker's crate holds {n}");
    }
}
