//! A factory's Batch control on the queue strip. Off, it is a switch like Repeat. On, a
//! size stepper grows out of its left side, joined to it, and while units wait, a Send
//! button beside that. The switch never moves: it is the rightmost part, and what grows
//! pushes only the queued tiles.

use super::queue::{switch, Layout, Queue, Switch};
use super::tip;
use crate::audio::Sfx;
use crate::hud::icons;
use crate::hud::{Hud, HudAction, Scene};
use crate::ui::{id, palette, rgb, type_scale, Rect, Ui};
use glam::Vec2;
use mc_sim::batch::MAX_BATCH;

/// The colour of a batch: a way of moving out.
const BATCH: u32 = crate::hud::style::Family::Movement.tone();

/// A factory's batch as the strip shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Muster {
    /// Units waiting in the whole batch, and how many it leaves at.
    pub(super) count: u16,
    pub(super) size: u16,
    /// Factories linked in it.
    pub(super) linked: usize,
}

/// How many of `factories` (unit ids) are in the batch most of them share, and how many
/// there are: what the Batch switch and Shift+L go by. All in one batch is all on.
pub(crate) fn batch_linked(
    queues: &[mc_sim::mirror::UnitOrders],
    factories: impl IntoIterator<Item = u32>,
) -> (usize, usize) {
    let groups: Vec<Option<u32>> = factories
        .into_iter()
        .map(|id| {
            crate::sim_thread::queue_of(queues, id)
                .and_then(|q| q.batch.as_ref())
                .map(|b| b.group)
        })
        .collect();
    let most = groups
        .iter()
        .flatten()
        .map(|g| groups.iter().filter(|o| **o == Some(*g)).count())
        .max()
        .unwrap_or(0);
    (most, groups.len())
}

/// How wide the Batch control is: the switch, and while it is on, the stepper and Send.
pub(super) fn width(queue: &Queue, l: &Layout) -> f32 {
    if queue.batch.all() {
        l.switch + l.join + l.stepper + l.join + l.send
    } else {
        l.switch
    }
}

/// Draws the Batch control leftward from `right`; returns the x left of it.
pub(super) fn control(
    hud: &mut Hud,
    ui: &mut Ui,
    s: &Scene,
    r: Rect,
    queue: &Queue,
    l: &Layout,
    right: f32,
) -> f32 {
    let split = queue.batch;
    let tr = Rect::new(right - l.switch, r.y + (r.h - 34.0) * 0.5, l.switch, 34.0);
    let muster = queue.muster.filter(|_| split.all());
    let linked = muster.map_or(1, |m| m.linked);
    let hint = if split.mixed() {
        format!(
            "{} of {} are in this batch.  Click: link them all  \u{b7}  Right-Click: no batch",
            split.on, split.of
        )
    } else if split.all() && linked > 1 {
        format!(
            "{linked} factories, one batch: their units wait at each door and leave together, on one set of orders.  Click: unlink"
        )
    } else if split.all() {
        "Units wait outside and leave together once the batch is full.  Click: off".to_owned()
    } else if split.of > 1 {
        "Link these factories: units wait at each door, and all leave together on one set of orders."
            .to_owned()
    } else {
        "Units wait outside and leave together once there are enough.".to_owned()
    };
    let line = muster.map(|m| {
        (
            format!("{} ready", m.count),
            f32::from(m.count) / f32::from(m.size.max(1)),
        )
    });
    let set = switch(
        hud,
        ui,
        tr,
        l,
        Switch {
            id: "queue-batch",
            split,
            glyph: icons::Glyph::Batch,
            label: "Batch",
            key: "\u{21e7}L",
            tone: BATCH,
            hint: &hint,
            detail: line.as_ref().map(|(text, full)| (text.as_str(), *full)),
        },
    );
    if let Some(on) = set {
        hud.actions.push(HudAction::Batch(on));
    }
    let Some(m) = muster else {
        return tr.x;
    };
    let sr = Rect::new(tr.x - l.join - l.stepper, tr.y, l.stepper, tr.h);
    size_stepper(hud, ui, s, sr, m);
    let send = Rect::new(sr.x - l.join - l.send, tr.y, l.send, tr.h);
    if m.count > 0 {
        send_button(hud, ui, send, m.count);
    }
    send.x
}

/// How many the batch leaves at, between a minus and a plus. A click on either half
/// steps it, as does the wheel over it; with Shift held, five at a time.
fn size_stepper(hud: &mut Hud, ui: &mut Ui, s: &Scene, sr: Rect, m: Muster) {
    let t = hud.tile(ui, id("queue-batch-size", 0), sr, false, true);
    let left = ui.cursor.x < sr.x + sr.w * 0.5;
    let (minus, plus) = (
        Vec2::new(sr.x + 11.0, sr.mid_y()),
        Vec2::new(sr.right() - 11.0, sr.mid_y()),
    );
    let lit = |on: bool| rgb(BATCH, if t.hovered && on { 1.0 } else { 0.55 });
    ui.stroke(minus - Vec2::X * 4.0, minus + Vec2::X * 4.0, 1.6, lit(left));
    ui.stroke(plus - Vec2::X * 4.0, plus + Vec2::X * 4.0, 1.6, lit(!left));
    ui.stroke(plus - Vec2::Y * 4.0, plus + Vec2::Y * 4.0, 1.6, lit(!left));
    ui.text_centred(
        sr.x + sr.w * 0.5,
        sr.y + 10.0,
        type_scale::MICRO,
        rgb(palette::FAINT, 1.0),
        "SIZE",
    );
    ui.text_centred(
        sr.x + sr.w * 0.5,
        sr.y + 23.0,
        type_scale::ITEM,
        rgb(palette::TEXT, 1.0),
        &m.size.to_string(),
    );
    if t.hovered {
        let hint = format!(
            "Leaves once {} are ready.  Click \u{2212}/+ or wheel  \u{b7}  Shift: 5 at a time",
            m.size
        );
        tip(ui, sr.x, sr.y - 46.0, &hint);
    }
    let wheel = if t.hovered { ui.input.scroll } else { 0.0 };
    if t.clicked || wheel != 0.0 {
        let down = if wheel != 0.0 { wheel < 0.0 } else { left };
        let step = if s.view.shift { 5 } else { 1 };
        let size = if down {
            m.size.saturating_sub(step).max(1)
        } else {
            m.size.saturating_add(step).min(MAX_BATCH)
        };
        if size != m.size {
            ui.audio
                .play(if t.clicked { Sfx::Select } else { Sfx::Tick });
            hud.actions.push(HudAction::BatchSize(size));
        }
    }
}

/// Send: the `count` ready leave now, without the rest.
fn send_button(hud: &mut Hud, ui: &mut Ui, sr: Rect, count: u16) {
    let t = hud.tile(ui, id("queue-send", 0), sr, false, true);
    let lit = 0.75 + 0.25 * (ui.time * 2.4).sin().abs() + 0.2 * t.glow;
    // Folded, the arrow alone.
    let labelled = sr.w >= 56.0;
    icons::glyph(
        ui,
        icons::Glyph::Move,
        Vec2::new(
            if labelled {
                sr.x + 15.0
            } else {
                sr.x + sr.w * 0.5
            },
            sr.mid_y(),
        ),
        6.5,
        rgb(BATCH, lit),
    );
    if labelled {
        ui.text(
            sr.x + 27.0,
            sr.mid_y(),
            type_scale::MICRO,
            rgb(palette::TEXT, 0.9 + 0.1 * t.glow),
            "Send",
        );
    }
    if t.hovered {
        let hint = match count {
            1 => "Send the one ready now.".to_owned(),
            n => format!("Send the {n} ready now, without the rest."),
        };
        tip(ui, sr.x, sr.y - 46.0, &hint);
    }
    if t.clicked {
        ui.audio.play(Sfx::Select);
        hud.actions.push(HudAction::SendBatch);
    }
}
