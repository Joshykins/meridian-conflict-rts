//! Bounded map-local heightmap assets. Values are signed height deltas normalized to one.
use super::*;
use std::io::Read;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Heightmap {
    width: usize,
    height: usize,
    values: Vec<f32>,
}
impl Document {
    pub(super) fn stamp(
        &self,
        at: [f32; 2],
        radius: f32,
        strength: f32,
        asset: &str,
        patches: &mut BTreeMap<(u32, u32), u16>,
    ) -> Result<(), String> {
        point(at)?;
        let size = self.map.info.size_metres().to_f32();
        if at[0] < 0.0
            || at[1] < 0.0
            || at[0] > size[0]
            || at[1] > size[1]
            || !radius.is_finite()
            || !(8.0..=1024.0).contains(&radius)
            || !strength.is_finite()
            || !(0.0..=256.0).contains(&strength)
        {
            return Err("Invalid heightmap location, radius or strength".into());
        }
        if asset.len() > 128
            || !asset.ends_with(".json")
            || !asset
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        {
            return Err("Use a JSON filename from the map's stamps folder".into());
        }
        let path = self.doc_folder().join("stamps").join(asset);
        let file = std::fs::File::open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut bytes = Vec::new();
        file.take(1_048_577)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > 1_048_576 {
            return Err("Heightmap exceeds 1 MiB".into());
        }
        let stamp: Heightmap =
            serde_json::from_slice(&bytes).map_err(|e| format!("Heightmap: {e}"))?;
        if !(2..=257).contains(&stamp.width)
            || !(2..=257).contains(&stamp.height)
            || stamp.values.len() != stamp.width * stamp.height
            || stamp
                .values
                .iter()
                .any(|v| !v.is_finite() || !(-1.0..=1.0).contains(v))
        {
            return Err("Heightmap needs 2–257 samples per side and values from -1 to 1".into());
        }
        let half = glam::Vec2::new(radius, radius * stamp.height as f32 / stamp.width as f32);
        let center = glam::Vec2::from(at);
        let cell = CELL_SIZE_M as f32;
        let (w, h) = self.map.info.size_cells();
        let min = ((center - half) / cell).ceil().max(glam::Vec2::ZERO);
        let max = ((center + half) / cell).floor();
        for y in min.y as u32..=(max.y as u32).min(h) {
            for x in min.x as u32..=(max.x as u32).min(w) {
                let uv = (glam::Vec2::new(x as f32 * cell, y as f32 * cell) - center + half)
                    / (half * 2.0);
                let u = (uv.x * (stamp.width - 1) as f32).clamp(0.0, (stamp.width - 1) as f32);
                let v = (uv.y * (stamp.height - 1) as f32).clamp(0.0, (stamp.height - 1) as f32);
                let x0 = u as usize;
                let y0 = v as usize;
                let x1 = (x0 + 1).min(stamp.width - 1);
                let y1 = (y0 + 1).min(stamp.height - 1);
                let top = stamp.values[y0 * stamp.width + x0] * (1.0 - u.fract())
                    + stamp.values[y0 * stamp.width + x1] * u.fract();
                let bottom = stamp.values[y1 * stamp.width + x0] * (1.0 - u.fract())
                    + stamp.values[y1 * stamp.width + x1] * u.fract();
                let delta = (top * (1.0 - v.fract()) + bottom * v.fract()) * strength
                    / self.map.info.z_step.to_f32();
                let value = self.sample(x, y, patches) + delta;
                patches.insert((x, y), value.round().clamp(0.0, u16::MAX as f32) as u16);
            }
        }
        Ok(())
    }
    fn doc_folder(&self) -> PathBuf {
        self.destination
            .clone()
            .unwrap_or_else(|| self.source.parent().unwrap_or(Path::new(".")).to_owned())
    }
}
