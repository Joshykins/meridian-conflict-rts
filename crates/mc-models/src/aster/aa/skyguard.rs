//! Skyguard: the tech 3 long-range SAM site. An armoured launcher block of four
//! cells on the pad, each under a hatch hinged on its outer edge (`gpu_consts::cells`):
//! the hatches swing up and out before a salvo, a missile stands in every loaded cell,
//! and the missiles are boosted straight up out of them. Fire control stands behind the
//! block. Nothing yaws.
use super::*;
use crate::gpu_consts::cells::{DECK, HALF, OFFSET};
use glam::Vec3;

/// Half the launcher block's width: the cells' outer walls.
const BLOCK: f32 = OFFSET + HALF + 0.75;
/// Where the block stands on the pad.
const FOOT: f32 = 1.2;

/// How the fire control behind the block is built.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Look {
    /// A cabinet and a mast with a turning phased-array panel on top.
    Mast,
    /// A tapered deckhouse carrying fixed array faces, one to each quarter.
    Array,
    /// A tall mast with a radome, the cabinet at its foot.
    Dome,
}

pub(super) fn build(b: &mut MeshBuilder, look: Look) {
    if b.coarse() {
        coarse(b);
        return;
    }
    platform(b, 11.5);
    block(b);
    cells(b);
    match look {
        Look::Mast => mast(b),
        Look::Array => array(b),
        Look::Dome => dome(b),
    }
}

/// The armoured block the cells are sunk in: a dark core, the cells' walls, skinned in
/// plating with a lip round the deck.
fn block(b: &mut MeshBuilder) {
    let h = DECK - FOOT;
    let z = FOOT + h * 0.5;
    let inner = OFFSET - HALF;
    let wall = BLOCK - (OFFSET + HALF);
    // The cross between the cells and the four outer walls: their inner faces are the
    // cells' walls, dark all the way down.
    b.paint(PLATING_DARK);
    b.cuboid(v3(0.0, 0.0, z), v3(BLOCK * 2.0, inner * 2.0, h));
    b.cuboid(v3(0.0, 0.0, z), v3(inner * 2.0, BLOCK * 2.0, h));
    // Further off the skin is left off, and the outer walls wear its plating.
    if !b.fine() {
        b.paint(PLATING);
    }
    for s in [-1.0, 1.0] {
        let at = s * (BLOCK - wall * 0.5);
        b.cuboid(v3(at, 0.0, z), v3(wall, BLOCK * 2.0, h));
        b.cuboid(v3(0.0, at, z), v3(BLOCK * 2.0, wall, h));
    }
    if !b.fine() {
        return;
    }
    // The cells' floors, far down.
    b.paint(METAL);
    for (cx, cy) in centres() {
        b.cuboid(v3(cx, cy, FOOT + 0.3), v3(HALF * 2.0, HALF * 2.0, 0.2));
    }
    // Armour skin on the outside, sloped in at the foot, and a plated deck lip.
    b.paint(PLATING);
    b.frustum(
        v3(0.0, 0.0, FOOT - 0.2),
        v2(BLOCK * 2.0 + 1.6, BLOCK * 2.0 + 1.6),
        v2(BLOCK * 2.0 + 0.3, BLOCK * 2.0 + 0.3),
        1.6,
        v2(0.0, 0.0),
    );
    for s in [-1.0, 1.0] {
        b.cuboid(
            v3(s * (BLOCK + 0.1), 0.0, z + 0.4),
            v3(0.2, BLOCK * 2.0 + 0.4, h - 0.8),
        );
        b.cuboid(
            v3(0.0, s * (BLOCK + 0.1), z + 0.4),
            v3(BLOCK * 2.0, 0.2, h - 0.8),
        );
    }
    b.paint(ACCENT);
    for s in [-1.0, 1.0] {
        b.cuboid(
            v3(s * (BLOCK + 0.12), 0.0, DECK - 0.35),
            v3(0.3, BLOCK * 2.0 + 0.5, 0.3),
        );
        b.cuboid(
            v3(0.0, s * (BLOCK + 0.12), DECK - 0.35),
            v3(BLOCK * 2.0 + 0.5, 0.3, 0.3),
        );
    }
    {
        // Exhaust vents low on the flanks, where the booster's blast is let out.
        b.paint(PLATING_DARK);
        b.mirror_y(|b| {
            for x in [-OFFSET, OFFSET] {
                b.cuboid(v3(x, BLOCK + 0.25, FOOT + 1.2), v3(1.6, 0.3, 0.9));
            }
        });
        team_panel(b, v3(BLOCK - 0.9, 0.0, DECK), v2(1.0, 1.6));
    }
}

/// Cell centres in the order the shader numbers them and the weapon's muzzles run
/// (`gpu_consts::cells`): back left, front right, back right, front left.
pub(super) fn centres() -> [(f32, f32); 4] {
    [
        (-OFFSET, -OFFSET),
        (OFFSET, OFFSET),
        (-OFFSET, OFFSET),
        (OFFSET, -OFFSET),
    ]
}

/// A hatch over each cell with its hinge on the outer edge, and the missile in it.
fn cells(b: &mut MeshBuilder) {
    for (cx, cy) in centres() {
        let side = cx.signum();
        b.with_part(part::CELL_HATCH, |b| {
            b.paint(PLATING);
            b.cuboid(
                v3(cx, cy, DECK + 0.12),
                v3(HALF * 2.0 + 0.15, HALF * 2.0 + 0.15, 0.24),
            );
            if b.fine() {
                // A raised rib across the lid, and the knuckles it swings on.
                b.paint(PLATING_DARK);
                b.cuboid(v3(cx, cy, DECK + 0.3), v3(HALF * 1.6, 0.3, 0.12));
                b.paint(METAL);
                for dy in [-0.55, 0.55] {
                    b.cylinder_between(
                        v3(side * (OFFSET + HALF), cy + dy - 0.2, DECK + 0.08),
                        v3(side * (OFFSET + HALF), cy + dy + 0.2, DECK + 0.08),
                        0.14,
                        0.14,
                        6,
                    );
                }
            }
            b.paint(ACCENT);
            b.decal(v3(cx - side * 0.55, cy, DECK + 0.25), v2(0.25, HALF * 1.6));
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
                b.sides(8),
            );
            if b.fine() {
                b.paint(ACCENT);
                b.cylinder_between(
                    v3(cx, cy, top - 1.2),
                    v3(cx, cy, top - 0.9),
                    0.41,
                    0.41,
                    b.sides(8),
                );
            }
            b.paint(PLATING_DARK);
            b.cylinder_between(
                v3(cx, cy, top - 0.9),
                v3(cx, cy, top),
                0.4,
                0.05,
                b.sides(8),
            );
        });
    }
}

/// A cabinet behind the block with a window onto the pad, and the team's colour.
fn cabinet(b: &mut MeshBuilder, x: f32, size: Vec3) {
    b.paint(PLATING_DARK);
    b.chamfered_box(v3(x, 0.0, FOOT + size.z * 0.5), size, 0.3);
    b.paint(GLASS);
    b.cuboid(
        v3(x + size.x * 0.5 + 0.02, 0.0, FOOT + size.z * 0.62),
        v3(0.1, size.y * 0.7, size.z * 0.3),
    );
    team_panel(
        b,
        v3(x, 0.0, FOOT + size.z),
        v2(size.x * 0.6, size.y * 0.45),
    );
}

/// A: a cabinet and a mast with a phased-array panel turning on top.
fn mast(b: &mut MeshBuilder) {
    let x = -BLOCK - 2.6;
    cabinet(b, x, v3(3.2, 4.4, 3.4));
    b.paint(PLATING);
    b.prism(v3(x, 0.0, FOOT + 3.4), b.sides(8), 0.9, 0.6, 6.2);
    let top = FOOT + 9.6;
    b.set_spinner_pivot(v3(x, 0.0, top));
    b.with_part(part::SPINNER, |b| {
        b.paint(PLATING_DARK);
        b.prism(v3(x, 0.0, top), b.sides(8), 0.8, 0.8, 0.5);
        // The panel leans back, facing out and up.
        b.paint(PLATING);
        b.beam(
            v3(x + 0.3, 0.0, top + 0.5),
            v3(x - 0.3, 0.0, top + 2.9),
            v2(0.35, 3.4),
            v2(0.35, 3.4),
        );
        b.paint(GLASS);
        b.beam(
            v3(x + 0.49, 0.0, top + 0.65),
            v3(x - 0.11, 0.0, top + 2.75),
            v2(0.05, 3.0),
            v2(0.05, 3.0),
        );
    });
}

/// B: a tapered deckhouse behind the block with a fixed array face to each quarter.
fn array(b: &mut MeshBuilder) {
    let x = -BLOCK - 2.9;
    b.paint(PLATING);
    b.frustum(
        v3(x, 0.0, FOOT),
        v2(4.6, 7.0),
        v2(3.0, 4.6),
        8.4,
        v2(0.0, 0.0),
    );
    b.paint(PLATING_DARK);
    b.cuboid(v3(x, 0.0, FOOT + 8.7), v3(2.4, 3.6, 0.6));
    // Octagonal faces on the four slopes, looking out diagonally.
    for (dx, dy) in [(1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)] {
        let out = Vec3::new(dx * 0.55, dy, 0.28).normalize();
        let at = v3(x + dx * 1.35, dy * 2.25, FOOT + 5.2);
        b.paint(PLATING_DARK);
        b.cylinder_between(at, at + out * 0.25, 1.35, 1.35, 8);
        if b.fine() {
            b.paint(GLASS);
            b.cylinder_between(at + out * 0.25, at + out * 0.3, 1.1, 1.1, 8);
        }
    }
    team_panel(b, v3(x, 0.0, FOOT + 9.0), v2(1.4, 2.2));
    if b.fine() {
        b.paint(METAL);
        antenna(b, v3(x - 0.6, 1.2, FOOT + 9.0), 2.6, 0.05);
    }
}

/// C: a tall mast with a radome, the cabinet at its foot.
fn dome(b: &mut MeshBuilder) {
    let x = -BLOCK - 2.6;
    cabinet(b, x, v3(3.2, 5.0, 2.8));
    b.paint(METAL);
    b.prism(v3(x, 0.0, FOOT + 2.8), b.sides(8), 0.55, 0.45, 7.0);
    b.paint(PLATING_DARK);
    b.prism(v3(x, 0.0, FOOT + 9.6), b.sides(8), 1.2, 1.5, 0.5);
    b.paint(PLATING);
    b.spheroid(v3(x, 0.0, FOOT + 11.4), v3(2.0, 2.0, 1.8), b.sides(12), 6);
}

/// From far off: pad, block, the four hatches, the fire control as a box and a post.
fn coarse(b: &mut MeshBuilder) {
    b.paint(PLATING_DARK);
    b.cuboid_open(v3(0.0, 0.0, 0.6), v3(19.55, 19.55, 1.2));
    b.paint(PLATING);
    b.cuboid_open(
        v3(0.0, 0.0, (FOOT + DECK) * 0.5),
        v3(BLOCK * 2.0, BLOCK * 2.0, DECK - FOOT),
    );
    for (cx, cy) in centres() {
        b.with_part(part::CELL_HATCH, |b| {
            b.paint(ACCENT);
            b.decal(v3(cx, cy, DECK + 0.02), v2(HALF * 2.0, HALF * 2.0));
        });
    }
    b.paint(PLATING_DARK);
    b.cuboid_open(v3(-BLOCK - 2.6, 0.0, FOOT + 1.7), v3(3.2, 4.4, 3.4));
    b.paint(PLATING);
    b.prism(v3(-BLOCK - 2.6, 0.0, FOOT + 3.4), 4, 0.8, 0.6, 7.0);
    team_panel(b, v3(-BLOCK - 2.6, 0.0, FOOT + 3.4), v2(2.0, 2.0));
}
