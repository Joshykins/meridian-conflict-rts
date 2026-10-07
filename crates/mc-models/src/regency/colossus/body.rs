//! Broad interlocking armor over a low mechanical keel. Each panel has a calm
//! face, a chamfered perimeter and a visible gap to the neighboring course.
use super::super::commander::form::{ring, sleeve, OCT};
use super::super::kit::{metal, seam, v3};
use super::detail::armor as dark_plate;
use super::{DECK, LANCE_PIVOT, RACE};
use crate::builder::{MeshBuilder, Section};
use crate::{material::*, part, pattern, rig};
use glam::{Vec2, Vec3};
pub(super) const EMITTER: Vec3 = Vec3::new(-14.0, 0.0, 35.2);

pub(super) fn carapace(b: &mut MeshBuilder) {
    seam(b);
    sleeve(
        b,
        &[
            ring(v3(-25.0, 0.0, 23.0), Vec3::Z, 9.0, 2.4),
            ring(v3(-17.0, 0.0, 24.2), Vec3::Z, 17.0, 5.0),
            ring(v3(1.0, 0.0, 24.0), Vec3::Z, 18.0, 5.0),
            ring(v3(16.0, 0.0, 23.0), Vec3::Z, 10.0, 3.8),
            ring(v3(21.0, 0.0, 23.0), Vec3::Z, 5.0, 2.5),
        ],
        &OCT,
    );
    b.mirror_y(shoulder);
    // Central sloping glacis: a clear unbroken wedge leading into the siege head.
    panel(b, -9.0, 17.0, 0.0, 7.5, 29.2, 3.0, false);
    // The rear reactor casing is recessed between the dorsal launcher and radiator banks.
    dark_plate(b);
    sleeve(
        b,
        &[
            ring(v3(-23.0, 0.0, 27.0), Vec3::Z, 4.2, 1.5),
            ring(v3(-17.0, 0.0, 31.0), Vec3::Z, 4.8, 3.0),
            ring(v3(-12.0, 0.0, 31.5), Vec3::Z, 4.3, 3.7),
        ],
        &OCT,
    );
    if b.fine() {
        b.mirror_y(|b| {
            seam(b);
            b.chamfered_box(v3(-15.0, 7.4, 30.0), v3(13.0, 3.0, 1.0), 0.35);
            for i in 0..7 {
                metal(b);
                b.chamfered_box(
                    v3(-20.5 + i as f32 * 1.6, 7.4, 30.65),
                    v3(0.45, 2.7, 1.0),
                    0.12,
                );
            }
        });
    }
}

fn shoulder(b: &mut MeshBuilder) {
    // Thick courses wrap the side mechanics instead of thin aircraft-like wings.
    panel(b, -25.0, -10.0, 12.0, 6.0, 26.8, 3.0, true);
    panel(b, -9.3, 4.5, 12.0, 7.0, 27.3, 3.4, false);
    panel(b, 5.2, 16.5, 10.0, 5.4, 26.0, 2.8, true);
    // The cheek cradles physically meet the independent chin-gun housings.
    dark_plate(b);
    sleeve(
        b,
        &[
            ring(v3(7.0, 8.6, 24.0), Vec3::Z, 3.5, 3.5),
            ring(v3(15.0, 8.6, 24.0), Vec3::Z, 3.0, 3.0),
            ring(v3(18.0, 8.6, 23.0), Vec3::Z, 2.2, 2.0),
        ],
        &OCT,
    );
    if b.fine() {
        seam(b);
        b.chamfered_box(v3(-4.0, 17.6, 25.0), v3(19.0, 1.3, 3.2), 0.35);
        for i in 0..9 {
            metal(b);
            b.chamfered_box(
                v3(-12.0 + i as f32 * 2.0, 18.0, 25.0),
                v3(0.35, 0.9, 2.2),
                0.1,
            );
        }
        b.paint(TEAM).pattern(pattern::PLAIN);
        b.face(&[
            v3(7.0, 9.0, 28.94),
            v3(10.0, 8.2, 28.26),
            v3(10.0, 9.0, 28.26),
            v3(7.0, 9.8, 28.94),
        ]);
    }
}

/// A raised slab with broad beveled edges and a forward sloping face.
fn panel(b: &mut MeshBuilder, tail: f32, nose: f32, y: f32, w: f32, z: f32, rise: f32, pale: bool) {
    let len = nose - tail;
    let outline = [
        v3(tail, y - w * 0.72, z),
        v3(tail + len * 0.18, y - w, z),
        v3(nose - len * 0.18, y - w * 0.83, z - 0.8),
        v3(nose, y - w * 0.43, z - 1.5),
        v3(nose, y + w * 0.43, z - 1.5),
        v3(nose - len * 0.18, y + w * 0.83, z - 0.8),
        v3(tail + len * 0.18, y + w, z),
        v3(tail, y + w * 0.72, z),
    ];
    let mid = (tail + nose) * 0.5;
    let upper: Vec<Vec3> = outline
        .iter()
        .map(|p| v3(mid + (p.x - mid) * 0.86, y + (p.y - y) * 0.85, p.z + rise))
        .collect();
    dark_plate(b);
    let mut bottom = outline.to_vec();
    bottom.reverse();
    b.face(&bottom);
    for i in 0..outline.len() {
        dark_plate(b);
        if b.fine() && i == 3 {
            b.pattern(pattern::EMBER_SLOT);
        }
        let next = (i + 1) % outline.len();
        b.face(&[outline[i], outline[next], upper[next], upper[i]]);
    }
    b.paint(if pale { PLATING } else { PLATING_DARK })
        .pattern(pattern::EMBER_MACHINED);
    b.face(&upper);
    if b.fine() {
        super::detail::panel_inlay(b, &upper);
    }
}
pub(super) fn coarse(b: &mut MeshBuilder) {
    b.with_part(part::TURRET, |b| {
        dark_plate(b);
        b.loft_z(
            &[
                [20.0, 0.0],
                [2.0, 19.0],
                [-25.0, 12.0],
                [-25.0, -12.0],
                [2.0, -19.0],
            ],
            &[Section::new(RACE, 0.75), Section::new(DECK + 6.0, 1.0)],
        );
        b.paint(TEAM);
        b.face(&[
            v3(0.0, -4.0, DECK + 6.02),
            v3(0.0, 4.0, DECK + 6.02),
            v3(-8.0, 0.0, DECK + 6.02),
        ]);
        dark_plate(b);
        b.with_limb(rig::ARM_GUN, |b| {
            b.beam(
                LANCE_PIVOT,
                LANCE_PIVOT + v3(21.0, 0.0, 0.0),
                Vec2::splat(8.0),
                Vec2::splat(3.0),
            )
        });
    });
}
