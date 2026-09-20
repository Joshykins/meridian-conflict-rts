//! Fog of war: per-player visibility on a coarse grid.
//!
//! Rebuilt from unit positions every tick, so it is derived state and is not
//! hashed or snapshotted. `explored` and `identified` accumulate; only the UI
//! reads them. Radar detects units without lighting the ground.

use mc_core::{Fx, FxVec2};

/// Fog cell edge: 64 m.
const CELL_SHIFT: u32 = 6;

pub struct Fog {
    width: i32,
    height: i32,
    /// Bit `p` set: player `p` sees this cell right now.
    visible: Vec<u8>,
    /// Bit `p` set: the cell is inside player `p`'s radar coverage.
    radar: Vec<u8>,
    explored: Vec<u8>,
    /// Bit `p` set: player `p` has seen this unit row with vision. A generation
    /// stamp keeps a reused row from staying known.
    identified: Vec<u8>,
    identified_gen: Vec<u16>,
    /// Bumped on every rebuild so the renderer knows when to re-upload.
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
            explored: vec![0; n],
            identified: Vec::new(),
            identified_gen: Vec::new(),
            version: 0,
        }
    }

    pub fn dims(&self) -> (u32, u32) {
        (self.width as u32, self.height as u32)
    }

    pub fn cell_size(&self) -> u32 {
        1 << CELL_SHIFT
    }

    pub fn begin(&mut self) {
        self.visible.fill(0);
        self.radar.fill(0);
        self.version = self.version.wrapping_add(1);
    }

    /// Marks the disc around `pos` for the players in `mask`.
    pub fn reveal(&mut self, pos: FxVec2, vision: Fx, radar: Fx, mask: u8) {
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

    fn stamp(
        grid: &mut [u8],
        mut also: Option<&mut Vec<u8>>,
        width: i32,
        height: i32,
        pos: FxVec2,
        radius: Fx,
        mask: u8,
    ) {
        let cx = pos.x.floor_int() >> CELL_SHIFT;
        let cy = pos.y.floor_int() >> CELL_SHIFT;
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

    #[inline]
    fn cell(&self, pos: FxVec2) -> usize {
        let x = (pos.x.floor_int() >> CELL_SHIFT).clamp(0, self.width - 1);
        let y = (pos.y.floor_int() >> CELL_SHIFT).clamp(0, self.height - 1);
        (y * self.width + x) as usize
    }

    /// True when any player in `mask` currently sees `pos`.
    #[inline]
    pub fn is_visible(&self, pos: FxVec2, mask: u8) -> bool {
        self.visible[self.cell(pos)] & mask != 0
    }

    /// Visible or on radar: good enough to shoot at.
    #[inline]
    pub fn is_detected(&self, pos: FxVec2, mask: u8) -> bool {
        let c = self.cell(pos);
        (self.visible[c] | self.radar[c]) & mask != 0
    }

    /// Players who currently see `pos` with vision.
    #[inline]
    pub fn visible_mask(&self, pos: FxVec2) -> u8 {
        self.visible[self.cell(pos)]
    }

    /// Records that the players in `mask` have seen this occupant with vision.
    pub fn identify(&mut self, row: usize, generation: u16, mask: u8) {
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
    pub fn is_identified(&self, row: usize, generation: u16, mask: u8) -> bool {
        self.identified
            .get(row)
            .is_some_and(|&m| self.identified_gen[row] == generation && m & mask != 0)
    }

    pub fn visible_cells(&self) -> &[u8] {
        &self.visible
    }

    pub fn radar_cells(&self) -> &[u8] {
        &self.radar
    }

    pub fn explored_cells(&self) -> &[u8] {
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
