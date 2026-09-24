//! Components shared by the Aster models: running gear, weapons, greebles.
//! Everything here draws in the caller's frame with the caller's part.

use glam::{Vec2, Vec3};

use crate::models::builder::{chamfered_rect, MeshBuilder, Section};
use crate::models::material::*;
use crate::models::part;

pub fn v3(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, y, z)
}

pub fn v2(x: f32, y: f32) -> Vec2 {
    Vec2::new(x, y)
}

/// The weapon highlight colour: blue for Aster energy weapons, orange for
/// conventional ones, and none at all for a plain gun: a dark bore, nothing lit.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Emitter {
    Blue,
    Orange,
    Unlit,
}

impl Emitter {
    pub fn material(self) -> u32 {
        match self {
            Emitter::Blue => GLOW,
            Emitter::Orange => GLOW_ORANGE,
            Emitter::Unlit => ACCENT,
        }
    }
}

// ---- running gear ----------------------------------------------------------

/// One tread unit on the +y side (mirror it for the pair): a lozenge-profiled
/// track run from `x_rear` to `x_front` with road-wheel hubs on its outer face.
pub fn track(
    b: &mut MeshBuilder,
    x_rear: f32,
    x_front: f32,
    y_inner: f32,
    y_outer: f32,
    height: f32,
) {
    b.with_part(part::LOCOMOTION, |b| {
        b.paint(TREAD);
        if b.coarse() {
            b.cuboid_open(
                v3(
                    (x_rear + x_front) * 0.5,
                    (y_inner + y_outer) * 0.5,
                    height * 0.5,
                ),
                v3(x_front - x_rear, y_outer - y_inner, height),
            );
            return;
        }
        let h = height;
        let profile = [
            [x_rear + 0.5 * h, 0.0],
            [x_front - 0.5 * h, 0.0],
            [x_front, 0.5 * h],
            [x_front - 0.22 * h, h],
            [x_rear + 0.22 * h, h],
            [x_rear, 0.5 * h],
        ];
        b.extrude_y(&profile, y_inner, y_outer);
        if b.fine() {
            // Road wheels and the larger drive sprocket, set into the outer face.
            let run = x_front - x_rear - 1.3 * height;
            let count = ((run / (0.78 * height)).round() as usize).max(2);
            b.paint(ACCENT);
            for i in 0..count {
                let x = x_rear + 0.65 * height + run * i as f32 / (count - 1) as f32;
                wheel_hub(b, v3(x, y_outer, 0.36 * height), 0.3 * height);
            }
            b.paint(METAL);
            wheel_hub(
                b,
                v3(x_front - 0.42 * height, y_outer, 0.64 * height),
                0.2 * height,
            );
            wheel_hub(
                b,
                v3(x_rear + 0.42 * height, y_outer, 0.64 * height),
                0.2 * height,
            );
        }
    });
}

/// Outward-facing hub cap on a +y side face.
fn wheel_hub(b: &mut MeshBuilder, center: Vec3, radius: f32) {
    b.cylinder_between(
        center - Vec3::Y * 0.02,
        center + Vec3::Y * 0.09,
        radius,
        radius * 0.7,
        6,
    );
}

/// A road wheel on the +y side with its axis across the body.
pub fn wheel(b: &mut MeshBuilder, center: Vec3, radius: f32, width: f32) {
    b.with_part(part::LOCOMOTION, |b| {
        let half = Vec3::Y * (width * 0.5);
        b.paint(TREAD);
        if b.coarse() {
            b.cuboid_open(center, v3(radius * 1.8, width, radius * 2.0));
            return;
        }
        b.cylinder_between(center - half, center + half, radius, radius, b.sides(10));
        if b.fine() {
            b.paint(PLATING);
            b.cylinder_between(
                center + half,
                center + half + Vec3::Y * 0.08,
                radius * 0.62,
                radius * 0.45,
                6,
            );
        }
    });
}

// ---- weapons ---------------------------------------------------------------

/// Runs `f` in a frame at `breech` whose +x axis points at `muzzle`
/// (both share the same y), passing the barrel length.
fn along_barrel(
    b: &mut MeshBuilder,
    breech: Vec3,
    muzzle: Vec3,
    f: impl FnOnce(&mut MeshBuilder, f32),
) {
    let d = muzzle - breech;
    let length = d.length();
    b.pitched(breech, d.z.atan2(d.truncate().length()), |b| f(b, length));
}

/// Twin-rail accelerator: two tapering rails side by side with an emitter
/// core glowing between them, in a white shroud at the breech end.
/// `rail` is one rail's (width, height); `gap` the clear distance between rails.
pub fn rail_gun(
    b: &mut MeshBuilder,
    breech: Vec3,
    muzzle: Vec3,
    rail: Vec2,
    gap: f32,
    emitter: Emitter,
) {
    along_barrel(b, breech, muzzle, |b, length| {
        let (w, h) = (rail.x, rail.y * 0.5);
        if b.coarse() {
            b.paint(METAL);
            b.beam(
                Vec3::ZERO,
                Vec3::X * length,
                v2(gap + 2.0 * w, 2.0 * h),
                v2(gap + 2.0 * w, 1.4 * h),
            );
            return;
        }
        b.paint(METAL);
        b.mirror_y(|b| {
            if b.fine() {
                b.extrude_y(
                    &[
                        [0.0, -h],
                        [length, -0.55 * h],
                        [length, 0.55 * h],
                        [length * 0.35, h],
                        [0.0, h],
                    ],
                    gap * 0.5,
                    gap * 0.5 + w,
                );
            } else {
                b.beam(
                    v3(0.0, gap * 0.5 + w * 0.5, 0.0),
                    v3(length, gap * 0.5 + w * 0.5, 0.0),
                    v2(w, 2.0 * h),
                    v2(w, 1.1 * h),
                );
            }
        });
        b.paint(emitter.material());
        b.block(
            v3(length * 0.2, -gap * 0.5, -0.3 * h),
            v3(length - 0.12, gap * 0.5, 0.3 * h),
        );
        let shroud = gap * 0.5 + w + 0.4 * h;
        barrel_shroud(b, length, 0.38, h, shroud);
        if b.fine() {
            b.paint(ACCENT);
            b.mirror_y(|b| {
                b.block(
                    v3(length * 0.62, gap * 0.5 - 0.02, -0.75 * h),
                    v3(length * 0.7, gap * 0.5 + w + 0.05, 0.75 * h),
                )
            });
            b.block(
                v3(length * 0.62, -gap * 0.5, -0.75 * h),
                v3(length * 0.7, gap * 0.5, -0.45 * h),
            );
            b.paint(emitter.material());
            b.plate(
                v3(length * 0.16, 0.0, 1.5 * h),
                v2(length * 0.2, shroud * 0.5),
                0.05,
                0.02,
            );
        }
    });
}

/// White breech housing: a chamfered sleeve over the first `along` of the
/// barrel. Shared by the rail and the howitzer so the family reads as one.
fn barrel_shroud(b: &mut MeshBuilder, length: f32, along: f32, h: f32, half_y: f32) {
    b.paint(PLATING);
    b.extrude_y_chamfered(
        &[
            [-0.1, -1.5 * h],
            [length * (along * 0.79), -1.5 * h],
            [length * along, -0.9 * h],
            [length * along, 0.9 * h],
            [length * (along * 0.79), 1.5 * h],
            [-0.1, 1.5 * h],
        ],
        half_y,
        (0.35 * h).max(0.06),
    );
}

/// Field howitzer: the rail's silhouette as a closed tube — same taper, same
/// white breech shroud — with a thermal jacket and a dark bore. A physical
/// projectile, not an accelerator: no gap, no glow, no gunmetal.
pub fn howitzer(b: &mut MeshBuilder, breech: Vec3, muzzle: Vec3, radius: f32) {
    along_barrel(b, breech, muzzle, |b, length| {
        let r = radius;
        let h = r * 0.95;
        if b.coarse() {
            b.paint(PLATING);
            b.beam(
                Vec3::ZERO,
                Vec3::X * length,
                Vec2::splat(r * 2.2),
                Vec2::splat(r * 1.8),
            );
            return;
        }
        // Closed rectangular tube, the rail's side profile without the split.
        // Slim past the shroud so the read is fat breech, long tube — not a
        // white bar of armour.
        b.paint(ACCENT);
        if b.fine() {
            b.extrude_y(
                &[
                    [0.0, -h],
                    [length, -0.50 * h],
                    [length, 0.50 * h],
                    [length * 0.35, h],
                    [0.0, h],
                ],
                -r * 0.62,
                r * 0.62,
            );
        } else {
            b.beam(
                Vec3::ZERO,
                Vec3::X * length,
                v2(r * 1.25, 2.0 * h),
                v2(r * 1.05, 1.05 * h),
            );
        }
        // Short, wide breech block — the rail's shroud, not a jacketed tube.
        barrel_shroud(b, length, 0.26, h * 1.22, r * 1.7);
        b.paint(PLATING);
        b.extrude_y_chamfered(
            &[
                [length * 0.26, -0.82 * h],
                [length * 0.40, -0.72 * h],
                [length * 0.40, 0.72 * h],
                [length * 0.26, 0.82 * h],
            ],
            r * 0.95,
            0.10,
        );
        b.paint(ACCENT);
        b.cylinder_between(
            Vec3::X * (length - 0.06),
            Vec3::X * (length + 0.02),
            r * 0.58,
            r * 0.58,
            6,
        );
        if b.fine() {
            for t in [0.64, 0.78] {
                let x = length * t;
                b.block(
                    v3(x, -r * 0.72, -0.62 * h),
                    v3(x + r * 0.28, r * 0.72, 0.62 * h),
                );
            }
            b.block(
                v3(-0.18, -r * 0.85, -0.85 * h),
                v3(0.42, r * 0.85, 0.85 * h),
            );
        }
    });
}

/// Faceted circular arch a siege A-frame sits on. Two plated ribs, black
/// cores, feet on the deck, a crown saddle at `pivot`. The trunnion belongs
/// at that crown — not hanging in the opening.
pub fn siege_arch(
    b: &mut MeshBuilder,
    deck_z: f32,
    pivot: Vec3,
    half_width: f32,
    front_x: f32,
    rear_x: f32,
    scale: f32,
) {
    let s = scale;
    let (peak_x, peak_z) = (pivot.x, pivot.z);
    let n = if b.fine() { 6 } else { 4 };
    // Circular segment through the feet and the crown — a parabola still
    // reads as a triangle from the RTS camera.
    let mid_x = peak_x;
    let half = ((peak_x - rear_x).max(front_x - peak_x)).max(0.5);
    let cz = (half * half + deck_z * deck_z - peak_z * peak_z) / (2.0 * (deck_z - peak_z));
    let r = (peak_z - cz).abs();
    let pts: Vec<Vec3> = (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let x = (mid_x - half) + 2.0 * half * t;
            let dx = x - mid_x;
            v3(x, half_width, cz + (r * r - dx * dx).max(0.0).sqrt())
        })
        .collect();
    b.mirror_y(|b| {
        for w in pts.windows(2) {
            b.paint(PLATING);
            b.beam(w[0], w[1], v2(0.64 * s, 0.54 * s), v2(0.56 * s, 0.46 * s));
            if b.fine() {
                b.paint(ACCENT);
                b.beam(
                    w[0] + v3(0.0, -0.15 * s, 0.0),
                    w[1] + v3(0.0, -0.15 * s, 0.0),
                    v2(0.36 * s, 0.30 * s),
                    v2(0.30 * s, 0.24 * s),
                );
            }
        }
        if b.fine() {
            b.paint(ACCENT);
            b.chamfered_box(
                v3(mid_x - half, half_width, deck_z + 0.14 * s),
                v3(0.88 * s, 0.58 * s, 0.32 * s),
                0.07 * s,
            );
            b.chamfered_box(
                v3(mid_x + half, half_width, deck_z + 0.14 * s),
                v3(0.88 * s, 0.58 * s, 0.32 * s),
                0.07 * s,
            );
        }
    });
    // Crown the A-frame bolts to.
    b.paint(PLATING);
    b.chamfered_box(
        v3(peak_x, 0.0, peak_z - 0.06 * s),
        v3(1.28 * s, half_width * 1.08, 0.30 * s),
        0.07 * s,
    );
}

/// Open A-frame that holds a pitching siege gun: two plated struts a side
/// over a black core, a trunnion through `pivot`, a peak plate with a team
/// flash. The barrel sits in the gap — a carrier, not a turret house.
pub fn siege_a_frame(
    b: &mut MeshBuilder,
    deck_z: f32,
    pivot: Vec3,
    half_width: f32,
    front_x: f32,
    rear_x: f32,
    peak_x: f32,
    peak_z: f32,
    scale: f32,
) {
    let s = scale;
    let peak = v3(peak_x, half_width, peak_z);
    let front_foot = v3(front_x, half_width, deck_z);
    let rear_foot = v3(rear_x, half_width, deck_z);
    b.mirror_y(|b| {
        b.paint(PLATING);
        b.beam(
            front_foot,
            peak,
            v2(0.52 * s, 0.46 * s),
            v2(0.34 * s, 0.30 * s),
        );
        b.beam(
            rear_foot,
            peak,
            v2(0.52 * s, 0.46 * s),
            v2(0.34 * s, 0.30 * s),
        );
        b.paint(ACCENT);
        b.beam(
            front_foot + v3(0.02, -0.12 * s, 0.04),
            peak + v3(0.02, -0.12 * s, -0.04),
            v2(0.28 * s, 0.24 * s),
            v2(0.18 * s, 0.16 * s),
        );
        b.beam(
            rear_foot + v3(-0.02, -0.12 * s, 0.04),
            peak + v3(-0.02, -0.12 * s, -0.04),
            v2(0.28 * s, 0.24 * s),
            v2(0.18 * s, 0.16 * s),
        );
    });
    b.paint(ACCENT);
    let ring = (1.35 * half_width / 1.42).max(1.1);
    b.prism(v3(0.0, 0.0, deck_z), 8, ring, ring, 0.16 * s);
    b.cylinder_between(
        v3(pivot.x, -half_width - 0.20 * s, pivot.z),
        v3(pivot.x, half_width + 0.20 * s, pivot.z),
        0.36 * s,
        0.36 * s,
        6,
    );
    // Cross-beam on the two arch tips — wide enough to reach both, not a
    // floating cap in the gap between them.
    let cap_h = 0.28 * s;
    b.paint(PLATING);
    b.chamfered_box(
        v3(peak_x, 0.0, peak_z - cap_h * 0.25),
        v3(0.78 * s, half_width * 2.0 + 0.30 * s, cap_h),
        0.08 * s,
    );
    team_panel(
        b,
        v3(peak_x, 0.0, peak_z + cap_h * 0.25),
        v2(0.55 * s, half_width * 1.7),
    );
}

/// Hanging mass behind the trunnion: the throwing arm's counterweight.
pub fn siege_counterweight(b: &mut MeshBuilder, at: Vec3, size: Vec3) {
    b.paint(ACCENT);
    b.chamfered_box(at, size, size.z * 0.13);
    b.paint(PLATING);
    b.plate(
        at + Vec3::Z * (size.z * 0.5),
        v2(size.x * 0.81, size.y * 0.82),
        0.07,
        0.03,
    );
    team_panel(
        b,
        at + Vec3::Z * (size.z * 0.55),
        v2(size.x * 0.48, size.y * 0.67),
    );
}

/// Conventional tube gun: gunmetal barrel, white thermal sleeve, slotted
/// muzzle brake and a hot bore.
pub fn cannon(b: &mut MeshBuilder, breech: Vec3, muzzle: Vec3, radius: f32, emitter: Emitter) {
    along_barrel(b, breech, muzzle, |b, length| {
        let r = radius;
        b.paint(METAL);
        if b.coarse() {
            b.beam(
                Vec3::ZERO,
                Vec3::X * length,
                Vec2::splat(r * 2.2),
                Vec2::splat(r * 1.8),
            );
            return;
        }
        let sides = b.sides(8);
        b.cylinder_between(Vec3::ZERO, Vec3::X * length, r * 1.1, r, sides);
        b.paint(PLATING);
        b.cylinder_between(
            Vec3::X * (length * 0.04),
            Vec3::X * (length * 0.42),
            r * 1.9,
            r * 1.6,
            6,
        );
        b.paint(ACCENT);
        let brake = (r * 3.2).min(length * 0.2);
        b.chamfered_box(
            v3(length - brake * 0.5 - 0.02, 0.0, 0.0),
            v3(brake, r * 3.6, r * 2.6),
            r * 0.6,
        );
        b.paint(emitter.material());
        b.cylinder_between(
            Vec3::X * (length - 0.06),
            Vec3::X * (length + 0.03),
            r * 0.75,
            r * 0.75,
            6,
        );
        if b.fine() {
            b.paint(ACCENT);
            b.cylinder_between(
                Vec3::X * (length * 0.42),
                Vec3::X * (length * 0.47),
                r * 1.45,
                r * 1.45,
                6,
            );
            b.cylinder_between(
                Vec3::X * (length * 0.66),
                Vec3::X * (length * 0.7),
                r * 1.4,
                r * 1.4,
                6,
            );
            if emitter != Emitter::Unlit {
                b.paint(emitter.material());
                b.mirror_y(|b| {
                    b.block(
                        v3(length - brake * 0.8, r * 1.8, -r * 0.5),
                        v3(length - brake * 0.25, r * 1.84, r * 0.5),
                    )
                });
            }
        }
    });
}

/// Heavy machine gun: a thin tube in a perforated jacket, a flash hider, and
/// a hot bore. Conventional fire — orange, not a rail.
pub fn machine_gun(b: &mut MeshBuilder, breech: Vec3, muzzle: Vec3, radius: f32) {
    along_barrel(b, breech, muzzle, |b, length| {
        let r = radius;
        if b.coarse() {
            b.paint(METAL);
            b.beam(
                Vec3::ZERO,
                Vec3::X * length,
                Vec2::splat(r * 2.1),
                Vec2::splat(r * 1.6),
            );
            return;
        }
        let sides = b.sides(6);
        b.paint(METAL);
        b.cylinder_between(Vec3::ZERO, Vec3::X * length, r * 1.08, r * 0.9, sides);
        // Receiver.
        b.cylinder_between(Vec3::ZERO, Vec3::X * (length * 0.14), r * 1.85, r * 1.4, 6);
        // Cooling jacket: dark, banded — not a cannon's white thermal sleeve.
        b.paint(ACCENT);
        b.cylinder_between(
            Vec3::X * (length * 0.16),
            Vec3::X * (length * 0.78),
            r * 1.58,
            r * 1.42,
            sides,
        );
        if b.fine() {
            b.paint(METAL);
            for t in [0.28, 0.44, 0.60, 0.74] {
                let x = length * t;
                b.cylinder_between(
                    Vec3::X * (x - r * 0.1),
                    Vec3::X * (x + r * 0.1),
                    r * 1.66,
                    r * 1.66,
                    6,
                );
            }
        }
        // Flash hider: a short open cone, not a baffle brake.
        let hid = (r * 2.6).min(length * 0.16);
        b.paint(ACCENT);
        b.cylinder_between(
            Vec3::X * (length - hid),
            Vec3::X * length,
            r * 1.55,
            r * 2.05,
            sides,
        );
        b.paint(GLOW_ORANGE);
        b.cylinder_between(
            Vec3::X * (length - 0.04),
            Vec3::X * (length + 0.02),
            r * 0.58,
            r * 0.5,
            6,
        );
    });
}

/// Reclaim processor: a focusing tube that draws mass in. The first half is
/// a dark housing with feed pipes and charge collars — plant, not a point-
/// defence lance. The mouth is a dark intake ring. Orange bore.
pub fn reclaim_gun(b: &mut MeshBuilder, breech: Vec3, muzzle: Vec3, radius: f32) {
    along_barrel(b, breech, muzzle, |b, length| {
        let r = radius;
        if b.coarse() {
            b.paint(ACCENT);
            b.beam(
                Vec3::ZERO,
                Vec3::X * length,
                Vec2::splat(r * 3.8),
                Vec2::splat(r * 2.0),
            );
            b.paint(GLOW_ORANGE);
            b.cuboid(Vec3::X * (length - r * 0.35), Vec3::splat(r * 1.15));
            return;
        }
        let sides = b.sides(8);
        b.paint(METAL);
        b.cylinder_between(Vec3::ZERO, Vec3::X * length, r * 1.18, r * 0.92, sides);
        // Processor housing covers most of the tube so the silhouette is plant,
        // not a point-defence lance. Gunmetal pipes and a white lid read from above.
        let house = length * 0.76;
        b.paint(ACCENT);
        b.block(
            v3(length * 0.04, -r * 2.05, -r * 0.35),
            v3(house, r * 2.05, r * 2.85),
        );
        b.paint(PLATING_DARK);
        b.cylinder_between(
            Vec3::X * (length * 0.02),
            Vec3::X * house,
            r * 2.35,
            r * 2.05,
            6,
        );
        b.paint(ACCENT);
        b.block(
            v3(-r * 0.5, -r * 2.15, -r * 1.3),
            v3(length * 0.16, r * 2.15, r * 2.4),
        );
        b.paint(PLATING);
        b.plate(
            v3(house * 0.48, 0.0, r * 2.85),
            v2(house * 0.68, r * 2.8),
            0.12,
            0.04,
        );
        b.mirror_y(|b| {
            b.block(
                v3(length * 0.12, r * 2.05, r * 0.35),
                v3(house * 0.9, r * 2.2, r * 2.45),
            )
        });
        b.paint(METAL);
        b.mirror_y(|b| {
            b.cylinder_between(
                v3(length * 0.1, r * 2.15, r * 0.55),
                v3(house * 0.98, r * 2.0, r * 0.7),
                r * 0.42,
                r * 0.32,
                6,
            );
            b.block(
                v3(length * 0.22, r * 1.7, r * 1.7),
                v3(length * 0.48, r * 2.55, r * 2.55),
            );
            // Gantry rails carry the last stretch of tube.
            b.cylinder_between(
                v3(house * 0.92, r * 1.15, r * 1.15),
                v3(length - r * 1.6, r * 0.95, r * 0.85),
                r * 0.28,
                r * 0.2,
                6,
            );
        });
        b.paint(ACCENT);
        b.cylinder_between(
            Vec3::X * (house - r * 0.35),
            Vec3::X * (house + r * 0.35),
            r * 2.15,
            r * 2.15,
            6,
        );
        // Mouth: a short dark intake the tube runs through, flush with the bore.
        let ring = (r * 2.8).min(length * 0.09);
        b.paint(METAL);
        b.cylinder_between(
            Vec3::X * (length - ring),
            Vec3::X * length,
            r * 1.85,
            r * 1.5,
            sides,
        );
        b.paint(ACCENT);
        b.mirror_y(|b| {
            b.block(
                v3(length - ring * 1.2, r * 0.5, -r * 0.28),
                v3(length - 0.04, r * 2.15, r * 0.28),
            )
        });
        b.block(
            v3(length - ring * 1.2, -r * 0.28, r * 0.5),
            v3(length - 0.04, r * 0.28, r * 1.95),
        );
        b.paint(GLOW_ORANGE);
        b.cylinder_between(
            Vec3::X * (length - 0.06),
            Vec3::X * (length + 0.02),
            r * 0.62,
            r * 0.62,
            6,
        );
        if b.fine() {
            b.paint(METAL);
            for t in [0.82, 0.9] {
                let x = length * t;
                b.cylinder_between(
                    Vec3::X * (x - r * 0.22),
                    Vec3::X * (x + r * 0.22),
                    r * 1.42,
                    r * 1.42,
                    6,
                );
            }
            b.paint(GLOW_ORANGE);
            glow_strip(
                b,
                v3(house * 0.42, 0.0, r * 2.98),
                v2(house * 0.28, r * 0.7),
                GLOW_ORANGE,
            );
        }
    });
}

// ---- shells ----------------------------------------------------------------

/// Plan of the Aster turret: narrow front face, swept cheeks, short bustle.
/// `length` runs from the bustle (-0.52) to the front face (+0.48).
pub fn turret_plan(length: f32, width: f32) -> Vec<[f32; 2]> {
    let (l, w) = (length, width * 0.5);
    vec![
        [0.48 * l, -0.36 * w],
        [0.48 * l, 0.36 * w],
        [0.22 * l, w],
        [-0.32 * l, w],
        [-0.52 * l, 0.55 * w],
        [-0.52 * l, -0.55 * w],
        [-0.32 * l, -w],
        [0.22 * l, -w],
    ]
}

/// The flat, full-width part of a turret roof, for placing hatches and panels.
#[derive(Clone, Copy)]
pub struct Roof {
    pub rear: f32,
    pub front: f32,
    pub half_width: f32,
    pub z: f32,
}

impl Roof {
    /// Point on the roof: `u` 0..1 from rear to front, `v` -1..1 from right to left.
    pub fn at(&self, u: f32, v: f32) -> Vec3 {
        v3(
            self.rear + (self.front - self.rear) * u,
            self.half_width * v,
            self.z,
        )
    }

    pub fn length(&self) -> f32 {
        self.front - self.rear
    }
}

/// Faceted turret shell over a [`turret_plan`]: undercut below, widest at a
/// third of its height, roof drawn in and set back so every face slopes.
pub fn turret_shell(b: &mut MeshBuilder, length: f32, width: f32, z0: f32, z1: f32) -> Roof {
    let plan = turret_plan(length, width);
    let h = z1 - z0;
    let (scale, shift) = (
        v2(0.62, 0.66),
        if b.coarse() { -0.04 * h } else { -0.12 * h },
    );
    let top = Section::scaled(z1, scale.x, scale.y).shifted(shift, 0.0);
    if b.coarse() {
        b.frustum_open(
            v3(-0.02 * length, 0.0, z0),
            v2(length * 0.9, width * 0.9),
            v2(length * 0.58, width * 0.62),
            h,
            v2(shift, 0.0),
        );
    } else {
        b.loft_z(
            &plan,
            &[
                Section::new(z0, 0.86),
                Section::new(z0 + 0.34 * h, 1.0),
                top,
            ],
        );
    }
    Roof {
        rear: -0.32 * length * scale.x + shift,
        front: 0.22 * length * scale.x + shift,
        half_width: width * 0.5 * scale.y,
        z: z1,
    }
}

/// Plan of a vehicle hull: chamfered nose, clipped tail corners.
pub fn hull_plan(x_rear: f32, x_front: f32, half_width: f32, nose: f32) -> Vec<[f32; 2]> {
    let w = half_width;
    let tail = nose * 0.45;
    vec![
        [x_front, -w + nose * 1.3],
        [x_front, w - nose * 1.3],
        [x_front - nose, w],
        [x_rear + tail, w],
        [x_rear, w - tail],
        [x_rear, -w + tail],
        [x_rear + tail, -w],
        [x_front - nose, -w],
    ]
}

/// A tracked hull: running gear, dark chassis tub, white faceted shell that
/// leaves the outer tread showing, team-colour front fenders.
pub struct Chassis {
    pub rear: f32,
    pub front: f32,
    /// Track inner edge, outer edge (y) and height.
    pub track: (f32, f32, f32),
    /// Two independent track units per side instead of one.
    pub split_tracks: bool,
    /// Height of the deck (top of the shell).
    pub deck: f32,
    /// Graphite hull instead of white: white is then applied as plates on top.
    pub dark: bool,
    /// A glowing sensor slit across the glacis. Not for plain tech 1 hulls.
    pub lit: bool,
}

/// Emits the chassis and returns its flat deck for the caller to furnish.
pub fn tracked_chassis(b: &mut MeshBuilder, c: &Chassis) -> Roof {
    let (inner, outer, track_height) = c.track;
    let length = c.front - c.rear;
    let half_width = inner + 0.42 * (outer - inner);
    let nose = length * 0.13;
    let (scale, shift) = (v2(0.8, 0.76), -0.04 * length);
    let plan = hull_plan(c.rear, c.front, half_width, nose);
    let belt = track_height * 0.74;
    let waist = belt + (c.deck - belt) * 0.42;
    b.set_treads((inner + outer) * 0.5, outer - inner, c.rear);

    b.mirror_y(|b| {
        if c.split_tracks && !b.coarse() {
            let mid = (c.rear + c.front) * 0.5;
            track(
                b,
                c.rear - 0.1,
                mid - 0.04 * length,
                inner,
                outer,
                track_height,
            );
            track(
                b,
                mid + 0.04 * length,
                c.front - 0.05,
                inner,
                outer,
                track_height,
            );
        } else {
            track(b, c.rear - 0.1, c.front - 0.05, inner, outer, track_height);
        }
        // Front fender in team colour, easy to read from above. A dark coarse
        // hull (the engineer) skips it to stay inside the far-LOD budget.
        if !(b.coarse() && c.dark) {
            let fender = v2(length * 0.2, (outer - inner) * 0.96);
            b.paint(TEAM);
            if b.fine() {
                b.plate(
                    v3(c.front - 0.34 * length, (inner + outer) * 0.5, track_height),
                    fender,
                    0.1,
                    0.05,
                );
            } else {
                b.decal(
                    v3(
                        c.front - 0.34 * length,
                        (inner + outer) * 0.5,
                        track_height + 0.06,
                    ),
                    fender,
                );
            }
        }
    });
    b.paint(if c.dark && !b.coarse() {
        PLATING_DARK
    } else {
        PLATING
    });
    if b.coarse() {
        b.frustum_open(
            v3((c.rear + c.front) * 0.5, 0.0, belt * 0.6),
            v2(length, half_width * 2.0),
            v2(length * scale.x, half_width * 2.0 * scale.y),
            c.deck - belt * 0.6,
            v2(shift, 0.0),
        );
    } else {
        b.loft_z(
            &plan,
            &[
                Section::new(belt, 0.95),
                Section::new(waist, 1.0),
                Section::scaled(c.deck, scale.x, scale.y).shifted(shift, 0.0),
            ],
        );
        b.paint(ACCENT);
        b.block(
            v3(c.rear + 0.3, -inner, track_height * 0.3),
            v3(c.front - 0.5, inner, belt + 0.05),
        );
    }
    if b.fine() {
        // Glacis sensor slit and hanging side skirts.
        if c.lit {
            on_slope(
                b,
                [c.front, waist],
                [c.front * scale.x + shift, c.deck],
                0.5,
                |b| glow_strip(b, Vec3::ZERO, v2(0.04 * length, half_width * 0.9), GLOW),
            );
        }
        b.mirror_y(|b| {
            b.paint(PLATING);
            let panels = if c.split_tracks { 4 } else { 3 };
            let pitch = length * 0.84 / panels as f32;
            for i in 0..panels {
                let x = c.rear + length * 0.08 + pitch * i as f32;
                b.block(
                    v3(x, outer, track_height * 0.52),
                    v3(
                        x + pitch * 0.93,
                        outer + 0.03 * (outer - inner) + 0.08,
                        track_height * 1.02,
                    ),
                );
            }
        });
    }
    Roof {
        rear: (c.rear + nose * 0.45) * scale.x + shift,
        front: (c.front - nose) * scale.x + shift,
        half_width: half_width * scale.y,
        z: c.deck,
    }
}

/// A hover hull: lift skirt, optional plenum light, white faceted shell.
/// The skirt is `LOCOMOTION`; it sits a few centimetres off the surface so the
/// craft reads as hovering on water as well as on land.
#[allow(dead_code)]
pub struct Hover {
    pub rear: f32,
    pub front: f32,
    pub half_width: f32,
    pub deck: f32,
    /// Glow along the lift plenum. Not for plain tech 1 hulls.
    pub lit: bool,
}

/// Emits the hover chassis and returns its flat deck for the caller to furnish.
#[allow(dead_code)]
pub fn hover_chassis(b: &mut MeshBuilder, c: &Hover) -> Roof {
    let length = c.front - c.rear;
    let nose = length * 0.16;
    let skirt = chamfered_rect(v2(length * 0.52, c.half_width + 0.18), length * 0.16);
    b.with_part(part::LOCOMOTION, |b| {
        b.paint(TREAD);
        if b.coarse() {
            b.frustum_open(
                v3((c.rear + c.front) * 0.5, 0.0, 0.08),
                v2(length + 0.3, (c.half_width + 0.22) * 2.0),
                v2(length + 0.55, (c.half_width + 0.32) * 2.0),
                0.52,
                v2(0.0, 0.0),
            );
        } else {
            b.at(v3((c.rear + c.front) * 0.5, 0.0, 0.0), |b| {
                b.loft_z(
                    &skirt,
                    &[
                        Section::new(0.1, 0.82),
                        Section::new(0.48, 1.0),
                        Section::new(0.58, 0.98),
                    ],
                );
            });
        }
    });
    if c.lit && !b.coarse() {
        b.paint(GLOW);
        b.at(v3((c.rear + c.front) * 0.5, 0.0, 0.0), |b| {
            b.loft_z(
                &skirt,
                &[Section::new(0.58, 0.94), Section::new(0.68, 0.94)],
            );
        });
    }
    let plan = hull_plan(c.rear, c.front, c.half_width, nose);
    let (scale, shift) = (v2(0.82, 0.78), -0.03 * length);
    let belt = if c.lit { 0.68 } else { 0.58 };
    b.paint(PLATING);
    if b.coarse() {
        b.frustum_open(
            v3((c.rear + c.front) * 0.5, 0.0, belt),
            v2(length, c.half_width * 2.0),
            v2(length * scale.x, c.half_width * 2.0 * scale.y),
            c.deck - belt,
            v2(shift, 0.0),
        );
    } else {
        b.loft_z(
            &plan,
            &[
                Section::new(belt, 0.96),
                Section::new(belt + (c.deck - belt) * 0.4, 1.0),
                Section::scaled(c.deck, scale.x, scale.y).shifted(shift, 0.0),
            ],
        );
        b.paint(ACCENT);
        b.block(
            v3(c.rear + 0.25, -c.half_width * 0.62, 0.28),
            v3(c.front - 0.4, c.half_width * 0.62, belt + 0.04),
        );
    }
    Roof {
        rear: (c.rear + nose * 0.45) * scale.x + shift,
        front: (c.front - nose) * scale.x + shift,
        half_width: c.half_width * scale.y,
        z: c.deck,
    }
}

/// Hover skirt under an amphibious hull: dropped on water, tucked up on land.
/// The shader poses `rig::FLOAT` verts from the water depth under the unit.
pub fn float_skirt(b: &mut MeshBuilder, rear: f32, front: f32, half_width: f32) {
    let length = front - rear;
    let mid = (rear + front) * 0.5;
    b.with_part(part::LOCOMOTION, |b| {
        b.with_float(|b| {
            // Afloat it is the hull's sides: a flared graphite pontoon from just
            // under the waterline (the shader lowers it 0.25) to the track tops, closing round the
            // treads (the shader pulls them in behind it), with a white gunwale
            // so the boat outline reads from above. On land the shader folds it
            // in between the tracks.
            b.paint(PLATING_DARK);
            if b.coarse() {
                return;
            }
            let plan = chamfered_rect(v2(length * 0.52, half_width + 0.32), length * 0.14);
            b.at(v3(mid, 0.0, 0.0), |b| {
                b.loft_z(
                    &plan,
                    &[
                        Section::new(0.0, 0.84),
                        Section::new(0.5, 0.97),
                        Section::new(1.15, 1.02),
                    ],
                );
                b.paint(PLATING);
                b.loft_z(
                    &plan,
                    &[
                        Section::new(1.15, 1.035),
                        Section::new(1.29, 1.035),
                        Section::new(1.33, 1.0),
                    ],
                );
            });
        });
    });
}

/// Frame lying on a sloped surface given by its side profile (x, z) from
/// `rear` to `front`: origin `along` of the way, +x toward `front`, +z out of
/// the surface (the travel direction turned a quarter turn from +x toward +z,
/// so list an undercut surface top-to-bottom).
pub fn on_slope(
    b: &mut MeshBuilder,
    front: [f32; 2],
    rear: [f32; 2],
    along: f32,
    f: impl FnOnce(&mut MeshBuilder),
) {
    let (dx, dz) = (front[0] - rear[0], front[1] - rear[1]);
    let at = v3(rear[0] + dx * along, 0.0, rear[1] + dz * along);
    b.pitched(at, dz.atan2(dx), f);
}

// ---- greebles --------------------------------------------------------------

/// Bare whip antenna: a black rod on a spring base, nothing lit.
pub fn whip(b: &mut MeshBuilder, base: Vec3, height: f32, lean: f32) {
    let tip = base + v3(-lean * height, 0.0, height);
    b.paint(ACCENT);
    b.cylinder_between(base, base + Vec3::Z * 0.18, 0.09, 0.07, 6);
    b.cylinder_between(base + Vec3::Z * 0.18, tip, 0.035, 0.015, 4);
}

/// Whip antenna with a lit tip.
pub fn antenna(b: &mut MeshBuilder, base: Vec3, height: f32, lean: f32) {
    let tip = base + v3(-lean * height, 0.0, height);
    b.paint(ACCENT);
    b.cylinder_between(base, tip, 0.05 + height * 0.012, 0.02 + height * 0.006, 4);
    b.paint(GLOW);
    b.cuboid(tip, Vec3::splat(0.07 + height * 0.025));
}

/// Recessed vent: dark tray with glowing slats, lying on a horizontal surface.
pub fn vent(b: &mut MeshBuilder, base_center: Vec3, size: Vec2, slats: usize, glow: u32) {
    b.paint(ACCENT);
    b.plate(base_center, size, 0.08, 0.04);
    b.paint(glow);
    let pitch = size.x / slats as f32;
    for i in 0..slats {
        let x = base_center.x - size.x * 0.5 + pitch * (i as f32 + 0.5);
        b.plate(
            v3(x, base_center.y, base_center.z + 0.08),
            v2(pitch * 0.42, size.y * 0.78),
            0.04,
            0.02,
        );
    }
}

/// Glow strip lying on a horizontal surface.
pub fn glow_strip(b: &mut MeshBuilder, base_center: Vec3, size: Vec2, glow: u32) {
    b.paint(glow);
    b.plate(base_center, size, 0.05, 0.02);
}

/// Team-colour panel lying on a horizontal surface: a raised plate at full
/// detail, a single quad below that.
pub fn team_panel(b: &mut MeshBuilder, base_center: Vec3, size: Vec2) {
    b.paint(TEAM);
    if b.fine() {
        b.plate(base_center, size, 0.07, 0.03);
    } else {
        b.decal(base_center + Vec3::Z * 0.07, size);
    }
}
