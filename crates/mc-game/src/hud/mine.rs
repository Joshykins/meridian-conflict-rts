//! A core mine in the unit panel: what it makes, how much of its reach it has
//! to itself, the ore in its territory, and what its next tier would add and
//! how long that takes to pay back.

use super::{Scene, MASS};
use crate::ui::{palette, rgb, type_scale, Rect, Ui};
use mc_data::{Blueprints, UnitBlueprint};
use mc_sim::mirror::{MineView, UnitInstance};

/// Height the block takes, points.
pub const HEIGHT: f32 = 54.0;

/// The mines among `units` that report their state, with their blueprints, in order.
pub fn views<'a>(s: &Scene<'a>, units: &[&UnitInstance]) -> Vec<(MineView, &'a UnitBlueprint)> {
    units
        .iter()
        .filter_map(|u| {
            let view = s.queue_of(u.unit_id).and_then(|q| q.mine)?;
            Some((view, s.blueprints.unit(mc_data::BlueprintId(u.blueprint as u16))))
        })
        .collect()
}

/// Materials per second a mine of `bp` would make on this territory.
pub fn rate_on(bp: &UnitBlueprint, land: &mc_sim::mines::Share) -> f32 {
    bp.mine
        .map_or(0.0, |m| land.rate(&m).to_f32())
}

/// What upgrading to the next tier adds per second, and the seconds its
/// materials cost takes to earn back from that gain.
pub fn upgrade_gain(blueprints: &Blueprints, bp: &UnitBlueprint, view: &MineView) -> Option<(f32, f32)> {
    let next = blueprints.unit(bp.upgrades_to?);
    let gain = rate_on(next, &view.land) - rate_on(bp, &view.land);
    (gain > 0.0).then(|| (gain, next.cost_mass.to_f32() / gain))
}

/// `93` as `1m 33s`, `40` as `40s`.
pub fn duration(seconds: f32) -> String {
    let s = seconds.max(0.0).round() as u32;
    if s >= 3600 {
        format!("{}h {:02}m", s / 3600, s / 60 % 60)
    } else if s >= 60 {
        format!("{}m {:02}s", s / 60, s % 60)
    } else {
        format!("{s}s")
    }
}

/// Draws the block for `mines` (one mine, or every mine in a selection:
/// output and gains are summed, efficiency averaged) at the top of `r`.
pub fn panel(ui: &mut Ui, s: &Scene, mines: &[(MineView, &UnitBlueprint)], r: Rect) {
    if mines.is_empty() {
        return;
    }
    let n = mines.len() as f32;
    let rate: f32 = mines.iter().map(|(m, _)| m.rate).sum();
    let share = mines.iter().map(|(m, _)| m.share).sum::<f32>() / n;
    let ore: f32 = mines.iter().map(|(m, _)| m.land.ore.to_f32()).sum();
    let land: f32 = mines.iter().map(|(m, _)| m.land.ground.to_f32()).sum();

    let (x, w) = (r.x, r.w);
    let mut y = r.y;
    ui.text(x, y, type_scale::MICRO, rgb(palette::DIM, 1.0), "Output");
    ui.text_right(x + w, y, type_scale::VALUE, rgb(MASS, 1.0), &format!("{rate:.1}/s"));
    let bar = Rect::new(x, y + 9.0, w, 4.0);
    ui.fill(bar, rgb(palette::LINE, 0.12));
    ui.gradient_h(
        Rect::new(bar.x, bar.y, bar.w * share.clamp(0.0, 1.0), bar.h),
        rgb(MASS, 0.6),
        rgb(MASS, 1.0),
    );
    y += 18.0;

    // How much of its reach it has to itself, and what lies in its territory.
    let tone = if share >= 0.9 {
        palette::TEXT
    } else if share >= 0.6 {
        palette::WARN
    } else {
        palette::BAD
    };
    let ground = if ore > 0.05 {
        format!("{land:.0} ha  \u{b7}  ore {ore:.1} ha")
    } else {
        format!("{land:.0} ha  \u{b7}  no ore")
    };
    ui.text(x, y, type_scale::MICRO, rgb(tone, 1.0), &format!("Efficiency {:.0}%", share * 100.0));
    ui.text_right(x + w, y, type_scale::MICRO, rgb(palette::DIM, 1.0), &ground);
    y += 15.0;

    // The next tier: what it adds here, and how soon it pays for itself.
    let gains: Vec<(f32, f32)> = mines
        .iter()
        .filter_map(|(m, bp)| upgrade_gain(s.blueprints, bp, m))
        .collect();
    if gains.is_empty() {
        ui.text(x, y, type_scale::MICRO, rgb(palette::FAINT, 1.0), "Top tier");
    } else {
        let gain: f32 = gains.iter().map(|g| g.0).sum();
        let cost: f32 = gains.iter().map(|g| g.0 * g.1).sum();
        ui.text(
            x,
            y,
            type_scale::MICRO,
            rgb(palette::TEXT, 0.85),
            &format!(
                "Next tier +{gain:.1}/s  \u{b7}  pays back in {}",
                duration(cost / gain)
            ),
        );
    }
}
