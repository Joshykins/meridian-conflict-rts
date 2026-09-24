//! What a unit makes and spends, drawn the same way wherever a unit is shown:
//! a strip with a MASS half and an ENERGY half, each with what comes in, what
//! goes out, and, only while the side is short, what it wanted and the share it got.

use super::{Scene, ENERGY, MASS};
use crate::ui::{palette, rgb, style, type_scale, Rect, Ui};
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

/// A unit's mass and energy flows: the sim's own figures when it reports them,
/// else what its blueprint says it makes and costs to keep running.
pub fn flows(s: &Scene, u: &UnitInstance, bp: &UnitBlueprint) -> (Flow, Flow) {
    if let Some(q) = s.queue_of(u.unit_id) {
        return (
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
        );
    }
    let e = &bp.economy;
    let upkeep = e.energy_upkeep.to_f32();
    (
        Flow {
            made: e.mass_income.to_f32(),
            ..Default::default()
        },
        Flow {
            made: e.energy_income.to_f32(),
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

const BIG: crate::ui::Style = style(mc_render::Face::Bold, 18.0, 0.8);

/// Whether a unit takes part in the economy at all: it makes something, keeps
/// something running, or builds. Those always show the strip, idle or not.
pub fn takes_part(bp: &UnitBlueprint) -> bool {
    let e = &bp.economy;
    bp.builder.is_some()
        || bp.reclaimer.is_some()
        || e.mass_income.to_f32() > 0.0
        || e.energy_income.to_f32() > 0.0
        || e.energy_upkeep.to_f32() > 0.0
}

/// The strip, `r` tall enough for two lines (about 44 points). Nothing is
/// drawn, and false returned, when the flows are empty and `always` is off.
pub fn strip(ui: &mut Ui, mass: Flow, energy: Flow, r: Rect, always: bool) -> bool {
    if mass.is_empty() && energy.is_empty() && !always {
        return false;
    }
    ui.fill_cut(r, 4.0, rgb(0xFFFFFF, 0.035));
    let half = r.w * 0.5;
    for (i, (name, flow, tone)) in [("Materials", mass, MASS), ("Energy", energy, ENERGY)]
        .into_iter()
        .enumerate()
    {
        let x = r.x + i as f32 * half + 10.0;
        let w = half - 20.0;
        if i > 0 {
            ui.vline(r.x + half, r.y + 6.0, r.h - 12.0, rgb(palette::LINE, 0.15));
        }
        ui.fill(Rect::new(x, r.y + 8.0, 3.0, 9.0), rgb(tone, 1.0));
        ui.text(x + 9.0, r.y + 13.0, type_scale::MICRO, rgb(tone, 1.0), name);
        if flow.is_empty() {
            // Nothing in or out right now: a quiet zero in the same place the rates go.
            ui.text_right(x + w, r.y + 30.0, BIG, rgb(palette::FAINT, 1.0), "0 /s");
            continue;
        }
        // Short of what it wanted: the share it got and what it asked for, on the label's row.
        let share = flow.share();
        let pulse = 0.7 + 0.3 * (ui.time * 4.0).sin().abs();
        if share < 0.995 {
            let label_end = x + 9.0 + ui.text_width(type_scale::MICRO, name) + 8.0;
            let wants = format!("{:.0}% of \u{2212}{}", share * 100.0, rate(flow.wanted));
            let (st, wants) = ui.fitted(type_scale::MICRO, &wants, x + w - label_end);
            ui.text_right(x + w, r.y + 13.0, st, rgb(palette::WARN, pulse), &wants);
            let track = Rect::new(x, r.bottom() - 5.0, w, 2.0);
            ui.fill(track, rgb(palette::WARN, 0.2));
            ui.fill(Rect::new(track.x, track.y, track.w * share, track.h), rgb(palette::WARN, pulse));
        }
        // What comes in, and what goes out, as big signed rates.
        let mut right = x + w;
        let unit_w = ui.text_width(type_scale::MICRO, "/s");
        ui.text(right - unit_w, r.y + 31.0, type_scale::MICRO, rgb(palette::DIM, 1.0), "/s");
        right -= unit_w + 4.0;
        if flow.used >= 0.005 || flow.wanted >= 0.005 {
            let out = format!("\u{2212}{}", rate(flow.used));
            let wd = ui.text_width(BIG, &out);
            ui.text(right - wd, r.y + 30.0, BIG, rgb(if share < 0.995 { palette::WARN } else { palette::BAD }, 1.0), &out);
            right -= wd + 10.0;
        }
        if flow.made >= 0.005 {
            let inc = format!("+{}", rate(flow.made));
            let wd = ui.text_width(BIG, &inc);
            ui.text(right - wd, r.y + 30.0, BIG, rgb(tone, 1.0), &inc);
        }
    }
    true
}
