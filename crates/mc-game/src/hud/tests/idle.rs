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
    rig.view.selection = vec![9, 7];
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
