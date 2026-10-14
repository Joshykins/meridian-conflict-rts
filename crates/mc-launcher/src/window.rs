//! The window: winit for the window and its input, softbuffer to put the
//! CPU-drawn frame on screen. It draws when something changed or moves, and
//! otherwise waits, looking in on running work four times a second.

use std::num::NonZeroU32;
use std::rc::Rc;
use std::time::{Duration, Instant};

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{CursorIcon, Theme, Window, WindowAttributes, WindowButtons, WindowId};

use crate::app::{Action, App};
use crate::install::{self, Log};
use crate::paint::{Backdrop, Painter};
use crate::screens;
use crate::ui::{Input, Memory, Ui};

struct Shown {
    window: Rc<Window>,
    surface: softbuffer::Surface<Rc<Window>, Rc<Window>>,
}

struct Launcher<'a> {
    app: App,
    log: &'a mut Log,
    shown: Option<Shown>,
    painter: Option<Painter>,
    input: Input,
    memory: Memory,
    opened: Instant,
    last: Instant,
    /// Draw again soon: something moves.
    busy: bool,
    /// What to do once the window has closed.
    after: Option<Action>,
    error: Option<String>,
}

/// Runs the window until the player plays or closes it; then does what was
/// asked (starting a build, or the launcher's update).
pub(crate) fn run(app: App, log: &mut Log) -> Result<Option<Action>, String> {
    let event_loop = EventLoop::new().map_err(|e| format!("no window system: {e}"))?;
    let mut launcher = Launcher {
        app,
        log,
        shown: None,
        painter: None,
        input: Input::default(),
        memory: Memory::default(),
        opened: Instant::now(),
        last: Instant::now(),
        busy: true,
        after: None,
        error: None,
    };
    event_loop
        .run_app(&mut launcher)
        .map_err(|e| format!("the window failed: {e}"))?;
    if let Some(e) = launcher.error {
        return Err(e);
    }
    let action = launcher.after.take();
    if let Some(Action::Play(m, args)) = &action {
        launcher.app.install.play(m, args, launcher.log)?;
    }
    Ok(action)
}

fn attributes(event_loop: &ActiveEventLoop) -> WindowAttributes {
    let (w, h) = screens::SIZE;
    let attrs = Window::default_attributes()
        .with_title("Meridian Conflict")
        .with_inner_size(LogicalSize::new(w as f64, h as f64))
        .with_resizable(false)
        .with_enabled_buttons(WindowButtons::CLOSE | WindowButtons::MINIMIZE)
        .with_theme(Some(Theme::Dark));
    windows_frame(attrs, event_loop)
}

/// The game's frame on Windows (mc-game `window_chrome.rs`): a near-black
/// title bar, light type, a red-orange edge, and the program icon.
#[cfg(windows)]
fn windows_frame(attrs: WindowAttributes, event_loop: &ActiveEventLoop) -> WindowAttributes {
    use crate::paint::palette;
    use winit::dpi::PhysicalSize;
    use winit::platform::windows::{Color, IconExtWindows, WindowAttributesExtWindows};
    use winit::window::Icon;

    const CAPTION: u32 = 0x0D0E11;
    let color = |rgb: u32| Color::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8);
    let scale = event_loop
        .primary_monitor()
        .map_or(1.0, |m| m.scale_factor());
    let small = (16.0 * scale).round() as u32;
    let icon = |size| Icon::from_resource(1, size).ok();
    attrs
        .with_window_icon(icon(Some(PhysicalSize::new(small, small))))
        .with_taskbar_icon(icon(None))
        .with_title_background_color(Some(color(CAPTION)))
        .with_title_text_color(color(palette::TEXT))
        .with_border_color(Some(color(palette::ACCENT_DEEP)))
}

#[cfg(not(windows))]
fn windows_frame(attrs: WindowAttributes, _event_loop: &ActiveEventLoop) -> WindowAttributes {
    attrs
}

impl Launcher<'_> {
    fn fail(&mut self, event_loop: &ActiveEventLoop, e: String) {
        self.log.line(&format!("window: {e}"));
        self.error = Some(e);
        event_loop.exit();
    }

    fn act(&mut self, event_loop: &ActiveEventLoop, action: Action) {
        match action {
            Action::Open(target) => install::open(&target),
            Action::Hide => {
                if let Some(s) = &self.shown {
                    s.window.set_visible(false);
                }
            }
            a @ (Action::Play(..) | Action::Relaunch | Action::Close) => {
                self.after = Some(a);
                event_loop.exit();
            }
        }
    }

    fn redraw(&mut self, event_loop: &ActiveEventLoop) {
        let Some(shown) = &mut self.shown else {
            return;
        };
        let size = shown.window.inner_size();
        let (Some(w), Some(h)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) else {
            return;
        };
        if let Err(e) = shown.surface.resize(w, h) {
            let e = format!("the window's surface: {e}");
            return self.fail(event_loop, e);
        }
        let scale = shown.window.scale_factor() as f32;
        let painter = self
            .painter
            .get_or_insert_with(|| Painter::new(size.width, size.height, scale, Backdrop::new()));
        painter.begin(size.width, size.height, scale);
        let now = Instant::now();
        let dt = now.duration_since(self.last).as_secs_f32().min(0.1);
        self.last = now;
        let time = now.duration_since(self.opened).as_secs_f32();
        let mut ui = Ui::new(painter, &self.input, &mut self.memory, time, dt);
        let action = screens::frame(&mut ui, &mut self.app);
        let (busy, hand) = (ui.busy, ui.hand);
        self.busy = busy;
        self.input.end_frame();
        shown.window.set_cursor(if hand {
            CursorIcon::Pointer
        } else {
            CursorIcon::Default
        });
        match shown.surface.buffer_mut() {
            Ok(mut buffer) => {
                painter.to_words(&mut buffer);
                if let Err(e) = buffer.present() {
                    self.log.line(&format!("present: {e}"));
                }
            }
            Err(e) => {
                let e = format!("the window's buffer: {e}");
                return self.fail(event_loop, e);
            }
        }
        if let Some(action) = action {
            self.act(event_loop, action);
        }
    }
}

impl ApplicationHandler for Launcher<'_> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.shown.is_some() {
            return;
        }
        let window = match event_loop.create_window(attributes(event_loop)) {
            Ok(w) => Rc::new(w),
            Err(e) => return self.fail(event_loop, format!("could not open a window: {e}")),
        };
        let surface = softbuffer::Context::new(window.clone())
            .and_then(|context| softbuffer::Surface::new(&context, window.clone()));
        match surface {
            Ok(surface) => self.shown = Some(Shown { window, surface }),
            Err(e) => self.fail(event_loop, format!("could not draw to the window: {e}")),
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let scale = self.shown.as_ref().map_or(1.0, |s| s.window.scale_factor());
        match event {
            WindowEvent::CloseRequested => {
                self.after = Some(Action::Close);
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => self.redraw(event_loop),
            WindowEvent::CursorMoved { position, .. } => {
                let p = position.to_logical::<f32>(scale);
                self.input.cursor = (p.x, p.y);
                self.busy = true;
            }
            WindowEvent::CursorLeft { .. } => {
                self.input.cursor = (-1.0, -1.0);
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                let down = state == ElementState::Pressed;
                self.input.down = down;
                if down {
                    self.input.pressed = true;
                } else {
                    self.input.released = true;
                }
                self.busy = true;
            }
            WindowEvent::MouseWheel { delta, .. } => {
                self.input.scroll -= match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 48.0,
                };
                self.busy = true;
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(action) = self.app.step(self.log) {
            self.act(event_loop, action);
        }
        let now = Instant::now();
        // Every wake draws: work finishing in the background changes what
        // the window says without an event of its own.
        if let Some(s) = &self.shown {
            if s.window.is_visible() != Some(false) {
                s.window.request_redraw();
            }
        }
        // Thirty frames a second while something moves; else a look at the
        // running work four times a second.
        let wait = if self.busy {
            Duration::from_millis(33)
        } else {
            Duration::from_millis(250)
        };
        event_loop.set_control_flow(ControlFlow::WaitUntil(now + wait));
    }
}
