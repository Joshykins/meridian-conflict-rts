//! The Dominion's stern: a staggered cluster of drives rather than a flat row. Two big
//! mains stand furthest aft and lowest, each in an armoured pod of its own under the hull,
//! a structural gantry between them; smaller auxiliaries sit higher and further forward,
//! one pair let into the hull's raked stern, one pair slung outboard under the chine.
//! Armour collars round every drive, exhaust shrouds over the mains, heat-sink fin banks
//! and coolant runs along the pods.
//!
//! The mains are the rig's drives (`RIG.drives`, `capital::drive_deep`, their vanes and
//! glow animated by `entity.wgsl`). The auxiliaries are local faceted bells without
//! `part::DRIVE` (the rig carries one pair of drives), listed in `NOZZLES` with the mains
//! so the exhaust effects fire from them; `AUX` holds them for a per-nozzle rig later.

use super::hull::HULL_BOTTOM;
use super::*;

/// The two mains: mouth x, |y|, axis height, size (1: the Bastion's 12 m bells).
pub(super) const MAIN_X: f32 = -246.0;
pub(super) const MAIN_Y: f32 = 20.0;
pub(super) const MAIN_Z: f32 = 46.0;
pub(super) const MAIN_SIZE: f32 = 1.3;
/// The auxiliaries (port; mirrored): mouth x, |y|, axis height, size.
pub(super) const AUX: [[f32; 4]; 2] = [[-234.0, 44.0, 74.0, 0.6], [-224.0, 64.0, 63.0, 0.5]];
/// The mains' pods: half width and half height about each main's axis, from their aft
/// face forward into the engineering block.
const POD: Vec2 = Vec2::new(19.0, 18.0);
const POD_AFT: f32 = -224.0;
const POD_FORE: f32 = -172.0;

pub(super) fn build(b: &mut MeshBuilder) {
    for s in [1.0f32, -1.0] {
        capital::drive_deep(b, v3(MAIN_X, s * MAIN_Y, MAIN_Z), MAIN_SIZE);
    }
    b.mirror_y(|b| {
        main_pod(b);
        for [x, y, z, size] in AUX {
            aux_drive(b, v3(x, y, z), size);
        }
    });
    gantry(b);
    transom(b);
}

/// The hull's stern face, stepped and broken up: two armoured ledges standing aft of it
/// across its width, ribs dividing it into bays, louvred vent panels in the bays, a heavy
/// armoured hatch on the centre line, heat-sink banks under the lower ledge over the pods.
fn transom(b: &mut MeshBuilder) {
    let fine = b.fine();
    let face = STERN;
    // The ledges, the lower broader and deeper.
    b.paint(PLATING).pattern(pattern::WARSHIP);
    b.loft(
        &[
            vec![
                v3(face + 1.0, -60.0, 68.0),
                v3(face + 1.0, 60.0, 68.0),
                v3(face + 1.0, 60.0, 73.0),
                v3(face + 1.0, -60.0, 73.0),
            ],
            vec![
                v3(face - 5.0, -56.0, 69.0),
                v3(face - 5.0, 56.0, 69.0),
                v3(face - 5.0, 56.0, 71.5),
                v3(face - 5.0, -56.0, 71.5),
            ],
        ],
        true,
        true,
    );
    b.paint(PLATING_DARK).pattern(pattern::WARSHIP);
    b.loft(
        &[
            vec![
                v3(face + 1.0, -50.0, 84.0),
                v3(face + 1.0, 50.0, 84.0),
                v3(face + 1.0, 50.0, 88.0),
                v3(face + 1.0, -50.0, 88.0),
            ],
            vec![
                v3(face - 3.0, -46.0, 84.6),
                v3(face - 3.0, 46.0, 84.6),
                v3(face - 3.0, 46.0, 86.8),
                v3(face - 3.0, -46.0, 86.8),
            ],
        ],
        true,
        true,
    );
    // Ribs dividing the face into bays, clear of the auxiliaries' housings.
    b.paint(PLATING).pattern(pattern::PLAIN);
    for y in [-58.0, -28.0, -12.0, 12.0, 28.0, 58.0] {
        b.block(v3(face - 2.2, y - 1.2, 72.5), v3(face + 1.0, y + 1.2, 84.5));
    }
    // The centre hatch in a heavy frame.
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.block(v3(face - 1.0, -10.0, 73.0), v3(face + 1.0, 10.0, 84.0));
    b.paint(PLATING_DARK).pattern(pattern::WARSHIP);
    b.block(v3(face - 1.6, -8.0, 74.0), v3(face + 0.5, 8.0, 83.0));
    // Louvred vents in the bays either side of the hatch.
    b.mirror_y(|b| {
        b.paint(TREAD).pattern(pattern::NONE);
        b.block(v3(face - 0.4, 14.0, 74.5), v3(face + 0.5, 26.0, 82.5));
        if fine {
            b.paint(METAL).pattern(pattern::PLAIN);
            let mut z = 75.5;
            while z < 82.0 {
                b.block(v3(face - 1.4, 14.5, z), v3(face - 0.2, 25.5, z + 0.6));
                z += 1.8;
            }
        }
    });
    // Heat-sink banks under the lower ledge, over the pods' aft ends.
    b.mirror_y(|b| {
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.block(v3(face - 4.0, 4.0, 64.0), v3(face + 1.0, 36.0, 68.2));
        if fine {
            b.paint(METAL).pattern(pattern::PLAIN);
            let mut y = 6.0;
            while y < 35.0 {
                b.block(
                    v3(face - 6.0, y - 0.35, 64.4),
                    v3(face - 3.8, y + 0.35, 68.0),
                );
                y += 2.6;
            }
        }
    });
}

/// The port main's pod (mirrored): a hard-faceted armoured nacelle round the drive's can,
/// raked into the engineering block forward; an armour collar round the bell's neck, an
/// exhaust shroud over the bell, a heat-sink fin bank down its outboard side between
/// coolant runs.
fn main_pod(b: &mut MeshBuilder) {
    let fine = b.fine();
    let c = v2(MAIN_Y, MAIN_Z);
    let ring = |x: f32, s: f32| {
        chamfered_rect(POD * s, 7.0 * s)
            .iter()
            .map(|p| v3(x, c.x + p[0], c.y + p[1]))
            .collect::<Vec<_>>()
    };
    b.paint(PLATING_DARK).pattern(pattern::WARSHIP);
    b.loft(
        &[
            ring(POD_AFT, 0.88),
            ring(POD_AFT + 5.0, 1.0),
            ring(POD_FORE - 12.0, 1.0),
            ring(POD_FORE, 0.8),
        ],
        true,
        true,
    );
    // Armour courses on the pod: a light belt round its middle, dark bands at its ends.
    b.paint(PLATING).pattern(pattern::WARSHIP);
    b.loft(&[ring(-214.0, 1.06), ring(-192.0, 1.06)], true, true);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.loft(
        &[ring(POD_AFT + 3.0, 1.04), ring(POD_AFT + 5.5, 1.04)],
        true,
        true,
    );
    // The nacelle cowl: a heavy armour plate over the pod's outboard upper quarter, stepped
    // proud of the belt, and a chin plate under it.
    let cowl = |x: f32| {
        let (o, h) = (c.x + POD.x, c.y + POD.y);
        vec![
            v3(x, c.x + 2.0, h + 0.2),
            v3(x, o - 7.0, h + 0.2),
            v3(x, o + 0.2, h - 7.0),
            v3(x, o + 0.2, c.y + 1.0),
            v3(x, o + 2.0, c.y + 1.0),
            v3(x, o + 2.0, h - 6.2),
            v3(x, o - 6.2, h + 2.0),
            v3(x, c.x + 2.0, h + 2.0),
        ]
    };
    b.paint(PLATING).pattern(pattern::WARSHIP);
    b.loft(&[cowl(POD_AFT + 6.0), cowl(-204.0)], true, true);
    b.paint(PLATING_DARK).pattern(pattern::WARSHIP);
    b.block(
        v3(POD_AFT + 6.0, c.x - 12.0, c.y - POD.y - 1.4),
        v3(-206.0, c.x + 12.0, c.y - POD.y + 0.2),
    );
    // The armour collar round the bell's neck, octagonal, proud of the pod's aft face.
    let collar = |x: f32, r: f32| {
        octagon(r)
            .iter()
            .map(|p| v3(x, c.x + p[0], c.y + p[1]))
            .collect::<Vec<_>>()
    };
    b.paint(PLATING).pattern(pattern::WARSHIP);
    let r = 12.2 * MAIN_SIZE;
    b.loft(
        &[
            collar(POD_AFT + 1.0, r + 0.4),
            collar(POD_AFT + 1.0, r + 3.6),
            collar(POD_AFT - 2.6, r + 2.8),
            collar(POD_AFT - 2.6, r + 0.4),
            collar(POD_AFT + 1.0, r + 0.4),
        ],
        false,
        false,
    );
    // The exhaust shroud: an armoured hood raked back over the bell's top.
    b.paint(PLATING_DARK).pattern(pattern::WARSHIP);
    b.loft(
        &[
            vec![
                v3(POD_AFT + 2.0, c.x - 13.0, c.y + POD.y - 2.0),
                v3(POD_AFT + 2.0, c.x + 13.0, c.y + POD.y - 2.0),
                v3(POD_AFT + 2.0, c.x + 11.0, c.y + POD.y + 1.2),
                v3(POD_AFT + 2.0, c.x - 11.0, c.y + POD.y + 1.2),
            ],
            vec![
                v3(MAIN_X + 4.0, c.x - 10.0, c.y + r + 1.0),
                v3(MAIN_X + 4.0, c.x + 10.0, c.y + r + 1.0),
                v3(MAIN_X + 4.0, c.x + 8.5, c.y + r + 2.6),
                v3(MAIN_X + 4.0, c.x - 8.5, c.y + r + 2.6),
            ],
        ],
        true,
        true,
    );
    // Heat-sink fin bank down the outboard side, in a dark frame.
    let side = c.x + POD.x;
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.block(
        v3(-218.0, side - 0.5, c.y - 9.0),
        v3(-184.0, side + 0.8, c.y + 9.0),
    );
    if fine {
        b.paint(METAL).pattern(pattern::PLAIN);
        for k in 0..9 {
            let x = -216.0 + k as f32 * 3.8;
            b.block(
                v3(x - 0.4, side, c.y - 8.0),
                v3(x + 0.4, side + 3.6, c.y + 8.0),
            );
        }
    }
    // Coolant runs along the pod's outboard side above and below the fin bank, clamped.
    b.paint(METAL).pattern(pattern::PLAIN);
    for dz in [-12.0, 12.0] {
        b.cylinder_between(
            v3(POD_FORE - 4.0, side + 0.2, c.y + dz),
            v3(POD_AFT + 4.0, side + 0.2, c.y + dz),
            1.0,
            1.0,
            6,
        );
    }
    if fine {
        b.paint(ACCENT).pattern(pattern::PLAIN);
        for x in [-212.0, -200.0, -188.0] {
            for dz in [-12.0, 12.0] {
                b.cuboid(v3(x, side + 0.2, c.y + dz), v3(1.6, 2.6, 2.6));
            }
        }
    }
    // The pod's belly: a keel strake and a dark plate under the fin bank.
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.block(
        v3(-220.0, c.x - 3.0, c.y - POD.y - 0.8),
        v3(POD_FORE - 8.0, c.x + 3.0, c.y - POD.y + 0.4),
    );
}

/// An auxiliary drive (port; mirrored) with its mouth at `m`, `size` against a size-1
/// bell 12 m across: a hard octagonal bell, dark inside, a thin lit ring deep in its
/// throat; an armour collar at its neck; an armoured housing running forward into the hull.
fn aux_drive(b: &mut MeshBuilder, m: Vec3, size: f32) {
    let r = 12.2 * size;
    let ring = |x: f32, r: f32| {
        octagon(r)
            .iter()
            .map(|p| v3(x, m.y + p[0], m.z + p[1]))
            .collect::<Vec<_>>()
    };
    // The bell: a thick octagonal shell flaring from its throat to the mouth.
    b.paint(PLATING_DARK).pattern(pattern::PLAIN);
    b.loft(
        &[
            ring(m.x, r * 0.86),
            ring(m.x, r),
            ring(m.x + 14.0 * size, r * 0.78),
            ring(m.x + 14.0 * size, r * 0.62),
            ring(m.x, r * 0.86),
        ],
        false,
        false,
    );
    b.paint(TREAD).pattern(pattern::NONE);
    b.extrude_x(
        &octagon(r * 0.64)
            .iter()
            .map(|p| [m.y + p[0], m.z + p[1]])
            .collect::<Vec<_>>(),
        m.x + 13.0 * size,
        m.x + 15.0 * size,
    );
    // The thin lit ring deep in the throat.
    b.paint(GLOW).pattern(pattern::PLAIN);
    b.loft(
        &[
            ring(m.x + 11.6 * size, r * 0.66),
            ring(m.x + 11.6 * size, r * 0.7),
            ring(m.x + 12.4 * size, r * 0.7),
            ring(m.x + 12.4 * size, r * 0.66),
            ring(m.x + 11.6 * size, r * 0.66),
        ],
        false,
        false,
    );
    // The collar at the bell's neck and the housing forward of it.
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.loft(
        &[
            ring(m.x + 13.0 * size, r * 0.8),
            ring(m.x + 13.0 * size, r * 1.08),
            ring(m.x + 17.0 * size, r * 1.08),
            ring(m.x + 17.0 * size, r * 0.8),
            ring(m.x + 13.0 * size, r * 0.8),
        ],
        false,
        false,
    );
    let house = |x: f32, s: f32| {
        chamfered_rect(v2(r * 1.05, r * 1.0) * s, r * 0.35 * s)
            .iter()
            .map(|p| v3(x, m.y + p[0], m.z + p[1]))
            .collect::<Vec<_>>()
    };
    b.paint(PLATING).pattern(pattern::WARSHIP);
    b.loft(
        &[
            house(m.x + 16.0 * size, 0.92),
            house(m.x + 20.0 * size, 1.0),
            house(m.x + 52.0 * size, 1.0),
            house(m.x + 60.0 * size, 0.7),
        ],
        true,
        true,
    );
    if b.fine() {
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.block(
            v3(m.x + 26.0 * size, m.y - r * 0.5, m.z + r * 1.0 - 0.2),
            v3(m.x + 46.0 * size, m.y + r * 0.5, m.z + r * 1.0 + 0.6),
        );
    }
}

/// The gantry between the mains' pods: a structural frame of armoured beams across the gap
/// between them under the hull, and struts from the pods up to the hull's underside.
fn gantry(b: &mut MeshBuilder) {
    b.paint(PLATING_DARK).pattern(pattern::PLAIN);
    for x in [-220.0, -204.0, -188.0] {
        b.beam(
            v3(x, -MAIN_Y, MAIN_Z - 10.0),
            v3(x, MAIN_Y, MAIN_Z - 10.0),
            v2(2.4, 3.0),
            v2(2.4, 3.0),
        );
        b.beam(
            v3(x, -MAIN_Y + 4.0, MAIN_Z + 8.0),
            v3(x, MAIN_Y - 4.0, MAIN_Z + 8.0),
            v2(2.0, 2.6),
            v2(2.0, 2.6),
        );
    }
    if b.fine() {
        b.paint(METAL).pattern(pattern::PLAIN);
        for (a, c) in [(-220.0, -204.0), (-204.0, -188.0)] {
            b.beam(
                v3(a, 0.0, MAIN_Z - 10.0),
                v3(c, 0.0, MAIN_Z + 8.0),
                v2(1.2, 1.2),
                v2(1.2, 1.2),
            );
        }
        b.mirror_y(|b| {
            for x in [-216.0, -196.0] {
                b.beam(
                    v3(x, MAIN_Y + 10.0, MAIN_Z + POD.y - 1.0),
                    v3(x - 4.0, MAIN_Y + 16.0, HULL_BOTTOM + 3.0),
                    v2(1.6, 1.6),
                    v2(1.2, 1.2),
                );
            }
        });
    }
}
