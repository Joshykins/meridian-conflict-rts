//! The Springald (`regency_t4_artillery`, a 6 x 6 lot, the Culverin's size): the
//! Regency's map gun, a Triune Pinch-fusion Howitzer. Like the Sunspear it gathers its
//! charge in front of the bore, here three knots of fusion at once, and lobs them high as
//! one shot that is drawn as three strands wound round each other (`Weapon::braid`). The
//! `muzzle` is the middle of the charge.
//!
//! It stands on a stepped octagonal plinth ringed by plated pylons. On the plinth a long
//! turret turns on a bronze race: plated cheeks either side of the trunnion, a short
//! breech (it lobs at up to 75 degrees, so nothing long hangs behind the trunnion), the
//! fusion plant that feeds it on the turret's back with radiator banks on its flanks, and
//! the gun (`rails`) laid 45 degrees up at rest, as a map gun stands, and never lower: it
//! lobs higher still (`keeps_aim` holds it where it last fired). The plant is three
//! capacitor cells (`back`).
//!
//! Its pivot and muzzle ([`LINE`]) are the unit file's
//! (`data/factions/regency/units/strategic.ron`).

mod back;
mod rails;

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::gpu_consts::charge_gear::{HEAT_STAGE, VENT};
use crate::material::*;
use crate::{part, rig};

use super::super::kit::{dark_plate, metal, seam, v3};
use super::super::machine::*;
use super::sunspear::coil_light;
use super::*;

/// The blueprint's radius and height.
pub(in crate::regency) const RADIUS: f32 = 30.0;
pub(in crate::regency) const HEIGHT: f32 = 48.0;
/// The gun's line: the trunnion over the turret's middle, the charge 52 m out down the bore
/// laid 45 degrees up (52 cos 45 = 36.77 out, 52 sin 45 = 36.77 up).
pub(super) const LINE: Line = Line::new(Vec3::new(0.0, 0.0, 18.0), Vec3::new(36.77, 0.0, 54.77));
/// How far round the charge its projectors stand: none closer than 0.4 of it.
#[cfg(test)]
const HOLD: f32 = 5.5;
/// The race the turret turns on, and its deck.
const RACE: f32 = 15.0;
const DECK: f32 = 8.0;
/// The turret roof.
const ROOF: f32 = 13.0;

pub(in crate::regency) fn springald(b: &mut MeshBuilder, _tech: u8) {
    b.set_turret_pivot(Vec3::Z * DECK);
    b.set_arm_pivot(LINE.pivot);
    b.set_recoil(LINE.pivot, LINE.muzzle, 2.5);
    if b.coarse() {
        coarse(
            b,
            &LINE,
            &Coarse {
                base_r: 32.0,
                base_h: DECK,
                x0: -22.0,
                x1: 10.0,
                half: 9.0,
                // Up to the plant's top.
                top: ROOF + 9.0,
                gun: Vec2::new(3.8, 3.8),
                tip: Vec2::splat(3.0),
                mouth: Some((44.0, 3.6)),
            },
        );
        return;
    }
    // Gear travels drawn at twice their metres: the lenses run 2.2 m into the charge, the
    // radiator lids lift 1.8 m.
    b.set_charge_gear(v3(-15.0, 0.0, ROOF), 2.0);
    plinth(b);
    b.with_part(part::TURRET, |b| {
        house(b);
        back::draw(b);
        b.with_limb(rig::ARM_GUN, |b| {
            gun_frame(b, &LINE, |b| {
                breech(b);
                rails::draw(b);
            })
        });
    });
}

/// The stepped octagonal plinth, ringed by eight plated pylons tied in by bronze shafts,
/// a lit hairline in each pylon's face; the owner's colour round its top and the race.
fn plinth(b: &mut MeshBuilder) {
    let fine = b.fine();
    step(b, 8, 24.0, 23.0, 0.0, 3.0);
    seam(b);
    b.prism(Vec3::Z * 3.0, 8, 21.0, 21.0, 0.5);
    step(b, 8, 19.5, 17.5, 3.5, DECK - 3.5);
    for k in 0..8 {
        b.yawed(Vec3::ZERO, (22.5 + 45.0 * k as f32).to_radians(), |b| {
            b.at(v3(23.0, 0.0, 0.0), |b| step(b, 4, 3.0, 2.0, 0.0, 12.0));
            shaft(b, v3(21.5, 0.0, 9.0), v3(17.0, 0.0, 7.0), 0.6);
            if fine {
                dark_plate(b);
                armour(
                    b,
                    &Frame::new(v3(21.6, 0.0, 11.0), v3(1.0, 0.0, 0.6), v3(-0.6, 0.0, 1.0)),
                    &swept(4.6, 2.0, 0.0, 0.5),
                    0.4,
                );
                slit(b, v3(25.15, 0.0, 6.0), v3(1.0, 0.0, 0.1), Vec3::Z, 2.0, 0.4);
            }
        });
    }
    let segs = if fine { 32 } else { 12 };
    b.paint(TEAM);
    hoop(b, Vec3::Z * (DECK + 0.02), RACE + 1.6, 2.0, 0.05, segs);
    metal(b);
    hoop(b, Vec3::Z * (DECK + 0.4), RACE - 0.6, 3.0, 0.8, segs);
}

/// The turret on the race: a long plated house, the cheeks either side of the trunnion
/// and a radiator bank on either flank of its back.
fn house(b: &mut MeshBuilder) {
    plan_hull(
        b,
        &[
            [11.0, 6.0],
            [4.0, 9.5],
            [-14.0, 9.5],
            [-21.0, 6.5],
            [-23.0, 0.0],
        ],
        &[(DECK, 1.0), (DECK + 3.5, 1.0), (ROOF, 0.86)],
    );
    team_patch(b, -12.0, -4.0, 2.5, ROOF + 0.01);
    cheeks(b, [-6.0, 6.0], 6.2, [ROOF - 0.2, 21.5], 1.8);
    collar(b, LINE.pivot, Vec3::Y, 1.6, 16.0);
    radiators(b);
}

/// A radiator bank on either flank of the turret's back: a plated box, a fusion-lit floor
/// under bars (`HEAT_STAGE`: dark until the shot) and lids that stand up off it with the
/// heat after the shot (`VENT`).
fn radiators(b: &mut MeshBuilder) {
    let fine = b.fine();
    let (x0, x1, y0, y1, z0, z1) = (-20.0, -9.0, 6.0, 10.0, DECK + 2.0, ROOF + 2.5);
    b.mirror_y(|b| {
        dark_plate(b);
        b.block(v3(x0, y0, z0), v3(x1, y1, z1 - 0.6));
        coil_light(b, HEAT_STAGE);
        b.face(&[
            v3(x0 + 0.4, y0 + 0.4, z1 - 0.45),
            v3(x1 - 0.4, y0 + 0.4, z1 - 0.45),
            v3(x1 - 0.4, y1 - 0.4, z1 - 0.45),
            v3(x0 + 0.4, y1 - 0.4, z1 - 0.45),
        ]);
        if fine {
            metal(b);
            for k in 0..9 {
                let x = x0 + 0.8 + (x1 - x0 - 1.6) * k as f32 / 8.0;
                b.block(
                    v3(x - 0.12, y0 + 0.4, z1 - 0.4),
                    v3(x + 0.12, y1 - 0.4, z1 - 0.15),
                );
            }
            slit(
                b,
                v3((x0 + x1) * 0.5, y1 + 0.02, z0 + 1.4),
                Vec3::Y,
                Vec3::X,
                7.0,
                0.5,
            );
        }
        b.with_charge_gear(VENT, |b| {
            let w = (x1 - x0) / 3.0;
            for k in 0..3 {
                let a = x0 + w * k as f32;
                dark_plate(b);
                armour(
                    b,
                    &Frame::new(v3(a + 0.15, (y0 + y1) * 0.5, z1), Vec3::X, Vec3::Z),
                    &[
                        [0.0, -(y1 - y0) * 0.5],
                        [0.0, (y1 - y0) * 0.5],
                        [w - 0.3, (y1 - y0) * 0.5],
                        [w - 0.3, -(y1 - y0) * 0.5],
                    ],
                    0.35,
                );
            }
        });
    });
}

/// The breech round the trunnion in the gun's frame: a short plated block, no more than 4 m
/// behind the trunnion so it clears the roof at full elevation; the gun's thickest part, its
/// front the size of the rails' fairing's root (`rails`), a plate lapped back over its top.
fn breech(b: &mut MeshBuilder) {
    dark_plate(b);
    hull_x(
        b,
        &[
            [-4.0, 8.4, 5.6, 0.0],
            [-1.5, 10.0, 8.4, 0.0],
            [5.0, 10.4, 9.6, 0.0],
            [8.5, 9.6, 9.6, 0.0],
        ],
        &CHAMFERED,
    );
    if b.fine() {
        armour(
            b,
            &Frame::new(v3(9.5, 0.0, 4.75), v3(-1.0, 0.0, -0.05), Vec3::Z),
            &swept(11.0, 3.6, 0.0, 0.4),
            0.4,
        );
        b.mirror_y(|b| slit(b, v3(1.5, 5.22, 0.0), Vec3::Y, Vec3::X, 5.0, 0.5));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn springald_holds_its_charge() {
        super::super::super::check_charge(
            "regency_springald",
            RADIUS,
            HEIGHT,
            Some(6),
            &[LINE.muzzle.to_array()],
            HOLD,
        );
    }

    /// The unit file's pivot and muzzle are the model's own.
    #[test]
    fn springald_line_is_the_unit_files() {
        let bp = mc_data::Blueprints::load(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"),
        )
        .unwrap();
        let unit = bp.unit(bp.id_of("regency_t4_artillery").unwrap());
        assert_eq!(unit.visual.mesh, "regency_springald");
        let w = &unit.weapons[0];
        let v = |p: mc_core::FxVec3| Vec3::from(p.to_f32());
        assert!(v(w.muzzle).distance(LINE.muzzle) < 0.02);
        assert!(v(w.pivot.unwrap()).distance(LINE.pivot) < 0.02);
        assert!((unit.radius.to_f32() - RADIUS).abs() < 0.01);
        assert!((unit.height.to_f32() - HEIGHT).abs() < 0.01);
    }
}
