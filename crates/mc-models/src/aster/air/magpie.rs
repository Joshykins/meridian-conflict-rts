//! Magpie: the tech 1 salvage drone, a small swept-wing tilt-engine aircraft with one
//! reclaim beam. It hangs low over a wreck field on two engine pods set half way out along
//! its wings (`part::VTOL_FRONT`,
//! stood up to hover and leaned with its speed) and works it from the air the way the
//! Gleaner works it from the road: the Cradle head (`reclaimers::cradle_head`) slung under
//! its belly on a yoke of its own (weapon slot 0, `reclaimer.heads[0]` in air.ron),
//! turning full circle and pitching from a little above level to straight down.
//!
//! Authored at blueprint scale (radius 3.0, height 1.9): model metres are unit metres.

use super::super::reclaimers::cradle_head;
use super::*;

/// The head's trunnion (the unit file's head `pivot`). Its mouth, the beam's emitter, is
/// 1.55 x [`HEAD`] ahead of it.
const PIVOT: Vec3 = Vec3::new(0.7, 0.0, 0.35);
/// The head's scale (1 for a 2 m head).
const HEAD: f32 = 0.55;
/// The underside of the airframe; the yoke's slewing ring hangs under it.
const BELLY: f32 = 1.1;
/// Where the wing sits on the body, and how thick it is.
const WING_Z: f32 = BELLY + 0.3;
const WING_T: f32 = 0.12;

/// The engine pod's pivot on the left (the right mirrors), half way out along the wing in
/// a gap between the inner and outer panels; how far the pod reaches ahead of and behind
/// it, and its radius.
const POD: Vec3 = Vec3::new(-0.55, 1.6, WING_Z + WING_T * 0.5);
const POD_AHEAD: f32 = 0.85;
const POD_BEHIND: f32 = 0.75;
const POD_R: f32 = 0.28;
/// The clear space either side of the pod, so it can stand up to hover.
const POD_GAP: f32 = POD_R + 0.04;

/// The swept wing, left side: from the root (y [`ROOT`]) to the tip (y [`TIP`]).
const ROOT: f32 = 0.7;
const TIP: f32 = 2.5;
/// The leading and trailing edges' x at `y` out along the wing.
fn lead(y: f32) -> f32 {
    0.6 - 0.733 * (y - ROOT)
}
fn trail(y: f32) -> f32 {
    -0.95 - 0.111 * (y - ROOT)
}

pub(super) fn build(b: &mut MeshBuilder) {
    b.set_vtol(crate::Vtol {
        pivots: [POD.to_array(), [0.0; 3]],
        pairs: 1,
        nozzle: [POD_BEHIND, POD_R * 0.8],
        fans: false,
    });
    if b.coarse() {
        coarse(b);
        return;
    }
    body(b);
    b.mirror_y(|b| {
        panel(b, ROOT, POD.y - POD_GAP);
        panel(b, POD.y + POD_GAP, TIP);
        // The trunnion the pod turns on, across the gap.
        b.paint(METAL);
        b.cylinder_between(
            v3(POD.x, POD.y - POD_GAP - 0.05, POD.z),
            v3(POD.x, POD.y + POD_GAP + 0.05, POD.z),
            0.08,
            0.08,
            b.sides(6),
        );
        b.with_part(part::VTOL_FRONT, jet_pod);
        // Twin fins at the tail.
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        b.extrude_y(
            &[
                [-1.1, BELLY + 0.5],
                [-1.75, BELLY + 0.5],
                [-1.95, BELLY + 1.15],
                [-1.65, BELLY + 1.15],
            ],
            0.32,
            0.4,
        );
    });
    team_panel(b, v3(-0.45, -1.0, WING_Z + WING_T), v2(0.4, 0.4));
    turret(b);
}

/// One panel of the left wing from `y0` out to `y1`, its dark leading edge.
fn panel(b: &mut MeshBuilder, y0: f32, y1: f32) {
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.extrude_z(
        &[
            [lead(y0), y0],
            [lead(y1), y1],
            [trail(y1), y1],
            [trail(y0), y0],
        ],
        WING_Z,
        WING_Z + WING_T,
    );
    if b.fine() {
        let z = WING_Z + WING_T * 0.5;
        b.paint(PLATING_DARK);
        b.beam(
            v3(lead(y0) - 0.06, y0 + 0.02, z),
            v3(lead(y1) - 0.06, y1 - 0.02, z),
            v2(0.14, WING_T * 1.1),
            v2(0.12, WING_T * 1.1),
        );
    }
}

/// The fuselage: a faceted wedge, its beak over the head, a sensor slot across it, a
/// glazed spine.
fn body(b: &mut MeshBuilder) {
    let plan = [
        [1.9, 0.0],
        [1.0, 0.7],
        [-1.2, 0.8],
        [-1.75, 0.45],
        [-1.75, -0.45],
        [-1.2, -0.8],
        [1.0, -0.7],
    ];
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.loft_z(
        &plan,
        &[Section::new(BELLY, 0.86), Section::new(BELLY + 0.22, 1.0)],
    );
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.loft_z(
        &plan,
        &[
            Section::new(BELLY + 0.22, 1.0),
            Section::new(BELLY + 0.6, 0.78),
        ],
    );
    if b.fine() {
        b.paint(GLOW_AMBER);
        b.cuboid(v3(1.55, 0.0, BELLY + 0.3), v3(0.06, 0.45, 0.08));
        b.paint(GLASS);
        b.beam(
            v3(0.2, 0.0, BELLY + 0.62),
            v3(1.05, 0.0, BELLY + 0.5),
            v2(0.45, 0.04),
            v2(0.3, 0.04),
        );
        b.paint(ACCENT);
        b.beam(
            v3(-1.5, 0.0, BELLY + 0.58),
            v3(-0.1, 0.0, BELLY + 0.64),
            v2(0.18, 0.06),
            v2(0.24, 0.06),
        );
    }
}

fn coarse(b: &mut MeshBuilder) {
    b.paint(PLATING);
    b.loft_z(
        &[[1.9, 0.0], [-1.75, 0.7], [-1.75, -0.7]],
        &[Section::new(BELLY, 1.0), Section::new(BELLY + 0.55, 0.85)],
    );
    // One slab across both wings, tip to tip.
    let mid = (lead(ROOT) + trail(ROOT) + lead(TIP) + trail(TIP)) * 0.25;
    let chord = (lead(ROOT) - trail(ROOT) + lead(TIP) - trail(TIP)) * 0.5;
    let z = WING_Z + WING_T * 0.5;
    b.beam(
        v3(mid, -TIP, z),
        v3(mid, TIP, z),
        v2(chord, WING_T),
        v2(chord, WING_T),
    );
    b.mirror_y(|b| {
        b.with_part(part::VTOL_FRONT, |b| {
            b.paint(PLATING_DARK);
            b.cylinder_between(
                POD + Vec3::X * POD_AHEAD,
                POD - Vec3::X * POD_BEHIND,
                POD_R,
                POD_R * 0.8,
                3,
            );
        });
    });
    b.paint(PLATING);
    b.cuboid(PIVOT + Vec3::X * 0.2, v3(0.9, 0.6, 0.45));
}

/// A jet pod about its pivot, lying along +x: a round body, the dark inlet, a gunmetal
/// nozzle with its blue mouth facing aft.
fn jet_pod(b: &mut MeshBuilder) {
    let (p, r) = (POD, POD_R);
    let sides = b.sides(8);
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.cylinder_between(
        p + Vec3::X * POD_AHEAD,
        p - Vec3::X * (POD_BEHIND - 0.3),
        r * 0.92,
        r,
        sides,
    );
    if b.fine() {
        b.paint(ACCENT);
        b.cylinder_between(
            p + Vec3::X * (POD_AHEAD + 0.02),
            p + Vec3::X * (POD_AHEAD - 0.04),
            r * 0.75,
            r * 0.75,
            sides,
        );
    }
    b.paint(METAL);
    b.cylinder_between(
        p - Vec3::X * (POD_BEHIND - 0.3),
        p - Vec3::X * POD_BEHIND,
        r,
        r * 0.8,
        sides,
    );
    b.paint(GLOW);
    b.cylinder_between(
        p - Vec3::X * (POD_BEHIND - 0.01),
        p - Vec3::X * (POD_BEHIND + 0.01),
        r * 0.62,
        r * 0.62,
        sides,
    );
    if b.fine() {
        b.paint(PLATING_DARK);
        b.beam(
            p + v3(POD_AHEAD - 0.2, 0.0, r * 0.9),
            p + v3(-POD_BEHIND + 0.35, 0.0, r * 0.9),
            v2(0.08, 0.06),
            v2(0.08, 0.06),
        );
    }
}

/// The Cradle hung under the belly: a fixed fairing, then gun house 0 with its slewing
/// ring, two long cheeks down past the trunnion and the head pitching between them. The
/// cheeks are long so the charge pack clears the ring when the head looks straight down.
fn turret(b: &mut MeshBuilder) {
    let p = PIVOT;
    let ring = BELLY - 0.14;
    b.paint(PLATING_DARK);
    b.prism(v3(p.x, 0.0, ring + 0.1), b.sides(10), 0.5, 0.56, 0.06);
    b.with_house(0, p, 0.0, |b| {
        b.paint(METAL);
        b.prism(v3(p.x, 0.0, ring), b.sides(12), 0.5, 0.5, 0.1);
        b.mirror_y(|b| {
            b.paint(PLATING).pattern(pattern::AIRFRAME);
            b.extrude_y(
                &[
                    [p.x - 0.28, ring],
                    [p.x + 0.2, ring],
                    [p.x + 0.2, p.z + 0.1],
                    [p.x + 0.06, p.z - 0.2],
                    [p.x - 0.14, p.z - 0.2],
                    [p.x - 0.26, p.z - 0.04],
                ],
                0.36,
                0.46,
            );
            if b.fine() {
                b.paint(METAL);
                b.cylinder_between(v3(p.x, 0.3, p.z), v3(p.x, 0.5, p.z), 0.1, 0.09, 8);
            }
        });
        b.with_recoil(|b| {
            cradle_head(b, p, HEAD);
        });
    });
}
