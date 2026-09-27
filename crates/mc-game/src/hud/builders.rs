//! The left column under the economy: the commander's card, then a card for idle
//! engineers and one for idle factories. All three say "idle" the same way: an
//! amber bar down the card's left edge and an amber dot before the word, pulsing
//! together.
//!
//! An idle card is one short strip: its title and idle count on the left, then a
//! small tile per type, tier included (a T1 and a T2 engineer are two tiles). Click
//! a tile: the next idle one of that type, the camera following. Shift-click: every
//! idle one of that type (again: the camera goes to them). The title block does the
//! same for every idle unit on the card.

use std::collections::HashMap;

use super::style::{domain_wash, Domain};
use super::{
    build, flag, has_flag, icons, refit, selection, whole, Hud, HudAction, Scene, HEALTHY,
};
use crate::audio::Sfx;
use crate::ui::{id, ink, palette, rgb, type_scale, Rect, Ui};
use glam::Vec2;
use mc_data::{cat, UnitBlueprint};
use mc_sim::mirror::{KIND_WRECK, STATE_IDLE};

/// Inside margin of an idle card.
const PAD: f32 = 8.0;
/// The label block on the card's left: the title over the idle count.
const LABEL_W: f32 = 72.0;
/// Square tiles in rows to the label's right.
const TILE: f32 = 36.0;
const TILE_GAP: f32 = 4.0;

#[derive(Default)]
pub(super) struct Builders {
    /// Which idle unit a right-click goes to next, by card (0 engineers, 1
    /// factories) and blueprint, `u32::MAX` standing for the card's ALL.
    next: HashMap<(u8, u32), usize>,
    /// 1 when the commander was hit, fading.
    commander_hit: f32,
}

/// The idle pulse, shared so every card's bar and dot beat together.
fn pulse(ui: &Ui) -> f32 {
    0.55 + 0.45 * (ui.time * 3.0).sin().abs()
}

/// The amber bar down an idle card's left edge, clear of its cut corners.
fn idle_edge(ui: &mut Ui, r: Rect, k: f32) {
    let edge = Rect::new(r.x, r.y + 10.0, 3.0, r.h - 20.0);
    ui.fill(edge, rgb(palette::WARN, k));
    ui.gradient_h(
        Rect::new(r.x + 3.0, edge.y, 36.0, edge.h),
        rgb(palette::WARN, 0.12 * k),
        rgb(palette::WARN, 0.0),
    );
}

/// The status line: a dot and the words, in `tone`.
fn status_line(ui: &mut Ui, x: f32, y: f32, w: f32, text: &str, tone: u32, k: f32) {
    ui.fill(Rect::new(x, y - 3.0, 5.0, 5.0), rgb(tone, k));
    ui.text_fit_left(x + 12.0, y, w - 12.0, type_scale::MICRO, rgb(tone, k), text);
}

/// The player's commander, always on show: its picture, health, what it is
/// doing. It flashes when hit and pulses when idle; a click selects it and
/// brings the camera to it.
pub(super) fn commander_card(hud: &mut Hud, ui: &mut Ui, s: &Scene, r: Rect, dt: f32) -> bool {
    let Some(u) = s.view.frame.units.iter().find(|u| {
        (u.owner_flags & 0xFF) as u8 == s.view.local
            && u.owner_flags & KIND_WRECK == 0
            && s.bp(u).has(cat::COMMANDER)
    }) else {
        return false;
    };
    let bp = s.bp(u);
    hud.claim(ui, r);
    let me = &mut hud.builders;
    if has_flag(u, flag::HURT) {
        me.commander_hit = 1.0;
    }
    me.commander_hit = (me.commander_hit - dt * 1.4).max(0.0);
    let hit = me.commander_hit;
    let res = ui.interact(id("commander-card", 0), r, true);
    ui.panel(r);
    if hit > 0.0 {
        let blink = 0.5 + 0.5 * (ui.time * 18.0).sin();
        ui.fill_cut(r, 10.0, rgb(palette::BAD, 0.25 * hit * blink));
        ui.bevel(r, 10.0, hit);
    }
    ui.fill_cut(r, 10.0, rgb(0xFFFFFF, 0.05 * res.glow));
    if res.clicked {
        ui.audio.play(Sfx::Select);
        hud.actions.push(HudAction::Select {
            units: vec![u.unit_id],
            focus: true,
        });
    }
    let idle = u.owner_flags & STATE_IDLE != 0 && hit == 0.0;
    if idle {
        idle_edge(ui, r, pulse(ui));
    }
    // Its picture on the left.
    let pic = Rect::new(r.x + 8.0, r.y + 8.0, r.h - 16.0, r.h - 16.0);
    domain_wash(ui, pic, Domain::of(bp), 0.2 + 0.3 * res.glow);
    if !hud.thumbs.draw(ui, bp.id, pic, 1.0) {
        icons::strategic(
            ui,
            bp.visual.icon,
            bp.tech,
            Vec2::new(pic.x + pic.w * 0.5, pic.mid_y()),
            14.0,
            s.team_color(s.view.local),
            ink(0.9),
        );
    }
    let x = pic.right() + 12.0;
    let cw = r.right() - 12.0 - x;
    let level = u.veterancy_level();
    ui.text_fit_left(
        x,
        r.y + 17.0,
        cw - 64.0,
        type_scale::CAPTION,
        rgb(0xFFFFFF, 1.0),
        &bp.name,
    );
    if level > 0 {
        selection::chevrons(ui, Vec2::new(r.right() - 58.0, r.y + 17.0), level);
    }
    if hit > 0.0 {
        status_line(
            ui,
            x,
            r.y + 36.0,
            cw - 70.0,
            "Under Fire",
            palette::BAD,
            1.0,
        );
    } else if idle {
        status_line(
            ui,
            x,
            r.y + 36.0,
            cw - 70.0,
            "Idle",
            palette::WARN,
            pulse(ui),
        );
    } else {
        let doing = s
            .queue_of(u)
            .and_then(|q| q.orders.first())
            .map_or("Working", |o| selection::activity(o.kind));
        status_line(ui, x, r.y + 36.0, cw - 70.0, doing, palette::DIM, 1.0);
    }
    let hp = mc_sim::veterancy_health(bp.health, level).to_f32();
    let tone = if u.health > 0.6 {
        HEALTHY
    } else if u.health > 0.3 {
        palette::WARN
    } else {
        palette::BAD
    };
    ui.text_right(
        x + cw,
        r.y + 36.0,
        type_scale::VALUE,
        rgb(tone, 1.0),
        &whole(u.health * hp),
    );
    let track = Rect::new(x, r.y + 50.0, cw, 5.0);
    ui.fill(track, rgb(palette::LINE, 0.12));
    ui.fill(
        Rect::new(
            track.x,
            track.y,
            track.w * u.health.clamp(0.0, 1.0),
            track.h,
        ),
        rgb(tone, 1.0),
    );
    // What it has fitted, as icons; the list shows over them.
    if refit::icon_row(ui, s.blueprints, bp.id, x, r.y + 72.0, 17.0) == 0.0 {
        ui.text(
            x,
            r.y + 72.0,
            type_scale::MICRO,
            rgb(palette::FAINT, 1.0),
            "No refits",
        );
    }
    if res.hovered {
        // Beside the card: under it are the idle cards.
        build::tip(
            ui,
            r.right() + 8.0,
            r.y,
            "Click selects and finds your commander  \u{b7}  Home",
        );
    }
    true
}

/// One kind of idle unit: its blueprint and the idle ones, in frame order.
type Kind<'a> = (&'a UnitBlueprint, Vec<u32>);

/// Idle engineers (not the commander, which has its own card) and idle factories,
/// each by type, in the order the cards show them.
fn idle_kinds<'a>(s: &'a Scene) -> [Vec<Kind<'a>>; 2] {
    let mut kinds: [Vec<Kind<'a>>; 2] = [Vec::new(), Vec::new()];
    for u in &s.view.frame.units {
        let mine = (u.owner_flags & 0xFF) as u8 == s.view.local && u.owner_flags & KIND_WRECK == 0;
        if !mine
            || u.owner_flags & STATE_IDLE == 0
            || has_flag(u, flag::UNDER_CONSTRUCTION | flag::IN_FACTORY)
        {
            continue;
        }
        let bp = s.bp(u);
        let card = if bp.has(cat::ENGINEER) && !bp.has(cat::COMMANDER) {
            0
        } else if bp.has(cat::FACTORY) {
            1
        } else {
            continue;
        };
        match kinds[card].iter_mut().find(|(k, _)| k.id == bp.id) {
            Some((_, ids)) => ids.push(u.unit_id),
            None => kinds[card].push((bp, vec![u.unit_id])),
        }
    }
    // Land, air, then sea; by tier within each. The blueprint id is unique, so no ties.
    let rank = |bp: &UnitBlueprint| {
        let domain = match Domain::of(bp) {
            Domain::Land => 0,
            Domain::Air => 1,
            Domain::Navy => 2,
            Domain::Both => 3,
            Domain::Support => 4,
        };
        (domain, bp.tech, bp.id.0)
    };
    for card in &mut kinds {
        card.sort_unstable_by_key(|(bp, _)| rank(bp));
    }
    kinds
}

/// How tall an idle card is with `types` tiles.
fn card_height(types: usize) -> f32 {
    let rows = types.div_ceil(per_row()).max(1);
    2.0 * PAD + rows as f32 * (TILE + TILE_GAP) - TILE_GAP
}

fn per_row() -> usize {
    ((super::COMMANDER_W - 3.0 * PAD - LABEL_W + TILE_GAP) / (TILE + TILE_GAP)) as usize
}

/// The idle engineer and factory cards, stacked down from `top` and kept above
/// `bottom`. Returns the y under the last one drawn.
pub(super) fn idle_cards(hud: &mut Hud, ui: &mut Ui, s: &Scene, top: f32, bottom: f32) -> f32 {
    let mut y = top;
    for (card, kinds) in idle_kinds(s).into_iter().enumerate() {
        if kinds.is_empty() {
            continue;
        }
        let r = Rect::new(
            super::EDGE,
            y + super::GAP,
            super::COMMANDER_W,
            card_height(kinds.len()),
        );
        // Deliberate: a card with no room left (the test range's tall panel on a
        // short window) is not drawn, rather than drawn over the deck.
        if r.bottom() > bottom {
            break;
        }
        idle_card(hud, ui, s, card as u8, r, &kinds);
        y = r.bottom();
    }
    y
}

fn idle_card(hud: &mut Hud, ui: &mut Ui, s: &Scene, card: u8, r: Rect, kinds: &[Kind]) {
    hud.glass(ui, r);
    let k = pulse(ui);
    idle_edge(ui, r, k);
    let (title, every) = if card == 0 {
        ("Engineers", "every idle engineer")
    } else {
        ("Factories", "every idle factory")
    };
    let all: Vec<u32> = kinds
        .iter()
        .flat_map(|(_, ids)| ids.iter().copied())
        .collect();

    // The title block, which picks from the whole card.
    let label = Rect::new(r.x + PAD, r.y + PAD, LABEL_W, TILE);
    let res = ui.interact(id("idle-all", card as usize), label, true);
    ui.fill_cut(label, 5.0, rgb(0xFFFFFF, 0.05 * res.glow));
    let x = label.x + 6.0;
    ui.text_fit_left(
        x,
        label.y + 11.0,
        LABEL_W - 8.0,
        type_scale::MICRO,
        rgb(palette::TEXT, 0.8 + 0.2 * res.glow),
        title,
    );
    status_line(
        ui,
        x,
        label.y + 27.0,
        LABEL_W - 8.0,
        &format!("{} Idle", all.len()),
        palette::WARN,
        k,
    );
    let lit = same_units(&s.view.selection, &all);
    pick(hud, ui, s, (card, u32::MAX), &all, lit, res.clicked);
    if res.hovered {
        tip(ui, r, r.y, every);
    }

    let per_row = per_row();
    let left = label.right() + PAD;
    for (i, (bp, ids)) in kinds.iter().enumerate() {
        let tr = Rect::new(
            left + (i % per_row) as f32 * (TILE + TILE_GAP),
            r.y + PAD + (i / per_row) as f32 * (TILE + TILE_GAP),
            TILE,
            TILE,
        );
        let lit = same_units(&s.view.selection, ids);
        let tile_id = id("idle-type", ((card as usize) << 16) | bp.id.0 as usize);
        let t = hud.tile(ui, tile_id, tr, lit, true);
        let art = Rect::new(tr.x + 1.0, tr.y + 1.0, tr.w - 2.0, tr.h - 2.0);
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
        // Tier in the top corner, how many idle in the bottom one.
        ui.text(
            tr.x + 3.0,
            tr.y + 7.0,
            type_scale::MICRO,
            rgb(palette::DIM, 1.0),
            &format!("T{}", bp.tech),
        );
        let count = ids.len().to_string();
        ui.fill(
            Rect::new(tr.right() - 14.0, tr.bottom() - 13.0, 13.0, 12.0),
            ink(0.6),
        );
        ui.text_right(
            tr.right() - 3.0,
            tr.bottom() - 7.0,
            type_scale::MICRO,
            rgb(palette::WARN, 1.0),
            &count,
        );
        pick(hud, ui, s, (card, bp.id.0 as u32), ids, lit, t.clicked);
        if t.hovered {
            tip(
                ui,
                r,
                tr.y,
                &format!("every idle {} (T{})", bp.name, bp.tech),
            );
        }
    }
}

/// What a click on an idle tile (or the title block) asks for: the next one of
/// `ids`, or with shift all of them; `lit` means the selection is already exactly
/// `ids`, and shift-clicking then finds them.
fn pick(
    hud: &mut Hud,
    ui: &mut Ui,
    s: &Scene,
    key: (u8, u32),
    ids: &[u32],
    lit: bool,
    clicked: bool,
) {
    if !clicked {
        return;
    }
    let action = if s.view.shift {
        HudAction::Select {
            units: ids.to_vec(),
            focus: lit,
        }
    } else {
        // One at a time, the camera following.
        let next = hud.builders.next.entry(key).or_insert(0);
        let i = *next % ids.len();
        *next = i + 1;
        HudAction::Select {
            units: vec![ids[i]],
            focus: true,
        }
    };
    ui.audio.play(Sfx::Select);
    hud.actions.push(action);
}

/// The hint for an idle tile, beside the card so it covers nothing on it.
fn tip(ui: &mut Ui, card: Rect, y: f32, all: &str) {
    build::tip(
        ui,
        card.right() + 8.0,
        y,
        &format!("Click selects the next one  \u{b7}  Shift-click selects {all}"),
    );
}

/// Whether the selection is exactly these units, in any order.
fn same_units(selection: &[u32], ids: &[u32]) -> bool {
    selection.len() == ids.len() && ids.iter().all(|id| selection.contains(id))
}
