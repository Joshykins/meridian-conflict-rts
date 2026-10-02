//! Map previews for the front end and the minimap: the map as it looks from
//! the strategic camera, drawn on the CPU into one of the overlay's image slots.
//!
//! The colours are the renderer's own, measured: each band of height and depth,
//! open ground, forest, rock, snow and ice was averaged from whole-map shots of
//! the real renderer (tropical and temperate maps apart), so a preview shows the
//! same beaches, shallows, forests and Precursor structures a match opens on.
//! A map with regions is drawn in each region's colours, with the climate walls'
//! lines between them.

use mc_data::regions::Walls;
use mc_data::weather::{Climate, MapLook};
use mc_map::{MapFile, PropKind};

/// Preview edge in pixels; the overlay's image slots are this big.
pub const SIZE: usize = mc_render::overlay::IMAGE_SLOT;

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [0, 1, 2].map(|i| a[i] + (b[i] - a[i]) * t)
}

fn ramp(stops: &[(f32, [f32; 3])], v: f32) -> [f32; 3] {
    let i = stops
        .iter()
        .rposition(|(at, _)| v >= *at)
        .unwrap_or(0)
        .min(stops.len() - 2);
    let t = ((v - stops[i].0) / (stops[i + 1].0 - stops[i].0)).clamp(0.0, 1.0);
    mix(stops[i].1, stops[i + 1].1, t)
}

fn smoothstep(a: f32, b: f32, v: f32) -> f32 {
    let t = ((v - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// An sRGB colour from its bytes.
const fn rgb(r: u8, g: u8, b: u8) -> [f32; 3] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0]
}

/// One climate's colours, as the strategic camera sees them.
struct Palette {
    /// Water by depth in metres, the seabed showing through the shallows.
    sea: [(f32, [f32; 3]); 8],
    sand: [f32; 3],
    /// Heights over which the beach gives way to ground.
    sand_to: (f32, f32),
    /// Open ground low down, and on the heights.
    open: [f32; 3],
    upland: [f32; 3],
    /// A closed canopy.
    forest: [f32; 3],
    /// Canyon country (`Climate::Desert`): the ground is coloured by the rock
    /// beds at its height (`desert_land`) and no snow lies by height.
    strata: bool,
}

const TEMPERATE: Palette = Palette {
    sea: [
        (0.0, rgb(140, 146, 147)),
        (2.0, rgb(127, 144, 149)),
        (4.5, rgb(100, 139, 147)),
        (8.0, rgb(66, 130, 143)),
        (12.5, rgb(49, 122, 139)),
        (20.0, rgb(38, 110, 134)),
        (32.0, rgb(34, 98, 127)),
        (50.0, rgb(32, 90, 121)),
    ],
    sand: rgb(150, 143, 139),
    sand_to: (2.0, 9.5),
    open: rgb(92, 111, 108),
    upland: rgb(96, 103, 107),
    forest: rgb(54, 77, 94),
    strata: false,
};

/// Bright sand, turquoise over the banks, azure then deep blue in the channels.
const TROPICAL: Palette = Palette {
    sea: [
        (0.0, rgb(176, 192, 190)),
        (2.0, rgb(131, 187, 188)),
        (4.5, rgb(100, 181, 184)),
        (8.0, rgb(71, 173, 180)),
        (12.5, rgb(58, 164, 176)),
        (20.0, rgb(48, 146, 168)),
        (32.0, rgb(45, 106, 147)),
        (50.0, rgb(44, 87, 135)),
    ],
    sand: rgb(190, 187, 180),
    sand_to: (6.0, 13.0),
    open: rgb(108, 128, 120),
    upland: rgb(100, 112, 112),
    forest: rgb(64, 97, 101),
    strata: false,
};

/// Reservoir water: jade over the pale shallows, teal, cobalt in the old
/// channel. Ground by height (`desert_land`). Measured like the others, from a
/// whole-map shot of Vermilion Gorge.
const DESERT: Palette = Palette {
    sea: [
        (0.0, rgb(113, 153, 150)),
        (2.0, rgb(73, 139, 138)),
        (4.5, rgb(50, 124, 124)),
        (8.0, rgb(40, 114, 119)),
        (12.5, rgb(35, 101, 111)),
        (20.0, rgb(34, 81, 100)),
        (32.0, rgb(34, 64, 93)),
        (50.0, rgb(33, 59, 91)),
    ],
    sand: rgb(128, 122, 112),
    sand_to: (0.3, 2.0),
    open: rgb(124, 119, 111),
    upland: rgb(133, 126, 122),
    forest: rgb(70, 78, 66),
    strata: true,
};

/// Canyon country's rock beds by height above the lake as steep ground shows
/// them, and flat ground at that height (shaders/desert.wgsl), measured.
const DESERT_CLIFF: [(f32, [f32; 3]); 12] = [
    (0.0, rgb(110, 118, 125)),
    (54.0, rgb(123, 124, 126)),
    (60.0, rgb(118, 118, 120)),
    (90.0, rgb(105, 97, 96)),
    (115.0, rgb(97, 85, 92)),
    (175.0, rgb(106, 84, 91)),
    (250.0, rgb(110, 77, 85)),
    (290.0, rgb(112, 84, 91)),
    (310.0, rgb(115, 93, 97)),
    (330.0, rgb(120, 102, 103)),
    (350.0, rgb(118, 105, 106)),
    (400.0, rgb(127, 120, 118)),
];
const DESERT_FLAT: [(f32, [f32; 3]); 11] = [
    (0.0, rgb(116, 117, 114)),
    (54.0, rgb(118, 114, 113)),
    (66.0, rgb(124, 119, 111)),
    (100.0, rgb(122, 116, 108)),
    (115.0, rgb(114, 102, 102)),
    (190.0, rgb(121, 90, 94)),
    (250.0, rgb(119, 83, 88)),
    (300.0, rgb(122, 87, 91)),
    (330.0, rgb(132, 122, 117)),
    (360.0, rgb(128, 122, 118)),
    (380.0, rgb(133, 126, 122)),
];

const ROCK: [f32; 3] = rgb(78, 86, 98);
const SNOW: [f32; 3] = rgb(188, 195, 203);
const ICE: [f32; 3] = rgb(140, 152, 165);

/// Where the preview's pixels fall on the map.
struct Frame {
    size: usize,
    /// The map's own pixels, and their offset in the square.
    w: usize,
    h: usize,
    pad: (usize, usize),
    metres_per_px: f32,
}

impl Frame {
    fn new(map: &MapFile, size: usize) -> Frame {
        let size_m = map.info().size_metres().to_f32();
        let metres_per_px = size_m[0].max(size_m[1]) / size as f32;
        let (w, h) = (
            (size_m[0] / metres_per_px) as usize,
            (size_m[1] / metres_per_px) as usize,
        );
        Frame {
            size,
            w,
            h,
            pad: ((size - w) / 2, (size - h) / 2),
            metres_per_px,
        }
    }

    /// The world position at the middle of map pixel (`px`, `py`); rows run
    /// top to bottom and the map's +Y is north.
    fn world(&self, px: f32, py: f32) -> [f32; 2] {
        [
            (px + 0.5) * self.metres_per_px,
            (self.h as f32 - py - 0.5) * self.metres_per_px,
        ]
    }

    /// The map pixel a world position lies in, as fractional coordinates.
    fn pixel(&self, at: [f32; 2]) -> [f32; 2] {
        [
            at[0] / self.metres_per_px - 0.5,
            self.h as f32 - at[1] / self.metres_per_px - 0.5,
        ]
    }

    /// Blends `c` over map pixel (`x`, `y`) by `k`, if it is in the square.
    fn blend(&self, rgba: &mut [u8], x: i64, y: i64, c: [f32; 3], k: f32) {
        let (x, y) = (x + self.pad.0 as i64, y + self.pad.1 as i64);
        if !((0..self.size as i64).contains(&x) && (0..self.size as i64).contains(&y)) {
            return;
        }
        let at = (y as usize * self.size + x as usize) * 4;
        for i in 0..3 {
            let v = rgba[at + i] as f32 / 255.0;
            rgba[at + i] = ((v + (c[i] - v) * k).clamp(0.0, 1.0) * 255.0) as u8;
        }
    }
}

/// RGBA8 (sRGB), `SIZE` x `SIZE`, north up. Non-square maps are letterboxed
/// with transparent pixels.
/// `look` is the map's (`MapConfig::look`): its climate, or its regions' and their walls.
pub fn render(map: &MapFile, look: &MapLook) -> Vec<u8> {
    render_at(map, look, SIZE)
}

/// As `render`, `size` x `size` (the map browser's thumbnails).
pub fn render_at(map: &MapFile, look: &MapLook, size: usize) -> Vec<u8> {
    let frame = Frame::new(map, size);
    let mut rgba = vec![0u8; size * size * 4];
    ground(map, look, &frame, &mut rgba);
    structures(map, &frame, &mut rgba);
    if !look.walls().is_empty() {
        wall_lines(look.walls(), &frame, &mut rgba);
    }
    ore(map, &frame, &mut rgba);
    rgba
}

fn palette_of(climate: Climate) -> &'static Palette {
    match climate {
        Climate::Temperate => &TEMPERATE,
        Climate::Tropical => &TROPICAL,
        Climate::Desert => &DESERT,
    }
}

/// The light along the foot of a climate wall (bindings.wgsl `wall_seam`).
const WALL_LINE: [f32; 3] = rgb(176, 226, 255);

/// The climate walls' lines: thin pale strokes, about a pixel wide.
fn wall_lines(walls: &Walls, frame: &Frame, rgba: &mut [u8]) {
    for py in 0..frame.h {
        for px in 0..frame.w {
            let at = frame.world(px as f32, py as f32);
            let off = walls.wall_distance(at[0], at[1]) / frame.metres_per_px;
            let cover = (1.1 - off).clamp(0.0, 1.0);
            if cover > 0.0 {
                frame.blend(rgba, px as i64, py as i64, WALL_LINE, cover * 0.85);
            }
        }
    }
}

/// The terrain: sea by depth, beach, open ground, forest, rock, snow and ice, in the
/// colours of the climate each spot lies in.
fn ground(map: &MapFile, look: &MapLook, frame: &Frame, rgba: &mut [u8]) {
    let info = map.info();
    let (ow, oh) = map.overview_dims();
    let overview = map.overview();
    let water = info.water_level.to_f32();
    // Bilinear height in metres above the water, at overview coordinates.
    let height = |x: f32, y: f32| {
        let (x, y) = (x.clamp(0.0, (ow - 1) as f32), y.clamp(0.0, (oh - 1) as f32));
        let (x0, y0) = ((x as u32).min(ow - 2), (y as u32).min(oh - 2));
        let (fx, fy) = (x - x0 as f32, y - y0 as f32);
        let z = |x: u32, y: u32| {
            info.sample_to_height(overview[(y * ow + x) as usize])
                .to_f32()
                - water
        };
        let (top, bottom) = (
            z(x0, y0) * (1.0 - fx) + z(x0 + 1, y0) * fx,
            z(x0, y0 + 1) * (1.0 - fx) + z(x0 + 1, y0 + 1) * fx,
        );
        top * (1.0 - fy) + bottom * fy
    };
    let mpp = frame.metres_per_px;
    // Overview samples per preview pixel, and per 32 m (its spacing).
    let (step, overview_m) = (mpp / 32.0, 32.0);
    let canopy = canopy(map, frame);
    let snow = snow_layer(map);

    for py in 0..frame.h {
        for px in 0..frame.w {
            let at = frame.world(px as f32, py as f32);
            let palette = palette_of(look.climate_at(at[0], at[1]));
            let (x, y) = (at[0] / overview_m, at[1] / overview_m);
            let z = height(x, y);
            // Slope over at least one overview spacing, so it does not alias.
            let d = step.max(1.0);
            let gx = (height(x + d, y) - height(x - d, y)) / (2.0 * d * overview_m);
            let gy = (height(x, y + d) - height(x, y - d)) / (2.0 * d * overview_m);
            let c = if z <= 0.0 {
                ramp(&palette.sea, -z)
            } else {
                let grade = (gx * gx + gy * gy).sqrt();
                let cover = canopy[py * frame.w + px];
                let (ice, lying) = snow.as_ref().map_or((0.0, None), |s| s.at(at));
                let shade = (1.0 + 0.6 * (gy - gx)).clamp(0.75, 1.25);
                land(palette, z, look.strata_lift, grade, cover, ice, lying).map(|v| v * shade)
            };
            frame.blend(rgba, px as i64, py as i64, c, 1.0);
            let i = ((py + frame.pad.1) * frame.size + px + frame.pad.0) * 4;
            rgba[i + 3] = 255;
        }
    }
}

/// Dry ground `z` metres up with slope `grade` (rise over run), under a canopy
/// `cover` (0 open to 1 closed), with the map's snow layer's glacier ice and
/// lying snow there (`lying` is `None` on a map without one: snow then lies by
/// height, as the terrain shader lays it). `strata_lift` is the map's: how far
/// its desert's rock beds lie below Vermilion Gorge's.
fn land(
    palette: &Palette,
    z: f32,
    strata_lift: f32,
    grade: f32,
    cover: f32,
    ice: f32,
    lying: Option<f32>,
) -> [f32; 3] {
    if palette.strata {
        let bed = z + strata_lift;
        let beds = mix(
            ramp(&DESERT_FLAT, bed),
            ramp(&DESERT_CLIFF, bed),
            smoothstep(0.35, 0.9, grade),
        );
        let sand = 1.0 - smoothstep(palette.sand_to.0, palette.sand_to.1, z);
        let c = mix(
            beds,
            palette.sand,
            sand * (1.0 - smoothstep(0.1, 0.3, grade)),
        );
        return mix(c, palette.forest, (cover * 1.6).min(1.0) * 0.8);
    }
    let sand = 1.0 - smoothstep(palette.sand_to.0, palette.sand_to.1, z);
    let open = mix(palette.open, palette.upland, smoothstep(40.0, 200.0, z));
    let mut c = mix(open, palette.sand, sand);
    c = mix(
        c,
        palette.forest,
        (cover * 1.6).min(1.0) * (1.0 - 0.6 * sand),
    );
    c = mix(c, ROCK, smoothstep(0.45, 0.95, grade));
    let snow = match lying {
        Some(lying) => smoothstep(0.3, 0.7, lying) * (1.0 - smoothstep(1.2, 2.5, grade)),
        None => smoothstep(350.0, 450.0, z) * (1.0 - smoothstep(0.75, 1.2, grade)),
    };
    let ice = smoothstep(0.3, 0.7, ice);
    c = mix(c, SNOW, snow * (1.0 - ice));
    mix(c, ICE, ice)
}

/// How closed the tree canopy is over each map pixel, 0 to 1: every tree's
/// crown area summed into its pixel, spread a pixel round, as the renderer's
/// ground cover map does.
fn canopy(map: &MapFile, frame: &Frame) -> Vec<f32> {
    let (w, h) = (frame.w, frame.h);
    let mut crown = vec![0f32; w * h];
    let px_area = frame.metres_per_px * frame.metres_per_px;
    for p in map.props().iter().filter(|p| p.kind.is_tree()) {
        // Crown radii of the tree models at scale 1 (as in mc-render's ground_cover).
        let reach = mc_render::ground_cover::crown_of(p.kind).0 * p.scale_milli as f32 / 1000.0;
        let [x, y] = frame.pixel(p.pos.to_f32());
        let (x, y) = ((x + 0.5).floor(), (y + 0.5).floor());
        if (0.0..w as f32).contains(&x) && (0.0..h as f32).contains(&y) {
            crown[y as usize * w + x as usize] += std::f32::consts::PI * reach * reach / px_area;
        }
    }
    let mut out = vec![0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            let (mut sum, mut n) = (0.0, 0.0);
            for (dx, dy, k) in [
                (0, 0, 4.0),
                (-1, 0, 1.0),
                (1, 0, 1.0),
                (0, -1, 1.0),
                (0, 1, 1.0),
            ] {
                let (sx, sy) = (x as i64 + dx, y as i64 + dy);
                if (0..w as i64).contains(&sx) && (0..h as i64).contains(&sy) {
                    sum += crown[sy as usize * w + sx as usize] * k;
                    n += k;
                }
            }
            out[y * w + x] = 1.0 - (-sum / n * 1.6).exp();
        }
    }
    out
}

/// The map's snow layer: glacier ice and lying snow on a 16 m grid.
struct SnowLayer<'a> {
    samples: &'a [u8],
    w: u32,
    h: u32,
    pitch: f32,
}

fn snow_layer(map: &MapFile) -> Option<SnowLayer<'_>> {
    let samples = map.snow()?;
    let (w, h) = map.info().snow_dims();
    let pitch = (mc_map::CELL_SIZE_M as u32 * mc_map::format::SNOW_STRIDE) as f32;
    Some(SnowLayer {
        samples,
        w,
        h,
        pitch,
    })
}

impl SnowLayer<'_> {
    /// (ice, lying snow), each 0 to 1, at a world position.
    fn at(&self, at: [f32; 2]) -> (f32, Option<f32>) {
        let x = ((at[0] / self.pitch).round() as u32).min(self.w - 1);
        let y = ((at[1] / self.pitch).round() as u32).min(self.h - 1);
        let i = (y * self.w + x) as usize * 2;
        (
            self.samples[i] as f32 / 255.0,
            Some(self.samples[i + 1] as f32 / 255.0),
        )
    }
}

/// How a Precursor structure shows from above.
#[derive(Clone, Copy)]
enum Tone {
    /// Pale stone standing on the ground.
    Solid,
    /// A deck or girder overhead.
    Deck,
    /// Paving flush with the ground.
    Paving,
    /// Lying on the seabed, seen through the water.
    Sunk,
}

impl Tone {
    fn color(self) -> ([f32; 3], f32) {
        match self {
            Tone::Solid => (rgb(160, 166, 176), 1.0),
            Tone::Deck => (rgb(156, 162, 172), 0.85),
            Tone::Paving => (rgb(72, 80, 96), 0.9),
            Tone::Sunk => (rgb(26, 52, 78), 0.3),
        }
    }
}

/// A Precursor structure's plan seen from above, at its authored size, as
/// rectangles in its own frame like `PropKind::solid_plan`: the solid parts,
/// and the paving, decks and seabed causeways that are not solid.
fn plan_view(kind: PropKind) -> (&'static [(i32, i32, i32, i32)], Tone) {
    match kind {
        PropKind::PrecursorFloor => (&[(0, 0, 200, 200)], Tone::Paving),
        PropKind::PrecursorSeaway => (&[(0, 0, 98, 30)], Tone::Sunk),
        PropKind::PrecursorConduit => (&[(0, 0, 30, 5)], Tone::Sunk),
        PropKind::PrecursorSpan => (&[(480, 0, 520, 14)], Tone::Deck),
        PropKind::PrecursorViaduct => (&[(100, 0, 100, 14)], Tone::Deck),
        PropKind::PrecursorPlatform => (&[(0, 0, 188, 118)], Tone::Deck),
        PropKind::PrecursorBoom => (&[(-10, 0, 98, 86), (335, 0, 335, 24)], Tone::Deck),
        // Floating high overhead or hanging down a wall: nothing on the ground.
        PropKind::PrecursorHalo | PropKind::PrecursorMonolith | PropKind::PrecursorLining => {
            (&[], Tone::Solid)
        }
        _ => (kind.solid_plan(), Tone::Solid),
    }
}

/// The Precursor structures, flat ones first so what stands on them shows.
fn structures(map: &MapFile, frame: &Frame, rgba: &mut [u8]) {
    let mut props: Vec<_> = map
        .props()
        .iter()
        .filter(|p| p.kind.is_precursor())
        .collect();
    props.sort_by_key(|p| !matches!(plan_view(p.kind).1, Tone::Paving | Tone::Sunk));
    let mpp = frame.metres_per_px;
    for p in props {
        let (plan, tone) = plan_view(p.kind);
        let (color, alpha) = tone.color();
        let scale = p.scale_milli as f32 / 1000.0;
        let turn = p.heading.0 as f32 / 65536.0 * std::f32::consts::TAU;
        let (sin, cos) = turn.sin_cos();
        let pos = p.pos.to_f32();
        for &(cx, cy, hx, hy) in plan {
            let (cx, cy) = (cx as f32 * scale, cy as f32 * scale);
            let mid = [pos[0] + cx * cos - cy * sin, pos[1] + cx * sin + cy * cos];
            // A part thinner than a pixel is drawn a pixel wide and fainter.
            let (hx, hy) = (hx as f32 * scale, hy as f32 * scale);
            let (wx, wy) = (hx.max(mpp * 0.5), hy.max(mpp * 0.5));
            let faint = (hx / wx) * (hy / wy);
            let reach = (wx + wy) / mpp + 1.0;
            let [mx, my] = frame.pixel(mid);
            for y in (my - reach).floor() as i64..=(my + reach).ceil() as i64 {
                for x in (mx - reach).floor() as i64..=(mx + reach).ceil() as i64 {
                    let w = frame.world(x as f32, y as f32);
                    let (dx, dy) = (w[0] - mid[0], w[1] - mid[1]);
                    let (lx, ly) = (dx * cos + dy * sin, dy * cos - dx * sin);
                    // Outside distance in pixels, a pixel of soft edge.
                    let out = ((lx.abs() - wx).max(ly.abs() - wy)) / mpp;
                    let cover = (0.5 - out).clamp(0.0, 1.0);
                    if cover > 0.0 {
                        frame.blend(rgba, x, y, color, cover * alpha * faint);
                    }
                }
            }
        }
    }
}

/// Ore fields as the match marks them: a materials red-orange rim round a faint fill.
fn ore(map: &MapFile, frame: &Frame, rgba: &mut [u8]) {
    let ore = crate::hud::MASS;
    let fill = rgb((ore >> 16) as u8, (ore >> 8) as u8, ore as u8);
    let mpp = frame.metres_per_px;
    let h = frame.h as f32;
    for region in map.ore_regions() {
        let pts: Vec<[f32; 2]> = region.points.iter().map(|p| p.to_f32()).collect();
        let (lo, hi) = region.bounds();
        let (lo, hi) = (lo.to_f32(), hi.to_f32());
        let px0 = (lo[0] / mpp).floor() as i64 - 1;
        let px1 = (hi[0] / mpp).ceil() as i64 + 1;
        let py0 = (h - hi[1] / mpp).floor() as i64 - 1;
        let py1 = (h - lo[1] / mpp).ceil() as i64 + 1;
        for py in py0..=py1 {
            for px in px0..=px1 {
                let d = polygon_distance(&pts, frame.world(px as f32, py as f32)) / mpp;
                // Inside, and a pixel of rim.
                let cover = (0.5 - d).clamp(0.0, 1.0);
                if cover <= 0.0 {
                    continue;
                }
                let rim = (1.0 - (d + 0.8).abs()).clamp(0.0, 1.0);
                let bright = fill.map(|v| v * (0.8 + 0.2 * rim));
                frame.blend(rgba, px, py, bright, cover * (0.3 + 0.65 * rim));
            }
        }
    }
}

/// Where a world position lands in a preview drawn into a `side`-point square:
/// an offset from the square's top-left corner.
pub fn locate(map: &MapFile, pos: [f32; 2], side: f32) -> glam::Vec2 {
    let size_m = map.info().size_metres().to_f32();
    let longest = size_m[0].max(size_m[1]);
    let k = side / longest;
    let pad = glam::Vec2::new((longest - size_m[0]) * 0.5, (longest - size_m[1]) * 0.5) * k;
    glam::Vec2::new(pos[0] * k, (size_m[1] - pos[1]) * k) + pad
}

/// The inverse of `locate`: the world position under an offset into the preview.
pub fn world_at(map: &MapFile, offset: glam::Vec2, side: f32) -> glam::Vec2 {
    let size_m = map.info().size_metres().to_f32();
    let longest = size_m[0].max(size_m[1]);
    let k = side / longest;
    let pad = glam::Vec2::new((longest - size_m[0]) * 0.5, (longest - size_m[1]) * 0.5) * k;
    let p = (offset - pad) / k;
    glam::Vec2::new(
        p.x.clamp(0.0, size_m[0]),
        (size_m[1] - p.y).clamp(0.0, size_m[1]),
    )
}

/// Signed distance from `p` to the polygon `pts`, negative inside.
fn polygon_distance(pts: &[[f32; 2]], p: [f32; 2]) -> f32 {
    let mut d = f32::MAX;
    let mut inside = false;
    let n = pts.len();
    for i in 0..n {
        let (a, b) = (pts[i], pts[(i + n - 1) % n]);
        let e = [b[0] - a[0], b[1] - a[1]];
        let w = [p[0] - a[0], p[1] - a[1]];
        let t =
            ((w[0] * e[0] + w[1] * e[1]) / (e[0] * e[0] + e[1] * e[1]).max(1e-6)).clamp(0.0, 1.0);
        let q = [w[0] - e[0] * t, w[1] - e[1] * t];
        d = d.min(q[0] * q[0] + q[1] * q[1]);
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < a[0] + e[0] * (p[1] - a[1]) / e[1] {
            inside = !inside;
        }
    }
    if inside {
        -d.sqrt()
    } else {
        d.sqrt()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beaches_meet_the_sea_without_a_seam() {
        for palette in [&TEMPERATE, &TROPICAL] {
            let sea = glam::Vec3::from(ramp(&palette.sea, 0.0));
            let beach = glam::Vec3::from(land(palette, 0.01, 0.0, 0.0, 0.0, 0.0, None));
            assert!((sea - beach).abs().max_element() < 0.1, "{sea} vs {beach}");
        }
    }

    #[test]
    fn a_lowered_desert_takes_the_higher_beds_colours() {
        // Ground at 20 m with the beds lowered 50 m is the bench's, as at 70 m.
        assert_eq!(
            land(&DESERT, 20.0, 50.0, 0.0, 0.0, 0.0, None),
            land(&DESERT, 70.0, 0.0, 0.0, 0.0, 0.0, None)
        );
        // The beach is by the water, whatever the beds.
        let beach = land(&DESERT, 0.1, 50.0, 0.0, 0.0, 0.0, None);
        assert_eq!(beach, land(&DESERT, 0.1, 0.0, 0.0, 0.0, 0.0, None));
    }

    #[test]
    fn tropical_shallows_are_brighter_than_temperate_ones() {
        let (t, c) = (ramp(&TROPICAL.sea, 6.0), ramp(&TEMPERATE.sea, 6.0));
        assert!(t[1] > c[1] + 0.1 && t[2] > c[2] + 0.1);
    }

    /// Writes every baked map's preview to `target/previews/<stem>.png`, to
    /// compare with a whole-map shot of the game without starting it, and prints
    /// how long each took. `cargo test -p mc-game --release preview_dump --
    /// --ignored --nocapture` (needs the baked `maps/*.mcmap`).
    #[test]
    #[ignore]
    fn preview_dump() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let out = root.join("target/previews");
        std::fs::create_dir_all(&out).unwrap();
        let root = root.join("maps");
        for entry in std::fs::read_dir(root).unwrap().flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|e| e != "mcmap") {
                continue;
            }
            let map = MapFile::open(&path).unwrap();
            let look = mc_data::weather::MapConfig::for_map(&path)
                .unwrap_or_default()
                .look();
            let started = std::time::Instant::now();
            let rgba = render(&map, &look);
            let name = path.file_stem().unwrap().to_string_lossy().into_owned();
            println!("{name}: {:.0} ms", started.elapsed().as_secs_f32() * 1000.0);
            let file = std::fs::File::create(out.join(format!("{name}.png"))).unwrap();
            let mut enc =
                png::Encoder::new(std::io::BufWriter::new(file), SIZE as u32, SIZE as u32);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            enc.write_header().unwrap().write_image_data(&rgba).unwrap();
        }
    }
}
