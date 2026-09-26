//! The Taproot, the Naga mass extractor, on its 3 x 3 lot (36 m square): a sealed bore.
//!
//! The ore is drawn up a sealed shaft by the bore's own gravity pinch, with no hammer and
//! nothing taken from the grid. What shows is the machine that holds the shaft open:
//!
//! - A plated bore housing on a ring footing, a toothed bronze drive collar turning round
//!   its neck (`part::SPINNER`), an armoured cap over it with the owner's colour on top
//!   and a red slot round its rim where the ore comes up hot.
//! - Four outrigger legs on the diagonals brace it against the ore field: ribbed bronze
//!   struts under plates lapped down and out into spikes, a clamp ram working at each
//!   foot (`part::PUMP`), a clawed pad gripping the ground (`part::ASHORE`).
//! - On open water the whole rig rides `LIFT` higher on four piles and a caisson down into
//!   the water (`part::AFLOAT`), recorded as a `Pit` with nothing dug, which is how the
//!   shader knows to do it (`models::Pit`).

use glam::{Vec2, Vec3};

use crate::models::builder::MeshBuilder;
use crate::models::material::*;
use crate::models::{part, Pit};

use super::kit::{dark_plate, metal, seam, v3};
use super::machine::*;

/// How much higher the rig stands on open water, on its piles.
const LIFT: f32 = 3.5;
/// The legs' bearings, and how far out their feet stand.
const LEGS: [f32; 4] = [45.0, 135.0, 225.0, 315.0];
const REACH: f32 = 12.8;
/// The housing: its foot and neck radius and height; the drive collar's height (the
/// spinner's pivot) and the cap's top.
const HOUSING: (f32, f32, f32) = (5.2, 3.6, 6.4);
const COLLAR: Vec3 = Vec3::new(0.0, 0.0, 7.2);
const CAP_TOP: f32 = 10.6;
const THICK: f32 = 0.5;

pub(super) fn taproot(b: &mut MeshBuilder, _tech: u8) {
    // Nothing is dug: the pit only tells the shader to lift the rig on water.
    b.set_pit(Pit {
        open: 0.0,
        radius: 5.0,
        stroke: 0.0,
        section: 0.0,
        rack: [0.0, 0.0],
        afloat_lift: LIFT,
    });
    b.set_spinner_pivot(COLLAR);
    if b.coarse() {
        coarse(b);
        return;
    }
    housing(b);
    for deg in LEGS {
        b.yawed(Vec3::ZERO, deg.to_radians(), leg);
    }
    b.with_part(part::AFLOAT, |b| {
        // The caisson under the housing, down into the water.
        dark_plate(b);
        b.prism(Vec3::ZERO, b.sides(8), 4.6, 5.0, LIFT + 0.8);
    });
}

/// Far off: the housing and cap, a tent along each leg, the owner's colour on top.
fn coarse(b: &mut MeshBuilder) {
    dark_plate(b);
    b.prism(Vec3::ZERO, 4, HOUSING.0 + 0.6, 2.8, CAP_TOP);
    for deg in LEGS {
        b.yawed(Vec3::ZERO, deg.to_radians(), |b| {
            let (top, tip) = (v3(3.0, 0.0, 7.0), v3(REACH + 1.8, 0.0, 0.3));
            let eave = top.lerp(tip, 0.5) - Vec3::Z * 1.2;
            dark_plate(b);
            b.face(&[top, eave - Vec3::Y * 1.8, tip]);
            b.face(&[top, tip, eave + Vec3::Y * 1.8]);
        });
    }
    b.paint(TEAM);
    b.prism(Vec3::Z * CAP_TOP, 3, 1.8, 1.4, 0.3);
    b.with_part(part::AFLOAT, |b| {
        dark_plate(b);
        b.frustum_open(
            Vec3::ZERO,
            Vec2::splat(6.0),
            Vec2::splat(5.0),
            LIFT + 0.8,
            Vec2::ZERO,
        );
    });
}

/// The bore housing: its ring footing, the plated housing, the drive collar turning round
/// its neck, the cap over it.
fn housing(b: &mut MeshBuilder) {
    let fine = b.fine();
    let sides = b.sides(12);
    let (r0, r1, h) = HOUSING;
    seam(b);
    b.prism(Vec3::ZERO, sides, r0 + 1.0, r0 + 0.6, 1.2);
    dark_plate(b);
    b.prism(Vec3::Z * 1.2, sides, r0, r1, h - 1.2);
    // Plates lapped down its flanks between the legs, their spikes toward the ground.
    for k in 0..4 {
        let a = (90.0 * k as f32).to_radians();
        let d = v3(a.cos(), a.sin(), 0.0);
        let top = d * (r1 + 0.3) + Vec3::Z * (h - 0.2);
        let foot = d * (r0 + 0.5) + Vec3::Z * 1.3;
        let f = Frame::new(top, foot - top, d + Vec3::Z * 0.2);
        dark_plate(b);
        Course {
            count: 2,
            step: 1.9,
            len: 3.0,
            half: 1.8,
            tip: 0.0,
            thick: THICK,
            tail: 0.4,
        }
        .lay(b, &f);
    }
    // The neck, the drive collar turning round it, and the cap.
    metal(b);
    b.prism(Vec3::Z * h, sides, r1 * 0.8, r1 * 0.8, COLLAR.z - h + 0.6);
    b.with_part(part::SPINNER, |b| {
        metal(b);
        hoop(b, COLLAR, r1 + 0.2, 1.6, 1.1, if fine { 24 } else { 12 });
        if fine {
            seam(b);
            teeth(b, COLLAR, r1 + 1.0, 16, v3(0.6, 0.6, 0.9));
        }
    });
    dark_plate(b);
    b.frustum(
        Vec3::Z * (COLLAR.z + 0.6),
        Vec2::splat(r1 * 1.8),
        Vec2::splat(r1 * 1.1),
        CAP_TOP - COLLAR.z - 0.6,
        Vec2::ZERO,
    );
    b.paint(TEAM);
    hoop(b, Vec3::Z * (CAP_TOP + 0.06), 1.4, 0.6, 0.12, b.sides(12));
    // A red slot round the cap's rim, where the ore comes up hot.
    for k in 0..4 {
        let a = (45.0 + 90.0 * k as f32).to_radians();
        let d = v3(a.cos(), a.sin(), 0.0);
        red_slot(
            b,
            d * (r1 * 0.87) + Vec3::Z * (COLLAR.z + 1.4),
            d,
            v3(-d.y, d.x, 0.0),
            2.2,
            0.25,
        );
    }
}

/// One outrigger leg, standing out along +x (turned into place by the caller): a ribbed
/// bronze strut from the housing down to its foot, plates lapped down it into spikes, a
/// clamp ram and a clawed pad on the ground (a pile down into the water afloat).
fn leg(b: &mut MeshBuilder) {
    let fine = b.fine();
    let root = v3(HOUSING.1 + 0.4, 0.0, HOUSING.2 - 0.6);
    let foot = v3(REACH - 1.5, 0.0, 4.2);
    ribbed(b, root, foot, 0.55, if fine { 3 } else { 0 });
    // Its plates, lapped down and out along the strut.
    let down = (foot - root).normalize();
    let up = v3(-down.z, 0.0, down.x);
    let f = Frame::new(root + up * 0.7 - down * 0.4, down, up);
    dark_plate(b);
    Course {
        count: 2,
        step: 3.8,
        len: 5.6,
        half: 1.5,
        tip: 0.0,
        thick: THICK,
        tail: 1.6,
    }
    .lay(b, &f);
    // The foot block the strut lands on, and the clamp rams either side of it working
    // down into the pad.
    dark_plate(b);
    b.block(v3(REACH - 3.0, -1.3, 1.2), v3(REACH, 1.3, 4.4));
    for y in [-1.75f32, 1.75] {
        piston(
            b,
            v3(REACH - 1.5, y, 4.2),
            v3(REACH - 1.5, y, 1.0),
            0.45,
            true,
        );
    }
    red_slot(b, v3(REACH + 0.02, 0.0, 3.2), Vec3::X, Vec3::Y, 1.6, 0.2);
    b.with_part(part::ASHORE, |b| {
        // The pad, its claws bitten into the ore.
        seam(b);
        b.block(v3(REACH - 3.4, -1.8, 0.0), v3(REACH + 0.4, 1.8, 1.2));
        if b.fine() {
            metal(b);
            for y in [-1.4f32, 0.0, 1.4] {
                b.beam(
                    v3(REACH + 0.2, y, 0.9),
                    v3(REACH + 1.3, y * 1.2, 0.12),
                    Vec2::new(0.45, 0.35),
                    Vec2::new(0.15, 0.15),
                );
            }
        }
    });
    b.with_part(part::AFLOAT, |b| {
        // A pile down from the foot block into the water.
        metal(b);
        let sides = b.sides(8);
        b.cylinder_between(
            v3(REACH - 1.5, 0.0, 0.0),
            v3(REACH - 1.5, 0.0, LIFT + 1.4),
            0.8,
            0.8,
            sides,
        );
    });
}

#[cfg(test)]
mod tests {
    use crate::models::{build_model_scaled, part};

    #[test]
    fn taproot() {
        super::super::check("naga_taproot", 12.8, 11.0, Some(3), &[]);
        // Built on water it stands on piles, its grip on the ground left out, and its
        // clamp rams work.
        let model = build_model_scaled("naga_taproot", 12.8, 11.0, 1).unwrap();
        assert!(
            model.pit.is_some_and(|p| p.afloat_lift > 0.0),
            "no afloat lift"
        );
        for lod in 0..2 {
            for kind in [part::AFLOAT, part::ASHORE] {
                assert!(
                    model.lods[lod].vertices.iter().any(|v| v.part == kind),
                    "lod {lod}: no part {kind}"
                );
            }
        }
        for kind in [part::PUMP, part::SPINNER] {
            assert!(
                model.lods[0].vertices.iter().any(|v| v.part == kind),
                "no part {kind}"
            );
        }
    }
}
