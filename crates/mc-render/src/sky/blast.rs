//! Blasts that shove the clouds about (clouds_sim.wgsl `cs_force`, kind blast).
use super::*;

/// At most this many blasts stir the weather at once: a cosmetic cap, the
/// newest past it leave the cloud alone (a carpet of shells, not a battle's
/// worth of nukes, fills it).
const MOST_BLASTS: usize = 48;

impl Sky {
    /// A blast big enough to shove the clouds about: its reach in metres and
    /// how hard it hits (1 a large explosion, 3 a reactor going up).
    /// A blast below the layer reaches it only if it is big for the gap: a
    /// shell or a tank going up on the ground leaves the cloud a couple of
    /// hundred metres overhead alone; an aircraft blowing up inside it, or a
    /// reactor, does not.
    pub fn blast(&mut self, at: Vec3, reach: f32, strength: f32, time: f32) {
        let gap = (self.floor_at(at.truncate()) + self.base - at.z).max(0.0);
        let Some((reach, strength)) = reaching(gap, reach, strength) else {
            return;
        };
        if self.blasts.len() < MOST_BLASTS {
            self.blasts.push((at.truncate(), reach, strength, time));
        }
    }
}

/// How far and how hard a blast `gap` metres under the cloud base reaches it,
/// or none. A blast with no reach is none: a nova's cap parting the cloud on
/// the frame it bursts is 0 m across, and smoothstep over 0 m made a NaN that
/// the weather carried over the whole map for the rest of the match, every
/// cloud a smooth smear.
fn reaching(gap: f32, reach: f32, strength: f32) -> Option<(f32, f32)> {
    if !(reach > 0.0 && reach.is_finite() && strength.is_finite() && gap.is_finite()) {
        return None;
    }
    let fade = 1.0 - smoothstep(0.0, reach * 0.5, gap);
    (fade >= 0.05).then_some((reach * (0.5 + 0.5 * fade), strength * fade))
}

#[cfg(test)]
mod tests {
    use super::reaching;

    #[test]
    fn a_blast_with_no_reach_leaves_the_cloud_alone() {
        assert_eq!(reaching(0.0, 0.0, 1.6), None);
        assert_eq!(reaching(0.0, f32::NAN, 1.6), None);
        assert_eq!(reaching(0.0, 700.0, f32::INFINITY), None);
    }

    #[test]
    fn a_blast_in_the_cloud_reaches_it_in_full() {
        assert_eq!(reaching(0.0, 700.0, 2.0), Some((700.0, 2.0)));
    }

    #[test]
    fn a_small_blast_far_under_the_cloud_does_not_reach_it() {
        assert_eq!(reaching(400.0, 300.0, 1.0), None);
        let (reach, strength) = reaching(100.0, 1600.0, 2.2).unwrap();
        assert!(reach > 800.0 && reach < 1600.0 && strength > 0.0 && strength < 2.2);
    }
}
