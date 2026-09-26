//! Landmarks: great works of the colonists that belong to a map
//! ([`PropKind::Dam`](crate::PropKind::Dam)). The baker cuts the ground for one
//! from these numbers and the renderer's model is built to the same numbers,
//! so the two meet.

/// An arch dam in plan, in the prop's own frame: x along its heading
/// (upstream, into the lake), y to its left, z up from the crest road, which
/// passes through the origin. All lengths in metres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DamPlan {
    /// Radius of the crest road's centreline. The arch bows upstream: its
    /// centre lies `radius` downstream of the origin, at `(-radius, 0)`.
    pub radius: f64,
    /// Half the angle the crest's arc subtends at that centre, in radians.
    /// The arc ends (the abutments) stand at `±half_angle` off the x axis.
    pub half_angle: f64,
    /// Half the dam's thickness at the crest, either side of the centreline.
    pub crest_half: f64,
    /// The terrain under the crest is level from `road_down` metres downstream
    /// of the centreline to `road_up` upstream of it, at the crest's height;
    /// the model's crest covers that band with a margin. Units walk it: it
    /// must hold a chain of 8 m path cells where the arc runs diagonal.
    pub road_up: f64,
    pub road_down: f64,
    /// The crest road's height above the water level.
    pub crest_z: f64,
    /// Depth of the gorge's bed under the dam, below the water level.
    pub bed: f64,
    /// Height above the water of the lake's old full pool: the top of the
    /// white mineral ring on the canyon's walls and the dam's upstream face.
    pub ring_top: f64,
    /// Length of level ground at crest height the baker leaves beyond each
    /// abutment, along the arc's tangent, for the road off the dam.
    pub approach: f64,
    /// The upstream face's batter: how far it leans out per metre down.
    pub up_batter: f64,
    /// The toe the upstream face flares into under the water: how far out,
    /// between which depths under the crest.
    pub toe: f64,
    pub toe_from: f64,
    pub toe_to: f64,
    /// The downstream face spreads as the depth to the power 1.5, times this.
    pub down_spread: f64,
    /// How far the dam's footing reaches under the gorge's bed.
    pub footing: f64,
}

impl DamPlan {
    /// Crest centreline point at angle `a` round the arch (0 at the origin).
    pub fn crest_at(&self, a: f64) -> (f64, f64) {
        (self.radius * a.cos() - self.radius, self.radius * a.sin())
    }

    /// A point in the dam's frame as (angle round the arch, offset from the
    /// crest centreline, positive upstream).
    pub fn arch_coords(&self, x: f64, y: f64) -> (f64, f64) {
        let (cx, cy) = (x + self.radius, y);
        (cy.atan2(cx), (cx * cx + cy * cy).sqrt() - self.radius)
    }

    /// Offset of the upstream face (positive, upstream of the crest's
    /// centreline) `depth` metres under the crest: a slight batter, flaring
    /// into a toe from just over the waterline to the gorge's floor.
    pub fn upstream_face(&self, depth: f64) -> f64 {
        let d = depth.max(0.0);
        let t = ((d - self.toe_from) / (self.toe_to - self.toe_from)).clamp(0.0, 1.0);
        self.crest_half + self.up_batter * d + self.toe * t * t * (3.0 - 2.0 * t)
    }

    /// Offset of the downstream face (negative, downstream) `depth` metres under
    /// the crest: plumb at the crest, battering out ever more with depth.
    pub fn downstream_face(&self, depth: f64) -> f64 {
        -(self.crest_half + self.down_spread * depth.max(0.0).powf(1.5))
    }

    /// How deep under the crest the upstream face stands `offset` out (the
    /// inverse of [`DamPlan::upstream_face`]); `None` past its footing.
    pub fn upstream_depth(&self, offset: f64) -> Option<f64> {
        self.face_depth(|d| self.upstream_face(d) >= offset)
    }

    /// How deep under the crest the downstream face stands `offset` out
    /// (negative, downstream); `None` past its footing.
    pub fn downstream_depth(&self, offset: f64) -> Option<f64> {
        self.face_depth(|d| self.downstream_face(d) <= offset)
    }

    /// The shallowest depth down to the footing where `reached` holds (the
    /// faces are monotonic in depth), to a centimetre.
    fn face_depth(&self, reached: impl Fn(f64) -> bool) -> Option<f64> {
        let (mut lo, mut hi) = (0.0, self.crest_z + self.bed + self.footing);
        if reached(lo) {
            return Some(0.0);
        }
        if !reached(hi) {
            return None;
        }
        while hi - lo > 0.01 {
            let mid = 0.5 * (lo + hi);
            if reached(mid) {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        Some(hi)
    }

    /// Straight distance between the abutments along the crest's chord.
    pub fn span(&self) -> f64 {
        2.0 * self.radius * self.half_angle.sin()
    }
}

/// The canyon map's dam (`bake/canyon.rs`, `mc-render/src/models/dam.rs`).
pub const DAM: DamPlan = DamPlan {
    radius: 280.0,
    half_angle: 0.733, // 42 degrees: a 375 m chord, the arch bowed 72 m upstream
    crest_half: 15.0,
    road_up: 14.0,
    road_down: 14.0,
    crest_z: 64.0,
    bed: 60.0,
    ring_top: 55.0,
    approach: 90.0,
    up_batter: 0.03,
    toe: 5.0,
    toe_from: 60.0,
    toe_to: 74.0,
    down_spread: 0.0225,
    footing: 12.0,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crest_points_sit_on_the_centreline() {
        for a in [-DAM.half_angle, -0.3, 0.0, 0.2, DAM.half_angle] {
            let (x, y) = DAM.crest_at(a);
            let (back, off) = DAM.arch_coords(x, y);
            assert!((back - a).abs() < 1e-9 && off.abs() < 1e-9);
        }
        const { assert!(DAM.road_up < DAM.crest_half && DAM.road_down < DAM.crest_half) };
        assert!((DAM.span() - 374.7).abs() < 1.0, "{}", DAM.span());
    }

    /// The faces bound the crest band at the top, the toe starts over the
    /// water, and the base is some 70 m thick at the footing.
    #[test]
    fn the_faces_hold_the_band_and_thicken_down() {
        assert_eq!(DAM.upstream_face(0.0), DAM.crest_half);
        assert_eq!(DAM.downstream_face(0.0), -DAM.crest_half);
        const { assert!(DAM.toe_from < DAM.crest_z && DAM.toe_to > DAM.crest_z) };
        let foot = DAM.crest_z + DAM.bed + DAM.footing;
        let base = DAM.upstream_face(foot) - DAM.downstream_face(foot);
        assert!((65.0..80.0).contains(&base), "{base}");
        let mut last = (DAM.upstream_face(0.0), DAM.downstream_face(0.0));
        for d in 1..=foot as i32 {
            let now = (DAM.upstream_face(d as f64), DAM.downstream_face(d as f64));
            assert!(now.0 >= last.0 && now.1 < last.1, "at {d}: {now:?}");
            last = now;
        }
    }
}
