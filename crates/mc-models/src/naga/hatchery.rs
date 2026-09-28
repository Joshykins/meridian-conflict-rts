//! The Skyforge, the Naga air factory, on its 8 x 8 lot (96 m square): a launch frame.
//!
//! The aircraft is made on the pad at the lot origin and lifts off it; the lane toward +x
//! is left open. From above the frame is an X of four swept towers round a ring.
//!
//! - The pad: a low plated deck, a bronze cradle ring on it, clamp rams round it.
//! - Four towers stand on the diagonals, each leaning in over the pad from a footing near
//!   the lot's corner: a pair of ribbed bronze legs laddered together, the back of it
//!   armoured with plates lapped down toward the ground and out into spikes, a brace ram
//!   under it, a press ram working at its foot (`part::PUMP`).
//! - Their heads carry a fixed plated race high over the pad, a toothed bronze lift ring
//!   turning inside it (`part::SPINNER`), and hang the violet fabricator heads aimed at
//!   the work (their mounts are `mc_sim::print_heads`, where the nanite streams pour
//!   from). The violet runs hot while the frame builds.
//! - The owner's colour is on each tower's head and footing.

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::material::*;
use crate::part;

use super::kit::{dark_plate, metal, seam, v3};
use super::machine::*;

/// The towers: which way each stands from the middle, and how far out its footing and
/// its head are.
const TOWERS: [f32; 4] = [45.0, 135.0, 225.0, 315.0];
const FOOT: f32 = 34.0;
const HEAD: f32 = 15.0;
const HEAD_Z: f32 = 26.0;
/// The machine houses between the towers, either side and behind; none in front.
const HOUSES: [f32; 3] = [90.0, 180.0, 270.0];
/// The race and the lift ring turning in it (the spinner's pivot).
const RING: Vec3 = Vec3::new(0.0, 0.0, 26.8);
const RACE_R: f32 = 13.6;
const RING_R: f32 = 11.6;
/// The pad.
const PAD_R: f32 = 14.0;
/// Tech 3's crown: a race raised over the lift ring on masts off the towers' heads.
const CROWN: Vec3 = Vec3::new(0.0, 0.0, 36.6);
const CROWN_R: f32 = 15.5;
/// The top of a tower's head, where a crown mast stands.
const HEAD_TOP: f32 = HEAD_Z + 2.4;
const THICK: f32 = 0.9;

pub(super) fn hatchery(b: &mut MeshBuilder, tech: u8) {
    b.set_spinner_pivot(RING);
    if b.coarse() {
        coarse(b, tech >= 3);
        return;
    }
    pad(b);
    for deg in TOWERS {
        b.yawed(Vec3::ZERO, deg.to_radians(), tower);
    }
    for deg in HOUSES {
        b.yawed(Vec3::ZERO, deg.to_radians(), house);
    }
    ring(b);
    tier(b, tech, 2, 0.2, |b| {
        for deg in HOUSES {
            b.yawed(Vec3::ZERO, deg.to_radians(), flywheels);
        }
        masts(b);
    });
    tier(b, tech, 3, 0.25, |b| {
        for deg in TOWERS {
            b.yawed(Vec3::ZERO, deg.to_radians(), tower_blades);
        }
        crown(b);
    });
}

/// Far off: the four towers as leaning wedges (running on up to the crown once it is
/// raised), the houses' roofs, the pad, the owner's colour on the heads.
fn coarse(b: &mut MeshBuilder, crowned: bool) {
    let top_z = if crowned { CROWN.z + 0.5 } else { HEAD_Z + 2.5 };
    for deg in TOWERS {
        b.yawed(Vec3::ZERO, deg.to_radians(), |b| {
            dark_plate(b);
            let base = [
                v3(FOOT + 6.0, -4.0, 0.0),
                v3(FOOT + 6.0, 4.0, 0.0),
                v3(FOOT - 7.0, 4.0, 0.0),
                v3(FOOT - 7.0, -4.0, 0.0),
            ];
            let top = [
                v3(HEAD + 3.0, -2.2, top_z),
                v3(HEAD + 3.0, 2.2, top_z),
                v3(HEAD - 2.0, 2.2, top_z),
                v3(HEAD - 2.0, -2.2, top_z),
            ];
            b.loft(&[base.to_vec(), top.to_vec()], false, true);
            b.paint(TEAM);
            b.face(&[
                v3(HEAD - 1.5, -1.8, top_z + 0.1),
                v3(HEAD + 2.5, -1.8, top_z + 0.1),
                v3(HEAD + 2.5, 1.8, top_z + 0.1),
                v3(HEAD - 1.5, 1.8, top_z + 0.1),
            ]);
        });
    }
    for deg in HOUSES {
        b.yawed(Vec3::ZERO, deg.to_radians(), |b| {
            dark_plate(b);
            b.face(&[
                v3(26.0, -7.0, 6.5),
                v3(40.0, -7.0, 6.5),
                v3(40.0, 7.0, 6.5),
                v3(26.0, 7.0, 6.5),
            ]);
        });
    }
    seam(b);
    b.face(&[
        v3(PAD_R, 0.0, 0.8),
        v3(0.0, PAD_R, 0.8),
        v3(-PAD_R, 0.0, 0.8),
        v3(0.0, -PAD_R, 0.8),
    ]);
}

/// The pad the aircraft is made on: a plated deck, the bronze cradle ring and clamp rams.
fn pad(b: &mut MeshBuilder) {
    let fine = b.fine();
    let sides = b.sides(16);
    seam(b);
    b.prism(Vec3::ZERO, sides, PAD_R + 1.0, PAD_R, 0.5);
    dark_plate(b);
    b.prism(Vec3::Z * 0.5, sides, PAD_R - 0.6, PAD_R - 1.4, 0.35);
    metal(b);
    hoop(
        b,
        Vec3::Z * 0.95,
        PAD_R - 3.5,
        1.2,
        0.5,
        if fine { 32 } else { 16 },
    );
    // Clamp rams round the cradle, lying toward the middle, on the diagonals between the
    // towers and fore and aft of the lane.
    for k in 0..8 {
        let a = (22.5 + 45.0 * k as f32).to_radians();
        let d = v3(a.cos(), a.sin(), 0.0);
        dark_plate(b);
        b.beam(
            d * (PAD_R - 0.5) + Vec3::Z * 0.85,
            d * (PAD_R + 2.8) + Vec3::Z * 0.85,
            Vec2::new(2.4, 1.1),
            Vec2::new(2.0, 1.1),
        );
        piston(
            b,
            d * (PAD_R + 1.5) + Vec3::Z * 1.1,
            d * (PAD_R - 4.5) + Vec3::Z * 1.1,
            0.5,
            false,
        );
    }
}

/// One tower, standing out along +x (turned into place by the caller): its footing, the
/// laddered bronze legs up to its head, the plates lapped down its back, its rams.
fn tower(b: &mut MeshBuilder) {
    let fine = b.fine();
    let foot = v3(FOOT, 0.0, 3.2);
    let head = v3(HEAD, 0.0, HEAD_Z);
    let up = (head - foot).normalize();
    // Its footing: a plated block with a lit slot looking out, the owner's colour on top.
    seam(b);
    b.frustum_open(
        v3(FOOT + 0.5, 0.0, 0.0),
        Vec2::new(13.0, 9.0),
        Vec2::new(10.0, 7.0),
        3.2,
        Vec2::ZERO,
    );
    red_slot(b, v3(FOOT + 6.4, 0.0, 1.6), Vec3::X, Vec3::Y, 4.0, 0.35);
    // The legs, laddered together.
    for y in [-2.3f32, 2.3] {
        ribbed(
            b,
            foot + v3(-1.0, y * 1.2, 0.0),
            head + v3(0.8, y * 0.7, -0.6),
            0.75,
            if fine { 5 } else { 0 },
        );
    }
    if fine {
        metal(b);
        for k in 1..6 {
            let at = foot.lerp(head, k as f32 / 6.0);
            let w = 2.3 * (1.2 - 0.5 * k as f32 / 6.0);
            b.beam(
                at - Vec3::Y * w,
                at + Vec3::Y * w,
                Vec2::new(0.5, 0.5),
                Vec2::new(0.5, 0.5),
            );
        }
    }
    // Its back, armoured: plates lapped down from the head toward the footing, each
    // pointing its spike down and out.
    let back = v3(up.z, 0.0, -up.x);
    let f = Frame::new(head + back * 1.4 + up * 1.2, -up, back);
    dark_plate(b);
    Course {
        count: 4,
        step: 6.4,
        len: 9.0,
        half: 3.6,
        tip: 0.0,
        thick: THICK,
        tail: 3.0,
    }
    .lay(b, &f);
    // Flank plates either side, canted out and lapped the same way, their spikes out.
    b.mirror_y(|b| {
        let side = (back * 0.35 + Vec3::Y).normalize();
        let f = Frame::new(head + back * 0.6 + Vec3::Y * 3.4 + up * 0.4, -up, side);
        dark_plate(b);
        Course {
            count: 3,
            step: 7.2,
            len: 9.0,
            half: 2.2,
            tip: -1.0,
            thick: THICK * 0.8,
            tail: 2.0,
        }
        .lay(b, &f);
    });
    // The brace ram under it, and a press ram working at the footing's inner end.
    piston(
        b,
        v3(FOOT - 7.5, 0.0, 1.2),
        foot.lerp(head, 0.62) - back * 0.9,
        0.7,
        false,
    );
    collar(b, v3(FOOT - 7.5, 0.0, 1.2), Vec3::Y, 1.0, 2.4);
    piston(
        b,
        v3(FOOT - 4.8, 0.0, 3.2),
        v3(FOOT - 4.8, 0.0, 8.4),
        0.6,
        true,
    );
    // Its head: a plated block that takes the race, the owner's colour on top.
    dark_plate(b);
    b.frustum(
        head + v3(-0.6, 0.0, -1.4),
        Vec2::new(6.0, 5.0),
        Vec2::new(4.6, 4.0),
        3.8,
        Vec2::new(0.6, 0.0),
    );
    b.paint(TEAM);
    b.face(&[
        head + v3(-1.2, -1.6, 2.42),
        head + v3(1.8, -1.6, 2.42),
        head + v3(1.8, 1.6, 2.42),
        head + v3(-1.2, 1.6, 2.42),
    ]);
    red_slot(b, head + v3(-3.0, 0.0, 0.6), -Vec3::X, Vec3::Y, 2.6, 0.3);
}

/// A machine house between two towers, standing out along +x (turned into place by the
/// caller): a plated block with its plates lapped outward into spikes, a gear laid on
/// its roof between two press rams, a drum across its inner end.
fn house(b: &mut MeshBuilder) {
    let fine = b.fine();
    let (x0, x1, half, top) = (25.0, 41.0, 8.0, 6.5);
    seam(b);
    b.frustum_open(
        v3((x0 + x1) * 0.5, 0.0, 0.0),
        Vec2::new(x1 - x0, half * 2.0),
        Vec2::new(x1 - x0 - 2.0, half * 2.0 - 2.0),
        top,
        Vec2::ZERO,
    );
    // Its plates: a course either side of the roof's middle, lapped outward.
    b.mirror_y(|b| {
        let f = Frame::new(v3(x0 + 1.5, 5.3, top + 0.3), Vec3::X, v3(0.0, 0.3, 1.0));
        dark_plate(b);
        Course {
            count: 2,
            step: 6.0,
            len: 8.5,
            half: 2.8,
            tip: -1.0,
            thick: THICK,
            tail: 3.0,
        }
        .lay(b, &f);
    });
    // The gear on its roof, between the two plate courses, and its rams.
    let gear_at = v3((x0 + x1) * 0.5 - 1.0, 0.0, top + 0.5);
    metal(b);
    b.prism(gear_at - Vec3::Z * 0.5, b.sides(16), 2.2, 2.2, 1.0);
    if fine {
        teeth(b, gear_at, 2.0, 10, v3(0.6, 0.7, 0.9));
    }
    for x in [x0 + 2.0, x1 - 2.5] {
        dark_plate(b);
        b.block(v3(x - 1.2, -1.2, top), v3(x + 1.2, 1.2, top + 0.8));
        piston(b, v3(x, 0.0, top + 0.8), v3(x, 0.0, top + 5.0), 0.6, true);
    }
    // A drum across its inner end.
    collar(b, v3(x0 - 0.2, 0.0, 2.6), Vec3::Y, 2.2, half * 1.6);
    red_slot(b, v3(x1 + 0.05, 0.0, 3.5), Vec3::X, Vec3::Y, 5.0, 0.3);
}

/// The fixed race on the towers' heads with the fabricator heads hung from it, aimed at
/// the work, and the toothed lift ring turning inside it on four carriages.
fn ring(b: &mut MeshBuilder) {
    let fine = b.fine();
    let segs = if fine { 32 } else { 16 };
    dark_plate(b);
    hoop(b, RING + Vec3::Z * 0.4, RACE_R, 2.0, 1.6, segs);
    for (mount, s, aim) in tier_heads("naga_hatchery", 1) {
        let over = (mount.truncate().normalize() * RACE_R).extend(RING.z - 0.2);
        dark_plate(b);
        b.beam(
            over,
            mount + Vec3::Z * 0.3,
            Vec2::new(1.4, 1.2),
            Vec2::new(1.0, 0.9),
        );
        fabricator(b, mount, aim, s);
    }
    b.with_part(part::SPINNER, |b| {
        metal(b);
        hoop(b, RING, RING_R, 1.4, 1.2, segs);
        if fine {
            seam(b);
            teeth(b, RING, RING_R - 0.7, 24, v3(-0.5, 0.45, 0.9));
        }
        dark_plate(b);
        for k in 0..4 {
            let a = (90.0 * k as f32).to_radians();
            let d = v3(a.cos(), a.sin(), 0.0);
            let across = v3(-d.y, d.x, 0.0);
            let at = RING + d * RING_R;
            b.beam(
                at - across * 1.7 + Vec3::Z * 0.3,
                at + across * 1.7 + Vec3::Z * 0.3,
                Vec2::new(2.2, 1.6),
                Vec2::new(2.2, 1.6),
            );
        }
    });
}

/// Tech 2: a flywheel press on a house's outer end, standing out along +x (turned into
/// place by the caller): a plated block, a ribbed shaft across it turning two flywheels,
/// a ram working beside each, plates lapped down over its outer edge.
fn flywheels(b: &mut MeshBuilder) {
    let fine = b.fine();
    let (x0, x1, half, top) = (41.0, 44.8, 6.6, 5.0);
    seam(b);
    b.frustum_open(
        v3((x0 + x1) * 0.5, 0.0, 0.0),
        Vec2::new(x1 - x0, half * 2.0),
        Vec2::new(x1 - x0 - 1.2, half * 2.0 - 1.2),
        top,
        Vec2::ZERO,
    );
    let shaft = v3(x0 + 2.0, 0.0, top + 2.8);
    ribbed(
        b,
        shaft - Vec3::Y * (half - 0.4),
        shaft + Vec3::Y * (half - 0.4),
        0.5,
        if fine { 2 } else { 0 },
    );
    dark_plate(b);
    for y in [-(half - 1.0), half - 1.0] {
        b.block(
            v3(shaft.x - 1.0, y - 0.6, top),
            v3(shaft.x + 1.0, y + 0.6, shaft.z),
        );
    }
    for y in [-2.6f32, 2.6] {
        collar(b, shaft + Vec3::Y * y, Vec3::Y, 2.8, 0.9);
        if fine {
            seam(b);
            let sides = b.sides(10);
            b.cylinder_between(
                shaft + Vec3::Y * (y - 0.5),
                shaft + Vec3::Y * (y + 0.5),
                0.8,
                0.8,
                sides,
            );
        }
    }
    b.mirror_y(|b| {
        piston(
            b,
            v3(x0 + 0.9, half - 2.4, top),
            v3(x0 + 0.9, half - 2.4, top + 5.0),
            0.5,
            true,
        );
        let f = Frame::new(
            v3(x1 - 2.0, 2.2, top + 1.0),
            v3(1.0, 0.0, -0.5),
            v3(0.5, 0.25, 1.0),
        );
        dark_plate(b);
        Course {
            count: 2,
            step: 1.4,
            len: 2.6,
            half: 2.4,
            tip: -1.0,
            thick: THICK * 0.8,
            tail: 0.8,
        }
        .lay(b, &f);
    });
}

/// Tech 2: a fabricator head on a plated mast off each flank house's inner end, aimed at
/// the work, a ram bracing the mast.
fn masts(b: &mut MeshBuilder) {
    for (mount, s, aim) in tier_heads("naga_hatchery", 2) {
        let out = mount.truncate().normalize().extend(0.0);
        let foot = mount + out * 3.8;
        let foot = v3(foot.x, foot.y, 6.5);
        dark_plate(b);
        b.block(foot - v3(1.4, 1.4, 0.2), foot + v3(1.4, 1.4, 0.8));
        b.beam(
            foot + Vec3::Z * 0.6,
            mount + out * 1.0 + Vec3::Z * 1.2,
            Vec2::new(1.6, 1.4),
            Vec2::new(1.2, 1.1),
        );
        b.beam(
            mount + out * 1.0 + Vec3::Z * 1.2,
            mount + Vec3::Z * 0.4,
            Vec2::new(1.1, 1.0),
            Vec2::new(0.9, 0.9),
        );
        piston(
            b,
            foot + out * 2.4 + Vec3::Z * 0.3,
            foot.lerp(mount, 0.6) + out * 0.8,
            0.35,
            false,
        );
        red_slot(
            b,
            foot + out * 1.45 + Vec3::Z * 2.0,
            out,
            Vec3::Z,
            1.6,
            0.25,
        );
        fabricator(b, mount, aim, s);
    }
}

/// Tech 3: a second course of heavy blade plates down a tower's back (turned into place
/// by the caller), over the first, their spikes out past it.
fn tower_blades(b: &mut MeshBuilder) {
    let foot = v3(FOOT, 0.0, 3.2);
    let head = v3(HEAD, 0.0, HEAD_Z);
    let up = (head - foot).normalize();
    let back = v3(up.z, 0.0, -up.x);
    b.mirror_y(|b| {
        let side = (back * 0.55 + Vec3::Y).normalize();
        let f = Frame::new(head + back * 1.6 + Vec3::Y * 5.2 - up * 2.0, -up, side);
        dark_plate(b);
        let plates = Course {
            count: 2,
            step: 8.0,
            len: 10.5,
            half: 2.6,
            tip: -1.0,
            thick: THICK * 1.2,
            tail: 3.5,
        }
        .lay(b, &f);
        metal(b);
        for (g, long) in &plates {
            let at = g.at(long * 0.3, 0.0, 0.0);
            b.cylinder_between(at, at - g.n * 1.4, 0.45, 0.45, 5);
        }
    });
}

/// Tech 3: the crown, a fixed race raised over the lift ring on ribbed masts off the
/// towers' heads, a ring of blade plates lapped round it sweeping out into spikes, and two
/// more fabricator heads hung from it aimed down through the ring at the work.
fn crown(b: &mut MeshBuilder) {
    let fine = b.fine();
    let segs = if fine { 24 } else { 12 };
    for deg in TOWERS {
        let d = v3(deg.to_radians().cos(), deg.to_radians().sin(), 0.0);
        let foot = d * (HEAD + 0.2) + Vec3::Z * HEAD_TOP;
        let top = d * CROWN_R + Vec3::Z * (CROWN.z - 0.6);
        ribbed(b, foot, top, 0.9, if fine { 3 } else { 0 });
        dark_plate(b);
        b.block(foot - v3(1.5, 1.5, 0.2), foot + v3(1.5, 1.5, 0.9));
        b.frustum(
            top - Vec3::Z * 1.4,
            Vec2::new(3.2, 3.2),
            Vec2::new(2.6, 2.6),
            1.4,
            Vec2::ZERO,
        );
    }
    dark_plate(b);
    hoop(b, CROWN, CROWN_R, 2.2, 1.6, segs);
    // Blade plates round the crown, lapped one over the next and swept out.
    for k in 0..8 {
        let a = (22.5 + 45.0 * k as f32).to_radians();
        let d = v3(a.cos(), a.sin(), 0.0);
        let along = v3(-d.y, d.x, 0.0);
        let f = Frame::new(
            CROWN + d * (CROWN_R - 1.0) + Vec3::Z * 0.85 - along * 2.4,
            (d * 0.55 + along * 0.85).normalize(),
            Vec3::Z + d * 0.2,
        );
        armour(b, &f, &swept(7.0, 1.6, 1.0, 0.35), THICK);
    }
    b.paint(TEAM);
    for deg in [90.0f32, 270.0] {
        let d = v3(deg.to_radians().cos(), deg.to_radians().sin(), 0.0);
        let across = v3(-d.y, d.x, 0.0);
        let at = CROWN + d * CROWN_R + Vec3::Z * 0.82;
        b.face(&[
            at - across * 2.2 - d * 0.7,
            at + across * 2.2 - d * 0.7,
            at + across * 2.2 + d * 0.7,
            at - across * 2.2 + d * 0.7,
        ]);
    }
    for (mount, s, aim) in tier_heads("naga_hatchery", 3) {
        let over = (mount.truncate().normalize() * CROWN_R).extend(CROWN.z - 0.4);
        dark_plate(b);
        b.beam(
            over,
            mount + Vec3::Z * 0.3,
            Vec2::new(1.4, 1.2),
            Vec2::new(1.0, 0.9),
        );
        fabricator(b, mount, aim, s);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_hatchery_fits_its_lot_at_every_tier() {
        for (tech, height) in [(1, 30.0), (2, 30.0), (3, 38.0)] {
            super::super::check_at("naga_hatchery", tech, 46.0, height, Some(8), &[]);
            super::super::check_heads_at("naga_hatchery", tech, 46.0, height);
        }
    }
}
