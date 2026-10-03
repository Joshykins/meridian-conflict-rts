//! The Excavator, the Regency mass extractor, on its 3 x 3 lot (36 m square): a bore cut by
//! a beam. It upgrades in place through four tiers, like ARC's core mine, each adding its
//! machinery to the last; the beam grows wider with every tier.
//!
//! Nothing strikes and nothing is driven down: a red-white excavation beam runs from the
//! emitter head straight down into the ground, cutting a glowing shaft, and the ore it
//! frees is drawn up the column round it (the beam, the converging pinch beams and the
//! rising ore are drawn by `renderer/regency_mine_fx.rs` from `Model::excavation`).
//!
//! - The bore: a low armoured collar round a funnel the beam has melted out of the ground,
//!   and the shaft down from it. Its walls are molten rock (`pattern::MOLTEN`): a black
//!   glass crust cracked with light at tech 1, running orange to white-hot at the deep
//!   core, hotter the deeper they go; white-hot at the bottom where the beam bites, its
//!   core running down the middle (`Model::pit`: the shader shows it through the ground).
//!   The funnel is wide and the collar low so the eye sees down into it.
//! - Four outrigger legs on the diagonals brace the rig against the ore field: bronze
//!   struts under plates lapped down and out into spikes, a foot block, a clawed pad
//!   gripping the ground. They carry a plated yoke over the mouth, and the emitter head hangs in it: a plated body,
//!   the owner's colour on its cap, a bronze lens barrel aimed down, its tip lit red.
//! - A collector ring floats round the beam over the mouth, turning (`part::SPINNER`),
//!   where the ore comes up.
//! - Tech 2: four pinch emitters on the legs aimed at the mouth, a second collector ring,
//!   focusing coils round the head, and a crown over it on four struts to a plated hub.
//!   Tech 3: four emitter posts on the lot's axes, a third ring, an armoured stage on the
//!   hub. Tech 4 (the deep core): plates lapped down over the crown, capacitor drums at
//!   every foot, a wider lens; its bore goes deepest and the beam surges.

use glam::{Vec2, Vec3};

use crate::builder::MeshBuilder;
use crate::material::*;
use crate::{part, pattern, Excavation, Pit};

use super::kit::{dark_plate, metal, seam, v3};
use super::machine::tier;
use super::machine::*;

/// The legs' bearings, and how far out their feet stand.
const LEGS: [f32; 4] = [45.0, 135.0, 225.0, 315.0];
const REACH: f32 = 12.8;
/// The bore's opening (`Pit::open`), and by tier: the funnel's radius there, the shaft's
/// radius where the funnel meets it, the depth of its floor, and the beam's width.
const OPEN: f32 = 1.6;
const FUNNEL: [f32; 4] = [4.0, 4.6, 5.2, 5.8];
const MOUTH: [f32; 4] = [2.4, 3.0, 3.6, 4.2];
const DEPTH: [f32; 4] = [-40.0, -60.0, -80.0, -120.0];
const BEAM: [f32; 4] = [0.7, 1.1, 1.6, 2.3];
/// Where the funnel narrows into the shaft.
const FUNNEL_FOOT: f32 = -4.0;
/// The collar round the funnel: how far it reaches past it, and its top. Low, so it hides
/// little of the funnel from the eye.
const COLLAR_W: f32 = 1.0;
const COLLAR_TOP: f32 = 2.0;
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
        radius: FUNNEL[t] + 0.1,
        stroke: 0.0,
        section: 0.0,
        rack: [0.0, 0.0],
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
    bore(b, tech);
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
    b.paint(GLOW_LASER).pattern(pattern::NONE);
    b.face(&ring(6, mouth, COLLAR_TOP + 0.02));
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

/// The bore at `tech`: a funnel from the opening down into the shaft, then the shaft to
/// its floor, in short lengths (the shader decides length by length whether the eye sees
/// it through the opening), its walls molten rock, a white-hot floor, and the beam's core
/// down the middle.
fn bore(b: &mut MeshBuilder, tech: u8) {
    let t = tech as usize - 1;
    // Below full detail only the top of the shaft, which is all the eye can follow.
    let (funnel, mouth, floor) = (FUNNEL[t], MOUTH[t], if b.fine() { DEPTH[t] } else { -24.0 });
    let n = if b.fine() { 16 } else { 8 };
    let mut depths = vec![OPEN, 0.8, -0.2, -1.4, -2.7, FUNNEL_FOOT];
    let mut z = FUNNEL_FOOT;
    while z > floor {
        z = (z - if z > -20.0 { 4.0 } else { 10.0 }).max(floor);
        depths.push(z);
    }
    // The funnel flares fastest at the top, so its wall lies back where the eye looks in;
    // the shaft narrows a little as it goes down.
    let radius = |z: f32| {
        if z >= FUNNEL_FOOT {
            let f = (z - FUNNEL_FOOT) / (OPEN - FUNNEL_FOOT);
            mouth + (funnel - mouth) * f * f
        } else {
            mouth * (1.0 - 0.18 * (FUNNEL_FOOT - z) / (FUNNEL_FOOT - DEPTH[t]))
        }
    };
    let rings: Vec<Vec<Vec3>> = depths.iter().map(|&z| ring(n, radius(z), z)).collect();
    b.paint(ACCENT).pattern(pattern::MOLTEN);
    for pair in rings.windows(2) {
        wall(b, &pair[0], &pair[1]);
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

/// The armoured collar round the funnel: a low plated ring stepped down to the opening,
/// clamp blocks round it, its inner lip lit red.
fn collar_ring(b: &mut MeshBuilder, tech: u8) {
    let mouth = FUNNEL[tech as usize - 1];
    let n = b.sides(16);
    let out = mouth + COLLAR_W;
    dark_plate(b);
    // Outer face, top and inner face down to the opening, one surface.
    let rings = [
        ring(n, out + 0.3, 0.0),
        ring(n, out, COLLAR_TOP),
        ring(n, mouth + 0.2, COLLAR_TOP),
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
                d * (mouth + 0.7) + Vec3::Z * (COLLAR_TOP - 0.15),
                d * (out + 0.3) + Vec3::Z * 0.3,
                Vec2::new(0.6, 0.35),
                Vec2::new(0.8, 0.3),
            );
        }
        b.paint(GLOW_LASER).pattern(pattern::NONE);
        wall(
            b,
            &ring(n, mouth + 0.03, OPEN + 0.2),
            &ring(n, mouth + 0.03, OPEN + 0.05),
        );
    }
}

/// One outrigger leg, standing out along +x (turned into place by the caller): a bronze
/// strut from the yoke down to its foot, plates lapped down it into spikes, a foot
/// block and a clawed pad on the ground.
fn leg(b: &mut MeshBuilder) {
    let (root, foot) = leg_line();
    shaft(b, root, foot, 0.55);
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
    // The foot block the strut lands on.
    dark_plate(b);
    b.block(v3(REACH - 3.0, -1.3, 1.2), v3(REACH, 1.3, 4.4));
    red_slot(b, v3(REACH + 0.02, 0.0, 3.2), Vec3::X, Vec3::Y, 1.6, 0.2);
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
    shaft(b, Vec3::Z * CAP_TOP, Vec3::Z * (HUB - 2.2), 0.5);
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

/// Tech 3's stage on the hub: a bronze column with plates lapped down it, red slots
/// between them, the owner's colour on top.
fn stage(b: &mut MeshBuilder) {
    let sides = b.sides(8);
    shaft(b, Vec3::Z * HUB, Vec3::Z * (STAGE - 0.8), 0.6);
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
        build_model_scaled("regency_taproot", 12.8, HEIGHTS[tech as usize - 1], tech).unwrap()
    }

    #[test]
    fn taproot() {
        for tech in 1..=4u8 {
            let h = HEIGHTS[tech as usize - 1];
            super::super::check_at("regency_taproot", tech, 12.8, h, Some(3), &[]);
            let model = built(tech);
            assert!(
                model.lods[0]
                    .vertices
                    .iter()
                    .any(|v| v.part == part::SPINNER),
                "tech {tech}: no spinner"
            );
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
