//! Dead ships go down slowly. The hull settles for a moment while it takes on a
//! list and goes down by the bow or the stern, then sinks faster and faster to
//! the seabed, where it becomes an ordinary wreck. On the way down it is absent
//! from targeting, selection, navigation and reclaim indexes, like a falling aircraft.
use crate::{SimError, SimEvent, World};
use mc_core::{Angle, Fx, FxVec2, StateHasher};
use mc_data::BlueprintId;
use serde::{Deserialize, Serialize};

/// Ticks the hull barely moves while it heels over: a second and a half.
const SETTLE_TICKS: u16 = 15;
/// Metres a tick it settles by meanwhile.
const SETTLE_SPEED: Fx = Fx::ratio(15, 1000);
/// How fast the sinking gathers pace, metres per tick per tick (0.3 m/s²)...
const SINK_ACCEL: Fx = Fx::ratio(3, 1000);
/// ...up to this many metres a tick (2.5 m/s).
const SINK_TOP: Fx = Fx::ratio(1, 4);
/// Share of its height the hull rests above the seabed: the keel is sunk into it.
const REST: Fx = Fx::ratio(3, 10);
/// Share of the full list and trim the hull has once it has settled.
const SETTLED_SHARE: Fx = Fx::ratio(3, 5);
/// A hull taller than this (metres) is a big ship: it trims less, and sinks faster.
const BIG_HULL: i32 = 12;
/// Most a big ship goes down by the head or the stern: six degrees.
const BIG_TRIM: u16 = Angle::from_degrees(6).0;
/// The frigate's height, metres: sinking paces are for it, and a taller hull's are
/// scaled up by its height over this, never down.
const PACE_HEIGHT: i32 = 10;

fn default_pace() -> Fx {
    Fx::ONE
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SinkingHull {
    pub blueprint: BlueprintId,
    pub unit_id: u32,
    pub pos: FxVec2,
    pub z: Fx,
    pub prev_z: Fx,
    pub heading: Angle,
    /// Signed roll, binary angle steps; `list` is where it ends up.
    pub roll: i16,
    pub prev_roll: i16,
    /// Signed pitch, binary angle steps, positive raises the bow. `trim` is the most
    /// it reaches; the hull comes level again as it lands on the bottom.
    pub pitch: i16,
    pub prev_pitch: i16,
    pub list: i16,
    pub trim: i16,
    /// Metres per tick, downward.
    pub sink: Fx,
    /// Waterline height it died at, and the one it comes to rest at.
    pub top: Fx,
    pub floor: Fx,
    pub age: u16,
    pub mass: Fx,
    /// How much faster than a frigate it gathers pace and sinks: its height over the
    /// frigate's, at least one.
    #[serde(default = "default_pace")]
    pub pace: Fx,
}

impl SinkingHull {
    /// A hull that has just died at `z`. How it heels over comes from its unit id,
    /// so every machine sinks it the same way.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        blueprint: BlueprintId,
        unit_id: u32,
        pos: FxVec2,
        z: Fx,
        heading: Angle,
        seabed: Fx,
        height: Fx,
        mass: Fx,
    ) -> SinkingHull {
        let seed = unit_id.wrapping_mul(0x9E37_79B1);
        let side = if seed & 1 == 0 { 1 } else { -1 };
        let bow = if seed & 2 == 0 { -1 } else { 1 };
        // A 16 to 24 degree list, and 8 to 14 degrees down by the bow or the stern.
        let list = Angle::from_degrees(16 + (seed >> 8) as i32 % 9).0 as i32 * side;
        let trim = Angle::from_degrees(8 + (seed >> 16) as i32 % 7).0 as i32 * bow;
        // A long, tall hull cannot stand on its end in the water: it trims by less.
        let trim = if height > Fx::from_int(BIG_HULL) {
            let scaled = (Fx::from_int(trim) * BIG_HULL / height).floor_int();
            scaled.clamp(-(BIG_TRIM as i32), BIG_TRIM as i32)
        } else {
            trim
        };
        let pace = (height / PACE_HEIGHT).max(Fx::ONE);
        let floor = (seabed + height * REST).min(z);
        SinkingHull {
            blueprint,
            unit_id,
            pos,
            z,
            prev_z: z,
            heading,
            roll: 0,
            prev_roll: 0,
            pitch: 0,
            prev_pitch: 0,
            list: list as i16,
            trim: trim as i16,
            sink: Fx::ZERO,
            top: z,
            floor,
            age: 0,
            mass,
            pace,
        }
    }

    /// How far down it is: 0 at the surface, 1 on the bottom.
    pub fn progress(&self) -> Fx {
        let span = self.top - self.floor;
        if span <= Fx::ZERO {
            return Fx::ONE;
        }
        ((self.top - self.z) / span).clamp(Fx::ZERO, Fx::ONE)
    }

    pub fn hash(&self, h: &mut StateHasher) {
        h.write_u64(self.blueprint.0 as u64 | (self.unit_id as u64) << 32);
        for v in [
            self.pos.x,
            self.pos.y,
            self.z,
            self.prev_z,
            self.sink,
            self.top,
            self.floor,
            self.mass,
            self.pace,
        ] {
            h.write_i64(v.0);
        }
        h.write_u64(
            self.heading.0 as u64
                | (self.roll as u16 as u64) << 16
                | (self.pitch as u16 as u64) << 32
                | (self.age as u64) << 48,
        );
        h.write_u64(self.list as u16 as u64 | (self.trim as u16 as u64) << 16);
    }

    /// One tick of going down. True once it is on the bottom.
    fn step(&mut self) -> bool {
        self.prev_z = self.z;
        self.prev_roll = self.roll;
        self.prev_pitch = self.pitch;
        self.age = self.age.saturating_add(1);
        if self.age <= SETTLE_TICKS {
            self.z = (self.z - SETTLE_SPEED).max(self.floor);
            let u = smooth(Fx::ratio(self.age as i64, SETTLE_TICKS as i64));
            self.roll = share(self.list, SETTLED_SHARE * u);
            self.pitch = share(self.trim, SETTLED_SHARE * u);
        } else {
            self.sink = (self.sink + SINK_ACCEL * self.pace).min(SINK_TOP * self.pace);
            self.z = (self.z - self.sink).max(self.floor);
            let p = self.progress();
            self.roll = share(self.list, SETTLED_SHARE + (Fx::ONE - SETTLED_SHARE) * p);
            // Down by the head (or the stern) while it goes under, level again on the bottom.
            let steepen = (p * 3).min(Fx::ONE);
            let level = smooth(((p - Fx::ratio(3, 4)) * 4).clamp(Fx::ZERO, Fx::ONE));
            self.pitch = share(
                self.trim,
                (SETTLED_SHARE + (Fx::ONE - SETTLED_SHARE) * steepen) * (Fx::ONE - level),
            );
        }
        self.z <= self.floor
    }
}

fn smooth(u: Fx) -> Fx {
    u * u * (Fx::from_int(3) - u * 2)
}

fn share(angle: i16, part: Fx) -> i16 {
    (Fx::from_int(angle as i32) * part).floor_int() as i16
}

impl World {
    pub(crate) fn run_sinking(&mut self) -> Result<(), SimError> {
        let mut i = 0;
        while i < self.state.sinking.len() {
            if !self.state.sinking[i].step() {
                i += 1;
                continue;
            }
            let hull = self.state.sinking.remove(i);
            if hull.mass > Fx::ZERO {
                let row = self.state.wrecks.spawn(
                    hull.blueprint,
                    hull.pos,
                    hull.floor,
                    hull.heading,
                    hull.mass,
                )?;
                self.state.wrecks.bank[row] = hull.roll;
                self.state.wrecks.prev_bank[row] = hull.roll;
            }
            self.events.push(SimEvent::ShipSettled {
                pos: hull.pos.extend(hull.floor),
                blueprint: hull.blueprint,
            });
        }
        Ok(())
    }
}
