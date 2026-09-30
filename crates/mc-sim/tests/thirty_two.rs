//! A full house: `MAX_PLAYERS` (32) AI commanders on a basin baked with 32 starts
//! (Meridian Crown's layout, README.md, at a size a test can bake), eight teams
//! of four. Everyone gets a commander, allies share vision up to the last slot's
//! bit, the AI builds, and the match hashes the same at one worker and at four,
//! and after a snapshot is restored.

use mc_core::{player_bit, MAX_PLAYERS};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::{BakeParams, MapFile};
use mc_sim::tables::Controller;
use mc_sim::{MatchConfig, PlayerSetup, World};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

const TEAMS: usize = 8;
const TICKS: u32 = 300;
const SNAPSHOT_AT: u32 = 150;

/// A 16 km basin with a start for every seat, baked once per test process.
fn map() -> &'static Path {
    static MAP: OnceLock<PathBuf> = OnceLock::new();
    MAP.get_or_init(|| {
        let path =
            std::env::temp_dir().join(format!("mc_sim_thirty_two_{}.mcmap", std::process::id()));
        let params = BakeParams {
            players: MAX_PLAYERS as u32,
            ..BakeParams::square("Crown Test", 8, 11)
        };
        mc_map::bake(&params, &path).unwrap();
        path
    })
}

fn world(map: &MapFile, threads: usize) -> World {
    let blueprints =
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap();
    let config = MatchConfig {
        seed: 32,
        players: (0..MAX_PLAYERS)
            .map(|i| PlayerSetup {
                name: format!("AI {i}"),
                faction: "Aster".into(),
                ai: Default::default(),
                // Neighbours on the ring fight side by side.
                team: (i * TEAMS / MAX_PLAYERS) as u8,
                controller: Controller::Ai,
                start: i as u8,
            })
            .collect(),
        cheats: false,
        fog: true,
        spawn_commanders: true,
    };
    World::new(
        map,
        Arc::new(blueprints),
        Arc::new(Pool::new(threads)),
        &config,
    )
    .unwrap()
}

#[test]
fn thirty_two_ai_commanders_play_the_same_match_everywhere() {
    let map = MapFile::open(map()).unwrap();
    assert_eq!(map.start_positions().len(), MAX_PLAYERS);

    let mut one = world(&map, 1);
    let mut four = world(&map, 4);
    assert_eq!(one.state.players.len(), MAX_PLAYERS);
    for p in 0..MAX_PLAYERS as u8 {
        let commander = one.state.players[p as usize].commander;
        assert!(
            one.state.units.row(commander).is_some(),
            "player {p} has a commander"
        );
        assert_eq!(
            one.team_mask(p).count_ones(),
            (MAX_PLAYERS / TEAMS) as u32,
            "player {p}'s team is four"
        );
        assert_ne!(one.team_mask(p) & player_bit(p), 0);
    }
    assert_eq!(one.team_mask(MAX_PLAYERS as u8 - 1), 0xF000_0000);

    let mut snapshot = None;
    for t in 1..=TICKS {
        let (a, b) = (one.tick(&[]).unwrap(), four.tick(&[]).unwrap());
        assert_eq!(a, b, "tick {t}: one worker and four differ");
        if t == SNAPSHOT_AT {
            snapshot = Some(one.snapshot());
        }
    }
    // The AI got to work: every side has more than its commander.
    for p in 0..MAX_PLAYERS as u8 {
        let u = &one.state.units;
        let owned = u.slots.iter().filter(|&r| u.owner[r] == p).count();
        assert!(owned > 1, "player {p} built nothing in {TICKS} ticks");
    }
    // The last slot sees its own start: vision reaches bit 31.
    let last = one.state.players[MAX_PLAYERS - 1].start;
    assert!(one.fog.is_visible(last, player_bit(MAX_PLAYERS as u8 - 1)));

    let mut restored = world(&map, 2);
    restored
        .restore(mc_map::Heightfield::load(&map).unwrap(), &snapshot.unwrap())
        .unwrap();
    let mut again = world(&map, 1);
    for _ in 0..SNAPSHOT_AT {
        again.tick(&[]).unwrap();
    }
    for t in SNAPSHOT_AT + 1..=TICKS {
        assert_eq!(
            again.tick(&[]).unwrap(),
            restored.tick(&[]).unwrap(),
            "tick {t}: the restored world strays"
        );
    }
    assert_eq!(again.hash(), one.hash());
}
