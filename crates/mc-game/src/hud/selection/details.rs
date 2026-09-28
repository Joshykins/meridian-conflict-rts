//! The details card over the unit panel (DETAILS or I): lore, the unit's figures,
//! its reach and every weapon.

use super::*;

/// Lore and every weapon, over the unit panel: opened by DETAILS (or I).
pub(super) fn details_card(ui: &mut Ui, hud: &mut Hud, bp: &UnitBlueprint, anchor: Rect) {
    let w = 560.0;
    let (pad, cw) = (18.0, w - 36.0);
    let lore = wrap_text(ui, type_scale::BODY, &bp.lore, cw);
    let card_w = (cw - 12.0) * 0.5;
    let weapon_h = |ui: &mut Ui, wp: &Weapon| {
        let lines = wrap_text(ui, type_scale::MICRO, &wp.lore, card_w - 20.0).len();
        44.0 + lines as f32 * 15.0 + 4.0 * 22.0 + 10.0
    };
    let sets = weapon_sets(bp);
    let mut rows_h = 0.0;
    for pair in sets.chunks(2) {
        let h = pair
            .iter()
            .map(|s| weapon_h(ui, &bp.weapons[s[0]]))
            .fold(0.0, f32::max);
        rows_h += h + 10.0;
    }
    let h = 64.0
        + lore.len() as f32 * 20.0
        + reach_list_h(bp)
        + crate::hud::volatile::destruction_h(ui, bp, cw)
        + if bp.weapons.is_empty() {
            10.0
        } else {
            26.0 + rows_h
        };
    let r = Rect::new(anchor.x, (anchor.y - 10.0 - h).max(14.0), w, h);
    hud.claim(ui, r);
    ui.panel(r);
    let x = r.x + pad;
    ui.text(
        x,
        r.y + 24.0,
        type_scale::ITEM,
        rgb(0xFFFFFF, 1.0),
        &bp.name,
    );
    let end = ui.text(
        x,
        r.y + 44.0,
        type_scale::MICRO,
        rgb(palette::DIM, 1.0),
        &format!(
            "T{}  \u{b7}  {}  \u{b7}  {}",
            bp.tech,
            bp.role,
            Domain::of(bp).label()
        ),
    );
    crate::hud::volatile::chip(ui, bp, end + 10.0, r.y + 44.0);
    let close = Rect::new(r.right() - 34.0, r.y + 12.0, 22.0, 22.0);
    let res = ui.interact(id("details-close", 0), close, true);
    let c = Vec2::new(close.x + 11.0, close.mid_y());
    let tone = rgb(palette::TEXT, 0.6 + 0.4 * res.glow);
    ui.stroke(c - Vec2::splat(5.0), c + Vec2::splat(5.0), 1.4, tone);
    ui.stroke(
        c + Vec2::new(-5.0, 5.0),
        c + Vec2::new(5.0, -5.0),
        1.4,
        tone,
    );
    if res.clicked {
        ui.audio.play(Sfx::Tick);
        hud.details_open = false;
    }
    let mut y = r.y + 70.0;
    for line in &lore {
        ui.text(x, y, type_scale::BODY, rgb(palette::TEXT, 0.9), line);
        y += 20.0;
    }
    if !lore.is_empty() {
        y += 6.0;
    }
    // Last frame's ring in focus lights its row and card now; what is under the
    // pointer this frame goes to the ground rings (`Rings::focus`) and the next frame.
    let focus = hud.details_focus;
    let mut hover = 0;
    y = reach_list(ui, bp, x, y, cw, focus, &mut hover) - 6.0;
    if bp.volatile() {
        y = crate::hud::volatile::destruction(ui, bp, x, y + 6.0, cw) - 6.0;
    }
    if !bp.weapons.is_empty() {
        y += 6.0;
        ui.section(x, y, cw, "Armament");
        y += 16.0;
        let rings = projections(bp);
        for (row, pair) in sets.chunks(2).enumerate() {
            let row_h = pair
                .iter()
                .map(|s| weapon_h(ui, &bp.weapons[s[0]]))
                .fold(0.0, f32::max);
            for (i, set) in pair.iter().enumerate() {
                let n = row * 2 + i;
                let mine: Vec<usize> = set
                    .iter()
                    .filter_map(|&k| crate::rings::projection_of(bp, k))
                    .collect();
                let mask = mine.iter().fold(0, |m, &k| m | bit(k));
                let r = Rect::new(x + i as f32 * (card_w + 12.0), y, card_w, row_h);
                let res = ui.interact(id("weapon-card", n), r, true);
                if res.hovered {
                    hover = mask;
                }
                let (lit, back) = focus_of(ui, "weapon-card-focus", n, focus, mask);
                let arcs: Vec<Option<crate::rings::Arc>> =
                    mine.iter().map(|&k| rings[k].arc).collect();
                let names: Vec<&str> = set.iter().map(|&k| bp.weapons[k].name.as_str()).collect();
                let name = crate::hud::armament::shared_name(&names);
                weapon_card(
                    ui,
                    &bp.weapons[set[0]],
                    &name,
                    set.len(),
                    r,
                    lit.max(res.glow),
                    back,
                    &arcs,
                );
            }
            y += row_h + 10.0;
        }
    }
    hud.details_focus = hover;
    hud.reach_focus = (hover != 0).then_some((bp.id.0 as u32, hover));
}
