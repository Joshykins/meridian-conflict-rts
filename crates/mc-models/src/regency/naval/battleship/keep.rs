//! The Flamberge's superstructure: a dreadnought's ziggurat, faceted and alien. Broad
//! tiers step in amidships, each sloped, capped with a plated eave and lapped in plates
//! swept back into spikes, a slot of red along its shoulder, a faceted glacis over its
//! nose; a horned bridge on the top (a narrow head, a red visor across its raked face, two
//! horns swept up and back); great swept buttress plates brace the lowest tier down to the
//! deck edges, clear of the beam secondaries.
//!
//! It carries the AA house on a tier at [`CROWN`] and the gravity lenses on its tiers
//! ([`lens_foot`]).

use glam::{Vec2, Vec3};

use crate::builder::{MeshBuilder, Section};
use crate::pattern;

use super::super::super::kit::{dark_plate, metal, seam, v3};
use super::super::super::machine::{armour, swept, Course, Frame};
use super::super::super::parts::{inlay, ridged};
use super::body::CROWN;
use super::hull::{deck, mirrored, tier, top_at, Station};

/// The tops the gravity lenses stand on: the forward pair's and the after pair's.
const FORE_LENS_FOOT: f32 = 12.45;
const AFT_LENS_FOOT: f32 = 9.85;

/// Where the lens at `at` stands: on the top of the tier under it.
pub(super) fn lens_foot(at: Vec3) -> f32 {
    if at.x > -10.0 {
        FORE_LENS_FOOT
    } else {
        AFT_LENS_FOOT
    }
}

/// One tier of a keep: centred at `x`, its plan's half (y >= 0, nose round to tail) about
/// that, from `z0` up to `z1` (the lowest tier from the deck).
struct Tier {
    x: f32,
    plan: &'static [[f32; 2]],
    z0: f32,
    z1: f32,
}

impl Tier {
    /// Its widest half-beam and the x it is widest at.
    fn shoulder(&self) -> (f32, f32) {
        self.plan.iter().fold(
            (0.0, 0.0),
            |(w, at), &[px, py]| {
                if py > w {
                    (py, self.x + px)
                } else {
                    (w, at)
                }
            },
        )
    }

    /// Its nose and tail along x.
    fn ends(&self) -> (f32, f32) {
        let xs = self.plan.iter().map(|p| p[0]);
        (
            self.x + xs.clone().fold(f32::MIN, f32::max),
            self.x + xs.fold(f32::MAX, f32::min),
        )
    }
}

/// The superstructure on the hull `h`.
pub(super) fn build(b: &mut MeshBuilder, h: &[Station]) {
    let tiers = &ZIGGURAT;
    let base = deck(h, -6.0) - 0.4;
    if b.coarse() {
        // Far off, the whole keep is its foot drawn in as it climbs to its top.
        let top = tiers.iter().map(|t| t.z1).fold(0.0, f32::max);
        let t = &tiers[0];
        tier(
            b,
            t.x,
            &mirrored(t.plan),
            base,
            top - 1.0,
            Vec2::new(0.4, 0.45),
        );
        return;
    }
    for t in tiers {
        let z0 = if t.z0 == 0.0 { base } else { t.z0 };
        tier(b, t.x, &mirrored(t.plan), z0, t.z1, Vec2::new(SLOPE, SLOPE));
        eave(b, t);
    }
    for t in tiers {
        dress(b, t);
    }
    head(b, v3(1.4, 0.0, 18.2), 1.1);
    buttresses(b, h);
}

/// The ziggurat: broad tiers stepping in amidships, the bridge's head on the top, the AA
/// house on a shelf of the third.
const ZIGGURAT: [Tier; 5] = [
    Tier {
        x: -8.0,
        plan: &[
            [19.0, 0.0],
            [12.0, 5.0],
            [-10.0, 6.6],
            [-18.0, 6.2],
            [-22.0, 3.0],
        ],
        z0: 0.0,
        z1: AFT_LENS_FOOT,
    },
    Tier {
        x: -4.0,
        plan: &[
            [13.0, 0.0],
            [8.0, 4.6],
            [-6.0, 6.2],
            [-11.0, 5.4],
            [-14.0, 2.6],
        ],
        z0: AFT_LENS_FOOT - 0.2,
        z1: FORE_LENS_FOOT,
    },
    Tier {
        x: -3.0,
        plan: &[
            [9.0, 0.0],
            [6.0, 3.8],
            [-4.0, 4.4],
            [-8.0, 3.6],
            [-9.5, 1.6],
        ],
        z0: FORE_LENS_FOOT - 0.2,
        z1: CROWN,
    },
    Tier {
        x: 0.5,
        plan: &[[5.0, 0.0], [3.4, 3.0], [-2.6, 3.2], [-4.0, 1.6]],
        z0: CROWN - 0.2,
        z1: 16.4,
    },
    Tier {
        x: 0.8,
        plan: &[[3.2, 0.0], [2.0, 2.2], [-2.0, 2.3], [-2.8, 1.2]],
        z0: 16.2,
        z1: 18.2,
    },
];

/// How far a tier's top is drawn in from its foot.
const SLOPE: f32 = 0.86;

/// The eave over a tier's top: a thin plated deck standing out past its walls, a seam
/// under it, as a pagoda's roofs do.
fn eave(b: &mut MeshBuilder, t: &Tier) {
    if !b.mid() {
        return;
    }
    let plan = mirrored(t.plan);
    b.at(v3(t.x, 0.0, 0.0), |b| {
        b.with_facets(|b| {
            seam(b);
            b.loft_z(
                &plan,
                &[
                    Section::new(t.z1 - 0.32, SLOPE * 0.99),
                    Section::new(t.z1 - 0.2, SLOPE * 1.08),
                ],
            );
            dark_plate(b);
            b.loft_z(
                &plan,
                &[
                    Section::new(t.z1 - 0.2, SLOPE * 1.08),
                    Section::new(t.z1 - 0.02, SLOPE * 1.06),
                ],
            );
        });
    });
}

/// A tier's dress: a course of plates lapped back down each flank, swept into spikes past
/// its tail; a slot of red along its shoulder; a faceted glacis plate over its nose.
fn dress(b: &mut MeshBuilder, t: &Tier) {
    let fine = b.fine();
    let (half, wide_at) = t.shoulder();
    let (nose, tail) = t.ends();
    let h = t.z1 - t.z0;
    let len = (wide_at - tail).max(2.0);
    let count = if fine {
        ((len / 3.6) as usize).clamp(2, 5)
    } else {
        1
    };
    b.mirror_y(|b| {
        dark_plate(b);
        Course {
            count,
            step: len / count as f32,
            len: len / count as f32 + 1.0,
            half: (h * 0.32).min(0.9),
            tip: -0.4,
            thick: 0.22,
            tail: 1.8,
        }
        .lay(
            b,
            &Frame::new(
                v3(wide_at, half * 0.97, t.z0 + h * 0.42),
                v3(-1.0, -0.04, 0.0),
                v3(0.0, 1.0, 0.25),
            ),
        );
        if fine && h > 1.6 {
            let y = half * 0.93 + 0.02;
            let z = t.z1 - h * 0.22;
            inlay(
                b,
                &[
                    v3(wide_at - 0.4, y, z - 0.07),
                    v3(tail + 1.6, y, z - 0.07),
                    v3(tail + 1.6, y, z + 0.07),
                    v3(wide_at - 0.4, y, z + 0.07),
                ],
                Vec3::Y,
                pattern::EMBER_SLOT,
            );
        }
    });
    if b.mid() && nose - wide_at > 2.0 {
        // The glacis over the nose, faceted, lying back up the tier's slope.
        let run = nose - wide_at;
        dark_plate(b);
        ridged(
            b,
            &Frame::new(
                v3(nose - 0.1, 0.0, t.z0 + h * 0.15),
                v3(-run, 0.0, h * 0.6).normalize(),
                v3(h * 0.6, 0.0, run).normalize(),
            ),
            &[
                [0.0, -0.3],
                [0.0, 0.3],
                [run * 0.7, half * 0.55],
                [run * 0.9, 0.0],
                [run * 0.7, -half * 0.55],
            ],
            0.2,
            [run * 0.45, 0.0],
            0.14,
        );
    }
}

/// The horned bridge: a narrow faceted head at `at` (the middle of its foot), `s` its
/// scale, a band of red visor across its raked face, two horns swept up and back off its
/// sides into spikes, a crest plate down its back.
fn head(b: &mut MeshBuilder, at: Vec3, s: f32) {
    let w = 1.5 * s;
    let h = 2.2 * s;
    let foot = [
        v3(2.4, 0.0, 0.0),
        v3(0.6, w / s, 0.0),
        v3(-2.6, 0.9 * w / s, 0.0),
        v3(-2.6, -0.9 * w / s, 0.0),
        v3(0.6, -w / s, 0.0),
    ];
    let crown = [
        v3(1.2, 0.0, h / s),
        v3(0.2, 0.75 * w / s, h / s),
        v3(-2.2, 0.6 * w / s, h / s),
        v3(-2.2, -0.6 * w / s, h / s),
        v3(0.2, -0.75 * w / s, h / s),
    ];
    let at_s = |p: Vec3| at + p * s;
    dark_plate(b);
    b.with_facets(|b| {
        b.loft(
            &[foot.map(at_s).to_vec(), crown.map(at_s).to_vec()],
            true,
            true,
        );
    });
    if b.fine() {
        // The visor across the raked face, both cheeks of it.
        b.mirror_y(|b| {
            let (a, c) = (at_s(foot[0]), at_s(foot[1]));
            let (up_a, up_c) = (at_s(crown[0]), at_s(crown[1]));
            let p = |e0: Vec3, e1: Vec3, t: f32| e0 + (e1 - e0) * t;
            let quad = [
                p(a, up_a, 0.45),
                p(c, up_c, 0.45),
                p(c, up_c, 0.62),
                p(a, up_a, 0.62),
            ];
            let n = (quad[1] - quad[0]).cross(quad[3] - quad[0]).normalize();
            let n = if n.x < 0.0 { -n } else { n };
            inlay(b, &quad, n, pattern::EMBER_SLOT);
        });
    }
    horns(b, at + v3(-0.8 * s, w * 0.7, h * 0.7), 4.2 * s);
    dark_plate(b);
    armour(
        b,
        &Frame::new(
            at + v3(-1.6 * s, 0.0, h - 0.05),
            v3(-1.0, 0.0, -0.45),
            Vec3::Z,
        ),
        &swept(4.2 * s, 0.6 * s, 0.0, 0.5),
        0.2,
    );
}

/// Horns swept up and back off a head or tier at `at` (port; mirrored): a plate raked
/// into a spike, a bronze spar under it.
fn horns(b: &mut MeshBuilder, at: Vec3, len: f32) {
    if !b.mid() {
        return;
    }
    b.mirror_y(|b| {
        dark_plate(b);
        armour(
            b,
            &Frame::new(at, v3(-0.75, 0.3, 0.62), v3(0.0, 1.0, 0.1)),
            &swept(len, 0.5, 0.4, 0.45),
            0.2,
        );
        if b.fine() {
            metal(b);
            b.cylinder_between(
                at + v3(0.1, -0.15, -0.3),
                at + v3(-len * 0.45, len * 0.15, len * 0.35),
                0.12,
                0.1,
                6,
            );
        }
    });
}

/// The ziggurat's buttresses: great swept plates from the lowest tier's shoulders down and
/// out to the deck edges, clear of the beam secondaries.
fn buttresses(b: &mut MeshBuilder, h: &[Station]) {
    if !b.mid() {
        return;
    }
    b.mirror_y(|b| {
        dark_plate(b);
        for x in [-6.0f32, -26.0] {
            let foot = v3(x, 10.4, top_at(h, x, 10.4));
            let head = v3(x + 1.5, 6.0, 8.8);
            let u = foot - head;
            let l = u.length();
            armour(
                b,
                &Frame::new(head, u, v3(0.0, 0.5, 1.0)),
                &[
                    [0.0, -1.2],
                    [0.0, 1.2],
                    [l * 0.8, 1.0],
                    [l + 1.4, 0.0],
                    [l * 0.8, -1.0],
                ],
                0.36,
            );
        }
    });
}
