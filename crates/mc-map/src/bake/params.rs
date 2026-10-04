//! The layouts a map can be baked to, the parameters of a bake, and what
//! each layout asks of them.

use super::{crosswater, frostline, siege, tripoint};
use crate::format::MapError;
use crate::{MAX_START_POSITIONS, TILE_SIZE_M};

/// The overall shape of the map.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Layout {
    /// A continent of terraces, ranges and lakes around a central city, for any number of
    /// players up to [`MAX_START_POSITIONS`].
    #[default]
    Basin,
    /// A main island with a central lake and two flanking town islands. Two players only.
    Islands,
    /// "Serac Divide": a designed two-player mountain map on a coast (see
    /// `alpine.rs`). Fair by a mirror across the middle wherever units can
    /// go; the mountains, glaciers and woods around that are not mirrored.
    /// Wants 8 km.
    Alpine,
    /// "Serac Sound": the same country for four against four, the sea down
    /// the east side. Eight starts, the south team's first. Wants 12 km.
    AlpineTeams,
    /// "The Axis": four against four on a tropical archipelago, each player on
    /// an island of their own round a jungle island where the Meridian opens
    /// (see `archipelago.rs`). Fair by a half turn in everything that matters
    /// to play; the coasts and woods are not turned. The west side first,
    /// starts in pairs. Wants 20 km.
    Archipelago,
    /// "Halden's Grip": four against four across a land bridge between two
    /// bays, after Seton's Clutch (see `bays.rs`). Fair by a half turn; the
    /// south-west team first, starts in pairs. Wants 16 km.
    TwinBays,
    /// "The Threshold": the survival map (see `threshold.rs`). One road along a
    /// coast between mountains and the sea; three defender starts in the west,
    /// the Precursor facility filling the east half, its start last. Four start
    /// positions; wants 16 km.
    Threshold,
    /// "Vermilion Gorge": three against three across a desert canyon, a
    /// reservoir filling its middle and an arch dam across its slot (see
    /// `canyon.rs`). Fair by a mirror across the north-south middle line;
    /// the west side's starts first, each followed by its mirror. Wants 12 km.
    Canyon,
    /// "Frostline": four against four across a land bridge between two
    /// oceans, the Precursors' climate wall down the middle: desert west of
    /// it, Alaska east (see `frostline.rs`). The country lies turned a little
    /// on the map, the oceans opening into two of its corners. Fair by a half
    /// turn wherever units can go; the west side first, starts in pairs.
    /// Exactly 16 km.
    Frostline,
    /// "Crosswater": four players, free for all, round a lake with an island
    /// and four fords; a sea round the map and into a bay between every two
    /// neighbours (see `crosswater.rs`). Fair by a quarter turn; the south-west start first,
    /// the others counter-clockwise. Exactly 16 km.
    Crosswater,
    /// "Tripoint": three players, each for themselves, in three climates
    /// (Alaska, desert, jungle) parted by the Precursors' climate walls, which
    /// meet at the installation in the middle (see `tripoint.rs`). Fair by a
    /// third of a turn wherever units can go; starts Alaska, desert, jungle.
    /// Exactly 12 km.
    Tripoint,
    /// "Halcyon": three against three at the wall of a besieged city, the
    /// city north of it, its outskirts south (see `siege.rs`). Not fair by
    /// symmetry: the city's starts alternate with the outskirts'. Exactly
    /// 12 km.
    Siege,
}

#[derive(Clone, Debug)]
pub struct BakeParams {
    pub name: String,
    pub tiles_w: u32,
    pub tiles_h: u32,
    pub seed: u64,
    /// Start positions, `1..=MAX_START_POSITIONS`. [`Layout::Islands`] takes exactly 2.
    pub players: u32,
    /// Worker threads; 0 uses every core. Does not affect the result.
    pub threads: usize,
    pub layout: Layout,
}

impl BakeParams {
    /// A square map of `size_tiles` tiles (2.048 km each) per edge, with a
    /// player count that suits the size: 2 up to 8 km, 4 up to 24 km, else 8.
    pub fn square(name: &str, size_tiles: u32, seed: u64) -> BakeParams {
        let players = match size_tiles {
            0..=4 => 2,
            5..=12 => 4,
            _ => 8,
        };
        BakeParams {
            name: name.to_owned(),
            tiles_w: size_tiles,
            tiles_h: size_tiles,
            seed,
            players,
            threads: 0,
            layout: Layout::Basin,
        }
    }

    /// A square two-player [`Layout::Islands`] map. The layout is sized by the
    /// map, and wants 3 tiles (6 km) or more per edge to have room for its lanes.
    pub fn islands(name: &str, size_tiles: u32, seed: u64) -> BakeParams {
        BakeParams {
            players: 2,
            layout: Layout::Islands,
            ..BakeParams::square(name, size_tiles, seed)
        }
    }

    /// A square two-player [`Layout::Alpine`] map.
    pub fn alpine(name: &str, size_tiles: u32, seed: u64) -> BakeParams {
        BakeParams {
            players: 2,
            layout: Layout::Alpine,
            ..BakeParams::square(name, size_tiles, seed)
        }
    }

    /// A square eight-player [`Layout::AlpineTeams`] map.
    pub fn alpine_teams(name: &str, size_tiles: u32, seed: u64) -> BakeParams {
        BakeParams {
            players: 8,
            layout: Layout::AlpineTeams,
            ..BakeParams::square(name, size_tiles, seed)
        }
    }

    /// A square eight-player [`Layout::Archipelago`] map.
    pub fn archipelago(name: &str, size_tiles: u32, seed: u64) -> BakeParams {
        BakeParams {
            players: 8,
            layout: Layout::Archipelago,
            ..BakeParams::square(name, size_tiles, seed)
        }
    }

    /// A square eight-player [`Layout::TwinBays`] map.
    pub fn twin_bays(name: &str, size_tiles: u32, seed: u64) -> BakeParams {
        BakeParams {
            players: 8,
            layout: Layout::TwinBays,
            ..BakeParams::square(name, size_tiles, seed)
        }
    }

    /// A square six-player [`Layout::Canyon`] map.
    pub fn canyon(name: &str, size_tiles: u32, seed: u64) -> BakeParams {
        BakeParams {
            players: 6,
            layout: Layout::Canyon,
            ..BakeParams::square(name, size_tiles, seed)
        }
    }

    /// A square eight-player [`Layout::Frostline`] map.
    pub fn frostline(name: &str, size_tiles: u32, seed: u64) -> BakeParams {
        BakeParams {
            players: 8,
            layout: Layout::Frostline,
            ..BakeParams::square(name, size_tiles, seed)
        }
    }

    /// A square four-player [`Layout::Crosswater`] map.
    pub fn crosswater(name: &str, size_tiles: u32, seed: u64) -> BakeParams {
        BakeParams {
            players: 4,
            layout: Layout::Crosswater,
            ..BakeParams::square(name, size_tiles, seed)
        }
    }

    /// A square three-player [`Layout::Tripoint`] map.
    pub fn tripoint(name: &str, size_tiles: u32, seed: u64) -> BakeParams {
        BakeParams {
            players: 3,
            layout: Layout::Tripoint,
            ..BakeParams::square(name, size_tiles, seed)
        }
    }

    /// A square six-player [`Layout::Siege`] map.
    pub fn siege(name: &str, size_tiles: u32, seed: u64) -> BakeParams {
        BakeParams {
            players: 6,
            layout: Layout::Siege,
            ..BakeParams::square(name, size_tiles, seed)
        }
    }

    /// A square [`Layout::Threshold`] map: three defender starts and the facility's.
    pub fn threshold(name: &str, size_tiles: u32, seed: u64) -> BakeParams {
        BakeParams {
            players: 4,
            layout: Layout::Threshold,
            ..BakeParams::square(name, size_tiles, seed)
        }
    }
}

impl BakeParams {
    /// Whether the layout can be baked for these players at this size.
    pub(super) fn check(&self) -> Result<(), MapError> {
        if !(1..=MAX_START_POSITIONS as u32).contains(&self.players) {
            return Err(MapError::Invalid(format!(
                "players must be 1..={MAX_START_POSITIONS}"
            )));
        }
        if self.layout == Layout::Archipelago && self.players != 8 {
            return Err(MapError::Invalid(
                "the Archipelago layout is for exactly 8 players".into(),
            ));
        }
        if self.layout == Layout::TwinBays && self.players != 8 {
            return Err(MapError::Invalid(
                "the TwinBays layout is for exactly 8 players".into(),
            ));
        }
        if self.layout == Layout::AlpineTeams && self.players != 8 {
            return Err(MapError::Invalid(
                "the AlpineTeams layout is for exactly 8 players".into(),
            ));
        }
        if matches!(self.layout, Layout::Islands | Layout::Alpine) && self.players != 2 {
            return Err(MapError::Invalid(format!(
                "the {:?} layout is for exactly 2 players",
                self.layout
            )));
        }
        if self.layout == Layout::Canyon
            && (self.players != 6 || self.tiles_w != 6 || self.tiles_h != 6)
        {
            return Err(MapError::Invalid(
                "the Canyon layout is for exactly 6 players on a 12 km map".into(),
            ));
        }
        if self.layout == Layout::Frostline
            && (self.players != 8
                || (self.tiles_w as i32 * TILE_SIZE_M) as f64 != frostline::SIZE
                || self.tiles_h != self.tiles_w)
        {
            return Err(MapError::Invalid(
                "the Frostline layout is for exactly 8 players on a 16 km map".into(),
            ));
        }
        if self.layout == Layout::Crosswater
            && (self.players != 4
                || (self.tiles_w as i32 * TILE_SIZE_M) as f64 != crosswater::SIZE
                || self.tiles_h != self.tiles_w)
        {
            return Err(MapError::Invalid(
                "the Crosswater layout is for exactly 4 players on a 16 km map".into(),
            ));
        }
        if self.layout == Layout::Tripoint
            && (self.players != 3
                || (self.tiles_w as i32 * TILE_SIZE_M) as f64 != tripoint::SIZE
                || self.tiles_h != self.tiles_w)
        {
            return Err(MapError::Invalid(
                "the Tripoint layout is for exactly 3 players on a 12 km map".into(),
            ));
        }
        if self.layout == Layout::Siege
            && (self.players != 6
                || (self.tiles_w as i32 * TILE_SIZE_M) as f64 != siege::SIZE
                || self.tiles_h != self.tiles_w)
        {
            return Err(MapError::Invalid(
                "the Siege layout is for exactly 6 players on a 12 km map".into(),
            ));
        }
        if self.layout == Layout::Threshold && self.players != 4 {
            return Err(MapError::Invalid(
                "the survival layout has exactly 4 starts (3 defenders and the engine)".into(),
            ));
        }
        Ok(())
    }
}
