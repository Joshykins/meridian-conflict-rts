//! The front end's backdrop is the game itself: a scripted battle running in
//! the real simulation, filmed by this director. Each "scene" is a camera move
//! over that battle; cuts between them dip through black.

use glam::{Vec2, Vec3};
use mc_map::MapFile;
use mc_render::Camera;

pub struct SceneInfo {
    pub name: &'static str,
}

/// A close orbit of the fighting, the whole map from high up, a run along the line of advance.
pub const SCENES: [SceneInfo; 3] = [
    SceneInfo { name: "Front Line" },
    SceneInfo { name: "Theatre" },
    SceneInfo { name: "Low Pass" },
];

/// How long a scene plays before auto-advance moves on.
pub const SCENE_SECONDS: f32 = 45.0;
const DIP_SECONDS: f32 = 0.45;

pub struct Director {
    pub scene: usize,
    /// Seconds into the current scene.
    pub t: f32,
    pub paused: bool,
    pub auto_advance: bool,
    /// Scene waiting for the dip to black to complete.
    pending: Option<usize>,
    /// 0 = clear, 1 = black.
    dip: f32,
    /// Set for one update when a cut lands on the first scene: time to restage the battle.
    pub restage: bool,
    battle: Vec2,
    along: Vec2,
    centre: Vec2,
    size: Vec2,
}

/// Where the backdrop battle is staged on a map: the ore field nearest the
/// middle (contested ground on every layout), and the direction the two armies
/// face each other along, which runs around the middle rather than across it
/// so that a central lake or city is not between them.
pub fn battle_site(map: &MapFile) -> (Vec2, Vec2) {
    let size = Vec2::from(map.info().size_metres().to_f32());
    let centre = size * 0.5;
    let site = map
        .ore_regions()
        .iter()
        .map(|r| {
            let sum: Vec2 = r.points.iter().map(|p| Vec2::from(p.to_f32())).sum();
            sum / r.points.len().max(1) as f32
        })
        .min_by(|a, b| a.distance(centre).total_cmp(&b.distance(centre)))
        .unwrap_or(centre);
    let out = (site - centre).try_normalize().unwrap_or(Vec2::X);
    (site, out.perp())
}

impl Director {
    pub fn new(map: &MapFile, auto_advance: bool) -> Director {
        let size = Vec2::from(map.info().size_metres().to_f32());
        let (battle, along) = battle_site(map);
        // Start on black and fade up, like any other cut.
        Director {
            scene: 0,
            t: 0.0,
            paused: false,
            auto_advance,
            pending: None,
            dip: 1.0,
            restage: false,
            battle,
            along,
            centre: size * 0.5,
            size,
        }
    }

    pub fn go(&mut self, scene: usize) {
        if self.pending.is_none() && scene != self.scene {
            self.pending = Some(scene % SCENES.len());
        }
    }

    pub fn next(&mut self) {
        self.go((self.scene + 1) % SCENES.len());
    }

    pub fn previous(&mut self) {
        self.go((self.scene + SCENES.len() - 1) % SCENES.len());
    }

    /// Cuts back to the first scene with a fresh battle.
    pub fn restart(&mut self) {
        if self.pending.is_none() {
            self.pending = Some(0);
        }
    }

    pub fn update(&mut self, dt: f32) {
        self.restage = false;
        if !self.paused {
            self.t += dt;
        }
        if self.auto_advance && !self.paused && self.t >= SCENE_SECONDS {
            self.next();
        }
        match self.pending {
            Some(scene) => {
                self.dip = (self.dip + dt / DIP_SECONDS).min(1.0);
                if self.dip >= 1.0 {
                    self.restage = scene == 0;
                    self.scene = scene;
                    self.t = 0.0;
                    self.pending = None;
                }
            }
            None => self.dip = (self.dip - dt / DIP_SECONDS).max(0.0),
        }
    }

    /// How black the backdrop is right now, 0..1.
    pub fn dip(&self) -> f32 {
        // Ease both ends so the cut does not feel like a linear dissolve.
        self.dip * self.dip * (3.0 - 2.0 * self.dip)
    }

    pub fn progress(&self) -> f32 {
        (self.t / SCENE_SECONDS).clamp(0.0, 1.0)
    }

    /// Where a scene looks, as a 0..1 position on the map's square preview.
    pub fn marker(&self, scene: usize) -> Vec2 {
        let p = if scene == 1 { self.centre } else { self.battle };
        let longest = self.size.max_element();
        (Vec2::new(p.x, self.size.y - p.y) + (Vec2::splat(longest) - self.size) * 0.5) / longest
    }

    /// Ground position the camera is looking at.
    pub fn focus(&self) -> Vec2 {
        let k = self.progress();
        match self.scene {
            0 => self.battle,
            1 => self.centre,
            _ => self.battle + self.along * (k - 0.5) * 900.0,
        }
    }

    pub fn apply(&self, camera: &mut Camera) {
        let t = self.t;
        let focus = self.focus();
        // Distance, yaw, and how far below the horizon the camera looks. The
        // game's own camera never gets this low; a backdrop wants the skyline.
        let (distance, yaw, pitch) = match self.scene {
            // A slow orbit close to the fighting.
            0 => (360.0 - 50.0 * (t * 0.05).sin(), 0.6 + t * 0.035, 0.27),
            // High over the whole map, turning slowly.
            1 => (self.size.max_element() * 0.55, -0.4 + t * 0.012, 0.62),
            // Skimming along the line of advance.
            _ => (
                220.0,
                self.along.x.atan2(self.along.y) + 0.35 * (t * 0.04).sin(),
                0.22,
            ),
        };
        camera.focus = Vec3::new(focus.x, focus.y, camera.focus.z);
        camera.distance = distance.clamp(mc_render::camera::MIN_DISTANCE, camera.max_distance());
        camera.yaw = yaw;
        // `tilt` lowers the pitch in proportion to how close the camera is; measure that and solve.
        camera.tilt = 0.0;
        let level = camera.pitch();
        camera.tilt = 0.1;
        let per_tilt = (level - camera.pitch()) / 0.1;
        camera.tilt = if per_tilt > 1e-4 {
            (level - pitch) / per_tilt
        } else {
            0.0
        };
    }
}
