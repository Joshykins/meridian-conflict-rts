//! Dead aircraft remain ballistic debris until they hit the terrain or water.
//! One that comes down on the open sea strikes the surface (`AircraftCrashed`
//! there, for the splash), then goes under: the water stops it within a second
//! or so and it sinks nose down to the seabed, where it becomes an ordinary
//! wreck (`ShipSettled`, for the rush of air that follows it up).
//! They are absent from targeting, selection, navigation and reclaim indexes.
use crate::tables::Landing;
use crate::{SimError, SimEvent, World};
use mc_core::{Angle, Fx, FxVec3, StateHasher, TICKS_PER_SECOND};
use mc_data::BlueprintId;
use serde::{Deserialize, Serialize};

/// Water this deep or more takes a falling aircraft under; shallower, it lands as on ground.
const DITCH_DEPTH: Fx = Fx::ONE;
/// Share of its way the hull keeps when it slaps into the surface...
const ENTRY_KEEP: Fx = Fx::ratio(45, 100);
/// ...and of the way it still makes each tick under water.
const WATER_DRAG: Fx = Fx::ratio(80, 100);
/// The steady sinking pace, metres a tick (3.5 m/s), and how fast the plunge
/// eases into it (share of the difference each tick).
const SINK_SPEED: Fx = Fx::ratio(35, 100);
const SINK_EASE: Fx = Fx::ratio(30, 100);
/// The rendered yaw spin of a falling wreck (mirror.rs, 0.3 rad/s), in binary angle
/// steps a tick, so the wreck left behind faces the way the falling one last did.
const SPIN_STEPS: i32 = 313;
/// Hull radius, metres, up to which a dead aircraft falls and tumbles at the full rate.
const LIGHT_RADIUS: i32 = 20;
/// Binary angle steps in a radian (65536 / 2 pi).
const STEPS_PER_RADIAN: i32 = 10_430;
/// A light hull's tumble, binary angle steps a tick: the nose goes over at 0.75 rad/s...
const TUMBLE_PITCH: i32 = 782;
/// ...and it rolls at 2.4 rad/s.
const TUMBLE_ROLL: i32 = 2_503;
/// Most a crashed hull lies nose down in the ground, and rolled either way.
const REST_PITCH: u16 = Angle::from_degrees(30).0;
const REST_ROLL: u16 = Angle::from_degrees(20).0;

/// How hard a dead aircraft of `radius` falls and tumbles, one for anything light.
/// A capital hull (a lift ship) comes down slowly and heavily, settling rather than
/// spinning (mirror.rs scales its tumble by the same share).
pub fn heft(radius: Fx) -> Fx {
    (Fx::from_int(LIGHT_RADIUS) / radius.max(Fx::ONE)).clamp(Fx::ratio(1, 5), Fx::ONE)
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AircraftCrash {
    pub blueprint: BlueprintId,
    pub unit_id: u32,
    pub pos: FxVec3,
    pub prev_pos: FxVec3,
    /// Metres per simulation tick.
    pub velocity: FxVec3,
    pub heading: Angle,
    pub bank: i16,
    pub age: u16,
    pub mass: Fx,
    /// Its age when it went into the sea; zero while it is still in the air.
    pub splashed: u16,
    /// The seabed under it, once it is in the water.
    pub floor: Fx,
    /// Signed pitch (bow up) and roll it tumbles through as it falls, binary angle
    /// steps, this tick and last. They stop where they are when it hits the sea, and
    /// the wreck it leaves on land lies the way it came down (`rest_attitude`).
    pub pitch: i16,
    pub prev_pitch: i16,
    pub roll: i16,
    pub prev_roll: i16,
}

impl AircraftCrash {
    pub fn hash(&self, h: &mut StateHasher) {
        h.write_u64(self.blueprint.0 as u64);
        h.write_u64(self.unit_id as u64);
        for p in [self.pos, self.prev_pos, self.velocity] {
            h.write_i64(p.x.0);
            h.write_i64(p.y.0);
            h.write_i64(p.z.0);
        }
        h.write_u64(self.heading.0 as u64);
        h.write_u64(self.bank as u16 as u64);
        h.write_u64(self.age as u64 | (self.splashed as u64) << 16);
        h.write_i64(self.mass.0);
        h.write_i64(self.floor.0);
        h.write_u64(
            self.pitch as u16 as u64
                | (self.prev_pitch as u16 as u64) << 16
                | (self.roll as u16 as u64) << 32
                | (self.prev_roll as u16 as u64) << 48,
        );
    }

    /// One tick of the tumble. A light hull goes over and over; a heavy one (`heft`
    /// under one) leans into its fall and rolls a little, easing toward a limit
    /// rather than tumbling.
    fn tumble(&mut self, heft: Fx) {
        self.prev_pitch = self.pitch;
        self.prev_roll = self.roll;
        let spin = self.spin();
        if heft >= Fx::ONE {
            self.pitch = self.pitch.wrapping_sub(TUMBLE_PITCH as i16);
            self.roll = self.roll.wrapping_add((spin * TUMBLE_ROLL) as i16);
            return;
        }
        // Eased toward `most` radians at the start rate `rate * heft` rad/s, as
        // `most * (1 - exp(-t * rate * heft / most))`.
        let ease = |now: i16, toward: i32, rate: Fx, most: Fx| {
            let k = rate * heft / most / TICKS_PER_SECOND as i32;
            let step = (Fx::from_int(toward - now as i32) * k).round_int();
            (now as i32 + step).clamp(i16::MIN as i32, i16::MAX as i32) as i16
        };
        let most_pitch = Fx::ratio(15, 100) + heft * Fx::ratio(3, 2);
        let most_roll = Fx::ratio(1, 5) + heft * 2;
        let steps = |radians: Fx| (radians * STEPS_PER_RADIAN).round_int();
        self.pitch = ease(self.pitch, -steps(most_pitch), Fx::ratio(3, 4), most_pitch);
        let toward = (self.bank as i32 + spin * steps(most_roll)).clamp(-32_000, 32_000);
        self.roll = ease(self.roll, toward, Fx::ratio(12, 5), most_roll);
    }

    /// The pitch and roll the wreck it leaves on land lies at: as it came down, nose
    /// into the ground, rolled at most so far. A light hull tumbling nose up or on
    /// its back when it hits lies nose down all the same.
    fn rest_attitude(&self) -> (i16, i16) {
        let most = REST_PITCH as i32;
        let pitch = if self.pitch < 0 && (self.pitch as i32) > -(most * 2) {
            (self.pitch as i32).max(-most)
        } else {
            // Somewhere between a tenth and a quarter of the way to its limit, by the hull.
            -(most / 4 + (self.unit_id.wrapping_mul(0x9E37_79B1) >> 20) as i32 % (most / 2))
        };
        let roll = (self.roll as i32).clamp(-(REST_ROLL as i32), REST_ROLL as i32);
        (pitch as i16, roll as i16)
    }

    /// Which way the rendered wreck spins as it falls (mirror.rs).
    pub fn spin(&self) -> i32 {
        if self.unit_id & 1 == 0 {
            1
        } else {
            -1
        }
    }

    /// The heading it is drawn with after `ticks` of falling.
    fn spun_heading(&self, ticks: u16, heft: Fx) -> Angle {
        let turn = (heft * (self.spin() * SPIN_STEPS * ticks as i32)).round_int();
        Angle(self.heading.0.wrapping_add(turn as u16))
    }
}

impl World {
    pub(crate) fn run_aircraft_crashes(&mut self) -> Result<(), SimError> {
        let mut crashes = std::mem::take(&mut self.state.aircraft_crashes);
        let bounds = self.terrain.size_metres();
        let gravity = Fx::ratio(24, (TICKS_PER_SECOND * TICKS_PER_SECOND) as i64);
        let water = self.terrain.water_level();
        for mut crash in crashes.drain(..) {
            crash.prev_pos = crash.pos;
            crash.age = crash.age.saturating_add(1);
            if crash.splashed != 0 {
                if self.sink_crash(&mut crash, bounds)? {
                    self.state.aircraft_crashes.push(crash);
                }
                continue;
            }
            crash.velocity.x *= Fx::ratio(98, 100);
            crash.velocity.y *= Fx::ratio(98, 100);
            let heft = heft(self.blueprints.unit(crash.blueprint).radius);
            crash.tumble(heft);
            crash.velocity.z -= gravity * heft;
            crash.pos += crash.velocity;
            crash.pos.x = crash.pos.x.clamp(Fx::ZERO, bounds.x);
            crash.pos.y = crash.pos.y.clamp(Fx::ZERO, bounds.y);
            let ground = self.terrain.height_at(crash.pos.xy());
            let surface = ground.max(water);
            if crash.pos.z > surface {
                self.state.aircraft_crashes.push(crash);
                continue;
            }
            crash.pos.z = surface;
            self.events.push(SimEvent::AircraftCrashed {
                pos: crash.pos,
                blueprint: crash.blueprint,
            });
            if ground <= water - DITCH_DEPTH {
                // Into the sea: the surface takes most of its way off it, then it goes under.
                crash.splashed = crash.age;
                crash.floor = ground;
                crash.velocity.x *= ENTRY_KEEP;
                crash.velocity.y *= ENTRY_KEEP;
                crash.velocity.z *= ENTRY_KEEP;
                self.state.aircraft_crashes.push(crash);
                continue;
            }
            if crash.mass > Fx::ZERO {
                let row = self.state.wrecks.spawn(
                    crash.blueprint,
                    crash.pos.xy(),
                    surface,
                    crash.spun_heading(crash.age, heft),
                    crash.mass,
                    self.state.tick,
                )?;
                let (pitch, roll) = crash.rest_attitude();
                let wrecks = &mut self.state.wrecks;
                wrecks.pitch[row] = pitch;
                wrecks.bank[row] = roll;
                wrecks.prev_bank[row] = roll;
                wrecks.landing[row] = Landing::Crashed as u8;
            }
            let radius = self.blueprints.unit(crash.blueprint).radius;
            self.add_stain(crash.pos.xy(), radius * Fx::ratio(3, 2), 72);
        }
        Ok(())
    }

    /// One tick of a dead aircraft going down through the water. False once it
    /// is on the seabed and has become a wreck.
    fn sink_crash(
        &mut self,
        crash: &mut AircraftCrash,
        bounds: mc_core::FxVec2,
    ) -> Result<bool, SimError> {
        crash.prev_pitch = crash.pitch;
        crash.prev_roll = crash.roll;
        crash.velocity.x *= WATER_DRAG;
        crash.velocity.y *= WATER_DRAG;
        crash.velocity.z += (-SINK_SPEED - crash.velocity.z) * SINK_EASE;
        crash.pos += crash.velocity;
        crash.pos.x = crash.pos.x.clamp(Fx::ZERO, bounds.x);
        crash.pos.y = crash.pos.y.clamp(Fx::ZERO, bounds.y);
        crash.floor = self.terrain.height_at(crash.pos.xy());
        if crash.pos.z > crash.floor {
            return Ok(true);
        }
        crash.pos.z = crash.floor;
        if crash.mass > Fx::ZERO {
            // It stopped spinning when it hit the water, and lies level on the bottom.
            let row = self.state.wrecks.spawn(
                crash.blueprint,
                crash.pos.xy(),
                crash.floor,
                crash.spun_heading(
                    crash.splashed,
                    heft(self.blueprints.unit(crash.blueprint).radius),
                ),
                crash.mass,
                self.state.tick,
            )?;
            self.state.wrecks.landing[row] = Landing::Ditched as u8;
        }
        self.events.push(SimEvent::ShipSettled {
            pos: crash.pos,
            blueprint: crash.blueprint,
        });
        Ok(false)
    }
}
