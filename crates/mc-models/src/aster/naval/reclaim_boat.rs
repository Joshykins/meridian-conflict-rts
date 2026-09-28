//! Reclaim boat (tech 1): a catamaran carrying a reclaim tower's head, so it strips
//! wrecks as it sails. Two slim hulls under a bridging deck, a low cab forward, and
//! amidships an open lattice tower with a glazed chute up its middle; the head turns
//! on the tower's top.
//!
//! The head is a gun house of its own bound to weapon slot 0 (`with_house`): what
//! turns stands on the slewing ring at `BASE`, and the head inside `with_recoil`
//! pitches about `PIVOT`, from straight down at a wreck on the seabed to steeply up
//! at one on a cliff-top shore. Nothing that does not pitch with it stands inside
//! its sweep (`SWEEP` about the trunnion, across the head's width), so the whole
//! range is clear: `head_sweep_is_clear` in the tests holds the head to it.
//!
//! The head is the land reclaimers' Cradle; its pivot and emitter are the unit
//! file's (`reclaimer.heads`). Unlit but for the red lamp, the Materials
//! glow in the intake (`GLOW_MATERIALS`) and the glazed chute the reclaimed stream
//! falls down (`pattern::MASS_FLOW`).
use super::*;
use crate::aster::reclaimers::cradle_turret;

/// Where the head turns and pitches: its yaw axis and trunnion.
pub(crate) const PIVOT: Vec3 = Vec3::new(-1.0, 0.0, 6.1);
/// The Cradle's scale on the boat (about 1 for a 2 m head).
const CRADLE_SCALE: f32 = 1.1;
/// The intake mouth, this far ahead of the pivot at rest: where the beam leaves.
pub(crate) const REACH: f32 = 1.55 * CRADLE_SCALE;
/// The farthest any pitching part may reach from the trunnion, measured in the plane of
/// pitch (the mouth's rim). The tests hold the heads and the hull to it.
#[cfg(test)]
pub(crate) const SWEEP: f32 = 1.9;
/// The top of the tower, where the slewing ring sits: far enough under the pivot for
/// the Cradle's mouth to pass over its yoke bridge pointing straight down.
const BASE: f32 = PIVOT.z - 2.4;

pub(super) fn cradle(b: &mut MeshBuilder) {
    catamaran(b);
    if b.coarse() {
        coarse_head(b, 1.3);
        return;
    }
    cradle_turret(b, 0, PIVOT, BASE, CRADLE_SCALE);
    // The boat's Cradle stands tall to clear its own yoke pointing straight down, so
    // its cheeks get armour of their own outside: a sloped plate each side, a heavy
    // bearing boss on each over the trunnion, and a dark foot plate tying them down.
    let p = PIVOT;
    b.with_house(0, p, 0.0, |b| {
        b.paint(ACCENT);
        b.chamfered_box(v3(p.x - 0.1, 0.0, BASE + 0.3), v3(1.7, 2.3, 0.16), 0.2);
        b.paint(PLATING);
        let plate = [
            [p.x - 0.8, BASE + 0.3],
            [p.x + 0.6, BASE + 0.3],
            [p.x + 0.6, p.z - 0.9],
            [p.x + 0.25, p.z + 0.3],
            [p.x - 0.45, p.z + 0.3],
            [p.x - 0.8, p.z - 0.2],
        ];
        b.mirror_y(|b| {
            b.at(v3(0.0, 1.03, 0.0), |b| {
                b.extrude_y_chamfered(&plate, 0.09, 0.04)
            });
            b.paint(ACCENT);
            b.cylinder_between(
                p + Vec3::Y * 1.1,
                p + Vec3::Y * 1.24,
                0.38,
                0.32,
                b.sides(10),
            );
            b.paint(PLATING);
        });
        if b.fine() {
            // Stiffening ribs down each plate's face.
            b.paint(ACCENT);
            b.mirror_y(|b| {
                for x in [p.x - 0.5, p.x + 0.25] {
                    b.block(
                        v3(x - 0.05, 1.12, BASE + 0.38),
                        v3(x + 0.05, 1.18, p.z - 0.5),
                    );
                }
            });
        }
    });
}

/// The last level's head: one box on the head's house, `width` across.
fn coarse_head(b: &mut MeshBuilder, width: f32) {
    b.with_house(0, PIVOT, 0.0, |b| {
        b.with_recoil(|b| {
            b.paint(PLATING);
            b.beam(
                PIVOT - Vec3::X * 0.9,
                PIVOT + Vec3::X * REACH,
                v2(width, 1.0),
                v2(width * 0.6, 0.7),
            );
        })
    });
}

// ---- the hull and tower ------------------------------------------------------

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

/// The boat without its head: the demi-hulls, the bridging deck, the lattice tower up
/// to `BASE` with its chute, the cab, bins and deck fittings.
fn catamaran(b: &mut MeshBuilder) {
    b.mirror_y(|b| b.at(v3(0.0, DEMI_Y, 0.0), |b| hull(b, &DEMI, &[0, 4])));
    let (deck0, deck1) = (1.55, 1.95);
    let platform = BASE;
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
        team_panel(b, v3(3.4, 0.0, deck1 + 1.6), v2(1.2, 1.6));
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
