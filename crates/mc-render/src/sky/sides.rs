//! Two weathers over one map: on a map split by a climate divide
//! (`mc_data::weather::ClimateDivide`) the map's own weather holds west of the line and
//! another east of it. The shaders pick cover, rain and how towering the clouds are by
//! side (`Atmosphere::east`, `divide`; bindings.wgsl `sky_at`), the weather simulation
//! keeps each side's cloud on its side (clouds_sim.wgsl), and here storm cells form on
//! a side by how stormy its weather is and die against the line. The wind is the
//! map's own on both sides.

use super::*;
use crate::gpu_consts::divide as consts;
use mc_data::weather::ClimateDivide;

const _: () = assert!(consts::POINTS as usize == ClimateDivide::MAX_POINTS);

/// A divide's line as the shaders hold it (`Globals::divide`, `Atmosphere::divide`):
/// its points, and how many (0 with no divide).
pub(crate) fn divide_points(
    divide: Option<&ClimateDivide>,
) -> ([[f32; 4]; consts::POINTS as usize], f32) {
    let mut points = [[0.0; 4]; consts::POINTS as usize];
    let Some(divide) = divide else {
        return (points, 0.0);
    };
    for (slot, p) in points.iter_mut().zip(&divide.line) {
        *slot = [p.0, p.1, 0.0, 0.0];
    }
    (points, divide.line.len().min(points.len()) as f32)
}

/// The map's climate divide as the weather sees it. The weather is parted only with
/// both a line and a weather east of it: a preset picked in skirmish set-up plays over
/// the whole map, line or no line.
#[derive(Default)]
pub(super) struct Sides {
    /// The line (`Sky::set_divide`).
    divide: Option<ClimateDivide>,
    /// The weather east of it (`Sky::set_weather_sides`).
    east: Option<Weather>,
}

/// What `Atmosphere` carries of the two sides.
pub(super) struct SideUniforms {
    /// `Atmosphere::east`: cover, towering, cloud mass size, rain.
    pub(super) east: [f32; 4],
    pub(super) divide: [[f32; 4]; consts::POINTS as usize],
    pub(super) divide_info: [f32; 4],
    /// The more towering of the two weathers: what bounds the cloud layer.
    pub(super) towering: f32,
}

impl Sides {
    /// The line and the weather east of it, on a map the weather is parted over.
    fn parted(&self) -> Option<(&ClimateDivide, Weather)> {
        Some((self.divide.as_ref()?, self.east?))
    }

    /// Which side of the line `at` is on: east (true) or west. None with one weather
    /// over the whole map.
    pub(super) fn side_of(&self, at: Vec2) -> Option<bool> {
        self.parted().map(|(d, _)| d.east_of(at.x, at.y))
    }

    /// The weather over `at`, where `west` is the map's own.
    pub(super) fn weather_at(&self, at: Vec2, west: Weather) -> Weather {
        match self.parted() {
            Some((d, east)) if d.east_of(at.x, at.y) => east,
            _ => west,
        }
    }

    /// How stormy a map `size` metres across is, taken as a whole (`Weather::storms`):
    /// each side's by its share of the map.
    pub(super) fn storms(&self, west: &Weather, size: Vec2) -> f32 {
        match self.parted() {
            Some((d, east)) => {
                let share = d.east_share(size.x, size.y);
                west.storms * (1.0 - share) + east.storms * share
            }
            None => west.storms,
        }
    }

    /// How readily the rainier of the map's weathers rains (`Weather::rain`).
    pub(super) fn rain(&self, west: &Weather) -> f32 {
        self.parted()
            .map_or(west.rain, |(_, east)| west.rain.max(east.rain))
    }

    /// How much of its strength a storm keeps at `at`: one that formed on `side` of the
    /// line dies away over its last metres to it and is gone at it (0). Any other
    /// storm (no divide, or one parked or called up) keeps it all.
    pub(super) fn storm_fade(&self, side: Option<bool>, at: Vec2) -> f32 {
        let (Some(east), Some((d, _))) = (side, self.parted()) else {
            return 1.0;
        };
        let across = d.east_distance(at.x, at.y);
        smoothstep(
            0.0,
            consts::STORM_FADE_M,
            if east { across } else { -across },
        )
    }

    /// Where a new storm forms on a map `size` metres across: anywhere but its rim.
    /// On a map with two weathers, on a side as often as its weather is stormy, and
    /// not right at the line, where it would die at once.
    pub(super) fn storm_site(&self, west: &Weather, size: Vec2, rng: &mut Rng) -> Vec2 {
        let margin = 0.1;
        let pick = |rng: &mut Rng| {
            Vec2::new(
                rng.range(margin, 1.0 - margin) * size.x,
                rng.range(margin, 1.0 - margin) * size.y,
            )
        };
        let mut pos = pick(rng);
        let Some((divide, east)) = self.parted() else {
            return pos;
        };
        let most = west.storms.max(east.storms).max(1e-3);
        // A deliberate cap: after this many tries the last site stands, whichever
        // side it fell on (a storm there is weak, or fades at the line).
        for _ in 0..32 {
            let across = divide.east_distance(pos.x, pos.y);
            let stormy = if across >= 0.0 {
                east.storms
            } else {
                west.storms
            };
            if across.abs() > consts::STORM_FADE_M && rng.next() * most < stormy {
                break;
            }
            pos = pick(rng);
        }
        pos
    }

    pub(super) fn uniforms(&self, west: &Weather) -> SideUniforms {
        let east = self.parted().map_or(*west, |(_, e)| e);
        let (divide, points) = divide_points(self.parted().map(|(d, _)| d));
        SideUniforms {
            east: [east.cover, east.towering, east.scale, east.rain],
            divide,
            divide_info: [points, 0.0, 0.0, 0.0],
            towering: west.towering.max(east.towering),
        }
    }
}

impl Sky {
    /// The map's climate divide (the renderer's `set_map_look`): the line that parts the
    /// weather when `set_weather_sides` has given the east side its own.
    pub fn set_divide(&mut self, divide: Option<ClimateDivide>) {
        if self.sides.divide == divide {
            return;
        }
        self.sides.divide = divide;
        if self.sides.east.is_some() {
            self.restart();
        }
    }

    /// Plays the match in `west` west of the map's climate divide and `east` east of
    /// it, or in `west` everywhere with no `east`: new storms for it, and the sky
    /// started over.
    pub fn set_weather_sides(&mut self, west: Weather, east: Option<Weather>) {
        self.weather = west;
        self.sides.east = east;
        self.wind = self.wind.normalize_or(Vec2::X) * west.wind.max(0.5);
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

    /// Where a new storm forms (`Sides::storm_site`).
    pub(super) fn storm_site(&mut self) -> Vec2 {
        self.sides
            .storm_site(&self.weather, self.map_size, &mut self.rng)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mc_data::weather::{MapConfig, WeatherPreset};

    fn parted() -> Sides {
        let config = MapConfig::parse(
            "(weather: Clear, divide: (line: [(4000, 0), (4000, 8000)], weather: Stormy))",
        )
        .unwrap();
        Sides {
            east: config.east_weather(None),
            divide: config.divide,
        }
    }

    #[test]
    fn each_side_has_its_own_weather() {
        let clear = Weather::from(WeatherPreset::Clear);
        let stormy = Weather::from(WeatherPreset::Stormy);
        let sides = parted();
        assert_eq!(sides.weather_at(Vec2::new(1000.0, 500.0), clear), clear);
        assert_eq!(sides.weather_at(Vec2::new(7000.0, 500.0), clear), stormy);
        assert_eq!(sides.side_of(Vec2::new(7000.0, 500.0)), Some(true));
        // Half the map is as stormy as the east side, half not at all.
        let storms = sides.storms(&clear, Vec2::splat(8000.0));
        assert!((storms - stormy.storms * 0.5).abs() < 0.05, "{storms}");
        let u = sides.uniforms(&clear);
        assert_eq!(
            u.east,
            [stormy.cover, stormy.towering, stormy.scale, stormy.rain]
        );
        assert_eq!(
            (u.divide_info[0], u.divide[1]),
            (2.0, [4000.0, 8000.0, 0.0, 0.0])
        );
        assert_eq!(u.towering, stormy.towering);
        // Rain is drawn if either side has any.
        assert_eq!((clear.rain, sides.rain(&clear)), (0.0, stormy.rain));
    }

    #[test]
    fn one_weather_without_a_line_or_an_east_side() {
        let fair = Weather::default();
        for sides in [
            Sides::default(),
            Sides {
                east: None,
                ..parted()
            },
            Sides {
                divide: None,
                ..parted()
            },
        ] {
            assert_eq!(sides.side_of(Vec2::new(7000.0, 500.0)), None);
            assert_eq!(sides.weather_at(Vec2::new(7000.0, 500.0), fair), fair);
            assert_eq!(sides.storms(&fair, Vec2::splat(8000.0)), fair.storms);
            assert_eq!(sides.storm_fade(Some(true), Vec2::new(10.0, 10.0)), 1.0);
            assert_eq!(sides.rain(&fair), fair.rain);
            let u = sides.uniforms(&fair);
            // The east side repeats the map's own weather, and no line is sent.
            assert_eq!(u.east, [fair.cover, fair.towering, fair.scale, fair.rain]);
            assert_eq!((u.divide_info[0], u.towering), (0.0, fair.towering));
        }
    }

    #[test]
    fn storms_form_on_the_stormy_side() {
        let clear = Weather::from(WeatherPreset::Clear);
        let size = Vec2::splat(8000.0);
        let sides = parted();
        let mut rng = Rng(0x5EED_1234_ABCD_0001);
        for _ in 0..200 {
            let at = sides.storm_site(&clear, size, &mut rng);
            // East of the line (the west's weather has no storms), clear of it,
            // and inside the map's rim.
            assert!(at.x > 4000.0 + consts::STORM_FADE_M, "{at}");
            assert!(at.x <= 7200.0 && at.y >= 800.0 && at.y <= 7200.0, "{at}");
        }
        // Both sides stormy: storms on both, more on the stormier.
        let fair = Weather::from(WeatherPreset::Fair);
        let east = (0..400)
            .filter(|_| sides.storm_site(&fair, size, &mut rng).x > 4000.0)
            .count();
        assert!((240..360).contains(&east), "{east} of 400 east");
        // With one weather over the map a site costs the two draws it always did.
        let (mut a, mut b) = (Rng(77), Rng(77));
        let at = Sides::default().storm_site(&fair, size, &mut a);
        let same = Vec2::new(b.range(0.1, 0.9) * size.x, b.range(0.1, 0.9) * size.y);
        assert_eq!((at, a.next()), (same, b.next()));
    }

    #[test]
    fn a_storm_dies_at_the_line() {
        let sides = parted();
        let at = |x: f32| Vec2::new(x, 3000.0);
        // An east-side storm: whole well east, fading over its last 600 m, gone at the line.
        assert_eq!(sides.storm_fade(Some(true), at(6000.0)), 1.0);
        let near = sides.storm_fade(Some(true), at(4300.0));
        assert!(near > 0.0 && near < 1.0, "{near}");
        assert_eq!(sides.storm_fade(Some(true), at(3900.0)), 0.0);
        // A west-side one the other way, and a parked or conjured storm never.
        assert_eq!(sides.storm_fade(Some(false), at(1000.0)), 1.0);
        assert_eq!(sides.storm_fade(Some(false), at(4100.0)), 0.0);
        assert_eq!(sides.storm_fade(None, at(4000.0)), 1.0);
    }
}
