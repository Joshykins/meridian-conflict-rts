//! The clock keeps to the pace of the slowest player's sim (relay.rs, *Pace*): a player
//! who hears every bundle but has not stepped them holds the match, unless it is frozen.

use std::thread;
use std::time::{Duration, Instant};

use super::*;

/// Pumps only `quick` for `for_`; `slow` hears the bundles (its reader thread answers
/// them) but its game never steps them.
fn pump_one(quick: &mut NetClient, for_: Duration) {
    let until = Instant::now() + for_;
    while Instant::now() < until {
        quick.pump();
        thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn a_sim_that_falls_behind_holds_the_clock_until_it_catches_up() {
    let relay = relay(2, |_| {});
    let mut clients = start_match(relay.local_addr(), 2);
    pump_until(&mut refs(&mut clients), "running", |cs| {
        cs.iter().all(|c| c.ticks() >= 20)
    });
    let [quick, slow] = &mut clients[..] else {
        unreachable!()
    };
    let slow_at = slow.ticks();
    // At 1 ms a tick an unheld clock would close hundreds in this time.
    pump_one(quick, Duration::from_millis(400));
    let lead = RelayConfig::default().input_delay as usize + mc_core::TICKS_PER_SECOND as usize;
    // The bundles in flight when `slow` stopped, then no more than the lead past them.
    assert!(
        quick.ticks() <= slow_at + 2 * lead,
        "the clock ran {} ticks past a sim stopped at {slow_at}",
        quick.ticks() - slow_at
    );

    let at = quick.ticks();
    pump_until(&mut [quick, slow], "both go on", |cs| {
        cs.iter().all(|c| c.ticks() >= at + 100)
    });
    assert_same_history(&clients[0].sim, &clients[1].sim, at + 100);
    relay.shutdown().unwrap();
}

#[test]
fn a_frozen_sim_does_not_hold_the_clock_past_the_stall() {
    let relay = relay(2, |c| c.sim_stall = Duration::from_millis(100));
    let mut clients = start_match(relay.local_addr(), 2);
    pump_until(&mut refs(&mut clients), "running", |cs| {
        cs.iter().all(|c| c.ticks() >= 20)
    });
    let [quick, slow] = &mut clients[..] else {
        unreachable!()
    };
    let slow_at = slow.ticks();
    pump_until(&mut [quick], "the match goes on without it", |cs| {
        cs[0].ticks() >= slow_at + 500
    });
    // Its reader heard every bundle: it catches up when it steps again.
    let at = clients[0].ticks();
    pump_until(&mut refs(&mut clients), "caught up", |cs| {
        cs[1].ticks() >= at
    });
    relay.shutdown().unwrap();
}
