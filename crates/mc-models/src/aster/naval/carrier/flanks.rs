//! What breaks up the flanks, different along each side: a stepped armour belt on the
//! lower face with breaks between its lengths, strakes along the upper wall that change
//! height, sponson housings of different lengths standing out over the knuckle, framed
//! boat-bay and hangar doors, a gallery or two, and intakes let into the faces.
//!
//! The flank's faces amidships (`body`): the upper wall leans in from the deck edge
//! (17 m, `PLAN`) to 6.2 m up, the knuckle steps out to `outer` + 0.5 between 5 and 3.4 m
//! up, and the bottom falls in to the waterline.

use super::*;

/// The deck edge's half beam where the flank runs straight (`PLAN`), the upper wall's half
/// beam (it leans in a little from the deck edge) and its foot and
/// head; the lower face's half beam.
const FLANK: f32 = 17.0;
const WALL: f32 = FLANK - 0.15;
const WALL_Z: (f32, f32) = (6.3, 8.0);
const LOWER: f32 = FLANK + 0.5;

/// One piece on a flank, x along it.
#[derive(Clone, Copy)]
enum Piece {
    /// A length of armour belt on the lower face: its x span, its foot and head.
    Belt(f32, f32, f32, f32),
    /// A strake along the upper wall: its x span and height.
    Strake(f32, f32, f32),
    /// A sponson housing standing out from the upper wall over the knuckle: x span, how
    /// far it stands out, its head.
    Sponson(f32, f32, f32, f32),
    /// A framed door or bay in the upper wall: x span, foot and head.
    Door(f32, f32, f32, f32),
    /// A gallery hung from the upper wall: x span, its deck's height.
    Gallery(f32, f32, f32),
    /// Intakes let into the lower face: x span, height, how many.
    Intakes(f32, f32, f32, usize),
}

/// The port side, stern first. Everything stands where the flank runs straight, from
/// x -42 to 30.
const PORT: [Piece; 15] = [
    Piece::Belt(-42.0, -38.0, 3.7, 4.8),
    Piece::Belt(-30.0, -12.0, 3.6, 4.5),
    Piece::Belt(-2.0, 16.0, 3.7, 4.9),
    Piece::Belt(20.0, 30.0, 3.8, 4.6),
    Piece::Door(-37.0, -31.0, 3.6, 4.8),
    Piece::Intakes(-11.0, -4.0, 4.2, 3),
    Piece::Intakes(16.5, 19.5, 4.4, 2),
    Piece::Strake(-41.0, -32.0, 7.6),
    Piece::Strake(-20.0, 4.0, 7.1),
    Piece::Strake(18.0, 30.0, 7.5),
    Piece::Door(-31.0, -22.0, 6.4, 7.9),
    Piece::Door(5.0, 9.5, 6.6, 7.8),
    Piece::Sponson(-11.0, -2.0, 1.8, 7.6),
    Piece::Sponson(24.0, 27.5, 1.2, 7.2),
    Piece::Gallery(11.0, 16.5, 6.9),
];

/// The starboard side, stern first: the same kinds of piece, laid out differently.
const STARBOARD: [Piece; 14] = [
    Piece::Belt(-42.0, -38.0, 3.6, 4.6),
    Piece::Belt(-34.0, -6.0, 3.7, 4.9),
    Piece::Belt(0.0, 14.0, 3.6, 4.4),
    Piece::Belt(20.0, 30.0, 3.7, 4.8),
    Piece::Intakes(-37.5, -34.5, 4.4, 2),
    Piece::Door(-4.0, -1.0, 3.7, 4.7),
    Piece::Intakes(15.0, 19.0, 4.1, 4),
    Piece::Strake(-26.0, -21.0, 7.3),
    Piece::Strake(7.0, 12.0, 7.7),
    Piece::Door(-40.0, -27.0, 6.4, 8.0),
    Piece::Door(14.0, 18.0, 6.7, 7.8),
    Piece::Sponson(-20.0, -6.0, 1.6, 7.4),
    Piece::Sponson(20.0, 29.0, 2.0, 7.8),
    Piece::Gallery(2.0, 6.5, 7.0),
];

pub(super) fn build(b: &mut MeshBuilder) {
    for (pieces, side) in [(&PORT[..], 1.0), (&STARBOARD[..], -1.0)] {
        for piece in pieces {
            draw(b, *piece, side);
        }
    }
}

/// An axis-aligned box from `y0` to `y1` out from the centreline on the `side`.
fn side_block(b: &mut MeshBuilder, side: f32, x: (f32, f32), y: (f32, f32), z: (f32, f32)) {
    let (a, c) = (side * y.0, side * y.1);
    b.block(v3(x.0, a.min(c), z.0), v3(x.1, a.max(c), z.1));
}

fn draw(b: &mut MeshBuilder, piece: Piece, side: f32) {
    match piece {
        Piece::Belt(x0, x1, z0, z1) => {
            // A thick plate with its top edge chamfered.
            let profile: Vec<[f32; 2]> = [
                [LOWER - 0.05, z0],
                [LOWER + 0.45, z0 + 0.15],
                [LOWER + 0.45, z1 - 0.25],
                [LOWER + 0.15, z1],
                [LOWER - 0.05, z1],
            ]
            .iter()
            .map(|&[y, z]| [side * y, z])
            .collect();
            b.paint(PLATING).pattern(pattern::HULL_PLATE);
            b.extrude_x(&profile, x0, x1);
        }
        Piece::Strake(x0, x1, z) => {
            if !b.fine() {
                return;
            }
            b.paint(PLATING).pattern(pattern::PLAIN);
            side_block(
                b,
                side,
                (x0, x1),
                (WALL - 0.05, WALL + 0.25),
                (z - 0.14, z + 0.14),
            );
        }
        Piece::Sponson(x0, x1, out, head) => {
            // Its underside raked up from the knuckle, its top drawn in toward the wall, its
            // ends cut back.
            let ring = |x: f32, k: f32| -> Vec<Vec3> {
                [
                    (WALL - 0.1, WALL_Z.0 - 0.2),
                    (WALL + out * k, WALL_Z.0 + 0.4),
                    (WALL + out * k, head - 0.4),
                    (WALL + out * k - 0.4, head),
                    (WALL - 0.1, head),
                ]
                .iter()
                .map(|&(y, z)| v3(x, side * y, z))
                .collect()
            };
            b.paint(PLATING).pattern(pattern::HULL_PLATE);
            b.loft(
                &[
                    ring(x0, 0.4),
                    ring(x0 + 0.8, 1.0),
                    ring(x1 - 0.8, 1.0),
                    ring(x1, 0.4),
                ],
                true,
                true,
            );
            if b.fine() {
                // A dark port in its face.
                b.paint(ACCENT).pattern(pattern::PLAIN);
                let len = ((x1 - x0) * 0.35).min(3.0);
                side_block(
                    b,
                    side,
                    ((x0 + x1) * 0.5 - len * 0.5, (x0 + x1) * 0.5 + len * 0.5),
                    (WALL + out - 0.02, WALL + out + 0.06),
                    (head - 1.2, head - 0.8),
                );
            }
        }
        Piece::Door(x0, x1, z0, z1) => {
            // On the lower face if it sits low, else on the upper wall.
            let face = if z1 < WALL_Z.0 { LOWER } else { WALL };
            b.paint(ACCENT).pattern(pattern::PLAIN);
            side_block(
                b,
                side,
                (x0 + 0.3, x1 - 0.3),
                (face - 0.05, face + 0.04),
                (z0 + 0.25, z1 - 0.25),
            );
            if !b.fine() {
                return;
            }
            // A frame standing proud round it, heavier over the head.
            b.paint(PLATING).pattern(pattern::PLAIN);
            side_block(
                b,
                side,
                (x0, x1),
                (face - 0.05, face + 0.22),
                (z1 - 0.25, z1 + 0.1),
            );
            side_block(
                b,
                side,
                (x0, x1),
                (face - 0.05, face + 0.14),
                (z0 - 0.05, z0 + 0.25),
            );
            for x in [x0, x1 - 0.3] {
                side_block(
                    b,
                    side,
                    (x, x + 0.3),
                    (face - 0.05, face + 0.16),
                    (z0 + 0.25, z1 - 0.25),
                );
            }
            if x1 - x0 > 5.0 {
                // A big door's leaves meet in the middle.
                b.paint(PLATING_DARK).pattern(pattern::PLAIN);
                let xm = (x0 + x1) * 0.5;
                side_block(
                    b,
                    side,
                    (xm - 0.06, xm + 0.06),
                    (face, face + 0.08),
                    (z0 + 0.25, z1 - 0.25),
                );
            }
        }
        Piece::Gallery(x0, x1, z) => {
            // A dark doorway in the wall, the gallery's deck under it on two brackets, a
            // solid parapet along its edge.
            b.paint(ACCENT).pattern(pattern::PLAIN);
            let xm = (x0 + x1) * 0.5;
            side_block(
                b,
                side,
                (xm - 0.8, xm + 0.8),
                (WALL - 0.05, WALL + 0.04),
                (z, z + 1.0),
            );
            b.paint(PLATING).pattern(pattern::PLAIN);
            side_block(b, side, (x0, x1), (WALL - 0.05, WALL + 1.4), (z - 0.25, z));
            side_block(b, side, (x0, x1), (WALL + 1.25, WALL + 1.4), (z, z + 0.55));
            if b.fine() {
                for x in [x0 + 0.6, x1 - 0.6] {
                    b.beam(
                        v3(x, side * (WALL - 0.05), z - 1.0),
                        v3(x, side * (WALL + 1.1), z - 0.25),
                        v2(0.2, 0.2),
                        v2(0.2, 0.2),
                    );
                }
            }
        }
        Piece::Intakes(x0, x1, z, n) => {
            if !b.fine() {
                return;
            }
            b.paint(ACCENT).pattern(pattern::PLAIN);
            let step = (x1 - x0) / n as f32;
            for k in 0..n {
                let x = x0 + step * (k as f32 + 0.5);
                side_block(
                    b,
                    side,
                    (x - step * 0.32, x + step * 0.32),
                    (LOWER - 0.05, LOWER + 0.05),
                    (z - 0.18, z + 0.18),
                );
            }
        }
    }
}
