//! The Regent: the Regency's tech 5 NR-Class Super Destroyer (`regency_t5_super_destroyer`,
//! mesh `regency_regent`), about three and a half kilometres from bow to tail. One hull,
//! the spear (`spear.rs`), laid out from the user's sketch.
//!
//! Every design carries the same guns (`guns.rs`): twelve grand batteries, four heavy
//! lances (two dorsal, two under the keel), sixteen suppression batteries, eight keel
//! guns, twelve twin repeaters, sixteen AA repeaters, four AA lances, eight Rondels and
//! two seeker spines of thirty-two cells. The Edict is fired from wherever on the hull is
//! nearest its mark, so it has no gun of its own.
//! +X is forward, +Y port, z 0 the lowest point. Authored in metres as built.

mod guns;
mod spear;

use glam::{Affine3A, Vec3};

use crate::builder::{MeshBuilder, Section};
use crate::library::ModelDef;
use crate::pattern;

use super::super::kit::{dark_plate, v3};
use super::super::machine::Frame;
use super::body::{Body, Station, BILGE, CHINE, DECK_EDGE, KEEL_EDGE, SPINE};
use super::dress::course;
use super::style::{shard, Finish};
use guns::{Kind, Mount, BATTERY_R};

/// The nominal radius and height every design is built to (the unit file's).
const RADIUS: f32 = 1900.0;
const HEIGHT: f32 = 760.0;

pub(crate) const MODELS: &[ModelDef] = &[ModelDef::new(
    "regency_regent",
    RADIUS,
    HEIGHT,
    spear::build,
)];

/// A grand battery's pocket: how far the chine is pressed in at its bottom, its half
/// lengths at the hull's line and at its flat bottom, and how far the ball's middle
/// stands out of the bottom (just inside the hull's line).
const NOTCH: f32 = 0.75 * BATTERY_R;
const POCKET: (f32, f32) = (1.7 * BATTERY_R, 0.45 * BATTERY_R);
const BALL_OUT: f32 = NOTCH - 0.1 * BATTERY_R;

/// `stations` with the chine pressed into a pocket at each `mounts` x.
fn pocketed(stations: &[Station], mounts: &[f32]) -> Body {
    let plain = Body {
        stations: stations.to_vec(),
    };
    let mut out = stations.to_vec();
    for &m in mounts {
        for (dx, notch) in [
            (POCKET.0, 0.0),
            (POCKET.1, NOTCH),
            (-POCKET.1, NOTCH),
            (-POCKET.0, 0.0),
        ] {
            out.push(Station {
                notch,
                ..plain.at(m + dx)
            });
        }
    }
    out.sort_by(|a, c| a.x.total_cmp(&c.x));
    Body { stations: out }
}

/// The port grand batteries in their pockets at `xs`.
fn batteries(body: &Body, xs: &[f32]) -> Vec<Mount> {
    xs.iter()
        .map(|&x| Mount::port(Kind::Battery, body.out(x, CHINE, BALL_OUT)))
        .collect()
}

/// Port mounts of `kind` down the hull at each x, `u` round the section, `h` out.
fn row(body: &Body, kind: Kind, xs: &[f32], u: f32, h: f32) -> Vec<Mount> {
    xs.iter()
        .map(|&x| Mount::port(kind, body.out(x, u, h)))
        .collect()
}

/// Keel guns hung under the keel flat at each x, `y` out from the keel line.
fn keel_guns(body: &Body, xs: &[f32], y: f32) -> Vec<Mount> {
    xs.iter()
        .map(|&x| {
            let u = body.u_down(x, y).min(KEEL_EDGE);
            Mount::port(Kind::Keel, body.point(x, u) - Vec3::Z * guns::KEEL * 0.55)
        })
        .collect()
}

/// The spine's top at `x`.
fn deck(body: &Body, x: f32) -> Vec3 {
    body.point(x, SPINE)
}

/// A terraced block on the spine centred at `x`: each tier `(tall, scale)` a pointed
/// hexagon in plan `len` by `half` at scale 1, set back and narrower than the one under
/// it, a lit slot round its brow. Returns its top.
fn terrace(b: &mut MeshBuilder, body: &Body, (x, len, half): Block, tiers: &[(f32, f32)]) -> f32 {
    let plan = |l: f32, h: f32| -> Vec<[f32; 2]> {
        vec![
            [l * 0.5, 0.0],
            [l * 0.24, h],
            [-l * 0.42, h * 0.94],
            [-l * 0.5, h * 0.55],
            [-l * 0.5, -h * 0.55],
            [-l * 0.42, -h * 0.94],
            [l * 0.24, -h],
        ]
    };
    // The foot runs down to the lowest point of the deck under it.
    let mut z = foot(body, x, len, half);
    for (k, &(tall, f)) in tiers.iter().enumerate() {
        let (l, h) = (len * f, half * f);
        let back = -len * 0.06 * k as f32;
        let p = plan(l, h);
        b.at(v3(x + back, 0.0, 0.0), |b| {
            dark_plate(b);
            b.with_facets(|b| {
                b.loft_z(
                    &p,
                    &[
                        Section::new(z, 1.0),
                        Section::new(z + tall * 0.72, 1.0),
                        Section::new(z + tall, 0.92),
                    ],
                )
            });
            if !b.coarse() {
                dark_plate(b);
                b.pattern(pattern::EMBER_SLOT);
                b.loft_z(
                    &p,
                    &[
                        Section::new(z + tall * 0.42, 1.004),
                        Section::new(z + tall * 0.55, 1.004),
                    ],
                );
            }
        });
        z += tall;
    }
    z
}

/// The deck's plates either side of the spine, `from` back to `to`, `count` of them.
fn deck_course(b: &mut MeshBuilder, body: &Body, (from, to): (f32, f32), count: usize) {
    dark_plate(b);
    super::dress::course(
        b,
        body,
        (from, to),
        count,
        (DECK_EDGE - 0.2, SPINE - 0.12, DECK_EDGE + 0.3),
        5.0,
    );
}

/// A terraced block on the spine: its middle, length and half-width.
type Block = (f32, f32, f32);

/// The tops of `terrace`'s tiers on `block`, lowest first.
fn tier_tops(body: &Body, (x, len, half): Block, tiers: &[(f32, f32); 3]) -> [f32; 3] {
    let mut z = foot(body, x, len, half);
    tiers.map(|(tall, _)| {
        z += tall;
        z
    })
}

/// The lowest point of the deck under a block `len` by `half` at `x`, a little sunk.
fn foot(body: &Body, x: f32, len: f32, half: f32) -> f32 {
    [x - len * 0.5, x, x + len * 0.5]
        .iter()
        .map(|&x| body.point(x, body.u_up(x, half)).z)
        .fold(f32::MAX, f32::min)
        - 4.0
}

/// A command tower's horns over `block`, its crown at `top`: two plates swept up and
/// back off the crown, a lit one swept out off each flank. Both sides.
fn horns(b: &mut MeshBuilder, (x, _, half): Block, top: f32) {
    if b.coarse() {
        return;
    }
    b.mirror_y(|b| {
        let f = Frame::new(
            v3(x + 40.0, half * 0.25, top - 8.0),
            v3(-1.0, 0.3, 0.2),
            v3(0.1, 0.35, 1.0),
        );
        shard(b, &f, (240.0, 18.0, 0.7), 5.0, Finish::Dark);
        let g = Frame::new(
            v3(x + 10.0, half * 0.6, top - 40.0),
            v3(-1.0, 0.4, 0.05),
            v3(0.0, 1.0, 0.5),
        );
        shard(b, &g, (200.0, 16.0, 0.5), 4.0, Finish::Lit);
    });
}

/// A saw of plates down the port chine from `from` back to `to`: `count` shards, each
/// `len` long, swept back and out past the chine so the hull's edge is cut into spikes.
fn chine_saw(b: &mut MeshBuilder, body: &Body, (from, to): (f32, f32), count: usize, len: f32) {
    let step = (from - to) / count.max(1) as f32;
    for k in 0..count {
        let x = from - step * k as f32;
        let f = super::style::on(body, x, CHINE - 0.05, 3.0, 0.28);
        shard(b, &f, (len, 30.0, 0.1), 6.0, Finish::Dark);
    }
}

/// A sub-hull: its faceted body and where it stands in the ship.
struct Piece {
    body: Body,
    place: Affine3A,
}

impl Piece {
    /// A body from `stations`, its chine pressed into a battery pocket at each `pockets`
    /// x, stood at `at` and turned `yaw` (radians, counter-clockwise from above).
    fn new(stations: &[Station], pockets: &[f32], at: Vec3, yaw: f32) -> Self {
        Self {
            body: pocketed(stations, pockets),
            place: Affine3A::from_translation(at) * Affine3A::from_rotation_z(yaw),
        }
    }

    /// Draws `f` in the piece's own frame.
    fn draw(&self, b: &mut MeshBuilder, f: impl FnOnce(&mut MeshBuilder, &Body)) {
        b.with(self.place, |b| f(b, &self.body));
    }
}

/// Which of a piece's plate courses are laid: the keel and bilge (close up, and only
/// where the underside shows) and the saw of swept plates down the chine.
#[derive(Clone, Copy)]
struct Courses {
    below: bool,
    saw: bool,
}

/// The plate courses on a piece's port side from `from` back to `to`, a plate about every
/// `pitch` metres: keel and bilge, flank, shoulder and deck, and the chine's saw.
fn plates(b: &mut MeshBuilder, body: &Body, (from, to): (f32, f32), pitch: f32, c: Courses) {
    let n = |k: f32| (((from - to) / (pitch * k)).round() as usize).max(2);
    let saw = c.saw;
    dark_plate(b);
    if b.fine() && c.below {
        course(
            b,
            body,
            (from - 20.0, to),
            n(1.3),
            (0.15, KEEL_EDGE + 0.35, KEEL_EDGE),
            4.0,
        );
        course(
            b,
            body,
            (from - 40.0, to),
            n(1.2),
            (BILGE - 0.3, CHINE + 0.25, CHINE + 0.1),
            4.0,
        );
    }
    course(
        b,
        body,
        (from, to),
        n(1.0),
        (KEEL_EDGE + 0.2, CHINE - 0.1, CHINE - 0.4),
        5.0,
    );
    course(
        b,
        body,
        (from - 10.0, to),
        n(1.0),
        (CHINE + 0.08, DECK_EDGE + 0.1, CHINE + 0.3),
        6.0,
    );
    deck_course(b, body, (from, to), n(1.1));
    if saw {
        chine_saw(b, body, (from - 60.0, to), n(1.0), pitch * 1.25);
    }
}
