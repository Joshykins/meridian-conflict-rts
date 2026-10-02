//! A weather for each region of a map (`mc_data::regions`): on a map parted by climate
//! walls every region has a weather and an air mass of its own. The shaders pick cover,
//! rain and how towering the clouds are by region (`Atmosphere::region_sky`, `walls`;
//! regions.wgsl `sky_at`), the weather simulation keeps each region's cloud in its
//! region (clouds_sim.wgsl), and here storm cells form in a region as often as its
//! weather is stormy and die against its walls. The wind is region 0's over the
//! whole map.
//!
//! The walls are here for every shader that asks where a region ends, the ground's
//! and the sea's too (bindings.wgsl `climate_at`): they read them from `Atmosphere`.

use super::*;
use crate::gpu_consts::regions as consts;
use mc_data::regions::{Walls, MAX_REGIONS};

const _: () = assert!(consts::MAX as usize == MAX_REGIONS);
const _: () = assert!(consts::WALL_SEGMENTS as usize == Walls::MAX_SEGMENTS);
const _: () = assert!(consts::TIE_M == Walls::TIE_M);

/// The map's regions as the weather sees them.
pub(super) struct Regions {
    /// The climate walls between them (`Sky::set_walls`); none on a map without regions.
    walls: Walls,
    /// The share of the map each region covers.
    shares: [f32; MAX_REGIONS],
    /// Each region's weather, region 0's first (`Sky::set_weathers`). A region past
    /// its end plays the map's own, the sky's `weather`.
    weathers: Vec<Weather>,
}

impl Default for Regions {
    fn default() -> Regions {
        Regions::new(Walls::default(), Vec2::ONE)
    }
}

/// What `Atmosphere` carries of the regions.
pub(super) struct RegionUniforms {
    pub(super) region_sky: [[f32; 4]; consts::MAX as usize],
    pub(super) walls: [[f32; 4]; consts::WALL_SEGMENTS as usize],
    pub(super) wall_sides: [[f32; 4]; consts::WALL_SEGMENTS as usize],
    pub(super) regions: [f32; 4],
    /// The most towering of the regions' weathers: what bounds the cloud layer.
    pub(super) towering: f32,
}

impl Regions {
    /// `walls` on a map `size` metres across, every region in the map's own weather.
    fn new(walls: Walls, size: Vec2) -> Regions {
        Regions {
            shares: walls.shares(size.x, size.y),
            walls,
            weathers: Vec::new(),
        }
    }

    /// Region `region`'s weather, where `own` is the map's own (region 0's).
    fn weather(&self, region: usize, own: Weather) -> Weather {
        self.weathers.get(region).copied().unwrap_or(own)
    }

    /// The region `at` is in. None on a map without regions.
    pub(super) fn region_of(&self, at: Vec2) -> Option<usize> {
        (!self.walls.is_empty()).then(|| self.walls.region_at(at.x, at.y))
    }

    /// The weather over `at`, where `own` is the map's own.
    pub(super) fn weather_at(&self, at: Vec2, own: Weather) -> Weather {
        self.weather(self.walls.region_at(at.x, at.y), own)
    }

    /// How stormy the map is, taken as a whole (`Weather::storms`): each region's by
    /// its share of the map.
    pub(super) fn storms(&self, own: &Weather) -> f32 {
        (0..self.walls.regions())
            .map(|r| self.weather(r, *own).storms * self.shares[r])
            .sum()
    }

    /// How readily the rainiest of the map's weathers rains (`Weather::rain`).
    pub(super) fn rain(&self, own: &Weather) -> f32 {
        (0..self.walls.regions())
            .map(|r| self.weather(r, *own).rain)
            .fold(0.0, f32::max)
    }

    /// How much of its strength a storm keeps at `at`: one that formed in `region`
    /// dies away over its last metres to a wall and is gone at it (0). Any other
    /// storm (no regions, or one parked or called up) keeps it all.
    pub(super) fn storm_fade(&self, region: Option<usize>, at: Vec2) -> f32 {
        let Some(region) = region else {
            return 1.0;
        };
        let probe = self.walls.probe(at.x, at.y);
        if probe.region != region {
            return 0.0;
        }
        smoothstep(0.0, consts::STORM_FADE_M, probe.wall)
    }

    /// Where a new storm forms on a map `size` metres across: anywhere but its rim.
    /// On a map with regions, in a region as often as its weather is stormy, and
    /// not right at a wall, where it would die at once.
    pub(super) fn storm_site(&self, own: &Weather, size: Vec2, rng: &mut Rng) -> Vec2 {
        let margin = 0.1;
        let pick = |rng: &mut Rng| {
            Vec2::new(
                rng.range(margin, 1.0 - margin) * size.x,
                rng.range(margin, 1.0 - margin) * size.y,
            )
        };
        let mut pos = pick(rng);
        if self.walls.is_empty() {
            return pos;
        }
        let most = (0..self.walls.regions())
            .map(|r| self.weather(r, *own).storms)
            .fold(1e-3, f32::max);
        // A deliberate cap: after this many tries the last site stands, whichever
        // region it fell in (a storm there is weak, or fades at the wall).
        for _ in 0..32 {
            let probe = self.walls.probe(pos.x, pos.y);
            let stormy = self.weather(probe.region, *own).storms;
            if probe.wall > consts::STORM_FADE_M && rng.next() * most < stormy {
                break;
            }
            pos = pick(rng);
        }
        pos
    }

    pub(super) fn uniforms(&self, own: &Weather) -> RegionUniforms {
        let regions = self.walls.regions();
        let mut out = RegionUniforms {
            region_sky: [[0.0; 4]; consts::MAX as usize],
            walls: [[0.0; 4]; consts::WALL_SEGMENTS as usize],
            wall_sides: [[0.0; 4]; consts::WALL_SEGMENTS as usize],
            regions: [0.0; 4],
            towering: 0.0,
        };
        for (r, slot) in out.region_sky.iter_mut().enumerate().take(regions) {
            let w = self.weather(r, *own);
            *slot = [w.cover, w.towering, w.scale, w.rain];
            out.towering = out.towering.max(w.towering);
        }
        let segments = self.walls.segments();
        for (i, s) in segments.iter().enumerate() {
            out.walls[i] = [s.a.0, s.a.1, s.b.0, s.b.1];
            out.wall_sides[i] = [s.left as f32, s.right as f32, s.along, 1.0 / s.length()];
        }
        out.regions = [
            segments.len() as f32,
            regions as f32,
            out.towering,
            self.rain(own),
        ];
        out
    }
}

impl Sky {
    /// The map's climate walls (the renderer's `set_map_look`): the regions the
    /// weathers of `set_weathers` play in. The sky starts over if they changed.
    pub fn set_walls(&mut self, walls: Walls) {
        if self.regions.walls == walls {
            return;
        }
        let weathers = std::mem::take(&mut self.regions.weathers);
        self.regions = Regions {
            weathers,
            ..Regions::new(walls, self.map_size)
        };
        self.restart();
    }

    /// Plays the match in `weathers`, one for each of the map's regions, region 0's
    /// first (one weather: all over the map): new storms for them, and the sky
    /// started over. The wind is the first's.
    pub fn set_weathers(&mut self, weathers: &[Weather]) {
        self.weather = weathers.first().copied().unwrap_or_default();
        self.regions.weathers = weathers.to_vec();
        self.wind = self.wind.normalize_or(Vec2::X) * self.weather.wind.max(0.5);
        self.restart();
    }

    /// The sky started over in its weather, with that weather's storms already under way.
    fn restart(&mut self) {
        self.storms.clear();
        self.flashes.clear();
        // A match opens with its weather already under way.
        for _ in 0..self.target_storms() {
            let mut s = self.new_storm();
            s.age = self.rng.range(0.1, 0.6) * s.life;
            self.storms.push(s);
        }
        self.push_parked_storm();
        self.reset = true;
    }

    /// Where a new storm forms (`Regions::storm_site`).
    pub(super) fn storm_site(&mut self) -> Vec2 {
        self.regions
            .storm_site(&self.weather, self.map_size, &mut self.rng)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mc_data::weather::{MapConfig, SkyChoice, WeatherPreset};

    const SIZE: Vec2 = Vec2::splat(8000.0);

    fn regions_of(map: &str) -> Regions {
        let config = MapConfig::parse(map).unwrap();
        Regions {
            weathers: config.weathers(&SkyChoice::default()),
            ..Regions::new(config.walls().clone(), SIZE)
        }
    }

    /// Clear west of x = 4000, stormy east of it.
    fn two() -> Regions {
        regions_of(
            "(regions: [(name: \"West\", weather: Clear), (name: \"East\", weather: Stormy)],
              walls: [(line: [(4000, 0), (4000, 8000)], left: 0, right: 1)])",
        )
    }

    /// Clear west of x = 4000; east of it fair south of y = 4000 and stormy north.
    fn three() -> Regions {
        regions_of(
            "(regions: [(name: \"West\", weather: Clear), (name: \"South\", weather: Fair),
                        (name: \"North\", weather: Stormy)],
              walls: [(line: [(4000, 0), (4000, 4000)], left: 0, right: 1),
                      (line: [(4000, 4000), (4000, 8000)], left: 0, right: 2),
                      (line: [(4000, 4000), (8000, 4000)], left: 2, right: 1)])",
        )
    }

    #[test]
    fn each_region_has_its_own_weather() {
        let clear = Weather::from(WeatherPreset::Clear);
        let stormy = Weather::from(WeatherPreset::Stormy);
        let regions = two();
        assert_eq!(regions.weather_at(Vec2::new(1000.0, 500.0), clear), clear);
        assert_eq!(regions.weather_at(Vec2::new(7000.0, 500.0), clear), stormy);
        assert_eq!(regions.region_of(Vec2::new(7000.0, 500.0)), Some(1));
        // Half the map is as stormy as the east region, half not at all.
        let storms = regions.storms(&clear);
        assert!((storms - stormy.storms * 0.5).abs() < 0.05, "{storms}");
        let u = regions.uniforms(&clear);
        assert_eq!(
            u.region_sky[..2],
            [
                [clear.cover, clear.towering, clear.scale, clear.rain],
                [stormy.cover, stormy.towering, stormy.scale, stormy.rain]
            ]
        );
        assert_eq!(u.region_sky[2], [0.0; 4]);
        assert_eq!(u.walls[0], [4000.0, 0.0, 4000.0, 8000.0]);
        assert_eq!(u.wall_sides[0], [0.0, 1.0, 0.0, 1.0 / 8000.0]);
        // One wall segment between two regions; the tallest cloud and the most rain
        // of the two, which bound the cloud layer and say whether rain is drawn.
        assert_eq!(u.regions, [1.0, 2.0, stormy.towering, stormy.rain]);
        assert_eq!(u.towering, stormy.towering);
        assert_eq!((clear.rain, regions.rain(&clear)), (0.0, stormy.rain));
    }

    #[test]
    fn three_regions_each_their_own() {
        let [clear, fair, stormy] = [
            WeatherPreset::Clear,
            WeatherPreset::Fair,
            WeatherPreset::Stormy,
        ]
        .map(Weather::from);
        let regions = three();
        assert_eq!(regions.weather_at(Vec2::new(1000.0, 6000.0), clear), clear);
        assert_eq!(regions.weather_at(Vec2::new(6000.0, 1000.0), clear), fair);
        assert_eq!(regions.weather_at(Vec2::new(6000.0, 7000.0), clear), stormy);
        // Half the map clear, a quarter fair, a quarter stormy.
        let storms = regions.storms(&clear);
        let expect = fair.storms * 0.25 + stormy.storms * 0.25;
        assert!((storms - expect).abs() < 0.05, "{storms}");
        let u = regions.uniforms(&clear);
        assert_eq!(u.regions, [3.0, 3.0, stormy.towering, stormy.rain]);
        assert_eq!(
            u.region_sky[2],
            [stormy.cover, stormy.towering, stormy.scale, stormy.rain]
        );
        // The wall between the east's two: north on its left, south on its right,
        // and it starts its own count of metres.
        assert_eq!(u.walls[2], [4000.0, 4000.0, 8000.0, 4000.0]);
        assert_eq!(u.wall_sides[2], [2.0, 1.0, 0.0, 1.0 / 4000.0]);
        // Storms form where the weather is stormy: none in the clear west, more in
        // the stormy north than in the fair south, and none by a wall.
        let mut rng = Rng(0x5EED_1234_ABCD_0003);
        let mut formed = [0; 3];
        for _ in 0..600 {
            let at = regions.storm_site(&clear, SIZE, &mut rng);
            assert!(at.x > 4000.0 + consts::STORM_FADE_M, "{at}");
            assert!((at.y - 4000.0).abs() > consts::STORM_FADE_M, "{at}");
            formed[regions.region_of(at).unwrap()] += 1;
        }
        assert_eq!(formed[0], 0);
        assert!(formed[2] > formed[1] * 2 && formed[1] > 60, "{formed:?}");
        // A storm of the north dies at either of its walls, and is nothing in the south.
        let at = |x: f32, y: f32| Vec2::new(x, y);
        assert_eq!(regions.storm_fade(Some(2), at(6000.0, 6000.0)), 1.0);
        let near = regions.storm_fade(Some(2), at(6000.0, 4300.0));
        assert!(near > 0.0 && near < 1.0, "{near}");
        let near = regions.storm_fade(Some(2), at(4300.0, 6000.0));
        assert!(near > 0.0 && near < 1.0, "{near}");
        assert_eq!(regions.storm_fade(Some(2), at(6000.0, 3900.0)), 0.0);
        assert_eq!(regions.storm_fade(Some(2), at(3900.0, 6000.0)), 0.0);
    }

    #[test]
    fn one_weather_without_walls() {
        let fair = Weather::default();
        let stormy = Weather::from(WeatherPreset::Stormy);
        for regions in [
            Regions::default(),
            // A second weather is nothing without a region to play in.
            Regions {
                weathers: vec![fair, stormy],
                ..Regions::default()
            },
        ] {
            assert_eq!(regions.region_of(Vec2::new(7000.0, 500.0)), None);
            assert_eq!(regions.weather_at(Vec2::new(7000.0, 500.0), fair), fair);
            assert_eq!(regions.storms(&fair), fair.storms);
            assert_eq!(regions.storm_fade(None, Vec2::new(10.0, 10.0)), 1.0);
            assert_eq!(regions.rain(&fair), fair.rain);
            let u = regions.uniforms(&fair);
            // Region 0 repeats the map's own weather, and no wall is sent.
            assert_eq!(
                u.region_sky[0],
                [fair.cover, fair.towering, fair.scale, fair.rain]
            );
            assert_eq!(u.regions, [0.0, 1.0, fair.towering, fair.rain]);
            assert_eq!(u.towering, fair.towering);
        }
        // Walls with one weather for the whole map: every region plays it.
        let regions = Regions {
            weathers: vec![stormy],
            ..two()
        };
        assert_eq!(regions.weather_at(Vec2::new(7000.0, 500.0), stormy), stormy);
        assert_eq!(regions.storms(&stormy), stormy.storms);
        let u = regions.uniforms(&stormy);
        assert_eq!(u.region_sky[0], u.region_sky[1]);
        assert_eq!(u.regions[..2], [1.0, 2.0]);
    }

    #[test]
    fn storms_form_in_the_stormy_region() {
        let clear = Weather::from(WeatherPreset::Clear);
        let regions = two();
        let mut rng = Rng(0x5EED_1234_ABCD_0001);
        for _ in 0..200 {
            let at = regions.storm_site(&clear, SIZE, &mut rng);
            // East of the wall (the west's weather has no storms), clear of it,
            // and inside the map's rim.
            assert!(at.x > 4000.0 + consts::STORM_FADE_M, "{at}");
            assert!(at.x <= 7200.0 && at.y >= 800.0 && at.y <= 7200.0, "{at}");
        }
        // Both regions stormy: storms in both, more in the stormier.
        let fair = Weather::from(WeatherPreset::Fair);
        let both = Regions {
            weathers: vec![fair, Weather::from(WeatherPreset::Stormy)],
            ..two()
        };
        let east = (0..400)
            .filter(|_| both.storm_site(&fair, SIZE, &mut rng).x > 4000.0)
            .count();
        assert!((240..360).contains(&east), "{east} of 400 east");
        // With one weather over the map a site costs the two draws it always did.
        let (mut a, mut b) = (Rng(77), Rng(77));
        let at = Regions::default().storm_site(&fair, SIZE, &mut a);
        let same = Vec2::new(b.range(0.1, 0.9) * SIZE.x, b.range(0.1, 0.9) * SIZE.y);
        assert_eq!((at, a.next()), (same, b.next()));
    }

    #[test]
    fn a_storm_dies_at_the_wall() {
        let regions = two();
        let at = |x: f32| Vec2::new(x, 3000.0);
        // An east-region storm: whole well east, fading over its last 600 m, gone at the wall.
        assert_eq!(regions.storm_fade(Some(1), at(6000.0)), 1.0);
        let near = regions.storm_fade(Some(1), at(4300.0));
        assert!(near > 0.0 && near < 1.0, "{near}");
        assert_eq!(regions.storm_fade(Some(1), at(3900.0)), 0.0);
        // A west-region one the other way, and a parked or conjured storm never.
        assert_eq!(regions.storm_fade(Some(0), at(1000.0)), 1.0);
        assert_eq!(regions.storm_fade(Some(0), at(4100.0)), 0.0);
        assert_eq!(regions.storm_fade(None, at(4000.0)), 1.0);
    }
}
