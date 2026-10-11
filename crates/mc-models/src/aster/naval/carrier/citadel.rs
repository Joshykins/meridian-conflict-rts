//! The command plateau over the stern quarter and the citadel on it: the bridge stands
//! aft. The plateau's outline zig-zags in steps down both sides, from the full beam at
//! the stern to a tongue over the trench, edged all round in a bronze band; the trench
//! runs on under it as a tunnel to the landing mouth. The citadel climbs in terraces to
//! an armoured house with slots in its faces, the radar and two masts on its roof; plated
//! steps lead down from it forward toward the tunnel's skylights, and round pads stand at
//! the plateau's outer corners.

use super::*;

/// The citadel's house: its roof, where the radar, masts and laser stand.
pub(super) const HOUSE_ROOF: f32 = 18.6;
/// The citadel's middle.
const CITADEL: Vec2 = Vec2::new(-41.0, 0.0);

/// The plateau's outline, counter-clockwise from the stern's port corner: a stern face
/// closing in with the hull, then forward down each side in steps and 45 degree cuts to
/// the tongue over the tunnel.
const OUTLINE: [[f32; 2]; 16] = [
    [-55.0, 10.4],
    [-55.0, -10.4],
    [-50.0, -13.2],
    [-42.0, -15.4],
    [-34.0, -15.4],
    [-30.0, -12.6],
    [-24.0, -12.6],
    [-20.0, -8.6],
    [PLATEAU_FORE, -8.6],
    [PLATEAU_FORE, 8.6],
    [-20.0, 8.6],
    [-24.0, 12.6],
    [-30.0, 12.6],
    [-34.0, 15.4],
    [-42.0, 15.4],
    [-50.0, 13.2],
];
/// The bronze band's width round the plateau's top.
const TRIM: f32 = 1.0;

pub(super) fn build(b: &mut MeshBuilder) {
    plateau(b);
    terraces(b);
    house(b);
    steps(b);
    kit::fleet_radar(b, v3(CITADEL.x + 4.0, 0.0, HOUSE_ROOF), HOUSE_ROOF + 3.0);
    if b.mid() {
        for y in [-2.6, 2.6] {
            antenna_unlit(b, v3(CITADEL.x - 4.4, y, HOUSE_ROOF), 5.0, 0.05);
        }
    }
    for y in [-10.8, 10.8] {
        kit::pad(b, v3(-48.5, y, PLATEAU), 2.4);
    }
    // Round the terraces, each fitting its own: a long housing and a hatch to starboard,
    // a service bay, a blister and a cover to port.
    kit::housing(b, v3(-40.0, -12.8, PLATEAU), v2(6.0, 2.4), 1.1);
    kit::dome(b, v3(-25.0, -10.0, PLATEAU), 0.8);
    kit::bay(b, v3(-38.5, 12.8, PLATEAU), v2(4.4, 3.0));
    kit::dome(b, v3(-44.5, 13.0, PLATEAU), 1.3);
    kit::cover(b, v3(-27.0, 10.2, PLATEAU), v2(2.4, 2.0));
}

/// The plateau: a bronze-sided slab whose top is laid in white plate inside a bronze
/// band round its edge, standing over the tunnel.
fn plateau(b: &mut MeshBuilder) {
    b.paint(PLATING).pattern(pattern::TRIM);
    b.extrude_z(&OUTLINE, STERN_DECK - 0.1, PLATEAU - 0.06);
    let inset = offset(&OUTLINE, TRIM);
    b.paint(PLATING).pattern(pattern::GENERIC);
    b.extrude_z(&inset, PLATEAU - 0.3, PLATEAU);
}

/// The citadel's terraces: two drawn-in tiers standing on the plateau, a coping between.
fn terraces(b: &mut MeshBuilder) {
    b.at(CITADEL.extend(0.0), |b| {
        b.paint(PLATING).pattern(pattern::HULL_PLATE);
        b.loft_z(
            &chamfered_rect(v2(12.0, 10.0), 3.0),
            &[
                Section::new(PLATEAU - 0.1, 1.0),
                Section::scaled(13.2, 0.88, 0.84),
            ],
        );
        b.paint(PLATING).pattern(pattern::GENERIC);
        b.loft_z(
            &chamfered_rect(v2(10.6, 8.4), 2.6),
            &[Section::new(13.1, 1.0), Section::new(13.4, 0.98)],
        );
        b.paint(PLATING).pattern(pattern::HULL_PLATE);
        b.loft_z(
            &chamfered_rect(v2(8.6, 6.4), 2.0),
            &[Section::new(13.3, 1.0), Section::scaled(15.6, 0.86, 0.82)],
        );
    });
}

/// The armoured house on top: a faceted block drawn in as it rises, a raised spine on its
/// roof, a nose stepping down forward toward the deck, and dark slots cut in its faces.
fn house(b: &mut MeshBuilder) {
    let plan = chamfered_rect(v2(7.2, 4.6), 1.6);
    b.at(CITADEL.extend(0.0), |b| {
        b.paint(PLATING).pattern(pattern::GENERIC);
        b.loft_z(
            &plan,
            &[
                Section::new(15.5, 1.0),
                Section::scaled(HOUSE_ROOF - 0.6, 0.96, 0.9),
                Section::scaled(HOUSE_ROOF, 0.9, 0.82),
            ],
        );
    });
    // The nose: a wedge stepping down forward from the house onto the upper terrace.
    b.paint(PLATING).pattern(pattern::GENERIC);
    b.loft(
        &[
            nose_ring(CITADEL.x + 6.4, 3.0, HOUSE_ROOF - 0.4),
            nose_ring(CITADEL.x + 10.0, 2.6, 15.6),
        ],
        true,
        true,
    );
    // The roof's spine.
    b.chamfered_box(
        v3(CITADEL.x - 1.0, 0.0, HOUSE_ROOF + 0.35),
        v3(6.0, 2.4, 0.7),
        0.5,
    );
    if !b.mid() {
        return;
    }
    // Slots: a row across the nose's face and down each flank of the house.
    b.paint(ACCENT).pattern(pattern::PLAIN);
    for k in 0..3 {
        let y = -1.0 + k as f32;
        b.beam(
            v3(CITADEL.x + 8.6, y, 16.4),
            v3(CITADEL.x + 7.0, y, 17.9),
            v2(0.3, 0.12),
            v2(0.3, 0.12),
        );
    }
    for side in [-1.0, 1.0] {
        for k in 0..4 {
            let x = CITADEL.x - 3.0 + 2.0 * k as f32;
            b.cuboid(v3(x, side * 4.4, 17.0), v3(1.2, 0.12, 0.35));
        }
    }
}

/// One ring of the house's nose at `x`: `half` wide, standing from the terrace to `top`,
/// port first (the nose runs forward, so the rings wind as the loft wants).
fn nose_ring(x: f32, half: f32, top: f32) -> Vec<Vec3> {
    [
        (-half, 15.5),
        (-half, top - 0.5),
        (-half + 0.6, top),
        (half - 0.6, top),
        (half, top - 0.5),
        (half, 15.5),
    ]
    .iter()
    .map(|&(y, z)| v3(x, -y, z))
    .collect()
}

/// Three steps of different depths and widths leading down forward from the citadel
/// toward the tunnel's skylights.
fn steps(b: &mut MeshBuilder) {
    b.paint(PLATING).pattern(pattern::HULL_PLATE);
    for (x1, depth, rise, half) in [
        (-21.0, 3.2, 0.5, 4.6),
        (-24.2, 2.6, 1.1, 3.8),
        (-26.8, 2.4, 1.8, 3.0),
    ] {
        b.block(
            v3(x1 - depth, -half, PLATEAU - 0.1),
            v3(x1, half, PLATEAU + rise),
        );
    }
}

/// `outline` (counter-clockwise) moved in by `by` metres along each edge's normal.
fn offset(outline: &[[f32; 2]], by: f32) -> Vec<[f32; 2]> {
    let n = outline.len();
    let p = |i: usize| Vec2::from(outline[i % n]);
    (0..n)
        .map(|i| {
            let (a, c, d) = (p(i + n - 1), p(i), p(i + 1));
            // Inward normals of the edges in and out of the corner.
            let n0 = (c - a).perp().normalize();
            let n1 = (d - c).perp().normalize();
            let m = n0 + n1;
            (c + m * (by / (1.0 + n0.dot(n1)))).to_array()
        })
        .collect()
}
