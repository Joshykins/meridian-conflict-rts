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
    pub energy_income: f32,
    pub mass_demand: f32,
    pub energy_demand: f32,
    pub efficiency: f32,
}

/// Everything the UI shows that is not in the render mirror.
#[derive(Clone, Debug, Default)]
pub struct SimStatus {
    /// The slot this machine plays; observers watch slot 0's colours but own nothing.
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
    /// Order queues of the units named in `SimHandle::watch`.
    pub queues: Vec<UnitOrders>,
    /// Every structure the watched side has planned and not begun.
    pub plans: Vec<PlannedBuild>,
    /// Set when the match cannot continue: a limit was hit, a desync, a lost connection.
    pub error: Option<String>,
}

pub struct Published {
    pub frame: RenderFrame,
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
}

pub struct SimHandle {
    pub shared: Shared,
    pub commands: Sender<Command>,
    /// Asks the session to stop its clock; only single-player sessions can.
    pub paused: Arc<AtomicBool>,
    /// Game speed in percent of real time; only single-player sessions follow it.
    pub speed: Arc<AtomicU32>,
    pub watch: Arc<Mutex<Watch>>,
    stop: Arc<AtomicBool>,
}

impl SimHandle {
    /// Copies out the latest published tick if it is newer than `serial`.
    /// Returns when it was published, for interpolation. Events are handed
    /// over exactly once.
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
        frame.clone_from(&p.frame);
        p.frame.events.clear();
        *status = p.status.clone();
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
}

pub fn status_of(world: &World, worst: u64) -> SimStatus {
    let s = &world.state;
    let f = |v: Fx| v.to_f32();
    let nav = world.nav.stats();
    SimStatus {
        local: None,
        tick: s.tick,
        hash: 0,
        players: s
            .players
            .iter()
            .map(|p| PlayerStatus {
                name: p.name.clone(),
                team: p.team,
                defeated: p.defeated,
                mass: f(p.mass),
                energy: f(p.energy),
                mass_capacity: f(p.mass_capacity),
                energy_capacity: f(p.energy_capacity),
                mass_income: f(p.mass_income),
                energy_income: f(p.energy_income),
                mass_demand: f(p.mass_demand),
                energy_demand: f(p.energy_demand),
                efficiency: f(p.efficiency),
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
        error: None,
    }
}

/// The orders the interface asked for. With cheats on (test scenes) any side's may be
/// asked for; otherwise only the local player's are handed out.
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
    status.plans.clear();
    if let Some(side) = side.filter(|s| (*s as usize) < world.state.players.len()) {
        world.write_plans(side, &mut status.plans);
    }
}

/// Spawns the sim thread. The world is built there too, so a big map loads
/// without blocking the window.
pub fn spawn(setup: SimSetup, mut session: Box<dyn Session + Send>) -> SimHandle {
    let shared: Shared = Arc::new(Mutex::new(Published {
        frame: RenderFrame::default(),
        status: SimStatus::default(),
        serial: 0,
        published_at: Instant::now(),
    }));
    let (tx, rx): (Sender<Command>, Receiver<Command>) = std::sync::mpsc::channel();
    let out = shared.clone();
    let (stop, paused) = (
        Arc::new(AtomicBool::new(false)),
        Arc::new(AtomicBool::new(false)),
    );
    let (stop_flag, paused_flag) = (stop.clone(), paused.clone());
    let speed = Arc::new(AtomicU32::new(100));
    let watch: Arc<Mutex<Watch>> = Arc::default();
    let (speed_flag, watch_list) = (speed.clone(), watch.clone());
    std::thread::Builder::new()
        .name("mc-sim".into())
        .spawn(move || {
            let fail = |message: String| {
                log::error!("{message}");
                let mut p = out.lock().unwrap();
                p.status.error = Some(message);
                p.serial += 1;
            };
            let mut world: Option<World> = None;
            let mut viewer: Option<u8> = None;
            let local = session.local_player().map(|p| p.0);
            let mut back = RenderFrame::default();
            let mut recent: std::collections::VecDeque<u64> = Default::default();
            let mut snapshot_at = None;
            let mut commands: Vec<PlayerCommand> = Vec::new();
            let mut prefetched = setup.prefetched;
            let mut is_paused = false;
            let mut speed_now = 100;
            let owns_clock = session.set_paused(false);
            let mut watched = Watch::default();
            loop {
                if stop_flag.load(Ordering::Relaxed) {
                    return;
                }
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
                let pending: Vec<Vec<u8>> = rx.try_iter().map(|c| c.encode()).collect();
                if !pending.is_empty() {
                    if let Err(e) = session.submit(pending) {
                        return fail(format!("could not send commands: {e}"));
                    }
                }
                let mut stepped = false;
                let events: Vec<SessionEvent> = prefetched.drain(..).chain(session.poll()).collect();
                for event in events {
                    match event {
                        SessionEvent::Started(start) => {
                            // Every machine derives the same match from the same start message.
                            let config = match crate::setup::config_from_start(&start) {
                                Ok(c) => c,
                                Err(e) => return fail(e),
                            };
                            viewer = if config.fog { local } else { None };
                            world = match World::new(&setup.map, setup.blueprints.clone(), setup.pool.clone(), &config) {
                                Ok(w) => Some(w),
                                Err(e) => return fail(e.to_string()),
                            };
                        }
                        SessionEvent::TickReady(bundle) => {
                            let Some(world) = world.as_mut() else { return fail("the session sent a tick before the match started".into()) };
                            commands.clear();
                            if let Some((first, second)) = &setup.scene {
                                match world.tick_count() {
                                    0 => commands.extend(first(world)),
                                    1 => commands.extend(second(world)),
                                    _ => {}
                                }
                            }
                            for (player, bytes) in bundle.commands() {
                                // Malformed input from a peer is ignored the same way everywhere.
                                if let Some(command) = Command::decode(bytes) {
                                    commands.push(PlayerCommand { player: player.0, command });
                                }
                            }
                            let hash = match world.tick(&commands) {
                                Ok(h) => h,
                                Err(e) => return fail(e.to_string()),
                            };
                            session.report_hash(bundle.tick, hash);
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
                            world.write_render_frame(viewer, &mut back);
                            let mut status = status_of(world, recent.iter().copied().max().unwrap_or(0));
                            status.hash = hash;
                            status.local = local;
                            status.owns_clock = owns_clock;
                            watched.clone_from(&watch_list.lock().unwrap());
                            write_watched(world, local, &watched, &mut status);
                            let mut p = out.lock().unwrap();
                            // Events of ticks the renderer never saw must not be lost.
                            if p.serial != 0 {
                                let mut carried = std::mem::take(&mut p.frame.events);
                                carried.append(&mut back.events);
                                carried.truncate(4096);
                                back.events = carried;
                            }
                            std::mem::swap(&mut p.frame, &mut back);
                            p.status = status;
                            p.serial += 1;
                            p.published_at = Instant::now();
                            stepped = true;
                        }
                        SessionEvent::SnapshotWanted { tick } => snapshot_at = Some(tick),
                        SessionEvent::SnapshotLoaded { blob, .. } => {
                            let base = match mc_map::Heightfield::load(&setup.map) {
                                Ok(t) => t,
                                Err(e) => return fail(e.to_string()),
                            };
                            let Some(world) = world.as_mut() else { return fail("the session sent a snapshot before the match started".into()) };
                            if let Err(e) = world.restore(base, &blob) {
                                return fail(e.to_string());
                            }
                        }
                        SessionEvent::Desync { tick, .. } => return fail(format!("desync detected at tick {tick}: this machine's simulation differs from the others")),
                        SessionEvent::Ended(reason) => {
                            log::info!("session ended: {reason:?}");
                            return;
                        }
                        _ => {}
                    }
                }
                // A new selection gets its queues now, not a tick later (or never, while paused).
                if let (false, Some(world)) = (stepped, world.as_ref()) {
                    let wanted = watch_list.lock().unwrap();
                    if *wanted != watched {
                        watched.clone_from(&wanted);
                        drop(wanted);
                        let mut p = out.lock().unwrap();
                        if p.serial != 0 {
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
        paused,
        speed,
        watch,
        stop,
    }
}
