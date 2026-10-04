//! Common construction vocabulary, without making every hull the same silhouette.
use super::super::kit::{dark_plate, metal, seam, segment, shell, v3};
use super::super::lift::bell;
use crate::builder::MeshBuilder;
use crate::material::{GLOW_LASER, GLOW_VIOLET, TEAM};
use glam::{Vec2, Vec3};

/// A swept hull lobe; its armour follows the curve as individual overlapping plates.
pub(super) fn lobe(b: &mut MeshBuilder, points: &[(Vec3, f32, f32)]) {
    seam(b);
    let reduced: Vec<_> = points
        .iter()
        .enumerate()
        .filter(|(i, _)| i % 2 == 0 || *i + 1 == points.len())
        .map(|(_, p)| *p)
        .collect();
    let body = if b.mid() { reduced.as_slice() } else { points };
    b.with_facets(|b| segment(b, body, Vec3::Z));
    dark_plate(b);
    let stride = if b.fine() { 1 } else { 2 };
    for i in (0..points.len() - 1).step_by(stride) {
        let a = points[i];
        let c = points[(i + stride).min(points.len() - 1)];
        let lift = Vec3::Z * 0.55;
        dark_plate(b);
        b.with_facets(|b| {
            shell(
                b,
                &[(a.0 + lift, a.1 * 1.03, a.2), (c.0 + lift, c.1 * 1.06, c.2)],
                Vec3::Z,
            )
        });
        b.paint(GLOW_LASER);
        let side = v3(0.0, a.1 * 0.92, a.2 * 0.15);
        b.cylinder_between(a.0 + side, c.0 + side * 0.8, 0.16, 0.16, 4);
    }
}

/// Far silhouette: deliberately preserves the open fork at fewer than sixty triangles.
pub(super) fn coarse_lobes(b: &mut MeshBuilder, outline: &[[f32; 2]], low: f32, high: f32) {
    dark_plate(b);
    b.mirror_y(|b| b.extrude_z(outline, low, high));
}

pub(super) fn mark(b: &mut MeshBuilder, at: Vec3, size: f32) {
    b.paint(TEAM);
    b.mirror_y(|b| {
        b.face(&[
            at,
            at + v3(-size, size * 0.4, 0.0),
            at + v3(-size * 1.25, size * 0.18, 0.0),
        ])
    });
}

/// Short swept fins rooted in the spine, rather than long exposed barrels or antennas.
pub(super) fn crest(b: &mut MeshBuilder, x: f32, y: f32, base: f32, top: f32, len: f32) {
    dark_plate(b);
    b.with_facets(|b| {
        b.extrude_y(
            &[
                [x, base],
                [x - len * 0.3, top],
                [x - len, top - 1.0],
                [x - len * 0.85, base],
            ],
            y - 0.6,
            y + 0.6,
        )
    });
}

/// Gravity drives remain the Regency's lift system; the stern's radiant lens is violet.
pub(super) fn drive(b: &mut MeshBuilder, at: Vec3, radius: f32) {
    bell(b, at, radius, radius * 0.8);
    metal(b);
    let end = at + v3(-radius * 0.8, 0.0, radius * 1.4);
    b.cylinder_between(end + Vec3::X * radius, end, radius, radius, b.sides(8));
    b.paint(GLOW_VIOLET);
    b.cylinder_between(
        end,
        end - Vec3::X * 0.15,
        radius * 0.7,
        radius * 0.7,
        b.sides(8),
    );
}

/// An independently aiming compact plasma casemate, attached to its actual weapon slot.
pub(super) fn gun(b: &mut MeshBuilder, weapon: usize, pivot: Vec3, muzzle: Vec3, width: f32) {
    b.with_house(weapon, pivot, 0.25, |b| {
        if b.coarse() {
            b.paint(GLOW_VIOLET);
            b.face(&[
                muzzle,
                muzzle + v3(-1.0, -width, 0.0),
                muzzle + v3(-1.0, width, 0.0),
            ]);
            return;
        }
        dark_plate(b);
        b.cuboid(pivot, v3(width * 2.2, width * 2.2, width));
        b.with_recoil(|b| {
            dark_plate(b);
            b.beam(
                pivot,
                muzzle - Vec3::X * 0.5,
                Vec2::new(width, width * 0.7),
                Vec2::new(width * 0.75, width * 0.6),
            );
            if b.fine() {
                seam(b);
                b.cylinder_between(
                    muzzle - Vec3::X * 1.8,
                    muzzle,
                    width * 0.65,
                    width * 0.65,
                    b.sides(8),
                );
            }
            b.paint(GLOW_VIOLET);
            b.cylinder_between(
                muzzle - Vec3::X * 0.1,
                muzzle,
                width * 0.43,
                width * 0.43,
                b.sides(8),
            );
        });
    });
}
