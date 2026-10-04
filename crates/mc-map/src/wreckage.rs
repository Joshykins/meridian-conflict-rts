//! Wreckage a map starts with: the salvage of fighting before the match.
//!
//! Every non-survival map is stamped with it after baking (`mc-bake`, or
//! `mc-bake --wreckage-only` on a map already baked, which keeps its terrain).
//! The fields are laid out in one symmetric sector of the map and copied to
//! every other sector by the layout's symmetry, so each player finds the same
//! salvage at the same distance; a wreck is kept only where the ground suits it
//! in every copy.
//!
//! What goes where:
//! - a couple of small scrap piles a few hundred metres out from each start,
//!   early mass for the first engineers;
//! - skirmish, clash and heavy fields in open ground away from the starts, the
//!   heavier ones where the players' ground meets (a heavy field can hold a T3
//!   hulk);
//! - ship hulks lying on the seabed, listing, where the map has deep enough water.
//!
//! Wrecks are named by blueprint key; the tiers are budgeted by rough weight
//! (T1 1, T2 4, T3 16) since the map does not know unit costs. The mass each
//! holds comes from the blueprints when the match starts, less weathering.

use crate::format::{MapError, MapWreck, MapWriter, Prop};
use crate::heightfield::Heightfield;
use crate::noise::{hash2, unit};
use crate::{encode_tile, Layout, MapFile, CELL_SIZE_M};
use mc_core::{Angle, Fx, FxVec2};
use std::collections::HashSet;
use std::f64::consts::{PI, TAU};
use std::path::Path;

/// How a layout is fair: the copies of one sector that make up the map.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Symmetry {
    /// `count` wedge pairs round the centre, the first start's axis at `base`
    /// (the basin and islands layouts, `bake.rs` `Terrain::fold`).
    Wedges { count: u32, base: f64 },
    /// A mirror across the middle: the north half folds onto the south.
    Mirror,
    /// A mirror across the north-south middle line: the east half folds onto the west.
    MirrorSides,
    /// A half turn: the north-east half folds onto the south-west.
    HalfTurn,
    /// Quarter turns: every quarter folds onto the south-west one.
    QuarterTurn,
    /// Thirds of a turn: the south-west and south-east thirds turn onto the
    /// north one, which lies between 30 and 150 degrees.
    Thirds,
}

impl Symmetry {
    /// The symmetry a layout was baked with, for `players` starts. None for
    /// the survival layouts: their economy is the waves' wreckage.
    pub fn of(layout: Layout, players: u32) -> Option<Symmetry> {
        match layout {
            Layout::Basin | Layout::Islands => Some(Symmetry::Wedges {
                count: players.max(1),
                base: if players <= 4 { PI / 4.0 } else { PI / 8.0 },
            }),
            Layout::Alpine | Layout::AlpineTeams => Some(Symmetry::Mirror),
            Layout::TwinBays | Layout::Archipelago | Layout::Frostline => Some(Symmetry::HalfTurn),
            Layout::Canyon => Some(Symmetry::MirrorSides),
            Layout::Crosswater => Some(Symmetry::QuarterTurn),
            Layout::Tripoint => Some(Symmetry::Thirds),
            // The siege lays its own wreckage with its plan.
            Layout::Threshold | Layout::Siege => None,
        }
    }

    /// The maps from the canonical sector to each copy; the first is the identity.
    fn images(self) -> Vec<Image> {
        match self {
            Symmetry::Wedges { count, base } => {
                let wedge = TAU / count as f64;
                (0..count)
                    .flat_map(|k| {
                        [None, Some(base)].map(|line| Image {
                            line,
                            rotate: k as f64 * wedge,
                        })
                    })
                    .collect()
            }
            Symmetry::Mirror => vec![
                Image::IDENTITY,
                Image {
                    line: Some(0.0),
                    rotate: 0.0,
                },
            ],
            Symmetry::MirrorSides => vec![
                Image::IDENTITY,
                Image {
                    line: Some(PI / 2.0),
                    rotate: 0.0,
                },
            ],
            Symmetry::HalfTurn => vec![
                Image::IDENTITY,
                Image {
                    line: None,
                    rotate: PI,
                },
            ],
            Symmetry::QuarterTurn => (0..4)
                .map(|k| Image {
                    line: None,
                    rotate: k as f64 * PI / 2.0,
                })
                .collect(),
            Symmetry::Thirds => (0..3)
                .map(|k| Image {
                    line: None,
                    rotate: k as f64 * TAU / 3.0,
                })
                .collect(),
        }
    }

    /// Whether `v` (from the map centre) lies in the canonical sector.
    fn canonical(self, v: (f64, f64)) -> bool {
        match self {
            Symmetry::Wedges { count, base } => {
                (v.1.atan2(v.0) - base).rem_euclid(TAU) < TAU / count as f64 / 2.0
            }
            Symmetry::Mirror => v.1 <= 0.0,
            Symmetry::MirrorSides => v.0 <= 0.0,
            Symmetry::HalfTurn => v.0 + v.1 <= 0.0,
            Symmetry::QuarterTurn => v.0 < 0.0 && v.1 <= 0.0,
            Symmetry::Thirds => (v.1.atan2(v.0) - PI / 6.0).rem_euclid(TAU) < TAU / 3.0,
        }
    }
}

/// A reflection across the line through the centre at angle `line`, if any,
/// then a turn by `rotate`.
#[derive(Clone, Copy, Debug)]
struct Image {
    line: Option<f64>,
    rotate: f64,
}

impl Image {
    const IDENTITY: Image = Image {
        line: None,
        rotate: 0.0,
    };

    fn angle(self, a: f64) -> f64 {
        self.line.map_or(a, |b| 2.0 * b - a) + self.rotate
    }

    fn vector(self, (x, y): (f64, f64)) -> (f64, f64) {
        let (x, y) = match self.line {
            Some(b) => {
                let (s, c) = (2.0 * b).sin_cos();
                (x * c + y * s, x * s - y * c)
            }
            None => (x, y),
        };
        let (s, c) = self.rotate.sin_cos();
        (x * c - y * s, x * s + y * c)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Ground {
    Land,
    Sea,
    /// Crashed aircraft: on land or on the seabed.
    Any,
}

/// One kind of wreck the fields draw on.
#[derive(Clone, Copy, Debug)]
struct Kind {
    key: &'static str,
    /// Footprint radius, metres, for spacing and ground checks.
    radius: f64,
    /// Budget weight: T1 1, T2 4, T3 16.
    weight: u32,
    ground: Ground,
}

const fn kind(key: &'static str, radius: f64, weight: u32, ground: Ground) -> Kind {
    Kind {
        key,
        radius,
        weight,
        ground,
    }
}

const SCRAP: [Kind; 4] = [
    kind("aster_t1_scout", 3.0, 1, Ground::Land),
    kind("aster_t1_bot", 3.0, 1, Ground::Land),
    kind("aster_t1_tank", 5.0, 1, Ground::Land),
    kind("aster_t1_engineer", 4.0, 1, Ground::Land),
];
const T1_LAND: [Kind; 6] = [
    kind("aster_t1_bot", 3.0, 1, Ground::Land),
    kind("aster_t1_tank", 5.0, 1, Ground::Land),
    kind("aster_t1_tank", 5.0, 1, Ground::Land),
    kind("aster_t1_artillery", 5.0, 1, Ground::Land),
    kind("aster_t1_mobile_aa", 4.0, 1, Ground::Land),
    kind("aster_t1_scout", 3.0, 1, Ground::Land),
];
const T2_LAND: [Kind; 5] = [
    kind("aster_t2_tank", 7.0, 4, Ground::Land),
    kind("aster_t2_tank", 7.0, 4, Ground::Land),
    kind("aster_t2_hover", 6.0, 4, Ground::Land),
    kind("aster_t2_missile", 6.0, 4, Ground::Land),
    kind("aster_t2_mobile_aa", 6.0, 4, Ground::Land),
];
const T3_LAND: [Kind; 3] = [
    kind("aster_t3_assault_bot", 14.0, 16, Ground::Land),
    kind("aster_t3_artillery", 11.0, 16, Ground::Land),
    kind("aster_t3_sniper", 9.0, 16, Ground::Land),
];
const AIR: [Kind; 4] = [
    kind("aster_t1_interceptor", 8.0, 1, Ground::Any),
    kind("aster_t1_bomber", 9.0, 1, Ground::Any),
    kind("aster_t1_rotor_gunship", 8.0, 1, Ground::Any),
    kind("aster_t2_gunship", 10.0, 4, Ground::Any),
];
const T1_SEA: [Kind; 3] = [
    kind("aster_t1_attack_boat", 7.0, 1, Ground::Sea),
    kind("aster_t1_frigate", 15.0, 1, Ground::Sea),
    kind("aster_t1_submarine", 10.0, 1, Ground::Sea),
];
const T2_SEA: [Kind; 3] = [
    kind("aster_t2_destroyer", 22.0, 4, Ground::Sea),
    kind("aster_t2_aa_cruiser", 22.0, 4, Ground::Sea),
    kind("aster_t2_missile_ship", 20.0, 4, Ground::Sea),
];

/// Every key the fields can name, for checking against the blueprints.
pub fn wreck_keys() -> Vec<&'static str> {
    let mut keys: Vec<&str> = [
        &SCRAP[..],
        &T1_LAND,
        &T2_LAND,
        &T3_LAND,
        &AIR,
        &T1_SEA,
        &T2_SEA,
    ]
    .iter()
    .flat_map(|list| list.iter().map(|k| k.key))
    .collect();
    keys.sort_unstable();
    keys.dedup();
    keys
}

/// What a stamp laid down.
#[derive(Clone, Debug, Default)]
pub struct WreckageReport {
    pub content_id: u64,
    pub wrecks: usize,
    pub scrap_piles: usize,
    pub fields: usize,
    pub sea_fields: usize,
    /// Budget weight laid per player (T1 1, T2 4, T3 16).
    pub weight_per_player: f64,
    pub trees_cleared: usize,
}

/// Lays wreckage on the map at `path`, replacing any it had, and rewrites the
/// file in place. The terrain, props (less the trees the wrecks lie on),
/// starts, ore and snow are kept as they are.
pub fn stamp(path: &Path, symmetry: Symmetry, seed: u64) -> Result<WreckageReport, MapError> {
    let file = MapFile::open(path)?;
    let terrain = Heightfield::load(&file)?;
    let mut report = WreckageReport::default();
    let wrecks = Planner::new(&file, &terrain, symmetry, seed).plan(&mut report);

    // The trees and small rocks under a wreck are gone.
    let mut props: Vec<Prop> = Vec::with_capacity(file.props().len());
    let cleared = |p: &Prop| {
        (p.kind.is_tree() || p.kind == crate::PropKind::RockSmall)
            && wrecks.iter().any(|(w, r)| {
                let d = w.pos - p.pos;
                let reach = fx(r + 4.0);
                d.x.abs() < reach && d.y.abs() < reach && d.length() < reach
            })
    };
    for p in file.props() {
        if cleared(p) {
            report.trees_cleared += 1;
        } else {
            props.push(*p);
        }
    }
    report.wrecks = wrecks.len();

    let tmp = path.with_extension("mcmap.stamping");
    let mut writer = MapWriter::create(&tmp, file.info().clone())?;
    let (tw, th) = file.size_tiles();
    for ty in 0..th {
        for tx in 0..tw {
            writer.push_tile(&encode_tile(&file.read_tile(tx, ty)?))?;
        }
    }
    writer.keep_layers(&file)?;
    writer.set_wrecks(wrecks.into_iter().map(|(w, _)| w).collect())?;
    let starts = file.start_positions().to_vec();
    let ore = file.ore_regions().to_vec();
    drop(file);
    report.content_id = writer.finish(props, &starts, &ore)?;
    std::fs::rename(&tmp, path)?;
    Ok(report)
}

struct Planner<'a> {
    terrain: &'a Heightfield,
    file: &'a MapFile,
    symmetry: Symmetry,
    images: Vec<Image>,
    seed: u64,
    size: (f64, f64),
    centre: (f64, f64),
    water: f64,
    starts: Vec<(f64, f64)>,
    /// Trees per 64 m cell.
    trees: Vec<u16>,
    tree_dims: (usize, usize),
    /// Town buildings, as points.
    buildings: Vec<(f64, f64)>,
    /// Path cells under a Precursor artifact's solid parts.
    solid: HashSet<(u32, u32)>,
    /// Land an army can drive to from a start, per `REACH_CELL` cell.
    reach: Vec<bool>,
    /// Water within `COAST_REACH` of that land, where the fleets fought.
    coastal: Vec<bool>,
    reach_dims: (usize, usize),
    /// Every wreck laid so far, in every copy: centre and radius.
    laid: Vec<((f64, f64), f64)>,
    /// Every field and pile centre laid so far, in every copy.
    sites: Vec<(f64, f64)>,
}

const TREE_CELL: f64 = 64.0;
const REACH_CELL: f64 = 32.0;
const COAST_REACH: f64 = 1500.0;

impl<'a> Planner<'a> {
    fn new(file: &'a MapFile, terrain: &'a Heightfield, symmetry: Symmetry, seed: u64) -> Self {
        let s = terrain.size_metres();
        let size = (s.x.to_f64(), s.y.to_f64());
        let tree_dims = (
            (size.0 / TREE_CELL) as usize + 1,
            (size.1 / TREE_CELL) as usize + 1,
        );
        let mut trees = vec![0u16; tree_dims.0 * tree_dims.1];
        let mut buildings = Vec::new();
        let mut solid = HashSet::new();
        let cells = file.info().size_cells();
        for p in file.props() {
            let (x, y) = (p.pos.x.to_f64(), p.pos.y.to_f64());
            if p.kind.is_tree() {
                let at = (y / TREE_CELL) as usize * tree_dims.0 + (x / TREE_CELL) as usize;
                if let Some(t) = trees.get_mut(at) {
                    *t = t.saturating_add(1);
                }
            } else if p.kind.is_building() {
                buildings.push((x, y));
            } else if p.kind.is_precursor() || p.kind.is_landmark() || p.kind.is_city() {
                for (y, x0, x1) in p.solid_runs(cells) {
                    solid.extend((x0..=x1).map(|x| (x, y)));
                }
            }
        }
        let reach_dims = (
            (size.0 / REACH_CELL) as usize,
            (size.1 / REACH_CELL) as usize,
        );
        let reach = reachable(terrain, reach_dims, file.start_positions());
        let coastal = coastal(terrain, reach_dims, &reach);
        Planner {
            reach,
            coastal,
            reach_dims,
            terrain,
            file,
            symmetry,
            images: symmetry.images(),
            seed: seed ^ 0x7772_6563_6B73,
            size,
            centre: (size.0 / 2.0, size.1 / 2.0),
            water: terrain.water_level().to_f64(),
            starts: file
                .start_positions()
                .iter()
                .map(|p| (p.x.to_f64(), p.y.to_f64()))
                .collect(),
            trees,
            tree_dims,
            buildings,
            solid,
            laid: Vec::new(),
            sites: Vec::new(),
        }
    }

    fn height(&self, (x, y): (f64, f64)) -> f64 {
        self.terrain.height_at(fx2((x, y))).to_f64() - self.water
    }

    fn slope(&self, (x, y): (f64, f64)) -> f64 {
        self.terrain.slope_at(fx2((x, y))).to_f64()
    }

    /// `p` in copy `image`.
    fn image(&self, image: Image, p: (f64, f64)) -> (f64, f64) {
        let v = image.vector((p.0 - self.centre.0, p.1 - self.centre.1));
        (self.centre.0 + v.0, self.centre.1 + v.1)
    }

    fn copies(&self, p: (f64, f64)) -> Vec<(f64, f64)> {
        self.images.iter().map(|&i| self.image(i, p)).collect()
    }

    fn trees_near(&self, (x, y): (f64, f64), reach: f64) -> u32 {
        let n = (reach / TREE_CELL).ceil() as i64;
        let (cx, cy) = ((x / TREE_CELL) as i64, (y / TREE_CELL) as i64);
        let mut count = 0;
        for j in cy - n..=cy + n {
            for i in cx - n..=cx + n {
                if i >= 0
                    && j >= 0
                    && (i as usize) < self.tree_dims.0
                    && (j as usize) < self.tree_dims.1
                {
                    count += self.trees[j as usize * self.tree_dims.0 + i as usize] as u32;
                }
            }
        }
        count
    }

    /// Whether a wreck of `radius` may lie at `p` on `ground`.
    fn fits(&self, p: (f64, f64), radius: f64, ground: Ground) -> bool {
        let margin = radius + 40.0;
        if p.0 < margin || p.1 < margin || p.0 > self.size.0 - margin || p.1 > self.size.1 - margin
        {
            return false;
        }
        let h = self.height(p);
        let rim = [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)]
            .map(|(dx, dy)| self.height((p.0 + dx * radius, p.1 + dy * radius)));
        // Pathing gives up at a slope of 1 in 2 (`mc-sim` nav); wrecks lie where units can reach.
        let land = h > 1.0 && rim.iter().all(|&r| r > 0.5) && self.slope(p) < 0.42;
        // Enough water to have sunk in; a big hull in a shallow bay breaks the surface.
        let deep = 2.5 + radius * 0.15;
        let sea = h < -deep && rim.iter().all(|&r| r < -deep * 0.6);
        let cell = {
            let (i, j) = ((p.0 / REACH_CELL) as usize, (p.1 / REACH_CELL) as usize);
            (i < self.reach_dims.0 && j < self.reach_dims.1).then_some(j * self.reach_dims.0 + i)
        };
        let land = land && cell.is_some_and(|c| self.reach[c]);
        let sea = sea && cell.is_some_and(|c| self.coastal[c]);
        let ground_ok = match ground {
            Ground::Land => land,
            Ground::Sea => sea,
            Ground::Any => land || sea,
        };
        if !ground_ok {
            return false;
        }
        let fp = fx2(p);
        let reach = radius + 30.0;
        let ore = self.file.ore_regions().iter().any(|r| {
            r.contains(fp)
                || [(reach, 0.0), (-reach, 0.0), (0.0, reach), (0.0, -reach)]
                    .iter()
                    .any(|&(dx, dy)| r.contains(fx2((p.0 + dx, p.1 + dy))))
        });
        if ore {
            return false;
        }
        if self
            .buildings
            .iter()
            .any(|b| (b.0 - p.0).powi(2) + (b.1 - p.1).powi(2) < (radius + 45.0).powi(2))
        {
            return false;
        }
        if !self.solid.is_empty() {
            let cell = CELL_SIZE_M as f64;
            let n = ((radius + 12.0) / cell).ceil() as i64;
            let (cx, cy) = ((p.0 / cell) as i64, (p.1 / cell) as i64);
            for j in cy - n..=cy + n {
                for i in cx - n..=cx + n {
                    if i >= 0 && j >= 0 && self.solid.contains(&(i as u32, j as u32)) {
                        return false;
                    }
                }
            }
        }
        true
    }

    /// Whether a wreck fits at `p` in every copy, clear of every wreck laid.
    fn fits_everywhere(&self, p: (f64, f64), radius: f64, ground: Ground) -> bool {
        let copies = self.copies(p);
        copies.iter().all(|&c| {
            self.fits(c, radius, ground)
                && self.laid.iter().all(|&(q, r)| {
                    (q.0 - c.0).powi(2) + (q.1 - c.1).powi(2) > (r + radius + 3.0).powi(2)
                })
        }) && copies.iter().enumerate().all(|(i, a)| {
            copies[i + 1..]
                .iter()
                .all(|b| (a.0 - b.0).powi(2) + (a.1 - b.1).powi(2) > (2.0 * radius + 6.0).powi(2))
        })
    }

    /// A site's copies must stand apart from each other and from every site laid.
    fn site_clear(&self, p: (f64, f64), own: f64, spacing: f64) -> bool {
        let copies = self.copies(p);
        copies.iter().enumerate().all(|(i, a)| {
            copies[i + 1..]
                .iter()
                .all(|b| (a.0 - b.0).powi(2) + (a.1 - b.1).powi(2) > (2.0 * own + 60.0).powi(2))
                && self
                    .sites
                    .iter()
                    .all(|s| (s.0 - a.0).powi(2) + (s.1 - a.1).powi(2) > spacing * spacing)
        })
    }

    /// Nearest and second-nearest start distances.
    fn start_distances(&self, p: (f64, f64)) -> (f64, f64, usize) {
        let mut d: Vec<(f64, usize)> = self
            .starts
            .iter()
            .enumerate()
            .map(|(i, s)| (((s.0 - p.0).powi(2) + (s.1 - p.1).powi(2)).sqrt(), i))
            .collect();
        d.sort_by(|a, b| a.0.total_cmp(&b.0));
        let first = d.first().copied().unwrap_or((f64::MAX, 0));
        let second = d.get(1).map_or(first.0 * 2.0, |s| s.0);
        (first.0, second, first.1)
    }

    fn plan(mut self, report: &mut WreckageReport) -> Vec<(MapWreck, f64)> {
        let mut out = Vec::new();
        let players = self.starts.len().max(1) as f64;
        let copies = self.images.len() as f64;
        let area_km2 = self.size.0 * self.size.1 / 1.0e6;
        // Weight per player grows with the ground each player has, within bounds.
        // About 4000 to 10000 mass a player (`zz_map_salvage_report`).
        let per_player = (area_km2 / players * 2.0).clamp(70.0, 170.0);
        let mut budget = per_player * players / copies;
        let mut laid_weight = 0.0;

        // Candidate sites on a jittered grid, canonical sector only.
        let step = 150.0;
        let (nx, ny) = ((self.size.0 / step) as i64, (self.size.1 / step) as i64);
        let mut candidates = Vec::new();
        for j in 0..ny {
            for i in 0..nx {
                let h = hash2(self.seed, i, j);
                let p = (
                    (i as f64 + 0.2 + 0.6 * unit(h, 0)) * step,
                    (j as f64 + 0.2 + 0.6 * unit(h, 24)) * step,
                );
                if self
                    .symmetry
                    .canonical((p.0 - self.centre.0, p.1 - self.centre.1))
                {
                    candidates.push((p, h));
                }
            }
        }

        // Scrap piles: two per player, a few hundred metres out from the start.
        // A start on a mirror line is its own copy, and each pile laid by it
        // comes back mirrored on its other side: that start lays one.
        let piles_for: Vec<usize> = self
            .starts
            .iter()
            .map(|&s| {
                let own = self
                    .copies(s)
                    .iter()
                    .filter(|c| (c.0 - s.0).powi(2) + (c.1 - s.1).powi(2) < 50.0 * 50.0)
                    .count();
                (2 / own.max(1)).max(1)
            })
            .collect();
        let mut piles = vec![0usize; self.starts.len()];
        let mut near: Vec<_> = candidates
            .iter()
            .filter(|(p, _)| {
                let (d, _, _) = self.start_distances(*p);
                (300.0..700.0).contains(&d)
            })
            .copied()
            .collect();
        near.sort_by_key(|&(_, h)| h >> 8);
        for (p, h) in near {
            let (_, _, start) = self.start_distances(p);
            if piles[start] >= piles_for[start]
                || self.trees_near(p, 80.0) > 4
                || !self.fits_everywhere(p, 20.0, Ground::Land)
                || !self.site_clear(p, 45.0, 250.0)
            {
                continue;
            }
            let count = 2 + (unit(h, 40) * 3.0) as usize;
            let kinds: Vec<Kind> = (0..count).map(|k| pick(&SCRAP, h, k)).collect();
            let w = self.lay_field(p, 45.0, &kinds, h, (720, 950), &mut out);
            if w > 0.0 {
                piles[start] += 1;
                laid_weight += w;
                report.scrap_piles += self.images.len();
            }
        }
        budget -= laid_weight;

        // Battle fields, the richest where the players' ground meets; open
        // ground before woods, land before sea.
        let clear_of_starts = (0.11 * self.size.0.min(self.size.1)).clamp(700.0, 1200.0);
        // Fields keep apart more on a big map, so they spread out from the
        // middle ground instead of crowding it.
        let spacing = (0.05 * self.size.0.min(self.size.1)).clamp(750.0, 3000.0);
        let mut far: Vec<_> = candidates
            .iter()
            .filter_map(|&(p, h)| {
                let (d1, d2, _) = self.start_distances(p);
                (d1 > clear_of_starts).then(|| {
                    let contest = (d1 / d2).clamp(0.0, 1.0);
                    let woods = self.trees_near(p, 120.0) as f64;
                    let sea = if self.height(p) < 0.0 { 0.6 } else { 1.0 };
                    let score = (0.25 + contest * contest) * (0.35 + unit(h, 16)) * sea
                        / (1.0 + woods / 20.0);
                    (p, h, contest, score)
                })
            })
            .collect();
        // Most of the salvage lies where armies walk; a quarter is kept for the
        // sea, and what the sea cannot take goes back to the land after.
        let mut sea_budget = budget * 0.25;
        let mut land_budget = budget - sea_budget;
        far.sort_by(|a, b| b.3.total_cmp(&a.3).then(a.1.cmp(&b.1)));
        for pass in 0..2 {
            if pass == 1 {
                land_budget += sea_budget.max(0.0);
                sea_budget = 0.0;
            }
            for &(p, h, contest, _) in &far {
                if land_budget <= 0.0 && sea_budget <= 0.0 {
                    break;
                }
                let land = self.height(p) > 1.5;
                let sea = self.height(p) < -6.0;
                if !(land || sea) {
                    continue;
                }
                let radius = if sea {
                    220.0
                } else {
                    90.0 + unit(h, 48) * 90.0
                };
                // Most of the ground round the centre must take a wreck.
                let ground = if sea { Ground::Sea } else { Ground::Land };
                let open = (0..12)
                    .filter(|&k| {
                        let (s, c) = (k as f64 * TAU / 12.0).sin_cos();
                        let r = radius * if k % 2 == 0 { 0.35 } else { 0.7 };
                        self.fits((p.0 + c * r, p.1 + s * r), 6.0, ground)
                    })
                    .count();
                if open < 8 {
                    continue;
                }
                let spent = if sea { sea_budget } else { land_budget };
                if spent <= 0.0 || !self.site_clear(p, radius, spacing) {
                    continue;
                }
                let pick_at = |list: &[Kind], k: usize| pick(list, h ^ 0x51, k);
                let mut kinds = Vec::new();
                if sea {
                    let n = 1 + (unit(h, 40) * 3.0) as usize;
                    for k in 0..n {
                        let heavy = contest > 0.7 && k == 0 || unit(h, 8 + k as u32 * 3) < 0.25;
                        kinds.push(if heavy {
                            pick_at(&T2_SEA, k)
                        } else {
                            pick_at(&T1_SEA, k)
                        });
                    }
                    if unit(h, 56) < 0.5 {
                        kinds.push(pick_at(&AIR, 7));
                    }
                } else if contest > 0.8 && unit(h, 56) < 0.4 {
                    // Heavy: a T3 hulk among T2 armour and T1 fodder.
                    kinds.push(pick_at(&T3_LAND, 0));
                    for k in 0..2 + (unit(h, 40) * 3.0) as usize {
                        kinds.push(pick_at(&T2_LAND, k));
                    }
                    for k in 0..3 + (unit(h, 44) * 4.0) as usize {
                        kinds.push(pick_at(&T1_LAND, k));
                    }
                    kinds.push(pick_at(&AIR, 3));
                } else if contest > 0.55 {
                    // Clash: T1 lines with some T2.
                    for k in 0..1 + (unit(h, 40) * 3.0) as usize {
                        kinds.push(pick_at(&T2_LAND, k));
                    }
                    for k in 0..4 + (unit(h, 44) * 5.0) as usize {
                        kinds.push(pick_at(&T1_LAND, k));
                    }
                    if unit(h, 58) < 0.6 {
                        kinds.push(pick_at(&AIR, 5));
                    }
                } else {
                    // Skirmish: a handful of T1.
                    for k in 0..3 + (unit(h, 44) * 4.0) as usize {
                        kinds.push(pick_at(&T1_LAND, k));
                    }
                    if unit(h, 58) < 0.25 {
                        kinds.push(pick_at(&AIR, 1));
                    }
                }
                let w = self.lay_field(p, radius, &kinds, h, (450, 850), &mut out);
                if w > 0.0 {
                    laid_weight += w;
                    if sea {
                        sea_budget -= w;
                        report.sea_fields += self.images.len();
                    } else {
                        land_budget -= w;
                        report.fields += self.images.len();
                    }
                }
            }
        }
        report.weight_per_player = laid_weight * copies / players;
        // Stable order: by position.
        out.sort_by_key(|a| (a.0.pos.y, a.0.pos.x));
        out
    }

    /// Lays one field of `kinds` round `centre` in every copy: two lines that
    /// met across a front, some wrecks slewed round. Returns the weight laid
    /// in the canonical sector.
    fn lay_field(
        &mut self,
        centre: (f64, f64),
        radius: f64,
        kinds: &[Kind],
        hash: u64,
        mass: (u16, u16),
        out: &mut Vec<(MapWreck, f64)>,
    ) -> f64 {
        let front = unit(hash, 20) * TAU;
        let (fs, fc) = front.sin_cos();
        let mut weight = 0.0;
        for (i, k) in kinds.iter().enumerate() {
            let side = if i % 2 == 0 { -1.0 } else { 1.0 };
            for attempt in 0..8 {
                let h = hash2(hash, i as i64, attempt);
                let (along, across) = if k.ground == Ground::Any {
                    // Aircraft came down anywhere over the fight.
                    (
                        (unit(h, 0) * 2.0 - 1.0) * radius,
                        (unit(h, 24) * 2.0 - 1.0) * radius,
                    )
                } else {
                    (
                        side * (0.1 + 0.6 * unit(h, 0)) * radius,
                        (unit(h, 24) * 2.0 - 1.0) * 0.85 * radius,
                    )
                };
                let p = (
                    centre.0 + along * fc - across * fs,
                    centre.1 + along * fs + across * fc,
                );
                if !self.fits_everywhere(p, k.radius, k.ground) {
                    continue;
                }
                // Facing the other line, give or take; one in four slewed anywhere.
                let heading = if unit(h, 48) < 0.25 {
                    unit(h, 8) * TAU
                } else {
                    front + if side < 0.0 { 0.0 } else { PI } + (unit(h, 8) - 0.5) * 1.2
                };
                let bank = if k.ground == Ground::Sea {
                    let deg = 4.0 + 14.0 * unit(h, 32);
                    (if unit(h, 56) < 0.5 { -deg } else { deg } * 65536.0 / 360.0) as i16
                } else {
                    0
                };
                let milli = mass.0 + ((mass.1 - mass.0) as f64 * unit(h, 40)) as u16;
                for &image in &self.images {
                    let at = self.image(image, p);
                    // A mirrored copy lists the other way.
                    let bank = if image.line.is_some() {
                        bank.saturating_neg()
                    } else {
                        bank
                    };
                    out.push((
                        MapWreck {
                            blueprint: k.key.into(),
                            pos: fx2(at),
                            heading: angle(image.angle(heading)),
                            bank,
                            mass_milli: milli,
                        },
                        k.radius,
                    ));
                    self.laid.push((at, k.radius));
                }
                weight += k.weight as f64 * milli as f64 / 1000.0;
                break;
            }
        }
        if weight > 0.0 {
            let copies = self.copies(centre);
            self.sites.extend(copies);
        }
        weight
    }
}

/// Flood fill over drivable land from the starts, one `REACH_CELL` cell at a time.
fn reachable(terrain: &Heightfield, (w, h): (usize, usize), starts: &[FxVec2]) -> Vec<bool> {
    let water = terrain.water_level();
    let limit = Fx::ratio(9, 20);
    let at =
        |i: usize, j: usize| fx2(((i as f64 + 0.5) * REACH_CELL, (j as f64 + 0.5) * REACH_CELL));
    let drivable = |i: usize, j: usize| {
        let p = at(i, j);
        terrain.height_at(p) > water && terrain.slope_at(p) < limit
    };
    let mut seen = vec![false; w * h];
    let mut queue = std::collections::VecDeque::new();
    for s in starts {
        let (i, j) = (
            (s.x.to_f64() / REACH_CELL) as usize,
            (s.y.to_f64() / REACH_CELL) as usize,
        );
        if i < w && j < h && !seen[j * w + i] {
            seen[j * w + i] = true;
            queue.push_back((i, j));
        }
    }
    while let Some((i, j)) = queue.pop_front() {
        let next = [
            (i.wrapping_sub(1), j),
            (i + 1, j),
            (i, j.wrapping_sub(1)),
            (i, j + 1),
        ];
        for (x, y) in next {
            if x < w && y < h && !seen[y * w + x] && drivable(x, y) {
                seen[y * w + x] = true;
                queue.push_back((x, y));
            }
        }
    }
    seen
}

/// Water cells within `COAST_REACH` of reachable land, by a flood out from it.
fn coastal(terrain: &Heightfield, (w, h): (usize, usize), reach: &[bool]) -> Vec<bool> {
    let water = terrain.water_level();
    let wet = |i: usize, j: usize| {
        terrain.height_at(fx2((
            (i as f64 + 0.5) * REACH_CELL,
            (j as f64 + 0.5) * REACH_CELL,
        ))) < water
    };
    let steps = (COAST_REACH / REACH_CELL) as u16;
    let mut dist = vec![u16::MAX; w * h];
    let mut queue = std::collections::VecDeque::new();
    for (at, _) in reach.iter().enumerate().filter(|(_, &r)| r) {
        dist[at] = 0;
        queue.push_back(at);
    }
    while let Some(at) = queue.pop_front() {
        let (i, j) = (at % w, at / w);
        if dist[at] >= steps {
            continue;
        }
        for (x, y) in [
            (i.wrapping_sub(1), j),
            (i + 1, j),
            (i, j.wrapping_sub(1)),
            (i, j + 1),
        ] {
            if x < w && y < h && dist[y * w + x] == u16::MAX && wet(x, y) {
                dist[y * w + x] = dist[at] + 1;
                queue.push_back(y * w + x);
            }
        }
    }
    dist.iter()
        .zip(reach)
        .map(|(&d, &r)| d != u16::MAX && !r)
        .collect()
}

fn pick(list: &[Kind], hash: u64, k: usize) -> Kind {
    list[(hash2(hash, k as i64, 0x6b) % list.len() as u64) as usize]
}

fn fx2((x, y): (f64, f64)) -> FxVec2 {
    FxVec2::new(fx(x), fx(y))
}

fn fx(v: f64) -> Fx {
    Fx((v * 65536.0).round() as i64)
}

fn angle(radians: f64) -> Angle {
    Angle(((radians / TAU).rem_euclid(1.0) * 65536.0) as u32 as u16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn images_map_the_sector_onto_the_map() {
        for sym in [
            Symmetry::Wedges {
                count: 2,
                base: PI / 4.0,
            },
            Symmetry::Wedges {
                count: 8,
                base: PI / 8.0,
            },
            Symmetry::Mirror,
            Symmetry::MirrorSides,
            Symmetry::HalfTurn,
            Symmetry::QuarterTurn,
            Symmetry::Thirds,
        ] {
            let images = sym.images();
            // A point in the sector: each copy is somewhere else, and exactly
            // one copy (the first) is in the sector itself.
            let mut found = None;
            for a in 1..64 {
                let v = (
                    400.0 * (a as f64 * 0.1).cos(),
                    400.0 * (a as f64 * 0.1).sin(),
                );
                if sym.canonical(v) {
                    found = Some(v);
                    break;
                }
            }
            let v = found.expect("a canonical point");
            let copies: Vec<_> = images.iter().map(|i| i.vector(v)).collect();
            assert_eq!(
                copies.iter().filter(|c| sym.canonical(**c)).count(),
                1,
                "{sym:?}"
            );
            assert!((copies[0].0 - v.0).abs() < 1e-9 && (copies[0].1 - v.1).abs() < 1e-9);
            // Headings travel with positions: a point ahead of `v` stays ahead.
            for i in &images {
                let ahead = i.vector((v.0 + 0.3f64.cos(), v.1 + 0.3f64.sin()));
                let at = i.vector(v);
                let dir = (ahead.1 - at.1).atan2(ahead.0 - at.0);
                let turned = i.angle(0.3);
                assert!(
                    ((dir - turned).rem_euclid(TAU) - 0.0).abs() < 1e-6
                        || ((dir - turned).rem_euclid(TAU) - TAU).abs() < 1e-6,
                    "{sym:?}"
                );
            }
        }
    }

    #[test]
    fn every_key_is_short_enough() {
        for k in wreck_keys() {
            assert!(k.len() <= crate::format::MAX_WRECK_KEY);
        }
    }
}
