//! The Dirk, the Regency's tech 1 attack boat: a low semi-submersible that lies dived in
//! ambush and comes up to fight. Dark plates lapped over a faceted hull, swept back into
//! points where they trail; bronze workings in the gaps between them; a pair of red
//! optics at the bow; plasma jets at the stern; a small gun house over the foredeck with
//! a twin plasmeric repeater, its bores red.
//!
//! Finish (docs/STYLE.md "The Regency look", "The navy"): dark plate (`dark_plate`),
//! dark seams (`seam`), dark bronze machinery (`metal`), red only in optics, bores and
//! jets. The hull's origin is its waterline; the keel is drawn below it, and the deck
//! sits low so the sea closes over it when it dives.
//!
//! Rig: the repeater turns on a gun house of its own (`MeshBuilder::with_house`, weapon 0)
//! about `PIVOT`, the barrels recoiling inside it. The numbers match
//! `regency_t1_attack_boat` in `data/factions/regency/units/naval.ron`.

use glam::{Vec2, Vec3};

use crate::builder::{MeshBuilder, Section};
use crate::library::ModelDef;
use crate::material::*;

use super::super::kit::{cable, dark_plate, metal, seam, v3};
use super::super::machine::collar;
use super::super::plating::plate;

const RADIUS: f32 = 6.5;
const HEIGHT: f32 = 3.2;

pub(crate) const MODELS: &[ModelDef] =
    &[ModelDef::new("regency_attack_boat", RADIUS, HEIGHT, dart)];

/// Where the gun house turns: the unit file's weapon `pivot`.
const PIVOT: Vec3 = Vec3::new(2.0, 0.0, 2.2);
/// The repeater's two muzzles: the unit file's weapon `muzzles`.
const MUZZLES: [Vec3; 2] = [Vec3::new(4.4, -0.3, 2.45), Vec3::new(4.4, 0.3, 2.45)];
/// Where the gun house's turning ring sits.
const RING_Z: f32 = 1.8;

/// Sides for a round part: `n` close up, half that (five at least) further off, so the
/// reduced level keeps to its share of the full one.
fn round(b: &MeshBuilder, n: usize) -> usize {
    if b.fine() {
        n
    } else {
        (n / 2).max(5)
    }
}

// ---- the hull ------------------------------------------------------------------

/// One cross-section of the faceted hull at `x`, mirrored about y = 0: the keel's z, then
/// (z, half beam) at the chine and at the deck edge, and the z of the crown along the
/// middle. The deck edge is the widest point.
#[derive(Clone, Copy)]
struct Station {
    x: f32,
    keel: f32,
    chine: [f32; 2],
    deck: [f32; 2],
    crown: f32,
}

const fn st(x: f32, keel: f32, chine: [f32; 2], deck: [f32; 2], crown: f32) -> Station {
    Station {
        x,
        keel,
        chine,
        deck,
        crown,
    }
}

/// The hull lofted stern first through `stations` to the `stem`'s point, in dark plate.
/// The coarse level keeps the `coarse` stations as a diamond: keel, deck edges, crown.
fn hull(b: &mut MeshBuilder, stations: &[Station], stem: Vec3, coarse: &[usize]) {
    let ring = |s: &Station, simple: bool| -> Vec<Vec3> {
        let p = |y: f32, z: f32| v3(s.x, y, z);
        if simple {
            return vec![
                p(0.0, s.keel),
                p(-s.deck[1], s.deck[0]),
                p(0.0, s.crown),
                p(s.deck[1], s.deck[0]),
            ];
        }
        vec![
            p(0.0, s.keel),
            p(-s.chine[1], s.chine[0]),
            p(-s.deck[1], s.deck[0]),
            p(0.0, s.crown),
            p(s.deck[1], s.deck[0]),
            p(s.chine[1], s.chine[0]),
        ]
    };
    let mut rings: Vec<Vec<Vec3>> = if b.coarse() {
        coarse.iter().map(|&i| ring(&stations[i], true)).collect()
    } else {
        stations.iter().map(|s| ring(s, false)).collect()
    };
    let n = rings[0].len();
    rings.push(vec![stem; n]);
    dark_plate(b);
    b.loft(&rings, true, false);
}

/// The two stations either side of `x` and how far between them.
fn between(stations: &[Station], x: f32) -> (Station, Station, f32) {
    let i = stations
        .windows(2)
        .position(|w| x <= w[1].x)
        .unwrap_or(stations.len() - 2);
    let (a, c) = (stations[i], stations[i + 1]);
    (a, c, ((x - a.x) / (c.x - a.x)).clamp(0.0, 1.0))
}

/// The deck's half beam at `x`, out to the deck edge.
fn beam_at(stations: &[Station], x: f32) -> f32 {
    let (a, c, t) = between(stations, x);
    a.deck[1] + (c.deck[1] - a.deck[1]) * t
}

/// The top of the hull at (`x`, `y`): down from the crown to the deck edge, flat beyond.
fn surface(stations: &[Station], x: f32, y: f32) -> f32 {
    let (a, c, t) = between(stations, x);
    let l = |p: f32, q: f32| p + (q - p) * t;
    let (crown, deck, dz) = (
        l(a.crown, c.crown),
        l(a.deck[1], c.deck[1]),
        l(a.deck[0], c.deck[0]),
    );
    let y = y.abs();
    if y <= deck {
        crown + (dz - crown) * (y / deck.max(1e-3))
    } else {
        dz
    }
}

/// An armour plate laid on the hull through the plan `outline` (x, y), `lift` over the
/// hull's top at each point, `thick` deep. Its edges are bevelled in at full detail; its
/// underside lies against the hull and is drawn only there.
fn lap(b: &mut MeshBuilder, stations: &[Station], outline: &[[f32; 3]], thick: f32) {
    let base: Vec<Vec3> = outline
        .iter()
        .map(|&[x, y, lift]| v3(x, y, surface(stations, x, y) + lift))
        .collect();
    let up = |k: f32, inset: f32| -> Vec<Vec3> {
        let mid = base.iter().copied().sum::<Vec3>() / base.len() as f32;
        base.iter()
            .map(|&p| {
                let towards = (mid - p).with_z(0.0);
                p + towards.normalize_or_zero() * towards.length().min(inset)
                    + Vec3::Z * (thick * k)
            })
            .collect()
    };
    dark_plate(b);
    if b.fine() {
        b.loft(&[base.clone(), up(0.6, 0.0), up(1.0, 0.05)], true, true);
    } else {
        let top = up(1.0, 0.0);
        b.loft(&[base, top], false, true);
    }
}

/// A plate's outline mirrored to the right side: (x, -y, lift), in the reverse order.
fn right(outline: &[[f32; 3]]) -> Vec<[f32; 3]> {
    outline.iter().rev().map(|&[x, y, l]| [x, -y, l]).collect()
}

/// A plate laid either side of the middle, `outline` the left one.
fn lap_pair(b: &mut MeshBuilder, stations: &[Station], outline: &[[f32; 3]], thick: f32) {
    lap(b, stations, outline, thick);
    lap(b, stations, &right(outline), thick);
}

/// The owner's colour: a chevron let flat into the deck at `x`, pointing forward.
fn chevron(b: &mut MeshBuilder, stations: &[Station], x: f32, size: f32) {
    let z = surface(stations, x, 0.0) + 0.04;
    let outline = [
        v3(x + size * 0.5, 0.0, z),
        v3(x - size * 0.4, size * 0.75, z - 0.08),
        v3(x - size * 0.65, size * 0.55, z - 0.06),
        v3(x - 0.05, 0.12, z),
        v3(x - size * 0.65, -size * 0.55, z - 0.06),
        v3(x - size * 0.4, -size * 0.75, z - 0.08),
    ];
    b.paint(TEAM);
    if b.coarse() {
        b.face(&[outline[0], outline[1], outline[5]]);
        return;
    }
    plate(b, &outline, Vec3::Z * 0.05);
}

/// A pair of red optics on the bow's flanks under a brow, at `x`, `half` out, `z` up.
fn optics(b: &mut MeshBuilder, x: f32, half: f32, z: f32) {
    if b.coarse() {
        return;
    }
    b.paint(GLOW_LASER);
    b.mirror_y(|b| {
        b.beam(
            v3(x, half, z),
            v3(x - 0.45, half + 0.12, z + 0.02),
            Vec2::new(0.1, 0.1),
            Vec2::new(0.06, 0.07),
        );
        if b.fine() {
            b.beam(
                v3(x - 0.65, half + 0.16, z + 0.03),
                v3(x - 0.85, half + 0.2, z + 0.03),
                Vec2::new(0.06, 0.07),
                Vec2::new(0.05, 0.05),
            );
        }
    });
}

/// A plasma jet at the stern: a bronze nozzle out of the transom at `at`, red heat in
/// its mouth.
fn jet(b: &mut MeshBuilder, at: Vec3, radius: f32) {
    if b.coarse() {
        return;
    }
    let mouth = at - Vec3::X * 0.55;
    seam(b);
    let sides = round(b, 10);
    if b.fine() {
        b.cylinder_between(at + Vec3::X * 0.3, at, radius * 1.15, radius * 1.15, sides);
    }
    metal(b);
    b.cylinder_between(at, mouth, radius, radius * 0.86, sides);
    if b.fine() {
        b.paint(GLOW_LASER);
        b.cylinder_between(
            mouth + Vec3::X * 0.06,
            mouth - Vec3::X * 0.02,
            radius * 0.66,
            radius * 0.6,
            8,
        );
    }
}

/// The bronze workings along the middle of the deck, showing between the plates: a spine
/// from `x1` back to `x0`, a collar at each `gaps`, and a cable either side up close.
fn spine(b: &mut MeshBuilder, stations: &[Station], x0: f32, x1: f32, gaps: &[f32]) {
    if b.coarse() {
        return;
    }
    let at = |x: f32| v3(x, 0.0, surface(stations, x, 0.0) + 0.02);
    metal(b);
    b.beam(at(x1), at(x0), Vec2::new(0.22, 0.14), Vec2::new(0.18, 0.12));
    if !b.fine() {
        return;
    }
    for &x in gaps {
        collar(b, at(x) + Vec3::Z * 0.04, Vec3::Y, 0.11, 0.5);
    }
    b.mirror_y(|b| {
        let points: Vec<Vec3> = [x1, (x0 + x1) * 0.5, x0]
            .iter()
            .map(|&x| at(x) + v3(0.0, 0.2, -0.02))
            .collect();
        metal(b);
        cable(b, &points, 0.04);
    });
}

// ---- the gun house ---------------------------------------------------------------

/// The fixed seat the gun house turns on, from the deck at `deck` up to the ring.
fn barbette(b: &mut MeshBuilder, deck: f32, radius: f32) {
    if b.coarse() {
        return;
    }
    seam(b);
    let sides = round(b, 10);
    b.prism(
        v3(PIVOT.x, 0.0, deck - 0.1),
        sides,
        radius * 1.08,
        radius,
        RING_Z - deck + 0.1,
    );
}

/// The gun house: a bronze ring, a low faceted house swept back into two points, red
/// sights on its cheeks, and the twin repeater through a seam-dark mantlet, bronze
/// barrels with red bores.
fn gun_house(b: &mut MeshBuilder) {
    // How far back the house's trailing points reach.
    let swept = 1.25;
    b.with_house(0, PIVOT, 0.25, |b| {
        let (x, top) = (PIVOT.x, PIVOT.z + 0.6);
        if b.coarse() {
            dark_plate(b);
            b.loft_z(
                &[
                    [x + 0.9, 0.0],
                    [x - 0.3, 0.7],
                    [x - swept, 0.0],
                    [x - 0.3, -0.7],
                ],
                &[
                    Section::new(RING_Z, 1.0),
                    Section::scaled(top - 0.1, 0.8, 0.7),
                ],
            );
            b.paint(METAL);
            b.face(&[
                v3(x + 0.8, -0.3, MUZZLES[0].z),
                MUZZLES[0],
                MUZZLES[1],
                v3(x + 0.8, 0.3, MUZZLES[0].z),
            ]);
            return;
        }
        metal(b);
        let sides = round(b, 12);
        b.prism(v3(x, 0.0, RING_Z), sides, 0.78, 0.74, 0.14);
        dark_plate(b);
        b.at(v3(x, 0.0, 0.0), |b| {
            let plan = [
                [0.95, 0.0],
                [0.62, 0.56],
                [-0.35, 0.82],
                [-swept, 0.62],
                [-0.8, 0.22],
                [-0.95, 0.0],
                [-0.8, -0.22],
                [-swept, -0.62],
                [-0.35, -0.82],
                [0.62, -0.56],
            ];
            b.loft_z(
                &plan,
                &[
                    Section::new(RING_Z + 0.14, 0.94),
                    Section::new(MUZZLES[0].z - 0.15, 1.0),
                    Section::scaled(top, 0.72, 0.66).shifted(-0.18, 0.0),
                ],
            );
        });
        // The sights: a red slit either cheek.
        b.paint(GLOW_LASER);
        b.mirror_y(|b| {
            b.beam(
                v3(x + 0.5, 0.62, MUZZLES[0].z - 0.05),
                v3(x + 0.05, 0.76, MUZZLES[0].z - 0.03),
                Vec2::new(0.06, 0.08),
                Vec2::new(0.06, 0.06),
            );
        });
        b.with_recoil(|b| {
            seam(b);
            b.chamfered_box(v3(x + 0.95, 0.0, MUZZLES[0].z), v3(0.4, 1.0, 0.42), 0.12);
            for m in MUZZLES {
                let breech = v3(x + 1.0, m.y, m.z);
                let bore = m - Vec3::X * 0.16;
                metal(b);
                let sides = round(b, 8);
                b.cylinder_between(breech, bore, 0.1, 0.085, sides);
                if b.fine() {
                    seam(b);
                    let s = breech + Vec3::X * 0.55;
                    b.cylinder_between(s, s + Vec3::X * 0.3, 0.13, 0.13, 8);
                }
                b.paint(GLOW_LASER);
                b.cylinder_between(bore, m, 0.1, 0.09, sides);
            }
        });
    });
}

// ---- the boat ---------------------------------------------------------------------

/// A single long faceted hull, a knife bow and a wide flat stern, low enough that the
/// sea washes over its deck.
const DART: [Station; 6] = [
    st(-6.0, -0.45, [-0.15, 1.35], [0.85, 1.55], 1.1),
    st(-3.6, -0.85, [-0.3, 1.6], [0.95, 1.8], 1.4),
    st(-0.6, -1.0, [-0.3, 1.65], [1.0, 1.85], 1.62),
    st(2.4, -0.95, [-0.25, 1.4], [1.0, 1.55], 1.6),
    st(4.5, -0.6, [0.0, 0.85], [0.95, 0.98], 1.3),
    st(5.7, -0.25, [0.25, 0.38], [0.85, 0.45], 1.05),
];
const DART_STEM: Vec3 = Vec3::new(6.45, 0.0, 0.75);

fn dart(b: &mut MeshBuilder, _tech: u8) {
    b.set_dust_line(0.4);
    let h = &DART;
    hull(b, h, DART_STEM, &[0, 2, 4]);
    let deck = surface(h, PIVOT.x, 0.0);
    barbette(b, deck, 0.8);
    gun_house(b);
    chevron(b, h, -2.6, 0.95);
    if b.coarse() {
        return;
    }
    // The bow shield: a plate forward of the gun, its trailing points flanking the house.
    lap_pair(
        b,
        h,
        &[
            [5.9, 0.04, 0.06],
            [5.2, 0.5, 0.06],
            [3.4, 1.25, 0.08],
            [2.3, 1.62, 0.16],
            [3.0, 0.95, 0.14],
            [3.1, 0.04, 0.12],
        ],
        0.14,
    );
    // Three plates lapped back from behind the house, each tail lifted over the head
    // of the next, their trailing edges swept back into points at the sides.
    for &(x0, x1, w, sweep) in &[
        (1.1f32, -1.5f32, 1.0f32, 0.9f32),
        (-1.1, -3.7, 1.02, 1.0),
        (-3.3, -5.7, 1.04, 1.2),
    ] {
        let (b0, b1) = (beam_at(h, x0) * w, beam_at(h, x1) * w);
        lap_pair(
            b,
            h,
            &[
                [x0, 0.06, 0.05],
                [x0 - 0.08, b0 * 0.55, 0.06],
                [x0 - 0.45, b0, 0.08],
                [x1 - sweep, b1 + 0.08, 0.26],
                [x1 + 0.4, b1 * 0.7, 0.22],
                [x1 + 0.15, 0.06, 0.2],
            ],
            0.13,
        );
    }
    spine(b, h, -5.6, 1.3, &[-1.3, -3.5]);
    optics(b, 5.0, 0.62, 0.92);
    b.mirror_y(|b| jet(b, v3(-5.95, 0.7, 0.22), 0.34));
    if b.fine() {
        // Intakes under the bow shield's trailing points, dark with a red line.
        b.mirror_y(|b| {
            seam(b);
            b.beam(
                v3(2.2, 1.45, 0.95),
                v3(1.4, 1.6, 0.92),
                Vec2::new(0.28, 0.2),
                Vec2::new(0.3, 0.2),
            );
            b.paint(GLOW_LASER);
            b.beam(
                v3(2.1, 1.5, 1.04),
                v3(1.5, 1.63, 1.02),
                Vec2::new(0.04, 0.03),
                Vec2::new(0.04, 0.03),
            );
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{build_model, part, rig};

    /// The Regency's checks (`regency::check`) as they stand for a hull: the keel goes
    /// below the waterline, and the gun turns on a house of its own, not a turret.
    #[test]
    fn fits_the_librarys_checks() {
        let key = "regency_attack_boat";
        let model = build_model(key).unwrap();
        let tris = |lod: usize| model.lods[lod].indices.len() / 3;
        let (full, mid, coarse) = (tris(0), tris(1), tris(2));
        let budget = super::super::super::triangles(key).unwrap();
        assert!(full <= budget && full >= 250, "{key}: {full} triangles");
        assert!(
            mid as f32 <= full as f32 * 0.45 + 20.0 && coarse < 60,
            "{key}: {full}/{mid}/{coarse}"
        );
        assert_eq!(model.houses.len(), 1, "{key}: one gun house");
        assert_eq!(model.houses[0].weapon, 0);
        assert_eq!(Vec3::from(model.houses[0].pivot), PIVOT);
        for (lod, mesh) in model.lods.iter().enumerate() {
            let name = format!("{key} lod{lod}");
            let top = mesh.vertices.iter().map(|v| v.pos[2]).fold(0.0, f32::max);
            assert!(
                (HEIGHT * 0.8..=HEIGHT * 1.25).contains(&top),
                "{name}: top {top}"
            );
            let keel = mesh.vertices.iter().map(|v| v.pos[2]).fold(0.0, f32::min);
            assert!(keel >= -1.5, "{name}: keel {keel}");
            let reach = mesh
                .vertices
                .iter()
                .map(|v| v.pos[0].hypot(v.pos[1]))
                .fold(0.0, f32::max);
            assert!(
                (RADIUS * 0.75..=RADIUS * 1.3).contains(&reach),
                "{name}: reach {reach}"
            );
            assert!(
                mesh.vertices.iter().all(|v| v.part != part::TURRET),
                "{name}: a turret"
            );
            assert!(
                mesh.vertices
                    .iter()
                    .any(|v| v.material == TEAM && v.normal[2] > 0.5),
                "{name}: no upward team colour"
            );
            assert!(
                mesh.vertices.iter().any(|v| v.material == PLATING_DARK),
                "{name}: no dark plate"
            );
            assert!(
                !mesh
                    .vertices
                    .iter()
                    .any(|v| v.material == GLOW || v.material == GLOW_ORANGE),
                "{name}: ARC's blue or orange light"
            );
            let house = || {
                mesh.vertices
                    .iter()
                    .filter(|v| v.rig & rig::LIMB_MASK == rig::HOUSE_FIRST)
                    .map(|v| Vec3::from(v.pos))
            };
            for m in MUZZLES {
                let near = house().map(|p| p.distance(m)).fold(f32::MAX, f32::min);
                assert!(near < 0.4, "{name}: gun house {near} m from muzzle {m}");
            }
            let past = house().map(|p| p.x).fold(f32::MIN, f32::max);
            assert!(past <= MUZZLES[0].x + 0.5, "{name}: house reaches {past}");
        }
    }

    #[test]
    fn the_unit_files_gun_is_the_models() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let blueprints = mc_data::Blueprints::load(&dir).unwrap();
        let bp = blueprints.unit(blueprints.id_of("regency_t1_attack_boat").unwrap());
        let v = |p: mc_core::FxVec3| Vec3::new(p.x.to_f32(), p.y.to_f32(), p.z.to_f32());
        assert_eq!(bp.visual.mesh, "regency_attack_boat");
        assert!(
            (bp.radius.to_f32() - RADIUS).abs() < 1e-3
                && (bp.height.to_f32() - HEIGHT).abs() < 1e-3
        );
        let w = &bp.weapons[0];
        assert!(w.mount, "the repeater turns on a gun house");
        let pivot = v(w.pivot.expect("a pivot"));
        assert!(pivot.distance(PIVOT) < 1e-3, "pivot {pivot}");
        assert!(v(w.muzzle).distance((MUZZLES[0] + MUZZLES[1]) * 0.5) < 1e-3);
        let muzzles: Vec<Vec3> = w.muzzles.iter().map(|&m| v(m)).collect();
        assert_eq!(muzzles.len(), 2);
        for (got, want) in muzzles.iter().zip(MUZZLES) {
            assert!(got.distance(want) < 1e-3, "muzzle {got} for {want}");
        }
    }
}
