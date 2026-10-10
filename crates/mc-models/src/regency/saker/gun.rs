//! The Saker's gun: a pinch bore split down its length into two plated halves, one
//! above the other (the Basilisk's upright split, drawn long), that hold the charge
//! between their tips. Red is let into the halves' flanks along the gap, slots in the
//! plate; swept plates lie in a course down each half's back and graphite bands clamp
//! both.
//!
//! Drawn in the gun's frame: the origin at the trunnion, +x down the bore, +z up off it.

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::{pattern, rig};

use super::super::kit::{dark_plate, metal, slab, v3};
use super::super::machine::{armour, Course, Frame};
use super::parts::{drum, inlay, ridged};
use super::turret::CHEEK;

/// The gun's line: its trunnion, how far its bore is raised at rest, and the bore's
/// length out to the middle of its charge.
#[derive(Clone, Copy, Debug)]
pub(super) struct Line {
    pub(super) pivot: Vec3,
    pitch_deg: f32,
    len: f32,
}

impl Line {
    fn pitch(&self) -> f32 {
        self.pitch_deg.to_radians()
    }

    /// The middle of the charge: the unit file's `muzzle`.
    pub(super) fn muzzle(&self) -> Vec3 {
        let a = self.pitch();
        self.pivot + Vec3::new(a.cos(), 0.0, a.sin()) * self.len
    }

    /// Records the turret's yaw axis (over the unit's middle at `deck`), the gun's
    /// trunnion and its recoil.
    pub(super) fn rig(&self, b: &mut MeshBuilder, deck: f32) {
        b.set_turret_pivot(v3(0.0, 0.0, deck));
        b.set_arm_pivot(self.pivot);
        b.set_recoil(self.pivot, self.muzzle(), 1.6);
    }
}

/// The bore's shape.
pub(super) struct Gun {
    pub(super) line: Line,
    /// Each half at four stations, breech to tip: x, half its width, its height off the
    /// gap.
    stations: [[f32; 3]; 4],
    /// Half the gap between the halves.
    gap: f32,
    /// Graphite bands clamped across both halves.
    bands: &'static [f32],
}

/// How far round the charge the halves' tips stand: none closer than 0.4 of it.
#[cfg(test)]
pub(super) const HOLD: f32 = 2.6;

/// The bore: 30 m from trunnion to charge.
pub(super) const BORE: Gun = Gun {
    line: Line {
        pivot: Vec3::new(0.5, 0.0, 9.0),
        pitch_deg: 4.0,
        len: 30.0,
    },
    stations: [
        [1.0, 1.4, 1.3],
        [5.0, 1.4, 1.3],
        [24.0, 1.05, 0.95],
        [28.8, 0.85, 0.75],
    ],
    gap: 0.35,
    bands: &[6.0, 13.0, 20.0],
};

/// A half's section, out from its flat inner face: across (-1 to 1), and out (0 to 1).
const HALF: [[f32; 2]; 6] = [
    [1.0, 0.0],
    [0.92, 0.6],
    [0.45, 1.0],
    [-0.45, 1.0],
    [-0.92, 0.6],
    [-1.0, 0.0],
];

impl Gun {
    fn len(&self) -> f32 {
        self.line.len
    }

    /// A half's half width and height at `x`, between its stations.
    fn at(&self, x: f32) -> Vec2 {
        let s = &self.stations;
        let k = s.windows(2).position(|w| x <= w[1][0]).unwrap_or(2);
        let (a, c) = (s[k], s[k + 1]);
        let t = ((x - a[0]) / (c[0] - a[0])).clamp(0.0, 1.0);
        Vec2::new(a[1], a[2]).lerp(Vec2::new(c[1], c[2]), t)
    }

    /// One half's ring at station `i`, `up` +1 above the gap or -1 below.
    fn ring(&self, i: usize, up: f32) -> Vec<Vec3> {
        let [x, w, h] = self.stations[i];
        HALF.iter()
            .map(|&[a, o]| v3(x, a * w, up * (self.gap + o * h)))
            .collect()
    }
}

/// The gun on its pitching arm: the trunnion pin, the breech, and the bore, which
/// recoils.
pub(super) fn gun(b: &mut MeshBuilder, g: &Gun) {
    let line = g.line;
    b.with_limb(rig::ARM_GUN, |b| {
        b.pitched(line.pivot, line.pitch(), |b| {
            if b.coarse() {
                coarse(b, g);
                return;
            }
            breech(b, g);
            b.with_recoil(|b| bore(b, g));
        });
    });
}

/// The trunnion drum through the cradle's cheeks, and the breech behind the bore: a
/// plated block, a plate lapped down each side swept back past it into a spike, a ridged
/// plate over its top and a lit gasket in its back.
fn breech(b: &mut MeshBuilder, g: &Gun) {
    drum(b, Vec3::ZERO, Vec3::Y, 1.0, (CHEEK + 0.7) * 2.0 + 0.3);
    let size = g.at(1.0);
    let (w, tall) = (size.x * 1.05, (g.gap + size.y) * 2.3);
    dark_plate(b);
    b.with_facets(|b| {
        b.beam(
            v3(-2.8, 0.0, 0.0),
            v3(1.6, 0.0, 0.0),
            Vec2::new(w * 2.0, tall),
            Vec2::new(w * 1.9, tall * 0.94),
        )
    });
    if !b.mid() {
        return;
    }
    let h = tall * 0.5;
    b.mirror_y(|b| {
        dark_plate(b);
        armour(
            b,
            &Frame::new(v3(1.4, w + 0.02, 0.0), -Vec3::X, Vec3::Y),
            &[
                [0.0, -h * 0.8],
                [0.0, h * 0.8],
                [3.2, h * 0.7],
                [5.0, h * 0.1],
                [3.2, -h * 0.7],
            ],
            0.3,
        );
    });
    if b.fine() {
        dark_plate(b);
        ridged(
            b,
            &Frame::new(v3(1.2, 0.0, h + 0.02), -Vec3::X, Vec3::Z),
            &[
                [0.0, -w * 0.8],
                [0.0, w * 0.8],
                [3.0, w * 0.7],
                [4.2, 0.0],
                [3.0, -w * 0.7],
            ],
            0.26,
            [1.6, 0.0],
            0.3,
        );
        inlay(
            b,
            &[
                v3(-2.8, -w * 0.6, -h * 0.5),
                v3(-2.8, w * 0.6, -h * 0.5),
                v3(-2.8, w * 0.6, h * 0.5),
                v3(-2.8, -w * 0.6, h * 0.5),
            ],
            -Vec3::X,
            pattern::EMBER_RIM,
        );
    }
}

/// Far off: the halves as two open spikes either side of the charge.
fn coarse(b: &mut MeshBuilder, g: &Gun) {
    let len = g.len();
    let size = g.at(len * 0.5);
    let off = g.gap + size.y * 0.5;
    dark_plate(b);
    for z in [-off, off] {
        let ring = vec![
            v3(-2.0, size.x, z - size.y * 0.5),
            v3(-2.0, -size.x, z - size.y * 0.5),
            v3(-2.0, 0.0, z + size.y * 0.6),
        ];
        b.loft(&[ring, vec![v3(len - 1.3, 0.0, z); 3]], false, false);
    }
}

/// The bore: the two halves either side of the gap, a graphite rod down the gap that
/// stops short of the charge, the bands, red slots in the flanks and a
/// course of plates down each half's back.
fn bore(b: &mut MeshBuilder, g: &Gun) {
    let len = g.len();
    let fine = b.fine();
    metal(b);
    b.cylinder_between(
        v3(1.6, 0.0, 0.0),
        v3(len - 3.6, 0.0, 0.0),
        g.gap * 0.7,
        g.gap * 0.6,
        b.sides(8),
    );
    for &x in g.bands {
        let s = g.at(x) * 1.07;
        metal(b);
        b.block(v3(x, -s.x, -(g.gap + s.y)), v3(x + 0.7, s.x, g.gap + s.y));
        if fine {
            // A plated cap clamped over the band, top and bottom.
            for up in [1.0f32, -1.0] {
                dark_plate(b);
                ridged(
                    b,
                    &Frame::new(v3(x - 0.15, 0.0, up * (g.gap + s.y)), Vec3::X, Vec3::Z * up),
                    &[[0.0, -s.x], [1.0, -s.x], [1.0, s.x], [0.0, s.x]],
                    0.16,
                    [0.5, 0.0],
                    0.12,
                );
            }
        }
    }
    for up in [1.0f32, -1.0] {
        let rings: Vec<Vec<Vec3>> = (0..4).map(|i| g.ring(i, up)).collect();
        dark_plate(b);
        b.with_facets(|b| b.loft(&rings, true, true));
        // Red let into each flank by the gap, a slot in a plate laid on it, a pace at a
        // time from the breech out toward the tip.
        let from = 2.0 + 0.8;
        let count = if fine { 3 } else { 2 };
        let pace = (len - 3.0 - from) / count as f32;
        dark_plate(b);
        b.pattern(pattern::EMBER_SLOT);
        for i in 0..count {
            let x0 = from + pace * i as f32;
            let x1 = x0 + pace * 0.6;
            let (s0, s1) = (g.at(x0), g.at(x1));
            for side in [1.0f32, -1.0] {
                let at = |x: f32, s: Vec2, o: f32| v3(x, side * s.x, up * (g.gap + o * s.y));
                slab(
                    b,
                    [
                        at(x0, s0, 0.1),
                        at(x1, s1, 0.1),
                        at(x1, s1, 0.3),
                        at(x0, s0, 0.3),
                    ],
                    Vec3::Y * (side * 0.05),
                );
            }
        }
        if b.mid() {
            claw(b, g, up);
            back_course(b, g, up);
        }
        if fine {
            flank_course(b, g, up);
        }
    }
}

/// The lip at a half's tip: a ridged plate over its back, swept forward to frame the
/// charge between the tips, its end short of the charge.
fn claw(b: &mut MeshBuilder, g: &Gun, up: f32) {
    let len = g.len();
    let x0 = len - 3.6;
    let s = g.at(x0);
    let tip = g.at(len - 1.2);
    dark_plate(b);
    ridged(
        b,
        &Frame::new(
            v3(x0, 0.0, up * (g.gap + s.y - 0.05)),
            Vec3::X,
            Vec3::Z * up,
        ),
        &[
            [0.0, -s.x * 0.9],
            [0.0, s.x * 0.9],
            [1.6, tip.x * 1.05],
            [2.4, tip.x * 0.4],
            [2.4, -tip.x * 0.4],
            [1.6, -tip.x * 1.05],
        ],
        0.3,
        [1.0, 0.0],
        0.3,
    );
}

/// Swept plates laid back down a half's back from near its tip, overlapping like
/// feathers, the last running past into a spike.
fn back_course(b: &mut MeshBuilder, g: &Gun, up: f32) {
    let len = g.len();
    let tip = len - 3.8;
    let front = g.at(tip);
    let back = g.at(2.0 + 1.0);
    let span = tip - 2.0 - 2.0;
    let rise = (back.y - front.y) / span;
    let count = if b.fine() { 4 } else { 2 };
    let step = span / count as f32;
    dark_plate(b);
    Course {
        count,
        step,
        len: step * 1.12,
        half: front.x * 0.62,
        tip: 0.0,
        thick: 0.2,
        tail: 0.6,
    }
    .lay(
        b,
        &Frame::new(
            v3(tip, 0.0, up * (g.gap + front.y - 0.04)),
            v3(-1.0, 0.0, up * rise),
            v3(0.0, 0.0, up),
        ),
    );
}

/// Swept plates laid back down each flank of a half, by its back, from near its tip.
fn flank_course(b: &mut MeshBuilder, g: &Gun, up: f32) {
    let len = g.len();
    let tip = len - 3.0;
    let from = 2.0 + 1.5;
    let (front, back) = (g.at(tip), g.at(from));
    let span = tip - from;
    let flare = (back.x - front.x) / span;
    let rise = (back.y - front.y) / span;
    b.mirror_y(|b| {
        dark_plate(b);
        Course {
            count: 3,
            step: span / 3.0,
            len: span / 3.0 * 1.1,
            half: front.y * 0.28,
            tip: up,
            thick: 0.16,
            tail: 0.4,
        }
        .lay(
            b,
            &Frame::new(
                v3(tip, front.x * 0.93, up * (g.gap + front.y * 0.62)),
                v3(-1.0, flare * 0.93, up * rise * 0.62),
                Vec3::Y,
            ),
        );
    });
}
