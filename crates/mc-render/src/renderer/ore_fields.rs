//! The ore fields on the ground (`fs_ore` in ground.wgsl): every field's corners
//! and the tiles that cover it, written after the stains and pads; how strongly
//! they show (faint in play, full during the mine survey); and the reaches of the
//! mines in sight, which the ore under them dims inside, so the ore still free to
//! claim stands out. A claim fades in over about half a second when a mine goes up
//! and out again when it is gone.

use crate::gpu::Buffer;
use mc_sim::mirror::StainInstance;
use std::mem::size_of;

/// Most mine reaches the ore is dimmed under, of those touching a field, oldest
/// first (presentation only: past it, the ore under the rest shows bright).
const MAX_CLAIMS: usize = 256;
/// Seconds a claim takes to dim in, or back out.
const CLAIM_FADE_S: f32 = 0.6;

/// A mine's reach, as the interface hands it over: who it is (so its fade carries
/// over from one frame to the next), where, how far.
#[derive(Clone, Copy, Debug)]
pub struct OreClaim {
    pub id: u32,
    pub centre: [f32; 2],
    pub reach: f32,
}

/// A claim as drawn: how far its dimming has come in, toward 1 while the mine
/// stands and back to 0 once it is gone.
struct Claim {
    id: u32,
    centre: [f32; 2],
    reach: f32,
    shown: f32,
    standing: bool,
}

pub(super) struct OreFields {
    /// Every field's corners first, then the tiles that cover the fields.
    splats: Vec<StainInstance>,
    /// How many entries at the front of `splats` are corners, not tiles.
    corners: usize,
    /// Every field's bounds, min and max, for which claims touch any.
    bounds: Vec<([f32; 2], [f32; 2])>,
    /// 0..1: how strongly ore fields show. Faint normally; full while a mine is
    /// placed. Eases toward the goal a little every frame.
    pub(super) highlight: f32,
    highlight_goal: f32,
    /// Seconds, for the veins' drifting glints.
    pub(super) vein_time: f32,
    claims: Vec<Claim>,
    /// When the claims were last eased; `None` before the first upload, when the
    /// mines already standing dim at once rather than all fading in on load.
    eased_at: Option<f32>,
    /// The tiles as written this frame: how many, and the first one's index.
    pub(super) count: u32,
    pub(super) first: u32,
    /// The claims as written this frame: the first one's index and how many
    /// (`Globals::ore_claims`).
    pub(super) claim_range: [u32; 2],
}

impl OreFields {
    pub(super) fn new(regions: &[mc_map::OreRegion]) -> OreFields {
        let rounded: Vec<mc_map::OreRegion> = regions.iter().map(rounded_ore).collect();
        let (splats, corners) = ore_splats(&rounded);
        let bounds = regions
            .iter()
            .map(|r| {
                let (lo, hi) = r.bounds();
                (lo.to_f32(), hi.to_f32())
            })
            .collect();
        OreFields {
            splats,
            corners,
            bounds,
            highlight: 0.0,
            highlight_goal: 0.0,
            vein_time: 0.0,
            claims: Vec::new(),
            eased_at: None,
            count: 0,
            first: 0,
            claim_range: [0; 2],
        }
    }

    pub(super) fn set_highlight(&mut self, k: f32) {
        self.highlight_goal = k.clamp(0.0, 1.0);
    }

    /// The mines standing now: new ones fade in, gone ones fade out. Only reaches
    /// that touch a field are kept.
    pub(super) fn set_claims(&mut self, claims: &[OreClaim]) {
        for c in &mut self.claims {
            c.standing = false;
        }
        for new in claims {
            let touches = self.bounds.iter().any(|(lo, hi)| {
                let near = [
                    new.centre[0].clamp(lo[0], hi[0]),
                    new.centre[1].clamp(lo[1], hi[1]),
                ];
                let d = [near[0] - new.centre[0], near[1] - new.centre[1]];
                d[0] * d[0] + d[1] * d[1] < new.reach * new.reach
            });
            if !touches {
                continue;
            }
            match self.claims.iter_mut().find(|c| c.id == new.id) {
                Some(c) => {
                    c.centre = new.centre;
                    c.reach = new.reach;
                    c.standing = true;
                }
                None => self.claims.push(Claim {
                    id: new.id,
                    centre: new.centre,
                    reach: new.reach,
                    shown: 0.0,
                    standing: true,
                }),
            }
        }
    }

    /// Writes the fields, and the claims after them, into `stains` from entry `used`
    /// on, all within `max` entries; eases the highlight a step and the claims to
    /// `time` (seconds).
    pub(super) fn upload(&mut self, stains: &Buffer, used: usize, max: usize, time: f32) {
        self.highlight += (self.highlight_goal - self.highlight) * 0.18;
        let step = match self.eased_at {
            Some(then) => ((time - then) / CLAIM_FADE_S).clamp(0.0, 1.0),
            None => 1.0,
        };
        self.eased_at = Some(time);
        for c in &mut self.claims {
            let goal = if c.standing { 1.0 } else { 0.0 };
            c.shown += (goal - c.shown).clamp(-step, step);
        }
        self.claims.retain(|c| c.standing || c.shown > 0.01);
        // All or nothing: a tile without its field's corners would draw garbage.
        let n = if self.splats.len() <= max.saturating_sub(used) {
            self.splats.len()
        } else {
            0
        };
        self.claim_range = [0; 2];
        if n > 0 {
            // Corners carry the highlight in their unused radius.
            self.vein_time += 1.0 / 60.0;
            for c in &mut self.splats[..self.corners] {
                c.radius = self.highlight;
            }
            stains.write(
                (used * size_of::<StainInstance>()) as u64,
                bytemuck::cast_slice(&self.splats[..n]),
            );
            let at = used + n;
            let room = max.saturating_sub(at).min(MAX_CLAIMS);
            let claims: Vec<StainInstance> = self
                .claims
                .iter()
                .take(room)
                .map(|c| StainInstance {
                    pos: c.centre,
                    radius: c.reach,
                    // How far it has dimmed in, as a stain's strength.
                    strength_seed: (c.shown.clamp(0.0, 1.0) * 255.0).round() as u32,
                })
                .collect();
            if !claims.is_empty() {
                stains.write(
                    (at * size_of::<StainInstance>()) as u64,
                    bytemuck::cast_slice(&claims),
                );
            }
            self.claim_range = [at as u32, claims.len() as u32];
        }
        self.first = (used + self.corners) as u32;
        self.count = n.saturating_sub(self.corners) as u32;
    }
}

/// Edge of the square decal tiles an ore field is drawn with, metres: six
/// patch quads of one terrain cell each.
const ORE_TILE_M: f32 = 48.0;

/// A field's outline with its corners rounded off (Chaikin corner cutting),
/// for the drawn rim only: the few corners of a map polygon read as a hard,
/// hand-cut shape. The rounded line lies just inside the real one; the sim
/// keeps the real corners. At most 64 corners, the rim shader walks them all.
fn rounded_ore(region: &mc_map::OreRegion) -> mc_map::OreRegion {
    let mut pts: Vec<[f32; 2]> = region.points.iter().map(|p| p.to_f32()).collect();
    for _ in 0..3 {
        if pts.len() < 3 || pts.len() * 2 > 64 {
            break;
        }
        let n = pts.len();
        pts = (0..n)
            .flat_map(|i| {
                let (a, b) = (pts[i], pts[(i + 1) % n]);
                let at = |t: f32| [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
                [at(0.25), at(0.75)]
            })
            .collect();
    }
    let fx = |v: f32| mc_core::Fx::from_f32(v);
    mc_map::OreRegion {
        points: pts
            .into_iter()
            .map(|p| mc_core::FxVec2::new(fx(p[0]), fx(p[1])))
            .collect(),
    }
}

/// Ore fields for the ground pass. Every field's corners go first (only `pos`
/// is used); then one tile per 48 m square that touches a field, whose
/// `strength_seed` packs the corner count (low 8 bits) and how far back from
/// the tile its field's first corner is (the rest), so the shader can find the
/// outline whatever offset the block is uploaded at. Returns the entries and
/// how many of them are corners.
fn ore_splats(regions: &[mc_map::OreRegion]) -> (Vec<StainInstance>, usize) {
    let mut corners = Vec::new();
    let mut firsts = Vec::new();
    for r in regions {
        firsts.push(corners.len());
        corners.extend(r.points.iter().map(|p| StainInstance {
            pos: p.to_f32(),
            radius: 0.0,
            strength_seed: 0,
        }));
    }
    let corner_count = corners.len();
    let mut tiles = Vec::new();
    for (r, &first) in regions.iter().zip(&firsts) {
        let (lo, hi) = r.bounds();
        let (lo, hi) = (lo.to_f32(), hi.to_f32());
        // One tile of margin for the rim and the ragged edge.
        let tx0 = ((lo[0] - 8.0) / ORE_TILE_M).floor() as i32;
        let ty0 = ((lo[1] - 8.0) / ORE_TILE_M).floor() as i32;
        let tx1 = ((hi[0] + 8.0) / ORE_TILE_M).floor() as i32;
        let ty1 = ((hi[1] + 8.0) / ORE_TILE_M).floor() as i32;
        let pts: Vec<[f32; 2]> = r.points.iter().map(|p| p.to_f32()).collect();
        for ty in ty0..=ty1 {
            for tx in tx0..=tx1 {
                let centre = [
                    (tx as f32 + 0.5) * ORE_TILE_M,
                    (ty as f32 + 0.5) * ORE_TILE_M,
                ];
                // Skip tiles wholly outside the outline (plus the margin).
                if polygon_distance(&pts, centre) > ORE_TILE_M * 0.72 + 8.0 {
                    continue;
                }
                let index = corner_count + tiles.len();
                let back = (index - first) as u32;
                tiles.push(StainInstance {
                    pos: centre,
                    radius: ORE_TILE_M * 0.5,
                    strength_seed: pts.len() as u32 | back << 8,
                });
            }
        }
    }
    corners.extend(tiles);
    (corners, corner_count)
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
