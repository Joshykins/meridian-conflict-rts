//! Lockstep networking: wire protocol, relay server, sessions and replay files.
//!
//! Only commands cross the network. Every machine runs the full simulation and
//! feeds it the same [`TickBundle`] for every tick; per-tick state hashes catch
//! the case where that was not enough. This crate is ignorant of game rules: a
//! command is an opaque byte string from `mc-sim`, a snapshot is an opaque
//! blob, a state hash is a `u64`.
//!
//! # The session contract
//!
//! The game loop talks to a [`Session`] and cannot tell single-player
//! ([`LocalSession`]), multiplayer ([`NetSession`]) and playback
//! ([`ReplaySession`]) apart. The one rule that keeps lockstep correct — *the
//! sim may advance to tick T only when it holds the bundle for T* — is carried
//! by the event stream itself, so the caller has no tick counter to get wrong:
//!
//! * `Started(MatchStart)` arrives first. Build tick-0 state from it.
//! * Each `TickReady(bundle)` means: apply `bundle.commands()` in iteration
//!   order, then step the sim exactly once, then call `credit_tick` so a local
//!   clock that overran does not mint catch-up ticks. Ticks arrive in order,
//!   without gaps, and at the pace the match should run; the sim never steps
//!   for any other reason. No `TickReady` this frame means the sim waits (the
//!   renderer keeps interpolating).
//! * `SnapshotLoaded { tick, blob }` replaces the whole sim state; the next
//!   `TickReady` is `tick + 1`. It is how late joiners, reconnecting players
//!   and observers enter a running match.
//! * `SnapshotWanted { tick }` always precedes `TickReady(tick)`. Right after
//!   stepping that tick, serialise the sim and call `provide_snapshot`.
//! * After stepping, call `report_hash(tick, hash)`; every tick or every Nth,
//!   as long as all machines choose the same ticks.
//! * `submit` whenever the player acts. The session decides which tick executes
//!   the commands (current tick plus input delay) and the commands come back
//!   inside a later `TickReady` — the sim never applies local input directly.
//!
//! ```no_run
//! # use mc_net::*;
//! # fn demo(mut session: Box<dyn Session>) {
//! # struct Sim; impl Sim {
//! #   fn new(_: &MatchStart) -> Sim { Sim } fn load(&mut self, _: &[u8]) {} fn save(&self) -> Vec<u8> { vec![] }
//! #   fn apply(&mut self, _: mc_core::PlayerId, _: &[u8]) {} fn step(&mut self) {} fn hash(&self) -> u64 { 0 } }
//! # let ui_commands: Vec<Vec<u8>> = vec![];
//! let mut sim: Option<Sim> = None;
//! let mut snapshot_at = None;
//! loop {
//!     session.submit(ui_commands.clone()).expect("show this to the player");
//!     for event in session.poll() {
//!         match event {
//!             SessionEvent::Started(start) => sim = Some(Sim::new(&start)),
//!             SessionEvent::SnapshotLoaded { blob, .. } => sim.as_mut().unwrap().load(&blob),
//!             SessionEvent::SnapshotWanted { tick } => snapshot_at = Some(tick),
//!             SessionEvent::TickReady(bundle) => {
//!                 let sim = sim.as_mut().unwrap();
//!                 for (player, command) in bundle.commands() {
//!                     sim.apply(player, command);
//!                 }
//!                 sim.step();
//!                 session.credit_tick();
//!                 session.report_hash(bundle.tick, sim.hash());
//!                 if snapshot_at == Some(bundle.tick) {
//!                     session.provide_snapshot(bundle.tick, sim.save()).unwrap();
//!                 }
//!             }
//!             SessionEvent::Ended(_) => return,
//!             _ => {}
//!         }
//!     }
//!     // render
//! }
//! # }
//! ```
//!
//! # Determinism
//!
//! The relay is the single author of bundle contents (see [`relay`]), and a
//! bundle lists commands by ascending player slot, each player's commands in
//! the order they were issued. A replay is the `MatchStart` plus the bundle
//! log, so playback feeds the sim byte-for-byte what the live match did.
//! Wall-clock time is used here only to decide *when* a tick is released,
//! never *what* is in it.

pub mod client;
pub mod protocol;
pub mod relay;
pub mod replay;
pub mod session;
mod wire;

#[cfg(test)]
mod tests;

pub use client::{ClientConfig, NetSession};
pub use protocol::{
    ContentId, Hello, LobbyPlayer, LobbyState, MatchConfig, MatchStart, Message, PlayerCommands,
    PlayerSetup, RefuseReason, Role, TickBundle, Welcome, MAX_COMMANDS_BYTES, MAX_COMMAND_LEN,
    MAX_FRAME_LEN, MAX_SNAPSHOT_LEN, PROTOCOL_VERSION,
};
pub use relay::{RelayConfig, RelayHandle, RelayServer, RelaySummary};
pub use replay::{
    Replay, ReplayReader, ReplayRecord, ReplayWriter, REPLAY_EXTENSION, REPLAY_FORMAT_VERSION,
};
pub use session::{
    EndReason, LocalSession, Pacing, ReplaySession, Session, SessionEvent, DEFAULT_TICK_BUDGET,
};
pub use wire::NetError;
