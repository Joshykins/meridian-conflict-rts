//! The crash screen's process. A crashed run starts the game again as
//! `meridian --crash-screen FILE --crash-title ... --crash-message ...`, waits
//! for it to say `ready` on stdout (its first frame is up) and exits. A fresh
//! process, so a corrupt heap or a lost device in the crashed one does not
//! follow it; and only the splash presenter (overlay, no scene, no map, no
//! data), so it comes up even when the game's data is what broke. When it
//! cannot come up, the crashed run shows the system's task dialog instead.

use super::screen::{Action, Screen, Summary};
use crate::audio::Audio;
use crate::ui::{self, Ui};
use glam::Vec2;
use mc_render::{Overlay, Splash, Target};
use std::io::{BufRead, Write};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::window::{Window, WindowId};

/// The window's size in logical pixels; the screen is laid out one point to one.
const SIZE: (f64, f64) = (1180.0, 760.0);

/// How long a crashed run waits for the screen before showing the task dialog.
const WAIT: Duration = Duration::from_secs(20);

/// Starts the crash screen for `summary`. True once its first frame is up;
/// false (and nothing left running) when it could not come up.
pub(super) fn launch(summary: &Summary) -> bool {
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    let mut command = std::process::Command::new(exe);
    command
        .arg("--crash-screen")
        .arg(&summary.path)
        .arg("--crash-title")
        .arg(&summary.title)
        .arg("--crash-message")
        .arg(&summary.message)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());
    if let Some(hint) = &summary.hint {
        command.arg("--crash-hint").arg(hint);
    }
    let Ok(mut child) = spawn_apart(&mut command) else {
        return false;
    };
    let Some(stdout) = child.stdout.take() else {
        let _ = child.kill();
        return false;
    };
    let (tx, rx) = std::sync::mpsc::channel();
    let _ = std::thread::Builder::new()
        .name("crash-screen".into())
        .spawn(move || {
            let mut line = String::new();
            let _ = std::io::BufReader::new(stdout).read_line(&mut line);
            let _ = tx.send(line.trim() == "ready");
        });
    match rx.recv_timeout(WAIT) {
        Ok(true) => {
            log::info!("crash screen up");
            true
        }
        outcome => {
            log::warn!("the crash screen did not come up ({outcome:?}); the task dialog instead");
            let _ = child.kill();
            false
        }
    }
}

/// Starts `command` outside the crashed run's job object, if it has one: a
/// launcher (or a terminal) that put the game in a job that kills its processes
/// when it closes would otherwise take the crash screen down as the crashed run
/// exits. A job that forbids leaving it refuses that; then it starts in the job.
fn spawn_apart(command: &mut std::process::Command) -> std::io::Result<std::process::Child> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
        if let Ok(child) = command.creation_flags(CREATE_BREAKAWAY_FROM_JOB).spawn() {
            return Ok(child);
        }
        command.creation_flags(0);
    }
    command.spawn()
}

/// The crash screen's process: a window until the player is done with it.
pub(crate) fn run(summary: Summary) -> Result<(), String> {
    let report = std::fs::read_to_string(&summary.path)
        .map_err(|e| format!("{}: {e}", summary.path.display()))?;
    let event_loop = EventLoop::new().map_err(|e| e.to_string())?;
    let now = Instant::now();
    let mut app = Reporter {
        screen: Screen::new(summary, &report),
        report,
        window: None,
        splash: None,
        overlay: Overlay::default(),
        memory: ui::Memory::default(),
        input: ui::Input::default(),
        audio: Audio::silent(),
        started: now,
        last: now,
        failed: None,
    };
    event_loop.run_app(&mut app).map_err(|e| e.to_string())?;
    log::info!("crash screen: closed ({:?})", app.failed);
    app.failed.map_or(Ok(()), Err)
}

struct Reporter {
    screen: Screen,
    report: String,
    window: Option<Arc<Window>>,
    splash: Option<Splash>,
    overlay: Overlay,
    memory: ui::Memory,
    input: ui::Input,
    audio: Audio,
    started: Instant,
    last: Instant,
    failed: Option<String>,
}

impl Reporter {
    fn fail(&mut self, event_loop: &ActiveEventLoop, message: String) {
        self.failed = Some(message);
        event_loop.exit();
    }

    fn open(&mut self, event_loop: &ActiveEventLoop) -> Result<(), String> {
        let attrs =
            Window::default_attributes().with_title("Meridian Conflict \u{b7} Crash report");
        let attrs = crate::window_chrome::frame(attrs, event_loop)
            .with_inner_size(winit::dpi::LogicalSize::new(SIZE.0, SIZE.1))
            .with_min_inner_size(winit::dpi::LogicalSize::new(900.0, 600.0))
            // White until the first frame is presented, as the game's window.
            .with_visible(!cfg!(windows));
        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .map_err(|e| format!("could not create a window: {e}"))?,
        );
        let size = window.inner_size();
        let (display, handle) = match (window.display_handle(), window.window_handle()) {
            (Ok(d), Ok(h)) => (d.as_raw(), h.as_raw()),
            _ => return Err("the window has no native handle".into()),
        };
        let splash = Splash::new(Target::Window {
            display,
            window: handle,
            width: size.width,
            height: size.height,
            vsync: true,
        })
        .map_err(|e| e.to_string())?;
        self.window = Some(window);
        self.splash = Some(splash);
        Ok(())
    }

    fn frame(&mut self, event_loop: &ActiveEventLoop) -> Result<(), String> {
        let (Some(window), Some(splash)) = (&self.window, &mut self.splash) else {
            return Ok(());
        };
        let now = Instant::now();
        let dt = (now - self.last).as_secs_f32().min(0.1);
        self.last = now;
        let time = (now - self.started).as_secs_f32();
        let size = window.inner_size();
        let viewport = Vec2::new(size.width.max(1) as f32, size.height.max(1) as f32);
        let action = draw(
            &mut self.screen,
            &mut self.overlay,
            &self.input,
            &mut self.memory,
            &self.audio,
            viewport,
            window.scale_factor() as f32,
            time,
            dt,
        );
        self.input.end_frame();
        splash.render(&self.overlay).map_err(|e| e.to_string())?;
        match action {
            Some(Action::Copy) => {
                let ok = crate::clipboard::copy(&self.report).is_ok();
                self.screen.copied(time, ok);
            }
            Some(Action::OpenFolder) => super::open_folder(self.screen.path()),
            Some(Action::Restart) => {
                if let Ok(exe) = std::env::current_exe() {
                    let _ = std::process::Command::new(exe).spawn();
                }
                event_loop.exit();
            }
            Some(Action::Quit) => event_loop.exit(),
            None => {}
        }
        Ok(())
    }
}

/// One frame of the screen into `overlay`, one point to a logical pixel.
#[expect(
    clippy::too_many_arguments,
    reason = "the frame's state, shared by the window and the screenshot"
)]
fn draw(
    screen: &mut Screen,
    overlay: &mut Overlay,
    input: &ui::Input,
    memory: &mut ui::Memory,
    audio: &Audio,
    viewport: Vec2,
    scale: f32,
    time: f32,
    dt: f32,
) -> Option<Action> {
    overlay.clear();
    memory.begin_frame();
    let mut ui = Ui::new(
        overlay,
        input,
        memory,
        audio,
        viewport,
        scale * ui::CANVAS_H / viewport.y,
        time,
        dt,
    );
    let action = screen.draw(&mut ui);
    memory.end_frame(input);
    action
}

impl ApplicationHandler for Reporter {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        if let Err(e) = self.open(event_loop) {
            return self.fail(event_loop, e);
        }
        if let Err(e) = self.frame(event_loop) {
            return self.fail(event_loop, e);
        }
        if let Some(w) = &self.window {
            w.set_visible(true);
            w.focus_window();
        }
        log::info!("crash screen: first frame up");
        // The crashed run waits for this before it exits.
        let mut out = std::io::stdout();
        let _ = writeln!(out, "ready");
        let _ = out.flush();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        self.input.feed(&event, false);
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Destroyed => log::info!("crash screen: window destroyed"),
            WindowEvent::Resized(size) => {
                if let Some(s) = &mut self.splash {
                    if let Err(e) = s.resize(size.width, size.height) {
                        self.fail(event_loop, e.to_string());
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                if let Err(e) = self.frame(event_loop) {
                    self.fail(event_loop, e);
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }
}

/// `--crash-screen FILE --screenshot OUT.png`: the screen, headless, once it
/// has come up, and `OUT-copied.png` just after Copy details. For working on it.
pub(crate) fn screenshot(
    summary: Summary,
    blueprints: Arc<mc_data::Blueprints>,
    pool: Arc<mc_jobs::Pool>,
    shot: &crate::headless::Shot,
    cursor: Option<[f32; 2]>,
) -> Result<(), String> {
    let report = std::fs::read_to_string(&summary.path)
        .map_err(|e| format!("{}: {e}", summary.path.display()))?;
    let path = crate::setup::backdrop_map().ok_or("no maps found")?;
    let map =
        Arc::new(mc_map::MapFile::open(&path).map_err(|e| format!("{}: {e}", path.display()))?);
    let mut renderer = mc_render::Renderer::new(
        Target::Headless {
            width: shot.width,
            height: shot.height,
        },
        mc_render::SceneDesc {
            map,
            blueprints,
            pool,
            team_colors: crate::setup::TEAM_COLORS,
        },
    )
    .map_err(|e| e.to_string())?;
    let viewport = Vec2::new(shot.width as f32, shot.height as f32);
    let camera = mc_render::Camera::new(Vec2::splat(1000.0), viewport);
    // The window's size in points at whatever size the shot is.
    let scale = viewport.y / SIZE.1 as f32;
    let mut screen = Screen::new(summary, &report);
    let (mut overlay, mut memory, audio) =
        (Overlay::default(), ui::Memory::default(), Audio::silent());
    let mut input = ui::Input::default();
    if let Some([x, y]) = cursor {
        input.cursor = Vec2::new(x, y);
    }
    let dt = 1.0 / 60.0;
    let out = Path::new(&shot.path);
    for i in 0..=150 {
        let time = i as f32 * dt;
        if i == 90 {
            crate::loading::shoot(&mut renderer, &camera, time, &overlay, shot, out)?;
            screen.copied(time, true);
        }
        draw(
            &mut screen,
            &mut overlay,
            &input,
            &mut memory,
            &audio,
            viewport,
            scale,
            time,
            dt,
        );
    }
    let copied = out.with_file_name(format!(
        "{}-copied.png",
        out.file_stem().and_then(|s| s.to_str()).unwrap_or("crash")
    ));
    crate::loading::shoot(&mut renderer, &camera, 2.5, &overlay, shot, &copied)
}
