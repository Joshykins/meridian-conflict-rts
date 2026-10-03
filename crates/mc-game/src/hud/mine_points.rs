//! The map's mine points on the battlefield while they matter: placing a mine, a
//! mine or a builder with a mine queued selected, or Control held. Each point in
//! view is a ring on the ground: free (the materials red-orange, turning slowly),
//! taken (its mine's side colour) or planned by the viewer's builders (grey,
//! marching). Over the point a mine being placed would go on: what it makes and
//! how soon it pays for itself.

use super::{overview_height, Scene, MASS};
use crate::game::{Mode, View};
use crate::ui::{palette, rgb, type_scale, Rect, Ui};
use glam::Vec2;
use mc_core::FxVec2;
use mc_data::{BlueprintId, Blueprints};
use mc_map::MapFile;
use mc_sim::mirror::{KIND_GHOST, KIND_WRECK};
use mc_sim::tables::OrderKind;
use std::f32::consts::TAU;

/// Metres from a point to its ring: just outside a mine's 3x3 lot.
const RING_M: f32 = 22.0;
/// What a mine has still to build: light grey.
const PLANNED: u32 = 0xC8C8C4;

/// Who has a mine point.
#[derive(Clone, Copy, PartialEq)]
enum Held {
    Free,
    /// A mine of this side stands on it.
    Taken(u8),
    /// The viewer's builders mean to put one there.
    Planned,
}

/// Whether the mine points are shown this frame.
fn shown(s: &Scene) -> bool {
    let placing = matches!(s.view.mode, Mode::Place(bp) if s.blueprints.unit(bp).mine.is_some());
    let selected = s
        .view
        .frame
        .units
        .iter()
        .any(|u| s.view.selection.contains(&u.unit_id) && s.bp(u).mine.is_some());
    // A builder with a mine still to start in its queue brings them up too.
    let ordering = s
        .view
        .status
        .queues
        .iter()
        .filter(|q| s.view.selection.contains(&q.unit_id))
        .flat_map(|q| &q.orders)
        .any(|o| o.kind == OrderKind::Build && s.blueprints.unit(o.blueprint).mine.is_some());
    placing || selected || ordering || s.show_reclaim
}

/// Each mine point, by who has it, in map order. Empty until the map's sites are known.
fn points(view: &View, blueprints: &Blueprints) -> Vec<(FxVec2, Held)> {
    let Some(sites) = view.sites.get() else {
        return Vec::new();
    };
    let is_mine = |b: u32| blueprints.unit(BlueprintId(b as u16)).mine.is_some();
    let mines: Vec<(Vec2, u8)> = view
        .frame
        .units
        .iter()
        .filter(|u| u.owner_flags & (KIND_WRECK | KIND_GHOST) == 0 && is_mine(u.blueprint))
        .map(|u| (Vec2::new(u.pos[0], u.pos[1]), (u.owner_flags & 0xFF) as u8))
        .collect();
    let planned: Vec<FxVec2> = view
        .status
        .queues
        .iter()
        .flat_map(|q| &q.orders)
        .filter(|o| o.kind == OrderKind::Build && blueprints.unit(o.blueprint).mine.is_some())
        .map(|o| o.at)
        .collect();
    sites
        .mine_points()
        .iter()
        .map(|&p| {
            let at = Vec2::from(p.to_f32());
            let held = match mines.iter().find(|(m, _)| m.distance(at) < 2.0) {
                Some(&(_, owner)) => Held::Taken(owner),
                None if planned.contains(&p) => Held::Planned,
                None => Held::Free,
            };
            (p, held)
        })
        .collect()
}

/// Draws the points in view, and the readout over the one a mine is being placed on.
pub(super) fn mine_points(ui: &mut Ui, s: &Scene) {
    let _t = mc_core::perf_span!("ui.mine_points");
    if !shown(s) {
        return;
    }
    let scale = ui.s;
    let viewport = s.camera.viewport / scale;
    let spin = ui.time * 0.25;
    let ghosts: Vec<Vec2> = s
        .view
        .frame
        .units
        .iter()
        .filter(|u| u.owner_flags & KIND_GHOST != 0)
        .map(|u| Vec2::new(u.pos[0], u.pos[1]))
        .collect();
    for (p, held) in points(s.view, s.blueprints) {
        let at = Vec2::from(p.to_f32());
        let ground = |xy: Vec2| {
            s.camera
                .project(xy.extend(overview_height(s.map, xy) + 1.5))
                .map(|q| q / scale)
        };
        let Some(middle) = ground(at) else {
            continue;
        };
        let margin = 60.0;
        if middle.x < -margin
            || middle.y < -margin
            || middle.x > viewport.x + margin
            || middle.y > viewport.y + margin
        {
            continue;
        }
        let (color, dashed) = match held {
            Held::Free => (rgb(MASS, 0.9), true),
            Held::Taken(owner) => {
                let c = s.team_color(owner);
                ([c[0], c[1], c[2], 0.75], false)
            }
            Held::Planned => (rgb(PLANNED, 0.85), true),
        };
        // Far out, a ring would be a speck: a small diamond on the point instead.
        let across = ground(at + Vec2::X * RING_M).map_or(0.0, |e| e.distance(middle));
        if across < 7.0 {
            let r = 4.0;
            let corners = [
                middle + Vec2::new(0.0, -r),
                middle + Vec2::new(r, 0.0),
                middle + Vec2::new(0.0, r),
                middle + Vec2::new(-r, 0.0),
            ];
            ui.polyline(&corners, 1.6, color, true);
            continue;
        }
        let segments = 40;
        let turn = match held {
            Held::Free => spin,
            Held::Planned => -spin * 2.0,
            Held::Taken(_) => 0.0,
        };
        let ring: Vec<Option<Vec2>> = (0..=segments)
            .map(|i| {
                ground(at + Vec2::from_angle(i as f32 / segments as f32 * TAU + turn) * RING_M)
            })
            .collect();
        let width = 2.0;
        for (i, pair) in ring.windows(2).enumerate() {
            if dashed && i % 2 == 1 {
                continue;
            }
            if let (Some(a), Some(b)) = (pair[0], pair[1]) {
                ui.stroke(a, b, width, color);
            }
        }
        // A remembered mine that may be gone: a faint ring inside the free one.
        if held == Held::Free && ghosts.iter().any(|g| g.distance(at) < 2.0) {
            ui.polyline(
                &[middle - Vec2::X * 3.0, middle + Vec2::X * 3.0],
                1.2,
                rgb(palette::DIM, 0.8),
                false,
            );
        }
    }
    placing_readout(ui, s);
}

/// Over the point a mine is being placed on: its output and how long its price
/// takes to earn back.
fn placing_readout(ui: &mut Ui, s: &Scene) {
    let (Mode::Place(bp), Some(site)) = (s.view.mode, s.placing) else {
        return;
    };
    let bp = s.blueprints.unit(bp);
    let Some(mine) = bp.mine else {
        return;
    };
    let on_point = s
        .view
        .sites
        .get()
        .is_some_and(|sites| sites.mine_points().contains(&site));
    if !on_point {
        return;
    }
    let at = Vec2::from(site.to_f32());
    let Some(p) = s
        .camera
        .project(at.extend(overview_height(s.map, at) + 1.5))
        .map(|q| q / ui.s)
    else {
        return;
    };
    let rate = mine.rate.to_f32();
    let value = format!("+{rate:.0}/s");
    let note = if rate > 0.0 {
        format!(
            "pays back in {}",
            super::mine::duration(bp.cost_mass.to_f32() / rate)
        )
    } else {
        String::new()
    };
    let w =
        26.0 + ui.text_width(type_scale::VALUE, &value) + ui.text_width(type_scale::MICRO, &note);
    let r = Rect::new(p.x - w * 0.5, p.y - 52.0, w, 22.0);
    ui.frost(r, 0.9);
    ui.fill(Rect::new(r.x, r.y, 2.0, r.h), rgb(MASS, 1.0));
    ui.text(
        r.x + 8.0,
        r.y + 4.0,
        type_scale::VALUE,
        rgb(MASS, 1.0),
        &value,
    );
    let after = r.x + 8.0 + ui.text_width(type_scale::VALUE, &value) + 6.0;
    ui.text(
        after,
        r.y + 6.0,
        type_scale::MICRO,
        rgb(palette::TEXT, 0.9),
        &note,
    );
    ui.stroke(Vec2::new(p.x, r.bottom()), p, 1.4, rgb(MASS, 0.8));
}

/// Which ore fields, in map order, have a mine the viewer sees on their point:
/// the renderer lights those fields as worked.
pub fn ore_tapped(view: &View, blueprints: &Blueprints, map: &MapFile) -> Vec<bool> {
    let taken: Vec<FxVec2> = points(view, blueprints)
        .into_iter()
        .filter(|(_, h)| matches!(h, Held::Taken(_)))
        .map(|(p, _)| p)
        .collect();
    // A point may stand a little off its field when the middle would not take a mine.
    let near = mc_core::Fx::from_int(120);
    map.ore_regions()
        .iter()
        .map(|r| {
            taken
                .iter()
                .any(|&p| r.contains(p) || p.distance(r.centre()) < near)
        })
        .collect()
}

/// The mine points on the minimap while they are shown: free ones hollow in the
/// materials red-orange, taken ones filled in their side's colour.
pub(super) fn chart(ui: &mut Ui, s: &Scene, at: &dyn Fn(Vec2) -> Vec2) {
    if !shown(s) {
        return;
    }
    for (p, held) in points(s.view, s.blueprints) {
        let c = at(Vec2::from(p.to_f32()));
        let hollow = |ui: &mut Ui, color| {
            let ring: Vec<Vec2> = (0..10)
                .map(|i| c + Vec2::from_angle(i as f32 / 10.0 * TAU) * 2.6)
                .collect();
            ui.polyline(&ring, 1.0, color, true);
        };
        match held {
            Held::Free => hollow(ui, rgb(MASS, 1.0)),
            Held::Planned => hollow(ui, rgb(PLANNED, 1.0)),
            Held::Taken(owner) => ui.dot(c, 2.6, s.team_color(owner)),
        }
    }
}
