//! The arms: the Regent's two great wings, laid out from the user's sketch, brought down to
//! the destroyer's size. Each flares out of the hull's chine amidships, curves out to its
//! corner and bends there to run straight aft into a claw; its trailing edge is an arch
//! falling into the claw's inner edge. Feathered plates are swept out along it, every
//! other one lit, raised strakes follow its sweep, and a lit line runs just behind its
//! leading edge (`pattern::EMBER_SLOT`).

use glam::Vec3;

use crate::builder::MeshBuilder;
use crate::pattern;

use super::super::super::kit::{dark_plate, v3};
use super::super::super::machine::Frame;
use super::super::style::{shard, Finish};

/// A section across the arm at `y` out: its leading and trailing edges (x), the height of
/// its middle and its thickness.
#[derive(Clone, Copy)]
pub(super) struct Rib {
    pub(super) y: f32,
    le: f32,
    te: f32,
    z: f32,
    t: f32,
}

const fn rib(y: f32, le: f32, te: f32, z: f32, t: f32) -> Rib {
    Rib { y, le, te, z, t }
}

/// The port arm as the Regent has it, root to corner and round the claw: the leading edge
/// the sketch's flaring outline, the claw's tip at x -440, 600 out. `Wing` lays it on the
/// destroyer.
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

/// Where the Regent's arm is laid on the hull and how big: `k` its size against the
/// Regent's, its root rib (the Regent's y 200, x 0, z 200) brought to `root`.
#[derive(Clone, Copy)]
pub(super) struct Wing {
    pub(super) k: f32,
    pub(super) root: Vec3,
}

impl Wing {
    /// The arm's ribs, laid on the hull.
    pub(super) fn ribs(&self) -> [Rib; 11] {
        let k = self.k;
        let r = self.root;
        ARM.map(|a| Rib {
            y: r.y + (a.y - 200.0) * k,
            le: r.x + a.le * k,
            te: r.x + a.te * k,
            z: r.z + (a.z - 200.0) * k,
            t: a.t * k,
        })
    }

    /// The arm's rib at `y` out, among its ribs before the claw (straight between them).
    pub(super) fn rib_at(&self, y: f32) -> Rib {
        let all = self.ribs();
        let ribs = &all[..5];
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

    /// Far off: the arm's outline as one slab.
    pub(super) fn coarse(&self, b: &mut MeshBuilder) {
        let r = self.ribs();
        let (root, corner, claw) = (r[0], r[7], r[10]);
        let z = root.z;
        dark_plate(b);
        b.extrude_z(
            &[
                [root.le, root.y],
                [r[3].le, r[3].y],
                [corner.le, corner.y],
                [corner.te, corner.y],
                [claw.te, claw.y],
                [r[4].te, r[4].y],
                [root.te, root.y],
            ],
            z - root.t * 0.3,
            z + root.t * 0.3,
        );
    }
}

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
    pub(super) fn top(&self, along: f32) -> Vec3 {
        let x = self.le + (self.te - self.le) * along;
        v3(x, self.y, self.z + self.t * 0.46)
    }
}

/// The port arm: its sections lofted root to corner and round the claw, plates laid along
/// it from the root out, following its sweep, a lit one every other; a long plate down the
/// claw; raised strakes along it and a lit line behind its leading edge.
pub(super) fn arm(b: &mut MeshBuilder, w: &Wing) {
    let ribs = w.ribs();
    let k = w.k;
    let rings: Vec<Vec<Vec3>> = ribs.iter().map(Rib::ring).collect();
    dark_plate(b);
    b.with_facets(|b| b.loft(&rings, true, true));
    if b.coarse() {
        return;
    }
    // Plates swept out along the arm, each from one rib toward the next, overlapping.
    let along = |s: f32| {
        let (i, f) = ((s as usize).min(3), s - (s as usize).min(3) as f32);
        ribs[i].top(0.3).lerp(ribs[i + 1].top(0.3), f) + Vec3::Z * (3.0 * k)
    };
    let n = if b.fine() { 5 } else { 3 };
    for j in 0..n {
        let s = j as f32 / n as f32 * 4.0;
        let (o, out) = (along(s), along((s + 1.0).min(4.0)));
        let r = ribs[(s as usize).min(4)];
        let chord = r.le - r.te;
        let frame = Frame::new(o, out - o, Vec3::Z);
        let paint = if j % 2 == 1 {
            Finish::Lit
        } else {
            Finish::Dark
        };
        shard(
            b,
            &frame,
            (
                (out - o).length() * 1.4,
                (chord * 0.12).clamp(14.0 * k, 40.0 * k),
                0.3,
            ),
            (7.0 * k).max(1.5),
            paint,
        );
    }
    // The claw: a plate laid down its back from the corner to near its tip.
    let corner = ribs[7].top(0.1);
    let root = ribs[4].top(0.05);
    let f = Frame::new(root + Vec3::Z * (3.0 * k), corner - root, Vec3::Z);
    shard(
        b,
        &f,
        ((corner - root).length() * 1.05, 22.0 * k, 0.0),
        (6.0 * k).max(1.5),
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
        strip(b, &line(along, k), 12.0 * k, 7.0 * k);
    }
    if b.fine() {
        dark_plate(b);
        b.pattern(pattern::EMBER_SLOT);
        strip(
            b,
            &line(0.05, 0.5 * k),
            (5.0 * k).max(1.6),
            (2.0 * k).max(0.8),
        );
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
