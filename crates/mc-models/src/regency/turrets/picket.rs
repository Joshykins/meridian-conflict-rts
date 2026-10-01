//! The Picket (tech 1 point defence, a one-cell lot): a Plasmeric Repeater, a low brawler
//! pouring out a stream of red bolts. Its gun is chunky and hunched, built to shed heat:
//! vents and fins that stand open to dump it, and a wide emitter mouth, not a rifle's bore.
//!
//!
//! It crouches on four bronze-jointed legs round a hexagonal hub. The repeater has a
//! hunched back of open heat louvres over red slits, a vent flap swung open each side, a
//! bronze feed under it, and a wide flat mouth with a red emitter slit and a focusing
//! nozzle. The mouth kicks back when it fires.
//!
//! Its pivot and muzzle are the unit file's (`data/factions/regency/units/structures.ron`).

use glam::Vec3;

use crate::builder::MeshBuilder;
use crate::material::*;
use crate::{part, rig};

use super::super::kit::{dark_plate, knuckle, metal, seam, segment, v3};
use super::super::machine::*;
use super::*;

pub(super) const LINE: Line = Line::new(Vec3::new(0.0, 0.0, 6.0), Vec3::new(5.2, 0.0, 6.8));
const THICK: f32 = 0.3;

pub(crate) fn picket(b: &mut MeshBuilder, _tech: u8) {
    LINE.rig(b, 0.45);
    if b.coarse() {
        coarse(
            b,
            &LINE,
            &Coarse {
                base_r: 5.4,
                base_h: 3.6,
                x0: -2.4,
                x1: 1.6,
                half: 1.6,
                top: 7.4,
                gun: Vec2::splat(1.2),
                tip: Vec2::splat(0.5),
                mouth: None,
            },
        );
        return;
    }
    legs(b);
    b.with_part(part::TURRET, |b| {
        saddle(b);
        b.with_limb(rig::ARM_GUN, |b| {
            gun_frame(b, &LINE, |b| hunched(b, LINE.len()))
        });
    });
}

/// A hexagonal hub crouched on four legs, each a plated thigh up to a bronze knee and a
/// shin down to a pad, a strut under the thigh; the race and the owner's colour on top.
fn legs(b: &mut MeshBuilder) {
    let fine = b.fine();
    seam(b);
    b.prism(Vec3::ZERO, 6, 2.5, 2.3, 0.5);
    step(b, 6, 2.2, 1.8, 0.5, 3.1);
    b.paint(TEAM);
    hoop(
        b,
        Vec3::Z * 3.62,
        1.65,
        0.35,
        0.05,
        if fine { 18 } else { 6 },
    );
    metal(b);
    hoop(b, Vec3::Z * 3.75, 1.4, 0.7, 0.3, if fine { 18 } else { 6 });
    for k in 0..4 {
        let a = (45.0 + 90.0 * k as f32).to_radians();
        b.yawed(Vec3::ZERO, a, |b| {
            let (hip, knee, foot) = (v3(1.5, 0.0, 2.6), v3(3.5, 0.0, 4.0), v3(4.8, 0.0, 0.5));
            dark_plate(b);
            segment(b, &[(hip, 0.5, 0.5), (knee, 0.42, 0.42)], Vec3::Z);
            segment(b, &[(knee, 0.38, 0.38), (foot, 0.3, 0.3)], Vec3::X);
            dark_plate(b);
            b.prism(foot.with_z(0.0), 6, 0.85, 0.6, 0.5);
            knuckle(b, knee, Vec3::Y, 0.45, 1.1);
            // The thigh's plate, swept out past the knee into its spike.
            dark_plate(b);
            armour(
                b,
                &Frame::new(hip + Vec3::Z * 0.45, knee - hip, Vec3::Z),
                &swept(3.0, 0.6, 0.0, 0.55),
                0.22,
            );
            if b.fine() {
                strut(b, v3(1.9, 0.0, 0.5), v3(3.0, 0.0, 3.5), 0.2);
            }
        });
    }
}

/// The turret's saddle on the race, a cheek either side of the gun swept back and up
/// into a spike.
fn saddle(b: &mut MeshBuilder) {
    dark_plate(b);
    b.prism(Vec3::Z * 3.9, 6, 1.9, 1.6, 0.9);
    b.mirror_y(|b| {
        dark_plate(b);
        b.block(v3(-0.7, 1.05, 4.6), v3(0.9, 1.35, 6.6));
        armour(
            b,
            &Frame::new(v3(0.9, 1.35, 5.0), v3(-1.0, 0.0, 0.6), Vec3::Y),
            &swept(3.0, 0.8, 0.0, 0.45),
            THICK,
        );
    });
}

/// The repeater in its own frame: a hunched receiver, heat louvres standing open along
/// its back over red slots, a vent flap swung open each side, a bronze feed under it, and
/// a wide flat mouth with a red emitter face that kicks back when it fires.
fn hunched(b: &mut MeshBuilder, len: f32) {
    let fine = b.fine();
    collar(b, Vec3::ZERO, Vec3::Y, 0.5, 2.2);
    dark_plate(b);
    hull_x(
        b,
        &[
            [-2.3, 1.0, 0.8, 0.1],
            [-1.4, 1.6, 1.5, 0.45],
            [0.2, 1.8, 1.9, 0.55],
            [1.8, 1.7, 1.5, 0.35],
            [3.2, 1.3, 0.9, 0.0],
        ],
        &KEELED,
    );
    if fine {
        // Louvres along the hunch, tilted back: open, dumping heat.
        for (x, z) in [(-1.0f32, 1.1), (-0.3, 1.32), (0.4, 1.42), (1.1, 1.34)] {
            dark_plate(b);
            Fin {
                len: 0.75,
                w0: 1.2,
                w1: 0.9,
                thick: 0.12,
            }
            .at(b, v3(x, 0.0, z), v3(-0.5, 0.0, 1.0), Vec3::X);
            slit(b, v3(x + 0.33, 0.0, z + 0.02), Vec3::Z, Vec3::Y, 0.6, 0.12);
        }
        metal(b);
        shaft(b, v3(-1.8, 0.0, -0.55), v3(2.4, 0.0, -0.55), 0.3);
    }
    b.mirror_y(|b| {
        // The flank vent, swung open over its red.
        dark_plate(b);
        Fin {
            len: 1.0,
            w0: 1.5,
            w1: 1.2,
            thick: 0.14,
        }
        .at(
            b,
            v3(0.7, 0.82, 0.95),
            v3(0.0, 0.85, -0.3),
            v3(0.0, 0.3, 1.0),
        );
        if fine {
            slit(b, v3(0.7, 0.87, 0.35), Vec3::Y, Vec3::X, 1.3, 0.22);
        }
    });
    b.with_recoil(|b| {
        metal(b);
        b.cylinder_between(v3(2.9, 0.0, 0.0), v3(3.6, 0.0, 0.0), 0.5, 0.5, b.sides(8));
        dark_plate(b);
        hull_x(
            b,
            &[
                [3.4, 1.2, 0.8, 0.0],
                [4.6, 2.0, 0.95, 0.0],
                [len, 2.1, 0.8, 0.0],
            ],
            &CHAMFERED,
        );
        b.paint(GLOW_LASER);
        hull_x(
            b,
            &[[len - 0.06, 1.2, 0.1, 0.0], [len + 0.02, 1.2, 0.1, 0.0]],
            &CHAMFERED,
        );
        // The focusing nozzle in the middle of the mouth.
        metal(b);
        b.cylinder_between(v3(len - 0.7, 0.0, 0.0), v3(len, 0.0, 0.0), 0.2, 0.2, 6);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picket_fits() {
        super::super::super::check("regency_barb", 5.5, 8.0, Some(1), &[LINE.muzzle.to_array()]);
    }
}
