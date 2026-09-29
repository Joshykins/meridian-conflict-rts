//! A: the ray. Head and body are one lifting body, pale above and dark below: the
//! hammerhead's wings blend back into a slim fuselage that ends in the single drive. An
//! eye-line of sensor apertures runs across the head's brow from tip to tip, a scanning
//! head sits on its crown, and winglets stand at the tips.
use super::*;

const NOZZLE: [f32; 3] = [-36.0, 0.0, 9.5];
const LIFT_JETS: [[f32; 3]; 4] = [
    [-12.0, -2.2, 5.0],
    [-12.0, 2.2, 5.0],
    [8.0, -2.2, 5.0],
    [8.0, 2.2, 5.0],
];

pub(super) static FIT: Fit = Fit {
    nozzles: [NOZZLE],
    lift_jets: LIFT_JETS,
    lamps: CapitalLamps {
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
    },
    rig: rig(NOZZLE, LIFT_JETS),
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
    lamp_fittings(b, &FIT.lamps);
}
