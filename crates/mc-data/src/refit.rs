//! Refits: parts fitted onto a unit where it stands, one slot at a time.
//!
//! A unit file lists `refits`: slots (an arm, the back, a shoulder), each with the
//! modules that can go in it. A slot holds one module at a time. A module that
//! names another as `after` is that module's next tier and takes its place; every
//! other module in the slot is an alternative, and fitting it takes out what is
//! there.
//!
//! Every loadout compiles to a blueprint of its own, so the sim, the renderer and
//! the interface see an ordinary unit with the weapons, builder, economy and
//! shield its parts give it. A refit swaps the unit's blueprint in place, like an
//! upgrade. Each module also compiles to a kit: a hidden blueprint with the
//! module's price and build time, which is what the refit assembles.

use crate::raw::{Cost, RawEconomy, RawShield, RawWeapon, Unit};
use crate::{BlueprintId, Blueprints, DataError, FactionId, UnitBlueprint};
use mc_core::Fx;
use serde::Deserialize;
use std::collections::BTreeMap;

/// Most slots one unit has. A loadout is a byte per slot.
pub const MAX_REFIT_SLOTS: usize = 6;
/// Most modules one unit has across its slots: each is a bit of the model's look.
pub const MAX_MODULES: usize = 30;
/// Most loadouts one unit may compile to (the product of each slot's choices).
pub const MAX_LOADOUTS: usize = 1024;

#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct RawRefitSlot {
    pub key: String,
    /// Where on the unit it is: "Right Arm", "Back".
    pub name: String,
    pub modules: Vec<RawModule>,
}

/// What one module adds to the unit it is fitted to.
#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct RawModule {
    pub key: String,
    pub name: String,
    /// One line for the interface: what it is for.
    #[serde(default)]
    pub summary: String,
    /// Its picture in the interface: a strategic icon shape.
    pub icon: crate::IconKind,
    /// The module in the same slot this is the next tier of. It can only go on over that one.
    #[serde(default)]
    pub after: Option<String>,
    /// The unit counts as this tech while the module is fitted. Zero: no change.
    #[serde(default)]
    pub tech: u8,
    pub cost: Cost,
    #[serde(default)]
    pub health: f64,
    #[serde(default)]
    pub regen: f64,
    #[serde(default)]
    pub vision: f64,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub economy: RawEconomy,
    #[serde(default)]
    pub build_power: f64,
    #[serde(default)]
    pub build_range: f64,
    /// Added to what the unit can build.
    #[serde(default)]
    pub builds: Vec<String>,
    /// More construction beams, from these points on the turret.
    #[serde(default)]
    pub emitters: Vec<(f64, f64, f64)>,
    /// Seconds the gear carrying `emitters` takes to unfold.
    #[serde(default)]
    pub unfold: f64,
    /// Where the gear carrying `emitters` pitches to point at the work.
    #[serde(default)]
    pub hinge: Option<(f64, f64, f64)>,
    /// Moves where the build arm's beam leaves (a longer projector on the arm).
    #[serde(default)]
    pub arm_emitter: Option<(f64, f64, f64)>,
    #[serde(default)]
    pub shield: Option<RawShield>,
    #[serde(default)]
    pub weapons: Vec<RawWeapon>,
    /// Weapons the unit already has that move when this module goes on (a gun that
    /// makes room for a bigger one): new muzzle, and pivot when given.
    #[serde(default)]
    pub remount: Vec<RawRemount>,
}

#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct RawRemount {
    pub weapon: String,
    pub muzzle: (f64, f64, f64),
    #[serde(default)]
    pub pivot: Option<(f64, f64, f64)>,
}

/// A unit with refit slots: its slots, their modules, and the blueprint of every loadout.
#[derive(Clone, Debug)]
pub struct RefitSet {
    /// The unit as it is built, with nothing fitted.
    pub base: BlueprintId,
    pub slots: Vec<RefitSlot>,
    /// Every loadout's blueprint, by `RefitSet::index`. The first is `base`.
    pub loadouts: Vec<BlueprintId>,
}

#[derive(Clone, Debug)]
pub struct RefitSlot {
    pub key: String,
    pub name: String,
    pub modules: Vec<Module>,
}

#[derive(Clone, Debug)]
pub struct Module {
    pub key: String,
    pub name: String,
    pub summary: String,
    pub icon: crate::IconKind,
    /// Index in the slot of the module this one is the next tier of.
    pub after: Option<u8>,
    pub tech: u8,
    pub cost_mass: Fx,
    pub cost_energy: Fx,
    pub build_time: Fx,
    /// The hidden blueprint a refit to this module assembles: its price and time.
    pub kit: BlueprintId,
    /// Its bit in the model's look. The mesh tags this module's pieces with its key.
    pub bit: u8,
}

/// What a unit of a refit set has fitted: per slot, zero for nothing or the module index plus one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Loadout {
    pub set: u16,
    pub fitted: [u8; MAX_REFIT_SLOTS],
}

impl Loadout {
    /// The module in `slot`, if any.
    #[inline]
    pub fn module(&self, slot: usize) -> Option<u8> {
        self.fitted[slot].checked_sub(1)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refit {
    /// A unit of the set, with what it has fitted.
    Loadout(Loadout),
    /// The hidden blueprint a refit assembles: `module` in `slot`.
    Kit { set: u16, slot: u8, module: u8 },
}

/// Why a module cannot go on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FitError {
    /// The kit is for another kind of unit.
    NotThisUnit,
    /// The module, or a later tier of it, is already on.
    Fitted,
    /// The module is the next tier of the one at this index in its slot, which is not on.
    Needs(u8),
}

impl RefitSet {
    /// Where a loadout's blueprint is in `loadouts`.
    pub fn index(&self, fitted: &[u8; MAX_REFIT_SLOTS]) -> usize {
        let mut index = 0;
        let mut radix = 1;
        for (s, slot) in self.slots.iter().enumerate() {
            index += fitted[s] as usize * radix;
            radix *= slot.modules.len() + 1;
        }
        index
    }

    fn fitted_at(&self, mut index: usize) -> [u8; MAX_REFIT_SLOTS] {
        let mut fitted = [0; MAX_REFIT_SLOTS];
        for (s, slot) in self.slots.iter().enumerate() {
            let n = slot.modules.len() + 1;
            fitted[s] = (index % n) as u8;
            index /= n;
        }
        fitted
    }

    /// `module` in `slot`, and each earlier tier it is fitted over, newest first.
    pub fn chain(&self, slot: usize, module: u8) -> impl Iterator<Item = u8> + '_ {
        std::iter::successors(Some(module), move |&m| self.slots[slot].modules[m as usize].after)
    }

    /// Every module the pieces of which are on show: those fitted, and the tiers under them.
    pub fn look(&self, fitted: &[u8; MAX_REFIT_SLOTS]) -> u32 {
        let mut bits = 0;
        for (s, slot) in self.slots.iter().enumerate() {
            if let Some(m) = fitted[s].checked_sub(1) {
                for m in self.chain(s, m) {
                    bits |= 1 << slot.modules[m as usize].bit;
                }
            }
        }
        bits
    }

    /// `fitted` with `module` put in `slot`.
    pub fn fit(
        &self,
        fitted: &[u8; MAX_REFIT_SLOTS],
        slot: usize,
        module: u8,
    ) -> Result<[u8; MAX_REFIT_SLOTS], FitError> {
        if let Some(on) = fitted[slot].checked_sub(1) {
            if self.chain(slot, on).any(|m| m == module) {
                return Err(FitError::Fitted);
            }
        }
        if let Some(before) = self.slots[slot].modules[module as usize].after {
            if fitted[slot] != before + 1 {
                return Err(FitError::Needs(before));
            }
        }
        let mut next = *fitted;
        next[slot] = module + 1;
        Ok(next)
    }

    /// What fitting `module` in `slot` would take out, if it is not the tier `module` goes over.
    pub fn replaces(&self, fitted: &[u8; MAX_REFIT_SLOTS], slot: usize, module: u8) -> Option<u8> {
        let on = fitted[slot].checked_sub(1)?;
        (self.slots[slot].modules[module as usize].after != Some(on)).then_some(on)
    }

    /// The module whose kit is `kit`, as (slot, module).
    pub fn find_kit(&self, kit: BlueprintId) -> Option<(usize, u8)> {
        self.slots.iter().enumerate().find_map(|(s, slot)| {
            slot.modules
                .iter()
                .position(|m| m.kit == kit)
                .map(|m| (s, m as u8))
        })
    }

    pub fn module(&self, slot: usize, module: u8) -> &Module {
        &self.slots[slot].modules[module as usize]
    }
}

impl Blueprints {
    /// The refit set `id` belongs to (as a loadout or a kit).
    pub fn refit_set(&self, id: BlueprintId) -> Option<&RefitSet> {
        match self.units.get(id.index())?.refit? {
            Refit::Loadout(l) => self.refits.get(l.set as usize),
            Refit::Kit { set, .. } => self.refits.get(set as usize),
        }
    }

    /// What a unit of blueprint `id` has fitted.
    pub fn loadout(&self, id: BlueprintId) -> Option<(&RefitSet, Loadout)> {
        match self.units.get(id.index())?.refit? {
            Refit::Loadout(l) => Some((self.refits.get(l.set as usize)?, l)),
            Refit::Kit { .. } => None,
        }
    }

    /// The set, slot and module a kit blueprint assembles.
    pub fn kit(&self, id: BlueprintId) -> Option<(&RefitSet, usize, u8)> {
        match self.units.get(id.index())?.refit? {
            Refit::Kit { set, slot, module } => {
                Some((self.refits.get(set as usize)?, slot as usize, module))
            }
            Refit::Loadout(_) => None,
        }
    }

    /// The blueprint a unit of `from` becomes once `kit` is fitted.
    pub fn refit_result(&self, from: BlueprintId, kit: BlueprintId) -> Result<BlueprintId, FitError> {
        let (set, loadout) = self.loadout(from).ok_or(FitError::NotThisUnit)?;
        let (kit_set, slot, module) = self.kit(kit).ok_or(FitError::NotThisUnit)?;
        if kit_set.base != set.base {
            return Err(FitError::NotThisUnit);
        }
        let next = set.fit(&loadout.fitted, slot, module)?;
        Ok(set.loadouts[set.index(&next)])
    }

    /// The modules on show on a unit of blueprint `id`, as the model's look bits. Zero for most units.
    pub fn look(&self, id: BlueprintId) -> u32 {
        self.loadout(id).map_or(0, |(set, l)| set.look(&l.fitted))
    }

    /// The blueprint a loadout or kit belongs to: its unit with nothing fitted. Itself for any other.
    pub fn base_of(&self, id: BlueprintId) -> BlueprintId {
        self.refit_set(id).map_or(id, |set| set.base)
    }

    /// Whether `id` is a blueprint of its own in lists and menus: not a loadout
    /// with something fitted, and not a kit.
    pub fn is_listed(&self, id: BlueprintId) -> bool {
        self.base_of(id) == id
    }
}

/// Adds each refittable unit's kits and loadouts to `units`, and returns the sets.
/// `units` holds the authored units, in `authored` order; `lookup` finds authored keys.
pub(crate) fn expand(
    authored: &[(u8, Unit)],
    units: &mut Vec<UnitBlueprint>,
    by_key: &mut BTreeMap<String, BlueprintId>,
    lookup: &dyn Fn(&str, &str) -> Result<BlueprintId, DataError>,
) -> Result<Vec<RefitSet>, DataError> {
    let mut sets = Vec::new();
    for (i, (faction, unit)) in authored.iter().enumerate() {
        if unit.refits.is_empty() {
            continue;
        }
        let base = BlueprintId(i as u16);
        let set_index = sets.len() as u16;
        let key = &unit.key;
        check(unit)?;
        let mut push = |raw: &Unit, refit: Refit, units: &mut Vec<UnitBlueprint>| {
            if units.len() >= u16::MAX as usize {
                return Err(DataError::Invalid("too many unit blueprints".into()));
            }
            let id = BlueprintId(units.len() as u16);
            let mut bp = raw.compile(id, FactionId(*faction), lookup)?;
            bp.refit = Some(refit);
            if by_key.insert(raw.key.clone(), id).is_some() {
                return Err(DataError::Invalid(format!("duplicate unit key {}", raw.key)));
            }
            units.push(bp);
            Ok(id)
        };

        let mut slots = Vec::with_capacity(unit.refits.len());
        let mut bit = 0u8;
        for (s, raw_slot) in unit.refits.iter().enumerate() {
            let mut modules: Vec<Module> = Vec::with_capacity(raw_slot.modules.len());
            for (m, raw) in raw_slot.modules.iter().enumerate() {
                let after = match &raw.after {
                    Some(k) => Some(
                        raw_slot.modules[..m]
                            .iter()
                            .position(|o| &o.key == k)
                            .ok_or_else(|| {
                                DataError::Invalid(format!(
                                    "{key}/{}: `after` must name an earlier module of slot {}, not {k}",
                                    raw.key, raw_slot.key
                                ))
                            })? as u8,
                    ),
                    None => None,
                };
                let kit = push(
                    &kit_unit(unit, raw),
                    Refit::Kit { set: set_index, slot: s as u8, module: m as u8 },
                    units,
                )?;
                modules.push(Module {
                    key: raw.key.clone(),
                    name: raw.name.clone(),
                    summary: raw.summary.clone(),
                    icon: raw.icon,
                    after,
                    tech: raw.tech,
                    cost_mass: units[kit.index()].cost_mass,
                    cost_energy: units[kit.index()].cost_energy,
                    build_time: units[kit.index()].build_time,
                    kit,
                    bit,
                });
                bit += 1;
            }
            slots.push(RefitSlot {
                key: raw_slot.key.clone(),
                name: raw_slot.name.clone(),
                modules,
            });
        }
        let mut set = RefitSet {
            base,
            slots,
            loadouts: Vec::new(),
        };
        let count: usize = set.slots.iter().map(|s| s.modules.len() + 1).product();
        if count > MAX_LOADOUTS {
            return Err(DataError::Invalid(format!(
                "{key}: {count} loadouts, more than {MAX_LOADOUTS}"
            )));
        }
        for index in 0..count {
            let fitted = set.fitted_at(index);
            let loadout = Refit::Loadout(Loadout { set: set_index, fitted });
            if index == 0 {
                // Nothing fitted: the unit as the file has it.
                units[base.index()].refit = Some(loadout);
                set.loadouts.push(base);
                continue;
            }
            let raw = loadout_unit(unit, &fitted);
            set.loadouts.push(push(&raw, loadout, units)?);
        }
        sets.push(set);
    }
    Ok(sets)
}

fn check(unit: &Unit) -> Result<(), DataError> {
    let key = &unit.key;
    if unit.upgrades_to.is_some() {
        return Err(DataError::Invalid(format!(
            "{key}: a unit is either refitted or upgraded, not both"
        )));
    }
    if unit.refits.len() > MAX_REFIT_SLOTS {
        return Err(DataError::Invalid(format!(
            "{key}: more than {MAX_REFIT_SLOTS} refit slots"
        )));
    }
    let modules: Vec<&RawModule> = unit.refits.iter().flat_map(|s| &s.modules).collect();
    if modules.len() > MAX_MODULES {
        return Err(DataError::Invalid(format!(
            "{key}: more than {MAX_MODULES} refit modules"
        )));
    }
    for (i, m) in modules.iter().enumerate() {
        if m.key.is_empty() || m.key.contains(['+', '#']) {
            return Err(DataError::Invalid(format!(
                "{key}: module key {:?} must be a plain word",
                m.key
            )));
        }
        if modules[..i].iter().any(|o| o.key == m.key) {
            return Err(DataError::Invalid(format!("{key}: two modules called {}", m.key)));
        }
        if let Some(r) = m.remount.iter().find(|r| !unit.weapons.iter().any(|w| w.name == r.weapon)) {
            return Err(DataError::Invalid(format!(
                "{key}/{}: remounts {}, which the unit does not carry",
                m.key, r.weapon
            )));
        }
        let builds = m.build_power != 0.0
            || m.build_range != 0.0
            || !m.builds.is_empty()
            || !m.emitters.is_empty()
            || m.arm_emitter.is_some();
        if builds && unit.builder.is_none() {
            return Err(DataError::Invalid(format!(
                "{key}/{}: only a builder takes construction modules",
                m.key
            )));
        }
    }
    for s in &unit.refits {
        if s.modules.is_empty() {
            return Err(DataError::Invalid(format!("{key}: slot {} is empty", s.key)));
        }
    }
    Ok(())
}

/// The hidden blueprint a refit to `module` assembles: the unit's frame, the module's price.
fn kit_unit(unit: &Unit, module: &RawModule) -> Unit {
    let mut kit = unit.clone();
    kit.key = format!("{}#{}", unit.key, module.key);
    kit.name = module.name.clone();
    kit.cost = module.cost.clone();
    kit.categories.retain(|c| c != "Commander");
    kit.economy = RawEconomy::default();
    kit.builder = None;
    kit.shield = None;
    kit.weapons.clear();
    kit.refits.clear();
    kit
}

/// The unit with `fitted` on it: its own stats plus every fitted module's.
fn loadout_unit(unit: &Unit, fitted: &[u8; MAX_REFIT_SLOTS]) -> Unit {
    let mut out = unit.clone();
    out.refits.clear();
    let mut keys = vec![unit.key.clone()];
    for (s, slot) in unit.refits.iter().enumerate() {
        let Some(m) = fitted[s].checked_sub(1) else {
            continue;
        };
        let m = &slot.modules[m as usize];
        keys.push(m.key.clone());
        out.tech = out.tech.max(m.tech);
        out.health += m.health;
        out.regen += m.regen;
        out.vision += m.vision;
        for c in &m.categories {
            if !out.categories.contains(c) {
                out.categories.push(c.clone());
            }
        }
        let e = &mut out.economy;
        e.mass_income += m.economy.mass_income;
        e.energy_income += m.economy.energy_income;
        e.energy_upkeep += m.economy.energy_upkeep;
        e.mass_storage += m.economy.mass_storage;
        e.energy_storage += m.economy.energy_storage;
        if let Some(b) = &mut out.builder {
            b.power += m.build_power;
            b.range += m.build_range;
            for k in &m.builds {
                if !b.builds.contains(k) {
                    b.builds.push(k.clone());
                }
            }
            b.emitters.extend(m.emitters.iter().copied());
            b.unfold = b.unfold.max(m.unfold);
            b.hinge = b.hinge.or(m.hinge);
            if let (Some(e), Some(arm)) = (m.arm_emitter, b.arm.as_mut()) {
                arm.emitter = e;
            }
        }
        for r in &m.remount {
            for w in out.weapons.iter_mut().filter(|w| w.name == r.weapon) {
                w.muzzle = r.muzzle;
                if r.pivot.is_some() {
                    w.pivot = r.pivot;
                }
            }
        }
        if m.shield.is_some() {
            out.shield = m.shield.clone();
        }
        out.weapons.extend(m.weapons.iter().cloned());
    }
    out.key = keys.join("+");
    out
}

/// Every weapon name a unit's file gives it, in its own list or in its modules'.
pub(crate) fn weapon_names(unit: &Unit) -> impl Iterator<Item = &str> {
    unit.weapons.iter().map(|w| w.name.as_str()).chain(
        unit.refits
            .iter()
            .flat_map(|s| &s.modules)
            .flat_map(|m| &m.weapons)
            .map(|w| w.name.as_str()),
    )
}
