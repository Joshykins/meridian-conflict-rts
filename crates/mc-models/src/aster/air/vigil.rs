//! Vigil: the tech 1 sensor ship, a small single-drive spacecraft with a hammerhead of
//! sensors across its bow. +X is forward, +Y left, the ground at z 0 when landed (it
//! sets down on skids).
//!
//! Contracts: each hull's `Fit` holds its drive mouth, belly lift jets, the lamp fittings
//! modelled on it and its `CapitalRig` (`models::capital_rig` and friends read them by
//! mesh). The sensor head turns as `part::SPINNER` about its pivot and stops when the
//! ship's power fails (`entity.wgsl`). The `~` keys are design variants of the hull.
use glam::Vec2;

use super::capital::{self, CapitalRig};
use super::*;
use crate::CapitalLamps;

mod crescent;
mod eyes;
mod ray;

/// One hull's anchors, shared with the renderer's effects.
pub(crate) struct Fit {
    pub nozzles: [[f32; 3]; 1],
    pub lift_jets: [[f32; 3]; 4],
    pub lamps: CapitalLamps,
    pub rig: CapitalRig,
}

/// Size of the single stern drive against the Bastion's 12 m bells.
const DRIVE_SCALE: f32 = 0.4;

/// The rig `entity.wgsl` animates for a hull with one drive on its centre line and four
/// lift jets (`[aft -y, aft +y, fore -y, fore +y]`): no legs (skids), no ramp or doors.
const fn rig(nozzle: [f32; 3], jets: [[f32; 3]; 4]) -> CapitalRig {
    CapitalRig {
        legs: None,
        door_hinge: 0.0,
        drives: Some(([nozzle[0], nozzle[2], 0.0, 0.0], DRIVE_SCALE)),
        lift_jets: Some(([jets[3][0], jets[3][1], jets[1][0], jets[1][1]], jets[3][2])),
        ramp: None,
    }
}

/// The hull drawn for `mesh`, if it is a Vigil.
pub(crate) fn fit(mesh: &str) -> Option<&'static Fit> {
    match mesh {
        "sensor_ship" => Some(&ray::FIT),
        "sensor_ship~eyes" => Some(&eyes::FIT),
        "sensor_ship~crescent" => Some(&crescent::FIT),
        _ => None,
    }
}

pub(crate) fn build_ray(b: &mut MeshBuilder) {
    ray::build(b);
}

pub(crate) fn build_eyes(b: &mut MeshBuilder) {
    eyes::build(b);
}

pub(crate) fn build_crescent(b: &mut MeshBuilder) {
    crescent::build(b);
}

/// A lens section across x: half span `w`, thickness `t`, centred at height `zc`. The
/// upper half (`up`) or lower half, closed across the widest line, so a hull lofted from
/// both is pale above and dark below.
fn lens(x: f32, w: f32, t: f32, zc: f32, up: bool) -> Vec<Vec3> {
    let s = if up { 1.0 } else { -0.85 };
    vec![
        v3(x, w, zc),
        v3(x, 0.78 * w, zc + 0.42 * t * s),
        v3(x, 0.32 * w, zc + 0.6 * t * s),
        v3(x, -0.32 * w, zc + 0.6 * t * s),
        v3(x, -0.78 * w, zc + 0.42 * t * s),
        v3(x, -w, zc),
    ]
}

/// A two-tone lifting body lofted through `stations` (x, half span, thickness, centre
/// height): pale plating above, dark below.
fn body(b: &mut MeshBuilder, stations: &[[f32; 4]]) {
    for (up, paint) in [(true, PLATING), (false, PLATING_DARK)] {
        b.paint(paint).pattern(pattern::AIRFRAME);
        b.loft(
            &stations
                .iter()
                .map(|s| lens(s[0], s[1], s[2], s[3], up))
                .collect::<Vec<_>>(),
            true,
            true,
        );
    }
}

/// An octagonal ring about the x axis through (y, z): half size `r`, corners cut by `cut`.
fn octagon(x: f32, y: f32, z: f32, r: f32, cut: f32) -> Vec<Vec3> {
    let k = r - cut;
    [
        (-k, -r),
        (k, -r),
        (r, -k),
        (r, k),
        (k, r),
        (-k, r),
        (-r, k),
        (-r, -k),
    ]
    .iter()
    .map(|&(dy, dz)| v3(x, y + dy, z + dz))
    .collect()
}

/// The stern: a dark collar from the hull's tail (`tail` x, half size `r`) back onto the
/// drive, and the drive itself.
fn stern(b: &mut MeshBuilder, nozzle: [f32; 3], tail: f32, r: f32) {
    let [nx, _, z] = nozzle;
    let aft = nx + 27.6 * DRIVE_SCALE + 0.4;
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.loft(
        &[
            octagon(tail + 1.0, 0.0, z, r, r * 0.35),
            octagon(aft + 1.0, 0.0, z, r * 1.02, r * 0.35),
            octagon(aft, 0.0, z, r * 0.95, r * 0.33),
        ],
        true,
        true,
    );
    b.paint(ACCENT);
    b.loft(
        &[
            octagon(aft + 0.2, 0.0, z, r * 0.98, r * 0.34),
            octagon(aft - 1.2, 0.0, z, r * 0.9, r * 0.32),
        ],
        false,
        true,
    );
    if b.fine() {
        b.paint(TEAM);
        b.loft(
            &[
                octagon(aft + 4.0, 0.0, z, r * 1.04, r * 0.36),
                octagon(aft + 2.6, 0.0, z, r * 1.04, r * 0.36),
            ],
            true,
            true,
        );
    }
    capital::drive(b, Vec3::from(nozzle), DRIVE_SCALE);
}

/// A skid on the ground at `y` from `x0` to `x1`, on struts up to the belly at height
/// `keel` and half width `keel_y`. Mirror it.
fn skid(b: &mut MeshBuilder, x0: f32, x1: f32, y: f32, keel: f32, keel_y: f32) {
    b.paint(ACCENT).pattern(pattern::AIRFRAME);
    b.frustum(
        v3((x0 + x1) * 0.5, y, 0.0),
        v2(x1 - x0, 2.2),
        v2(x1 - x0 - 3.0, 1.6),
        1.1,
        v2(0.0, 0.0),
    );
    b.paint(METAL);
    for x in [x0 + 2.5, x1 - 2.5] {
        b.beam(
            v3(x, y, 1.0),
            v3(x + 1.0, keel_y, keel + 0.5),
            v2(1.1, 0.9),
            v2(1.0, 0.8),
        );
    }
}

/// The belly lift jets, `LIFT_JETS` of a hull.
fn lift_jets(b: &mut MeshBuilder, jets: &[[f32; 3]; 4]) {
    for jet in jets {
        capital::lift_jet(b, Vec3::from(*jet), 0.4);
    }
}

/// Fittings for the lamps the renderer lights: flood housings, nav pods, strobes, beacons.
fn lamp_fittings(b: &mut MeshBuilder, lamps: &CapitalLamps) {
    for at in lamps.floods {
        let c = Vec3::from(*at);
        b.paint(ACCENT);
        b.cuboid(c + v3(0.0, 0.0, 0.5), v3(1.6, 1.3, 0.8));
        b.paint(GLOW);
        b.cuboid(c + v3(0.0, 0.0, 0.1), v3(1.0, 0.8, 0.1));
    }
    for (at, glow) in [(lamps.nav_port, GLOW_RED), (lamps.nav_starboard, GLOW)] {
        b.paint(glow);
        b.cuboid(Vec3::from(at), v3(0.8, 0.6, 0.6));
    }
    for at in lamps.strobes {
        b.paint(GLOW);
        b.cuboid(Vec3::from(*at), v3(0.5, 0.5, 0.5));
    }
    for at in lamps.beacons {
        let c = Vec3::from(*at);
        b.paint(ACCENT);
        b.prism(c - v3(0.0, 0.0, 0.9), b.sides(8), 0.7, 0.6, 0.7);
        b.paint(GLOW_AMBER);
        b.prism(c - v3(0.0, 0.0, 0.2), b.sides(8), 0.45, 0.4, 0.5);
    }
}

/// A sensor aperture: a band of the ARC's orange visor glass from `from` to `to`, `size`
/// across (width, height), standing a little out of a dark bezel toward `out`.
fn aperture(b: &mut MeshBuilder, from: Vec3, to: Vec3, size: Vec2, out: Vec3) {
    b.paint(ACCENT);
    b.beam(from, to, size + v2(0.6, 0.6), size + v2(0.6, 0.6));
    b.paint(VISOR);
    let out = out.normalize_or_zero() * 0.25;
    b.beam(from + out, to + out, size, size);
}

/// A scanning sensor head on the crown at `at` (its foot): a low faceted drum with an
/// aperture looking forward, swinging to look about (`set_spinner_scan`).
fn scanner(b: &mut MeshBuilder, at: Vec3, r: f32) {
    b.paint(METAL);
    b.prism(at - v3(0.0, 0.0, 0.8), b.sides(10), r * 1.1, r, 0.9);
    b.set_spinner_pivot(at);
    b.set_spinner_scan();
    b.with_part(part::SPINNER, |b| {
        b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
        b.prism(at, b.sides(10), r, r * 0.85, r * 0.55);
        b.paint(PLATING).pattern(pattern::PLAIN);
        b.prism(
            at + v3(0.0, 0.0, r * 0.55),
            b.sides(10),
            r * 0.85,
            r * 0.55,
            r * 0.22,
        );
        b.paint(VISOR);
        b.cuboid(at + v3(r * 0.86, 0.0, r * 0.3), v3(0.3, r * 1.1, r * 0.22));
    });
}

#[cfg(test)]
mod tests {
    use crate::{build_model_fitted, part};

    #[test]
    fn every_vigil_hull_turns_its_sensor_and_sits_on_its_skids() {
        for key in ["sensor_ship", "sensor_ship~eyes", "sensor_ship~crescent"] {
            let model = build_model_fitted(key, 36.0, 22.0, 1, &[]).unwrap();
            let tris = model.lods.each_ref().map(|lod| lod.indices.len() / 3);
            // Budgets and the reduced share are `tests::lods_reduce_and_respect_budgets`.
            println!("{key} triangles: {tris:?}");
            let lod = &model.lods[0];
            assert!(
                lod.vertices.iter().any(|v| v.part == part::SPINNER),
                "{key}"
            );
            for v in &lod.vertices {
                assert!(
                    [part::HULL, part::SPINNER, part::DRIVE].contains(&v.part),
                    "{key}: part {}",
                    v.part
                );
                // Nothing hangs under the ground it lands on.
                assert!(v.pos[2] >= -0.61, "{key}: {:?}", v.pos);
            }
            assert!(super::fit(key).is_some(), "{key} has its anchors");
        }
    }
}
