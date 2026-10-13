//! Survival's warp-ins on the map: where every Seedcaster squad and every Gatewright
//! fleet will come in, drawn from the moment it is marked, for every side
//! (`RenderFrame::warp_ins`).
//!
//! - A squad's mark: the arrival shock's ring, a dashed ring round where the squad will
//!   stand with a dot at each unit's place, a crosshair, and a countdown with what it
//!   brings, pulsing faster as it comes.
//! - A fleet's mark: a dashed ring round where the fleet will come out, a ring under
//!   each ship's exit, a line up to its middle, a crosshair and its countdown (the
//!   jump's spool and transit reckoned in).
//! - On the minimap: a line from its Shaper to the mark, and the mark's ring.
//!
//! The Progenitor's warheads are drawn by `nuke_marks.rs`, which shows their marks to
//! the defenders (`nuke_marks::progenitor`).

use crate::game::View;
use crate::hud::survival::VIOLET;
use crate::nuke_marks::{ground_ring, project, surface, tag};
use crate::orders::Field;
use crate::ui::{palette, rgb, Rect, Ui};
use glam::Vec2;
use mc_sim::survival::{WarpInInstance, WarpInKind};

fn clock(seconds: f32) -> String {
    let s = seconds.max(0.0).ceil() as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

/// What a mark's countdown says it brings.
fn label(w: &WarpInInstance) -> String {
    match w.kind {
        WarpInKind::Squad => format!(
            "Warp-in  \u{b7}  {} T{}  \u{b7}  {}",
            w.units,
            w.tech,
            clock(w.eta)
        ),
        WarpInKind::Fleet => format!(
            "Progenitor fleet  \u{b7}  {} ships T{}  \u{b7}  {}",
            w.units,
            w.tech,
            clock(w.eta)
        ),
    }
}

/// Every warp-in marked and where it comes in, for this frame.
pub fn draw(ui: &mut Ui, field: &Field) {
    let _t = mc_core::perf_span!("ui.warp_in_marks");
    let t = ui.time;
    for w in &field.view.frame.warp_ins {
        let c = Vec2::new(w.at[0], w.at[1]);
        let whole = (w.age + w.eta).max(1.0);
        let urgency = (1.0 - w.eta / whole).clamp(0.0, 1.0);
        let pulse = 0.5 + 0.5 * (t * (2.0 + 9.0 * urgency)).sin();
        if w.kind == WarpInKind::Squad {
            ground_ring(
                ui,
                field,
                c,
                w.radius,
                2.2,
                rgb(VIOLET, 0.55 + 0.4 * pulse),
                false,
                0.0,
            );
        }
        ground_ring(
            ui,
            field,
            c,
            w.reach,
            1.5,
            rgb(VIOLET, 0.35 + 0.3 * pulse),
            true,
            t * 0.15,
        );
        // Where each unit comes out: a dot on the ground for a squad's, a ring under
        // each ship's exit for a fleet's.
        for s in &w.spots {
            let p = Vec2::new(s[0], s[1]);
            match w.kind {
                WarpInKind::Squad => {
                    if let Some(g) = project(ui, field, p.extend(surface(field, p) + 1.0)) {
                        ui.disc(g, 2.0, rgb(VIOLET, 0.6 + 0.3 * pulse));
                    }
                }
                WarpInKind::Fleet => ground_ring(
                    ui,
                    field,
                    p,
                    40.0,
                    1.2,
                    rgb(VIOLET, 0.4 + 0.3 * pulse),
                    false,
                    0.0,
                ),
            }
        }
        let ground = project(ui, field, c.extend(surface(field, c) + 2.0));
        if let Some(g) = ground {
            for d in [Vec2::X, Vec2::Y] {
                ui.stroke(g - d * 15.0, g - d * 5.0, 1.8, rgb(VIOLET, 0.95));
                ui.stroke(g + d * 5.0, g + d * 15.0, 1.8, rgb(VIOLET, 0.95));
            }
            let tone = if w.eta < 5.0 { palette::BAD } else { VIOLET };
            tag(ui, g + Vec2::new(0.0, 28.0), &label(w), tone);
        }
        // A fleet coming: a line up from the mark to the height its ships come out at.
        if let (WarpInKind::Fleet, Some(top), Some(g)) = (w.kind, w.spots.first(), ground) {
            if let Some(up) = project(ui, field, c.extend(top[2])) {
                ui.stroke(g, up, 1.2, rgb(VIOLET, 0.35 + 0.3 * pulse));
            }
        }
    }
}

/// The warp-ins on the minimap: each one's line from its Shaper to its mark and the
/// mark's ring. `chart_pos` turns world metres into the chart; the map is `metres` across.
pub fn minimap(
    ui: &mut Ui,
    view: &View,
    chart: Rect,
    metres: f32,
    chart_pos: &dyn Fn(Vec2) -> Vec2,
) {
    let pulse = 0.5 + 0.5 * (ui.time * 6.0).sin();
    for w in &view.frame.warp_ins {
        let from = chart_pos(Vec2::new(w.from[0], w.from[1]));
        let mark = chart_pos(Vec2::new(w.at[0], w.at[1]));
        if chart.contains(from) && chart.contains(mark) {
            ui.stroke(from, mark, 1.0, rgb(VIOLET, 0.35));
        }
        if chart.contains(mark) {
            let r = (w.reach / metres * chart.w).max(3.0);
            ui.arc(
                mark,
                r + 2.0 * pulse,
                0.0,
                std::f32::consts::TAU,
                1.2,
                rgb(VIOLET, 0.6 + 0.4 * pulse),
            );
            ui.disc(mark, 1.5, rgb(VIOLET, 1.0));
        }
    }
}
