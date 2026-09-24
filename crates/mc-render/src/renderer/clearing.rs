//! A builder's clearing field over a lot (`SimEvent::LotClearing`), and the
//! trees it takes (`SimEvent::TreeVaporized`). One effect for the whole lot:
//! a curtain of reclaim-coloured heat rises along its edge while it is up,
//! each wave runs out from the middle as a square front of motes, and a tree
//! the front reaches comes apart into glowing bits that drift up and in.
//!
//! Everything is scheduled when the event arrives: puffs may start in the
//! future, so a wave's whole sweep is written at once. A vaporizing tree is
//! drawn as a dynamic copy of its prop; `health` above one tells entity.wgsl
//! how far it has come apart (`vapor_edge`).

use super::Renderer;
use glam::{Vec2, Vec3};
use mc_core::TICKS_PER_SECOND;
use mc_sim::mirror::{RenderFrame, SimEvent, UnitInstance};
use mc_sim::trees::{CLEAR_WAVES, WAVE_TICKS};

/// A heat mote in the reclaim beam's colours: white-hot, cooling through orange to red as it climbs.
pub(super) const PUFF_RECLAIM: f32 = 22.0;
/// Seconds a tree takes to come apart once the front reaches it.
const VAPORIZE: f32 = 1.1;
/// Share of a wave's period its front takes to cross its band.
const SWEEP: f32 = 0.85;
/// Metres between motes along the edge curtain and along a front.
const CURTAIN_STEP: f32 = 4.0;
const FRONT_STEP: f32 = 3.5;
/// How high the curtain and the fronts reach: up through a wood's canopy, so
/// the field shows over the trees and not only under them.
const REACH_UP: f32 = 24.0;

#[derive(Clone, Copy)]
pub(super) struct VaporTree {
    instance: UnitInstance,
    prop: u32,
    start: f32,
}

/// A wave front on its way this tick: where, the band it crosses, and when.
struct Front {
    center: Vec2,
    from: f32,
    to: f32,
    start: f32,
}

/// Seconds between waves.
fn period() -> f32 {
    WAVE_TICKS as f32 / TICKS_PER_SECOND as f32
}

/// A point `along` (0..1) the edge of a square `r` metres each way from `center`.
fn on_square(center: Vec2, r: f32, along: f32) -> (Vec2, Vec2) {
    let s = along.fract() * 4.0;
    let side = s.floor() as u32;
    let t = s.fract() * 2.0 - 1.0;
    let (offset, out) = match side {
        0 => (Vec2::new(t, -1.0), Vec2::NEG_Y),
        1 => (Vec2::new(1.0, t), Vec2::X),
        2 => (Vec2::new(-t, 1.0), Vec2::Y),
        _ => (Vec2::new(-1.0, -t), Vec2::NEG_X),
    };
    (center + offset * r, out)
}

impl Renderer {
    /// Schedules the field, its waves and the trees they take for this tick's events.
    pub(super) fn clear_lots(&mut self, frame: &RenderFrame, time: f32) {
        let period = period();
        self.fallen_trees.vapor.retain(|t| time - t.start < VAPORIZE);
        self.fallen_trees.seen.retain(|s| time - s.1 < period * 0.5);
        let mut fronts = Vec::new();
        for event in &frame.events {
            let SimEvent::LotClearing { pos, half, wave } = event else {
                continue;
            };
            // The same tick's events can be handed over more than once.
            let key = (pos.x.raw() as u64).rotate_left(21) ^ pos.y.raw() as u64 ^ *wave as u64;
            if self.fallen_trees.seen.iter().any(|s| s.0 == key) {
                continue;
            }
            self.fallen_trees.seen.push((key, time));
            let center = Vec2::from(pos.to_f32());
            let half = half.to_f32();
            if *wave == 0 {
                self.raise_field(center, half, time);
            } else {
                let band = half / CLEAR_WAVES as f32;
                let front = Front {
                    center,
                    from: band * (*wave - 1) as f32,
                    to: band * *wave as f32,
                    start: time,
                };
                self.send_front(&front, *wave == CLEAR_WAVES);
                fronts.push(front);
            }
        }
        for event in &frame.events {
            let SimEvent::TreeVaporized { prop, center } = event else {
                continue;
            };
            if self.fallen_trees.vapor.iter().any(|t| t.prop == *prop) {
                continue;
            }
            let Some(&instance) = self.prop_instances.get(*prop as usize) else {
                continue;
            };
            let kind = instance.blueprint.wrapping_sub(self.tree_model_base);
            if kind >= 4 {
                continue;
            }
            let center = Vec2::from(center.to_f32());
            let at = Vec2::new(instance.pos[0], instance.pos[1]);
            // It goes when the front gets to it, not when the sim struck it off.
            let start = fronts
                .iter()
                .find(|f| f.center == center)
                .map_or(time, |f| {
                    let d = (at - center).abs().max_element();
                    let k = ((d - f.from) / (f.to - f.from).max(0.01)).clamp(0.0, 1.0);
                    f.start + k * period * SWEEP
                });
            if self.fallen_trees.vapor.len() >= 512 {
                self.fallen_trees.vapor.remove(0);
            }
            self.fallen_trees.vapor.push(VaporTree { instance, prop: *prop, start });
            let height = [12.0, 14.0, 18.0, 9.0][kind as usize] * instance._pad as f32 * 0.001;
            let foot = at.extend(self.ground_height(at));
            let inward = (center - at).normalize_or_zero().extend(0.0);
            for i in 0..18 {
                let k = i as f32 / 18.0;
                let angle = self.scatter.unit() * std::f32::consts::TAU;
                let spread = height * 0.24 * self.scatter.unit().sqrt();
                let pos = foot
                    + Vec3::new(angle.cos() * spread, angle.sin() * spread, 0.0)
                    + Vec3::Z * height * (0.2 + 0.8 * self.scatter.unit());
                // Crowns go first: the high bits are the early ones.
                let when = start + (1.0 - (pos.z - foot.z) / height).clamp(0.0, 1.0) * VAPORIZE * 0.7
                    + k * 0.12;
                let vel = inward * (1.0 + self.scatter.unit() * 1.5)
                    + Vec3::Z * (1.5 + self.scatter.unit() * 2.5);
                let life = 1.0 + self.scatter.unit() * 0.6;
                self.push_puff(PUFF_RECLAIM, pos, vel, when, life, (0.55, 0.12));
            }
        }
    }

    /// The field going up: heat rising along the lot's edge for as long as it lasts,
    /// brighter at the corners.
    fn raise_field(&mut self, center: Vec2, half: f32, time: f32) {
        let lasts = period() * (CLEAR_WAVES as f32 + 0.3);
        let points = ((8.0 * half / CURTAIN_STEP) as usize).max(8);
        let beats = (lasts / 0.45).ceil() as usize;
        for i in 0..points {
            let (xy, out) = on_square(center, half, i as f32 / points as f32);
            let ground = self.ground_height(xy);
            for b in 0..beats {
                let when = time + (b as f32 + self.scatter.unit()) * lasts / beats as f32;
                let pos = xy.extend(ground + 0.2 + self.scatter.unit() * REACH_UP);
                let vel = Vec3::Z * (2.0 + self.scatter.unit() * 2.0) - out.extend(0.0) * 0.4;
                let life = 1.1 + self.scatter.unit() * 0.5;
                self.push_puff(PUFF_RECLAIM, pos, vel, when, life, (0.9, 0.2));
            }
        }
        for corner in 0..4 {
            // Each side starts at a corner.
            let (xy, _) = on_square(center, half, corner as f32 * 0.25);
            let ground = self.ground_height(xy);
            let count = (lasts / 0.1) as usize;
            for c in 0..count {
                let when = time + c as f32 * 0.1 + self.scatter.unit() * 0.1;
                let vel = Vec3::Z * (5.0 + self.scatter.unit() * 3.0);
                self.push_puff(PUFF_RECLAIM, xy.extend(ground + 0.3), vel, when, 1.6, (1.4, 0.3));
            }
        }
    }

    /// One wave's front: a square of motes running out across its band.
    fn send_front(&mut self, front: &Front, last: bool) {
        const STEPS: usize = 7;
        let sweep = period() * SWEEP;
        for s in 0..STEPS {
            let k = (s as f32 + 0.5) / STEPS as f32;
            let r = front.from + (front.to - front.from) * k;
            let when = front.start + sweep * k;
            let points = ((8.0 * r / FRONT_STEP) as usize).max(6);
            let phase = self.scatter.unit();
            for i in 0..points {
                let (xy, out) = on_square(front.center, r.max(0.5), (i as f32 + phase) / points as f32);
                // Most hug the ground; some run through the crowns.
                let up = self.scatter.unit();
                let pos = xy.extend(self.ground_height(xy) + 0.3 + up * up * REACH_UP);
                let vel = out.extend(0.0) * (2.5 + self.scatter.unit() * 1.5)
                    + Vec3::Z * (1.2 + self.scatter.unit() * 2.4);
                let life = 0.7 + self.scatter.unit() * 0.4;
                let size = if last && s == STEPS - 1 { (1.6, 0.3) } else { (1.1, 0.2) };
                let when = when + self.scatter.unit() * 0.03;
                self.push_puff(PUFF_RECLAIM, pos, vel, when, life, size);
            }
        }
    }

    /// Trees coming apart, posed for the vertex shader: whole until the front
    /// reaches them, then `health` runs from one to two as they go.
    pub(super) fn vapor_instances(&self, time: f32) -> impl Iterator<Item = UnitInstance> + '_ {
        self.fallen_trees.vapor.iter().map(move |tree| {
            let mut instance = tree.instance;
            let v = (time - tree.start) / VAPORIZE;
            instance.health = if v <= 0.0 { 1.0 } else { 1.0 + v.clamp(0.001, 1.0) };
            instance
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use super::{period, VAPORIZE};
    use glam::Vec2;
    use mc_core::{Fx, FxVec2};
    use mc_sim::trees::CLEAR_WAVES;

    /// Real Vulkan check: a clearing field over a patch of forest, played the
    /// way the sim sends it. `CLEARING_DIR` gets a picture of each stage.
    #[test]
    #[ignore = "requires Vulkan and maps/dev16.mcmap"]
    fn a_clearing_field_vaporizes_a_wood_in_waves() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let out = std::env::var("CLEARING_DIR").map(std::path::PathBuf::from).ok();
        let map = Arc::new(MapFile::open(root.join("maps/dev16.mcmap")).unwrap());
        let blueprints = Arc::new(Blueprints::load(&root.join("data")).unwrap());
        let start = Vec2::from(map.start_positions()[0].to_f32());
        let trees: Vec<(usize, Vec2)> = map.props().iter().enumerate()
            .filter(|(_, p)| p.kind.is_tree())
            .map(|(i, p)| (i, Vec2::from(p.pos.to_f32())))
            .collect();
        let edge = trees.iter().min_by(|a, b| a.1.distance_squared(start)
            .total_cmp(&b.1.distance_squared(start))).unwrap().1;
        // A power plant's lot (4 x 4 cells), set into the wood from its edge.
        let half = 26.0f32;
        let center = edge + (edge - start).normalize() * 18.0;
        let fx = |v: f32| Fx::from_f32(v);
        let pos = FxVec2::new(fx(center.x), fx(center.y));
        let on_lot: Vec<_> = trees.iter()
            .filter(|t| (t.1 - center).abs().max_element() <= half)
            .copied().collect();
        assert!(on_lot.len() > 10, "a wood to clear ({} trees)", on_lot.len());

        let mut renderer = Renderer::new(Target::Headless { width: 960, height: 720 }, SceneDesc {
            map: map.clone(), blueprints, pool: Arc::new(Pool::new(2)),
            team_colors: [[0.1, 0.6, 0.9]; 8],
        }).unwrap();
        let mut camera = Camera::new(Vec2::from(map.info().size_metres().to_f32()), Vec2::new(960.0, 720.0));
        camera.focus = center.extend(renderer.ground_height(center) + 4.0);
        camera.distance = 120.0;
        camera.tilt = 0.5;
        let mut frame = RenderFrame::default();
        frame.props_dead = vec![0; map.props().len().div_ceil(32)];
        let overlay = Overlay::default();
        let mut shots = vec![(0.55, "1-field-up"), (1.9, "2-second-wave"), (3.5, "3-last-wave"), (5.2, "4-cleared")];
        shots.reverse();
        let (mut time, mut wave) = (0.0f32, 0u8);
        let mut next_wave = 0.0f32;
        while time < 5.3 {
            frame.events.clear();
            if wave <= CLEAR_WAVES && time >= next_wave {
                frame.events.push(SimEvent::LotClearing { pos, half: fx(half), wave });
                let reach = half * wave as f32 / CLEAR_WAVES as f32;
                for &(index, at) in &on_lot {
                    let gone = frame.props_dead[index / 32] & (1 << (index % 32)) != 0;
                    if wave > 0 && !gone && (at - center).abs().max_element() <= reach {
                        frame.props_dead[index / 32] |= 1 << (index % 32);
                        frame.events.push(SimEvent::TreeVaporized { prop: index as u32, center: pos });
                    }
                }
                wave += 1;
                next_wave += period();
            }
            renderer.render(&FrameInput { camera: &camera, time, alpha: 1.0,
                sim: Some(&frame), ghosts: &[], marks: &[], ranges: &[], ranges_drawn: 0,
                overlay: &overlay, build_grid: false }).unwrap();
            if shots.last().is_some_and(|s| time >= s.0) {
                let (_, name) = shots.pop().unwrap();
                if let Some(dir) = &out {
                    let pixels = renderer.read_pixels().unwrap();
                    let mut ppm = b"P6\n960 720\n255\n".to_vec();
                    for pixel in pixels.chunks_exact(4) { ppm.extend_from_slice(&pixel[..3]); }
                    std::fs::create_dir_all(dir).unwrap();
                    std::fs::write(dir.join(format!("{name}.ppm")), ppm).unwrap();
                }
            }
            time += 0.05;
        }
        assert!(on_lot.iter().all(|t| frame.props_dead[t.0 / 32] & (1 << (t.0 % 32)) != 0));
        assert!(renderer.fallen_trees.vapor.len() <= on_lot.len());
        renderer.clear_lots(&RenderFrame::default(), time + VAPORIZE);
        assert!(renderer.fallen_trees.vapor.is_empty(), "vaporized trees are gone in the end");
    }
}
