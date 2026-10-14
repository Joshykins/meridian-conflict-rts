//! Immediate-mode widgets over the painter, behaving like the game's (mc-game
//! `ui`): a control eases its glow in on hover, sinks a little while held, and
//! fires on release over it.

use std::collections::HashMap;

use crate::paint::{ink, palette, rgb, type_scale, Painter, Rect, Style};

/// The pointer, in points.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Input {
    pub(crate) cursor: (f32, f32),
    pub(crate) down: bool,
    /// Pressed or released since the last frame.
    pub(crate) pressed: bool,
    pub(crate) released: bool,
    /// Lines scrolled since the last frame, down positive.
    pub(crate) scroll: f32,
}

impl Input {
    /// Forgets this frame's edges.
    pub(crate) fn end_frame(&mut self) {
        self.pressed = false;
        self.released = false;
        self.scroll = 0.0;
    }
}

/// What lasts from frame to frame: eased values, and which control the
/// pointer went down on.
#[derive(Default)]
pub(crate) struct Memory {
    anims: HashMap<u64, f32>,
    active: Option<u64>,
}

pub(crate) type Id = u64;

/// An id from a name and an index (a row of a list).
pub(crate) fn id(name: &str, i: usize) -> Id {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in name.bytes().chain(i.to_le_bytes()) {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Response {
    pub(crate) held: bool,
    pub(crate) clicked: bool,
    /// Hover, eased: 0..1.
    pub(crate) glow: f32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ButtonKind {
    Primary,
    Secondary,
}

pub(crate) struct Ui<'a> {
    pub(crate) p: &'a mut Painter,
    input: &'a Input,
    mem: &'a mut Memory,
    /// Seconds since the launcher opened, for things that move.
    pub(crate) time: f32,
    dt: f32,
    /// Something wants the next frame soon (an easing, a running job).
    pub(crate) busy: bool,
    /// The pointer is over something that takes a click.
    pub(crate) hand: bool,
}

impl<'a> Ui<'a> {
    pub(crate) fn new(
        p: &'a mut Painter,
        input: &'a Input,
        mem: &'a mut Memory,
        time: f32,
        dt: f32,
    ) -> Ui<'a> {
        Ui {
            p,
            input,
            mem,
            time,
            dt,
            busy: false,
            hand: false,
        }
    }

    pub(crate) fn size(&self) -> (f32, f32) {
        self.p.size()
    }

    pub(crate) fn scroll(&self) -> f32 {
        self.input.scroll
    }

    pub(crate) fn cursor(&self) -> (f32, f32) {
        self.input.cursor
    }

    /// Eases the value kept under `id` toward `target`, `rate` per second.
    pub(crate) fn ease(&mut self, id: Id, target: f32, rate: f32) -> f32 {
        let v = self.mem.anims.entry(id).or_insert(target);
        *v += (target - *v) * (1.0 - (-rate * self.dt).exp());
        if (*v - target).abs() < 0.002 {
            *v = target;
        } else {
            self.busy = true;
        }
        *v
    }

    /// Hover, press and click on `r`; a list's rows are clipped by its view.
    pub(crate) fn interact(&mut self, id: Id, r: Rect, enabled: bool) -> Response {
        let visible = self.p.clip_rect().map_or(r, |c| r.clip(c));
        let over = enabled && visible.contains(self.input.cursor);
        if over && self.input.pressed {
            self.mem.active = Some(id);
        }
        let held = self.mem.active == Some(id) && self.input.down;
        let clicked = self.mem.active == Some(id) && self.input.released && over;
        if self.input.released && self.mem.active == Some(id) {
            self.mem.active = None;
        }
        let glow = self.ease(id, if over { 1.0 } else { 0.0 }, 14.0);
        if over {
            self.hand = true;
        }
        Response {
            held,
            clicked,
            glow,
        }
    }

    /// The game's button (mc-game `Ui::button`): a primary one in red-orange
    /// glass with a sheen and marching chevrons, a secondary one in dark glass.
    pub(crate) fn button(
        &mut self,
        id: Id,
        r: Rect,
        label: &str,
        kind: ButtonKind,
        enabled: bool,
    ) -> bool {
        self.button_styled(id, r, label, kind, enabled, type_scale::BUTTON)
    }

    pub(crate) fn button_styled(
        &mut self,
        id: Id,
        r: Rect,
        label: &str,
        kind: ButtonKind,
        enabled: bool,
        st: Style,
    ) -> bool {
        let res = self.interact(id, r, enabled);
        let press = self.ease(id ^ 1, if res.held { 1.0 } else { 0.0 }, 30.0);
        let live = if enabled { 1.0 } else { 0.35 };
        let r = Rect::new(r.x, r.y + press * 1.5, r.w, r.h);
        let p = &mut *self.p;
        match kind {
            ButtonKind::Primary => {
                p.fill(
                    r,
                    rgb(palette::ACCENT_DEEP, (0.42 + 0.35 * res.glow) * live),
                );
                p.gradient_h(
                    r,
                    rgb(palette::ACCENT, (0.5 + 0.3 * res.glow) * live),
                    rgb(palette::ACCENT, 0.08 * live),
                );
                p.frame(r, rgb(palette::ACCENT, (0.65 + 0.35 * res.glow) * live));
                if res.glow > 0.01 {
                    let k = (self.time * 0.9).fract();
                    let x = r.x + (r.w + 120.0) * k - 120.0;
                    let (x0, x1) = (x.max(r.x), (x + 120.0).min(r.right()));
                    if x1 > x0 {
                        let a = 0.16 * res.glow;
                        let half = (x1 - x0) * 0.5;
                        p.gradient_h(
                            Rect::new(x0, r.y, half, r.h),
                            rgb(0xFFFFFF, 0.0),
                            rgb(0xFFFFFF, a),
                        );
                        p.gradient_h(
                            Rect::new(x0 + half, r.y, half, r.h),
                            rgb(0xFFFFFF, a),
                            rgb(0xFFFFFF, 0.0),
                        );
                    }
                    self.busy = true;
                }
            }
            ButtonKind::Secondary => {
                p.fill(r, ink(0.55));
                p.gradient_h(
                    r,
                    rgb(palette::ACCENT, 0.18 * res.glow),
                    rgb(palette::ACCENT, 0.0),
                );
                p.frame(r, rgb(palette::LINE, (0.22 + 0.5 * res.glow) * live));
            }
        }
        p.fill(
            Rect::new(r.x, r.y, 3.0, r.h),
            rgb(palette::ACCENT, (0.35 + 0.65 * res.glow) * live),
        );
        let tone = if kind == ButtonKind::Primary {
            rgb(0xFFFFFF, live)
        } else {
            rgb(palette::TEXT, (0.78 + 0.22 * res.glow) * live)
        };
        let w = p.text_width(st, label);
        let chevrons = if kind == ButtonKind::Primary {
            34.0
        } else {
            0.0
        };
        let x = r.x + (r.w - w - chevrons) * 0.5 + res.glow * 2.0;
        p.text(x, r.mid_y(), st, tone, label);
        if kind == ButtonKind::Primary {
            for i in 0..3 {
                let phase = ((self.time * 2.2 - i as f32 * 0.22).fract() + 1.0).fract();
                let a = (0.25 + 0.75 * (1.0 - phase) * res.glow.max(0.25)) * live;
                let cx = x + w + 14.0 + i as f32 * 9.0;
                let my = r.mid_y();
                p.polyline(
                    &[(cx, my - 5.0), (cx + 5.0, my), (cx, my + 5.0)],
                    1.6,
                    rgb(0xFFFFFF, a),
                );
            }
            if enabled {
                self.busy = true;
            }
        }
        res.clicked
    }

    /// A tab or a segment of a picker: text that lights when chosen, with a
    /// red-orange bar under the chosen one.
    pub(crate) fn tab(&mut self, id: Id, r: Rect, label: &str, chosen: bool) -> bool {
        let res = self.interact(id, r, true);
        let lit = self.ease(id ^ 3, if chosen { 1.0 } else { 0.0 }, 16.0);
        let p = &mut *self.p;
        p.fill(r, rgb(palette::LINE, 0.04 * res.glow + 0.05 * lit));
        p.fill(
            Rect::new(r.x, r.bottom() - 2.0, r.w, 2.0),
            rgb(palette::ACCENT, lit),
        );
        p.hline(r.x, r.bottom() - 1.0, r.w, rgb(palette::LINE, 0.12));
        let tone = rgb(palette::TEXT, 0.55 + 0.25 * res.glow + 0.45 * lit);
        p.text_centred(r.mid_x(), r.mid_y(), type_scale::VALUE, tone, label);
        res.clicked && !chosen
    }

    /// Text that takes a click: underlined on hover.
    pub(crate) fn link(&mut self, id: Id, x: f32, y: f32, st: Style, label: &str) -> bool {
        self.link_to(id, x, y, st, label, false)
    }

    /// A link that leaves the launcher (a page in the browser): an arrow out
    /// after it. Returns how wide it was and whether it was clicked.
    pub(crate) fn link_out(
        &mut self,
        id: Id,
        x: f32,
        y: f32,
        st: Style,
        label: &str,
    ) -> (f32, bool) {
        let w = self.p.text_width(st, label) + 14.0;
        (w, self.link_to(id, x, y, st, label, true))
    }

    fn link_to(&mut self, id: Id, x: f32, y: f32, st: Style, label: &str, out: bool) -> bool {
        let text_w = self.p.text_width(st, label);
        let w = text_w + if out { 14.0 } else { 0.0 };
        let r = Rect::new(x - 4.0, y - st.size * 0.7, w + 8.0, st.size * 1.4);
        let res = self.interact(id, r, true);
        let tone = rgb(palette::TEXT, 0.7 + 0.3 * res.glow);
        self.p.text(x, y, st, tone, label);
        if out {
            // A small arrow up and out to the right.
            let (ax, ay) = (x + text_w + 6.0, y + 3.0);
            let lit = rgb(palette::ACCENT, 0.6 + 0.4 * res.glow);
            self.p.stroke((ax, ay), (ax + 6.0, ay - 6.0), 1.2, lit);
            self.p.polyline(
                &[
                    (ax + 1.5, ay - 6.0),
                    (ax + 6.0, ay - 6.0),
                    (ax + 6.0, ay - 1.5),
                ],
                1.2,
                lit,
            );
        }
        self.p.hline(
            x,
            y + st.size * 0.55,
            text_w,
            rgb(palette::ACCENT, 0.25 + 0.75 * res.glow),
        );
        res.clicked
    }

    /// A thin bar filled to `k` (0..1); `None` sweeps, for work of unknown length.
    pub(crate) fn progress(&mut self, r: Rect, k: Option<f32>) {
        let p = &mut *self.p;
        p.fill(r, rgb(palette::LINE, 0.10));
        match k {
            Some(k) => {
                let w = r.w * k.clamp(0.0, 1.0);
                p.gradient_h(
                    Rect::new(r.x, r.y, w, r.h),
                    rgb(palette::ACCENT_DEEP, 1.0),
                    rgb(palette::ACCENT, 1.0),
                );
                if w > 2.0 {
                    p.fill(
                        Rect::new(r.x + w - 2.0, r.y - 2.0, 2.0, r.h + 4.0),
                        rgb(0xFFFFFF, 0.9),
                    );
                }
            }
            None => {
                let t = (self.time * 0.7).fract();
                let x = r.x + (r.w + 120.0) * t - 120.0;
                let (x0, x1) = (x.max(r.x), (x + 120.0).min(r.right()));
                if x1 > x0 {
                    p.gradient_h(
                        Rect::new(x0, r.y, x1 - x0, r.h),
                        rgb(palette::ACCENT, 0.0),
                        rgb(palette::ACCENT, 0.9),
                    );
                }
                self.busy = true;
            }
        }
    }
}
