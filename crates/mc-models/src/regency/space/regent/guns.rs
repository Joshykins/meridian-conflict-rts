//! The Regent's guns, each drawn about its pivot at the origin facing +x at rest, +z up;
//! `arm` stands every mount of a hull's `Mount` list where it belongs. The red light is
//! drawn by the plates (`pattern::EMBER_SLOT`, `EMBER_RIM`); only the bores are violet.
//!
//! The kinds, by what they are for (sizes as built):
//! - Grand pinch-plasma battery: the Scourge's twin-forked ball at five times its size,
//!   seated in a pocket pressed into the chine.
//! - Heavy pinch-fusion lance: a long faceted house with a pinch barrel, in a deck well
//!   (dorsal) or hung from the keel (ventral).
//! - Suppression battery: a plated ball with a short pinch snout on the shoulder.
//! - Keel gun: a ball hung under the keel, twin pinch snouts.
//! - Twin repeater: a low house, two ringed barrels.
//! - AA repeater: twin ringed barrels raised high on a cowled pedestal.
//! - AA lance: a slim house, one long barrel raised steep, its coils lit.
//! - Rondel: a gravity crush lens on a post.
//! - Seeker spine: a long armoured block of hatched cells.

use std::f32::consts::{FRAC_PI_2, PI};

use glam::{Affine3A, Vec2, Vec3};

use crate::builder::{chamfered_rect, CellGrid, MeshBuilder, Section};
use crate::material::GLOW_VIOLET;
use crate::{part, pattern};

use super::super::super::kit::{dark_plate, metal, seam, v3};
use super::super::super::machine::{hoop_on, Frame};
use super::super::super::turrets::rondel::crush_lens;
use super::super::destroyer::battery;
use super::super::dress::pale_plate;
use super::super::style::{shard, Finish};
use super::super::suzerain::guns::pinch_barrel;

/// The grand battery's scale over the Scourge's authored ball (radius 8).
pub(super) const BATTERY: f32 = 5.0;
/// The grand battery ball's radius as built: the chine pockets are cut to it.
pub(super) const BATTERY_R: f32 = 8.0 * BATTERY;
/// The heavy lance house's half-width, and its bore ahead of the pivot.
pub(super) const LANCE: f32 = 36.0;
pub(super) const LANCE_REACH: f32 = 210.0;
/// The suppression and keel balls' radii.
pub(super) const SUPPRESSION: f32 = 20.0;
pub(super) const KEEL: f32 = 20.0;
/// The repeaters' and AA guns' house half-widths.
pub(super) const REPEATER: f32 = 13.0;
pub(super) const AA: f32 = 11.0;
pub(super) const AA_LANCE: f32 = 13.0;
/// A Rondel lens's face radius.
pub(super) const RONDEL: f32 = 15.0;
/// The seeker cells' pitch and each lid's half-width.
pub(super) const CELL_PITCH: f32 = 20.0;
const CELL_HALF: f32 = 8.0;

#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Kind {
    Battery,
    Lance,
    VentralLance,
    Suppression,
    Keel,
    Repeater,
    Aa,
    AaLance,
    Rondel,
}

/// The guns that turn on houses of their own, a weapon group each, in the unit file's
/// weapon order (`space.ron`): weapon `k` is `GROUPS[k]`, its mounts the guns of that kind
/// in `guns`' order. The seekers' spines are the next weapon; the Rondels are its
/// anti-missile lenses, no weapon.
pub(super) const GROUPS: [Kind; 8] = [
    Kind::Lance,
    Kind::VentralLance,
    Kind::Battery,
    Kind::Suppression,
    Kind::Keel,
    Kind::Repeater,
    Kind::Aa,
    Kind::AaLance,
];

impl Kind {
    /// How far above its mount's pivot a gun's house turns and pitches: an AA gun's
    /// trunnion on its pedestal; anything else turns about the mount.
    fn trunnion(self) -> f32 {
        match self {
            Kind::Aa => 4.5,
            Kind::AaLance => 6.5,
            _ => 0.0,
        }
    }

    /// How far its recoiling pieces kick back when it fires, metres.
    fn travel(self) -> f32 {
        match self {
            Kind::Battery => 0.8 * BATTERY,
            Kind::Lance | Kind::VentralLance => 4.0,
            Kind::Suppression | Kind::Keel => 2.4,
            Kind::Repeater | Kind::Aa => 1.2,
            Kind::AaLance => 2.0,
            Kind::Rondel => 0.0,
        }
    }

    /// A gun's mouths off its house's pivot as it faces the nose: where its shots leave
    /// (the unit file's `muzzles`), in firing order.
    #[cfg(test)]
    pub(super) fn muzzles(self) -> Vec<Vec3> {
        let barrels = |x: f32, y: f32, z: f32| vec![v3(x, y, z), v3(x, -y, z)];
        match self {
            Kind::Battery => super::super::destroyer::battery::gaps(BATTERY).to_vec(),
            Kind::Lance | Kind::VentralLance => vec![v3(LANCE_REACH, 0.0, 0.0)],
            Kind::Suppression => vec![v3(SUPPRESSION * 0.5 + SUPPRESSION * 2.6, 0.0, 0.0)],
            // Hung under the keel the right way up: its first barrel is on its left.
            Kind::Keel => barrels(KEEL * 0.5 + KEEL * 2.4, KEEL * 0.36, 0.0),
            Kind::Repeater => barrels(REPEATER * 4.3, REPEATER * 0.38, REPEATER * 0.05),
            Kind::Aa => barrels(AA * 3.5, AA * 0.32, 0.0),
            Kind::AaLance => vec![v3(AA_LANCE * 7.05, 0.0, 0.0)],
            Kind::Rondel => Vec::new(),
        }
    }

    /// Mounted on the centreline: one of each, not a pair.
    fn centre(self) -> bool {
        matches!(self, Kind::Lance | Kind::VentralLance)
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

/// `p` held to a quarter metre.
fn snap(p: Vec3) -> Vec3 {
    (p * 4.0).round() / 4.0
}

impl Mount {
    pub(super) fn new(kind: Kind, pivot: Vec3, yaw: f32) -> Self {
        Self {
            kind,
            pivot: snap(pivot),
            yaw,
        }
    }

    /// Facing out to port.
    pub(super) fn port(kind: Kind, pivot: Vec3) -> Self {
        Self::new(kind, pivot, FRAC_PI_2)
    }

    /// Facing forward.
    pub(super) fn ahead(kind: Kind, pivot: Vec3) -> Self {
        Self::new(kind, pivot, 0.0)
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

/// Every gun that turns on a house of its own, in gun-slot order (`GROUPS`): each group's
/// port guns in `port`'s order, then the same to starboard, or its centreline guns in
/// `centre`'s order. Gun `k` of the list is the unit's gun slot `k`.
pub(super) fn guns(port: &[Mount], centre: &[Mount]) -> Vec<Mount> {
    GROUPS
        .iter()
        .flat_map(|&kind| {
            let of = |list: &[Mount]| list.iter().filter(|m| m.kind == kind).copied().collect();
            let mut out: Vec<Mount> = if kind.centre() { of(centre) } else { of(port) };
            if !kind.centre() {
                let starboard: Vec<Mount> = out.iter().map(|m| m.mirrored()).collect();
                out.extend(starboard);
            }
            out
        })
        .collect()
}

/// A block of seeker cells: its middle, its deck's height, the cells along and across.
#[derive(Clone, Copy, Debug)]
pub(super) struct Spine {
    pub(super) at: Vec2,
    pub(super) deck: f32,
    pub(super) foot: f32,
    pub(super) along: usize,
    pub(super) across: usize,
}

impl Spine {
    /// The cells in the order they fire: along, then across.
    fn fire(&self) -> Vec<(u8, u8)> {
        (0..self.along as u8)
            .flat_map(|i| (0..self.across as u8).map(move |j| (i, j)))
            .collect()
    }

    /// The port spine's and then the starboard spine's seekers, each in firing order, as
    /// they stand in their cells (the unit file's `muzzles`).
    #[cfg(test)]
    pub(super) fn muzzles(&self) -> Vec<Vec3> {
        [1.0f32, -1.0]
            .iter()
            .flat_map(|&side| {
                let c = Vec2::new(self.at.x, self.at.y * side);
                self.fire().into_iter().map(move |(i, j)| {
                    let at = |k: u8, n: usize| (k as f32 - (n as f32 - 1.0) * 0.5) * CELL_PITCH;
                    (c + Vec2::new(at(i, self.along), at(j, self.across)))
                        .extend(self.deck - SEEKER_DROP)
                })
            })
            .collect()
    }
}

/// How far under its spine's deck a seeker's mouth stands in its cell: its lit nose.
const SEEKER_DROP: f32 = 6.0;

/// Each gun of `guns` (in gun-slot order) on a house of its own bound to its slot, turning
/// from where it rests. A starboard gun is drawn as its port twin, mirrored.
pub(super) fn arm(b: &mut MeshBuilder, guns: &[Mount]) {
    for (slot, m) in guns.iter().enumerate() {
        if m.pivot.y < 0.0 && !m.kind.centre() {
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
            Kind::Battery => b.with(Affine3A::from_scale(Vec3::splat(BATTERY)), battery::house),
            Kind::Lance => lance(b),
            Kind::VentralLance => b.with(Affine3A::from_rotation_x(PI), lance),
            Kind::Suppression => ball(b, SUPPRESSION, SUPPRESSION * 2.6, 1),
            Kind::Keel => b.with(Affine3A::from_rotation_x(PI), |b| {
                ball(b, KEEL, KEEL * 2.4, 2)
            }),
            Kind::Repeater => repeater(b),
            Kind::Aa => aa(b),
            Kind::AaLance => aa_lance(b),
            Kind::Rondel => rondel(b),
        })
    });
}

/// The Rondels: crush lenses on posts, fixed in the hull.
pub(super) fn rondels(b: &mut MeshBuilder, mounts: &[Mount]) {
    for m in mounts.iter().filter(|m| m.kind == Kind::Rondel) {
        b.at(m.pivot, rondel);
    }
}

/// Where a Rondel's lens (`rondel`) looks from, off its mount's pivot: the unit file's
/// `anti_missile_mounts`.
pub(super) const RONDEL_LENS: f32 = RONDEL * 1.4;

/// The heavy lance: a long pointed house, its walls raked in, a stepped roof, horn plates
/// swept back off its flanks, a lit brow either side of a long pinch barrel.
fn lance(b: &mut MeshBuilder) {
    let s = LANCE;
    let plan = [
        [s * 1.9, 0.0],
        [s * 1.0, s],
        [-s * 1.6, s * 0.95],
        [-s * 2.1, s * 0.5],
        [-s * 2.1, -s * 0.5],
        [-s * 1.6, -s * 0.95],
        [s * 1.0, -s],
    ];
    let (low, high) = (-s * 0.5, s * 0.55);
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
                Section::new(high - 0.1, 0.78),
                Section::new(high + s * 0.14, 0.64),
            ],
        )
    });
    if !b.coarse() {
        b.mirror_y(|b| {
            let f = Frame::new(
                v3(s * 0.9, s * 0.86, s * 0.05),
                v3(-1.0, 0.16, 0.08),
                v3(0.0, 1.0, 0.5),
            );
            shard(b, &f, (s * 3.6, s * 0.32, 0.6), s * 0.12, Finish::Dark);
            dark_plate(b);
            b.pattern(pattern::EMBER_SLOT);
            let k = 0.84;
            b.face(&[
                v3(s * 1.9 + 0.2, 0.0, low + s * 0.3),
                v3(s * 1.0 + 0.2, s, low + s * 0.3),
                v3(s * 1.0 * k + 0.2, s * k, high - 0.05),
                v3(s * 1.9 * k + 0.2, 0.0, high - 0.05),
            ]);
        });
    }
    b.with_recoil(|b| pinch_barrel(b, LANCE_REACH, s * 0.36));
}

/// A plated ball of radius `r`, a cowl plate swept back over it, `barrels` pinch snouts
/// reaching `reach` out of its face side by side.
fn ball(b: &mut MeshBuilder, r: f32, reach: f32, barrels: usize) {
    b.with_recoil(|b| {
        let sides = b.sides(8);
        dark_plate(b);
        b.with_facets(|b| b.spheroid(Vec3::ZERO, v3(r, r, r * 0.85), sides, 4));
        dark_plate(b);
        b.with_facets(|b| {
            b.cylinder_between(
                v3(r * 0.66, 0.0, 0.0),
                v3(r * 0.92, 0.0, 0.0),
                r * 0.66,
                r * 0.42,
                6,
            )
        });
        if !b.coarse() {
            let f = Frame::new(v3(r * 0.5, 0.0, r * 0.7), v3(-1.0, 0.0, -0.25), Vec3::Z);
            shard(b, &f, (r * 2.4, r * 0.55, 0.0), r * 0.12, Finish::Dark);
        }
        let gap = r * 0.36;
        for k in 0..barrels {
            let y = (k as f32 - (barrels as f32 - 1.0) * 0.5) * gap * 2.0;
            b.at(v3(r * 0.5, y, 0.0), |b| pinch_barrel(b, reach, r * 0.3));
        }
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
        root + Vec3::X * (len - 0.4),
        root + Vec3::X * len,
        r * 0.5,
        r * 0.5,
        sides,
    );
}

/// The twin repeater: a low pointed house, two ringed barrels out of its brow.
fn repeater(b: &mut MeshBuilder) {
    let s = REPEATER;
    let plan = [
        [s * 1.3, 0.0],
        [s * 0.7, s],
        [-s * 1.2, s * 0.9],
        [-s * 1.4, 0.0],
        [-s * 1.2, -s * 0.9],
        [s * 0.7, -s],
    ];
    dark_plate(b);
    b.with_facets(|b| {
        b.loft_z(
            &plan,
            &[
                Section::new(-s * 0.4, 1.0),
                Section::new(s * 0.3, 0.9),
                Section::new(s * 0.55, 0.7),
            ],
        )
    });
    b.with_recoil(|b| {
        for y in [s * 0.38, -s * 0.38] {
            ringed(b, v3(s * 0.9, y, s * 0.05), s * 3.4, s * 0.17);
        }
    });
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
        b.chamfered_box(v3(0.0, 0.0, 0.0), v3(s * 1.2, s * 1.1, s * 0.7), s * 0.15);
        for y in [s * 0.32, -s * 0.32] {
            ringed(b, v3(s * 0.5, y, 0.0), s * 3.0, s * 0.14);
        }
    });
}

/// The AA lance: a slim house, one long barrel on a trunnion at its roof, which is the
/// house's pivot (`Kind::trunnion`), coils down it lit; authored level, the gun's pitch
/// raises it.
fn aa_lance(b: &mut MeshBuilder) {
    let s = AA_LANCE;
    let foot = -Kind::AaLance.trunnion();
    dark_plate(b);
    b.with_facets(|b| {
        b.loft_z(
            &chamfered_rect(Vec2::new(s * 1.2, s * 0.8), s * 0.4),
            &[
                Section::new(foot - s * 0.5, 1.0),
                Section::new(foot + s * 0.5, 0.85),
            ],
        )
    });
    b.with_recoil(|b| {
        let sides = b.sides(6);
        metal(b);
        b.cylinder_between(
            v3(-s, 0.0, 0.0),
            v3(s * 7.0, 0.0, 0.0),
            s * 0.3,
            s * 0.22,
            sides,
        );
        if b.fine() {
            dark_plate(b);
            b.pattern(pattern::EMBER_RIM);
            for k in 0..4 {
                let x = s * (0.8 + 1.4 * k as f32);
                b.cylinder_between(
                    v3(x, 0.0, 0.0),
                    v3(x + s * 0.4, 0.0, 0.0),
                    s * 0.5,
                    s * 0.5,
                    sides,
                );
            }
        }
        b.paint(GLOW_VIOLET);
        b.cylinder_between(
            v3(s * 7.0, 0.0, 0.0),
            v3(s * 7.05, 0.0, 0.0),
            s * 0.16,
            s * 0.16,
            sides,
        );
    });
}

/// A Rondel: a gravity crush lens looking up on a short plated post.
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

/// The seeker spines, port then starboard: each an armoured block stood on the hull, its
/// sides run down into it, a lit coaming round its deck and a block of hatched cells on top
/// (`MeshBuilder::cell_block`), its pale lids hinged on their outboard edges, a seeker
/// standing under each. One weapon fires them all, port cells first.
pub(super) fn spines(b: &mut MeshBuilder, s: Spine) {
    let half = Vec2::new(
        CELL_PITCH * s.along as f32 * 0.5 + 6.0,
        CELL_PITCH * s.across as f32 * 0.5 + 6.0,
    );
    let plan = chamfered_rect(half, 9.0);
    for side in [1.0f32, -1.0] {
        let c = Vec2::new(s.at.x, s.at.y * side);
        b.at(c.extend(0.0), |b| {
            dark_plate(b);
            b.with_facets(|b| {
                b.loft_z(
                    &plan,
                    &[
                        Section::new(s.foot, 1.1),
                        Section::new(s.deck - 6.0, 1.05),
                        Section::new(s.deck - 1.5, 1.0),
                    ],
                )
            });
            seam(b);
            b.loft_z(
                &plan,
                &[Section::new(s.deck - 1.5, 1.0), Section::new(s.deck, 0.97)],
            );
            if !b.coarse() {
                dark_plate(b);
                b.pattern(pattern::EMBER_RIM);
                b.loft_z(
                    &plan,
                    &[
                        Section::new(s.deck - 4.6, 1.055),
                        Section::new(s.deck - 3.4, 1.055),
                    ],
                );
            }
        });
        if b.coarse() {
            continue;
        }
        let grid = CellGrid {
            centre: c,
            deck: s.deck,
            pitch: CELL_PITCH,
            half: CELL_HALF,
            nx: s.along as u8,
            ny: s.across as u8,
            hinge_y: true,
        };
        for m in b.cell_block(grid, &s.fire()) {
            cell(b, m, (m.y - c.y).signum(), s.deck);
        }
    }
}

/// One cell at `m` on a spine's `deck`: its lid, hinged on its edge `out` (+1 to port of
/// the cell's middle), and the seeker standing under it.
fn cell(b: &mut MeshBuilder, m: Vec2, out: f32, deck: f32) {
    b.with_part(part::CELL_HATCH, |b| {
        pale_plate(b);
        b.with_facets(|b| {
            b.loft_z(
                &chamfered_rect(Vec2::splat(CELL_HALF), 1.6),
                &[
                    Section::new(deck, 1.0).shifted(m.x, m.y),
                    Section::new(deck + 0.7, 0.9).shifted(m.x, m.y),
                ],
            )
        });
        if b.fine() {
            metal(b);
            let hinge = m.y + out * CELL_HALF;
            for dx in [-4.0, 4.0] {
                b.cylinder_between(
                    v3(m.x + dx - 1.5, hinge, deck + 0.35),
                    v3(m.x + dx + 1.5, hinge, deck + 0.35),
                    0.4,
                    0.4,
                    4,
                );
            }
        }
    });
    b.with_part(part::CELL_ROUND, |b| {
        // The seeker's nose comes up to its mouth (`SEEKER_DROP`), a little under the lid.
        let top = deck - 1.0;
        let nose = deck - SEEKER_DROP;
        let foot = if b.fine() { top - 30.0 } else { top - 10.0 };
        let sides = b.sides(6);
        metal(b);
        b.cylinder_between(v3(m.x, m.y, foot), v3(m.x, m.y, nose), 3.0, 3.0, sides);
        b.paint(GLOW_VIOLET);
        b.cylinder_between(v3(m.x, m.y, nose), v3(m.x, m.y, top), 3.0, 0.6, sides);
    });
}

/// An armoured well `r` across round a lance's `pivot`, `depth` deep, let into the deck
/// (`up`) or hung under the keel; its rim's gasket lit and a graphite race the house
/// turns on.
pub(super) fn well(b: &mut MeshBuilder, pivot: Vec3, r: f32, depth: f32, up: bool) {
    let sides = 8;
    let plan: Vec<[f32; 2]> = (0..sides)
        .map(|k| {
            let a = std::f32::consts::TAU * (k as f32 + 0.5) / sides as f32;
            [a.cos() * r, a.sin() * r]
        })
        .collect();
    let dir = if up { 1.0 } else { -1.0 };
    let rim = pivot.z - dir * LANCE * 0.2;
    let (lo, hi) = if up {
        (rim - depth, rim)
    } else {
        (rim, rim + depth)
    };
    b.at(v3(pivot.x, pivot.y, 0.0), |b| {
        dark_plate(b);
        b.with_facets(|b| b.loft_z(&plan, &[Section::new(lo, 1.0), Section::new(hi, 0.94)]));
        if !b.coarse() {
            dark_plate(b);
            b.pattern(pattern::EMBER_RIM);
            let z = if up { hi } else { lo - 0.6 };
            b.loft_z(&plan, &[Section::new(z, 0.95), Section::new(z + 0.6, 0.9)]);
        }
    });
    if b.fine() {
        metal(b);
        hoop_on(
            b,
            pivot - Vec3::Z * dir * LANCE * 0.15,
            Vec3::Z,
            r * 0.7,
            2.0,
            3.0,
            12,
        );
    }
}
