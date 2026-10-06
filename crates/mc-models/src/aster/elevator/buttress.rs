//! Design C (`space_elevator~c`): buttress towers on stepped terraces. Each tower
//! stands on three stepped terraces fanned round its side of the well; two tall
//! buttress walls rise from them, sloped down to the back like a dam's, a spine and a
//! bridge between them, and plated struts down their flanks to the terraces. The jaw is
//! a curved claw: an armoured talon arched up from the hinge and down onto the cable,
//! in jointed plates, a ram along its inner curve.
//!
//! Tower frame: x out along the tower's bearing, y across, z up.

use glam::{Vec2, Vec3};

use super::*;

/// The terraces: inner radius, outer edge (x), half the arc they take, and their top.
const TERRACES: [(f32, f32, f32, f32); 3] = [
    (70.0, 148.0, 0.62, 12.0),
    (76.0, 142.0, 0.5, 24.0),
    (82.0, 136.0, 0.38, 36.0),
];
const TERRACE_TOP: f32 = TERRACES[2].3;
/// The buttress walls: inner and outer faces (y), and their side profile (x, z).
const WALL_IN: f32 = 17.5;
const WALL_OUT: f32 = 37.0;
const WALL: [[f32; 2]; 7] = [
    [80.0, TERRACE_TOP - 0.5],
    [142.0, TERRACE_TOP - 0.5],
    [140.0, 80.0],
    [132.0, 160.0],
    [104.0, 160.0],
    [84.0, 146.0],
    [80.0, 128.0],
];
/// The bridge between the walls' heads, behind the claw's swing.
const BRIDGE_X: f32 = 126.0;
const BRIDGE_Z: (f32, f32) = (146.0, 158.0);

/// The claw: its curve (a cubic from the hinge to the head), half its width at the root
/// and at the tip, half its depth.
const CLAW: [Vec3; 4] = [
    HINGE,
    Vec3::new(82.0, 0.0, 194.0),
    Vec3::new(62.0, 0.0, 258.0),
    Vec3::new(28.0, 0.0, 242.0),
];
const CLAW_HALF: (f32, f32) = (15.0, 11.0);
const CLAW_DEEP: (f32, f32) = (14.0, 10.0);
const HEAD_Z: f32 = GRIP_Z + 1.0;

pub(super) fn tower(b: &mut MeshBuilder) {
    terraces(b);
    b.mirror_y(wall);
    spine(b);
    let sides = b.sides(16);
    b.paint(METAL).pattern(pattern::PLAIN);
    b.cylinder_between(
        HINGE - Vec3::Y * (WALL_OUT + 5.0),
        HINGE + Vec3::Y * (WALL_OUT + 5.0),
        7.0,
        7.0,
        sides,
    );
}

/// The plan of one terrace: an arc round the well and a straight back.
fn terrace_plan(r: f32, x: f32, half: f32, steps: usize) -> Vec<[f32; 2]> {
    let mut plan: Vec<[f32; 2]> = (0..=steps)
        .map(|i| {
            let a = -half + 2.0 * half * i as f32 / steps as f32;
            [r * a.cos(), r * a.sin()]
        })
        .collect();
    // Kept inside the lot when the tower is turned to 120 degrees.
    let y = (x * half.tan()).min(x * 0.34);
    plan.push([x, y]);
    plan.push([x, -y]);
    plan
}

/// The stepped terraces, each with a dark riser band and a hazard edge, the owner's
/// colour on the top one.
fn terraces(b: &mut MeshBuilder) {
    let fine = b.fine();
    let steps = if b.mid() { 6 } else { 3 };
    let mut z0 = 0.0;
    for (k, &(r, x, half, top)) in TERRACES.iter().enumerate() {
        let plan = terrace_plan(r, x, half, steps);
        b.paint(if k == 0 { PLATING_DARK } else { PLATING });
        b.loft_z(
            &plan,
            &[
                Section::new(z0, 1.0),
                Section::new(top - 1.5, 0.99),
                Section::new(top, 0.985),
            ],
        );
        if fine {
            b.paint(PLATING).pattern(pattern::HAZARD);
            annulus(
                b,
                steps * 6,
                r * 0.985 - 0.4,
                r * 0.985 + 2.5,
                top - 0.2,
                top + 0.12,
                -half,
                half,
            );
        }
        z0 = top - 0.5;
    }
    if !fine {
        return;
    }
    // Doors in the lowest riser's flanks, toward the ways out.
    b.mirror_y(|b| {
        let (r, x, half, _) = TERRACES[0];
        let plan = terrace_plan(r, x, half, 1);
        let (e0, e1) = (Vec2::from(plan[1]), Vec2::from(plan[2]));
        let at = e0.lerp(e1, 0.5);
        let along = (e1 - e0).normalize();
        b.yawed(at.extend(0.0), along.y.atan2(along.x), |b| {
            b.paint(PLATING_DARK);
            b.block(v3(-12.0, -2.0, SLAB_TOP), v3(12.0, 4.0, 11.0));
            b.paint(ACCENT).pattern(pattern::SHUTTER);
            b.block(v3(-9.0, 3.9, SLAB_TOP), v3(9.0, 4.3, 9.0));
        });
    });
}

/// One buttress wall (+y; mirrored): sloped down to the back, its outer edge
/// chamfered, the hinge boss, struts to the terraces, lamps on its head.
fn wall(b: &mut MeshBuilder) {
    let fine = b.fine();
    let ring = |y: f32, inset: f32| -> Vec<Vec3> {
        let c = Vec2::new(110.0, 98.0);
        WALL.iter()
            .map(|p| {
                let q = c + (Vec2::new(p[0], p[1]) - c) * (1.0 - inset);
                v3(q.x, y, q.y)
            })
            .collect()
    };
    b.paint(PLATING);
    b.loft(
        &[
            ring(WALL_IN, 0.0),
            ring(WALL_OUT - 3.0, 0.0),
            ring(WALL_OUT, 0.04),
        ],
        true,
        true,
    );
    let sides = b.sides(16);
    b.paint(PLATING_DARK);
    b.cylinder_between(
        HINGE + Vec3::Y * (WALL_OUT - 1.0),
        HINGE + Vec3::Y * (WALL_OUT + 4.0),
        13.0,
        11.5,
        sides,
    );
    // Plated struts from the terraces up the wall's flank: a brace is a plated strut.
    b.paint(PLATING_DARK);
    for (foot, head) in [
        (
            v3(134.0, 48.0, TERRACES[1].3),
            v3(130.0, WALL_OUT - 1.0, 112.0),
        ),
        (
            v3(104.0, 50.0, TERRACES[0].3),
            v3(100.0, WALL_OUT - 1.0, 104.0),
        ),
    ] {
        girder_across(b, foot, head, v2(6.0, 4.0), v2(4.5, 3.0));
    }
    team_panel(
        b,
        v3(118.0, (WALL_IN + WALL_OUT) * 0.5, 160.0),
        v2(18.0, 12.0),
    );
    if !fine {
        return;
    }
    // A ram under each strut, its armoured sleeve.
    ram(
        b,
        v3(122.0, 52.0, TERRACES[0].3 + 1.0),
        v3(118.0, WALL_OUT + 3.0, 84.0),
        3.6,
        0.55,
    );
    b.paint(PLATING).pattern(pattern::HAZARD);
    b.block(v3(85.0, WALL_IN, 144.0), v3(89.0, WALL_OUT - 3.0, 148.0));
    floodlight(b, v3(108.0, WALL_OUT - 4.0, 161.5), v3(0.0, 0.0, 0.0), 2.4);
    b.paint(GLOW_RED);
    b.cuboid(v3(107.0, WALL_IN + 2.0, 161.0), v3(1.6, 1.6, 2.0));
    b.paint(METAL).pattern(pattern::PLAIN);
    for dy in [4.0f32, 8.0] {
        b.cylinder_between(
            v3(139.8, WALL_IN + dy, TERRACE_TOP),
            v3(132.8, WALL_IN + dy, 152.0),
            0.9,
            0.9,
            6,
        );
    }
}

/// A plated strut from `a` to `z` whose section is square to its own run (no plane).
fn girder_across(b: &mut MeshBuilder, a: Vec3, z: Vec3, half_a: Vec2, half_b: Vec2) {
    b.beam(a, z, half_a * 2.0, half_b * 2.0);
}

/// The spine between the walls behind the claw's swing, and the bridge over it that
/// carries the beacon.
fn spine(b: &mut MeshBuilder) {
    b.paint(PLATING_DARK);
    b.extrude_y(
        &[
            [118.0, TERRACE_TOP - 0.5],
            [141.0, TERRACE_TOP - 0.5],
            [138.0, 96.0],
            [130.0, 134.0],
            [118.0, 134.0],
        ],
        -WALL_IN - 0.5,
        WALL_IN + 0.5,
    );
    // The claw's seat under the hinge.
    b.paint(PLATING);
    b.extrude_y(
        &[
            [80.0, TERRACE_TOP - 0.5],
            [119.0, TERRACE_TOP - 0.5],
            [119.0, 94.0],
            [100.0, 101.0],
            [84.0, 94.0],
        ],
        -WALL_IN,
        WALL_IN,
    );
    b.paint(PLATING);
    b.loft_z(
        &chamfered_rect(v2((142.0 - BRIDGE_X) * 0.5, WALL_OUT), 5.0),
        &[
            Section::new(BRIDGE_Z.0, 0.94).shifted((142.0 + BRIDGE_X) * 0.5, 0.0),
            Section::new(BRIDGE_Z.0 + 3.0, 1.0).shifted((142.0 + BRIDGE_X) * 0.5, 0.0),
            Section::new(BRIDGE_Z.1, 0.96).shifted((142.0 + BRIDGE_X) * 0.5, 0.0),
        ],
    );
    b.mirror_y(|b| team_panel(b, v3(134.0, 18.0, BRIDGE_Z.1), v2(12.0, 10.0)));
    if b.fine() {
        b.paint(PLATING).pattern(pattern::HAZARD);
        b.block(
            v3(BRIDGE_X - 0.3, -WALL_IN, BRIDGE_Z.1 - 4.0),
            v3(BRIDGE_X + 1.0, WALL_IN, BRIDGE_Z.1 - 2.2),
        );
        antenna_unlit(b, v3(138.0, -8.0, BRIDGE_Z.1 - 0.5), 22.0, 0.0);
    }
}

// ---- the claw -----------------------------------------------------------------------

/// A point on the claw's curve.
fn curve(t: f32) -> Vec3 {
    let u = 1.0 - t;
    CLAW[0] * (u * u * u)
        + CLAW[1] * (3.0 * u * u * t)
        + CLAW[2] * (3.0 * u * t * t)
        + CLAW[3] * (t * t * t)
}

/// The claw (`part::ELEVATOR_ARM`), shut on the cable: the hinge drum, the arched talon
/// in jointed plates, the head and its pads, a ram along the inner curve.
pub(super) fn jaw(b: &mut MeshBuilder) {
    let fine = b.fine();
    let sides = b.sides(18);
    b.paint(PLATING_DARK);
    b.cylinder_between(
        HINGE - Vec3::Y * CLAW_HALF.0,
        HINGE + Vec3::Y * CLAW_HALF.0,
        15.0,
        15.0,
        sides,
    );
    // The talon: one swept body, then its armour plates over it in overlapping courses.
    let n = if fine { 14 } else { 7 };
    let frame = |t: f32| {
        let p = curve(t);
        let axis = (curve((t + 0.01).min(1.0)) - curve((t - 0.01).max(0.0))).normalize();
        let up = Vec3::Y.cross(axis).normalize();
        (p, up)
    };
    let ring = |t: f32, grow: f32| -> Vec<Vec3> {
        let (p, up) = frame(t);
        let half = v2(
            CLAW_HALF.0 + (CLAW_HALF.1 - CLAW_HALF.0) * t + grow,
            CLAW_DEEP.0 + (CLAW_DEEP.1 - CLAW_DEEP.0) * t + grow,
        );
        chamfered_rect(half, 3.0)
            .iter()
            .map(|q| p + Vec3::Y * q[0] + up * q[1])
            .collect()
    };
    b.paint(PLATING);
    let rings: Vec<Vec<Vec3>> = (0..=n).map(|i| ring(i as f32 / n as f32, 0.0)).collect();
    b.loft(&rings, true, true);
    let plates = if fine { 5 } else { 3 };
    b.paint(PLATING_DARK);
    for k in 0..plates {
        let t0 = 0.08 + 0.84 * k as f32 / plates as f32;
        let t1 = t0 + 0.84 / plates as f32 * 0.8;
        let mid = (t0 + t1) * 0.5;
        b.loft(&[ring(t0, 0.5), ring(mid, 0.9), ring(t1, 0.7)], true, true);
    }
    head(b);
    // The ram along the inner curve, from a lug near the root to one near the head.
    let (p0, up0) = frame(0.22);
    let (p1, up1) = frame(0.78);
    let foot = p0 - up0 * 19.0;
    let hand = p1 - up1 * 16.0;
    b.paint(PLATING_DARK);
    b.beam(p0, foot, v2(12.0, 6.0), v2(12.0, 5.0));
    b.beam(p1, hand, v2(12.0, 5.0), v2(12.0, 5.0));
    ram(b, foot, hand, 4.0, 0.55);
    if fine {
        b.paint(METAL).pattern(pattern::PLAIN);
        for y in [-7.0f32, 7.0] {
            let pts: Vec<Vec3> = (0..=6)
                .map(|i| {
                    let (p, up) = frame(i as f32 / 6.0 * 0.9);
                    p + up * 13.5 + Vec3::Y * y
                })
                .collect();
            for w in pts.windows(2) {
                b.cylinder_between(w[0], w[1], 0.8, 0.8, 6);
            }
        }
    }
}

/// The head at the talon's tip: a wedge turned down onto the cable, its pads.
fn head(b: &mut MeshBuilder) {
    let ring = |x: f32, half: Vec2, z: f32| -> Vec<Vec3> {
        chamfered_rect(half, 3.0)
            .iter()
            .map(|p| v3(x, p[0], z + p[1]))
            .collect()
    };
    b.paint(PLATING_DARK);
    b.loft(
        &[
            ring(34.0, v2(11.0, 10.0), 242.0),
            ring(26.0, v2(11.5, 14.0), HEAD_Z + 2.0),
            ring(20.0, v2(11.0, 13.0), HEAD_Z),
            ring(14.5, v2(9.6, 11.5), HEAD_Z),
        ],
        true,
        true,
    );
    grip_pad(b, 15.0, 0.6, 10.0);
    if b.fine() {
        b.paint(PLATING).pattern(pattern::HAZARD);
        b.block(v3(15.5, -8.0, HEAD_Z + 11.8), v3(21.0, 8.0, HEAD_Z + 12.6));
        b.paint(GLOW_LAMP);
        b.block(v3(16.0, -3.0, HEAD_Z - 12.75), v3(22.0, 3.0, HEAD_Z - 12.5));
    }
}
