//! Sparse byte patches keep paint requests atomic without copying whole layers.
use super::point;
use mc_map::edit::{EditMap, Layer};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(in crate::editor) enum Paint {
    Snow,
    Ice,
    Trail,
    Ground,
}
#[derive(Default)]
pub(super) struct Patches {
    bytes: BTreeMap<(Layer, usize), u8>,
    introduced: BTreeSet<Layer>,
}
#[derive(Clone)]
pub(super) struct Change {
    layer: Layer,
    absent: bool,
    bytes: Vec<(usize, u8, u8)>,
}
impl Patches {
    fn read(&self, map: &EditMap, layer: Layer, i: usize) -> u8 {
        self.bytes.get(&(layer, i)).copied().unwrap_or_else(|| {
            layer
                .read(map)
                .get(i)
                .copied()
                .unwrap_or(layer.defaults()[i % layer.channels()])
        })
    }
    fn put(&mut self, layer: Layer, i: usize, value: u8) {
        self.bytes.insert((layer, i), value);
    }
    pub fn brush(
        &mut self,
        map: &EditMap,
        at: [f32; 2],
        radius: f32,
        paint: Paint,
        value: u8,
        heading: f32,
    ) -> Result<(), String> {
        inside(map, at)?;
        if !radius.is_finite() || !(8.0..=1024.0).contains(&radius) || !heading.is_finite() {
            return Err("Paint radius must be 8–1024 m and heading must be finite".into());
        }
        if matches!(paint, Paint::Ground) && value > 8 {
            return Err("Ground kind must be 0–8".into());
        }
        let (layer, channel) = match paint {
            Paint::Snow => (Layer::Snow, 1),
            Paint::Ice => (Layer::Snow, 0),
            Paint::Trail => (Layer::Ways, 0),
            Paint::Ground => (Layer::Streets, 3),
        };
        if layer == Layer::Snow && layer.read(map).is_empty() {
            self.introduced.insert(layer);
        }
        let (w, h) = layer.dims(&map.info);
        let pitch = layer.pitch();
        let min = [
            ((at[0] - radius) / pitch).max(0.0) as u32,
            ((at[1] - radius) / pitch).max(0.0) as u32,
        ];
        let max = [
            ((at[0] + radius) / pitch).ceil().min((w - 1) as f32) as u32,
            ((at[1] + radius) / pitch).ceil().min((h - 1) as f32) as u32,
        ];
        for y in min[1]..=max[1] {
            for x in min[0]..=max[0] {
                let d = ((x as f32 * pitch - at[0]).powi(2) + (y as f32 * pitch - at[1]).powi(2))
                    .sqrt();
                if d > radius {
                    continue;
                }
                let i = (y * w + x) as usize * layer.channels();
                let old = self.read(map, layer, i + channel);
                let next = if matches!(paint, Paint::Ground) {
                    (old & 0xf0) | value
                } else {
                    (old as f32 + (value as f32 - old as f32) * (1.0 - d / radius).min(1.0)).round()
                        as u8
                };
                self.put(layer, i + channel, next);
                if matches!(paint, Paint::Trail) {
                    let angle = heading.to_radians() * 2.0;
                    self.put(layer, i + 1, (127.5 + 127.5 * angle.cos()).round() as u8);
                    self.put(layer, i + 2, (127.5 + 127.5 * angle.sin()).round() as u8);
                }
            }
        }
        Ok(())
    }
    pub fn road(
        &mut self,
        map: &EditMap,
        from: [f32; 2],
        to: [f32; 2],
        width: f32,
        kind: u8,
    ) -> Result<(), String> {
        inside(map, from)?;
        inside(map, to)?;
        let delta = [to[0] - from[0], to[1] - from[1]];
        let length = (delta[0] * delta[0] + delta[1] * delta[1]).sqrt();
        if !(8.0..=8192.0).contains(&length)
            || !width.is_finite()
            || !(2.0..=120.0).contains(&width)
            || kind > 5
        {
            return Err("Road length must be 8–8192 m, width 2–120 m, and kind 0–5".into());
        }
        let layer = Layer::Streets;
        let (w, h) = layer.dims(&map.info);
        let pitch = layer.pitch();
        let reach = width * 0.5 + 32.0;
        let min = [
            ((from[0].min(to[0]) - reach) / pitch).max(0.0) as u32,
            ((from[1].min(to[1]) - reach) / pitch).max(0.0) as u32,
        ];
        let max = [
            ((from[0].max(to[0]) + reach) / pitch)
                .ceil()
                .min((w - 1) as f32) as u32,
            ((from[1].max(to[1]) + reach) / pitch)
                .ceil()
                .min((h - 1) as f32) as u32,
        ];
        for y in min[1]..=max[1] {
            for x in min[0]..=max[0] {
                let p = [x as f32 * pitch - from[0], y as f32 * pitch - from[1]];
                let along = (p[0] * delta[0] + p[1] * delta[1]) / length;
                if along < 0.0 || along > length {
                    continue;
                }
                let side = (delta[0] * p[1] - delta[1] * p[0]) / length;
                if side.abs() > reach {
                    continue;
                }
                let i = (y * w + x) as usize * 4;
                let existing = self.read(map, layer, i + 2) & 0x7f;
                let old_distance = self.read(map, layer, i).abs_diff(128) as f32 * 0.25;
                if kind != 0 && existing != 0 && old_distance < side.abs() {
                    continue;
                }
                self.put(
                    layer,
                    i,
                    (128.0 + side * 4.0).round().clamp(0.0, 255.0) as u8,
                );
                self.put(layer, i + 1, (width * 2.0).round() as u8);
                self.put(layer, i + 2, kind);
            }
        }
        Ok(())
    }
    pub fn changes(self, map: &EditMap) -> Vec<Change> {
        let mut layers: BTreeMap<Layer, Vec<(usize, u8, u8)>> = BTreeMap::new();
        for layer in self.introduced {
            layers.entry(layer).or_default();
        }
        for ((layer, i), new) in self.bytes {
            let old = layer
                .read(map)
                .get(i)
                .copied()
                .unwrap_or(layer.defaults()[i % layer.channels()]);
            if old != new {
                layers.entry(layer).or_default().push((i, old, new));
            }
        }
        layers
            .into_iter()
            .map(|(layer, bytes)| Change {
                layer,
                absent: layer.read(map).is_empty(),
                bytes,
            })
            .collect()
    }
}
fn inside(map: &EditMap, at: [f32; 2]) -> Result<(), String> {
    point(at)?;
    let size = map.info.size_metres().to_f32();
    if at[0] < 0.0 || at[1] < 0.0 || at[0] > size[0] || at[1] > size[1] {
        Err("Paint is outside the map".into())
    } else {
        Ok(())
    }
}
impl Change {
    pub fn restore(&self, map: &mut EditMap, forward: bool) -> (Layer, [u32; 4]) {
        let (w, h) = self.layer.dims(&map.info);
        let mut rect = [w, h, 0, 0];
        if !forward && self.absent {
            self.layer.write(map).clear();
        } else {
            self.layer.initialize(map);
            let data = self.layer.write(map);
            for &(i, old, new) in &self.bytes {
                data[i] = if forward { new } else { old };
                let sample = i / self.layer.channels();
                let (x, y) = (sample as u32 % w, sample as u32 / w);
                rect = [
                    rect[0].min(x),
                    rect[1].min(y),
                    rect[2].max(x),
                    rect[3].max(y),
                ];
            }
        }
        // Adding/removing snow changes the shader's automatic-snow sentinel
        // everywhere, even beyond the stroke.
        if self.absent {
            rect = [0, 0, w - 1, h - 1];
        }
        (self.layer, rect)
    }
}
