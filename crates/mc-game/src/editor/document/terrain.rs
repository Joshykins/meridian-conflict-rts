use super::*;
impl Document {
    pub(super) fn brush(
        &self,
        at: [f32; 2],
        radius: f32,
        strength: f32,
        brush: Brush,
        patches: &mut BTreeMap<(u32, u32), u16>,
    ) -> Result<(), String> {
        point(at)?;
        if !radius.is_finite()
            || !(8.0..=1024.0).contains(&radius)
            || !strength.is_finite()
            || !(0.0..=256.0).contains(&strength)
        {
            return Err("Brush radius must be 8–1024 m and strength 0–256 m".into());
        }
        let (w, h) = self.map.info.size_cells();
        let cell = CELL_SIZE_M as f32;
        if at[0] < 0.0 || at[1] < 0.0 || at[0] > w as f32 * cell || at[1] > h as f32 * cell {
            return Err("Brush is outside the map".into());
        }
        let min = [
            ((at[0] - radius) / cell).max(0.0) as u32,
            ((at[1] - radius) / cell).max(0.0) as u32,
        ];
        let max = [
            ((at[0] + radius) / cell).ceil().min(w as f32) as u32,
            ((at[1] + radius) / cell).ceil().min(h as f32) as u32,
        ];
        let old = |x, y| {
            patches
                .get(&(x, y))
                .copied()
                .unwrap_or(self.map.samples()[(y * (w + 1) + x) as usize])
        };
        let target = old((at[0] / cell).round() as u32, (at[1] / cell).round() as u32) as f32;
        let step = self.map.info.z_step.to_f32();
        let mut updates = Vec::new();
        for y in min[1]..=max[1] {
            for x in min[0]..=max[0] {
                let distance =
                    ((x as f32 * cell - at[0]).powi(2) + (y as f32 * cell - at[1]).powi(2)).sqrt();
                if distance > radius {
                    continue;
                }
                let weight = (1.0 - distance / radius).clamp(0.0, 1.0);
                let value = old(x, y) as f32;
                let next = match brush {
                    Brush::Raise => value + strength / step * weight,
                    Brush::Lower => value - strength / step * weight,
                    Brush::Flatten => value + (target - value) * weight,
                    Brush::Cliff => {
                        value + (target + strength / step - value) * (weight * 10.0).min(1.0)
                    }
                    Brush::Noise => {
                        let hash = x.wrapping_mul(0x9E3779B9) ^ y.wrapping_mul(0x85EBCA6B);
                        let hash = (hash ^ (hash >> 16)).wrapping_mul(0x7FEB352D);
                        let noise = (hash ^ (hash >> 15)) as f64 / u32::MAX as f64 * 2.0 - 1.0;
                        value + noise as f32 * strength / step * weight
                    }
                    Brush::Smooth => {
                        let mut sum = 0.0;
                        let mut count = 0.0;
                        for ny in y.saturating_sub(1)..=(y + 1).min(h) {
                            for nx in x.saturating_sub(1)..=(x + 1).min(w) {
                                sum += old(nx, ny) as f32;
                                count += 1.0;
                            }
                        }
                        value + (sum / count - value) * weight
                    }
                };
                updates.push(((x, y), next.round().clamp(0.0, u16::MAX as f32) as u16));
            }
        }
        patches.extend(updates);
        Ok(())
    }

    pub(super) fn sample(&self, x: u32, y: u32, patches: &BTreeMap<(u32, u32), u16>) -> f32 {
        let (w, h) = self.map.info.size_cells();
        let (x, y) = (x.min(w), y.min(h));
        patches
            .get(&(x, y))
            .copied()
            .unwrap_or(self.map.samples()[(y * (w + 1) + x) as usize]) as f32
    }
    pub(super) fn ramp(
        &self,
        from: [f32; 2],
        to: [f32; 2],
        width: f32,
        patches: &mut BTreeMap<(u32, u32), u16>,
    ) -> Result<(), String> {
        let size = self.map.info.size_metres().to_f32();
        for p in [from, to] {
            point(p)?;
            if p[0] < 0.0 || p[1] < 0.0 || p[0] > size[0] || p[1] > size[1] {
                return Err("Ramp endpoints must be inside the map".into());
            }
        }
        if !width.is_finite() || !(8.0..=1024.0).contains(&width) {
            return Err("Ramp width must be 8–1024 m".into());
        }
        let a = glam::Vec2::from(from);
        let b = glam::Vec2::from(to);
        let delta = b - a;
        if delta.length() < 8.0 || delta.length() > 8192.0 {
            return Err("Ramp length must be 8–8192 m".into());
        }
        let cell = CELL_SIZE_M as f32;
        let start = self.sample(
            (a.x / cell).round() as u32,
            (a.y / cell).round() as u32,
            patches,
        );
        let end = self.sample(
            (b.x / cell).round() as u32,
            (b.y / cell).round() as u32,
            patches,
        );
        let (w, h) = self.map.info.size_cells();
        let min = (a.min(b) - glam::Vec2::splat(width)).max(glam::Vec2::ZERO) / cell;
        let max = (a.max(b) + glam::Vec2::splat(width)) / cell;
        for y in min.y as u32..=(max.y.ceil() as u32).min(h) {
            for x in min.x as u32..=(max.x.ceil() as u32).min(w) {
                let pos = glam::Vec2::new(x as f32 * cell, y as f32 * cell);
                let t = ((pos - a).dot(delta) / delta.length_squared()).clamp(0.0, 1.0);
                let blend = (1.0 - pos.distance(a + delta * t) / width).clamp(0.0, 1.0);
                if blend == 0.0 {
                    continue;
                }
                let old = self.sample(x, y, patches);
                let value = old + (start + (end - start) * t - old) * blend;
                patches.insert((x, y), value.round().clamp(0.0, u16::MAX as f32) as u16);
            }
        }
        Ok(())
    }
}
