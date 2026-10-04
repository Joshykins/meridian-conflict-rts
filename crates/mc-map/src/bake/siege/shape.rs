//! Halcyon's land: the city's plain and its old town's hill, the outskirts'
//! rolling farmland, and the plan cut into them (levelled lots, craters).

use super::*;

/// The ground at the foot of the wall, either side of it.
const WALL_FOOT: f64 = 34.0;

impl Terrain {
    pub(super) fn siege_shape(&self, x: f64, y: f64) -> f64 {
        let p = (x, y);
        let wall = y_at(WALL, x);
        // The city: a plain rising gently north from the wall, the old town
        // on a low hill round the square, a little give in the ground.
        let city = WALL_FOOT
            + 18.0 * smoothstep(wall, SIZE, y)
            + 12.0 * bump(dist(p, OLD_TOWN.0) / (1.5 * OLD_TOWN.1))
            + 3.0 * self.tilt.fbm(x / 1_700.0, y / 1_700.0, 2, 0.5)
            + 1.2 * self.detail.fbm(x / 420.0, y / 420.0, 2, 0.5);
        // The outskirts: farmland rolling more the further from the wall,
        // whose glacis is kept level; the quarry hill under the southern base.
        let roll = self.cont.fbm(x / 2_300.0 + 5.3, y / 2_300.0 - 1.7, 3, 0.5);
        let knolls = self.mtn.fbm(x / 760.0 - 2.2, y / 760.0 + 9.1, 2, 0.5);
        let grain = self.detail.fbm(x / 260.0 + 1.3, y / 260.0 - 4.4, 2, 0.5);
        let away = smoothstep(wall - 380.0, wall - 1_600.0, y);
        let quarry = 20.0 * bump(dist(p, OUT_STARTS[1]) / 1_500.0);
        let out = WALL_FOOT + away * (34.0 * roll + 9.0 * knolls) + 1.2 * grain + quarry;
        let w = smoothstep(wall - 120.0, wall + 160.0, y);
        out + (city - out) * w
    }

    /// The plan cut into the land: each lot's ground levelled to it and
    /// eased back into the land round it; craters dug, never under a lot.
    pub(super) fn siege_cut(&self, x: f64, y: f64, h: f64) -> f64 {
        let p = (x, y);
        let mut h = h;
        let mut kept = 0.0_f64;
        if let Some(index) = self.siege.lot_index.as_ref() {
            // Each lot near enough pulls the ground to its level, the nearest
            // hardest, so two lots side by side on a slope meet in a ramp, not
            // a step. The wall's curtain follows the land (its segments sink
            // into it): it only keeps craters off.
            let (mut pull, mut sum, mut weight) = (0.0_f64, 0.0, 0.0);
            let mut seen: Vec<u32> = Vec::new();
            for i in index.near(p, LOT_EASE + 8.0) {
                if seen.contains(&i) {
                    continue;
                }
                seen.push(i);
                let lot = &self.siege.lots[i as usize];
                let outside = lot.rect.outside(p);
                let k = 1.0 - smoothstep(LOT_FLAT, LOT_EASE, outside);
                if k <= 0.0 {
                    continue;
                }
                kept = kept.max(k);
                if lot.kind == PropKind::CityWall {
                    continue;
                }
                let w = k / (outside.max(0.0) + 0.5).powi(2);
                pull = pull.max(k);
                sum += w * lot.level;
                weight += w;
            }
            if weight > 0.0 {
                h += (sum / weight - h) * pull;
            }
        }
        if let Some(index) = self.siege.crater_index.as_ref() {
            // The deepest bowl and the highest lip here: craters that overlap
            // do not dig each other deeper.
            let (mut bowl, mut lip) = (0.0_f64, 0.0_f64);
            for i in index.near(p, 40.0) {
                let c = self.siege.craters[i as usize];
                // Never steeper than a vehicle can climb out of.
                let depth = c.depth.min(0.2 * c.radius);
                let d = dist(p, c.at) / c.radius;
                if d < 1.0 {
                    let s = 1.0 - d * d;
                    bowl = bowl.max(depth * s * s);
                }
                // The thrown-up lip.
                lip = lip.max(0.16 * depth * bump((d - 1.05).abs() / 0.45));
            }
            h += (lip - bowl) * (1.0 - kept);
        }
        h
    }
}

/// A lot is level this far out past its footprint (its apron), and eases into
/// the land by this far.
pub(super) const LOT_FLAT: f64 = 3.0;
pub(super) const LOT_EASE: f64 = 18.0;
