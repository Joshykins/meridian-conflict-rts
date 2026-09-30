//! The queue strip over the construction panel: what the builder has queued, and the
//! Repeat, Pause and Batch switches (`batch.rs`). A selection of several builders can be
//! split on any switch; the buttons show how it is split and what a click will do. On a
//! narrow screen the switches fold to their glyphs, so the strip always fits.

use super::{pause_mark, tip, Stack, BUILDING};
use crate::audio::Sfx;
use crate::hud::icons;
use crate::hud::style::{domain_wash, Domain};
use crate::hud::{Hud, HudAction, Scene};
use crate::ui::{id, ink, palette, rgb, type_scale, Rect, Ui};
use glam::Vec2;
use mc_sim::tables::OrderKind;

/// How much of a selection has a switch (Repeat, Pause) on. The buttons and their keys
/// share one rule: when all of it is on, the switch turns it all off; when only some or
/// none is, it turns it all on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::hud) struct Split {
    pub(in crate::hud) on: usize,
    pub(in crate::hud) of: usize,
}

impl Split {
    /// One `true` for each unit the switch is on for, one `false` for each it is off for.
    pub(in crate::hud) fn count(states: impl IntoIterator<Item = bool>) -> Split {
        states.into_iter().fold(Split::default(), |s, on| Split {
            on: s.on + usize::from(on),
            of: s.of + 1,
        })
    }

    pub(in crate::hud) fn all(self) -> bool {
        self.of > 0 && self.on == self.of
    }

    /// Some of the selection has it on and some does not.
    pub(in crate::hud) fn mixed(self) -> bool {
        self.on > 0 && self.on < self.of
    }

    /// What a click (or the key) sets the whole selection to.
    pub(in crate::hud) fn next(self) -> bool {
        !self.all()
    }
}

/// What the queue strip shows.
pub(super) struct Queue<'a> {
    pub(super) stacks: &'a [Stack],
    /// How far along the front entry is.
    pub(super) progress: f32,
    /// Seconds before the front entry is done at the pace it is going, when it is moving.
    pub(super) eta: Option<f32>,
    pub(super) is_factory: bool,
    /// The selected factories that build their queues over and over.
    pub(super) repeat: Split,
    /// The selected factories in one batch: `on` counts those in the batch most of them
    /// share, so the switch is all on only when they are all linked in one.
    pub(super) batch: Split,
    /// This factory's batch while it is in one.
    pub(super) muster: Option<super::batch::Muster>,
    /// The selected builders whose work is paused.
    pub(super) pause: Split,
    /// This builder's work is paused: its queue waits, the front entry holds where it got to.
    pub(super) paused: bool,
    /// What the front entry is (producing, building, upgrading), when it heads the queue.
    pub(super) front: Option<OrderKind>,
}

/// The strip's measures: full, or folded to fit a narrow screen.
pub(super) struct Layout {
    /// The head: the caption, or what is under way and how far along.
    head: f32,
    /// A switch, and the gap between switches.
    pub(super) switch: f32,
    gap: f32,
    /// The batch size stepper and Send, and the gap joining them to the Batch switch.
    pub(super) stepper: f32,
    pub(super) send: f32,
    pub(super) join: f32,
}

const FULL: Layout = Layout {
    head: 176.0,
    switch: 96.0,
    gap: 10.0,
    stepper: 76.0,
    send: 64.0,
    join: 4.0,
};

/// Switches as glyphs alone (the label and key are in the tip), a shorter head.
const FOLDED: Layout = Layout {
    head: 132.0,
    switch: 40.0,
    gap: 6.0,
    stepper: 62.0,
    send: 30.0,
    join: 3.0,
};

/// A queued tile, and the gap after it.
const STACK_W: f32 = 48.0;
const STACK_GAP: f32 = 5.0;

/// The measures that fit `w`: full while the head, the switches and a queued tile or two
/// fit at full size.
fn layout(w: f32, queue: &Queue) -> &'static Layout {
    let wants = |l: &Layout| {
        let mut switches = l.switch;
        if queue.is_factory {
            switches += l.gap + l.switch + l.gap + super::batch::width(queue, l);
        }
        14.0 + l.head + 18.0 + 2.0 * (STACK_W + STACK_GAP) + 30.0 + switches + l.gap + 12.0
    };
    if w >= wants(&FULL) {
        &FULL
    } else {
        &FOLDED
    }
}

pub(super) fn draw(hud: &mut Hud, ui: &mut Ui, s: &Scene, r: Rect, queue: &Queue) {
    hud.glass(ui, r);
    let l = layout(r.w, queue);
    let end = head(ui, s, r, queue, l.head);
    let pause = queue.pause;

    // Repeat, for a factory: build the queue over and over.
    let mut right = r.right() - 12.0;
    let mut batch_right = None;
    if queue.is_factory {
        let tr = Rect::new(right - l.switch, r.y + (r.h - 34.0) * 0.5, l.switch, 34.0);
        let hint = if queue.repeat.mixed() {
            format!(
                "{} of {} factories repeat.  Click: all repeat  \u{b7}  Right-Click: none do",
                queue.repeat.on, queue.repeat.of
            )
        } else {
            "The factory builds its queue over and over.".to_owned()
        };
        let set = switch(
            hud,
            ui,
            tr,
            l,
            Switch {
                id: "queue-repeat",
                split: queue.repeat,
                glyph: icons::Glyph::Repeat,
                label: "Repeat",
                key: "L",
                tone: palette::TEXT,
                hint: &hint,
                detail: None,
            },
        );
        if let Some(on) = set {
            hud.actions.push(HudAction::Repeat(on));
        }
        right = tr.x - l.gap;
    }
    // Pause, for any builder: the queue stays, the spending stops.
    {
        let tr = Rect::new(right - l.switch, r.y + (r.h - 34.0) * 0.5, l.switch, 34.0);
        let (glyph, label, hint) = if pause.mixed() {
            (
                icons::Glyph::Pause,
                "Pause All",
                format!(
                    "{} of {} are paused.  Click: pause the rest  \u{b7}  Right-Click: resume all",
                    pause.on, pause.of
                ),
            )
        } else if pause.all() {
            (
                icons::Glyph::Play,
                "Resume",
                "Carry on from where it stopped.".to_owned(),
            )
        } else {
            (
                icons::Glyph::Pause,
                "Pause",
                "Keep the queue but spend nothing. Helpers wait too.".to_owned(),
            )
        };
        let set = switch(
            hud,
            ui,
            tr,
            l,
            Switch {
                id: "queue-pause",
                split: pause,
                glyph,
                label,
                key: "Z",
                tone: BUILDING,
                hint: &hint,
                detail: None,
            },
        );
        if let Some(paused) = set {
            hud.actions.push(HudAction::PauseWork(paused));
        }
        right = tr.x - l.gap;
        if queue.is_factory {
            batch_right = Some(right);
        }
    }
    // Batch, for a factory, leftmost: it is the one that grows, and it grows away from
    // the other switches.
    if let Some(at) = batch_right {
        right = super::batch::control(hud, ui, s, r, queue, l, at) - l.gap;
    }
    stack_tiles(hud, ui, s, r, queue, end + 18.0, right);
}

/// The strip's head. While something is under way: what the builder is doing, to what,
/// how far along in the construction amber, and what waits behind it. With nothing under
/// way: the queue's caption. Returns the x after it.
fn head(ui: &mut Ui, s: &Scene, r: Rect, queue: &Queue, head_w: f32) -> f32 {
    let x = r.x + 14.0;
    let total: usize = queue.stacks.iter().map(|k| k.count).sum();
    let pause = queue.pause;
    let waiting = total.saturating_sub(1);
    let (note, tone) = if pause.mixed() {
        (
            format!("{} of {} paused  \u{b7}  Z pauses all", pause.on, pause.of),
            BUILDING,
        )
    } else if queue.paused {
        (format!("{waiting} waiting  \u{b7}  Z resumes"), BUILDING)
    } else if queue.front.is_some() {
        let next = match waiting {
            0 => "Nothing after it".to_owned(),
            n => format!("{n} more queued"),
        };
        (
            format!("{next}  \u{b7}  right-click removes"),
            palette::FAINT,
        )
    } else {
        (
            format!("{total} queued  \u{b7}  right-click removes"),
            palette::FAINT,
        )
    };
    let (front, first) = match (queue.front, queue.stacks.first()) {
        (Some(kind), Some(first)) => (kind, first),
        _ => {
            // Nothing under way: the caption, over the count.
            let label = if queue.is_factory {
                "Production Queue"
            } else {
                "Build Queue"
            };
            ui.fill(
                Rect::new(x, r.mid_y() - 5.0, 3.0, 10.0),
                rgb(palette::TEXT, 0.9),
            );
            ui.text_fit_left(
                x + 12.0,
                r.mid_y() - 8.0,
                head_w - 12.0,
                type_scale::CAPTION,
                rgb(palette::DIM, 1.0),
                label,
            );
            ui.text_fit_left(
                x + 12.0,
                r.mid_y() + 9.0,
                head_w - 12.0,
                type_scale::MICRO,
                rgb(tone, 1.0),
                &note,
            );
            return x + head_w;
        }
    };

    let paused = queue.paused;
    let p = queue.progress.clamp(0.0, 1.0);
    let lit = if paused { 0.55 } else { 1.0 };
    // The bar at the left edge: amber, breathing while the work goes on.
    let breathe = if paused {
        0.5
    } else {
        0.6 + 0.4 * (ui.time * 3.2).sin().abs()
    };
    ui.fill(
        Rect::new(x, r.y + 12.0, 3.0, r.h - 24.0),
        rgb(BUILDING, 0.9 * breathe),
    );
    let (tx, w) = (x + 12.0, head_w - 12.0);
    // What it is doing, and to what: a refit by its module's name.
    let doing = if paused {
        "Paused"
    } else {
        crate::hud::selection::activity(front)
    };
    let item = s.blueprints.unit(first.blueprint);
    let name = crate::hud::refit::queued_name(s.blueprints, first.blueprint).unwrap_or(&item.name);
    // One figure at the right: the time left while the work moves (the rail under it
    // shows how far along), the percentage while it stands still.
    let eta = queue.eta.filter(|_| !paused);
    let figure = match eta {
        Some(t) => crate::hud::mine::duration(t.ceil()),
        None => format!("{:.0}%", p * 100.0),
    };
    let figure_w = ui.text_width(type_scale::ITEM, &figure);
    ui.text(
        tx,
        r.y + 13.0,
        type_scale::MICRO,
        rgb(BUILDING, 0.85 * lit),
        doing,
    );
    ui.text_fit_left(
        tx,
        r.y + 28.0,
        w - figure_w - 10.0,
        type_scale::CAPTION,
        rgb(palette::TEXT, if paused { 0.7 } else { 1.0 }),
        name,
    );
    ui.text_right(
        tx + w,
        r.y + 23.0,
        type_scale::ITEM,
        rgb(if eta.is_some() { 0xFFE3A0 } else { BUILDING }, lit),
        &figure,
    );
    // The rail: filled to the progress, a glint running along it while it moves.
    let rail = Rect::new(tx, r.y + 38.0, w, 3.0);
    ui.fill(rail, rgb(BUILDING, 0.16));
    let done = rail.w * p;
    if paused {
        ui.fill(Rect::new(rail.x, rail.y, done, rail.h), rgb(BUILDING, 0.5));
    } else {
        ui.gradient_h(
            Rect::new(rail.x, rail.y, done, rail.h),
            rgb(BUILDING, 0.7),
            rgb(0xFFE3A0, 1.0),
        );
        let gx = rail.x + done * (ui.time * 0.8).fract();
        ui.gradient_h(
            Rect::new(
                (gx - 14.0).max(rail.x),
                rail.y - 1.0,
                (gx - rail.x).min(14.0),
                rail.h + 2.0,
            ),
            rgb(0xFFFFFF, 0.0),
            rgb(0xFFFFFF, 0.8),
        );
        // A bright tick where the fill ends.
        ui.fill(
            Rect::new(rail.x + done - 1.0, rail.y - 2.0, 2.0, rail.h + 4.0),
            rgb(0xFFFFFF, 0.9 * breathe),
        );
    }
    ui.text_fit_left(tx, r.y + 51.0, w, type_scale::MICRO, rgb(tone, 1.0), &note);
    x + head_w
}

/// A switch button on the strip.
pub(super) struct Switch<'a> {
    pub(super) id: &'static str,
    pub(super) split: Split,
    pub(super) glyph: icons::Glyph,
    pub(super) label: &'a str,
    pub(super) key: &'static str,
    /// The colour it lights in while on.
    pub(super) tone: u32,
    pub(super) hint: &'a str,
    /// While on for the whole selection: a line under the label, and how full the gauge
    /// along the foot is.
    pub(super) detail: Option<(&'a str, f32)>,
}

/// Draws a switch: lit while all of the selection has it on, dark while none does. A split
/// selection gets a half-lit frame, a gauge along the foot filled to the share that has it
/// on, and the count under the label. Returns what a click sets the selection to: `next`
/// on a click, and on a split selection a right-click turns it all off. Folded, it is its
/// glyph alone, and the tip names it.
pub(super) fn switch(hud: &mut Hud, ui: &mut Ui, tr: Rect, l: &Layout, sw: Switch) -> Option<bool> {
    let Switch { split, tone, .. } = sw;
    let (all, mixed) = (split.all(), split.mixed());
    let t = hud.tile(ui, id(sw.id, 0), tr, all, true);
    let lit = if all { tone } else { palette::TEXT };
    let folded = l.switch < FULL.switch;
    icons::glyph(
        ui,
        sw.glyph,
        Vec2::new(
            if folded {
                tr.x + tr.w * 0.5
            } else {
                tr.x + 16.0
            },
            tr.mid_y() - if folded { 2.0 } else { 0.0 },
        ),
        7.0,
        rgb(lit, 0.8 + 0.2 * t.glow),
    );
    let detail = sw.detail.filter(|_| all);
    if folded {
        if mixed {
            dashed_frame(ui, tr, rgb(tone, 0.55 + 0.3 * t.glow));
            gauge(ui, tr, tone, split.on as f32 / split.of as f32);
        }
        if let Some((_, full)) = detail {
            gauge(ui, tr, tone, full);
        }
        if t.hovered {
            let key = sw.key.replace('\u{21e7}', "Shift+");
            let line = detail.map_or(String::new(), |(d, _)| format!("  \u{b7}  {d}"));
            let hint = format!("{} ({key}){line}  \u{b7}  {}", sw.label, sw.hint);
            tip(ui, tr.x, tr.y - 46.0, &hint);
        }
        return switch_click(ui, t, split);
    }
    let label_y = if mixed || detail.is_some() {
        tr.mid_y() - 5.0
    } else {
        tr.mid_y()
    };
    let label_end = ui.text(
        tr.x + 30.0,
        label_y,
        type_scale::MICRO,
        rgb(
            if all {
                tone
            } else if mixed {
                palette::TEXT
            } else {
                palette::DIM
            },
            1.0,
        ),
        sw.label,
    );
    // A leading shift mark is drawn, not written: the font has no arrow.
    let (shift, key) = match sw.key.strip_prefix('\u{21e7}') {
        Some(key) => (true, key),
        None => (false, sw.key),
    };
    let key_x = tr.right() - 5.0 - ui.text_width(type_scale::MICRO, key);
    // A long label (Batch x3) takes the key's corner: the key is in the tip.
    let key_room = label_end < key_x - if shift { 13.0 } else { 4.0 };
    if key_room {
        ui.text(
            key_x,
            tr.y + 8.0,
            type_scale::MICRO,
            rgb(palette::FAINT, 1.0),
            key,
        );
    }
    if shift && key_room {
        crate::hud::selection::shift_mark(
            ui,
            Vec2::new(key_x - 7.0, tr.y + 8.0),
            rgb(palette::FAINT, 1.0),
        );
    }
    if mixed {
        // Half lit: a dashed frame in the switch's colour, and the share that has it on.
        dashed_frame(ui, tr, rgb(tone, 0.55 + 0.3 * t.glow));
        ui.text(
            tr.x + 30.0,
            tr.mid_y() + 7.0,
            type_scale::MICRO,
            rgb(tone, 0.95),
            &format!("{}/{} on", split.on, split.of),
        );
        gauge(ui, tr, tone, split.on as f32 / split.of as f32);
    }
    if let Some((line, full)) = detail {
        ui.text(
            tr.x + 30.0,
            tr.mid_y() + 7.0,
            type_scale::MICRO,
            rgb(tone, 0.95),
            line,
        );
        gauge(ui, tr, tone, full);
    }
    if t.hovered {
        tip(ui, tr.x, tr.y - 46.0, sw.hint);
    }
    switch_click(ui, t, split)
}

/// What a click on a switch sets the selection to.
fn switch_click(ui: &mut Ui, t: crate::hud::Tile, split: Split) -> Option<bool> {
    let mixed = split.mixed();
    if t.clicked {
        ui.audio.play(Sfx::Select);
        Some(split.next())
    } else if t.right_clicked && mixed {
        ui.audio.play(Sfx::Back);
        Some(false)
    } else {
        None
    }
}

/// A gauge along the foot of the switch `tr`, filled to `full` (zero to one).
fn gauge(ui: &mut Ui, tr: Rect, tone: u32, full: f32) {
    let gauge = Rect::new(tr.x + 6.0, tr.bottom() - 5.0, tr.w - 12.0, 2.0);
    ui.fill(gauge, rgb(tone, 0.18));
    ui.fill(
        Rect::new(gauge.x, gauge.y, gauge.w * full.clamp(0.0, 1.0), gauge.h),
        rgb(tone, 0.95),
    );
}

/// A frame of short dashes just inside `r`.
fn dashed_frame(ui: &mut Ui, r: Rect, color: crate::ui::Color) {
    let (dash, gap, inset): (f32, f32, f32) = (5.0, 4.0, 2.0);
    let (x0, x1, y0, y1) = (
        r.x + inset,
        r.right() - inset,
        r.y + inset,
        r.bottom() - inset,
    );
    let mut x = x0 + 6.0;
    while x < x1 - 6.0 {
        let w = dash.min(x1 - 6.0 - x);
        ui.fill(Rect::new(x, y0, w, 1.0), color);
        ui.fill(Rect::new(x, y1 - 1.0, w, 1.0), color);
        x += dash + gap;
    }
    let mut y = y0 + 6.0;
    while y < y1 - 6.0 {
        let h = dash.min(y1 - 6.0 - y);
        ui.fill(Rect::new(x0, y, 1.0, h), color);
        ui.fill(Rect::new(x1 - 1.0, y, 1.0, h), color);
        y += dash + gap;
    }
}

/// The queued stacks, from `x` up to `right`, and a count of those that do not fit.
fn stack_tiles(hud: &mut Hud, ui: &mut Ui, s: &Scene, r: Rect, queue: &Queue, x: f32, right: f32) {
    let Queue {
        stacks,
        progress,
        is_factory,
        paused,
        ..
    } = *queue;
    let (w, h, gap) = (STACK_W, 46.0, STACK_GAP);
    let mut x = x;
    // Room for every tile, or for as many as fit with the count of the rest after them.
    let fits = |room: f32| ((room + gap) / (w + gap)).max(0.0) as usize;
    let room = if fits(right - x) >= stacks.len() {
        stacks.len()
    } else {
        fits(right - 30.0 - x)
    };
    for (i, k) in stacks.iter().take(room).enumerate() {
        let item = s.blueprints.unit(k.blueprint);
        let tr = Rect::new(x, r.y + (r.h - h) * 0.5, w, h);
        let t = hud.tile(ui, id("queue", i), tr, false, true);
        // What is being built is lit in the construction amber alone; the rest wait in their domain's colour.
        if i == 0 && paused {
            held(ui, tr, progress);
        } else if i == 0 {
            building(ui, tr, progress);
        } else {
            domain_wash(
                ui,
                Rect::new(tr.x + 3.0, tr.y + 3.0, tr.w - 6.0, tr.h - 6.0),
                Domain::of(item),
                t.glow * 0.5,
            );
        }
        if !hud.thumbs.draw(
            ui,
            item.id,
            Rect::new(tr.x + 3.0, tr.y + 2.0, 36.0, 36.0),
            1.0,
        ) {
            icons::strategic(
                ui,
                item.visual.icon,
                item.tech,
                Vec2::new(tr.x + 18.0, tr.y + 18.0),
                9.5,
                rgb(palette::TEXT, 0.8 + 0.2 * t.glow),
                ink(0.9),
            );
        }
        if i == 0 && paused {
            pause_mark(ui, Vec2::new(tr.x + 18.0, tr.y + 18.0), 18.0);
        }
        let refit_name = crate::hud::refit::queued_name(s.blueprints, k.blueprint);
        if let Some(name) = refit_name {
            // A refit: the module's name across the foot of the tile.

            ui.fill(
                Rect::new(tr.x + 3.0, tr.bottom() - 17.0, tr.w - 6.0, 13.0),
                ink(0.75),
            );
            ui.text_fit(
                tr.x + tr.w * 0.5,
                tr.bottom() - 10.5,
                tr.w - 8.0,
                type_scale::MICRO,
                rgb(0xFFFFFF, 1.0),
                name,
            );
        }
        let count = if k.upgrade {
            "UP".to_owned()
        } else {
            format!("{}", k.count)
        };
        ui.text_right(
            tr.right() - 5.0,
            tr.y + 12.0,
            type_scale::VALUE,
            rgb(0xFFFFFF, 1.0),
            &count,
        );
        if i == 0 {
            front_track(ui, tr, progress, paused);
        }
        if k.upgrade {
            if t.right_clicked {
                ui.audio.play(Sfx::Back);
                // A tier is cancelled by its own blueprint too, with the tiers after it.
                hud.actions.push(HudAction::CancelRefit(k.blueprint));
            }
        } else if is_factory {
            if t.clicked {
                ui.audio.play(Sfx::Select);
                hud.actions.push(HudAction::Build(k.blueprint));
            }
            if t.right_clicked {
                ui.audio.play(Sfx::Back);
                hud.actions.push(HudAction::Cancel(k.blueprint));
            }
        } else if t.right_clicked {
            // An engineer's site: the last one of the stack comes out.
            ui.audio.play(Sfx::Back);
            hud.actions.push(HudAction::CancelOrder {
                kind: OrderKind::Build,
                pos: k.last,
            });
        }
        if t.hovered {
            let hint = if let Some(hint) = crate::hud::refit::queue_hint(s.blueprints, k.blueprint)
            {
                hint
            } else if k.upgrade {
                format!("Upgrade to {}  \u{b7}  Right-Click Cancels", item.name)
            } else if is_factory {
                format!(
                    "{}  \u{b7}  Click Adds  \u{b7}  Right-Click Removes",
                    item.name
                )
            } else {
                format!("{}  \u{b7}  Right-Click Removes the Last", item.name)
            };
            tip(ui, tr.x, r.y - 32.0, &hint);
        }
        x += w + gap;
    }
    if stacks.len() > room && right - x >= 24.0 {
        ui.text(
            x + 4.0,
            r.mid_y(),
            type_scale::VALUE,
            rgb(palette::DIM, 1.0),
            &format!("+{}", stacks.len() - room),
        );
    }
}

/// The front entry's progress along the foot of its tile, in the construction amber.
fn front_track(ui: &mut Ui, tr: Rect, progress: f32, paused: bool) {
    if paused {
        // Where it stopped, still: no glint while nothing is spent.
        let track = Rect::new(tr.x + 5.0, tr.bottom() - 8.0, tr.w - 10.0, 3.0);
        ui.fill(track, rgb(BUILDING, 0.18));
        ui.fill(
            Rect::new(
                track.x,
                track.y,
                track.w * progress.clamp(0.0, 1.0),
                track.h,
            ),
            rgb(BUILDING, 0.55),
        );
    } else {
        // Its progress, in the construction amber, with a glint running along it.
        let track = Rect::new(tr.x + 5.0, tr.bottom() - 8.0, tr.w - 10.0, 3.0);
        ui.fill(track, rgb(BUILDING, 0.18));
        let done = track.w * progress.clamp(0.0, 1.0);
        ui.gradient_h(
            Rect::new(track.x, track.y, done, track.h),
            rgb(BUILDING, 0.7),
            rgb(0xFFE3A0, 1.0),
        );
        let glint = (ui.time * 0.8).fract();
        let gx = track.x + done * glint;
        ui.gradient_h(
            Rect::new(
                (gx - 10.0).max(track.x),
                track.y - 1.0,
                (gx - track.x).min(10.0),
                track.h + 2.0,
            ),
            rgb(0xFFFFFF, 0.0),
            rgb(0xFFFFFF, 0.8),
        );
    }
}

/// The item being built: an amber frame breathing around it, amber light
/// welling up from the foot, and a sheen sweeping across it.
fn building(ui: &mut Ui, tr: Rect, progress: f32) {
    let breathe = 0.55 + 0.45 * (ui.time * 3.2).sin().abs();
    let inner = Rect::new(tr.x + 3.0, tr.y + 3.0, tr.w - 6.0, tr.h - 6.0);
    ui.gradient_v(
        inner,
        rgb(BUILDING, 0.08),
        rgb(BUILDING, 0.22 + 0.2 * progress.clamp(0.0, 1.0) * breathe),
    );
    let sweep = (ui.time * 0.55).fract();
    let sx = inner.x - 14.0 + (inner.w + 28.0) * sweep;
    let (x0, x1) = (sx.max(inner.x), (sx + 14.0).min(inner.right()));
    if x1 > x0 {
        ui.gradient_h(
            Rect::new(x0, inner.y, x1 - x0, inner.h),
            rgb(0xFFD58A, 0.0),
            rgb(0xFFD58A, 0.16),
        );
    }
    ui.outline_cut(
        tr,
        5.0,
        rgb(BUILDING, 0.9 * breathe),
        rgb(BUILDING, breathe),
    );
}

/// The front of a paused queue: the construction amber held still and dimmed, with a
/// pause mark over the picture.
fn held(ui: &mut Ui, tr: Rect, progress: f32) {
    let inner = Rect::new(tr.x + 3.0, tr.y + 3.0, tr.w - 6.0, tr.h - 6.0);
    ui.gradient_v(
        inner,
        rgb(BUILDING, 0.04),
        rgb(BUILDING, 0.10 + 0.08 * progress.clamp(0.0, 1.0)),
    );
    ui.outline_cut(tr, 5.0, rgb(BUILDING, 0.45), rgb(BUILDING, 0.6));
}

#[cfg(test)]
mod tests {
    use super::Split;

    #[test]
    fn a_switch_turns_all_on_unless_all_are_on() {
        let none = Split::count([false, false, false]);
        let some = Split::count([true, false, true]);
        let all = Split::count([true, true]);
        assert_eq!(some, Split { on: 2, of: 3 });
        assert!(!none.mixed() && !none.all() && none.next());
        assert!(some.mixed() && !some.all() && some.next());
        assert!(!all.mixed() && all.all() && !all.next());
        // Nothing selected that has the switch: neither on nor split.
        let empty = Split::count([]);
        assert!(!empty.all() && !empty.mixed());
    }
}
