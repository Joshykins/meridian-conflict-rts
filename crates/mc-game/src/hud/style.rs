//! The HUD's colour code: what a unit's tile is tinted by (where it fights, or
//! what it makes), which family an order belongs to, and veterancy.

use crate::ui::{ink, rgb, Rect, Ui};
use mc_data::{cat, MoveLayer, UnitBlueprint};

pub const LAND: u32 = 0x5FBF4A;
pub const AIR: u32 = 0x62C6FF;
pub const NAVY: u32 = 0x2F6BFF;
/// What a land unit's tile says it fights on: dry ground, a sand beige.
pub const GROUND: u32 = 0xCDB98F;
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
        } else if bp.water_build
            || bp.categories & (cat::LAND | cat::NAVAL) == cat::LAND | cat::NAVAL
        {
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
            Domain::Land => (GROUND, GROUND),
            Domain::Air => (AIR, AIR),
            Domain::Navy => (NAVY, NAVY),
            Domain::Both => (GROUND, NAVY),
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
    ui.gradient(
        lower,
        [
            rgb(left, 0.0),
            rgb(right, 0.0),
            rgb(right, foot),
            rgb(left, foot),
        ],
    );
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

/// What a thing is for, each with its own colour, so engineers, reclaimers,
/// tanks, mines and generators can be told apart at a glance. Declared in the
/// order a shelf lists them, so like sits beside like.
#[derive(Clone, Copy, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub enum Role {
    Engineer,
    Reclaimer,
    Factory,
    /// Everything that makes, keeps or recovers materials: mines, fabricators,
    /// materials stores, reclaim towers.
    Material,
    /// Power plants and energy stores.
    Energy,
    Combat,
    AntiAir,
    Artillery,
    Intel,
}

impl Role {
    pub fn of(bp: &UnitBlueprint) -> Role {
        let has = |c: u32| bp.categories & c != 0;
        let energy_store = bp.economy.energy_storage > mc_core::Fx::ZERO
            && bp.economy.mass_storage == mc_core::Fx::ZERO;
        if has(cat::ENGINEER) {
            Role::Engineer
        } else if bp.is_salvager() {
            Role::Reclaimer
        } else if has(cat::FACTORY) {
            Role::Factory
        } else if has(cat::POWER) || (has(cat::STORAGE) && energy_store) {
            Role::Energy
        } else if has(cat::EXTRACTOR | cat::ECONOMY | cat::STORAGE) {
            Role::Material
        } else if has(cat::ANTI_AIR) {
            Role::AntiAir
        } else if has(cat::ARTILLERY) {
            Role::Artillery
        } else if has(cat::INTEL | cat::SHIELD | cat::SCOUT) {
            Role::Intel
        } else {
            Role::Combat
        }
    }

    pub const fn tone(self) -> u32 {
        match self {
            Role::Engineer => 0x2FB8A8,
            Role::Reclaimer => 0x9A6BD6,
            Role::Factory => 0x7C8FB0,
            Role::Material => 0xC9803A,
            Role::Energy => 0xF2D338,
            Role::Combat => 0xD4483A,
            Role::AntiAir => 0x9BD13A,
            Role::Artillery => 0xB2406E,
            Role::Intel => 0xB4BCCB,
        }
    }
}

/// A tile's picture well in its role's colour, strongest at the top where it
/// used to be black, settling toward the foot.
pub fn role_backdrop(ui: &mut Ui, r: Rect, role: Role, glow: f32) {
    let tone = role.tone();
    ui.fill(r, ink(0.55));
    ui.gradient_v(
        r,
        rgb(tone, 0.34 + 0.1 * glow),
        rgb(tone, 0.12 + 0.06 * glow),
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
    pub const fn tone(self) -> u32 {
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
/// with its own key, left to right in this order: factories, economy,
/// defenses and intel, then the units a structure builder makes itself.
#[derive(Clone, Copy, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub enum Purpose {
    Builders,
    Economy,
    Combat,
    AntiAir,
    Artillery,
    Support,
    Units,
}

impl Purpose {
    pub const ALL: [Purpose; 7] = [
        Purpose::Builders,
        Purpose::Economy,
        Purpose::Combat,
        Purpose::AntiAir,
        Purpose::Artillery,
        Purpose::Support,
        Purpose::Units,
    ];

    /// Its shelf on a structure builder's panel (`structures`), or on a factory's,
    /// where everything is mobile and sorts by role instead.
    pub fn of(bp: &UnitBlueprint, structures: bool) -> Purpose {
        let has = |c: u32| bp.categories & c != 0;
        if structures && has(cat::MOBILE) {
            Purpose::Units
        } else if has(cat::ENGINEER | cat::FACTORY) || bp.is_salvager() {
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
            Purpose::Units => "Units",
        }
    }

    pub fn key(self) -> char {
        ['Q', 'W', 'E', 'R', 'T', 'Y', 'I'][self as usize]
    }
}

/// Keys for the items on a shelf, in order.
pub const ITEM_KEYS: [char; 9] = ['A', 'S', 'D', 'F', 'G', 'H', 'J', 'K', 'L'];
