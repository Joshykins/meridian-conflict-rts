//! The spacecraft rig shared by Aster capital ships (the Bastion first): landing legs that
//! swing out of flush belly bays, stern drives with glowing bells, downward lift jets, and
//! rotary cannons in houses of their own.
//!
//! A ship describes its rig in a [`CapitalRig`], builds the pieces here at the places the
//! rig names, and registers the rig in `models::capital_rig` by mesh key. The renderer puts
//! [`CapitalRig::gpu`] in `ModelInfo::capital`, and `entity.wgsl` animates from that alone:
//! - legs (`part::GEAR`, `GEAR_STRUT`, `GEAR_FOOT`) and doors (`GEAR_DOOR`) over the gear value
//!   (`mirror::UNIT_GEAR_SHIFT`); a hovering ship with legs stops heaving once they are down;
//! - the drives' glow with the ship's speed, the lift jets' with its climb or descent, and
//!   the iris vanes (`part::DRIVE`) about each drive's axis;
//! - the belly ramp (`part::RAMP`) about `ramp`, for a ship with one;
//! - rotary barrels in a house (`rig::SPIN` inside `with_house`) turn while that gun fires.
use std::f32::consts::TAU;

use glam::{Affine3A, Vec3};

use super::*;

/// One pair of landing legs (the -y leg mirrors the +y one).
#[derive(Clone, Copy, Debug)]
pub struct Leg {
    /// Where the +y leg swings, model space. A leg of `size` 1 has its hinge 36 m up.
    pub hinge: [f32; 3],
    /// Which way the foot goes as the leg stows: +1 aft, -1 forward.
    pub stow: f32,
    /// The bay under the leg: x from, x to, |y| of the inner and outer door hinges.
    pub bay: [f32; 4],
    /// 1 for the Bastion's legs; everything about the leg scales with it.
    pub size: f32,
}

/// Everything `entity.wgsl` needs to animate a spacecraft (see the module comment).
#[derive(Clone, Copy, Debug)]
pub struct CapitalRig {
    /// Fore and aft pairs of legs, or none.
    pub legs: Option<[Leg; 2]>,
    /// Height of the bay doors' hinges, just under the keel.
    pub door_hinge: f32,
    /// Stern drives: mouth x, axis height, |y| of the inner and of the outer pair (the same
    /// for a single pair), and their `size` (1: the Bastion's 12 m bells).
    pub drives: Option<([f32; 4], f32)>,
    /// Lift jets: fore x, |y|, aft x, |y|, and their mouths' height.
    pub lift_jets: Option<([f32; 4], f32)>,
    /// The belly ramp's hinge (x, z), for a ship with one (`part::RAMP`).
    pub ramp: Option<[f32; 2]>,
}

impl CapitalRig {
    /// The rig as `ModelInfo::capital` carries it (layout in `common.wgsl`).
    pub fn gpu(&self) -> [[f32; 4]; 7] {
        let mut out = [[0.0; 4]; 7];
        if let Some([fore, aft]) = self.legs {
            for (i, leg) in [fore, aft].iter().enumerate() {
                out[i] = [leg.hinge[0], leg.hinge[1], leg.hinge[2], leg.stow];
            }
            out[2] = [fore.bay[2], fore.bay[3], aft.bay[2], aft.bay[3]];
            out[3] = [self.door_hinge, fore.size, aft.size, 0.0];
        }
        if let Some((d, size)) = self.drives {
            out[4] = d;
            out[6][1] = size;
        }
        if let Some((j, z)) = self.lift_jets {
            out[5] = j;
            out[6][0] = z;
        }
        if let Some([x, z]) = self.ramp {
            out[6][2] = x;
            out[6][3] = z;
        }
        out
    }
}

/// A leg of `size` 1 splays this far out at the foot, and its strut telescopes this far.
pub const LEG_SPLAY: f32 = 3.8;
#[allow(dead_code)] // the shader and `stowed` carry the travel
pub const LEG_TRAVEL: f32 = 12.0;
/// The hinge of a leg of `size` 1, above the ground under its foot.
pub const LEG_HINGE: f32 = 36.0;

/// A solid of revolution about the line through `c` along `axis` (+X or -Z): `profile`
/// is a closed outline of (distance along the axis, radius) points, no radius zero.
pub fn lathe(b: &mut MeshBuilder, c: Vec3, axis: Vec3, profile: &[[f32; 2]], n: usize) {
    let (u, w) = if axis.x.abs() > 0.5 { (Vec3::Y, Vec3::Z) } else { (Vec3::X, Vec3::Y) };
    let mut rings: Vec<Vec<Vec3>> = profile
        .iter()
        .map(|&[t, r]| {
            (0..n)
                .map(|k| {
                    let a = (k as f32 + 0.5) * TAU / n as f32;
                    c + axis * t + (u * a.cos() + w * a.sin()) * r
                })
                .collect()
        })
        .collect();
    rings.push(rings[0].clone());
    b.loft(&rings, false, false);
}

/// A thin plate standing out radially from the x axis through `c`: a cooling fin or a
/// petal, from `x0` to `x1` (relative to `c`), `r0..r1` out, `thick` across.
pub fn fin(b: &mut MeshBuilder, c: Vec3, angle: f32, x0: f32, x1: f32, r0: f32, r1: [f32; 2], thick: f32) {
    let radial = v3(0.0, angle.cos(), angle.sin());
    let side = v3(0.0, -angle.sin(), angle.cos()) * (thick * 0.5);
    let ring = |x: f32, r1: f32| {
        vec![
            c + v3(x, 0.0, 0.0) + radial * r0 - side,
            c + v3(x, 0.0, 0.0) + radial * r0 + side,
            c + v3(x, 0.0, 0.0) + radial * r1 + side,
            c + v3(x, 0.0, 0.0) + radial * r1 - side,
        ]
    };
    b.loft(&[ring(x0, r1[0]), ring(x1, r1[1])], true, true);
}

/// A closed annular shell about the x axis through `c`, from `x0` to `x1`.
pub fn collar(b: &mut MeshBuilder, c: Vec3, x0: f32, x1: f32, outer: [f32; 2], inner: [f32; 2], n: usize) {
    lathe(b, c, Vec3::X, &[[x0, inner[0]], [x0, outer[0]], [x1, outer[1]], [x1, inner[1]]], n);
}

/// Runs `f` scaled by `size` about `at` (the pieces here are authored at the Bastion's size).
fn sized(b: &mut MeshBuilder, at: Vec3, size: f32, f: impl FnOnce(&mut MeshBuilder)) {
    b.with(Affine3A::from_translation(at) * Affine3A::from_scale(Vec3::splat(size)), f);
}

/// One stern drive, its mouth at `c` (on the rig's drive row), facing aft: a finned can, a
/// gimbal ring, and a flared bell lined with glow that the shader keeps dark while the
/// drive is cold and burns hotter the deeper it goes under thrust; iris vanes
/// (`part::DRIVE`) turn in the throat. The bell is 12 m across its mouth at `size` 1,
/// its throat 17 m in; the can runs 35 m forward of the mouth.
pub fn drive(b: &mut MeshBuilder, c: Vec3, size: f32) {
    sized(b, c, size, |b| {
        let c = Vec3::ZERO;
        let n = if b.fine() { 16 } else { 8 };
        b.paint(METAL);
        b.cylinder_between(c + v3(35.0, 0.0, 0.0), c + v3(20.5, 0.0, 0.0), 10.4, 10.4, n);
        b.paint(ACCENT);
        collar(b, c, 20.8, 17.2, [12.0, 11.4], [8.4, 8.4], n);
        b.paint(PLATING_DARK);
        lathe(
            b,
            c,
            Vec3::X,
            &[[17.4, 9.2], [12.0, 10.2], [0.6, 12.2], [-0.8, 12.1], [-0.8, 11.7], [11.0, 9.1], [16.6, 8.2]],
            n,
        );
        b.paint(GLOW);
        lathe(b, c, Vec3::X, &[[16.6, 8.2], [11.0, 9.1], [-0.5, 11.65], [-0.5, 11.25], [11.0, 8.7], [16.6, 7.8]], n);
        b.cylinder_between(c + v3(17.2, 0.0, 0.0), c + v3(16.6, 0.0, 0.0), 7.9, 7.9, n);
        if b.fine() {
            // A heat ring: a band of hot metal round the bell's neck.
            b.paint(GLOW_ORANGE);
            let r = 10.2 - 0.18;
            collar(b, c, 13.6, 12.4, [r + 0.3, r + 0.4], [r - 0.4, r - 0.3], n);
        }
        b.with_part(part::DRIVE, |b| {
            b.paint(METAL);
            b.cylinder_between(c + v3(16.5, 0.0, 0.0), c + v3(9.0, 0.0, 0.0), 2.4, 0.9, b.sides(8));
            if b.mid() {
                let vanes = if b.fine() { 6 } else { 3 };
                for k in 0..vanes {
                    let a = k as f32 * TAU / vanes as f32;
                    fin(b, c, a, 15.8, 14.8, 2.0, [7.3, 7.0], 0.7);
                }
            }
        });
        if b.mid() {
            // Cooling fins down the can.
            b.paint(ACCENT);
            let fins = if b.fine() { 12 } else { 6 };
            for k in 0..fins {
                let a = (k as f32 + 0.5) * TAU / fins as f32;
                fin(b, c, a, 27.6, 21.0, 10.3, [13.4, 12.2], 0.6);
            }
        }
        if b.fine() {
            // Actuator rams from the frame to the bell.
            b.paint(METAL);
            for a in [0.8f32, 3.95] {
                let d = v3(0.0, a.cos(), a.sin());
                b.cylinder_between(c + v3(31.0, 0.0, 0.0) + d * 12.8, c + v3(16.0, 0.0, 0.0) + d * 10.6, 0.8, 0.55, 6);
            }
        }
    });
}

/// A downward lift jet, its mouth centre at `c` (on the rig's lift jets): a gimballed bell
/// in a collar reaching 3.2 m up into the hull (at `size` 1), its throat glowing inside.
pub fn lift_jet(b: &mut MeshBuilder, c: Vec3, size: f32) {
    sized(b, c, size, |b| {
        let c = Vec3::ZERO;
        let n = if b.fine() { 10 } else { 6 };
        if b.fine() {
            b.paint(ACCENT);
            lathe(b, c, -Vec3::Z, &[[-3.2, 6.6], [-1.8, 6.6], [-1.8, 5.4], [-3.2, 5.4]], n);
        }
        b.paint(PLATING_DARK);
        lathe(b, c, -Vec3::Z, &[[-2.9, 4.6], [0.0, 5.4], [0.0, 4.8], [-2.7, 3.6]], n);
        b.paint(GLOW);
        b.cylinder_between(c + v3(0.0, 0.0, 2.75), c + v3(0.0, 0.0, 2.4), 3.7, 3.7, n);
        if b.fine() {
            b.paint(METAL);
            for k in 0..4 {
                let a = k as f32 * TAU / 4.0 + 0.4;
                let d = v3(a.cos(), a.sin(), 0.0);
                b.beam(c + d * 0.6 + v3(0.0, 0.0, 1.2), c + d * 4.5 + v3(0.0, 0.0, 1.0), v2(0.4, 0.8), v2(0.4, 0.8));
            }
        }
    });
}

/// Both legs of each pair in `rig.legs`, authored fully out, with their bays' doors shut
/// just under the keel (`door_sill` their outer face) and a dark well inside.
pub fn gear(b: &mut MeshBuilder, rig: &CapitalRig, door_sill: f32) {
    let Some(legs) = rig.legs else { return };
    let hinge = rig.door_hinge;
    b.mirror_y(|b| {
        for leg in legs {
            let [x0, x1, y0, y1] = leg.bay;
            let top = hinge + (hinge - door_sill) + 0.05;
            b.paint(ACCENT).pattern(pattern::NONE);
            let well = hinge + 0.05;
            b.face(&[v3(x0, y0, well), v3(x0, y1, well), v3(x1, y1, well), v3(x1, y0, well)]);
            b.with_part(part::GEAR_DOOR, |b| {
                b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
                let mid = (y0 + y1) * 0.5;
                for (a, c) in [(y0, mid - 0.1), (mid + 0.1, y1)] {
                    b.block(v3(x0, a, door_sill), v3(x1, c, top));
                }
            });
            let [hx, hy, hz] = leg.hinge;
            // The leg is authored at size 1 with its hinge LEG_HINGE up over its foot.
            let foot = v3(hx, hy, hz - LEG_HINGE * leg.size);
            sized(b, foot, leg.size, |b| leg_at_size_one(b, leg.stow));
        }
    });
}

/// One leg at size 1: hinge at (0, 0, 36), foot on z 0 at y `LEG_SPLAY`.
fn leg_at_size_one(b: &mut MeshBuilder, dir: f32) {
    let hz = LEG_HINGE;
    b.with_part(part::GEAR, |b| {
        // Hinge trunnion, the armoured main leg, and the shock strut's cylinder.
        b.paint(METAL);
        b.cylinder_between(v3(0.0, -4.5, hz), v3(0.0, 4.5, hz), 3.4, 3.4, b.sides(10));
        b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
        b.beam(v3(0.0, 0.2, hz), v3(0.0, 2.0, 18.0), v2(7.0, 8.0), v2(6.0, 7.0));
        b.paint(ACCENT);
        b.cylinder_between(v3(0.0, 1.9, 19.5), v3(0.0, 2.6, 12.5), 3.0, 3.0, b.sides(10));
        // Drag brace, raked back to the side the leg stows away from.
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        b.beam(v3(-6.5 * dir, 0.3, 33.0), v3(-1.5 * dir, 1.6, 20.0), v2(2.6, 2.8), v2(2.2, 2.4));
        if b.mid() {
            // A pair of hydraulic rams down the leg's faces, and a gland ring.
            b.paint(METAL);
            for dx in [-4.3, 4.3] {
                b.cylinder_between(v3(dx, 0.6, 32.0), v3(dx, 1.7, 21.0), 0.9, 0.9, 6);
            }
            b.cylinder_between(v3(0.0, 2.5, 13.4), v3(0.0, 2.6, 12.2), 3.4, 3.4, b.sides(10));
        }
        if b.fine() {
            b.paint(GLOW_AMBER);
            b.cuboid(v3(4.1 * dir, 1.4, 26.0), v3(0.3, 1.0, 1.0));
        }
    });
    b.with_part(part::GEAR_STRUT, |b| {
        b.paint(METAL);
        b.cylinder_between(v3(0.0, 2.4, 17.0), v3(0.0, 3.6, 4.2), 1.9, 1.9, b.sides(10));
        b.paint(ACCENT);
        b.cuboid(v3(0.0, LEG_SPLAY, 3.8), v3(4.6, 4.6, 2.4));
    });
    b.with_part(part::GEAR_FOOT, |b| {
        b.paint(ACCENT);
        b.prism(v3(0.0, LEG_SPLAY, 2.0), b.sides(8), 3.2, 2.8, 1.4);
        // Two broad armoured pads (dark plating: the shader folds only these) that fold
        // up against the strut about their inner top edges (x ±1, z 2.4).
        b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
        for side in [-1.0, 1.0] {
            b.frustum(v3(side * 4.6, LEG_SPLAY, 0.0), v2(7.2, 10.0), v2(6.2, 9.0), 2.4, v2(-side * 0.4, 0.0));
        }
    });
}

/// Where a leg vertex (`part::GEAR`, `GEAR_STRUT`, `GEAR_FOOT`) ends up with the gear
/// stowed: the same moves `entity.wgsl` makes at gear 0. For tests.
#[cfg(test)]
pub fn stowed(rig: &CapitalRig, p: Vec3, part_id: u32, material: u32) -> Vec3 {
    let legs = rig.legs.unwrap();
    let mid = (legs[0].hinge[0] + legs[1].hinge[0]) * 0.5;
    let leg = legs[if p.x > mid { 0 } else { 1 }];
    let k = leg.size;
    let side = p.y.signum();
    let rot_y = |d: Vec3, a: f32| v3(d.x * a.cos() + d.z * a.sin(), d.y, -d.x * a.sin() + d.z * a.cos());
    let mut q = p;
    if part_id == part::GEAR_FOOT && material == PLATING_DARK {
        let d = (p.x - leg.hinge[0]).signum();
        let pad = v3(leg.hinge[0] + d * k, 0.0, leg.hinge[2] - (LEG_HINGE - 2.4) * k);
        q = rot_y(q - pad, -d * 1.4) + pad;
    }
    if part_id != part::GEAR {
        q += v3(0.0, -1.5 * side, 13.2).normalize() * LEG_TRAVEL * k;
    }
    let h = v3(leg.hinge[0], leg.hinge[1] * side, leg.hinge[2]);
    rot_y(q - h, leg.stow * std::f32::consts::FRAC_PI_2) + h
}

/// A big rotary cannon in its armoured house, bound to weapon `weapon` and turning about
/// `pivot`, authored facing +x with its muzzles `12 * size` ahead of the pivot (the unit
/// file's `muzzle`). The barrel cluster turns while the gun fires; nothing kicks back.
/// `hang`: slung under the hull, the house hangs below the post it turns on. The post is
/// slim, so the gun depresses (or, slung, elevates) to the pitch limit without striking
/// it; keep anything else under the barrels' sweep below `pivot.z - 6.5 * size`.
pub fn rotary_house(b: &mut MeshBuilder, weapon: usize, pivot: Vec3, hang: bool, size: f32) {
    b.with_house(weapon, pivot, 0.0, |b| {
        sized(b, pivot, size, |b| rotary_gun(b, hang));
    });
}

fn rotary_gun(b: &mut MeshBuilder, hang: bool) {
    let pivot = Vec3::ZERO;
    let s = if hang { -1.0 } else { 1.0 };
    let n = b.sides(12);
    b.paint(ACCENT);
    if hang {
        b.prism(pivot + Vec3::Z * 3.3, n, 3.5, 3.5, 1.8);
    } else {
        b.prism(pivot - Vec3::Z * 6.6, n, 3.5, 3.5, 3.3);
    }
    // The house: a faceted armoured box, low at the back, stepped at the front.
    let lift = if hang { -0.4 } else { 0.4 };
    let section = |x: f32, w: f32, h: f32| {
        let c = pivot + v3(x, 0.0, lift);
        [(-w, -0.55), (-0.75 * w, -1.0), (0.75 * w, -1.0), (w, -0.55), (w, 0.45), (0.6 * w, 1.0), (-0.6 * w, 1.0), (-w, 0.45)]
            .iter()
            .map(|&(y, k)| c + v3(0.0, y, s * k * h))
            .collect::<Vec<_>>()
    };
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.loft(
        &[section(-8.5, 3.8, 2.5), section(-6.8, 5.4, 3.7), section(0.8, 5.5, 3.9), section(3.0, 4.1, 3.0)],
        true,
        true,
    );
    if b.mid() {
        // Optics on the left cheek and the owner's band across the roof.
        b.paint(ACCENT);
        b.cuboid(pivot + v3(-0.5, 6.0, lift + s * 1.8), v3(5.0, 1.6, 2.2));
        b.paint(GLOW);
        b.cuboid(pivot + v3(2.05, 6.0, lift + s * 1.8), v3(0.14, 0.8, 0.8));
        b.paint(TEAM);
        b.beam(pivot + v3(-4.5, -3.3, lift + s * 3.95), pivot + v3(-4.5, 3.3, lift + s * 3.95), v2(1.6, 0.2), v2(1.6, 0.2));
    }
    // What pitches with the gun (`with_recoil`, with no kick) and, inside it, the barrel
    // cluster that turns about the bore.
    b.with_recoil(|b| {
        let m = b.sides(10);
        b.paint(ACCENT);
        b.cylinder_between(pivot + Vec3::X * 2.4, pivot + Vec3::X * 4.6, 2.7, 2.5, m);
        b.with_spin(pivot, |b| {
            b.paint(METAL);
            b.cylinder_between(pivot + Vec3::X * 4.6, pivot + Vec3::X * 6.0, 2.2, 2.2, m);
            let barrels = if b.fine() { 6 } else { 3 };
            for k in 0..barrels {
                let a = (k as f32 + 0.5) * TAU / barrels as f32;
                let off = v3(0.0, a.cos(), a.sin()) * 1.4;
                b.cylinder_between(pivot + off + Vec3::X * 6.0, pivot + off + Vec3::X * 12.0, 0.44, 0.42, if b.fine() { 6 } else { 4 });
            }
            if b.fine() {
                b.cylinder_between(pivot + Vec3::X * 6.0, pivot + Vec3::X * 11.4, 0.6, 0.6, 6);
                b.paint(ACCENT);
                collar(b, pivot, 8.4, 9.2, [2.2, 2.2], [1.8, 1.8], 8);
                collar(b, pivot, 11.1, 12.1, [2.25, 2.25], [1.85, 1.85], 8);
            }
        });
    });
}
