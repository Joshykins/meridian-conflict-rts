//! The HUD's colour code: what a unit's tile is tinted by (where it fights, or
//! what it makes), which family an order belongs to, and veterancy.

use crate::ui::{rgb, Rect, Ui};
use mc_data::{cat, MoveLayer, UnitBlueprint};

pub const LAND: u32 = 0x5FBF4A;
pub const AIR: u32 = 0x62C6FF;
pub const NAVY: u32 = 0x2F6BFF;
/// Structures that fight nothing and make nothing that moves: extractors, power, walls.
pub const SUPPORT: u32 = 0x8C8C88;
/// Veterancy and everything to do with it.
pub const VETERANCY: u32 = 0xFFD23C;

/// Where a unit lives, or what a structure makes: one colour, or two for both.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Domain {
    Land,
    Air,
    Navy,
    /// Land and water: amphibious and hover units, structures that stand on either.
    Both,
    Support,
}

impl Domain {
    pub fn of(bp: &UnitBlueprint) -> Domain {
        if let Some(m) = &bp.motion {
            return match m.layer {
                MoveLayer::Land => Domain::Land,
                MoveLayer::Air => Domain::Air,
                MoveLayer::Naval => Domain::Navy,
                MoveLayer::Amphibious | MoveLayer::Hover => Domain::Both,
            };
        }
        // A structure: the air flag on a factory means what it makes.
        if bp.categories & cat::AIR != 0 {
            Domain::Air
        } else if bp.water_build || bp.categories & (cat::LAND | cat::NAVAL) == cat::LAND | cat::NAVAL {
            Domain::Both
        } else if bp.categories & cat::NAVAL != 0 {
            Domain::Navy
        } else if bp.categories & (cat::FACTORY | cat::DEFENSE) != 0 {
            Domain::Land
        } else {
            Domain::Support
        }
    }

    /// The colours, left and right. The same twice for one domain.
    pub fn tones(self) -> (u32, u32) {
        match self {
            Domain::Land => (LAND, LAND),
            Domain::Air => (AIR, AIR),
            Domain::Navy => (NAVY, NAVY),
            Domain::Both => (LAND, NAVY),
            Domain::Support => (SUPPORT, SUPPORT),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Domain::Land => "Land",
            Domain::Air => "Air",
            Domain::Navy => "Naval",
            Domain::Both => "Land \u{b7} Naval",
            Domain::Support => "Support",
        }
    }
}

/// A tile's backing in its domain's colour: dark at the top, a stronger band
/// at the foot, split down the middle for a unit that works on both.
pub fn domain_wash(ui: &mut Ui, r: Rect, domain: Domain, glow: f32) {
    let (left, right) = domain.tones();
    // Quiet: the colour rises from the foot and is gone by the middle, so it says
    // what the unit is without tinting the picture. Two colours blend across.
    let foot = 0.13 + 0.09 * glow;
    let lower = Rect::new(r.x, r.y + r.h * 0.4, r.w, r.h * 0.6);
    ui.gradient(lower, [rgb(left, 0.0), rgb(right, 0.0), rgb(right, foot), rgb(left, foot)]);
    // The domain as a solid edge along the bottom.
    let h = 2.0;
    ui.fill(
        Rect::new(r.x, r.bottom() - h, r.w * 0.5, h),
        rgb(left, 0.85),
    );
    ui.fill(
        Rect::new(r.x + r.w * 0.5, r.bottom() - h, r.w * 0.5, h),
        rgb(right, 0.85),
    );
}

/// The families on the order card, each with its own colour.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Family {
    Movement,
    Combat,
    Stance,
    Engineering,
    Control,
    /// A lift ship's landing, loading and unloading.
    Transport,
}

impl Family {
    pub fn tone(self) -> u32 {
        match self {
            Family::Movement => 0x7FD0FF,
            Family::Combat => 0xFF4B3A,
            Family::Stance => 0xFFB43C,
            Family::Engineering => 0x78E08A,
            Family::Control => 0xF2F2F0,
            Family::Transport => AIR,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Family::Movement => "Move",
            Family::Combat => "Attack",
            Family::Stance => "Stance",
            Family::Engineering => "Work",
            Family::Control => "Control",
            Family::Transport => "Transport",
        }
    }
}

/// What something is built for: the shelves of the construction panel, each
/// with its own key.
#[derive(Clone, Copy, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub enum Purpose {
    Economy,
    Builders,
    Combat,
    AntiAir,
    Artillery,
    Support,
}

impl Purpose {
    pub const ALL: [Purpose; 6] = [
        Purpose::Economy,
        Purpose::Builders,
        Purpose::Combat,
        Purpose::AntiAir,
        Purpose::Artillery,
        Purpose::Support,
    ];

    pub fn of(bp: &UnitBlueprint) -> Purpose {
        let has = |c: u32| bp.categories & c != 0;
        if has(cat::ENGINEER | cat::FACTORY) {
            Purpose::Builders
        } else if has(cat::EXTRACTOR | cat::POWER | cat::STORAGE | cat::ECONOMY) {
            Purpose::Economy
        } else if has(cat::ANTI_AIR) {
            Purpose::AntiAir
        } else if has(cat::ARTILLERY) {
            Purpose::Artillery
        } else if has(cat::INTEL | cat::SHIELD | cat::SCOUT) {
            Purpose::Support
        } else {
            Purpose::Combat
        }
    }

    /// Its name on a structure builder's panel, or on a factory's.
    pub fn label(self, structures: bool) -> &'static str {
        match self {
            Purpose::Economy => "Economy",
            Purpose::Builders if structures => "Factories",
            Purpose::Builders => "Builders",
            Purpose::Combat if structures => "Defense",
            Purpose::Combat => "Combat",
            Purpose::AntiAir => "Anti-Air",
            Purpose::Artillery => "Artillery",
            Purpose::Support => "Intel",
        }
    }

    pub fn key(self) -> char {
        ['Q', 'W', 'E', 'R', 'T', 'Y'][self as usize]
    }
}

/// Keys for the items on a shelf, in order.
pub const ITEM_KEYS: [char; 9] = ['A', 'S', 'D', 'F', 'G', 'H', 'J', 'K', 'L'];
