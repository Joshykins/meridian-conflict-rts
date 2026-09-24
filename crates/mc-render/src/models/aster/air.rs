//! Aster aircraft: a planform of their own for each role, paired jet nozzles.
//! +X is forward. Wings are built in plan view.

use glam::{Affine3A, Vec3};

use super::parts::*;
use crate::models::builder::{MeshBuilder, Section};
use crate::models::material::*;
use crate::models::{part, pattern, rig};

mod bastion;
pub mod capital;
mod courier;
pub use courier::{LAMPS as COURIER_LAMPS, LIFT_JETS as COURIER_LIFT_JETS, NOZZLES as COURIER_NOZZLES, RIG as COURIER_RIG};
pub use bastion::{LIFT_JETS as BASTION_LIFT_JETS, NOZZLES as BASTION_NOZZLES, RIG as BASTION_RIG, LAMPS as BASTION_LAMPS};
mod gannet;
mod kestrel;
pub(in crate::models) mod osprey;
mod peregrine;
mod raptor;
mod shrike;

pub use kestrel::{NACELLES as KESTREL_NACELLES, NOZZLES as KESTREL_NOZZLES};
pub use osprey::{
    CRADLES as OSPREY_CRADLES, DRONE_NOZZLES, HOLD_CEILING as OSPREY_HOLD_CEILING,
    NACELLES as OSPREY_NACELLES, NOZZLES as OSPREY_NOZZLES,
};

/// Loft rings for a hard-chined hull. Each station is x, then (half width, height)
/// pairs from the keel up to the spine; `from..=to` picks the pairs, mirrored across
/// the centre line.
fn band<const N: usize>(stations: &[[f32; N]], from: usize, to: usize) -> Vec<Vec<Vec3>> {
    stations
        .iter()
        .map(|s| {
            let half: Vec<[f32; 2]> = (from..=to).map(|k| [s[1 + 2 * k], s[2 + 2 * k]]).collect();
            half.iter()
                .map(|p| v3(s[0], p[0], p[1]))
                .chain(half.iter().rev().map(|p| v3(s[0], -p[0], p[1])))
                .collect()
        })
        .collect()
}

/// A plan outline drawn in toward its middle: an armour plate let into a panel.
fn inset(plan: &[[f32; 2]], scale: f32) -> Vec<[f32; 2]> {
    let n = plan.len() as f32;
    let [cx, cy] = plan.iter().fold([0.0, 0.0], |a, p| [a[0] + p[0] / n, a[1] + p[1] / n]);
    plan.iter().map(|p| [cx + (p[0] - cx) * scale, cy + (p[1] - cy) * scale]).collect()
}

fn wing(b: &mut MeshBuilder, plan: &[[f32; 2]], z: f32, thickness: f32) {
    b.paint(PLATING);
    b.extrude_z(plan, z, z + thickness);
}

/// Courier: compact spacecraft with an enclosed stern cargo bay.
pub fn light_transport(b: &mut MeshBuilder, _tech: u8) {
    courier::build(b);
}

/// Bastion: capital assault transport with a vehicle hangar and fusion drives.
pub fn lift_ship(b: &mut MeshBuilder, _tech: u8) {
    bastion::build(b);
}

/// The Gannet: gull-winged tech 2 torpedo bomber with a chin sonar ([`gannet`]).
pub fn torpedo_bomber(b: &mut MeshBuilder, _tech: u8) {
    gannet::build(b);
}

/// The Shrike: straight-winged tech 1 gun drone ([`shrike`]).
pub fn interceptor(b: &mut MeshBuilder, _tech: u8) {
    shrike::build(b);
}

/// Broad straight wing, enclosed nose, twin nacelles and an eight-bomb belly rack.
pub fn bomber(b: &mut MeshBuilder, _tech: u8) {
    if b.coarse() {
        b.paint(PLATING);
        b.frustum_open(
            v3(-0.1, 0.0, 1.1),
            v2(8.8, 1.9),
            v2(5.6, 1.1),
            1.2,
            v2(-0.4, 0.0),
        );
        b.mirror_y(|b| {
            b.face(&[
                v3(1.45, 0.8, 1.34),
                v3(0.95, 5.0, 1.34),
                v3(-1.35, 5.0, 1.34),
                v3(-1.65, 0.8, 1.34),
            ]);
            b.face(&[
                v3(-4.5, 0.65, 1.25),
                v3(-2.8, 0.65, 1.25),
                v3(-4.5, 0.65, 2.55),
            ]);
        });
        b.paint(TEAM);
        b.face(&[
            v3(-1.5, -0.4, 2.3),
            v3(0.3, -0.4, 2.3),
            v3(0.3, 0.4, 2.3),
            v3(-1.5, 0.4, 2.3),
        ]);
        b.paint(METAL);
        b.cuboid_open(v3(0.6, 0.0, 0.42), v3(2.3, 2.25, 0.1));
        return;
    }
    let plan = [
        [4.1, -0.42],
        [4.1, 0.42],
        [2.55, 0.88],
        [-2.85, 1.0],
        [-4.55, 0.5],
        [-4.55, -0.5],
        [-2.85, -1.0],
        [2.55, -0.88],
    ];
    b.paint(PLATING_DARK);
    b.loft_z(&plan, &[Section::new(0.32, 0.8), Section::new(1.0, 1.0)]);
    b.paint(PLATING);
    b.loft_z(
        &plan,
        &[
            Section::new(1.0, 1.0),
            Section::scaled(1.9, 0.86, 0.8),
            Section::scaled(2.22, 0.61, 0.5).shifted(-0.15, 0.0),
        ],
    );
    b.mirror_y(|b| {
        wing(
            b,
            &[
                [1.45, 0.8],
                [1.25, 4.65],
                [0.95, 5.0],
                [-1.35, 5.0],
                [-1.65, 0.8],
            ],
            1.1,
            0.24,
        );
        wing(
            b,
            &[[-3.05, 0.5], [-3.25, 2.05], [-4.4, 2.05], [-4.4, 0.5]],
            1.22,
            0.14,
        );
        b.paint(PLATING);
        b.extrude_y(
            &[[-4.4, 1.25], [-2.8, 1.25], [-3.85, 2.55], [-4.5, 2.55]],
            0.55,
            0.73,
        );
        if b.fine() {
            b.paint(PLATING_DARK);
            b.extrude_z(
                &[[-0.85, 2.9], [-0.7, 4.78], [-1.18, 4.78], [-1.38, 2.9]],
                1.345,
                1.38,
            );
            b.paint(TEAM);
            b.extrude_z(
                &[[0.8, 3.3], [0.8, 4.2], [0.3, 4.2], [0.3, 3.3]],
                1.35,
                1.39,
            );
            // Armoured nacelles: black intake forward, nozzle aft.
            b.paint(PLATING_DARK);
            b.cylinder_between(
                v3(-2.3, 2.35, 0.95),
                v3(1.3, 2.35, 0.95),
                0.43,
                0.36,
                b.sides(8),
            );
            b.paint(PLATING);
            b.beam(
                v3(-1.85, 2.35, 1.23),
                v3(1.12, 2.35, 1.23),
                v2(0.66, 0.17),
                v2(0.55, 0.17),
            );
            b.paint(ACCENT);
            b.cylinder_between(v3(1.31, 2.35, 0.95), v3(1.33, 2.35, 0.95), 0.28, 0.28, 6);
            b.paint(METAL);
            b.cylinder_between(
                v3(-2.25, 2.35, 0.95),
                v3(-2.65, 2.35, 0.95),
                0.34,
                0.27,
                b.sides(8),
            );
            b.paint(ACCENT);
            b.cylinder_between(v3(-2.65, 2.35, 0.95), v3(-2.67, 2.35, 0.95), 0.22, 0.22, 6);
        } else {
            b.paint(PLATING_DARK);
            b.cylinder_between(v3(-2.68, 2.35, 0.95), v3(1.3, 2.35, 0.95), 0.33, 0.36, 4);
        }
    });
    team_panel(b, v3(-0.75, 0.0, 2.23), v2(1.85, 0.72));
    b.paint(ACCENT);
    b.cuboid_open(v3(0.6, 0.0, 0.52), v3(2.8, 2.5, 0.18));
    if b.fine() {
        for x in [1.4, -0.2] {
            for y in [-1.05, -0.35, 0.35, 1.05] {
                b.paint(METAL);
                b.cylinder_between(v3(x - 0.3, y, 0.42), v3(x + 0.3, y, 0.42), 0.1, 0.1, 6);
                if b.fine() {
                    b.paint(PLATING_DARK);
                    b.cuboid(v3(x - 0.23, y, 0.42), v3(0.15, 0.27, 0.035));
                }
            }
        }
    } else {
        b.paint(METAL);
        b.cuboid_open(v3(0.6, 0.0, 0.42), v3(2.3, 2.25, 0.1));
    }
}

// Shared detailing; each airframe below defines its own planform and equipment.
fn fuselage(b: &mut MeshBuilder, length: f32, width: f32, height: f32) {
    let h = length * 0.5;
    b.paint(PLATING);
    b.loft_z(
        &[
            [h, 0.0],
            [h * 0.55, width],
            [-h * 0.8, width],
            [-h, 0.0],
            [-h * 0.8, -width],
            [h * 0.55, -width],
        ],
        &[
            Section::new(0.35, 0.75),
            Section::new(height * 0.48, 1.0),
            Section::scaled(height, 0.7, 0.5),
        ],
    );
    b.paint(GLASS);
    b.frustum(
        v3(h * 0.45, 0.0, height * 0.72),
        v2(length * 0.2, width),
        v2(length * 0.1, width * 0.6),
        height * 0.25,
        v2(-0.2, 0.0),
    );
    team_panel(
        b,
        v3(-h * 0.25, 0.0, height + 0.02),
        v2(length * 0.22, width * 0.65),
    );
}
fn engine(b: &mut MeshBuilder, x: f32, y: f32, z: f32, length: f32, r: f32) {
    b.paint(PLATING_DARK);
    b.cylinder_between(v3(x, y, z), v3(x + length, y, z), r, r * 0.82, b.sides(8));
    if !b.coarse() {
        b.paint(METAL);
        b.cylinder_between(
            v3(x - 0.35, y, z),
            v3(x, y, z),
            r * 0.8,
            r * 0.9,
            b.sides(8),
        );
        b.paint(ACCENT);
        b.cylinder_between(
            v3(x - 0.37, y, z),
            v3(x - 0.36, y, z),
            r * 0.63,
            r * 0.63,
            b.sides(8),
        );
        b.cylinder_between(
            v3(x + length + 0.01, y, z),
            v3(x + length + 0.02, y, z),
            r * 0.64,
            r * 0.64,
            b.sides(8),
        );
    }
}
fn tail(b: &mut MeshBuilder, x: f32, y: f32, z: f32, h: f32) {
    b.paint(PLATING);
    b.extrude_y(
        &[
            [x - 1.0, z],
            [x + 1.4, z],
            [x + 0.1, z + h],
            [x - 1.1, z + h],
        ],
        y - 0.1,
        y + 0.1,
    );
}
fn gun(b: &mut MeshBuilder, x: f32, y: f32, z: f32, blue: bool) {
    b.paint(METAL);
    b.cylinder_between(v3(x - 1.5, y, z), v3(x, y, z), 0.19, 0.12, b.sides(6));
    if blue && b.fine() {
        b.paint(GLOW);
        b.cuboid(v3(x - 0.4, y, z + 0.15), v3(0.65, 0.08, 0.07));
    }
}

/// AA gun whose barrel is socketed through the receiver, not laid beside it.
/// `muzzle` is the bore exit; +x is the bore.
fn seated_gun(b: &mut MeshBuilder, muzzle: Vec3) {
    let house = muzzle - Vec3::X * 0.72;
    b.paint(ACCENT);
    b.cuboid(house, v3(0.95, 0.58, 0.48));
    b.paint(PLATING);
    b.cuboid(house + Vec3::Z * 0.27, v3(0.78, 0.44, 0.08));
    b.paint(METAL);
    b.cylinder_between(muzzle - Vec3::X * 1.2, muzzle, 0.16, 0.1, b.sides(6));
    b.cylinder_between(
        muzzle - Vec3::X * 0.32,
        muzzle - Vec3::X * 0.2,
        0.24,
        0.22,
        b.sides(6),
    );
}
pub fn scout_air(b: &mut MeshBuilder, _: u8) {
    if !b.fine() {
        reduced_air(b, 5.8, 6.6, 1.7, 0.0, false, false);
        return;
    }
    fuselage(b, 5.8, 0.45, 1.5);
    b.mirror_y(|b| {
        wing(
            b,
            &[[1.3, 0.3], [0.2, 3.3], [-0.55, 3.3], [-0.8, 0.35]],
            0.65,
            0.1,
        );
        tail(b, -2.0, 0.45, 0.8, 0.9);
    });
    engine(b, -2.6, 0.0, 0.65, 1.5, 0.3);
    b.paint(GLASS);
    b.prism(v3(1.8, 0.0, 0.1), b.sides(8), 0.25, 0.3, 0.3);
}
pub fn rotor_gunship(b: &mut MeshBuilder, _: u8) {
    if !b.fine() {
        b.paint(PLATING);
        b.frustum_open(v3(0.0,0.0,0.8),v2(5.0,1.5),v2(3.5,0.8),1.4,v2(0.0,0.0));
        b.paint(PLATING_DARK);
        b.beam(v3(-1.8,0.0,1.0),v3(-5.0,0.0,1.7),v2(0.65,0.7),v2(0.25,0.35));
        b.face(&[v3(-5.3,0.0,1.2),v3(-3.2,0.0,1.2),v3(-4.1,0.0,2.7)]);
        b.set_spinner_pivot(v3(0.0,0.0,2.9));
        b.with_part(part::ROTOR, |b| {
            for y in [false,true] {
                if y { b.face(&[v3(-0.15,-5.2,3.0),v3(0.15,-5.2,3.0),v3(0.15,5.2,3.0),v3(-0.15,5.2,3.0)]); }
                else { b.face(&[v3(-5.2,-0.15,3.0),v3(5.2,-0.15,3.0),v3(5.2,0.15,3.0),v3(-5.2,0.15,3.0)]); }
            }
        });
        b.mirror_y(|b| {
            b.paint(METAL);
            b.cuboid_open(v3(0.65,1.7,0.65),v3(1.3,0.7,0.7));
            if b.mid() { b.cuboid_open(v3(0.0,0.9,0.1),v3(4.0,0.12,0.18)); }
        });
        b.set_turret_pivot(v3(2.1,0.0,0.5));
        b.set_arm_pivot(v3(2.1,0.0,0.5));
        b.with_part(part::TURRET, |b| b.with_limb(rig::ARM_GUN, |b| {
            b.cuboid_open(v3(2.65,0.0,0.5),v3(1.1,0.2,0.2));
        }));
        return;
    }
    fuselage(b, 5.0, 0.75, 2.2);
    b.paint(PLATING_DARK);
    b.beam(
        v3(-1.8, 0.0, 1.0),
        v3(-5.0, 0.0, 1.7),
        v2(0.65, 0.7),
        v2(0.25, 0.35),
    );
    tail(b, -4.2, 0.0, 1.2, 1.5);
    b.mirror_y(|b| {
        wing(
            b,
            &[[0.5, 0.5], [0.0, 2.1], [-1.0, 2.1], [-1.2, 0.5]],
            0.95,
            0.17,
        );
        b.paint(METAL);
        b.cuboid(v3(0.0, 0.9, 0.1), v3(4.0, 0.12, 0.18));
        b.cylinder_between(
            v3(0.0, 1.7, 0.65),
            v3(1.3, 1.7, 0.65),
            0.35,
            0.35,
            b.sides(8),
        );
    });
    b.set_spinner_pivot(v3(0.0, 0.0, 2.9));
    b.paint(METAL);
    b.prism(v3(0.0, 0.0, 2.0), 6, 0.15, 0.15, 1.0);
    b.with_part(part::ROTOR, |b| {
        b.paint(PLATING_DARK);
        b.cuboid(v3(0.0, 0.0, 3.0), v3(10.4, 0.3, 0.08));
        b.cuboid(v3(0.0, 0.0, 3.0), v3(0.3, 10.4, 0.08));
    });
    b.set_turret_pivot(v3(2.1, 0.0, 0.5));
    b.set_arm_pivot(v3(2.1, 0.0, 0.5));
    b.with_part(part::TURRET, |b| b.with_limb(rig::ARM_GUN, |b| gun(b, 3.2, 0.0, 0.5, false)));
}
pub fn support(b: &mut MeshBuilder, _: u8) {
    if !b.fine() {
        reduced_air(b, 12.0, 15.0, 3.5, 0.0, false, true);
        return;
    }
    fuselage(b, 12.0, 1.0, 2.7);
    b.mirror_y(|b| {
        wing(
            b,
            &[[2.5, 0.7], [0.0, 7.5], [-2.1, 7.5], [-2.8, 0.8]],
            1.0,
            0.2,
        );
        engine(b, -3.0, 3.5, 0.9, 3.4, 0.6);
        tail(b, -4.8, 0.8, 1.8, 1.7);
    });
    b.paint(METAL);
    b.prism(v3(-0.8, 0.0, 2.1), 6, 0.2, 0.2, 0.9);
    b.set_spinner_pivot(v3(-0.8, 0.0, 3.0));
    b.with_part(part::SPINNER, |b| {
        b.paint(PLATING_DARK);
        b.prism(v3(-0.8, 0.0, 3.0), b.sides(12), 2.4, 2.1, 0.4);
        b.paint(TEAM);
        b.cuboid(v3(-0.8, 0.0, 3.42), v3(3.5, 0.28, 0.05));
    });
    b.paint(GLOW);
    b.prism(v3(3.5, 0.0, 2.0), 6, 0.4, 0.25, 0.55);
}
/// The Osprey: the tech 2 reclaim carrier, four ducted lift fans and a drone hold ([`osprey`]).
pub fn carrier(b: &mut MeshBuilder, _: u8) {
    osprey::build(b);
}
/// The Salvage Drone the Osprey fields ([`osprey::drone`]).
pub fn drone(b: &mut MeshBuilder, _: u8) {
    osprey::drone(b);
}
/// The Kestrel: the tech 2 vector-thrust heavy gunship ([`kestrel`]).
pub fn gunship(b: &mut MeshBuilder, _: u8) {
    kestrel::build(b);
}
pub fn fortress(b: &mut MeshBuilder, _: u8) {
    if !b.fine() {
        reduced_air(b, 21.0, 25.0, 5.0, 1.0, false, false);
        return;
    }
    fuselage(b, 21.0, 1.8, 4.0);
    b.mirror_y(|b| {
        wing(
            b,
            &[[3.5, 1.2], [2.0, 12.5], [-2.0, 12.5], [-3.3, 1.2]],
            1.8,
            0.4,
        );
        // Charred leading edge and a black inner plate on the main wing.
        b.paint(ACCENT);
        b.extrude_z(
            &[[3.42, 1.35], [1.95, 12.35], [1.55, 12.2], [3.05, 1.45]],
            2.18,
            2.26,
        );
        b.extrude_z(
            &[[1.4, 2.4], [0.35, 10.6], [-1.15, 10.6], [-1.55, 2.4]],
            2.2,
            2.27,
        );
        b.paint(PLATING_DARK);
        for y in [4.2, 7.4, 10.6] {
            b.cuboid(v3(0.15, y, 2.24), v3(3.4, 0.14, 0.08));
        }
        wing(
            b,
            &[[-6.0, 1.0], [-7.0, 5.5], [-9.7, 5.5], [-9.2, 1.0]],
            2.1,
            0.2,
        );
        b.paint(ACCENT);
        b.extrude_z(
            &[[-6.15, 1.15], [-7.05, 5.25], [-7.35, 5.2], [-6.55, 1.2]],
            2.28,
            2.34,
        );
        for y in [5.0, 9.0] {
            engine(b, -3.8, y, 1.6, 6.0, 0.75);
        }
        tail(b, -8.5, 0.8, 2.5, 2.4);
        // Wing gun: pedestal into the receiver, barrel on the same axis as the muzzle.
        b.paint(PLATING);
        b.prism(v3(0.55, 3.0, 2.05), b.sides(8), 0.36, 0.28, 0.58);
        seated_gun(b, v3(1.4, 3.0, 2.8));
    });
    b.mirror_y(|b| {
        for y in [5.0, 9.0] {
            b.paint(PLATING);
            b.beam(v3(-2.8, y, 2.25), v3(1.9, y, 2.25), v2(1.2, 0.22), v2(0.9, 0.18));
            b.paint(METAL);
            b.cylinder_between(v3(2.15, y, 1.6), v3(2.45, y, 1.6), 0.78, 0.65, 8);
        }
        for y in [3.8, 7.4, 11.0] {
            b.paint(PLATING_DARK);
            b.cuboid(v3(-1.1, y, 2.23), v3(1.4, 1.8, 0.06));
            b.paint(TEAM);
            b.cuboid(v3(0.7, y, 2.24), v3(0.45, 1.4, 0.05));
        }
        for x in [-5.0, -3.0, -1.0, 1.0] {
            b.paint(PLATING_DARK);
            b.cuboid(v3(x, 1.4, 3.1), v3(1.55, 0.25, 0.6));
        }
    });
    b.paint(PLATING_DARK);
    b.cuboid(v3(-3.2, 0.0, 3.95), v3(3.6, 0.75, 0.5));
    b.paint(ACCENT);
    for x in [-6.5, -4.2, -1.6, 1.2, 3.4] {
        b.cuboid(v3(x, 0.0, 3.72), v3(0.18, 1.35, 0.12));
    }
    b.cuboid(v3(-1.5, 0.0, 3.78), v3(8.5, 0.22, 0.1));
    b.paint(GLASS);
    b.cuboid(v3(5.2, 0.0, 3.1), v3(0.7, 1.5, 0.35));
    b.paint(PLATING);
    b.prism(v3(1.05, 0.0, 3.85), b.sides(8), 0.42, 0.32, 0.8);
    seated_gun(b, v3(2.0, 0.0, 4.8));
    // Tail gun points aft. The yawed frame's origin is the trunnion, so the
    // barrel is authored along local +x and comes out at the muzzle.
    b.paint(PLATING);
    b.prism(v3(-9.2, 0.0, 2.15), b.sides(8), 0.4, 0.3, 0.5);
    b.yawed(v3(-8.45, 0.0, 2.8), std::f32::consts::PI, |b| {
        seated_gun(b, v3(1.55, 0.0, 0.0));
    });
    b.paint(ACCENT);
    b.cuboid(v3(0.0, 0.0, 0.35), v3(12.0, 2.4, 0.38));
    if b.fine() {
        for x in [-5.0, -3.5, -2.0, -0.5, 1.0, 2.5, 4.0] {
            b.paint(METAL);
            b.cuboid(v3(x, 0.0, 0.12), v3(0.45, 2.05, 0.16));
            b.paint(PLATING_DARK);
            b.cuboid(v3(x, 0.0, 0.22), v3(0.7, 0.12, 0.08));
        }
    }
}
/// The Peregrine: tech 2 missile interceptor, a needle with a trident's prongs ([`peregrine`]).
pub fn interceptor_t2(b: &mut MeshBuilder, _: u8) {
    peregrine::build(b);
}
/// The Raptor: tech 3 forward-swept lance fighter ([`raptor`]).
pub fn superiority(b: &mut MeshBuilder, _: u8) {
    raptor::build(b);
}
pub fn strategic(b: &mut MeshBuilder, _: u8) {
    if !b.fine() {
        reduced_air(b, 16.0, 30.0, 4.5, -5.0, false, false);
        return;
    }
    fuselage(b, 16.0, 1.5, 3.4);
    b.mirror_y(|b| {
        wing(
            b,
            &[
                [6.4, 0.5],
                [-4.3, 15.0],
                [-6.0, 15.0],
                [-3.4, 5.2],
                [-6.8, 1.0],
            ],
            1.1,
            0.32,
        );
        engine(b, -5.5, 2.2, 1.4, 4.0, 0.8);
        tail(b, -5.0, 2.8, 1.5, 2.5);
    });
    b.paint(ACCENT);
    b.cuboid(v3(0.2, 0.0, 0.3), v3(6.0, 2.4, 0.4));
}
/// The Thunderhead: an unmanned, hard-chined armoured tub built round its gun. The
/// Avenger's seven barrels run out of a channel between two armoured cheeks, well past
/// them, and swivel a little in their mount. A Talon rocket hangs under each wing.
pub fn assault(b: &mut MeshBuilder, _: u8) {
    if !b.fine() && !b.mid() {
        reduced_air(b, 20.0, 27.0, 5.5, 1.0, false, false);
        return;
    }
    // Hull stations, nose to tail: x, then (half width, height) at the keel, the
    // chine, the shoulder and the spine. Flat faces and hard edges all the way.
    const HULL: [[f32; 9]; 4] = [
        [3.2, 0.95, 0.8, 1.8, 1.8, 1.5, 3.3, 0.75, 3.85],
        [0.0, 1.05, 0.6, 2.1, 1.8, 1.8, 3.5, 0.95, 4.3],
        [-4.0, 0.9, 0.8, 1.9, 1.9, 1.6, 3.4, 0.85, 4.0],
        [-9.6, 0.4, 1.7, 0.8, 2.2, 0.7, 3.0, 0.3, 3.3],
    ];
    // White armour above the chines over a graphite belly.
    let band = |from: usize, to: usize| -> Vec<Vec<Vec3>> {
        HULL.iter()
            .map(|s| {
                let half: Vec<[f32; 2]> = (from..=to).map(|k| [s[1 + 2 * k], s[2 + 2 * k]]).collect();
                half.iter()
                    .map(|p| v3(s[0], p[0], p[1]))
                    .chain(half.iter().rev().map(|p| v3(s[0], -p[0], p[1])))
                    .collect()
            })
            .collect()
    };
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.loft(&band(1, 3), true, true);
    b.paint(PLATING_DARK);
    b.loft(&band(0, 1), true, true);
    team_panel(b, v3(-2.2, 0.0, 4.2), v2(3.2, 0.9));

    // The gun bay: a graphite floor and breech housing, a white roof bridging the cheeks.
    b.paint(PLATING_DARK);
    b.block(v3(1.2, -1.3, 0.75), v3(6.6, 1.3, 1.0));
    if b.fine() {
        b.chamfered_box(v3(3.6, 0.0, 1.95), v3(4.6, 1.9, 1.7), 0.25);
    } else {
        b.cuboid(v3(3.6, 0.0, 1.95), v3(4.6, 1.9, 1.7));
    }
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.extrude_z(&[[6.9, -1.35], [6.9, 1.35], [1.2, 1.35], [1.2, -1.35]], 2.95, 3.3);
    if b.fine() {
        // The drone's eye: a sensor window let into the roof's front edge.
        b.paint(GLASS);
        b.plate(v3(6.35, 0.0, 3.3), v2(0.7, 1.7), 0.06, 0.03);
        glow_strip(b, v3(3.2, 0.0, 3.3), v2(3.0, 0.08), GLOW_ORANGE);
    }
    b.mirror_y(|b| {
        // An armoured cheek down each side of the gun, chisel-nosed.
        let cheek = |x: f32, inner: f32, outer: f32, bottom: f32, top: f32| -> Vec<Vec3> {
            let mid = (bottom + top) * 0.5;
            vec![
                v3(x, inner, bottom),
                v3(x, outer - 0.35, bottom),
                v3(x, outer, mid),
                v3(x, outer - 0.35, top),
                v3(x, inner, top),
            ]
        };
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        b.loft(
            &[
                cheek(1.0, 1.0, 2.3, 0.7, 3.45),
                cheek(6.6, 1.3, 2.3, 0.85, 3.05),
                cheek(9.4, 1.35, 1.95, 1.45, 2.35),
            ],
            true,
            true,
        );
        if b.fine() {
            b.paint(PLATING_DARK);
            b.beam(v3(9.35, 1.65, 1.5), v3(6.6, 1.95, 0.9), v2(0.5, 0.18), v2(0.7, 0.2));
        }
    });

    // The Avenger, on a mount that yaws about its breech (the sim's `ASSAULT_GUN_PIVOT_X`).
    b.set_turret_pivot(v3(7.0, 0.0, GUN_Z));
    b.with_part(part::TURRET, |b| {
        b.paint(PLATING_DARK);
        b.cylinder_between(v3(5.9, 0.0, GUN_Z), v3(7.5, 0.0, GUN_Z), 0.72, 0.72, b.sides(8));
        b.paint(ACCENT);
        b.cylinder_between(v3(7.4, 0.0, GUN_Z), v3(8.3, 0.0, GUN_Z), 0.74, 0.66, b.sides(8));
        // Seven barrels in a spinning cluster, clamped twice along their length.
        b.with_spin(v3(8.3, 0.0, GUN_Z), |b| {
            b.paint(METAL);
            b.cylinder_between(v3(8.2, 0.0, GUN_Z), v3(8.9, 0.0, GUN_Z), 0.6, 0.6, b.sides(8));
            if b.fine() {
                for i in 0..7 {
                    let a = i as f32 * std::f32::consts::TAU / 7.0;
                    let off = v3(0.0, a.cos() * 0.4, a.sin() * 0.4);
                    b.cylinder_between(v3(8.8, 0.0, GUN_Z) + off, v3(AVENGER_MUZZLE, 0.0, GUN_Z) + off, 0.11, 0.1, 5);
                }
                b.cylinder_between(v3(AVENGER_MUZZLE - 0.02, 0.0, GUN_Z), v3(AVENGER_MUZZLE, 0.0, GUN_Z), 0.16, 0.16, 6);
            } else {
                b.cylinder_between(v3(8.8, 0.0, GUN_Z), v3(AVENGER_MUZZLE, 0.0, GUN_Z), 0.5, 0.48, 7);
            }
            b.paint(ACCENT);
            for (x, r) in [(10.7, 0.6), (12.6, 0.58)] {
                b.cylinder_between(v3(x, 0.0, GUN_Z), v3(x + 0.35, 0.0, GUN_Z), r, r, b.sides(7));
            }
            // The hot ring at the muzzles.
            b.paint(GLOW_ORANGE);
            b.cylinder_between(
                v3(AVENGER_MUZZLE - 0.3, 0.0, GUN_Z),
                v3(AVENGER_MUZZLE - 0.18, 0.0, GUN_Z),
                0.56,
                0.56,
                b.sides(7),
            );
        });
    });

    b.mirror_y(|b| {
        // Thick armoured wing, swept a little and set well forward, with a cropped tip.
        let plan = [[4.2, 1.6], [1.5, 12.6], [0.6, 13.6], [-2.4, 13.6], [-3.3, 12.8], [-1.5, 1.6]];
        b.paint(PLATING_DARK);
        b.extrude_z(&plan, 1.55, 1.75);
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        b.extrude_z(&plan, 1.75, 2.05);
        // The wing's owner stripe, out near the tip.
        b.paint(TEAM).pattern(pattern::TEAM_BAND);
        b.plate(v3(-0.6, 10.6, 2.05), v2(3.6, 1.2), 0.04, 0.02);
        if b.fine() {
            // Leading-edge armour strip, dark.
            b.paint(PLATING_DARK);
            b.beam(v3(4.15, 1.9, 1.8), v3(1.55, 12.5, 1.8), v2(0.5, 0.5), v2(0.4, 0.4));
            glow_strip(b, v3(-0.9, 13.62, 1.8), v2(1.4, 0.06), GLOW_ORANGE);
        }
        // A pylon under each wing: a dark blade, the Talon slung under it.
        b.paint(PLATING_DARK);
        b.extrude_y(&[[1.9, 1.55], [-1.5, 1.55], [-0.9, 0.85], [1.4, 0.85]], 6.6, 6.9);
        let rail = v3(0.3, TALON.y, TALON.z);
        b.paint(PLATING);
        if !b.fine() {
            b.cylinder_between(rail + Vec3::X * -1.7, TALON, 0.24, 0.1, 5);
        } else {
            b.cylinder_between(rail + Vec3::X * -1.7, rail + Vec3::X * 1.0, 0.24, 0.24, 6);
            b.paint(METAL);
            b.cylinder_between(rail + Vec3::X * 1.0, TALON, 0.24, 0.05, 6);
            b.paint(GLOW_ORANGE);
            b.cylinder_between(rail + Vec3::X * 0.55, rail + Vec3::X * 0.7, 0.25, 0.25, 6);
            b.paint(ACCENT);
            for k in 0..4 {
                let a = (k as f32 + 0.5) * std::f32::consts::FRAC_PI_2;
                let out = v3(0.0, a.cos(), a.sin());
                b.beam(rail + Vec3::X * -1.2 + out * 0.3, rail + Vec3::X * -1.65 + out * 0.45, v2(0.04, 0.35), v2(0.04, 0.2));
            }
        }

        // Engine nacelle, hexagonal, high on the tail: faceted, not round.
        let (ey, ez) = (3.4, 3.6);
        let hex = |x: f32, r: f32| -> Vec<Vec3> {
            (0..6)
                .map(|k| {
                    let a = (k as f32 + 0.5) * std::f32::consts::TAU / 6.0;
                    v3(x, ey + a.cos() * r, ez + a.sin() * r * 0.9)
                })
                .collect()
        };
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        if b.fine() {
            b.loft(&[hex(-7.0, 1.05), hex(-5.2, 1.3), hex(-1.4, 1.3), hex(-0.6, 1.15)], true, true);
            b.paint(METAL);
            b.loft(&[hex(-7.4, 0.8), hex(-6.95, 0.95)], true, true);
            b.paint(ACCENT);
            b.loft(&[hex(-0.55, 0.98), hex(-0.5, 0.98)], true, true);
        } else {
            b.loft(&[hex(-7.4, 1.0), hex(-0.6, 1.25)], true, true);
        }
        // Pylon from the spine to the nacelle.
        b.paint(PLATING_DARK);
        b.extrude_y(&[[-5.8, 3.55], [-1.8, 3.55], [-2.4, 3.9], [-5.4, 3.9]], 0.6, ey - 0.95);

        // Tailplane with a canted fin standing on each end.
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        b.extrude_z(&[[-7.0, 0.3], [-7.6, 5.2], [-9.9, 5.2], [-9.9, 0.3]], 2.75, 3.1);
        // Canted outward, standing on the tailplane's tip.
        let cant = glam::Affine3A::from_translation(v3(0.0, 5.0, 3.0))
            * glam::Affine3A::from_rotation_x(-0.22);
        b.with(cant, |b| {
            b.extrude_y(&[[-10.1, 0.0], [-7.1, 0.0], [-8.4, 2.9], [-9.9, 2.9]], -0.14, 0.14);
            if b.fine() {
                b.paint(PLATING_DARK);
                b.extrude_y(&[[-9.95, 2.3], [-8.15, 2.3], [-8.4, 2.9], [-9.9, 2.9]], -0.16, 0.16);
            }
        });
    });
}

/// Nose of the Talon rocket under the (left) wing: the weapon's muzzle.
const TALON: Vec3 = Vec3::new(2.0, 6.75, 0.55);
/// Height of the Avenger's bore above the model's origin.
const GUN_Z: f32 = 1.9;
/// Where its barrels end, forward of the origin: long, well clear of the nose.
const AVENGER_MUZZLE: f32 = 14.0;

fn reduced_air(
    b: &mut MeshBuilder,
    length: f32,
    span: f32,
    height: f32,
    sweep: f32,
    rotor: bool,
    radar: bool,
) {
    b.paint(PLATING);
    b.frustum_open(
        v3(0.0, 0.0, height * 0.25),
        v2(length, span * 0.2),
        v2(length * 0.6, span * 0.1),
        height * 0.45,
        v2(-length * 0.1, 0.0),
    );
    if rotor {
        b.set_spinner_pivot(v3(0.0, 0.0, height * 0.95));
        b.with_part(part::ROTOR, |b| {
            b.paint(METAL);
            b.face(&[
                v3(-span * 0.5, -0.2, height * 0.95),
                v3(span * 0.5, -0.2, height * 0.95),
                v3(span * 0.5, 0.2, height * 0.95),
                v3(-span * 0.5, 0.2, height * 0.95),
            ]);
            b.face(&[
                v3(-0.2, -span * 0.5, height * 0.95),
                v3(0.2, -span * 0.5, height * 0.95),
                v3(0.2, span * 0.5, height * 0.95),
                v3(-0.2, span * 0.5, height * 0.95),
            ]);
        });
    } else {
        b.mirror_y(|b| {
            b.face(&[
                v3(length * 0.22, span * 0.06, height * 0.4),
                v3(sweep, span * 0.5, height * 0.4),
                v3(sweep - length * 0.18, span * 0.5, height * 0.4),
                v3(-length * 0.2, span * 0.06, height * 0.4),
            ]);
            b.face(&[
                v3(-length * 0.45, span * 0.05, height * 0.5),
                v3(-length * 0.25, span * 0.05, height * 0.5),
                v3(-length * 0.45, span * 0.05, height),
            ]);
        });
    }
    b.paint(TEAM);
    b.face(&[
        v3(-length * 0.1, -0.2, height * 0.71),
        v3(length * 0.1, -0.2, height * 0.71),
        v3(length * 0.1, 0.2, height * 0.71),
        v3(-length * 0.1, 0.2, height * 0.71),
    ]);
    if radar {
        b.paint(PLATING_DARK);
        b.set_spinner_pivot(v3(-0.8, 0.0, height * 0.85));
        b.with_part(part::SPINNER, |b| {
            b.cuboid_open(v3(-0.8, 0.0, height * 0.9), v3(4.0, 4.0, 0.3))
        });
    }
    if b.mid() {
        b.paint(GLASS);
        b.cuboid_open(
            v3(length * 0.2, 0.0, height * 0.7),
            v3(length * 0.16, span * 0.07, height * 0.15),
        );
        if !rotor {
            b.mirror_y(|b| {
                b.paint(PLATING_DARK);
                b.cuboid_open(
                    v3(-length * 0.2, span * 0.15, height * 0.35),
                    v3(length * 0.25, span * 0.08, height * 0.25),
                );
            });
        }
    }
}
