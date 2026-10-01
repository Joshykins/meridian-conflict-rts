//! Wrecks in the render frame: settled salvage, hulls still falling out of the sky and
//! hulls going down through the water.
//!
//! How a settled wreck lies is worked out from its model's size and domain and from how
//! it came down ([`Landing`]), never from which unit it was. A big hull breaks into
//! sections: one instance each, clipped to its stretch of the hull along the model's
//! length, strewn along the line it lay on and dug into the ground at its own angle, all
//! sharing the wreck's id, so a click on any of them picks (and reclaims) the one wreck.
//! Each section also draws its hull's inside, so the torn ends are not hollow
//! ([`WRECK_INNER`]).

use super::{
    UnitInstance, KIND_WRECK, WRECK_COUNT_SHIFT, WRECK_FALLING, WRECK_INNER, WRECK_POSED,
    WRECK_SECTION_SHIFT, WRECK_SETTLED, WRECK_SINKING,
};
use crate::tables::Landing;
use crate::World;
use bytemuck::Zeroable;
use mc_core::{FxVec2, TICKS_PER_SECOND};
use mc_data::UnitBlueprint;
use std::f32::consts::TAU;

/// Most sections a wreck breaks into.
const MOST_SECTIONS: usize = 4;
/// A cut past either end of the hull: the section runs to its end, uncut.
const OPEN: f32 = 4.0;
/// Extra instances sections and hull insides may add to a frame. A cosmetic cap: past
/// it, further big wrecks are drawn whole (`mc_render::MAX_DYNAMIC` has room for these).
pub const WRECK_EXTRA_INSTANCES: usize = 4096;

/// One section of a wreck: the stretch of the hull it keeps, along the model's length in
/// shares of its reach (-1 the stern, 1 the bow), and where it lies relative to where
/// the whole hull would.
#[derive(Clone, Copy)]
struct Section {
    lo: f32,
    hi: f32,
    /// Metres along the hull's heading and across it, from the wreck's position.
    along: f32,
    across: f32,
    yaw: f32,
    pitch: f32,
    roll: f32,
}

/// A number from zero to one for `seed` and `k`.
fn hash(seed: u32, k: u32) -> f32 {
    let mut n = seed.wrapping_mul(0x9E37_79B1) ^ k.wrapping_mul(0x85EB_CA77);
    n ^= n >> 15;
    n = n.wrapping_mul(0x2C1B_3C6D);
    n ^= n >> 12;
    (n >> 8) as f32 / 16_777_216.0
}

/// How many sections a wreck breaks into, from its size and domain and how it came
/// down. A spacecraft breaks up; so does anything big out of the sky; a big ship breaks
/// its back on the seabed. The rest lie whole.
fn section_count(bp: &UnitBlueprint, landing: Landing) -> usize {
    let r = bp.radius.to_f32();
    let fell = landing == Landing::Crashed;
    let n = if bp.is_capital_ship() {
        2 + (r >= 100.0) as usize + fell as usize
    } else if (bp.motion.is_some() && r >= 50.0) || (fell && r >= 25.0) {
        2
    } else {
        1
    };
    n.min(MOST_SECTIONS)
}

/// The sections of a wreck of `bp` that came down as `landing`, lying at `pitch` and
/// `roll` (radians) as a whole. The hull breaks its back: the pieces stay in line with a
/// gap between them, each a little off the line, and each piece's broken end goes down
/// into the ground.
fn sections(
    bp: &UnitBlueprint,
    landing: Landing,
    seed: u32,
    pitch: f32,
    roll: f32,
    out: &mut [Section; MOST_SECTIONS],
) -> usize {
    let n = section_count(bp, landing);
    if n == 1 {
        out[0] = Section {
            lo: -OPEN,
            hi: OPEN,
            along: 0.0,
            across: 0.0,
            yaw: 0.0,
            pitch,
            roll,
        };
        return 1;
    }
    let reach = bp.radius.to_f32().max(1.0);
    // Where the hull parts: evenly, each cut moved a little by the hull's own seed.
    let mut cuts = [OPEN; MOST_SECTIONS + 1];
    cuts[0] = -OPEN;
    for (i, cut) in cuts.iter_mut().enumerate().take(n).skip(1) {
        let even = -1.0 + 2.0 * i as f32 / n as f32;
        *cut = even + (hash(seed, i as u32) - 0.5) * 0.6 / n as f32;
    }
    cuts[n] = OPEN;
    // How far apart the pieces lie, as a share of the hull's reach.
    let spread = 0.06 + 0.1 * hash(seed, 11);
    for (i, s) in out.iter_mut().enumerate().take(n) {
        let (lo, hi) = (cuts[i], cuts[i + 1]);
        let mid = 0.5 * (lo.max(-1.0) + hi.min(1.0));
        let h = |k: u32| hash(seed, 16 + i as u32 * 8 + k) - 0.5;
        // The broken end goes down: a bow piece is broken at its stern, so it lies bow
        // up; a stern piece bow down; one from the middle either way.
        let dig = 0.12 + 0.2 * (h(0) + 0.5);
        let down = if i == n - 1 {
            1.0
        } else if i == 0 {
            -1.0
        } else if h(1) > 0.0 {
            1.0
        } else {
            -1.0
        };
        *s = Section {
            lo,
            hi,
            along: mid * reach * (1.0 + spread) + h(2) * reach * spread * 0.2,
            across: h(3) * reach * 0.12,
            yaw: h(4) * 0.25,
            pitch: pitch * 0.3 + down * dig,
            roll: roll + h(5) * 0.25,
        };
    }
    n
}

impl World {
    /// Settled wrecks, shown wherever the viewer knows of them (`wreck_known`).
    pub(super) fn push_wrecks(&self, viewer: Option<u8>, units: &mut Vec<UnitInstance>) {
        let s = &self.state;
        let mut extra = 0;
        for row in s.wrecks.slots.iter() {
            if viewer.is_some_and(|v| !self.wreck_known(row, v)) {
                continue;
            }
            let at = s.wrecks.pos[row];
            let pos = at.extend(s.wrecks.z[row]).to_f32();
            let heading = s.wrecks.heading[row].to_radians_f32();
            let bp = self.blueprints.unit(s.wrecks.blueprint[row]);
            // Seconds since it was left, for the thrown turret; the map's own lie long settled.
            let age = (s.tick.saturating_sub(s.wrecks.born[row]) as f32 / TICKS_PER_SECOND as f32)
                .min(WRECK_SETTLED);
            let fresh = !s.wrecks.from_map[row] && age < WRECK_SETTLED;
            let turret = s.wrecks.turret[row].to_radians_f32();
            let landing = landing_of(s.wrecks.landing[row]);
            let angle = |a: i16| a as f32 * (TAU / 65536.0);
            let id = s.wrecks.slots.handle(row).0;
            let mut pieces = [Section {
                lo: 0.0,
                hi: 0.0,
                along: 0.0,
                across: 0.0,
                yaw: 0.0,
                pitch: 0.0,
                roll: 0.0,
            }; MOST_SECTIONS];
            let mut n = sections(
                bp,
                landing,
                id,
                angle(s.wrecks.pitch[row]),
                angle(s.wrecks.bank[row]),
                &mut pieces,
            );
            // Each section past the first, and each one's inside, is one more instance.
            if n > 1 && extra + 2 * n > WRECK_EXTRA_INSTANCES {
                pieces[0] = Section {
                    lo: -OPEN,
                    hi: OPEN,
                    along: 0.0,
                    across: 0.0,
                    yaw: 0.0,
                    pitch: angle(s.wrecks.pitch[row]),
                    roll: angle(s.wrecks.bank[row]),
                };
                n = 1;
            }
            if n > 1 {
                extra += 2 * n - 1;
            }
            let ground = self.terrain.height_at(at);
            let (sin, cos) = heading.sin_cos();
            for (i, piece) in pieces.iter().take(n).enumerate() {
                let dx = cos * piece.along - sin * piece.across;
                let dy = sin * piece.along + cos * piece.across;
                let mut p = pos;
                if n > 1 {
                    p[0] += dx;
                    p[1] += dy;
                    // As high over the ground where it lies as the whole wreck is over its own.
                    let here = self.terrain.height_at(self.clamp_to_map(
                        at + FxVec2::new(mc_core::Fx::from_f32(dx), mc_core::Fx::from_f32(dy)),
                    ));
                    p[2] += (here - ground).to_f32();
                }
                let word = WRECK_POSED
                    | landing as u32
                    | (i as u32) << WRECK_SECTION_SHIFT
                    | (n as u32) << WRECK_COUNT_SHIFT;
                let whole = UnitInstance {
                    prev_pos: p,
                    prev_heading: heading + piece.yaw,
                    pos: p,
                    heading: heading + piece.yaw,
                    blueprint: bp.id.0 as u32,
                    owner_flags: KIND_WRECK,
                    health: (s.wrecks.mass[row] / s.wrecks.mass_max[row]).to_f32(),
                    build: 1.0,
                    turret_yaw: turret,
                    prev_turret_yaw: turret,
                    radius: bp.radius.to_f32(),
                    unit_id: id,
                    gait: if fresh {
                        [(age - 1.0 / TICKS_PER_SECOND as f32).max(0.0), age, 1.0]
                    } else {
                        [0.0; 3]
                    },
                    // Pitch last tick and this, then the stretch of the hull this piece keeps.
                    arm_pitch: [piece.pitch, piece.pitch, piece.lo, piece.hi],
                    _pad2: [piece.roll, piece.roll],
                    refit_modules: word,
                    deploy: 1.0,
                    prev_deploy: 1.0,
                    ..UnitInstance::zeroed()
                };
                units.push(whole);
                if n > 1 {
                    units.push(UnitInstance {
                        refit_modules: word | WRECK_INNER,
                        ..whole
                    });
                }
            }
        }
    }

    /// Hulls still coming down out of the sky: tumbling as they fall (`AircraftCrash`),
    /// and once in the sea turning to hang nose down, coming level over their last few
    /// metres to the bottom. Like wrecks, they show under the fog anywhere the viewer's
    /// team has explored, so a kill made out of sight is still seen coming down.
    pub(super) fn push_crashes(&self, viewer: Option<u8>, units: &mut Vec<UnitInstance>) {
        let s = &self.state;
        for crash in &s.aircraft_crashes {
            if let (Some(v), true) = (viewer, s.fog_enabled) {
                if !self.fog.is_explored(crash.pos.xy(), self.team_mask(v)) {
                    continue;
                }
            }
            let bp = self.blueprints.unit(crash.blueprint);
            let spin = crash.spin() as f32;
            let heading = crash.heading.to_radians_f32();
            let tps = TICKS_PER_SECOND as f32;
            let heft = crate::aircraft_crash::heft(bp.radius).to_f32();
            let angle = |a: i16| a as f32 * (TAU / 65536.0);
            let pose = |ticks: u16, pitch: i16, roll: i16, above: f32| {
                let air = ticks.min(if crash.splashed != 0 {
                    crash.splashed
                } else {
                    u16::MAX
                });
                let yaw = heading + spin * air as f32 / tps * 0.3 * heft;
                let (pitch, roll) = (angle(pitch), angle(roll));
                if crash.splashed == 0 {
                    return (yaw, pitch, roll);
                }
                let under = ticks.saturating_sub(crash.splashed) as f32 / tps;
                let turn = {
                    let u = (under / 2.0).clamp(0.0, 1.0);
                    u * u * (3.0 - 2.0 * u)
                };
                let level = {
                    let u = ((6.0 - above) / 5.0).clamp(0.0, 1.0);
                    u * u * (3.0 - 2.0 * u)
                };
                let nose = -0.55 * (1.0 - level);
                (
                    yaw,
                    pitch + (nose - pitch) * turn,
                    roll + (0.0 - roll) * turn,
                )
            };
            let floor = crash.floor.to_f32();
            let (prev_yaw, prev_pitch, prev_roll) = pose(
                crash.age.saturating_sub(1),
                crash.prev_pitch,
                crash.prev_roll,
                crash.prev_pos.z.to_f32() - floor,
            );
            let (yaw, pitch, roll) = pose(
                crash.age,
                crash.pitch,
                crash.roll,
                crash.pos.z.to_f32() - floor,
            );
            units.push(UnitInstance {
                prev_pos: crash.prev_pos.to_f32(),
                pos: crash.pos.to_f32(),
                prev_heading: prev_yaw,
                heading: yaw,
                blueprint: bp.id.0 as u32,
                owner_flags: KIND_WRECK,
                health: 1.0,
                build: 1.0,
                radius: bp.radius.to_f32(),
                unit_id: crash.unit_id,
                packed: WRECK_FALLING,
                // These otherwise unused wreck fields carry the tumble pitch.
                arm_pitch: [prev_pitch, pitch, 0.0, 0.0],
                _pad2: [prev_roll, roll],
                deploy: 1.0,
                prev_deploy: 1.0,
                ..UnitInstance::zeroed()
            });
        }
    }

    /// Ships' hulls going down through the water, listing and trimmed as the sim sinks them.
    /// Shown anywhere explored, like the wreck they settle into.
    pub(super) fn push_sinking(&self, viewer: Option<u8>, units: &mut Vec<UnitInstance>) {
        let s = &self.state;
        for hull in &s.sinking {
            if let (Some(v), true) = (viewer, s.fog_enabled) {
                if !self.fog.is_explored(hull.pos, self.team_mask(v)) {
                    continue;
                }
            }
            let bp = self.blueprints.unit(hull.blueprint);
            let angle = |a: i16| a as f32 * (TAU / 65536.0);
            let heading = hull.heading.to_radians_f32();
            units.push(UnitInstance {
                prev_pos: hull.pos.extend(hull.prev_z).to_f32(),
                pos: hull.pos.extend(hull.z).to_f32(),
                prev_heading: heading,
                heading,
                blueprint: bp.id.0 as u32,
                owner_flags: KIND_WRECK,
                health: hull.progress().to_f32(),
                build: 1.0,
                radius: bp.radius.to_f32(),
                unit_id: hull.unit_id,
                packed: WRECK_SINKING,
                arm_pitch: [angle(hull.prev_pitch), angle(hull.pitch), 0.0, 0.0],
                _pad2: [angle(hull.prev_roll), angle(hull.roll)],
                deploy: 1.0,
                prev_deploy: 1.0,
                ..UnitInstance::zeroed()
            });
        }
    }
}

/// A wreck's `landing` column as a [`Landing`]; an unknown number lies in place.
fn landing_of(n: u8) -> Landing {
    match n {
        1 => Landing::Crashed,
        2 => Landing::Sank,
        3 => Landing::Ditched,
        _ => Landing::InPlace,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blueprints() -> mc_data::Blueprints {
        mc_data::Blueprints::load(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"),
        )
        .expect("blueprints")
    }

    fn count(key: &str, landing: Landing) -> usize {
        let b = blueprints();
        section_count(b.unit(b.id_of(key).expect(key)), landing)
    }

    #[test]
    fn big_hulls_break_up_and_small_ones_lie_whole() {
        assert_eq!(count("aster_t1_tank", Landing::InPlace), 1);
        assert_eq!(count("aster_t1_interceptor", Landing::Crashed), 1);
        assert_eq!(count("aster_t1_frigate", Landing::Sank), 1);
        assert_eq!(count("aster_t3_battleship", Landing::Sank), 2);
        // Out of the sky a spacecraft breaks into more pieces than where it stood.
        let sky = count("aster_t2_lift_ship", Landing::Crashed);
        assert!(sky > count("aster_t2_lift_ship", Landing::InPlace));
        assert!(sky <= MOST_SECTIONS);
    }

    #[test]
    fn sections_cover_the_hull_once() {
        let b = blueprints();
        let bp = b.unit(b.id_of("aster_t3_frigate").expect("frigate"));
        let mut out = [Section {
            lo: 0.0,
            hi: 0.0,
            along: 0.0,
            across: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            roll: 0.0,
        }; MOST_SECTIONS];
        for seed in 0..64 {
            let n = sections(bp, Landing::Crashed, seed, -0.3, 0.1, &mut out);
            assert_eq!(out[0].lo, -OPEN);
            assert_eq!(out[n - 1].hi, OPEN);
            for w in out[..n].windows(2) {
                assert_eq!(w[0].hi, w[1].lo, "pieces meet");
                assert!(w[0].lo < w[0].hi);
                assert!(w[0].along < w[1].along, "pieces lie in hull order");
            }
        }
    }
}
