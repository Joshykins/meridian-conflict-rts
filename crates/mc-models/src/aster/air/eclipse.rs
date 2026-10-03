//! Eclipse: the tech 3 strategic bomber, a faceted flying wing in white with graphite
//! panels. A straight leading edge swept 37 degrees, a sawtooth trailing edge cut into
//! elevons with split drag rudders at the tips; a chined cockpit blade down the middle,
//! the AEB's capacitor bank along its spine and a V tail standing on the wing behind
//! it; two buried engines breathing through angular dorsal scoops and exhausting flat
//! into heat-tiled troughs. One AEB bomb (an electric bore's whole charge in a casing) hangs in an open bay
//! under the belly; the only light on it is the bomb's thin blue seams.
use super::*;

/// A wing station: `(y, z, lead, trail, thick)`, the chord at height `z`.
type Station = (f32, f32, f32, f32, f32);

const WING: [Station; 5] = [
    (0.0, 1.8, 8.6, -7.6, 2.6),
    (2.4, 1.78, 6.78, -5.7, 1.85),
    (5.0, 1.72, 4.8, -3.6, 1.0),
    (9.0, 1.65, 1.76, -6.8, 0.6),
    (15.0, 1.6, -2.8, -3.9, 0.12),
];
/// The upper skin's height over the chord, as a share of the thickness, at chord
/// fractions 0, 0.2, 0.6 and 1 (the facet lines of [`upper`]).
const CROWN: [(f32, f32); 4] = [(0.0, 0.0), (0.2, 0.5), (0.6, 0.42), (1.0, 0.06)];

/// The upper half of a faceted wing station: lead, two shoulders, trail.
fn upper(s: Station) -> Vec<Vec3> {
    let (y, z, lead, trail, t) = s;
    CROWN
        .iter()
        .map(|&(f, h)| v3(lead + (trail - lead) * f, y, z + t * h))
        .collect()
}

/// The lower half, the same way round as [`upper`]: trail, two shoulders, lead.
fn lower(s: Station) -> Vec<Vec3> {
    let (y, z, lead, trail, t) = s;
    let c = lead - trail;
    vec![
        v3(trail, y, z + t * 0.06),
        v3(lead - c * 0.6, y, z - t * 0.4),
        v3(lead - c * 0.18, y, z - t * 0.42),
        v3(lead, y, z),
    ]
}

/// A diamond through the station, for the reduced levels.
fn diamond(s: Station) -> Vec<Vec3> {
    let (y, z, lead, trail, t) = s;
    let c = lead - trail;
    vec![
        v3(lead, y, z),
        v3(lead - c * 0.4, y, z + t * 0.5),
        v3(trail, y, z),
        v3(lead - c * 0.4, y, z - t * 0.4),
    ]
}

/// The wing's station at span `y`, between the two either side.
fn station_at(y: f32) -> Station {
    let y = y.clamp(0.0, WING[4].0);
    let k = WING.windows(2).position(|w| y <= w[1].0).unwrap_or(3);
    let (a, c) = (WING[k], WING[k + 1]);
    let t = (y - a.0) / (c.0 - a.0);
    let mix = |p: f32, q: f32| p + (q - p) * t;
    (
        y,
        mix(a.1, c.1),
        mix(a.2, c.2),
        mix(a.3, c.3),
        mix(a.4, c.4),
    )
}

/// The leading and trailing edges at span `y`.
fn edges(y: f32) -> (f32, f32) {
    let (_, _, lead, trail, _) = station_at(y);
    (lead, trail)
}

/// The point on the upper skin over `(x, y)`.
fn top(x: f32, y: f32) -> Vec3 {
    let (_, z, lead, trail, t) = station_at(y);
    let f = ((lead - x) / (lead - trail)).clamp(0.0, 1.0);
    let k = CROWN.windows(2).position(|w| f <= w[1].0).unwrap_or(2);
    let (a, c) = (CROWN[k], CROWN[k + 1]);
    let h = a.1 + (c.1 - a.1) * (f - a.0) / (c.0 - a.0);
    v3(x, y, z + t * h)
}

/// The chord fraction of `x` at span `y`: 0 on the leading edge, 1 on the trailing.
fn fraction(x: f32, y: f32) -> f32 {
    let (lead, trail) = edges(y);
    (lead - x) / (lead - trail)
}

/// A panel laid on the upper skin, `lift` above it: the quad `front` (two plan points,
/// inboard first) to `back`, each side at one span. It is cut on the skin's facet lines
/// so every piece lies flat on its facet, never through a crease.
fn skin(b: &mut MeshBuilder, front: [[f32; 2]; 2], back: [[f32; 2]; 2], lift: f32) {
    let ends = [0, 1].map(|s| {
        let y = front[s][1];
        (y, fraction(front[s][0], y), fraction(back[s][0], y))
    });
    let mut cuts: Vec<[f32; 2]> = vec![ends.map(|e| e.1)];
    for (c, _) in &CROWN[1..3] {
        let inside = |e: &(f32, f32, f32)| (e.1.min(e.2)..e.1.max(e.2)).contains(c);
        if ends.iter().any(inside) {
            cuts.push(ends.map(|e| c.clamp(e.1.min(e.2), e.1.max(e.2))));
        }
    }
    cuts.push(ends.map(|e| e.2));
    let at = |f: f32, s: usize| {
        let (y, _, _) = ends[s];
        let (lead, trail) = edges(y);
        top(lead + (trail - lead) * f, y) + Vec3::Z * lift * 2.0
    };
    for w in cuts.windows(2) {
        let (f, g) = (w[0], w[1]);
        b.face(&[at(f[0], 0), at(f[1], 1), at(g[1], 1), at(g[0], 0)]);
    }
}

/// Where the elevons hinge at span `y`: this far ahead of the trailing edge.
const HINGE: f32 = 1.15;

/// The engines' centre line out along the span.
const ENGINE_Y: f32 = 3.6;
/// The cockpit blade, stations as `band` reads them: x, then (half width, height)
/// pairs from where it leaves the wing up to its crest.
const RIDGE: [[f32; 7]; 5] = [
    [8.2, 0.3, 1.98, 0.18, 2.12, 0.0, 2.16],
    [5.8, 1.25, 2.75, 0.8, 3.22, 0.0, 3.32],
    [3.0, 1.5, 2.95, 0.92, 3.5, 0.0, 3.6],
    [-1.0, 1.45, 2.85, 0.82, 3.35, 0.0, 3.45],
    [-4.8, 0.7, 2.2, 0.35, 2.5, 0.0, 2.55],
];
/// The engine scoop about its own centre line: x, then (half width, height over the
/// skin) at its foot and its roof.
const SCOOP: [(f32, f32, f32); 4] = [
    (3.7, 0.72, 0.55),
    (2.6, 0.82, 0.62),
    (-1.2, 0.78, 0.46),
    (-2.9, 0.6, 0.04),
];
/// Where the flat exhaust slot sits, ahead of the trailing edge.
const SLOT_X: f32 = -3.1;
pub(crate) const NOZZLES: [[f32; 3]; 2] = [[-3.25, -ENGINE_Y, 2.12], [-3.25, ENGINE_Y, 2.12]];

pub(super) fn build(b: &mut MeshBuilder) {
    if b.coarse() {
        coarse(b);
        return;
    }
    let fine = b.fine();
    b.mirror_y(|b| {
        if fine {
            b.paint(PLATING).pattern(pattern::AIRFRAME);
            b.loft(&WING.map(upper), true, true);
            b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
            b.loft(&WING.map(lower), true, true);
            surface(b);
        } else {
            b.paint(PLATING);
            b.loft(&WING.map(diamond), true, true);
        }
        engine(b);
        fin(b);
    });
    // The cockpit blade: white over a graphite chine.
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.loft(&band(&RIDGE, 1, 2), true, true);
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.loft(&band(&RIDGE, 0, 1), true, true);
    if fine {
        cockpit(b);
        nav_lights(b);
    }
    capacitors(b);
    team_panel(b, v3(-2.6, 0.0, 3.16), v2(1.1, 0.3));
    bay(b, 2.0, -2.6, 0.75, 1.0);
}

/// The detail on one wing's upper skin: armour plates, elevons and their hinge lines,
/// a dark leading edge, the split drag rudder at the tip, the owner's band.
fn surface(b: &mut MeshBuilder) {
    let hinge = |y: f32| edges(y).1 + HINGE;
    // Armour plates in two rows, a panel line between them at the sawtooth's corner.
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    for (y0, y1, ahead) in [(5.6, 8.7, 0.8), (9.3, 13.6, 0.6)] {
        let front = [[edges(y0).0 - ahead, y0], [edges(y1).0 - ahead, y1]];
        let back = [[hinge(y0) + 0.25, y0], [hinge(y1) + 0.25, y1]];
        skin(b, front, back, 0.025);
    }
    // An inboard plate either side of the engine.
    for (y0, y1) in [(1.6, 2.35), (4.55, 4.95)] {
        skin(
            b,
            [[edges(y0).0 - 1.6, y0], [edges(y1).0 - 1.6, y1]],
            [[-1.4, y0], [-1.2, y1]],
            0.025,
        );
    }
    // Elevons: three along the outer sawtooth, one along the inner, each a plate of
    // the other tone behind a dark hinge line.
    for (y0, y1) in [(5.25, 8.8), (9.2, 11.4), (11.6, 13.9)] {
        b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
        skin(
            b,
            [[hinge(y0) - 0.08, y0], [hinge(y1) - 0.08, y1]],
            [[edges(y0).1 + 0.04, y0], [edges(y1).1 + 0.04, y1]],
            0.02,
        );
        b.paint(ACCENT);
        skin(
            b,
            [[hinge(y0) + 0.02, y0], [hinge(y1) + 0.02, y1]],
            [[hinge(y0) - 0.08, y0], [hinge(y1) - 0.08, y1]],
            0.03,
        );
    }
    // The dark leading edge, root to tip.
    b.paint(PLATING_DARK);
    for (y0, y1) in [(2.4, 5.0), (5.0, 9.0), (9.0, 14.9)] {
        skin(
            b,
            [[edges(y0).0, y0], [edges(y1).0, y1]],
            [[edges(y0).0 - 0.4, y0], [edges(y1).0 - 0.3, y1]],
            0.02,
        );
    }
    // The owner's band across the outer wing.
    b.paint(TEAM).pattern(pattern::TEAM_BAND);
    skin(
        b,
        [[edges(12.6).0 - 0.35, 12.6], [edges(13.1).0 - 0.35, 13.1]],
        [[hinge(12.6) + 0.3, 12.6], [hinge(13.1) + 0.3, 13.1]],
        0.03,
    );
    // The split drag rudder, a little open: its upper leaf lifted, its lower dropped.
    let (y0, y1) = (14.0, 14.85);
    let (h0, h1) = (top(hinge(y0), y0), top(hinge(y1), y1));
    let (t0, t1) = (
        v3(edges(y0).1 - 0.1, y0, 0.0),
        v3(edges(y1).1 - 0.1, y1, 0.0),
    );
    b.paint(PLATING_DARK);
    for dz in [0.32, -0.3] {
        let (e0, e1) = (t0 + Vec3::Z * (h0.z + dz), t1 + Vec3::Z * (h1.z + dz));
        if dz > 0.0 {
            b.face(&[h0, h1, e1, e0]);
            b.face(&[e0, e1, h1, h0]);
        } else {
            let (l0, l1) = (h0 - Vec3::Z * 0.06, h1 - Vec3::Z * 0.06);
            b.face(&[l0, e0, e1, l1]);
            b.face(&[l1, e1, e0, l0]);
        }
    }
}

/// One engine: an angular scoop on the skin with a black throat behind a splitter
/// plate, and its flat slot exhausting into a heat-tiled trough to the trailing edge.
fn engine(b: &mut MeshBuilder) {
    let y = ENGINE_Y;
    let ring = |&(x, w, h): &(f32, f32, f32)| {
        vec![
            top(x, y + w) - Vec3::Z * 0.08,
            top(x, y + w * 0.7) + Vec3::Z * h,
            top(x, y - w * 0.7) + Vec3::Z * h,
            top(x, y - w) - Vec3::Z * 0.08,
        ]
    };
    b.paint(PLATING).pattern(pattern::AIRFRAME);
    b.loft(&SCOOP.iter().map(ring).collect::<Vec<_>>(), true, true);
    if !b.fine() {
        return;
    }
    let mouth = ring(&SCOOP[0]);
    let inner: Vec<Vec3> = mouth
        .iter()
        .map(|p| v3(p.x + 0.02, y + (p.y - y) * 0.8, p.z.max(mouth[0].z + 0.08)))
        .collect();
    b.paint(ACCENT);
    b.face(&[inner[0], inner[1], inner[2], inner[3]]);
    // The serrated lip: a sawtooth plate along the scoop's mouth.
    b.paint(METAL);
    let lip = mouth[1].z + 0.02;
    b.face(&[
        v3(SCOOP[0].0 + 0.35, y + 0.6, lip),
        v3(SCOOP[0].0 - 0.15, y + 0.6, lip),
        v3(SCOOP[0].0 + 0.1, y, lip),
        v3(SCOOP[0].0 - 0.15, y - 0.6, lip),
        v3(SCOOP[0].0 + 0.35, y - 0.6, lip),
    ]);
    // The boundary-layer splitter standing off the inboard side.
    b.paint(PLATING_DARK);
    let foot = top(SCOOP[0].0 + 0.4, y - 0.95);
    b.beam(
        foot + v3(0.0, 0.0, 0.25),
        top(1.6, y - 0.95) + Vec3::Z * 0.2,
        v2(0.04, 0.42),
        v2(0.04, 0.3),
    );
    // The trough: heat tiles from the slot to the trailing edge, a lip each side.
    let tail = edges(y).1;
    b.paint(METAL);
    skin(
        b,
        [[SLOT_X, y - 0.66], [SLOT_X, y + 0.66]],
        [[tail + 0.02, y - 0.66], [tail + 0.02, y + 0.66]],
        0.02,
    );
    b.paint(ACCENT);
    for k in 1..4 {
        let x = SLOT_X + (tail - SLOT_X) * k as f32 / 4.0;
        skin(
            b,
            [[x, y - 0.62], [x, y + 0.62]],
            [[x - 0.05, y - 0.62], [x - 0.05, y + 0.62]],
            0.035,
        );
    }
    b.paint(PLATING_DARK);
    for s in [-1.0, 1.0] {
        b.beam(
            top(SLOT_X + 0.3, y + 0.72 * s) + Vec3::Z * 0.06,
            top(tail, y + 0.75 * s) + Vec3::Z * 0.04,
            v2(0.1, 0.12),
            v2(0.08, 0.06),
        );
    }
    // The slot itself, black under a lid.
    let slot = top(SLOT_X, y);
    b.paint(ACCENT);
    b.beam(
        slot + v3(0.1, 0.0, 0.08),
        slot + v3(-0.12, 0.0, 0.08),
        v2(1.2, 0.16),
        v2(1.2, 0.16),
    );
    b.paint(PLATING_DARK);
    b.beam(
        slot + v3(0.5, 0.0, 0.2),
        slot + v3(-0.2, 0.0, 0.22),
        v2(1.3, 0.06),
        v2(1.3, 0.05),
    );
}

/// One fin of the V tail, canted out 35 degrees. It stands on the skin over its whole
/// root, the root following the skin's slope, and ends at the trailing edge.
fn fin(b: &mut MeshBuilder) {
    let (y, cant, height) = (1.5, 0.62, 2.5);
    let [front, back, tip_front, tip_back] = [-2.4, -6.3, -5.3, -6.5];
    let root = top((front + back) * 0.5, y);
    let sink = |x: f32| top(x, y).z - root.z - 0.12;
    let plan = [
        [back, sink(back)],
        [front, sink(front)],
        [tip_front, height],
        [tip_back, height],
    ];
    let place = Affine3A::from_translation(v3(0.0, y, root.z)) * Affine3A::from_rotation_x(-cant);
    b.with(place, |b| {
        b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
        b.extrude_y(&plan, -0.08, 0.08);
        if b.fine() {
            // A white face let into each side, the owner's band across the tip.
            let face = [
                [back + 0.45, sink(back + 0.45) + 0.45],
                [front - 0.9, sink(front - 0.9) + 0.45],
                [tip_front - 0.25, height - 0.35],
                [tip_back + 0.3, height - 0.35],
            ];
            b.paint(PLATING).pattern(pattern::AIRFRAME);
            b.extrude_y(&face, -0.1, 0.1);
            b.paint(TEAM).pattern(pattern::TEAM_BAND);
            b.extrude_y(
                &[
                    [tip_back + 0.05, height - 0.3],
                    [tip_front - 0.1, height - 0.3],
                    [tip_front + 0.05, height - 0.05],
                    [tip_back - 0.02, height - 0.05],
                ],
                -0.11,
                0.11,
            );
        }
    });
    if b.fine() {
        // A dark fillet along the root, where the fin meets the skin.
        b.paint(PLATING_DARK);
        b.beam(
            top(front - 0.2, y) + Vec3::Z * 0.02,
            top(back + 0.1, y) + Vec3::Z * 0.02,
            v2(0.36, 0.1),
            v2(0.3, 0.1),
        );
    }
}

/// The cockpit blade's glazing: a faceted visor round its brow, framed in graphite,
/// and a dark sensor eye under the nose.
fn cockpit(b: &mut MeshBuilder) {
    let r = |k: usize, pair: usize| {
        let s = RIDGE[k];
        v3(s[0], s[1 + 2 * pair], s[2 + 2 * pair])
    };
    let lerp = |p: Vec3, q: Vec3, t: f32| p + (q - p) * t;
    b.mirror_y(|b| {
        // The brow facet (shoulder to crest) and the cheek below it (chine to shoulder).
        for (lo, hi) in [(1, 2), (0, 1)] {
            let (a, c) = (r(0, lo), r(1, lo));
            let (d, e) = (r(0, hi), r(1, hi));
            let quad = [
                lerp(a, c, 0.45),
                lerp(a, c, 0.88),
                lerp(d, e, 0.88),
                lerp(d, e, 0.45),
            ];
            let quad = quad.map(|p| p + Vec3::Z * 0.03 + Vec3::Y * 0.01 * p.y.signum());
            b.paint(GLASS);
            b.face(&quad);
            b.face(&[quad[3], quad[2], quad[1], quad[0]]);
            // A frame bar across the middle.
            b.paint(PLATING_DARK);
            let (m0, m1) = (lerp(quad[0], quad[1], 0.5), lerp(quad[3], quad[2], 0.5));
            b.beam(m0, m1, v2(0.08, 0.05), v2(0.08, 0.05));
        }
    });
    b.paint(TREAD);
    b.spheroid(v3(6.6, 0.0, 1.0), v3(0.5, 0.32, 0.18), 8, 4);
}

/// Steady navigation lights at the wingtips, red to port and green to starboard.
fn nav_lights(b: &mut MeshBuilder) {
    let (lead, _) = edges(14.9);
    let z = top(lead - 0.5, 14.9).z;
    b.paint(GLOW_NAV_RED);
    b.cuboid(v3(lead - 0.5, 14.95, z), v3(0.3, 0.1, 0.08));
    b.paint(GLOW_NAV_GREEN);
    b.cuboid(v3(lead - 0.5, -14.95, z), v3(0.3, 0.1, 0.08));
}

/// The AEB's capacitor bank down the spine: a graphite strake, three steel drums, the
/// charge showing in thin seams down its sides.
fn capacitors(b: &mut MeshBuilder) {
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.beam(
        v3(2.6, 0.0, 3.55),
        v3(-4.6, 0.0, 2.55),
        v2(0.9, 0.3),
        v2(0.5, 0.25),
    );
    if !b.fine() {
        return;
    }
    b.paint(METAL);
    for (x, z) in [(1.9, 3.82), (0.0, 3.62), (-1.9, 3.38)] {
        b.cylinder_between(v3(x, 0.0, z), v3(x - 1.5, 0.0, z - 0.14), 0.3, 0.3, 8);
        b.paint(PLATING_DARK);
        b.cylinder_between(
            v3(x + 0.04, 0.0, z),
            v3(x - 0.1, 0.0, z - 0.01),
            0.33,
            0.33,
            8,
        );
        b.paint(METAL);
    }
    b.paint(GLOW);
    b.mirror_y(|b| {
        b.beam(
            v3(2.5, 0.46, 3.55),
            v3(-4.3, 0.26, 2.6),
            v2(0.04, 0.04),
            v2(0.04, 0.04),
        );
    });
}

/// Far away: the wing as three diamonds, the blade, the bay as a dark patch.
fn coarse(b: &mut MeshBuilder) {
    b.mirror_y(|b| {
        b.paint(PLATING);
        b.loft(&[WING[0], WING[2], WING[4]].map(diamond), false, false);
    });
    b.paint(PLATING);
    b.loft(&band(&[RIDGE[0], RIDGE[2], RIDGE[4]], 1, 2), false, false);
    b.paint(ACCENT);
    b.face(&[
        v3(-2.6, 0.75, 0.88),
        v3(2.0, 0.75, 0.88),
        v3(2.0, -0.75, 0.88),
        v3(-2.6, -0.75, 0.88),
    ]);
}

/// The AEB bomb, centred on `c` and lying along x, `half` long each way: a faceted
/// graphite case, a pale nose, a steel capacitor collar fore and aft, four thin blue
/// seams down its length where the charge shows, the owner's band, four swept fins.
fn bomb(b: &mut MeshBuilder, c: Vec3, half: f32, r: f32) {
    let (nose, tail) = (c + Vec3::X * half, c - Vec3::X * half);
    let x = Vec3::X;
    if !b.fine() {
        b.paint(PLATING_DARK);
        b.cylinder_between(tail + x * 0.5, nose - x * 0.8, r * 0.6, r, 6);
        b.paint(PLATING);
        b.cylinder_between(nose - x * 0.8, nose, r, 0.05, 6);
        return;
    }
    b.paint(PLATING_DARK);
    b.cylinder_between(tail + x * 0.9, nose - x * 1.0, r, r, 8);
    b.cylinder_between(tail + x * 0.15, tail + x * 0.9, r * 0.55, r, 8);
    b.paint(PLATING);
    b.cylinder_between(nose - x * 1.0, nose - x * 0.4, r, r * 0.6, 8);
    b.cylinder_between(nose - x * 0.4, nose, r * 0.6, 0.05, 8);
    b.paint(TEAM);
    b.cylinder_between(nose - x * 1.25, nose - x * 1.05, r + 0.01, r + 0.01, 8);
    b.paint(METAL);
    for at in [nose - x * 1.55, tail + x * 1.15] {
        b.cylinder_between(at, at + x * 0.3, r + 0.05, r + 0.05, 8);
    }
    // The seams: the charge showing between the case's plates.
    b.paint(GLOW);
    for i in 0..4 {
        let a = (i as f32 + 0.5) * std::f32::consts::FRAC_PI_2;
        let out = v3(0.0, a.cos(), a.sin()) * (r + 0.01);
        b.beam(
            tail + x * 1.5 + out,
            nose - x * 1.6 + out,
            v2(0.05, 0.05),
            v2(0.05, 0.05),
        );
    }
    // Swept fins in an X round the tail.
    b.paint(PLATING);
    for i in 0..4 {
        let a = (i as f32 + 0.5) * std::f32::consts::FRAC_PI_2;
        b.with(
            Affine3A::from_translation(tail) * Affine3A::from_rotation_x(a),
            |b| {
                b.extrude_y(
                    &[
                        [0.05, r * 0.4],
                        [1.0, r * 0.4],
                        [0.35, r * 1.6],
                        [0.0, r * 1.6],
                    ],
                    -0.03,
                    0.03,
                );
            },
        );
    }
}

/// An open bay under a keel `half` wide at `keel_z`, from `front` back to `back`:
/// a black well, both doors swung down, the bomb hanging half out of it.
fn bay(b: &mut MeshBuilder, front: f32, back: f32, half: f32, keel_z: f32) {
    let length = front - back;
    let mid = front - length * 0.5;
    b.paint(ACCENT);
    b.cuboid(v3(mid, 0.0, keel_z - 0.03), v3(length, half * 2.0, 0.06));
    b.mirror_y(|b| {
        let hinge = v3(mid, half, keel_z - 0.02);
        b.with(
            Affine3A::from_translation(hinge) * Affine3A::from_rotation_x(-0.3),
            |b| {
                b.paint(PLATING_DARK);
                b.cuboid(v3(0.0, 0.0, -0.4), v3(length - 0.1, 0.05, 0.8));
            },
        );
    });
    let r = (half * 0.62).min(0.5);
    bomb(b, v3(mid, 0.0, keel_z - r * 0.55), length * 0.5 - 0.15, r);
}
