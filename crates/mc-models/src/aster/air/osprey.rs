//! Osprey: the tech 1 reclaim carrier, a heavy-lift tilt-jet that hangs over a wreck
//! field while its four Salvage Drones work it. The drones ride slung under the wing,
//! two a side, each gripped by the clamp of a pylon; they drop clear to fly and rise
//! back onto the clamp to come home (`air_support::seat_drones`), so the flock is part
//! of the silhouette. Two blue-burning jet nacelles tilt to hover and lie down to cruise.
//! Amber, the colour of Aster's economy, marks the clamps and the salvage hopper.
//!
//! The Salvage Drone ([`drone`]) is here too.
use super::*;

/// The pylons on the left (+y) wing, inboard first (the right mirrors): where each
/// drone's clamp grips it, x and y. The blueprint's `drone_sockets` say the same.
pub(crate) const PYLONS: [[f32; 2]; 2] = [[0.4, 3.6], [0.4, 6.4]];
/// A docked drone's feet (its own z 0) in the Osprey's frame: the sockets' z.
pub(crate) const DOCK_Z: f32 = 0.25;
/// The drone's two lugs the clamp jaws close on: fore and aft of its middle, their tops
/// this high over its feet.
pub(crate) const LUG_X: [f32; 2] = [0.62, -0.62];
pub(crate) const LUG_TOP: f32 = 1.0;
/// The widest a drone may be, either side of its middle, to clear its neighbour.
#[cfg(test)]
pub(crate) const DRONE_HALF_WIDTH: f32 = 1.2;

/// Where the clamp jaws meet the lugs, in the Osprey's frame.
const GRIP_Z: f32 = DOCK_Z + LUG_TOP;

/// A wing section: a slim lens round the chord, (y, z, leading x, trailing x, thickness).
fn section(s: (f32, f32, f32, f32, f32)) -> Vec<Vec3> {
    let (y, z, lead, trail, t) = s;
    let chord = lead - trail;
    vec![
        v3(lead, y, z),
        v3(lead - chord * 0.3, y, z + t * 0.5),
        v3(trail, y, z + t * 0.1),
        v3(trail, y, z - t * 0.1),
        v3(lead - chord * 0.3, y, z - t * 0.5),
    ]
}

/// The underside of a wing between two sections at span `y`.
fn wing_under(root: (f32, f32, f32, f32, f32), tip: (f32, f32, f32, f32, f32), y: f32) -> f32 {
    let k = ((y - root.0) / (tip.0 - root.0)).clamp(0.0, 1.0);
    let z = root.1 + (tip.1 - root.1) * k;
    let t = root.4 + (tip.4 - root.4) * k;
    z - t * 0.45
}

/// A faceted section about the x axis through `c`: half width `w`, half height `h`,
/// corners cut by `cut`.
fn facet(c: Vec3, x: f32, w: f32, h: f32, cut: f32) -> Vec<Vec3> {
    [
        [w, h - cut],
        [w - cut, h],
        [-(w - cut), h],
        [-w, h - cut],
        [-w, -(h - cut)],
        [-(w - cut), -h],
        [w - cut, -h],
        [w, -(h - cut)],
    ]
    .iter()
    .map(|p| v3(x, c.y + p[0], c.z + p[1]))
    .collect()
}

/// A tilt-jet nacelle about its pivot, lying along +x: a faceted body with a dark
/// inlet, drawn in to a gunmetal nozzle with the blue hot slot in it.
struct Jet {
    ahead: f32,
    behind: f32,
    w: f32,
    h: f32,
}

impl Jet {
    fn build(&self, b: &mut MeshBuilder, pivot: Vec3) {
        let (w, h) = (self.w, self.h);
        let x = |d: f32| pivot.x + d;
        let cut = w.min(h) * 0.34;
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        let body = [
            facet(pivot, x(self.ahead), w * 0.9, h * 0.88, cut),
            facet(pivot, x(self.ahead - 0.5), w, h, cut),
            facet(pivot, x(-self.behind + 0.5), w, h, cut),
            facet(pivot, x(-self.behind + 0.2), w * 0.84, h * 0.74, cut * 0.8),
        ];
        if b.fine() {
            b.loft(&body, false, false);
        } else {
            b.loft(&[body[0].clone(), body[3].clone()], false, false);
        }
        b.paint(ACCENT);
        b.face(&facet(
            pivot,
            x(self.ahead - 0.08),
            w * 0.8,
            h * 0.78,
            cut * 0.9,
        ));
        b.paint(METAL);
        b.loft(
            &[
                facet(pivot, x(-self.behind + 0.2), w * 0.84, h * 0.74, cut * 0.8),
                facet(pivot, x(-self.behind), w * 0.74, h * 0.6, cut * 0.6),
            ],
            false,
            false,
        );
        b.paint(GLOW);
        let mut slot = facet(pivot, x(-self.behind + 0.06), w * 0.6, h * 0.46, cut * 0.4);
        slot.reverse();
        b.face(&slot);
        if b.fine() {
            // The inlet's lip, the owner's band, a strake along the top.
            b.paint(PLATING_DARK);
            b.loft(
                &[
                    facet(pivot, x(self.ahead + 0.02), w * 0.94, h * 0.92, cut),
                    facet(pivot, x(self.ahead - 0.12), w * 0.96, h * 0.94, cut),
                ],
                false,
                false,
            );
            b.beam(
                v3(x(self.ahead - 0.7), pivot.y, pivot.z + h),
                v3(x(-self.behind + 0.6), pivot.y, pivot.z + h),
                v2(0.18, 0.1),
                v2(0.18, 0.1),
            );
            b.paint(TEAM);
            b.loft(
                &[
                    facet(pivot, x(-0.1), w * 1.02, h * 1.02, cut),
                    facet(pivot, x(-0.45), w * 1.02, h * 1.02, cut),
                ],
                false,
                false,
            );
        }
    }
}

/// Sets the VTOL pair and builds a nacelle on each side about `pivot` (left side).
fn jets(b: &mut MeshBuilder, pivot: Vec3, jet: &Jet) {
    b.set_vtol(crate::Vtol {
        pivots: [pivot.to_array(), [0.0; 3]],
        pairs: 1,
        nozzle: [jet.behind, jet.h * 0.46],
        fans: false,
    });
    b.mirror_y(|b| b.with_part(part::VTOL_FRONT, |b| jet.build(b, pivot)));
}

/// A drone pylon under the wing at `p`, from the wing's underside at `top` down to the
/// clamp: a blade, a clamp beam fore and aft, a jaw over each of the drone's lugs, an
/// amber lamp on the blade's nose.
fn pylon(b: &mut MeshBuilder, p: [f32; 2], top: f32) {
    let (x, y) = (p[0], p[1]);
    let beam_z = GRIP_Z + 0.3;
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.extrude_y(
        &[
            [x + 0.55, beam_z],
            [x + 0.85, top + 0.05],
            [x - 0.75, top + 0.05],
            [x - 0.55, beam_z],
        ],
        y - 0.08,
        y + 0.08,
    );
    b.paint(METAL);
    b.beam(
        v3(x + LUG_X[0] + 0.14, y, beam_z),
        v3(x + LUG_X[1] - 0.14, y, beam_z),
        v2(0.3, 0.2),
        v2(0.3, 0.2),
    );
    if !b.fine() {
        return;
    }
    b.paint(ACCENT);
    for lx in LUG_X {
        b.cuboid(v3(x + lx, y, GRIP_Z + 0.08), v3(0.22, 0.36, 0.3));
    }
    b.paint(GLOW_AMBER);
    b.cuboid(v3(x + 0.62, y, beam_z + 0.28), v3(0.12, 0.1, 0.12));
}

/// Pylons under both wings.
fn pylons(b: &mut MeshBuilder, under: impl Fn(f32) -> f32) {
    b.mirror_y(|b| {
        for p in PYLONS {
            pylon(b, p, under(p[1]));
        }
    });
}

/// Far away: a plan of the hull and wing, and a nacelle block at each pivot.
fn coarse(b: &mut MeshBuilder, hull: &[[f32; 2]], wing: &[[f32; 2]], z: f32, pivot: Vec3) {
    b.paint(PLATING);
    b.extrude_z(hull, z - 0.9, z + 0.5);
    b.mirror_y(|b| {
        b.paint(PLATING);
        b.face(
            &wing
                .iter()
                .map(|p| v3(p[0], p[1], z + 0.15))
                .collect::<Vec<_>>(),
        );
        b.paint(PLATING_DARK);
        b.with_part(part::VTOL_FRONT, |b| {
            b.cuboid(pivot, v3(3.6, 1.3, 1.3));
        });
    });
}

// ---- A: twin-boom tilt-jet ----------------------------------------------------

/// A: a deep pod fuselage with a glazed nose, a long straight shoulder wing with a
/// nacelle at each tip, twin booms back to a tailplane and two fins. From above: a
/// long plank of wing with the four drones under it, the booms framing the hopper.
pub(super) fn build(b: &mut MeshBuilder) {
    const HULL: [[f32; 9]; 5] = [
        [6.2, 0.1, 1.3, 0.25, 1.45, 0.2, 1.75, 0.05, 1.85],
        [5.0, 0.6, 0.7, 1.1, 1.2, 0.95, 2.3, 0.35, 2.6],
        [3.2, 0.9, 0.45, 1.45, 1.15, 1.35, 2.9, 0.55, 3.3],
        [-1.6, 0.9, 0.45, 1.5, 1.15, 1.4, 2.95, 0.6, 3.35],
        [-3.8, 0.35, 1.5, 0.7, 1.8, 0.6, 2.9, 0.25, 3.1],
    ];
    const ROOT: (f32, f32, f32, f32, f32) = (1.3, 2.95, 1.9, -1.3, 0.42);
    const TIP: (f32, f32, f32, f32, f32) = (8.5, 2.75, 1.5, -0.9, 0.3);
    const PIVOT: Vec3 = Vec3::new(0.3, 9.15, 2.75);
    const BOOM_Y: f32 = 2.3;
    let jet = Jet {
        ahead: 2.0,
        behind: 1.9,
        w: 0.66,
        h: 0.74,
    };
    if b.coarse() {
        b.set_vtol(crate::Vtol {
            pivots: [PIVOT.to_array(), [0.0; 3]],
            pairs: 1,
            nozzle: [jet.behind, jet.h * 0.46],
            fans: false,
        });
        coarse(
            b,
            &[
                [6.2, 0.0],
                [3.2, 1.4],
                [-3.8, 0.7],
                [-3.8, -0.7],
                [3.2, -1.4],
            ],
            &[[1.9, 0.0], [1.5, 8.5], [-0.9, 8.5], [-1.3, 0.0]],
            2.8,
            PIVOT,
        );
        return;
    }
    let fine = b.fine();
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.loft(&band(&HULL, 0, 1), true, true);
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.loft(&band(&HULL, 1, 2), true, true);
    b.paint(GLASS);
    b.loft(&band(&HULL[..3], 2, 3), true, false);
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.loft(&band(&HULL[2..], 2, 3), false, true);
    // The salvage hopper on the back: a dark bin with amber slots, the owner's panel.
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.loft_z(
        &[
            [1.6, 0.0],
            [1.0, 0.95],
            [-2.8, 0.95],
            [-3.3, 0.0],
            [-2.8, -0.95],
            [1.0, -0.95],
        ],
        &[
            crate::builder::Section::new(3.2, 1.0),
            crate::builder::Section::new(3.85, 0.88),
        ],
    );
    if fine {
        vent(b, v3(-0.8, 0.0, 3.85), v2(2.6, 1.1), 5, GLOW_AMBER);
    }
    team_panel(b, v3(2.2, 0.0, 3.25), v2(1.0, 0.8));

    // The wing, straight through, and the booms off its trailing edge.
    b.mirror_y(|b| {
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        b.loft(&[section(ROOT), section(TIP)], true, true);
        if b.fine() {
            b.paint(TEAM).pattern(pattern::TEAM_BAND);
            b.loft(
                &[
                    section((7.3, 2.79, 1.62, -1.02, 0.34)),
                    section((7.9, 2.77, 1.56, -0.96, 0.33)),
                ],
                false,
                false,
            );
        }
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        b.beam(
            v3(0.6, BOOM_Y, 2.7),
            v3(-7.3, BOOM_Y, 2.85),
            v2(0.5, 0.55),
            v2(0.34, 0.4),
        );
        // A fin on each boom's end.
        b.extrude_y(
            &[[-5.9, 2.9], [-6.9, 4.5], [-7.7, 4.5], [-7.5, 2.9]],
            BOOM_Y - 0.08,
            BOOM_Y + 0.08,
        );
        if b.fine() {
            b.paint(PLATING_DARK);
            b.extrude_y(
                &[[-6.75, 4.25], [-6.9, 4.5], [-7.7, 4.5], [-7.65, 4.25]],
                BOOM_Y - 0.09,
                BOOM_Y + 0.09,
            );
        }
    });
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.extrude_z(
        &[
            [-6.2, BOOM_Y],
            [-7.4, BOOM_Y],
            [-7.4, -BOOM_Y],
            [-6.2, -BOOM_Y],
        ],
        2.9,
        3.05,
    );
    if fine {
        // A chin window for the salvage master, looking down.
        b.paint(GLASS);
        b.extrude_y(
            &[[5.3, 0.95], [4.2, 0.5], [3.4, 0.5], [3.4, 0.9]],
            -0.5,
            0.5,
        );
    }
    pylons(b, |y| wing_under(ROOT, TIP, y));
    jets(b, PIVOT, &jet);
}

// ---- B: flying wing -----------------------------------------------------------

/// B: a broad blended flying wing, the body swelling out of it with the canopy up
/// front, the drones slung under the outer wing, the two nacelles riding side by side
/// off the trailing edge over the tail. From above: a wide arrowhead.
pub(super) fn build_b(b: &mut MeshBuilder) {
    const HULL: [[f32; 9]; 5] = [
        [6.4, 0.1, 1.6, 0.3, 1.75, 0.25, 2.0, 0.05, 2.1],
        [4.6, 0.8, 1.0, 1.4, 1.5, 1.2, 2.6, 0.4, 2.95],
        [2.0, 1.2, 0.7, 1.9, 1.5, 1.7, 3.0, 0.6, 3.4],
        [-3.0, 1.1, 0.8, 1.8, 1.55, 1.6, 2.95, 0.6, 3.3],
        [-5.2, 0.5, 1.6, 0.9, 1.9, 0.8, 2.6, 0.3, 2.75],
    ];
    const ROOT: (f32, f32, f32, f32, f32) = (1.6, 2.35, 4.4, -4.6, 0.8);
    const TIP: (f32, f32, f32, f32, f32) = (9.0, 2.2, 0.7, -1.6, 0.26);
    const PIVOT: Vec3 = Vec3::new(-5.6, 2.3, 3.2);
    let jet = Jet {
        ahead: 2.1,
        behind: 1.9,
        w: 0.7,
        h: 0.72,
    };
    if b.coarse() {
        b.set_vtol(crate::Vtol {
            pivots: [PIVOT.to_array(), [0.0; 3]],
            pairs: 1,
            nozzle: [jet.behind, jet.h * 0.46],
            fans: false,
        });
        coarse(
            b,
            &[
                [6.4, 0.0],
                [2.0, 1.9],
                [-5.2, 0.9],
                [-5.2, -0.9],
                [2.0, -1.9],
            ],
            &[[4.4, 0.0], [0.7, 9.0], [-1.6, 9.0], [-4.6, 0.0]],
            2.3,
            PIVOT,
        );
        return;
    }
    let fine = b.fine();
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.loft(&band(&HULL, 0, 1), true, true);
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.loft(&band(&HULL, 1, 2), true, true);
    b.loft(&band(&HULL[2..], 2, 3), false, true);
    b.paint(GLASS);
    b.loft(&band(&HULL[..3], 2, 3), true, false);
    b.mirror_y(|b| {
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        b.loft(&[section(ROOT), section(TIP)], true, true);
        // A canted winglet on each tip, dark-capped.
        let cant = Affine3A::from_translation(v3(0.0, TIP.0 - 0.05, TIP.1))
            * Affine3A::from_rotation_x(-0.35);
        b.with(cant, |b| {
            b.paint(PLATING).pattern(pattern::AIRFRAME);
            b.extrude_y(
                &[[0.4, 0.0], [-0.8, 1.5], [-1.6, 1.5], [-1.6, 0.0]],
                -0.07,
                0.07,
            );
            if b.fine() {
                b.paint(TEAM).pattern(pattern::TEAM_BAND);
                b.extrude_y(
                    &[[-0.55, 1.15], [-0.8, 1.5], [-1.6, 1.5], [-1.6, 1.15]],
                    -0.08,
                    0.08,
                );
            }
        });
        // The nacelle's mount: a stub reaching back off the body over the tail.
        b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
        b.beam(
            v3(-3.4, 1.3, 2.95),
            v3(-5.6, PIVOT.y - 0.75, PIVOT.z),
            v2(0.5, 0.5),
            v2(0.3, 0.36),
        );
        if b.fine() {
            vent(b, v3(0.2, 3.6, 2.62), v2(1.8, 0.5), 3, GLOW_AMBER);
        }
    });
    // The hopper hump and the owner's panel on it.
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.loft_z(
        &[
            [1.2, 0.0],
            [0.6, 0.9],
            [-3.0, 0.9],
            [-3.6, 0.0],
            [-3.0, -0.9],
            [0.6, -0.9],
        ],
        &[
            crate::builder::Section::new(3.25, 1.0),
            crate::builder::Section::new(3.8, 0.84),
        ],
    );
    if fine {
        vent(b, v3(-1.3, 0.0, 3.8), v2(2.4, 1.0), 5, GLOW_AMBER);
    }
    team_panel(b, v3(3.0, 0.0, 3.3), v2(1.1, 0.9));
    pylons(b, |y| wing_under(ROOT, TIP, y));
    jets(b, PIVOT, &jet);
}

// ---- C: crane -----------------------------------------------------------------

/// C: a long slim crane of a spine, a low straight wing with the drones under it and a
/// nacelle on a mount reaching aft off the middle of each wing, a T-tail far back.
/// From above: a long cross with a jet halfway out along each arm.
pub(super) fn build_c(b: &mut MeshBuilder) {
    const HULL: [[f32; 9]; 6] = [
        [7.4, 0.08, 2.0, 0.2, 2.15, 0.15, 2.4, 0.04, 2.5],
        [5.6, 0.5, 1.4, 0.9, 1.9, 0.8, 2.9, 0.3, 3.2],
        [3.4, 0.75, 1.2, 1.05, 1.8, 0.95, 3.1, 0.45, 3.45],
        [-2.2, 0.75, 1.25, 1.05, 1.85, 0.95, 3.1, 0.45, 3.45],
        [-5.0, 0.35, 1.9, 0.55, 2.2, 0.5, 2.95, 0.2, 3.1],
        [-8.2, 0.15, 2.45, 0.25, 2.55, 0.22, 2.85, 0.08, 2.95],
    ];
    const ROOT: (f32, f32, f32, f32, f32) = (0.9, 2.05, 1.8, -1.2, 0.4);
    const TIP: (f32, f32, f32, f32, f32) = (8.6, 1.85, 1.45, -0.85, 0.28);
    const PIVOT: Vec3 = Vec3::new(-2.5, 5.0, 2.95);
    let jet = Jet {
        ahead: 2.1,
        behind: 1.9,
        w: 0.62,
        h: 0.7,
    };
    if b.coarse() {
        b.set_vtol(crate::Vtol {
            pivots: [PIVOT.to_array(), [0.0; 3]],
            pairs: 1,
            nozzle: [jet.behind, jet.h * 0.46],
            fans: false,
        });
        coarse(
            b,
            &[
                [7.4, 0.0],
                [3.4, 1.0],
                [-8.2, 0.25],
                [-8.2, -0.25],
                [3.4, -1.0],
            ],
            &[[1.8, 0.0], [1.45, 8.6], [-0.85, 8.6], [-1.2, 0.0]],
            2.0,
            PIVOT,
        );
        return;
    }
    let fine = b.fine();
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.loft(&band(&HULL, 0, 1), true, true);
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.loft(&band(&HULL, 1, 2), true, true);
    b.paint(GLASS);
    b.loft(&band(&HULL[..2], 2, 3), true, false);
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.loft(&band(&HULL[1..], 2, 3), false, true);
    b.mirror_y(|b| {
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        b.loft(&[section(ROOT), section(TIP)], true, true);
        // The nacelle's mount: a fairing along the wing's top, running aft of it.
        b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
        b.beam(
            v3(0.9, PIVOT.y, 2.15),
            v3(PIVOT.x + 0.3, PIVOT.y, PIVOT.z - 0.55),
            v2(0.36, 0.3),
            v2(0.3, 0.3),
        );
        if b.fine() {
            b.paint(PLATING_DARK);
            b.cuboid(v3(PIVOT.x + 0.3, PIVOT.y, PIVOT.z - 0.4), v3(0.4, 0.3, 0.5));
            b.paint(TEAM).pattern(pattern::TEAM_BAND);
            b.loft(
                &[
                    section((7.6, 1.88, 1.5, -0.9, 0.3)),
                    section((8.1, 1.87, 1.48, -0.88, 0.29)),
                ],
                false,
                false,
            );
        }
    });
    // The T-tail: a fin on the end of the spine, the tailplane across its top.
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.extrude_y(
        &[[-6.4, 2.9], [-7.6, 5.0], [-8.4, 5.0], [-8.2, 2.9]],
        -0.1,
        0.1,
    );
    b.extrude_z(
        &[[-7.6, 2.6], [-8.6, 2.6], [-8.6, -2.6], [-7.6, -2.6]],
        4.9,
        5.06,
    );
    if fine {
        // Dark caps on the tailplane's tips.
        b.paint(PLATING_DARK);
        b.mirror_y(|b| b.cuboid(v3(-8.1, 2.45, 4.98), v3(1.05, 0.32, 0.2)));
    }
    // A salvage hopper behind the cab, amber slots in its lid.
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.loft_z(
        &[
            [2.6, 0.0],
            [2.1, 0.85],
            [-1.9, 0.85],
            [-2.4, 0.0],
            [-1.9, -0.85],
            [2.1, -0.85],
        ],
        &[
            crate::builder::Section::new(3.35, 1.0),
            crate::builder::Section::new(4.0, 0.86),
        ],
    );
    if fine {
        vent(b, v3(0.1, 0.0, 4.0), v2(2.6, 1.0), 5, GLOW_AMBER);
    }
    team_panel(b, v3(-3.6, 0.0, 3.2), v2(1.3, 0.6));
    pylons(b, |y| wing_under(ROOT, TIP, y));
    jets(b, PIVOT, &jet);
}

// ---- Salvage Drone -----------------------------------------------------------

/// The reclaim emitter at the tip of the claw: the blueprint's `emitter`.
const EMITTER: Vec3 = Vec3::new(1.15, 0.0, 0.35);
/// Where the two steering jets leave the drone (`models::aircraft_exhausts`).
pub(crate) const DRONE_NOZZLES: [[f32; 3]; 2] = [[-1.05, -0.5, 0.42], [-1.05, 0.5, 0.42]];
/// The lift fan's axis, up through the body (`part::ROTOR` turns about it).
const DRONE_FAN: Vec3 = Vec3::new(0.0, 0.0, 0.5);

/// The lugs on the drone's back the pylon's jaws close on.
fn lugs(b: &mut MeshBuilder, from_z: f32) {
    if !b.fine() {
        return;
    }
    b.paint(METAL);
    for lx in LUG_X {
        b.cuboid(
            v3(lx, 0.0, (from_z + LUG_TOP) * 0.5),
            v3(0.16, 0.16, LUG_TOP - from_z),
        );
    }
}

/// The lift fan's blades in their ring, turning (`part::ROTOR`).
fn fan(b: &mut MeshBuilder, z: f32, r: f32) {
    b.paint(ACCENT);
    b.prism(
        DRONE_FAN.with_z(z - 0.3),
        b.sides(10),
        r * 0.9,
        r * 0.9,
        0.3,
    );
    b.paint(METAL);
    b.prism(DRONE_FAN.with_z(z - 0.2), b.sides(6), 0.12, 0.09, 0.26);
    let blades = if b.fine() { 4 } else { 2 };
    b.with_part(part::ROTOR, |b| {
        b.paint(PLATING_DARK);
        for k in 0..blades {
            let a = k as f32 * std::f32::consts::TAU / 4.0;
            let out = v3(a.cos(), a.sin(), 0.0);
            b.beam(
                DRONE_FAN.with_z(z) + out * 0.1,
                DRONE_FAN.with_z(z - 0.03) + out * r * 0.85,
                v2(0.14, 0.03),
                v2(0.2, 0.03),
            );
        }
    });
}

/// The two steering jets at the back, each a little can with its blue slot.
fn steering_jets(b: &mut MeshBuilder) {
    b.mirror_y(|b| {
        let n = Vec3::from(DRONE_NOZZLES[1]);
        b.paint(METAL);
        b.cylinder_between(n + Vec3::X * 0.45, n, 0.13, 0.1, b.sides(6));
        if b.fine() {
            b.paint(GLOW);
            b.cylinder_between(n + Vec3::X * 0.02, n + Vec3::X * 0.01, 0.07, 0.07, 6);
        }
    });
}

/// The emitter head: a dark block with its amber lens.
fn emitter(b: &mut MeshBuilder) {
    b.paint(ACCENT);
    b.cuboid(EMITTER - Vec3::X * 0.16, v3(0.32, 0.36, 0.3));
    if b.fine() {
        b.paint(GLOW_AMBER);
        b.cuboid(EMITTER - Vec3::X * 0.03, v3(0.08, 0.22, 0.18));
    }
}

fn drone_coarse(b: &mut MeshBuilder) {
    b.paint(PLATING);
    b.prism(v3(0.0, 0.0, 0.3), 6, 0.9, 0.8, 0.5);
    b.paint(GLOW_AMBER);
    b.cuboid(EMITTER - Vec3::X * 0.15, v3(0.3, 0.3, 0.3));
}

/// A, the grapple: a flat faceted wedge with the fan through its middle, two claw arms
/// reaching down and forward to the emitter, the lugs on posts over the fan.
pub(super) fn drone(b: &mut MeshBuilder) {
    b.set_spinner_pivot(DRONE_FAN);
    if b.coarse() {
        drone_coarse(b);
        return;
    }
    let plan = [
        [0.95, 0.0],
        [0.55, 0.62],
        [-0.8, 0.72],
        [-1.05, 0.4],
        [-1.05, -0.4],
        [-0.8, -0.72],
        [0.55, -0.62],
    ];
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.loft_z(
        &plan,
        &[
            crate::builder::Section::new(0.3, 0.86),
            crate::builder::Section::new(0.45, 1.0),
        ],
    );
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.loft_z(
        &plan,
        &[
            crate::builder::Section::new(0.45, 1.0),
            crate::builder::Section::new(0.7, 0.9),
        ],
    );
    fan(b, 0.72, 0.5);
    lugs(b, 0.6);
    // The claw: two arms down to the emitter, tines either side of it.
    b.paint(PLATING_DARK);
    b.mirror_y(|b| {
        b.beam(
            v3(0.45, 0.32, 0.35),
            v3(EMITTER.x - 0.25, 0.2, EMITTER.z),
            v2(0.12, 0.14),
            v2(0.1, 0.12),
        );
        if b.fine() {
            b.paint(METAL);
            b.beam(
                v3(EMITTER.x - 0.1, 0.22, EMITTER.z - 0.05),
                v3(EMITTER.x + 0.2, 0.26, EMITTER.z - 0.25),
                v2(0.05, 0.08),
                v2(0.03, 0.05),
            );
        }
    });
    emitter(b);
    steering_jets(b);
    team_panel(b, v3(-0.8, 0.0, 0.7), v2(0.3, 0.6));
}

/// B, the store: a slim pod like an underwing store, hung by its lugs, the fan in a
/// ring saddled on its back, a chin emitter, cruciform tail fins round the jets.
pub(super) fn drone_b(b: &mut MeshBuilder) {
    b.set_spinner_pivot(DRONE_FAN);
    if b.coarse() {
        drone_coarse(b);
        return;
    }
    let c = v3(0.0, 0.0, 0.48);
    let ring = |x: f32, r: f32| -> Vec<Vec3> {
        (0..8)
            .map(|k| {
                let a = (k as f32 + 0.5) * std::f32::consts::TAU / 8.0;
                v3(x, c.y + a.cos() * r, c.z + a.sin() * r)
            })
            .collect()
    };
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    if b.fine() {
        b.loft(
            &[
                ring(1.05, 0.08),
                ring(0.8, 0.3),
                ring(0.4, 0.4),
                ring(-0.6, 0.38),
                ring(-0.95, 0.24),
            ],
            true,
            true,
        );
    } else {
        b.loft(
            &[ring(1.05, 0.12), ring(0.4, 0.4), ring(-0.95, 0.24)],
            true,
            true,
        );
    }
    // The fan ring on a saddle, and the lugs through it.
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.prism(v3(0.0, 0.0, 0.7), b.sides(10), 0.72, 0.72, 0.14);
    fan(b, 0.86, 0.62);
    lugs(b, 0.8);
    // The chin emitter on a short stalk.
    b.paint(PLATING_DARK);
    b.beam(
        v3(0.6, 0.0, 0.2),
        v3(EMITTER.x - 0.25, 0.0, EMITTER.z),
        v2(0.18, 0.14),
        v2(0.14, 0.12),
    );
    emitter(b);
    // Tail fins in an X, the steering jets between them.
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    let fins: &[(f32, f32)] = if b.fine() {
        &[(1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)]
    } else {
        &[(1.0, 1.0), (-1.0, 1.0)]
    };
    for &(s, t) in fins {
        b.beam(
            v3(-0.55, s * 0.2, c.z + t * 0.18),
            v3(-0.95, s * 0.5, c.z + t * 0.3),
            v2(0.04, 0.3),
            v2(0.04, 0.22),
        );
    }
    steering_jets(b);
    team_panel(b, v3(-0.45, 0.0, 0.84), v2(0.24, 0.3));
}

/// C, the crab: a round shell with the fan through it, four short legs folded under
/// it and the front pair closed on the emitter; the lugs on its back.
pub(super) fn drone_c(b: &mut MeshBuilder) {
    b.set_spinner_pivot(DRONE_FAN);
    if b.coarse() {
        drone_coarse(b);
        return;
    }
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.prism(v3(0.0, 0.0, 0.3), b.sides(12), 0.75, 0.95, 0.18);
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.prism(v3(0.0, 0.0, 0.48), b.sides(12), 0.95, 0.78, 0.2);
    fan(b, 0.72, 0.55);
    lugs(b, 0.62);
    // Legs: out, then down, the front pair meeting at the emitter.
    b.paint(METAL);
    b.mirror_y(|b| {
        for (x, reach) in [(0.45, 0.9), (-0.45, 0.85)] {
            let hip = v3(x, 0.55, 0.35);
            let knee = v3(x * 1.4, reach, 0.3);
            let foot = if x > 0.0 {
                v3(EMITTER.x - 0.2, 0.22, EMITTER.z - 0.1)
            } else {
                v3(x * 1.7, reach - 0.05, 0.06)
            };
            if b.fine() {
                b.beam(hip, knee, v2(0.1, 0.1), v2(0.09, 0.09));
                b.beam(knee, foot, v2(0.09, 0.09), v2(0.05, 0.05));
            } else {
                b.beam(hip, foot, v2(0.1, 0.1), v2(0.06, 0.06));
            }
        }
    });
    emitter(b);
    steering_jets(b);
    team_panel(b, v3(-0.5, 0.0, 0.68), v2(0.28, 0.5));
}
