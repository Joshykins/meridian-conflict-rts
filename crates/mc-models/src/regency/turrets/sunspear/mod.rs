//! The Sunspear (tech 3 point defence, a 4 x 4 lot): a Pinch-fusion Cannon in an
//! armoured casemate. Over its charge the fusion is squeezed into a knot just in front of
//! the gun's closed mouth, white before it goes, then launched as one fast shot.
//!
//! The gun is a faceted, banded core with four plated rails crossed round it like an X,
//! all ending flush in a closed mouth; it comes out of a mantlet in the casemate's face.
//! The casemate is a low faceted wedge, an arrowhead in plan, tall cheeks either side
//! of the gun lapped in plates swept back into spikes, swept plates down its flanks running
//! past its tail into fins; on its roof the fusion lies in a low keeled spine with vent
//! lids either side. It turns on a low chamfered keep, panelled and
//! buttressed at its cut corners.
//!
//! The light is the plates' own (`pattern::EMBER_SLOT`), slots let into them, never a lit
//! piece. One motion through the charge: the rails flare off the core (`FLARE`), the slots
//! along them rippling toward the mouth, and snap back the instant it fires as the gun
//! kicks back; the vent lids stand up with the heat and settle as it cools.
//!
//! Drawn in metres. Its pivot and muzzle ([`LINE`]) are the unit file's
//! (`data/factions/regency/units/structures.ron`); the muzzle is the knot, in front of
//! the mouth ([`MOUTH`]).

mod casemate;
mod gun;
mod keep;

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::{part, pattern, rig};

use super::super::kit::{dark_plate, metal, seam, v3};
use super::super::parts::inlay;
use super::{gun_frame, team_patch, Line};

/// The gun's trunnion and the knot of fusion: the unit file's `pivot` and `muzzle`.
pub(super) const LINE: Line = Line::new(Vec3::new(0.0, 0.0, 13.0), Vec3::new(35.0, 0.0, 13.0));
/// How far down the bore its mouth is, from the trunnion: the knot forms in front of it.
const MOUTH: f32 = 33.2;
/// Where the turret's race sits on the keep.
const DECK: f32 = 6.0;
/// The casemate's roof.
const ROOF: f32 = 15.6;
/// How far round the knot the mouth and the rails' tips stand: none closer than 0.4 of it.
#[cfg(test)]
const HOLD: f32 = 4.0;

pub(crate) fn sunspear(b: &mut MeshBuilder, _tech: u8) {
    LINE.rig(b, 2.5);
    b.set_charge_gear(LINE.pivot, 2.0);
    if b.coarse() {
        far(b);
        return;
    }
    keep::keep(b);
    b.with_part(part::TURRET, |b| {
        casemate::casemate(b);
        b.with_limb(rig::ARM_GUN, |b| gun_frame(b, &LINE, gun::gun));
    });
}

/// Far off: a block of a keep, the casemate's block, the owner's colour on its roof and
/// the gun a bar out to its mouth that still pitches.
fn far(b: &mut MeshBuilder) {
    dark_plate(b);
    b.prism(Vec3::ZERO, 4, 21.0, 19.0, DECK);
    b.with_part(part::TURRET, |b| {
        dark_plate(b);
        b.block(v3(-16.0, -9.0, DECK), v3(10.0, 9.0, ROOF - 0.6));
        team_patch(b, -10.0, -4.0, 3.0, ROOF - 0.59);
        b.with_limb(rig::ARM_GUN, |b| {
            gun_frame(b, &LINE, |b| {
                dark_plate(b);
                b.beam(
                    v3(4.0, 0.0, 0.0),
                    v3(MOUTH, 0.0, 0.0),
                    Vec2::splat(4.4),
                    Vec2::splat(3.4),
                );
            })
        });
    });
}

/// A red slot let into a face whose outward normal is `out`: `len` along `along` about
/// `at`, `h` across. In charge gear it ripples with the charge.
fn slot(b: &mut MeshBuilder, at: Vec3, out: Vec3, along: Vec3, len: f32, h: f32) {
    let along = along.normalize() * (len * 0.5);
    let across = out.normalize().cross(along).normalize() * (h * 0.5);
    inlay(
        b,
        &[
            at - along - across,
            at + along - across,
            at + along + across,
            at - along + across,
        ],
        out,
        pattern::EMBER_SLOT,
    );
}

/// Vent lids over a grille `x` long and `y` across on a roof at `z`, `lids` of them, each
/// slotted red; they stand up with the heat after the shot (`VENT`) and settle as it
/// cools, over a grille of bars.
fn vents(b: &mut MeshBuilder, x: [f32; 2], y: [f32; 2], z: f32, lids: usize) {
    use crate::gpu_consts::charge_gear::VENT;
    seam(b);
    b.block(v3(x[0], y[0], z - 0.4), v3(x[1], y[1], z));
    if b.fine() {
        metal(b);
        let bars = 2 * lids + 1;
        for k in 0..bars {
            let a = x[0] + 0.3 + (x[1] - x[0] - 0.6) * k as f32 / (bars - 1) as f32;
            b.block(
                v3(a - 0.09, y[0] + 0.15, z),
                v3(a + 0.09, y[1] - 0.15, z + 0.12),
            );
        }
    }
    let w = (x[1] - x[0]) / lids as f32;
    b.with_charge_gear(VENT, |b| {
        for k in 0..lids {
            let a = x[0] + w * k as f32 + 0.1;
            dark_plate(b);
            b.block(
                v3(a, y[0] + 0.1, z + 0.15),
                v3(a + w - 0.2, y[1] - 0.1, z + 0.45),
            );
            if b.fine() {
                slot(
                    b,
                    v3(a + (w - 0.2) * 0.5, (y[0] + y[1]) * 0.5, z + 0.45),
                    Vec3::Z,
                    Vec3::Y,
                    (y[1] - y[0]) * 0.7,
                    0.22,
                );
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sunspear_holds_its_charge() {
        super::super::super::check_charge(
            "regency_fusion_cannon",
            24.0,
            18.0,
            Some(4),
            &[LINE.muzzle.to_array()],
            HOLD,
        );
    }
}
