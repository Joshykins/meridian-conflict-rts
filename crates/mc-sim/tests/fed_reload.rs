//! Fed reloads (`mc_sim::fed_reload`): an artillery installation's reload draws energy
//! while it runs and goes only as fast as it is paid, and power plants built against it
//! cut the draw and speed it up by the same share.

use super::adjacency::{row, spawn, world};
use mc_core::Fx;
use mc_sim::{UnitId, World};

/// An Onager at (792, 792), its lot 780..804 both ways.
const AT: (i32, i32) = (792, 792);

/// Starts a full reload on the gun and gives its side `energy` in store.
fn reload(w: &mut World, gun: UnitId, energy: i32) {
    let r = row(w, gun);
    let ticks = w.blueprints.unit(w.state.units.blueprint[r]).weapons[0].reload_ticks;
    w.state.units.weapon_cooldown[r][0] = ticks;
    let pl = &mut w.state.players[0];
    pl.bonus_storage[1] = Fx::from_int(100_000);
    pl.energy = Fx::from_int(energy);
}

/// Ticks until the gun's reload is done (at most `limit`), and the energy it drew on
/// its first tick, a second.
fn time_it(w: &mut World, gun: UnitId, limit: u32) -> (u32, f32) {
    let mut first = 0.0;
    for t in 1..=limit {
        w.tick(&[]).unwrap();
        let r = row(w, gun);
        if t == 1 {
            first = w.flows[r].used[1].to_f32() * 10.0;
        }
        if w.state.units.weapon_cooldown[r][0] == 0 {
            return (t, first);
        }
    }
    (limit + 1, first)
}

#[test]
fn a_paid_reload_takes_its_time_and_draws_its_power() {
    let mut w = world();
    let gun = spawn(&mut w, 0, "aster_t2_artillery", AT.0, AT.1, 1000);
    reload(&mut w, gun, 50_000);
    let (ticks, drew) = time_it(&mut w, gun, 200);
    // A 5.5 s reload; its last tick is counted with the economy's.
    assert_eq!(ticks, 54);
    assert!((drew - 300.0).abs() < 1.0, "draws {drew} a second");
    // Done: it draws nothing more.
    w.tick(&[]).unwrap();
    assert_eq!(w.flows[row(&w, gun)].wanted[1], Fx::ZERO);
}

#[test]
fn an_unpaid_reload_stands_still() {
    let mut w = world();
    let gun = spawn(&mut w, 0, "aster_t2_artillery", AT.0, AT.1, 1000);
    reload(&mut w, gun, 0);
    let before = w.state.units.weapon_cooldown[row(&w, gun)][0];
    for _ in 0..30 {
        w.tick(&[]).unwrap();
    }
    assert_eq!(
        w.state.units.weapon_cooldown[row(&w, gun)][0],
        before,
        "no energy, no reload"
    );
}

#[test]
fn power_plants_against_it_speed_the_reload_and_cut_its_draw() {
    let mut w = world();
    let gun = spawn(&mut w, 0, "aster_t2_artillery", AT.0, AT.1, 1000);
    // Four Reactor IIIs pinwheeled round its lot, a whole side each: the 60% of a ring.
    for (x, y) in [(852, 756), (732, 828), (828, 852), (756, 732)] {
        spawn(&mut w, 0, "aster_t3_power", x, y, 1000);
    }
    w.tick(&[]).unwrap();
    reload(&mut w, gun, 50_000);
    let (ticks, drew) = time_it(&mut w, gun, 200);
    // 54 ticks at 1.6 times the pace.
    assert_eq!(ticks, 34, "ringed reload");
    assert!((drew - 120.0).abs() < 1.0, "draws {drew} a second");
}

#[test]
fn a_charged_gun_is_fed_down_to_its_charge() {
    let mut w = world();
    let gun = spawn(&mut w, 0, "aster_t4_artillery", 792, 792, 1000);
    let r = row(&w, gun);
    let weapon = &w.blueprints.unit(w.state.units.blueprint[r]).weapons[0];
    let (reload_ticks, charge) = (weapon.reload_ticks, weapon.charge_ticks);
    assert!(charge > 0);
    reload(&mut w, gun, 0);
    for _ in 0..20 {
        w.tick(&[]).unwrap();
    }
    assert_eq!(w.state.units.weapon_cooldown[r][0], reload_ticks, "unpaid");
    // Fed to a tick above the charge; combat counts on from there without power.
    w.state.units.weapon_cooldown[r][0] = charge + 3;
    w.state.players[0].energy = Fx::from_int(50_000);
    w.tick(&[]).unwrap();
    assert_eq!(w.state.units.weapon_cooldown[r][0], charge + 2);
    w.state.players[0].energy = Fx::ZERO;
    w.tick(&[]).unwrap();
    assert_eq!(w.state.units.weapon_cooldown[r][0], charge + 2, "unpaid");
    w.state.players[0].energy = Fx::from_int(50_000);
    w.tick(&[]).unwrap();
    assert_eq!(
        w.state.units.weapon_cooldown[r][0], charge,
        "fed to charge + 1, then counted"
    );
    w.state.players[0].energy = Fx::ZERO;
    w.tick(&[]).unwrap();
    assert_eq!(w.state.units.weapon_cooldown[r][0], charge - 1);
}
