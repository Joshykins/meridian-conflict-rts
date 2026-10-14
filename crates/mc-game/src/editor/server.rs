//! Native offscreen editor, sharing commands, UI rendering and reload with the window.
use super::*;
pub(crate) fn serve(
    path: &Path,
    blueprints: Arc<Blueprints>,
    pool: Arc<Pool>,
    size: (u32, u32),
) -> Result<(), String> {
    let mut editor = Editor::open(
        path,
        blueprints.clone(),
        pool.clone(),
        Vec2::new(size.0 as f32, size.1 as f32),
    )?;
    editor.api.headless = true;
    let target = || mc_render::Target::Headless {
        width: size.0,
        height: size.1,
    };
    let mut renderer = mc_render::Renderer::new(
        target(),
        mc_render::SceneDesc {
            map: editor.map.clone(),
            blueprints,
            pool,
            team_colors: crate::setup::TEAM_COLORS,
        },
    )
    .map_err(|e| e.to_string())?;
    editor.apply_environment(&mut renderer);
    let mut settings = crate::settings::Settings::default();
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
    let started = std::time::Instant::now();
    println!(
        "Offscreen editor {} ready; use scripts/editor.sh. Close with op=close.",
        std::process::id()
    );
    loop {
        let now = std::time::Instant::now();
        let event = editor.frame(FrameCtx {
            renderer: &mut renderer,
            overlay: &mut overlay,
            input: &input,
            memory: &mut memory,
            audio: &audio,
            settings: &mut settings,
            settings_changed: &mut changed,
            display_changed: &mut display,
            now,
            dt: 0.1,
            time: started.elapsed().as_secs_f32(),
            cover: None,
        })?;
        if matches!(event, Some(Event::Leave)) {
            return Ok(());
        }
        if editor.reload_requested() {
            editor.begin_reload(target());
        }
        if let Some(reloaded) = editor.poll_reload() {
            editor.install_reload(reloaded, &mut renderer, &mut settings, &audio, false)?;
        }
        // This is an authoring service, not a game: ten frames per second keeps
        // the mailbox responsive without competing with a foreground match.
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}
