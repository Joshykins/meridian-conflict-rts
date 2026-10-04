//! A unit's own priority in a stall (`Command::SetPriority`): Last / Even / First, the
//! economy row's control (`focus.rs`) for the selected units alone. It sits on the queue
//! strip beside Pause, and small in the head of the order card's column that holds Pause.
//! First overrides the Mines/Power row for the unit's work; Even leaves it to the row.

use super::build::{tip, BUILDING};
use super::segmented;
use super::{HudAction, Scene};
use crate::audio::Sfx;
use crate::ui::{id, palette, rgb, type_scale, Color, Rect, Ui};
use glam::Vec2;
use mc_sim::focus::Priority;
use mc_sim::mirror::UnitInstance;

/// The segments, left to right.
const SEGMENTS: [(Priority, &str); 3] = [
    (Priority::Last, "Last"),
    (Priority::Even, "Even"),
    (Priority::First, "First"),
];

/// Where a priority's segment sits, 0 (Last) to 2 (First).
fn slot(p: Priority) -> usize {
    match p {
        Priority::Last => 0,
        Priority::Even => 1,
        Priority::First => 2,
    }
}

/// How a selection's units that can take a priority stand, by segment.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::hud) struct Mix {
    counts: [usize; 3],
}

impl Mix {
    /// The selected units with work a priority orders, and sites still going up.
    pub(in crate::hud) fn of(s: &Scene, units: &[&UnitInstance]) -> Mix {
        let mut mix = Mix::default();
        for u in units {
            if super::has_flag(u, mc_sim::tables::flag::UNDER_CONSTRUCTION)
                || mc_sim::focus::prioritizable(s.blueprints, s.bp(u))
            {
                mix.counts[slot(u.priority())] += 1;
            }
        }
        mix
    }

    pub(in crate::hud) fn any(self) -> bool {
        self.counts.iter().sum::<usize>() > 0
    }

    /// The priority all of them share, if they do.
    pub(in crate::hud) fn shared(self) -> Option<Priority> {
        let total: usize = self.counts.iter().sum();
        SEGMENTS
            .iter()
            .map(|&(p, _)| p)
            .find(|&p| total > 0 && self.counts[slot(p)] == total)
    }

    /// "2 First · 3 Even", for a split selection's tip.
    fn split_line(self) -> String {
        SEGMENTS
            .iter()
            .rev()
            .filter(|&&(p, _)| self.counts[slot(p)] > 0)
            .map(|&(p, name)| format!("{} {name}", self.counts[slot(p)]))
            .collect::<Vec<_>>()
            .join("  \u{b7}  ")
    }
}

/// How big the control is drawn.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::hud) enum Size {
    /// On the queue strip: a chevron over each segment's name.
    Full,
    /// On a folded strip, or in the order card's column head: chevrons alone.
    Glyphs,
}

/// The colour a segment lights in: grey for Last, white for Even, construction amber for
/// First (the strategic icon's chevron is the same).
fn tone(p: Priority) -> u32 {
    match p {
        Priority::Last => palette::DIM,
        Priority::Even => palette::TEXT,
        Priority::First => BUILDING,
    }
}

/// What a segment does, for its tip.
fn what(p: Priority) -> &'static str {
    match p {
        Priority::First => "First: in a stall, this work is paid in full before anything else. Helpers and builders on its sites go first with it",
        Priority::Even => "Even: this work follows the Mines/Power row under the economy panel",
        Priority::Last => "Last: this work is built only out of what everything else leaves over",
    }
}

/// Draws the control in `r` for `mix`, keyed by `key`. A click on a segment sets the whole
/// selection to it; on the one they all share already, back to Even. The tip goes `tip_up`
/// over the control. Pushes the order to the HUD's actions.
pub(in crate::hud) fn control(
    hud: &mut super::Hud,
    ui: &mut Ui,
    r: Rect,
    mix: Mix,
    size: Size,
    key: &'static str,
    tip_up: f32,
) {
    let shared = mix.shared();
    // Interaction first, so the body can brighten under the pointer before its marks.
    let res: [_; 3] =
        std::array::from_fn(|n| ui.interact(id(key, n), segmented::segment(r, n, 3), true));
    segmented::body(ui, r, res.iter().fold(0.0, |a, s| s.glow.max(a)));
    // The pill slides to the segment the selection shares, and takes its colour.
    let at = ui.ease(id(key, 9), shared.map_or(1.0, |p| slot(p) as f32), 16.0);
    let lit = ui.ease(id(key, 10), if shared.is_some() { 1.0 } else { 0.0 }, 16.0);
    segmented::pill(ui, r, at, 3, blend(at), lit);
    let total: usize = mix.counts.iter().sum();
    let mut hover = None;
    for (n, &(which, label)) in SEGMENTS.iter().enumerate() {
        let sr = segmented::segment(r, n, 3);
        let res = &res[n];
        segmented::hover(ui, r, n, 3, res.glow);
        segmented::divider(ui, r, n, 3, shared.map(|_| at));
        let on = shared == Some(which);
        let some = mix.counts[n] > 0;
        // Lit, it reads white on its pill, as a lit tile's glyph does.
        let c = if on {
            rgb(0xFFFFFF, 1.0)
        } else if some {
            // A split selection: each part that has it is half lit.
            rgb(tone(which), 0.75)
        } else {
            rgb(palette::FAINT, 0.85 + 0.15 * res.glow)
        };
        let mid = sr.x + sr.w * 0.5;
        match size {
            Size::Full => {
                chevron(ui, Vec2::new(mid, sr.mid_y() - 6.0), which, 4.0, c);
                ui.text_centred(mid, sr.mid_y() + 7.0, type_scale::MICRO, c, label);
            }
            Size::Glyphs => chevron(ui, Vec2::new(mid, sr.mid_y()), which, 3.5, c),
        }
        // A split selection: a gauge along each segment's foot, filled to its share.
        if shared.is_none() && some {
            let inner = segmented::inner_segment(r, n, 3);
            let g = Rect::new(inner.x + 3.0, inner.bottom() - 2.0, inner.w - 6.0, 2.0);
            ui.fill(g, rgb(tone(which), 0.18));
            ui.fill(
                Rect::new(g.x, g.y, g.w * mix.counts[n] as f32 / total as f32, g.h),
                rgb(tone(which), 0.95),
            );
        }
        if res.hovered {
            hover = Some(which);
        }
        if res.clicked {
            let next = if on { Priority::Even } else { which };
            ui.audio.play(match next {
                Priority::First => Sfx::ToggleOn,
                Priority::Last => Sfx::ToggleOff,
                Priority::Even => Sfx::Tick,
            });
            hud.actions.push(HudAction::Priority(next));
        }
    }
    if let Some(which) = hover {
        let act = if shared == Some(which) && which != Priority::Even {
            "  \u{b7}  Click to set back to Even"
        } else {
            ""
        };
        let split = if shared.is_none() {
            format!("  \u{b7}  Now {}", mix.split_line())
        } else {
            String::new()
        };
        tip(
            ui,
            r.x,
            r.y - tip_up,
            &format!("Priority  \u{b7}  {}{split}{act}", what(which)),
        );
    }
}

/// The queue strip's note on a unit's own priority: what it is, and in a stall how fast
/// its tier goes (`Player::tier_speed`). `None` for Even.
pub(in crate::hud) fn note(s: &Scene, p: Priority) -> Option<(String, u32)> {
    let word = match p {
        Priority::Even => return None,
        Priority::First => "First",
        Priority::Last => "Last",
    };
    let player = s.view.status.players.get(s.view.local as usize);
    let speed = player
        .filter(|pl| pl.efficiency < 0.999)
        .and_then(|pl| pl.tier_speed[p.tier()]);
    let text = match speed {
        Some(k) => format!("{word}  \u{b7}  stalling, at {:.0}%", k * 100.0),
        None => format!("{word} in a stall"),
    };
    Some((text, tone(p)))
}

/// A chevron about `c`, `s` half wide: up for First, down for Last, a bar for Even.
fn chevron(ui: &mut Ui, c: Vec2, p: Priority, s: f32, color: Color) {
    let lift = match p {
        Priority::First => -1.0,
        Priority::Last => 1.0,
        Priority::Even => {
            ui.stroke(c - Vec2::new(s, 0.0), c + Vec2::new(s, 0.0), 1.6, color);
            return;
        }
    };
    let tip = c + Vec2::new(0.0, lift * s * 0.55);
    let foot = lift * -s * 0.55;
    ui.stroke(c + Vec2::new(-s, foot), tip, 1.6, color);
    ui.stroke(tip, c + Vec2::new(s, foot), 1.6, color);
}

/// The highlight's colour at `at` segments from Last.
fn blend(at: f32) -> Color {
    let (a, b, k) = if at < 1.0 {
        (tone(Priority::Last), tone(Priority::Even), at)
    } else {
        (tone(Priority::Even), tone(Priority::First), at - 1.0)
    };
    let (a, b) = (rgb(a, 1.0), rgb(b, 1.0));
    std::array::from_fn(|n| a[n] + (b[n] - a[n]) * k)
}
