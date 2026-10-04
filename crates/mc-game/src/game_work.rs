//! Work under way shows without a selection. Whatever a side builds carries its
//! progress bar (bars only, no ring): a site going up, a structure upgrading, a
//! factory's product, an engineer's build or assist. A site or an upgrade also
//! carries its percentage, and the time it has left while its work moves
//! (`mirror::WorkLeft`), on a tab hanging from its construction bar; the engineers
//! round it do not, so a crowd of them does not repeat the figure.

use super::{unit_bar_shield, unit_bar_work, View};
use crate::orders::Field;
use crate::ui::{palette, rgb, style, Rect, Style, Ui};
use glam::{Vec2, Vec3};
use mc_render::Mark;
use mc_sim::mirror::{UnitInstance, KIND_WRECK, STATE_UNIDENTIFIED};
use mc_sim::tables::{flag, OrderKind};
use std::collections::HashSet;

const UNDER: u32 = (flag::UNDER_CONSTRUCTION as u32) << 8;
/// The construction bar as it reaches the screen (`icons.wgsl` `fs_bar`, after tone mapping).
const BAR: u32 = 0xE8D774;
const PAUSED: u32 = 0xA3A3A0;
const FIGURE: Style = style(mc_render::Face::Bold, 14.0, 0.5);

fn shown(u: &UnitInstance, friend: &impl Fn(u8) -> bool) -> bool {
    u.owner_flags & (KIND_WRECK | STATE_UNIDENTIFIED) == 0 && friend((u.owner_flags & 0xFF) as u8)
}

/// A site going up, or a structure rebuilding into its next tier.
fn rising(u: &UnitInstance) -> bool {
    u.owner_flags & UNDER != 0 || u.upgrade > 0.0
}

/// Appends a bars-only mark for every friendly unit at work that `marks` does not
/// already hold. `friend` says whose work shows: the viewer's side and its allies.
pub(crate) fn bar_marks(view: &View, friend: impl Fn(u8) -> bool, marks: &mut Vec<Mark>) {
    let mut marked: HashSet<u32> = marks.iter().map(|m| m.unit_index).collect();
    let units = &view.frame.units;
    let mut push = |i: usize, work: f32, marks: &mut Vec<Mark>| {
        if work >= 0.0 && marked.insert(i as u32) {
            marks.push(Mark {
                unit_index: i as u32,
                kind: Mark::BARS_ONLY,
                work,
                shield: unit_bar_shield(units[i].unit_id, &view.frame.shields),
            });
        }
    };
    // Sites, upgrades and launchers read their progress off the unit itself.
    for (i, u) in units.iter().enumerate() {
        if shown(u, &friend) {
            push(i, unit_bar_work(u, &[]), marks);
        }
    }
    // Factories and engineers, off their queue.
    for q in &view.status.queues {
        let working = q.orders.first().is_some_and(|o| {
            matches!(
                o.kind,
                OrderKind::Build | OrderKind::Produce | OrderKind::Upgrade | OrderKind::Assist
            )
        });
        if !working || q.progress <= 0.0 {
            continue;
        }
        let Some(&i) = view.index_of.get(&q.unit_id) else {
            continue;
        };
        if shown(&units[i], &friend) {
            push(i, q.progress.clamp(0.0, 1.0), marks);
        }
    }
}

/// Status bar heights in pixels, as `icons.wgsl` `vs_bar` stacks them.
const HEALTH_H: f32 = 6.0;
const BUILD_H: f32 = 5.0;
const SHIELD_H: f32 = 4.0;
const GAP: f32 = 2.0;

/// The percentage tag under each friendly site's or upgrade's bars, with the time
/// left while its work moves.
pub(crate) fn draw_tags(ui: &mut Ui, field: &Field, alpha: f32, friend: impl Fn(u8) -> bool) {
    let _t = mc_core::perf_span!("ui.work_tags");
    let view = field.view;
    let eta_of = |id: u32| {
        view.frame
            .work_left
            .iter()
            .find(|w| w.unit_id == id)
            .map(|w| w.seconds)
    };
    let camera = field.camera;
    let eye = camera.eye();
    let scale = camera.projection_scale();
    for u in &view.frame.units {
        if !rising(u) || !shown(u, &friend) {
            continue;
        }
        let p = Vec3::from(u.prev_pos).lerp(Vec3::from(u.pos), alpha);
        let ground = field.renderer.ground_height(Vec2::new(p.x, p.y));
        let feet = Vec3::new(p.x, p.y, p.z.max(ground));
        let Some(at) = camera.project(feet) else {
            continue;
        };
        let px = scale / feet.distance(eye).max(1.0);
        // Zoomed out to icons, the tag fades with the model.
        let fade = ((u.radius * px - 8.0) / 10.0).clamp(0.0, 1.0);
        if fade <= 0.0 {
            continue;
        }
        // The bar stack as `vs_bar` lays it out, in output pixels: its top sits
        // `max(r * 0.6 px, 6) + 2` under the feet, and construction is its last row.
        let shield = view.frame.shields.iter().any(|s| s.unit_id == u.unit_id);
        let stack = HEALTH_H + BUILD_H + GAP + if shield { SHIELD_H + GAP } else { 0.0 };
        let half_w = (u.radius * 0.8 * px).max(16.0);
        let bottom = at.y + (u.radius * 0.6 * px).max(6.0) + 2.0 + stack;
        let share = if u.owner_flags & UNDER != 0 {
            u.build
        } else {
            u.upgrade
        };
        let was = ui.fade;
        ui.fade *= fade;
        tab(
            ui,
            (at.x - half_w) / ui.s,
            2.0 * half_w / ui.s,
            bottom / ui.s,
            share,
            eta_of(u.unit_id).filter(|_| !u.paused()),
            u.paused(),
        );
        ui.fade = was;
    }
}

/// A tab hanging from the construction bar where its fill ends, in the bar's own
/// colour: one reading of progress, not a second bar, and after a rule the time left.
/// `left`/`width` are the bar's span and `bottom` its lower edge, in points.
fn tab(
    ui: &mut Ui,
    left: f32,
    width: f32,
    bottom: f32,
    share: f32,
    eta: Option<f32>,
    paused: bool,
) {
    let share = share.clamp(0.0, 1.0);
    let tone = if paused { PAUSED } else { BAR };
    let text = if paused {
        format!("{:.0}%  PAUSED", (share * 100.0).floor())
    } else {
        format!("{:.0}%", (share * 100.0).floor())
    };
    let time = eta.map(|t| crate::hud::mine::duration(t.ceil()));
    let (h, notch, cut, pad, rule) = (20.0, 4.0, 3.0, 8.0, 13.0);
    let text_w = ui.text_width(FIGURE, &text);
    let time_w = time
        .as_ref()
        .map_or(0.0, |t| ui.text_width(FIGURE, t) + rule);
    let w = text_w + time_w + 2.0 * pad;
    let tip = left + width * share;
    // Centred on the fill's end, kept under the bar.
    let x = (tip - w * 0.5).clamp(left, (left + width - w).max(left));
    let r = Rect::new(x, bottom + notch, w, h);
    ui.fill_cut(r, cut, rgb(palette::INK, 0.82));
    ui.hline(r.x + cut, r.y, r.w - 2.0 * cut, rgb(tone, 0.9));
    // The notch: a small wedge up from the tab to the bar, at the fill's end.
    let tip = tip.clamp(r.x + cut + notch, r.right() - cut - notch);
    ui.triangle(
        Vec2::new(tip, bottom),
        Vec2::new(tip + notch, r.y + 0.5),
        Vec2::new(tip - notch, r.y + 0.5),
        rgb(tone, 0.9),
    );
    let mid = r.y + h * 0.5 + 0.5;
    let end = ui.text(r.x + pad, mid, FIGURE, rgb(tone, 1.0), &text);
    if let Some(time) = time {
        let at = end + rule * 0.5;
        ui.fill(
            Rect::new(at - 0.5, r.y + 5.0, 1.0, h - 10.0),
            rgb(tone, 0.35),
        );
        ui.text(
            at + rule * 0.5,
            mid,
            FIGURE,
            rgb(palette::TEXT, 0.95),
            &time,
        );
    }
}
