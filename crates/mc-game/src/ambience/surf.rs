//! The breakers heard where and when they are seen to break. A few points of the
//! shore round the focus are found by walking out from it over the ground, and
//! each one's breakers are timed with `mc_render::shore`, the CPU copy of the
//! sums shore.wgsl draws them by: a wave's crash is played so its loud part
//! lands as the wave breaks, and close up the wash as its foam reaches the sand.

use glam::Vec2;
use mc_render::gpu_consts::surf;
use mc_render::shore;

/// Seconds into `wave_break` at which its crash starts (data/sounds/ambience.ron).
pub(super) const CRASH_AT: f32 = 1.0;
/// Seconds into `wave_wash` at which the foam is at the waterline.
pub(super) const WASH_AT: f32 = 0.1;

/// The rate a breaker's sounds play at: the same wave bigger is lower (and, played
/// slower, longer), never a different sound.
pub(super) fn pitch(size: f32) -> f32 {
    1.06 - 0.14 * size.clamp(0.0, 1.2)
}

/// Directions walked out from the focus, and samples along each.
const RAYS: usize = 12;
const STEPS: usize = 6;
/// Shore points followed at once: the nearest this many, spread out.
const MOST: usize = 4;
/// Seconds between looks for the shore.
const REFRESH: f32 = 0.5;
/// Seconds ahead a breaker is handed to the mixer (with a delay to its moment), so
/// it is never late by the frame it falls in.
const AHEAD: f32 = 0.15;
/// Most breakers ahead looked at per point: on the flattest shelf a big wave breaks
/// about 60 s (8 periods) before it lands; a deliberate cap, as no shore is flatter.
const MOST_AHEAD: f32 = 10.0;

/// A point of the shore being listened to.
struct Spot {
    /// At the waterline.
    xy: Vec2,
    /// The point its breakers are worked out for (`shore::surf_point`).
    at: Vec2,
    /// Rise per metre of the bed the breakers roll in over there (`shore::bed`: the
    /// real bed's, but never steeper than `SURF_MAX_SLOPE`).
    slope: f32,
}

/// A breaker (or its wash) to play.
#[derive(Clone, Copy, Debug)]
pub(super) struct Hit {
    pub(super) xy: Vec2,
    /// About 1 on the open coast (the climate included), as `shore::Breaker::size`.
    pub(super) size: f32,
    /// The wash up the beach, not the crash.
    pub(super) wash: bool,
    /// On the renderer's clock: when the wave breaks (or lands, for a wash). Only
    /// the tests read it: the sound needs only `start`.
    #[cfg(test)]
    at: f32,
    /// When its sound starts, so that its crash (or wash) comes at `at`.
    pub(super) start: f32,
}

#[derive(Default)]
pub(super) struct Surf {
    spots: Vec<Spot>,
    refresh_in: f32,
    /// The clock up to which breakers have been handed out.
    heard_to: Option<f32>,
}

impl Surf {
    /// Finds the shore round `focus` now and then, and puts into `out` every breaker
    /// (and every wash) whose sound starts from the last call up to `AHEAD` past
    /// `clock`, the crash that much before the wave breaks. `reach` is how far out to
    /// look, `ground` the terrain's height and `surf_at` the point a shore point's
    /// breakers are worked out for (`Renderer::surf_point`), `sea` the water level,
    /// `climate` `shore::climate_scale`.
    #[expect(
        clippy::too_many_arguments,
        reason = "presentation: the ambience's inputs, read once a frame"
    )]
    pub(super) fn step(
        &mut self,
        (ground, surf_at): (&dyn Fn(Vec2) -> f32, &dyn Fn(Vec2) -> Vec2),
        sea: f32,
        focus: Vec2,
        reach: f32,
        clock: f32,
        climate: f32,
        dt: f32,
        out: &mut Vec<Hit>,
    ) {
        self.refresh_in -= dt;
        if self.refresh_in <= 0.0 {
            self.refresh_in = REFRESH;
            self.spots = find(ground, surf_at, sea, focus, reach);
        }
        let to = clock + AHEAD;
        // The first call, or the clock jumped (a new match, a reset): nothing is owed.
        let from = match self.heard_to {
            Some(t) if t <= to && t > clock - 1.0 => t,
            _ => clock,
        };
        self.heard_to = Some(to);
        for spot in &self.spots {
            breakers(spot, clock, climate, from, to, out);
        }
    }
}

/// The crashes and washes at `spot` whose moment falls in `(from, to]`.
fn breakers(spot: &Spot, clock: f32, climate: f32, from: f32, to: f32, out: &mut Vec<Hit>) {
    let next = shore::next_breaker(spot.at, spot.slope, clock, climate);
    // A wave's crash may come several waves before it lands on a flat shelf, and
    // a bigger one breaks further out, so each wave in reach is looked at.
    let deepest = surf::HEIGHT * climate * surf::BREAK_RATIO;
    let ahead = (shore::travel(deepest, spot.slope) / surf::PERIOD)
        .ceil()
        .min(MOST_AHEAD) as i32
        + 1;
    for k in -1..=ahead {
        let wave = next.wave + k as f32;
        let size = shore::size(wave, spot.at) * climate;
        // As `shore::next_breaker` has it: it lands when its crest reaches the
        // waterline, and breaks in water as deep as it is high times the ratio, the
        // swell's travel time from there before.
        let lands = shore::crest_time(spot.at, wave, 0.0);
        let breaks = shore::crest_time(
            spot.at,
            wave,
            shore::travel(surf::HEIGHT * size * surf::BREAK_RATIO, spot.slope),
        );
        // A section that rolls in unbroken (`shore::breaking`) is heard only washing up.
        let crashes = shore::breaking(wave, spot.at) > 0.3;
        for (at, wash, lead) in [(breaks, false, CRASH_AT), (lands, true, WASH_AT)] {
            if !wash && !crashes {
                continue;
            }
            let start = at - lead / pitch(size);
            if start > from && start <= to {
                out.push(Hit {
                    xy: spot.xy,
                    size,
                    wash,
                    #[cfg(test)]
                    at,
                    start,
                });
            }
        }
    }
}

/// The nearest few points of the waterline round `focus`, each at least a quarter of
/// `reach` (and 60 m) from the others: walks out along `RAYS` directions and takes
/// the first place each crosses the water level, then measures the bed there.
fn find(
    ground: &dyn Fn(Vec2) -> f32,
    surf_at: &dyn Fn(Vec2) -> Vec2,
    sea: f32,
    focus: Vec2,
    reach: f32,
) -> Vec<Spot> {
    let depth = |xy: Vec2| sea - ground(xy);
    let centre = depth(focus);
    let mut found: Vec<(f32, Vec2)> = Vec::new();
    for ray in 0..RAYS {
        let dir = Vec2::from_angle(ray as f32 / RAYS as f32 * std::f32::consts::TAU + 0.13);
        let (mut was, mut was_r) = (centre, 0.0);
        for step in 1..=STEPS {
            let r = reach * step as f32 / STEPS as f32;
            let d = depth(focus + dir * r);
            if (d > 0.0) != (was > 0.0) {
                // Where the ground crosses the water, taken straight between the two
                // samples, then once more from a sample there.
                let r0 = was_r + (r - was_r) * was / (was - d);
                let d0 = depth(focus + dir * r0);
                let r1 = if (d0 > 0.0) == (was > 0.0) {
                    r0 + (r - r0) * d0 / (d0 - d)
                } else {
                    was_r + (r0 - was_r) * was / (was - d0)
                };
                found.push((r1, focus + dir * r1));
                break;
            }
            (was, was_r) = (d, r);
        }
    }
    found.sort_by(|a, b| a.0.total_cmp(&b.0));
    let apart = (reach * 0.25).max(60.0);
    let mut spots: Vec<Spot> = Vec::new();
    for (_, xy) in found {
        if spots.len() == MOST {
            break;
        }
        if spots.iter().all(|s| s.xy.distance(xy) >= apart) {
            let bed = shore::shore_at(ground, sea, xy);
            spots.push(Spot {
                xy,
                at: surf_at(xy),
                slope: shore::bed(0.0, bed.slope).1,
            });
        }
    }
    spots
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A straight beach along x = 1472, the sea to the east, the bed rising 3 in 100.
    fn beach(xy: Vec2) -> f32 {
        (1472.0 - xy.x) * 0.03
    }

    /// A region's own surf, as `shore::surf_point` has it for region 2.
    fn far_off(xy: Vec2) -> Vec2 {
        shore::surf_point(2, xy)
    }

    fn listen(ground: &dyn Fn(Vec2) -> f32, focus: Vec2, seconds: f32) -> Vec<(f32, Hit)> {
        listen_in(ground, &|xy| xy, focus, seconds)
    }

    fn listen_in(
        ground: &dyn Fn(Vec2) -> f32,
        surf_at: &dyn Fn(Vec2) -> Vec2,
        focus: Vec2,
        seconds: f32,
    ) -> Vec<(f32, Hit)> {
        let mut surf = Surf::default();
        let mut hits = Vec::new();
        let dt = 1.0 / 30.0;
        for f in 0..(seconds / dt) as usize {
            let clock = 500.0 + f as f32 * dt;
            let mut out = Vec::new();
            surf.step(
                (ground, surf_at),
                0.0,
                focus,
                300.0,
                clock,
                1.0,
                dt,
                &mut out,
            );
            hits.extend(out.into_iter().map(|h| (clock, h)));
        }
        hits
    }

    #[test]
    fn breakers_are_heard_on_a_shore_as_they_break_and_not_at_sea_or_inland() {
        let hits = listen(&beach, Vec2::new(1400.0, 1024.0), 120.0);
        let crashes: Vec<_> = hits.iter().filter(|h| !h.1.wash).collect();
        // Up to four points along the beach, a breaker every 7.5 s at each, less the
        // sections that roll in unbroken (`shore::breaking`) and are not heard crashing.
        assert!(
            (20..=70).contains(&crashes.len()),
            "{} crashes in two minutes",
            crashes.len()
        );
        for (clock, h) in &hits {
            assert!((h.xy.x - 1472.0).abs() < 2.0, "{h:?} is off the waterline");
            // Handed out no later than it starts, and no more than `AHEAD` early.
            assert!(
                h.start >= clock - 1e-3 && h.start <= clock + AHEAD + 1e-3,
                "{h:?} at {clock}"
            );
            let lead = if h.wash { WASH_AT } else { CRASH_AT };
            assert!((h.at - h.start - lead / pitch(h.size)).abs() < 1e-3);
        }
        // A crash for every breaking wave at each point: none twice. (On a shelf a
        // big wave breaks further out, so two may break close together.)
        let mut at_spot: std::collections::BTreeMap<i32, Vec<(f32, f32)>> = Default::default();
        for (_, h) in &crashes {
            at_spot
                .entry(h.xy.y.round() as i32)
                .or_default()
                .push((h.at, h.size));
        }
        for waves in at_spot.values() {
            assert!((8..=18).contains(&waves.len()), "{waves:?}");
            for (i, a) in waves.iter().enumerate() {
                assert!(
                    waves[i + 1..].iter().all(|b| b.1 != a.1),
                    "a wave twice: {waves:?}"
                );
            }
        }
        assert!(listen(&|_| -30.0, Vec2::new(1400.0, 1024.0), 30.0).is_empty());
        assert!(listen(&|_| 30.0, Vec2::new(1400.0, 1024.0), 30.0).is_empty());
    }

    /// shore.wgsl draws crest m at depth d when (time + travel(d) + lag) / PERIOD = m,
    /// over the breakers' bed (`shore::bed`, never steeper than `SURF_MAX_SLOPE`); a
    /// breaker breaks at the depth its height times the ratio. Every crash heard is at
    /// such a moment, and every wash at the moment its crest reaches the waterline, on
    /// a beach and under a cliff alike.
    /// On a map with regions each region's breakers are worked out for a point of its
    /// own (shore.wgsl `surf_point`): the crashes heard there are that point's.
    #[test]
    fn the_crash_is_heard_when_the_shader_breaks_the_wave() {
        let cliff = |xy: Vec2| (1472.0 - xy.x) * 0.6;
        let here = |xy: Vec2| xy;
        for (ground, surf_at) in [
            (
                &beach as &dyn Fn(Vec2) -> f32,
                &here as &dyn Fn(Vec2) -> Vec2,
            ),
            (&cliff, &here),
            (&beach, &far_off),
        ] {
            let hits = listen_in(ground, surf_at, Vec2::new(1440.0, 1024.0), 60.0);
            assert!(hits.iter().any(|h| h.1.wash) && hits.iter().any(|h| !h.1.wash));
            for (_, h) in hits {
                let slope = shore::bed(0.0, shore::shore_at(ground, 0.0, h.xy).slope).1;
                assert!(slope <= surf::MAX_SLOPE);
                let deep = if h.wash {
                    0.0
                } else {
                    surf::HEIGHT * h.size * surf::BREAK_RATIO
                };
                let at = surf_at(h.xy);
                let p = (h.at + shore::travel(deep, slope) + shore::lag(at, h.at)) / surf::PERIOD;
                assert!(
                    (p - p.round()).abs() * surf::PERIOD < 1.0 / 30.0,
                    "{h:?} is {:.3} s off its crest",
                    (p - p.round()) * surf::PERIOD
                );
                assert!((shore::size(p.round(), at) - h.size).abs() < 1e-4, "{h:?}");
            }
        }
    }
}
