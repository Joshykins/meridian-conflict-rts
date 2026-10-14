//! Sliders for wide settings rows and compact authoring cards.
use super::*;
impl Ui<'_> {
    /// A labelled row with a 0..1 slider. Returns true while the value changes.
    pub fn slider(&mut self, id: Id, r: Rect, label: &str, value: &mut f32) -> bool {
        let res = self.interact(id, r, true);
        self.row(r, label, "", res.glow);
        let track = Rect::new(r.right() - 300.0, r.mid_y() - 2.0, 220.0, 4.0);
        let mut changed = false;
        if res.held {
            let v = ((self.cursor.x - self.shift.x - track.x) / track.w).clamp(0.0, 1.0);
            // Snap to 5% steps; each step ticks, so dragging sounds like a detent wheel.
            let v = (v * 20.0).round() / 20.0;
            if (v - *value).abs() > 1e-4 {
                *value = v;
                changed = true;
                self.audio.play(Sfx::Tick);
            }
        }
        self.fill(track, rgb(palette::LINE, 0.16));
        self.fill(
            Rect::new(track.x, track.y, track.w * *value, track.h),
            rgb(palette::ACCENT, 0.9),
        );
        for i in 0..=10 {
            let x = track.x + track.w * i as f32 / 10.0;
            self.vline(
                x,
                track.bottom() + 5.0,
                if i % 5 == 0 { 6.0 } else { 3.0 },
                rgb(palette::LINE, 0.3),
            );
        }
        let grow = 1.5 * res.glow + if res.held { 1.5 } else { 0.0 };
        let knob = Rect::new(
            track.x + track.w * *value - 4.0 - grow * 0.5,
            r.mid_y() - 9.0 - grow * 0.5,
            8.0 + grow,
            18.0 + grow,
        );
        self.fill(knob, rgb(0xFFFFFF, 1.0));
        self.text_right(
            r.right() - 16.0,
            r.mid_y(),
            type_scale::VALUE,
            rgb(palette::TEXT, 1.0),
            &format!("{:.0}", *value * 100.0),
        );
        changed
    }

    /// A compact slider with its value-bearing label above the track.
    pub fn slider_compact(&mut self, id: Id, r: Rect, label: &str, value: &mut f32) -> bool {
        let response = self.interact(id, r, true);
        self.text_fit_left(
            r.x,
            r.y + 7.0,
            r.w,
            type_scale::MICRO,
            rgb(palette::TEXT, 1.0),
            label,
        );
        let track = Rect::new(r.x + 4.0, r.y + 20.0, r.w - 8.0, 3.0);
        let mut changed = false;
        if response.held {
            let next = ((self.cursor.x - self.shift.x - track.x) / track.w).clamp(0.0, 1.0);
            if (next - *value).abs() > 0.0001 {
                *value = next;
                changed = true;
            }
        }
        self.fill(track, rgb(palette::LINE, 0.35));
        self.fill(
            Rect::new(track.x, track.y, track.w * *value, track.h),
            rgb(palette::ACCENT, 0.9),
        );
        self.fill(
            Rect::new(track.x + track.w * *value - 3.0, track.y - 4.0, 6.0, 11.0),
            rgb(palette::TEXT, 1.0),
        );
        changed
    }
}
