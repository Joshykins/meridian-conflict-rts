//! A core mine in the unit panel: what it makes, and what its next tier would
//! add and how long that takes to pay back.

use super::{Scene, MASS};
use crate::ui::{palette, rgb, type_scale, Rect, Ui};
use mc_data::UnitBlueprint;
use mc_sim::mirror::{MineView, UnitInstance};

/// Height the block takes, points.
pub const HEIGHT: f32 = 40.0;

/// The mines among `units` that report their state, with their blueprints, in order.
pub fn views<'a>(s: &Scene<'a>, units: &[&UnitInstance]) -> Vec<(MineView, &'a UnitBlueprint)> {
    units
        .iter()
        .filter_map(|u| {
            let view = s.queue_of(u).and_then(|q| q.mine)?;
            Some((
                view,
                s.blueprints.unit(mc_data::BlueprintId(u.blueprint as u16)),
            ))
        })
        .collect()
}

/// Materials per second a unit of `bp` makes at full power: a core mine's rate or
/// a fabricator's output.
pub fn makes(bp: &UnitBlueprint) -> f32 {
    bp.mine.map_or(0.0, |m| m.rate.to_f32()) + bp.fabricator.map_or(0.0, |f| f.mass.to_f32())
}

/// What climbing from `bp` to `to` (one tier or several) adds per second, and
/// the seconds the materials paid on the way take to earn back from that gain.
/// Each upgrade costs the difference between the tiers, so the climb costs the
/// difference between its ends.
pub fn climb_gain(bp: &UnitBlueprint, to: &UnitBlueprint) -> Option<(f32, f32)> {
    let gain = makes(to) - makes(bp);
    let cost = (to.cost_mass - bp.cost_mass).to_f32().max(0.0);
    (gain > 0.0).then(|| (gain, cost / gain))
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

/// Draws the block for `mines` (one mine, or every mine in a selection: output
/// and gains are summed) at the top of `r`.
pub fn panel(ui: &mut Ui, s: &Scene, mines: &[(MineView, &UnitBlueprint)], r: Rect) {
    if mines.is_empty() {
        return;
    }
    let rate: f32 = mines.iter().map(|(m, _)| m.rate).sum();
    let full: f32 = mines.iter().map(|(m, _)| m.full).sum();

    let (x, w) = (r.x, r.w);
    let mut y = r.y;
    ui.text(x, y, type_scale::MICRO, rgb(palette::DIM, 1.0), "Output");
    ui.text_right(
        x + w,
        y,
        type_scale::VALUE,
        rgb(MASS, 1.0),
        &format!("{rate:.1}/s"),
    );
    let bar = Rect::new(x, y + 9.0, w, 4.0);
    ui.fill(bar, rgb(palette::LINE, 0.12));
    let share = if full > 0.0 { rate / full } else { 1.0 };
    ui.gradient_h(
        Rect::new(bar.x, bar.y, bar.w * share.clamp(0.0, 1.0), bar.h),
        rgb(MASS, 0.6),
        rgb(MASS, 1.0),
    );
    y += 18.0;

    // The next tier: what it adds, and how soon it pays for itself.
    let gains: Vec<(f32, f32)> = mines
        .iter()
        .filter_map(|(_, bp)| climb_gain(bp, s.blueprints.unit(bp.upgrades_to?)))
        .collect();
    let (tone, line) = if gains.is_empty() {
        (rgb(palette::FAINT, 1.0), "Top tier".to_string())
    } else {
        let gain: f32 = gains.iter().map(|g| g.0).sum();
        let cost: f32 = gains.iter().map(|g| g.0 * g.1).sum();
        (
            rgb(palette::TEXT, 0.85),
            format!(
                "Next tier +{gain:.0}/s  \u{b7}  pays back in {}",
                duration(cost / gain)
            ),
        )
    };
    // Short of energy, the mines dig slower: say so before the next tier.
    let line = if share < 0.99 {
        format!("No power: digging at {:.0}%", share * 100.0)
    } else {
        line
    };
    let tone = if share < 0.99 {
        rgb(palette::WARN, 1.0)
    } else {
        tone
    };
    ui.text(x, y, type_scale::MICRO, tone, &line);
}
