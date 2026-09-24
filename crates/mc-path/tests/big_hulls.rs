//! The capital-ship size class: a size 5 hull needs a clear square of six cells (48 m).

use mc_core::{Fx, FxVec2};
use mc_path::terrain::*;
use mc_path::*;
use std::sync::Arc;

/// Two deep basins joined by a channel `width` cells wide, land all round.
fn strait(width: i32) -> impl FnMut(i32, i32) -> u8 {
    move |x, y| {
        let basin = !(60..140).contains(&x) && (40..200).contains(&y) && (20..236).contains(&x);
        let channel = (60..140).contains(&x) && (120..120 + width).contains(&y);
        if basin || channel {
            DEEP
        } else {
            LAND
        }
    }
}

/// Steps a hull from `start` along the field to `goal`. True if it arrives.
fn sails(width: i32, size: SizeClass) -> bool {
    let mut nav = Nav::new(
        NavGrid::from_fn(256, 256, strait(width)).unwrap(),
        NavConfig::default(),
        Arc::new(InlineSpawner),
    );
    let (start, goal) = (Cell::new(30, 120).center(), Cell::new(200, 120).center());
    let mut tick = 0;
    nav.begin_tick(tick);
    let id = nav
        .request(MoveLayer::Naval, size, goal, &[start])
        .unwrap();
    let mut pos: FxVec2 = start;
    for _ in 0..4000 {
        tick += 1;
        nav.begin_tick(tick);
        match nav.sample(id, pos) {
            Sample::Direction(d) => {
                pos += d * Fx::from_int(5);
                assert!(nav.is_passable(MoveLayer::Naval, size, Cell::from_pos(pos)));
            }
            Sample::NeedsExtend => nav.extend(id, pos).unwrap(),
            Sample::Pending => {}
            Sample::Arrived => return true,
            Sample::Unreachable | Sample::Failed(_) => return false,
        }
    }
    panic!("still sailing at {:?}", Cell::from_pos(pos));
}

#[test]
fn a_size_five_hull_needs_a_48_m_square() {
    let big = SizeClass::new(5).unwrap();
    assert_eq!(big.cells(), 6);
    assert!(SizeClass::new(6).is_err());
    for (width, fits) in [(4, false), (5, false), (6, true), (7, true)] {
        let grid = NavGrid::from_fn(256, 256, strait(width)).unwrap();
        let open = (120..120 + width).any(|y| grid.is_passable(MoveLayer::Naval, big, Cell::new(100, y)));
        assert_eq!(open, fits, "a {width}-cell channel");
    }
}

#[test]
fn a_size_five_hull_refuses_a_32_m_channel_and_takes_a_56_m_one() {
    let big = SizeClass::new(5).unwrap();
    assert!(!sails(4, big), "through 32 m of water");
    assert!(sails(7, big), "not through 56 m of water");
    // A frigate-sized hull takes the narrow one.
    assert!(sails(4, SizeClass::LARGE));
}
