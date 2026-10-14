//! Terrain-following cloud floor shared by initial loading and live map edits.
use super::*;
pub(super) fn sample(map_size: Vec2, water_level: f32, ground: impl Fn(Vec2) -> f32) -> Vec<f32> {
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
                    let at = (centre + Vec2::new(i as f32, j as f32) * cell * 0.5)
                        .clamp(Vec2::ZERO, map_size);
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
    floor
}
pub(super) fn range(floor: &[f32]) -> (f32, f32) {
    (
        floor.iter().copied().fold(f32::INFINITY, f32::min),
        floor.iter().copied().fold(f32::NEG_INFINITY, f32::max),
    )
}
impl Sky {
    #[track_caller]
    pub(crate) fn edit_floor(
        &mut self,
        gpu: &Gpu,
        water_level: f32,
        ground: impl Fn(Vec2) -> f32,
    ) -> Result<(), GpuError> {
        let floor = sample(self.map_size, water_level, ground);
        gpu.upload_image(
            &self.floor_image,
            0,
            0,
            None,
            bytemuck::cast_slice(&floor),
            false,
        )?;
        self.floor_range = range(&floor);
        self.floor = floor;
        self.history_valid = false;
        Ok(())
    }
}
