//! The Naga land factory: a hall of muster on its 8 x 8 lot (96 m square).
//!
//! The unit forms on a round floor at the lot origin and marches out toward +x down a lit
//! processional way, between two obelisks. Nothing stands on the floor or in the way.
//!
//! - At the back, the portal: a tall wall under a pointed arch, a flared vault in front
//!   of it carried on three receding arches. In the wall a lit doorway, a slot of red
//!   light over it, and over that the Naga eye.
//! - Either side, a gallery on the dais: battered outside with a walkway ledge and flying
//!   buttresses on spired piers, sheer inside with slit windows and doors, highest at
//!   the back.
//! - Steps up the dais at the back and at the front of each gallery.

use glam::{Vec2, Vec3};

use crate::models::builder::MeshBuilder;
use crate::models::material::*;

use super::kit::{hide, metal, under_hide, v3};
use super::style::*;

/// The gallery walls' middle line, and where they run along x.
const WALL_Y: f32 = 25.0;
const WALL_X: [f32; 2] = [-24.0, 31.0];
/// Where the buttresses and vanes stand along the galleries.
const BAYS: [f32; 4] = [-16.0, -4.0, 8.0, 20.0];
/// The dais: its first step's top, its second's.
const DAIS: [f32; 2] = [1.0, 2.0];
/// The portal's back wall: its front face (x), half width and apex.
const WALL_FACE: f32 = -37.0;
const PORTAL: (f32, f32) = (15.0, 25.5);
/// The vault's sections, back to front: (x, half width, apex). The arches stand on the
/// last three.
const VAULT: [(f32, f32, f32); 4] = [
    (-37.0, 10.4, 19.4),
    (-33.5, 11.8, 21.3),
    (-30.5, 13.0, 23.0),
    (-28.0, 13.8, 24.2),
];

/// The gallery wall's height above the ground at `x`: highest at the back.
fn wall_top(x: f32) -> f32 {
    let t = ((x - WALL_X[0]) / (WALL_X[1] - WALL_X[0])).clamp(0.0, 1.0);
    10.0 - 4.0 * t
}

pub(super) fn hall(b: &mut MeshBuilder, _tech: u8) {
    if b.coarse() {
        hall_coarse(b);
        return;
    }
    hall_floor(b);
    hall_dais(b);
    portal(b);
    b.mirror_y(|b| {
        gallery(b);
        for x in BAYS {
            bay(b, x);
        }
        obelisk(b);
    });
}

/// Far off: the dais, the galleries, the portal's wall and its slot of light, the gate,
/// and the owner's colour.
fn hall_coarse(b: &mut MeshBuilder) {
    hide(b);
    b.frustum_open(
        v3(-34.5, 0.0, 0.0),
        Vec2::new(25.0, 76.0),
        Vec2::new(21.0, 68.0),
        DAIS[1],
        Vec2::ZERO,
    );
    let (half, apex) = PORTAL;
    let wall = [
        v3(WALL_FACE, -half, DAIS[1]),
        v3(WALL_FACE, half, DAIS[1]),
        v3(WALL_FACE, half, 14.0),
        v3(WALL_FACE, 0.0, apex),
        v3(WALL_FACE, -half, 14.0),
    ];
    b.face(&wall);
    b.face(&[wall[0], wall[4], wall[3], wall[2], wall[1]]);
    b.mirror_y(|b| {
        hide(b);
        b.frustum_open(
            v3(4.0, 28.0, 0.0),
            Vec2::new(58.0, 18.0),
            Vec2::new(54.0, 4.0),
            9.0,
            Vec2::new(-1.0, -3.0),
        );
        let gate = [
            v3(37.0, 15.0, 0.0),
            v3(40.0, 15.0, 0.0),
            v3(38.5, 14.0, 16.5),
        ];
        b.face(&gate);
        b.face(&[gate[0], gate[2], gate[1]]);
    });
    b.paint(GLOW_LASER);
    b.face(&[
        v3(WALL_FACE + 0.1, -0.6, DAIS[1]),
        v3(WALL_FACE + 0.1, 0.6, DAIS[1]),
        v3(WALL_FACE + 0.1, 0.6, 13.0),
        v3(WALL_FACE + 0.1, -0.6, 13.0),
    ]);
    b.paint(TEAM);
    b.face(&[
        v3(-40.0, -1.5, DAIS[1] + 0.05),
        v3(-26.0, -1.5, DAIS[1] + 0.05),
        v3(-26.0, 1.5, DAIS[1] + 0.05),
        v3(-40.0, 1.5, DAIS[1] + 0.05),
    ]);
}

/// The floor the unit forms on, flush with the ground, ringed in red; the processional way
/// out of it to the gate, and back to the portal.
fn hall_floor(b: &mut MeshBuilder) {
    hide(b);
    b.prism(Vec3::ZERO, b.sides(24), 13.0, 12.8, 0.12);
    metal(b);
    b.prism(Vec3::ZERO, b.sides(24), 10.6, 10.5, 0.16);
    if b.fine() {
        let n = 32;
        for k in 0..n {
            let a0 = k as f32 / n as f32 * std::f32::consts::TAU;
            let a1 = (k + 1) as f32 / n as f32 * std::f32::consts::TAU;
            let at = |a: f32| v3(a.cos() * 11.8, a.sin() * 11.8, 0.18);
            inlay(b, at(a0), at(a1), 0.16);
        }
    }
    for y in [-5.0, 5.0] {
        inlay(b, v3(13.0, y, 0.18), v3(44.0, y, 0.18), 0.16);
    }
    inlay(b, v3(-12.8, 0.0, 0.18), v3(-22.0, 0.0, 0.18), 0.16);
}

/// The dais under the portal and the galleries, two steps high, with stairs up it.
fn hall_dais(b: &mut MeshBuilder) {
    step(
        b,
        Vec2::new(-47.0, -38.0),
        Vec2::new(-22.0, 38.0),
        0.0,
        DAIS[0],
        0.4,
    );
    step(
        b,
        Vec2::new(-45.5, -34.0),
        Vec2::new(-24.0, 34.0),
        DAIS[0],
        DAIS[1],
        0.4,
    );
    stairs(b, v3(-19.6, 0.0, 0.0), Vec2::X, 12.0, DAIS[0], 4);
    b.mirror_y(|b| {
        step(
            b,
            Vec2::new(-22.5, 19.0),
            Vec2::new(34.0, 38.0),
            0.0,
            DAIS[0],
            0.4,
        );
        step(
            b,
            Vec2::new(-24.0, 21.0),
            Vec2::new(31.5, 34.0),
            DAIS[0],
            DAIS[1],
            0.4,
        );
        stairs(b, v3(36.6, 29.5, 0.0), Vec2::X, 6.0, DAIS[0], 4);
    });
}

/// The portal: the back wall under its pointed arch, the flared vault in front of it on
/// three receding arches, the doorway, the slot and the eye.
fn portal(b: &mut MeshBuilder) {
    let fine = b.fine();
    let n = if fine { 10 } else { 5 };
    let (half, apex) = PORTAL;
    hide(b);
    let outline: Vec<[f32; 2]> = ogive_outline(half, DAIS[1], apex, n, 0.0)
        .iter()
        .map(|p| [p.x, p.y])
        .collect();
    b.extrude_x(&outline, -45.0, WALL_FACE);
    // The vault: a shell of pointed arch section, flaring toward the front.
    let shell = 0.9;
    let rings: Vec<Vec<Vec3>> = VAULT
        .iter()
        .map(|&(x, half, apex)| {
            let mut ring = ogive_outline(half, DAIS[1], apex, n, shell * 0.5);
            let mut inner = ogive_outline(half, DAIS[1], apex, n, -shell * 0.5);
            inner.reverse();
            ring.append(&mut inner);
            ring.iter().map(|p| v3(x, p.x, p.y)).collect()
        })
        .collect();
    b.loft(&rings, true, true);
    // The arches it stands on, proud of it inside and out; the front one traced in red.
    for (k, &(x, half, apex)) in VAULT.iter().enumerate().skip(1) {
        let w = 1.3 + 0.25 * k as f32;
        for side in [-1.0, 1.0] {
            let leg = ogive_leg(side, half, DAIS[1], apex, n);
            if k == 1 || fine {
                hide(b);
                band_yz(b, x, &leg, w, 1.6);
            }
            if fine && k == VAULT.len() - 1 {
                b.paint(GLOW_LASER);
                let inside: Vec<Vec2> = ogive_outline(half, DAIS[1], apex, n, -w * 0.5 - 0.05)
                    .into_iter()
                    .filter(|p| p.x * side >= 0.0)
                    .collect();
                band_yz(b, x + 0.85, &inside, 0.2, 0.12);
            }
        }
    }
    // A spire either side of it, the tallest things in the hall.
    b.mirror_y(|b| {
        hide(b);
        b.block(v3(-43.5, 15.0, DAIS[1]), v3(-38.5, 20.0, DAIS[1] + 4.0));
        spire(b, v3(-41.0, 17.5, DAIS[1] + 4.0), 2.1, 20.0);
    });
    // A cornice where the wall meets the vault.
    if fine {
        metal(b);
        let top = ogive_outline(half, DAIS[1], apex, n, 0.0);
        band_yz(b, WALL_FACE + 0.1, &top, 0.7, 0.4);
    }
    // The doorway, lit, the slot of light over it, and the eye over that.
    let face = WALL_FACE + 0.06;
    metal(b);
    b.block(
        v3(WALL_FACE - 0.1, -2.3, DAIS[1]),
        v3(face + 0.3, 2.3, DAIS[1] + 5.2),
    );
    under_hide(b);
    b.block(
        v3(WALL_FACE - 0.2, -1.6, DAIS[1]),
        v3(face + 0.4, 1.6, DAIS[1] + 4.6),
    );
    b.paint(GLOW_LASER);
    b.beam(
        v3(face + 0.1, 0.0, DAIS[1] + 6.0),
        v3(face + 0.1, 0.0, DAIS[1] + 12.5),
        Vec2::new(0.9, 0.25),
        Vec2::new(0.5, 0.25),
    );
    if fine {
        inlay(
            b,
            v3(face + 0.45, -1.7, DAIS[1] + 4.75),
            v3(face + 0.45, 1.7, DAIS[1] + 4.75),
            0.2,
        );
        eye(b, face, DAIS[1] + 14.3);
    }
    b.paint(TEAM);
    b.face(&[
        v3(-44.0, -1.3, DAIS[1] + 0.02),
        v3(-40.0, -1.3, DAIS[1] + 0.02),
        v3(-40.0, 1.3, DAIS[1] + 0.02),
        v3(-44.0, 1.3, DAIS[1] + 0.02),
    ]);
}

/// A spire standing at `foot`: an octagonal shaft `r` across, a collar of metal, and a
/// needle over it, `height` tall in all.
fn spire(b: &mut MeshBuilder, foot: Vec3, r: f32, height: f32) {
    let sides = b.sides(8);
    hide(b);
    b.prism(foot, sides, r, r * 0.8, height * 0.45);
    let neck = foot + Vec3::Z * (height * 0.45);
    if b.fine() {
        metal(b);
        b.prism(neck, sides, r * 0.95, r * 0.95, height * 0.04);
    }
    hide(b);
    b.prism(
        neck + Vec3::Z * (height * 0.04),
        sides,
        r * 0.8,
        0.0,
        height * 0.51,
    );
}

/// The Naga eye inlaid on a wall facing +x at `face`, centred at height `z`: a lens with a
/// slit pupil.
fn eye(b: &mut MeshBuilder, face: f32, z: f32) {
    let (w, h) = (3.0, 1.2);
    let n = 8;
    for lid in [-1.0, 1.0] {
        let pts: Vec<Vec3> = (0..=n)
            .map(|i| {
                let y = -w + 2.0 * w * i as f32 / n as f32;
                v3(face + 0.05, y, z + lid * h * (1.0 - (y / w).powi(2)))
            })
            .collect();
        for p in pts.windows(2) {
            inlay(b, p[0], p[1], 0.22);
        }
    }
    b.paint(GLOW_LASER);
    b.beam(
        v3(face + 0.05, 0.0, z - h * 0.8),
        v3(face + 0.05, 0.0, z + h * 0.8),
        Vec2::new(0.4, 0.2),
        Vec2::new(0.4, 0.2),
    );
}

/// One gallery on the dais (+y side): battered outside, stepped back above a walkway
/// ledge, round crowned and banded in the owner's colour; sheer inside, with an inlaid
/// line under the crown, lit slit windows and doors.
fn gallery(b: &mut MeshBuilder) {
    let xs: Vec<f32> = if b.fine() {
        (0..=11)
            .map(|k| WALL_X[0] + (WALL_X[1] - WALL_X[0]) * k as f32 / 11.0)
            .collect()
    } else {
        vec![WALL_X[0], 3.5, WALL_X[1]]
    };
    let section = |x: f32| -> Vec<Vec3> {
        let h = wall_top(x);
        [
            [-2.5, DAIS[1]],
            [5.0, DAIS[1]],
            [4.2, h - 3.2],
            [2.6, h - 2.4],
            [1.2, h - 2.4],
            [1.2, h],
            [0.6, h + 0.7],
            [-0.8, h + 0.9],
            [-2.0, h + 0.5],
            [-2.5, h - 0.4],
        ]
        .iter()
        .map(|[y, z]| v3(x, WALL_Y + y, *z))
        .collect()
    };
    hide(b);
    let rings: Vec<Vec<Vec3>> = xs.iter().map(|&x| section(x)).collect();
    b.loft(&rings, true, true);
    b.paint(TEAM);
    for w in xs.windows(2) {
        let (x0, x1) = (w[0], w[1]);
        b.face(&[
            v3(x0, WALL_Y - 0.7, wall_top(x0) + 0.93),
            v3(x1, WALL_Y - 0.7, wall_top(x1) + 0.93),
            v3(x1, WALL_Y + 0.2, wall_top(x1) + 0.93),
            v3(x0, WALL_Y + 0.2, wall_top(x0) + 0.93),
        ]);
    }
    if !b.fine() {
        return;
    }
    let face = WALL_Y - 2.55;
    for w in xs.windows(2) {
        inlay(
            b,
            v3(w[0], face, wall_top(w[0]) - 1.3),
            v3(w[1], face, wall_top(w[1]) - 1.3),
            0.14,
        );
    }
    for k in 0..12 {
        let x = -21.0 + k as f32 * 4.4;
        if (x + 10.0).abs() < 2.5 || (x - 14.0).abs() < 2.5 {
            continue;
        }
        slit(b, v3(x, face, DAIS[1] + 2.3), -Vec3::Y, 0.45, 1.7);
    }
    for x in [-10.0, 14.0] {
        door(b, x, face);
    }
}

/// A door in the gallery's inner face at `x`: a dark leaf in a trimmed frame, lit over its head.
fn door(b: &mut MeshBuilder, x: f32, face: f32) {
    let z0 = DAIS[1];
    metal(b);
    b.block(
        v3(x - 1.8, face - 0.25, z0),
        v3(x + 1.8, face + 0.1, z0 + 3.9),
    );
    under_hide(b);
    b.block(
        v3(x - 1.3, face - 0.35, z0),
        v3(x + 1.3, face - 0.2, z0 + 3.4),
    );
    inlay(
        b,
        v3(x - 1.4, face - 0.4, z0 + 3.65),
        v3(x + 1.4, face - 0.4, z0 + 3.65),
        0.18,
    );
}

/// One bay of a gallery at `x` (+y side): a flying buttress from the dais up to the ledge
/// outside, a spire on the pier at its foot.
fn bay(b: &mut MeshBuilder, x: f32) {
    let fine = b.fine();
    let h = wall_top(x);
    let n = if fine { 8 } else { 4 };
    hide(b);
    let arc: Vec<Vec2> = curve(
        v3(36.4, DAIS[0], 0.0),
        v3(36.0, h - 2.6, 0.0),
        v3(29.3, h - 3.2, 0.0),
        n,
    )
    .iter()
    .map(|p| p.truncate())
    .collect();
    band_yz(b, x, &arc, 1.1, 1.4);
    // Its foot: a pier, a spire standing on it.
    b.block(v3(x - 1.1, 35.4, DAIS[0]), v3(x + 1.1, 37.6, DAIS[0] + 3.0));
    spire(b, v3(x, 36.5, DAIS[0] + 3.0), 1.1, h + 1.5 - DAIS[0]);
}

/// One of the gate's two obelisks at the end of the processional way (+y side), on its own
/// plinth: a tapering shaft leaning a little in over the way, collared in metal, capped
/// with a point, its inner face inlaid.
fn obelisk(b: &mut MeshBuilder) {
    let n = if b.fine() { 8 } else { 4 };
    step(
        b,
        Vec2::new(35.0, 11.5),
        Vec2::new(42.0, 18.5),
        0.0,
        1.2,
        0.3,
    );
    let spine = curve(
        v3(38.5, 15.0, 1.1),
        v3(38.5, 15.0, 9.0),
        v3(38.5, 14.0, 15.0),
        n,
    );
    let (widths, thicks) = (taper(2.6, 1.6, n), taper(1.8, 1.1, n));
    hide(b);
    blade_sweep(b, &spine, &widths, &thicks, Vec3::X);
    // The cap.
    let top = spine[n];
    let lean = (spine[n] - spine[n - 1]).normalize();
    b.loft(
        &[
            vec![
                top + v3(widths[n], 0.0, 0.0),
                top + v3(0.0, thicks[n], 0.0),
                top - v3(widths[n], 0.0, 0.0),
                top - v3(0.0, thicks[n], 0.0),
            ],
            vec![top + lean * 2.2; 4],
        ],
        false,
        false,
    );
    if b.fine() {
        metal(b);
        let at = n / 3;
        b.cylinder_between(
            spine[at] - v3(0.0, 0.0, 0.35),
            spine[at] + v3(0.0, 0.0, 0.35),
            widths[at] * 1.08,
            widths[at] * 1.08,
            8,
        );
        let face: Vec<Vec3> = spine
            .iter()
            .zip(&thicks)
            .map(|(p, t)| *p - v3(0.0, t * 0.8 + 0.06, 0.0))
            .collect();
        for w in face[at + 1..].windows(2) {
            inlay(b, w[0], w[1], 0.22);
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_hall_fits_its_lot() {
        super::super::check("naga_brood", 46.0, 22.0, Some(8), &[]);
    }
}
