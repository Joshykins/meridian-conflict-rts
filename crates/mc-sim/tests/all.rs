//! Every mc-sim integration test, in one test binary: one link instead of one
//! per file, and all of them share one pool of test threads. A new test file
//! goes in the list below (`every_test_file_is_listed` fails otherwise);
//! `perf_budgets.rs` is its own binary, since its time budgets need a quiet
//! machine.
//!
//! One file's tests: `cargo test --release -p mc-sim --test sim -- <file>::`.

mod air_bombing;
mod air_guard;
mod air_landing;
mod air_patrol_smooth;
mod air_refinements;
mod air_roster;
mod aircraft_crash;
mod atoll;
mod battle;
mod bore_and_seabed;
mod broadside;
mod build_line;
mod citadel;
mod combat;
mod commander;
mod crowd;
mod culverin;
mod deck_up;
mod determinism;
mod determinism_gate;
mod drone_port;
mod economy;
mod engineer_upgrade;
mod factions;
mod factory;
mod factory_orders;
mod fog;
mod formation_terrain;
mod formations;
mod frigate;
mod guard;
mod javelin_role;
mod lift_ship;
mod line_of_fire;
mod map_wreckage;
mod mines;
mod missile_defense;
mod naga;
mod narwhal;
mod naval;
mod naval_roster;
mod nuke_salvo;
mod nukes;
mod path_budget;
mod patrol;
mod pause;
mod place;
mod player_orders;
mod radar;
mod range;
mod reclaim;
mod reclaim_heads;
mod reform;
mod repair;
mod shield;
mod site_map;
mod skyguard;
mod straight_moves;
mod stream;
mod survival;
mod titan;
mod titan_water;
mod torpedo_bomber;
mod trees;
mod untrusted_input;
mod volatile;
mod warp;
mod zz_aa_probe;
mod zz_ai_duel_probe;
mod zz_ai_layout_probe;
mod zz_ai_stall_probe;
mod zz_ai_stream_probe;
mod zz_ai_threat_probe;
mod zz_naval_duel_probe;
mod zz_petrel_probe;
mod zz_restore_probe;
mod zz_tempest_probe;

#[test]
fn every_test_file_is_listed() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let listed = include_str!("all.rs");
    for entry in std::fs::read_dir(&dir).unwrap().flatten() {
        let path = entry.path();
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if path.extension().is_some_and(|e| e == "rs") && !["all", "perf_budgets"].contains(&stem) {
            assert!(
                listed.contains(&format!("\nmod {stem};")),
                "tests/{stem}.rs is not compiled: add `mod {stem};` to tests/all.rs"
            );
        }
    }
}
