//! Skyguard: the tech 3 long-range SAM site. An armoured launcher block of four
//! cells forward on the pad, each under a hatch hinged on its outer edge
//! (`gpu_consts::cells`): the hatches swing up and out before a salvo, a missile
//! stands in every loaded cell, and the missiles are boosted straight up out of them.
//! The block is dug in behind sloped armoured berms on its flanks and front, with a
//! ramp up to its deck from behind and magazines at the back corners. Behind it stands
//! the fire-control deckhouse: a tapered eight-sided tower with a fixed array face on
//! each diagonal facet, laid flat on its facet. Nothing yaws.
use super::*;
use crate::builder::{ngon, Section};
use crate::gpu_consts::cells::{CENTRE, DECK, HALF, OFFSET};
use crate::pattern;
use glam::{Vec2, Vec3};

/// Half the launcher block's width: the cells' outer walls.
const BLOCK: f32 = OFFSET + HALF + 0.75;
/// Where the block stands, on the pad's top.
const FOOT: f32 = 0.9;
/// Half the pad's width.
const PAD: f32 = 10.2;
/// The deckhouse's middle, behind the block.
const TOWER: f32 = -5.7;
/// The deckhouse's plan at its foot, one quarter: a side across x, a diagonal facet
/// (where an array face sits), a side across y.
const TOWER_X: f32 = 2.7;
const TOWER_Y: f32 = 3.4;
const TOWER_SIDE_X: f32 = 1.0;
const TOWER_SIDE_Y: f32 = 1.2;
/// The deckhouse's height over its plinth, and how far its plan has drawn in at the top.
const TOWER_H: f32 = 7.6;
const TOWER_TOP: f32 = 0.74;

pub(super) fn build(b: &mut MeshBuilder) {
    if b.coarse() {
        coarse(b);
        return;
    }
    pad(b);
    block(b);
    cells(b);
    deckhouse(b);
    revetment(b);
}

/// Cell centres in the order the shader numbers them and the weapon's muzzles run
/// (`gpu_consts::cells`): back left, front right, back right, front left.
pub(super) fn centres() -> [(f32, f32); 4] {
    [
        (CENTRE - OFFSET, -OFFSET),
        (CENTRE + OFFSET, OFFSET),
        (CENTRE - OFFSET, OFFSET),
        (CENTRE + OFFSET, -OFFSET),
    ]
}

/// A rectangle `hx` by `hy` (halves) with its corners cut `cut` back, counterclockwise.
fn chamfered(hx: f32, hy: f32, cut: f32) -> Vec<[f32; 2]> {
    vec![
        [hx, -hy + cut],
        [hx, hy - cut],
        [hx - cut, hy],
        [-hx + cut, hy],
        [-hx, hy - cut],
        [-hx, -hy + cut],
        [-hx + cut, -hy],
        [hx - cut, -hy],
    ]
}

/// The pad: an octagonal plated slab over a darker kerb.
fn pad(b: &mut MeshBuilder) {
    let plan = chamfered(PAD, PAD, 2.4);
    b.paint(PLATING_DARK);
    b.loft_z(
        &plan,
        &[Section::new(0.0, 1.0), Section::new(FOOT - 0.25, 0.99)],
    );
    b.paint(PLATING);
    b.loft_z(
        &plan,
        &[Section::new(FOOT - 0.25, 0.975), Section::new(FOOT, 0.955)],
    );
}

/// The armoured block the cells are sunk in: a dark core whose inner faces are the
/// cells' walls, armour skin with ribs and vents on the outside, the booster uptake
/// between the hatches, rails round the deck.
fn block(b: &mut MeshBuilder) {
    let h = DECK - FOOT;
    let z = FOOT + h * 0.5;
    let inner = OFFSET - HALF;
    let wall = BLOCK - (OFFSET + HALF);
    b.paint(PLATING_DARK);
    b.cuboid(v3(CENTRE, 0.0, z), v3(BLOCK * 2.0, inner * 2.0, h));
    b.cuboid(v3(CENTRE, 0.0, z), v3(inner * 2.0, BLOCK * 2.0, h));
    // Further off the skin is left off, and the outer walls wear its plating.
    if !b.fine() {
        b.paint(PLATING);
    }
    for s in [-1.0, 1.0] {
        let at = s * (BLOCK - wall * 0.5);
        b.cuboid(v3(CENTRE + at, 0.0, z), v3(wall, BLOCK * 2.0, h));
        b.cuboid(v3(CENTRE, at, z), v3(BLOCK * 2.0, wall, h));
    }
    // The uptake between the cells, where a booster's exhaust is let out upward.
    b.paint(PLATING_DARK);
    b.cuboid(v3(CENTRE, 0.0, DECK + 0.15), v3(1.7, 1.7, 0.3));
    if !b.fine() {
        return;
    }
    vent(b, v3(CENTRE, 0.0, DECK + 0.3), v2(1.3, 1.3), 3, METAL);
    // Armour skin: a sloped glacis at the foot, then plates standing proud of the core
    // in two courses, ribs between the plates.
    let skin = BLOCK * 2.0 + 0.3;
    b.paint(PLATING);
    b.frustum(
        v3(CENTRE, 0.0, FOOT),
        v2(skin + 1.4, skin + 1.4),
        v2(skin, skin),
        1.4,
        v2(0.0, 0.0),
    );
    for s in [-1.0, 1.0] {
        b.paint(PLATING);
        for (z0, z1) in [(FOOT + 1.4, FOOT + 3.6), (FOOT + 3.75, DECK - 0.55)] {
            let (c, t) = ((z0 + z1) * 0.5, z1 - z0);
            b.cuboid(
                v3(CENTRE + s * (BLOCK + 0.1), 0.0, c),
                v3(0.2, BLOCK * 2.0 - 0.3, t),
            );
            b.cuboid(
                v3(CENTRE, s * (BLOCK + 0.1), c),
                v3(BLOCK * 2.0 - 0.3, 0.2, t),
            );
        }
        b.paint(PLATING_DARK);
        for k in [-1.0, 0.0, 1.0] {
            b.cuboid(
                v3(CENTRE + k * BLOCK * 0.62, s * (BLOCK + 0.22), FOOT + 3.4),
                v3(0.3, 0.3, 4.0),
            );
        }
        for k in [-1.0, 1.0] {
            b.cuboid(
                v3(CENTRE + s * (BLOCK + 0.22), k * BLOCK * 0.5, FOOT + 3.4),
                v3(0.3, 0.3, 4.0),
            );
        }
    }
    // A lip round the deck.
    b.paint(ACCENT);
    for s in [-1.0, 1.0] {
        b.cuboid(
            v3(CENTRE + s * (BLOCK + 0.12), 0.0, DECK - 0.3),
            v3(0.35, BLOCK * 2.0 + 0.6, 0.4),
        );
        b.cuboid(
            v3(CENTRE, s * (BLOCK + 0.12), DECK - 0.3),
            v3(BLOCK * 2.0 + 0.6, 0.35, 0.4),
        );
    }
    // The booster vents low on the flanks, louvred.
    b.mirror_y(|b| {
        for x in [CENTRE - OFFSET, CENTRE + OFFSET] {
            b.paint(PLATING_DARK);
            b.cuboid(v3(x, BLOCK + 0.3, FOOT + 1.9), v3(1.8, 0.3, 1.0));
            b.paint(METAL);
            for k in 0..2 {
                b.cuboid(
                    v3(x, BLOCK + 0.47, FOOT + 1.7 + k as f32 * 0.4),
                    v3(1.6, 0.06, 0.1),
                );
            }
        }
    });
    // The team's colour on the front plate.
    b.paint(TEAM);
    b.cuboid(
        v3(CENTRE + BLOCK + 0.23, 0.0, FOOT + 4.9),
        v3(0.04, 2.6, 1.2),
    );
    // Rails along the deck's flanks, clear of the hatches' swing to the front and back.
    b.paint(METAL);
    let top = DECK + 1.0;
    let edge = BLOCK - 0.15;
    let rail = v2(0.08, 0.08);
    for s in [-1.0, 1.0] {
        b.beam(
            v3(CENTRE - edge + 0.3, s * edge, top),
            v3(CENTRE + edge - 0.3, s * edge, top),
            rail,
            rail,
        );
        for k in 0..3 {
            let x = CENTRE - edge + 0.3 + k as f32 * (edge * 2.0 - 0.6) / 2.0;
            b.beam(v3(x, s * edge, DECK), v3(x, s * edge, top), rail, rail);
        }
    }
}

/// A hatch over each cell with its hinge on the outer edge, and the missile in it.
fn cells(b: &mut MeshBuilder) {
    for (cx, cy) in centres() {
        let side = (cx - CENTRE).signum();
        let hinge = CENTRE + side * (OFFSET + HALF);
        b.with_part(part::CELL_HATCH, |b| {
            b.paint(PLATING);
            b.cuboid(
                v3(cx, cy, DECK + 0.12),
                v3(HALF * 2.0 + 0.15, HALF * 2.0 + 0.15, 0.24),
            );
            if b.fine() {
                // Raised ribs across the lid, and the knuckles it swings on.
                b.paint(PLATING_DARK);
                for dy in [-0.45, 0.45] {
                    b.cuboid(v3(cx, cy + dy, DECK + 0.3), v3(HALF * 1.6, 0.18, 0.12));
                }
                b.paint(METAL);
                for dy in [-0.55, 0.55] {
                    b.cylinder_between(
                        v3(hinge, cy + dy - 0.2, DECK + 0.08),
                        v3(hinge, cy + dy + 0.2, DECK + 0.08),
                        0.14,
                        0.14,
                        4,
                    );
                }
            }
            b.paint(ACCENT);
            b.decal(v3(cx - side * 0.6, cy, DECK + 0.25), v2(0.22, HALF * 1.6));
        });
        b.with_part(part::CELL_ROUND, |b| {
            let top = DECK - 0.25;
            // Only its head shows over the cell's rim from further off.
            let foot = if b.fine() { top - 5.4 } else { top - 2.0 };
            b.paint(PLATING);
            b.cylinder_between(
                v3(cx, cy, foot),
                v3(cx, cy, top - 0.9),
                0.4,
                0.4,
                b.sides(6),
            );
            if b.fine() {
                b.paint(ACCENT);
                b.cylinder_between(
                    v3(cx, cy, top - 1.2),
                    v3(cx, cy, top - 0.9),
                    0.41,
                    0.41,
                    b.sides(6),
                );
            }
            b.paint(PLATING_DARK);
            b.cylinder_between(
                v3(cx, cy, top - 0.9),
                v3(cx, cy, top),
                0.4,
                0.05,
                b.sides(6),
            );
        });
    }
}

/// The deckhouse's plan at its foot, about its middle, counterclockwise.
fn tower_plan() -> Vec<[f32; 2]> {
    vec![
        [TOWER_X, -TOWER_SIDE_X],
        [TOWER_X, TOWER_SIDE_X],
        [TOWER_SIDE_Y, TOWER_Y],
        [-TOWER_SIDE_Y, TOWER_Y],
        [-TOWER_X, TOWER_SIDE_X],
        [-TOWER_X, -TOWER_SIDE_X],
        [-TOWER_SIDE_Y, -TOWER_Y],
        [TOWER_SIDE_Y, -TOWER_Y],
    ]
}

/// A section of the deckhouse's plan at `z`, scaled about its middle.
fn tower_section(z: f32, scale: f32) -> Section {
    Section {
        z,
        scale: Vec2::splat(scale),
        shift: Vec2::new(TOWER, 0.0),
    }
}

/// The fire-control deckhouse: a plinth, the tapered tower with an array face on each
/// diagonal facet, a roof with the director and its masts, a door and a stair.
fn deckhouse(b: &mut MeshBuilder) {
    let plan = tower_plan();
    let base = FOOT + 0.5;
    let top = base + TOWER_H;
    b.paint(PLATING_DARK);
    b.loft_z(
        &plan,
        &[tower_section(FOOT, 1.08), tower_section(base, 1.08)],
    );
    b.paint(PLATING);
    b.loft_z(
        &plan,
        &[tower_section(base, 1.0), tower_section(top, TOWER_TOP)],
    );
    // A roof course and the director standing on it.
    b.paint(PLATING_DARK);
    b.loft_z(
        &plan,
        &[
            tower_section(top, TOWER_TOP * 1.04),
            tower_section(top + 0.45, TOWER_TOP * 1.04),
        ],
    );
    b.paint(PLATING);
    b.chamfered_box(v3(TOWER - 0.3, 0.0, top + 1.25), v3(2.2, 2.6, 1.6), 0.3);
    b.paint(GLASS);
    b.cuboid(v3(TOWER + 0.82, 0.0, top + 1.45), v3(0.06, 1.9, 0.5));
    team_panel(b, v3(TOWER - 0.3, 0.0, top + 2.05), v2(1.2, 1.4));
    for (dx, dy) in [(1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)] {
        array_face(b, dx, dy, base, top);
    }
    if !b.fine() {
        return;
    }
    b.paint(METAL);
    b.cylinder_between(
        v3(TOWER - 1.0, 0.0, top + 2.05),
        v3(TOWER - 1.0, 0.0, top + 4.6),
        0.12,
        0.08,
        6,
    );
    b.cuboid(v3(TOWER - 1.0, 0.0, top + 4.0), v3(0.12, 1.4, 0.1));
    antenna_unlit(b, v3(TOWER - 1.5, 1.3, top + 0.45), 2.4, 0.05);
    antenna_unlit(b, v3(TOWER - 1.5, -1.3, top + 0.45), 1.8, 0.05);
    // The door on the back, down a short stair.
    b.paint(PLATING_DARK);
    b.cuboid(
        v3(TOWER - TOWER_X - 0.05, 0.0, base + 1.1),
        v3(0.25, 1.4, 2.2),
    );
    b.paint(PLATING).pattern(pattern::SHUTTER);
    b.cuboid(
        v3(TOWER - TOWER_X - 0.19, 0.0, base + 1.05),
        v3(0.06, 1.1, 2.0),
    );
    b.paint(METAL);
    for k in 0..3 {
        let f = k as f32;
        b.cuboid(
            v3(
                TOWER - TOWER_X - 0.75 - f * 0.35,
                0.0,
                base - 0.1 - f * 0.17,
            ),
            v3(0.35, 1.3, 0.12),
        );
    }
}

/// The array face on the deckhouse's facet toward (`dx`, `dy`): an octagonal panel laid
/// flat on the facet halfway up it, standing proud of it, the aperture inside.
fn array_face(b: &mut MeshBuilder, dx: f32, dy: f32, base: f32, top: f32) {
    let q = |p: [f32; 2]| Vec2::new(p[0] * dx, p[1] * dy);
    let (p1, p2) = (q([TOWER_X, TOWER_SIDE_X]), q([TOWER_SIDE_Y, TOWER_Y]));
    let mid = (p1 + p2) * 0.5;
    // The facet draws in toward the tower's axis going up.
    let shrink = (1.0 - TOWER_TOP) / (top - base);
    let z = base + (top - base) * 0.48;
    let scale = 1.0 - shrink * (z - base);
    let centre = (Vec2::new(TOWER, 0.0) + mid * scale).extend(z);
    let mut along = (p2 - p1).normalize().extend(0.0);
    let up = (-mid * shrink).extend(1.0).normalize();
    let mut out = along.cross(up);
    if out.truncate().dot(mid) < 0.0 {
        along = -along;
        out = -out;
    }
    let width = (p2 - p1).length() * scale;
    let r = (width * 0.5 - 0.15).min(1.3);
    let ring = |r: f32, off: f32| -> Vec<Vec3> {
        ngon(8, r)
            .into_iter()
            .map(|[a, u]| centre + out * off + along * a + up * u)
            .collect()
    };
    b.paint(PLATING_DARK);
    b.loft(
        &[ring(r, 0.02), ring(r, 0.18), ring(r * 0.92, 0.26)],
        false,
        true,
    );
    if b.fine() {
        b.paint(GLASS);
        b.face(&ring(r * 0.76, 0.27));
    }
}

/// Covered cable troughs over the pad from the deckhouse to the block, `y` either side.
fn conduits(b: &mut MeshBuilder, y: f32) {
    let (from, to) = (TOWER + TOWER_X + 0.3, CENTRE - BLOCK - 0.6);
    for s in [-1.0, 1.0] {
        b.paint(ACCENT).pattern(pattern::CONDUIT);
        b.cuboid(
            v3((from + to) * 0.5, s * y, FOOT + 0.12),
            v3(to - from, 0.5, 0.24),
        );
    }
}

/// The block dug in. Sloped armoured berms stand round its flanks and front to two
/// thirds of its height, a ramp runs up to its deck from behind, magazines stand at the
/// back corners.
fn revetment(b: &mut MeshBuilder) {
    let crest = FOOT + 4.0;
    let (inner, outer) = (BLOCK + 0.35, BLOCK + 3.6);
    let middle = Vec2::new(CENTRE, 0.0);
    for dir in [Vec2::X, Vec2::Y, -Vec2::Y] {
        let across = dir.perp();
        // A point `d` out from the block's middle toward `dir`, `a` along the berm; each
        // berm reaches as far along as it stands out, so the next meets it on the diagonal.
        let p = |d: f32, a: f32, z: f32| (middle + dir * d + across * a * d).extend(z);
        let (i0, i1) = (inner, inner + 0.7);
        let (c1, o) = (inner + 1.7, outer);
        b.paint(PLATING);
        // The steep inner face, the crest, the long glacis, and the two ends.
        b.face(&[
            p(i0, 1.0, FOOT),
            p(i0, -1.0, FOOT),
            p(i1, -1.0, crest),
            p(i1, 1.0, crest),
        ]);
        b.paint(PLATING_DARK);
        b.face(&[
            p(i1, 1.0, crest),
            p(i1, -1.0, crest),
            p(c1, -1.0, crest),
            p(c1, 1.0, crest),
        ]);
        b.paint(PLATING);
        b.face(&[
            p(c1, 1.0, crest),
            p(c1, -1.0, crest),
            p(o, -1.0, FOOT),
            p(o, 1.0, FOOT),
        ]);
        for e in [-1.0, 1.0] {
            let end = [
                p(i0, e, FOOT),
                p(i1, e, crest),
                p(c1, e, crest),
                p(o, e, FOOT),
            ];
            if e > 0.0 {
                b.face(&end);
            } else {
                b.face(&[end[3], end[2], end[1], end[0]]);
            }
        }
        if b.fine() {
            // A striped coping along the crest.
            let d = (i1 + c1) * 0.5;
            let len = d * 1.8;
            b.paint(PLATING).pattern(pattern::HAZARD);
            let size = if dir.x != 0.0 {
                v3(0.5, len, 0.06)
            } else {
                v3(len, 0.5, 0.06)
            };
            b.cuboid((middle + dir * d).extend(crest + 0.03), size);
        }
    }
    if !b.fine() {
        return;
    }
    // Magazines at the back corners, half sunk, their doors toward the block.
    b.mirror_y(|b| {
        let at = v3(TOWER + 1.2, 7.4, FOOT);
        b.paint(PLATING);
        b.frustum(at, v2(4.2, 3.0), v2(3.4, 2.2), 2.2, v2(0.0, 0.0));
        b.paint(PLATING_DARK);
        b.cuboid(at + v3(1.95, 0.0, 0.9), v3(0.2, 1.4, 1.6));
        b.paint(PLATING).pattern(pattern::SHUTTER);
        b.cuboid(at + v3(2.06, 0.0, 0.85), v3(0.05, 1.2, 1.4));
        b.paint(ACCENT);
        b.cuboid(at + v3(0.0, 0.0, 2.3), v3(2.6, 1.6, 0.2));
    });
    // The ramp up the back of the block to its deck.
    b.paint(PLATING).pattern(pattern::WALKWAY);
    let (x0, x1) = (CENTRE - BLOCK - 3.4, CENTRE - BLOCK - 0.3);
    b.face(&[
        v3(x0, -0.9, FOOT + 0.02),
        v3(x1, -0.9, DECK - 0.4),
        v3(x1, 0.9, DECK - 0.4),
        v3(x0, 0.9, FOOT + 0.02),
    ]);
    conduits(b, 1.9);
}

/// From far off: pad, block, the four hatches, the deckhouse as a tapered box.
fn coarse(b: &mut MeshBuilder) {
    b.paint(PLATING_DARK);
    b.cuboid_open(v3(0.0, 0.0, FOOT * 0.5), v3(PAD * 2.0, PAD * 2.0, FOOT));
    b.paint(PLATING);
    b.cuboid_open(
        v3(CENTRE, 0.0, (FOOT + DECK) * 0.5),
        v3(BLOCK * 2.0, BLOCK * 2.0, DECK - FOOT),
    );
    for (cx, cy) in centres() {
        b.with_part(part::CELL_HATCH, |b| {
            b.paint(ACCENT);
            b.decal(v3(cx, cy, DECK + 0.02), v2(HALF * 2.0, HALF * 2.0));
        });
    }
    b.frustum_open(
        v3(TOWER, 0.0, FOOT),
        v2(TOWER_X * 2.0, TOWER_Y * 2.0),
        v2(TOWER_X * 2.0 * TOWER_TOP, TOWER_Y * 2.0 * TOWER_TOP),
        TOWER_H + 0.5,
        v2(0.0, 0.0),
    );
    team_panel(
        b,
        v3(TOWER, 0.0, FOOT + TOWER_H + 0.5),
        v2(TOWER_X, TOWER_Y),
    );
}
