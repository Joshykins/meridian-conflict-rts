//! The Partisan, the Regency's tech 3 air superiority fighter: a long blade of a jet built
//! round a Pinch-fusion Rifle. A faceted hull drawn out into a nose blade, wings swept back
//! hard with swept plates lapped over them and their tips turned down, bronze workings
//! between the plates, red optics at the nose and red heat in its plasma jets.
//!
//! Finish (docs/STYLE.md "The Regency look", "Aircraft"): dark plate, dark seams, dark
//! bronze on the machinery, red only in the optics, the rifle's coils and the jets. A drone:
//! no canopy, only the optics where a pilot would sit.
//!
//! Authored at the blueprint's size (`regency_t3_air_superiority` in
//! `data/factions/regency/units/air_t3.ron`): model metres are unit metres, and the rifle's
//! muzzles are the unit file's.

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::library::ModelDef;
use crate::material::*;

use super::super::kit::{dark_plate, metal, seam, v3};
use super::blade_jet::{
    blade, chevron, feathers, fin, half_width, hull, intake, lap_pair, nozzle, optics, pin_line,
    st, surface, wing, workings, Station,
};

pub(crate) const RADIUS: f32 = 9.8;
pub(crate) const HEIGHT: f32 = 3.0;

pub(crate) const MODELS: &[ModelDef] = &[ModelDef::new("regency_partisan", RADIUS, HEIGHT, lance)];

/// The airframe: its hull, nose blade, wing and plates over it, fins, jets and rifle.
struct Design {
    hull: &'static [Station],
    nose: Vec3,
    coarse: &'static [usize],
    /// The nose blade: root, tip, half width at the root, thickness.
    blade: (Vec3, Vec3, f32, f32),
    /// The wing in plan (left side), its height and thickness.
    wing: &'static [[f32; 2]],
    wing_z: f32,
    /// Swept plates over the wing, nearest the root first.
    feathers: &'static [&'static [[f32; 2]]],
    /// Down-turned fins (left side): root, chord, span, droop, sweep, taper.
    fins: &'static [(Vec3, f32, f32, f32, f32, f32)],
    /// Plasma jets (left side): where the can starts, radius, length.
    jets: &'static [(Vec3, f32, f32)],
    /// The rifle down the middle: where the breech sits, and the muzzle.
    rifle: (Vec3, Vec3),
    /// The optics: x, half width, z, slit length.
    optics: (f32, f32, f32, f32),
}

// ---- the lance -------------------------------------------------------------------

/// The rifle's muzzle: the unit file's.
const MUZZLE: Vec3 = Vec3::new(9.3, 0.0, 1.0);

/// A long slim hull, the rifle laid down its keel and out under the nose blade, a cranked
/// delta wing, two jets.
const LANCE_HULL: [Station; 6] = [
    st(-8.4, 0.95, [1.3, 0.9], [1.75, 0.6], 2.05),
    st(-6.0, 0.7, [1.25, 1.15], [1.9, 0.72], 2.3),
    st(-2.0, 0.6, [1.2, 1.2], [1.95, 0.7], 2.35),
    st(1.5, 0.7, [1.25, 1.0], [1.85, 0.55], 2.15),
    st(4.0, 0.95, [1.35, 0.7], [1.75, 0.35], 1.9),
    st(5.6, 1.15, [1.4, 0.4], [1.6, 0.2], 1.68),
];
const LANCE_WING: [[f32; 2]; 6] = [
    [3.2, 0.7],
    [-1.4, 3.0],
    [-5.2, 7.8],
    [-6.6, 7.8],
    [-6.0, 4.0],
    [-7.6, 0.8],
];
const LANCE_FEATHERS: [&[[f32; 2]]; 3] = [
    &[
        [2.6, 0.9],
        [-1.2, 2.9],
        [-5.3, 3.4],
        [-3.0, 2.0],
        [-4.6, 0.95],
    ],
    &[
        [-0.9, 3.1],
        [-3.4, 5.6],
        [-7.0, 6.2],
        [-5.3, 4.2],
        [-4.6, 3.3],
    ],
    &[
        [-3.3, 5.8],
        [-5.0, 7.6],
        [-7.6, 8.0],
        [-6.2, 6.7],
        [-5.6, 6.0],
    ],
];
const LANCE: Design = Design {
    hull: &LANCE_HULL,
    nose: Vec3::new(6.4, 0.0, 1.45),
    coarse: &[0, 3],
    blade: (
        Vec3::new(5.9, 0.0, 1.5),
        Vec3::new(10.4, 0.0, 1.42),
        0.62,
        0.28,
    ),
    wing: &LANCE_WING,
    wing_z: 1.2,
    feathers: &LANCE_FEATHERS,
    fins: &[
        (Vec3::new(-5.2, 7.8, 1.3), 1.4, 1.2, 0.45, 0.9, 0.6),
        (Vec3::new(-6.6, 0.95, 1.1), 1.8, 1.0, 0.6, 1.0, 0.55),
    ],
    jets: &[(Vec3::new(-8.2, 0.55, 1.35), 0.5, 0.9)],
    rifle: (Vec3::new(2.0, 0.0, 0.62), MUZZLE),
    optics: (5.0, 0.42, 1.62, 0.8),
};

fn lance(b: &mut MeshBuilder, _tech: u8) {
    jet(b, &LANCE);
}

// ---- the airframe ---------------------------------------------------------------

fn jet(b: &mut MeshBuilder, d: &Design) {
    if b.coarse() {
        coarse(b, d);
        return;
    }
    hull(b, d.hull, d.nose, d.coarse);
    let (root, tip, half, thick) = d.blade;
    blade(b, root, tip, half, thick);
    b.mirror_y(|b| {
        wing(b, d.wing, d.wing_z, 0.16);
        feathers(b, d.feathers, d.wing_z + 0.16, 0.1, 0.07);
        if b.fine() {
            wing_detail(b, d);
        }
        for &(root, chord, span, droop, sweep, taper) in d.fins {
            fin(b, root, chord, span, droop, sweep, taper, 0.12);
        }
        for &(at, r, len) in d.jets {
            nozzle(b, at, r, len);
        }
    });
    let (breech, muzzle) = d.rifle;
    rifle(b, breech, muzzle);
    back(b, d);
    let (x, half, z, len) = d.optics;
    optics(b, x, half, z, len);
}

/// Up close, on the left wing: the bronze workings along its root under the plates' edges,
/// a red pin line down each plate's leading edge, an intake under the chine.
fn wing_detail(b: &mut MeshBuilder, d: &Design) {
    let w = d.wing;
    let top = d.wing_z + 0.16;
    let root_y = w[0][1] + 0.35;
    workings(
        b,
        v3(w[0][0] - 0.4, root_y, top + 0.1),
        v3(w[w.len() - 1][0] + 0.8, root_y, top + 0.1),
        0.09,
        &[0.3, 0.6],
    );
    for (k, f) in d.feathers.iter().enumerate() {
        let z = top + 0.07 * k as f32 + 0.115;
        let (a, c) = (f[0], f[1]);
        pin_line(
            b,
            v3(a[0] - 0.15, a[1] + 0.1, z),
            v3(c[0] - 0.15, c[1] - 0.05, z),
        );
    }
    let h = d.hull;
    let x = h[h.len() - 3].x;
    let s = h[h.len() - 3];
    intake(
        b,
        v3(x, s.chine[1] * 0.85, s.chine[0] - 0.25),
        v3(x - 2.2, s.chine[1] * 0.9, s.chine[0] - 0.3),
        Vec2::new(0.45, 0.35),
    );
}

/// The back: plates lapped down the hull from behind the nose, each tail swept into a
/// point over the wing root, the bronze spine showing between the pairs, and the owner's
/// chevron.
fn back(b: &mut MeshBuilder, d: &Design) {
    let h = d.hull;
    let (tail, front) = (h[0].x, h[h.len() - 2].x);
    let length = front - tail;
    // Three pairs of plates down the back, each starting a third further aft.
    for k in 0..3 {
        let x0 = front - length * (0.12 + 0.28 * k as f32);
        let x1 = x0 - length * 0.3;
        let (w0, w1) = (half_width(h, x0), half_width(h, x1));
        lap_pair(
            b,
            h,
            &[
                [x0, 0.12, 0.05],
                [x0 - 0.2, w0 * 0.75, 0.06],
                [x1 - 0.9, w1 * 1.05 + 0.25, 0.24],
                [x1 + 0.3, w1 * 0.55, 0.2],
                [x1 + 0.1, 0.12, 0.18],
            ],
            0.12,
        );
    }
    if b.fine() {
        let at = |x: f32| v3(x, 0.0, surface(h, x, 0.0) + 0.06);
        workings(
            b,
            at(front - length * 0.1),
            at(tail + length * 0.08),
            0.13,
            &[0.36, 0.66],
        );
    }
    let x = tail + length * 0.32;
    chevron(b, v3(x, 0.0, surface(h, x, 0.0) + 0.35), 1.3);
}

/// A Pinch-fusion Rifle from its `breech` forward to its `muzzle`: a dark plated shroud, a
/// bronze barrel through three red coil rings, and a split projector fork whose mouth is the
/// muzzle.
fn rifle(b: &mut MeshBuilder, breech: Vec3, muzzle: Vec3) {
    let along = (muzzle - breech).normalize();
    let len = muzzle.distance(breech);
    let shroud_end = breech + along * (len * 0.45);
    dark_plate(b);
    b.with_facets(|b| {
        b.beam(
            breech,
            shroud_end,
            Vec2::new(0.62, 0.55),
            Vec2::new(0.5, 0.42),
        )
    });
    let sides = b.sides(8);
    metal(b);
    b.cylinder_between(shroud_end, muzzle - along * 0.5, 0.16, 0.14, sides);
    if b.fine() {
        for k in 0..3 {
            let at = shroud_end + along * (0.5 + 0.55 * k as f32);
            b.paint(GLOW_LASER);
            b.cylinder_between(at, at + along * 0.12, 0.21, 0.21, sides);
        }
    }
    // The projector fork: two bronze prongs either side of the bore, seam-dark at the roots.
    seam(b);
    let fork = muzzle - along * 0.9;
    b.cylinder_between(fork - along * 0.2, fork + along * 0.05, 0.24, 0.24, sides);
    let side = Vec3::Z.cross(along).normalize_or(Vec3::Y) * 0.16;
    metal(b);
    for s in [side, -side] {
        b.beam(
            fork + s,
            muzzle + s * 0.8,
            Vec2::new(0.08, 0.16),
            Vec2::new(0.05, 0.1),
        );
    }
    b.paint(GLOW_LASER);
    b.cylinder_between(muzzle - along * 0.5, muzzle, 0.09, 0.06, sides);
}

/// Far off: the hull as a diamond, the blade, a triangle of wing a side, the chevron.
fn coarse(b: &mut MeshBuilder, d: &Design) {
    hull(b, d.hull, d.nose, d.coarse);
    let (root, tip, half, thick) = d.blade;
    blade(b, root, tip, half, thick);
    let w = d.wing;
    let tip_at = w
        .iter()
        .copied()
        .fold([0.0, 0.0], |a, p| if p[1] > a[1] { p } else { a });
    let tri = [w[0], tip_at, w[w.len() - 1]];
    b.mirror_y(|b| wing(b, &tri, d.wing_z, 0.2));
    let h = d.hull;
    let x = h[0].x + (h[h.len() - 2].x - h[0].x) * 0.32;
    chevron(b, v3(x, 0.0, surface(h, x, 0.0) + 0.3), 1.6);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fits_the_librarys_checks() {
        super::super::super::check("regency_partisan", RADIUS, HEIGHT, None, &[]);
    }

    #[test]
    fn the_unit_files_rifle_ends_at_the_models_muzzle() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let blueprints = mc_data::Blueprints::load(&dir).unwrap();
        let bp = blueprints.unit(blueprints.id_of("regency_t3_air_superiority").unwrap());
        assert_eq!(bp.visual.mesh, "regency_partisan");
        assert!(
            (bp.radius.to_f32() - RADIUS).abs() < 1e-3
                && (bp.height.to_f32() - HEIGHT).abs() < 1e-3
        );
        let m = bp.weapons[0].muzzle;
        let m = Vec3::new(m.x.to_f32(), m.y.to_f32(), m.z.to_f32());
        assert!(m.distance(MUZZLE) < 1e-3, "muzzle {m}");
        let model = crate::build_model("regency_partisan").unwrap();
        let near = model.lods[0]
            .vertices
            .iter()
            .filter(|v| v.material == GLOW_LASER)
            .map(|v| Vec3::from(v.pos).distance(MUZZLE))
            .fold(f32::MAX, f32::min);
        assert!(near < 0.15, "rifle {near} m from the muzzle");
    }
}
