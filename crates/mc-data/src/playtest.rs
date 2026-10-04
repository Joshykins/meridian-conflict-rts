//! Playtest-only units: a unit file marks one `playtest: true`, and a release build
//! (`mc_core::Channel::has_playtest_content`) does not have it at all.
//!
//! [`strip`] takes such units out of the authored sources before they are compiled,
//! so ids stay dense and nothing can point at a unit that is not there. Every other
//! unit's mention of one goes with it:
//!
//! - a builder's or refit module's build list drops it; a list that held only
//!   playtest-only units is an error (that builder must be marked too), not an
//!   empty factory,
//! - an upgrade into it is dropped: the unit stops at its own tier,
//! - a unit that cannot work without it (a faction's commander, a drone, a gun's
//!   sabot casing) is an error: mark that unit playtest-only too.
//!
//! `load_for_release_succeeds` (below) runs this over the checked-in data, so a
//! flag that would break a release build fails `scripts/check.sh` first.

use crate::load::Sources;
use crate::raw::{RawWeapon, Unit};
use crate::DataError;
use std::collections::BTreeSet;

/// Removes every playtest-only unit, its lore and every mention of it (module docs).
pub(crate) fn strip(sources: &mut Sources) -> Result<(), DataError> {
    let gone: BTreeSet<String> = sources
        .iter()
        .flat_map(|(_, units, _)| units.iter().filter(|u| u.playtest))
        .map(|u| u.key.clone())
        .collect();
    if gone.is_empty() {
        return Ok(());
    }
    for (faction, units, lore) in sources.iter_mut() {
        units.retain(|u| !u.playtest);
        lore.retain(|key, _| !gone.contains(key));
        needed(&gone, &faction.key, &faction.commander)?;
        for unit in units.iter_mut() {
            strip_unit(unit, &gone)?;
        }
    }
    Ok(())
}

fn strip_unit(unit: &mut Unit, gone: &BTreeSet<String>) -> Result<(), DataError> {
    let key = unit.key.as_str();
    if let Some(b) = &mut unit.builder {
        drop_from(&mut b.builds, gone, key)?;
    }
    if unit.upgrades_to.as_ref().is_some_and(|k| gone.contains(k)) {
        unit.upgrades_to = None;
    }
    if let Some(drone) = &unit.drone {
        needed(gone, key, drone)?;
    }
    casings(&unit.weapons, gone, key)?;
    for slot in &mut unit.refits {
        for module in &mut slot.modules {
            let at = format!("{key}/{}", module.key);
            drop_from(&mut module.builds, gone, &at)?;
            if let Some(drone) = &module.drone {
                needed(gone, &at, drone)?;
            }
            casings(&module.weapons, gone, &at)?;
        }
    }
    Ok(())
}

fn casings(weapons: &[RawWeapon], gone: &BTreeSet<String>, at: &str) -> Result<(), DataError> {
    for sabot in weapons.iter().filter_map(|w| w.sabot.as_ref()) {
        needed(gone, at, &sabot.casing)?;
    }
    Ok(())
}

/// Takes the playtest-only units out of a build list.
fn drop_from(builds: &mut Vec<String>, gone: &BTreeSet<String>, at: &str) -> Result<(), DataError> {
    let had = !builds.is_empty();
    builds.retain(|k| !gone.contains(k));
    if had && builds.is_empty() {
        return Err(DataError::Invalid(format!(
            "{at} builds only playtest-only units: mark it `playtest: true` too, or give it something a release build has"
        )));
    }
    Ok(())
}

/// `at` cannot work without `key`: it must not be playtest-only.
fn needed(gone: &BTreeSet<String>, at: &str, key: &str) -> Result<(), DataError> {
    if gone.contains(key) {
        return Err(DataError::Invalid(format!(
            "{at} needs {key}, which is playtest-only: mark {at} `playtest: true` too"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BlueprintId, Blueprints};
    use mc_core::Channel;
    use std::path::{Path, PathBuf};

    fn data_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")
    }

    /// The checked-in data, with `keys` marked playtest-only.
    fn marked(keys: &[&str]) -> Sources {
        let mut sources = Blueprints::read_sources(&data_dir()).unwrap();
        for unit in sources
            .iter_mut()
            .flat_map(|(_, units, _)| units.iter_mut())
        {
            if keys.contains(&unit.key.as_str()) {
                unit.playtest = true;
            }
        }
        sources
    }

    /// Every id a blueprint holds, with what holds it.
    fn mentions(b: &Blueprints) -> Vec<(String, BlueprintId)> {
        let mut out = Vec::new();
        for u in &b.units {
            let ids = u
                .builder
                .iter()
                .flat_map(|b| b.builds.iter().copied())
                .chain(u.upgrades_to)
                .chain(u.drone)
                .chain(u.weapons.iter().filter_map(|w| w.sabot.map(|s| s.casing)));
            out.extend(ids.map(|id| (u.key.clone(), id)));
        }
        out.extend(b.factions.iter().map(|f| (f.key.clone(), f.commander)));
        out
    }

    /// A tank three factories build, and a sonar an engineer builds, a module
    /// lists and the tier below upgrades into.
    const MARKED: [&str; 2] = ["aster_t1_tank", "aster_t3_sonar"];

    #[test]
    fn a_release_set_has_no_playtest_unit_and_no_dangling_mention() {
        let full = Blueprints::compile(marked(&MARKED)).unwrap();
        for key in MARKED {
            assert!(
                full.id_of(key).is_some(),
                "{key} is not in the data any more"
            );
        }
        let mut sources = marked(&MARKED);
        strip(&mut sources).unwrap();
        let release = Blueprints::compile(sources).unwrap();
        for key in MARKED {
            assert!(release.id_of(key).is_none(), "{key} is still there");
        }
        for (holder, id) in mentions(&release) {
            let Some(unit) = release.units.get(id.index()) else {
                panic!("{holder} names id {}, past the end", id.0);
            };
            assert!(
                !MARKED.contains(&unit.key.as_str()),
                "{holder} names {}",
                unit.key
            );
        }
        let t2 = release.unit_by_key("aster_t2_sonar").unwrap();
        assert_eq!(
            t2.upgrades_to, None,
            "the upgrade into the playtest sonar goes"
        );
        assert!(release.units.len() < full.units.len());
        assert_ne!(release.content_hash(), full.content_hash());
    }

    #[test]
    fn a_unit_that_needs_a_playtest_unit_is_refused() {
        // The commander's drone port module makes this drone.
        let err = strip(&mut marked(&["aster_reclaim_drone"])).unwrap_err();
        assert!(
            err.to_string().contains("needs aster_reclaim_drone"),
            "{err}"
        );
    }

    #[test]
    fn a_builder_left_with_nothing_to_build_is_refused() {
        let sources = Blueprints::read_sources(&data_dir()).unwrap();
        let factory = sources
            .iter()
            .flat_map(|(_, units, _)| units)
            .find(|u| u.key == "aster_t1_land_factory")
            .unwrap();
        let builds = factory.builder.as_ref().unwrap().builds.clone();
        let keys: Vec<&str> = builds.iter().map(String::as_str).collect();
        let err = strip(&mut marked(&keys)).unwrap_err();
        assert!(
            err.to_string().contains("builds only playtest-only units"),
            "{err}"
        );
    }

    /// The guard on the checked-in data: whatever is flagged, a release build of it loads.
    #[test]
    fn load_for_release_succeeds() {
        let release = Blueprints::load_for(&data_dir(), Channel::Release).unwrap();
        let dev = Blueprints::load_for(&data_dir(), Channel::Dev).unwrap();
        let playtest = Blueprints::load_for(&data_dir(), Channel::Playtest).unwrap();
        let all = Blueprints::load(&data_dir()).unwrap();
        assert_eq!(dev.content_hash(), all.content_hash());
        assert_eq!(playtest.content_hash(), all.content_hash());
        assert!(release.units.len() <= all.units.len());
    }
}
