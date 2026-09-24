//! Captures several views of a map's terrain and props in one run, for
//! judging ground materials and forests without starting a match.
//!
//!   cargo run --release -p mc-render --example terrain_shots -- maps/dev16.mcmap OUT_DIR \
//!       near:6000,6000,120,0.5,0.3 far:6000,6000,900,0.9,0.3
//!
//! Each view is `name:x,y,distance,tilt[,yaw]`; frames are written as `OUT_DIR/name.ppm`.

use std::sync::Arc;

use glam::{Vec2, Vec3};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::MapFile;
use mc_render::{Camera, FrameInput, Overlay, Renderer, SceneDesc, Target};
use mc_sim::mirror::RenderFrame;

const W: u32 = 960;
const H: u32 = 640;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [map_path, out, views @ ..] = args.as_slice() else {
        eprintln!("usage: terrain_shots MAP OUT_DIR name:x,y,dist,tilt[,yaw]...");
        std::process::exit(2);
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let map = Arc::new(MapFile::open(map_path).expect("map"));
    std::fs::create_dir_all(out).unwrap();
    // The forest cover the terrain shader sees, as a greyscale map.
    let cover = mc_render::ground_cover::ground_cover(&map);
    let mut pgm = format!("P5\n{} {}\n255\n", cover.width, cover.height).into_bytes();
    pgm.extend(cover.texels.chunks_exact(4).map(|t| t[0]));
    std::fs::write(std::path::Path::new(out).join("cover.pgm"), pgm).unwrap();
    if views.is_empty() {
        return;
    }
    let blueprints = Arc::new(Blueprints::load(&root.join("data")).expect("blueprints"));
    let mut renderer = Renderer::new(
        Target::Headless { width: W, height: H },
        SceneDesc { map: map.clone(), blueprints, pool: Arc::new(Pool::new(4)), team_colors: [[0.1, 0.6, 0.9]; 8] },
    )
    .expect("renderer");
    let mut frame = RenderFrame::default();
    frame.props_dead = vec![0; map.props().len().div_ceil(32)];
    let overlay = Overlay::default();
    // `x < 0` in a view: the middle of the 400 m square holding the most trees.
    let mut cells = std::collections::HashMap::new();
    for p in map.props().iter().filter(|p| p.kind.is_tree()) {
        let xy = Vec2::from(p.pos.to_f32());
        *cells.entry(((xy.x / 400.0) as i32, (xy.y / 400.0) as i32)).or_insert(0) += 1;
    }
    let densest = cells.iter().max_by_key(|(_, n)| **n).map(|(c, _)| Vec2::new(c.0 as f32 + 0.5, c.1 as f32 + 0.5) * 400.0);
    eprintln!("{} trees; densest square at {densest:?}", map.props().iter().filter(|p| p.kind.is_tree()).count());
    for view in views {
        let (name, spec) = view.split_once(':').expect("name:spec");
        let v: Vec<f32> = spec.split(',').map(|p| p.parse().expect("number")).collect();
        let mut camera = Camera::new(Vec2::from(map.info().size_metres().to_f32()), Vec2::new(W as f32, H as f32));
        let xy = match densest {
            Some(d) if v[0] < 0.0 => d + Vec2::new(v[0] + 1.0, v[1]),
            _ => Vec2::new(v[0], v[1]),
        };
        camera.focus = Vec3::new(xy.x, xy.y, renderer.ground_height(xy));
        camera.distance = v[2];
        camera.tilt = v[3];
        camera.yaw = v.get(4).copied().unwrap_or(0.0);
        let started = std::time::Instant::now();
        // Enough frames for tile streaming to settle at the new position.
        for i in 0..12 {
            renderer
                .render(&FrameInput {
                    camera: &camera, time: 10.0, alpha: 1.0,
                    sim: (i == 0).then_some(&frame), ghosts: &[], marks: &[], ranges: &[], ranges_drawn: 0,
                    overlay: &overlay, build_grid: false,
                })
                .expect("render");
        }
        let pixels = renderer.read_pixels().expect("pixels");
        let mut ppm = format!("P6\n{W} {H}\n255\n").into_bytes();
        for pixel in pixels.chunks_exact(4) {
            ppm.extend_from_slice(&pixel[..3]);
        }
        let path = std::path::Path::new(out).join(format!("{name}.ppm"));
        std::fs::write(&path, ppm).unwrap();
        eprintln!("{} in {:.1}s", path.display(), started.elapsed().as_secs_f32());
    }
}
