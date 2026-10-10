//! The seed dispenser in the tail: a faceted housing run aft out of the hull to a broad
//! mouth, its throat recessed and dark behind guide vanes. The seed of cloud leaves from
//! the middle of the mouth (the Storm Seeder's muzzle).

use super::*;

pub(super) struct Dispenser {
    /// The middle of the mouth.
    pub(super) mouth: Vec3,
    /// Half width and half height of the mouth.
    pub(super) w: f32,
    pub(super) h: f32,
    /// Where the housing leaves the hull, and its size there as a share of the mouth's.
    pub(super) root_x: f32,
    pub(super) root: f32,
    /// How deep the throat goes in from the mouth.
    pub(super) depth: f32,
}

/// Emits `points` as one face, wound to face `toward`.
pub(super) fn face_toward(b: &mut MeshBuilder, points: &[Vec3], toward: Vec3) {
    let n = points
        .iter()
        .enumerate()
        .map(|(i, p)| p.cross(points[(i + 1) % points.len()]))
        .sum::<Vec3>();
    if n.dot(toward) >= 0.0 {
        b.face(points);
    } else {
        let back: Vec<Vec3> = points.iter().rev().copied().collect();
        b.face(&back);
    }
}

impl Dispenser {
    /// The mouth's outline at `x`, `scale` of its size: a flattened octagon.
    fn ring(&self, x: f32, scale: f32) -> Vec<Vec3> {
        let (w, h) = (self.w * scale, self.h * scale);
        [
            (w, -0.45),
            (w, 0.45),
            (0.7 * w, 1.0),
            (-0.7 * w, 1.0),
            (-w, 0.45),
            (-w, -0.45),
            (-0.7 * w, -1.0),
            (0.7 * w, -1.0),
        ]
        .iter()
        .map(|&(y, k)| v3(x, self.mouth.y + y, self.mouth.z + k * h))
        .collect()
    }

    pub(super) fn build(&self, b: &mut MeshBuilder) {
        let m = self.mouth;
        if b.coarse() {
            b.paint(PLATING_DARK);
            b.beam(
                v3(self.root_x, m.y, m.z),
                m,
                v2(self.w * 2.0 * self.root, self.h * 2.0 * self.root),
                v2(self.w * 2.0, self.h * 2.0),
            );
            return;
        }
        let mid = m.x + (self.root_x - m.x) * 0.35;
        b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
        b.with_facets(|b| {
            b.loft(
                &[
                    self.ring(self.root_x, self.root),
                    self.ring(mid, 1.0),
                    self.ring(m.x + 0.6, 1.0),
                ],
                true,
                false,
            );
            // The lip: a collar of plate round the mouth, standing proud.
            b.paint(PLATING).pattern(pattern::AIRFRAME);
            b.loft(
                &[
                    self.ring(m.x + 0.6, 1.0),
                    self.ring(m.x + 0.6, 1.06),
                    self.ring(m.x, 1.06),
                ],
                false,
                false,
            );
        });
        // The lip's face, and the throat recessed behind it.
        let (outer, inner) = (self.ring(m.x, 1.06), self.ring(m.x, 0.84));
        let back = m.x + self.depth;
        let deep = self.ring(back, 0.7);
        b.paint(PLATING);
        for i in 0..8 {
            let j = (i + 1) % 8;
            face_toward(b, &[outer[i], outer[j], inner[j], inner[i]], -Vec3::X);
        }
        b.paint(ACCENT);
        for i in 0..8 {
            let j = (i + 1) % 8;
            let middle = (inner[i] + inner[j]) * 0.5;
            let inward = v3(0.0, m.y - middle.y, m.z - middle.z);
            face_toward(b, &[inner[i], inner[j], deep[j], deep[i]], inward);
        }
        face_toward(b, &deep, -Vec3::X);
        if !b.fine() {
            return;
        }
        // Guide vanes across the throat and a splitter down its middle.
        b.paint(METAL);
        let x = m.x + 0.5;
        for k in [-0.36, 0.36] {
            let z = m.z + k * self.h;
            b.beam(
                v3(x, m.y - self.w * 0.82, z),
                v3(x, m.y + self.w * 0.82, z),
                v2(0.6, 0.16),
                v2(0.6, 0.16),
            );
        }
        b.paint(PLATING_DARK);
        b.beam(
            v3(x - 0.2, m.y, m.z - self.h * 0.82),
            v3(x - 0.2, m.y, m.z + self.h * 0.82),
            v2(0.2, 0.5),
            v2(0.2, 0.5),
        );
    }
}
