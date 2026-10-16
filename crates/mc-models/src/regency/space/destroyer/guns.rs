//! The destroyer's guns on houses of their own, each drawn about its pivot at the origin
//! facing +x at rest, +z up; `arm` stands every mount where it belongs, bound to its gun
//! slot. The red light is drawn by the plates (`pattern::EMBER_SLOT`, `EMBER_RIM`); only
//! the bores are violet.
//!
//! - Pinch-plasma battery: a twin-forked ball seated in a pocket pressed into the chine
//!   (`battery.rs`).
//! - Twin AA repeater: twin ringed barrels raised high on a cowled pedestal.
//! - Rondel: a gravity crush lens on a post, fixed in the hull (anti-missile, no weapon).

use std::f32::consts::FRAC_PI_2;

use glam::{Affine3A, Vec3};

use crate::builder::MeshBuilder;
use crate::material::GLOW_VIOLET;
use crate::pattern;

use super::super::super::kit::{dark_plate, metal, v3};
use super::super::super::machine::Frame;
use super::super::super::turrets::rondel::crush_lens;
use super::super::style::{shard, Finish};
use super::battery;

/// The AA repeater's house half-width.
pub(super) const AA: f32 = 4.5;
/// A Rondel lens's face radius.
pub(super) const RONDEL: f32 = 6.25;
/// Where a Rondel's lens looks from, off its mount's pivot: the unit file's
/// `anti_missile_mounts`.
pub(super) const RONDEL_LENS: f32 = RONDEL * 1.4;

#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Kind {
    Battery,
    Aa,
}

/// The weapon groups in the unit file's weapon order after the lance (weapon 0, laid from
/// the core, no house): weapon `1 + k` is `GROUPS[k]`, its mounts the guns of that kind in
/// `guns`' order.
pub(super) const GROUPS: [Kind; 2] = [Kind::Battery, Kind::Aa];

/// The lance's gun slot: the house slots of the groups follow it.
const LANCE_SLOTS: usize = 1;

impl Kind {
    /// How far above its mount's pivot a gun's house turns and pitches: an AA gun's
    /// trunnion on its pedestal; a battery turns about its ball.
    fn trunnion(self) -> f32 {
        match self {
            Kind::Aa => AA * 0.5,
            Kind::Battery => 0.0,
        }
    }

    /// How far its recoiling pieces kick back when it fires, metres.
    fn travel(self) -> f32 {
        match self {
            Kind::Battery => 0.8 * battery::SIZE,
            Kind::Aa => AA * 0.11,
        }
    }

    /// A gun's mouths off its house's pivot as it faces the nose: where its shots leave
    /// (the unit file's `muzzles`), in firing order.
    #[cfg(test)]
    pub(super) fn muzzles(self) -> Vec<Vec3> {
        match self {
            Kind::Battery => battery::gaps(battery::SIZE).to_vec(),
            Kind::Aa => vec![v3(AA * 3.5, AA * 0.32, 0.0), v3(AA * 3.5, -AA * 0.32, 0.0)],
        }
    }
}

/// One gun: its kind, its pivot and which way it faces at rest (radians, counter-clockwise
/// from +x seen from above). The pivot is held to a quarter metre, so the unit file can
/// give it exactly.
#[derive(Clone, Copy, Debug)]
pub(super) struct Mount {
    pub(super) kind: Kind,
    pub(super) pivot: Vec3,
    pub(super) yaw: f32,
}

impl Mount {
    /// Facing out to port, its pivot held to a quarter metre.
    pub(super) fn port(kind: Kind, pivot: Vec3) -> Self {
        Self {
            kind,
            pivot: (pivot * 4.0).round() / 4.0,
            yaw: FRAC_PI_2,
        }
    }

    /// The same gun on the starboard side.
    fn mirrored(self) -> Self {
        Self {
            pivot: v3(self.pivot.x, -self.pivot.y, self.pivot.z),
            yaw: -self.yaw,
            ..self
        }
    }

    /// Where its house turns and pitches (`MeshBuilder::with_house`).
    pub(super) fn house(&self) -> Vec3 {
        self.pivot + Vec3::Z * self.kind.trunnion()
    }
}

/// Every gun on a house, in gun-slot order: each group's port guns in `port`'s order, then
/// the same to starboard. Gun `k` of the list is the unit's gun slot `1 + k`.
pub(super) fn guns(port: &[Mount]) -> Vec<Mount> {
    GROUPS
        .iter()
        .flat_map(|&kind| {
            let mut out: Vec<Mount> = port.iter().filter(|m| m.kind == kind).copied().collect();
            let starboard: Vec<Mount> = out.iter().map(|m| m.mirrored()).collect();
            out.extend(starboard);
            out
        })
        .collect()
}

/// Each gun of `guns` (in gun-slot order) on a house of its own bound to its slot, turning
/// from where it rests. A starboard gun is drawn as its port twin, mirrored.
pub(super) fn arm(b: &mut MeshBuilder, guns: &[Mount]) {
    for (k, m) in guns.iter().enumerate() {
        let slot = LANCE_SLOTS + k;
        if m.pivot.y < 0.0 {
            let port = m.mirrored();
            b.with(Affine3A::from_scale(v3(1.0, -1.0, 1.0)), |b| {
                house(b, slot, &port)
            });
        } else {
            house(b, slot, m);
        }
    }
}

/// Gun `slot`'s house at `m`: authored about its pivot facing the nose (+x), the sim's
/// yaw turning it to where it rests.
fn house(b: &mut MeshBuilder, slot: usize, m: &Mount) {
    let at = m.house();
    b.with_house(slot, at, m.kind.travel(), |b| {
        b.at(at, |b| match m.kind {
            Kind::Battery => b.with(
                Affine3A::from_scale(Vec3::splat(battery::SIZE)),
                battery::house,
            ),
            Kind::Aa => aa(b),
        })
    });
}

/// A ringed repeater barrel from `root` forward `len`, `r` thick: graphite, two rings,
/// the bore violet.
fn ringed(b: &mut MeshBuilder, root: Vec3, len: f32, r: f32) {
    let sides = b.sides(6);
    metal(b);
    b.cylinder_between(root, root + Vec3::X * len, r, r * 0.8, sides);
    if b.fine() {
        dark_plate(b);
        b.pattern(pattern::EMBER_RIM);
        for k in [0.35, 0.7] {
            let c = root + Vec3::X * (len * k);
            b.cylinder_between(c, c + Vec3::X * (r * 1.6), r * 1.6, r * 1.6, sides);
        }
    }
    b.paint(GLOW_VIOLET);
    b.cylinder_between(
        root + Vec3::X * (len - 0.2),
        root + Vec3::X * len,
        r * 0.5,
        r * 0.5,
        sides,
    );
}

/// The AA repeater: a cowled pedestal, twin ringed barrels on a trunnion at its top, which
/// is the house's pivot (`Kind::trunnion`); authored level, the gun's pitch raises them.
fn aa(b: &mut MeshBuilder) {
    let s = AA;
    let foot = -Kind::Aa.trunnion();
    dark_plate(b);
    b.with_facets(|b| b.prism(v3(0.0, 0.0, foot - s * 0.5), 6, s * 1.1, s * 0.8, s * 0.9));
    if !b.coarse() {
        let f = Frame::new(
            v3(s * 0.6, 0.0, foot + s * 0.5),
            v3(-1.0, 0.0, -0.1),
            Vec3::Z,
        );
        shard(b, &f, (s * 2.4, s * 0.8, 0.0), s * 0.15, Finish::Dark);
    }
    b.with_recoil(|b| {
        metal(b);
        b.chamfered_box(Vec3::ZERO, v3(s * 1.2, s * 1.1, s * 0.7), s * 0.15);
        for y in [s * 0.32, -s * 0.32] {
            ringed(b, v3(s * 0.5, y, 0.0), s * 3.0, s * 0.14);
        }
    });
}

/// The Rondels: crush lenses looking up on short plated posts, fixed in the hull.
pub(super) fn rondels(b: &mut MeshBuilder, at: &[Vec3]) {
    for &p in at {
        b.at(p, rondel);
    }
}

fn rondel(b: &mut MeshBuilder) {
    let r = RONDEL;
    dark_plate(b);
    b.with_facets(|b| b.prism(v3(0.0, 0.0, -r * 0.6), 8, r * 1.5, r * 1.1, r * 0.8));
    // The lens kit is authored at a structure's scale: lay it at the lens's.
    let k = r / 0.72;
    b.with(
        Affine3A::from_translation(v3(0.0, 0.0, RONDEL_LENS))
            * Affine3A::from_scale(Vec3::splat(k)),
        |b| crush_lens(b, Vec3::ZERO, Vec3::Z, 0.72),
    );
}
