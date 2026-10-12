//! The Bardiche, the Regency's tech 3 lance anti-air (`regency_t3_lance_aa`): a low
//! armoured hull on gravity lift, no legs, so it crosses water as it crosses land, and on
//! a turret over its middle one long Pinch lance. Over a short charge a knot of plasma
//! is squeezed between the tines at the lance's end, then pinched out as one fast bolt.
//! The unit file's `muzzle` is that knot, held between the tines; its `pivot` is the
//! lance's trunnion.
//!
//! Three designs while one is picked, differing in the hull, how the lance is carried and
//! how its tines hold the charge:
//!
//! - **A** (`split`, `regency_lance_aa`): a long pointed wedge, the lance carried low
//!   between two tall split cheeks, a pair of flat tines either side of the charge.
//! - **B** (`yoke`, `regency_lance_aa~b`): a broad blunt hull lapped in feathered plates
//!   down its flanks, the lance hung in a tall keeled yoke over a capacitor housing, two
//!   rails above and below its bore reaching past the mouth to hold the charge.
//! - **C** (`sheath`, `regency_lance_aa~c`): a slim hull with a raked dorsal keel aft,
//!   the lance sheathed in plates lapped back like feathers, its mouth three claws closed
//!   round the charge, carried in a low house between two raked fins.
//!
//! Finish (docs/STYLE.md "The Regency look"): dark plates, seam-dark joints, dark bronze on
//! the machinery. The red is the plates' own (`pattern::EMBER_SLOT`): slots let into
//! them, never a lit piece of mesh. No violet: it does not build.
//!
//! Rig: a hovercraft (`MeshBuilder::set_hover`): the shader heaves the hull over its lift
//! bells (`part::LOCOMOTION`), each marked for its red plasma (`add_lift`). The turret
//! turns about the unit's middle; the lance pitches about its trunnion (`rig::ARM_GUN`),
//! resting raised at the sky (`combat.rs` `IDLE_SKY_PITCH`). The numbers match
//! `regency_t3_lance_aa` in `data/factions/regency/units/land_t3.ron`.

mod sheath;
mod split;
mod yoke;

#[cfg(test)]
mod tests;

use glam::{Vec2, Vec3};

use crate::builder::{MeshBuilder, Section};
use crate::part;

use super::kit::{dark_plate, metal, seam, v3};
use super::parts::inlay;

pub(super) use sheath::sheath;
pub(super) use split::split;
pub(super) use yoke::yoke;

pub(super) const RADIUS: f32 = 7.0;
pub(super) const HEIGHT: f32 = 6.0;
/// The lance's trunnion and the knot of plasma held between its tines: the unit file's
/// weapon `pivot` and `muzzle`.
const PIVOT: Vec3 = Vec3::new(0.4, 0.0, 3.4);
#[cfg(test)]
const MUZZLE: Vec3 = Vec3::new(8.6, 0.0, 3.4);
/// How far round the charge the tines stand: none closer than 0.4 of it.
#[cfg(test)]
const HOLD: f32 = 1.0;
/// Where the turret turns on the hull.
const DECK: f32 = 2.5;
/// Where each lift bell's mouth opens.
const MOUTH: f32 = 0.25;
/// The lance's breech behind its trunnion: short, so the lance raised at the sky clears
/// the turret behind it.
const BREECH: f32 = PIVOT.x - 0.6;

/// The rig every design shares.
fn rig(b: &mut MeshBuilder) {
    b.set_hover();
    b.set_turret_pivot(v3(0.0, 0.0, DECK));
    b.set_arm_pivot(PIVOT);
    b.set_dust_line(0.8);
}

/// The hull lofted from `plan` in flat facets over a seam-dark belly, up to the deck.
fn hull(b: &mut MeshBuilder, plan: &[[f32; 2]]) {
    seam(b);
    b.loft_z(plan, &[Section::new(0.75, 0.82), Section::new(1.0, 0.86)]);
    dark_plate(b);
    b.with_facets(|b| {
        b.loft_z(
            plan,
            &[
                Section::new(0.95, 0.88),
                Section::new(1.6, 1.0),
                Section::new(2.2, 0.9),
                Section::new(DECK, 0.62).shifted(-0.6, 0.0),
            ],
        );
    });
}

/// A lift drum at each of `at` (left side; the right's are their mirrors), a lift bell in
/// each marked for its plasma.
fn lifts(b: &mut MeshBuilder, at: &[(f32, f32)], r: f32) {
    b.mirror_y(|b| {
        for &(x, y) in at {
            b.add_lift(v3(x, y, MOUTH), r * 0.7);
            metal(b);
            let sides = b.sides(12);
            b.prism(v3(x, y, 0.62), sides, r, r * 0.95, 0.36);
            b.with_part(part::LOCOMOTION, |b| {
                seam(b);
                b.prism(v3(x, y, 0.34), sides, r * 0.86, r * 0.84, 0.5);
                b.prism(v3(x, y, MOUTH), sides, r * 0.55, r * 0.7, 0.09);
            });
        }
    });
}

/// The team colour: a point on the deck at `x`, aft.
fn team(b: &mut MeshBuilder, x: f32, z: f32) {
    b.paint(crate::material::TEAM);
    b.face(&[
        v3(x, 0.0, z + 0.03),
        v3(x - 1.3, 0.7, z + 0.03),
        v3(x - 1.3, -0.7, z + 0.03),
    ]);
}

/// A red slot let into a face whose outward normal is `out`: `len` along `along` about
/// `at`, `h` across.
fn slot(b: &mut MeshBuilder, at: Vec3, out: Vec3, along: Vec3, len: f32, h: f32) {
    let along = along.normalize() * (len * 0.5);
    let across = out.normalize().cross(along).normalize() * (h * 0.5);
    inlay(
        b,
        &[
            at - along - across,
            at + along - across,
            at + along + across,
            at - along + across,
        ],
        out,
        crate::pattern::EMBER_SLOT,
    );
}

/// Far off: a lozenge of a hull, a wedge of a turret, the lance a bar that still pitches
/// and its tines a pair either side of the charge, across it (`upright` false) or above
/// and below it.
fn coarse(b: &mut MeshBuilder, plan: &[[f32; 2]; 4], upright: bool) {
    dark_plate(b);
    b.loft_z(plan, &[Section::new(0.9, 0.9), Section::new(DECK, 0.7)]);
    b.with_part(part::LOCOMOTION, |b| {
        seam(b);
        b.face(&[v3(4.0, 0.0, 0.5), v3(-4.0, -2.6, 0.5), v3(-4.0, 2.6, 0.5)]);
    });
    team(b, -2.6, DECK);
    b.with_part(part::TURRET, |b| {
        dark_plate(b);
        b.loft_z(
            &[[2.0, 0.0], [-2.4, 1.7], [-2.4, -1.7]],
            &[
                Section::new(DECK, 1.0),
                Section::new(PIVOT.z + 1.5, 0.55).shifted(-0.6, 0.0),
            ],
        );
        b.with_limb(crate::rig::ARM_GUN, |b| {
            dark_plate(b);
            let z = PIVOT.z;
            b.beam(
                v3(BREECH, 0.0, z),
                v3(6.4, 0.0, z),
                Vec2::splat(0.8),
                Vec2::splat(0.6),
            );
            for s in [-1.0f32, 1.0] {
                let off = |x: f32| {
                    if upright {
                        v3(x, 0.0, z + 0.8 * s)
                    } else {
                        v3(x, 0.8 * s, z)
                    }
                };
                let (wide, thin) = if upright {
                    (Vec2::new(0.8, 0.2), Vec2::new(0.1, 0.2))
                } else {
                    (Vec2::new(0.2, 0.8), Vec2::new(0.2, 0.1))
                };
                b.beam(off(6.2), off(9.0), wide, thin);
            }
        });
    });
}
