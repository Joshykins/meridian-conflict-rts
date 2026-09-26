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
    /// the model's crest covers that band with a margin. Units walk it.
    pub road_up: f64,
    pub road_down: f64,
    /// The crest road's height above the water level.
    pub crest_z: f64,
    /// Depth of the gorge's bed under the dam, below the water level.
    pub bed: f64,
    /// Length of level ground at crest height the baker leaves beyond each
    /// abutment, along the arc's tangent, for the road off the dam.
    pub approach: f64,
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
    road_up: 8.0,
    road_down: 12.0,
    crest_z: 64.0,
    bed: 60.0,
    approach: 90.0,
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
}
