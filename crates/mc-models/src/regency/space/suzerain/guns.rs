//! The Suzerain's guns. The heavy guns are long faceted houses with a pinch barrel: two
//! blades one over the other, the plasma squeezed out of the gap between their tips.
//! The suppression batteries are faceted ball mounts, a short twin-bladed snout out of
//! each. The light is drawn by the plates (`pattern::EMBER_SLOT`, `EMBER_RIM`); only the
//! bore at each muzzle is violet.

use glam::{Vec2, Vec3};

use crate::builder::{MeshBuilder, Section};
use crate::material::GLOW_VIOLET;
use crate::pattern;

use super::super::super::kit::{dark_plate, metal, v3};
use super::super::super::machine::Frame;
use super::super::style::{shard, Finish};

/// A heavy gun bound to `weapon`: its house turning about `pivot`, the barrel's bore
/// `reach` ahead of it, `size` the house's half-width.
pub(super) fn heavy(b: &mut MeshBuilder, weapon: usize, pivot: Vec3, reach: f32, size: f32) {
    b.with_house(weapon, pivot, 0.3 * size, |b| {
        b.at(pivot, |b| {
            house(b, size);
            b.with_recoil(|b| pinch_barrel(b, reach, size * 0.42));
        });
    });
}

/// A suppression battery bound to `weapon`: a ball of radius `r` about `pivot`, its
/// snout's bore `reach` ahead of it.
pub(super) fn ball(b: &mut MeshBuilder, weapon: usize, pivot: Vec3, reach: f32, r: f32) {
    b.with_house(weapon, pivot, 0.2 * r, |b| {
        b.at(pivot, |b| {
            b.with_recoil(|b| {
                let sides = b.sides(8);
                dark_plate(b);
                b.with_facets(|b| b.spheroid(Vec3::ZERO, v3(r, r, r * 0.85), sides, 4));
                dark_plate(b);
                b.with_facets(|b| {
                    b.cylinder_between(
                        v3(r * 0.66, 0.0, 0.0),
                        v3(r * 0.92, 0.0, 0.0),
                        r * 0.62,
                        r * 0.4,
                        6,
                    )
                });
                // A cowl plate swept back over the ball from its face.
                let f = Frame::new(v3(r * 0.5, 0.0, r * 0.7), v3(-1.0, 0.0, -0.25), Vec3::Z);
                shard(b, &f, (r * 2.4, r * 0.55, 0.0), r * 0.12, Finish::Dark);
                pinch_barrel(b, reach, r * 0.36);
            });
        });
    });
}

/// The heavy gun's house about the origin: a long pointed hexagon in plan, its walls
/// raked in, a pale roof, a red slot let into its brow.
fn house(b: &mut MeshBuilder, s: f32) {
    let plan = [
        [s * 1.5, 0.0],
        [s * 0.8, s],
        [-s * 1.3, s * 0.95],
        [-s * 1.7, s * 0.5],
        [-s * 1.7, -s * 0.5],
        [-s * 1.3, -s * 0.95],
        [s * 0.8, -s],
    ];
    let low = -s * 0.55;
    let high = s * 0.5;
    dark_plate(b);
    b.with_facets(|b| {
        b.loft_z(
            &plan,
            &[
                Section::new(low, 0.92),
                Section::new(low + s * 0.3, 1.0),
                Section::new(high, 0.84),
            ],
        )
    });
    dark_plate(b);
    b.with_facets(|b| {
        b.loft_z(
            &plan,
            &[
                Section::new(high - 0.05, 0.78),
                Section::new(high + s * 0.12, 0.66),
            ],
        )
    });
    // Horn plates swept back off the house's flanks past its rear into spikes.
    b.mirror_y(|b| {
        let f = Frame::new(
            v3(s * 0.7, s * 0.86, s * 0.05),
            v3(-1.0, 0.16, 0.08),
            v3(0.0, 1.0, 0.5),
        );
        shard(b, &f, (s * 3.0, s * 0.32, 0.6), s * 0.12, Finish::Dark);
    });
    if !b.coarse() {
        // The brow: two raked faces either side of the barrel, each with its red optic.
        dark_plate(b);
        b.pattern(pattern::EMBER_SLOT);
        b.mirror_y(|b| {
            let k = 0.84;
            b.face(&[
                v3(s * 1.5 + 0.08, 0.0, low + s * 0.3),
                v3(s * 0.8 + 0.08, s, low + s * 0.3),
                v3(s * 0.8 * k + 0.08, s * k, high - 0.02),
                v3(s * 1.5 * k + 0.08, 0.0, high - 0.02),
            ])
        });
    }
}

/// A pinch barrel from the house forward to its bore at `reach`: a graphite breech,
/// two faceted blades over and under the bore line, closing in toward the tip, and the
/// bore's violet between their tips. `w` is a blade's half-width at its root.
pub(in crate::regency::space) fn pinch_barrel(b: &mut MeshBuilder, reach: f32, w: f32) {
    let root = reach * 0.3;
    metal(b);
    b.beam(
        v3(root * 0.4, 0.0, 0.0),
        v3(root * 1.4, 0.0, 0.0),
        Vec2::new(w * 1.1, w * 1.2),
        Vec2::new(w * 0.9, w),
    );
    if !b.coarse() {
        let gap = w * 0.5;
        for flip in [1.0f32, -1.0] {
            let rings: Vec<Vec<Vec3>> = [
                (root, w, w * 0.9, gap * 1.6),
                (reach * 0.72, w * 0.8, w * 0.6, gap * 1.1),
                (reach - 0.2, w * 0.5, w * 0.35, gap * 0.7),
            ]
            .iter()
            .map(|&(x, half, tall, off)| {
                let z = off * flip;
                let (near, far) = (z, z + tall * flip);
                vec![
                    v3(x, -half, near),
                    v3(x, half, near),
                    v3(x, half * 0.6, far),
                    v3(x, -half * 0.6, far),
                ]
            })
            .collect();
            dark_plate(b);
            b.with_facets(|b| b.loft(&rings, true, true));
        }
        if b.fine() {
            // A machined collar at the blades' root, its gasket lit.
            dark_plate(b);
            b.pattern(pattern::EMBER_RIM);
            b.with_facets(|b| {
                b.cylinder_between(
                    v3(root * 1.05, 0.0, 0.0),
                    v3(root * 1.3, 0.0, 0.0),
                    w * 1.5,
                    w * 1.5,
                    6,
                )
            });
        }
    }
    // The bore: the plasma's point between the blades' tips.
    b.paint(GLOW_VIOLET);
    let sides = b.sides(8);
    b.cylinder_between(
        v3(reach - 0.3, 0.0, 0.0),
        v3(reach, 0.0, 0.0),
        w * 0.4,
        w * 0.4,
        sides,
    );
}
