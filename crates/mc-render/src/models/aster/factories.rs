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
use mc_sim::print_heads::{factory_heads, PrintHead, PAD_DECK, TUBE};

use super::parts::*;
use super::structures::kit;
use crate::models::builder::{chamfered_rect, MeshBuilder, Section};
use crate::models::material::*;
use crate::models::pattern;

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
        b.block(v3(min.x - 0.05, min.y - 0.05, max.z), v3(max.x + 0.05, max.y + 0.05, max.z + 0.12));
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
                let (y0, y1) = if hung { (-s * 0.35, s * 0.55) } else { (-s * 0.55, s * 0.35) };
                b.mirror_y(|b| b.block(v3(-s * 0.4, s * 0.62, y0), v3(s * 0.4, s * 0.95, y1)));
                b.paint(PLATING);
                let collar = if hung { s * 0.55 } else { -s * 0.67 };
                b.block(v3(-s * 0.62, -s * 0.97, collar), v3(s * 0.52, s * 0.97, collar + s * 0.12));
                b.paint(METAL);
                b.cylinder_between(v3(0.0, -s * 1.0, 0.0), v3(0.0, s * 1.0, 0.0), s * 0.26, s * 0.26, 6);
            }
            b.pitched(Vec3::ZERO, pitch, |b| {
                // Housing: black body, white lid, conduit down each flank.
                b.paint(ACCENT);
                if b.fine() {
                    b.chamfered_box(v3(-s * 0.2, 0.0, 0.0), v3(s * 2.0, s * 1.2, s * 0.9), s * 0.2);
                } else {
                    b.cuboid(v3(-s * 0.2, 0.0, 0.0), v3(s * 2.0, s * 1.2, s * 0.9));
                }
                b.paint(PLATING);
                b.plate(v3(-s * 0.3, 0.0, s * 0.45), v2(s * 1.6, s * 0.9), s * 0.1, s * 0.04);
                if b.fine() {
                    b.paint(ACCENT).pattern(pattern::CONDUIT);
                    b.block(v3(-s * 1.05, -s * 0.64, -s * 0.3), v3(-s * 0.45, s * 0.64, s * 0.25));
                    b.paint(PLATING);
                    b.block(v3(-s * 1.25, -s * 0.5, -s * 0.36), v3(-s * 1.15, s * 0.5, s * 0.36));
                }
                // Barrel, focusing ring, collar.
                b.paint(METAL);
                b.cylinder_between(v3(s * 0.8, 0.0, 0.0), v3(s * 2.65, 0.0, 0.0), s * 0.3, s * 0.24, b.sides(8));
                if b.fine() {
                    b.paint(PLATING);
                    b.cylinder_between(v3(s * 1.35, 0.0, 0.0), v3(s * 1.6, 0.0, 0.0), s * 0.46, s * 0.46, 8);
                    b.paint(GLOW_AMBER);
                    b.cylinder_between(v3(s * 2.05, 0.0, 0.0), v3(s * 2.13, 0.0, 0.0), s * 0.34, s * 0.34, 8);
                    b.paint(ACCENT);
                    b.cylinder_between(v3(s * 2.55, 0.0, 0.0), v3(s * 2.82, 0.0, 0.0), s * 0.38, s * 0.34, 8);
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
                b.cylinder_between(v3(s * 2.82, 0.0, 0.0), v3(s * TUBE, 0.0, 0.0), s * 0.2, s * 0.05, 6);
            });
        }
    });
}

/// Fabricator drum: a black drum under a white shell, an amber charge ring, a
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
    b.paint(GLOW_AMBER);
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
        b.prism(at + Vec3::Z * (height * 0.92), sides, radius * 0.5, radius * 0.4, height * 0.08);
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
/// owner's colour along its top and a conduit down its +x face.
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
            b.block(v3(x + 1.25, legs_y - 0.8, base_z + 3.0), v3(x + 1.55, legs_y + 0.4, deck_z - 1.0));
        }
    });
    let span = legs_y * 2.0;
    b.paint(ACCENT);
    b.cuboid(v3(x, 0.0, deck_z + 0.3), v3(3.2, span + 3.8, 0.6));
    b.paint(PLATING);
    b.chamfered_box(v3(x, 0.0, deck_z + 1.4), v3(3.4, span + 4.8, 1.6), 0.4);
    b.paint(PLATING).pattern(pattern::TEAM_BAND);
    b.plate(v3(x, 0.0, deck_z + 2.2), v2(2.6, span - 3.2), 0.12, 0.04);
    conduit(b, v3(x + 1.7, -span * 0.46, deck_z + 0.5), v3(x + 1.95, span * 0.46, deck_z + 1.3));
}

/// Red obstruction lamp on a short stalk.
fn beacon(b: &mut MeshBuilder, at: Vec3) {
    b.paint(ACCENT);
    b.prism(at, 6, 0.16, 0.11, 0.4);
    b.paint(GLOW_RED);
    b.cuboid(at + Vec3::Z * 0.58, Vec3::splat(0.38));
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
    b.cuboid(v3(-pad_x - 1.15, 0.0, 0.82), v3(2.3, pad_y * 2.0 + 2.3, 1.64));
    b.cuboid(v3(pad_x + 1.15, 0.0, 0.82), v3(2.3, pad_y * 2.0 + 2.3, 1.64));
    b.paint(PLATING);
    b.mirror_y(|b| {
        b.cuboid(
            v3(0.0, pad_y + 1.15, 1.72),
            v3(pad_x * 2.0 + 1.2, 2.05, 0.16),
        );
    });
    // The end lids stand a hair proud of the side lids where they cross at the corners.
    b.cuboid(v3(-pad_x - 1.15, 0.0, 1.74), v3(2.05, pad_y * 2.0 + 1.2, 0.2));
    b.cuboid(v3(pad_x + 1.15, 0.0, 1.74), v3(2.05, pad_y * 2.0 + 1.2, 0.2));
    b.with_lift(|b| {
        b.paint(ACCENT).pattern(pattern::DECK);
        b.cuboid(
            v3(0.0, 0.0, PAD_DECK - 0.1),
            v3(pad_x * 2.0 - 0.7, pad_y * 2.0 - 0.7, 0.22),
        );
    });
}

/// Control tower: white shaft on a black plinth, a band of glass, a white roof.
fn factory_tower(b: &mut MeshBuilder, at: Vec3, base_z: f32, tower: f32) {
    let plan = chamfered_rect(v2(5.0, 5.8), 1.4);
    b.at(at, |b| {
        b.paint(ACCENT);
        b.loft_z(&plan, &[Section::new(base_z, 1.05), Section::new(base_z + 1.4, 1.05)]);
        b.paint(PLATING);
        b.loft_z(
            &plan,
            &[
                Section::new(base_z + 1.4, 1.0),
                Section::new(tower - 4.8, 0.9),
            ],
        );
        b.paint(GLASS);
        b.loft_z(&plan, &[Section::new(tower - 4.8, 0.94), Section::new(tower - 3.2, 0.96)]);
        b.paint(ACCENT);
        b.loft_z(
            &plan,
            &[
                Section::new(tower - 3.2, 0.96),
                Section::new(tower - 2.4, 0.96),
                Section::scaled(tower, 0.6, 0.68).shifted(-0.4, 0.0),
            ],
        );
        b.paint(PLATING).pattern(pattern::TEAM_BAND);
        b.plate(v3(-0.4, 0.0, tower), v2(3.9, 4.6), 0.14, 0.05);
    });
}


/// [`on_slope`] for a surface given by its cross-section (y, z): the frame's x runs
/// along the building, +y toward `front`, +z out of the surface.
fn on_slope_y(b: &mut MeshBuilder, front: [f32; 2], rear: [f32; 2], along: f32, f: impl FnOnce(&mut MeshBuilder)) {
    let (dy, dz) = (front[0] - rear[0], front[1] - rear[1]);
    let at = v3(0.0, rear[0] + dy * along, rear[1] + dz * along);
    b.with(Affine3A::from_translation(at) * Affine3A::from_rotation_x(dz.atan2(dy)), f);
}

// ---- Forge: land factory ---------------------------------------------------------

/// Land foundry: a U of white halls round the pad, a road out +x, fabricators on
/// the halls' print shelves.
pub fn factory_land(b: &mut MeshBuilder, tech: u8) {
    const WALL_H: f32 = 8.4;
    const WALL_Y: f32 = 26.0;
    const HOUSE_Z: f32 = 13.6;
    const TOWER: f32 = FACTORY_TOWER;
    /// Top of the print shelves the fabricators stand on.
    const SHELF: f32 = WALL_H * 0.68 + 0.8;

    if b.coarse() {
        b.paint(PLATING);
        b.frustum_open(
            v3(-26.0, 0.0, 0.0),
            v2(22.0, 36.0),
            v2(16.0, 28.0),
            HOUSE_Z,
            v2(-1.2, 0.0),
        );
        b.mirror_y(|b| b.cuboid_open(v3(2.0, WALL_Y, WALL_H * 0.5), v3(36.0, 8.0, WALL_H)));
        b.paint(ACCENT);
        b.frustum_open(v3(29.0, 0.0, 0.0), v2(31.0, 34.0), v2(27.0, 26.0), 0.9, v2(0.0, 0.0));
        let tower_h = TOWER - HOUSE_Z
            + match tech {
                1 => 0.0,
                2 => 8.0,
                _ => 16.0,
            };
        b.paint(PLATING);
        b.frustum_open(
            v3(-28.0, -16.0, HOUSE_Z),
            v2(9.0, 11.0),
            v2(
                if tech >= 2 { 4.0 } else { 6.4 },
                if tech >= 2 { 5.0 } else { 8.0 },
            ),
            tower_h,
            v2(0.0, 0.0),
        );
        team_panel(b, v3(-24.0, 0.0, HOUSE_Z), v2(10.0, 8.0));
        b.mirror_y(|b| team_panel(b, v3(2.0, WALL_Y, WALL_H), v2(8.0, 4.0)));
        return;
    }

    // Courtyard floor between the halls: graphite plate round the pad's well, so the
    // deck still shows when it drops.
    b.paint(PLATING_DARK);
    apron(b, v2(-13.4, -21.7), v2(21.0, 21.7), v2(13.5, 11.7), 0.28);
    factory_pad(b, 11.2, 9.4);

    // Rear house behind the pad: white walls on a black plinth, a white roof.
    b.paint(PLATING);
    b.extrude_y_chamfered(
        &[
            [-38.0, 0.0],
            [-14.2, 0.0],
            [-14.2, 7.2],
            [-20.0, HOUSE_Z],
            [-32.5, HOUSE_Z],
            [-38.0, 9.2],
        ],
        22.0,
        2.2,
    );
    b.paint(ACCENT);
    b.block(v3(-38.4, -21.2, 0.0), v3(-13.9, 21.2, 1.5));
    b.paint(PLATING).pattern(pattern::TEAM_BAND);
    b.plate(v3(-26.25, 0.0, HOUSE_Z), v2(11.8, 38.0), 0.2, 0.06);
    // The forge's mouth: a long bank of louvres over the door, the fire showing through.
    on_slope(b, [-14.35, 7.35], [-19.85, HOUSE_Z - 0.05], 0.5, |b| {
        b.paint(ACCENT);
        b.plate(Vec3::ZERO, v2(6.6, 36.0), 0.16, 0.05);
        b.paint(ACCENT).pattern(pattern::FURNACE);
        b.plate(v3(0.0, 0.0, 0.16), v2(3.0, 26.0), 0.06, 0.03);
    });
    // The back slope carries a run of conduit.
    on_slope(b, [-32.5, HOUSE_Z], [-38.0, 9.2], 0.5, |b| {
        b.paint(ACCENT).pattern(pattern::CONDUIT);
        b.plate(Vec3::ZERO, v2(2.2, 34.0), 0.12, 0.04);
    });
    team_panel(b, v3(-26.0, 0.0, HOUSE_Z + 0.22), v2(8.0, 7.0));
    // The hull comes out of a dark aperture lined with conduit, under a white lintel.
    b.paint(ACCENT);
    b.block(v3(-14.2, -10.4, 0.0), v3(-13.6, 10.4, 7.6));
    b.paint(ACCENT).pattern(pattern::CONDUIT);
    b.block(v3(-14.28, -9.4, 1.0), v3(-13.5, 9.4, 6.8));
    b.paint(PLATING);
    b.block(v3(-14.3, -10.8, 7.6), v3(-13.3, 10.8, 8.4));
    // Roof exhaust: drawn, not modelled.
    b.paint(ACCENT).pattern(pattern::FURNACE);
    b.plate(v3(-26.0, -6.6, HOUSE_Z + 0.2), v2(6.4, 3.4), 0.08, 0.04);
    if b.fine() {
        // Black pilasters up the rear wall, so the long white face has a rhythm.
        for y in [-15.0, -5.0, 5.0, 15.0] {
            b.paint(ACCENT);
            b.block(v3(-38.35, y - 0.6, 1.5), v3(-37.9, y + 0.6, 8.9));
        }
    }

    // Side halls: white, a black plinth and cornice, one lid per hall, one inner cheek.
    b.mirror_y(|b| {
        b.paint(PLATING);
        b.chamfered_box(v3(2.0, WALL_Y, WALL_H * 0.5), v3(38.0, 8.6, WALL_H), 1.05);
        b.paint(ACCENT);
        b.chamfered_box(v3(2.0, WALL_Y, 0.7), v3(38.4, 9.0, 1.4), 1.2);
        b.chamfered_box(v3(2.0, WALL_Y, WALL_H - 0.42), v3(38.3, 8.9, 0.96), 1.15);
        b.paint(PLATING);
        b.plate(v3(2.0, WALL_Y, WALL_H), v2(35.0, 7.2), 0.18, 0.06);
        team_panel(b, v3(-1.0, WALL_Y, WALL_H + 0.18), v2(6.0, 3.0));
        b.paint(ACCENT);
        b.block(v3(-15.0, WALL_Y - 4.42, 1.3), v3(18.0, WALL_Y - 4.05, WALL_H - 0.9));
        if b.fine() {
            vent(b, v3(14.5, WALL_Y, WALL_H + 0.18), v2(4.6, 2.4), 3, ACCENT);
            // The outer wall: buttress ribs, and between them a band of conduit.
            for x in [-12.0, -4.0, 4.0, 12.0] {
                b.paint(ACCENT);
                b.frustum(v3(x, WALL_Y + 4.3, 0.0), v2(1.4, 1.6), v2(1.0, 0.3), WALL_H - 0.9, v2(0.0, -0.6));
            }
            b.paint(ACCENT).pattern(pattern::CONDUIT);
            b.block(v3(-15.0, WALL_Y + 4.25, 3.2), v3(19.0, WALL_Y + 4.42, 4.6));
            // Brackets under the print shelf, into the wall.
            for x in [-9.0, 0.0, 10.0] {
                b.paint(ACCENT);
                b.beam(
                    v3(x, WALL_Y - 4.4, 2.2),
                    v3(x, WALL_Y - 6.6, SHELF - 1.4),
                    v2(0.6, 0.5),
                    v2(0.6, 0.5),
                );
            }
        }
        // Print shelf on the inner face: a ledge the fabricators stand on, deep
        // enough to read from above, lit along its face.
        conduit(b, v3(-11.0, WALL_Y - 7.3, SHELF - 1.48), v3(15.0, WALL_Y - 4.3, SHELF - 0.12));
    });

    factory_tower(b, v3(-28.0, -16.0, 0.0), HOUSE_Z, TOWER);

    for (mount, s, aim) in heads("factory_land", 1) {
        fabricator(b, mount, aim, s, SHELF);
    }
    kit(b, tech, 2, 0.18, |b| {
        for (mount, s, aim) in heads("factory_land", 2) {
            fabricator(b, mount, aim, s, SHELF);
        }
    });
    kit(b, tech, 3, 0.14, |b| {
        // Two more on brackets off the forge door's frame.
        b.mirror_y(|b| {
            b.paint(ACCENT);
            b.block(v3(-14.4, 6.9, 5.9), v3(-12.0, 9.1, 6.8));
            b.paint(PLATING);
            b.block(v3(-14.3, 7.02, 6.8), v3(-12.1, 8.98, 6.92));
        });
        for (mount, s, aim) in heads("factory_land", 3) {
            fabricator(b, mount, aim, s, 6.92);
        }
    });

    // The road out: a black kerbed apron from the pad's lip to the lot edge.
    b.paint(ACCENT);
    b.frustum(v3(29.0, 0.0, 0.2), v2(31.0, 36.0), v2(27.0, 28.0), 0.4, v2(0.4, 0.0));
    b.paint(PLATING).pattern(pattern::ROADWAY);
    b.plate(v3(29.4, 0.0, 0.62), v2(27.0, 26.0), 0.12, 0.04);
    if b.fine() {
        b.mirror_y(|b| {
            b.paint(ACCENT);
            b.block(v3(16.0, 13.2, 0.55), v3(42.0, 14.2, 0.9));
        });
    }

    kit(b, tech, 2, 0.28, |b| {
        // A gate gantry over the courtyard's mouth, standing on the hall roofs, high
        // enough for the tallest walker to leave under it.
        portal_gantry(b, 18.5, WALL_Y, WALL_H, 26.0);
        b.paint(ACCENT);
        b.cuboid(v3(18.5, 0.0, 25.35), v3(4.0, 10.0, 1.3));
        b.paint(GLOW_AMBER);
        b.cuboid(v3(18.5, 0.0, 24.66), v3(3.0, 8.0, 0.08));
        // Fabricator drums on the house roof, piped down to the forge.
        for y in [9.5, 16.2] {
            fabricator_tank(b, v3(-26.4, y, HOUSE_Z + 0.2), 2.7, 6.0);
            b.paint(METAL);
            b.cylinder_between(v3(-23.8, y, HOUSE_Z + 1.2), v3(-20.6, y, HOUSE_Z + 0.6), 0.35, 0.35, 6);
        }
        // Assembly lofts on the hall roofs.
        b.mirror_y(|b| {
            b.paint(PLATING);
            b.cuboid_open(v3(-10.0, WALL_Y, WALL_H + 3.6), v3(10.0, 5.6, 7.2));
            b.paint(ACCENT);
            b.cuboid(v3(-10.0, WALL_Y, WALL_H + 0.7), v3(10.3, 5.9, 1.2));
            b.cuboid(v3(-10.0, WALL_Y, WALL_H + 6.63), v3(10.3, 5.9, 0.7));
            b.paint(PLATING);
            b.plate(v3(-10.0, WALL_Y, WALL_H + 7.2), v2(9.2, 5.0), 0.16, 0.05);
            if b.fine() {
                vent(b, v3(-10.0, WALL_Y, WALL_H + 7.36), v2(4.0, 2.6), 3, GLOW_AMBER);
            }
        });
        b.paint(PLATING);
        b.prism(v3(-28.0, -16.0, TOWER), 8, 3.2, 2.6, 2.2);
        b.paint(ACCENT);
        b.prism(v3(-28.0, -16.0, TOWER + 2.2), 8, 2.6, 2.4, 0.3);
        b.paint(METAL);
        b.prism(v3(-28.4, -16.0, TOWER + 2.2), 6, 0.65, 0.26, 8.0);
    });

    kit(b, tech, 3, 0.32, |b| {
        b.paint(METAL);
        b.prism(v3(-28.4, -16.0, TOWER + 2.2), 6, 1.1, 0.42, 14.0);
        // Furnace feed pipes: a bank either side up the rear wall from a pump, over
        // onto the back slope and up it into a manifold at the roof's edge.
        b.mirror_y(|b| {
            const R: f32 = 0.42;
            const RISER_X: f32 = -39.0;
            let sides = b.sides(8);
            // Along the back slope, lifted clear of it.
            let (foot, top) = (Vec2::new(-38.0, 9.2), Vec2::new(-32.5, HOUSE_Z));
            let normal = (top - foot).perp().normalize() * (R + 0.35);
            let on_slope = |t: f32| foot.lerp(top, t) + normal;
            let (low, high) = (on_slope(0.0), on_slope(0.92));
            let ys = [7.8, 9.0, 10.2];
            b.paint(ACCENT);
            b.block(v3(-39.8, 7.0, 0.0), v3(-38.4, 11.0, 1.6));
            b.block(v3(high.x - 0.4, 6.9, high.y - 0.9), v3(high.x + 0.9, 11.1, high.y + 0.8));
            for y in ys {
                let bend = v3(RISER_X, y, low.y - 0.3);
                let low = v3(low.x, y, low.y);
                b.paint(METAL);
                b.cylinder_between(v3(RISER_X, y, 1.4), bend, R, R, sides);
                b.cylinder_between(bend, low, R, R, sides);
                b.cylinder_between(low, v3(high.x, y, high.y), R, R, sides);
                if b.fine() {
                    b.paint(ACCENT);
                    for (a, c) in [(3.6, 4.0), (bend.z - 0.2, bend.z + 0.2)] {
                        b.cylinder_between(v3(RISER_X, y, a), v3(RISER_X, y, c), R + 0.12, R + 0.12, sides);
                    }
                    let run = (v3(high.x, y, high.y) - low).normalize();
                    b.cylinder_between(low - run * 0.2, low + run * 0.2, R + 0.12, R + 0.12, sides);
                    // A sight glass on the riser, the furnace showing in it.
                    b.paint(GLOW_ORANGE);
                    b.cylinder_between(v3(RISER_X, y, 5.6), v3(RISER_X, y, 6.2), R + 0.04, R + 0.04, sides);
                }
            }
        });
        // Tall print lofts beside the first, a conduit down their inner faces.
        b.mirror_y(|b| {
            b.paint(PLATING);
            b.cuboid_open(v3(7.0, WALL_Y + 0.4, WALL_H + 4.4), v3(8.4, 6.2, 8.8));
            b.paint(ACCENT);
            b.cuboid(v3(7.0, WALL_Y + 0.4, WALL_H + 0.7), v3(8.7, 6.5, 1.4));
            b.paint(ACCENT).pattern(pattern::CONDUIT);
            b.block(v3(3.2, WALL_Y - 2.9, WALL_H + 2.0), v3(10.8, WALL_Y - 2.7, WALL_H + 7.4));
            b.paint(PLATING);
            b.plate(v3(7.0, WALL_Y + 0.4, WALL_H + 8.8), v2(7.6, 5.6), 0.16, 0.05);
        });
    });

    if tech >= 2 {
        b.paint(GLOW);
        for y in [9.5, 16.2] {
            b.prism(v3(-26.4, y, HOUSE_Z + 6.2), 8, 0.9, 0.6, 0.6);
        }
        if b.fine() {
            antenna(b, v3(-25.6, -20.0, TOWER + 2.2), 3.4, 0.08);
        }
    }
    if tech >= 3 {
        b.paint(GLOW);
        b.prism(v3(-28.4, -16.0, TOWER + 16.0), 6, 0.5, 0.2, 1.0);
        if b.fine() {
            b.mirror_y(|b| glow_strip(b, v3(7.0, WALL_Y + 0.4, WALL_H + 8.96), v2(3.4, 2.0), GLOW));
            antenna(b, v3(-31.0, -20.0, TOWER), 4.2, 0.04);
        }
    }
    if b.fine() {
        antenna(b, v3(-31.2, -12.0, TOWER), 4.8, 0.0);
    }
}

// ---- Aerie: air factory -----------------------------------------------------------

const AIR_HANGAR_Y: f32 = 27.0;
const AIR_GANTRY_X: f32 = -19.0;
const AIR_GANTRY_Z: f32 = 18.0;
const AIR_TOWER: Vec3 = Vec3::new(33.0, -37.0, 0.0);

/// Air hangar: a clear lift deck with nothing over it, a white hangar either side
/// with its shutters on the deck, a portal gantry over the deck's back edge carrying
/// the fabricators, a blast fence behind that, a marked launch lane out +x and a
/// control tower beside the lane.
pub fn factory_air(b: &mut MeshBuilder, tech: u8) {
    const HANGAR_X0: f32 = -44.0;
    const HANGAR_X1: f32 = 18.0;
    const EAVES: f32 = 8.5;
    const RIDGE: f32 = 13.0;
    let y0 = AIR_HANGAR_Y;

    if b.coarse() {
        b.paint(PLATING_DARK);
        b.face(&[v3(-46.0, -26.0, 0.3), v3(46.0, -26.0, 0.3), v3(46.0, 26.0, 0.3), v3(-46.0, 26.0, 0.3)]);
        b.paint(PLATING);
        b.mirror_y(|b| {
            b.frustum_open(
                v3((HANGAR_X0 + HANGAR_X1) * 0.5, y0 + 9.0, 0.0),
                v2(HANGAR_X1 - HANGAR_X0, 18.0),
                v2(HANGAR_X1 - HANGAR_X0, 14.0),
                RIDGE,
                v2(0.0, -2.0),
            )
        });
        b.cuboid_open(v3(AIR_GANTRY_X, 0.0, AIR_GANTRY_Z + 1.2), v3(3.0, 54.0, 2.4));
        b.cuboid_open(AIR_TOWER + Vec3::Z * 12.0, v3(7.0, 7.0, 24.0));
        team_panel(b, v3(-13.0, y0 + 7.0, RIDGE), v2(20.0, 3.0));
        if tech >= 2 {
            let top = if tech >= 3 { 40.0 } else { 31.0 };
            b.paint(METAL);
            b.face(&[
                AIR_TOWER + v3(-0.5, 0.0, 24.0),
                AIR_TOWER + v3(0.5, 0.0, 24.0),
                AIR_TOWER + v3(0.5, 0.0, top),
                AIR_TOWER + v3(-0.5, 0.0, top),
            ]);
        }
        return;
    }

    // Deck: a graphite apron across the lot, the lift let into it.
    b.paint(PLATING_DARK);
    apron(b, v2(-46.0, -26.0), v2(46.0, 26.0), v2(17.3, 17.3), 0.3);
    factory_pad(b, 15.0, 15.0);

    // Launch lane: white, chevrons toward the way out, lights that run while it builds.
    b.paint(ACCENT);
    b.frustum(v3(31.6, 0.0, 0.3), v2(28.8, 26.0), v2(28.0, 25.0), 0.3, v2(0.0, 0.0));
    b.paint(PLATING).pattern(pattern::ROADWAY);
    b.plate(v3(31.6, 0.0, 0.6), v2(27.6, 23.0), 0.1, 0.04);

    // Hangars: white shells on a black plinth, shutters facing the deck.
    b.mirror_y(|b| {
        b.paint(PLATING);
        b.extrude_x(
            &[
                [y0, 0.0],
                [y0 + 18.0, 0.0],
                [y0 + 18.0, EAVES],
                [y0 + 16.0, EAVES + 1.2],
                [y0 + 1.0, RIDGE],
                [y0, RIDGE - 0.6],
            ],
            HANGAR_X0,
            HANGAR_X1,
        );
        b.paint(ACCENT);
        b.block(v3(HANGAR_X0 - 0.3, y0 - 0.3, 0.0), v3(HANGAR_X1 + 0.3, y0 + 18.3, 1.4));
        // Gable ends: black edging along the roofline.
        let edged: &[f32] = if b.fine() { &[HANGAR_X0 - 0.2, HANGAR_X1 + 0.2] } else { &[] };
        for &x in edged {
            b.beam(v3(x, y0 - 0.2, RIDGE - 0.4), v3(x, y0 + 1.0, RIDGE + 0.2), v2(0.8, 0.6), v2(0.8, 0.6));
            b.beam(v3(x, y0 + 1.0, RIDGE + 0.2), v3(x, y0 + 16.0, EAVES + 1.4), v2(0.8, 0.6), v2(0.8, 0.6));
            b.beam(v3(x, y0 + 16.0, EAVES + 1.4), v3(x, y0 + 18.3, EAVES + 0.2), v2(0.8, 0.6), v2(0.8, 0.6));
        }
        // Roof: a team band down the long slope, vents beside it.
        on_slope_y(b, [y0 + 16.0, EAVES + 1.2], [y0 + 1.0, RIDGE], 0.5, |b| {
            b.paint(PLATING).pattern(pattern::TEAM_BAND);
            b.plate(v3(-13.0, -2.5, 0.0), v2(56.0, 3.2), 0.14, 0.05);
            team_panel(b, v3(12.0, 3.4, 0.0), v2(6.0, 3.0));
        });
        // Deck face: black pilasters between three shutter bays, conduit over them.
        b.paint(ACCENT);
        for x in [-40.0, -22.0, -4.0, 14.0] {
            b.block(v3(x - 0.9, y0 - 0.6, 0.0), v3(x + 0.9, y0 + 0.2, RIDGE - 0.8));
        }
        // Between them the hangar stands open: a dark mouth, lights let into it.
        for x in [-31.0, -13.0, 5.0] {
            b.paint(ACCENT);
            b.block(v3(x - 8.0, y0 - 0.2, 1.4), v3(x + 8.0, y0 + 0.1, 8.6));
        }
        conduit(b, v3(-41.0, y0 - 0.9, 9.0), v3(15.0, y0 - 0.1, 10.0));
        if b.fine() {
            on_slope_y(b, [y0 + 16.0, EAVES + 1.2], [y0 + 1.0, RIDGE], 0.5, |b| {
                for x in [-31.0, -13.0, 5.0] {
                    vent(b, v3(x, 3.2, 0.0), v2(3.6, 2.0), 3, ACCENT);
                }
            });
            // Outer wall: a band of black at the eaves, lights let into it.
            b.paint(ACCENT);
            b.block(v3(HANGAR_X0 + 1.0, y0 + 18.0, EAVES - 1.6), v3(HANGAR_X1 - 1.0, y0 + 18.25, EAVES - 0.4));
        }
    });

    // Blast fence behind the deck: white plates raked toward it on black ribs.
    b.paint(ACCENT);
    b.block(v3(-31.0, -24.0, 0.0), v3(-29.6, 24.0, 1.2));
    on_slope(b, [-25.2, 6.8], [-30.2, 1.1], 0.5, |b| {
        b.paint(PLATING);
        b.plate(Vec3::ZERO, v2(7.4, 46.0), 0.25, 0.08);
        if b.fine() {
            b.paint(ACCENT).pattern(pattern::CONDUIT);
            b.plate(v3(0.0, 0.0, 0.25), v2(1.0, 40.0), 0.08, 0.03);
        }
    });
    if b.fine() {
        for y in [-22.0, -11.0, 0.0, 11.0, 22.0] {
            b.paint(ACCENT);
            b.extrude_y(&[[-31.2, 0.0], [-29.0, 0.0], [-24.6, 6.8], [-26.0, 7.2]], y - 0.4, y + 0.4);
        }
    }

    // Portal gantry over the deck's back edge.
    portal_gantry(b, AIR_GANTRY_X, 23.6, 0.0, AIR_GANTRY_Z);
    // The carriage: a black trolley under the bridge, the fabricators hung from it.
    b.paint(ACCENT);
    b.cuboid(v3(AIR_GANTRY_X, 0.0, AIR_GANTRY_Z - 0.6), v3(4.2, 16.0, 1.2));
    b.paint(PLATING);
    b.mirror_y(|b| b.plate(v3(AIR_GANTRY_X, 2.0, AIR_GANTRY_Z), v2(4.4, 2.6), 0.1, 0.04));
    for (mount, s, aim) in heads("factory_air", 1) {
        fabricator(b, mount, aim, s, AIR_GANTRY_Z - 1.0);
    }

    // Control tower beside the lane.
    factory_tower(b, AIR_TOWER, 0.0, 24.0);
    beacon(b, AIR_TOWER + v3(2.2, -2.6, 24.14));
    if b.fine() {
        antenna(b, AIR_TOWER + v3(-2.5, 3.0, 24.0), 3.6, 0.06);
        b.paint(ACCENT).pattern(pattern::CONDUIT);
        b.block(AIR_TOWER + v3(4.3, -2.0, 2.0), AIR_TOWER + v3(4.65, 2.0, 16.0));
    }

    // Suite II: a second portal over the deck's front edge, fabricators on the hangars'
    // deck faces, drums behind the fence, a radar mast.
    kit(b, tech, 2, 0.22, |b| {
        portal_gantry(b, 19.5, 23.6, 0.3, AIR_GANTRY_Z);
        b.mirror_y(|b| {
            b.paint(ACCENT);
            b.block(v3(-4.0, y0 - 3.0, 9.0), v3(4.0, y0 - 0.6, 10.2));
            b.paint(PLATING);
            b.block(v3(-3.8, y0 - 2.9, 10.2), v3(3.8, y0 - 0.7, 10.34));
        });
        for (mount, s, aim) in heads("factory_air", 2) {
            fabricator(b, mount, aim, s, 10.34);
        }
        for y in [-13.0, 13.0] {
            fabricator_tank(b, v3(-38.0, y, 0.3), 3.6, 9.0);
            b.paint(METAL);
            b.cylinder_between(v3(-34.6, y, 2.0), v3(-30.4, y * 0.8, 1.0), 0.4, 0.4, 6);
        }
        b.paint(ACCENT);
        b.prism(AIR_TOWER + v3(-0.4, 0.0, 24.14), 8, 2.6, 2.0, 1.6);
        b.paint(METAL);
        b.prism(AIR_TOWER + v3(-0.4, 0.0, 25.7), 6, 0.5, 0.25, 5.0);
        b.paint(PLATING);
        b.beam(AIR_TOWER + v3(-0.4, -2.4, 29.4), AIR_TOWER + v3(-0.4, 2.4, 29.4), v2(0.3, 1.1), v2(0.3, 1.1));
    });

    // Suite III: cantilever gantries on the hangar roofs with fabricators of their
    // own, and the tower's mast raised.
    kit(b, tech, 3, 0.3, |b| {
        // Roof lanterns along each hangar's ridge, lit along the deck side.
        b.mirror_y(|b| {
            for (x0, x1) in [(-40.0, -12.0), (-3.0, 14.0)] {
                let cx = (x0 + x1) * 0.5;
                b.paint(PLATING);
                b.cuboid_open(v3(cx, y0 + 5.0, 13.3), v3(x1 - x0, 4.0, 3.8));
                b.paint(ACCENT);
                b.cuboid(v3(cx, y0 + 5.0, 15.0), v3(x1 - x0 + 0.3, 4.3, 0.5));
                b.paint(PLATING).pattern(pattern::TEAM_BAND);
                b.plate(v3(cx, y0 + 5.0, 15.25), v2(x1 - x0 - 1.0, 3.4), 0.12, 0.04);
                b.paint(GLOW);
                b.cuboid(v3(cx, y0 + 2.96, 13.9), v3(x1 - x0 - 2.0, 0.1, 0.6));
            }
        });
        b.mirror_y(|b| {
            b.paint(PLATING);
            b.cuboid_open(v3(-8.0, y0 + 4.0, RIDGE - 1.0), v3(4.2, 4.2, 2.0));
            b.paint(ACCENT);
            b.frustum_open(v3(-8.0, y0 + 4.0, RIDGE), v2(3.0, 3.0), v2(2.2, 2.2), 11.8, v2(0.0, -0.4));
            conduit(b, v3(-9.8, y0 + 2.6, RIDGE + 1.0), v3(-9.45, y0 + 4.4, 23.0));
            b.paint(ACCENT);
            b.cuboid(v3(-8.0, y0 - 5.0, 24.2), v3(2.4, 21.0, 0.6));
            b.paint(PLATING);
            b.chamfered_box(v3(-8.0, y0 - 5.0, 25.4), v3(2.8, 22.0, 1.8), 0.4);
            b.paint(ACCENT);
            b.cuboid(v3(-8.0, 17.0, 23.2), v3(3.2, 3.6, 1.4));
        });
        for (mount, s, aim) in heads("factory_air", 3) {
            fabricator(b, mount, aim, s, 22.6);
        }
        b.paint(PLATING);
        b.cuboid_open(AIR_TOWER + v3(-0.4, 0.0, 32.5), v3(1.8, 1.8, 4.0));
        b.paint(METAL);
        b.prism(AIR_TOWER + v3(-0.4, 0.0, 34.5), 6, 0.4, 0.15, 4.0);
    });

    if tech >= 2 {
        b.paint(GLOW);
        b.mirror_y(|b| b.cuboid(v3(-13.0, y0 + 18.3, EAVES - 1.0), v3(40.0, 0.2, 0.3)));
        if b.fine() {
            antenna(b, AIR_TOWER + v3(-3.0, -3.2, 24.0), 2.8, 0.1);
        }
    }
    if tech >= 3 {
        b.paint(GLOW);
        b.prism(AIR_TOWER + v3(-0.4, 0.0, 38.5), 6, 0.35, 0.15, 0.8);
        if b.fine() {
            b.mirror_y(|b| glow_strip(b, v3(-8.0, y0 - 5.0, 26.3), v2(1.2, 16.0), GLOW));
        }
    }
}

// ---- Wharf: naval factory --------------------------------------------------------

/// The quay's berth face: everything the Wharf has stands behind it (-y).
const QUAY_FACE: f32 = -24.0;
const QUAY_BACK: f32 = -46.0;
const QUAY_X: f32 = 44.0;
/// Deck level over the water (a naval structure's origin is on the water).
const DECK: f32 = 5.0;
/// How far the piles run down: past the seabed of any water the Wharf stands in.
const PILE_FOOT: f32 = -90.0;
const CRANE_X: [f32; 2] = [-24.0, 20.0];
const CRANE_Y: f32 = -31.0;
const CRANE_Z: f32 = 21.0;
const JIB_TIP: f32 = -11.0;
const WHARF_TOWER: Vec3 = Vec3::new(35.0, -38.0, DECK);
/// The suite III hammerhead crane: where its tower stands, and the top of that tower.
const HAMMER: Vec3 = Vec3::new(0.0, -37.0, 33.0);

/// Naval yard: a one-sided quay on piles driven into the seabed. The berth is open
/// water at the lot origin, where the hull floats while it is printed; nothing stands
/// on the far side of it, so a ship longer or wider than the yard still fits. Cranes
/// reach out over the berth to hang their fabricators over the hull.
pub fn factory_naval(b: &mut MeshBuilder, tech: u8) {
    let quay_mid = (QUAY_FACE + QUAY_BACK) * 0.5;
    let quay_w = QUAY_FACE - QUAY_BACK;
    // Works hall at the back of the quay: x0..x1 along it, its berth side at `front`.
    let (hx0, hx1, back, front) = (-40.0, -6.0, -45.0, -34.0);
    let hall_top = DECK + 11.0;

    if b.coarse() {
        b.paint(PLATING_DARK);
        b.cuboid_open(v3(0.0, quay_mid, (DECK + PILE_FOOT) * 0.5), v3(QUAY_X * 2.0, quay_w, DECK - PILE_FOOT));
        b.paint(PLATING);
        b.cuboid_open(v3((hx0 + hx1) * 0.5, (back + front) * 0.5, DECK + 5.5), v3(hx1 - hx0, front - back, 11.0));
        b.cuboid_open(WHARF_TOWER + Vec3::Z * 10.4, v3(7.0, 7.0, 20.8));
        for x in CRANE_X {
            b.cuboid_open(v3(x, CRANE_Y, CRANE_Z * 0.5), v3(3.0, 3.0, CRANE_Z));
            b.face(&[
                v3(x - 1.2, CRANE_Y - 10.0, CRANE_Z + 1.2),
                v3(x + 1.2, CRANE_Y - 10.0, CRANE_Z + 1.2),
                v3(x + 1.2, JIB_TIP, CRANE_Z + 1.2),
                v3(x - 1.2, JIB_TIP, CRANE_Z + 1.2),
            ]);
        }
        team_panel(b, v3((hx0 + hx1) * 0.5, -41.0, hall_top), v2(18.0, 5.0));
        if tech == 2 {
            b.paint(METAL);
            b.face(&[
                WHARF_TOWER + v3(-0.5, 0.0, 20.8),
                WHARF_TOWER + v3(0.5, 0.0, 20.8),
                WHARF_TOWER + v3(0.5, 0.0, 30.7 - DECK),
                WHARF_TOWER + v3(-0.5, 0.0, 30.7 - DECK),
            ]);
        }
        if tech >= 3 {
            b.paint(PLATING);
            b.face(&[
                v3(HAMMER.x - 2.0, HAMMER.y, DECK),
                v3(HAMMER.x + 2.0, HAMMER.y, DECK),
                v3(HAMMER.x + 2.0, HAMMER.y, HAMMER.z + 7.0),
                v3(HAMMER.x - 2.0, HAMMER.y, HAMMER.z + 7.0),
            ]);
        }
        return;
    }

    // Piles, down past the seabed: a row standing proud of the berth face, capped at
    // the coping, and a row under the back of the deck. A wale ties each row at the
    // waterline, and cross-bracing runs between them, half in the water.
    let piles = [-40.0, -24.0, -8.0, 8.0, 24.0, 40.0];
    let (front_row, back_row) = (QUAY_FACE + 1.3, QUAY_BACK + 3.0);
    for x in piles {
        b.paint(PLATING_DARK).pattern(pattern::PILE);
        b.cylinder_between(v3(x, front_row, PILE_FOOT), v3(x, front_row, DECK + 0.5), 1.2, 1.2, b.sides(8));
        b.paint(PLATING_DARK).pattern(pattern::PILE);
        b.cylinder_between(v3(x, back_row, PILE_FOOT), v3(x, back_row, DECK - 1.6), 1.1, 1.1, b.sides(8));
        b.paint(ACCENT);
        b.cuboid(v3(x, front_row, DECK + 0.7), v3(3.0, 3.0, 0.5));
        if b.fine() {
            b.paint(PLATING);
            b.plate(v3(x, front_row, DECK + 0.95), v2(2.6, 2.6), 0.12, 0.04);
            b.paint(ACCENT);
            b.cuboid(v3(x, back_row, DECK - 2.0), v3(3.0, 3.0, 0.8));
            b.paint(METAL);
            b.beam(v3(x, front_row, DECK - 2.2), v3(x, back_row, -8.0), v2(0.5, 0.5), v2(0.5, 0.5));
            b.beam(v3(x, back_row, DECK - 2.2), v3(x, front_row, -8.0), v2(0.5, 0.5), v2(0.5, 0.5));
        }
    }
    for y in [front_row, back_row] {
        b.paint(PLATING_DARK).pattern(pattern::PILE);
        b.cuboid(v3(0.0, y, 0.3), v3(QUAY_X * 2.0 - 4.0, 1.0, 1.4));
    }
    if b.fine() {
        // Walings along the exposed row, level with the deck's underside.
        b.paint(METAL);
        b.cuboid(v3(0.0, front_row, DECK - 2.4), v3(QUAY_X * 2.0 - 4.0, 0.6, 0.6));
    }

    // The deck: a graphite slab, a white deck on it, a hazard-striped coping along
    // the berth, and a conduit run on the berth face under it.
    b.paint(PLATING_DARK);
    b.cuboid(v3(0.0, quay_mid, DECK - 1.0), v3(QUAY_X * 2.0, quay_w, 2.0));
    b.paint(PLATING);
    b.plate(v3(0.0, quay_mid - 1.0, DECK), v2(QUAY_X * 2.0 - 1.0, quay_w - 3.0), 0.12, 0.04);
    b.paint(PLATING).pattern(pattern::HAZARD);
    b.block(v3(-QUAY_X, QUAY_FACE - 2.4, DECK), v3(QUAY_X, QUAY_FACE + 0.3, DECK + 0.7));
    conduit(b, v3(-QUAY_X + 1.0, QUAY_FACE - 0.1, DECK - 1.8), v3(QUAY_X - 1.0, QUAY_FACE + 0.35, DECK - 0.8));
    // Fenders on the berth face between the piles, down into the water.
    b.paint(TREAD);
    for x in [-32.0, -16.0, 0.0, 16.0, 32.0] {
        b.cylinder_between(v3(x, QUAY_FACE + 0.9, -2.6), v3(x, QUAY_FACE + 0.9, DECK - 0.4), 0.9, 0.9, b.sides(8));
    }
    if b.fine() {
        // Bollards along the coping, lamps at its ends, ladders down the face.
        for x in [-40.0, -28.0, -12.0, 4.0, 16.0, 32.0] {
            b.paint(ACCENT);
            b.cylinder_between(v3(x, QUAY_FACE - 1.2, DECK + 0.7), v3(x, QUAY_FACE - 1.2, DECK + 1.5), 0.45, 0.55, 8);
            b.cylinder_between(v3(x, QUAY_FACE - 1.2, DECK + 1.5), v3(x, QUAY_FACE - 1.2, DECK + 1.7), 0.7, 0.7, 8);
        }
        for x in [-42.5, 42.5] {
            beacon(b, v3(x, QUAY_FACE - 1.0, DECK + 0.7));
        }
        for x in [-30.0, 22.0] {
            b.paint(METAL);
            for dx in [-0.35, 0.35] {
                b.block(v3(x + dx - 0.06, QUAY_FACE + 0.3, -3.0), v3(x + dx + 0.06, QUAY_FACE + 0.42, DECK + 0.7));
            }
            for k in 0..6 {
                let z = -2.5 + k as f32 * 1.0;
                b.block(v3(x - 0.35, QUAY_FACE + 0.32, z), v3(x + 0.35, QUAY_FACE + 0.4, z + 0.08));
            }
        }
    }

    // Works hall: white, a black plinth and cornice, shutters on the berth side.
    b.paint(PLATING);
    b.extrude_x(
        &[
            [back, DECK],
            [front, DECK],
            [front, DECK + 8.4],
            [front - 2.4, hall_top],
            [back + 1.0, hall_top],
            [back, hall_top - 1.4],
        ],
        hx0,
        hx1,
    );
    b.paint(ACCENT);
    b.block(v3(hx0 - 0.3, back - 0.3, DECK), v3(hx1 + 0.3, front + 0.3, DECK + 1.3));
    b.block(v3(hx0 - 0.25, front - 1.6, DECK + 8.0), v3(hx1 + 0.25, front + 0.25, DECK + 8.8));
    b.paint(PLATING).pattern(pattern::TEAM_BAND);
    b.plate(v3((hx0 + hx1) * 0.5, -42.0, hall_top), v2(hx1 - hx0 - 2.0, 3.8), 0.14, 0.05);
    for x in [-32.0, -15.0] {
        b.paint(ACCENT);
        b.block(v3(x - 6.4, front, DECK), v3(x + 6.4, front + 0.35, DECK + 7.6));
        b.paint(ACCENT).pattern(pattern::CONDUIT);
        b.block(v3(x - 5.8, front + 0.1, DECK + 1.2), v3(x + 5.8, front + 0.45, DECK + 6.6));
    }
    if b.fine() {
        vent(b, v3(-33.0, -38.3, hall_top), v2(4.0, 2.4), 3, ACCENT);
        vent(b, v3(-13.0, -38.3, hall_top), v2(4.0, 2.4), 3, ACCENT);
        b.paint(ACCENT).pattern(pattern::FURNACE);
        b.plate(v3(-23.0, -38.3, hall_top), v2(5.0, 2.4), 0.08, 0.04);
        // Pilasters along the back wall.
        for x in [-34.0, -23.0, -12.0] {
            b.paint(ACCENT);
            b.block(v3(x - 0.6, back - 0.35, DECK + 1.3), v3(x + 0.6, back + 0.1, hall_top - 1.6));
        }
    }

    // Fabricator cranes: a white tower on a black plinth, a slewing house, a jib out
    // over the berth with the fabricator hung under its tip, a counter-jib and weight.
    for x in CRANE_X {
        let foot = v3(x, CRANE_Y, DECK);
        b.paint(ACCENT);
        b.cuboid_open(foot + Vec3::Z * 0.7, v3(5.0, 5.0, 1.4));
        b.paint(PLATING);
        b.cuboid_open(foot + Vec3::Z * (1.4 + (CRANE_Z - DECK - 3.4) * 0.5), v3(3.0, 3.0, CRANE_Z - DECK - 3.4));
        if b.fine() {
            // The tower's lattice, drawn as black bracing on each white face.
            for n in [Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y] {
                b.paint(ACCENT);
                let side = Vec3::Z.cross(n) * 1.25;
                let at = foot + n * 1.56;
                b.beam(at + side + Vec3::Z * 2.0, at - side + Vec3::Z * 8.6, v2(0.3, 0.3), v2(0.3, 0.3));
                b.beam(at - side + Vec3::Z * 8.6, at + side + Vec3::Z * 15.2, v2(0.3, 0.3), v2(0.3, 0.3));
            }
        }
        // Slewing house, glazed toward the berth.
        b.paint(ACCENT);
        b.cuboid(v3(x, CRANE_Y - 1.0, CRANE_Z - 1.2), v3(4.0, 6.0, 2.4));
        b.paint(PLATING);
        b.plate(v3(x, CRANE_Y - 1.4, CRANE_Z), v2(3.6, 5.0), 0.12, 0.04);
        team_panel(b, v3(x, CRANE_Y - 2.6, CRANE_Z + 0.12), v2(2.6, 2.0));
        if b.fine() {
            b.paint(GLASS);
            b.block(v3(x - 1.6, CRANE_Y + 1.95, CRANE_Z - 2.0), v3(x + 1.6, CRANE_Y + 2.05, CRANE_Z - 0.9));
        }
        // Jib out to the tip over the berth; counter-jib back over the quay, its weight.
        b.paint(PLATING);
        b.beam(v3(x, CRANE_Y + 2.0, CRANE_Z + 0.8), v3(x, JIB_TIP, CRANE_Z + 0.5), v2(2.2, 1.8), v2(1.6, 1.3));
        b.paint(ACCENT);
        b.beam(v3(x, CRANE_Y + 2.0, CRANE_Z - 0.35), v3(x, JIB_TIP + 0.4, CRANE_Z - 0.05), v2(1.8, 0.5), v2(1.3, 0.4));
        b.beam(v3(x, CRANE_Y - 4.0, CRANE_Z + 0.4), v3(x, CRANE_Y - 10.0, CRANE_Z + 0.4), v2(1.8, 1.2), v2(1.8, 1.2));
        b.paint(PLATING_DARK);
        b.cuboid(v3(x, CRANE_Y - 10.5, CRANE_Z - 0.6), v3(3.0, 3.0, 3.2));
        conduit(b, v3(x - 0.2, CRANE_Y + 2.0, CRANE_Z + 1.62), v3(x + 0.2, JIB_TIP + 1.5, CRANE_Z + 1.75));
        // A mast over the house, stays to both ends.
        b.paint(ACCENT);
        b.frustum(v3(x, CRANE_Y - 1.0, CRANE_Z + 0.1), v2(1.2, 1.2), v2(0.4, 0.4), 4.5, v2(0.0, 0.0));
        if b.fine() {
            b.paint(METAL);
            b.cylinder_between(v3(x, CRANE_Y - 1.0, CRANE_Z + 4.4), v3(x, JIB_TIP + 3.0, CRANE_Z + 1.3), 0.1, 0.1, 4);
            b.cylinder_between(v3(x, CRANE_Y - 1.0, CRANE_Z + 4.4), v3(x, CRANE_Y - 10.0, CRANE_Z + 1.0), 0.1, 0.1, 4);
            antenna(b, v3(x + 0.35, CRANE_Y - 1.0, CRANE_Z + 4.5), 2.0, 0.0);
        }
        // Tip block, the fabricator hung under it.
        b.paint(ACCENT);
        b.cuboid(v3(x, JIB_TIP - 0.6, CRANE_Z - 0.2), v3(2.4, 2.4, 1.4));
    }
    for (mount, s, aim) in heads("factory_naval", 1) {
        fabricator(b, mount, aim, s, CRANE_Z - 0.7);
    }

    // Harbour tower at the far end of the quay.
    factory_tower(b, WHARF_TOWER, 0.0, 24.0 - DECK);
    beacon(b, WHARF_TOWER + v3(2.2, -2.6, 24.14 - DECK));
    if b.fine() {
        antenna(b, WHARF_TOWER + v3(-2.5, 3.0, 24.0 - DECK), 3.6, 0.06);
    }

    // Suite II: fabricators on the coping, drums on the deck, the tower's radar mast.
    kit(b, tech, 2, 0.22, |b| {
        // A fabrication gallery along the berth, on posts, under the crane jibs.
        let gz = DECK + 10.2;
        for x in [-36.0, -16.0, 14.0, 36.0] {
            b.paint(ACCENT);
            b.cuboid_open(v3(x, -27.0, DECK + (gz - 1.0 - DECK) * 0.5), v3(1.3, 1.3, gz - 1.0 - DECK));
        }
        b.paint(ACCENT);
        b.cuboid(v3(0.0, -27.0, gz - 0.8), v3(83.0, 2.0, 0.5));
        b.paint(PLATING);
        b.chamfered_box(v3(0.0, -27.0, gz), v3(84.0, 2.6, 1.6), 0.4);
        conduit(b, v3(-40.0, -25.75, gz - 0.55), v3(40.0, -25.55, gz + 0.35));
        glow_strip(b, v3(0.0, -27.0, gz + 0.8), v2(70.0, 0.4), GLOW);
        // An upper storey on the works hall.
        b.paint(PLATING);
        b.cuboid_open(v3(-27.0, -42.15, hall_top + 3.0), v3(18.0, 4.7, 6.0));
        b.paint(ACCENT);
        b.cuboid(v3(-27.0, -42.15, hall_top + 5.63), v3(18.3, 5.0, 0.7));
        b.paint(ACCENT).pattern(pattern::CONDUIT);
        b.block(v3(-35.0, -39.8, hall_top + 1.2), v3(-19.0, -39.65, hall_top + 4.4));
        b.paint(PLATING).pattern(pattern::TEAM_BAND);
        b.plate(v3(-27.0, -42.15, hall_top + 6.0), v2(17.0, 4.0), 0.12, 0.04);
        for (mount, s, aim) in heads("factory_naval", 2) {
            b.paint(ACCENT);
            b.block(v3(mount.x - 1.6, QUAY_FACE - 3.4, DECK + 0.7), v3(mount.x + 1.6, QUAY_FACE - 0.4, DECK + 2.6));
            b.paint(PLATING);
            b.block(v3(mount.x - 1.5, QUAY_FACE - 3.3, DECK + 2.6), v3(mount.x + 1.5, QUAY_FACE - 0.5, DECK + 2.74));
            fabricator(b, mount, aim, s, DECK + 2.74);
        }
        for x in [9.0, 15.5] {
            fabricator_tank(b, v3(x, -41.5, DECK + 0.12), 2.9, 7.0);
        }
        b.paint(METAL);
        b.cylinder_between(v3(6.1, -41.5, DECK + 1.2), v3(hx1, -41.5, DECK + 1.2), 0.35, 0.35, 6);
        b.paint(ACCENT);
        b.prism(WHARF_TOWER + v3(-0.4, 0.0, 24.14 - DECK), 8, 2.6, 2.0, 1.6);
        b.paint(METAL);
        b.prism(WHARF_TOWER + v3(-0.4, 0.0, 25.7 - DECK), 6, 0.5, 0.25, 5.0);
        b.paint(PLATING);
        b.beam(
            WHARF_TOWER + v3(-0.4, -2.4, 29.4 - DECK),
            WHARF_TOWER + v3(-0.4, 2.4, 29.4 - DECK),
            v2(0.3, 1.1),
            v2(0.3, 1.1),
        );
    });

    // Suite III: a hammerhead crane amidships, taller than the rest, with a trolley
    // of two fabricators run out over the berth.
    kit(b, tech, 3, 0.3, |b| {
        let (x, y, top) = (HAMMER.x, HAMMER.y, HAMMER.z);
        let jib_z = top + 1.0;
        b.paint(ACCENT);
        b.cuboid_open(v3(x, y, DECK + 0.8), v3(6.4, 6.4, 1.6));
        b.paint(PLATING);
        b.frustum_open(v3(x, y, DECK + 1.6), v2(4.4, 4.4), v2(3.2, 3.2), top - DECK - 3.8, v2(0.0, 0.0));
        conduit(b, v3(x + 1.9, y - 0.6, DECK + 3.0), v3(x + 2.2, y + 0.6, top - 5.0));
        // Slewing house, the jib from the quay's back edge out over the berth.
        b.paint(ACCENT);
        b.cuboid(v3(x, y, top - 1.2), v3(5.0, 7.0, 2.8));
        b.paint(PLATING);
        b.chamfered_box(v3(x, (-44.0 + -4.5) * 0.5, jib_z), v3(3.0, 39.5, 2.2), 0.5);
        b.paint(ACCENT);
        b.cuboid(v3(x, (-40.0 + -5.0) * 0.5, jib_z - 1.3), v3(2.4, 35.0, 0.4));
        b.paint(PLATING).pattern(pattern::TEAM_BAND);
        b.plate(v3(x, -20.0, jib_z + 1.1), v2(2.2, 26.0), 0.1, 0.04);
        // Counterweight under the jib's back end, a mast over the house.
        b.paint(PLATING_DARK);
        b.cuboid(v3(x, -43.8, jib_z - 2.8), v3(4.0, 4.0, 3.6));
        b.paint(ACCENT);
        b.frustum(v3(x, y, jib_z + 1.1), v2(1.4, 1.4), v2(0.4, 0.4), 5.0, v2(0.0, 0.0));
        // The trolley: a cross-beam under the jib carrying the pair.
        b.paint(ACCENT);
        b.cuboid(v3(x, -7.0, jib_z - 1.9), v3(12.0, 3.4, 1.6));
        b.paint(PLATING);
        b.plate(v3(x, -7.0, jib_z - 1.1), v2(11.4, 3.0), 0.1, 0.04);
        for (mount, s, aim) in heads("factory_naval", 3) {
            fabricator(b, mount, aim, s, jib_z - 2.5);
        }
        // The harbour tower's mast raised.
        b.paint(PLATING);
        b.cuboid_open(WHARF_TOWER + v3(-0.4, 0.0, 32.5 - DECK), v3(1.8, 1.8, 4.0));
        b.paint(METAL);
        b.prism(WHARF_TOWER + v3(-0.4, 0.0, 34.5 - DECK), 6, 0.4, 0.15, 4.0);
    });

    if tech >= 2 {
        b.paint(GLOW);
        b.cuboid(v3((hx0 + hx1) * 0.5, front + 0.35, DECK + 8.4), v3(hx1 - hx0 - 1.0, 0.2, 0.3));
        if b.fine() {
            antenna(b, WHARF_TOWER + v3(-3.0, -3.2, 24.0 - DECK), 2.8, 0.1);
        }
    }
    if tech >= 3 {
        b.paint(GLOW);
        b.prism(v3(HAMMER.x, HAMMER.y, HAMMER.z + 7.1), 6, 0.35, 0.15, 0.8);
        if b.fine() {
            glow_strip(b, v3(HAMMER.x, -30.0, HAMMER.z + 2.2), v2(1.0, 12.0), GLOW);
        }
    }
}
