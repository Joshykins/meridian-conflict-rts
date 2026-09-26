//! Craters big blasts leave in the ground: a glassed pool in the middle that glows
//! white, orange, then red in the cracks of its crust and cools to black glass over
//! minutes; a shallow bowl with a raised lip, read through its shading; ragged
//! charcoal and soot streaks thrown out to about the blast's radius, feathering into
//! scorched, dulled ground.
//!
//! Presentation only, and general: a crater is a centre, a radius (the blast's damage
//! radius), a heat (0 to 1) and a style, so other races' and other sizes' blasts can
//! leave their own. The list rides in scene set binding 29 (`CraterList` in
//! bindings.wgsl) and terrain.wgsl shades each one analytically, per pixel, only where
//! the pixel lies inside it. Nothing here is random per frame: every crater's look is
//! fixed by its seed and its age.

use super::Renderer;
use crate::camera::Camera;
use crate::gpu::{Buffer, Gpu, GpuError};
use ash::vk;
use bytemuck::{Pod, Zeroable};
use glam::{Vec2, Vec3};
use std::mem::size_of;

/// Craters kept, at most (mirrors `CraterList` in bindings.wgsl). Past that the oldest
/// cold one goes first.
pub(super) const MAX_CRATERS: usize = 48;

/// How a crater looks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub(super) enum CraterStyle {
    /// A nuclear blast: the middle fused to glass, molten at first.
    Glassed,
    /// A big conventional blast: bowl, lip and scorch, no glass.
    Blast,
}

impl CraterStyle {
    /// The share of the radius that melts into a pool.
    fn pool(self) -> f32 {
        match self {
            CraterStyle::Glassed => 0.19,
            CraterStyle::Blast => 0.0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuCrater {
    /// x, y, radius, the time it was made.
    at: [f32; 4],
    /// heat (0-1), seconds it takes to cool, pool share of the radius, seed.
    look: [f32; 4],
}

#[derive(Clone, Copy)]
struct Crater {
    gpu: GpuCrater,
}

impl Crater {
    fn start(&self) -> f32 {
        self.gpu.at[3]
    }
    fn cool(&self) -> f32 {
        self.gpu.look[1]
    }
    /// Still glowing at `time`.
    fn hot(&self, time: f32) -> bool {
        time - self.start() < self.cool() * 1.3
    }
}

pub(super) struct Craters {
    list: Vec<Crater>,
    buffer: Buffer,
    seed: u32,
    /// Craters in the GPU copy.
    uploaded: usize,
}

impl Craters {
    pub(super) fn new(gpu: &Gpu) -> Result<Craters, GpuError> {
        let buffer = gpu.host_buffer(
            (16 + MAX_CRATERS * size_of::<GpuCrater>()) as u64,
            vk::BufferUsageFlags::STORAGE_BUFFER,
        )?;
        buffer.write(0, &vec![0u8; buffer.size as usize]);
        Ok(Craters { list: Vec::new(), buffer, seed: 0x2545_f491, uploaded: 0 })
    }

    /// What the scene set's binding 29 reads.
    pub(super) fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    /// A new match: the ground is whole again.
    pub(super) fn clear(&mut self) {
        self.list.clear();
    }

    fn add(&mut self, at: Vec2, radius: f32, heat: f32, time: f32, style: CraterStyle) {
        if radius < 1.0 {
            return;
        }
        self.seed = self.seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        let seed = (self.seed >> 9) as f32 / (1u32 << 23) as f32;
        let heat = heat.clamp(0.0, 1.0);
        // A warhead's pool takes about six minutes to go black; a commander's four.
        let cool = if style == CraterStyle::Glassed { 120.0 + 240.0 * heat } else { 60.0 + 60.0 * heat };
        // Another burst on one already here (a salvo on one mark): the pool is heated
        // again and widens a little, instead of a copy laid over it that crowds older
        // craters out of the list.
        if let Some(c) = self.list.iter_mut().find(|c| {
            c.gpu.look[2] == style.pool()
                && Vec2::new(c.gpu.at[0], c.gpu.at[1]).distance(at) < 0.35 * radius.max(c.gpu.at[2])
        }) {
            let r = c.gpu.at[2].max(radius);
            c.gpu.at[2] = r.max((r * 1.04).min(radius * 1.5));
            c.gpu.at[3] = time;
            c.gpu.look[0] = c.gpu.look[0].max(heat);
            c.gpu.look[1] = c.gpu.look[1].max(cool);
            self.list.sort_by(|a, b| a.start().total_cmp(&b.start()));
            return;
        }
        let crater = Crater {
            gpu: GpuCrater {
                at: [at.x, at.y, radius, time],
                look: [heat, cool, style.pool(), seed * 997.0],
            },
        };
        if self.list.len() >= MAX_CRATERS {
            // The oldest cold one goes; if all still glow, the oldest.
            let drop = self
                .list
                .iter()
                .enumerate()
                .filter(|(_, c)| !c.hot(time))
                .min_by(|a, b| a.1.start().total_cmp(&b.1.start()))
                .or_else(|| self.list.iter().enumerate().min_by(|a, b| a.1.start().total_cmp(&b.1.start())))
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.list.remove(drop);
        }
        // Oldest first, so a newer crater is laid over an older one.
        self.list.push(crater);
        self.list.sort_by(|a, b| a.start().total_cmp(&b.start()));
    }

    /// Writes the list for this frame: only craters that could be on screen, so the
    /// terrain's per-pixel loop stays short.
    fn upload(&mut self, camera: &Camera) {
        let eye = camera.eye();
        let focus = camera.focus;
        // Generous: anything within the view's reach round the focus.
        let reach = (eye.distance(focus) * 4.0).max(6000.0);
        let mut gpu = [GpuCrater::zeroed(); MAX_CRATERS];
        let mut n = 0;
        for c in &self.list {
            let at = Vec2::new(c.gpu.at[0], c.gpu.at[1]);
            if at.distance(focus.truncate()) - c.gpu.at[2] * 1.4 > reach {
                continue;
            }
            gpu[n] = c.gpu;
            n += 1;
        }
        if n == 0 && self.uploaded == 0 {
            return;
        }
        let header = [n as u32, 0, 0, 0];
        self.buffer.write(0, bytemuck::bytes_of(&header));
        if n > 0 {
            self.buffer.write(16, bytemuck::cast_slice(&gpu[..n]));
        }
        self.uploaded = n;
    }
}

impl Renderer {
    /// Leaves a crater: the glassed kind a nuclear blast leaves. `at` its middle,
    /// `radius` the blast's damage radius (about 520 m for a warhead, 300 for a
    /// commander's reactor), `heat` how hot it burned (0-1: how bright the pool glows
    /// and how long it takes to cool), `time` when it went off (the renderer's clock).
    pub(super) fn add_crater(&mut self, at: Vec2, radius: f32, heat: f32, time: f32) {
        self.add_crater_styled(at, radius, heat, time, CraterStyle::Glassed);
    }

    /// `add_crater` with a style of its own.
    pub(super) fn add_crater_styled(&mut self, at: Vec2, radius: f32, heat: f32, time: f32, style: CraterStyle) {
        self.craters.add(at, radius, heat, time, style);
    }

    /// Sends this frame's craters to the GPU and lets the hot ones warm the country
    /// round them a little.
    pub(super) fn upload_craters(&mut self, time: f32, camera: &Camera) {
        self.craters.upload(camera);
        let water = self.map_info.water_level.to_f32();
        for i in 0..self.craters.list.len() {
            let c = self.craters.list[i].gpu;
            let (heat, cool, pool) = (c.look[0], c.look[1], c.look[2]);
            let age = time - c.at[3];
            if pool <= 0.0 || age < 0.0 || age > cool {
                continue;
            }
            let at = Vec2::new(c.at[0], c.at[1]);
            let ground = self.ground_height(at);
            if ground < water {
                continue;
            }
            // Follows the cracks' cooling in terrain.wgsl.
            let t = heat * (1.0 - age / cool).clamp(0.0, 1.0).powf(2.2);
            let r = c.at[2];
            let scale = (r / 520.0).powi(2);
            let color = Vec3::new(1.0, 0.3 + 0.3 * t, 0.06 + 0.1 * t) * 1.2e5 * t * t * scale;
            self.lights.lamp(Vec3::new(at.x, at.y, ground + r * 0.3), Vec3::NEG_Z, color * 2.0, r * 0.9, 180.0, 1.0);
        }
    }
}

#[cfg(test)]
mod shots {
    use crate::camera::Camera;
    use crate::overlay::Overlay;
    use crate::renderer::{FrameInput, Renderer, SceneDesc, Target};
    use glam::Vec2;
    use mc_sim::mirror::RenderFrame;
    use std::sync::Arc;

    /// Leaves craters headless and writes frames of them at several ages.
    /// `CRATER_AT` = `x,y[;x,y...]` (a crater at each), `CRATER_RADIUS` (520),
    /// `CRATER_HEAT` (1), `CRATER_TIMES` seconds after the blast, `CRATER_CAMS` =
    /// `dist,yaw,tilt;...` (radians), `CRATER_SIZE` = `w,h`, `CRATER_OUT` the folder.
    /// Prints the scene pass's median GPU time with and without the craters.
    #[test]
    #[ignore = "requires Vulkan and maps/dev16.mcmap"]
    fn crater_shots() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let map = Arc::new(mc_map::MapFile::open(root.join(std::env::var("CRATER_MAP").unwrap_or_else(|_| "maps/dev16.mcmap".into()))).unwrap());
        let blueprints = Arc::new(mc_data::Blueprints::load(&root.join("data")).unwrap());
        let env = |key: &str, def: &str| std::env::var(key).unwrap_or_else(|_| def.into());
        let nums = |s: &str| -> Vec<f32> { s.split(',').map(|v| v.trim().parse().unwrap()).collect() };
        let size = nums(&env("CRATER_SIZE", "1280,720"));
        let (w, h) = (size[0] as u32, size[1] as u32);
        let mut renderer = Renderer::new(
            Target::Headless { width: w, height: h },
            SceneDesc { map: map.clone(), blueprints, pool: Arc::new(mc_jobs::Pool::new(2)), team_colors: [[0.1, 0.6, 0.9]; 8] },
        )
        .unwrap();
        // `startN[+dx,dy]`: the map's start position N, moved by dx, dy.
        let spots: Vec<Vec2> = env("CRATER_AT", "3825,5925").split(';').map(|s| {
            if let Some(rest) = s.strip_prefix("start") {
                let (i, off) = rest.split_once('+').unwrap_or((rest, "0,0"));
                let v = nums(off);
                return Vec2::from(map.start_positions()[i.parse::<usize>().unwrap()].to_f32()) + Vec2::new(v[0], v[1]);
            }
            let v = nums(s);
            Vec2::new(v[0], v[1])
        }).collect();
        println!("craters at {spots:?}");
        let radius = nums(&env("CRATER_RADIUS", "520"))[0];
        let heat = nums(&env("CRATER_HEAT", "1"))[0];
        let times = nums(&env("CRATER_TIMES", "1,20,60,120,180,300,400"));
        let cams: Vec<Vec<f32>> = env("CRATER_CAMS", "1300,0.6,0.2;4000,0.6,0.0").split(';').map(nums).collect();
        let out = std::path::PathBuf::from(env("CRATER_OUT", &root.join("artifacts/crater").display().to_string()));
        std::fs::create_dir_all(&out).unwrap();
        if std::env::var("CRATER_CLOUDS").is_err() {
            renderer.set_weather(mc_data::weather::WeatherPreset::Clear.into());
        }
        let mut frame = RenderFrame::default();
        frame.props_dead = vec![0; map.props().len().div_ceil(32)];
        // The blast took the trees.
        for (i, p) in map.props().iter().enumerate() {
            let pos = Vec2::from(p.pos.to_f32());
            if p.kind.is_tree() && spots.iter().any(|s| s.distance(pos) < radius * 1.3) {
                frame.props_dead[i / 32] |= 1 << (i % 32);
            }
        }
        // A stain far off, so the renderer does not take this for a fresh world.
        frame.stains.push(mc_sim::mirror::StainInstance { pos: [0.0, 0.0], radius: 1.0, strength_seed: 0 });
        let overlay = Overlay::default();
        let start = 100.0;
        let mut first = true;
        let mut render = |renderer: &mut Renderer, camera: &Camera, t: f32, first: &mut bool| {
            renderer
                .render(&FrameInput {
                    camera,
                    time: t,
                    alpha: 1.0,
                    sim: (*first).then_some(&frame),
                    ghosts: &[],
                    marks: &[],
                    ranges: &[],
                    ranges_drawn: 0,
                    overlay: &overlay,
                    build_grid: false,
                })
                .unwrap();
            *first = false;
        };
        let write = |renderer: &mut Renderer, name: String| {
            let pixels = renderer.read_pixels().unwrap();
            let mut ppm = format!("P6\n{w} {h}\n255\n").into_bytes();
            for p in pixels.chunks_exact(4) {
                ppm.extend_from_slice(&p[..3]);
            }
            std::fs::write(out.join(&name), ppm).unwrap();
            println!("wrote {name}");
        };
        let mut camera = Camera::new(Vec2::from(map.info().size_metres().to_f32()), Vec2::new(w as f32, h as f32));
        let at = spots[0];
        let median = |renderer: &mut Renderer, camera: &Camera, t: f32, first: &mut bool, render: &mut dyn FnMut(&mut Renderer, &Camera, f32, &mut bool)| {
            let mut scene = Vec::new();
            for k in 0..40 {
                render(renderer, camera, t + k as f32 * 0.016, first);
                if let Some(&(_, ms)) = renderer.stats.gpu_passes.iter().find(|p| p.0 == "scene") {
                    if k >= 5 {
                        scene.push(ms);
                    }
                }
            }
            scene.sort_by(f32::total_cmp);
            scene.get(scene.len() / 2).copied().unwrap_or(0.0)
        };
        // Cost first, with the ground bare.
        camera.focus = at.extend(renderer.ground_height(at));
        camera.distance = cams[0][0];
        camera.yaw = cams[0][1];
        camera.tilt = cams[0][2];
        let bare = median(&mut renderer, &camera, start - 2.0, &mut first, &mut render);
        for &spot in &spots {
            renderer.add_crater(spot, radius, heat, start);
        }
        let with = median(&mut renderer, &camera, start + 60.0, &mut first, &mut render);
        println!("scene pass median: {bare:.3} ms bare, {with:.3} ms with {} crater(s)", spots.len());
        for (ci, cam) in cams.iter().enumerate() {
            camera.focus = at.extend(renderer.ground_height(at));
            camera.distance = cam[0];
            camera.yaw = cam[1];
            camera.tilt = cam[2];
            for &age in &times {
                for k in 0..3 {
                    render(&mut renderer, &camera, start + age + k as f32 * 0.016, &mut first);
                }
                write(&mut renderer, format!("crater_c{ci}_{age:05.0}.ppm"));
            }
        }
    }
}
