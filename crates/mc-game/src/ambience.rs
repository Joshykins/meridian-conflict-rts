//! The living world heard around the camera: wind that swells and drops in
//! gusts, leaves and needles rustling where the forest is, surf along the
//! shore and the swell out at sea; birds calling by day, crickets, frogs and
//! the odd owl by night. The recipes are `data/sounds/ambience.ron`.
//!
//! What is heard comes from where the camera looks. At load a coarse habitat
//! map is made from the map on a background thread (`Habitat`: how closed the
//! forest is, how much of it conifer, open water, wet lowland), and each
//! frame a ring of points round the focus, as wide as the view, is read from
//! it: how much forest and water is about, and on which side. Over that come
//! the sky's cues (how dark it is, the wind, the rain, the climate) and the
//! battle. A fight is no place for birdsong: the battle as it is heard (the shots,
//! hits and deaths the mix plays, at the level it plays them) all but silences
//! the world. Birds, insects and frogs stop; leaves, surf and waves duck to
//! nearly nothing; only a low wind is left. A nuclear blast anywhere on the map
//! hushes it for most of a minute. After the guns stop the world holds its breath
//! a few seconds and creeps back over twenty or so, the insects before the birds.
//!
//! It is detail: everything fades as the camera climbs, and from strategic
//! zoom only a faint high wind is left. All of it plays on the weather volume.
//! The beds are loops handed to `Audio::set_weather_loops` with the rain's
//! (`Game::battle_sounds` merges them); the calls are one-shots, scattered at
//! random times, places and pitches, some repeated or answered. The waves are
//! not scattered: each breaker is heard crashing where and when it is seen to
//! break, and from close by its foam washing up the sand (`surf`).

mod surf;

use crate::audio::Audio;
use glam::{Vec2, Vec3};
use mc_data::{SoundId, SoundLibrary};
use mc_map::MapFile;
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;

/// Everything here against the rest of the weather volume: the rain loops
/// peak near this level at a moderate shower seen from close by.
const LEVEL: f32 = 1.0;

/// What the ambience listens to each frame, besides the map.
#[derive(Clone, Copy)]
pub struct Cues {
    pub focus: Vec3,
    /// The camera's distance from its focus, metres: how far out it is zoomed.
    pub distance: f32,
    pub yaw: f32,
    /// 0 in daylight to 1 at night (the sky's `darkness`).
    pub darkness: f32,
    /// The wind over the ground, metres a second.
    pub wind: f32,
    /// How hard it rains where the camera looks, 0 to 1.
    pub rain: f32,
    pub tropical: bool,
    /// Canyon-country desert: ravens and wrens instead of songbirds, no frogs,
    /// sparse crickets, and a sheltered lake's lapping instead of open swell.
    pub desert: bool,
    /// The water level, metres.
    pub sea: f32,
    /// The clock the shaders animate by (`Renderer::time`), which times the breakers.
    pub clock: f32,
}

/// The ambience's sounds by name, looked up again whenever the library changes.
struct Ids {
    generation: u32,
    wind_open: Option<SoundId>,
    wind_high: Option<SoundId>,
    leaves: Option<SoundId>,
    needles: Option<SoundId>,
    surf: Option<SoundId>,
    sea: Option<SoundId>,
    crickets: Option<SoundId>,
    night_tropical: Option<SoundId>,
    frogs: Option<SoundId>,
    wave_break: Option<SoundId>,
    wave_wash: Option<SoundId>,
    leaves_gust: Option<SoundId>,
    owl: Option<SoundId>,
    frog_croak: Option<SoundId>,
    frog_knock: Option<SoundId>,
    /// Day calls, each with how often it is picked, and whether it prefers open ground.
    birds: Vec<(SoundId, f32, bool)>,
    tropical_birds: Vec<(SoundId, f32, bool)>,
    desert_birds: Vec<(SoundId, f32, bool)>,
}

impl Ids {
    fn look_up(library: &SoundLibrary, generation: u32) -> Ids {
        let id = |name: &str| library.id_of(name);
        let calls = |list: &[(&str, f32, bool)]| {
            list.iter()
                .filter_map(|&(name, weight, open)| id(name).map(|s| (s, weight, open)))
                .collect::<Vec<_>>()
        };
        Ids {
            generation,
            wind_open: id("amb_wind_open"),
            wind_high: id("amb_wind_high"),
            leaves: id("amb_leaves"),
            needles: id("amb_needles"),
            surf: id("amb_surf"),
            sea: id("amb_sea"),
            crickets: id("amb_crickets"),
            night_tropical: id("amb_night_tropical"),
            frogs: id("amb_frogs"),
            wave_break: id("wave_break"),
            wave_wash: id("wave_wash"),
            leaves_gust: id("leaves_gust"),
            owl: id("owl_hoot"),
            frog_croak: id("frog_croak"),
            frog_knock: id("frog_knock"),
            birds: calls(&[
                ("bird_chirp", 3.0, false),
                ("bird_trill", 2.0, false),
                ("bird_warble", 2.0, false),
                ("bird_call_far", 1.0, false),
                ("crow_caw", 1.0, true),
            ]),
            tropical_birds: calls(&[
                ("parrot_squawk", 2.0, false),
                ("trop_whistle", 3.0, false),
                ("trop_bell", 1.0, false),
                ("trop_chatter", 2.0, false),
                ("trop_coo", 1.0, true),
            ]),
            // Ravens over the rim, a wren's trill off the canyon walls, a far call.
            desert_birds: calls(&[
                ("crow_caw", 3.0, true),
                ("bird_trill", 1.5, false),
                ("bird_call_far", 1.0, true),
            ]),
        }
    }
}

/// A coarse map of what grows and lies where, for the sound only.
pub struct Habitat {
    cell: f32,
    w: usize,
    h: usize,
    /// How closed the forest is overhead, 0 to 1.
    forest: Vec<f32>,
    /// How much of that forest is conifer, 0 to 1.
    conifer: Vec<f32>,
    /// How much of the cell is under the sea, 0 to 1.
    water: Vec<f32>,
    /// How much of it is low wet ground just above the water line, 0 to 1.
    wet: Vec<f32>,
}

/// Cells of the habitat map: at least this many metres, and no more than this many along an edge.
const HABITAT_CELL_M: f32 = 64.0;
const HABITAT_MAX_CELLS: f32 = 512.0;

impl Habitat {
    pub fn build(map: &MapFile) -> Habitat {
        let size = map.info().size_metres().to_f32();
        let cell = (size[0].max(size[1]) / HABITAT_MAX_CELLS).max(HABITAT_CELL_M);
        let w = ((size[0] / cell).ceil() as usize).max(1);
        let h = ((size[1] / cell).ceil() as usize).max(1);
        let mut crown = vec![0f32; w * h];
        let mut needle = vec![0f32; w * h];
        for p in map.props().iter().filter(|p| p.kind.is_tree()) {
            let xy = p.pos.to_f32();
            let scale = p.scale_milli as f32 / 1000.0;
            // Crown radii of the tree models, as the ground cover has them.
            let (reach, conifer) = mc_render::ground_cover::crown_of(p.kind);
            let area = std::f32::consts::PI * (reach * scale).powi(2) / (cell * cell);
            let (x, y) = ((xy[0] / cell) as usize, (xy[1] / cell) as usize);
            if x < w && y < h {
                crown[y * w + x] += area;
                needle[y * w + x] += area * conifer;
            }
        }
        let forest: Vec<f32> = crown.iter().map(|c| 1.0 - (-c * 1.6).exp()).collect();
        let conifer = needle
            .iter()
            .zip(&crown)
            .map(|(n, c)| (n / c.max(1e-4)).clamp(0.0, 1.0))
            .collect();
        // Water and wet ground from the overview heights, every 32 m.
        let (ow, oh) = map.overview_dims();
        let heights = map.overview();
        let spacing = (mc_map::CELL_SIZE_M as u32 * mc_map::OVERVIEW_STRIDE) as f32;
        let info = map.info();
        let sea = info.water_level.to_f32();
        let (mut water, mut wet, mut count) =
            (vec![0f32; w * h], vec![0f32; w * h], vec![0f32; w * h]);
        for sy in 0..oh as usize {
            for sx in 0..ow as usize {
                let z = info
                    .sample_to_height(heights[sy * ow as usize + sx])
                    .to_f32();
                let (x, y) = (
                    (sx as f32 * spacing / cell) as usize,
                    (sy as f32 * spacing / cell) as usize,
                );
                if x >= w || y >= h {
                    continue;
                }
                let i = y * w + x;
                count[i] += 1.0;
                if z < sea {
                    water[i] += 1.0;
                } else if z < sea + 4.0 {
                    wet[i] += 1.0;
                }
            }
        }
        for i in 0..w * h {
            let n = count[i].max(1.0);
            water[i] /= n;
            wet[i] /= n;
        }
        Habitat {
            cell,
            w,
            h,
            forest,
            conifer,
            water,
            wet,
        }
    }

    /// (forest, conifer, water, wet) in the cell under `xy`; off the map is open sea
    /// on a map with a sea, and bare ground on one without.
    fn at(&self, xy: Vec2) -> [f32; 4] {
        let (x, y) = ((xy.x / self.cell).floor(), (xy.y / self.cell).floor());
        if x < 0.0 || y < 0.0 || x >= self.w as f32 || y >= self.h as f32 {
            return [0.0, 0.0, 0.0, 0.0];
        }
        let i = y as usize * self.w + x as usize;
        [self.forest[i], self.conifer[i], self.water[i], self.wet[i]]
    }
}

/// What is round the focus, read from the habitat and eased over a second or so,
/// so a camera moving over forest edges and coastlines does not jump.
#[derive(Clone, Copy, Default)]
struct Surround {
    forest: f32,
    conifer: f32,
    water: f32,
    wet: f32,
    /// Where the forest and the water are, -1 left to 1 right.
    forest_pan: f32,
    water_pan: f32,
}

/// A small fast random source for timing and placing the calls.
struct Dice(u32);

impl Dice {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 >> 8) as f32 / 16_777_216.0
    }

    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.next()
    }

    /// True about `rate` times a second, checked each frame of `dt`.
    fn chance(&mut self, rate: f32, dt: f32) -> bool {
        self.next() < (rate * dt).min(0.5)
    }
}

/// A slow random swell, about 1 on average: eases from one random level to the
/// next, each held for a random few seconds, raised so gusts stand out of lulls.
#[derive(Clone, Copy)]
struct Gust {
    from: f32,
    to: f32,
    at: f32,
    span: f32,
}

impl Default for Gust {
    fn default() -> Gust {
        Gust {
            from: 0.5,
            to: 0.5,
            at: 0.0,
            span: 1.0,
        }
    }
}

impl Gust {
    fn step(&mut self, dt: f32, dice: &mut Dice, (shortest, longest): (f32, f32)) -> f32 {
        self.at += dt;
        if self.at >= self.span {
            self.at -= self.span;
            self.from = self.to;
            self.to = dice.next();
            self.span = dice.range(shortest, longest);
        }
        let k = (self.at / self.span).clamp(0.0, 1.0);
        let e = k * k * (3.0 - 2.0 * k);
        let v = self.from + (self.to - self.from) * e;
        0.3 + 1.6 * v * v
    }
}

#[derive(Default)]
pub struct Ambience {
    habitat: Option<Arc<Habitat>>,
    building: Option<Receiver<Habitat>>,
    ids: Option<Ids>,
    dice: Option<Dice>,
    surround: Surround,
    /// Wind gusts: the open-ground pair, the trees' pair, and the high wind.
    gusts: [Gust; 5],
    /// Eased gain of each bed voice, by its place in `BEDS`.
    beds: [f32; BEDS],
    /// Seconds since the battle was last heard loudly, and how loud it is now.
    quiet: f32,
    battle: f32,
    /// Seconds left of the hush after a nuclear blast (`blast`).
    hush: f32,
    /// Whether the score is playing, checked now and then.
    music: bool,
    music_check: f32,
    /// The breakers along the shore near the focus, and how loud the last ones
    /// near by were, fading (the surf bed swells with them).
    surf: surf::Surf,
    surge: f32,
    loops: Vec<(SoundId, f32, f32, f32)>,
    /// Calls to play this frame, as (sound, gain, pan, pitch, delay).
    calls: Vec<(SoundId, f32, f32, f32, f32)>,
    /// `MERIDIAN_AMBIENCE_LOG`: the mix is logged every two seconds, for tuning.
    log: Option<bool>,
    log_in: f32,
    logged_calls: Vec<String>,
}

/// Bed voices: wind open (2), wind high, leaves (2), needles (2), surf, sea (2),
/// crickets (2), tropical night (2), frogs.
const BEDS: usize = 15;
/// The first `WINDS` beds are the wind, which a fight ducks less than the rest.
const WINDS: usize = 3;

/// Seconds a nuclear blast hushes the world for, and how many of them it is
/// silent before it starts to come back.
const HUSH: f32 = 70.0;
const HUSH_HELD: f32 = 40.0;

impl Ambience {
    /// The beds that should sound now, as `Audio::set_weather_loops` wants them;
    /// `Game::battle_sounds` hands them over with the rain.
    pub fn loops(&self) -> &[(SoundId, f32, f32, f32)] {
        &self.loops
    }

    /// The last tick's battle as the player hears it: `din` is the summed gain of
    /// the shots, hits and deaths the mix played (`Game::battle_sounds`), so a fight
    /// on screen or a loud one just off it ducks the world, and one too far off to
    /// hear does not. Call it for each fresh tick.
    pub fn listen(&mut self, din: f32) {
        if din > 0.12 {
            self.quiet = 0.0;
        }
        self.battle = self.battle.max(din.min(4.0));
    }

    /// A nuclear blast, wherever it was: the world goes still for most of a minute.
    pub fn blast(&mut self) {
        self.hush = HUSH;
        self.quiet = 0.0;
        self.battle = 4.0;
    }

    /// Moves the ambience on by `dt`: eases the beds towards what the place,
    /// the sky and the battle call for, and scatters the calls. `ground` is the
    /// terrain's height at a point (`Renderer::ground_height`), for the shore.
    pub fn frame(
        &mut self,
        map: &Arc<MapFile>,
        cues: &Cues,
        ground: &dyn Fn(Vec2) -> f32,
        audio: &Audio,
        dt: f32,
    ) {
        self.start_habitat(map);
        self.dice
            .get_or_insert_with(|| Dice((map.content_id() as u32) | 1));
        self.music_check -= dt;
        if self.music_check <= 0.0 {
            self.music_check = 2.0;
            self.music = audio.music_status().is_some();
        }
        let (library, generation) = audio.library();
        self.step(&library, generation, cues, ground, dt);
        if *self
            .log
            .get_or_insert_with(|| std::env::var_os("MERIDIAN_AMBIENCE_LOG").is_some())
        {
            self.log_in -= dt;
            self.logged_calls
                .extend(self.calls.iter().map(|c| library.sound(c.0).name.clone()));
            if self.log_in <= 0.0 {
                self.log_in = 2.0;
                let s = &self.surround;
                let beds: Vec<String> = self
                    .loops
                    .iter()
                    .map(|l| format!("{} {:.3}@{:+.1}", library.sound(l.0).name, l.1, l.2))
                    .collect();
                log::info!(
                    "ambience: forest {:.2} (conifer {:.2}) water {:.2} wet {:.2}, quiet {:.0} s; beds [{}]; calls [{}]",
                    s.forest,
                    s.conifer,
                    s.water,
                    s.wet,
                    self.quiet.min(999.0),
                    beds.join(", "),
                    std::mem::take(&mut self.logged_calls).join(", ")
                );
            }
        }
        for (sound, gain, pan, pitch, delay) in self.calls.drain(..) {
            audio.play_weather_after(sound, gain, pan, pitch, delay);
        }
    }

    /// `frame` without the device: the beds into `loops`, the calls into `calls`.
    fn step(
        &mut self,
        library: &SoundLibrary,
        generation: u32,
        cues: &Cues,
        ground: &dyn Fn(Vec2) -> f32,
        dt: f32,
    ) {
        let dt = dt.clamp(0.0, 0.25);
        if self
            .ids
            .as_ref()
            .is_none_or(|ids| ids.generation != generation)
        {
            self.ids = Some(Ids::look_up(library, generation));
        }
        let dice = self.dice.get_or_insert(Dice(0x2545_F491));
        self.quiet += dt;
        // The world holds still a few seconds after the last shot before it stirs.
        if self.quiet > 3.0 {
            self.battle *= (-dt / 6.0).exp();
        }
        self.hush = (self.hush - dt).max(0.0);

        // Round the focus, as wide as the view: the middle and two rings.
        let right = Vec2::new(cues.yaw.cos(), -cues.yaw.sin());
        let reach = (cues.distance * 0.5).clamp(50.0, 2000.0);
        let mut read = Surround::default();
        if let Some(habitat) = &self.habitat {
            let centre = cues.focus.truncate();
            let (mut weight, mut forest_side, mut water_side) = (0.0, 0.0, 0.0);
            for ring in 0..3 {
                let (count, radius, w) = match ring {
                    0 => (1, 0.0, 2.0),
                    1 => (6, 0.5, 1.0),
                    _ => (8, 1.0, 0.7),
                };
                for k in 0..count {
                    let a = k as f32 / count as f32 * std::f32::consts::TAU + ring as f32 * 0.4;
                    let offset = Vec2::from_angle(a) * radius * reach;
                    let [forest, conifer, water, wet] = habitat.at(centre + offset);
                    let side = offset.dot(right) / reach;
                    read.forest += forest * w;
                    read.conifer += conifer * forest * w;
                    read.water += water * w;
                    read.wet += wet * w;
                    forest_side += forest * side * w;
                    water_side += water * side * w;
                    weight += w;
                }
            }
            read.conifer /= read.forest.max(1e-4);
            read.forest /= weight;
            read.water /= weight;
            read.wet /= weight;
            read.forest_pan = (forest_side / (read.forest * weight).max(1e-4)).clamp(-1.0, 1.0);
            read.water_pan = (water_side / (read.water * weight).max(1e-4)).clamp(-1.0, 1.0);
        }
        let ease = 1.0 - (-dt / 1.2).exp();
        let s = &mut self.surround;
        for (have, want) in [
            (&mut s.forest, read.forest),
            (&mut s.conifer, read.conifer),
            (&mut s.water, read.water),
            (&mut s.wet, read.wet),
            (&mut s.forest_pan, read.forest_pan),
            (&mut s.water_pan, read.water_pan),
        ] {
            *have += (want - *have) * ease;
        }
        let s = self.surround;

        // Close by, 1; a few hundred metres up, a half; from strategic zoom, nothing.
        let detail = 1.0 / (1.0 + (cues.distance / 650.0).powi(2));
        let far = smoothstep(250.0, 2500.0, cues.distance);
        // The sea is a wide sound and carries higher up, but not to strategic zoom.
        let wide = detail.sqrt() * (1.0 - smoothstep(2000.0, 5000.0, cues.distance));
        let day = 1.0 - smoothstep(0.35, 0.75, cues.darkness);
        let night = smoothstep(0.45, 0.85, cues.darkness);
        let dry = (1.0 - cues.rain).powi(2);
        let windy = (cues.wind / 12.0).clamp(0.35, 1.7) + cues.rain * 0.4;
        let land = 1.0 - s.water;
        let open = land * (1.0 - s.forest);
        let broadleaf = s.forest * (1.0 - s.conifer);
        let conifer = s.forest * s.conifer;
        // Shore: water and land both in earshot; sea: little but water.
        let shore = (4.0 * s.water * (1.0 - s.water)).min(1.0);
        let sea = (s.water - 0.5).max(0.0) * 2.0;
        // After a nuclear blast: still, then easing back over its last stretch.
        let still = 1.0 - smoothstep(0.0, HUSH - HUSH_HELD, self.hush);
        let calm_birds = smoothstep(10.0, 35.0, self.quiet) * still;
        let calm_insects = smoothstep(4.0, 15.0, self.quiet) * still;
        let level = LEVEL * if self.music { 0.8 } else { 1.0 };
        // Leaves, surf and waves all but go under a fight; the wind stays, low.
        let duck = level / (1.0 + self.battle * 6.0) * (0.03 + 0.97 * still);
        let wind_duck = level / (1.0 + self.battle * 1.5) * (0.3 + 0.7 * still);
        self.surge *= (-dt / 2.5).exp();

        let g: [f32; 5] = [
            self.gusts[0].step(dt, dice, (3.0, 9.0)),
            self.gusts[1].step(dt, dice, (3.0, 9.0)),
            self.gusts[2].step(dt, dice, (1.5, 5.0)),
            self.gusts[3].step(dt, dice, (1.5, 5.0)),
            self.gusts[4].step(dt, dice, (5.0, 14.0)),
        ];
        let trees = |g: f32| windy.powf(1.3) * g.powf(1.4);
        let chorus = if cues.tropical { 0.0 } else { 1.0 };
        // The desert's night is thinner, it has no frogs, and its water is a
        // sheltered lake: lapping along the shore, little open swell.
        let (sparse, frogs, swell) = if cues.desert {
            (0.45, 0.0, 0.35)
        } else {
            (1.0, 1.0, 1.0)
        };
        let frog_chorus = chorus * frogs;
        // (gain, pan, pitch) of each bed voice, in `BEDS` order.
        let want: [(f32, f32, f32); BEDS] = [
            (windy * g[0] * open * detail * 0.2, -0.45, 0.99),
            (windy * g[1] * open * detail * 0.2, 0.45, 1.01),
            (
                windy * g[4] * (0.03 + 0.09 * far) / (1.0 + cues.distance / 30_000.0),
                0.0,
                1.0,
            ),
            (trees(g[2]) * broadleaf * detail * 0.22, -0.5, 0.985),
            (trees(g[3]) * broadleaf * detail * 0.22, 0.5, 1.015),
            (trees(g[2]) * conifer * detail * 0.22, -0.5, 1.0),
            (trees(g[3]) * conifer * detail * 0.22, 0.5, 1.02),
            (
                shore * wide * (0.7 + 0.3 * windy) * 0.3 * (0.8 + 0.5 * self.surge),
                s.water_pan * 0.6,
                1.0,
            ),
            (sea * wide * (0.6 + 0.4 * windy) * 0.16 * swell, -0.4, 0.99),
            (sea * wide * (0.6 + 0.4 * windy) * 0.16 * swell, 0.4, 1.01),
            (
                night * land * dry * detail * calm_insects * chorus * sparse * 0.14,
                -0.5,
                0.985,
            ),
            (
                night * land * dry * detail * calm_insects * chorus * sparse * 0.14,
                0.5,
                1.02,
            ),
            (
                night * land * dry * detail * calm_insects * (1.0 - chorus) * 0.16,
                -0.5,
                0.99,
            ),
            (
                night * land * dry * detail * calm_insects * (1.0 - chorus) * 0.16,
                0.5,
                1.015,
            ),
            (
                night
                    * (s.wet * 3.0).min(1.0)
                    * dry.sqrt()
                    * detail
                    * calm_insects
                    * frog_chorus
                    * 0.2,
                0.0,
                1.0,
            ),
        ];
        let ids = self.ids.as_ref().expect("looked up above");
        let sounds: [Option<SoundId>; BEDS] = [
            ids.wind_open,
            ids.wind_open,
            ids.wind_high,
            ids.leaves,
            ids.leaves,
            ids.needles,
            ids.needles,
            ids.surf,
            ids.sea,
            ids.sea,
            ids.crickets,
            ids.crickets,
            ids.night_tropical,
            ids.night_tropical,
            ids.frogs,
        ];
        let ease = 1.0 - (-dt / 0.6).exp();
        self.loops.clear();
        for i in 0..BEDS {
            let (gain, pan, pitch) = want[i];
            let was = self.beds[i];
            let duck = if i < WINDS { wind_duck } else { duck };
            self.beds[i] += (gain * duck - self.beds[i]) * ease;
            // A voice that has faded is let go (the mixer fades it to nothing),
            // and one that is only just wanted waits until it is audible.
            let keep = if was > 0.0015 { 0.001 } else { 0.002 };
            if let (Some(sound), true) = (sounds[i], self.beds[i] > keep) {
                self.loops.push((sound, self.beds[i], pan, pitch));
            }
        }

        // The calls. Birds where there is forest, most at its edges; fewer in rain
        // and strong wind; none at night or near a fight. They are detail: gone by
        // the time the camera is high enough to see a base whole.
        let close = detail * detail.sqrt();
        let edge = 4.0 * s.forest * (1.0 - s.forest);
        let perches = land * (0.15 + 0.5 * s.forest + 1.1 * edge);
        let weather = dry * (1.0 - ((cues.wind - 14.0) / 10.0).clamp(0.0, 0.6));
        let birds = 0.6 * day * close * perches * weather * calm_birds;
        let calls = if cues.tropical {
            &ids.tropical_birds
        } else if cues.desert {
            &ids.desert_birds
        } else {
            &ids.birds
        };
        if !calls.is_empty() && dice.chance(birds, dt) {
            let total: f32 = calls
                .iter()
                .map(|c| c.1 * if c.2 { 0.5 + open } else { 1.0 })
                .sum();
            let mut pick = dice.range(0.0, total);
            let mut call = calls[0].0;
            for &(sound, weight, open_ground) in calls {
                pick -= weight * if open_ground { 0.5 + open } else { 1.0 };
                if pick <= 0.0 {
                    call = sound;
                    break;
                }
            }
            let pan =
                (s.forest_pan * s.forest.min(1.0) * 0.5 + dice.range(-0.7, 0.7)).clamp(-0.9, 0.9);
            let pitch = dice.range(0.94, 1.06);
            let gain = dice.range(0.35, 1.0) * 0.16 * duck * detail;
            self.calls.push((call, gain, pan, pitch, 0.0));
            // A bird often says it again, and another may answer from across the way.
            if dice.next() < 0.4 {
                self.calls
                    .push((call, gain * 0.9, pan, pitch, dice.range(0.8, 1.9)));
            }
            if dice.next() < 0.25 {
                let answer = (-pan + dice.range(-0.2, 0.2)).clamp(-0.9, 0.9);
                self.calls.push((
                    call,
                    gain * 0.7,
                    answer,
                    pitch * dice.range(0.96, 1.04),
                    dice.range(1.2, 2.0),
                ));
            }
        }
        // The night's odd voices over its chorus.
        if !cues.tropical {
            let owls = night * close * land * (0.2 + 0.8 * s.forest) * calm_birds / 35.0;
            if let (Some(owl), true) = (ids.owl, dice.chance(owls, dt)) {
                let pan = (s.forest_pan * 0.4 + dice.range(-0.6, 0.6)).clamp(-0.9, 0.9);
                self.calls
                    .push((owl, 0.12 * duck * detail, pan, dice.range(0.95, 1.05), 0.0));
            }
            let croaks = night * close * (s.wet * 3.0).min(1.0) * calm_insects * frogs / 5.0;
            if let (Some(frog), true) = (ids.frog_croak, dice.chance(croaks, dt)) {
                self.calls.push((
                    frog,
                    dice.range(0.06, 0.13) * duck * detail,
                    dice.range(-0.8, 0.8),
                    dice.range(0.85, 1.15),
                    0.0,
                ));
            }
        } else {
            let knocks =
                night * close * ((s.wet * 3.0).min(1.0) + 0.4 * s.forest) * calm_insects / 2.5;
            if let (Some(frog), true) = (ids.frog_knock, dice.chance(knocks, dt)) {
                self.calls.push((
                    frog,
                    dice.range(0.05, 0.12) * duck * detail,
                    dice.range(-0.8, 0.8),
                    dice.range(0.9, 1.12),
                    0.0,
                ));
            }
        }
        // A strong gust through the trees now and then, on top of the rustle.
        let rush = s.forest * detail * windy * (g[2].max(g[3]) - 1.2).max(0.0) / 3.0;
        if let (Some(gust), true) = (ids.leaves_gust, dice.chance(rush, dt)) {
            let pan = (s.forest_pan * 0.5 + dice.range(-0.5, 0.5)).clamp(-0.9, 0.9);
            self.calls.push((
                gust,
                dice.range(0.08, 0.16) * windy.min(1.3) * duck * detail,
                pan,
                dice.range(0.9, 1.1),
                0.0,
            ));
        }
        self.waves(cues, ground, dt, detail, (windy, duck));
    }

    /// The breakers along the shore near the focus: each crash played so it lands as
    /// the wave is seen to break, louder and a little lower the bigger the wave and
    /// quieter the further along the shore; close up, the wash as the foam reaches the
    /// sand. `windy` and `duck` are the wind's and the battle's say.
    fn waves(
        &mut self,
        cues: &Cues,
        ground: &dyn Fn(Vec2) -> f32,
        dt: f32,
        detail: f32,
        (windy, duck): (f32, f32),
    ) {
        // Nothing to hear from strategic zoom, so no looking for the shore either.
        if cues.distance > 5000.0 {
            return;
        }
        let ids = self.ids.as_ref().expect("looked up above");
        let focus = cues.focus.truncate();
        let reach = (cues.distance * 0.6).clamp(120.0, 1500.0);
        let climate = mc_render::shore::climate_scale(cues.desert, cues.tropical, cues.wind);
        let mut hits = Vec::new();
        self.surf.step(
            ground, cues.sea, focus, reach, cues.clock, climate, dt, &mut hits,
        );
        let right = Vec2::new(cues.yaw.cos(), -cues.yaw.sin());
        // How far along the shore a breaker is still heard well.
        let earshot = 150.0 + 0.6 * cues.distance;
        let close = detail * detail.sqrt();
        for hit in hits {
            let offset = hit.xy - focus;
            let near = 1.0 / (1.0 + (offset.length() / earshot).powi(2));
            let pan = (offset.dot(right) / (offset.length() + 0.3 * earshot)).clamp(-0.9, 0.9);
            let (sound, gain) = if hit.wash {
                (ids.wave_wash, 0.16 * hit.size.powf(0.6) * near * close)
            } else {
                self.surge = self.surge.max(hit.size * near);
                (
                    ids.wave_break,
                    0.3 * hit.size.powf(1.3) * near * detail.powf(0.6),
                )
            };
            let gain = gain * duck * (0.8 + 0.2 * windy.min(1.5));
            // `start` is the recipe's lead-in before the crash, at this pitch.
            let delay = (hit.start - cues.clock).max(0.0);
            if let (Some(sound), true) = (sound, gain > 0.004) {
                self.calls
                    .push((sound, gain, pan, surf::pitch(hit.size), delay));
            }
        }
    }

    /// The habitat map is built once, off the frame; until it is ready the world is open ground.
    fn start_habitat(&mut self, map: &Arc<MapFile>) {
        if self.habitat.is_some() {
            return;
        }
        if let Some(rx) = &self.building {
            match rx.try_recv() {
                Ok(habitat) => {
                    self.habitat = Some(Arc::new(habitat));
                    self.building = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => self.building = None,
            }
            return;
        }
        let (tx, rx) = mpsc::channel();
        let theirs = map.clone();
        let spawned = std::thread::Builder::new()
            .name("mc-ambience".into())
            .spawn(move || {
                let _ = tx.send(Habitat::build(&theirs));
            });
        if spawned.is_ok() {
            self.building = Some(rx);
        } else {
            // No thread to spare: build it here, once.
            self.habitat = Some(Arc::new(Habitat::build(map)));
        }
    }
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn library() -> SoundLibrary {
        SoundLibrary::load(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"))
            .unwrap()
    }

    /// Two kilometres square: broadleaf forest to the west, open grass in the
    /// middle, the sea to the east with a strip of wet ground along it.
    fn habitat() -> Habitat {
        let (w, h, cell) = (32, 32, 64.0);
        let mut hab = Habitat {
            cell,
            w,
            h,
            forest: vec![0.0; w * h],
            conifer: vec![0.0; w * h],
            water: vec![0.0; w * h],
            wet: vec![0.0; w * h],
        };
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                match x {
                    0..=11 => hab.forest[i] = 0.9,
                    22 => hab.wet[i] = 0.8,
                    23.. => hab.water[i] = 1.0,
                    _ => {}
                }
            }
        }
        hab
    }

    fn cues(x: f32, distance: f32, darkness: f32) -> Cues {
        Cues {
            focus: Vec3::new(x, 1024.0, 0.0),
            distance,
            yaw: 0.0,
            darkness,
            wind: 12.0,
            rain: 0.0,
            tropical: false,
            desert: false,
            sea: 0.0,
            clock: 0.0,
        }
    }

    /// The ground under `habitat()`: rising 3 in 100 from the sea's edge at x = 1472 m.
    fn ground(xy: Vec2) -> f32 {
        (1472.0 - xy.x) * 0.03
    }

    /// Runs `seconds` of frames and returns the names of the beds sounding at the
    /// end and of every call made on the way.
    fn run(
        amb: &mut Ambience,
        library: &SoundLibrary,
        cues: &Cues,
        seconds: f32,
    ) -> (Vec<(String, f32)>, Vec<String>) {
        let mut heard = Vec::new();
        let mut now = *cues;
        for f in 0..(seconds * 30.0) as usize {
            now.clock = cues.clock + f as f32 / 30.0;
            amb.step(library, 0, &now, &ground, 1.0 / 30.0);
            heard.extend(amb.calls.drain(..).map(|c| library.sound(c.0).name.clone()));
        }
        let beds = amb
            .loops()
            .iter()
            .map(|l| (library.sound(l.0).name.clone(), l.1))
            .collect();
        (beds, heard)
    }

    fn fresh() -> Ambience {
        Ambience {
            habitat: Some(Arc::new(habitat())),
            quiet: 60.0,
            ..Default::default()
        }
    }

    fn has(beds: &[(String, f32)], name: &str) -> bool {
        beds.iter().any(|b| b.0 == name)
    }

    #[test]
    fn a_forest_edge_by_day_has_leaves_and_birds_and_no_night() {
        let library = library();
        let mut amb = fresh();
        let (beds, calls) = run(&mut amb, &library, &cues(760.0, 150.0, 0.0), 120.0);
        assert!(
            has(&beds, "amb_leaves") && has(&beds, "amb_wind_open"),
            "{beds:?}"
        );
        assert!(
            !has(&beds, "amb_crickets") && !has(&beds, "amb_surf"),
            "{beds:?}"
        );
        assert!(calls.len() > 10, "{} calls in two minutes", calls.len());
        assert!(calls
            .iter()
            .all(|c| !c.starts_with("owl") && !c.starts_with("frog")));
        let kinds: std::collections::BTreeSet<_> = calls.iter().collect();
        assert!(kinds.len() >= 4, "{kinds:?}");
    }

    #[test]
    fn night_brings_the_chorus_and_the_shore_its_frogs_and_surf() {
        let library = library();
        let mut amb = fresh();
        let (beds, calls) = run(&mut amb, &library, &cues(1440.0, 150.0, 1.0), 120.0);
        assert!(
            has(&beds, "amb_crickets") && has(&beds, "amb_frogs") && has(&beds, "amb_surf"),
            "{beds:?}"
        );
        assert!(
            calls
                .iter()
                .all(|c| !c.starts_with("bird") && !c.starts_with("crow")),
            "{calls:?}"
        );
        assert!(
            calls
                .iter()
                .any(|c| c.starts_with("wave") || c.starts_with("frog")),
            "{calls:?}"
        );
    }

    /// On the beach the breakers are heard, each started just ahead of the moment
    /// its crash is due, and the wash too from close by; inland, none.
    #[test]
    fn the_shore_has_its_breakers_and_inland_has_none() {
        let library = library();
        let mut amb = fresh();
        let at = cues(1440.0, 150.0, 0.0);
        let (mut crashes, mut washes) = (0, 0);
        for f in 0..60 * 30 {
            let now = Cues {
                clock: f as f32 / 30.0,
                ..at
            };
            amb.step(&library, 0, &now, &ground, 1.0 / 30.0);
            for (sound, gain, _, _, delay) in amb.calls.drain(..) {
                let name = &library.sound(sound).name;
                crashes += (name == "wave_break") as u32;
                washes += (name == "wave_wash") as u32;
                if name.starts_with("wave") {
                    assert!(gain > 0.0 && (0.0..0.2).contains(&delay), "{name} {delay}");
                }
            }
        }
        assert!(
            (12..=40).contains(&crashes),
            "{crashes} crashes in a minute"
        );
        assert!(washes >= 6, "{washes} washes");
        let (_, inland) = run(&mut fresh(), &library, &cues(760.0, 150.0, 0.0), 60.0);
        assert!(inland.iter().all(|c| !c.starts_with("wave")), "{inland:?}");
    }

    #[test]
    fn from_strategic_zoom_only_a_faint_high_wind_is_left() {
        let library = library();
        let mut amb = fresh();
        let (beds, calls) = run(&mut amb, &library, &cues(1024.0, 12_000.0, 0.0), 30.0);
        assert!(calls.is_empty(), "{calls:?}");
        assert_eq!(beds.len(), 1, "{beds:?}");
        assert_eq!(beds[0].0, "amb_wind_high");
        assert!(beds[0].1 < 0.25, "{beds:?}");
    }

    #[test]
    fn birds_go_quiet_in_a_fight_and_come_back_after() {
        let library = library();
        let mut amb = fresh();
        let at = cues(760.0, 150.0, 0.0);
        amb.listen(1.0);
        let (_, during) = run(&mut amb, &library, &at, 5.0);
        assert!(during.is_empty(), "{during:?}");
        let (_, after) = run(&mut amb, &library, &at, 120.0);
        assert!(!after.is_empty());
    }

    /// The bed level of everything but the wind, and the loudest call made.
    fn below_the_wind(beds: &[(String, f32)], calls: &[(String, f32)]) -> f32 {
        beds.iter()
            .filter(|b| !b.0.starts_with("amb_wind"))
            .chain(calls)
            .map(|b| b.1)
            .fold(0.0, f32::max)
    }

    /// Sounds by name and gain.
    type Named = Vec<(String, f32)>;

    /// Runs `seconds` of a fight heard at `din` on every tick (ten a second): the
    /// beds sounding at the end and every call made on the way.
    fn fight(
        amb: &mut Ambience,
        library: &SoundLibrary,
        cues: &Cues,
        seconds: f32,
        din: f32,
    ) -> (Named, Named) {
        let mut calls = Vec::new();
        for f in 0..(seconds * 30.0) as usize {
            if f % 3 == 0 {
                amb.listen(din);
            }
            amb.step(library, 0, cues, &ground, 1.0 / 30.0);
            calls.extend(
                amb.calls
                    .drain(..)
                    .map(|c| (library.sound(c.0).name.clone(), c.1)),
            );
        }
        let beds = amb
            .loops()
            .iter()
            .map(|l| (library.sound(l.0).name.clone(), l.1))
            .collect();
        (beds, calls)
    }

    #[test]
    fn a_fight_on_the_shore_leaves_only_the_wind() {
        let library = library();
        let mut amb = fresh();
        let at = cues(1440.0, 150.0, 1.0);
        let (beds, calls) = fight(&mut amb, &library, &at, 30.0, 0.0);
        let calm = below_the_wind(&beds, &calls);
        assert!(calm > 0.05, "{beds:?}");
        let (beds, calls) = fight(&mut amb, &library, &at, 10.0, 1.5);
        assert!(
            below_the_wind(&beds, &calls) < calm / 10.0,
            "{beds:?} {calls:?}"
        );
        assert!(calls.iter().all(|c| c.0.starts_with("wave")), "{calls:?}");
        assert!(has(&beds, "amb_wind_open"), "{beds:?}");
        // Thirty seconds after the last shot the shore is back.
        let (beds, calls) = fight(&mut amb, &library, &at, 30.0, 0.0);
        assert!(below_the_wind(&beds, &calls) > calm * 0.5, "{beds:?}");
    }

    #[test]
    fn a_nuke_anywhere_hushes_the_world_for_most_of_a_minute() {
        let library = library();
        let mut amb = fresh();
        let at = cues(1440.0, 150.0, 1.0);
        let (beds, calls) = fight(&mut amb, &library, &at, 30.0, 0.0);
        let calm = below_the_wind(&beds, &calls);
        amb.blast();
        let (beds, calls) = fight(&mut amb, &library, &at, 35.0, 0.0);
        assert!(
            below_the_wind(&beds, &calls) < calm / 20.0,
            "{beds:?} {calls:?}"
        );
        assert!(calls.iter().all(|c| c.0.starts_with("wave")), "{calls:?}");
        // Well over a minute on, it has all come back.
        let (beds, calls) = fight(&mut amb, &library, &at, 60.0, 0.0);
        assert!(below_the_wind(&beds, &calls) > calm * 0.5, "{beds:?}");
        assert!(calls.iter().any(|c| c.0.starts_with("frog")), "{calls:?}");
    }
}

#[cfg(test)]
mod map_probe {
    #[test]
    #[ignore]
    fn zz_habitat_probe() {
        for path in crate::setup::list_maps() {
            if path.extension().is_none_or(|e| e != "mcmap") {
                continue;
            }
            let map = mc_map::MapFile::open(&path).unwrap();
            let t = std::time::Instant::now();
            let h = super::Habitat::build(&map);
            let mean = |v: &[f32]| v.iter().sum::<f32>() / v.len() as f32;
            eprintln!(
                "{}: {}x{} cells of {:.0} m in {:.0} ms; forest {:.2} conifer {:.2} water {:.2} wet {:.3}",
                path.display(),
                h.w,
                h.h,
                h.cell,
                t.elapsed().as_secs_f32() * 1000.0,
                mean(&h.forest),
                mean(&h.conifer),
                mean(&h.water),
                mean(&h.wet)
            );
        }
    }
}
