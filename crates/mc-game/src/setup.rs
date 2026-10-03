//! Match set-up shared by the windowed game, the headless tools and the test scenes.

use mc_core::{Angle, FxVec2};
use mc_data::{BlueprintId, Blueprints};
use mc_map::MapFile;
use mc_sim::tables::Controller;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup};
use std::path::PathBuf;

/// A colour per player slot, linear RGB.
pub type Palette = [[f32; 3]; mc_core::MAX_PLAYERS];

/// Player colours by slot, linear RGB.
pub const TEAM_COLORS: Palette = [
    [0.05, 0.35, 1.0],
    [1.0, 0.08, 0.06],
    [0.1, 0.85, 0.2],
    [1.0, 0.75, 0.05],
    [0.65, 0.15, 1.0],
    [0.05, 0.85, 0.85],
    [1.0, 0.4, 0.05],
    [0.9, 0.9, 0.9],
    // Past the first eight: other hues, then darker and paler turns of them.
    [1.0, 0.22, 0.55],
    [0.55, 1.0, 0.08],
    [0.3, 0.6, 1.0],
    [0.45, 0.02, 0.04],
    [0.0, 0.42, 0.36],
    [0.55, 0.3, 0.04],
    [0.78, 0.62, 1.0],
    [0.3, 0.3, 0.33],
    [0.02, 0.32, 0.06],
    [1.0, 0.52, 0.42],
    [0.03, 0.08, 0.45],
    [0.42, 0.45, 0.04],
    [0.85, 0.02, 0.85],
    [0.42, 1.0, 0.65],
    [1.0, 0.8, 0.52],
    [0.32, 0.02, 0.36],
    [0.62, 1.0, 1.0],
    [0.62, 0.14, 0.02],
    [0.14, 0.28, 0.5],
    [1.0, 1.0, 0.45],
    [0.62, 0.08, 0.3],
    [0.08, 0.62, 0.45],
    [0.4, 0.32, 1.0],
    [0.06, 0.06, 0.07],
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Scene {
    /// Commanders and AI opponents: the actual game.
    Skirmish,
    /// Two pre-built armies meet in the middle of the map.
    Battle,
    /// Every player fields a large army: the full-load performance scene.
    Stress,
    /// One of every unit in a row, for looking at models.
    Showcase,
    /// Buildings standing against each other at the first start position, for their
    /// adjacency conduits: a Reactor III between a factory and a Fabricator III (bound to
    /// it), a Reactor I powering the fabricator, and a fabricator against the factory;
    /// ARC's yard, and the Regency's 240 m south of it.
    Adjacency,
    /// The battle staged behind the front end's menus.
    Backdrop,
    /// Two armies named on the command line (`--blue KEY:N`, `--red KEY:N`) meet at the
    /// map centre: any "this fight lags" report as one headless command.
    Matchup,
    /// The test range: one unit on a pad and a panel that does things to it.
    Range,
    /// A reclaimer tower and an idle engineer among wrecks and enemies that hold their fire.
    Reclaim,
    /// Idle engineers among wounded friends: they mend what they can reach.
    Repair,
    /// Repeating air Vs and a land block: transit, turn, then hover/hold.
    Formations,
    /// Repeated bombing passes over a heavy dome, for flight/effect inspection.
    Aircraft,
    /// A destroyed bomber falling from cruise altitude, for crash inspection.
    AircraftCrash,
    /// The same bomber destroyed over open sea: it strikes the water and sinks to the seabed.
    AircraftDitch,
    /// A yard of wrecks, one of each kind, all destroyed on the second tick: vehicles and
    /// structures where they stand, aircraft and spacecraft out of the sky, ships at sea.
    Wreckage,
    /// Structures built out on open sea: the anti-air emplacements on their floats, the
    /// Regency flak cannon, and the Wharf on its piles.
    Offshore,
    /// A tank block and a flight on patrol loops, a post added to each after the start.
    Patrol,
    /// Hovercraft, waders and a drowned tank off the coast west of the first
    /// start position, for looking at the water.
    Sea,
    /// Two small fleets on deep water near the first start position, in range of each
    /// other: a frigate, attack boats, a submarine and a sonar post a side.
    Naval,
    /// The same fleets holding their fire, for looking at the ships.
    NavalStill,
    /// The second fleet alone, holding its fire, and two Gannets sent in at it from
    /// 900 m west: torpedo runs to look at.
    TorpedoRun,
    /// A survival match on a survival map: the Replication Engine against one commander.
    Survival,
}

impl Scene {
    pub fn parse(s: &str) -> Option<Scene> {
        Some(match s {
            "skirmish" => Scene::Skirmish,
            "battle" => Scene::Battle,
            "stress" => Scene::Stress,
            "matchup" => Scene::Matchup,
            "showcase" => Scene::Showcase,
            "adjacency" => Scene::Adjacency,
            "backdrop" => Scene::Backdrop,
            "range" => Scene::Range,
            "reclaim" => Scene::Reclaim,
            "repair" => Scene::Repair,
            "formations" => Scene::Formations,
            "aircraft" => Scene::Aircraft,
            "aircraft-crash" => Scene::AircraftCrash,
            "aircraft-ditch" => Scene::AircraftDitch,
            "wreckage" => Scene::Wreckage,
            "offshore" => Scene::Offshore,
            "patrol" => Scene::Patrol,
            "sea" => Scene::Sea,
            "naval" => Scene::Naval,
            "naval-still" => Scene::NavalStill,
            "torpedo-run" => Scene::TorpedoRun,
            "survival" => Scene::Survival,
            _ => return None,
        })
    }
}

#[derive(Clone)]
pub struct Options {
    pub map: PathBuf,
    pub scene: Scene,
    pub players: usize,
    pub seed: u64,
    /// Units per player in the stress scene.
    pub army: u16,
    pub fog: bool,
    /// Watch an all-AI match: no human slot, the camera opens on the whole map.
    pub observe: bool,
    pub ai: mc_sim::AiConfig,
    /// The range's subject, a blueprint key, and the scenario it opens with.
    pub subject: String,
    pub scenario: Option<crate::range::Scenario>,
    /// Range shots: take this much (permille of full health) off the subject as it opens,
    /// so a damaged look can be staged without a fight.
    pub hurt: i16,
    /// The range's subject stands out on open water near the pad, as a structure built
    /// either way does at sea (its floats, its legs to the seabed).
    pub afloat: bool,
    /// Split the players into this many sides by where their zones lie; 0: everyone alone.
    pub teams: usize,
    /// The matchup scene's armies: side (0 blue, 1 red), blueprint key, count.
    pub matchup: Vec<(u8, String, u16)>,
    /// Play this recorded match instead of setting one up (`crate::replay`).
    pub replay: Option<PathBuf>,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            map: PathBuf::new(),
            scene: Scene::Skirmish,
            players: 2,
            seed: 1,
            army: 500,
            fog: true,
            observe: false,
            ai: Default::default(),
            subject: crate::range::DEFAULT_SUBJECT.into(),
            scenario: None,
            hurt: 0,
            afloat: false,
            teams: 0,
            matchup: Vec::new(),
            replay: None,
        }
    }
}

/// Where the range's subject stands: the first start position, which the baker keeps flat.
pub fn range_pad(map: &MapFile) -> FxVec2 {
    map.start_positions()
        .first()
        .copied()
        .unwrap_or(map.info().size_metres() * mc_core::Fx::HALF)
}

/// Where a headless range stands `subject`: on the pad, or out on open water near it for
/// a hull or a structure that is built only on water (as the windowed range lines them
/// up), so a ship is shot afloat, not sunk into the pad, or for anything when `afloat`.
fn subject_pad(
    map: &MapFile,
    blueprints: &Blueprints,
    subject: BlueprintId,
    afloat: bool,
) -> FxVec2 {
    let bp = blueprints.unit(subject);
    let wet = afloat
        || bp.water_only()
        || bp
            .motion
            .is_some_and(|m| m.layer == mc_data::MoveLayer::Naval);
    let pad = range_pad(map);
    if wet {
        sea_near(map, pad).unwrap_or(pad)
    } else {
        pad
    }
}

/// Open sea near the first start position, where the aircraft-ditch scene drops its bomber.
pub fn ditch_point(map: &MapFile) -> FxVec2 {
    open_sea(map, range_pad(map))
}

/// Open sea near `from` with room for a line of ships; `None` when the map has none.
pub fn sea_near(map: &MapFile, from: FxVec2) -> Option<FxVec2> {
    Some(open_sea(map, from)).filter(|&p| p != from)
}

/// The directories searched for `maps/`: the executable's directory and the
/// working directory, each with everything above it (a packaged build keeps
/// `maps/` beside the exe, and a shortcut may start it anywhere).
fn roots() -> Vec<PathBuf> {
    let exe = std::env::current_exe().ok();
    let exe_roots = exe.iter().flat_map(|e| e.ancestors().skip(1));
    let cwd = std::env::current_dir().ok();
    let cwd_roots = cwd.iter().flat_map(|d| d.ancestors());
    exe_roots
        .chain(cwd_roots)
        .map(|a| a.to_path_buf())
        .collect()
}

/// Every `.mcmap` in the nearest `maps/` directory, sorted by name.
pub fn list_maps() -> Vec<PathBuf> {
    let Some(dir) = roots()
        .into_iter()
        .map(|r| r.join("maps"))
        .find(|d| d.is_dir())
    else {
        return Vec::new();
    };
    let mut maps: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "mcmap"))
        .collect();
    maps.sort();
    maps
}

/// Which file in `maps/` holds each map content found so far: finding one
/// opens every map there, and the loading screen, the minimap and the match
/// each ask.
static MAP_PATHS: std::sync::Mutex<std::collections::BTreeMap<u64, PathBuf>> =
    std::sync::Mutex::new(std::collections::BTreeMap::new());

/// The settings file of `map` (`maps/<stem>.ron`: its weather and time of
/// day), found by matching the map's content against the files in `maps/`.
pub fn map_config(map: &MapFile) -> mc_data::weather::MapConfig {
    let known = MAP_PATHS
        .lock()
        .ok()
        .and_then(|m| m.get(&map.content_id()).cloned());
    let path = known.or_else(|| {
        let path = list_maps().into_iter().find(|path| {
            MapFile::open(path).is_ok_and(|other| other.content_id() == map.content_id())
        })?;
        if let Ok(mut m) = MAP_PATHS.lock() {
            m.insert(map.content_id(), path.clone());
        }
        Some(path)
    });
    let Some(path) = path else {
        return Default::default();
    };
    mc_data::weather::MapConfig::for_map(&path).unwrap_or_else(|e| {
        log::warn!("{e}; playing fair weather in the afternoon");
        Default::default()
    })
}

/// The map the front end stages its backdrop on: the island map if it is
/// there, otherwise the smallest map there is (it loads fastest).
pub fn backdrop_map() -> Option<PathBuf> {
    let maps = list_maps();
    let size = |p: &PathBuf| std::fs::metadata(p).map_or(u64::MAX, |m| m.len());
    maps.iter()
        .find(|p| p.file_stem().is_some_and(|s| s == "twin_shoals"))
        .or_else(|| maps.iter().min_by_key(|p| size(p)))
        .cloned()
}

pub fn find_map(name: Option<&str>) -> Result<PathBuf, String> {
    let candidates: Vec<PathBuf> = match name {
        Some(n) => vec![PathBuf::from(n), PathBuf::from(format!("maps/{n}.mcmap"))],
        None => vec![
            PathBuf::from("maps/dev16.mcmap"),
            PathBuf::from("maps/meridian_basin.mcmap"),
        ],
    };
    let roots = roots();
    for c in &candidates {
        if c.is_absolute() && c.exists() {
            return Ok(c.clone());
        }
        for root in &roots {
            if root.join(c).exists() {
                return Ok(root.join(c));
            }
        }
    }
    Err(format!(
        "no map found (looked for {candidates:?}). Bake one with: cargo run --release -p mc-map --bin mc-bake -- --size-km 16 --seed 7 -o maps/dev16.mcmap"
    ))
}

pub fn match_config(opts: &Options, map: &MapFile) -> MatchConfig {
    if opts.scene == Scene::Survival {
        match crate::survival::scene_match(opts, map) {
            Ok((config, _)) => return config,
            Err(e) => log::error!("{e}"),
        }
    }
    // The range is always blue against red.
    let wanted = if opts.scene == Scene::Range {
        2
    } else {
        opts.players
    };
    let count = wanted.clamp(
        1,
        map.start_positions().len().clamp(1, mc_core::MAX_PLAYERS),
    );
    let sides = match opts.teams {
        0 => (0..count as u8).collect(),
        n => {
            let zones: Vec<glam::Vec2> = map.start_positions()[..count]
                .iter()
                .map(|p| glam::Vec2::from(p.to_f32()))
                .collect();
            crate::ui::teams::by_ground(&zones, n)
        }
    };
    let players = (0..count)
        .map(|i| PlayerSetup {
            name: match (opts.scene, opts.observe, i) {
                (Scene::Range, _, 0) => "Blue".into(),
                (Scene::Range, _, _) => "Red".into(),
                (_, true, _) => format!("ARC AI {}", i + 1),
                (_, _, 0) => "Commander".into(),
                _ => format!("ARC AI {i}"),
            },
            faction: "Aster".into(),
            ai: opts.ai,
            team: sides[i],
            controller: if opts.observe {
                Controller::Ai
            } else if i == 0 || opts.scene != Scene::Skirmish {
                Controller::Human
            } else {
                Controller::Ai
            },
            start: i as u8,
        })
        .collect();
    MatchConfig {
        seed: opts.seed,
        players,
        cheats: opts.scene != Scene::Skirmish,
        fog: opts.fog && opts.scene == Scene::Skirmish,
        spawn_commanders: opts.scene == Scene::Skirmish,
    }
}

/// The `reclaim` scene: enemies placed farther north of the tower than this start as wrecks.
const RECLAIM_SCENE_NORTH: i32 = 70;

/// Commands for the first tick of a test scene.
pub fn opening_commands(
    opts: &Options,
    map: &MapFile,
    blueprints: &Blueprints,
    config: &MatchConfig,
) -> Vec<PlayerCommand> {
    let size = map.info().size_metres();
    let centre = size * mc_core::Fx::HALF;
    let id = |key: &str| blueprints.id_of(key).expect("blueprint exists");
    let spawn = |owner: u8, key: &str, pos: FxVec2, heading: Angle, count: u16| PlayerCommand {
        player: owner,
        command: Command::DebugSpawn {
            owner,
            blueprint: id(key),
            pos,
            heading,
            count,
            flags: 0,
            build: 1000,
        },
    };
    let mut out = Vec::new();
    match opts.scene {
        Scene::Aircraft => {
            let base = range_pad(map);
            out.push(PlayerCommand {
                player: 1,
                command: Command::DebugFreeBuild {
                    player: 1,
                    on: true,
                },
            });
            out.push(spawn(1, "aster_t3_shield", base, Angle::ZERO, 1));
            for i in 0..3 {
                out.push(spawn(
                    0,
                    "aster_t1_bomber",
                    base + FxVec2::from_ints(-350 - i * 24, (i - 1) * 28),
                    Angle::ZERO,
                    1,
                ));
            }
        }
        Scene::AircraftCrash => {
            out.push(spawn(0, "aster_t1_bomber", range_pad(map), Angle::ZERO, 1));
        }
        Scene::AircraftDitch => {
            out.push(spawn(
                0,
                "aster_t1_bomber",
                ditch_point(map),
                Angle::ZERO,
                1,
            ));
        }
        Scene::Wreckage => {
            let at = |x: i32, y: i32| range_pad(map) + FxVec2::from_ints(x, y);
            let yard = [
                // Vehicles where they stand, then structures.
                ("aster_t1_scout", at(-60, -40)),
                ("aster_t1_tank", at(-30, -40)),
                ("aster_t2_tank", at(0, -40)),
                ("aster_t3_artillery", at(35, -40)),
                ("aster_t4_assault_tank", at(90, -40)),
                ("aster_t1_power", at(-50, -110)),
                ("aster_t2_point_defense", at(0, -110)),
                ("aster_t1_land_factory", at(80, -120)),
                // Out of the sky: aircraft, then spacecraft.
                ("aster_t1_interceptor", at(-40, 40)),
                ("aster_t1_bomber", at(0, 40)),
                ("aster_t3_strategic_bomber", at(50, 40)),
                ("aster_t3_corvette", at(-150, 260)),
                ("aster_t3_lift_ship", at(200, 300)),
            ];
            for (key, pos) in yard {
                out.push(spawn(0, key, pos, Angle::ZERO, 1));
            }
            // Ships at sea, and a bomber over them.
            let sea = ditch_point(map);
            for (key, dx) in [
                ("aster_t1_frigate", -90),
                ("aster_t3_battleship", 20),
                ("aster_t1_bomber", 140),
            ] {
                out.push(spawn(
                    0,
                    key,
                    sea + FxVec2::from_ints(dx, 0),
                    Angle::ZERO,
                    1,
                ));
            }
        }
        Scene::Offshore => {
            let at = ditch_point(map);
            for (key, dx, dy) in [
                ("aster_t1_aa", -60, 80),
                ("aster_t2_aa", 0, 80),
                ("aster_t3_sam", 60, 80),
                ("regency_t1_aa", 0, -80),
                ("aster_t1_naval_factory", 40, -200),
            ] {
                out.push(spawn(
                    0,
                    key,
                    at + FxVec2::from_ints(dx, dy),
                    Angle::ZERO,
                    1,
                ));
            }
        }
        Scene::Patrol => {
            let base = range_pad(map);
            for (key, count, y) in [("aster_t1_interceptor", 5, 120), ("aster_t1_tank", 8, -60)] {
                for i in 0..count {
                    out.push(spawn(
                        0,
                        key,
                        base + FxVec2::from_ints(-200 + (i % 4) * 22, y + (i / 4) * 22),
                        Angle::ZERO,
                        1,
                    ));
                }
            }
        }
        Scene::Formations => {
            let base = range_pad(map);
            for (key, count, y) in [
                ("aster_t1_interceptor", 10, 100),
                ("aster_t1_bomber", 5, -30),
                ("aster_t1_tank", 9, -130),
            ] {
                for i in 0..count {
                    out.push(spawn(
                        0,
                        key,
                        base + FxVec2::from_ints(-240 + (i % 5) * 22, y + (i / 5) * 22),
                        Angle::ZERO,
                        1,
                    ));
                }
            }
        }
        Scene::Skirmish | Scene::Survival => {}
        Scene::Sea => {
            // Walk west from the start position to where the sea is 3 m deep.
            let base = map.start_positions().first().copied().unwrap_or(centre);
            let mut shore = base;
            if let Ok(field) = mc_map::Heightfield::load(map) {
                let deep = field.water_level() - mc_core::Fx::from_int(3);
                for step in 0..400 {
                    let p = base + FxVec2::from_ints(-20 * step, 0);
                    if field.height_at(p) < deep {
                        shore = p;
                        break;
                    }
                }
            }
            // Printed so a headless shot can be framed on it (`--camera X,Y,DIST`).
            eprintln!(
                "sea scene: shore at {:.0},{:.0}",
                shore.x.to_f32(),
                shore.y.to_f32()
            );
            for (key, x, y) in [
                ("aster_t2_hover", 0, 0),
                ("aster_t2_hover", -40, 30),
                ("aster_t2_hover", -80, -20),
                ("aster_commander", 25, 55),
                ("aster_t1_engineer", 30, -45),
                ("aster_t1_tank", -30, -70),
            ] {
                out.push(spawn(
                    0,
                    key,
                    shore + FxVec2::from_ints(x, y),
                    Angle::from_degrees(200),
                    1,
                ));
            }
        }
        Scene::TorpedoRun => {
            let base = map.start_positions().first().copied().unwrap_or(centre);
            let sea = open_sea(map, base);
            eprintln!(
                "torpedo run: fleet at {:.0},{:.0}",
                sea.x.to_f32(),
                sea.y.to_f32()
            );
            for (owner, key, x, y, flags) in [
                (1u8, "aster_t1_frigate", 0, 0, mc_sim::tables::flag::PASSIVE),
                (
                    1,
                    "aster_t1_attack_boat",
                    -40,
                    70,
                    mc_sim::tables::flag::PASSIVE,
                ),
                (
                    1,
                    "aster_t1_submarine",
                    60,
                    -80,
                    mc_sim::tables::flag::PASSIVE,
                ),
                (0, "aster_t2_torpedo_bomber", -900, 20, 0),
                (0, "aster_t2_torpedo_bomber", -920, -30, 0),
            ] {
                out.push(PlayerCommand {
                    player: owner,
                    command: Command::DebugSpawn {
                        owner,
                        blueprint: id(key),
                        pos: sea + FxVec2::from_ints(x, y),
                        heading: Angle::ZERO,
                        count: 1,
                        flags,
                        build: 1000,
                    },
                });
            }
        }
        Scene::Naval | Scene::NavalStill => {
            let base = map.start_positions().first().copied().unwrap_or(centre);
            let sea = open_sea(map, base);
            // Printed so a headless shot can be framed on it (`--camera X,Y,DIST`).
            eprintln!(
                "naval scene: fleets either side of {:.0},{:.0}",
                sea.x.to_f32(),
                sea.y.to_f32()
            );
            let flags = if opts.scene == Scene::NavalStill {
                mc_sim::tables::flag::PASSIVE
            } else {
                0
            };
            // Both fleets are paid: shields, sonar and interceptor lasers draw energy. The
            // stores are filled a tick later (`scene_orders`), once the storage exists.
            for player in 0..2u8 {
                out.push(PlayerCommand {
                    player,
                    command: Command::DebugStorage {
                        player,
                        mass: 20_000,
                        energy: 400_000,
                    },
                });
            }
            for (owner, side, heading) in
                [(0u8, -1, Angle::ZERO), (1u8, 1, Angle::from_degrees(180))]
            {
                for (key, x, y) in [
                    ("aster_t1_frigate", 190, 0),
                    ("aster_t1_attack_boat", 150, 60),
                    ("aster_t1_attack_boat", 150, -60),
                    ("aster_t1_submarine", 230, 90),
                    ("aster_t1_sonar", 300, -110),
                    // The capital ships astern: the battleship on the line, the missile
                    // ship and the strategic submarine off its quarters.
                    ("aster_t3_battleship", 420, 0),
                    ("aster_t2_missile_ship", 340, 80),
                    ("aster_t3_submarine", 380, -90),
                    // The escorts and the rest of the roster round them.
                    ("aster_t2_destroyer", 270, -50),
                    ("aster_t2_aa_cruiser", 310, 50),
                    ("aster_t2_submarine", 250, 150),
                    ("aster_t2_shield_boat", 350, -20),
                    ("aster_t3_carrier", 540, 120),
                    ("aster_t3_rail_trimaran", 600, -150),
                    ("aster_t1_mobile_reclaimer", 200, -170),
                ] {
                    out.push(PlayerCommand {
                        player: owner,
                        command: Command::DebugSpawn {
                            owner,
                            blueprint: id(key),
                            pos: sea + FxVec2::from_ints(side * x, side * y),
                            heading,
                            count: 1,
                            flags,
                            build: 1000,
                        },
                    });
                }
            }
        }
        Scene::Range => {
            let subject = blueprints
                .id_of(&opts.subject)
                .unwrap_or_else(|| id(crate::range::DEFAULT_SUBJECT));
            out.extend(
                crate::range::opening_commands(
                    blueprints,
                    subject_pad(map, blueprints, subject, opts.afloat),
                    subject,
                    opts.scenario,
                )
                .into_iter()
                .map(|command| PlayerCommand { player: 0, command }),
            );
        }
        Scene::Battle => {
            let gap = FxVec2::from_ints(420, 0);
            for (owner, side, heading) in [
                (0u8, centre - gap, Angle::ZERO),
                (1u8, centre + gap, Angle::HALF_TURN),
            ] {
                if owner as usize >= config.players.len() {
                    continue;
                }
                let back = if owner == 0 {
                    FxVec2::from_ints(-140, 0)
                } else {
                    FxVec2::from_ints(140, 0)
                };
                out.push(spawn(owner, "aster_t1_tank", side, heading, 48));
                out.push(spawn(
                    owner,
                    "aster_t2_tank",
                    side + FxVec2::from_ints(0, 220),
                    heading,
                    16,
                ));
                out.push(spawn(
                    owner,
                    "aster_t1_bot",
                    side - FxVec2::from_ints(0, 200),
                    heading,
                    30,
                ));
                out.push(spawn(owner, "aster_t1_artillery", side + back, heading, 12));
                out.push(spawn(
                    owner,
                    "aster_t2_missile",
                    side + back + back,
                    heading,
                    6,
                ));
                out.push(spawn(
                    owner,
                    "aster_t3_assault_bot",
                    side + back + FxVec2::from_ints(0, 160),
                    heading,
                    6,
                ));
                out.push(spawn(
                    owner,
                    "aster_commander",
                    side + back + back,
                    heading,
                    1,
                ));
            }
        }
        Scene::Matchup => {
            // 800 m apart across the centre, facing each other.
            let gap = FxVec2::from_ints(400, 0);
            // A spawn puts down one square block of at most `MAX_DEBUG_SPAWN`: a bigger
            // army (a stress test of thousands) is several blocks, one behind another.
            let block = mc_sim::command::MAX_DEBUG_SPAWN;
            // How far back each side's last block reaches (the first is centred on its spot).
            let mut depth: [Option<i32>; 2] = [None; 2];
            for (side, key, count) in &opts.matchup {
                let (pos, facing, back) = match side {
                    0 => (centre - gap, Angle::ZERO, -1),
                    _ => (centre + gap, Angle::HALF_TURN, 1),
                };
                let spacing = blueprints.unit(id(key)).radius.ceil_int() * 2 + 3;
                let mut left = (*count).max(1);
                while left > 0 {
                    let n = left.min(block);
                    let side_len = (n as f32).sqrt().ceil() as i32 * spacing;
                    let reach = &mut depth[(*side).min(1) as usize];
                    let middle = reach.map_or(0, |r| r + 20 + side_len / 2);
                    *reach = Some(middle + side_len / 2);
                    let at = pos + FxVec2::from_ints(back * middle, 0);
                    out.push(spawn(*side, key, at, facing, n));
                    left -= n;
                }
            }
        }
        Scene::Stress => {
            let n = config.players.len() as i32;
            for p in 0..n {
                let angle = Angle(((p as i64 * 65536) / n as i64) as u16);
                let ring = mc_core::Fx::from_int((size.x.floor_int() / 5).min(2500));
                let pos = centre + FxVec2::from_angle(angle) * ring;
                let facing = angle + Angle::HALF_TURN;
                let per = opts.army;
                out.push(spawn(p as u8, "aster_t1_tank", pos, facing, per / 2));
                out.push(spawn(
                    p as u8,
                    "aster_t1_bot",
                    pos + FxVec2::from_angle(angle + Angle::QUARTER_TURN)
                        * mc_core::Fx::from_int(500),
                    facing,
                    per / 4,
                ));
                out.push(spawn(
                    p as u8,
                    "aster_t2_tank",
                    pos - FxVec2::from_angle(angle + Angle::QUARTER_TURN)
                        * mc_core::Fx::from_int(500),
                    facing,
                    per / 8,
                ));
                out.push(spawn(
                    p as u8,
                    "aster_t1_artillery",
                    pos + FxVec2::from_angle(angle) * mc_core::Fx::from_int(300),
                    facing,
                    per / 8,
                ));
            }
        }
        Scene::Backdrop => {
            // Two combined-arms groups either side of contested ground, close
            // enough that the shooting starts within seconds of the menu appearing.
            let (site, along) = crate::ui::backdrop::battle_site(map);
            let at =
                |v: glam::Vec2| FxVec2::new(mc_core::Fx::from_f32(v.x), mc_core::Fx::from_f32(v.y));
            for (owner, side) in [(0u8, -1.0f32), (1u8, 1.0)] {
                let facing = -along * side;
                let heading = Angle::from_degrees(facing.y.atan2(facing.x).to_degrees() as i32);
                let front = site + along * side * 330.0;
                let rear = front + along * side * 150.0;
                let flank = along.perp() * 130.0;
                out.push(spawn(owner, "aster_t1_tank", at(front), heading, 36));
                out.push(spawn(owner, "aster_t1_bot", at(front + flank), heading, 20));
                out.push(spawn(
                    owner,
                    "aster_t2_tank",
                    at(front - flank),
                    heading,
                    10,
                ));
                out.push(spawn(owner, "aster_t2_hover", at(rear - flank), heading, 6));
                out.push(spawn(owner, "aster_t1_artillery", at(rear), heading, 8));
                out.push(spawn(
                    owner,
                    "aster_t3_assault_bot",
                    at(rear + flank),
                    heading,
                    4,
                ));
            }
        }
        Scene::Reclaim => {
            // On the flat ground of the first start position. On the second tick
            // (`scene_orders`) the enemies up north are wrecked, for the engineer beside them
            // to clear by itself, and the tower is told to take the nearest one to its east apart.
            let base = map.start_positions().first().copied().unwrap_or(centre);
            let held = |key: &str, at: (i32, i32)| PlayerCommand {
                player: 0,
                command: Command::DebugSpawn {
                    owner: 1,
                    blueprint: id(key),
                    pos: base + FxVec2::from_ints(at.0, at.1),
                    heading: Angle::HALF_TURN,
                    count: 1,
                    flags: mc_sim::tables::flag::PASSIVE,
                    build: 1000,
                },
            };
            out.push(spawn(0, "aster_t2_reclaimer", base, Angle::ZERO, 1));
            out.push(spawn(
                0,
                "aster_mass_storage",
                base + FxVec2::from_ints(-96, -64),
                Angle::ZERO,
                1,
            ));
            out.push(spawn(
                0,
                "aster_t1_engineer",
                base + FxVec2::from_ints(-40, RECLAIM_SCENE_NORTH + 10),
                Angle::QUARTER_TURN,
                1,
            ));
            out.extend([
                held("aster_t2_tank", (120, 30)),
                held("aster_t1_tank", (95, -60)),
                held("aster_t1_tank", (150, -20)),
            ]);
            out.extend([
                held("aster_t1_tank", (-70, RECLAIM_SCENE_NORTH + 50)),
                held("aster_t2_tank", (-25, RECLAIM_SCENE_NORTH + 60)),
                held("aster_t1_tank", (40, RECLAIM_SCENE_NORTH + 30)),
            ]);
        }
        Scene::Repair => {
            // Idle masons among wounded friends: they mend what they can
            // reach. Storage so the stitch has mass and energy to spend.
            let base = map.start_positions().first().copied().unwrap_or(centre);
            let friend = |key: &str, at: (i32, i32)| PlayerCommand {
                player: 0,
                command: Command::DebugSpawn {
                    owner: 0,
                    blueprint: id(key),
                    pos: base + FxVec2::from_ints(at.0, at.1),
                    heading: Angle::ZERO,
                    count: 1,
                    flags: mc_sim::tables::flag::PASSIVE,
                    build: 1000,
                },
            };
            out.push(spawn(
                0,
                "aster_mass_storage",
                base + FxVec2::from_ints(-120, -80),
                Angle::ZERO,
                1,
            ));
            out.push(spawn(
                0,
                "aster_energy_storage",
                base + FxVec2::from_ints(-72, -80),
                Angle::ZERO,
                1,
            ));
            out.push(spawn(
                0,
                "aster_t1_engineer",
                base + FxVec2::from_ints(-20, 10),
                Angle::ZERO,
                1,
            ));
            out.push(spawn(
                0,
                "aster_t1_engineer",
                base + FxVec2::from_ints(10, -16),
                Angle::from_degrees(40),
                1,
            ));
            out.extend([
                friend("aster_t1_tank", (36, 18)),
                friend("aster_t1_tank", (52, -22)),
                friend("aster_t1_bot", (-8, 40)),
                friend("aster_t2_tank", (80, 8)),
            ]);
        }
        Scene::Adjacency => {
            // Lots are 12 m build cells (`mc_map::BUILD_CELL_M`): a reactor III and a
            // factory 96 m across, fabricators and a reactor I 24 m.
            let base = map.start_positions().first().copied().unwrap_or(centre);
            // The same yard twice: ARC's, and the Regency's 240 m south of it.
            for (faction, south) in [("aster", 0), ("regency", 240)] {
                for (kind, x, y) in [
                    ("t3_power", 0, 0),
                    ("t3_land_factory", 96, 0),
                    ("t3_fabricator", -60, 12),
                    ("t1_power", -84, 12),
                    ("t3_fabricator", 72, 60),
                ] {
                    out.push(spawn(
                        0,
                        &format!("{faction}_{kind}"),
                        base + FxVec2::from_ints(x, y - south),
                        Angle::from_degrees(270),
                        1,
                    ));
                }
            }
        }
        Scene::Showcase => {
            // Laid out on the flat ground of the first start position: mobile units in
            // front, structures in a row behind them.
            let base = map.start_positions().first().copied().unwrap_or(centre);
            let (mut mobile_x, mut structure_x) = (-280, -560);
            for bp in blueprints
                .units
                .iter()
                .filter(|bp| blueprints.is_listed(bp.id))
            {
                if bp.is_structure() {
                    out.push(spawn(
                        0,
                        &bp.key,
                        base + FxVec2::from_ints(structure_x, 176),
                        Angle::from_degrees(270),
                        1,
                    ));
                    structure_x += bp.footprint.0 as i32 * mc_map::BUILD_CELL_M + 48;
                } else {
                    out.push(spawn(
                        0,
                        &bp.key,
                        base + FxVec2::from_ints(mobile_x, 0),
                        Angle::from_degrees(250),
                        1,
                    ));
                    mobile_x += 40;
                }
            }
        }
    }
    out
}

/// The wreckage scene's lift ship is shot down this many ticks in, up in the clouds by then.
pub const WRECKAGE_LATE_KILL: u32 = 450;

/// Orders later in a headless run (`headless.rs`), at tick `t`: the wreckage scene shoots
/// down what took off from its lot, so it breaks up in the sky.
pub fn late_orders(opts: &Options, world: &mc_sim::World, t: u32) -> Vec<PlayerCommand> {
    let u = &world.state.units;
    if opts.scene == Scene::Range
        && opts.scenario == Some(crate::range::Scenario::Destruct)
        && t == 1 + crate::range::DESTRUCT_WAIT
    {
        // The subject stands a moment before it goes, so a film sees it go.
        let subject = world.blueprints.id_of(&opts.subject);
        return vec![PlayerCommand {
            player: 0,
            command: Command::SelfDestruct {
                timed: false,
                units: u
                    .slots
                    .iter()
                    .filter(|&r| u.owner[r] == 0 && Some(u.blueprint[r]) == subject)
                    .map(|r| u.id(r))
                    .collect(),
            },
        }];
    }
    if opts.scene != Scene::Wreckage || t != WRECKAGE_LATE_KILL {
        return Vec::new();
    }
    vec![PlayerCommand {
        player: 0,
        command: Command::SelfDestruct {
            timed: false,
            units: u.slots.iter().map(|r| u.id(r)).collect(),
        },
    }]
}

/// Follow-up orders for a scene once its units exist (second tick).
pub fn scene_orders(
    opts: &Options,
    map: &MapFile,
    blueprints: &Blueprints,
    world: &mc_sim::World,
) -> Vec<PlayerCommand> {
    let u = &world.state.units;
    if matches!(opts.scene, Scene::Naval | Scene::NavalStill) {
        return (0..2u8)
            .map(|player| PlayerCommand {
                player,
                command: Command::DebugStock {
                    player,
                    mass: Some(1000),
                    energy: Some(1000),
                },
            })
            .collect();
    }
    if matches!(opts.scene, Scene::AircraftCrash | Scene::AircraftDitch) {
        return vec![PlayerCommand {
            player: 0,
            command: Command::SelfDestruct {
                timed: false,
                units: u.slots.iter().map(|r| u.id(r)).collect(),
            },
        }];
    }
    if opts.scene == Scene::Wreckage {
        // Everything but what is raised on a lot: that lifts off first (`late_orders`).
        return vec![PlayerCommand {
            player: 0,
            command: Command::SelfDestruct {
                timed: false,
                units: u
                    .slots
                    .iter()
                    .filter(|&r| !blueprints.unit(u.blueprint[r]).is_site_built_unit())
                    .map(|r| u.id(r))
                    .collect(),
            },
        }];
    }
    if opts.scene == Scene::TorpedoRun {
        let Some(fleet) = u.slots.iter().find(|&r| u.owner[r] == 1).map(|r| u.pos[r]) else {
            return Vec::new();
        };
        return vec![PlayerCommand {
            player: 0,
            command: Command::AttackMove {
                units: u
                    .slots
                    .iter()
                    .filter(|&r| u.owner[r] == 0)
                    .map(|r| u.id(r))
                    .collect(),
                target: fleet,
                queue: false,
            },
        }];
    }
    if opts.scene == Scene::Aircraft {
        let target = u.slots.iter().find(|&r| u.owner[r] == 1).map(|r| u.id(r));
        return target
            .map(|target| PlayerCommand {
                player: 0,
                command: Command::Attack {
                    units: u
                        .slots
                        .iter()
                        .filter(|&r| u.owner[r] == 0)
                        .map(|r| u.id(r))
                        .collect(),
                    target,
                    queue: false,
                },
            })
            .into_iter()
            .collect();
    }
    if opts.scene == Scene::Patrol {
        let base = range_pad(map);
        let mut commands = Vec::new();
        for (key, route) in [
            ("aster_t1_tank", [(0, -60), (60, 60), (-120, 110)]),
            (
                "aster_t1_interceptor",
                [(300, 120), (300, 500), (-100, 500)],
            ),
        ] {
            let bp = blueprints.id_of(key).unwrap();
            let units: Vec<_> = u
                .slots
                .iter()
                .filter(|&r| u.blueprint[r] == bp)
                .map(|r| u.id(r))
                .collect();
            let posts: Vec<FxVec2> = route
                .iter()
                .map(|&(x, y)| base + FxVec2::from_ints(x, y))
                .collect();
            let mut command = |command| commands.push(PlayerCommand { player: 0, command });
            // As the player lays it: one click patrols, each shift-click adds a post.
            command(Command::Patrol {
                units: units.clone(),
                points: vec![posts[0]],
                queue: false,
            });
            for pair in posts.windows(2) {
                command(Command::PatrolInsert {
                    units: units.clone(),
                    after: pair[0],
                    point: pair[1],
                });
            }
        }
        return commands;
    }
    if opts.scene == Scene::Formations {
        let base = range_pad(map);
        let mut commands = Vec::new();
        for (key, y) in [
            ("aster_t1_interceptor", 100),
            ("aster_t1_bomber", -30),
            ("aster_t1_tank", -130),
        ] {
            let bp = blueprints.id_of(key).unwrap();
            let units: Vec<_> = u
                .slots
                .iter()
                .filter(|&r| u.blueprint[r] == bp)
                .map(|r| u.id(r))
                .collect();
            for (at, queue) in [
                (base + FxVec2::from_ints(60, y - 100), false),
                (base + FxVec2::from_ints(140, y + 90), true),
            ] {
                commands.push(PlayerCommand {
                    player: 0,
                    command: Command::Move {
                        units: units.clone(),
                        target: at,
                        queue,
                    },
                });
            }
        }
        return commands;
    }
    if opts.scene == Scene::Range {
        let subject = blueprints.id_of(&opts.subject).unwrap_or_else(|| {
            blueprints
                .id_of(crate::range::DEFAULT_SUBJECT)
                .expect("blueprint exists")
        });
        let blue: Vec<_> = u
            .slots
            .iter()
            .filter(|&r| u.owner[r] == 0)
            .map(|r| (u.blueprint[r], u.id(r)))
            .collect();
        let mut owed = crate::range::owed_commands(
            blueprints,
            subject_pad(map, blueprints, subject, opts.afloat),
            subject,
            opts.scenario,
            &blue,
        );
        if opts.hurt > 0 {
            owed.push(Command::DebugDamage {
                units: blue
                    .iter()
                    .filter(|(b, _)| *b == subject)
                    .map(|(_, id)| *id)
                    .collect(),
                permille: opts.hurt,
            });
        }
        return owed
            .into_iter()
            .map(|command| PlayerCommand { player: 0, command })
            .collect();
    }
    if opts.scene == Scene::Reclaim {
        let base = map
            .start_positions()
            .first()
            .copied()
            .unwrap_or(map.info().size_metres() * mc_core::Fx::HALF);
        let doomed: Vec<_> = u
            .slots
            .iter()
            .filter(|&r| {
                u.owner[r] == 1 && u.pos[r].y > base.y + mc_core::Fx::from_int(RECLAIM_SCENE_NORTH)
            })
            .map(|r| u.id(r))
            .collect();
        let north = base.y + mc_core::Fx::from_int(RECLAIM_SCENE_NORTH);
        let towers: Vec<_> = u
            .slots
            .iter()
            .filter(|&r| world.bp(r).reclaimer.is_some())
            .map(|r| u.id(r))
            .collect();
        let quarry = u
            .slots
            .iter()
            .filter(|&r| u.owner[r] == 1 && u.pos[r].y <= north)
            .min_by_key(|&r| u.pos[r].distance(base))
            .map(|r| u.id(r));
        let mut out = vec![PlayerCommand {
            player: 0,
            command: Command::DebugDamage {
                units: doomed,
                permille: 1000,
            },
        }];
        out.extend(quarry.map(|target| PlayerCommand {
            player: 0,
            command: Command::ReclaimUnit {
                units: towers,
                target,
                queue: false,
            },
        }));
        return out;
    }
    if opts.scene == Scene::Repair {
        let wounded: Vec<_> = u
            .slots
            .iter()
            .filter(|&r| {
                u.owner[r] == 0
                    && world.bp(r).is_mobile()
                    && !world.bp(r).has(mc_data::cat::ENGINEER)
            })
            .map(|r| u.id(r))
            .collect();
        return vec![
            PlayerCommand {
                player: 0,
                command: Command::DebugFreeBuild {
                    player: 0,
                    on: true,
                },
            },
            PlayerCommand {
                player: 0,
                command: Command::DebugDamage {
                    units: wounded,
                    permille: 620,
                },
            },
        ];
    }
    if !matches!(
        opts.scene,
        Scene::Battle | Scene::Stress | Scene::Backdrop | Scene::Matchup
    ) {
        return Vec::new();
    }
    let centre = if opts.scene == Scene::Backdrop {
        let site = crate::ui::backdrop::battle_site(map).0;
        FxVec2::new(mc_core::Fx::from_f32(site.x), mc_core::Fx::from_f32(site.y))
    } else {
        map.info().size_metres() * mc_core::Fx::HALF
    };
    let mut by_owner: std::collections::BTreeMap<u8, Vec<mc_sim::UnitId>> = Default::default();
    for row in u.slots.iter() {
        if world.bp(row).is_mobile() {
            by_owner.entry(u.owner[row]).or_default().push(u.id(row));
        }
    }
    let mut out = Vec::new();
    for (owner, ids) in by_owner {
        for chunk in ids.chunks(mc_sim::command::MAX_COMMAND_UNITS) {
            out.push(PlayerCommand {
                player: owner,
                command: Command::AttackMove {
                    units: chunk.to_vec(),
                    target: centre,
                    queue: false,
                },
            });
        }
    }
    out
}

/// The nearest point to `from` with deep water 360 m either way along x and 140 m
/// along y, room for two fleets facing each other. `from` itself if there is none.
fn open_sea(map: &MapFile, from: FxVec2) -> FxVec2 {
    let Ok(field) = mc_map::Heightfield::load(map) else {
        return from;
    };
    let deep = field.water_level() - mc_core::Fx::from_int(14);
    let size = map.info().size_metres();
    let open = |p: FxVec2| {
        (-9..=9).all(|i| {
            (-2..=2).all(|j| {
                let q = p + FxVec2::from_ints(i * 40, j * 70);
                q.x > mc_core::Fx::ZERO
                    && q.y > mc_core::Fx::ZERO
                    && q.x < size.x
                    && q.y < size.y
                    && field.height_at(q) < deep
            })
        })
    };
    // Rings of candidates outward from `from`, 60 m apart, nearest first.
    for ring in 0..120 {
        let steps = (ring * 8).max(1);
        for s in 0..steps {
            let a = std::f32::consts::TAU * s as f32 / steps as f32;
            let r = 60.0 * ring as f32;
            let p = from + FxVec2::from_ints((a.cos() * r) as i32, (a.sin() * r) as i32);
            if open(p) {
                return p;
            }
        }
    }
    from
}
