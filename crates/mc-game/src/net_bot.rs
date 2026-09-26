//! A headless network player for soak tests: joins a relay, runs the whole
//! simulation like any client, and plays by fuzzing orders.
//!
//! `meridian --connect HOST:PORT --bot chaos --ticks 6000` plays 10 minutes and
//! exits with the final hash. Several of these against one relay, with AI
//! commanders filling the other seats, is a long real match with no window:
//! `scripts/net-soak.sh` runs it. A desync ends the run with exit code 3 and
//! the sections of the state that differed.
//!
//! The chaos bot is not trying to win. It issues every kind of command, with
//! valid, stale and nonsense arguments, so the command path and every order
//! system run under lockstep. `idle` only keeps up.

use crate::setup;
use mc_core::{Angle, Fx, FxVec2, Rng};
use mc_data::{BlueprintId, Blueprints};
use mc_jobs::Pool;
use mc_map::MapFile;
use mc_net::{Session, SessionEvent};
use mc_sim::state_hash::{self, SectionHashes};
use mc_sim::tables::FireState;
use mc_sim::{Command, PlayerCommand, UnitId, World};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Bot {
    Idle,
    Chaos,
}

impl Bot {
    pub fn parse(s: &str) -> Option<Bot> {
        match s {
            "idle" => Some(Bot::Idle),
            "chaos" => Some(Bot::Chaos),
            _ => None,
        }
    }
}

pub struct BotRun {
    pub addr: String,
    pub name: String,
    pub bot: Bot,
    /// Leave after this many ticks; 0 plays until the match ends.
    pub ticks: u32,
    /// Hang up at this tick and come back with the reconnect token.
    pub drop_at: Option<u32>,
}

/// Section hashes of recent ticks, for the desync report.
const KEPT_SECTIONS: usize = 1024;

/// Exit code for a desync, so the soak script can tell it from other failures.
pub const DESYNC_EXIT: i32 = 3;

pub fn run(
    run: BotRun,
    map: Arc<MapFile>,
    blueprints: Arc<Blueprints>,
    pool: Arc<Pool>,
    template: Vec<u8>,
) -> Result<(), String> {
    let content = mc_net::ContentId {
        map_id: map.content_id(),
        blueprint_hash: blueprints.content_hash(),
    };
    let (mut session, mut pending, slot) =
        crate::app::lobby(&run.addr, &run.name, content, template.clone())?;
    let mut token = session.token();
    log::info!("{}: playing slot {slot} as a {:?} bot", run.name, run.bot);
    let mut world: Option<World> = None;
    let mut rng = Rng::new(0x5EED ^ slot as u64);
    let mut recent: VecDeque<(u32, SectionHashes)> = VecDeque::with_capacity(KEPT_SECTIONS);
    let mut snapshot_at = None;
    let mut dropped = false;
    let started = Instant::now();
    loop {
        let events: Vec<SessionEvent> = pending.drain(..).chain(session.poll()).collect();
        if events.is_empty() {
            std::thread::sleep(Duration::from_millis(2));
        }
        for event in events {
            match event {
                SessionEvent::Joined(w) => token = Some(w.token),
                SessionEvent::Started(start) => {
                    // A reconnect is sent the start again, then a snapshot of now.
                    let config = setup::config_from_start(&start)?;
                    let mut w = World::new(&map, blueprints.clone(), pool.clone(), &config)
                        .map_err(|e| e.to_string())?;
                    if let Some(survival) = crate::survival::from_start(&start)? {
                        w.begin_survival(survival).map_err(|e| e.to_string())?;
                    }
                    world = Some(w);
                    session.loaded();
                }
                SessionEvent::SnapshotLoaded { tick, blob } => {
                    let w = world.as_mut().ok_or("a snapshot came before the start")?;
                    let base = mc_map::Heightfield::load(&map).map_err(|e| e.to_string())?;
                    w.restore(base, &blob).map_err(|e| e.to_string())?;
                    log::info!("{}: re-entered the match at tick {tick}", run.name);
                }
                SessionEvent::SnapshotWanted { tick } => snapshot_at = Some(tick),
                SessionEvent::TickReady(bundle) => {
                    let w = world.as_mut().ok_or("a tick came before the start")?;
                    let commands: Vec<PlayerCommand> = bundle
                        .commands()
                        .filter_map(|(p, bytes)| {
                            Command::decode(bytes).map(|command| PlayerCommand {
                                player: p.0,
                                command,
                            })
                        })
                        .collect();
                    w.tick(&commands).map_err(|e| e.to_string())?;
                    let sections = w.hash_sections();
                    let hash = state_hash::combine(&sections);
                    session.report_hash(bundle.tick, hash);
                    if recent.len() == KEPT_SECTIONS {
                        recent.pop_front();
                    }
                    recent.push_back((bundle.tick, sections));
                    if snapshot_at == Some(bundle.tick) {
                        if let Err(e) = session.provide_snapshot(bundle.tick, w.snapshot()) {
                            log::warn!("{}: snapshot not sent: {e}", run.name);
                        }
                        snapshot_at = None;
                    }
                    session.credit_tick();
                    if bundle.tick % 1000 == 0 {
                        log::info!(
                            "{}: tick {} hash {hash:016x}, {} units, {:.0} s",
                            run.name,
                            bundle.tick,
                            w.state.units.slots.live(),
                            started.elapsed().as_secs_f32()
                        );
                    }
                    if run.bot == Bot::Chaos {
                        let orders =
                            chaos(w, slot, &blueprints, map.info().size_metres(), &mut rng);
                        session.submit(orders).map_err(|e| e.to_string())?;
                    }
                    if run.ticks > 0 && bundle.tick + 1 >= run.ticks {
                        println!(
                            "net-bot: {} slot {slot} tick {} hash {hash:016x}",
                            run.name, bundle.tick
                        );
                        return Ok(());
                    }
                    if !dropped && run.drop_at == Some(bundle.tick) {
                        dropped = true;
                        log::info!("{}: hanging up at tick {}", run.name, bundle.tick);
                        drop(session);
                        std::thread::sleep(Duration::from_millis(1500));
                        let mut config =
                            crate::app::net_config(&run.name, mc_net::Role::Player, content);
                        config.token = token;
                        session = mc_net::NetSession::connect(run.addr.as_str(), config)
                            .map_err(|e| format!("reconnect: {e}"))?;
                        recent.clear();
                        snapshot_at = None;
                        // The rest of this poll belongs to the old connection.
                        break;
                    }
                }
                SessionEvent::Desync { tick, hashes } => {
                    let ours = recent.iter().find(|(t, _)| *t == tick);
                    eprintln!("net-bot: {} DESYNC at tick {tick}: {hashes:x?}", run.name);
                    if let Some((_, sections)) = ours {
                        for (name, h) in state_hash::SECTIONS.iter().zip(sections) {
                            eprintln!("net-bot:   {name:<12} {h:016x}");
                        }
                    }
                    std::process::exit(DESYNC_EXIT);
                }
                SessionEvent::Ended(reason) => {
                    return Err(format!("{}: the session ended: {reason:?}", run.name));
                }
                _ => {}
            }
        }
    }
}

/// Between none and a few orders a tick, of every kind, some of them nonsense.
fn chaos(
    world: &World,
    slot: u8,
    blueprints: &Blueprints,
    size: FxVec2,
    rng: &mut Rng,
) -> Vec<Vec<u8>> {
    let units = &world.state.units;
    let mine: Vec<usize> = units
        .slots
        .iter()
        .filter(|&r| units.owner[r] == slot)
        .collect();
    let theirs: Vec<usize> = units
        .slots
        .iter()
        .filter(|&r| units.owner[r] != slot)
        .collect();
    let mut out = Vec::new();
    // About one order every third of a second, in bursts.
    let count = match rng.below(10) {
        0..=6 => 0,
        7 | 8 => 1,
        _ => 1 + rng.below(4),
    };
    for _ in 0..count {
        let n = 1 + rng.below(24) as usize;
        let group: Vec<UnitId> = pick(rng, &mine, n)
            .into_iter()
            .map(|r| units.id(r))
            .collect();
        let near = |rng: &mut Rng, r: Option<usize>| -> FxVec2 {
            let base = r.map_or(FxVec2::new(size.x / 2, size.y / 2), |r| units.pos[r]);
            let d = |rng: &mut Rng| Fx::from_int(rng.below(1600) as i32 - 800);
            let p = base + FxVec2::new(d(rng), d(rng));
            FxVec2::new(p.x.clamp(Fx::ZERO, size.x), p.y.clamp(Fx::ZERO, size.y))
        };
        let anchor = mine
            .get(rng.below(mine.len().max(1) as u32) as usize)
            .copied();
        let target = theirs
            .get(rng.below(theirs.len().max(1) as u32) as usize)
            .map(|&r| units.id(r))
            .unwrap_or(mc_sim::slots::Handle(rng.next_u32()));
        let any_bp = BlueprintId(rng.below(blueprints.units.len() as u32 + 4) as u16);
        let buildable = anchor
            .and_then(|r| world.bp(r).builder.as_ref())
            .and_then(|b| {
                b.builds
                    .get(rng.below(b.builds.len().max(1) as u32) as usize)
            })
            .copied()
            .unwrap_or(any_bp);
        let pos = near(rng, anchor);
        let queue = rng.below(3) == 0;
        let command = match rng.below(34) {
            0..=4 => Command::Move {
                units: group,
                target: pos,
                queue,
            },
            5..=7 => Command::AttackMove {
                units: group,
                target: pos,
                queue,
            },
            8 => Command::FormationMove {
                units: group,
                target: pos,
                queue,
                attack_move: rng.below(2) == 0,
                together: rng.below(2) == 0,
                spacing: rng.below(4) as u8,
            },
            9 | 10 => Command::Attack {
                units: group,
                target,
                queue,
            },
            11 | 12 => Command::Build {
                units: group,
                blueprint: buildable,
                pos,
                heading: Angle(rng.next_u32() as u16),
                queue,
            },
            13 => Command::Produce {
                factories: group,
                blueprint: buildable,
                count: 1 + rng.below(5) as u8,
            },
            14 => Command::Patrol {
                units: group,
                points: (0..1 + rng.below(4)).map(|_| near(rng, anchor)).collect(),
                queue,
            },
            15 => Command::Guard {
                units: group,
                pos,
                radius: Fx::from_int(40 + rng.below(600) as i32),
                queue,
            },
            16 => Command::Stop { units: group },
            17 => Command::Assist {
                units: group,
                target: mine
                    .get(rng.below(mine.len().max(1) as u32) as usize)
                    .map_or(target, |&r| units.id(r)),
                queue,
            },
            18 => Command::ReclaimUnit {
                units: group,
                target,
                queue,
            },
            19 => {
                let wrecks = &world.state.wrecks.slots;
                let rows: Vec<usize> = wrecks.iter().collect();
                match rows.get(rng.below(rows.len().max(1) as u32) as usize) {
                    Some(&w) => Command::ReclaimWreck {
                        units: group,
                        wreck: wrecks.handle(w),
                        queue,
                    },
                    None => Command::Stop { units: group },
                }
            }
            20 => Command::Upgrade { units: group },
            21 => Command::SetFireState {
                units: group,
                state: if rng.below(2) == 0 {
                    FireState::HoldFire
                } else {
                    FireState::FireAtWill
                },
            },
            22 => Command::AttackGround {
                units: group,
                pos,
                queue,
            },
            23 => Command::Bombard {
                units: group,
                pos,
                radius: Fx::from_int(rng.below(400) as i32),
                queue,
            },
            24 => Command::Reform {
                units: group,
                together: rng.below(2) == 0,
                spacing: rng.below(3) as u8,
            },
            25 => Command::SetDive {
                units: group,
                dive: rng.below(2) == 0,
            },
            26 => Command::SetPaused {
                units: group,
                paused: rng.below(3) == 0,
            },
            27 => Command::SetRally {
                factories: group,
                pos,
            },
            28 => Command::SetRepeat {
                factories: group,
                repeat: rng.below(2) == 0,
            },
            29 => Command::Orbit {
                units: group,
                pos,
                target,
                radius: Fx::from_int(rng.below(1300) as i32),
                queue,
            },
            30 => Command::LaunchNuke { units: group, pos },
            31 => Command::Strike {
                units: group,
                pos,
                queue,
            },
            // Refused without cheats, on every machine alike.
            32 => Command::DebugSpawn {
                owner: slot,
                blueprint: any_bp,
                pos,
                heading: Angle(0),
                count: 5,
                flags: 0,
                build: 1000,
            },
            _ => {
                // Bytes that do not decode are skipped everywhere the same way.
                let len = rng.below(40) as usize;
                out.push((0..len).map(|_| rng.next_u32() as u8).collect());
                continue;
            }
        };
        out.push(command.encode());
    }
    out
}

/// Up to `n` distinct entries of `from`, in a random order.
fn pick(rng: &mut Rng, from: &[usize], n: usize) -> Vec<usize> {
    let mut pool = from.to_vec();
    let mut out = Vec::new();
    while !pool.is_empty() && out.len() < n {
        let i = rng.below(pool.len() as u32) as usize;
        out.push(pool.swap_remove(i));
    }
    out
}
