//! The simulation runs on its own thread, driven by a session. The render
//! thread never waits for it: it picks up whatever mirror was published last
//! and keeps interpolating, so frame rate is independent of sim load.

use mc_core::Fx;
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::MapFile;
use mc_net::{Session, SessionEvent};
use mc_sim::mirror::{PlannedBuild, UnitOrders};
use mc_sim::{Command, PlayerCommand, RenderFrame, World};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Clone, Debug, Default)]
pub struct PlayerStatus {
    pub name: String,
    pub team: u8,
    pub defeated: bool,
    pub mass: f32,
    pub energy: f32,
    pub mass_capacity: f32,
    pub energy_capacity: f32,
    pub mass_income: f32,
    /// Materials a second coming in from reclaim, on top of `mass_income`.
    pub reclaim_income: f32,
    /// Materials a second the mines will add as they finish growing into their
    /// territories and digging out to their ore: `mass_income` plus this is the
    /// income once every mine is fully grown.
    pub mine_growth: f32,
    pub energy_income: f32,
    pub mass_demand: f32,
    pub energy_demand: f32,
    /// What was really spent: the demand as far as the stall let it be paid.
    pub mass_spent: f32,
    pub energy_spent: f32,
    pub efficiency: f32,
    /// How fast building actually goes against full speed (see `mc_sim::Player::build_speed`).
    pub build_speed: f32,
    /// Share of the mines' energy covered, and the materials a second lost for want of it.
    pub mine_power: f32,
    pub mine_lost: f32,
    /// How the side's economy pays for new mines and power in a stall, and how fast each
    /// paying tier builds (see `mc_sim::Player::tier_speed`).
    pub focus: mc_sim::focus::Focus,
    pub tier_speed: [Option<f32>; 3],
    /// What the side fields, counted by the sim (not the viewer's fogged picture).
    pub forces: Forces,
    pub units_lost: u32,
    pub units_killed: u32,
    /// What its AI is thinking, when a Commander plays it (the observer's overlay).
    pub mind: Option<mc_sim::ai::AiMind>,
}

/// A side's standing units, for the observer's panel.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Forces {
    /// Mobile fighters: not engineers, not the commander.
    pub army: u32,
    /// The army's worth in materials.
    pub army_value: f32,
    pub engineers: u32,
    pub factories: u32,
    pub mines: u32,
    pub generators: u32,
    /// Every finished structure, the above included.
    pub structures: u32,
}

/// Everything the UI shows that is not in the render mirror.
#[derive(Clone, Debug, Default)]
pub struct SimStatus {
    /// The slot this machine plays; observers have none and own nothing.
    pub local: Option<u8>,
    pub tick: u32,
    pub hash: u64,
    pub players: Vec<PlayerStatus>,
    /// Sim phase timings of the last tick, nanoseconds.
    pub phases: Vec<(&'static str, u64)>,
    pub tick_ns: u64,
    /// Worst tick of the last hundred.
    pub worst_tick_ns: u64,
    pub units: usize,
    pub projectiles: usize,
    pub wrecks: usize,
    pub stains: usize,
    pub orders: usize,
    pub flow_fields: usize,
    pub late_paths: u64,
    pub winner: Option<u8>,
    /// This machine owns the match clock: it can pause and change the game speed.
    pub owns_clock: bool,
    /// Order queues of the units named in `SimHandle::watch`, in unit id order (`queue_of`).
    pub queues: Vec<UnitOrders>,
    /// Every structure the watched side has planned and not begun.
    pub plans: Vec<PlannedBuild>,
    /// The core mines the watched side's allies have planned and not begun, with
    /// whose they are: placing a mine in one's reach is warned of.
    pub ally_mine_plans: Vec<(u8, PlannedBuild)>,
    /// Set when the match cannot continue: a limit was hit, a desync, a lost connection.
    pub error: Option<String>,
    /// Survival's rounds and nodes; None in any other match.
    pub survival: Option<mc_sim::SurvivalStatus>,
    /// A replay being watched: its length, and the tick a seek is running to.
    pub replay: Option<ReplayStatus>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ReplayStatus {
    pub length: u32,
    pub seeking: Option<u32>,
    /// The test range's weather as the recording showed it at this point.
    pub range_sky: Option<crate::range::RangeSky>,
}

pub struct Published {
    pub frame: RenderFrame,
    /// `frame` is a mirror the interface has not taken yet. Taking it swaps the
    /// interface's last frame in (`SimHandle::pull`), so the sim thread writes its next
    /// tick into that one: three frames go round, and none is copied under the lock.
    pub fresh: bool,
    pub status: SimStatus,
    /// Bumped on every publish.
    pub serial: u64,
    pub published_at: Instant,
}

pub type Shared = Arc<Mutex<Published>>;

/// Whose orders the interface wants published with every tick.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Watch {
    /// Units whose order queues are wanted (the selection).
    pub units: Vec<u32>,
    /// The side the interface commands: its planned structures are always published.
    pub side: u8,
    /// The queues of everything on that side are wanted as well (shift is held).
    pub everyone: bool,
    /// An observer looking through one side's eyes: its fog and what it can see.
    /// `None` sees everything. Ignored on a machine that plays.
    pub perspective: Option<u8>,
}

pub struct SimHandle {
    pub shared: Shared,
    pub commands: Sender<Command>,
    /// What this machine shows that a recording should keep (`recorder::Note`).
    pub notes: Sender<crate::recorder::Note>,
    /// A network match's link, chat and pause (`netplay.rs`); `None` on one machine.
    pub net: Option<crate::netplay::NetPlay>,
    /// Asks the session to stop its clock; only single-player sessions can.
    pub paused: Arc<AtomicBool>,
    /// Game speed in percent of real time; only single-player sessions follow it.
    pub speed: Arc<AtomicU32>,
    pub watch: Arc<Mutex<Watch>>,
    /// A replay being watched: the tick to jump to (`replay::Scrubber`).
    pub seek: Arc<Mutex<Option<u32>>>,
    /// The whole match as it went, for the battle report (`chronicle.rs`).
    pub chronicle: Arc<Mutex<crate::chronicle::Chronicle>>,
    stop: Arc<AtomicBool>,
}

impl SimHandle {
    /// Takes the latest published tick if it is newer than `serial`, swapping `frame`
    /// for it (the sim thread writes a later tick into the old one). Returns when it
    /// was published, for interpolation. Events are handed over exactly once.
    pub fn pull(
        &self,
        serial: &mut u64,
        frame: &mut RenderFrame,
        status: &mut SimStatus,
    ) -> Option<Instant> {
        let mut p = self.shared.lock().unwrap();
        if p.serial == *serial {
            return None;
        }
        *serial = p.serial;
        // A new status alone (the watched queues changed) leaves the frame as it is.
        if p.fresh {
            std::mem::swap(frame, &mut p.frame);
            p.fresh = false;
            // What comes back is the interface's old frame: its events were handed over.
            p.frame.events.clear();
        }
        status.clone_from(&p.status);
        Some(p.published_at)
    }
}

/// Leaving a match (or the front end's backdrop) ends its simulation.
impl Drop for SimHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// Commands a test scene injects locally. Single-machine sessions only:
/// they never go through the session, so they would desync a network match.
pub type SceneScript = Box<dyn Fn(&World) -> Vec<PlayerCommand> + Send>;

pub struct SimSetup {
    pub map: Arc<MapFile>,
    pub blueprints: Arc<Blueprints>,
    pub pool: Arc<Pool>,
    /// Session events already polled while waiting in the lobby, `Started` included.
    pub prefetched: Vec<SessionEvent>,
    /// Applied on the first and on the second tick.
    pub scene: Option<(SceneScript, SceneScript)>,
    /// A network match: the sim thread's side of `netplay`, and the interface's.
    pub net: Option<(crate::netplay::NetDriver, crate::netplay::NetPlay)>,
    /// Records what is played, for a match whose session does not record itself.
    pub recorder: Option<crate::recorder::Recorder>,
    /// The battle report's record is kept here once the match is decided, or as far
    /// as it went when it is left (`chronicle::file_for` the match's replay).
    pub keep_chronicle: Option<std::path::PathBuf>,
}

/// Counts every side's finished units.
fn forces_of(world: &World, players: &mut [PlayerStatus]) {
    use mc_data::cat;
    use mc_sim::tables::flag;
    let units = &world.state.units;
    for row in units.slots.iter() {
        let Some(p) = players.get_mut(units.owner[row] as usize) else {
            continue;
        };
        if units.flags[row] & (flag::UNDER_CONSTRUCTION | flag::IN_FACTORY | flag::UPGRADE) != 0 {
            continue;
        }
        let bp = world.bp(row);
        let f = &mut p.forces;
        if !bp.is_mobile() {
            f.structures += 1;
            f.factories += bp.has(cat::FACTORY) as u32;
            f.mines += bp.has(cat::EXTRACTOR) as u32;
            f.generators += bp.has(cat::POWER) as u32;
        } else if bp.has(cat::ENGINEER) {
            f.engineers += 1;
        } else if !bp.has(cat::COMMANDER) && !bp.is_salvager() {
            f.army += 1;
            f.army_value += bp.cost_mass.to_f32();
        }
    }
}

pub fn status_of(world: &World, worst: u64) -> SimStatus {
    let s = &world.state;
    let f = |v: Fx| v.to_f32();
    let nav = world.nav.stats();
    let mut status = SimStatus {
        local: None,
        tick: s.tick,
        hash: 0,
        players: s
            .players
            .iter()
            .enumerate()
            .map(|(i, p)| PlayerStatus {
                name: p.name.clone(),
                team: p.team,
                defeated: p.defeated,
                mass: f(p.mass),
                energy: f(p.energy),
                mass_capacity: f(p.mass_capacity),
                energy_capacity: f(p.energy_capacity),
                mass_income: f(p.mass_income),
                reclaim_income: f(p.reclaim_income),
                mine_growth: 0.0,
                energy_income: f(p.energy_income),
                mass_demand: f(p.mass_demand),
                energy_demand: f(p.energy_demand),
                mass_spent: f(p.mass_spent),
                energy_spent: f(p.energy_spent),
                efficiency: f(p.efficiency),
                build_speed: f(p.build_speed),
                mine_power: f(p.mine_power),
                mine_lost: f(p.mine_lost),
                focus: p.focus,
                tier_speed: p.tier_speed.map(|s| s.map(f)),
                forces: Forces::default(),
                units_lost: p.units_lost,
                units_killed: p.units_killed,
                mind: world.ai_mind(i as u8),
            })
            .collect(),
        phases: world.timings.phases.clone(),
        tick_ns: world.timings.total_ns,
        worst_tick_ns: worst,
        units: s.units.slots.live(),
        projectiles: s.projectiles.len(),
        wrecks: s.wrecks.slots.live(),
        stains: s.stains.len(),
        orders: s.orders.live(),
        flow_fields: nav.live_fields,
        late_paths: nav.late_joins,
        winner: s.winner,
        owns_clock: false,
        queues: Vec::new(),
        plans: Vec::new(),
        ally_mine_plans: Vec::new(),
        error: None,
        survival: world.survival_status(),
        replay: None,
    };
    forces_of(world, &mut status.players);
    mine_growth_of(world, &mut status.players);
    status
}

/// What every side's mines have still to grow into, materials a second.
fn mine_growth_of(world: &World, players: &mut [PlayerStatus]) {
    let units = &world.state.units;
    for (&id, m) in &world.state.mines.by_unit {
        let Some(row) = units.row(id) else {
            continue;
        };
        let Some(bp) = world.bp(row).mine else {
            continue;
        };
        let owner = units.owner[row] as usize;
        let (Some(p), Some(pl)) = (players.get_mut(owner), world.state.players.get(owner)) else {
            continue;
        };
        // The test range's income dial turns the mines' output too.
        let dial = pl.income_permille[0] as f32 / 1000.0;
        p.mine_growth += (m.full_rate(&bp) - m.rate(&bp)).max(Fx::ZERO).to_f32() * dial;
    }
}

/// The orders the interface asked for. With cheats on (test scenes) any side's may be
/// asked for; otherwise only the local player's are handed out.
/// Puts order queues in unit id order, which `queue_of` looks them up by.
pub fn sort_queues(queues: &mut [UnitOrders]) {
    queues.sort_unstable_by_key(|q| q.unit_id);
}

/// The queue of unit `id` among queues in unit id order (`sort_queues`; `SimStatus::queues`
/// always is): a binary search, since the interface asks for the queue of each of a
/// few thousand selected units every frame.
pub fn queue_of(queues: &[UnitOrders], id: u32) -> Option<&UnitOrders> {
    queues
        .binary_search_by_key(&id, |q| q.unit_id)
        .ok()
        .map(|i| &queues[i])
}

fn write_watched(world: &World, local: Option<u8>, watch: &Watch, status: &mut SimStatus) {
    let viewer = if world.state.cheats { None } else { local };
    let side = if world.state.cheats {
        Some(watch.side)
    } else {
        local
    };
    world.write_orders(
        viewer,
        &watch.units,
        side.filter(|_| watch.everyone),
        &mut status.queues,
    );
    sort_queues(&mut status.queues);
    status.plans.clear();
    if local.is_none() {
        // An observer sees every side's planned structures.
        for i in 0..world.state.players.len() {
            let mut extra = Vec::new();
            world.write_plans(i as u8, &mut extra);
            status.plans.extend(extra);
        }
    } else if let Some(side) = side.filter(|s| (*s as usize) < world.state.players.len()) {
        world.write_plans(side, &mut status.plans);
    }
    write_ally_mine_plans(world, local.and(side), &mut status.ally_mine_plans);
}

/// The core mines `side`'s allies have planned, for the warning when the side places one
/// in their reach. Observers place nothing.
fn write_ally_mine_plans(world: &World, side: Option<u8>, out: &mut Vec<(u8, PlannedBuild)>) {
    out.clear();
    let Some(side) = side.filter(|s| (*s as usize) < world.state.players.len()) else {
        return;
    };
    let team = world.state.players[side as usize].team;
    let mut plans = Vec::new();
    for (i, p) in world.state.players.iter().enumerate() {
        if i == side as usize || p.team != team {
            continue;
        }
        world.write_plans(i as u8, &mut plans);
        out.extend(
            plans
                .drain(..)
                .filter(|b| world.blueprints.unit(b.blueprint).mine.is_some())
                .map(|b| (i as u8, b)),
        );
    }
}

/// Whose eyes the render frame is drawn through: the player's own, or, for an
/// observer in a fogged match, whichever side it chose (`None`: all of them).
fn eyes(fog: bool, local: Option<u8>, watch: &Watch) -> Option<u8> {
    if fog {
        local.or(watch.perspective)
    } else {
        None
    }
}

/// Spawns the sim thread. The world is built there too, so a big map loads
/// without blocking the window.
pub fn spawn(setup: SimSetup, mut session: Box<dyn Session + Send>) -> SimHandle {
    let shared: Shared = Arc::new(Mutex::new(Published {
        frame: RenderFrame::default(),
        fresh: false,
        status: SimStatus::default(),
        serial: 0,
        published_at: Instant::now(),
    }));
    let (tx, rx): (Sender<Command>, Receiver<Command>) = std::sync::mpsc::channel();
    let (notes_tx, notes_rx) = std::sync::mpsc::channel::<crate::recorder::Note>();
    let out = shared.clone();
    let (stop, paused) = (
        Arc::new(AtomicBool::new(false)),
        Arc::new(AtomicBool::new(false)),
    );
    let (stop_flag, paused_flag) = (stop.clone(), paused.clone());
    let speed = Arc::new(AtomicU32::new(100));
    let watch: Arc<Mutex<Watch>> = Arc::default();
    let (speed_flag, watch_list) = (speed.clone(), watch.clone());
    let seek: Arc<Mutex<Option<u32>>> = Arc::default();
    let seek_asked = seek.clone();
    let chronicle = Arc::new(Mutex::new(crate::chronicle::Chronicle::new(
        glam::Vec2::from(setup.map.info().size_metres().to_f32()),
    )));
    let chronicle_kept = chronicle.clone();
    let (mut net, net_play) = match setup.net {
        Some((driver, play)) => (Some(driver), Some(play)),
        None => (None, None),
    };
    std::thread::Builder::new()
        .name("mc-sim".into())
        .spawn(move || {
            // The match cannot go on without this thread: a panic here ends the game
            // with the crash window, rather than leaving it frozen.
            let _fatal = crate::crash::FatalGuard;
            let fail = |message: String| {
                log::error!("{message}");
                let mut p = out.lock().unwrap();
                p.status.error = Some(message);
                p.serial += 1;
            };
            use mc_net::EndReason;
            let mut world: Option<World> = None;
            let mut fog = false;
            let local = session.local_player().map(|p| p.0);
            let mut back = RenderFrame::default();
            let mut recent: std::collections::VecDeque<u64> = Default::default();
            let mut snapshot_at = None;
            let mut commands: Vec<PlayerCommand> = Vec::new();
            let mut prefetched = setup.prefetched;
            let mut is_paused = false;
            let mut speed_now = 100;
            // Only a match on this machine alone has a clock to own; a network match pauses
            // through the relay, for everyone.
            let owns_clock = net.is_none() && session.set_paused(false);
            let mut watched = Watch::default();
            let mut scrub = session.length().map(|_| crate::replay::Scrubber::new());
            let mut recorder = setup.recorder;
            let blueprint_hash = setup.blueprints.content_hash();
            let mut keep_chronicle = setup.keep_chronicle;
            let mut keep = |chronicle: &crate::chronicle::Chronicle| {
                if let Some(path) = keep_chronicle.take().filter(|_| !chronicle.samples.is_empty()) {
                    if let Err(e) = chronicle.save(&path, blueprint_hash) {
                        log::warn!("the battle report's record was not kept: {e}");
                    }
                }
            };
            // A replay: the range's weather as it was recorded, by the last note played.
            let mut range_sky = None;
            loop {
                if stop_flag.load(Ordering::Relaxed) {
                    keep(&chronicle_kept.lock().unwrap());
                    return;
                }
                if let Some(n) = &mut net {
                    n.serve(&mut *session);
                    if n.rejoining() {
                        match n.try_rejoin() {
                            Ok(Some(fresh)) => session = Box::new(fresh),
                            Ok(None) => {}
                            Err(e) => return fail(e),
                        }
                    }
                } else {
                    if paused_flag.load(Ordering::Relaxed) != is_paused {
                        is_paused = !is_paused;
                        if !session.set_paused(is_paused) {
                            log::debug!("this session cannot pause");
                        }
                    }
                    let speed_wanted = speed_flag.load(Ordering::Relaxed);
                    if speed_wanted != speed_now {
                        speed_now = speed_wanted;
                        session.set_speed(speed_now);
                    }
                }
                // Asked before the world is built (a replay opened at a mark): kept until it is.
                let asked = world.as_ref().and_then(|_| seek_asked.lock().unwrap().take());
                if let (Some(to), Some(s), Some(world)) = (asked, &mut scrub, world.as_mut()) {
                    if let Err(e) = s.seek(world, &setup.map, session.as_mut(), to) {
                        log::warn!("could not jump to tick {to}: {e}");
                    }
                }
                for note in notes_rx.try_iter() {
                    if let Some(r) = &mut recorder {
                        r.note(note.encode());
                    }
                }
                // An order to a whole army travels as one command.
                const _: () = assert!(
                    mc_sim::command::MAX_COMMAND_BYTES as usize <= mc_net::MAX_COMMAND_LEN
                );
                let pending: Vec<Vec<u8>> = rx.try_iter().map(|c| c.encode()).collect();
                if !pending.is_empty() {
                    if let Err(e) = session.submit(pending) {
                        // Orders given while the connection is down have nowhere to go.
                        if net.as_ref().is_some_and(|n| n.rejoining()) {
                            log::warn!("orders dropped while reconnecting: {e}");
                        } else {
                            return fail(format!("could not send commands: {e}"));
                        }
                    }
                }
                let mut stepped = false;
                let events: Vec<SessionEvent> = prefetched.drain(..).chain(session.poll()).collect();
                for event in events {
                    if net.as_mut().is_some_and(|n| n.on_event(&event)) {
                        continue;
                    }
                    // Orders given on pause: carried out, published, but no time passes.
                    let held = matches!(event, SessionEvent::HeldReady(_));
                    match event {
                        // Back after a lost connection: the match stands, a snapshot follows.
                        SessionEvent::Started(_) if world.is_some() => {}
                        SessionEvent::Started(start) => {
                            if let Some(r) = &mut recorder {
                                r.started(&start);
                            }
                            // Every machine derives the same match from the same start message.
                            let options = match crate::match_options::MatchOptions::from_start(&start) {
                                Ok(o) => o,
                                Err(e) => return fail(e),
                            };
                            let config = options.config;
                            fog = config.fog;
                            world = match World::new(&setup.map, setup.blueprints.clone(), setup.pool.clone(), &config) {
                                Ok(w) => Some(w),
                                Err(e) => return fail(e.to_string()),
                            };
                            if let Some(w) = &world {
                                let kind = if net.is_some() {
                                    "a network match"
                                } else if scrub.is_some() {
                                    "a replay"
                                } else {
                                    "on this machine"
                                };
                                crate::crash::context(
                                    "match",
                                    format!(
                                        "{:?}, {} sides, fog {}, {kind}, playing as {local:?}",
                                        setup.map.name(),
                                        w.state.players.len(),
                                        if fog { "on" } else { "off" },
                                    ),
                                );
                            }
                            if let Some(survival) = options.survival {
                                if let Err(e) = world.as_mut().unwrap().begin_survival(survival) {
                                    return fail(e.to_string());
                                }
                            }
                            if let (Some(s), Some(world)) = (&mut scrub, world.as_mut()) {
                                s.keep(world);
                            }
                            // A network match's clock waits for every machine to get here.
                            session.loaded();
                        }
                        SessionEvent::TickReady(_) | SessionEvent::HeldReady(_)
                            if net.as_ref().is_some_and(|n| n.frozen) => {}
                        SessionEvent::TickReady(bundle) | SessionEvent::HeldReady(bundle) => {
                            let Some(world) = world.as_mut() else { return fail("the session sent a tick before the match started".into()) };
                            commands.clear();
                            if let (false, Some((first, second))) = (held, &setup.scene) {
                                match world.tick_count() {
                                    0 => commands.extend(first(world)),
                                    1 => commands.extend(second(world)),
                                    _ => {}
                                }
                            }
                            let staged = commands.len();
                            for (player, bytes) in bundle.commands() {
                                // Malformed input from a peer is ignored the same way everywhere.
                                if let Some(command) = Command::decode(bytes) {
                                    commands.push(PlayerCommand { player: player.0, command });
                                }
                            }
                            if let Some(r) = &mut recorder {
                                // What the scene staged comes first; the rest is the bundle's.
                                if held {
                                    r.held(&bundle);
                                } else {
                                    r.tick(&bundle, &commands[..staged]);
                                }
                            }
                            let hash = if held {
                                match world.apply_held(&commands) {
                                    Ok(h) => h,
                                    Err(e) => return fail(e.to_string()),
                                }
                            } else {
                                let sections = match world.tick_sections(&commands) {
                                    Ok(s) => s,
                                    Err(e) => return fail(e.to_string()),
                                };
                                if let Some(n) = &mut net {
                                    n.record(bundle.tick, sections);
                                    n.ticking();
                                }
                                mc_sim::state_hash::combine(&sections)
                            };
                            if !held {
                                session.report_hash(bundle.tick, hash);
                                crate::crash::sim_tick(bundle.tick);
                                if let Some(r) = &mut recorder {
                                    r.hash(bundle.tick, hash);
                                }
                                if snapshot_at == Some(bundle.tick) {
                                    if let Err(e) = session.provide_snapshot(bundle.tick, world.snapshot()) {
                                        log::warn!("snapshot for a joining player was not sent: {e}");
                                    }
                                    snapshot_at = None;
                                }
                                recent.push_back(world.timings.total_ns);
                                if recent.len() > 100 {
                                    recent.pop_front();
                                }
                            }
                            {
                                let mut chronicle = chronicle_kept.lock().unwrap();
                                chronicle.record(world);
                                if chronicle.ended.is_some() {
                                    keep(&chronicle);
                                }
                            }
                            // A seek runs up to its tick without drawing, showing progress now and then.
                            let rushing = match &mut scrub {
                                Some(s) => {
                                    if !held {
                                        s.keep(world);
                                    }
                                    s.rushing(world)
                                }
                                None => false,
                            };
                            if rushing && !world.tick_count().is_multiple_of(50) {
                                stepped = true;
                                continue;
                            }
                            watched.clone_from(&watch_list.lock().unwrap());
                            world.write_render_frame(eyes(fog, local, &watched), &mut back);
                            if rushing {
                                back.events.clear();
                            }
                            let mut status = status_of(world, recent.iter().copied().max().unwrap_or(0));
                            status.hash = hash;
                            status.local = local;
                            status.owns_clock = owns_clock;
                            write_watched(world, local, &watched, &mut status);
                            status.replay = scrub.as_ref().map(|s| ReplayStatus {
                                length: session.length().unwrap_or(0),
                                seeking: s.target,
                                range_sky,
                            });
                            let mut p = out.lock().unwrap();
                            // Events of ticks the renderer never saw must not be lost.
                            if p.serial != 0 {
                                let mut carried = std::mem::take(&mut p.frame.events);
                                carried.append(&mut back.events);
                                carried.truncate(4096);
                                back.events = carried;
                            }
                            std::mem::swap(&mut p.frame, &mut back);
                            p.fresh = true;
                            p.status = status;
                            p.serial += 1;
                            // Held: the frame is new but no time passed, so nothing re-interpolates.
                            if !held {
                                p.published_at = Instant::now();
                                session.credit_tick();
                            }
                            stepped = true;
                        }
                        SessionEvent::SnapshotWanted { tick } => snapshot_at = Some(tick),
                        SessionEvent::SnapshotLoaded { tick, blob } => {
                            if let Some(r) = &mut recorder {
                                r.restored(tick);
                            }
                            let base = match mc_map::Heightfield::load(&setup.map) {
                                Ok(t) => t,
                                Err(e) => return fail(e.to_string()),
                            };
                            let Some(world) = world.as_mut() else { return fail("the session sent a snapshot before the match started".into()) };
                            if let Err(e) = world.restore(base, &blob) {
                                return fail(e.to_string());
                            }
                            if let Some(n) = &mut net {
                                n.restored();
                            }
                        }
                        SessionEvent::Desync { tick, hashes } => match (&mut net, world.as_mut()) {
                            // The match stops, but the thread stays to hear the other players' reports.
                            (Some(n), Some(w)) => fail(n.desync(tick, &hashes, w, &mut *session)),
                            _ => return fail(format!("desync detected at tick {tick}: this machine's simulation differs from the others")),
                        },
                        SessionEvent::Ended(reason) => {
                            if net.as_mut().is_some_and(|n| n.lost(&reason)) {
                                break;
                            }
                            log::info!("session ended: {reason:?}");
                            if let Some(r) = &mut recorder {
                                r.finish();
                            }
                            match reason {
                                EndReason::Finished => return,
                                EndReason::Refused { reason, detail } => {
                                    return fail(format!("{}: {detail}", reason.describe()))
                                }
                                EndReason::ConnectionLost(e) => {
                                    // A desync already said why the match stopped.
                                    if net.as_ref().is_some_and(|n| n.frozen) {
                                        return;
                                    }
                                    return fail(format!("the connection to the match was lost: {e}"));
                                }
                            }
                        }
                        SessionEvent::Note(bytes) => match crate::recorder::Note::decode(&bytes) {
                            Some(crate::recorder::Note::RangeSky(sky)) => range_sky = Some(sky),
                            None => log::debug!("a note this build does not know was skipped"),
                        },
                        _ => {}
                    }
                }
                // A new selection gets its queues now, not a tick later (or never, while paused).
                if let (false, Some(world)) = (stepped, world.as_ref()) {
                    let wanted = watch_list.lock().unwrap();
                    if *wanted != watched {
                        let looked = eyes(fog, local, &watched);
                        watched.clone_from(&wanted);
                        drop(wanted);
                        let mut p = out.lock().unwrap();
                        if p.serial != 0 {
                            // A paused observer switching eyes sees the change at once.
                            let eyes_now = eyes(fog, local, &watched);
                            if eyes_now != looked {
                                let events = std::mem::take(&mut p.frame.events);
                                world.write_render_frame(eyes_now, &mut p.frame);
                                p.frame.events = events;
                                p.fresh = true;
                            }
                            write_watched(world, local, &watched, &mut p.status);
                            p.serial += 1;
                        }
                    }
                }
                if !stepped {
                    std::thread::sleep(std::time::Duration::from_millis(2));
                }
            }
        })
        .expect("spawn sim thread");
    SimHandle {
        shared,
        commands: tx,
        notes: notes_tx,
        net: net_play,
        paused,
        speed,
        watch,
        seek,
        chronicle,
        stop,
    }
}
