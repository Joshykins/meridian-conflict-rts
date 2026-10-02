//! Fog of war: per-player visibility on a coarse grid.
//!
//! The live layers are rebuilt from unit positions at the end of every tick,
//! and read during the next one (targeting, sonar, the AI). `explored` and
//! `identified` accumulate, and the AI reads `identified`. So the fog as a
//! whole is part of the match: it travels in snapshots (a rejoining player
//! must remember what its side has seen), and what the simulation reads of its
//! memory is hashed. Radar detects units without lighting the ground. Sonar is
//! the only layer that finds a hull under water (`naval.rs`).

use mc_core::{Fx, FxVec2, PlayerMask, StateHasher};
use serde::{Deserialize, Serialize};

/// Fog cell edge: 64 m.
const CELL_SHIFT: u32 = 6;

/// One vision disc, as the rebuild stamped it.
#[derive(Clone, Copy)]
pub(crate) struct Sight {
    /// The unit seeing, whose travel the drawn disc follows; `None` for a fixed reveal.
    pub(crate) row: Option<usize>,
    pub(crate) pos: FxVec2,
    pub(crate) vision: Fx,
    pub(crate) mask: PlayerMask,
}

#[derive(Serialize, Deserialize)]
pub struct Fog {
    width: i32,
    height: i32,
    /// Bit `p` set: player `p` sees this cell right now.
    visible: Vec<PlayerMask>,
    /// Bit `p` set: the cell is inside player `p`'s radar coverage.
    radar: Vec<PlayerMask>,
    /// Bit `p` set: the cell is inside player `p`'s sonar coverage.
    sonar: Vec<PlayerMask>,
    explored: Vec<PlayerMask>,
    /// Bit `p` set: player `p` has seen this unit row with vision. A generation
    /// stamp keeps a reused row from staying known.
    identified: Vec<PlayerMask>,
    identified_gen: Vec<u16>,
    /// Every vision disc of the last rebuild, as stamped and before any are merged,
    /// for the renderer to draw round (`mirror/fog.rs`). Never read by the simulation.
    #[serde(skip)]
    sights: Vec<Sight>,
    /// Bumped on every rebuild so the renderer knows when to re-upload.
    #[serde(skip)]
    pub version: u32,
}

impl Fog {
    pub fn new(map_size: FxVec2) -> Fog {
        let width = (map_size.x.ceil_int() >> CELL_SHIFT).max(1) + 1;
        let height = (map_size.y.ceil_int() >> CELL_SHIFT).max(1) + 1;
        let n = (width * height) as usize;
        Fog {
            width,
            height,
            visible: vec![0; n],
            radar: vec![0; n],
            sonar: vec![0; n],
            explored: vec![0; n],
            identified: Vec::new(),
            identified_gen: Vec::new(),
            sights: Vec::new(),
            version: 0,
        }
    }

    pub fn dims(&self) -> (u32, u32) {
        (self.width as u32, self.height as u32)
    }

    pub fn begin(&mut self) {
        self.visible.fill(0);
        self.radar.fill(0);
        self.sonar.fill(0);
        self.sights.clear();
        self.version = self.version.wrapping_add(1);
    }

    /// Marks the disc around `pos` for the players in `mask`.
    pub fn reveal(&mut self, pos: FxVec2, vision: Fx, radar: Fx, mask: PlayerMask) {
        self.note_sight(None, pos, vision, mask);
        self.stamp_discs(pos, vision, radar, mask);
    }

    /// Keeps a vision disc for the renderer; `stamp_discs` marks the grid.
    pub(crate) fn note_sight(
        &mut self,
        row: Option<usize>,
        pos: FxVec2,
        vision: Fx,
        mask: PlayerMask,
    ) {
        if vision > Fx::ZERO {
            self.sights.push(Sight {
                row,
                pos,
                vision,
                mask,
            });
        }
    }

    /// The vision discs of the last rebuild (`note_sight`).
    pub(crate) fn sights(&self) -> &[Sight] {
        &self.sights
    }

    /// `reveal` without keeping the disc for the renderer: for discs already noted.
    pub(crate) fn stamp_discs(&mut self, pos: FxVec2, vision: Fx, radar: Fx, mask: PlayerMask) {
        if vision > Fx::ZERO {
            Self::stamp(
                &mut self.visible,
                Some(&mut self.explored),
                self.width,
                self.height,
                pos,
                vision,
                mask,
            );
        }
        if radar > Fx::ZERO {
            Self::stamp(
                &mut self.radar,
                None,
                self.width,
                self.height,
                pos,
                radar,
                mask,
            );
        }
    }

    /// Marks the sonar disc around `pos` for the players in `mask`.
    pub fn reveal_sonar(&mut self, pos: FxVec2, sonar: Fx, mask: PlayerMask) {
        if sonar > Fx::ZERO {
            Self::stamp(
                &mut self.sonar,
                None,
                self.width,
                self.height,
                pos,
                sonar,
                mask,
            );
        }
    }

    fn stamp(
        grid: &mut [PlayerMask],
        mut also: Option<&mut Vec<PlayerMask>>,
        width: i32,
        height: i32,
        pos: FxVec2,
        radius: Fx,
        mask: PlayerMask,
    ) {
        let (cx, cy) = Self::cell_of(pos);
        let r = (radius.ceil_int() >> CELL_SHIFT) + 1;
        let r2 = r * r;
        for dy in -r..=r {
            let y = cy + dy;
            if y < 0 || y >= height {
                continue;
            }
            // Half-width of the disc on this row, by integer search; r is small.
            let mut half = r;
            while half > 0 && half * half + dy * dy > r2 {
                half -= 1;
            }
            let x0 = (cx - half).max(0);
            let x1 = (cx + half).min(width - 1);
            if x0 > x1 {
                continue;
            }
            let row = (y * width) as usize;
            for cell in &mut grid[row + x0 as usize..=row + x1 as usize] {
                *cell |= mask;
            }
            if let Some(extra) = also.as_deref_mut() {
                for cell in &mut extra[row + x0 as usize..=row + x1 as usize] {
                    *cell |= mask;
                }
            }
        }
    }

    /// The fog cell `pos` is in, as the discs `reveal` stamps round it see it.
    #[inline]
    pub(crate) fn cell_of(pos: FxVec2) -> (i32, i32) {
        (
            pos.x.floor_int() >> CELL_SHIFT,
            pos.y.floor_int() >> CELL_SHIFT,
        )
    }

    #[inline]
    fn cell(&self, pos: FxVec2) -> usize {
        let x = (pos.x.floor_int() >> CELL_SHIFT).clamp(0, self.width - 1);
        let y = (pos.y.floor_int() >> CELL_SHIFT).clamp(0, self.height - 1);
        (y * self.width + x) as usize
    }

    /// True when any player in `mask` currently sees `pos`.
    #[inline]
    pub fn is_visible(&self, pos: FxVec2, mask: PlayerMask) -> bool {
        self.visible[self.cell(pos)] & mask != 0
    }

    /// True when any player in `mask` has ever seen `pos`.
    #[inline]
    pub fn is_explored(&self, pos: FxVec2, mask: PlayerMask) -> bool {
        self.explored[self.cell(pos)] & mask != 0
    }

    /// Visible or on radar: good enough to shoot at.
    #[inline]
    pub fn is_detected(&self, pos: FxVec2, mask: PlayerMask) -> bool {
        let c = self.cell(pos);
        (self.visible[c] | self.radar[c]) & mask != 0
    }

    /// True when `pos` is inside the sonar coverage of any player in `mask`.
    #[inline]
    pub fn is_sonar(&self, pos: FxVec2, mask: PlayerMask) -> bool {
        self.sonar[self.cell(pos)] & mask != 0
    }

    /// Players who currently see `pos` with vision.
    #[inline]
    pub fn visible_mask(&self, pos: FxVec2) -> PlayerMask {
        self.visible[self.cell(pos)]
    }

    /// Records that the players in `mask` have seen this occupant with vision.
    pub fn identify(&mut self, row: usize, generation: u16, mask: PlayerMask) {
        if self.identified.len() <= row {
            self.identified.resize(row + 1, 0);
            self.identified_gen.resize(row + 1, 0);
        }
        if self.identified_gen[row] != generation {
            self.identified[row] = 0;
            self.identified_gen[row] = generation;
        }
        self.identified[row] |= mask;
    }

    /// True when any player in `mask` has seen this occupant with vision.
    #[inline]
    pub fn is_identified(&self, row: usize, generation: u16, mask: PlayerMask) -> bool {
        self.identified
            .get(row)
            .is_some_and(|&m| self.identified_gen[row] == generation && m & mask != 0)
    }

    pub fn visible_cells(&self) -> &[PlayerMask] {
        &self.visible
    }

    /// What the simulation reads of the fog's memory: who has identified which unit.
    pub fn hash_memory(&self, h: &mut StateHasher) {
        h.write_u32s(&self.identified);
        h.write_u64(self.identified_gen.len() as u64);
        for &g in &self.identified_gen {
            h.write_u32(g as u32);
        }
    }

    /// Takes a snapshot's fog in place of this one, if it has this map's shape.
    pub(crate) fn replace_with(&mut self, other: Fog) -> Result<(), String> {
        let cells = (self.width * self.height) as usize;
        let shaped = other.width == self.width
            && other.height == self.height
            && [&other.visible, &other.radar, &other.sonar, &other.explored]
                .iter()
                .all(|g| g.len() == cells)
            && other.identified.len() == other.identified_gen.len();
        if !shaped {
            return Err("the snapshot's fog does not fit this map".into());
        }
        let version = self.version.wrapping_add(1);
        *self = other;
        self.version = version;
        Ok(())
    }

    pub fn explored_cells(&self) -> &[PlayerMask] {
        &self.explored
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reveal_is_a_disc_per_player() {
        let mut fog = Fog::new(FxVec2::from_ints(4096, 4096));
        fog.begin();
        fog.reveal(
            FxVec2::from_ints(1000, 1000),
            Fx::from_int(200),
            Fx::from_int(600),
            0b01,
        );
        assert!(fog.is_visible(FxVec2::from_ints(1100, 1000), 0b01));
        assert!(!fog.is_visible(FxVec2::from_ints(1100, 1000), 0b10));
        assert!(!fog.is_visible(FxVec2::from_ints(1500, 1000), 0b01));
        assert!(fog.is_detected(FxVec2::from_ints(1500, 1000), 0b01));
        assert!(!fog.is_detected(FxVec2::from_ints(2500, 1000), 0b01));
        fog.begin();
        assert!(!fog.is_visible(FxVec2::from_ints(1000, 1000), 0b01));
        assert!(fog.explored_cells().iter().any(|c| *c != 0));
    }

    #[test]
    fn radar_does_not_explore() {
        let mut fog = Fog::new(FxVec2::from_ints(4096, 4096));
        fog.begin();
        fog.reveal(
            FxVec2::from_ints(1000, 1000),
            Fx::ZERO,
            Fx::from_int(600),
            0b01,
        );
        assert!(!fog.is_visible(FxVec2::from_ints(1000, 1000), 0b01));
        assert!(fog.is_detected(FxVec2::from_ints(1500, 1000), 0b01));
        assert!(fog.explored_cells().iter().all(|c| *c == 0));
    }

    #[test]
    fn identify_survives_a_reused_row() {
        let mut fog = Fog::new(FxVec2::from_ints(4096, 4096));
        fog.identify(3, 1, 0b01);
        assert!(fog.is_identified(3, 1, 0b01));
        assert!(!fog.is_identified(3, 1, 0b10));
        fog.identify(3, 2, 0b10);
        assert!(!fog.is_identified(3, 1, 0b01));
        assert!(fog.is_identified(3, 2, 0b10));
    }
}
