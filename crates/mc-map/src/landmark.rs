//! Landmarks: great works of the colonists that belong to a map
//! ([`PropKind::Dam`](crate::PropKind::Dam)). The baker cuts the ground for one
//! from these numbers and the renderer's model is built to the same numbers,
//! so the two meet.

/// A straight concrete gravity dam, in the prop's own frame: x along its
/// heading (upstream, into the lake), y along the crest, z up from the dry
/// riverbed at its toe. The origin is the middle of the downstream toe. All
/// lengths in metres. The dam is scenery: nothing walks its crest.
///
/// In section the upstream face stands plumb at `x = base`, from the crest
/// down past the waterline to the lake's bed; the downstream face rises in
/// a straight batter from the toe at `x = 0` to the crest's downstream edge
/// at `x = base - crest_width`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GravityDam {
    /// The crest's length, centred on the origin.
    pub length: f64,
    /// How far each end runs on into the canyon's walls past `length / 2`.
    pub key: f64,
    /// The dry riverbed at the toe, above the water level: the origin.
    pub floor_z: f64,
    /// The crest, above the water level.
    pub crest_z: f64,
    /// The footprint's depth along x, toe to heel.
    pub base: f64,
    pub crest_width: f64,
    /// The lake's bed against the upstream face, below the water level.
    pub bed: f64,
    /// Height above the water of the lake's old full pool: the top of the
    /// white mineral ring on the canyon's walls and the dam's upstream face.
    pub ring_top: f64,
}

impl GravityDam {
    /// The crest's height over the toe (the model's top).
    pub fn height(&self) -> f64 {
        self.crest_z - self.floor_z
    }

    /// Where the downstream face stands (x) at `z` over the toe.
    pub fn downstream_x(&self, z: f64) -> f64 {
        (self.base - self.crest_width) * (z / self.height()).clamp(0.0, 1.0)
    }

    /// The dam's top surface over a point of its plan, in its frame (z over
    /// the toe); `None` off the dam.
    pub fn surface(&self, x: f64, y: f64) -> Option<f64> {
        if y.abs() > self.length / 2.0 + self.key || !(0.0..=self.base).contains(&x) {
            return None;
        }
        let run = self.base - self.crest_width;
        Some(if x >= run {
            self.height()
        } else {
            self.height() * x / run
        })
    }
}

/// The canyon map's dam, after the Three Gorges (`bake/canyon.rs`,
/// `mc-render/src/models/dam.rs`).
pub const GORGE_DAM: GravityDam = GravityDam {
    length: 1_400.0,
    key: 60.0,
    floor_z: 64.0,
    crest_z: 230.0,
    base: 150.0,
    crest_width: 36.0,
    bed: 45.0,
    ring_top: 55.0,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_gravity_dam_rises_from_its_toe_to_its_crest() {
        let d = GORGE_DAM;
        assert_eq!(d.surface(0.0, 0.0), Some(0.0));
        assert_eq!(d.surface(d.base, d.length / 2.0), Some(d.height()));
        assert_eq!(d.surface(-1.0, 0.0), None);
        assert!((d.downstream_x(d.height()) - (d.base - d.crest_width)).abs() < 1e-9);
        const { assert!(GORGE_DAM.floor_z > 0.0 && GORGE_DAM.crest_z > GORGE_DAM.ring_top) };
    }
}
