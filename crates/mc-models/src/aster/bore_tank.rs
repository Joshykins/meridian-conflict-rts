//! Arbalest: tech 3 lightning sniper, in the Fulgur's language. A low, wide wedge on
//! long tracks under a flat turret carrying the Argon Electric Bore: a slim faceted
//! barrel with a dark core in swept strakes and a stepped crown, its blue only thin
//! seams. Two capacitor drums lie on the turret's back, and the engine breathes through
//! two sunk louvres. It stakes itself down to fire as the Trebuchet does: a launcher
//! tube on each corner swings down and fires a spike into the ground, one corner after
//! another (`parts::ground_stakes`, `gpu_consts::stake`).

use glam::Vec3;

use super::parts::*;
use crate::builder::{MeshBuilder, Section};
use crate::material::*;
use crate::{part, pattern, rig};

/// Where the bore ends, as the unit file has it.
const MUZZLE: Vec3 = Vec3::new(10.3, 0.0, 3.3);
/// The barrel's trunnion, where it leaves the breech.
const BREECH: Vec3 = Vec3::new(2.0, 0.0, 3.3);
/// The deck, and the turret race on it: the height the stakes' hinges are placed for.
const DECK: f32 = 1.88;
const TAIL: f32 = -5.2;

pub(super) fn bore_tank(b: &mut MeshBuilder, _tech: u8) {
    hull(b);
    ground_stakes(b);
    turret(b);
}

/// The hull and tracks.
fn hull(b: &mut MeshBuilder) {
    let (rear, front) = (-5.0, 5.3);
    let (inner, outer, track_h) = (2.45, 3.95, 1.25);
    b.set_treads((inner + outer) * 0.5, outer - inner, rear);
    b.set_dust_line(1.5);

    // Long single tracks, low and wide apart.
    b.mirror_y(|b| track(b, rear, front, inner, outer, track_h));

    // Hull: a dark tub between the tracks under a white wedge that runs from a
    // sharp low nose back to a flat deck, overhanging the treads.
    if b.coarse() {
        b.paint(PLATING);
        b.frustum_open(
            v3(0.2, 0.0, 0.7),
            v2(11.0, 7.6),
            v2(8.4, 7.2),
            DECK - 0.7,
            v2(-0.9, 0.0),
        );
    } else {
        b.paint(ACCENT);
        b.extrude_y(
            &[
                [-4.9, 0.35],
                [4.2, 0.35],
                [5.6, 0.95],
                [5.6, 1.15],
                [-4.9, 1.15],
            ],
            -inner,
            inner,
        );
        // Faceted upper hull: a pointed plan drawn in to a narrow deck, so every side slopes.
        b.paint(PLATING);
        b.loft_z(
            &hull_plan(TAIL, 6.8, outer + 0.1, 2.8),
            &[
                Section::new(1.12, 1.0),
                Section::new(1.4, 1.0),
                Section::scaled(DECK, 0.86, 0.72).shifted(-0.7, 0.0),
            ],
        );
    }
    if b.fine() {
        // Skirts over the upper run, a sensor slit and a team chevron on the glacis.
        b.mirror_y(|b| {
            b.paint(PLATING);
            b.block(v3(-4.4, outer + 0.02, 0.62), v3(4.1, outer + 0.14, 1.14));
            b.paint(ACCENT);
            for x in [-1.6, 1.3] {
                b.block(v3(x, outer + 0.1, 0.62), v3(x + 0.12, outer + 0.17, 1.14));
            }
        });
        let glacis = ([6.1, 1.4], [6.1 * 0.86 - 0.7, DECK]);
        on_slope(b, glacis.0, glacis.1, 0.62, |b| {
            glow_strip(b, Vec3::ZERO, v2(0.14, 2.2), GLOW);
        });
        on_slope(b, glacis.0, glacis.1, 0.25, |b| {
            b.paint(PLATING).pattern(pattern::TEAM_BAND);
            b.plate(Vec3::ZERO, v2(0.4, 3.0), 0.05, 0.02);
        });
        // Engine deck: a louvre either side of the tail.
        b.mirror_y(|b| heat_sink(b, v3(-3.9, 1.35, DECK)));
        b.paint(ACCENT);
        b.plate(v3(-3.9, 0.0, DECK), v2(0.9, 0.9), 0.1, 0.04);
    }
    if !b.coarse() {
        // Team flashes on the front fenders, read from above.
        b.mirror_y(|b| team_panel(b, v3(1.6, 2.35, DECK), v2(1.4, 0.8)));
    }
}

/// An engine vent as the Fulgur's are, `at` the middle of its foot: a low faceted housing,
/// the louvre sunk in a dark frame with the fire breathing between the slats.
fn heat_sink(b: &mut MeshBuilder, at: Vec3) {
    let top = at.z + 0.18;
    b.paint(PLATING).pattern(pattern::PLAIN);
    b.at(v3(at.x, at.y, 0.0), |b| {
        b.loft_z(
            &[
                [-0.6, -0.5],
                [0.45, -0.5],
                [0.6, -0.3],
                [0.6, 0.35],
                [0.45, 0.5],
                [-0.6, 0.5],
            ],
            &[
                Section::new(at.z - 0.05, 1.0),
                Section::scaled(top, 0.95, 0.9),
            ],
        );
    });
    b.paint(PLATING_DARK);
    b.plate(v3(at.x, at.y, top), v2(1.0, 0.8), 0.05, 0.02);
    b.paint(ACCENT).pattern(pattern::FURNACE);
    b.plate(v3(at.x, at.y, top + 0.02), v2(0.8, 0.6), 0.04, 0.01);
    b.pattern(pattern::PLAIN);
    if b.fine() {
        b.add_exhaust(v3(at.x, at.y, top + 0.1), v3(-0.2, 0.0, 1.2), 0.45);
    }
}

/// The low turret: a faceted body over a dark race, a mantlet the barrel sits down in,
/// capacitor drums on its back.
fn turret(b: &mut MeshBuilder) {
    b.set_turret_pivot(v3(0.0, 0.0, DECK));
    b.set_recoil(BREECH, MUZZLE, 0.45);
    b.set_arm_pivot(BREECH);
    b.with_part(part::TURRET, |b| {
        if b.coarse() {
            coarse_turret(b);
            return;
        }
        b.paint(ACCENT);
        b.prism(v3(0.0, 0.0, DECK - 0.03), b.sides(10), 2.3, 2.25, 0.2);
        b.paint(PLATING);
        b.loft_z(
            &turret_plan(6.8, 4.3),
            &[
                Section::new(DECK + 0.12, 1.0),
                Section::scaled(3.05, 0.8, 0.7).shifted(-0.4, 0.0),
            ],
        );
        b.paint(PLATING);
        b.extrude_y_chamfered(
            &[
                [-2.4, 2.8],
                [2.3, 2.8],
                [2.6, 3.1],
                [2.25, 3.42],
                [-1.4, 3.4],
                [-2.4, 3.1],
            ],
            0.7,
            0.2,
        );
        team_panel(b, v3(-0.9, -1.2, 3.02), v2(1.6, 0.5));
        pods(b);
        if b.fine() {
            b.paint(GLASS);
            b.chamfered_box(v3(1.3, 1.25, 3.1), v3(0.6, 0.4, 0.3), 0.1);
            antenna(b, v3(-2.3, -1.3, 2.9), 1.2, 0.1);
        }
        b.with_limb(rig::ARM_GUN | rig::RECOIL, |b| {
            lance(b, BREECH, MUZZLE, 0.36)
        });
    });
}

/// Two capacitor pods lying fore and aft on the turret's back as the AEB-2's ride its
/// gun: dark drums in metal bands, one thin seam along each top.
fn pods(b: &mut MeshBuilder) {
    b.mirror_y(|b| {
        let (y, z, r) = (1.35, 3.25, 0.42);
        b.paint(PLATING_DARK);
        b.cylinder_between(v3(-3.3, y, z), v3(-1.0, y, z), r, r, b.sides(8));
        b.paint(METAL);
        for x in [-3.25, -2.5, -1.75, -1.1] {
            b.cylinder_between(v3(x, y, z), v3(x + 0.1, y, z), r * 1.1, r * 1.1, b.sides(8));
        }
        b.paint(PLATING);
        b.block(v3(-3.0, y - 0.3, 2.6), v3(-1.3, y + 0.3, z - r * 0.7));
        if b.fine() {
            b.paint(GLOW);
            b.block(
                v3(-3.1, y - 0.03, z + r * 0.93),
                v3(-1.2, y + 0.03, z + r * 1.0),
            );
            b.paint(METAL);
            b.cylinder_between(v3(-1.0, y, z), v3(-0.3, 0.6, 3.5), 0.07, 0.07, 5);
        }
    });
}

/// The turret far off: a block, the team flash, the barrel.
fn coarse_turret(b: &mut MeshBuilder) {
    b.paint(PLATING);
    b.frustum_open(
        v3(-0.1, 0.0, DECK),
        v2(6.5, 4.3),
        v2(3.8, 1.5),
        4.2 - DECK,
        v2(0.4, 0.0),
    );
    team_panel(b, v3(0.3, 0.0, 4.2), v2(2.4, 1.1));
    b.with_limb(rig::ARM_GUN | rig::RECOIL, |b| {
        b.paint(PLATING_DARK);
        b.beam(BREECH, MUZZLE, v2(0.8, 0.8), v2(0.4, 0.4));
    });
}

/// The Arbalest's long gun, level from `breech` to `muzzle`: the AEB-2's parts drawn out
/// for a sniper. A faceted housing with a raised top plate, a slim dark core carried in a
/// pair of swept side strakes with a seam at each root, a dark band at its middle, and a
/// long stepped crown with ports. `r` scales the section.
fn lance(b: &mut MeshBuilder, breech: Vec3, muzzle: Vec3, r: f32) {
    use super::bolt_rifle::{bore_face, hexagon, ring, seam, vents};
    let length = muzzle.x - breech.x;
    let l = |f: f32| f * length;
    b.at(breech, |b| {
        b.paint(PLATING_DARK).pattern(pattern::PLAIN);
        b.loft(
            &[
                ring(&hexagon(r * 1.9, r * 1.8), l(-0.1)),
                ring(&hexagon(r * 2.1, r * 2.0), l(-0.03)),
            ],
            true,
            true,
        );
        b.paint(PLATING);
        b.loft(
            &[
                ring(&hexagon(r * 2.2, r * 2.1), l(-0.04)),
                ring(&hexagon(r * 2.4, r * 2.3), l(0.04)),
                ring(&hexagon(r * 2.4, r * 2.3), l(0.24)),
                ring(&hexagon(r * 1.4, r * 1.3), l(0.34)),
            ],
            true,
            true,
        );
        b.paint(PLATING_DARK);
        b.loft(
            &[
                ring(
                    &[
                        [-r * 0.7, r * 1.1],
                        [r * 0.7, r * 1.1],
                        [r * 0.55, r * 1.35],
                        [-r * 0.55, r * 1.35],
                    ],
                    l(0.0),
                ),
                ring(
                    &[
                        [-r * 0.6, r * 1.1],
                        [r * 0.6, r * 1.1],
                        [r * 0.45, r * 1.28],
                        [-r * 0.45, r * 1.28],
                    ],
                    l(0.26),
                ),
            ],
            true,
            true,
        );
        // The core, and the band that steadies it.
        let core: Vec<[f32; 2]> = (0..8)
            .map(|k| {
                let a = (k as f32 + 0.5) * std::f32::consts::TAU / 8.0;
                [a.cos() * r * 0.5, a.sin() * r * 0.5]
            })
            .collect();
        b.paint(PLATING_DARK);
        b.extrude_x(&core, l(0.3), l(0.86));
        b.loft(
            &[
                ring(&hexagon(r * 1.25, r * 1.2), l(0.56)),
                ring(&hexagon(r * 1.25, r * 1.2), l(0.61)),
            ],
            true,
            true,
        );
        // The strakes, swept back from the crown to the housing.
        b.mirror_y(|b| {
            b.paint(PLATING);
            b.loft(
                &[
                    ring(
                        &[
                            [r * 0.4, -r * 0.1],
                            [r * 1.05, -r * 0.05],
                            [r * 1.05, r * 0.05],
                            [r * 0.4, r * 0.1],
                        ],
                        l(0.34),
                    ),
                    ring(
                        &[
                            [r * 0.4, -r * 0.1],
                            [r * 0.7, -r * 0.05],
                            [r * 0.7, r * 0.05],
                            [r * 0.4, r * 0.1],
                        ],
                        l(0.84),
                    ),
                ],
                true,
                true,
            );
            if b.fine() {
                b.paint(GLOW);
                b.block(
                    v3(l(0.36), r * 0.45, r * 0.08),
                    v3(l(0.8), r * 0.62, r * 0.12),
                );
                seam(b, l(0.05), l(0.22), r * 1.2, r * 0.25, r);
                vents(b, l(0.06), l(0.025), 6, r * 1.2, (-r * 0.6, -r * 0.2), r);
            }
        });
        // The crown.
        b.paint(ACCENT);
        b.loft(
            &[
                ring(&hexagon(r * 1.2, r * 1.15), l(0.84)),
                ring(&hexagon(r * 1.5, r * 1.45), l(0.87)),
            ],
            true,
            true,
        );
        b.paint(PLATING);
        b.loft(
            &[
                ring(&hexagon(r * 1.6, r * 1.55), l(0.87)),
                ring(&hexagon(r * 1.65, r * 1.6), l(0.96)),
                ring(&hexagon(r * 1.25, r * 1.2), length),
            ],
            true,
            true,
        );
        bore_face(b, length, v2(r * 0.8, r * 0.75));
        if b.fine() {
            b.mirror_y(|b| vents(b, l(0.885), l(0.02), 3, r * 0.82, (-r * 0.25, r * 0.25), r));
        }
    });
}
