//! Design B (`space_elevator~b`): three armoured A-frame pylons over a fortified well.
//! Each pylon is two legs leaning in over a low plinth, braced across, a back stay
//! from the plinth's tail up to a crown over the legs' heads, and the hinge carried on
//! trunnion blocks inside the legs. The jaw is one straight box girder driven by twin
//! rams along its back. Round the well a sloped armoured bastion fills each tower's
//! side, gate piers at the ways out.
//!
//! Tower frame: x out along the tower's bearing, y across, z up.

use glam::{Vec2, Vec3};

use super::*;

/// The plinth: its middle along x, half size at its foot, top.
const PLINTH_X: f32 = 120.0;
const PLINTH_HALF: Vec2 = Vec2::new(32.0, 54.0);
const PLINTH_TOP: f32 = 22.0;
/// A leg (+y): its middle and half size (x, y) at the foot, the knee and the head.
const LEG: [(Vec3, Vec2); 3] = [
    (
        Vec3::new(124.0, 40.0, PLINTH_TOP - 0.5),
        Vec2::new(20.0, 12.0),
    ),
    (Vec3::new(112.0, 32.0, 92.0), Vec2::new(16.0, 10.0)),
    (Vec3::new(106.0, 27.0, 150.0), Vec2::new(13.0, 8.5)),
];
/// The back stay: foot and head on the tower's middle line.
const STAY: (Vec3, Vec3) = (
    Vec3::new(148.0, 0.0, PLINTH_TOP - 0.5),
    Vec3::new(134.0, 0.0, 150.0),
);
/// The crown over the legs' heads and the stay's.
const CROWN_X: (f32, f32) = (122.0, 148.0);
const CROWN_Z: (f32, f32) = (144.0, 158.0);

/// The jaw: half its width, and the back of its head.
const JAW_HALF: f32 = 15.0;
const HEAD_BACK: f32 = 34.0;
const HEAD_Z: f32 = GRIP_Z + 1.0;
const TIP: Vec3 = Vec3::new(HEAD_BACK + 4.0, 0.0, HEAD_Z + 4.0);
const _: () = assert!(LEG[2].0.y - LEG[2].1.y >= JAW_HALF + 1.5);

/// The bastion round the well: its inner and outer radius and its top.
const BASTION_IN: f32 = WELL_R + 1.0;
const BASTION_OUT: f32 = 92.0;
const BASTION_TOP: f32 = 14.0;

pub(super) fn tower(b: &mut MeshBuilder) {
    plinth(b);
    b.mirror_y(leg);
    stay(b);
    crown(b);
    let sides = b.sides(16);
    b.paint(METAL).pattern(pattern::PLAIN);
    b.cylinder_between(
        HINGE - Vec3::Y * 34.0,
        HINGE + Vec3::Y * 34.0,
        6.5,
        6.5,
        sides,
    );
}

/// The low plinth under the legs, battered, its foot band, doors toward the ways out.
fn plinth(b: &mut MeshBuilder) {
    let plan = chamfered_rect(PLINTH_HALF, 16.0);
    let at = |z: f32, s: f32| Section::new(z, s).shifted(PLINTH_X, 0.0);
    b.paint(ACCENT);
    b.loft_z(&plan, &[at(0.0, 1.03), at(3.5, 1.02)]);
    b.paint(PLATING);
    b.loft_z(
        &plan,
        &[
            at(3.0, 1.0),
            at(PLINTH_TOP - 2.0, 0.9),
            at(PLINTH_TOP, 0.88),
        ],
    );
    b.mirror_y(|b| {
        let y = PLINTH_HALF.y * 0.96;
        b.paint(PLATING_DARK);
        b.block(
            v3(PLINTH_X - 14.0, y - 4.0, SLAB_TOP),
            v3(PLINTH_X + 14.0, y + 3.0, 18.0),
        );
        b.paint(ACCENT).pattern(pattern::SHUTTER);
        b.block(
            v3(PLINTH_X - 10.0, y + 2.9, SLAB_TOP),
            v3(PLINTH_X + 10.0, y + 3.3, 14.0),
        );
        team_panel(b, v3(PLINTH_X + 14.0, 30.0, PLINTH_TOP), v2(12.0, 10.0));
    });
}

/// A leg (+y; mirrored): an armoured column leaning in, the trunnion block that carries
/// the hinge, the bracing across to the other leg, a ram standing it off the plinth.
fn leg(b: &mut MeshBuilder) {
    let fine = b.fine();
    let ring = |(c, half): (Vec3, Vec2), grow: f32| -> Vec<Vec3> {
        chamfered_rect(half + Vec2::splat(grow), 4.0)
            .iter()
            .map(|p| v3(c.x + p[0], c.y + p[1], c.z))
            .collect()
    };
    b.paint(PLATING);
    b.loft(
        &[ring(LEG[0], 0.0), ring(LEG[1], 0.0), ring(LEG[2], 0.0)],
        true,
        true,
    );
    // Armour courses round the leg.
    if !fine {
        return leg_frame(b);
    }
    b.paint(PLATING_DARK);
    for t in [0.3f32, 0.7] {
        let (c, h) = (LEG[0].0.lerp(LEG[1].0, t), LEG[0].1.lerp(LEG[1].1, t));
        let (c2, h2) = (
            LEG[0].0.lerp(LEG[1].0, t + 0.06),
            LEG[0].1.lerp(LEG[1].1, t + 0.06),
        );
        b.loft(&[ring((c, h), 0.7), ring((c2, h2), 0.7)], true, true);
    }
    let (c, h) = (LEG[1].0.lerp(LEG[2].0, 0.45), LEG[1].1.lerp(LEG[2].1, 0.45));
    let (c2, h2) = (LEG[1].0.lerp(LEG[2].0, 0.55), LEG[1].1.lerp(LEG[2].1, 0.55));
    b.loft(&[ring((c, h), 0.7), ring((c2, h2), 0.7)], true, true);
    leg_frame(b);
}

/// What a leg carries at every level: the trunnion block, the bracing across, the ram.
fn leg_frame(b: &mut MeshBuilder) {
    let fine = b.fine();
    // The trunnion block inside the leg's head, round the jaw's pin.
    let y0 = JAW_HALF + 1.0;
    b.paint(PLATING_DARK);
    b.extrude_y(
        &[
            [HINGE_R - 14.0, HINGE_Z - 16.0],
            [LEG[2].0.x, HINGE_Z - 34.0],
            [LEG[2].0.x, HINGE_Z + 16.0],
            [HINGE_R - 4.0, HINGE_Z + 16.0],
            [HINGE_R - 14.0, HINGE_Z + 6.0],
        ],
        y0,
        LEG[2].0.y + 2.0,
    );
    let sides = b.sides(16);
    b.paint(PLATING).pattern(pattern::PLAIN);
    b.cylinder_between(
        HINGE + Vec3::Y * (LEG[2].0.y + 1.9),
        HINGE + Vec3::Y * (LEG[2].0.y + 4.0),
        9.0,
        8.0,
        sides,
    );
    // The bracing across: a heavy beam at the knee and one low down.
    b.paint(PLATING_DARK);
    for t in [0.25f32, 0.95] {
        let c = LEG[0].0.lerp(LEG[1].0, t);
        b.block(v3(c.x - 6.0, 0.0, c.z - 5.0), v3(c.x + 6.0, c.y, c.z + 5.0));
    }
    // The stand-off ram from the plinth's flank up the leg's outer face.
    let foot = v3(PLINTH_X + 4.0, PLINTH_HALF.y * 0.86, PLINTH_TOP);
    let head = LEG[1].0 + v3(4.0, LEG[1].1.y + 3.0, 6.0);
    b.paint(PLATING_DARK);
    b.block(foot - v3(5.0, 4.0, 2.0), foot + v3(5.0, 4.0, 4.0));
    ram(b, foot + Vec3::Z * 2.0, head, 3.4, 0.5);
    if !fine {
        return;
    }
    // X bracing between the legs under the knee, out of the jaw's swing.
    b.paint(PLATING_DARK);
    let (lo, hi) = (LEG[0].0.lerp(LEG[1].0, 0.3), LEG[0].0.lerp(LEG[1].0, 0.9));
    b.beam(
        v3(lo.x, lo.y - 4.0, lo.z),
        v3(hi.x, 0.0, hi.z - 4.0),
        v2(3.0, 3.0),
        v2(3.0, 3.0),
    );
    // Floodlights on the leg's head, and a red lamp.
    floodlight(
        b,
        LEG[2].0 + v3(-LEG[2].1.x - 1.0, 4.0, -8.0),
        v3(0.0, 0.0, 0.0),
        2.2,
    );
    b.paint(GLOW_RED);
    b.cuboid(LEG[2].0 + v3(-6.0, 4.0, 1.0), v3(1.6, 1.6, 2.0));
    // A cable run down the leg's back.
    b.paint(METAL).pattern(pattern::PLAIN);
    let off = v3(LEG[0].1.x * 0.7, 0.0, 0.0);
    b.cylinder_between(LEG[0].0 + off * 1.45, LEG[2].0 + off * 1.3, 1.0, 1.0, 6);
}

/// The back stay from the plinth's tail to the crown.
fn stay(b: &mut MeshBuilder) {
    b.paint(PLATING_DARK);
    girder(b, STAY.0, STAY.1, v2(10.0, 9.0), v2(7.0, 7.0), 3.0);
    if b.fine() {
        b.paint(METAL).pattern(pattern::PLAIN);
        for y in [-4.0f32, 4.0] {
            b.cylinder_between(
                STAY.0 + v3(6.0, y, 0.0),
                STAY.1 + v3(5.0, y, -4.0),
                0.9,
                0.9,
                6,
            );
        }
    }
}

/// The crown over the legs' heads: the owner's colour, a mast and floodlights.
fn crown(b: &mut MeshBuilder) {
    let half = v2((CROWN_X.1 - CROWN_X.0) * 0.5, LEG[2].0.y + LEG[2].1.y + 2.0);
    let mid = (CROWN_X.0 + CROWN_X.1) * 0.5;
    b.paint(PLATING);
    b.loft_z(
        &chamfered_rect(half, 9.0),
        &[
            Section::new(CROWN_Z.0, 0.9).shifted(mid, 0.0),
            Section::new(CROWN_Z.0 + 5.0, 1.0).shifted(mid, 0.0),
            Section::new(CROWN_Z.1, 0.94).shifted(mid, 0.0),
        ],
    );
    b.mirror_y(|b| team_panel(b, v3(mid, 18.0, CROWN_Z.1), v2(20.0, 8.0)));
    b.paint(PLATING).pattern(pattern::HAZARD);
    b.block(
        v3(CROWN_X.0 - 0.3, -half.y + 7.0, CROWN_Z.1 - 5.0),
        v3(CROWN_X.0 + 1.5, half.y - 7.0, CROWN_Z.1 - 3.0),
    );
    if b.fine() {
        antenna_unlit(b, v3(138.0, -20.0, CROWN_Z.1 - 0.5), 24.0, 0.0);
        for y in [-24.0f32, 24.0] {
            floodlight(
                b,
                v3(CROWN_X.0 + 3.0, y, CROWN_Z.1 + 1.4),
                v3(0.0, 0.0, 0.0),
                2.4,
            );
        }
    }
}

/// The fortified well on a tower's side (`part::HULL`, after the tower): a sloped
/// armoured bastion across the tower's arc of the well, and gate piers where it ends at
/// the ways out.
pub(super) fn bastion(b: &mut MeshBuilder) {
    let fine = b.fine();
    let (a0, a1) = (-TAU / 6.0 + EXIT_HALF + 0.05, TAU / 6.0 - EXIT_HALF - 0.05);
    let arcs = b.sides(12);
    let section = [
        (BASTION_IN, SLAB_TOP),
        (BASTION_OUT, SLAB_TOP),
        (BASTION_OUT, BASTION_TOP - 3.0),
        (BASTION_OUT - 4.0, BASTION_TOP),
        (BASTION_IN + 6.0, BASTION_TOP),
        (BASTION_IN, BASTION_TOP - 4.0),
    ];
    let steps = ((arcs as f32 * (a1 - a0) / TAU).ceil() as usize).max(1);
    let rings: Vec<Vec<Vec3>> = (0..=steps)
        .map(|i| {
            let a = a0 + (a1 - a0) * i as f32 / steps as f32;
            let (s, c) = a.sin_cos();
            section.iter().map(|&(r, z)| v3(c * r, s * r, z)).collect()
        })
        .collect();
    b.paint(PLATING);
    b.loft(&rings, true, true);
    b.paint(PLATING).pattern(pattern::HAZARD);
    annulus(
        b,
        arcs,
        BASTION_IN + 0.4,
        BASTION_IN + 6.4,
        BASTION_TOP - 0.1,
        BASTION_TOP + 0.2,
        a0,
        a1,
    );
    // Gate piers at both ends.
    for a in [a0, a1] {
        b.yawed(Vec3::ZERO, a, |b| {
            let x = (BASTION_IN + BASTION_OUT) * 0.5;
            b.paint(PLATING_DARK);
            b.chamfered_box(
                v3(x, 0.0, (SLAB_TOP + 22.0) * 0.5),
                v3(BASTION_OUT - BASTION_IN, 9.0, 22.0 - SLAB_TOP),
                2.5,
            );
            b.paint(PLATING).pattern(pattern::HAZARD);
            b.block(
                v3(BASTION_IN - 0.3, -4.6, 6.0),
                v3(BASTION_OUT - 4.0, 4.6, 8.0),
            );
            if fine {
                b.paint(GLOW_AMBER);
                b.cuboid(v3(x, 0.0, 22.6), v3(2.0, 2.0, 1.2));
                floodlight(b, v3(BASTION_IN + 2.0, 0.0, 20.0), v3(0.0, 0.0, 0.0), 1.8);
            }
        });
    }
    if fine {
        // Embrasures along the bastion's inner face.
        for k in 0..5 {
            let a = a0 + (a1 - a0) * (k as f32 + 0.5) / 5.0;
            b.yawed(Vec3::ZERO, a, |b| {
                b.paint(ACCENT);
                b.block(
                    v3(BASTION_IN - 0.4, -3.0, 6.0),
                    v3(BASTION_IN + 1.6, 3.0, 8.4),
                );
            });
        }
    }
}

// ---- the jaw ------------------------------------------------------------------------

/// The jaw (`part::ELEVATOR_ARM`), shut on the cable: the hinge drum, one straight box
/// girder in to the head, armour along it, and the twin rams along its back.
pub(super) fn jaw(b: &mut MeshBuilder) {
    let fine = b.fine();
    let sides = b.sides(18);
    b.paint(PLATING_DARK);
    b.cylinder_between(
        HINGE - Vec3::Y * JAW_HALF,
        HINGE + Vec3::Y * JAW_HALF,
        15.5,
        15.5,
        sides,
    );
    b.paint(PLATING);
    girder(
        b,
        HINGE,
        TIP,
        v2(JAW_HALF, 13.5),
        v2(JAW_HALF - 2.0, 11.0),
        4.0,
    );
    let along = |t: f32| HINGE.lerp(TIP, t);
    b.paint(PLATING_DARK);
    let sleeves: &[(f32, f32)] = if fine {
        &[(0.18, 0.42), (0.5, 0.74)]
    } else {
        &[]
    };
    for &(t0, t1) in sleeves {
        girder(
            b,
            along(t0),
            along(t1),
            v2(JAW_HALF + 0.5, 14.2),
            v2(JAW_HALF - 0.6, 13.0),
            4.0,
        );
    }
    // The rams along the girder's back, from a lug over the drum to the head's back.
    let axis = (TIP - HINGE).normalize();
    let up = Vec3::Y.cross(axis).normalize();
    let foot = HINGE + up * 20.0 + axis * 8.0;
    let hand = along(0.86) + up * 16.0;
    b.paint(PLATING_DARK);
    b.beam(HINGE + axis * 6.0, foot, v2(16.0, 7.0), v2(16.0, 6.0));
    b.beam(along(0.86), hand + up * 1.5, v2(16.0, 6.0), v2(16.0, 6.0));
    for y in [-7.0f32, 7.0] {
        ram(b, foot + Vec3::Y * y, hand + Vec3::Y * y, 3.6, 0.6);
    }
    head(b);
    // The owner's colour on the head's top.
    team_panel(b, v3(HEAD_BACK - 8.0, 0.0, HEAD_Z + 14.0), v2(12.0, 14.0));
    if fine {
        b.paint(METAL).pattern(pattern::PLAIN);
        for y in [-12.0f32, 12.0] {
            b.cylinder_between(
                along(0.12) + up * 13.6 + Vec3::Y * y,
                along(0.95) + up * 11.1 + Vec3::Y * (y * 0.85),
                1.0,
                1.0,
                6,
            );
        }
    }
}

/// The grip head: a squared yoke at the girder's tip, narrowing to the pads.
fn head(b: &mut MeshBuilder) {
    let ring = |x: f32, half: Vec2| -> Vec<Vec3> {
        chamfered_rect(half, 2.5)
            .iter()
            .map(|p| v3(x, p[0], HEAD_Z + p[1]))
            .collect()
    };
    b.paint(PLATING_DARK);
    b.loft(
        &[
            ring(HEAD_BACK + 6.0, v2(JAW_HALF - 1.0, 13.0)),
            ring(HEAD_BACK - 8.0, v2(JAW_HALF, 14.0)),
            ring(19.0, v2(11.5, 14.0)),
            ring(14.5, v2(9.6, 12.0)),
        ],
        true,
        true,
    );
    grip_pad(b, 15.0, 0.6, 10.0);
    if b.fine() {
        b.paint(PLATING).pattern(pattern::HAZARD);
        b.block(v3(15.0, -8.6, HEAD_Z + 13.6), v3(22.0, 8.6, HEAD_Z + 14.3));
        b.paint(GLOW_LAMP);
        b.block(v3(16.0, -3.0, HEAD_Z - 13.2), v3(24.0, 3.0, HEAD_Z - 12.95));
    }
}
