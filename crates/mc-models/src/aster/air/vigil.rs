//! Vigil: the tech 1 sensor ship, a spacecraft built round one great search radar.
//! +X is forward, +Y left, the ground at z 0 when landed (it sets down on skids).
//!
//! Contracts: each hull's `Fit` holds its drive mouths, belly lift jets, the lamp
//! fittings modelled on it and its `CapitalRig` (`models::capital_rig` and friends read
//! them by mesh). The radar turns as `part::SPINNER` about its pivot and stops when the
//! ship's power fails (`entity.wgsl`). The `~` keys are design variants of the hull.
use super::capital::{self, CapitalRig};
use super::*;
use crate::CapitalLamps;
use glam::Vec2;

mod boom;
mod hammer;
mod truss;

/// One hull's anchors, shared with the renderer's effects.
pub(crate) struct Fit {
    pub nozzles: [[f32; 3]; 2],
    pub lift_jets: [[f32; 3]; 4],
    pub lamps: CapitalLamps,
    pub rig: CapitalRig,
}

/// Size of the stern drives against the Bastion's 12 m bells.
const DRIVE_SCALE: f32 = 0.45;

/// The rig `entity.wgsl` animates for a hull with two stern drives and four lift jets
/// (`[aft -y, aft +y, fore -y, fore +y]`): no legs (skids), no ramp or doors.
const fn rig(nozzles: [[f32; 3]; 2], jets: [[f32; 3]; 4]) -> CapitalRig {
    CapitalRig {
        legs: None,
        door_hinge: 0.0,
        drives: Some((
            [nozzles[1][0], nozzles[1][2], nozzles[1][1], nozzles[1][1]],
            DRIVE_SCALE,
        )),
        lift_jets: Some(([jets[3][0], jets[3][1], jets[1][0], jets[1][1]], jets[3][2])),
        ramp: None,
    }
}

/// The hull drawn for `mesh`, if it is a Vigil.
pub(crate) fn fit(mesh: &str) -> Option<&'static Fit> {
    match mesh {
        "sensor_ship" => Some(&hammer::FIT),
        "sensor_ship~boom" => Some(&boom::FIT),
        "sensor_ship~truss" => Some(&truss::FIT),
        _ => None,
    }
}

pub(crate) fn build_hammer(b: &mut MeshBuilder) {
    hammer::build(b);
}

pub(crate) fn build_boom(b: &mut MeshBuilder) {
    boom::build(b);
}

pub(crate) fn build_truss(b: &mut MeshBuilder) {
    truss::build(b);
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

/// A drive nacelle on the +y side, from `fore` back to the drive mouth `nozzle`, a pale
/// saddle over it, the drive, and the lift jet `jet` in a fairing under it. Mirror it.
fn nacelle(b: &mut MeshBuilder, nozzle: [f32; 3], fore: f32, jet: [f32; 3]) {
    let [nx, y, z] = nozzle;
    let aft = nx + 27.6 * DRIVE_SCALE + 0.4;
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.loft(
        &[
            octagon(fore, y - 1.0, z, 3.0, 1.2),
            octagon(fore - 12.0, y, z, 6.0, 2.2),
            octagon(aft + 1.5, y, z, 6.2, 2.2),
            octagon(aft, y, z, 5.7, 2.0),
        ],
        true,
        true,
    );
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    const SADDLE: [[f32; 2]; 6] = [
        [-4.0, 6.6],
        [4.0, 6.6],
        [6.6, 4.0],
        [6.2, 3.8],
        [3.8, 6.2],
        [-3.8, 6.2],
    ];
    let saddle = |x: f32| {
        SADDLE
            .iter()
            .map(|&[dy, dz]| v3(x, y + dy, z + dz))
            .collect::<Vec<_>>()
    };
    b.loft(&[saddle(fore - 11.0), saddle(aft + 2.0)], true, true);
    if b.fine() {
        b.paint(TEAM);
        b.beam(
            v3(fore - 15.0, y - 3.4, z + 6.7),
            v3(fore - 15.0, y + 3.4, z + 6.7),
            v2(1.2, 0.2),
            v2(1.2, 0.2),
        );
        b.paint(GLOW_RED);
        b.cuboid(v3(aft - 0.2, y + 5.0, z + 3.4), v3(0.6, 0.8, 0.8));
    }
    b.paint(ACCENT);
    b.loft(
        &[
            octagon(aft + 0.2, y, z, 5.9, 2.1),
            octagon(aft - 1.2, y, z, 5.4, 1.9),
        ],
        false,
        true,
    );
    capital::drive(b, Vec3::from(nozzle), DRIVE_SCALE);
    let jet = Vec3::from(jet);
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.frustum(
        v3(jet.x, jet.y, jet.z + 1.2),
        v2(9.0, 7.0),
        v2(12.0, 8.0),
        z - 5.0 - jet.z,
        v2(0.0, 0.0),
    );
    capital::lift_jet(b, jet, 0.6);
}

/// A skid on the ground at `y` from `x0` to `x1`, on struts up to the keel at height
/// `keel` and half width `keel_y`. Mirror it.
fn skid(b: &mut MeshBuilder, x0: f32, x1: f32, y: f32, keel: f32, keel_y: f32) {
    b.paint(ACCENT).pattern(pattern::AIRFRAME);
    b.frustum(
        v3((x0 + x1) * 0.5, y, 0.0),
        v2(x1 - x0, 3.0),
        v2(x1 - x0 - 4.0, 2.2),
        1.4,
        v2(0.0, 0.0),
    );
    b.paint(METAL);
    let n = ((x1 - x0) / 20.0).ceil().max(1.0) as usize + 1;
    for k in 0..n {
        let x = x0 + 3.0 + (x1 - x0 - 6.0) * k as f32 / (n - 1) as f32;
        b.beam(
            v3(x, y, 1.2),
            v3(x + 1.5, keel_y, keel + 0.6),
            v2(1.6, 1.2),
            v2(1.4, 1.0),
        );
    }
}

/// Fittings for the lamps the renderer lights: flood housings, nav pods, strobes, beacons.
fn lamp_fittings(b: &mut MeshBuilder, lamps: &CapitalLamps) {
    for at in lamps.floods {
        let c = Vec3::from(*at);
        b.paint(ACCENT);
        b.cuboid(c + v3(0.0, 0.0, 0.7), v3(2.2, 1.8, 1.0));
        b.paint(GLOW);
        b.cuboid(c + v3(0.0, 0.0, 0.15), v3(1.4, 1.0, 0.12));
    }
    for (at, glow) in [(lamps.nav_port, GLOW_RED), (lamps.nav_starboard, GLOW)] {
        let c = Vec3::from(at);
        let out = c.y.signum();
        b.paint(PLATING_DARK);
        b.beam(
            c - v3(0.0, out * 1.6, 0.0),
            c - v3(0.0, out * 0.3, 0.0),
            v2(2.2, 1.4),
            v2(1.8, 1.0),
        );
        b.paint(glow);
        b.cuboid(c, v3(1.0, 0.7, 0.7));
    }
    for at in lamps.strobes {
        b.paint(GLOW);
        b.cuboid(Vec3::from(*at), v3(0.6, 0.6, 0.6));
    }
    for at in lamps.beacons {
        let c = Vec3::from(*at);
        b.paint(ACCENT);
        b.prism(c - v3(0.0, 0.0, 1.2), b.sides(8), 0.9, 0.8, 0.9);
        b.paint(GLOW_AMBER);
        b.prism(c - v3(0.0, 0.0, 0.3), b.sides(8), 0.6, 0.5, 0.7);
    }
}

/// A radiator bank: dark fins over a hot tray, `size` (x, y) centred at `c` on a deck.
fn radiators(b: &mut MeshBuilder, c: Vec3, size: Vec2) {
    b.paint(ACCENT);
    b.plate(c, size, 0.5, 0.2);
    if !b.fine() {
        return;
    }
    let n = (size.x / 2.4).floor() as usize;
    for k in 0..n {
        let x = c.x - size.x * 0.5 + 1.2 + k as f32 * 2.4;
        b.paint(PLATING_DARK);
        b.beam(
            v3(x, c.y - size.y * 0.42, c.z + 1.8),
            v3(x, c.y + size.y * 0.42, c.z + 1.8),
            v2(0.7, 2.6),
            v2(0.7, 2.6),
        );
        if k + 1 < n {
            b.paint(GLOW_ORANGE);
            b.beam(
                v3(x + 1.2, c.y - size.y * 0.4, c.z + 0.7),
                v3(x + 1.2, c.y + size.y * 0.4, c.z + 0.7),
                v2(0.45, 0.15),
                v2(0.45, 0.15),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{build_model_fitted, part};

    #[test]
    fn every_vigil_hull_turns_its_radar_and_sits_on_its_skids() {
        for key in ["sensor_ship", "sensor_ship~boom", "sensor_ship~truss"] {
            let model = build_model_fitted(key, 56.0, 40.0, 1, &[]).unwrap();
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
