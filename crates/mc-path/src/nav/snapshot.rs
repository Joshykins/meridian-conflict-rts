//! Snapshot export/import, for late join and reconnect.
//!
//! A joiner must observe exactly what everyone else does from the next tick
//! on, and almost everything in `Nav` is history: which corridors exist, what
//! is cached, slot generations, the free-list order that decides the next
//! `FieldId`, and builds in flight. So the blob carries all of it, tiles
//! included. An in-flight build is exported as its finished result, still
//! un-adopted, with its ready tick and the blocks recorded against it; the
//! importer adopts it on the same tick and draws the same conclusions. Waiting
//! for the result is invisible to the exporter, whose `begin_tick` would have
//! waited for the same value anyway.
//!
//! Left out because they cannot be observed: the sector graph cache, `Arc`
//! sharing between tiles, and the two timing-dependent counters.

use super::*;
use crate::field::Tile;
use crate::grid::SECTOR_AREA;
use crate::wire::{Reader, Writer};

const MAGIC: u32 = u32::from_le_bytes(*b"MCPN");
const FORMAT: u32 = 1;
const BAD: PathError = PathError::BadSnapshot;
/// Serialised tile: sector, version, seed hash, base, flags, directions, border costs.
const TILE_BYTES: usize = 4 + 4 + 8 + 4 + 1 + SECTOR_AREA + 4 * 4 * SECTOR;
/// Border costs are path lengths; anything near `u32::MAX` that is not the
/// `INF` marker would overflow when a build adds to it.
const MAX_COST: u32 = 1 << 30;
const INF: u32 = u32::MAX;

fn cfg_words(cfg: &NavConfig) -> [u64; 9] {
    [
        cfg.max_fields as u64,
        cfg.max_anchors as u64,
        cfg.max_tiles_per_field as u64,
        cfg.max_total_tiles as u64,
        cfg.max_search_nodes as u64,
        cfg.base_latency as u64,
        cfg.sectors_per_latency_tick as u64,
        cfg.corridor_margin as i64 as u64,
        cfg.los_radius as i64 as u64,
    ]
}

fn write_anchors(out: &mut Writer, anchors: &[Anchor]) {
    out.u32(anchors.len() as u32);
    anchors.iter().for_each(|a| out.u32(a.1));
}

fn write_data(out: &mut Writer, data: &FieldData) {
    let (x0, y0, x1, y1) = data.bounds;
    [x0, y0, x1, y1].iter().for_each(|&v| out.i32(v));
    out.u32(data.stats.tiles_built);
    out.u32(data.stats.tiles_reused);
    out.u32(data.stats.search_nodes);
    out.u32s(&data.unreachable);
    out.u32(data.tiles.len() as u32);
    for (sector, tile) in &data.tiles {
        out.u32(*sector);
        out.u32(tile.version);
        out.u64(tile.seed_hash);
        out.u32(tile.base);
        out.u8(tile.patched as u8 | (tile.marked as u8) << 1);
        out.bytes(&tile.dirs);
        tile.edges.iter().flatten().for_each(|&c| out.u32(c));
    }
}

struct Limits {
    sectors: u32,
    cells: u32,
    max_tiles: usize,
}

fn read_anchors(r: &mut Reader, grid: &NavGrid, lim: &Limits) -> Result<Vec<Anchor>, PathError> {
    let n = r.count(4)?;
    let mut anchors: Vec<Anchor> = Vec::with_capacity(n);
    for _ in 0..n {
        let cell = r.u32()?;
        if cell >= lim.cells {
            return Err(BAD);
        }
        let a = (grid.sector_of(grid.cell_from_index(cell)), cell);
        if anchors.last().is_some_and(|&l| l >= a) {
            return Err(BAD);
        }
        anchors.push(a);
    }
    Ok(anchors)
}

fn read_data(r: &mut Reader, lim: &Limits) -> Result<FieldData, PathError> {
    let bounds = (r.i32()?, r.i32()?, r.i32()?, r.i32()?);
    let stats = BuildStats { tiles_built: r.u32()?, tiles_reused: r.u32()?, search_nodes: r.u32()? };
    let unreachable = r.u32s()?;
    if unreachable.iter().any(|&c| c >= lim.cells) {
        return Err(BAD);
    }
    let n = r.count(TILE_BYTES)?;
    if n > lim.max_tiles {
        return Err(BAD);
    }
    let mut tiles: Vec<(u32, Arc<Tile>)> = Vec::with_capacity(n);
    for _ in 0..n {
        let sector = r.u32()?;
        if sector >= lim.sectors || tiles.last().is_some_and(|l| l.0 >= sector) {
            return Err(BAD);
        }
        let (version, seed_hash, base, flags) = (r.u32()?, r.u64()?, r.u32()?, r.u8()?);
        let mut tile = Tile { dirs: [0; SECTOR_AREA], edges: [[0; SECTOR]; 4], version, seed_hash, base, patched: flags & 1 != 0, marked: flags & 2 != 0 };
        tile.dirs.copy_from_slice(r.bytes(SECTOR_AREA)?);
        for c in tile.edges.iter_mut().flatten() {
            *c = r.u32()?;
            if *c != INF && (*c < base || *c > MAX_COST) {
                return Err(BAD);
            }
        }
        if flags > 3 || base > MAX_COST {
            return Err(BAD);
        }
        tiles.push((sector, Arc::new(tile)));
    }
    Ok(FieldData { tiles, unreachable, bounds, stats })
}

fn read_error(r: &mut Reader) -> Result<PathError, PathError> {
    PathError::from_code(r.u8()?).ok_or(BAD)
}

impl Nav {
    /// Everything that can influence what this `Nav` does from now on, except
    /// the static terrain. Call between ticks. Blocks until builds in flight
    /// have finished (they stay un-adopted); changes nothing observable here.
    pub fn export_state(&mut self) -> Vec<u8> {
        let mut out = Writer::default();
        out.u32(MAGIC);
        out.u32(FORMAT);
        cfg_words(&self.cfg).iter().for_each(|&w| out.u64(w));
        self.grid.write_dynamic(&mut out);
        out.u64(self.tick);
        let s = &self.stats;
        for v in [s.builds_scheduled, s.builds_adopted, s.repairs_scheduled, s.extends_scheduled, s.tiles_built, s.tiles_reused, s.search_nodes, s.fields_evicted] {
            out.u64(v);
        }
        out.u32s(&self.free);
        out.u32(self.slots.len() as u32);
        for slot in &self.slots {
            out.u32(slot.generation);
            let Some(f) = &slot.field else {
                out.u8(0);
                continue;
            };
            out.u8(1);
            out.u8(f.layer as u8);
            out.u8(f.size.index());
            out.u32(f.key.2);
            out.u32(f.refcount);
            out.u64(f.released_tick);
            write_anchors(&mut out, &f.anchors);
            write_anchors(&mut out, &f.queued_anchors);
            out.u8(f.queued_repair as u8);
            out.u8(f.error.map_or(0, |e| e.code() + 1));
            out.u8(f.data.is_some() as u8);
            if let Some(d) = &f.data {
                write_data(&mut out, d);
            }
            out.u8(f.pending.is_some() as u8);
            if let Some(p) = &f.pending {
                out.u64(p.ready_tick);
                write_anchors(&mut out, &p.routing);
                out.u32s(&p.dirty);
                out.u8(p.dirty_all as u8);
                out.u8(p.unblocked as u8);
                let mut v = p.slot.value.lock().unwrap_or_else(|e| e.into_inner());
                while v.is_none() {
                    v = p.slot.ready.wait(v).unwrap_or_else(|e| e.into_inner());
                }
                match v.as_ref().unwrap() {
                    Ok(d) => {
                        out.u8(0);
                        write_data(&mut out, d);
                    }
                    Err(e) => {
                        out.u8(1);
                        out.u8(e.code());
                    }
                }
            }
        }
        out.buf
    }

    /// Rebuilds a `Nav` from `export_state` output. `base_grid` must be the
    /// same map with no blockers applied and `cfg` the exporter's config;
    /// either mismatch, or any damage to `bytes`, is `PathError::BadSnapshot`.
    pub fn import_state(base_grid: NavGrid, cfg: NavConfig, spawner: Arc<dyn Spawner>, bytes: &[u8]) -> Result<Nav, PathError> {
        let mut r = Reader::new(bytes);
        if r.u32()? != MAGIC || r.u32()? != FORMAT {
            return Err(BAD);
        }
        for want in cfg_words(&cfg) {
            if r.u64()? != want {
                return Err(BAD);
            }
        }
        let mut nav = Nav::new(base_grid, cfg, spawner);
        nav.grid.read_dynamic(&mut r)?;
        nav.tick = r.u64()?;
        let s = &mut nav.stats;
        for v in [&mut s.builds_scheduled, &mut s.builds_adopted, &mut s.repairs_scheduled, &mut s.extends_scheduled, &mut s.tiles_built, &mut s.tiles_reused, &mut s.search_nodes, &mut s.fields_evicted] {
            *v = r.u64()?;
        }
        nav.free = r.u32s()?;
        let (sw, sh) = nav.grid.sector_dims();
        let lim = Limits { sectors: (sw * sh) as u32, cells: (nav.grid.width() * nav.grid.height()) as u32, max_tiles: nav.cfg.max_tiles_per_field };
        let slots = r.count(5)?;
        if slots > nav.cfg.max_fields {
            return Err(BAD);
        }
        for index in 0..slots as u32 {
            let generation = r.u32()?;
            let mut slot = Slot { generation, field: None };
            if r.flag()? {
                let layer = *MoveLayer::ALL.get(r.u8()? as usize).ok_or(BAD)?;
                let size = SizeClass::new(r.u8()?).map_err(|_| BAD)?;
                let goal_index = r.u32()?;
                if goal_index >= lim.cells {
                    return Err(BAD);
                }
                let key = (layer as u8, size.index(), goal_index);
                let (refcount, released_tick) = (r.u32()?, r.u64()?);
                let anchors = read_anchors(&mut r, &nav.grid, &lim)?;
                let queued_anchors = read_anchors(&mut r, &nav.grid, &lim)?;
                let queued_repair = r.flag()?;
                let error = match r.u8()? {
                    0 => None,
                    c => Some(PathError::from_code(c - 1).ok_or(BAD)?),
                };
                let data = if r.flag()? { Some(Arc::new(read_data(&mut r, &lim)?)) } else { None };
                let pending = if r.flag()? {
                    let ready_tick = r.u64()?;
                    let routing = read_anchors(&mut r, &nav.grid, &lim)?;
                    let dirty = r.u32s()?;
                    if dirty.len() > MAX_DIRTY || dirty.iter().any(|&s| s >= lim.sectors) {
                        return Err(BAD);
                    }
                    let (dirty_all, unblocked) = (r.flag()?, r.flag()?);
                    let result = if r.flag()? { Err(read_error(&mut r)?) } else { Ok(read_data(&mut r, &lim)?) };
                    let slot = Arc::new(ResultSlot { value: Mutex::new(Some(result)), ready: Condvar::new() });
                    Some(Pending { ready_tick, slot, routing, dirty, dirty_all, unblocked })
                } else {
                    None
                };
                nav.total_tiles += data.as_ref().map_or(0, |d| d.tiles.len());
                if nav.by_key.insert(key, index).is_some() {
                    return Err(BAD);
                }
                let goal = nav.grid.cell_from_index(goal_index);
                slot.field = Some(Field { layer, size, goal, key, refcount, released_tick, anchors, data, error, pending, queued_anchors, queued_repair });
            }
            nav.slots.push(slot);
        }
        r.finish()?;
        // The free list must name each empty slot exactly once: its order picks future ids.
        let mut listed = vec![false; nav.slots.len()];
        for &i in &nav.free {
            let empty = nav.slots.get(i as usize).is_some_and(|s| s.field.is_none());
            if !empty || std::mem::replace(&mut listed[i as usize], true) {
                return Err(BAD);
            }
        }
        if nav.free.len() != nav.slots.iter().filter(|s| s.field.is_none()).count() || nav.total_tiles > nav.cfg.max_total_tiles {
            return Err(BAD);
        }
        Ok(nav)
    }
}
