//! The set-up screen's frame: the header, three columns on glass and the
//! footer. It fits the canvas: the margins, gaps and columns narrow on a narrow
//! screen, and the header and footer tighten on a short one, so the chart in
//! the middle keeps its room.

use super::super::{id, ink, palette, rgb, type_scale, ButtonKind, Rect, Ui};
use super::Mode;
use glam::Vec2;

/// A canvas this short gets the tight header and footer.
const SHORT: f32 = 900.0;

/// The screen's side margin: 64 on a wide canvas, down to 28 on a narrow one.
pub fn margin(ui: &Ui) -> f32 {
    (ui.size.x * 0.033).clamp(28.0, 64.0)
}

/// Where the header's title sits and its rule runs, and where the columns start.
struct Head {
    title: f32,
    rule: f32,
    top: f32,
}

fn head(ui: &Ui) -> Head {
    if ui.size.y < SHORT {
        Head {
            title: 52.0,
            rule: 84.0,
            top: 116.0,
        }
    } else {
        Head {
            title: 84.0,
            rule: 124.0,
            top: 160.0,
        }
    }
}

/// The footer's band: the bottom of its buttons, and the bottom of the columns.
fn foot(ui: &Ui) -> (f32, f32) {
    let h = ui.size.y;
    if h < SHORT {
        (h - 28.0, h - 28.0 - 52.0 - 34.0)
    } else {
        (h - 64.0, h - 172.0)
    }
}

/// The screen's header: the mode's mark and name (the emblem and "Lobby"
/// while a lobby has no plan yet) and a caption, over the dark backdrop.
pub fn header(ui: &mut Ui, mode: Option<Mode>, caption: &str, enter: f32) {
    let (w, h) = (ui.size.x, ui.size.y);
    let left = margin(ui);
    let at = head(ui);
    ui.fill(Rect::new(0.0, 0.0, w, h), ink(0.66 * enter));
    ui.scrim(
        Rect::new(0.0, 0.0, w, at.top + 60.0),
        0.6 * enter,
        0.0,
        false,
    );
    ui.fade = enter;
    ui.shift.y = 14.0 * (1.0 - enter);
    let mark = Vec2::new(left + 15.0, at.title);
    match mode {
        Some(Mode::Survival) => super::super::survival::engine_mark(ui, mark, 12.0, 1.0, false),
        _ => ui.emblem(mark, 13.0, rgb(palette::TEXT, 0.9)),
    }
    let title = mode.map_or("Lobby", Mode::title);
    let end = ui.text(
        left + 50.0,
        at.title,
        type_scale::TITLE,
        rgb(0xFFFFFF, 1.0),
        title,
    );
    ui.text(
        end + 18.0,
        at.title + 6.0,
        type_scale::CAPTION,
        rgb(palette::DIM, 1.0),
        caption,
    );
    ui.fill(
        Rect::new(left, at.rule, 58.0, 2.0),
        rgb(palette::ACCENT, 1.0),
    );
    ui.gradient_h(
        Rect::new(left + 66.0, at.rule, w - 2.0 * left - 66.0, 1.0),
        rgb(palette::LINE, 0.35),
        rgb(palette::LINE, 0.04),
    );
}

/// The y of the header's title line, for what sits beside it (a lobby's code).
pub fn title_y(ui: &Ui) -> f32 {
    head(ui).title
}

/// The three columns, each side one on its glass: (left, centre, right). The
/// left holds the match and the chat, the centre the chart, the right the
/// commanders. The sides narrow before the chart does, down to what their
/// rows need.
pub fn columns(ui: &mut Ui) -> (Rect, Rect, Rect) {
    let w = ui.size.x;
    let left_x = margin(ui);
    let top = head(ui).top;
    let (_, bottom) = foot(ui);
    let gap = (w * 0.026).clamp(30.0, 50.0);
    let left_w = (w * 0.2).clamp(270.0, 380.0);
    let right_w = (w * 0.345).clamp(560.0, 660.0);
    let left = Rect::new(left_x, top, left_w, bottom - top);
    let right = Rect::new(w - left_x - right_w, top, right_w, bottom - top);
    let from = left.right() + gap;
    let centre = Rect::new(from, top, (right.x - gap - from).max(0.0), bottom - top);
    for side in [left, right] {
        ui.panel(Rect::new(
            side.x - 22.0,
            top - 20.0,
            side.w + 44.0,
            side.h + 40.0,
        ));
    }
    (left, centre, right)
}

/// What the footer's buttons were clicked for.
#[derive(Default)]
pub struct Footer {
    pub back: bool,
    pub launch: bool,
    /// The button beside the launch (Open to Others), when there is one.
    pub beside: bool,
}

/// The footer: back on the left, the launch on the right with `beside` (a
/// label, and whether it may be clicked) left of it, a line beside those in
/// `tone`, and a notice over that line.
pub fn footer(
    ui: &mut Ui,
    back: &str,
    launch: (&str, ButtonKind, bool),
    beside: Option<(&str, bool)>,
    line: &str,
    tone: u32,
    notice: Option<&str>,
) -> Footer {
    let w = ui.size.x;
    let left = margin(ui);
    let (base, _) = foot(ui);
    let short = ui.size.y < SHORT;
    let narrow = w < 1600.0;
    let (back_w, go_w, side_w) = if narrow {
        (160.0, 260.0, 200.0)
    } else {
        (200.0, 340.0, 240.0)
    };
    let (back_h, go_h) = if short { (46.0, 52.0) } else { (52.0, 58.0) };
    let back = ui.button(
        id("lineup-back", 0),
        Rect::new(left, base - back_h, back_w, back_h),
        back,
        ButtonKind::Secondary,
        true,
    );
    let go_rect = Rect::new(w - left - go_w, base - go_h, go_w, go_h);
    let go = ui.button(id("lineup-go", 0), go_rect, launch.0, launch.1, launch.2);
    let mut text_x = go_rect.x;
    let mut side = false;
    if let Some((label, can)) = beside {
        let r = Rect::new(go_rect.x - 16.0 - side_w, go_rect.y, side_w, go_rect.h);
        side = ui.button(id("lineup-beside", 0), r, label, ButtonKind::Secondary, can) && can;
        text_x = r.x;
    }
    // The line takes what the buttons leave between them.
    let room = text_x - 24.0 - (left + back_w + 24.0);
    fit_right(
        ui,
        text_x - 24.0,
        go_rect.mid_y(),
        room,
        type_scale::CAPTION,
        rgb(tone, 1.0),
        line,
    );
    if let Some(text) = notice {
        fit_right(
            ui,
            text_x - 24.0,
            go_rect.mid_y() - 26.0,
            room,
            type_scale::CAPTION,
            rgb(palette::WARN, 1.0),
            text,
        );
    }
    Footer {
        back,
        launch: go && launch.2,
        beside: side,
    }
}

/// Right-aligned text that never runs past `width`.
fn fit_right(
    ui: &mut Ui,
    right: f32,
    y: f32,
    width: f32,
    st: super::super::Style,
    color: super::super::Color,
    text: &str,
) {
    let (st, text) = ui.fitted(st, text, width.max(0.0));
    ui.text_right(right, y, st, color, &text);
}
