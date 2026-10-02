//! The Scythe, the Regency's tech 3 heavy scavenger: a long keeled craft hung on six
//! gravity lift bells that follows the army and takes whole wrecks apart, the biggest
//! included, with two nanite heads at the ends of plated arms reaching forward from its
//! shoulders (the unit file's `reclaimer`, one head each). No rotors and no
//! plumes: it floats on its bells, red plasma crackling under each (`lift::bell`, renderer
//! `lift_fx.rs`).
//!
//! Finish (docs/STYLE.md "The Regency look"): dark plates lapped over bronze workings,
//! red optics, violet only on the lenses that work. What it takes apart goes into the
//! hoppers and the dark vault on its back.
//!
//! Rig: each head is a gun house of its own bound to its head index (`with_house`): it
//! turns about its pivot, and what is inside `with_recoil` pitches about it. Authored at the
//! blueprint's size (`regency_t3_scavenger` in `data/factions/regency/units/air_t3.ron`):
//! `HEADS` are the unit file's head pivots and emitters (the left one; the right mirrors
//! it).

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::library::ModelDef;
use crate::material::*;

use super::super::kit::{dark_plate, metal, seam, v3};
use super::super::lift::bell;
use super::super::machine::red_slot;
use super::blade_jet::{
    chevron, half_width, hull, lap_pair, optics, slab, st, surface, workings, Station,
};

pub(crate) const RADIUS: f32 = 12.0;
pub(crate) const HEIGHT: f32 = 5.6;

pub(crate) const MODELS: &[ModelDef] =
    &[ModelDef::new("regency_scythe", RADIUS, HEIGHT, harvester)];

/// A head: its pivot, where it turns, and its lens's tip, where the strands leave.
#[derive(Clone, Copy)]
pub(crate) struct Head {
    pub(crate) pivot: Vec3,
    pub(crate) emitter: Vec3,
}

/// The left head (head 0; head 1 is its mirror): the unit file's.
pub(crate) const HEADS: Head = Head {
    pivot: Vec3::new(8.6, 5.2, 2.9),
    emitter: Vec3::new(10.6, 5.2, 2.9),
};

/// Both heads: head 0 on the left as `head` gives it, head 1 its mirror on the right.
fn heads(b: &mut MeshBuilder, head: Head) {
    let flip = Vec3::new(1.0, -1.0, 1.0);
    nanite_head(b, head, 0);
    let right = Head {
        pivot: head.pivot * flip,
        emitter: head.emitter * flip,
    };
    nanite_head(b, right, 1);
}

// ---- the harvester -----------------------------------------------------------------

/// A long keeled hull on six bells, a plated arm reaching forward from each shoulder with
/// a head at its end, the vault and hoppers on its back.
const HARVESTER_HULL: [Station; 5] = [
    st(-10.0, 1.4, [2.6, 2.8], [3.9, 1.8], 4.3),
    st(-6.0, 1.1, [2.5, 3.6], [4.1, 2.3], 4.6),
    st(0.0, 1.0, [2.5, 3.8], [4.2, 2.4], 4.7),
    st(5.0, 1.2, [2.6, 3.2], [4.0, 2.0], 4.4),
    st(7.6, 1.6, [2.7, 1.8], [3.6, 1.0], 3.8),
];

fn harvester(b: &mut MeshBuilder, _tech: u8) {
    b.set_hover();
    let h = &HARVESTER_HULL;
    hull(b, h, v3(8.8, 0.0, 2.9), &[0, 3]);
    if b.coarse() {
        vault(b, -3.4, surface(h, -3.4, 0.0) - 0.15);
        chevron(b, v3(4.2, 0.0, surface(h, 4.2, 0.0) + 0.06), 2.2);
        return;
    }
    b.mirror_y(|b| {
        for (x, r) in [(-7.0f32, 1.3f32), (-1.6, 1.4), (3.8, 1.3)] {
            bell(b, v3(x, 2.6, 0.4), r, 0.85);
        }
        // The arm: a plated boom from the shoulder forward and out to the head's house.
        let shoulder = v3(3.6, 3.5, 3.1);
        let p = HEADS.pivot;
        metal(b);
        let sides = b.sides(10);
        b.cylinder_between(
            shoulder - Vec3::Y * 0.5,
            shoulder + Vec3::Y * 0.6,
            0.7,
            0.7,
            sides,
        );
        dark_plate(b);
        b.with_facets(|b| {
            b.beam(
                shoulder + Vec3::Y * 0.2,
                v3(p.x - 0.4, p.y, p.z - 0.75),
                Vec2::new(1.2, 1.0),
                Vec2::new(0.9, 0.7),
            )
        });
        if b.fine() {
            workings(
                b,
                shoulder + v3(0.3, 0.6, 0.55),
                v3(p.x - 1.2, p.y + 0.45, p.z - 0.2),
                0.1,
                &[0.5],
            );
        }
        metal(b);
        b.prism(v3(p.x, p.y, p.z - 0.85), sides, 0.85, 0.8, 0.2);
    });
    heads(b, HEADS);
    // Swept plates lapped down the flanks, three courses.
    let (tail, front) = (h[0].x, h[h.len() - 2].x);
    let length = front - tail;
    for k in 0..3 {
        let x0 = front - length * (0.05 + 0.3 * k as f32);
        let x1 = x0 - length * 0.34;
        let (w0, w1) = (half_width(h, x0), half_width(h, x1));
        lap_pair(
            b,
            h,
            &[
                [x0, w0 * 0.35, 0.06],
                [x0 - 0.3, w0 * 1.02, 0.08],
                [x1 - 1.8, w1 * 1.12 + 0.3, 0.32],
                [x1 + 0.4, w1 * 0.75, 0.28],
                [x1 + 0.2, w1 * 0.4, 0.25],
            ],
            0.18,
        );
    }
    if b.fine() {
        let at = |x: f32| v3(x, 0.0, surface(h, x, 0.0) + 0.06);
        workings(b, at(front - 0.6), at(tail + 1.0), 0.2, &[0.3, 0.7]);
    }
    vault(b, -3.4, surface(h, -3.4, 0.0) - 0.15);
    chevron(b, v3(4.2, 0.0, surface(h, 4.2, 0.0) + 0.06), 2.2);
    optics(b, 7.6, 1.1, 3.2, 1.0);
}

// ---- shared -------------------------------------------------------------------------

/// The vault on the back at `x`, standing on `z`: a dark plated box with a red slot in its
/// lid, a bronze hopper either side.
fn vault(b: &mut MeshBuilder, x: f32, z: f32) {
    dark_plate(b);
    b.with_facets(|b| {
        b.frustum(
            v3(x, 0.0, z),
            Vec2::new(4.6, 2.6),
            Vec2::new(3.8, 2.0),
            1.5,
            Vec2::new(-0.3, 0.0),
        )
    });
    if !b.coarse() {
        red_slot(b, v3(x - 0.3, 0.0, z + 1.5), Vec3::Z, Vec3::X, 2.6, 0.22);
        b.mirror_y(|b| {
            metal(b);
            let sides = b.sides(10);
            b.cylinder_between(
                v3(x + 2.4, 1.9, z + 0.75),
                v3(x - 2.6, 1.9, z + 0.75),
                0.7,
                0.7,
                sides,
            );
            if b.fine() {
                seam(b);
                for dx in [-1.6f32, 0.0, 1.6] {
                    b.cylinder_between(
                        v3(x + dx - 0.12, 1.9, z + 0.75),
                        v3(x + dx + 0.12, 1.9, z + 0.75),
                        0.78,
                        0.78,
                        sides,
                    );
                }
            }
        });
    }
}

/// A nanite head bound to head `index`: a bronze yaw collar and armoured cheeks that
/// turn, and on its trunnion a keeled cowl with red optics, a bronze nozzle out of a seam
/// collar and the violet lens whose tip is the emitter.
fn nanite_head(b: &mut MeshBuilder, head: Head, index: usize) {
    let (p, tip) = (head.pivot, head.emitter);
    b.with_house(index, p, 0.0, |b| {
        metal(b);
        let sides = b.sides(12);
        b.prism(v3(p.x, p.y, p.z - 0.7), sides, 0.9, 0.82, 0.22);
        // The cheeks either side of the trunnion.
        dark_plate(b);
        let cheek = [
            [p.x - 0.8, p.z - 0.5],
            [p.x + 0.55, p.z - 0.5],
            [p.x + 0.45, p.z + 0.25],
            [p.x - 0.05, p.z + 0.55],
            [p.x - 0.9, p.z + 0.1],
        ];
        for y in [p.y + 0.62, p.y - 0.8] {
            let side: Vec<Vec3> = cheek.iter().map(|&[x, z]| v3(x, y, z)).collect();
            slab(b, &side, Vec3::Y * 0.18, false);
        }
        b.with_recoil(|b| {
            metal(b);
            b.cylinder_between(p - Vec3::Y * 0.6, p + Vec3::Y * 0.6, 0.3, 0.3, sides);
            dark_plate(b);
            b.with_facets(|b| {
                b.beam(
                    p - Vec3::X * 0.9,
                    p + Vec3::X * 0.9,
                    Vec2::new(0.9, 0.8),
                    Vec2::new(0.75, 0.62),
                )
            });
            if b.fine() {
                red_slot(b, p + v3(0.4, 0.0, 0.42), Vec3::Z, Vec3::X, 0.7, 0.12);
            }
            let collar = p + Vec3::X * 0.9;
            seam(b);
            b.cylinder_between(collar, collar + Vec3::X * 0.18, 0.36, 0.36, sides);
            let lens = tip - Vec3::X * 0.4;
            metal(b);
            b.cylinder_between(collar + Vec3::X * 0.18, lens, 0.2, 0.16, sides);
            b.paint(GLOW_VIOLET);
            b.cylinder_between(lens, tip, 0.24, 0.06, sides);
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fits_the_librarys_checks() {
        super::super::super::check("regency_scythe", RADIUS, HEIGHT, None, &[]);
    }

    #[test]
    fn floats_on_bells_with_a_lens_at_each_emitter() {
        let model = crate::build_model("regency_scythe").unwrap();
        assert!(model.hover && model.legs.is_none() && model.treads.is_none());
        assert_eq!(model.houses.len(), 2, "a house a head");
        assert_eq!(model.lifts.len(), 6, "six bells");
        let violet: Vec<Vec3> = model.lods[0]
            .vertices
            .iter()
            .filter(|v| v.material == GLOW_VIOLET)
            .map(|v| Vec3::from(v.pos))
            .collect();
        for tip in [HEADS.emitter, HEADS.emitter * Vec3::new(1.0, -1.0, 1.0)] {
            let near = violet
                .iter()
                .map(|p| p.distance(tip))
                .fold(f32::MAX, f32::min);
            assert!(near < 0.08, "lens {near} m from the emitter {tip}");
        }
    }

    #[test]
    fn the_unit_files_heads_are_the_models() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let blueprints = mc_data::Blueprints::load(&dir).unwrap();
        let bp = blueprints.unit(blueprints.id_of("regency_t3_scavenger").unwrap());
        assert_eq!(bp.visual.mesh, "regency_scythe");
        assert!(
            (bp.radius.to_f32() - RADIUS).abs() < 1e-3
                && (bp.height.to_f32() - HEIGHT).abs() < 1e-3
        );
        let v = |p: mc_core::FxVec3| Vec3::new(p.x.to_f32(), p.y.to_f32(), p.z.to_f32());
        let reclaimer = bp.reclaimer.expect("it reclaims");
        let heads = reclaimer.heads();
        assert_eq!(heads.len(), 2);
        let flip = Vec3::new(1.0, -1.0, 1.0);
        for (head, k) in heads.iter().zip([Vec3::ONE, flip]) {
            assert!(v(head.pivot.expect("a pivot")).distance(HEADS.pivot * k) < 1e-3);
            assert!(v(head.emitter).distance(HEADS.emitter * k) < 1e-3);
        }
    }
}
