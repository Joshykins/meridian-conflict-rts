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
        &s.map.name(),
    );
    ui.text_right(
        outer.right() - 34.0,
        outer.y + 12.0,
        type_scale::MICRO,
        rgb(palette::FAINT, 1.0),
        &format!("{:05.0} \u{b7} {:05.0}", focus.x, focus.y),
    );
    // Folds the map away; the MAP tab left in its place brings it back.
    let fold = Rect::new(outer.right() - 26.0, outer.y + 3.0, 20.0, 18.0);
    let res = ui.interact(id("minimap-hide", 0), fold, true);
    ui.fill(fold, rgb(palette::TEXT, 0.15 * res.glow));
    ui.hline(fold.x + 5.0, fold.mid_y(), 10.0, rgb(palette::TEXT, 0.7 + 0.3 * res.glow));
    if res.clicked {
        ui.audio.play(crate::audio::Sfx::Tick);
        hud.minimap_hidden = true;
    }

    ui.fill(chart, ink(0.9));
    ui.image(
        MINIMAP_SLOT,
        [0.0, 0.0, size, size],
        chart,
        [0.82, 0.82, 0.82, 1.0],
    );

    // Ore fields are baked into the chart image (`ui::preview`); the ones a
    // mine the viewer has seen is working get a bright materials outline.
    let tapped = super::ore_tapped(s.map, s.blueprints, &view.frame.units);
    for (region, _) in s.map.ore_regions().iter().zip(&tapped).filter(|(_, t)| **t) {
        let pts: Vec<Vec2> = region
            .points
            .iter()
            .map(|p| chart_pos(s, chart, Vec2::from(p.to_f32())))
            .collect();
        for (i, &a) in pts.iter().enumerate() {
            let b = pts[(i + 1) % pts.len()];
            ui.stroke(a, b, 1.6, rgb(super::MASS, 1.0));
        }
    }
    super::survival::minimap(hud, ui, s, &|p| chart_pos(s, chart, p));

    // Every mine in sight with its territory: faint always, bright during the
    // survey (placing or selecting a mine, or Ctrl).
    let survey = matches!(view.mode, crate::game::Mode::Place(bp) if s.blueprints.unit(bp).mine.is_some())
        || s.show_reclaim
        || view
            .frame
            .units
            .iter()
            .any(|u| view.selection.contains(&u.unit_id) && s.bp(u).mine.is_some());
    let mines = super::mines_in_sight(s.blueprints, &view.frame.units);
    let all: Vec<(Vec2, f32)> = mines.iter().map(|&(p, r, _)| (p, r)).collect();
    let (line, fill) = if survey { (0.95, 0.22) } else { (0.45, 0.08) };
    for (i, &(centre, reach, _)) in mines.iter().enumerate() {
        // Every panel and dropdown is drawn after the minimap: an 8-player
        // match's hundred-odd territories must never use up their vertices.
        if ui.o.vertices.len() > mc_render::overlay::MAX_OVERLAY_VERTICES / 3 {
            break;
        }
        let c = chart_pos(s, chart, centre);
        // A territory is a few pixels across on the chart: as many points as it has pixels round.
        let px = chart_pos(s, chart, centre + Vec2::new(reach, 0.0)).x - c.x;
        let points = (px * std::f32::consts::TAU / 3.0).clamp(10.0, 80.0) as usize;
        let others: Vec<(Vec2, f32)> = all
            .iter()
            .enumerate()
            .filter(|&(j, &(p, r))| j != i && p.distance(centre) < reach + r)
            .map(|(_, &o)| o)
            .collect();
        let pts: Vec<Vec2> = super::territory(centre, reach, &others)
            .into_iter()
            .step_by(super::TERRITORY_SEGMENTS.div_ceil(points))
            .map(|p| chart_pos(s, chart, p))
            .collect();
        for pair in pts.windows(2) {
            ui.triangle(c, pair[0], pair[1], rgb(super::MASS, fill));
            ui.stroke(pair[0], pair[1], 1.2, rgb(super::MASS, line));
        }
        if let (Some(&a), Some(&b)) = (pts.last(), pts.first()) {
            ui.stroke(a, b, 1.2, rgb(super::MASS, line));
        }
        ui.disc(c, 2.6, rgb(super::MASS, 1.0));
    }

    // Reach: the side's radar cover and shield domes, the selection's guns and eyes.
    let metre = chart_pos(s, chart, Vec2::new(1000.0, 0.0)).x - chart_pos(s, chart, Vec2::ZERO).x;
    let metre = metre / 1000.0;
    let reach_marks = view.frame.units.iter().filter(|u| {
        (u.owner_flags & 0xFF) as u8 == view.local
            && u.owner_flags & KIND_WRECK == 0
            && !has_flag(u, flag::UNDER_CONSTRUCTION | flag::IN_FACTORY)
    });
    for u in reach_marks.take(400) {
        let bp = s.bp(u);
        let c = chart_pos(s, chart, Vec2::new(u.pos[0], u.pos[1]));
        if bp.radar.to_f32() > 0.0 {
            ring(ui, chart, c, bp.radar.to_f32() * metre, rgb(0x78E08A, 0.55));
        }
        if let Some(sh) = bp.shield.filter(|sh| !sh.is_hull()) {
            ring(ui, chart, c, sh.radius.to_f32() * metre, rgb(super::style::AIR, 0.6));
        }
    }
    for u in view
        .selection
        .iter()
        .take(60)
        .filter_map(|id| view.index_of.get(id))
        .map(|&i| &view.frame.units[i])
    {
        let bp = s.bp(u);
        let c = chart_pos(s, chart, Vec2::new(u.pos[0], u.pos[1]));
        let range = bp.max_weapon_range().to_f32();
        if range > 0.0 {
            ring(ui, chart, c, range * metre, rgb(super::style::Family::Combat.tone(), 0.75));
        }
        if bp.vision.to_f32() > 0.0 {
            ring(ui, chart, c, bp.vision.to_f32() * metre, rgb(0xFFFFFF, 0.3));
        }
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
    if res.hovered && ui.input.right_pressed && !view.observing {
        hud.actions.push(HudAction::OrderAt(at));
    }
}

/// A circle on the chart, cut to its edge; too small to read is left off.
fn ring(ui: &mut Ui, chart: Rect, c: Vec2, radius: f32, color: crate::ui::Color) {
    if radius < 2.0 {
        return;
    }
    let n = ((radius * 0.8) as usize).clamp(16, 64);
    let at = |i: usize| c + Vec2::from_angle(i as f32 / n as f32 * std::f32::consts::TAU) * radius;
    for i in 0..n {
        if let Some((a, b)) = clip(at(i), at(i + 1), chart) {
            ui.stroke(a, b, 1.0, color);
        }
    }
}

fn chart_pos(s: &Scene, chart: Rect, world: Vec2) -> Vec2 {
    Vec2::new(chart.x, chart.y) + preview::locate(s.map, world.into(), chart.w)
}
