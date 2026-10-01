//! The Anvil, the Regency land factory, on its 8 x 8 lot (96 m square): a press works.
//!
//! The unit is made on the lot origin and leaves toward +x, down the open bay between
//! two armoured machine wings. From above the works is an arrowhead: both wings are swept
//! back, and their plates lap back into points along the rear.
//!
//! - Each wing is a machine hall under three rows of heavy plates that overlap like
//!   feathers and sweep back, the last of each row running out into a spike. Bronze
//!   shows wherever they part: the ribbed deck they ride on. Across the roof, between
//!   the front and back courses, a plated hatch with a lit seam down it. Plated struts
//!   brace the outer row off the plinth. The inner face, toward the bay, is open
//!   machinery under the plates' eaves: bronze columns, pipe runs, a lit seam. Its prow
//!   is a door post, its edge lit violet where the unit leaves, red optics looking down
//!   the way.
//! - Over the bay, a fixed plated race carried off the wings' hatches, the fabricator
//!   heads hung from it aimed at the work (their mounts are `mc_core::print_heads`, where
//!   the nanite streams pour from), and inside it a toothed bronze ring turning on four
//!   carriages (`part::SPINNER`). The violet runs hot while the works builds.
//! - Behind the bay the press block: two feed rollers under a plated hood facing the bay,
//!   a slot down its roof, plates lapped back over it.
//! - The owner's colour runs along the leading plate of each wing, over the bay, and
//!   down the press block's slot.

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::material::*;
use crate::part;

use super::kit::{cable, dark_plate, metal, seam, v3};
use super::machine::*;

/// The wings: their inner faces stand at `INNER`, their outsides at `OUTER`.
const INNER: f32 = 13.0;
const OUTER: f32 = 36.0;
/// Where a wing's front edge meets its inner face; it runs back `SWEEP` by the outside.
const FRONT: f32 = 34.0;
const SWEEP: f32 = 12.0;
/// A wing's back edge: square across the back of the lot, then its outer corner cut forward
/// on the diagonal (`|x| + |y|` at most `CORNER_CUT`), so the wing's outline is swept.
const BACK: f32 = -44.0;
const CORNER_CUT: f32 = 66.0;
/// The roof: its ridge over the inner face, its eave over the outside.
const RIDGE: f32 = 17.0;
const EAVE: f32 = 5.0;
/// The plates' thickness.
const THICK: f32 = 0.9;
/// The fabrication ring: its middle (the spinner's pivot) and radius; the race it turns
/// in.
const RING: Vec3 = Vec3::new(0.0, 0.0, 15.2);
const RING_R: f32 = 10.6;
const RACE_R: f32 = 12.0;
/// The hatch across each wing's roof, between its front and back courses.
const HATCH: [f32; 2] = [7.0, -7.0];
/// The press block behind the bay.
const PRESS: [f32; 2] = [-38.0, -18.0];
const PRESS_TOP: f32 = 9.0;

/// Tech 2's outboard machine banks along each wing's outside: from `BANK_Y[0]` out to
/// `BANK_Y[1]`, `BANK_X[0]` (back) to `BANK_X[1]` (front), `BANK_TOP` high.
const BANK_Y: [f32; 2] = [37.2, 44.6];
const BANK_X: [f32; 2] = [-20.0, 16.0];
const BANK_TOP: f32 = 5.0;
/// Tech 3's lifted ring: its middle (the spinner's pivot once lifted), the ring's and the
/// race's radii, and the pylons carrying the race (one each side of the hatch, both
/// wings): the +x, +y one's foot in the hatch and its head on the race.
const HIGH: Vec3 = Vec3::new(0.0, 0.0, 33.0);
const HIGH_RING_R: f32 = 12.0;
const HIGH_RACE_R: f32 = 14.2;
const HIGH_FOOT: Vec3 = Vec3::new(5.0, 23.5, 11.2);
const HIGH_TOP: Vec3 = Vec3::new(5.0, 13.29, 33.0);

/// How far back a wing's front edge has swept at `y`.
fn swept_x(edge: f32, y: f32) -> f32 {
    edge - (y - INNER) * SWEEP / (OUTER - INNER)
}

/// Where a wing's back edge is at `y`.
fn back_x(y: f32) -> f32 {
    (y - CORNER_CUT).max(BACK)
}

/// The roof's height at `y` across a wing.
fn roof(y: f32) -> f32 {
    RIDGE - (y - INNER) * (RIDGE - EAVE) / (OUTER - INNER)
}

pub(super) fn brood(b: &mut MeshBuilder, tech: u8) {
    // Tech 3 lifts the fabrication ring high enough for the battle scorpion to stand
    // under it: the low race, its arms and its four heads come down.
    let lifted = tech >= 3;
    b.set_spinner_pivot(if lifted { HIGH } else { RING });
    if b.coarse() {
        coarse(b, lifted);
        return;
    }
    b.mirror_y(|b| {
        wing(b);
        gallery(b);
        prow(b);
        if !lifted {
            ring_arm(b, 72.0f32.to_radians());
            ring_arm(b, 108.0f32.to_radians());
        }
    });
    press(b);
    if !lifted {
        ring(b);
    }
    tier(b, tech, 2, 0.2, |b| {
        b.mirror_y(outboard);
        press_heads(b);
    });
    tier(b, tech, 3, 0.25, |b| {
        high_ring(b, lifted);
        b.mirror_y(blades);
        stern(b);
    });
}

/// Far off: the two swept wings, the press block, the owner's colour on the wings; once
/// lifted, the pylons that carry the ring (as one slab a side), the wings drawn plainer
/// to pay for them.
fn coarse(b: &mut MeshBuilder, lifted: bool) {
    b.mirror_y(|b| {
        dark_plate(b);
        let ring = |z: f32, grow: f32| -> Vec<Vec3> {
            let (y0, y1) = (INNER, OUTER + grow);
            vec![
                v3(swept_x(FRONT, y0), y0, z),
                v3(back_x(y0), y0, z),
                v3(back_x(y1), y1, z),
                v3(swept_x(FRONT, y1), y1, z),
            ]
        };
        let mut top = ring(RIDGE + 1.2, -24.0);
        for p in &mut top {
            p.z = roof(p.y) + 1.2;
        }
        if lifted {
            b.loft(&[ring(0.0, 0.0), top], false, true);
            b.beam(
                v3(0.0, HIGH_FOOT.y, HIGH_FOOT.z),
                v3(0.0, HIGH_TOP.y, HIGH.z + 1.2),
                Vec2::new(HIGH_FOOT.x * 2.0 + 3.0, 2.0),
                Vec2::new(HIGH_FOOT.x * 2.0 + 3.0, 2.0),
            );
        } else {
            b.loft(&[ring(0.0, 0.0), ring(EAVE, 0.0), top], false, true);
        }
        b.paint(TEAM);
        let y = INNER + 2.0;
        b.face(&[
            v3(swept_x(FRONT, y) - 14.0, y - 1.5, RIDGE + 1.35),
            v3(swept_x(FRONT, y) - 2.0, y - 1.5, RIDGE + 1.35),
            v3(swept_x(FRONT, y) - 2.0, y + 1.5, RIDGE + 1.1),
            v3(swept_x(FRONT, y) - 14.0, y + 1.5, RIDGE + 1.1),
        ]);
    });
    dark_plate(b);
    b.frustum_open(
        v3((PRESS[0] + PRESS[1]) * 0.5, 0.0, 0.0),
        Vec2::new(PRESS[1] - PRESS[0], INNER * 2.0),
        Vec2::new(PRESS[1] - PRESS[0] - 4.0, INNER * 2.0 - 6.0),
        PRESS_TOP + 1.0,
        Vec2::ZERO,
    );
    if !lifted {
        b.paint(GLOW_VIOLET);
        b.cylinder_between(
            RING - Vec3::Y * RING_R,
            RING + Vec3::Y * RING_R,
            0.6,
            0.6,
            3,
        );
    }
}

/// One wing (+y): the hall's core, the bronze deck on it, three rows of plates lapped
/// back over it, the hatch between them and the struts bracing the outer row.
fn wing(b: &mut MeshBuilder) {
    let fine = b.fine();
    // The hall's core, standing on the lot.
    seam(b);
    let core = |front: bool| -> Vec<Vec3> {
        let section = [
            (INNER + 3.0, 0.0),
            (OUTER - 1.0, 0.0),
            (OUTER - 1.0, EAVE - 0.6),
            (INNER + 3.0, RIDGE - 0.8),
        ];
        section
            .iter()
            .map(|&(y, z)| {
                let x = if front {
                    swept_x(FRONT, y) - 1.5
                } else {
                    back_x(y) + 1.5
                };
                v3(x, y, z)
            })
            .collect()
    };
    b.loft(&[core(false), core(true)], true, true);

    // The machinery deck on the core's roof: bronze, ribbed across, where the plates
    // over it leave gaps.
    let deck = |front: bool| -> Vec<Vec3> {
        [
            (INNER + 1.0, 0.0),
            (OUTER - 0.5, 0.0),
            (OUTER - 0.5, 0.5),
            (INNER + 1.0, 0.5),
        ]
        .iter()
        .map(|&(y, dz)| {
            let x = if front {
                swept_x(FRONT, y) - 1.0
            } else {
                back_x(y) + 1.0
            };
            v3(x, y, roof(y) - 0.9 + dz)
        })
        .collect()
    };
    metal(b);
    b.loft(&[deck(false), deck(true)], true, true);
    if fine {
        // Ribs across it, every 4 m, swept like the wing's ends.
        let (ya, yb) = (INNER + 1.0, OUTER - 0.5);
        let run = (yb - ya) * SWEEP / (OUTER - INNER);
        let mut xa = swept_x(FRONT, ya) - 2.0;
        while xa - run > back_x(yb) + 1.0 && xa > back_x(ya) + 1.0 {
            b.beam(
                v3(xa, ya, roof(ya) - 0.3),
                v3(xa - run, yb, roof(yb) - 0.3),
                Vec2::new(0.7, 0.6),
                Vec2::new(0.7, 0.6),
            );
            xa -= 4.0;
        }
    }

    // Three rows of blade plates lapped back over the deck, the inner one over the
    // gallery's eaves, the outer one hanging past the wall. Each row is two courses,
    // with the hatch open between them.
    let rows: [(f32, f32); 3] = [
        (INNER - 1.2, INNER + 7.0),
        (INNER + 8.4, INNER + 15.4),
        (INNER + 16.8, OUTER + 1.5),
    ];
    for (i, &(y0, y1)) in rows.iter().enumerate() {
        let (z0, z1) = (roof(y0) + 0.4, roof(y1) + 0.4);
        let y = (y0 + y1) * 0.5;
        let slope = v3(0.0, y1 - y0, z1 - z0);
        let normal = v3(0.0, -slope.z, slope.y);
        let front = swept_x(FRONT, y) + 2.0 - i as f32 * 1.0;
        // The last plate's spike ends inside the wing's cut corner, at the row's outer edge.
        let tail = 6.0;
        let back = back_x(y1.max(y0) + 1.0) + tail + 0.5;
        let [bay_front, bay_back] = HATCH;
        let half = slope.length() * 0.5;
        dark_plate(b);
        let mut plates = Vec::new();
        let rear = if bay_back - back > 24.0 { 3 } else { 2 };
        for (from, to, count, tail) in [(front, bay_front, 2, 3.0), (bay_back, back, rear, tail)] {
            let len = (from - to) * 0.62;
            let f = Frame::new(v3(from, y, (z0 + z1) * 0.5), -Vec3::X, normal);
            plates.extend(
                Course {
                    count,
                    step: (from - to - len) / (count - 1) as f32,
                    len,
                    half,
                    tip: -1.0,
                    thick: THICK,
                    tail,
                }
                .lay(b, &f),
            );
        }
        if fine {
            // The standoffs they ride on, down to the deck.
            metal(b);
            for &(g, long) in &plates {
                let top = g.at(long * 0.3, 0.0, 0.0);
                b.cylinder_between(top, top - g.n * 1.0, 0.5, 0.5, 4);
            }
        }
        if i == 0 {
            // The owner's colour along the leading plate's inner edge, over the bay.
            let (g, _) = plates[0];
            let (t0, t1) = (half - 2.2, half - 0.9);
            b.paint(TEAM);
            b.face(&[
                g.at(1.0, t0, THICK + 0.03),
                g.at(13.0, t0, THICK + 0.03),
                g.at(13.0, t1, THICK + 0.03),
                g.at(1.0, t1, THICK + 0.03),
            ]);
        }
    }
    hatch(b);
    // The struts under the outer row.
    if fine {
        for k in 0..3 {
            let y = OUTER + 0.6;
            let x = swept_x(FRONT, y) - 8.0 - k as f32 * 18.0;
            strut(b, v3(x, y, 1.4), v3(x + 1.5, y - 3.2, EAVE + 0.4), 0.55);
        }
    }
}

/// The hatch across a wing's roof (+y side), where the ring's arms come off: a plate laid
/// down the roof between the front and back courses, a lit seam down its middle.
fn hatch(b: &mut MeshBuilder) {
    let mid = (HATCH[0] + HATCH[1]) * 0.5;
    let (y0, y1) = (INNER + 0.5, OUTER - 1.0);
    let normal = v3(0.0, RIDGE - EAVE, OUTER - INNER);
    let f = Frame::new(
        v3(mid, y0, roof(y0) - 0.1),
        v3(0.0, y1 - y0, roof(y1) - roof(y0)),
        normal,
    );
    let len = v3(0.0, y1 - y0, roof(y1) - roof(y0)).length();
    let half = (HATCH[0] - HATCH[1]) * 0.5 - 1.0;
    dark_plate(b);
    armour(
        b,
        &f,
        &[[0.0, -half], [0.0, half], [len, half], [len, -half]],
        THICK,
    );
    if b.fine() {
        red_slot(b, f.at(len * 0.5, 0.0, THICK), f.n, f.u, len - 4.0, 0.25);
    }
}

/// A wing's inner face toward the bay (+y side): the hall's core stands back under the
/// eaves, and in front of it bronze columns, pipe runs and a lit seam.
fn gallery(b: &mut MeshBuilder) {
    let fine = b.fine();
    let face = INNER + 1.4;
    let top = RIDGE - 1.6;
    let columns = [-22.0, -9.0, 4.0, 17.0, 29.0];
    for &x in &columns {
        shaft(b, v3(x, face, 1.2), v3(x, face, top), 0.7);
        dark_plate(b);
        b.block(v3(x - 1.2, face - 0.9, 0.0), v3(x + 1.2, face + 1.4, 1.6));
    }
    if fine {
        metal(b);
        for z in [3.2, 9.8] {
            cable(
                b,
                &[v3(columns[0], face + 0.6, z), v3(columns[4], face + 0.6, z)],
                0.32,
            );
        }
    }
    red_slot(
        b,
        v3(6.0, face + 1.5, top - 1.2),
        -Vec3::Y,
        Vec3::X,
        30.0,
        0.18,
    );
}

/// A wing's prow (+y side): an armoured door post at the bay's mouth, its inner edge lit
/// violet; red optics on its face, plates swept back off it.
fn prow(b: &mut MeshBuilder) {
    let x = FRONT;
    dark_plate(b);
    let post = |dz: f32, z: f32| -> Vec<Vec3> {
        vec![
            v3(x + 1.5 - dz, INNER - 1.2, z),
            v3(x - 4.0, INNER - 1.2, z),
            v3(x - 4.0, INNER + 4.0, z),
            v3(x - 1.0 - dz, INNER + 4.0, z),
        ]
    };
    b.loft(
        &[post(0.0, 0.0), post(0.0, 12.0), post(3.0, RIDGE + 0.5)],
        false,
        true,
    );
    b.paint(GLOW_VIOLET);
    b.beam(
        v3(x + 1.0, INNER - 1.3, 0.8),
        v3(x - 1.2, INNER - 1.3, 12.0),
        Vec2::new(0.35, 0.5),
        Vec2::new(0.35, 0.5),
    );
    // Its optics: three red slots down the face, looking out along the way.
    let out = v3(1.0, 0.0, 0.0);
    for (k, z) in [7.0, 8.4, 9.8].into_iter().enumerate() {
        red_slot(
            b,
            v3(x + 1.4 - z * 0.03, INNER + 1.0, z),
            out,
            Vec3::Y,
            2.4 - k as f32 * 0.5,
            0.3,
        );
    }
}

/// A plated arm from the fixed race out to the wing's ridge (+y side, at `angle` round
/// the ring), a plated strut under it.
fn ring_arm(b: &mut MeshBuilder, angle: f32) {
    let d = v3(angle.cos(), angle.sin(), 0.0);
    let from = RING + d * (RACE_R + 0.6) + Vec3::Z * 0.4;
    let to = v3(from.x + d.x * 2.0, INNER + 3.0, RIDGE + 0.6);
    dark_plate(b);
    b.beam(from, to, Vec2::new(1.8, 1.3), Vec2::new(2.6, 1.6));
    if b.fine() {
        strut(
            b,
            v3(to.x, INNER + 1.4, RIDGE - 3.5),
            from - Vec3::Z * 0.6,
            0.35,
        );
    }
}

/// The fixed race over the bay with the fabricator heads hung from it, aimed at the
/// work, and the toothed ring turning inside it on four plated carriages.
fn ring(b: &mut MeshBuilder) {
    let fine = b.fine();
    let segs = if fine { 32 } else { 16 };
    dark_plate(b);
    hoop(b, RING + Vec3::Z * 0.6, RACE_R, 1.8, 1.4, segs);
    for (mount, s, aim) in tier_heads("regency_brood", 1) {
        // The hanger from the race down to the head's trunnion.
        let over = (mount.truncate().normalize() * RACE_R).extend(RING.z);
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
        hoop(b, RING, RING_R, 1.3, 1.1, segs);
        if fine {
            seam(b);
            teeth(b, RING, RING_R - 0.65, 24, v3(-0.5, 0.45, 0.8));
        }
        dark_plate(b);
        for k in 0..4 {
            let a = (90.0 * k as f32).to_radians();
            let d = v3(a.cos(), a.sin(), 0.0);
            let across = v3(-d.y, d.x, 0.0);
            let at = RING + d * RING_R;
            b.beam(
                at - across * 1.6 + Vec3::Z * 0.3,
                at + across * 1.6 + Vec3::Z * 0.3,
                Vec2::new(2.0, 1.5),
                Vec2::new(2.0, 1.5),
            );
        }
    });
}

/// The press block behind the bay: its core, two feed rollers under a hood facing the
/// bay, a slot down its roof in the owner's colour, and plates lapped back over it.
fn press(b: &mut MeshBuilder) {
    let fine = b.fine();
    let [back, front] = PRESS;
    seam(b);
    b.frustum_open(
        v3((back + front) * 0.5, 0.0, 0.0),
        Vec2::new(front - back, INNER * 2.0 + 1.0),
        Vec2::new(front - back - 2.0, INNER * 2.0),
        PRESS_TOP,
        Vec2::ZERO,
    );
    // The feed rollers, and the hood over them.
    for y in [-6.0f32, 6.0] {
        collar(b, v3(front + 0.2, y, 3.4), Vec3::Y, 3.0, 9.0);
        if fine {
            for k in [-1.0, 1.0] {
                collar(b, v3(front + 0.2, y + k * 4.7, 3.4), Vec3::Y, 1.4, 0.8);
            }
        }
    }
    dark_plate(b);
    let hood = Frame::new(
        v3(front + 4.2, 0.0, 7.4),
        v3(-1.0, 0.0, 0.6),
        v3(0.6, 0.0, 1.0),
    );
    armour(
        b,
        &hood,
        &[[0.0, -12.6], [0.0, 12.6], [5.0, 12.0], [5.0, -12.0]],
        THICK,
    );
    b.mirror_y(|b| {
        let f = Frame::new(
            v3(front + 1.0, 7.4, PRESS_TOP + 0.4),
            -Vec3::X,
            v3(0.0, 0.22, 1.0),
        );
        dark_plate(b);
        Course {
            count: 3,
            step: 7.0,
            len: 10.0,
            half: 5.6,
            tip: -0.6,
            thick: THICK,
            tail: 4.0,
        }
        .lay(b, &f);
    });
    // The owner's colour down the slot.
    b.paint(TEAM);
    b.face(&[
        v3(front - 3.0, -1.2, PRESS_TOP + 0.05),
        v3(front - 3.0, 1.2, PRESS_TOP + 0.05),
        v3(back + 2.0, 1.2, PRESS_TOP + 0.05),
        v3(back + 2.0, -1.2, PRESS_TOP + 0.05),
    ]);
}

/// Tech 2: an outboard machine bank along a wing's outside (+y side): a low plated
/// housing, feed lines up the wing's wall, and a course of plates lapped back along its
/// outer edge into a spike.
fn outboard(b: &mut MeshBuilder) {
    let fine = b.fine();
    let [y0, y1] = BANK_Y;
    let [x0, x1] = BANK_X;
    seam(b);
    b.frustum_open(
        v3((x0 + x1) * 0.5, (y0 + y1) * 0.5, 0.0),
        Vec2::new(x1 - x0, y1 - y0),
        Vec2::new(x1 - x0 - 2.0, y1 - y0 - 1.6),
        BANK_TOP,
        Vec2::ZERO,
    );
    let wall_y = y0 + 1.8;
    // The plates along the outer edge, shedding outward, lapped back into a spike.
    let f = Frame::new(
        v3(x1 + 1.0, y1 - 2.7, BANK_TOP + 0.4),
        -Vec3::X,
        v3(0.0, 0.35, 1.0),
    );
    dark_plate(b);
    Course {
        count: 4,
        step: 8.3,
        len: 11.0,
        half: 3.0,
        tip: -1.0,
        thick: THICK,
        tail: 3.0,
    }
    .lay(b, &f);
    red_slot(
        b,
        v3(x1 - 0.9, (y0 + y1) * 0.5, BANK_TOP * 0.5),
        Vec3::X,
        Vec3::Y,
        4.0,
        0.3,
    );
    if fine {
        // Feed lines from the bank up the wing's wall.
        metal(b);
        for x in [-6.0, 4.0] {
            cable(
                b,
                &[
                    v3(x, wall_y + 0.6, BANK_TOP),
                    v3(x, OUTER + 0.4, BANK_TOP + 0.2),
                    v3(x, OUTER - 0.4, EAVE + 0.2),
                ],
                0.3,
            );
        }
    }
}

/// Tech 2: two fabricator heads on plated arms off the press block's face, over the hood,
/// aimed at the work, a plated strut under each arm.
fn press_heads(b: &mut MeshBuilder) {
    for (mount, s, aim) in tier_heads("regency_brood", 2) {
        let foot = v3(PRESS[1] - 4.0, mount.y, PRESS_TOP + 1.0);
        dark_plate(b);
        b.block(foot - v3(1.6, 1.4, 1.0), foot + v3(1.6, 1.4, 0.8));
        b.beam(
            foot + Vec3::Z * 0.6,
            mount + v3(-0.8, 0.0, 0.9),
            Vec2::new(1.6, 1.3),
            Vec2::new(1.2, 1.0),
        );
        strut(
            b,
            foot + v3(1.4, 0.0, 0.2),
            mount + v3(-0.5, 0.0, -0.4),
            0.4,
        );
        fabricator(b, mount, aim, s);
    }
}

/// Tech 3: the lifted ring. Four pylons (a pair off each wing's hatch) lean in to
/// carry a fixed race high over the bay, the fabricator heads hung from it aimed at the
/// work, a toothed bronze ring turning inside it on four carriages. `spins`: whether
/// this is the ring the spinner turns (not while it is still an upgrade piece).
fn high_ring(b: &mut MeshBuilder, spins: bool) {
    let fine = b.fine();
    let segs = if fine { 24 } else { 12 };
    b.mirror_y(|b| {
        for sx in [-1.0f32, 1.0] {
            pylon(b, sx);
        }
    });
    dark_plate(b);
    hoop(b, HIGH + Vec3::Z * 0.5, HIGH_RACE_R, 2.4, 1.8, segs);
    // Blade plates lapped round the race, each swept out past it into a spike.
    for k in 0..8 {
        let a = (22.5 + 45.0 * k as f32).to_radians();
        let d = v3(a.cos(), a.sin(), 0.0);
        let along = v3(-d.y, d.x, 0.0);
        let f = Frame::new(
            HIGH + d * (HIGH_RACE_R - 1.2) + Vec3::Z * 1.45 - along * 2.6,
            (d * 0.55 + along * 0.85).normalize(),
            Vec3::Z + d * 0.2,
        );
        armour(b, &f, &swept(8.0, 1.9, 1.0, 0.4), THICK);
    }
    for (mount, s, aim) in tier_heads("regency_brood", 3) {
        let over = (mount.truncate().normalize() * HIGH_RACE_R).extend(HIGH.z);
        dark_plate(b);
        b.beam(
            over,
            mount + Vec3::Z * 0.3,
            Vec2::new(1.5, 1.3),
            Vec2::new(1.1, 1.0),
        );
        fabricator(b, mount, aim, s);
    }
    let ring = |b: &mut MeshBuilder| {
        metal(b);
        hoop(b, HIGH, HIGH_RING_R, 1.4, 1.2, segs);
        if fine {
            seam(b);
            teeth(b, HIGH, HIGH_RING_R - 0.7, 16, v3(-0.6, 0.6, 0.9));
        }
        dark_plate(b);
        for k in 0..4 {
            let a = (45.0 + 90.0 * k as f32).to_radians();
            let d = v3(a.cos(), a.sin(), 0.0);
            let across = v3(-d.y, d.x, 0.0);
            let at = HIGH + d * HIGH_RING_R;
            b.beam(
                at - across * 1.7 + Vec3::Z * 0.3,
                at + across * 1.7 + Vec3::Z * 0.3,
                Vec2::new(2.2, 1.6),
                Vec2::new(2.2, 1.6),
            );
        }
    };
    if spins {
        b.with_part(part::SPINNER, ring);
    } else {
        ring(b);
    }
}

/// One pylon of the lifted ring (+y wing, `sx` the side of the hatch): a
/// bronze leg from a plated foot in the hatch, leaning in to a plated head under the
/// race; plates lapped down its outer face into spikes, a plated strut bracing it off the
/// wing's roof.
fn pylon(b: &mut MeshBuilder, sx: f32) {
    let foot = v3(HIGH_FOOT.x * sx, HIGH_FOOT.y, HIGH_FOOT.z);
    let head = v3(HIGH_TOP.x * sx, HIGH_TOP.y, HIGH_TOP.z - 1.0);
    let up = (head - foot).normalize();
    dark_plate(b);
    b.block(foot - v3(2.4, 2.6, 1.6), foot + v3(2.4, 2.6, 0.8));
    shaft(b, foot + Vec3::Z * 0.6, head - Vec3::Z * 0.4, 0.95);
    // Its outer face, armoured: plates lapped down from the head toward the foot, their
    // spikes pointing down and out.
    let out = v3(0.0, up.z, -up.y).normalize();
    let f = Frame::new(head + out * 1.1 + up * 0.6, -up, out);
    dark_plate(b);
    Course {
        count: 3,
        step: 6.4,
        len: 8.4,
        half: 2.6,
        tip: 0.0,
        thick: THICK,
        tail: 2.6,
    }
    .lay(b, &f);
    // The strut off the wing's roof, and the head that takes the race.
    let brace = v3(foot.x, OUTER - 7.0, roof(OUTER - 7.0) + 0.2);
    strut(b, brace, foot.lerp(head, 0.55) + out * 0.6, 0.6);
    dark_plate(b);
    b.frustum(
        head + v3(0.0, 0.0, -1.2),
        Vec2::new(3.6, 4.6),
        Vec2::new(3.0, 3.8),
        2.9,
        Vec2::ZERO,
    );
    b.paint(TEAM);
    b.face(&[
        head + v3(-1.2, -1.4, 1.72),
        head + v3(1.2, -1.4, 1.72),
        head + v3(1.2, 1.4, 1.72),
        head + v3(-1.2, 1.4, 1.72),
    ]);
    red_slot(b, head + v3(0.0, 2.35, 0.2), Vec3::Y, Vec3::X, 2.0, 0.3);
}

/// Tech 3: heavy blade plates over an outboard bank (+y side), standing off it on posts,
/// swept back into long spikes past its end.
fn blades(b: &mut MeshBuilder) {
    let [y0, y1] = BANK_Y;
    let x1 = BANK_X[1];
    let f = Frame::new(
        v3(x1 + 3.0, (y0 + y1) * 0.5 + 0.6, BANK_TOP + 3.4),
        -Vec3::X,
        v3(0.0, 0.45, 1.0),
    );
    dark_plate(b);
    let plates = Course {
        count: 3,
        step: 10.0,
        len: 14.0,
        half: 3.4,
        tip: -1.0,
        thick: 1.1,
        tail: 6.0,
    }
    .lay(b, &f);
    metal(b);
    for (g, long) in &plates {
        let top = g.at(long * 0.35, 0.0, 0.0);
        b.cylinder_between(v3(top.x, top.y - 1.0, BANK_TOP), top, 0.5, 0.4, 6);
    }
    // A lit seam along the leading plate's edge, where the bank looks forward.
    let (g, _) = plates[0];
    red_slot(b, g.at(0.2, 0.0, 1.15), g.n, g.v, 4.0, 0.22);
}

/// Tech 3: a flywheel drum across the back of the press block under a plated hood lapped
/// back into spikes, driving the press through two rods.
fn stern(b: &mut MeshBuilder) {
    let fine = b.fine();
    let at = v3(PRESS[0] - 3.0, 0.0, 3.4);
    collar(b, at, Vec3::Y, 3.1, 16.0);
    dark_plate(b);
    for y in [-7.0f32, 7.0] {
        b.block(v3(at.x - 2.6, y - 1.0, 0.0), v3(at.x + 2.6, y + 1.0, 1.4));
    }
    b.mirror_y(|b| {
        let f = Frame::new(
            v3(PRESS[0] + 0.6, 4.2, PRESS_TOP + 0.2),
            v3(-1.0, 0.0, -0.35),
            v3(-0.35, 0.2, 1.0),
        );
        dark_plate(b);
        Course {
            count: 2,
            step: 3.0,
            len: 5.0,
            half: 3.8,
            tip: -0.4,
            thick: THICK,
            tail: 1.6,
        }
        .lay(b, &f);
        metal(b);
        b.cylinder_between(
            v3(at.x + 1.0, 5.0, at.z + 1.6),
            v3(PRESS[0] + 2.0, 5.0, PRESS_TOP - 0.5),
            0.5,
            0.4,
            if fine { 6 } else { 4 },
        );
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_brood_fits_its_lot_at_every_tier() {
        for (tech, height) in [(1, 22.0), (2, 22.0), (3, 35.0)] {
            super::super::check_at("regency_brood", tech, 46.0, height, Some(8), &[]);
            super::super::check_heads_at("regency_brood", tech, 46.0, height);
        }
    }
}
