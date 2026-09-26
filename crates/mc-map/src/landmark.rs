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

/// A power line, after the Three Gorges' 500 kV double-circuit lines: lattice
/// towers standing a span apart in a straight line, each carrying the span on
/// along its heading (+x) to the next. In a tower's frame: y across the line,
/// z up from its foot, which the bake levels at the line's one height, so
/// every tower's clamps meet the last one's wires.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PowerLine {
    /// Tower to tower along the line.
    pub span: f64,
    /// Each circuit's three phases, low to high: (y out from the line's axis,
    /// z over the foot) of the clamp its wire hangs from, one circuit either
    /// side.
    pub phases: [(f64, f64); 3],
    /// The two earth wires over the tower's peaks, (y, z).
    pub earth: (f64, f64),
    /// How far a wire hangs below its clamps at mid-span.
    pub sag: f64,
}

/// The dam's lines.
pub const GORGE_LINE: PowerLine = PowerLine {
    span: 360.0,
    phases: [(11.0, 30.0), (14.0, 38.0), (11.0, 46.0)],
    earth: (6.0, 56.0),
    sag: 9.0,
};

/// A switchyard at the dam's foot: a fenced yard on level ground round the
/// origin, its line leaving along +x. The first tower stands at `first`
/// along +x from the origin; the yard's model draws the span to it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Switchyard {
    /// The fence's half lengths along x and y.
    pub half: (f64, f64),
    /// The line's first tower, along +x from the origin.
    pub first: f64,
}

pub const GORGE_YARD: Switchyard = Switchyard {
    half: (120.0, 75.0),
    first: 120.0 + 0.5 * GORGE_LINE.span,
};

/// The dam's operations town: a level lot round the origin, these half
/// lengths along x and y, streets through it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Town {
    pub half: (f64, f64),
}

pub const GORGE_TOWN: Town = Town {
    half: (170.0, 120.0),
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
