//! Vigil: the tech 1 sensor ship, a small single-drive spacecraft. Head and body are one
//! lifting body, pale above and dark below: a hammerhead of sensors across the bow blends
//! back into a slim fuselage that ends in the drive. An eye-line of sensor apertures runs
//! across the head's brow from tip to tip, a scanning head sits on its crown, and winglets
//! stand at the tips. +X is forward, +Y left, the ground at z 0 when landed (skids).
//!
//! Contracts: the drive mouth is `NOZZLES`, belly lift jets `LIFT_JETS`, the lamp
//! fittings modelled on the hull `LAMPS`, and `RIG` what `entity.wgsl` animates. The
//! scanning head turns as `part::SPINNER` about its pivot and stops when the ship's power
//! fails.
use glam::Vec2;

use super::capital::{self, CapitalRig};
use super::*;

/// Size of the single stern drive against the Bastion's 12 m bells.
const DRIVE_SCALE: f32 = 0.4;

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
fn lamp_fittings(b: &mut MeshBuilder, lamps: &crate::CapitalLamps) {
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

/// The drive's mouth (model space): the exhaust trail and drive effects start here.
pub(crate) const NOZZLES: [[f32; 3]; 1] = [[-36.0, 0.0, 9.5]];
const NOZZLE: [f32; 3] = NOZZLES[0];
/// Downward lift jets, mouth centres, `[aft -y, aft +y, fore -y, fore +y]`.
pub(crate) const LIFT_JETS: [[f32; 3]; 4] = [
    [-12.0, -2.2, 5.0],
    [-12.0, 2.2, 5.0],
    [8.0, -2.2, 5.0],
    [8.0, 2.2, 5.0],
];

/// Lamp fittings on the hull (model space): floods under the head and the belly, nav
/// lights at the head's tips, strobes at the nose and the fin, a beacon on the spine.
pub(crate) const LAMPS: crate::CapitalLamps = crate::CapitalLamps {
    floods: &[
        [26.0, 3.0, 6.6],
        [26.0, -3.0, 6.6],
        [-17.0, 4.0, 4.9],
        [-17.0, -4.0, 4.9],
    ],
    nav_port: [20.5, 26.4, 9.5],
    nav_starboard: [20.5, -26.4, 9.5],
    strobes: &[[35.8, 0.0, 9.0], [-18.8, 0.0, 21.0]],
    beacons: &[[-4.0, 0.0, 15.9]],
    hold: None,
};

/// What `entity.wgsl` animates (`models::capital_rig`): the drive on the centre line and
/// the lift jets. No legs (skids), no ramp or doors.
pub(crate) const RIG: CapitalRig = CapitalRig {
    legs: None,
    door_hinge: 0.0,
    drives: Some(([NOZZLE[0], NOZZLE[2], 0.0, 0.0], DRIVE_SCALE)),
    lift_jets: Some((
        [
            LIFT_JETS[3][0],
            LIFT_JETS[3][1],
            LIFT_JETS[1][0],
            LIFT_JETS[1][1],
        ],
        LIFT_JETS[3][2],
    )),
    ramp: None,
};

/// The lifting body: x, half span, thickness, centre height.
const STATIONS: [[f32; 4]; 9] = [
    [35.5, 1.5, 1.2, 9.0],
    [33.0, 9.0, 3.2, 9.0],
    [29.0, 20.0, 4.2, 9.2],
    [24.0, 26.0, 4.6, 9.4],
    [17.0, 26.0, 4.2, 9.6],
    [15.5, 10.0, 7.0, 9.8],
    [6.0, 8.0, 9.0, 10.0],
    [-10.0, 7.2, 9.0, 10.0],
    [-20.0, 6.4, 10.0, 9.5],
];

/// Where station `i`'s upper surface is `u` of the way out (0.78: the brow line).
fn brow(i: usize, side: f32) -> Vec3 {
    let [x, w, t, zc] = STATIONS[i];
    v3(x + 0.1, side * 0.78 * w, zc + 0.42 * t + 0.05)
}

pub(super) fn build(b: &mut MeshBuilder) {
    if b.coarse() {
        b.paint(PLATING);
        b.cuboid(v3(-6.0, 0.0, 10.0), v3(52.0, 14.0, 9.0));
        b.cuboid(v3(24.0, 0.0, 9.4), v3(14.0, 52.0, 4.0));
        b.paint(PLATING_DARK);
        b.cuboid(v3(-26.0, 0.0, 9.5), v3(12.0, 10.0, 10.0));
        return;
    }
    body(b, &STATIONS);
    stern(b, NOZZLE, -20.0, 5.2);
    // The eye-line: apertures across the brow, tip to tip.
    aperture(
        b,
        brow(1, 1.0),
        brow(1, -1.0),
        v2(0.5, 0.7),
        v3(1.0, 0.0, 0.4),
    );
    b.mirror_y(|b| {
        aperture(
            b,
            brow(1, 1.0),
            brow(2, 1.0),
            v2(0.5, 0.7),
            v3(11.0, 4.0, 4.0),
        );
        aperture(
            b,
            brow(2, 1.0),
            brow(3, 1.0),
            v2(0.5, 0.7),
            v3(6.0, 5.0, 4.0),
        );
        // Winglets at the tips, up and down, the owner's colour on the upper one.
        let [_, w, _, zc] = STATIONS[3];
        b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
        b.frustum(
            v3(20.5, w, zc - 0.2),
            v2(7.5, 0.7),
            v2(3.5, 0.5),
            4.6,
            v2(-2.5, 0.0),
        );
        b.frustum(
            v3(20.5, w, zc - 3.4),
            v2(3.2, 0.5),
            v2(6.5, 0.7),
            3.2,
            v2(1.2, 0.0),
        );
        b.paint(TEAM);
        b.frustum(
            v3(18.0, w, zc + 4.4),
            v2(3.5, 0.55),
            v2(3.3, 0.5),
            0.6,
            v2(-0.2, 0.0),
        );
        if b.fine() {
            // Chevrons in the owner's colour over the head, dark panel seams behind them.
            b.beam(
                v3(21.5, 5.0, 12.25),
                v3(23.5, 14.0, 11.95),
                v2(1.0, 0.1),
                v2(1.0, 0.1),
            );
            b.paint(ACCENT);
            b.beam(
                v3(18.5, 9.0, 12.0),
                v3(18.5, 22.0, 11.2),
                v2(0.4, 0.1),
                v2(0.4, 0.1),
            );
        }
        skid(b, -16.0, 12.0, 5.0, 5.4, 3.5);
    });
    // Canopy and the dorsal fin.
    b.paint(PLATING_DARK);
    b.spheroid(v3(9.0, 0.0, 14.8), v3(5.6, 2.9, 1.6), b.sides(12), 4);
    b.paint(GLASS);
    b.spheroid(v3(9.4, 0.0, 15.1), v3(4.8, 2.4, 1.5), b.sides(12), 4);
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.frustum(
        v3(-15.0, 0.0, 14.6),
        v2(10.0, 0.9),
        v2(4.5, 0.6),
        6.2,
        v2(-3.5, 0.0),
    );
    b.paint(TEAM);
    b.frustum(
        v3(-18.5, 0.0, 20.6),
        v2(4.6, 0.62),
        v2(4.2, 0.6),
        0.5,
        v2(-0.2, 0.0),
    );
    scanner(b, v3(24.5, 0.0, 12.1), 3.0);
    lift_jets(b, &LIFT_JETS);
    lamp_fittings(b, &LAMPS);
}

#[cfg(test)]
mod tests {
    use crate::{build_model_fitted, part};

    #[test]
    fn the_vigil_turns_its_sensor_and_sits_on_its_skids() {
        let model = build_model_fitted("sensor_ship", 36.0, 22.0, 1, &[]).unwrap();
        let lod = &model.lods[0];
        assert!(lod.vertices.iter().any(|v| v.part == part::SPINNER));
        for v in &lod.vertices {
            assert!(
                [part::HULL, part::SPINNER, part::DRIVE].contains(&v.part),
                "part {}",
                v.part
            );
            // Nothing hangs under the ground it lands on.
            assert!(v.pos[2] >= -0.61, "{:?}", v.pos);
        }
        // The drive's nozzle swivels about the rig's axis: it sits on the centre line.
        let nozzle = lod.vertices.iter().filter(|v| v.part == part::DRIVE);
        assert!(nozzle
            .clone()
            .all(|v| v.pos[1].abs() < 6.0 && v.pos[0] < -20.0));
        assert_eq!(crate::capital_rig("sensor_ship"), Some(super::RIG.gpu()));
    }
}
