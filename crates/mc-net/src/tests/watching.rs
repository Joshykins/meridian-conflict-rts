//! A player whose side is out watches the rest of the match: the relay takes no
//! more orders or clock changes from it, and it talks with the observers in a
//! channel the players still fighting never hear.

use super::*;

/// The sequence number a test client writes into each command it sends.
fn seq(command: &[u8]) -> u32 {
    u32::from_le_bytes(command[1..5].try_into().unwrap())
}

#[test]
fn a_player_out_of_the_match_watches_and_talks_with_the_observers() {
    let relay = relay(3, |_| {});
    let addr = relay.local_addr();
    let mut clients = start_match(addr, 3);
    // Joining once the clock runs, the observer comes in from a snapshot.
    pump_until(&mut refs(&mut clients), "30 ticks", |cs| {
        cs.iter().all(|c| c.ticks() >= 30)
    });
    let observer = connect_with(addr, "watcher", |c| c.role = Role::Observer);
    clients.push(observer);
    pump_until(&mut refs(&mut clients), "the observer is in", |cs| {
        cs[3].loaded_at.is_some()
    });

    // Player 2 is out: what it says from here on goes to the watchers alone.
    clients[2].session.watching();
    let out_from = clients[2].sent.len() as u32;
    clients[2].session.chat("gg, watching now", 0).unwrap();
    // Its own line back means the relay has it watching (the observer speaks on another
    // connection, which could otherwise be heard first).
    pump_until(&mut refs(&mut clients), "its own line back", |cs| {
        !cs[2].watchers_chat.is_empty()
    });
    clients[3].session.chat("welcome", 0b001).unwrap();
    clients[0].session.chat("push mid", 0).unwrap();
    // Everyone the checks below read, the watchers too: each line comes on its own
    // connection, and a loaded machine can deliver them in any order.
    pump_until(&mut refs(&mut clients), "chat", |cs| {
        cs[2].watchers_chat.len() == 2
            && cs[3].watchers_chat.len() == 2
            && cs[1..].iter().all(|c| c.chat.len() == 1)
    });
    let said = vec![
        (Some(PlayerId(2)), "gg, watching now".to_owned()),
        (None, "welcome".to_owned()),
    ];
    for watcher in &clients[2..] {
        let mut heard = watcher.watchers_chat.clone();
        heard.sort();
        let mut want = said.clone();
        want.sort();
        assert_eq!(heard, want);
        // The players still fighting are heard by everyone, as before.
        assert_eq!(watcher.chat, [(Some(PlayerId(0)), "push mid".to_owned())]);
    }
    for player in &clients[..2] {
        assert!(
            player.watchers_chat.is_empty(),
            "a player heard the watchers"
        );
    }

    // Its turns still close ticks, but carry no orders, and it cannot stop the clock.
    clients[2].session.set_paused(true);
    let seen = clients[0].ticks();
    pump_until(&mut refs(&mut clients), "200 more ticks", |cs| {
        cs[..3].iter().all(|c| c.ticks() >= seen + 200)
    });
    assert!(clients[0].clock.is_none_or(|(paused, ..)| !paused));
    assert!(clients[2].sent.len() as u32 > out_from + 10);
    let ordered: Vec<u32> = clients[0]
        .bundles
        .iter()
        .flat_map(|b| b.commands())
        .filter(|(slot, _)| *slot == PlayerId(2))
        .map(|(_, c)| seq(c))
        .collect();
    assert!(
        ordered.iter().all(|&n| n < out_from),
        "orders sent while watching were carried out"
    );
    assert!(clients.iter().all(|c| c.desync.is_none()));
    relay.shutdown().unwrap();
}
