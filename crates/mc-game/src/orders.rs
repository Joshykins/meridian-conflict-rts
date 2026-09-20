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
use mc_sim::command::MAX_COMMAND_UNITS;
use mc_sim::mirror::{UnitInstance, KIND_GHOST, KIND_WRECK};
use mc_sim::tables::OrderKind;
use mc_sim::{Command, Handle};
use std::collections::HashSet;

/// How near a waypoint a press has to be to pick it up, in pixels.
const GRAB_REACH: f32 = 14.0;
/// Most structures one place-drag may put down.
pub const MAX_DRAG_SITES: usize = 64;
/// The overlay's vertex budget is shared with the HUD: a line costs 18 vertices, a waypoint about 200.
const MAX_LINES: usize = 900;
const MAX_NODES: usize = 120;
const MAX_GHOSTS: usize = 256;
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
}

fn draggable(kind: OrderKind) -> bool {
    matches!(
        kind,
        OrderKind::Move | OrderKind::AttackMove | OrderKind::Build
    )
}

fn to_fx(p: Vec2) -> FxVec2 {
    FxVec2::new(Fx::from_f32(p.x), Fx::from_f32(p.y))
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
    site_among(field, blueprint, ground, moving, &[])
}

/// `site`, treating `taken` as structures already spoken for (the earlier
/// buildings of a place-drag).
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
    let mut pos = to_fx(ground.truncate());
    let mut valid = true;
    if bp.needs_deposit {
        let nearest = map
            .mass_deposits()
            .iter()
            .min_by_key(|d| d.distance_sq(pos))?;
        valid = nearest.distance(pos) < Fx::from_int(60);
        if valid {
            pos = *nearest;
        }
    }
    let pos = mc_sim::world::snap_to_build_grid(bp, pos);
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
    if ground.z < map.info().water_level.to_f32() {
        valid = false;
    }
    Some((pos, valid))
}

/// Centres a drag from `from` to `to` would occupy, spaced a footprint apart so
/// they sit edge to edge. One centre when the pointer has not moved a footprint,
/// and one for a structure that has to sit on a deposit.
pub fn line_centres(
    footprint: (u8, u8),
    needs_deposit: bool,
    from: FxVec2,
    to: FxVec2,
) -> Vec<FxVec2> {
    let from = snap_footprint(footprint, from);
    let to = snap_footprint(footprint, to);
    if needs_deposit || from == to {
        return vec![if needs_deposit { to } else { from }];
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
) -> Vec<(FxVec2, bool)> {
    let bp = field.blueprints.unit(blueprint);
    let mut out = Vec::new();
    let mut taken = Vec::new();
    for centre in line_centres(bp.footprint, bp.needs_deposit, from, to) {
        let xy = Vec2::from(centre.to_f32());
        let Some((pos, valid)) = site_among(
            field,
            blueprint,
            xy.extend(field.renderer.ground_height(xy)),
            None,
            &taken,
        ) else {
            continue;
        };
        if out.iter().any(|(p, _)| *p == pos) {
            continue;
        }
        out.push((pos, valid));
        if valid {
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

impl OrderMap {
    pub fn dragging(&self) -> bool {
        self.drag.is_some()
    }

    /// An order is in hand: whether it could be put down where the pointer is.
    pub fn in_hand(&self) -> Option<bool> {
        self.drag
            .as_ref()
            .map(|_| self.aim.is_some_and(|(_, valid)| valid))
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
            if grip.key == key {
                return Vec2::from(aim.to_f32());
            }
        }
        match self.settling.iter().find(|(k, ..)| *k == key) {
            Some((_, to, _)) => Vec2::from(to.to_f32()),
            None => Vec2::from(pos),
        }
    }

    fn ground_under(field: &Field, cursor: Vec2) -> Option<Vec3> {
        let (origin, dir) = field.camera.ray(cursor);
        field.renderer.pick_ground(origin, dir)
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
                .flat_map(|q| q.orders.iter())
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
                let Some(p) = camera.project(at.extend(renderer.ground_height(at) + 1.0)) else {
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
            .filter(|q| {
                q.orders
                    .iter()
                    .any(|o| o.kind == key.kind && o.at == key.at)
            })
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
        let status = &field.view.status;
        let held = |key: &Key| {
            status.plans.iter().any(|p| {
                key.kind == OrderKind::Build && p.at == key.at && p.blueprint == key.blueprint
            }) || status
                .queues
                .iter()
                .flat_map(|q| q.orders.iter())
                .any(|o| o.kind == key.kind && o.at == key.at)
        };
        self.settling
            .retain(|(key, _, tick)| status.tick <= tick + SETTLE_TICKS && held(key));
        // The order in hand was carried out, or cancelled, under the pointer.
        if self.drag.as_ref().is_some_and(|grip| !held(&grip.key)) {
            self.cancel();
        }
        self.hover = None;
        if let Some(grip) = &self.drag {
            self.aim = Self::ground_under(field, cursor).and_then(|ground| match grip.key.kind {
                OrderKind::Build => site(field, grip.key.blueprint, ground, Some(grip.key.at)),
                _ => Some((to_fx(ground.truncate()), true)),
            });
        } else if field.view.shift && !over_ui {
            self.hover = self.pick(field, cursor);
        }
    }

    /// Ghosts of the planned structures: the selection's, and the whole side's while shift
    /// is held, a structure is being placed or a plan is in hand. Several builders sharing
    /// a plan make one ghost.
    pub fn ghosts(&self, field: &Field, out: &mut Vec<UnitInstance>) {
        let Field {
            view,
            blueprints,
            renderer,
            ..
        } = field;
        let everyone = view.shift || self.drag.is_some() || matches!(view.mode, Mode::Place(_));
        let mut seen: HashSet<(FxVec2, BlueprintId)> = HashSet::new();
        for plan in &view.status.plans {
            if out.len() >= MAX_GHOSTS
                || !(everyone || view.selection.contains(&plan.unit_id))
                || !seen.insert((plan.at, plan.blueprint))
            {
                continue;
            }
            let key = Key {
                kind: OrderKind::Build,
                at: plan.at,
                blueprint: plan.blueprint,
            };
            let at = self.shown(key, plan.pos);
            let in_hand = self.drag.as_ref().is_some_and(|grip| grip.key == key);
            let valid = !in_hand || self.aim.is_some_and(|(_, valid)| valid);
            let p = [at.x, at.y, renderer.ground_height(at)];
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
            });
        }
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

    /// The order lines: from each unit through its waypoints, the selection's brighter
    /// than the rest of the side's, and everything brighter while shift is held.
    pub fn draw(&self, ui: &mut Ui, field: &Field) {
        let Field {
            view,
            camera,
            renderer,
            ..
        } = field;
        let viewport = camera.viewport;
        let on_screen =
            |p: Vec2| p.cmpge(Vec2::splat(-40.0)).all() && p.cmple(viewport + 40.0).all();
        let (mut lines, mut nodes) = (MAX_LINES, MAX_NODES);
        let mut drawn: HashSet<(u8, FxVec2)> = HashSet::new();
        for queue in &view.status.queues {
            let Some(unit) = view
                .index_of
                .get(&queue.unit_id)
                .map(|&i| &view.frame.units[i])
            else {
                continue;
            };
            let selected = view.selection.contains(&queue.unit_id);
            let strength = match (selected, view.shift) {
                (true, true) => 1.0,
                (true, false) => 0.55,
                (false, _) => 0.4,
            };
            let mut from = camera.project(Vec3::from(unit.pos));
            for order in queue.orders.iter().take(24) {
                if matches!(order.kind, OrderKind::Produce | OrderKind::Upgrade) {
                    break;
                }
                let tone = match order.kind {
                    OrderKind::Attack | OrderKind::AttackMove => palette::BAD,
                    OrderKind::Build | OrderKind::Assist => palette::WARN,
                    OrderKind::Reclaim | OrderKind::ReclaimUnit => hud::MASS,
                    _ => palette::ACCENT,
                };
                let key = Key {
                    kind: order.kind,
                    at: order.at,
                    blueprint: if order.kind == OrderKind::Build {
                        order.blueprint
                    } else {
                        BlueprintId(0)
                    },
                };
                let at = if draggable(order.kind) {
                    self.shown(key, order.pos)
                } else {
                    Vec2::from(order.pos)
                };
                let to = camera.project(at.extend(renderer.ground_height(at) + 1.0));
                if let (Some(a), Some(b), true) = (from, to, lines > 0) {
                    // Far off-screen ends would make for enormous quads; the line is not worth it.
                    let visible = on_screen(a)
                        || on_screen(b)
                        || ((a.x < 0.0) != (b.x < 0.0) || (a.y < 0.0) != (b.y < 0.0));
                    if visible
                        && a.abs().max_element() < 20_000.0
                        && b.abs().max_element() < 20_000.0
                    {
                        lines -= 1;
                        ui.stroke(a / ui.s, b / ui.s, 1.2, ui::rgb(tone, 0.5 * strength));
                    }
                }
                // A waypoint a group shares is drawn once, by whoever comes first: the selection.
                if let Some(b) = to.filter(|b| {
                    on_screen(*b) && nodes > 0 && drawn.insert((order.kind as u8, order.at))
                }) {
                    nodes -= 1;
                    let c = b / ui.s;
                    ui.disc(c, 3.2, ui::rgb(tone, 0.9 * strength));
                    if selected {
                        ui.disc(c, 1.4, ui::rgb(0xFFFFFF, strength));
                    }
                }
                from = to;
            }
        }
        // What a press would pick up, or what is in hand.
        let held = self
            .drag
            .as_ref()
            .zip(self.aim)
            .map(|(grip, (aim, valid))| (grip.key, Vec2::from(aim.to_f32()), valid));
        if let Some((key, at, valid)) = held.or(self.hover.map(|(key, at)| (key, at, true))) {
            if let Some(p) = camera.project(at.extend(renderer.ground_height(at) + 1.0)) {
                let tone = if !valid {
                    palette::BAD
                } else if key.kind == OrderKind::Build {
                    palette::WARN
                } else {
                    0xFFFFFF
                };
                let radius = if self.drag.is_some() { 9.0 } else { 7.0 };
                ui.arc(
                    p / ui.s,
                    radius,
                    0.0,
                    std::f32::consts::TAU,
                    1.6,
                    ui::rgb(tone, 0.95),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn extractors_do_not_line_up() {
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
}
