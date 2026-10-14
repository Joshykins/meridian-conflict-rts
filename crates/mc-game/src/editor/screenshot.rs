//! Native GPU screenshots run the same editor frame as the interactive window.
use super::*;
pub(crate) fn screenshot(
    path: &Path,
    blueprints: Arc<Blueprints>,
    pool: Arc<Pool>,
    shot: &crate::headless::Shot,
) -> Result<(), String> {
    let viewport = Vec2::new(shot.width as f32, shot.height as f32);
    let mut editor = Editor::open(path, blueprints.clone(), pool.clone(), viewport)?;
    editor.api.enabled = false;
    if let Some([x, y, d, yaw]) = shot.camera {
        editor.camera.focus = Vec2::new(x, y).extend(0.0);
        editor.camera.distance = d;
        editor.camera.yaw = yaw.to_radians();
    }
    let scene = mc_render::SceneDesc {
        map: editor.map.clone(),
        blueprints,
        pool,
        team_colors: crate::setup::TEAM_COLORS,
    };
    let mut renderer = mc_render::Renderer::new(
        mc_render::Target::Headless {
            width: shot.width,
            height: shot.height,
        },
        scene,
    )
    .map_err(|e| e.to_string())?;
    editor.apply_environment(&mut renderer);
    let mut settings = crate::settings::Settings::default();
    settings.apply_quality(shot.quality.unwrap_or_default());
    settings
        .apply_graphics(&mut renderer)
        .map_err(|e| e.to_string())?;
    let audio = crate::audio::Audio::silent();
    let mut overlay = mc_render::Overlay::default();
    let mut memory = crate::ui::Memory::default();
    let input = crate::ui::Input {
        cursor: Vec2::splat(-100.0),
        ..Default::default()
    };
    let mut changed = false;
    let mut display = false;
    for i in 0..80 {
        editor.frame(FrameCtx {
            renderer: &mut renderer,
            overlay: &mut overlay,
            input: &input,
            memory: &mut memory,
            audio: &audio,
            settings: &mut settings,
            settings_changed: &mut changed,
            display_changed: &mut display,
            now: std::time::Instant::now(),
            dt: 1.0 / 30.0,
            time: i as f32 / 30.0,
            cover: None,
        })?;
    }
    let rgba = renderer
        .read_pixels()
        .ok_or("headless editor has no pixels")?;
    crate::headless::write_png(Path::new(&shot.path), shot.width, shot.height, &rgba)
}
