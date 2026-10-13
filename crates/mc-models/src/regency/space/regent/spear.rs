//! The spear: the Regent's one hull, laid out from the user's sketch. From above: a
//! long narrow spearhead whose edges flare out from a third of the way back into two
//! great arms, each curving out to its corner and bending there to run straight aft into
//! a claw; under each arm its trailing edge is an arch, open between the claw and the
//! tail; behind the arms a long narrow tail of notched vertebrae tapers to a point. From
//! the side it is hunched, deepest under the bridge at the arms' roots, under feathered
//! carapace plates; a small swept fin stands on its back, a pair of armoured spikes
//! hangs under the arms' roots and a lesser one under the tail. The twelve grand batteries make a gallery down the spearhead's
//! chines; the drives sit in two pods tucked under the arms' roots.
//!

use std::sync::LazyLock;

use glam::{Affine3A, Vec2, Vec3};

use crate::builder::{MeshBuilder, Section};
use crate::pattern;

use super::super::super::kit::{dark_plate, metal, v3};
use super::super::super::machine::Frame;
use super::super::body::{feather, st, Body, Station, CHINE, DECK_EDGE, SPINE};
use super::super::dress::{drives, lifts, mark};
use super::super::style::{self, shard, Finish};
use super::guns::{
    arm, guns, rondels, spines, well, Kind, Mount, Spine, AA, AA_LANCE, LANCE, REPEATER, RONDEL,
    SUPPRESSION,
};
use super::{
    batteries, deck, horns, keel_guns, plates, pocketed, row, terrace, tier_tops, Block, Courses,
    Piece,
};

/// How far the whole ship stands off z 0, so the fin hung under it clears the ground.
pub(super) const LIFT: f32 = 330.0;

/// The hull, stern first: the tail's vertebrae, the body under the arms' roots, the
/// spearhead. Half-beams are the sketch's outline.
const STATIONS: [Station; 21] = [
    st(-1460.0, 3.0, 2.0, 205.0, 219.7, 2.0, 237.6, 240.0),
    st(-1200.0, 18.0, 5.8, 168.3, 216.2, 7.2, 274.4, 282.4),
    st(-1000.0, 34.0, 10.9, 140.0, 213.5, 13.6, 302.8, 315.0),
    st(-800.0, 56.0, 17.9, 120.0, 209.2, 22.4, 317.6, 332.5),
    st(-700.0, 80.0, 25.6, 110.0, 207.1, 32.0, 325.1, 341.2),
    st(-600.0, 90.0, 28.8, 100.0, 205.0, 36.0, 332.5, 350.0),
    st(-500.0, 140.0, 44.8, 90.0, 203.4, 56.0, 341.1, 360.0),
    st(-400.0, 128.0, 41.0, 80.0, 201.8, 51.2, 349.7, 370.0),
    st(-300.0, 170.0, 54.4, 70.0, 200.2, 68.0, 358.3, 380.0),
    st(-200.0, 172.0, 55.0, 60.0, 199.7, 68.8, 369.2, 392.5),
    st(-100.0, 175.0, 56.0, 50.0, 199.1, 70.0, 380.1, 405.0),
    st(60.0, 230.0, 73.6, 43.6, 200.5, 92.0, 391.0, 417.2),
    st(250.0, 290.0, 92.8, 43.3, 199.3, 116.0, 388.7, 414.7),
    st(450.0, 290.0, 92.8, 55.0, 197.8, 116.0, 371.2, 395.0),
    st(600.0, 238.0, 76.2, 65.0, 197.3, 95.2, 357.9, 380.0),
    st(700.0, 210.0, 67.2, 75.0, 199.6, 84.0, 350.9, 371.7),
    st(800.0, 200.0, 64.0, 85.0, 201.9, 80.0, 343.8, 363.3),
    st(900.0, 222.0, 71.0, 95.0, 204.2, 88.8, 336.8, 355.0),
    st(1100.0, 190.0, 60.8, 125.0, 211.1, 76.0, 315.6, 330.0),
    st(1500.0, 80.0, 25.6, 175.0, 221.2, 32.0, 277.3, 285.0),
    st(1900.0, 6.0, 2.0, 215.0, 227.6, 2.4, 242.9, 245.0),
];

/// The grand batteries' pockets down each chine of the spearhead.
const BATTERIES: [f32; 6] = [1350.0, 1180.0, 1010.0, 840.0, 700.0, 560.0];

/// The tail's vertebrae: each a bulge plated in a chevron, a lit notch behind it.
const VERTEBRAE: [f32; 3] = [-300.0, -500.0, -700.0];

pub(super) static HULL: LazyLock<Body> = LazyLock::new(|| pocketed(&STATIONS, &BATTERIES));

/// A keel spike: how it hangs.
#[derive(Clone, Copy)]
struct Spike {
    /// Its housing's middle along the hull, and out from the keel (a pair if not 0).
    x: f32,
    y: f32,
    /// How far it rakes aft of straight down, and cants out, radians.
    rake: f32,
    cant: f32,
    /// Its size against the authored one (a 380 m spike from a 300 m housing).
    size: f32,
}

/// The spikes under the hull: a pair under the arms' roots, canted out, and a lesser
/// one under the tail.
const SPIKES: [Spike; 2] = [
    Spike {
        x: 200.0,
        y: 60.0,
        rake: 0.3,
        cant: 0.18,
        size: 0.75,
    },
    Spike {
        x: -880.0,
        y: 0.0,
        rake: 0.45,
        cant: 0.0,
        size: 0.45,
    },
];
/// The authored spike's length from its housing to its tip.
const SPIKE_LEN: f32 = 380.0;

pub(super) fn build(b: &mut MeshBuilder, _tech: u8) {
    b.at(Vec3::Z * LIFT, draw);
}

/// A section across an arm at `y` out: its leading and trailing edges (x), the height
/// of its middle and its thickness.
#[derive(Clone, Copy)]
struct Rib {
    y: f32,
    le: f32,
    te: f32,
    z: f32,
    t: f32,
}

const fn rib(y: f32, le: f32, te: f32, z: f32, t: f32) -> Rib {
    Rib { y, le, te, z, t }
}

/// The port arm, root to corner and round the claw: the leading edge is the sketch's
/// flaring outline, the trailing edge its arch falling into the claw's inner edge, the
/// claw's tip at x -440, 600 out, its outer edge running back up to the corner.
const ARM: [Rib; 11] = [
    rib(200.0, 660.0, 10.0, 200.0, 110.0),
    rib(300.0, 460.0, -5.0, 200.0, 96.0),
    rib(380.0, 330.0, -30.0, 198.0, 84.0),
    rib(450.0, 230.0, -65.0, 196.0, 72.0),
    rib(510.0, 145.0, -110.0, 194.0, 60.0),
    rib(550.0, 95.0, -280.0, 192.0, 50.0),
    rib(580.0, 60.0, -380.0, 190.0, 42.0),
    rib(600.0, 30.0, -440.0, 188.0, 34.0),
    rib(620.0, -10.0, -400.0, 188.0, 30.0),
    rib(635.0, -60.0, -300.0, 188.0, 24.0),
    rib(642.0, -110.0, -180.0, 188.0, 14.0),
];
impl Rib {
    /// The section's corners: the leading edge, over the top to the trailing edge and
    /// back under it.
    fn ring(&self) -> Vec<Vec3> {
        let c = self.le - self.te;
        let (le, te, z, t, y) = (self.le, self.te, self.z, self.t, self.y);
        vec![
            v3(le, y, z),
            v3(le - c * 0.12, y, z + t * 0.5),
            v3(te + c * 0.15, y, z + t * 0.42),
            v3(te, y, z - t * 0.05),
            v3(te + c * 0.15, y, z - t * 0.35),
            v3(le - c * 0.12, y, z - t * 0.5),
        ]
    }

    /// The top at `along` the chord (0 the leading edge, 1 the trailing).
    fn top(&self, along: f32) -> Vec3 {
        let x = self.le + (self.te - self.le) * along;
        v3(x, self.y, self.z + self.t * 0.46)
    }
}

/// The arm's rib at `y` out, among the arm's own ribs before the claw (straight between
/// them).
fn rib_at(y: f32) -> Rib {
    let ribs = &ARM[..5];
    let i = ribs
        .windows(2)
        .position(|w| y <= w[1].y)
        .unwrap_or(ribs.len() - 2);
    let (a, c) = (ribs[i], ribs[i + 1]);
    let k = ((y - a.y) / (c.y - a.y)).clamp(0.0, 1.0);
    let f = |p: f32, q: f32| p + (q - p) * k;
    Rib {
        y,
        le: f(a.le, c.le),
        te: f(a.te, c.te),
        z: f(a.z, c.z),
        t: f(a.t, c.t),
    }
}

const TOWER: Block = (160.0, 280.0, 74.0);
const TOWER_TIERS: [(f32, f32); 3] = [(34.0, 1.0), (30.0, 0.8), (24.0, 0.6)];

/// The drive pod tucked under the port arm's root, about its own middle.
const POD: [Station; 3] = [
    st(-110.0, 50.0, 20.0, 10.0, 50.0, 24.0, 100.0, 106.0),
    st(20.0, 54.0, 22.0, 0.0, 50.0, 26.0, 106.0, 114.0),
    st(110.0, 22.0, 8.0, 26.0, 58.0, 10.0, 88.0, 92.0),
];
const POD_AT: Vec3 = Vec3::new(-80.0, 150.0, 130.0);
/// The pod's drive: its mouth (pod frame) and radius.
const POD_DRIVE: (Vec3, f32) = (Vec3::new(-110.0, 0.0, 52.0), 32.0);

static PODS: LazyLock<Piece> = LazyLock::new(|| Piece::new(&POD, &[], POD_AT, 0.0));

/// Every gun but the seekers: port side (drawn mirrored) and on the centreline.
pub(super) fn mounts(body: &Body) -> (Vec<Mount>, Vec<Mount>) {
    let mut port = batteries(body, &BATTERIES);
    port.extend(row(
        body,
        Kind::Suppression,
        &[1600.0, 1260.0, 920.0],
        4.0,
        6.0,
    ));
    port.extend(row(body, Kind::Suppression, &[-350.0, -600.0], 4.0, 6.0));
    for (y, along) in [(330.0, 0.1), (430.0, 0.12), (500.0, 0.15)] {
        port.push(Mount::port(
            Kind::Suppression,
            rib_at(y).top(along) + Vec3::Z * (SUPPRESSION * 0.3),
        ));
    }
    port.extend(row(
        body,
        Kind::Repeater,
        &[1700.0, 1430.0, 1090.0, -420.0, -650.0, -900.0],
        DECK_EDGE,
        REPEATER * 0.4,
    ));
    for (y, along) in [(260.0, 0.85), (360.0, 0.85), (440.0, 0.8), (500.0, 0.72)] {
        port.push(Mount::port(
            Kind::Aa,
            rib_at(y).top(along) + Vec3::Z * (AA * 0.5),
        ));
    }
    port.extend(row(
        body,
        Kind::Aa,
        &[-250.0, -460.0, -560.0, -780.0],
        4.5,
        AA * 0.5,
    ));
    port.extend(row(body, Kind::Rondel, &[1450.0, 760.0], 4.6, RONDEL * 0.5));
    port.extend(row(
        body,
        Kind::Rondel,
        &[-150.0, -680.0],
        4.6,
        RONDEL * 0.5,
    ));
    port.extend(keel_guns(body, &[1500.0, 1000.0, 650.0, 350.0], 40.0));
    let tower = tier_tops(body, TOWER, &TOWER_TIERS);
    for (x, z) in [(220.0, tower[0]), (120.0, tower[1])] {
        port.push(Mount::port(Kind::AaLance, v3(x, 36.0, z + AA_LANCE * 0.5)));
    }
    let mut centre: Vec<Mount> = [1450.0, 1250.0]
        .iter()
        .map(|&x| Mount::ahead(Kind::Lance, deck(body, x) + Vec3::Z * (LANCE * 0.3)))
        .collect();
    centre.extend([1150.0, 800.0].iter().map(|&x| {
        Mount::ahead(
            Kind::VentralLance,
            body.point(x, 0.0) - Vec3::Z * (LANCE * 0.3),
        )
    }));
    (port, centre)
}

/// The port arm's seeker spine on the arm's root, four cells along and eight across.
pub(super) fn seekers() -> Spine {
    let at = Vec2::new(180.0, 360.0);
    let rib = rib_at(at.y + 86.0);
    let top = rib.z + rib.t * 0.5;
    Spine {
        at,
        // Held to a quarter metre, as a gun's pivot is: the unit file gives the cells exactly.
        deck: ((top + 10.0) * 4.0).round() / 4.0,
        foot: top - rib.t * 0.7,
        along: 4,
        across: 8,
    }
}

fn draw(b: &mut MeshBuilder) {
    let body: &Body = &HULL;
    if b.coarse() {
        coarse(b);
        return;
    }
    let (port, centre) = mounts(body);
    body.hull(b);
    b.mirror_y(|b| {
        carapace(b, body);
        vertebrae(b, body);
        arm_hull(b);
        pod(b);
        rondels(b, &port);
    });
    spines(b, seekers());
    for m in &centre {
        well(b, m.pivot, LANCE * 2.3, LANCE * 0.8, m.kind == Kind::Lance);
    }
    arm(b, &guns(&port, &centre));
    let crown = terrace(b, body, TOWER, &TOWER_TIERS);
    horns(b, TOWER, crown);
    fins(b, body);
    lifts(
        b,
        body,
        &[
            (450.0, 90.0),
            (450.0, -90.0),
            (-150.0, 70.0),
            (-150.0, -70.0),
        ],
        32.0,
    );
    mark(b, deck(body, 700.0) + Vec3::Z * 0.4, 50.0);
}

/// Far off: the hull's plan as one slab, each arm a slab of its outline.
fn coarse(b: &mut MeshBuilder) {
    style::slab(
        b,
        &[
            [1900.0, 0.0],
            [600.0, 238.0],
            [-100.0, 175.0],
            [-1460.0, 0.0],
        ],
        60.0,
        420.0,
    );
    let z = ARM[0].z;
    b.mirror_y(|b| {
        b.extrude_z(
            &[
                [660.0, 200.0],
                [-10.0, 620.0],
                [-440.0, 600.0],
                [-110.0, 510.0],
                [10.0, 200.0],
            ],
            z - 30.0,
            z + 30.0,
        );
    });
    mark(b, v3(700.0, 0.0, 421.0), 60.0);
}

/// The carapace, port side: feathered plates over the shoulder and deck down the
/// spearhead and over the arms' roots, each one's trailing edge lifted clear over the
/// head of the next, so the hull reads as layered shell; the hull's own plate courses
/// under them, and the tail's carapace aft to the first vertebra.
fn carapace(b: &mut MeshBuilder, body: &Body) {
    plates(
        b,
        body,
        (1880.0, -1440.0),
        230.0,
        Courses {
            below: true,
            saw: false,
        },
    );
    for (from, to, count, lift) in [(1850.0, 0.0, 8, 34.0), (-40.0, -260.0, 1, 24.0)] {
        let step = (from - to) / count as f32;
        for k in 0..count {
            let head = from - step * k as f32;
            dark_plate(b);
            feather(
                b,
                body,
                (head, head - step * 1.3),
                (CHINE + 0.45, SPINE - 0.42, DECK_EDGE - 0.2),
                12.0,
                lift,
                0.45,
            );
        }
    }
}

/// The tail's vertebrae, port side: each plated in a chevron pointed aft on the spine,
/// the notch behind it lit.
fn vertebrae(b: &mut MeshBuilder, body: &Body) {
    for x in VERTEBRAE {
        dark_plate(b);
        feather(
            b,
            body,
            (x + 60.0, x - 110.0),
            (CHINE - 0.3, SPINE - 0.02, SPINE - 0.05),
            12.0,
            22.0,
            0.6,
        );
        if !b.coarse() {
            let f = style::on(body, x - 100.0, CHINE + 0.6, 0.5, 0.1);
            shard(b, &f, (70.0, 10.0, 0.2), 4.0, Finish::Lit);
        }
    }
}

/// The port arm: its sections lofted root to corner and round the claw, and plates laid
/// along it from the root out, following its sweep, a lit one every other; a long plate
/// down the claw.
fn arm_hull(b: &mut MeshBuilder) {
    let ribs = &ARM;
    let rings: Vec<Vec<Vec3>> = ribs.iter().map(Rib::ring).collect();
    dark_plate(b);
    b.with_facets(|b| b.loft(&rings, true, true));
    if b.coarse() {
        return;
    }
    // Plates swept out along the arm, each from one rib toward the next, overlapping.
    let along = |s: f32| {
        let (i, f) = ((s as usize).min(3), s - (s as usize).min(3) as f32);
        ribs[i].top(0.3).lerp(ribs[i + 1].top(0.3), f) + Vec3::Z * 3.0
    };
    let n = if b.fine() { 5 } else { 3 };
    for k in 0..n {
        let s = k as f32 / n as f32 * 4.0;
        let (o, out) = (along(s), along((s + 1.0).min(4.0)));
        let chord = ribs[(s as usize).min(4)].le - ribs[(s as usize).min(4)].te;
        let frame = Frame::new(o, out - o, Vec3::Z);
        let paint = if k % 2 == 1 {
            Finish::Lit
        } else {
            Finish::Dark
        };
        shard(
            b,
            &frame,
            (
                (out - o).length() * 1.4,
                (chord * 0.12).clamp(14.0, 40.0),
                0.3,
            ),
            7.0,
            paint,
        );
    }
    // The claw: a plate laid down its back from the corner to near its tip.
    let corner = ribs[7].top(0.1);
    let root = ribs[4].top(0.05);
    let f = Frame::new(root + Vec3::Z * 3.0, corner - root, Vec3::Z);
    shard(
        b,
        &f,
        ((corner - root).length() * 1.05, 22.0, 0.0),
        6.0,
        Finish::Dark,
    );
    // The arm's long lines: raised strakes following its sweep from the root round the
    // corner and down the claw, near its leading edge, down its middle and along its
    // arched trailing edge; a lit line just behind the leading edge.
    let line = |along: f32, lift: f32| -> Vec<Vec3> {
        ribs[..8]
            .iter()
            .map(|r| r.top(along) + Vec3::Z * lift)
            .collect()
    };
    dark_plate(b);
    for along in [0.14, 0.5, 0.86] {
        strip(b, &line(along, 1.0), 12.0, 7.0);
    }
    if b.fine() {
        dark_plate(b);
        b.pattern(pattern::EMBER_SLOT);
        strip(b, &line(0.05, 0.5), 5.0, 2.0);
    }
}

/// A raised strip along `points` on a surface facing up: `w` across and `h` high,
/// following the line round its bends.
fn strip(b: &mut MeshBuilder, points: &[Vec3], w: f32, h: f32) {
    let n = points.len();
    let rings: Vec<Vec<Vec3>> = (0..n)
        .map(|k| {
            let (a, c) = (points[k.saturating_sub(1)], points[(k + 1).min(n - 1)]);
            let side = Vec3::Z.cross(c - a).normalize_or_zero() * (w * 0.5);
            let p = points[k];
            vec![
                p - side,
                p + side,
                p + side + Vec3::Z * h,
                p - side + Vec3::Z * h,
            ]
        })
        .collect();
    b.with_facets(|b| b.loft(&rings, true, true));
}

/// The drive pod under the port arm's root: its hull, a grille ringing the mouth lit in
/// slots, the drive's violet throat in it.
fn pod(b: &mut MeshBuilder) {
    PODS.draw(b, |b, body| {
        body.hull(b);
        if !b.coarse() {
            b.mirror_y(|b| {
                plates(
                    b,
                    body,
                    (100.0, -100.0),
                    100.0,
                    Courses {
                        below: false,
                        saw: false,
                    },
                )
            });
        }
        let (mouth, r) = POD_DRIVE;
        dark_plate(b);
        b.pattern(pattern::EMBER_SLOT);
        b.with_facets(|b| {
            b.cylinder_between(
                mouth + Vec3::X * 10.0,
                mouth - Vec3::X * 2.0,
                r * 1.45,
                r * 1.5,
                8,
            )
        });
        drives(b, &[(mouth.to_array(), r)], 40.0);
    });
}

/// A blade on edge from `root` (the middle of its leading edge), running aft along
/// `aft`, `tall` either side of that line, `len` long: a swept plate drawn as a shard.
fn blade(b: &mut MeshBuilder, root: Vec3, aft: Vec3, len: f32, tall: f32, lit: bool) {
    let f = Frame::new(root, aft, Vec3::Y);
    let paint = if lit { Finish::Lit } else { Finish::Dark };
    shard(
        b,
        &f,
        (len, tall, 0.9),
        (tall * 0.08).clamp(3.0, 12.0),
        paint,
    );
}

/// The spike's sections along it from the housing to the tip: how far along (0 to 1),
/// its chord fore and aft, its width.
const SPIKE: [(f32, f32, f32); 5] = [
    (0.0, 190.0, 90.0),
    (0.2, 170.0, 80.0),
    (0.55, 130.0, 62.0),
    (0.85, 95.0, 46.0),
    (1.0, 80.0, 38.0),
];
/// How far the chisel tip's cut rises from its trailing edge to its leading edge.
const CHISEL: f32 = 34.0;

/// The keel spike on the port side of the keel (or on it): a faceted armoured housing
/// hung under the hull, plates swept back down its flanks, a lit slot in its brow; out
/// of it a thick tapering prism jutting down and aft, ribbed down its faces, two
/// collars round it, its tip cut off on a slant like a chisel.
fn spike(b: &mut MeshBuilder, body: &Body, s: Spike) {
    let reach = 150.0 * s.size;
    let keel = [s.x - reach, s.x, s.x + reach]
        .iter()
        .map(|&x| body.point(x, 0.0).z)
        .fold(f32::MIN, f32::max);
    // Authored about the keel under the housing's middle, at full size.
    let place =
        Affine3A::from_translation(v3(s.x, s.y, keel)) * Affine3A::from_scale(Vec3::splat(s.size));
    b.with(place, |b| spike_at(b, s));
}

/// The spike about the keel under its housing's middle, at its authored size.
fn spike_at(b: &mut MeshBuilder, s: Spike) {
    let keel = 0.0;
    // The housing.
    let plan = [
        [150.0, 0.0],
        [100.0, 70.0],
        [-110.0, 66.0],
        [-150.0, 30.0],
        [-150.0, -30.0],
        [-110.0, -66.0],
        [100.0, -70.0],
    ];
    dark_plate(b);
    b.with_facets(|b| {
        b.loft_z(
            &plan,
            &[
                Section::new(keel - 120.0, 0.72),
                Section::new(keel - 60.0, 0.95),
                Section::new(keel + 20.0, 1.0),
            ],
        )
    });
    if !b.coarse() {
        for side in [1.0f32, -1.0] {
            let f = Frame::new(
                v3(110.0, side * 64.0, keel - 30.0),
                v3(-1.0, side * 0.08, -0.25),
                v3(0.0, side, -0.3),
            );
            shard(b, &f, (260.0, 34.0, 0.4), 7.0, Finish::Dark);
        }
        let f = Frame::new(v3(140.0, 0.0, keel - 50.0), v3(-0.4, 0.0, -1.0), Vec3::X);
        shard(b, &f, (60.0, 22.0, 0.0), 3.0, Finish::Lit);
    }
    // The prism, perpendicular sections down its axis.
    let origin = v3(-20.0, 0.0, keel - 100.0);
    let axis = v3(-s.rake.sin(), s.cant.sin(), -s.rake.cos()).normalize();
    let chord = (Vec3::X - axis * Vec3::X.dot(axis)).normalize();
    let across = axis.cross(chord);
    let ring = |t: f32, c: f32, w: f32, grow: f32| -> Vec<Vec3> {
        let mid = origin + axis * (t * SPIKE_LEN);
        let (c, w) = (c * grow, w * grow);
        [
            (0.5, 0.0),
            (0.25, 0.5),
            (-0.35, 0.5),
            (-0.5, 0.0),
            (-0.35, -0.5),
            (0.25, -0.5),
        ]
        .iter()
        .map(|&(u, v)| {
            let p = mid + chord * (u * c) + across * (v * w);
            if t >= 1.0 {
                // The chisel: the tip's cut rises toward its leading edge.
                p - axis * (CHISEL * (u + 0.5))
            } else {
                p
            }
        })
        .collect()
    };
    let rings: Vec<Vec<Vec3>> = SPIKE.iter().map(|&(t, c, w)| ring(t, c, w, 1.0)).collect();
    dark_plate(b);
    b.with_facets(|b| b.loft(&rings, true, true));
    if b.coarse() {
        return;
    }
    // Collars round it.
    for k in [1, 2] {
        let (t, c, w) = SPIKE[k];
        let band = [ring(t, c, w, 1.08), ring(t - 0.05, c, w, 1.08)];
        dark_plate(b);
        b.with_facets(|b| b.loft(&band, true, true));
    }
    // Ridges down its faces, root to tip.
    if b.fine() {
        metal(b);
        for corner in [1usize, 2, 4, 5] {
            for w in rings.windows(2) {
                b.beam(
                    w[0][corner],
                    w[1][corner],
                    Vec2::new(4.0, 4.0),
                    Vec2::new(3.0, 3.0),
                );
            }
        }
    }
}

/// The fins: a small one on the back aft of the bridge, and the keel spikes.
fn fins(b: &mut MeshBuilder, body: &Body) {
    if b.coarse() {
        return;
    }
    let back = body.point(-60.0, SPINE);
    blade(
        b,
        back + Vec3::Z * 30.0,
        v3(-1.0, 0.0, 0.35),
        200.0,
        30.0,
        false,
    );
    for s in SPIKES {
        if s.y > 0.0 {
            b.mirror_y(|b| spike(b, body, s));
        } else {
            spike(b, body, s);
        }
    }
}
