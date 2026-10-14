use super::*;
use mc_core::{Angle, Fx, FxVec2};
use mc_map::{
    placed::{Doodad, Footprint},
    PropKind,
};
#[test]
fn custom_prop_render_shape_matches_scaled_rotated_hit_shape() {
    let p = Prop {
        kind: PropKind::CityBillboard,
        pos: FxVec2::from_ints(100, 200),
        heading: Angle::QUARTER_TURN,
        scale_milli: 1500,
        wear_milli: 0,
    };
    let mut d = Doodad {
        id: 1,
        name: "sign".into(),
        prop_index: 0,
        tint: [255; 3],
        blocking: true,
        health: 400,
        reclaim: 20,
        footprint: Some(Footprint {
            half: FxVec2::from_ints(8, 2),
            height: Fx::from_int(15),
        }),
    };
    assert!(Shape::of(&p, None).is_none());
    let shape = Shape::of(&p, Some(&d)).unwrap();
    assert_eq!(shape.centre, Vec2::new(100.0, 200.0));
    assert_eq!(shape.half, Vec2::new(12.0, 3.0));
    assert!((shape.along - Vec2::Y).length() < 0.001);
    assert_eq!(shape.top, 22.5);
    let (heaps, list) = heaps_for(&[p], |_| Some(&d), 10, 25, |_| 4.0);
    assert!(!heaps.is_empty());
    assert_eq!(list[0], (10, 0));
    assert!(heaps.iter().all(|h| h.pos[2] == 4.0));
    d.health = 0;
    d.blocking = false;
    assert!(Shape::of(&p, Some(&d)).is_none());
}
