use super::*;
#[test]
fn destroyed_named_prop_fires_once_and_survives_checkpoint_resume() {
    let prop = mc_map::Prop {
        kind: mc_map::PropKind::CityBillboard,
        pos: FxVec2::from_ints(450, 450),
        heading: Angle::ZERO,
        scale_milli: 1000,
        wear_milli: 0,
    };
    let doodad = mc_map::placed::Doodad {
        id: 5,
        name: "sign".into(),
        prop_index: 0,
        tint: [255; 3],
        blocking: true,
        health: 120,
        reclaim: 0,
        footprint: Some(mc_map::placed::Footprint {
            half: FxVec2::from_ints(8, 2),
            height: Fx::from_int(15),
        }),
    };
    let make = || world_props(0, vec![prop], vec![doodad.clone()]);
    let mut w = make();
    let mut d = definition();
    d.placed.next_id = 6;
    d.placed.doodads.push(doodad.clone());
    d.events.push(Event {
        name: "quip".into(),
        when: Condition::All(vec![
            Condition::PropDead("sign".into()),
            Condition::DialogueIdle,
        ]),
        actions: vec![Action::Say {
            line: "quiet".into(),
            ticks: 30,
        }],
    });
    let mut invalid = d.clone();
    invalid.events.last_mut().unwrap().when = Condition::PropDead("missing".into());
    assert!(w.begin_mission(invalid).is_err());
    w.begin_mission(d).unwrap();
    w.tick(&[]).unwrap();
    assert!(!w.state.mission.as_ref().unwrap().fired.contains_key("quip"));
    w.damage_structure(0, Fx::from_int(1000), false);
    for _ in 0..35 {
        w.tick(&[]).unwrap();
    }
    assert!(w.state.mission.as_ref().unwrap().fired.contains_key("quip"));
    let mut resumed = make();
    resumed.restore(terrain(), &w.snapshot()).unwrap();
    assert_eq!(w.hash(), resumed.hash());
    for _ in 0..40 {
        assert_eq!(w.tick(&[]).unwrap(), resumed.tick(&[]).unwrap());
    }
    assert_eq!(w.state.mission.as_ref().unwrap().fired["quip"], 30);
}
#[test]
fn prop_dead_keeps_existing_condition_wire_ids() {
    let bytes = bincode::serialize(&Condition::PropDead("sign".into())).unwrap();
    assert_eq!(&bytes[..4], &12u32.to_le_bytes());
    let decoded: Condition = crate::decode_untrusted(&bytes, 1024).unwrap();
    assert!(matches!(decoded,Condition::PropDead(name) if name=="sign"));
    assert_eq!(
        &bincode::serialize(&Condition::DialogueIdle).unwrap()[..4],
        &9u32.to_le_bytes()
    );
}
