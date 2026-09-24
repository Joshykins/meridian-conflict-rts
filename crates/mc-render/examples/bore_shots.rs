//! Native GPU inspection of both Argon models and their timed discharge.
//! Run: cargo run --release -p mc-render --example bore_shots -- maps/dev16.mcmap artifacts/argon
//! Writes model closeups plus 40 frames per weapon (20 fps), including launch and decay.
use std::{path::Path, sync::Arc};
use glam::{Vec2, Vec3};
use mc_core::{Fx, FxVec3};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::MapFile;
use mc_render::{Camera, FrameInput, Overlay, Renderer, SceneDesc, Target};
use mc_sim::mirror::{RenderFrame, SimEvent, StainInstance, UnitInstance};

fn fixed(v: Vec3) -> FxVec3 {
    let f = |x: f32| Fx::ratio((x * 1000.0) as i64, 1000);
    FxVec3::new(f(v.x), f(v.y), f(v.z))
}
fn save(renderer: &mut Renderer, out: &Path, name: &str) {
    let pixels = renderer.read_pixels().expect("pixels");
    let mut ppm = b"P6\n1280 800\n255\n".to_vec();
    for pixel in pixels.chunks_exact(4) { ppm.extend_from_slice(&pixel[..3]); }
    std::fs::write(out.join(format!("{name}.ppm")), ppm).unwrap();
}
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let map = Arc::new(MapFile::open(&args[0]).unwrap());
    let out = Path::new(&args[1]);
    std::fs::create_dir_all(out).unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let blueprints = Arc::new(Blueprints::load(&root.join("data")).unwrap());
    let size = Vec2::from(map.info().size_metres().to_f32());
    let spot = Vec2::from(map.start_positions()[0].to_f32());
    let overlay = Overlay::default();
    for (key, name) in [("aster_t3_sniper", "arbalest"), ("aster_t4_assault_tank", "fulgur")] {
        let mut renderer = Renderer::new(Target::Headless { width: 1280, height: 800 }, SceneDesc {
            map: map.clone(), blueprints: blueprints.clone(), pool: Arc::new(Pool::new(4)),
            team_colors: [[0.1, 0.45, 0.95]; 8],
        }).unwrap();
        let id = blueprints.id_of(key).unwrap();
        let bp = blueprints.unit(id);
        let weapon = &bp.weapons[0];
        let ground = renderer.ground_height(spot);
        let base = spot.extend(ground);
        let mut unit: UnitInstance = bytemuck::Zeroable::zeroed();
        unit.pos = base.to_array(); unit.prev_pos = unit.pos;
        unit.blueprint = id.index() as u32;
        unit.health = 1.0; unit.build = 1.0; unit.radius = bp.radius.to_f32();
        unit.deploy = 1.0; unit.prev_deploy = 1.0; unit.unit_id = 1;
        unit._pad3[0] |= 255 << 16;
        let mut frame = RenderFrame::default();
        frame.props_dead = vec![u32::MAX; map.props().len().div_ceil(32)];
        frame.units.push(unit);
        let mut camera = Camera::new(size, Vec2::new(1280.0, 800.0));
        camera.focus = base + Vec3::Z * bp.height.to_f32() * 0.35;
        camera.distance = bp.radius.to_f32() * 4.3;
        camera.tilt = 0.65; camera.yaw = -0.8;
        for i in 0..8 {
            renderer.render(&FrameInput { camera: &camera, time: 10.0 + i as f32 * 0.1, alpha: 1.0,
                sim: Some(&frame), ghosts: &[], marks: &[], ranges: &[], ranges_drawn: 0,
                overlay: &overlay, build_grid: false }).unwrap();
        }
        save(&mut renderer, out, &format!("{name}-model"));
        for (pose, pitch) in [("uphill", 0.28), ("downhill", -0.18)] {
            frame.units[0].arm_pitch = [pitch, pitch, 0.0, 0.0];
            renderer.render(&FrameInput { camera: &camera, time: 10.9, alpha: 1.0,
                sim: Some(&frame), ghosts: &[], marks: &[], ranges: &[], ranges_drawn: 0,
                overlay: &overlay, build_grid: false }).unwrap();
            save(&mut renderer, out, &format!("{name}-{pose}"));
        }
        let original_focus = camera.focus;
        let original_distance = camera.distance;
        let original_yaw = camera.yaw;
        let pivot = Vec3::from(weapon.pivot.unwrap().to_f32());
        camera.focus = base + pivot;
        camera.distance = bp.radius.to_f32() * 1.45;
        camera.yaw = 0.8;
        for (pose, pitch, recoil) in [("socket-level", 0.0, 0.0), ("socket-up", 0.45, 1.0), ("socket-down", -0.3, 1.0)] {
            frame.units[0].arm_pitch = [pitch, pitch, 0.0, 0.0];
            frame.units[0].recoil = recoil;
            frame.units[0].prev_recoil = recoil;
            renderer.render(&FrameInput { camera: &camera, time: 10.9, alpha: 1.0,
                sim: Some(&frame), ghosts: &[], marks: &[], ranges: &[], ranges_drawn: 0,
                overlay: &overlay, build_grid: false }).unwrap();
            save(&mut renderer, out, &format!("{name}-{pose}"));
        }
        camera.focus = original_focus;
        camera.distance = original_distance;
        camera.yaw = original_yaw;
        frame.units[0].recoil = 0.0;
        frame.units[0].prev_recoil = 0.0;
        frame.units[0].arm_pitch = [0.0; 4];
        let from = base + Vec3::from(weapon.muzzle.to_f32());
        let target_xy = spot + Vec2::new(240.0, 0.0);
        let to = target_xy.extend(renderer.ground_height(target_xy) + 2.0);
        // Keep an impact stain in the mirror, as a live world does after the strike.
        frame.stains.push(StainInstance { pos: to.truncate().to_array(), radius: 5.0, strength_seed: 180 });
        camera.focus = (base + to) * 0.5;
        camera.distance = 350.0; camera.tilt = 0.65; camera.yaw = 0.0;
        for i in 0..40 {
            frame.events.clear();
            if i == 0 {
                frame.events.push(SimEvent::ShotFired { pos: fixed(from), vel: fixed((to-from).normalize() * 80.0),
                    travel: FxVec3::ZERO, color: weapon.color, owner: 0, blueprint: id, weapon: 0 });
            }
            if i == 8 {
                frame.events.push(SimEvent::Impact { pos: fixed(to), target_motion: FxVec3::ZERO,
                    splash: weapon.splash, color: weapon.color, after: Fx::ZERO,
                    on_unit: true, on_shield: false, blueprint: id, weapon: 0 });
                frame.events.push(SimEvent::BoreDischarge { from: fixed(from), to: fixed(to),
                    width: weapon.bore.unwrap().width, after: Fx::ZERO, owner: 0, blueprint: id, weapon: 0 });
            }
            renderer.render(&FrameInput { camera: &camera, time: 11.0 + i as f32 * 0.05, alpha: 1.0,
                sim: Some(&frame), ghosts: &[], marks: &[], ranges: &[], ranges_drawn: 0,
                overlay: &overlay, build_grid: false }).unwrap();
            save(&mut renderer, out, &format!("{name}-{i:02}"));
        }
        eprintln!("captured {name}");
    }
}
