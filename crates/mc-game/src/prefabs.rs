//! Prefabs: layouts of structures the player saved, to put down again in one go
//! with any builder that can raise them.
//!
//! A prefab is a list of structures by blueprint key, each at a whole-metre offset
//! from the middle of the group. It is the player's own, kept beside the settings
//! (`prefabs.ron`), and nothing of it reaches the simulation: placing one sends the
//! same `Command::Build` orders as placing its structures one by one would.

use mc_core::{Fx, FxVec2};
use mc_data::{BlueprintId, Blueprints, FactionId};
use mc_sim::placement::Unfit;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Most prefabs the library holds; saving one more is refused, with a notice.
pub const MAX_PREFABS: usize = 48;
/// Most structures in one prefab; a bigger selection is refused, with a notice.
pub const MAX_MEMBERS: usize = 64;
/// Longest name, in characters.
pub const MAX_NAME: usize = 32;
/// The file is read only up to this size: a prefab library is a few kilobytes.
const MAX_FILE_BYTES: u64 = 1 << 20;
/// Farthest a member may stand from the middle, metres: a hand-edited file
/// cannot throw a structure across the map.
const MAX_OFFSET: i32 = 2048;

/// One structure of a prefab: its blueprint, and where it stands from the
/// prefab's middle, metres east (`x`) and north (`y`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Member {
    pub key: String,
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Prefab {
    pub name: String,
    pub members: Vec<Member>,
    /// Not saved: which prefab placement is about, steady while the library changes.
    #[serde(skip)]
    pub id: PrefabId,
}

/// A prefab in the library while the game runs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct PrefabId(pub u32);

/// A prefab's member as this build of the game knows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Resolved {
    pub blueprint: BlueprintId,
    pub x: i32,
    pub y: i32,
}

impl Prefab {
    /// Its members whose blueprints this build has (one retired since it was saved is
    /// left out), and how many were left out.
    pub fn resolve(&self, blueprints: &Blueprints) -> (Vec<Resolved>, usize) {
        let mut out = Vec::with_capacity(self.members.len());
        for m in &self.members {
            match blueprints.id_of(&m.key) {
                Some(blueprint) if blueprints.unit(blueprint).built_on_site() => {
                    out.push(Resolved {
                        blueprint,
                        x: m.x,
                        y: m.y,
                    });
                }
                _ => {}
            }
        }
        let gone = self.members.len() - out.len();
        (out, gone)
    }

    /// The faction most of its structures belong to: whose builders it is offered to.
    pub fn faction(&self, blueprints: &Blueprints) -> Option<FactionId> {
        let (members, _) = self.resolve(blueprints);
        let mut counts: Vec<(FactionId, usize)> = Vec::new();
        for m in &members {
            let f = blueprints.unit(m.blueprint).faction;
            match counts.iter_mut().find(|(g, _)| *g == f) {
                Some((_, n)) => *n += 1,
                None => counts.push((f, 1)),
            }
        }
        // The first faction seen wins a tie: `max_by_key` keeps the last of equals.
        counts.iter().rev().max_by_key(|(_, n)| *n).map(|(f, _)| *f)
    }

    /// What it costs to raise all of it: materials and energy.
    pub fn cost(&self, blueprints: &Blueprints) -> (f32, f32) {
        let (members, _) = self.resolve(blueprints);
        members.iter().fold((0.0, 0.0), |(m, e), r| {
            let bp = blueprints.unit(r.blueprint);
            (m + bp.cost_materials.to_f32(), e + bp.cost_energy.to_f32())
        })
    }

    /// Its structures counted by kind, most first, then in the order first saved.
    pub fn census(&self, blueprints: &Blueprints) -> Vec<(BlueprintId, usize)> {
        let (members, _) = self.resolve(blueprints);
        let mut out: Vec<(BlueprintId, usize)> = Vec::new();
        for m in &members {
            match out.iter_mut().find(|(b, _)| *b == m.blueprint) {
                Some((_, n)) => *n += 1,
                None => out.push((m.blueprint, 1)),
            }
        }
        out.sort_by_key(|&(_, n)| std::cmp::Reverse(n));
        out
    }
}

/// Turns an offset a quarter turn anticlockwise (seen from above) `quarters` times.
pub fn rotate(x: i32, y: i32, quarters: u8) -> (i32, i32) {
    match quarters % 4 {
        0 => (x, y),
        1 => (-y, x),
        2 => (-x, -y),
        _ => (y, -x),
    }
}

/// The ground a prefab's middle snaps to: build-cell corners, so every member keeps
/// its place on the grid relative to the others.
pub fn snap_anchor(at: FxVec2) -> FxVec2 {
    let cell = Fx::from_int(mc_map::BUILD_CELL_M);
    let snap = |v: Fx| Fx::from_int((v / cell).round_int()) * cell;
    FxVec2::new(snap(at.x), snap(at.y))
}

/// Where each member goes with the prefab's middle at `anchor`, turned `quarters`
/// quarter turns, snapped to the build grid as the sim will snap it.
pub fn layout(
    blueprints: &Blueprints,
    members: &[Resolved],
    anchor: FxVec2,
    quarters: u8,
) -> Vec<(BlueprintId, FxVec2)> {
    let anchor = snap_anchor(anchor);
    members
        .iter()
        .map(|m| {
            let (x, y) = rotate(m.x, m.y, quarters);
            let at = FxVec2::new(anchor.x + Fx::from_int(x), anchor.y + Fx::from_int(y));
            let bp = blueprints.unit(m.blueprint);
            (m.blueprint, mc_sim::world::snap_to_build_grid(bp, at))
        })
        .collect()
}

/// The lots a layout covers, as `(lo, hi)` corners in metres, for drawing.
pub fn bounds(
    blueprints: &Blueprints,
    sites: &[(BlueprintId, FxVec2)],
) -> Option<([f32; 2], [f32; 2])> {
    let half = |b: BlueprintId| {
        let f = blueprints.unit(b).footprint;
        let cell = mc_map::BUILD_CELL_M as f32 * 0.5;
        [f.0 as f32 * cell, f.1 as f32 * cell]
    };
    sites.iter().fold(None, |acc, &(b, at)| {
        let [x, y] = at.to_f32();
        let h = half(b);
        let (lo, hi) = ([x - h[0], y - h[1]], [x + h[0], y + h[1]]);
        Some(match acc {
            None => (lo, hi),
            Some((a, b)) => (
                [a[0].min(lo[0]), a[1].min(lo[1])],
                [b[0].max(hi[0]), b[1].max(hi[1])],
            ),
        })
    })
}

/// Why a selection cannot be saved as a prefab.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Uncaptured {
    /// Nothing in it is a structure, standing, going up or planned.
    Nothing,
    /// More structures than a prefab holds.
    TooMany(usize),
}

impl Uncaptured {
    pub fn label(self) -> String {
        match self {
            Uncaptured::Nothing => {
                "Select structures, or engineers with structures planned, to save a prefab"
                    .to_owned()
            }
            Uncaptured::TooMany(n) => {
                format!("{n} structures: a prefab holds {MAX_MEMBERS} at most")
            }
        }
    }
}

/// A prefab of these structures (`(blueprint, centre)`, standing or planned), with
/// its middle on the build-cell corner nearest the middle of their lots, and a name
/// made from what is in it. Repeats of the same structure on the same lot count once.
pub fn capture(
    blueprints: &Blueprints,
    structures: &[(BlueprintId, FxVec2)],
) -> Result<Prefab, Uncaptured> {
    let mut sites: Vec<(BlueprintId, FxVec2)> = Vec::new();
    for &(b, at) in structures {
        let bp = blueprints.unit(b);
        if !bp.built_on_site() || bp.is_mobile() {
            continue;
        }
        let at = mc_sim::world::snap_to_build_grid(bp, at);
        if !sites.contains(&(b, at)) {
            sites.push((b, at));
        }
    }
    if sites.is_empty() {
        return Err(Uncaptured::Nothing);
    }
    if sites.len() > MAX_MEMBERS {
        return Err(Uncaptured::TooMany(sites.len()));
    }
    let (lo, hi) = bounds(blueprints, &sites).expect("sites is not empty");
    let middle = FxVec2::new(
        Fx::from_f32((lo[0] + hi[0]) * 0.5),
        Fx::from_f32((lo[1] + hi[1]) * 0.5),
    );
    let anchor = snap_anchor(middle);
    // Rows from the north-west, as a page is read: the order it is built in when the
    // builders stand nowhere in particular, and the order of its census.
    sites.sort_by_key(|(_, at)| (std::cmp::Reverse(at.y), at.x));
    let members = sites
        .iter()
        .map(|&(b, at)| Member {
            key: blueprints.unit(b).key.clone(),
            x: (at.x - anchor.x).round_int(),
            y: (at.y - anchor.y).round_int(),
        })
        .collect();
    let mut prefab = Prefab {
        name: String::new(),
        members,
        id: PrefabId::default(),
    };
    prefab.name = default_name(blueprints, &prefab);
    Ok(prefab)
}

/// "Reactor + 4 Fabricator": its biggest group of structures, and the next.
fn default_name(blueprints: &Blueprints, prefab: &Prefab) -> String {
    let census = prefab.census(blueprints);
    let part = |&(b, n): &(BlueprintId, usize)| {
        let name = &blueprints.unit(b).name;
        if n > 1 {
            format!("{n} {name}")
        } else {
            name.clone()
        }
    };
    let name = match census.as_slice() {
        [] => "Prefab".to_owned(),
        [one] => part(one),
        [a, b] => format!("{} + {}", part(a), part(b)),
        [a, b, ..] => format!("{} + {} +", part(a), part(b)),
    };
    clean_name(&name)
}

/// A name as the library keeps it: printable, one line, not too long, not blank.
pub fn clean_name(name: &str) -> String {
    let name: String = name
        .chars()
        .filter(|c| c.is_ascii_graphic() || *c == ' ')
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let name: String = name.chars().take(MAX_NAME).collect();
    let name = name.trim_end().to_owned();
    if name.is_empty() {
        "Prefab".to_owned()
    } else {
        name
    }
}

/// What the file holds.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct LibraryFile {
    prefabs: Vec<Prefab>,
}

/// Why the library could not be written.
#[derive(Debug)]
pub enum SaveError {
    /// Nowhere to keep it: no settings folder on this machine.
    NoFolder,
    Io(std::io::Error),
    Ron(ron::Error),
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SaveError::NoFolder => write!(f, "no settings folder"),
            SaveError::Io(e) => write!(f, "{e}"),
            SaveError::Ron(e) => write!(f, "{e}"),
        }
    }
}

/// What came of putting a prefab in the library.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kept {
    /// A new one.
    Added(PrefabId),
    /// It took the place of the one with its name.
    Replaced(PrefabId),
}

/// Most recently used first: the prefabs the build panel shows, in its order.
#[derive(Default)]
pub struct Library {
    prefabs: Vec<Prefab>,
    next_id: u32,
    /// Where it is kept; `None`: only for this run (tests, no settings folder).
    path: Option<PathBuf>,
}

impl Library {
    /// The player's library, from beside the settings. A file that cannot be read
    /// gives an empty library and a warning in the log; it is not overwritten until
    /// the player saves a prefab.
    pub fn load() -> Library {
        let path = crate::settings::config_dir().map(|d| d.join("prefabs.ron"));
        let mut library = Library {
            path: path.clone(),
            ..Library::default()
        };
        let Some(path) = path else {
            return library;
        };
        match read_file(&path) {
            Ok(Some(prefabs)) => {
                log::info!("{} prefabs from {}", prefabs.len(), path.display());
                for p in prefabs {
                    library.push_back(p);
                }
            }
            Ok(None) => {}
            Err(e) => log::warn!("{}: {e}; no prefabs this run", path.display()),
        }
        library
    }

    /// An empty library kept nowhere, for a scene with no player's files behind it.
    pub fn none() -> &'static Library {
        static NONE: Library = Library {
            prefabs: Vec::new(),
            next_id: 0,
            path: None,
        };
        &NONE
    }

    /// The prefabs in the file at `path`, kept nowhere: a staged screenshot's library.
    pub fn read(path: &std::path::Path) -> Result<Library, String> {
        let prefabs =
            read_file(path)?.ok_or_else(|| format!("{}: no such file", path.display()))?;
        let mut library = Library::default();
        for p in prefabs {
            library.push_back(p);
        }
        Ok(library)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Prefab> {
        self.prefabs.iter()
    }

    pub fn len(&self) -> usize {
        self.prefabs.len()
    }

    pub fn get(&self, id: PrefabId) -> Option<&Prefab> {
        self.prefabs.iter().find(|p| p.id == id)
    }

    pub fn by_name(&self, name: &str) -> Option<&Prefab> {
        self.prefabs
            .iter()
            .find(|p| p.name.eq_ignore_ascii_case(name))
    }

    fn push_back(&mut self, mut p: Prefab) {
        p.name = clean_name(&p.name);
        // A hand-edited file is held to what a save would have let through.
        let why = if p.members.is_empty() {
            Some("it is empty")
        } else if p.members.len() > MAX_MEMBERS {
            Some("it has too many structures")
        } else if p
            .members
            .iter()
            .any(|m| m.x.abs() > MAX_OFFSET || m.y.abs() > MAX_OFFSET)
        {
            Some("a structure stands too far out")
        } else if self.prefabs.len() >= MAX_PREFABS {
            Some("the library is full")
        } else if self.by_name(&p.name).is_some() {
            Some("another has its name")
        } else {
            None
        };
        if let Some(why) = why {
            log::warn!("prefab {:?} skipped: {why}", p.name);
            return;
        }
        p.id = self.fresh_id();
        self.prefabs.push(p);
    }

    fn fresh_id(&mut self) -> PrefabId {
        self.next_id += 1;
        PrefabId(self.next_id)
    }

    /// Puts `prefab` first in the library, in place of any with its name. `None`
    /// when the library is full (and nothing has the name).
    pub fn keep(&mut self, mut prefab: Prefab) -> Option<Kept> {
        prefab.name = clean_name(&prefab.name);
        let old = self
            .prefabs
            .iter()
            .position(|p| p.name.eq_ignore_ascii_case(&prefab.name));
        let kept = match old {
            Some(i) => {
                prefab.id = self.prefabs.remove(i).id;
                Kept::Replaced(prefab.id)
            }
            None if self.prefabs.len() >= MAX_PREFABS => return None,
            None => {
                prefab.id = self.fresh_id();
                Kept::Added(prefab.id)
            }
        };
        self.prefabs.insert(0, prefab);
        Some(kept)
    }

    /// Takes a prefab out; returns it.
    pub fn remove(&mut self, id: PrefabId) -> Option<Prefab> {
        let i = self.prefabs.iter().position(|p| p.id == id)?;
        Some(self.prefabs.remove(i))
    }

    /// Moves a prefab to the front, as the one used last.
    pub fn touch(&mut self, id: PrefabId) -> bool {
        match self.prefabs.iter().position(|p| p.id == id) {
            Some(0) | None => false,
            Some(i) => {
                let p = self.prefabs.remove(i);
                self.prefabs.insert(0, p);
                true
            }
        }
    }

    /// Writes the library where it is kept.
    pub fn save(&self) -> Result<(), SaveError> {
        let Some(path) = &self.path else {
            return Err(SaveError::NoFolder);
        };
        let file = LibraryFile {
            prefabs: self.prefabs.clone(),
        };
        let text = ron::ser::to_string_pretty(&file, ron::ser::PrettyConfig::default())
            .map_err(SaveError::Ron)?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(SaveError::Io)?;
        }
        // Written aside and moved over, so a crash mid-write leaves the old file whole.
        let aside = path.with_extension("ron.new");
        std::fs::write(&aside, text).map_err(SaveError::Io)?;
        std::fs::rename(&aside, path).map_err(SaveError::Io)
    }
}

/// The prefabs in the file at `path`; `None` when there is no file yet.
fn read_file(path: &std::path::Path) -> Result<Option<Vec<Prefab>>, String> {
    let meta = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    if meta.len() > MAX_FILE_BYTES {
        return Err(format!(
            "{} bytes, more than a prefab library holds ({MAX_FILE_BYTES})",
            meta.len()
        ));
    }
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let file: LibraryFile = ron::from_str(&text).map_err(|e| e.to_string())?;
    Ok(Some(file.prefabs))
}

/// One structure of the prefab in hand, where it would go.
#[derive(Clone, Copy, Debug)]
pub struct PrefabSite {
    pub blueprint: BlueprintId,
    pub at: FxVec2,
    /// Whether the ground and the lot will take it.
    pub fit: Result<(), Unfit>,
    /// Whether a selected builder can raise it.
    pub buildable: bool,
}

impl PrefabSite {
    pub fn goes_down(&self) -> bool {
        self.fit.is_ok() && self.buildable
    }
}

/// Why structures of the prefab in hand will not go down: the commonest reason with
/// how many it holds back ("Lot taken (2)"), then how many more for other reasons.
pub fn left_out_reason(sites: &[PrefabSite]) -> String {
    let unbuildable = sites.iter().filter(|s| !s.buildable).count();
    let mut reasons: Vec<(&'static str, usize)> = Vec::new();
    for s in sites.iter().filter(|s| s.buildable) {
        if let Err(why) = s.fit {
            match reasons.iter_mut().find(|(l, _)| *l == why.label()) {
                Some((_, n)) => *n += 1,
                None => reasons.push((why.label(), 1)),
            }
        }
    }
    if unbuildable > 0 {
        reasons.push(("Not built by this selection", unbuildable));
    }
    reasons.sort_by_key(|&(_, n)| std::cmp::Reverse(n));
    // The commonest, counted; the others only counted.
    let mut out = String::new();
    for (i, (label, n)) in reasons.iter().enumerate() {
        match i {
            0 if *n > 1 => out.push_str(&format!("{label} ({n})")),
            0 => out.push_str(label),
            _ => {
                let rest: usize = reasons[1..].iter().map(|(_, n)| n).sum();
                out.push_str(&format!(", {rest} more"));
                break;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests;
