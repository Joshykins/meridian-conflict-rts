//! The whole queue, when it is too long for the strip: the strip's last slot becomes a
//! tile counting what is left out, and a click on it opens every queued entry in rows over
//! the strip, each tile working as it does on the strip. The wheel scrolls a queue too
//! long even for the tray. Another builder, a queue that fits again, the tile or a press
//! away from the tray, strip and panel close it.

use super::queue::{stack_tile, Queue, STACK_GAP, STACK_H, STACK_W};
use super::tip;
use crate::audio::Sfx;
use crate::hud::{Hud, Scene, GAP};
use crate::ui::{id, palette, rgb, type_scale, Rect, Ui};
use glam::Vec2;

/// Whether the whole queue is open, for which builder, and how many rows it is scrolled.
#[derive(Default)]
pub(in crate::hud) struct Tray {
    open: bool,
    unit: u32,
    row: usize,
}

/// Rows the tray shows before the wheel scrolls it.
const ROWS: usize = 5;
const PAD: f32 = 12.0;
const HEADER: f32 = 24.0;
const ROW_GAP: f32 = 6.0;

/// The tiles across the tray.
fn columns(w: f32) -> usize {
    (((w - 2.0 * PAD + STACK_GAP) / (STACK_W + STACK_GAP)) as usize).max(1)
}

/// Where the tray stands over the strip `r`, while it is open for this queue's builder.
pub(super) fn rect(hud: &mut Hud, r: Rect, queue: &Queue) -> Option<Rect> {
    let tray = &mut hud.queue_tray;
    if tray.open && tray.unit != queue.unit {
        *tray = Tray::default();
    }
    if !tray.open || queue.stacks.is_empty() {
        return None;
    }
    let rows = queue.stacks.len().div_ceil(columns(r.w)).min(ROWS);
    let h = 2.0 * PAD + HEADER + rows as f32 * (STACK_H + ROW_GAP) - ROW_GAP;
    Some(Rect::new(r.x, r.y - GAP - h, r.w, h))
}

/// The strip's last slot while the queue overflows it: how many are left out, and a
/// chevron that opens the tray (or closes it, lit, while it is open).
pub(super) fn more(hud: &mut Hud, ui: &mut Ui, tr: Rect, unit: u32, hidden: usize, tip_y: f32) {
    let open = hud.queue_tray.open;
    let t = hud.tile(ui, id("queue-more", 0), tr, open, true);
    let tone = if open { palette::TEXT } else { palette::DIM };
    ui.text_centred(
        tr.x + tr.w * 0.5,
        tr.mid_y() + 4.0,
        type_scale::VALUE,
        rgb(palette::TEXT, 0.85 + 0.15 * t.glow),
        &format!("+{hidden}"),
    );
    // Up while it would open, down while it would close.
    let (c, d) = (Vec2::new(tr.x + tr.w * 0.5, tr.y + 10.0), 4.0);
    let tip_dy = if open { d * 0.5 } else { -d * 0.5 };
    ui.triangle(
        c + Vec2::new(-d, -tip_dy),
        c + Vec2::new(d, -tip_dy),
        c + Vec2::new(0.0, tip_dy),
        rgb(tone, 0.9 + 0.1 * t.glow),
    );
    if t.clicked {
        ui.audio.play(if open { Sfx::Back } else { Sfx::Select });
        hud.queue_tray = Tray {
            open: !open,
            unit,
            row: 0,
        };
    }
    if t.hovered {
        let hint = if open {
            "Close the Whole Queue".to_owned()
        } else {
            format!("{hidden} More Queued  \u{b7}  Click Shows the Whole Queue")
        };
        tip(ui, tr.x, tip_y, &hint);
    }
}

/// The tray over the strip `r`, while it is open and the queue overflows the strip.
/// Returns the top of what is drawn: the tray's, or the strip's while it is shut.
pub(super) fn draw(
    hud: &mut Hud,
    ui: &mut Ui,
    s: &Scene,
    r: Rect,
    queue: &Queue,
    overflow: bool,
) -> f32 {
    if !overflow {
        hud.queue_tray.open = false;
    }
    let Some(tray) = rect(hud, r, queue) else {
        return r.y;
    };
    let pointer = ui.cursor - ui.shift;
    // A press anywhere but the tray, the strip and the panel puts it away.
    let near = Rect::new(r.x, tray.y, r.w, queue.panel.bottom() - tray.y);
    if ui.interactive && ui.input.pressed && !near.contains(pointer) {
        hud.queue_tray.open = false;
        return r.y;
    }
    hud.glass(ui, tray);
    let stacks = queue.stacks;
    let cols = columns(r.w);
    let rows = stacks.len().div_ceil(cols);
    let last = rows.saturating_sub(ROWS);
    if ui.interactive && tray.contains(pointer) && ui.input.scroll != 0.0 {
        let row = hud.queue_tray.row;
        hud.queue_tray.row = if ui.input.scroll > 0.0 {
            row.saturating_sub(1)
        } else {
            row + 1
        };
    }
    let row = hud.queue_tray.row.min(last);
    hud.queue_tray.row = row;

    // The header: the whole of it, and where the view is in a long one.
    let (x, y) = (tray.x + PAD + 2.0, tray.y + PAD);
    let units: usize = stacks.iter().map(|k| k.count).sum();
    ui.fill(
        Rect::new(tray.x + PAD, y + 3.0, 3.0, 10.0),
        rgb(palette::TEXT, 0.9),
    );
    let end = ui.text(
        x + 10.0,
        y + 8.0,
        type_scale::CAPTION,
        rgb(palette::DIM, 1.0),
        "Whole Queue",
    );
    let entries = if stacks.len() == 1 {
        "entry"
    } else {
        "entries"
    };
    ui.text(
        end + 12.0,
        y + 8.0,
        type_scale::MICRO,
        rgb(palette::FAINT, 1.0),
        &format!("{units} queued in {} {entries}", stacks.len()),
    );
    if last > 0 {
        ui.text_right(
            tray.right() - PAD,
            y + 8.0,
            type_scale::MICRO,
            rgb(palette::FAINT, 1.0),
            &format!(
                "Rows {}\u{2013}{} of {rows}  \u{b7}  Wheel Scrolls",
                row + 1,
                row + ROWS
            ),
        );
    }

    let top = y + HEADER;
    let tip_y = tray.y - 32.0;
    for (i, k) in stacks.iter().enumerate().skip(row * cols).take(ROWS * cols) {
        let (c, rr) = (i % cols, i / cols - row);
        let tr = Rect::new(
            tray.x + PAD + c as f32 * (STACK_W + STACK_GAP),
            top + rr as f32 * (STACK_H + ROW_GAP),
            STACK_W,
            STACK_H,
        );
        stack_tile(hud, ui, s, queue, i, k, tr, id("queue-all", i), tip_y);
    }
    tray.y
}
