//! The Flamberge's three main houses: twin-rail Pinch-fusion Cannon, the Sunspear's gun
//! made twin and armoured for a ship.
//!
//! Each stands on a barbette skirted in faceted plates. The house is low and faceted: a
//! toothed race under it, a glacis forward with a mantlet the bores run out through, its
//! flanks lapped in two courses of plates swept back into saw-toothed spikes with a slot of
//! red let along them, a machined roof with two optics, the fusion core in a hood over its
//! back between two radiator packs, a tail of plates swept back off it. Out of the mantlet
//! run two slim bronze bores wound with pinch coils, held between three thin plated rails
//! whose heads carry gravity lenses aimed into the charges; ties, sleeves and conduits run
//! between them.
//!
//! It works with its own gun's charge (`rig::CHARGE_GEAR_MASK`, the house's
//! `HousePose::charge`): through the charge the bores' jackets run out (`EXTEND`), the
//! outer rails part (`SPREAD`), the lens heads slide out along them into the charges
//! (`REACH`, the middle one straight out), the coils light from the breech one after
//! another and the rails' inner faces light with them. The shot kicks the gun back hard
//! (`KICK`); the radiator lids stand up off glowing grilles (`VENT`, `HEAT_STAGE`) and
//! settle as it cools.

use glam::{Vec2, Vec3};

use crate::builder::{MeshBuilder, Section};
use crate::gpu_consts::charge_gear::{EXTEND, HEAT_STAGE, REACH, SPREAD, VENT};
use crate::pattern;

use super::super::super::kit::{cable, dark_plate, metal, seam, v3};
use super::super::super::machine::{armour, collar, hoop, hoop_on, swept, teeth, Course, Frame};
use super::super::super::parts::{face_out, inlay, ridged};
use super::super::super::turrets::sunspear::{coil_light, lens};
use super::guns::facets;
use super::hull::{body_x, mirrored, pointed, team_patch, tier, CHAMFERED};
use super::{charge, MAIN, TWIN};

/// How far the gun kicks back when it fires.
const KICK: f32 = 1.4;
/// How far out from the house's middle its outer rails stand, at rest, and their width.
const RAIL_Y: f32 = 2.0;
const RAIL_W: f32 = 0.36;
/// The bores' radius.
const BORE: f32 = 0.3;
/// The house's plan about its pivot, half of it (y >= 0), nose round to tail.
const PLAN: [[f32; 2]; 6] = [
    [3.8, 1.8],
    [2.2, 3.5],
    [-2.6, 3.7],
    [-5.2, 3.2],
    [-6.4, 1.8],
    [-6.7, 0.0],
];

/// Main gun `i` (house `i`) on its barbette up from the deck at `deck`.
pub(super) fn main_gun(b: &mut MeshBuilder, i: usize, deck: f32) {
    let p = MAIN[i];
    let base = p.z - 1.3;
    if !b.coarse() {
        barbette(b, p, base, deck);
    }
    b.with_house(i, p, KICK, |b| {
        if b.coarse() {
            coarse_house(b, i);
            return;
        }
        house(b, i);
        b.with_recoil(|b| gun(b, i));
    });
}

/// The barbette under house `p`, from the deck at `deck` up to the house's `base`: a tall
/// one (the raised house's) on a plated tower pointed forward and swept back aft, with
/// optic slots in its prow; a drum skirted in faceted plates leaning out off it, red slots
/// between them; a seam and a bronze race on top.
fn barbette(b: &mut MeshBuilder, p: Vec3, base: f32, deck: f32) {
    let sides = facets(b, 16);
    let drum = if base - deck > 2.5 {
        let top = base - 0.9;
        tier(
            b,
            p.x,
            &pointed(6.0, 6.0, 4.4),
            deck - 0.4,
            top,
            Vec2::new(0.9, 0.88),
        );
        if b.mid() {
            b.mirror_y(|b| {
                dark_plate(b);
                Course {
                    count: 2,
                    step: 2.6,
                    len: 3.6,
                    half: 0.7,
                    tip: -0.4,
                    thick: 0.22,
                    tail: 1.6,
                }
                .lay(
                    b,
                    &Frame::new(
                        v3(p.x + 1.0, 4.2, top - 1.2),
                        v3(-1.0, -0.1, 0.0),
                        v3(0.0, 1.0, 0.3),
                    ),
                );
            });
        }
        top - 0.1
    } else {
        deck - 0.4
    };
    dark_plate(b);
    b.prism(v3(p.x, 0.0, drum), sides, 3.7, 3.45, base - drum);
    if b.mid() {
        let n = if b.fine() { 8 } else { 6 };
        let h = (base - drum - 0.3).max(0.8);
        for k in 0..n {
            let a = std::f32::consts::TAU * (k as f32 + 0.5) / n as f32;
            let out = v3(a.cos(), a.sin(), 0.0);
            let foot = v3(p.x, 0.0, drum) + out * 3.8;
            let f = Frame::new(foot, Vec3::Z * 0.92 - out * 0.2, out);
            let w = 3.8 * std::f32::consts::PI / n as f32 * 0.82;
            dark_plate(b);
            ridged(
                b,
                &f,
                &[
                    [0.0, -w],
                    [0.0, w],
                    [h, w * 0.85],
                    [h + 0.35, 0.0],
                    [h, -w * 0.85],
                ],
                0.22,
                [h * 0.45, 0.0],
                0.12,
            );
            if b.fine() && base - drum > 1.4 {
                // A slot of red let into the drum between this plate and the next.
                let a2 = a + std::f32::consts::PI / n as f32;
                let o2 = v3(a2.cos(), a2.sin(), 0.0);
                let at = v3(p.x, 0.0, drum + h * 0.5) + o2 * 3.62;
                let up = Vec3::Z * (h * 0.3);
                let side = v3(-o2.y, o2.x, 0.0) * 0.08;
                inlay(
                    b,
                    &[
                        at - up - side,
                        at - up + side,
                        at + up + side,
                        at + up - side,
                    ],
                    o2,
                    pattern::EMBER_SLOT,
                );
            }
        }
    }
    seam(b);
    b.prism(v3(p.x, 0.0, base - 0.3), sides, 3.5, 3.5, 0.26);
    if b.fine() {
        metal(b);
        hoop(b, v3(p.x, 0.0, base - 0.04), 2.95, 0.5, 0.14, sides);
    }
}

/// Far off, a house is a plated wedge from its back to its charges.
fn coarse_house(b: &mut MeshBuilder, i: usize) {
    let (p, c) = (MAIN[i], charge(i));
    let base = p.z - 1.3;
    dark_plate(b);
    b.loft(
        &[
            vec![
                v3(p.x - 6.0, -3.0, base),
                v3(p.x - 6.0, 3.0, base),
                v3(p.x - 6.0, 0.0, p.z + 1.4),
            ],
            vec![
                v3(c.x, -2.2, c.z - 0.5),
                v3(c.x, 2.2, c.z - 0.5),
                v3(c.x, 0.0, c.z + 0.5),
            ],
        ],
        true,
        true,
    );
}

/// The house about main pivot `i`: race and body, its flanks, roof, hood and radiators.
fn house(b: &mut MeshBuilder, i: usize) {
    let p = MAIN[i];
    let base = p.z - 1.3;
    let roof = p.z + 0.95;
    let sides = facets(b, 14);
    // The race it turns on, toothed.
    dark_plate(b);
    b.prism(v3(p.x - 1.0, 0.0, base), sides, 3.6, 3.4, 0.5);
    if b.fine() {
        metal(b);
        teeth(
            b,
            v3(p.x - 1.0, 0.0, base + 0.08),
            3.45,
            18,
            v3(0.16, 0.26, 0.3),
        );
    }
    let plan = mirrored(&PLAN);
    b.at(v3(p.x, 0.0, 0.0), |b| {
        b.with_facets(|b| {
            seam(b);
            b.loft_z(
                &plan,
                &[
                    Section::new(base + 0.5, 0.96),
                    Section::new(base + 0.75, 0.96),
                ],
            );
            dark_plate(b);
            b.loft_z(
                &plan,
                &[
                    Section::new(base + 0.75, 1.0),
                    Section::new(p.z - 0.3, 1.04),
                    Section::scaled(roof, 0.84, 0.8).shifted(-0.5, 0.0),
                ],
            );
        });
    });
    if b.mid() {
        inlay(
            b,
            &[
                v3(p.x - 4.4, -1.6, roof),
                v3(p.x - 1.4, -1.6, roof),
                v3(p.x - 1.4, 1.6, roof),
                v3(p.x - 4.4, 1.6, roof),
            ],
            Vec3::Z,
            pattern::EMBER_MACHINED,
        );
    }
    team_patch(b, p.x - 1.0, p.x + 1.0, 0.8, roof + 0.04);
    hood(b, p, roof);
    flanks(b, p, base, roof);
    tail(b, p, roof);
}

/// A house's flanks (+y side, mirrored): two courses of plates lapped back down it, the
/// lower swept into saw-toothed spikes past the back, a slot of red let along the seam
/// between them; a faceted shoulder plate over the glacis; an optic on the roof's edge;
/// a radiator pack beside the hood.
fn flanks(b: &mut MeshBuilder, p: Vec3, base: f32, roof: f32) {
    let fine = b.fine();
    b.mirror_y(|b| {
        dark_plate(b);
        Course {
            count: if fine { 3 } else { 2 },
            step: 2.9,
            len: 3.6,
            half: 0.42,
            tip: -0.5,
            thick: 0.2,
            tail: 1.8,
        }
        .lay(
            b,
            &Frame::new(
                v3(p.x + 2.4, 3.7, base + 0.95),
                v3(-1.0, 0.07, 0.0),
                v3(0.0, 1.0, 0.12),
            ),
        );
        Course {
            count: if fine { 3 } else { 1 },
            step: 2.6,
            len: 3.2,
            half: 0.36,
            tip: 0.4,
            thick: 0.18,
            tail: 1.0,
        }
        .lay(
            b,
            &Frame::new(
                v3(p.x + 1.6, 3.52, p.z + 0.2),
                v3(-1.0, 0.04, 0.0),
                v3(0.0, 1.0, 0.45),
            ),
        );
        if fine {
            inlay(
                b,
                &[
                    v3(p.x + 1.8, 3.82, base + 1.52),
                    v3(p.x - 4.8, 3.82, base + 1.52),
                    v3(p.x - 4.8, 3.82, base + 1.62),
                    v3(p.x + 1.8, 3.82, base + 1.62),
                ],
                Vec3::Y,
                pattern::EMBER_SLOT,
            );
        }
        // The glacis's shoulder, faceted.
        ridged(
            b,
            &Frame::new(
                v3(p.x + 3.7, 1.75, base + 0.8),
                v3(-0.62, 0.78, 0.0),
                v3(0.62, 0.45, 0.64),
            ),
            &[[0.0, -0.8], [1.9, -0.8], [2.1, 0.7], [-0.1, 0.7]],
            0.2,
            [1.0, 0.0],
            0.12,
        );
        if fine {
            // An optic on the roof's forward edge: a chamfered box, red let into its face.
            let at = v3(p.x + 1.6, 2.2, roof + 0.3);
            dark_plate(b);
            b.chamfered_box(at, v3(0.9, 0.6, 0.55), 0.14);
            inlay(
                b,
                &[
                    at + v3(0.46, -0.2, -0.12),
                    at + v3(0.46, 0.2, -0.12),
                    at + v3(0.46, 0.2, 0.12),
                    at + v3(0.46, -0.2, 0.12),
                ],
                Vec3::X,
                pattern::EMBER_SLOT,
            );
        }
        radiator(b, p, roof);
    });
}

/// The core's hood over the house's back: a plated hood gathered to a ridge, the core's
/// star in a well let into it, two bronze rings tilted across it.
fn hood(b: &mut MeshBuilder, p: Vec3, roof: f32) {
    let core = v3(p.x - 4.9, 0.0, roof + 0.45);
    dark_plate(b);
    ridged(
        b,
        &Frame::new(v3(p.x - 3.1, 0.0, roof - 0.05), -Vec3::X, Vec3::Z),
        &[
            [0.0, -1.3],
            [0.0, 1.3],
            [2.8, 1.5],
            [3.9, 0.9],
            [3.9, -0.9],
            [2.8, -1.5],
        ],
        0.32,
        [1.8, 0.0],
        0.2,
    );
    hoop(b, core, 1.05, 0.35, 0.45, facets(b, 14));
    coil_light(b, 0);
    b.spheroid(
        core,
        v3(0.55, 0.55, 0.34),
        facets(b, 10),
        if b.fine() { 5 } else { 3 },
    );
    metal(b);
    for axis in [v3(1.0, 0.0, 2.0), v3(-1.0, 0.0, 2.0)] {
        hoop_on(b, core, axis, 0.88, 0.16, 0.2, facets(b, 14));
    }
}

/// A radiator pack beside the hood (+y side): a plated box, a fusion-lit floor dark until
/// the shot (`HEAT_STAGE`), a grille over it, and two lids that stand up off it with the
/// heat after the shot (`VENT`) and settle as it cools.
fn radiator(b: &mut MeshBuilder, p: Vec3, roof: f32) {
    let (x0, x1, y0, y1) = (p.x - 6.2, p.x - 3.3, 1.6, 2.85);
    let (z0, z1) = (roof - 0.5, roof + 0.28);
    dark_plate(b);
    b.block(v3(x0, y0, z0), v3(x1, y1, z1 - 0.2));
    for (a, c) in [
        (v3(x0, y0, z1 - 0.2), v3(x1, y0 + 0.16, z1)),
        (v3(x0, y1 - 0.16, z1 - 0.2), v3(x1, y1, z1)),
        (v3(x0, y0, z1 - 0.2), v3(x0 + 0.16, y1, z1)),
        (v3(x1 - 0.16, y0, z1 - 0.2), v3(x1, y1, z1)),
    ] {
        b.block(a, c);
    }
    coil_light(b, HEAT_STAGE);
    b.face(&[
        v3(x0 + 0.16, y0 + 0.16, z1 - 0.16),
        v3(x1 - 0.16, y0 + 0.16, z1 - 0.16),
        v3(x1 - 0.16, y1 - 0.16, z1 - 0.16),
        v3(x0 + 0.16, y1 - 0.16, z1 - 0.16),
    ]);
    if b.fine() {
        metal(b);
        for k in 0..5 {
            let x = x0 + 0.4 + (x1 - x0 - 0.8) * k as f32 / 4.0;
            b.block(
                v3(x - 0.05, y0 + 0.16, z1 - 0.14),
                v3(x + 0.05, y1 - 0.16, z1 - 0.05),
            );
        }
    }
    b.with_charge_gear(VENT, |b| {
        let w = (x1 - x0) / 2.0;
        for k in 0..2 {
            let a = x0 + w * k as f32;
            dark_plate(b);
            armour(
                b,
                &Frame::new(v3(a + 0.06, (y0 + y1) * 0.5, z1), Vec3::X, Vec3::Z),
                &[
                    [0.0, -(y1 - y0) * 0.5],
                    [0.0, (y1 - y0) * 0.5],
                    [w - 0.12, (y1 - y0) * 0.5],
                    [w - 0.12, -(y1 - y0) * 0.5],
                ],
                0.16,
            );
        }
    });
}

/// The tail of the house: plates swept back off its back either side into spikes, and a
/// bronze conduit run from the hood down into the barbette.
fn tail(b: &mut MeshBuilder, p: Vec3, roof: f32) {
    if !b.mid() {
        return;
    }
    let fine = b.fine();
    b.mirror_y(|b| {
        dark_plate(b);
        armour(
            b,
            &Frame::new(
                v3(p.x - 5.2, 1.2, roof - 0.35),
                v3(-1.0, 0.25, -0.2),
                v3(0.0, 0.25, 1.0),
            ),
            &swept(3.2, 0.55, 0.4, 0.45),
            0.2,
        );
        if fine {
            metal(b);
            cable(
                b,
                &[
                    v3(p.x - 5.6, 0.75, roof + 0.1),
                    v3(p.x - 6.5, 0.9, roof - 0.6),
                    v3(p.x - 6.2, 1.0, p.z - 1.1),
                ],
                0.13,
            );
        }
    });
}

/// The gun in the house's frame (it pitches and kicks): the mantlet and breech, two slim
/// bores wound with coils, their jackets running out through the charge, three thin rails
/// out past the mouths with lens heads at their tips, ties and conduits.
fn gun(b: &mut MeshBuilder, i: usize) {
    let fine = b.fine();
    let (p, c) = (MAIN[i], charge(i));
    let (x, z) = (p.x, c.z);
    collar(b, p, Vec3::Y, 0.8, 5.0);
    // The mantlet, faceted, over the breech the bores leave through.
    dark_plate(b);
    ridged(
        b,
        &Frame::new(v3(x + 3.3, 0.0, z - 0.9), Vec3::Z, Vec3::X),
        &[[0.0, -2.1], [0.0, 2.1], [1.8, 1.8], [2.0, 0.0], [1.8, -1.8]],
        0.32,
        [1.0, 0.0],
        0.18,
    );
    body_x(
        b,
        0.0,
        &[[x - 0.5, 3.6, 1.4, z], [x + 3.4, 4.0, 1.6, z]],
        &CHAMFERED,
    );
    let sides = facets(b, 8);
    let mouth = c.x - 2.3;
    let coils: &[(f32, u32)] = if fine {
        &[(4.8, 1), (6.2, 2), (7.6, 4)]
    } else {
        &[(5.0, 1), (7.0, 3)]
    };
    for y in [-TWIN, TWIN] {
        metal(b);
        b.cylinder_between(v3(x + 3.4, y, z), v3(x + 8.3, y, z), BORE, BORE, sides);
        for &(dx, stage) in coils {
            let at = v3(x + dx, y, z);
            dark_plate(b);
            hoop_on(b, at, Vec3::X, BORE + 0.2, 0.2, 0.32, sides);
            coil_light(b, stage);
            hoop_on(
                b,
                at + Vec3::X * 0.22,
                Vec3::X,
                BORE + 0.12,
                0.08,
                0.1,
                sides,
            );
        }
        // The jacket: runs out along the bore through the charge.
        b.with_charge_gear(EXTEND, |b| {
            dark_plate(b);
            b.cylinder_between(
                v3(x + 7.9, y, z),
                v3(mouth - 0.5, y, z),
                BORE + 0.08,
                BORE + 0.04,
                sides,
            );
            hoop_on(
                b,
                v3(x + 9.0, y, z),
                Vec3::X,
                BORE + 0.14,
                0.12,
                0.25,
                sides,
            );
            b.cylinder_between(
                v3(mouth - 0.6, y, z),
                v3(mouth, y, z),
                BORE + 0.16,
                BORE + 0.12,
                sides,
            );
            coil_light(b, 5);
            hoop_on(
                b,
                v3(mouth + 0.02, y, z),
                Vec3::X,
                BORE + 0.02,
                0.1,
                0.04,
                sides,
            );
        });
    }
    b.mirror_y(|b| {
        b.with_charge_gear(SPREAD, |b| outer_rail(b, x, c, z, fine));
        b.with_charge_gear(REACH, |b| lens_head(b, c, RAIL_Y, 0.48, 1.2));
    });
    // The middle rail, fixed, thin; its head runs straight out into the gap.
    dark_plate(b);
    body_x(
        b,
        0.0,
        &[
            [x + 3.4, 0.36, 1.5, z + 0.05],
            [x + 6.0, 0.32, 1.3, z],
            [c.x - 2.6, 0.3, 1.2, z],
        ],
        &CHAMFERED,
    );
    b.with_charge_gear(EXTEND, |b| {
        lens_head(b, c, 0.0, 0.34, 1.1);
        if fine {
            for y in [-0.24f32, 0.24] {
                lens(b, v3(c.x - 0.6, y, z), v3(c.x, y.signum() * TWIN, z), 0.2);
            }
        }
    });
    if fine {
        // Ties across the rails on their fixed middles, and conduits from the breech.
        metal(b);
        for tx in [x + 4.2, c.x - 4.0] {
            for tz in [-0.62f32, 0.62] {
                b.beam(
                    v3(tx, -(RAIL_Y - 0.5), z + tz),
                    v3(tx, RAIL_Y - 0.5, z + tz),
                    Vec2::new(0.18, 0.18),
                    Vec2::new(0.18, 0.18),
                );
            }
        }
        b.mirror_y(|b| {
            cable(
                b,
                &[
                    v3(x + 1.0, 1.8, z + 0.8),
                    v3(x + 3.0, 2.2, z + 0.9),
                    v3(x + 4.6, RAIL_Y - 0.25, z + 0.55),
                ],
                0.1,
            );
        });
    }
}

/// An outer rail (+y side), parting through the charge: a thin plated bar out past the
/// bores, its top edge saw-toothed with small plates swept back, its inner face lit with
/// the charge, and the ties' outer sleeves riding with it.
fn outer_rail(b: &mut MeshBuilder, x: f32, c: Vec3, z: f32, fine: bool) {
    dark_plate(b);
    body_x(
        b,
        RAIL_Y,
        &[
            [x + 3.0, RAIL_W, 1.2, z],
            [x + 4.2, RAIL_W, 1.7, z],
            [c.x - 1.8, RAIL_W * 0.9, 1.45, z],
            [c.x + 0.2, RAIL_W * 0.8, 1.1, z],
        ],
        &CHAMFERED,
    );
    if b.mid() {
        // The top edge, saw-toothed: plates swept back along it.
        Course {
            count: if fine { 4 } else { 2 },
            step: 1.7,
            len: 2.0,
            half: 0.2,
            tip: 0.0,
            thick: 0.14,
            tail: 0.7,
        }
        .lay(
            b,
            &Frame::new(
                v3(c.x - 1.2, RAIL_Y, z + 0.66),
                v3(-1.0, 0.0, -0.08),
                Vec3::Z,
            ),
        );
    }
    if fine {
        // The inner face, lit from the breech to the tip as the charge climbs.
        let y = RAIL_Y - RAIL_W * 0.5 - 0.02;
        for (k, (x0, x1)) in [(x + 4.6, x + 7.0), (x + 7.2, c.x - 2.0)]
            .into_iter()
            .enumerate()
        {
            coil_light(b, 2 + 2 * k as u32);
            face_out(
                b,
                &[
                    v3(x0, y, z - 0.1),
                    v3(x1, y, z - 0.1),
                    v3(x1, y, z + 0.1),
                    v3(x0, y, z + 0.1),
                ],
                -Vec3::Y,
            );
        }
        metal(b);
        for tx in [x + 4.2, c.x - 4.0] {
            for tz in [-0.62f32, 0.62] {
                b.beam(
                    v3(tx, RAIL_Y - 0.6, z + tz),
                    v3(tx, RAIL_Y - 0.2, z + tz),
                    Vec2::new(0.26, 0.26),
                    Vec2::new(0.26, 0.26),
                );
            }
        }
    }
}

/// A rail's head at `y`: a plated sleeve over the rail's tip, its nose swept in; on the
/// outer rails two gravity lenses on its inner face aimed into the charge.
fn lens_head(b: &mut MeshBuilder, c: Vec3, y: f32, w: f32, h: f32) {
    dark_plate(b);
    body_x(
        b,
        y,
        &[
            [c.x - 2.4, w, h, c.z],
            [c.x + 0.1, w, h, c.z],
            [c.x + 0.6, w * 0.55, h * 0.6, c.z],
        ],
        &CHAMFERED,
    );
    if y > 0.0 {
        for dz in [-0.3f32, 0.3] {
            lens(
                b,
                v3(c.x - 0.45, y - w * 0.5 - 0.04, c.z + dz),
                v3(c.x, TWIN, c.z),
                0.2,
            );
        }
    }
}
