//! The idle cards under the commander card: engineers, factories, reclaimers.

use super::*;

#[test]
fn an_idle_engineer_tile_steps_through_them_and_shift_takes_them_all() {
    let mut rig = Rig::new("aster_t1_engineer");
    rig.view.selection.clear();
    rig.view.frame.units[0].owner_flags |= STATE_IDLE;
    let mut second = rig.view.frame.units[0];
    second.unit_id = 9;
    rig.view.frame.units.push(second);
    rig.view.index_of.insert(9, 1);
    // No commander: the card sits right under the economy, its first tile right
    // of the title block.
    let card_y = EDGE + ECONOMY_H + GAP;
    let tile = Vec2::new(EDGE + 8.0 + 66.0 + 8.0 + 18.0, card_y + 8.0 + 18.0);
    let one = |id| {
        vec![HudAction::Select {
            units: vec![id],
            focus: true,
        }]
    };
    assert_eq!(rig.click(tile), one(7));
    assert_eq!(rig.click(tile), one(9));
    assert_eq!(rig.click(tile), one(7), "and round again");
    rig.view.shift = true;
    assert_eq!(
        rig.click(tile),
        vec![HudAction::Select {
            units: vec![7, 9],
            focus: false
        }]
    );
    rig.view.selection = vec![9, 7].into();
    assert_eq!(
        rig.click(tile),
        vec![HudAction::Select {
            units: vec![7, 9],
            focus: true
        }],
        "a second shift-click finds them"
    );
    rig.view.shift = false;
}

/// A salvage unit's tile, the first on the card right under the economy.
fn reclaimer_rig() -> (Rig, Vec2) {
    let mut rig = Rig::new("aster_t1_land_reclaimer");
    rig.view.selection.clear();
    rig.view.frame.units[0].owner_flags |= STATE_IDLE;
    let card_y = EDGE + ECONOMY_H + GAP;
    let tile = Vec2::new(EDGE + 8.0 + 66.0 + 8.0 + 18.0, card_y + 8.0 + 18.0);
    (rig, tile)
}

#[test]
fn an_idle_reclaimer_has_a_card_and_its_tile_selects_it() {
    let (mut rig, tile) = reclaimer_rig();
    assert_eq!(
        rig.click(tile),
        vec![HudAction::Select {
            units: vec![7],
            focus: true,
        }]
    );
}

#[test]
fn a_reclaimer_working_a_wreck_on_its_own_is_not_idle() {
    for f in [flag::RECLAIMING, flag::WORKING] {
        let (mut rig, tile) = reclaimer_rig();
        rig.view.frame.units[0].owner_flags |= (f as u32) << 8;
        assert!(rig.click(tile).is_empty(), "no card while it works");
    }
}

#[test]
fn a_reclaimer_with_a_wreck_in_reach_is_not_idle() {
    for (off, idle) in [(20.0, false), (5000.0, true)] {
        let (mut rig, tile) = reclaimer_rig();
        let mut wreck = rig.view.frame.units[0];
        wreck.unit_id = 11;
        wreck.owner_flags = mc_sim::mirror::KIND_WRECK;
        wreck.pos[0] += off;
        wreck.radius = 2.0;
        rig.view.frame.units.push(wreck);
        assert_eq!(!rig.click(tile).is_empty(), idle, "a wreck {off} m off");
    }
}

/// Puts unit 7 in the hold of a Courier, unit 9, which joins the frame.
fn stow(rig: &mut Rig) {
    let ship = rig.blueprints.id_of("aster_t2_lift_ship").expect("courier");
    let mut carrier = rig.view.frame.units[0];
    carrier.unit_id = 9;
    carrier.blueprint = ship.0 as u32;
    carrier.owner_flags = 0;
    rig.view.frame.units.push(carrier);
    rig.view.index_of.insert(9, 1);
    let u = &mut rig.view.frame.units[0];
    u.owner_flags |= STATE_IDLE | (flag::IN_FACTORY as u32) << 8;
    u.status[0] |= mc_sim::mirror::UNIT_STORED;
    u.status[2] = 9;
}

#[test]
fn an_engineer_riding_in_a_hold_is_not_idle() {
    let mut rig = Rig::new("aster_t1_engineer");
    rig.view.selection.clear();
    stow(&mut rig);
    let card_y = EDGE + ECONOMY_H + GAP;
    let tile = Vec2::new(EDGE + 8.0 + 66.0 + 8.0 + 18.0, card_y + 8.0 + 18.0);
    assert!(rig.click(tile).is_empty(), "no Engineers card");
}

#[test]
fn the_commander_card_of_a_commander_in_a_hold_selects_the_ship() {
    let mut rig = Rig::new("aster_commander");
    rig.view.selection.clear();
    stow(&mut rig);
    let card = Vec2::new(
        EDGE + COMMANDER_W * 0.5,
        EDGE + ECONOMY_H + GAP + COMMANDER_H * 0.5,
    );
    assert_eq!(
        rig.click(card),
        vec![HudAction::Select {
            units: vec![9],
            focus: true
        }]
    );
}
