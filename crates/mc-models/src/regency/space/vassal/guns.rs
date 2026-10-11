//! The Vassal's guns. The Pinch-plasma Lance hangs under the keel in a long faceted house
//! on a pylon, its pinch barrel (the Suzerain's, drawn out) reaching far ahead. The Twin
//! Escort Plasma Repeaters are small raked houses, two slim barrels side by side out of
//! each; the belly's hangs from its pylon. The light is drawn by the plates
//! (`pattern::EMBER_SLOT`, `EMBER_RIM`); only the bores are violet.

use glam::{Vec2, Vec3};

use crate::builder::{MeshBuilder, Section};
use crate::material::GLOW_VIOLET;
use crate::pattern;

use super::super::super::kit::{dark_plate, metal, v3};
use super::super::super::machine::Frame;
use super::super::style::{shard, Finish};
use super::super::suzerain::guns::pinch_barrel;

/// The lance bound to `weapon`: its house turning about `pivot` under the keel, the bore
/// `reach` ahead, `s` the house's half-width.
pub(super) fn lance(b: &mut MeshBuilder, weapon: usize, pivot: Vec3, reach: f32, s: f32) {
    b.with_house(weapon, pivot, 0.25 * s, |b| {
        b.at(pivot, |b| {
            let plan = [
                [s * 2.4, 0.0],
                [s * 1.2, s],
                [-s * 1.6, s * 0.9],
                [-s * 2.2, s * 0.4],
                [-s * 2.2, -s * 0.4],
                [-s * 1.6, -s * 0.9],
                [s * 1.2, -s],
            ];
            dark_plate(b);
            b.with_facets(|b| {
                b.loft_z(
                    &plan,
                    &[
                        Section::new(-s * 0.6, 0.7),
                        Section::new(-s * 0.2, 1.0),
                        Section::new(s * 0.45, 0.9),
                    ],
                )
            });
            if !b.coarse() {
                // The brow's red optics, either side of the barrel.
                dark_plate(b);
                b.pattern(pattern::EMBER_SLOT);
                b.mirror_y(|b| {
                    b.face(&[
                        v3(s * 2.4 + 0.06, 0.0, -s * 0.2),
                        v3(s * 1.2 + 0.06, s, -s * 0.2),
                        v3(s * 1.2 * 0.9 + 0.06, s * 0.9, s * 0.42),
                        v3(s * 2.4 * 0.9 + 0.06, 0.0, s * 0.42),
                    ])
                });
                // Fins swept back off the flanks past the rear into spikes.
                b.mirror_y(|b| {
                    let f = Frame::new(
                        v3(s * 0.8, s * 0.92, 0.0),
                        v3(-1.0, 0.14, -0.05),
                        v3(0.0, 1.0, -0.4),
                    );
                    shard(b, &f, (s * 3.6, s * 0.35, 0.5), s * 0.12, Finish::Dark);
                });
            }
            b.with_recoil(|b| pinch_barrel(b, reach, s * 0.5));
        });
    });
}

/// A twin repeater bound to `weapon`: a house about `pivot`, its two barrels `gap` apart
/// with their bores `reach` ahead; `s` the house's half-width.
pub(super) fn twin(b: &mut MeshBuilder, weapon: usize, pivot: Vec3, reach: f32, gap: f32, s: f32) {
    b.with_house(weapon, pivot, 0.2 * s, |b| {
        b.at(pivot, |b| {
            let plan = [
                [s * 1.3, 0.0],
                [s * 0.7, s],
                [-s * 1.1, s * 0.9],
                [-s * 1.4, s * 0.4],
                [-s * 1.4, -s * 0.4],
                [-s * 1.1, -s * 0.9],
                [s * 0.7, -s],
            ];
            dark_plate(b);
            b.with_facets(|b| {
                b.loft_z(
                    &plan,
                    &[
                        Section::new(-s * 0.5, 0.8),
                        Section::new(-s * 0.15, 1.0),
                        Section::new(s * 0.5, 0.78),
                    ],
                )
            });
            if !b.coarse() {
                // A cowl swept back over the roof, and a lit slot in its brow.
                let f = Frame::new(v3(s * 0.9, 0.0, s * 0.52), v3(-1.0, 0.0, -0.06), Vec3::Z);
                shard(b, &f, (s * 2.6, s * 0.6, 0.0), s * 0.1, Finish::Lit);
            }
            b.with_recoil(|b| {
                for y in [gap * 0.5, -gap * 0.5] {
                    let root = v3(s * 0.8, y, 0.0);
                    let tip = v3(reach, y, 0.0);
                    metal(b);
                    b.beam(
                        root,
                        tip - Vec3::X * 0.4,
                        Vec2::new(s * 0.2, s * 0.22),
                        Vec2::new(s * 0.14, s * 0.16),
                    );
                    if b.fine() {
                        // A faceted jacket a third of the way down, its gasket lit.
                        dark_plate(b);
                        b.pattern(pattern::EMBER_RIM);
                        let c = root.lerp(tip, 0.35);
                        b.with_facets(|b| {
                            b.cylinder_between(c, c + Vec3::X * (s * 0.5), s * 0.3, s * 0.27, 6)
                        });
                    }
                    b.paint(GLOW_VIOLET);
                    let sides = b.sides(6);
                    b.cylinder_between(tip - Vec3::X * 0.4, tip, s * 0.12, s * 0.12, sides);
                }
            });
        });
    });
}

/// A faceted pylon from `top` (in the hull) down to `bottom` (a house's roof), or up from
/// the deck: a tapered hexagonal post, `r` across at its root, a lit collar at the house.
pub(super) fn pylon(b: &mut MeshBuilder, top: Vec3, bottom: Vec3, r: f32) {
    dark_plate(b);
    b.with_facets(|b| b.cylinder_between(top, bottom, r, r * 0.75, 6));
    if !b.coarse() {
        dark_plate(b);
        b.pattern(pattern::EMBER_RIM);
        let d = (bottom - top).normalize_or_zero();
        let c = bottom - d * 0.6;
        b.with_facets(|b| b.cylinder_between(c, c + d * 0.4, r * 0.95, r * 0.95, 6));
    }
}
