//! The authoring schema. These types mirror the RON files one to one and are
//! converted to the fixed-point blueprint tables by `compile`.

use crate::{cat, BlueprintId, Builder, DataError, Economy, FactionId, Motion, UnitBlueprint, Visual, Weapon, MAX_WEAPONS};
use mc_core::{Fx, FxVec3, TICKS_PER_SECOND};
use serde::Deserialize;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize)]
pub enum MoveLayer {
    Land,
    Amphibious,
    Naval,
    Hover,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize)]
pub enum Trajectory {
    /// Flat, fast shot. Blocked by terrain in the way.
    Direct,
    /// Lobbed under gravity; clears terrain and walls.
    Ballistic,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize)]
pub enum WeaponColor {
    Blue,
    Orange,
}

/// Strategic icon shape. Tech level adds pips; the owner adds colour.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize)]
pub enum IconKind {
    Commander,
    Engineer,
    Bot,
    Tank,
    Artillery,
    AntiAir,
    Scout,
    Factory,
    Extractor,
    Power,
    Storage,
    Defense,
    Intel,
    Wall,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Faction {
    pub key: String,
    pub name: String,
    pub abbreviation: String,
    pub description: String,
    pub commander: String,
    pub plating_color: [f32; 3],
    pub accent_color: [f32; 3],
    pub highlight_color: [f32; 3],
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Unit {
    pub key: String,
    pub name: String,
    pub role: String,
    pub tech: u8,
    pub categories: Vec<String>,
    pub health: f64,
    #[serde(default)]
    pub regen: f64,
    pub cost: Cost,
    pub radius: f64,
    pub height: f64,
    #[serde(default)]
    pub footprint: (u8, u8),
    pub vision: f64,
    #[serde(default)]
    pub radar: f64,
    #[serde(default)]
    pub motion: Option<RawMotion>,
    #[serde(default)]
    pub economy: RawEconomy,
    #[serde(default)]
    pub needs_deposit: bool,
    #[serde(default)]
    pub builder: Option<RawBuilder>,
    #[serde(default)]
    pub upgrades_to: Option<String>,
    #[serde(default)]
    pub weapons: Vec<RawWeapon>,
    #[serde(default = "default_wreck")]
    pub wreck_fraction: f64,
    pub mesh: String,
    pub icon: IconKind,
}

fn default_wreck() -> f64 {
    0.81
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cost {
    pub mass: f64,
    pub energy: f64,
    pub time: f64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawMotion {
    pub layer: MoveLayer,
    pub size: u8,
    pub speed: f64,
    pub accel: f64,
    /// Degrees per second.
    pub turn: f64,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields, default)]
pub struct RawEconomy {
    pub mass_income: f64,
    pub energy_income: f64,
    pub energy_upkeep: f64,
    pub mass_storage: f64,
    pub energy_storage: f64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawBuilder {
    pub power: f64,
    pub range: f64,
    pub builds: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawWeapon {
    pub name: String,
    pub damage: f64,
    #[serde(default)]
    pub splash: f64,
    #[serde(default)]
    pub range_min: f64,
    pub range: f64,
    /// Seconds between salvos.
    pub reload: f64,
    #[serde(default = "one")]
    pub salvo: u8,
    /// Seconds between the shots of a salvo.
    #[serde(default)]
    pub salvo_delay: f64,
    pub speed: f64,
    pub trajectory: Trajectory,
    /// Degrees per second; zero fixes the weapon to the hull.
    #[serde(default)]
    pub turret_turn: f64,
    /// Full firing arc in degrees, centred on the hull's forward axis.
    #[serde(default = "full_arc")]
    pub arc: f64,
    pub muzzle: (f64, f64, f64),
    /// Aim error in degrees.
    #[serde(default)]
    pub spread: f64,
    pub targets: Vec<String>,
    pub color: WeaponColor,
}

fn one() -> u8 {
    1
}

fn full_arc() -> f64 {
    360.0
}

fn fx(v: f64) -> Fx {
    Fx((v * Fx::ONE.0 as f64).round() as i64)
}

/// Degrees to angle steps.
fn steps(deg: f64) -> f64 {
    deg * 65536.0 / 360.0
}

fn ticks(seconds: f64) -> u32 {
    (seconds * TICKS_PER_SECOND as f64).round().max(0.0) as u32
}

fn mask(names: &[String], ctx: &str) -> Result<u32, DataError> {
    let mut m = 0;
    for n in names {
        m |= cat::parse(n).ok_or_else(|| DataError::Invalid(format!("{ctx}: unknown category {n}")))?;
    }
    Ok(m)
}

impl Unit {
    pub(crate) fn compile(
        &self,
        id: BlueprintId,
        faction: FactionId,
        lookup: &dyn Fn(&str, &str) -> Result<BlueprintId, DataError>,
    ) -> Result<UnitBlueprint, DataError> {
        let key = &self.key;
        if self.weapons.len() > MAX_WEAPONS {
            return Err(DataError::Invalid(format!("{key}: more than {MAX_WEAPONS} weapons")));
        }
        let mut categories = mask(&self.categories, key)?;
        categories |= if self.motion.is_some() { cat::MOBILE } else { cat::STRUCTURE };

        let motion = match &self.motion {
            Some(m) => {
                if m.size > 3 {
                    return Err(DataError::Invalid(format!("{key}: size class must be 0..=3")));
                }
                Some(Motion {
                    layer: m.layer,
                    size_class: m.size,
                    speed: fx(m.speed),
                    accel: fx(m.accel),
                    turn_rate: (steps(m.turn) / TICKS_PER_SECOND as f64).round().clamp(1.0, 32767.0) as u16,
                })
            }
            None => None,
        };

        let builder = match &self.builder {
            Some(b) => {
                let mut builds = Vec::with_capacity(b.builds.len());
                for k in &b.builds {
                    builds.push(lookup(k, key)?);
                }
                Some(Builder { power: fx(b.power), range: fx(b.range), builds })
            }
            None => None,
        };

        let mut weapons = Vec::with_capacity(self.weapons.len());
        for w in &self.weapons {
            let ctx = format!("{key}/{}", w.name);
            weapons.push(Weapon {
                name: w.name.clone(),
                damage: fx(w.damage),
                splash: fx(w.splash),
                range_min: fx(w.range_min),
                range_max: fx(w.range),
                reload_ticks: ticks(w.reload).clamp(1, u16::MAX as u32) as u16,
                salvo: w.salvo.max(1),
                salvo_delay_ticks: ticks(w.salvo_delay).clamp(0, 255) as u8,
                projectile_speed: fx(w.speed),
                trajectory: w.trajectory,
                turret_turn: (steps(w.turret_turn) / TICKS_PER_SECOND as f64).round().clamp(0.0, 32767.0) as u16,
                half_arc: if w.arc >= 360.0 { 0x8000 } else { (steps(w.arc) / 2.0).round() as u16 },
                muzzle: FxVec3::new(fx(w.muzzle.0), fx(w.muzzle.1), fx(w.muzzle.2)),
                spread: steps(w.spread).round() as u16,
                target_mask: mask(&w.targets, &ctx)?,
                color: w.color,
            });
        }

        let e = &self.economy;
        Ok(UnitBlueprint {
            id,
            key: key.clone(),
            name: self.name.clone(),
            role: self.role.clone(),
            faction,
            tech: self.tech,
            categories,
            health: fx(self.health),
            regen: fx(self.regen),
            cost_mass: fx(self.cost.mass),
            cost_energy: fx(self.cost.energy),
            build_time: fx(self.cost.time),
            radius: fx(self.radius),
            height: fx(self.height),
            footprint: self.footprint,
            vision: fx(self.vision),
            radar: fx(self.radar),
            motion,
            economy: Economy {
                mass_income: fx(e.mass_income),
                energy_income: fx(e.energy_income),
                energy_upkeep: fx(e.energy_upkeep),
                mass_storage: fx(e.mass_storage),
                energy_storage: fx(e.energy_storage),
            },
            needs_deposit: self.needs_deposit,
            builder,
            upgrades_to: match &self.upgrades_to {
                Some(k) => Some(lookup(k, key)?),
                None => None,
            },
            weapons,
            wreck_fraction: fx(self.wreck_fraction),
            visual: Visual { mesh: self.mesh.clone(), icon: self.icon },
        })
    }
}
