//! The machine: one Precursor megastructure per map, laid out on one axis and
//! built into the ground. Its nodes are bastions (`PrecursorBastion`, with
//! cantilevered booms reaching out of them) and towers into the clouds
//! (`PrecursorTower`); spans (`PrecursorSpan`) bridge the valleys between them.
//! Each node stands on a bench cut level into the mountain for it (`Bench`): the
//! ground inside is set to the bench's level, and outside it rises or falls back
//! to the landscape over a steep face, so the machine sits in a notch of its own
//! the way a dam sits in a gorge. The axis runs on off the map: the map shows only
//! part of the machine.
//!
//! Spans stand on their node's bench, so a span's deck is at the bench's level plus
//! `DECK` times its scale. Two spans from benches at one level meet in the middle
//! with a gap of light between their ends; a span that ends in a tower may arrive
//! at any height up its shaft. The model sizes here match
//! `mc-models/src/precursor_mega.rs`.
//!
//! Round every bench stand the machine's doodads, set out square to it: beacons at
//! the corners, pylons flanking each boom's shoulder, a course of revetment wherever
//! the bench is cut into higher ground, conduits of light in the ground under a
//! span where it leaves.

use super::{Layout, Terrain};
use crate::format::PropKind;
use crate::noise::smoothstep;
use std::f64::consts::{FRAC_PI_2, PI};

/// One precursor artifact to stand on the map, on the terrain as it is shaped.
#[derive(Clone, Copy, Debug)]
pub(super) struct PrecursorSite {
    pub kind: PropKind,
    pub x: f64,
    pub y: f64,
    /// Radians, counter-clockwise from +x.
    pub heading: f64,
    pub scale: f64,
}

/// Straight-line distance, metres.
pub(super) fn dist(a: (f64, f64), b: (f64, f64)) -> f64 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

impl PrecursorSite {
    /// A circle round the solid plan, metres.
    pub(super) fn reach(&self) -> f64 {
        let plan = self.kind.solid_plan();
        let r = plan
            .iter()
            .map(|&(cx, cy, hx, hy)| ((cx.abs() + hx) as f64).hypot((cy.abs() + hy) as f64))
            .fold(0.0, f64::max);
        let r = if r == 0.0 { 32.0 } else { r };
        r * self.scale
    }
}

/// Whether a point is within `margin` of the solid plan of any of `sites`.
pub(super) fn in_solid(sites: &[PrecursorSite], x: f64, y: f64, margin: f64) -> bool {
    sites.iter().any(|s| {
        let plan = s.kind.solid_plan();
        if plan.is_empty() {
            return false;
        }
        let r = s.reach() + margin;
        if (x - s.x).abs() > r || (y - s.y).abs() > r {
            return false;
        }
        let (sn, c) = s.heading.sin_cos();
        let (dx, dy) = ((x - s.x) / s.scale, (y - s.y) / s.scale);
        let (lx, ly) = (dx * c + dy * sn, dy * c - dx * sn);
        let m = margin / s.scale;
        plan.iter().any(|&(cx, cy, hx, hy)| {
            (lx - cx as f64).abs() < hx as f64 + m && (ly - cy as f64).abs() < hy as f64 + m
        })
    })
}

/// A span's length and deck height over its bench at scale 1, metres.
const SPAN_LEN: f64 = 1_000.0;
#[cfg(test)]
const DECK: f64 = 84.0;
/// Where two spans meet: the light between their ends.
const SPAN_GAP: f64 = 10.0;
/// Solid half extents of a bastion's first tier, a tower's plinth, and a boom's
/// shoulder (whose middle is 10 m behind its origin), at scale 1.
const BASTION: (f64, f64) = (266.0, 181.0);
const TOWER: f64 = 80.0;
/// The Axis's foot, half its width at scale 1.
const AXIS_FOOT: f64 = 150.0;
/// The citadel's plinth, half extents at scale 1.
const CITADEL: (f64, f64) = (330.0, 280.0);
const SHOULDER: (f64, f64, f64) = (-10.0, 98.0, 86.0);
/// Level ground kept round the solid parts on a bench.
const APRON: f64 = 34.0;

/// Ground cut level for a node: a rectangle square to `heading`.
#[derive(Clone, Copy, Debug)]
pub(super) struct Bench {
    pub x: f64,
    pub y: f64,
    pub heading: f64,
    pub hx: f64,
    pub hy: f64,
    pub level: f64,
    /// How far outside the rectangle the ground takes to return to the landscape.
    pub blend: f64,
}

impl Bench {
    /// Distance outside the rectangle (negative inside), with rounded corners.
    pub(super) fn outside(&self, x: f64, y: f64) -> f64 {
        let (s, c) = self.heading.sin_cos();
        let (dx, dy) = (x - self.x, y - self.y);
        let (lx, ly) = (
            (dx * c + dy * s).abs() - self.hx,
            (dy * c - dx * s).abs() - self.hy,
        );
        if lx > 0.0 && ly > 0.0 {
            lx.hypot(ly)
        } else {
            lx.max(ly)
        }
    }

    /// Rough circle round the bench and its face, for a quick reject.
    fn reach(&self) -> f64 {
        self.hx.hypot(self.hy) + self.blend
    }
}

/// A node of the machine, for spans to join.
#[derive(Clone, Copy, Debug)]
pub(super) struct Node {
    pub x: f64,
    pub y: f64,
    pub level: f64,
    pub tower: bool,
}

/// Lays one machine.
pub(super) struct Machine<'a> {
    t: &'a Terrain,
    pub sites: Vec<PrecursorSite>,
    pub benches: Vec<Bench>,
    /// Faces steeper than this blend are cut; fills spread over `blend` too.
    blend: f64,
}

impl<'a> Machine<'a> {
    pub(super) fn new(t: &'a Terrain, blend: f64) -> Machine<'a> {
        Machine {
            t,
            sites: Vec::new(),
            benches: Vec::new(),
            blend,
        }
    }

    pub(super) fn put(&mut self, kind: PropKind, (x, y): (f64, f64), heading: f64, scale: f64) {
        self.sites.push(PrecursorSite {
            kind,
            x,
            y,
            heading,
            scale,
        });
    }

    fn bench(&mut self, (x, y): (f64, f64), heading: f64, hx: f64, hy: f64, level: f64) {
        self.benches.push(Bench {
            x,
            y,
            heading,
            hx,
            hy,
            level,
            blend: self.blend,
        });
        self.dress(self.benches.len() - 1);
    }

    /// A bastion facing `heading`, on a bench at `level`.
    pub(super) fn bastion(&mut self, at: (f64, f64), heading: f64, scale: f64, level: f64) -> Node {
        self.put(PropKind::PrecursorBastion, at, heading, scale);
        self.bench(
            at,
            heading,
            BASTION.0 * scale + APRON,
            BASTION.1 * scale + APRON,
            level,
        );
        Node {
            x: at.0,
            y: at.1,
            level,
            tower: false,
        }
    }

    /// A tower on a bench at `level`.
    pub(super) fn tower(&mut self, at: (f64, f64), heading: f64, scale: f64, level: f64) -> Node {
        self.put(PropKind::PrecursorTower, at, heading, scale);
        let half = TOWER * scale + APRON;
        self.bench(at, heading, half, half, level);
        Node {
            x: at.0,
            y: at.1,
            level,
            tower: true,
        }
    }

    /// The Axis (`PrecursorAxis`) standing on a bench at `level`: a node spans
    /// may run into, as into a tower's shaft.
    pub(super) fn axis(&mut self, at: (f64, f64), heading: f64, scale: f64, level: f64) -> Node {
        self.put(PropKind::PrecursorAxis, at, heading, scale);
        let half = AXIS_FOOT * scale + APRON;
        self.bench(at, heading, half, half, level);
        Node {
            x: at.0,
            y: at.1,
            level,
            tower: true,
        }
    }

    /// The Axis's citadel on a bench at `level`: a node spans run into.
    pub(super) fn citadel(&mut self, at: (f64, f64), heading: f64, scale: f64, level: f64) -> Node {
        self.put(PropKind::PrecursorCitadel, at, heading, scale);
        self.bench(
            at,
            heading,
            CITADEL.0 * scale + APRON,
            CITADEL.1 * scale + APRON,
            level,
        );
        Node {
            x: at.0,
            y: at.1,
            level,
            tower: true,
        }
    }

    /// A flush line of light in the ground along `heading`.
    pub(super) fn conduit(&mut self, at: (f64, f64), heading: f64) {
        self.put(PropKind::PrecursorConduit, at, heading, 1.0);
    }

    /// Two booms side by side out of a bastion's prow, reaching along its heading,
    /// their shoulders just apart; pylons flank each shoulder.
    pub(super) fn booms(&mut self, from: Node, heading: f64, scale: f64, bastion_scale: f64) {
        let (s, c) = heading.sin_cos();
        let apart = 2.0 * SHOULDER.2 * scale + 24.0;
        let out = BASTION.0 * bastion_scale - SHOULDER.1 * scale * 0.6;
        for side in [-0.5, 0.5] {
            let at = (
                from.x + c * out - s * apart * side,
                from.y + s * out + c * apart * side,
            );
            self.put(PropKind::PrecursorBoom, at, heading, scale);
            // The shoulder stands on the bastion's bench; widen it where it stands proud.
            let mid = (at.0 + c * SHOULDER.0 * scale, at.1 + s * SHOULDER.0 * scale);
            self.benches.push(Bench {
                x: mid.0,
                y: mid.1,
                heading,
                hx: SHOULDER.1 * scale + APRON,
                hy: SHOULDER.2 * scale + APRON,
                level: from.level,
                blend: self.blend,
            });
            for flank in [-1.0, 1.0] {
                let d = (SHOULDER.2 * scale + 18.0) * flank;
                let fwd = (SHOULDER.1 * scale + 10.0) * 0.4;
                let p = (mid.0 + c * fwd - s * d, mid.1 + s * fwd + c * d);
                self.put(PropKind::PrecursorPylon, p, heading, 1.6 * scale);
            }
        }
    }

    /// Spans from `a` to `b`. Nodes at one level get a span from each, meeting in
    /// the middle; otherwise one span leaves `a` and runs into `b`, which must then
    /// be a tower (checked in tests by the deck clearing the ground).
    pub(super) fn link(&mut self, a: Node, b: Node) {
        let d = (b.x - a.x).hypot(b.y - a.y);
        let heading = (b.y - a.y).atan2(b.x - a.x);
        if (a.level - b.level).abs() < 0.5 && d > 1.3 * SPAN_LEN {
            let scale = (d - SPAN_GAP) * 0.5 / SPAN_LEN;
            self.span(a, heading, scale);
            self.span(b, heading + PI, scale);
        } else {
            debug_assert!(
                b.tower || (a.level - b.level).abs() < 0.5,
                "a span must end in a tower or meet a span"
            );
            // Into a tower, well inside its shaft; to a bastion, at its face.
            let into = if b.tower { 0.0 } else { BASTION.1 * 0.7 };
            self.span(a, heading, (d - into) / SPAN_LEN);
        }
    }

    /// One span out of a node along `heading`: off the map, or as half a link.
    pub(super) fn span(&mut self, from: Node, heading: f64, scale: f64) {
        self.put(PropKind::PrecursorSpan, (from.x, from.y), heading, scale);
        // Two lines of light in the ground under its flanks, the whole way along it
        // wherever the ground is even enough to take them: the machine's line across
        // the valleys. Flush, so they block nothing.
        let (s, c) = heading.sin_cos();
        let mut along = 30.0;
        while along < SPAN_LEN * scale - 30.0 {
            for side in [-1.0, 1.0] {
                let off = 30.0 * scale * side;
                let p = (from.x + c * along - s * off, from.y + s * along + c * off);
                let clear = self
                    .benches
                    .iter()
                    .all(|b| b.outside(p.0, p.1) > b.blend + 20.0);
                let inside = p.0 > 40.0
                    && p.1 > 40.0
                    && p.0 < self.t.size_x - 40.0
                    && p.1 < self.t.size_y - 40.0;
                if clear && inside && self.walkable_flat(p) {
                    self.put(PropKind::PrecursorConduit, p, heading, 1.0);
                }
            }
            along += 61.0;
        }
    }

    /// Ground gentle enough for a flush conduit to lie on.
    fn walkable_flat(&self, (x, y): (f64, f64)) -> bool {
        let z = |dx: f64, dy: f64| self.t.natural(x + dx, y + dy);
        let h = z(0.0, 0.0);
        h > 1.0
            && [(30.0, 0.0), (-30.0, 0.0), (0.0, 30.0), (0.0, -30.0)]
                .iter()
                .all(|&(dx, dy)| (z(dx, dy) - h).abs() < 3.0)
    }

    /// A bench's doodads: spires in a bastion's corners and pylons in rows along
    /// its edges, beacons in a tower's corners, revetment along any cut face.
    fn dress(&mut self, index: usize) {
        let b = self.benches[index];
        let (s, c) = b.heading.sin_cos();
        let world = |lx: f64, ly: f64| (b.x + lx * c - ly * s, b.y + lx * s + ly * c);
        let bastion = b.hx > 200.0;
        for (sx, sy) in [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
            let p = world(sx * (b.hx - 20.0), sy * (b.hy - 20.0));
            if bastion {
                self.put(
                    PropKind::PrecursorSpire,
                    p,
                    b.heading + FRAC_PI_2 * 0.5,
                    1.5,
                );
            } else {
                self.put(PropKind::PrecursorBeacon, p, b.heading, 1.2);
            }
        }
        if bastion {
            // Rows of pylons down the long sides, square to them.
            for sy in [-1.0, 1.0] {
                let count = ((2.0 * b.hx - 120.0) / 95.0).floor() as usize;
                let each = (2.0 * b.hx - 120.0) / count.max(1) as f64;
                for i in 1..count {
                    let lx = -b.hx + 60.0 + each * i as f64;
                    self.put(
                        PropKind::PrecursorPylon,
                        world(lx, sy * (b.hy - 14.0)),
                        b.heading + FRAC_PI_2,
                        1.1,
                    );
                }
            }
        }
        // Each side: its outward normal (local), half length along it, distance out.
        let sides = [
            ((1.0, 0.0), b.hy, b.hx),
            ((-1.0, 0.0), b.hy, b.hx),
            ((0.0, 1.0), b.hx, b.hy),
            ((0.0, -1.0), b.hx, b.hy),
        ];
        // Wall props: 80 m long, 18 m tall at scale 1, face at x = 0 looking +x.
        let scale = 2.0;
        let long = 80.0 * scale;
        for ((nx, ny), half, out) in sides {
            let count = ((2.0 * half - 40.0) / long).floor().max(0.0) as usize;
            if count == 0 {
                continue;
            }
            let each = (2.0 * half - 40.0) / count as f64;
            for i in 0..count {
                let along = -half + 20.0 + each * (i as f64 + 0.5);
                // Along the side: the tangent is the normal turned a quarter.
                let (lx, ly) = (nx * (out - 1.0) - ny * along, ny * (out - 1.0) + nx * along);
                let p = world(lx, ly);
                let beyond = world(lx + nx * (b.blend * 0.6), ly + ny * (b.blend * 0.6));
                // Only where the ground behind stands higher: a real cut face.
                if self.t.natural(beyond.0, beyond.1) > b.level + 0.6 * 18.0 * scale {
                    let facing = ny.atan2(nx) + b.heading + PI;
                    self.put(PropKind::PrecursorWall, p, facing, each / 80.0);
                }
            }
        }
    }
}

impl Terrain {
    /// The artifacts as map props.
    pub(super) fn precursor_props(&self, out: &mut Vec<crate::format::Prop>) {
        use mc_core::{Angle, Fx, FxVec2};
        for s in &self.precursor {
            if s.x < 0.0 || s.y < 0.0 || s.x > self.size_x || s.y > self.size_y {
                continue;
            }
            let heading = (s.heading / std::f64::consts::TAU * 65_536.0)
                .round()
                .rem_euclid(65_536.0) as u16;
            out.push(crate::format::Prop {
                kind: s.kind,
                pos: FxVec2::new(
                    Fx((s.x * 65_536.0).round() as i64),
                    Fx((s.y * 65_536.0).round() as i64),
                ),
                heading: Angle(heading),
                scale_milli: (s.scale * 1_000.0).round().clamp(100.0, 65_000.0) as u16,
            });
        }
    }

    /// The ground as the machine's benches cut and fill it.
    pub(super) fn machine_ground(&self, x: f64, y: f64, h: f64) -> f64 {
        let mut h = h;
        for b in &self.benches {
            let r = b.reach();
            if (x - b.x).abs() > r || (y - b.y).abs() > r {
                continue;
            }
            let d = b.outside(x, y);
            if d < b.blend {
                h += (b.level - h) * (1.0 - smoothstep(0.0, b.blend, d));
            }
        }
        h
    }

    /// How much of a glacier's ice may show here: none on a bench or its faces.
    pub(super) fn machine_ice(&self, x: f64, y: f64) -> f64 {
        self.benches.iter().fold(1.0, |k: f64, b| {
            let r = b.reach() + b.blend;
            if (x - b.x).abs() > r || (y - b.y).abs() > r {
                return k;
            }
            k.min(smoothstep(b.blend, 2.5 * b.blend, b.outside(x, y)))
        })
    }

    /// Whether trees and rocks may stand here: off the machine's benches and clear
    /// of its artifacts. On the archipelago the jungle grows right up to the walls:
    /// only the solid footprints themselves are kept clear.
    pub(super) fn machine_clear(&self, x: f64, y: f64) -> bool {
        if self.layout == Layout::Archipelago {
            return !self.in_precursor_solid(x, y, 6.0);
        }
        if self.benches.iter().any(|b| b.outside(x, y) < b.blend * 0.5) {
            return false;
        }
        if self.layout == Layout::Threshold {
            return self.threshold_clear(x, y);
        }

        true
    }

    /// Whether a point is within `margin` of any artifact's solid plan.
    pub(super) fn in_precursor_solid(&self, x: f64, y: f64, margin: f64) -> bool {
        in_solid(&self.precursor, x, y, margin)
    }

    /// Lays the map's machine: its artifacts join `self.precursor`, its benches
    /// shape the ground from here on.
    pub(super) fn lay_machine(&mut self) {
        // MC_BAKE_NO_MACHINE: bake without it, for A/B comparisons.
        if std::env::var_os("MC_BAKE_NO_MACHINE").is_some() {
            return;
        }
        let (sites, benches) = {
            let m = match self.layout {
                Layout::Alpine => self.machine_divide(),
                Layout::AlpineTeams => self.machine_sound(),
                Layout::Threshold => self.machine_threshold(),
                Layout::Archipelago => self.machine_axis(),
                Layout::Frostline => self.machine_wall(),
                _ => return,
            };
            (m.sites, m.benches)
        };
        self.precursor.extend(sites);
        self.benches.extend(benches);
    }

    /// Serac Divide: along the middle line, a tower in the western massif and a
    /// bastion on the divide between the valleys, the spans between them 500 m
    /// over the inland valley, booms reaching east over the coast meadow toward
    /// the sea, and a span off the west edge. Everything on the mirror line and
    /// square to it, so the solid ground mirrors onto itself.
    fn machine_divide(&self) -> Machine<'_> {
        const D: f64 = 8_192.0;
        let f = self.size / D;
        let mid = self.size_y / 2.0;
        let mut m = Machine::new(self, 44.0 * f);
        let level = 490.0;
        let west = m.tower((150.0 * f, mid), 0.0, 1.0 * f, level);
        let divide = m.bastion((2_300.0 * f, mid), 0.0, 1.0 * f, level);
        m.booms(divide, 0.0, 1.35 * f, 1.0 * f);
        m.link(west, divide);
        m.span(west, PI, 1.0 * f);
        m
    }

    /// Serac Sound: across the icefield on the middle line, from a tower over the
    /// west edge to a bastion on the last ridge above the pass, its booms reaching
    /// out over the slopes toward the pass and the sea.
    fn machine_sound(&self) -> Machine<'_> {
        const D: f64 = 12_288.0;
        let f = self.size / D;
        let mid = self.size_y / 2.0;
        let at = |x: f64| (x * f, mid);
        let mut m = Machine::new(self, 48.0 * f);
        let west = m.tower(at(300.0), 0.0, 1.0 * f, 725.0);
        let cap = m.bastion(at(1_350.0), 0.0, 1.0 * f, 730.0);
        let field = m.tower(at(2_650.0), 0.0, 1.1 * f, 612.0);
        let ridge = m.bastion(at(3_480.0), 0.0, 1.0 * f, 685.0);
        m.booms(ridge, 0.0, 1.5 * f, 1.0 * f);
        m.link(cap, west);
        m.link(cap, field);
        m.link(ridge, field);
        m.span(west, PI, 1.0 * f);
        m
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every span's keel clears the ground from where it leaves its bench to its end,
    /// and every node stands on its bench.
    #[test]
    fn spans_clear_the_ground() {
        use crate::bake::test_maps::{SERAC_DIVIDE, SERAC_SOUND, THE_AXIS, THRESHOLD};
        let maps: [(&str, &Terrain); 4] = [
            ("Serac Divide", &SERAC_DIVIDE),
            ("Serac Sound", &SERAC_SOUND),
            ("The Threshold", &THRESHOLD),
            ("The Axis", &THE_AXIS),
        ];
        let mut problems = Vec::new();
        for (name, t) in maps {
            assert!(!t.benches.is_empty(), "{name}: no machine");
            for s in t
                .precursor
                .iter()
                .filter(|s| s.kind == PropKind::PrecursorSpan)
            {
                let bench = t
                    .benches
                    .iter()
                    .find(|b| (b.x - s.x).hypot(b.y - s.y) < 1.0)
                    .expect("a span leaves a bench");
                let keel = bench.level + (DECK - 42.0) * s.scale;
                let (sn, c) = s.heading.sin_cos();
                let mut worst = f64::NEG_INFINITY;
                let mut along = 0.0;
                while along < SPAN_LEN * s.scale {
                    let p = (s.x + c * along, s.y + sn * along);
                    if p.0 >= 0.0
                        && p.1 >= 0.0
                        && p.0 <= t.size_x
                        && p.1 <= t.size_y
                        && bench.outside(p.0, p.1) > bench.blend
                    {
                        worst = worst.max(t.natural(p.0, p.1) - keel);
                    }
                    along += 16.0;
                }
                if worst > 0.0 {
                    problems.push(format!(
                        "{name}: span from {:?} runs {worst:.0} m into the ground",
                        (s.x as i64, s.y as i64)
                    ));
                }
            }
            for b in &t.benches {
                let h = t.natural(b.x, b.y);
                if (h - b.level).abs() > 0.5 {
                    problems.push(format!(
                        "{name}: bench at {:?} is at {h:.0}, not {:.0}",
                        (b.x as i64, b.y as i64),
                        b.level
                    ));
                }
            }
        }
        assert!(problems.is_empty(), "{}", problems.join("\n"));
    }
}
