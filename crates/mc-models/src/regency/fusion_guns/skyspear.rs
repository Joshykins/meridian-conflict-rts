//! The tech 3 mobile anti-spaceship gun: a Pinch-fusion gun raised to the sky. Its turret is a
//! low plated hull on a bronze race, the gun held high between two plated cheeks, the
//! caged fusion core standing on the turret's back.
//!
//! The gun is the Sunspear's own at [`S`] times its size: two tall rails either side of
//! a short coiled bore, lens heads at their tips, the rails parting and the heads sliding
//! out into the charge as it builds. It rides the tracked, skirted body (`chassis`).
//!
//! The unit file's pivot and muzzle are [`LINE`]'s (`regency_t3_mobile_aa`,
//! `data/factions/regency/units/land_t3.ron`).

use glam::{Affine3A, Vec3};

use crate::builder::MeshBuilder;
use crate::part;

use super::super::kit::v3;
use super::super::turrets::sunspear;
use super::chassis::{body, Hull};
use super::{coarse_gun, fusion_core, team_patch, turret_hull, yoke, Line};

/// The blueprint's radius and height.
pub(in crate::regency) const RADIUS: f32 = 7.0;
pub(in crate::regency) const HEIGHT: f32 = 10.0;

pub(super) const HULL: Hull = Hull {
    nose: 6.0,
    tail: -5.4,
    half: 3.4,
    deck: 3.0,
};
/// The Sunspear's gun at this size.
const S: f32 = 0.3;
/// The gun's line: raised 50 degrees at rest, the Sunspear's bore at [`S`].
pub(super) const LINE: Line = Line {
    pivot: Vec3::new(0.0, 0.0, 5.4),
    pitch_deg: 50.0,
    len: 25.0 * S,
};
/// How far round the charge its projectors stand: none closer than 0.4 of it.
#[cfg(test)]
pub(super) const HOLD: f32 = 4.0 * S;
/// The turret roof.
const ROOF: f32 = 4.0;

pub(super) fn draw(b: &mut MeshBuilder) {
    LINE.rig(b, HULL.deck, 0.5);
    body(b, &HULL);
    if b.coarse() {
        coarse_gun(b, &LINE, HULL.deck, ROOF, 2.6, HOLD_COARSE);
        return;
    }
    b.with_part(part::TURRET, |b| {
        turret_hull(
            b,
            &[[2.2, 1.2], [0.8, 2.15], [-2.4, 2.15], [-3.5, 1.1]],
            HULL.deck,
            ROOF,
        );
        team_patch(b, 0.6, 1.7, 0.7, ROOF + 0.01);
        yoke(
            b,
            LINE.pivot,
            1.75,
            0.38,
            [-1.1, 1.1],
            [ROOF - 0.1, LINE.pivot.z + 0.6],
        );
        fusion_core(b, v3(-2.6, 0.0, ROOF), S, S);
        LINE.gun(b, rails);
    });
}

/// The prongs' offset either side of the charge, far off.
const HOLD_COARSE: f32 = 0.9;

/// The Sunspear's rails gun at [`S`].
fn rails(b: &mut MeshBuilder) {
    b.with(Affine3A::from_scale(Vec3::splat(S)), |b| {
        sunspear::rails(b, 25.0);
        sunspear::pinch_coils(b);
    });
}
