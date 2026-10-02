//! A map's regions: stretches of it with a climate and a weather of their own, parted
//! by climate walls (`maps/<stem>.ron`, see `crate::weather`). Here is where the walls
//! run and which region a point lies in; the shaders do the same sums
//! (mc-render `shaders/regions.wgsl`), so the ground, the sea, the sky and the map
//! previews agree on every line.

use serde::Deserialize;

use crate::weather::{Climate, Weather, WeatherPreset, WeatherTweaks};

/// The most regions a map may have (the shaders hold this many: mc-models
/// `gpu_consts::regions::MAX`).
pub const MAX_REGIONS: usize = 8;

/// One region of a map: its name, its climate and its weather.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Region {
    /// What skirmish set-up calls it ("Weather: Alaska"). Empty on a map without regions.
    pub name: String,
    pub climate: Climate,
    pub weather: WeatherPreset,
    pub tweaks: WeatherTweaks,
}

impl Region {
    /// The region's own weather: its preset with its tweaks.
    pub fn weather(&self) -> Weather {
        self.tweaks.apply(Weather::from(self.weather))
    }
}

/// A climate wall: a line between two regions.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Wall {
    /// The line in map metres (x, y; y north), two points or more. It ends on or
    /// beyond the map's edge, or on another wall.
    pub line: Vec<(f32, f32)>,
    /// The region on the left hand walking the line from its first point to its
    /// last (west of a line that runs south to north), and the one on the right.
    pub left: usize,
    pub right: usize,
}

/// One straight stretch of a wall.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Segment {
    pub a: (f32, f32),
    pub b: (f32, f32),
    /// The regions on its left and right hand, walking from `a` to `b`.
    pub left: usize,
    pub right: usize,
    /// Metres along its wall at `a`, from the wall's first point.
    pub along: f32,
}

impl Segment {
    pub fn length(&self) -> f32 {
        (self.b.0 - self.a.0).hypot(self.b.1 - self.a.1)
    }

    /// Where (`x`, `y`) lies by the segment.
    fn reach(&self, x: f32, y: f32) -> Reach {
        let (ex, ey) = (self.b.0 - self.a.0, self.b.1 - self.a.1);
        let (px, py) = (x - self.a.0, y - self.a.1);
        let length = ex.hypot(ey);
        let t = (px * ex + py * ey) / (ex * ex + ey * ey);
        // Past an end the nearest point is that end as the file wrote it, so two
        // segments that share it measure the very same distance.
        let (end, near) = if t <= 0.0 {
            (true, self.a)
        } else if t >= 1.0 {
            (true, self.b)
        } else {
            (false, (self.a.0 + ex * t, self.a.1 + ey * t))
        };
        let cross = ex * py - ey * px;
        Reach {
            distance: (x - near.0).hypot(y - near.1),
            t: t.clamp(0.0, 1.0),
            end,
            off_line: cross.abs() / length,
            left: cross > 0.0,
        }
    }
}

struct Reach {
    /// Metres from the point to the segment.
    distance: f32,
    /// How far along the segment its nearest point lies, 0 at `a` to 1 at `b`.
    t: f32,
    /// The nearest point is one of the segment's ends.
    end: bool,
    /// Metres from the point to the segment's line carried on past its ends.
    off_line: f32,
    /// The point is on the left hand of that line.
    left: bool,
}

/// Why a map's regions or walls cannot be used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegionError {
    /// `regions` lists this many: fewer than 2 or more than `MAX_REGIONS`.
    Regions(usize),
    /// The file has `regions` and this top-level field as well; with regions,
    /// each region says its own.
    OwnAndRegions(&'static str),
    /// The file has walls and no regions for them to part.
    WallsWithoutRegions,
    /// This wall's line has this many points: fewer than 2.
    Points { wall: usize, points: usize },
    /// This point of this wall is not a number.
    NotFinite { wall: usize, point: usize },
    /// This point of this wall is where the one before it is.
    Repeated { wall: usize, point: usize },
    /// This wall has the same region on both hands.
    SameSides { wall: usize },
    /// This wall names a region the map does not have.
    NoRegion { wall: usize, region: usize },
    /// The walls have this many segments between them: more than `Walls::MAX_SEGMENTS`.
    Segments(usize),
}

impl std::fmt::Display for RegionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            RegionError::Regions(n) => {
                write!(f, "a map has 2 to {MAX_REGIONS} regions, not {n}")
            }
            RegionError::OwnAndRegions(field) => write!(
                f,
                "a map with regions has no `{field}` of its own: each region says its own"
            ),
            RegionError::WallsWithoutRegions => {
                write!(f, "walls with no regions for them to part")
            }
            RegionError::Points { wall, points } => {
                write!(f, "wall {wall}: a line has 2 points or more, not {points}")
            }
            RegionError::NotFinite { wall, point } => {
                write!(f, "wall {wall}: point {point} is not a number")
            }
            RegionError::Repeated { wall, point } => write!(
                f,
                "wall {wall}: point {point} is where point {} is",
                point - 1
            ),
            RegionError::SameSides { wall } => {
                write!(f, "wall {wall}: the same region on its left and its right")
            }
            RegionError::NoRegion { wall, region } => {
                write!(f, "wall {wall}: the map has no region {region}")
            }
            RegionError::Segments(n) => write!(
                f,
                "the walls have {n} segments between them, more than {}",
                Walls::MAX_SEGMENTS
            ),
        }
    }
}

impl std::error::Error for RegionError {}

/// Where a point lies among a map's walls (`Walls::probe`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WallProbe {
    /// The region the point is in.
    pub region: usize,
    /// Metres to the nearest wall; infinite on a map without walls.
    pub wall: f32,
    /// Metres along that wall, from its first point, of the wall's nearest point.
    pub along: f32,
}

/// A map's climate walls, checked: which region every point of the map lies in.
/// None on a map without regions, which is all region 0.
#[derive(Clone, Debug, PartialEq)]
pub struct Walls {
    walls: Vec<Wall>,
    segments: Vec<Segment>,
    /// How many regions the walls lie between: 1 with no walls.
    regions: usize,
}

impl Default for Walls {
    fn default() -> Walls {
        Walls {
            walls: Vec::new(),
            segments: Vec::new(),
            regions: 1,
        }
    }
}

impl Walls {
    /// The most segments a map's walls may have between them (the shaders hold
    /// this many: mc-models `gpu_consts::regions::WALL_SEGMENTS`).
    pub const MAX_SEGMENTS: usize = 32;

    /// A segment a point lies beside is taken as nearer than another segment's end
    /// that is nearer by less than this many metres (`probe`).
    pub const TIE_M: f32 = 0.05;

    /// `walls` between `regions` regions, if they can be used: each a line of two
    /// or more different points, between two different regions the map has, and no
    /// more than `MAX_SEGMENTS` segments in all.
    pub fn new(walls: Vec<Wall>, regions: usize) -> Result<Walls, RegionError> {
        let mut segments = Vec::new();
        let mut count = 0;
        for (wall, w) in walls.iter().enumerate() {
            if w.line.len() < 2 {
                return Err(RegionError::Points {
                    wall,
                    points: w.line.len(),
                });
            }
            for (point, p) in w.line.iter().enumerate() {
                if !(p.0.is_finite() && p.1.is_finite()) {
                    return Err(RegionError::NotFinite { wall, point });
                }
                if point > 0 && *p == w.line[point - 1] {
                    return Err(RegionError::Repeated { wall, point });
                }
            }
            if w.left == w.right {
                return Err(RegionError::SameSides { wall });
            }
            if let Some(region) = [w.left, w.right].into_iter().find(|r| *r >= regions) {
                return Err(RegionError::NoRegion { wall, region });
            }
            count += w.line.len() - 1;
            let mut along = 0.0;
            for pair in w.line.windows(2) {
                let segment = Segment {
                    a: pair[0],
                    b: pair[1],
                    left: w.left,
                    right: w.right,
                    along,
                };
                along += segment.length();
                segments.push(segment);
            }
        }
        if count > Walls::MAX_SEGMENTS {
            return Err(RegionError::Segments(count));
        }
        Ok(Walls {
            walls,
            segments,
            regions: regions.max(1),
        })
    }

    /// The walls as the map's file gives them.
    pub fn walls(&self) -> &[Wall] {
        &self.walls
    }

    /// Every wall's segments, in the walls' order.
    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    /// How many regions the map has: 1 without walls.
    pub fn regions(&self) -> usize {
        self.regions
    }

    /// Whether the map has no walls: all of it is region 0.
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    /// Where (`x`, `y`) lies: the region on its side of the nearest wall segment,
    /// how far that wall is and how far along it. A point on a wall is on its right.
    /// (shaders/regions.wgsl `region_probe` is the same.)
    ///
    /// Where the nearest point of several segments is the one end they share (a
    /// corner, or walls meeting), the segment whose line, carried on past its ends,
    /// the point is furthest from decides. A segment the point lies beside (its
    /// nearest point not an end) counts as nearer than another's end within
    /// `TIE_M`: where a wall ends on the middle of another, the two are as near
    /// all along a line across the far region, and rounding must not pick.
    pub fn probe(&self, x: f32, y: f32) -> WallProbe {
        let mut probe = WallProbe {
            region: 0,
            wall: f32::INFINITY,
            along: 0.0,
        };
        let mut end = true;
        let mut off_line = -1.0;
        for s in &self.segments {
            let reach = s.reach(x, y);
            let margin = match (reach.end, end) {
                (true, false) => -Walls::TIE_M,
                (false, true) => Walls::TIE_M,
                _ => 0.0,
            };
            let shared_end = reach.end && end && reach.distance == probe.wall;
            if reach.distance < probe.wall + margin || (shared_end && reach.off_line > off_line) {
                probe.wall = reach.distance;
                probe.region = if reach.left { s.left } else { s.right };
                probe.along = s.along + reach.t * s.length();
                end = reach.end;
                off_line = reach.off_line;
            }
        }
        probe
    }

    /// The region (`x`, `y`) is in: 0 on a map without walls.
    pub fn region_at(&self, x: f32, y: f32) -> usize {
        self.probe(x, y).region
    }

    /// Metres from (`x`, `y`) to the nearest wall; infinite on a map without walls.
    pub fn wall_distance(&self, x: f32, y: f32) -> f32 {
        self.probe(x, y).wall
    }

    /// How much of each region's look (`x`, `y`) takes, summing to 1: all of its own
    /// region's `half` metres or more from every wall, and across a wall the two
    /// sides hand over within `half` metres either side of it, smoothly through
    /// the places where walls meet. (shaders/regions.wgsl `region_shares`.)
    ///
    /// A region's weight falls from 1 to 0 as the point goes from `half` metres
    /// inside it to `half` metres outside it, where "outside" is the distance to
    /// the nearest wall the region lies along; the weights are then scaled to sum to 1.
    pub fn weights(&self, x: f32, y: f32, half: f32) -> [f32; MAX_REGIONS] {
        let mut weights = [0.0; MAX_REGIONS];
        let probe = self.probe(x, y);
        if probe.wall >= half {
            weights[probe.region] = 1.0;
            return weights;
        }
        // Metres outside each region: to the nearest wall it lies along.
        let mut outside = [f32::INFINITY; MAX_REGIONS];
        for s in &self.segments {
            let d = s.reach(x, y).distance;
            outside[s.left] = outside[s.left].min(d);
            outside[s.right] = outside[s.right].min(d);
        }
        outside[probe.region] = -probe.wall;
        let mut sum = 0.0;
        for (w, s) in weights.iter_mut().zip(outside) {
            *w = 1.0 - smoothstep(-half, half, s);
            sum += *w;
        }
        weights.map(|w| w / sum)
    }

    /// The share of a `width` by `height` metre map each region covers.
    pub fn shares(&self, width: f32, height: f32) -> [f32; MAX_REGIONS] {
        const GRID: usize = 64;
        let mut shares = [0.0; MAX_REGIONS];
        if self.is_empty() {
            shares[0] = 1.0;
            return shares;
        }
        let cell = 1.0 / (GRID * GRID) as f32;
        for j in 0..GRID {
            for i in 0..GRID {
                let x = (i as f32 + 0.5) / GRID as f32 * width;
                let y = (j as f32 + 0.5) / GRID as f32 * height;
                shares[self.region_at(x, y)] += cell;
            }
        }
        shares
    }
}

/// 0 at or below `a`, 1 at or above `b`, eased between (as the shaders' `smoothstep`).
fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests;
