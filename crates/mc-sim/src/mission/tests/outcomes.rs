use super::*;
#[test]
fn optional_rescue_and_loss_are_exclusive_even_after_checkpoint_restore() {
    for lost_first in [false, true] {
        let mut w = world(0);
        let mut d = definition();
        d.events[0].actions.push(Action::Objective {
            line: "rescue".into(),
            status: Objective::Active,
        });
        d.events.extend([
            Event {
                name: "rescued".into(),
                when: Condition::All(vec![
                    Condition::Objective {
                        line: "rescue".into(),
                        status: Objective::Active,
                    },
                    Condition::AliveGroup("holdouts".into()),
                    Condition::Inside {
                        unit: "hero".into(),
                        region: "square".into(),
                    },
                ]),
                actions: vec![Action::Objective {
                    line: "rescue".into(),
                    status: Objective::Complete,
                }],
            },
            Event {
                name: "lost".into(),
                when: Condition::All(vec![
                    Condition::Objective {
                        line: "rescue".into(),
                        status: Objective::Active,
                    },
                    Condition::DeadGroup("holdouts".into()),
                ]),
                actions: vec![Action::Objective {
                    line: "rescue".into(),
                    status: Objective::Failed,
                }],
            },
        ]);
        w.begin_mission(d).unwrap();
        if lost_first {
            remove_holdout(&mut w);
        } else {
            arrive(&mut w);
        }
        w.tick(&[]).unwrap();
        let snapshot = w.snapshot();
        let mut resumed = world(0);
        resumed.restore(terrain(), &snapshot).unwrap();
        for world in [&mut w, &mut resumed] {
            if lost_first {
                arrive(world);
            } else {
                remove_holdout(world);
            }
            world.tick(&[]).unwrap();
            let m = world.state.mission.as_ref().unwrap();
            assert_eq!(
                m.objectives["rescue"],
                if lost_first {
                    Objective::Failed
                } else {
                    Objective::Complete
                }
            );
            assert_eq!(m.fired.contains_key("lost"), lost_first);
            assert_eq!(m.fired.contains_key("rescued"), !lost_first);
        }
        assert_eq!(w.hash(), resumed.hash());
    }
}
fn remove_holdout(w: &mut World) {
    let id = w.state.mission.as_ref().unwrap().units["holdout"];
    w.remove_unit_row(w.state.units.row(id).unwrap(), false)
        .unwrap();
}
