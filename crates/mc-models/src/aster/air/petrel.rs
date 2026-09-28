//! Petrel: the tech 1 light bomber. A slim, sharp-chined fuselage behind a long
//! black sensor nose, on a broad wing swept back about 32 degrees. A faceted jet
//! nacelle runs through each wing, its intake well out ahead of the leading
//! edge; the swept tailplane crosses a raked fin half way up. The bay doors
//! hang open under the belly over eight bombs in two rows. From above, a swept
//! cross with two long pods on it.
use super::*;

/// A wing section at `(y, z, lead, trail, thick)`: a slim lens round the chord.
fn section(s: (f32, f32, f32, f32, f32)) -> Vec<Vec3> {
    let (y, z, lead, trail, t) = s;
    let chord = lead - trail;
    vec![
        v3(lead, y, z),
        v3(lead - chord * 0.3, y, z + t * 0.5),
        v3(trail, y, z + t * 0.12),
        v3(trail, y, z - t * 0.08),
        v3(lead - chord * 0.3, y, z - t * 0.5),
    ]
}

fn wing_loft(b: &mut MeshBuilder, sections: &[(f32, f32, f32, f32, f32)]) {
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.loft(
        &sections.iter().map(|&s| section(s)).collect::<Vec<_>>(),
        true,
        true,
    );
}

/// A bomb centred on `c`, `half` long each way: a graphite case, a pale ogive
/// nose, box fins, the owner's band. Plain slab below the fine LOD.
fn bomb(b: &mut MeshBuilder, c: Vec3, half: f32, r: f32) {
    let k = half / 0.72;
    let (nose, tail) = (c + Vec3::X * half, c - Vec3::X * half);
    if !b.fine() {
        b.paint(PLATING_DARK);
        b.beam(tail, nose, v2(r * 1.7, r * 1.7), v2(r * 1.7, r * 1.7));
        return;
    }
    b.paint(PLATING_DARK);
    b.cylinder_between(
        tail + Vec3::X * 0.28 * k,
        nose - Vec3::X * 0.42 * k,
        r * 0.7,
        r,
        8,
    );
    b.paint(PLATING);
    b.cylinder_between(
        nose - Vec3::X * 0.42 * k,
        nose - Vec3::X * 0.14 * k,
        r,
        r * 0.62,
        8,
    );
    b.cylinder_between(nose - Vec3::X * 0.14 * k, nose, r * 0.62, 0.03, 8);
    b.paint(TEAM);
    b.cylinder_between(
        nose - Vec3::X * 0.56 * k,
        nose - Vec3::X * 0.46 * k,
        r + 0.005,
        r + 0.005,
        8,
    );
    // Box fins: four plates round the tail, their tips joined by a ring.
    b.paint(METAL);
    for i in 0..4 {
        let a = (i as f32 + 0.5) * std::f32::consts::FRAC_PI_2;
        let out = v3(0.0, a.cos(), a.sin());
        let root = tail + Vec3::X * 0.18 * k + out * r * 0.5;
        b.beam(root, root + out * r, v2(0.02, 0.36 * k), v2(0.02, 0.36 * k));
    }
    b.cylinder_between(tail, tail + Vec3::X * 0.3 * k, r * 1.35, r * 1.35, 8);
}

/// An open bomb bay under a keel `half` wide at `keel_z`, from `front` back to
/// `back`: a black well, both doors swung down and out, and eight bombs in two
/// rows of four.
fn bay(b: &mut MeshBuilder, front: f32, back: f32, half: f32, keel_z: f32) {
    let length = front - back;
    b.paint(ACCENT);
    b.cuboid(
        v3(front - length * 0.5, 0.0, keel_z - 0.03),
        v3(length, half * 2.0, 0.06),
    );
    if !b.coarse() {
        b.mirror_y(|b| {
            let hinge = v3(front - length * 0.5, half, keel_z - 0.02);
            b.with(
                Affine3A::from_translation(hinge) * Affine3A::from_rotation_x(-0.35),
                |b| {
                    b.paint(PLATING_DARK);
                    b.cuboid(v3(0.0, 0.0, -0.26), v3(length - 0.1, 0.04, 0.52));
                },
            );
        });
    }
    let bomb_half = (length / 4.0 - 0.1) * 0.5;
    for i in 0..4 {
        let x = front - length / 8.0 - i as f32 * length / 4.0;
        for y in [-half * 0.54, half * 0.54] {
            bomb(b, v3(x, y, keel_z - 0.24), bomb_half, 0.14);
        }
    }
}

/// A round nozzle can ending at `end`, black inside.
fn nozzle(b: &mut MeshBuilder, end: Vec3, r: f32) {
    b.paint(METAL);
    b.cylinder_between(end + Vec3::X * 0.6, end, r, r * 0.85, b.sides(10));
    if b.fine() {
        b.paint(ACCENT);
        b.cylinder_between(end, end + Vec3::X * 0.01, r * 0.65, r * 0.65, 8);
    }
}

/// Hull stations nose to tail: x, then (half width, height) at keel, chine,
/// shoulder, spine. Slim and round-backed, the chine a sharp edge down the nose.
const HULL: [[f32; 9]; 6] = [
    [6.2, 0.02, 0.98, 0.04, 1.02, 0.03, 1.08, 0.01, 1.1],
    [4.6, 0.22, 0.74, 0.52, 0.94, 0.34, 1.3, 0.1, 1.44],
    [2.8, 0.34, 0.62, 0.66, 0.9, 0.46, 1.46, 0.16, 1.62],
    [-1.5, 0.38, 0.6, 0.62, 0.9, 0.48, 1.46, 0.16, 1.6],
    [-4.2, 0.2, 0.86, 0.34, 0.98, 0.26, 1.3, 0.08, 1.38],
    [-5.5, 0.08, 1.02, 0.12, 1.06, 0.1, 1.2, 0.03, 1.24],
];
/// Mid-set swept wing: root, nacelle, tip.
const WING: [(f32, f32, f32, f32, f32); 3] = [
    (0.45, 1.0, 2.0, -1.9, 0.34),
    (2.2, 1.02, 0.9, -2.3, 0.3),
    (5.5, 1.12, -1.2, -2.6, 0.1),
];
/// The (left) nacelle's centre line.
const NACELLE_Y: f32 = 2.2;
/// Nacelle stations about its own centre line, same layout as [`HULL`]: a
/// faceted box with chamfered corners, the intake face at the front.
const NACELLE: [[f32; 9]; 4] = [
    [3.0, 0.3, 0.62, 0.44, 0.92, 0.36, 1.3, 0.14, 1.4],
    [2.2, 0.3, 0.6, 0.46, 0.92, 0.38, 1.32, 0.14, 1.44],
    [-1.8, 0.28, 0.62, 0.44, 0.94, 0.36, 1.3, 0.12, 1.4],
    [-2.5, 0.22, 0.7, 0.34, 0.96, 0.28, 1.22, 0.1, 1.3],
];
/// The black intake throat let into the nacelle's face.
const THROAT: [[f32; 9]; 2] = [
    [3.03, 0.23, 0.69, 0.35, 0.93, 0.28, 1.24, 0.1, 1.33],
    [2.7, 0.23, 0.69, 0.35, 0.93, 0.28, 1.24, 0.1, 1.33],
];
/// Where the (left) exhaust ends (`aircraft_exhausts`).
const NOZZLE: Vec3 = Vec3::new(-3.0, NACELLE_Y, 1.0);
const FIN: [[f32; 2]; 4] = [[-5.55, 1.2], [-3.3, 1.35], [-5.0, 3.1], [-5.65, 3.1]];
/// Swept tailplane in plan, set half way up the fin.
const TAILPLANE: [[f32; 2]; 4] = [[-3.95, 0.05], [-5.05, 2.2], [-5.6, 2.2], [-5.45, 0.05]];
const TAIL_Z: f32 = 1.95;

pub(super) fn build(b: &mut MeshBuilder) {
    if b.coarse() {
        coarse(b);
        return;
    }
    let fine = b.fine();

    b.paint(PLATING_DARK);
    b.loft(&band(&HULL[..2], 0, 3), true, true);
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.loft(&band(&HULL[1..], 1, 3), true, true);
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.loft(&band(&HULL[1..], 0, 1), true, true);
    team_panel(b, v3(-1.0, 0.0, 1.6), v2(1.4, 0.26));
    if fine {
        // The bomb-aimer's window in the nose, a chin sensor and a pitot probe.
        b.paint(GLASS);
        b.plate(v3(4.9, 0.0, 1.3), v2(0.6, 0.34), 0.04, 0.02);
        b.paint(GLASS);
        b.spheroid(v3(4.4, 0.0, 0.72), v3(0.3, 0.2, 0.14), 8, 4);
        b.paint(METAL);
        b.cylinder_between(v3(6.15, 0.0, 1.02), v3(6.8, 0.0, 1.02), 0.04, 0.015, 5);
        // A dark dorsal spine running back into the fin.
        b.paint(PLATING_DARK);
        b.beam(
            v3(2.6, 0.0, 1.58),
            v3(-3.6, 0.0, 1.56),
            v2(0.2, 0.09),
            v2(0.26, 0.09),
        );
    }
    bay(b, 2.3, -1.9, 0.5, 0.6);

    // Raked fin, dark-capped, the tailplane crossing it half way up.
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.extrude_y(&FIN, -0.08, 0.08);
    b.paint(PLATING_DARK);
    b.extrude_y(
        &[[-5.62, 2.75], [-4.66, 2.75], [-5.0, 3.1], [-5.65, 3.1]],
        -0.09,
        0.09,
    );

    b.mirror_y(|b| {
        wing_loft(b, &WING);
        let [_, mid, tip] = WING;
        if b.fine() {
            let (z, lead, _) = along(mid, tip, 2.75);
            b.paint(PLATING_DARK);
            b.beam(
                v3(lead + 0.02, 2.75, z),
                v3(tip.2 + 0.02, tip.0 - 0.05, tip.1),
                v2(0.14, 0.12),
                v2(0.1, 0.07),
            );
        }
        b.paint(TEAM).pattern(pattern::TEAM_BAND);
        b.beam(
            v3(tip.2 - 0.05, tip.0 - 0.45, tip.1 + 0.03),
            v3(tip.3 + 0.05, tip.0 - 0.45, tip.1 + 0.03),
            v2(0.35, 0.08),
            v2(0.35, 0.08),
        );
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        b.extrude_z(&TAILPLANE, TAIL_Z, TAIL_Z + 0.08);

        // The nacelle runs through the wing, its intake well out ahead.
        b.at(Vec3::Y * NACELLE_Y, |b| {
            b.paint(PLATING).pattern(pattern::AIRFRAME);
            b.loft(&band(&NACELLE, 1, 3), true, true);
            b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
            b.loft(&band(&NACELLE, 0, 1), true, true);
            b.paint(ACCENT);
            b.loft(&band(&THROAT, 0, 3), true, true);
        });
        nozzle(b, NOZZLE, 0.32);
    });
}

/// The chord line's height and edges at span `y`, between two sections.
fn along(a: (f32, f32, f32, f32, f32), c: (f32, f32, f32, f32, f32), y: f32) -> (f32, f32, f32) {
    let t = (y - a.0) / (c.0 - a.0);
    let mix = |p: f32, q: f32| p + (q - p) * t;
    (mix(a.1, c.1), mix(a.2, c.2), mix(a.3, c.3))
}

/// Far away: the hull, the swept wing with its two pods, the fin.
fn coarse(b: &mut MeshBuilder) {
    let stations = [HULL[0], HULL[2], HULL[5]];
    b.paint(PLATING);
    b.loft(&band(&stations, 1, 2), true, true);
    b.mirror_y(|b| {
        let [root, _, tip] = WING;
        b.paint(PLATING);
        b.face(&[
            v3(root.2, root.0, root.1),
            v3(tip.2, tip.0, tip.1),
            v3(tip.3, tip.0, tip.1),
            v3(root.3, root.0, root.1),
        ]);
        // The nacelle, seen from above.
        b.paint(PLATING_DARK);
        let (front, y) = (NACELLE[0][0], NACELLE_Y);
        b.face(&[
            v3(front, y - 0.45, 1.44),
            v3(front, y + 0.45, 1.44),
            v3(NOZZLE.x, y + 0.3, 1.3),
            v3(NOZZLE.x, y - 0.3, 1.3),
        ]);
    });
    b.paint(PLATING);
    let fin = FIN.map(|p| v3(p[0], 0.0, p[1]));
    b.face(&fin);
    b.face(&fin.into_iter().rev().collect::<Vec<_>>());
    // The bomb bay, for the muzzles.
    b.paint(PLATING_DARK);
    b.cuboid_open(v3(0.2, 0.0, 0.46), v3(4.2, 0.7, 0.22));
    b.paint(TEAM);
    b.decal(v3(-1.0, 0.0, 1.6), v2(1.4, 0.26));
}
