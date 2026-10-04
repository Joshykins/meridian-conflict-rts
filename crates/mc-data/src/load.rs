//! Reading `data/` into blueprints: every unit, or (a release build) every unit
//! but the playtest-only ones (`crate::playtest`).

use crate::{parse_file, raw, refit, sorted_entries, Blueprints, DataError};
use mc_core::Channel;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Each faction as its files write it: the faction, its units and their lore.
pub(crate) type Sources = Vec<(
    raw::Faction,
    Vec<raw::Unit>,
    BTreeMap<String, raw::RawUnitLore>,
)>;

impl Blueprints {
    /// Loads every faction under `<data_dir>/factions`, playtest-only units too. Factions
    /// and units get ids in sorted key order, so ids do not depend on directory listing
    /// order.
    pub fn load(data_dir: &Path) -> Result<Blueprints, DataError> {
        Self::compile(Self::read_sources(data_dir)?)
    }

    /// Loads what a build of `channel` has: everything, or, where the channel has no
    /// playtest content, everything but the units marked `playtest: true` and every
    /// mention of them (`crate::playtest::strip`).
    pub fn load_for(data_dir: &Path, channel: Channel) -> Result<Blueprints, DataError> {
        let mut sources = Self::read_sources(data_dir)?;
        if !channel.has_playtest_content() {
            crate::playtest::strip(&mut sources)?;
        }
        Self::compile(sources)
    }

    /// Every faction's files under `<data_dir>/factions`, checked against each other
    /// but not yet compiled.
    pub(crate) fn read_sources(data_dir: &Path) -> Result<Sources, DataError> {
        let factions_dir = data_dir.join("factions");
        let mut sources = Vec::new();
        for dir in sorted_entries(&factions_dir)? {
            if !dir.is_dir() {
                continue;
            }
            let faction_path = dir.join("faction.ron");
            let faction: raw::Faction = parse_file(&faction_path)?;
            let mut units = Vec::new();
            // A faction on a stand-in roster has no units folder yet.
            let unit_dir = dir.join("units");
            let unit_files = if unit_dir.is_dir() {
                sorted_entries(&unit_dir)?
            } else {
                Vec::new()
            };
            for file in unit_files {
                if file.extension().is_some_and(|e| e == "ron") {
                    let list: Vec<raw::Unit> = parse_file(&file)?;
                    units.extend(list);
                }
            }
            let lore_path = dir.join("lore.ron");
            let lore: BTreeMap<String, raw::RawUnitLore> = if lore_path.is_file() {
                // A map of bare strings is the older shape: the units' text alone.
                parse_file(&lore_path).or_else(|e| {
                    parse_file::<BTreeMap<String, String>>(&lore_path)
                        .map(|flat| {
                            flat.into_iter()
                                .map(|(k, lore)| {
                                    (
                                        k,
                                        raw::RawUnitLore {
                                            lore,
                                            ..Default::default()
                                        },
                                    )
                                })
                                .collect()
                        })
                        .map_err(|_| e)
                })?
            } else {
                BTreeMap::new()
            };
            // Only this faction's units and their own weapons: a name that matches nothing is a typo.
            let mut texts = BTreeMap::new();
            for (key, entry) in lore {
                let Some(unit) = units.iter().find(|u| u.key == key) else {
                    return Err(DataError::Invalid(format!(
                        "{}: unknown unit key {key}",
                        lore_path.display()
                    )));
                };
                if let Some(name) = entry
                    .weapons
                    .keys()
                    .find(|n| !refit::weapon_names(unit).any(|w| w == n.as_str()))
                {
                    return Err(DataError::Invalid(format!(
                        "{}: {key} has no weapon {name}",
                        lore_path.display()
                    )));
                }
                texts.insert(key, entry);
            }
            sources.push((faction, units, texts));
        }
        Ok(sources)
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
        roots
            .into_iter()
            .map(|r| r.join("data"))
            .find(|d| d.join("factions").is_dir())
    }
}
