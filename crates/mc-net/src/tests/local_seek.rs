use super::*;
fn start() -> MatchStart {
    MatchStart {
        content: CONTENT,
        seed: 99,
        input_delay: 0,
        players: vec![PlayerSetup {
            slot: PlayerId(0),
            name: "test".into(),
            data: vec![],
        }],
        options: vec![],
    }
}
#[test]
fn local_forward_seek_runs_every_bundle_and_preserves_pause_and_commands() {
    let mut fast = LocalSession::new(start(), PlayerId(0), Pacing::RealTime).unwrap();
    fast.set_tick_budget(7);
    fast.set_paused(true);
    fast.submit(vec![vec![1, 2, 3]]).unwrap();
    assert!(fast.seek(0, 103));
    let mut accelerated = FakeSim::default();
    let mut ticks = vec![];
    for _ in 0..30 {
        for event in fast.poll() {
            if let SessionEvent::TickReady(bundle) = event {
                ticks.push(bundle.tick);
                accelerated.step(&bundle);
            }
        }
    }
    assert_eq!(ticks, (0..103).collect::<Vec<_>>());
    assert!(!fast.seek(103, 50));
    assert!(!fast.seek(100, 200));
    assert!(
        fast.poll().is_empty(),
        "the prior paused state must survive the jump"
    );
    let mut ordinary = LocalSession::new(start(), PlayerId(0), Pacing::PerPoll(1)).unwrap();
    ordinary.submit(vec![vec![1, 2, 3]]).unwrap();
    let mut reference = FakeSim::default();
    for _ in 0..103 {
        for event in ordinary.poll() {
            if let SessionEvent::TickReady(bundle) = event {
                reference.step(&bundle);
            }
        }
    }
    assert_eq!(accelerated.history, reference.history);
    fast.set_paused(false);
    assert!(fast
        .poll()
        .iter()
        .any(|e| matches!(e,SessionEvent::TickReady(bundle) if bundle.tick==103)));
}
