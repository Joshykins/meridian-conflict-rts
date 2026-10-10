//! What a unit makes and spends, drawn the same way wherever a unit is shown:
//! one row with a Materials half and an Energy half, each the net per second.

use super::{Scene, ENERGY, MASS};
use crate::ui::{id, palette, rgb, type_scale, Rect, Ui};
use mc_data::UnitBlueprint;
use mc_sim::mirror::UnitInstance;

/// One resource, per second.
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct Flow {
    pub made: f32,
    pub wanted: f32,
    pub used: f32,
}

impl Flow {
    pub fn is_empty(&self) -> bool {
        self.made < 0.005 && self.wanted < 0.005 && self.used < 0.005
    }

    fn add(&mut self, o: Flow) {
        self.made += o.made;
        self.wanted += o.wanted;
        self.used += o.used;
    }

    /// The share of what it wanted that it got; 1 when it wanted nothing.
    pub fn share(&self) -> f32 {
        if self.wanted < 0.005 {
            1.0
        } else {
            (self.used / self.wanted).clamp(0.0, 1.0)
        }
    }
}

/// A unit's mass and energy flows: the sim's own figures when it reports them (the
/// selection, the unit under the pointer and the side's busy units: `sim_thread::Watch`),
/// else what its blueprint says.
pub fn flows(s: &Scene, u: &UnitInstance, bp: &UnitBlueprint) -> (Flow, Flow) {
    match s.queue_of(u) {
        Some(q) => (
            Flow {
                made: q.mass_made,
                wanted: q.mass_wanted,
                used: q.mass_used,
            },
            Flow {
                made: q.energy_made,
                wanted: q.energy_wanted,
                used: q.energy_used,
            },
        ),
        None => rated(bp, s.income_dial((u.owner_flags & 0xFF) as u8)),
    }
}

/// What a blueprint makes, a fabricator's materials included, as a side with income
/// `dial` is paid it, and costs to keep running.
fn rated(bp: &UnitBlueprint, dial: [f32; 2]) -> (Flow, Flow) {
    let e = &bp.economy;
    let upkeep = e.energy_upkeep.to_f32();
    (
        Flow {
            made: (e.mass_income.to_f32() + bp.fabricator.map_or(0.0, |f| f.mass.to_f32()))
                * dial[0],
            ..Default::default()
        },
        Flow {
            made: e.energy_income.to_f32() * dial[1],
            wanted: upkeep,
            used: upkeep,
        },
    )
}

/// The flows of several units together.
pub fn total(s: &Scene, units: &[&UnitInstance]) -> (Flow, Flow) {
    let (mut m, mut e) = (Flow::default(), Flow::default());
    for u in units {
        let (a, b) = flows(s, u, s.bp(u));
        m.add(a);
        e.add(b);
    }
    (m, e)
}

fn rate(v: f32) -> String {
    if v >= 100.0 {
        format!("{:.0}", v)
    } else {
        format!("{:.1}", v)
    }
}

/// What one resource comes to per second, signed.
fn signed(v: f32) -> String {
    if v >= 0.005 {
        format!("+{}", rate(v))
    } else if v <= -0.005 {
        format!("\u{2212}{}", rate(-v))
    } else {
        "0".to_owned()
    }
}

/// Height of the strip.
pub const STRIP_H: f32 = 24.0;

/// One row with a half for each resource: its name, and the net per second on the
/// right, green while it gains, red while it drains, amber with a bar of the share it
/// gets while the side is short. What comes in, goes out and was wanted shows under
/// the pointer. Nothing is drawn, and false returned, when both flows are empty.
pub fn strip(ui: &mut Ui, mass: Flow, energy: Flow, r: Rect) -> bool {
    if mass.is_empty() && energy.is_empty() {
        return false;
    }
    ui.fill_cut(r, 4.0, rgb(0xFFFFFF, 0.035));
    let half = r.w * 0.5;
    let mid = r.mid_y();
    let unit_w = ui.text_width(type_scale::MICRO, "/s");
    let pulse = 0.7 + 0.3 * (ui.time * 4.0).sin().abs();
    let mut hovered = None;
    for (i, (name, flow, tone)) in [("Materials", mass, MASS), ("Energy", energy, ENERGY)]
        .into_iter()
        .enumerate()
    {
        let (x, w) = (r.x + i as f32 * half + 10.0, half - 20.0);
        if i > 0 {
            ui.vline(r.x + half, r.y + 5.0, r.h - 10.0, rgb(palette::LINE, 0.15));
        }
        ui.fill(Rect::new(x, mid - 4.5, 3.0, 9.0), rgb(tone, 1.0));
        ui.text(x + 9.0, mid, type_scale::MICRO, rgb(tone, 1.0), name);
        let net = flow.made - flow.used;
        let short = flow.share() < 0.995;
        let value_tone = if short {
            palette::WARN
        } else if net >= 0.005 {
            super::HEALTHY
        } else if net <= -0.005 {
            palette::BAD
        } else {
            palette::FAINT
        };
        let alpha = if short { pulse } else { 1.0 };
        ui.text_right(x + w, mid, type_scale::MICRO, rgb(palette::DIM, 1.0), "/s");
        ui.text_right(
            x + w - unit_w - 3.0,
            mid,
            type_scale::VALUE,
            rgb(value_tone, alpha),
            &signed(net),
        );
        if short {
            let track = Rect::new(x, r.bottom() - 4.0, w, 2.0);
            ui.fill(track, rgb(palette::WARN, 0.2));
            ui.fill(
                Rect::new(track.x, track.y, track.w * flow.share(), track.h),
                rgb(palette::WARN, pulse),
            );
        }
        let cell = Rect::new(r.x + i as f32 * half, r.y, half, r.h);
        if !flow.is_empty() && ui.interact(id("economy-strip", i), cell, true).hovered {
            hovered = Some((name, flow));
        }
    }
    if let Some((name, flow)) = hovered {
        let mut parts = Vec::new();
        if flow.made >= 0.005 {
            parts.push(format!("makes +{}", rate(flow.made)));
        }
        if flow.used >= 0.005 || flow.wanted >= 0.005 {
            parts.push(format!("uses \u{2212}{}", rate(flow.used)));
        }
        if flow.share() < 0.995 {
            parts.push(format!(
                "wants \u{2212}{}, gets {:.0}%",
                rate(flow.wanted),
                flow.share() * 100.0
            ));
        }
        let text = format!("{name}: {} per second", parts.join("  \u{b7}  "));
        super::build::tip(ui, r.x, r.y - 30.0, &text);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use mc_data::Blueprints;

    #[test]
    fn a_fabricator_unreported_by_the_sim_still_shows_what_it_makes() {
        let blueprints = Blueprints::load(&Blueprints::locate_data_dir().expect("data dir"))
            .expect("blueprints");
        let bp = blueprints.unit(blueprints.id_of("aster_t2_fabricator").expect("fabricator"));
        let made = bp.fabricator.expect("a fabricator").mass.to_f32();
        let (mass, energy) = rated(bp, [1.0; 2]);
        assert!(made > 0.0);
        assert_eq!(mass.made, made + bp.economy.mass_income.to_f32());
        assert_eq!(energy.used, bp.economy.energy_upkeep.to_f32());
    }

    #[test]
    fn a_side_on_an_income_multiplier_sees_what_it_is_paid() {
        let blueprints = Blueprints::load(&Blueprints::locate_data_dir().expect("data dir"))
            .expect("blueprints");
        let bp = blueprints.unit(blueprints.id_of("aster_t2_fabricator").expect("fabricator"));
        let (base, base_e) = rated(bp, [1.0; 2]);
        let (mass, energy) = rated(bp, [3.0, 2.0]);
        assert_eq!(mass.made, base.made * 3.0);
        assert_eq!(energy.made, base_e.made * 2.0);
        // Upkeep is not income: the dial leaves it alone.
        assert_eq!(energy.used, base_e.used);
    }
}
