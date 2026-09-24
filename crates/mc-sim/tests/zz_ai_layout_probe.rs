//! AI-only match on a real map, printing how tidy each side's base is: how
//! many power plants stand alone or far from home, how many clusters the
//! power is split into, and how much each shield actually covers. With
//! `LAYOUT_OUT=dir` it also writes a top-down PPM of every base.
//! `LAYOUT=meridian_basin:4:30 cargo test --release -p mc-sim --test zz_ai_layout_probe -- --ignored --nocapture`
//! (map, players, minutes; optional `:seed`).
use mc_core::{Fx, FxVec2};
use mc_data::{cat, Blueprints, MoveLayer};
use mc_jobs::Pool;
use mc_sim::tables::Controller;
use mc_sim::{AiConfig, Difficulty, MatchConfig, PlayerSetup, World};
use std::path::Path;
use std::sync::Arc;

#[test]
#[ignore]
fn layout() {
    let spec = std::env::var("LAYOUT").unwrap_or_else(|_| "meridian_basin:4:30".into());
    let parts: Vec<&str> = spec.split(':').collect();
    let players: u8 = parts.get(1).and_then(|m| m.parse().ok()).unwrap_or(4);
    let minutes: u32 = parts.get(2).and_then(|m| m.parse().ok()).unwrap_or(30);
    let seed: u64 = parts.get(3).and_then(|m| m.parse().ok()).unwrap_or(7);
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let bps = Arc::new(Blueprints::load(&root.join("data")).unwrap());
    let map = mc_map::MapFile::open(root.join(format!("maps/{}.mcmap", parts[0]))).unwrap();
    let config = MatchConfig {
        seed,
        players: (0..players)
            .map(|i| PlayerSetup {
                name: format!("AI {i}"),
                faction: "Aster".into(),
                ai: AiConfig {
                    difficulty: Difficulty::Normal,
                    ..AiConfig::default()
                },
                team: i,
                controller: Controller::Ai,
                start: i,
            })
            .collect(),
        cheats: false,
        fog: true,
        spawn_commanders: true,
    };
    let mut w = World::new(&map, bps, Arc::new(Pool::new(1)), &config).unwrap();
    println!("layout {spec}");
    for (p, pl) in w.state.players.iter().enumerate() {
        println!(
            "  P{p} starts at {:.0},{:.0}",
            pl.start.x.to_f32(),
            pl.start.y.to_f32()
        );
    }
    let every: u32 = std::env::var("LAYOUT_EVERY")
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or(5);
    let (
        mut sum_alone,
        mut sum_far,
        mut sum_power,
        mut sum_useless,
        mut sum_shields,
        mut sum_clusters,
    ) = (0, 0, 0, 0, 0, 0);
    // Seconds each side spent short of energy, and mass it made.
    let mut short = vec![0u32; players as usize];
    let mut made = vec![0f32; players as usize];
    for minute in 1..=minutes {
        for t in 0..600 {
            w.tick(&[]).unwrap();
            if t % 10 == 0 {
                for (p, pl) in w.state.players.iter().enumerate() {
                    short[p] += (pl.energy_demand > pl.energy_income
                        && pl.energy < pl.energy_capacity / 10)
                        as u32;
                    made[p] += pl.mass_income.to_f32();
                }
            }
        }
        let last = minute == minutes || w.state.winner.is_some();
        if minute % every != 0 && !last {
            continue;
        }
        let s = &w.state;
        for p in 0..s.players.len() {
            if s.players[p].defeated {
                continue;
            }
            let start = s.players[p].start;
            // (pos, footprint metres, blueprint categories, mass cost, shield radius)
            let mut own: Vec<(FxVec2, i32, u32, f32, f32)> = Vec::new();
            for row in s
                .units
                .slots
                .iter()
                .filter(|&r| s.units.owner[r] as usize == p)
            {
                let bp = w.bp(row);
                if !bp.is_structure() {
                    continue;
                }
                own.push((
                    s.units.pos[row],
                    bp.footprint.0.max(bp.footprint.1) as i32 * mc_map::BUILD_CELL_M,
                    bp.categories,
                    bp.cost_mass.to_f32(),
                    bp.shield.as_ref().map_or(0.0, |sh| sh.radius.to_f32()),
                ));
            }
            let power: Vec<_> = own.iter().filter(|o| o.2 & cat::POWER != 0).collect();
            if last && std::env::var("LAYOUT_LIST").is_ok_and(|v| v == p.to_string()) {
                for row in s
                    .units
                    .slots
                    .iter()
                    .filter(|&r| s.units.owner[r] as usize == p)
                {
                    let bp = w.bp(row);
                    if bp.has(cat::POWER) {
                        let q = s.units.pos[row] - start;
                        println!(
                            "      {} {:.0},{:.0} active {}",
                            bp.key,
                            q.x.to_f32(),
                            q.y.to_f32(),
                            s.units.is_active(row)
                        );
                    }
                }
            }
            let gap = |a: &(FxVec2, i32, u32, f32, f32), b: &(FxVec2, i32, u32, f32, f32)| {
                a.0.distance(b.0).to_f32() - (a.1 + b.1) as f32 / 2.0
            };
            // Alone: no other structure (mines aside) within 40 m of its edge.
            let alone = power
                .iter()
                .filter(|a| {
                    !own.iter()
                        .any(|b| b.0 != a.0 && b.2 & cat::EXTRACTOR == 0 && gap(a, b) < 40.0)
                })
                .count();
            let far = power
                .iter()
                .filter(|a| a.0.distance(start).to_f32() > 700.0)
                .count();
            // Clusters: plants linked edge to edge within 40 m.
            let mut group = vec![usize::MAX; power.len()];
            let mut clusters = 0;
            for i in 0..power.len() {
                if group[i] != usize::MAX {
                    continue;
                }
                group[i] = clusters;
                let mut stack = vec![i];
                while let Some(j) = stack.pop() {
                    for k in 0..power.len() {
                        if group[k] == usize::MAX && gap(power[j], power[k]) < 40.0 {
                            group[k] = clusters;
                            stack.push(k);
                        }
                    }
                }
                clusters += 1;
            }
            let mut shields = Vec::new();
            for sh in own.iter().filter(|o| o.4 > 0.0) {
                let covered: f32 = own
                    .iter()
                    .filter(|o| o.0 != sh.0 && o.4 == 0.0 && o.2 & cat::EXTRACTOR == 0)
                    .filter(|o| o.0.distance(sh.0).to_f32() < sh.4)
                    .map(|o| o.3)
                    .sum();
                shields.push(covered);
            }
            let useless = shields.iter().filter(|&&c| c < 600.0).count();
            if last {
                sum_alone += alone;
                sum_far += far;
                sum_power += power.len();
                sum_useless += useless;
                sum_shields += shields.len();
                sum_clusters += clusters;
            }
            println!(
                "  {minute:>2}m P{p}: mass made {:.0} energy-starved {}s factories {} structures {} | power {} alone {alone} >700m {far} clusters {clusters} | shields {} covering {:?} (useless {useless})",
                made[p],
                short[p],
                own.iter().filter(|o| o.2 & cat::FACTORY != 0).count(),
                own.len(),
                power.len(),
                shields.len(),
                shields.iter().map(|c| *c as i32).collect::<Vec<_>>(),
            );
            if let (true, Ok(dir)) = (last, std::env::var("LAYOUT_OUT")) {
                draw(
                    &w,
                    &own,
                    start,
                    &format!("{dir}/{}_s{seed}_p{p}.ppm", parts[0]),
                );
            }
        }
        if last {
            break;
        }
    }
    println!(
        "  total at end: power {sum_power} alone {sum_alone} far {sum_far} clusters {sum_clusters} | shields {sum_shields} useless {sum_useless}"
    );
}

/// 1600 m square around the start at 2 m a pixel: land grey-green, no-go
/// ground dark, water blue, structures filled by kind, shields as rings.
fn draw(w: &World, own: &[(FxVec2, i32, u32, f32, f32)], start: FxVec2, path: &str) {
    const N: i32 = 800;
    const M: f32 = 2.0;
    let origin = (start.x.to_f32() - N as f32, start.y.to_f32() - N as f32);
    let mut img = vec![[20u8, 20, 24]; (N * N) as usize];
    for y in 0..N {
        for x in 0..N {
            let at = FxVec2::new(
                Fx::from_f32(origin.0 + x as f32 * M),
                Fx::from_f32(origin.1 + y as f32 * M),
            );
            if !w.terrain.in_bounds(at) {
                continue;
            }
            img[(y * N + x) as usize] = if w.nav.passable(MoveLayer::Land, 0, at) {
                [74, 88, 70]
            } else if w.nav.passable(MoveLayer::Naval, 0, at) {
                [30, 60, 110]
            } else {
                [45, 42, 40]
            };
        }
    }
    let mut put = |x: i32, y: i32, c: [u8; 3]| {
        if (0..N).contains(&x) && (0..N).contains(&y) {
            img[(y * N + x) as usize] = c;
        }
    };
    for o in own {
        let c = if o.2 & cat::FACTORY != 0 {
            [230, 230, 230]
        } else if o.2 & cat::POWER != 0 {
            [255, 200, 40]
        } else if o.2 & cat::EXTRACTOR != 0 {
            [200, 90, 255]
        } else if o.2 & cat::SHIELD != 0 {
            [60, 230, 255]
        } else if o.2 & cat::DEFENSE != 0 {
            [255, 70, 60]
        } else {
            [150, 150, 170]
        };
        let cx = (o.0.x.to_f32() - origin.0) / M;
        let cy = (o.0.y.to_f32() - origin.1) / M;
        let h = o.1 as f32 / 2.0 / M;
        for y in (cy - h) as i32..(cy + h) as i32 {
            for x in (cx - h) as i32..(cx + h) as i32 {
                put(x, y, c);
            }
        }
        if o.4 > 0.0 {
            let r = o.4 / M;
            for k in 0..720 {
                let a = k as f32 / 720.0 * std::f32::consts::TAU;
                put(
                    (cx + r * a.cos()) as i32,
                    (cy + r * a.sin()) as i32,
                    [60, 230, 255],
                );
            }
        }
    }
    let (sx, sy) = (N / 2, N / 2);
    for d in -6..=6 {
        put(sx + d, sy, [255, 255, 255]);
        put(sx, sy + d, [255, 255, 255]);
    }
    let mut out = format!("P6 {N} {N} 255\n").into_bytes();
    for px in img {
        out.extend_from_slice(&px);
    }
    std::fs::write(path, out).unwrap();
}
