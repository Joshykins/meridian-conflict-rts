//! The single-unit panel's status page: integrity, shield, rank, what it is doing,
//! what it has reclaimed, and the few figures left on the panel.

use super::*;

/// `on_strip`: what it makes shows on the queue strip, so the card leaves it out.
pub(super) fn status_page(
    ui: &mut Ui,
    s: &Scene,
    u: &UnitInstance,
    bp: &UnitBlueprint,
    r: Rect,
    on_strip: bool,
) {
    let (x, cw) = (r.x, r.w);
    let level = u.veterancy_level();
    let kills = u.kill_count();
    let hp = veterancy_health(bp.health, level).to_f32();
    let mut y = r.y;
    let building = has_flag(u, flag::UNDER_CONSTRUCTION);
    if building {
        y += construction(ui, u, x, y, cw);
    }
    ui.text(x, y, type_scale::MICRO, rgb(palette::DIM, 1.0), "Integrity");
    ui.text_right(
        x + cw,
        y,
        type_scale::VALUE,
        rgb(health_tone(u.health), 1.0),
        &format!("{} / {}", whole(u.health * hp), whole(hp)),
    );
    bar(
        ui,
        Rect::new(x, y + 9.0, cw, 6.0),
        u.health,
        health_tone(u.health),
    );
    y += 24.0;

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
            rgb(crate::hud::style::AIR, 1.0),
            &format!("{} / {}", whole(sh.health * max), whole(max)),
        );
        bar(
            ui,
            Rect::new(x, y + 9.0, cw, 6.0),
            sh.health,
            crate::hud::style::AIR,
        );
        y += 24.0;
    }

    // Veterancy, in its own yellow: chevrons for the rank, the rank's name, kills.
    let rank = RANKS[level.min(VETERANCY_MAX) as usize];
    chevrons(ui, Vec2::new(x + 5.0, y), level);
    ui.text(
        x + 5.0 + VETERANCY_MAX as f32 * 11.0 + 4.0,
        y,
        type_scale::VALUE,
        rgb(VETERANCY, 1.0),
        rank,
    );
    let vet_label = if level >= VETERANCY_MAX {
        format!("{kills} Kill{}", if kills == 1 { "" } else { "s" })
    } else {
        format!(
            "{kills} Kill{}  \u{b7}  {:.0}%",
            if kills == 1 { "" } else { "s" },
            u.veterancy_progress() * 100.0
        )
    };
    ui.text_right(
        x + cw,
        y,
        type_scale::MICRO,
        rgb(VETERANCY, 0.85),
        &vet_label,
    );
    y += 20.0;

    // What it is doing.
    let queue = s.queue_of(u);
    let doing = if building {
        // Shown at the top of the card.
        None
    } else if let Some(front) = queue.and_then(|q| q.orders.first()) {
        let progress = queue.map_or(0.0, |q| q.progress);
        let making = matches!(
            front.kind,
            OrderKind::Build | OrderKind::Produce | OrderKind::Upgrade
        );
        let label = if making {
            format!(
                "{} {}",
                activity(front.kind),
                s.blueprints.unit(front.blueprint).name
            )
        } else {
            match front.formation_phase {
                1 => "Forming on Move",
                2 => "In Formation",
                3 => "Crossing Obstacle",
                _ => activity(front.kind),
            }
            .to_owned()
        };
        (!(making && on_strip)).then(|| (label, (making && progress > 0.0).then_some(progress)))
    } else {
        None
    };
    // Paused work says so first, in the construction amber: what waits, and where it stopped.
    let doing = match doing {
        _ if !u.paused() || building => doing,
        Some((label, progress)) => Some((format!("Paused  \u{b7}  {label}"), progress)),
        None => Some(("Paused  \u{b7}  Z Resumes".to_owned(), None)),
    };
    if let Some((label, progress)) = doing {
        let tone = if u.paused() { PAUSED } else { palette::TEXT };
        ui.text_fit_left(x, y, cw - 50.0, type_scale::MICRO, rgb(tone, 0.9), &label);
        if let Some(p) = progress {
            ui.text_right(
                x + cw,
                y,
                type_scale::VALUE,
                rgb(tone, 1.0),
                &format!("{:.0}%", p * 100.0),
            );
            bar(ui, Rect::new(x, y + 9.0, cw, 5.0), p, tone);
        }
        y += 20.0;
    }

    // The figures that matter, each in its colour with a bar against the roster's usual spread.
    let mut gauges: Vec<(&str, String, f32, u32)> = Vec::new();
    if !bp.weapons.is_empty() {
        gauges.push((
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
        gauges.push(("Range", format!("{:.0} m", range), range / 1000.0, tone));
    }
    if let Some(m) = &bp.motion {
        let v = m.speed.to_f32();
        gauges.push((
            "Speed",
            format!("{:.0} m/s", v),
            v / 60.0,
            Family::Movement.tone(),
        ));
    }
    if let Some(b) = &bp.builder {
        let p = b.power.to_f32();
        gauges.push((
            "Build Power",
            format!("{:.0}", p),
            p / 100.0,
            Family::Engineering.tone(),
        ));
    }
    if bp.vision.to_f32() > 0.0 {
        let v = bp.vision.to_f32();
        gauges.push(("Vision", format!("{:.0} m", v), v / 800.0, palette::TEXT));
    }
    if bp.radar.to_f32() > 0.0 {
        let v = bp.radar.to_f32();
        gauges.push((
            "Radar",
            format!("{:.0} m", v),
            v / 3000.0,
            Reach::Radar.tone(),
        ));
    }
    if bp.sonar.to_f32() > 0.0 {
        let v = bp.sonar.to_f32();
        gauges.push((
            "Sonar",
            format!("{:.0} m", v),
            v / 3000.0,
            Reach::Sonar.tone(),
        ));
    }
    let col = (cw - 14.0) * 0.5;
    for (i, (label, value, share, tone)) in gauges.iter().enumerate() {
        let (gx, gy) = (x + (i % 2) as f32 * (col + 14.0), y + (i / 2) as f32 * 22.0);
        if gy + 12.0 > r.bottom() {
            break;
        }
        gauge(ui, gx, gy, col, label, value, *share, *tone);
    }
}
