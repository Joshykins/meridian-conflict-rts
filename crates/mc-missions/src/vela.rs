//! The Contract: a complete route from the beach to the Calder spaceport.
use crate::script::Script;
#[path = "vela_propaganda.rs"]
mod propaganda;
use mc_map::placed::Placed;
use mc_sim::mission::{Action as A, Condition as C, Definition, Objective as O};
use std::future::Future;
use std::task::{Context, Poll, Waker};

fn spawn(group: &str) -> A {
    A::Spawn(group.into())
}
fn objective(line: &str, status: O) -> A {
    A::Objective {
        line: line.into(),
        status,
    }
}
fn after(event: &str, ticks: u32) -> C {
    C::After {
        event: event.into(),
        ticks,
    }
}
fn inside(region: &str) -> C {
    C::Inside {
        unit: "merrick".into(),
        region: region.into(),
    }
}
fn dead(group: &str) -> C {
    C::DeadGroup(group.into())
}
fn go(group: &str, point: &str, attack: bool) -> A {
    A::Move {
        group: group.into(),
        point: point.into(),
        attack,
    }
}
fn say(line: &str) -> A {
    A::Say {
        line: line.into(),
        ticks: 50,
    }
}
fn camera(name: &str, ticks: u32) -> A {
    A::Camera {
        name: name.into(),
        ticks,
    }
}

pub fn definition(placed: Placed) -> Definition {
    let script = Script::default();
    // These futures only compile dependencies. No future survives into the sim
    // or a checkpoint; everything below becomes Definition.events.
    let mut task = Box::pin(contract(script.clone()));
    let mut context = Context::from_waker(Waker::noop());
    assert!(matches!(task.as_mut().poll(&mut context), Poll::Ready(())));
    Definition {
        key: "vela_m1".into(),
        placed,
        events: {
            propaganda::events(&script);
            script.finish()
        },
    }
}

async fn contract(s: Script) {
    cold_open(&s).await;
    coast_road(&s).await;
    miners(&s).await;
    betrayal(&s).await;
    arc_push(&s).await;
    spaceport(&s).await;
}

async fn cold_open(s: &Script) {
    s.event(
        "merrick_fallen",
        C::DeadUnit("merrick".into()),
        vec![
            say("m1.fail.04"),
            objective("m1.obj.06", O::Failed),
            A::Lose,
        ],
    );
    s.event(
        "merrick_wounded",
        C::HealthBelow {
            unit: "merrick".into(),
            percent: 50,
        },
        vec![say("m1.fail.01"), say("m1.fail.02")],
    );
    s.event(
        "merrick_critical",
        C::HealthBelow {
            unit: "merrick".into(),
            percent: 25,
        },
        vec![say("m1.fail.03")],
    );
    s.then(
        "landing",
        C::Elapsed(0),
        vec![
            A::Controls(false),
            spawn("landing_ship"),
            spawn("company"),
            A::Load {
                group: "company".into(),
                carrier: "bastion_landing".into(),
            },
            A::Arrive("bastion_landing".into()),
            spawn("company_bombers"),
            spawn("roadblock"),
            spawn("checkpoints"),
            spawn("bridge_guns"),
            spawn("camp_guards"),
            spawn("pdf"),
            spawn("square_ambush"),
            spawn("warehouse_attackers"),
            spawn("harbour_defence"),
            spawn("spaceport"),
            spawn("escape_ship"),
            A::Protect {
                unit: "mara_bastion".into(),
                enabled: true,
            },
            A::Land {
                unit: "mara_bastion".into(),
                point: "escape_pad".into(),
                unload: false,
            },
            objective("m1.obj.06", O::Active),
            camera("landing", 450),
            go("company_bombers", "beach", true),
        ],
    );
    s.event(
        "bastion_touchdown",
        after("landing", 40),
        vec![A::Land {
            unit: "bastion_landing".into(),
            point: "beach".into(),
            unload: true,
        }],
    );
    for line in [
        "m1.open.01",
        "m1.open.02",
        "m1.open.03",
        "m1.open.04",
        "m1.open.05",
        "m1.open.06",
        "m1.open.07",
    ] {
        s.say(line, 55).await;
    }
    s.wait("company_out", C::Unloaded("company".into())).await;
    s.wait("ramps_clear", after("company_out", 80)).await;
    s.then(
        "landed",
        C::Elapsed(0),
        vec![
            A::Controls(true),
            A::Checkpoint(1),
            objective("m1.obj.01", O::Active),
            A::TakeOff("bastion_landing".into()),
            go("landing_ship", "offshore", false),
            go("company_bombers", "offshore", false),
        ],
    );
    s.event(
        "company_air_departed",
        after("landed", 200),
        vec![
            A::Despawn("landing_ship".into()),
            A::Despawn("company_bombers".into()),
        ],
    );
}

async fn coast_road(s: &Script) {
    s.say("m1.road.01", 45).await;
    s.say("m1.road.01h", 40).await;
    s.say("m1.road.02", 40).await;
    s.wait("first_contact", inside("first_contact")).await;
    s.say("m1.road.03", 40).await;
    s.say("m1.road.03h", 40).await;
    s.wait("roadblock_clear", dead("roadblock")).await;
    s.say("m1.road.04", 25).await;
    s.event(
        "radio_warning",
        after("roadblock_clear", 200),
        vec![say("m1.road.04a"), say("m1.road.04b"), say("m1.road.04c")],
    );
    s.wait("pdf_linkup", inside("junction")).await;
    s.then(
        "pdf_advances",
        C::Elapsed(0),
        vec![go("pdf", "bridge_west", true)],
    );
    s.say("m1.road.05", 55).await;
    s.say("m1.road.06", 50).await;
    s.say("m1.road.07", 30).await;
    s.say("m1.road.08", 65).await;
    s.say("m1.road.09", 65).await;
    s.say("m1.road.10", 45).await;
    s.say("m1.road.11", 25).await;
    s.say("m1.road.12", 55).await;
    s.say("m1.road.12h", 50).await;
    s.say("m1.road.13", 50).await;
    s.wait("ridge", inside("ridge")).await;
    s.say("m1.road.15", 55).await;
    s.say("m1.road.15h", 35).await;
    s.say("m1.road.16", 65).await;
    s.say("m1.road.16h", 40).await;
    s.say("m1.road.17", 50).await;
    s.wait("bridge_clear", dead("bridge_guns")).await;
    s.then(
        "bridge_checkpoint",
        C::Elapsed(0),
        vec![A::Checkpoint(2), go("pdf", "camp", true)],
    );
    s.say("m1.road.18", 50).await;
    s.say("m1.road.19", 25).await;
    s.say("m1.road.20", 45).await;
}

async fn miners(s: &Script) {
    s.wait("camp_found", inside("camp")).await;
    s.then(
        "free_miners",
        C::Elapsed(0),
        vec![objective("m1.obj.02", O::Active)],
    );
    s.say("m1.mine.01", 50).await;
    s.say("m1.mine.02", 55).await;
    s.wait("miners_freed", dead("camp_guards")).await;
    s.then(
        "miners_objective",
        C::Elapsed(0),
        vec![objective("m1.obj.02", O::Complete), spawn("miners")],
    );
    for line in [
        "m1.mine.03",
        "m1.mine.04",
        "m1.mine.05",
        "m1.mine.06",
        "m1.mine.07",
        "m1.mine.08",
    ] {
        s.say(line, 50).await;
    }
    s.then(
        "reach_square",
        C::Elapsed(0),
        vec![
            objective("m1.obj.01", O::Complete),
            objective("m1.obj.03", O::Active),
            go("miners", "beach", false),
            go("pdf", "square", false),
        ],
    );
    s.say("m1.town.01", 55).await;
    s.say("m1.town.02", 55).await;
}

async fn betrayal(s: &Script) {
    s.wait("square_entered", inside("square")).await;
    s.then(
        "betrayal",
        C::Elapsed(0),
        vec![
            A::Checkpoint(3),
            A::Team { player: 2, team: 1 },
            objective("m1.obj.03", O::Complete),
            objective("m1.obj.04", O::Active),
            go("pdf", "square", true),
            go("square_ambush", "square", true),
            camera("square", 50),
        ],
    );
    for line in [
        "m1.turn.01",
        "m1.turn.02",
        "m1.turn.03",
        "m1.turn.04",
        "m1.turn.05",
        "m1.turn.06",
        "m1.turn.07",
        "m1.turn.08",
        "m1.turn.09",
        "m1.turn.10",
        "m1.turn.10h",
    ] {
        s.say(line, 40).await;
    }
    holdouts(s);
}

async fn arc_push(s: &Script) {
    arrivals(s);
    s.wait("breakout", inside("breakout")).await;
    s.then(
        "arc_arrival",
        C::Elapsed(0),
        vec![
            A::Checkpoint(4),
            objective("m1.obj.04", O::Complete),
            spawn("arc_air"),
            spawn("arc_west"),
            spawn("arc_warehouses"),
            spawn("arc_west_ship"),
            spawn("arc_warehouse_ship"),
            A::Load {
                group: "arc_west".into(),
                carrier: "arc_west_transport".into(),
            },
            A::Load {
                group: "arc_warehouses".into(),
                carrier: "arc_warehouse_transport".into(),
            },
            A::Land {
                unit: "arc_west_transport".into(),
                point: "arc_west_drop".into(),
                unload: true,
            },
            A::Land {
                unit: "arc_warehouse_transport".into(),
                point: "arc_warehouse_drop".into(),
                unload: true,
            },
            go("arc_air", "square", true),
            camera("arc_arrival", 70),
        ],
    );
    for line in [
        "m1.arc.01",
        "m1.arc.02",
        "m1.arc.03",
        "m1.arc.04",
        "m1.arc.05",
        "m1.arc.06",
        "m1.arc.07",
        "m1.arc.08",
        "m1.arc.09",
        "m1.arc.10",
    ] {
        s.say(line, 45).await;
    }
    s.then(
        "harbour_push",
        C::Elapsed(0),
        vec![
            objective("m1.obj.09", O::Active),
            go("arc_air", "harbour", true),
            go("arc_west", "harbour", true),
            go("arc_warehouses", "harbour", true),
        ],
    );
    for line in [
        "m1.push.01",
        "m1.push.02",
        "m1.push.03",
        "m1.push.04",
        "m1.push.05",
        "m1.push.06",
        "m1.push.07",
    ] {
        s.say(line, 45).await;
    }
    s.wait("flank", inside("harbour")).await;
    s.then(
        "flank_attack",
        C::Elapsed(0),
        vec![spawn("pdf_column"), go("pdf_column", "harbour", true)],
    );
    for line in [
        "m1.push.08",
        "m1.push.09",
        "m1.arc.11",
        "m1.arc.12",
        "m1.arc.13",
    ] {
        s.say(line, 45).await;
    }
    s.then(
        "flank_strike",
        C::Elapsed(0),
        vec![go("arc_air", "flank", true)],
    );
    for line in [
        "m1.push.10",
        "m1.push.11",
        "m1.push.12",
        "m1.push.13",
        "m1.push.14",
        "m1.push.15",
        "m1.push.16",
        "m1.push.17",
    ] {
        s.say(line, 40).await;
    }
}

async fn spaceport(s: &Script) {
    s.wait("spaceport_reached", inside("spaceport")).await;
    s.then(
        "final_assault",
        C::Elapsed(0),
        vec![
            A::Checkpoint(5),
            objective("m1.obj.09", O::Complete),
            objective("m1.obj.05", O::Active),
            go("arc_air", "spaceport", true),
            go("arc_west", "spaceport", true),
            go("arc_warehouses", "spaceport", true),
        ],
    );
    s.say("m1.fin.01", 55).await;
    s.say("m1.fin.02", 55).await;
    s.say("m1.fin.03", 40).await;
    s.say("m1.fin.04", 50).await;
    s.then(
        "mara_escapes",
        C::Elapsed(0),
        vec![
            A::TakeOff("mara_bastion".into()),
            go("escape_ship", "escape", false),
        ],
    );
    s.say("m1.fin.05", 50).await;
    s.wait("base_destroyed", dead("spaceport")).await;
    s.then(
        "base_complete",
        C::Elapsed(0),
        vec![
            objective("m1.obj.05", O::Complete),
            objective("m1.obj.06", O::Complete),
            A::Checkpoint(6),
            A::Controls(false),
            camera("dig_site", 600),
        ],
    );
    for line in [
        "m1.fin.06",
        "m1.fin.07",
        "m1.fin.08",
        "m1.fin.09",
        "m1.fin.10",
        "m1.fin.11",
        "m1.fin.12",
        "m1.fin.13",
        "m1.fin.14",
        "m1.fin.15",
        "m1.fin.16",
    ] {
        s.say(line, 50).await;
    }
    s.then(
        "contract_complete",
        C::Elapsed(0),
        vec![A::Controls(true), A::Win],
    );
}

fn holdouts(s: &Script) {
    s.event(
        "holdouts_refuse",
        after("betrayal", 200),
        vec![
            spawn("holdouts"),
            say("m1.hold.01"),
            say("m1.hold.02"),
            say("m1.hold.03"),
            objective("m1.obj.07", O::Active),
            go("warehouse_attackers", "warehouse", true),
        ],
    );
    s.event(
        "holdouts_rescued",
        C::All(vec![
            after("holdouts_refuse", 0),
            C::AliveGroup("holdouts".into()),
            C::Objective {
                line: "m1.obj.07".into(),
                status: O::Active,
            },
            dead("warehouse_attackers"),
            inside("warehouse"),
        ]),
        vec![
            objective("m1.obj.07", O::Complete),
            say("m1.hold.04"),
            say("m1.hold.05"),
            go("holdouts", "harbour", true),
        ],
    );
    s.event(
        "holdouts_lost",
        C::All(vec![
            after("holdouts_refuse", 0),
            dead("holdouts"),
            C::Objective {
                line: "m1.obj.07".into(),
                status: O::Active,
            },
        ]),
        vec![
            objective("m1.obj.07", O::Failed),
            say("m1.hold.06"),
            say("m1.hold.07"),
        ],
    );
    s.event("boxed_in", after("betrayal", 300), vec![say("m1.turn.11")]);
}

// Ground allies arrive as actual cargo. Their orders begin once the ramps clear.
fn arrivals(s: &Script) {
    for (group, ship, carrier, destination) in [
        ("arc_west", "arc_west_ship", "arc_west_transport", "harbour"),
        (
            "arc_warehouses",
            "arc_warehouse_ship",
            "arc_warehouse_transport",
            "warehouse",
        ),
    ] {
        let out = format!("{group}_unloaded");
        let clear = format!("{group}_ramps_clear");
        let departed = format!("{group}_departed");
        s.event(
            &out,
            C::All(vec![after("arc_arrival", 0), C::Unloaded(group.into())]),
            vec![],
        );
        s.event(
            &clear,
            after(&out, 80),
            vec![
                go(group, destination, true),
                A::TakeOff(carrier.into()),
                go(ship, "offshore", false),
            ],
        );
        s.event(&departed, after(&clear, 400), vec![A::Despawn(ship.into())]);
    }
}
