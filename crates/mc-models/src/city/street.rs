//! The city's street furniture and works: street lights, the elevated maglev line
//! and its stations, billboards, cars, barricades, wind turbines, solar arrays, a
//! comms mast and monuments. Many of these stand by the tens of thousands: their
//! coarse and far levels are a handful of triangles.

use glam::{Vec2, Vec3};
use mc_map::city::{TRANSIT_DECK_M, TRANSIT_SEGMENT_M, TRANSIT_STATION_M};
use mc_map::PropKind;

use super::kit::*;
use crate::builder::{hash_unit, MeshBuilder};
use crate::gpu_consts::city as pat;
use crate::material::{GLOW_LAMP, GLOW_NAV_RED};

/// An upright bar of `n` flat sides from `z0` to `z1`, tapering from `r0` to `r1`,
/// open at both ends: a mast seen only from outside and above.
fn pole(b: &mut MeshBuilder, at: Vec2, r0: f32, r1: f32, z0: f32, z1: f32, n: usize) {
    let ring = |r: f32, z: f32| {
        (0..n)
            .map(|i| {
                let a = (i as f32 + 0.5) * std::f32::consts::TAU / n as f32;
                (at + Vec2::from_angle(a) * r).extend(z)
            })
            .collect::<Vec<_>>()
    };
    b.loft(&[ring(r0, z0), ring(r1, z1)], false, false);
}

// ---- a street light ----------------------------------------------------------------------

/// A street light at the kerb: a slim tapering mast 10 m tall, an arm reaching out
/// over the road (+x) to a flat LED head.
pub(super) fn streetlight(b: &mut MeshBuilder, _tech: u8) {
    let h = 10.0;
    let reach = 3.2;
    paint(b, pat::WHITE_STEEL);
    if b.far() {
        pole(b, Vec2::ZERO, 0.14, 0.08, -0.5, h, 3);
        return;
    }
    if b.coarse() {
        pole(b, Vec2::ZERO, 0.14, 0.08, -0.5, h, 3);
        b.beam(
            v3(0.0, 0.0, h - 0.2),
            v3(reach, 0.0, h),
            Vec2::splat(0.12),
            Vec2::splat(0.12),
        );
        return;
    }
    pole(b, Vec2::ZERO, 0.13, 0.07, -0.5, h, b.sides(6));
    // The arm, rising a little out over the road, and the head.
    b.beam(
        v3(0.0, 0.0, h - 0.4),
        v3(reach, 0.0, h),
        Vec2::new(0.1, 0.12),
        Vec2::new(0.1, 0.1),
    );
    boxed(
        b,
        v3(reach - 0.3, -0.2, h - 0.15),
        v3(reach + 0.6, 0.2, h + 0.05),
        pat::STEEL,
    );
    b.paint(GLOW_LAMP);
    b.face(&[
        v3(reach + 0.5, -0.16, h - 0.16),
        v3(reach + 0.5, 0.16, h - 0.16),
        v3(reach - 0.2, 0.16, h - 0.16),
        v3(reach - 0.2, -0.16, h - 0.16),
    ]);
    if b.fine() {
        // The base and its service hatch.
        boxed(
            b,
            v3(-0.25, -0.25, -0.3),
            v3(0.25, 0.25, 1.0),
            pat::WHITE_STEEL,
        );
    }
}

// ---- the maglev ----------------------------------------------------------------------------

/// The guideway's section (y, z): a box girder tapering below, its deck at the top.
fn girder(deck: f32, coarse: bool) -> Vec<[f32; 2]> {
    if coarse {
        return vec![
            [-2.5, deck - 2.2],
            [2.5, deck - 2.2],
            [2.5, deck],
            [-2.5, deck],
        ];
    }
    vec![
        [-1.4, deck - 2.4],
        [1.4, deck - 2.4],
        [2.5, deck - 1.2],
        [2.5, deck],
        [-2.5, deck],
        [-2.5, deck - 1.2],
    ]
}

/// The guideway along x from `x0` to `x1`: the girder, the levitation rails on its
/// deck, a band of light along each edge.
fn guideway(b: &mut MeshBuilder, x0: f32, x1: f32, split: bool) {
    let deck = TRANSIT_DECK_M as f32;
    paint(b, pat::CONCRETE);
    // In two pours meeting in the middle, over a pylon there.
    let mid = (x0 + x1) * 0.5;
    let pours: &[(f32, f32)] = if split {
        &[(x0, mid), (mid, x1)]
    } else {
        &[(x0, x1)]
    };
    for &(a, c) in pours {
        b.with_facets(|b| b.extrude_x(&girder(deck, b.coarse()), a, c));
    }
    if !b.mid() {
        return;
    }
    // The guideway proper: a raised central spine and two side rails.
    boxed(b, v3(x0, -0.6, deck), v3(x1, 0.6, deck + 0.7), pat::STEEL);
    for s in [-1.0f32, 1.0] {
        boxed(
            b,
            v3(x0, s * 2.0 - 0.15, deck),
            v3(x1, s * 2.0 + 0.15, deck + 0.45),
            pat::STEEL,
        );
        let (min, max) = (
            v3(x0, s * 2.5 - 0.02, deck - 1.1),
            v3(x1, s * 2.5 + 0.02, deck - 0.9),
        );
        boxed(b, min.min(max), max.max(min), pat::LED);
    }
    if b.fine() {
        // The stator packs along the spine's flanks, a joint every 4 m.
        for x in cells(x0, x1 - x0, 2.0) {
            boxed(
                b,
                v3(x - 0.8, -0.75, deck + 0.1),
                v3(x + 0.8, 0.75, deck + 0.55),
                pat::STEEL,
            );
        }
        for x in cells(x0, x1 - x0, 4.0) {
            boxed(
                b,
                v3(x - 0.06, -2.55, deck - 1.2),
                v3(x + 0.06, 2.55, deck),
                pat::CONCRETE,
            );
        }
    }
}

/// A slim pylon at `x`: a tapering stem, splaying into a Y under the girder.
fn pylon(b: &mut MeshBuilder, x: f32, plan: Rect) {
    let deck = TRANSIT_DECK_M as f32;
    let foot = plan.size() * 0.5;
    paint(b, pat::CONCRETE);
    b.frustum_open(
        v3(x, 0.0, -1.0),
        foot * 2.0 * 0.9,
        Vec2::new(foot.x * 1.1, foot.y * 0.9),
        deck - 3.4,
        Vec2::ZERO,
    );
    if b.mid() {
        // The Y: two arms out under the girder's flanks, bearings on them.
        for s in [-1.0f32, 1.0] {
            b.beam(
                v3(x, s * 0.6, deck - 4.6),
                v3(x, s * 1.9, deck - 2.4),
                Vec2::new(1.4, 0.7),
                Vec2::new(1.2, 0.6),
            );
        }
    }
}

/// A segment of the elevated maglev line: 64 m of guideway on one pylon in the
/// avenue's median, joining end to end with the next.
pub(super) fn transit(b: &mut MeshBuilder, _tech: u8) {
    let (plan, _) = part(PropKind::CityTransit, 0);
    let h = TRANSIT_SEGMENT_M as f32 * 0.5;
    guideway(b, -h, h, true);
    pylon(b, 0.0, plan);
}

/// A maglev station over the avenue: the guideway between two side platforms on a
/// wide deck, carried by two pylons; a curved glass canopy over it on steel ribs,
/// platform screen doors, and stairs down to the pavement on either side.
pub(super) fn transit_station(b: &mut MeshBuilder, _tech: u8) {
    let (east, top) = part(PropKind::CityTransitStation, 0);
    let (west, _) = part(PropKind::CityTransitStation, 1);
    let h = TRANSIT_STATION_M as f32 * 0.5;
    let deck = TRANSIT_DECK_M as f32;
    let wide = 9.0;
    // Far off the canopy hides the line through it.
    if !b.coarse() {
        guideway(b, -h, h, false);
    }
    pylon(b, east.centre().x, east);
    pylon(b, west.centre().x, west);
    // The platforms: a slab each side, its edge lit.
    for s in [-1.0f32, 1.0] {
        if b.coarse() {
            break;
        }
        let (y0, y1) = (s * 2.6, s * wide);
        let r = Rect::new(-h + 4.0, y0.min(y1), h - 4.0, y0.max(y1));
        solid(b, r, deck - 1.4, deck + 0.4, pat::CONCRETE, pat::PAVING);
    }
    // The canopy: a glass vault from the platforms' outer edges up to the top.
    let rise = top - deck - 2.6;
    let n = if b.fine() {
        10
    } else if b.mid() {
        6
    } else {
        3
    };
    let arc = |r: f32, z0: f32| -> Vec<[f32; 2]> {
        (0..=n)
            .map(|i| {
                let a = std::f32::consts::PI * i as f32 / n as f32;
                [-r * a.cos(), z0 + rise * a.sin()]
            })
            .collect()
    };
    let spring = deck + 2.6;
    let mut shell = arc(wide, spring);
    let mut inner = arc(wide - 0.25, spring - 0.25);
    inner.reverse();
    shell.extend(inner);
    paint(b, pat::ROOF_GLASS);
    b.extrude_x(&shell, -h + 6.0, h - 6.0);
    if !b.mid() {
        return;
    }
    // Columns up to the canopy's springing, and its steel ribs.
    for x in cells(-h + 6.0, 2.0 * h - 12.0, 8.0) {
        for s in [-1.0f32, 1.0] {
            boxed(
                b,
                v3(x - 0.15, s * wide - 0.15, deck + 0.4),
                v3(x + 0.15, s * wide + 0.15, spring),
                pat::STEEL,
            );
        }
        if b.fine() {
            let mut rib = arc(wide + 0.25, spring);
            let mut under = arc(wide - 0.05, spring - 0.05);
            under.reverse();
            rib.extend(under);
            paint(b, pat::STEEL);
            b.extrude_x(&rib, x - 0.12, x + 0.12);
        }
    }
    // Platform screen doors: a glass wall along each platform's edge.
    for s in [-1.0f32, 1.0] {
        paint(b, pat::CURTAIN);
        let y = s * 2.75;
        facing(
            b,
            vec![
                v3(-h + 8.0, y, deck + 0.4),
                v3(h - 8.0, y, deck + 0.4),
                v3(h - 8.0, y, deck + 2.6),
                v3(-h + 8.0, y, deck + 2.6),
            ],
            Vec3::new(0.0, -s, 0.0),
        );
        // Stairs down to the pavement off the platform's outer edge, and a lift.
        let x = s * h * 0.45;
        let y0 = s * wide;
        let y1 = s * (wide + 4.0);
        paint(b, pat::CONCRETE);
        b.extrude_x(
            &[[y0, deck - 0.4], [y0, deck + 0.4], [y1, 0.4], [y1, 0.0]],
            x - 1.4,
            x + 1.4,
        );
        let lift = Rect::centred(-x, s * (wide + 1.6), 1.3, 1.3);
        walls(b, lift, 0.0, deck + 3.0, pat::CURTAIN);
        deck_top(b, lift, deck + 3.0);
    }
}

fn deck_top(b: &mut MeshBuilder, r: Rect, z: f32) {
    deck(b, r, z, pat::ROOF_FLAT);
}

// ---- a billboard ---------------------------------------------------------------------------

/// An LED billboard on a pole, its screen facing the road (+x): 6 by 3 m, its
/// underside 8.5 m up, a steel frame round it and a catwalk under it.
pub(super) fn billboard(b: &mut MeshBuilder, _tech: u8) {
    let (w, h, z) = (6.0, 3.0, 8.5);
    paint(b, pat::STEEL);
    if b.far() {
        facing(
            b,
            vec![
                v3(0.3, -w * 0.5, z),
                v3(0.3, w * 0.5, z),
                v3(0.3, w * 0.5, z + h),
                v3(0.3, -w * 0.5, z + h),
            ],
            Vec3::X,
        );
        return;
    }
    pole(b, Vec2::ZERO, 0.3, 0.25, -0.5, z, b.sides(8).min(6));
    boxed(
        b,
        v3(-0.2, -w * 0.5 - 0.15, z - 0.1),
        v3(0.25, w * 0.5 + 0.15, z + h + 0.1),
        pat::STEEL,
    );
    paint(b, pat::SCREEN);
    facing(
        b,
        vec![
            v3(0.27, -w * 0.5, z),
            v3(0.27, w * 0.5, z),
            v3(0.27, w * 0.5, z + h),
            v3(0.27, -w * 0.5, z + h),
        ],
        Vec3::X,
    );
    if b.fine() {
        boxed(
            b,
            v3(0.25, -w * 0.5, z - 0.25),
            v3(1.0, w * 0.5, z - 0.15),
            pat::STEEL,
        );
        boxed(b, v3(-0.4, -0.4, -0.3), v3(0.4, 0.4, 0.8), pat::CONCRETE);
    }
}

// ---- a car ---------------------------------------------------------------------------------

/// A civilian car, 4.6 m along x: a body, a glasshouse over it, four wheels. Its paint
/// is the instance's; worn past half, a burnt-out shell.
pub(super) fn car(b: &mut MeshBuilder, _tech: u8) {
    let (l, w) = (4.6, 1.85);
    paint(b, pat::CAR);
    if b.far() {
        b.cuboid_open(v3(0.0, 0.0, 0.7), v3(l, w, 1.0));
        return;
    }
    // The body in profile, chamfered nose and tail.
    let body = [
        [-l * 0.5, 0.3],
        [l * 0.5, 0.3],
        [l * 0.5, 0.75],
        [l * 0.5 - 0.35, 0.95],
        [-l * 0.5 + 0.2, 1.0],
        [-l * 0.5, 0.85],
    ];
    if b.coarse() {
        b.extrude_y(&body, -w * 0.5, w * 0.5);
        paint(b, pat::CAR_GLASS);
        b.frustum_open(
            v3(-0.25, 0.0, 0.98),
            Vec2::new(2.5, w - 0.15),
            Vec2::new(1.6, w - 0.35),
            0.5,
            Vec2::new(-0.15, 0.0),
        );
        return;
    }
    if !b.fine() {
        b.with_facets(|b| b.extrude_y(&body, -w * 0.5, w * 0.5));
        paint(b, pat::CAR_GLASS);
        let glass = [[-1.55, 0.98], [0.75, 0.98], [0.05, 1.47], [-1.25, 1.47]];
        b.with_facets(|b| b.extrude_y(&glass, -w * 0.5 + 0.08, w * 0.5 - 0.08));
        // Wheels: a dark block an axle.
        for x in [1.4f32, -1.4] {
            boxed(
                b,
                v3(x - 0.33, -w * 0.5 + 0.01, 0.0),
                v3(x + 0.33, w * 0.5 - 0.01, 0.66),
                pat::TYRE,
            );
        }
        return;
    }
    // Up close: a bonnet sloping to the nose, a boot, the flanks drawn in toward the
    // top (tumblehome), a glasshouse with raked pillars, arches over the wheels.
    let body = [
        [-2.3, 0.32],
        [2.3, 0.32],
        [2.33, 0.55],
        [2.25, 0.72],
        [1.9, 0.82],
        [0.85, 0.95],
        [-1.6, 1.0],
        [-2.2, 0.93],
        [-2.33, 0.72],
    ];
    b.with_facets(|b| b.extrude_y_chamfered(&body, w * 0.5, 0.14));
    paint(b, pat::CAR_GLASS);
    let glass = [[-1.62, 0.97], [0.85, 0.94], [0.1, 1.46], [-1.2, 1.48]];
    b.with_facets(|b| b.extrude_y_chamfered(&glass, w * 0.5 - 0.1, 0.2));
    boxed(
        b,
        v3(-1.12, -w * 0.5 + 0.3, 1.47),
        v3(0.05, w * 0.5 - 0.3, 1.5),
        pat::CAR,
    );
    // Bumpers, lamps.
    for (x0, x1) in [(2.26, 2.36), (-2.36, -2.26)] {
        boxed(
            b,
            v3(x0, -w * 0.5 + 0.05, 0.3),
            v3(x1, w * 0.5 - 0.05, 0.46),
            pat::TYRE,
        );
    }
    for y in [-0.65f32, 0.65] {
        paint(b, pat::CAR_GLASS);
        facing(
            b,
            vec![
                v3(2.3, y - 0.22, 0.6),
                v3(2.3, y + 0.22, 0.6),
                v3(2.25, y + 0.22, 0.7),
                v3(2.25, y - 0.22, 0.7),
            ],
            Vec3::new(1.0, 0.0, 0.4),
        );
    }
    // Wheels under arches.
    for (x, side) in [(1.4f32, 1.0f32), (1.4, -1.0), (-1.4, 1.0), (-1.4, -1.0)] {
        let y = side * (w * 0.5 - 0.12);
        paint(b, pat::TYRE);
        b.cylinder_between(v3(x, y - 0.11, 0.33), v3(x, y + 0.11, 0.33), 0.33, 0.33, 8);
        let face_y = side * (w * 0.5 + 0.005);
        let arch: Vec<Vec3> = (0..=6)
            .map(|k| {
                let a = std::f32::consts::PI * k as f32 / 6.0;
                v3(x + 0.42 * a.cos(), face_y, 0.32 + 0.42 * a.sin())
            })
            .collect();
        paint(b, pat::SHADOW);
        facing(b, arch, Vec3::Y * side);
    }
}

// ---- a barricade ---------------------------------------------------------------------------

/// A 16 m row of anti-tank hedgehogs, three steel beams crossed at their middles, with
/// sandbag walls between them. Dressing: troops walk through it.
pub(super) fn barricade(b: &mut MeshBuilder, _tech: u8) {
    let n = 4;
    let len = 16.0;
    for k in 0..n {
        let x = -len * 0.5 + (k as f32 + 0.5) * len / n as f32;
        let at = v3(x, 0.0, 0.85);
        let turn = hash_unit(3, k as u32) * 1.2;
        paint(b, pat::STEEL);
        let arms: &[Vec3] = if b.coarse() {
            &[Vec3::new(1.0, 0.0, 0.6)]
        } else {
            &[
                Vec3::new(1.0, 0.0, 0.6),
                Vec3::new(-0.5, 0.87, 0.6),
                Vec3::new(-0.5, -0.87, 0.6),
            ]
        };
        // Up close all three beams; further off two read as the cross.
        let shown = if b.fine() {
            arms.len()
        } else {
            arms.len().min(2)
        };
        for d in &arms[..shown] {
            let d = Vec2::from_angle(turn)
                .rotate(d.truncate())
                .extend(d.z)
                .normalize()
                * 1.1;
            b.beam(at - d, at + d, Vec2::splat(0.18), Vec2::splat(0.18));
        }
        // Sandbags between the hedgehogs.
        if k + 1 < n && b.mid() {
            let mid = x + len / n as f32 * 0.5;
            paint(b, pat::SANDBAG);
            for row in 0..if b.fine() { 3 } else { 1 } {
                let z = row as f32 * 0.3;
                let off = if row % 2 == 0 { 0.0 } else { 0.3 };
                b.chamfered_box(v3(mid + off, 0.0, z + 0.15), v3(2.2, 0.8, 0.3), 0.12);
            }
        }
    }
}

// ---- a wind turbine ------------------------------------------------------------------------

/// A modern wind turbine: a white tapering tower to a nacelle 95 m up, a nose cone
/// and three blades 45 m long, stopped.
pub(super) fn wind_turbine(b: &mut MeshBuilder, _tech: u8) {
    let (plan, hub) = part(PropKind::CityWindTurbine, 0);
    let foot = plan.size().x * 0.5;
    let sides = if b.fine() { 16 } else { 6 };
    paint(b, pat::WHITE_STEEL);
    if b.coarse() {
        pole(b, Vec2::ZERO, foot * 0.85, foot * 0.45, -1.0, hub - 1.0, 4);
    } else {
        b.prism(v3(0.0, 0.0, -1.0), sides, foot * 0.85, foot * 0.45, hub);
    }
    // The nacelle, the hub a little ahead of the tower, facing +x.
    boxed(
        b,
        v3(-5.0, -1.8, hub - 2.0),
        v3(4.0, 1.8, hub + 1.6),
        pat::WHITE_STEEL,
    );
    let nose = v3(4.0, 0.0, hub - 0.2);
    if b.fine() {
        paint(b, pat::WHITE_STEEL);
        b.cylinder_between(nose, nose + Vec3::X * 2.6, 1.6, 0.25, b.sides(10));
    }
    let blades = 3;
    for k in 0..blades {
        let a = 0.3 + k as f32 * std::f32::consts::TAU / blades as f32;
        let dir = Vec3::new(0.0, a.cos(), a.sin());
        let root = nose + Vec3::X * 0.8 + dir * 1.2;
        let tip = nose + Vec3::X * 0.8 + dir * 45.0;
        paint(b, pat::WHITE_STEEL);
        b.beam(root, tip, Vec2::new(2.6, 0.5), Vec2::new(0.6, 0.12));
    }
    if b.fine() {
        boxed(
            b,
            v3(-foot, -foot, -1.0),
            v3(foot, foot, 0.4),
            pat::CONCRETE,
        );
        b.paint(GLOW_NAV_RED);
        b.cuboid(v3(-4.4, 0.0, hub + 1.9), Vec3::splat(0.4));
    }
}

// ---- a solar array -------------------------------------------------------------------------

/// A solar field 40 by 24 m: rows of panels tilted to the sun on steel frames.
pub(super) fn solar_array(b: &mut MeshBuilder, _tech: u8) {
    let (lx, ly) = (40.0, 24.0);
    let rows = if b.coarse() { 3 } else { 6 };
    let pitch = ly / rows as f32;
    for k in 0..rows {
        let y0 = -ly * 0.5 + k as f32 * pitch + 0.3;
        let y1 = y0 + pitch * 0.62;
        paint(b, pat::SOLAR);
        facing(
            b,
            vec![
                v3(-lx * 0.5, y0, 0.6),
                v3(lx * 0.5, y0, 0.6),
                v3(lx * 0.5, y1, 2.0),
                v3(-lx * 0.5, y1, 2.0),
            ],
            Vec3::new(0.0, -0.5, 0.87),
        );
        if b.fine() {
            for x in cells(-lx * 0.5, lx, 4.0) {
                boxed(
                    b,
                    v3(x - 0.05, y0 + 0.2, -0.3),
                    v3(x + 0.05, y0 + 0.3, 0.7),
                    pat::STEEL,
                );
                boxed(
                    b,
                    v3(x - 0.05, y1 - 0.3, -0.3),
                    v3(x + 0.05, y1 - 0.2, 2.0),
                    pat::STEEL,
                );
            }
        }
    }
}

// ---- a comms mast --------------------------------------------------------------------------

/// A 110 m lattice comms mast: four legs tapering to the top, braced in X panels,
/// platforms with dishes and panel antennas, a whip on top, red aviation lights.
pub(super) fn mast_tower(b: &mut MeshBuilder, _tech: u8) {
    let (plan, top) = part(PropKind::CityMast, 0);
    let foot = plan.size().x * 0.5 - 0.3;
    let head = 1.0;
    let at = |z: f32| foot + (head - foot) * z / top;
    if b.coarse() {
        paint(b, pat::STEEL);
        b.frustum_open(
            v3(0.0, 0.0, -1.0),
            Vec2::splat(2.0 * foot),
            Vec2::splat(2.0 * head),
            top + 1.0,
            Vec2::ZERO,
        );
        return;
    }
    let corners = [(1.0f32, 1.0f32), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)];
    for (sx, sy) in corners {
        paint(b, pat::STEEL);
        b.beam(
            v3(sx * foot, sy * foot, -1.0),
            v3(sx * head, sy * head, top),
            Vec2::splat(0.35),
            Vec2::splat(0.2),
        );
    }
    let bays = if b.fine() { 14 } else { 0 };
    for k in 0..bays {
        let z0 = top * k as f32 / bays as f32;
        let z1 = top * (k + 1) as f32 / bays as f32;
        for i in 0..4 {
            let (ax, ay) = corners[i];
            let (bx, by) = corners[(i + 1) % 4];
            let p = |s: (f32, f32), z: f32| v3(s.0 * at(z), s.1 * at(z), z);
            paint(b, pat::STEEL);
            b.beam(
                p((ax, ay), z0),
                p((bx, by), z1),
                Vec2::splat(0.1),
                Vec2::splat(0.1),
            );
            b.beam(
                p((bx, by), z0),
                p((ax, ay), z1),
                Vec2::splat(0.1),
                Vec2::splat(0.1),
            );
        }
    }
    // Platforms, dishes and panel antennas.
    for (k, z) in [top * 0.45, top * 0.7, top * 0.9].iter().enumerate() {
        let r = at(*z) + 1.2;
        boxed(b, v3(-r, -r, z - 0.2), v3(r, r, *z), pat::STEEL);
        if b.mid() {
            let d = Vec2::from_angle(0.7 + k as f32 * 2.1);
            let c = (d * (r + 0.4)).extend(*z + 1.4);
            paint(b, pat::WHITE_STEEL);
            b.cylinder_between(c, c + d.extend(0.0) * 0.7, 1.3, 0.25, b.sides(10));
            for s in [-1.0f32, 1.0] {
                let p = (d.perp() * s * r).extend(*z);
                boxed(
                    b,
                    p - v3(0.25, 0.25, 0.0),
                    p + v3(0.25, 0.25, 2.6),
                    pat::WHITE_STEEL,
                );
            }
        }
    }
    mast(b, v3(0.0, 0.0, top), 6.0, 0.25);
    if b.mid() {
        b.paint(GLOW_NAV_RED);
        for z in [top * 0.5, top + 6.0] {
            b.cuboid(v3(0.0, 0.0, z + 0.3), Vec3::splat(0.5));
        }
    }
}

// ---- a monument ----------------------------------------------------------------------------

/// A memorial fountain: a round stone basin of water, a stepped plinth in it, and on
/// the plinth three bronze blades leaning together to a point 9 m up, a ring of light
/// round their foot. The colony's founders' memorial.
pub(super) fn monument(b: &mut MeshBuilder, _tech: u8) {
    let (plan, top) = part(PropKind::CityMonument, 0);
    let r = plan.size().x * 0.5 - 0.2;
    let sides = if b.fine() { 24 } else { 10 };
    // The basin's rim and its water, the plinth stepped up in it.
    paint(b, pat::STONE);
    if b.coarse() {
        pole(b, Vec2::ZERO, r, r, -1.0, 0.7, 6);
    } else {
        b.prism(v3(0.0, 0.0, -1.0), sides, r, r, 1.7);
        paint(b, pat::WATER);
        b.prism(v3(0.0, 0.0, 0.65), sides, r - 0.5, r - 0.5, 0.06);
        paint(b, pat::STONE);
        b.prism(v3(0.0, 0.0, 0.5), b.sides(8), 2.6, 2.6, 0.8);
        b.prism(v3(0.0, 0.0, 1.3), b.sides(8), 1.9, 1.9, 0.9);
    }
    // The blades: from the plinth's edge leaning in to meet at the top.
    let base = 2.2;
    for k in 0..3 {
        let a = 0.5 + k as f32 * std::f32::consts::TAU / 3.0;
        let foot = (Vec2::from_angle(a) * 1.3).extend(2.2);
        let tip = (Vec2::from_angle(a) * 0.12).extend(top);
        paint(b, pat::COPPER);
        b.beam(foot, tip, Vec2::new(0.9, 0.25), Vec2::new(0.15, 0.08));
    }
    if b.fine() {
        b.prism(v3(0.0, 0.0, base), b.sides(16), 1.95, 1.95, 0.08);
        paint(b, pat::LED);
        b.prism(v3(0.0, 0.0, base), b.sides(16), 1.97, 1.97, 0.06);
    }
}
