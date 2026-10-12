//! Lots on the ground while placing: their outlines, the build grid round the
//! pointer, and the sites a place-drag lays in a line. Every lot is turned the way its
//! structure faces (`UnitBlueprint::footprint_at`). Moved out of `orders.rs`.

use super::{
    site_verdict, surface_height, surface_under, wall_line, Field, MAX_DRAG_SITES, MAX_GHOSTS,
};
use crate::prefabs::Laid;
use crate::ui::{self, palette, Ui};
use glam::Vec2;
use mc_core::{Angle, Fx, FxVec2};
use mc_data::{BlueprintId, Blueprints};
use mc_sim::mirror::{UnitInstance, KIND_GHOST, KIND_PROP, KIND_WRECK};
use mc_sim::placement::Unfit;

/// Half a lot in metres, its structure turned to `heading` (radians, as drawn).
pub(super) fn half_lot(blueprints: &Blueprints, blueprint: BlueprintId, heading: f32) -> Vec2 {
    let f = blueprints
        .unit(blueprint)
        .footprint_at(Angle::from_radians_f32(heading));
    Vec2::new(f.0 as f32, f.1 as f32) * (mc_map::BUILD_CELL_M as f32 * 0.5)
}

/// Where the build grid is drawn while a structure is placed or a plan is in hand:
/// around the pointer, reaching further as the camera pulls back, with the lots
/// already taken near it (structures standing or begun, and plans), nearest first.
pub fn build_grid_focus(
    field: &Field,
    cursor: Vec2,
    in_hand: Option<(FxVec2, BlueprintId)>,
) -> Option<(Vec2, f32, Vec<[f32; 4]>)> {
    let centre = surface_under(field, cursor)?.truncate();
    let radius = (field.camera.distance * 0.3).clamp(60.0, 480.0);
    let mut lots: Vec<(f32, [f32; 4])> = Vec::new();
    let mut lot = |c: Vec2, blueprint: BlueprintId, heading: f32| {
        let half = half_lot(field.blueprints, blueprint, heading);
        let gap = ((c - centre).abs() - half).max(Vec2::ZERO).length();
        if gap < radius {
            let (lo, hi) = (c - half, c + half);
            lots.push((gap, [lo.x, lo.y, hi.x, hi.y]));
        }
    };
    for u in &field.view.frame.units {
        let blueprint = BlueprintId(u.blueprint as u16);
        if u.owner_flags & (KIND_WRECK | KIND_GHOST | KIND_PROP) == 0
            && field.blueprints.unit(blueprint).is_structure()
        {
            lot(Vec2::new(u.pos[0], u.pos[1]), blueprint, u.heading);
        }
    }
    for plan in &field.view.status.plans {
        if in_hand != Some((plan.at, plan.blueprint)) {
            lot(Vec2::from(plan.pos), plan.blueprint, plan.heading);
        }
    }
    lots.sort_by(|a, b| a.0.total_cmp(&b.0));
    Some((centre, radius, lots.into_iter().map(|(_, r)| r).collect()))
}

/// The lot outline on every structure preview: orange where it can go, red where it cannot.
pub fn ghost_footprints(ui: &mut Ui, field: &Field, ghosts: &[UnitInstance]) {
    let scale = ui.s;
    let project = |q: Vec2| {
        field
            .camera
            .project(q.extend(surface_height(field, q) + 1.0))
            .filter(|p| p.abs().max_element() < 20_000.0)
            .map(|p| p / scale)
    };
    for g in ghosts.iter().take(MAX_GHOSTS) {
        let blueprint = BlueprintId(g.blueprint as u16);
        if g.owner_flags & KIND_GHOST == 0 || !field.blueprints.unit(blueprint).built_on_site() {
            continue;
        }
        let tone = if g.health > 0.0 {
            palette::WARN
        } else {
            palette::BAD
        };
        let half = half_lot(field.blueprints, blueprint, g.heading);
        let c = Vec2::new(g.pos[0], g.pos[1]);
        // A lot off the screen draws nothing: a big base's plans would otherwise
        // spend the overlay on grids no one sees.
        if !on_screen(&project, c, half, field.camera.viewport / scale) {
            continue;
        }
        if !lot_marker(ui, &project, c, half, g.health > 0.0, tone) {
            footprint_grid(ui, &project, c, half, tone);
        }
    }
}

/// Whether any of the lot at `c` shows in a screen `size` points across.
fn on_screen(project: &dyn Fn(Vec2) -> Option<Vec2>, c: Vec2, half: Vec2, size: Vec2) -> bool {
    let corners: Vec<Vec2> = [
        Vec2::new(-1.0, -1.0),
        Vec2::new(1.0, -1.0),
        Vec2::new(1.0, 1.0),
        Vec2::new(-1.0, 1.0),
    ]
    .iter()
    .filter_map(|&k| project(c + half * k))
    .collect();
    let Some(&first) = corners.first() else {
        return false;
    };
    let (lo, hi) = corners
        .iter()
        .fold((first, first), |(lo, hi), &p| (lo.min(p), hi.max(p)));
    // The far-off mark is drawn at least this big round the centre.
    let pad = Vec2::splat(16.0);
    (hi + pad).cmpge(Vec2::ZERO).all() && (lo - pad).cmple(size).all()
}

/// Seen from far off a lot is a few pixels across, and its grid says nothing:
/// there it gets a mark of a readable size, with a cross where it cannot go.
/// True when the mark stands in for the grid.
fn lot_marker(
    ui: &mut Ui,
    project: &dyn Fn(Vec2) -> Option<Vec2>,
    c: Vec2,
    half: Vec2,
    fits: bool,
    tone: u32,
) -> bool {
    const LEGIBLE: f32 = 26.0;
    let Some(centre) = project(c) else {
        return false;
    };
    let across = [
        Vec2::new(-1.0, -1.0),
        Vec2::new(1.0, -1.0),
        Vec2::new(1.0, 1.0),
        Vec2::new(-1.0, 1.0),
    ]
    .iter()
    .filter_map(|&k| project(c + half * k))
    .map(|p| (p - centre).abs().max_element() * 2.0)
    .fold(0.0, f32::max);
    // Fades in as the lot itself shrinks past legible.
    let k = ((LEGIBLE - across) / (LEGIBLE * 0.5)).clamp(0.0, 1.0);
    if k <= 0.0 {
        return false;
    }
    let side = LEGIBLE.max(across);
    let r = ui::Rect::new(centre.x - side * 0.5, centre.y - side * 0.5, side, side);
    ui.fill(r, ui::rgb(tone, 0.18 * k));
    if fits {
        ui.brackets(r, side * 0.32, ui::rgb(tone, 0.95 * k));
    } else {
        ui.frame(r, ui::rgb(tone, 0.95 * k));
        let inset = side * 0.28;
        let (a, b) = (
            Vec2::new(r.x + inset, r.y + inset),
            Vec2::new(r.right() - inset, r.bottom() - inset),
        );
        ui.stroke(a, b, 2.0, ui::rgb(tone, k));
        ui.stroke(
            Vec2::new(a.x, b.y),
            Vec2::new(b.x, a.y),
            2.0,
            ui::rgb(tone, k),
        );
    }
    k >= 1.0
}

/// Centres a drag from `from` to `to` would occupy, spaced a footprint apart so
/// they sit edge to edge. One centre when the pointer has not moved a footprint,
/// and one for a structure that is never laid in a line (a core mine).
pub(super) fn line_centres(
    footprint: (u8, u8),
    single: bool,
    from: FxVec2,
    to: FxVec2,
) -> Vec<FxVec2> {
    let from = snap_footprint(footprint, from);
    let to = snap_footprint(footprint, to);
    if single || from == to {
        return vec![if single { to } else { from }];
    }
    let step_x = (footprint.0 as i32).max(1) * mc_map::BUILD_CELL_M;
    let step_y = (footprint.1 as i32).max(1) * mc_map::BUILD_CELL_M;
    let dx = (to.x - from.x).round_int();
    let dy = (to.y - from.y).round_int();
    let steps = (dx.abs() / step_x)
        .max(dy.abs() / step_y)
        .clamp(1, MAX_DRAG_SITES as i32 - 1);
    let mut out = Vec::with_capacity((steps + 1) as usize);
    out.push(from);
    for i in 1..=steps {
        let snapped = snap_footprint(
            footprint,
            FxVec2::from_ints(
                from.x.round_int() + dx * i / steps,
                from.y.round_int() + dy * i / steps,
            ),
        );
        if out.last() != Some(&snapped)
            && !out
                .iter()
                .any(|&p| footprints_overlap(footprint, p, snapped))
        {
            out.push(snapped);
        }
    }
    out
}

/// Sites a place-drag from `from` to `to` would put down, each turned to `heading`,
/// and whether each looks free.
pub fn drag_sites(
    field: &Field,
    blueprint: BlueprintId,
    heading: Angle,
    from: FxVec2,
    to: FxVec2,
) -> Vec<(FxVec2, Result<(), Unfit>)> {
    let bp = field.blueprints.unit(blueprint);
    let mut out = Vec::new();
    let mut taken = Vec::new();
    let centres = if bp.has(mc_data::cat::WALL) && bp.footprint == (1, 1) {
        wall_line::wall_line(from, to)
    } else {
        line_centres(bp.footprint_at(heading), bp.mine.is_some(), from, to)
    };
    for centre in centres {
        let xy = Vec2::from(centre.to_f32());
        let Some((pos, fit)) = site_verdict(
            field,
            blueprint,
            xy.extend(surface_height(field, xy)),
            heading,
            None,
            &taken,
        ) else {
            continue;
        };
        if out.iter().any(|(p, _)| *p == pos) {
            continue;
        }
        out.push((pos, fit));
        if fit.is_ok() {
            taken.push(Laid {
                blueprint,
                at: pos,
                heading,
            });
        }
    }
    out
}

pub(super) fn snap_footprint(footprint: (u8, u8), pos: FxVec2) -> FxVec2 {
    let cell = Fx::from_int(mc_map::BUILD_CELL_M);
    let snap = |v: Fx, cells: u8| {
        if cells % 2 == 1 {
            Fx::from_int((v / cell).floor_int()) * cell + cell / 2
        } else {
            Fx::from_int((v / cell).round_int()) * cell
        }
    };
    FxVec2::new(snap(pos.x, footprint.0), snap(pos.y, footprint.1))
}

pub(super) fn footprints_overlap(footprint: (u8, u8), a: FxVec2, b: FxVec2) -> bool {
    let reach = |n: u8| n as i32 * mc_map::BUILD_CELL_M;
    let d = a - b;
    d.x.abs().round_int() < reach(footprint.0) && d.y.abs().round_int() < reach(footprint.1)
}

/// A structure's lot on the ground: its build cells, and a firmer line round the edge.
fn footprint_grid(
    ui: &mut Ui,
    project: &dyn Fn(Vec2) -> Option<Vec2>,
    c: Vec2,
    half: Vec2,
    tone: u32,
) {
    let cell = mc_map::BUILD_CELL_M as f32;
    let cells = (half * 2.0 / cell).round().max(Vec2::ONE);
    let (nx, ny) = (cells.x as usize, cells.y as usize);
    let corner = c - half;
    let at = |i: usize, j: usize| project(corner + Vec2::new(i as f32, j as f32) * cell);
    // Each line goes a cell at a time, so it follows the ground.
    for i in 0..=nx {
        let edge = i == 0 || i == nx;
        for j in 0..ny {
            if let (Some(a), Some(b)) = (at(i, j), at(i, j + 1)) {
                ui.stroke(
                    a,
                    b,
                    if edge { 2.0 } else { 1.0 },
                    ui::rgb(tone, if edge { 0.95 } else { 0.45 }),
                );
            }
        }
    }
    for j in 0..=ny {
        let edge = j == 0 || j == ny;
        for i in 0..nx {
            if let (Some(a), Some(b)) = (at(i, j), at(i + 1, j)) {
                ui.stroke(
                    a,
                    b,
                    if edge { 2.0 } else { 1.0 },
                    ui::rgb(tone, if edge { 0.95 } else { 0.45 }),
                );
            }
        }
    }
}
