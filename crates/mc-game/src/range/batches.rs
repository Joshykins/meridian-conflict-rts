//! The range's scenarios that stage a whole batch of units: a space elevator's first
//! platform (`Scenario::Elevator`), a gravitic engine's target camp (`Scenario::Fracture`)
//! and the camp an Edict is fired on (`Scenario::Edict`).

use mc_core::{Angle, Fx, FxVec2};
use mc_data::{Blueprints, UnitBlueprint};
use mc_sim::Command;

use super::{PendingOrder, Staged, BLUE, DEFAULT_SUBJECT, RED};

/// `Scenario::Elevator`: from what the elevator brings down, a line of its heaviest tanks,
/// a flight of gunships (or whatever armed aircraft it has), and its dearest land units,
/// enough to fill most of one platform, rallying east down the range.
pub(super) fn elevator_batch(
    blueprints: &Blueprints,
    bp: &UnitBlueprint,
    pad: FxVec2,
) -> Result<Staged, &'static str> {
    let builds = bp
        .builder
        .as_ref()
        .filter(|_| bp.elevator.is_some())
        .ok_or("This Is Not a Space Elevator")?
        .builds
        .iter()
        .map(|&id| blueprints.unit(id));
    let layer = |b: &UnitBlueprint| b.motion.map(|m| m.layer);
    let tank = builds
        .clone()
        .filter(|b| b.visual.icon == mc_data::IconKind::Tank && b.tech < 4)
        .max_by_key(|b| b.tech)
        .ok_or("It Brings Down No Tanks")?;
    let air = builds
        .clone()
        .filter(|b| layer(b) == Some(mc_data::MoveLayer::Air) && !b.weapons.is_empty())
        .min_by_key(|b| {
            (
                b.visual.icon != mc_data::IconKind::Gunship,
                std::cmp::Reverse(b.tech),
            )
        })
        .ok_or("It Brings Down No Aircraft")?;
    let heavy = builds
        .filter(|b| layer(b) == Some(mc_data::MoveLayer::Land) && !b.weapons.is_empty())
        .max_by_key(|b| b.cost_materials)
        .ok_or("It Brings Down No Land Units")?;
    // As many of the dearest as leave a little of the payload over, so some materials come
    // down too: at least one, at most four.
    let payload = bp.elevator.map_or(Fx::ZERO, |e| e.payload);
    let room = payload * 9 / 10 - tank.cost_materials * 6 - air.cost_materials * 4;
    let heavies = (room / heavy.cost_materials.max(Fx::ONE))
        .floor_int()
        .clamp(1, 4) as u8;
    let order = PendingOrder::ProduceMix {
        batch: [(tank.id, 6), (air.id, 4), (heavy.id, heavies)],
        rally: pad + FxVec2::from_ints(520, 0),
    };
    Ok((Vec::new(), Some((bp.id, order))))
}

/// How far down the range `Scenario::Fracture` aims, metres.
pub(super) const FRACTURE_RANGE: i32 = 2000;

/// `Scenario::Fracture`: a red camp round `mark` (tanks, a few power plants, aircraft over
/// them), and the engine turned on it at once.
pub(super) fn fracture_camp(
    blueprints: &Blueprints,
    bp: &UnitBlueprint,
    mark: FxVec2,
) -> Result<Staged, &'static str> {
    if bp.gravitic.is_none() {
        return Err("This Is Not a Gravitic Engine");
    }
    let id = |key: &str| blueprints.id_of(key).ok_or("No Camp To Strike");
    let spawn = |blueprint, x: i32, y: i32, count, heading| Command::DebugSpawn {
        owner: RED,
        blueprint,
        pos: mark + FxVec2::from_ints(x, y),
        heading,
        count,
        flags: 0,
        build: 1000,
    };
    let (tank, power, gunship) = (
        id(DEFAULT_SUBJECT)?,
        id("aster_t1_power")?,
        id("aster_t2_gunship")?,
    );
    let west = Angle::HALF_TURN;
    let mut commands = vec![
        // The engine's upkeep paid in full, so it never falters unless told to.
        Command::DebugFreeBuild {
            player: BLUE,
            on: true,
        },
        Command::DebugStock {
            player: BLUE,
            materials: None,
            energy: Some(1000),
        },
    ];
    for (x, y) in [
        (0, 0),
        (-60, -70),
        (-60, 70),
        (60, -40),
        (60, 40),
        (-140, 0),
        (140, 0),
    ] {
        commands.push(spawn(tank, x, y, 1, west));
    }
    for (x, y) in [(-110, -150), (90, 160), (200, -120)] {
        commands.push(spawn(power, x, y, 1, Angle::ZERO));
    }
    for (x, y) in [(-80, 120), (120, -150), (0, -200)] {
        commands.push(spawn(gunship, x, y, 1, west));
    }
    Ok((
        commands,
        Some((bp.id, PendingOrder::Fracture { pos: mark })),
    ))
}

/// How far down the range `Scenario::Edict` aims, metres: 3.5 km off a Regent's bow.
pub(super) const EDICT_RANGE: i32 = 5000;

/// `Scenario::Edict`: a red camp round `mark` (tanks in rings out to past the blast's
/// reach, a dome and a few power plants), and the ship's Edict fired on it at once.
pub(super) fn edict_camp(
    blueprints: &Blueprints,
    bp: &UnitBlueprint,
    mark: FxVec2,
) -> Result<Staged, &'static str> {
    if bp.edict.is_none() {
        return Err("This Ship Carries No Edict");
    }
    let id = |key: &str| blueprints.id_of(key).ok_or("No Camp To Strike");
    let spawn = |blueprint, x: i32, y: i32| Command::DebugSpawn {
        owner: RED,
        blueprint,
        pos: mark + FxVec2::from_ints(x, y),
        heading: Angle::HALF_TURN,
        count: 1,
        flags: 0,
        build: 1000,
    };
    let (tank, power, dome) = (
        id(DEFAULT_SUBJECT)?,
        id("aster_t1_power")?,
        id("aster_t3_shield")?,
    );
    // The gather paid in full, so it never stalls.
    let mut commands = vec![Command::DebugFreeBuild {
        player: BLUE,
        on: true,
    }];
    for ring in [0, 250, 600, 950, 1300] {
        let n = if ring == 0 { 1 } else { 8 };
        for k in 0..n {
            let a = Angle::from_degrees(k * 360 / n + ring / 50);
            let at = FxVec2::from_angle(a) * Fx::from_int(ring);
            commands.push(spawn(tank, at.x.round_int(), at.y.round_int()));
        }
    }
    for (x, y) in [(-180, -260), (220, 300), (520, -160)] {
        commands.push(spawn(power, x, y));
    }
    commands.push(spawn(dome, -700, 500));
    Ok((commands, Some((bp.id, PendingOrder::Edict { pos: mark }))))
}
