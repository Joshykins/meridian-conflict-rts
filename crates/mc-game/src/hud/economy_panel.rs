//! The economy panel, top left: materials and energy, what comes in and goes out,
//! the side's "build first" focus under them, and the stall chip beside them.

use super::focus::{self, Short};
use super::{
    whole, Hud, Scene, DECK_H, ECONOMY_H, ECONOMY_W, EDGE, ENERGY, GAP, LOW, MASS, STALL_CHIP_W,
};
use crate::ui::{ink, palette, rgb, type_scale, Rect, Ui};

/// Height of the figures, over the focus strip.
const FIGURES_H: f32 = 68.0;

impl Hud {
    /// Returns the y below it.
    pub(super) fn economy(&mut self, ui: &mut Ui, s: &Scene, dt: f32) -> f32 {
        if s.view.observing {
            // Down to the control-group chips' line above the deck.
            let bottom = ui.size.y - EDGE - DECK_H - GAP;
            return self.observer_panel(ui, s, bottom);
        }
        let block = 292.0;
        let r = Rect::new(EDGE, EDGE, ECONOMY_W, ECONOMY_H);
        let Some(p) = s.view.status.players.get(s.view.local as usize) else {
            return r.bottom();
        };
        self.glass(ui, r);
        // The test range's free build spends nothing, whatever the builders ask for.
        let free = s.view.range.as_ref().is_some_and(|range| range.free_build);
        // Materials come in from the mines and from reclaim; the reclaim's share is shown too.
        let rows = [
            (
                "Materials",
                p.mass,
                p.mass_capacity,
                p.mass_income + p.reclaim_income,
                p.mass_demand,
                MASS,
            ),
            (
                "Energy",
                p.energy,
                p.energy_capacity,
                p.energy_income,
                p.energy_demand,
                ENERGY,
            ),
        ];
        // Each resource's state this frame: dry and short, or running low.
        let mut short = [Short::Fine; 2];
        for (i, (name, have, cap, income, demand, tone)) in rows.into_iter().enumerate() {
            let x = r.x + 16.0 + i as f32 * (block + 14.0);
            if i > 0 {
                ui.vline(
                    x - 8.0,
                    r.y + 10.0,
                    FIGURES_H - 20.0,
                    rgb(palette::LINE, 0.14),
                );
            }
            // What the builders are asking for, not what a stall lets them have: the
            // sum is how far short the income falls.
            let spend = demand;
            let net = income - spend;
            // The store itself only drains by what is really being spent.
            let drain = income - demand * p.efficiency;
            let empty = !free && have < 1.0 && net < -0.05;
            // Dry within fifteen seconds, or nearly there already.
            let low = !free
                && !empty
                && drain < -0.05
                && (have + drain * 15.0 <= 0.0 || have < cap * 0.15);
            short[i] = if empty {
                Short::Dry
            } else if low {
                Short::Low
            } else {
                Short::Fine
            };
            // Empty is a hard red blink; low a slower yellow swell.
            let alert = if empty {
                Some((
                    palette::BAD,
                    if (ui.time * 3.0).fract() < 0.5 {
                        1.0
                    } else {
                        0.2
                    },
                ))
            } else if low {
                Some((LOW, 0.5 + 0.5 * (ui.time * 5.0).sin()))
            } else {
                None
            };
            if let Some((color, k)) = alert {
                let wash = Rect::new(x - 7.0, r.y + 5.0, block + 2.0, FIGURES_H - 10.0);
                let strength = if empty { 1.6 } else { 1.0 };
                ui.gradient_v(
                    wash,
                    rgb(color, (0.05 + 0.15 * k) * strength),
                    rgb(color, (0.02 + 0.06 * k) * strength),
                );
                ui.frame(wash, rgb(color, 0.25 + 0.7 * k));
            }
            ui.fill(Rect::new(x, r.y + 12.0, 3.0, 10.0), rgb(tone, 1.0));
            let end = ui.text(
                x + 10.0,
                r.y + 17.0,
                type_scale::CAPTION,
                rgb(tone, 1.0),
                name,
            );
            let have_color = alert.map_or(rgb(palette::TEXT, 1.0), |(color, k)| {
                rgb(color, 0.7 + 0.3 * k)
            });
            let end = ui.text(
                end + 12.0,
                r.y + 17.0,
                type_scale::VALUE,
                have_color,
                &whole(have),
            );
            let net_text = if net.abs() >= 100.0 {
                format!("{}{}", if net < 0.0 { "-" } else { "+" }, whole(net.abs()))
            } else {
                format!("{net:+.1}")
            };
            let net_w = ui.text_width(type_scale::BUTTON, &net_text);
            ui.text_right(
                x + block - 12.0,
                r.y + 17.0,
                type_scale::BUTTON,
                rgb(if net < -0.05 { palette::BAD } else { tone }, 1.0),
                &net_text,
            );
            // The capacity gives way when the numbers get long.
            let cap_text = format!("/ {}", whole(cap));
            if end + 5.0 + ui.text_width(type_scale::MICRO, &cap_text)
                < x + block - 12.0 - net_w - 8.0
            {
                ui.text(
                    end + 5.0,
                    r.y + 17.5,
                    type_scale::MICRO,
                    rgb(palette::FAINT, 1.0),
                    &cap_text,
                );
            }

            let track = Rect::new(x, r.y + 32.0, block - 12.0, 6.0);
            ui.fill(
                track,
                alert.map_or(rgb(palette::LINE, 0.13), |(color, k)| {
                    rgb(color, 0.12 + 0.3 * k)
                }),
            );
            let fill = (have / cap.max(1.0)).clamp(0.0, 1.0);
            let bar = alert.map_or(tone, |(color, _)| color);
            ui.gradient_h(
                Rect::new(track.x, track.y, track.w * fill, track.h),
                rgb(bar, 0.55),
                rgb(bar, 1.0),
            );
            ui.fill(
                Rect::new(
                    track.x + track.w * fill - 1.0,
                    track.y - 2.0,
                    2.0,
                    track.h + 4.0,
                ),
                rgb(0xFFFFFF, if fill > 0.002 { 0.9 } else { 0.0 }),
            );
            for k in 1..4 {
                ui.vline(
                    track.x + track.w * k as f32 / 4.0,
                    track.bottom() + 2.0,
                    3.0,
                    rgb(palette::LINE, 0.25),
                );
            }
            let end = ui.text(
                x,
                r.y + 54.0,
                type_scale::MICRO,
                rgb(palette::DIM, 1.0),
                &format!("Income  +{income:.1}"),
            );
            if i == 0 && p.reclaim_income > 0.05 {
                let share = p.reclaim_income / income.max(0.01) * 100.0;
                ui.text(
                    end + 8.0,
                    r.y + 54.0,
                    type_scale::MICRO,
                    rgb(MASS, 0.95),
                    &format!("{share:.0}% reclaim"),
                );
            }
            ui.text_right(
                x + block - 12.0,
                r.y + 54.0,
                type_scale::MICRO,
                rgb(palette::DIM, 1.0),
                &format!("Spend  -{spend:.1}"),
            );
        }
        focus::strip(self, ui, p, r, short, dt);
        self.focus_ui.watch(p, short, dt, &mut self.notices);
        if p.efficiency < 0.999 {
            let pulse = 0.65 + 0.35 * (ui.time * 5.0).sin().abs();
            let what = match short {
                [Short::Dry, Short::Dry] => "Materials and energy stall",
                [Short::Dry, _] => "Materials stall",
                [_, Short::Dry] => "Energy stall",
                _ => "Stalling",
            };
            // Kinds put first or last say their own speed on the priority row, so
            // this is the speed of everything else.
            let split = p.tier_speed[0].is_some() || p.tier_speed[2].is_some();
            let head = match (p.tier_speed[1], split) {
                (Some(rest), true) => {
                    format!("{what}  \u{b7}  the rest at {:.0}%", rest * 100.0)
                }
                (None, true) => what.to_owned(),
                (rest, false) => format!(
                    "{what}  \u{b7}  building at {:.0}%",
                    rest.unwrap_or(p.build_speed) * 100.0
                ),
            };
            // Out of energy the mines slow too: say what that costs.
            let mines = mines_short(p);
            let w = mines
                .as_ref()
                .map_or(0.0, |m| ui.text_width(type_scale::MICRO, m));
            let h = if mines.is_some() { 46.0 } else { 28.0 };
            let chip = Rect::new(r.right() + GAP, r.y, (w + 28.0).max(STALL_CHIP_W), h);
            ui.fill(chip, ink(0.7));
            ui.frame(chip, rgb(palette::BAD, 0.7 * pulse));
            ui.fill(
                Rect::new(chip.x, chip.y, 3.0, chip.h),
                rgb(palette::BAD, pulse),
            );
            ui.text(
                chip.x + 14.0,
                chip.y + 14.0,
                type_scale::CAPTION,
                rgb(palette::BAD, pulse),
                &head,
            );
            if let Some(mines) = mines {
                ui.text(
                    chip.x + 14.0,
                    chip.y + 32.0,
                    type_scale::MICRO,
                    rgb(ENERGY, 0.75 + 0.25 * pulse),
                    &mines,
                );
            }
        }
        r.bottom()
    }
}

/// What an energy stall is costing the mines, when it costs them anything:
/// the share of full output they dig at and the materials a second lost.
pub(super) fn mines_short(p: &crate::sim_thread::PlayerStatus) -> Option<String> {
    (p.mine_power < 0.999 && p.mine_lost > 0.05).then(|| {
        let lost = if p.mine_lost >= 100.0 {
            whole(p.mine_lost)
        } else {
            format!("{:.1}", p.mine_lost)
        };
        let dig = mc_sim::mines::mine_power(mc_core::Fx::from_f32(p.mine_power)).to_f32();
        format!(
            "No power for the mines  \u{b7}  digging at {:.0}%  \u{b7}  -{lost} materials/s",
            dig * 100.0
        )
    })
}
