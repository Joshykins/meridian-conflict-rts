//! Core mines: every hectare in reach pays, ore far more; overlapping mines
//! split the ground along a straight line; tiers raise the yield, not the reach.

use mc_core::{Angle, Fx, FxVec2};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::{Heightfield, OreRegion};
use mc_sim::mines::mine_rate;
use mc_sim::tables::Controller;
use mc_sim::world::MapData;
use mc_sim::{Command, MatchConfig, PlayerCommand, PlayerSetup, UnitId, World};
use std::path::Path;
use std::sync::Arc;

/// A square ore field `2 * half` metres across.
fn square(x: i32, y: i32, half: i32) -> OreRegion {
    OreRegion {
        points: [(-1, -1), (1, -1), (1, 1), (-1, 1)]
            .into_iter()
            .map(|(dx, dy)| FxVec2::from_ints(x + dx * half, y + dy * half))
            .collect(),
    }
}

fn world(ore: Vec<OreRegion>) -> World {
    let blueprints = Arc::new(
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
    );
    let terrain = Heightfield::flat(1280, 1280, Fx::from_int(20));
    let map = MapData {
        name: "mines".into(),
        content_id: 1,
        ore,
        starts: vec![FxVec2::from_ints(512, 512), FxVec2::from_ints(9500, 9500)],
        props: Vec::new(),
    };
    let player = |name: &str, team| PlayerSetup {
        name: name.into(),
        faction: "Aster".into(),
        ai: Default::default(),
        team,
        controller: Controller::Human,
        start: team,
    };
    let config = MatchConfig {
        seed: 5,
        players: vec![player("you", 0), player("them", 1)],
        cheats: true,
        fog: false,
        spawn_commanders: false,
    };
    let mut w =
        World::with_terrain(terrain, map, blueprints, Arc::new(Pool::new(1)), &config).unwrap();
    for p in &mut w.state.players {
        p.mass_capacity = Fx::from_int(100_000);
        p.energy_capacity = Fx::from_int(1_000_000);
    }
    w
}

fn cmd(player: u8, command: Command) -> PlayerCommand {
    PlayerCommand { player, command }
}

/// A finished mine for `owner` at (x, y); returns its id after one tick.
fn mine(w: &mut World, owner: u8, x: i32, y: i32) -> UnitId {
    let bp = w.blueprints.id_of("aster_core_mine").unwrap();
    w.tick(&[cmd(
        0,
        Command::DebugSpawn {
            owner,
            blueprint: bp,
            pos: FxVec2::from_ints(x, y),
            heading: Angle::ZERO,
            count: 1,
            flags: 0,
            build: 1000,
        },
    )])
    .unwrap();
    let u = &w.state.units;
    let row = u
        .slots
        .iter()
        .filter(|&r| u.blueprint[r] == bp && u.owner[r] == owner)
        .min_by_key(|&r| u.pos[r].distance_sq(FxVec2::from_ints(x, y)))
        .expect("mine spawned");
    u.id(row)
}

fn spec(w: &World) -> mc_data::Mine {
    let bp = w.blueprints.id_of("aster_core_mine").unwrap();
    w.blueprints.unit(bp).mine.unwrap()
}

fn made(w: &World, id: UnitId) -> Fx {
    let m = w.state.mines.by_unit[&id].clone();
    let row = w.state.units.row(id).unwrap();
    m.rate(&w.bp(row).mine.unwrap())
}

/// Every mine done digging, as if long finished.
fn dig_out(w: &mut World) {
    for m in w.state.mines.by_unit.values_mut() {
        m.age = 1_000_000;
    }
}

fn hectares(r: Fx) -> f64 {
    let r = r.to_f64();
    std::f64::consts::PI * r * r / 10_000.0
}

#[test]
fn every_hectare_in_reach_pays_and_ore_pays_more() {
    // One field 160 m across, well inside the reach of a mine at its middle.
    let mut w = world(vec![square(2500, 2500, 80)]);
    let bare = mine(&mut w, 0, 7002, 5002);
    let rich = mine(&mut w, 0, 2502, 2502);
    w.tick(&[]).unwrap();
    let m = spec(&w);
    dig_out(&mut w);
    let (b, r) = (
        w.state.mines.by_unit[&bare].clone(),
        w.state.mines.by_unit[&rich].clone(),
    );
    // All land in reach counts, on a 32 m grid: within a few percent of the circle.
    let circle = hectares(m.reach);
    let land = b.land.ground.to_f64();
    assert!(
        (land - circle).abs() < circle * 0.02,
        "{land} of {circle} ha"
    );
    assert_eq!(b.land.ore, Fx::ZERO);
    assert_eq!(b.land.efficiency(&m), Fx::ONE);
    // 160 m square = 2.56 ha, counted on an 8 m grid.
    let ore = r.land.ore.to_f64();
    assert!((ore - 2.56).abs() < 0.05, "{ore} ha");
    assert!(made(&w, rich) > made(&w, bare));
    assert_eq!(made(&w, bare), m.base + m.ground * b.land.ground);
    assert_eq!(
        made(&w, bare),
        mine_rate(&m, Fx::ONE, b.land.ground, Fx::ZERO)
    );
    // Both are paid into the owner's income.
    w.tick(&[]).unwrap();
    let income = w.state.players[0].mass_income;
    let sum = made(&w, bare) + made(&w, rich);
    assert!(
        (income - sum).abs() < Fx::ratio(1, 100),
        "{income:?} vs {sum:?}"
    );
}

#[test]
fn overlapping_mines_split_the_ground_along_a_straight_line() {
    // The field sits right on the line between the two.
    let mut w = world(vec![square(5000, 5000, 80)]);
    let reach = spec(&w).reach.floor_int();
    let a = mine(&mut w, 0, 5002 - reach / 2, 5002);
    w.tick(&[]).unwrap();
    let alone = w.state.mines.by_unit[&a].clone();
    // Anyone may build right beside it: an enemy mine a reach away.
    let b = mine(&mut w, 1, 5002 + reach / 2, 5002);
    w.tick(&[]).unwrap();
    let m = spec(&w);
    let (sa, sb) = (
        w.state.mines.by_unit[&a].clone(),
        w.state.mines.by_unit[&b].clone(),
    );
    assert!(sa.land.ground < alone.land.ground);
    assert!(sa.land.ore < alone.land.ore);
    // Two circles a radius apart: each keeps its circle less the lens half
    // beyond the chord, about 80%.
    let kept = (sa.land.ground / alone.land.ground).to_f64();
    assert!((kept - 0.80).abs() < 0.02, "kept {kept} of its land");
    // Half the field went too, so it keeps less of its output than of its land.
    let e = sa.land.efficiency(&m).to_f64();
    assert!(e < kept && e > 0.5, "efficiency {e}");
    // Two alike split down the middle: the same territory each (to the grid).
    let (ga, gb) = (sa.land.ground.to_f64(), sb.land.ground.to_f64());
    assert!((ga - gb).abs() < ga * 0.02, "{ga} vs {gb} ha");
    // Nothing counted twice: the field is split between them, whole.
    let total = (sa.land.ore + sb.land.ore).to_f64();
    assert!((total - 2.56).abs() < 0.05, "{total} ha");
    // Take the neighbour away and the first has its reach to itself again.
    w.tick(&[cmd(0, Command::DebugRemove { units: vec![b] })])
        .unwrap();
    w.tick(&[]).unwrap();
    assert_eq!(w.state.mines.by_unit[&a].land, alone.land);
}

#[test]
fn packing_mines_together_pays_no_more_than_the_ground_they_share() {
    // A 7 x 7 block of mines 100 m apart, against one mine on its own and four
    // spread out so their reaches barely touch.
    let rate = |at: &[(i32, i32)]| {
        let mut w = world(Vec::new());
        let ids: Vec<UnitId> = at.iter().map(|&(x, y)| mine(&mut w, 0, x, y)).collect();
        w.tick(&[]).unwrap();
        dig_out(&mut w);
        ids.iter().map(|&id| made(&w, id).to_f64()).sum::<f64>()
    };
    let one = rate(&[(5002, 5002)]);
    let block: Vec<(i32, i32)> = (0..49)
        .map(|i| (4702 + i % 7 * 100, 4702 + i / 7 * 100))
        .collect();
    let packed = rate(&block);
    let spread = rate(&[(3002, 3002), (7002, 3002), (3002, 7002), (7002, 7002)]);
    // The block covers a little more ground than one circle, so it makes a
    // little more; shafts crowded into it share one circle's base between them.
    assert!(
        packed < one * 2.5,
        "49 packed make {packed}/s, one alone {one}/s"
    );
    assert!(
        spread > packed,
        "4 spread make {spread}/s, 49 packed {packed}/s"
    );
}

#[test]
fn an_upgraded_mine_keeps_its_reach_but_yields_more_once_the_side_has_the_tech() {
    let mut w = world(Vec::new());
    let id = mine(&mut w, 0, 5002, 5002);
    let t1 = spec(&w);
    let t2 = w.blueprints.id_of("aster_core_mine_t2").unwrap();
    // Not before the side has tech 2: nothing is queued. (Free building would skip this.)
    w.tick(&[cmd(0, Command::Upgrade { units: vec![id] })])
        .unwrap();
    let row = w.state.units.row(id).unwrap();
    assert!(
        w.state.orders.front(&w.state.units, row).is_none(),
        "upgraded without tech 2"
    );
    w.tick(&[cmd(
        0,
        Command::DebugFreeBuild {
            player: 0,
            on: true,
        },
    )])
    .unwrap();
    let before = w.state.mines.by_unit[&id].clone();
    // A tech 2 engineer brings the side there.
    let engineer = w.blueprints.id_of("aster_t2_engineer").unwrap();
    w.tick(&[cmd(
        0,
        Command::DebugSpawn {
            owner: 0,
            blueprint: engineer,
            pos: FxVec2::from_ints(5400, 5002),
            heading: Angle::ZERO,
            count: 1,
            flags: 0,
            build: 1000,
        },
    )])
    .unwrap();
    assert_eq!(w.side_tech(0), 2);
    w.tick(&[cmd(0, Command::Upgrade { units: vec![id] })])
        .unwrap();
    for _ in 0..3000 {
        w.tick(&[]).unwrap();
        if w.state
            .units
            .row(id)
            .is_some_and(|r| w.state.units.blueprint[r] == t2)
        {
            break;
        }
    }
    let row = w
        .state
        .units
        .row(id)
        .expect("the same unit after the refit");
    assert_eq!(w.state.units.blueprint[row], t2);
    w.tick(&[]).unwrap();
    let s = w.state.mines.by_unit[&id].clone();
    let spec2 = w.bp(row).mine.unwrap();
    assert_eq!(spec2.reach, t1.reach);
    assert_eq!(s.land, before.land, "the same territory");
    assert!(s.full_rate(&spec2) > before.full_rate(&t1) * 2);
    assert!(s.age > before.age, "digging carries on through the upgrade");
}

#[test]
fn a_mine_stops_paying_when_it_dies() {
    let mut w = world(Vec::new());
    let id = mine(&mut w, 0, 5002, 5002);
    w.tick(&[]).unwrap();
    assert!(w.state.players[0].mass_income > Fx::ZERO);
    w.tick(&[cmd(0, Command::DebugRemove { units: vec![id] })])
        .unwrap();
    w.tick(&[]).unwrap();
    assert!(w.state.mines.by_unit.is_empty());
    assert_eq!(w.state.players[0].mass_income, Fx::ZERO);
}

#[test]
fn ore_pays_only_once_the_drift_reaches_it() {
    let mut w = world(vec![square(5600, 5000, 80)]);
    let id = mine(&mut w, 0, 5002, 5002);
    w.tick(&[]).unwrap();
    let m = w.state.mines.by_unit[&id].clone();
    let spec = spec(&w);
    assert_eq!(m.veins.len(), 1);
    let vein = m.veins[0];
    let region = &w.map.ore[0];
    // Down the shaft to the field's depth, then out to its middle.
    let at = w.state.units.pos[w.state.units.row(id).unwrap()];
    let expect = mc_sim::mines::dig_ticks(at, region.centre(), region.depth());
    assert_eq!(vein.reached_at, expect);
    let depth = region.depth().to_f64();
    assert!((140.0..=400.0).contains(&depth), "{depth} m down");
    // At first it works next to nothing but its shaft: its land spreads out from it.
    let land = m.rate(&spec) - spec.base;
    assert!(land < (m.full_rate(&spec) - spec.base) / 100, "{land:?}");
    while w.state.mines.by_unit[&id].age < vein.reached_at {
        w.tick(&[]).unwrap();
    }
    let m = w.state.mines.by_unit[&id].clone();
    // Now the ore counts, on top of the land it has spread over so far.
    assert_eq!(
        m.rate(&spec),
        mc_sim::mines::mine_rate(&spec, Fx::ONE, m.worked_ground(), vein.ore)
    );
    assert!(m.rate(&spec) > spec.ground * m.worked_ground());
}

#[test]
fn a_new_mine_spreads_over_its_land_from_nothing() {
    let mut w = world(Vec::new());
    let id = mine(&mut w, 0, 5002, 5002);
    w.tick(&[]).unwrap();
    let spec = spec(&w);
    let start = w.state.mines.by_unit[&id].clone();
    assert!(start.worked_ground() < start.land.ground / 300);
    // A minute on it works a circle of SPREAD_SPEED metres a second.
    for _ in 0..600 {
        w.tick(&[]).unwrap();
    }
    let m = w.state.mines.by_unit[&id].clone();
    let r = (mc_sim::mines::SPREAD_SPEED * 60) as f64;
    let circle = std::f64::consts::PI * r * r / 10_000.0;
    let worked = m.worked_ground().to_f64();
    assert!(
        (worked - circle).abs() < circle * 0.15,
        "{worked} ha vs {circle}"
    );
    assert!(m.rate(&spec) > start.rate(&spec));
    // In the end, all of it.
    let mut m = m;
    m.age = 1_000_000;
    assert_eq!(m.worked_ground(), m.land.ground);
}
