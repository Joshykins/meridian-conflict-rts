//! The Naga commander, a heavy armoured scorpion: eight plated legs on hydraulic rams, a
//! pair of great claws forward, a long jointed tail curled high over its back and ending
//! in a venom bulb with a twin stinger.
//!
//! Shape: a wedge of a head under an overhanging brow, a pinched neck collar, then a
//! broad abdomen of overlapping arched plates that climbs to a raised socket where the
//! tail plugs in. Every plate is black hide (`PLATING_DARK` under `pattern::EMBER`), the
//! soft hide between them lit red at its seams (`ACCENT` under `EMBER`).
//!
//! Rig: the body is `HULL`. Each leg is two bones posed by `entity.wgsl` `crawl_leg`
//! (`MeshBuilder::set_crawl_legs`), pair by pair. The pincers ride `rig::TAIL` on the hull
//! (`MeshBuilder::with_claw`): the arm swings about its shoulder, the inner finger opens
//! about its hinge (`claw_pose`). The tail is the turret: segment `i` turns about joint
//! `i` of `TAIL` (`tail_pose`), writhing, leaning in to strike and taking a share of the
//! aim; the bulb and the two prongs ride the last joint. The tail never swivels whole: its
//! top four joints bend round to the target (`AIM_SHARE`), within the unit file's
//! `aim_arc`, and past that the body turns. The sim turns the muzzles about the one point
//! that best matches that chain (`turret_at`). The prongs pitch about their joints:
//! the gun on the right (`ARM_GUN`), the fabricator on the left (`ARM_TOOL`). The numbers
//! match `data/factions/naga/units/command.ron`.
//!
//! Refits: `eng_2` puts a crown of fabricator hooks round the tool prong (they turn
//! while it builds), spinneret glands along the back and heavier plates on the claws;
//! `eng_3` adds a lance that runs out of the prong while it builds, banks of glands down
//! the spine and a ridged crest on the carapace.

use glam::{Vec2, Vec3};

use crate::models::builder::MeshBuilder;
use crate::models::material::*;
use crate::models::{part, pattern, rig};

use super::kit::*;

/// The stinger prongs' joints (left: the tool) and tips, as the unit file has them.
const PRONG_PIVOT: Vec3 = Vec3::new(-1.6, 0.95, 18.6);
const TOOL_TIP: Vec3 = Vec3::new(3.6, 0.95, 17.4);
const GUN_TIP: Vec3 = Vec3::new(4.2, -0.95, 17.3);
/// Suite III's lance at rest; it runs out 1.28 m while it builds (`rig::WORK_EXTEND`), to
/// the unit file's `arm_emitter` x of 6.9.
const LANCE_TIP: f32 = 5.62;

/// The tail's spine, root on the socket to where the venom bulb rides, and the middle of
/// its curl (for which way is "outside").
const TAIL: [Vec3; 10] = [
    Vec3::new(-6.4, 0.0, 6.9),
    Vec3::new(-8.8, 0.0, 8.0),
    Vec3::new(-10.7, 0.0, 9.7),
    Vec3::new(-11.9, 0.0, 11.8),
    Vec3::new(-12.2, 0.0, 14.1),
    Vec3::new(-11.6, 0.0, 16.3),
    Vec3::new(-10.3, 0.0, 18.0),
    Vec3::new(-8.6, 0.0, 19.1),
    Vec3::new(-6.8, 0.0, 19.6),
    Vec3::new(-5.1, 0.0, 19.5),
];
const CURL: Vec3 = Vec3::new(-7.6, 0.0, 13.2);
/// How the aim is shared over the tail's top joints, lowest first (`entity.wgsl`
/// `tail_pose`: the fourth joint from the top takes 1/10, the top one 4/10).
#[cfg(test)]
const AIM_SHARE: [f32; 4] = [0.1, 0.2, 0.3, 0.4];
/// The single point the stinger swings about that best matches that chain over the aim
/// arc: the unit file's `turret_at`, and the model's turret pivot.
const AIM_PIVOT: f32 = -6.8;
/// Half widths of the tail's armour rings at each joint, root to bulb.
const TAIL_WIDTH: [f32; 10] = [1.7, 1.62, 1.54, 1.46, 1.38, 1.3, 1.22, 1.14, 1.06, 1.0];

/// Left legs, front to back: hip, knee, the foot's tip, and where in the cycle it lifts.
/// Two alternating fours (front and third left with second and fourth right), a little
/// out of step so it scuttles rather than marches.
const LEGS: [(Vec3, Vec3, Vec3, f32); 4] = [
    (Vec3::new(4.8, 3.0, 4.9), Vec3::new(7.6, 5.5, 8.0), Vec3::new(10.8, 7.8, 0.0), 0.0),
    (Vec3::new(2.2, 3.4, 4.9), Vec3::new(3.5, 6.5, 8.5), Vec3::new(5.1, 10.0, 0.0), 0.5),
    (Vec3::new(-0.5, 3.4, 5.0), Vec3::new(-1.0, 6.6, 8.5), Vec3::new(-1.8, 10.2, 0.0), 0.08),
    (Vec3::new(-3.2, 3.2, 5.1), Vec3::new(-4.8, 6.1, 8.2), Vec3::new(-7.9, 8.8, 0.0), 0.58),
];

/// The left claw: shoulder, elbow, wrist, the palm's end, and the moving finger's hinge.
const SHOULDER: Vec3 = Vec3::new(9.0, 2.3, 4.9);
const ELBOW: Vec3 = Vec3::new(11.3, 4.9, 6.3);
const WRIST: Vec3 = Vec3::new(13.3, 4.7, 5.8);
const PALM: Vec3 = Vec3::new(16.2, 4.1, 5.4);
const JAW_HINGE: Vec3 = Vec3::new(16.0, 3.2, 5.3);

pub(super) fn commander(b: &mut MeshBuilder, _tech: u8) {
    b.set_crawl_legs(&LEGS, 8.0, 0.62, 1.6);
    b.set_tail(&TAIL, TAIL[TAIL.len() - 1].z - 0.4);
    b.set_claw(SHOULDER, JAW_HINGE);
    // Where the sim turns the stinger's muzzles (`turret_at` in the unit file). The
    // shader bends the tail's top joints instead of swivelling about it.
    b.set_turret_pivot(v3(AIM_PIVOT, 0.0, PRONG_PIVOT.z));
    b.set_arm_pivot(PRONG_PIVOT);
    b.set_dust_line(4.2);

    if b.coarse() {
        coarse(b);
        return;
    }
    body(b);
    b.mirror_y(|b| {
        for (i, &(hip, knee, foot, _)) in LEGS.iter().enumerate() {
            b.with_pair(i, |b| leg(b, hip, knee, foot, i));
        }
        claw(b);
    });
    tail(b);
    stinger(b);
}

/// Far off: a slab of a body, flat legs that do not walk, the tail in two bars.
fn coarse(b: &mut MeshBuilder) {
    hide(b);
    b.frustum(v3(1.0, 0.0, 3.6), Vec2::new(16.0, 7.0), Vec2::new(12.0, 4.4), 3.6, Vec2::new(0.6, 0.0));
    b.with_part(part::LOCOMOTION, |b| {
        b.paint(PLATING_DARK);
        b.mirror_y(|b| {
            for &(hip, knee, foot, _) in &LEGS {
                b.face(&[hip, foot, knee]);
                b.face(&[hip, knee, foot]);
            }
        });
    });
    b.mirror_y(|b| {
        b.with_claw(false, |b| {
            hide(b);
            b.face(&[SHOULDER, PALM + v3(2.2, -0.8, 0.0), ELBOW + Vec3::Z]);
            b.face(&[SHOULDER, ELBOW + Vec3::Z, PALM + v3(2.2, -0.8, 0.0)]);
        });
    });
    // The tail in two three-sided bars, keel out.
    let bar = |b: &mut MeshBuilder, a: Vec3, c: Vec3, wa: f32, wc: f32| {
        let (side, up) = frame(c - a, (a + c) * 0.5 - CURL);
        let tri = |p: Vec3, w: f32| vec![p + up * w, p + side * w - up * (w * 0.6), p - side * w - up * (w * 0.6)];
        b.loft(&[tri(a, wa), tri(c, wc)], true, true);
    };
    b.with_tail(0, |b| {
        hide(b);
        bar(b, TAIL[0], TAIL[4], 1.8, 1.5);
    });
    b.with_tail(4, |b| {
        hide(b);
        bar(b, TAIL[4], TAIL[9], 1.5, 1.1);
    });
    // The bulb and the suites as a few facets each, so a refit still shows from far off.
    b.with_part(part::TURRET, |b| {
        hide(b);
        let bulb = [v3(-5.4, 1.4, 19.2), v3(-5.4, -1.4, 19.2), v3(-5.4, 0.0, 21.0)];
        b.loft(&[bulb.to_vec(), vec![TOOL_TIP.with_y(0.0); 3]], true, false);
        b.paint(GLOW_LASER);
        b.module("eng_2", 0.3, |b| {
            b.loft(&[vec![v3(-2.0, 0.6, 19.4), v3(-2.0, 1.6, 19.4), v3(-2.0, 1.1, 20.4)], vec![v3(0.6, 1.1, 19.0); 3]], true, false);
        });
    });
    b.module("eng_3", 0.3, |b| {
        b.paint(GLOW_LASER);
        b.loft(&[vec![v3(-4.0, -0.5, 8.4), v3(-4.0, 0.5, 8.4), v3(-4.0, 0.0, 9.6)], vec![v3(4.0, 0.0, 7.6); 3]], true, false);
    });
}

/// The abdomen's plates, front to back: middle x, half length, half width, base height
/// and how high its arch stands. Widest behind the first legs, narrowing to the socket.
const TERGITES: [(f32, f32, f32, f32, f32); 6] = [
    (2.3, 0.8, 3.5, 6.3, 1.3),
    (0.5, 0.8, 4.3, 6.45, 1.5),
    (-1.3, 0.78, 4.6, 6.6, 1.6),
    (-3.0, 0.74, 4.3, 6.75, 1.5),
    (-4.6, 0.66, 3.6, 6.85, 1.3),
    (-6.0, 0.58, 2.8, 6.9, 1.05),
];

fn body(b: &mut MeshBuilder) {
    // The soft hide the plates ride on, its seams glowing: jaws to the tail's socket.
    under_hide(b);
    segment(
        b,
        &[
            (v3(11.8, 0.0, 4.4), 0.8, 0.45),
            (v3(10.4, 0.0, 4.85), 1.9, 0.95),
            (v3(7.8, 0.0, 5.2), 2.6, 1.25),
            (v3(5.2, 0.0, 5.3), 2.3, 1.2),
            (v3(3.5, 0.0, 5.3), 1.9, 1.05),
            (v3(1.8, 0.0, 5.6), 3.3, 1.6),
            (v3(-1.0, 0.0, 5.9), 4.1, 1.85),
            (v3(-3.8, 0.0, 6.2), 3.5, 1.75),
            (v3(-6.2, 0.0, 6.6), 2.3, 1.4),
            (v3(-7.2, 0.0, 6.9), 1.8, 1.2),
        ],
        Vec3::Z,
    );
    head(b);
    // The neck: a ringed collar of bare hide between head and abdomen.
    if b.fine() {
        for (k, x) in [4.2f32, 3.5, 2.8].into_iter().enumerate() {
            if k == 1 {
                metal(b);
            } else {
                under_hide(b);
            }
            b.cylinder_between(v3(x + 0.25, 0.0, 5.5), v3(x - 0.25, 0.0, 5.5), 2.4 - k as f32 * 0.1, 2.4 - k as f32 * 0.1, 10);
        }
    }
    abdomen(b);
    flanks(b);
    socket(b);
    belly(b);

    // The coxae: a heavy drum at each hip the leg turns in, capped with a plate.
    b.mirror_y(|b| {
        for &(hip, _, foot, _) in &LEGS {
            let out = (foot - hip).truncate().extend(0.0).normalize();
            under_hide(b);
            let sides = b.sides(10).min(10);
            b.cylinder_between(hip - out * 0.9, hip + out * 0.3, 1.25, 1.1, sides);
            if b.fine() {
                hide(b);
                shell(b, &[(hip - out * 0.8 + Vec3::Z * 0.55, 1.25, 0.65), (hip + out * 0.35 + Vec3::Z * 0.45, 1.15, 0.55)], Vec3::Z);
            }
        }
    });

    // Suite II: spinneret glands along the flanks of the back, lit.
    b.module("eng_2", 0.3, |b| {
        b.mirror_y(|b| {
            for x in [1.4f32, -0.4, -2.2] {
                under_hide(b);
                b.cylinder_between(v3(x + 0.6, 3.0, 7.7), v3(x - 0.7, 3.1, 7.95), 0.5, 0.36, 6);
                b.paint(GLOW_LASER);
                b.cylinder_between(v3(x - 0.7, 3.1, 7.95), v3(x - 0.95, 3.12, 8.0), 0.3, 0.12, 6);
            }
        });
    });
    // Suite III: a crest of blades down the spine and a gland bank either side of it.
    b.module("eng_3", 0.25, |b| {
        hide(b);
        for (i, x) in [8.4f32, 6.2, 2.6, 0.8, -1.0, -2.7, -4.3].into_iter().enumerate() {
            let tall = if i < 2 { 1.0 } else { 1.4 - i as f32 * 0.07 };
            let z = if i < 2 { 7.0 } else { TERGITES[(i - 2).min(5)].3 + TERGITES[(i - 2).min(5)].4 - 0.1 };
            spike(b, v3(x + 0.4, 0.0, z - 0.1), v3(x - 1.1, 0.0, z + tall), 0.34);
        }
        b.mirror_y(|b| {
            under_hide(b);
            segment(b, &[(v3(2.4, 1.9, 8.2), 0.45, 0.3), (v3(-4.0, 1.7, 8.6), 0.5, 0.36)], Vec3::Z);
            b.paint(GLOW_LASER);
            for x in [1.6f32, 0.2, -1.2, -2.6] {
                b.beam(v3(x, 1.85, 8.62 - x * 0.06), v3(x - 0.5, 1.83, 8.67 - x * 0.06), Vec2::new(0.3, 0.08), Vec2::new(0.24, 0.08));
            }
        });
    });
}

/// The head: a heavy wedge under an overhanging brow, horned, a cluster of red eyes in the
/// dark beneath the brow, the jaws below.
fn head(b: &mut MeshBuilder) {
    hide(b);
    // The carapace: broad and low, climbing from the brow to the neck.
    shell(
        b,
        &[
            (v3(12.2, 0.0, 5.1), 0.8, 0.3),
            (v3(11.0, 0.0, 5.75), 2.1, 0.7),
            (v3(8.6, 0.0, 6.2), 2.9, 1.0),
            (v3(5.8, 0.0, 6.3), 3.0, 1.0),
            (v3(3.8, 0.0, 6.05), 2.4, 0.8),
        ],
        Vec3::Z,
    );
    // The brow: a visor jutting over the eyes, and a raised crest behind it.
    shell(b, &[(v3(10.4, 0.0, 6.45), 1.7, 0.45), (v3(12.9, 0.0, 5.75), 0.9, 0.22)], Vec3::Z);
    if b.fine() {
        shell(b, &[(v3(10.2, 0.0, 7.0), 0.8, 0.5), (v3(7.8, 0.0, 7.35), 1.1, 0.7), (v3(5.0, 0.0, 7.2), 0.9, 0.55)], Vec3::Z);
    }
    // Cheek plates hanging over the jaws' roots, flared out.
    b.mirror_y(|b| {
        hide(b);
        let hint = v3(0.0, 1.0, 0.7);
        shell(b, &[(v3(10.6, 1.8, 5.0), 0.85, 0.35), (v3(8.2, 2.6, 5.35), 1.2, 0.5), (v3(5.4, 2.85, 5.45), 1.05, 0.45)], hint);
        if b.fine() {
            // Horns off the brow, swept back.
            blade(b, v3(10.0, 1.4, 6.5), v3(7.0, 2.4, 8.9), 0.42, v3(0.0, 1.0, 0.2));
            blade(b, v3(7.2, 2.6, 6.5), v3(5.0, 3.5, 7.9), 0.34, v3(0.0, 1.0, 0.2));
            // Twin crest ridges down the carapace.
            hide(b);
            b.beam(v3(9.8, 1.3, 6.55), v3(4.6, 1.7, 7.05), Vec2::new(0.3, 0.36), Vec2::new(0.36, 0.28));
            // A lit seam along the cheek.
            b.paint(GLOW_LASER);
            b.beam(v3(9.6, 2.3, 4.85), v3(6.2, 3.0, 5.0), Vec2::new(0.1, 0.07), Vec2::new(0.1, 0.07));
        }
    });
    // Eyes: in the dark under the brow's lip, the pair in the middle biggest.
    b.paint(GLOW_LASER);
    b.mirror_y(|b| {
        b.beam(v3(11.75, 0.35, 5.2), v3(11.95, 0.9, 5.05), Vec2::new(0.3, 0.2), Vec2::new(0.24, 0.16));
        if b.fine() {
            b.beam(v3(11.3, 1.25, 5.05), v3(11.4, 1.6, 4.95), Vec2::new(0.2, 0.14), Vec2::new(0.16, 0.12));
            b.beam(v3(10.7, 1.85, 5.0), v3(10.75, 2.1, 4.9), Vec2::new(0.16, 0.12), Vec2::new(0.14, 0.1));
            b.beam(v3(9.2, 2.45, 6.05), v3(9.6, 2.35, 5.95), Vec2::new(0.16, 0.12), Vec2::new(0.12, 0.1));
        }
    });
    // The jaws: two hooked, segmented chelicerae under the brow.
    b.mirror_y(|b| {
        hide(b);
        segment(b, &[(v3(10.8, 0.75, 4.25), 0.55, 0.5), (v3(12.3, 0.72, 4.05), 0.5, 0.45), (v3(13.2, 0.55, 3.8), 0.3, 0.3)], Vec3::Z);
        if b.fine() {
            b.paint(METAL).pattern(pattern::EMBER);
            b.cylinder_between(v3(13.0, 0.6, 3.85), v3(13.9, 0.2, 3.25), 0.26, 0.04, 5);
            knuckle(b, v3(12.3, 0.72, 4.05), Vec3::Y, 0.36, 0.95);
        }
    });
    // The owner's colour: a chevron on the brow.
    b.paint(TEAM);
    b.mirror_y(|b| b.beam(v3(7.0, 1.15, 7.2), v3(5.2, 2.5, 6.95), Vec2::new(0.4, 0.07), Vec2::new(0.4, 0.07)));
}

/// Half of one plate of the abdomen (the left), a cross-section at `x`: an arch from the
/// spine's edge out over the flank, its rim hanging past the hide, hollowed under, swept
/// back `sweep` metres at the spine so the plates read as chevrons.
fn tergite_ring(b: &MeshBuilder, x: f32, w: f32, z: f32, h: f32, sweep: f32) -> Vec<Vec3> {
    let shape: &[[f32; 2]] = if b.fine() {
        &[[1.0, -0.72], [0.78, -0.3], [0.2, 0.72], [0.17, 0.95], [0.4, 0.9], [0.72, 0.6], [0.97, 0.0]]
    } else {
        &[[1.0, -0.72], [0.2, 0.72], [0.2, 0.95], [0.75, 0.6]]
    };
    shape.iter().map(|&[s, u]| v3(x - sweep * (1.0 - s.abs()), s * w, z + u * h)).collect()
}

/// The abdomen: a bare, ribbed spine down the middle, and on either side of it domed
/// chevron plates, each apart from the next so the working hide shows between them, rims
/// hanging over the flanks. Vertebrae ride the spine between the plates.
fn abdomen(b: &mut MeshBuilder) {
    let sweep = 0.7;
    for (i, &(x, half, w, z, h)) in TERGITES.iter().enumerate() {
        b.mirror_y(|b| {
            hide(b);
            b.loft(
                &[
                    tergite_ring(b, x + half, w * 0.92, z - 0.1, h * 0.9, sweep),
                    tergite_ring(b, x, w, z, h, sweep),
                    tergite_ring(b, x - half, w * 0.97, z + 0.12, h * 1.02, sweep),
                ],
                true,
                true,
            );
        });
        let top = z + h;
        // The vertebra on the spine under the plate's middle, keeled, a blade raked back.
        metal(b);
        b.beam(v3(x + half * 0.9 - sweep, 0.0, top - 0.45), v3(x - half * 0.9 - sweep, 0.0, top - 0.3), Vec2::new(0.62, 0.55), Vec2::new(0.62, 0.6));
        if !b.fine() {
            continue;
        }
        hide(b);
        blade(b, v3(x - sweep, 0.0, top), v3(x - half - 1.6 - sweep * 0.5, 0.0, top + 1.0 - i as f32 * 0.06), 0.4, Vec3::Y);
        b.mirror_y(|b| {
            // A lit seam down each side of the spine.
            b.paint(GLOW_LASER);
            b.beam(v3(x + half * 0.7 - sweep, 0.5, top - 0.55), v3(x - half * 0.7 - sweep, 0.5, top - 0.45), Vec2::new(0.08, 0.08), Vec2::new(0.08, 0.08));
            // The flank plate hanging off the rim over the legs, a barb at its back corner.
            hide(b);
            let rim = v3(x, w * 0.97, z - 0.72 * h + 0.25);
            b.loft(
                &[
                    vec![rim + v3(half, 0.0, 0.0), rim + v3(half * 0.95, 0.45, -1.5), rim + v3(half * 0.95, 0.15, -1.6), rim + v3(half, -0.3, 0.0)],
                    vec![rim + v3(-half, 0.0, 0.1), rim + v3(-half * 1.05, 0.5, -1.6), rim + v3(-half * 1.05, 0.2, -1.7), rim + v3(-half, -0.3, 0.1)],
                ],
                true,
                true,
            );
            spike(b, rim + v3(-half * 0.6, 0.4, -1.25), rim + v3(-half - 1.1, 1.05, -1.35), 0.22);
            // Two lit gill slits on the rear plates' flanks.
            if i >= 2 {
                b.paint(GLOW_LASER);
                for k in 0..2 {
                    let at = rim + v3(half * 0.6 - k as f32 * half * 0.9, 0.52, -0.8);
                    b.beam(at, at - v3(half * 0.5, 0.0, -0.12), Vec2::new(0.08, 0.08), Vec2::new(0.08, 0.05));
                }
            }
            // A blade on the plate's shoulder, raked back and out.
            hide(b);
            let shoulder = v3(x - sweep * 0.3, w * 0.7, z + 0.6 * h);
            blade(b, shoulder, shoulder + v3(-half - 1.1, w * 0.25, 1.0), 0.3, v3(0.0, 1.0, -0.4));
        });
        // A rib of bare metal across the gap behind the plate.
        if i + 1 < TERGITES.len() {
            let gx = (x - half + TERGITES[i + 1].0 + TERGITES[i + 1].1) * 0.5 - sweep * 0.5;
            metal(b);
            b.mirror_y(|b| cable(b, &[v3(gx, 0.3, top - 0.5), v3(gx, w * 0.5, z + h * 0.55), v3(gx, w * 0.85, z + h * 0.05)], 0.16));
        }
    }
    // The owner's colour: a chevron across the widest plate.
    if b.fine() {
        b.paint(TEAM);
        let (x, _, w, z, h) = TERGITES[2];
        b.mirror_y(|b| {
            b.beam(v3(x - 0.62, 0.55, z + h * 0.98), v3(x - 0.2, w * 0.4, z + h * 0.88), Vec2::new(0.4, 0.07), Vec2::new(0.4, 0.07))
        });
    }
}

/// The working flanks: cable runs from the head back to the socket under the plates'
/// rims, a ram from the body down to each hip, rams across the neck.
fn flanks(b: &mut MeshBuilder) {
    if !b.fine() {
        return;
    }
    b.mirror_y(|b| {
        metal(b);
        for (dy, dz) in [(0.0f32, 0.0f32), (0.25, 0.32)] {
            cable(
                b,
                &[
                    v3(9.0, 2.0 + dy, 5.0 + dz),
                    v3(5.6, 2.2 + dy, 5.2 + dz),
                    v3(3.4, 1.8 + dy, 5.3 + dz),
                    v3(1.2, 3.0 + dy, 5.7 + dz),
                    v3(-2.0, 3.4 + dy, 6.0 + dz),
                    v3(-5.0, 2.8 + dy, 6.3 + dz),
                    v3(-6.9, 1.8 + dy, 6.7 + dz),
                ],
                0.13,
            );
        }
        for &(hip, _, _, _) in &LEGS {
            ram(b, v3(hip.x - 0.7, 1.9, 6.6), hip + v3(0.3, -0.1, 1.05), 0.26);
        }
        // Across the neck, head to the first plate.
        ram(b, v3(5.2, 1.5, 6.1), v3(2.9, 2.0, 6.5), 0.24);
        ram(b, v3(5.0, 2.1, 5.2), v3(2.7, 2.6, 5.5), 0.22);
    });
}

/// The raised socket at the back the tail plugs into: an armoured collar on two rams.
fn socket(b: &mut MeshBuilder) {
    let root = TAIL[0];
    hide(b);
    let sides = b.sides(12).min(12);
    b.cylinder_between(root + v3(1.7, 0.0, -0.9), root + v3(-0.2, 0.0, 0.1), 2.35, 2.05, sides);
    if b.fine() {
        under_hide(b);
        b.cylinder_between(root + v3(-0.2, 0.0, 0.1), root + v3(-0.55, 0.0, 0.3), 1.85, 1.85, 12);
        b.mirror_y(|b| {
            ram(b, v3(-3.4, 2.1, 7.6), root + v3(0.2, 1.9, 0.7), 0.34);
            hide(b);
            blade(b, root + v3(0.8, 1.9, 0.4), root + v3(-1.4, 2.9, 1.6), 0.36, v3(0.0, 1.0, 0.3));
        });
    }
}

/// Under it all: ribbed belly plates.
fn belly(b: &mut MeshBuilder) {
    under_hide(b);
    b.frustum(v3(1.0, 0.0, 3.3), Vec2::new(14.0, 4.4), Vec2::new(16.0, 6.0), 1.2, Vec2::ZERO);
    if b.fine() {
        hide(b);
        for x in [6.4f32, 3.6, 0.8, -2.0, -4.6] {
            b.beam(v3(x + 0.9, 0.0, 3.35), v3(x - 0.9, 0.0, 3.4), Vec2::new(5.2, 0.4), Vec2::new(5.4, 0.4));
        }
    }
}

/// One left leg of pair `pair`: a heavy armoured thigh on a ram up to a spiked knee, a
/// plated shin down to a clawed foot, red light in the seams.
fn leg(b: &mut MeshBuilder, hip: Vec3, knee: Vec3, foot: Vec3, pair: usize) {
    let out = (foot - hip).truncate().extend(0.0).normalize();
    // Toward the outside of the leg's bend, for the keels.
    let outside = Vec3::Z + out * 0.4;
    let heft = if pair == 0 || pair == 3 { 1.0 } else { 1.08 };
    let thigh = knee - hip;
    let shin = foot - knee;
    let ankle = knee + shin * 0.66;
    b.with_part(part::LOCOMOTION, |b| {
        b.with_limb(rig::THIGH, |b| {
            under_hide(b);
            segment(b, &[(hip, 0.95 * heft, 1.0 * heft), (knee - thigh * 0.05, 0.75 * heft, 0.8 * heft)], outside);
            // The armour over it: an arched plate along the top, keeled.
            hide(b);
            shell(
                b,
                &[
                    (hip + thigh * 0.12 + outside.normalize() * 0.35, 1.15 * heft, 0.85 * heft),
                    (hip + thigh * 0.55 + outside.normalize() * 0.4, 1.12 * heft, 0.9 * heft),
                    (knee - thigh * 0.08 + outside.normalize() * 0.3, 0.92 * heft, 0.7 * heft),
                ],
                outside,
            );
            if b.fine() {
                // The ram under the thigh that lifts it.
                let (_, up) = frame(thigh, outside);
                ram(b, hip + thigh * 0.1 - up * 0.95, knee - thigh * 0.18 - up * 0.75, 0.3);
                // A red seam along the plate's flank.
                b.paint(GLOW_LASER);
                let (side, _) = frame(thigh, outside);
                b.beam(hip + thigh * 0.25 + side * 1.05 * heft, hip + thigh * 0.8 + side * 0.9 * heft, Vec2::new(0.1, 0.1), Vec2::new(0.08, 0.08));
                // A blade over the knee, raked back.
                hide(b);
                blade(b, knee - thigh.normalize() * 0.7 + Vec3::Z * 0.6, knee + Vec3::Z * 2.4 - shin.normalize() * 0.6 - Vec3::X * 0.7, 0.4, out);
            }
        });
        b.with_limb(rig::SHIN, |b| {
            knuckle(b, knee, out.cross(Vec3::Z), 0.92 * heft, 1.7 * heft);
            under_hide(b);
            segment(b, &[(knee + shin * 0.03, 0.72 * heft, 0.8 * heft), (ankle, 0.5, 0.55)], out + Vec3::Z);
            hide(b);
            // The shin's armour, on its outer face.
            shell(
                b,
                &[(knee + shin * 0.08, 0.95 * heft, 0.75 * heft), (knee + shin * 0.4, 0.85 * heft, 0.68), (ankle - shin * 0.04, 0.6, 0.45)],
                out + Vec3::Z,
            );
            // The foot: a heavy hooked claw driven into the ground, a spur behind it.
            under_hide(b);
            knuckle(b, ankle, out.cross(Vec3::Z), 0.5, 0.95);
            hide(b);
            segment(b, &[(ankle, 0.5, 0.52), (ankle.lerp(foot, 0.55), 0.36, 0.4), (foot + Vec3::Z * 0.06, 0.05, 0.05)], out + Vec3::Z);
            if b.fine() {
                let back = -out * 0.9 + Vec3::Z * 0.2;
                spike(b, ankle + Vec3::Z * -0.2, ankle.lerp(foot, 0.7) + back + Vec3::Z * 0.1, 0.24);
                ram(b, knee + shin * 0.12 - out * 0.7, ankle - out * 0.45 + Vec3::Z * 0.2, 0.22);
                b.paint(GLOW_LASER);
                let (_, up) = frame(shin, out + Vec3::Z);
                b.beam(knee + shin * 0.14 + up * 0.72, knee + shin * 0.55 + up * 0.55, Vec2::new(0.14, 0.09), Vec2::new(0.1, 0.07));
            }
        });
    });
}

/// The left claw on its arm: a swollen, spiked palm, a fixed outer finger, and the inner
/// finger that bites (`rig::CLAW_JAW`), both toothed, a red bite between them.
fn claw(b: &mut MeshBuilder) {
    let tip_fixed = v3(18.7, 3.5, 5.2);
    let tip_jaw = v3(18.3, 2.75, 5.05);
    b.with_claw(false, |b| {
        under_hide(b);
        segment(b, &[(SHOULDER, 0.85, 0.85), (ELBOW, 0.7, 0.72)], Vec3::Z);
        segment(b, &[(ELBOW, 0.72, 0.72), (WRIST, 0.85, 0.85)], Vec3::Z);
        hide(b);
        shell(b, &[(SHOULDER.lerp(ELBOW, 0.1) + Vec3::Z * 0.35, 0.95, 0.75), (SHOULDER.lerp(ELBOW, 0.9) + Vec3::Z * 0.3, 0.85, 0.65)], Vec3::Z);
        shell(b, &[(ELBOW.lerp(WRIST, 0.12) + Vec3::Z * 0.3, 0.88, 0.68), (ELBOW.lerp(WRIST, 0.92) + Vec3::Z * 0.35, 1.0, 0.75)], Vec3::Z);
        knuckle(b, ELBOW, Vec3::Z, 0.75, 1.4);
        knuckle(b, WRIST, v3(0.3, 1.0, 0.0), 0.8, 1.2);
        // The palm: swollen, its outer face plated, spikes along its top.
        hide(b);
        segment(
            b,
            &[(WRIST, 0.95, 0.9), (WRIST.lerp(PALM, 0.3), 1.7, 1.3), (WRIST.lerp(PALM, 0.7), 1.6, 1.2), (PALM, 1.1, 0.95)],
            Vec3::Z,
        );
        // The fixed finger.
        segment(b, &[(PALM + v3(-0.3, 0.35, 0.15), 0.62, 0.62), (v3(17.6, 4.15, 5.35), 0.46, 0.5), (tip_fixed, 0.06, 0.1)], Vec3::Z);
        if b.fine() {
            ram(b, SHOULDER + v3(0.4, -0.3, -0.7), ELBOW + v3(-0.5, -0.4, -0.6), 0.26);
            ram(b, ELBOW + v3(0.3, -0.5, -0.5), WRIST + v3(-0.4, -0.6, -0.4), 0.24);
            // Spikes along the palm's top, in the darker hide (hide on a blade this thin
            // catches the sun as a white sliver).
            under_hide(b);
            for k in 0..3 {
                let at = WRIST.lerp(PALM, 0.2 + k as f32 * 0.25) + v3(0.0, 0.3, 1.1 - k as f32 * 0.05);
                spike(b, at, at + v3(-0.9, 0.3, 0.95 - k as f32 * 0.15), 0.3);
            }
            // Teeth inside the fixed finger.
            under_hide(b);
            for k in 0..3 {
                let at = PALM.lerp(tip_fixed, 0.25 + k as f32 * 0.22) + v3(0.0, -0.35, 0.0);
                spike(b, at, at + v3(0.25, -0.45, -0.05), 0.14);
            }
            b.paint(TEAM);
            b.beam(WRIST.lerp(PALM, 0.2) + v3(0.0, 1.55, 0.2), WRIST.lerp(PALM, 0.75) + v3(0.0, 1.45, 0.2), Vec2::new(0.08, 0.5), Vec2::new(0.08, 0.45));
            b.paint(GLOW_LASER);
            b.beam(WRIST.lerp(PALM, 0.35) + v3(0.0, -1.62, -0.1), WRIST.lerp(PALM, 0.85) + v3(0.0, -1.2, -0.1), Vec2::new(0.1, 0.12), Vec2::new(0.08, 0.1));
        }
        // Suite II: heavier plates over the palm.
        b.module("eng_2", 0.55, |b| {
            hide(b);
            shell(b, &[(WRIST.lerp(PALM, 0.2) + v3(0.0, 0.2, 1.0), 1.4, 0.45), (WRIST.lerp(PALM, 0.85) + v3(0.0, 0.1, 0.9), 1.1, 0.4)], Vec3::Z);
        });
        // Suite III: a ridge of blades along the arm.
        b.module("eng_3", 0.5, |b| {
            hide(b);
            spike(b, ELBOW + v3(-0.5, -0.1, 0.6), ELBOW + v3(-1.8, 0.4, 2.3), 0.34);
            spike(b, WRIST + v3(0.6, 0.0, 0.8), WRIST + v3(-0.2, 0.3, 2.2), 0.3);
        });
    });
    // The moving finger, hinged inside the palm.
    b.with_claw(true, |b| {
        hide(b);
        segment(b, &[(JAW_HINGE + v3(-0.4, 0.1, 0.0), 0.55, 0.55), (v3(17.3, 2.7, 5.2), 0.45, 0.46), (tip_jaw, 0.06, 0.1)], Vec3::Z);
        if b.fine() {
            knuckle(b, JAW_HINGE, Vec3::Z, 0.45, 1.0);
            under_hide(b);
            for k in 0..3 {
                let at = JAW_HINGE.lerp(tip_jaw, 0.3 + k as f32 * 0.2) + v3(0.0, 0.3, 0.0);
                spike(b, at, at + v3(0.25, 0.42, -0.05), 0.13);
            }
            b.paint(GLOW_LASER);
            b.beam(JAW_HINGE + v3(0.3, 0.42, 0.0), tip_jaw + v3(-0.5, 0.2, 0.0), Vec2::new(0.08, 0.1), Vec2::new(0.06, 0.08));
        }
    });
}

/// The tail: nine segments from the socket to the venom bulb, each a working joint of the
/// machine, not a sleeve: a bare ribbed core turning on a drum, an arched plate over the
/// outside of the curl, a separate pad on each flank, a belly plate, a ram along each side
/// and cables down the inside. Every plate stops short of both joints, so each joint shows
/// as a gap with the core and its red seams in it.
fn tail(b: &mut MeshBuilder) {
    for i in 0..TAIL.len() - 1 {
        let (a, c) = (TAIL[i], TAIL[i + 1]);
        let (wa, wc) = (TAIL_WIDTH[i], TAIL_WIDTH[i + 1]);
        let dir = c - a;
        let along = dir.normalize();
        let mid = (a + c) * 0.5;
        let outward = (mid - CURL).normalize();
        let (side, up) = frame(dir, outward);
        let (p0, p1) = (a + dir * 0.17, c - dir * 0.1);
        b.with_tail(i, |b| {
            // The core: bare and narrow, lit at its seams, turning on a drum at its joint.
            under_hide(b);
            segment(b, &[(a, wa * 0.5, wa * 0.5), (c, wc * 0.5, wc * 0.5)], outward);
            knuckle(b, a, Vec3::Y, wa * 0.64, wa * 1.9);
            // The arch over the outside of the curl, lifted at its back edge.
            hide(b);
            let arch = |p: Vec3, w: f32, k: f32| (p + up * (w * 0.4), w * 1.0 * k, w * 0.72 * k);
            if b.fine() {
                shell(b, &[arch(p0, wa, 0.88), arch(a.lerp(c, 0.55), wa, 1.0), arch(p1, wc, 1.1)], outward);
            } else {
                shell(b, &[arch(p0, wa, 0.9), arch(p1, wc, 1.05)], outward);
            }
            if b.coarse() {
                return;
            }
            // The crest blade on the arch, raked back toward the root.
            let top = mid + up * (wa * 1.1);
            blade(b, top + along * 0.5, top + up * (1.4 + wa * 0.4) - along * 1.2, 0.5, side);
            // The belly plate on the inside of the curl.
            let belly = |p: Vec3, w: f32| (p - up * (w * 0.62), w * 0.55, w * 0.3);
            shell(b, &[belly(a + dir * 0.26, wa), belly(c - dir * 0.2, wc)], -up);
            if !b.fine() {
                return;
            }
            for s in [-1.0f32, 1.0] {
                // A pad on each flank, standing off the core under the arch's rim.
                hide(b);
                let at = |p: Vec3, w: f32| p + side * (s * w * 0.98) - up * (w * 0.2);
                let (f0, f1) = (at(a + dir * 0.28, wa), at(c - dir * 0.22, wc));
                slab(
                    b,
                    [f0 - up * (wa * 0.34), f0 + up * (wa * 0.3), f1 + up * (wc * 0.34), f1 - up * (wc * 0.3)],
                    side * (s * 0.22),
                );
                // A side blade off the pad, fanned out and back.
                let root = (f0 + f1) * 0.5 + up * (wa * 0.2);
                blade(b, root + along * 0.4, root + (up * 0.7 + side * s).normalize() * 1.3 - along * 1.0, 0.34, up);
                // The ram that bends this joint, along the flank inside the pad.
                ram(b, a + dir * 0.1 + side * (s * wa * 0.62) + up * (wa * 0.05), c - dir * 0.16 + side * (s * wc * 0.62) + up * (wc * 0.05), 0.15 + wa * 0.05);
                // A red seam in the gap between the arch and the pad.
                b.paint(GLOW_LASER);
                let seam = a + dir * 0.34 + side * (s * wa * 0.86) + up * (wa * 0.2);
                b.beam(seam, seam + dir * 0.4, Vec2::new(0.08, 0.08), Vec2::new(0.08, 0.08));
                // Cables down the inside of the curl.
                metal(b);
                cable(b, &[a + dir * 0.02 + side * (s * wa * 0.26) - up * (wa * 0.44), c - dir * 0.02 + side * (s * wc * 0.26) - up * (wc * 0.44)], 0.1);
            }
            // A red line down the belly plate.
            b.paint(GLOW_LASER);
            b.beam(a + dir * 0.3 - up * (wa * 0.93), c - dir * 0.25 - up * (wc * 0.9), Vec2::new(0.18, 0.08), Vec2::new(0.14, 0.08));
        });
    }
}

/// The venom bulb on the tail's tip, and its two prongs: gun on the right, fabricator on the left.
fn stinger(b: &mut MeshBuilder) {
    b.with_part(part::TURRET, |b| {
        let root = TAIL[TAIL.len() - 1];
        knuckle(b, root, Vec3::Y, 0.85, 1.7);
        // The bulb: swollen, plated over the top, lit sacs down its flanks.
        under_hide(b);
        segment(
            b,
            &[(root, 0.95, 0.9), (v3(-3.8, 0.0, 19.8), 1.7, 1.5), (v3(-2.3, 0.0, 19.5), 1.55, 1.3), (v3(-0.9, 0.0, 18.8), 0.8, 0.7)],
            Vec3::Z,
        );
        hide(b);
        shell(b, &[(v3(-4.6, 0.0, 20.25), 1.55, 1.05), (v3(-3.2, 0.0, 20.35), 1.75, 1.1), (v3(-1.7, 0.0, 19.9), 1.3, 0.85)], Vec3::Z);
        b.paint(GLOW_LASER);
        b.beam(v3(-2.4, 0.0, 21.2), v3(-1.7, 0.0, 20.85), Vec2::new(0.3, 0.12), Vec2::new(0.2, 0.1));
        if b.fine() {
            b.mirror_y(|b| {
                b.paint(GLOW_LASER);
                b.beam(v3(-4.0, 1.66, 19.4), v3(-2.2, 1.5, 19.25), Vec2::new(0.1, 0.14), Vec2::new(0.1, 0.1));
                hide(b);
                blade(b, v3(-3.6, 1.2, 20.6), v3(-5.6, 2.3, 21.4), 0.34, v3(0.0, 1.0, 0.3));
            });
            hide(b);
            spike(b, v3(-3.9, 0.0, 21.2), v3(-6.2, 0.0, 22.6), 0.42);
        }

        // The gun prong: a long barbed needle sheathed in hide, a red ring at the muzzle.
        b.with_limb(rig::ARM_GUN, |b| prong(b, PRONG_PIVOT * v3(1.0, -1.0, 1.0), GUN_TIP, true));

        // The fabricator prong: a hooked needle, its tip lit.
        b.with_limb(rig::ARM_TOOL, |b| {
            let (pivot, tip) = (PRONG_PIVOT, TOOL_TIP);
            prong(b, pivot, tip, false);

            // Suite II: a crown of three fabricator hooks round the prong, turning while it builds.
            b.module("eng_2", 0.3, |b| {
                b.with_work(rig::WORK_TWIST, |b| {
                    let axis = v3(0.0, pivot.y, pivot.z);
                    for k in 0..3 {
                        let a = k as f32 * std::f32::consts::TAU / 3.0 + 0.5;
                        let r = Vec3::new(0.0, a.cos(), a.sin());
                        under_hide(b);
                        b.cylinder_between(axis + v3(-0.2, 0.0, 0.0) + r * 0.6, axis + v3(1.2, 0.0, 0.0) + r * 1.0, 0.15, 0.12, 5);
                        b.paint(GLOW_LASER);
                        b.cylinder_between(axis + v3(1.2, 0.0, 0.0) + r * 1.0, axis + v3(1.9, 0.0, 0.0) + r * 0.58, 0.12, 0.03, 5);
                    }
                    b.paint(METAL).pattern(pattern::EMBER);
                    b.cylinder_between(axis + v3(-0.45, 0.0, 0.0), axis + v3(-0.1, 0.0, 0.0), 0.82, 0.82, b.sides(10).min(10));
                });
            });
            // Suite III: the lance, sleeved under the prong; it runs out while it builds.
            b.module("eng_3", 0.3, |b| {
                let (y, z0, z1) = (tip.y, 17.85, 17.45);
                hide(b);
                b.beam(v3(0.2, y, z0 + 0.1), v3(2.8, y, z1 + 0.25), Vec2::new(0.5, 0.36), Vec2::new(0.4, 0.3));
                b.with_work(rig::WORK_EXTEND, |b| {
                    b.paint(METAL).pattern(pattern::EMBER);
                    b.cylinder_between(v3(1.4, y, z0 - 0.15), v3(LANCE_TIP - 0.45, y, z1), 0.13, 0.1, 6);
                    b.paint(GLOW_LASER);
                    b.cylinder_between(v3(LANCE_TIP - 0.45, y, z1), v3(LANCE_TIP, y, z1 - 0.02), 0.12, 0.02, 6);
                });
            });
        });
    });
}

/// One stinger prong from its joint to its tip: a hide sheath, a long needle, barbs
/// raked back along it, and (for the gun) a lit muzzle ring.
fn prong(b: &mut MeshBuilder, pivot: Vec3, tip: Vec3, gun: bool) {
    let bend = v3(1.0, tip.y, 18.45);
    let reach = (tip - bend).normalize();
    knuckle(b, pivot, Vec3::Y, 0.6, 0.75);
    hide(b);
    segment(b, &[(pivot, 0.55, 0.6), (bend, 0.46, 0.5), (bend + reach * 1.0, 0.3, 0.32)], Vec3::Z);
    if gun {
        b.paint(METAL).pattern(pattern::EMBER);
        b.cylinder_between(bend, tip, 0.24, 0.2, b.sides(8).min(8));
        b.paint(GLOW_LASER);
        b.cylinder_between(tip - reach * 0.4, tip, 0.28, 0.25, b.sides(8).min(8));
    } else {
        b.paint(METAL).pattern(pattern::EMBER);
        b.cylinder_between(bend, tip - reach * 0.55, 0.2, 0.14, 6);
        b.paint(GLOW_LASER);
        b.cylinder_between(tip - reach * 0.6, tip, 0.15, 0.03, 6);
    }
    if b.fine() {
        // Barbs raked back along the needle, up and out.
        hide(b);
        let out = Vec3::Y * tip.y.signum();
        for k in 0..2 {
            let at = bend.lerp(tip, 0.25 + k as f32 * 0.3);
            blade(b, at + Vec3::Z * 0.15, at - reach * 0.8 + Vec3::Z * 0.55 + out * 0.3, 0.16, out);
        }
        // A red line down the sheath.
        b.paint(GLOW_LASER);
        b.beam(pivot + v3(0.4, 0.0, 0.55), bend + v3(-0.2, 0.0, 0.45), Vec2::new(0.1, 0.1), Vec2::new(0.08, 0.08));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{build_model_fitted, Model};

    fn fitted() -> Model {
        build_model_fitted("naga_commander", 10.4, 19.0, 1, &["eng_2", "eng_3"]).unwrap()
    }

    #[test]
    fn stands_on_eight_legs_each_rigged_to_its_pair() {
        let model = fitted();
        let crawl = model.legs.and_then(|l| l.crawl).expect("a crawler");
        assert_eq!(crawl.pairs, 4);
        for lod in &model.lods[..2] {
            for pair in 0..4u32 {
                let bones = |limb: u32| {
                    lod.vertices.iter().filter(move |v| {
                        v.part == part::LOCOMOTION
                            && v.rig & rig::LIMB_MASK == limb
                            && (v.rig & rig::PAIR_MASK) >> rig::PAIR_SHIFT == pair
                    })
                };
                for limb in [rig::THIGH, rig::SHIN] {
                    assert!(bones(limb).any(|v| v.pos[1] > 0.0), "pair {pair} left bone {limb}");
                    assert!(bones(limb).any(|v| v.pos[1] < 0.0), "pair {pair} right bone {limb}");
                }
                // The foot's tip reaches the ground its pair's rest pose names.
                let [_, _, foot] = crawl.joints[pair as usize];
                let low = bones(rig::SHIN).map(|v| v.pos[2]).fold(f32::MAX, f32::min);
                assert!(low < 0.3, "pair {pair} foot at {low}");
                let reach = bones(rig::SHIN).map(|v| v.pos[1]).fold(0.0f32, f32::max);
                assert!((reach - foot[1]).abs() < 0.6, "pair {pair} reaches y {reach} for {}", foot[1]);
            }
        }
        // No leg piece is a refit piece: their pair rides the `UPGRADE_AT` bits.
        assert!(model.lods[0].vertices.iter().all(|v| v.part != part::LOCOMOTION || v.rig & (rig::UPGRADE | rig::MODULE_MASK) == 0));
    }

    #[test]
    fn tail_segments_turn_about_their_own_joints() {
        let model = fitted();
        let crawl = model.legs.unwrap().crawl.unwrap();
        assert_eq!(crawl.tail_count, TAIL.len());
        for lod in &model.lods[..2] {
            for seg in 0..TAIL.len() - 1 {
                let verts: Vec<Vec3> = lod
                    .vertices
                    .iter()
                    .filter(|v| {
                        v.part == part::TURRET && v.rig & rig::LIMB_MASK == rig::TAIL && ((v.rig & rig::TAIL_SEG_MASK) >> rig::TAIL_SEG_SHIFT) as usize == seg
                    })
                    .map(|v| Vec3::from(v.pos))
                    .collect();
                assert!(!verts.is_empty(), "segment {seg} has no geometry");
                // Every piece of it sits near its own stretch of spine, not another's.
                let (a, c) = (TAIL[seg], TAIL[seg + 1]);
                let reach = TAIL_WIDTH[seg] * 2.2 + 2.0;
                for v in &verts {
                    let t = ((*v - a).dot(c - a) / (c - a).length_squared()).clamp(0.0, 1.0);
                    assert!(v.distance(a.lerp(c, t)) < reach, "segment {seg} vertex {v} strays");
                }
            }
        }
    }

    #[test]
    fn claws_are_rigged_arm_and_jaw() {
        let model = fitted();
        let crawl = model.legs.unwrap().crawl.unwrap();
        assert_eq!(crawl.claw, Some([SHOULDER.to_array(), JAW_HINGE.to_array()]));
        let lod = &model.lods[0];
        let claw = |seg: u32| {
            lod.vertices.iter().filter(move |v| v.part == part::HULL && v.rig & rig::LIMB_MASK == rig::TAIL && (v.rig & rig::TAIL_SEG_MASK) >> rig::TAIL_SEG_SHIFT == seg)
        };
        assert!(claw(rig::CLAW_ARM).any(|v| v.pos[1] > 0.0) && claw(rig::CLAW_ARM).any(|v| v.pos[1] < 0.0));
        assert!(claw(rig::CLAW_JAW).any(|v| v.pos[1] > 0.0) && claw(rig::CLAW_JAW).any(|v| v.pos[1] < 0.0));
        // The jaw is only the finger, forward of its hinge.
        let low = claw(rig::CLAW_JAW).map(|v| v.pos[0]).fold(f32::MAX, f32::min);
        assert!(low > JAW_HINGE.x - 1.2, "jaw reaches back to {low}");
    }

    #[test]
    fn stinger_prongs_end_at_the_unit_files_muzzle_and_emitter() {
        let model = fitted();
        let tip = |limb: u32| {
            model.lods[0]
                .vertices
                .iter()
                .filter(|v| v.part == part::TURRET && v.rig & rig::LIMB_MASK == limb && v.rig & rig::MODULE_MASK == 0)
                .map(|v| Vec3::from(v.pos))
                .max_by(|a, b| a.x.total_cmp(&b.x))
                .unwrap()
        };
        assert!(tip(rig::ARM_GUN).distance(GUN_TIP) < 0.35, "gun tip {}", tip(rig::ARM_GUN));
        assert!(tip(rig::ARM_TOOL).distance(TOOL_TIP) < 0.35, "tool tip {}", tip(rig::ARM_TOOL));
        assert_eq!(model.arm_pivot, Some(PRONG_PIVOT.to_array()));
        // The sim swings the muzzles about the unit file's `turret_at`.
        assert_eq!((model.turret_pivot[0], model.turret_pivot[1]), (-6.8, 0.0));
        // Suite III's lance at rest, 1.28 m short of the unit file's `arm_emitter`.
        let lance = model.lods[0]
            .vertices
            .iter()
            .filter(|v| v.rig & rig::WORK_MASK == rig::WORK_EXTEND)
            .map(|v| v.pos[0])
            .fold(f32::MIN, f32::max);
        assert!((lance + 1.28 - 6.9).abs() < 0.05, "lance tip {lance}");
        // The tail bends from its root to the bulb.
        let tail = model.legs.unwrap().crawl.unwrap().tail;
        assert!(tail[0] < 7.5 && tail[1] > 17.0, "{tail:?}");
    }

    #[test]
    fn the_sims_single_pivot_follows_the_bending_tail() {
        // The shader turns each of the top four joints by its share of the aim; the sim
        // turns the muzzles about `AIM_PIVOT` by the whole aim. Over the unit file's
        // `aim_arc` (60 degrees either side) the two stay within half a metre.
        let turn = |p: Vec2, c: Vec2, a: f32| c + Vec2::from_angle(a).rotate(p - c);
        let joints = &TAIL[TAIL.len() - 4..];
        let mut worst = 0.0f32;
        for deg in (5..=60).step_by(5) {
            let a = (deg as f32).to_radians();
            for tip in [GUN_TIP, TOOL_TIP] {
                let mut bent = tip.truncate();
                for (j, share) in joints.iter().zip(AIM_SHARE).rev() {
                    bent = turn(bent, j.truncate(), a * share);
                }
                let swung = turn(tip.truncate(), Vec2::new(AIM_PIVOT, 0.0), a);
                worst = worst.max(bent.distance(swung));
            }
        }
        assert!(worst < 0.55, "muzzle off the drawn tip by {worst} m");
        assert!((AIM_SHARE.iter().sum::<f32>() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn fits_the_triangle_budgets() {
        // As the library's budget test counts it: the bare unit, no suites fitted.
        let model = crate::models::build_model("naga_commander").unwrap();
        let suited = fitted();
        let fitted_full = suited.lods[0].indices.len() / 3;
        assert!(fitted_full <= super::super::COMMANDER_TRIANGLES + 1000, "fully suited {fitted_full}");
        let [full, mid, coarse] = [0, 1, 2].map(|i| model.lods[i].indices.len() / 3);
        assert!(
            full <= super::super::COMMANDER_TRIANGLES && mid as f32 <= full as f32 * 0.45 + 20.0 && coarse < 60,
            "{full}/{mid}/{coarse}"
        );
        for lod in &model.lods {
            assert!(lod.vertices.iter().all(|v| v.pos[2] >= -1e-3), "below ground");
        }
    }
}
