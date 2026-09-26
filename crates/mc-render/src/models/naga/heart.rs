//! The Heart, the Naga power plant: a star core. Three tiers, each its own structure: the
//! Heart on a 2 x 2 lot (24 m square), tech 2 on a 4 x 4 (48 m), tech 3 on an 8 x 8
//! (96 m). A higher tier holds a bigger star in more cage (docs/STYLE.md, "The Naga
//! suite").
//!
//! Gravity pinches plasma into a small star, and the plant is the cage that holds it:
//!
//! - The star in the middle: a white-hot core wrapped in red flares (`GLOW_LAMP`,
//!   `GLOW_LASER`).
//! - The gravity cage round it: fixed bronze rings crossed about it, and turning ones
//!   (`part::SPINNER`) with their weights; tech 1 has two fixed and one turning, tech 2
//!   three and two, tech 3 four and three.
//! - Armoured pylons round it (four, six, eight), each on its own foot with plates lapped
//!   out toward the lot's edge, aim bronze pinch emitters at the star, their tips lit red,
//!   and carry a plated frame over it, open in the middle so the star shows from above.
//!   The owner's colour is on the pylons' heads.
//! - Coolant rams work in turn between the pylons (`part::PUMP`).
//! - From tech 2 each pylon is banded with bronze field collars; tech 3 adds a second
//!   row of pinch emitters, aimed down at the star from the frame, and swept plates
//!   lapped out over the frame.
//!
//! The tiers are one machine drawn bigger (`Plant::scale`) with more of it.
//! Nothing on it can go off: a breached cage lets the star fall in on itself and go out
//! (no Heart has a death blast).

use glam::{Affine3A, Vec2, Vec3};

use crate::models::builder::MeshBuilder;
use crate::models::material::*;
use crate::models::part;

use super::kit::{dark_plate, metal, seam, v3};
use super::machine::*;

/// The star's middle (and the spinner's pivot) and its core's radius, at tech 1's size.
const STAR: Vec3 = Vec3::new(0.0, 0.0, 4.0);
const CORE_R: f32 = 1.35;
/// How far out the pylons stand, and their tops.
const PYLON_R: f32 = 5.2;
const TOP: f32 = 7.0;
const THICK: f32 = 0.4;

/// One tier of the plant: how many pylons, fixed and turning cage rings and flares it
/// has, whether it has the crown (tech 3's second emitter row and frame plates), and how
/// much bigger than tech 1 it is drawn (across, across, up).
struct Plant {
    pylons: usize,
    fixed: usize,
    turning: usize,
    flares: usize,
    collars: bool,
    crown: bool,
    scale: Vec3,
    /// The pylons' width against tech 1's: more of them stand slimmer.
    slim: f32,
}

const HEART: [Plant; 3] = [
    Plant {
        pylons: 4,
        fixed: 2,
        turning: 1,
        flares: 3,
        collars: false,
        crown: false,
        scale: Vec3::ONE,
        slim: 1.0,
    },
    Plant {
        pylons: 6,
        fixed: 3,
        turning: 2,
        flares: 4,
        collars: true,
        crown: false,
        scale: Vec3::new(2.1, 2.1, 2.4),
        slim: 0.85,
    },
    Plant {
        pylons: 8,
        fixed: 4,
        turning: 3,
        flares: 5,
        collars: true,
        crown: true,
        scale: Vec3::new(3.9, 3.9, 4.7),
        slim: 0.7,
    },
];

impl Plant {
    /// The pylons' bearings: on the lot's diagonals (tech 1), else spread evenly with
    /// none on an axis.
    fn bearings(&self) -> impl Iterator<Item = f32> + '_ {
        let step = 360.0 / self.pylons as f32;
        (0..self.pylons).map(move |k| (step * 0.5 + step * k as f32).to_radians())
    }
}

pub(super) fn heart(b: &mut MeshBuilder, _tech: u8) {
    plant(b, &HEART[0]);
}

pub(super) fn heart_2(b: &mut MeshBuilder, _tech: u8) {
    plant(b, &HEART[1]);
}

pub(super) fn heart_3(b: &mut MeshBuilder, _tech: u8) {
    plant(b, &HEART[2]);
}

fn plant(b: &mut MeshBuilder, p: &Plant) {
    b.with(Affine3A::from_scale(p.scale), |b| {
        b.set_spinner_pivot(STAR);
        if b.coarse() {
            coarse(b, p);
            return;
        }
        star(b, p);
        cage(b, p);
        for a in p.bearings() {
            b.yawed(Vec3::ZERO, a, |b| {
                b.with(Affine3A::from_scale(v3(1.0, p.slim, 1.0)), |b| pylon(b, p));
            });
        }
        frame(b, p);
        rams(b, p);
    });
}

/// Far off: a fin for each pylon with the owner's colour on its head, the frame as one
/// band, the star.
fn coarse(b: &mut MeshBuilder, p: &Plant) {
    for a in p.bearings() {
        b.yawed(Vec3::ZERO, a, |b| {
            let (foot, head, tip) = (
                v3(PYLON_R - 1.2, 0.0, 0.0),
                v3(PYLON_R - 0.3, 0.0, TOP),
                v3(PYLON_R + 5.0, 0.0, 0.2),
            );
            dark_plate(b);
            b.face(&[foot, tip, head]);
            b.face(&[head, tip, foot]);
            b.paint(TEAM);
            b.face(&[
                v3(PYLON_R - 1.4, -0.8, TOP + 0.02),
                v3(PYLON_R + 0.2, -0.8, TOP + 0.02),
                v3(PYLON_R + 0.2, 0.8, TOP + 0.02),
                v3(PYLON_R - 1.4, 0.8, TOP + 0.02),
            ]);
        });
    }
    b.paint(GLOW_LAMP);
    b.cylinder_between(
        STAR - Vec3::Z * CORE_R,
        STAR + Vec3::Z * CORE_R,
        CORE_R,
        CORE_R * 0.6,
        3,
    );
}

/// The star: a white-hot core wrapped in red flares, each a ring round it at its own
/// tilt.
fn star(b: &mut MeshBuilder, p: &Plant) {
    let fine = b.fine();
    b.paint(GLOW_LAMP);
    let sides = b.sides(12);
    b.spheroid(STAR, Vec3::splat(CORE_R), sides, if fine { 8 } else { 5 });
    b.paint(GLOW_LASER);
    let flares = if fine { p.flares } else { 1 };
    for k in 0..flares {
        let a = (360.0 / flares as f32 * k as f32).to_radians();
        let lean = 0.35 + 0.25 * (k % 2) as f32;
        hoop_on(
            b,
            STAR,
            v3(a.cos(), a.sin(), lean),
            CORE_R * (1.3 + 0.06 * (k % 3) as f32),
            0.16,
            0.34,
            if fine { 16 } else { 8 },
        );
    }
}

/// The gravity cage: fixed bronze rings crossed about the star, and the turning ones,
/// each leaning its own way, with their weights.
fn cage(b: &mut MeshBuilder, p: &Plant) {
    let fine = b.fine();
    let segs = if fine { 24 } else { 10 };
    metal(b);
    for k in 0..p.fixed {
        // Crossed in pairs: the first two lean across y, the next two across x.
        let tilt = if k % 2 == 0 { 0.8 } else { -0.8 };
        let axis = if k < 2 {
            v3(0.0, tilt, 1.0)
        } else {
            v3(tilt, 0.0, 1.0)
        };
        hoop_on(b, STAR, axis, 2.8 - 0.1 * k as f32, 0.4, 0.5, segs);
    }
    b.with_part(part::SPINNER, |b| {
        for k in 0..p.turning {
            // The first turns level; the rest lean well over, each a different way, so
            // they sweep through the cage as they go round.
            let (lean, toward) = if k == 0 {
                (0.0, 0.0)
            } else {
                (38.0f32.to_radians(), (150.0 * k as f32).to_radians())
            };
            let r = 3.5 + 0.35 * k as f32;
            b.yawed(STAR, toward, |b| {
                b.pitched(Vec3::ZERO, lean, |b| {
                    metal(b);
                    hoop(b, Vec3::ZERO, r, 0.55, 0.55, segs);
                    dark_plate(b);
                    for w in 0..4 {
                        let a = (90.0 * w as f32 + 45.0 * k as f32).to_radians();
                        let d = v3(a.cos(), a.sin(), 0.0);
                        b.cuboid(d * r, v3(1.0, 1.0, 1.0));
                    }
                });
            });
        }
    });
}

/// One pylon, standing out along +x (turned into place by the caller): its foot with
/// plates lapped out toward the lot's edge, an armoured post, a bronze pinch emitter
/// aimed at the star with its tip lit red, the owner's colour on its head; field
/// collars round the post from tech 2, and from tech 3 a second emitter aimed down at
/// the star from the head.
fn pylon(b: &mut MeshBuilder, p: &Plant) {
    let fine = b.fine();
    let x = PYLON_R;
    seam(b);
    b.frustum_open(
        v3(x + 0.4, 0.0, 0.0),
        Vec2::new(3.6, 3.0),
        Vec2::new(3.0, 2.6),
        0.8,
        Vec2::ZERO,
    );
    let f = Frame::new(v3(x + 1.4, 0.0, 0.9), v3(1.0, 0.0, -0.18), Vec3::Z);
    dark_plate(b);
    Course {
        count: if fine { 2 } else { 1 },
        step: 1.6,
        len: 2.6,
        half: 1.5,
        tip: 0.0,
        thick: THICK,
        tail: 0.5,
    }
    .lay(b, &f);
    dark_plate(b);
    b.frustum_open(
        v3(x, 0.0, 0.8),
        Vec2::new(2.2, 2.0),
        Vec2::new(1.5, 1.5),
        TOP - 0.8,
        Vec2::new(-0.5, 0.0),
    );
    let f = Frame::new(
        v3(x + 0.35, 0.0, TOP - 0.3),
        v3(0.3, 0.0, -1.0),
        v3(1.0, 0.0, 0.3),
    );
    dark_plate(b);
    Course {
        count: 2,
        step: 2.0,
        len: 3.0,
        half: 1.3,
        tip: 0.0,
        thick: THICK,
        tail: 0.8,
    }
    .lay(b, &f);
    b.paint(TEAM);
    b.face(&[
        v3(x - 1.1, -0.6, TOP + 0.02),
        v3(x + 0.1, -0.6, TOP + 0.02),
        v3(x + 0.1, 0.6, TOP + 0.02),
        v3(x - 1.1, 0.6, TOP + 0.02),
    ]);
    // The pinch emitter: a bronze barrel from the post toward the star, its tip red.
    emitter(
        b,
        v3(x - 0.8, 0.0, STAR.z),
        v3(CORE_R + 1.0, 0.0, STAR.z),
        0.6,
    );
    if fine {
        red_slot(b, v3(x + 0.62, 0.0, 2.4), Vec3::X, Vec3::Z, 1.4, 0.18);
    }
    if p.collars && fine {
        for z in [1.6f32, 5.6] {
            collar(b, v3(x - 0.1, 0.0, z), Vec3::Z, 1.25, 0.35);
        }
        // Plates lapped down each flank of the post, over a ribbed bronze spine.
        ribbed(
            b,
            v3(x + 0.95, 0.0, 1.0),
            v3(x + 0.75, 0.0, TOP - 1.2),
            0.22,
            4,
        );
        b.mirror_y(|b| {
            let f = Frame::new(
                v3(x, 1.05, TOP - 1.4),
                v3(0.12, 0.1, -1.0),
                v3(0.2, 1.0, 0.0),
            );
            dark_plate(b);
            Course {
                count: 3,
                step: 1.5,
                len: 2.0,
                half: 0.6,
                tip: 0.6,
                thick: THICK * 0.7,
                tail: 0.5,
            }
            .lay(b, &f);
        });
    }
    if p.crown {
        // The upper emitter, down from under the head at the star.
        let from = v3(x - 1.1, 0.0, TOP - 0.9);
        let to = STAR + (from - STAR).normalize() * (CORE_R + 1.3);
        emitter(b, from, to, 0.45);
        // Swept plates lapped out over the frame from the head, points inward.
        let f = Frame::new(v3(x - 0.7, 0.0, TOP + 0.1), v3(-1.0, 0.0, -0.12), Vec3::Z);
        dark_plate(b);
        Course {
            count: 1,
            step: 0.0,
            len: 1.4,
            half: 0.7,
            tip: 0.0,
            thick: THICK * 0.8,
            tail: 0.6,
        }
        .lay(b, &f);
    }
}

/// A pinch emitter from `from` toward the star, ending at `to`: a bronze collar on its
/// mount, a bronze barrel, its tip lit red.
fn emitter(b: &mut MeshBuilder, from: Vec3, to: Vec3, mount: f32) {
    let fine = b.fine();
    let d = (to - from).normalize();
    collar(b, from, d, mount, 0.8);
    metal(b);
    b.cylinder_between(from, to, 0.3, 0.22, if fine { 8 } else { 6 });
    b.paint(GLOW_LASER);
    b.cylinder_between(to, to + d * 0.25, 0.24, 0.1, 6);
}

/// The plated frame over the cage: a beam from each pylon's head to the next, open in
/// the middle so the star shows from above.
fn frame(b: &mut MeshBuilder, p: &Plant) {
    dark_plate(b);
    let at = |a: f32| v3(a.cos(), a.sin(), 0.0) * (PYLON_R - 0.6) + Vec3::Z * (TOP - 0.5);
    let heads: Vec<f32> = p.bearings().collect();
    for (k, &a0) in heads.iter().enumerate() {
        let a1 = heads[(k + 1) % heads.len()];
        b.beam(at(a0), at(a1), Vec2::new(0.9, 0.8), Vec2::new(0.9, 0.8));
    }
}

/// The coolant rams between the pylons, each on a block, working in turn.
fn rams(b: &mut MeshBuilder, p: &Plant) {
    let step = 360.0 / p.pylons as f32;
    let reach = if p.pylons > 6 { 6.1 } else { 6.4 };
    for k in 0..p.pylons {
        let a = (step * k as f32).to_radians();
        let at = v3(a.cos(), a.sin(), 0.0) * reach;
        dark_plate(b);
        b.block(at - v3(0.9, 0.9, 0.0), at + v3(0.9, 0.9, 0.8));
        piston(b, at + Vec3::Z * 0.8, at + Vec3::Z * 4.0, 0.4, true);
    }
}

#[cfg(test)]
mod tests {
    use crate::models::{build_model_scaled, part};

    /// Each tier's mesh at its unit file's size and lot.
    const TIERS: [(&str, f32, f32, u32); 3] = [
        ("naga_heart", 6.9, 7.5, 2),
        ("naga_heart_2", 18.75, 18.0, 4),
        ("naga_heart_3", 42.5, 35.0, 8),
    ];

    #[test]
    fn heart() {
        for (key, r, h, cells) in TIERS {
            super::super::check(key, r, h, Some(cells), &[]);
            let model = build_model_scaled(key, r, h, 1).unwrap();
            for kind in [part::PUMP, part::SPINNER] {
                assert!(
                    model.lods[0].vertices.iter().any(|v| v.part == kind),
                    "{key}: no part {kind}"
                );
            }
        }
    }

    /// A higher tier holds a bigger star.
    #[test]
    fn a_higher_tier_holds_a_bigger_star() {
        let star = |(key, r, h, _): (&str, f32, f32, u32)| {
            let model = build_model_scaled(key, r, h, 1).unwrap();
            let lamp: Vec<_> = model.lods[0]
                .vertices
                .iter()
                .filter(|v| v.material == crate::models::material::GLOW_LAMP)
                .map(|v| v.pos[0])
                .collect();
            lamp.iter().fold(f32::MIN, |a, &b| a.max(b))
                - lamp.iter().fold(f32::MAX, |a, &b| a.min(b))
        };
        let sizes = TIERS.map(star);
        assert!(sizes[0] < sizes[1] && sizes[1] < sizes[2], "{sizes:?}");
    }
}
