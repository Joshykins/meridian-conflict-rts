//! The Ark (`regency_t3_assault_transport`): the Regency's heavy transport, the Bastion's
//! counterpart at the Bastion's size. A broad arrowhead: the hold's hull run out into
//! wings laid in feathered plates whose tails are the saw-toothed trailing edge, two
//! armour ridges down the spine with the graphite workings in the trench between them,
//! a raised bridge on the foredeck and canted fins over the stern drives
//! (docs/STYLE.md "The Regency look").
//!
//! It never sets down: it hangs on its gravity lifts with its hold floor [`FLOOR`]
//! metres up and lets a long ramp down through its belly. The hull round the hold is an
//! arch lofted along x ([`Sec`], [`HULL`]): a slot [`LANE`] wide is left open from the
//! ground to the hold roof, so units walk up the ramp and along the hold, and in under
//! the stern. Forward of the hinge a deck closes the slot at the floor; the ramp
//! (`part::RAMP`, authored open, one in two so it swings flush with the belly to close)
//! fills it aft. The numbers are the unit file's `transport`.
use glam::Vec3;

use super::super::kit::{dark_plate, v3};
use super::super::machine::{armour, collar, red_slot, swept, Frame};
use crate::builder::MeshBuilder;
use crate::library::ModelDef;
use crate::material::TEAM;

mod deck;
mod hold;
mod stern;
#[cfg(test)]
mod tests;
mod wing;

pub(crate) const MODELS: &[ModelDef] =
    &[ModelDef::new("regency_ark", RADIUS, HEIGHT, build).with_ramp(HINGE, FLOOR)];

const RADIUS: f32 = 150.0;
const HEIGHT: f32 = 100.0;

/// The hold, as the unit file's `transport` has it: the floor's height once down, the
/// ramp's hinge and lip, and half the lane's width (the unit file's `width` and a metre).
const FLOOR: f32 = 38.0;
const HINGE: f32 = -10.0;
const LIP: f32 = -86.0;
const LANE: f32 = 23.0;
/// The underside of the hold's roof (the floor and the unit file's 36 m clearance, and
/// some), and the front bulkhead.
const ROOF: f32 = 78.0;
const FRONT: f32 = 58.0;

/// The hull, stern to prow.
const HULL: [Sec; 10] = [
    Sec::open(-150.0, 28.0, 44.0, 56.0, 72.0, 50.0),
    Sec::open(-120.0, 36.0, 38.0, 54.0, 82.0, 56.0),
    Sec::open(-88.0, 40.0, 32.0, 52.0, 86.0, 78.0),
    Sec::open(-30.0, 44.0, 30.0, 52.0, 88.0, 78.0),
    Sec::open(30.0, 42.0, 30.0, 52.0, 88.0, 78.0),
    Sec::open(58.0, 38.0, 30.0, 52.0, 86.0, 78.0),
    Sec::shut(74.0, 32.0, 32.0, 52.0, 84.0),
    Sec::shut(110.0, 20.0, 40.0, 54.0, 76.0),
    Sec::shut(140.0, 8.0, 48.0, 55.0, 64.0),
    Sec::shut(158.0, 1.0, 53.0, 56.0, 58.0),
];

/// One station of the hull: at `x`, `w` its half width at the beam (`beam` high),
/// `belly` the outer keel's height, `top` the deck's. The hold slot under it is `notch`
/// either side of the centre line and reaches up to `roof`; a closed station has a
/// sliver of a slot (`notch` under a metre, `roof` just over the belly).
#[derive(Clone, Copy)]
struct Sec {
    x: f32,
    w: f32,
    belly: f32,
    beam: f32,
    top: f32,
    notch: f32,
    roof: f32,
}

/// The faces of the hull's +y half, from the slot's foot round to the deck's middle:
/// what [`surface`] takes to say which face a point is on.
#[derive(Clone, Copy)]
enum Face {
    /// The slot's foot out to the keel's chine.
    Belly = 1,
    /// The keel's chine up to the knuckle at the beam.
    Lower = 2,
    /// The knuckle up to the upper chine.
    Upper = 3,
    /// The upper chine up to the deck's edge.
    Slope = 4,
    /// The deck's edge in to its middle.
    Deck = 5,
}

impl Sec {
    const fn open(x: f32, w: f32, belly: f32, beam: f32, top: f32, roof: f32) -> Self {
        Sec {
            x,
            w,
            belly,
            beam,
            top,
            notch: LANE,
            roof,
        }
    }

    const fn shut(x: f32, w: f32, belly: f32, beam: f32, top: f32) -> Self {
        Sec {
            x,
            w,
            belly,
            beam,
            top,
            notch: 0.3,
            roof: belly + 1.6,
        }
    }

    /// The +y half of the section: the slot's foot, the keel's chine, the knuckle at the
    /// beam, the upper chine, the deck's edge and the deck's middle. Hard chines: a flat
    /// belly, a knuckle at the beam and a broad flat deck.
    fn half(&self) -> [Vec3; 6] {
        [
            v3(self.x, self.notch, self.belly + 1.5),
            v3(self.x, self.w * 0.86, self.belly),
            v3(self.x, self.w, self.beam),
            v3(
                self.x,
                self.w * 0.9,
                self.beam + (self.top - self.beam) * 0.5,
            ),
            v3(self.x, self.w * 0.5, self.top),
            v3(self.x, 0.0, self.top + 1.0),
        ]
    }

    /// The ring round this station: the +y half, the -y half back, and over the slot.
    fn ring(&self) -> Vec<Vec3> {
        let half = self.half();
        let mut ring = half.to_vec();
        ring.extend(half[..5].iter().rev().map(|p| *p * v3(1.0, -1.0, 1.0)));
        ring.push(v3(self.x, -self.notch, self.roof));
        ring.push(v3(self.x, self.notch, self.roof));
        ring
    }

    fn lerp(&self, o: &Sec, k: f32) -> Sec {
        let l = |a: f32, c: f32| a + (c - a) * k;
        Sec {
            x: l(self.x, o.x),
            w: l(self.w, o.w),
            belly: l(self.belly, o.belly),
            beam: l(self.beam, o.beam),
            top: l(self.top, o.top),
            notch: l(self.notch, o.notch),
            roof: l(self.roof, o.roof),
        }
    }
}

/// The hull's section at `x`, as the loft between its stations has it.
fn at(x: f32) -> Sec {
    let x = x.clamp(HULL[0].x, HULL[HULL.len() - 1].x);
    let i = HULL
        .windows(2)
        .position(|w| x <= w[1].x)
        .unwrap_or(HULL.len() - 2);
    let (a, c) = (HULL[i], HULL[i + 1]);
    a.lerp(&c, (x - a.x) / (c.x - a.x))
}

/// The point `t` of the way across `face` of the hull's +y half at `x`, and the face's
/// outward normal there.
fn surface(x: f32, face: Face, t: f32) -> (Vec3, Vec3) {
    let j = face as usize;
    let point = |x: f32| {
        let h = at(x).half();
        h[j - 1].lerp(h[j], t)
    };
    let h = at(x).half();
    let across = h[j] - h[j - 1];
    let along = point(x + 1.0) - point(x - 1.0);
    let mut n = along.cross(across).normalize();
    let p = point(x);
    if n.dot(p - v3(x, 0.0, at(x).beam)) < 0.0 {
        n = -n;
    }
    (p, n)
}

/// A frame on `face` at `x`, `t` across it, its `u` running aft along the hull.
fn frame_on(x: f32, face: Face, t: f32, lift: f32) -> Frame {
    let (p, n) = surface(x, face, t);
    let (q, _) = surface(x - 2.0, face, t);
    Frame::new(p + n * lift, q - p, n)
}

fn build(b: &mut MeshBuilder, _tech: u8) {
    if b.coarse() {
        coarse(b);
        return;
    }
    let rings: Vec<Vec<Vec3>> = HULL.iter().map(Sec::ring).collect();
    dark_plate(b);
    b.with_facets(|b| b.loft(&rings, true, true));
    hold::build(b);
    b.mirror_y(|b| {
        wing::build(b);
        deck::flank(b);
        deck::ridge(b);
        stern::fin(b);
        stern::drives(b);
        stern::lifts(b);
        mark(b, v3(-60.0, 56.0, 62.6), 10.0);
    });
    deck::bridge(b);
    deck::prow(b);
    mark(b, v3(-56.0, 0.0, 89.4), 7.0);
}

fn coarse(b: &mut MeshBuilder) {
    let half = [[-150.0, 30.0], [-102.0, 80.0], [-68.0, 80.0], [100.0, 38.0]];
    let mut outline = vec![[-140.0, 0.0]];
    outline.extend(half);
    outline.push([158.0, 0.0]);
    outline.extend(half.iter().rev().map(|&[x, y]| [x, -y]));
    dark_plate(b);
    b.extrude_z(&outline, 44.0, 84.0);
    mark(b, v3(10.0, 0.0, 84.05), 14.0);
}

/// A lit red optic: a slot on the face through `at` (outward `out`), running `along`.
fn optic(b: &mut MeshBuilder, at: Vec3, out: Vec3, along: Vec3, len: f32) {
    if b.mid() {
        red_slot(b, at, out, along, len, len * 0.22);
    }
}

/// The faction's mark: a swept chevron of team colour lying on the deck at `at`.
fn mark(b: &mut MeshBuilder, at: Vec3, size: f32) {
    b.paint(TEAM);
    b.mirror_y(|b| {
        b.face(&[
            at + v3(size * 0.5, 0.0, 0.0),
            at + v3(-size, size * 0.55, 0.0),
            at + v3(-size * 0.7, size * 0.2, 0.0),
            at + v3(-size * 0.2, 0.0, 0.0),
        ])
    });
}

/// A plate of armour on the face with frame `f`: swept back `len` from its leading edge,
/// `half` across, its spike at `tip` across.
fn shard(b: &mut MeshBuilder, f: &Frame, len: f32, half: f32, tip: f32, thick: f32) {
    dark_plate(b);
    armour(b, f, &swept(len, half, tip, 0.35), thick);
}

/// A drum of graphite machinery between plates: a collar about `axis` at `at`.
fn drum(b: &mut MeshBuilder, at: Vec3, axis: Vec3, r: f32, width: f32) {
    if b.mid() {
        collar(b, at, axis, r, width);
    }
}
