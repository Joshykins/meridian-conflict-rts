//! A gun whose barrels all fire on one tick (a battleship's triple turret, `salvo_delay`
//! 0) is heard as one report, a little heavier, not one per barrel: the same recipe
//! started three times in the same instant combs into a phasey buzz and eats the mixer's
//! voices. Every battery of one hull charging on the same tick (`volley`) with the same
//! sound is likewise heard as one charge.

use mc_data::Blueprints;
use mc_sim::SimEvent;

/// For each of `events`, how many it stands for: zero when it is folded into an earlier
/// one (a shot from the same gun within a hull's radius, or the same hull's charge with
/// the same sound), else one plus those folded into it.
pub(crate) fn fold(events: &[SimEvent], bps: &Blueprints) -> Vec<u8> {
    let mut count = vec![1u8; events.len()];
    for (i, event) in events.iter().enumerate() {
        let earlier = match event {
            SimEvent::ShotFired {
                pos,
                blueprint,
                weapon,
                ..
            } => {
                let reach = bps.unit(*blueprint).radius.to_f32();
                let at = pos.xy().to_f32();
                events[..i].iter().position(|e| {
                    matches!(e, SimEvent::ShotFired { pos: p, blueprint: b, weapon: w, .. }
                    if b == blueprint && w == weapon && {
                        let q = p.xy().to_f32();
                        (q[0] - at[0]).hypot(q[1] - at[1]) < reach
                    })
                })
            }
            SimEvent::WeaponCharging {
                unit,
                blueprint,
                weapon,
                ..
            } => {
                let sound = &bps.unit(*blueprint).weapons[*weapon as usize].sounds.charge;
                events[..i].iter().position(|e| {
                    matches!(e, SimEvent::WeaponCharging { unit: u, blueprint: b, weapon: w, .. }
                        if u == unit && bps.unit(*b).weapons[*w as usize].sounds.charge == *sound)
                })
            }
            _ => None,
        };
        // Folded into the first of its kind, which is never itself folded.
        if let Some(first) = earlier.filter(|&j| count[j] > 0) {
            count[first] = count[first].saturating_add(1);
            count[i] = 0;
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use mc_core::{Fx, FxVec3};
    use mc_data::BlueprintId;
    use mc_sim::Handle;

    fn blueprints() -> Blueprints {
        Blueprints::load(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"))
            .unwrap()
    }

    fn shot(b: BlueprintId, weapon: u8, x: i32) -> SimEvent {
        SimEvent::ShotFired {
            pos: FxVec3::new(Fx::from_int(x), Fx::from_int(100), Fx::from_int(10)),
            vel: FxVec3::new(Fx::ONE, Fx::ZERO, Fx::ZERO),
            travel: FxVec3::ZERO,
            color: mc_data::WeaponColor::Blue,
            owner: 0,
            blueprint: b,
            weapon,
        }
    }

    fn charge(b: BlueprintId, unit: u32, weapon: u8) -> SimEvent {
        SimEvent::WeaponCharging {
            unit: Handle(unit),
            pos: FxVec3::ZERO,
            owner: 0,
            blueprint: b,
            weapon,
        }
    }

    /// A Leviathan's broadside: three barrels of each of three turrets on one tick are
    /// three reports, one per turret, and its three charges are one; a second
    /// Leviathan far off is heard on its own.
    #[test]
    fn a_broadside_is_one_report_per_turret_and_one_charge() {
        let bps = blueprints();
        let lev = bps.id_of("aster_t3_battleship").unwrap();
        let mut events = Vec::new();
        for weapon in 0..3u8 {
            for barrel in 0..3 {
                events.push(shot(lev, weapon, 1000 + barrel * 3));
            }
        }
        events.push(shot(lev, 0, 3000));
        events.extend((0..3).map(|w| charge(lev, 7, w)));
        events.push(charge(lev, 8, 0));
        let heard = fold(&events, &bps);
        assert_eq!(heard, vec![3, 0, 0, 3, 0, 0, 3, 0, 0, 1, 3, 0, 0, 1]);
    }
}
