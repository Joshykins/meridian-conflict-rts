//! Fitted service plates and cooling hardware. Detail stays on the owning rigid
//! part so it follows the head and planted-leg animation without sliding.
use crate::builder::MeshBuilder;
use crate::{material::*, pattern};
use glam::{Vec2, Vec3};

pub(super) fn armor(b: &mut MeshBuilder) {
    b.paint(PLATING_DARK).pattern(pattern::EMBER_MACHINED);
}

/// A fitted inner course follows the actual eight-sided armor face, preserving
/// its bevel and silhouette. A recessed service hatch divides the broad face.
pub(super) fn panel_inlay(b: &mut MeshBuilder, upper: &[Vec3]) {
    let center = upper.iter().copied().sum::<Vec3>() / upper.len() as f32;
    let n = (upper[1] - upper[0]).cross(upper[2] - upper[0]).normalize();
    let inset: Vec<Vec3> = upper
        .iter()
        .map(|p| center + (*p - center) * 0.78 + n * 0.045)
        .collect();
    b.paint(METAL).pattern(pattern::EMBER);
    for i in 0..upper.len() {
        let next = (i + 1) % upper.len();
        let a = inset[i];
        let c = inset[next];
        // Thin exposed steel gasket seated inside the broad plate, not glowing.
        b.face(&[
            a,
            c,
            c.lerp(center + n * 0.045, 0.025),
            a.lerp(center + n * 0.045, 0.025),
        ]);
    }
    let forward = (upper[3] - upper[0]).normalize();
    let side = n.cross(forward).normalize();
    let length = (upper[3] - upper[0]).length() * 0.22;
    let width = (upper[7] - upper[0]).length() * 0.22;
    let at = center + n * 0.09;
    b.paint(PLATING).pattern(pattern::EMBER_MACHINED);
    b.face(&[
        at - forward * length - side * width,
        at + forward * length - side * width,
        at + forward * length + side * width,
        at - forward * length + side * width,
    ]);
    // An offset cooling gallery leaves the plate center calm and readable.
    let vent = at - forward * (length * 1.55);
    b.paint(METAL).pattern(pattern::EMBER);
    for i in 0..3 {
        let p = vent + forward * (i as f32 * 0.3);
        b.beam(
            p - side * (width * 0.85),
            p + side * (width * 0.85),
            Vec2::new(0.1, 0.12),
            Vec2::new(0.1, 0.12),
        );
    }
}

pub(super) fn shin(b: &mut MeshBuilder, a: Vec3, c: Vec3, face: Vec3, across: Vec3) {
    if !b.fine() {
        return;
    }
    // Paired tie rods, clamps and layered cheek scales frame the central optic.
    for sign in [-1.0, 1.0] {
        let side = across * sign;
        let start = a + side * 3.7 + face * 2.5;
        let end = c + side * 2.7 + face * 2.35;
        b.paint(METAL).pattern(pattern::EMBER);
        b.cylinder_between(start, end, 0.22, 0.22, 4);
        for t in [0.12, 0.88] {
            let p = start.lerp(end, t);
            armor(b);
            b.cylinder_between(
                p - (end - start).normalize() * 0.22,
                p + (end - start).normalize() * 0.22,
                0.36,
                0.36,
                4,
            );
        }
        for t in [0.24, 0.70] {
            let p = a.lerp(c, t) + side * 2.9 + face * 3.0;
            b.paint(PLATING).pattern(pattern::EMBER_MACHINED);
            b.beam(
                p - side * 0.65,
                p + side * 0.65,
                Vec2::new(0.3, 0.45),
                Vec2::new(0.24, 0.32),
            );
        }
    }
}
