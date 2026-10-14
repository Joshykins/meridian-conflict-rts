//! Resample only the painted area; canopy and cliff channels remain intact.
use super::*;
use mc_map::edit::{EditMap, Layer};
impl GroundCover {
    pub(crate) fn paint(&mut self, map: &EditMap, layer: Layer, rect: [u32; 4]) -> [u32; 4] {
        let size = map.info.size_metres().to_f32();
        let cell = (size[0].max(size[1]) / MAX_TEXELS as f32).max(MIN_CELL_M);
        let pitch = layer.pitch();
        // Bilinear samples on either side can influence a cover texel.
        let bounds = [
            (((rect[0] as f32 - 1.0) * pitch / cell - 0.5)
                .floor()
                .max(0.0) as u32)
                .min(self.width - 1),
            (((rect[1] as f32 - 1.0) * pitch / cell - 0.5)
                .floor()
                .max(0.0) as u32)
                .min(self.height - 1),
            (((rect[2] as f32 + 1.0) * pitch / cell).ceil() as u32).min(self.width - 1),
            (((rect[3] as f32 + 1.0) * pitch / cell).ceil() as u32).min(self.height - 1),
        ];
        let (w, h) = (
            (bounds[2] - bounds[0] + 1) as usize,
            (bounds[3] - bounds[1] + 1) as usize,
        );
        let data = layer.read(map);
        let dims = layer.dims(&map.info);
        let streets = if layer == Layer::Streets && !data.is_empty() {
            streets_region(data, dims, cell, [bounds[0], bounds[1]], (w, h))
        } else {
            Vec::new()
        };
        for y in bounds[1]..=bounds[3] {
            for x in bounds[0]..=bounds[2] {
                let i = (y * self.width + x) as usize * 4;
                match layer {
                    Layer::Snow => {
                        self.texels[i + 2] =
                            sample(data, dims, (2, 0), [x, y], (cell, pitch)).round() as u8;
                        self.texels[i + 3] = if data.is_empty() {
                            0
                        } else {
                            (1.0 + sample(data, dims, (2, 1), [x, y], (cell, pitch)) * 254.0
                                / 255.0)
                                .round() as u8
                        };
                    }
                    Layer::Ways => {
                        for c in 0..3 {
                            self.ways[i + c] =
                                sample(data, dims, (3, c), [x, y], (cell, pitch)).round() as u8;
                        }
                    }
                    Layer::Streets => {
                        let j = ((y - bounds[1]) as usize * w + (x - bounds[0]) as usize) * 4;
                        self.streets[i..i + 4].copy_from_slice(if streets.is_empty() {
                            &[0; 4]
                        } else {
                            &streets[j..j + 4]
                        });
                    }
                }
            }
        }
        if layer == Layer::Snow {
            self.mips = mips_of(
                &self.texels,
                (self.width as usize, self.height as usize),
                ICE_MIPS,
            );
        }
        bounds
    }
}
fn sample(
    data: &[u8],
    (w, h): (u32, u32),
    (channels, channel): (usize, usize),
    [x, y]: [u32; 2],
    (cell, pitch): (f32, f32),
) -> f32 {
    if data.is_empty() {
        return 0.0;
    }
    let sx = (x as f32 + 0.5) * cell / pitch;
    let sy = (y as f32 + 0.5) * cell / pitch;
    let (x0, y0) = (sx.floor() as u32, sy.floor() as u32);
    let at = |x: u32, y: u32| {
        data[((y.min(h - 1) * w + x.min(w - 1)) as usize) * channels + channel] as f32
    };
    let (fx, fy) = (sx.fract(), sy.fract());
    (at(x0, y0) * (1.0 - fx) + at(x0 + 1, y0) * fx) * (1.0 - fy)
        + (at(x0, y0 + 1) * (1.0 - fx) + at(x0 + 1, y0 + 1) * fx) * fy
}

#[cfg(test)]
mod tests;
