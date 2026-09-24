//! Renders structures with the real renderer and shaders, without a match:
//! each shot places some blueprints on the map and films them from the RTS camera.
//!
//!   cargo run --release -p mc-render --example structure_shots -- maps/twin_shoals.mcmap OUT_DIR \
//!       forge:aster_t1_land_factory@7000,7000,0:120,0.2,0.6
//!
//! A shot is `name:KEY@x,y,heading[,altitude[,ramp]][+KEY@x,y,heading...]:distance,tilt[,yaw[,dx,dy]]`; the
//! camera looks at the first structure, or `dx, dy` metres off it. `x` of `water` puts the structure on the first open water
//! the scan finds (at least 14 m deep and 150 m from the shore), `land` the first flat
//! dry ground; the second number is then an x offset from it. Frames are `OUT_DIR/name.ppm`.

use std::sync::Arc;

use glam::{Vec2, Vec3};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::MapFile;
use mc_render::{Camera, FrameInput, Overlay, Renderer, SceneDesc, Target};
use mc_sim::mirror::{RenderFrame, UnitInstance};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [map_path, out, shots @ ..] = args.as_slice() else {
        eprintln!("usage: structure_shots MAP OUT_DIR name:KEY@x,y,heading[+...]:dist,tilt[,yaw,dx,dy,dz]...");
        std::process::exit(2);
    };
    let (w, h) = std::env::var("SHOT_SIZE")
        .ok()
        .and_then(|s| s.split_once('x').map(|(a, b)| (a.parse().unwrap(), b.parse().unwrap())))
        .unwrap_or((960u32, 640u32));
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
            team_colors: [[0.1, 0.45, 0.95]; 8],
        },
    )
    .expect("renderer");
    // Optional cloud-heavy presentation for atmospheric hull inspection.
    if std::env::var("SHOT_OVERCAST").is_ok() {
        let mut weather: mc_data::weather::Weather = mc_data::weather::WeatherPreset::Overcast.into();
        weather.rain = 0.0;
        renderer.set_weather(weather);
    }
    let water = map.info().water_level.to_f32();
    let size = Vec2::from(map.info().size_metres().to_f32());
    // Open water: deep enough, and every point 150 m around it wet too.
    let open_water = || {
        let wet = |p: Vec2| renderer.ground_height(p) < water - 4.0;
        let mut y = size.y * 0.3;
        while y < size.y - 400.0 {
            let mut x = 400.0;
            while x < size.x - 400.0 {
                let p = Vec2::new(x, y);
                if renderer.ground_height(p) < water - 14.0
                    && (0..8).all(|i| {
                        let a = i as f32 * std::f32::consts::TAU / 8.0;
                        wet(p + Vec2::new(a.cos(), a.sin()) * 150.0)
                    })
                {
                    return Some(p);
                }
                x += 97.0;
            }
            y += 97.0;
        }
        None
    };
    // Flat dry ground: 60 m around it within a metre and a half of it.
    let flat_land = || {
        let mut y = 400.0;
        while y < size.y - 400.0 {
            let mut x = 400.0;
            while x < size.x - 400.0 {
                let p = Vec2::new(x, y);
                let g = renderer.ground_height(p);
                if g > water + 4.0
                    && (0..8).all(|i| {
                        let a = i as f32 * std::f32::consts::TAU / 8.0;
                        (renderer.ground_height(p + Vec2::new(a.cos(), a.sin()) * 60.0) - g).abs() < 1.5
                    })
                {
                    return Some(p);
                }
                x += 97.0;
            }
            y += 97.0;
        }
        None
    };
    let (open_water, flat_land) = (open_water(), flat_land());
    let overlay = Overlay::default();
    for shot in shots {
        let mut parts = shot.splitn(3, ':');
        let (name, placed, view) = (parts.next().unwrap(), parts.next().unwrap(), parts.next().unwrap());
        let mut frame = RenderFrame::default();
        // Props hidden: the structure is what is being looked at.
        frame.props_dead = vec![u32::MAX; map.props().len().div_ceil(32)];
        let mut focus = None;
        for (i, item) in placed.split('+').enumerate() {
            let (key, at) = item.split_once('@').expect("KEY@x,y,heading");
            let v: Vec<&str> = at.split(',').collect();
            let xy = if v[0] == "water" {
                open_water.expect("no open water on this map") + Vec2::new(v[1].parse().unwrap(), 0.0)
            } else if v[0] == "land" {
                flat_land.expect("no flat land on this map") + Vec2::new(v[1].parse().unwrap(), 0.0)
            } else {
                Vec2::new(v[0].parse().unwrap(), v[1].parse().unwrap())
            };
            let heading: f32 = v[2].parse::<f32>().unwrap().to_radians();
            let id = blueprints.id_of(key).unwrap_or_else(|| panic!("no blueprint {key}"));
            let bp = blueprints.unit(id);
            let ground = renderer.ground_height(xy);
            // A hull rides the water; its origin is the waterline.
            let naval = bp.motion.is_some_and(|m| m.layer == mc_data::MoveLayer::Naval);
            let z = if bp.water_build || naval { ground.max(water) } else { ground };
            let mut u: UnitInstance = bytemuck::Zeroable::zeroed();
            let altitude: f32 = v.get(3).map(|s| s.parse().unwrap()).unwrap_or(0.0);
            u.pos = [xy.x, xy.y, z + altitude];
            u.deploy = v.get(4).map(|s| s.parse().unwrap()).unwrap_or(0.0);
            u.prev_deploy = u.deploy;
            if altitude == 0.0 { u._pad3[0] |= 255 << 16; }
            u.prev_pos = u.pos;
            u.heading = heading;
            u.prev_heading = heading;
            u.blueprint = id.index() as u32;
            u.health = 1.0;
            u.build = 1.0;
            u.radius = bp.radius.to_f32();
            u.unit_id = i as u32 + 1;
            focus.get_or_insert(Vec3::from(u.pos));
            frame.units.push(u);
        }
        let v: Vec<f32> = view.split(',').map(|p| p.parse().expect("number")).collect();
        let mut camera = Camera::new(size, Vec2::new(w as f32, h as f32));
        // Optional fourth to sixth numbers: where to look, off the first structure.
        let off = |i: usize| v.get(i).copied().unwrap_or(0.0);
        camera.focus = focus.unwrap() + Vec3::new(off(3), off(4), off(5));
        camera.distance = v[0];
        camera.tilt = v[1];
        camera.yaw = v.get(2).copied().unwrap_or(0.0);
        let started = std::time::Instant::now();
        let frames: usize = std::env::var("SHOT_FRAMES").ok().and_then(|v| v.parse().ok()).unwrap_or(8);
        let speed: f32 = std::env::var("SHOT_SPEED").ok().and_then(|v|v.parse().ok()).unwrap_or(0.0);
        for i in 0..frames {
            for u in &mut frame.units {
                u.prev_pos = u.pos;
                if blueprints.unit(mc_data::BlueprintId(u.blueprint as u16)).transport.is_some() {
                    u.pos[0] += u.heading.cos()*speed*0.1;
                    u.pos[1] += u.heading.sin()*speed*0.1;
                    if i > 0 {
                        camera.focus.x += u.heading.cos()*speed*0.1;
                        camera.focus.y += u.heading.sin()*speed*0.1;
                    }
                }
            }
            renderer
                .render(&FrameInput {
                    camera: &camera, // At tick birth particles and hull both start at prev_pos. Static
                    // captures are unchanged; moving captures must use the same phase.
                    time: 10.0 + i as f32 * 0.1, alpha: if speed == 0.0 { 1.0 } else { 0.0 },
                    sim: Some(&frame), ghosts: &[], marks: &[], ranges: &[], ranges_drawn: 0,
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
        eprintln!("{} at {:?} in {:.1}s", path.display(), camera.focus, started.elapsed().as_secs_f32());
    }
}
