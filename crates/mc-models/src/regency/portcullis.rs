//! The Portcullis (`regency_t3_warp_damper`, a 4 x 4 lot, radius 18, height 46): the
//! Regency's warp dampener, the Undertow's match. Its field projector, a red lens, sits
//! over the middle at [`PROJECTOR`] (0.95 of the height), where the renderer lights it and
//! throws its tether from (`renderer/damper_fx.rs`); bronze rings turn round the core and
//! the lens (`part::SPINNER`), still when the grid stalls, as the field is.
//!
//! Four plated talons rise from sockets at the plinth's corners and curl in over a plated
//! core, their tips closed round the lens like a fist and tied by a bronze crown ring
//! (picked by the user over a gate of two pylons and a ringed mast, 2026-10-15).
//!
//! - The plinth: two plated octagonal steps, a lit seam between them, the owner's colour
//!   on four tabs; a plated socket at each corner where a talon roots.
//! - A talon: three swept plates, red gaskets inside their rims (`pattern::EMBER_RIM`),
//!   a lit ridge up each, bronze knuckle drums at the joints over a bronze spine, red
//!   optic slots on the inner faces (`pattern::EMBER_SLOT`). Plated arms reach to its
//!   knuckles from collars on the core.
//! - The core: a plated drum with plates lapped down it and red slots let into it, a
//!   bronze column rising out of it to the lens's cup, a toothed ring turning round it.

use std::f32::consts::{FRAC_PI_4, TAU};

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::material::*;
use crate::{part, pattern};

use super::kit::{cable, dark_plate, knuckle, metal, seam, v3};
use super::machine::*;

/// The field projector's centre (0.95 of the height, `damper_fx::PROJECTOR`).
const PROJECTOR: Vec3 = Vec3::new(0.0, 0.0, 43.7);
/// The plinth: its foot's radius and its top.
const PLINTH_R: f32 = 19.0;
const PLINTH_TOP: f32 = 3.6;
/// A talon's line in its own plane (out along +x, up z): the foot in its socket, the two
/// knuckles, and the tip by the lens.
const TALON: [(f32, f32); 4] = [(15.4, 2.0), (14.4, 17.0), (10.2, 31.0), (3.6, 45.4)];
/// Each of its plates' half width.
const WIDTH: [f32; 3] = [2.8, 2.3, 1.6];
/// The core drum: foot and top radius, and its top; the column's radius.
const DRUM: (f32, f32, f32) = (6.6, 5.0, 16.0);
const COLUMN: f32 = 1.6;
/// The crown ring tying the talons' tips: its height and radius.
const CROWN: (f32, f32) = (40.6, 5.4);
/// The toothed ring turning round the core: its height and radius.
const RING: (f32, f32) = (24.0, 6.6);

/// The direction at bearing `a` (radians) round z.
fn out(a: f32) -> Vec3 {
    v3(a.cos(), a.sin(), 0.0)
}

/// A point on the talon's line, out along +x.
fn talon_at(k: usize) -> Vec3 {
    v3(TALON[k].0, 0.0, TALON[k].1)
}

/// The owner's colour on a flat top: a square `half` across at `at`.
fn tab(b: &mut MeshBuilder, at: Vec3, along: Vec3, half: f32) {
    let along = along.normalize() * half;
    let across = Vec3::Z.cross(along);
    b.paint(TEAM);
    b.face(&[
        at - along - across,
        at + along - across,
        at + along + across,
        at - along + across,
    ]);
}

/// A flat plate seen from both sides, far off.
fn card(b: &mut MeshBuilder, quad: [Vec3; 4]) {
    b.face(&quad);
    b.face(&[quad[3], quad[2], quad[1], quad[0]]);
}

/// A rectangular plate `len` along `f.u` and `half` either side, its face patterned.
fn panel(b: &mut MeshBuilder, f: &Frame, len: f32, half: f32, thick: f32, look: u32) {
    b.paint(PLATING_DARK).pattern(look);
    armour(
        b,
        f,
        &[[0.0, -half], [len, -half], [len, half], [0.0, half]],
        thick,
    );
}

pub(super) fn portcullis(b: &mut MeshBuilder, _tech: u8) {
    b.set_spinner_pivot(PROJECTOR);
    plinth(b);
    if b.coarse() {
        coarse(b);
        return;
    }
    b.with_facets(|b| {
        core(b);
        for k in 0..4 {
            b.yawed(Vec3::ZERO, FRAC_PI_4 + k as f32 * TAU / 4.0, talon);
        }
        crown(b);
    });
    lens(b);
    halo(b);
}

/// Far off: the plinth, the core as a cone, each talon as two plates, the lens.
fn coarse(b: &mut MeshBuilder) {
    dark_plate(b);
    b.prism(
        Vec3::Z * PLINTH_TOP,
        4,
        DRUM.0,
        COLUMN,
        CROWN.0 - PLINTH_TOP,
    );
    for k in 0..4 {
        let d = out(FRAC_PI_4 + k as f32 * TAU / 4.0);
        let side = Vec3::Z.cross(d);
        let at = |i: usize| d * TALON[i].0 + Vec3::Z * TALON[i].1;
        card(
            b,
            [
                at(0) - side * WIDTH[0],
                at(0) + side * WIDTH[0],
                at(2) + side * WIDTH[2],
                at(2) - side * WIDTH[2],
            ],
        );
        b.face(&[at(2) - side * WIDTH[2], at(2) + side * WIDTH[2], at(3)]);
    }
    lens(b);
}

/// The plinth: two plated octagonal steps with a lit seam between them, the owner's
/// colour on four tabs. Far off, one drum and two tabs.
fn plinth(b: &mut MeshBuilder) {
    dark_plate(b);
    if b.coarse() {
        b.prism(Vec3::ZERO, 4, PLINTH_R, PLINTH_R - 4.0, PLINTH_TOP);
        for k in 0..2 {
            let d = out(k as f32 * TAU / 2.0);
            tab(b, d * 9.0 + Vec3::Z * (PLINTH_TOP + 0.02), d, 2.0);
        }
        return;
    }
    b.prism(Vec3::ZERO, 8, PLINTH_R, PLINTH_R - 1.2, 1.6);
    seam(b);
    b.prism(Vec3::Z * 1.6, 8, PLINTH_R - 2.2, PLINTH_R - 2.2, 0.3);
    dark_plate(b);
    b.prism(
        Vec3::Z * 1.9,
        8,
        PLINTH_R - 2.6,
        PLINTH_R - 4.4,
        PLINTH_TOP - 1.9,
    );
    for k in 0..4 {
        let d = out(k as f32 * TAU / 4.0);
        tab(b, d * (PLINTH_R - 1.6) + Vec3::Z * 1.62, d, 0.9);
    }
}

/// The core: the plated drum with plates lapped down it and red slots between them, a
/// lit seam, the bronze column out of it with its collars, and the toothed ring turning
/// round it on studs.
fn core(b: &mut MeshBuilder) {
    let fine = b.fine();
    let (r0, r1, top) = DRUM;
    dark_plate(b);
    b.prism(Vec3::Z * PLINTH_TOP, 8, r0, r1, top - PLINTH_TOP);
    seam(b);
    b.prism(Vec3::Z * (top - 0.2), 8, r1 + 0.15, r1 + 0.1, 0.4);
    dark_plate(b);
    b.prism(Vec3::Z * (top + 0.2), 8, r1, 2.6, 3.6);
    // The drum's faces: a red slot on each one between the talons, plates lapped down the
    // ones under them.
    let apothem = (TAU / 16.0).cos();
    let slope = (r0 - r1) / (top - PLINTH_TOP) * apothem;
    for k in 0..8 {
        let d = out((k as f32 + 0.5) * TAU / 8.0);
        let face = |z: f32| {
            d * ((r0 - (z - PLINTH_TOP) / (top - PLINTH_TOP) * (r0 - r1)) * apothem) + Vec3::Z * z
        };
        if k % 2 == 0 {
            let f = Frame::new(face(5.0) + d * 0.02, Vec3::Z - d * slope, d);
            panel(b, &f, 8.6, 1.3, 0.3, pattern::EMBER_SLOT);
        } else {
            let f = Frame::new(face(top - 0.6) + d * 0.05, d * slope - Vec3::Z, d);
            dark_plate(b);
            Course {
                count: if fine { 3 } else { 2 },
                step: 3.4,
                len: 4.2,
                half: 1.7,
                tip: 0.0,
                thick: 0.4,
                tail: 1.2,
            }
            .lay(b, &f);
        }
    }
    shaft(b, Vec3::Z * (top + 3.6), PROJECTOR - Vec3::Z * 1.6, COLUMN);
    for z in [RING.0, 31.0, 37.0] {
        collar(b, Vec3::Z * z, Vec3::Z, COLUMN + 0.7, 1.4);
    }
    seam(b);
    b.prism(Vec3::Z * 33.5, 8, COLUMN + 0.35, COLUMN + 0.35, 1.6);
    let segs = if fine { 28 } else { 12 };
    b.with_part(part::SPINNER, |b| {
        metal(b);
        hoop(b, Vec3::Z * RING.0, RING.1, 1.0, 0.9, segs);
        if fine {
            teeth(b, Vec3::Z * RING.0, RING.1 + 0.5, 16, v3(0.6, 0.5, 0.7));
        }
        for k in 0..4 {
            let d = out(k as f32 * TAU / 4.0);
            dark_plate(b);
            b.beam(
                d * (COLUMN + 1.3) + Vec3::Z * RING.0,
                d * (RING.1 - 0.4) + Vec3::Z * RING.0,
                Vec2::new(1.0, 0.8),
                Vec2::new(0.8, 0.6),
            );
        }
    });
}

/// One talon, out along +x (yawed into place by the caller): its socket, three swept
/// plates over a bronze spine with knuckle drums at the joints, red slots on their inner
/// faces, and the plated arms from the core to its knuckles.
fn talon(b: &mut MeshBuilder) {
    let fine = b.fine();
    let pts: [Vec3; 4] = std::array::from_fn(talon_at);
    // The socket it roots in: a plated block on the plinth's corner, a lit seam round it.
    dark_plate(b);
    b.chamfered_box(v3(15.0, 0.0, PLINTH_TOP + 1.0), v3(6.4, 6.8, 2.6), 0.8);
    seam(b);
    b.block(
        v3(12.0, -3.2, PLINTH_TOP + 2.25),
        v3(18.0, 3.2, PLINTH_TOP + 2.45),
    );
    for k in 0..3 {
        let (a, c) = (pts[k], pts[k + 1]);
        let len = (c - a).length();
        let f = Frame::new(a, c - a, v3(1.0, 0.0, 0.3));
        // The spine under the plate, from knuckle to knuckle.
        shaft(b, f.at(0.0, 0.0, -1.1), f.at(len, 0.0, -1.1), 0.55);
        let shoulder = if k == 2 { 0.4 } else { 0.85 };
        b.paint(PLATING_DARK).pattern(pattern::EMBER_RIM);
        armour(b, &f, &swept(len + 1.2, WIDTH[k], 0.0, shoulder), 0.8);
        // Its ridge, lit.
        seam(b);
        b.beam(
            f.at(1.0, 0.0, 0.82),
            f.at(len * 0.85, 0.0, 0.82),
            Vec2::new(0.45, 0.14),
            Vec2::new(0.25, 0.14),
        );
        // A red optic slot on its inner face.
        if k < 2 || fine {
            let g = Frame::new(f.at(len * 0.2, 0.0, -0.05), f.u, -f.n);
            panel(b, &g, len * 0.5, WIDTH[k] * 0.45, 0.25, pattern::EMBER_SLOT);
        }
        // A second, smaller plate lapped over its outer face, close up.
        if fine && k < 2 {
            let g = Frame::new(f.at(len * 0.35, 0.0, 0.8), f.u, f.n);
            b.paint(PLATING_DARK).pattern(pattern::EMBER_RIM);
            armour(b, &g, &swept(len * 0.45, WIDTH[k] * 0.6, 0.0, 0.7), 0.4);
        }
    }
    for &k in &pts[1..3] {
        knuckle(b, k - v3(0.9, 0.0, 0.0), Vec3::Y, 1.5, WIDTH[1] * 1.5);
    }
    // The arms from the core: a plated strut from a collar on the drum to the lower
    // knuckle, and from the column's collar to the upper one, each with bronze ends.
    let low = (v3(DRUM.1 + 0.4, 0.0, 14.0), pts[1] - v3(1.6, 0.0, 0.0));
    let high = (v3(COLUMN + 0.8, 0.0, 31.0), pts[2] - v3(1.6, 0.0, 0.0));
    for (a, c, r) in [(low.0, low.1, 0.75), (high.0, high.1, 0.55)] {
        strut(b, a, c, r);
        collar(b, a, c - a, r * 1.5, 0.9);
        collar(b, c, c - a, r * 1.5, 0.9);
    }
    if fine {
        // Cables from the socket up to the lower knuckle, and power run along the arm.
        metal(b);
        cable(
            b,
            &[
                v3(12.6, 1.6, PLINTH_TOP + 2.2),
                v3(12.4, 1.8, 9.0),
                pts[1] + v3(-1.8, 1.4, -1.6),
            ],
            0.3,
        );
        cable(
            b,
            &[
                v3(12.6, -1.6, PLINTH_TOP + 2.2),
                v3(12.4, -1.8, 9.0),
                pts[1] + v3(-1.8, -1.4, -1.6),
            ],
            0.3,
        );
    }
}

/// The crown: a bronze ring through the talons' tips, a lit band inside it, bronze arms
/// from it in to the lens's cup.
fn crown(b: &mut MeshBuilder) {
    let fine = b.fine();
    let (z, r) = CROWN;
    metal(b);
    hoop(b, Vec3::Z * z, r, 0.9, 1.0, if fine { 28 } else { 12 });
    seam(b);
    hoop(
        b,
        Vec3::Z * z,
        r - 0.55,
        0.2,
        0.6,
        if fine { 28 } else { 12 },
    );
    for k in 0..4 {
        let d = out(k as f32 * TAU / 4.0);
        dark_plate(b);
        b.beam(
            d * (r - 0.4) + Vec3::Z * z,
            d * 1.6 + Vec3::Z * (PROJECTOR.z - 1.4),
            Vec2::new(0.9, 0.7),
            Vec2::new(0.7, 0.5),
        );
    }
}

/// The lens at the projector, in a bronze cup on the column. Far off, the lens alone.
fn lens(b: &mut MeshBuilder) {
    let sides = b.sides(10);
    if !b.coarse() {
        metal(b);
        b.cylinder_between(
            PROJECTOR - Vec3::Z * 1.8,
            PROJECTOR - Vec3::Z * 0.6,
            1.3,
            1.8,
            sides,
        );
    }
    b.paint(GLOW_LASER);
    let rings = if b.fine() { 4 } else { 2 };
    b.spheroid(PROJECTOR, v3(1.3, 1.3, 1.5), sides, rings);
}

/// The toothed bronze ring turning round the lens, inside the talons' tips.
fn halo(b: &mut MeshBuilder) {
    if b.coarse() {
        return;
    }
    let fine = b.fine();
    b.with_part(part::SPINNER, |b| {
        metal(b);
        hoop(b, PROJECTOR, 2.7, 0.5, 0.5, if fine { 24 } else { 10 });
        if fine {
            teeth(b, PROJECTOR, 2.95, 12, v3(0.4, 0.35, 0.4));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portcullis_fits_its_lot() {
        super::super::check("regency_portcullis", 18.0, 46.0, Some(4), &[]);
    }

    /// The lens is where the renderer puts the projector (`damper_fx::PROJECTOR`, 0.95 of
    /// the height), and the rings turn round it.
    #[test]
    fn the_lens_is_the_projector() {
        assert!((PROJECTOR.z - 46.0 * 0.95).abs() < 0.01);
        let model = crate::build_model_scaled("regency_portcullis", 18.0, 46.0, 3).unwrap();
        let near = model.lods[0]
            .vertices
            .iter()
            .filter(|v| v.material == GLOW_LASER)
            .map(|v| Vec3::from(v.pos).distance(PROJECTOR))
            .fold(f32::MAX, f32::min);
        assert!(near < 1.6, "lens {near} m from the projector");
        assert!(
            model.lods[0]
                .vertices
                .iter()
                .any(|v| v.part == part::SPINNER),
            "nothing turns"
        );
    }
}
