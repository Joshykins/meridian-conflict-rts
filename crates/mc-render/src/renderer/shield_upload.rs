//! Shields into the GPU's shield list: which domes of a team fuse, and which hulls
//! are near each shell (the short contact list the fragment shader walks). The
//! hulls come out of a coarse grid of the units, built once a tick, so a battle of
//! tens of thousands of units costs each shield only the cells round its shell,
//! not a pass over every unit.

use super::*;
use glam::Vec2;
use mc_sim::mirror::ShieldInstance;

/// Metres across a grid cell: a few hulls' reach, so a small field looks at a
/// handful of cells and a big dome at a few dozen.
const CELL: f32 = 64.0;
/// Cells along a side at most: a map's worth at `CELL`, coarser beyond.
const MOST_CELLS: usize = 512;

/// Units by grid cell (counting-sorted), for "who is near here" by cells.
struct UnitGrid {
    origin: Vec2,
    cell: f32,
    cols: usize,
    rows: usize,
    /// Per cell, where its units start in `items`; one more entry closes the last.
    starts: Vec<u32>,
    /// Unit indices, cell after cell, each cell's in index order.
    items: Vec<u32>,
}

impl UnitGrid {
    fn new(units: &[UnitInstance], keep: impl Fn(&UnitInstance) -> bool) -> UnitGrid {
        let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
        for u in units.iter().filter(|u| keep(u)) {
            let p = Vec2::new(u.pos[0], u.pos[1]);
            lo = lo.min(p);
            hi = hi.max(p);
        }
        if lo.x > hi.x {
            lo = Vec2::ZERO;
            hi = Vec2::ZERO;
        }
        let span = (hi - lo).max_element().max(1.0);
        let cell = CELL.max(span / MOST_CELLS as f32);
        let cols = ((hi.x - lo.x) / cell) as usize + 1;
        let rows = ((hi.y - lo.y) / cell) as usize + 1;
        let mut grid = UnitGrid {
            origin: lo,
            cell,
            cols,
            rows,
            starts: vec![0; cols * rows + 1],
            items: Vec::new(),
        };
        let at = |g: &UnitGrid, u: &UnitInstance| {
            let (x, y) = g.cell_of(Vec2::new(u.pos[0], u.pos[1]));
            y * g.cols + x
        };
        for u in units.iter().filter(|u| keep(u)) {
            let c = at(&grid, u);
            grid.starts[c + 1] += 1;
        }
        for c in 0..cols * rows {
            grid.starts[c + 1] += grid.starts[c];
        }
        let mut next = grid.starts.clone();
        grid.items = vec![0; grid.starts[cols * rows] as usize];
        for (i, u) in units.iter().enumerate().filter(|(_, u)| keep(u)) {
            let c = at(&grid, u);
            grid.items[next[c] as usize] = i as u32;
            next[c] += 1;
        }
        grid
    }

    fn cell_of(&self, p: Vec2) -> (usize, usize) {
        let c = ((p - self.origin) / self.cell).max(Vec2::ZERO);
        (
            (c.x as usize).min(self.cols - 1),
            (c.y as usize).min(self.rows - 1),
        )
    }

    /// Every unit in the cells a square `reach` either side of `centre` touches.
    fn near(&self, centre: Vec2, reach: f32) -> impl Iterator<Item = usize> + '_ {
        let (x0, y0) = self.cell_of(centre - Vec2::splat(reach));
        let (x1, y1) = self.cell_of(centre + Vec2::splat(reach));
        (y0..=y1).flat_map(move |y| {
            let row = y * self.cols;
            let (a, b) = (
                self.starts[row + x0] as usize,
                self.starts[row + x1 + 1] as usize,
            );
            self.items[a..b].iter().map(|&i| i as usize)
        })
    }
}

impl Renderer {
    pub(super) fn upload_shields(&mut self, frame: &RenderFrame, eye: Vec3) {
        let n = frame.shields.len().min(MAX_SHIELDS);
        let src = &frame.shields[..n];
        self.live_effect_barriers.clear();
        for s in src {
            if s.open < 200.0 / 255.0
                || s.health <= 0.0
                || s.packed & ((1 << 24) | (1 << 26)) != 0
                || s.radius <= 0.01
            {
                continue;
            }
            self.live_effect_barriers
                .push(EffectBarrier::of(s, self.map_info.water_level.to_f32()));
        }
        self.effect_barriers.write(
            0,
            bytemuck::cast_slice(&[self.live_effect_barriers.len() as u32, 0, 0, 0]),
        );
        self.effect_barriers
            .write(16, bytemuck::cast_slice(&self.live_effect_barriers));
        let units = &frame.units[..self.sim_units as usize];
        let skip_hull = KIND_GHOST | KIND_PROP | (mc_sim::tables::flag::IN_FACTORY as u32) << 8;
        let touches = |e: &UnitInstance| e.radius >= 0.4 && e.owner_flags & skip_hull == 0;
        let grid = (n > 0).then(|| UnitGrid::new(units, touches));
        // How far past a shell a hull still counts: the biggest hull's reach.
        let reach_most = units
            .iter()
            .filter(|e| touches(e))
            .map(|e| e.radius * 2.2 + 4.0)
            .fold(0.0, f32::max);
        // Each shield's unit, found in one pass over the units. The first instance of a
        // unit is the unit itself; wreck sections come later.
        let mut wearers: Vec<u32> = src.iter().map(|s| s.unit_id).collect();
        wearers.sort_unstable();
        let mut worn: Vec<(u32, u32)> = units
            .iter()
            .enumerate()
            .filter(|(_, u)| wearers.binary_search(&u.unit_id).is_ok())
            .map(|(i, u)| (u.unit_id, i as u32))
            .collect();
        worn.sort_by_key(|w| w.0);
        worn.dedup_by_key(|w| w.0);
        let entity_of = |s: &ShieldInstance| {
            worn.binary_search_by_key(&s.unit_id, |w| w.0)
                .ok()
                .map(|k| worn[k].1)
        };
        let mut gpu = Vec::with_capacity(n);
        for (i, s) in src.iter().enumerate() {
            let team = (s.packed >> 8) & 255;
            let hull = (s.packed >> 25) & 1 == 1;
            let mut overlap = 0u32;
            // Hull wraps stay their own membrane: they do not fuse with a dome. Nor does a veil.
            if !hull && s.packed & mc_sim::mirror::SHIELD_VEIL == 0 {
                for (j, other) in src.iter().enumerate() {
                    if i == j
                        || ((other.packed >> 8) & 255) != team
                        || (other.packed >> 25) & 1 == 1
                        || other.packed & mc_sim::mirror::SHIELD_VEIL != 0
                    {
                        continue;
                    }
                    let dx = s.pos[0] - other.pos[0];
                    let dy = s.pos[1] - other.pos[1];
                    let dz = s.pos[2] - other.pos[2];
                    let r = s.radius + other.radius + SHIELD_PAD * 2.0;
                    if dx * dx + dy * dy + dz * dz <= r * r {
                        overlap = 1;
                        break;
                    }
                }
            }
            let mut contacts = [0u32; SHIELD_CONTACTS];
            let mut scores = [f32::MAX; SHIELD_CONTACTS];
            let mut contact_n = 0u32;
            let shell = s.radius;
            // The dome is flattened (`mc_data::dome_height`): measure in the space where it is round.
            let stretch = if hull {
                1.0
            } else {
                s.radius / mc_data::dome_height_f32(s.radius).max(0.001)
            };
            // Across the ground a contact is never further out than the shell and a reach.
            let near = grid
                .iter()
                .flat_map(|g| g.near(Vec2::new(s.pos[0], s.pos[1]), shell + reach_most));
            for ei in near {
                let e = &units[ei];
                if e.unit_id == s.unit_id {
                    continue;
                }
                let reach = e.radius * 2.2 + 4.0;
                let ox = e.pos[0] - s.pos[0];
                let oy = e.pos[1] - s.pos[1];
                let mut oz = (e.pos[2] - s.pos[2]) * stretch;
                if !hull {
                    // Under the rim the glass is the wall down to the ground: only
                    // the distance across counts there.
                    oz = oz.max(0.0);
                }
                let d2 = ox * ox + oy * oy + oz * oz;
                let lo = shell - reach;
                let hi = shell + reach;
                if d2 < lo.max(0.0) * lo.max(0.0) || d2 > hi * hi {
                    continue;
                }
                let ex = e.pos[0] - eye.x;
                let ey = e.pos[1] - eye.y;
                let ez = e.pos[2] - eye.z;
                let score = ex * ex + ey * ey + ez * ez;
                if contact_n < SHIELD_CONTACTS as u32 {
                    let k = contact_n as usize;
                    contacts[k] = ei as u32;
                    scores[k] = score;
                    contact_n += 1;
                    continue;
                }
                let mut worst = 0usize;
                for k in 1..SHIELD_CONTACTS {
                    if scores[k] > scores[worst] {
                        worst = k;
                    }
                }
                if score < scores[worst] {
                    contacts[worst] = ei as u32;
                    scores[worst] = score;
                }
            }
            gpu.push(GpuShield {
                pos: s.pos,
                radius: s.radius,
                prev_open: s.prev_open,
                open: s.open,
                health: s.health,
                packed: s.packed,
                unit_id: s.unit_id,
                projector: s.projector,
                height: s.height,
                overlap,
                contact_n,
                prev_radius: s.prev_radius,
                entity: entity_of(s).unwrap_or(crate::gpu_consts::shield::NO_ENTITY),
                _pad: 0,
                contacts,
            });
        }
        if n > 0 {
            self.shields.write(0, bytemuck::cast_slice(&gpu));
        }
        let prev = self.shield_count as usize;
        if n < prev {
            let zeros = vec![GpuShield::zeroed(); prev - n];
            self.shields.write(
                (n * size_of::<GpuShield>()) as u64,
                bytemuck::cast_slice(&zeros),
            );
        }
        self.shield_count = n as u32;
        let is_hull = |s: &&ShieldInstance| (s.packed >> 25) & 1 == 1;
        self.hull_shield_count = src.iter().filter(is_hull).count() as u32;
        self.hull_draws.clear();
        for s in src.iter().filter(is_hull) {
            let unit = entity_of(s).map(|e| units[e as usize].blueprint);
            let Some(&[first, lods]) = unit.and_then(|b| self.model_draws.get(b as usize)) else {
                self.hull_draws.clear();
                break;
            };
            for slot in first..first + lods {
                if !self.hull_draws.contains(&slot) {
                    self.hull_draws.push(slot);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytemuck::Zeroable;

    fn at(x: f32, y: f32) -> UnitInstance {
        UnitInstance {
            pos: [x, y, 0.0],
            ..UnitInstance::zeroed()
        }
    }

    /// The grid hands back everything within reach (and may add a few more from the
    /// same cells), whatever the spread of the units.
    #[test]
    fn grid_finds_every_unit_within_reach() {
        let units: Vec<UnitInstance> = (0..2000)
            .map(|i| {
                let f = i as f32;
                at((f * 37.3) % 9000.0 - 200.0, (f * 91.7) % 7000.0 + 50.0)
            })
            .collect();
        let grid = UnitGrid::new(&units, |u| u.pos[0] != 100.0);
        for (centre, reach) in [
            (Vec2::new(0.0, 0.0), 300.0),
            (Vec2::new(4000.0, 3500.0), 64.0),
            (Vec2::new(-5000.0, 20000.0), 100.0),
            (Vec2::new(8800.0, 6900.0), 900.0),
        ] {
            let mut found: Vec<usize> = grid.near(centre, reach).collect();
            found.sort_unstable();
            for (i, u) in units.iter().enumerate() {
                let d = (Vec2::new(u.pos[0], u.pos[1]) - centre).abs();
                if d.x <= reach && d.y <= reach {
                    assert!(found.binary_search(&i).is_ok(), "{i} missed near {centre}");
                }
            }
        }
    }
}
