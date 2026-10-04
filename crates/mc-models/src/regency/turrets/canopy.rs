//! The Canopy (anti-air, a one-cell lot): a Plasmeric Repeater rippling plasma bolts at
//! aircraft, one out of each barrel in turn: the organ. Four short tubes side by
//! side in one clamped block on a tripod of plated struts round a bronze column. The organ
//! is drawn level; the sim holds it up at the sky at rest, so it reads as anti-air from
//! any angle. Built at sea it stands on a triangular raft.
//!
//! Its pivot and muzzles are the unit file's (`data/factions/regency/units/structures.ron`):
//! `muzzle` is the middle of the row of tube mouths, `muzzles` the four mouths.

use glam::{Vec2, Vec3};

use crate::builder::{MeshBuilder, Section};
use crate::material::*;
use crate::{part, rig};

use super::super::kit::{dark_plate, metal, seam, segment, v3};
use super::super::machine::*;
use super::*;

/// The organ's trunnion and its middle muzzle, the bore level. Its four tubes stand side
/// by side across it (`TUBES`), their mouths in a row through the muzzle.
pub(super) const LINE: Line = Line::new(Vec3::new(0.0, 0.0, 5.8), Vec3::new(3.8, 0.0, 5.8));
/// The tubes across the bore.
const TUBES: [f32; 4] = [-1.35, -0.45, 0.45, 1.35];
const THICK: f32 = 0.3;

pub(crate) fn canopy(b: &mut MeshBuilder, _tech: u8) {
    LINE.rig(b, 0.4);
    if b.coarse() {
        coarse(
            b,
            &LINE,
            &Coarse {
                base_r: 5.6,
                base_h: 4.8,
                x0: -1.8,
                x1: 1.2,
                half: 2.0,
                top: LINE.pivot.z + 0.6,
                gun: Vec2::new(1.1, 1.2),
                tip: Vec2::new(0.5, 0.4),
                mouth: None,
            },
        );
        return;
    }
    tripod(b);
    raft(b);
    b.with_part(part::TURRET, |b| {
        cradle(b, 5.3, 2.05);
        b.with_limb(rig::ARM_GUN, |b| {
            gun_frame(b, &LINE, |b| organ(b, LINE.len()))
        });
    });
}

/// Built at sea, the tripod stands on one triangular raft (`part::AFLOAT`, drawn only in
/// water): a plated pontoon under all three feet with a bronze rim round its deck.
fn raft(b: &mut MeshBuilder) {
    b.with_part(part::AFLOAT, |b| {
        let plan: Vec<[f32; 2]> = (0..3)
            .flat_map(|k| {
                let a = (120.0 * k as f32).to_radians();
                [a - 0.3, a + 0.3].map(|t| [t.cos() * 6.0, t.sin() * 6.0])
            })
            .collect();
        dark_plate(b);
        if !b.fine() {
            b.loft_z(&plan, &[Section::new(-1.1, 0.9), Section::new(0.38, 1.0)]);
            return;
        }
        b.loft_z(
            &plan,
            &[
                Section::new(-1.1, 0.86),
                Section::new(-0.4, 0.98),
                Section::new(0.0, 1.0),
            ],
        );
        metal(b);
        b.loft_z(
            &plan,
            &[
                Section::new(0.0, 1.0),
                Section::new(0.3, 1.0),
                Section::new(0.38, 0.97),
            ],
        );
    });
}

/// A bronze column on three plated struts raked out to pads, a strut under each,
/// and a collar on top with the owner's colour round it.
fn tripod(b: &mut MeshBuilder) {
    let fine = b.fine();
    seam(b);
    b.prism(Vec3::ZERO, 6, 1.6, 1.5, 0.5);
    shaft(b, Vec3::Z * 0.5, Vec3::Z * 4.8, 0.7);
    for k in 0..3 {
        b.yawed(Vec3::ZERO, (120.0 * k as f32).to_radians(), |b| {
            let (hip, foot) = (v3(0.8, 0.0, 4.2), v3(4.9, 0.0, 0.5));
            dark_plate(b);
            segment(b, &[(hip, 0.45, 0.45), (foot, 0.32, 0.32)], Vec3::Z);
            b.prism(foot.with_z(0.0), 6, 0.9, 0.65, 0.5);
            dark_plate(b);
            armour(
                b,
                &Frame::new(hip + Vec3::Z * 0.45, foot - hip, v3(1.0, 0.0, 1.0)),
                &swept(3.4, 0.6, 0.0, 0.6),
                0.22,
            );
            strut(b, v3(0.9, 0.0, 0.5), v3(2.7, 0.0, 2.5), 0.2);
            if fine {
                slit(
                    b,
                    v3(3.0, 0.0, 2.4),
                    v3(1.0, 0.0, 1.0),
                    v3(1.0, 0.0, -1.0),
                    0.9,
                    0.12,
                );
            }
        });
    }
    step(b, 6, 1.9, 1.7, 4.8, 0.5);
    b.paint(TEAM);
    hoop(
        b,
        Vec3::Z * 5.32,
        1.45,
        0.4,
        0.05,
        if fine { 16 } else { 6 },
    );
}

/// The turret: a saddle on the base's top at `z` and a cheek either side of the guns,
/// `half` out, each swept back and up into a spike.
fn cradle(b: &mut MeshBuilder, z: f32, half: f32) {
    step(b, 6, 1.6, 1.3, z, 0.5);
    b.mirror_y(|b| {
        dark_plate(b);
        b.block(v3(-1.0, half - 0.2, z + 0.1), v3(0.9, half + 0.2, z + 1.4));
        armour(
            b,
            &Frame::new(v3(0.9, half + 0.2, z + 0.4), v3(-1.0, 0.0, 0.5), Vec3::Y),
            &swept(2.6, 0.6, 0.0, 0.45),
            THICK,
        );
    });
}

/// The organ in its own frame: four tubes in a row through a plated breech and two
/// clamps, a red sight on the breech; the inner tubes and their hot-rimmed sleeves recoil.
fn organ(b: &mut MeshBuilder, len: f32) {
    let fine = b.fine();
    collar(b, Vec3::ZERO, Vec3::Y, 0.45, 4.3);
    dark_plate(b);
    hull_x(
        b,
        &[[-1.6, 3.4, 1.2, 0.0], [-0.3, 3.6, 1.3, 0.0]],
        &CHAMFERED,
    );
    let clamps: &[f32] = if fine { &[0.6, 2.0] } else { &[1.3] };
    for &x in clamps {
        dark_plate(b);
        hull_x(
            b,
            &[[x - 0.25, 3.9, 1.0, 0.0], [x + 0.25, 3.9, 1.0, 0.0]],
            &CHAMFERED,
        );
    }
    if fine {
        slit(b, v3(-0.9, 0.0, 0.66), Vec3::Z, Vec3::Y, 2.2, 0.14);
    }
    let sides = b.sides(8);
    for y in TUBES {
        dark_plate(b);
        b.cylinder_between(v3(-0.3, y, 0.0), v3(2.7, y, 0.0), 0.38, 0.36, sides);
        b.with_recoil(|b| {
            if fine {
                metal(b);
                b.cylinder_between(v3(2.5, y, 0.0), v3(3.4, y, 0.0), 0.26, 0.26, sides);
            }
            dark_plate(b);
            b.cylinder_between(v3(3.2, y, 0.0), v3(len, y, 0.0), 0.36, 0.4, sides);
            if fine {
                mouth_rim(b, v3(len, y, 0.0), 0.4, sides);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canopy_fits() {
        super::super::super::check(
            "regency_spitter",
            5.5,
            7.5,
            Some(1),
            &[LINE.muzzle.to_array()],
        );
    }

    /// The unit file's muzzles are the four tube mouths, in a row through its muzzle.
    #[test]
    fn the_unit_files_muzzles_are_the_tubes() {
        let bp = mc_data::Blueprints::load(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"),
        )
        .unwrap();
        let unit = bp
            .units
            .iter()
            .find(|u| u.visual.mesh == "regency_spitter")
            .unwrap();
        let mouths: Vec<Vec3> = unit.weapons[0]
            .muzzles
            .iter()
            .map(|p| Vec3::from(p.to_f32()))
            .collect();
        assert_eq!(mouths.len(), TUBES.len());
        for (m, y) in mouths.iter().zip(TUBES) {
            assert!(
                m.distance(LINE.muzzle + Vec3::Y * y) < 0.02,
                "tube {y}: {m}"
            );
        }
    }
}
