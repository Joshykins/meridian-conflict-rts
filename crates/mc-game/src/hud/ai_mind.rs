//! The AI's mind, for an observer (`docs/AI_COMMANDER.md`, "Overlay"): when the
//! watched side is played by a Commander, a card down the right column shows its
//! plans and stakes, each operation with its phase, size and trade, what it has
//! seen and what has hurt it, and its last decisions with why.

use super::{Hud, Scene};
use crate::ui::{palette, rgb, type_scale, Rect, Ui};
use mc_sim::ai::AiMind;

const LINE: f32 = 17.0;
const HEAD: f32 = 22.0;
/// Plans shown that it does not hold: the next best few.
const UNHELD: usize = 3;
const NOTES: usize = 5;

/// Lines the card needs, to size it before drawing.
fn lines(m: &AiMind) -> (usize, usize) {
    let plans = m.plans.iter().filter(|p| p.level > 0).count() + UNHELD;
    let ops = m.ops.len().max(1);
    (plans, ops)
}

fn kilo(m: i32) -> String {
    if m >= 10_000 {
        format!("{}k", m / 1000)
    } else if m >= 1000 {
        format!("{}.{}k", m / 1000, m / 100 % 10)
    } else {
        m.to_string()
    }
}

impl Hud {
    /// Draws the card from `top` down, right edge at `right`, no lower than `bottom`.
    pub(super) fn ai_mind(&mut self, ui: &mut Ui, s: &Scene, right: f32, top: f32, bottom: f32) {
        let view = s.view;
        if !view.observing {
            return;
        }
        let Some(who) = view.perspective else { return };
        let Some(p) = view.status.players.get(who as usize) else {
            return;
        };
        let Some(m) = &p.mind else { return };
        let w = 340.0;
        let (plans, ops) = lines(m);
        let want =
            HEAD * 4.0 + LINE * (plans + ops + 2 + NOTES.min(m.notes.len().max(1))) as f32 + 16.0;
        let h = want.min(bottom - top);
        if h < HEAD * 3.0 {
            return;
        }
        let r = Rect::new(right - w, top, w, h);
        self.glass(ui, r);
        self.claim(ui, r);
        ui.fill(Rect::new(r.x, r.y, 3.0, r.h), s.team_color(who));
        let (x, x2) = (r.x + 14.0, r.right() - 12.0);
        let mut y = r.y + 14.0;
        let fits = |y: f32| y + LINE * 0.5 <= r.bottom() - 6.0;
        let dim = rgb(palette::DIM, 1.0);
        let faint = rgb(palette::FAINT, 1.0);
        let text = rgb(palette::TEXT, 1.0);
        let heading = |ui: &mut Ui, y: &mut f32, title: &str| {
            ui.text(x, *y, type_scale::MICRO, dim, title);
            *y += HEAD - 4.0;
        };

        ui.text(x, y, type_scale::OVERLINE, text, "AI MIND");
        ui.text_right(x2, y, type_scale::CAPTION, s.team_color(who), &p.name);
        y += HEAD + 2.0;

        // Plans: what it holds, with a four-step stake bar, then what came next.
        heading(ui, &mut y, "PLANS  \u{b7}  STAKE  \u{b7}  APPEAL");
        let held = m.plans.iter().filter(|p| p.level > 0);
        let next = m.plans.iter().filter(|p| p.level == 0).take(UNHELD);
        for plan in held.chain(next) {
            if !fits(y) {
                return;
            }
            let on = plan.level > 0;
            ui.text(
                x,
                y,
                type_scale::CAPTION,
                if on { text } else { faint },
                plan.name,
            );
            for k in 0..3u8 {
                let seg = Rect::new(x + 150.0 + f32::from(k) * 22.0, y - 3.0, 18.0, 6.0);
                let lit = k < plan.level;
                ui.fill(
                    seg,
                    if lit {
                        if plan.level == 3 {
                            rgb(palette::ACCENT, 1.0)
                        } else {
                            rgb(palette::WARN, 0.9)
                        }
                    } else {
                        rgb(palette::LINE, 0.10)
                    },
                );
            }
            ui.text(
                x + 222.0,
                y,
                type_scale::MICRO,
                if on { dim } else { faint },
                plan.stake,
            );
            ui.text_right(
                x2,
                y,
                type_scale::VALUE,
                if on { text } else { faint },
                &plan.appeal.to_string(),
            );
            y += LINE;
        }
        y += 6.0;

        // Operations: kind, phase, size against what it wants, its trade.
        heading(
            ui,
            &mut y,
            "OPERATIONS  \u{b7}  SIZE  \u{b7}  KILLED / LOST",
        );
        if m.ops.is_empty() && fits(y) {
            ui.text(x, y, type_scale::CAPTION, faint, "None");
            y += LINE;
        }
        for op in &m.ops {
            if !fits(y) {
                return;
            }
            let tone = if op.engaged { text } else { dim };
            let end = ui.text(x, y, type_scale::CAPTION, tone, op.kind);
            ui.text(
                end.max(x + 70.0),
                y,
                type_scale::MICRO,
                if op.engaged {
                    rgb(palette::ACCENT, 1.0)
                } else {
                    faint
                },
                op.phase,
            );
            let size = if op.want > 0 {
                format!("{}u {}/{}", op.units, kilo(op.mass), kilo(op.want))
            } else {
                format!("{}u {}", op.units, kilo(op.mass))
            };
            ui.text(x + 158.0, y, type_scale::MICRO, dim, &size);
            let good = op.killed >= op.lost;
            ui.text_right(
                x2,
                y,
                type_scale::MICRO,
                if op.killed + op.lost == 0 {
                    faint
                } else if good {
                    rgb(palette::TEXT, 0.95)
                } else {
                    rgb(palette::BAD, 1.0)
                },
                &format!("{} / {}", kilo(op.killed), kilo(op.lost)),
            );
            y += LINE;
        }
        y += 6.0;

        // What it knows and what hurts it, a line each.
        if fits(y) {
            let seen: Vec<String> = m
                .seen
                .iter()
                .map(|(k, ago)| match ago {
                    Some(0) => format!("{k} now"),
                    Some(a) => format!("{k} {a}m"),
                    None => format!("{k} \u{2014}"),
                })
                .collect();
            ui.text(
                x,
                y,
                type_scale::MICRO,
                dim,
                &format!("SEEN  {}", seen.join("  ")),
            );
            y += LINE;
        }
        if fits(y) {
            let hurt = if m.hurt.is_empty() {
                "nothing lately".to_string()
            } else {
                m.hurt
                    .iter()
                    .map(|(k, v)| format!("{k} {}", kilo(*v)))
                    .collect::<Vec<_>>()
                    .join("  ")
            };
            ui.text(x, y, type_scale::MICRO, dim, &format!("HURT BY  {hurt}"));
            y += LINE + 6.0;
        }

        // The last decisions, newest first.
        if fits(y) {
            heading(ui, &mut y, "DECISIONS");
        }
        for n in m.notes.iter().rev().take(NOTES) {
            if !fits(y) {
                return;
            }
            ui.text(x, y, type_scale::MICRO, faint, &format!("{}m", n.minute));
            let end = ui.text(x + 34.0, y, type_scale::CAPTION, text, n.plan);
            ui.text(
                end + 6.0,
                y,
                type_scale::MICRO,
                dim,
                &format!("to {}  ({})", n.stake, n.why),
            );
            y += LINE;
        }
    }
}
