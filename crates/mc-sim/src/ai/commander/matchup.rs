//! How equal mass of one unit fares against another, from their profiles
//! (`docs/AI_COMMANDER.md`, "Matchups"). Calibrated against staged fights in the sim
//! (`matchup_tests.rs`, `zz_matchup_probe`).
//!
//! Lanchester's square law: a side's fighting worth goes with its numbers squared
//! times how fast it kills (its damage a second over the other's health). A longer
//! reach gets free volleys while the shorter closes the gap; a static target that is
//! out-ranged never shoots back. Splash counts for more against cheap crowds.
use super::profile::{Profile, Target};
use mc_core::Fx;

/// The most a matchup is ever said to favour one side.
const CAP: Fx = Fx::from_int(64);
/// Splash's bonus against a target whose unit costs this little or less, scaled down
/// for dearer ones.
const CROWD_COST: Fx = Fx::from_int(400);
/// How much of their damage splash adds against a crowd (x, of 1).
const SPLASH_BONUS: Fx = Fx::ratio(3, 4);
/// Equal mass staged: what the comparison is made at.
const STAKE: Fx = Fx::from_int(4000);

/// Damage a second `a` puts on `b` from `d` metres, crowds counted.
fn damage(a: &Profile, b: &Profile, d: Fx) -> Fx {
    let Some(class) = b.is else {
        return Fx::ZERO;
    };
    let dps = a.dps_at(class, d, b);
    if dps == Fx::ZERO {
        return Fx::ZERO;
    }
    let crowd = if b.cost <= CROWD_COST {
        Fx::ONE
    } else {
        (CROWD_COST / b.cost).min(Fx::ONE)
    };
    dps + dps * a.splash * SPLASH_BONUS * crowd
}

fn reach(a: &Profile, b: &Profile) -> Fx {
    b.is.map_or(Fx::ZERO, |c: Target| a.reach[c as usize])
}

/// The result of `STAKE` mass of `a` against as much of `b`: above one `a` wins, by
/// how much (Lanchester's ratio of fighting worth), capped at `CAP` either way.
///
/// The fight is had at the shorter of the two reaches (the one with the shorter
/// closes to it), so a gun with a long minimum range that the other side closes
/// inside does not count there; it does while the gap closes.
pub(in crate::ai) fn fight(a: &Profile, b: &Profile) -> Fx {
    let (ra, rb) = (reach(a, b), reach(b, a));
    if ra == Fx::ZERO && rb == Fx::ZERO {
        return Fx::ONE;
    }
    // Where the fight settles: inside the shorter reach, or the longer one when
    // the other cannot shoot back at all (it closes to its own weapons' reach).
    let close = match (ra > Fx::ZERO, rb > Fx::ZERO) {
        (true, true) => ra.min(rb),
        (true, false) => ra,
        _ => rb,
    };
    let (da, db) = (damage(a, b, close), damage(b, a, close));
    let mut na = STAKE / a.cost.max(Fx::ONE);
    let mut nb = STAKE / b.cost.max(Fx::ONE);
    // The longer reach shoots while the other closes, if it can close at all.
    if ra > rb {
        if !b.mobile() {
            return CAP;
        }
        let t = (ra - rb) / b.speed.max(Fx::ONE);
        let far = damage(a, b, ra);
        let lost = (t * na * far / (nb * b.ehp).max(Fx::ONE)).min(Fx::ratio(9, 10));
        nb -= nb * lost;
    } else if rb > ra {
        if !a.mobile() {
            return Fx::ONE / CAP;
        }
        let t = (rb - ra) / a.speed.max(Fx::ONE);
        let far = damage(b, a, rb);
        let lost = (t * nb * far / (na * a.ehp).max(Fx::ONE)).min(Fx::ratio(9, 10));
        na -= na * lost;
    }
    if db == Fx::ZERO && da == Fx::ZERO {
        // Neither can shoot at the distance it settles at: whoever had the range
        // phase is ahead by what it took.
        return (na / nb.max(Fx::ratio(1, 100))).clamp(Fx::ONE / CAP, CAP);
    }
    if db == Fx::ZERO {
        return CAP;
    }
    if da == Fx::ZERO {
        return Fx::ONE / CAP;
    }
    // Worth is numbers squared times kill rate (damage over the other's health); as
    // a ratio, taken a factor at a time so nothing grows large.
    let numbers = (na / nb.max(Fx::ratio(1, 100))).clamp(Fx::ONE / CAP, CAP);
    let rate = (da / db).clamp(Fx::ONE / CAP, CAP);
    let toughness = (a.ehp.max(Fx::ONE) / b.ehp.max(Fx::ONE)).clamp(Fx::ONE / CAP, CAP);
    ((numbers * numbers).min(CAP) * rate * toughness).clamp(Fx::ONE / CAP, CAP)
}

/// `fight` folded to the range -1..1: how sure the estimate is that `a` wins.
pub(in crate::ai) fn edge(a: &Profile, b: &Profile) -> Fx {
    let r = fight(a, b).clamp(Fx::ONE / CAP, CAP);
    (r - Fx::ONE) / (r + Fx::ONE)
}

#[cfg(test)]
#[path = "matchup_tests.rs"]
mod tests;
