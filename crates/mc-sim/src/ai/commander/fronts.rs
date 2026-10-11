//! Fronts (`docs/AI_COMMANDER.md`, "Fronts"): where the side's ground meets an
//! enemy's. Each think, every zone gets a claim from each side: ours from what we
//! hold there, the enemy's from what we know of it (`knowledge.rs`). The claims spread
//! over the zone graph by walk (or sail) distance, a strong claim reaching further.
//! A zone each side reaches about as soon as the other is on the border, and a
//! connected run of border zones is a front, one set per enemy player, on land and
//! on water. A cliff or a strait breaks a run, so two passes make two fronts.
//!
//! A front keeps its id from one think to the next by the zones it shares with the
//! front it was (`FrontRecord`), so what it has done can be credited to it.
use super::knowledge::ZoneKnowledge;
use super::zones::{Ground, Zones};
use mc_core::StateHasher;
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// Least claim (materials) that makes a zone a source of a side's reach.
const CLAIM_MIN: i32 = 250;
/// Metres further a claim reaches for each 10 materials of it, up to `REACH_MOST`.
const REACH_MOST: i64 = 1500;
/// Metres within which both sides reach a zone about as soon: it is on the border
/// (as is a zone touching the other side's ground).
const SLACK: i64 = 250;
/// Hops along a border a front spans at most: a longer border is cut into sectors,
/// each a front of its own.
const SECTOR: u32 = 4;
/// A front this small with this little at stake on either side is no front.
const SCRAP_ZONES: usize = 2;
const SCRAP_STAKE: i32 = 600;
/// Hops behind a front whose value counts as what is at stake there.
const BEHIND: usize = 2;
/// Fronts a side keeps at most (a deliberate cap: on a vast map the smallest go).
const MOST_FRONTS: usize = 48;

/// A front as the side keeps it between thinks.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(in crate::ai) struct FrontRecord {
    pub id: u32,
    pub enemy: u8,
    pub water: bool,
    /// Its zones, ascending.
    pub zones: Vec<u16>,
    /// Tick it was first found.
    pub since: u32,
}

impl FrontRecord {
    pub(in crate::ai) fn hash(&self, h: &mut StateHasher) {
        h.write_u64(
            u64::from(self.id)
                | u64::from(self.enemy) << 32
                | u64::from(self.water) << 40
                | (self.zones.len() as u64) << 48,
        );
        for z in &self.zones {
            h.write_u64(u64::from(*z));
        }
        h.write_u64(u64::from(self.since));
    }
}

/// A front this think, with what the side reads of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::ai) struct Front {
    pub id: u32,
    pub enemy: u8,
    pub water: bool,
    pub zones: Vec<u16>,
    /// Their zones beside the front, the way through to them.
    pub entries: Vec<u16>,
    /// What stands within `BEHIND` hops behind it, ours and theirs: what is at stake.
    pub stake_us: i32,
    pub stake_them: i32,
    /// Armed materials near it: ours, and what we believe of theirs.
    pub ours: i32,
    pub theirs: i32,
    /// Hops from it to their nearest held zone of most value: how far a push gets.
    pub depth: u16,
    /// Our zone just behind it that holds the most of ours: where an army stands to
    /// hold it. `None` when no zone of ours touches it.
    pub rear: Option<u16>,
}

/// What each side holds in each zone, as claims.
pub(in crate::ai) struct Claims {
    /// Our structure and army materials, by zone.
    pub ours: Vec<i32>,
    pub our_army: Vec<i32>,
    /// What we believe enemy `p` holds (structures, sure-weighted, and army), by zone.
    pub theirs: Vec<Vec<i32>>,
    pub their_army: Vec<Vec<i32>>,
}

impl Claims {
    /// Enemy claims from what the side knows, `enemies` the players it fights (indexed
    /// by player), `starts` each player's start zone: a start never seen is still claimed.
    pub(in crate::ai) fn read(
        zones: &Zones,
        known: &[ZoneKnowledge],
        enemies: &[bool],
        ours: Vec<i32>,
        our_army: Vec<i32>,
        tick: u32,
    ) -> Claims {
        let n = zones.zones.len();
        let mut theirs = vec![vec![0; n]; enemies.len()];
        let mut their_army = vec![vec![0; n]; enemies.len()];
        for (z, k) in known.iter().enumerate().take(n) {
            for h in &k.held {
                let p = h.player as usize;
                if !enemies.get(p).copied().unwrap_or(false) {
                    continue;
                }
                theirs[p][z] += (h.built_materials() * h.built_sure(tick) / 100).floor_int();
                their_army[p][z] += h.army_now(tick).floor_int();
            }
        }
        for (p, &e) in enemies.iter().enumerate() {
            let Some(&s) = zones.starts.get(p) else {
                continue;
            };
            if e && (s as usize) < n && theirs[p][s as usize] < CLAIM_MIN * 4 {
                theirs[p][s as usize] = CLAIM_MIN * 4;
            }
        }
        Claims {
            ours,
            our_army,
            theirs,
            their_army,
        }
    }
}

/// Walk distance from the nearest claim of at least `CLAIM_MIN` to every zone on
/// `ground`, a claim starting ahead by its strength; `i64::MAX` where none reaches.
fn reach(zones: &Zones, ground: Ground, claim: &[i32], army: &[i32]) -> Vec<i64> {
    let n = zones.zones.len();
    let mut d = vec![i64::MAX; n];
    let mut heap = BinaryHeap::new();
    for z in 0..n {
        if zones.zones[z].ground != ground {
            continue;
        }
        let mut c = claim[z] + army[z] / 2;
        // At sea a coast's claim reaches out over the water beside it, at half.
        if ground == Ground::Water {
            c += zones.zones[z]
                .coast
                .iter()
                .map(|&l| claim[l as usize] / 2)
                .max()
                .unwrap_or(0);
        }
        if c >= CLAIM_MIN {
            let start = -(i64::from(c) / 10).min(REACH_MOST);
            d[z] = start;
            heap.push(Reverse((start, z as u16)));
        }
    }
    while let Some(Reverse((dz, z))) = heap.pop() {
        if dz > d[z as usize] {
            continue;
        }
        let here = zones.zones[z as usize].centre;
        for &a in &zones.zones[z as usize].adj {
            let step = i64::from(here.distance(zones.zones[a as usize].centre).floor_int());
            let next = dz + step.max(1);
            if next < d[a as usize] {
                d[a as usize] = next;
                heap.push(Reverse((next, a)));
            }
        }
    }
    d
}

/// Zones within `hops` of `from` on its own side of the border (`side[z]`), `from`
/// included, in breadth-first order.
fn behind(zones: &Zones, from: &[u16], side: &[bool], hops: usize) -> Vec<u16> {
    let mut seen = vec![false; zones.zones.len()];
    let mut ring: Vec<u16> = from.to_vec();
    for &z in &ring {
        seen[z as usize] = true;
    }
    let mut out = ring.clone();
    for _ in 0..hops {
        let mut next = Vec::new();
        for &z in &ring {
            for &a in &zones.zones[z as usize].adj {
                if !seen[a as usize] && side[a as usize] {
                    seen[a as usize] = true;
                    next.push(a);
                }
            }
        }
        out.extend(&next);
        ring = next;
    }
    out
}

/// The fronts against each enemy this think, on land and on water, ids carried over
/// from `old` by the zones they share. Returns the fronts and the next free id.
pub(in crate::ai) fn find(
    zones: &Zones,
    claims: &Claims,
    old: &[FrontRecord],
    mut next_id: u32,
    tick: u32,
) -> (Vec<Front>, Vec<FrontRecord>, u32) {
    let n = zones.zones.len();
    let mut found: Vec<Front> = Vec::new();
    for (p, theirs) in claims.theirs.iter().enumerate() {
        if theirs.iter().all(|c| *c == 0) {
            continue;
        }
        for ground in [Ground::Land, Ground::Water] {
            let du = reach(zones, ground, &claims.ours, &claims.our_army);
            let dt = reach(zones, ground, theirs, &claims.their_army[p]);
            let reached = |z: usize| du[z] != i64::MAX && dt[z] != i64::MAX;
            let ours_side: Vec<bool> = (0..n).map(|z| reached(z) && du[z] <= dt[z]).collect();
            let their_side: Vec<bool> = (0..n).map(|z| reached(z) && dt[z] < du[z]).collect();
            // On the border: both reach it about as soon, or it touches the other side.
            let border: Vec<bool> = (0..n)
                .map(|z| {
                    reached(z)
                        && ((du[z] - dt[z]).abs() <= SLACK
                            || zones.zones[z].adj.iter().any(|&a| {
                                reached(a as usize) && ours_side[a as usize] != ours_side[z]
                            }))
                })
                .collect();
            let mut taken = vec![false; n];
            for start in 0..n {
                if !border[start] || taken[start] {
                    continue;
                }
                let mut run = vec![start as u16];
                taken[start] = true;
                let mut k = 0;
                while k < run.len() {
                    let z = run[k] as usize;
                    k += 1;
                    for &a in &zones.zones[z].adj {
                        if border[a as usize] && !taken[a as usize] {
                            taken[a as usize] = true;
                            run.push(a);
                        }
                    }
                }
                for mut run in sectors(zones, run) {
                    run.sort_unstable();
                    let entries: Vec<u16> = {
                        let mut e: Vec<u16> = run
                            .iter()
                            .flat_map(|&z| zones.zones[z as usize].adj.iter().copied())
                            .filter(|&a| their_side[a as usize] && !border[a as usize])
                            .collect();
                        e.sort_unstable();
                        e.dedup();
                        e
                    };
                    let ours_behind = behind(zones, &run, &ours_side, BEHIND);
                    let theirs_behind = behind(zones, &run, &their_side, BEHIND);
                    let sum =
                        |list: &[u16], v: &[i32]| list.iter().map(|&z| v[z as usize]).sum::<i32>();
                    // Ore counts for both sides: free ore near a front is what an expansion
                    // race is for.
                    let ore = |list: &[u16]| {
                        list.iter()
                            .map(|&z| {
                                super::zones::ORE_VALUE * i32::from(zones.zones[z as usize].ore)
                            })
                            .sum::<i32>()
                    };
                    let stake_us = sum(&ours_behind, &claims.ours) + ore(&ours_behind);
                    let stake_them = sum(&theirs_behind, theirs) + ore(&theirs_behind);
                    let near_us = behind(zones, &run, &ours_side, 1);
                    let near_them = behind(zones, &run, &their_side, 1);
                    let ours = sum(&near_us, &claims.our_army);
                    let theirs_near = sum(&near_them, &claims.their_army[p]);
                    if run.len() <= SCRAP_ZONES && stake_us.max(stake_them) < SCRAP_STAKE {
                        continue;
                    }
                    let depth = depth_to(zones, &run, &their_side, theirs);
                    let rear = near_us
                        .iter()
                        .copied()
                        .filter(|z| !run.contains(z))
                        .max_by_key(|&z| (claims.ours[z as usize], Reverse(z)));
                    found.push(Front {
                        id: 0,
                        enemy: p as u8,
                        water: ground == Ground::Water,
                        zones: run,
                        entries,
                        stake_us,
                        stake_them,
                        ours,
                        theirs: theirs_near,
                        depth,
                        rear,
                    });
                }
            }
        }
    }
    // The fronts with most at stake are kept when there are too many.
    found.sort_by_key(|f| {
        (
            Reverse(f.stake_us + f.stake_them),
            f.enemy,
            f.water,
            f.zones[0],
        )
    });
    found.truncate(MOST_FRONTS);
    // Ids: the biggest fronts pick first, each the old front of its enemy and ground it
    // shares most zones with (the older on a tie), so a front that splits keeps its id
    // on its larger part and two that merge keep the older.
    let mut order: Vec<usize> = (0..found.len()).collect();
    order.sort_by_key(|&i| (Reverse(found[i].zones.len()), i));
    let mut used = vec![false; old.len()];
    let mut records = Vec::with_capacity(found.len());
    for i in order {
        let f = &found[i];
        let best = old
            .iter()
            .enumerate()
            .filter(|(j, o)| !used[*j] && o.enemy == f.enemy && o.water == f.water)
            .map(|(j, o)| (shared(&o.zones, &f.zones), Reverse(o.id), j))
            .filter(|&(s, ..)| s > 0)
            .max();
        let (id, since) = match best {
            Some((_, _, j)) => {
                used[j] = true;
                (old[j].id, old[j].since)
            }
            None => {
                next_id += 1;
                (next_id, tick)
            }
        };
        records.push(FrontRecord {
            id,
            enemy: f.enemy,
            water: f.water,
            zones: f.zones.clone(),
            since,
        });
        found[i].id = id;
    }
    found.sort_by_key(|f| f.id);
    records.sort_by_key(|r| r.id);
    (found, records, next_id)
}

/// A run of border zones cut into sectors of `SECTOR` hops, measured from one of
/// its ends (the zone furthest from its first zone), so a border across the map is
/// several fronts rather than one.
fn sectors(zones: &Zones, run: Vec<u16>) -> Vec<Vec<u16>> {
    if run.len() <= SECTOR as usize {
        return vec![run];
    }
    let hops = |from: u16| -> Vec<(u16, u32)> {
        let mut out = vec![(from, 0)];
        let mut k = 0;
        while k < out.len() {
            let (z, d) = out[k];
            k += 1;
            for &a in &zones.zones[z as usize].adj {
                if run.contains(&a) && !out.iter().any(|&(o, _)| o == a) {
                    out.push((a, d + 1));
                }
            }
        }
        out
    };
    let end = hops(run[0])
        .iter()
        .max_by_key(|&&(z, d)| (d, Reverse(z)))
        .map_or(run[0], |&(z, _)| z);
    let mut cut: Vec<Vec<u16>> = Vec::new();
    for (z, d) in hops(end) {
        let k = (d / SECTOR) as usize;
        if cut.len() <= k {
            cut.resize(k + 1, Vec::new());
        }
        cut[k].push(z);
    }
    cut.retain(|c| !c.is_empty());
    cut
}

/// Zones two ascending lists share.
fn shared(a: &[u16], b: &[u16]) -> usize {
    let (mut i, mut j, mut n) = (0, 0, 0);
    while i < a.len() && j < b.len() {
        match a[i].cmp(&b[j]) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => {
                n += 1;
                i += 1;
                j += 1;
            }
        }
    }
    n
}

/// Hops from `run` across their side to their most valuable zone; 0 when none.
fn depth_to(zones: &Zones, run: &[u16], side: &[bool], theirs: &[i32]) -> u16 {
    let Some(goal) = (0..zones.zones.len())
        .filter(|&z| side[z])
        .max_by_key(|&z| (theirs[z], Reverse(z)))
        .filter(|&z| theirs[z] > 0)
    else {
        return 0;
    };
    let mut seen = vec![false; zones.zones.len()];
    let mut ring: Vec<u16> = run.to_vec();
    for &z in run {
        seen[z as usize] = true;
    }
    let mut hops = 0;
    while !ring.is_empty() && hops < u16::MAX {
        if ring.contains(&(goal as u16)) {
            return hops;
        }
        hops += 1;
        let mut next = Vec::new();
        for &z in &ring {
            for &a in &zones.zones[z as usize].adj {
                if !seen[a as usize] {
                    seen[a as usize] = true;
                    next.push(a);
                }
            }
        }
        ring = next;
    }
    0
}

impl crate::World {
    /// Finds `player`'s fronts this think, from what it holds and what it knows: their
    /// records kept for the next think, and what was read of them for the overlay.
    pub(in crate::ai) fn find_fronts(
        &mut self,
        player: u8,
        zones: &Zones,
        zv: &super::zones::ZoneView,
        profiles: &super::profile::Profiles,
    ) {
        let n = zones.zones.len();
        let mut our_army = vec![0; n];
        let units = &self.state.units;
        for r in units.slots.iter() {
            if units.owner[r] != player || !units.is_active(r) {
                continue;
            }
            let p = profiles.get(units.blueprint[r]);
            if p.armed() && p.mobile() {
                if let Some(z) = zones.of(units.pos[r]) {
                    our_army[z as usize] += p.materials.floor_int();
                }
            }
        }
        let ours: Vec<i32> = zv.held.iter().map(|m| m.floor_int()).collect();
        let enemies: Vec<bool> = (0..self.state.players.len())
            .map(|p| !self.state.players[p].defeated && self.are_enemies(player, p as u8))
            .collect();
        let tick = self.state.tick;
        let c = &self.state.ai[player as usize].commander;
        let claims = Claims::read(zones, &c.knowledge, &enemies, ours, our_army, tick);
        let (fronts, records, next) = find(zones, &claims, &c.fronts, c.next_front, tick);
        let c = &mut self.state.ai[player as usize].commander;
        c.fronts = records;
        c.next_front = next;
        c.scratch.fronts = fronts;
    }
}

#[cfg(test)]
mod tests;
