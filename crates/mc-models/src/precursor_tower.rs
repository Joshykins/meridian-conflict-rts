//! The Precursor tower (`mc_map::PropKind::PrecursorTower`), Forerunner style: a
//! sheer spine and a slab leaning against it, the sky open between them, braced
//! across both flanks, its top cut on a slant, one burning seam and a beam of light
//! out of the top. The map levels a bench at its origin and spans meet it at
//! `DECK_Z` (84 m), on the stepped foot.

use std::f32::consts::FRAC_PI_4;

use glam::{Affine3A, Vec2, Vec3};

use super::builder::{MeshBuilder, Section};
use super::library::ModelDef;
use super::precursor::{
    cut_rect, dark, fine_rect, key_light, key_seam, light, pale, panel, seam, v3,
};

/// Where the beam out of the top ends.
const BEAM_TOP: f32 = 1_400.0;

pub(super) const MODELS: &[ModelDef] = &[ModelDef::new("precursor_tower", 110.0, 930.0, tower)];

/// A side profile (x, z) extruded across y from `y0` to `y1`, its flanks chamfered.
fn slab(b: &mut MeshBuilder, profile: &[[f32; 2]], y0: f32, y1: f32, chamfer: f32) {
    b.at(v3(0.0, (y0 + y1) * 0.5, 0.0), |b| {
        b.extrude_y_chamfered(profile, (y1 - y0) * 0.5, chamfer);
    });
}

/// The beam out of the top: a thin column of live light from `at` to `BEAM_TOP`.
fn beam(b: &mut MeshBuilder, at: Vec3) {
    key_light(b);
    let core = fine_rect(b, 2.4, 2.4, 0.6);
    b.loft_z(
        &core,
        &[
            Section::new(at.z, 1.0).shifted(at.x, at.y),
            Section::new(BEAM_TOP, 0.5).shifted(at.x, at.y),
        ],
    );
}

/// Dark recessed panels on the flat flank at `y` (facing `side`), in tiers between
/// `back` and the front edge `front(z)`.
fn flank_panels(
    b: &mut MeshBuilder,
    y: f32,
    side: f32,
    back: f32,
    tiers: &[(f32, f32)],
    front: impl Fn(f32) -> f32,
) {
    if b.coarse() {
        return;
    }
    dark(b);
    let out = v3(0.0, side, 0.0);
    for &(z0, z1) in tiers {
        let quad = [
            v3(back, y, z0),
            v3(front(z0), y, z0),
            v3(front(z1), y, z1),
            v3(back, y, z1),
        ];
        panel(b, &quad, out, 0.8, 1.5);
    }
}

// ---- Tower -----------------------------------------------------------------------
//
// A sheer spine at the back and a slab leaning against it from the front, the sky
// open between them; struts across the gap, diagonal braces over both flanks, a
// stepped foot on splayed legs. The spine's top is cut on a slant, a fin behind it,
// and the beam leaves the cut.

fn tower(b: &mut MeshBuilder, _tech: u8) {
    // The stepped foot: a footing to -80 m with a lit kerb, then two steps to the
    // spans' deck. At the coarse level, one block.
    pale(b);
    let foot = cut_rect(b, 78.0, 78.0, 18.0);
    if b.coarse() {
        b.loft_z(&foot, &[Section::new(-80.0, 1.0), Section::new(92.0, 0.7)]);
    } else {
        b.loft_z(
            &foot,
            &[
                Section::new(-80.0, 1.0),
                Section::new(0.0, 1.0),
                Section::new(24.0, 0.97),
            ],
        );
        for (half, z0, z1) in [(66.0, 23.0, 50.0), (54.0, 49.0, 92.0)] {
            pale(b);
            let step = fine_rect(b, half, half, half * 0.22);
            b.loft_z(&step, &[Section::new(z0, 1.0), Section::new(z1, 0.92)]);
        }
        if b.fine() {
            light(b);
            b.loft_z(
                &foot,
                &[Section::new(12.0, 0.995), Section::new(15.0, 0.99)],
            );
        }
    }
    // Splayed legs out of the corners.
    if b.mid() {
        b.radial(4, |b| {
            b.with(Affine3A::from_rotation_z(FRAC_PI_4), |b| {
                pale(b);
                b.beam(
                    v3(118.0, 0.0, -20.0),
                    v3(66.0, 0.0, 120.0),
                    Vec2::new(16.0, 22.0),
                    Vec2::new(12.0, 16.0),
                );
            });
        });
    }
    // The spine.
    pale(b);
    let spine = [
        [-50.0, 90.0],
        [-12.0, 90.0],
        [-12.0, 862.0],
        [-30.0, 918.0],
        [-50.0, 900.0],
    ];
    slab(b, &spine, -26.0, 26.0, 2.0);
    key_seam(
        b,
        v3(-11.8, 0.0, 100.0),
        v3(-11.8, 0.0, 850.0),
        Vec3::X,
        4.0,
    );
    // The leaning slab, meeting the spine at 660 m.
    pale(b);
    let lean = [[10.0, 90.0], [56.0, 90.0], [4.0, 680.0], [-12.0, 660.0]];
    slab(b, &lean, -20.0, 20.0, 1.6);
    beam(b, v3(-30.0, 0.0, 915.0));
    if b.coarse() {
        return;
    }
    if b.fine() {
        dress(b);
    }
    // Struts across the gap.
    dark(b);
    for z in [230.0f32, 420.0] {
        let x = 56.0 - (z - 90.0) * 52.0 / 590.0 - 20.0;
        b.beam(
            v3(-12.0, 0.0, z),
            v3(x, 0.0, z - 10.0),
            Vec2::new(26.0, 14.0),
            Vec2::new(26.0, 14.0),
        );
    }
    // Diagonal braces over both flanks, from the leaning slab's foot up the spine.
    pale(b);
    for side in [-1.0f32, 1.0] {
        b.beam(
            v3(46.0, side * 25.0, 120.0),
            v3(-40.0, side * 29.0, 620.0),
            Vec2::new(8.0, 20.0),
            Vec2::new(8.0, 16.0),
        );
    }
}

/// Full detail only: the leaning slab's dormant channel, the fin behind the spine's
/// top, dark panels on the spine's flanks.
fn dress(b: &mut MeshBuilder) {
    // A dormant channel down the leaning slab's face.
    let (a, c) = (v3(56.0, 0.0, 100.0), v3(4.0, 0.0, 670.0));
    let run = c - a;
    let out = v3(run.z, 0.0, -run.x).normalize();
    seam(b, a + out * 0.1, c + out * 0.1, out, 3.0);
    // A fin behind the spine, up top.
    pale(b);
    let fin = [
        [-66.0, 560.0],
        [-50.0, 540.0],
        [-50.0, 880.0],
        [-58.0, 890.0],
    ];
    slab(b, &fin, -3.0, 3.0, 0.8);
    // Blocks stepping down the spine's back to the foot.
    for i in 0..4 {
        let (depth, half, z0) = (
            36.0 - 8.0 * i as f32,
            22.0 - 3.0 * i as f32,
            92.0 + 70.0 * i as f32,
        );
        b.chamfered_box(
            v3(-50.0 - depth * 0.5 + 1.0, 0.0, z0 + 35.0),
            v3(depth, half * 2.0, 70.0),
            3.0,
        );
    }
    // Pale bands across the spine's back face above them.
    let back = v3(-1.0, 0.0, 0.0);
    let mut z = 420.0;
    while z < 860.0 {
        let quad = [
            v3(-50.0, 22.0, z),
            v3(-50.0, -22.0, z),
            v3(-50.0, -22.0, z + 8.0),
            v3(-50.0, 22.0, z + 8.0),
        ];
        panel(b, &quad, back, 1.4, 0.5);
        z += 110.0;
    }
    for side in [-1.0f32, 1.0] {
        flank_panels(
            b,
            side * 26.05,
            side,
            -46.0,
            &[(130.0, 300.0), (680.0, 830.0)],
            |_| -16.0,
        );
    }
}
