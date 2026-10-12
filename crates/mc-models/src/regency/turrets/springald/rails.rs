//! The Springald's gun, in the gun's frame (origin at the trunnion, +x down the bore):
//! a breech jacket round the trunnion, and out of it the bore, which runs back into the
//! jacket on the shot (`with_recoil`). The bore is a bottle of pinch coils inside three
//! tall plated rails a third of a turn apart. The jacket's front runs on into a plated
//! fairing that goes from round to three lobes, feathered plates laid down each lobe, and
//! the rails grow out of the lobes, so gun and body are one piece.
//!
//! Each rail is a fixed root seated in its lobe and a blade ahead of it that rides a saddle
//! on the root: plated, a course of swept plates down its back, red slots let into its
//! flanks and clamps round it. Through the charge the three blades draw back off the bore
//! (`charge_gear::FLARE`), the slots rippling out toward the muzzle; they slam shut on the
//! shot. The charge is held at [`LINE`]'s muzzle between three gravity lenses, one on a
//! sleeve at each blade's tip, so the three knots of its triune shot are gathered into one
//! where it leaves from.

use std::f32::consts::TAU;

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::gpu_consts::charge_gear::FLARE;
use crate::pattern;

use super::super::super::kit::{dark_plate, metal, seam, slab, v3};
use super::super::super::machine::{armour, hoop_on, swept, Course, Frame};
use super::super::super::parts::{drum, inlay, ridged};
use super::super::fusion_kit::{coil_light, lens};
use super::super::{bar_through, hull_x, CHAMFERED};
use super::{CHEEK, LINE};

/// The three bearings round the bore, the first straight up.
fn bearings() -> [f32; 3] {
    [0.0, 1.0, 2.0].map(|k| TAU * 0.25 + TAU * k / 3.0)
}

/// Out from the bore at bearing `a`, across it.
fn radial(a: f32) -> Vec3 {
    v3(0.0, a.cos(), a.sin())
}

/// Round the bore at bearing `a`, the way the bearing turns.
fn round(a: f32) -> Vec3 {
    v3(0.0, -a.sin(), a.cos())
}

/// `x` down the bore and `r` out from it at bearing `a`.
fn at(x: f32, a: f32, r: f32) -> Vec3 {
    v3(x, 0.0, 0.0) + radial(a) * r
}

/// Rails' distance out from the bore at their roots, where the blades start, and at their
/// tips.
const ROOT_R: f32 = 4.4;
const RAIL_R: f32 = 3.7;
const TIP_R: f32 = 3.1;
/// Where the blade leaves its root's saddle, and where its spine runs straightest.
const SADDLE: f32 = 27.0;
const KNEE: f32 = 36.0;
/// How far back from the charge the blades' tips run level with the bore.
const TRACK: f32 = 8.0;
/// The jacket's front, where the bore slides out of it.
const MOUTH: f32 = 9.0;

/// The breech jacket round the trunnion, fixed on the trunnion; the bore slides back into
/// it. A plated block, the gun's thickest part, a ridged plate lapped back over its top,
/// plates lapped down its flanks swept back into spikes, buffer boxes on its flanks and a
/// lit gasket in its back.
pub(super) fn breech(b: &mut MeshBuilder) {
    drum(b, Vec3::ZERO, Vec3::Y, 2.6, (CHEEK + 0.4) * 2.0);
    dark_plate(b);
    b.with_facets(|b| {
        hull_x(
            b,
            &[
                [-4.0, 8.6, 6.0, 0.0],
                [-1.8, 10.0, 8.6, 0.0],
                [5.0, 10.6, 9.8, 0.0],
                [MOUTH, 10.0, 9.8, 0.0],
            ],
            &CHAMFERED,
        )
    });
    // The collar at its mouth that the bore runs in.
    seam(b);
    hoop_on(b, v3(MOUTH, 0.0, 0.0), Vec3::X, 4.2, 1.6, 0.6, b.sides(16));
    if !b.mid() {
        return;
    }
    let fine = b.fine();
    dark_plate(b);
    ridged(
        b,
        &Frame::new(v3(MOUTH + 0.4, 0.0, 4.88), -Vec3::X, Vec3::Z),
        &[
            [0.0, -3.8],
            [0.0, 3.8],
            [8.0, 3.6],
            [12.8, 1.2],
            [12.8, -1.2],
            [8.0, -3.6],
        ],
        0.35,
        [5.0, 0.0],
        0.5,
    );
    if fine {
        inlay(
            b,
            &[
                v3(-4.02, -2.4, -1.6),
                v3(-4.02, 2.4, -1.6),
                v3(-4.02, 2.4, 1.6),
                v3(-4.02, -2.4, 1.6),
            ],
            -Vec3::X,
            pattern::EMBER_RIM,
        );
    }
    b.mirror_y(|b| {
        dark_plate(b);
        armour(
            b,
            &Frame::new(v3(MOUTH + 0.3, 5.32, 1.0), -Vec3::X, Vec3::Y),
            &[
                [0.0, -1.8],
                [0.0, 2.6],
                [8.0, 2.5],
                [13.6, 1.0],
                [8.0, -1.6],
            ],
            0.35,
        );
        armour(
            b,
            &Frame::new(v3(MOUTH - 0.6, 5.32, -2.2), -Vec3::X, Vec3::Y),
            &[
                [0.0, -1.6],
                [0.0, 1.2],
                [7.0, 1.1],
                [11.4, -0.6],
                [7.0, -1.5],
            ],
            0.3,
        );
        if fine {
            inlay(
                b,
                &[
                    v3(MOUTH - 1.4, 5.66, -2.4),
                    v3(1.0, 5.66, -2.4),
                    v3(1.0, 5.66, -1.9),
                    v3(MOUTH - 1.4, 5.66, -1.9),
                ],
                Vec3::Y,
                pattern::EMBER_SLOT,
            );
            dark_plate(b);
            b.chamfered_box(v3(4.6, 5.6, 3.6), v3(5.4, 1.2, 1.6), 0.35);
            inlay(
                b,
                &[
                    v3(2.2, 6.22, 3.1),
                    v3(7.0, 6.22, 3.1),
                    v3(7.0, 6.22, 4.1),
                    v3(2.2, 6.22, 4.1),
                ],
                Vec3::Y,
                pattern::EMBER_MACHINED,
            );
        }
    });
}

/// The bore past the jacket: everything that runs back on the shot.
pub(super) fn bore(b: &mut MeshBuilder) {
    let len = LINE.len();
    let fine = b.fine();
    fairing(b);
    metal(b);
    b.cylinder_between(
        v3(MOUTH - 1.0, 0.0, 0.0),
        v3(len - 3.5, 0.0, 0.0),
        1.45,
        1.1,
        b.sides(10),
    );
    let stack: &[(f32, f32, u32)] = if fine {
        &[
            (29.5, 2.35, 1),
            (32.8, 2.25, 2),
            (36.1, 2.2, 3),
            (39.4, 2.1, 4),
            (42.7, 2.0, 5),
            (46.0, 1.9, 6),
        ]
    } else {
        &[(32.8, 2.25, 2), (39.4, 2.1, 4), (46.0, 1.9, 6)]
    };
    coils(b, stack);
    for a in bearings() {
        root(b, a);
        b.with_charge_gear(FLARE, |b| blade(b, a, len));
    }
}

/// A rail's root, seated in its lobe of the fairing: a plated bar out of the lobe to the
/// saddle the blade rides on, a plated web under the saddle down to the bore.
fn root(b: &mut MeshBuilder, a: f32) {
    dark_plate(b);
    bar_through(
        b,
        &[
            (at(12.0, a, ROOT_R), Vec2::new(2.4, 3.2)),
            (at(SADDLE + 2.0, a, RAIL_R + 0.3), Vec2::new(1.9, 2.4)),
        ],
        round(a),
    );
    seam(b);
    bar_through(
        b,
        &[
            (at(SADDLE - 0.6, a, RAIL_R + 0.1), Vec2::new(2.6, 2.6)),
            (at(SADDLE + 2.4, a, RAIL_R), Vec2::new(2.6, 2.6)),
        ],
        round(a),
    );
    if b.fine() {
        dark_plate(b);
        slab(
            b,
            [
                at(SADDLE - 4.0, a, 2.0),
                at(SADDLE + 1.5, a, 2.0),
                at(SADDLE + 1.5, a, RAIL_R - 0.9),
                at(SADDLE - 5.5, a, ROOT_R - 0.9),
            ],
            round(a) * 0.5,
        );
    }
}

/// The blade's distance out from the bore at `x`, along its spine.
fn rail_r(x: f32) -> f32 {
    let tip = LINE.len() - TRACK;
    if x < KNEE {
        RAIL_R + 0.2 - 0.3 * ((x - SADDLE) / (KNEE - SADDLE)).clamp(0.0, 1.0)
    } else {
        RAIL_R - 0.1 - (RAIL_R - 0.1 - TIP_R) * ((x - KNEE) / (tip - KNEE)).clamp(0.0, 1.0)
    }
}

/// Half the blade's width across the bore at `x`.
fn half_w(x: f32) -> f32 {
    let tip = LINE.len() - TRACK;
    if x < KNEE {
        0.85 - 0.15 * ((x - SADDLE) / (KNEE - SADDLE)).clamp(0.0, 1.0)
    } else {
        0.7 - 0.15 * ((x - KNEE) / (tip - KNEE)).clamp(0.0, 1.0)
    }
}

/// A rail's blade from its saddle to its tip, all of it charge gear: a plated bar level
/// with the bore over its last stretch, a ridged lip over its heel, a course of swept
/// plates down its back, red slots let into both flanks (they ripple with the charge,
/// `charge_gear::SLOT_TAG`), clamps round it and the lens's sleeve at its tip.
fn blade(b: &mut MeshBuilder, a: f32, len: f32) {
    let fine = b.fine();
    dark_plate(b);
    let spine = [
        (at(SADDLE - 0.2, a, rail_r(SADDLE)), Vec2::new(1.7, 2.4)),
        (at(KNEE, a, rail_r(KNEE)), Vec2::new(1.4, 2.0)),
        (at(len - TRACK, a, TIP_R), Vec2::new(1.1, 1.4)),
        (at(len - 1.8, a, TIP_R), Vec2::new(0.8, 1.0)),
    ];
    b.with_facets(|b| bar_through(b, &spine, round(a)));
    if !b.mid() {
        sleeve(b, len - 4.9, a, len);
        return;
    }
    ridged(
        b,
        &Frame::new(
            at(SADDLE - 0.6, a, rail_r(SADDLE) + 1.2),
            Vec3::X,
            radial(a),
        ),
        &[[0.0, -1.3], [3.6, -1.1], [4.6, 0.0], [3.6, 1.1], [0.0, 1.3]],
        0.3,
        [1.6, 0.0],
        0.3,
    );
    dark_plate(b);
    Course {
        count: if fine { 4 } else { 2 },
        step: 4.6,
        len: 5.4,
        half: 0.7,
        tip: 0.0,
        thick: 0.28,
        tail: 1.4,
    }
    .lay(
        b,
        &Frame::new(at(len - 9.0, a, TIP_R + 0.68), -Vec3::X, radial(a)),
    );
    if fine {
        dark_plate(b);
        b.pattern(pattern::EMBER_SLOT);
        for (x0, x1) in [
            (SADDLE + 2.5, 33.5),
            (35.5, 40.5),
            (42.5, len - TRACK - 0.5),
        ] {
            let (r0, r1) = (rail_r(x0), rail_r(x1));
            for side in [1.0f32, -1.0] {
                let p = |x: f32, r: f32, o: f32| at(x, a, r + o) + round(a) * (half_w(x) * side);
                slab(
                    b,
                    [
                        p(x0, r0, -0.35),
                        p(x1, r1, -0.35),
                        p(x1, r1, 0.25),
                        p(x0, r0, 0.25),
                    ],
                    round(a) * (side * 0.06),
                );
            }
        }
        for x in [34.5f32, 41.5] {
            let r = rail_r(x);
            let s = Vec2::new(half_w(x) * 2.0 + 0.4, 2.1);
            metal(b);
            bar_through(
                b,
                &[(at(x - 0.35, a, r), s), (at(x + 0.35, a, r), s)],
                round(a),
            );
        }
    }
    sleeve(b, len - 4.9, a, len);
}

/// A lens's sleeve round the blade at bearing `a`, its middle `x` down the bore: a plated
/// collar a little proud of the blade all round, a swept plate over it, the lens on a stalk
/// off its inner face, ahead of the last pinch coil, looking at the charge at `charge`
/// down the bore.
fn sleeve(b: &mut MeshBuilder, x: f32, a: f32, charge: f32) {
    dark_plate(b);
    bar_through(
        b,
        &[
            (at(x - 1.1, a, TIP_R), Vec2::new(1.6, 1.7)),
            (at(x + 1.1, a, TIP_R), Vec2::new(1.5, 1.6)),
        ],
        round(a),
    );
    if b.mid() {
        armour(
            b,
            &Frame::new(at(x + 1.3, a, TIP_R + 0.85), -Vec3::X, radial(a)),
            &swept(3.6, 0.8, 0.0, 0.5),
            0.22,
        );
    }
    let face = TIP_R - 0.85;
    metal(b);
    b.cylinder_between(
        at(x + 0.3, a, face + 0.05),
        at(x + 1.2, a, face - 0.5),
        0.35,
        0.3,
        b.sides(8),
    );
    lens(b, at(x + 1.4, a, face - 0.55), v3(charge, 0.0, 0.0), 0.6);
}

/// The fairing's stations: x, middle radius, how far the lobes stand out and the hollows
/// sink.
const STATIONS: [(f32, f32, f32); 5] = [
    (MOUTH - 1.0, 4.0, 0.1),
    (13.0, 4.4, 0.6),
    (19.0, 3.7, 1.0),
    (25.0, 3.0, 1.3),
    (30.0, 2.5, 1.5),
];

/// The fairing's (middle, lobe) at `x`, between its stations.
fn station(x: f32) -> (f32, f32) {
    let s = &STATIONS;
    let k = s
        .windows(2)
        .position(|w| x <= w[1].0)
        .unwrap_or(s.len() - 2);
    let (s0, s1) = (s[k], s[k + 1]);
    let t = ((x - s0.0) / (s1.0 - s0.0)).clamp(0.0, 1.0);
    (s0.1 + (s1.1 - s0.1) * t, s0.2 + (s1.2 - s0.2) * t)
}

/// The jacket's front run on into the rails: a plated fairing lofted from the jacket's
/// width, narrowing into three lobes, one under each rail's root, the hollows between
/// them closing onto the bore, so the gun is thickest at its neck and tapers to the
/// muzzle. Feathered plates are laid down each lobe's flanks, a red line in each hollow,
/// and graphite bands clamp it, a plated saddle over each lobe.
fn fairing(b: &mut MeshBuilder) {
    let n = if b.fine() { 36 } else { 12 };
    let rings: Vec<Vec<Vec3>> = STATIONS
        .iter()
        .map(|&(x, mid, lobe)| {
            (0..n)
                .map(|i| {
                    let t = TAU * i as f32 / n as f32;
                    // Three lobes, the first straight up (bearing a quarter turn).
                    let r = mid + lobe * (3.0 * (t - TAU * 0.25)).cos();
                    at(x, t, r.max(0.8))
                })
                .collect()
        })
        .collect();
    dark_plate(b);
    b.loft(&rings, true, true);
    if !b.mid() {
        return;
    }
    let fine = b.fine();
    let crest = |x: f32| {
        let (mid, lobe) = station(x);
        mid + lobe
    };
    let hollow = |x: f32| {
        let (mid, lobe) = station(x);
        mid - lobe
    };
    // The fairing's radius at `x` and bearing `t`.
    let surface = |x: f32, t: f32| {
        let (mid, lobe) = station(x);
        mid + lobe * (3.0 * (t - TAU * 0.25)).cos()
    };
    for a in bearings() {
        for side in [-1.0f32, 1.0] {
            let off = a + side * 0.42;
            let (x0, x1) = (24.0, 11.0);
            let p0 = at(x0, off, surface(x0, off) + 0.05);
            let p1 = at(x1, off, surface(x1, off) + 0.05);
            dark_plate(b);
            Course {
                count: if fine { 3 } else { 2 },
                step: 4.2,
                len: 4.8,
                half: 0.9,
                tip: -side * 0.6,
                thick: 0.26,
                tail: 1.2,
            }
            .lay(b, &Frame::new(p0, p1 - p0, radial(off)));
        }
        if fine {
            let h = a + TAU / 6.0;
            let (x0, x1) = (14.0, 27.0);
            inlay(
                b,
                &[
                    at(x0, h - 0.05, hollow(x0)),
                    at(x1, h - 0.07, hollow(x1)),
                    at(x1, h + 0.07, hollow(x1)),
                    at(x0, h + 0.05, hollow(x0)),
                ],
                radial(h),
                pattern::EMBER_SLOT,
            );
        }
    }
    for x in [12.0f32, 21.0] {
        metal(b);
        hoop_on(
            b,
            v3(x, 0.0, 0.0),
            Vec3::X,
            hollow(x) + 0.25,
            0.6,
            0.9,
            b.sides(18),
        );
        if fine {
            for a in bearings() {
                dark_plate(b);
                ridged(
                    b,
                    &Frame::new(at(x - 0.9, a, crest(x) - 0.15), Vec3::X, radial(a)),
                    &[[0.0, -1.5], [1.8, -1.5], [1.8, 1.5], [0.0, 1.5]],
                    0.3,
                    [0.9, 0.0],
                    0.25,
                );
            }
        }
    }
}

/// Pinch coils down the bore at `(x, r, stage)`, each a plated ring wound in graphite with
/// a fusion light inside, lit from the breech through the charge, and three plated lugs
/// round it between the rails.
fn coils(b: &mut MeshBuilder, coils: &[(f32, f32, u32)]) {
    let segs = b.sides(16);
    let fine = b.fine();
    for &(x, r, stage) in coils {
        let c = Vec3::X * x;
        dark_plate(b);
        hoop_on(b, c, Vec3::X, r, 1.0, 1.3, segs);
        metal(b);
        hoop_on(b, c, Vec3::X, r + 0.6, 0.3, 0.9, segs);
        coil_light(b, stage);
        hoop_on(b, c, Vec3::X, r - 0.6, 0.2, 0.8, segs);
        if fine {
            for a in bearings() {
                let h = a + TAU / 6.0;
                dark_plate(b);
                bar_through(
                    b,
                    &[
                        (at(x - 0.5, h, r + 0.3), Vec2::new(0.9, 0.9)),
                        (at(x + 0.5, h, r + 0.3), Vec2::new(0.9, 0.9)),
                    ],
                    round(h),
                );
            }
        }
    }
}
