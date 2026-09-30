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
use mc_data::{BlueprintId, UnitBlueprint};
use mc_render::Face;
use mc_sim::mirror::KIND_WRECK;

/// The digit on a unit's badge.
const BADGE_DIGIT: Style = style(Face::Bold, 10.5, 0.0);
const BADGE: f32 = 13.0;

/// The type a group has most of (the lower blueprint id on a tie).
fn main_type<'a>(s: &'a Scene, members: &[u32]) -> Option<&'a UnitBlueprint> {
    let mut counts: Vec<(u16, usize)> = Vec::new();
    for id in members {
        let Some(&i) = s.view.index_of.get(id) else {
            continue;
        };
        let bp = s.view.frame.units[i].blueprint as u16;
        match counts.iter_mut().find(|(b, _)| *b == bp) {
            Some((_, n)) => *n += 1,
            None => counts.push((bp, 1)),
        }
    }
    counts
        .iter()
        .max_by_key(|&&(bp, n)| (n, std::cmp::Reverse(bp)))
        .map(|&(bp, _)| s.blueprints.unit(BlueprintId(bp)))
}

/// The groups card, from `top` and kept above `bottom`. Returns the y under it.
pub(super) fn card(hud: &mut Hud, ui: &mut Ui, s: &Scene, top: f32, bottom: f32) -> f32 {
    let groups: Vec<(usize, Vec<u32>)> = s
        .view
        .groups
        .filled()
        .map(|(n, m)| (n, m.to_vec()))
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
    for (i, (n, members)) in groups.iter().enumerate() {
        let tr = Rect::new(
            left + (i % per_row) as f32 * (TILE + TILE_GAP),
            r.y + PAD + (i / per_row) as f32 * (TILE + TILE_GAP),
            TILE,
            TILE,
        );
        let lit = same_units(&s.view.selection, members);
        let t = hud.tile(ui, id("group-tile", *n), tr, lit, true);
        let art = Rect::new(tr.x + 1.0, tr.y + 1.0, tr.w - 2.0, tr.h - 2.0);
        if let Some(bp) = main_type(s, members) {
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
                for id in members {
                    if !units.contains(id) {
                        units.push(*id);
                    }
                }
                HudAction::Select {
                    units: units.to_vec(),
                    focus: false,
                }
            } else {
                HudAction::Select {
                    units: members.clone(),
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
pub(super) fn badges(ui: &mut Ui, s: &Scene) {
    let _t = mc_core::perf_span!("ui.group_badges");
    const EACH_MOST: usize = 64;
    let viewport = s.camera.viewport;
    let eye = s.camera.eye();
    let scale = s.camera.projection_scale();
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
    let mut shown: Vec<Vec2> = Vec::new();
    for (n, members) in s.view.groups.filled() {
        let digit = n.to_string();
        shown.clear();
        for id in members {
            let Some(&i) = s.view.index_of.get(id) else {
                continue;
            };
            let u = &s.view.frame.units[i];
            if u.owner_flags & KIND_WRECK != 0 || has_flag(u, flag::IN_FACTORY) {
                continue;
            }
            let at = Vec3::from(u.pos);
            let Some(p) = s.camera.project(at) else {
                continue;
            };
            if p.x < -40.0 || p.y < -40.0 || p.x > viewport.x + 40.0 || p.y > viewport.y + 40.0 {
                continue;
            }
            // The bars' size, in window pixels, as the shader works it out.
            let px = scale / eye.distance(at).max(1.0);
            let half_w = (u.radius * 0.8 * px).max(16.0);
            let below = (u.radius * 0.6 * px).max(6.0) + 2.0;
            shown.push(Vec2::new(
                (p.x - half_w - 3.0) / ui.s,
                (p.y + below + 3.0) / ui.s,
            ));
        }
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
