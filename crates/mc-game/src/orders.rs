//! Orders on the battlefield: a line through each unit's waypoints, a ghost for
//! every structure that is planned and not begun, and, with shift held, either
//! of them picked up and dragged somewhere else. Placement also lives here: where
//! a structure would stand, and the line of sites a place-drag occupies.
//!
//! Without shift it is the selection's orders that show; with it, the whole
//! side's. A drag leaves as one `Command::RelocateOrder` for every unit that
//! holds the order, so a group's shared waypoint moves for the whole group.

use crate::game::{Mode, View};
use crate::hud;
use crate::ui::{self, palette, Ui};
use glam::{Vec2, Vec3};
use mc_core::{Fx, FxVec2};
use mc_data::{BlueprintId, Blueprints};
use mc_map::MapFile;
use mc_render::{Camera, Renderer};
use mc_sim::command::{MAX_COMMAND_UNITS, MAX_ORBIT_RADIUS, MIN_ORBIT_RADIUS};
use mc_sim::mirror::{UnitInstance, KIND_GHOST, KIND_PROP, KIND_WRECK, STATE_RADAR};
use mc_sim::placement::Unfit;
use mc_sim::tables::OrderKind;
use mc_sim::{Command, Handle};
use std::collections::HashSet;

/// How near a waypoint a press has to be to pick it up, in pixels.
const GRAB_REACH: f32 = 20.0;
/// Most structures one place-drag may put down.
pub const MAX_DRAG_SITES: usize = 64;
/// The overlay's vertex budget is shared with the HUD: a line costs 18 vertices, a waypoint about 200.
const MAX_LINES: usize = 900;
const MAX_NODES: usize = 120;
const MAX_GHOSTS: usize = 256;
const MAX_AIR_GUIDES: usize = 256;
/// A dropped order is shown where it was dropped for this many ticks, while the
/// sim has yet to say so itself. Longer than any command delay; after it, the sim refused.
const SETTLE_TICKS: u32 = 10;

/// What the orders are read from and projected with.
pub struct Field<'a> {
    pub view: &'a View,
    pub blueprints: &'a Blueprints,
    pub map: &'a MapFile,
    pub camera: &'a Camera,
    pub renderer: &'a Renderer,
}

/// Names an order the way the sim finds it again: by kind and exact position.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Key {
    kind: OrderKind,
    at: FxVec2,
    /// What a `Build` makes; zero otherwise.
    blueprint: BlueprintId,
}

/// An order picked up with the pointer.
struct Grip {
    key: Key,
    /// Where the pointer was when it took hold, in pixels.
    pressed_at: Vec2,
    /// Everyone who holds it.
    units: Vec<u32>,
}

/// How a drag ended.
pub enum Dropped {
    Moved(Command),
    /// Put down where it was: the press was a click.
    Stayed,
    /// Not there: off the map, or a structure that does not fit.
    Refused,
}

#[derive(Default)]
pub struct OrderMap {
    drag: Option<Grip>,
    /// Where the drag would put the order down, and whether it may.
    aim: Option<(FxVec2, bool)>,
    /// The order a press would pick up, and where it is drawn.
    hover: Option<(Key, Vec2)>,
    /// Orders dropped a moment ago: (order, where to, the tick it was dropped in).
    settling: Vec<(Key, FxVec2, u32)>,
    /// Pixels per interface point, as last drawn: badges are sized in points.
    scale: f32,
    /// Command groups as the player made them: formation -> members. A group outlives
    /// its orders; a member leaves it only when it is given another order.
    kept: std::collections::BTreeMap<u64, Vec<u32>>,
    /// Units given an order since the last frame: they leave their groups. Filled from
    /// `Game::send`, which only borrows the map.
    ordered: std::cell::RefCell<Vec<u32>>,
    /// (unit, group) pairs a unit has left for good, while its queue may still show the old one.
    left: HashSet<(u32, u64)>,
}

fn draggable(kind: OrderKind) -> bool {
    matches!(
        kind,
        OrderKind::Move
            | OrderKind::AttackMove
            | OrderKind::Build
            | OrderKind::Patrol
            | OrderKind::Orbit
            | OrderKind::Guard
    )
}

/// How far an orbit's centre may drift and still be the same orbit, metres: one round a
/// unit goes where the unit goes. As `mc_sim`'s `ORBIT_FOLLOW_SLACK`.
const ORBIT_SLACK: f32 = 64.0;

impl Key {
    /// `other` names this same order, allowing for an orbit's centre following its unit.
    fn names(&self, other: Key) -> bool {
        *self == other
            || (self.kind == OrderKind::Orbit
                && other.kind == OrderKind::Orbit
                && Vec2::from(self.at.to_f32()).distance(Vec2::from(other.at.to_f32())) <= ORBIT_SLACK)
    }
}

/// Patrol routes: the order card's movement colour.
const PATROL: u32 = 0x7FD0FF;
/// Orbits: a deeper blue than a patrol, so a circle and a loop read apart.
const ORBIT: u32 = 0x4C8DFF;

/// A structure's lot on the ground: its build cells, and a firmer line round the edge.
fn footprint_grid(ui: &mut Ui, project: &dyn Fn(Vec2) -> Option<Vec2>, c: Vec2, half: Vec2, tone: u32) {
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
                ui.stroke(a, b, if edge { 2.0 } else { 1.0 }, ui::rgb(tone, if edge { 0.95 } else { 0.45 }));
            }
        }
    }
    for j in 0..=ny {
        let edge = j == 0 || j == ny;
        for i in 0..nx {
            if let (Some(a), Some(b)) = (at(i, j), at(i + 1, j)) {
                ui.stroke(a, b, if edge { 2.0 } else { 1.0 }, ui::rgb(tone, if edge { 0.95 } else { 0.45 }));
            }
        }
    }
}

/// A ring on the ground around `c`, `radius` metres, dashed.
fn ground_ring(ui: &mut Ui, field: &Field, c: Vec2, radius: f32, color: [f32; 4]) {
    const SEGMENTS: usize = 48;
    let point = |i: usize| {
        let p = c + Vec2::from_angle(i as f32 / SEGMENTS as f32 * std::f32::consts::TAU) * radius;
        field
            .camera
            .project(p.extend(field.renderer.surface_height(p) + 1.0))
    };
    for i in (0..SEGMENTS).step_by(2) {
        if let (Some(a), Some(b)) = (point(i), point(i + 1)) {
            ui.stroke(a / ui.s, b / ui.s, 1.6, color);
        }
    }
}

fn to_fx(p: Vec2) -> FxVec2 {
    FxVec2::new(Fx::from_f32(p.x), Fx::from_f32(p.y))
}
/// The edge of a guard area: a dashed ring on the ground, finer the bigger it is,
/// with a faint second ring just inside so it reads as a zone, not a range.
fn guard_ring(ui: &mut Ui, field: &Field, c: Vec2, radius: f32, strength: f32) {
    let segments = ((radius / 10.0) as usize).clamp(48, 192) & !1;
    let point = |i: usize, r: f32| {
        let p = c + Vec2::from_angle(i as f32 / segments as f32 * std::f32::consts::TAU) * r;
        field
            .camera
            .project(p.extend(field.renderer.surface_height(p) + 1.0))
    };
    let tone = hud::style::Family::Stance.tone();
    let inner = (radius - 6.0).max(radius * 0.97);
    for i in (0..segments).step_by(2) {
        if let (Some(a), Some(b)) = (point(i, radius), point(i + 1, radius)) {
            ui.stroke(a / ui.s, b / ui.s, 1.8, ui::rgb(tone, 0.85 * strength));
        }
        if let (Some(a), Some(b)) = (point(i, inner), point(i + 2, inner)) {
            ui.stroke(a / ui.s, b / ui.s, 1.0, ui::rgb(tone, 0.22 * strength));
        }
    }
}

fn half_footprint(blueprints: &Blueprints, blueprint: BlueprintId) -> Vec2 {
    let bp = blueprints.unit(blueprint);
    Vec2::new(bp.footprint.0 as f32, bp.footprint.1 as f32) * (mc_map::BUILD_CELL_M as f32 * 0.5)
}

/// Where `blueprint` would stand with the pointer on `ground`, and whether it looks
/// buildable there. `moving` is the site of the plan being dragged, when it is one.
/// The sim has the final say; this only drives the preview colour.
pub fn site(
    field: &Field,
    blueprint: BlueprintId,
    ground: Vec3,
    moving: Option<FxVec2>,
) -> Option<(FxVec2, bool)> {
    site_verdict(field, blueprint, ground, moving, &[]).map(|(pos, fit)| (pos, fit.is_ok()))
}

/// `site`, with why it will not do when it will not.
pub fn site_verdict(
    field: &Field,
    blueprint: BlueprintId,
    ground: Vec3,
    moving: Option<FxVec2>,
    taken: &[FxVec2],
) -> Option<(FxVec2, Result<(), Unfit>)> {
    let bp = field.blueprints.unit(blueprint);
    let pos = mc_sim::world::snap_to_build_grid(bp, to_fx(ground.truncate()));
    // The ground and the map's cities first, by the sim's own rules.
    if let Some(sites) = field.view.sites.get() {
        if let Err(why) = sites.check(bp, pos) {
            return Some((pos, Err(why)));
        }
    }
    let (pos, valid) = site_among(field, blueprint, ground, moving, taken)?;
    let why = if field.view.sites.get().is_some() { Unfit::Taken } else { Unfit::Water };
    Some((pos, if valid { Ok(()) } else { Err(why) }))
}

/// `site`, treating `taken` as structures already spoken for (the earlier
/// buildings of a place-drag). The ground itself is `site_verdict`'s, when
/// the map's sites are known.
fn site_among(
    field: &Field,
    blueprint: BlueprintId,
    ground: Vec3,
    moving: Option<FxVec2>,
    taken: &[FxVec2],
) -> Option<(FxVec2, bool)> {
    let Field {
        view,
        blueprints,
        map,
        ..
    } = field;
    let bp = blueprints.unit(blueprint);
    let mut valid = true;
    let pos = mc_sim::world::snap_to_build_grid(bp, to_fx(ground.truncate()));
    // Core mines may stand close: they share what they reach (the HUD shows it).
    if moving == Some(pos) {
        return Some((pos, true));
    }
    let half = half_footprint(blueprints, blueprint);
    let p = Vec2::from(pos.to_f32());
    let overlaps = |centre: Vec2, other: BlueprintId| {
        let d = (centre - p).abs();
        let reach = half + half_footprint(blueprints, other);
        d.x < reach.x && d.y < reach.y
    };
    for u in &view.frame.units {
        let other = BlueprintId(u.blueprint as u16);
        if u.owner_flags & KIND_WRECK == 0
            && blueprints.unit(other).is_structure()
            && overlaps(Vec2::new(u.pos[0], u.pos[1]), other)
        {
            valid = false;
        }
    }
    for plan in &view.status.plans {
        let same = plan.blueprint == blueprint && plan.at == pos;
        let selected = view.selection.contains(&plan.unit_id);
        let out_of_the_way = match moving {
            // The plan in hand; and the same plan in another queue, which it may join.
            Some(from) => same || (plan.at == from && plan.blueprint == blueprint),
            // An order that is not queued replaces the selection's plans; the same plan elsewhere can be joined.
            None => (selected && !view.shift) || (same && !selected),
        };
        if !out_of_the_way && overlaps(Vec2::from(plan.pos), plan.blueprint) {
            valid = false;
        }
    }
    for &centre in taken {
        if overlaps(Vec2::from(centre.to_f32()), blueprint) {
            valid = false;
        }
    }
    let water = map.info().water_level.to_f32();
    if view.sites.get().is_some() {
        // The ground was judged against the map, above.
    } else if !bp.water_build && field.renderer.ground_height(ground.truncate()) < water {
        valid = false;
    }
    // A naval yard wants open water under its whole lot: the middle and the corners.
    if bp.water_only() && view.sites.get().is_none() {
        let half = Vec2::new(bp.footprint.0 as f32, bp.footprint.1 as f32) * (mc_map::BUILD_CELL_M as f32 * 0.5);
        let at = Vec2::from(pos.to_f32());
        for corner in [Vec2::ZERO, half, -half, Vec2::new(half.x, -half.y), Vec2::new(-half.x, half.y)] {
            if field.renderer.ground_height(at + corner) >= water {
                valid = false;
            }
        }
    }
    Some((pos, valid))
}

/// Where a structure at `xy` stands: the ground, or the water's surface over the sea.
pub fn surface_height(field: &Field, xy: Vec2) -> f32 {
    field.renderer.surface_height(xy)
}

/// The point under `cursor` a structure would be placed at: the ground, or where
/// the ray meets the water's surface when the ground under it is seabed.
pub fn surface_under(field: &Field, cursor: Vec2) -> Option<Vec3> {
    let (origin, dir) = field.camera.ray(cursor);
    field.renderer.pick_surface(origin, dir)
}

/// Where the build grid is drawn while a structure is placed or a plan is in hand:
/// around the pointer, reaching further as the camera pulls back, with the lots
/// already taken near it (structures standing or begun, and plans), nearest first.
pub fn build_grid_focus(field: &Field, cursor: Vec2, in_hand: Option<(FxVec2, BlueprintId)>) -> Option<(Vec2, f32, Vec<[f32; 4]>)> {
    let centre = surface_under(field, cursor)?.truncate();
    let radius = (field.camera.distance * 0.3).clamp(60.0, 480.0);
    let mut lots: Vec<(f32, [f32; 4])> = Vec::new();
    let mut lot = |c: Vec2, blueprint: BlueprintId| {
        let half = half_footprint(field.blueprints, blueprint);
        let gap = ((c - centre).abs() - half).max(Vec2::ZERO).length();
        if gap < radius {
            let (lo, hi) = (c - half, c + half);
            lots.push((gap, [lo.x, lo.y, hi.x, hi.y]));
        }
    };
    for u in &field.view.frame.units {
        let blueprint = BlueprintId(u.blueprint as u16);
        if u.owner_flags & (KIND_WRECK | KIND_GHOST | KIND_PROP) == 0 && field.blueprints.unit(blueprint).is_structure() {
            lot(Vec2::new(u.pos[0], u.pos[1]), blueprint);
        }
    }
    for plan in &field.view.status.plans {
        if in_hand != Some((plan.at, plan.blueprint)) {
            lot(Vec2::from(plan.pos), plan.blueprint);
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
        let tone = if g.health > 0.0 { palette::WARN } else { palette::BAD };
        let half = half_footprint(field.blueprints, blueprint);
        let c = Vec2::new(g.pos[0], g.pos[1]);
        if !lot_marker(ui, &project, c, half, g.health > 0.0, tone) {
            footprint_grid(ui, &project, c, half, tone);
        }
    }
}

/// Seen from far off a lot is a few pixels across, and its grid says nothing:
/// there it gets a mark of a readable size, with a cross where it cannot go.
/// True when the mark stands in for the grid.
fn lot_marker(ui: &mut Ui, project: &dyn Fn(Vec2) -> Option<Vec2>, c: Vec2, half: Vec2, fits: bool, tone: u32) -> bool {
    const LEGIBLE: f32 = 26.0;
    let Some(centre) = project(c) else { return false };
    let across = [Vec2::new(-1.0, -1.0), Vec2::new(1.0, -1.0), Vec2::new(1.0, 1.0), Vec2::new(-1.0, 1.0)]
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
        let (a, b) = (Vec2::new(r.x + inset, r.y + inset), Vec2::new(r.right() - inset, r.bottom() - inset));
        ui.stroke(a, b, 2.0, ui::rgb(tone, k));
        ui.stroke(Vec2::new(a.x, b.y), Vec2::new(b.x, a.y), 2.0, ui::rgb(tone, k));
    }
    k >= 1.0
}

/// Centres a drag from `from` to `to` would occupy, spaced a footprint apart so
/// they sit edge to edge. One centre when the pointer has not moved a footprint,
/// and one for a structure that is never laid in a line (a core mine).
pub fn line_centres(
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

/// Sites a place-drag from `from` to `to` would put down, and whether each looks free.
pub fn drag_sites(
    field: &Field,
    blueprint: BlueprintId,
    from: FxVec2,
    to: FxVec2,
) -> Vec<(FxVec2, Result<(), Unfit>)> {
    let bp = field.blueprints.unit(blueprint);
    let mut out = Vec::new();
    let mut taken = Vec::new();
    for centre in line_centres(bp.footprint, bp.mine.is_some(), from, to) {
        let xy = Vec2::from(centre.to_f32());
        let Some((pos, fit)) = site_verdict(
            field,
            blueprint,
            xy.extend(surface_height(field, xy)),
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
            taken.push(pos);
        }
    }
    out
}

fn snap_footprint(footprint: (u8, u8), pos: FxVec2) -> FxVec2 {
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

fn footprints_overlap(footprint: (u8, u8), a: FxVec2, b: FxVec2) -> bool {
    let reach = |n: u8| n as i32 * mc_map::BUILD_CELL_M;
    let d = a - b;
    d.x.abs().round_int() < reach(footprint.0) && d.y.abs().round_int() < reach(footprint.1)
}

/// Where a patrol given now would loop back to: the middle of the selection, or, with
/// shift held, of where the selection's queues leave it.
pub fn patrol_start(view: &View) -> Option<Vec2> {
    let mut sum = Vec2::ZERO;
    let mut n = 0.0;
    for id in &view.selection {
        let queued = view
            .shift
            .then(|| view.status.queues.iter().find(|q| q.unit_id == *id))
            .flatten()
            .and_then(|q| {
                q.route()
                    .rev()
                    .find(|o| !matches!(o.kind, OrderKind::Produce | OrderKind::Upgrade))
            });
        let p = match queued {
            Some(o) if o.formation != 0 => Some(Vec2::from(o.pos)),
            Some(o) => Some(Vec2::from(o.pos) + Vec2::from(o.offset)),
            None => view
                .index_of
                .get(id)
                .map(|&i| Vec2::new(view.frame.units[i].pos[0], view.frame.units[i].pos[1])),
        };
        if let Some(p) = p {
            sum += p;
            n += 1.0;
        }
    }
    (n > 0.0).then(|| sum / n)
}

/// The legs of the selection's patrol loops, `(from, to)` post by post, each once
/// however many units fly it. Posts inserted but not yet in the queues are counted.
pub fn patrol_legs(view: &View) -> Vec<(FxVec2, FxVec2)> {
    let mut legs = Vec::new();
    for queue in view.status.queues.iter().filter(|q| view.selection.contains(&q.unit_id)) {
        let mut posts: Vec<FxVec2> = queue
            .route()
            .filter(|o| o.kind == OrderKind::Patrol)
            .map(|o| o.at)
            .collect();
        for &(after, point) in &view.patrol_inserts {
            if !posts.contains(&point) {
                if let Some(i) = posts.iter().position(|&p| p == after) {
                    posts.insert(i + 1, point);
                }
            }
        }
        if posts.len() < 2 {
            continue;
        }
        for i in 0..posts.len() {
            let leg = (posts[i], posts[(i + 1) % posts.len()]);
            if !legs.contains(&leg) {
                legs.push(leg);
            }
        }
    }
    legs
}

/// The patrol leg nearest `ground`: where a shift-click there would put a new post.
pub fn patrol_insert_leg(view: &View, ground: Vec2) -> Option<(FxVec2, FxVec2)> {
    let mut best: Option<(f32, (FxVec2, FxVec2))> = None;
    for leg in patrol_legs(view) {
        let (a, b) = (Vec2::from(leg.0.to_f32()), Vec2::from(leg.1.to_f32()));
        let ab = b - a;
        let t = if ab.length_squared() > 0.0 {
            ((ground - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let d = ground.distance(a + ab * t);
        if best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, leg));
        }
    }
    best.map(|(_, leg)| leg)
}

/// Height guides for the selected aircraft: a line down to the spot on the ground
/// each one is over. Radar-only contacts have no visible hull to anchor one to.
fn aircraft_guides(ui: &mut Ui, field: &Field, alpha: f32) {
    let viewport = field.camera.viewport;
    let on_screen = |p: Vec2| p.cmpge(Vec2::ZERO).all() && p.cmple(viewport).all();
    let mut drawn = 0;
    for unit in &field.view.frame.units {
        if drawn >= MAX_AIR_GUIDES {
            return;
        }
        if unit.owner_flags & (KIND_GHOST | KIND_PROP | KIND_WRECK | STATE_RADAR) != 0
            || !field
                .blueprints
                .unit(BlueprintId(unit.blueprint as u16))
                .motion
                .is_some_and(|m| m.layer == mc_data::MoveLayer::Air)
            || !field.view.selection.contains(&unit.unit_id)
        {
            continue;
        }
        let position = Vec3::from(unit.prev_pos).lerp(Vec3::from(unit.pos), alpha);
        let ground = position
            .truncate()
            .extend(field.renderer.surface_height(position.truncate()) + 1.0);
        if position.z <= ground.z {
            continue;
        }
        let (Some(top), Some(foot)) =
            (field.camera.project(position), field.camera.project(ground))
        else {
            continue;
        };
        if !(on_screen(top) || on_screen(foot))
            || top.abs().max_element() >= 20_000.0
            || foot.abs().max_element() >= 20_000.0
        {
            continue;
        }
        drawn += 1;
        let team = field.view.colors[(unit.owner_flags & 0xFF) as usize % 8];
        let color = [team[0], team[1], team[2], 0.7];
        ui.stroke(top / ui.s, foot / ui.s, 1.0, color);
        if on_screen(foot) {
            let c = foot / ui.s;
            ui.stroke(c - Vec2::X * 3.0, c + Vec2::X * 3.0, 1.0, color);
            ui.stroke(c - Vec2::Y * 3.0, c + Vec2::Y * 3.0, 1.0, color);
        }
    }
}

impl OrderMap {
    pub fn dragging(&self) -> bool {
        self.drag.is_some()
    }

    /// A planned structure is in hand.
    /// The plan in hand, by its site and blueprint.
    pub fn plan_in_hand(&self) -> Option<(FxVec2, BlueprintId)> {
        self.drag
            .as_ref()
            .filter(|grip| grip.key.kind == OrderKind::Build)
            .map(|grip| (grip.key.at, grip.key.blueprint))
    }

    pub fn dragging_plan(&self) -> bool {
        self.drag.as_ref().is_some_and(|grip| grip.key.kind == OrderKind::Build)
    }

    /// An order is in hand: whether it could be put down where the pointer is.
    pub fn in_hand(&self) -> Option<bool> {
        self.drag
            .as_ref()
            .map(|_| self.aim.is_some_and(|(_, valid)| valid))
    }

    /// The selection's patrol post nearest `cursor`, within reach: what a right-click takes out.
    pub fn patrol_post_at(&self, field: &Field, cursor: Vec2) -> Option<FxVec2> {
        let view = field.view;
        let mut best: Option<(f32, FxVec2)> = None;
        for order in view
            .status
            .queues
            .iter()
            .filter(|q| view.selection.contains(&q.unit_id))
            .flat_map(|q| q.route())
            .filter(|o| o.kind == OrderKind::Patrol)
        {
            let at = Vec2::from(order.pos);
            let Some(p) = field
                .camera
                .project(at.extend(field.renderer.surface_height(at) + 1.0))
            else {
                continue;
            };
            let d = p.distance(cursor);
            if d <= GRAB_REACH && best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, order.at));
            }
        }
        best.map(|(_, at)| at)
    }

    /// A route or a bombardment being laid out: the posts so far, joined up and
    /// run on to the pointer; the ring a bombardment or orbit drag would cover.
    pub fn draw_pending(&self, ui: &mut Ui, field: &Field, cursor: Vec2) {
        let view = field.view;
        let ground = Self::ground_under(field, cursor).map(|g| g.truncate());
        let scale = ui.s;
        let project = |p: Vec2| {
            field
                .camera
                .project(p.extend(field.renderer.surface_height(p) + 1.0))
                .map(|q| q / scale)
        };
        if view.mode == Mode::Target(crate::game::Targeting::Patrol) {
            // The posts already laid are live orders and drawn as such. Here: the leg the
            // next click would add, from the last post (or from the group) to the pointer,
            // and the way back from there to where the loop began.
            let start = view
                .patrol_posts
                .first()
                .map(|p| Vec2::from(p.to_f32()))
                .or_else(|| patrol_start(view));
            let last = view.patrol_posts.last().map(|p| Vec2::from(p.to_f32())).or(start);
            // With shift over a live patrol: the nearest leg, bent through the pointer.
            let leg = view
                .shift
                .then(|| ground.and_then(|g| patrol_insert_leg(view, g)))
                .flatten();
            if let (Some((from, to)), Some(g)) = (leg, ground) {
                let (from, to) = (Vec2::from(from.to_f32()), Vec2::from(to.to_f32()));
                if let (Some(a), Some(b), Some(c)) = (project(from), project(g), project(to)) {
                    flow(ui, a, b, PATROL, 0.9, Some(ui.time), false, 64);
                    flow(ui, b, c, PATROL, 0.9, Some(ui.time), false, 64);
                    waypoint(ui, b, PATROL, 0.9, true, ui.time);
                }
            } else if let (Some(from), Some(g)) = (last, ground) {
                if let (Some(a), Some(b)) = (project(from), project(g)) {
                    flow(ui, a, b, PATROL, 0.9, Some(ui.time), false, 64);
                    if view.patrol_posts.len() > 1 {
                        if let Some(s) = start.and_then(project) {
                            flow(ui, b, s, PATROL, 0.6, None, true, 64);
                        }
                    }
                    waypoint(ui, b, PATROL, 0.9, true, ui.time);
                }
            }
        }
        if view.mode == Mode::Target(crate::game::Targeting::Bombard) {
            if let (Some(centre), Some(g)) = (view.circle_from, ground) {
                let radius = centre.distance(g).clamp(30.0, 250.0);
                ground_ring(ui, field, centre, radius, ui::rgb(palette::BAD, 0.9));
                if let Some(c) = project(centre) {
                    ui.disc(c, 3.0, ui::rgb(palette::BAD, 1.0));
                }
            }
        }
        if view.mode == Mode::Target(crate::game::Targeting::Guard) {
            // Only once dragged: a click keeps the size the selection has.
            if let (Some(centre), Some(g)) = (view.circle_from, ground) {
                if centre.distance(g) >= 10.0 {
                    guard_ring(ui, field, centre, centre.distance(g).clamp(40.0, 2400.0), 1.0);
                }
                if let Some(c) = project(centre) {
                    ui.disc(c, 3.0, ui::rgb(hud::style::Family::Stance.tone(), 1.0));
                }
            }
        }
        if view.mode == Mode::Target(crate::game::Targeting::Orbit) {
            // Only once dragged: a click leaves each aircraft its own circle.
            if let (Some(centre), Some(g)) = (view.circle_from, ground) {
                if centre.distance(g) >= 10.0 {
                    let radius = centre.distance(g).clamp(MIN_ORBIT_RADIUS.to_f32(), MAX_ORBIT_RADIUS.to_f32());
                    orbit_ring(ui, &project, centre, radius, 0.9, Some(ui.time), MAX_LINES);
                }
            }
        }
    }

    /// With shift held, the pointer is over an order it could pick up.
    pub fn hovering(&self) -> bool {
        self.hover.is_some()
    }

    pub fn cancel(&mut self) {
        self.drag = None;
        self.aim = None;
    }

    /// Where an order is drawn: in hand, where it was just dropped, or where the sim has it.
    fn shown(&self, key: Key, pos: [f32; 2]) -> Vec2 {
        if let (Some(grip), Some((aim, _))) = (&self.drag, self.aim) {
            if grip.key.names(key) {
                return Vec2::from(aim.to_f32());
            }
        }
        match self.settling.iter().find(|(k, ..)| k.names(key)) {
            Some((_, to, _)) => Vec2::from(to.to_f32()),
            None => Vec2::from(pos),
        }
    }

    fn ground_under(field: &Field, cursor: Vec2) -> Option<Vec3> {
        let (origin, dir) = field.camera.ray(cursor);
        field.renderer.pick_surface(origin, dir)
    }

    /// The order a press at `cursor` would pick up: the nearest waypoint in reach,
    /// or else the planned structure the pointer is over. While a structure is
    /// being placed only plans count, where a click could not build anyway.
    fn pick(&self, field: &Field, cursor: Vec2) -> Option<(Key, Vec2)> {
        let Field {
            view,
            camera,
            renderer,
            ..
        } = field;
        let planned = |key: Key| {
            view.status
                .plans
                .iter()
                .any(|p| p.at == key.at && p.blueprint == key.blueprint)
        };
        let mut best: Option<(f32, Key, Vec2)> = None;
        if view.mode == Mode::Normal {
            for order in view
                .status
                .queues
                .iter()
                .flat_map(|q| q.route())
                .filter(|o| draggable(o.kind))
            {
                let key = Key {
                    kind: order.kind,
                    at: order.at,
                    blueprint: if order.kind == OrderKind::Build {
                        order.blueprint
                    } else {
                        BlueprintId(0)
                    },
                };
                // A structure that has been begun is on the map, not in the plans.
                if order.kind == OrderKind::Build && !planned(key) {
                    continue;
                }
                let at = self.shown(key, order.pos);
                let Some(p) = camera.project(at.extend(renderer.surface_height(at) + 1.0)) else {
                    continue;
                };
                let d = p.distance(cursor);
                if d <= GRAB_REACH && best.is_none_or(|(bd, ..)| d < bd) {
                    best = Some((d, key, at));
                }
            }
        }
        if best.is_none() {
            let ground = Self::ground_under(field, cursor)?.truncate();
            for plan in &view.status.plans {
                let key = Key {
                    kind: OrderKind::Build,
                    at: plan.at,
                    blueprint: plan.blueprint,
                };
                let at = self.shown(key, plan.pos);
                let (d, half) = (
                    (at - ground).abs(),
                    half_footprint(field.blueprints, plan.blueprint),
                );
                if d.x < half.x && d.y < half.y && best.is_none_or(|(bd, ..)| d.length() < bd) {
                    best = Some((d.length(), key, at));
                }
            }
        }
        best.map(|(_, key, at)| (key, at))
    }

    /// A left press with shift held: true when it picked an order up, and the press is spent.
    pub fn press(&mut self, field: &Field, cursor: Vec2) -> bool {
        if !matches!(field.view.mode, Mode::Normal | Mode::Place(_)) {
            return false;
        }
        let Some((key, _)) = self.pick(field, cursor) else {
            return false;
        };
        let status = &field.view.status;
        let mut units: Vec<u32> = status
            .queues
            .iter()
            .filter(|q| q.route().any(|o| o.kind == key.kind && o.at == key.at))
            .map(|q| q.unit_id)
            .collect();
        if key.kind == OrderKind::Build {
            units.extend(
                status
                    .plans
                    .iter()
                    .filter(|p| p.at == key.at && p.blueprint == key.blueprint)
                    .map(|p| p.unit_id),
            );
        }
        units.sort_unstable();
        units.dedup();
        units.truncate(MAX_COMMAND_UNITS);
        self.drag = Some(Grip {
            key,
            pressed_at: cursor,
            units,
        });
        self.aim = None;
        self.hover = None;
        true
    }

    /// The left button came up. `None` when nothing was in hand. `still` is how far the
    /// pointer may have strayed for the press to have been a click and not a drag.
    pub fn release(&mut self, tick: u32, cursor: Vec2, still: f32) -> Option<Dropped> {
        let grip = self.drag.take()?;
        Some(match self.aim.take() {
            _ if grip.pressed_at.distance(cursor) < still => Dropped::Stayed,
            Some((to, true)) if to == grip.key.at => Dropped::Stayed,
            Some((to, true)) => {
                self.settling.retain(|(k, ..)| *k != grip.key);
                self.settling.push((grip.key, to, tick));
                Dropped::Moved(Command::RelocateOrder {
                    units: grip.units.into_iter().map(Handle).collect(),
                    kind: grip.key.kind,
                    from: grip.key.at,
                    to,
                })
            }
            _ => Dropped::Refused,
        })
    }

    /// Once a frame, before anything is drawn: where the order in hand would go, what the
    /// pointer could pick up, and which dropped orders the sim has caught up with.
    pub fn update(&mut self, field: &Field, cursor: Vec2, over_ui: bool) {
        self.keep_groups(field.view);
        let status = &field.view.status;
        // An orbit in hand round a unit: keep up with where its centre has got to, so the
        // drop names it where it is now.
        if let Some(grip) = self.drag.as_mut().filter(|g| g.key.kind == OrderKind::Orbit) {
            let near = status
                .queues
                .iter()
                .filter(|q| grip.units.contains(&q.unit_id))
                .flat_map(|q| q.route())
                .filter(|o| grip.key.names(key_of(o)))
                .map(|o| o.at)
                .min_by(|a, b| {
                    let d = |p: &FxVec2| (*p - grip.key.at).length();
                    d(a).cmp(&d(b))
                });
            if let Some(at) = near {
                grip.key.at = at;
            }
        }
        let held = |key: &Key| {
            status.plans.iter().any(|p| {
                key.kind == OrderKind::Build && p.at == key.at && p.blueprint == key.blueprint
            }) || status
                .queues
                .iter()
                .flat_map(|q| q.route())
                .any(|o| key.names(key_of(o)))
        };
        self.settling
            .retain(|(key, _, tick)| status.tick <= tick + SETTLE_TICKS && held(key));
        // The order in hand was carried out, or cancelled, under the pointer.
        if self.drag.as_ref().is_some_and(|grip| !held(&grip.key)) {
            self.cancel();
        }
        self.hover = None;
        if let Some(grip) = &self.drag {
            self.aim = match grip.key.kind {
                OrderKind::Build => surface_under(field, cursor)
                    .and_then(|ground| site(field, grip.key.blueprint, ground, Some(grip.key.at))),
                _ => Self::ground_under(field, cursor).map(|ground| (to_fx(ground.truncate()), true)),
            };
        } else if field.view.shift && !over_ui {
            self.hover = self.pick(field, cursor);
        }
    }

    /// Ghosts of every planned structure of the side, so a planned site always shows.
    /// Several builders sharing a plan make one ghost. The ones in focus come first
    /// (the selection's, and all of them while shift is held, a structure is being
    /// placed or a plan is in hand), and how many of them is returned: those get
    /// the lot outline.
    pub fn ghosts(&self, field: &Field, out: &mut Vec<UnitInstance>) -> usize {
        let Field {
            view,
            blueprints,
            ..
        } = field;
        let everyone = view.shift || self.drag.is_some() || matches!(view.mode, Mode::Place(_));
        let in_focus = |plan: &mc_sim::mirror::PlannedBuild| everyone || view.selection.contains(&plan.unit_id);
        let mut seen: HashSet<(FxVec2, BlueprintId)> = HashSet::new();
        let focused = view.status.plans.iter().filter(|p| in_focus(p));
        let rest = view.status.plans.iter().filter(|p| !in_focus(p));
        let mut outlined = out.len();
        for (plan, focus) in focused.map(|p| (p, true)).chain(rest.map(|p| (p, false))) {
            if out.len() >= MAX_GHOSTS || !seen.insert((plan.at, plan.blueprint)) {
                continue;
            }
            if focus {
                outlined = out.len() + 1;
            }
            let key = Key {
                kind: OrderKind::Build,
                at: plan.at,
                blueprint: plan.blueprint,
            };
            let at = self.shown(key, plan.pos);
            let in_hand = self.drag.as_ref().is_some_and(|grip| grip.key == key);
            let valid = !in_hand || self.aim.is_some_and(|(_, valid)| valid);
            let p = [at.x, at.y, surface_height(field, at)];
            out.push(UnitInstance {
                prev_pos: p,
                prev_heading: plan.heading,
                pos: p,
                heading: plan.heading,
                blueprint: plan.blueprint.0 as u32,
                owner_flags: view.local as u32 | KIND_GHOST,
                health: if valid { 1.0 } else { 0.0 },
                build: 1.0,
                turret_yaw: 0.0,
                radius: blueprints.unit(plan.blueprint).radius.to_f32(),
                unit_id: u32::MAX,
                _pad: 0,
                gait: [0.0; 3],
                upgrade: 0.0,
                arm_pitch: [0.0; 4],
                prev_turret_yaw: 0.0,
                weld: [0.0; 3],
                recoil: 0.0,
                prev_recoil: 0.0,
                weld_first: 0,
                weld_count: 0,
                deploy: 0.0,
                prev_deploy: 0.0,
                _pad2: [0.0; 2],
                refit_modules: 0,
                _pad3: [0; 3],
                mount: [0.0; 4],
                spin_recoil: [0.0; 4],
            });
        }
        outlined
    }

    /// The ghost of the plan in hand, for its range rings.
    pub fn ghost_in_hand<'a>(&self, ghosts: &'a [UnitInstance]) -> Option<&'a UnitInstance> {
        let (grip, (aim, _)) = (self.drag.as_ref()?, self.aim?);
        let at = aim.to_f32();
        ghosts.iter().find(|g| {
            grip.key.kind == OrderKind::Build
                && g.blueprint == grip.key.blueprint.0 as u32
                && g.pos[0] == at[0]
                && g.pos[1] == at[1]
        })
    }

    /// These units were given an order that is not their group's: they leave it.
    pub fn ordered(&self, units: &[Handle]) {
        self.ordered.borrow_mut().extend(units.iter().map(|h| h.0));
    }

    /// Brings the kept groups up to date: a unit whose current order belongs to a group is
    /// in that group, and out of any other; one that was ordered elsewhere leaves; an idle
    /// one stays where it was. The dead, and groups of fewer than two, drop away.
    fn keep_groups(&mut self, view: &View) {
        for id in self.ordered.take() {
            for (&formation, members) in &mut self.kept {
                if members.contains(&id) {
                    members.retain(|m| *m != id);
                    self.left.insert((id, formation));
                }
            }
        }
        for queue in &view.status.queues {
            let Some(front) = queue.orders.first() else {
                continue;
            };
            let id = queue.unit_id;
            let joined = front.formation != 0
                && !matches!(front.kind, OrderKind::Produce | OrderKind::Upgrade)
                && !self.left.contains(&(id, front.formation));
            if joined {
                for (&formation, members) in &mut self.kept {
                    if formation != front.formation && members.contains(&id) {
                        members.retain(|m| *m != id);
                    }
                }
                let members = self.kept.entry(front.formation).or_default();
                if !members.contains(&id) {
                    members.push(id);
                }
            } else if front.formation == 0 {
                // Walking on its own now: out of every group.
                for members in self.kept.values_mut() {
                    members.retain(|m| *m != id);
                }
            }
        }
        for members in self.kept.values_mut() {
            members.retain(|id| view.index_of.contains_key(id));
        }
        self.kept.retain(|_, members| members.len() > 1);
        let live: HashSet<u64> = self.kept.keys().copied().collect();
        self.left.retain(|(id, formation)| live.contains(formation) && view.index_of.contains_key(id));
    }

    /// Each kept command group: the middle of its members (the world, interpolated) and who they are.
    fn groups(&self, field: &Field, alpha: f32) -> Vec<Group> {
        let view = field.view;
        self.kept
            .iter()
            .filter_map(|(&formation, members)| {
                let mut middle = Vec3::ZERO;
                let mut found = Vec::new();
                for id in members {
                    let Some(unit) = view.index_of.get(id).map(|&i| &view.frame.units[i]) else {
                        continue;
                    };
                    middle += Vec3::from(unit.prev_pos).lerp(Vec3::from(unit.pos), alpha);
                    found.push(*id);
                }
                (found.len() > 1).then(|| Group {
                    formation,
                    middle: middle / found.len() as f32,
                    selected: found.iter().any(|id| view.selection.contains(id)),
                    members: found,
                })
            })
            .collect()
    }

    /// The group whose count badge is under `cursor` (pixels): who a click there selects.
    pub fn group_at(&self, field: &Field, cursor: Vec2) -> Option<Vec<u32>> {
        let scale = self.scale.max(0.4);
        self.groups(field, 1.0)
            .into_iter()
            .filter(|g| g.members.len() > 1)
            .filter_map(|g| {
                let p = field.camera.project(g.middle)?;
                let d = p.distance(cursor) / scale;
                (d <= badge_radius(g.members.len()) + 3.0).then_some((d, g.members))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, members)| members)
    }

    /// The order lines, one for each unit walking on its own and one for each group,
    /// through the waypoints: the selection's, and with shift held the whole side's.
    /// Every command group wears the count of its members in its middle, always.
    pub fn draw(&mut self, ui: &mut Ui, field: &Field, alpha: f32) {
        self.scale = ui.s;
        aircraft_guides(ui, field, alpha);
        let Field {
            view,
            camera,
            renderer,
            ..
        } = field;
        let viewport = camera.viewport;
        let on_screen =
            |p: Vec2| p.cmpge(Vec2::splat(-40.0)).all() && p.cmple(viewport + 40.0).all();
        let sane = |p: Vec2| p.abs().max_element() < 20_000.0;
        let ground = |p: Vec2| camera.project(p.extend(renderer.surface_height(p) + 1.0));
        let t = ui.time;
        let groups = self.groups(field, alpha);
        let mut budget = MAX_LINES;
        let mut nodes = MAX_NODES;
        // Legs and waypoints several units share are drawn once.
        let mut legs: HashSet<(Origin, u8, FxVec2)> = HashSet::new();
        let mut drawn: HashSet<(u8, FxVec2)> = HashSet::new();
        let mut markers: Vec<(Vec2, u32, f32, bool)> = Vec::new();
        // Orbits: centre and radius in world metres, strength, lively.
        let mut rings: Vec<(Vec2, f32, f32, bool)> = Vec::new();
        let mut guards: Vec<(Vec2, f32, f32)> = Vec::new();
        let mut guarded: HashSet<(i64, i64)> = HashSet::new();
        for queue in &view.status.queues {
            let Some(unit) = view
                .index_of
                .get(&queue.unit_id)
                .map(|&i| &view.frame.units[i])
            else {
                continue;
            };
            let selected = view.selection.contains(&queue.unit_id);
            if !selected && !view.shift {
                continue;
            }
            let strength = match (selected, view.shift) {
                (true, true) => 1.0,
                (true, false) => 0.8,
                (false, true) => 0.6,
                (false, false) => 0.35,
            };
            let lively = selected || view.shift;
            // A group's line leaves from the middle of the group, a lone unit's from the unit.
            let (mut origin, start) = match queue.orders.first() {
                Some(o) if o.formation != 0 => match groups.iter().find(|g| g.formation == o.formation) {
                    Some(g) => (Origin::Group(o.formation), g.middle),
                    None => continue,
                },
                _ => (
                    Origin::Unit(queue.unit_id),
                    Vec3::from(unit.prev_pos).lerp(Vec3::from(unit.pos), alpha),
                ),
            };
            let mut from = camera.project(start);
            // Where the line so far ends, world metres: an orbit's line stops at its circle.
            let mut from_world = start.truncate();
            // A patrol is a loop: its last post runs back to its first.
            let posts: Vec<(FxVec2, Vec2)> = queue
                .route()
                .filter(|o| o.kind == OrderKind::Patrol)
                .map(|o| (o.at, self.shown(key_of(o), o.pos)))
                .collect();
            if posts.len() > 1 {
                let ((last_at, a), (first_at, b)) = (posts[posts.len() - 1], posts[0]);
                if legs.insert((Origin::Post(OrderKind::Patrol as u8, last_at), OrderKind::Patrol as u8, first_at)) {
                    if let (Some(a), Some(b)) = (ground(a), ground(b)) {
                        if (on_screen(a) || on_screen(b)) && sane(a) && sane(b) {
                            budget = budget.saturating_sub(flow(
                                ui,
                                a / ui.s,
                                b / ui.s,
                                PATROL,
                                strength * 0.7,
                                lively.then_some(t),
                                true,
                                budget,
                            ));
                        }
                    }
                }
            }
            // Production and refits hold up what is queued behind them; a factory's
            // standing orders are its products' way out.
            let route = queue
                .orders
                .iter()
                .take_while(|o| !matches!(o.kind, OrderKind::Produce | OrderKind::Upgrade))
                .chain(&queue.standing);
            for order in route.take(24) {
                let tone = tone_of(order.kind);
                let at = if draggable(order.kind) {
                    self.shown(key_of(order), order.pos)
                } else {
                    Vec2::from(order.pos)
                };
                // A group is sent to one point; where each member stands there is the group's business.
                let destination = if order.kind == OrderKind::Orbit && order.radius > 0.0 {
                    let d = from_world - at;
                    let len = d.length();
                    if len > 1.0 {
                        at + d / len * order.radius.min(len)
                    } else {
                        from_world
                    }
                } else if order.formation != 0 {
                    at
                } else {
                    at + Vec2::from(order.offset)
                };
                let to = ground(destination);
                if legs.insert((origin, order.kind as u8, order.at)) {
                    if let (Some(a), Some(b)) = (from, to) {
                        // Far off-screen ends would make for enormous quads; the line is not worth it.
                        let visible = on_screen(a)
                            || on_screen(b)
                            || ((a.x < 0.0) != (b.x < 0.0) || (a.y < 0.0) != (b.y < 0.0));
                        if visible && sane(a) && sane(b) && budget > 0 {
                            budget = budget.saturating_sub(flow(
                                ui,
                                a / ui.s,
                                b / ui.s,
                                tone,
                                strength,
                                lively.then_some(t),
                                false,
                                budget,
                            ));
                        }
                    }
                }
                if order.kind == OrderKind::Guard
                    && order.radius > 0.0
                    && guarded.insert((order.at.x.0, order.at.y.0))
                {
                    guards.push((at, order.radius, strength));
                }
                if order.kind == OrderKind::Orbit {
                    if order.radius > 0.0 && drawn.insert((order.kind as u8, order.at)) {
                        rings.push((at, order.radius, strength, lively));
                    }
                } else if let Some(b) = ground(at).filter(|b| {
                    on_screen(*b) && nodes > 0 && drawn.insert((order.kind as u8, order.at))
                }) {
                    nodes -= 1;
                    markers.push((b / ui.s, tone, strength, selected));
                }
                origin = if order.formation != 0 {
                    Origin::Post(order.kind as u8, order.at)
                } else {
                    Origin::Unit(queue.unit_id)
                };
                from = to;
                from_world = destination;
            }
        }
        let scale = ui.s;
        let project = |p: Vec2| ground(p).filter(|q| sane(*q)).map(|q| q / scale);
        for (c, radius, strength, lively) in rings {
            budget = budget.saturating_sub(orbit_ring(ui, &project, c, radius, strength, lively.then_some(t), budget));
        }
        for (c, radius, strength) in guards {
            guard_ring(ui, field, c, radius, strength);
        }
        // Waypoints over the lines, and the groups' badges over those.
        for (c, tone, strength, selected) in markers {
            waypoint(ui, c, tone, strength, selected, t);
        }
        for g in groups.iter().filter(|g| g.members.len() > 1) {
            let Some(p) = camera.project(g.middle).filter(|p| on_screen(*p)) else {
                continue;
            };
            let c = p / ui.s;
            let r = badge_radius(g.members.len());
            let hovered = !self.dragging() && ui.cursor.distance(c) <= r + 3.0;
            badge(ui, c, g.members.len(), g.selected || view.shift, hovered, t);
        }
        // What a press would pick up, or what is in hand.
        let held = self
            .drag
            .as_ref()
            .zip(self.aim)
            .map(|(grip, (aim, valid))| (grip.key, Vec2::from(aim.to_f32()), valid));
        if let Some((key, at, valid)) = held.or(self.hover.map(|(key, at)| (key, at, true))) {
            if let Some(p) = ground(at) {
                let tone = if !valid {
                    palette::BAD
                } else if key.kind == OrderKind::Build {
                    palette::WARN
                } else {
                    0xFFFFFF
                };
                let radius = if self.drag.is_some() { 14.0 } else { 12.0 };
                let c = p / ui.s;
                ui.arc(c, radius, 0.0, std::f32::consts::TAU, 1.6, ui::rgb(tone, 0.95));
                // Four ticks turning slowly round it: it can be picked up.
                let spin = t * 1.5;
                for i in 0..4 {
                    let a = spin + i as f32 * std::f32::consts::FRAC_PI_2;
                    let d = Vec2::from_angle(a);
                    ui.stroke(c + d * (radius + 2.0), c + d * (radius + 6.0), 1.6, ui::rgb(tone, 0.9));
                }
            }
        }
    }
}

/// The kind and exact position an order is found by.
fn key_of(order: &mc_sim::mirror::QueuedOrder) -> Key {
    Key {
        kind: order.kind,
        at: order.at,
        blueprint: if order.kind == OrderKind::Build {
            order.blueprint
        } else {
            BlueprintId(0)
        },
    }
}

fn tone_of(kind: OrderKind) -> u32 {
    match kind {
        OrderKind::Attack | OrderKind::AttackMove | OrderKind::AttackGround | OrderKind::Bombard => {
            palette::BAD
        }
        OrderKind::Move | OrderKind::Board | OrderKind::Land | OrderKind::Unload => {
            hud::style::Family::Movement.tone()
        }
        OrderKind::Patrol => PATROL,
        OrderKind::Orbit => ORBIT,
        OrderKind::Guard => hud::style::Family::Stance.tone(),
        OrderKind::Build | OrderKind::Assist => palette::WARN,
        OrderKind::Reclaim | OrderKind::ReclaimUnit => hud::MASS,
        _ => palette::ACCENT,
    }
}

/// Where an order line starts: a unit walking alone, a group's middle, or the waypoint before.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Origin {
    Unit(u32),
    Group(u64),
    Post(u8, FxVec2),
}

/// A command group on the map, as its badge shows it.
struct Group {
    formation: u64,
    /// The middle of its members, world metres.
    middle: Vec3,
    members: Vec<u32>,
    selected: bool,
}

/// Points between the pulses running down a line, and how fast they run, points a second.
const FLOW_SPACING: f32 = 22.0;
const FLOW_SPEED: f32 = 30.0;

/// An order line from `a` to `b`, in points: a soft glow, a fine core and, while it is
/// lively (`time`), pulses running down it the way the order goes. `looping` is a
/// patrol's way back to its first post, dashed. Returns the strokes it cost.
#[allow(clippy::too_many_arguments)]
fn flow(
    ui: &mut Ui,
    a: Vec2,
    b: Vec2,
    tone: u32,
    strength: f32,
    time: Option<f32>,
    looping: bool,
    budget: usize,
) -> usize {
    let len = a.distance(b);
    if len < 1.0 {
        return 0;
    }
    let dir = (b - a) / len;
    let mut cost = 1;
    ui.stroke(a, b, 6.0, ui::rgb(tone, 0.14 * strength));
    if looping {
        let n = ((len / 9.0) as usize).clamp(1, 120);
        for i in (0..n).step_by(2) {
            let (t0, t1) = (i as f32 / n as f32, ((i + 1) as f32 / n as f32).min(1.0));
            ui.stroke(a.lerp(b, t0), a.lerp(b, t1), 1.2, ui::rgb(tone, 0.55 * strength));
        }
        cost += n / 2 + 1;
    } else {
        ui.stroke(a, b, 1.3, ui::rgb(tone, 0.55 * strength));
        cost += 1;
    }
    let Some(time) = time else {
        return cost;
    };
    // The pulses: short bright dashes, fading in off the start and out into the end.
    let offset = (time * FLOW_SPEED).rem_euclid(FLOW_SPACING);
    let mut d = offset;
    let most = budget.saturating_sub(cost).min(48);
    let mut n = 0;
    while d < len && n < most {
        let head = d.min(len);
        let tail = (d - 7.0).max(0.0);
        let fade = (head / 24.0).min((len - head) / 24.0).clamp(0.0, 1.0);
        if head > tail {
            ui.stroke(
                a + dir * tail,
                a + dir * head,
                2.0,
                ui::rgb(mix_white(tone, 0.45), 0.85 * strength * fade),
            );
        }
        d += FLOW_SPACING;
        n += 1;
    }
    cost + n
}

/// `tone` taken `amount` of the way to white.
fn mix_white(tone: u32, amount: f32) -> u32 {
    let ch = |shift: u32| {
        let v = ((tone >> shift) & 0xFF) as f32;
        ((v + (255.0 - v) * amount).round() as u32).min(255) << shift
    };
    ch(16) | ch(8) | ch(0)
}

/// A waypoint: a ring round a white heart, and for the selection a broken ring turning
/// slowly outside it. Sized to be easy to pick up with shift held.
fn waypoint(ui: &mut Ui, c: Vec2, tone: u32, strength: f32, selected: bool, time: f32) {
    ui.disc(c, 8.5, ui::rgb(0x000000, 0.35 * strength));
    ui.arc(c, 8.0, 0.0, std::f32::consts::TAU, 1.8, ui::rgb(tone, 0.95 * strength));
    ui.disc(c, 2.8, ui::rgb(0xFFFFFF, strength));
    if selected {
        let spin = time * 0.9;
        for i in 0..3 {
            let from = spin + i as f32 * std::f32::consts::TAU / 3.0;
            ui.arc(c, 12.0, from, from + 1.2, 1.4, ui::rgb(tone, 0.65 * strength));
        }
    }
}

/// An orbit on the ground: the circle, `radius` metres round `c`, with a soft glow and,
/// while it is lively (`time`), chevrons flying round it the way the aircraft circle
/// (anticlockwise); a small mark at the middle. `project` takes world metres to points.
/// Returns the strokes it cost.
fn orbit_ring(
    ui: &mut Ui,
    project: &dyn Fn(Vec2) -> Option<Vec2>,
    c: Vec2,
    radius: f32,
    strength: f32,
    time: Option<f32>,
    budget: usize,
) -> usize {
    use std::f32::consts::TAU;
    const SEGMENTS: usize = 72;
    let on = |a: f32| project(c + Vec2::from_angle(a) * radius);
    let view = ui.size + 40.0;
    let seen = |p: Vec2| p.cmpge(Vec2::splat(-40.0)).all() && p.cmple(view).all();
    let mut cost = 0;
    let mut prev = on(0.0);
    for i in 1..=SEGMENTS {
        let next = on(i as f32 / SEGMENTS as f32 * TAU);
        if let (Some(a), Some(b)) = (prev, next) {
            if (seen(a) || seen(b)) && cost + 2 <= budget {
                ui.stroke(a, b, 6.0, ui::rgb(ORBIT, 0.14 * strength));
                ui.stroke(a, b, 1.5, ui::rgb(ORBIT, 0.7 * strength));
                cost += 2;
            }
        }
        prev = next;
    }
    if let Some(m) = project(c).filter(|m| seen(*m)) {
        ui.disc(m, 5.0, ui::rgb(0x000000, 0.35 * strength));
        ui.arc(m, 4.5, 0.0, TAU, 1.4, ui::rgb(ORBIT, 0.95 * strength));
        ui.disc(m, 1.8, ui::rgb(0xFFFFFF, strength));
        cost += 1;
    }
    let Some(time) = time else {
        return cost;
    };
    // Chevrons about 90 m apart, flying round at 60 m/s.
    let count = ((TAU * radius / 90.0) as usize).clamp(4, 16);
    let spin = time * 60.0 / radius.max(1.0);
    let bright = ui::rgb(mix_white(ORBIT, 0.5), 0.9 * strength);
    for i in 0..count {
        if cost + 3 > budget {
            break;
        }
        let a = spin + i as f32 / count as f32 * TAU;
        let (Some(head), Some(tail)) = (on(a), on(a - 18.0 / radius.max(1.0))) else {
            continue;
        };
        if !seen(head) || head.distance(tail) < 2.0 {
            continue;
        }
        let dir = (head - tail).normalize();
        let side = dir.perp() * 4.0;
        let back = head - dir * 6.0;
        ui.stroke(tail, head, 2.0, bright);
        ui.stroke(head, back + side, 1.8, bright);
        ui.stroke(head, back - side, 1.8, bright);
        cost += 3;
    }
    cost
}

/// How big a group's badge is, in points: room for its count.
fn badge_radius(count: usize) -> f32 {
    if count >= 100 {
        17.0
    } else if count >= 10 {
        15.0
    } else {
        13.5
    }
}

/// A group's badge in the middle of its members: faint glass with their count, ringed in
/// white, so the units show through it. Lit (selected, or shift held) it
/// firms up a little and a slow sweep turns round it; hovered, it firms up and swells.
fn badge(ui: &mut Ui, c: Vec2, count: usize, lit: bool, hovered: bool, time: f32) {
    let tone = 0xFFFFFF;
    let r = badge_radius(count) + if hovered { 2.0 } else { 0.0 };
    let strength = if hovered {
        0.95
    } else if lit {
        0.6
    } else {
        0.45
    };
    ui.disc(c, r, ui::rgb(0x05070A, 0.4 * strength));
    ui.arc(c, r, 0.0, std::f32::consts::TAU, 1.4, ui::rgb(tone, 0.7 * strength));
    if lit {
        let spin = time * 1.2;
        for from in [spin, spin + std::f32::consts::PI] {
            ui.arc(c, r + 3.0, from, from + 1.4, 1.2, ui::rgb(tone, 0.7 * strength));
        }
    }
    // The count reads over a bright icon too: a dark edge round it, and firmer than the glass.
    let text = count.to_string();
    let ink = (strength + 0.3).min(1.0);
    for d in [Vec2::X, -Vec2::X, Vec2::Y, -Vec2::Y] {
        let p = c + d;
        ui.text_centred(p.x, p.y, ui::type_scale::VALUE, ui::rgb(0x000000, 0.7 * ink), &text);
    }
    ui.text_centred(c.x, c.y, ui::type_scale::VALUE, ui::rgb(0xFFFFFF, ink), &text);
}

#[cfg(test)]
mod tests {
    use super::*;
    use mc_sim::mirror::{QueuedOrder, UnitOrders};

    /// A side of `n` units; `orders[i]` is unit i's front order's group (None: idle).
    fn side(view: &mut View, orders: &[Option<u64>]) {
        view.frame.units = (0..orders.len())
            .map(|i| UnitInstance {
                unit_id: i as u32 + 1,
                // Plain old data: all zeroes is a unit at the origin.
                ..unsafe { std::mem::zeroed() }
            })
            .collect();
        view.index_of = (0..orders.len()).map(|i| (i as u32 + 1, i)).collect();
        view.status.queues = orders
            .iter()
            .enumerate()
            .filter_map(|(i, f)| {
                f.map(|formation| UnitOrders {
                    unit_id: i as u32 + 1,
                    orders: vec![QueuedOrder {
                        formation,
                        offset: [0.0; 2],
                        moving_slot: None,
                        formation_phase: 0,
                        kind: OrderKind::Move,
                        pos: [0.0; 2],
                        at: FxVec2::ZERO,
                        blueprint: BlueprintId(0),
                        radius: 0.0,
                    }],
                    ..Default::default()
                })
            })
            .collect();
    }

    fn kept(map: &OrderMap) -> Vec<(u64, Vec<u32>)> {
        map.kept.iter().map(|(f, m)| (*f, m.clone())).collect()
    }

    #[test]
    fn a_group_outlives_its_orders_until_its_members_are_ordered_away() {
        let mut view = View::new(0, [[1.0; 3]; 8], false);
        let mut map = OrderMap::default();
        side(&mut view, &[Some(7), Some(7), Some(7), Some(7)]);
        map.keep_groups(&view);
        assert_eq!(kept(&map), vec![(7, vec![1, 2, 3, 4])]);
        // They arrive: nothing is queued any more, and the group stays.
        side(&mut view, &[None, None, None, None]);
        map.keep_groups(&view);
        assert_eq!(kept(&map), vec![(7, vec![1, 2, 3, 4])]);
        // Two are sent off together: a group of their own, the rest stay in the old one.
        map.ordered(&[Handle(3), Handle(4)]);
        side(&mut view, &[None, None, Some(7), Some(7)]);
        map.keep_groups(&view);
        assert_eq!(kept(&map), vec![(7, vec![1, 2])], "the old order is not theirs any more");
        side(&mut view, &[None, None, Some(9), Some(9)]);
        map.keep_groups(&view);
        assert_eq!(kept(&map), vec![(7, vec![1, 2]), (9, vec![3, 4])]);
        // Stop leaves no queue to read: the order alone takes a unit out, and a group of one is none.
        map.ordered(&[Handle(1)]);
        side(&mut view, &[None, None, Some(9), Some(9)]);
        map.keep_groups(&view);
        assert_eq!(kept(&map), vec![(9, vec![3, 4])]);
        // The dead drop out too.
        side(&mut view, &[None, None, Some(9)]);
        map.keep_groups(&view);
        assert!(kept(&map).is_empty());
    }

    #[test]
    fn a_click_is_one_site() {
        let at = FxVec2::from_ints(64, 64);
        assert_eq!(
            line_centres((2, 2), false, at, at),
            vec![snap_footprint((2, 2), at)]
        );
    }

    #[test]
    fn a_row_of_power_sits_edge_to_edge() {
        let from = FxVec2::from_ints(0, 0);
        let to = FxVec2::from_ints(96, 0);
        let sites = line_centres((2, 2), false, from, to);
        assert_eq!(
            sites,
            vec![
                FxVec2::from_ints(0, 0),
                FxVec2::from_ints(24, 0),
                FxVec2::from_ints(48, 0),
                FxVec2::from_ints(72, 0),
                FxVec2::from_ints(96, 0)
            ]
        );
    }

    #[test]
    fn a_short_drag_stays_one_building() {
        let sites = line_centres(
            (2, 2),
            false,
            FxVec2::from_ints(0, 0),
            FxVec2::from_ints(16, 0),
        );
        assert_eq!(sites, vec![FxVec2::from_ints(0, 0)]);
    }

    #[test]
    fn a_diagonal_keeps_clearance() {
        let sites = line_centres(
            (2, 2),
            false,
            FxVec2::from_ints(0, 0),
            FxVec2::from_ints(64, 64),
        );
        assert_eq!(sites.len(), 3);
        for pair in sites.windows(2) {
            assert!(
                !footprints_overlap((2, 2), pair[0], pair[1]),
                "{pair:?} overlap"
            );
        }
    }

    #[test]
    fn mines_do_not_line_up() {
        assert_eq!(
            line_centres(
                (2, 2),
                true,
                FxVec2::from_ints(0, 0),
                FxVec2::from_ints(200, 0)
            )
            .len(),
            1
        );
    }

    #[test]
    fn a_long_drag_caps() {
        let to = FxVec2::from_ints(MAX_DRAG_SITES as i32 * 64, 0);
        assert!(line_centres((2, 2), false, FxVec2::ZERO, to).len() <= MAX_DRAG_SITES);
    }

    #[test]
    fn a_shift_click_goes_into_the_nearest_patrol_leg() {
        let mut view = View::new(0, [[1.0; 3]; 8], false);
        let post = |x, y| FxVec2::from_ints(x, y);
        let (a, b, c) = (post(0, 0), post(100, 0), post(100, 100));
        // Two units flying one loop, a post apart.
        view.status.queues = [(1, [a, b, c]), (2, [b, c, a])]
            .into_iter()
            .map(|(unit_id, posts)| UnitOrders {
                unit_id,
                orders: posts
                    .iter()
                    .map(|&at| QueuedOrder {
                        formation: 0,
                        offset: [0.0; 2],
                        moving_slot: None,
                        formation_phase: 0,
                        kind: OrderKind::Patrol,
                        pos: at.to_f32(),
                        at,
                        blueprint: BlueprintId(0),
                        radius: 0.0,
                    })
                    .collect(),
                ..Default::default()
            })
            .collect();
        view.selection = vec![1, 2];
        assert_eq!(patrol_legs(&view), vec![(a, b), (b, c), (c, a)]);
        assert_eq!(patrol_insert_leg(&view, Vec2::new(50.0, -10.0)), Some((a, b)));
        assert_eq!(patrol_insert_leg(&view, Vec2::new(120.0, 60.0)), Some((b, c)));
        assert_eq!(patrol_insert_leg(&view, Vec2::new(30.0, 60.0)), Some((c, a)));
        // A post sent but not yet in the queues splits its leg at once.
        let d = post(50, -40);
        view.patrol_inserts.push((a, d));
        assert_eq!(patrol_insert_leg(&view, Vec2::new(80.0, -30.0)), Some((d, b)));
        view.selection.clear();
        assert_eq!(patrol_insert_leg(&view, Vec2::ZERO), None);
    }
}
