use super::super::*;
use super::FlakBurst;

/// Shoots every flak gun's burst through the real Vulkan renderer at four ages, into
/// `artifacts/flak/<weapon>-<age>.ppm` (or `FLAK_OUT`): the blink, the shrapnel at
/// the edge of the splash, the smoke with its tendrils out, and the puff hanging on the
/// wind. Run on the Windows build: `cargo test --release -p mc-render --lib
/// flak_bursts_render -- --ignored`.
#[test]
#[ignore = "requires Vulkan and maps/crosswater.mcmap"]
fn flak_bursts_render() {
    use glam::Vec2;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let map = Arc::new(MapFile::open(root.join("maps/crosswater.mcmap")).unwrap());
    let blueprints = Arc::new(Blueprints::load(&root.join("data")).unwrap());
    let mut renderer = Renderer::new(
        Target::Headless {
            width: 960,
            height: 720,
        },
        SceneDesc {
            map: map.clone(),
            blueprints: blueprints.clone(),
            pool: Arc::new(Pool::new(2)),
            team_colors: [[0.1, 0.6, 0.9]; mc_core::MAX_PLAYERS],
        },
    )
    .unwrap();
    renderer.fog_enabled = false;
    let xy = Vec2::new(12360.0, 12380.0);
    let target = xy.extend(renderer.ground_height(xy) + 160.0);
    let mut camera = Camera::new(
        Vec2::from(map.info().size_metres().to_f32()),
        Vec2::new(960.0, 720.0),
    );
    camera.focus = target;
    camera.distance = 230.0;
    camera.tilt = 0.25;
    let frame = RenderFrame {
        props_dead: vec![0; map.props().len().div_ceil(32)],
        ..Default::default()
    };
    let overlay = Overlay::default();
    let draw = |renderer: &mut Renderer, time| {
        renderer
            .render(&FrameInput {
                camera: &camera,
                time,
                alpha: 1.0,
                sim: Some(&frame),
                ghosts: &[],
                marks: &[],
                ranges: &[],
                ranges_drawn: 0,
                overlay: &overlay,
                build_grid: false,
                icons: true,
            })
            .unwrap();
    };
    let output = std::env::var("FLAK_OUT")
        .map_or_else(|_| root.join("artifacts/flak"), std::path::PathBuf::from);
    std::fs::create_dir_all(&output).unwrap();
    let mut guns: Vec<&mc_data::Weapon> = Vec::new();
    for w in blueprints.units.iter().flat_map(|u| &u.weapons) {
        if w.flak && !guns.iter().any(|g| g.name == w.name) {
            guns.push(w);
        }
    }
    assert!(guns.len() >= 3, "flak guns: {}", guns.len());
    for (index, gun) in guns.iter().enumerate() {
        let time = 10.0 + index as f32 * 20.0;
        for _ in 0..24 {
            draw(&mut renderer, time - 8.0);
        }
        renderer.flak_burst(
            &FlakBurst {
                at: target,
                motion: Vec3::ZERO,
                splash: gun.splash.to_f32(),
                impact: gun.impact,
                direct: false,
            },
            time,
        );
        let name = gun.name.to_lowercase().replace(' ', "-");
        for (label, age) in [
            ("blink", 0.04),
            ("shrapnel", 0.3),
            ("puff", 1.0),
            ("hang", 3.5),
        ] {
            draw(&mut renderer, time + age);
            let pixels = renderer.read_pixels().unwrap();
            let mut ppm = b"P6\n960 720\n255\n".to_vec();
            for pixel in pixels.as_chunks::<4>().0 {
                ppm.extend_from_slice(&pixel[..3]);
            }
            std::fs::write(output.join(format!("{name}-{label}.ppm")), ppm).unwrap();
        }
    }
}
