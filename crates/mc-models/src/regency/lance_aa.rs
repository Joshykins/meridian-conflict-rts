//! The Bardiche, the Regency's tech 3 lance anti-air (`regency_t3_lance_aa`): a low
//! armoured hull on gravity lift, no legs, so it crosses water as it crosses land, and on
//! a turret over its middle one long Pinch lance. Over a short charge a knot of plasma
//! is squeezed between the tines at the lance's end, then pinched out as one fast bolt.
//! The unit file's `muzzle` is that knot, held between the tines; its `pivot` is the
//! lance's trunnion.
//!
//! The hull is a long pointed wedge lofted in flat facets, two courses of plates swept back
//! down each flank, a glacis over the nose. The lance is carried low between two tall split
//! cheeks on the turret, each lapped in a plate swept back into a spike, a low housing
//! behind them; a pair of flat tines reaches past its mouth either side of the charge,
//! lit red inside.
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

use glam::{Vec2, Vec3};

use crate::builder::{MeshBuilder, Section};
use crate::{part, rig};

use super::commander::form::{blade, ring, sleeve, OCT};
use super::kit::{dark_plate, metal, seam, v3};
use super::machine::{armour, collar, swept, Frame};
use super::parts::inlay;

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

/// A hovercraft whose turret turns about its middle and whose lance pitches about its trunnion.
fn set_rig(b: &mut MeshBuilder) {
    b.set_hover();
    b.set_turret_pivot(v3(0.0, 0.0, DECK));
    b.set_arm_pivot(PIVOT);
    b.set_dust_line(0.8);
}

/// The hull lofted in flat facets over a seam-dark belly, up to the deck.
fn hull(b: &mut MeshBuilder) {
    seam(b);
    b.loft_z(&HULL, &[Section::new(0.75, 0.82), Section::new(1.0, 0.86)]);
    dark_plate(b);
    b.with_facets(|b| {
        b.loft_z(
            &HULL,
            &[
                Section::new(0.95, 0.88),
                Section::new(1.6, 1.0),
                Section::new(2.2, 0.9),
                Section::new(DECK, 0.62).shifted(-0.6, 0.0),
            ],
        );
    });
}

/// A lift drum under each of `LIFTS` and its mirror, a lift bell in each marked for its
/// plasma.
fn lifts(b: &mut MeshBuilder) {
    let r = 1.0;

    b.mirror_y(|b| {
        for (x, y) in LIFTS {
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
/// and its tines a pair either side of the charge.
fn coarse(b: &mut MeshBuilder) {
    let plan = [[7.0, 0.0], [-1.0, 3.9], [-6.2, 0.0], [-1.0, -3.9]];
    dark_plate(b);
    b.loft_z(&plan, &[Section::new(0.9, 0.9), Section::new(DECK, 0.7)]);
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
        b.with_limb(rig::ARM_GUN, |b| {
            dark_plate(b);
            let z = PIVOT.z;
            b.beam(
                v3(BREECH, 0.0, z),
                v3(6.4, 0.0, z),
                Vec2::splat(0.8),
                Vec2::splat(0.6),
            );
            for s in [-1.0f32, 1.0] {
                b.beam(
                    v3(6.2, 0.8 * s, z),
                    v3(9.0, 0.8 * s, z),
                    Vec2::new(0.2, 0.8),
                    Vec2::new(0.2, 0.1),
                );
            }
        });
    });
}

/// The hull's plan: a pointed wedge, its stern cut in between two trailing corners.
const HULL: [[f32; 2]; 10] = [
    [7.0, 0.0],
    [5.6, 2.0],
    [2.0, 3.6],
    [-4.4, 3.9],
    [-6.4, 2.7],
    [-5.8, 0.0],
    [-6.4, -2.7],
    [-4.4, -3.9],
    [2.0, -3.6],
    [5.6, -2.0],
];
const LIFTS: [(f32, f32); 2] = [(4.0, 2.4), (-4.0, 2.8)];
/// The cheeks' inner faces either side of the lance.
const GAP: f32 = 0.6;

pub(super) fn lance_aa(b: &mut MeshBuilder, _tech: u8) {
    set_rig(b);
    if b.coarse() {
        coarse(b);
        return;
    }
    hull(b);
    nose(b);
    b.mirror_y(flank);
    lifts(b);
    b.with_part(part::TURRET, |b| {
        house(b);
        b.with_limb(rig::ARM_GUN, lance);
    });
}

/// A glacis plate over the nose, and a red optic either side of it.
fn nose(b: &mut MeshBuilder) {
    dark_plate(b);
    blade(
        b,
        v3(6.6, 0.0, 1.8),
        v3(-1.0, 0.0, 0.26),
        v3(0.3, 0.0, 1.0),
        4.2,
        1.5,
        0.0,
        0.32,
    );
    b.mirror_y(|b| {
        slot(
            b,
            v3(4.6, 2.62, 1.5),
            v3(0.5, 1.0, 0.0),
            v3(-1.0, 0.5, 0.0),
            1.4,
            0.16,
        );
    });
}

/// The left flank's plates: two courses swept back, the second running out past the
/// stern into a spike.
fn flank(b: &mut MeshBuilder) {
    dark_plate(b);
    blade(
        b,
        v3(4.6, 3.0, 1.95),
        v3(-1.0, 0.1, -0.05),
        v3(0.0, 1.0, 0.75),
        5.4,
        0.8,
        0.2,
        0.32,
    );
    blade(
        b,
        v3(0.6, 3.75, 1.8),
        v3(-1.0, 0.06, -0.04),
        v3(0.0, 1.0, 0.7),
        7.4,
        0.85,
        0.4,
        0.34,
    );
}

/// The turret: a bronze race, a low housing aft, and two tall cheeks either side of the
/// lance, each lapped in a plate swept back into a spike, a slot let into it.
fn house(b: &mut MeshBuilder) {
    metal(b);
    let sides = b.sides(16);
    b.prism(v3(0.0, 0.0, DECK - 0.05), sides, 2.4, 2.35, 0.3);
    dark_plate(b);
    b.with_facets(|b| {
        b.loft_z(
            &[
                [-1.4, 0.0],
                [-1.7, 1.9],
                [-3.3, 1.7],
                [-3.9, 0.0],
                [-3.3, -1.7],
                [-1.7, -1.9],
            ],
            &[
                Section::new(DECK + 0.2, 1.0),
                Section::new(DECK + 0.75, 0.94),
                Section::new(DECK + 1.0, 0.7).shifted(-0.3, 0.0),
            ],
        );
    });
    team(b, -1.5, DECK + 1.0);
    b.mirror_y(|b| {
        dark_plate(b);
        b.with_facets(|b| {
            b.extrude_y(
                &[
                    [-2.4, DECK + 0.2],
                    [1.9, DECK + 0.2],
                    [1.6, PIVOT.z + 0.7],
                    [0.2, PIVOT.z + 1.6],
                    [-1.0, PIVOT.z + 1.5],
                    [-2.2, PIVOT.z + 0.4],
                ],
                GAP,
                1.75,
            );
        });
        let f = Frame::new(v3(1.9, 1.78, DECK + 0.7), v3(-1.0, 0.0, 0.55), Vec3::Y);
        armour(b, &f, &swept(4.8, 0.62, -0.3, 0.55), 0.2);
        if b.fine() {
            slot(b, f.at(1.8, 0.0, 0.2), Vec3::Y, f.u, 1.8, 0.16);
        }
    });
}

/// The lance, in the turret's frame (it pitches about `PIVOT`): a tapering plated bore
/// through a bronze trunnion, bronze pinch bands round it, a plate down its back, a slot
/// down each side, and two flat tines reaching past its mouth either side of the charge,
/// lit red inside.
fn lance(b: &mut MeshBuilder) {
    let z = PIVOT.z;
    collar(b, PIVOT, Vec3::Y, 0.44, GAP * 2.0 + 0.1);
    let w = |x: f32| -> f32 {
        if x < 1.4 {
            0.45
        } else {
            0.45 - (x - 1.4) / 5.2 * 0.17
        }
    };
    dark_plate(b);
    sleeve(
        b,
        &[
            ring(v3(BREECH, 0.0, z), Vec3::Z, 0.3, 0.3),
            ring(v3(1.4, 0.0, z), Vec3::Z, w(1.4), w(1.4)),
            ring(v3(6.6, 0.0, z), Vec3::Z, w(6.6), w(6.6)),
            ring(v3(7.0, 0.0, z), Vec3::Z, 0.24, 0.24),
        ],
        &OCT,
    );
    if b.fine() {
        for x in [2.6f32, 4.0, 5.4] {
            metal(b);
            let r = w(x) + 0.07;
            sleeve(
                b,
                &[
                    ring(v3(x - 0.16, 0.0, z), Vec3::Z, r, r),
                    ring(v3(x + 0.16, 0.0, z), Vec3::Z, r, r),
                ],
                &OCT,
            );
        }
    }
    dark_plate(b);
    blade(
        b,
        v3(6.4, 0.0, z + w(6.4) - 0.02),
        v3(-1.0, 0.0, -0.035),
        Vec3::Z,
        5.0,
        0.36,
        0.0,
        0.16,
    );
    b.mirror_y(|b| {
        // A slot down the bore's side, on its flat.
        let (x0, x1) = (2.9, 5.1);
        let at = v3((x0 + x1) * 0.5, w((x0 + x1) * 0.5) + 0.005, z);
        let along = v3(x1 - x0, w(x1) - w(x0), 0.0);
        slot(b, at, Vec3::Y, along, x1 - x0, 0.14);
        tine(b, z);
    });
}

/// The left tine: a flat blade standing on edge beside the charge, tied to the mouth,
/// its inner face slotted red.
fn tine(b: &mut MeshBuilder, z: f32) {
    let inner = 0.72;
    dark_plate(b);
    armour(
        b,
        &Frame::new(v3(6.2, inner, z), Vec3::X, Vec3::Y),
        &swept(2.8, 0.44, 0.0, 0.6),
        0.2,
    );
    dark_plate(b);
    b.beam(
        v3(6.0, 0.2, z),
        v3(6.6, inner + 0.1, z),
        Vec2::new(0.5, 0.3),
        Vec2::new(0.5, 0.3),
    );
    if b.fine() {
        slot(b, v3(7.4, inner - 0.01, z), -Vec3::Y, Vec3::X, 1.3, 0.16);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build_model;
    use crate::material::GLOW_VIOLET;

    const KEY: &str = "regency_lance_aa";

    #[test]
    fn holds_its_charge() {
        super::super::check_charge(KEY, RADIUS, HEIGHT, None, &[MUZZLE.to_array()], HOLD);
    }

    #[test]
    fn the_unit_files_lance_is_the_models() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let blueprints = mc_data::Blueprints::load(&dir).unwrap();
        let bp = blueprints.unit(blueprints.id_of("regency_t3_lance_aa").unwrap());
        let v = |p: mc_core::FxVec3| Vec3::new(p.x.to_f32(), p.y.to_f32(), p.z.to_f32());
        assert_eq!(bp.visual.mesh, KEY);
        assert!((bp.radius.to_f32() - RADIUS).abs() < 1e-3);
        assert!((bp.height.to_f32() - HEIGHT).abs() < 1e-3);
        let w = &bp.weapons[0];
        assert!(v(w.muzzle).distance(MUZZLE) < 1e-3);
        assert!(v(w.pivot.unwrap()).distance(PIVOT) < 1e-3);
        assert!(bp.turret_at.is_none(), "the turret turns about the middle");
        assert_eq!(
            bp.motion.map(|m| m.layer),
            Some(mc_data::MoveLayer::Hover),
            "it rides its lift"
        );
        let model = build_model(KEY).unwrap();
        assert!(model.hover && model.legs.is_none() && model.treads.is_none());
        assert_eq!(Vec3::from(model.arm_pivot.unwrap()), PIVOT);
        assert!(Vec3::from(model.turret_pivot).truncate().length() < 1e-4);
        assert!(model.lifts.len() >= 4, "one under each corner");
        for lod in &model.lods {
            assert!(
                lod.vertices.iter().all(|v| v.material != GLOW_VIOLET),
                "it does not build"
            );
        }
    }
}
