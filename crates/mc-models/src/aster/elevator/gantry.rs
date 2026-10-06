//! Design A, the base: three hunched gantry towers. Each is a battered plinth, two
//! tall armoured cheeks with the jaw riding on a pin between them, a heavy spine behind
//! and a crown bridging the cheeks over it. The jaw is a knuckle boom: a lower boom
//! up from the hinge, a knuckle, a forearm reaching in over the well to the grip head,
//! and the knuckle's twin rams under it.
//!
//! Tower frame: x out along the tower's bearing, y across, z up.

use glam::{Vec2, Vec3};

use super::*;

/// The plinth: its middle along x, half size at its foot, top, and batter at the top.
const PLINTH_X: f32 = 112.0;
const PLINTH_HALF: Vec2 = Vec2::new(38.0, 52.0);
const PLINTH_TOP: f32 = 40.0;
const PLINTH_BATTER: Vec2 = Vec2::new(0.84, 0.8);
/// The cheeks: inner and outer faces (y), and their side profile (x, z).
const CHEEK_IN: f32 = 17.5;
const CHEEK_OUT: f32 = 42.0;
const CHEEK: [[f32; 2]; 6] = [
    [76.0, PLINTH_TOP - 0.5],
    [140.0, PLINTH_TOP - 0.5],
    [140.0, 126.0],
    [128.0, 146.0],
    [100.0, 146.0],
    [76.0, 134.0],
];
/// The spine behind the jaw's swing, and the crown bridging the cheeks over it.
const SPINE_X: f32 = 118.0;
const CROWN_X: f32 = 112.0;
const CROWN_Z: (f32, f32) = (140.0, 158.0);

/// The jaw: the knuckle, and the back of the grip head.
const KNUCKLE: Vec3 = Vec3::new(52.0, 0.0, 236.0);
const HEAD_BACK: f32 = 32.0;
const HEAD_Z: f32 = GRIP_Z + 2.0;
/// Half the jaw's width: it rides between the cheeks.
const JAW_HALF: f32 = 16.0;
const _: () = assert!(JAW_HALF + 1.0 <= CHEEK_IN);
const _: () = assert!(CROWN_Z.1 + 4.0 <= BEACON_Z && CROWN_X < BEACON_R);

pub(super) fn tower(b: &mut MeshBuilder) {
    plinth(b);
    b.mirror_y(cheek);
    spine(b);
    crown(b);
    // The jaw's pin through both cheeks.
    let sides = b.sides(16);
    b.paint(METAL).pattern(pattern::PLAIN);
    b.cylinder_between(
        HINGE - Vec3::Y * (CHEEK_OUT + 6.0),
        HINGE + Vec3::Y * (CHEEK_OUT + 6.0),
        7.0,
        7.0,
        sides,
    );
}

/// The battered plinth the tower stands on, its foot band and its blast doors.
fn plinth(b: &mut MeshBuilder) {
    let fine = b.fine();
    let plan = chamfered_rect(PLINTH_HALF, 14.0);
    let at = |z: f32, s: Vec2| Section::scaled(z, s.x, s.y).shifted(PLINTH_X, 0.0);
    let batter = |z: f32| Vec2::ONE.lerp(PLINTH_BATTER, z / PLINTH_TOP);
    b.paint(ACCENT);
    b.loft_z(
        &plan,
        &[at(0.0, Vec2::splat(1.03)), at(4.0, Vec2::splat(1.02))],
    );
    b.paint(PLATING);
    b.loft_z(
        &plan,
        &[
            at(3.5, batter(3.5)),
            at(PLINTH_TOP - 3.0, batter(PLINTH_TOP - 3.0)),
            at(PLINTH_TOP, batter(PLINTH_TOP) - Vec2::splat(0.02)),
        ],
    );
    // A dark course round the plinth.
    b.paint(PLATING_DARK);
    b.loft_z(
        &plan,
        &[
            at(18.0, batter(18.0) + Vec2::splat(0.012)),
            at(22.0, batter(22.0) + Vec2::splat(0.012)),
        ],
    );
    // Blast doors on both flanks, facing the ways out.
    b.mirror_y(|b| {
        let y = PLINTH_HALF.y * batter(10.0).y;
        b.paint(PLATING_DARK);
        b.block(
            v3(PLINTH_X - 16.0, y - 4.0, SLAB_TOP),
            v3(PLINTH_X + 16.0, y + 4.5, 21.0),
        );
        b.paint(ACCENT).pattern(pattern::SHUTTER);
        b.block(
            v3(PLINTH_X - 12.0, y + 4.4, SLAB_TOP),
            v3(PLINTH_X + 12.0, y + 4.8, 17.0),
        );
        if fine {
            b.paint(PLATING).pattern(pattern::HAZARD);
            b.block(
                v3(PLINTH_X - 16.0, y + 4.5, 17.0),
                v3(PLINTH_X + 16.0, y + 4.9, 18.4),
            );
            floodlight(
                b,
                v3(PLINTH_X, y + 5.5, 20.0),
                v3(PLINTH_X, y + 30.0, 0.0),
                1.6,
            );
        }
    });
}

/// One cheek (+y; mirrored): a tall armoured wall with a chamfered outer edge, the
/// hinge boss on its outer face, an X of bracing and the ram that stands it off.
fn cheek(b: &mut MeshBuilder) {
    let fine = b.fine();
    let ring = |y: f32, inset: f32| -> Vec<Vec3> {
        let c = Vec2::new(108.0, 95.0);
        CHEEK
            .iter()
            .map(|p| {
                let q = c + (Vec2::new(p[0], p[1]) - c) * (1.0 - inset);
                v3(q.x, y, q.y)
            })
            .collect()
    };
    b.paint(PLATING);
    b.loft(
        &[
            ring(CHEEK_IN, 0.0),
            ring(CHEEK_OUT - 3.0, 0.0),
            ring(CHEEK_OUT, 0.04),
        ],
        true,
        true,
    );
    // The hinge boss and its cap.
    let sides = b.sides(16);
    b.paint(PLATING_DARK);
    b.cylinder_between(
        HINGE + Vec3::Y * (CHEEK_OUT - 1.0),
        HINGE + Vec3::Y * (CHEEK_OUT + 4.0),
        14.0,
        12.5,
        sides,
    );
    b.paint(PLATING).pattern(pattern::PLAIN);
    b.cylinder_between(
        HINGE + Vec3::Y * (CHEEK_OUT + 3.9),
        HINGE + Vec3::Y * (CHEEK_OUT + 5.5),
        8.0,
        7.0,
        sides,
    );
    // Stiffener bands across the outer face.
    let y = CHEEK_OUT - 0.6;
    b.paint(PLATING_DARK);
    for z in [68.0, 104.0] {
        b.block(v3(78.0, y, z - 3.0), v3(138.0, y + 2.2, z + 3.0));
    }
    // The stand-off ram from the plinth up the cheek's back.
    b.paint(PLATING_DARK);
    b.block(
        v3(116.0, CHEEK_OUT - 0.5, 110.0),
        v3(128.0, CHEEK_OUT + 9.0, 122.0),
    );
    b.block(
        v3(136.0, CHEEK_OUT - 4.0, PLINTH_TOP - 4.0),
        v3(148.0, CHEEK_OUT + 9.0, PLINTH_TOP + 6.0),
    );
    ram(
        b,
        v3(142.0, CHEEK_OUT + 5.0, PLINTH_TOP + 4.0),
        v3(122.0, CHEEK_OUT + 5.0, 115.0),
        4.2,
        0.55,
    );
    // The owner's colour along the cheek's top.
    team_panel(
        b,
        v3(112.0, (CHEEK_IN + CHEEK_OUT) * 0.5, 146.0),
        v2(20.0, 14.0),
    );
    if !fine {
        return;
    }
    // An X of plated bracing across the face, between the bands.
    b.paint(PLATING_DARK);
    for (a, z) in [
        (v3(82.0, y, 72.0), v3(134.0, y, 100.0)),
        (v3(134.0, y, 72.0), v3(82.0, y, 100.0)),
    ] {
        b.beam(a, z, v2(4.0, 1.6), v2(4.0, 1.6));
    }
    // Floodlights on the cheek's head, on the well; a red lamp on its leading corner.
    floodlight(b, v3(102.0, CHEEK_OUT - 4.0, 147.5), v3(0.0, 0.0, 0.0), 2.2);
    b.paint(GLOW_RED);
    b.cuboid(v3(101.0, CHEEK_IN + 2.0, 147.0), v3(1.6, 1.6, 2.0));
    // Cable runs down the cheek's back to the plinth.
    b.paint(METAL).pattern(pattern::PLAIN);
    for dy in [3.0f32, 6.0] {
        b.cylinder_between(
            v3(140.6, CHEEK_IN + dy + 4.0, PLINTH_TOP),
            v3(140.6, CHEEK_IN + dy + 4.0, 124.0),
            0.9,
            0.9,
            6,
        );
    }
}

/// The spine behind the jaw, between the cheeks and out past them: a stepped armoured
/// block, its back ribbed, cable trays up it.
fn spine(b: &mut MeshBuilder) {
    let fine = b.fine();
    b.paint(PLATING_DARK);
    let plan = chamfered_rect(v2((150.0 - SPINE_X) * 0.5, CHEEK_OUT), 6.0);
    let mid = (150.0 + SPINE_X) * 0.5;
    b.loft_z(
        &plan,
        &[
            Section::new(PLINTH_TOP - 0.5, 1.0).shifted(mid, 0.0),
            Section::scaled(CROWN_Z.0 + 0.5, 0.92, 0.96).shifted(mid - 1.0, 0.0),
        ],
    );
    // The jaw's cradle under the hinge: it rests on it open.
    b.paint(PLATING);
    b.extrude_y(
        &[
            [80.0, PLINTH_TOP - 0.5],
            [SPINE_X + 1.0, PLINTH_TOP - 0.5],
            [SPINE_X + 1.0, 96.0],
            [100.0, 102.0],
            [84.0, 96.0],
        ],
        -CHEEK_IN,
        CHEEK_IN,
    );
    b.paint(PLATING).pattern(pattern::HAZARD);
    b.block(v3(83.0, -CHEEK_IN, 93.0), v3(87.0, CHEEK_IN, 97.0));
    if !fine {
        return;
    }
    // Ribs down the spine's back and the cable trays between them.
    for y in [-24.0f32, -8.0, 8.0, 24.0] {
        b.paint(PLATING);
        b.block(
            v3(149.0, y - 2.0, PLINTH_TOP),
            v3(152.0, y + 2.0, CROWN_Z.0 - 4.0),
        );
    }
    b.paint(METAL).pattern(pattern::PLAIN);
    for y in [-16.0f32, 0.0, 16.0] {
        b.cylinder_between(
            v3(151.0, y, PLINTH_TOP),
            v3(151.0, y, CROWN_Z.0 - 2.0),
            1.4,
            1.4,
            8,
        );
    }
    // Vents along the spine's flanks.
    b.mirror_y(|b| {
        b.paint(ACCENT);
        for k in 0..3 {
            let z = 56.0 + 24.0 * k as f32;
            b.block(
                v3(126.0, CHEEK_OUT - 0.2, z),
                v3(144.0, CHEEK_OUT + 0.6, z + 6.0),
            );
        }
    });
}

/// The crown: a heavy bridge over the cheeks' backs, the owner's colour on it, the
/// beacon's mast in its middle (`super::beacon`), a mast and floodlights.
fn crown(b: &mut MeshBuilder) {
    let fine = b.fine();
    let half = v2((152.0 - CROWN_X) * 0.5, CHEEK_OUT + 4.0);
    let mid = (152.0 + CROWN_X) * 0.5;
    b.paint(PLATING);
    b.loft_z(
        &chamfered_rect(half, 8.0),
        &[
            Section::new(CROWN_Z.0, 0.94).shifted(mid, 0.0),
            Section::new(CROWN_Z.0 + 4.0, 1.0).shifted(mid, 0.0),
            Section::new(CROWN_Z.1 - 2.0, 1.0).shifted(mid, 0.0),
            Section::new(CROWN_Z.1, 0.95).shifted(mid, 0.0),
        ],
    );
    b.mirror_y(|b| team_panel(b, v3(mid + 2.0, 22.0, CROWN_Z.1), v2(18.0, 10.0)));
    b.paint(PLATING).pattern(pattern::HAZARD);
    b.block(
        v3(CROWN_X - 0.4, -half.y + 6.0, CROWN_Z.1 - 4.0),
        v3(CROWN_X + 1.0, half.y - 6.0, CROWN_Z.1 - 2.2),
    );
    if !fine {
        return;
    }
    b.paint(PLATING_DARK);
    b.chamfered_box(v3(144.0, 0.0, CROWN_Z.1 + 3.0), v3(10.0, 16.0, 6.0), 2.0);
    antenna_unlit(b, v3(146.0, -26.0, CROWN_Z.1), 26.0, 0.0);
    antenna_unlit(b, v3(146.0, 26.0, CROWN_Z.1), 18.0, 0.0);
    for y in [-30.0f32, 30.0] {
        floodlight(
            b,
            v3(CROWN_X + 3.0, y, CROWN_Z.1 + 1.6),
            v3(0.0, 0.0, 0.0),
            2.4,
        );
    }
}

// ---- the jaw ------------------------------------------------------------------------

/// The jaw (`part::ELEVATOR_ARM`), shut on the cable: the hinge drum, the lower boom,
/// the knuckle, the forearm, the grip head and its pads, and the knuckle's rams.
pub(super) fn jaw(b: &mut MeshBuilder) {
    let fine = b.fine();
    let sides = b.sides(20);
    b.paint(PLATING_DARK);
    b.cylinder_between(
        HINGE - Vec3::Y * JAW_HALF,
        HINGE + Vec3::Y * JAW_HALF,
        15.0,
        15.0,
        sides,
    );
    // The lower boom, an armour sleeve over its middle, and the knuckle.
    b.paint(PLATING);
    girder(
        b,
        HINGE,
        KNUCKLE,
        v2(JAW_HALF, 13.0),
        v2(JAW_HALF - 1.0, 12.0),
        4.0,
    );
    let along = |t: f32| HINGE.lerp(KNUCKLE, t);
    let up = Vec3::Y.cross((KNUCKLE - HINGE).normalize()).normalize();
    b.paint(PLATING_DARK);
    girder(
        b,
        along(0.2),
        along(0.72),
        v2(JAW_HALF + 0.5, 13.8),
        v2(JAW_HALF - 0.4, 12.9),
        4.0,
    );
    b.cylinder_between(
        KNUCKLE - Vec3::Y * (JAW_HALF - 0.5),
        KNUCKLE + Vec3::Y * (JAW_HALF - 0.5),
        14.0,
        14.0,
        sides,
    );
    // The forearm, in over the well to the head.
    let wrist = v3(HEAD_BACK, 0.0, KNUCKLE.z);
    b.paint(PLATING);
    girder(
        b,
        KNUCKLE,
        wrist,
        v2(JAW_HALF - 1.0, 11.5),
        v2(JAW_HALF - 1.5, 11.0),
        3.4,
    );
    team_panel(b, v3(42.0, 0.0, KNUCKLE.z + 11.25), v2(12.0, 14.0));
    head(b);
    // The knuckle's twin rams under the boom and the forearm, and their lugs.
    let foot = along(0.32) - up * 17.0;
    let hand = v3(40.0, 0.0, HEAD_Z - 16.0);
    b.paint(PLATING_DARK);
    b.beam(along(0.32), foot, v2(20.0, 8.0), v2(20.0, 7.0));
    b.beam(
        v3(42.0, 0.0, KNUCKLE.z - 6.0),
        hand,
        v2(20.0, 7.0),
        v2(20.0, 7.0),
    );
    for y in [-7.0f32, 7.0] {
        ram(b, foot + Vec3::Y * y, hand + Vec3::Y * y, 3.4, 0.55);
    }
    if !fine {
        return;
    }
    // Hydraulic lines up the boom's back, and the knuckle's hub caps.
    b.paint(METAL).pattern(pattern::PLAIN);
    for y in [-6.0f32, 6.0] {
        b.cylinder_between(
            HINGE + up * 14.5 + Vec3::Y * y,
            KNUCKLE + up * 12.5 + Vec3::Y * y,
            1.0,
            1.0,
            6,
        );
    }
    b.paint(PLATING).pattern(pattern::PLAIN);
    for s in [-1.0f32, 1.0] {
        b.cylinder_between(
            KNUCKLE + Vec3::Y * s * (JAW_HALF - 0.6),
            KNUCKLE + Vec3::Y * s * (JAW_HALF + 0.6),
            7.5,
            6.5,
            12,
        );
    }
    // Hazard bands on the forearm's nose.
    b.paint(PLATING).pattern(pattern::HAZARD);
    let nose = v3(HEAD_BACK + 0.5, 0.0, KNUCKLE.z);
    girder(
        b,
        nose + Vec3::X * 3.0,
        nose,
        v2(JAW_HALF - 1.0, 11.6),
        v2(JAW_HALF - 1.0, 11.6),
        3.4,
    );
}

/// The grip head: a heavy wedge narrowing toward the cable, its pads on the band, a
/// lamp under it on the deck.
fn head(b: &mut MeshBuilder) {
    let ring = |x: f32, half: Vec2| -> Vec<Vec3> {
        chamfered_rect(half, 3.0)
            .iter()
            .map(|p| v3(x, p[0], HEAD_Z + p[1]))
            .collect()
    };
    b.paint(PLATING_DARK);
    b.loft(
        &[
            ring(HEAD_BACK + 2.0, v2(JAW_HALF, 14.0)),
            ring(21.0, v2(12.0, 13.5)),
            ring(14.5, v2(9.6, 12.0)),
        ],
        true,
        true,
    );
    grip_pad(b, 15.0, 0.62, 10.5);
    if b.fine() {
        b.paint(PLATING).pattern(pattern::HAZARD);
        b.block(v3(15.5, -8.0, HEAD_Z + 12.2), v3(21.0, 8.0, HEAD_Z + 13.0));
        b.paint(GLOW_LAMP);
        b.block(v3(18.0, -3.0, HEAD_Z - 13.15), v3(24.0, 3.0, HEAD_Z - 12.9));
    }
}
