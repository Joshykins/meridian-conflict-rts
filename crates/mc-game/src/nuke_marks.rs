//! Nuclear warfare on the map (`docs/NUKES.md`): what the interface draws in the world.
//!
//! Every flight drawn here is the one the sim flies (`nukes::WarheadPath`), never a
//! sketch of it: the aim preview, a launch waiting its turn and a warhead in the air all
//! sample the same path.
//!
//! - Every warhead in flight, whoever's: a red-orange arrowhead, what is left of its
//!   flight ahead of it and that flight's track over the ground, and at the mark the
//!   blast's rings with a countdown, pulsing faster as it comes. Interceptors are cyan darts.
//! - Our own launches ordered and not yet away: each mark numbered in the order given, its
//!   rings, the flight from the silo that will fire it, and the time to the burst.
//! - While a launch is being aimed: the blast's rings under the pointer (the core, the
//!   damage, the country set alight), the flight from the silo that would fire it, how
//!   long it takes, how many warheads are left to give out, and every enemy interceptor
//!   array known, its cover drawn, with a warning when the mark lies under it.
//! - An interceptor array selected, or being placed: the whole side's network, its
//!   cover merged into one outline, each array's rounds, and what a new site adds.

use crate::game::{Mode, Targeting};
use crate::hud::silo::{self, INTERCEPT, WARHEAD};
use crate::orders::Field;
use crate::ui::{self, palette, rgb, type_scale, Ui};
use glam::{Vec2, Vec3};
use mc_data::strategic::StrategicKind;
use mc_data::BlueprintId;
use mc_sim::mirror::{UnitInstance, KIND_WRECK, STRATEGIC_WARHEAD};
use mc_sim::nukes::WarheadPath;

mod cover;

/// Points a flight is drawn with.
const PATH_POINTS: usize = 72;

fn surface(field: &Field, p: Vec2) -> f32 {
    field.renderer.surface_height(p)
}

fn project(ui: &Ui, field: &Field, p: Vec3) -> Option<Vec2> {
    field.camera.project(p).map(|q| q / ui.s)
}

/// A ring on the ground, `radius` metres round `c`: dashed (`dash` of every 2 segments
/// drawn) or solid.
fn ground_ring(
    ui: &mut Ui,
    field: &Field,
    c: Vec2,
    radius: f32,
    width: f32,
    color: ui::Color,
    dashed: bool,
    spin: f32,
) {
    let segments = ((radius / 14.0) as usize).clamp(48, 240) & !1;
    let s = ui.s;
    let point = |i: usize| {
        let a = i as f32 / segments as f32 * std::f32::consts::TAU + spin;
        let p = c + Vec2::from_angle(a) * radius;
        field
            .camera
            .project(p.extend(surface(field, p) + 2.0))
            .map(|q| q / s)
    };
    let step = if dashed { 2 } else { 1 };
    for i in (0..segments).step_by(step) {
        if let (Some(a), Some(b)) = (point(i), point(i + 1)) {
            ui.stroke(a, b, width, color);
        }
    }
}

/// A dashed screen line from `a` to `b`, the dashes crawling toward `b`.
fn dashed(ui: &mut Ui, a: Vec2, b: Vec2, width: f32, color: ui::Color, speed: f32) {
    let len = a.distance(b);
    if len < 2.0 {
        return;
    }
    let dir = (b - a) / len;
    let (dash, gap) = (9.0, 7.0);
    let mut t = (ui.time * speed) % (dash + gap) - (dash + gap);
    while t < len {
        let s = t.max(0.0);
        let e = (t + dash).min(len);
        if e > s {
            ui.stroke(a + dir * s, a + dir * e, width, color);
        }
        t += dash + gap;
    }
}

fn clock(seconds: f32) -> String {
    let s = seconds.max(0.0).ceil() as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

/// A label with a dark backing, centred on `at`.
fn tag(ui: &mut Ui, at: Vec2, text: &str, color: u32) {
    let w = ui.text_width(type_scale::MICRO, text) + 14.0;
    let r = ui::Rect::new(at.x - w * 0.5, at.y - 9.0, w, 18.0);
    ui.fill(r, ui::ink(0.72));
    ui.fill(ui::Rect::new(r.x, r.y, 2.0, r.h), rgb(color, 0.9));
    ui.text_centred(at.x + 1.0, at.y, type_scale::MICRO, rgb(color, 1.0), text);
}

/// A flight through the air, `points` along it: drawn as a curve in space, dashed and
/// crawling toward its end, with its track over the ground under it. `lit` brightens it
/// toward the end.
fn flight(ui: &mut Ui, field: &Field, points: &[Vec3], width: f32, tone: u32, alpha: f32) {
    let crawl = (ui.time * 6.0) as usize;
    let n = points.len().max(2) as f32;
    let mut last: Option<(Vec2, Option<Vec2>)> = None;
    for (i, &p) in points.iter().enumerate() {
        let air = project(ui, field, p);
        let under = p.truncate();
        let ground = project(ui, field, under.extend(surface(field, under) + 2.0));
        if let (Some((pa, pg)), Some(a)) = (last, air) {
            let k = 0.55 + 0.45 * i as f32 / n;
            if !(i + crawl).is_multiple_of(3) {
                ui.stroke(pa, a, width, rgb(tone, alpha * k));
            }
            if let (Some(pg), Some(g)) = (pg, ground) {
                if (i + crawl).is_multiple_of(2) {
                    ui.stroke(pg, g, width * 0.8, rgb(tone, alpha * 0.55));
                }
            }
        }
        last = air.map(|a| (a, ground));
    }
}

/// `path` from `from` metres on, as points for `flight`.
fn trace(path: &WarheadPath, from: f32) -> Vec<Vec3> {
    path.trace(mc_core::Fx::from_f32(from), PATH_POINTS)
        .into_iter()
        .map(Vec3::from)
        .collect()
}

/// A warhead's blast radius and core (strategic.ron), for marks with no silo at hand.
pub const WARHEAD_BLAST: (f32, f32) = (520.0, 200.0);

/// The blast radius and core of `u`'s warhead.
fn blast_of(field: &Field, u: Option<&UnitInstance>) -> (f32, f32) {
    u.and_then(|u| {
        field
            .blueprints
            .unit(BlueprintId(u.blueprint as u16))
            .strategic
            .as_ref()?
            .blast
    })
    .map_or(WARHEAD_BLAST, |b| (b.radius.to_f32(), b.core.to_f32()))
}

/// A launch of ours ordered and not yet away: its number at the mark, its rings, the
/// flight from its silo, and when it will land.
fn planned(
    ui: &mut Ui,
    field: &Field,
    path: &WarheadPath,
    number: usize,
    text: &str,
    radius: f32,
    faint: bool,
) {
    let t = ui.time;
    let k = if faint { 0.6 } else { 1.0 };
    flight(ui, field, &trace(path, 0.0), 1.3, WARHEAD, 0.45 * k);
    let c = Vec2::from(path.mark.xy().to_f32());
    ground_ring(
        ui,
        field,
        c,
        radius,
        1.6,
        rgb(WARHEAD, 0.55 * k),
        true,
        t * 0.05,
    );
    let Some(g) = project(ui, field, c.extend(surface(field, c) + 2.0)) else {
        return;
    };
    // A numbered badge on the mark.
    ui.disc(g, 11.0, ui::ink(0.8));
    ui.arc(
        g,
        11.0,
        0.0,
        std::f32::consts::TAU,
        1.6,
        rgb(WARHEAD, 0.9 * k),
    );
    ui.text_centred(
        g.x,
        g.y,
        type_scale::VALUE,
        rgb(0xFFFFFF, k),
        &number.to_string(),
    );
    tag(ui, g + Vec2::new(0.0, 26.0), text, WARHEAD);
}

fn owner_of(u: &UnitInstance) -> u8 {
    (u.owner_flags & 0xFF) as u8
}

/// Everything above, for this frame.
pub fn draw(
    ui: &mut Ui,
    field: &Field,
    alpha: f32,
    cursor: Option<Vec3>,
    placing: Option<(BlueprintId, Vec2)>,
) {
    let view = field.view;
    let t = ui.time;
    let local = view.local;
    let team = |p: u8| view.status.players.get(p as usize).map(|pl| pl.team);
    let enemy = |p: u8| !view.observing && team(p) != team(local);

    // ---- warheads and interceptors in flight -------------------------------------------
    for m in &view.frame.strategic {
        let pos = Vec3::from(m.prev_pos).lerp(Vec3::from(m.pos), alpha);
        let prev = Vec3::from(m.prev_pos);
        let warhead = m.kind == STRATEGIC_WARHEAD;
        let tone = if warhead { WARHEAD } else { INTERCEPT };
        let Some(at) = project(ui, field, pos) else {
            continue;
        };
        let heading = project(ui, field, pos + (pos - prev).normalize_or(Vec3::Z) * 40.0)
            .map(|q| (q - at).normalize_or(Vec2::NEG_Y))
            .unwrap_or(Vec2::NEG_Y);
        if warhead {
            let mark = Vec3::from(m.mark);
            let ground = mark.truncate().extend(surface(field, mark.truncate()));
            // What is left of its flight, and its track over the ground.
            if let Some(track) = view
                .frame
                .warhead_tracks
                .iter()
                .find(|w| w.serial == m.serial)
            {
                let mut points = vec![pos];
                points.extend(
                    trace(&track.path, track.travelled.to_f32())
                        .into_iter()
                        .skip(1),
                );
                flight(ui, field, &points, 1.5, WARHEAD, 0.6);
            }
            // The blast's rings where it will land, pulsing faster as it comes.
            let urgency = (1.0 - m.eta / 40.0).clamp(0.0, 1.0);
            let pulse = 0.5 + 0.5 * (t * (2.0 + 8.0 * urgency)).sin();
            let c = mark.truncate();
            ground_ring(
                ui,
                field,
                c,
                WARHEAD_BLAST.0,
                2.0,
                rgb(WARHEAD, 0.45 + 0.4 * pulse),
                true,
                t * 0.05,
            );
            ground_ring(
                ui,
                field,
                c,
                WARHEAD_BLAST.1,
                1.6,
                rgb(WARHEAD, 0.35 + 0.3 * pulse),
                false,
                0.0,
            );
            if let Some(g) = project(ui, field, ground) {
                // Crosshair.
                for d in [Vec2::X, Vec2::Y] {
                    ui.stroke(g - d * 14.0, g - d * 5.0, 1.6, rgb(WARHEAD, 0.9));
                    ui.stroke(g + d * 5.0, g + d * 14.0, 1.6, rgb(WARHEAD, 0.9));
                }
                let who = if owner_of_missile(m.owner) == local {
                    "Our warhead"
                } else if enemy(m.owner as u8) {
                    "Incoming warhead"
                } else {
                    "Warhead"
                };
                let text = if m.boost > 0.5 {
                    format!("{who}  \u{b7}  launching")
                } else {
                    format!("{who}  \u{b7}  {}", clock(m.eta))
                };
                tag(
                    ui,
                    g + Vec2::new(0.0, 26.0),
                    &text,
                    if enemy(m.owner as u8) {
                        palette::BAD
                    } else {
                        WARHEAD
                    },
                );
            }
            // The warhead: an arrowhead along its flight, in a ring.
            let side = heading.perp();
            let nose = at + heading * 11.0;
            let back = at - heading * 7.0;
            ui.triangle(
                nose,
                back + side * 7.0,
                back - side * 7.0,
                rgb(WARHEAD, 1.0),
            );
            ui.stroke(
                at - heading * 7.0,
                at - heading * 15.0,
                2.2,
                rgb(0xFFFFFF, 0.8),
            );
            ui.arc(
                at,
                15.0 + 2.0 * pulse,
                0.0,
                std::f32::consts::TAU,
                1.2,
                rgb(WARHEAD, 0.5),
            );
        } else {
            // An interceptor: a slim dart, and a thin line to what it hunts.
            if let Some(q) = project(ui, field, Vec3::from(m.mark)) {
                dashed(ui, at, q, 1.0, rgb(INTERCEPT, 0.4), 60.0);
            }
            let side = heading.perp();
            ui.triangle(
                at + heading * 9.0,
                at - heading * 6.0 + side * 3.5,
                at - heading * 6.0 - side * 3.5,
                rgb(tone, 1.0),
            );
        }
    }

    // ---- interceptor cover ------------------------------------------------------------
    let aiming = view.mode == Mode::Target(Targeting::Nuke);
    let array_cover = |u: &UnitInstance| -> Option<f32> {
        let bp = field.blueprints.unit(BlueprintId(u.blueprint as u16));
        bp.strategic
            .as_ref()
            .filter(|s| s.kind == StrategicKind::Interceptor)
            .map(|s| s.coverage.to_f32())
    };
    let mut covered = false;
    if aiming {
        for u in &view.frame.units {
            if u.owner_flags & KIND_WRECK != 0 || u.build < 1.0 {
                continue;
            }
            let Some(cover) = array_cover(u) else {
                continue;
            };
            if !enemy(owner_of(u)) {
                continue;
            }
            let at = Vec2::new(u.pos[0], u.pos[1]);
            let under = cursor.is_some_and(|c| c.truncate().distance(at) <= cover);
            covered |= under;
            let k = if under { 0.9 } else { 0.35 };
            ground_ring(
                ui,
                field,
                at,
                cover,
                if under { 2.2 } else { 1.4 },
                rgb(INTERCEPT, k),
                true,
                -t * 0.03,
            );
        }
    } else {
        // Our side's network, merged, while an array is placed or selected (`cover.rs`).
        cover::draw(ui, field, placing);
    }

    // ---- our launches ordered and not yet away -----------------------------------------
    // Numbered in the order given; those just sent follow until the sim shows them.
    let mut number = 0;
    for p in &view.frame.planned_launches {
        let silo_unit = view.index_of.get(&p.silo).map(|&i| &view.frame.units[i]);
        number += 1;
        let text = if p.opening {
            format!("Doors opening  \u{b7}  {}", clock(p.eta))
        } else {
            format!("Queued  \u{b7}  {}", clock(p.eta))
        };
        planned(
            ui,
            field,
            &p.path,
            number,
            &text,
            blast_of(field, silo_unit).0,
            false,
        );
    }
    for sent in &view.nuke_sent {
        let Some(u) = view.index_of.get(&sent.silo).map(|&i| &view.frame.units[i]) else {
            continue;
        };
        number += 1;
        let path = silo::path_from(field.blueprints, u, sent.at.extend(surface(field, sent.at)));
        planned(
            ui,
            field,
            &path,
            number,
            "Ordered",
            blast_of(field, Some(u)).0,
            true,
        );
    }

    // ---- aiming a launch ----------------------------------------------------------------
    if !aiming {
        return;
    }
    let Some(target) = cursor else { return };
    let c = target.truncate();
    // The silo the next click would fire, and every warhead left to give out.
    let silo = silo::next_silo(view, field.blueprints, c);
    let free: u32 = silo::selected_silos(view, field.blueprints)
        .map(|u| silo::free_warheads(view, u))
        .sum();
    let (radius, core) = blast_of(
        field,
        silo.or_else(|| silo::selected_silos(view, field.blueprints).next()),
    );
    let breathe = 0.5 + 0.5 * (t * 3.0).sin();
    ground_ring(
        ui,
        field,
        c,
        radius * 1.3,
        1.2,
        rgb(palette::WARN, 0.35),
        true,
        t * 0.04,
    );
    ground_ring(
        ui,
        field,
        c,
        radius,
        2.4,
        rgb(WARHEAD, 0.75 + 0.2 * breathe),
        false,
        0.0,
    );
    ground_ring(ui, field, c, core, 1.8, rgb(WARHEAD, 0.9), true, -t * 0.08);
    let pointer = project(ui, field, target + Vec3::Z * 2.0);
    if let Some(g) = pointer {
        for k in 0..3 {
            let a = t * 0.8 + k as f32 * std::f32::consts::TAU / 3.0;
            ui.arc(g, 10.0, a - 0.5, a + 0.5, 7.0, rgb(WARHEAD, 0.85));
        }
        ui.disc(g, 2.5, rgb(WARHEAD, 1.0));
    }
    // The flight from the silo that would fire, exactly as it would fly, and how long.
    let mut label = format!("Blast {radius:.0} m");
    if let Some(u) = silo {
        let path = silo::path_from(field.blueprints, u, target);
        flight(ui, field, &trace(&path, 0.0), 1.6, WARHEAD, 0.75);
        label += &format!(
            "  \u{b7}  flight {}",
            clock(silo::flight_seconds(field.blueprints, u, &path))
        );
    }
    if let Some(g) = pointer {
        // Warheads left to give out, beside the pointer.
        let count = match free {
            0 => "No warhead free".to_owned(),
            1 => "1 warhead".to_owned(),
            n => format!("{n} warheads"),
        };
        let cw = ui.text_width(type_scale::VALUE, &count) + 18.0;
        let r = ui::Rect::new(g.x + 20.0, g.y - 30.0, cw, 22.0);
        ui.fill(r, ui::ink(0.78));
        ui.fill(ui::Rect::new(r.x, r.y, 2.0, r.h), rgb(WARHEAD, 1.0));
        ui.text(
            r.x + 10.0,
            r.y + 15.0,
            type_scale::VALUE,
            rgb(if free > 0 { 0xFFFFFF } else { palette::BAD }, 1.0),
            &count,
        );
        if free > 1 {
            let hint = if view.shift {
                "Click queues the next"
            } else {
                "Shift-click to queue more"
            };
            let hw = ui.text_width(type_scale::MICRO, hint) + 14.0;
            let h = ui::Rect::new(r.x, r.bottom() + 2.0, hw, 17.0);
            ui.fill(h, ui::ink(0.66));
            ui.text(
                h.x + 7.0,
                h.y + 12.0,
                type_scale::MICRO,
                rgb(palette::TEXT, 0.85),
                hint,
            );
        }
        tag(ui, g + Vec2::new(0.0, 30.0), &label, WARHEAD);
        if covered {
            tag(
                ui,
                g + Vec2::new(0.0, 52.0),
                "Under interceptor cover",
                INTERCEPT,
            );
        }
    }
}

fn owner_of_missile(owner: u32) -> u8 {
    owner as u8
}
