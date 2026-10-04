//! The city wall: ARC's fortification between the city (+y of a wall segment, +x
//! of a gate) and its outskirts. Reinforced concrete cast in lifts, a walkway along
//! the top behind a parapet with firing slits, towers along it and gatehouses where
//! the roads go through. Segments are `WALL_SEGMENT_M` long and join end to end along
//! x without a seam: each is the same cross-section drawn its whole length.

use glam::{Vec2, Vec3};
use mc_map::city::{GATE_PASSAGE_M, WALL_SEGMENT_M};
use mc_map::PropKind;

use super::kit::*;
use crate::builder::MeshBuilder;
use crate::gpu_consts::city as pat;

/// How the wall is built (one design question for the wall, its towers and gates).
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Style {
    /// A battered outer face rising to a parapet, the walkway behind it, counterforts
    /// down the city side.
    Bastion,
    /// Sheer faces, an overhanging firing gallery near the top, the city side stepped
    /// down to a firing terrace and a row of casemates.
    Casemate,
    /// A rubble glacis heaped against a precast panel wall, a steel gallery on the
    /// city side.
    Glacis,
}

/// Half the segment's length.
fn half_len() -> f32 {
    WALL_SEGMENT_M as f32 * 0.5
}

/// The wall's cross-section (y, z) for a style, outer face (-y) first.
fn section(style: Style, coarse: bool) -> Vec<[f32; 2]> {
    let (wall, top) = part(PropKind::CityWall, 0);
    let (o, i) = (wall.min.y, wall.max.y);
    match (style, coarse) {
        (Style::Bastion, true) => vec![[o, -2.0], [o + 3.5, top], [2.5, top], [2.5, -2.0]],
        (Style::Bastion, false) => vec![
            [o, -2.0],
            [o + 3.5, top - 4.0],
            [o + 3.5, top],
            [o + 4.7, top],
            [o + 4.7, top - 1.5],
            [2.5, top - 1.5],
            [2.5, -2.0],
        ],
        (Style::Casemate, true) => vec![
            [o + 0.5, -2.0],
            [o + 0.5, top],
            [-1.0, top],
            [-1.0, 14.0],
            [i, 6.5],
            [i, -2.0],
        ],
        (Style::Casemate, false) => vec![
            [o + 0.5, -2.0],
            [o + 0.5, 16.0],
            [o - 0.8, 16.8],
            [o - 0.8, top],
            [o + 0.6, top],
            [o + 0.6, top - 1.5],
            [-1.0, top - 1.5],
            [-1.0, 14.0],
            [6.5, 14.0],
            [6.5, 6.5],
            [i, 6.5],
            [i, -2.0],
        ],
        (Style::Glacis, _) => vec![
            [o + 5.5, -2.0],
            [o + 5.5, top],
            [o + 11.5, top],
            [o + 11.5, -2.0],
        ],
    }
}

pub(super) fn wall(b: &mut MeshBuilder, _tech: u8) {
    wall_segment(b, Style::Bastion);
}

pub(super) fn wall_casemate(b: &mut MeshBuilder, _tech: u8) {
    wall_segment(b, Style::Casemate);
}

pub(super) fn wall_glacis(b: &mut MeshBuilder, _tech: u8) {
    wall_segment(b, Style::Glacis);
}

fn wall_segment(b: &mut MeshBuilder, style: Style) {
    let (wall, top) = part(PropKind::CityWall, 0);
    let h = half_len();
    paint(b, pat::FORT);
    b.extrude_x(&section(style, b.coarse()), -h, h);
    if style == Style::Glacis {
        // The glacis: rubble and spoil heaped against the outer face.
        let o = wall.min.y;
        paint(b, pat::RUBBLE);
        let berm = if b.coarse() {
            vec![[o, -2.0], [o + 5.5, 10.0], [o + 5.5, -2.0]]
        } else {
            vec![[o, -2.0], [o + 2.5, 5.5], [o + 5.5, 10.0], [o + 5.5, -2.0]]
        };
        b.extrude_x(&berm, -h, h);
    }
    if b.coarse() {
        return;
    }
    match style {
        Style::Bastion => {
            // Counterforts down the city side, every 16 m.
            for k in 0..4 {
                let x = -h + 8.0 + 16.0 * k as f32;
                paint(b, pat::FORT);
                b.extrude_x(
                    &[
                        [2.4, -2.0],
                        [wall.max.y, -2.0],
                        [wall.max.y, 2.0],
                        [3.6, top - 4.0],
                        [2.4, top - 4.0],
                    ],
                    x - 1.6,
                    x + 1.6,
                );
            }
            walkway(b, wall.min.y + 4.7, 2.5, top - 1.5);
            slits(b, wall.min.y + 3.5, top - 1.2, top - 0.3, 4.0);
        }
        Style::Casemate => {
            walkway(b, wall.min.y + 0.6, -1.0, top - 1.5);
            // Gun slits along the gallery, and casemate doors on the city side.
            slits(b, wall.min.y - 0.8, 18.0, 19.4, 5.33);
            if b.fine() {
                for k in 0..4 {
                    let x = -h + 8.0 + 16.0 * k as f32;
                    paint(b, pat::BLAST_DOOR);
                    facing(
                        b,
                        vec![
                            v3(x - 1.6, wall.max.y + 0.05, 0.0),
                            v3(x + 1.6, wall.max.y + 0.05, 0.0),
                            v3(x + 1.6, wall.max.y + 0.05, 3.6),
                            v3(x - 1.6, wall.max.y + 0.05, 3.6),
                        ],
                        Vec3::Y,
                    );
                }
                rail(b, 6.4, 14.0, h);
            }
        }
        Style::Glacis => {
            let (o, i) = (wall.min.y + 5.5, wall.min.y + 11.5);
            walkway(b, o, i, top);
            // The steel gallery on the city side, on columns.
            let deck = 18.0;
            for k in 0..8 {
                let x = -h + 4.0 + 8.0 * k as f32;
                boxed(
                    b,
                    v3(x - 0.25, wall.max.y - 2.0, -0.5),
                    v3(x + 0.25, wall.max.y - 1.5, deck),
                    pat::STEEL,
                );
            }
            boxed(
                b,
                v3(-h, i, deck - 0.4),
                v3(h, wall.max.y - 1.3, deck),
                pat::STEEL,
            );
            if b.fine() {
                rail(b, wall.max.y - 1.4, deck, h);
                // The precast panels' joints: a rib every 1.5 m up the outer face.
                for k in 0..42 {
                    let x = -h + 0.75 + 1.5 * k as f32;
                    boxed(
                        b,
                        v3(x - 0.1, o - 0.25, 10.0),
                        v3(x + 0.1, o, top),
                        pat::FORT,
                    );
                }
            }
            slits(b, o, top - 2.2, top - 1.3, 4.0);
        }
    }
}

/// The walkway on top between the parapet (`from`) and the city side (`to`), and a
/// rail along its open edge.
fn walkway(b: &mut MeshBuilder, from: f32, to: f32, z: f32) {
    let h = half_len();
    paint(b, pat::PAVING);
    b.face(&[
        v3(-h, from, z + 0.02),
        v3(h, from, z + 0.02),
        v3(h, to, z + 0.02),
        v3(-h, to, z + 0.02),
    ]);
    if b.fine() {
        rail(b, to - 0.15, z, h);
        // Floodlights on posts.
        for x in [-h * 0.5, h * 0.5] {
            mast(b, v3(x, to - 0.4, z), 5.0, 0.2);
            boxed(
                b,
                v3(x - 0.5, to - 0.9, z + 4.8),
                v3(x + 0.5, to - 0.3, z + 5.4),
                pat::STEEL,
            );
        }
    }
}

/// A rail at `y`, a metre over `z`, the segment's length.
fn rail(b: &mut MeshBuilder, y: f32, z: f32, h: f32) {
    boxed(
        b,
        v3(-h, y - 0.05, z + 1.0),
        v3(h, y + 0.05, z + 1.1),
        pat::STEEL,
    );
    for k in 0..16 {
        let x = -h + 2.0 + 4.0 * k as f32;
        boxed(
            b,
            v3(x - 0.04, y - 0.04, z),
            v3(x + 0.04, y + 0.04, z + 1.0),
            pat::STEEL,
        );
    }
}

/// Firing slits in the outer face at `y` between `z0` and `z1`, `pitch` apart.
fn slits(b: &mut MeshBuilder, y: f32, z0: f32, z1: f32, pitch: f32) {
    if !b.mid() {
        return;
    }
    let h = half_len();
    let n = (2.0 * h / pitch).round() as usize;
    paint(b, pat::SHADOW);
    for k in 0..n {
        let x = -h + (k as f32 + 0.5) * 2.0 * h / n as f32;
        facing(
            b,
            vec![
                v3(x - 0.35, y - 0.04, z0),
                v3(x + 0.35, y - 0.04, z0),
                v3(x + 0.35, y - 0.04, z1),
                v3(x - 0.35, y - 0.04, z1),
            ],
            Vec3::NEG_Y,
        );
    }
}

// ---- towers ----------------------------------------------------------------------------

pub(super) fn tower(b: &mut MeshBuilder, _tech: u8) {
    let (r, top) = part(PropKind::CityWallTower, 0);
    fort_tower(b, Style::Bastion, r, top, Side::Front, true);
}

pub(super) fn tower_casemate(b: &mut MeshBuilder, _tech: u8) {
    let (r, top) = part(PropKind::CityWallTower, 0);
    fort_tower(b, Style::Casemate, r, top, Side::Front, true);
}

pub(super) fn tower_glacis(b: &mut MeshBuilder, _tech: u8) {
    let (r, top) = part(PropKind::CityWallTower, 0);
    fort_tower(b, Style::Glacis, r, top, Side::Front, true);
}

/// A tower of the wall in `style` on `r` up to `top`: its door on `inner`, a radar and
/// a mast on its roof when `crowned`.
fn fort_tower(b: &mut MeshBuilder, style: Style, r: Rect, top: f32, inner: Side, crowned: bool) {
    let c = r.centre();
    let deck_z = top - 4.0;
    let gun = deck_z - 8.0;
    match style {
        Style::Bastion => {
            let waist = r.grow(-2.2);
            paint(b, pat::FORT);
            b.loft(&[r.ring(-2.0), waist.ring(gun)], false, false);
            if b.coarse() {
                solid(b, waist, gun, top, pat::FORT, pat::ROOF_FLAT);
                return;
            }
            walls(b, waist, gun, deck_z, pat::FORT);
            ledge(b, waist, waist.grow(1.2), deck_z, false, pat::FORT);
            walls(b, waist.grow(1.2), deck_z, top, pat::FORT);
            parapet_top(b, waist.grow(1.2), top);
            slit_ring(b, waist, gun + 3.0, gun + 4.4, 3.5);
        }
        Style::Casemate => {
            let body = r.grow(-0.5);
            walls(b, body, -2.0, gun, pat::FORT);
            if b.coarse() {
                solid(b, r, gun, top, pat::FORT, pat::ROOF_FLAT);
                return;
            }
            ledge(b, body, r, gun, false, pat::FORT);
            walls(b, r, gun, top, pat::FORT);
            parapet_top(b, r, top);
            slit_ring(b, r, gun + 3.0, gun + 4.4, 4.0);
            // Observation cupolas on the corners.
            for (sx, sy) in [(1.0f32, 1.0f32), (-1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)] {
                let p = c + Vec2::new(sx, sy) * (r.size() * 0.5 - Vec2::splat(3.0));
                paint(b, pat::FORT);
                b.prism(p.extend(top - 1.5), b.sides(10), 1.8, 1.8, 2.2);
                if b.fine() {
                    b.spheroid(p.extend(top + 0.7), v3(1.8, 1.8, 1.0), 10, 3);
                }
            }
        }
        Style::Glacis => {
            let body = r.grow(-4.5);
            paint(b, pat::RUBBLE);
            b.loft(&[r.ring(-2.0), body.ring(10.0)], false, false);
            if b.coarse() {
                solid(b, body, 10.0, top, pat::FORT, pat::ROOF_FLAT);
                return;
            }
            walls(b, body, 10.0, gun + 4.0, pat::FORT);
            deck(b, body, gun + 4.0, pat::PAVING);
            // The watch cabin: steel walls, armoured glass all round.
            let cabin = body.grow(-1.5);
            walls(b, cabin, gun + 4.0, gun + 5.2, pat::STEEL);
            walls(b, cabin, gun + 5.2, top - 0.8, pat::RIBBON);
            solid(
                b,
                cabin.grow(0.6),
                top - 0.8,
                top,
                pat::STEEL,
                pat::ROOF_FLAT,
            );
            if b.fine() {
                for side in Side::ALL {
                    let (a0, a1, _) = side.run(body);
                    for a in cells(a0, a1 - a0, 1.5) {
                        let (min, max) =
                            side.block(body, a - 0.1, a + 0.1, 0.0, 0.25, 10.0, gun + 4.0);
                        boxed(b, min, max, pat::FORT);
                    }
                }
            }
        }
    }
    // The door to the city side, and the roof's kit.
    let (a0, a1, _) = inner.run(r);
    let mid = (a0 + a1) * 0.5;
    let face = match style {
        Style::Bastion => r.grow(-0.4),
        Style::Casemate => r.grow(-0.5),
        Style::Glacis => r.grow(-4.5),
    };
    let door_z = if style == Style::Glacis { 10.0 } else { 0.0 };
    paint(b, pat::BLAST_DOOR);
    panel(
        b,
        face,
        inner,
        mid - 2.0,
        mid + 2.0,
        door_z,
        door_z + 4.2,
        0.08,
    );
    if !crowned || !b.mid() {
        return;
    }
    let roof = if style == Style::Glacis {
        top
    } else {
        top - 1.4
    };
    // A radome on a plinth, a lattice mast with its dishes.
    boxed(
        b,
        v3(c.x - 3.0, c.y - 3.0, roof),
        v3(c.x + 3.0, c.y + 3.0, roof + 1.5),
        pat::FORT,
    );
    paint(b, pat::STEEL);
    b.spheroid(
        v3(c.x, c.y, roof + 3.3),
        Vec3::splat(2.8),
        b.sides(14),
        if b.fine() { 6 } else { 3 },
    );
    let m = v3(c.x - r.size().x * 0.3, c.y - r.size().y * 0.3, roof);
    mast(b, m, 16.0, 0.9);
    if b.fine() {
        for k in 0..3 {
            let z = roof + 6.0 + 3.5 * k as f32;
            paint(b, pat::STEEL);
            b.cylinder_between(
                m + v3(0.4, 0.0, z - roof),
                m + v3(0.9, 0.0, z - roof),
                0.8,
                0.1,
                10,
            );
        }
        // Floodlights at the corners.
        for (sx, sy) in [(1.0f32, 1.0f32), (-1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)] {
            let p = c + Vec2::new(sx, sy)
                * (r.size() * 0.5 - Vec2::splat(if style == Style::Glacis { 6.0 } else { 2.5 }));
            boxed(
                b,
                (p - Vec2::splat(0.4)).extend(roof),
                (p + Vec2::splat(0.4)).extend(roof + 0.9),
                pat::STEEL,
            );
        }
    }
}

/// The parapet round a tower's roof `r` at `top`, its slits and the roof inside.
fn parapet_top(b: &mut MeshBuilder, r: Rect, top: f32) {
    let inner = r.grow(-1.0);
    paint(b, pat::FORT);
    b.loft(&[inner.ring(top), inner.ring(top - 1.4)], false, false);
    ledge(b, inner, r, top, true, pat::FORT);
    deck(b, inner, top - 1.4, pat::ROOF_FLAT);
}

/// Firing slits all round `r` from `z0` to `z1`, `pitch` apart.
fn slit_ring(b: &mut MeshBuilder, r: Rect, z0: f32, z1: f32, pitch: f32) {
    if !b.mid() {
        return;
    }
    paint(b, pat::SHADOW);
    for side in Side::ALL {
        let (a0, a1, _) = side.run(r);
        for a in cells(a0 + 1.0, a1 - a0 - 2.0, pitch) {
            panel(b, r, side, a - 0.35, a + 0.35, z0, z1, 0.04);
        }
    }
}

// ---- the gatehouse -----------------------------------------------------------------------

pub(super) fn gate(b: &mut MeshBuilder, _tech: u8) {
    gatehouse(b, Style::Bastion);
}

pub(super) fn gate_casemate(b: &mut MeshBuilder, _tech: u8) {
    gatehouse(b, Style::Casemate);
}

pub(super) fn gate_glacis(b: &mut MeshBuilder, _tech: u8) {
    gatehouse(b, Style::Glacis);
}

/// Two towers either side of the road (along x, the city at +x), a bridge between them
/// carrying a gallery over the road, and the blast doors standing open against the
/// towers' inner walls.
fn gatehouse(b: &mut MeshBuilder, style: Style) {
    let towers: Vec<(Rect, f32)> = (0..2).map(|i| part(PropKind::CityGate, i)).collect();
    let road = GATE_PASSAGE_M as f32 * 0.5;
    let top = towers[0].1;
    for (k, &(r, t)) in towers.iter().enumerate() {
        // Each tower's door opens off the road.
        let inner = if k == 0 { Side::Back } else { Side::Front };
        fort_tower(b, style, r, t, inner, k == 0);
    }
    let len = towers[0].0.size().x;
    // The bridge: a gallery across the road, its underside clear of traffic.
    let soffit = top - 12.0;
    let bridge = Rect::new(-len * 0.3, -road - 1.0, len * 0.3, road + 1.0);
    if b.coarse() {
        solid(b, bridge, soffit, top - 3.0, pat::FORT, pat::ROOF_FLAT);
        return;
    }
    walls(b, bridge, soffit, top - 3.0, pat::FORT);
    deck(b, bridge, top - 3.0, pat::PAVING);
    paint(b, pat::FORT);
    b.face(&bridge.ring(soffit).into_iter().rev().collect::<Vec<_>>());
    // Slits along the gallery, both ways.
    paint(b, pat::SHADOW);
    for side in [Side::East, Side::West] {
        for a in cells(-road, 2.0 * road, 4.0) {
            panel(
                b,
                bridge,
                side,
                a - 0.35,
                a + 0.35,
                soffit + 4.0,
                soffit + 5.4,
                0.04,
            );
        }
    }
    // The blast doors, swung open flat against the passage walls, hinged at the
    // outer (attackers', -x) face.
    let x0 = towers[0].0.min.x + 1.5;
    let leaf = road;
    for s in [-1.0f32, 1.0] {
        let y = s * (road - 1.0);
        boxed(
            b,
            v3(x0, y - 0.8, 0.0),
            v3(x0 + leaf, y + 0.8, 14.0),
            pat::BLAST_DOOR,
        );
        if b.fine() {
            // Hinge pins and the door's track across the road.
            boxed(
                b,
                v3(x0 - 0.6, y - 1.2, 0.0),
                v3(x0 + 0.4, y + 1.2, 14.5),
                pat::STEEL,
            );
        }
    }
    if b.fine() {
        paint(b, pat::STEEL);
        b.face(&[
            v3(x0 - 0.3, -road, 0.03),
            v3(x0 + 0.3, -road, 0.03),
            v3(x0 + 0.3, road, 0.03),
            v3(x0 - 0.3, road, 0.03),
        ]);
        // Lamps under the bridge; hazard bollards at the towers' corners.
        for y in [-road * 0.5, 0.0, road * 0.5] {
            boxed(
                b,
                v3(-0.6, y - 0.6, soffit - 0.5),
                v3(0.6, y + 0.6, soffit),
                pat::STEEL,
            );
        }
        for s in [-1.0f32, 1.0] {
            for x in [towers[0].0.min.x, towers[0].0.max.x] {
                paint(b, pat::BLAST_DOOR);
                b.prism(v3(x, s * (road - 0.8), 0.0), 8, 0.4, 0.4, 1.2);
            }
        }
    }
}
