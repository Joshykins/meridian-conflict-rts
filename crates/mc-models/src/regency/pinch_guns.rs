//! The Regency's heavy point defences, the squeezed-plasma emplacements (docs/STYLE.md "The
//! Regency suite"): bigger, heavier machines than the Picket, on lots of their own.
//!
//! - **Pinched-plasmeric Cannon** (tech 2, a 2 x 2 lot): an armoured octagonal keep skirted
//!   in plates lapped down into spikes, a toothed bronze ring turning slowly round its top
//!   under the turret and four rams pumping between the skirt plates. On it a long wedge
//!   of a turret, plates lapped back, the gravity pinch drum across its back. The gun is a
//!   bronze bore through a column of pinch rings, a plated spine along its top, pressure
//!   ports either side behind the muzzle and a red-rimmed muzzle block.
//! - **Pinch-fusion Cannon** (tech 3, a 4 x 4 lot): the same keep twice the size, worked
//!   harder (eight rams, a second ring), and a heavier turret with the fusion chamber on
//!   its back: a caged red core held in three bronze hoops, conduits running forward from
//!   it to the breech. Its gun is longer, its pinch rings closer and more, the spine
//!   doubled, the ports paired.
//!
//! The numbers match `data/factions/regency/units/structures.ron`: a turret's pivot is the
//! weapon's `pivot` and its gun's tip the `muzzle`. The bore, rings and muzzle recoil.

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::material::*;
use crate::{part, rig};

use super::defense::skirt;
use super::kit::{cable, dark_plate, metal, seam, v3};
use super::machine::*;

pub(super) const PINCH_PIVOT: Vec3 = Vec3::new(0.0, 0.0, 8.4);
pub(super) const PINCH_MUZZLE: Vec3 = Vec3::new(11.6, 0.0, 8.4);
pub(super) const FUSION_PIVOT: Vec3 = Vec3::new(0.0, 0.0, 14.2);
pub(super) const FUSION_MUZZLE: Vec3 = Vec3::new(26.0, 0.0, 14.2);
const THICK: f32 = 0.3;

/// The emplacement: its keep's scale over the tech 2 one, its gun, and how elaborate it is.
struct Gun {
    s: f32,
    pivot: Vec3,
    muzzle: Vec3,
    /// Pinch rings along the bore.
    rings: usize,
    /// The tech 3 machine: more rams and rings, the fusion chamber, paired ports.
    fusion: bool,
}

const PINCH: Gun = Gun {
    s: 1.0,
    pivot: PINCH_PIVOT,
    muzzle: PINCH_MUZZLE,
    rings: 5,
    fusion: false,
};
const FUSION: Gun = Gun {
    s: 1.7,
    pivot: FUSION_PIVOT,
    muzzle: FUSION_MUZZLE,
    rings: 9,
    fusion: true,
};

impl Gun {
    /// A point on the bore, `x` metres out from the pivot.
    fn at(&self, x: f32) -> Vec3 {
        self.pivot + (self.muzzle - self.pivot).normalize() * x
    }

    fn length(&self) -> f32 {
        self.muzzle.distance(self.pivot)
    }
}

pub(super) fn pinch_cannon(b: &mut MeshBuilder, _tech: u8) {
    emplacement(b, &PINCH);
}

pub(super) fn fusion_cannon(b: &mut MeshBuilder, _tech: u8) {
    emplacement(b, &FUSION);
}

fn emplacement(b: &mut MeshBuilder, g: &Gun) {
    b.set_turret_pivot(g.pivot);
    b.set_arm_pivot(g.pivot);
    b.set_recoil(g.pivot, g.muzzle, 0.9 * g.s);
    b.set_spinner_pivot(Vec3::ZERO);
    if b.coarse() {
        coarse(b, g);
        return;
    }
    keep(b, g);
    b.with_part(part::TURRET, |b| {
        head(b, g);
        if g.fusion {
            chamber(b, g);
        }
        b.with_limb(rig::ARM_GUN, |b| gun(b, g));
    });
}

/// Far off: the keep, the turret's wedge, the gun in one bar, the owner's colour.
fn coarse(b: &mut MeshBuilder, g: &Gun) {
    let s = g.s;
    dark_plate(b);
    b.prism(Vec3::ZERO, 8, 9.2 * s, 6.6 * s, 5.0 * s);
    b.with_part(part::TURRET, |b| {
        dark_plate(b);
        b.beam(
            v3(-5.6 * s, 0.0, g.pivot.z),
            v3(3.4 * s, 0.0, g.pivot.z),
            Vec2::new(7.0 * s, 4.2 * s),
            Vec2::new(4.2 * s, 3.0 * s),
        );
        b.paint(TEAM);
        let z = g.pivot.z + 2.12 * s;
        b.face(&[
            v3(-4.6 * s, -1.4 * s, z),
            v3(-1.6 * s, -1.4 * s, z),
            v3(-1.6 * s, 1.4 * s, z),
            v3(-4.6 * s, 1.4 * s, z),
        ]);
        b.with_limb(rig::ARM_GUN, |b| {
            dark_plate(b);
            b.beam(
                g.at(2.0 * s),
                g.muzzle,
                Vec2::new(1.8 * s, 1.6 * s),
                Vec2::new(0.5, 0.5),
            );
        });
    });
}

/// The keep: a seam-dark plinth, an armoured octagonal drum skirted in plates lapped down
/// into spikes, rams pumping in the gaps between them, the owner's colour round its top,
/// a toothed bronze ring turning under the turret's race.
fn keep(b: &mut MeshBuilder, g: &Gun) {
    let (s, fine) = (g.s, b.fine());
    let segs = if fine { 24 } else { 8 };
    seam(b);
    b.prism(Vec3::ZERO, 8, 8.6 * s, 8.0 * s, 1.2 * s);
    dark_plate(b);
    b.prism(Vec3::Z * 1.2 * s, 8, 7.6 * s, 6.6 * s, 3.6 * s);
    let plates = if fine { 8 } else { 4 };
    skirt(b, plates, 7.2 * s, 4.2 * s, 9.9 * s, 2.1 * s);
    if g.fusion && fine {
        // A second course of skirt plates between the first, shorter.
        b.yawed(Vec3::ZERO, std::f32::consts::PI / plates as f32, |b| {
            skirt(b, plates, 7.0 * s, 3.2 * s, 8.8 * s, 1.5 * s);
        });
    }
    // Rams pumping in the gaps between the skirt plates.
    let rams = if g.fusion { 8 } else { 4 };
    for k in 0..rams {
        let a = std::f32::consts::TAU * k as f32 / rams as f32 + 0.35;
        let d = v3(a.cos(), a.sin(), 0.0);
        piston(
            b,
            d * 8.4 * s + Vec3::Z * 0.6 * s,
            d * 7.9 * s + Vec3::Z * 4.4 * s,
            0.32 * s,
            true,
        );
    }
    b.paint(TEAM);
    hoop(b, Vec3::Z * 4.85 * s, 6.2 * s, 0.7 * s, 0.1, segs);
    // The ring that turns round the top, and the race the turret rides.
    b.with_part(part::SPINNER, |b| {
        metal(b);
        hoop(b, Vec3::Z * 5.15 * s, 5.3 * s, 0.6 * s, 0.6 * s, segs);
        if fine {
            seam(b);
            teeth(
                b,
                Vec3::Z * 5.15 * s,
                5.6 * s,
                if g.fusion { 32 } else { 20 },
                v3(0.45, 0.35, 0.45) * s,
            );
        }
    });
    metal(b);
    hoop(b, Vec3::Z * 5.25 * s, 4.2 * s, 1.2 * s, 0.9 * s, segs);
    if g.fusion && fine {
        // A second, lower ring standing out of the drum's flank.
        seam(b);
        hoop(b, Vec3::Z * 3.4 * s, 7.25 * s, 0.4 * s, 0.5 * s, segs);
    }
    if fine {
        for k in 0..4 {
            let a = (45.0 + 90.0 * k as f32).to_radians();
            let d = v3(a.cos(), a.sin(), 0.0);
            red_slot(
                b,
                d * 7.3 * s + Vec3::Z * 2.6 * s,
                d + Vec3::Z * 0.25,
                v3(-d.y, d.x, 0.0),
                1.4 * s,
                0.22 * s,
            );
        }
    }
}

/// The turret: a long wedge on the race, plates lapped back over it into spikes, the
/// owner's colour on its roof, trunnion cheeks either side of the gun and the pinch drum
/// across its back.
fn head(b: &mut MeshBuilder, g: &Gun) {
    let (s, fine) = (g.s, b.fine());
    let z0 = 5.3 * s;
    let z1 = g.pivot.z + 1.2 * s;
    let top = g.pivot.z + 2.1 * s;
    let ring = |z: f32, grow: f32| -> Vec<Vec3> {
        [
            (3.6, -2.4),
            (3.6, 2.4),
            (-1.6, 4.0),
            (-5.8, 2.6),
            (-5.8, -2.6),
            (-1.6, -4.0),
        ]
        .iter()
        .map(|&(x, y): &(f32, f32)| v3((x + grow * x.signum()) * s, (y + grow * y.signum()) * s, z))
        .collect()
    };
    dark_plate(b);
    b.loft(&[ring(z0, 0.0), ring(z1, 0.0), ring(top, -0.9)], true, true);
    // Its plates: a course down the roof, one down each flank, lapped back into spikes.
    let roof = Frame::new(v3(2.6 * s, 0.0, top + 0.02), v3(-1.0, 0.0, -0.06), Vec3::Z);
    Course {
        count: if fine { 3 } else { 1 },
        step: 2.1 * s,
        len: 3.2 * s,
        half: 1.9 * s,
        tip: 0.0,
        thick: THICK,
        tail: 1.4 * s,
    }
    .lay(b, &roof);
    b.mirror_y(|b| {
        let f = Frame::new(
            v3(2.4 * s, 3.3 * s, z1 - 0.6 * s),
            v3(-1.0, 0.32, -0.1),
            v3(0.0, 1.0, 1.4),
        );
        dark_plate(b);
        Course {
            count: if fine { 3 } else { 1 },
            step: 2.0 * s,
            len: 3.0 * s,
            half: 1.3 * s,
            tip: -1.0,
            thick: THICK,
            tail: 1.3 * s,
        }
        .lay(b, &f);
        // The trunnion cheeks.
        dark_plate(b);
        b.block(
            v3(-1.2 * s, 1.35 * s, g.pivot.z - 1.4 * s),
            v3(2.0 * s, 1.95 * s, g.pivot.z + 1.4 * s),
        );
    });
    b.paint(TEAM);
    b.face(&[
        v3(-4.8 * s, -1.2 * s, top + 0.01),
        v3(-2.8 * s, -1.2 * s, top + 0.01),
        v3(-2.8 * s, 1.2 * s, top + 0.01),
        v3(-4.8 * s, 1.2 * s, top + 0.01),
    ]);
    if !g.fusion {
        // The gravity pinch drum across the back, a red slot in its end.
        let c = v3(-5.2 * s, 0.0, g.pivot.z - 0.3 * s);
        collar(b, c, Vec3::Y, 1.25 * s, 5.8 * s);
        if fine {
            seam(b);
            for y in [-2.2f32, 2.2] {
                b.cylinder_between(
                    c + Vec3::Y * (y - 0.2) * s,
                    c + Vec3::Y * (y + 0.2) * s,
                    1.45 * s,
                    1.45 * s,
                    10,
                );
            }
            red_slot(
                b,
                c + Vec3::Y * 2.95 * s,
                Vec3::Y,
                Vec3::X,
                1.2 * s,
                0.2 * s,
            );
            red_slot(
                b,
                c - Vec3::Y * 2.95 * s,
                -Vec3::Y,
                Vec3::X,
                1.2 * s,
                0.2 * s,
            );
        }
    }
}

/// The fusion chamber on the tech 3 turret's back: a red core held in three bronze hoops,
/// cage bars over it, conduits running forward to the breech either side.
fn chamber(b: &mut MeshBuilder, g: &Gun) {
    let (s, fine) = (g.s, b.fine());
    let c = v3(-5.0 * s, 0.0, g.pivot.z + 2.3 * s);
    let r = 1.6 * s;
    b.paint(GLOW_LASER);
    b.spheroid(
        c,
        Vec3::splat(r * 0.55),
        if fine { 10 } else { 6 },
        if fine { 6 } else { 4 },
    );
    metal(b);
    let segs = if fine { 18 } else { 8 };
    for axis in [Vec3::Z, v3(1.0, 0.0, 0.6), v3(-1.0, 0.0, 0.6)] {
        hoop_on(b, c, axis, r, 0.3 * s, 0.35 * s, segs);
    }
    // The cradle it sits in, and the conduits forward to the breech.
    dark_plate(b);
    b.frustum(
        c - Vec3::Z * (r + 0.9 * s),
        Vec2::new(4.4 * s, 4.0 * s),
        Vec2::new(3.2 * s, 3.0 * s),
        0.9 * s,
        Vec2::ZERO,
    );
    if fine {
        metal(b);
        b.mirror_y(|b| {
            cable(
                b,
                &[
                    c + v3(0.6 * s, 1.2 * s, -0.8 * s),
                    c + v3(2.6 * s, 1.7 * s, -1.4 * s),
                    g.pivot + v3(-1.6 * s, 1.6 * s, 0.3 * s),
                ],
                0.28 * s,
            );
        });
        // Cage bars over the top, fore and aft.
        dark_plate(b);
        for x in [-0.6f32, 0.6] {
            b.beam(
                c + v3(x * r, -r * 1.05, 0.0),
                c + v3(x * r, r * 1.05, 0.0),
                Vec2::new(0.3 * s, 0.3 * s),
                Vec2::new(0.3 * s, 0.3 * s),
            );
        }
    }
}

/// The gun: a breech block on the trunnion, then a bronze bore through its pinch rings
/// under a plated spine, pressure ports behind the muzzle and a red-rimmed muzzle block.
/// All but the breech recoils.
fn gun(b: &mut MeshBuilder, g: &Gun) {
    let (s, fine) = (g.s, b.fine());
    let dir = (g.muzzle - g.pivot).normalize();
    let side = Vec3::Y;
    let up = dir.cross(side).normalize();
    let len = g.length();
    collar(b, g.pivot, Vec3::Y, 0.8 * s, 3.2 * s);
    dark_plate(b);
    b.beam(
        g.at(0.3 * s),
        g.at(3.4 * s),
        Vec2::new(2.3 * s, 2.1 * s),
        Vec2::new(1.9 * s, 1.8 * s),
    );
    b.with_recoil(|b| {
        metal(b);
        b.cylinder_between(g.at(3.0 * s), g.at(len - 0.6 * s), 0.45 * s, 0.42 * s, 8);
        // The pinch rings, closer toward the muzzle; a red line inside two of them.
        let (first, last) = (4.0 * s, len - 2.6 * s);
        let rings = if fine { g.rings } else { 2 };
        for k in 0..rings {
            let u = k as f32 / (rings.max(2) - 1) as f32;
            let x = first + (last - first) * u.powf(0.85);
            metal(b);
            hoop_on(
                b,
                g.at(x),
                dir,
                0.95 * s,
                0.35 * s,
                0.45 * s,
                if fine { 12 } else { 6 },
            );
            if fine && (k == 1 || k + 2 == rings) {
                b.paint(GLOW_LASER);
                hoop_on(b, g.at(x), dir, 0.66 * s, 0.1 * s, 0.14 * s, 10);
            }
        }
        // The spine over them: plates lapped back from the muzzle, bronze showing below.
        let spines: &[f32] = if g.fusion { &[-0.55, 0.55] } else { &[0.0] };
        for &y in spines {
            let f = Frame::new(
                g.at(len - 2.0 * s) + up * 1.1 * s + side * y * s,
                -dir + up * 0.02,
                up + side * y * 0.4,
            );
            dark_plate(b);
            Course {
                count: if fine { 3 } else { 1 },
                step: (len - 6.0 * s) / 3.0,
                len: (len - 6.0 * s) / 3.0 + 0.6 * s,
                half: if g.fusion { 0.5 * s } else { 0.75 * s },
                tip: 0.0,
                thick: THICK,
                tail: 0.9 * s,
            }
            .lay(b, &f);
        }
        // The pressure ports either side behind the muzzle.
        let ports: &[f32] = if g.fusion { &[1.6, 2.6] } else { &[1.6] };
        b.mirror_y(|b| {
            for &back in ports {
                let x = len - back * s;
                dark_plate(b);
                b.beam(
                    g.at(x - 0.35 * s) + side * 0.7 * s,
                    g.at(x + 0.35 * s) + side * 1.15 * s,
                    Vec2::new(0.8 * s, 0.6 * s),
                    Vec2::new(0.8 * s, 0.45 * s),
                );
                if fine {
                    red_slot(b, g.at(x) + side * 1.2 * s, side, dir, 0.5 * s, 0.16 * s);
                }
            }
        });
        dark_plate(b);
        b.cylinder_between(
            g.at(len - 0.9 * s),
            g.muzzle,
            0.78 * s,
            0.72 * s,
            b.sides(8),
        );
        b.paint(GLOW_LASER);
        hoop_on(
            b,
            g.at(len - 0.12 * s),
            dir,
            0.72 * s,
            0.1 * s,
            0.14 * s,
            if fine { 12 } else { 6 },
        );
        metal(b);
        b.cylinder_between(g.at(len - 0.4 * s), g.muzzle + dir * 0.02, 0.28, 0.28, 6);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinch_cannon_fits() {
        super::super::check(
            "regency_pinch_cannon",
            10.5,
            11.0,
            Some(2),
            &[PINCH_MUZZLE.to_array()],
        );
    }

    #[test]
    fn fusion_cannon_fits() {
        super::super::check(
            "regency_fusion_cannon",
            20.0,
            17.0,
            Some(4),
            &[FUSION_MUZZLE.to_array()],
        );
    }

    /// The unit file's pivot and muzzle are the model's own.
    #[test]
    fn the_unit_files_guns_are_the_models() {
        let bp = mc_data::Blueprints::load(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"),
        )
        .unwrap();
        for (mesh, pivot, muzzle) in [
            ("regency_pinch_cannon", PINCH_PIVOT, PINCH_MUZZLE),
            ("regency_fusion_cannon", FUSION_PIVOT, FUSION_MUZZLE),
        ] {
            let unit = bp.units.iter().find(|u| u.visual.mesh == mesh).expect(mesh);
            let w = &unit.weapons[0];
            let v = |p: mc_core::FxVec3| Vec3::from(p.to_f32());
            assert!(v(w.muzzle).distance(muzzle) < 0.02, "{mesh} muzzle");
            assert!(v(w.pivot.unwrap()).distance(pivot) < 0.02, "{mesh} pivot");
        }
    }

    #[test]
    fn the_keep_has_working_machinery() {
        for key in ["regency_pinch_cannon", "regency_fusion_cannon"] {
            let model = crate::build_model_scaled(key, 10.5, 11.0, 1).unwrap();
            let lod = &model.lods[0];
            assert!(
                lod.vertices.iter().any(|v| v.part == part::SPINNER),
                "{key}: ring"
            );
            assert!(
                lod.vertices.iter().any(|v| v.part == part::PUMP),
                "{key}: rams"
            );
            assert!(
                lod.vertices.iter().any(|v| v.rig & rig::RECOIL != 0),
                "{key}: recoil"
            );
        }
    }
}
