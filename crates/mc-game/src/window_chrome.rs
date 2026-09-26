//! The window's frame in the game's colours: a black title bar with the
//! program icon, light type and a red-orange edge, instead of the system's
//! white one. Windows 11 takes the colours; Windows 10 and the Linux
//! desktops take the dark theme.

use winit::event_loop::ActiveEventLoop;
use winit::window::{Theme, WindowAttributes};

/// `attrs` with the game's frame and icon.
pub(crate) fn frame(attrs: WindowAttributes, event_loop: &ActiveEventLoop) -> WindowAttributes {
    let attrs = attrs.with_theme(Some(Theme::Dark));
    windows_frame(attrs, event_loop)
}

#[cfg(windows)]
fn windows_frame(attrs: WindowAttributes, event_loop: &ActiveEventLoop) -> WindowAttributes {
    use crate::ui::palette;
    use winit::dpi::PhysicalSize;
    use winit::platform::windows::{Color, IconExtWindows, WindowAttributesExtWindows};
    use winit::window::Icon;

    // The title bar: the HUD's glass, nearly black.
    const CAPTION: u32 = 0x0D0E11;
    let color = |rgb: u32| Color::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8);
    // meridian.rc's icon, ordinal 1. The small one (title bar) at the
    // screen's scale; the large one (taskbar, Alt+Tab) at the system's size.
    let scale = event_loop
        .primary_monitor()
        .map_or(1.0, |m| m.scale_factor());
    let small = (16.0 * scale).round() as u32;
    let icon = |size| {
        Icon::from_resource(1, size)
            .inspect_err(|e| log::warn!("no program icon: {e}"))
            .ok()
    };
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
