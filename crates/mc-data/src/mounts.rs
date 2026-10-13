//! Weapon groups: one weapon of a unit file that drives many guns (`RawWeapon::mounts`).
//!
//! A group's guns are alike in everything but where they stand: each mount is a house of
//! its own on the hull, with its own pivot, the way it rests and the arc it covers, and in
//! the sim its own aim, target, reload and salvo (`mc_sim::mounts`). The group's `muzzle`
//! and `muzzles` are given in the house's frame, about its pivot as it faces the nose.
//!
//! Loading compiles each mount to an ordinary `Weapon` of its own (`Weapon::mount_weapons`),
//! so every check the sim makes of a gun (its reach, its arc, its pitch, its muzzles) reads
//! a mount the way it reads any house. The group itself carries the first mount's geometry,
//! so what reads it without knowing of groups (a look, a sound) still finds a gun there.
//!
//! Gun slots number every gun of a unit: weapon by weapon, a group's mounts in their order
//! (`UnitBlueprint::gun_slot`). A model binds each house to its gun slot, and the mirror
//! poses houses by it (`mirror::HousePose`).

use mc_core::{Angle, FxVec3};
use serde::Deserialize;

use crate::raw::{fx, steps, RawWeapon};
use crate::{DataError, UnitBlueprint, Weapon};

/// Most guns (gun slots) one unit carries: the renderer's rig numbers house slots this far
/// (`gpu_consts::house`), and `HousePose` records hold them eight at a time.
pub const MAX_GUNS: usize = 128;

/// One gun of a group, as the unit file gives it.
#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(deny_unknown_fields)]
pub struct RawMount {
    /// Where its house turns and pitches, in the hull's frame (x forward, y left, z up).
    pub pivot: (f64, f64, f64),
    /// Which way it rests, degrees off the nose, positive to the left; its arc is centred
    /// there, and it takes only targets inside that arc.
    #[serde(default)]
    pub facing: f64,
    /// Its full arc in degrees. None: the group's `arc`.
    #[serde(default)]
    pub arc: Option<f64>,
}

/// One gun of a group (`Weapon::mounts`): its pivot, rest and arc.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mount {
    pub pivot: FxVec3,
    pub facing: Angle,
    /// Half-width of its arc about `facing`; 0x8000 is all round.
    pub half_arc: u16,
}

/// Refuses a group that asks for what a gun of a group cannot do. A group's guns are
/// houses that turn (`turret_turn`), each placed by its mount, firing shots of its own;
/// what keeps state for a whole unit (cells, orbs, drones, a lens's link, a sweep's
/// torso, a storm, a twin's turn) stays with ordinary weapons.
pub(crate) fn check(ctx: &str, w: &RawWeapon) -> Result<(), DataError> {
    if w.mounts.is_empty() {
        return Ok(());
    }
    let refused = [
        (w.mounts.len() > crate::MAX_GUNS, "too many mounts"),
        (w.turret_turn <= 0.0, "a group's guns turn (turret_turn)"),
        (w.pivot.is_some(), "a group's pivots are its mounts'"),
        (
            w.facing != 0.0 || w.rear,
            "a group's facings are its mounts'",
        ),
        (w.sway > 0.0 || w.fixed_rack, "a group's guns are houses"),
        (
            w.vertical_launch || w.hatch > 0.0,
            "cells are an ordinary weapon",
        ),
        (w.orbs > 0, "orbs are an ordinary weapon"),
        (w.launches, "drones are an ordinary weapon"),
        (w.lens.is_some(), "a lens is an ordinary weapon"),
        (w.cone.is_some(), "a cone is an ordinary weapon"),
        (w.strafe.is_some(), "a strafe is an ordinary weapon"),
        (w.cloud.is_some(), "a storm seed is an ordinary weapon"),
        (w.bore.is_some(), "a bore is an ordinary weapon"),
        (
            w.split || w.split_ordered,
            "a split salvo is an ordinary weapon",
        ),
        (
            w.on_dash || w.dash_only,
            "a dash weapon is an ordinary weapon",
        ),
        (
            w.keep_aim || w.keeps_aim,
            "a group's guns choose their own marks",
        ),
        (
            w.intercepts || w.torpedo,
            "torpedoes are an ordinary weapon",
        ),
        (w.barrels > 0, "a barrel-timed gun is an ordinary weapon"),
        (
            w.reload_energy > 0.0,
            "a fed reload is an ordinary weapon's",
        ),
    ];
    for (bad, why) in refused {
        if bad {
            return Err(DataError::Invalid(format!("{ctx}: weapon group: {why}")));
        }
    }
    if w.mounts.iter().any(|m| {
        [m.pivot.0, m.pivot.1, m.pivot.2, m.facing]
            .iter()
            .chain(m.arc.iter())
            .any(|v| !v.is_finite())
            || m.arc.is_some_and(|a| !(0.0..=360.0).contains(&a))
    }) {
        return Err(DataError::Invalid(format!(
            "{ctx}: a mount's pivot and facing are finite and its arc 0..=360"
        )));
    }
    Ok(())
}

/// The mounts of `w`, compiled.
pub(crate) fn compile(w: &RawWeapon) -> Vec<Mount> {
    w.mounts
        .iter()
        .map(|m| Mount {
            pivot: FxVec3::new(fx(m.pivot.0), fx(m.pivot.1), fx(m.pivot.2)),
            facing: Angle(steps(m.facing).round() as i64 as u16),
            half_arc: half_arc(m.arc.unwrap_or(w.arc)),
        })
        .collect()
}

/// Half an arc of `degrees`, in angle steps; a full turn is all round.
pub(crate) fn half_arc(degrees: f64) -> u16 {
    if degrees >= 360.0 {
        0x8000
    } else {
        (steps(degrees) / 2.0).round() as u16
    }
}

/// Makes `group` (compiled with its muzzles in the house frame) a group of `mounts`: each
/// mount's own weapon, its muzzles carried out to its pivot, and the group itself given
/// the first mount's geometry.
pub(crate) fn assemble(group: &mut Weapon, mounts: Vec<Mount>) {
    if mounts.is_empty() {
        return;
    }
    let at = |m: &Mount, p: FxVec3| m.pivot + p;
    let guns: Vec<Weapon> = mounts
        .iter()
        .map(|m| Weapon {
            pivot: Some(m.pivot),
            muzzle: at(m, group.muzzle),
            muzzles: group.muzzles.iter().map(|&p| at(m, p)).collect(),
            facing: m.facing,
            half_arc: m.half_arc,
            mount: true,
            ..group.clone()
        })
        .collect();
    *group = Weapon {
        mounts,
        mount_weapons: Vec::new(),
        ..guns[0].clone()
    };
    group.mount_weapons = guns;
}

impl Weapon {
    /// How many guns this weapon is: its mounts, or one.
    pub fn guns(&self) -> usize {
        self.mounts.len().max(1)
    }

    /// A weapon group (`mounts`).
    pub fn is_group(&self) -> bool {
        !self.mounts.is_empty()
    }

    /// Gun `m` of this weapon: a group's mount, or the weapon itself.
    pub fn gun(&self, m: usize) -> &Weapon {
        self.mount_weapons.get(m).unwrap_or(self)
    }
}

impl UnitBlueprint {
    /// The first gun slot of weapon `w`: every weapon before it takes one slot, a group
    /// one for each of its mounts.
    pub fn gun_slot(&self, w: usize) -> usize {
        self.weapons.iter().take(w).map(Weapon::guns).sum()
    }

    /// Every gun slot of the unit.
    pub fn gun_count(&self) -> usize {
        self.gun_slot(self.weapons.len())
    }

    /// The weapon and mount (zero for an ordinary weapon) of gun slot `slot`.
    pub fn gun_of_slot(&self, slot: usize) -> Option<(usize, usize)> {
        let mut first = 0;
        for (w, weapon) in self.weapons.iter().enumerate() {
            let n = weapon.guns();
            if slot < first + n {
                return Some((w, slot - first));
            }
            first += n;
        }
        None
    }

    /// The farthest a group's pivot lies off the unit's middle across the map, metres:
    /// what a search for its guns' marks round the hull widens by. Zero for an ordinary
    /// weapon.
    pub fn mount_reach(&self, w: usize) -> mc_core::Fx {
        self.weapons[w]
            .mounts
            .iter()
            .map(|m| m.pivot.xy().length())
            .max()
            .unwrap_or(mc_core::Fx::ZERO)
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use mc_core::{Angle, Fx, FxVec3};

    use crate::{BlueprintId, Blueprints, DataError, FactionId, UnitBlueprint};

    /// A ship of `weapon` (RON), compiled as the loader compiles a unit file's.
    fn ship(weapon: &str) -> Result<UnitBlueprint, DataError> {
        let text = format!(
            "(key: \"ship\", name: \"Ship\", role: \"Test\", tech: 1, categories: [\"Naval\"],
              health: 100, cost: (materials: 1, energy: 1, time: 1), radius: 50, height: 10,
              vision: 100, motion: (layer: Naval, size: 0, speed: 10, accel: 1, turn: 10),
              hull_mounts: true, weapons: [{weapon}], mesh: \"ship\", icon: Warship)"
        );
        let unit: crate::raw::Unit = ron::Options::default()
            .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
            .from_str(&text)
            .unwrap();
        unit.compile(BlueprintId(0), FactionId(0), &|_, _| Ok(BlueprintId(0)))
    }

    const GROUP: &str = "(name: \"Battery\", damage: 10, range: 500, reload: 1, speed: 400,
        trajectory: Direct, turret_turn: 60, arc: 180, muzzle: (6, 0, 1),
        muzzles: [(6, 1, 1), (6, -1, 1)], targets: [\"Naval\"], color: Orange,
        mounts: [(pivot: (20, 8, 4), facing: 90), (pivot: (-20, -8, 4), facing: -90, arc: 360)])";

    /// Each mount is a whole weapon of its own: a house at its pivot, its muzzles carried out
    /// from it, resting and covering its own arc; the group takes its first mount's
    /// geometry, and gun slots count the mounts after the weapons before them.
    #[test]
    fn a_groups_mounts_are_houses_at_their_pivots() {
        let plain = "(name: \"Gun\", damage: 1, range: 100, reload: 1, speed: 100,
            trajectory: Direct, muzzle: (1, 0, 1), targets: [\"Naval\"], color: Orange)";
        let u = ship(&format!("{plain}, {GROUP}, {plain}")).unwrap();
        let group = &u.weapons[1];
        assert!(group.is_group() && group.guns() == 2 && !u.weapons[0].is_group());
        let fx =
            |x: i32, y: i32, z: i32| FxVec3::new(Fx::from_int(x), Fx::from_int(y), Fx::from_int(z));
        let port = group.gun(0);
        assert_eq!(port.pivot, Some(fx(20, 8, 4)));
        assert_eq!(port.muzzle, fx(26, 8, 5));
        assert_eq!(port.muzzles, vec![fx(26, 9, 5), fx(26, 7, 5)]);
        assert_eq!(port.facing, Angle::from_degrees(90));
        assert_eq!(port.half_arc, 0x4000);
        assert!(port.mount && !port.is_group());
        let starboard = group.gun(1);
        assert_eq!(starboard.muzzle, fx(-14, -8, 5));
        assert_eq!(starboard.facing, Angle::from_degrees(-90));
        assert_eq!(starboard.half_arc, 0x8000);
        assert_eq!(group.pivot, port.pivot);
        assert_eq!(group.mounts.len(), 2);
        assert_eq!((u.gun_slot(1), u.gun_slot(2), u.gun_count()), (1, 3, 4));
        assert_eq!(u.gun_of_slot(2), Some((1, 1)));
        assert_eq!(u.gun_of_slot(3), Some((2, 0)));
        assert_eq!(u.gun_of_slot(4), None);
        assert_eq!(u.mount_reach(1), fx(20, 8, 0).xy().length());
    }

    /// What keeps state for a whole unit stays with ordinary weapons: a group that asks for
    /// it is refused at load, as is one with a pivot of its own.
    #[test]
    fn a_group_that_asks_for_a_whole_units_state_is_refused() {
        for extra in [
            "pivot: (0, 0, 1),",
            "split: true,",
            "orbs: 2, orb_time: 1,",
            "facing: 90,",
        ] {
            let weapon = GROUP.replacen("mounts:", &format!("{extra} mounts:"), 1);
            assert!(ship(&weapon).is_err(), "{extra}");
        }
        assert!(ship(&GROUP.replace("turret_turn: 60", "turret_turn: 0")).is_err());
    }

    /// Every unit of the data keeps to the gun limit, and its groups' guns are houses.
    #[test]
    fn every_units_guns_fit() {
        let bp =
            Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap();
        for unit in &bp.units {
            assert!(unit.gun_count() <= super::MAX_GUNS, "{}", unit.key);
            for weapon in unit.weapons.iter().filter(|w| w.is_group()) {
                assert_eq!(weapon.mount_weapons.len(), weapon.mounts.len());
                assert!(weapon.mount_weapons.iter().all(|g| g.mount), "{}", unit.key);
            }
        }
    }
}
