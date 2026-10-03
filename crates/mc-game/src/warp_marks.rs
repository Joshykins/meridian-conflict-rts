//! Warp in the world (`mc_sim::warp`): what the interface draws on the battlefield.
//!
//! - While the warp order is in hand (`Targeting::Warp`): a line from each selected ship
//!   to where it would come out (the mark, with the ship kept in its place in the group's
//!   formation, as the sim does) and a ghost ring at that exit. Enemy dampener fields are
//!   never shown (`mc_sim::mirror::warp`). Next to the pointer, `cursor_card` gives how far
//!   the jump goes, the energy it takes (each ship's drive priced by its own distance,
//!   `mc_data::Warp::charge`, summed), how long the charge runs and the store against it,
//!   following the pointer as it moves.
//! - A ship charging its drive: a charge bar over it, filling as the grid pays.
//! - One of ours in warp: its exit ringed, with the seconds left.
//! - A stunned unit: an electric bolt and STUNNED over it, with the seconds left for ours.

use crate::game::{Mode, Targeting};
use crate::hud::warp::{seconds, stun_left, STUN, WARP};
use crate::hud::{whole, ENERGY};
use crate::nuke_marks::{dashed, ground_ring, project, surface, tag};
use crate::orders::Field;
use crate::ui::{self, ink, palette, rgb, type_scale, Rect, Ui};
use glam::{Vec2, Vec3};
use mc_data::BlueprintId;
use mc_sim::mirror::{UnitInstance, KIND_GHOST, KIND_PROP, KIND_WRECK};
use mc_sim::tables::WarpPhase;

/// Metres an exit is kept inside the map's edge (`World::clamp_to_map`).
const EDGE_MARGIN: f32 = 4.0;
/// A mark nearer than this many hull radii is not worth a jump (`warp.rs` `MIN_JUMP_RADII`).
const MIN_JUMP_RADII: f32 = 2.0;

/// One selected ship's jump toward the mark, as the sim would take it.
pub struct Jump {
    /// Where it stands, at its height.
    pub from: Vec3,
    /// Where it comes out: its place in the formation about the mark, kept on the map.
    pub to: Vec2,
    pub radius: f32,
    /// Metres from where it stands to where it comes out.
    pub distance: f32,
    /// The drive's price a kilometre; the energy this jump's whole charge takes, and the
    /// seconds it takes at full power (the sim's own sums, `mc_data::Warp`).
    pub per_km: f32,
    pub energy: f32,
    pub spool: f32,
    /// Too near to be worth a jump: the sim drops the order.
    pub short: bool,
}

fn owner_of(u: &UnitInstance) -> u8 {
    (u.owner_flags & 0xFF) as u8
}

fn at(u: &UnitInstance, alpha: f32) -> Vec3 {
    Vec3::from(u.prev_pos).lerp(Vec3::from(u.pos), alpha)
}

/// Whether `owner` is an enemy of the side the interface speaks for.
fn enemy(field: &Field, owner: u8) -> bool {
    let view = field.view;
    let team = |p: u8| view.status.players.get(p as usize).map_or(p, |pl| pl.team);
    !view.observing && team(owner) != team(view.local)
}

/// Where a ship at `from` comes out, sent toward `mark` with its group standing about
/// `middle`, on a map `size` metres across: it keeps its place about the middle, and the
/// exit is kept on the map (`World::order_warp`).
fn exit(from: Vec2, middle: Vec2, mark: Vec2, size: Vec2) -> Vec2 {
    (mark + (from - middle)).clamp(Vec2::splat(EDGE_MARGIN), size - EDGE_MARGIN)
}

/// Every selected ship of ours with a drive, and its jump toward `mark`.
pub fn jumps(field: &Field, mark: Vec2, alpha: f32) -> Vec<Jump> {
    let view = field.view;
    let size = field.map.info().size_metres();
    let size = Vec2::new(size.x.to_f32(), size.y.to_f32());
    let ships: Vec<(&UnitInstance, mc_data::Warp, f32)> = view
        .selection
        .iter()
        .filter_map(|id| view.index_of.get(id))
        .map(|&i| &view.frame.units[i])
        .filter(|u| owner_of(u) == view.local && u.owner_flags & KIND_WRECK == 0)
        .filter_map(|u| {
            let bp = field.blueprints.unit(BlueprintId(u.blueprint as u16));
            Some((u, bp.warp?, bp.radius.to_f32()))
        })
        .collect();
    let middle = ships
        .iter()
        .map(|(u, ..)| at(u, alpha).truncate())
        .sum::<Vec2>()
        / ships.len().max(1) as f32;
    ships
        .into_iter()
        .map(|(u, d, radius)| {
            let from = at(u, alpha);
            let to = exit(from.truncate(), middle, mark, size);
            let distance = from.truncate().distance(to);
            let far = mc_core::Fx::from_f32(distance);
            Jump {
                from,
                to,
                radius,
                distance,
                per_km: d.per_km.to_f32(),
                energy: d.charge(far).to_f32(),
                spool: d.charge_ticks(far) as f32 / 10.0,
                short: distance < radius * MIN_JUMP_RADII,
            }
        })
        .collect()
}

/// Everything above but the pointer's card, for this frame.
pub fn draw(ui: &mut Ui, field: &Field, alpha: f32, cursor: Option<Vec3>) {
    let _t = mc_core::perf_span!("ui.warp_marks");
    if field.view.mode == Mode::Target(Targeting::Warp) {
        if let Some(c) = cursor {
            aim(ui, field, c.truncate(), alpha);
        }
    }
    under_way(ui, field, alpha);
    stuns(ui, field, alpha);
}

/// The warp order in hand, aimed at `mark`: lines and exits.
fn aim(ui: &mut Ui, field: &Field, mark: Vec2, alpha: f32) {
    let all = jumps(field, mark, alpha);
    for j in &all {
        let height = j.from.z - surface(field, j.from.truncate());
        let exit = j.to.extend(surface(field, j.to) + height.max(0.0));
        let k = if j.short { 0.35 } else { 1.0 };
        if let (Some(a), Some(b)) = (project(ui, field, j.from), project(ui, field, exit)) {
            dashed(ui, a, b, 1.8, rgb(WARP, 0.85 * k), 40.0);
            // The ghost it comes out as: a ring at its height, a tick down to the ground.
            ui.arc(b, 9.0, 0.0, std::f32::consts::TAU, 2.0, rgb(WARP, k));
            ui.disc(b, 2.2, rgb(palette::TEXT, k));
            if let Some(g) = project(ui, field, j.to.extend(surface(field, j.to) + 2.0)) {
                ui.stroke(b, g, 1.0, rgb(WARP, 0.45 * k));
            }
            if j.short {
                tag(
                    ui,
                    b - Vec2::new(0.0, 24.0),
                    "Too near to jump",
                    palette::DIM,
                );
            }
        }
        ground_ring(
            ui,
            field,
            j.to,
            j.radius * 1.4,
            1.8,
            rgb(WARP, 0.8 * k),
            false,
            0.0,
        );
    }
}

/// A bar `share` full, centred on `at`, with `label` over it.
fn bar(ui: &mut Ui, at: Vec2, share: f32, label: &str, tone: u32) {
    let (w, h) = (74.0, 5.0);
    let r = Rect::new(at.x - w * 0.5, at.y - h * 0.5, w, h);
    ui.fill(
        Rect::new(r.x - 2.0, r.y - 2.0, r.w + 4.0, r.h + 4.0),
        ink(0.75),
    );
    ui.fill(r, rgb(tone, 0.2));
    ui.fill(
        Rect::new(r.x, r.y, r.w * share.clamp(0.0, 1.0), r.h),
        rgb(tone, 1.0),
    );
    ui.text_centred(
        at.x,
        at.y - 12.0,
        type_scale::MICRO,
        rgb(palette::TEXT, 1.0),
        label,
    );
}

/// Jumps under way: charge bars over ships spooling, our exits ringed while in warp.
fn under_way(ui: &mut Ui, field: &Field, alpha: f32) {
    let view = field.view;
    for w in &view.frame.warps {
        let ship = view.index_of.get(&w.unit_id).map(|&i| &view.frame.units[i]);
        match w.phase {
            WarpPhase::Spool => {
                let pos = ship.map_or(Vec3::from(w.from), |u| at(u, alpha));
                let over = pos + Vec3::Z * (w.radius * 0.8 + 6.0);
                if let Some(p) = project(ui, field, over) {
                    let text = format!("Charging warp  {:.0}%", w.charge * 100.0);
                    bar(ui, p - Vec2::new(0.0, 16.0), w.charge, &text, WARP);
                }
            }
            WarpPhase::Transit if !enemy(field, w.owner) => {
                let c = Vec2::new(w.to[0], w.to[1]);
                let tone = if w.dampened { palette::BAD } else { WARP };
                let pulse = 0.55 + 0.45 * (ui.time * 5.0).sin().abs();
                ground_ring(
                    ui,
                    field,
                    c,
                    w.radius * 1.4,
                    2.0,
                    rgb(tone, pulse),
                    true,
                    ui.time * 0.4,
                );
                if let Some(p) = project(ui, field, Vec3::from(w.to)) {
                    let left = seconds(w.length.saturating_sub(w.ticks) as f32 / 10.0);
                    let text = if w.dampened {
                        format!("Dampened  \u{b7}  {left}")
                    } else {
                        format!("In warp  \u{b7}  {left}")
                    };
                    tag(ui, p, &text, tone);
                }
            }
            _ => {}
        }
    }
}

/// An electric bolt, `h` points tall, centred on `c`.
fn bolt(ui: &mut Ui, c: Vec2, h: f32, color: ui::Color) {
    let p = |x: f32, y: f32| c + Vec2::new(x, y) * h;
    let points = [p(0.12, -0.5), p(-0.16, 0.02), p(0.08, 0.02), p(-0.12, 0.5)];
    for w in points.windows(2) {
        ui.stroke(w[0], w[1], 2.0, color);
    }
}

/// Stunned units, ours and those we see: a bolt and STUNNED, with the seconds for ours.
fn stuns(ui: &mut Ui, field: &Field, alpha: f32) {
    let view = field.view;
    for u in &view.frame.units {
        if u.owner_flags & (KIND_WRECK | KIND_GHOST | KIND_PROP) != 0 || u.in_warp() {
            continue;
        }
        let k = u.stun(alpha);
        if k <= 0.01 {
            continue;
        }
        let over = at(u, alpha) + Vec3::Z * (u.radius * 0.8 + 6.0);
        let Some(p) = project(ui, field, over) else {
            continue;
        };
        let flicker = 0.6 + 0.4 * (ui.time * 17.0 + u.unit_id as f32).sin().abs();
        let text = match stun_left(view, u).flatten() {
            Some(t) => format!("STUNNED  {}", seconds(t)),
            None => "STUNNED".to_owned(),
        };
        // Clear of a charge bar that might be there too.
        let at = p - Vec2::new(0.0, 40.0);
        tag(ui, at, &text, STUN);
        let w = ui.text_width(type_scale::MICRO, &text) + 14.0;
        bolt(
            ui,
            at - Vec2::new(w * 0.5 + 9.0, 0.0),
            16.0,
            rgb(STUN, flicker * k.max(0.4)),
        );
    }
}

/// The warp order's card next to the pointer: how far the jump goes, the energy it takes
/// against the store, and how long the charge runs at full power. It is worked out afresh
/// for wherever the pointer is.
pub fn cursor_card(ui: &mut Ui, field: &Field, cursor: Option<Vec3>) {
    let view = field.view;
    if view.mode != Mode::Target(Targeting::Warp) {
        return;
    }
    let all = cursor.map_or_else(Vec::new, |c| jumps(field, c.truncate(), 1.0));
    let stored = view
        .status
        .players
        .get(view.local as usize)
        .map_or(0.0, |p| p.energy);
    let (title, lines, cover) = card_text(&all, stored);
    let tone = WARP;
    let head = ui.text_width(type_scale::CAPTION, &title);
    let w = lines
        .iter()
        .map(|(l, _)| ui.text_width(type_scale::MICRO, l))
        .fold(head, f32::max)
        + 26.0;
    let h = 44.0 + lines.len() as f32 * 16.0;
    let p = ui.cursor + Vec2::new(20.0, 22.0);
    let r = Rect::new(
        p.x.min(ui.size.x - w - 4.0),
        p.y.min(ui.size.y - h - 6.0),
        w,
        h,
    );
    ui.frost(r, 0.74);
    ui.fill(Rect::new(r.x, r.y, 2.0, r.h), rgb(tone, 1.0));
    ui.text(
        r.x + 13.0,
        r.y + 14.0,
        type_scale::CAPTION,
        rgb(tone, 1.0),
        &title,
    );
    // The store against what the jump takes: full when it covers it all.
    let track = Rect::new(r.x + 13.0, r.y + 27.0, r.w - 26.0, 4.0);
    ui.fill(track, rgb(ENERGY, 0.16));
    ui.fill(
        Rect::new(track.x, track.y, track.w * cover.clamp(0.0, 1.0), track.h),
        rgb(if cover < 1.0 { palette::WARN } else { ENERGY }, 0.95),
    );
    for (i, (line, tone)) in lines.iter().enumerate() {
        ui.text(
            r.x + 13.0,
            r.y + 44.0 + i as f32 * 16.0,
            type_scale::MICRO,
            rgb(*tone, 1.0),
            line,
        );
    }
}

/// "4.2 km": a jump's distance in kilometres, to a tenth.
fn km(metres: f32) -> String {
    format!("{:.1} km", metres / 1000.0)
}

/// The card's title, its lines and their colours, and how much of the charge the store
/// covers (0 to 1 and over): for `all` the jumps and `stored` energy. The title gives the
/// distance (the farthest ship's) and the energy all the jumps take together, each ship's
/// priced by its own distance; the lines the charge's time, and the store against it.
fn card_text(all: &[Jump], stored: f32) -> (String, Vec<(String, u32)>, f32) {
    let going: Vec<&Jump> = all.iter().filter(|j| !j.short).collect();
    let need: f32 = going.iter().map(|j| j.energy).sum();
    let spool = going.iter().map(|j| j.spool).fold(0.0, f32::max);
    let far = going.iter().map(|j| j.distance).fold(0.0, f32::max);
    let short = stored < need;
    let title = format!("Warp  {}  \u{b7}  {} E", km(far), whole(need));
    let ships = match going.as_slice() {
        [] => "Too near to jump".to_owned(),
        [j] => format!(
            "{} E/km  \u{b7}  charges {spool:.1} s at full power",
            whole(j.per_km)
        ),
        many => format!(
            "{} ships  \u{b7}  charge {spool:.1} s at full power",
            many.len()
        ),
    };
    let store = if short {
        format!(
            "Stored {} of {} E  \u{b7}  {} short: charges slower",
            whole(stored),
            whole(need),
            whole(need - stored)
        )
    } else {
        format!("Stored {} of {} E", whole(stored), whole(need))
    };
    let mut lines = vec![
        (ships, palette::DIM),
        (store, if short { palette::WARN } else { palette::DIM }),
    ];
    lines.push((
        "LMB warps  \u{b7}  Shift queues  \u{b7}  RMB cancels".to_owned(),
        palette::DIM,
    ));
    let cover = if need > 0.0 { stored / need } else { 1.0 };
    (title, lines, cover)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A jump of `distance` metres by a drive of `per_km` and `spool` seconds, priced as
    /// the sim prices it.
    fn jump(per_km: f32, spool: f32, distance: f32, short: bool) -> Jump {
        let d = mc_data::Warp {
            per_km: mc_core::Fx::from_f32(per_km),
            spool_ticks: (spool * 10.0) as u16,
            cooldown_ticks: 400,
            speed: mc_core::Fx::from_int(300),
        };
        let far = mc_core::Fx::from_f32(distance);
        Jump {
            from: Vec3::ZERO,
            to: Vec2::ZERO,
            radius: 20.0,
            distance,
            per_km,
            energy: d.charge(far).to_f32(),
            spool: d.charge_ticks(far) as f32 / 10.0,
            short,
        }
    }

    #[test]
    fn an_exit_keeps_its_place_in_the_formation_and_stays_on_the_map() {
        let size = Vec2::splat(10000.0);
        let middle = Vec2::new(1000.0, 1000.0);
        // As far and the same way from the mark as it stood from the group's middle.
        assert_eq!(
            exit(
                Vec2::new(1200.0, 900.0),
                middle,
                Vec2::new(8000.0, 5000.0),
                size
            ),
            Vec2::new(8200.0, 4900.0)
        );
        // Off the map: kept four metres inside its edge.
        let off = exit(
            Vec2::new(1000.0, 700.0),
            middle,
            Vec2::new(5000.0, 100.0),
            size,
        );
        assert_eq!(off, Vec2::new(5000.0, EDGE_MARGIN));
    }

    #[test]
    fn the_card_totals_the_charge_against_the_store() {
        // A Courier and a Bastion 2 km out; a third ship too near to jump is left out.
        let all = [
            jump(1500.0, 3.0, 2000.0, false),
            jump(8000.0, 4.0, 2000.0, false),
            jump(20000.0, 5.0, 30.0, true),
        ];
        let (title, lines, cover) = card_text(&all, 24000.0);
        assert_eq!(title, "Warp  2.0 km  \u{b7}  19,000 E");
        assert!(cover > 1.0);
        // The Bastion's 4 s spool, stretched a tenth a kilometre: 4.8 s.
        assert_eq!(lines[0].0, "2 ships  \u{b7}  charge 4.8 s at full power");
        assert_eq!(
            lines[1],
            ("Stored 24,000 of 19,000 E".to_owned(), palette::DIM)
        );
        // A store that cannot cover it: warning-coloured, with how far short.
        let (_, lines, cover) = card_text(&all, 6200.0);
        assert!(cover < 1.0);
        assert_eq!(
            lines[1],
            (
                "Stored 6,200 of 19,000 E  \u{b7}  12,800 short: charges slower".to_owned(),
                palette::WARN
            )
        );
    }

    #[test]
    fn a_jump_twice_as_far_costs_twice_as_much_on_the_card() {
        // One Courier: its price a kilometre, and the charge's time, on the card.
        let (title, lines, _) = card_text(&[jump(1500.0, 3.0, 5000.0, false)], 1e6);
        assert_eq!(title, "Warp  5.0 km  \u{b7}  7,500 E");
        assert_eq!(
            lines[0].0,
            "1,500 E/km  \u{b7}  charges 4.5 s at full power"
        );
        let (title, lines, _) = card_text(&[jump(1500.0, 3.0, 10000.0, false)], 1e6);
        assert_eq!(title, "Warp  10.0 km  \u{b7}  15,000 E");
        assert_eq!(
            lines[0].0,
            "1,500 E/km  \u{b7}  charges 6.0 s at full power"
        );
        // Under a kilometre is priced as one.
        let (title, ..) = card_text(&[jump(1500.0, 3.0, 400.0, false)], 1e6);
        assert_eq!(title, "Warp  0.4 km  \u{b7}  1,500 E");
    }
}
