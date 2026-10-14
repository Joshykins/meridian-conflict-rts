use super::tests::{terrain, world_salvage};
use crate::{city, Command, PlayerCommand, World};
use mc_core::{Angle, Fx, FxVec2};

fn setup(w: &mut World, key: &str, x: i32) -> usize {
    let bp = w.blueprints.id_of(key).unwrap();
    w.spawn_unit(bp, 0, FxVec2::from_ints(x, 512), Angle::ZERO, true)
        .unwrap();
    let u = &w.state.units;
    u.slots.iter().find(|&r| u.blueprint[r] == bp).unwrap()
}

#[test]
fn partial_salvage_respects_storage_and_restores_exactly() {
    let mut w = world_salvage(true, 120, 100);
    let row = setup(&mut w, "aster_t1_engineer", 460);
    w.state.players[0].materials = Fx::ZERO;
    w.state.players[0].materials_capacity = Fx::from_int(7);
    assert!(!w.drain_prop(row, 0, Fx::from_int(100_000), 0));
    assert_eq!(w.state.city.salvage[0], Fx::from_int(93));
    assert_eq!(w.state.players[0].materials, Fx::from_int(7));
    let hash = w.hash();
    assert!(!w.drain_prop(row, 0, Fx::from_int(100_000), 0));
    assert_eq!(w.hash(), hash, "full stores do not consume salvage");
    let bytes = w.snapshot();
    let mut restored = world_salvage(true, 120, 100);
    restored.restore(terrain(), &bytes).unwrap();
    assert_eq!(restored.hash(), hash);
    for world in [&mut w, &mut restored] {
        world.state.players[0].materials_capacity = Fx::from_int(1000);
        assert!(world.drain_prop(row, 0, Fx::from_int(100_000), 0));
        assert_eq!(world.state.players[0].materials, Fx::from_int(100));
        assert_eq!(
            world.state.players[0].reclaimed_materials,
            Fx::from_int(100)
        );
        assert!(!world.is_prop_alive(0));
        assert_ne!(world.state.city.flags[0] & city::RECLAIMED, 0);
        let mut frame = crate::RenderFrame::default();
        world.write_render_frame(None, &mut frame);
        assert_ne!(
            frame.city[0].flags & city::RECLAIMED,
            0,
            "the renderer must hide the collected rubble too"
        );

        assert!(world
            .nav
            .passable(mc_data::MoveLayer::Land, 0, FxVec2::from_ints(512, 512)));
        world.state.validate().unwrap();
    }
    assert_eq!(w.hash(), restored.hash());
}

#[test]
fn area_order_collects_intact_and_collapsed_props_with_builders_and_heads() {
    for (key, health, destroy) in [
        ("aster_t1_engineer", 120, false),
        ("aster_t1_engineer", 120, true),
        ("aster_t2_reclaimer", 0, false),
    ] {
        let mut w = world_salvage(true, health, 30);
        let row = setup(&mut w, key, 430);
        setup(&mut w, "aster_materials_storage", 250);
        w.state.players[0].materials = Fx::ZERO;
        if destroy {
            w.damage_structure(0, Fx::from_int(120), false);
        }
        let id = w.state.units.id(row);
        w.tick(&[PlayerCommand {
            player: 0,
            command: Command::ReclaimArea {
                units: vec![id],
                pos: FxVec2::from_ints(512, 512),
                radius: Fx::from_int(100),
                queue: false,
            },
        }])
        .unwrap();
        let mut beam = false;
        for _ in 0..1000 {
            w.tick(&[]).unwrap();
            beam |= !w.reclaims.is_empty();
            if w.state.city.salvage[0] == Fx::ZERO {
                break;
            }
        }
        assert_eq!(w.state.city.salvage[0], Fx::ZERO, "{key}");
        assert!(beam, "{key} emits reclaim beams");
        assert_eq!(
            w.state.players[0].reclaimed_materials,
            Fx::from_int(30),
            "{key}"
        );
        assert_ne!(w.state.city.flags[0] & city::RECLAIMED, 0);
    }
}

#[test]
fn corrupt_or_inflated_salvage_is_rejected() {
    let mut w = world_salvage(false, 0, 20);
    assert_eq!(w.state.city.salvage, vec![Fx::from_int(20)]);
    w.state.city.salvage[0] = Fx::from_int(21);
    let bytes = w.snapshot();
    assert!(world_salvage(false, 0, 20)
        .restore(terrain(), &bytes)
        .is_err());
    w.state.city.salvage[0] = -Fx::ONE;
    assert!(w.state.validate().is_err());
}
