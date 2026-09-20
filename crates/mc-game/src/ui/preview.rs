//! Map previews for the front end: the map's overview heights drawn as a
//! tactical chart (dark sea, lit relief, depth bands, a bright shoreline) into
//! one of the overlay's image slots.

use mc_map::MapFile;

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

/// RGBA8 (sRGB), `SIZE` x `SIZE`, north up. Non-square maps are letterboxed
/// with transparent pixels.
pub fn render(map: &MapFile) -> Vec<u8> {
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

    let size_m = info.size_metres().to_f32();
    let longest = size_m[0].max(size_m[1]);
    let metres_per_px = longest / SIZE as f32;
    // Overview samples per preview pixel (the overview is one sample per 32 m).
    let step = metres_per_px / 32.0;
    let (w, h) = (
        (size_m[0] / metres_per_px) as usize,
        (size_m[1] / metres_per_px) as usize,
    );
    let (pad_x, pad_y) = ((SIZE - w) / 2, (SIZE - h) / 2);

    let land = [
        (0.0, [0.30, 0.34, 0.27]),
        (12.0, [0.20, 0.27, 0.22]),
        (45.0, [0.25, 0.30, 0.24]),
        (90.0, [0.36, 0.37, 0.30]),
        (160.0, [0.47, 0.46, 0.41]),
        (300.0, [0.72, 0.74, 0.76]),
    ];
    let sea = [
        (0.0, [0.10, 0.27, 0.33]),
        (6.0, [0.06, 0.17, 0.23]),
        (30.0, [0.035, 0.10, 0.15]),
        (70.0, [0.02, 0.06, 0.10]),
    ];

    let mut rgba = vec![0u8; SIZE * SIZE * 4];
    for py in 0..h {
        for px in 0..w {
            // Image rows run top to bottom; the map's +Y is north.
            let (x, y) = (
                (px as f32 + 0.5) * step,
                (h as f32 - py as f32 - 0.5) * step,
            );
            let z = height(x, y);
            let gx = (height(x + step, y) - height(x - step, y)) / (2.0 * metres_per_px);
            let gy = (height(x, y + step) - height(x, y - step)) / (2.0 * metres_per_px);
            let mut c = if z > 0.0 {
                // Lit from the north-west, exaggerated so terraces read at this size.
                let shade = (1.0 + 2.2 * (gy - gx)).clamp(0.45, 1.7);
                // Faint contour lines every 25 m: how far, in pixels, to the nearest level.
                let slope = (gx * gx + gy * gy).sqrt().max(1e-3);
                let f = (z / 25.0).fract();
                let line =
                    (1.0 - f.min(1.0 - f) * 25.0 / (slope * metres_per_px) / 0.8).clamp(0.0, 1.0);
                ramp(&land, z).map(|v| v * shade * (1.0 - 0.22 * line))
            } else {
                ramp(&sea, -z)
            };
            // The shoreline glows: a band either side of the water's edge.
            let shore = (1.0
                - (z.abs() / (2.5 + (gx * gx + gy * gy).sqrt() * metres_per_px * 0.7)))
                .clamp(0.0, 1.0);
            c = mix(c, [0.35, 0.80, 0.86], shore * shore * 0.55);
            let at = ((py + pad_y) * SIZE + px + pad_x) * 4;
            for i in 0..3 {
                rgba[at + i] = (c[i].clamp(0.0, 1.0) * 255.0) as u8;
            }
            rgba[at + 3] = 255;
        }
    }

    // Mass deposits as a green cross, the same mark the match HUD uses.
    for d in map.mass_deposits() {
        let p = d.to_f32();
        let (cx, cy) = (
            (p[0] / metres_per_px) as i64 + pad_x as i64,
            (h as f32 - p[1] / metres_per_px) as i64 + pad_y as i64,
        );
        for (dx, dy) in [
            (0, 0),
            (1, 0),
            (-1, 0),
            (0, 1),
            (0, -1),
            (2, 0),
            (-2, 0),
            (0, 2),
            (0, -2),
        ] {
            let (x, y) = (cx + dx, cy + dy);
            if (0..SIZE as i64).contains(&x) && (0..SIZE as i64).contains(&y) {
                let at = (y as usize * SIZE + x as usize) * 4;
                rgba[at..at + 4].copy_from_slice(&[111, 227, 155, 255]);
            }
        }
    }
    rgba
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
