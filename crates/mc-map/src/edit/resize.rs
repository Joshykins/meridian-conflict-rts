//! Tile-aligned crop and extension preserve exact samples in the overlap.
use super::*;
use mc_core::Fx;
impl EditMap {
    /// Origin is measured in old-map tiles; negative values extend west/south.
    /// Returns old-to-new prop indices so authoring components retain identity.
    pub fn resized(
        &self,
        origin: [i32; 2],
        tiles: [u32; 2],
        fill: Fx,
    ) -> Result<(Self, Vec<Option<u32>>), MapError> {
        let mut info = self.info.clone();
        info.tiles_w = tiles[0];
        info.tiles_h = tiles[1];
        info.validate()?;
        if origin
            .iter()
            .any(|v| v.unsigned_abs() > crate::MAX_MAP_TILES)
        {
            return Err(MapError::Invalid(
                "Resize origin exceeds map bounds limit".into(),
            ));
        }
        let delta = FxVec2::from_ints(
            origin[0] * crate::TILE_SIZE_M,
            origin[1] * crate::TILE_SIZE_M,
        );
        let size = info.size_metres();
        let (w, h) = info.size_cells();
        let (ow, oh) = self.info.size_cells();
        let mut samples = vec![info.height_to_sample(fill); ((w + 1) * (h + 1)) as usize];
        for y in 0..=h {
            let sy = y as i32 + origin[1] * TILE_CELLS as i32;
            if !(0..=oh as i32).contains(&sy) {
                continue;
            }
            for x in 0..=w {
                let sx = x as i32 + origin[0] * TILE_CELLS as i32;
                if (0..=ow as i32).contains(&sx) {
                    samples[(y * (w + 1) + x) as usize] =
                        self.samples[(sy as u32 * (ow + 1) + sx as u32) as usize];
                }
            }
        }
        let mut map = Self {
            info,
            samples,
            props: vec![],
            tints: vec![],
            starts: self
                .starts
                .iter()
                .map(|p| clamp(*p - delta, size))
                .collect(),
            ore: vec![],
            snow: vec![],
            ways: vec![],
            streets: vec![],
            wrecks: vec![],
            decks: vec![],
        };
        for layer in [Layer::Snow, Layer::Ways, Layer::Streets] {
            copy_layer(self, &mut map, layer, origin);
        }
        let mut remap = vec![None; self.props.len()];
        for (i, p) in self.props.iter().enumerate() {
            let mut prop = *p;
            prop.pos -= delta;
            if inside(prop.pos, size) {
                remap[i] = Some(map.props.len() as u32);
                map.props.push(prop);
                if !self.tints.is_empty() {
                    map.tints.push(self.tints[i]);
                }
            }
        }
        for wreck in &self.wrecks {
            let mut wreck = wreck.clone();
            wreck.pos -= delta;
            if inside(wreck.pos, size) {
                map.wrecks.push(wreck);
            }
        }
        for ore in &self.ore {
            let points = clip_polygon(ore.points.iter().map(|p| *p - delta).collect(), size);
            if points.len() >= 3 {
                map.ore.push(OreRegion { points });
            }
        }
        for deck in &self.decks {
            if let Some((a, b)) = clip_line(deck.a - delta, deck.b - delta, size) {
                if a != b {
                    map.decks.push(Deck { a, b, ..*deck });
                }
            }
        }
        Ok((map, remap))
    }
}
fn copy_layer(old: &EditMap, new: &mut EditMap, layer: Layer, origin: [i32; 2]) {
    let source = layer.read(old);
    if source.is_empty() {
        return;
    }
    layer.initialize(new);
    let (w, h) = layer.dims(&new.info);
    let (ow, oh) = layer.dims(&old.info);
    let channels = layer.channels();
    let tile_stride = (ow - 1) / old.info.tiles_w;
    let target = layer.write(new);
    for y in 0..h {
        let sy = y as i32 + origin[1] * tile_stride as i32;
        if !(0..oh as i32).contains(&sy) {
            continue;
        }
        for x in 0..w {
            let sx = x as i32 + origin[0] * tile_stride as i32;
            if !(0..ow as i32).contains(&sx) {
                continue;
            }
            let to = (y * w + x) as usize * channels;
            let from = (sy as u32 * ow + sx as u32) as usize * channels;
            target[to..to + channels].copy_from_slice(&source[from..from + channels]);
        }
    }
}
fn inside(p: FxVec2, size: FxVec2) -> bool {
    p.x >= Fx::ZERO && p.y >= Fx::ZERO && p.x <= size.x && p.y <= size.y
}
fn clamp(p: FxVec2, size: FxVec2) -> FxVec2 {
    FxVec2::new(p.x.clamp(Fx::ZERO, size.x), p.y.clamp(Fx::ZERO, size.y))
}
fn coordinate(p: FxVec2, axis: usize) -> Fx {
    if axis == 0 {
        p.x
    } else {
        p.y
    }
}
fn intersection(a: FxVec2, b: FxVec2, axis: usize, edge: Fx) -> FxVec2 {
    let av = coordinate(a, axis);
    let bv = coordinate(b, axis);
    let other = if axis == 0 {
        a.y.0 + ((b.y.0 - a.y.0) as i128 * (edge.0 - av.0) as i128 / (bv.0 - av.0) as i128) as i64
    } else {
        a.x.0 + ((b.x.0 - a.x.0) as i128 * (edge.0 - av.0) as i128 / (bv.0 - av.0) as i128) as i64
    };
    if axis == 0 {
        FxVec2::new(edge, Fx(other))
    } else {
        FxVec2::new(Fx(other), edge)
    }
}
fn clip_polygon(mut points: Vec<FxVec2>, size: FxVec2) -> Vec<FxVec2> {
    for (axis, edge, lower) in [
        (0, Fx::ZERO, true),
        (0, size.x, false),
        (1, Fx::ZERO, true),
        (1, size.y, false),
    ] {
        let mut out = vec![];
        if let Some(&last) = points.last() {
            let mut a = last;
            for &b in &points {
                let a_in = if lower {
                    coordinate(a, axis) >= edge
                } else {
                    coordinate(a, axis) <= edge
                };
                let b_in = if lower {
                    coordinate(b, axis) >= edge
                } else {
                    coordinate(b, axis) <= edge
                };
                if a_in != b_in {
                    out.push(intersection(a, b, axis, edge));
                }
                if b_in {
                    out.push(b);
                }
                a = b;
            }
        }
        out.dedup();
        if out.len() > 1 && out.first() == out.last() {
            out.pop();
        }
        points = out;
    }
    points
}
fn clip_line(mut a: FxVec2, mut b: FxVec2, size: FxVec2) -> Option<(FxVec2, FxVec2)> {
    for (axis, edge, lower) in [
        (0, Fx::ZERO, true),
        (0, size.x, false),
        (1, Fx::ZERO, true),
        (1, size.y, false),
    ] {
        let a_in = if lower {
            coordinate(a, axis) >= edge
        } else {
            coordinate(a, axis) <= edge
        };
        let b_in = if lower {
            coordinate(b, axis) >= edge
        } else {
            coordinate(b, axis) <= edge
        };
        if !a_in && !b_in {
            return None;
        }
        if a_in != b_in {
            let at = intersection(a, b, axis, edge);
            if a_in {
                b = at;
            } else {
                a = at;
            }
        }
    }
    Some((a, b))
}
