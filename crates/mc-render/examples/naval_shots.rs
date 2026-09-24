//! Renders ships afloat with the real renderer and shaders, without a match: each
//! shot stands some hulls on the first open water the map has and films them.
//!
//!   cargo run --release -p mc-render --example naval_shots -- maps/twin_shoals.mcmap OUT_DIR \
//!       pike:aster_t1_frigate@0,0,30:60,0.6,20
//!
//! A shot is `name:ITEM[+ITEM...]:distance,tilt[,yaw[,dx,dy]]`, the camera on the
//! water over the first hull (or `dx, dy` metres off it). An item is
//! `KEY@dx,dy,heading[,health[,turret[,mount[,sink[,pitch,roll[,depth[,upgrade]]]]]]]`: metres off
//! the spot and degrees, health 0..1, the first turret's and the mount's yaw in degrees;
//! `sink` 1 mirrors it as a hull going down (`WRECK_SINKING`) with `pitch`, `roll`
//! (degrees) and `depth` (metres under the water); `sink` 2 a settled wreck. `upgrade`
//! (0..1) shows it that far through a refit to its next tier.
//! Frames are `OUT_DIR/name.ppm`; `SHOT_SIZE=WxH`, `SHOT_TIME` (seconds) set the rest.

use std::sync::Arc;

use glam::{Vec2, Vec3};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::MapFile;
use mc_render::{Camera, FrameInput, Overlay, Renderer, SceneDesc, Target};
use mc_sim::mirror::{RenderFrame, UnitInstance, KIND_WRECK, WRECK_SINKING};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [map_path, out, shots @ ..] = args.as_slice() else {
        eprintln!("usage: naval_shots MAP OUT_DIR name:KEY@dx,dy,heading[,...][+...]:dist,tilt[,yaw[,dx,dy]]...");
        std::process::exit(2);
    };
    let (w, h) = std::env::var("SHOT_SIZE")
        .ok()
        .and_then(|s| s.split_once('x').map(|(a, b)| (a.parse().unwrap(), b.parse().unwrap())))
        .unwrap_or((1280u32, 800u32));
    let time: f32 = std::env::var("SHOT_TIME").ok().and_then(|s| s.parse().ok()).unwrap_or(10.0);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let map = Arc::new(MapFile::open(map_path).expect("map"));
    std::fs::create_dir_all(out).unwrap();
    let blueprints = Arc::new(Blueprints::load(&root.join("data")).expect("blueprints"));
    let mut renderer = Renderer::new(
        Target::Headless { width: w, height: h },
        SceneDesc {
            map: map.clone(),
            blueprints: blueprints.clone(),
            pool: Arc::new(Pool::new(4)),
            team_colors: [[0.1, 0.45, 0.95], [0.9, 0.15, 0.1], [0.1, 0.45, 0.95], [0.1, 0.45, 0.95], [0.1, 0.45, 0.95], [0.1, 0.45, 0.95], [0.1, 0.45, 0.95], [0.1, 0.45, 0.95]],
        },
    )
    .expect("renderer");
    let water = map.info().water_level.to_f32();
    let size = Vec2::from(map.info().size_metres().to_f32());
    // Open water: deep enough, and every point 150 m around it wet too.
    let spot = {
        let wet = |p: Vec2| renderer.ground_height(p) < water - 4.0;
        let mut found = None;
        let mut y = size.y * 0.3;
        'scan: while y < size.y - 400.0 {
            let mut x = 400.0;
            while x < size.x - 400.0 {
                let p = Vec2::new(x, y);
                if renderer.ground_height(p) < water - 14.0
                    && (0..8).all(|i| {
                        let a = i as f32 * std::f32::consts::TAU / 8.0;
                        wet(p + Vec2::new(a.cos(), a.sin()) * 150.0)
                    })
                {
                    found = Some(p);
                    break 'scan;
                }
                x += 97.0;
            }
            y += 97.0;
        }
        found.expect("no open water on this map")
    };
    let overlay = Overlay::default();
    for shot in shots {
        let mut parts = shot.splitn(3, ':');
        let (name, placed, view) = (parts.next().unwrap(), parts.next().unwrap(), parts.next().unwrap());
        let mut frame = RenderFrame::default();
        frame.props_dead = vec![u32::MAX; map.props().len().div_ceil(32)];
        let mut focus = None;
        for (i, item) in placed.split('+').enumerate() {
            let (key, at) = item.split_once('@').expect("KEY@dx,dy,heading");
            let v: Vec<f32> = at.split(',').map(|p| p.parse().expect("number")).collect();
            let arg = |k: usize, or: f32| v.get(k).copied().unwrap_or(or);
            let xy = spot + Vec2::new(v[0], v[1]);
            let id = blueprints.id_of(key).unwrap_or_else(|| panic!("no blueprint {key}"));
            let bp = blueprints.unit(id);
            let sink = arg(6, 0.0) as u32;
            let z = if sink == 2 { renderer.ground_height(xy) } else { water - arg(9, 0.0) };
            let mut u: UnitInstance = bytemuck::Zeroable::zeroed();
            u.pos = [xy.x, xy.y, z];
            u.prev_pos = u.pos;
            u.heading = v[2].to_radians();
            u.prev_heading = u.heading;
            u.blueprint = id.index() as u32;
            u.health = arg(3, 1.0);
            u.build = 1.0;
            u.upgrade = arg(10, 0.0);
            u.radius = bp.radius.to_f32();
            u.unit_id = i as u32 + 7;
            u.turret_yaw = arg(4, 0.0).to_radians();
            u.prev_turret_yaw = u.turret_yaw;
            let mount = arg(5, 0.0).to_radians() - u.turret_yaw;
            u.mount = [mount, mount, 0.2, 0.2];
            if sink == 1 {
                u.owner_flags |= KIND_WRECK;
                u._pad = WRECK_SINKING;
                u.health = 0.3;
                let (pitch, roll) = (arg(7, 0.0).to_radians(), arg(8, 0.0).to_radians());
                u.arm_pitch = [pitch, pitch, 0.0, 0.0];
                u._pad2 = [roll, roll];
            } else if sink == 2 {
                u.owner_flags |= KIND_WRECK;
                u.health = 1.0;
                let roll = arg(8, 0.0).to_radians();
                u._pad2 = [roll, roll];
            }
            frame.units.push(u);
            focus.get_or_insert(Vec3::new(xy.x, xy.y, water));
        }
        let v: Vec<f32> = view.split(',').map(|p| p.parse().expect("number")).collect();
        let mut camera = Camera::new(size, Vec2::new(w as f32, h as f32));
        camera.focus = focus.unwrap() + Vec3::new(v.get(3).copied().unwrap_or(0.0), v.get(4).copied().unwrap_or(0.0), 0.0);
        camera.distance = v[0];
        camera.tilt = v[1];
        camera.yaw = v.get(2).copied().unwrap_or(0.0);
        for i in 0..8 {
            renderer
                .render(&FrameInput {
                    camera: &camera, time: time + i as f32 * 0.1, alpha: 1.0,
                    sim: (i == 0).then_some(&frame), ghosts: &[], marks: &[], ranges: &[], ranges_drawn: 0,
                    overlay: &overlay, build_grid: false,
                })
                .expect("render");
        }
        let pixels = renderer.read_pixels().expect("pixels");
        let mut ppm = format!("P6\n{w} {h}\n255\n").into_bytes();
        for pixel in pixels.chunks_exact(4) {
            ppm.extend_from_slice(&pixel[..3]);
        }
        let path = std::path::Path::new(out).join(format!("{name}.ppm"));
        std::fs::write(&path, ppm).unwrap();
        eprintln!("{}", path.display());
    }
}
