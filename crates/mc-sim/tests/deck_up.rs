//! A unit on a lift ship's ramp leans with the ramp, not the ground (`Deck::up`).
use mc_sim::transport::Deck;

fn deck(open: f32) -> Deck {
    Deck { pos: [0.0, 0.0], dir: [1.0, 0.0], ground: 0.0, hinge: -16.0, lip: -84.0, front: 60.0, half_width: 20.0, floor: 34.0, open }
}

#[test]
fn a_unit_on_the_ramp_leans_with_it_and_stands_level_in_the_hold() {
    let d = deck(1.0);
    let mid = d.up([-50.0, 0.0]).expect("on the ramp");
    // Ramp rises toward +x (the hinge): up tilts back toward -x, by the ramp's slope.
    let slope = 34.0f32 / 68.0;
    let want = slope.atan();
    let tilt = mid[0].abs().atan2(mid[2]);
    assert!(mid[0] < 0.0 && (tilt - want).abs() < 0.01, "{mid:?}");
    assert_eq!(d.up([20.0, 0.0]), Some([0.0, 0.0, 1.0]), "level on the hold floor");
    assert!(d.up([-50.0, 25.0]).is_none() && d.up([-90.0, 0.0]).is_none(), "off the deck");
    // Easing in at the foot, not a snap.
    let foot = d.up([-83.0, 0.0]).unwrap();
    assert!(foot[0].abs() < mid[0].abs() * 0.2);
    // A half-open ramp is half as steep.
    let half = deck(0.5).up([-50.0, 0.0]).unwrap();
    assert!(half[0].abs() < mid[0].abs() && half[0] < 0.0);
    // Turned with the ship.
    let turned = Deck { dir: [0.0, 1.0], ..d }.up([0.0, -50.0]).unwrap();
    assert!((turned[1] - mid[0]).abs() < 1e-5 && turned[0].abs() < 1e-5);
}
