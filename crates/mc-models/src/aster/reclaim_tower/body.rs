//! The tower's shaft, from the podium to the head house, in the three bodies open to
//! the user (CLAUDE.md section 9):
//! - Keep: a plated square shaft tapering up, heavy columns at its cut corners, bands.
//! - Braced: four box-girder legs round a plated core, the lower third clad, braced
//!   above, a stair tower beside it.
//! - Stack: a round ribbed shaft with stiffener rings and a lift shaft tied to its side.
//!
//! Each carries the drop chute down its back (-x) into the plant, and the same plated
//! head house on top.

use glam::{Vec2, Vec3};

use super::super::parts::*;
use super::{chute, CAP, HOUSE, PODIUM};
use crate::builder::{chamfered_rect, MeshBuilder, Section};
use crate::material::*;

/// Where the stair or lift shaft beside the tower stands along x: behind the pods.
const STAIR_X: f32 = -3.6;

/// Half the head house's width.
pub(super) const HOUSE_HALF: f32 = 5.6;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Body {
    Keep,
    Braced,
    Stack,
}

impl Body {
    /// The shaft's half width (or radius) at the podium and under the head house.
    fn span(self) -> (f32, f32) {
        match self {
            Body::Keep => (6.2, 4.8),
            Body::Braced => (8.1, 5.3),
            Body::Stack => (5.8, 4.6),
        }
    }

    /// How far out from the axis the shaft's face is at height `z`: where works hung
    /// on the tower meet it.
    pub(super) fn half(self, z: f32) -> f32 {
        let (low, high) = self.span();
        let f = ((z - PODIUM) / (CAP - PODIUM)).clamp(0.0, 1.0);
        low + (high - low) * f
    }
}

/// The shaft and head house as two blocks.
pub(super) fn coarse(b: &mut MeshBuilder, body: Body) {
    let (low, high) = body.span();
    b.paint(PLATING);
    b.frustum_open(
        v3(0.0, 0.0, PODIUM),
        Vec2::splat(low * 2.0),
        Vec2::splat(high * 2.0),
        CAP - PODIUM,
        Vec2::ZERO,
    );
    b.cuboid_open(
        v3(0.0, 0.0, CAP + HOUSE * 0.5),
        v3(HOUSE_HALF * 2.0, HOUSE_HALF * 2.0, HOUSE),
    );
}

pub(super) fn body(b: &mut MeshBuilder, body: Body) {
    match body {
        Body::Keep => keep(b),
        Body::Braced => braced(b),
        Body::Stack => stack(b),
    }
    // The drop chute down the back, from the head's feed to the plant's hopper.
    let back = |z: f32| -body.half(z) - 1.1;
    chute(
        b,
        v3(back(CAP), 0.0, CAP + 0.6),
        v3(back(PODIUM + 6.0), 0.0, PODIUM + 6.0),
        0.8,
    );
    chute(
        b,
        v3(back(PODIUM + 6.0), 0.0, PODIUM + 6.0),
        v3(-10.6, 0.0, 8.6),
        0.8,
    );
    if b.fine() {
        // Brackets holding it to the shaft.
        b.paint(PLATING_DARK);
        for z in [11.0, 19.0] {
            b.beam(
                v3(-body.half(z) + 0.2, 0.0, z),
                v3(back(z) - 0.6, 0.0, z),
                Vec2::new(1.9, 0.5),
                Vec2::new(1.9, 0.5),
            );
        }
    }
    head_house(b, body);
}

/// Keep: a square plated shaft, cut corners carried by heavy columns, two bands.
fn keep(b: &mut MeshBuilder) {
    let (low, high) = Body::Keep.span();
    b.paint(PLATING);
    b.with_bevel(0.15, |b| {
        b.loft_z(
            &chamfered_rect(Vec2::splat(low), 1.8),
            &[
                Section::new(PODIUM, 1.0),
                Section::new(CAP + 0.2, high / low),
            ],
        );
    });
    // Corner columns, standing proud of the chamfers.
    let corner = |z: f32| Body::Keep.half(z) - 0.75;
    b.paint(PLATING_DARK);
    b.radial(4, |b| {
        b.beam(
            v3(corner(PODIUM), corner(PODIUM), PODIUM - 0.2),
            v3(corner(CAP), corner(CAP), CAP + 0.2),
            Vec2::splat(2.5),
            Vec2::splat(1.9),
        );
        if b.mid() {
            // A foot block where each column meets the podium.
            b.cuboid(
                v3(corner(PODIUM) + 0.2, corner(PODIUM) + 0.2, PODIUM + 0.6),
                Vec3::new(3.2, 3.2, 1.2),
            );
        }
    });
    // Bands round the shaft between the columns.
    b.paint(ACCENT);
    for z in [11.0, 19.5] {
        let h = Body::Keep.half(z);
        b.loft_z(
            &chamfered_rect(Vec2::splat(h + 0.25), 1.9),
            &[Section::new(z, 1.0), Section::new(z + 0.8, 1.0)],
        );
    }
    if b.fine() {
        faces(b, Body::Keep);
    }
}

/// Doors, louvres and plates on the shaft's faces.
fn faces(b: &mut MeshBuilder, body: Body) {
    let h = |z: f32| body.half(z);
    // A door at the foot of the front face, and a louvre bank on each flank.
    b.paint(ACCENT);
    b.block(
        v3(h(PODIUM + 3.0) - 0.1, -1.3, PODIUM),
        v3(h(PODIUM + 3.0) + 0.25, 1.3, PODIUM + 3.4),
    );
    b.paint(PLATING_DARK);
    b.block(
        v3(h(PODIUM + 4.0) - 0.1, -1.7, PODIUM + 3.4),
        v3(h(PODIUM + 4.0) + 0.4, 1.7, PODIUM + 3.9),
    );
    b.mirror_y(|b| {
        b.yawed(Vec3::ZERO, std::f32::consts::FRAC_PI_2, |b| {
            let z = 15.0;
            b.pitched(v3(h(z), 0.0, z), -std::f32::consts::FRAC_PI_2, |b| {
                vent(b, v3(0.0, 0.0, 0.0), Vec2::new(4.2, 2.4), 4, ACCENT);
            });
            b.paint(PLATING_DARK);
            b.block(
                v3(h(24.0) - 0.05, -1.6, 22.0),
                v3(h(24.0) + 0.12, 1.6, 25.6),
            );
        });
    });
}

/// Braced: four box-girder legs round a plated core, clad up to the first girt,
/// crossed braces above, a stair tower on the -y side tied in at each girt.
fn braced(b: &mut MeshBuilder) {
    let leg = |z: f32| Body::Braced.half(z) - 0.9;
    // The core the chute's feed runs in.
    b.paint(PLATING);
    b.prism(
        v3(0.0, 0.0, PODIUM),
        b.sides(8),
        3.6,
        3.0,
        CAP - PODIUM + 0.2,
    );
    b.paint(PLATING_DARK);
    b.radial(4, |b| {
        b.beam(
            v3(leg(PODIUM), leg(PODIUM), PODIUM - 0.2),
            v3(leg(CAP), leg(CAP), CAP + 0.2),
            Vec2::splat(2.6),
            Vec2::splat(1.9),
        );
    });
    // Cladding over the lower third.
    let clad = 12.0;
    b.paint(PLATING);
    b.frustum_open(
        v3(0.0, 0.0, PODIUM),
        Vec2::splat(leg(PODIUM) * 2.0 + 0.4),
        Vec2::splat(leg(clad) * 2.0 + 0.4),
        clad - PODIUM,
        Vec2::ZERO,
    );
    // Girts.
    b.paint(ACCENT);
    for z in [clad, 19.8] {
        let k = leg(z);
        b.radial(4, |b| {
            b.beam(
                v3(k + 0.3, -k, z),
                v3(k + 0.3, k, z),
                Vec2::new(1.1, 1.2),
                Vec2::new(1.1, 1.2),
            );
        });
    }
    // Braces in the two open bays.
    for (za, zb) in [(clad, 19.8), (19.8, CAP)] {
        let (ka, kb) = (leg(za), leg(zb));
        b.paint(METAL);
        b.radial(4, |b| {
            b.beam(
                v3(ka, -ka, za),
                v3(kb, kb, zb),
                Vec2::splat(0.55),
                Vec2::splat(0.55),
            );
            if b.fine() {
                b.beam(
                    v3(ka, ka, za),
                    v3(kb, -kb, zb),
                    Vec2::splat(0.55),
                    Vec2::splat(0.55),
                );
            }
        });
    }
    // The stair tower beside the -y face, tied in at the girts.
    let y = -(leg(PODIUM) + 2.6);
    b.paint(PLATING_DARK);
    b.cuboid(v3(STAIR_X, y, (0.4 + CAP) * 0.5), v3(2.8, 2.8, CAP - 0.4));
    b.paint(ACCENT);
    b.cuboid(v3(STAIR_X, y, CAP + 0.4), v3(3.2, 3.2, 0.8));
    for z in [clad, 19.8] {
        b.paint(PLATING_DARK);
        b.beam(
            v3(STAIR_X, y + 1.3, z),
            v3(STAIR_X, -leg(z) + 0.4, z),
            Vec2::new(1.6, 0.6),
            Vec2::new(1.6, 0.6),
        );
    }
    if b.fine() {
        // Windows up the stair tower.
        b.paint(ACCENT);
        for z in [8.0, 14.0, 20.0] {
            b.block(
                v3(STAIR_X - 0.8, y - 1.5, z),
                v3(STAIR_X + 0.8, y - 1.38, z + 1.6),
            );
        }
        faces(b, Body::Braced);
    }
}

/// Stack: a round shaft, ribbed and ringed, a lift shaft tied to its -y side.
fn stack(b: &mut MeshBuilder) {
    let (low, high) = Body::Stack.span();
    let sides = b.sides(16);
    b.paint(PLATING);
    b.prism(v3(0.0, 0.0, PODIUM), sides, low, high, CAP - PODIUM + 0.2);
    // Stiffener rings.
    b.paint(ACCENT);
    for z in [PODIUM + 0.4, 9.5, 15.5, 21.5] {
        let r = Body::Stack.half(z) + 0.3;
        b.prism(v3(0.0, 0.0, z), sides, r, r, 0.7);
    }
    // Ribs up the shaft.
    let ribs = if b.fine() { 8 } else { 4 };
    b.paint(PLATING_DARK);
    b.radial(ribs, |b| {
        b.beam(
            v3(low + 0.1, 0.0, PODIUM),
            v3(high + 0.1, 0.0, CAP),
            Vec2::new(0.8, 0.9),
            Vec2::new(0.7, 0.6),
        );
    });
    // The lift shaft, up to the head house's side.
    let y = -(low + 2.4);
    b.paint(PLATING_DARK);
    b.loft_z(
        &chamfered_rect(Vec2::splat(1.5), 0.4)
            .iter()
            .map(|p| [p[0] + STAIR_X, p[1] + y])
            .collect::<Vec<_>>(),
        &[
            Section::new(PODIUM, 1.0),
            Section::new(CAP + HOUSE - 0.6, 1.0),
        ],
    );
    b.paint(PLATING);
    for z in [9.5, 21.5] {
        b.beam(
            v3(STAIR_X, y + 1.3, z + 0.3),
            v3(STAIR_X, -Body::Stack.half(z) + 0.3, z + 0.3),
            Vec2::new(1.4, 0.8),
            Vec2::new(1.4, 0.8),
        );
    }
    if b.fine() {
        b.paint(ACCENT);
        b.block(
            v3(STAIR_X - 1.2, y - 1.6, PODIUM),
            v3(STAIR_X + 1.2, y - 1.45, PODIUM + 3.0),
        );
        faces(b, Body::Stack);
    }
}

/// The plated head house on the shaft: the machinery deck the head turns on, a skirt
/// down to the shaft, a band round its top and a rail round its roof.
fn head_house(b: &mut MeshBuilder, body: Body) {
    let half = HOUSE_HALF;
    b.paint(PLATING);
    b.with_bevel(0.15, |b| {
        b.chamfered_box(
            v3(0.0, 0.0, CAP + HOUSE * 0.5),
            v3(half * 2.0, half * 2.0, HOUSE),
            1.2,
        );
    });
    b.paint(PLATING_DARK);
    let under = body.half(CAP - 1.6).min(half - 0.6);
    b.frustum(
        v3(0.0, 0.0, CAP - 1.6),
        Vec2::splat(under * 2.0),
        Vec2::splat(half * 2.0 - 0.4),
        1.6,
        Vec2::ZERO,
    );
    if b.mid() {
        b.paint(ACCENT);
        b.chamfered_box(
            v3(0.0, 0.0, CAP + HOUSE - 0.4),
            v3(half * 2.0 + 0.2, half * 2.0 + 0.2, 0.55),
            1.3,
        );
    }
    if b.fine() {
        b.radial(4, |b| {
            b.paint(ACCENT);
            b.block(
                v3(half - 0.02, -1.0, CAP + 0.35),
                v3(half + 0.1, 1.0, CAP + 2.4),
            );
        });
        b.paint(METAL);
        rail_square(b, CAP + HOUSE, half - 0.25, 1.0);
    }
}

/// A square guard rail at height `z` round a deck of half width `half`.
pub(super) fn rail_square(b: &mut MeshBuilder, z: f32, half: f32, height: f32) {
    b.radial(4, |b| {
        b.beam(
            v3(half, -half, z + height),
            v3(half, half, z + height),
            Vec2::splat(0.12),
            Vec2::splat(0.12),
        );
        for f in [-0.66, 0.0, 0.66] {
            b.beam(
                v3(half, half * f, z),
                v3(half, half * f, z + height),
                Vec2::splat(0.1),
                Vec2::splat(0.1),
            );
        }
    });
}
