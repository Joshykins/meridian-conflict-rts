//! Native GPU close-ups of units, and one shot from each.
//! Run: cargo run --release -p mc-render --example unit_closeups -- maps/dev16.mcmap OUT_DIR KEY [KEY..]
//! Per unit: `KEY-front.ppm` and `KEY-back.ppm`, then `KEY-shot-NN.ppm`, 12 frames
//! (20 fps) of weapon 0 firing at the ground 120 m ahead.
use glam::{Vec2, Vec3};
use mc_core::{Fx, FxVec3};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::MapFile;
use mc_render::{Camera, FrameInput, Overlay, Renderer, SceneDesc, Target};
use mc_sim::mirror::{RenderFrame, ShieldInstance, SimEvent, StainInstance, UnitInstance};
use std::{path::Path, sync::Arc};

const W: u32 = 1280;
const H: u32 = 800;

fn fixed(v: Vec3) -> FxVec3 {
    let f = |x: f32| Fx::ratio((x * 1000.0) as i64, 1000);
    FxVec3::new(f(v.x), f(v.y), f(v.z))
}

fn save(renderer: &mut Renderer, out: &Path, name: &str) {
    let pixels = renderer.read_pixels().expect("pixels");
    let mut ppm = format!("P6\n{W} {H}\n255\n").into_bytes();
    for pixel in pixels.as_chunks::<4>().0 {
        ppm.extend_from_slice(&pixel[..3]);
    }
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
    for key in &args[2..] {
        let mut renderer = Renderer::new(
            Target::Headless {
                width: W,
                height: H,
            },
            SceneDesc {
                map: map.clone(),
                blueprints: blueprints.clone(),
                pool: Arc::new(Pool::new(4)),
                team_colors: [[0.1, 0.45, 0.95]; mc_core::MAX_PLAYERS],
            },
        )
        .unwrap();
        let id = blueprints
            .id_of(key)
            .unwrap_or_else(|| panic!("no unit {key}"));
        let bp = blueprints.unit(id);
        let base = spot.extend(renderer.ground_height(spot));
        let mut unit: UnitInstance = bytemuck::Zeroable::zeroed();
        unit.pos = base.to_array();
        unit.prev_pos = unit.pos;
        unit.blueprint = id.index() as u32;
        unit.health = 1.0;
        unit.build = 1.0;
        unit.radius = bp.radius.to_f32();
        unit.deploy = 1.0;
        unit.prev_deploy = 1.0;
        unit.unit_id = 1;
        let mut frame = RenderFrame {
            props_dead: vec![u32::MAX; map.props().len().div_ceil(32)],
            ..Default::default()
        };
        // A world with no scorch reads as a new one and drops its effects.
        frame.stains.push(StainInstance {
            pos: (spot + Vec2::new(0.0, 400.0)).to_array(),
            radius: 2.0,
            strength_seed: 40,
        });
        frame.units.push(unit);
        // Its shield up, as the sim mirrors one (mirror.rs).
        if let Some(spec) = bp.shield {
            frame.shields.push(ShieldInstance {
                pos: unit.pos,
                radius: spec.radius.to_f32(),
                prev_open: 1.0,
                open: 1.0,
                health: 1.0,
                packed: (bp.tech as u32) << 16 | u32::from(spec.is_hull()) << 25,
                unit_id: 1,
                projector: if spec.is_hull() {
                    0.0
                } else {
                    mc_data::SHIELD_PROJECTOR_HEIGHT
                },
                height: bp.height.to_f32() + mc_data::HULL_SHIELD_PAD as f32,
                prev_radius: 0.0,
            });
        }
        let mut camera = Camera::new(size, Vec2::new(W as f32, H as f32));
        camera.focus = base + Vec3::Z * bp.height.to_f32() * 0.5;
        camera.distance = bp.radius.to_f32().max(bp.height.to_f32() * 0.5) * 3.4;
        camera.tilt = 0.35;
        let render = |renderer: &mut Renderer, camera: &Camera, frame: &RenderFrame, time: f32| {
            renderer
                .render(&FrameInput {
                    camera,
                    time,
                    alpha: 1.0,
                    sim: Some(frame),
                    ghosts: &[],
                    marks: &[],
                    ranges: &[],
                    ranges_drawn: 0,
                    overlay: &overlay,
                    build_grid: false,
                })
                .unwrap();
        };
        for (name, yaw) in [("front", -0.7), ("back", 2.4)] {
            camera.yaw = yaw;
            for i in 0..6 {
                render(&mut renderer, &camera, &frame, 10.0 + i as f32 * 0.1);
            }
            save(&mut renderer, out, &format!("{key}-{name}"));
        }
        let Some(weapon) = bp.weapons.first() else {
            continue;
        };
        let muzzle = base + Vec3::from(weapon.muzzle.to_f32());
        let target_xy = spot + Vec2::new(120.0, 0.0);
        let to = target_xy.extend(renderer.ground_height(target_xy) + 1.0);
        camera.focus = (muzzle + to) * 0.5;
        camera.distance = 160.0;
        // SHOT_MUZZLE=DIST frames the muzzle from DIST metres instead.
        if let Some(d) = std::env::var("SHOT_MUZZLE")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
        {
            camera.focus = muzzle;
            camera.distance = d;
        }
        camera.yaw = -1.2;
        camera.tilt = 0.4;
        // Metres a tick, as the sim moves a shot.
        let step = weapon.projectile_speed.to_f32().max(1.0);
        // A hitscan shot lands on the tick it is fired.
        let flight = if weapon.hitscan { 0 } else { 4 };
        for i in 0..12 {
            frame.events.clear();
            if i == 1 {
                frame.events.push(SimEvent::ShotFired {
                    pos: fixed(muzzle),
                    vel: fixed((to - muzzle).normalize() * step),
                    travel: FxVec3::ZERO,
                    color: weapon.color,
                    owner: 0,
                    blueprint: id,
                    weapon: 0,
                });
            }
            if i == 1 + flight {
                frame.events.push(SimEvent::Impact {
                    pos: fixed(to),
                    target_motion: FxVec3::ZERO,
                    splash: weapon.splash,
                    color: weapon.color,
                    after: Fx::ZERO,
                    on_unit: false,
                    on_shield: false,
                    blueprint: id,
                    weapon: 0,
                });
            }
            render(&mut renderer, &camera, &frame, 11.0 + i as f32 * 0.05);
            save(&mut renderer, out, &format!("{key}-shot-{i:02}"));
        }
        eprintln!("captured {key}");
    }
}
