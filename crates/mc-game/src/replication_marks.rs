//! Survival's replication artillery on the map: where every Seedcaster shell will land,
//! drawn from the moment it leaves, for every side (`RenderFrame::shells`). The arc is
//! the sim's own (`ShellInstance::at`), so what is drawn is what flies.
//!
//! - The shell itself: a violet diamond on its arc, what is left of the arc ahead of it
//!   and its track over the ground.
//! - Its mark: the burst's ring, a dashed ring round where its squad will stand, a
//!   crosshair, and a countdown with what it prints, pulsing faster as it comes.
//! - On the minimap: the line from the shell to its mark and the mark's ring.
//!
//! The Progenitor's warheads are drawn by `nuke_marks.rs`, which shows their marks to
//! the defenders (`nuke_marks::progenitor`).

use crate::game::View;
use crate::hud::survival::VIOLET;
use crate::nuke_marks::{ground_ring, project, surface, tag};
use crate::orders::Field;
use crate::ui::{palette, rgb, Rect, Ui};
use glam::{Vec2, Vec3};
use mc_sim::survival::ShellInstance;

/// Points the arc ahead of a shell is drawn with.
const ARC_POINTS: usize = 40;

/// Metres round its mark a shell's squad stands: its block of `units`, roughly.
fn squad_reach(units: u16) -> f32 {
    30.0 + 14.0 * (units as f32).sqrt()
}

fn clock(seconds: f32) -> String {
    let s = seconds.max(0.0).ceil() as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

/// What is left of `shell`'s arc from `from` (0..1 of its flight) on, with its track
/// over the ground under it, dashed and crawling toward the mark.
fn arc_ahead(ui: &mut Ui, field: &Field, shell: &ShellInstance, from: f32, alpha: f32) {
    let crawl = (ui.time * 6.0) as usize;
    let mut last: Option<(Vec2, Option<Vec2>)> = None;
    for i in 0..=ARC_POINTS {
        let t = from + (1.0 - from) * i as f32 / ARC_POINTS as f32;
        let p = Vec3::from(shell.at(t));
        let air = project(ui, field, p);
        let under = p.truncate();
        let ground = project(ui, field, under.extend(surface(field, under) + 2.0));
        if let (Some((pa, pg)), Some(a)) = (last, air) {
            let k = 0.5 + 0.5 * i as f32 / ARC_POINTS as f32;
            if !(i + crawl).is_multiple_of(3) {
                ui.stroke(pa, a, 1.5, rgb(VIOLET, alpha * k));
            }
            if let (Some(pg), Some(g)) = (pg, ground) {
                if (i + crawl).is_multiple_of(2) {
                    ui.stroke(pg, g, 1.1, rgb(VIOLET, alpha * 0.45));
                }
            }
        }
        last = air.map(|a| (a, ground));
    }
}

/// Every replication shell in the air and where it lands, for this frame.
pub fn draw(ui: &mut Ui, field: &Field, alpha: f32) {
    let _t = mc_core::perf_span!("ui.replication_marks");
    let t = ui.time;
    for shell in &field.view.frame.shells {
        let pos = Vec3::from(shell.prev_pos).lerp(Vec3::from(shell.pos), alpha);
        let share = ((shell.age + alpha * 0.1) / shell.flight.max(0.1)).clamp(0.0, 1.0);
        arc_ahead(ui, field, shell, share, 0.65);
        // The mark: the burst, where the squad will stand, pulsing faster as it comes.
        let c = Vec2::new(shell.to[0], shell.to[1]);
        let urgency = (1.0 - shell.eta / shell.flight.max(1.0)).clamp(0.0, 1.0);
        let pulse = 0.5 + 0.5 * (t * (2.0 + 9.0 * urgency)).sin();
        ground_ring(
            ui,
            field,
            c,
            shell.radius,
            2.2,
            rgb(VIOLET, 0.55 + 0.4 * pulse),
            false,
            0.0,
        );
        ground_ring(
            ui,
            field,
            c,
            squad_reach(shell.units),
            1.5,
            rgb(VIOLET, 0.35 + 0.3 * pulse),
            true,
            t * 0.15,
        );
        if let Some(g) = project(ui, field, c.extend(surface(field, c) + 2.0)) {
            for d in [Vec2::X, Vec2::Y] {
                ui.stroke(g - d * 15.0, g - d * 5.0, 1.8, rgb(VIOLET, 0.95));
                ui.stroke(g + d * 5.0, g + d * 15.0, 1.8, rgb(VIOLET, 0.95));
            }
            let text = format!(
                "Replication shell  \u{b7}  {} T{}  \u{b7}  {}",
                shell.units,
                shell.tech,
                clock(shell.eta)
            );
            let tone = if shell.eta < 5.0 {
                palette::BAD
            } else {
                VIOLET
            };
            tag(ui, g + Vec2::new(0.0, 28.0), &text, tone);
        }
        // The shell: a diamond on its arc.
        if let Some(at) = project(ui, field, pos) {
            let r = 6.0 + 1.5 * pulse;
            let (n, e) = (Vec2::new(0.0, -r), Vec2::new(r * 0.7, 0.0));
            ui.triangle(at + n, at + e, at - e, rgb(VIOLET, 1.0));
            ui.triangle(at - n, at + e, at - e, rgb(VIOLET, 1.0));
            ui.arc(
                at,
                r + 6.0,
                0.0,
                std::f32::consts::TAU,
                1.2,
                rgb(VIOLET, 0.5),
            );
        }
    }
}

/// The shells on the minimap: each one's line to its mark and the burst's ring there.
/// `chart_pos` turns world metres into the chart; the map is `metres` across.
pub fn minimap(
    ui: &mut Ui,
    view: &View,
    chart: Rect,
    metres: f32,
    chart_pos: &dyn Fn(Vec2) -> Vec2,
) {
    let pulse = 0.5 + 0.5 * (ui.time * 6.0).sin();
    for shell in &view.frame.shells {
        let at = chart_pos(Vec2::new(shell.pos[0], shell.pos[1]));
        let mark = chart_pos(Vec2::new(shell.to[0], shell.to[1]));
        if chart.contains(at) && chart.contains(mark) {
            ui.stroke(at, mark, 1.0, rgb(VIOLET, 0.55));
        }
        if chart.contains(mark) {
            let r = (squad_reach(shell.units) / metres * chart.w).max(3.0);
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
        if chart.contains(at) {
            ui.disc(at, 2.5, rgb(VIOLET, 1.0));
        }
    }
}
