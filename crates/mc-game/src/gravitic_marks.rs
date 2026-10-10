//! A gravitic engine's work on the map (mc-sim `gravitic.rs`, the Regency's The Fallen):
//! what the interface draws in the world, in the titan marks' hand (`titan_marks.rs`) and
//! the engine's violet. Read off the sim's own wells (`RenderFrame::wells`), so it keeps
//! the sim's timing:
//!
//! - Gathering: the path its rift will run, crawling out to the mark, the well's reach
//!   there with the gathering filling round it, a crosshair and a countdown. The other
//!   side sees "Gravity well incoming".
//! - Running: the path ahead of the rift's front, a point of light at the front, and the
//!   seconds to the strike.
//! - Holding: the well's edge, named, and whether it falters (its side cannot pay).
//! - Aiming (`Targeting::Fracture`, J): the well where it would strike, the path from each
//!   picked engine, and how long the gathering and the run take. Lines only.

use crate::game::{Mode, Targeting};
use crate::orders::Field;
use crate::titan_marks::{arc, cross, ground, metres, ring, seconds, tag};
use crate::ui::{palette, rgb, Ui};
use glam::{Vec2, Vec3};
use mc_data::{gravitic::Gravitic, BlueprintId};
use mc_sim::mirror::{WellStage, WellView};
use std::f32::consts::TAU;

/// A gravitic engine's marks: violet, and the white of its held gravity.
pub const WELL: u32 = 0xB07CFF;
const HOT: u32 = 0xF0E6FF;

const TPS: f32 = mc_core::TICKS_PER_SECOND as f32;

fn gravitic_of<'a>(field: &Field<'a>, b: BlueprintId) -> Option<&'a Gravitic> {
    field.blueprints.unit(b).gravitic.as_ref()
}

/// Everything above, for this frame.
pub fn draw(ui: &mut Ui, field: &Field, cursor: Option<Vec3>) {
    let _t = mc_core::perf_span!("ui.gravitic_marks");
    let view = field.view;
    let team = |p: u8| view.status.players.get(p as usize).map(|pl| pl.team);
    let hostile = |p: u8| !view.observing && team(p) != team(view.local);
    for w in &view.frame.wells {
        under_way(ui, field, w, hostile(w.owner));
    }
    if !matches!(view.mode, Mode::Target(Targeting::Fracture)) || view.observing {
        return;
    }
    let Some(at) = cursor.map(|c| c.truncate()) else {
        return;
    };
    let engines: Vec<(Vec2, &Gravitic)> = view
        .selection
        .iter()
        .filter_map(|id| view.index_of.get(id))
        .map(|&i| &view.frame.units[i])
        .filter(|u| (u.owner_flags & 0xFF) as u8 == view.local)
        .filter_map(|u| {
            let g = gravitic_of(field, BlueprintId(u.blueprint as u16))?;
            Some((Vec2::new(u.pos[0], u.pos[1]), g))
        })
        .collect();
    let Some(&(_, first)) = engines.first() else {
        return;
    };
    aim(ui, field, &engines, first, at);
}

/// The ground path from `a` to `b`, in steps short enough to follow the hills.
fn path(
    ui: &mut Ui,
    field: &Field,
    a: Vec2,
    b: Vec2,
    width: f32,
    tone: u32,
    alpha: f32,
    crawl: f32,
) {
    let len = a.distance(b);
    if len < 1.0 {
        return;
    }
    let steps = ((len / 60.0) as usize).clamp(2, 400);
    let (dash, gap) = (40.0, 28.0);
    let shift = (ui.time * crawl) % (dash + gap);
    let mut last = ground(ui, field, a);
    for i in 1..=steps {
        let s0 = len * (i - 1) as f32 / steps as f32;
        let p = ground(ui, field, a.lerp(b, i as f32 / steps as f32));
        // Dashed by the distance along the ground, crawling toward the mark.
        let on = crawl == 0.0 || ((s0 - shift).rem_euclid(dash + gap)) < dash;
        if let (true, Some(p0), Some(p1)) = (on, last, p) {
            ui.stroke(p0, p1, width, rgb(tone, alpha));
        }
        last = p;
    }
}

/// One engine's work under way: gathering, its rift running, or its well holding.
fn under_way(ui: &mut Ui, field: &Field, w: &WellView, enemy: bool) {
    let t = ui.time;
    let tone = if enemy { palette::BAD } else { WELL };
    let (from, front, mark) = (
        Vec2::new(w.from[0], w.from[1]),
        Vec2::new(w.front[0], w.front[1]),
        Vec2::new(w.mark[0], w.mark[1]),
    );
    let r = w.radius;
    let name = field.blueprints.unit(w.blueprint).name.as_str();
    match w.stage {
        WellStage::Gathering => {
            let k = w.progress;
            let pulse = 0.5 + 0.5 * (t * (2.0 + 8.0 * k)).sin();
            path(
                ui,
                field,
                from,
                mark,
                1.4,
                tone,
                0.25 + 0.35 * k,
                80.0 + 300.0 * k,
            );
            ring(
                ui,
                field,
                mark,
                r,
                2.4,
                rgb(tone, 0.55 + 0.35 * pulse),
                false,
                0.0,
            );
            let a0 = -TAU * 0.25;
            arc(
                ui,
                field,
                mark,
                r * 1.05,
                a0,
                a0 + TAU * k,
                3.4,
                rgb(HOT, 0.95),
                false,
            );
            arc(
                ui,
                field,
                mark,
                r * 1.05,
                a0 + TAU * k,
                a0 + TAU,
                1.0,
                rgb(tone, 0.3),
                true,
            );
            ring(
                ui,
                field,
                mark,
                r * 0.33,
                1.4,
                rgb(tone, 0.5),
                true,
                -t * 0.15,
            );
            if let Some(g) = ground(ui, field, mark) {
                cross(ui, g, 16.0 + 6.0 * pulse * k, rgb(tone, 1.0));
                let head = if enemy {
                    format!("Gravity well incoming  \u{b7}  {}", seconds(w.left))
                } else {
                    format!("{name} gathering  \u{b7}  {}", seconds(w.left))
                };
                tag(ui, g + Vec2::new(0.0, 30.0), &head, tone, 1.0);
            }
        }
        WellStage::Running => {
            path(ui, field, front, mark, 1.6, tone, 0.6, 400.0);
            if let Some(p) = ground(ui, field, front) {
                ui.disc(p, 6.0, rgb(HOT, 0.5));
                ui.disc(p, 3.0, rgb(0xFFFFFF, 1.0));
            }
            let quick = 0.5 + 0.5 * (t * 14.0).sin();
            ring(
                ui,
                field,
                mark,
                r,
                2.6,
                rgb(HOT, 0.6 + 0.4 * quick),
                false,
                0.0,
            );
            ring(
                ui,
                field,
                mark,
                r * (0.3 + 0.7 * (1.0 - w.progress)),
                1.6,
                rgb(tone, 0.7),
                false,
                0.0,
            );
            if let Some(g) = ground(ui, field, mark) {
                cross(ui, g, 18.0, rgb(tone, 1.0));
                let head = if enemy {
                    format!("Rift incoming  \u{b7}  {}", seconds(w.left))
                } else {
                    format!("Rift strikes in {}", seconds(w.left))
                };
                tag(ui, g + Vec2::new(0.0, 30.0), &head, tone, 1.0);
            }
        }
        WellStage::Holding => {
            // The edge only, so what is inside stays plain to see and to pick.
            let alpha = if w.powered {
                0.7
            } else {
                0.25 + 0.25 * (t * 9.0).sin().abs()
            };
            ring(ui, field, mark, r, 2.0, rgb(tone, alpha), false, 0.0);
            ring(
                ui,
                field,
                mark,
                r * 0.985,
                1.0,
                rgb(HOT, 0.25 * alpha),
                true,
                t * 0.04,
            );
            if let Some(g) = ground(ui, field, mark + Vec2::new(0.0, -r)) {
                let text = match (enemy, w.powered) {
                    (true, _) => "Gravity well".to_owned(),
                    (false, true) => format!("{name}'s well  \u{b7}  {}", metres(r)),
                    (false, false) => format!("{name}'s well faltering: no power"),
                };
                tag(
                    ui,
                    g,
                    &text,
                    if w.powered { tone } else { palette::WARN },
                    0.9,
                );
            }
        }
    }
}

/// Aiming a fracture at `at`: where the well would strike, the path from each engine.
fn aim(ui: &mut Ui, field: &Field, engines: &[(Vec2, &Gravitic)], g: &Gravitic, at: Vec2) {
    let t = ui.time;
    let r = g.radius.to_f32();
    let mut far: f32 = 0.0;
    for &(from, _) in engines {
        path(ui, field, from, at, 1.4, WELL, 0.55, 220.0);
        far = far.max(from.distance(at));
    }
    ring(ui, field, at, r, 2.2, rgb(WELL, 0.85), false, 0.0);
    ring(ui, field, at, r * 0.33, 1.2, rgb(HOT, 0.5), true, t * 0.2);
    if let Some(p) = ground(ui, field, at) {
        cross(ui, p, 16.0, rgb(WELL, 1.0));
        let gather = g.gather_ticks as f32 / TPS;
        let run = far / (g.step.to_f32() * TPS).max(1.0);
        tag(
            ui,
            p + Vec2::new(0.0, 30.0),
            &format!(
                "Gravity well {}  \u{b7}  strikes in {}",
                metres(r),
                seconds(gather + run)
            ),
            WELL,
            1.0,
        );
    }
}
