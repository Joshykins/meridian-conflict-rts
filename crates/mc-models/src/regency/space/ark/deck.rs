//! The hull's armour above the wings and forward of them: courses of plates swept back
//! along the deck, the upper slope and the lower flank; the +y armour ridge down the
//! spine and the workings in the trench inside it; the bridge on the foredeck; the
//! prow's keel blade and the lit chine that runs back from it to the wing.
use glam::{Vec2, Vec3};

use super::super::super::kit::{dark_plate, metal, seam, v3};
use super::super::super::machine::{red_slot, Frame};
use super::{at, drum, frame_on, optic, shard, surface, Face};
use crate::builder::MeshBuilder;
use crate::material::GLOW_LASER;

/// The +y side's courses of plates: along the deck outside the ridge, down the upper
/// slope, and on the lower flank under the wing; forward of the wing, the knuckle too.
pub(super) fn flank(b: &mut MeshBuilder) {
    if !b.mid() {
        return;
    }
    let fine = b.fine();
    // (face, across, from x, to x, plates, half width, thickness)
    let courses: &[(Face, f32, f32, f32, usize, f32, f32)] = &[
        (
            Face::Deck,
            0.18,
            100.0,
            -140.0,
            if fine { 12 } else { 6 },
            3.6,
            1.0,
        ),
        (
            Face::Slope,
            0.5,
            132.0,
            -140.0,
            if fine { 14 } else { 7 },
            6.0,
            1.2,
        ),
        (
            Face::Upper,
            0.55,
            146.0,
            104.0,
            if fine { 3 } else { 2 },
            5.0,
            1.0,
        ),
        (
            Face::Lower,
            0.4,
            150.0,
            -146.0,
            if fine { 14 } else { 6 },
            6.0,
            1.2,
        ),
        (
            Face::Belly,
            0.6,
            120.0,
            -60.0,
            if fine { 6 } else { 0 },
            5.0,
            0.8,
        ),
    ];
    for &(face, t, from, to, count, half, thick) in courses {
        let step = (from - to) / count.max(1) as f32;
        for k in 0..count {
            let x = from - step * k as f32;
            let f = frame_on(x, face, t, 0.15);
            // Each plate a shade over its run, so its tail lifts off the next one's head.
            shard(b, &f, step * 1.25, half, 0.3, thick);
        }
    }
    // The knuckle at the beam forward of the wing, lit: the prow's chine.
    let chine: Vec<Vec3> = (0..=6)
        .map(|k| {
            let x = 152.0 - k as f32 * 9.0;
            let (p, n) = surface(x, Face::Upper, 0.0);
            p + n * 0.25
        })
        .collect();
    b.paint(GLOW_LASER);
    for w in chine.windows(2) {
        b.cylinder_between(w[0], w[1], 0.3, 0.3, 4);
    }
    // The red slits along the upper slope over the hold: the hold's lights.
    if fine {
        for k in 0..5 {
            let x = 40.0 - k as f32 * 22.0;
            let (p, n) = surface(x, Face::Slope, 0.82);
            red_slot(b, p + n * 1.0, n, Vec3::X, 6.0, 0.6);
        }
    }
}

/// The +y armour ridge along the spine (behind the bridge), plates swept back off its
/// outer face, and the workings in the trench between the two ridges: a shaft, drums,
/// cable runs, ribs across the trench and red slots between them.
pub(super) fn ridge(b: &mut MeshBuilder) {
    let line = [
        v3(62.0, 8.5, 87.4),
        v3(30.0, 8.5, 89.6),
        v3(-30.0, 8.5, 89.6),
        v3(-88.0, 8.5, 87.6),
        v3(-122.0, 8.5, 83.4),
    ];
    dark_plate(b);
    for w in line.windows(2) {
        b.with_facets(|b| b.beam(w[0], w[1], Vec2::new(5.0, 6.0), Vec2::new(5.0, 6.0)));
    }
    if !b.mid() {
        return;
    }
    let per = if b.fine() { 3 } else { 1 };
    for w in line.windows(2) {
        for j in 0..per {
            let k = (j as f32 + 0.5) / per as f32;
            let at = w[0].lerp(w[1], k) + v3(0.0, 2.2, 2.6);
            let f = Frame::new(at, v3(-1.0, 0.32, 0.05), v3(0.0, 0.8, 1.0));
            shard(b, &f, 26.0 / per as f32 + 8.0, 3.0, 0.8, 1.0);
        }
    }
    for w in line.windows(2) {
        let a = w[0] - v3(0.0, 8.5, 1.6);
        let c = w[1] - v3(0.0, 8.5, 1.6);
        drum(b, a, c - a, 2.6, 5.0);
        if b.fine() {
            metal(b);
            b.cylinder_between(a + v3(0.0, 3.4, 0.0), c + v3(0.0, 3.4, 0.0), 1.1, 1.1, 6);
            b.cylinder_between(a + v3(0.0, 1.8, -0.4), c + v3(0.0, 1.8, -0.4), 0.6, 0.6, 5);
            red_slot(
                b,
                a.lerp(c, 0.5) + v3(0.0, 1.0, 0.2),
                Vec3::Z,
                c - a,
                12.0,
                0.8,
            );
            // Ribs across the trench, ridge to ridge.
            let ribs = ((c - a).length() / 14.0) as usize;
            dark_plate(b);
            for r in 1..ribs.max(2) {
                let p = a.lerp(c, r as f32 / ribs.max(2) as f32);
                b.beam(
                    p + v3(0.0, 0.0, 2.6),
                    p + v3(0.0, 6.4, 2.6),
                    Vec2::new(2.0, 1.4),
                    Vec2::new(2.0, 1.4),
                );
            }
        }
    }
}

/// The bridge: a low armoured wedge on the foredeck, its brow two plates swept back
/// into horns, a row of red optics down each forward face, an armoured visor over them.
pub(super) fn bridge(b: &mut MeshBuilder) {
    let base = |x: f32| at(x).top + 0.4;
    let station = |x: f32, half: f32, h: f32| -> Vec<Vec3> {
        let z = base(x) - 1.2;
        vec![
            v3(x, -half, z),
            v3(x, half, z),
            v3(x, half * 0.82, z + h * 0.55),
            v3(x, half * 0.5, z + h),
            v3(x, -half * 0.5, z + h),
            v3(x, -half * 0.82, z + h * 0.55),
        ]
    };
    dark_plate(b);
    b.with_facets(|b| {
        b.loft(
            &[
                station(60.0, 13.0, 10.0),
                station(86.0, 11.0, 11.0),
                station(104.0, 6.0, 6.5),
                station(116.0, 0.6, 1.5),
            ],
            true,
            true,
        )
    });
    if !b.mid() {
        return;
    }
    let fine = b.fine();
    b.mirror_y(|b| {
        // The brow's horn plates, swept up and back over the bridge's crown.
        let f = Frame::new(
            v3(98.0, 4.0, base(98.0) + 6.6),
            v3(-1.0, 0.28, 0.32),
            v3(0.1, 0.35, 1.0),
        );
        shard(b, &f, 42.0, 3.4, 0.7, 1.6);
        // Optics down the forward face, under the visor.
        for k in 0..if fine { 4 } else { 2 } {
            let x = 104.0 - k as f32 * 5.0;
            let half = 6.0 + (104.0 - x) / 18.0 * 5.0;
            let z = base(x) + 4.0 - k as f32 * 0.2;
            optic(
                b,
                v3(x, half * 0.86 + 0.4, z),
                v3(0.4, 1.0, 0.2),
                v3(1.0, -0.25, 0.0),
                3.2,
            );
        }
        // Plates swept back down the bridge's flanks.
        for k in 0..if fine { 3 } else { 1 } {
            let x = 84.0 - k as f32 * 9.0;
            let f = Frame::new(
                v3(x, 11.5, base(x) + 2.4),
                v3(-1.0, 0.3, 0.0),
                v3(0.0, 1.0, 0.45),
            );
            shard(b, &f, 18.0, 2.8, 0.4, 1.0);
        }
    });
    if fine {
        // The sensor mast behind the bridge: a graphite spindle with two lit collars.
        let foot = v3(62.0, 0.0, base(62.0) + 8.0);
        metal(b);
        b.cylinder_between(foot, foot + v3(-6.0, 0.0, 10.0), 1.4, 0.7, 8);
        seam(b);
        for k in [0.45, 0.8] {
            let c = foot + v3(-6.0, 0.0, 10.0) * k;
            b.cylinder_between(c, c + v3(-0.3, 0.0, 0.5), 1.8, 1.8, 8);
        }
        b.paint(GLOW_LASER);
        b.cylinder_between(
            foot + v3(-6.0, 0.0, 10.0),
            foot + v3(-6.2, 0.0, 10.4),
            0.6,
            0.6,
            6,
        );
    }
}

/// The prow's keel blade under the needle, and its two mandible plates.
pub(super) fn prow(b: &mut MeshBuilder) {
    dark_plate(b);
    b.with_facets(|b| {
        b.extrude_y(
            &[[96.0, 40.0], [164.0, 50.0], [152.0, 54.5], [96.0, 52.0]],
            -1.4,
            1.4,
        )
    });
    if !b.mid() {
        return;
    }
    b.paint(GLOW_LASER);
    b.cylinder_between(v3(162.0, 0.0, 50.6), v3(100.0, 0.0, 41.0), 0.35, 0.35, 4);
    b.mirror_y(|b| {
        let f = Frame::new(
            v3(150.0, 2.0, 51.5),
            v3(-1.0, 0.35, -0.15),
            v3(0.0, 1.0, -0.4),
        );
        shard(b, &f, 34.0, 2.6, -0.6, 1.0);
    });
}
