//! The Harpoon, the Regency torpedo launcher (`regency_t1_torpedo_defense` in
//! `data/factions/regency/units/naval.ron`), on a 2 x 2 lot at sea; refitted to tech 2
//! where it floats, its last tier.
//!
//! A low hexagonal caisson of sloped plate round an open well, three sensor horns standing
//! off its deck. In the well a launcher house turns at the waterline on a bearing under the
//! water (weapon 0, `MeshBuilder::with_house`): a plated drum whose cowl stands
//! out of the water, so it shows from above where it aims, and two gravitic tubes under
//! the surface, their mouths lit red. The tech 2 refit hangs a second house lower on the
//! same axis (weapon 1) and fits a fixed interceptor tube in the float at each of three
//! points (weapon 2), all under the water, and stands a taller horn by each of the three.
//!
//! The origin is the waterline; what is under it is `part::AFLOAT`.

use glam::{Vec2, Vec3};

use crate::builder::{ngon, MeshBuilder, Section};
use crate::material::*;
use crate::part;

use super::kit::{dark_plate, metal, seam, segment, v3};
use super::machine::tier;
use super::machine::*;

/// Each tier's height (the unit file's).
const TOPS: [f32; 2] = [6.0, 8.0];
/// The launcher houses' pivots on the axis (weapons 0 and 1), and their muzzles:
/// `MUZZLE_X` forward of the axis, `MUZZLE_Y` either side (the unit file's).
const HOUSES: [f32; 2] = [-1.0, -4.0];
const MUZZLE_X: f32 = 4.0;
const MUZZLE_Y: f32 = 0.5;
/// How far the tubes kick back when they fire.
const TRAVEL: f32 = 0.25;
/// The interceptor tubes' mouths (weapon 2): this far out, at this depth, at 0, 120 and
/// 240 degrees (the unit file's).
const INTERCEPT_R: f32 = 7.4;
const INTERCEPT_Z: f32 = -1.2;
/// When in the refit the tech 2 pieces come on.
const REFIT_AT: f32 = 0.15;
const THICK: f32 = 0.26;

fn around(deg: f32, r: f32, z: f32) -> Vec3 {
    let (s, c) = deg.to_radians().sin_cos();
    v3(r * c, r * s, z)
}

/// A gravitic tube's mouth at `m`, facing `out`: a seam-dark rim, the red glow of the
/// plasma held in it.
fn tube_mouth(b: &mut MeshBuilder, m: Vec3, out: Vec3, r: f32) {
    let sides = b.sides(8);
    seam(b);
    b.cylinder_between(m - out * 0.2, m, r * 1.12, r * 1.12, sides);
    b.paint(GLOW_LASER);
    b.cylinder_between(m - out * 0.02, m + out * 0.04, r * 0.7, r * 0.5, sides);
}

/// One launcher house, in its own frame facing +x about its pivot depth `z`: a plated
/// drum turning on the axis, cheeks either side of the tubes, a magazine behind, the two
/// tubes out to their mouths (they kick back). `crown`: the top house's drum stands up
/// out of the water to a cowl of plates lapped back, a red sight in its face and the
/// owner's colour on top.
fn launcher(b: &mut MeshBuilder, z: f32, crown: bool) {
    let fine = b.fine();
    let sides = if fine { 10 } else { 6 };
    let (z0, z1) = if crown {
        (z - 0.8, z + 2.0)
    } else {
        (z - 0.75, z + 0.75)
    };
    dark_plate(b);
    b.prism(v3(0.0, 0.0, z0), sides, 1.35, 1.25, z1 - z0);
    if fine {
        seam(b);
        b.prism(v3(0.0, 0.0, z0 - 0.12), sides, 1.2, 1.2, 0.12);
    }
    if crown {
        let f = Frame::new(v3(1.3, 0.0, z1), v3(-1.0, 0.0, -0.12), Vec3::Z);
        dark_plate(b);
        Course {
            count: if fine { 2 } else { 1 },
            step: 1.3,
            len: 2.0,
            half: 1.05,
            tip: 0.0,
            thick: THICK,
            tail: 0.9,
        }
        .lay(b, &f);
        b.paint(TEAM);
        b.face(&[
            v3(0.9, -0.5, z1 + THICK + 0.06),
            v3(0.9, 0.5, z1 + THICK + 0.06),
            v3(0.1, 0.5, z1 + THICK + 0.1),
            v3(0.1, -0.5, z1 + THICK + 0.1),
        ]);
        red_slot(b, v3(1.3, 0.0, z1 - 0.5), Vec3::X, Vec3::Y, 1.0, 0.18);
    }
    b.mirror_y(|b| {
        dark_plate(b);
        b.block(v3(0.6, 0.85, z - 0.5), v3(2.6, 1.1, z + 0.5));
    });
    dark_plate(b);
    if fine {
        b.chamfered_box(v3(-1.9, 0.0, z), v3(1.3, 1.7, 1.1), 0.2);
    } else {
        b.block(v3(-2.5, -0.8, z - 0.5), v3(-1.2, 0.8, z + 0.5));
    }
    b.with_recoil(|b| {
        let sides = b.sides(8);
        for y in [MUZZLE_Y, -MUZZLE_Y] {
            dark_plate(b);
            b.cylinder_between(v3(0.9, y, z), v3(MUZZLE_X - 0.2, y, z), 0.3, 0.27, sides);
            if fine {
                metal(b);
                b.cylinder_between(v3(1.9, y, z), v3(2.3, y, z), 0.36, 0.36, sides);
            }
            tube_mouth(b, v3(MUZZLE_X, y, z), Vec3::X, 0.27);
        }
    });
}

/// Far off: a house as one flat plate from the axis out to its muzzles.
fn coarse_house(b: &mut MeshBuilder, weapon: usize) {
    let z = HOUSES[weapon];
    b.with_house(weapon, v3(0.0, 0.0, z), TRAVEL, |b| {
        dark_plate(b);
        b.face(&[
            v3(-1.2, -0.8, z + 0.3),
            v3(MUZZLE_X, -0.8, z + 0.3),
            v3(MUZZLE_X, 0.8, z + 0.3),
            v3(-1.2, 0.8, z + 0.3),
        ]);
    });
}

/// The two launcher houses: weapon 0 at the waterline, weapon 1 under it at tech 2,
/// on a graphite spindle down the axis.
fn houses(b: &mut MeshBuilder, tech: u8) {
    // The houses stand in the water: drawn only there, where the structure always is.
    b.with_part(part::AFLOAT, |b| {
        b.with_house(0, v3(0.0, 0.0, HOUSES[0]), TRAVEL, |b| {
            launcher(b, HOUSES[0], true)
        })
    });
    tier(b, tech, 2, REFIT_AT, |b| {
        b.with_part(part::AFLOAT, |b| {
            shaft(
                b,
                v3(0.0, 0.0, HOUSES[0] - 0.8),
                v3(0.0, 0.0, HOUSES[1] - 0.9),
                0.3,
            );
            b.with_house(1, v3(0.0, 0.0, HOUSES[1]), TRAVEL, |b| {
                launcher(b, HOUSES[1], false)
            });
        });
    });
}

/// Tech 2's interceptor tube at each of the three points (all under the water): a short
/// plated tube out through the float, its mouth lit red.
fn interceptors(b: &mut MeshBuilder) {
    b.with_part(part::AFLOAT, |b| {
        for k in 0..3 {
            b.yawed(Vec3::ZERO, (120.0 * k as f32).to_radians(), |b| {
                let m = v3(INTERCEPT_R, 0.0, INTERCEPT_Z);
                dark_plate(b);
                b.cylinder_between(m - Vec3::X * 1.5, m - Vec3::X * 0.2, 0.34, 0.3, b.sides(8));
                tube_mouth(b, m, Vec3::X, 0.28);
            });
        }
    });
}

fn team_top(b: &mut MeshBuilder, z: f32, r: f32) {
    b.paint(TEAM);
    b.face(&[v3(r, 0.0, z), v3(0.0, r, z), v3(-r, 0.0, z), v3(0.0, -r, z)]);
}

/// Far off: the caisson as one hexagonal slab, a plated
/// spire to the top with the owner's colour on it, the houses as plates.
fn coarse_float(b: &mut MeshBuilder, tech: u8) {
    // One slab, under the water and over it: the structure stands only at sea.
    let plan = ngon(6, 7.8);
    b.with_part(part::AFLOAT, |b| {
        dark_plate(b);
        b.loft_z(
            &plan,
            &[
                Section::new(-1.6, 0.9),
                Section::new(0.0, 1.0),
                Section::new(1.0, 0.85),
            ],
        );
    });
    dark_plate(b);
    let top = TOPS[tech as usize - 1];
    b.frustum(
        Vec3::Z * 1.0,
        Vec2::splat(2.4),
        Vec2::splat(0.8),
        top - 1.0,
        Vec2::ZERO,
    );
    team_top(b, top + 0.02, 0.4);
    b.with_part(part::AFLOAT, |b| {
        coarse_house(b, 0);
        if tech >= 2 {
            coarse_house(b, 1);
        }
    });
}

/// The caisson's walls: the inner face's distance from the axis, the outer face's at the
/// keel and at the deck, and the keel's and the deck's heights.
const WELL: f32 = 4.9;
const OUTER: (f32, f32) = (6.8, 5.9);
const KEEL: f32 = -1.6;
const CAISSON_DECK: f32 = 1.5;
/// The bearing the launcher turns on, under the water.
const BEARING: f32 = -2.6;

pub(super) fn torpedo(b: &mut MeshBuilder, tech: u8) {
    let tech = tech.clamp(1, 2);
    if b.coarse() {
        coarse_float(b, tech);
        return;
    }
    let fine = b.fine();
    for k in 0..6 {
        b.yawed(Vec3::ZERO, (60.0 * k as f32).to_radians(), |b| {
            wall(b, k % 2 == 0)
        });
    }
    // The bearing: three plated spokes from the walls under the water to a hub on the
    // axis, under the launcher.
    b.with_part(part::AFLOAT, |b| {
        for k in 0..3 {
            let d = around(60.0 + 120.0 * k as f32, 1.0, 0.0);
            strut(
                b,
                d * (WELL + 0.4) + Vec3::Z * (KEEL + 0.1),
                d * 1.1 + Vec3::Z * BEARING,
                0.24,
            );
        }
        collar(b, Vec3::Z * BEARING, Vec3::Z, 1.2, 0.5);
    });
    for k in 0..3 {
        b.yawed(Vec3::ZERO, (60.0 + 120.0 * k as f32).to_radians(), horn);
    }
    houses(b, tech);
    tier(b, tech, 2, REFIT_AT, |b| {
        interceptors(b);
        for k in 0..3 {
            b.yawed(
                Vec3::ZERO,
                (60.0 + 120.0 * k as f32).to_radians(),
                tall_horn,
            );
        }
    });
    if fine {
        // A walk round the well's lip.
        metal(b);
        hoop(
            b,
            Vec3::Z * (CAISSON_DECK + 0.05),
            WELL + 0.1,
            0.25,
            0.12,
            6,
        );
    }
}

/// One wall of the hexagon, the one facing +x: a sloped plated section mitred into its
/// neighbours, the part under the water drawn only there. `team`: it carries the owner's
/// colour on its deck.
fn wall(b: &mut MeshBuilder, team: bool) {
    let t = 30f32.to_radians().tan();
    let quad = |z: f32, a_in: f32, a_out: f32, side: f32| -> Vec<Vec3> {
        vec![v3(a_in, side * a_in * t, z), v3(a_out, side * a_out * t, z)]
    };
    let section = |side: f32, z0: f32, z1: f32| -> Vec<Vec3> {
        let out = |z: f32| OUTER.0 + (OUTER.1 - OUTER.0) * (z - KEEL) / (CAISSON_DECK - KEEL);
        let mut r = quad(z0, WELL, out(z0), side);
        let mut top = quad(z1, WELL, out(z1), side);
        top.reverse();
        r.extend(top);
        r
    };
    b.with_part(part::AFLOAT, |b| {
        dark_plate(b);
        b.loft(
            &[section(-1.0, KEEL, 0.0), section(1.0, KEEL, 0.0)],
            true,
            true,
        );
    });
    dark_plate(b);
    b.loft(
        &[
            section(-1.0, 0.0, CAISSON_DECK),
            section(1.0, 0.0, CAISSON_DECK),
        ],
        true,
        true,
    );
    if b.fine() {
        // Two plates lapped down the wall's face toward the sea, a seam band along it at
        // the waterline.
        let out = |z: f32| OUTER.0 + (OUTER.1 - OUTER.0) * (z - KEEL) / (CAISSON_DECK - KEEL);
        for y in [-1.5, 1.5] {
            let f = Frame::new(
                v3(out(CAISSON_DECK) - 0.1, y, CAISSON_DECK + 0.05),
                v3(out(0.2) - out(CAISSON_DECK), 0.0, 0.2 - CAISSON_DECK),
                v3(1.0, 0.0, 0.6),
            );
            dark_plate(b);
            armour(b, &f, &swept(1.25, 1.2, 0.0, 0.6), THICK);
        }
        let band = |side: f32| -> Vec<Vec3> {
            let mut r = quad(0.1, out(0.1) - 0.1, out(0.1) + 0.08, side);
            let mut top = quad(0.4, out(0.4) - 0.1, out(0.4) + 0.08, side);
            top.reverse();
            r.extend(top);
            r
        };
        seam(b);
        b.loft(&[band(-1.0), band(1.0)], true, true);
    }
    if team {
        b.paint(TEAM);
        let (x0, x1) = (WELL + 0.25, OUTER.1 - 0.3);
        b.face(&[
            v3(x0, -0.6, CAISSON_DECK + 0.02),
            v3(x1, -0.6, CAISSON_DECK + 0.02),
            v3(x1, 0.6, CAISSON_DECK + 0.02),
            v3(x0, 0.6, CAISSON_DECK + 0.02),
        ]);
    }
}

/// A sensor horn on the deck at a corner-free wall, along +x: a plated block on the deck,
/// a horn of plates swept up and in to a point at tech 1's height, a red slot in its face.
fn horn(b: &mut MeshBuilder) {
    let base = (WELL + OUTER.1) * 0.5;
    dark_plate(b);
    b.block(
        v3(base - 0.6, -0.7, CAISSON_DECK),
        v3(base + 0.5, 0.7, CAISSON_DECK + 0.8),
    );
    let foot = v3(base, 0.0, CAISSON_DECK + 0.6);
    let tip = v3(base - 1.4, 0.0, TOPS[0]);
    dark_plate(b);
    segment(b, &[(foot, 0.42, 0.42), (tip, 0.08, 0.08)], Vec3::Y);
    let up = (tip - foot).normalize();
    dark_plate(b);
    armour(
        b,
        &Frame::new(foot + v3(0.4, 0.0, 0.0), up, v3(1.0, 0.0, 0.3)),
        &swept(foot.distance(tip) * 0.75, 0.45, 0.0, 0.5),
        0.2,
    );
    red_slot(
        b,
        foot + up * 1.4 + v3(0.36, 0.0, 0.0),
        v3(1.0, 0.0, 0.25),
        up,
        0.8,
        0.14,
    );
}

/// Tech 2's taller horn at the same wall: a plate lapped up behind tech 1's to tech 2's
/// height, a second red slot in it.
fn tall_horn(b: &mut MeshBuilder) {
    let base = (WELL + OUTER.1) * 0.5;
    let foot = v3(base - 0.5, 0.0, CAISSON_DECK + 0.6);
    let tip = v3(base - 2.2, 0.0, TOPS[1]);
    dark_plate(b);
    segment(b, &[(foot, 0.36, 0.36), (tip, 0.07, 0.07)], Vec3::Y);
    let up = (tip - foot).normalize();
    red_slot(
        b,
        foot + up * 4.0 + v3(0.24, 0.0, 0.05),
        v3(1.0, 0.0, 0.35),
        up,
        0.7,
        0.12,
    );
}

#[cfg(test)]
mod tests {
    use glam::Vec3;

    use crate::{build_model_scaled, rig};

    use super::*;

    const DESIGNS: [&str; 1] = ["regency_torpedo"];

    #[test]
    fn every_design_fits_at_both_tiers() {
        for key in DESIGNS {
            for tech in 1..=2u8 {
                super::super::check_at(key, tech, 8.0, TOPS[tech as usize - 1], Some(2), &[]);
            }
        }
    }

    /// Each house's tubes end at the unit file's muzzles, and at tech 2 an interceptor tube
    /// mouth is at each of its interceptor muzzles.
    #[test]
    fn the_unit_files_muzzles_are_the_models() {
        let bp = mc_data::Blueprints::load(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"),
        )
        .unwrap();
        let unit = bp
            .units
            .iter()
            .find(|u| u.key == "regency_t2_torpedo_defense")
            .unwrap();
        for key in DESIGNS {
            let model = build_model_scaled(key, 8.0, TOPS[1], 2).unwrap();
            assert_eq!(model.houses.len(), 2, "{key}: two launcher houses");
            let house = |w: usize| {
                model
                    .houses
                    .iter()
                    .position(|h| h.weapon as usize == w)
                    .map(|s| s as u32)
            };
            for (w, weapon) in unit.weapons.iter().enumerate() {
                for m in &weapon.muzzles {
                    let m = Vec3::from(m.to_f32());
                    let near = model.lods[0]
                        .vertices
                        .iter()
                        .filter(|v| match house(w) {
                            Some(slot) => v.rig & rig::LIMB_MASK == rig::HOUSE_FIRST + slot,
                            None => v.rig & rig::LIMB_MASK == 0,
                        })
                        .map(|v| Vec3::from(v.pos).distance(m))
                        .fold(f32::MAX, f32::min);
                    assert!(
                        near < 0.45,
                        "{key}: weapon {w} muzzle {m} has nothing within {near} m"
                    );
                }
            }
        }
    }

    /// The launcher turns clear of the float: nothing that does not turn stands within
    /// the house's sweep at its depth.
    #[test]
    fn the_launcher_turns_clear_of_the_float() {
        for key in DESIGNS {
            let model = build_model_scaled(key, 8.0, TOPS[1], 2).unwrap();
            for v in model.lods[0]
                .vertices
                .iter()
                .filter(|v| v.rig & rig::LIMB_MASK == 0)
            {
                let p = Vec3::from(v.pos);
                let reach = p.truncate().length();
                for z in HOUSES {
                    if (p.z - z).abs() < 0.6 && reach > 0.5 {
                        assert!(reach > MUZZLE_X + 0.4, "{key}: {p} in the sweep at {z}");
                    }
                }
            }
        }
    }
}
