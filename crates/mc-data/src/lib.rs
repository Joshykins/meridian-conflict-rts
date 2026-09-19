//! Factions and unit blueprints.
//!
//! Blueprints are authored as RON under `data/factions/<faction>/`. Authoring
//! uses decimal literals; loading converts them once into fixed point, so the
//! simulation only ever sees integers. Decimal parsing and the scale by 2^16
//! are exactly rounded IEEE operations, so every machine compiles the same
//! tables, and [`Blueprints::content_hash`] lets peers verify that before a
//! match starts.

mod raw;

use mc_core::{Fx, FxVec3, StateHasher};
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

pub use raw::{IconKind, MoveLayer, Trajectory, WeaponColor};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct BlueprintId(pub u16);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct FactionId(pub u8);

impl BlueprintId {
    #[inline]
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// Category bits. Weapons pick targets by mask, build menus and the AI filter by them.
pub mod cat {
    pub const MOBILE: u32 = 1 << 0;
    pub const STRUCTURE: u32 = 1 << 1;
    pub const LAND: u32 = 1 << 2;
    pub const AIR: u32 = 1 << 3;
    pub const NAVAL: u32 = 1 << 4;
    pub const COMMANDER: u32 = 1 << 5;
    pub const ENGINEER: u32 = 1 << 6;
    pub const FACTORY: u32 = 1 << 7;
    pub const ECONOMY: u32 = 1 << 8;
    pub const DEFENSE: u32 = 1 << 9;
    pub const DIRECT_FIRE: u32 = 1 << 10;
    pub const ARTILLERY: u32 = 1 << 11;
    pub const ANTI_AIR: u32 = 1 << 12;
    pub const SCOUT: u32 = 1 << 13;
    pub const WALL: u32 = 1 << 14;
    pub const EXTRACTOR: u32 = 1 << 15;
    pub const POWER: u32 = 1 << 16;
    pub const STORAGE: u32 = 1 << 17;
    pub const INTEL: u32 = 1 << 18;
    pub const EXPERIMENTAL: u32 = 1 << 19;

    pub(crate) fn parse(name: &str) -> Option<u32> {
        Some(match name {
            "Mobile" => MOBILE,
            "Structure" => STRUCTURE,
            "Land" => LAND,
            "Air" => AIR,
            "Naval" => NAVAL,
            "Commander" => COMMANDER,
            "Engineer" => ENGINEER,
            "Factory" => FACTORY,
            "Economy" => ECONOMY,
            "Defense" => DEFENSE,
            "DirectFire" => DIRECT_FIRE,
            "Artillery" => ARTILLERY,
            "AntiAir" => ANTI_AIR,
            "Scout" => SCOUT,
            "Wall" => WALL,
            "Extractor" => EXTRACTOR,
            "Power" => POWER,
            "Storage" => STORAGE,
            "Intel" => INTEL,
            "Experimental" => EXPERIMENTAL,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug)]
pub struct Faction {
    pub id: FactionId,
    /// Name used in code, asset names and short labels: "Aster".
    pub key: String,
    /// Full name: "Asterian Reach Command".
    pub name: String,
    pub abbreviation: String,
    pub description: String,
    /// The unit a player of this faction starts with.
    pub commander: BlueprintId,
    /// Linear RGB, presentation only.
    pub plating_color: [f32; 3],
    pub accent_color: [f32; 3],
    pub highlight_color: [f32; 3],
}

#[derive(Clone, Copy, Debug)]
pub struct Motion {
    pub layer: MoveLayer,
    /// 0..=3, see `mc-path`.
    pub size_class: u8,
    /// Metres per second.
    pub speed: Fx,
    /// Metres per second squared.
    pub accel: Fx,
    /// Angle steps per tick.
    pub turn_rate: u16,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Economy {
    /// Per second.
    pub mass_income: Fx,
    pub energy_income: Fx,
    /// Per second, drawn while the unit is active.
    pub energy_upkeep: Fx,
    pub mass_storage: Fx,
    pub energy_storage: Fx,
}

#[derive(Clone, Debug)]
pub struct Builder {
    /// Build time units contributed per second.
    pub power: Fx,
    pub range: Fx,
    pub builds: Vec<BlueprintId>,
}

#[derive(Clone, Debug)]
pub struct Weapon {
    pub name: String,
    pub damage: Fx,
    /// Splash radius; zero for a single-target hit.
    pub splash: Fx,
    pub range_min: Fx,
    pub range_max: Fx,
    pub reload_ticks: u16,
    /// Shots per reload cycle and the gap between them.
    pub salvo: u8,
    pub salvo_delay_ticks: u8,
    /// Metres per second.
    pub projectile_speed: Fx,
    pub trajectory: Trajectory,
    /// Angle steps per tick. Zero means the weapon is fixed to the hull.
    pub turret_turn: u16,
    /// Half-width of the firing arc around the hull's forward axis; 0x8000 is all-round.
    pub half_arc: u16,
    /// Muzzle position in unit space (x forward, y left, z up).
    pub muzzle: FxVec3,
    /// Random aim error, angle steps.
    pub spread: u16,
    pub target_mask: u32,
    pub color: WeaponColor,
}

#[derive(Clone, Debug)]
pub struct Visual {
    pub mesh: String,
    pub icon: IconKind,
}

#[derive(Clone, Debug)]
pub struct UnitBlueprint {
    pub id: BlueprintId,
    pub key: String,
    pub name: String,
    pub role: String,
    pub faction: FactionId,
    pub tech: u8,
    pub categories: u32,
    pub health: Fx,
    /// Health per second.
    pub regen: Fx,
    pub cost_mass: Fx,
    pub cost_energy: Fx,
    /// Build time units; divide by build power for seconds.
    pub build_time: Fx,
    /// Collision radius on the ground plane.
    pub radius: Fx,
    pub height: Fx,
    /// Structure size in build cells. `(0, 0)` for mobile units.
    pub footprint: (u8, u8),
    pub vision: Fx,
    pub radar: Fx,
    pub motion: Option<Motion>,
    pub economy: Economy,
    /// Structure must sit on a mass deposit.
    pub needs_deposit: bool,
    pub builder: Option<Builder>,
    pub upgrades_to: Option<BlueprintId>,
    pub weapons: Vec<Weapon>,
    /// Share of `cost_mass` left in the wreck.
    pub wreck_fraction: Fx,
    pub visual: Visual,
}

impl UnitBlueprint {
    #[inline]
    pub fn has(&self, categories: u32) -> bool {
        self.categories & categories == categories
    }

    #[inline]
    pub fn is_structure(&self) -> bool {
        self.categories & cat::STRUCTURE != 0
    }

    #[inline]
    pub fn is_mobile(&self) -> bool {
        self.motion.is_some()
    }

    pub fn max_weapon_range(&self) -> Fx {
        self.weapons.iter().map(|w| w.range_max).max().unwrap_or(Fx::ZERO)
    }
}

/// Most weapons one unit can carry. The sim stores weapon state in fixed slots.
pub const MAX_WEAPONS: usize = 4;

/// Every faction and unit, indexed by id. Immutable for the length of a match.
#[derive(Clone, Debug, Default)]
pub struct Blueprints {
    pub factions: Vec<Faction>,
    pub units: Vec<UnitBlueprint>,
    by_key: BTreeMap<String, BlueprintId>,
}

#[derive(Debug)]
pub enum DataError {
    Io(PathBuf, std::io::Error),
    Parse(PathBuf, String),
    Invalid(String),
}

impl fmt::Display for DataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DataError::Io(p, e) => write!(f, "{}: {e}", p.display()),
            DataError::Parse(p, e) => write!(f, "{}: {e}", p.display()),
            DataError::Invalid(e) => write!(f, "invalid blueprint data: {e}"),
        }
    }
}

impl std::error::Error for DataError {}

impl Blueprints {
    /// Loads every faction under `<data_dir>/factions`. Factions and units get
    /// ids in sorted key order, so ids do not depend on directory listing order.
    pub fn load(data_dir: &Path) -> Result<Blueprints, DataError> {
        let factions_dir = data_dir.join("factions");
        let mut sources = Vec::new();
        for dir in sorted_entries(&factions_dir)? {
            if !dir.is_dir() {
                continue;
            }
            let faction_path = dir.join("faction.ron");
            let faction: raw::Faction = parse_file(&faction_path)?;
            let mut units = Vec::new();
            for file in sorted_entries(&dir.join("units"))? {
                if file.extension().is_some_and(|e| e == "ron") {
                    let list: Vec<raw::Unit> = parse_file(&file)?;
                    units.extend(list);
                }
            }
            sources.push((faction, units));
        }
        Self::compile(sources)
    }

    /// Finds `data/` next to the executable or in a parent of the working directory.
    pub fn locate_data_dir() -> Option<PathBuf> {
        let mut roots = Vec::new();
        if let Ok(exe) = std::env::current_exe() {
            roots.extend(exe.ancestors().skip(1).map(Path::to_path_buf));
        }
        if let Ok(cwd) = std::env::current_dir() {
            roots.extend(cwd.ancestors().map(Path::to_path_buf));
        }
        roots.into_iter().map(|r| r.join("data")).find(|d| d.join("factions").is_dir())
    }

    fn compile(mut sources: Vec<(raw::Faction, Vec<raw::Unit>)>) -> Result<Blueprints, DataError> {
        sources.sort_by(|a, b| a.0.key.cmp(&b.0.key));
        let mut all: Vec<(u8, raw::Unit)> = Vec::new();
        for (fi, (_, units)) in sources.iter_mut().enumerate() {
            all.extend(units.drain(..).map(|u| (fi as u8, u)));
        }
        all.sort_by(|a, b| a.1.key.cmp(&b.1.key));
        if all.len() > u16::MAX as usize {
            return Err(DataError::Invalid("too many unit blueprints".into()));
        }
        let mut by_key = BTreeMap::new();
        for (i, (_, u)) in all.iter().enumerate() {
            if by_key.insert(u.key.clone(), BlueprintId(i as u16)).is_some() {
                return Err(DataError::Invalid(format!("duplicate unit key {}", u.key)));
            }
        }
        let lookup = |key: &str, ctx: &str| {
            by_key
                .get(key)
                .copied()
                .ok_or_else(|| DataError::Invalid(format!("{ctx}: unknown unit {key}")))
        };

        let mut units = Vec::with_capacity(all.len());
        for (i, (fi, u)) in all.iter().enumerate() {
            units.push(u.compile(BlueprintId(i as u16), FactionId(*fi), &lookup)?);
        }
        let mut factions = Vec::new();
        for (i, (f, _)) in sources.iter().enumerate() {
            factions.push(Faction {
                id: FactionId(i as u8),
                key: f.key.clone(),
                name: f.name.clone(),
                abbreviation: f.abbreviation.clone(),
                description: f.description.clone(),
                commander: lookup(&f.commander, &f.key)?,
                plating_color: f.plating_color,
                accent_color: f.accent_color,
                highlight_color: f.highlight_color,
            });
        }
        Ok(Blueprints { factions, units, by_key })
    }

    #[inline]
    pub fn unit(&self, id: BlueprintId) -> &UnitBlueprint {
        &self.units[id.index()]
    }

    pub fn unit_by_key(&self, key: &str) -> Option<&UnitBlueprint> {
        self.by_key.get(key).map(|id| self.unit(*id))
    }

    pub fn id_of(&self, key: &str) -> Option<BlueprintId> {
        self.by_key.get(key).copied()
    }

    pub fn faction_by_key(&self, key: &str) -> Option<&Faction> {
        self.factions.iter().find(|f| f.key.eq_ignore_ascii_case(key))
    }

    /// Hash of everything that can influence the simulation. Names, meshes and
    /// colours are left out: they cannot cause a desync.
    pub fn content_hash(&self) -> u64 {
        let mut h = StateHasher::new();
        h.write_u64(self.units.len() as u64);
        for u in &self.units {
            h.write_u8s(u.key.as_bytes());
            h.write_u64(u.faction.0 as u64);
            h.write_u64(u.tech as u64);
            h.write_u64(u.categories as u64);
            for v in [u.health, u.regen, u.cost_mass, u.cost_energy, u.build_time, u.radius, u.height, u.vision, u.radar, u.wreck_fraction] {
                h.write_i64(v.0);
            }
            h.write_u64(u.footprint.0 as u64 | (u.footprint.1 as u64) << 8);
            match &u.motion {
                Some(m) => {
                    h.write_u64(1 + m.layer as u64 | (m.size_class as u64) << 8 | (m.turn_rate as u64) << 16);
                    h.write_i64(m.speed.0);
                    h.write_i64(m.accel.0);
                }
                None => h.write_u64(0),
            }
            let e = &u.economy;
            for v in [e.mass_income, e.energy_income, e.energy_upkeep, e.mass_storage, e.energy_storage] {
                h.write_i64(v.0);
            }
            h.write_u64(u.needs_deposit as u64);
            match &u.builder {
                Some(b) => {
                    h.write_i64(b.power.0);
                    h.write_i64(b.range.0);
                    h.write_u64(b.builds.len() as u64);
                    for id in &b.builds {
                        h.write_u64(id.0 as u64);
                    }
                }
                None => h.write_u64(u64::MAX),
            }
            h.write_u64(u.upgrades_to.map_or(u64::MAX, |id| id.0 as u64));
            h.write_u64(u.weapons.len() as u64);
            for w in &u.weapons {
                for v in [w.damage, w.splash, w.range_min, w.range_max, w.projectile_speed, w.muzzle.x, w.muzzle.y, w.muzzle.z] {
                    h.write_i64(v.0);
                }
                h.write_u64(w.reload_ticks as u64 | (w.salvo as u64) << 16 | (w.salvo_delay_ticks as u64) << 24 | (w.trajectory as u64) << 32);
                h.write_u64(w.turret_turn as u64 | (w.half_arc as u64) << 16 | (w.spread as u64) << 32);
                h.write_u64(w.target_mask as u64);
            }
        }
        h.write_u64(self.factions.len() as u64);
        for f in &self.factions {
            h.write_u8s(f.key.as_bytes());
            h.write_u64(f.commander.0 as u64);
        }
        h.finish()
    }
}

fn sorted_entries(dir: &Path) -> Result<Vec<PathBuf>, DataError> {
    let read = std::fs::read_dir(dir).map_err(|e| DataError::Io(dir.to_path_buf(), e))?;
    let mut paths = Vec::new();
    for entry in read {
        paths.push(entry.map_err(|e| DataError::Io(dir.to_path_buf(), e))?.path());
    }
    paths.sort();
    Ok(paths)
}

fn parse_file<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, DataError> {
    let text = std::fs::read_to_string(path).map_err(|e| DataError::Io(path.to_path_buf(), e))?;
    // Optional fields are written bare (`motion: (..)`), not wrapped in `Some`.
    ron::Options::default()
        .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
        .from_str(&text)
        .map_err(|e| DataError::Parse(path.to_path_buf(), e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")
    }

    #[test]
    fn aster_loads_and_is_consistent() {
        let bp = Blueprints::load(&data_dir()).unwrap();
        let aster = bp.faction_by_key("aster").expect("Aster is defined");
        assert_eq!(aster.name, "Asterian Reach Command");
        let acu = bp.unit(aster.commander);
        assert!(acu.has(cat::COMMANDER | cat::MOBILE));
        assert!(acu.builder.is_some());

        for u in &bp.units {
            assert!(u.weapons.len() <= MAX_WEAPONS, "{}", u.key);
            assert!(u.health > Fx::ZERO, "{}", u.key);
            assert_eq!(u.is_structure(), u.footprint != (0, 0), "{} footprint", u.key);
            assert_eq!(u.is_structure(), u.motion.is_none(), "{} motion", u.key);
            for w in &u.weapons {
                assert!(w.range_max > w.range_min && w.reload_ticks > 0, "{} {}", u.key, w.name);
            }
        }
        // Everything except the commander can be built by something.
        for u in &bp.units {
            let buildable = bp.units.iter().any(|b| {
                b.builder.as_ref().is_some_and(|x| x.builds.contains(&u.id)) || b.upgrades_to == Some(u.id)
            });
            assert!(buildable || u.has(cat::COMMANDER), "{} cannot be built", u.key);
        }
    }

    #[test]
    fn hash_is_stable_across_loads() {
        let a = Blueprints::load(&data_dir()).unwrap();
        let b = Blueprints::load(&data_dir()).unwrap();
        assert_eq!(a.content_hash(), b.content_hash());
        assert_ne!(a.content_hash(), Blueprints::default().content_hash());
    }
}
