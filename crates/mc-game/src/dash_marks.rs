//! A dash on the map (mc-sim `dash.rs`, the Regency Seraph's teleport): what the interface
//! draws in the world, in the titan marks' hand (`titan_marks.rs`) and the Regency's red.
//! Everything is read off the sim's own dashes (`RenderFrame::dashes`), so it keeps the
//! sim's timing:
//!
//! - A dasher selected: the band it dashes into, from where the ground is too close
//!   (`Dash::min`) out to its reach (`Dash::range`), each edge named; its recharge sweeping
//!   round the reach and counting down, or the band breathing when it is ready.
//! - Charging: where it comes down, its wave's reach washed over the ground there, the
//!   charge filling round it, a crosshair and a countdown, the line from it to the mark,
//!   and a ring drawing in round the dasher itself. The other side sees "Incoming".
//! - Gone: its path to the mark, a point of light running down it, and the mark pulsing
//!   white with the seconds to its arrival.
//! - Landed: its wave's front rolling out to the reach, and where it stops.
//! - Aiming an attack with a dasher picked: its wave at the pointer, and whether the
//!   point lies in its dash band, too close or too far.
//!
//! The owner's side sees all of it; the other side sees a dash where it sees the dasher
//! charging or the ground it comes down on (`mirror/dash.rs`).

use crate::game::{Mode, Targeting};
use crate::orders::Field;
use crate::titan_marks::{
    arc, crawl, cross, disc, ground, label_bearing, metres, project, ring, seconds, tag,
};
use crate::ui::{palette, rgb, Ui};
use glam::{Vec2, Vec3};
use mc_data::{BlueprintId, UnitBlueprint};
use mc_sim::mirror::{DashStage, DashView, UnitInstance};
use std::f32::consts::TAU;

/// A dash's marks: the Regency's red, and the white-hot of its charge.
const DASH: u32 = 0xFF4A5E;
const HOT: u32 = 0xFFE2E8;

const TPS: f32 = mc_core::TICKS_PER_SECOND as f32;

fn bp_of<'a>(field: &'a Field, b: BlueprintId) -> &'a UnitBlueprint {
    field.blueprints.unit(b)
}

/// The farthest `dash_only` weapon of `bp`: its landing's wave, name and reach.
fn wave_of(bp: &UnitBlueprint) -> Option<(&str, f32)> {
    bp.weapons
        .iter()
        .filter(|w| w.dash_only)
        .map(|w| (w.name.as_str(), w.range_max.to_f32()))
        .reduce(|a, b| if b.1 > a.1 { b } else { a })
}

/// Everything above, for this frame.
pub fn draw(ui: &mut Ui, field: &Field, alpha: f32, cursor: Option<Vec3>) {
    let _t = mc_core::perf_span!("ui.dash_marks");
    let view = field.view;
    let team = |p: u8| view.status.players.get(p as usize).map(|pl| pl.team);
    let hostile = |p: u8| !view.observing && team(p) != team(view.local);
    let mut busy = Vec::new();
    for d in &view.frame.dashes {
        busy.push(d.unit_id);
        under_way(ui, field, d, hostile(d.owner), alpha);
    }

    let picked: Vec<&UnitInstance> = view
        .selection
        .iter()
        .filter_map(|id| view.index_of.get(id))
        .map(|&i| &view.frame.units[i])
        .filter(|u| {
            bp_of(field, BlueprintId(u.blueprint as u16)).dash.is_some()
                && (u.owner_flags & 0xFF) as u8 == view.local
                && !view.observing
        })
        .collect();
    let aiming = matches!(
        view.mode,
        Mode::Target(Targeting::Attack | Targeting::Bombard | Targeting::Strike)
    );
    for u in &picked {
        if busy.contains(&u.unit_id) {
            continue;
        }
        band(ui, field, u, alpha);
    }
    if let (true, Some(u), Some(at)) = (aiming, picked.first(), cursor) {
        aim(ui, field, u, at);
    }
}

/// Seconds before `u` may dash again (`UnitOrders::dash_recharge`).
fn recharge(field: &Field, u: &UnitInstance) -> f32 {
    crate::sim_thread::queue_of(&field.view.status.queues, u.unit_id)
        .map_or(0.0, |q| q.dash_recharge)
}

/// A selected dasher's band: too close inside, its reach outside, its recharge round it.
fn band(ui: &mut Ui, field: &Field, u: &UnitInstance, alpha: f32) {
    let bp = bp_of(field, BlueprintId(u.blueprint as u16));
    let Some(dash) = bp.dash else {
        return;
    };
    let t = ui.time;
    let c = Vec3::from(u.prev_pos)
        .lerp(Vec3::from(u.pos), alpha)
        .truncate();
    let (lo, hi) = (dash.min.to_f32(), dash.range.to_f32());
    let left = recharge(field, u);
    let full = dash.cooldown_ticks.max(1) as f32 / TPS;
    let ready = left <= 0.0;
    let breathe = 0.5 + 0.5 * (t * 2.2).sin();
    // The ground it may come down on, washed faintly; its reach, and the too-close edge.
    disc(
        ui,
        field,
        c,
        hi,
        rgb(DASH, if ready { 0.035 } else { 0.02 }),
    );
    disc(ui, field, c, lo, rgb(0x000000, 0.05));
    let edge = if ready { 0.55 + 0.3 * breathe } else { 0.35 };
    ring(ui, field, c, hi, 2.0, rgb(DASH, edge), false, 0.0);
    ring(
        ui,
        field,
        c,
        hi * 0.985,
        1.0,
        rgb(HOT, 0.25 * edge),
        false,
        0.0,
    );
    ring(
        ui,
        field,
        c,
        lo,
        1.4,
        rgb(palette::WARN, 0.5),
        true,
        t * 0.03,
    );
    // Ticks round the reach, every 30 degrees, sliding slowly round while it is ready.
    let spin = if ready { t * 0.05 } else { 0.0 };
    for k in 0..12 {
        let a = spin + k as f32 * TAU / 12.0;
        let dir = Vec2::from_angle(a);
        if let (Some(p), Some(q)) = (
            ground(ui, field, c + dir * hi * 0.965),
            ground(ui, field, c + dir * hi * 1.02),
        ) {
            ui.stroke(p, q, 1.6, rgb(DASH, 0.6 * edge));
        }
    }
    if !ready {
        // The recharge filling round just outside the reach, from the top.
        let k = (1.0 - left / full).clamp(0.0, 1.0);
        let a0 = -TAU * 0.25;
        arc(
            ui,
            field,
            c,
            hi * 1.035,
            a0,
            a0 + TAU * k,
            3.0,
            rgb(DASH, 0.9),
            false,
        );
        arc(
            ui,
            field,
            c,
            hi * 1.035,
            a0 + TAU * k,
            a0 + TAU,
            1.0,
            rgb(DASH, 0.25),
            true,
        );
    }
    // The names, opposite where the guns' reach is named (`titan_marks::reach_labels`).
    let bearing = Vec2::from_angle(label_bearing(ui, field, c, &[lo, hi]) + TAU * 0.5);
    if let Some(g) = ground(ui, field, c + bearing * hi) {
        let text = if ready {
            format!("Dash ready  \u{b7}  {}", metres(hi))
        } else {
            format!("Dash recharging  \u{b7}  {}", seconds(left))
        };
        tag(ui, g, &text, DASH, 1.0);
    }
    if let Some(g) = ground(ui, field, c + bearing * lo) {
        tag(
            ui,
            g,
            &format!("Too close to dash  {}", metres(lo)),
            palette::WARN,
            0.85,
        );
    }
}

/// One dash under way: charging, gone or landed.
fn under_way(ui: &mut Ui, field: &Field, d: &DashView, enemy: bool, alpha: f32) {
    let view = field.view;
    let t = ui.time;
    let bp = bp_of(field, d.blueprint);
    let tone = if enemy { palette::BAD } else { DASH };
    let to = Vec2::new(d.to[0], d.to[1]);
    let from = Vec2::new(d.from[0], d.from[1]);
    let (wave_name, wave) = wave_of(bp).unwrap_or(("Landing", bp.radius.to_f32() * 3.0));
    let len = d.length.max(1) as f32;
    let k = ((d.ticks as f32 + alpha) / len).clamp(0.0, 1.0);
    let left = (len - d.ticks as f32 - alpha).max(0.0) / TPS;
    let who = bp.name.as_str();
    match d.stage {
        DashStage::Charging => {
            let pulse = 0.5 + 0.5 * (t * (3.0 + 10.0 * k)).sin();
            disc(
                ui,
                field,
                to,
                wave,
                rgb(
                    tone,
                    if enemy {
                        0.08 + 0.08 * pulse
                    } else {
                        0.05 + 0.03 * k
                    },
                ),
            );
            ring(
                ui,
                field,
                to,
                wave,
                2.4,
                rgb(tone, 0.6 + 0.35 * pulse),
                false,
                0.0,
            );
            // The charge filling round just outside it, and its inner heart.
            let a0 = -TAU * 0.25;
            arc(
                ui,
                field,
                to,
                wave * 1.05,
                a0,
                a0 + TAU * k,
                3.4,
                rgb(HOT, 0.95),
                false,
            );
            arc(
                ui,
                field,
                to,
                wave * 1.05,
                a0 + TAU * k,
                a0 + TAU,
                1.0,
                rgb(tone, 0.3),
                true,
            );
            ring(
                ui,
                field,
                to,
                wave * 0.3,
                1.4,
                rgb(tone, 0.5),
                true,
                -t * 0.2,
            );
            // A ring drawing in round the dasher as the charge fills.
            if let Some(u) = view.index_of.get(&d.unit_id).map(|&i| &view.frame.units[i]) {
                let at = Vec2::new(u.pos[0], u.pos[1]);
                let r = bp.radius.to_f32() * (1.2 + 3.0 * (1.0 - k));
                ring(
                    ui,
                    field,
                    at,
                    r,
                    2.0 + 2.0 * k,
                    rgb(HOT, 0.35 + 0.6 * k),
                    false,
                    0.0,
                );
                let chest = Vec3::from(u.pos) + Vec3::Z * bp.height.to_f32() * 0.6;
                if let (Some(a), Some(b)) = (project(ui, field, chest), ground(ui, field, to)) {
                    crawl(ui, a, b, 1.6, rgb(tone, 0.4 + 0.4 * k), 60.0 + 200.0 * k);
                }
            }
            if let Some(g) = ground(ui, field, to) {
                cross(ui, g, 16.0 + 6.0 * pulse * k, rgb(tone, 1.0));
                let head = if enemy {
                    format!("Incoming {who}  \u{b7}  {}", seconds(left))
                } else {
                    format!("{who} dash charging  \u{b7}  {}", seconds(left))
                };
                tag(ui, g + Vec2::new(0.0, 30.0), &head, tone, 1.0);
                let foot = format!("{wave_name} {}", metres(wave));
                tag(ui, g + Vec2::new(0.0, 51.0), &foot, HOT, 0.85);
            }
        }
        DashStage::Gone => {
            // Its path, a point of light running down it over the first part of the gap.
            if let (Some(a), Some(b)) = (ground(ui, field, from), ground(ui, field, to)) {
                ui.stroke(a, b, 1.0, rgb(tone, 0.3 * (1.0 - k)));
                let run = (k / 0.4).clamp(0.0, 1.0);
                if run < 1.0 {
                    let p = a.lerp(b, run);
                    ui.disc(p, 5.0, rgb(HOT, 0.5));
                    ui.disc(p, 2.5, rgb(0xFFFFFF, 1.0));
                }
            }
            let quick = 0.5 + 0.5 * (t * (8.0 + 16.0 * k)).sin();
            disc(ui, field, to, wave, rgb(tone, 0.06 + 0.06 * quick));
            ring(
                ui,
                field,
                to,
                wave,
                2.6,
                rgb(HOT, 0.6 + 0.4 * quick),
                false,
                0.0,
            );
            // Closing in on the mark as it comes.
            let r = wave * (0.3 + 0.7 * (1.0 - k));
            ring(
                ui,
                field,
                to,
                r,
                1.6,
                rgb(HOT, 0.4 + 0.5 * k),
                true,
                t * 0.6,
            );
            if let Some(g) = ground(ui, field, to) {
                cross(ui, g, 18.0, rgb(0xFFFFFF, 0.7 + 0.3 * quick));
                let head = if enemy {
                    format!("{who} arriving  \u{b7}  {}", seconds(left))
                } else {
                    format!("Arriving  \u{b7}  {}", seconds(left))
                };
                tag(ui, g + Vec2::new(0.0, 30.0), &head, tone, 1.0);
            }
        }
        DashStage::Landed => {
            // Its wave's front rolling out, as fast as the sim's, and where it stops.
            let age = (d.ticks as f32 + alpha) / TPS;
            let front = (age * d.wave_speed).min(wave);
            let fade = (1.0 - k).clamp(0.0, 1.0).sqrt();
            ring(
                ui,
                field,
                to,
                wave,
                1.4,
                rgb(tone, 0.45 * fade),
                true,
                -t * 0.05,
            );
            ring(ui, field, to, front, 3.2, rgb(HOT, 0.95 * fade), false, 0.0);
            ring(
                ui,
                field,
                to,
                front * 0.97,
                1.2,
                rgb(tone, 0.6 * fade),
                false,
                0.0,
            );
            if let Some(g) = ground(ui, field, to) {
                tag(ui, g + Vec2::new(0.0, 30.0), wave_name, tone, fade);
            }
        }
    }
}

/// Aiming an attack with a dasher picked: its wave at the pointer, and whether it would
/// dash there.
fn aim(ui: &mut Ui, field: &Field, u: &UnitInstance, at: Vec3) {
    let bp = bp_of(field, BlueprintId(u.blueprint as u16));
    let Some(dash) = bp.dash else {
        return;
    };
    let t = ui.time;
    let c = at.truncate();
    let from = Vec2::new(u.pos[0], u.pos[1]);
    let d = from.distance(c);
    let (lo, hi) = (dash.min.to_f32(), dash.range.to_f32());
    let (verdict, good) = if d < lo {
        (format!("Too close to dash ({})", metres(lo)), false)
    } else if d > hi {
        (format!("Out of dash reach ({})", metres(hi)), false)
    } else {
        (format!("Dashes in  \u{b7}  {}", metres(d)), true)
    };
    let tone = if good { DASH } else { palette::WARN };
    let (wave_name, wave) = wave_of(bp).unwrap_or(("Landing", bp.radius.to_f32() * 3.0));
    let breathe = 0.5 + 0.5 * (t * 3.0).sin();
    if good {
        disc(ui, field, c, wave, rgb(DASH, 0.05));
    }
    ring(
        ui,
        field,
        c,
        wave,
        2.4,
        rgb(tone, 0.7 + 0.25 * breathe),
        false,
        0.0,
    );
    ring(ui, field, c, wave * 0.3, 1.4, rgb(tone, 0.8), true, t * 0.2);
    let chest = Vec3::from(u.pos) + Vec3::Z * bp.height.to_f32() * 0.6;
    if let (Some(a), Some(b)) = (project(ui, field, chest), ground(ui, field, c)) {
        crawl(ui, a, b, 1.4, rgb(tone, 0.6), 80.0);
    }
    if let Some(g) = project(ui, field, at + Vec3::Z * 2.0) {
        for k in 0..3 {
            let a = t * 1.2 + k as f32 * TAU / 3.0;
            ui.arc(g, 11.0, a - 0.45, a + 0.45, 3.0, rgb(tone, 0.9));
        }
        ui.disc(g, 2.5, rgb(tone, 1.0));
        let left = recharge(field, u);
        let state = if left > 0.0 {
            format!("Dash recharging  \u{b7}  {}", seconds(left))
        } else {
            format!(
                "Dash ready  \u{b7}  charge {}",
                seconds(dash.windup_ticks as f32 / TPS)
            )
        };
        let head = format!("{wave_name} {}  \u{b7}  {state}", metres(wave));
        tag(ui, g + Vec2::new(0.0, 32.0), &head, DASH, 1.0);
        tag(ui, g + Vec2::new(0.0, 53.0), &verdict, tone, 1.0);
    }
}
