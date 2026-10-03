//! The build card: everything about a blueprint, over the build tile the pointer is
//! on. Its price, its figures (what it makes, spends and stores, what it pays back
//! in, its adjacency), its lore and its armament.

use crate::hud::style::Domain;
use crate::hud::{whole, Hud, ENERGY, MASS};
use crate::ui::{palette, rgb, type_scale, Rect, Ui};
use mc_core::TICKS_PER_SECOND;
use mc_data::{Blueprints, UnitBlueprint};

/// Everything about a blueprint, over the tile the pointer is on.
#[expect(
    clippy::too_many_arguments,
    reason = "the card's blueprint, its builder's power and where it goes"
)]
pub(in crate::hud) fn data_card(
    hud: &Hud,
    ui: &mut Ui,
    blueprints: &Blueprints,
    item: &UnitBlueprint,
    build_power: f32,
    hint: &str,
    tile: Rect,
    bottom: f32,
) {
    let mut rows: Vec<(String, String, u32)> = vec![(
        "Integrity".into(),
        whole(item.health.to_f32()),
        palette::TEXT,
    )];
    if let Some(m) = &item.motion {
        rows.push((
            "Speed".into(),
            format!("{:.0} m/s", m.speed.to_f32()),
            palette::TEXT,
        ));
    }
    let e = &item.economy;
    for (label, v, tone, sign) in [
        ("Materials Income", e.mass_income, MASS, "+"),
        ("Energy Income", e.energy_income, ENERGY, "+"),
        ("Energy Upkeep", e.energy_upkeep, ENERGY, "-"),
        ("Materials Storage", e.mass_storage, MASS, "+"),
        ("Energy Storage", e.energy_storage, ENERGY, "+"),
    ] {
        if v.to_f32() > 0.0 {
            rows.push((label.into(), format!("{sign}{}", whole(v.to_f32())), tone));
        }
    }
    // A fabricator: what it makes with its upkeep paid, and how soon that buys it back.
    if let Some(f) = item.fabricator {
        let rate = f.mass.to_f32();
        rows.push(("Makes".into(), format!("+{rate:.0} materials/s"), MASS));
        if rate > 0.0 {
            rows.push((
                "Pays Back In".into(),
                crate::hud::mine::duration(item.cost_mass.to_f32() / rate),
                MASS,
            ));
            let per = e.energy_upkeep.to_f32() / rate;
            rows.push(("Energy per Material".into(), format!("{per:.0}"), ENERGY));
        }
    }
    rows.extend(crate::hud::adjacency::card_rows(blueprints, item));
    if let Some(r) = &item.reclaimer {
        let charge = if r.charge_ticks > 0 {
            format!(
                "  \u{b7}  {:.1} s charge",
                r.charge_ticks as f32 / TICKS_PER_SECOND as f32
            )
        } else {
            String::new()
        };
        rows.push((
            "Reclaim Beam".into(),
            format!(
                "{:.0}/s  \u{b7}  {:.0} m{charge}",
                r.power.to_f32(),
                r.range.to_f32()
            ),
            MASS,
        ));
    }
    if let Some(b) = &item.builder {
        rows.push((
            "Build Power".into(),
            format!("{:.0}", b.power.to_f32()),
            palette::TEXT,
        ));
    }
    if let Some(sh) = item.shield {
        let line = if sh.is_hull() {
            format!(
                "{}  \u{b7}  +{:.0}/s",
                whole(sh.health.to_f32()),
                sh.regen.to_f32()
            )
        } else {
            format!(
                "{}  \u{b7}  {:.0} m  \u{b7}  +{:.0}/s",
                whole(sh.health.to_f32()),
                sh.radius.to_f32(),
                sh.regen.to_f32()
            )
        };
        rows.push(("Shield".into(), line, palette::TEXT));
    }
    if item.radar.to_f32() > 0.0 {
        rows.push((
            "Radar".into(),
            format!("{:.0} m", item.radar.to_f32()),
            palette::TEXT,
        ));
    }
    if item.sonar.to_f32() > 0.0 {
        rows.push((
            "Sonar".into(),
            format!("{:.0} m", item.sonar.to_f32()),
            palette::TEXT,
        ));
    }

    let w = 404.0;
    let (x, cw) = (18.0, w - 36.0);
    // Lore, wrapped to the card.
    let lore = crate::hud::selection::wrap_text(ui, type_scale::BODY, &item.lore, cw);
    let guns = crate::hud::armament::groups(&item.weapons);
    // Short figures sit two to a line; one too long for half the card has a line of its own.
    let half = cw * 0.5 - 12.0;
    let (short, long): (Vec<_>, Vec<_>) = rows.into_iter().partition(|(label, value, _)| {
        ui.text_width(type_scale::MICRO, label) + ui.text_width(type_scale::VALUE, value) + 12.0
            <= half
    });
    let row_h = 19.0;
    let lines = short.len().div_ceil(2) + long.len();
    let h = 112.0
        + if lore.is_empty() {
            0.0
        } else {
            lore.len() as f32 * 19.0 + 10.0
        }
        + lines as f32 * row_h
        + crate::hud::armament::height(&guns)
        + 34.0;
    let r = Rect::new(
        (tile.x + tile.w * 0.5 - w * 0.5).clamp(14.0, ui.size.x - w - 14.0),
        (bottom - h).max(14.0),
        w,
        h,
    );
    ui.panel(r);
    let x = r.x + x;
    let pic = Rect::new(r.right() - 76.0, r.y + 8.0, 64.0, 64.0);
    hud.thumbs.draw(ui, item.id, pic, 1.0);
    ui.text(
        x,
        r.y + 26.0,
        type_scale::ITEM,
        rgb(0xFFFFFF, 1.0),
        &item.name,
    );
    let end = ui.text(
        x,
        r.y + 48.0,
        type_scale::MICRO,
        rgb(palette::TEXT, 1.0),
        &format!(
            "Tech {}  \u{b7}  {}  \u{b7}  {}",
            item.tech,
            item.role,
            Domain::of(item).label()
        ),
    );
    crate::hud::volatile::chip(ui, item, end + 8.0, r.y + 48.0);

    // The price: mass, energy, and how long this builder takes over it.
    let seconds = item.build_time.to_f32() / build_power.max(0.1);
    let costs = [
        ("Materials", whole(item.cost_mass.to_f32()), MASS),
        ("Energy", whole(item.cost_energy.to_f32()), ENERGY),
        ("Time", crate::hud::clock(seconds), palette::TEXT),
    ];
    for (i, (label, value, tone)) in costs.iter().enumerate() {
        let cx = x + i as f32 * (cw - 70.0) / 3.0;
        ui.fill(Rect::new(cx, r.y + 66.0, 2.0, 28.0), rgb(*tone, 0.9));
        ui.text(
            cx + 10.0,
            r.y + 72.0,
            type_scale::MICRO,
            rgb(palette::FAINT, 1.0),
            label,
        );
        ui.text(
            cx + 10.0,
            r.y + 88.0,
            type_scale::VALUE,
            rgb(*tone, 1.0),
            value,
        );
    }
    ui.hline(x, r.y + 104.0, cw, rgb(palette::LINE, 0.16));
    let mut y = r.y + 120.0;
    for line in &lore {
        ui.text(x, y, type_scale::BODY, rgb(palette::DIM, 1.0), line);
        y += 19.0;
    }
    if !lore.is_empty() {
        y += 10.0;
    }
    let col = cw * 0.5 + 12.0;
    for pair in short.chunks(2) {
        for (i, (label, value, tone)) in pair.iter().enumerate() {
            let fx = x + i as f32 * col;
            ui.text(fx, y, type_scale::MICRO, rgb(palette::DIM, 1.0), label);
            ui.text_right(
                fx + cw * 0.5 - 12.0,
                y,
                type_scale::VALUE,
                rgb(*tone, 1.0),
                value,
            );
        }
        if pair.len() == 2 {
            ui.vline(x + cw * 0.5 + 1.0, y - 7.0, 14.0, rgb(palette::LINE, 0.12));
        }
        y += row_h;
    }
    for (label, value, tone) in &long {
        // A long label gives way to its figures, never runs into them.
        let room = cw - ui.text_width(type_scale::VALUE, value) - 14.0;
        let label = super::shorten(ui, label, room);
        ui.text(x, y, type_scale::MICRO, rgb(palette::DIM, 1.0), &label);
        ui.text_right(x + cw, y, type_scale::VALUE, rgb(*tone, 1.0), value);
        y += row_h;
    }
    crate::hud::armament::draw(ui, &guns, x, y, cw);
    ui.text(
        x,
        r.bottom() - 16.0,
        type_scale::MICRO,
        rgb(palette::FAINT, 1.0),
        hint,
    );
}
