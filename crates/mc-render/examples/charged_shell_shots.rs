//! Native GPU look at the Leviathan's charged shells landing (`Weapon::discharge`).
//! Run: cargo run --release -p mc-render --example charged_shell_shots -- maps/dev16.mcmap OUT_DIR
//! Writes 24 frames (20 fps) of a three-shell salvo landing on open ground.
use std::{path::Path, sync::Arc};
use glam::{Vec2, Vec3};
use mc_core::{Fx, FxVec3};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::MapFile;
use mc_render::{Camera, FrameInput, Overlay, Renderer, SceneDesc, Target};
use mc_sim::mirror::{RenderFrame, ShieldInstance, SimEvent, StainInstance, UnitInstance};

fn fixed(v: Vec3) -> FxVec3 {
    let f = |x: f32| Fx::ratio((x * 1000.0) as i64, 1000);
    FxVec3::new(f(v.x), f(v.y), f(v.z))
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let map = Arc::new(MapFile::open(&args[0]).unwrap());
    let out = Path::new(&args[1]);
    std::fs::create_dir_all(out).unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let blueprints = Arc::new(Blueprints::load(&root.join("data")).unwrap());
    let size = Vec2::from(map.info().size_metres().to_f32());
    let spot = Vec2::from(map.start_positions()[0].to_f32()) + Vec2::new(200.0, 0.0);
    let overlay = Overlay::default();
    let mut renderer = Renderer::new(Target::Headless { width: 1280, height: 800 }, SceneDesc {
        map: map.clone(), blueprints: blueprints.clone(), pool: Arc::new(Pool::new(4)),
        team_colors: [[0.1, 0.45, 0.95]; 8],
    }).unwrap();
    let id = blueprints.id_of("aster_t3_battleship").unwrap();
    let weapon = &blueprints.unit(id).weapons[0];
    // The Leviathan fires plain shells now; the effect is still drawn at its old reach.
    let discharge = if weapon.discharge > 0.0 { weapon.discharge } else { 40.0 };
    let mut frame = RenderFrame { props_dead: vec![0; map.props().len().div_ceil(32)], ..Default::default() };
    // A world with no scorch at all reads as a new one, and the renderer drops its lightning.
    frame.stains.push(StainInstance { pos: (spot + Vec2::new(0.0, 300.0)).to_array(), radius: 2.0, strength_seed: 40 });
    let mut camera = Camera::new(size, Vec2::new(1280.0, 800.0));
    camera.focus = spot.extend(renderer.ground_height(spot));
    camera.distance = 170.0;
    camera.tilt = 0.45;
    camera.yaw = 0.6;
    // Coming in from the west, falling at about 40 degrees.
    let incoming = Vec3::new(1.0, 0.1, -0.85).normalize();
    for i in 0..24 {
        frame.events.clear();
        if i == 2 {
            for (k, off) in [Vec2::new(-14.0, -10.0), Vec2::new(0.0, 4.0), Vec2::new(16.0, 12.0)].into_iter().enumerate() {
                let xy = spot + off;
                let to = xy.extend(renderer.ground_height(xy));
                let after = Fx::ratio(k as i64, 4);
                frame.events.push(SimEvent::Impact { pos: fixed(to), target_motion: FxVec3::ZERO,
                    splash: weapon.splash, color: weapon.color, after,
                    on_unit: false, on_shield: false, blueprint: id, weapon: 0 });
                frame.events.push(SimEvent::ShellDischarge { from: fixed(to - incoming * discharge),
                    to: fixed(to), after, blueprint: id, weapon: 0 });
            }
        }
        renderer.render(&FrameInput { camera: &camera, time: 11.0 + i as f32 * 0.05, alpha: 1.0,
            sim: Some(&frame), ghosts: &[], marks: &[], ranges: &[], ranges_drawn: 0,
            overlay: &overlay, build_grid: false }).unwrap();
        let pixels = renderer.read_pixels().expect("pixels");
        let mut ppm = b"P6\n1280 800\n255\n".to_vec();
        for pixel in pixels.as_chunks::<4>().0 { ppm.extend_from_slice(&pixel[..3]); }
        std::fs::write(out.join(format!("charged-{i:02}.ppm")), ppm).unwrap();
    }
    eprintln!("captured charged shells");

    // A tech 2 shield dome and a Paladin's hull field in the faction's shield colour,
    // with a few hits on the dome.
    let dome_id = blueprints.id_of("aster_t2_shield").unwrap();
    let dome_bp = blueprints.unit(dome_id);
    let walker_id = blueprints.id_of("aster_t3_assault_bot").unwrap();
    let walker_bp = blueprints.unit(walker_id);
    let place = |renderer: &Renderer, xy: Vec2, id: mc_data::BlueprintId, bp: &mc_data::UnitBlueprint, uid: u32| {
        let mut u: UnitInstance = bytemuck::Zeroable::zeroed();
        u.pos = xy.extend(renderer.ground_height(xy)).to_array();
        u.prev_pos = u.pos;
        u.blueprint = id.index() as u32;
        u.health = 1.0; u.build = 1.0; u.radius = bp.radius.to_f32();
        u.deploy = 1.0; u.prev_deploy = 1.0; u.unit_id = uid;
        u
    };
    let dome_xy = spot + Vec2::new(0.0, -260.0);
    let walker_xy = dome_xy + Vec2::new(150.0, -30.0);
    frame.events.clear();
    frame.units = vec![place(&renderer, dome_xy, dome_id, dome_bp, 7), place(&renderer, walker_xy, walker_id, walker_bp, 8)];
    let dome_r = dome_bp.shield.unwrap().radius.to_f32();
    frame.shields = vec![
        ShieldInstance {
            pos: frame.units[0].pos, radius: dome_r, prev_open: 1.0, open: 1.0, health: 0.8,
            packed: 2 << 16, unit_id: 7, projector: 30.0, height: dome_bp.height.to_f32(), prev_radius: 0.0,
        },
        ShieldInstance {
            pos: frame.units[1].pos, radius: walker_bp.radius.to_f32() * 1.2, prev_open: 1.0, open: 1.0, health: 1.0,
            packed: 3 << 16 | 1 << 25, unit_id: 8, projector: 0.0, height: walker_bp.height.to_f32(), prev_radius: 0.0,
        },
    ];
    let centre = Vec3::from(frame.units[0].pos);
    camera.focus = centre + Vec3::new(60.0, -20.0, 30.0);
    camera.distance = 420.0;
    camera.tilt = 0.35;
    camera.yaw = 0.5;
    for i in 0..12 {
        frame.events.clear();
        if i % 3 == 1 {
            let a = i as f32 * 0.9;
            let n = Vec3::new(a.cos() * 0.8, a.sin() * 0.8, 0.6).normalize();
            frame.events.push(SimEvent::Impact { pos: fixed(centre + n * dome_r), target_motion: FxVec3::ZERO,
                splash: weapon.splash, color: weapon.color, after: Fx::ZERO,
                on_unit: false, on_shield: true, blueprint: id, weapon: 0 });
        }
        renderer.render(&FrameInput { camera: &camera, time: 20.0 + i as f32 * 0.05, alpha: 1.0,
            sim: Some(&frame), ghosts: &[], marks: &[], ranges: &[], ranges_drawn: 0,
            overlay: &overlay, build_grid: false }).unwrap();
        let pixels = renderer.read_pixels().expect("pixels");
        let mut ppm = b"P6\n1280 800\n255\n".to_vec();
        for pixel in pixels.as_chunks::<4>().0 { ppm.extend_from_slice(&pixel[..3]); }
        std::fs::write(out.join(format!("shield-{i:02}.ppm")), ppm).unwrap();
    }
    eprintln!("captured shields");
}
