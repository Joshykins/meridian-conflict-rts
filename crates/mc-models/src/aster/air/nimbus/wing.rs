//! A faceted wing laid out by stations along the span, as the Eclipse's is: each station
//! a chord at one span with its thickness, the upper skin in three flat facets (the
//! crown's lines), the lower in three. Panels laid on it are cut on those lines
//! ([`Wing::skin`]), so every piece lies flat on its facet.

use super::*;

/// A wing station: `(y, z, lead, trail, thick)`, the chord at height `z`.
pub(super) type Station = (f32, f32, f32, f32, f32);

/// The upper skin's height over the chord, as a share of the thickness, at chord
/// fractions 0, 0.2, 0.6 and 1.
const CROWN: [(f32, f32); 4] = [(0.0, 0.0), (0.2, 0.5), (0.6, 0.42), (1.0, 0.06)];
/// The lower skin's depth under the chord the same way.
const KEEL: [(f32, f32); 4] = [(0.0, 0.0), (0.18, -0.42), (0.6, -0.4), (1.0, 0.06)];

pub(super) struct Wing {
    pub(super) stations: &'static [Station],
}

impl Wing {
    /// The station at span `y`, between the two either side.
    pub(super) fn at(&self, y: f32) -> Station {
        let s = self.stations;
        let y = y.clamp(s[0].0, s[s.len() - 1].0);
        let k = s
            .windows(2)
            .position(|w| y <= w[1].0)
            .unwrap_or(s.len() - 2);
        let (a, c) = (s[k], s[k + 1]);
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
    pub(super) fn edges(&self, y: f32) -> (f32, f32) {
        let (_, _, lead, trail, _) = self.at(y);
        (lead, trail)
    }

    /// The chord fraction of `x` at span `y`: 0 on the leading edge, 1 on the trailing.
    pub(super) fn fraction(&self, x: f32, y: f32) -> f32 {
        let (lead, trail) = self.edges(y);
        (lead - x) / (lead - trail)
    }

    fn surface(&self, x: f32, y: f32, profile: &[(f32, f32); 4]) -> Vec3 {
        let (_, z, lead, trail, t) = self.at(y);
        let f = ((lead - x) / (lead - trail)).clamp(0.0, 1.0);
        let k = profile.windows(2).position(|w| f <= w[1].0).unwrap_or(2);
        let (a, c) = (profile[k], profile[k + 1]);
        let h = a.1 + (c.1 - a.1) * (f - a.0) / (c.0 - a.0);
        v3(x, y, z + t * h)
    }

    /// The point on the upper skin over `(x, y)` (`y` either side).
    pub(super) fn top(&self, x: f32, y: f32) -> Vec3 {
        self.surface(x, y.abs(), &CROWN).with_y(y)
    }

    /// The point on the lower skin under `(x, y)` (`y` either side).
    pub(super) fn bottom(&self, x: f32, y: f32) -> Vec3 {
        self.surface(x, y.abs(), &KEEL).with_y(y)
    }

    fn ring(s: Station, profile: &[(f32, f32); 4]) -> Vec<Vec3> {
        let (y, z, lead, trail, t) = s;
        profile
            .iter()
            .map(|&(f, h)| v3(lead + (trail - lead) * f, y, z + t * h))
            .collect()
    }

    /// The left half: upper skin in `upper` paint, lower in `lower`, every facet flat.
    /// Below full detail each station is a diamond.
    pub(super) fn build(&self, b: &mut MeshBuilder, upper: u32, lower: u32) {
        b.with_facets(|b| {
            if b.fine() {
                b.paint(upper).pattern(pattern::AIRFRAME);
                let rings: Vec<_> = self
                    .stations
                    .iter()
                    .map(|&s| Self::ring(s, &CROWN))
                    .collect();
                b.loft(&rings, true, true);
                b.paint(lower).pattern(pattern::AIRFRAME);
                let rings: Vec<_> = self
                    .stations
                    .iter()
                    .map(|&s| Self::ring(s, &KEEL))
                    .collect();
                b.loft(&rings, true, true);
            } else {
                b.paint(upper);
                let rings: Vec<_> = self.stations.iter().map(|&s| Self::diamond(s)).collect();
                b.loft(&rings, true, true);
            }
        });
    }

    /// The left half as diamonds through `picks` of the stations, no caps: the coarse level.
    pub(super) fn coarse(&self, b: &mut MeshBuilder, picks: &[usize], paint: u32) {
        b.paint(paint);
        let rings: Vec<_> = picks
            .iter()
            .map(|&k| Self::diamond(self.stations[k]))
            .collect();
        b.loft(&rings, false, false);
    }

    fn diamond(s: Station) -> Vec<Vec3> {
        let (y, z, lead, trail, t) = s;
        let c = lead - trail;
        vec![
            v3(lead, y, z),
            v3(lead - c * 0.35, y, z + t * 0.48),
            v3(trail, y, z + t * 0.06),
            v3(lead - c * 0.4, y, z - t * 0.4),
        ]
    }

    /// A panel laid on the upper skin, `lift` above it: the quad `front` (two plan points,
    /// each `[x, y]`) to `back`, each side at one span. It is cut on the skin's facet lines
    /// so every piece lies flat on its facet; keep both spans between the same two stations.
    pub(super) fn skin(
        &self,
        b: &mut MeshBuilder,
        front: [[f32; 2]; 2],
        back: [[f32; 2]; 2],
        lift: f32,
    ) {
        let ends = [0, 1].map(|s| {
            let y = front[s][1];
            (
                y,
                self.fraction(front[s][0], y),
                self.fraction(back[s][0], y),
            )
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
            let (lead, trail) = self.edges(y);
            self.top(lead + (trail - lead) * f, y) + Vec3::Z * lift
        };
        for w in cuts.windows(2) {
            let (f, g) = (w[0], w[1]);
            let quad = [at(f[0], 0), at(f[1], 1), at(g[1], 1), at(g[0], 0)];
            // Wound to face up whichever way round the corners were given.
            let up = (quad[1] - quad[0]).cross(quad[3] - quad[0]).z > 0.0;
            if up {
                b.face(&quad);
            } else {
                b.face(&[quad[3], quad[2], quad[1], quad[0]]);
            }
        }
    }
}
