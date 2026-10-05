//! An AI commander's settings in the line-up: its doctrine, the forces it
//! favours and the income it is given. They sit in a line of choices under the AI's row (`seats`), and
//! are said in short wherever a line will not fit.

use super::roster::Seat;
use crate::ui::{id, palette, rgb, type_scale, Rect, Ui};
use glam::Vec2;
use mc_sim::Doctrine;

/// The line of an AI's settings under its row.
pub(super) const AI_LINE: f32 = 36.0;
const FORCE_PRESETS: [[u8; 3]; 4] = [[100, 100, 100], [160, 60, 60], [60, 160, 60], [60, 60, 160]];
const FORCE_LABELS: [&str; 4] = ["Balanced", "Land", "Air", "Naval"];

/// The incomes an AI may be given, in thousandths of a fair share (`AiConfig::income`).
const INCOMES: [u16; 5] = [1000, 1500, 2000, 3000, 5000];
const INCOME_LABELS: [&str; 5] = ["1x", "1.5x", "2x", "3x", "5x"];

const DOCTRINES: [Doctrine; 4] = [
    Doctrine::Adaptive,
    Doctrine::Aggressive,
    Doctrine::Economic,
    Doctrine::Defensive,
];

/// An AI's settings as a line of choices in `r`: its doctrine, the forces it
/// favours and its income. Whoever may plan changes them; everyone else reads them.
pub(super) fn ai_line(ui: &mut Ui, seat: &mut Seat, r: Rect, host: bool) {
    let key = seat.key as usize;
    let ai = &mut seat.ai;
    let doctrines = DOCTRINES.map(doctrine_label);
    // A custom mix (from a saved config) is said as such until changed.
    let force_at = FORCE_PRESETS.iter().position(|w| *w == ai.domain_weights);
    let mut forces = FORCE_LABELS.to_vec();
    if force_at.is_none() {
        forces.push("Custom");
    }
    let income_at = INCOMES.iter().position(|i| *i == ai.income);
    let mut incomes = INCOME_LABELS.to_vec();
    if income_at.is_none() {
        incomes.push("Custom");
    }
    let fields: [(&str, &[&str], usize); 3] = [
        (
            "Doctrine",
            &doctrines,
            DOCTRINES
                .iter()
                .position(|d| *d == ai.doctrine)
                .unwrap_or(0),
        ),
        ("Forces", &forces, force_at.unwrap_or(FORCE_LABELS.len())),
        ("Income", &incomes, income_at.unwrap_or(INCOMES.len())),
    ];
    // Each as wide as its label and longest option; the labels go when the
    // line is too narrow for them, and the choices share what is left over.
    let gap = 6.0;
    let natural = |ui: &mut Ui, label: &str, options: &[&str], labelled: bool| {
        let value = options
            .iter()
            .map(|o| ui.text_width(type_scale::CAPTION, o))
            .fold(0.0, f32::max);
        let label = if labelled {
            ui.text_width(type_scale::MICRO, label) + 7.0
        } else {
            0.0
        };
        label + value + 34.0
    };
    let room = r.w - gap * (fields.len() - 1) as f32;
    let mut labelled = true;
    let mut widths: Vec<f32> = fields
        .iter()
        .map(|(l, o, _)| natural(ui, l, o, true))
        .collect();
    if widths.iter().sum::<f32>() > room {
        labelled = false;
        widths = fields
            .iter()
            .map(|(l, o, _)| natural(ui, l, o, false))
            .collect();
    }
    let k = room / widths.iter().sum::<f32>();
    let mut x = r.x;
    for (n, ((label, options, at), w)) in fields.iter().zip(widths).enumerate() {
        let w = w * k;
        let cell = Rect::new(x, r.y, w, r.h);
        let label = if labelled { *label } else { "" };
        if let Some(pick) = ui.choice(id("ai-line", key * 4 + n), cell, label, options, *at, host) {
            match n {
                0 => ai.doctrine = DOCTRINES[pick],
                1 => {
                    if let Some(w) = FORCE_PRESETS.get(pick) {
                        ai.domain_weights = *w;
                    }
                }
                _ => {
                    if let Some(i) = INCOMES.get(pick) {
                        ai.income = *i;
                    }
                }
            }
        }
        x += w + gap;
    }
}

/// "AI Settings" under an AI's name, with a caret: the list is too crowded
/// to show every AI's line, and a click opens (or closes) this one's. True
/// when clicked.
pub(super) fn settings_toggle(
    ui: &mut Ui,
    key: usize,
    open: bool,
    x: f32,
    row: Rect,
    name_w: f32,
) -> bool {
    let text = "AI Settings";
    let tw = ui.text_width(type_scale::MICRO, text);
    let tune = Rect::new(x - 4.0, row.y + 28.0, (tw + 24.0).min(name_w + 8.0), 22.0);
    let res = ui.interact(id("slot-ai-tune", key), tune, true);
    let tone = rgb(
        if open || res.glow > 0.3 {
            palette::ACCENT
        } else {
            palette::DIM
        },
        1.0,
    );
    ui.text(x, row.y + 38.0, type_scale::MICRO, tone, text);
    caret(ui, x + tw + 9.0, row.y + 38.0, open, res.glow);
    res.clicked
}

/// An AI's settings in short: "Adaptive \u{b7} Balanced", and its income
/// when it is not a fair share: "\u{b7} 2x Income".
pub(super) fn summary(ai: &mc_sim::AiConfig) -> String {
    let mut s = format!(
        "{}  \u{b7}  {}",
        doctrine_label(ai.doctrine),
        force_label(ai.domain_weights)
    );
    if ai.income != 1000 {
        s += &format!("  \u{b7}  {} Income", income_label(ai.income));
    }
    s
}

/// "2x", "1.5x", "0.25x": an income in thousandths as a multiple.
fn income_label(permille: u16) -> String {
    let whole = permille / 1000;
    match permille % 1000 {
        0 => format!("{whole}x"),
        part => format!("{}x", format!("{whole}.{part:03}").trim_end_matches('0')),
    }
}

/// A caret at `x`, `y`: down for something that opens, up once it is open.
pub(super) fn caret(ui: &mut Ui, x: f32, y: f32, open: bool, glow: f32) {
    let d = if open { -1.0 } else { 1.0 };
    let tone = rgb(
        if open || glow > 0.3 {
            palette::ACCENT
        } else {
            palette::DIM
        },
        1.0,
    );
    let c = Vec2::new(x, y);
    ui.triangle(
        c + Vec2::new(-3.5, -2.0 * d),
        c + Vec2::new(3.5, -2.0 * d),
        c + Vec2::new(0.0, 2.5 * d),
        tone,
    );
}

fn doctrine_label(d: Doctrine) -> &'static str {
    match d {
        Doctrine::Adaptive => "Adaptive",
        Doctrine::Aggressive => "Aggressive",
        Doctrine::Economic => "Economic",
        Doctrine::Defensive => "Defensive",
    }
}

fn force_label(weights: [u8; 3]) -> &'static str {
    FORCE_PRESETS
        .iter()
        .position(|w| *w == weights)
        .map_or("Custom", |i| FORCE_LABELS[i])
}

#[cfg(test)]
mod tests {
    #[test]
    fn incomes_are_said_as_multiples() {
        assert_eq!(super::income_label(1500), "1.5x");
        assert_eq!(super::income_label(5000), "5x");
        assert_eq!(super::income_label(250), "0.25x");
        assert_eq!(super::income_label(1125), "1.125x");
    }
}
