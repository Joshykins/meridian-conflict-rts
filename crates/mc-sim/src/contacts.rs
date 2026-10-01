//! Hull contacts: after steering proposes where every ground unit goes this
//! tick, hulls that would sink into each other are pushed apart.
//!
//! A crowd of thousands is split into 64 m tiles. Each pair of hulls in touch
//! belongs to the tile of its first unit, and only reaches into the tiles
//! round it, so tiles three apart share no unit: the nine tiles of a 3 x 3
//! block take turns, and all the tiles of one turn run side by side. Pairs that
//! reach further (a giant's) are pushed after the tiles, one by one. The order
//! is fixed by rows and tiles, so every machine pushes the same way.

use crate::spatial::{kind, SpatialIndex, NO_OWNER};
use crate::tables::flag;
use crate::World;
use mc_core::{Fx, FxVec2};
use mc_data::MoveLayer;

/// Room left in the contact search for the pushes that part a crowd: two
/// passes of the largest correction, from both sides.
const CONTACT_PUSH_ROOM: Fx = Fx::from_int(12);
/// Most a hull is pushed by one pair in one pass, metres.
const MOST_PUSH: Fx = Fx::from_int(6);
/// A crowd pressed against a slope needs more passes to spread.
const PASSES: usize = 8;
/// Tile edge: 64 m. Two hulls of up to 16 m paired within
/// `CONTACT_PUSH_ROOM` of touching stand in the same tile or next-door ones.
const TILE_SHIFT: u32 = 6;
/// Widest hull a tile takes; pairs with a bigger one are pushed after.
const TILE_RADIUS: Fx = Fx::from_int(16);
/// Moves handed to one worker at a time while pairing.
const CHUNK: usize = 128;

/// What a push needs of a hull, looked up once a tick instead of per push.
#[derive(Clone, Copy)]
struct Hull {
    row: usize,
    radius: Fx,
    mass: Fx,
    layer: MoveLayer,
    size_class: u8,
}

/// A tile, and the pairs of its hulls it could not hold.
type Built = (Tile, Vec<(usize, usize)>);

/// The pairs of one tile, over the few units they touch.
struct Tile {
    /// Indices into the moves, ascending.
    units: Vec<usize>,
    /// Their hulls, side by side for the passes.
    hulls: Vec<Hull>,
    /// Pairs as indices into `units`, in row order.
    pairs: Vec<(u32, u32)>,
}

impl World {
    /// Resolve overlaps between the proposed positions `at` of the moving units
    /// `rows`, including different ground locomotion types. Steering alone is
    /// not a collision constraint.
    pub(crate) fn resolve_mobile_contacts(&self, rows: &[usize], at: &mut [FxVec2]) {
        let span = mc_core::perf_span!("contacts.pairs");
        let pairs = self.contact_pairs(rows, at);
        drop(span);
        let span = mc_core::perf_span!("contacts.tiles");
        mc_core::perf_count!("move.pairs", pairs.len());
        let hulls: Vec<Hull> = rows
            .iter()
            .map(|&row| {
                let motion = self.bp(row).motion.unwrap();
                Hull {
                    row,
                    radius: self.bp(row).radius,
                    mass: self.crowd_mass(row),
                    layer: motion.layer,
                    size_class: motion.size_class,
                }
            })
            .collect();
        let (turns, apart) = tiles(&self.pool, &pairs, at, &hulls);
        mc_core::perf_count!("move.apart_pairs", apart.len());
        mc_core::perf_count!("move.tiles", turns.iter().map(Vec::len).sum::<usize>());
        mc_core::perf_count!(
            "move.biggest_tile",
            turns
                .iter()
                .flatten()
                .map(|t| t.pairs.len())
                .max()
                .unwrap_or(0)
        );
        drop(span);
        let _span = mc_core::perf_span!("contacts.passes");
        // A tile that pushed nothing, none of whose hulls has moved since, would
        // push nothing again: it sits the pass out. `moved[k]` is the step (one
        // per turn, and one for the pairs apart) that last moved hull `k`.
        let mut moved = vec![0u32; at.len()];
        let mut ran: Vec<Vec<(u32, bool)>> =
            turns.iter().map(|t| vec![(0, true); t.len()]).collect();
        let mut step = 0u32;
        for _ in 0..PASSES {
            mc_core::perf_count!("move.pair_passes");
            let mut pushed_any = false;
            for (turn, ran) in turns.iter().zip(&mut ran) {
                step += 1;
                let mut live: Vec<usize> = (0..turn.len())
                    .filter(|&t| {
                        let (when, pushed) = ran[t];
                        pushed || turn[t].units.iter().any(|&k| moved[k] > when)
                    })
                    .collect();
                mc_core::perf_count!("move.tile_runs", live.len());
                // Biggest first, a tile at a time: the workers take them in turn, and
                // a crowd's dense tile no longer leaves the rest waiting on its chunk.
                // The tiles of a turn share no hull, so the order changes nothing.
                live.sort_by_key(|&t| std::cmp::Reverse(turn[t].pairs.len()));
                let chunk = 1;
                let done: Vec<Vec<(bool, Vec<FxVec2>)>> =
                    self.pool
                        .parallel_map_chunks(live.len(), chunk, |_, range| {
                            let _work = mc_core::perf_span!("contacts.tile_work");
                            live[range]
                                .iter()
                                .map(|&t| {
                                    let tile = &turn[t];
                                    let mut any = false;
                                    let mut local: Vec<FxVec2> =
                                        tile.units.iter().map(|&k| at[k]).collect();
                                    for &(a, b) in &tile.pairs {
                                        let (a, b) = (a as usize, b as usize);
                                        let (pa, pb) = (local[a], local[b]);
                                        if let Some((pa, pb)) =
                                            self.push_pair(&tile.hulls[a], &tile.hulls[b], pa, pb)
                                        {
                                            any = true;
                                            (local[a], local[b]) = (pa, pb);
                                        }
                                    }
                                    (any, local)
                                })
                                .collect()
                        });
                for (&t, (any, local)) in live.iter().zip(done.into_iter().flatten()) {
                    pushed_any |= any;
                    ran[t] = (step, any);
                    for (&k, pos) in turn[t].units.iter().zip(local) {
                        if at[k] != pos {
                            at[k] = pos;
                            moved[k] = step;
                        }
                    }
                }
            }
            step += 1;
            for &(i, j) in &apart {
                if let Some((pa, pb)) = self.push_pair(&hulls[i], &hulls[j], at[i], at[j]) {
                    pushed_any = true;
                    (at[i], at[j]) = (pa, pb);
                    (moved[i], moved[j]) = (step, step);
                }
            }
            if !pushed_any {
                break;
            }
        }
    }

    /// Every pair of moving ground hulls that could meet this tick, as indices
    /// into `rows`, sorted: those whose proposed spots come within reach of
    /// touching, with room for the pushes. They are found on an index of the
    /// proposed spots, built for the purpose.
    fn contact_pairs(&self, rows: &[usize], at: &[FxVec2]) -> Vec<(usize, usize)> {
        // What the pairing asks of each hull, looked up once: radius, naval, strider.
        // Flight spacing belongs to formations, not ground hull contacts.
        let hulls: Vec<Option<(Fx, bool, bool)>> = rows
            .iter()
            .map(|&row| {
                let motion = self.bp(row).motion.unwrap();
                (motion.layer != MoveLayer::Air).then_some((
                    self.bp(row).radius,
                    motion.layer == MoveLayer::Naval,
                    motion.stride,
                ))
            })
            .collect();
        let mut spots = SpatialIndex::new(self.terrain.size_metres());
        for (i, hull) in hulls.iter().enumerate() {
            if let Some((radius, ..)) = *hull {
                spots.insert(kind::UNIT, NO_OWNER, i, at[i], radius);
            }
        }
        spots.build();
        let chunks: Vec<Vec<(usize, usize)>> =
            self.pool
                .parallel_map_chunks(rows.len(), CHUNK, |_, range| {
                    let mut pairs = Vec::new();
                    for i in range {
                        let Some((radius, naval, stride)) = hulls[i] else {
                            continue;
                        };
                        let first = pairs.len();
                        let reach = radius + Fx::ONE + CONTACT_PUSH_ROOM;
                        spots.query(at[i], reach, kind::UNIT, |e| {
                            let j = e.row as usize;
                            let Some((_, other_naval, other_stride)) = hulls[j] else {
                                return true;
                            };
                            // A dived submarine slips under a floating hull, and it over it.
                            if j > i
                                && !stride
                                && !other_stride
                                && !(naval && other_naval && self.hulls_pass(rows[i], rows[j]))
                            {
                                pairs.push((i, j));
                            }
                            true
                        });
                        // Pushes land in pair order: sort each hull's so it follows
                        // the rows, not the index layout. The moves are in row
                        // order and the chunks come back in order, so the whole
                        // list is sorted.
                        pairs[first..].sort_unstable();
                    }
                    pairs
                });
        chunks.into_iter().flatten().collect()
    }

    /// Where hulls `a` at `pa` and `b` at `pb` stand once pushed apart, or
    /// `None` when they are not in touch.
    fn push_pair(&self, a: &Hull, b: &Hull, pa: FxVec2, pb: FxVec2) -> Option<(FxVec2, FxVec2)> {
        let (ra, rb) = (a.radius, b.radius);
        let delta = pa - pb;
        let touch = ra + rb + Fx::ONE;
        // Out of touch is decided before the square root, exactly.
        if !delta.shorter_than(touch) {
            return None;
        }
        let dist = delta.length();
        let overlap = touch - dist;
        let dir = if dist > Fx::EPSILON {
            // `delta.normalize()` without taking the root again.
            FxVec2::new(delta.x / dist, delta.y / dist)
        } else {
            FxVec2::from_angle(mc_core::Angle((a.row as u16).wrapping_mul(9973)))
        };
        mc_core::perf_count!("move.pushes");
        let correction = overlap.min(MOST_PUSH);
        // Where `row` from `from` stands pushed `amount` along `direction`, if it can stand there.
        let pushed = |hull: &Hull, from: FxVec2, direction: FxVec2, amount: Fx| {
            let candidate = self.clamp_to_map(from + direction * amount);
            self.nav
                .passable(hull.layer, hull.size_class, candidate)
                .then_some(candidate)
        };
        let (share_a, share_b) = (share(a.mass, b.mass), share(b.mass, a.mass));
        let na = pushed(a, pa, dir, correction * share_a);
        let nb = pushed(b, pb, -dir, correction * share_b);
        // A hull pressed against a slope cannot give way; the other
        // takes the whole push, or two hulls stay sunk into each other.
        let (na, nb) = match (na, nb) {
            (None, Some(_)) => (None, pushed(b, pb, -dir, correction).or(nb)),
            (Some(_), None) => (pushed(a, pa, dir, correction).or(na), None),
            both => both,
        };
        Some((na.unwrap_or(pa), nb.unwrap_or(pb)))
    }

    /// How firmly a hull holds its ground in a crowd: the ground it covers,
    /// twice over while it is on its way somewhere. A Fulgur shoulders a
    /// column of tanks aside and they cannot shove it back; a unit under
    /// orders makes a parked one of its size step aside.
    fn crowd_mass(&self, row: usize) -> Fx {
        let r = self.bp(row).radius;
        let flags = self.state.units.flags[row];
        let going = flags & flag::HAS_FIELD != 0 && flags & flag::HOLD == 0;
        r * r * if going { 2 } else { 1 }
    }

    /// The share of an overlap between `row` and `other` that `row` gives way
    /// by. Under an eighth, none: a hull that much heavier is a wall to the
    /// other, or a column pressing on a parked Fulgur walks it off a tank's
    /// width at a time.
    pub(crate) fn give_way(&self, row: usize, other: usize) -> Fx {
        share(self.crowd_mass(row), self.crowd_mass(other))
    }
}

/// [`World::give_way`] from the two crowd masses.
fn share(mine: Fx, theirs: Fx) -> Fx {
    if mine == theirs {
        return Fx::HALF;
    }
    let share = theirs / (mine + theirs).max(Fx::EPSILON);
    if share < Fx::ratio(1, 8) {
        Fx::ZERO
    } else {
        share
    }
}

/// Sorts `pairs` (sorted, as `contact_pairs` gives them) into tiles by where
/// their first hull stands, grouped into the nine turns of a 3 x 3 block, and
/// the pairs no tile can hold (a hull wider than `TILE_RADIUS`, or the other
/// hull beyond the next tile). The hulls are sorted into tiles, not the pairs,
/// and each tile is put together on the pool.
fn tiles(
    pool: &mc_jobs::Pool,
    pairs: &[(usize, usize)],
    at: &[FxVec2],
    hulls: &[Hull],
) -> (Vec<Vec<Tile>>, Vec<(usize, usize)>) {
    let cell = |p: FxVec2| (p.x.floor_int() >> TILE_SHIFT, p.y.floor_int() >> TILE_SHIFT);
    // `starts[i]..starts[i + 1]`: the pairs whose first hull is `i`.
    let mut starts = vec![0usize; at.len() + 1];
    for &(i, _) in pairs {
        starts[i + 1] += 1;
    }
    for i in 1..starts.len() {
        starts[i] += starts[i - 1];
    }
    let mut firsts: Vec<((i32, i32), usize)> = (0..at.len())
        .filter(|&i| starts[i + 1] > starts[i])
        .map(|i| (cell(at[i]), i))
        .collect();
    // Hulls are unique, so the sort is total: each tile's pairs stay in row order.
    firsts.sort_unstable();
    let runs: Vec<&[((i32, i32), usize)]> = firsts.chunk_by(|a, b| a.0 == b.0).collect();
    let built: Vec<Vec<Built>> = pool.parallel_map_chunks(runs.len(), 16, |_, range| {
        // Each hull's place in the tile being put together; `NONE` between tiles.
        const NONE: u32 = u32::MAX;
        let mut place = vec![NONE; at.len()];
        runs[range]
            .iter()
            .map(|run| {
                let tile = run[0].0;
                let (mut held, mut apart) = (Vec::new(), Vec::new());
                for &(_, i) in *run {
                    for &(i, j) in &pairs[starts[i]..starts[i + 1]] {
                        let tj = cell(at[j]);
                        if hulls[i].radius > TILE_RADIUS
                            || hulls[j].radius > TILE_RADIUS
                            || (tile.0 - tj.0).abs() > 1
                            || (tile.1 - tj.1).abs() > 1
                        {
                            apart.push((i, j));
                        } else {
                            held.push((i, j));
                        }
                    }
                }
                let mut units: Vec<usize> = held.iter().flat_map(|&(i, j)| [i, j]).collect();
                units.sort_unstable();
                units.dedup();
                for (n, &k) in units.iter().enumerate() {
                    place[k] = n as u32;
                }
                let pairs = held.iter().map(|&(i, j)| (place[i], place[j])).collect();
                for &k in &units {
                    place[k] = NONE;
                }
                let hulls = units.iter().map(|&k| hulls[k]).collect();
                (
                    Tile {
                        units,
                        hulls,
                        pairs,
                    },
                    apart,
                )
            })
            .collect()
    });
    let mut turns: Vec<Vec<Tile>> = (0..9).map(|_| Vec::new()).collect();
    let mut apart = Vec::new();
    for (run, (tile, far)) in runs.iter().zip(built.into_iter().flatten()) {
        let (x, y) = run[0].0;
        apart.extend(far);
        if !tile.pairs.is_empty() {
            turns[(x.rem_euclid(3) * 3 + y.rem_euclid(3)) as usize].push(tile);
        }
    }
    (turns, apart)
}
