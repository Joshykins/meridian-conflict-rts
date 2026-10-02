//! The Regency's jet kit for their tech 2 aircraft (docs/STYLE.md "The Regency look",
//! "Aircraft"): Cybertronian-jet airframes, symmetrical, a blade for a nose, fins turned
//! down, swept plates lapped back over bronze workings, red optics, and drives that burn
//! red in a bronze ring (never ARC's orange).
//!
//! Every airframe is lofted through chined stations ([`Station`]) and dressed with blades
//! ([`blade`]): a wing is a plan outline laid at a slope ([`plan`]), a fin a side outline
//! stood up at a cant ([`side`]). A Gravitic Seeker or Torpedo is held, not carried: a
//! plasma charge between two bronze tines ([`cradle`]), the muzzle at the charge.

use glam::Vec3;

use crate::builder::MeshBuilder;
use crate::material::*;

use super::super::kit::{dark_plate, metal, seam, v3};
use super::super::machine::{armour, hoop_on, red_slot, shaft, swept, Course, Frame};

/// A hull station: where along it, its half width at the chine, and the heights of its
/// back, its chine and its keel.
#[derive(Clone, Copy, Debug)]
pub(super) struct Station {
    pub(super) x: f32,
    pub(super) w: f32,
    pub(super) top: f32,
    pub(super) chine: f32,
    pub(super) keel: f32,
}

pub(super) const fn st(x: f32, w: f32, top: f32, chine: f32, keel: f32) -> Station {
    Station {
        x,
        w,
        top,
        chine,
        keel,
    }
}

/// A point of a hull: a station with no width, everything at `z`.
pub(super) const fn tip(x: f32, z: f32) -> Station {
    st(x, 0.0, z, z, z)
}

/// A chined cross-section: a low ridge along the back, sloped flanks out to a hard chine,
/// a shallow V under it to the keel. Fewer facets further off.
fn section(b: &MeshBuilder, s: Station) -> Vec<Vec3> {
    let (up, down) = (s.top - s.chine, s.chine - s.keel);
    let side: Vec<[f32; 2]> = if b.fine() {
        vec![
            [0.5, s.top - 0.15 * up],
            [0.92, s.chine + 0.35 * up],
            [1.0, s.chine],
            [0.62, s.chine - 0.6 * down],
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

/// A body lofted through `stations`, tail to nose, in dark plate. Far off, its tail, its
/// widest station and its nose.
pub(super) fn body(b: &mut MeshBuilder, stations: &[Station]) {
    dark_plate(b);
    let picked: Vec<Station> = if b.coarse() && stations.len() > 3 {
        let last = stations.len() - 1;
        let wide = (1..last)
            .max_by(|&i, &j| stations[i].w.total_cmp(&stations[j].w))
            .unwrap_or(last / 2);
        vec![stations[0], stations[wide], stations[last]]
    } else {
        stations.to_vec()
    };
    let rings: Vec<Vec<Vec3>> = picked.iter().map(|&s| section(b, s)).collect();
    b.loft(&rings, true, true);
}

/// A flat blade of `outline` (points in one plane), `half` thick in the middle along
/// `normal`, its faces drawn in toward the middle by `k` so its edges come to a blade.
/// Far off, a sheet seen from both sides.
pub(super) fn blade(b: &mut MeshBuilder, outline: &[Vec3], normal: Vec3, half: f32, k: f32) {
    let c = outline.iter().copied().sum::<Vec3>() / outline.len() as f32;
    let n = normal.normalize() * half;
    let face = |s: f32, k: f32| -> Vec<Vec3> {
        outline.iter().map(|&p| c + (p - c) * k + n * s).collect()
    };
    if b.coarse() {
        sheet(b, outline);
    } else {
        b.loft(&[face(-1.0, k), outline.to_vec(), face(1.0, k)], true, true);
    }
}

/// A flat polygon seen from both sides: what a blade or a charge is far off.
pub(super) fn sheet(b: &mut MeshBuilder, outline: &[Vec3]) {
    b.face(outline);
    let back: Vec<Vec3> = outline.iter().rev().copied().collect();
    b.face(&back);
}

/// A plan outline `(x, y)` laid on the plane through `z0` at the centreline, falling
/// `droop` per metre out from it (a wing's anhedral; negative lifts it).
pub(super) fn plan(points: &[[f32; 2]], z0: f32, droop: f32) -> Vec<Vec3> {
    points
        .iter()
        .map(|&[x, y]| v3(x, y, z0 - y.abs() * droop))
        .collect()
}

/// A side outline `(x, z)` stood up at `y`, leaning out by `cant` (radians) about x: a
/// positive cant on the port side throws a fin hanging below its root outward.
pub(super) fn side(points: &[[f32; 2]], y: f32, cant: f32) -> Vec<Vec3> {
    points
        .iter()
        .map(|&[x, z]| v3(x, y + z * cant.sin(), z * cant.cos()))
        .collect()
}

/// A wing (port; mirror it for both) from plan `outline` at `z0`, falling `droop` per
/// metre out, `half` thick at its root, in dark plate.
pub(super) fn wing(b: &mut MeshBuilder, outline: &[[f32; 2]], z0: f32, droop: f32, half: f32) {
    dark_plate(b);
    blade(b, &plan(outline, z0, droop), Vec3::Z, half, 0.86);
}

/// A fin turned down: side outline `outline` (`(x, z)`, z measured from its root at
/// `root_z`, so negative hangs below) stood at `y`, its foot thrown out by `cant`.
pub(super) fn down_fin(b: &mut MeshBuilder, outline: &[[f32; 2]], y: f32, root_z: f32, cant: f32) {
    dark_plate(b);
    let pts: Vec<Vec3> = side(outline, y, cant)
        .into_iter()
        .map(|p| p + Vec3::Z * root_z)
        .collect();
    blade(b, &pts, v3(0.0, cant.cos(), -cant.sin()), 0.06, 0.8);
}

/// The nose blade: a keel blade (`(x, z)`) along the centreline reaching ahead of the
/// body, edged, in dark plate with a seam-dark core.
pub(super) fn nose_blade(b: &mut MeshBuilder, outline: &[[f32; 2]], half: f32) {
    dark_plate(b);
    blade(b, &side(outline, 0.0, 0.0), Vec3::Y, half, 0.7);
}

/// A course of swept plates lapped back along `u` from `o`, face out along `n`: how the
/// Regency armour a wing or a back. Fewer, longer plates at the reduced level; none far
/// off.
pub(super) fn plates(b: &mut MeshBuilder, o: Vec3, u: Vec3, n: Vec3, course: Course) {
    if b.coarse() {
        return;
    }
    dark_plate(b);
    let c = if b.fine() {
        course
    } else {
        Course {
            count: course.count.div_ceil(2),
            step: course.step * 2.0,
            ..course
        }
    };
    c.lay(b, &Frame::new(o, u, n));
}

/// Bronze workings showing between plates: a shaft from `a` to `c` with collars along
/// it. Only near.
pub(super) fn workings(b: &mut MeshBuilder, a: Vec3, c: Vec3, r: f32, collars: usize) {
    if b.coarse() {
        return;
    }
    shaft(b, a, c, r);
    if b.fine() {
        metal(b);
        let axis = c - a;
        for k in 0..collars {
            let t = (k as f32 + 0.5) / collars as f32;
            hoop_on(b, a + axis * t, axis, r + 0.03, 0.06, 0.1, 8);
        }
    }
}

/// A drive at `at` facing aft: a dark bronze ring `r` round a red core, the gas leaving
/// along -x (marked for its heat haze). Its red is the Regency's: their exhaust never
/// burns ARC's orange.
pub(super) fn drive(b: &mut MeshBuilder, at: Vec3, r: f32, len: f32) {
    b.add_exhaust(at, Vec3::NEG_X, r * 0.7);
    let x = Vec3::X;
    b.paint(GLOW_LASER);
    if b.coarse() {
        b.face(&[
            at + v3(0.0, -r, -r * 0.6),
            at + v3(0.0, r, -r * 0.6),
            at + v3(0.0, r, r * 0.6),
            at + v3(0.0, -r, r * 0.6),
        ]);
        return;
    }
    b.cylinder_between(
        at + x * (len * 0.5),
        at - x * 0.02,
        r * 0.62,
        r * 0.5,
        b.sides(8),
    );
    metal(b);
    hoop_on(
        b,
        at + x * (len * 0.5),
        x,
        r,
        0.16,
        len,
        if b.fine() { 12 } else { 6 },
    );
    if b.fine() {
        seam(b);
        hoop_on(b, at + x * (len * 0.9), x, r * 0.86, 0.1, 0.12, 12);
    }
}

/// A Gravitic cradle (port side as given): two bronze tines from `root` reaching to `tip`
/// a little above and below it, and between their ends the charge they hold, a red ball
/// whose middle is the muzzle. Far off, the charge alone.
pub(super) fn cradle(b: &mut MeshBuilder, root: Vec3, tip: Vec3, r: f32) {
    let along = (tip - root).normalize();
    let up = (Vec3::Z - along * along.z).normalize() * (r * 1.15);
    if !b.coarse() {
        seam(b);
        b.cylinder_between(
            root - along * 0.2,
            root + along * 0.25,
            r * 1.2,
            r * 1.1,
            b.sides(8),
        );
        for s in [1.0, -1.0] {
            shaft(
                b,
                root + up * (0.6 * s),
                tip + up * s - along * (r * 0.3),
                r * 0.28,
            );
        }
    }
    b.paint(GLOW_LASER);
    if b.coarse() {
        let (side, up) = (Vec3::Y * r, Vec3::Z * r);
        sheet(b, &[tip - up, tip + side, tip + up, tip - side]);
        return;
    }
    let sides = if b.fine() { 8 } else { 5 };
    b.spheroid(
        tip,
        Vec3::splat(r * 0.75),
        sides,
        if b.fine() { 4 } else { 3 },
    );
}

/// Red optics either side of the nose at `at` (port), along `along`.
pub(super) fn optics(b: &mut MeshBuilder, at: Vec3, out: Vec3, along: Vec3, len: f32) {
    if !b.fine() {
        return;
    }
    b.mirror_y(|b| red_slot(b, at, out, along, len, 0.09));
}

/// The team's mark: a swept plate pointing forward from `rear` along `u`, lying on a face
/// whose normal is `n`. Far off, a flat arrowhead.
pub(super) fn team_mark(b: &mut MeshBuilder, rear: Vec3, u: Vec3, n: Vec3, len: f32, half: f32) {
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
    armour(b, &f, &swept(len, half, 0.0, 0.35), 0.05);
}

#[cfg(test)]
pub(super) mod check {
    //! The checks every tech 2 airframe shares: it fits its blueprint's radius and height
    //! and stays off the ground, wears team colour and dark plate at every level of detail,
    //! keeps to its budget, never shows ARC's blue or orange light, and holds a charge
    //! (or a lens, `glow`) at each muzzle.
    use glam::Vec3;

    use crate::material::{GLOW, GLOW_ORANGE, PLATING_DARK, TEAM};

    /// Unit `key` from the data, checked to draw `mesh` at the model's own size.
    pub(crate) fn blueprint(
        key: &str,
        mesh: &str,
        radius: f32,
        height: f32,
    ) -> mc_data::UnitBlueprint {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let blueprints = mc_data::Blueprints::load(&dir).unwrap();
        let bp = blueprints.unit(blueprints.id_of(key).unwrap()).clone();
        assert_eq!(bp.visual.mesh, mesh);
        assert!((bp.radius.to_f32() - radius).abs() < 1e-3, "{key}: radius");
        assert!((bp.height.to_f32() - height).abs() < 1e-3, "{key}: height");
        bp
    }

    /// The muzzles of a unit's first weapon, in model metres.
    pub(crate) fn muzzles(bp: &mc_data::UnitBlueprint) -> Vec<Vec3> {
        bp.weapons[0]
            .muzzles
            .iter()
            .map(|p| Vec3::new(p.x.to_f32(), p.y.to_f32(), p.z.to_f32()))
            .collect()
    }

    pub(crate) fn airframe(key: &str, radius: f32, height: f32, muzzles: &[Vec3], glow: u32) {
        let model = crate::build_model_scaled(key, radius, height, 2).expect(key);
        let tris = |lod: usize| model.lods[lod].indices.len() / 3;
        let (full, mid, coarse) = (tris(0), tris(1), tris(2));
        let budget = super::super::super::triangles(key).unwrap_or(2600);
        assert!(
            full <= budget && full >= 250,
            "{key}: {full} (budget {budget})"
        );
        assert!(
            mid as f32 <= full as f32 * 0.5 + 20.0 && coarse < 60,
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
                (height * 0.8..=height * 1.25).contains(&top),
                "{name}: top {top}"
            );
            let low = mesh
                .vertices
                .iter()
                .map(|v| v.pos[2])
                .fold(f32::MAX, f32::min);
            assert!(low >= -1e-3, "{name}: below its belly line at {low}");
            let reach = mesh
                .vertices
                .iter()
                .map(|v| v.pos[0].hypot(v.pos[1]))
                .fold(0.0, f32::max);
            assert!(
                (radius * 0.75..=radius * 1.3).contains(&reach),
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
                !mesh
                    .vertices
                    .iter()
                    .any(|v| v.material == GLOW || v.material == GLOW_ORANGE),
                "{name}: ARC's blue or orange light"
            );
            for &m in muzzles {
                let near = mesh
                    .vertices
                    .iter()
                    .filter(|v| v.material == glow)
                    .map(|v| Vec3::from(v.pos).distance(m))
                    .fold(f32::MAX, f32::min);
                assert!(near < 0.6, "{name}: nothing held {near} m from muzzle {m}");
            }
        }
    }
}
