//! A capital ship (`UnitBlueprint::is_capital_ship`) killed in the air: blown open, coming
//! down burning, and breaking up along its length where it hits.
//!
//! An aircraft's death is sized by its radius, which for a hull hundreds of metres long
//! made one fireball as big as the ship that took half a minute to burn out, and one
//! smoke puff swelling to 450 m round the middle of the hull as it fell. Here the fire
//! is sized by the plating it burns on (`fire_size`), a few dozen metres, and the size
//! of the ship shows in how many there are and how far along it they run:
//!
//! - Dying in the air: blasts inside the hull a beat apart, not one ball round it.
//! - Falling: fires at fixed places along the hull as `entity.wgsl` poses it (`HullFrame`
//!   on the model's upper surface, `BurnGrid`), each streaming a column of smoke up behind
//!   the fall, and now and then a blast somewhere on it.
//! - Hitting: blasts run along the keel from the end that struck first, each a short hot
//!   ball that is gone in a couple of seconds, with dust thrown out both sides of it,
//!   then smoke columns off the wreck.
//!
//! Presentation only. The sim says only where it hit and what it was; how the hull lay
//! comes from the last falling instance seen (`HullCrashFx::falling`).

use super::wreck_fx::{hash, HullFrame};
use super::{Renderer, PUFF_CLOD, PUFF_DUST, PUFF_FIRE, PUFF_SMOKE, PUFF_SPARK};
use glam::Vec3;
use mc_data::BlueprintId;
use mc_sim::mirror::UnitInstance;
use std::collections::HashMap;

/// Seconds a falling hull out of sight is remembered for its crash.
const FORGET: f32 = 3.0;
/// How fast the break-up runs along the hull from where it struck, m/s.
const RIPPLE: f32 = 180.0;

#[derive(Default)]
pub(super) struct HullCrashFx {
    /// Falling capital hulls by id: the instance last seen and when.
    falling: HashMap<u32, (UnitInstance, f32)>,
}

impl HullCrashFx {
    pub(super) fn clear(&mut self) {
        self.falling.clear();
    }
}

/// A hull's shape for its fires: half its length, its height, and the radius of one fire
/// on it.
struct Hull {
    reach: f32,
    height: f32,
    fire: f32,
}

impl Renderer {
    fn hull(&self, blueprint: u32) -> Hull {
        let bp = self.blueprints.unit(BlueprintId(blueprint as u16));
        let (reach, height) = self
            .burn_sites
            .get(blueprint as usize)
            .map_or((bp.radius.to_f32(), bp.height.to_f32()), |s| {
                (s.reach, s.height)
            });
        Hull {
            reach,
            height,
            fire: fire_size(reach),
        }
    }

    /// A point of the hull's plating `along` of the way from its middle to bow (-1 to 1)
    /// and `across` of its half length to one side, in the model's own frame: on the metal
    /// if there is any there, else on the keel line at mid height.
    fn plating(&self, blueprint: u32, reach: f32, along: f32, across: f32) -> Vec3 {
        let (x, y) = (along * reach, across * reach);
        self.burn_sites
            .get(blueprint as usize)
            .and_then(|site| {
                site.grid
                    .surface(x, y)
                    .map(|(p, _)| Vec3::from(p))
                    .or(Some(Vec3::new(x, 0.0, site.height * 0.5)))
            })
            .unwrap_or(Vec3::new(x, 0.0, 0.0))
    }

    /// One tick of a capital hull falling (`aircraft_crash_trails`): fires at a few fixed
    /// places along it, each laying smoke that rises off it as it drops.
    pub(super) fn capital_falling(&mut self, u: &UnitInstance, time: f32) {
        self.hull_crash_fx.falling.insert(u.unit_id, (*u, time));
        let hull = self.hull(u.blueprint);
        let tick = self.tick_seconds.max(0.02);
        let s = hull.fire;
        // One fire per 60 m or so of hull, fixed on it for the whole fall.
        let fires = ((hull.reach / 30.0).ceil() as u32).clamp(3, 9);
        for k in 0..fires {
            let along = hash(u.unit_id, 101 + k) * 1.7 - 0.85;
            let across = (hash(u.unit_id, 211 + k) - 0.5) * 0.18;
            let local = self.plating(u.blueprint, hull.reach, along, across);
            let t = self.scatter.unit();
            let at = HullFrame::of(u, t, true).at(local);
            let start = time + t * tick;
            if self.under_sea(at) {
                continue;
            }
            // The hotter fires (by the hull, not the tick) burn bigger.
            let heat = 0.7 + 0.6 * hash(u.unit_id, 307 + k);
            let lick = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                2.0 + self.scatter.unit(),
            );
            let life = 0.6 + self.scatter.unit() * 0.3;
            self.push_puff(
                PUFF_FIRE,
                at,
                lick * 2.0,
                start,
                life,
                (s * 0.45 * heat, s * heat),
            );
            if self.scatter.unit() < 0.7 {
                let rise = Vec3::new(
                    1.5 + self.scatter.signed(),
                    0.5 + self.scatter.signed(),
                    6.0 + self.scatter.unit() * 4.0,
                );
                let life = 6.0 + self.scatter.unit() * 2.5;
                self.push_puff(
                    PUFF_SMOKE,
                    at + Vec3::Z * s * 0.4,
                    rise,
                    start + 0.05,
                    life,
                    (s * 0.7, s * (3.5 + 1.5 * heat)),
                );
            }
            if self.scatter.unit() < 0.3 {
                let vel = self.scatter.upward(0.1) * (8.0 + self.scatter.unit() * 10.0);
                self.push_puff(PUFF_SPARK, at, vel, start, 0.9, (0.4, 0.05));
            }
        }
        // Now and then something inside goes up.
        if self.scatter.unit() < 0.06 {
            let along = self.scatter.signed() * 0.85;
            let across = self.scatter.signed() * 0.08;
            let local = self.plating(u.blueprint, hull.reach, along, across);
            let at = HullFrame::of(u, 1.0, true).at(local);
            self.fireball(at, s * 1.3, 3, 0.45, 0.85, time);
            for _ in 0..8 {
                let vel = self.scatter.upward(-0.2) * (14.0 + self.scatter.unit() * 16.0);
                self.push_puff(PUFF_SPARK, at, vel, time, 1.1, (0.5, 0.06));
            }
        }
    }

    /// Forgets the falling hulls not seen for a while (gone out of the mirror some other way).
    pub(super) fn forget_falling_hulls(&mut self, time: f32) {
        self.hull_crash_fx
            .falling
            .retain(|_, (_, seen)| time - *seen < FORGET);
    }

    /// A capital ship killed in the air: blasts inside it a beat apart, and the
    /// flash and front of one big one. Its fall and crash follow (`capital_falling`).
    pub(super) fn capital_air_death(&mut self, at: Vec3, blueprint: u32, time: f32) {
        let hull = self.hull(blueprint);
        let s = hull.fire;
        self.push_effect(at.to_array(), time, s * 6.0, 0.16, 1.0, 0.0);
        self.push_shockwave(
            at.to_array(),
            time,
            hull.reach * 1.6,
            0.6,
            0.6,
            0.0,
            Vec3::ZERO,
        );
        for i in 0..6 {
            let off = Vec3::new(
                self.scatter.signed() * hull.reach * 0.55,
                self.scatter.signed() * hull.reach * 0.55,
                self.scatter.signed() * s,
            );
            let delay = i as f32 * 0.14 + self.scatter.unit() * 0.1;
            let size = s * (1.5 + self.scatter.unit() * 0.8);
            self.fireball(at + off, size, 4, 0.6, 0.95, time + delay);
            for _ in 0..6 {
                let vel = self.scatter.upward(-0.3) * (18.0 + self.scatter.unit() * 24.0);
                self.push_puff(PUFF_SPARK, at + off, vel, time + delay, 1.3, (0.5, 0.06));
            }
        }
        for _ in 0..12 {
            let vel = self.scatter.upward(0.1) * (15.0 + self.scatter.unit() * 15.0);
            self.push_puff(PUFF_CLOD, at, vel, time, 2.0, (0.9, 0.4));
        }
    }

    /// A capital ship hitting the ground or the sea at `at`: the hull breaks up from the
    /// end that struck, blast after blast along its length, dust thrown out both sides,
    /// and smoke standing off the wreck after.
    pub(super) fn capital_crash(&mut self, at: Vec3, blueprint: u32, time: f32) {
        let hull = self.hull(blueprint);
        let s = hull.fire;
        // The hull as it last fell, if it was seen: nearest of that kind to where it hit.
        let seen = self
            .hull_crash_fx
            .falling
            .iter()
            .filter(|(_, (u, _))| u.blueprint == blueprint)
            .map(|(id, (u, _))| {
                (
                    *id,
                    *u,
                    Vec3::from(u.pos).truncate().distance(at.truncate()),
                )
            })
            .filter(|(_, _, d)| *d < hull.reach * 2.0)
            .min_by(|a, b| a.2.total_cmp(&b.2).then(a.0.cmp(&b.0)));
        let frame = seen.map(|(id, u, _)| {
            self.hull_crash_fx.falling.remove(&id);
            (u.unit_id, HullFrame::of(&u, 1.0, true))
        });
        let sea = self.sea_level();
        let on_land = self.ground_height(at.truncate()) > sea + 0.5;

        self.push_effect(at.to_array(), time, s * 7.0, 0.18, 5.0, 1.0);
        self.push_shockwave(
            at.to_array(),
            time,
            22.0 + hull.reach * 2.0,
            0.8,
            0.8,
            1.0,
            Vec3::ZERO,
        );

        // Points along the keel, each with when the break-up reaches it.
        let blasts = ((hull.reach / 18.0).ceil() as u32).clamp(6, 26);
        let mut points: Vec<Vec3> = (0..blasts)
            .map(|i| {
                let along = (i as f32 + 0.5) / blasts as f32 * 1.8 - 0.9
                    + self.scatter.signed() * 0.4 / blasts as f32;
                let across = self.scatter.signed() * 0.1;
                let local = self.plating(blueprint, hull.reach, along, across);
                match &frame {
                    Some((_, f)) => f.at(local),
                    // Not seen falling: laid out anyhow round where it hit.
                    None => at + Vec3::new(local.x, local.y, local.z),
                }
            })
            .collect();
        // It strikes with its lowest end first, as it fell.
        let first = points
            .iter()
            .copied()
            .min_by(|a, b| a.z.total_cmp(&b.z))
            .unwrap_or(at);
        points.sort_by(|a, b| a.distance(first).total_cmp(&b.distance(first)));
        // The wreck it leaves lies flatter than it fell and stands most of its height
        // off the ground: a blast in the plating as it fell would burn inside it, unseen.
        for p in &mut points {
            let deck = self.ground_height(p.truncate()) + hull.height * 0.75 + s * 0.3;
            p.z = p.z.max(deck);
        }
        let first = first.truncate().extend(points[0].z);
        let unit = frame.as_ref().map_or(blueprint, |(id, _)| *id);
        // Half its beam, for the dust to rise clear of its sides.
        let beam = (hull.reach * 0.22).max(s);
        for (i, p) in points.iter().copied().enumerate() {
            let delay = p.distance(first) / RIPPLE + self.scatter.unit() * 0.12;
            let start = time + delay;
            let size = s * (1.6 + self.scatter.unit() * 0.9);
            // Short and hot: burns under a second, gone as smoke in two or three.
            let burn = 0.55 + self.scatter.unit() * 0.35;
            self.fireball(p, size, 4, burn, 0.85, start);
            if i % 2 == 0 {
                for _ in 0..8 {
                    let vel = self.scatter.upward(0.15) * (16.0 + self.scatter.unit() * 26.0);
                    let life = 0.8 + self.scatter.unit() * 1.2;
                    self.push_puff(PUFF_SPARK, p, vel, start, life, (0.6, 0.06));
                }
                for _ in 0..3 {
                    let vel = self.scatter.upward(0.35) * (12.0 + self.scatter.unit() * 14.0);
                    self.push_puff(PUFF_CLOD, p, vel, start, 2.2, (1.2, 0.5));
                }
            }
            if on_land {
                // Dust thrown out to both sides of the keel where it ploughs in.
                for side in [-1.0f32, 1.0] {
                    let out = match &frame {
                        Some((_, f)) => f.at(Vec3::Y * side) - f.at(Vec3::ZERO),
                        None => Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0),
                    };
                    let out = out.truncate().normalize_or_zero().extend(0.08);
                    let foot = p.truncate().extend(self.ground_height(p.truncate()) + 1.0);
                    let life = 3.0 + self.scatter.unit() * 1.5;
                    let speed = 18.0 + self.scatter.unit() * 14.0;
                    self.push_puff(
                        PUFF_DUST,
                        foot + out * (beam + s),
                        out * speed,
                        start + 0.05,
                        life,
                        (s * 0.8, s * 3.2),
                    );
                }
            }
        }
        // Smoke standing off the wreck once the blasts have run: a few columns, not a
        // blanket over it. (Its own wreck smoke takes over from there: wreck_fx.rs.)
        let columns = (blasts / 4).clamp(2, 6) as usize;
        for c in 0..columns {
            let p = points[(c * points.len()) / columns];
            let delay = p.distance(first) / RIPPLE + 0.8;
            for k in 0..6 {
                let rise = Vec3::new(
                    1.2 + self.scatter.signed(),
                    0.4 + self.scatter.signed(),
                    5.0 + self.scatter.unit() * 3.0,
                );
                let life = 6.5 + self.scatter.unit() * 3.0 + hash(unit, 401 + c as u32);
                self.push_puff(
                    PUFF_SMOKE,
                    p + Vec3::Z * s * 0.5,
                    rise,
                    time + delay + k as f32 * 0.45,
                    life,
                    (s, s * 4.5),
                );
            }
        }
    }
}

/// The radius of one fire or blast on a capital hull `reach` metres from middle to bow,
/// metres: by the plating it burns on, not the ship's size.
fn fire_size(reach: f32) -> f32 {
    (reach * 0.06).clamp(4.0, 15.0)
}

#[cfg(test)]
mod tests {
    use super::fire_size;

    #[test]
    fn a_fire_on_a_capital_hull_is_sized_by_its_plating_not_its_length() {
        // The Dreadnought is about 500 m long: its fires stay a few dozen metres across.
        assert!(fire_size(250.0) <= 15.0);
        assert!(fire_size(36.0) >= 2.0);
        assert!(fire_size(150.0) < fire_size(250.0) + 0.01);
    }
}
