//! The minimap: the map's tactical chart with every known unit on it and the
//! camera's footprint. Left button looks (or aims the order being targeted),
//! right button orders the selection there.

use super::{has_flag, Hud, HudAction, Scene};
use crate::game::Mode;
use crate::ui::{id, ink, palette, preview, rgb, type_scale, Rect, Ui};
use glam::{Vec2, Vec3};
use mc_sim::mirror::{KIND_WRECK, STATE_UNIDENTIFIED};
use mc_sim::tables::flag;

/// The overlay image slot the chart lives in during a match.
pub const MINIMAP_SLOT: usize = 0;
/// Most unit marks drawn; beyond it every n-th unit stands for its neighbours.
const MAX_MARKS: usize = 3000;

/// Clips a segment to a rectangle (Liang-Barsky). `None` when it misses.
fn clip(a: Vec2, b: Vec2, r: Rect) -> Option<(Vec2, Vec2)> {
    let d = b - a;
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for (p, q) in [
        (-d.x, a.x - r.x),
        (d.x, r.right() - a.x),
        (-d.y, a.y - r.y),
        (d.y, r.bottom() - a.y),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
        } else {
            let t = q / p;
            if p < 0.0 {
                t0 = t0.max(t);
            } else {
                t1 = t1.min(t);
            }
        }
    }
    (t0 <= t1).then(|| (a + d * t0, a + d * t1))
}

pub fn draw(hud: &mut Hud, ui: &mut Ui, s: &Scene, outer: Rect) {
    hud.glass(ui, outer);
    let view = s.view;
    let header = 22.0;
    let side = (outer.w - 12.0).min(outer.h - header - 8.0);
    let chart = Rect::new(
        outer.x + (outer.w - side) * 0.5,
        outer.y + header,
        side,
        side,
    );
    let size = preview::SIZE as f32;

    let focus = s.camera.focus.truncate();
    ui.text(
        outer.x + 10.0,
        outer.y + 12.0,
        type_scale::MICRO,
        rgb(palette::DIM, 1.0),
        &s.map.name().to_uppercase(),
    );
    ui.text_right(
        outer.right() - 10.0,
        outer.y + 12.0,
        type_scale::MICRO,
        rgb(palette::FAINT, 1.0),
        &format!("{:05.0} \u{b7} {:05.0}", focus.x, focus.y),
    );

    ui.fill(chart, ink(0.9));
    ui.image(
        MINIMAP_SLOT,
        [0.0, 0.0, size, size],
        chart,
        [0.82, 0.82, 0.82, 1.0],
    );

    // Mass deposits: a live mark so they read on the chart, not only as a
    // baked pixel. Occupied sites sit under the extractor's square.
    for d in s.map.mass_deposits() {
        let p = chart_pos(s, chart, Vec2::from(d.to_f32()));
        ui.fill(
            Rect::new(p.x - 2.2, p.y - 2.2, 4.4, 4.4),
            rgb(super::MASS, 0.95),
        );
        ui.fill(
            Rect::new(p.x - 0.7, p.y - 2.6, 1.4, 5.2),
            rgb(0x0A120E, 0.85),
        );
        ui.fill(
            Rect::new(p.x - 2.6, p.y - 0.7, 5.2, 1.4),
            rgb(0x0A120E, 0.85),
        );
    }

    // Units. Structures are squares a touch larger; the selection is white.
    let units = &view.frame.units;
    let stride = units.len().div_ceil(MAX_MARKS).max(1);
    for u in units.iter().step_by(stride) {
        if u.owner_flags & KIND_WRECK != 0 || has_flag(u, flag::IN_FACTORY) {
            continue;
        }
        let p = chart_pos(s, chart, Vec2::new(u.pos[0], u.pos[1]));
        let unknown = u.owner_flags & STATE_UNIDENTIFIED != 0;
        let half = if !unknown && s.bp(u).is_structure() {
            1.7
        } else {
            1.2
        };
        ui.fill(
            Rect::new(p.x - half, p.y - half, half * 2.0, half * 2.0),
            if unknown {
                rgb(0x9AA0A8, 1.0)
            } else {
                s.team_color((u.owner_flags & 0xFF) as u8)
            },
        );
    }
    for unit in view
        .selection
        .iter()
        .take(400)
        .filter_map(|id| view.index_of.get(id))
        .map(|&i| &units[i])
    {
        let p = chart_pos(s, chart, Vec2::new(unit.pos[0], unit.pos[1]));
        ui.fill(
            Rect::new(p.x - 1.4, p.y - 1.4, 2.8, 2.8),
            rgb(0xFFFFFF, 1.0),
        );
    }

    // The camera's footprint: the view's corners cast onto the ground plane.
    let vp = s.camera.viewport;
    let ground = s.camera.focus.z;
    let corners = [Vec2::ZERO, Vec2::new(vp.x, 0.0), vp, Vec2::new(0.0, vp.y)].map(|pixel| {
        let (origin, dir): (Vec3, Vec3) = s.camera.ray(pixel);
        // A ray at or above the horizon never lands: stop it a long way out.
        let t = if dir.z < -1e-4 {
            ((ground - origin.z) / dir.z).min(60_000.0)
        } else {
            60_000.0
        };
        chart_pos(s, chart, (origin + dir * t).truncate())
    });
    for i in 0..4 {
        if let Some((a, b)) = clip(corners[i], corners[(i + 1) % 4], chart) {
            ui.stroke(a, b, 1.3, rgb(0xFFFFFF, 0.92));
        }
    }

    ui.frame(chart, rgb(palette::LINE, 0.22));
    // Scale ticks along the top and left edges, like a chart's border.
    for k in 1..8 {
        let t = k as f32 / 8.0;
        let len = if k % 4 == 0 { 6.0 } else { 3.0 };
        ui.vline(
            chart.x + chart.w * t,
            chart.y,
            len,
            rgb(palette::LINE, 0.45),
        );
        ui.hline(
            chart.x,
            chart.y + chart.h * t,
            len,
            rgb(palette::LINE, 0.45),
        );
    }

    let res = ui.interact_with(id("minimap", 0), chart, true, false);
    let at = preview::world_at(s.map, ui.cursor - Vec2::new(chart.x, chart.y), chart.w);
    if matches!(view.mode, Mode::Target(_)) {
        if res.clicked {
            hud.actions.push(HudAction::TargetAt(at));
        }
    } else if res.held {
        hud.actions.push(HudAction::LookAt(at));
    }
    if res.hovered && ui.input.right_pressed {
        hud.actions.push(HudAction::OrderAt(at));
    }
}

fn chart_pos(s: &Scene, chart: Rect, world: Vec2) -> Vec2 {
    Vec2::new(chart.x, chart.y) + preview::locate(s.map, world.into(), chart.w)
}
