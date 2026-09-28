//! Reclaim boat (tech 1): a small working hull carrying a reclaim tower's head,
//! so it strips wrecks as it sails. The head is a gun house of its own bound to
//! weapon slot 0 (`with_house`): the yoke turns about `PIVOT` by the reclaimer's
//! yaw, and the head inside `with_recoil` pitches about it, from straight down at
//! a wreck on the seabed to steeply up at one on a cliff-top shore. Nothing on
//! the hull stands inside the head's sweep: the turntable sits `SWEEP` below the
//! pivot and the snout's tip reaches no further than that.
//!
//! Three designs are open for the user (`reclaim_boat~a`, `~b`, `~c`); all share
//! the pivot and the emitter (`reclaimer.emitter` in the unit file), so the data
//! does not depend on the pick. Unlit but for the red lamp, the Materials glow at
//! the collector's mouth (`GLOW_MATERIALS`) and the glazed chutes the reclaimed
//! stream falls down (`pattern::MASS_FLOW`).
use std::f32::consts::{FRAC_PI_2, FRAC_PI_8, TAU};

use super::*;

/// Where the head turns and pitches: its yaw axis and trunnion.
const PIVOT: Vec3 = Vec3::new(-1.0, 0.0, 5.3);
/// The emitter, at the collector's mouth, this far ahead of the pivot at rest.
const REACH: f32 = 1.3;
/// How far below the pivot the turntable's top sits: more than anything on the head
/// reaches from the pivot, so it pitches straight down and up clear of its base.
const SWEEP: f32 = 1.4;

/// The turntable the yoke turns on, a dark drum on a gunmetal ring.
fn turntable(b: &mut MeshBuilder, radius: f32) {
    let top = PIVOT.z - SWEEP;
    b.paint(METAL);
    b.prism(
        v3(PIVOT.x, 0.0, top - 0.32),
        b.sides(12),
        radius,
        radius,
        0.12,
    );
    b.paint(ACCENT);
    b.prism(
        v3(PIVOT.x, 0.0, top - 0.2),
        b.sides(12),
        radius * 0.94,
        radius * 0.86,
        0.2,
    );
}

/// The collector: a dark cone flaring to the mouth at `REACH`, a thin lip of the
/// reclaim glow at the mouth. Along +x from `from` (pivot frame, level).
fn collector(b: &mut MeshBuilder, from: f32, r0: f32, r1: f32) {
    let at = |x: f32| PIVOT + Vec3::X * x;
    b.paint(METAL);
    b.cylinder_between(at(from), at(REACH - 0.12), r0, r1, b.sides(10));
    b.paint(GLOW_MATERIALS);
    b.cylinder_between(
        at(REACH - 0.12),
        at(REACH),
        r1 * 1.04,
        r1 * 0.9,
        b.sides(10),
    );
    if b.fine() {
        // The dark throat inside the lip, so the mouth reads as an opening.
        b.paint(ACCENT);
        b.cylinder_between(at(REACH - 0.02), at(REACH + 0.01), r1 * 0.66, r1 * 0.66, 10);
    }
}

// ---- A: the tug with a Scavenger's head -------------------------------------

const TUG: [Station; 6] = [
    station(-7.6, -0.5, [-0.3, 2.0], [0.9, 2.35], [1.6, 2.4]),
    station(-4.0, -1.0, [-0.55, 2.2], [0.85, 2.55], [1.65, 2.6]),
    station(0.5, -1.2, [-0.6, 2.1], [0.9, 2.5], [1.75, 2.55]),
    station(4.5, -1.0, [-0.4, 1.5], [1.1, 2.0], [2.0, 2.1]),
    station(7.0, -0.4, [0.1, 0.6], [1.5, 1.1], [2.35, 1.2]),
    station(8.2, 0.7, [0.9, 0.0], [1.7, 0.0], [2.6, 0.0]),
];

/// A: a stubby workboat, wheelhouse forward, and amidships a Scavenger tower in
/// small: an octagonal bunker, a yoke, and a boxy processor head with the collector
/// out front. Mass hoppers on the quarters.
pub(super) fn build_a(b: &mut MeshBuilder) {
    hull(b, &TUG, &[0, 3, 5]);
    let deck = deck_at(&TUG, PIVOT.x).0;
    let top = PIVOT.z - SWEEP - 0.32;
    if b.coarse() {
        b.paint(ACCENT);
        b.frustum_open(
            v3(PIVOT.x, 0.0, deck),
            v2(3.2, 3.2),
            v2(2.2, 2.2),
            top - deck,
            v2(0.0, 0.0),
        );
        b.paint(PLATING);
        b.frustum_open(
            v3(4.0, 0.0, 2.0),
            v2(2.6, 2.8),
            v2(2.2, 2.4),
            2.3,
            v2(0.0, 0.0),
        );
        b.with_house(0, PIVOT, 0.0, |b| {
            b.with_recoil(|b| {
                b.paint(ACCENT);
                b.cuboid(PIVOT - Vec3::X * 0.1, v3(1.8, 1.5, 1.1));
            })
        });
        return;
    }

    // The bunker: black octagon tapering to the turntable, grey band, white plates.
    b.paint(ACCENT);
    b.at(v3(PIVOT.x, 0.0, 0.0), |b| {
        b.loft_z(
            &ngon(8, 1.75),
            &[
                Section::new(deck - 0.2, 1.0),
                Section::new(deck + 0.5, 0.98),
                Section::new(top, 0.72),
            ],
        );
        b.paint(PLATING_DARK);
        b.prism(v3(0.0, 0.0, deck + 0.35), 8, 1.8, 1.8, 0.24);
    });
    let apothem = 1.75 * FRAC_PI_8.cos();
    b.at(v3(PIVOT.x, 0.0, 0.0), |b| {
        if !b.fine() {
            return;
        }
        b.radial(4, |b| {
            on_slope(
                b,
                [apothem * 0.98, deck + 0.5],
                [apothem * 0.72, top],
                0.5,
                |b| {
                    b.paint(PLATING);
                    b.plate(Vec3::ZERO, v2(0.62, 0.7), 0.07, 0.02);
                },
            );
        })
    });
    turntable(b, 1.2);

    b.with_house(0, PIVOT, 0.0, |b| {
        // Yoke: two dark cheeks off the turntable, gunmetal trunnion hubs.
        b.paint(ACCENT);
        b.mirror_y(|b| {
            b.at(v3(0.0, 1.08, 0.0), |b| {
                b.extrude_y_chamfered(
                    &[
                        [PIVOT.x - 0.75, PIVOT.z - SWEEP],
                        [PIVOT.x + 0.75, PIVOT.z - SWEEP],
                        [PIVOT.x + 0.45, PIVOT.z + 0.1],
                        [PIVOT.x + 0.2, PIVOT.z + 0.4],
                        [PIVOT.x - 0.3, PIVOT.z + 0.4],
                        [PIVOT.x - 0.55, PIVOT.z + 0.1],
                    ],
                    0.1,
                    0.03,
                )
            });
        });
        b.paint(METAL);
        b.cylinder_between(
            PIVOT - Vec3::Y * 1.25,
            PIVOT + Vec3::Y * 1.25,
            0.28,
            0.28,
            b.sides(8),
        );
        b.with_recoil(|b| {
            // The processor: a black housing, a white lid, a grey drum behind.
            b.paint(ACCENT);
            b.chamfered_box(PIVOT + v3(-0.05, 0.0, 0.0), v3(1.9, 1.6, 1.15), 0.22);
            b.paint(PLATING);
            b.plate(PIVOT + v3(-0.1, 0.0, 0.575), v2(1.5, 1.3), 0.06, 0.02);
            b.paint(PLATING_DARK);
            b.cylinder_between(
                PIVOT + v3(-0.75, -0.62, -0.1),
                PIVOT + v3(-0.75, 0.62, -0.1),
                0.44,
                0.44,
                b.sides(8),
            );
            collector(b, 0.7, 0.34, 0.5);
            if b.fine() {
                // Claw prongs round the mouth and a sight box on the lid.
                b.paint(ACCENT);
                for k in 0..3 {
                    let ang = k as f32 * TAU / 3.0 + FRAC_PI_2;
                    let r = v3(0.0, ang.cos(), ang.sin());
                    b.beam(
                        PIVOT + Vec3::X * 0.95 + r * 0.42,
                        PIVOT + Vec3::X * 1.28 + r * 0.56,
                        v2(0.09, 0.09),
                        v2(0.05, 0.05),
                    );
                }
                b.block(PIVOT + v3(0.1, 0.35, 0.6), PIVOT + v3(0.4, 0.6, 0.76));
                b.paint(GLASS);
                b.block(PIVOT + v3(0.4, 0.39, 0.63), PIVOT + v3(0.42, 0.56, 0.73));
            }
        });
    });

    tug_house(b, 3.8);

    // Mass hoppers on the quarters: black bins, grey bands, white lids.
    b.mirror_y(|b| {
        let x = -5.2;
        let z = deck_at(&TUG, x).0;
        b.paint(ACCENT);
        b.at(v3(x, 1.25, 0.0), |b| {
            b.loft_z(
                &ngon(b.sides(8), 0.85),
                &[
                    Section::new(z - 0.1, 1.0),
                    Section::new(z + 0.9, 1.0),
                    Section::new(z + 1.2, 0.7),
                ],
            );
            // A glazed slot in the outboard wall: the reclaimed stream falling in.
            b.paint(ACCENT).pattern(pattern::MASS_FLOW);
            b.block(v3(-0.22, 0.74, z + 0.15), v3(0.22, 0.86, z + 0.85));
            if b.fine() {
                b.paint(PLATING_DARK);
                b.prism(v3(0.0, 0.0, z - 0.05), 8, 0.9, 0.9, 0.22);
                b.paint(PLATING);
                b.prism(v3(0.0, 0.0, z + 1.2), 8, 0.62, 0.5, 0.1);
            }
        });
    });
    // Glazed feed chutes raked down from the bunker's shoulders into the hoppers.
    b.paint(ACCENT).pattern(pattern::MASS_FLOW);
    b.mirror_y(|b| {
        b.beam(
            v3(PIVOT.x - 1.1, 0.7, top - 0.1),
            v3(-4.7, 1.2, deck + 1.35),
            v2(0.32, 0.36),
            v2(0.3, 0.32),
        );
    });

    walkway(b, &TUG, -7.3, -2.6, 0.22);
    walkway(b, &TUG, 5.3, 7.8, 0.2);
    rub_rail(b, &TUG, -7.6, 8.0, 0.18);
    if !b.fine() {
        return;
    }
    tug_fittings(b);
    rails(b, &TUG, -7.2, -3.0, 0.6, 0.14);
    rails(b, &TUG, 5.4, 7.7, 0.55, 0.12);
}

/// The tug's wheelhouse at `x`: dark sill, grey house, screen all round, lid, the red
/// lamp on a short mast and the exhaust.
fn tug_house(b: &mut MeshBuilder, x: f32) {
    let plan = chamfered_rect(v2(1.3, 1.45), 0.4);
    b.at(v3(x, 0.0, 0.0), |b| {
        b.paint(ACCENT);
        b.loft_z(
            &chamfered_rect(v2(1.36, 1.51), 0.42),
            &[Section::new(1.9, 1.0), Section::new(2.2, 1.0)],
        );
        b.paint(PLATING);
        b.loft_z(&plan, &[Section::new(2.15, 1.0), Section::new(3.3, 0.97)]);
        b.paint(GLASS);
        b.loft_z(
            &plan,
            &[
                Section::new(3.3, 0.97),
                Section::scaled(3.9, 0.88, 0.9).shifted(-0.12, 0.0),
            ],
        );
        b.paint(PLATING);
        b.loft_z(
            &plan,
            &[
                Section::scaled(3.9, 0.9, 0.92).shifted(-0.1, 0.0),
                Section::scaled(4.12, 0.86, 0.88).shifted(-0.14, 0.0),
            ],
        );
    });
    team_panel(b, v3(x - 0.15, 0.0, 4.12), v2(1.3, 1.3));
    b.paint(ACCENT);
    b.prism(v3(x - 0.7, 0.0, 4.12), b.sides(6), 0.1, 0.07, 0.8);
    beacon(b, v3(x - 0.7, 0.0, 4.92));
    if b.fine() {
        b.paint(PLATING);
        b.cylinder_between(
            v3(x - 1.2, -1.2, 3.0),
            v3(x - 1.9, -1.3, 4.0),
            0.15,
            0.13,
            8,
        );
        b.paint(ACCENT);
        b.block(v3(x + 1.1, -0.9, 3.25), v3(x + 1.24, 0.9, 3.3));
        whip(b, v3(x - 0.9, 0.9, 4.12), 1.0, 0.08);
    }
}

/// Fenders, bollards, a foredeck hatch and the anchor: fine detail on the tug hull.
fn tug_fittings(b: &mut MeshBuilder) {
    b.paint(TREAD);
    b.mirror_y(|b| {
        for x in [-5.6, -2.4, 1.4] {
            let (z, half) = deck_at(&TUG, x);
            b.cylinder_between(
                v3(x, half + 0.1, z - 0.15),
                v3(x, half + 0.1, z - 0.7),
                0.16,
                0.16,
                6,
            );
        }
    });
    for x in [6.4, -6.9] {
        let (z, half) = deck_at(&TUG, x);
        b.mirror_y(|b| bollard(b, v3(x, half - 0.32, z + 0.05), 0.3));
    }
    b.paint(PLATING);
    b.plate(
        v3(6.2, 0.0, deck_at(&TUG, 6.2).0 + 0.05),
        v2(0.8, 0.8),
        0.06,
        0.03,
    );
    b.paint(METAL);
    b.cylinder_between(v3(7.6, 0.0, 2.4), v3(8.0, 0.0, 2.2), 0.06, 0.06, 4);
}

// ---- B: the lighthouse -----------------------------------------------------

const BARGE: [Station; 6] = [
    station(-7.8, -0.4, [-0.2, 2.5], [0.7, 2.85], [1.45, 2.9]),
    station(-4.5, -0.9, [-0.5, 2.7], [0.7, 3.0], [1.5, 3.05]),
    station(0.5, -1.05, [-0.55, 2.65], [0.75, 3.0], [1.55, 3.05]),
    station(4.5, -0.9, [-0.35, 2.05], [0.95, 2.55], [1.8, 2.6]),
    station(7.0, -0.35, [0.15, 0.9], [1.3, 1.5], [2.15, 1.55]),
    station(8.3, 0.6, [0.9, 0.0], [1.6, 0.0], [2.4, 0.0]),
];

/// B: the tower is the superstructure. A low, beamy work hull with one round
/// pedestal rising from it, the cab's glass band wrapped round its waist, and on
/// top a searchlight drum turning in a fork: the head pitches inside its own round
/// outline. Open decks fore and aft: a hopper hatch aft, a davit and bins forward.
pub(super) fn build_b(b: &mut MeshBuilder) {
    hull(b, &BARGE, &[0, 3, 5]);
    let deck = deck_at(&BARGE, PIVOT.x).0;
    let top = PIVOT.z - SWEEP - 0.32;
    if b.coarse() {
        b.paint(PLATING);
        b.prism(v3(PIVOT.x, 0.0, deck - 0.1), 4, 2.3, 1.5, top - deck + 0.1);
        b.with_house(0, PIVOT, 0.0, |b| {
            b.with_recoil(|b| {
                b.paint(ACCENT);
                b.cuboid(PIVOT, v3(1.7, 1.6, 1.7));
            })
        });
        return;
    }

    // The pedestal: a dark skirt, the grey column, the cab's glass band, a grey cap
    // narrowing to the turntable. Round, so it shades smooth.
    let ring = ngon(b.sides(16), 1.0);
    b.at(v3(PIVOT.x, 0.0, 0.0), |b| {
        b.paint(ACCENT);
        b.loft_z(
            &ring,
            &[
                Section::new(deck - 0.2, 2.25),
                Section::new(deck + 0.25, 2.2),
                Section::new(deck + 0.4, 1.95),
            ],
        );
        b.paint(PLATING);
        b.loft_z(
            &ring,
            &[
                Section::new(deck + 0.38, 1.9),
                Section::new(2.95, 1.75),
                Section::new(3.05, 1.8),
            ],
        );
        b.paint(GLASS);
        b.loft_z(&ring, &[Section::new(3.05, 1.8), Section::new(3.5, 1.72)]);
        b.paint(PLATING);
        b.loft_z(
            &ring,
            &[
                Section::new(3.5, 1.78),
                Section::new(3.62, 1.78),
                Section::new(top, 1.3),
            ],
        );
    });
    turntable(b, 1.15);
    // Frame bars across the glass: the cab's mullions.
    if b.fine() {
        b.paint(ACCENT);
        let n = 8;
        for k in 0..n {
            let a = (k as f32 + 0.5) * TAU / n as f32;
            let d = v3(a.cos(), a.sin(), 0.0);
            b.beam(
                v3(PIVOT.x, 0.0, 3.03) + d * 1.79,
                v3(PIVOT.x, 0.0, 3.52) + d * 1.73,
                v2(0.1, 0.06),
                v2(0.1, 0.06),
            );
        }
    }

    b.with_house(0, PIVOT, 0.0, |b| {
        // The fork: two arms off the turntable up to the drum's trunnions.
        b.paint(ACCENT);
        b.mirror_y(|b| {
            b.beam(
                v3(PIVOT.x, 0.9, PIVOT.z - SWEEP),
                v3(PIVOT.x, 1.1, PIVOT.z),
                v2(0.62, 0.2),
                v2(0.42, 0.16),
            );
            b.paint(METAL);
            b.cylinder_between(
                PIVOT + Vec3::Y * 0.95,
                PIVOT + Vec3::Y * 1.26,
                0.3,
                0.3,
                b.sides(8),
            );
            b.paint(ACCENT);
        });
        b.with_recoil(|b| {
            // The drum: grey barrel on its trunnion axis, dark end caps, and the
            // collector's hood and mouth out of its face.
            b.paint(PLATING);
            b.cylinder_between(
                PIVOT - Vec3::Y * 0.84,
                PIVOT + Vec3::Y * 0.84,
                0.98,
                0.98,
                b.sides(14),
            );
            b.paint(ACCENT);
            b.mirror_y(|b| {
                if !b.fine() {
                    return;
                }
                b.cylinder_between(
                    PIVOT + Vec3::Y * 0.7,
                    PIVOT + Vec3::Y * 0.92,
                    1.02,
                    0.92,
                    b.sides(14),
                );
            });
            b.paint(ACCENT);
            b.chamfered_box(PIVOT + v3(0.92, 0.0, 0.0), v3(0.4, 1.1, 1.1), 0.2);
            collector(b, 1.0, 0.4, 0.48);
            if b.fine() {
                // A dark band round the drum, and the rangefinder slot over the mouth.
                b.paint(PLATING_DARK);
                b.cylinder_between(PIVOT - Vec3::Y * 0.14, PIVOT + Vec3::Y * 0.14, 1.0, 1.0, 14);
                b.paint(METAL);
                b.block(PIVOT + v3(0.3, -0.22, 0.8), PIVOT + v3(0.72, 0.22, 0.98));
                b.paint(GLASS);
                b.block(PIVOT + v3(0.72, -0.16, 0.83), PIVOT + v3(0.74, 0.16, 0.95));
            }
        });
    });

    // A red lamp on a whip off the pedestal's cap, outside the head's sweep; down its back the glazed chute the
    // reclaimed stream falls through, raked into the hopper hatch aft.
    b.paint(ACCENT);
    b.prism(v3(PIVOT.x - 0.25, 1.62, 3.6), 6, 0.1, 0.07, 1.1);
    beacon(b, v3(PIVOT.x - 0.25, 1.62, 4.7));
    let az = deck_at(&BARGE, -5.0).0;
    let bend = v3(PIVOT.x - 2.12, 0.0, deck + 0.95);
    b.paint(ACCENT).pattern(pattern::MASS_FLOW);
    b.beam(
        v3(PIVOT.x - 1.62, 0.0, 3.62),
        bend,
        v2(0.42, 0.34),
        v2(0.42, 0.34),
    );
    b.beam(
        bend,
        v3(-3.55, 0.0, az + 0.5),
        v2(0.42, 0.34),
        v2(0.42, 0.34),
    );

    // Aft: the hopper hatch, a gunmetal coaming round a white lid; forward the bins.
    b.paint(ACCENT);
    b.block(v3(-6.6, -1.8, az - 0.05), v3(-3.4, 1.8, az + 0.45));
    b.paint(PLATING);
    b.plate(v3(-5.0, 0.0, az + 0.45), v2(2.9, 3.2), 0.08, 0.03);
    team_panel(b, v3(-5.0, 0.0, az + 0.53), v2(1.2, 1.6));
    b.mirror_y(|b| {
        let fz = deck_at(&BARGE, 3.2).0;
        b.paint(ACCENT);
        b.chamfered_box(v3(3.2, 1.3, fz + 0.4), v3(1.6, 1.1, 0.8), 0.12);
        if b.fine() {
            b.paint(PLATING);
            b.plate(v3(3.2, 1.3, fz + 0.8), v2(1.4, 0.9), 0.06, 0.02);
        }
    });
    walkway(b, &BARGE, 1.4, 7.9, 0.25);
    walkway(b, &BARGE, -7.5, -6.8, 0.25);
    rub_rail(b, &BARGE, -7.8, 8.1, 0.2);
    if !b.fine() {
        return;
    }
    // Davit on the port bow, fenders, bollards, rails.
    let dz = deck_at(&BARGE, 5.4).0;
    b.paint(ACCENT);
    b.cylinder_between(v3(5.4, -1.9, dz), v3(5.4, -1.9, dz + 1.5), 0.12, 0.1, 8);
    b.paint(PLATING);
    b.beam(
        v3(5.4, -1.9, dz + 1.4),
        v3(6.4, -1.9, dz + 1.75),
        v2(0.16, 0.18),
        v2(0.1, 0.1),
    );
    b.paint(TREAD);
    b.mirror_y(|b| {
        for x in [-5.8, -2.2, 1.8] {
            let (z, half) = deck_at(&BARGE, x);
            b.cylinder_between(
                v3(x, half + 0.1, z - 0.15),
                v3(x, half + 0.1, z - 0.7),
                0.17,
                0.17,
                6,
            );
        }
    });
    for x in [6.6, -7.2] {
        let (z, half) = deck_at(&BARGE, x);
        b.mirror_y(|b| bollard(b, v3(x, half - 0.35, z + 0.05), 0.3));
    }
    rails(b, &BARGE, 4.4, 7.9, 0.55, 0.14);
    rails(b, &BARGE, -7.5, -3.0, 0.6, 0.14);
}

// ---- C: the catamaran ------------------------------------------------------

/// One of the catamaran's slim demi-hulls, on its own centreline.
const DEMI: [Station; 5] = [
    station(-7.4, -0.2, [-0.3, 0.55], [0.6, 0.78], [1.5, 0.8]),
    station(-3.0, -0.9, [-0.55, 0.62], [0.6, 0.85], [1.6, 0.86]),
    station(2.5, -0.95, [-0.55, 0.6], [0.6, 0.84], [1.7, 0.85]),
    station(6.4, -0.5, [-0.1, 0.35], [0.9, 0.55], [1.95, 0.56]),
    station(8.2, 0.4, [0.7, 0.0], [1.4, 0.0], [2.2, 0.0]),
];
/// How far out from the centreline each demi-hull runs.
const DEMI_Y: f32 = 1.85;

/// C: a catamaran. Two slim hulls under a bridging deck, a low cab forward, and
/// amidships an open lattice tower with a gimbal ring at its top: the head is a
/// ball turning in the ring, the collector's snout out of it, so it points
/// anywhere, straight down between the hulls included.
pub(super) fn build_c(b: &mut MeshBuilder) {
    b.mirror_y(|b| b.at(v3(0.0, DEMI_Y, 0.0), |b| hull(b, &DEMI, &[0, 4])));
    let (deck0, deck1) = (1.55, 1.95);
    let platform = PIVOT.z - SWEEP - 0.32;
    // The bridging deck: a dark underside, a grey box, walkway on top.
    if b.coarse() {
        b.paint(PLATING);
        b.cuboid_open(
            v3(-0.65, 0.0, (deck0 + deck1) * 0.5),
            v3(12.5, 5.1, deck1 - deck0),
        );
        b.paint(PLATING);
        b.frustum_open(
            v3(3.4, 0.0, deck1),
            v2(2.2, 3.0),
            v2(1.8, 2.6),
            1.6,
            v2(0.0, 0.0),
        );
        b.paint(ACCENT);
        b.frustum_open(
            v3(PIVOT.x, 0.0, deck1),
            v2(2.2, 2.2),
            v2(1.2, 1.2),
            platform - deck1,
            v2(0.0, 0.0),
        );
        b.with_house(0, PIVOT, 0.0, |b| {
            b.with_recoil(|b| {
                b.paint(PLATING);
                b.cuboid(PIVOT, v3(1.6, 1.6, 1.6));
            })
        });
        return;
    }

    b.paint(ACCENT);
    b.block(v3(-6.9, -DEMI_Y, deck0), v3(5.6, DEMI_Y, deck0 + 0.2));
    b.paint(PLATING);
    b.block(v3(-6.9, -2.55, deck0 + 0.2), v3(5.6, 2.55, deck1));

    // The lattice tower: four dark legs tapering up to a platform, rungs and bracing.
    let (foot, head) = (1.15, 0.72);
    let leg = |sx: f32, sy: f32, z: f32| {
        let h = foot + (head - foot) * (z - deck1) / (platform - deck1);
        v3(PIVOT.x + sx * h, sy * h, z)
    };
    b.paint(ACCENT);
    for (sx, sy) in [(-1.0, -1.0), (-1.0, 1.0), (1.0, 1.0), (1.0, -1.0)] {
        b.beam(
            leg(sx, sy, deck1),
            leg(sx, sy, platform),
            v2(0.2, 0.2),
            v2(0.16, 0.16),
        );
    }
    b.paint(PLATING);
    let rungs: &[f32] = if b.fine() { &[2.7, 3.35] } else { &[3.0] };
    for &z in rungs {
        for ((ax, ay), (ex, ey)) in [
            ((-1.0, -1.0), (1.0, -1.0)),
            ((-1.0, 1.0), (1.0, 1.0)),
            ((-1.0, -1.0), (-1.0, 1.0)),
            ((1.0, -1.0), (1.0, 1.0)),
        ] {
            b.beam(
                leg(ax, ay, z),
                leg(ex, ey, z),
                v2(0.09, 0.09),
                v2(0.09, 0.09),
            );
        }
    }
    if b.fine() {
        // Cross-bracing in each face.
        b.paint(ACCENT);
        for (a, e) in [
            ((-1.0, -1.0), (1.0, -1.0)),
            ((-1.0, 1.0), (1.0, 1.0)),
            ((-1.0, -1.0), (-1.0, 1.0)),
            ((1.0, -1.0), (1.0, 1.0)),
        ] {
            b.beam(
                leg(a.0, a.1, 2.1),
                leg(e.0, e.1, 2.7),
                v2(0.06, 0.06),
                v2(0.06, 0.06),
            );
            b.beam(
                leg(e.0, e.1, 2.7),
                leg(a.0, a.1, 3.35),
                v2(0.06, 0.06),
                v2(0.06, 0.06),
            );
        }
    }
    // The glazed chute down the middle: the reclaimed stream falls from the head to the bins.
    b.paint(ACCENT).pattern(pattern::MASS_FLOW);
    b.prism(
        v3(PIVOT.x, 0.0, deck1),
        b.sides(8),
        0.34,
        0.3,
        platform - deck1,
    );
    b.paint(PLATING);
    b.chamfered_box(v3(PIVOT.x, 0.0, platform - 0.1), v3(1.9, 1.9, 0.2), 0.3);
    turntable(b, 0.95);

    b.with_house(0, PIVOT, 0.0, |b| {
        // Two posts up from the turntable to the gimbal arcs.
        b.paint(ACCENT);
        b.mirror_y(|b| {
            b.beam(
                v3(PIVOT.x, 0.85, PIVOT.z - SWEEP),
                v3(PIVOT.x, 1.12, PIVOT.z - 0.3),
                v2(0.5, 0.16),
                v2(0.36, 0.14),
            );
        });
        // The gimbal: an arc each side round the ball, about the trunnion axis, well
        // clear of the snout's sweep between them.
        let n = if b.fine() { 8 } else { 3 };
        b.paint(PLATING_DARK);
        b.mirror_y(|b| {
            for k in 0..n {
                let arc = |k: usize| {
                    let a = -1.0 + 2.0 * k as f32 / n as f32;
                    PIVOT + v3(0.0, 1.12 * a.cos(), 1.12 * a.sin())
                };
                b.beam(arc(k), arc(k + 1), v2(0.3, 0.14), v2(0.3, 0.14));
            }
        });
        b.with_recoil(|b| {
            // The ball: grey, a dark band round it on the trunnion axis, trunnion
            // hubs in the ring, and the collector out of its face.
            b.paint(PLATING);
            b.spheroid(PIVOT, Vec3::splat(1.0), b.sides(14), b.sides(8));
            b.paint(ACCENT);
            b.cylinder_between(
                PIVOT - Vec3::Y * 0.2,
                PIVOT + Vec3::Y * 0.2,
                1.02,
                1.02,
                b.sides(14),
            );
            b.paint(METAL);
            b.cylinder_between(PIVOT - Vec3::Y * 1.2, PIVOT + Vec3::Y * 1.2, 0.17, 0.17, 6);
            collector(b, 0.85, 0.36, 0.44);
            if b.fine() {
                b.paint(ACCENT);
                b.block(PIVOT + v3(-0.2, -0.2, 0.9), PIVOT + v3(0.3, 0.2, 1.1));
                b.paint(GLASS);
                b.block(PIVOT + v3(0.3, -0.14, 0.94), PIVOT + v3(0.32, 0.14, 1.06));
            }
        });
    });

    // The cab forward on the bridging deck: low, wide, glass all round.
    let plan = chamfered_rect(v2(1.05, 1.5), 0.36);
    b.at(v3(3.5, 0.0, 0.0), |b| {
        b.paint(PLATING);
        b.loft_z(&plan, &[Section::new(deck1, 1.0), Section::new(2.8, 0.98)]);
        b.paint(GLASS);
        b.loft_z(
            &plan,
            &[
                Section::new(2.8, 0.98),
                Section::scaled(3.3, 0.88, 0.92).shifted(-0.1, 0.0),
            ],
        );
        b.paint(PLATING);
        b.loft_z(
            &plan,
            &[
                Section::scaled(3.3, 0.9, 0.94).shifted(-0.08, 0.0),
                Section::scaled(3.5, 0.84, 0.88).shifted(-0.12, 0.0),
            ],
        );
    });
    team_panel(b, v3(3.38, 0.0, 3.5), v2(1.1, 1.4));
    b.paint(ACCENT);
    b.prism(v3(2.8, 0.0, 3.5), 6, 0.1, 0.07, 0.9);
    beacon(b, v3(2.8, 0.0, 4.4));

    // Mass bins aft between the hulls, a hose reel, walkway strips on the deck.
    b.mirror_y(|b| {
        b.paint(ACCENT);
        b.chamfered_box(v3(-5.0, 1.2, deck1 + 0.5), v3(2.4, 1.3, 1.0), 0.15);
        if b.fine() {
            b.paint(PLATING);
            b.plate(v3(-5.0, 1.2, deck1 + 1.0), v2(2.1, 1.1), 0.06, 0.02);
        }
    });
    b.paint(PLATING).pattern(pattern::WALKWAY);
    b.block(v3(-6.7, -0.45, deck1), v3(2.4, 0.45, deck1 + 0.04));
    // The demi-hulls' bows run past the bridging deck: a dark rubbing strake on each.
    b.mirror_y(|b| {
        b.at(v3(0.0, DEMI_Y, 0.0), |b| {
            b.paint(ACCENT);
            b.block(v3(5.6, -0.72, 1.62), v3(7.4, 0.72, 1.72));
        })
    });
    if !b.fine() {
        return;
    }
    b.paint(PLATING_DARK);
    b.cylinder_between(v3(-2.6, -1.3, 2.45), v3(-2.6, -0.5, 2.45), 0.4, 0.4, 8);
    b.paint(TREAD);
    b.mirror_y(|b| {
        for x in [-5.0, -1.0, 3.0] {
            b.cylinder_between(
                v3(x, DEMI_Y + 0.9, 1.45),
                v3(x, DEMI_Y + 0.9, 0.9),
                0.15,
                0.15,
                6,
            );
        }
    });
    b.mirror_y(|b| {
        for x in [-6.4, 5.0] {
            bollard(b, v3(x, 2.25, deck1), 0.28);
        }
    });
    b.paint(METAL);
    b.mirror_y(|b| {
        for (x0, x1) in [(-6.8, -3.4), (-3.4, 0.2)] {
            for x in [x0, x1] {
                b.cylinder_between(
                    v3(x, 2.45, deck1),
                    v3(x, 2.45, deck1 + 0.6),
                    0.035,
                    0.035,
                    4,
                );
            }
            b.beam(
                v3(x0, 2.45, deck1 + 0.6),
                v3(x1, 2.45, deck1 + 0.6),
                v2(0.05, 0.05),
                v2(0.05, 0.05),
            );
        }
    });
}
