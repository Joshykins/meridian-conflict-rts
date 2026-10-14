//! The Edict on the map (mc-sim `edict.rs`, the Regent's main gun): what the interface
//! draws in the world, in the titan marks' hand (`titan_marks.rs`) and the Regency's
//! pinch-fusion red and white. Read off the sim's own work (`RenderFrame::edicts`), so it
//! keeps the sim's timing:
//!
//! - Gathering: the blast's core and reach and the wave's rim round the mark, the gather
//!   filling round it, the line down from the firing point on the hull (which moves with
//!   it), a crosshair and a countdown; "stalled: no power" when its side cannot pay. The
//!   other side sees "Edict incoming".
//! - Fired: the wave's front as it rolls out and burns.
//! - Aiming (`Targeting::Edict`, J): each picked ship's reach (the hull's spine grown by
//!   the range), the rings where it would strike, the line from the hull's nearest point,
//!   and whether it fires from where it is or closes first. Lines only.

use crate::game::{Mode, Targeting};
use crate::orders::Field;
use crate::titan_marks::{arc, cross, ground, metres, project, ring, seconds, tag};
use crate::ui::{palette, rgb, Ui};
use glam::{Vec2, Vec3};
use mc_data::{edict::Edict, BlueprintId, UnitBlueprint};
use mc_sim::mirror::{EdictStage, EdictView};
use std::f32::consts::{PI, TAU};

/// The Edict's marks: Regency red, and the white of the pinched plasma.
pub const EDICT: u32 = 0xFF4A3A;
const HOT: u32 = 0xFFF1E6;

const TPS: f32 = mc_core::TICKS_PER_SECOND as f32;

/// The hull's spine in the world plane (mc-sim `body::spine`): its two ends.
fn spine(bp: &UnitBlueprint, pos: Vec2, heading: f32) -> (Vec2, Vec2) {
    let nose = Vec2::from_angle(heading);
    match bp.body {
        Some(b) => (
            pos + nose * (b.aft + b.beam).to_f32(),
            pos + nose * (b.fore - b.beam).to_f32(),
        ),
        None => (pos, pos),
    }
}

/// The point of `a..b` nearest `p`.
fn nearest(a: Vec2, b: Vec2, p: Vec2) -> Vec2 {
    let d = b - a;
    let len = d.length_squared();
    if len < 1.0 {
        return a;
    }
    a + d * ((p - a).dot(d) / len).clamp(0.0, 1.0)
}

/// Everything above, for this frame.
pub fn draw(ui: &mut Ui, field: &Field, cursor: Option<Vec3>) {
    let _t = mc_core::perf_span!("ui.edict_marks");
    let view = field.view;
    let team = |p: u8| view.status.players.get(p as usize).map(|pl| pl.team);
    let hostile = |p: u8| !view.observing && team(p) != team(view.local);
    for e in &view.frame.edicts {
        under_way(ui, field, e, hostile(e.owner));
    }
    if !matches!(view.mode, Mode::Target(Targeting::Edict)) || view.observing {
        return;
    }
    let Some(at) = cursor.map(|c| c.truncate()) else {
        return;
    };
    let ships: Vec<(Vec2, Vec2, f32, &Edict)> = (view.picked_units().into_iter())
        .filter(|u| (u.owner_flags & 0xFF) as u8 == view.local)
        .filter_map(|u| {
            let bp = field.blueprints.unit(BlueprintId(u.blueprint as u16));
            let e = bp.edict.as_ref()?;
            let (a, b) = spine(bp, Vec2::new(u.pos[0], u.pos[1]), u.heading);
            Some((a, b, u.pos[2], e))
        })
        .collect();
    if ships.is_empty() {
        return;
    }
    aim(ui, field, &ships, at);
}

/// The blast's core, its reach and the wave's rim round `mark`.
fn rings(ui: &mut Ui, field: &Field, mark: Vec2, e: &EdictView, tone: u32, alpha: f32) {
    let t = ui.time;
    ring(ui, field, mark, e.radius, 2.4, rgb(tone, alpha), false, 0.0);
    ring(
        ui,
        field,
        mark,
        e.core,
        1.6,
        rgb(HOT, 0.7 * alpha),
        false,
        0.0,
    );
    if e.wave > e.radius + 1.0 {
        ring(
            ui,
            field,
            mark,
            e.wave,
            1.0,
            rgb(tone, 0.4 * alpha),
            true,
            t * 0.03,
        );
    }
}

/// One ship's Edict under way: gathering, or its wave burning.
fn under_way(ui: &mut Ui, field: &Field, e: &EdictView, enemy: bool) {
    let t = ui.time;
    let tone = if enemy { palette::BAD } else { EDICT };
    let mark = Vec2::new(e.mark[0], e.mark[1]);
    match e.stage {
        EdictStage::Gathering => {
            let k = e.progress;
            let pulse = 0.5 + 0.5 * (t * (2.0 + 10.0 * k)).sin();
            let alpha = if e.powered { 0.55 + 0.35 * pulse } else { 0.35 };
            rings(ui, field, mark, e, tone, alpha);
            let a0 = -TAU * 0.25;
            arc(
                ui,
                field,
                mark,
                e.radius * 1.05,
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
                e.radius * 1.05,
                a0 + TAU * k,
                a0 + TAU,
                1.0,
                rgb(tone, 0.3),
                true,
            );
            // The line down from the firing point on the hull.
            let from = Vec3::from(e.from);
            if let (Some(p), Some(g)) = (project(ui, field, from), ground(ui, field, mark)) {
                crate::titan_marks::crawl(
                    ui,
                    p,
                    g,
                    1.2,
                    rgb(tone, 0.35 + 0.4 * k),
                    30.0 + 90.0 * k,
                );
                ui.disc(p, 4.0 + 5.0 * k * pulse, rgb(HOT, 0.6));
            }
            if let Some(g) = ground(ui, field, mark) {
                cross(ui, g, 16.0 + 8.0 * pulse * k, rgb(tone, 1.0));
                let name = field.blueprints.unit(e.blueprint).name.as_str();
                let (head, tone) = match (enemy, e.powered) {
                    (true, _) => (format!("Edict incoming  \u{b7}  {}", seconds(e.left)), tone),
                    (false, true) => (
                        format!("{name}'s Edict  \u{b7}  fires in {}", seconds(e.left)),
                        tone,
                    ),
                    (false, false) => (format!("{name}'s Edict stalled: no power"), palette::WARN),
                };
                tag(ui, g + Vec2::new(0.0, 30.0), &head, tone, 1.0);
            }
        }
        EdictStage::Fired if e.seconds < e.wave_life => {
            let fade = (1.0 - e.seconds / e.wave_life).clamp(0.0, 1.0);
            ring(
                ui,
                field,
                mark,
                e.wave_reach,
                2.0,
                rgb(tone, 0.6 * fade),
                false,
                0.0,
            );
        }
        EdictStage::Fired => {}
    }
}

/// Aiming the Edict at `at`: each ship's reach, the rings where it would strike, and the
/// line from the nearest hull.
fn aim(ui: &mut Ui, field: &Field, ships: &[(Vec2, Vec2, f32, &Edict)], at: Vec2) {
    let t = ui.time;
    let &(_, _, _, e) = &ships[0];
    let range = e.range.to_f32();
    let mut closest = f32::MAX;
    for &(a, b, z, _) in ships {
        reach(ui, field, a, b, range);
        let from = nearest(a, b, at);
        closest = closest.min(from.distance(at));
        if let (Some(p), Some(g)) = (project(ui, field, from.extend(z)), ground(ui, field, at)) {
            crate::titan_marks::crawl(ui, p, g, 1.2, rgb(EDICT, 0.6), 60.0);
            ui.disc(p, 4.0, rgb(HOT, 0.7));
        }
    }
    let (r, core, wave) = (
        e.blast.radius.to_f32(),
        e.blast.core.to_f32(),
        e.wave.radius.to_f32(),
    );
    ring(ui, field, at, r, 2.2, rgb(EDICT, 0.85), false, 0.0);
    ring(ui, field, at, core, 1.4, rgb(HOT, 0.6), false, 0.0);
    if wave > r + 1.0 {
        ring(ui, field, at, wave, 1.0, rgb(EDICT, 0.4), true, t * 0.03);
    }
    if let Some(p) = ground(ui, field, at) {
        cross(ui, p, 16.0, rgb(EDICT, 1.0));
        let gather = e.gather_ticks as f32 / TPS;
        let text = if closest <= range {
            format!("Edict {}  \u{b7}  fires in {}", metres(r), seconds(gather))
        } else {
            format!("Out of reach: closes {} first", metres(closest - range))
        };
        tag(ui, p + Vec2::new(0.0, 30.0), &text, EDICT, 1.0);
    }
}

/// A ship's reach: the hull's spine `a..b` grown by `range`, as a stadium on the ground.
fn reach(ui: &mut Ui, field: &Field, a: Vec2, b: Vec2, range: f32) {
    let along = (b - a).normalize_or(Vec2::X);
    let side = along.perp() * range;
    let heading = along.to_angle();
    let tone = rgb(EDICT, 0.45);
    arc(
        ui,
        field,
        b,
        range,
        heading - PI / 2.0,
        heading + PI / 2.0,
        1.4,
        tone,
        false,
    );
    arc(
        ui,
        field,
        a,
        range,
        heading + PI / 2.0,
        heading + PI * 1.5,
        1.4,
        tone,
        false,
    );
    let steps = ((a.distance(b) / 60.0) as usize).clamp(2, 200);
    for s in [side, -side] {
        let mut last = ground(ui, field, a + s);
        for i in 1..=steps {
            let p = ground(ui, field, a.lerp(b, i as f32 / steps as f32) + s);
            if let (Some(p0), Some(p1)) = (last, p) {
                ui.stroke(p0, p1, 1.4, tone);
            }
            last = p;
        }
    }
}
