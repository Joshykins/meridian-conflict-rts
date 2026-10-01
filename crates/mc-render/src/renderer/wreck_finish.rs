//! The last of a wreck going up a reclaim beam (`SimEvent::Reclaimed` with `wreck`).
//!
//! The sim frees a wreck the tick its last mass goes, so the hull would simply vanish
//! while it still glowed with the work. Here a copy of the hull as it was last drawn
//! stays for [`GOING`] seconds: the reclaim net spreads over all of it and flares, then
//! the metal burns away from the top down behind a white-hot edge (`wreck.wgsl`
//! `wreck_going`), shedding motes of Materials that lift off it, over a soft flare of
//! light. The copy says how far it has gone in its `fx[2]`, past 1 (`UnitInstance::fx`).
//!
//! Presentation only: nothing here goes back into the sim.

use super::{clearing::PUFF_RECLAIM, wreck_fx::HullFrame, Renderer};
use crate::camera::Camera;
use glam::Vec3;
use mc_sim::mirror::{
    SimEvent, UnitInstance, KIND_WRECK, WRECK_COUNT_SHIFT, WRECK_INNER, WRECK_POSED,
};

/// Seconds the last of a wreck takes to go.
const GOING: f32 = 1.4;
/// Most hull instances going at once (a broken hull is several). A cosmetic cap: past
/// it the oldest go at once.
pub(super) const MOST_SHOWN: usize = 256;
/// Motes all going hulls may shed in one tick, at most (cosmetic).
const MOTES_PER_TICK: usize = 160;
/// Seconds since reclaim last took mass from a wreck, under which it may be finishing.
const WORKED: f32 = 1.0;

struct Going {
    instance: UnitInstance,
    start: f32,
}

#[derive(Default)]
pub(super) struct WreckFinish {
    /// Last tick's instances of the wrecks reclaim was working: only these can finish.
    worked: Vec<UnitInstance>,
    going: Vec<Going>,
}

impl WreckFinish {
    pub(super) fn clear(&mut self) {
        *self = WreckFinish::default();
    }

    /// Starts the hulls whose last mass went up a beam this tick, from how they were drawn
    /// last tick, and notes which wrecks reclaim is working now.
    fn note(&mut self, units: &[UnitInstance], events: &[SimEvent], time: f32) {
        let mut ids: Vec<u32> = Vec::new();
        for event in events {
            let SimEvent::Reclaimed {
                pos, wreck: true, ..
            } = event
            else {
                continue;
            };
            let at = Vec3::from(pos.to_f32()).truncate();
            // The nearest worked wreck that is gone this tick: a broken hull's sections lie
            // apart along it, so within about its reach of where the sim kept it.
            let gone = |u: &&UnitInstance| {
                !units
                    .iter()
                    .any(|now| now.unit_id == u.unit_id && now.owner_flags & KIND_WRECK != 0)
            };
            let nearest = self
                .worked
                .iter()
                .filter(|u| Vec3::from(u.pos).truncate().distance(at) <= u.radius * 1.6 + 4.0)
                .filter(gone)
                .min_by(|a, b| {
                    let d = |u: &UnitInstance| Vec3::from(u.pos).truncate().distance_squared(at);
                    d(a).total_cmp(&d(b))
                });
            if let Some(u) = nearest {
                if !ids.contains(&u.unit_id) {
                    ids.push(u.unit_id);
                }
            }
        }
        for u in self.worked.iter().filter(|u| ids.contains(&u.unit_id)) {
            if self.going.len() >= MOST_SHOWN {
                self.going.remove(0);
            }
            self.going.push(Going {
                instance: *u,
                start: time,
            });
        }
        self.worked.clear();
        self.worked.extend(units.iter().filter(|u| {
            u.owner_flags & KIND_WRECK != 0 && u.packed == 0 && u.fx[2] > 0.0 && u.fx[3] < WORKED
        }));
    }

    /// The hulls still going, each told how far it has gone; those gone are dropped.
    pub(super) fn instances(&mut self, time: f32) -> Vec<UnitInstance> {
        self.going.retain(|g| time - g.start < GOING);
        self.going
            .iter()
            .map(|g| {
                let mut instance = g.instance;
                // Past 1: going, 1 + how far (`wreck.wgsl wreck_going`), and the work on.
                instance.fx[2] = 1.0 + ((time - g.start) / GOING).clamp(1e-3, 1.0);
                instance.fx[3] = 0.0;
                instance
            })
            .collect()
    }
}

/// How far down the hull the going edge has burnt, as a share of its height from the
/// top, for `share` of the way through: as `wreck_going` moves it.
fn front(share: f32) -> f32 {
    let t = ((share - 0.2) / 0.8).clamp(0.0, 1.0);
    -0.3 + 1.6 * t * t * (3.0 - 2.0 * t)
}

impl Renderer {
    /// This tick's finished wrecks: their hulls start going, and those going shed motes.
    pub(super) fn wreck_finish(
        &mut self,
        units: &[UnitInstance],
        events: &[SimEvent],
        time: f32,
        camera: &Camera,
    ) {
        self.wreck_finish.note(units, events, time);
        let reach = camera.distance * 2.5 + 300.0;
        let mut budget = MOTES_PER_TICK;
        // Gathered first: standing a mote on the hull reads the site, shedding needs the
        // whole renderer.
        let mut motes: Vec<(Vec3, f32)> = Vec::new();
        for g in &self.wreck_finish.going {
            let u = &g.instance;
            if u.refit_modules & WRECK_INNER != 0
                || Vec3::from(u.pos).distance(camera.focus) > reach
            {
                continue;
            }
            let Some(site) = self.burn_sites.get(u.blueprint as usize) else {
                continue;
            };
            let share = (time - g.start) / GOING;
            // Down to the edge as it burns in: motes come off where the metal is going.
            let edge = site.height * (1.0 - front(share)).clamp(0.0, 1.0);
            // The stretch of the model this section keeps (`wreck.wgsl wreck_pose`).
            let posed = u.refit_modules & WRECK_POSED != 0;
            let count = (u.refit_modules >> WRECK_COUNT_SHIFT) & 15;
            let (lo, hi, centre) = if posed && count > 1 {
                let (lo, hi) = (u.arm_pitch[2] * site.bounds, u.arm_pitch[3] * site.bounds);
                (lo, hi, 0.5 * (lo.max(-site.bounds) + hi.min(site.bounds)))
            } else {
                (f32::MIN, f32::MAX, 0.0)
            };
            let size = site.reach.max(1.0);
            // A busy flurry early, thinning as the last of it goes.
            let n = ((size / 1.5).clamp(6.0, 30.0) * (1.2 - share)).ceil() as usize;
            for _ in 0..n.min(budget) {
                let angle = self.scatter.unit() * std::f32::consts::TAU;
                let out = size * 0.95 * self.scatter.unit().sqrt();
                let x = (angle.cos() * out).clamp(lo, hi);
                let Some((p, _)) = site.grid.surface(x, angle.sin() * out) else {
                    continue;
                };
                if p[0] < lo || p[0] > hi {
                    continue;
                }
                let z = p[2].min(edge + site.height * 0.08 * self.scatter.signed());
                let local = Vec3::new(p[0] - centre, p[1], z.max(0.0));
                motes.push((HullFrame::of(u, 1.0, posed).at(local), size));
                budget -= 1;
            }
        }
        for (at, size) in motes {
            let vel = self.scatter.upward(0.5) * (2.5 + self.scatter.unit() * 5.0);
            let life = 0.5 + self.scatter.unit() * 0.8;
            let start = time + self.scatter.unit() * self.tick_seconds;
            self.push_puff(
                PUFF_RECLAIM,
                at,
                vel,
                start,
                life,
                (0.12 + size * 0.015, 0.03),
            );
        }
    }

    /// The last of a unit, or of a wreck, went up a reclaim beam: a soft flare and a few
    /// embers, no blast, no smoke. A wreck's hull goes on burning away (`wreck_finish`).
    pub(super) fn reclaimed_flare(
        &mut self,
        pos: Vec3,
        blueprint: mc_data::BlueprintId,
        wreck: bool,
        time: f32,
    ) {
        let bp = self.blueprints.unit(blueprint);
        let (r, h) = (
            bp.radius.to_f32(),
            bp.height.to_f32() * if wreck { 0.25 } else { 0.5 },
        );
        let core = pos + Vec3::Z * h;
        // In the Materials red-orange of the beam it went up, not a gun's orange; a wreck's
        // a little longer and wider, as its hull goes on glowing.
        let (size, life) = if wreck { (1.7, 0.55) } else { (1.3, 0.3) };
        self.push_effect(
            core.to_array(),
            time,
            r * size,
            life,
            crate::gpu_consts::effect::MATERIALS as f32,
            0.0,
        );
        for _ in 0..10 {
            let vel = self.scatter.upward(0.6) * (2.0 + self.scatter.unit() * 4.0);
            let off = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.0) * r * 0.6;
            let life = 0.4 + self.scatter.unit() * 0.5;
            self.push_puff(
                PUFF_RECLAIM,
                core + off,
                vel,
                time,
                life,
                (0.14 + r * 0.02, 0.04),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mc_core::{Fx, FxVec3};
    use mc_data::BlueprintId;

    fn wreck(id: u32, x: f32, unmade: f32, since: f32) -> UnitInstance {
        UnitInstance {
            pos: [x, 0.0, 0.0],
            prev_pos: [x, 0.0, 0.0],
            owner_flags: KIND_WRECK,
            unit_id: id,
            radius: 5.0,
            fx: [0.0, 0.0, unmade, since],
            ..bytemuck::Zeroable::zeroed()
        }
    }

    fn reclaimed(x: i32) -> SimEvent {
        SimEvent::Reclaimed {
            pos: FxVec3::new(Fx::from_int(x), Fx::ZERO, Fx::ZERO),
            blueprint: BlueprintId(0),
            wreck: true,
        }
    }

    /// The worked wreck that went this tick goes on as a hull burning away, and is gone
    /// after `GOING`; a wreck nobody was working, or one still there, does not.
    #[test]
    fn a_finished_wreck_burns_away_then_goes() {
        let mut f = WreckFinish::default();
        let worked = wreck(7, 0.0, 0.9, 0.0);
        let idle = wreck(8, 2.0, 0.0, 600.0);
        let near = wreck(9, 3.0, 0.5, 0.0);
        f.note(&[worked, idle, near], &[], 0.0);
        // Wreck 7's last mass went; 8 and 9 are still there.
        f.note(&[idle, near], &[reclaimed(0)], 0.1);
        let going = f.instances(0.1 + GOING * 0.5);
        assert_eq!(going.len(), 1);
        assert_eq!(going[0].unit_id, 7);
        assert!((going[0].fx[2] - 1.5).abs() < 1e-3, "{}", going[0].fx[2]);
        assert!(f.instances(0.1 + GOING).is_empty());
    }

    /// The edge starts above the hull, so it first flares whole, and ends below it.
    #[test]
    fn the_edge_runs_from_over_the_top_to_under_the_keel() {
        assert!(front(0.0) < -0.22 && front(0.2) < -0.22);
        assert!(front(1.0) > 1.22);
    }
}
