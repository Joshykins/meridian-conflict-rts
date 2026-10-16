//! The Lens's body, hung between its legs: a seam-dark belly band under plated flanks, the
//! hips socketed in plated housings at its corners, and an open well in its back under the
//! core, where three rotor rings turn (`part::ORBIT`), neighbours opposite ways: the
//! Fallen's machinery in small.

use std::f32::consts::TAU;

use glam::Vec3;

use crate::builder::{MeshBuilder, Section};
use crate::gpu_consts::surface_mark as mark;
use crate::material::PLATING_DARK;
use crate::pattern;

use super::super::commander::form::OCT;
use super::super::kit::{dark_plate, metal, nanite_plate, seam, spike, v3};
use super::super::machine::{armour, hoop, swept, Frame};
use super::{legs, CORE};

/// The body, planted: its belly, the top of its seam band, its back, and its reach.
const BELLY: f32 = 0.45;
const BAND: f32 = 0.9;
const BACK: f32 = 2.5;
const BODY: f32 = 3.0;

pub(super) fn body(b: &mut MeshBuilder) {
    // The owner's colour on the body's back, behind the well.
    b.mark(
        v3(-2.8, -0.3, BACK - 0.15),
        v3(-1.8, 0.3, BACK + 0.15),
        mark::TEAM,
        0.0,
    );
    let plan: Vec<[f32; 2]> = OCT.iter().map(|&[x, y]| [x * BODY, y * BODY]).collect();
    if b.coarse() {
        dark_plate(b);
        b.loft_z(&plan, &[Section::new(BELLY, 0.8), Section::new(BACK, 0.85)]);
        return;
    }
    hull(b, &plan);
    hips(b);
    rotor_well(b);
}

/// The hung hull: a seam-dark band under plated flanks drawn in to its back, red optics at
/// its front. Round a rotor well the back is a collar, open in the middle.
fn hull(b: &mut MeshBuilder, plan: &[[f32; 2]]) {
    seam(b);
    b.loft_z(
        plan,
        &[Section::new(BELLY, 0.8), Section::new(BAND + 0.02, 0.96)],
    );
    dark_plate(b);
    let top = BACK - 0.5;
    b.with_facets(|b| {
        b.loft_z(
            plan,
            &[
                Section::new(BAND, 1.0),
                Section::new(BAND + (top - BAND) * 0.55, 1.0),
                Section::new(top, 0.82),
            ],
        )
    });
    // Under it, the works it hangs from: a graphite drum and the race the hips turn in.
    metal(b);
    let sides = b.sides(12);
    b.prism(v3(0.0, 0.0, BELLY - 0.25), sides, 1.5, 1.8, 0.27);
    b.mirror_y(|b| {
        b.mark(v3(2.9, 0.25, 1.5), v3(3.1, 0.75, 1.75), mark::LAMP, 0.0);
        b.mark(v3(-2.9, 0.4, 1.0), v3(-1.0, 2.6, 2.2), mark::OUTLINE, 0.0);
    });
}

/// At each corner a plated housing the hip turns in: a graphite race, and a swept plate
/// over it whose point runs back down the leg's side.
fn hips(b: &mut MeshBuilder) {
    b.mirror_y(|b| {
        for &(hip, _, foot, _) in &legs::CRAWL {
            let out = (foot - hip).with_z(0.0).normalize();
            let across = Vec3::Z.cross(out);
            metal(b);
            let sides = b.sides(10);
            b.cylinder_between(hip - out * 0.9, hip - out * 0.2, 0.75, 0.7, sides);
            dark_plate(b);
            let f = Frame::new(
                hip - out * 1.1 + Vec3::Z * 0.75,
                out - Vec3::Z * 0.25,
                Vec3::Z + out * 0.4,
            );
            b.with_facets(|b| armour(b, &f, &swept(1.9, 0.75, 0.0, 0.5), 0.22));
            // Cheek plates either side of the socket, at the full level.
            if !b.fine() {
                continue;
            }
            for s in [-1.0, 1.0] {
                let g = Frame::new(
                    hip - out * 0.9 + across * (s * 0.78) + Vec3::Z * 0.15,
                    out - Vec3::Z * 0.4,
                    across * s,
                );
                b.with_facets(|b| armour(b, &g, &swept(1.3, 0.5, -0.4 * s, 0.55), 0.12));
            }
        }
    });
}

/// One rotor ring about the z axis, from `z0` to `z1`, `outer` to `inner` across, turning
/// at `rate` (rad/s, the sign its way): a drum of dark plate girdled by a thin band rimmed
/// violet (`NANITE_RIM`), and swept vanes lying on its top, their points trailing the way it
/// turns. At the reduced level, the drum alone.
fn rotor(b: &mut MeshBuilder, z0: f32, z1: f32, outer: f32, inner: f32, vanes: usize, rate: f32) {
    b.with_orbit(Vec3::Z, rate, |b| {
        let segs = if b.fine() { 12 } else { 8 };
        let (mid, w, h) = ((outer + inner) * 0.5, outer - inner, z1 - z0);
        b.with_facets(|b| {
            dark_plate(b);
            hoop(b, Vec3::Z * (z0 + h * 0.5), mid, w, h, segs);
            if b.fine() {
                b.paint(PLATING_DARK).pattern(pattern::NANITE_RIM);
                hoop(
                    b,
                    Vec3::Z * (z0 + h * 0.45),
                    outer + 0.03,
                    0.08,
                    h * 0.3,
                    segs,
                );
            }
        });
        if !b.fine() {
            return;
        }
        let way = rate.signum();
        for i in 0..vanes {
            let a = TAU * i as f32 / vanes as f32;
            let d = v3(a.cos(), a.sin(), 0.0);
            let along = v3(-a.sin(), a.cos(), 0.0) * way;
            if i % 3 == 0 {
                nanite_plate(b);
            } else {
                dark_plate(b);
            }
            let f = Frame::new(
                d * mid + Vec3::Z * (z1 - 0.02) - along * (w * 0.5),
                along + Vec3::Z * 0.2,
                Vec3::Z,
            );
            let len = (TAU * mid / vanes as f32 * 1.15).min(h * 2.6);
            b.with_facets(|b| armour(b, &f, &swept(len, w * 0.3, 0.6, 0.45), 0.08));
        }
    });
}

/// The well in the back, open: three rotor rings stacked in it, each smaller and higher,
/// neighbours turning opposite ways, and three prongs rising off its lip to hold the core.
fn rotor_well(b: &mut MeshBuilder) {
    b.set_spinner_pivot(Vec3::ZERO);
    let collar = BACK - 0.5;
    // The well's floor and its lip.
    dark_plate(b);
    let sides = b.sides(16);
    b.prism(v3(0.0, 0.0, BAND + 0.3), sides, 2.1, 2.1, 0.12);
    seam(b);
    hoop(b, Vec3::Z * (collar + 0.06), 2.2, 0.3, 0.16, sides);
    rotor(b, BAND + 0.45, collar - 0.15, 1.95, 1.4, 6, 0.9);
    rotor(b, collar, collar + 0.75, 1.55, 1.1, 5, -1.3);
    rotor(b, collar + 0.95, collar + 1.5, 1.15, 0.8, 4, 1.8);
    cradle(b, 2.15, collar + 0.05, 0.42);
}

/// Three tall prongs standing off a rim of radius `r` at `z`, bending at a knee and rising
/// on round the core to hold it between their points, glazed above the knee.
fn cradle(b: &mut MeshBuilder, r: f32, z: f32, width: f32) {
    for k in 0..3 {
        let a = (120.0 * k as f32 + 60.0f32).to_radians();
        let d = v3(a.cos(), a.sin(), 0.0);
        let root = d * r + Vec3::Z * z;
        let knee = d * 1.9 + Vec3::Z * (CORE.z - 1.4);
        let tip = d * 1.25 + Vec3::Z * (CORE.z + 0.7);
        dark_plate(b);
        spike(b, root, knee + (knee - root).normalize() * 0.3, width);
        nanite_plate(b);
        spike(b, knee, tip, width * 0.7);
    }
}
