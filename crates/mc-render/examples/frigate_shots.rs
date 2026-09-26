//! Resolute heavy frigate visual check: native GPU shots of the `aster_t3_frigate` model from the
//! RTS camera and close up, aloft (gear stowed) and on its lot (gear down), with the
//! Bastion alongside for scale in one of them, and nose down laying its spinal gun (`dive`).
//! cargo run --release -p mc-render --example frigate_shots -- maps/dev16.mcmap OUT [only-shot-names...]
//! Env: FRIGATE_W / FRIGATE_H (default 1600x1000).
use glam::{Vec2, Vec3};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::MapFile;
use mc_render::{Camera, FrameInput, Overlay, Renderer, SceneDesc, Target};
use mc_sim::mirror::{HousePose, RenderFrame, UnitInstance, UNIT_HOUSE_SHIFT};
use std::{f32::consts::PI, path::Path, sync::Arc};

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let map = Arc::new(MapFile::open(&args[0]).unwrap());
    let out = Path::new(&args[1]);
    let only: Vec<String> = args[2..].to_vec();
    std::fs::create_dir_all(out).unwrap();
    let w: u32 = std::env::var("FRIGATE_W").ok().and_then(|v| v.parse().ok()).unwrap_or(1600);
    let h: u32 = std::env::var("FRIGATE_H").ok().and_then(|v| v.parse().ok()).unwrap_or(1000);
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let blueprints = Arc::new(Blueprints::load(&root.join("data")).unwrap());
    let size = Vec2::from(map.info().size_metres().to_f32());
    let spot = Vec2::from(map.start_positions()[0].to_f32());
    let overlay = Overlay::default();
    let mut renderer = Renderer::new(
        Target::Headless { width: w, height: h },
        SceneDesc { map: map.clone(), blueprints: blueprints.clone(), pool: Arc::new(Pool::new(4)), team_colors: [[0.1, 0.45, 0.95]; 8] },
    )
    .unwrap();
    let ground = renderer.ground_height(spot);
    let make = |key: &str, pos: Vec3, heading: f32, unit_id: u32| {
        let id = blueprints.id_of(key).unwrap();
        let mut unit: UnitInstance = bytemuck::Zeroable::zeroed();
        unit.pos = pos.to_array();
        unit.prev_pos = unit.pos;
        unit.heading = heading;
        unit.prev_heading = heading;
        unit.blueprint = id.index() as u32;
        unit.health = 1.0;
        unit.build = 1.0;
        unit.radius = blueprints.unit(id).radius.to_f32();
        unit.unit_id = unit_id;
        unit
    };
    // Turrets at rest: fore dead ahead, aft astern, flanks outboard, flanks depressed.
    let mut pose = HousePose::default();
    let rest = [(0.0, 0.0), (0.0, -0.4), (PI, 0.0), (1.22, -0.6), (-1.22, -0.6)];
    for (w, (yaw, pitch)) in rest.into_iter().enumerate() {
        pose.pose[w] = [yaw, yaw, pitch, pitch];
    }
    // name, altitude over the ground, camera (dx, dy, dz focus offset), distance, tilt, yaw, bastion
    type Shot = (&'static str, f32, [f32; 3], f32, f32, f32, bool);
    let shots: &[Shot] = &[
        ("rts", 300.0, [0.0, 0.0, 40.0], 1400.0, 0.0, -0.5, false),
        ("rts-far", 300.0, [0.0, 0.0, 40.0], 2600.0, 0.0, 0.4, true),
        ("quarter-bow", 300.0, [30.0, 0.0, 50.0], 560.0, 0.55, -0.9, false),
        ("quarter-stern", 300.0, [-30.0, 0.0, 50.0], 560.0, 0.55, 2.4, false),
        ("side", 300.0, [0.0, 0.0, 45.0], 620.0, 1.05, 0.0, false),
        ("bow-close", 300.0, [150.0, 0.0, 50.0], 220.0, 0.6, -1.2, false),
        ("spine-close", 300.0, [-40.0, 0.0, 60.0], 260.0, 0.2, -2.6, false),
        ("belly", 300.0, [0.0, 0.0, 20.0], 520.0, 1.45, 0.8, false),
        ("lot", 0.0, [0.0, 0.0, 40.0], 700.0, 0.45, -0.7, false),
        ("dive", 300.0, [0.0, 0.0, 30.0], 760.0, 1.1, 0.35, false),
        ("drives", 300.0, [-165.0, 0.0, 42.0], 150.0, 1.15, 2.75, false),
        ("chin", 300.0, [100.0, 0.0, 20.0], 160.0, 1.4, -0.6, false),
    ];
    for &(name, alt, off, dist, tilt, yaw, bastion) in shots {
        if !only.is_empty() && !only.iter().any(|o| o == name) {
            continue;
        }
        let base = spot.extend(ground + alt);
        let mut frame = RenderFrame { props_dead: vec![u32::MAX; map.props().len().div_ceil(32)], ..Default::default() };
        let mut ship = make("aster_t3_frigate", base, 0.0, 1);
        if name == "dive" {
            // The hull pitched 20 degrees nose down (slot 0 of `arm_pitch`: previous, now).
            ship.arm_pitch = [-0.35, -0.35, 0.0, 0.0];
        }
        frame.houses.push(pose);
        ship._pad3[1] |= 1 << UNIT_HOUSE_SHIFT;
        frame.units.push(ship);
        if bastion {
            frame.units.push(make("aster_t2_lift_ship", base + Vec3::new(0.0, 330.0, -60.0), 0.0, 2));
        }
        frame.units.push(make("aster_t3_assault_bot", spot.extend(ground) + Vec3::new(0.0, -120.0, 0.0), 0.0, 3));
        let mut camera = Camera::new(size, Vec2::new(w as f32, h as f32));
        camera.focus = base + Vec3::from(off);
        camera.distance = dist;
        camera.tilt = tilt;
        camera.yaw = yaw;
        for i in 0..8 {
            renderer
                .render(&FrameInput {
                    camera: &camera,
                    time: 10.0 + i as f32 * 0.1,
                    alpha: 1.0,
                    sim: Some(&frame),
                    ghosts: &[],
                    marks: &[],
                    ranges: &[],
                    ranges_drawn: 0,
                    overlay: &overlay,
                    build_grid: false,
                })
                .unwrap();
        }
        let pixels = renderer.read_pixels().unwrap();
        let mut ppm = format!("P6\n{w} {h}\n255\n").into_bytes();
        for pixel in pixels.as_chunks::<4>().0 {
            ppm.extend_from_slice(&pixel[..3]);
        }
        std::fs::write(out.join(format!("frigate-{name}.ppm")), ppm).unwrap();
        println!("wrote frigate-{name}");
    }
}
