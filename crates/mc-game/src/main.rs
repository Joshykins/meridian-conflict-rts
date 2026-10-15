//! `meridian`: the game, and the tools and test scenes that run on the same runtime.

#![expect(
    unsafe_code,
    reason = "Win32 thread priority, moving GPU handles between threads"
)]
#![expect(
    unreachable_pub,
    reason = "a binary crate exports nothing; rustc already reports unused items"
)]

mod adjacency_marks;
mod ambience;
mod app;
mod audio;
mod build_info;
mod builds;
mod campaign;
mod campaign_checkpoints;
mod chronicle;
mod cine;
mod clipboard;
mod cover_marks;
mod crash;
mod dash_marks;
mod destruct_marks;
mod edict_marks;
mod editor;
mod fast_hash;
mod film;
mod formation_drag;
mod game;
mod gravitic_marks;
mod headless;
mod hud;
mod issues;
mod line_of_fire;
mod live;
mod loading;
mod map_players;
mod match_options;
mod mind_map;
mod net_bot;
mod netplay;
mod nuke_marks;
mod orders;
mod perf_out;
mod pick;
mod pointer;
mod prefabs;
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
mod unit_rows;
mod unit_shot;
mod warp_in_marks;
mod warp_marks;
mod window_chrome;

use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::MapFile;
use setup::{Options, Scene};
use std::sync::Arc;

pub use build_info::BUILD;

/// "Build N" for the menu and the opening screen; "Dev build" where there is no number.
fn build_label() -> String {
    match build_info::number() {
        Some(n) => format!("Build {n}"),
        None => "Dev build".to_owned(),
    }
}

const USAGE: &str = "\
meridian [options]

With no match options the game opens its front end: main menu, match set-up
(skirmish and survival), multiplayer, settings. Any of --map, --scene, --players, --seed, --army, --connect or --observe goes
straight into a match instead.

  --map NAME|PATH        map to play (default: maps/crosswater.mcmap, else maps/meridian_basin.mcmap)
  --scene NAME           skirmish (default) | battle | stress | showcase | adjacency | range | reclaim | repair | formations | aircraft | aircraft-crash | aircraft-ditch | offshore | wreckage
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
                         | pause (its work paused) | short (its side has half the energy
                         its upkeep asks for) | elevator (a space elevator with tanks,
                         gunships and tech 4s queued for its first platform)
                         | dash (a pack of red tanks for a unit that dashes, the Seraph)
                         | pack (a pack of red tanks for a gravitic lens to hold and crush)
                         | fracture (a gravitic engine turned at once on a red camp 2 km
                         down the range: tanks, power plants and gunships)
                         | fracture-release (the same, then the engine falters for 3 s
                         and is stopped, letting its well go)
                         | edict (a ship with an Edict, the Regent, fired at once on a red
                         camp 5 km down the range: rings of tanks, a dome, power plants)
  --players N            player slots, 1-32 (default 2; slot 0 is you, the rest are AI)
  --teams N              split the players into N sides by where their zones lie (default: all alone)
  --observe              watch an all-AI match (no human slot; the camera opens on the whole map)
  --ai-difficulty NAME  easy | normal | hard (how well it spends and how many orders it gives)
  --ai-doctrine NAME    adaptive | aggressive | economic | defensive
  --ai-domains L,A,N    land, air, naval production preferences, 0-200 each
  --ai-income X         times a fair income the AI gets, 0.25-10 (e.g. 1.5, 2, 3, 5)
  --income X             times a fair income your own seat gets, 0.25-10 (default 1)
  --ai-build-rate X     times the normal build rate the AI's builders work at, 0.25-10
  --build-rate X         times the normal build rate your own builders work at, 0.25-10 (default 1)
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
  --quality PRESET       screenshot: draw at that graphics preset, low | medium | high |
                         ultra (default: high, whatever the card)
  --select KEY           match screenshot: select player 0's first unit whose blueprint key
                         contains KEY (all of them with a trailing *), not the commander;
                         any side's when player 0 has none (a survival Shaper; the
                         camera looks at it unless --camera is given)
  --unit-picker          range screenshot: show the unit browser
  --range-maps           range screenshot: show the map browser
  --paused               match screenshot: show the match paused
  --net-shot STATE       match screenshot: stage a network match's moment: play | chat |
                         watching | paused | waiting | rejoin | desync (watching: this side is
                         out and watches, its chat line open among the observers)
  --report PAGE[@M:SS]   match screenshot: the battle report over the match, open on PAGE
                         (overview, economy, military, battlefield, timeline); the
                         battlefield replay stands at M:SS, or at the end. With --observe
                         and --ticks long enough the match is decided and has its verdict
  --plans                match screenshot: the commander has structures planned and a way to
                         walk, and shift is held: ghosts, order lines, the order under --cursor
  --drag X,Y             with --plans: the order under --cursor has been dragged to this pixel
  --build-grid           match screenshot: draw the build grid, as while placing a structure
  --prefabs FILE         match screenshot: the player's prefabs read from FILE (a prefabs.ron),
                         the selected builder's construction panel turned to prefabs
  --place @NAME          with --prefabs: that prefab in hand at --cursor (KEY, not @NAME:
                         that structure being placed)
  --place-turn Q[m]      with --place: what is in hand turned Q quarter turns anticlockwise
                         (as R turns it clockwise), and with m (a prefab) mirrored first
  --save-prefab          match screenshot: the Save Prefab box open on the selection (its
                         structures, and with --plans the commander's planned ones)
  --follow N             match screenshot: play N more ticks through the renderer first,
                         so smoke, dust, track marks and shells in flight are in the picture
  --alpha A              with --follow: how far into the last tick the kept frame is (0..1)
  --film DIR             match screenshot: film a camera move after --ticks into DIR as
                         numbered PNGs (00000.png, ...), no interface; --screenshot gets the
                         last frame. The camera aims at the middle of the mobile units,
                         followed smoothly, or at --camera X,Y. scripts/film.sh cuts the clips
  --film-secs S          with --film: the clip's length (default 8)
  --film-fps N           with --film: frames a second (default 30)
  --film-speed X         with --film: match seconds a second of film, 0.5 slow motion (default 1)
  --film-from RIG        with --film: where the camera starts, YAW,PITCH,DIST[,FOV[,LIFT[,SIDE]]]:
                         the bearing it looks along and degrees below the horizon, metres
                         from the aim point, lens in degrees (default 34), the aim point's
                         metres above the ground (default 4) and to the camera's right (0)
  --film-to RIG          with --film: where it ends, eased there from --film-from (an orbit
                         turns YAW, a crane PITCH, a push in DIST, a truck past the fight SIDE)
  --film-on KEY          with --film: aim at the units whose blueprint key contains KEY
  --unit-shot KEY        with --screenshot: that unit alone on the range, no HUD, from several
                         angles in one run, three to a row (--size is one view; default 800x600);
                         or a map prop by its model key (city_office), its front its street
                         side, --hurt taking that much off its health (city windows break)
  --views LIST           with --unit-shot: the angles, default front34,front,left,rear34,back,top;
                         each front34 | front | left | right | rear34 | back | top | low, or
                         BEARING:ELEVATION in degrees (bearing from the nose, towards its left)
  --look X,Y,Z           with --unit-shot: aim at this point in the unit's own frame (metres:
                         x forward, y left, z up from the ground under its centre)
  --zoom F               with --unit-shot: close in F times on the body or the --look point
  --frames N             with --unit-shot: an animated PNG of N frames at 20 a second, the sim
                         playing on (two frames a tick) from the first --views angle
  --turn DEG             with --frames: the camera turns this many degrees about the unit
  --fire SECONDS         with --unit-shot of a city structure: it has been burning this long
  --gutted               with --unit-shot of a city structure: burnt out
  --skin KEY             with --unit-shot: every model drawn in skin KEY (data/skins/KEY.ron),
                         not its faction's own
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
  --mission-pilot       with --bench/--screenshot on Vela: play the authoring smoke route
  --editor MAP           open a map name or .mcmap path in the map editor
  --editor-headless      with --editor MAP: native offscreen editor API; close with op=close
  --editor-apply FILE    apply a JSON command array to the map folder and save; optional screenshot
  --smoke                open the front end, play a default skirmish for a few seconds, return
                         to the front end and exit: an unattended check of every stage change
  --crash-screen FILE    the crash screen for a saved report, as a crashed run starts it (with
                         --crash-title T, --crash-message M, --crash-hint H); with
                         --screenshot OUT.png: drawn headless, and OUT-copied.png after Copy details
  --crash-test KIND      fail on purpose 8 s in, to check the crash window: panic (main
                         thread) | sim (a thread the game needs) | native (an access
                         violation, Windows) | error (an error that ends the game, raised
                         as a failed Vulkan call is)
  --dump-sounds DIR      write the synthesised sound set as WAV files and exit
  --dump-cursors FILE.png  write every mouse pointer, over dark, grass and bright ground, and exit
  --replay-only          with --replay: an old build playing an old replay for a newer one
                         (MeridianConflict --replay starts it so): leaving the replay quits,
                         and the settings file is left as it was
  --open SCREEN          open the front end on SCREEN (menu | skirmish | survival |
                         multiplayer | history | settings) instead of the menu
  --version              print the build's name, number, channel, commit and simulation
                         fingerprint, and exit

Graphics presets (Auto, Low, Medium, High, Ultra) are in Settings > Display. These override
a preset's scenery for A/B measurements (docs/SWITCHES.md): MERIDIAN_SIMPLE_SHADING=0|1,
MERIDIAN_PROP_DETAIL=a,b,c, MERIDIAN_CLOUD_RES=1-4, MERIDIAN_SHADOW_SIZE=512-4096,
MERIDIAN_SHADOW_DISTANCE=metres, MERIDIAN_GTAO=0|1, MERIDIAN_GRASS=0,
MERIDIAN_GRASS_DENSITY=0-1, MERIDIAN_WATER_REFLECTIONS=0|1.
MERIDIAN_FOLLOW_FRAMES=2-16: headless frames a followed tick is drawn in (default 2).
MERIDIAN_DEV=1 (play.sh sets it): the main menu also offers the campaign and the map editor.

On AMD GPUs every draw is marked so a lost device's error log names the draw the GPU
stopped in (models drawn one at a time, a little slower); MERIDIAN_GPU_CRUMBS=0 turns it off,
MERIDIAN_GPU_CRUMBS=1 turns it on for other GPUs whose driver can mark (NVIDIA).

MERIDIAN_AIM=MODE with --screenshot: an order being aimed at --cursor, with the selection:
1 a warhead launch, ground a titan's strike, reclaim the Reclaim order, warp a warp jump
(each ship's exit in formation and the energy card), dash a dash by hand (the wave where
it would come down), fracture a gravitic engine's rift (its path and the well at the point),
edict a ship's Edict (its 6 km reach, the firing point on the hull and the blast and wave rings).

MERIDIAN_SURVIVAL=rounds:grace:interval:intensity:fronts:tier:nodes with --scene survival:
the rules it plays (docs/SWITCHES.md, with MERIDIAN_SURVIVAL_SPAWN and MERIDIAN_SURVIVAL_WHY);
MERIDIAN_SURVIVAL_PLAY_ON=1: the engine plays on after the defenders fall, its rounds
coming (printing nothing more) and its Shapers growing, to shoot grown Shapers from a
headless --observe run.

MERIDIAN_ISSUE_NOTE=TEXT with --screenshot: the F1 report card's note, as if typed.

MERIDIAN_HISTORY_REPORT=N[:PAGE] with --ui history: match N's battle report (1 = the
newest) on PAGE (overview, economy, military, battlefield, timeline), read from the replay
first when no record was kept.

MERIDIAN_GROUPS=KEY,KEY,... with --screenshot: control groups 2, 3, ... hold player 0's
units whose blueprint key contains each KEY (group 1 is the selection).

MERIDIAN_CHANNEL=dev|playtest|release at compile time: who the build is for (default
dev). It is part of the build's name, which replays record and network players must
share (docs/RELEASES.md). A release build has no unit or map marked `playtest: true`.

MERIDIAN_STORE=URL at compile time: the build store a published build's install has.
MERIDIAN_LAUNCHER (set by the launcher, MeridianConflict.exe, for the game it starts)
is the launcher that keeps it up to date and runs old replays in their own builds.

MERIDIAN_SERVER_PLAYTEST=HOST[:PORT], MERIDIAN_SERVER_RELEASE=HOST[:PORT] at compile time:
the server a playtest or a release build plays on; players cannot change it (none:
the multiplayer screen opens on the local network, as a dev build's always does).

MERIDIAN_MP_SERVER=HOST[:PORT]: the game plays on that server instead of its build's
(also for --ui shots of the multiplayer screen).
";

fn main() {
    crash::install();
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info,symphonia=warn"),
    )
    .format_timestamp_millis()
    .target(env_logger::Target::Pipe(Box::new(crash::LogTee)))
    .init();
    let code = crash::run_guarded(run);
    if code != 0 {
        std::process::exit(code);
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
    let mut editor_path = None;
    let mut editor_headless = false;
    let mut editor_actions = None;
    let mut replay_only = false;
    let mut open: Option<ui::front::Screen> = None;
    let mut crash_test = None;
    let mut crash_screen: Option<crash::Summary> = None;
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
    let mut turn = prefabs::Turn::default();
    let mut prefabs: Option<String> = None;
    let mut save_prefab = false;
    let (mut plans, mut drag): (bool, Option<[f32; 2]>) = (false, None);
    let mut build_grid = false;
    let mut quality = None;
    let (mut follow, mut alpha) = (0u32, 1.0f32);
    let (mut film, mut film_dir, mut film_opts) = (
        film::Film::new(Default::default()),
        None::<std::path::PathBuf>,
        false,
    );
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
                | "--ai-income"
                | "--income"
                | "--ai-build-rate"
                | "--build-rate"
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
            "--ai-income" => opts.ai.income = income_flag("--ai-income", &value("--ai-income")?, mc_sim::INCOME_RANGE)?,
            "--income" => opts.income = income_flag("--income", &value("--income")?, mc_sim::INCOME_RANGE)?,
            "--ai-build-rate" => opts.ai.build = income_flag("--ai-build-rate", &value("--ai-build-rate")?, mc_sim::BUILD_RANGE)?,
            "--build-rate" => opts.build = income_flag("--build-rate", &value("--build-rate")?, mc_sim::BUILD_RANGE)?,
            "--unit" => opts.subject = value("--unit")?,
            "--unit-shot" => unit_shot_key = Some(value("--unit-shot")?),
            "--shot-server" => shot_server = Some(value("--shot-server")?.into()),
            a if unit_shot::flag(&mut unit_shot, a, &mut value)? => {}
            "--afloat" => opts.afloat = true,
            "--hurt" => opts.hurt = value("--hurt")?.parse::<i16>().ok().filter(|p| (0..100).contains(p)).ok_or("--hurt takes a percentage under 100")? * 10,
            "--scenario" => opts.scenario = Some(range::Scenario::parse(&value("--scenario")?).ok_or("--scenario takes under-fire, close, targets, build, work, salvage, upgrade, march, turn, destruct, lift, warp, warp-dampened, pause, short, elevator, dash, pack, fracture, fracture-release or edict")?),
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
            "--screenshot" => shot = Some(headless::Shot { path: value("--screenshot")?, camera: None, focus_z: None, width: 0, height: 0, select: None, cursor: None, paused: false, net: None, unit_picker: false, range_maps: false, refit_tab: false, details: false, report: None, range_tab: None, place: None, turn: Default::default(), prefabs: None, save_prefab: false, plans: false, drag: None, follow: 0, alpha: 1.0, build_grid: false, quality: None, film: None }),
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
            "--place-turn" => turn = prefabs::Turn::parse(&value("--place-turn")?).ok_or("--place-turn takes quarter turns 0..3, then m to mirror (a prefab): 1, 2m")?,
            "--prefabs" => prefabs = Some(value("--prefabs")?),
            "--save-prefab" => save_prefab = true,
            "--plans" => plans = true,
            "--build-grid" => build_grid = true,
            "--quality" => quality = Some(settings::Quality::parse(&value("--quality")?).ok_or("--quality takes low, medium, high or ultra")?),
            "--drag" => {
                let v: Vec<f32> = value("--drag")?.split(',').filter_map(|p| p.trim().parse().ok()).collect();
                drag = Some([*v.first().ok_or("--drag takes X,Y")?, *v.get(1).ok_or("--drag takes X,Y")?]);
            }
            "--follow" => follow = value("--follow")?.parse().map_err(|_| "--follow takes a number of ticks")?,
            "--alpha" => alpha = value("--alpha")?.parse::<f32>().map_err(|_| "--alpha takes a number from 0 to 1")?.clamp(0.0, 1.0),
            "--film" => film_dir = Some(value("--film")?.into()),
            "--film-secs" | "--film-fps" | "--film-speed" | "--film-from" | "--film-to" | "--film-on" => {
                let text = value(&arg)?;
                let f = &mut film;
                film_opts = true;
                let number = |what: &str| text.parse::<f32>().ok().filter(|v| *v > 0.0).ok_or(format!("{arg} takes {what}"));
                match arg.as_str() {
                    "--film-secs" => f.secs = number("seconds")?,
                    "--film-fps" => f.fps = number("frames a second")?.round() as u32,
                    "--film-speed" => f.speed = number("a speed, 1 the match's own")?,
                    "--film-on" => f.on = Some(text),
                    "--film-from" => f.from = film::Rig::parse(&text).ok_or("--film-from takes YAW,PITCH,DIST[,FOV[,LIFT[,SIDE]]]")?,
                    _ => f.to = film::Rig::parse(&text).ok_or("--film-to takes YAW,PITCH,DIST[,FOV[,LIFT[,SIDE]]]")?,
                }
            }
            "--smoke" => smoke = true,
            "--mission-pilot" => opts.mission_pilot = true,
            "--editor-headless" => editor_headless=true,
            "--editor-apply" => editor_actions = Some(value("--editor-apply")?),
            "--editor" => editor_path = Some(args.next().ok_or("--editor needs a map name or path")?),
            "--replay-only" => {
                replay_only = true;
                settings::keep_unsaved();
            }
            "--open" => open = Some(ui::front::Screen::parse(&value("--open")?).ok_or("--open takes menu, skirmish, survival, multiplayer, history or settings")?),
            "--dump-sounds" => dump_sounds = Some(value("--dump-sounds")?),
            "--crash-screen" => crash_screen = Some(crash::Summary { path: value("--crash-screen")?.into(), title: "The game crashed".into(), message: String::new(), hint: None }),
            "--crash-title" | "--crash-message" | "--crash-hint" => {
                let text = value(&arg)?;
                let s = crash_screen.as_mut().ok_or("--crash-title, --crash-message and --crash-hint go after --crash-screen")?;
                match arg.as_str() {
                    "--crash-title" => s.title = text,
                    "--crash-message" => s.message = text,
                    _ => s.hint = Some(text),
                }
            }
            "--crash-test" => crash_test = Some(crash::Drill::parse(&value("--crash-test")?).ok_or("--crash-test takes panic, sim, native or error")?),
            "--dump-cursors" => {
                let (rgba, width, height) = pointer::sheet(2.0);
                return headless::write_png(std::path::Path::new(&value("--dump-cursors")?), width, height, &rgba);
            }
            "-h" | "--help" => {
                print!("{USAGE}");
                return Ok(());
            }
            "--version" => {
                print!("{}", build_info::version_text());
                return Ok(());
            }
            other => return Err(format!("unknown option {other}\n\n{USAGE}")),
        }
    }

    // The crash screen needs no data: it comes up when the data is what broke.
    if let Some(summary) = crash_screen.take_if(|_| shot.is_none()) {
        return crash::run_screen(summary);
    }

    if let Some(key) = &unit_shot_key {
        opts.scene = Scene::Range;
        opts.subject = key.clone();
    }

    let reading = std::time::Instant::now();
    let data_dir = Blueprints::locate_data_dir().ok_or(
        "could not find the data/ directory next to the executable or above the working directory",
    )?;
    let blueprints = Arc::new(
        Blueprints::load_for(&data_dir, build_info::channel()).map_err(|e| e.to_string())?,
    );
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

    if let Some(actions) = editor_actions {
        let name = editor_path
            .as_deref()
            .ok_or("--editor-apply requires --editor MAP")?;
        let path = setup::find_map(Some(name))?;
        editor::apply_file(&path, std::path::Path::new(&actions), &blueprints)?;
        if shot.is_none() {
            return Ok(());
        }
    }
    if editor_headless {
        let name = editor_path
            .as_deref()
            .ok_or("--editor-headless requires --editor MAP")?;
        let path = if std::path::Path::new(name).is_file() {
            std::path::PathBuf::from(name)
        } else {
            setup::find_map(Some(name))?
        };
        return editor::serve(&path, blueprints, pool, size);
    }
    if let (Some(name), Some(shot)) = (&editor_path, &mut shot) {
        let path = if std::path::Path::new(name).is_file() {
            std::path::PathBuf::from(name)
        } else {
            setup::find_map(Some(name))?
        };
        (shot.width, shot.height) = size;
        shot.camera = camera;
        return editor::screenshot(&path, blueprints, pool, shot);
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
    if let Some(summary) = crash_screen {
        let mut shot = shot
            .take()
            .ok_or("--crash-screen draws in a window, or with --screenshot FILE.png")?;
        (shot.width, shot.height) = size;
        return crash::screen_shot(summary, blueprints, pool, &shot, cursor);
    }
    if let Some(drill) = crash_test {
        crash::arm_drill(drill);
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
            editor: editor_path,
            smoke,
            open,
            replay_only,
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
    if film_opts && film_dir.is_none() {
        return Err("--film-secs, --film-fps, --film-speed, --film-from, --film-to and --film-on go with --film DIR".into());
    }
    if film_dir.is_some() && shot.is_none() {
        return Err("--film draws headless: give it --screenshot FILE.png (its last frame)".into());
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
        (shot.place, shot.turn) = (place, turn);
        (shot.prefabs, shot.save_prefab) = (prefabs, save_prefab);
        (shot.width, shot.height) = size;
        (shot.select, shot.cursor, shot.paused) = (select, cursor, paused);
        shot.net = net_shot;
        (shot.follow, shot.alpha) = (follow, alpha);
        (shot.plans, shot.drag) = (plans, drag);
        shot.build_grid = build_grid;
        shot.quality = quality;
        shot.film = film_dir.map(|dir| film::Film { dir, ..film });
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
            mission: None,
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
        editor: None,
        smoke: false,
        open,
        replay_only,
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

/// `--income`'s, `--ai-income`'s, `--build-rate`'s and `--ai-build-rate`'s multiple,
/// as thousandths of normal, within `range`.
fn income_flag(flag: &str, v: &str, range: std::ops::RangeInclusive<u16>) -> Result<u16, String> {
    v.parse::<f64>()
        .ok()
        .map(|x| (x * 1000.0).round())
        .filter(|p| (*range.start() as f64..=*range.end() as f64).contains(p))
        .map(|p| p as u16)
        .ok_or(format!(
            "{flag} takes a multiple from {} to {}",
            *range.start() as f64 / 1000.0,
            *range.end() as f64 / 1000.0
        ))
}
