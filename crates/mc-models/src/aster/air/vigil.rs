//! Vigil: the tech 2 sensor ship, a small single-drive spacecraft. A faceted armoured
//! hull, dark below and plated pale above, runs from a blunt prow back to the drive;
//! sponsons either side carry the lift jets; a spine of radiators runs down its back.
//! Its sensors take after the Watchtower: two booms reach out ahead of the prow from
//! sockets in its cheeks, each a needle ringed with lit scan-fin wreaths. It stands on
//! two pairs of short legs that fold into flush belly bays in flight (`capital::gear`).
//! +X is forward, +Y left, the ground at z 0 with the legs down.
//!
//! Contracts: the drive mouth is `NOZZLES`, the lift jets under the sponsons `LIFT_JETS`,
//! the lamp fittings modelled on the hull `LAMPS`, and `RIG` what `entity.wgsl` animates.
use std::f32::consts::FRAC_PI_2;

use glam::Affine3A;

use super::capital::{self, CapitalRig, Leg};
use super::*;
use crate::aster::structures::scan_wreath;
use crate::builder::{ngon, Section};

/// Size of the single stern drive against the Bastion's 12 m bells.
const DRIVE_SCALE: f32 = 0.4;
/// The flat belly the legs stand the ship over, and the leg's size against the Bastion's.
const KEEL: f32 = 6.5;
const LEG: f32 = KEEL / 36.0;
const LEG_Y: f32 = 5.0;
const BAY: f32 = 1.3;
/// The sponsons' axis: |y| and height.
const SPONSON_Y: f32 = 10.8;
const SPONSON_Z: f32 = 9.6;

/// The drive's mouth (model space): the exhaust trail and drive effects start here.
pub(crate) const NOZZLES: [[f32; 3]; 1] = [[-36.0, 0.0, 11.0]];
const NOZZLE: [f32; 3] = NOZZLES[0];
/// Downward lift jets under the sponsons, mouth centres, `[aft -y, aft +y, fore -y, fore +y]`.
pub(crate) const LIFT_JETS: [[f32; 3]; 4] = [
    [-11.0, -SPONSON_Y, 6.8],
    [-11.0, SPONSON_Y, 6.8],
    [9.0, -SPONSON_Y, 6.8],
    [9.0, SPONSON_Y, 6.8],
];

/// Lamp fittings on the hull (model space): floods under the sponsons, nav lights on
/// their noses, strobes on the prow and the spine's aft riser, a beacon mid-spine.
pub(crate) const LAMPS: crate::CapitalLamps = crate::CapitalLamps {
    floods: &[
        [14.0, SPONSON_Y, 7.0],
        [14.0, -SPONSON_Y, 7.0],
        [-17.0, SPONSON_Y, 7.0],
        [-17.0, -SPONSON_Y, 7.0],
    ],
    nav_port: [18.2, SPONSON_Y, SPONSON_Z],
    nav_starboard: [18.2, -SPONSON_Y, SPONSON_Z],
    strobes: &[[31.2, 0.0, 13.6], [-18.6, 0.0, 18.6]],
    beacons: &[[-2.0, 0.0, 19.4]],
    hold: None,
};

/// What `entity.wgsl` animates (`models::capital_rig`): the legs (the fore pair stows
/// aft, the aft pair forward), the drive on the centre line and the lift jets.
pub(crate) const RIG: CapitalRig = CapitalRig {
    legs: Some([
        Leg {
            hinge: [20.0, LEG_Y, KEEL],
            stow: 1.0,
            bay: [12.6, 21.0, LEG_Y - BAY, LEG_Y + BAY],
            size: LEG,
        },
        Leg {
            hinge: [-18.0, LEG_Y, KEEL],
            stow: -1.0,
            bay: [-19.0, -10.6, LEG_Y - BAY, LEG_Y + BAY],
            size: LEG,
        },
    ]),
    door_hinge: KEEL - 0.3,
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

/// The hull's stations: x, half width, half height, centre height.
const HULL: [[f32; 4]; 8] = [
    [31.0, 3.0, 2.6, 11.4],
    [28.0, 6.4, 4.2, 11.6],
    [20.0, 7.6, 5.0, 11.8],
    [8.0, 8.0, 5.3, 11.9],
    [-6.0, 7.8, 5.3, 11.9],
    [-14.0, 7.0, 5.0, 11.6],
    [-19.0, 5.8, 4.6, 11.3],
    [-22.0, 4.8, 4.0, 11.0],
];

/// A faceted section at x: flat belly, broad chines, a sloped deck and a narrow top,
/// `grow` proud of the hull's own.
fn section(x: f32, w: f32, h: f32, zc: f32, grow: f32) -> Vec<Vec3> {
    let (w, h) = (w + grow, h + grow);
    [
        (0.5, -1.0),
        (0.94, -0.55),
        (1.0, 0.05),
        (0.82, 0.62),
        (0.4, 1.0),
        (-0.4, 1.0),
        (-0.82, 0.62),
        (-1.0, 0.05),
        (-0.94, -0.55),
        (-0.5, -1.0),
    ]
    .iter()
    .map(|&(y, z)| v3(x, y * w, zc + z * h))
    .collect()
}

/// The upper half of a section (chine to chine over the top), for the carapace plates.
fn upper(x: f32, w: f32, h: f32, zc: f32, grow: f32) -> Vec<Vec3> {
    section(x, w, h, zc, grow)[2..8].to_vec()
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

/// The dark hull, then the pale carapace over it in three overlapping plates, each a
/// little proud of the one behind.
fn hull(b: &mut MeshBuilder) {
    b.paint(PLATING_DARK).pattern(pattern::WARSHIP);
    b.loft(
        &HULL
            .iter()
            .map(|&[x, w, h, z]| section(x, w, h, z, 0.0))
            .collect::<Vec<_>>(),
        true,
        true,
    );
    for (from, to, grow) in [(1, 2, 0.45), (2, 4, 0.3), (4, 6, 0.15)] {
        b.paint(PLATING).pattern(pattern::WARSHIP);
        let rings: Vec<Vec<Vec3>> = (from..=to)
            .map(|i| {
                let [x, w, h, z] = HULL[i];
                upper(x, w, h, z, grow)
            })
            .collect();
        b.loft(&rings, true, true);
    }
    // The owner's colour across the brow.
    b.paint(TEAM);
    b.beam(
        v3(25.0, -2.6, 16.25),
        v3(25.0, 2.6, 16.25),
        v2(1.4, 0.2),
        v2(1.4, 0.2),
    );
    if b.fine() {
        // Access panels and vents down the dark flanks under the carapace's edge.
        b.mirror_y(|b| {
            b.paint(ACCENT);
            for x in [14.0, 10.0, -2.0, 2.0] {
                b.cuboid(v3(x, 8.0, 11.0), v3(2.6, 0.2, 1.6));
            }
            b.paint(METAL);
            for i in 0..5 {
                b.cuboid(v3(-9.0 - i as f32 * 1.2, 7.4, 10.4), v3(0.5, 0.2, 2.2));
            }
        });
    }
}

/// The prow's face: a dark bezel with a visor slit, and the cheek sockets the booms
/// come out of.
fn prow(b: &mut MeshBuilder) {
    let [x, w, h, z] = HULL[0];
    b.paint(ACCENT);
    b.loft(
        &[
            section(x, w, h, z, 0.0),
            section(x + 0.6, w * 0.8, h * 0.8, z, 0.0),
        ],
        false,
        true,
    );
    b.paint(VISOR);
    b.cuboid(v3(x + 0.62, 0.0, z + 0.7), v3(0.2, w * 1.1, 0.5));
    b.mirror_y(|b| {
        b.paint(PLATING_DARK).pattern(pattern::WARSHIP);
        b.loft(
            &[
                octagon(20.0, 6.0, 10.2, 1.6, 0.6),
                octagon(25.0, 5.8, 10.0, 2.6, 0.9),
                octagon(29.4, 5.2, 9.8, 2.3, 0.8),
            ],
            true,
            true,
        );
        b.paint(METAL);
        b.loft(
            &[
                octagon(29.4, 5.2, 9.8, 1.9, 0.65),
                octagon(30.0, 5.2, 9.8, 1.9, 0.65),
            ],
            false,
            true,
        );
        boom(b, v3(29.8, 5.2, 9.8), 0.1, 0.62, 2);
    });
}

/// A sensor boom: the Watchtower's needle and scan-fin wreaths (`structures.rs`) laid
/// forward, from `root` out along +x (turned `yaw` off the centre line), `k` times the
/// tower's size, `tiers` wreaths along it and a lit sensor tip.
fn boom(b: &mut MeshBuilder, root: Vec3, yaw: f32, k: f32, tiers: usize) {
    let frame = Affine3A::from_translation(root)
        * Affine3A::from_rotation_z(yaw)
        * Affine3A::from_rotation_y(FRAC_PI_2)
        * Affine3A::from_scale(Vec3::splat(k));
    b.with(frame, |b| {
        let tip = 4.0 + 6.6 * tiers as f32;
        b.paint(PLATING).pattern(pattern::WARSHIP);
        b.loft_z(
            &ngon(b.sides(8), 1.0),
            &[
                Section::new(0.0, 2.3),
                Section::new(2.6, 1.7),
                Section::new(tip * 0.5, 1.0),
                Section::new(tip, 0.45),
                Section::new(tip + 1.6, 0.12),
            ],
        );
        let wreaths = [(3.2, 9.6, 1.95, 1.3, 0.3), (10.2, 16.2, 1.75, 1.2, 1.35)];
        for &(z0, z1, r0, r1, turn) in wreaths.iter().take(tiers) {
            scan_wreath(b, z0, z1, r0, r1, turn, true);
        }
        if b.fine() {
            b.paint(GLOW);
            b.prism(v3(0.0, 0.0, 1.4), 8, 1.95, 1.95, 0.16);
        }
        b.paint(VISOR);
        b.spheroid(v3(0.0, 0.0, tip + 1.7), v3(0.35, 0.35, 0.5), 6, 2);
    });
}

/// The sponsons: a pod either side on a pylon from the hull's chine, lift jets under it,
/// the nav light on its nose, the owner's stripe and a sensor strip down its flank.
fn sponsons(b: &mut MeshBuilder) {
    b.mirror_y(|b| {
        let (y, z) = (SPONSON_Y, SPONSON_Z);
        b.paint(PLATING).pattern(pattern::WARSHIP);
        b.loft(
            &[
                octagon(18.4, y, z, 0.9, 0.35),
                octagon(15.0, y, z, 2.3, 0.8),
                octagon(4.0, y, z, 2.6, 0.9),
                octagon(-12.0, y, z, 2.6, 0.9),
                octagon(-18.0, y, z, 2.0, 0.7),
                octagon(-20.0, y, z, 1.2, 0.45),
            ],
            true,
            true,
        );
        b.paint(PLATING_DARK).pattern(pattern::WARSHIP);
        b.frustum(
            v3(-1.0, y - 2.7, z - 1.0),
            v2(24.0, 3.0),
            v2(19.0, 2.4),
            1.9,
            v2(-1.0, -0.2),
        );
        b.paint(TEAM);
        b.beam(
            v3(6.0, y + 2.4, z + 1.1),
            v3(-8.0, y + 2.4, z + 1.1),
            v2(0.25, 0.5),
            v2(0.25, 0.5),
        );
        if b.fine() {
            b.paint(VISOR);
            b.beam(
                v3(12.0, y + 2.45, z - 0.4),
                v3(2.0, y + 2.6, z - 0.4),
                v2(0.2, 0.4),
                v2(0.2, 0.4),
            );
            b.paint(ACCENT);
            for x in [-2.0, -6.0, -10.0] {
                b.cuboid(v3(x, y + 2.55, z - 0.5), v3(1.6, 0.2, 1.4));
            }
        }
    });
    for jet in LIFT_JETS {
        capital::lift_jet(b, Vec3::from(jet), 0.4);
    }
}

/// The spine: a raised ridge down the back with radiator banks either side, an aft
/// riser up to the strobe, aerials and the beacon's plinth.
fn spine(b: &mut MeshBuilder) {
    let top = HULL[3][3] + HULL[3][2];
    b.paint(PLATING_DARK).pattern(pattern::WARSHIP);
    b.frustum(
        v3(-3.0, 0.0, top - 0.3),
        v2(30.0, 3.6),
        v2(27.0, 2.4),
        1.6,
        v2(-0.6, 0.0),
    );
    b.frustum(
        v3(-15.5, 0.0, top - 0.8),
        v2(6.0, 2.8),
        v2(3.0, 1.2),
        18.3 - top + 0.8,
        v2(-1.5, 0.0),
    );
    b.paint(ACCENT);
    b.prism(v3(-2.0, 0.0, top + 1.3), b.sides(8), 0.9, 0.7, 1.6);
    if b.fine() {
        for y in [-3.4, 3.4] {
            b.paint(METAL);
            for i in 0..8 {
                let x = 9.0 - i as f32 * 2.0;
                b.cuboid(v3(x, y, top + 0.35), v3(0.35, 2.2, 1.1));
            }
            b.paint(GLOW_ORANGE);
            b.cuboid(v3(2.0, y, top - 0.05), v3(15.0, 1.6, 0.12));
        }
        b.paint(METAL);
        for (x, y, h) in [(13.0, 2.0, 4.0), (13.0, -2.0, 3.2), (-9.0, 1.2, 3.6)] {
            b.cylinder_between(v3(x, y, top), v3(x - 1.2, y, top + h), 0.16, 0.07, 5);
        }
    }
}

/// The keel plate under the belly, where the leg bays sit flush, and the legs.
fn keel(b: &mut MeshBuilder) {
    b.paint(PLATING_DARK).pattern(pattern::WARSHIP);
    b.frustum(
        v3(1.0, 0.0, KEEL - 0.2),
        v2(41.0, 11.0),
        v2(44.0, 12.8),
        1.2,
        v2(0.0, 0.0),
    );
    capital::gear(b, &RIG, KEEL - 0.7);
}

/// The stern: a dark collar from the hull's tail back onto the drive, and the drive.
fn stern(b: &mut MeshBuilder) {
    let [nx, _, z] = NOZZLE;
    let [tail, _, r, _] = HULL[7];
    let aft = nx + 27.6 * DRIVE_SCALE + 0.4;
    b.paint(PLATING_DARK).pattern(pattern::WARSHIP);
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
    capital::drive(b, Vec3::from(NOZZLE), DRIVE_SCALE);
}

/// Fittings for the lamps the renderer lights: flood housings, nav pods, strobes, beacons.
fn lamp_fittings(b: &mut MeshBuilder) {
    for at in LAMPS.floods {
        let c = Vec3::from(*at);
        b.paint(ACCENT);
        b.cuboid(c + v3(0.0, 0.0, 0.5), v3(1.6, 1.3, 0.8));
        b.paint(GLOW);
        b.cuboid(c + v3(0.0, 0.0, 0.1), v3(1.0, 0.8, 0.1));
    }
    for (at, glow) in [(LAMPS.nav_port, GLOW_RED), (LAMPS.nav_starboard, GLOW)] {
        b.paint(glow);
        b.cuboid(Vec3::from(at), v3(0.8, 0.6, 0.6));
    }
    for at in LAMPS.strobes {
        b.paint(GLOW);
        b.cuboid(Vec3::from(*at), v3(0.5, 0.5, 0.5));
    }
    for at in LAMPS.beacons {
        b.paint(GLOW_AMBER);
        b.prism(
            Vec3::from(*at) - v3(0.0, 0.0, 0.2),
            b.sides(8),
            0.45,
            0.4,
            0.5,
        );
    }
}

pub(super) fn build(b: &mut MeshBuilder) {
    if b.coarse() {
        b.paint(PLATING);
        b.cuboid(v3(4.0, 0.0, 11.8), v3(52.0, 15.0, 10.0));
        b.paint(PLATING_DARK);
        for y in [-SPONSON_Y, SPONSON_Y] {
            b.cuboid(v3(-1.0, y, SPONSON_Z), v3(36.0, 5.0, 5.0));
        }
        b.cuboid(v3(-28.0, 0.0, 11.0), v3(14.0, 8.0, 8.0));
        return;
    }
    hull(b);
    prow(b);
    sponsons(b);
    spine(b);
    keel(b);
    stern(b);
    lamp_fittings(b);
}

#[cfg(test)]
mod tests {
    use crate::{build_model_fitted, part};

    #[test]
    fn the_vigil_stands_on_legs_that_stow_and_its_booms_reach_past_the_prow() {
        let model = build_model_fitted("sensor_ship", 36.0, 22.0, 1, &[]).unwrap();
        let lod = &model.lods[0];
        let low = lod
            .vertices
            .iter()
            .map(|v| v.pos[2])
            .fold(f32::MAX, f32::min);
        assert!((-0.7..0.3).contains(&low), "lowest point {low}");
        // Only the legs reach the ground: the belly stands clear on them.
        for v in &lod.vertices {
            if ![part::GEAR, part::GEAR_STRUT, part::GEAR_FOOT].contains(&v.part) {
                assert!(v.pos[2] > 4.0, "part {} at {:?}", v.part, v.pos);
            }
        }
        // The cheek booms run out well ahead of the prow.
        let nose = lod
            .vertices
            .iter()
            .map(|v| v.pos[0])
            .fold(f32::MIN, f32::max);
        assert!(nose > super::HULL[0][0] + 8.0, "{nose}");
        // The drive's nozzle swivels about the rig's axis: it sits on the centre line.
        assert!(lod
            .vertices
            .iter()
            .filter(|v| v.part == part::DRIVE)
            .all(|v| v.pos[1].abs() < 6.0 && v.pos[0] < -20.0));
        assert_eq!(crate::capital_rig("sensor_ship"), Some(super::RIG.gpu()));
    }
}
