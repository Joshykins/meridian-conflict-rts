//! The report's colour for what a figure counts, beside each side's colour for whose
//! it is: materials, energy, reclaim, armies, losses and every kind of unit keep one
//! colour on every page. Where the HUD already colours a thing (materials, energy,
//! domains, attack, engineering) the report uses the HUD's colour, so it reads as the
//! match did.

use super::analysis::{Metric, MomentKind};
use crate::hud;
use crate::hud::style::{Family, AIR, LAND, NAVY, SUPPORT};
use crate::ui::palette;

pub const MATERIALS: u32 = hud::MASS;
pub const ENERGY: u32 = hud::ENERGY;
pub const SALVAGE: u32 = 0x3FE0C5;
/// How much of the asked-for spending was paid.
pub const EFFICIENCY: u32 = hud::HEALTHY;
/// Fighting units: their number and worth.
pub const ARMY: u32 = 0xD4DCE8;
/// Units and structures finished, and tiers reached: the HUD's construction amber.
pub const BUILT: u32 = 0xFFA928;
pub const DESTROYED: u32 = Family::Combat.tone();
pub const LOST: u32 = 0x9097A6;
pub const STALLED: u32 = palette::WARN;

/// `analysis::DOMAINS`: land, air, naval, structures.
pub const DOMAINS: [u32; 4] = [LAND, AIR, NAVY, SUPPORT];

/// `analysis::SPEND_KINDS`: army, experimental, engineers, economy, defence, industry.
pub const SPEND: [u32; 6] = [
    ARMY,
    0xC77DFF,
    Family::Engineering.tone(),
    MATERIALS,
    0x5AA9FF,
    SUPPORT,
];

pub fn metric(m: Metric) -> u32 {
    match m {
        Metric::MassIncome | Metric::Collected | Metric::Spending | Metric::Stored => MATERIALS,
        Metric::EnergyIncome => ENERGY,
        Metric::Efficiency => EFFICIENCY,
        Metric::ArmyValue | Metric::ArmySize => ARMY,
        Metric::Destroyed => DESTROYED,
        Metric::Lost => LOST,
        Metric::ReclaimRate | Metric::Reclaimed => SALVAGE,
    }
}

pub fn moment(k: MomentKind) -> u32 {
    match k {
        MomentKind::Start => palette::DIM,
        MomentKind::FirstBlood | MomentKind::Battle => DESTROYED,
        MomentKind::Tier => BUILT,
        MomentKind::Domain => ARMY,
        MomentKind::Expansion => MATERIALS,
        MomentKind::Lead | MomentKind::End => palette::ACCENT,
        MomentKind::Salvage => SALVAGE,
        MomentKind::Experimental => SPEND[1],
        MomentKind::ExperimentalLost | MomentKind::Defeat => palette::BAD,
        MomentKind::Warhead => hud::silo::WARHEAD,
    }
}
