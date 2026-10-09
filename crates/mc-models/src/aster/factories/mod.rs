//! Aster factories: the Forge (land), the Aerie (air) and the Wharf (naval).
//!
//! One engineering language runs through all three, taken from the commander's
//! build arm: white-clad halls with black only as plinths, cornices, frames and
//! the conduit runs that light while the works are building; fabricator heads
//! (a black housing with a white lid, a gunmetal barrel through a white focusing
//! ring, three prongs round an amber emitter) that stand on something solid;
//! white-lidded fabricator drums; amber only where the building builds.
//!
//! Each factory prints on its lot origin, where the sim spawns the hull.
//! `factory_print_heads` in the sim fires from the fabricators' amber tips:
//! mounts and scales here and there are the same numbers. Later tiers bolt on
//! without stretching the T1 hull, and the next tier's kit rides as upgrade
//! pieces.

use glam::{Affine3A, Vec2, Vec3};
use mc_core::print_heads::{factory_heads, PrintHead, PAD_DECK, TUBE};

use super::parts::*;
use super::structures::kit;
use crate::builder::{chamfered_rect, MeshBuilder, Section};
use crate::material::*;
use crate::part;
use crate::pattern;

mod air;
mod land;
mod naval;

pub(super) use air::factory_air;
pub(super) use land::factory_land;
pub(super) use naval::factory_naval;

/// Authored land heights per tech: 28, 34, 42 m.
const FACTORY_TOWER: f32 = 24.0;

// ---- Shared engineering kit ----------------------------------------------------

/// The fabricator heads of `mesh`'s tier `tier` kit, with the point they aim at.
fn heads(mesh: &str, tier: u8) -> impl Iterator<Item = (Vec3, f32, Vec3)> {
    let factory = factory_heads(mesh).expect("a factory mesh");
    let aim = Vec3::from(factory.aim);
    factory
        .heads
        .iter()
        .filter(move |h: &&PrintHead| h.tier == tier)
        .map(move |h| (Vec3::from(h.mount), h.scale, aim))
}

/// A black run of conduit (lit while the works are building) with a white cap on top.
fn conduit(b: &mut MeshBuilder, min: Vec3, max: Vec3) {
    b.paint(ACCENT).pattern(pattern::CONDUIT);
    b.block(min, max);
    if b.fine() {
        b.paint(PLATING);
        b.block(
            v3(min.x - 0.05, min.y - 0.05, max.z),
            v3(max.x + 0.05, max.y + 0.05, max.z + 0.12),
        );
    }
}

/// A fabricator head, the factories' print gun, on a pedestal. `mount` is the
/// trunnion; the head yaws and pitches to point at `aim`, and its amber tip is
/// `TUBE * s` from the trunnion that way. The pedestal runs from under the
/// yoke (or over it, for a head hung from a jib) to `anchor_z`, where it meets
/// the wall, deck or jib that carries it, so nothing floats.
fn fabricator(b: &mut MeshBuilder, mount: Vec3, aim: Vec3, s: f32, anchor_z: f32) {
    let d = aim - mount;
    let yaw = d.y.atan2(d.x);
    let pitch = d.z.atan2(d.truncate().length());
    let hung = anchor_z > mount.z;
    b.yawed(mount, yaw, |b| {
        {
            // Pedestal and yoke: black, a white collar where it meets the yoke.
            b.paint(ACCENT);
            let (lo, hi) = if hung {
                (s * 0.55, anchor_z - mount.z)
            } else {
                (anchor_z - mount.z, -s * 0.55)
            };
            if hi > lo {
                b.block(v3(-s * 0.55, -s * 0.8, lo), v3(s * 0.45, s * 0.8, hi));
            }
            if b.fine() {
                let (y0, y1) = if hung {
                    (-s * 0.35, s * 0.55)
                } else {
                    (-s * 0.55, s * 0.35)
                };
                b.mirror_y(|b| b.block(v3(-s * 0.4, s * 0.62, y0), v3(s * 0.4, s * 0.95, y1)));
                b.paint(PLATING);
                let collar = if hung { s * 0.55 } else { -s * 0.67 };
                b.block(
                    v3(-s * 0.62, -s * 0.97, collar),
                    v3(s * 0.52, s * 0.97, collar + s * 0.12),
                );
                b.paint(METAL);
                b.cylinder_between(
                    v3(0.0, -s * 1.0, 0.0),
                    v3(0.0, s * 1.0, 0.0),
                    s * 0.26,
                    s * 0.26,
                    6,
                );
            }
            b.pitched(Vec3::ZERO, pitch, |b| {
                // Housing: black body, white lid, conduit down each flank.
                b.paint(ACCENT);
                if b.fine() {
                    b.chamfered_box(
                        v3(-s * 0.2, 0.0, 0.0),
                        v3(s * 2.0, s * 1.2, s * 0.9),
                        s * 0.2,
                    );
                } else {
                    b.cuboid(v3(-s * 0.2, 0.0, 0.0), v3(s * 2.0, s * 1.2, s * 0.9));
                }
                b.paint(PLATING);
                b.plate(
                    v3(-s * 0.3, 0.0, s * 0.45),
                    v2(s * 1.6, s * 0.9),
                    s * 0.1,
                    s * 0.04,
                );
                if b.fine() {
                    b.paint(ACCENT).pattern(pattern::CONDUIT);
                    b.block(
                        v3(-s * 1.05, -s * 0.64, -s * 0.3),
                        v3(-s * 0.45, s * 0.64, s * 0.25),
                    );
                    b.paint(PLATING);
                    b.block(
                        v3(-s * 1.25, -s * 0.5, -s * 0.36),
                        v3(-s * 1.15, s * 0.5, s * 0.36),
                    );
                }
                // Barrel, focusing ring, collar.
                b.paint(METAL);
                b.cylinder_between(
                    v3(s * 0.8, 0.0, 0.0),
                    v3(s * 2.65, 0.0, 0.0),
                    s * 0.3,
                    s * 0.24,
                    b.sides(8),
                );
                if b.fine() {
                    b.paint(PLATING);
                    b.cylinder_between(
                        v3(s * 1.35, 0.0, 0.0),
                        v3(s * 1.6, 0.0, 0.0),
                        s * 0.46,
                        s * 0.46,
                        8,
                    );
                    b.paint(ACCENT);
                    b.cylinder_between(
                        v3(s * 2.55, 0.0, 0.0),
                        v3(s * 2.82, 0.0, 0.0),
                        s * 0.38,
                        s * 0.34,
                        8,
                    );
                }
                // Prongs round the emitter.
                if b.fine() {
                    b.paint(METAL);
                    for k in 0..3 {
                        let a = (k as f32 + 0.25) * std::f32::consts::TAU / 3.0;
                        let (cy, cz) = (a.cos(), a.sin());
                        b.beam(
                            v3(s * 2.75, cy * s * 0.3, cz * s * 0.3),
                            v3(s * 3.5, cy * s * 0.17, cz * s * 0.17),
                            v2(s * 0.12, s * 0.12),
                            v2(s * 0.07, s * 0.07),
                        );
                    }
                }
                b.paint(GLOW_AMBER);
                b.cylinder_between(
                    v3(s * 2.82, 0.0, 0.0),
                    v3(s * TUBE, 0.0, 0.0),
                    s * 0.2,
                    s * 0.05,
                    6,
                );
            });
        }
    });
}

/// Fabricator drum: a black drum under a white shell, a black band at its waist, a
/// white lid with a black cap, feet on a white plinth.
fn fabricator_tank(b: &mut MeshBuilder, at: Vec3, radius: f32, height: f32) {
    let sides = b.sides(10);
    b.paint(ACCENT);
    b.prism(at, sides, radius * 1.12, radius * 1.12, height * 0.12);
    b.paint(PLATING);
    b.cylinder_between(
        at + Vec3::Z * (height * 0.12),
        at + Vec3::Z * (height * 0.5),
        radius,
        radius,
        sides,
    );
    b.paint(ACCENT);
    b.cylinder_between(
        at + Vec3::Z * (height * 0.5),
        at + Vec3::Z * (height * 0.56),
        radius * 0.97,
        radius * 0.97,
        sides,
    );
    if b.fine() {
        b.paint(ACCENT);
        b.cylinder_between(
            at + Vec3::Z * (height * 0.56),
            at + Vec3::Z * (height * 0.64),
            radius * 0.97,
            radius * 0.97,
            sides,
        );
    }
    b.paint(PLATING);
    b.cylinder_between(
        at + Vec3::Z * (height * 0.56),
        at + Vec3::Z * (height * 0.92),
        radius,
        radius * 0.86,
        sides,
    );
    if b.fine() {
        b.paint(ACCENT);
        b.prism(
            at + Vec3::Z * (height * 0.92),
            sides,
            radius * 0.5,
            radius * 0.4,
            height * 0.08,
        );
        // Hoops.
        b.paint(ACCENT);
        for f in [0.24, 0.8] {
            b.cylinder_between(
                at + Vec3::Z * (height * f),
                at + Vec3::Z * (height * f + 0.18),
                radius * 1.03,
                radius * 1.03,
                sides,
            );
        }
    }
}

/// A floor slab from `min` to `max` (plan) and `height` thick, with a hole of
/// half size `well` at the lot centre for the lift's well.
fn apron(b: &mut MeshBuilder, min: Vec2, max: Vec2, well: Vec2, height: f32) {
    let slab = |b: &mut MeshBuilder, lo: Vec2, hi: Vec2| {
        if hi.x > lo.x && hi.y > lo.y {
            b.block(lo.extend(0.0), hi.extend(height));
        }
    };
    slab(b, min, v2(-well.x, max.y));
    slab(b, v2(well.x, min.y), max);
    slab(b, v2(-well.x, min.y), v2(well.x, -well.y));
    slab(b, v2(-well.x, well.y), v2(well.x, max.y));
}

/// A portal gantry across the lot at `x`: black legs on white footings at `±legs_y`,
/// standing on `base_z`, and a white box girder on a black soffit at `deck_z`, the
/// owner's colour along its top, a conduit down its +x face and clearance stripes
/// on its -x face.
fn portal_gantry(b: &mut MeshBuilder, x: f32, legs_y: f32, base_z: f32, deck_z: f32) {
    b.mirror_y(|b| {
        b.paint(PLATING);
        b.cuboid_open(v3(x, legs_y, base_z + 0.9), v3(4.4, 3.8, 1.8));
        b.paint(ACCENT);
        b.frustum_open(
            v3(x, legs_y, base_z + 1.8),
            v2(3.0, 2.6),
            v2(2.4, 2.0),
            deck_z - base_z - 1.8,
            v2(0.0, -0.3),
        );
        if b.fine() {
            b.paint(ACCENT).pattern(pattern::CONDUIT);
            b.block(
                v3(x + 1.25, legs_y - 0.8, base_z + 3.0),
                v3(x + 1.55, legs_y + 0.4, deck_z - 1.0),
            );
        }
    });
    let span = legs_y * 2.0;
    b.paint(ACCENT);
    b.cuboid(v3(x, 0.0, deck_z + 0.3), v3(3.2, span + 3.8, 0.6));
    b.paint(PLATING);
    b.chamfered_box(v3(x, 0.0, deck_z + 1.4), v3(3.4, span + 4.8, 1.6), 0.4);
    b.paint(PLATING);
    b.plate(v3(x, 0.0, deck_z + 2.2), v2(2.6, span - 3.2), 0.12, 0.04);
    team_panel(b, v3(x, 0.0, deck_z + 2.32), v2(1.1, span - 6.0));
    if b.fine() {
        use crate::gpu_consts::surface_mark as mark;
        b.mark(
            v3(x - 1.85, -span * 0.3, deck_z + 0.8),
            v3(x - 1.55, span * 0.3, deck_z + 1.9),
            mark::HAZARD,
            0.0,
        );
    }
    conduit(
        b,
        v3(x + 1.7, -span * 0.46, deck_z + 0.5),
        v3(x + 1.95, span * 0.46, deck_z + 1.3),
    );
}

/// Shared lift deck at the lot centre: a black well, a white lid on each
/// lip (the same plan as the wall under it), the deck itself dark so the
/// hull reads.
fn factory_pad(b: &mut MeshBuilder, pad_x: f32, pad_y: f32) {
    b.paint(ACCENT);
    b.mirror_y(|b| {
        b.cuboid(
            v3(0.0, pad_y + 1.15, 0.82),
            v3(pad_x * 2.0 + 1.4, 2.3, 1.64),
        );
    });
    b.cuboid(
        v3(-pad_x - 1.15, 0.0, 0.82),
        v3(2.3, pad_y * 2.0 + 2.3, 1.64),
    );
    b.cuboid(
        v3(pad_x + 1.15, 0.0, 0.82),
        v3(2.3, pad_y * 2.0 + 2.3, 1.64),
    );
    b.paint(PLATING);
    b.mirror_y(|b| {
        b.cuboid(
            v3(0.0, pad_y + 1.15, 1.72),
            v3(pad_x * 2.0 + 1.2, 2.05, 0.16),
        );
    });
    // The end lids stand a hair proud of the side lids where they cross at the corners.
    b.cuboid(
        v3(-pad_x - 1.15, 0.0, 1.74),
        v3(2.05, pad_y * 2.0 + 1.2, 0.2),
    );
    b.cuboid(
        v3(pad_x + 1.15, 0.0, 1.74),
        v3(2.05, pad_y * 2.0 + 1.2, 0.2),
    );
    b.with_lift(|b| {
        b.paint(ACCENT).pattern(pattern::DECK);
        b.cuboid(
            v3(0.0, 0.0, PAD_DECK - 0.1),
            v3(pad_x * 2.0 - 0.7, pad_y * 2.0 - 0.7, 0.22),
        );
    });
}

/// Control tower: white shaft on a black plinth, a band of glass, a white roof with
/// the owner's colour on it.
fn factory_tower(b: &mut MeshBuilder, at: Vec3, base_z: f32, tower: f32) {
    let plan = chamfered_rect(v2(5.0, 5.8), 1.4);
    b.at(at, |b| {
        b.paint(ACCENT);
        b.loft_z(
            &plan,
            &[Section::new(base_z, 1.05), Section::new(base_z + 1.4, 1.05)],
        );
        b.paint(PLATING);
        b.loft_z(
            &plan,
            &[
                Section::new(base_z + 1.4, 1.0),
                Section::new(tower - 4.8, 0.9),
            ],
        );
        b.paint(GLASS);
        b.loft_z(
            &plan,
            &[
                Section::new(tower - 4.8, 0.94),
                Section::new(tower - 3.2, 0.96),
            ],
        );
        b.paint(ACCENT);
        b.loft_z(
            &plan,
            &[
                Section::new(tower - 3.2, 0.96),
                Section::new(tower - 2.4, 0.96),
                Section::scaled(tower, 0.6, 0.68).shifted(-0.4, 0.0),
            ],
        );
        b.paint(PLATING);
        b.plate(v3(-0.4, 0.0, tower), v2(3.9, 4.6), 0.14, 0.05);
        team_panel(b, v3(-0.4, 0.0, tower + 0.14), v2(2.6, 3.2));
    });
}

/// [`on_slope`] for a surface given by its cross-section (y, z): the frame's x runs
/// along the building, +y toward `front`, +z out of the surface.
fn on_slope_y(
    b: &mut MeshBuilder,
    front: [f32; 2],
    rear: [f32; 2],
    along: f32,
    f: impl FnOnce(&mut MeshBuilder),
) {
    let (dy, dz) = (front[0] - rear[0], front[1] - rear[1]);
    let at = v3(0.0, rear[0] + dy * along, rear[1] + dz * along);
    b.with(
        Affine3A::from_translation(at) * Affine3A::from_rotation_x(dz.atan2(dy)),
        f,
    );
}
