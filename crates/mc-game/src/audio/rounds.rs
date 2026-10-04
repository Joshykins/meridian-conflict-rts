//! A stream gun whose `fire` is one round (`WeaponSounds::each_round`) is heard round by
//! round, each as it is drawn leaving the muzzle (`mc_sim::mirror::round_gap`), so the
//! reports land on the flashes rather than one report standing for the whole shot.

/// How many times a shot of `weapon` is heard, and the seconds between them, for ticks
/// `tick_seconds` long: once a round when its sound is one round, else once.
pub(crate) fn heard(weapon: &mc_data::Weapon, tick_seconds: f32) -> (u8, f32) {
    if weapon.sounds.each_round && weapon.rounds > 1 {
        let gap = mc_sim::mirror::round_gap(weapon) * tick_seconds;
        (weapon.rounds, gap)
    } else {
        (1, 0.0)
    }
}

/// Round `k`'s pitch off the shot's: rounds of one burst are never quite alike.
pub(crate) fn pitch(k: u8) -> f32 {
    [1.0, 0.985, 1.012, 0.994][k as usize % 4]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Breacher's gatling-breach cannons: each shot is three rounds a tick apart,
    /// heard as three; a gun whose `fire` is its whole burst is heard once.
    #[test]
    fn a_round_by_round_gun_is_heard_on_each_round() {
        let bps = mc_data::Blueprints::load(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"),
        )
        .unwrap();
        let breacher = bps.unit(bps.id_of("aster_t4_breacher").unwrap());
        let gun = &breacher.weapons[1];
        let (rounds, gap) = heard(gun, 0.1);
        assert_eq!(rounds, gun.rounds);
        assert!(rounds > 1);
        let drawn = mc_sim::mirror::round_gap(gun) * 0.1;
        assert!(
            (gap - drawn).abs() < 1e-6,
            "heard {gap} s apart, drawn {drawn} s"
        );
        assert_eq!(heard(&breacher.weapons[0], 0.1), (1, 0.0));
    }
}
