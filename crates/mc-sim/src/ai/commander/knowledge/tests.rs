use super::*;

fn mass(m: i32) -> Fx {
    Fx::from_int(m)
}

fn holding(player: u8, factories: i32, army: i32, tick: u32) -> Holding {
    let mut h = Holding::empty(player);
    if factories > 0 {
        h.built[Built::Factory as usize] = mass(factories);
        h.count[Built::Factory as usize] = 1;
        h.built_at = tick;
    }
    if army > 0 {
        h.army[Domain::Land as usize] = mass(army);
        h.army_at = tick;
    }
    h
}

#[test]
fn a_zone_wholly_in_view_shows_what_is_there_now() {
    let old = vec![holding(1, 800, 500, 100)];
    // Looked at it all: the factory is gone, a smaller army stands.
    let now = vec![holding(1, 0, 200, 900)];
    let new = refresh(&old, &now, 1000, 900);
    assert_eq!(new.len(), 1);
    assert_eq!(new[0].built[Built::Factory as usize], Fx::ZERO);
    assert_eq!(new[0].army[Domain::Land as usize], mass(200));
    assert_eq!(new[0].built_at, 900);
}

#[test]
fn a_zone_out_of_view_keeps_what_was_known() {
    let old = vec![holding(1, 800, 500, 100)];
    let new = refresh(&old, &[], 0, 900);
    assert_eq!(new, old);
}

#[test]
fn half_in_view_keeps_half_of_what_was_not_seen_again() {
    let old = vec![holding(1, 800, 0, 100)];
    let new = refresh(&old, &[], 500, 900);
    assert_eq!(new[0].built[Built::Factory as usize], mass(400));
    // Mostly looked at: the knowledge is fresh, though less is there.
    assert_eq!(new[0].built_at, 900);
}

#[test]
fn something_on_radar_is_never_less_than_what_was_known() {
    let old = vec![holding(1, 800, 0, 100)];
    let now = vec![holding(1, 300, 0, 900)];
    let new = refresh(&old, &now, 0, 900);
    assert_eq!(new[0].built[Built::Factory as usize], mass(800));
}

#[test]
fn an_army_long_unseen_is_forgotten_but_structures_are_kept() {
    let old = vec![holding(1, 800, 500, 100)];
    let new = refresh(&old, &[], 0, 100 + ARMY_FORGET);
    assert_eq!(new[0].army[Domain::Land as usize], Fx::ZERO);
    assert_eq!(new[0].built[Built::Factory as usize], mass(800));
}

#[test]
fn a_zone_looked_at_and_found_empty_forgets_its_holder() {
    let old = vec![holding(1, 800, 500, 100), holding(2, 0, 300, 100)];
    let now = vec![holding(2, 0, 300, 900)];
    let new = refresh(&old, &now, 1000, 900);
    assert_eq!(new.len(), 1);
    assert_eq!(new[0].player, 2);
}

#[test]
fn sureness_fades_slowly_for_structures_and_fast_for_armies() {
    let h = holding(1, 800, 500, 1000);
    assert_eq!(h.built_sure(1000 + BUILT_SURE), 100);
    assert_eq!(h.built_sure(1000 + BUILT_FADE * 2), BUILT_FLOOR);
    assert_eq!(h.army_sure(1000 + ARMY_SURE), 100);
    assert_eq!(h.army_sure(1000 + ARMY_FORGET), 0);
    assert!(h.army_now(1000 + 1000) < mass(500));
}
