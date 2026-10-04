//! Scourge: the Regency's tech 3 space destroyer (`regency_t3_space_destroyer`, mesh
//! `regency_space_destroyer`). A long hull of swelling and pinching sections, clad in
//! overlapping plate courses that sweep back into spikes, a dark trench down the spine
//! carrying the power conduit to the drives, red optics in the chine seam at the head.
//! Two broad pincers sweep out of the hump and curve back in past the stern either side
//! of the drives, each with its dampening channel down the inside (`tines.rs`); through
//! the ship's middle stands the drum of the energy core the Heavy Pinch-fusion Lance is
//! fired from, lit above and below (`core.rs`). Two blocks of open seeker cells stand on
//! the hump (`cells.rs`).
//! +X is forward, +Y port, the ground at z 0 (it is built on its lot, the core lowest).
//!
//! The ship is authored in its first hull's frame and built `SCALE` times bigger, moved
//! `SHIFT` forward so its middle, pincers and all, is the origin (`frame`).
//!
//! Contracts: `CORE_MUZZLE` is `space.ron` weapon 0's `muzzle`, and the seeker cells are
//! weapon 1's `muzzles` (tests.rs).

mod body;
mod cells;
mod core;
mod fittings;
mod tines;

use glam::{Affine3A, Vec3};

use crate::builder::MeshBuilder;
use crate::material::{GLOW_LASER, GLOW_VIOLET, TEAM};

use super::super::kit::{dark_plate, metal, v3};
use super::hull::mark;
use body::{feather, skin, skin_in, st, tube, Body};
use tines::{knot, Tine};

/// The hull: a wide, shallow head on a narrow neck, the body swelling again aft to the
/// hump, and the head rounding off into a broad blunt front.
const HULL: Body = Body {
    stations: &[
        st(-132.0, 12.0, 21.0, 27.0, 34.0),
        st(-108.0, 24.0, 17.0, 26.5, 42.0),
        st(-70.0, 31.0, 14.5, 26.0, 45.5),
        st(-30.0, 26.0, 14.0, 25.5, 41.0),
        st(10.0, 19.0, 14.0, 25.0, 37.0),
        st(44.0, 24.0, 15.5, 25.0, 37.0),
        st(76.0, 36.0, 18.0, 25.0, 36.0),
        st(104.0, 33.0, 19.5, 24.5, 33.0),
        st(122.0, 27.0, 21.0, 24.5, 30.0),
        st(132.0, 17.0, 22.5, 24.5, 28.0),
        st(136.0, 4.0, 23.6, 24.5, 26.5),
    ],
    upper: 2.8,
    lower: 2.2,
    far: &[0, 2, 4, 6, 8, 10],
};

/// The ship is authored this much smaller than it is built.
const SCALE: f32 = 1.25;
/// And moved this far forward (authoring metres) when built.
const SHIFT: f32 = 25.0;

/// The authoring frame placed on the model: scaled up and moved forward.
fn frame() -> Affine3A {
    Affine3A::from_translation(Vec3::X * SHIFT * SCALE) * Affine3A::from_scale(Vec3::splat(SCALE))
}

/// Where the lance leaves the core: the lens's face at the pod's lowest point (model
/// frame, metres; `core::LENS` placed by `frame`).
#[cfg(test)]
pub(crate) const CORE_MUZZLE: [f32; 3] = place(core::LENS);

/// The stern drives' mouths (model frame), for the drive effects.
pub(crate) const NOZZLES: [[f32; 3]; 3] = {
    let d = fittings::DRIVES;
    [place(d[0].0), place(d[1].0), place(d[2].0)]
};

/// A point in the authoring frame, placed on the model (`frame`).
const fn place(p: [f32; 3]) -> [f32; 3] {
    [(p[0] + SHIFT) * SCALE, p[1] * SCALE, p[2] * SCALE]
}

/// The drives' size as the drive effects take it (1: a 12 m mouth): the great drive's.
pub(crate) const DRIVE_SIZE: f32 = fittings::DRIVES[0].1 * SCALE / 12.0;

/// Where each seeker block's seekers leave from (model frame), port block first, in
/// firing order: `space.ron` weapons 1 and 2's `muzzles`.
#[cfg(test)]
pub(crate) fn seeker_muzzles() -> Vec<Vec<Vec3>> {
    cells::muzzles(&HULL)
        .iter()
        .map(|block| {
            block
                .iter()
                .map(|&p| Vec3::from(place(p.to_array())))
                .collect()
        })
        .collect()
}

/// The pincers: rooted deep in the hump, swept out wide and thick, curving back in to
/// their points past the stern.
const PINCER: Tine = Tine {
    knots: &[
        knot(-40.0, 20.0, 27.0, 5.0, 6.0),
        knot(-70.0, 38.0, 27.0, 11.0, 8.0),
        knot(-105.0, 58.0, 27.0, 11.0, 7.5),
        knot(-140.0, 60.0, 27.5, 9.0, 6.0),
        knot(-168.0, 46.0, 28.0, 6.0, 4.0),
        knot(-186.0, 34.0, 28.5, 0.8, 0.8),
    ],
    channel: (0.3, 0.95),
};

pub(super) fn destroyer(b: &mut MeshBuilder, _tech: u8) {
    b.with(frame(), |b| {
        let body = &HULL;
        if b.coarse() {
            let mut fins = vec![([[110.0, 12.0], [-90.0, 30.0], [-70.0, 20.0]], 25.0)];
            fins.extend(tines::far(&PINCER));
            coarse(b, body, &fins);
            return;
        }
        body.hull(b);
        b.mirror_y(|b| plates(b, body));
        trench(b, body);
        b.mirror_y(|b| optics(b, body));
        lifts(b, body);
        fittings::chine_blades(b, body, -84.0, 8.0);
        fittings::drives(b);
        tines::tines(b, &PINCER);
        core::core(b, body);
        cells::blocks(b, body);
    });
}

/// The plate courses, port side: a cowl over the head, a mantle course down the upper
/// flank, a lower course staggered against it, a keel course, and the two ridges either
/// side of the spine trench. The gaps between courses are the hull showing through,
/// dark: the seams.
fn plates(b: &mut MeshBuilder, body: &Body) {
    let (stern, bow) = body.span();
    dark_plate(b);
    // The cowl: one big plate over the head, its tail swept into a spike on the flank.
    feather(
        b,
        body,
        (bow - 5.0, bow - 48.0),
        (26.0, 88.5, 40.0),
        1.6,
        1.8,
        0.5,
    );
    let course = |b: &mut MeshBuilder,
                  (from, to): (f32, f32),
                  count: usize,
                  band: (f32, f32, f32),
                  thick: f32| {
        let step = (from - to) / count as f32;
        for k in 0..count {
            let head = from - step * k as f32;
            feather(b, body, (head, head - step - 9.0), band, thick, 1.3, 0.45);
        }
    };
    course(b, (bow - 38.0, stern + 18.0), 3, (24.0, 72.0, 44.0), 1.1);
    course(b, (bow - 22.0, stern + 30.0), 4, (-40.0, 16.0, -8.0), 1.0);
    course(b, (bow - 30.0, stern + 34.0), 3, (-84.0, -46.0, -60.0), 0.9);
    // The spine ridges, standing proud either side of the trench.
    let ridge = (
        body.phi_up(0.0, 9.5),
        body.phi_up(0.0, 3.6),
        body.phi_up(0.0, 6.0),
    );
    course(b, (bow - 32.0, stern + 30.0), 3, ridge, 2.2);
    // The owner's colour: a strip between the ridge and the mantle, over the fore body.
    b.paint(TEAM);
    let (lo, hi) = (ridge.0 - 4.0, ridge.0 - 1.6);
    skin(b, body, bow - 40.0, bow - 110.0, |t| {
        let k = 1.0 - (t - 0.8).max(0.0) * 5.0;
        (lo, lo + (hi - lo) * k, -0.1, 0.45)
    });
}

/// The trench down the spine: the conduit from the head to the drives, coupled every
/// few metres through a violet ring, the trench crossed by dark ribs.
fn trench(b: &mut MeshBuilder, body: &Body) {
    let (stern, bow) = body.span();
    let (from, to) = (bow - 34.0, stern + 26.0);
    let n = if b.fine() { 24 } else { 6 };
    let at = |t: f32| {
        let x = from + (to - from) * t;
        body.out(x, 90.0, 1.3)
    };
    metal(b);
    let sides = b.sides(8);
    let path: Vec<Vec3> = (0..=n).map(|i| at(i as f32 / n as f32)).collect();
    tube(b, &path, 1.3, sides);
    let couplings = if b.fine() { 6 } else { 3 };
    for i in 0..couplings {
        let t = (i as f32 + 0.5) / couplings as f32;
        let x = from + (to - from) * t;
        let c = body.out(x, 90.0, 1.3);
        // A plated sleeve over the conduit, a thin lit seam round its middle.
        dark_plate(b);
        b.cylinder_between(c + Vec3::X * 3.0, c - Vec3::X * 3.0, 2.0, 2.0, sides);
        b.paint(GLOW_LASER);
        b.cylinder_between(c + Vec3::X * 0.2, c - Vec3::X * 0.2, 2.08, 2.08, sides);
        if b.fine() {
            // A rib across the trench, ridge to ridge, behind each coupling.
            let rib = x - 7.0;
            let half = body.phi_up(rib, 6.5);
            skin_in(b, body, (rib + 0.8, rib - 0.8), (1, 4), |_| {
                (half, 180.0 - half, -0.1, 0.9)
            });
        }
    }
}

/// The optics: red slots in the chine seam at the head, largest forward, and the lit
/// seam itself running aft from them.
fn optics(b: &mut MeshBuilder, body: &Body) {
    let (stern, bow) = body.span();
    b.paint(GLOW_LASER);
    for (k, len) in [(0.0, 5.0), (8.0, 3.6), (14.5, 2.6)] {
        let x = bow - 30.0 - k;
        skin_in(b, body, (x + len * 0.5, x - len * 0.5), (1, 2), |t| {
            let w = 1.6 * (1.0 - 0.3 * t);
            (20.0 - w, 20.0 + w, -0.1, 0.14)
        });
    }
    if b.fine() {
        let (from, to) = (bow - 52.0, stern + 80.0);
        let n = 30;
        let path: Vec<Vec3> = (0..=n)
            .map(|i| body.out(from + (to - from) * i as f32 / n as f32, 20.0, 0.05))
            .collect();
        tube(b, &path, 0.22, 4);
    }
}

/// The lift plates under the belly, fore and aft of the lance: gravity lenses set flush
/// in plated rings (the Regency's lift; `MeshBuilder::add_lift`).
fn lifts(b: &mut MeshBuilder, body: &Body) {
    let (stern, bow) = body.span();
    for x in [bow * 0.45, stern * 0.55] {
        for phi in [-66.0, -114.0] {
            let n = body.normal(x, phi);
            let p = body.point(x, phi);
            let sides = b.sides(16);
            dark_plate(b);
            b.cylinder_between(p - n * 0.6, p + n * 0.7, 5.6, 5.2, sides);
            b.paint(GLOW_VIOLET);
            let lens = p + n * 0.85;
            b.cylinder_between(p + n * 0.7, lens, 3.8, 3.8, sides);
            b.add_lift(lens, 3.8);
        }
    }
}

/// Far off: the hull's outline in plan as one slab, the plan's fins as flat triangles
/// (drawn both ways up) and the owner's mark.
fn coarse(b: &mut MeshBuilder, body: &Body, fins: &[([[f32; 2]; 3], f32)]) {
    let s = body.stations;
    let mut outline: Vec<[f32; 2]> = body.far.iter().map(|&i| [s[i].x, s[i].w]).collect();
    outline.extend(body.far.iter().rev().map(|&i| [s[i].x, -s[i].w]));
    let keel = s.iter().map(|s| s.keel).sum::<f32>() / s.len() as f32;
    let spine = s.iter().map(|s| s.spine).fold(0.0, f32::max);
    dark_plate(b);
    b.extrude_z(&outline, keel, spine);
    for &(tri, z) in fins {
        b.mirror_y(|b| {
            let pts = tri.map(|[x, y]| v3(x, y, z));
            b.face(&pts);
            b.face(&[pts[2], pts[1], pts[0]]);
        });
    }
    mark(b, v3(s[s.len() * 3 / 4].x, 0.0, spine + 0.1), 14.0);
}
