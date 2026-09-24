//! Courier: compact Aster spacecraft with an enclosed, ground-level stern bay.
//! +X is forward, +Y left, the ground at z 0 when landed.
//!
//! Contracts with the sim (`air.ron` transport) and the entity shader: the 28 x 26 m
//! tunnel (|y| < 14, z 0..26) runs from the stern doors (x -42) to the forward bulkhead
//! (x 24) and nothing of the hull stands inside it; the two plug doors (`part::HOLD_DOOR`)
//! slide 14.2 m sideways into the shoulders with `deploy` (`entity.wgsl`, icon bit
//! 0x2000000). Drive mouths are `NOZZLES`, belly lift jets `LIFT_JETS`; the lamp
//! fittings modelled on the hull are the `LAMPS` table.
use super::capital::{self, CapitalRig};
use super::*;

/// The two stern drives' mouths (model space): the exhaust trails and drive effects start here.
pub const NOZZLES: [[f32; 3]; 2] = [[-59.0, -26.0, 18.0], [-59.0, 26.0, 18.0]];
/// Downward lift jets, mouth centres: under the drive nacelles and under the prow's cheeks.
pub const LIFT_JETS: [[f32; 3]; 4] = [
    [-35.0, -26.0, 6.6],
    [-35.0, 26.0, 6.6],
    [35.0, -9.5, 4.6],
    [35.0, 9.5, 4.6],
];
/// Lamp fittings on the hull (model space), for the renderer's capital-ship lamps:
/// landing floods (fore pair under the chin, aft pair under the nacelles), red/green
/// nav lights on the shoulders' widest points, white strobes at the nose and nacelle
/// tails, amber beacons at the door lintel, and the hold's ceiling lamp with the x where
/// its light spills out onto the ground behind the doors.
pub const LAMPS: crate::models::CapitalLamps = crate::models::CapitalLamps {
    floods: &[[47.0, 5.0, 5.2], [47.0, -5.0, 5.2], [-27.0, 26.0, 7.6], [-27.0, -26.0, 7.6]],
    nav_port: [-8.0, 38.4, 23.0],
    nav_starboard: [-8.0, -38.4, 23.0],
    strobes: &[[59.6, 0.0, 16.0], [-41.0, 26.0, 29.4], [-41.0, -26.0, 29.4]],
    beacons: &[[-45.2, 16.4, 31.6], [-45.2, -16.4, 31.6]],
    hold: Some(([-8.0, 0.0, 25.9], -50.0)),
};

/// Size of the stern drives against the Bastion's 12 m bells.
const DRIVE_SCALE: f32 = 0.55;
/// What `entity.wgsl` animates (`models::capital_rig`): the drives' glow and iris vanes, the
/// lift jets' glow (one mouth height covers both pairs: 4.6 fore, 6.6 aft, glow 4 m up).
/// No legs (it sets down on skids) and no ramp (plug doors, `part::HOLD_DOOR`).
pub const RIG: CapitalRig = CapitalRig {
    legs: None,
    door_hinge: 0.0,
    drives: Some(([NOZZLES[1][0], NOZZLES[1][2], NOZZLES[1][1], NOZZLES[1][1]], DRIVE_SCALE)),
    lift_jets: Some(([LIFT_JETS[3][0], LIFT_JETS[3][1], LIFT_JETS[1][0], LIFT_JETS[1][1]], LIFT_JETS[3][2])),
    ramp: None,
};

/// Half width of the tunnel: every inner wall stands at or outside it.
const BAY: f32 = 14.0;
/// The tunnel's roof (the hull's underside over the bay).
const CEILING: f32 = 26.2;
/// The stern door plane and the forward bulkhead.
const DOOR_X: f32 = -42.0;
const BULKHEAD_X: f32 = 24.0;

/// Shoulder stations: x, outer half width, crown. The keel steps up from the tunnel floor
/// to a chine; skids carry it on the ground.
const SHOULDERS: [[f32; 3]; 6] = [
    [-44.0, 30.0, 30.0],
    [-34.0, 35.0, 33.5],
    [-14.0, 38.0, 35.0],
    [2.0, 36.0, 35.5],
    [14.0, 31.0, 33.0],
    [23.0, 26.0, 30.0],
];

fn shoulder(x: f32, width: f32, top: f32) -> Vec<Vec3> {
    vec![
        v3(x, BAY, 0.0),
        v3(x, width - 8.0, 1.4),
        v3(x, width - 1.5, 7.0),
        v3(x, width, 12.0),
        v3(x, width, top - 9.0),
        v3(x, width - 7.0, top),
        v3(x, BAY, top),
    ]
}

/// Outer half width and crown of the shoulder at `x`.
fn shoulder_surface(x: f32) -> (f32, f32) {
    for p in SHOULDERS.windows(2) {
        if x <= p[1][0] {
            let t = ((x - p[0][0]) / (p[1][0] - p[0][0])).clamp(0.0, 1.0);
            return (p[0][1] + (p[1][1] - p[0][1]) * t, p[0][2] + (p[1][2] - p[0][2]) * t);
        }
    }
    (26.0, 30.0)
}

/// An octagonal ring about the x axis through (y, z): half size `r`, corners cut by `cut`.
fn octagon(x: f32, y: f32, z: f32, r: f32, cut: f32) -> Vec<Vec3> {
    let k = r - cut;
    [(-k, -r), (k, -r), (r, -k), (r, k), (k, r), (-k, r), (-r, k), (-r, -k)]
        .iter()
        .map(|&(dy, dz)| v3(x, y + dy, z + dz))
        .collect()
}

pub(super) fn build(b: &mut MeshBuilder) {
    if b.coarse() {
        b.paint(PLATING_DARK);
        b.frustum_open(v3(-8.0, 0.0, 1.0), v2(80.0, 72.0), v2(64.0, 50.0), 29.0, v2(-6.0, 0.0));
        b.paint(PLATING);
        b.loft(&band(&[[22.0, 23.0, 3.0, 27.0, 16.0, 18.0, 27.0], [58.0, 5.0, 12.0, 9.0, 16.0, 6.0, 20.0]], 0, 2), true, true);
        b.paint(PLATING_DARK);
        for y in [-26.0, 26.0] {
            b.cuboid(v3(-52.0, y, 18.0), v3(16.0, 14.0, 14.0));
        }
        return;
    }
    hull(b);
    bay(b);
    prow(b);
    dorsal(b);
    stern(b);
    lamps(b);
    doors(b);
}

/// The two shoulders the bay runs between: dark hull, pale armour brows along their
/// crowns, a service belt with a lit rail down each flank, and landing skids.
fn hull(b: &mut MeshBuilder) {
    b.mirror_y(|b| {
        b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
        let rings = SHOULDERS.iter().map(|p| shoulder(p[0], p[1], p[2])).collect::<Vec<_>>();
        b.loft(&rings, true, true);
        // Armour brows over the shoulders' upper edges, broken into three plates.
        for (x0, x1) in [(-40.0, -22.0), (-19.0, 0.0), (3.0, 20.0)] {
            let section = |x: f32| {
                let (w, h) = shoulder_surface(x);
                vec![
                    v3(x, w - 13.0, h + 0.2),
                    v3(x, w - 7.5, h + 1.6),
                    v3(x, w + 1.2, h - 6.0),
                    v3(x, w + 1.0, h - 11.0),
                    v3(x, w - 1.0, h - 11.6),
                    v3(x, w - 6.0, h - 0.2),
                ]
            };
            b.paint(PLATING).pattern(pattern::AIRFRAME);
            b.loft(&[section(x0), section(x1)], true, true);
            // A run of hatch lids let into the brow's crown.
            let on_top = |x: f32, u: f32, lift: f32| {
                let (w, h) = shoulder_surface(x);
                let (a, c) = (v2(w - 13.0, h + 0.2), v2(w - 7.5, h + 1.6));
                let n = v2(-(c.y - a.y), c.x - a.x).normalize();
                let q = a + (c - a) * u + n * lift;
                v3(x, q.x, q.y)
            };
            let strip = |x: f32, lift: f32| {
                vec![on_top(x, 0.15, -0.3), on_top(x, 0.85, -0.3), on_top(x, 0.85, lift), on_top(x, 0.15, lift)]
            };
            b.paint(ACCENT);
            b.loft(&[strip(x0 + 1.5, 0.2), strip(x1 - 1.5, 0.2)], true, true);
            if b.fine() {
                b.paint(METAL).pattern(pattern::PLAIN);
                let cells = ((x1 - x0 - 3.0) / 4.5).floor().max(1.0) as usize;
                for k in 0..cells {
                    let xa = x0 + 1.5 + (x1 - x0 - 3.0) * (k as f32 + 0.15) / cells as f32;
                    let xb = x0 + 1.5 + (x1 - x0 - 3.0) * (k as f32 + 0.85) / cells as f32;
                    let lid = |x: f32| {
                        vec![on_top(x, 0.25, 0.15), on_top(x, 0.75, 0.15), on_top(x, 0.75, 0.4), on_top(x, 0.25, 0.4)]
                    };
                    b.loft(&[lid(xa), lid(xb)], true, true);
                }
            }
            if b.mid() {
                // The owner's stripe down the brow's outer face.
                let face = |x: f32, s: f32| {
                    let (w, h) = shoulder_surface(x);
                    v3(x, w + 1.25, h - 7.0 - s)
                };
                b.paint(TEAM);
                b.beam(face(x0 + 2.0, 0.0), face(x1 - 2.0, 0.0), v2(0.25, 1.1), v2(0.25, 1.1));
            }
        }
        // A service belt down the flank, standing proud, with a lit rail and brackets.
        let (x0, x1) = (-38.0, 18.0);
        let (w0, _) = shoulder_surface(x0);
        let (wm, _) = shoulder_surface(-14.0);
        let (w1, _) = shoulder_surface(x1);
        b.paint(ACCENT);
        b.beam(v3(x0, w0 + 0.2, 15.5), v3(-14.0, wm + 0.2, 15.5), v2(2.2, 4.0), v2(2.2, 4.0));
        b.beam(v3(-14.0, wm + 0.2, 15.5), v3(x1, w1 + 0.2, 15.5), v2(2.2, 4.0), v2(2.2, 4.0));
        if b.fine() {
            b.paint(GLOW_AMBER);
            b.beam(v3(x0 + 1.0, w0 + 1.35, 17.0), v3(-14.0, wm + 1.35, 17.0), v2(0.3, 0.35), v2(0.3, 0.35));
            b.beam(v3(-14.0, wm + 1.35, 17.0), v3(x1 - 1.0, w1 + 1.35, 17.0), v2(0.3, 0.35), v2(0.3, 0.35));
            b.paint(PLATING_DARK);
            for x in [-34.0, -26.0, -18.0, -10.0, -2.0, 6.0, 14.0] {
                let (w, _) = shoulder_surface(x);
                b.beam(v3(x, w + 1.0, 13.0), v3(x, w + 1.0, 18.2), v2(1.2, 1.3), v2(1.2, 1.3));
            }
            // Pale access panels between belt and chine.
            b.paint(PLATING).pattern(pattern::PLAIN);
            for x in [-29.0, 1.0, 12.0] {
                let (w, _) = shoulder_surface(x);
                b.cuboid(v3(x, w + 0.1, 21.5), v3(6.0, 0.3, 4.0));
            }
        }
        // Heavy skids under the chine: the hull rests on these on the ground.
        b.paint(ACCENT).pattern(pattern::AIRFRAME);
        b.frustum(v3(-10.0, 24.0, 0.0), v2(52.0, 7.0), v2(56.0, 8.0), 1.5, v2(0.0, 0.0));
        // Radiator terrace on each shoulder's widest crown: louvres over a hot tray.
        let (x, y) = (-12.0, 19.5);
        let (_, h) = shoulder_surface(x);
        b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
        b.frustum_open(v3(x, y, h - 1.5), v2(16.0, 8.0), v2(14.0, 6.0), 2.6, v2(-0.5, 0.0));
        if b.mid() {
            b.paint(ACCENT);
            b.plate(v3(x - 0.5, y, h + 1.1), v2(12.5, 5.2), 0.2, 0.1);
            for k in 0..5 {
                let xx = x - 5.0 + k as f32 * 2.4;
                b.paint(PLATING_DARK);
                b.beam(v3(xx, y - 2.6, h + 2.0), v3(xx, y + 2.6, h + 2.0), v2(0.7, 1.6), v2(0.7, 1.6));
                if b.fine() && k < 4 {
                    b.paint(GLOW_ORANGE);
                    b.beam(v3(xx + 1.2, y - 2.3, h + 1.35), v3(xx + 1.2, y + 2.3, h + 1.35), v2(0.4, 0.15), v2(0.4, 0.15));
                }
            }
        }
    });
}

/// The ground-level tunnel: a walkway deck, ribs down its walls, a lit ceiling, and the
/// forward bulkhead with its hatch.
fn bay(b: &mut MeshBuilder) {
    b.paint(PLATING_DARK).pattern(pattern::WALKWAY);
    b.cuboid(v3((DOOR_X - 1.0 + BULKHEAD_X) * 0.5, 0.0, 0.04), v3(BULKHEAD_X - DOOR_X + 1.0, BAY * 2.0, 0.08));
    // Roof over the bay; the dorsal sits on it.
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.frustum(v3(-10.5, 0.0, CEILING), v2(68.0, 30.0), v2(60.0, 25.0), 6.6, v2(-1.5, 0.0));
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.cuboid(v3(BULKHEAD_X + 1.0, 0.0, 13.0), v3(2.0, BAY * 2.0, 26.0));
    if b.mid() {
        // Ceiling light strips down the bay, seen from behind through the open doors.
        b.paint(GLOW);
        for y in [-7.0, 7.0] {
            b.cuboid(v3(-9.0, y, CEILING - 0.08), v3(56.0, 1.2, 0.1));
        }
    }
    b.mirror_y(|b| {
        if b.fine() {
            b.paint(METAL);
            for x in [-34.0, -23.0, -12.0, -1.0, 10.0, 20.0] {
                b.cuboid(v3(x, BAY + 0.35, 13.0), v3(0.8, 0.5, 26.0));
                b.cuboid(v3(x, BAY - 2.0, CEILING - 0.09), v3(0.8, 4.0, 0.16));
            }
        }
        if b.fine() {
            b.paint(GLOW_AMBER);
            for x in [-28.5, -17.5, -6.5, 4.5, 15.0] {
                b.cuboid(v3(x, BAY - 0.05, 23.5), v3(1.2, 0.12, 0.35));
                b.cuboid(v3(x, BAY - 0.05, 1.2), v3(2.4, 0.12, 0.25));
            }
            // Tie-down points and cable reels along the wall's foot.
            b.paint(ACCENT);
            for x in [-30.0, -12.0, 6.0] {
                b.chamfered_box(v3(x, BAY + 0.4, 3.0), v3(5.0, 0.8, 3.0), 0.3);
            }
            // Bulkhead: a pressure hatch outlined in amber, and stowage lockers.
            let face = BULKHEAD_X - 0.05;
            b.paint(PLATING).pattern(pattern::PLAIN);
            b.cuboid(v3(face, 7.5, 12.0), v3(0.3, 7.0, 18.0));
            b.paint(GLOW_AMBER);
            b.cuboid(v3(face - 0.2, 3.2, 9.0), v3(0.12, 0.3, 14.0));
            b.cuboid(v3(face - 0.2, 0.0, 16.1), v3(0.12, 6.7, 0.3));
        }
    });
    if b.fine() {
        b.paint(ACCENT);
        b.cuboid(v3(BULKHEAD_X - 0.1, 0.0, 8.0), v3(0.3, 6.0, 16.0));
        b.paint(GLOW);
        b.cuboid(v3(BULKHEAD_X - 0.3, 0.0, 19.0), v3(0.1, 3.0, 0.8));
    }
}

/// Long wedge prow with split armour jaws, a raked cockpit canopy, a chin sensor keel,
/// the forward lift jets and the landing floods.
fn prow(b: &mut MeshBuilder) {
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.loft(
        &band(
            &[
                [20.0, 20.0, 2.0, 27.0, 13.0, 22.0, 26.0, 14.0, 29.0],
                [40.0, 11.0, 5.5, 18.0, 13.0, 16.0, 22.0, 10.0, 25.0],
                [59.0, 4.0, 11.0, 8.0, 15.0, 7.0, 19.0, 4.0, 20.0],
            ],
            0,
            3,
        ),
        true,
        true,
    );
    b.mirror_y(|b| {
        // Pale armour jaws over the upper cheeks, a dark gap left beneath them.
        let cheek = |x: f32, inside: f32, outside: f32, base: f32, top: f32| {
            vec![
                v3(x, inside, base),
                v3(x, outside - 3.0, base - 1.5),
                v3(x, outside, base + 3.0),
                v3(x, outside - 4.0, top),
                v3(x, inside + 2.0, top),
            ]
        };
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        b.loft(
            &[cheek(23.5, 10.5, 26.5, 18.5, 29.4), cheek(39.5, 8.0, 19.0, 16.0, 26.0), cheek(56.0, 4.5, 9.5, 14.8, 20.8)],
            true,
            true,
        );
        // Chin outrigger, carrying the forward lift jet.
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        b.loft(
            &[
                vec![v3(24.0, 13.0, 3.0), v3(24.0, 21.0, 5.5), v3(24.0, 20.0, 9.0), v3(24.0, 13.0, 8.0)],
                vec![v3(46.0, 5.0, 7.0), v3(46.0, 12.5, 8.5), v3(46.0, 11.5, 11.0), v3(46.0, 5.0, 10.5)],
            ],
            true,
            true,
        );
        if b.mid() {
            b.paint(TEAM);
            b.beam(v3(22.0, 24.5, 26.2), v3(50.0, 11.5, 19.8), v2(1.6, 0.3), v2(1.0, 0.3));
        }
        if b.fine() {
            b.paint(ACCENT);
            b.beam(v3(23.0, 19.0, 30.0), v3(48.0, 9.0, 24.6), v2(1.8, 0.35), v2(1.2, 0.3));
            b.paint(GLOW_AMBER);
            b.beam(v3(26.0, 22.4, 19.0), v3(46.0, 15.0, 15.0), v2(0.25, 0.3), v2(0.25, 0.3));
        }
    });
    // Cockpit: a faceted canopy on the prow's back behind a dark visor brow.
    b.paint(ACCENT);
    b.loft(&band(&[[22.0, 9.0, 29.2, 6.5, 31.6], [37.0, 7.5, 25.6, 5.0, 27.6]], 0, 1), true, true);
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.loft(&band(&[[23.0, 7.4, 30.9, 5.0, 33.0], [35.5, 6.2, 27.0, 3.8, 28.9]], 0, 1), true, true);
    // Raked window slits across the hood's front, dark glass.
    b.paint(GLASS);
    b.loft(&band(&[[31.0, 6.75, 28.75, 4.45, 30.55], [36.0, 6.2, 27.0, 3.8, 28.9]], 0, 1), true, true);
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.loft(&band(&[[20.0, 9.5, 31.0, 6.0, 33.8], [24.0, 8.2, 32.8, 5.2, 34.2]], 0, 1), true, true);
    if b.fine() {
        // Canopy frames.
        b.paint(METAL);
        for x in [26.5, 30.0, 33.5] {
            let t = (x - 23.0) / 12.5;
            let z = 33.0 + (28.9 - 33.0) * t;
            let y = 5.0 + (3.8 - 5.0) * t;
            b.beam(v3(x, -y, z - 0.35), v3(x, y, z - 0.35), v2(0.45, 1.0), v2(0.45, 1.0));
        }
    }
    // Chin sensor keel and the nose's lamp.
    b.paint(ACCENT);
    b.loft(&band(&[[40.0, 4.0, 4.5, 3.0, 8.0], [58.0, 3.0, 9.5, 2.2, 12.0]], 0, 1), true, true);
    b.paint(GLASS);
    b.cuboid(v3(59.1, 0.0, 16.2), v3(0.3, 7.0, 2.0));
    b.paint(GLOW);
    b.cuboid(v3(59.3, 0.0, 16.2), v3(0.15, 2.0, 0.4));
    if b.fine() {
        b.paint(METAL);
        b.cylinder_between(v3(58.0, 0.0, 10.5), v3(63.0, 0.0, 10.8), 0.35, 0.2, 5);
        b.paint(GLOW_RED);
        b.cuboid(v3(58.3, 0.0, 12.3), v3(0.4, 1.0, 0.5));
    }
    for port in &LIFT_JETS[2..] {
        let c = Vec3::from(*port);
        b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
        b.frustum(c + v3(0.0, 0.0, 1.8), v2(8.0, 7.0), v2(9.0, 7.5), 2.2, v2(0.5, 0.0));
        capital::lift_jet(b, c, 0.62);
    }
}

/// The dorsal: a stepped spine over the bay roof, a radiator bank, a sensor house and a
/// mast with a turning radar bar (`part::SPINNER`), sensor domes and the owner's panel.
fn dorsal(b: &mut MeshBuilder) {
    let roof = CEILING + 6.6;
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.frustum_open(v3(-10.0, 0.0, roof - 0.2), v2(46.0, 22.0), v2(40.0, 17.0), 3.2, v2(-1.0, 0.0));
    let spine = roof + 3.0;
    // A dark gallery belt round the spine.
    b.paint(ACCENT);
    b.frustum_open(v3(-10.3, 0.0, roof + 1.1), v2(45.6, 20.8), v2(45.2, 20.4), 0.9, v2(0.0, 0.0));
    // Radiator bank aft: fins over a hot tray, between low side rails.
    b.paint(ACCENT);
    b.plate(v3(-22.0, 0.0, spine), v2(14.0, 12.0), 0.5, 0.2);
    if b.mid() {
        for k in 0..6 {
            let x = -28.0 + k as f32 * 2.4;
            b.paint(PLATING_DARK);
            b.beam(v3(x, -5.2, spine + 1.8), v3(x, 5.2, spine + 1.8), v2(0.7, 2.6), v2(0.7, 2.6));
            if b.fine() && k < 5 {
                b.paint(GLOW_ORANGE);
                b.beam(v3(x + 1.2, -4.8, spine + 0.7), v3(x + 1.2, 4.8, spine + 0.7), v2(0.45, 0.15), v2(0.45, 0.15));
            }
        }
        b.paint(METAL);
        b.mirror_y(|b| {
            b.beam(v3(-29.5, 6.2, spine + 1.4), v3(-14.5, 6.2, spine + 1.4), v2(0.6, 0.8), v2(0.6, 0.8));
        });
    }
    // Sensor house forward, glazed slit round its face.
    let deck = spine;
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.frustum(v3(0.0, 0.0, deck - 0.2), v2(15.0, 13.0), v2(12.5, 10.5), 4.2, v2(-0.6, 0.0));
    b.paint(GLASS);
    b.frustum(v3(0.2, 0.0, deck + 2.2), v2(14.0, 12.0), v2(13.2, 11.3), 1.1, v2(-0.3, 0.0));
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.frustum(v3(-0.4, 0.0, deck + 3.8), v2(13.6, 11.6), v2(11.0, 9.0), 1.0, v2(-0.4, 0.0));
    b.paint(TEAM);
    b.frustum(v3(0.0, 0.0, deck + 0.6), v2(15.0, 13.1), v2(14.8, 12.9), 0.8, v2(-0.1, 0.0));
    team_panel(b, v3(-10.6, 0.0, spine), v2(4.2, 8.0));
    // The mast, its yard and the radar bar turning on top.
    let top = deck + 4.8;
    b.paint(METAL);
    b.beam(v3(-3.0, 0.0, top), v3(-3.4, 0.0, top + 8.0), v2(1.2, 1.2), v2(0.7, 0.7));
    if b.mid() {
        b.beam(v3(-3.2, -3.8, top + 4.0), v3(-3.2, 3.8, top + 4.0), v2(0.45, 0.45), v2(0.45, 0.45));
        b.set_spinner_pivot(v3(-3.4, 0.0, top + 8.0));
        b.with_part(part::SPINNER, |b| {
            b.paint(ACCENT);
            b.cuboid(v3(-3.4, 0.0, top + 8.6), v3(1.3, 8.0, 1.1));
            b.paint(METAL);
            b.cuboid(v3(-3.4, 0.0, top + 8.1), v3(1.0, 1.0, 0.6));
        });
    }
    if b.fine() {
        b.paint(GLOW_RED);
        b.cuboid(v3(-3.4, 0.0, top + 9.6), v3(0.5, 0.5, 0.5));
        b.cuboid(v3(-3.2, 4.0, top + 4.2), v3(0.4, 0.4, 0.4));
        b.cuboid(v3(-3.2, -4.0, top + 4.2), v3(0.4, 0.4, 0.4));
        b.paint(METAL);
        b.mirror_y(|b| {
            b.beam(v3(-0.5, 2.6, top), v3(-3.1, 0.4, top + 4.0), v2(0.35, 0.35), v2(0.25, 0.25));
            // Whip aerials at the sensor house's rear corners.
            b.beam(v3(-5.5, 4.6, deck + 4.2), v3(-5.7, 4.6, deck + 9.5), v2(0.22, 0.22), v2(0.12, 0.12));
        });
    }
    // Sensor domes either side of the spine's aft end.
    b.mirror_y(|b| {
        b.paint(PLATING_DARK);
        let n = b.sides(10);
        b.prism(v3(-37.5, 7.0, roof - 0.3), n, 2.6, 2.4, 1.0);
        b.paint(PLATING).pattern(pattern::PLAIN);
        b.spheroid(v3(-37.5, 7.0, roof + 1.2), v3(2.1, 2.1, 1.6), b.sides(8), 3);
    });
}

/// The drive nacelles fused to the aft shoulders, the door portal between them, the
/// stern's lights, and the aft lift jets under the nacelles.
fn stern(b: &mut MeshBuilder) {
    b.mirror_y(|b| {
        let [nx, y, z] = NOZZLES[1];
        // Nacelle: an octagonal housing growing out of the shoulder.
        let aft = nx + 27.6 * DRIVE_SCALE + 0.4;
        b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
        b.loft(&[octagon(-18.0, y, z, 6.0, 2.5), octagon(-28.0, y, z, 9.6, 3.4), octagon(aft + 2.0, y, z, 9.8, 3.4), octagon(aft, y, z, 9.0, 3.2)], true, true);
        // Pale armour saddle over the nacelle, with the owner's band.
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        // (y, z) off the nacelle axis: a shell over its top and outer side.
        const SADDLE: [[f32; 2]; 8] = [[-6.7, 10.6], [6.7, 10.6], [10.6, 6.7], [10.6, 2.0], [9.9, 2.0], [9.9, 6.4], [6.4, 9.9], [-6.4, 9.9]];
        let saddle = |x: f32| SADDLE.iter().map(|&[dy, dz]| v3(x, y + dy, z + dz)).collect::<Vec<_>>();
        b.loft(&[saddle(-26.0), saddle(aft + 3.0)], true, true);
        b.paint(TEAM);
        b.beam(v3(-31.0, y - 5.0, z + 10.5), v3(-31.0, y + 5.0, z + 10.5), v2(1.4, 0.2), v2(1.4, 0.2));
        // Thrust frame: a collar round the can where it leaves the housing.
        b.paint(ACCENT);
        b.loft(&[octagon(aft + 0.2, y, z, 9.4, 3.3), octagon(aft - 1.8, y, z, 8.6, 3.0)], false, true);
        capital::drive(b, Vec3::from(NOZZLES[1]), DRIVE_SCALE);
        // Aft lift jet under the nacelle.
        let jet = Vec3::from(LIFT_JETS[1]);
        capital::lift_jet(b, jet, 0.7);
        if b.fine() {
            // Stern lights: red at the nacelle's outboard corner, amber along its head.
            b.paint(GLOW_RED);
            b.cuboid(v3(aft - 0.2, y + 8.0, z + 6.5), v3(0.6, 1.0, 1.0));
            b.paint(GLOW_AMBER);
            b.beam(v3(-27.0, y + 9.7, z + 1.0), v3(aft + 2.0, y + 9.9, z + 1.0), v2(0.3, 0.3), v2(0.3, 0.3));
            // Actuator fairings between nacelle and shoulder.
            b.paint(METAL);
            for dz in [-5.5, 5.5] {
                b.cylinder_between(v3(-29.0, y - 8.0, z + dz), v3(aft + 1.0, y - 8.0, z + dz), 0.7, 0.7, 6);
            }
        }
        // Door portal: a heavy frame round the opening, hazard-striped posts.
        b.paint(PLATING).pattern(pattern::HAZARD);
        b.block(v3(DOOR_X - 3.2, BAY, 0.0), v3(DOOR_X - 0.45, BAY + 3.4, CEILING));
        b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
        b.block(v3(DOOR_X - 3.4, BAY + 3.4, 0.0), v3(DOOR_X - 0.45, BAY + 5.4, CEILING + 3.0));
        if b.fine() {
            b.paint(GLOW_AMBER);
            b.cuboid(v3(DOOR_X - 3.3, BAY + 0.4, 13.0), v3(0.15, 0.3, 22.0));
            b.paint(METAL);
            for z in [4.0, 13.0, 22.0] {
                b.cuboid(v3(DOOR_X - 3.5, BAY + 4.4, z), v3(0.5, 1.4, 1.6));
            }
        }
        // Beacon housing on the lintel's end.
        b.paint(ACCENT);
        let bc = Vec3::from(LAMPS.beacons[0]);
        b.prism(bc - v3(0.0, 0.0, 1.4), b.sides(8), 1.0, 0.9, 1.0);
        b.paint(GLOW_AMBER);
        b.prism(bc - v3(0.0, 0.0, 0.4), b.sides(8), 0.7, 0.6, 0.8);
    });
    // Lintel over the opening and the aft fairing above it.
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.block(v3(DOOR_X - 3.4, -BAY - 3.4, CEILING), v3(DOOR_X - 0.45, BAY + 3.4, CEILING + 4.6));
    b.paint(PLATING).pattern(pattern::HAZARD);
    b.block(v3(DOOR_X - 3.6, -BAY, CEILING - 0.0), v3(DOOR_X - 3.4, BAY, CEILING + 1.6));
    if b.fine() {
        b.paint(GLOW_AMBER);
        b.cuboid(v3(DOOR_X - 3.7, 0.0, CEILING + 2.8), v3(0.15, 20.0, 0.3));
        b.paint(GLOW_RED);
        b.cuboid(v3(DOOR_X - 3.6, 0.0, CEILING + 3.9), v3(0.4, 2.0, 0.6));
    }
}

/// Fittings for the lamps the renderer lights (`LAMPS`): flood housings, nav pods, strobes.
fn lamps(b: &mut MeshBuilder) {
    for (i, at) in LAMPS.floods.iter().enumerate() {
        let c = Vec3::from(*at);
        b.paint(ACCENT);
        b.cuboid(c + v3(0.0, 0.0, 0.8), v3(2.6, 2.0, 1.2));
        b.paint(GLOW);
        b.cuboid(c + v3(if i < 2 { 0.4 } else { 0.0 }, 0.0, 0.15), v3(1.6, 1.2, 0.12));
    }
    for (at, glow) in [(LAMPS.nav_port, GLOW_RED), (LAMPS.nav_starboard, GLOW)] {
        let c = Vec3::from(at);
        let out = c.y.signum();
        b.paint(PLATING_DARK);
        b.beam(c - v3(0.0, out * 2.0, 0.0), c - v3(0.0, out * 0.4, 0.0), v2(2.6, 1.6), v2(2.0, 1.2));
        b.paint(glow);
        b.cuboid(c, v3(1.2, 0.8, 0.8));
    }
    for at in &LAMPS.strobes[1..] {
        let c = Vec3::from(*at);
        b.paint(METAL);
        b.beam(c - v3(0.0, 0.0, 1.2), c - v3(0.0, 0.0, 0.3), v2(0.5, 0.5), v2(0.35, 0.35));
        b.paint(GLOW);
        b.cuboid(c, v3(0.6, 0.6, 0.6));
    }
}

/// The stern plug doors slide sideways into the shoulders (`part::HOLD_DOOR`). No ramp.
fn doors(b: &mut MeshBuilder) {
    b.with_part(part::HOLD_DOOR, |b| {
        b.mirror_y(|b| {
            b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
            b.cuboid(v3(DOOR_X + 0.5, 7.0, 13.0), v3(1.0, 14.0, 26.0));
            if b.mid() {
                // Pale pressure plate with a raised frame, lit seam at the meeting edge.
                b.paint(PLATING).pattern(pattern::AIRFRAME);
                b.chamfered_box(v3(DOOR_X - 0.2, 7.2, 13.0), v3(0.4, 12.2, 23.0), 0.6);
                b.paint(GLOW_AMBER);
                b.cuboid(v3(DOOR_X - 0.1, 0.25, 13.0), v3(0.2, 0.3, 24.0));
            }
            if b.fine() {
                b.paint(ACCENT);
                for z in [6.0, 13.0, 20.0] {
                    b.cuboid(v3(DOOR_X - 0.5, 7.2, z), v3(0.25, 10.0, 1.0));
                }
                b.paint(TEAM);
                b.cuboid(v3(DOOR_X - 0.5, 10.5, 24.0), v3(0.2, 4.0, 0.6));
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{build_model_fitted, part};
    #[test]
    fn courier_has_enclosed_commander_bay_no_weapons_or_ramp_and_bounded_lods() {
        let model = build_model_fitted("light_transport", 58.0, 38.0, 1, &[]).unwrap();
        let tris = model.lods.each_ref().map(|lod| lod.indices.len() / 3);
        println!("Courier triangles: {tris:?}");
        assert!(tris[0] <= 9000 && tris[2] < 60);
        assert!(tris[1] as f32 <= tris[0] as f32 * 0.5 + 20.0);
        assert!(model.lods[0].vertices.iter().any(|v| v.part == part::HOLD_DOOR));
        assert!(model.lods[0].vertices.iter().any(|v| v.part == part::SPINNER));
        for v in &model.lods[0].vertices {
            assert!([part::HULL, part::HOLD_DOOR, part::SPINNER, part::DRIVE].contains(&v.part));
            if v.part != part::HOLD_DOOR && v.pos[0] > -40.0 && v.pos[0] < 23.0 && v.pos[2] > 0.1 && v.pos[2] < 26.0 {
                assert!(v.pos[1].abs() >= 13.8, "obstructed loading tunnel: {:?}", v.pos);
            }
        }
        // Nothing hangs under the ground it lands on.
        assert!(model.lods[0].vertices.iter().all(|v| v.pos[2] >= -0.61));
    }
}
