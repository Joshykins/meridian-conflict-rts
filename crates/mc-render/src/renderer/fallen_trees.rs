//! Trees a big walker knocked over (`SimEvent::TreeTrampled`). The sim only
//! marks them dead, so the static tree stops drawing; here a copy of it tips
//! over from its foot, away from the walker, lands in a puff of dust, lies
//! there a while and then sinks into the ground. The vertex shader poses it:
//! a prop with a nonzero `arm_pitch.x` is pitched about its foot along its
//! heading, and `arm_pitch.z` lowers it by that many metres. `arm_pitch.w` is
//! the tree's own stretch in height (`height_stretch`), standing or fallen.

use super::{Renderer, PUFF_CLOD, PUFF_DUST};
use glam::{Vec2, Vec3};
use mc_sim::mirror::{RenderFrame, SimEvent, UnitInstance};

/// Most fallen trees kept at once; the oldest go first.
const MOST: usize = 2048;
/// Most fallen and vaporizing trees drawn at once (`fallen_tree_instances`).
pub(super) const MOST_SHOWN: usize = MOST + super::clearing::VAPOR_MOST;
/// Seconds a blast-thrown tree takes to go from standing to lying down, and the
/// most a trampled one takes: a walker's falls follow its pace (`trample_fall`).
const FALL: f32 = 1.1;
/// The least a trampled tree takes to fall, however fast the walker: quicker
/// reads as the tree vanishing rather than going over.
const FALL_QUICKEST: f32 = 0.3;
/// How far over a tree ends up, radians: its branches prop it a little off the ground.
const LIE: f32 = 1.48;
/// Seconds it lies there before it starts to sink, and how long sinking takes.
const LINGER: f32 = 30.0;
const SINK: f32 = 8.0;

/// Tree prop models come first among the props (`PropKind::ALL` order: broadleaf,
/// conifer, pine, dead, palm, jungle, juniper, pinyon, cottonwood); a prop's
/// model minus `tree_model_base` below this is a tree.
pub(super) const TREE_KINDS: u32 = 9;
/// Roughly how tall each of those trees stands at scale 1, metres.
pub(super) const TREE_HEIGHTS: [f32; TREE_KINDS as usize] =
    [12.0, 14.0, 18.0, 9.0, 15.0, 22.0, 6.0, 9.0, 17.0];

/// Tree `prop`'s own stretch in height, over what its map scale gives it, kept in
/// its instance's `arm_pitch[3]`: the vertex shader stretches the tree up by it, so
/// a wood's trees stand at uneven heights whatever the size of their crowns.
/// Two rolls averaged, so most stay near their size and a few stand well over or
/// under their neighbours.
pub(super) fn height_stretch(prop: u32) -> f32 {
    let roll =
        (super::wreck_fx::hash(prop, 0x7472_6565) + super::wreck_fx::hash(prop, 0x7461_6C6C)) * 0.5;
    0.7 + 0.6 * roll
}

/// How tall a tree prop stands, metres: its kind's height at its scale and stretch.
pub(super) fn tree_height(kind: u32, instance: &UnitInstance) -> f32 {
    let stretch = if instance.arm_pitch[3] > 0.0 {
        instance.arm_pitch[3]
    } else {
        1.0
    };
    TREE_HEIGHTS[kind as usize] * instance.packed as f32 * 0.001 * stretch
}

#[derive(Clone, Copy)]
pub(super) struct FallenTree {
    instance: UnitInstance,
    prop: u32,
    start: f32,
    /// Seconds from standing to lying down.
    fall: f32,
    height: f32,
    landed: bool,
}

/// Seconds a tree knocked over at `at` by a walker at `from` moving `motion` a tick
/// takes to fall: it is down by the time the walker's middle reaches its foot, so
/// the walker never wades through the part still standing. A slow walker's trees
/// take their time, a fast one's go over at once.
fn trample_fall(at: Vec2, from: Vec2, motion: Vec2) -> f32 {
    let speed = motion.length() * mc_core::TICKS_PER_SECOND as f32;
    if speed <= 1e-3 {
        return FALL;
    }
    (at.distance(from) / speed).clamp(FALL_QUICKEST, FALL)
}

/// Trees drawn as dynamic props: knocked over, or coming apart in a clearing
/// field (`clearing.rs`), and the clearing waves already scheduled.
#[derive(Default)]
pub(super) struct FallenTrees {
    fallen: Vec<FallenTree>,
    pub(super) vapor: Vec<super::clearing::VaporTree>,
    pub(super) seen: Vec<(u64, f32)>,
}

impl Renderer {
    /// Starts a fall for each tree trampled this tick.
    pub(super) fn trample_trees(&mut self, frame: &RenderFrame, time: f32) {
        self.fallen_trees
            .fallen
            .retain(|t| time - t.start < LINGER + SINK);
        for event in &frame.events {
            let SimEvent::TreeTrampled { prop, from, motion } = event else {
                continue;
            };
            // The same tick's events can be handed over more than once.
            if self.fallen_trees.fallen.iter().any(|t| t.prop == *prop) {
                continue;
            }
            let Some(&instance) = self.prop_instances.get(*prop as usize) else {
                continue;
            };
            let kind = instance.blueprint.wrapping_sub(self.tree_model_base);
            if kind >= TREE_KINDS {
                continue;
            }
            let at = Vec2::new(instance.pos[0], instance.pos[1]);
            let from = Vec2::from(from.to_f32());
            let motion = Vec2::from(motion.to_f32());
            let fall = trample_fall(at, from, motion);
            let away = (at - from).normalize_or_zero();
            let ahead = motion.normalize_or_zero();
            // Mostly the way the walker goes, pushed off to the side it passed on.
            let dir = (ahead + away * 0.7).normalize_or(ahead);
            let height = tree_height(kind, &instance);
            let mut instance = instance;
            // A little turn of its own, so a row of trees does not fall in step.
            let heading = dir.y.atan2(dir.x) + self.scatter.signed() * 0.25;
            instance.heading = heading;
            instance.prev_heading = heading;
            if self.fallen_trees.fallen.len() >= MOST {
                self.fallen_trees.fallen.remove(0);
            }
            self.fallen_trees.fallen.push(FallenTree {
                instance,
                prop: *prop,
                // Out of step a little, by a share of its fall.
                start: time + self.scatter.unit() * 0.15 * fall / FALL,
                fall,
                height,
                landed: false,
            });
            // Snapped at the foot: chips of wood.
            let foot = Vec3::new(at.x, at.y, self.ground_height(at) + 0.8);
            for _ in 0..5 {
                let spray = Vec3::new(
                    dir.x + self.scatter.signed() * 0.8,
                    dir.y + self.scatter.signed() * 0.8,
                    0.6 + self.scatter.unit(),
                );
                self.push_puff(PUFF_CLOD, foot, spray * 6.0, time, 0.8, (0.3, 0.2));
            }
        }
    }

    /// A tree a blast threw flat (`prop`), falling `away` from it from `start`: scorched,
    /// quick, and with more dust than a trampled one.
    pub(super) fn blow_down_tree(&mut self, prop: u32, away: Vec2, start: f32) {
        if self.fallen_trees.fallen.iter().any(|t| t.prop == prop) {
            return;
        }
        let Some(&instance) = self.prop_instances.get(prop as usize) else {
            return;
        };
        let kind = instance.blueprint.wrapping_sub(self.tree_model_base);
        if kind >= TREE_KINDS {
            return;
        }
        let height = tree_height(kind, &instance);
        let mut instance = instance;
        let heading = away.y.atan2(away.x) + self.scatter.signed() * 0.12;
        instance.heading = heading;
        instance.prev_heading = heading;
        // Seared by the flash before the air arrived.
        instance.health = 0.12;
        if self.fallen_trees.fallen.len() >= MOST {
            self.fallen_trees.fallen.remove(0);
        }
        self.fallen_trees.fallen.push(FallenTree {
            instance,
            prop,
            start,
            fall: FALL,
            height,
            landed: false,
        });
    }

    /// Dust where each tree comes down.
    pub(super) fn land_fallen_trees(&mut self, time: f32) {
        for i in 0..self.fallen_trees.fallen.len() {
            let tree = self.fallen_trees.fallen[i];
            if tree.landed || time - tree.start < tree.fall {
                continue;
            }
            self.fallen_trees.fallen[i].landed = true;
            let at = Vec2::new(tree.instance.pos[0], tree.instance.pos[1]);
            let dir = Vec2::from_angle(tree.instance.heading);
            let side = dir.perp();
            for k in 0..6 {
                let along = at + dir * tree.height * (0.35 + 0.12 * k as f32);
                let pos = along.extend(self.ground_height(along) + 0.4)
                    + side.extend(0.0) * self.scatter.signed() * tree.height * 0.12;
                let vel = side.extend(0.0) * self.scatter.signed() * 1.6
                    + dir.extend(0.0) * 0.8
                    + Vec3::Z * (0.8 + self.scatter.unit());
                let size = (tree.height * 0.08, tree.height * 0.22);
                let life = 1.6 + self.scatter.unit();
                self.push_puff(PUFF_DUST, pos, vel, time, life, size);
            }
        }
    }

    /// The fallen and vaporizing trees as they stand at `time`, posed for the vertex shader.
    pub(super) fn fallen_tree_instances(&self, time: f32) -> Vec<UnitInstance> {
        self.fallen_trees
            .fallen
            .iter()
            .map(|tree| {
                let age = (time - tree.start).max(0.0);
                // Slow to start, as a tree goes: it picks up speed as it leans.
                let f = (age / tree.fall).min(1.0);
                let mut tilt = LIE * f * f;
                // A small bounce on the branches as it lands.
                let bounce = (age - tree.fall) / 0.45;
                if (0.0..1.0).contains(&bounce) {
                    tilt -= 0.07 * (bounce * std::f32::consts::PI).sin();
                }
                let sink = ((age - LINGER) / SINK).clamp(0.0, 1.0) * tree.height * 0.4;
                let mut instance = tree.instance;
                // Negative pitch leans the top along the heading.
                let pitch = -tilt.max(1e-4);
                instance.arm_pitch = [pitch, pitch, sink, instance.arm_pitch[3]];
                instance
            })
            .chain(self.vapor_instances(time))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use glam::Vec2;

    /// A trampled tree is down by the time the walker's middle reaches its foot:
    /// a fast walker's trees fall quicker than a slow one's, within bounds.
    #[test]
    fn trampled_trees_fall_at_the_walkers_pace() {
        use super::{trample_fall, FALL, FALL_QUICKEST};
        let at = Vec2::new(8.0, 0.0);
        // The commander's 30 m/s: 3 m a tick, the foot 8 m off.
        let quick = trample_fall(at, Vec2::ZERO, Vec2::new(3.0, 0.0));
        // A titan's slow tread: 1 m a tick.
        let slow = trample_fall(at, Vec2::ZERO, Vec2::new(1.0, 0.0));
        assert!(quick < slow, "{quick} vs {slow}");
        assert!(
            quick <= 8.0 / 30.0 + 0.05,
            "down before the commander is on it: {quick}"
        );
        assert_eq!(trample_fall(at, Vec2::ZERO, Vec2::new(0.1, 0.0)), FALL);
        assert_eq!(
            trample_fall(at, Vec2::ZERO, Vec2::new(50.0, 0.0)),
            FALL_QUICKEST
        );
        assert_eq!(trample_fall(at, Vec2::ZERO, Vec2::ZERO), FALL);
    }

    /// Real Vulkan check: a walker's path knocks over a clump of trees.
    /// `FALLEN_TREES_DIR` gets a picture before, during and after the fall.
    #[test]
    #[ignore = "requires Vulkan and maps/dev16.mcmap"]
    fn trampled_trees_fall_and_lie_down() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let out = std::env::var("FALLEN_TREES_DIR")
            .map(std::path::PathBuf::from)
            .ok();
        let map = Arc::new(MapFile::open(root.join("maps/dev16.mcmap")).unwrap());
        let blueprints = Arc::new(Blueprints::load(&root.join("data")).unwrap());
        let start = Vec2::from(map.start_positions()[0].to_f32());
        let trees: Vec<(usize, Vec2)> = map
            .props()
            .iter()
            .enumerate()
            .filter(|(_, p)| p.kind.is_tree())
            .map(|(i, p)| (i, Vec2::from(p.pos.to_f32())))
            .collect();
        let first = trees
            .iter()
            .min_by(|a, b| {
                a.1.distance_squared(start)
                    .total_cmp(&b.1.distance_squared(start))
            })
            .unwrap()
            .1;
        // The walker comes from the east, out of the wood, so the trees fall into the open.
        let clump: Vec<_> = trees
            .iter()
            .filter(|t| t.1.distance(first) < 14.0)
            .copied()
            .collect();
        assert!(!clump.is_empty());
        let mut renderer = Renderer::new(
            Target::Headless {
                width: 960,
                height: 720,
            },
            SceneDesc {
                map: map.clone(),
                blueprints,
                pool: Arc::new(Pool::new(2)),
                team_colors: [[0.1, 0.6, 0.9]; mc_core::MAX_PLAYERS],
            },
        )
        .unwrap();
        let mut camera = Camera::new(
            Vec2::from(map.info().size_metres().to_f32()),
            Vec2::new(960.0, 720.0),
        );
        camera.focus = first.extend(renderer.ground_height(first) + 5.0);
        camera.distance = 70.0;
        camera.tilt = 0.35;
        let mut frame = RenderFrame {
            props_dead: vec![0; map.props().len().div_ceil(32)],
            ..Default::default()
        };
        let overlay = Overlay::default();
        let shoot =
            |renderer: &mut Renderer, frame: &RenderFrame, from: f32, to: f32, name: &str| {
                let mut time = from;
                while time <= to {
                    renderer
                        .render(&FrameInput {
                            camera: &camera,
                            time,
                            alpha: 1.0,
                            sim: Some(frame),
                            ghosts: &[],
                            marks: &[],
                            ranges: &[],
                            ranges_drawn: 0,
                            overlay: &overlay,
                            build_grid: false,
                            icons: true,
                        })
                        .unwrap();
                    // A few frames a stage: enough for the tile cache and effects to settle.
                    time += ((to - from) / 6.0).max(0.05);
                }
                if let Some(dir) = &out {
                    let pixels = renderer.read_pixels().unwrap();
                    let mut ppm = b"P6\n960 720\n255\n".to_vec();
                    for pixel in pixels.as_chunks::<4>().0 {
                        ppm.extend_from_slice(&pixel[..3]);
                    }
                    std::fs::create_dir_all(dir).unwrap();
                    std::fs::write(dir.join(format!("{name}.ppm")), ppm).unwrap();
                }
            };
        shoot(&mut renderer, &frame, 0.0, 1.0, "0-standing");
        for &(index, _) in &clump {
            frame.props_dead[index / 32] |= 1 << (index % 32);
            frame.events.push(SimEvent::TreeTrampled {
                prop: index as u32,
                from: mc_core::FxVec2::new(
                    mc_core::Fx::from_f32(first.x + 12.0),
                    mc_core::Fx::from_f32(first.y),
                ),
                motion: mc_core::FxVec2::from_ints(-2, 0),
            });
        }
        shoot(&mut renderer, &frame, 1.05, 1.6, "1-falling");
        assert_eq!(
            renderer.fallen_trees.fallen.len(),
            clump.len(),
            "one fall per tree, however often the tick is shown"
        );
        frame.events.clear();
        shoot(&mut renderer, &frame, 1.65, 4.0, "2-down");
        shoot(&mut renderer, &frame, 4.05, 36.0, "3-sinking");
        shoot(&mut renderer, &frame, 36.05, 41.0, "4-gone");
        assert!(
            renderer.fallen_trees.fallen.is_empty(),
            "fallen trees go in the end"
        );
    }
}
