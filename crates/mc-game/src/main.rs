//! `meridian`: the game, and the tools and test scenes that run on the same runtime.

mod app;
mod audio;
mod game;
mod headless;
mod hud;
mod orders;
mod pointer;
mod range;
mod rings;
mod settings;
mod setup;
mod sim_thread;
mod ui;

use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::MapFile;
use setup::{Options, Scene};
use std::sync::Arc;

const USAGE: &str = "\
meridian [options]

With no match options the game opens its front end: main menu, skirmish set-up,
settings. Any of --map, --scene, --players, --seed, --army or --connect goes
straight into a match instead.

  --map NAME|PATH        map to play (default: maps/dev16.mcmap, else maps/meridian_basin.mcmap)
  --scene NAME           skirmish (default) | battle | stress | showcase | range | reclaim
  --range                the test range (same as --scene range): one unit on a pad and a
                         panel to attack it, destroy it, scrub its build state, have it
                         built, give it targets, and reset
  --unit KEY             the range's subject, a blueprint key (default aster_t1_tank)
  --scenario NAME        open the range with a scenario staged: under-fire | targets | build
  --players N            player slots, 1-8 (default 2; slot 0 is you, the rest are AI)
  --army N               units per player in the stress scene (default 500)
  --seed N               match seed
  --no-fog               reveal the map
  --no-vsync             present as fast as possible (for measuring frame rate)
  --connect HOST:PORT    join a network match on an mc-relay (the first to join hosts;
                         the host's --map/--players/--seed define the match)
  --name NAME            your name in a network match
  --threads N            worker threads (default: all cores)
  --bench TICKS          run the scene headless and print sim timings
  --screenshot FILE.png  render one frame headless after --ticks and exit
  --ticks N              ticks to simulate before a screenshot (default 0)
  --camera X,Y,DIST[,YAW]  screenshot camera: focus in metres, eye distance, yaw in degrees
  --size WxH             screenshot size (default 1920x1080)
  --select KEY           match screenshot: select player 0's first unit whose blueprint key
                         contains KEY (all of them with a trailing *), not the commander
  --paused               match screenshot: show the pause card
  --plans                match screenshot: the commander has structures planned and a way to
                         walk, and shift is held: ghosts, order lines, the order under --cursor
  --drag X,Y             with --plans: the order under --cursor has been dragged to this pixel
  --follow N             match screenshot: play N more ticks through the renderer first,
                         so smoke, dust, track marks and shells in flight are in the picture
  --alpha A              with --follow: how far into the last tick the kept frame is (0..1)
  --ui SCREEN            with --screenshot: draw a front-end screen instead of a match:
                         menu | skirmish | settings
  --cursor X,Y           with --ui: where the pointer is, in pixels
  --smoke                open the front end, play a default skirmish for a few seconds, return
                         to the front end and exit: an unattended check of every stage change
  --dump-sounds DIR      write the synthesised sound set as WAV files and exit
  --dump-cursors FILE.png  write every mouse pointer, over dark, grass and bright ground, and exit
";

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    if let Err(e) = run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let mut map_name: Option<String> = None;
    let mut opts = Options::default();
    let mut threads: Option<usize> = None;
    let mut bench: Option<u32> = None;
    let mut ticks = 0u32;
    let mut shot: Option<headless::Shot> = None;
    let mut camera = None;
    let mut size = (1920u32, 1080u32);
    let mut vsync = true;
    let mut connect: Option<String> = None;
    let mut name = String::from("Commander");
    // Set by any option that describes a match: skip the front end.
    let mut direct = false;
    let mut ui_screen: Option<ui::front::Screen> = None;
    let mut cursor: Option<[f32; 2]> = None;
    let mut smoke = false;
    let mut dump_sounds: Option<String> = None;
    let mut select: Option<String> = None;
    let mut paused = false;
    let (mut plans, mut drag): (bool, Option<[f32; 2]>) = (false, None);
    let (mut follow, mut alpha) = (0u32, 1.0f32);

    while let Some(arg) = args.next() {
        let mut value = |name: &str| {
            args.next()
                .ok_or(format!("{name} needs a value\n\n{USAGE}"))
        };
        direct |= matches!(
            arg.as_str(),
            "--map"
                | "--scene"
                | "--players"
                | "--army"
                | "--seed"
                | "--connect"
                | "--no-fog"
                | "--range"
        );
        match arg.as_str() {
            "--map" => map_name = Some(value("--map")?),
            "--scene" => opts.scene = Scene::parse(&value("--scene")?).ok_or("unknown scene")?,
            "--range" => opts.scene = Scene::Range,
            "--unit" => opts.subject = value("--unit")?,
            "--scenario" => opts.scenario = Some(range::Scenario::parse(&value("--scenario")?).ok_or("--scenario takes under-fire, close, targets, build, work, upgrade, march or destruct")?),
            "--players" => opts.players = value("--players")?.parse().map_err(|_| "--players takes a number")?,
            "--army" => opts.army = value("--army")?.parse().map_err(|_| "--army takes a number")?,
            "--seed" => opts.seed = value("--seed")?.parse().map_err(|_| "--seed takes a number")?,
            "--no-fog" => opts.fog = false,
            "--no-vsync" => vsync = false,
            "--connect" => connect = Some(value("--connect")?),
            "--name" => name = value("--name")?,
            "--threads" => threads = Some(value("--threads")?.parse().map_err(|_| "--threads takes a number")?),
            "--bench" => bench = Some(value("--bench")?.parse().map_err(|_| "--bench takes a tick count")?),
            "--ticks" => ticks = value("--ticks")?.parse().map_err(|_| "--ticks takes a number")?,
            "--screenshot" => shot = Some(headless::Shot { path: value("--screenshot")?, camera: None, width: 0, height: 0, select: None, cursor: None, paused: false, plans: false, drag: None, follow: 0, alpha: 1.0 }),
            "--camera" => {
                let v: Vec<f32> = value("--camera")?.split(',').filter_map(|p| p.trim().parse().ok()).collect();
                if v.len() < 3 {
                    return Err("--camera takes X,Y,DIST[,YAW]".into());
                }
                camera = Some([v[0], v[1], v[2], v.get(3).copied().unwrap_or(0.0)]);
            }
            "--size" => {
                let v = value("--size")?;
                let (w, h) = v.split_once('x').ok_or("--size takes WxH")?;
                size = (w.parse().map_err(|_| "--size takes WxH")?, h.parse().map_err(|_| "--size takes WxH")?);
            }
            "--ui" => ui_screen = Some(ui::front::Screen::parse(&value("--ui")?).ok_or("--ui takes menu, skirmish or settings")?),
            "--cursor" => {
                let v: Vec<f32> = value("--cursor")?.split(',').filter_map(|p| p.trim().parse().ok()).collect();
                cursor = Some([*v.first().ok_or("--cursor takes X,Y")?, *v.get(1).ok_or("--cursor takes X,Y")?]);
            }
            "--select" => select = Some(value("--select")?),
            "--paused" => paused = true,
            "--plans" => plans = true,
            "--drag" => {
                let v: Vec<f32> = value("--drag")?.split(',').filter_map(|p| p.trim().parse().ok()).collect();
                drag = Some([*v.first().ok_or("--drag takes X,Y")?, *v.get(1).ok_or("--drag takes X,Y")?]);
            }
            "--follow" => follow = value("--follow")?.parse().map_err(|_| "--follow takes a number of ticks")?,
            "--alpha" => alpha = value("--alpha")?.parse::<f32>().map_err(|_| "--alpha takes a number from 0 to 1")?.clamp(0.0, 1.0),
            "--smoke" => smoke = true,
            "--dump-sounds" => dump_sounds = Some(value("--dump-sounds")?),
            "--dump-cursors" => {
                let (rgba, width, height) = pointer::sheet(2.0);
                return headless::write_png(std::path::Path::new(&value("--dump-cursors")?), width, height, &rgba);
            }
            "-h" | "--help" => {
                print!("{USAGE}");
                return Ok(());
            }
            other => return Err(format!("unknown option {other}\n\n{USAGE}")),
        }
    }

    let data_dir = Blueprints::locate_data_dir().ok_or(
        "could not find the data/ directory next to the executable or above the working directory",
    )?;
    let blueprints = Arc::new(Blueprints::load(&data_dir).map_err(|e| e.to_string())?);
    // The sound library is data too, and a unit file naming a sound that is not in it is an error here, not silence later.
    let sounds = mc_data::SoundLibrary::load(&data_dir).map_err(|e| e.to_string())?;
    sounds.check(&blueprints).map_err(|e| e.to_string())?;
    if let Some(dir) = dump_sounds {
        return audio::dump(std::path::Path::new(&dir), &sounds);
    }
    let pool = Arc::new(match threads {
        Some(n) => Pool::new(n),
        None => Pool::with_default_threads(),
    });

    if opts.scene == Scene::Range && blueprints.id_of(&opts.subject).is_none() {
        let keys: Vec<&str> = blueprints.units.iter().map(|u| u.key.as_str()).collect();
        return Err(format!(
            "--unit {:?} is not a unit. The units are: {}",
            opts.subject,
            keys.join(", ")
        ));
    }

    if let Some(screen) = ui_screen {
        let mut shot = shot
            .take()
            .ok_or("--ui draws a screenshot: give it --screenshot FILE.png")?;
        (shot.width, shot.height) = size;
        return headless::ui_screenshot(screen, blueprints, pool, ticks, &shot, cursor);
    }
    if !direct && bench.is_none() && shot.is_none() {
        log::info!(
            "{} blueprints, {} worker threads",
            blueprints.units.len(),
            pool.thread_count()
        );
        return app::run(app::AppArgs {
            blueprints,
            sounds,
            pool,
            force_no_vsync: !vsync,
            direct: None,
            smoke,
        });
    }

    opts.map = setup::find_map(map_name.as_deref())?;
    let map =
        Arc::new(MapFile::open(&opts.map).map_err(|e| format!("{}: {e}", opts.map.display()))?);
    log::info!(
        "map {:?} ({} x {} tiles), {} blueprints, {} worker threads",
        map.name(),
        map.size_tiles().0,
        map.size_tiles().1,
        blueprints.units.len(),
        pool.thread_count()
    );
    log::debug!(
        "start positions: {:?}",
        map.start_positions()
            .iter()
            .map(|p| p.to_f32())
            .collect::<Vec<_>>()
    );

    if let Some(ticks) = bench {
        headless::run_sim(&opts, &map, &blueprints, &pool, ticks, true)?;
        return Ok(());
    }
    if let Some(mut shot) = shot {
        shot.camera = camera;
        (shot.width, shot.height) = size;
        (shot.select, shot.cursor, shot.paused) = (select, cursor, paused);
        (shot.follow, shot.alpha) = (follow, alpha);
        (shot.plans, shot.drag) = (plans, drag);
        return headless::screenshot(&opts, map, blueprints, pool, ticks, &shot);
    }

    let config = setup::match_config(&opts, &map);
    let start = match &connect {
        Some(addr) => {
            let template = bincode::serialize(&config).map_err(|e| e.to_string())?;
            let content = mc_net::ContentId {
                map_id: map.content_id(),
                blueprint_hash: blueprints.content_hash(),
            };
            let (session, prefetched, local) = app::lobby(addr, &name, content, template)?;
            game::GameStart {
                map,
                colors: setup::TEAM_COLORS,
                session: Box::new(session),
                prefetched,
                local,
                start_index: local as usize,
                scene: None,
                range: None,
            }
        }
        None if opts.scene == Scene::Range => {
            app::range_start(&map, &blueprints, &opts.subject, opts.scenario)?
        }
        None => {
            // A replay is the start message plus the command log; scripted scenes
            // inject commands outside the session, so only real matches are recorded.
            let skirmish = opts.scene == Scene::Skirmish;
            let request = ui::skirmish::MatchRequest {
                map: map.clone(),
                config,
                colors: setup::TEAM_COLORS,
            };
            let mut start = app::local_start(request, &blueprints, skirmish)?;
            // Test scenes script their armies locally; that only works on one machine.
            start.scene = (!skirmish).then(|| app::scene_scripts(&opts, &map, &blueprints));
            start
        }
    };
    app::run(app::AppArgs {
        blueprints,
        sounds,
        pool,
        force_no_vsync: !vsync,
        direct: Some(start),
        smoke: false,
    })
}
