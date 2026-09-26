//! Giant walkers (`Motion::stride`, the Behemoth): they stride straight over structures,
//! steep ground and shallow water instead of pathing round them, and every footfall
//! crushes what is under the sole (`Stomp`).
//!
//! Its guns leave more behind than their hits: a giant bore's charge spreads out from
//! where it struck as a lightning storm that grows for seconds and vaporises what is in
//! it (`DischargeStorm`), and a giant rail gun throws out a spent sabot with every shot
//! that bursts where it lands and lies there as scrap (`FallingSabot`).

use mc_core::{Angle, Fx, FxVec2, FxVec3, StateHasher};
use mc_data::{cat, BlueprintId, MoveLayer, Storm};
use serde::{Deserialize, Serialize};

use crate::spatial::kind;
use crate::tables::{flag, UnitId};
use crate::world::{State, World};
use crate::{SimError, SimEvent};

/// Per-tick share of a sabot's speed its drag leaves, e^(-1.3 x 0.1): the same flight
/// the renderer's casings fly (puffs.wgsl `casing_flight`), so the one drawn lands where
/// and when the sim's does.
const SABOT_DECAY: Fx = Fx::ratio(878_095, 1_000_000);
const SABOT_DRAG: Fx = Fx::ratio(13, 10);
/// puffs.wgsl `CASING_FALL`: z falls by this times t squared.
const SABOT_FALL: Fx = Fx::from_int(10);
/// Where the Tempest's port is off its muzzle (the top barrel's), at the Behemoth's built
/// size: out to the right, and down (the model's `titan::EJECT`, 4x).
const EJECT_OUT: Fx = Fx::ratio(344, 10);
const EJECT_DOWN: Fx = Fx::ratio(156, 10);
/// A sabot landing this near a heap of its kind adds to the heap.
const SABOT_HEAP: Fx = Fx::from_int(18);
/// Leave this many wreck slots for everything else.
const SABOT_WRECK_SPARE: usize = 512;

/// A giant bore's lightning storm, spreading from where it struck.
#[derive(Clone, Serialize, Deserialize)]
pub struct DischargeStorm {
    pub pos: FxVec2,
    pub z: Fx,
    pub radius: Fx,
    pub ticks: u16,
    pub age: u16,
    /// Per second at its heart.
    pub damage: Fx,
    pub owner: u8,
    pub source: UnitId,
    pub mask: u32,
}

impl DischargeStorm {
    pub(crate) fn struck(at: FxVec3, storm: Storm, owner: u8, source: UnitId, mask: u32) -> Self {
        DischargeStorm {
            pos: at.xy(),
            z: at.z,
            radius: storm.radius,
            ticks: storm.ticks.max(1),
            age: 0,
            damage: storm.damage,
            owner,
            source,
            mask,
        }
    }

    /// How far it has spread by now: a fifth of its reach at once, the rest over its
    /// life, quickly at first.
    pub fn reach(&self) -> Fx {
        let f = Fx::from_int(self.age as i32) / Fx::from_int(self.ticks as i32);
        self.radius * (Fx::ratio(1, 5) + Fx::ratio(4, 5) * (f * 2 - f * f))
    }
}

/// A spent sabot in the air.
#[derive(Clone, Serialize, Deserialize)]
pub struct FallingSabot {
    pub from: FxVec3,
    pub vel: FxVec3,
    pub age: u16,
    /// SABOT_DECAY to the power of `age`.
    pub decay: Fx,
    pub owner: u8,
    pub source: UnitId,
    pub blueprint: BlueprintId,
    pub weapon: u8,
    /// Its own tumble and where it lands: every throw differs (`sabot_throw`).
    #[serde(default)]
    pub seed: u32,
}

impl FallingSabot {
    /// Thrown from `back` metres behind a muzzle at `muzzle` of a shot flying `shot` (per
    /// tick): out to the right of the barrel, a little up and back.
    pub(crate) fn thrown(
        muzzle: FxVec3,
        shot: FxVec3,
        back: Fx,
        owner: u8,
        source: UnitId,
        blueprint: BlueprintId,
        weapon: u8,
        seed: u32,
    ) -> Self {
        let (from, vel) = sabot_throw(muzzle, shot, back, seed);
        FallingSabot {
            from,
            vel,
            age: 0,
            decay: Fx::ONE,
            owner,
            source,
            blueprint,
            weapon,
            seed,
        }
    }

    /// Where it is now.
    pub fn at(&self) -> FxVec3 {
        self.at_age(self.age, self.decay)
    }

    /// Where it was a tick ago.
    pub fn before(&self) -> FxVec3 {
        let decay = if self.age == 0 {
            Fx::ONE
        } else {
            self.decay / SABOT_DECAY
        };
        self.at_age(self.age.saturating_sub(1), decay)
    }

    /// Its tumble after `age` ticks: yaw, pitch and roll in radians. Every case leaves the
    /// port the same way, lying along the barrel; only in the air does each one start to
    /// turn end over end and roll, at its own rate, the spin gathering over its first second.
    #[expect(
        clippy::float_arithmetic,
        clippy::disallowed_types,
        clippy::disallowed_methods,
        reason = "presentation: the render mirror spins the falling sabot by it; the wreck's heading uses `landed_yaw`"
    )]
    pub fn tumble(&self, age: u16) -> (f32, f32, f32) {
        let r = |k: u32| {
            (mix32(self.seed.wrapping_mul(0x9E37_79B1).wrapping_add(k)) % 10_000) as f32 / 10_000.0
        };
        let t = age as f32 / 10.0;
        let spun = t * t / (t + 1.0);
        let (vx, vy) = (self.vel.x.to_f32(), self.vel.y.to_f32());
        // The barrel's heading: the throw goes out to its right.
        let aim = vx.atan2(-vy);
        let yaw = aim + (r(2) - 0.5) * 1.6 * spun;
        let pitch = (1.4 + r(3) * 2.2) * spun * if r(4) < 0.5 { 1.0 } else { -1.0 };
        let roll = (r(5) - 0.5) * 5.0 * spun;
        (yaw, pitch, roll)
    }

    /// The yaw of `tumble` at the moment it lands, in fixed point: the wreck's heading is
    /// sim state, so it may not come from floats (`atan2` differs between platforms).
    fn landed_yaw(&self) -> Angle {
        let r2 = Fx::ratio(
            (mix32(self.seed.wrapping_mul(0x9E37_79B1).wrapping_add(2)) % 10_000) as i64,
            10_000,
        );
        let t = Fx::from_int(self.age as i32) / 10;
        let spun = t * t / (t + Fx::ONE);
        let aim = Angle::atan2(self.vel.x, -self.vel.y);
        // (r - 0.5) * 1.6 * spun radians, in binary angle steps (65536 per 2 pi).
        let steps = ((r2 - Fx::HALF) * spun)
            .mul_div(16 * 65_536 * 100_000, 10 * 628_318)
            .round_int();
        Angle(aim.0.wrapping_add(steps.rem_euclid(65_536) as u16))
    }

    fn at_age(&self, age: u16, decay: Fx) -> FxVec3 {
        let t = Fx::from_int(age as i32) / 10;
        let mut p = self.from + self.vel * ((Fx::ONE - decay) / SABOT_DRAG);
        p.z -= SABOT_FALL * t * t;
        p + self.drift() * (t * t)
    }

    /// How each case wanders off the common throw as it tumbles (m/s², half of it, so the
    /// offset is this times t squared): nothing at the port, then more every moment, a
    /// little up or down, well behind or a little ahead, out or back in, by its seed.
    fn drift(&self) -> FxVec3 {
        let r = |k: u32| {
            Fx::ratio(
                (mix32(self.seed.wrapping_mul(0x27D4_EB2F).wrapping_add(k)) % 1000) as i64,
                1000,
            )
        };
        let flat = FxVec2::new(self.vel.x, self.vel.y);
        let right = flat * (Fx::ONE / flat.length().max(Fx::EPSILON));
        let ahead = FxVec2::new(-right.y, right.x);
        let along = r(1) * 8 - Fx::from_int(5);
        let out = r(2) * 6 - Fx::from_int(3);
        let rise = r(3) * 3 - Fx::ratio(3, 2);
        (ahead * along + right * out).extend(rise)
    }
}

/// Where a sabot leaves the gun and how fast (m/s), for a muzzle at `muzzle`, a shot
/// flying `shot` and the ejector `back` metres behind the muzzle: kicked out to the right
/// of the barrel and up, every case the same (the seed is kept for the call's shape; the
/// cases part in the air).
pub fn sabot_throw(muzzle: FxVec3, shot: FxVec3, back: Fx, _seed: u32) -> (FxVec3, FxVec3) {
    let len = shot.length().max(Fx::EPSILON);
    let dir = shot * (Fx::ONE / len);
    let flat = FxVec2::new(dir.y, -dir.x);
    let right = flat * (Fx::ONE / flat.length().max(Fx::EPSILON));
    let right = right.extend(Fx::ZERO);
    // Out of the port in the flank of the gun's body (the model's `titan::EJECT`), thrown
    // out and up at about 34 degrees, every case alike: they part only in the air
    // (`FallingSabot::drift`, `tumble`).
    let up = FxVec3::new(Fx::ZERO, Fx::ZERO, Fx::ONE);
    let from = muzzle - dir * back + right * EJECT_OUT - up * EJECT_DOWN;
    let chute = right * Fx::ratio(83, 100) + up * Fx::ratio(56, 100);
    let vel = chute * Fx::from_int(44);
    (from, vel)
}

/// A well-mixed 32-bit hash.
fn mix32(v: u32) -> u32 {
    let mut x = v ^ (v >> 16);
    x = x.wrapping_mul(0x7FEB_352D);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846C_A68B);
    x ^ (x >> 16)
}

pub(crate) fn hash_giants(s: &State, h: &mut StateHasher) {
    h.write_u64(s.storms.len() as u64);
    for st in &s.storms {
        for v in [st.pos.x, st.pos.y, st.z, st.radius, st.damage] {
            h.write_i64(v.0);
        }
        h.write_u64(st.ticks as u64 | (st.age as u64) << 16 | (st.owner as u64) << 32);
        h.write_u64(st.mask as u64 | (st.source.0 as u64) << 32);
    }
    h.write_u64(s.sabots.len() as u64);
    for sb in &s.sabots {
        for v in [
            sb.from.x, sb.from.y, sb.from.z, sb.vel.x, sb.vel.y, sb.vel.z, sb.decay,
        ] {
            h.write_i64(v.0);
        }
        h.write_u64(
            sb.age as u64
                | (sb.owner as u64) << 16
                | (sb.weapon as u64) << 24
                | (sb.blueprint.0 as u64) << 32,
        );
        h.write_u32(sb.seed);
        h.write_u32(sb.source.0);
    }
}

/// How deep a strider wades: it walks the bottom where the water over it is shallower
/// than this, and stops at the edge of anything deeper. A strider stands hundreds of
/// metres tall, and the deepest sea on the maps is about 80 m: it wades every sea there
/// is with the water still below its knees, its guns far above it.
pub const STRIDE_WADE: Fx = Fx::from_int(160);

impl World {
    /// Where a striding walker can put its feet: on the map, on land or under water no
    /// deeper than it wades. Slopes and structures do not stop it.
    pub(crate) fn stride_footing(&self, pos: FxVec2) -> bool {
        self.terrain.in_bounds(pos)
            && self.terrain.height_at(pos) >= self.terrain.water_level() - STRIDE_WADE
    }

    /// Whether one of `a` and `b` is a striding walker: it steps over the other, and the
    /// two never shove each other. What it steps on is another matter (`run_stomps`).
    pub(crate) fn steps_over(&self, a: usize, b: usize) -> bool {
        let strides = |r: usize| self.bp(r).motion.is_some_and(|m| m.stride);
        strides(a) || strides(b)
    }

    /// Footfalls of the striding walkers that moved this tick: `before` is each one's
    /// ground counter (`gait`) before the move. A foot comes down each time the counter
    /// passes another `Stomp::pace`, and every enemy ground unit under the sole takes the
    /// stomp. Structures are stepped over.
    pub(crate) fn run_stomps(&mut self, before: &[(usize, u32)]) {
        let mut blows = Vec::new();
        for &(row, was) in before {
            let Some(stomp) = self.bp(row).stomp else {
                continue;
            };
            let units = &self.state.units;
            let pace = (stomp.pace * 256).floor_int().max(1) as u32;
            let now = units.gait[row];
            // Steps landed this tick (wrapping counter), at most two.
            let (from, to) = (was / pace, now / pace);
            if from == to || now.wrapping_sub(was) > pace * 2 {
                continue;
            }
            let forward = FxVec2::from_angle(units.heading[row]);
            let left = FxVec2::new(-forward.y, forward.x);
            let side = if to % 2 == 0 { Fx::ONE } else { -Fx::ONE };
            let at = units.pos[row] + forward * stomp.reach + left * (stomp.gauge * side);
            blows.push((row, at, stomp.radius, stomp.damage));
        }
        for (row, at, radius, damage) in blows {
            let owner = self.state.units.owner[row];
            let source = self.state.units.id(row);
            let mut crushed = Vec::new();
            self.index.query(
                at,
                radius + Fx::from_int(12),
                crate::spatial::kind::UNIT,
                |e| {
                    let other = e.row as usize;
                    if other != row
                        && self.unit_entry_is_current(e)
                        && e.pos.distance(at) <= radius + e.radius / 2
                    {
                        crushed.push(other);
                    }
                    true
                },
            );
            for other in crushed {
                let bp = self.bp(other);
                let grounded = bp.motion.is_some_and(|m| m.layer != MoveLayer::Air)
                    && !bp.is_structure()
                    && bp.categories & cat::EXPERIMENTAL == 0;
                if !grounded
                    || !self.are_enemies(owner, self.state.units.owner[other])
                    || self.state.units.has_flag(other, flag::IN_FACTORY)
                    || self.submerged(other)
                {
                    continue;
                }
                if self.shield_blocking(other) && bp.shield.is_some_and(|s| s.is_hull()) {
                    self.damage_shield(other, damage);
                } else {
                    self.damage_unit(other, damage, owner, source);
                }
            }
        }
    }
}

impl World {
    /// This tick's storms and falling sabots.
    pub(crate) fn run_giants(&mut self) -> Result<(), SimError> {
        if !self.state.storms.is_empty() {
            self.run_storms()?;
        }
        if !self.state.sabots.is_empty() {
            self.run_sabots()?;
        }
        Ok(())
    }

    /// The live storm `row`'s bore is feeding, if any.
    pub(crate) fn storm_of(&self, row: usize) -> Option<&DischargeStorm> {
        let id = self.state.units.id(row);
        self.state.storms.iter().find(|s| s.source == id)
    }

    fn run_storms(&mut self) -> Result<(), SimError> {
        let storms = std::mem::take(&mut self.state.storms);
        let mut left = Vec::with_capacity(storms.len());
        for mut storm in storms {
            // The gun is what discharges into it: with the machine gone, the storm dies.
            if self.state.units.row(storm.source).is_none() {
                self.events.push(SimEvent::StormCollapsed {
                    pos: storm.pos.extend(storm.z),
                });
                continue;
            }
            let reach = storm.reach();
            let point = storm.pos.extend(storm.z + Fx::from_int(8));
            // Full over its inner half, falling to two fifths at its edge.
            let per_tick = storm.damage / Fx::from_int(mc_core::TICKS_PER_SECOND as i32);
            let mut struck = Vec::new();
            self.index
                .query(storm.pos, reach + Fx::from_int(40), kind::UNIT, |e| {
                    let r = e.row as usize;
                    if self.unit_entry_is_current(e)
                        && self.state.units.is_active(r)
                        && self.are_enemies(storm.owner, self.state.units.owner[r])
                        && self.hittable(r, storm.mask)
                    {
                        let d = self.state.units.pos[r].distance(storm.pos) - self.bp(r).radius;
                        if d <= reach {
                            struck.push((r, d.max(Fx::ZERO)));
                        }
                    }
                    true
                });
            let mut charged = Vec::new();
            for (r, d) in struck {
                let edge = ((d * 2 - reach) / reach.max(Fx::ONE)).clamp(Fx::ZERO, Fx::ONE);
                let damage = per_tick * (Fx::ONE - edge * Fx::ratio(3, 5));
                let bp = self.bp(r);
                let target = self.state.units.pos[r].extend(self.state.units.z[r] + bp.height / 2);
                match self.blast_blocker(point, target, Some(r)) {
                    Some(shield) => {
                        if !charged.contains(&shield) {
                            self.damage_shield(shield, damage);
                            charged.push(shield);
                        }
                    }
                    None => self.damage_unit(r, damage, storm.owner, storm.source),
                }
            }
            // It vaporises the woods as it spreads, and scorches the ground.
            if storm.age % 3 == 0 {
                let mut trees = Vec::new();
                self.prop_index.query(storm.pos, reach, kind::PROP, |e| {
                    let prop = e.row as usize;
                    if e.pos.distance(storm.pos) <= reach
                        && self.map.props[prop].kind.is_tree()
                        && self.is_prop_alive(prop)
                    {
                        trees.push(prop);
                    }
                    true
                });
                for prop in trees {
                    self.state.props_dead[prop / 64] |= 1 << (prop % 64);
                    self.events.push(SimEvent::TreeVaporized {
                        prop: prop as u32,
                        center: storm.pos,
                    });
                }
            }
            if storm.age % 10 == 0 {
                self.add_stain(storm.pos, reach, 160)?;
            }
            storm.age += 1;
            if storm.age < storm.ticks {
                left.push(storm);
            }
        }
        left.append(&mut self.state.storms);
        self.state.storms = left;
        Ok(())
    }

    fn run_sabots(&mut self) -> Result<(), SimError> {
        let sabots = std::mem::take(&mut self.state.sabots);
        let mut flying = Vec::with_capacity(sabots.len());
        let water = self.terrain.water_level();
        for mut sabot in sabots {
            sabot.age += 1;
            sabot.decay *= SABOT_DECAY;
            let at = sabot.at();
            let size = self.terrain.size_metres();
            let off_map = at.x < Fx::ZERO || at.y < Fx::ZERO || at.x > size.x || at.y > size.y;
            let ground = if off_map {
                Fx::ZERO
            } else {
                self.terrain.height_at(at.xy())
            };
            if !off_map && at.z > ground.max(water) && sabot.age < 600 {
                flying.push(sabot);
                continue;
            }
            if off_map {
                continue;
            }
            self.sabot_lands(&sabot, at.xy().extend(ground.max(water)), ground < water)?;
        }
        flying.append(&mut self.state.sabots);
        self.state.sabots = flying;
        Ok(())
    }

    /// A sabot comes down at `at`: it bursts, and (on dry ground) its scrap lies there.
    fn sabot_lands(
        &mut self,
        sabot: &FallingSabot,
        at: FxVec3,
        in_water: bool,
    ) -> Result<(), SimError> {
        let Some(spec) = self
            .blueprints
            .unit(sabot.blueprint)
            .weapons
            .get(sabot.weapon as usize)
            .and_then(|w| w.sabot)
        else {
            return Ok(());
        };
        self.events.push(SimEvent::SabotLanded {
            pos: at,
            blueprint: sabot.blueprint,
            weapon: sabot.weapon,
        });
        let mut struck = Vec::new();
        self.index
            .query(at.xy(), spec.splash + Fx::from_int(40), kind::UNIT, |e| {
                let r = e.row as usize;
                if self.unit_entry_is_current(e)
                    && self.state.units.is_active(r)
                    && self.are_enemies(sabot.owner, self.state.units.owner[r])
                    && self.hittable(r, cat::LAND | cat::NAVAL | cat::STRUCTURE)
                    && self.state.units.pos[r].distance(at.xy()) <= spec.splash + self.bp(r).radius
                {
                    struck.push(r);
                }
                true
            });
        for r in struck {
            let bp = self.bp(r);
            let target = self.state.units.pos[r].extend(self.state.units.z[r] + bp.height / 2);
            match self.blast_blocker(
                at + FxVec3::new(Fx::ZERO, Fx::ZERO, Fx::ONE),
                target,
                Some(r),
            ) {
                Some(shield) => self.damage_shield(shield, spec.damage),
                None => self.damage_unit(r, spec.damage, sabot.owner, sabot.source),
            }
        }
        if in_water {
            return Ok(());
        }
        self.add_stain(at.xy(), spec.splash / 2, 90)?;
        // Onto the heap of its kind where one lies close, or a heap of its own.
        let wrecks = &mut self.state.wrecks;
        let heap = wrecks.slots.iter().find(|&w| {
            wrecks.blueprint[w] == spec.wreck && wrecks.pos[w].distance(at.xy()) <= SABOT_HEAP
        });
        match heap {
            Some(w) => {
                wrecks.mass[w] += spec.mass;
                wrecks.mass_max[w] += spec.mass;
            }
            None if wrecks.slots.live() + SABOT_WRECK_SPARE < crate::tables::MAX_WRECKS => {
                // It lies the way it was turned as it came down.
                let heading = sabot.landed_yaw();
                let row = wrecks.spawn(spec.wreck, at.xy(), at.z, heading, spec.mass)?;
                wrecks.mass_max[row] = spec.mass;
            }
            // Deliberate: casings never take the last SABOT_WRECK_SPARE wreck slots, which
            // are kept for the wrecks of units. The casing's mass is lost with it.
            None => {}
        }
        Ok(())
    }
}
