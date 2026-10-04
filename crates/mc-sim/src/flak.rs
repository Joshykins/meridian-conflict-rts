//! Flak (`Weapon::flak`): a slow shell with a timed fuse. It bursts as it comes level
//! with the point it was laid on, the lead on an aircraft, unless its proximity fuse
//! has set it off on something first (`combat.rs` `sweep_projectile`), so a near miss
//! still catches a flight in its splash. A plasma airburst (`Weapon::airburst`) has the
//! same fuse.

use mc_core::{Fx, FxVec3};
use mc_data::Weapon;

/// Where along this tick's step, `from` to `from + vel`, a flak shell's timed fuse
/// fires: as it comes level with `mark`. `None` for any other shot, and for a flak
/// shell or airburst whose mark is still ahead of this step or already behind it.
pub(crate) fn fuse(weapon: &Weapon, mark: FxVec3, from: FxVec3, vel: FxVec3) -> Option<Fx> {
    let len_sq = vel.length_sq();
    if !(weapon.flak || weapon.airburst) || len_sq <= Fx::EPSILON {
        return None;
    }
    let t = (mark - from).dot(vel) / len_sq;
    (Fx::ZERO..=Fx::ONE).contains(&t).then_some(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flak() -> Weapon {
        let bp = mc_data::Blueprints::load(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"),
        )
        .unwrap();
        let squall = bp.unit(bp.id_of("aster_t2_mobile_aa").unwrap());
        squall.weapons[0].clone()
    }

    #[test]
    fn the_fuse_fires_in_the_step_that_passes_the_mark() {
        let w = flak();
        assert!(w.flak);
        let vel = FxVec3::new(Fx::from_int(30), Fx::ZERO, Fx::from_int(10));
        let mark = FxVec3::new(Fx::from_int(100), Fx::ZERO, Fx::from_int(40));
        let before = FxVec3::new(Fx::from_int(60), Fx::ZERO, Fx::from_int(20));
        assert_eq!(fuse(&w, mark, before, vel), None, "still short of the mark");
        let passing = FxVec3::new(Fx::from_int(85), Fx::ZERO, Fx::from_int(35));
        let t = fuse(&w, mark, passing, vel).expect("bursts this step");
        assert!(t > Fx::ZERO && t < Fx::ONE);
        let past = FxVec3::new(Fx::from_int(120), Fx::ZERO, Fx::from_int(46));
        assert_eq!(fuse(&w, mark, past, vel), None, "long gone");
        let plasma = Weapon {
            flak: false,
            airburst: true,
            ..w.clone()
        };
        assert_eq!(
            fuse(&plasma, mark, passing, vel),
            Some(t),
            "an airburst too"
        );
        let plain = Weapon { flak: false, ..w };
        assert_eq!(
            fuse(&plain, mark, passing, vel),
            None,
            "only flak and airbursts have a timed fuse"
        );
    }
}
