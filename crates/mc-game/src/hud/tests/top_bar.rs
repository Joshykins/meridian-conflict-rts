//! The top bar in a network match: the link readout, then the speed and pause that
//! every player may use (the relay tells everyone who did).

use super::*;

#[test]
fn a_network_player_changes_the_speed_and_an_observer_cannot() {
    let mut rig = Rig::new("aster_t1_tank");
    rig.view.status.owns_clock = false;
    rig.net = Some(crate::netplay::NetLink::default());
    rig.settle();
    // The bar grows left by the link readout: the speed and pause keep their places.
    assert_eq!(rig.click(speed_faster()), vec![HudAction::SetSpeed(150)]);
    assert_eq!(rig.click(pause_button()), vec![HudAction::Pause]);
    rig.view.observing = true;
    rig.settle();
    assert_eq!(rig.click(speed_faster()), vec![]);
    assert_eq!(rig.click(pause_button()), vec![]);
}
