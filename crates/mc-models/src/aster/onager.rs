//! The Onager's gun and mount: a long round barrel on a breech that reaches back
//! past the trunnion, carried in a cradle between two trunnion towers.

use glam::{Vec2, Vec3};

use super::parts::*;
use super::structures::howitzer_bunker;
use crate::builder::MeshBuilder;
use crate::material::*;
use crate::{part, rig};

/// Barrel radius at the breech end of the tube.
const R: f32 = 0.84;
/// Barrel tube radius at the cradle: slim against the breech so it reads as a barrel.
const TUBE: f32 = 0.5;
/// Where the breech block ends and the cradle begins, along the barrel.
const BREECH_END: f32 = 3.9;
/// Cradle length along the barrel.
const CRADLE_LEN: f32 = 5.0;

/// The recoiling group along +x from the breech: a stepped, raked breech block, a long
/// tapering tube with a bore evacuator on it, a slotted muzzle brake. Only the tube is
/// round; the rest is cut from flats with raked ends and chamfers.
fn tube(b: &mut MeshBuilder, breech: Vec3, muzzle: Vec3) {
    let r = R;
    let d = muzzle - breech;
    let length = d.length();
    b.pitched(breech, d.z.atan2(d.truncate().length()), |b| {
        let at = |x: f32| Vec3::X * x;
        b.paint(PLATING);
        if b.coarse() {
            b.beam(
                Vec3::ZERO,
                Vec3::X * length,
                Vec2::splat(r * 2.6),
                Vec2::splat(r * 1.6),
            );
            return;
        }
        let n = b.sides(10);
        // Breech block: raked back, stepped top, undercut chin.
        b.extrude_y_chamfered(
            &[
                [0.4, -1.7 * r],
                [0.0, -0.9 * r],
                [0.0, 1.5 * r],
                [0.9, 2.2 * r],
                [2.5, 2.2 * r],
                [3.2, 1.8 * r],
                [BREECH_END, 1.6 * r],
                [BREECH_END, -1.6 * r],
                [3.0, -1.8 * r],
            ],
            1.4 * r,
            0.16,
        );
        b.paint(ACCENT);
        b.extrude_y_chamfered(
            &[
                [-0.04, -0.7 * r],
                [-0.04, 1.2 * r],
                [0.04, 1.2 * r],
                [0.04, -0.7 * r],
            ],
            1.0 * r,
            0.02,
        );
        // Tube: one taper from the cradle to the brake.
        let end = length * 0.945;
        b.cylinder_between(at(BREECH_END), at(end), TUBE, TUBE * 0.78, n);
        // Bore evacuator: a long swell, ramped at both ends.
        let e = length * 0.6;
        b.paint(PLATING);
        b.cylinder_between(at(e), at(e + 1.0), TUBE * 0.95, TUBE * 1.5, n);
        b.cylinder_between(at(e + 1.0), at(e + 2.7), TUBE * 1.5, TUBE * 1.45, n);
        b.cylinder_between(at(e + 2.7), at(e + 3.5), TUBE * 1.45, TUBE * 0.9, n);
        // Muzzle brake: raked top and bottom slabs, a web between, and the side
        // ports left open onto the dark core.
        let (x0, x1) = (end - 0.1, length);
        let tr = TUBE * 0.78;
        let (lo, hi, wy) = (0.7 * tr, 1.75 * tr, 1.55 * tr);
        b.paint(ACCENT);
        b.extrude_y_chamfered(
            &[
                [x0, -lo - 0.05],
                [x1 - 0.1, -lo - 0.05],
                [x1 - 0.1, lo + 0.05],
                [x0, lo + 0.05],
            ],
            1.3 * tr,
            0.03,
        );
        b.paint(PLATING);
        for sign in [-1.0f32, 1.0] {
            let (a, c) = (sign * lo, sign * hi);
            b.extrude_y_chamfered(&[[x0, a], [x1, a], [x1 - 0.18, c], [x0 + 0.1, c]], wy, 0.06);
        }
        let mid = (x0 + x1) * 0.5;
        b.chamfered_box(at(mid), v3(0.2, 2.0 * wy * 0.95, 2.0 * hi * 0.98), 0.05);
    });
}

/// Side profile of a trunnion tower about the pivot, in (x, z): wide at the foot,
/// narrowing to a rounded-off head that carries the bearing.
fn tower_profile(floor: f32, pivot: Vec3, inset: f32) -> Vec<[f32; 2]> {
    let top = pivot.z + 1.35;
    [
        [-3.4 + inset, floor],
        [3.4 - inset, floor],
        [2.4 - inset, floor + 1.7],
        [1.55 - inset, top - 0.8],
        [0.9 - inset * 0.5, top],
        [pivot.x - 0.9 + inset * 0.5, top],
        [-1.9 + inset, top - 0.9],
        [-2.6 + inset, floor + 1.7],
    ]
    .iter()
    .map(|p| [p[0], p[1]])
    .collect()
}

pub(super) fn onager(b: &mut MeshBuilder) {
    howitzer_bunker(b);
    // Authored at 14 / 12, fitted to the 10.5 / 9 lot (x0.75). The line runs through
    // the pivot, which sits a third of the way along the breech.
    let (breech, muzzle) = (v3(-5.9, 0.0, 6.48), v3(19.4, 0.0, 12.55));
    let elevation = (muzzle - breech).z.atan2((muzzle - breech).x);
    let pivot = v3(-0.4, 0.0, 7.8);
    let deck_z = 4.2;
    let r = R;
    b.set_turret_pivot(v3(0.0, 0.0, deck_z));
    b.set_arm_pivot(pivot);
    b.set_recoil(breech, muzzle, 2.8);
    b.with_part(part::TURRET, |b| {
        b.with_limb(rig::ARM_GUN, |b| {
            b.with_recoil(|b| tube(b, breech, muzzle));
        });
        if b.coarse() {
            b.paint(PLATING);
            b.frustum_open(
                v3(-0.35, 0.0, deck_z),
                v2(6.2, 4.8),
                v2(2.4, 1.8),
                pivot.z - deck_z - 1.2,
                v2(-0.05, 0.0),
            );
            team_panel(b, v3(pivot.x, 0.0, pivot.z - 1.0), v2(1.4, 0.7));
            return;
        }
        // Turntable the mount stands on; it yaws with the gun.
        b.paint(ACCENT);
        b.prism(v3(0.0, 0.0, deck_z), b.sides(8), 3.5, 3.4, 0.16);
        b.paint(PLATING);
        b.prism(v3(0.0, 0.0, deck_z + 0.16), b.sides(8), 3.35, 3.2, 0.14);
        let floor = deck_z + 0.3;
        // A tower each side: dark core, plated face, a bearing boss at the head.
        let inner = 1.7 * r + 0.15;
        b.mirror_y(|b| {
            b.paint(ACCENT);
            b.extrude_y(&tower_profile(floor, pivot, 0.0), inner, inner + 0.75);
            b.paint(PLATING);
            b.extrude_y_chamfered(
                &tower_profile(floor, pivot, 0.28)
                    .iter()
                    .map(|p| [p[0], p[1]])
                    .collect::<Vec<_>>(),
                inner + 0.5,
                0.1,
            );
            b.paint(ACCENT);
            b.cylinder_between(
                v3(pivot.x, inner + 0.7, pivot.z),
                v3(pivot.x, inner + 1.15, pivot.z),
                1.05,
                0.95,
                b.sides(10),
            );
        });
        // Base block tying the towers together under the breech.
        b.paint(ACCENT);
        b.chamfered_box(
            v3(0.0, 0.0, floor + 0.5),
            v3(5.6, 2.0 * inner + 0.6, 1.0),
            0.12,
        );
        team_panel(b, v3(0.0, 0.0, deck_z + 0.44), v2(1.8, 1.8));
        // The cradle and recuperators pitch with the tube; only the tube slides.
        b.with_limb(rig::ARM_GUN, |b| {
            b.pitched(breech, elevation, |b| {
                b.paint(PLATING);
                b.extrude_y_chamfered(
                    &[
                        [BREECH_END, -1.6 * r],
                        [BREECH_END + 0.5, -1.95 * r],
                        [BREECH_END + CRADLE_LEN - 0.5, -1.95 * r],
                        [BREECH_END + CRADLE_LEN, -1.5 * r],
                        [BREECH_END + CRADLE_LEN, 1.5 * r],
                        [BREECH_END + CRADLE_LEN - 0.8, 2.0 * r],
                        [BREECH_END + 0.6, 2.0 * r],
                        [BREECH_END, 1.5 * r],
                    ],
                    1.7 * r,
                    0.14,
                );
                if b.fine() {
                    b.paint(ACCENT);
                    b.mirror_y(|b| {
                        let x = BREECH_END + CRADLE_LEN;
                        b.chamfered_box(v3(x, 0.62, 0.52), v3(0.7, 0.5, 0.5), 0.08);
                        b.cylinder_between(
                            v3(x, 0.62, 0.52),
                            v3(x + 5.2, 0.62, 0.52),
                            0.14,
                            0.1,
                            6,
                        );
                    });
                }
            });
        });
    });
}
