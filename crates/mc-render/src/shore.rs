//! When and where the breakers drawn by shore.wgsl break and land, worked out on
//! the CPU with the very same sums, so the ambience can play a wave's crash as
//! it is seen to break. shore.wgsl says how the waves work; a change to one of
//! these sums is made there too.

use glam::Vec2;
use mc_models::gpu_consts::surf;

/// The seabed round a point, as the shader measures it.
#[derive(Clone, Copy, Debug)]
pub struct Shore {
    /// Metres of water over the bed; on land, minus the ground's height above it.
    pub depth: f32,
    /// Rise per metre of the bed, and the way up it (toward the shore).
    pub slope: f32,
    pub up: Vec2,
}

/// `height` is the ground's height at a point (`Renderer::ground_height`), `water` the sea level.
pub fn shore_at(height: impl Fn(Vec2) -> f32, water: f32, xy: Vec2) -> Shore {
    let r = surf::SLOPE_REACH;
    let gx = height(xy + Vec2::new(r, 0.0)) - height(xy - Vec2::new(r, 0.0));
    let gy = height(xy + Vec2::new(0.0, r)) - height(xy - Vec2::new(0.0, r));
    let g = Vec2::new(gx, gy) / (2.0 * r);
    let slope = g.length();
    Shore {
        depth: water - height(xy),
        slope,
        up: g / slope.max(0.0001),
    }
}

/// The breakers' size on a map's water: a canyon lake, a tropical shore or the open coast.
pub fn climate_scale(desert: bool, tropical: bool) -> f32 {
    if desert {
        surf::DESERT
    } else if tropical {
        surf::TROPICAL
    } else {
        1.0
    }
}

/// The bed the breakers roll in over at a point `depth` metres deep on a bed
/// rising at `slope`: (depth, slope) as far out as the real one, never steeper than
/// `SURF_MAX_SLOPE`.
pub fn bed(depth: f32, slope: f32) -> (f32, f32) {
    let virtual_slope = slope.clamp(surf::MIN_SLOPE, surf::MAX_SLOPE);
    (
        depth / slope.max(surf::MIN_SLOPE) * virtual_slope,
        virtual_slope,
    )
}

/// Seconds the swell at `depth` takes to reach the waterline over a bed rising at `slope`.
pub fn travel(depth: f32, slope: f32) -> f32 {
    2.0 * (depth.max(0.0) / 9.81).sqrt() / slope.max(surf::MIN_SLOPE)
}

/// Seconds by which the breakers at `xy` run ahead of the ones further along the shore.
pub fn lag(xy: Vec2, time: f32) -> f32 {
    1.4 * (xy.dot(Vec2::new(0.0061, 0.0023)) + time * 0.021).sin()
        + 0.9 * (xy.dot(Vec2::new(-0.0027, 0.0074)) + 1.7).sin()
        + 0.4 * (xy.dot(Vec2::new(0.0152, -0.0101)) + time * 0.05).sin()
}

/// The size of breaker `m` at `xy`, about 1, before the climate scales it.
pub fn size(m: f32, xy: Vec2) -> f32 {
    let set = 0.62 + 0.38 * (m * 0.93 + 0.4).sin();
    let stretch = 0.72 + 0.28 * (xy.dot(Vec2::new(0.0113, 0.0041)) + m * 1.7).sin();
    set * stretch
}

/// One breaker at a point of the shore.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Breaker {
    /// Which wave it is (the same number for its whole run in, anywhere along the shore).
    pub wave: f32,
    /// Its size, about 1 on the open coast (the climate included).
    pub size: f32,
    /// When it breaks: its crest reaches water `SURF_BREAK_RATIO` times as deep as it is high.
    pub breaks: f32,
    /// When its white water reaches the waterline and the wash starts up the beach.
    pub lands: f32,
}

/// The next breaker to land at `xy`, a point at the waterline (where `lag` is
/// read), on a shore whose bed rises at `slope`. `time` is the renderer's clock
/// (`Renderer::time`). `climate` is `climate_scale`.
pub fn next_breaker(xy: Vec2, slope: f32, time: f32, climate: f32) -> Breaker {
    // At the waterline travel is 0: wave m lands when (time + lag) / PERIOD = m.
    let p = (time + lag(xy, time)) / surf::PERIOD;
    let wave = p.floor() + 1.0;
    let lands = time + (wave - p) * surf::PERIOD;
    let size = size(wave, xy) * climate;
    let breaks_at_depth = surf::HEIGHT * size * surf::BREAK_RATIO;
    Breaker {
        wave,
        size,
        // (Over the breakers' bed, `bed`: never steeper than `SURF_MAX_SLOPE`.)
        breaks: lands
            - travel(
                breaks_at_depth,
                slope.clamp(surf::MIN_SLOPE, surf::MAX_SLOPE),
            ),
        lands,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_breaker_breaks_before_it_lands_and_the_next_one_is_a_period_on() {
        let xy = Vec2::new(1200.0, 3400.0);
        let a = next_breaker(xy, 0.05, 100.0, 1.0);
        assert!(a.breaks < a.lands && a.lands >= 100.0);
        assert!(
            a.lands - a.breaks < 30.0,
            "a gentle beach breaks in seconds, not minutes"
        );
        let b = next_breaker(xy, 0.05, a.lands + 0.01, 1.0);
        assert_eq!(b.wave, a.wave + 1.0);
        assert!((b.lands - a.lands - surf::PERIOD).abs() < 0.5);
    }

    #[test]
    fn the_swell_slows_as_it_shoals_and_a_steep_shore_takes_it_at_once() {
        assert!(travel(4.0, 0.05) > travel(1.0, 0.05));
        assert!(travel(1.0, 0.5) < travel(1.0, 0.05));
        assert_eq!(travel(-1.0, 0.05), 0.0);
    }
}
