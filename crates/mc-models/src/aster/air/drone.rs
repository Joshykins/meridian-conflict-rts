//! The Salvage Drone the commander's drone port fields: a small fan-lift grapple with one
//! reclaim emitter on a claw. It rides a pad on the commander's back between jobs.
use super::*;

/// The drone's two lugs over the fan, fore and aft of its middle, their tops this high
/// over its feet.
const LUG_X: [f32; 2] = [0.62, -0.62];
const LUG_TOP: f32 = 1.0;

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

/// The Salvage Drone, a grapple: a flat faceted wedge with the fan through its middle, two claw arms
/// reaching down and forward to the emitter, the lugs on posts over the fan.
pub(super) fn build(b: &mut MeshBuilder) {
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
