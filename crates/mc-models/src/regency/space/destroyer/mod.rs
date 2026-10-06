//! Scourge: the Regency's tech 4 space destroyer (`regency_t4_space_destroyer`, mesh
//! `regency_space_destroyer`). One long faceted arrowhead of hard chines and flat
//! armour, widest aft of the middle and run forward to a chisel bow, two great blades
//! swept back out of its chines past the stern, a terraced citadel aft. Two tones of
//! plate are laid over it in courses that sweep back into spikes: charcoal over the
//! deck and flanks, a pale stripe course down each shoulder. Through the ship's middle
//! stands the drum of the energy core the Heavy Pinch-fusion Lance is fired from
//! (`core.rs`); six Pinch-plasma Batteries turn in ball mounts on sponsons swelling
//! out of the chines (`battery.rs`), out past the flanks so they fire down clear of
//! the hull, and two blocks of hatched seeker cells on the stern deck (`cells.rs`).
//! +X is forward, +Y port, the ground at z 0 (it is built on its lot, the core lowest).
//! Authored in metres as built.
//!
//! Contracts (tests.rs): the core's lens is `space.ron` weapon 0's `muzzle`, the
//! batteries' pivots and fork gaps are weapons 1 to 6's, and the cells are weapon 7's
//! `muzzles`.

mod battery;
mod body;
mod cells;
mod core;

use glam::Vec3;

use crate::builder::{MeshBuilder, Section};
use crate::material::{GLOW_LASER, GLOW_VIOLET, PLATING, TEAM};
use crate::pattern;

use super::super::kit::{dark_plate, metal, seam, v3};
use super::super::machine::hoop_on;
use battery::{battery, BALL_R};
use body::{feather, st, Body, BILGE, CHINE, DECK_EDGE, KEEL_EDGE, SPINE};
use cells::{blocks, Cells};
use core::{core, Core};

const HULL: Body = Body {
    stations: &[
        st(-310.0, 58.0, 20.0, 50.0, 68.0, 30.0, 96.0, 100.0),
        st(-262.0, 100.0, 26.0, 40.0, 64.0, 40.0, 108.0, 116.0),
        st(-160.0, 114.0, 28.0, 36.0, 62.0, 44.0, 112.0, 122.0),
        st(-40.0, 98.0, 24.0, 34.0, 62.0, 38.0, 106.0, 114.0),
        st(80.0, 78.0, 20.0, 38.0, 62.0, 30.0, 98.0, 104.0),
        st(200.0, 54.0, 16.0, 44.0, 63.0, 22.0, 88.0, 92.0),
        st(300.0, 30.0, 10.0, 52.0, 64.0, 12.0, 78.0, 80.0),
        st(362.0, 8.0, 3.0, 60.0, 65.0, 3.0, 70.0, 70.0),
    ],
    y: 0.0,
};

/// The core stands at the ship's middle, so the beam leaves the same point whichever
/// way it is laid.
const CORE: Core = Core {
    x: 0.0,
    low: 20.0,
    high: 118.0,
};

const CELLS: Cells = Cells { x: -280.0, y: 22.0 };

/// The drives' mouths and radii: a great drive on the centreline over two lesser.
const DRIVES: [([f32; 3], f32); 3] = [
    ([-310.0, 0.0, 80.0], 14.0),
    ([-310.0, -32.0, 64.0], 8.0),
    ([-310.0, 32.0, 64.0], 8.0),
];

/// The stern drives' mouths, for the drive effects.
pub(crate) const NOZZLES: [[f32; 3]; 3] = [DRIVES[0].0, DRIVES[1].0, DRIVES[2].0];

/// The drives' size as the drive effects take it (1: a 12 m mouth): the great drive's.
pub(crate) const DRIVE_SIZE: f32 = DRIVES[0].1 / 12.0;

/// The batteries' sponsons down each chine, fore to aft (port of each pair first),
/// all ahead of the great blades' roots.
const MOUNTS: [f32; 3] = [230.0, 130.0, 30.0];
/// The chine's fairing: its run (head, tail), and at a sponson how far it stands out of
/// the chine, how high its deck stands over it and how deep its belly hangs under it.
const FAIRING: (f32, f32) = (290.0, -20.0);
const SPONSON_OUT: f32 = 30.0;
const SPONSON_UP: f32 = 12.0;
const SPONSON_DOWN: f32 = 13.0;
/// How far out of the chine the ball's socket is cut, and how far over the socket's
/// rim the ball's middle stands.
const SOCKET_OUT: f32 = 0.6 * SPONSON_OUT;
const BALL_SEAT: f32 = 0.35 * BALL_R;

/// The socket's rim on the port sponson at `x`.
fn socket(x: f32) -> Vec3 {
    HULL.point(x, CHINE) + v3(0.0, SOCKET_OUT, SPONSON_UP)
}

/// Each battery's pivot in weapon order (weapons 1 to 6): the ball's middle, seated in
/// its socket.
fn battery_pivots() -> [Vec3; 6] {
    std::array::from_fn(|k| {
        let side = if k % 2 == 0 { 1.0 } else { -1.0 };
        let p = socket(MOUNTS[k / 2]) + Vec3::Z * BALL_SEAT;
        v3(p.x, p.y * side, p.z)
    })
}

/// Where the lance leaves the core: the lower lens's face (`space.ron` weapon 0's
/// `muzzle`).
#[cfg(test)]
pub(crate) fn lance_muzzle() -> Vec3 {
    CORE.lens()
}

/// Each battery's pivot and its forks' gaps, port fork first (weapons 1 to 6).
#[cfg(test)]
pub(crate) fn battery_mounts() -> Vec<(Vec3, [Vec3; 2])> {
    battery_pivots()
        .into_iter()
        .map(|p| (p, battery::muzzles(p)))
        .collect()
}

/// Where the seekers leave from, port block then starboard, in firing order (weapon 7's
/// `muzzles`).
#[cfg(test)]
pub(crate) fn seeker_muzzles() -> Vec<Vec3> {
    CELLS.muzzles(&HULL)
}

pub(super) fn destroyer(b: &mut MeshBuilder, _tech: u8) {
    let body = &HULL;
    if b.coarse() {
        coarse(b);
        return;
    }
    body.hull(b);
    b.mirror_y(|b| plates(b, body));
    trench(b, body, (330.0, -296.0), 2.6);
    b.mirror_y(|b| {
        pale_plate(b);
        blade(b, body);
        dark_plate(b);
        strake(b, body, (340.0, 120.0), |t| 3.0 + 8.0 * t, (2.6, 1.0), 2.0);
        optics(b, body, 320.0, -220.0);
    });
    citadel(b, body, -196.0, 110.0, 34.0, &[12.0, 10.0, 8.0, 6.0]);
    drives(b, &DRIVES);
    lifts(
        b,
        body,
        &[
            (140.0, 34.0),
            (140.0, -34.0),
            (-160.0, 50.0),
            (-160.0, -50.0),
        ],
    );
    core(b, body, CORE);
    blocks(b, body, CELLS);
    b.mirror_y(|b| fairing(b, body));
    for (k, pivot) in battery_pivots().into_iter().enumerate() {
        battery(b, 1 + k, pivot);
    }
    mark(b, v3(120.0, 0.0, 104.5), 46.0);
}

/// The great blade out of the port chine: rooted from amidships back to the stern,
/// reaching far out and swept into a spike past the stern.
fn blade(b: &mut MeshBuilder, body: &Body) {
    strake(
        b,
        body,
        (20.0, -394.0),
        |t| 8.0 + 92.0 * (t * 1.6).min(1.0) - 40.0 * ((t - 0.75).max(0.0) * 4.0).powi(2),
        (6.0, 1.4),
        6.0,
    );
}

/// How much the chine's fairing swells at `x`: 1 at a sponson, easing to 0 between.
fn swell(x: f32) -> f32 {
    let s = MOUNTS
        .iter()
        .map(|&m| (1.0 - (x - m).abs() / 44.0).clamp(0.0, 1.0))
        .fold(0.0, f32::max);
    s * s * (3.0 - 2.0 * s)
}

/// The port chine's fairing: one faceted strake grown out of the hull along the chine,
/// its deck rising off the shoulder and its belly off the bilge, low between the
/// batteries and swelling round each into a sponson with the ball's socket cut in its
/// deck, a pale collar round the ball.
fn fairing(b: &mut MeshBuilder, body: &Body) {
    let (head, tail) = FAIRING;
    let n = if b.fine() { 30 } else { 12 };
    let rings: Vec<Vec<Vec3>> = (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let x = head + (tail - head) * t;
            let c = body.point(x, CHINE);
            // Pointed at both ends, run back into the chine.
            let end = (t * 7.0).min((1.0 - t) * 7.0).min(1.0);
            let s = swell(x) * end;
            let out = 4.0 * end + (SPONSON_OUT - 4.0) * s;
            let up = 2.0 * end + (SPONSON_UP - 2.0) * s;
            let down = 3.0 * end + (SPONSON_DOWN - 3.0) * s;
            vec![
                body.out(x, CHINE + 0.3 + 0.35 * s, -0.6),
                c + v3(0.0, out * 0.72, up),
                c + v3(0.0, out, up * 0.35),
                c + v3(0.0, out * 0.7, -down),
                body.out(x, CHINE - 0.3 - 0.4 * s, -0.6),
            ]
        })
        .collect();
    dark_plate(b);
    b.with_facets(|b| b.loft(&rings, true, true));
    let sides = b.sides(12);
    for &x in &MOUNTS {
        let rim = socket(x);
        pale_plate(b);
        b.with_facets(|b| {
            b.cylinder_between(
                rim - Vec3::Z * 2.0,
                rim + Vec3::Z * 1.4,
                BALL_R * 1.14,
                BALL_R * 1.0,
                10,
            )
        });
        if b.fine() {
            metal(b);
            hoop_on(b, rim + Vec3::Z * 1.4, Vec3::Z, BALL_R, 0.4, 1.0, sides);
        }
    }
}

/// Far off: the hull's plan as one slab, the blades as flat triangles (drawn both
/// ways up) and the owner's mark.
fn coarse(b: &mut MeshBuilder) {
    let half = [[362.0, 0.0], [200.0, 54.0], [-160.0, 114.0], [-310.0, 58.0]];
    let mut outline: Vec<[f32; 2]> = half.to_vec();
    outline.extend(half[1..].iter().rev().map(|&[x, y]| [x, -y]));
    dark_plate(b);
    b.extrude_z(&outline, 40.0, 112.0);
    pale_plate(b);
    b.mirror_y(|b| {
        for tri in [
            [
                v3(20.0, 100.0, 64.0),
                v3(-242.0, 203.0, 62.0),
                v3(-160.0, 112.0, 63.0),
            ],
            [
                v3(-160.0, 112.0, 63.0),
                v3(-242.0, 203.0, 62.0),
                v3(-392.0, 116.0, 58.0),
            ],
        ] {
            b.face(&tri);
            b.face(&[tri[2], tri[1], tri[0]]);
        }
    });
    mark(b, v3(120.0, 0.0, 112.1), 50.0);
}

/// The plate courses, port side: charcoal everywhere but a pale stripe course down
/// the shoulder from bow to stern.
fn plates(b: &mut MeshBuilder, body: &Body) {
    dark_plate(b);
    if b.fine() {
        // The keel and the bilge, which the mid level leaves to the hull.
        course(
            b,
            body,
            (320.0, -290.0),
            5,
            (0.15, KEEL_EDGE + 0.35, KEEL_EDGE),
            1.8,
        );
        course(
            b,
            body,
            (300.0, -300.0),
            6,
            (BILGE - 0.3, CHINE + 0.25, CHINE + 0.1),
            1.6,
        );
    }
    course(
        b,
        body,
        (350.0, -296.0),
        8,
        (KEEL_EDGE + 0.2, CHINE - 0.1, CHINE - 0.4),
        2.2,
    );
    // The deck: charcoal plates either side of the trench.
    course(
        b,
        body,
        (356.0, -300.0),
        7,
        (DECK_EDGE - 0.2, SPINE - 0.15, DECK_EDGE + 0.3),
        2.4,
    );
    // The pale stripe down the shoulder.
    pale_plate(b);
    course(
        b,
        body,
        (358.0, -306.0),
        9,
        (CHINE + 0.08, DECK_EDGE + 0.1, CHINE + 0.3),
        2.8,
    );
    // A narrow charcoal course over the stripe's lower edge, its spikes over it.
    dark_plate(b);
    feather(
        b,
        body,
        (300.0, 40.0),
        (CHINE - 0.05, CHINE + 0.35, CHINE),
        3.0,
        2.0,
        0.6,
    );
    feather(
        b,
        body,
        (40.0, -260.0),
        (CHINE - 0.05, CHINE + 0.35, CHINE),
        3.0,
        2.0,
        0.6,
    );
}

/// The second plate: a paler steel under the same film (regency.wgsl `REG_FILM_PALE`),
/// laid in bands against the charcoal for a two-tone hull.
fn pale_plate(b: &mut MeshBuilder) {
    b.paint(PLATING).pattern(pattern::EMBER);
}

/// A course of `count` plates staggered from `from` (forward) back to `to`, each a
/// feather over `band` (`u` low, high and its spike's tip), `thick` deep.
fn course(
    b: &mut MeshBuilder,
    body: &Body,
    (from, to): (f32, f32),
    count: usize,
    band: (f32, f32, f32),
    thick: f32,
) {
    let step = (from - to) / count as f32;
    let over = step * 0.22;
    for k in 0..count {
        let head = from - step * k as f32;
        feather(
            b,
            body,
            (head, head - step - over),
            band,
            thick,
            thick * 0.5,
            0.42,
        );
    }
}

/// The spine's trench: the conduit from the head to the drives in a dark channel down
/// the deck, coupled every so often through a sleeve with a lit seam.
fn trench(b: &mut MeshBuilder, body: &Body, (from, to): (f32, f32), r: f32) {
    let n = if b.fine() { 12 } else { 6 };
    let path: Vec<Vec3> = (0..=n)
        .map(|i| body.out(from + (to - from) * i as f32 / n as f32, SPINE, r * 0.6))
        .collect();
    metal(b);
    let sides = b.sides(8);
    for w in path.windows(2) {
        b.cylinder_between(w[0], w[1], r, r, sides);
    }
    let couplings = if b.fine() { 7 } else { 3 };
    for i in 0..couplings {
        let x = from + (to - from) * (i as f32 + 0.5) / couplings as f32;
        let c = body.out(x, SPINE, r * 0.6);
        dark_plate(b);
        b.with_facets(|b| {
            b.cylinder_between(c + Vec3::X * 6.0, c - Vec3::X * 6.0, r * 1.7, r * 1.7, 6)
        });
        b.paint(GLOW_LASER);
        b.cylinder_between(c + Vec3::X * 0.5, c - Vec3::X * 0.5, r * 1.78, r * 1.78, 6);
    }
}

/// Red optics: slots let into the shoulder near the bow, largest forward, and a lit
/// seam run aft from them along the chine.
fn optics(b: &mut MeshBuilder, body: &Body, bow: f32, seam_to: f32) {
    b.paint(GLOW_LASER);
    for (k, len) in [(0.0, 12.0), (18.0, 8.0), (32.0, 5.5)] {
        let x = bow - k;
        body::skin(b, body, (x, x - len), |t| {
            let w = 0.22 * (1.0 - 0.4 * t);
            (CHINE + 0.35 - w, CHINE + 0.35 + w, -0.2, 0.35)
        });
    }
    if b.fine() {
        let n = 24;
        let path: Vec<Vec3> = (0..=n)
            .map(|i| {
                let x = bow - 40.0 + (seam_to - bow + 40.0) * i as f32 / n as f32;
                body.out(x, CHINE, 0.2)
            })
            .collect();
        for w in path.windows(2) {
            b.cylinder_between(w[0], w[1], 0.5, 0.5, 4);
        }
    }
}

/// A blade out of the hull's chine from `head` back to `tail`, reaching `span(t)` out
/// beyond the chine and drooping `droop` by its tail: a hard, flat-faceted lozenge.
fn strake(
    b: &mut MeshBuilder,
    body: &Body,
    (head, tail): (f32, f32),
    span: impl Fn(f32) -> f32,
    thick: (f32, f32),
    droop: f32,
) {
    let along = if b.fine() { 12 } else { 4 };
    let rings: Vec<Vec<Vec3>> = (0..=along)
        .map(|i| {
            let t = i as f32 / along as f32;
            let x = head + (tail - head) * t;
            let p = body.point(x.max(body.span().0 + 1.0), CHINE);
            let inner = p - Vec3::Y * 4.0;
            let outer = v3(x, p.y + span(t), p.z - droop * t);
            let th = thick.0 + (thick.1 - thick.0) * t;
            let mid = inner.lerp(outer, 0.4);
            vec![
                inner + Vec3::Z * th,
                mid + Vec3::Z * (th * 0.8),
                outer,
                mid - Vec3::Z * (th * 0.5),
                inner - Vec3::Z * th,
            ]
        })
        .collect();
    b.with_facets(|b| b.loft(&rings, true, true));
}

/// A stepped citadel on the deck at `x`: `tiers` of pointed hexagonal blocks, each set
/// back and narrower than the one under it, the sides charcoal, the roofs pale, a red
/// slit across each tier's prow.
fn citadel(b: &mut MeshBuilder, body: &Body, x: f32, len: f32, half: f32, tiers: &[f32]) {
    let deck = body.at(x).spine - 3.0;
    let plan = |l: f32, h: f32| -> Vec<[f32; 2]> {
        vec![
            [l * 0.5, 0.0],
            [l * 0.22, h],
            [-l * 0.42, h * 0.92],
            [-l * 0.5, h * 0.55],
            [-l * 0.5, -h * 0.55],
            [-l * 0.42, -h * 0.92],
            [l * 0.22, -h],
        ]
    };
    let mut z = deck;
    for (k, &tall) in tiers.iter().enumerate() {
        let f = 1.0 - 0.24 * k as f32;
        let (l, h) = (len * f, half * f);
        let back = -len * 0.08 * k as f32;
        let p = plan(l, h);
        b.at(v3(x + back, 0.0, 0.0), |b| {
            dark_plate(b);
            b.with_facets(|b| {
                b.loft_z(
                    &p,
                    &[
                        Section::new(z, 1.0),
                        Section::new(z + tall * 0.7, 1.0),
                        Section::new(z + tall, 0.9),
                    ],
                )
            });
            pale_plate(b);
            b.with_facets(|b| {
                b.loft_z(
                    &p,
                    &[
                        Section::new(z + tall - 0.3, 0.86),
                        Section::new(z + tall + 1.2, 0.78),
                    ],
                )
            });
            if !b.coarse() {
                b.paint(GLOW_LASER);
                let s = l * 0.5;
                b.with_facets(|b| {
                    b.loft_z(
                        &[
                            [s * 0.98, 0.0],
                            [s * 0.5, h * 0.9],
                            [s * 0.42, h * 0.9],
                            [s * 0.9, 0.0],
                            [s * 0.42, -h * 0.9],
                            [s * 0.5, -h * 0.9],
                        ],
                        &[
                            Section::new(z + tall * 0.42, 1.005),
                            Section::new(z + tall * 0.5, 1.005),
                        ],
                    )
                });
            }
        });
        z += tall;
    }
}

/// The stern's drives: each `(mouth's middle, radius)` a violet throat deep in a
/// faceted cowl, a graphite hood round its lip; hot air shimmers off them.
fn drives(b: &mut MeshBuilder, drives: &[([f32; 3], f32)]) {
    let sides = b.sides(20);
    for &(c, r) in drives {
        let c = Vec3::from(c);
        dark_plate(b);
        b.with_facets(|b| {
            b.cylinder_between(c + Vec3::X * 22.0, c, r * 1.32, r * 1.2, 8);
        });
        metal(b);
        hoop_on(
            b,
            c - Vec3::X * 0.5,
            Vec3::X,
            r * 1.12,
            r * 0.22,
            2.2,
            sides,
        );
        b.paint(GLOW_VIOLET);
        b.cylinder_between(
            c + Vec3::X * 4.0,
            c + Vec3::X * 1.5,
            r * 0.92,
            r * 0.92,
            sides,
        );
        if b.fine() {
            seam(b);
            hoop_on(
                b,
                c + Vec3::X * 1.0,
                Vec3::X,
                r * 0.97,
                r * 0.08,
                1.5,
                sides,
            );
        }
        b.add_exhaust(c, -Vec3::X, r);
    }
}

/// The lift plates under the belly: gravity lenses set flush in faceted rings.
fn lifts(b: &mut MeshBuilder, body: &Body, at: &[(f32, f32)]) {
    for &(x, y) in at {
        let u = body.u_down(x, y);
        let n = body.normal(x, u);
        let p = body.point(x, u);
        let sides = b.sides(16);
        dark_plate(b);
        b.with_facets(|b| b.cylinder_between(p - n * 1.5, p + n * 1.8, 14.0, 13.0, 8));
        b.paint(GLOW_VIOLET);
        let lens = p + n * 2.2;
        b.cylinder_between(p + n * 1.8, lens, 9.5, 9.5, sides);
        b.add_lift(lens, 9.5);
    }
}

/// The owner's colour: a chevron on the deck at `at`, `size` long.
fn mark(b: &mut MeshBuilder, at: Vec3, size: f32) {
    b.paint(TEAM);
    b.mirror_y(|b| {
        b.face(&[
            at,
            at + v3(-size, size * 0.42, 0.0),
            at + v3(-size * 1.3, size * 0.2, 0.0),
        ])
    });
}
