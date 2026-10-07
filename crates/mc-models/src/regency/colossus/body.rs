//! Sculpted siege carapace: a central reactor keel between swept shoulder shells.
//! Deep mechanical channels and overlapping armour establish detail at three scales.
use super::super::commander::form::{blade, ring, sleeve, KEEL, OCT};
use super::super::kit::{dark_plate, metal, seam, v3};
use super::{DECK, LANCE_PIVOT, RACE};
use crate::builder::{MeshBuilder, Section};
use crate::{material::*, part, pattern, rig};
use glam::{Vec2, Vec3};

pub(super) const EMITTER: Vec3 = Vec3::new(-14.0, 0.0, 55.2);

pub(super) fn carapace(b: &mut MeshBuilder) {
    // A deep central keel physically joins the rotating race to the lance cradle.
    dark_plate(b);
    sleeve(
        b,
        &[
            ring(v3(-24.0, 0.0, 43.0), Vec3::Z, 3.0, 2.2),
            ring(v3(-15.0, 0.0, 44.0), Vec3::Z, 9.0, 5.5),
            ring(v3(-2.0, 0.0, 44.5), Vec3::Z, 11.0, 6.5),
            ring(v3(10.0, 0.0, 44.5), Vec3::Z, 9.0, 5.0),
            ring(v3(20.0, 0.0, 44.0), Vec3::Z, 4.5, 3.6),
        ],
        &KEEL,
    );
    b.mirror_y(shoulder);
    reactor(b);
    // Twin recessed cheek gun cradles continue the shoulder's lower edge.
    b.mirror_y(|b| {
        dark_plate(b);
        sleeve(
            b,
            &[
                ring(v3(3.0, 8.2, 44.0), Vec3::Z, 2.6, 2.8),
                ring(v3(12.0, 8.6, 44.5), Vec3::Z, 3.8, 3.4),
                ring(v3(19.5, 8.6, 44.5), Vec3::Z, 2.3, 2.4),
            ],
            &OCT,
        );
    });
}

fn shoulder(b: &mut MeshBuilder) {
    // Swept manta-like shells: broad at the front, long pointed aft corners.
    dark_plate(b);
    sleeve(
        b,
        &[
            ring(v3(15.0, 8.2, 46.5), Vec3::Z, 2.2, 2.7),
            ring(v3(7.0, 12.0, 47.5), Vec3::Z, 6.2, 4.0),
            ring(v3(-5.0, 13.5, 47.0), Vec3::Z, 6.8, 3.8),
            ring(v3(-17.0, 13.0, 45.5), Vec3::Z, 4.5, 2.4),
            ring(v3(-28.0, 10.0, 44.8), Vec3::Z, 0.4, 0.5),
        ],
        &KEEL,
    );
    // Deliberately separated armour courses, with bronze mechanisms down in the gaps.
    let count = if b.fine() { 5 } else { 3 };
    for i in 0..count {
        let x = 10.0 - i as f32 * if b.fine() { 5.6 } else { 9.0 };
        dark_plate(b);
        blade(
            b,
            v3(x, 12.5, 49.1),
            v3(-1.0, 0.16, -0.12),
            Vec3::Z,
            10.0,
            4.9,
            0.35,
            0.8,
        );
        if b.fine() {
            metal(b);
            blade(
                b,
                v3(x - 0.5, 12.5, 49.7),
                v3(-1.0, 0.16, -0.12),
                Vec3::Z,
                1.0,
                3.8,
                0.0,
                0.16,
            );
        }
    }
    if b.fine() {
        // Recessed radiator ribs along the lower outside flank.
        seam(b);
        b.beam(
            v3(6.0, 17.3, 46.0),
            v3(-15.0, 16.1, 44.4),
            Vec2::new(1.0, 1.7),
            Vec2::new(0.8, 1.2),
        );
        for i in 0..12 {
            let t = i as f32 / 12.0;
            let c = v3(5.0, 17.8, 46.0).lerp(v3(-14.0, 16.4, 44.4), t);
            metal(b);
            b.beam(
                c - Vec3::Z * 0.7,
                c + Vec3::Z * 0.7,
                Vec2::new(0.28, 0.35),
                Vec2::new(0.18, 0.25),
            );
        }
        // Flush identification inlay on each shoulder, not a glowing floating bar.
        b.paint(TEAM).pattern(pattern::PLAIN);
        blade(
            b,
            v3(5.0, 13.0, 50.0),
            v3(-1.0, 0.16, -0.12),
            Vec3::Z,
            4.0,
            1.2,
            0.0,
            0.06,
        );
    }
}

fn reactor(b: &mut MeshBuilder) {
    // A low armoured crown replaces the aircraft tail fin. The rocket pack remains
    // forward of it; alternating heat sinks lead back to a shield-emitter cap.
    dark_plate(b);
    sleeve(
        b,
        &[
            ring(v3(-10.0, 0.0, 50.0), Vec3::Z, 3.8, 2.8),
            ring(v3(-14.0, 0.0, 51.0), Vec3::Z, 4.4, 4.2),
            ring(v3(-19.0, 0.0, 49.5), Vec3::Z, 3.4, 3.0),
            ring(v3(-24.0, 0.0, 46.0), Vec3::Z, 1.4, 1.0),
        ],
        &KEEL,
    );
    if b.fine() {
        for i in 0..7 {
            let x = -11.5 - i as f32 * 1.3;
            b.mirror_y(|b| {
                metal(b);
                blade(
                    b,
                    v3(x, 3.4, 50.8 - i as f32 * 0.35),
                    v3(-0.35, 1.0, -0.3),
                    Vec3::Z,
                    3.1,
                    0.35,
                    0.0,
                    0.35,
                );
            });
        }
    }
}

pub(super) fn coarse(b: &mut MeshBuilder) {
    b.with_part(part::TURRET, |b| {
        dark_plate(b);
        b.loft_z(
            &[
                [20.0, 0.0],
                [2.0, 19.0],
                [-27.0, 10.0],
                [-21.0, 0.0],
                [-27.0, -10.0],
                [2.0, -19.0],
            ],
            &[Section::new(RACE, 0.65), Section::new(DECK + 4.0, 1.0)],
        );
        b.paint(TEAM);
        b.face(&[
            v3(0.0, -4.0, DECK + 4.02),
            v3(0.0, 4.0, DECK + 4.02),
            v3(-8.0, 0.0, DECK + 4.02),
        ]);
        dark_plate(b);
        b.with_limb(rig::ARM_GUN, |b| {
            b.beam(
                LANCE_PIVOT,
                LANCE_PIVOT + v3(20.0, 0.0, 0.0),
                Vec2::new(8.0, 8.0),
                Vec2::new(6.0, 5.0),
            );
        });
    });
}
