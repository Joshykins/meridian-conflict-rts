//! What is selected, and what it can be told to do: the selection panel (with
//! STATUS, WEAPONS and DOSSIER pages) and the order card, whose orders come in
//! colour-coded families and only when the selection can carry them out.

use super::build::tip;
use super::icons::{self, Glyph};
use super::style::{domain_wash, Domain, Family, VETERANCY};
use super::{has_flag, whole, Hud, HudAction, Scene};
use crate::audio::Sfx;
use crate::game::{Mode, Targeting};
use crate::rings::{projections, Reach};
use crate::ui::{id, ink, palette, rgb, type_scale, Rect, Style, Ui};
use glam::Vec2;
use mc_data::{cat, MoveLayer, Trajectory, UnitBlueprint, Weapon};
use mc_sim::mirror::UnitInstance;
use mc_sim::tables::{flag, OrderKind};
use mc_sim::{veterancy_health, FireState, VETERANCY_MAX};

mod details;
mod pause_order;
mod status;

use details::details_card;
use status::status_page;

pub const ORDER_W: f32 = 104.0;
pub const ORDER_H: f32 = 40.0;
/// Order names: medium weight, tracked lightly so they fit their tile.
const LABEL: Style = crate::ui::style(mc_render::Face::Medium, 11.5, 1.2);
pub const ORDER_GAP: f32 = 5.0;
/// Width of the order card for this many families.
pub fn orders_width(families: usize) -> f32 {
    families as f32 * (ORDER_W + ORDER_GAP) - ORDER_GAP + 28.0
}

/// Damage per second of one weapon. `salvo` counts every projectile of a salvo;
/// `salvo_batch` only groups them.
pub fn weapon_dps(w: &Weapon) -> f32 {
    w.damage.to_f32() * w.salvo.max(1) as f32 / (w.reload_ticks.max(1) as f32 * 0.1)
}

/// Damage per second of everything a unit carries.
pub fn dps(bp: &UnitBlueprint) -> f32 {
    bp.weapons.iter().map(weapon_dps).sum()
}

/// A weapon's name and its figures, two to a row when drawn.
pub fn weapon_rows(w: &Weapon) -> (String, Vec<(&'static str, String)>) {
    let shots = w.salvo.max(1) as u32;
    let mut rows = vec![
        (
            "Damage",
            if shots > 1 {
                format!("{} \u{d7} {shots}", whole(w.damage.to_f32()))
            } else {
                whole(w.damage.to_f32())
            },
        ),
        ("DPS", format!("{:.0}", weapon_dps(w))),
        (
            "Range",
            if w.range_min.to_f32() > 0.0 {
                format!(
                    "{:.0}\u{2013}{:.0} m",
                    w.range_min.to_f32(),
                    w.range_max.to_f32()
                )
            } else {
                format!("{:.0} m", w.range_max.to_f32())
            },
        ),
        ("Reload", format!("{:.1} s", w.reload_ticks as f32 * 0.1)),
    ];
    if w.splash.to_f32() > 0.0 {
        rows.push(("Splash", format!("{:.0} m", w.splash.to_f32())));
    }
    if w.projectile_speed.to_f32() > 0.0 {
        rows.push((
            "Velocity",
            format!("{:.0} m/s", w.projectile_speed.to_f32()),
        ));
    }
    let kind = if w.missile && w.guided {
        "Guided Missile"
    } else if w.missile {
        "Rocket"
    } else if w.trajectory == Trajectory::Ballistic {
        "Ballistic"
    } else {
        "Direct"
    };
    rows.push(("Type", kind.to_owned()));
    let mut targets = Vec::new();
    for (bit, label) in [
        (cat::LAND, "Land"),
        (cat::AIR, "Air"),
        (cat::NAVAL, "Naval"),
    ] {
        if w.target_mask & bit != 0 {
            targets.push(label);
        }
    }
    rows.push(("Targets", targets.join(" \u{b7} ")));
    (w.name.clone(), rows)
}

/// Words laid into lines no wider than `width`.
pub fn wrap_text(ui: &mut Ui, st: Style, text: &str, width: f32) -> Vec<String> {
    ui.wrap(st, text, width)
}

fn health_tone(share: f32) -> u32 {
    if share > 0.6 {
        super::HEALTHY
    } else if share > 0.3 {
        palette::WARN
    } else {
        palette::BAD
    }
}

/// The missing share stays in the bar's own colour, dimmed, so its length reads.
fn bar(ui: &mut Ui, r: Rect, share: f32, tone: u32) {
    ui.fill(r, rgb(tone, 0.18));
    ui.gradient_h(
        Rect::new(r.x, r.y, r.w * share.clamp(0.0, 1.0), r.h),
        rgb(tone, 0.85),
        rgb(tone, 1.0),
    );
}

pub const RANKS: [&str; 6] = [
    "Recruit",
    "Veteran",
    "Hardened",
    "Elite",
    "Heroic",
    "Legendary",
];

pub fn chevrons(ui: &mut Ui, origin: Vec2, level: u8) {
    for i in 0..VETERANCY_MAX {
        let c = origin + Vec2::new(i as f32 * 11.0, 0.0);
        let r = 5.0;
        let tip = c + Vec2::new(0.0, -r);
        let left = c + Vec2::new(-r * 0.85, r * 0.5);
        let right = c + Vec2::new(r * 0.85, r * 0.5);
        if i < level {
            ui.triangle(tip, left, right, rgb(VETERANCY, 1.0));
        } else {
            ui.stroke(left, tip, 1.2, rgb(palette::LINE, 0.35));
            ui.stroke(tip, right, 1.2, rgb(palette::LINE, 0.35));
        }
    }
}

/// Paused work, in the construction amber it would otherwise be lit in.
const PAUSED: u32 = 0xFFA928;

pub fn activity(kind: OrderKind) -> &'static str {
    match kind {
        OrderKind::Move => "Moving",
        OrderKind::AttackMove => "Attack-Moving",
        OrderKind::Attack => "Attacking",
        OrderKind::Build => "Building",
        OrderKind::Assist => "Assisting",
        OrderKind::Reclaim | OrderKind::ReclaimUnit | OrderKind::ReclaimArea => "Reclaiming",
        OrderKind::Produce => "Producing",
        OrderKind::Upgrade => "Upgrading",
        OrderKind::AttackGround => "Firing on Ground",
        OrderKind::Strike => "Calling the Storm",
        OrderKind::Bombard => "Bombarding",
        OrderKind::Patrol => "Patrolling",
        OrderKind::Guard => "Guarding",
        OrderKind::Board => "Boarding",
        OrderKind::Land => "Setting Down",
        OrderKind::Unload => "Unloading",
        OrderKind::Warp => "Warping",
    }
}

/// `strip`: the construction panel and its queue strip are up beside the card, so what a
/// builder makes, and how far along, shows there instead.
pub fn info(hud: &mut Hud, ui: &mut Ui, s: &Scene, units: &[&UnitInstance], r: Rect, strip: bool) {
    let _t = mc_core::perf_span!("ui.selection_info");
    hud.glass(ui, r);
    let (x, cw) = (r.x + 16.0, r.w - 32.0);
    if let [u] = units {
        single(hud, ui, s, u, r, strip);
        return;
    }
    // A lift ship with units picked out of its hold: the ship's panel stays up,
    // its hold showing which are picked.
    let bases: Vec<&&UnitInstance> = units
        .iter()
        .filter(|u| s.bp(u).transport.is_some())
        .collect();
    if let [base] = bases[..] {
        if units
            .iter()
            .all(|u| u.unit_id == base.unit_id || u.stored())
        {
            single(hud, ui, s, base, r, strip);
            return;
        }
    }

    // Many units: a tile per type, most numerous first.
    let mut types: Vec<(u32, Vec<u32>)> = Vec::new();
    for u in units {
        match types.iter_mut().find(|(bp, _)| *bp == u.blueprint) {
            Some((_, ids)) => ids.push(u.unit_id),
            None => types.push((u.blueprint, vec![u.unit_id])),
        }
    }
    types.sort_by_key(|(bp, ids)| (std::cmp::Reverse(ids.len()), *bp));
    ui.section(
        x,
        r.y + 20.0,
        cw,
        &format!("{} Units Selected", units.len()),
    );
    // Its record, right of the heading, only once there is one: kills, then what its
    // reclaimers have brought in between them.
    let kills: u32 = units.iter().map(|u| u.kill_count()).sum();
    let reclaimed: f32 = units
        .iter()
        .filter(|u| s.bp(u).sends_reclaimers())
        .filter_map(|u| s.queue_of(u).map(|q| q.reclaimed))
        .sum();
    let mut right = x + cw;
    if kills > 0 {
        let kills = format!("{kills} kill{}", if kills == 1 { "" } else { "s" });
        ui.text_right(
            right,
            r.y + 20.0,
            type_scale::MICRO,
            rgb(VETERANCY, 1.0),
            &kills,
        );
        right -= ui.text_width(type_scale::MICRO, &kills) + 14.0;
    }
    if reclaimed >= 0.5 {
        ui.text_right(
            right,
            r.y + 20.0,
            type_scale::MICRO,
            rgb(super::MASS, 1.0),
            &format!("{} materials reclaimed", whole(reclaimed)),
        );
    }
    let health: f32 = units.iter().map(|u| u.health).sum::<f32>() / units.len() as f32;
    bar(
        ui,
        Rect::new(x, r.y + 32.0, cw, 4.0),
        health,
        health_tone(health),
    );
    // What the whole selection makes and spends, in the same strip as for one unit.
    let (mass, energy) = super::economy::total(s, units);
    let economy = super::economy::strip(
        ui,
        mass,
        energy,
        Rect::new(x - 4.0, r.y + 44.0, cw + 8.0, super::economy::STRIP_H),
    );
    let top = if economy { r.y + 76.0 } else { r.y + 48.0 };

    // A selection of core mines only: their investment and one feed for all of them.
    let mines = super::mine::views(s, units);
    if !mines.is_empty() && mines.len() == units.len() {
        super::mine::panel(ui, s, &mines, Rect::new(x, top, cw, super::mine::HEIGHT));
        return;
    }

    let (tw, th, gap, per_row) = (56.0, 64.0, 6.0, 5);
    let rows = if economy { 1 } else { 2 };
    let shown = types.len().min(per_row * rows);
    for (i, (bp_id, ids)) in types.iter().take(shown).enumerate() {
        let bp = s.blueprints.unit(mc_data::BlueprintId(*bp_id as u16));
        let tr = Rect::new(
            x + (i % per_row) as f32 * (tw + gap),
            top + (i / per_row) as f32 * (th + gap),
            tw,
            th,
        );
        let t = hud.tile(ui, id("sel-type", *bp_id as usize), tr, false, true);
        let art = Rect::new(tr.x + 1.0, tr.y + 1.0, tr.w - 2.0, 48.0);
        domain_wash(ui, art, Domain::of(bp), t.glow);
        if !hud.thumbs.draw(
            ui,
            bp.id,
            Rect::new(art.x + (art.w - 48.0) * 0.5, art.y, 48.0, 48.0),
            1.0,
        ) {
            icons::strategic(
                ui,
                bp.visual.icon,
                bp.tech,
                Vec2::new(tr.x + tr.w * 0.5, tr.y + 24.0),
                12.0,
                rgb(palette::TEXT, 0.8 + 0.2 * t.glow),
                ink(0.9),
            );
        }
        ui.text_centred(
            tr.x + tr.w * 0.5,
            tr.bottom() - 11.0,
            type_scale::VALUE,
            rgb(0xFFFFFF, 1.0),
            &ids.len().to_string(),
        );
        // Some of this type paused: the pause mark in the corner.
        if units.iter().any(|u| u.blueprint == *bp_id && u.paused()) {
            super::build::pause_mark(ui, Vec2::new(tr.right() - 10.0, tr.y + 10.0), 14.0);
        }
        if t.clicked {
            ui.audio.play(Sfx::Select);
            // Click keeps only this type; shift-click drops it.
            let keep: Vec<u32> = if s.view.shift {
                units
                    .iter()
                    .filter(|u| u.blueprint != *bp_id)
                    .map(|u| u.unit_id)
                    .collect()
            } else {
                ids.clone()
            };
            hud.actions.push(HudAction::Select {
                units: keep,
                focus: false,
            });
        }
        if t.hovered {
            ui.text_right(
                r.right() - 16.0,
                r.bottom() - 10.0,
                type_scale::MICRO,
                rgb(palette::DIM, 1.0),
                &format!("{}  \u{b7}  Shift-Click Removes", bp.name),
            );
        }
    }
    if types.len() > shown {
        ui.text(
            x,
            r.bottom() - 10.0,
            type_scale::MICRO,
            rgb(palette::FAINT, 1.0),
            &format!("+{} More Types", types.len() - shown),
        );
    }
}

/// Colours of a weapon's kind, for its card: the colour of its ring on the ground.
pub fn weapon_tone(w: &Weapon) -> u32 {
    Reach::of(w).tone()
}

/// A piece of a range ring, centred on `y`, drawn the way the ground draws it:
/// the farthest ring of a kind full, a shorter one of the same kind finer and
/// fainter. With a dead zone its first third is dashed, as the inner edge is.
pub fn ring_swatch(ui: &mut Ui, x: f32, y: f32, w: f32, reach: Reach, rank: u8, dead: bool) {
    let rank = rank.min(crate::rings::RANKS - 1) as f32;
    let (h, a) = (2.0 - 0.6 * rank, 1.0 - 0.28 * rank);
    let tone = rgb(reach.tone(), a);
    let mut from = x;
    if dead {
        let dashed = (w * 0.4).round();
        let mut d = x;
        while d < x + dashed {
            ui.fill(Rect::new(d, y - 0.5, 2.0, 1.0), tone);
            d += 3.5;
        }
        from = x + dashed + 1.5;
    }
    ui.fill(Rect::new(from, y - h * 0.5, x + w - from, h), tone);
}

/// Rings in focus, as bits by `projections` index.
fn bit(ring: usize) -> u64 {
    if ring < 64 {
        1 << ring
    } else {
        0
    }
}

/// How far a row or card of the details card is lit (its rings, `mine`, are in focus)
/// and how far it steps back (other rings are).
fn focus_of(ui: &mut Ui, key: &str, i: usize, focus: u64, mine: u64) -> (f32, f32) {
    let lit = focus != 0 && focus & mine != 0;
    let back = focus != 0 && focus & mine == 0;
    (
        ui.ease(id(key, 2 * i), if lit { 1.0 } else { 0.0 }, 14.0),
        ui.ease(id(key, 2 * i + 1), if back { 1.0 } else { 0.0 }, 12.0),
    )
}

/// The details card's weapons, one card per set of identical mounts (same numbers, same
/// lore): indices into `bp.weapons`, first by where they are listed.
fn weapon_sets(bp: &UnitBlueprint) -> Vec<Vec<usize>> {
    let mut sets: Vec<Vec<usize>> = Vec::new();
    for (i, w) in bp.weapons.iter().enumerate() {
        match sets.iter_mut().find(|s| {
            let o = &bp.weapons[s[0]];
            super::armament::same(o, w) && o.lore == w.lore
        }) {
            Some(set) => set.push(i),
            None => sets.push(vec![i]),
        }
    }
    sets
}

/// Every ring the unit puts on the ground, in its colour and line. Returns the y below.
/// A row under the pointer puts its ring in focus (`hover`); `focus` is last frame's.
fn reach_list(
    ui: &mut Ui,
    bp: &UnitBlueprint,
    x: f32,
    y: f32,
    cw: f32,
    focus: u64,
    hover: &mut u64,
) -> f32 {
    let all = projections(bp);
    if all.is_empty() {
        return y;
    }
    ui.section(x, y, cw, "Reach");
    let mut y = y + 18.0;
    for (i, p) in all.iter().enumerate() {
        let row = Rect::new(x - 6.0, y - 9.0, cw + 12.0, 18.0);
        let res = ui.interact_with(id("reach-row", i), row, true, false);
        if res.hovered {
            *hover = bit(i);
        }
        let (lit, back) = focus_of(ui, "reach-row-focus", i, focus, bit(i));
        if lit > 0.01 {
            ui.fill(row, rgb(p.reach.tone(), 0.14 * lit));
            ui.fill(
                Rect::new(row.x, row.y + 2.0, 2.0, row.h - 4.0),
                rgb(p.reach.tone(), lit),
            );
        }
        let fade = ui.fade;
        ui.fade *= 1.0 - 0.6 * back;
        ring_swatch(
            ui,
            x,
            y,
            22.0,
            p.reach,
            if lit > 0.5 { 0 } else { p.rank },
            p.inner > 0.0,
        );
        ui.text_fit_left(
            x + 32.0,
            y,
            cw * 0.5,
            type_scale::VALUE,
            rgb(p.reach.tone(), 1.0),
            &p.name,
        );
        // A gun that cannot turn all the way round says where it can shoot.
        let kind = match p.arc {
            Some(arc) => format!("{}  \u{b7}  {}", p.reach.label(), arc.label()),
            None => p.reach.label().to_owned(),
        };
        ui.text(
            x + 32.0 + cw * 0.5 + 6.0,
            y,
            type_scale::MICRO,
            rgb(palette::DIM, 1.0),
            &kind,
        );
        let value = if p.inner > 0.0 {
            format!("{:.0}\u{2013}{:.0} m", p.inner, p.outer)
        } else {
            format!("{:.0} m", p.outer)
        };
        ui.text_right(
            x + cw,
            y,
            type_scale::VALUE,
            rgb(palette::TEXT, 1.0),
            &value,
        );
        ui.fade = fade;
        y += 18.0;
    }
    // What the lines mean, when there is more than one way they are drawn here.
    let fine = all.iter().any(|p| p.rank > 0);
    let dead = all.iter().any(|p| p.inner > 0.0);
    if fine || dead {
        let mut lx = x;
        let faint = rgb(palette::FAINT, 1.0);
        ring_swatch(ui, lx, y, 14.0, Reach::Build, 0, false);
        lx = ui.text(lx + 20.0, y, type_scale::MICRO, faint, "Farthest") + 14.0;
        if fine {
            ring_swatch(ui, lx, y, 14.0, Reach::Build, 1, false);
            lx = ui.text(
                lx + 20.0,
                y,
                type_scale::MICRO,
                faint,
                "Shorter Gun, Same Kind",
            ) + 14.0;
        }
        if dead {
            ring_swatch(ui, lx, y, 14.0, Reach::Build, 0, true);
            ui.text(lx + 20.0, y, type_scale::MICRO, faint, "Minimum Range");
        }
        y += 18.0;
    }
    y + 6.0
}

/// Height `reach_list` takes.
fn reach_list_h(bp: &UnitBlueprint) -> f32 {
    let all = projections(bp);
    if all.is_empty() {
        return 0.0;
    }
    let legend = all.iter().any(|p| p.rank > 0 || p.inner > 0.0);
    18.0 + (all.len() + usize::from(legend)) as f32 * 18.0 + 6.0
}

/// A labelled figure with a bar under it: `share` of the bar is filled.
pub(super) fn gauge(
    ui: &mut Ui,
    x: f32,
    y: f32,
    w: f32,
    label: &str,
    value: &str,
    share: f32,
    tone: u32,
) {
    ui.text(x, y, type_scale::MICRO, rgb(palette::DIM, 1.0), label);
    ui.text_right(x + w, y, type_scale::VALUE, rgb(palette::TEXT, 1.0), value);
    let track = Rect::new(x, y + 9.0, w, 3.0);
    ui.fill(track, rgb(tone, 0.14));
    ui.fill(
        Rect::new(track.x, track.y, track.w * share.clamp(0.02, 1.0), track.h),
        rgb(tone, 0.95),
    );
}

fn single(hud: &mut Hud, ui: &mut Ui, s: &Scene, u: &UnitInstance, r: Rect, strip: bool) {
    let bp = s.bp(u);
    let (x, cw) = (r.x + 16.0, r.w - 32.0);
    // The unit's picture on its domain's colour, its strategic icon in the corner.
    let badge = Rect::new(x, r.y + 14.0, 50.0, 50.0);
    domain_wash(ui, badge, Domain::of(bp), 0.3);
    let owner = s.team_color((u.owner_flags & 0xFF) as u8);
    let drew = hud.thumbs.draw(ui, bp.id, badge, 1.0);
    icons::strategic(
        ui,
        bp.visual.icon,
        bp.tech,
        if drew {
            Vec2::new(badge.x + 8.0, badge.y + 8.0)
        } else {
            Vec2::new(badge.x + 25.0, badge.y + 23.0)
        },
        if drew { 5.5 } else { 13.0 },
        owner,
        ink(0.9),
    );
    let tx = badge.right() + 12.0;
    let details = Rect::new(r.right() - 16.0 - 26.0, r.y + 14.0, 26.0, 24.0);
    ui.text_fit_left(
        tx,
        r.y + 24.0,
        details.x - tx - 8.0,
        type_scale::ITEM,
        rgb(0xFFFFFF, 1.0),
        &bp.name,
    );
    // Tier and role; after them what a refitted unit has fitted (the list opens under
    // the pointer), or the volatile chip.
    let sub = format!("T{}  \u{b7}  {}", bp.tech, bp.role);
    let room = r.right() - 16.0 - tx;
    let end = tx + ui.text_width(type_scale::MICRO, &sub).min(room);
    ui.text_fit_left(
        tx,
        r.y + 46.0,
        room,
        type_scale::MICRO,
        rgb(palette::DIM, 1.0),
        &sub,
    );
    let refits = super::refit::icon_row(ui, s.blueprints, bp.id, end + 8.0, r.y + 46.0, 16.0);
    if refits <= 0.0 && bp.volatile() {
        super::volatile::chip(ui, bp, end + 8.0, r.y + 46.0);
    }
    // Details (I) opens lore, figures and every weapon in a card of their own.
    let t = hud.tile(ui, id("unit-details", 0), details, hud.details_open, true);
    let c = Vec2::new(details.x + details.w * 0.5, details.mid_y());
    let ink_tone = rgb(palette::TEXT, 0.7 + 0.3 * t.glow);
    ui.arc(c, 7.0, 0.0, std::f32::consts::TAU, 1.2, ink_tone);
    ui.disc(c + Vec2::new(0.0, -3.2), 1.1, ink_tone);
    ui.fill(Rect::new(c.x - 0.7, c.y - 1.0, 1.4, 5.0), ink_tone);
    if t.hovered {
        super::build::tip(ui, details.x - 40.0, details.y - 32.0, "Details  \u{b7}  I");
    }
    if t.clicked {
        ui.audio.play(Sfx::Tick);
        hud.details_open = !hud.details_open;
    }

    // A lift ship's hold has a panel of its own, right of the order card (`hud/cargo.rs`).
    status_page(
        ui,
        s,
        u,
        bp,
        Rect::new(x, r.y + 82.0, cw, r.bottom() - 10.0 - r.y - 82.0),
        strip && super::build::has_strip(s, bp),
    );
    // The card rises and fades in over the panel, and sinks away when closed.
    let k = ui.ease(
        id("unit-details-card", 0),
        if hud.details_open { 1.0 } else { 0.0 },
        16.0,
    );
    if k > 0.01 {
        let (fade, shift, live) = (ui.fade, ui.shift, ui.interactive);
        ui.fade *= k;
        ui.shift.y += (1.0 - k) * 20.0;
        ui.interactive &= hud.details_open;
        details_card(ui, hud, bp, r);
        (ui.fade, ui.shift, ui.interactive) = (fade, shift, live);
    } else {
        hud.details_focus = 0;
    }
}

/// One weapon: its kind's colour, its name and lore, what it hits, and bars for the numbers.
/// `count` identical mounts share the card, under `name`. `lit` is how far it is in focus
/// (its rings lit on the ground), `back` how far another card is; `arcs` are where its
/// rings reach (`None`: all the way round).
fn weapon_card(
    ui: &mut Ui,
    w: &Weapon,
    name: &str,
    count: usize,
    r: Rect,
    lit: f32,
    back: f32,
    arcs: &[Option<crate::rings::Arc>],
) {
    let tone = weapon_tone(w);
    let fade = ui.fade;
    ui.fade *= 1.0 - 0.55 * back;
    ui.fill_cut(r, 6.0, rgb(tone, 0.06 + 0.12 * lit));
    ui.bevel(r, 6.0, 0.5 * (1.0 - lit));
    if lit > 0.01 {
        ui.outline_cut(r, 6.0, rgb(tone, 0.55 * lit), rgb(tone, lit));
    }
    ui.fill(
        Rect::new(r.x + 1.0, r.y + 8.0, 3.0 + 2.0 * lit, r.h - 16.0),
        rgb(tone, 1.0),
    );
    let (x, cw) = (r.x + 12.0, r.w - 22.0);
    // Where it shoots, top right: the name and chips keep clear of it.
    let glyph = 15.0;
    if !arcs.is_empty() {
        firing_arc(
            ui,
            Vec2::new(r.right() - 10.0 - glyph, r.y + 10.0 + glyph),
            glyph,
            arcs,
            tone,
            lit,
        );
    }
    let (_, figures) = weapon_rows(w);
    let times = format!("\u{d7}{count}");
    let room = cw
        - 2.0 * glyph
        - 8.0
        - if count > 1 {
            ui.text_width(type_scale::CAPTION, &times) + 6.0
        } else {
            0.0
        };
    ui.text_fit_left(
        x,
        r.y + 14.0,
        room,
        type_scale::CAPTION,
        rgb(tone, 1.0),
        name,
    );
    if count > 1 {
        let end = x + ui.text_width(type_scale::CAPTION, name).min(room);
        ui.text(
            end + 6.0,
            r.y + 14.0,
            type_scale::CAPTION,
            rgb(palette::TEXT, 0.85),
            &times,
        );
    }
    // Kind and targets as chips.
    let mut cx = x;
    let kind = figures
        .iter()
        .find(|f| f.0 == "Type")
        .map(|f| f.1.clone())
        .unwrap_or_default();
    let mut chips = vec![(kind, tone)];
    for (bit, label, c) in [
        (cat::LAND, "Land", super::style::LAND),
        (cat::AIR, "Air", super::style::AIR),
        (cat::NAVAL, "Naval", super::style::NAVY),
    ] {
        if w.target_mask & bit != 0 {
            chips.push((label.to_owned(), c));
        }
    }
    if w.splash.to_f32() > 0.0 {
        chips.push((format!("Splash {:.0} m", w.splash.to_f32()), palette::DIM));
    }
    for (label, c) in chips {
        let cw_ = ui.text_width(type_scale::MICRO, &label) + 10.0;
        let chip = Rect::new(cx, r.y + 24.0, cw_, 14.0);
        ui.fill(chip, rgb(c, 0.18));
        ui.text(
            chip.x + 5.0,
            chip.mid_y(),
            type_scale::MICRO,
            rgb(c, 1.0),
            &label,
        );
        cx += cw_ + 4.0;
    }
    let mut y = r.y + 50.0;
    for line in wrap_text(ui, type_scale::MICRO, &w.lore, cw) {
        ui.text(x, y, type_scale::MICRO, rgb(palette::DIM, 1.0), &line);
        y += 15.0;
    }
    y += 4.0;
    let shots = w.salvo.max(1) as f32;
    let reload = w.reload_ticks as f32 * 0.1;
    let damage = if shots > 1.0 {
        format!("{} \u{d7} {}", whole(w.damage.to_f32()), shots as u32)
    } else {
        whole(w.damage.to_f32())
    };
    let range = if w.range_min.to_f32() > 0.0 {
        format!(
            "{:.0}\u{2013}{:.0} m",
            w.range_min.to_f32(),
            w.range_max.to_f32()
        )
    } else {
        format!("{:.0} m", w.range_max.to_f32())
    };
    let rows = [
        // A card of several mounts gives one mount's figures.
        (
            if count > 1 {
                "Damage / s Each"
            } else {
                "Damage / s"
            },
            format!("{:.0}", weapon_dps(w)),
            weapon_dps(w) / 300.0,
        ),
        ("Per Shot", damage, w.damage.to_f32() / 600.0),
        ("Range", range, w.range_max.to_f32() / 1000.0),
        (
            "Reload",
            format!("{reload:.1} s"),
            1.0 - (reload / 10.0).min(0.95),
        ),
    ];
    for (label, value, share) in rows {
        gauge(ui, x, y, cw, label, &value, share, tone);
        y += 22.0;
    }
    ui.fade = fade;
}

/// Where a weapon can shoot, seen from above with the nose up: a faint circle, and
/// the wedge it reaches filled in its colour (the whole disc for a gun that turns
/// all the way round). Brighter while its card is in focus.
fn firing_arc(
    ui: &mut Ui,
    c: Vec2,
    radius: f32,
    arcs: &[Option<crate::rings::Arc>],
    tone: u32,
    lit: f32,
) {
    use std::f32::consts::{FRAC_PI_2, PI, TAU};
    ui.disc(c, radius, rgb(0x000000, 0.35));
    ui.arc(c, radius, 0.0, TAU, 1.0, rgb(palette::LINE, 0.3));
    // Overlapping wedges of a set build up where more mounts reach.
    let fill = rgb(tone, (0.22 + 0.2 * lit) / (arcs.len() as f32).sqrt());
    let at = |a: f32| c + Vec2::new(a.cos(), a.sin()) * (radius - 0.5);
    for arc in arcs {
        // Screen angles run clockwise from +x; the nose is up and `turn` is to the left.
        let (mid, half) = arc.map_or((-FRAC_PI_2, PI), |a| {
            (-FRAC_PI_2 - if a.aft { PI } else { a.turn }, a.half.min(PI))
        });
        let steps = ((half / PI * 32.0).ceil() as usize).max(3);
        for k in 0..steps {
            let (a0, a1) = (
                mid - half + 2.0 * half * k as f32 / steps as f32,
                mid - half + 2.0 * half * (k + 1) as f32 / steps as f32,
            );
            ui.triangle(c, at(a0), at(a1), fill);
        }
        ui.arc(
            c,
            radius,
            mid - half,
            mid + half,
            1.5,
            rgb(tone, 0.85 + 0.15 * lit),
        );
        if half < PI {
            for a in [mid - half, mid + half] {
                ui.stroke(c, at(a), 1.0, rgb(tone, 0.6 + 0.3 * lit));
            }
        }
    }
    // The hull's nose.
    ui.triangle(
        c + Vec2::new(0.0, -4.0),
        c + Vec2::new(-2.5, 2.5),
        c + Vec2::new(2.5, 2.5),
        rgb(palette::TEXT, 0.9),
    );
}

/// Compact dossier for a unit under the pointer that is not the selection.
/// `anchor` is the bottom-left of the card; height is chosen to fit.
/// Returns the rectangle that was drawn.
pub fn hover_card(hud: &Hud, ui: &mut Ui, s: &Scene, u: &UnitInstance, anchor: Rect) -> Rect {
    let bp = s.bp(u);
    let (w, pad) = (anchor.w, 16.0);
    let has_shield = s
        .view
        .frame
        .shields
        .iter()
        .any(|sh| sh.unit_id == u.unit_id);
    let lore = wrap_text(ui, type_scale::BODY, &bp.lore, w - pad * 2.0);
    let rows = 3 + usize::from(has_shield);
    let h = 78.0
        + rows as f32 * 28.0
        + if lore.is_empty() {
            0.0
        } else {
            lore.len() as f32 * 19.0 + 8.0
        };
    let r = Rect::new(anchor.x, anchor.y - h, w, h);
    ui.panel(r);
    let (x, cw) = (r.x + pad, r.w - pad * 2.0);
    hud.thumbs.draw(
        ui,
        bp.id,
        Rect::new(r.right() - 64.0, r.y + 6.0, 56.0, 56.0),
        1.0,
    );
    let owner = s.team_color((u.owner_flags & 0xFF) as u8);
    ui.text(x, r.y + 22.0, type_scale::ITEM, owner, &bp.name);
    let tier = format!("T{}", bp.tech);
    let at = x + ui.text_width(type_scale::MICRO, &tier) + 8.0;
    let refits = super::refit::icon_row(ui, s.blueprints, bp.id, at, r.y + 44.0, 16.0);
    let end = ui.text(
        x,
        r.y + 44.0,
        type_scale::MICRO,
        rgb(palette::TEXT, 1.0),
        &if refits > 0.0 {
            tier
        } else {
            format!("T{}  \u{b7}  {}", bp.tech, bp.role)
        },
    );
    if refits <= 0.0 {
        super::volatile::chip(ui, bp, end + 8.0, r.y + 44.0);
    }

    let level = u.veterancy_level();
    let hp = veterancy_health(bp.health, level).to_f32();
    let mut y = r.y + 68.0;
    for line in &lore {
        ui.text(x, y, type_scale::BODY, rgb(palette::DIM, 1.0), line);
        y += 19.0;
    }
    if !lore.is_empty() {
        y += 8.0;
    }
    ui.text(x, y, type_scale::MICRO, rgb(palette::DIM, 1.0), "Integrity");
    if level > 0 {
        chevrons(ui, Vec2::new(x + 92.0, y), level);
    }
    ui.text_right(
        x + cw,
        y,
        type_scale::VALUE,
        rgb(palette::TEXT, 1.0),
        &format!("{} / {}", whole(u.health * hp), whole(hp)),
    );
    bar(
        ui,
        Rect::new(x, y + 10.0, cw, 5.0),
        u.health,
        health_tone(u.health),
    );
    y += 28.0;

    if let Some(sh) = s
        .view
        .frame
        .shields
        .iter()
        .find(|sh| sh.unit_id == u.unit_id)
    {
        let max = bp.shield.map(|sp| sp.health.to_f32()).unwrap_or(0.0);
        ui.text(x, y, type_scale::MICRO, rgb(palette::DIM, 1.0), "Shield");
        ui.text_right(
            x + cw,
            y,
            type_scale::VALUE,
            rgb(palette::TEXT, 1.0),
            &format!("{} / {}", whole(sh.health * max), whole(max)),
        );
        bar(
            ui,
            Rect::new(x, y + 10.0, cw, 5.0),
            sh.health,
            super::style::AIR,
        );
        y += 28.0;
    }

    let mut facts: Vec<(&str, String, u32)> = Vec::new();
    if !bp.weapons.is_empty() {
        facts.push(("Damage / s", format!("{:.0}", dps(bp)), palette::TEXT));
        facts.push((
            "Range",
            format!("{:.0} m", bp.max_weapon_range().to_f32()),
            palette::TEXT,
        ));
    }
    if let Some(m) = &bp.motion {
        facts.push((
            "Speed",
            format!("{:.0} m/s", m.speed.to_f32()),
            palette::TEXT,
        ));
    }
    facts_grid(ui, &facts, x, y, cw, r.bottom() - 12.0);
    r
}

fn facts_grid(ui: &mut Ui, facts: &[(&str, String, u32)], x: f32, y: f32, cw: f32, floor: f32) {
    let col = cw * 0.5;
    for (i, (label, value, tone)) in facts.iter().enumerate() {
        let (fx, fy) = (x + (i % 2) as f32 * (col + 6.0), y + (i / 2) as f32 * 18.0);
        if fy > floor {
            break;
        }
        ui.text(fx, fy, type_scale::MICRO, rgb(palette::FAINT, 1.0), label);
        ui.text_right(
            fx + col - 8.0,
            fy,
            type_scale::VALUE,
            rgb(*tone, 1.0),
            value,
        );
    }
}

struct Order {
    glyph: Glyph,
    label: &'static str,
    key: &'static str,
    hint: &'static str,
    action: HudAction,
    lit: bool,
}

/// Who carries out an order given to a `bp`: the unit itself, and for a factory the
/// units it makes as well, since each takes the factory's orders as its own.
pub fn ordered_as<'a>(
    blueprints: &'a mc_data::Blueprints,
    bp: &'a UnitBlueprint,
) -> impl Iterator<Item = &'a UnitBlueprint> + 'a {
    let products = match &bp.builder {
        Some(b) if bp.has(cat::FACTORY) => &b.builds[..],
        _ => &[],
    };
    std::iter::once(bp).chain(products.iter().map(|id| blueprints.unit(*id)))
}

/// What the selection can do, family by family. A family nobody selected can
/// use is left off the card; so is any order in it that nobody can carry out.
/// A factory offers what its products can do: they are the ones who will do it.
fn families(s: &Scene, units: &[&UnitInstance]) -> Vec<(Family, Vec<Order>)> {
    let bps: Vec<&UnitBlueprint> = units
        .iter()
        .flat_map(|u| ordered_as(s.blueprints, s.bp(u)))
        .collect();
    let mobile = bps.iter().any(|b| b.is_mobile());
    let air = bps
        .iter()
        .any(|b| b.motion.is_some_and(|m| m.layer == MoveLayer::Air));
    let armed = bps.iter().any(|b| !b.weapons.is_empty());
    let hits_ground = bps.iter().any(|b| {
        b.weapons
            .iter()
            .any(|w| w.target_mask & (cat::LAND | cat::NAVAL | cat::STRUCTURE) != 0)
    });
    let builders = bps.iter().any(|b| b.is_mobile() && b.builder.is_some());
    let reclaimers = units.iter().any(|u| s.bp(u).sends_reclaimers());
    let targeting = |t: Targeting| s.view.mode == Mode::Target(t);
    // A selection of only aircraft that orbit (the Argus) calls its guard an Orbit.
    let orbiters = mobile
        && bps
            .iter()
            .filter(|b| b.is_mobile())
            .all(|b| b.orbit.is_some());
    let guard = || {
        if orbiters {
            return Order { glyph: Glyph::Guard, label: "Orbit", key: "", hint: "Orbit (Ctrl+G): press on a spot, or on a friendly unit to circle it, and drag out the ring. It flies round the ring itself, taking what it can reach and going after enemies that come in. Left with nothing to do, it orbits where it is. Shift queues; shift-drag the centre to move it. Any other order takes it off.", action: HudAction::Target(Targeting::Guard), lit: targeting(Targeting::Guard) };
        }
        Order { glyph: Glyph::Guard, label: "Guard", key: "", hint: "Guard (Ctrl+G): press on a spot, or on a friendly unit to go with it, and drag out the ring. These units are stationed there: they hold it and go after enemies that come in. Aircraft circle it halfway out, a group in formation. Shift queues; shift-drag the centre to move it. Any other order takes a unit off.", action: HudAction::Target(Targeting::Guard), lit: targeting(Targeting::Guard) }
    };
    // The stance most of the armed selection is in.
    let stance = {
        let mut n = [0usize; 3];
        for u in units {
            if ordered_as(s.blueprints, s.bp(u)).any(|b| !b.weapons.is_empty()) {
                n[u.fire_state() as usize] += 1;
            }
        }
        [
            FireState::FireAtWill,
            FireState::HoldPosition,
            FireState::HoldFire,
        ]
        .into_iter()
        .max_by_key(|s| n[*s as usize])
        .unwrap_or_default()
    };

    let mut out = Vec::new();
    let mut movement = Vec::new();
    if mobile {
        movement.push(Order {
            glyph: Glyph::Move,
            label: "Move",
            key: "m",
            hint: "Move to a point. Right-click does the same.",
            action: HudAction::Target(Targeting::Move),
            lit: targeting(Targeting::Move),
        });
        movement.push(Order { glyph: Glyph::Patrol, label: "Patrol", key: "P", hint: "Click a point: the group patrols out to it and back, in formation. Hold shift to add more posts: over a patrol already flown, each goes into the leg nearest the pointer. Shift-drag a post to move it, right-click one to drop it.", action: HudAction::Target(Targeting::Patrol), lit: targeting(Targeting::Patrol) });
    }
    // Capital ships with a warp drive jump: charge off the grid, then out and back in.
    if units.iter().any(|u| s.bp(u).warp.is_some()) {
        movement.push(Order { glyph: Glyph::Warp, label: "Warp", key: "O", hint: "Warp (O): click a point. Each ship stops, charges its drive off the grid (a big draw for a few seconds; slower while energy runs short) and jumps, as far as its drive reaches toward the point. The pointer shows the energy the jump takes. An enemy dampener's field drags a jump into it and throws the ship out hurt and stunned. The drive then recharges.", action: HudAction::Target(Targeting::Warp), lit: targeting(Targeting::Warp) });
    }
    // Unarmed aircraft have no stances; they still circle on guard, as scouts and pickets.
    if air && !armed {
        movement.push(guard());
    }
    if units.iter().any(|u| s.bp(u).is_mobile()) && units.len() > 1 {
        movement.push(Order {
            glyph: Glyph::Formation,
            label: "Formation",
            key: "G",
            hint: "AIR Vs / LAND BLOCKS. SET TRAVEL TOGETHER OR FREE, SPACING, OR FORM UP HERE.",
            action: HudAction::FormationPanel,
            lit: s.view.formation_panel,
        });
    }
    if !movement.is_empty() {
        out.push((Family::Movement, movement));
    }

    let mut combat = Vec::new();
    if armed {
        combat.push(Order {
            glyph: Glyph::Attack,
            label: "Attack",
            key: "",
            hint: "Click an enemy: go after it and fire on it until it dies, whatever the stance.",
            action: HudAction::Target(Targeting::Attack),
            lit: targeting(Targeting::Attack),
        });
        if mobile {
            combat.push(Order {
                glyph: Glyph::AttackMove,
                label: "Atk-Move",
                key: "F",
                hint: "Click a point: move there, stopping to fight whatever comes in range.",
                action: HudAction::Target(Targeting::AttackMove),
                lit: targeting(Targeting::AttackMove),
            });
        }
    }
    if hits_ground {
        combat.push(Order {
            glyph: Glyph::Bombard,
            label: "Bombard",
            key: "K",
            hint: "Click the ground: shell that spot until told otherwise. Press on a centre and drag out a circle: spread the shells over it.",
            action: HudAction::Target(Targeting::Bombard),
            lit: targeting(Targeting::Bombard),
        });
    }
    if !combat.is_empty() {
        out.push((Family::Combat, combat));
    }

    // How the selection behaves between orders. The lit one is the current stance.
    if armed {
        out.push((
            Family::Stance,
            vec![
                Order { glyph: Glyph::FireAtWill, label: "Engage", key: "", hint: "Engage: shoot anything in range and go after enemies seen close by, then come back. The default.", action: HudAction::FireState(FireState::FireAtWill), lit: stance == FireState::FireAtWill },
                Order { glyph: Glyph::HoldPosition, label: "Hold Pos", key: "H", hint: "Hold position: stop here and shoot anything in range, but never chase. Later moves keep it. H again to engage.", action: HudAction::FireState(FireState::HoldPosition), lit: stance == FireState::HoldPosition },
                Order { glyph: Glyph::HoldFire, label: "Hold Fire", key: "Y", hint: "Hold fire: never shoot unless given an attack order. Y again to engage.", action: HudAction::FireState(FireState::HoldFire), lit: stance == FireState::HoldFire },
            ],
        ));
        if mobile {
            if let Some((_, stances)) = out.last_mut() {
                stances.push(guard());
            }
        }
    }
    // A lift ship loads (sets down, ramp lowered), unloads, or once down lifts off:
    // a column of its own, keys shown (a leading shift mark is Shift+key).
    let lifts: Vec<_> = units
        .iter()
        .filter(|u| s.bp(u).transport.is_some())
        .collect();
    // Any other capital ship only sets down or lifts off, and only when told to.
    let landers: Vec<_> = units.iter().filter(|u| s.bp(u).is_capital_ship()).collect();
    if lifts.is_empty() && !landers.is_empty() {
        let down = landers.iter().any(|u| u.set_down());
        let mut land = vec![
            Order { glyph: Glyph::Land, label: "Land", key: "L", hint: "Land (L): click the ground. It glides down onto the nearest ground big and flat enough and stays there until given another order. It sets down only when told to. On the ground its hull cannot turn or pitch, so its spinal gun is silent; its turrets still fire.", action: HudAction::Target(Targeting::Land), lit: targeting(Targeting::Land) },
        ];
        if down {
            land.push(Order { glyph: Glyph::TakeOff, label: "Take Off", key: "\u{21e7}L", hint: "Take off (Shift+L): rise straight up off the ground and climb back to cruise height. Any move order does this too.", action: HudAction::TakeOff, lit: false });
        }
        out.push((Family::Transport, land));
    }
    if !lifts.is_empty() {
        use mc_sim::mirror::LiftPhase;
        let phases: Vec<LiftPhase> = lifts
            .iter()
            .filter_map(|u| {
                s.queue_of(u)
                    .and_then(|q| q.cargo.as_ref())
                    .map(|c| c.phase)
            })
            .collect();
        let down = phases.iter().any(|p| {
            matches!(
                p,
                LiftPhase::RampOpening | LiftPhase::Ready | LiftPhase::Unloading
            )
        });
        let mut lift = vec![
            Order { glyph: Glyph::Board, label: "Load", key: "L", hint: "Load (L): click the ground. It glides down onto the nearest ground big and flat enough and lowers its ramp. Right-click it with land units to board them. Its guns reach the ground only once it is down out of the clouds.", action: HudAction::Target(Targeting::Land), lit: targeting(Targeting::Land) },
            Order { glyph: Glyph::Unload, label: "Unload", key: "U", hint: "Unload (U): click the ground. It sets down there, lowers its ramp and lets the whole hold walk out behind it. Click a unit in the hold to let out just that one.", action: HudAction::Target(Targeting::Unload), lit: targeting(Targeting::Unload) },
        ];
        if down {
            lift.push(Order { glyph: Glyph::TakeOff, label: "Take Off", key: "\u{21e7}L", hint: "Take off (Shift+L): raise the ramp, rise straight up off the ground and climb back to the clouds. Any move order does this too.", action: HudAction::TakeOff, lit: false });
        }
        out.push((Family::Transport, lift));
    }

    let mut work = Vec::new();
    if builders {
        work.push(Order {
            glyph: Glyph::Assist,
            label: "Assist",
            key: "C",
            hint: "Assist (C): click a unit to help it build, repair it or feed its shield. Press on open ground and drag out a ring (a click there gives the usual size) for an area assist: the engineers stay on it and work everything inside, raising what goes up (factories' products too) and mending what is hurt first, reclaiming wrecks only while there is room for the mass, and take up new work as it appears. Stays until another order.",
            action: HudAction::Target(Targeting::Assist),
            lit: targeting(Targeting::Assist),
        });
    }
    if reclaimers {
        work.push(Order {
            glyph: Glyph::Reclaim,
            label: "Reclaim",
            key: "R",
            hint: "Take a wreck, one of your own units or an enemy apart for its materials.",
            action: HudAction::Target(Targeting::Reclaim),
            lit: targeting(Targeting::Reclaim),
        });
    }
    // Submarines dive and surface: the button says which way the selection will go.
    let subs = units.iter().filter(|u| s.bp(u).dive.is_some()).count();
    if subs > 0 {
        let down = units
            .iter()
            .filter(|u| s.bp(u).dive.is_some() && u.dive_goal())
            .count();
        work.push(if down * 2 > subs {
            Order { glyph: Glyph::Surface, label: "Surface", key: "V", hint: "Bring the submarines up. On the surface anything can see and shoot them, and they can see further.", action: HudAction::Dive(false), lit: false }
        } else {
            Order { glyph: Glyph::Dive, label: "Dive", key: "V", hint: "Take the submarines down. Dived, only sonar finds them and only torpedoes reach them.", action: HudAction::Dive(true), lit: false }
        });
    }
    // Builders, factories and anything that upgrades can pause their work.
    work.extend(pause_order::pause_order(s, units));
    work.push(Order { glyph: Glyph::Stop, label: "Stop", key: "X",
 hint: "Drop every order. A factory clears its queue and forgets the orders it hands its units.", action: HudAction::Stop, lit: false });
    out.push((
        if builders || reclaimers {
            Family::Engineering
        } else {
            Family::Control
        },
        work,
    ));
    out
}

/// How many families the order card will show, for laying out the deck.
pub fn order_families(s: &Scene, units: &[&UnitInstance]) -> usize {
    let _t = mc_core::perf_span!("ui.order_families");
    families(s, units).len()
}

pub fn orders(hud: &mut Hud, ui: &mut Ui, s: &Scene, units: &[&UnitInstance], r: Rect) {
    let _t = mc_core::perf_span!("ui.selection_orders");
    hud.glass(ui, r);
    let x = r.x + 14.0;
    let card = families(s, units);
    let mobile = units.iter().any(|u| s.bp(u).is_mobile());

    if mobile && units.len() > 1 && s.view.formation_panel {
        formation_panel(hud, ui, s, r);
    }

    let mut hint = None;
    for (col, (family, orders)) in card.iter().enumerate() {
        let tone = family.tone();
        let cx = x + col as f32 * (ORDER_W + ORDER_GAP);
        // The family's name over its column, in its colour.
        ui.fill(Rect::new(cx, r.y + 12.0, ORDER_W, 2.0), rgb(tone, 0.9));
        ui.text(
            cx,
            r.y + 24.0,
            type_scale::MICRO,
            rgb(tone, 1.0),
            family.label(),
        );
        for (i, o) in orders.iter().enumerate() {
            // Reclaim is lit in the Materials red-orange it brings in, not the family's colour.
            let tone = if o.action == HudAction::Target(Targeting::Reclaim) {
                super::MASS
            } else {
                tone
            };
            let tr = Rect::new(
                cx,
                r.y + 36.0 + i as f32 * (ORDER_H + ORDER_GAP),
                ORDER_W,
                ORDER_H,
            );
            let t = hud.tile(ui, id(o.label, col), tr, false, true);
            let lit_k = ui.ease(id(o.label, col) ^ 3, if o.lit { 1.0 } else { 0.0 }, 16.0);
            // The family colour is a stripe down the left; the chosen one is lit white.
            // Both kept clear of the tile's cut corners.
            ui.fill(
                Rect::new(tr.x + 1.0, tr.y + 7.0, 2.0, tr.h - 14.0),
                rgb(tone, 0.55 + 0.45 * t.glow.max(lit_k)),
            );
            ui.gradient_h(
                Rect::new(tr.x + 3.0, tr.y + 3.0, tr.w * 0.5, tr.h - 6.0),
                rgb(tone, 0.05 + 0.08 * t.glow + 0.12 * lit_k),
                rgb(tone, 0.0),
            );
            let color = rgb(
                if o.lit { 0xFFFFFF } else { tone },
                0.78 + 0.22 * t.glow.max(lit_k),
            );
            icons::glyph(ui, o.glyph, Vec2::new(tr.x + 17.0, tr.mid_y()), 7.5, color);
            ui.text_fit_left(
                tr.x + 31.0,
                tr.mid_y() + 1.0,
                tr.w - 31.0 - 26.0,
                LABEL,
                rgb(palette::TEXT, 0.82 + 0.18 * t.glow.max(lit_k)),
                o.label,
            );
            // A leading shift mark: Shift and the key, the mark drawn left of the cap.
            let mut keys = o.key.chars();
            let (shift, key) = match keys.next() {
                Some('\u{21e7}') => (true, keys.next()),
                first => (false, first),
            };
            if let Some(key) = key {
                let (cx, cy) = (tr.right() - 21.0, tr.mid_y() - 7.5);
                super::build::key_cap(ui, cx, cy, key, o.lit || t.hovered);
                if shift {
                    shift_mark(
                        ui,
                        Vec2::new(cx - 7.0, cy + 7.5),
                        rgb(palette::TEXT, if o.lit || t.hovered { 1.0 } else { 0.7 }),
                    );
                }
            }
            if t.clicked {
                ui.audio.play(Sfx::Select);
                hud.actions.push(o.action.clone());
            }
            if t.hovered {
                hint = Some(o.hint);
            }
        }
    }
    if let Some(hint) = hint {
        tip(ui, r.x, r.y - 34.0, hint);
    }
}

/// The shift key's mark, an outlined arrow up, centred on `c`: before a key cap it means Shift+key.
pub(in crate::hud) fn shift_mark(ui: &mut Ui, c: Vec2, color: crate::ui::Color) {
    let (top, w, stem) = (c + Vec2::new(0.0, -5.5), 4.5, 2.2);
    let points = [
        top,
        c + Vec2::new(w, -0.5),
        c + Vec2::new(stem, -0.5),
        c + Vec2::new(stem, 5.0),
        c + Vec2::new(-stem, 5.0),
        c + Vec2::new(-stem, -0.5),
        c + Vec2::new(-w, -0.5),
    ];
    ui.polyline(&points, 1.1, color, true);
}

fn formation_panel(hud: &mut Hud, ui: &mut Ui, s: &Scene, r: Rect) {
    let cw = r.w - 24.0;
    let panel = Rect::new(r.x, r.y - 154.0, r.w, 144.0);
    hud.glass(ui, panel);
    ui.section(
        panel.x + 12.0,
        panel.y + 18.0,
        panel.w - 24.0,
        "Formation \u{b7} Selection",
    );
    let choices = [
        (
            "Together",
            HudAction::FormationTogether(true),
            s.view.formation_together,
        ),
        (
            "Free Move",
            HudAction::FormationTogether(false),
            !s.view.formation_together,
        ),
    ];
    for (i, (label, action, lit)) in choices.into_iter().enumerate() {
        let button = Rect::new(
            panel.x + 12.0 + i as f32 * (cw + 4.0) / 2.0,
            panel.y + 29.0,
            (cw - 4.0) / 2.0,
            27.0,
        );
        let t = hud.tile(ui, id("formation-mode", i), button, lit, true);
        let _ = t.glow;
        ui.text_centred(
            button.x + button.w * 0.5,
            button.mid_y(),
            type_scale::MICRO,
            rgb(palette::TEXT, 1.0),
            label,
        );
        if t.clicked {
            hud.actions.push(action);
            ui.audio.play(Sfx::Select);
        }
    }
    for (i, label) in ["Compact", "Standard", "Wide"].into_iter().enumerate() {
        let button = Rect::new(
            panel.x + 12.0 + i as f32 * (cw + 4.0) / 3.0,
            panel.y + 62.0,
            (cw - 8.0) / 3.0,
            25.0,
        );
        let t = hud.tile(
            ui,
            id("formation-spacing", i),
            button,
            s.view.formation_spacing == i as u8,
            true,
        );
        ui.text_centred(
            button.x + button.w * 0.5,
            button.mid_y(),
            type_scale::MICRO,
            rgb(palette::TEXT, 1.0),
            label,
        );
        if t.clicked {
            hud.actions.push(HudAction::FormationSpacing(i as u8));
            ui.audio.play(Sfx::Select);
        }
    }
    let button = Rect::new(panel.x + 12.0, panel.y + 94.0, cw, 26.0);
    let t = hud.tile(ui, id("form-up", 0), button, false, true);
    ui.text_centred(
        button.x + button.w * 0.5,
        button.mid_y(),
        type_scale::MICRO,
        rgb(palette::TEXT, 1.0),
        "Form UP Here",
    );
    if t.clicked {
        hud.actions.push(HudAction::FormUp);
        ui.audio.play(Sfx::Select);
    }
    ui.text(
        panel.x + 12.0,
        panel.y + 132.0,
        type_scale::MICRO,
        rgb(palette::DIM, 1.0),
        "AIR: REPEATING Vs    LAND: BLOCKS",
    );
}
