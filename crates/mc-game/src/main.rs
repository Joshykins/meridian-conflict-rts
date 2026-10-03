//! `meridian`: the game, and the tools and test scenes that run on the same runtime.

#![expect(
    unsafe_code,
    reason = "Win32 thread priority, moving GPU handles between threads"
)]
#![expect(
    unreachable_pub,
    reason = "a binary crate exports nothing; rustc already reports unused items"
)]

mod ambience;
mod app;
mod audio;
mod chronicle;
mod cine;
mod clipboard;
mod cover_marks;
mod crash;
mod destruct_marks;
mod formation_drag;
mod game;
mod headless;
mod hud;
mod issues;
mod line_of_fire;
mod loading;
mod match_options;
mod net_bot;
mod netplay;
mod nuke_marks;
mod orders;
mod perf_out;
mod pick;
mod pointer;
mod range;
mod recorder;
mod replay;
mod rings;
mod selected;
mod settings;
mod setup;
mod shot_server;
mod sim_thread;
mod survival;
mod titan_marks;
mod ui;
mod unit_shot;
mod warp_marks;
mod window_chrome;

use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::MapFile;
use setup::{Options, Scene};
use std::sync::Arc;

/// `<version>+<commit>`: network players must match (build.rs).
pub const BUILD: &str = env!("MERIDIAN_BUILD");

/// The number of commits up to this build, empty outside a git checkout (build.rs).
const BUILD_NUMBER: &str = env!("MERIDIAN_BUILD_NUMBER");

/// "Build N" for the menu and the opening screen; "Dev build" where there is no number.
fn build_label() -> String {
    if BUILD_NUMBER.is_empty() {
        "Dev build".to_owned()
    } else {
        format!("Build {BUILD_NUMBER}")
    }
}

const USAGE: &str = "\
meridian [options]

With no match options the game opens its front end: main menu, match set-up
(skirmish and survival), multiplayer, settings. Any of --map, --scene, --players, --seed, --army, --connect or --observe goes
straight into a match instead.

  --map NAME|PATH        map to play (default: maps/dev16.mcmap, else maps/meridian_basin.mcmap)
  --scene NAME           skirmish (default) | battle | stress | showcase | range | reclaim | repair | formations | aircraft | aircraft-crash | aircraft-ditch | offshore | wreckage
  --range                the test range (same as --scene range): one unit on a pad and a
                         panel to attack it, destroy it, scrub its build state, have it
                         built, give it targets, and reset
  --unit KEY             the range's subject, a blueprint key (default aster_t1_tank)
  --hurt PERCENT         the range's subject opens with this much of its health gone
  --afloat               the range's subject stands on open water near the pad (a structure
                         that can be built at sea shows its floats and its legs to the seabed)
  --scenario NAME        open the range with a scenario staged: under-fire | close | targets
                         | build | work | salvage | upgrade | march | turn | destruct | lift | warp
                         | warp-dampened (a jump into a red warp dampener's field)
  --players N            player slots, 1-32 (default 2; slot 0 is you, the rest are AI)
  --teams N              split the players into N sides by where their zones lie (default: all alone)
  --observe              watch an all-AI match (no human slot; the camera opens on the whole map)
  --ai-difficulty NAME  easy | normal | hard (how well it spends and how many orders it gives)
  --ai-doctrine NAME    adaptive | aggressive | economic | defensive
  --ai-domains L,A,N    land, air, naval production preferences, 0-200 each
  --army N               units per player in the stress scene (default 500)
  --seed N               match seed
  --no-fog               reveal the map
  --no-vsync             present as fast as possible (for measuring frame rate)
  --at M:SS|mark:N       with --replay: play to this match time, or to mark N of this
                         match in replays/issues.log (instead of --ticks)
  --replay FILE          play a recorded match (replays/<id>.mcreplay) in a window; with
                         --screenshot, --bench or --perf, headless up to --ticks. The map is
                         found by content id unless --map is given. Marks from the profiler's
                         Mark Issue (F1) are in replays/issues.log with the command to stage them
  --connect HOST:PORT    join a network match on an mc-relay (the first to join hosts;
                         the host's --map/--players/--seed define the match)
  --name NAME            your name in a network match
  --bot idle|chaos       with --connect: play headless as a soak-test bot (chaos fuzzes every
                         kind of order); leaves after --ticks N, exits 3 on a desync
  --drop-at TICK         with --bot: hang up at this tick and rejoin with the reconnect token
  --threads N            worker threads (default: all cores)
  --bench TICKS          run the scene headless and print sim timings
  --blue KEY:N, --red KEY:N  matchup scene: two armies meet at the map centre (repeatable;
                         past 1024 an army stands in blocks, one behind another)
  --perf FILE.json       with --bench or --screenshot: write cost reports (FILE.sim.json,
                         FILE.frames.json, FILE.ui.json for the interface over a shot's
                         warm-up frames, and .txt tables; see perf_out.rs)
  --screenshot FILE.png  render one frame headless after --ticks and exit
  --ticks N              ticks to simulate before a screenshot (default 0)
  --camera X,Y,DIST[,YAW[,Z]]  screenshot camera: focus in metres, eye distance, yaw in degrees,
                         focus height in metres (default: the ground)
  --size WxH             screenshot size (default 1920x1080)
  --select KEY           match screenshot: select player 0's first unit whose blueprint key
                         contains KEY (all of them with a trailing *), not the commander
  --unit-picker          range screenshot: show the unit browser
  --range-maps           range screenshot: show the map browser
  --paused               match screenshot: show the match paused
  --net-shot STATE       match screenshot: stage a network match's moment: play | chat |
                         paused | waiting | rejoin | desync
  --report PAGE[@M:SS]   match screenshot: the battle report over the match, open on PAGE
                         (overview, economy, military, battlefield, timeline); the
                         battlefield replay stands at M:SS, or at the end. With --observe
                         and --ticks long enough the match is decided and has its verdict
  --plans                match screenshot: the commander has structures planned and a way to
                         walk, and shift is held: ghosts, order lines, the order under --cursor
  --drag X,Y             with --plans: the order under --cursor has been dragged to this pixel
  --build-grid           match screenshot: draw the build grid, as while placing a structure
  --follow N             match screenshot: play N more ticks through the renderer first,
                         so smoke, dust, track marks and shells in flight are in the picture
  --alpha A              with --follow: how far into the last tick the kept frame is (0..1)
  --unit-shot KEY        with --screenshot: that unit alone on the range, no HUD, from several
                         angles in one run, three to a row (--size is one view; default 800x600)
  --views LIST           with --unit-shot: the angles, default front34,front,left,rear34,back,top;
                         each front34 | front | left | right | rear34 | back | top | low, or
                         BEARING:ELEVATION in degrees (bearing from the nose, towards its left)
  --look X,Y,Z           with --unit-shot: aim at this point in the unit's own frame (metres:
                         x forward, y left, z up from the ground under its centre)
  --zoom F               with --unit-shot: close in F times on the body or the --look point
  --frames N             with --unit-shot: an animated PNG of N frames at 20 a second, the sim
                         playing on (two frames a tick) from the first --views angle
  --turn DEG             with --frames: the camera turns this many degrees about the unit
  --shot-server DIR      stay up with a warm renderer and answer --unit-shot requests dropped in
                         DIR as NAME.req files (one argument per line; --reload re-reads data/,
                         --shaders recompiles the WGSL, --map NAME shoots on that map, a sea for
                         ships; none, its own); writes NAME.done; scripts/shot.sh drives it
  --ui SCREEN            with --screenshot: draw a front-end screen instead of a match:
                         menu | skirmish | survival (the set-up screen in that mode) |
                         multiplayer | history | settings
  --loading SECONDS      with --screenshot: the loading screen that long after it came up;
                         FROM:TO:FPS shoots a run of numbered frames (FILE-0000.png, ...)
  --opening              with --loading: the run's opening screen instead of a map's
  --standing-down        with --loading: the screen out of a match, back to the front end
  --cursor X,Y           with --ui: where the pointer is, in pixels
  --smoke                open the front end, play a default skirmish for a few seconds, return
                         to the front end and exit: an unattended check of every stage change
  --dump-sounds DIR      write the synthesised sound set as WAV files and exit
  --dump-cursors FILE.png  write every mouse pointer, over dark, grass and bright ground, and exit

MERIDIAN_SIMPLE_SHADING=1 uses cheaper terrain/shadow shading in headless captures.
Interactive Low and Balanced presets enable it automatically; High/Ultra use full shading.

On AMD GPUs every draw is marked so a lost device's error log names the draw the GPU
stopped in (models drawn one at a time, a little slower); MERIDIAN_GPU_CRUMBS=0 turns it off.

MERIDIAN_AIM=MODE with --screenshot: an order being aimed at --cursor, with the selection:
1 a warhead launch, ground a titan's strike, reclaim the Reclaim order, warp a warp jump
(each ship's exit in formation and the energy card).

MERIDIAN_ISSUE_NOTE=TEXT with --screenshot: the F1 report card's note, as if typed.

MERIDIAN_HISTORY_REPORT=N[:PAGE] with --ui history: match N's battle report (1 = the
newest) on PAGE (overview, economy, military, battlefield, timeline), read from the replay
first when no record was kept.

MERIDIAN_GROUPS=KEY,KEY,... with --screenshot: control groups 2, 3, ... hold player 0's
units whose blueprint key contains each KEY (group 1 is the selection).

MERIDIAN_BUILD=NAME at compile time names the build in the replays it records
(default: the package version with -dev).
";

fn main() {
    crash::install();
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info,symphonia=warn"),
    )
    .format_timestamp_millis()
    .target(env_logger::Target::Pipe(Box::new(crash::LogTee)))
    .init();
    if let Err(e) = run() {
        eprintln!("error: {e}");
        crash::report_error(&e);
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
    let mut at: Option<String> = None;
    let mut shot: Option<headless::Shot> = None;
    let mut camera = None;
    let mut focus_z = None;
    let mut size = (1920u32, 1080u32);
    let mut vsync = true;
    let mut connect: Option<String> = None;
    let mut name = String::from("Commander");
    // Set by any option that describes a match: skip the front end.
    let mut direct = false;
    let mut ui_screen: Option<ui::front::Screen> = None;
    let mut loading_at: Option<Vec<f32>> = None;
    let mut loading_screen = loading::Screen::Briefing;
    let mut cursor: Option<[f32; 2]> = None;
    let mut smoke = false;
    let mut dump_sounds: Option<String> = None;
    let mut select: Option<String> = None;
    let mut paused = false;
    let mut net_shot: Option<String> = None;
    let mut unit_picker = false;
    let mut range_maps = false;
    let mut refit_tab = false;
    let mut details = false;
    let mut report = None;
    let mut range_tab: Option<String> = None;
    let mut place: Option<String> = None;
    let (mut plans, mut drag): (bool, Option<[f32; 2]>) = (false, None);
    let mut build_grid = false;
    let (mut follow, mut alpha) = (0u32, 1.0f32);
    let mut bot: Option<net_bot::Bot> = None;
    let mut drop_at: Option<u32> = None;
    let mut sized = false;
    let mut unit_shot = unit_shot::Spec::default();
    let mut shot_server: Option<std::path::PathBuf> = None;
    let mut unit_shot_key: Option<String> = None;

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
                | "--teams"
                | "--army"
                | "--blue"
                | "--red"
                | "--seed"
                | "--connect"
                | "--replay"
                | "--no-fog"
                | "--range"
                | "--unit-shot"
                | "--shot-server"
                | "--observe"
                | "--ai-difficulty"
                | "--ai-doctrine"
                | "--ai-domains"
        );
        match arg.as_str() {
            "--map" => map_name = Some(value("--map")?),
            "--scene" => opts.scene = Scene::parse(&value("--scene")?).ok_or("unknown scene")?,
            "--range" => opts.scene = Scene::Range,
            "--observe" => opts.observe = true,
            "--replay" => opts.replay = Some(std::path::PathBuf::from(value("--replay")?)),
            "--ai-difficulty" => opts.ai.difficulty = match value("--ai-difficulty")?.as_str() {
                "easy" => mc_sim::Difficulty::Easy, "normal" => mc_sim::Difficulty::Normal, "hard" => mc_sim::Difficulty::Hard,
                _ => return Err("--ai-difficulty takes easy, normal or hard".into()),
            },
            "--ai-doctrine" => opts.ai.doctrine = match value("--ai-doctrine")?.as_str() {
                "adaptive" => mc_sim::Doctrine::Adaptive, "aggressive" => mc_sim::Doctrine::Aggressive,
                "economic" => mc_sim::Doctrine::Economic, "defensive" => mc_sim::Doctrine::Defensive,
                _ => return Err("--ai-doctrine takes adaptive, aggressive, economic or defensive".into()),
            },
            "--ai-domains" => {
                let v = value("--ai-domains")?;
                let weights: Vec<u8> = v.split(',').map(str::parse).collect::<Result<_,_>>()
                    .map_err(|_| "--ai-domains takes three numbers from 0-200")?;
                if weights.len() != 3 || weights.iter().any(|w| *w > 200) {
                    return Err("--ai-domains takes three numbers from 0-200".into());
                }
                opts.ai.domain_weights.copy_from_slice(&weights);
            },
            "--unit" => opts.subject = value("--unit")?,
            "--unit-shot" => unit_shot_key = Some(value("--unit-shot")?),
            "--shot-server" => shot_server = Some(value("--shot-server")?.into()),
            a if unit_shot::flag(&mut unit_shot, a, &mut value)? => {}
            "--afloat" => opts.afloat = true,
            "--hurt" => opts.hurt = value("--hurt")?.parse::<i16>().ok().filter(|p| (0..100).contains(p)).ok_or("--hurt takes a percentage under 100")? * 10,
            "--scenario" => opts.scenario = Some(range::Scenario::parse(&value("--scenario")?).ok_or("--scenario takes under-fire, close, targets, build, work, salvage, upgrade, march, turn, destruct, lift, warp or warp-dampened")?),
            "--players" => opts.players = value("--players")?.parse().map_err(|_| "--players takes a number")?,
            "--teams" => opts.teams = value("--teams")?.parse().map_err(|_| "--teams takes a number")?,
            "--army" => opts.army = value("--army")?.parse().map_err(|_| "--army takes a number")?,
            "--seed" => opts.seed = value("--seed")?.parse().map_err(|_| "--seed takes a number")?,
            "--no-fog" => opts.fog = false,
            "--no-vsync" => vsync = false,
            "--connect" => connect = Some(value("--connect")?),
            "--name" => name = value("--name")?,
            "--bot" => bot = Some(net_bot::Bot::parse(&value("--bot")?).ok_or("--bot takes idle or chaos")?),
            "--drop-at" => drop_at = Some(value("--drop-at")?.parse().map_err(|_| "--drop-at takes a tick")?),
            "--threads" => threads = Some(value("--threads")?.parse().map_err(|_| "--threads takes a number")?),
            "--blue" | "--red" => {
                let v = value(&arg)?;
                let (key, n) = v.split_once(':').unwrap_or((v.as_str(), "1"));
                let n: u16 = n.parse().map_err(|_| format!("{arg} takes KEY:COUNT"))?;
                opts.matchup.push(((arg == "--red") as u8, key.to_owned(), n));
                opts.scene = Scene::Matchup;
            }
            "--perf" => perf_out::set(std::path::PathBuf::from(value("--perf")?)),
            "--bench" => bench = Some(value("--bench")?.parse().map_err(|_| "--bench takes a tick count")?),
            "--ticks" => ticks = value("--ticks")?.parse().map_err(|_| "--ticks takes a number")?,
            "--at" => at = Some(value("--at")?),
            "--screenshot" => shot = Some(headless::Shot { path: value("--screenshot")?, camera: None, focus_z: None, width: 0, height: 0, select: None, cursor: None, paused: false, net: None, unit_picker: false, range_maps: false, refit_tab: false, details: false, report: None, range_tab: None, place: None, plans: false, drag: None, follow: 0, alpha: 1.0, build_grid: false }),
            "--camera" => {
                let v: Vec<f32> = value("--camera")?.split(',').filter_map(|p| p.trim().parse().ok()).collect();
                if v.len() < 3 {
                    return Err("--camera takes X,Y,DIST[,YAW[,Z]]".into());
                }
                camera = Some([v[0], v[1], v[2], v.get(3).copied().unwrap_or(0.0)]);
                focus_z = v.get(4).copied();
            }
            "--size" => {
                let v = value("--size")?;
                let (w, h) = v.split_once('x').ok_or("--size takes WxH")?;
                size = (w.parse().map_err(|_| "--size takes WxH")?, h.parse().map_err(|_| "--size takes WxH")?);
                sized = true;
            }
            "--loading" => loading_at = Some(loading_times(&value("--loading")?).ok_or("--loading takes SECONDS or FROM:TO:FPS")?),
            "--opening" => loading_screen = loading::Screen::Opening,
            "--standing-down" => loading_screen = loading::Screen::StandingDown,
            "--ui" => ui_screen = Some(ui::front::Screen::parse(&value("--ui")?).ok_or("--ui takes menu, skirmish, survival, multiplayer, history or settings")?),
            "--cursor" => {
                let v: Vec<f32> = value("--cursor")?.split(',').filter_map(|p| p.trim().parse().ok()).collect();
                cursor = Some([*v.first().ok_or("--cursor takes X,Y")?, *v.get(1).ok_or("--cursor takes X,Y")?]);
            }
            "--select" => select = Some(value("--select")?),
            "--paused" => paused = true,
            "--net-shot" => net_shot = Some(value("--net-shot")?),
            "--unit-picker" => unit_picker = true,
            "--range-maps" => range_maps = true,
            "--refit-tab" => refit_tab = true,
            "--details" => details = true,
            "--report" => report = Some(value("--report")?),
            "--range-tab" => range_tab = Some(value("--range-tab")?),
            "--place" => place = Some(value("--place")?),
            "--plans" => plans = true,
            "--build-grid" => build_grid = true,
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

    if let Some(key) = &unit_shot_key {
        opts.scene = Scene::Range;
        opts.subject = key.clone();
    }

    let reading = std::time::Instant::now();
    let data_dir = Blueprints::locate_data_dir().ok_or(
        "could not find the data/ directory next to the executable or above the working directory",
    )?;
    let blueprints = Arc::new(Blueprints::load(&data_dir).map_err(|e| e.to_string())?);
    // The sound library is data too, and a unit file naming a sound that is not in it is an error here, not silence later.
    let sounds = mc_data::SoundLibrary::load(&data_dir).map_err(|e| e.to_string())?;
    sounds.check(&blueprints).map_err(|e| e.to_string())?;
    log::debug!(
        "data/ read in {:.0} ms",
        reading.elapsed().as_secs_f32() * 1000.0
    );
    if let Some(dir) = dump_sounds {
        return audio::dump(std::path::Path::new(&dir), &sounds);
    }
    let pool = Arc::new(match threads {
        Some(n) => Pool::new(n),
        None => Pool::with_default_threads(),
    });

    if opts.scene == Scene::Range && blueprints.id_of(&opts.subject).is_none() {
        let keys: Vec<&str> = blueprints
            .units
            .iter()
            .filter(|u| blueprints.is_listed(u.id))
            .map(|u| u.key.as_str())
            .collect();
        return Err(format!(
            "--unit {:?} is not a unit. The units are: {}",
            opts.subject,
            keys.join(", ")
        ));
    }

    if let Some(times) = loading_at {
        let mut shot = shot
            .take()
            .ok_or("--loading draws a screenshot: give it --screenshot FILE.png")?;
        (shot.width, shot.height) = size;
        return loading::screenshot(blueprints, pool, &shot, &times, loading_screen);
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

    let playback = opts
        .replay
        .as_deref()
        .map(replay::Playback::open)
        .transpose()?;
    if let Some(p) = &playback {
        if map_name.is_none() {
            map_name = Some(replay::find_map(p.start())?.display().to_string());
        }
        // Drawn as it was played: with the match's fog, through slot 0's eyes.
        opts.fog = p.config.fog;
        opts.observe = p.start().players.is_empty();
    }
    if let Some(at) = &at {
        let path = opts.replay.as_deref().ok_or("--at goes with --replay")?;
        ticks = replay::tick_at(path, at)?;
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
        headless::run_sim(&opts, &map, &blueprints, &pool, ticks, true, None)?;
        return Ok(());
    }
    if let Some(dir) = shot_server {
        return shot_server::serve(&dir, &opts, map, &data_dir, pool);
    }
    if unit_shot_key.is_some() {
        let shot = shot
            .take()
            .ok_or("--unit-shot draws a screenshot: give it --screenshot FILE.png")?;
        unit_shot.path = shot.path;
        if sized {
            (unit_shot.width, unit_shot.height) = size;
        }
        if unit_shot.views.is_empty() {
            unit_shot.views = unit_shot::parse_views(unit_shot::SHEET)?;
        }
        return unit_shot::run(&opts, map, blueprints, pool, ticks, &unit_shot);
    }
    if let Some(mut shot) = shot {
        shot.camera = camera;
        shot.focus_z = focus_z;
        shot.unit_picker = unit_picker;
        shot.range_maps = range_maps;
        shot.refit_tab = refit_tab;
        shot.details = details;
        shot.report = report;
        shot.range_tab = range_tab;
        shot.place = place;
        (shot.width, shot.height) = size;
        (shot.select, shot.cursor, shot.paused) = (select, cursor, paused);
        shot.net = net_shot;
        (shot.follow, shot.alpha) = (follow, alpha);
        (shot.plans, shot.drag) = (plans, drag);
        shot.build_grid = build_grid;
        return headless::screenshot(&opts, map, blueprints, pool, ticks, &shot);
    }

    let config = setup::match_config(&opts, &map);
    // What a host sends its relay: this machine's command-line match as the template.
    let template = || {
        match_options::MatchOptions {
            config: config.clone(),
            survival: None,
            colors: setup::TEAM_COLORS,
            map: map.name().to_owned(),
            map_id: map.content_id(),
            sky: Default::default(),
        }
        .encode()
    };
    if let Some(bot) = bot {
        let addr = connect.ok_or("--bot plays a network match: give it --connect HOST:PORT")?;
        let template = template()?;
        let run = net_bot::BotRun {
            addr,
            name,
            bot,
            ticks,
            drop_at,
        };
        return net_bot::run(run, map, blueprints, pool, template);
    }
    let start = match &connect {
        Some(addr) => {
            let template = template()?;
            let content = mc_net::ContentId {
                map_id: map.content_id(),
                blueprint_hash: blueprints.content_hash(),
            };
            let (session, prefetched, local) = app::lobby(addr, &name, content, template)?;
            // Coming back after a dropped connection takes the seat's token.
            let mut again = app::net_config(&name, mc_net::Role::Player, content);
            again.token = session.token();
            let rejoin = netplay::Rejoin {
                addr: addr.clone(),
                config: again,
            };
            // The host's settings, not our template: they say who starts where.
            let options = prefetched.iter().find_map(|e| match e {
                mc_net::SessionEvent::Started(s) => match_options::MatchOptions::from_start(s).ok(),
                _ => None,
            });
            let sky = options.as_ref().map(|o| o.sky).unwrap_or_default();
            let roster = options.map_or(Vec::new(), |o| o.config.players);
            let start_index = roster
                .get(local as usize)
                .map_or(local as usize, |p| p.start as usize);
            let mut start = game::GameStart {
                map,
                colors: setup::TEAM_COLORS,
                sky,
                session: Box::new(session),
                prefetched,
                local,
                start_index,
                roster,
                observing: false,
                scene: None,
                range: None,
                record: None,
                recorder: None,
                seek: None,
                net: Some(rejoin),
                keep: Vec::new(),
            };
            app::record_in_sim(&mut start);
            start
        }
        None if playback.is_some() => {
            let at = (ticks > 0).then_some(ticks);
            replay::game_start(playback.expect("checked by the guard"), map.clone(), at)
        }
        None if opts.scene == Scene::Range => {
            app::range_start(&map, &blueprints, &opts.subject, opts.scenario)?
        }
        None => {
            // A replay is the start message plus the command log; scripted scenes
            // inject commands outside the session, so only real matches are recorded.
            let skirmish = matches!(opts.scene, Scene::Skirmish | Scene::Survival);
            let mut colors = setup::TEAM_COLORS;
            let survival = if opts.scene == Scene::Survival {
                colors[1] = survival::ENGINE_COLOR;
                Some(survival::scene_match(&opts, &map)?.1)
            } else {
                None
            };
            let request = ui::setup::MatchRequest {
                map: map.clone(),
                config,
                colors,
                survival,
                sky: Default::default(),
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

/// `--loading`'s times: one, or `FROM:TO:FPS` for a run of frames.
fn loading_times(v: &str) -> Option<Vec<f32>> {
    let parts: Vec<f32> = v
        .split(':')
        .map(|p| p.trim().parse().ok())
        .collect::<Option<_>>()?;
    match parts[..] {
        [at] => Some(vec![at]),
        [from, to, fps] if fps > 0.0 && to >= from => {
            let n = ((to - from) * fps).floor() as usize;
            Some((0..=n).map(|i| from + i as f32 / fps).collect())
        }
        _ => None,
    }
}
