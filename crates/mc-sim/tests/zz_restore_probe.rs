//! A long AI match on a real map, restored from a snapshot mid-way: the restored
//! world must step exactly as the original from then on. This is what a player
//! rejoining a network match relies on; the determinism matrix checks the same
//! on a small synthetic match, this on a big real one with AI commanders.
//!
//! `RESTORE=haldens_grip:8:4000:60 cargo test --release -p mc-sim --test sim -- zz_restore_probe:: --ignored --nocapture`
//! (map, seats, snapshot tick, ticks to compare after it; optional `:seed`). Prints the
//! first tick and state sections that differ, or that none did. Needs the baked map.
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_sim::state_hash::{differing, SECTIONS};
use mc_sim::tables::Controller;
use mc_sim::{AiConfig, Difficulty, MatchConfig, PlayerSetup, World};
use std::path::Path;
use std::sync::Arc;

#[test]
#[ignore = "probe: needs a baked map and minutes; see the header"]
fn a_restored_world_steps_like_the_original() {
    let spec = std::env::var("RESTORE").unwrap_or_else(|_| "haldens_grip:8:4000:60".into());
    let parts: Vec<&str> = spec.split(':').collect();
    let map_name = parts[0];
    let seats: usize = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(8);
    let at: u32 = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(4000);
    let after: u32 = parts.get(3).and_then(|s| s.parse().ok()).unwrap_or(60);
    let seed: u64 = parts.get(4).and_then(|s| s.parse().ok()).unwrap_or(7);

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let bps = Arc::new(Blueprints::load(&root.join("data")).unwrap());
    let map = mc_map::MapFile::open(root.join(format!("maps/{map_name}.mcmap"))).unwrap();
    let seats = seats.min(map.start_positions().len());
    let config = MatchConfig {
        seed,
        players: (0..seats)
            .map(|i| PlayerSetup {
                name: format!("AI {i}"),
                faction: "Aster".into(),
                ai: AiConfig {
                    difficulty: Difficulty::Hard,
                    ..AiConfig::default()
                },
                team: (i % 2) as u8,
                controller: Controller::Ai,
                start: i as u8,
            })
            .collect(),
        cheats: false,
        fog: true,
        spawn_commanders: true,
    };
    let pool = Arc::new(Pool::new(4));
    let mut original = World::new(&map, bps.clone(), pool.clone(), &config).unwrap();
    for t in 0..at {
        original.tick(&[]).unwrap();
        if t % 1000 == 0 {
            eprintln!("tick {t}: {} units", original.state.units.slots.live());
        }
    }
    let blob = original.snapshot();
    eprintln!("snapshot at tick {at}: {} bytes", blob.len());
    let mut restored = World::new(&map, bps, pool, &config).unwrap();
    restored
        .restore(mc_map::Heightfield::load(&map).unwrap(), &blob)
        .unwrap();
    let (a, b) = (original.hash_sections(), restored.hash_sections());
    assert!(
        differing(&a, &b).is_empty(),
        "differs straight after the restore: {:?}",
        differing(&a, &b)
    );
    for t in 0..after {
        let a = original.tick_sections(&[]).unwrap();
        let b = restored.tick_sections(&[]).unwrap();
        let apart = differing(&a, &b);
        assert!(
            apart.is_empty(),
            "{} ticks after the restore (tick {}), these sections differ: {apart:?} (of {SECTIONS:?})",
            t + 1,
            at + t + 1
        );
    }
    eprintln!("the restored world stepped like the original for {after} ticks");
}
