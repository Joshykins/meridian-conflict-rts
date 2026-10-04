//! The economy's priorities, one slim row under the economy panel's figures: new
//! mines under materials, new power under energy, each paid Last, Even or First in
//! a stall (`mc_sim::focus`). A resource that stalls lights its kind's First as the
//! fix, and says so in a note.

use super::build::tip;
use super::notices::{Glyph, Notices};
use super::segmented;
use super::{Hud, HudAction, ENERGY, LOW, MASS};
use crate::audio::Sfx;
use crate::sim_thread::PlayerStatus;
use crate::ui::{id, palette, rgb, type_scale, Color, Rect, Ui};
use glam::Vec2;
use mc_sim::focus::{Focus, Priority};

/// Height of the priority row under the economy's figures.
pub const FOCUS_H: f32 = 24.0;
/// One segment of a kind's Last / Even / First control.
const SEG_W: f32 = 44.0;
const SEG_H: f32 = 18.0;
/// The segments, left to right.
const SEGMENTS: [(Priority, &str); 3] = [
    (Priority::Last, "Last"),
    (Priority::Even, "Even"),
    (Priority::First, "First"),
];
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

/// The kinds, in panel order: mines under materials, power under energy.
#[derive(Clone, Copy)]
struct Kind {
    name: &'static str,
    tone: u32,
    /// What its resource's stall note tells the player to build.
    build: &'static str,
    /// What else First puts first with it (`Focus::priority`), for the tip.
    first_also: &'static str,
}

const KINDS: [Kind; 2] = [
    Kind {
        name: "Mines",
        tone: MASS,
        build: "Materials stalling  \u{b7}  build more mines or reclaim wrecks",
        first_also: ", reclaimers, economy refits",
    },
    Kind {
        name: "Power",
        tone: ENERGY,
        build: "Energy stalling  \u{b7}  build more power generators",
        first_also: ", economy refits",
    },
];

fn get(f: Focus, i: usize) -> Priority {
    if i == 0 {
        f.mines
    } else {
        f.power
    }
}

fn with(mut f: Focus, i: usize, p: Priority) -> Focus {
    if i == 0 {
        f.mines = p;
    } else {
        f.power = p;
    }
    f
}

/// Where a priority's segment sits, 0 (Last) to 2 (First).
fn slot(p: Priority) -> f32 {
    match p {
        Priority::Last => 0.0,
        Priority::Even => 1.0,
        Priority::First => 2.0,
    }
}

/// The controls' eased highlights and the stall notes' timers. Presentation only.
#[derive(Default)]
pub struct FocusUi {
    /// Each kind's highlight position, in segments from Last; `None` before the first frame.
    pos: [Option<f32>; 2],
    /// A click not yet answered by the sim: what it asked for, and seconds left to show it.
    pending: Option<(Focus, f32)>,
    /// Seconds each resource has been dry, and been fine since it last was.
    dry: [f32; 2],
    fine: [f32; 2],
    /// Seconds since each stall's last note; `None`: this stall has had none.
    noted: [Option<f32>; 2],
}

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
            let kind = KINDS[i];
            let title = if get(p.focus, i) == Priority::First {
                kind.build.to_owned()
            } else {
                format!("{}, or put {} first", kind.build, kind.name.to_lowercase())
            };
            notices.note(format!("stall-{i}"), title, kind.tone, Glyph::Bar, None);
        }
    }
}

/// The row across the bottom of the economy panel `panel`: each kind's control under
/// its resource's figures.
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
    let stalling = p.efficiency < 0.999;
    for (i, kind) in KINDS.iter().enumerate() {
        let now = get(focus, i);
        let pos = hud.focus_ui.pos[i].get_or_insert(slot(now));
        *pos += (slot(now) - *pos) * (dt * 16.0).min(1.0);
        let row = Rect::new(
            panel.x + 16.0 + i as f32 * (block + 14.0),
            top,
            block - 12.0,
            FOCUS_H - 2.0,
        );
        // How fast its kind builds, while a stall makes that worth saying.
        let speed = p.tier_speed[now.tier()].filter(|_| stalling && now != Priority::Even);
        let face = Face {
            kind: *kind,
            now,
            pos: *pos,
            short: short[i],
            speed,
        };
        if let Some(pick) = control(ui, row, i, &face) {
            // The one on already goes back to Even.
            let next = if pick == now { Priority::Even } else { pick };
            ui.audio.play(match next {
                Priority::First => Sfx::ToggleOn,
                Priority::Last => Sfx::ToggleOff,
                Priority::Even => Sfx::Tick,
            });
            let next = with(focus, i, next);
            hud.focus_ui.pending = Some((next, PENDING));
            hud.actions.push(HudAction::Focus(next));
        }
    }
}

/// What one kind's control shows.
struct Face {
    kind: Kind,
    now: Priority,
    /// Eased highlight position, in segments from Last.
    pos: f32,
    short: Short,
    /// Its tier's build speed, 0..1, when worth showing.
    speed: Option<f32>,
}

/// One kind's row: its mark and name, then Last / Even / First. The segment clicked.
fn control(ui: &mut Ui, row: Rect, i: usize, f: &Face) -> Option<Priority> {
    let t = ui.time;
    let mid = row.mid_y();
    let tone = f.kind.tone;
    // Short of its resource and not put first: First is the fix, and breathes to say so.
    let alarm = match f.short {
        Short::Dry => Some((palette::BAD, 0.5 + 0.5 * (t * 6.0).sin())),
        Short::Low => Some((LOW, 0.5 + 0.5 * (t * 3.5).sin())),
        Short::Fine => None,
    }
    .filter(|_| f.now != Priority::First);

    let mark = Vec2::new(row.x + 6.0, mid);
    let mark_ink = rgb(
        if f.now == Priority::First {
            tone
        } else {
            palette::DIM
        },
        1.0,
    );
    if i == 0 {
        ore(ui, mark, 5.5, mark_ink);
    } else {
        bolt(ui, mark, 6.0, mark_ink);
    }
    let end = ui.text(
        row.x + 17.0,
        mid,
        type_scale::CAPTION,
        rgb(palette::TEXT, 0.85),
        f.kind.name,
    );

    let seg = Rect::new(
        row.right() - SEG_W * 3.0,
        mid - SEG_H * 0.5,
        SEG_W * 3.0,
        SEG_H,
    );
    // What the row says between the name and the control.
    let note = match (f.speed, alarm) {
        (Some(s), _) => Some((
            format!("at {:.0}%", s * 100.0),
            if f.now == Priority::First {
                tone
            } else {
                palette::DIM
            },
            1.0,
        )),
        (None, Some((c, k))) if f.short == Short::Dry => {
            Some(("Stalling".to_owned(), c, 0.7 + 0.3 * k))
        }
        _ => None,
    };
    if let Some((text, c, a)) = note {
        if end + 8.0 + ui.text_width(type_scale::MICRO, &text) < seg.x - 8.0 {
            ui.text_right(seg.x - 8.0, mid, type_scale::MICRO, rgb(c, a), &text);
        }
    }

    let key = if i == 0 {
        "hud-focus-mines"
    } else {
        "hud-focus-power"
    };
    // Interaction first, so the body can brighten under the pointer before its marks.
    let res: [_; 3] =
        std::array::from_fn(|n| ui.interact(id(key, n), segmented::segment(seg, n, 3), true));
    segmented::body(ui, seg, res.iter().fold(0.0, |a, s| s.glow.max(a)));
    // The pill slides between segments and takes the colour of where it rests:
    // grey for Last, white for Even, the resource's own for First.
    let hi_ink = if f.pos < 1.0 {
        mix(rgb(palette::DIM, 1.0), rgb(palette::TEXT, 1.0), f.pos)
    } else {
        mix(rgb(palette::TEXT, 1.0), rgb(tone, 1.0), f.pos - 1.0)
    };
    segmented::pill(ui, seg, f.pos, 3, hi_ink, 1.0);

    if let Some((c, k)) = alarm {
        let first = segmented::inner_segment(seg, 2, 3);
        ui.gradient_h(first, rgb(c, 0.05), rgb(c, 0.12 + 0.18 * k));
        sheen(ui, first, t);
        ui.outline_cut(first, 2.0, rgb(c, 0.35 + 0.55 * k), rgb(c, 0.35 + 0.55 * k));
    }

    let mut picked = None;
    let mut hover = None;
    for (n, &(which, label)) in SEGMENTS.iter().enumerate() {
        let r = segmented::segment(seg, n, 3);
        let res = &res[n];
        segmented::hover(ui, seg, n, 3, res.glow);
        segmented::divider(ui, seg, n, 3, Some(f.pos));
        let on = which == f.now;
        let ink = match (on, which, alarm) {
            (true, ..) => rgb(0xFFFFFF, 1.0),
            (false, Priority::First, Some((c, k))) => rgb(c, 0.7 + 0.3 * k),
            (false, ..) => rgb(palette::FAINT, 0.9 + 0.1 * res.glow),
        };
        let ink = if !on && res.glow > 0.01 {
            mix(ink, rgb(palette::TEXT, 0.9), res.glow * 0.6)
        } else {
            ink
        };
        ui.text_centred(r.x + r.w * 0.5, mid, type_scale::MICRO, ink, label);
        if res.hovered {
            hover = Some(which);
        }
        if res.clicked {
            picked = Some(which);
        }
    }
    if let Some(which) = hover {
        let kind = f.kind.name.to_lowercase();
        let what = match which {
            Priority::First => format!("{} first: in a stall, new {kind}{} and their upgrades are paid in full before anything else", f.kind.name, f.kind.first_also),
            Priority::Even => format!("{} even: in a stall, new {kind} slow down with everything else", f.kind.name),
            Priority::Last => format!("{} last: new {kind} are built only out of what everything else leaves over", f.kind.name),
        };
        let act = if which == f.now && which != Priority::Even {
            "  \u{b7}  Click to set back to Even"
        } else {
            ""
        };
        tip(ui, row.x, row.bottom() + 8.0, &format!("{what}{act}"));
    }
    picked
}

/// A soft band of light that sweeps across `r` again and again.
fn sheen(ui: &mut Ui, r: Rect, t: f32) {
    let band = 30.0;
    let x = r.x - band + (t * 0.7).fract() * (r.w + band * 2.0);
    let (lo, hi) = (x.max(r.x), (x + band).min(r.right()));
    if hi <= lo {
        return;
    }
    let half = x + band * 0.5;
    if half > lo {
        ui.gradient_h(
            Rect::new(lo, r.y, half.min(hi) - lo, r.h),
            rgb(palette::TEXT, 0.0),
            rgb(palette::TEXT, 0.10),
        );
    }
    if hi > half {
        let from = half.max(lo);
        ui.gradient_h(
            Rect::new(from, r.y, hi - from, r.h),
            rgb(palette::TEXT, 0.10),
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
        1.2,
        color,
    );
}

/// A closed outline through `pts`, scaled by `s` about `c`.
fn outline(ui: &mut Ui, c: Vec2, s: f32, pts: &[Vec2], color: Color) {
    for (k, &a) in pts.iter().enumerate() {
        let b = pts[(k + 1) % pts.len()];
        ui.stroke(c + a * s, c + b * s, 1.4, color);
    }
}

fn mix(a: Color, b: Color, k: f32) -> Color {
    std::array::from_fn(|n| a[n] + (b[n] - a[n]) * k)
}
