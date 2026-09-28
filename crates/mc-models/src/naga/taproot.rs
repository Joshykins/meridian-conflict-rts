//! The Excavator, the Naga mass extractor, on its 3 x 3 lot (36 m square): a bore cut by
//! a beam. It upgrades in place through four tiers, like ARC's core mine, each adding its
//! machinery to the last; the beam grows wider with every tier.
//!
//! Nothing strikes and nothing is driven down: a red-white excavation beam runs from the
//! emitter head straight down into the ground, cutting a glowing shaft, and the ore it
//! frees is drawn up the column round it (the beam, the converging pinch beams and the
//! rising ore are drawn by `renderer/naga_mine_fx.rs` from `Model::excavation`).
//!
//! - The bore: an armoured collar round its mouth, and the shaft down from it, its walls
//!   glassed dark with lit red bands where the beam has cut, white-hot at the bottom where
//!   it bites, the beam's core running down it (`Model::pit`: the shader shows it through
//!   the ground).
//! - Four outrigger legs on the diagonals brace the rig against the ore field: ribbed
//!   bronze struts under plates lapped down and out into spikes, a clamp ram working at
//!   each foot (`part::PUMP`), a clawed pad gripping the ground (`part::ASHORE`). They
//!   carry a plated yoke over the mouth, and the emitter head hangs in it: a plated body,
//!   the owner's colour on its cap, a bronze lens barrel aimed down, its tip lit red.
//! - A collector ring floats round the beam over the mouth, turning (`part::SPINNER`),
//!   where the ore comes up.
//! - Tech 2: four pinch emitters on the legs aimed at the mouth, a second collector ring,
//!   focusing coils round the head, and a crown over it on four struts to a plated hub.
//!   Tech 3: four emitter posts on the lot's axes, a third ring, an armoured stage on the
//!   hub. Tech 4 (the deep core): plates lapped down over the crown, capacitor drums at
//!   every foot, a wider lens; its bore goes deepest and the beam surges.
//! - On open water the whole rig rides `LIFT` higher on four piles, a coaming round the
//!   beam down into the water (`part::AFLOAT`), no bore.

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::material::*;
use crate::{part, pattern, Excavation, Pit};

use super::kit::{dark_plate, metal, seam, v3};
use super::machine::tier;
use super::machine::*;

/// How much higher the rig stands on open water, on its piles.
const LIFT: f32 = 3.5;
/// The legs' bearings, and how far out their feet stand.
const LEGS: [f32; 4] = [45.0, 135.0, 225.0, 315.0];
const REACH: f32 = 12.8;
/// The bore's opening (`Pit::open`), and by tier: its mouth's radius, the depth of its
/// floor, and the beam's width.
const OPEN: f32 = 1.6;
const MOUTH: [f32; 4] = [2.4, 3.0, 3.6, 4.2];
const DEPTH: [f32; 4] = [-40.0, -60.0, -80.0, -120.0];
const BEAM: [f32; 4] = [0.7, 1.1, 1.6, 2.3];
/// The collar round the mouth: how far it reaches past the mouth, and its top.
const COLLAR_W: f32 = 1.6;
const COLLAR_TOP: f32 = 2.2;
/// The yoke the legs carry over the mouth: its height and radius.
const YOKE: (f32, f32) = (8.2, 3.2);
/// The emitter head: its lens tip (where the beam leaves), the body's foot and the cap's
/// top.
const EMIT: f32 = 6.2;
const BODY: f32 = 6.9;
const CAP_TOP: f32 = 10.6;
/// The collector rings by tier: height and radius. They turn about the beam.
const COLLECTORS: [(f32, f32); 3] = [(3.0, 3.6), (4.5, 4.4), (5.9, 5.2)];
/// The crown's hub (tech 2) and the stage on it (tech 3): their tops.
const HUB: f32 = 14.4;
const STAGE: f32 = 18.4;
/// Where the leg pinch emitters sit along their legs, from the root; and the tech 3
/// emitter posts: how far out on the axes, and their emitters' height.
const LEG_PINCH: f32 = 0.42;
const POST: (f32, f32) = (9.0, 5.4);
/// Seconds between the deep core's surges.
const SURGE: f32 = 7.0;
const THICK: f32 = 0.5;

pub(super) fn taproot(b: &mut MeshBuilder, tech: u8) {
    let tech = tech.clamp(1, 4);
    let t = tech as usize - 1;
    b.set_pit(Pit {
        open: OPEN,
        radius: MOUTH[t] + 0.1,
        stroke: 0.0,
        section: 0.0,
        rack: [0.0, 0.0],
        afloat_lift: LIFT,
    });
    b.set_excavation(Excavation {
        emitter: [0.0, 0.0, EMIT],
        width: BEAM[t],
        pinches: pinches(tech),
        surge: if tech >= 4 { SURGE } else { 0.0 },
    });
    b.set_spinner_pivot(Vec3::Z * COLLECTORS[0].0);
    if b.coarse() {
        coarse(b, tech);
        return;
    }
    b.with_part(part::ASHORE, |b| bore(b, tech));
    collar_ring(b, tech);
    for deg in LEGS {
        b.yawed(Vec3::ZERO, deg.to_radians(), leg);
    }
    head(b);
    for k in 1..=3u8 {
        tier(b, tech, k, 0.3, |b| {
            b.with_part(part::SPINNER, |b| collector(b, k));
        });
    }
    tier(b, tech, 2, 0.15, |b| {
        for deg in LEGS {
            b.yawed(Vec3::ZERO, deg.to_radians(), leg_pinch);
        }
        coils(b);
        crown(b);
    });
    tier(b, tech, 3, 0.15, |b| {
        for deg in LEGS {
            b.yawed(Vec3::ZERO, (deg + 45.0).to_radians(), post);
        }
        stage(b);
    });
    tier(b, tech, 4, 0.15, |b| {
        for deg in LEGS {
            b.yawed(Vec3::ZERO, deg.to_radians(), drums);
        }
        shroud(b);
        lens_ring(b);
    });
    b.with_part(part::AFLOAT, |b| {
        // The coaming round the beam, down into the water.
        dark_plate(b);
        let sides = b.sides(12);
        hoop(b, Vec3::Z * (LIFT * 0.5), MOUTH[t] + 0.9, 1.2, LIFT, sides);
    });
}

/// The pinch emitters' tips at `tech`: on the legs from tech 2, on the posts from tech 3.
fn pinches(tech: u8) -> Vec<[f32; 3]> {
    let mut tips = Vec::new();
    if tech >= 2 {
        tips.extend(LEGS.map(|deg| yaw(leg_pinch_tip(), deg)));
    }
    if tech >= 3 {
        tips.extend(LEGS.map(|deg| yaw(post_tip(), deg + 45.0)));
    }
    tips.iter().map(|p| p.to_array()).collect()
}

fn yaw(p: Vec3, deg: f32) -> Vec3 {
    let (s, c) = deg.to_radians().sin_cos();
    v3(p.x * c - p.y * s, p.x * s + p.y * c, p.z)
}

/// A leg (along +x) from its root under the yoke down to its foot.
fn leg_line() -> (Vec3, Vec3) {
    (
        v3(YOKE.1 + 0.2, 0.0, YOKE.0 - 0.6),
        v3(REACH - 1.5, 0.0, 4.2),
    )
}

/// Where a leg's pinch emitter is mounted (leg along +x), and its tip, aimed at the mouth.
fn leg_pinch_mount() -> Vec3 {
    let (root, foot) = leg_line();
    root.lerp(foot, LEG_PINCH) + Vec3::Z * 1.1
}

fn leg_pinch_tip() -> Vec3 {
    let mount = leg_pinch_mount();
    mount + (Vec3::Z * OPEN - mount).normalize() * 1.9
}

/// The tip of a tech 3 post's emitter (post along +x), aimed at the mouth.
fn post_tip() -> Vec3 {
    let mount = v3(POST.0 - 0.6, 0.0, POST.1);
    mount + (Vec3::Z * OPEN - mount).normalize() * 2.2
}

/// Far off: a red mouth, a tent along each leg, the head, the crown from tech 2, the
/// owner's colour on top.
fn coarse(b: &mut MeshBuilder, tech: u8) {
    let mouth = MOUTH[tech as usize - 1];
    b.with_part(part::ASHORE, |b| {
        b.paint(GLOW_LASER).pattern(pattern::NONE);
        b.face(&ring(6, mouth, COLLAR_TOP + 0.02));
    });
    dark_plate(b);
    b.prism(Vec3::Z * BODY, 4, 1.9, 1.3, CAP_TOP - BODY);
    for deg in LEGS {
        b.yawed(Vec3::ZERO, deg.to_radians(), |b| {
            let (top, tip) = (v3(3.0, 0.0, YOKE.0), v3(REACH + 1.8, 0.0, 0.3));
            let eave = top.lerp(tip, 0.5) - Vec3::Z * 1.2;
            dark_plate(b);
            b.face(&[top, eave - Vec3::Y * 1.8, tip]);
            b.face(&[top, tip, eave + Vec3::Y * 1.8]);
        });
    }
    let top = match tech {
        1 => CAP_TOP,
        2 => HUB,
        _ => STAGE,
    };
    if tech >= 2 {
        dark_plate(b);
        b.prism(Vec3::Z * CAP_TOP, 4, 2.2, 1.0, top - CAP_TOP);
    }
    b.paint(TEAM);
    b.prism(Vec3::Z * top, 3, 1.2, 0.9, 0.3);
    b.with_part(part::AFLOAT, |b| {
        dark_plate(b);
        b.frustum_open(
            Vec3::ZERO,
            Vec2::splat(mouth * 2.0 + 3.0),
            Vec2::splat(mouth * 2.0 + 2.0),
            LIFT + 0.8,
            Vec2::ZERO,
        );
    });
}

/// `n` points round a level circle of radius `r` at height `z`, anticlockwise from above.
fn ring(n: usize, r: f32, z: f32) -> Vec<Vec3> {
    (0..n)
        .map(|i| {
            let a = std::f32::consts::TAU * i as f32 / n as f32;
            v3(a.cos() * r, a.sin() * r, z)
        })
        .collect()
}

/// The bore at `tech`: the shaft from the opening down to its floor in short lengths
/// (the shader decides length by length whether the eye sees it through the opening),
/// glassed dark walls with a lit red band at every joint, a white-hot floor, and the
/// beam's core down the middle.
fn bore(b: &mut MeshBuilder, tech: u8) {
    let t = tech as usize - 1;
    // Below full detail only the top of the shaft, which is all the eye can follow.
    let (mouth, floor) = (MOUTH[t], if b.fine() { DEPTH[t] } else { -24.0 });
    let n = if b.fine() { 16 } else { 8 };
    let mut depths = vec![OPEN, OPEN - 1.0];
    let mut z = OPEN - 1.0;
    while z > floor {
        z = (z - if z > -20.0 { 4.0 } else { 10.0 }).max(floor);
        depths.push(z);
    }
    let radius = |z: f32| mouth * (1.0 - 0.18 * (OPEN - z) / (OPEN - DEPTH[t]));
    let rings: Vec<Vec<Vec3>> = depths.iter().map(|&z| ring(n, radius(z), z)).collect();
    for (k, pair) in rings.windows(2).enumerate() {
        let (upper, lower) = (&pair[0], &pair[1]);
        seam(b);
        wall(b, upper, lower);
        if k > 0 && b.fine() && upper[0].z > -30.0 {
            // A band where the beam glassed the rock, just proud of the wall, as far down
            // as the eye can follow.
            let z = upper[0].z;
            let r = radius(z) - 0.06;
            b.paint(GLOW_LASER).pattern(pattern::NONE);
            wall(b, &ring(n, r, z + 0.15), &ring(n, r, z - 0.15));
        }
    }
    b.paint(GLOW_LAMP).pattern(pattern::NONE);
    b.face(rings.last().expect("the bore has a floor"));
    // The beam's core, down the middle to where it bites.
    b.cylinder_between(
        Vec3::Z * (OPEN - 0.05),
        Vec3::Z * (floor + 0.1),
        BEAM[t] * 0.3,
        BEAM[t] * 0.2,
        6,
    );
}

/// Quads down a shaft's wall from `upper` to `lower` (rings of the same count,
/// anticlockwise from above), facing in toward the middle.
fn wall(b: &mut MeshBuilder, upper: &[Vec3], lower: &[Vec3]) {
    let n = upper.len();
    for i in 0..n {
        let j = (i + 1) % n;
        b.face(&[lower[i], upper[i], upper[j], lower[j]]);
    }
}

/// The armoured collar round the mouth: a plated ring stepped down to the opening, clamp
/// blocks round it, its inner lip lit red.
fn collar_ring(b: &mut MeshBuilder, tech: u8) {
    let mouth = MOUTH[tech as usize - 1];
    let n = b.sides(16);
    let out = mouth + COLLAR_W;
    dark_plate(b);
    // Outer face, top and inner face down to the opening, one surface.
    let rings = [
        ring(n, out + 0.4, 0.0),
        ring(n, out, COLLAR_TOP),
        ring(n, mouth + 0.3, COLLAR_TOP),
        ring(n, mouth, OPEN),
    ];
    for pair in rings.windows(2) {
        let (a, c) = (&pair[0], &pair[1]);
        for i in 0..n {
            let j = (i + 1) % n;
            b.face(&[c[i], a[i], a[j], c[j]]);
        }
    }
    if b.fine() {
        seam(b);
        for k in 0..8 {
            let a = (22.5 + 45.0 * k as f32).to_radians();
            let d = v3(a.cos(), a.sin(), 0.0);
            b.beam(
                d * (mouth + 0.6) + Vec3::Z * COLLAR_TOP,
                d * (out + 0.4) + Vec3::Z * 0.45,
                Vec2::new(0.7, 0.5),
                Vec2::new(0.9, 0.4),
            );
        }
        b.paint(GLOW_LASER).pattern(pattern::NONE);
        wall(
            b,
            &ring(n, mouth + 0.02, OPEN + 0.25),
            &ring(n, mouth + 0.02, OPEN + 0.05),
        );
    }
}

/// One outrigger leg, standing out along +x (turned into place by the caller): a ribbed
/// bronze strut from the yoke down to its foot, plates lapped down it into spikes, a
/// clamp ram and a clawed pad on the ground (a pile down into the water afloat).
fn leg(b: &mut MeshBuilder) {
    let fine = b.fine();
    let (root, foot) = leg_line();
    ribbed(b, root, foot, 0.55, if fine { 3 } else { 0 });
    // Its plates, lapped down and out along the strut.
    let down = (foot - root).normalize();
    let up = v3(-down.z, 0.0, down.x);
    let f = Frame::new(root + up * 0.7 - down * 0.4, down, up);
    dark_plate(b);
    Course {
        count: 2,
        step: 3.8,
        len: 5.6,
        half: 1.5,
        tip: 0.0,
        thick: THICK,
        tail: 1.6,
    }
    .lay(b, &f);
    // The foot block the strut lands on, and the clamp rams either side of it working
    // down into the pad.
    dark_plate(b);
    b.block(v3(REACH - 3.0, -1.3, 1.2), v3(REACH, 1.3, 4.4));
    for y in [-1.75f32, 1.75] {
        piston(
            b,
            v3(REACH - 1.5, y, 4.2),
            v3(REACH - 1.5, y, 1.0),
            0.45,
            true,
        );
    }
    red_slot(b, v3(REACH + 0.02, 0.0, 3.2), Vec3::X, Vec3::Y, 1.6, 0.2);
    b.with_part(part::ASHORE, |b| {
        // The pad, its claws bitten into the ore.
        seam(b);
        b.block(v3(REACH - 3.4, -1.8, 0.0), v3(REACH + 0.4, 1.8, 1.2));
        if b.fine() {
            metal(b);
            for y in [-1.4f32, 0.0, 1.4] {
                b.beam(
                    v3(REACH + 0.2, y, 0.9),
                    v3(REACH + 1.3, y * 1.2, 0.12),
                    Vec2::new(0.45, 0.35),
                    Vec2::new(0.15, 0.15),
                );
            }
        }
    });
    b.with_part(part::AFLOAT, |b| {
        // A pile down from the foot block into the water.
        metal(b);
        let sides = b.sides(8);
        b.cylinder_between(
            v3(REACH - 1.5, 0.0, 0.0),
            v3(REACH - 1.5, 0.0, LIFT + 1.4),
            0.8,
            0.8,
            sides,
        );
    });
}

/// The yoke over the mouth and the emitter head hung in it: a plated body on four bronze
/// hangers, the owner's colour on its cap, red slots round it, and the bronze lens barrel
/// aimed down, its tip lit red.
fn head(b: &mut MeshBuilder) {
    let fine = b.fine();
    let sides = b.sides(12);
    dark_plate(b);
    let yoke_sides = b.sides(8);
    hoop(b, Vec3::Z * YOKE.0, YOKE.1, 1.3, 1.0, yoke_sides);
    metal(b);
    for k in 0..4 {
        let a = (90.0 * k as f32).to_radians();
        let d = v3(a.cos(), a.sin(), 0.0);
        b.cylinder_between(
            d * (YOKE.1 - 0.5) + Vec3::Z * YOKE.0,
            d * 1.3 + Vec3::Z * (YOKE.0 + 0.4),
            0.22,
            0.22,
            6,
        );
    }
    // The body: plated, swept plates lapped down its flanks.
    dark_plate(b);
    b.prism(Vec3::Z * BODY, sides, 1.5, 1.2, CAP_TOP - BODY - 0.5);
    for k in 0..4 {
        let a = (45.0 + 90.0 * k as f32).to_radians();
        let d = v3(a.cos(), a.sin(), 0.0);
        let f = Frame::new(
            d * 1.35 + Vec3::Z * (CAP_TOP - 0.6),
            d * 0.25 - Vec3::Z,
            d + Vec3::Z * 0.2,
        );
        dark_plate(b);
        Course {
            count: 2,
            step: 1.1,
            len: 1.6,
            half: 0.7,
            tip: 0.0,
            thick: 0.3,
            tail: 0.6,
        }
        .lay(b, &f);
        if fine {
            let e = a + 45f32.to_radians();
            let side = v3(e.cos(), e.sin(), 0.0);
            red_slot(
                b,
                side * 1.45 + Vec3::Z * (BODY + 0.9),
                side,
                Vec3::Z,
                0.8,
                0.16,
            );
        }
    }
    dark_plate(b);
    b.frustum(
        Vec3::Z * (CAP_TOP - 0.5),
        Vec2::splat(2.4),
        Vec2::splat(1.6),
        0.5,
        Vec2::ZERO,
    );
    b.paint(TEAM);
    b.face(&[
        v3(0.75, 0.0, CAP_TOP + 0.03),
        v3(0.0, 0.75, CAP_TOP + 0.03),
        v3(-0.75, 0.0, CAP_TOP + 0.03),
        v3(0.0, -0.75, CAP_TOP + 0.03),
    ]);
    // The lens barrel, aimed down the bore.
    collar(b, Vec3::Z * (BODY - 0.1), Vec3::Z, 0.9, 0.4);
    metal(b);
    b.cylinder_between(
        Vec3::Z * (BODY - 0.1),
        Vec3::Z * (EMIT + 0.3),
        0.6,
        0.45,
        sides,
    );
    b.paint(GLOW_LASER);
    b.cylinder_between(Vec3::Z * (EMIT + 0.3), Vec3::Z * EMIT, 0.45, 0.3, sides);
}

/// Tier `k`'s collector ring (it turns about the beam): a toothed bronze hoop, plated
/// carriages on it whose plates trail back against its turn, red slots facing in.
fn collector(b: &mut MeshBuilder, k: u8) {
    let fine = b.fine();
    let (z, r) = COLLECTORS[k as usize - 1];
    let c = Vec3::Z * z;
    metal(b);
    hoop(b, c, r, 0.6, 0.45, if fine { 24 } else { 10 });
    if fine {
        seam(b);
        teeth(b, c, r - 0.3, 12, v3(-0.3, 0.25, 0.35));
    }
    for q in 0..3 {
        let a = (120.0 * q as f32 + 40.0 * k as f32).to_radians();
        let d = v3(a.cos(), a.sin(), 0.0);
        // The ring turns anticlockwise from above: the plates trail clockwise.
        let back = v3(a.sin(), -a.cos(), 0.0);
        let at = c + d * r;
        let f = Frame::new(at - back * 0.6 + Vec3::Z * 0.3, back, Vec3::Z + d * 0.2);
        dark_plate(b);
        Course {
            count: 1,
            step: 0.0,
            len: 1.4,
            half: 0.5,
            tip: -0.5,
            thick: 0.25,
            tail: 0.6,
        }
        .lay(b, &f);
        red_slot(b, at - d * 0.32, -d, back, 0.8, 0.14);
    }
}

/// Tech 2's pinch emitter on a leg (along +x): a bronze barrel on a plated saddle, aimed
/// at the mouth, its tip lit red.
fn leg_pinch(b: &mut MeshBuilder) {
    let (mount, tip) = (leg_pinch_mount(), leg_pinch_tip());
    dark_plate(b);
    b.cuboid(mount - Vec3::Z * 0.6, v3(1.2, 1.3, 0.8));
    emitter(b, mount, tip, 0.28);
}

/// A pinch emitter from its mount to `tip`: a bronze collar, a bronze barrel, the tip
/// lit red.
fn emitter(b: &mut MeshBuilder, mount: Vec3, tip: Vec3, r: f32) {
    let d = (tip - mount).normalize();
    collar(b, mount, d, r * 1.8, 0.6);
    metal(b);
    b.cylinder_between(mount, tip - d * 0.25, r, r * 0.8, 8);
    b.paint(GLOW_LASER);
    b.cylinder_between(tip - d * 0.25, tip, r * 0.8, r * 0.35, 6);
}

/// Tech 2's focusing coils: bronze hoops stacked round the emitter body.
fn coils(b: &mut MeshBuilder) {
    let sides = b.sides(16);
    metal(b);
    let stack: &[f32] = if b.fine() {
        &[BODY + 0.5, BODY + 1.2, BODY + 1.9]
    } else {
        &[BODY + 1.2]
    };
    for &z in stack {
        hoop(b, Vec3::Z * z, 1.75, 0.35, 0.4, sides);
    }
}

/// Tech 2's crown: a plated strut up from each leg's root to a hub over the head, plates
/// lapped down off the hub, the owner's colour on its top.
fn crown(b: &mut MeshBuilder) {
    let sides = b.sides(8);
    for deg in LEGS {
        b.yawed(Vec3::ZERO, deg.to_radians(), |b| {
            let (root, _) = leg_line();
            dark_plate(b);
            b.beam(
                root + Vec3::Z * 0.6,
                v3(1.3, 0.0, HUB - 2.2),
                Vec2::new(0.9, 0.8),
                Vec2::new(0.7, 0.6),
            );
            let f = Frame::new(
                v3(1.2, 0.0, HUB - 0.3),
                v3(0.45, 0.0, -1.0),
                v3(1.0, 0.0, 0.45),
            );
            dark_plate(b);
            Course {
                count: 2,
                step: 1.2,
                len: 1.8,
                half: 0.8,
                tip: 0.0,
                thick: 0.3,
                tail: 0.7,
            }
            .lay(b, &f);
        });
    }
    ribbed(b, Vec3::Z * CAP_TOP, Vec3::Z * (HUB - 2.2), 0.5, 2);
    dark_plate(b);
    b.prism(Vec3::Z * (HUB - 2.4), sides, 1.6, 1.0, 2.4);
    b.paint(TEAM);
    b.face(&[
        v3(0.7, 0.0, HUB + 0.03),
        v3(0.0, 0.7, HUB + 0.03),
        v3(-0.7, 0.0, HUB + 0.03),
        v3(0.0, -0.7, HUB + 0.03),
    ]);
}

/// Tech 3's emitter post on a lot axis (along +x): a plated tower, its emitter aimed at
/// the mouth from its head, a red slot on its face.
fn post(b: &mut MeshBuilder) {
    let (x, z) = POST;
    seam(b);
    b.block(v3(x - 1.4, -1.4, 0.0), v3(x + 1.4, 1.4, 0.7));
    dark_plate(b);
    b.frustum_open(
        v3(x, 0.0, 0.7),
        Vec2::new(2.0, 1.8),
        Vec2::new(1.3, 1.2),
        z + 0.4,
        Vec2::new(-0.3, 0.0),
    );
    let f = Frame::new(
        v3(x + 0.6, 0.0, z + 0.9),
        v3(0.35, 0.0, -1.0),
        v3(1.0, 0.0, 0.3),
    );
    dark_plate(b);
    Course {
        count: 2,
        step: 1.6,
        len: 2.4,
        half: 1.0,
        tip: 0.0,
        thick: 0.35,
        tail: 0.8,
    }
    .lay(b, &f);
    emitter(b, v3(x - 0.6, 0.0, z), post_tip(), 0.3);
    red_slot(b, v3(x + 1.02, 0.0, 2.6), Vec3::X, Vec3::Z, 1.2, 0.18);
}

/// Tech 3's stage on the hub: a ribbed column with plates lapped down it, red slots
/// between them, the owner's colour on top.
fn stage(b: &mut MeshBuilder) {
    let sides = b.sides(8);
    ribbed(b, Vec3::Z * HUB, Vec3::Z * (STAGE - 0.8), 0.6, 3);
    for k in 0..4 {
        let a = (45.0 + 90.0 * k as f32).to_radians();
        let d = v3(a.cos(), a.sin(), 0.0);
        let f = Frame::new(
            d * 0.9 + Vec3::Z * (STAGE - 0.4),
            d * 0.2 - Vec3::Z,
            d + Vec3::Z * 0.15,
        );
        dark_plate(b);
        Course {
            count: if b.fine() { 3 } else { 1 },
            step: 1.1,
            len: 1.6,
            half: 0.65,
            tip: 0.0,
            thick: 0.3,
            tail: 0.6,
        }
        .lay(b, &f);
        let e = a + 45f32.to_radians();
        let side = v3(e.cos(), e.sin(), 0.0);
        red_slot(
            b,
            side * 0.66 + Vec3::Z * (STAGE - 2.2),
            side,
            Vec3::Z,
            1.2,
            0.15,
        );
    }
    dark_plate(b);
    b.prism(Vec3::Z * (STAGE - 0.8), sides, 1.1, 0.7, 0.8);
    b.paint(TEAM);
    b.face(&[
        v3(0.5, 0.0, STAGE + 0.03),
        v3(0.0, 0.5, STAGE + 0.03),
        v3(-0.5, 0.0, STAGE + 0.03),
        v3(0.0, -0.5, STAGE + 0.03),
    ]);
}

/// The deep core's capacitor drums at a foot (along +x): two bronze drums on a plated
/// cradle inside the foot block.
fn drums(b: &mut MeshBuilder) {
    if !b.fine() {
        return;
    }
    dark_plate(b);
    b.block(v3(REACH - 5.6, -1.6, 0.0), v3(REACH - 3.2, 1.6, 0.8));
    for y in [-0.8f32, 0.8] {
        collar(b, v3(REACH - 4.4, y, 1.7), Vec3::Z, 0.7, 1.8);
        seam(b);
        b.cylinder_between(
            v3(REACH - 4.4, y, 2.6),
            v3(REACH - 4.4, y, 2.9),
            0.5,
            0.4,
            8,
        );
    }
}

/// The deep core's shroud: plates lapped down over the crown's struts, all round.
fn shroud(b: &mut MeshBuilder) {
    for k in 0..8 {
        let a = (22.5 + 45.0 * k as f32).to_radians();
        let d = v3(a.cos(), a.sin(), 0.0);
        let f = Frame::new(
            d * 1.9 + Vec3::Z * (HUB - 1.0),
            d * 0.8 - Vec3::Z,
            d + Vec3::Z * 0.6,
        );
        dark_plate(b);
        Course {
            count: if b.fine() { 2 } else { 1 },
            step: 1.4,
            len: 2.0,
            half: 0.8,
            tip: 0.0,
            thick: 0.35,
            tail: 0.8,
        }
        .lay(b, &f);
    }
}

/// The deep core's wider lens: a broad red ring round the barrel's tip on a bronze
/// collar.
fn lens_ring(b: &mut MeshBuilder) {
    let sides = b.sides(16);
    collar(b, Vec3::Z * (EMIT + 0.6), Vec3::Z, 1.1, 0.5);
    b.paint(GLOW_LASER);
    hoop(b, Vec3::Z * (EMIT + 0.3), 0.95, 0.3, 0.2, sides);
}

#[cfg(test)]
mod tests {
    use crate::{build_model_scaled, material, part, rig, Model};

    /// The unit files' heights, by tier (tier 4 is drawn at tier 3's size).
    const HEIGHTS: [f32; 4] = [11.0, 15.0, 19.0, 19.0];

    fn built(tech: u8) -> Model {
        build_model_scaled("naga_taproot", 12.8, HEIGHTS[tech as usize - 1], tech).unwrap()
    }

    #[test]
    fn taproot() {
        for tech in 1..=4u8 {
            let h = HEIGHTS[tech as usize - 1];
            super::super::check_at("naga_taproot", tech, 12.8, h, Some(3), &[]);
            // Built on water it stands on piles, its grip on the ground and its bore left
            // out, and its clamp rams work.
            let model = built(tech);
            assert!(
                model.pit.is_some_and(|p| p.afloat_lift > 0.0),
                "no afloat lift"
            );
            for lod in 0..2 {
                for kind in [part::AFLOAT, part::ASHORE] {
                    assert!(
                        model.lods[lod].vertices.iter().any(|v| v.part == kind),
                        "tech {tech} lod {lod}: no part {kind}"
                    );
                }
            }
            for kind in [part::PUMP, part::SPINNER] {
                assert!(
                    model.lods[0].vertices.iter().any(|v| v.part == kind),
                    "tech {tech}: no part {kind}"
                );
            }
            // The next tier goes up in the refit; the deep core is the last.
            let refit = model.lods[0]
                .vertices
                .iter()
                .any(|v| v.rig & rig::UPGRADE != 0);
            assert_eq!(refit, tech < 4, "tech {tech}: refit pieces");
        }
    }

    /// The beam comes down out of the lens into the bore, and grows with every tier as the
    /// bore widens and deepens; from tech 2 pinch beams join it from lit emitter tips.
    #[test]
    fn the_beam_grows_every_tier() {
        let mut last = (0.0, 0.0, 0.0);
        for tech in 1..=4u8 {
            let model = built(tech);
            let pit = model.pit.unwrap();
            let beam = model.excavation.clone().expect("it digs with a beam");
            let deepest = model.lods[0]
                .vertices
                .iter()
                .map(|v| v.pos[2])
                .fold(f32::MAX, f32::min);
            assert!(beam.emitter[2] > pit.open + 3.0, "tech {tech}: emitter");
            assert!(
                beam.width > last.0 && pit.radius > last.1 && deepest < last.2,
                "tech {tech}: {} {} {deepest}",
                beam.width,
                pit.radius
            );
            last = (beam.width, pit.radius, deepest);
            assert_eq!(beam.pinches.len(), [0, 4, 8, 8][tech as usize - 1]);
            for p in &beam.pinches {
                let tip = glam::Vec3::from(*p);
                let lit = model.lods[0]
                    .vertices
                    .iter()
                    .filter(|v| v.material == material::GLOW_LASER)
                    .map(|v| glam::Vec3::from(v.pos).distance(tip))
                    .fold(f32::MAX, f32::min);
                assert!(lit < 0.3, "tech {tech}: no emitter tip near {tip}");
            }
            assert_eq!(beam.surge > 0.0, tech == 4);
        }
    }
}
