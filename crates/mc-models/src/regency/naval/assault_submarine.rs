//! The Rapier, the Regency's tech 3 assault submarine (`regency_t3_submarine` in
//! `data/factions/regency/units/naval.ron`): the fastest hull under the sea, a
//! swallowtail. A slim chined pressure hull with big swept planes at its shoulders, so
//! from above it is a dart, two tail booms forking the stern, dark plate lapped back over
//! bronze workings, six torpedo tubes in fangs either side of the chin under the water
//! and, when it surfaces, light guns and anti-air lifted out of the deck
//! (docs/STYLE.md "The Regency look", "The navy").
//!
//! The origin is the waterline; the hull is drawn below it, the sail's top is the unit's
//! height. The weapon points are the unit file's:
//!
//! - 0, six bow tubes ([`TUBES`]): red-mouthed, fixed in the hull.
//! - 1, two interceptor doors astern ([`INTERCEPT`]).
//! - Houses 2 and 3, the twin repeaters fore and aft ([`FORE`], [`AFT`]): low plated
//!   cupolas that sit in the deck's lines, the aft one authored forward and rested round.
//! - House 4, the AA organ on the sail's back ([`AA`]), held up at the sky.
//! - 5, six seeker hatches in the sail's top ([`SEEKERS`]).

use glam::{Vec2, Vec3};

use crate::builder::{MeshBuilder, Section};
use crate::library::ModelDef;
use crate::material::*;

use super::super::kit::{dark_plate, metal, seam, v3};
use super::super::machine::{
    armour, collar, hoop, hoop_on, mouth_rim, red_slot, swept, Course, Frame,
};

const RADIUS: f32 = 26.0;
const HEIGHT: f32 = 5.0;

pub(crate) const MODELS: &[ModelDef] = &[ModelDef::new(
    "regency_assault_submarine",
    RADIUS,
    HEIGHT,
    swallowtail,
)];

/// The bow tubes' mouths, in the unit file's order.
const TUBES: [[f32; 3]; 6] = [
    [25.0, -1.0, -1.0],
    [25.0, 1.0, -1.0],
    [25.0, -1.0, -2.0],
    [25.0, 1.0, -2.0],
    [25.0, -2.0, -1.5],
    [25.0, 2.0, -1.5],
];
const BOW_FACE: f32 = 25.0;
/// The interceptor doors astern.
const INTERCEPT: [[f32; 3]; 2] = [[-25.0, -1.6, -2.0], [-25.0, 1.6, -2.0]];
/// The repeaters' pivots; their twin bores run [`BORE`] forward of it, [`BORE_Y`] apart
/// either side, [`BORE_RISE`] over it.
const FORE: Vec3 = Vec3::new(10.0, 0.0, 2.9);
const AFT: Vec3 = Vec3::new(-12.0, 0.0, 2.7);
const BORE: f32 = 3.0;
const BORE_Y: f32 = 0.35;
const BORE_RISE: f32 = 0.3;
/// The AA organ's trunnion on the sail's back, and the middle of its two tube mouths.
const AA: Vec3 = Vec3::new(-2.5, 0.0, 4.6);
const AA_MUZZLE: Vec3 = Vec3::new(-0.3, 0.0, 4.6);
/// The seeker hatches, in the unit file's order: the muzzles are their doors.
const SEEKERS: [[f32; 3]; 6] = [
    [4.0, -0.8, 4.5],
    [4.0, 0.8, 4.5],
    [2.5, -0.8, 4.5],
    [2.5, 0.8, 4.5],
    [1.0, -0.8, 4.5],
    [1.0, 0.8, 4.5],
];
/// The sail's flat top, where the hatches lie.
const SAIL_TOP: f32 = 4.42;

// ---- the hull ---------------------------------------------------------------------------

/// A hull station: where along it, its half width at the chine, and the heights of its
/// deck ridge, its chine and its keel.
#[derive(Clone, Copy, Debug)]
struct Station {
    x: f32,
    w: f32,
    top: f32,
    chine: f32,
    keel: f32,
}

const fn st(x: f32, w: f32, top: f32, chine: f32, keel: f32) -> Station {
    Station {
        x,
        w,
        top,
        chine,
        keel,
    }
}

/// A point of a hull: a station with no width, everything at `z`.
const fn tip(x: f32, z: f32) -> Station {
    st(x, 0.0, z, z, z)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// A chined cross-section: a low ridge along the deck, sloped upper flanks out to a hard
/// chine, a V under it to the keel. Fewer facets further off.
fn section(b: &MeshBuilder, s: Station) -> Vec<Vec3> {
    let (up, down) = (s.top - s.chine, s.chine - s.keel);
    let side: Vec<[f32; 2]> = if b.fine() {
        vec![
            [0.42, s.top - 0.1 * up],
            [0.78, s.top - 0.42 * up],
            [1.0, s.chine],
            [0.84, s.chine - 0.38 * down],
            [0.46, s.chine - 0.8 * down],
        ]
    } else if b.mid() {
        vec![
            [0.7, s.top - 0.3 * up],
            [1.0, s.chine],
            [0.6, s.chine - 0.65 * down],
        ]
    } else {
        vec![[1.0, s.chine]]
    };
    let mut ring = vec![v3(s.x, 0.0, s.top)];
    ring.extend(side.iter().map(|&[f, z]| v3(s.x, f * s.w, z)));
    ring.push(v3(s.x, 0.0, s.keel));
    ring.extend(side.iter().rev().map(|&[f, z]| v3(s.x, -f * s.w, z)));
    ring
}

/// A hull lofted through `stations`, stern to bow, or through `far` of them at the last
/// level of detail.
fn hull(b: &mut MeshBuilder, stations: &[Station], far: &[usize]) {
    let rings: Vec<Vec<Vec3>> = if b.coarse() {
        far.iter().map(|&i| section(b, stations[i])).collect()
    } else {
        stations.iter().map(|&s| section(b, s)).collect()
    };
    dark_plate(b);
    b.loft(&rings, true, true);
}

/// The deck ridge's height at `x`.
fn top_at(stations: &[Station], x: f32) -> f32 {
    let i = stations
        .windows(2)
        .position(|w| x >= w[0].x && x <= w[1].x)
        .unwrap_or(0);
    let (a, c) = (stations[i], stations[i + 1]);
    lerp(a.top, c.top, ((x - a.x) / (c.x - a.x)).clamp(0.0, 1.0))
}

/// A flat blade of `outline` (points in one plane), `half` thick in the middle along
/// `normal`, its faces drawn in toward the middle by `k` so its edges come to a blade.
/// Far off, a plain slab.
fn fin(b: &mut MeshBuilder, outline: &[Vec3], normal: Vec3, half: f32, k: f32) {
    let c = outline.iter().copied().sum::<Vec3>() / outline.len() as f32;
    let n = normal.normalize() * half;
    let face = |s: f32, k: f32| -> Vec<Vec3> {
        outline.iter().map(|&p| c + (p - c) * k + n * s).collect()
    };
    if b.coarse() {
        // Far off only what is seen from above is left: a flat plane's top face.
        if normal.normalize().z.abs() > 0.9 {
            flat(b, outline);
        }
    } else {
        b.loft(&[face(-1.0, k), outline.to_vec(), face(1.0, k)], true, true);
    }
}

/// A flat face over `outline`, turned up.
fn flat(b: &mut MeshBuilder, outline: &[Vec3]) {
    let mut p = outline.to_vec();
    let n = p
        .windows(3)
        .fold(Vec3::ZERO, |n, w| n + (w[1] - w[0]).cross(w[2] - w[1]));
    if n.z < 0.0 {
        p.reverse();
    }
    b.face(&p);
}

/// A plan outline `(x, y)` laid flat at `z`.
fn plan(points: &[[f32; 2]], z: f32) -> Vec<Vec3> {
    points.iter().map(|&[x, y]| v3(x, y, z)).collect()
}

/// A plan outline: `half` of it (y >= 0) from the front round to the back, mirrored.
fn mirrored(half: &[[f32; 2]]) -> Vec<[f32; 2]> {
    half.iter()
        .copied()
        .chain(
            half.iter()
                .rev()
                .filter(|p| p[1] > 0.0)
                .map(|&[x, y]| [x, -y]),
        )
        .collect()
}

/// A box with its corners cut.
const CHAMFERED: [[f32; 2]; 8] = [
    [1.0, -0.6],
    [0.6, -1.0],
    [-0.6, -1.0],
    [-1.0, -0.6],
    [-1.0, 0.6],
    [-0.6, 1.0],
    [0.6, 1.0],
    [1.0, 0.6],
];

/// A body lofted along x through `stations` (x, width, height, centre y, centre z) of
/// `shape`.
fn body_x(b: &mut MeshBuilder, stations: &[[f32; 5]], shape: &[[f32; 2]]) {
    let rings: Vec<Vec<Vec3>> = stations
        .iter()
        .map(|&[x, w, h, y, z]| {
            shape
                .iter()
                .map(|[s, u]| v3(x, y + s * w * 0.5, z + u * h * 0.5))
                .collect()
        })
        .collect();
    b.loft(&rings, true, true);
}

/// One swept plate of `len` from `o` back along `u`, face out along `n`, its spike at
/// `tip` across it.
fn spike_plate(b: &mut MeshBuilder, o: Vec3, u: Vec3, n: Vec3, len: f32, half: f32, tip: f32) {
    dark_plate(b);
    armour(b, &Frame::new(o, u, n), &swept(len, half, tip, 0.4), 0.16);
}

/// The team's mark: a swept plate pointing forward from `rear` on the deck at `z`.
/// Far off, a flat arrowhead.
fn team_mark(b: &mut MeshBuilder, rear: Vec3, len: f32, half: f32) {
    b.paint(TEAM);
    let f = Frame::new(rear, Vec3::X, Vec3::Z);
    if b.coarse() {
        b.face(&[
            f.at(len, 0.0, 0.05),
            f.at(0.0, half, 0.05),
            f.at(0.0, -half, 0.05),
        ]);
        return;
    }
    armour(b, &f, &swept(len, half, 0.0, 0.35), 0.08);
}

// ---- the weapons ------------------------------------------------------------------------

/// A bow tube's mouth at `m`, facing forward: a dark bore, the red of the plasma its
/// gravity holds, a dark bronze rim proud of both.
fn tube_mouth(b: &mut MeshBuilder, m: Vec3, r: f32) {
    let out = Vec3::X;
    let sides = b.sides(10);
    seam(b);
    b.cylinder_between(m - out * 0.25, m + out * 0.02, r, r, sides);
    b.paint(GLOW_LASER);
    b.cylinder_between(m - out * 0.02, m + out * 0.05, r * 0.66, r * 0.5, sides);
    if b.fine() {
        metal(b);
        hoop_on(b, m + out * 0.04, out, r + 0.04, 0.12, 0.16, 10);
    }
}

/// All six bow tubes. Far off, a red triangle over each side's three.
fn bow_tubes(b: &mut MeshBuilder) {
    if b.coarse() {
        b.paint(GLOW_LASER);
        b.mirror_y(|b| {
            let p = |i: usize, dy: f32, dz: f32| Vec3::from(TUBES[i]) + v3(0.04, dy, dz);
            b.face(&[p(1, -0.1, 0.15), p(3, -0.1, -0.15), p(5, 0.15, 0.0)]);
        });
        return;
    }
    for m in TUBES {
        tube_mouth(b, Vec3::from(m), 0.3);
    }
}

/// The interceptor doors in a stern face at `face`: a dark ring sunk round each, a
/// bronze door.
fn interceptor_doors(b: &mut MeshBuilder, face: f32, r: f32) {
    if b.coarse() {
        return;
    }
    let sides = b.sides(8);
    for [x, y, z] in INTERCEPT {
        seam(b);
        b.cylinder_between(v3(face + 0.2, y, z), v3(x - 0.04, y, z), r, r, sides);
        metal(b);
        b.cylinder_between(
            v3(x - 0.06, y, z),
            v3(x - 0.12, y, z),
            r * 0.75,
            r * 0.7,
            sides,
        );
    }
}

/// A gravity drive at `at`: a dark bronze ring `r` round a red core, held off the hull
/// on bronze spokes, facing aft.
fn drive(b: &mut MeshBuilder, at: Vec3, r: f32, len: f32) {
    if b.coarse() {
        return;
    }
    let x = Vec3::X;
    b.paint(GLOW_LASER);
    b.cylinder_between(
        at + x * (len * 0.6),
        at - x * (len * 0.4),
        r * 0.42,
        r * 0.18,
        b.sides(8),
    );
    metal(b);
    hoop_on(b, at, x, r, 0.26, len, if b.fine() { 14 } else { 6 });
    if b.fine() {
        for (y, z) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
            let d = v3(0.0, y, z);
            metal(b);
            b.cylinder_between(
                at + x * (len * 0.6) + d * (r * 0.38),
                at + d * (r - 0.1),
                0.09,
                0.09,
                6,
            );
        }
    }
}

/// A twin plasmeric repeater house for `weapon` at `p`, lifted on a bronze column out of
/// a plated collar in the deck at `deck`: a low plated cupola drawn to a point, cheek plates swept back into spikes, a
/// red sight forward, and the twin bores in a clamped breech that pitches.
fn repeater(b: &mut MeshBuilder, weapon: usize, p: Vec3, deck: f32) {
    let base = p.z - 0.55;
    if !b.coarse() {
        let sides = b.sides(12);
        dark_plate(b);
        b.prism(v3(p.x, 0.0, deck - 0.7), sides, 2.1, 1.9, 0.95);
        metal(b);
        b.prism(v3(p.x, 0.0, deck), sides, 1.2, 1.2, base - deck);
        if b.fine() {
            hoop(b, v3(p.x, 0.0, deck + 0.3), 1.25, 0.2, 0.12, sides);
        }
    }
    b.with_house(weapon, p, 0.3, |b| {
        if b.coarse() {
            return;
        }
        let cupola = mirrored(&[
            [2.1, 0.0],
            [1.1, 1.1],
            [-1.0, 1.35],
            [-2.3, 1.05],
            [-1.9, 0.0],
        ]);
        dark_plate(b);
        b.at(v3(p.x, 0.0, 0.0), |b| {
            b.loft_z(
                &cupola,
                &[
                    Section::new(base, 1.0),
                    Section::new(base + 0.3, 1.0),
                    Section::scaled(p.z + 0.62, 0.72, 0.66).shifted(-0.45, 0.0),
                ],
            );
        });
        b.mirror_y(|b| {
            let f = Frame::new(
                v3(p.x + 1.3, 1.2, base + 0.12),
                v3(-1.0, 0.1, 0.06),
                v3(0.0, 1.0, 0.5),
            );
            dark_plate(b);
            armour(b, &f, &swept(4.1, 0.34, 0.5, 0.5), 0.14);
        });
        if b.fine() {
            red_slot(
                b,
                v3(p.x + 0.9, 0.0, p.z + 0.45),
                v3(1.0, 0.0, 1.2),
                Vec3::Y,
                0.9,
                0.08,
            );
        }
        b.with_recoil(|b| twin_bores(b, p));
    });
}

/// The repeater's breech and twin bores in the house's frame, level.
fn twin_bores(b: &mut MeshBuilder, p: Vec3) {
    let z = p.z + BORE_RISE;
    collar(b, p, Vec3::Y, 0.35, 1.5);
    dark_plate(b);
    body_x(
        b,
        &[
            [p.x - 0.5, 1.25, 0.62, 0.0, z],
            [p.x + 1.2, 1.15, 0.55, 0.0, z],
        ],
        &CHAMFERED,
    );
    let sides = b.sides(8);
    for y in [-BORE_Y, BORE_Y] {
        dark_plate(b);
        b.cylinder_between(v3(p.x + 1.1, y, z), v3(p.x + BORE, y, z), 0.15, 0.13, sides);
        if b.fine() {
            metal(b);
            b.cylinder_between(v3(p.x + 1.9, y, z), v3(p.x + 2.3, y, z), 0.18, 0.18, sides);
            b.paint(GLOW_LASER);
            hoop_on(
                b,
                v3(p.x + BORE - 0.02, y, z),
                Vec3::X,
                0.1,
                0.05,
                0.05,
                sides,
            );
        }
    }
}

/// The AA house on the sail's back: a hex step on a bronze race, a plated cheek either
/// side swept back into a spike, and two short AA tubes in a clamped breech held up at
/// the sky, red rims at their mouths.
fn aa_house(b: &mut MeshBuilder) {
    let base = SAIL_TOP - 0.05;
    if b.fine() {
        metal(b);
        hoop(b, v3(AA.x, 0.0, base), 1.2, 0.3, 0.12, 12);
    }
    let d = AA_MUZZLE - AA;
    let len = d.length();
    b.with_house(4, AA, 0.2, |b| {
        if b.coarse() {
            return;
        }
        dark_plate(b);
        b.prism(v3(AA.x, 0.0, base), 6, 1.25, 1.05, 0.3);
        b.mirror_y(|b| {
            dark_plate(b);
            b.block(
                v3(AA.x - 0.8, 0.72, base + 0.25),
                v3(AA.x + 0.6, 0.98, AA.z + 0.45),
            );
            let f = Frame::new(
                v3(AA.x + 0.6, 1.0, AA.z + 0.15),
                v3(-1.0, 0.0, 0.3),
                Vec3::Y,
            );
            armour(b, &f, &swept(2.5, 0.45, 0.3, 0.45), 0.14);
        });
        b.with_recoil(|b| {
            b.at(AA, |b| organ(b, len));
        });
    });
}

/// The AA organ in its own frame (the trunnion at the origin, +x up the bore).
fn organ(b: &mut MeshBuilder, len: f32) {
    collar(b, Vec3::ZERO, Vec3::Y, 0.3, 1.4);
    dark_plate(b);
    body_x(
        b,
        &[[-0.55, 1.25, 0.66, 0.0, 0.0], [0.35, 1.3, 0.7, 0.0, 0.0]],
        &CHAMFERED,
    );
    let sides = b.sides(8);
    for y in [-0.3f32, 0.3] {
        dark_plate(b);
        b.cylinder_between(v3(0.3, y, 0.0), v3(len - 0.3, y, 0.0), 0.22, 0.2, sides);
        if b.fine() {
            metal(b);
            b.cylinder_between(v3(0.9, y, 0.0), v3(1.15, y, 0.0), 0.25, 0.25, sides);
        }
        dark_plate(b);
        b.cylinder_between(v3(len - 0.32, y, 0.0), v3(len, y, 0.0), 0.21, 0.24, sides);
        if b.fine() {
            mouth_rim(b, v3(len, y, 0.0), 0.24, sides);
        }
    }
}

/// The seeker hatches in the sail's top: a dark sunk frame round each, a plated door
/// flush with the top in two leaves on bronze hinges, and a red line down between the rows.
fn seeker_hatches(b: &mut MeshBuilder) {
    if b.coarse() {
        return;
    }
    for [x, y, z] in SEEKERS {
        seam(b);
        b.block(
            v3(x - 0.58, y - 0.44, z - 0.12),
            v3(x + 0.58, y + 0.44, z - 0.04),
        );
        dark_plate(b);
        // Two leaves meeting across the middle, each on a bronze hinge at its end.
        for k in [-1.0f32, 1.0] {
            let (x0, x1) = (x + k * 0.03, x + k * 0.5);
            b.block(
                v3(x0.min(x1), y - 0.37, z - 0.08),
                v3(x0.max(x1), y + 0.37, z),
            );
        }
        if b.fine() {
            for k in [-1.0f32, 1.0] {
                collar(b, v3(x + k * 0.52, y, z - 0.04), Vec3::Y, 0.07, 0.66);
            }
        }
    }
    if b.fine() {
        red_slot(
            b,
            v3(2.5, 0.0, SAIL_TOP - 0.02),
            Vec3::Z,
            Vec3::X,
            4.2,
            0.08,
        );
    }
}

/// The sail: `outline` (a plan, `mirrored`) lofted from inside the hull at `foot` up to
/// the flat top, each section `(z, scale x, scale y, shift x)`; the hatches, the AA repeater
/// house and a pair of red optics forward under the top.
fn sail(b: &mut MeshBuilder, outline: &[[f32; 2]], sections: &[[f32; 4]], optic: Vec3) {
    let plan = mirrored(outline);
    dark_plate(b);
    let s: Vec<Section> = sections
        .iter()
        .map(|&[z, sx, sy, dx]| Section::scaled(z, sx, sy).shifted(dx, 0.0))
        .collect();
    if b.coarse() {
        let nose = outline[0];
        let wide = outline
            .iter()
            .fold([0.0f32, 0.0], |w, p| if p[1] > w[1] { *p } else { w });
        let tail = outline[outline.len() - 1];
        let diamond = [nose, wide, tail, [wide[0], -wide[1]]];
        b.loft_z(&diamond, &[s[0], s[s.len() - 1]]);
    } else {
        b.loft_z(&plan, &s);
    }
    aa_house(b);
    seeker_hatches(b);
    if b.fine() {
        b.mirror_y(|b| red_slot(b, optic, Vec3::Y, v3(1.0, 0.0, -0.4), 1.1, 0.12));
    }
}

// ---- the boat ---------------------------------------------------------------------------

/// The hull: slim and long, ending between its two tail booms.
const HULL: [Station; 9] = [
    st(-20.0, 0.6, 0.0, -1.0, -1.8),
    st(-16.0, 1.8, 1.0, -0.9, -2.9),
    st(-11.0, 2.7, 1.7, -0.85, -3.6),
    st(-3.0, 3.2, 2.1, -0.8, -4.2),
    st(6.0, 3.2, 2.15, -0.8, -4.2),
    st(14.0, 2.7, 1.9, -0.8, -3.8),
    st(20.0, 2.1, 1.4, -0.7, -3.1),
    st(24.6, 1.3, 0.8, -0.55, -2.3),
    tip(28.6, -0.4),
];

/// The sail: a long blade-backed wedge.
const SAIL: [[f32; 2]; 7] = [
    [8.5, 0.0],
    [6.0, 1.3],
    [4.5, 1.65],
    [0.0, 1.7],
    [-4.0, 1.5],
    [-7.0, 0.7],
    [-8.5, 0.0],
];

/// Where the tail booms run, their middles across and up.
const BOOM_Y: f32 = 2.45;
const BOOM_Z: f32 = -0.95;

/// The boat: a slim hull with big swept planes at its shoulders, so from above it
/// is a dart, and two tail booms that fork the stern, each with its drive and an
/// interceptor door; fangs of tubes either side of the chin; guns lifted on bronze columns.
fn swallowtail(b: &mut MeshBuilder, _tech: u8) {
    let h = &HULL;
    hull(b, h, &[0, 4, 8]);
    sail(
        b,
        &SAIL,
        &[
            [1.6, 1.0, 1.0, 0.0],
            [3.2, 0.96, 0.95, -0.5],
            [SAIL_TOP, 0.9, 0.92, -0.9],
        ],
        v3(5.2, 1.3, 3.7),
    );
    // The shoulder planes: big swept blades just over the water, raked into points.
    dark_plate(b);
    b.mirror_y(|b| {
        fin(
            b,
            &plan(&[[9.0, 2.4], [-4.5, 10.2], [-8.6, 10.8], [-7.8, 2.4]], 1.35),
            Vec3::Z,
            0.18,
            0.88,
        );
    });
    // The tail booms, from the planes' roots back past the stern.
    b.mirror_y(|b| {
        dark_plate(b);
        let shape: Vec<[f32; 2]> = if b.coarse() {
            vec![[1.0, 0.0], [0.0, -1.0], [-1.0, 0.0], [0.0, 1.0]]
        } else {
            CHAMFERED.to_vec()
        };
        let root = [-6.0, 0.6, 0.8, BOOM_Y - 0.3, BOOM_Z + 0.4];
        let (fore, aft) = (
            [-11.0, 2.0, 3.0, BOOM_Y, BOOM_Z],
            [-25.0, 2.0, 3.1, BOOM_Y, BOOM_Z],
        );
        if b.coarse() {
            body_x(b, &[fore, aft], &shape);
        } else {
            body_x(b, &[root, fore, aft], &shape);
        }
        drive(b, v3(-25.5, BOOM_Y + 0.25, BOOM_Z + 0.55), 0.75, 0.45);
        // A blade up from each boom, canted out.
        dark_plate(b);
        fin(
            b,
            &[
                v3(-17.0, BOOM_Y + 0.1, 0.5),
                v3(-22.5, BOOM_Y + 0.9, 2.8),
                v3(-25.2, BOOM_Y + 1.0, 2.9),
                v3(-24.6, BOOM_Y, 0.5),
            ],
            v3(0.0, 1.0, -0.35),
            0.14,
            0.85,
        );
    });
    fangs(b);
    bow_tubes(b);
    repeater(b, 2, FORE, top_at(h, FORE.x));
    repeater(b, 3, AFT, top_at(h, AFT.x));
    team_mark(b, v3(15.0, 0.0, top_at(h, 15.0) - 0.02), 3.0, 0.9);
    if b.coarse() {
        return;
    }
    interceptor_doors(b, -25.0, 0.25);
    if !b.fine() {
        return;
    }
    b.mirror_y(|b| {
        // A bronze trench along each plane's root, a plate lapped over it, and a red line
        // along the plane's leading edge.
        metal(b);
        b.beam(
            v3(7.0, 2.6, 1.4),
            v3(-7.0, 2.6, 1.4),
            Vec2::new(0.5, 0.45),
            Vec2::new(0.5, 0.45),
        );
        spike_plate(
            b,
            v3(7.5, 2.7, 1.6),
            v3(-1.0, 0.55, -0.02),
            Vec3::Z,
            9.0,
            0.8,
            0.6,
        );
        red_slot(
            b,
            v3(2.3, 6.2, 1.55),
            Vec3::Z,
            v3(-13.5, 7.6, 0.0),
            6.0,
            0.07,
        );
        dark_plate(b);
        Course {
            count: 3,
            step: 4.0,
            len: 4.6,
            half: 0.7,
            tip: -0.6,
            thick: 0.16,
            tail: 1.0,
        }
        .lay(
            b,
            &Frame::new(v3(23.0, 1.5, 0.75), Vec3::NEG_X, v3(0.0, 0.7, 0.7)),
        );
    });
    // Down the back between the booms: lapped plates over a bronze spine.
    metal(b);
    b.cylinder_between(v3(-6.0, 0.0, 2.05), v3(-19.0, 0.0, 0.3), 0.28, 0.28, 8);
    dark_plate(b);
    Course {
        count: 2,
        step: 2.6,
        len: 3.0,
        half: 1.2,
        tip: 0.0,
        thick: 0.18,
        tail: 1.8,
    }
    .lay(
        b,
        &Frame::new(v3(-14.6, 0.0, 1.45), v3(-1.0, 0.0, -0.2), Vec3::Z),
    );
}

/// The fangs: two deep narrow pods either side of the chin, their faces
/// raked back from a point at the top, three tubes in each.
fn fangs(b: &mut MeshBuilder) {
    if b.coarse() {
        return;
    }
    b.mirror_y(|b| {
        dark_plate(b);
        let shape: Vec<[f32; 2]> = if b.coarse() {
            vec![[1.0, 0.0], [0.0, -1.0], [-1.0, 0.0], [0.0, 1.0]]
        } else {
            vec![
                [1.0, -0.4],
                [0.5, -1.0],
                [-0.6, -1.0],
                [-1.0, -0.3],
                [-0.8, 0.8],
                [0.0, 1.0],
                [0.8, 0.8],
            ]
        };
        body_x(
            b,
            &[
                [12.0, 0.4, 0.6, 1.4, -1.0],
                [19.0, 2.1, 2.5, 1.5, -1.5],
                [BOW_FACE, 2.3, 2.6, 1.5, -1.5],
            ],
            &shape,
        );
        if b.mid() {
            spike_plate(
                b,
                v3(25.2, 1.5, -0.15),
                v3(-1.0, 0.0, 0.0),
                v3(0.0, 0.5, 1.0),
                9.0,
                0.85,
                0.7,
            );
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{build_model, part, rig, Model};

    const KEY: &str = "regency_assault_submarine";

    fn house_verts(model: &Model, lod: usize, weapon: u8) -> Vec<Vec3> {
        let slot = model
            .houses
            .iter()
            .position(|h| h.weapon == weapon)
            .expect("a house for the weapon") as u32;
        model.lods[lod]
            .vertices
            .iter()
            .filter(|v| v.rig & rig::LIMB_MASK == rig::HOUSE_FIRST + slot)
            .map(|v| Vec3::from(v.pos))
            .collect()
    }

    fn near(points: &[Vec3], m: Vec3) -> f32 {
        points
            .iter()
            .map(|p| p.distance(m))
            .fold(f32::MAX, f32::min)
    }

    fn bores(p: Vec3) -> [Vec3; 2] {
        [-BORE_Y, BORE_Y].map(|y| p + v3(BORE, y, BORE_RISE))
    }

    /// The boat floats and fits its blueprint, keeps to its budget, wears dark plate
    /// and the owner's colour, keeps its houses on the unit file's pivots, and holds every
    /// weapon where the unit file fires it.
    #[test]
    fn it_fits_and_holds_its_weapons() {
        let key = KEY;
        let model = build_model(key).expect(key);
        let houses: Vec<(u8, Vec3)> = model
            .houses
            .iter()
            .map(|h| (h.weapon, Vec3::from(h.pivot)))
            .collect();
        assert_eq!(houses, vec![(4, AA), (2, FORE), (3, AFT)], "{key}: houses");
        let tris = |lod: usize| model.lods[lod].indices.len() / 3;
        let (full, mid, coarse) = (tris(0), tris(1), tris(2));
        let budget = super::super::super::triangles(key).unwrap();
        assert!(full <= budget, "{key}: {full} (budget {budget})");
        assert!(
            mid as f32 <= full as f32 * 0.45 + 20.0 && coarse < 60,
            "{key}: {full}/{mid}/{coarse}"
        );
        for (lod, mesh) in model.lods.iter().enumerate() {
            let name = format!("{key} lod{lod}");
            let (lo, hi) = mesh
                .vertices
                .iter()
                .fold((f32::MAX, f32::MIN), |(lo, hi), v| {
                    (lo.min(v.pos[2]), hi.max(v.pos[2]))
                });
            assert!(
                (HEIGHT * 0.8..=HEIGHT * 1.25).contains(&hi),
                "{name}: top {hi}"
            );
            assert!((-4.5..-2.0).contains(&lo), "{name}: keel {lo}");
            let reach = mesh
                .vertices
                .iter()
                .map(|v| v.pos[0].hypot(v.pos[1]))
                .fold(0.0, f32::max);
            assert!(
                (RADIUS * 0.75..=RADIUS * 1.3).contains(&reach),
                "{name}: reach {reach}"
            );
            assert!(
                mesh.vertices
                    .iter()
                    .any(|v| v.material == TEAM && v.normal[2] > 0.5),
                "{name}: no upward team colour"
            );
            assert!(mesh.vertices.iter().any(|v| v.material == PLATING_DARK));
            assert!(
                !mesh.vertices.iter().any(|v| v.material == GLOW
                    || v.material == GLOW_ORANGE
                    || v.part == part::LOCOMOTION),
                "{name}: ARC's light or running gear"
            );
            let red: Vec<Vec3> = mesh
                .vertices
                .iter()
                .filter(|v| v.material == GLOW_LASER && v.rig & rig::LIMB_MASK == 0)
                .map(|v| Vec3::from(v.pos))
                .collect();
            for m in TUBES {
                let m = Vec3::from(m);
                let d = near(&red, m);
                assert!(d < 0.5, "{name}: no red mouth within {d} m of {m}");
            }
            if lod == 2 {
                continue;
            }
            let hull: Vec<Vec3> = mesh
                .vertices
                .iter()
                .filter(|v| v.rig & rig::LIMB_MASK == 0)
                .map(|v| Vec3::from(v.pos))
                .collect();
            for m in INTERCEPT.iter().chain(SEEKERS.iter()) {
                let m = Vec3::from(*m);
                let d = near(&hull, m);
                assert!(d < 0.4, "{name}: nothing within {d} m of {m}");
            }
            for (weapon, p) in [(2, FORE), (3, AFT)] {
                let gun = house_verts(&model, lod, weapon);
                for m in bores(p) {
                    let d = near(&gun, m);
                    assert!(d < 0.4, "{name}: bores {d} m from {m}");
                }
                let past = gun.iter().map(|q| q.x).fold(f32::MIN, f32::max);
                assert!(past <= p.x + BORE + 0.1, "{name}: house reaches {past}");
            }
            let d = near(&house_verts(&model, lod, 4), AA_MUZZLE);
            assert!(d < 0.4, "{name}: AA tubes {d} m from the muzzle");
        }
    }

    /// The unit file's size, pivots and muzzles are the model's.
    #[test]
    fn the_unit_files_weapons_are_the_models() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let blueprints = mc_data::Blueprints::load(&dir).unwrap();
        let bp = blueprints.unit(blueprints.id_of("regency_t3_submarine").unwrap());
        assert_eq!(bp.visual.mesh, "regency_assault_submarine");
        assert!((bp.radius.to_f32() - RADIUS).abs() < 1e-3);
        assert!((bp.height.to_f32() - HEIGHT).abs() < 1e-3);
        let v = |p: mc_core::FxVec3| Vec3::new(p.x.to_f32(), p.y.to_f32(), p.z.to_f32());
        let close = |a: Vec3, b: Vec3| a.distance(b) < 0.02;
        let w = &bp.weapons;
        let all = |i: usize, want: &[Vec3]| {
            let got: Vec<Vec3> = w[i].muzzles.iter().map(|&p| v(p)).collect();
            assert_eq!(got.len(), want.len(), "weapon {i}");
            for (a, b) in got.iter().zip(want) {
                assert!(close(*a, *b), "weapon {i}: unit file {a}, model {b}");
            }
        };
        all(0, &TUBES.map(Vec3::from));
        all(1, &INTERCEPT.map(Vec3::from));
        all(2, &bores(FORE));
        all(3, &bores(AFT));
        all(5, &SEEKERS.map(Vec3::from));
        assert!(close(v(w[2].pivot.unwrap()), FORE));
        assert!(close(v(w[3].pivot.unwrap()), AFT));
        assert!(close(v(w[4].pivot.unwrap()), AA));
        assert!(close(v(w[4].muzzle), AA_MUZZLE));
    }
}
