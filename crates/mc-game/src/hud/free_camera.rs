//! Free camera (Ctrl+Alt): the panels fold away to the edges they sit on and a
//! guide to the camera's keys takes their place. Ctrl+Alt again, or Esc once
//! nothing is locked on, brings them back. The flight itself is `cine.rs`.
//!
//! The guide opens full for a few seconds, then folds into a pill at the foot of
//! the screen; the pointer on the pill, or H, opens it again. Left alone, the
//! pill fades out too, and the pointer hides, so the frame is clean for a
//! picture or a recording. Its key caps light while their keys are held, so it
//! reads as a live instrument rather than a help page.

use super::Hud;
use crate::audio::Sfx;
use crate::ui::{id, ink, palette, rgb, style, type_scale, Rect, Ui};
use glam::Vec2;
use mc_render::Face;

/// Camera keys held, and modes on, this frame (`FreeCamera::held`).
pub mod held {
    pub const FLY: u32 = 1;
    pub const RISE: u32 = 1 << 1;
    pub const FAST: u32 = 1 << 2;
    pub const DOLLY: u32 = 1 << 3;
    pub const MOUSELOOK: u32 = 1 << 4;
    pub const ORBIT: u32 = 1 << 5;
    pub const DRAG: u32 = 1 << 6;
    pub const LENS: u32 = 1 << 7;
    pub const FRAME: u32 = 1 << 8;
    pub const FOLLOW: u32 = 1 << 9;
    pub const AIM: u32 = 1 << 10;
    pub const LOCK: u32 = 1 << 11;
    pub const PLAY: u32 = 1 << 12;
    pub const GRID: u32 = 1 << 13;
    pub const RESET: u32 = 1 << 14;
    pub const SMOOTH: u32 = 1 << 15;
    pub const CTRL: u32 = 1 << 16;
    pub const BARS: u32 = 1 << 17;
}

/// How long the whole fold takes, stagger included, in seconds.
const FOLD_SECONDS: f32 = 0.42;
/// How long one region takes to leave, as a share of the fold.
const PART_SHARE: f32 = 0.68;
/// How far a region slides as it goes, in points.
const SLIDE: f32 = 84.0;
/// The guide stays open this long after the camera is freed.
const GUIDE_OPEN_SECONDS: f32 = 5.0;
/// The pill stays this long after something changed, then fades from the frame.
const PILL_LINGER: f32 = 2.4;

const HEAD_H: f32 = 42.0;
const FULL_W: f32 = 900.0;
const ROWS: usize = 6;
const ROW: f32 = 25.0;
const FOOT_H: f32 = 38.0;
const FULL_H: f32 = HEAD_H + 40.0 + ROW * ROWS as f32 + 8.0 + FOOT_H;
const CAP_H: f32 = 18.0;
/// Cinema bars: 2.39:1.
const SCOPE: f32 = 2.39;

/// A region of the HUD that folds away together, toward the edge it sits on.
#[derive(Clone, Copy)]
pub enum Part {
    /// Economy, commander card, range panel: out to the left.
    Left = 0,
    /// The mission clock and speed control: up.
    Top = 1,
    /// Minimap, profiler, survival's card: out to the right.
    Right = 2,
    /// The deck and its chips: down.
    Deck = 3,
}

impl Part {
    fn dir(self) -> Vec2 {
        match self {
            Part::Left => Vec2::new(-1.0, 0.0),
            Part::Top => Vec2::new(0.0, -1.0),
            Part::Right => Vec2::new(1.0, 0.0),
            Part::Deck => Vec2::new(0.0, 1.0),
        }
    }

    /// Where in the fold this region starts to leave: the deck goes first, as the
    /// biggest thing on screen, and the clock last. Coming back runs it backwards.
    fn delay(self) -> f32 {
        let order = match self {
            Part::Deck => 0.0,
            Part::Right => 1.0,
            Part::Left => 2.0,
            Part::Top => 3.0,
        };
        order / 3.0 * (1.0 - PART_SHARE)
    }
}

/// What the guide reports of the camera, filled by the game each frame.
#[derive(Clone)]
pub struct Status {
    pub speed: f32,
    pub focal: f32,
    pub smoothing: &'static str,
    pub locked: bool,
    pub grid: bool,
    pub following: bool,
    /// The name of what is locked on.
    pub aim: Option<String>,
    /// Where it is on screen (window pixels) and how big.
    pub aim_at: Option<(Vec2, f32)>,
    pub playing: Option<usize>,
    pub at_shot: Option<usize>,
    pub shots: [bool; 9],
    pub game_speed: u32,
    pub paused: bool,
    /// The pointer moved lately: the guide and reticle may show.
    pub pointer_live: bool,
}

impl Default for Status {
    fn default() -> Status {
        Status {
            speed: 1.0,
            focal: crate::cine::focal_mm(mc_render::camera::FOV_Y),
            smoothing: crate::cine::Easing::default().label(),
            locked: false,
            grid: false,
            following: false,
            aim: None,
            aim_at: None,
            playing: None,
            at_shot: None,
            shots: [false; 9],
            game_speed: 100,
            paused: false,
            pointer_live: true,
        }
    }
}

#[derive(Default)]
pub struct FreeCamera {
    pub on: bool,
    /// Seconds since the camera was freed.
    pub age: f32,
    /// Camera keys held this frame, `held` bits.
    pub held: u32,
    /// H: the guide stays open.
    pub pinned: bool,
    /// B: cinema bars over the picture.
    pub bars: bool,
    pub status: Status,
    /// The pointer was on the guide last frame.
    hovered: bool,
    /// 0 with every panel in place, 1 with all of them folded away.
    fold: f32,
    /// `age` when something last changed, so the pill shows for a moment.
    touched: f32,
    /// A shot just saved, and when: its slot flashes.
    flashed: Option<(usize, f32)>,
}

impl FreeCamera {
    /// Frees the camera or gives the panels back. Returns whether it is now free.
    pub fn toggle(&mut self, ui_sound: &crate::audio::Audio) -> bool {
        self.set(!self.on);
        ui_sound.play(if self.on {
            Sfx::ToggleOn
        } else {
            Sfx::ToggleOff
        });
        self.on
    }

    pub fn set(&mut self, on: bool) {
        if on && !self.on {
            self.age = 0.0;
            self.touched = 0.0;
        }
        self.on = on;
    }

    /// H: opens the guide and keeps it open, or puts it away.
    pub fn toggle_guide(&mut self) {
        if self.pinned || self.age < GUIDE_OPEN_SECONDS {
            self.pinned = false;
            self.age = self.age.max(GUIDE_OPEN_SECONDS);
        } else {
            self.pinned = true;
        }
        self.poke();
    }

    /// Something changed: the pill shows for a moment.
    pub fn poke(&mut self) {
        self.touched = self.age;
    }

    /// The camera is being flown: keeps the pill only if it is already showing.
    pub fn poke_quiet(&mut self) {
        if self.age - self.touched < PILL_LINGER {
            self.touched = self.age;
        }
    }

    pub fn flash(&mut self, slot: usize) {
        self.flashed = Some((slot, self.age));
    }

    /// Folds everything away at once, for a still picture.
    pub fn snap(&mut self, pinned: bool) {
        self.on = true;
        self.fold = 1.0;
        self.pinned = pinned;
        self.age = if pinned {
            0.0
        } else {
            GUIDE_OPEN_SECONDS + 1.0
        };
        self.touched = self.age;
    }

    pub fn advance(&mut self, dt: f32) {
        let step = dt / FOLD_SECONDS;
        self.fold = (self.fold + if self.on { step } else { -step }).clamp(0.0, 1.0);
        if self.on {
            self.age += dt;
        }
    }

    /// How far `part` has folded away, eased: 0 in place, 1 gone.
    pub fn part(&self, part: Part) -> f32 {
        ease_in_out(self.progress(part))
    }

    /// 0 when `part` is in place, 1 when it is gone.
    fn progress(&self, part: Part) -> f32 {
        ((self.fold - part.delay()) / PART_SHARE).clamp(0.0, 1.0)
    }
}

fn ease_in_out(t: f32) -> f32 {
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) * 0.5
    }
}

fn smoothstep(lo: f32, hi: f32, x: f32) -> f32 {
    let t = ((x - lo) / (hi - lo)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// What `fold_begin` changed, for `fold_end` to put back.
pub struct Folded {
    fade: f32,
    shift: Vec2,
    interactive: bool,
    unclaimed: bool,
}

type Row = (&'static [&'static str], &'static str, u32);

const COLUMNS: [(&str, &[Row]); 4] = {
    use held::*;
    [
        (
            "Fly",
            &[
                (&["W", "A", "S", "D"], "Fly where you look", FLY),
                (&["E", "Q"], "Rise, sink", RISE),
                (&["Shift"], "Faster", FAST),
                (&["Wheel"], "Dolly in, out", DOLLY),
                (&["Right", "Wheel"], "Flight speed", 0),
                (&["R"], "Reset lens, speed", RESET),
            ],
        ),
        (
            "Look",
            &[
                (&["Right drag"], "Look around", MOUSELOOK),
                (&["Alt"], "Orbit the aim", ORBIT),
                (&["Middle drag"], "Slide", DRAG),
                (&["Z", "X"], "Lens long, wide", LENS),
                (&["G"], "Thirds grid", GRID),
                (&["B"], "Cinema bars", BARS),
            ],
        ),
        (
            "Aim",
            &[
                (&["Click"], "Lock on", AIM),
                (&["F"], "Frame it", FRAME),
                (&["T"], "Follow", FOLLOW),
                (&["Esc"], "Let go", 0),
                (&["Home"], "Commander", 0),
            ],
        ),
        (
            "Shots",
            &[
                (&["Ctrl", "1-9"], "Save shot", CTRL),
                (&["1-9"], "Glide to shot", 0),
                (&["Shift", "1-9"], "Cut to shot", 0),
                (&["P"], "Play shots", PLAY),
                (&["L"], "Lock camera", LOCK),
                (&["N"], "Smoothing", SMOOTH),
            ],
        ),
    ]
};

impl Hud {
    /// Starts drawing `part`: it slides toward its edge and fades as the fold runs,
    /// and stops taking the pointer as soon as the camera is freed.
    pub(super) fn fold_begin(&mut self, ui: &mut Ui, part: Part) -> Folded {
        let saved = Folded {
            fade: ui.fade,
            shift: ui.shift,
            interactive: ui.interactive,
            unclaimed: self.unclaimed,
        };
        let p = self.free.progress(part);
        if p > 0.0 {
            ui.shift += part.dir() * SLIDE * ease_in_out(p);
            // The glass is gone a little before the slide ends, so nothing parks at the edge.
            ui.fade *= 1.0 - smoothstep(0.0, 0.8, p);
        }
        if self.free.on {
            ui.interactive = false;
            self.unclaimed = true;
        }
        saved
    }

    pub(super) fn fold_end(&mut self, ui: &mut Ui, saved: Folded) {
        ui.fade = saved.fade;
        ui.shift = saved.shift;
        ui.interactive = saved.interactive;
        self.unclaimed = saved.unclaimed;
    }

    /// Everything the free camera draws: bars and grid over the picture, the
    /// lock-on reticle, the corner marks as it opens, and the guide.
    pub(super) fn free_camera_guide(&mut self, ui: &mut Ui, dt: f32) {
        self.free.advance(dt);
        let shown = ui.ease(
            id("free-cam-shown", 0),
            if self.free.on { 1.0 } else { 0.0 },
            11.0,
        );
        if shown < 0.005 {
            return;
        }
        let (w, h) = (ui.size.x, ui.size.y);
        let (fade, shift) = (ui.fade, ui.shift);
        ui.fade *= shown;
        self.picture_aids(ui);
        ui.shift.y += (1.0 - shown) * 28.0;
        if self.free.on {
            self.viewfinder(ui);
        }

        // Open: the first seconds, pinned with H, or with the pointer on it.
        let open_goal = self.free.on
            && (self.free.pinned || self.free.age < GUIDE_OPEN_SECONDS || self.free.hovered);
        let open = ui.ease(
            id("free-cam-open", 0),
            if open_goal { 1.0 } else { 0.0 },
            9.0,
        );
        let e = ease_in_out(open);
        let pill_w = self.pill_width(ui);
        let cw = pill_w + (FULL_W.min(w - 40.0) - pill_w) * e;
        let ch = HEAD_H + (FULL_H - HEAD_H) * e;
        let bottom = h - 24.0;
        let r = Rect::new((w - cw) * 0.5, bottom - ch, cw, ch);

        // Left alone, the pill leaves the frame; the pointer coming near, or a
        // change of mode, brings it back.
        let near = {
            let d = (ui.cursor - Vec2::new(w * 0.5, bottom - ch * 0.5)).abs();
            let dx = (d.x - cw * 0.5).max(0.0) / 260.0;
            let dy = (d.y - ch * 0.5).max(0.0) / 200.0;
            1.0 - smoothstep(0.0, 1.0, dx.max(dy))
        };
        let recent = 1.0
            - smoothstep(
                PILL_LINGER,
                PILL_LINGER + 0.6,
                self.free.age - self.free.touched,
            );
        let live = if self.free.status.pointer_live {
            near
        } else {
            0.0
        };
        let presence_goal = open.max(recent).max(live);
        let presence = ui.ease(id("free-cam-presence", 0), presence_goal, 8.0);
        let over = r.contains(ui.cursor - ui.shift) && self.free.on && presence > 0.3;
        self.free.hovered = over;
        self.aim_reticle(ui, presence);
        if presence < 0.01 {
            (ui.fade, ui.shift) = (fade, shift);
            return;
        }
        ui.fade *= presence;
        ui.shift.y += (1.0 - presence) * 10.0;
        let hover = ui.ease(id("free-cam-hover", 0), if over { 1.0 } else { 0.0 }, 14.0);

        self.claim(ui, r);
        if over && ui.input.pressed && ui.interactive {
            self.free.pinned = !self.free.pinned;
            ui.audio.play(if self.free.pinned {
                Sfx::ToggleOn
            } else {
                Sfx::ToggleOff
            });
        }

        ui.frost_cut(r, 8.0, 0.86);
        ui.bevel(r, 8.0, 0.7 + 0.3 * hover.max(open));
        // The live line along the foot, red-orange: the camera is the player's.
        let bar = (r.w - 16.0) * (0.3 + 0.7 * e);
        ui.fill(
            Rect::new(r.x + (r.w - bar) * 0.5, r.bottom() - 2.0, bar, 2.0),
            rgb(palette::ACCENT, 0.85),
        );
        self.guide_head(ui, Rect::new(r.x, r.y, r.w, HEAD_H), e);
        if open > 0.55 {
            let k = smoothstep(0.55, 1.0, open);
            let f = ui.fade;
            ui.fade *= k;
            ui.shift.y += (1.0 - k) * 6.0;
            self.guide_body(ui, Rect::new(r.x, r.y + HEAD_H, r.w, r.h - HEAD_H - FOOT_H));
            self.guide_foot(ui, Rect::new(r.x, r.bottom() - FOOT_H, r.w, FOOT_H));
            ui.shift.y -= (1.0 - k) * 6.0;
            ui.fade = f;
        }
        (ui.fade, ui.shift) = (fade, shift);
    }

    fn bar_height(&self, ui: &mut Ui) -> f32 {
        let k = ui.ease(
            id("free-cam-bars", 0),
            if self.free.bars && self.free.on {
                1.0
            } else {
                0.0
            },
            7.0,
        );
        let (w, h) = (ui.size.x, ui.size.y);
        ((h - w / SCOPE) * 0.5).max(0.0) * ease_in_out(k)
    }

    /// Cinema bars and the thirds grid: part of the picture, so they stay when
    /// the guide fades.
    fn picture_aids(&mut self, ui: &mut Ui) {
        let (w, h) = (ui.size.x, ui.size.y);
        let bars = self.bar_height(ui);
        if bars > 0.2 {
            ui.fill(Rect::new(0.0, 0.0, w, bars), rgb(palette::INK, 1.0));
            ui.fill(Rect::new(0.0, h - bars, w, bars), rgb(palette::INK, 1.0));
        }
        let grid = ui.ease(
            id("free-cam-grid", 0),
            if self.free.status.grid && self.free.on {
                1.0
            } else {
                0.0
            },
            10.0,
        );
        if grid > 0.01 {
            let (top, tall) = (bars, h - bars * 2.0);
            let line = rgb(palette::LINE, 0.26 * grid);
            let t = 1.0 / ui.s;
            for i in 1..3 {
                let k = i as f32 / 3.0;
                ui.vline(w * k, top, tall, line);
                ui.hline(0.0, top + tall * k, w, line);
            }
            // The centre, faintly.
            let c = Vec2::new(w * 0.5, top + tall * 0.5);
            ui.fill(
                Rect::new(c.x - 6.0, c.y - t * 0.5, 12.0, t),
                rgb(palette::LINE, 0.35 * grid),
            );
            ui.fill(
                Rect::new(c.x - t * 0.5, c.y - 6.0, t, 12.0),
                rgb(palette::LINE, 0.35 * grid),
            );
        }
    }

    /// Corner marks round what is locked on, with its name; they show with the guide.
    fn aim_reticle(&mut self, ui: &mut Ui, presence: f32) {
        let lock = ui.ease(
            id("free-cam-aim", 0),
            if self.free.status.aim_at.is_some() {
                1.0
            } else {
                0.0
            },
            12.0,
        );
        let Some((px, size)) = self.free.status.aim_at else {
            return;
        };
        let k = lock * presence;
        if k < 0.01 {
            return;
        }
        let c = px / ui.s;
        let half = (size / ui.s).clamp(14.0, 160.0) * (1.0 + 0.5 * (1.0 - lock));
        let r = Rect::new(c.x - half, c.y - half, half * 2.0, half * 2.0);
        ui.brackets(
            r,
            (half * 0.4).clamp(6.0, 20.0),
            rgb(palette::ACCENT, 0.9 * k),
        );
        if let Some(name) = &self.free.status.aim {
            let label = if self.free.status.following {
                format!("{name}  \u{b7}  following")
            } else {
                name.clone()
            };
            ui.text_centred(
                c.x,
                r.bottom() + 12.0,
                type_scale::MICRO,
                rgb(palette::TEXT, 0.85 * k),
                &label,
            );
        }
    }

    /// The pill: its mark and name, what the camera is doing, the guide key and the way back.
    fn pill_width(&self, ui: &mut Ui) -> f32 {
        let name = ui.text_width(type_scale::BUTTON, "Free camera");
        let state = self.state_line();
        let state_w = if state.is_empty() {
            0.0
        } else {
            ui.text_width(type_scale::CAPTION, &state) + 28.0
        };
        let keys = ui.text_width(type_scale::CAPTION, "Keys pinned");
        let back = ui.text_width(type_scale::CAPTION, "Show interface");
        let caps = cap_w(ui, "H") + cap_w(ui, "Ctrl") + cap_w(ui, "Alt") + 12.0;
        20.0 + 22.0 + name + state_w + 28.0 + keys + 8.0 + caps + 8.0 + back + 28.0 + 18.0
    }

    /// What the camera is doing, in a few words, for the head.
    fn state_line(&self) -> String {
        let s = &self.free.status;
        let mut parts: Vec<String> = Vec::new();
        if let Some(n) = s.playing {
            parts.push(format!("Playing shot {}", n + 1));
        }
        if s.locked {
            parts.push("Camera locked".into());
        }
        if let Some(name) = &s.aim {
            parts.push(if s.following {
                format!("Following {name}")
            } else {
                format!("On {name}")
            });
        }
        parts.join("  \u{b7}  ")
    }

    fn guide_head(&mut self, ui: &mut Ui, r: Rect, open: f32) {
        let held = self.free.held;
        let y = r.y + HEAD_H * 0.5;
        // The mark: a small viewfinder with the live point in it.
        let m = Rect::new(r.x + 18.0, y - 7.0, 18.0, 14.0);
        ui.brackets(m, 5.0, rgb(palette::TEXT, 0.9));
        let beat = 0.55 + 0.45 * (ui.time * 2.6).sin().abs();
        ui.disc(
            Vec2::new(m.x + m.w * 0.5, y),
            2.4,
            rgb(palette::ACCENT, beat),
        );
        let mut x = ui.text(
            m.right() + 11.0,
            y,
            type_scale::BUTTON,
            rgb(palette::TEXT, 1.0),
            "Free camera",
        );
        let state = self.state_line();
        if !state.is_empty() {
            x += 14.0;
            ui.vline(x, r.y + 12.0, HEAD_H - 24.0, rgb(palette::LINE, 0.16));
            x += 14.0;
            x = ui.text(x, y, type_scale::CAPTION, rgb(palette::ACCENT, 1.0), &state);
        }

        // From the right: Ctrl Alt, Show interface.
        let back = ui.text_width(type_scale::CAPTION, "Show interface");
        let mut rx = r.right() - 18.0 - back;
        ui.text(
            rx,
            y,
            type_scale::CAPTION,
            rgb(palette::DIM, 1.0),
            "Show interface",
        );
        rx -= 9.0;
        rx -= cap_w(ui, "Alt");
        let chord = held & held::CTRL != 0 && held & held::ORBIT != 0;
        cap(ui, rx, y, "Alt", chord);
        rx -= 4.0 + cap_w(ui, "Ctrl");
        cap(ui, rx, y, "Ctrl", held & held::CTRL != 0);
        ui.vline(
            rx - 14.0,
            r.y + 12.0,
            HEAD_H - 24.0,
            rgb(palette::LINE, 0.16),
        );

        // H opens and pins the keys, left of that.
        let pinned = self.free.pinned;
        let label = if pinned { "Keys pinned" } else { "Keys" };
        let lw = ui.text_width(type_scale::CAPTION, label);
        let hx = rx - 28.0 - lw - 8.0 - cap_w(ui, "H");
        if hx > x + 12.0 {
            let end = hx + cap(ui, hx, y, "H", pinned) + 8.0;
            ui.text(
                end,
                y,
                type_scale::CAPTION,
                rgb(if pinned { palette::TEXT } else { palette::DIM }, 1.0),
                label,
            );
        }
        // A hairline under the head once the body is out.
        if open > 0.01 {
            ui.gradient_h(
                Rect::new(r.x + 16.0, r.bottom(), (r.w - 32.0) * 0.5, 1.0 / ui.s),
                rgb(palette::LINE, 0.0),
                rgb(palette::LINE, 0.18 * open),
            );
            ui.gradient_h(
                Rect::new(r.x + r.w * 0.5, r.bottom(), (r.w - 32.0) * 0.5, 1.0 / ui.s),
                rgb(palette::LINE, 0.18 * open),
                rgb(palette::LINE, 0.0),
            );
        }
    }

    fn guide_body(&mut self, ui: &mut Ui, r: Rect) {
        let pad = 22.0;
        let col_w = (r.w - pad * 2.0) / COLUMNS.len() as f32;
        let held = self.free.held | if self.free.bars { held::BARS } else { 0 };
        for (c, (title, rows)) in COLUMNS.iter().enumerate() {
            let x = r.x + pad + col_w * c as f32;
            ui.section(x, r.y + 22.0, col_w - 18.0, title);
            for (i, (keys, label, bit)) in rows.iter().enumerate() {
                let y = r.y + 48.0 + ROW * i as f32;
                let live = *bit != 0 && held & bit != 0;
                // The caps light, and the row with them.
                let glow = ui.ease(
                    id("free-cam-row", c * 8 + i),
                    if live { 1.0 } else { 0.0 },
                    18.0,
                );
                if glow > 0.01 {
                    ui.gradient_h(
                        Rect::new(x - 6.0, y - ROW * 0.5 + 2.0, col_w - 14.0, ROW - 4.0),
                        rgb(palette::ACCENT, 0.16 * glow),
                        rgb(palette::ACCENT, 0.0),
                    );
                    ui.fill(
                        Rect::new(x - 6.0, y - ROW * 0.5 + 2.0, 2.0, ROW - 4.0),
                        rgb(palette::ACCENT, glow),
                    );
                }
                let mut cx = x + 4.0;
                for k in keys.iter() {
                    cx += cap(ui, cx, y, k, live) + 3.0;
                }
                ui.text_fit_left(
                    cx + 6.0,
                    y,
                    (x + col_w - 16.0 - cx - 6.0).max(10.0),
                    type_scale::CAPTION,
                    rgb(palette::TEXT, 0.72 + 0.28 * glow),
                    label,
                );
            }
        }
    }

    /// The shots saved, and the camera's settings.
    fn guide_foot(&mut self, ui: &mut Ui, r: Rect) {
        let s = self.free.status.clone();
        ui.hline(r.x + 16.0, r.y, r.w - 32.0, rgb(palette::LINE, 0.1));
        let y = r.mid_y();
        let mut x = r.x + 22.0;
        x = ui.text(x, y, type_scale::MICRO, rgb(palette::FAINT, 1.0), "Shots") + 10.0;
        for i in 0..9 {
            let b = Rect::new(x, y - 9.0, 18.0, 18.0);
            let current = s.at_shot == Some(i);
            let playing = s.playing == Some(i);
            let flash = self
                .free
                .flashed
                .filter(|(slot, _)| *slot == i)
                .map_or(0.0, |(_, at)| {
                    1.0 - smoothstep(0.0, 0.8, self.free.age - at)
                });
            if s.shots[i] {
                let pulse = if playing {
                    0.5 + 0.5 * (ui.time * 5.0).sin().abs()
                } else {
                    1.0
                };
                let tone = if current {
                    palette::ACCENT
                } else {
                    palette::LINE
                };
                ui.fill_cut(
                    b,
                    3.0,
                    rgb(
                        tone,
                        (if current { 0.85 } else { 0.22 }) * pulse + 0.6 * flash,
                    ),
                );
                ui.text_centred(
                    b.x + 9.0,
                    y + 0.5,
                    CAP_STYLE,
                    rgb(if current { palette::INK } else { palette::TEXT }, 1.0),
                    &(i + 1).to_string(),
                );
            } else {
                ui.outline_cut(b, 3.0, rgb(palette::LINE, 0.14), rgb(palette::LINE, 0.24));
                ui.text_centred(
                    b.x + 9.0,
                    y + 0.5,
                    CAP_STYLE,
                    rgb(palette::FAINT, 0.8),
                    &(i + 1).to_string(),
                );
            }
            x += 22.0;
        }
        // Readouts from the right.
        let speed = format!("{:.2}", s.speed);
        let speed = format!(
            "\u{d7}{}",
            speed.trim_end_matches('0').trim_end_matches('.')
        );
        let readouts = [
            (
                "Time",
                if s.paused {
                    "Paused".to_string()
                } else {
                    format!("{}%", s.game_speed)
                },
            ),
            ("Smoothing", s.smoothing.to_string()),
            ("Lens", format!("{:.0} mm", s.focal)),
            ("Speed", speed),
        ];
        let mut rx = r.right() - 22.0;
        for (label, value) in readouts {
            let vw = ui.text_width(type_scale::VALUE, &value);
            ui.text(
                rx - vw,
                y,
                type_scale::VALUE,
                rgb(palette::TEXT, 1.0),
                &value,
            );
            let lw = ui.text_width(type_scale::MICRO, label);
            ui.text(
                rx - vw - 8.0 - lw,
                y,
                type_scale::MICRO,
                rgb(palette::FAINT, 1.0),
                label,
            );
            rx -= vw + 8.0 + lw + 24.0;
        }
    }

    /// Corner marks that close in from the screen's edge when the camera is freed,
    /// then fade: the frame has become the player's.
    fn viewfinder(&mut self, ui: &mut Ui) {
        let t = self.free.age;
        if t > 1.6 {
            return;
        }
        let come = ease_in_out((t / 0.45).min(1.0));
        let alpha = (1.0 - smoothstep(0.7, 1.6, t)) * come;
        let inset = 64.0 - 36.0 * come;
        let r = Rect::new(
            inset,
            inset,
            ui.size.x - inset * 2.0,
            ui.size.y - inset * 2.0,
        );
        ui.brackets(r, 44.0, rgb(palette::TEXT, 0.55 * alpha));
        let tick = rgb(palette::ACCENT, 0.9 * alpha);
        ui.fill(Rect::new(r.x, r.y, 12.0, 2.0), tick);
        ui.fill(
            Rect::new(r.right() - 12.0, r.bottom() - 2.0, 12.0, 2.0),
            tick,
        );
    }
}

const CAP_STYLE: crate::ui::Style = style(Face::Bold, 11.0, 0.3);

fn cap_w(ui: &mut Ui, key: &str) -> f32 {
    (ui.text_width(CAP_STYLE, key) + 11.0).max(CAP_H)
}

/// A key cap sized to its key, centred on `y`; filled white while held. Returns its width.
fn cap(ui: &mut Ui, x: f32, y: f32, key: &str, live: bool) -> f32 {
    let w = cap_w(ui, key);
    let r = Rect::new(x, y - CAP_H * 0.5, w, CAP_H);
    if live {
        ui.fill_cut(r, 3.0, rgb(0xFFFFFF, 0.92));
    } else {
        ui.fill_cut(r, 3.0, ink(0.5));
        ui.outline_cut(r, 3.0, rgb(palette::LINE, 0.34), rgb(palette::LINE, 0.6));
    }
    ui.text_centred(
        r.x + w * 0.5,
        y + 0.5,
        CAP_STYLE,
        rgb(
            if live { palette::INK } else { palette::TEXT },
            if live { 1.0 } else { 0.9 },
        ),
        key,
    );
    w
}
