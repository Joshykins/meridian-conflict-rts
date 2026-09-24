//! The sky, the light it gives, and the weather in it.
//!
//! One sun sets the whole scene's light: its colour after the air it came
//! through, the sky's blue from single Rayleigh/Mie scattering, and the land's
//! bounce (`Atmosphere`, scene set binding 22). Every lit shader reads it, and
//! `apply_haze` blends distance into the same sky.
//!
//! Over the map runs a weather field on the GPU (`clouds_sim.wgsl`): cloud
//! cover and storms carried by the wind, cut by aircraft, thrown back by big
//! blasts, parted by anything vast in the air, healing over afterwards. Its
//! state is scene set binding 21, so the ground gets the clouds' shadows. The
//! clouds themselves are ray-marched at half size (`clouds.wgsl`) and laid over
//! the picture before the icons, thinned away round what the player is looking
//! at and what they have selected. Storms flash with lightning that lights the
//! cloud from inside and the ground below, and sometimes strikes.
//!
//! Everything here is cosmetic and client-side: nothing feeds back into the sim.

use crate::camera::Camera;
use crate::gpu::{Buffer, Gpu, GpuError, Image, ImageDesc};
use crate::pipelines::{self, Blend, Depth, Layouts, Passes, PipelineDesc, VertexKind, CLOUD_MARCH_FORMAT, HDR_FORMAT};
use ash::vk;
use bytemuck::{Pod, Zeroable};
use glam::{Vec2, Vec3, Vec4};
use mc_data::weather::{Weather, WeatherPreset};

/// Texels a side of the weather map, whatever the map's size.
pub const WEATHER_RES: u32 = 1024;
/// Texels across the clouds' shade on the land (clouds.wgsl `cs_shade`).
const SHADE_RES: u32 = 512;
const NOISE_RES: u32 = 128;
pub const MAX_DISTURBERS: usize = 128;
const MAX_STORMS: usize = 8;
const MAX_CLEARS: usize = 8;
const MAX_FLASHES: usize = 4;
/// Texels a side of the cloud floor (the smoothed land the layer rides on).
const FLOOR_RES: u32 = 128;
/// The cloud base above the floor. Fair-weather cloud sits where aircraft fly
/// (22-300 m up): bases wander from cloud to cloud (`cloud_at`), so fixed
/// wings cruise through them and a low one can swallow a gunship.
const CLOUD_BASE: f32 = 150.0;
/// Fair-weather tops above the base, before towering weather lifts them.
const CLOUD_DECK: f32 = 380.0;
/// Mirrors `RAIN_DROPS` in clouds.wgsl.
const RAIN_DROPS: u32 = 15000;
/// The cloud march runs at 1/this of the screen each way (`MERIDIAN_CLOUD_RES`).
fn march_divisor() -> u32 {
    std::env::var("MERIDIAN_CLOUD_RES").ok().and_then(|v| v.parse().ok()).unwrap_or(3).clamp(1, 4)
}

/// Toward the sun at `hour` (24-hour clock) on an equinox day: up at 6 from
/// -x, across the sky tilted toward +y so it is never quite overhead, down at
/// 18 to +x. The afternoon default (15.3) lights relief from the right of the
/// default view, throwing shadows across the screen.
pub fn sun_at_hour(hour: f32) -> Vec3 {
    let a = (hour - 6.0) / 12.0 * std::f32::consts::PI;
    Vec3::new(-a.cos() * 0.95, 0.36, a.sin() * 0.95).normalize()
}

/// Toward the moon, which lights the night.
const MOON: Vec3 = Vec3::new(-0.45, 0.35, 0.74);

/// 1 when the sun is well down, 0 in daylight.
fn night(sun: Vec3) -> f32 {
    1.0 - smoothstep(-0.12, 0.03, sun.z)
}

/// The direction the scene is lit from at `hour`: the sun, or the moon once it is dark.
pub fn light_at_hour(hour: f32) -> Vec3 {
    let sun = sun_at_hour(hour);
    if night(sun) > 0.5 {
        MOON.normalize()
    } else {
        Vec3::new(sun.x, sun.y, sun.z.max(0.03)).normalize()
    }
}

// Mirrors `Atmosphere` in common.wgsl.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Atmosphere {
    sun_color: [f32; 4],
    sky_color: [f32; 4],
    horizon_color: [f32; 4],
    ground_color: [f32; 4],
    wind: [f32; 4],
    layer: [f32; 4],
    weather: [f32; 4],
    view: [f32; 4],
    counts: [f32; 4],
    clears: [[f32; 4]; MAX_CLEARS],
    flashes: [[f32; 4]; MAX_FLASHES],
    bolts: [[f32; 4]; MAX_FLASHES],
    prev_view_proj: [[f32; 4]; 4],
    frame: [f32; 4],
    shape: [f32; 4],
}
const _: () = assert!(std::mem::size_of::<Atmosphere>() == 496);

/// Something stirring the weather this frame. Mirrors clouds_sim.wgsl.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
pub struct Disturber {
    /// xy where it is now, radius (m), kind (0 wake, 1 blast, 2 hull).
    a: [f32; 4],
    /// xy velocity (m/s), strength, age (s).
    b: [f32; 4],
    /// z height (m) it moves at; the rest spare.
    c: [f32; 4],
}

const KIND_WAKE: f32 = 0.0;
const KIND_BLAST: f32 = 1.0;

// Air, as in common.wgsl.
const RAYLEIGH: Vec3 = Vec3::new(5.8e-6, 13.5e-6, 33.1e-6);
const RAYLEIGH_H: f32 = 8000.0;
const MIE: f32 = 8.0e-6;
const MIE_H: f32 = 1400.0;
/// The sun above the air, in the scene's units.
const SUN_POWER: f32 = 4.3;
const SKY_GAIN: f32 = 5.0;
/// The sky's light on the ground, against the sky's own drawn brightness:
/// the drawn sky is lifted for the look, what it lights is kept physical.
const AMBIENT_GAIN: f32 = 3.1;

fn air_mass(up: f32) -> f32 {
    let up = up.clamp(0.0, 1.0);
    let zenith = up.acos().to_degrees();
    (1.0 / (up + 0.50572 * (96.07995 - zenith).max(0.01).powf(-1.6364))).min(38.0)
}

fn phase_rayleigh(mu: f32) -> f32 {
    0.0596831 * (1.0 + mu * mu)
}

fn phase_hg(mu: f32, g: f32) -> f32 {
    let d = 1.0 + g * g - 2.0 * g * mu;
    0.0795775 * (1.0 - g * g) / (d * d.sqrt())
}

/// The sun's light at the ground.
fn sun_at_ground(sun: Vec3) -> Vec3 {
    let m = air_mass(sun.z);
    let tau = (RAYLEIGH * RAYLEIGH_H + Vec3::splat(MIE * MIE_H * 1.1)) * m;
    // A little less reddening than the whole column would give: the eye
    // adapts, and a low sun should read warm, not orange.
    let t = Vec3::new((-tau.x).exp(), (-tau.y).exp(), (-tau.z).exp()).powf(0.6);
    t * SUN_POWER
}

/// `sky_radiance` from bindings.wgsl without the multiple-scattering fill.
fn sky_single(d: Vec3, sun: Vec3, sun_rgb: Vec3) -> Vec3 {
    let mu = d.dot(sun);
    let m = air_mass(d.z.max(0.0));
    let r = RAYLEIGH * RAYLEIGH_H * m;
    let mie = Vec3::splat(MIE * MIE_H * m);
    let ext = r + mie * 1.1;
    let scatter = r * phase_rayleigh(mu) + mie * phase_hg(mu, 0.78);
    let fade = Vec3::ONE - Vec3::new((-ext.x).exp(), (-ext.y).exp(), (-ext.z).exp());
    sun_rgb * scatter / ext.max(Vec3::splat(1e-6)) * fade * SKY_GAIN
}

struct Lighting {
    sun: Vec3,
    sky: Vec3,
    horizon: Vec3,
    ground: Vec3,
}

/// The scene's light at `hour`: sunlight by day, reddening as the sun gets low,
/// a cool moonlight at night (kept bright enough to play by).
fn lighting_at_hour(hour: f32) -> (Lighting, f32) {
    let sun = sun_at_hour(hour);
    let n = night(sun);
    let lit = Vec3::new(sun.x, sun.y, sun.z.max(0.03)).normalize();
    let mut day = lighting(lit);
    // The sun's disk slips below the horizon: its direct light goes first.
    let up = smoothstep(-0.04, 0.06, sun.z);
    // A low sun lights flat ground at a glancing angle and through a lot of
    // air; lift it (and the sky) so dawn and dusk stay warm and readable
    // instead of murky.
    let low = 1.0 - smoothstep(0.12, 0.55, sun.z);
    day.sun *= up * (1.0 + 0.9 * low);
    day.sky *= 1.0 + 0.6 * low;
    day.ground *= 1.0 + 0.6 * low;
    let moon = MOON.normalize();
    // Day-for-night: a strong cool moon and a blue sky glow, so a night
    // battle stays readable.
    let moonlight = Vec3::new(0.34, 0.45, 0.72) * 2.4;
    let night_light = Lighting {
        sun: moonlight,
        sky: Vec3::new(0.09, 0.12, 0.21),
        horizon: Vec3::new(0.045, 0.06, 0.11),
        ground: Vec3::new(0.012, 0.015, 0.02) + moonlight * moon.z * 0.02,
    };
    let mix = |a: Vec3, b: Vec3| a.lerp(b, n);
    (
        Lighting {
            sun: mix(day.sun, night_light.sun),
            sky: mix(day.sky, night_light.sky),
            horizon: mix(day.horizon, night_light.horizon),
            ground: mix(day.ground, night_light.ground),
        },
        n,
    )
}

/// The whole scene's light for one sun.
fn lighting(sun: Vec3) -> Lighting {
    let sun_rgb = sun_at_ground(sun);
    // Cosine-weighted mean of the sky over the upper hemisphere: what a face
    // looking straight up is lit by, over pi.
    let mut sum = Vec3::ZERO;
    let mut weight = 0.0;
    for i in 0..24 {
        for j in 1..12 {
            let az = i as f32 / 24.0 * std::f32::consts::TAU;
            let el = j as f32 / 12.0 * std::f32::consts::FRAC_PI_2;
            let d = Vec3::new(az.cos() * el.cos(), az.sin() * el.cos(), el.sin());
            let w = el.sin() * el.cos();
            sum += sky_single(d, sun, sun_rgb) * w;
            weight += w;
        }
    }
    let sky_single_mean = sum / weight;
    // Light scattered more than once, which the single-scatter sky leaves out.
    let sky = sky_single_mean * (AMBIENT_GAIN / SKY_GAIN) * 1.3;
    let away = Vec3::new(-sun.x, -sun.y, 0.0).normalize_or_zero();
    let horizon = sky_single(Vec3::new(away.x, away.y, 0.03).normalize(), sun, sun_rgb);
    // Grass and soil bounce about a fifth of what lands on them, green-brown.
    let land = Vec3::new(0.16, 0.17, 0.11);
    let ground = land * (sun_rgb * sun.z / std::f32::consts::PI + sky);
    Lighting { sun: sun_rgb, sky, horizon, ground }
}

/// A small, fast generator: the weather needs variety, not quality.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
    fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.next()
    }
}

struct Storm {
    pos: Vec2,
    vel: Vec2,
    radius: f32,
    peak: f32,
    age: f32,
    life: f32,
    next_flash: f32,
}

impl Storm {
    /// Building, raging, then raining itself out.
    fn strength(&self) -> f32 {
        let t = self.age / self.life;
        let rise = smoothstep(0.0, 0.2, t);
        let fall = 1.0 - smoothstep(0.7, 1.0, t);
        self.peak * rise * fall
    }
}

struct Flash {
    pos: Vec3,
    start: f32,
    /// Where it strikes the ground, if it does.
    bolt: Option<Vec2>,
    seed: f32,
    /// Return strokes: (seconds after the start, brightness).
    strokes: [(f32, f32); 4],
}

impl Flash {
    const LIFE: f32 = 0.9;

    fn brightness(&self, now: f32) -> f32 {
        let t = now - self.start;
        let mut b = 0.0;
        for (at, amp) in self.strokes {
            if t >= at {
                b += amp * (-(t - at) / 0.07).exp();
            }
        }
        b
    }
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// A lightning flash, for the thunder that follows it.
#[derive(Clone, Copy, Debug)]
pub struct Thunder {
    /// Where the flash was: in the cloud, or where it struck the ground.
    pub pos: Vec3,
    /// 1 an ordinary flash.
    pub strength: f32,
    /// It struck the ground: a crack, not only a roll.
    pub bolt: bool,
}

/// A half float, as the weather map stores it.
fn f16_to_f32(h: u16) -> f32 {
    let sign = if h & 0x8000 != 0 { -1.0 } else { 1.0 };
    let exp = ((h >> 10) & 0x1f) as i32;
    let frac = (h & 0x3ff) as f32;
    sign * match exp {
        0 => frac * 2f32.powi(-24),
        31 => f32::INFINITY,
        _ => (1.0 + frac / 1024.0) * 2f32.powi(exp - 15),
    }
}

/// Aircraft as the last tick saw them, carried forward between ticks.
struct Flyer {
    prev: Vec3,
    pos: Vec3,
    radius: f32,
}

/// Everything the weather needs from one frame.
pub struct SkyFrame<'a> {
    pub camera: &'a Camera,
    pub time: f32,
    /// Position between the last two ticks.
    pub alpha: f32,
    /// Seconds a tick covers.
    pub tick_seconds: f32,
    /// Units the player has selected: indices into the last tick's units.
    pub selected: &'a [u32],
}

pub struct Sky {
    // Resources.
    state: [Image; 2],
    flow: [Image; 2],
    noise: Image,
    /// The sunlight the drawn clouds let through to the land, over the map.
    shade: Image,
    /// The march this frame, then two histories it is folded into in turn.
    targets: Vec<Image>,
    target_fbs: Vec<vk::Framebuffer>,
    march_size: (u32, u32),
    atmos: Buffer,
    disturbers_buf: Buffer,
    storms_buf: Buffer,
    sampler: vk::Sampler,
    pool: vk::DescriptorPool,
    sim_layout: vk::DescriptorSetLayout,
    draw_layout: vk::DescriptorSetLayout,
    sim_pipeline_layout: vk::PipelineLayout,
    draw_pipeline_layout: vk::PipelineLayout,
    sim_sets: [vk::DescriptorSet; 2],
    /// Set 1 of the cloud passes, one per history: `k` reads history `1 - k`, writes `k`.
    draw_sets: [vk::DescriptorSet; 2],
    advect: vk::Pipeline,
    force: vk::Pipeline,
    noise_pipeline: vk::Pipeline,
    shade_pipeline: vk::Pipeline,
    sky_pipeline: vk::Pipeline,
    march_pipeline: vk::Pipeline,
    resolve_pipeline: vk::Pipeline,
    composite_pipeline: vk::Pipeline,
    rain_pipeline: vk::Pipeline,
    modules: [vk::ShaderModule; 2],
    /// The march target's pass, then the histories' (`bloom_down`'s format).
    march_pass: vk::RenderPass,
    resolve_pass: vk::RenderPass,

    // Temporal accumulation.
    frame_index: u32,
    prev_view_proj: Option<glam::Mat4>,
    history_valid: bool,

    // Weather.
    map_size: Vec2,
    /// The cloud base above the floor.
    base: f32,
    /// The floor, row by row, and its lowest and highest points.
    floor: Vec<f32>,
    floor_image: Image,
    floor_range: (f32, f32),
    wind: Vec2,
    drift: Vec2,
    storms: Vec<Storm>,
    flashes: Vec<Flash>,
    flyers: Vec<Flyer>,
    /// Where every unit of the last tick stands, for the selection's clearings.
    units: Vec<Option<Vec2>>,
    blasts: Vec<(Vec2, f32, f32, f32)>,
    clears: Vec<Vec4>,
    clear_strength: f32,
    rng: Rng,
    last_time: Option<f32>,
    step: f32,
    reset: bool,
    /// Storms a map this size carries in usual weather.
    usual_storms: f32,
    /// Where a raging storm is parked and never moves on (the test range's
    /// "storm overhead"; `MERIDIAN_WEATHER=storm` parks one by the map's middle).
    parked_storm: Option<Vec2>,
    weather: Weather,
    /// When in the day it is (24-hour clock).
    hour: f32,
    /// Weather map texels round the camera's focus, copied back each frame for
    /// the sound of rain: read one frame late, after the fence.
    readback: Buffer,
    readback_pending: bool,
    /// How hard it is raining where the camera looks, 0 to 1.
    rain_here: f32,
    /// Lightning the player should hear: where, how bright, and when.
    thunder: Vec<Thunder>,
    /// The weather map texel at the corner of the 4x4 block copied back.
    focus_texel: (u32, u32),
    /// Clouds drawn at all (`MERIDIAN_CLOUDS=0` turns them off; the sky,
    /// light and haze stay).
    clouds: bool,

}

impl Sky {
    pub fn new(
        gpu: &Gpu,
        layouts: &Layouts,
        passes: &Passes,
        map_size: Vec2,
        water_level: f32,
        seed: u64,
        ground: impl Fn(Vec2) -> f32,
    ) -> Result<Sky, GpuError> {
        // The cloud floor: the land smoothed over a few hundred metres (the sea
        // where there is no land), so the layer rides over hills and plateaus
        // at the height aircraft fly, not at a fixed height above the sea.
        let n = FLOOR_RES as usize;
        let cell = map_size / FLOOR_RES as f32;
        let mut floor = vec![0.0f32; n * n];
        for y in 0..n {
            for x in 0..n {
                let centre = (Vec2::new(x as f32, y as f32) + 0.5) * cell;
                let mut sum = 0.0;
                for j in -1..=1 {
                    for i in -1..=1 {
                        let at = (centre + Vec2::new(i as f32, j as f32) * cell * 0.5).clamp(Vec2::ZERO, map_size);
                        sum += ground(at).max(water_level);
                    }
                }
                floor[y * n + x] = sum / 9.0;
            }
        }
        for _ in 0..3 {
            let before = floor.clone();
            for y in 0..n {
                for x in 0..n {
                    let mut sum = 0.0;
                    for j in -1i32..=1 {
                        for i in -1i32..=1 {
                            let xx = (x as i32 + i).clamp(0, n as i32 - 1) as usize;
                            let yy = (y as i32 + j).clamp(0, n as i32 - 1) as usize;
                            sum += before[yy * n + xx];
                        }
                    }
                    floor[y * n + x] = sum / 9.0;
                }
            }
        }
        let floor_min = floor.iter().copied().fold(f32::INFINITY, f32::min);
        let floor_max = floor.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        log::info!("cloud floor {floor_min:.0}..{floor_max:.0} m, sea {water_level:.0} m, centre {:.0} m", floor[(n / 2) * n + n / 2]);
        eprintln!("cloud floor {floor_min:.0}..{floor_max:.0} m, sea {water_level:.0} m, centre {:.0} m", floor[(n / 2) * n + n / 2]);
        let floor_image = gpu.image(&ImageDesc {
            width: FLOOR_RES,
            height: FLOOR_RES,
            format: vk::Format::R32_SFLOAT,
            usage: vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST,
            layers: 1,
            mips: 1,
            array: false,
        })?;
        gpu.upload_image(&floor_image, 0, 0, None, bytemuck::cast_slice(&floor), true)?;
        let dev = &gpu.device;
        let storage = vk::ImageUsageFlags::STORAGE | vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_SRC;
        let weather_image = || {
            gpu.image(&ImageDesc {
                width: WEATHER_RES,
                height: WEATHER_RES,
                format: vk::Format::R16G16B16A16_SFLOAT,
                usage: storage,
                layers: 1,
                mips: 1,
                array: false,
            })
        };
        let state = [weather_image()?, weather_image()?];
        let flow = [weather_image()?, weather_image()?];
        let noise = noise_image(gpu)?;
        let shade = gpu.image(&ImageDesc {
            width: SHADE_RES,
            height: SHADE_RES,
            format: vk::Format::R16G16B16A16_SFLOAT,
            usage: vk::ImageUsageFlags::STORAGE | vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST,
            layers: 1,
            mips: 1,
            array: false,
        })?;
        let whole = vk::ImageSubresourceRange {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            base_mip_level: 0,
            level_count: 1,
            base_array_layer: 0,
            layer_count: 1,
        };
        gpu.submit_once(|cmd| unsafe {
            for image in state.iter().chain(&flow).chain([&noise]) {
                gpu.transition(cmd, image.image, whole, vk::ImageLayout::UNDEFINED, vk::ImageLayout::GENERAL);
            }
            // Full sun and alpha 0 (never written): no shade until the clouds cast one.
            gpu.transition(cmd, shade.image, whole, vk::ImageLayout::UNDEFINED, vk::ImageLayout::GENERAL);
            let lit = vk::ClearColorValue { float32: [1.0, 0.0, 0.0, 0.0] };
            gpu.device.cmd_clear_color_image(cmd, shade.image, vk::ImageLayout::GENERAL, &lit, &[whole]);
        })?;

        let atmos = gpu.host_buffer(std::mem::size_of::<Atmosphere>() as u64, vk::BufferUsageFlags::UNIFORM_BUFFER)?;
        let disturbers_buf = gpu.host_buffer(
            (MAX_DISTURBERS * std::mem::size_of::<Disturber>()) as u64,
            vk::BufferUsageFlags::STORAGE_BUFFER,
        )?;
        let storms_buf = gpu.host_buffer((MAX_STORMS * 16) as u64, vk::BufferUsageFlags::STORAGE_BUFFER)?;
        let sampler = gpu.sampler(vk::Filter::LINEAR, vk::SamplerAddressMode::CLAMP_TO_EDGE, false, None)?;

        use vk::DescriptorType as T;
        let set_layout = |bindings: &[(u32, vk::DescriptorType)], stages| {
            let b: Vec<_> = bindings
                .iter()
                .map(|&(binding, ty)| {
                    vk::DescriptorSetLayoutBinding::default()
                        .binding(binding)
                        .descriptor_type(ty)
                        .descriptor_count(1)
                        .stage_flags(stages)
                })
                .collect();
            unsafe { dev.create_descriptor_set_layout(&vk::DescriptorSetLayoutCreateInfo::default().bindings(&b), None) }
        };
        let sim_layout = set_layout(
            &[
                (0, T::UNIFORM_BUFFER),
                (1, T::SAMPLED_IMAGE),
                (2, T::SAMPLED_IMAGE),
                (3, T::STORAGE_IMAGE),
                (4, T::STORAGE_IMAGE),
                (5, T::SAMPLER),
                (6, T::STORAGE_BUFFER),
                (7, T::STORAGE_BUFFER),
                (8, T::STORAGE_IMAGE),
            ],
            vk::ShaderStageFlags::COMPUTE,
        )?;
        let gfx = vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT;
        let draw_layout = set_layout(
            &[
                (0, T::SAMPLED_IMAGE),
                (1, T::SAMPLED_IMAGE),
                (2, T::SAMPLED_IMAGE),
                (3, T::SAMPLED_IMAGE),
                (4, T::SAMPLED_IMAGE),
                (5, T::STORAGE_IMAGE),
            ],
            gfx | vk::ShaderStageFlags::COMPUTE,
        )?;
        let pipeline_layout = |sets: &[vk::DescriptorSetLayout], stages| {
            let push = [vk::PushConstantRange { stage_flags: stages, offset: 0, size: 8 }];
            unsafe {
                dev.create_pipeline_layout(
                    &vk::PipelineLayoutCreateInfo::default().set_layouts(sets).push_constant_ranges(&push),
                    None,
                )
            }
        };
        let sim_pipeline_layout = pipeline_layout(&[sim_layout], vk::ShaderStageFlags::COMPUTE)?;
        let draw_pipeline_layout = pipeline_layout(&[layouts.scene_set, draw_layout], gfx)?;

        let sizes = [
            vk::DescriptorPoolSize { ty: T::UNIFORM_BUFFER, descriptor_count: 2 },
            vk::DescriptorPoolSize { ty: T::SAMPLED_IMAGE, descriptor_count: 16 },
            vk::DescriptorPoolSize { ty: T::STORAGE_IMAGE, descriptor_count: 8 },
            vk::DescriptorPoolSize { ty: T::SAMPLER, descriptor_count: 2 },
            vk::DescriptorPoolSize { ty: T::STORAGE_BUFFER, descriptor_count: 4 },
        ];
        let pool = unsafe {
            dev.create_descriptor_pool(&vk::DescriptorPoolCreateInfo::default().max_sets(4).pool_sizes(&sizes), None)
        }?;
        let alloc = |layout: vk::DescriptorSetLayout| -> Result<vk::DescriptorSet, GpuError> {
            let layouts = [layout];
            Ok(unsafe {
                dev.allocate_descriptor_sets(
                    &vk::DescriptorSetAllocateInfo::default().descriptor_pool(pool).set_layouts(&layouts),
                )
            }?[0])
        };
        let sim_sets = [alloc(sim_layout)?, alloc(sim_layout)?];
        let draw_sets = [alloc(draw_layout)?, alloc(draw_layout)?];

        let general = vk::ImageLayout::GENERAL;
        for (i, set) in sim_sets.iter().enumerate() {
            let (from, to) = if i == 0 { (0, 1) } else { (1, 0) };
            write_buffer(gpu, *set, 0, T::UNIFORM_BUFFER, &atmos);
            write_image(gpu, *set, 1, T::SAMPLED_IMAGE, state[from].view, general);
            write_image(gpu, *set, 2, T::SAMPLED_IMAGE, flow[from].view, general);
            write_image(gpu, *set, 3, T::STORAGE_IMAGE, state[to].view, general);
            write_image(gpu, *set, 4, T::STORAGE_IMAGE, flow[to].view, general);
            let info = [vk::DescriptorImageInfo { sampler, image_view: vk::ImageView::null(), image_layout: vk::ImageLayout::UNDEFINED }];
            let write = [vk::WriteDescriptorSet::default().dst_set(*set).dst_binding(5).descriptor_type(T::SAMPLER).image_info(&info)];
            unsafe { dev.update_descriptor_sets(&write, &[]) };
            write_buffer(gpu, *set, 6, T::STORAGE_BUFFER, &disturbers_buf);
            write_buffer(gpu, *set, 7, T::STORAGE_BUFFER, &storms_buf);
            write_image(gpu, *set, 8, T::STORAGE_IMAGE, noise.view, general);
        }
        for set in draw_sets {
            write_image(gpu, set, 1, T::SAMPLED_IMAGE, noise.view, general);
            write_image(gpu, set, 5, T::STORAGE_IMAGE, shade.view, general);
        }

        let sim_module = gpu.shader(include_bytes!(concat!(env!("OUT_DIR"), "/clouds_sim.spv")))?;
        let draw_module = gpu.shader(include_bytes!(concat!(env!("OUT_DIR"), "/clouds.spv")))?;
        let advect = pipelines::compute_pipeline(gpu, sim_module, c"cs_advect", sim_pipeline_layout)?;
        let force = pipelines::compute_pipeline(gpu, sim_module, c"cs_force", sim_pipeline_layout)?;
        let noise_pipeline = pipelines::compute_pipeline(gpu, sim_module, c"cs_noise", sim_pipeline_layout)?;
        let shade_pipeline = pipelines::compute_pipeline(gpu, draw_module, c"cs_shade", draw_pipeline_layout)?;
        let graphics = |fs, layout, pass, blend, depth| {
            pipelines::graphics_pipeline(
                gpu,
                &PipelineDesc {
                    module: draw_module,
                    vs: c"vs_fullscreen",
                    fs,
                    layout,
                    pass,
                    vertex: VertexKind::None,
                    blend,
                    depth,
                    cull: vk::CullModeFlags::NONE,
                },
            )
        };
        // The sky needs only set 0: the scene's own layout, drawn inside the scene pass
        // where the depth it tests against is still being written.
        let sky_pipeline = graphics(c"fs_sky", layouts.scene, passes.scene, Blend::Opaque, Depth::Test)?;
        let march_pipeline = graphics(c"fs_march", draw_pipeline_layout, passes.cloud_march, Blend::Opaque, Depth::Off)?;
        let resolve_pipeline = graphics(c"fs_resolve", draw_pipeline_layout, passes.bloom_down, Blend::Opaque, Depth::Off)?;
        let composite_pipeline =
            graphics(c"fs_composite", draw_pipeline_layout, passes.scene_over, Blend::Premultiplied, Depth::Off)?;
        let rain_pipeline = pipelines::graphics_pipeline(
            gpu,
            &PipelineDesc {
                module: draw_module,
                vs: c"vs_rain",
                fs: c"fs_rain",
                layout: draw_pipeline_layout,
                pass: passes.scene_over,
                vertex: VertexKind::None,
                blend: Blend::Premultiplied,
                depth: Depth::Test,
                cull: vk::CullModeFlags::NONE,
            },
        )?;

        // Bake the cloud noise once.
        gpu.submit_once(|cmd| unsafe {
            dev.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::COMPUTE, noise_pipeline);
            dev.cmd_bind_descriptor_sets(cmd, vk::PipelineBindPoint::COMPUTE, sim_pipeline_layout, 0, &[sim_sets[0]], &[]);
            let n = NOISE_RES / 4;
            dev.cmd_dispatch(cmd, n, n, n);
        })?;

        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let heading = rng.range(0.0, std::f32::consts::TAU);
        let wind = Vec2::new(heading.cos(), heading.sin()) * rng.range(0.8, 1.2);
        let area = map_size.x * map_size.y;
        let usual_storms = (area / (11000.0 * 11000.0)).clamp(1.0, 4.0);
        let parked_storm = std::env::var("MERIDIAN_WEATHER")
            .is_ok_and(|v| v == "storm")
            .then(|| map_size * 0.5 + Vec2::new(1500.0, 1200.0));
        let readback = gpu.host_buffer(16 * 8, vk::BufferUsageFlags::TRANSFER_DST)?;

        let mut sky = Sky {
            state,
            flow,
            noise,
            shade,
            targets: Vec::new(),
            target_fbs: Vec::new(),
            march_size: (0, 0),
            atmos,
            disturbers_buf,
            storms_buf,
            sampler,
            pool,
            sim_layout,
            draw_layout,
            sim_pipeline_layout,
            draw_pipeline_layout,
            sim_sets,
            draw_sets,
            advect,
            force,
            noise_pipeline,
            shade_pipeline,
            sky_pipeline,
            march_pipeline,
            resolve_pipeline,
            composite_pipeline,
            rain_pipeline,
            modules: [sim_module, draw_module],
            march_pass: passes.cloud_march,
            resolve_pass: passes.bloom_down,
            frame_index: 0,
            prev_view_proj: None,
            history_valid: false,
            map_size,
            base: CLOUD_BASE,
            floor,
            floor_image,
            floor_range: (floor_min, floor_max),
            wind,
            drift: Vec2::new(rng.range(0.0, 50_000.0), rng.range(0.0, 50_000.0)),
            storms: Vec::new(),
            flashes: Vec::new(),
            flyers: Vec::new(),
            units: Vec::new(),
            blasts: Vec::new(),
            clears: Vec::new(),
            clear_strength: 0.0,
            rng,
            last_time: None,
            step: 0.0,
            reset: true,
            usual_storms,
            parked_storm,
            weather: Weather::default(),
            hour: mc_data::weather::TimeOfDay::default().hour(),
            readback,
            readback_pending: false,
            rain_here: 0.0,
            thunder: Vec::new(),
            focus_texel: (0, 0),
            clouds: std::env::var("MERIDIAN_CLOUDS").map_or(true, |v| v != "0"),
        };
        // `MERIDIAN_WEATHER` names a preset for testing (`storm` also parks one overhead).
        let preset = match std::env::var("MERIDIAN_WEATHER").unwrap_or_default().as_str() {
            "clear" => Some(WeatherPreset::Clear),
            "fair" => Some(WeatherPreset::Fair),
            "cloudy" => Some(WeatherPreset::Cloudy),
            "stormy" | "storm" => Some(WeatherPreset::Stormy),
            "overcast" => Some(WeatherPreset::Overcast),
            _ => None,
        };
        sky.set_weather(preset.map_or_else(Weather::default, Weather::from));
        // `MERIDIAN_HOUR` sets the time of day for testing.
        if let Some(hour) = std::env::var("MERIDIAN_HOUR").ok().and_then(|v| v.parse().ok()) {
            sky.set_hour(hour);
        }
        Ok(sky)
    }

    /// Plays the match in `weather`: new storms for it, and the sky started over.
    pub fn set_weather(&mut self, weather: Weather) {
        self.weather = weather;
        self.wind = self.wind.normalize_or(Vec2::X) * weather.wind.max(0.5);
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

    /// Parks a raging storm over `at` until told otherwise (none: lets the
    /// weather run by itself again). The sky starts over with it in place.
    pub fn park_storm(&mut self, at: Option<Vec2>) {
        self.storms.retain(|s| s.life < 1.0e8);
        self.parked_storm = at;
        self.push_parked_storm();
        self.reset = true;
    }

    fn push_parked_storm(&mut self) {
        if let Some(at) = self.parked_storm {
            let mut s = self.new_storm();
            s.pos = at;
            s.radius = 2600.0;
            s.peak = 1.0;
            s.life = 1.0e9;
            s.age = s.life * 0.5;
            s.vel = Vec2::ZERO;
            self.storms.push(s);
        }
    }

    /// Plays the match at `hour` (24-hour clock).
    pub fn set_hour(&mut self, hour: f32) {
        self.hour = hour.rem_euclid(24.0);
    }

    /// How far the day has gone for lamps: 0 in daylight, 1 once the sun is
    /// down, already rising through a low evening sun.
    pub fn darkness(&self) -> f32 {
        1.0 - smoothstep(-0.06, 0.22, sun_at_hour(self.hour).z)
    }

    /// Where the scene's light comes from now: the sun, or the moon at night.
    pub fn light_direction(&self) -> Vec3 {
        light_at_hour(self.hour)
    }

    fn target_storms(&self) -> usize {
        ((self.usual_storms * self.weather.storms).round() as usize).min(MAX_STORMS - 1)
    }

    /// Which way the wind blows over the ground, a unit vector.
    pub fn wind_heading(&self) -> Vec2 {
        self.wind.normalize_or(Vec2::X)
    }

    /// How hard it is raining where the camera looks, 0 to 1 (a frame late).
    pub fn rain_here(&self) -> f32 {
        self.rain_here
    }

    /// Lightning since the last call, for thunder.
    pub fn take_thunder(&mut self) -> Vec<Thunder> {
        std::mem::take(&mut self.thunder)
    }

    /// The weather map (scene set binding 21) and atmosphere (binding 22).
    pub fn weather_view(&self) -> vk::ImageView {
        self.state[0].view
    }

    /// The cloud floor (scene set binding 24).
    pub fn shade_view(&self) -> vk::ImageView {
        self.shade.view
    }

    pub fn floor_view(&self) -> vk::ImageView {
        self.floor_image.view
    }

    /// The cloud floor under `xy`, bilinear like the shaders' `cloud_floor`.
    fn floor_at(&self, xy: Vec2) -> f32 {
        let n = FLOOR_RES as usize;
        let p = (xy / self.map_size * FLOOR_RES as f32 - 0.5).clamp(Vec2::ZERO, Vec2::splat(FLOOR_RES as f32 - 1.001));
        let (x, y) = (p.x as usize, p.y as usize);
        let f = p - Vec2::new(x as f32, y as f32);
        let at = |x: usize, y: usize| self.floor[y.min(n - 1) * n + x.min(n - 1)];
        let top = at(x, y) + (at(x + 1, y) - at(x, y)) * f.x;
        let bottom = at(x, y + 1) + (at(x + 1, y + 1) - at(x, y + 1)) * f.x;
        let floor = top + (bottom - top) * f.y;
        // Past the edge it eases down to the lowest floor, as the shaders' does.
        let out = (-xy).max(xy - self.map_size).max(Vec2::ZERO).length();
        floor + (self.floor_range.0 - floor) * smoothstep(0.0, 2500.0, out)
    }

    /// The weather's flow and aircraft wakes (scene set binding 23).
    pub fn flow_view(&self) -> vk::ImageView {
        self.flow[0].view
    }

    pub fn atmosphere_buffer(&self) -> &Buffer {
        &self.atmos
    }

    /// Size-dependent targets: the march, its two histories, and the depth it stops at.
    pub fn resize(&mut self, gpu: &Gpu, width: u32, height: u32, depth: vk::ImageView) -> Result<(), GpuError> {
        unsafe {
            for fb in self.target_fbs.drain(..) {
                gpu.device.destroy_framebuffer(fb, None);
            }
        }
        for old in self.targets.drain(..) {
            gpu.destroy_image(old);
        }
        let div = march_divisor();
        let (w, h) = (width.div_ceil(div).max(1), height.div_ceil(div).max(1));
        for k in 0..3 {
            let image = gpu.image(&ImageDesc {
                width: w,
                height: h,
                format: if k == 0 { CLOUD_MARCH_FORMAT } else { HDR_FORMAT },
                usage: vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED,
                layers: 1,
                mips: 1,
                array: false,
            })?;
            let views = [image.view];
            let fb = unsafe {
                gpu.device.create_framebuffer(
                    &vk::FramebufferCreateInfo::default()
                        .render_pass(if k == 0 { self.march_pass } else { self.resolve_pass })
                        .attachments(&views)
                        .width(w)
                        .height(h)
                        .layers(1),
                    None,
                )
            }?;
            // Give the target its sampled layout before anything reads it.
            gpu.submit_once(|cmd| unsafe {
                gpu.transition(
                    cmd,
                    image.image,
                    vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0,
                        level_count: 1,
                        base_array_layer: 0,
                        layer_count: 1,
                    },
                    vk::ImageLayout::UNDEFINED,
                    vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                );
            })?;
            self.targets.push(image);
            self.target_fbs.push(fb);
        }
        let read = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
        let sampled = vk::DescriptorType::SAMPLED_IMAGE;
        for (k, set) in self.draw_sets.iter().enumerate() {
            write_image(gpu, *set, 0, sampled, depth, vk::ImageLayout::DEPTH_STENCIL_READ_ONLY_OPTIMAL);
            write_image(gpu, *set, 2, sampled, self.targets[0].view, read);
            write_image(gpu, *set, 3, sampled, self.targets[1 + (1 - k)].view, read);
            write_image(gpu, *set, 4, sampled, self.targets[1 + k].view, read);
        }
        self.march_size = (w, h);
        self.history_valid = false;
        Ok(())
    }

    fn new_storm(&mut self) -> Storm {
        let margin = 0.1;
        let pos = Vec2::new(
            self.rng.range(margin, 1.0 - margin) * self.map_size.x,
            self.rng.range(margin, 1.0 - margin) * self.map_size.y,
        );
        let drift = Vec2::new(self.rng.range(-3.0, 3.0), self.rng.range(-3.0, 3.0));
        Storm {
            pos,
            vel: self.wind * self.rng.range(0.5, 0.9) + drift,
            // Now and then a vast one: a supercell tens of kilometres across.
            radius: self.rng.range(1300.0, 2800.0)
                * self.weather.scale.max(0.5)
                * if self.rng.next() < 0.15 * self.weather.scale { 2.2 } else { 1.0 },
            peak: self.rng.range(0.7, 1.0),
            age: 0.0,
            life: self.rng.range(240.0, 600.0),
            next_flash: self.rng.range(1.0, 6.0),
        }
    }

    /// Where each of a new tick's units stands.
    pub fn set_units(&mut self, units: impl Iterator<Item = Option<Vec2>>) {
        self.units.clear();
        self.units.extend(units);
    }

    /// Clearings over the selection: its units gathered into up to eight
    /// groups, each a centre and radius.
    fn selection_zones(&self, selected: &[u32]) -> Vec<Vec4> {
        let mut zones: Vec<(Vec2, Vec2, u32)> = Vec::new();
        for &i in selected {
            let Some(&Some(p)) = self.units.get(i as usize) else { continue };
            let near = zones.iter().position(|(lo, hi, _)| ((*lo + *hi) * 0.5).distance(p) < 1400.0);
            match near {
                Some(k) => {
                    let (lo, hi, n) = &mut zones[k];
                    *lo = lo.min(p);
                    *hi = hi.max(p);
                    *n += 1;
                }
                None if zones.len() < MAX_CLEARS => zones.push((p, p, 1)),
                None => {}
            }
        }
        zones
            .into_iter()
            .map(|(lo, hi, _)| {
                let c = (lo + hi) * 0.5;
                Vec4::new(c.x, c.y, (hi - lo).length() * 0.5, 1.0)
            })
            .collect()
    }

    /// Aircraft, from a new tick's units: where each was last tick and is now.
    pub fn set_flyers(&mut self, flyers: impl Iterator<Item = (Vec3, Vec3, f32)>) {
        self.flyers.clear();
        self.flyers.extend(flyers.map(|(prev, pos, radius)| Flyer { prev, pos, radius }));
    }

    /// A blast big enough to shove the clouds about: its reach in metres and
    /// how hard it hits (1 a large explosion, 3 a reactor going up).
    /// A blast below the layer reaches it only if it is big for the gap: a
    /// shell or a tank going up on the ground leaves the cloud a couple of
    /// hundred metres overhead alone; an aircraft blowing up inside it, or a
    /// reactor, does not.
    pub fn blast(&mut self, at: Vec3, reach: f32, strength: f32, time: f32) {
        let gap = (self.floor_at(at.truncate()) + self.base - at.z).max(0.0);
        let fade = 1.0 - smoothstep(0.0, reach * 0.5, gap);
        if fade < 0.05 {
            return;
        }
        if self.blasts.len() < 48 {
            self.blasts.push((at.truncate(), reach * (0.5 + 0.5 * fade), strength * fade, time));
        }
    }

    /// Steps the weather and writes this frame's uniforms. Call once per frame,
    /// before recording.
    pub fn update(&mut self, frame: &SkyFrame) {
        let dt = match self.last_time {
            Some(last) => (frame.time - last).clamp(0.0, 0.1),
            None => 0.0,
        };
        self.last_time = Some(frame.time);
        self.step = dt;
        let now = frame.time;

        // The wind veers slowly; the air mass drifts with it.
        let veer = (now * 0.004).sin() * 0.0006 * dt;
        self.wind = Vec2::from_angle(veer).rotate(self.wind);
        self.drift += self.wind * dt;

        // Storms drift, build, rage and die; new ones form to keep the count.
        for s in &mut self.storms {
            s.age += dt;
            s.pos += s.vel * dt;
        }
        let size = self.map_size;
        self.storms.retain(|s| {
            s.age < s.life && s.pos.x > -s.radius * 2.0 && s.pos.y > -s.radius * 2.0
                && s.pos.x < size.x + s.radius * 2.0 && s.pos.y < size.y + s.radius * 2.0
        });
        let natural = self.storms.iter().filter(|s| s.life < 1.0e8).count();
        if natural < self.target_storms() && self.rng.next() < dt / 20.0 {
            let s = self.new_storm();
            self.storms.push(s);
        }

        // Lightning in the strong ones.
        self.flashes.retain(|f| now - f.start < Flash::LIFE);
        for i in 0..self.storms.len() {
            let strength = self.storms[i].strength();
            self.storms[i].next_flash -= dt * smoothstep(0.6, 0.9, strength);
            if self.storms[i].next_flash > 0.0 || self.flashes.len() >= MAX_FLASHES {
                continue;
            }
            // Now and then, not a strobe: about one flash a storm every 25 s.
            let rate = if self.storms[i].life > 1.0e8 { 1.8 } else { 25.0 / self.weather.lightning.max(0.01) };
            if self.weather.lightning <= 0.0 && self.storms[i].life < 1.0e8 {
                self.storms[i].next_flash = 1.0e9;
                continue;
            }
            self.storms[i].next_flash = -rate * (1.0 - self.rng.next()).max(1e-3).ln();
            let (pos, radius) = (self.storms[i].pos, self.storms[i].radius);
            let angle = self.rng.range(0.0, std::f32::consts::TAU);
            let at = pos + Vec2::from_angle(angle) * radius * self.rng.range(0.0, 0.55);
            let height = self.floor_at(at) + self.base + self.rng.range(500.0, 2200.0);
            let bolt = (self.rng.next() < 0.45).then(|| at + Vec2::new(self.rng.range(-500.0, 500.0), self.rng.range(-500.0, 500.0)));
            let mut strokes = [(0.0, 0.0); 4];
            let mut at_t = 0.0;
            for (k, s) in strokes.iter_mut().enumerate() {
                *s = (at_t, if k == 0 { 1.0 } else { self.rng.range(0.3, 0.9) });
                at_t += self.rng.range(0.05, 0.16);
            }
            let seed = self.rng.range(0.0, 1000.0);
            self.thunder.push(Thunder {
                pos: bolt.map_or(at.extend(height), |g| g.extend(0.0)),
                strength: strokes.iter().map(|s| s.1).sum::<f32>() * 0.5,
                bolt: bolt.is_some(),
            });
            self.flashes.push(Flash { pos: at.extend(height), start: now, bolt, seed, strokes });
        }

        // The rain round the camera's focus, from last frame's copy of the map.
        let camera = frame.camera;
        if self.readback_pending {
            let mut bytes = [0u8; 16 * 8];
            self.readback.read(0, &mut bytes);
            let mut rain = 0.0;
            for texel in bytes.chunks_exact(8) {
                rain += f16_to_f32(u16::from_le_bytes([texel[6], texel[7]])).clamp(0.0, 1.0);
            }
            let rain = rain / 16.0;
            // Eased, so the sound swells and dies away rather than stepping.
            let k = 1.0 - (-dt / 1.5).exp();
            self.rain_here += (rain - self.rain_here) * k;
        }
        let texel = (camera.focus.truncate() / self.map_size * WEATHER_RES as f32).floor();
        self.focus_texel = (
            (texel.x.max(0.0) as u32).min(WEATHER_RES - 4),
            (texel.y.max(0.0) as u32).min(WEATHER_RES - 4),
        );
        let mut atmos = Atmosphere::zeroed();
        let (light, dark) = lighting_at_hour(self.hour);
        // w: how brightly the sky dome and clouds take that light. The
        // moonlight is day-for-night, too strong to light a believable night sky.
        atmos.sun_color = light.sun.extend(1.0 - 0.96 * dark).to_array();
        // w: brightness of the disk (the sun's, or a dimmer moon's).
        atmos.sky_color = light.sky.extend(24.0 - 16.0 * dark).to_array();
        // w: how dark it is, for the stars.
        atmos.horizon_color = light.horizon.extend(dark).to_array();
        atmos.ground_color = light.ground.extend(0.0).to_array();
        atmos.wind = [self.drift.x, self.drift.y, self.wind.x, self.wind.y];
        let w = self.weather;
        // Heights above the cloud floor; the floor's range rides in frame.w and shape.w.
        atmos.layer = [self.base, self.base + CLOUD_DECK, self.base + 2600.0 + 4200.0 * w.towering, w.cover];
        atmos.shape = [w.towering, w.scale, w.rain, self.floor_range.1];
        atmos.weather = [self.map_size.x / WEATHER_RES as f32, self.map_size.x, self.map_size.y, now];
        // Mostly, not wholly: a veil stays, so the weather still reads overhead.
        let reach = (camera.distance * 0.22 + 100.0).min(1200.0 + camera.distance * 0.1);
        // Off: the middle of the screen is see-through in the composite instead
        // (a hologram of the cloud), so nothing is taken out of the sky there.
        // z: the camera's distance, for the bubble of clear air round the eye.
        let _ = reach;
        atmos.view = [camera.focus.x, camera.focus.y, camera.distance, 0.0];
        let view_proj = camera.view_proj();
        self.frame_index = self.frame_index.wrapping_add(1);
        atmos.prev_view_proj = self.prev_view_proj.unwrap_or(view_proj).to_cols_array_2d();
        atmos.frame = [
            (self.frame_index % 1024) as f32,
            (self.history_valid && self.prev_view_proj.is_some()) as u32 as f32,
            // The see-through middle of the screen: only while something is selected.
            self.clear_strength,
            self.floor_range.0,
        ];
        self.prev_view_proj = Some(view_proj);
        self.history_valid = true;

        // Clear zones over the selection ease in and out.
        let want = !frame.selected.is_empty();
        self.clear_strength = if want {
            (self.clear_strength + dt / 0.35).min(1.0)
        } else {
            (self.clear_strength - dt / 0.6).max(0.0)
        };
        if want {
            self.clears = self.selection_zones(frame.selected);
        }
        let zones = if self.clear_strength > 0.0 { self.clears.len().min(MAX_CLEARS) } else { 0 };
        for (slot, c) in atmos.clears.iter_mut().zip(&self.clears).take(zones) {
            let margin = 220.0 + camera.distance * 0.06;
            *slot = [c.x, c.y, c.z + margin, self.clear_strength];
        }

        let mut flashes = 0;
        for f in &self.flashes {
            let b = f.brightness(now);
            atmos.flashes[flashes] = f.pos.extend(b).to_array();
            atmos.bolts[flashes] = match f.bolt {
                Some(g) => [g.x, g.y, f.seed, b],
                None => [0.0; 4],
            };
            flashes += 1;
        }

        // Stirrers: aircraft carried forward between ticks, blasts, hulls.
        let mut disturbers: Vec<Disturber> = Vec::with_capacity(MAX_DISTURBERS);
        let tick = frame.tick_seconds.max(0.02);
        for f in &self.flyers {
            let pos = f.prev.lerp(f.pos, frame.alpha);
            // Only what flies in the cloud stirs it: a gunship hugging the
            // ground a hundred metres under the base leaves it alone.
            let floor = self.floor_at(pos.truncate());
            let gap = (floor + atmos.layer[0] - pos.z).max(pos.z - floor - atmos.layer[2]).max(0.0);
            let within = 1.0 - smoothstep(0.0, 60.0, gap);
            if within < 0.02 {
                continue;
            }
            let vel = (f.pos - f.prev) / tick;
            let speed = vel.truncate().length();
            // Capital ships leave vapor particles and never modify the weather field.
            if f.radius >= 24.0 { continue; }
            let (kind,radius,strength) = (KIND_WAKE,(f.radius*3.0).clamp(24.0,50.0),
                (smoothstep(8.0,60.0,speed)*0.85+0.15)*within);
            disturbers.push(Disturber {
                a: [pos.x, pos.y, radius, kind],
                b: [vel.x, vel.y, strength, 0.0],
                c: [pos.z, 0.0, 0.0, 0.0],
            });
            if disturbers.len() >= MAX_DISTURBERS - 24 {
                break;
            }
        }
        self.blasts.retain(|b| now - b.3 < 6.0);
        for &(at, reach, strength, start) in &self.blasts {
            if disturbers.len() >= MAX_DISTURBERS {
                break;
            }
            disturbers.push(Disturber {
                a: [at.x, at.y, reach, KIND_BLAST],
                b: [0.0, 0.0, strength, now - start],
                c: [0.0; 4],
            });
        }
        let storms: Vec<[f32; 4]> = self
            .storms
            .iter()
            .take(MAX_STORMS)
            .map(|s| [s.pos.x, s.pos.y, s.radius, s.strength()])
            .collect();
        atmos.counts = [zones as f32, flashes as f32, disturbers.len() as f32, storms.len() as f32];
        self.atmos.write(0, bytemuck::bytes_of(&atmos));
        self.disturbers_buf.write(0, bytemuck::cast_slice(&disturbers));
        self.storms_buf.write(0, bytemuck::cast_slice(&storms));
    }

    /// The weather step: before anything that reads the weather map.
    pub fn record_sim(&mut self, gpu: &Gpu, cmd: vk::CommandBuffer) {
        let dev = &gpu.device;
        let groups = WEATHER_RES.div_ceil(8);
        let barrier = |dst_stage: vk::PipelineStageFlags| unsafe {
            let b = [vk::MemoryBarrier::default()
                .src_access_mask(vk::AccessFlags::SHADER_WRITE | vk::AccessFlags::SHADER_READ)
                .dst_access_mask(vk::AccessFlags::SHADER_READ | vk::AccessFlags::SHADER_WRITE)];
            dev.cmd_pipeline_barrier(cmd, vk::PipelineStageFlags::COMPUTE_SHADER, dst_stage, vk::DependencyFlags::empty(), &b, &[], &[]);
        };
        unsafe {
            let push = |reset: u32| {
                let data: [u32; 2] = [self.step.to_bits(), reset];
                dev.cmd_push_constants(cmd, self.sim_pipeline_layout, vk::ShaderStageFlags::COMPUTE, 0, bytemuck::bytes_of(&data));
            };
            if !self.reset {
                dev.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::COMPUTE, self.advect);
                dev.cmd_bind_descriptor_sets(cmd, vk::PipelineBindPoint::COMPUTE, self.sim_pipeline_layout, 0, &[self.sim_sets[0]], &[]);
                push(0);
                dev.cmd_dispatch(cmd, groups, groups, 1);
                barrier(vk::PipelineStageFlags::COMPUTE_SHADER);
            }
            dev.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::COMPUTE, self.force);
            dev.cmd_bind_descriptor_sets(cmd, vk::PipelineBindPoint::COMPUTE, self.sim_pipeline_layout, 0, &[self.sim_sets[1]], &[]);
            push(self.reset as u32);
            dev.cmd_dispatch(cmd, groups, groups, 1);
            barrier(
                vk::PipelineStageFlags::VERTEX_SHADER
                    | vk::PipelineStageFlags::FRAGMENT_SHADER
                    | vk::PipelineStageFlags::COMPUTE_SHADER
                    | vk::PipelineStageFlags::TRANSFER,
            );
            // A 4x4 block round the focus, for the rain's sound next frame.
            let copy = [vk::BufferImageCopy::default()
                .image_subresource(vk::ImageSubresourceLayers {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    mip_level: 0,
                    base_array_layer: 0,
                    layer_count: 1,
                })
                .image_offset(vk::Offset3D { x: self.focus_texel.0 as i32, y: self.focus_texel.1 as i32, z: 0 })
                .image_extent(vk::Extent3D { width: 4, height: 4, depth: 1 })];
            dev.cmd_copy_image_to_buffer(cmd, self.state[0].image, vk::ImageLayout::GENERAL, self.readback.buffer, &copy);
            let host = [vk::MemoryBarrier::default()
                .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                .dst_access_mask(vk::AccessFlags::HOST_READ)];
            dev.cmd_pipeline_barrier(cmd, vk::PipelineStageFlags::TRANSFER, vk::PipelineStageFlags::HOST, vk::DependencyFlags::empty(), &host, &[], &[]);
        }
        self.readback_pending = true;
        self.reset = false;
    }

    /// The clouds' shade on the land: after `record_sim`, before anything is lit.
    pub fn record_shade(&self, gpu: &Gpu, cmd: vk::CommandBuffer, scene_set: vk::DescriptorSet) {
        if !self.clouds {
            return;
        }
        let dev = &gpu.device;
        let groups = SHADE_RES.div_ceil(8);
        let shaders = vk::PipelineStageFlags::VERTEX_SHADER | vk::PipelineStageFlags::FRAGMENT_SHADER | vk::PipelineStageFlags::COMPUTE_SHADER;
        unsafe {
            // Last frame's lighting read it; this dispatch reads and rewrites it.
            let before = [vk::MemoryBarrier::default()
                .src_access_mask(vk::AccessFlags::SHADER_READ)
                .dst_access_mask(vk::AccessFlags::SHADER_READ | vk::AccessFlags::SHADER_WRITE)];
            dev.cmd_pipeline_barrier(cmd, shaders, vk::PipelineStageFlags::COMPUTE_SHADER, vk::DependencyFlags::empty(), &before, &[], &[]);
            dev.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::COMPUTE, self.shade_pipeline);
            dev.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::COMPUTE,
                self.draw_pipeline_layout,
                0,
                &[scene_set, self.draw_sets[0]],
                &[],
            );
            dev.cmd_dispatch(cmd, groups, groups, 1);
            let after = [vk::MemoryBarrier::default()
                .src_access_mask(vk::AccessFlags::SHADER_WRITE)
                .dst_access_mask(vk::AccessFlags::SHADER_READ)];
            dev.cmd_pipeline_barrier(cmd, vk::PipelineStageFlags::COMPUTE_SHADER, shaders, vk::DependencyFlags::empty(), &after, &[], &[]);
        }
    }

    /// The sky behind everything: inside the scene pass, set 0 bound, after the opaque scene.
    pub fn draw_sky(&self, gpu: &Gpu, cmd: vk::CommandBuffer) {
        unsafe {
            gpu.device.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.sky_pipeline);
            gpu.device.cmd_draw(cmd, 3, 1, 0, 0);
        }
    }

    fn history(&self) -> usize {
        (self.frame_index & 1) as usize
    }

    /// The cloud march and its fold into the history, between the scene pass and `scene_over`.
    pub fn record_march(&self, gpu: &Gpu, cmd: vk::CommandBuffer, scene_set: vk::DescriptorSet) {
        let (w, h) = self.march_size;
        if w == 0 || !self.clouds {
            return;
        }
        let dev = &gpu.device;
        let k = self.history();
        let set = self.draw_sets[k];
        for (fb, pass, pipeline) in [
            (self.target_fbs[0], self.march_pass, self.march_pipeline),
            (self.target_fbs[1 + k], self.resolve_pass, self.resolve_pipeline),
        ] {
            unsafe {
                dev.cmd_begin_render_pass(
                    cmd,
                    &vk::RenderPassBeginInfo::default()
                        .render_pass(pass)
                        .framebuffer(fb)
                        .render_area(vk::Rect2D { offset: vk::Offset2D::default(), extent: vk::Extent2D { width: w, height: h } }),
                    vk::SubpassContents::INLINE,
                );
                dev.cmd_set_viewport(cmd, 0, &[vk::Viewport { x: 0.0, y: 0.0, width: w as f32, height: h as f32, min_depth: 0.0, max_depth: 1.0 }]);
                dev.cmd_set_scissor(cmd, 0, &[vk::Rect2D { offset: vk::Offset2D::default(), extent: vk::Extent2D { width: w, height: h } }]);
                dev.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, pipeline);
                dev.cmd_bind_descriptor_sets(cmd, vk::PipelineBindPoint::GRAPHICS, self.draw_pipeline_layout, 0, &[scene_set, set], &[]);
                dev.cmd_draw(cmd, 3, 1, 0, 0);
                dev.cmd_end_render_pass(cmd);
            }
        }
    }

    /// Rain falling round the camera when zoomed in: inside `scene_over`, depth-tested.
    pub fn draw_rain(&self, gpu: &Gpu, cmd: vk::CommandBuffer, scene_set: vk::DescriptorSet) {
        if self.targets.is_empty() || self.weather.rain <= 0.0 {
            return;
        }
        unsafe {
            let dev = &gpu.device;
            dev.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.rain_pipeline);
            dev.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                self.draw_pipeline_layout,
                0,
                &[scene_set, self.draw_sets[self.history()]],
                &[],
            );
            dev.cmd_draw(cmd, 6, RAIN_DROPS, 0, 0);
        }
    }

    /// The clouds over the picture: inside `scene_over`, full-size viewport set.
    /// Leaves set 1 bound to its own layout; set 0 stays the scene set.
    pub fn draw_composite(&self, gpu: &Gpu, cmd: vk::CommandBuffer, scene_set: vk::DescriptorSet) {
        if self.targets.is_empty() || !self.clouds {
            return;
        }
        unsafe {
            let dev = &gpu.device;
            dev.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::GRAPHICS, self.composite_pipeline);
            dev.cmd_bind_descriptor_sets(
                cmd,
                vk::PipelineBindPoint::GRAPHICS,
                self.draw_pipeline_layout,
                0,
                &[scene_set, self.draw_sets[self.history()]],
                &[],
            );
            dev.cmd_draw(cmd, 3, 1, 0, 0);
        }
    }

    pub fn destroy(&mut self, gpu: &Gpu) {
        unsafe {
            let dev = &gpu.device;
            for p in [
                self.advect,
                self.force,
                self.noise_pipeline,
                self.shade_pipeline,
                self.sky_pipeline,
                self.march_pipeline,
                self.resolve_pipeline,
                self.composite_pipeline,
                self.rain_pipeline,
            ] {
                dev.destroy_pipeline(p, None);
            }
            for m in self.modules {
                dev.destroy_shader_module(m, None);
            }
            dev.destroy_pipeline_layout(self.sim_pipeline_layout, None);
            dev.destroy_pipeline_layout(self.draw_pipeline_layout, None);
            dev.destroy_descriptor_pool(self.pool, None);
            dev.destroy_descriptor_set_layout(self.sim_layout, None);
            dev.destroy_descriptor_set_layout(self.draw_layout, None);
            dev.destroy_sampler(self.sampler, None);
            for fb in self.target_fbs.drain(..) {
                dev.destroy_framebuffer(fb, None);
            }
        }
        for image in self.state.iter().chain(&self.flow).chain([&self.noise, &self.shade, &self.floor_image]).chain(&self.targets) {
            gpu.destroy_image_ref(image);
        }
        for b in [&mut self.atmos, &mut self.disturbers_buf, &mut self.storms_buf, &mut self.readback] {
            gpu.destroy_buffer(std::mem::replace(b, Buffer::null()));
        }
    }
}

fn write_image(gpu: &Gpu, set: vk::DescriptorSet, binding: u32, ty: vk::DescriptorType, view: vk::ImageView, layout: vk::ImageLayout) {
    let info = [vk::DescriptorImageInfo { sampler: vk::Sampler::null(), image_view: view, image_layout: layout }];
    let write = [vk::WriteDescriptorSet::default().dst_set(set).dst_binding(binding).descriptor_type(ty).image_info(&info)];
    unsafe { gpu.device.update_descriptor_sets(&write, &[]) };
}

fn write_buffer(gpu: &Gpu, set: vk::DescriptorSet, binding: u32, ty: vk::DescriptorType, buffer: &Buffer) {
    let info = [buffer.info()];
    let write = [vk::WriteDescriptorSet::default().dst_set(set).dst_binding(binding).descriptor_type(ty).buffer_info(&info)];
    unsafe { gpu.device.update_descriptor_sets(&write, &[]) };
}

/// The 3D cloud noise, 128 texels a side, RGBA8.
fn noise_image(gpu: &Gpu) -> Result<Image, GpuError> {
    let dev = &gpu.device;
    let format = vk::Format::R8G8B8A8_UNORM;
    let info = vk::ImageCreateInfo::default()
        .image_type(vk::ImageType::TYPE_3D)
        .format(format)
        .extent(vk::Extent3D { width: NOISE_RES, height: NOISE_RES, depth: NOISE_RES })
        .mip_levels(1)
        .array_layers(1)
        .samples(vk::SampleCountFlags::TYPE_1)
        .tiling(vk::ImageTiling::OPTIMAL)
        .usage(vk::ImageUsageFlags::STORAGE | vk::ImageUsageFlags::SAMPLED)
        .initial_layout(vk::ImageLayout::UNDEFINED);
    unsafe {
        let image = dev.create_image(&info, None)?;
        let memory = gpu.allocate(dev.get_image_memory_requirements(image), vk::MemoryPropertyFlags::DEVICE_LOCAL)?;
        dev.bind_image_memory(image, memory, 0)?;
        let aspect = vk::ImageAspectFlags::COLOR;
        let view = dev.create_image_view(
            &vk::ImageViewCreateInfo::default()
                .image(image)
                .view_type(vk::ImageViewType::TYPE_3D)
                .format(format)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: aspect,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                }),
            None,
        )?;
        Ok(Image { image, view, memory, format, width: NOISE_RES, height: NOISE_RES, layers: 1, mips: 1, aspect })
    }
}

#[cfg(test)]
mod shots {
    use crate::camera::Camera;
    use crate::overlay::Overlay;
    use crate::renderer::{FrameInput, Renderer, SceneDesc, Target};
    use glam::Vec2;
    use mc_sim::mirror::RenderFrame;
    use std::sync::Arc;

    /// Renders the sky and weather headless. `SKY_SHOTS` = `name:x,y,dist,yaw,tilt[,seconds]; ...`,
    /// `SKY_MAP` the map (dev16), `SKY_OUT` the folder for the PPMs.
    #[test]
    #[ignore = "requires Vulkan and a map"]
    fn sky_shots() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let map_name = std::env::var("SKY_MAP").unwrap_or_else(|_| "dev16".into());
        let map = Arc::new(mc_map::MapFile::open(root.join(format!("maps/{map_name}.mcmap"))).expect("open map"));
        let blueprints = Arc::new(mc_data::Blueprints::load(&root.join("data")).unwrap());
        let (w, h) = if std::env::var("SKY_BIG").is_ok() { (2560u32, 1440u32) } else { (1600u32, 900u32) };
        let mut renderer = Renderer::new(
            Target::Headless { width: w, height: h },
            SceneDesc { map: map.clone(), blueprints, pool: Arc::new(mc_jobs::Pool::new(2)), team_colors: [[0.1, 0.6, 0.9]; 8] },
        )
        .unwrap();
        if let Some(hour) = std::env::var("SKY_HOUR").ok().and_then(|v| v.parse().ok()) {
            renderer.set_hour(hour);
        }
        // SKY_NORAIN: the overcast preset without its rain.
        if std::env::var("SKY_NORAIN").is_ok() {
            let mut w: mc_data::weather::Weather = mc_data::weather::WeatherPreset::Overcast.into();
            w.rain = 0.0;
            renderer.set_weather(w);
        }
        // SKY_CLEAR: the clear-weather preset (a starry night with few clouds).
        if std::env::var("SKY_CLEAR").is_ok() {
            renderer.set_weather(mc_data::weather::WeatherPreset::Clear.into());
        }
        let out = std::path::PathBuf::from(std::env::var("SKY_OUT").unwrap_or_else(|_| root.join("artifacts/sky").display().to_string()));
        std::fs::create_dir_all(&out).unwrap();
        let mut frame = RenderFrame::default();
        frame.props_dead = vec![0; map.props().len().div_ceil(32)];
        let overlay = Overlay::default();
        let spec = std::env::var("SKY_SHOTS").unwrap_or_else(|_| "far:8192,8192,20000,0.4,0".into());
        for shot in spec.split(';').filter(|s| !s.trim().is_empty()) {
            let (name, nums) = shot.trim().split_once(':').unwrap();
            let v: Vec<f32> = nums.split(',').map(|n| n.trim().parse().unwrap()).collect();
            let mut camera = Camera::new(Vec2::from(map.info().size_metres().to_f32()), Vec2::new(w as f32, h as f32));
            let xy = Vec2::new(v[0], v[1]);
            camera.focus = xy.extend(renderer.ground_height(xy));
            camera.distance = v[2];
            camera.yaw = v[3];
            camera.tilt = v[4];
            let seconds = v.get(5).copied().unwrap_or(2.0);
            let started = std::time::Instant::now();
            let mut t = 0.0;
            // Weather needs a few frames to settle; the first uploads the tick.
            // SKY_DT: seconds a frame (default 0.05); SKY_SEQ=N writes the last N frames.
            let step: f32 = std::env::var("SKY_DT").ok().and_then(|v| v.parse().ok()).unwrap_or(0.05);
            let seq: usize = std::env::var("SKY_SEQ").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
            let frames = (seconds / step).max(3.0) as usize;
            // SKY_STIR: a jet flying west to east through the view, a blast at the focus.
            let stir = std::env::var("SKY_STIR").is_ok();
            let mut sums: Vec<f32> = Vec::new();
            let mut previous: Option<Vec<u8>> = None;
            // SKY_PAN: metres a second the camera scrolls east, as a player would.
            let pan: f32 = std::env::var("SKY_PAN").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0);
            // SKY_TREE_BLAST: seconds into the shot a 60 m blast goes off at the focus (tree_wind.rs).
            let tree_blast: Option<f32> = std::env::var("SKY_TREE_BLAST").ok().and_then(|v| v.parse().ok());
            for i in 0..frames {
                camera.focus.x = xy.x + pan * t;
                // SKY_GROUND: the focus follows the ground as it pans, as the match camera does.
                if std::env::var("SKY_GROUND").is_ok() {
                    camera.focus.z = renderer.ground_height(camera.focus.truncate());
                    if i % 6 == 0 {
                        println!("{name}: frame {i} focus {:.0},{:.0} ground {:.1}", camera.focus.x, camera.focus.y, camera.focus.z);
                    }
                }
                if tree_blast.is_some_and(|at| (t - at).abs() < 0.025) {
                    renderer.cloud_blast(camera.focus, 60.0, 1.0);
                }
                // SKY_STIR=patrol: a flight of eight going up and down lanes
                // through the view and two big hulls creeping along, for as
                // long as the shot runs (how a match stirs the sky).
                if std::env::var("SKY_STIR").is_ok_and(|v| v == "patrol") {
                    let leg = 3000.0;
                    let mut flyers = Vec::new();
                    for k in 0..8 {
                        let s = (t * 90.0 + k as f32 * 700.0) % (2.0 * leg);
                        let (y, dir) = if s < leg { (s, 1.0) } else { (2.0 * leg - s, -1.0) };
                        let x = xy.x - 1400.0 + k as f32 * 400.0;
                        let at = glam::Vec3::new(x, xy.y - leg * 0.5 + y, 300.0);
                        flyers.push((at - glam::Vec3::Y * 9.0 * dir, at, 6.0));
                    }
                    for k in 0..2 {
                        let at = glam::Vec3::new(xy.x - 600.0 + k as f32 * 1200.0, xy.y - 800.0 + t * 5.0, 320.0);
                        flyers.push((at - glam::Vec3::Y * 0.5, at, 18.0));
                    }
                    renderer.sky_mut().set_flyers(flyers.into_iter());
                } else if stir {
                    let y = xy.y - 5000.0 + t * 180.0;
                    let at = glam::Vec3::new(xy.x + 1500.0, y, renderer.ground_height(glam::Vec2::new(xy.x + 1500.0, y)).max(0.0) + 260.0);
                    renderer.sky_mut().set_flyers([(at - glam::Vec3::Y * 9.0, at, 6.0)].into_iter());
                    if i == frames / 3 {
                        renderer.sky_mut().blast(glam::Vec3::new(xy.x + 3500.0, xy.y - 2500.0, 0.0), 900.0, 2.0, 100.0 + t);
                    }
                }
                renderer
                    .render(&FrameInput {
                        camera: &camera,
                        time: 100.0 + t,
                        alpha: 1.0,
                        sim: (i == 0).then_some(&frame),
                        ghosts: &[],
                        marks: &[],
                        ranges: &[],
                        ranges_drawn: 0,
                        overlay: &overlay,
                        build_grid: false,
                    })
                    .unwrap();
                t += step;
                if seq > 0 && i + seq >= frames {
                    let now = renderer.read_pixels().unwrap();
                    let mut ppm = format!("P6\n{w} {h}\n255\n").into_bytes();
                    for p in now.chunks_exact(4) {
                        ppm.extend_from_slice(&p[..3]);
                    }
                    std::fs::write(out.join(format!("{name}_seq{:02}.ppm", i + seq - frames)), ppm).unwrap();
                }
                // SKY_FLICKER: how much the last frames differ from each other with the camera still.
                if std::env::var("SKY_FLICKER").is_ok() && i + 6 >= frames {
                    let now = renderer.read_pixels().unwrap();
                    if let Some(before) = &previous {
                        let before: &Vec<u8> = before;
                        let mut diffs: Vec<u32> = now.iter().zip(before).map(|(a, b)| (*a as i32 - *b as i32).unsigned_abs()).collect();
                        let mean = diffs.iter().map(|&d| d as f64).sum::<f64>() / diffs.len() as f64;
                        diffs.sort_unstable();
                        let p99 = diffs[diffs.len() * 99 / 100];
                        let over8 = diffs.iter().filter(|&&d| d > 8).count() as f64 / diffs.len() as f64;
                        println!("{name}: frame {i} vs previous: mean {mean:.3}, p99 {p99}, share over 8 {:.4}", over8);
                    }
                    if i + 2 == frames {
                        let mut ppm = format!("P6\n{w} {h}\n255\n").into_bytes();
                        for p in now.chunks_exact(4) {
                            ppm.extend_from_slice(&p[..3]);
                        }
                        std::fs::write(out.join(format!("{name}_prev.ppm")), ppm).unwrap();
                    }
                    previous = Some(now);
                }
                if i + 20 >= frames {
                    for (k, (_, ms)) in renderer.stats.gpu_passes.iter().enumerate() {
                        if sums.len() <= k {
                            sums.push(0.0);
                        }
                        sums[k] += ms / 20.0;
                    }
                }
            }
            let pixels = renderer.read_pixels().unwrap();
            let mut ppm = format!("P6\n{w} {h}\n255\n").into_bytes();
            for p in pixels.chunks_exact(4) {
                ppm.extend_from_slice(&p[..3]);
            }
            std::fs::write(out.join(format!("{name}.ppm")), ppm).unwrap();
            let names: Vec<_> = renderer.stats.gpu_passes.iter().map(|p| p.0).collect();
            let mean: Vec<String> = names.iter().zip(&sums).map(|(n, ms)| format!("{n} {ms:.2}")).collect();
            println!("{name}: mean of last 20 frames: {} ({} frames in {:?})", mean.join(", "), frames, started.elapsed());
        }
    }
}

#[cfg(test)]
mod light_values {
    #[test]
    fn print_lighting() {
        let sun = super::light_at_hour(15.3);
        let l = super::lighting(sun);
        let zen = super::sky_single(glam::Vec3::Z, sun, l.sun);
        println!("sun {:?}\nsky {:?}\nhorizon {:?}\nground {:?}\nzenith {:?}", l.sun, l.sky, l.horizon, l.ground, zen);
    }
}
