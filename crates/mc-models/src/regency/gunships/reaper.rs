//! The Reaper, the Regency's tech 3 beam assault craft (`regency_t3_assault_aircraft`): a
//! heavy craft on six lift bells that flies runs at its mark and rakes it with two
//! Plasmeric beams from projectors fixed under it, one either side (the unit file's
//! `strafe`).
//!
//! Each projector (`projector`) is a plated housing with an emitter barrel, laid down a
//! little toward the ground ahead; its mouth is a muzzle of the unit file's `muzzles`.
//!
//! The shape is a scythe: a long plated hull under a high keel, and a crescent wing either
//! side swept back like a scythe's blade into a down-turned tip, three bells under each.

use glam::{Vec2, Vec3};

use crate::builder::{MeshBuilder, Section};
use crate::library::ModelDef;
use crate::part;

use super::super::kit::{dark_plate, metal, seam, v3};
use super::super::lift::bell;
use super::super::machine::{armour, collar, shaft, swept, Course, Frame};
use super::{body, head, plan, team_mark};

pub(crate) const MODELS: &[ModelDef] = &[
    ModelDef::new("regency_assault_aircraft", RADIUS, HEIGHT, scythe_roots),
    ModelDef::new(
        "regency_assault_aircraft~cheeks",
        RADIUS,
        HEIGHT,
        scythe_cheeks,
    ),
    ModelDef::new(
        "regency_assault_aircraft~blades",
        RADIUS,
        HEIGHT,
        scythe_blades,
    ),
];

pub(crate) const RADIUS: f32 = 14.0;
pub(crate) const HEIGHT: f32 = 6.0;

/// Where the two projectors hang (the port one; the starboard mirrors it): the mouth (the
/// unit file's muzzle) and how long the projector is back from it.
#[derive(Clone, Copy)]
struct Mount {
    mouth: Vec3,
    length: f32,
}

/// Under the wing roots, reaching out ahead of the leading edge.
const ROOTS: Mount = Mount {
    mouth: Vec3::new(5.0, 3.0, 0.9),
    length: 3.6,
};
/// Close under the nose's cheeks, either side of the chin.
const CHEEKS: Mount = Mount {
    mouth: Vec3::new(7.4, 1.6, 0.65),
    length: 3.2,
};
/// Out under the blades, between the first and second bells.
const BLADES: Mount = Mount {
    mouth: Vec3::new(1.6, 6.4, 1.0),
    length: 3.4,
};

/// How far the projectors are laid down off level, toward the ground their beams land on.
const DROOP: f32 = 0.2;

/// One beam projector, fixed: a plated housing hung on a web from the hull above, a bronze
/// emitter barrel ringed with collars, and a heavy ring round its mouth at `mouth`.
fn projector(b: &mut MeshBuilder, mouth: Vec3, length: f32) {
    let dir = v3(DROOP.cos(), 0.0, -DROOP.sin());
    let back = mouth - dir * length;
    dark_plate(b);
    if b.coarse() {
        // Far off: a wedge from the housing's back to the mouth.
        let (top, port, star) = (
            back + Vec3::Z * 0.35,
            back + v3(0.0, 0.4, -0.3),
            back + v3(0.0, -0.4, -0.3),
        );
        b.face(&[mouth, top, port]);
        b.face(&[mouth, star, top]);
        b.face(&[mouth, port, star]);
        b.face(&[top, star, port]);
        return;
    }
    let housing = back + dir * length * 0.62;
    b.beam(back, housing, Vec2::new(0.8, 0.7), Vec2::new(0.62, 0.56));
    // The web it hangs from, up into the hull or wing over it.
    let web = back + dir * length * 0.3;
    b.beam(
        web + Vec3::Z * 0.2,
        v3(web.x, web.y, 1.5),
        Vec2::new(0.16, length * 0.4),
        Vec2::new(0.16, length * 0.5),
    );
    metal(b);
    let sides = b.sides(8);
    b.cylinder_between(housing - dir * 0.1, mouth - dir * 0.12, 0.17, 0.15, sides);
    for k in [0.72f32, 0.84] {
        collar(b, back + dir * length * k, dir, 0.24, 0.08);
    }
    collar(b, mouth - dir * 0.1, dir, 0.26, 0.12);
}

/// Both projectors, port and starboard: the guns (`part::TURRET`), though they never turn.
fn projectors(b: &mut MeshBuilder, mount: Mount) {
    b.with_part(part::TURRET, |b| {
        b.mirror_y(|b| projector(b, mount.mouth, mount.length));
    });
}

/// Far off: a plan's flat solid, the team colour on top, one dark face for the bells, and
/// the projectors.
fn coarse(b: &mut MeshBuilder, outline: &[[f32; 2]], top: f32, mount: Mount) {
    dark_plate(b);
    b.extrude_z(outline, 1.0, top);
    team_mark(b, -1.0, top, 2.4);
    b.with_part(part::LOCOMOTION, |b| {
        seam(b);
        b.face(&[v3(4.0, 0.0, 0.3), v3(-5.0, -4.0, 0.3), v3(-5.0, 4.0, 0.3)]);
    });
    projectors(b, mount);
}

/// A plated keel along the back from `fore` to `aft`, `top` high, lapped in a course of
/// plates that runs past its end into a spike.
fn keel(b: &mut MeshBuilder, fore: f32, aft: f32, deck: f32, top: f32) {
    dark_plate(b);
    b.with_facets(|b| {
        b.loft_z(
            &plan(&[[fore, 0.0], [fore - 1.2, 0.7], [aft + 1.0, 0.8], [aft, 0.0]]),
            &[Section::new(deck - 0.1, 1.0), Section::new(top, 0.7)],
        )
    });
    let len = fore - aft;
    let f = Frame::new(v3(fore - 0.6, 0.0, top), v3(-1.0, 0.0, -0.03), Vec3::Z);
    Course {
        count: if b.fine() { 4 } else { 2 },
        step: if b.fine() { len / 4.4 } else { len / 2.2 },
        len: len / 3.6,
        half: 0.55,
        tip: 0.0,
        thick: 0.18,
        tail: 2.0,
    }
    .lay(b, &f);
}

const SCYTHE_HULL: [[f32; 2]; 6] = [
    [8.6, 0.0],
    [6.6, 1.2],
    [3.0, 2.0],
    [-4.0, 2.1],
    [-8.2, 1.2],
    [-9.6, 0.0],
];

/// The crescent wing's outline, root to tip along the leading edge and back.
const SCYTHE_WING: [[f32; 2]; 9] = [
    [3.2, 1.8],
    [1.6, 5.0],
    [-1.4, 8.4],
    [-5.4, 11.4],
    [-9.8, 12.6],
    [-8.2, 10.2],
    [-6.6, 7.4],
    [-6.2, 4.4],
    [-6.6, 1.8],
];

fn scythe_roots(b: &mut MeshBuilder, _tech: u8) {
    scythe(b, ROOTS);
}

fn scythe_cheeks(b: &mut MeshBuilder, _tech: u8) {
    scythe(b, CHEEKS);
}

fn scythe_blades(b: &mut MeshBuilder, _tech: u8) {
    scythe(b, BLADES);
}

fn scythe(b: &mut MeshBuilder, mount: Mount) {
    if b.coarse() {
        let outline = plan(&[
            [8.6, 0.0],
            [3.2, 2.0],
            [-5.4, 11.4],
            [-9.8, 12.6],
            [-6.4, 4.0],
            [-9.6, 0.0],
        ]);
        coarse(b, &outline, 4.9, mount);
        return;
    }
    body(b, &SCYTHE_HULL, 1.0, 3.8, 0.82);
    keel(b, 5.6, -8.6, 3.7, 5.0);
    head(b, 8.6, 2.6, 1.5);
    team_mark(b, -1.5, 5.18, 0.7);
    b.mirror_y(|b| {
        seam(b);
        b.with_facets(|b| b.extrude_z(&SCYTHE_WING, 1.35, 1.6));
        dark_plate(b);
        b.with_facets(|b| {
            b.loft_z(
                &SCYTHE_WING,
                &[Section::new(1.6, 1.0), Section::new(2.15, 1.0)],
            )
        });
        // Plates lapped back along the blade, the last a spike past the tip.
        let f = Frame::new(v3(1.4, 4.4, 2.15), v3(-0.62, 0.78, 0.0), Vec3::Z);
        Course {
            count: if b.fine() { 3 } else { 2 },
            step: if b.fine() { 3.1 } else { 6.2 },
            len: 3.4,
            half: 0.9,
            tip: -0.6,
            thick: 0.18,
            tail: 1.6,
        }
        .lay(b, &f);
        // The tip turned down into a blade.
        armour(
            b,
            &Frame::new(v3(-8.6, 11.6, 1.6), v3(-0.5, 0.3, -0.8), v3(0.2, 1.0, 0.1)),
            &swept(1.5, 0.6, 0.3, 0.5),
            0.16,
        );
        // The lift's machinery along the wing root, in bronze.
        if b.fine() {
            metal(b);
            shaft(b, v3(1.0, 2.2, 1.5), v3(-5.6, 2.3, 1.5), 0.16);
            collar(b, v3(-2.0, 2.25, 1.5), Vec3::X, 0.26, 0.2);
        }
        bell(b, v3(0.2, 4.6, 0.0), 1.0, 1.4);
        bell(b, v3(-3.3, 7.9, 0.0), 0.95, 1.4);
        bell(b, v3(-6.9, 10.6, 0.0), 0.85, 1.4);
    });
    projectors(b, mount);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_design_fits_the_librarys_checks() {
        for (key, mount) in MODELS.iter().map(|d| d.key).zip([ROOTS, CHEEKS, BLADES]) {
            let starboard = mount.mouth * v3(1.0, -1.0, 1.0);
            let muzzles = [mount.mouth.to_array(), starboard.to_array()];
            super::super::super::check(key, RADIUS, HEIGHT, None, &muzzles);
            let model = crate::build_model(key).unwrap();
            assert_eq!(model.lifts.len(), 6, "{key}: six lift bells");
        }
    }
}
