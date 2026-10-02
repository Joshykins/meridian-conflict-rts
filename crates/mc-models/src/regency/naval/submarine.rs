//! The Stiletto, the Regency's tech 1 attack submarine (`regency_t1_submarine` in
//! `data/factions/regency/units/naval.ron`): a chined pressure hull drawn to a point,
//! dark plates lapped back over it into swept spikes, a sail cut as a swept blade, and
//! two bow tubes under the waterline: dark bronze-rimmed mouths glowing red with the
//! plasma their gravity holds. No screw: a bronze gravity-drive ring round a red core.
//!
//! The origin is the waterline; the hull is drawn below it, the sail's top is the unit's
//! height. Everything is `HULL`: the tubes are fixed in the bow (docs/STYLE.md "The navy").
//!
//! The dagger: a slim needle prow between two tube cheeks, the sail a dark blade with
//! its red optics forward under the crown.

use glam::Vec3;

use crate::builder::MeshBuilder;
use crate::library::ModelDef;
use crate::material::*;

use super::super::kit::{dark_plate, metal, seam, v3};
use super::super::machine::{armour, hoop_on, red_slot, shaft, swept, Course, Frame};

const RADIUS: f32 = 10.5;
const HEIGHT: f32 = 3.4;

pub(crate) const MODELS: &[ModelDef] =
    &[ModelDef::new("regency_submarine", RADIUS, HEIGHT, dagger)];

/// The port bow tube's mouth, the unit file's muzzle; the starboard one is its mirror.
const TUBE: Vec3 = Vec3::new(10.0, 0.7, -0.9);
/// The hull's axis, where the drive and the tail fins centre.
const AXIS_Z: f32 = -1.0;

// ---- shared pieces --------------------------------------------------------------------

/// A hull station: where along it, its half width at the chine, and the heights of its
/// deck ridge, its chine and its keel.
#[derive(Clone, Copy, Debug)]
struct Station {
    x: f32,
    w: f32,
    top: f32,
    chine: f32,
    keel: f32,
}

const fn st(x: f32, w: f32, top: f32, chine: f32, keel: f32) -> Station {
    Station {
        x,
        w,
        top,
        chine,
        keel,
    }
}

/// A point of a hull: a station with no width, everything at `z`.
const fn tip(x: f32, z: f32) -> Station {
    st(x, 0.0, z, z, z)
}

/// A chined cross-section: a low ridge along the deck, sloped upper flanks out to a hard
/// chine, a V under it to the keel. Fewer facets further off.
fn section(b: &MeshBuilder, s: Station) -> Vec<Vec3> {
    let (up, down) = (s.top - s.chine, s.chine - s.keel);
    let side: Vec<[f32; 2]> = if b.fine() {
        vec![
            [0.55, s.top - 0.18 * up],
            [0.95, s.chine + 0.4 * up],
            [1.0, s.chine],
            [0.6, s.chine - 0.62 * down],
        ]
    } else if b.mid() {
        vec![[0.7, s.top - 0.25 * up], [1.0, s.chine]]
    } else {
        vec![[1.0, s.chine]]
    };
    let mut ring = vec![v3(s.x, 0.0, s.top)];
    ring.extend(side.iter().map(|&[f, z]| v3(s.x, f * s.w, z)));
    ring.push(v3(s.x, 0.0, s.keel));
    ring.extend(side.iter().rev().map(|&[f, z]| v3(s.x, -f * s.w, z)));
    ring
}

/// A hull lofted through `stations`, stern to bow.
fn hull(b: &mut MeshBuilder, stations: &[Station]) {
    let rings: Vec<Vec<Vec3>> = stations.iter().map(|&s| section(b, s)).collect();
    b.loft(&rings, true, true);
}

/// A flat blade of `outline` (points in one plane), `half` thick in the middle along
/// `normal`, its faces drawn in toward the middle by `k` so its edges come to a blade.
/// Far off, a plain slab.
fn fin(b: &mut MeshBuilder, outline: &[Vec3], normal: Vec3, half: f32, k: f32) {
    let c = outline.iter().copied().sum::<Vec3>() / outline.len() as f32;
    let n = normal.normalize() * half;
    let face = |s: f32, k: f32| -> Vec<Vec3> {
        outline.iter().map(|&p| c + (p - c) * k + n * s).collect()
    };
    if b.coarse() {
        b.loft(&[face(-1.0, 1.0), face(1.0, 1.0)], true, true);
    } else {
        b.loft(&[face(-1.0, k), outline.to_vec(), face(1.0, k)], true, true);
    }
}

/// A plan outline `(x, y)` laid on the plane through `z0` at the origin rising `slope`
/// per metre forward.
fn plan(points: &[[f32; 2]], z0: f32, slope: f32) -> Vec<Vec3> {
    points
        .iter()
        .map(|&[x, y]| v3(x, y, z0 + x * slope))
        .collect()
}

/// A side outline `(x, z)` stood up at `y`, leaning out by `cant` (radians) about x.
fn side(points: &[[f32; 2]], y: f32, cant: f32) -> Vec<Vec3> {
    points
        .iter()
        .map(|&[x, z]| v3(x, y + z * cant.sin(), z * cant.cos()))
        .collect()
}

/// A bow tube's mouth at `m`, facing `out`: a dark bore, the red of the held plasma in
/// it, and a dark bronze rim proud of both. Far off, a red square.
fn tube_mouth(b: &mut MeshBuilder, m: Vec3, out: Vec3, r: f32) {
    let out = out.normalize();
    if b.coarse() {
        let (side, up) = (out.cross(Vec3::Z).normalize() * r, Vec3::Z * r);
        let c = m + out * 0.03;
        b.paint(GLOW_LASER);
        b.face(&[c + side - up, c - side - up, c - side + up, c + side + up]);
        return;
    }
    let sides = b.sides(10);
    seam(b);
    b.cylinder_between(m - out * 0.25, m + out * 0.02, r, r, sides);
    b.paint(GLOW_LASER);
    b.cylinder_between(m - out * 0.02, m + out * 0.05, r * 0.66, r * 0.5, sides);
    if b.fine() {
        metal(b);
        hoop_on(b, m + out * 0.04, out, r + 0.04, 0.16, 0.18, 10);
    }
}

/// The gravity drive at the stern: a dark bronze ring `r` round the axis at `x`, a red
/// core burning in it, held off the tail on bronze spokes.
fn drive(b: &mut MeshBuilder, at: Vec3, r: f32, len: f32) {
    let x = Vec3::X;
    b.paint(GLOW_LASER);
    b.cylinder_between(
        at + x * (len * 0.6),
        at - x * (len * 0.4),
        r * 0.42,
        r * 0.18,
        b.sides(8),
    );
    if b.coarse() {
        return;
    }
    metal(b);
    hoop_on(b, at, x, r, 0.24, len, if b.fine() { 14 } else { 6 });
    if b.fine() {
        for (y, z) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
            let d = v3(0.0, y, z);
            shaft(
                b,
                at + x * (len * 0.5) + d * (r * 0.38),
                at + d * (r - 0.1),
                0.07,
            );
        }
    }
}

/// The team's mark: a swept plate pointing forward from `rear`, along `u`, lying on a
/// face whose normal is `n`. Far off, a flat arrowhead.
fn team_mark(b: &mut MeshBuilder, rear: Vec3, u: Vec3, n: Vec3, len: f32, half: f32) {
    b.paint(TEAM);
    let f = Frame::new(rear, u, n);
    if b.coarse() {
        b.face(&[
            f.at(len, 0.0, 0.05),
            f.at(0.0, half, 0.05),
            f.at(0.0, -half, 0.05),
        ]);
        return;
    }
    armour(b, &f, &swept(len, half, 0.0, 0.35), 0.07);
}

/// Red optics: a lit slot either side of a blade at `at` (port), along `along`.
fn optics(b: &mut MeshBuilder, at: Vec3, along: Vec3, len: f32) {
    if !b.fine() {
        return;
    }
    b.mirror_y(|b| red_slot(b, at, Vec3::Y, along, len, 0.12));
}

/// One swept plate of `len` from `o` back along `u`, face out along `n`, its spike at
/// `tip` across it, running `tail` past its course.
fn spike_plate(b: &mut MeshBuilder, o: Vec3, u: Vec3, n: Vec3, len: f32, half: f32, tip: f32) {
    dark_plate(b);
    armour(b, &Frame::new(o, u, n), &swept(len, half, tip, 0.35), 0.12);
}

// ---- the dagger --------------------------------------------------------------------

/// The dagger's hull, stern to its needle prow.
const HULL: [Station; 8] = [
    st(-8.9, 0.36, -0.55, -1.0, -1.45),
    st(-7.6, 0.85, 0.05, -1.0, -2.0),
    st(-5.0, 1.35, 0.38, -0.95, -2.5),
    st(1.0, 1.55, 0.48, -0.9, -2.7),
    st(6.0, 1.45, 0.45, -0.9, -2.6),
    st(8.4, 1.0, 0.36, -0.85, -2.25),
    st(10.4, 0.42, 0.2, -0.8, -1.6),
    tip(12.3, -0.45),
];
const HULL_FAR: [Station; 4] = [HULL[0], HULL[2], HULL[4], HULL[7]];

/// The dagger's sail, a swept blade (x, z): its trailing edge undercut into a spike.
const SAIL: [[f32; 2]; 6] = [
    [5.6, -0.1],
    [3.0, 3.4],
    [1.2, 3.4],
    [-2.6, 1.4],
    [-0.6, 0.55],
    [-0.9, -0.1],
];

/// The dagger: a slim chined hull drawn to a needle prow between two tube cheeks, a
/// blade of a sail, lapped plates down its back and flanks, swept planes and blades aft.
fn dagger(b: &mut MeshBuilder, _tech: u8) {
    dark_plate(b);
    if b.coarse() {
        hull(b, &HULL_FAR);
        fin(b, &side(&SAIL, 0.0, 0.0), Vec3::Y, 0.32, 1.0);
        b.mirror_y(|b| {
            tube_mouth(b, TUBE, Vec3::X, 0.3);
            dark_plate(b);
            b.face(&[
                v3(-5.2, 1.2, AXIS_Z),
                v3(-9.6, 3.0, AXIS_Z),
                v3(-8.4, 0.7, AXIS_Z),
            ]);
        });
        team_mark(b, v3(6.4, 0.0, 0.47), Vec3::X, Vec3::Z, 1.8, 0.55);
        return;
    }
    hull(b, &HULL);

    // The tube cheeks: chined pods along the lower flanks, proud of the prow as it
    // narrows, their mouths facing forward; a swept plate over each.
    b.mirror_y(|b| {
        dark_plate(b);
        let c = |x: f32, r: f32| -> Vec<Vec3> {
            let n = if b.fine() { 6 } else { 4 };
            (0..n)
                .map(|i| {
                    let a = std::f32::consts::TAU * (i as f32 + 0.5) / n as f32;
                    v3(x, TUBE.y + a.cos() * r, TUBE.z - 0.02 + a.sin() * r * 0.85)
                })
                .collect()
        };
        b.loft(&[c(5.4, 0.12), c(7.6, 0.44), c(TUBE.x, 0.47)], true, true);
        tube_mouth(b, TUBE, Vec3::X, 0.3);
        spike_plate(
            b,
            v3(10.15, 0.86, -0.4),
            v3(-1.0, 0.0, -0.05),
            v3(0.0, 0.5, 1.0),
            3.6,
            0.4,
            0.6,
        );
    });

    // The sail, a blade, its optics forward under the crown.
    dark_plate(b);
    fin(b, &side(&SAIL, 0.0, 0.0), Vec3::Y, 0.34, 0.84);
    optics(b, v3(2.95, 0.29, 2.75), v3(1.0, 0.0, -1.3), 0.7);
    // Sail planes: swept plates through the blade, their tips raked back into spikes.
    dark_plate(b);
    fin(
        b,
        &plan(&[[2.6, 0.0], [1.6, 1.7], [0.4, 1.85], [0.6, 0.0]], 2.3, 0.0),
        Vec3::Z,
        0.09,
        0.85,
    );
    b.mirror_y(|b| {
        fin(
            b,
            &plan(&[[2.6, 0.2], [1.6, 1.7], [0.2, 2.0], [0.9, 0.2]], 2.3, 0.0),
            Vec3::Z,
            0.09,
            0.85,
        );
    });

    // Down the back: a bronze spine under three lapped plates, the last one's tail a spike.
    metal(b);
    shaft(b, v3(-0.9, 0.0, 0.42), v3(-7.9, 0.0, -0.05), 0.2);
    dark_plate(b);
    Course {
        count: if b.fine() { 3 } else { 2 },
        step: if b.fine() { 1.75 } else { 2.6 },
        len: 2.1,
        half: 0.85,
        tip: 0.0,
        thick: 0.14,
        tail: 1.0,
    }
    .lay(
        b,
        &Frame::new(v3(-1.2, 0.0, 0.46), v3(-1.0, 0.0, -0.06), Vec3::Z),
    );
    team_mark(b, v3(6.4, 0.0, 0.47), Vec3::X, Vec3::Z, 1.8, 0.55);

    // Along each flank, a course of plates whose spikes flare down and out.
    b.mirror_y(|b| {
        dark_plate(b);
        Course {
            count: if b.fine() { 4 } else { 2 },
            step: if b.fine() { 2.5 } else { 5.0 },
            len: 2.9,
            half: 0.4,
            tip: -0.7,
            thick: 0.12,
            tail: 0.8,
        }
        .lay(
            b,
            &Frame::new(v3(5.6, 1.14, -0.04), Vec3::NEG_X, v3(0.0, 0.72, 0.69)),
        );
    });

    // Aft: swept diving planes, blades above and below, the drive.
    dark_plate(b);
    b.mirror_y(|b| {
        fin(
            b,
            &plan(
                &[[-5.2, 1.0], [-7.6, 2.8], [-9.6, 3.05], [-8.4, 0.6]],
                AXIS_Z,
                0.0,
            ),
            Vec3::Z,
            0.12,
            0.85,
        );
    });
    fin(
        b,
        &side(
            &[[-6.4, -0.2], [-8.4, 1.45], [-9.9, 1.6], [-8.9, -0.4]],
            0.0,
            0.0,
        ),
        Vec3::Y,
        0.12,
        0.85,
    );
    fin(
        b,
        &side(
            &[[-6.8, -1.6], [-8.9, -1.6], [-9.9, -3.2], [-8.5, -3.1]],
            0.0,
            0.0,
        ),
        Vec3::Y,
        0.12,
        0.85,
    );
    drive(b, v3(-9.35, 0.0, AXIS_Z), 0.78, 0.4);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{build_model_scaled, part};

    const KEYS: [&str; 1] = ["regency_submarine"];

    fn muzzles() -> [Vec3; 2] {
        [TUBE * v3(1.0, -1.0, 1.0), TUBE]
    }

    /// The library's naval rules, here so the hull is tested on its own: it fits its
    /// radius and height, keeps its keel above 4.5 m down, wears team colour and dark
    /// plate at every level of detail, keeps to its budget, and a tube mouth is at each
    /// muzzle, fixed in the hull.
    #[test]
    fn fits_the_librarys_naval_checks() {
        for key in KEYS {
            let model = build_model_scaled(key, RADIUS, HEIGHT, 1).expect(key);
            let tris = |lod: usize| model.lods[lod].indices.len() / 3;
            let (full, mid, coarse) = (tris(0), tris(1), tris(2));
            let budget = super::super::super::triangles(key).unwrap();
            assert!(
                full <= budget && full >= 250,
                "{key}: {full} (budget {budget})"
            );
            assert!(
                mid as f32 <= full as f32 * 0.45 + 20.0 && coarse < 60,
                "{key}: {full}/{mid}/{coarse}"
            );
            for (lod, mesh) in model.lods.iter().enumerate() {
                let name = format!("{key} lod{lod}");
                let top = mesh
                    .vertices
                    .iter()
                    .map(|v| v.pos[2])
                    .fold(f32::MIN, f32::max);
                assert!(
                    (HEIGHT * 0.8..=HEIGHT * 1.25).contains(&top),
                    "{name}: top {top}"
                );
                let low = mesh
                    .vertices
                    .iter()
                    .map(|v| v.pos[2])
                    .fold(f32::MAX, f32::min);
                assert!((-4.5..-1.0).contains(&low), "{name}: keel at {low}");
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
                    mesh.vertices.iter().all(|v| v.part == part::HULL),
                    "{name}: the tubes are fixed in the hull"
                );
                assert!(
                    !mesh
                        .vertices
                        .iter()
                        .any(|v| v.material == GLOW || v.material == GLOW_ORANGE),
                    "{name}: ARC's blue or orange light"
                );
                for m in muzzles() {
                    let near = mesh
                        .vertices
                        .iter()
                        .filter(|v| v.material == GLOW_LASER)
                        .map(|v| Vec3::from(v.pos).distance(m))
                        .fold(f32::MAX, f32::min);
                    // The mouth's glow is a disc round the muzzle: its rim is within reach.
                    assert!(near < 0.5, "{name}: no red mouth within {near} m of {m}");
                }
            }
        }
    }

    #[test]
    fn the_unit_files_tubes_are_the_models() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let blueprints = mc_data::Blueprints::load(&dir).unwrap();
        let bp = blueprints.unit(blueprints.id_of("regency_t1_submarine").unwrap());
        let v = |p: mc_core::FxVec3| Vec3::new(p.x.to_f32(), p.y.to_f32(), p.z.to_f32());
        assert_eq!(bp.visual.mesh, "regency_submarine");
        assert!((bp.radius.to_f32() - RADIUS).abs() < 1e-3);
        assert!((bp.height.to_f32() - HEIGHT).abs() < 1e-3);
        let weapon = &bp.weapons[0];
        let drawn: Vec<Vec3> = weapon.muzzles.iter().map(|&p| v(p)).collect();
        assert_eq!(drawn.len(), 2);
        for (a, m) in drawn.iter().zip(muzzles()) {
            assert!(a.distance(m) < 1e-3, "unit file muzzle {a}, model {m}");
        }
        let middle = (muzzles()[0] + muzzles()[1]) * 0.5;
        assert!(v(weapon.muzzle).distance(middle) < 1e-3);
    }
}
