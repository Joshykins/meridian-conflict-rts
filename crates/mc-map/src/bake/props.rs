//! Trees, rocks and city buildings: the props a map is dressed with, read off
//! each baked tile's own samples.

use super::*;

/// The start clearings: how far the woods' edge stands from a start, as a share
/// of the pad's core (base, and swing with the bearing), how deep the wood's
/// thinning edge is and how ragged its line, as shares of the core.
const CLEARING: (f64, f64) = (0.62, 0.55);
const CLEARING_EDGE: f64 = 0.5;
const CLEARING_TATTER: f64 = 0.13;
/// The smallest core (m) a glade is scaled by, so a small map's start still
/// has room for a base before the woods.
const CLEARING_MIN_CORE: f64 = 340.0;

impl Terrain {
    /// How thickly trees grow at a point, 0..=1, and how much of the stand is
    /// conifer. Sampled at the folded position, so every player gets the same
    /// woods. Big forests come from one broad field with clearings cut into
    /// it; small groves and copses dot the open ground between them.
    pub(super) fn forest_density(&self, x: f64, y: f64, height: f64, slope: f64) -> (f64, f64) {
        let (density, conifer) = if self.is_alpine() {
            self.alpine_forest(x, y, height, slope)
        } else {
            match self.layout {
                Layout::Archipelago => self.archipelago_forest(x, y, height, slope),
                Layout::TwinBays => self.bays_forest(x, y, height, slope),
                Layout::Threshold => self.threshold_forest(x, y, height, slope),
                Layout::Canyon => self.canyon_forest(x, y, height, slope),
                Layout::Frostline => self.frostline_forest(x, y, height, slope),
                _ => self.basin_forest(x, y, height, slope),
            }
        };
        (density * self.start_clearing(x, y), conifer)
    }

    /// 0 in the clearing round a start, rising to 1 in the woods. The clearing's
    /// edge wanders in and out with the bearing and thins out over a wood's edge,
    /// so a start sits in a glade the landscape runs up to, not in a drawn circle.
    /// Scaled by each start pad's level core.
    pub(super) fn start_clearing(&self, x: f64, y: f64) -> f64 {
        let mut k = 1.0f64;
        for p in &self.pads {
            let (dx, dy) = (x - p.x, y - p.y);
            let d = dx.hypot(dy);
            let core = p.core.max(CLEARING_MIN_CORE);
            if d > (1.0 + CLEARING_EDGE + CLEARING_TATTER) * core
                || !self.starts.contains(&(p.x, p.y))
            {
                continue;
            }
            let (s, c) = dy.atan2(dx).sin_cos();
            let wander = self
                .forest_kind
                .fbm(p.x / 311.0 + c * 1.7, p.y / 311.0 + s * 1.7, 3, 0.5);
            let edge = (CLEARING.0 + CLEARING.1 * wander).clamp(0.45, 0.95) * core;
            let tatter =
                CLEARING_TATTER * core * self.detail.fbm(x / 70.0 + 4.1, y / 70.0 - 9.3, 2, 0.5);
            k = k.min(smoothstep(edge, edge + CLEARING_EDGE * core, d + tatter));
        }
        k
    }

    /// The open basin layouts' woods: one broad field with clearings, and copses.
    fn basin_forest(&self, x: f64, y: f64, height: f64, slope: f64) -> (f64, f64) {
        let (px, py, _) = self.fold(x, y);
        let l = self.l_forest;
        let broad = self.forest.fbm(px / l, py / l, 3, 0.5);
        let forest = smoothstep(self.forest_edge, self.forest_edge + 0.22, broad);
        let copse = smoothstep(
            0.42,
            0.62,
            self.forest
                .fbm(px / (0.16 * l) + 71.3, py / (0.16 * l) - 19.1, 2, 0.5),
        );
        let clearing = smoothstep(
            0.30,
            0.55,
            self.forest
                .fbm(px / (0.09 * l) - 33.7, py / (0.09 * l) + 57.2, 2, 0.5),
        );
        let tree_floor = if self.layout == Layout::Islands {
            6.0
        } else {
            1.5
        };
        let habitable = smoothstep(tree_floor, tree_floor + 6.0, height)
            * (1.0 - smoothstep(0.28, 0.5, slope))
            * (1.0 - smoothstep(230.0, 290.0, height));
        let density = (forest * (1.0 - 0.85 * clearing)).max(copse * 0.75) * habitable;
        // Conifers take the high and the cold ground, in stands, not a salt-and-pepper mix.
        // Species do not change play, so an unfolded term may break up the
        // straight line a mirror axis would otherwise draw between stands.
        let cold = smoothstep(60.0, 170.0, height) * 0.9
            + self.forest_kind.fbm(px / 1500.0, py / 1500.0, 2, 0.5) * 0.9
            + self
                .forest_kind
                .fbm(x / 380.0 + 41.0, y / 380.0 - 13.0, 2, 0.5)
                * 0.45;
        (density, smoothstep(-0.15, 0.35, cold))
    }

    /// Trees expected on the map for a given forest edge, from a fixed sample
    /// of the landscape: bakes stay reproducible.
    pub(super) fn expected_trees(&self, samples: &[(f64, f64, f64, f64)]) -> f64 {
        let per_sample =
            self.size_x * self.size_y / (FOREST_GRID_M * FOREST_GRID_M) / samples.len() as f64;
        samples
            .iter()
            .map(|&(x, y, h, s)| self.forest_density(x, y, h, s).0 * FOREST_ACCEPT)
            .sum::<f64>()
            * per_sample
    }

    /// Raises the forest edge until the map's trees fit [`MAX_TREES`]. Small
    /// maps keep the full woods; the largest lose whole forests rather than
    /// thinning every one of them into an orchard.
    pub(super) fn fit_forests(&mut self) {
        let mut samples = Vec::new();
        let step = (self.size_x * self.size_y / 30_000.0).sqrt();
        let (nx, ny) = ((self.size_x / step) as i64, (self.size_y / step) as i64);
        for j in 0..ny {
            for i in 0..nx {
                let hash = hash2(self.seed ^ 0x6669_7466, i, j);
                let x = (i as f64 + unit(hash, 0)) * step;
                let y = (j as f64 + unit(hash, 24)) * step;
                let h = self.height(x, y);
                if h > 1.5 {
                    samples.push((x, y, h, self.slope(x, y)));
                }
            }
        }
        if samples.is_empty() {
            return;
        }
        let (mut lo, mut hi) = (self.forest_edge, 1.0);
        if self.expected_trees(&samples) <= MAX_TREES as f64 {
            return;
        }
        for _ in 0..16 {
            self.forest_edge = 0.5 * (lo + hi);
            if self.expected_trees(&samples) > MAX_TREES as f64 {
                lo = self.forest_edge;
            } else {
                hi = self.forest_edge;
            }
        }
        self.forest_edge = hi;
    }

    pub(super) fn tree_kind(&self, x: f64, y: f64, conifer: f64, hash: u64) -> PropKind {
        // The archipelago's `conifer` share is its palms.
        if self.layout == Layout::Archipelago {
            return self.archipelago_tree(conifer, hash);
        }
        // The canyon's `conifer` slot carries the ground's height.
        if self.layout == Layout::Canyon {
            return self.canyon_tree(conifer, hash);
        }
        // Frostline's likewise, and its kinds go by the side of the wall.
        if self.layout == Layout::Frostline {
            return self.frostline_tree(x, y, conifer, hash);
        }
        // The alpine map's woods are not mirrored.
        let (px, py) = match self.layout {
            Layout::Alpine | Layout::AlpineTeams => (x, y),
            _ => {
                let (px, py, _) = self.fold(x, y);
                (px, py)
            }
        };
        // Pines stand in their own patches among the firs.
        let pines = self.forest_kind.get(px / 420.0 + 11.0, py / 420.0 - 5.0) > 0.1;
        match (unit(hash, 40), unit(hash, 48)) {
            (dead, _) if dead < 0.02 => PropKind::TreeDead,
            (_, pick) if pick < conifer && pines => PropKind::TreePine,
            (_, pick) if pick < conifer => PropKind::TreeConifer,
            _ => PropKind::TreeBroadleaf,
        }
    }

    /// Trees and rocks for one tile, read off the tile's own samples so props
    /// react to the terrain the player will actually see.
    pub(super) fn tile_props(&self, tx: u32, ty: u32, samples: &[u16], pads: &[Pad]) -> Vec<Prop> {
        let n = TILE_SAMPLES as usize;
        let cell = CELL_SIZE_M as f64;
        let (x0, y0) = (
            (tx as i32 * TILE_SIZE_M) as f64,
            (ty as i32 * TILE_SIZE_M) as f64,
        );
        let (min_z, step) = z_range(self.layout);
        let (min_z, step) = (min_z.to_f64(), step.to_f64());
        let z = |s: u16| min_z + s as f64 * step;
        // Height, slope and lowest corner of the sample cell under a point.
        let ground = |x: f64, y: f64| {
            let (lx, ly) = ((x - x0) / cell, (y - y0) / cell);
            let (cx, cy) = ((lx as usize).min(n - 2), (ly as usize).min(n - 2));
            let at = cy * n + cx;
            let (z00, z10, z01, z11) = (
                z(samples[at]),
                z(samples[at + 1]),
                z(samples[at + n]),
                z(samples[at + n + 1]),
            );
            let gx = (z10 - z00 + z11 - z01) / (2.0 * cell);
            let gy = (z01 - z00 + z11 - z10) / (2.0 * cell);
            (
                (z00 + z10 + z01 + z11) / 4.0,
                (gx * gx + gy * gy).sqrt(),
                z00.min(z10).min(z01).min(z11),
            )
        };
        let blocked = |x: f64, y: f64| {
            if self.layout == Layout::Canyon && !self.canyon_clear(x, y) {
                return true;
            }
            // Starts sit in ragged glades (`start_clearing`), not drawn circles;
            // town pads keep their streets clear.
            self.start_clearing(x, y) < 0.02
                || pads.iter().any(|p| {
                    !self.starts.contains(&(p.x, p.y))
                        && (x - p.x).powi(2) + (y - p.y).powi(2) < p.outer * p.outer
                })
        };
        let mut props = Vec::new();
        // The alpine map is rockier, and its woods climb steeper ground.
        let alpine = self.is_alpine() || self.layout == Layout::Threshold;
        let arctic = false;

        let (rock_slope, rock_roll) = if alpine || arctic {
            (0.18, 0.8)
        } else {
            (0.3, 0.93)
        };
        let forest_slope = if alpine { 0.95 } else { 0.5 };

        // Rocks on the slopes, and lone trees out in the open.
        let per_tile = (TILE_SIZE_M as f64 / PROP_GRID_M) as i64;
        for j in 0..per_tile {
            for i in 0..per_tile {
                let (gi, gj) = (tx as i64 * per_tile + i, ty as i64 * per_tile + j);
                let hash = hash2(self.seed ^ 0x7072_6F70, gi, gj);
                let x = (gi as f64 + unit(hash, 0)) * PROP_GRID_M;
                let y = (gj as f64 + unit(hash, 24)) * PROP_GRID_M;
                let roll = unit(hash2(self.seed ^ 0x726F_6C6C, gi, gj), 0);
                let (height, slope, lowest) = ground(x, y);
                if lowest < 1.5 || blocked(x, y) {
                    continue;
                }
                let (density, conifer) = self.forest_density(x, y, height, slope);
                let (kind, scale) = if density > 0.0 && slope < 0.3 && roll < 0.05 * (1.0 - density)
                {
                    (
                        self.tree_kind(x, y, conifer, hash),
                        800 + ((hash >> 32) % 600) as u16,
                    )
                } else if (rock_slope..1.5).contains(&slope)
                    && roll > rock_roll
                    && !(self.is_alpine() && self.alpine_ice(x, y) > 0.05)
                    && self.machine_clear(x, y)
                {
                    let desert = self.layout == Layout::Canyon
                        || (self.layout == Layout::Frostline && frostline::east_of(x, y) < 0.0);
                    let kind = if desert {
                        // Blocks of bedded sandstone fallen from the walls.
                        [PropKind::RockSlab, PropKind::RockLarge][(roll > 0.985) as usize]
                    } else if roll > 0.98 {
                        PropKind::RockLarge
                    } else {
                        PropKind::RockSmall
                    };
                    (kind, 700 + ((hash >> 32) % 700) as u16)
                } else {
                    continue;
                };
                props.push(Prop {
                    kind,
                    pos: FxVec2::new(Fx((x * 65536.0) as i64), Fx((y * 65536.0) as i64)),
                    heading: Angle((hash >> 8) as u16),
                    scale_milli: scale,
                });
            }
        }

        // Forests: a fine jittered grid, kept where the woods are dense.
        let per_tile = (TILE_SIZE_M as f64 / FOREST_GRID_M) as i64;
        for j in 0..per_tile {
            for i in 0..per_tile {
                let (gi, gj) = (tx as i64 * per_tile + i, ty as i64 * per_tile + j);
                let hash = hash2(self.seed ^ 0x776F_6F64, gi, gj);
                let roll = unit(hash2(self.seed ^ 0x7374_616E, gi, gj), 0);
                if roll >= FOREST_ACCEPT {
                    continue;
                }
                // Jitter within the middle of the cell keeps trunks from touching.
                let x = (gi as f64 + 0.15 + 0.7 * unit(hash, 0)) * FOREST_GRID_M;
                let y = (gj as f64 + 0.15 + 0.7 * unit(hash, 24)) * FOREST_GRID_M;
                let (height, slope, lowest) = ground(x, y);
                if lowest < 1.5 || slope >= forest_slope {
                    continue;
                }
                let (density, conifer) = self.forest_density(x, y, height, slope);
                if roll >= density * FOREST_ACCEPT || blocked(x, y) || !self.machine_clear(x, y) {
                    continue;
                }
                // Tall in the heart of a wood, shorter and bushier at its edge.
                let scale = 650.0 + density * 400.0 + unit(hash, 32) * 450.0;
                props.push(Prop {
                    kind: self.tree_kind(x, y, conifer, hash),
                    pos: FxVec2::new(Fx((x * 65536.0) as i64), Fx((y * 65536.0) as i64)),
                    heading: Angle((hash >> 8) as u16),
                    scale_milli: (scale as u16).min(1500),
                });
            }
        }
        props
    }

    /// Street grids of buildings on the city and town pads: blocks of three
    /// lots between streets, towers toward the middle, vacant lots here and
    /// there, the square around a town's ore field left open.
    pub(super) fn city_props(&self, out: &mut Vec<Prop>) {
        for (index, town) in self.towns.iter().enumerate() {
            let reach = (town.radius / CITY_LOT_M) as i64;
            let (sin, cos) = town.heading.sin_cos();
            let heading = (town.heading / TAU * 65536.0).rem_euclid(65536.0) as u16;
            for j in -reach..=reach {
                for i in -reach..=reach {
                    let (lx, ly) = (i as f64 * CITY_LOT_M, j as f64 * CITY_LOT_M);
                    let d = (lx * lx + ly * ly).sqrt();
                    let hash = hash2(self.seed ^ (0x6369_7479 + index as u64), i, j);
                    let street = i.rem_euclid(4) == 0 || j.rem_euclid(4) == 0;
                    if street || d > town.radius - CITY_LOT_M / 2.0 || unit(hash, 0) < 0.12 {
                        continue;
                    }
                    let (x, y) = (town.x + lx * cos - ly * sin, town.y + lx * sin + ly * cos);
                    if self.ore.iter().any(|f| f.covers(x, y, CITY_LOT_M / 2.0)) {
                        continue;
                    }
                    let pick = unit(hash, 24);
                    let kind = match d / town.radius {
                        f if f < 0.3 => [PropKind::BuildingTower, PropKind::BuildingLarge]
                            [(pick < 0.4) as usize],
                        f if f < 0.65 => [PropKind::BuildingLarge, PropKind::BuildingMedium]
                            [(pick < 0.6) as usize],
                        _ => [PropKind::BuildingMedium, PropKind::BuildingSmall]
                            [(pick < 0.65) as usize],
                    };
                    out.push(Prop {
                        kind,
                        pos: FxVec2::new(Fx((x * 65536.0) as i64), Fx((y * 65536.0) as i64)),
                        // Square to the street grid, facing any of the four ways.
                        heading: Angle(heading.wrapping_add(((hash >> 50) as u16 & 3) << 14)),
                        scale_milli: 850 + ((hash >> 32) % 400) as u16,
                    });
                }
            }
        }
    }
}
