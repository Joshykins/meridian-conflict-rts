//! The economy's "build first" switches, joined under the economy panel: Mines first
//! under materials, Power first under energy (`mc_sim::focus`). One or neither is on.
//! A resource that stalls lights its switch as the fix, and says so in a note.

use super::build::tip;
use super::notices::{Glyph, Notices};
use super::{Hud, HudAction, ENERGY, LOW, MASS};
use crate::audio::Sfx;
use crate::sim_thread::PlayerStatus;
use crate::ui::{id, ink, palette, rgb, type_scale, Color, Rect, Ui};
use glam::Vec2;
use mc_sim::focus::Focus;

/// Height of the switch strip under the economy's figures.
pub const FOCUS_H: f32 = 38.0;
/// How long a stall lasts before its note, seconds: a blip is not worth one.
const NOTE_AFTER: f32 = 1.5;
/// How long a stall must be over before a new one gets a new note, seconds.
const NOTE_QUIET: f32 = 20.0;
/// How often a stall that goes on is noted again, seconds.
const NOTE_AGAIN: f32 = 60.0;
/// How long a click shows its answer before the sim's own arrives, seconds.
const PENDING: f32 = 0.6;

/// How a resource stands this frame (`Hud::economy`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Short {
    Fine,
    /// Running dry within seconds.
    Low,
    /// Dry, and asked for more than comes in.
    Dry,
}

/// The switches' eased knobs and the stall notes' timers. Presentation only.
#[derive(Default)]
pub struct FocusUi {
    /// Knob travel per switch, 0 off to 1 on, materials then energy.
    knob: [f32; 2],
    /// A click not yet answered by the sim: what it asked for, and seconds left to show it.
    pending: Option<(Focus, f32)>,
    /// Seconds each resource has been dry, and been fine since it last was.
    dry: [f32; 2],
    fine: [f32; 2],
    /// Seconds since each stall's last note; `None`: this stall has had none.
    noted: [Option<f32>; 2],
}

/// The switches, in panel order: under materials, under energy.
const SWITCHES: [(Focus, &str, u32); 2] = [
    (Focus::Materials, "Mines first", MASS),
    (Focus::Power, "Power first", ENERGY),
];

impl FocusUi {
    /// The focus to show: a click's answer until the sim's arrives.
    fn shown(&mut self, p: &PlayerStatus, dt: f32) -> Focus {
        match &mut self.pending {
            Some((f, left)) if *f != p.focus && *left > 0.0 => {
                *left -= dt;
                *f
            }
            _ => {
                self.pending = None;
                p.focus
            }
        }
    }

    /// Notes a stall that has lasted, once per stall and again each minute it goes on,
    /// naming what to build.
    pub fn watch(&mut self, p: &PlayerStatus, short: [Short; 2], dt: f32, notices: &mut Notices) {
        for i in 0..2 {
            if short[i] != Short::Dry {
                self.dry[i] = 0.0;
                self.fine[i] += dt;
                if self.fine[i] > NOTE_QUIET {
                    self.noted[i] = None;
                }
                continue;
            }
            self.fine[i] = 0.0;
            self.dry[i] += dt;
            if let Some(t) = &mut self.noted[i] {
                *t += dt;
            }
            let due = match self.noted[i] {
                None => self.dry[i] > NOTE_AFTER,
                Some(t) => t > NOTE_AGAIN,
            };
            if !due {
                continue;
            }
            self.noted[i] = Some(0.0);
            let (focus, _, tone) = SWITCHES[i];
            let build = if i == 0 {
                "Materials stalling  \u{b7}  build more mines or reclaim wrecks"
            } else {
                "Energy stalling  \u{b7}  build more power generators"
            };
            let title = if p.focus == focus {
                build.to_owned()
            } else {
                format!(
                    "{build}, or put {} first",
                    if i == 0 { "mines" } else { "power" }
                )
            };
            notices.note(format!("stall-{i}"), title, tone, Glyph::Bar, None);
        }
    }
}

/// The strip across the bottom of the economy panel `panel`, one switch under each
/// resource's figures.
pub fn strip(
    hud: &mut Hud,
    ui: &mut Ui,
    p: &PlayerStatus,
    panel: Rect,
    short: [Short; 2],
    dt: f32,
) {
    let top = panel.bottom() - FOCUS_H;
    ui.hline(
        panel.x + 10.0,
        top,
        panel.w - 20.0,
        rgb(palette::LINE, 0.10),
    );
    let focus = hud.focus_ui.shown(p, dt);
    let block = (panel.w - 46.0) * 0.5;
    for (i, &(which, label, tone)) in SWITCHES.iter().enumerate() {
        let on = focus == which;
        let k = &mut hud.focus_ui.knob[i];
        *k += (f32::from(u8::from(on)) - *k) * (dt * 14.0).min(1.0);
        let knob = *k;
        let r = Rect::new(
            panel.x + 16.0 + i as f32 * (block + 14.0),
            top + 6.0,
            block - 12.0,
            26.0,
        );
        let face = Face {
            label,
            tone,
            on,
            knob,
            short: short[i],
            paid: p.focus_efficiency,
        };
        if switch(ui, r, i, &face, which) {
            let next = if on { Focus::Neither } else { which };
            ui.audio
                .play(if on { Sfx::ToggleOff } else { Sfx::ToggleOn });
            hud.focus_ui.pending = Some((next, PENDING));
            hud.actions.push(HudAction::Focus(next));
        }
    }
}

/// What one switch shows.
struct Face {
    label: &'static str,
    tone: u32,
    on: bool,
    /// Eased knob travel, 0..1.
    knob: f32,
    short: Short,
    /// Share of the focus paid last tick.
    paid: f32,
}

/// One switch; whether it was clicked.
fn switch(ui: &mut Ui, r: Rect, i: usize, f: &Face, which: Focus) -> bool {
    let res = ui.interact(id("hud-focus", i), r, true);
    let t = ui.time;
    let glow = res.glow;
    // Off while its resource runs short: it is the fix, and it breathes to say so.
    let alarm = match f.short {
        Short::Dry => Some((palette::BAD, 0.5 + 0.5 * (t * 6.0).sin())),
        Short::Low => Some((LOW, 0.5 + 0.5 * (t * 3.5).sin())),
        Short::Fine => None,
    }
    .filter(|_| !f.on);

    ui.fill(r, ink(0.35));
    if f.knob > 0.01 {
        ui.gradient_h(
            r,
            rgb(f.tone, (0.26 + 0.1 * glow) * f.knob),
            rgb(f.tone, 0.03 * f.knob),
        );
    }
    if let Some((c, k)) = alarm {
        ui.gradient_h(r, rgb(c, 0.08 + 0.14 * k), rgb(c, 0.0));
        sheen(ui, r, t);
    }
    if glow > 0.01 {
        ui.fill(r, rgb(palette::TEXT, 0.05 * glow));
    }
    let edge = match alarm {
        Some((c, k)) => rgb(c, 0.35 + 0.55 * k),
        None if f.on => rgb(f.tone, 0.55 + 0.3 * glow),
        None => rgb(palette::LINE, 0.10 + 0.3 * glow),
    };
    ui.frame(r, edge);
    ui.fill(
        Rect::new(r.x, r.y, 2.0 + f.knob, r.h),
        match alarm {
            Some((c, k)) => rgb(c, 0.5 + 0.5 * k),
            None => rgb(f.tone, 0.25 + 0.75 * f.knob),
        },
    );

    // The toggle: a track whose knob slides over and takes the resource's colour.
    let mid = r.mid_y();
    let track = Rect::new(r.x + 12.0, mid - 6.0, 26.0, 12.0);
    ui.fill(track, ink(0.6));
    ui.fill(
        Rect::new(track.x, track.y, track.w * f.knob, track.h),
        rgb(f.tone, 0.35 * f.knob),
    );
    ui.frame(track, rgb(palette::LINE, 0.18 + 0.2 * glow));
    let knob_ink = match alarm {
        Some((c, k)) => rgb(c, 0.6 + 0.4 * k),
        None => mix(rgb(palette::DIM, 0.9), rgb(f.tone, 1.0), f.knob),
    };
    ui.fill(
        Rect::new(track.x + 2.0 + f.knob * 12.0, track.y + 2.0, 10.0, 8.0),
        knob_ink,
    );

    let mark = Vec2::new(track.right() + 16.0, mid);
    let mark_ink = if f.on {
        rgb(f.tone, 1.0)
    } else {
        rgb(palette::TEXT, 0.6 + 0.3 * glow)
    };
    match which {
        Focus::Power => bolt(ui, mark, 7.0, mark_ink),
        _ => ore(ui, mark, 6.5, mark_ink),
    }
    ui.text(
        mark.x + 13.0,
        mid,
        type_scale::CAPTION,
        rgb(palette::TEXT, if f.on { 1.0 } else { 0.78 + 0.22 * glow }),
        f.label,
    );

    let (state, tone) = match (f.on, f.short) {
        (true, _) if f.paid < 0.995 => (
            format!("Paid first  \u{b7}  {:.0}%", f.paid * 100.0),
            f.tone,
        ),
        (true, _) => ("Paid first".to_owned(), f.tone),
        (false, Short::Dry) => ("Stalling  \u{b7}  click to fix".to_owned(), palette::BAD),
        (false, Short::Low) => ("Running low".to_owned(), LOW),
        (false, Short::Fine) => ("Off".to_owned(), palette::FAINT),
    };
    let lit = match alarm {
        Some((_, k)) => 0.7 + 0.3 * k,
        None => 1.0,
    };
    ui.text_right(
        r.right() - 10.0,
        mid,
        type_scale::MICRO,
        rgb(tone, lit),
        &state,
    );

    if res.hovered {
        let what = if which == Focus::Power {
            "Power first: while short, new power plants and their upgrades are paid in full before anything else"
        } else {
            "Mines first: while short, new mines and their upgrades are paid in full before anything else"
        };
        let act = if f.on {
            "Click to turn off"
        } else {
            "Click to turn on"
        };
        tip(ui, r.x, r.bottom() + 8.0, &format!("{what}  \u{b7}  {act}"));
    }
    res.clicked
}

/// A soft band of light that sweeps across `r` again and again.
fn sheen(ui: &mut Ui, r: Rect, t: f32) {
    let band = 46.0;
    let x = r.x - band + (t * 0.55).fract() * (r.w + band * 2.0);
    let (lo, hi) = (x.max(r.x), (x + band).min(r.right()));
    if hi <= lo {
        return;
    }
    let half = x + band * 0.5;
    if half > lo {
        ui.gradient_h(
            Rect::new(lo, r.y, half.min(hi) - lo, r.h),
            rgb(palette::TEXT, 0.0),
            rgb(palette::TEXT, 0.09),
        );
    }
    if hi > half {
        let from = half.max(lo);
        ui.gradient_h(
            Rect::new(from, r.y, hi - from, r.h),
            rgb(palette::TEXT, 0.09),
            rgb(palette::TEXT, 0.0),
        );
    }
}

/// A lightning bolt of half-height `s` about `c`.
fn bolt(ui: &mut Ui, c: Vec2, s: f32, color: Color) {
    let pts = [
        Vec2::new(0.3, -1.0),
        Vec2::new(-0.4, 0.12),
        Vec2::new(0.12, 0.12),
        Vec2::new(-0.3, 1.0),
        Vec2::new(0.4, -0.12),
        Vec2::new(-0.12, -0.12),
    ];
    outline(ui, c, s, &pts, color);
}

/// A cut ore crystal of half-height `s` about `c`.
fn ore(ui: &mut Ui, c: Vec2, s: f32, color: Color) {
    let pts = [
        Vec2::new(0.0, -1.0),
        Vec2::new(0.8, -0.35),
        Vec2::new(0.5, 0.95),
        Vec2::new(-0.5, 0.95),
        Vec2::new(-0.8, -0.35),
    ];
    outline(ui, c, s, &pts, color);
    ui.stroke(
        c + Vec2::new(-0.8, -0.35) * s,
        c + Vec2::new(0.8, -0.35) * s,
        1.3,
        color,
    );
    ui.stroke(
        c + Vec2::new(0.0, -1.0) * s,
        c + Vec2::new(0.0, 0.95) * s,
        1.1,
        color,
    );
}

/// A closed outline through `pts`, scaled by `s` about `c`.
fn outline(ui: &mut Ui, c: Vec2, s: f32, pts: &[Vec2], color: Color) {
    for (k, &a) in pts.iter().enumerate() {
        let b = pts[(k + 1) % pts.len()];
        ui.stroke(c + a * s, c + b * s, 1.5, color);
    }
}

fn mix(a: Color, b: Color, k: f32) -> Color {
    std::array::from_fn(|n| a[n] + (b[n] - a[n]) * k)
}
