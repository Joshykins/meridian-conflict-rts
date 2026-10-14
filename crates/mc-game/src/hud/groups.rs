//! Control groups on screen (the keys are in `game_groups.rs`): a card in the
//! left column under the idle cards, a tile per group, and the group's number
//! beside each member's bars on the battlefield.
//!
//! A tile shows the group's key, a picture of the type it has most of, and how
//! many are in it. Click: select the group; again once selected: find it.
//! Shift-click: add it to the selection.

use super::builders::{card_height, per_row, same_units, LABEL_W, PAD, TILE, TILE_GAP};
use super::style::{domain_wash, Domain};
use super::{build, flag, has_flag, icons, Hud, HudAction, Scene};
use crate::audio::Sfx;
use crate::ui::{id, ink, palette, rgb, style, type_scale, Rect, Style, Ui};
use glam::{Vec2, Vec3};
use mc_data::BlueprintId;
use mc_render::Face;
use mc_sim::mirror::{UnitInstance, KIND_WRECK};

/// The digit on a unit's badge.
const BADGE_DIGIT: Style = style(Face::Bold, 10.5, 0.0);
const BADGE: f32 = 13.0;

/// Each control group's members as found in the frame: worked out once per pull and
/// whenever the groups or the selection change, not every frame, for groups of tens of
/// thousands.
#[derive(Default)]
pub(super) struct GroupsKept {
    /// The pull and the selection (`Picked::stamp`) they were found for.
    key: Option<(u64, u64)>,
    groups: Vec<Found>,
}

struct Found {
    /// Its key.
    n: usize,
    /// The group as it was when found, to tell when it changes.
    ids: Vec<u32>,
    /// Its members' indices in `frame.units`.
    rows: Vec<u32>,
    /// The box round where those of them that wear the number stand (`badges`).
    reach: Option<(Vec3, Vec3)>,
    /// The type it has most of (the lower blueprint id on a tie).
    main: Option<BlueprintId>,
    /// It is the selection.
    lit: bool,
}

impl GroupsKept {
    fn found(&mut self, s: &Scene) -> &[Found] {
        let view = s.view;
        let key = (view.pulls(), view.picked().stamp());
        let mut groups = view.groups.filled();
        // Never pulled (a view filled by hand): every time, as it may have changed.
        let same = view.pulls() > 0
            && self.key == Some(key)
            && self.groups.iter().all(|f| {
                groups
                    .next()
                    .is_some_and(|(n, ids)| n == f.n && ids == f.ids)
            })
            && groups.next().is_none();
        if !same {
            self.key = Some(key);
            self.groups = view
                .groups
                .filled()
                .map(|(n, ids)| find(s, n, ids))
                .collect();
        }
        &self.groups
    }
}

fn find(s: &Scene, n: usize, ids: &[u32]) -> Found {
    let view = s.view;
    let rows: Vec<u32> = (ids.iter())
        .filter_map(|&id| view.unit_index(id))
        .map(|i| i as u32)
        .collect();
    let mut counts: Vec<(u16, usize)> = Vec::new();
    let mut reach: Option<(Vec3, Vec3)> = None;
    for &i in &rows {
        let u = &view.frame.units[i as usize];
        let bp = u.blueprint as u16;
        match counts.iter_mut().find(|(b, _)| *b == bp) {
            Some((_, n)) => *n += 1,
            None => counts.push((bp, 1)),
        }
        if u.owner_flags & KIND_WRECK == 0 && !has_flag(u, flag::IN_FACTORY) {
            let at = Vec3::from(u.pos);
            let (lo, hi) = reach.unwrap_or((at, at));
            reach = Some((lo.min(at), hi.max(at)));
        }
    }
    let main = (counts.iter())
        .max_by_key(|&&(bp, n)| (n, std::cmp::Reverse(bp)))
        .map(|&(bp, _)| BlueprintId(bp));
    Found {
        n,
        ids: ids.to_vec(),
        rows,
        reach,
        main,
        lit: same_units(&view.selection, ids),
    }
}

/// The groups card, from `top` and kept above `bottom`. Returns the y under it.
pub(super) fn card(hud: &mut Hud, ui: &mut Ui, s: &Scene, top: f32, bottom: f32) -> f32 {
    let groups: Vec<(usize, &[u32])> = s.view.groups.filled().collect();
    // What each looks like, kept from when its members were last found.
    let looks: Vec<(Option<BlueprintId>, bool)> = (hud.kept.groups.found(s).iter())
        .map(|f| (f.main, f.lit))
        .collect();
    let r = Rect::new(
        super::EDGE,
        top + super::GAP,
        super::COMMANDER_W,
        card_height(groups.len()),
    );
    // Deliberate, as for the idle cards: no room, no card.
    if r.bottom() > bottom {
        return top;
    }
    hud.glass(ui, r);
    let label = Rect::new(r.x + PAD, r.y + PAD, LABEL_W, TILE);
    let x = label.x + 6.0;
    ui.text(
        x,
        label.y + 11.0,
        type_scale::MICRO,
        rgb(palette::TEXT, 0.9),
        "Groups",
    );
    if groups.is_empty() {
        ui.text_fit_left(
            label.right() + PAD,
            label.mid_y(),
            r.right() - label.right() - 2.0 * PAD,
            type_scale::MICRO,
            rgb(palette::FAINT, 1.0),
            "Ctrl + 1\u{2013}0 makes one",
        );
        let hover = ui.interact(id("groups-hint", 0), r, true);
        if hover.hovered {
            tip(ui, r, r.y, "Ctrl + a number makes the selection a group");
        }
        return r.bottom();
    }
    let units: usize = groups.iter().map(|(_, m)| m.len()).sum();
    ui.text_fit_left(
        x,
        label.y + 27.0,
        LABEL_W - 8.0,
        type_scale::MICRO,
        rgb(palette::DIM, 1.0),
        &format!("{units} units"),
    );

    let per_row = per_row();
    let left = label.right() + PAD;
    for (i, (&(n, members), &(main, lit))) in groups.iter().zip(&looks).enumerate() {
        let tr = Rect::new(
            left + (i % per_row) as f32 * (TILE + TILE_GAP),
            r.y + PAD + (i / per_row) as f32 * (TILE + TILE_GAP),
            TILE,
            TILE,
        );
        let t = hud.tile(ui, id("group-tile", n), tr, lit, true);
        let art = Rect::new(tr.x + 1.0, tr.y + 1.0, tr.w - 2.0, tr.h - 2.0);
        if let Some(bp) = main.map(|bp| s.blueprints.unit(bp)) {
            domain_wash(ui, art, Domain::of(bp), t.glow);
            if !hud.thumbs.draw(ui, bp.id, art, 1.0) {
                icons::strategic(
                    ui,
                    bp.visual.icon,
                    bp.tech,
                    Vec2::new(art.x + art.w * 0.5, art.mid_y()),
                    10.0,
                    rgb(palette::TEXT, 0.8 + 0.2 * t.glow),
                    ink(0.9),
                );
            }
        }
        // The key in the top corner, how many in the bottom one.
        ui.key_cap(tr.x + 1.0, tr.y + 1.0, &n.to_string(), true);
        ui.fill(
            Rect::new(tr.right() - 16.0, tr.bottom() - 13.0, 15.0, 12.0),
            ink(0.6),
        );
        ui.text_right(
            tr.right() - 3.0,
            tr.bottom() - 7.0,
            type_scale::MICRO,
            rgb(palette::TEXT, 1.0),
            &members.len().to_string(),
        );
        if t.clicked {
            ui.audio.play(Sfx::Select);
            let action = if s.view.shift {
                let mut units = s.view.selection.clone();
                units.extend(members.iter().copied());
                HudAction::Select {
                    units: units.to_vec(),
                    focus: false,
                }
            } else {
                HudAction::Select {
                    units: members.to_vec(),
                    focus: lit,
                }
            };
            hud.actions.push(action);
        }
        if t.hovered {
            tip(
                ui,
                r,
                tr.y,
                &format!(
                    "Group {n}  \u{b7}  {n} selects it, twice finds it  \u{b7}  Ctrl+{n} sets it  \u{b7}  Ctrl+Shift+{n} adds to it"
                ),
            );
        }
    }
    r.bottom()
}

/// The hint for the card, beside it so it covers nothing on it.
fn tip(ui: &mut Ui, card: Rect, y: f32, text: &str) {
    build::tip(ui, card.right() + 8.0, y, text);
}

/// Each group member's number on the battlefield: a small key cap left of
/// where its bars sit (`icons.wgsl` `vs_bar`: on the ground in front of the
/// hull, as wide as it). A group with more than `EACH_MOST` members on screen
/// wears one cap in the middle of them instead: a cap on each of thousands is a
/// carpet of digits, and costs more than the rest of the interface together.
pub(super) fn badges(kept: &mut GroupsKept, ui: &mut Ui, s: &Scene) {
    let _t = mc_core::perf_span!("ui.group_badges");
    const EACH_MOST: usize = 64;
    // A group of more members than this finds out first from a share of them whether
    // more than `EACH_MOST` are on screen; when they are, its one cap goes in the middle
    // of that share's. A deliberate cosmetic cap: the middle of thousands moves by less
    // than a pixel, and looking at each of them every frame cost more than the rest of
    // the interface.
    const SHARE: usize = 4096;
    let viewport = s.camera.viewport;
    let eye = s.camera.eye();
    let scale = s.camera.projection_scale();
    let projector = s.camera.projector();
    let cap = |ui: &mut Ui, at: Vec2, digit: &str| {
        let r = Rect::new(at.x - BADGE, at.y - BADGE * 0.5, BADGE, BADGE);
        ui.fill(r, ink(0.8));
        ui.frame(r, rgb(0xFFFFFF, 0.85));
        ui.text_centred(
            r.x + r.w * 0.5 + 0.5,
            r.mid_y(),
            BADGE_DIGIT,
            rgb(0xFFFFFF, 1.0),
            digit,
        );
    };
    let away = |p: Vec2| {
        [
            p.x < -40.0,
            p.y < -40.0,
            p.x > viewport.x + 40.0,
            p.y > viewport.y + 40.0,
        ]
    };
    // Where a member's number goes, if it is on screen: left of where its bars sit.
    let points = ui.s;
    let badge_at = |u: &UnitInstance| {
        if u.owner_flags & KIND_WRECK != 0 || has_flag(u, flag::IN_FACTORY) {
            return None;
        }
        let at = Vec3::from(u.pos);
        let p = projector.project(at)?;
        if away(p).into_iter().any(|a| a) {
            return None;
        }
        // The bars' size, in window pixels, as the shader works it out.
        let px = scale / eye.distance(at).max(1.0);
        let half_w = (u.radius * 0.8 * px).max(16.0);
        let below = (u.radius * 0.6 * px).max(6.0) + 2.0;
        Some(Vec2::new(
            (p.x - half_w - 3.0) / points,
            (p.y + below + 3.0) / points,
        ))
    };
    let units = &s.view.frame.units;
    let mut shown: Vec<Vec2> = Vec::new();
    for g in kept.found(s) {
        let Some((lo, hi)) = g.reach else {
            continue;
        };
        // Its box all past one edge of the screen: so is every member.
        let corners: Option<Vec<[bool; 4]>> = (0..8)
            .map(|k| {
                let pick = |bit: usize, a: f32, b: f32| if k & bit == 0 { a } else { b };
                let c = Vec3::new(
                    pick(1, lo.x, hi.x),
                    pick(2, lo.y, hi.y),
                    pick(4, lo.z, hi.z),
                );
                projector.project(c).map(away)
            })
            .collect();
        if corners.is_some_and(|c| (0..4).any(|side| c.iter().all(|a| a[side]))) {
            continue;
        }
        let digit = g.n.to_string();
        if g.rows.len() > SHARE {
            let step = g.rows.len().div_ceil(SHARE);
            let (mut n, mut sum) = (0usize, Vec2::ZERO);
            for &i in g.rows.iter().step_by(step) {
                if let Some(at) = badge_at(&units[i as usize]) {
                    n += 1;
                    sum += at;
                }
            }
            if n > EACH_MOST {
                cap(ui, sum / n as f32, &digit);
                continue;
            }
        }
        shown.clear();
        shown.extend(g.rows.iter().filter_map(|&i| badge_at(&units[i as usize])));
        if shown.len() > EACH_MOST {
            let middle = shown.iter().copied().sum::<Vec2>() / shown.len() as f32;
            cap(ui, middle, &digit);
        } else {
            for &at in &shown {
                cap(ui, at, &digit);
            }
        }
    }
}
