//! The Regency's air force (`data/factions/regency/units/air.ron`, `air_t3.ron`,
//! docs/AIR_ROSTER.md "The Regency's air force"). Their jets are drones in the Cybertronian-jet manner (docs/STYLE.md
//! "The Regency look"): a nose blade, down-turned fins, dark plates lapped back over bronze
//! workings, red optics, and red heat in the exhausts where ARC's burn orange. What hovers
//! hangs on lift bells (`super::lift`), red plasma under them: no rotors, no jet plumes.
//! One file per airframe, each with its own catalogue entries (`MODELS`) and its own
//! planform; the pieces here are only the small parts they share (the tech 3 jets share
//! theirs in `blade_jet`).

pub(crate) mod augur;
mod blade_jet;
pub(crate) mod coffer;
pub(crate) mod flechette;
pub(crate) mod maul;
pub(crate) mod partisan;
pub(crate) mod petard;
pub(crate) mod quarrel;

use glam::{Affine3A, Vec3};

use crate::builder::MeshBuilder;
use crate::material::*;

use super::kit::{dark_plate, metal, v3};
use super::machine::{armour, red_slot, swept, Frame};
use super::plating::plate;

/// One station of a faceted fuselage at `x`: its half width at the chines, and the z of
/// its keel, its chines and the ridge along its back.
#[derive(Clone, Copy, Debug)]
pub(super) struct St {
    pub(super) x: f32,
    pub(super) w: f32,
    pub(super) keel: f32,
    pub(super) chine: f32,
    pub(super) ridge: f32,
}

pub(super) const fn st(x: f32, w: f32, keel: f32, chine: f32, ridge: f32) -> St {
    St {
        x,
        w,
        keel,
        chine,
        ridge,
    }
}

/// A faceted fuselage lofted tail first through `stations`, offset `y` across, closing to
/// a point at `nose` if given: a keel along the bottom, flanks out to the chines, the back
/// raked in to a ridge. Flat facets meeting at clear edges, in the current paint.
pub(super) fn body(b: &mut MeshBuilder, stations: &[St], y: f32, nose: Option<Vec3>) {
    let fine = !b.coarse();
    let ring = |s: &St| -> Vec<Vec3> {
        let p = |dy: f32, z: f32| v3(s.x, y + dy, z);
        let shoulder = s.ridge - (s.ridge - s.chine) * 0.3;
        if fine {
            vec![
                p(0.0, s.keel),
                p(-s.w, s.chine),
                p(-s.w * 0.55, shoulder),
                p(0.0, s.ridge),
                p(s.w * 0.55, shoulder),
                p(s.w, s.chine),
            ]
        } else {
            vec![
                p(0.0, s.keel),
                p(-s.w, s.chine),
                p(0.0, s.ridge),
                p(s.w, s.chine),
            ]
        }
    };
    // Far off: its tail, middle and nose stations alone.
    let picked: Vec<St> = if fine || stations.len() <= 3 {
        stations.to_vec()
    } else {
        vec![
            stations[0],
            stations[stations.len() / 2],
            stations[stations.len() - 1],
        ]
    };
    let mut rings: Vec<Vec<Vec3>> = picked.iter().map(ring).collect();
    if let Some(n) = nose {
        let k = rings[0].len();
        rings.push(vec![n; k]);
    }
    b.with_facets(|b| b.loft(&rings, true, nose.is_none()));
}

/// A flat plate in plan: `outline` (x, y) laid at `z`, `t` thick, in the current paint:
/// a wing, a nose blade, a plate across a back.
pub(super) fn sheet(b: &mut MeshBuilder, outline: &[[f32; 2]], z: f32, t: f32) {
    let points: Vec<Vec3> = outline.iter().map(|&[x, y]| v3(x, y, z)).collect();
    b.with_facets(|b| plate(b, &points, Vec3::Z * t));
}

/// Far off, a plate in plan as one face up: `outline` (x, y) at `z`.
pub(super) fn flat(b: &mut MeshBuilder, outline: &[[f32; 2]], z: f32) {
    let points: Vec<Vec3> = outline.iter().map(|&[x, y]| v3(x, y, z)).collect();
    // Wound to face up whichever way the outline runs.
    let area: f32 = points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .map(|(p, q)| p.x * q.y - q.x * p.y)
        .sum();
    if area < 0.0 {
        b.face(&points.into_iter().rev().collect::<Vec<_>>());
    } else {
        b.face(&points);
    }
}

/// Far off, a blade standing over a tail as one face seen edge on from above, at `y`:
/// from `x0` back to `x1`, its top at `top`.
pub(super) fn tail_face(b: &mut MeshBuilder, y: f32, x0: f32, x1: f32, base: f32, top: f32) {
    let k = x0 - x1;
    b.face(&[
        v3(x0, y, base),
        v3(x1 + k * 0.45, y, top),
        v3(x1, y, top),
        v3(x1 + k * 0.1, y, base),
    ]);
}

/// A down-turned fin hanging from `root` on the left side: `outline` (x, z) below its root
/// line (z negative), `t` thick, canted out `cant` radians at its foot.
pub(super) fn fin(b: &mut MeshBuilder, root: Vec3, cant: f32, outline: &[[f32; 2]], t: f32) {
    let at = Affine3A::from_translation(root) * Affine3A::from_rotation_x(cant);
    b.with(at, |b| {
        b.with_facets(|b| b.extrude_y(outline, -t * 0.5, t * 0.5))
    });
}

/// A course of `count` swept plates lapped back along `u` from `o`, faces toward `n`, each
/// `len` long and `half` either side, `step` apart, the last drawn out into a spike: the
/// layered armour over the bronze. In dark plate; close up only past the first.
#[expect(
    clippy::too_many_arguments,
    reason = "a course is placed by all of these"
)]
pub(super) fn lap(
    b: &mut MeshBuilder,
    o: Vec3,
    u: Vec3,
    n: Vec3,
    count: usize,
    step: f32,
    len: f32,
    half: f32,
    thick: f32,
) {
    dark_plate(b);
    let f = Frame::new(o, u, n);
    let count = if b.fine() { count } else { count.min(1) };
    for k in 0..count {
        let last = k + 1 == count;
        let g = Frame::new(f.at(k as f32 * step, 0.0, 0.0), f.u + f.n * 0.06, f.n);
        let long = if last { len * 1.35 } else { len };
        armour(b, &g, &swept(long, half, 0.0, 0.45), thick);
    }
}

/// A jet's exhaust, its mouth at `mouth` facing aft: a bronze can `len` long and `r`
/// across, a dark lip round its mouth and red heat set in it (`GLOW_LASER`; ARC's burn
/// orange).
pub(super) fn exhaust(b: &mut MeshBuilder, mouth: Vec3, r: f32, len: f32) {
    b.add_exhaust(mouth, -Vec3::X, r);
    let sides = b.sides(8);
    metal(b);
    b.cylinder_between(mouth + Vec3::X * len, mouth, r * 0.85, r, sides);
    if b.fine() {
        dark_plate(b);
        b.cylinder_between(
            mouth + Vec3::X * (r * 0.5),
            mouth - Vec3::X * 0.01,
            r * 1.12,
            r * 1.08,
            sides,
        );
    }
    b.paint(GLOW_LASER);
    b.cylinder_between(
        mouth + Vec3::X * 0.05,
        mouth - Vec3::X * 0.03,
        r * 0.7,
        r * 0.7,
        sides,
    );
}

/// A pair of red optics, one either side: a lit slot at `at` (the left one) on a face
/// whose outward normal is `out`, `len` along `along`, `h` across.
pub(super) fn optics(b: &mut MeshBuilder, at: Vec3, out: Vec3, along: Vec3, len: f32, h: f32) {
    b.mirror_y(|b| red_slot(b, at, out, along, len, h));
}

/// The team's mark on a back: a chevron pointing forward, its point at `at`, `k` long.
pub(super) fn chevron(b: &mut MeshBuilder, at: Vec3, k: f32) {
    b.paint(TEAM);
    b.mirror_y(|b| {
        b.face(&[
            at,
            at + v3(-1.0, 0.55, 0.0) * k,
            at + v3(-1.3, 0.4, 0.0) * k,
            at + v3(-0.3, 0.0, 0.0) * k,
        ])
    });
}

#[cfg(test)]
mod tests {
    use crate::material::*;
    use crate::{build_model, part};
    use glam::Vec3;

    /// The shipped unit `key` and the roster it is in.
    pub(super) fn blueprint(key: &str) -> (mc_data::Blueprints, mc_data::BlueprintId) {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let blueprints = mc_data::Blueprints::load(&dir).unwrap();
        let id = blueprints.id_of(key).unwrap();
        (blueprints, id)
    }

    /// A point of the unit file, in metres.
    pub(super) fn v(p: mc_core::FxVec3) -> Vec3 {
        Vec3::new(p.x.to_f32(), p.y.to_f32(), p.z.to_f32())
    }

    /// What every Regency jet keeps to: no ARC light anywhere, red heat in its exhausts and
    /// red optics, and nothing turning about the hull but its guns.
    pub(super) fn reads_as_a_regency_jet(key: &str) {
        let model = build_model(key).unwrap();
        for (lod, mesh) in model.lods.iter().enumerate() {
            // Far off the red is too small to see; it is left off.
            assert!(
                lod == 2 || mesh.vertices.iter().any(|v| v.material == GLOW_LASER),
                "{key} lod{lod}: no red"
            );
            assert!(
                !mesh
                    .vertices
                    .iter()
                    .any(|v| v.material == GLOW || v.material == GLOW_ORANGE),
                "{key} lod{lod}: ARC's light"
            );
            assert!(
                mesh.vertices
                    .iter()
                    .all(|v| v.part == part::HULL || v.part == part::TURRET),
                "{key} lod{lod}: a moving part"
            );
        }
        // Symmetrical: every vertex has its mirror across the centre line.
        let mesh = &model.lods[0];
        let points: Vec<Vec3> = mesh.vertices.iter().map(|v| Vec3::from(v.pos)).collect();
        for p in points.iter().step_by(7) {
            let m = Vec3::new(p.x, -p.y, p.z);
            assert!(
                points.iter().any(|q| q.distance(m) < 1e-3),
                "{key}: {p} has no mirror"
            );
        }
    }
}
