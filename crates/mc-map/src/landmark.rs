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
/// `mc-models/src/dam.rs`).
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
/// towers standing a span apart in a straight line along their heading (+x),
/// and at each tower a span of wires on to the next, pitched about `pivot` to
/// meet the ground the next one stands on. In a tower's frame: y across the
/// line, z up from its foot.
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
    /// The height over a tower's foot its span is pitched about: the middle
    /// of the clamps and peaks, so none of them strays far as it pitches.
    pub pivot: f64,
}

/// A Progenitor hard-light bridge (`PropKind::PrecursorBridge`) as drawn at scale
/// 1: its deck runs from the origin along +x for `span` metres, `half` either side,
/// its walking surface at the origin's height. A map lays the bridge's deck
/// (`crate::deck::Deck`) to it, scaled as the prop is, so the road units walk on is
/// the one drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BridgePlan {
    pub span: f64,
    pub half: f64,
}

pub const HARDLIGHT_BRIDGE: BridgePlan = BridgePlan {
    span: 1_600.0,
    half: 280.0,
};

/// The dam's lines.
pub const GORGE_LINE: PowerLine = PowerLine {
    span: 360.0,
    phases: [(11.0, 30.0), (14.0, 38.0), (11.0, 46.0)],
    earth: (6.0, 56.0),
    sag: 9.0,
    pivot: 43.0,
};

/// A switchyard at the dam's foot: a fenced yard on level ground round the
/// origin, its line leaving along +x. The first tower stands at `first`
/// along +x from the origin, on ground as high as the yard's; the yard's
/// model draws the span to it.
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

/// Frostline's climate wall (`bake/frostline.rs`): the line the Precursors'
/// towers stand on, south edge to north edge, map metres. Desert west of it,
/// Alaska east. The map's sidecar (`maps/frostline.ron`) gives the renderer
/// the same line; `tests/frostline.rs` holds the two together.
pub const FROSTLINE_WALL: &[(f64, f64)] = &[
    (4_992.0, 0.0),
    (4_992.0, 2_400.0),
    (8_192.0, 5_600.0),
    (8_192.0, 10_784.0),
    (11_392.0, 13_984.0),
    (11_392.0, 16_384.0),
];

/// Degrees Frostline's country is turned clockwise about the map's middle
/// from the frame it is designed in (`bake/frostline.rs`), where the two
/// landmasses lie due west and east of each other.
pub const FROSTLINE_TURN_DEG: f64 = 20.0;

/// A point of Frostline's design, as it lies on the 16 384 m map.
pub fn frostline_on_map((x, y): (f64, f64)) -> (f64, f64) {
    let mid = 8_192.0;
    let (s, c) = FROSTLINE_TURN_DEG.to_radians().sin_cos();
    let (dx, dy) = (x - mid, y - mid);
    (mid + dx * c + dy * s, mid - dx * s + dy * c)
}

/// Metres Frostline's desert stands below Vermilion Gorge's beds: its low
/// country, 14 to 50 m over the sea, is the canyon's bench (64 to 100 m), and
/// its mesas are cut in the beds above. The sidecar's `strata_lift`.
pub const FROSTLINE_STRATA_LIFT: f64 = 50.0;

/// Tripoint's climate walls (`bake/tripoint.rs`): three lines out from the
/// Precursor installation in the middle of the 12 288 m map, a third of a turn
/// apart, each to beyond the map's edge: north-east, north-west and south.
/// Map metres. Alaska lies between the first two, the desert between the
/// second and the third, the jungle between the third and the first. The
/// map's sidecar (`maps/tripoint.ron`) gives the renderer the same lines;
/// `tests/tripoint.rs` holds the two together.
pub const TRIPOINT_WALLS: [[(f64, f64); 2]; 3] = [
    [(6_144.0, 6_144.0), (13_072.0, 10_144.0)],
    [(6_144.0, 6_144.0), (-784.0, 10_144.0)],
    [(6_144.0, 6_144.0), (6_144.0, -1_856.0)],
];

/// Tripoint's regions, in the sidecar's order: Alaska, the desert, the jungle.
pub const TRIPOINT_ALASKA: usize = 0;
pub const TRIPOINT_DESERT: usize = 1;
pub const TRIPOINT_JUNGLE: usize = 2;

/// Which of Tripoint's regions a point of the map lies in, by its walls
/// ([`TRIPOINT_WALLS`]): the region between the two walls either side of it.
pub fn tripoint_region((x, y): (f64, f64)) -> usize {
    let [(cx, cy), _] = TRIPOINT_WALLS[0];
    let heading = |w: usize| {
        let [_, (ex, ey)] = TRIPOINT_WALLS[w];
        (ey - cy).atan2(ex - cx)
    };
    let turn = |from: f64, to: f64| (to - from).rem_euclid(std::f64::consts::TAU);
    let at = (y - cy).atan2(x - cx);
    // Counter-clockwise from the north-east wall: Alaska up to the north-west
    // wall, the desert on to the south wall, the jungle back round.
    let a = turn(heading(0), at);
    if a < turn(heading(0), heading(1)) {
        TRIPOINT_ALASKA
    } else if a < turn(heading(0), heading(2)) {
        TRIPOINT_DESERT
    } else {
        TRIPOINT_JUNGLE
    }
}

/// The Last Harvest's regions (`bake/harvest.rs`), in the sidecar's order: the
/// autumn heartland and its valley up to the Progenitor's base, the desert
/// to the north-west, the alpine country to the north-east, the isles to the
/// south-west and the fjords to the south-east.
pub const HARVEST_HEARTLAND: usize = 0;
pub const HARVEST_DESERT: usize = 1;
pub const HARVEST_ALPINE: usize = 2;
pub const HARVEST_ISLES: usize = 3;
pub const HARVEST_FJORDS: usize = 4;
/// How many regions The Last Harvest has.
pub const HARVEST_REGIONS: usize = 5;

/// A climate wall's line, and the regions on its left and right hands.
pub type HarvestWall = (&'static [(f64, f64)], usize, usize);

/// The Last Harvest's climate walls on the 24 576 m map, map metres: each
/// line with the region on its left and on its right hand, walking it from its
/// first point. Two junctions, west and east of the heartland, where three
/// regions meet. The map's sidecar (`maps/last_harvest.ron`) gives the
/// renderer the same lines; `tests/last_harvest.rs` holds the two together.
pub const HARVEST_WALLS: [HarvestWall; 6] = [
    // Desert | heartland: from the north edge down the foot of the desert's
    // mesas, beside the base and the valley, to the west junction.
    (
        &[
            (8_600.0, 24_900.0),
            (8_400.0, 21_800.0),
            (8_900.0, 18_600.0),
            (8_900.0, 15_400.0),
            (7_900.0, 12_500.0),
            HARVEST_WEST_JUNCTION,
        ],
        HARVEST_HEARTLAND,
        HARVEST_DESERT,
    ),
    // Desert | isles: from the west edge along the foot of the desert's sea cliffs.
    (
        &[
            (-300.0, 11_500.0),
            (1_800.0, 11_000.0),
            (3_900.0, 11_400.0),
            HARVEST_WEST_JUNCTION,
        ],
        HARVEST_DESERT,
        HARVEST_ISLES,
    ),
    // Isles | heartland: just off the heartland's west coast, to the south edge.
    (
        &[
            HARVEST_WEST_JUNCTION,
            (6_600.0, 9_500.0),
            (5_700.0, 8_100.0),
            (5_900.0, 5_800.0),
            (5_400.0, 4_600.0),
            (6_600.0, 2_500.0),
            (6_400.0, -300.0),
        ],
        HARVEST_HEARTLAND,
        HARVEST_ISLES,
    ),
    // Alpine | heartland: from the north edge along the alpine foothills
    // beside the base and the valley, to the east junction.
    (
        &[
            (16_000.0, 24_900.0),
            (16_200.0, 21_700.0),
            (15_800.0, 18_200.0),
            (15_600.0, 15_000.0),
            (16_600.0, 12_600.0),
            HARVEST_EAST_JUNCTION,
        ],
        HARVEST_ALPINE,
        HARVEST_HEARTLAND,
    ),
    // Alpine | fjords: from the east junction over the ranges to the east edge.
    (
        &[
            HARVEST_EAST_JUNCTION,
            (20_000.0, 11_600.0),
            (22_300.0, 11_100.0),
            (24_900.0, 11_500.0),
        ],
        HARVEST_ALPINE,
        HARVEST_FJORDS,
    ),
    // Fjords | heartland: over the tundra west of the sound, which the cold
    // creeps into from the plateau (the bake's snow comes in by degrees).
    (
        &[
            HARVEST_EAST_JUNCTION,
            (17_300.0, 9_200.0),
            (16_900.0, 7_000.0),
            (17_300.0, 4_800.0),
            (17_000.0, 2_600.0),
            (17_200.0, -300.0),
        ],
        HARVEST_FJORDS,
        HARVEST_HEARTLAND,
    ),
];
/// Where the desert, the isles and the heartland meet; where the alpine
/// country, the fjords and the heartland meet.
pub const HARVEST_WEST_JUNCTION: (f64, f64) = (6_400.0, 11_200.0);
pub const HARVEST_EAST_JUNCTION: (f64, f64) = (18_300.0, 10_900.0);

/// Which of The Last Harvest's regions a point of the map lies in: the one on
/// its side of the nearest wall segment ([`HARVEST_WALLS`]), by the rule the
/// renderer reads the sidecar's walls by (`mc_data::regions::Walls::probe`):
/// where the nearest point of two segments is the end they share, the one
/// whose line, carried on, the point is furthest from decides.
pub fn harvest_region(p: (f64, f64)) -> usize {
    harvest_probe(p, None).0
}

/// Metres The Last Harvest's regions merge over either side of a wall (the
/// sidecar's `blend`): no lit wall, the look handing over along a wandering line.
pub const HARVEST_BLEND: f64 = 500.0;

/// Where a point's look (its climate's colour, its trees, the relief its region
/// raises) reads The Last Harvest's walls: moved by the renderer's own wander
/// (`mc_data::regions::wander`), so the ground's shape and its colour change
/// together, along no ruled line. [`harvest_region`] of the point itself is the
/// sidecar's region, as the sky and the set-up read it.
pub fn harvest_look(p: (f64, f64)) -> (f64, f64) {
    let (x, y) = mc_data::regions::wander(p.0 as f32, p.1 as f32, HARVEST_BLEND as f32);
    (x as f64, y as f64)
}

/// Metres a point lies inside one of The Last Harvest's regions (negative
/// outside it): to the nearest wall that bounds that region.
pub fn harvest_depth(p: (f64, f64), region: usize) -> f64 {
    let (at, d) = harvest_probe(p, Some(region));
    if at == region {
        d
    } else {
        -d
    }
}

/// The region a point is in, and its distance to the nearest wall (of those
/// bounding `region`, if one is given).
fn harvest_probe((x, y): (f64, f64), region: Option<usize>) -> (usize, f64) {
    let mut near = f64::INFINITY;
    let mut best = (f64::INFINITY, -1.0, true, HARVEST_HEARTLAND);
    let region_of = |l: usize, r: usize| region.is_none_or(|g| g == l || g == r);
    for &(line, left, right) in &HARVEST_WALLS {
        for w in line.windows(2) {
            let ((ax, ay), (bx, by)) = (w[0], w[1]);
            let (ex, ey) = (bx - ax, by - ay);
            let (px, py) = (x - ax, y - ay);
            let len2 = ex * ex + ey * ey;
            let t = (px * ex + py * ey) / len2;
            let end = !(0.0..1.0).contains(&t) || t == 0.0;
            let tc = t.clamp(0.0, 1.0);
            let d = (px - ex * tc).hypot(py - ey * tc);
            let cross = ex * py - ey * px;
            let off_line = cross.abs() / len2.sqrt();
            let side = if cross > 0.0 { left } else { right };
            let (bd, boff, bend, _) = best;
            let margin = match (end, bend) {
                (true, false) => -0.05,
                (false, true) => 0.05,
                _ => 0.0,
            };
            let shared = end && bend && d == bd;
            if d < bd + margin || (shared && off_line > boff) {
                best = (d, off_line, end, side);
            }
            if region_of(left, right) {
                near = near.min(d);
            }
        }
    }
    (best.3, if region.is_some() { near } else { best.0 })
}

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

    #[test]
    fn tripoints_walls_part_its_three_regions() {
        let mid = 6_144.0;
        let at = |deg: f64| {
            let (s, c) = deg.to_radians().sin_cos();
            (mid + 3_000.0 * c, mid + 3_000.0 * s)
        };
        assert_eq!(tripoint_region(at(90.0)), TRIPOINT_ALASKA);
        assert_eq!(tripoint_region(at(210.0)), TRIPOINT_DESERT);
        assert_eq!(tripoint_region(at(330.0)), TRIPOINT_JUNGLE);
        // Either side of each wall, a few metres off it.
        for (deg, left, right) in [
            (30.0, TRIPOINT_ALASKA, TRIPOINT_JUNGLE),
            (150.0, TRIPOINT_DESERT, TRIPOINT_ALASKA),
            (270.0, TRIPOINT_JUNGLE, TRIPOINT_DESERT),
        ] {
            assert_eq!(tripoint_region(at(deg + 0.2)), left);
            assert_eq!(tripoint_region(at(deg - 0.2)), right);
        }
    }
}
