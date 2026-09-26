//! Behemoth visual check: native GPU shots of the `aster_t5_titan` model posed by hand:
//! standing, at points through its stride, the torso turned with the arms following, the
//! arms pitched, the rail cluster spun, and from the RTS camera.
//! cargo run --release -p mc-render --example titan_shots -- maps/dev16.mcmap OUT [only-shot-names...]
//! Env: TITAN_W / TITAN_H (default 1600x1000).
use glam::{Vec2, Vec3};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::MapFile;
use mc_render::{Camera, FrameInput, Overlay, Renderer, SceneDesc, Target};
use mc_sim::mirror::{HousePose, RenderFrame, StainInstance, UnitInstance, UNIT_HOUSE_SHIFT};
use std::{path::Path, sync::Arc};

/// name, camera (focus offset from the feet, distance, tilt, yaw), ground walked (m),
/// walking (m a tick), torso yaw, arm pitch, cluster turn (rad), time (s)
type Shot = (
    &'static str,
    [f32; 3],
    f32,
    f32,
    f32,
    f32,
    f32,
    f32,
    f32,
    f32,
    f32,
);

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let map = Arc::new(MapFile::open(&args[0]).unwrap());
    let out = Path::new(&args[1]);
    let only: Vec<String> = args[2..].to_vec();
    std::fs::create_dir_all(out).unwrap();
    let w: u32 = std::env::var("TITAN_W")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1600);
    let h: u32 = std::env::var("TITAN_H")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1000);
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let blueprints = Arc::new(Blueprints::load(&root.join("data")).unwrap());
    let size = Vec2::from(map.info().size_metres().to_f32());
    // TITAN_AT=dx,dy moves it off the pad (onto a slope, say).
    let at = std::env::var("TITAN_AT").ok().and_then(|v| {
        let p: Vec<f32> = v.split(',').filter_map(|x| x.parse().ok()).collect();
        (p.len() == 2).then(|| Vec2::new(p[0], p[1]))
    });
    let spot = Vec2::from(map.start_positions()[0].to_f32()) + at.unwrap_or(Vec2::new(60.0, 40.0));
    let overlay = Overlay::default();
    let mut renderer = Renderer::new(
        Target::Headless {
            width: w,
            height: h,
        },
        SceneDesc {
            map: map.clone(),
            blueprints: blueprints.clone(),
            pool: Arc::new(Pool::new(4)),
            team_colors: [[0.1, 0.45, 0.95]; 8],
        },
    )
    .unwrap();
    // TITAN_AT=slope: the steepest ground within a couple of kilometres, for the feet.
    let spot = if std::env::var("TITAN_AT").is_ok_and(|v| v == "slope") {
        let pad = Vec2::from(map.start_positions()[0].to_f32());
        let h = |p: Vec2| renderer.ground_height(p);
        let mut best = (0.0f32, spot);
        for i in -40..=40 {
            for j in -40..=40 {
                let p = pad + Vec2::new(i as f32, j as f32) * 50.0;
                let g = ((h(p + Vec2::X * 60.0) - h(p - Vec2::X * 60.0)).abs()
                    + (h(p + Vec2::Y * 60.0) - h(p - Vec2::Y * 60.0)).abs())
                    / 120.0;
                if g > best.0 && g < 0.25 && h(p) > 1.0 {
                    best = (g, p);
                }
            }
        }
        println!("slope {:.2} at {:?}", best.0, best.1);
        best.1
    } else {
        spot
    };
    let ground = renderer.ground_height(spot);
    let id = blueprints.id_of("aster_t5_titan").unwrap();
    let d = std::f32::consts::PI / 180.0;
    // The shots are framed for the model as authored (radius 40); the unit file may build it bigger.
    let k = blueprints.unit(id).radius.to_f32() / 40.0;
    let k_scale = k;
    let shots: &[Shot] = &[
        (
            "front",
            [0.0, 0.0, 50.0],
            330.0,
            0.55,
            -1.45,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            10.0,
        ),
        (
            "quarter",
            [0.0, 0.0, 50.0],
            330.0,
            0.45,
            -0.8,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            10.0,
        ),
        (
            "side",
            [0.0, 0.0, 50.0],
            330.0,
            0.6,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            10.0,
        ),
        (
            "back",
            [0.0, 0.0, 50.0],
            330.0,
            0.5,
            1.9,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            10.0,
        ),
        (
            "rts",
            [0.0, 0.0, 40.0],
            700.0,
            0.0,
            -0.6,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            10.0,
        ),
        (
            "gatling",
            [40.0, -30.0, 86.0],
            110.0,
            0.45,
            -0.5,
            0.0,
            0.0,
            0.0,
            0.0,
            0.4,
            10.0,
        ),
        (
            "bore",
            [40.0, 30.0, 86.0],
            120.0,
            0.45,
            -2.6,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            10.0,
        ),
        (
            "head",
            [8.0, 0.0, 106.0],
            90.0,
            0.3,
            -1.1,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            10.0,
        ),
        (
            "feet",
            [5.0, 0.0, 8.0],
            90.0,
            0.35,
            -0.8,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            10.0,
        ),
        (
            "turned",
            [0.0, 0.0, 55.0],
            340.0,
            0.55,
            0.4,
            0.0,
            0.0,
            50.0 * d,
            6.0 * d,
            0.0,
            10.0,
        ),
        (
            "pitched",
            [20.0, 0.0, 60.0],
            300.0,
            0.9,
            1.4,
            0.0,
            0.0,
            0.0,
            -10.0 * d,
            0.0,
            10.0,
        ),
        (
            "walk0",
            [0.0, 0.0, 50.0],
            330.0,
            0.9,
            0.0,
            0.0,
            1.1,
            0.0,
            0.0,
            0.0,
            10.0,
        ),
        (
            "walk1",
            [0.0, 0.0, 50.0],
            330.0,
            0.9,
            0.0,
            8.0,
            1.1,
            0.0,
            0.0,
            0.0,
            10.0,
        ),
        (
            "walk2",
            [0.0, 0.0, 50.0],
            330.0,
            0.9,
            0.0,
            16.0,
            1.1,
            0.0,
            0.0,
            0.0,
            10.0,
        ),
        (
            "walk3",
            [0.0, 0.0, 50.0],
            330.0,
            0.9,
            0.0,
            24.0,
            1.1,
            0.0,
            0.0,
            0.0,
            10.0,
        ),
        (
            "walk4",
            [0.0, 0.0, 50.0],
            330.0,
            0.9,
            0.0,
            32.0,
            1.1,
            0.0,
            0.0,
            0.0,
            10.0,
        ),
        (
            "walk5",
            [0.0, 0.0, 50.0],
            330.0,
            0.9,
            0.0,
            40.0,
            1.1,
            0.0,
            0.0,
            0.0,
            10.0,
        ),
        (
            "walk6",
            [0.0, 0.0, 50.0],
            330.0,
            0.9,
            0.0,
            48.0,
            1.1,
            0.0,
            0.0,
            0.0,
            10.0,
        ),
        (
            "walk7",
            [0.0, 0.0, 50.0],
            330.0,
            0.9,
            0.0,
            56.0,
            1.1,
            0.0,
            0.0,
            0.0,
            10.0,
        ),
        (
            "walkq",
            [0.0, 0.0, 50.0],
            330.0,
            0.5,
            0.6,
            20.0,
            1.1,
            0.0,
            0.0,
            0.0,
            10.0,
        ),
        (
            "scale",
            [0.0, -20.0, 30.0],
            300.0,
            0.6,
            -0.6,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            10.0,
        ),
        (
            "port",
            [10.0, -42.0, 90.0],
            110.0,
            0.15,
            0.9,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            10.0,
        ),
        (
            "portwide",
            [10.0, -40.0, 50.0],
            200.0,
            0.3,
            0.2,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            10.0,
        ),
        (
            "hi1",
            [0.0, 0.0, 70.0],
            300.0,
            0.25,
            -0.8,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            10.0,
        ),
        (
            "hi2",
            [0.0, 0.0, 70.0],
            300.0,
            0.2,
            2.4,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            10.0,
        ),
        (
            "top",
            [0.0, 0.0, 90.0],
            170.0,
            0.1,
            -1.2,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            10.0,
        ),
        (
            "topback",
            [-8.0, 0.0, 95.0],
            140.0,
            0.15,
            2.9,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            10.0,
        ),
        (
            "idle",
            [40.0, 30.0, 86.0],
            120.0,
            0.45,
            -2.6,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            10.0,
        ),
        (
            "charge25",
            [40.0, 30.0, 86.0],
            120.0,
            0.45,
            -2.6,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            10.0,
        ),
        (
            "charge60",
            [40.0, 30.0, 86.0],
            120.0,
            0.45,
            -2.6,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            10.0,
        ),
        (
            "charge95",
            [40.0, 30.0, 86.0],
            120.0,
            0.45,
            -2.6,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            10.0,
        ),
        (
            "blaze",
            [40.0, 30.0, 86.0],
            120.0,
            0.45,
            -2.6,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            10.0,
        ),
        (
            "spent",
            [40.0, 30.0, 86.0],
            120.0,
            0.45,
            -2.6,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            10.0,
        ),
        (
            "slope",
            [0.0, 0.0, 50.0],
            340.0,
            1.0,
            0.0,
            20.0,
            1.1,
            0.0,
            0.0,
            0.0,
            10.0,
        ),
        (
            "slopeq",
            [0.0, 0.0, 50.0],
            340.0,
            0.6,
            -0.8,
            20.0,
            1.1,
            0.0,
            0.0,
            0.0,
            10.0,
        ),
        (
            "slopef",
            [0.0, 0.0, 40.0],
            300.0,
            1.25,
            -1.57,
            20.0,
            1.1,
            0.0,
            0.0,
            0.0,
            10.0,
        ),
        (
            "slopes",
            [0.0, 0.0, 40.0],
            300.0,
            1.25,
            0.0,
            40.0,
            1.1,
            0.0,
            0.0,
            0.0,
            10.0,
        ),
        (
            "pods",
            [4.0, 18.0, 110.0],
            70.0,
            0.5,
            -2.2,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            10.0,
        ),
    ];
    for &(name, off, dist, tilt, yaw, walked, speed, torso, pitch, spin, time) in shots {
        if !only.is_empty() && !only.iter().any(|o| o == name) {
            continue;
        }
        let base = spot.extend(ground);
        let mut frame = RenderFrame {
            props_dead: vec![u32::MAX; map.props().len().div_ceil(32)],
            ..Default::default()
        };
        // A world with no scorch reads as a new one and drops its effects.
        frame.stains.push(StainInstance {
            pos: (spot + Vec2::new(0.0, 900.0)).to_array(),
            radius: 2.0,
            strength_seed: 40,
        });
        let mut unit: UnitInstance = bytemuck::Zeroable::zeroed();
        // The feet stay where they are: the body is placed back along its walk.
        unit.pos = base.to_array();
        unit.prev_pos = unit.pos;
        unit.blueprint = id.index() as u32;
        unit.health = 1.0;
        unit.build = 1.0;
        unit.radius = 40.0;
        unit.unit_id = 7;
        unit.gait = [walked * k, speed * k, speed * k];
        unit.turret_yaw = torso;
        unit.prev_turret_yaw = torso;
        unit.spin_recoil = [spin, spin, 0.0, 0.0];
        // The AEB's charge, as `renderer/titan_charge.rs` hands it over (the last frame is
        // drawn at `time + 0.14`).
        let now = time + 0.14;
        let never = -1.0e4;
        unit.mount = match name {
            "charge25" => [now - 1.5, now + 4.5, never, -1000.0],
            "charge60" => [now - 3.6, now + 2.4, never, -1000.0],
            "charge95" => [now - 5.7, now + 0.3, never, -1000.0],
            "blaze" => [now - 6.1, now - 0.1, now - 0.1, -1000.0],
            "spent" => [now - 8.5, now - 2.5, now - 2.5, -1000.0],
            _ => [never, never, never, -1000.0],
        };
        let mut pose = HousePose::default();
        // The pods and arms turn with the torso; the shoulder flak rest turned a little outboard of it.
        let rest = [torso, torso, torso, torso + 30.0 * d, torso - 30.0 * d];
        for (w, yaw) in rest.into_iter().enumerate() {
            // Weapon 0 is the pods on the torso; 1 and 2 the arms.
            let p = if w == 1 || w == 2 { pitch } else { 0.0 };
            pose.pose[w] = [yaw, yaw, p, p];
        }
        frame.houses.push(pose);
        unit._pad3[1] |= 1 << UNIT_HOUSE_SHIFT;
        frame.units.push(unit);
        if name.starts_with("port") {
            // Two spent cases on the ground under the chute, for their size.
            if let Some(case) = blueprints.id_of("aster_t5_titan_sabot") {
                for (k, (dx, dy, yaw)) in [(24.0f32, -52.0f32, 0.4f32), (-6.0, -60.0, 2.1)]
                    .into_iter()
                    .enumerate()
                {
                    let mut u: UnitInstance = bytemuck::Zeroable::zeroed();
                    let p = spot + Vec2::new(dx, dy) * k_scale;
                    u.pos = p.extend(renderer.ground_height(p)).to_array();
                    u.prev_pos = u.pos;
                    u.heading = yaw;
                    u.prev_heading = yaw;
                    u.blueprint = case.index() as u32;
                    u.health = 1.0;
                    u.build = 1.0;
                    u.radius = blueprints.unit(case).radius.to_f32();
                    u.unit_id = 20 + k as u32;
                    frame.units.push(u);
                }
            }
        }
        if name == "scale" {
            // The Fulgur (tech 4) alongside, and a Paladin, for scale.
            for (key, at, uid) in [
                ("aster_t4_assault_tank", Vec2::new(40.0, -60.0) * k, 8),
                ("aster_t3_assault_bot", Vec2::new(20.0, -30.0) * k, 9),
            ] {
                let other = blueprints.id_of(key).unwrap();
                let mut u: UnitInstance = bytemuck::Zeroable::zeroed();
                let p = spot + at;
                u.pos = p.extend(renderer.ground_height(p)).to_array();
                u.prev_pos = u.pos;
                u.blueprint = other.index() as u32;
                u.health = 1.0;
                u.build = 1.0;
                u.radius = blueprints.unit(other).radius.to_f32();
                u.unit_id = uid;
                frame.units.push(u);
            }
        }
        let mut camera = Camera::new(size, Vec2::new(w as f32, h as f32));
        camera.focus = base + Vec3::from(off) * k;
        camera.distance = dist * k;
        camera.tilt = tilt;
        camera.yaw = yaw;
        for i in 0..8 {
            renderer
                .render(&FrameInput {
                    camera: &camera,
                    time: time + i as f32 * 0.02,
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
        std::fs::write(out.join(format!("titan-{name}.ppm")), ppm).unwrap();
        println!("wrote titan-{name}");
    }
}
