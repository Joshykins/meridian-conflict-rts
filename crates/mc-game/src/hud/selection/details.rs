//! The details card over the unit panel (its i button or I): lore, the unit's figures
//! (damage, range, speed, build power, vision, radar, sonar), its reach and every weapon.

use super::*;

/// Lore, figures and every weapon, over the unit panel: opened by its i button (or I).
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
    let figures = figures(bp);
    let h = 64.0
        + lore.len() as f32 * 20.0
        + figures_h(&figures)
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
    if !figures.is_empty() {
        ui.section(x, y, cw, "Figures");
        y += 18.0;
        let col = (cw - 14.0) * 0.5;
        for (i, (label, value, share, tone)) in figures.iter().enumerate() {
            let (gx, gy) = (x + (i % 2) as f32 * (col + 14.0), y + (i / 2) as f32 * 22.0);
            gauge(ui, gx, gy, col, label, value, *share, *tone);
        }
        y += figures.len().div_ceil(2) as f32 * 22.0 + 6.0;
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

/// The unit's headline figures, each in its colour with a bar against the roster's usual spread.
fn figures(bp: &UnitBlueprint) -> Vec<(&'static str, String, f32, u32)> {
    let mut out = Vec::new();
    if !bp.weapons.is_empty() {
        out.push((
            "Damage / s",
            format!("{:.0}", dps(bp)),
            dps(bp) / 400.0,
            Family::Combat.tone(),
        ));
        let range = bp.max_weapon_range().to_f32();
        // In the colour of the farthest gun's ring.
        let tone = bp
            .weapons
            .iter()
            .max_by(|a, b| a.range_max.cmp(&b.range_max))
            .map_or(Reach::Direct, Reach::of)
            .tone();
        out.push(("Range", format!("{:.0} m", range), range / 1000.0, tone));
    }
    if let Some(m) = &bp.motion {
        let v = m.speed.to_f32();
        out.push((
            "Speed",
            format!("{:.0} m/s", v),
            v / 60.0,
            Family::Movement.tone(),
        ));
    }
    if let Some(b) = &bp.builder {
        let p = b.power.to_f32();
        out.push((
            "Build Power",
            format!("{:.0}", p),
            p / 100.0,
            Family::Engineering.tone(),
        ));
    }
    // What it sees by, in the colours of its rings.
    let vision = bp.vision.to_f32();
    if vision > 0.0 {
        out.push((
            "Vision",
            format!("{:.0} m", vision),
            vision / 800.0,
            palette::TEXT,
        ));
    }
    for (label, reach, v) in [
        ("Radar", Reach::Radar, bp.radar.to_f32()),
        ("Sonar", Reach::Sonar, bp.sonar.to_f32()),
    ] {
        if v > 0.0 {
            out.push((label, format!("{:.0} m", v), v / 3000.0, reach.tone()));
        }
    }
    out
}

fn figures_h(figures: &[(&str, String, f32, u32)]) -> f32 {
    if figures.is_empty() {
        0.0
    } else {
        18.0 + figures.len().div_ceil(2) as f32 * 22.0 + 6.0
    }
}
