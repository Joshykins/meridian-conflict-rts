//! Amphibious emplacements and tracked AA share recognisable weapon assemblies.
use super::parts::*;
mod gnat;
mod squall;
mod sunder;
use crate::models::builder::MeshBuilder;
use crate::models::{material::*, part, rig};

fn mount(b: &mut MeshBuilder, tier: u8, z: f32) {
    b.set_turret_pivot(v3(0.0, 0.0, z));
    b.set_arm_pivot(v3(0.0, 0.0, z));
    b.with_part(part::TURRET, |b| {
        b.paint(PLATING_DARK);
        b.prism(v3(0.0, 0.0, z - 0.9), b.sides(10), 2.2, 1.8, 1.1);
        b.paint(PLATING);
        b.cuboid(v3(-0.5, 0.0, z + 0.35), v3(3.0, 2.5, 1.3));
        b.with_limb(rig::ARM_GUN, |b| {
            b.paint(METAL);
            let end = if tier == 1 {
                5.5
            } else if tier == 2 {
                6.0
            } else {
                8.0
            };
            if tier == 1 {
                rotary(b, z, end);
            } else if tier == 2 {
                b.cylinder_between(v3(0.8, 0.0, z), v3(end, 0.0, z), 0.52, 0.29, b.sides(8));
                b.paint(METAL);
                b.cuboid(v3(end - 0.15, 0.0, z), v3(0.55, 0.95, 0.6));
            } else {
                b.cuboid(v3(3.6, 0.0, z), v3(6.5, 1.15, 1.0));
                b.paint(PLATING);
                b.mirror_y(|b| b.cuboid(v3(2.5, 0.85, z), v3(4.8, 0.45, 1.6)));
                b.paint(GLOW);
                b.cuboid(v3(5.0, 0.0, z + 0.52), v3(4.6, 0.45, 0.08));
                b.cuboid(v3(7.9, 0.0, z), v3(0.1, 0.6, 0.6));
                if b.fine() {
                    b.mirror_y(|b| {
                        for x in [0.7, 1.6, 2.5, 3.4] {
                            b.paint(METAL);
                            b.cuboid(v3(x, 1.12, z), v3(0.3, 0.4, 1.8));
                        }
                    });
                }
            }
        });
        team_panel(b, v3(-0.5, 0.0, z + 1.02), v2(1.4, 1.3));
    });
}
/// The Sparrow's gun: a receiver on the trunnion, six barrels in clamps that spin
/// about the bore line, a drum feeding it from the left and the ejection port on
/// the right, where the casings come out.
fn rotary(b: &mut MeshBuilder, z: f32, end: f32) {
    let axis = v3(0.0, 0.0, z);
    b.paint(ACCENT);
    b.cylinder_between(v3(0.3, 0.0, z), v3(1.45, 0.0, z), 0.56, 0.5, b.sides(10));
    if b.fine() {
        b.paint(PLATING_DARK);
        b.cuboid(v3(0.95, -0.5, z + 0.1), v3(0.55, 0.12, 0.3));
        // The feed: a drum on the turret's left side and the chute to the receiver.
        b.paint(PLATING);
        b.cylinder_between(v3(-0.4, 1.3, z - 0.2), v3(-0.4, 2.05, z - 0.2), 0.7, 0.7, b.sides(10));
        b.paint(METAL);
        b.beam(v3(-0.1, 1.35, z), v3(0.8, 0.4, z + 0.05), v2(0.3, 0.26), v2(0.26, 0.22));
    }
    b.with_spin(axis, |b| {
        b.paint(METAL);
        if b.fine() {
            for i in 0..6 {
                let a = i as f32 * std::f32::consts::TAU / 6.0;
                let off = v3(0.0, a.cos() * 0.3, a.sin() * 0.3);
                b.cylinder_between(v3(1.45, 0.0, z) + off, v3(end, 0.0, z) + off * 0.85, 0.1, 0.085, 6);
            }
            b.paint(PLATING_DARK);
            for (x, r) in [(1.5, 0.47), (3.2, 0.43), (end - 0.35, 0.4)] {
                b.cylinder_between(v3(x, 0.0, z), v3(x + 0.22, 0.0, z), r, r, b.sides(8));
            }
        } else {
            // From further off the barrels read as one fat tube.
            b.cylinder_between(v3(1.45, 0.0, z), v3(end, 0.0, z), 0.42, 0.36, 6);
        }
    });
}
fn platform(b: &mut MeshBuilder, radius: f32) {

    b.paint(PLATING_DARK);
    b.plate(v3(0.0, 0.0, 0.1), v2(radius * 1.7, radius * 1.7), 1.0, 1.2);
    b.mirror_y(|b| {
        b.paint(PLATING);
        b.chamfered_box(
            v3(0.0, radius * 0.62, 1.1),
            v3(radius * 1.5, radius * 0.35, 2.0),
            0.5,
        );
        if !b.coarse() {
            b.paint(METAL);
            b.cuboid(v3(0.0, radius * 0.62, 2.15), v3(radius, 0.5, 0.18));
        }
    });
}
pub fn gun(b: &mut MeshBuilder, _: u8) {
    if !b.fine() {
        reduced_aa(b, 6.0, 6.0);
        return;
    }
    platform(b, 5.5);
    b.paint(METAL);
    b.prism(v3(0.0, 0.0, 1.0), b.sides(8), 1.5, 1.2, 4.3);
    mount(b, 1, 6.0);
}
pub fn array(b: &mut MeshBuilder, _: u8) {
    if b.fine() {
        platform(b, 9.5);
    } else {
        b.paint(PLATING_DARK);
        if b.coarse() {
            b.decal(v3(0.0, 0.0, 1.3), v2(16.15, 16.15));
        } else {
            b.cuboid_open(v3(0.0, 0.0, 0.7), v3(16.15, 16.15, 1.2));
        }
        if b.mid() {
            b.mirror_y(|b| {
                b.paint(PLATING);
                b.cuboid_open(v3(0.0, 5.89, 1.1), v3(14.25, 3.325, 2.0));
            });
        }
    }
    b.paint(PLATING);
    b.frustum(
        v3(0.0, 0.0, 1.0),
        v2(11.0, 11.0),
        v2(9.0, 9.0),
        4.7,
        v2(0.0, 0.0),
    );
    if !b.fine() {
        b.paint(METAL);
        b.cuboid_open(v3(0.0, 0.0, 6.2), v3(7.6, 7.6, 1.0));
    }
    for x in [-3.0, -1.0, 1.0, 3.0] {
        for y in [-3.0, -1.0, 1.0, 3.0] {
            if b.fine() {
                b.paint(METAL);
                b.cuboid(v3(x, y, 6.2), v3(1.6, 1.6, 1.0));
            }
            b.paint(ACCENT);
            // Flat mouths keep the 4x4 grid readable within the coarse budget.
            let z = 6.75;
            b.face(&[v3(x-0.55,y-0.55,z), v3(x+0.55,y-0.55,z),
                v3(x+0.55,y+0.55,z), v3(x-0.55,y+0.55,z)]);
        }
    }
    team_panel(b, v3(-6.0, 0.0, 2.2), v2(1.5, 5.0));
}
pub fn sam(b: &mut MeshBuilder, _: u8) {
    if !b.fine() {
        reduced_sam(b);
        return;
    }
    platform(b, 11.5);
    b.paint(PLATING_DARK);
    b.frustum(
        v3(0.0, 0.0, 1.5),
        v2(8.0, 8.0),
        v2(5.5, 5.5),
        3.0,
        v2(0.0, 0.0),
    );
    b.paint(PLATING);
    b.prism(v3(0.0, 0.0, 4.0), b.sides(8), 2.4, 2.1, 8.0);
    b.paint(ACCENT);
    b.prism(v3(0.0, 0.0, 12.0), b.sides(8), 1.65, 1.65, 0.05);
    b.paint(METAL);
    b.prism(v3(0.0, 0.0, 12.05), b.sides(8), 0.6, 0.02, 1.7);
    b.paint(PLATING_DARK);
    b.cuboid(v3(-5.5, 0.0, 4.0), v3(3.0, 4.0, 4.0));
    b.paint(GLASS);
    b.cuboid(v3(-3.97, 0.0, 4.3), v3(0.1, 3.0, 2.0));
    team_panel(b, v3(-5.5, 0.0, 6.1), v2(2.5, 2.0));
}
pub fn shatter(b: &mut MeshBuilder, _: u8) {
    if b.coarse() {
        b.paint(ACCENT);
        b.cuboid_open(v3(0.0, 0.0, 0.8), v3(18.0, 16.5, 1.6));
        b.paint(PLATING_DARK);
        b.cuboid_open(v3(0.0, 0.0, 4.8), v3(6.0, 5.8, 8.0));
        b.set_turret_pivot(v3(0.0, 0.0, 11.0));
        b.set_arm_pivot(v3(0.0, 0.0, 11.0));
        b.set_recoil(v3(0.0, 0.0, 11.0), v3(8.0, 0.0, 11.0), 1.35);
        b.with_part(part::TURRET, |b| {
            b.paint(PLATING);
            b.cuboid_open(v3(0.0, 0.0, 10.0), v3(3.5, 5.0, 3.0));
            b.with_limb(rig::ARM_GUN | rig::RECOIL, |b| {
                b.paint(ACCENT);
                b.cuboid_open(v3(2.5, 0.0, 11.0), v3(11.0, 2.8, 2.0));
            });
        });
        team_panel(b, v3(0.0, 0.0, 12.02), v2(1.2, 1.4));
        return;
    }
    if !b.fine() {
        b.paint(ACCENT);
        b.plate(v3(0.0, 0.0, 0.1), v2(18.0, 16.5), 1.25, 2.2);
        b.paint(PLATING_DARK);
        b.frustum(
            v3(0.0, 0.0, 1.2),
            v2(10.0, 9.0),
            v2(5.8, 5.8),
            7.8,
            v2(0.0, 0.0),
        );
        b.mirror_y(|b| {
            b.paint(PLATING);
            b.cuboid_open(v3(-1.0, 5.5, 2.6), v3(10.5, 3.0, 3.0));
        });
        shatter_mount(b, 11.0);
        return;
    }
    // A low, braced casemate: separate service housings break up the slab.
    b.paint(ACCENT);
    b.plate(v3(0.0, 0.0, 0.1), v2(18.0, 16.5), 1.25, 2.2);
    b.paint(PLATING_DARK);
    b.frustum(
        v3(0.0, 0.0, 1.2),
        v2(10.0, 9.0),
        v2(6.0, 5.8),
        6.4,
        v2(0.0, 0.0),
    );
    b.mirror_y(|b| {
        b.paint(PLATING);
        b.chamfered_box(v3(-1.0, 5.5, 2.6), v3(10.5, 3.0, 3.0), 0.7);
        b.paint(ACCENT);
        b.chamfered_box(v3(-2.4, 5.5, 4.12), v3(5.4, 2.3, 0.22), 0.3);
        b.paint(PLATING);
        b.frustum(
            v3(0.0, 2.4, 2.0),
            v2(3.1, 3.3),
            v2(2.4, 1.0),
            5.9,
            v2(0.0, -0.3),
        );
        if !b.coarse() {
            b.paint(METAL);
            b.cylinder_between(v3(3.1, 4.8, 2.3), v3(1.6, 2.1, 7.8), 0.24, 0.18, 8);
            b.paint(ACCENT);
            b.cuboid(v3(0.0, 3.0, 6.1), v3(1.1, 0.18, 2.0));
            b.paint(GLOW);
            b.cuboid(v3(0.0, 3.1, 6.2), v3(0.2, 0.1, 1.2));
        }
        if b.fine() {
            for x in [-4.4, -3.5, -2.6, -1.7, -0.8] {
                b.paint(METAL);
                b.cuboid(v3(x, 5.5, 4.28), v3(0.22, 1.9, 0.18));
            }
            b.paint(ACCENT);
            b.chamfered_box(v3(2.4, 5.5, 4.18), v3(2.6, 2.2, 0.3), 0.3);
            b.paint(METAL);
            b.cuboid(v3(2.4, 5.5, 4.37), v3(1.0, 0.18, 0.14));
            for x in [-7.2, 6.8] {
                b.prism(v3(x, 6.5, 1.36), 6, 0.32, 0.32, 0.25);
            }
        }
    });
    b.paint(PLATING);
    b.chamfered_box(v3(-4.0, 0.0, 3.8), v3(2.8, 4.2, 4.3), 0.5);
    team_panel(b, v3(-4.0, 0.0, 5.98), v2(1.7, 2.9));
    // Front access cassette, with a dark inset and separate hinge/latch hardware.
    b.paint(PLATING);
    b.chamfered_box(v3(3.9, 0.0, 4.6), v3(1.25, 3.7, 3.3), 0.35);
    b.paint(ACCENT);
    b.cuboid(v3(4.54, 0.0, 4.6), v3(0.08, 2.8, 2.35));
    b.paint(METAL);
    b.cuboid(v3(4.62, 0.85, 4.6), v3(0.14, 0.2, 0.7));
    for z in [3.9, 5.3] {
        b.cuboid(v3(4.62, -1.15, z), v3(0.16, 0.45, 0.22));
    }
    // Overlapping spindle, race and saddle remain connected at every yaw.
    b.paint(ACCENT);
    b.prism(v3(0.0, 0.0, 7.5), b.sides(12), 2.9, 2.9, 1.3);
    b.paint(METAL);
    b.prism(v3(0.0, 0.0, 8.55), b.sides(12), 2.6, 2.6, 0.5);
    shatter_mount(b, 11.0);
}

fn shatter_mount(b: &mut MeshBuilder, z: f32) {
    b.set_turret_pivot(v3(0.0, 0.0, z));
    b.set_arm_pivot(v3(0.0, 0.0, z));
    b.set_recoil(v3(0.0, 0.0, z), v3(8.0, 0.0, z), 1.35);
    b.with_part(part::TURRET, |b| {
        b.paint(ACCENT);
        b.prism(v3(0.0, 0.0, z - 2.2), b.sides(12), 2.65, 2.2, 0.85);
        b.mirror_y(|b| {
            b.paint(PLATING);
            b.chamfered_box(v3(-0.5, 2.0, z - 0.3), v3(3.4, 1.0, 3.0), 0.4);
            b.paint(ACCENT);
            b.cylinder_between(v3(0.0, 1.25, z), v3(0.0, 2.6, z), 0.85, 0.85, b.sides(12));
            if b.fine() {
                b.paint(METAL);
                b.cylinder_between(v3(0.0, 2.61, z), v3(0.0, 2.73, z), 0.43, 0.43, 8);
            }
        });
        b.with_limb(rig::ARM_GUN | rig::RECOIL, |b| {
            b.paint(ACCENT);
            b.chamfered_box(v3(-0.4, 0.0, z), v3(5.2, 2.8, 2.25), 0.5);
            b.chamfered_box(v3(4.2, 0.0, z), v3(7.3, 1.5, 1.4), 0.2);
            b.mirror_y(|b| {
                b.paint(PLATING);
                b.chamfered_box(v3(2.2, 1.0, z + 0.25), v3(4.5, 0.55, 1.55), 0.2);
                if b.fine() {
                    b.paint(METAL);
                    b.cylinder_between(
                        v3(-2.0, 0.9, z + 1.05),
                        v3(1.6, 0.9, z + 1.05),
                        0.19,
                        0.19,
                        8,
                    );
                }
                b.paint(GLOW);
                b.cuboid(v3(4.8, 0.8, z), v3(3.6, 0.08, 0.22));
                if b.fine() {
                    for x in [-1.8, -1.1, -0.4] {
                        b.paint(METAL);
                        b.cuboid(v3(x, 1.42, z - 0.15), v3(0.25, 0.1, 0.9));
                    }
                }
            });
            b.paint(METAL);
            b.chamfered_box(v3(7.35, 0.0, z), v3(1.25, 2.0, 1.8), 0.2);
            b.paint(ACCENT);
            b.cuboid(v3(7.99, 0.0, z), v3(0.06, 1.5, 1.3));
            b.paint(GLOW);
            b.cuboid(v3(8.03, 0.0, z), v3(0.03, 0.75, 0.38));
            team_panel(b, v3(-1.7, 0.0, z + 1.14), v2(1.4, 1.6));
        });
    });
}
pub fn mobile(b: &mut MeshBuilder, tech: u8) {
    if tech == 1 {
        gnat::build(b);
        return;
    }
    if tech == 2 {
        squall::build(b);
        return;
    }
    if tech == 3 {
        sunder::build(b);
        return;
    }
    let r = match tech {
        1 => 4.0,
        2 => 5.5,
        _ => 7.0,
    };
    let z = match tech {
        1 => 3.5,
        2 => 4.5,
        _ => 5.5,
    };
    b.set_treads(r * 0.7, r * 0.36, -r * 0.9);
    if !b.fine() {
        reduced_aa(b, r, z);
        return;
    }
    b.paint(PLATING);
    b.frustum(
        v3(0.0, 0.0, 0.8),
        v2(r * 1.8, r * 1.5),
        v2(r * 1.4, r),
        z - 1.5,
        v2(-0.3, 0.0),
    );
    b.with_part(part::LOCOMOTION, |b| {
        b.mirror_y(|b| {
            b.paint(TREAD);
            b.chamfered_box(v3(0.0, r * 0.7, 0.9), v3(r * 1.9, r * 0.36, 1.8), 0.35);
            if b.fine() {
                for x in [-0.6, 0.0, 0.6] {
                    b.paint(METAL);
                    b.cylinder_between(
                        v3(x * r, r * 0.89, 0.9),
                        v3(x * r, r * 0.91, 0.9),
                        0.55,
                        0.55,
                        8,
                    );
                }
            }
        })
    });
    mount(b, tech, z);
}

/// Skyguard without the close-up greebles: pad, pedestal, mast, launch mouth,
/// side cabinet. Nothing yaws.
fn reduced_sam(b: &mut MeshBuilder) {
    b.paint(PLATING_DARK);
    b.cuboid_open(v3(0.0, 0.0, 0.6), v3(19.55, 19.55, 1.2));
    if b.mid() {
        b.mirror_y(|b| {
            b.paint(PLATING);
            b.cuboid_open(v3(0.0, 7.13, 1.1), v3(17.25, 4.03, 2.0));
        });
        b.paint(PLATING_DARK);
        b.frustum_open(
            v3(0.0, 0.0, 1.5),
            v2(8.0, 8.0),
            v2(5.5, 5.5),
            3.0,
            v2(0.0, 0.0),
        );
        b.paint(PLATING);
        b.prism(v3(0.0, 0.0, 4.0), b.sides(8), 2.4, 2.1, 8.0);
        b.paint(ACCENT);
        b.prism(v3(0.0, 0.0, 12.0), 4, 1.65, 1.65, 0.2);
        b.paint(METAL);
        b.prism(v3(0.0, 0.0, 12.2), 4, 0.55, 0.04, 1.5);
        b.paint(PLATING_DARK);
        b.cuboid(v3(-5.5, 0.0, 4.0), v3(3.0, 4.0, 4.0));
        b.paint(GLASS);
        b.cuboid(v3(-3.97, 0.0, 4.3), v3(0.1, 3.0, 2.0));
    } else {
        b.paint(PLATING_DARK);
        b.cuboid_open(v3(0.0, 0.0, 2.75), v3(8.0, 8.0, 3.1));
        b.paint(PLATING);
        b.prism(v3(0.0, 0.0, 4.0), 4, 2.4, 2.1, 8.0);
        b.paint(ACCENT);
        b.decal(v3(0.0, 0.0, 12.02), v2(2.6, 2.6));
        b.paint(METAL);
        b.prism(v3(0.0, 0.0, 12.05), 4, 0.5, 0.04, 1.5);
        b.paint(PLATING_DARK);
        b.cuboid_open(v3(-5.5, 0.0, 4.0), v3(3.0, 4.0, 4.0));
    }
    team_panel(b, v3(-5.5, 0.0, 6.1), v2(2.5, 2.0));
}

fn reduced_aa(b: &mut MeshBuilder, r: f32, z: f32) {
    b.paint(PLATING_DARK);
    b.cuboid_open(v3(0.0, 0.0, 0.8), v3(r * 1.6, r * 1.5, 1.6));
    b.paint(PLATING);
    b.cuboid_open(v3(0.0, 0.0, z * 0.5), v3(r * 0.6, r * 0.6, z));
    b.set_turret_pivot(v3(0.0, 0.0, z));
    b.set_arm_pivot(v3(0.0, 0.0, z));
    b.with_part(part::TURRET, |b| {
        b.paint(METAL);
        b.cuboid_open(v3(r * 0.6, 0.0, z), v3(r * 1.1, r * 0.35, 0.4));
    });
    b.paint(TEAM);
    b.face(&[
        v3(-r * 0.2, -r * 0.2, z + 0.02),
        v3(r * 0.2, -r * 0.2, z + 0.02),
        v3(r * 0.2, r * 0.2, z + 0.02),
        v3(-r * 0.2, r * 0.2, z + 0.02),
    ]);
    if b.mid() {
        b.mirror_y(|b| {
            b.paint(PLATING);
            b.cuboid_open(v3(0.0, r * 0.6, 1.5), v3(r * 1.5, r * 0.25, 1.0));
        });
    }
}
