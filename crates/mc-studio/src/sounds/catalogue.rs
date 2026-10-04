//! Every sound the game makes, for the Sounds screen: the library's recipes
//! (`data/sounds/*.ron`, `data/factions/*/sounds.ron`) and the interface set,
//! each with the words written above it in its file and what in the game uses
//! it (from the unit files, the library's defaults, and the game's own code).

use mc_data::sounds::Sound;
use mc_data::{Blueprints, SoundLibrary};
use mc_sfx::Sfx;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Something that plays a sound.
#[derive(Clone, Debug, PartialEq)]
pub struct User {
    /// Who: a unit's name, "ARC builders", "The game".
    pub who: String,
    /// When: "fires Tank Cannon", "dies".
    pub when: String,
    /// For grouping: a faction's short name, or "".
    pub side: String,
    /// It plays because nothing else was named (the library's defaults).
    pub fallback: bool,
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub name: String,
    /// Heading in the list: "ARC", "Battle: guns", "Interface".
    pub group: String,
    /// Where it is written, relative to `data`, or "" for the interface set.
    pub file: String,
    /// The comment written above it.
    pub about: String,
    /// The comment above each of its layers, by layer ("" where there is none).
    pub layer_notes: Vec<String>,
    /// The recipe; `None` for the interface set, which is written in code.
    pub sound: Option<Sound>,
    pub sfx: Option<Sfx>,
    pub users: Vec<User>,
    /// Sounds written `like` this one.
    pub bases_of: Vec<String>,
    /// Where it comes in the files, read in the order the list shows them.
    order: usize,
}

#[derive(Default)]
pub struct Catalogue {
    pub entries: Vec<Entry>,
}

impl Catalogue {
    pub fn get(&self, name: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.name == name)
    }
}

/// The files whose change means the catalogue must be read again.
pub fn watched(data: &Path) -> Vec<(PathBuf, Option<SystemTime>)> {
    let mut files = Vec::new();
    collect(&data.join("sounds"), &mut files);
    collect(&data.join("factions"), &mut files);
    files.sort();
    files
        .into_iter()
        .map(|p| {
            let t = crate::files::mtime(&p);
            (p, t)
        })
        .collect()
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect(&p, out);
        } else if p.extension().is_some_and(|x| x == "ron") {
            out.push(p);
        }
    }
}

/// Reads the library, the unit files and the game's code into a catalogue.
pub fn load(data: &Path) -> Result<Catalogue, String> {
    let library = SoundLibrary::load(data).map_err(|e| e.to_string())?;
    let blueprints = Blueprints::load(data).map_err(|e| e.to_string())?;
    let mut users: BTreeMap<String, Vec<User>> = BTreeMap::new();
    unit_users(&library, &blueprints, &mut users);
    code_users(data, &library, &mut users);
    let notes = notes(data);

    let mut entries = Vec::new();
    for s in &library.sounds {
        let Note {
            file: path,
            section,
            about,
            layers: layer_notes,
            order,
        } = notes.get(&s.name).cloned().unwrap_or_default();
        let place = match blueprints
            .factions
            .iter()
            .find(|f| f.key.eq_ignore_ascii_case(&s.file))
        {
            Some(f) => f.abbreviation.clone(),
            None => crate::workbench::pretty(&s.file),
        };
        let group = if section.is_empty() {
            place
        } else {
            format!("{place}: {section}")
        };
        let bases_of = library
            .sounds
            .iter()
            .filter(|o| o.like.as_ref().is_some_and(|(b, _)| *b == s.name))
            .map(|o| o.name.clone())
            .collect();
        let layer_notes = if layer_notes.len() == s.layers.len() {
            layer_notes
        } else {
            vec![String::new(); s.layers.len()]
        };
        entries.push(Entry {
            name: s.name.clone(),
            group,
            file: path,
            about,
            layer_notes,
            sound: Some(s.clone()),
            sfx: None,
            users: users.remove(&s.name).unwrap_or_default(),
            bases_of,
            order,
        });
    }
    for sfx in Sfx::ALL {
        entries.push(Entry {
            name: sfx.name().to_owned(),
            group: "Interface".into(),
            file: String::new(),
            about: interface_about(sfx).into(),
            layer_notes: Vec::new(),
            sound: None,
            sfx: Some(sfx),
            users: vec![User {
                who: "Menus and the HUD".into(),
                when: interface_about(sfx).to_lowercase(),
                side: String::new(),
                fallback: false,
            }],
            bases_of: Vec::new(),
            order: usize::MAX,
        });
    }
    // As the files have them: the factions' first, then the general files,
    // each in its own order under its headings; the interface set last.
    entries.sort_by_key(|e| (e.order, e.name.clone()));
    Ok(Catalogue { entries })
}

fn interface_about(sfx: Sfx) -> &'static str {
    match sfx {
        Sfx::Hover => "The pointer moving onto a control",
        Sfx::Select => "A control clicked, going into a screen",
        Sfx::Back => "Leaving a screen, cancelling",
        Sfx::ToggleOn => "A switch turned on",
        Sfx::ToggleOff => "A switch turned off",
        Sfx::Tick => "One step of a slider",
        Sfx::Deny => "Something that can't be done",
        Sfx::Launch => "The match starting",
        Sfx::Order => "Units taking an order",
        Sfx::Victory => "Winning",
        Sfx::Defeat => "Losing",
    }
}

/// Who uses what, as the game decides it (`Game::sound_table`): a unit's own
/// names first, else its faction's (selection), else the library's defaults.
fn unit_users(
    library: &SoundLibrary,
    blueprints: &Blueprints,
    users: &mut BTreeMap<String, Vec<User>>,
) {
    let d = &library.defaults;
    let mut add = |name: Option<&String>, fallback: bool, who: &str, when: String, side: &str| {
        if let Some(n) = name {
            let list = users.entry(n.clone()).or_default();
            let u = User {
                who: who.to_owned(),
                when,
                side: side.to_owned(),
                fallback,
            };
            if !list.contains(&u) {
                list.push(u);
            }
        }
    };
    for f in &blueprints.factions {
        let side = &f.abbreviation;
        let who = format!("{side} builders");
        match &f.sounds.build {
            Some(b) => {
                add(
                    Some(&b.beam),
                    false,
                    &who,
                    "building (the beam)".into(),
                    side,
                );
                add(
                    Some(&b.start),
                    false,
                    &who,
                    "starting to build".into(),
                    side,
                );
                add(Some(&b.end), false, &who, "finishing a build".into(), side);
            }
            None => {
                for (n, when) in [
                    ("build_beam", "building (the beam)"),
                    ("build_start", "starting to build"),
                    ("build_end", "finishing a build"),
                ] {
                    add(Some(&n.to_owned()), true, &who, when.into(), side);
                }
            }
        }
    }
    for u in &blueprints.units {
        let f = &blueprints.factions[u.faction.0 as usize];
        let side = f.abbreviation.as_str();
        let who = format!("{} (T{} {})", u.name, u.tech, u.role.to_lowercase());
        let s = &u.sounds;
        add(
            s.death.as_ref().or(d.death.as_ref()),
            s.death.is_none(),
            &who,
            "dies".into(),
            side,
        );
        add(s.moving.as_ref(), false, &who, "moves".into(), side);
        add(s.step.as_ref(), false, &who, "steps".into(), side);
        add(
            s.step_far.as_ref(),
            false,
            &who,
            "steps, heard from far away".into(),
            side,
        );
        let faction_select = f.sounds.select.get(&u.visual.icon);
        let select = s
            .select
            .as_ref()
            .or(faction_select)
            .or(d.select.get(&u.visual.icon));
        add(select, s.select.is_none(), &who, "is selected".into(), side);
        // A capital ship's engine and ramp sounds are named after its moving
        // loop (`audio/capital.rs`): `<moving>_<part>`.
        if let Some(moving) = &s.moving {
            for other in &library.sounds {
                if let Some(part) = other
                    .name
                    .strip_prefix(moving.as_str())
                    .and_then(|p| p.strip_prefix('_'))
                {
                    let when = format!("flies ({})", part.replace('_', " "));
                    add(Some(&other.name), false, &who, when, side);
                }
            }
        }
        for w in &u.weapons {
            let s = &w.sounds;
            let gun = &w.name;
            add(
                s.fire.as_ref().or(d.fire.as_ref()),
                s.fire.is_none(),
                &who,
                format!("fires {gun}"),
                side,
            );
            add(
                s.charge.as_ref(),
                false,
                &who,
                format!("charges {gun}"),
                side,
            );
            add(
                s.spin.as_ref(),
                false,
                &who,
                format!("spins up {gun}"),
                side,
            );
            add(s.whir.as_ref(), false, &who, format!("whirs {gun}"), side);
            add(
                s.spindown.as_ref(),
                false,
                &who,
                format!("runs down {gun}"),
                side,
            );
            add(
                s.hold.as_ref(),
                false,
                &who,
                format!("holds {gun} on"),
                side,
            );
            add(
                s.far.as_ref(),
                false,
                &who,
                format!("{gun} hits, heard from far away"),
                side,
            );
            add(
                s.impact.as_ref().or(d.impact.as_ref()),
                s.impact.is_none(),
                &who,
                format!("{gun} hits a unit"),
                side,
            );
            let ground = s.ground.as_ref().or(s.impact.as_ref());
            add(
                ground.or(d.ground.as_ref()),
                ground.is_none(),
                &who,
                format!("{gun} hits the ground"),
                side,
            );
            add(
                s.casing.as_ref(),
                false,
                &who,
                format!("{gun} drops a casing"),
                side,
            );
            add(
                s.flight.as_ref(),
                false,
                &who,
                format!("{gun} shots in flight"),
                side,
            );
        }
    }
}

/// Sounds the game names in its own code (weather, shields, nukes, the
/// ambience): found as quoted names in `crates/mc-game/src`, if the source is
/// there to read. Test code is skipped.
fn code_users(data: &Path, library: &SoundLibrary, users: &mut BTreeMap<String, Vec<User>>) {
    let Some(root) = data.parent() else { return };
    let src = root.join("crates").join("mc-game").join("src");
    let mut files = Vec::new();
    collect_rs(&src, &mut files);
    files.sort();
    for path in files {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let text = text.split("#[cfg(test)]").next().unwrap_or("");
        let file = path
            .strip_prefix(&src)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        for s in &library.sounds {
            let quoted = format!("\"{}\"", s.name);
            let prefix = s.name.strip_prefix("capital_");
            let by_part = prefix.is_some_and(|part| {
                text.contains("capital_{name}") && text.contains(&format!("\"{part}\""))
            });
            if text.contains(&quoted) || by_part {
                let list = users.entry(s.name.clone()).or_default();
                let when = if by_part {
                    "a capital ship with no sound of its own for this".to_owned()
                } else {
                    format!("plays it ({file})")
                };
                let u = User {
                    who: "The game".into(),
                    when,
                    side: String::new(),
                    fallback: by_part,
                };
                if !list.contains(&u) {
                    list.push(u);
                }
            }
        }
    }
}

fn collect_rs(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect_rs(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

/// What the library files' text says about a sound.
#[derive(Clone, Default)]
struct Note {
    /// Relative to the repository: `data/sounds/battle.ron`.
    file: String,
    /// The `// ---- heading ----` it is under.
    section: String,
    /// The comment above it.
    about: String,
    /// The comment above each of its layers.
    layers: Vec<String>,
    /// Its place across all the files.
    order: usize,
}

type Notes = BTreeMap<String, Note>;

const LAYERS: [&str; 13] = [
    "Tone(", "Stack(", "Fm(", "Burst(", "Hiss(", "Sweep(", "Roll(", "Rumble(", "Drone(", "Wind(",
    "Chirp(", "Chorus(", "Drive(",
];

fn notes(data: &Path) -> Notes {
    let mut files = Vec::new();
    if let Ok(rd) = std::fs::read_dir(data.join("factions")) {
        for e in rd.flatten() {
            let f = e.path().join("sounds.ron");
            if f.is_file() {
                files.push(f);
            }
        }
    }
    files.sort();
    let mut general = Vec::new();
    collect(&data.join("sounds"), &mut general);
    general.sort();
    files.extend(general);
    let mut out = Notes::new();
    for path in files {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let rel = path
            .strip_prefix(data)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        read_notes(&text, &format!("data/{rel}"), &mut out);
    }
    out
}

fn read_notes(text: &str, file: &str, out: &mut Notes) {
    let mut section = String::new();
    let mut comment: Vec<String> = Vec::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        let t = line.trim();
        if let Some(c) = t.strip_prefix("//") {
            let c = c.trim();
            // `// ---- guns ----`: a heading over the sounds that follow.
            if c.starts_with("--") {
                section = c.trim_matches(|ch: char| ch == '-' || ch == ' ').to_owned();
                comment.clear();
            } else {
                comment.push(c.to_owned());
            }
            continue;
        }
        if let Some(name) = sound_key(t) {
            let order = out.len();
            out.insert(
                name.to_owned(),
                Note {
                    file: file.to_owned(),
                    section: section.clone(),
                    about: comment.join(" "),
                    layers: Vec::new(),
                    order,
                },
            );
            current = Some(name.to_owned());
        } else if LAYERS.iter().any(|l| t.starts_with(l)) {
            if let Some(n) = out.get_mut(current.as_deref().unwrap_or("")) {
                n.layers.push(comment.join(" "));
            }
        }
        comment.clear();
    }
}

/// `"name": (` at the start of a line: a sound's key.
fn sound_key(line: &str) -> Option<&str> {
    let rest = line.strip_prefix('"')?;
    let (name, after) = rest.split_once('"')?;
    let after = after.trim_start().strip_prefix(':')?.trim_start();
    (after.starts_with('(') && !name.is_empty()).then_some(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")
    }

    #[test]
    fn every_sound_is_listed_with_its_users() {
        let c = load(&data()).unwrap();
        let tank = c.get("tank_gun").expect("the Warden's gun");
        assert!(tank.group.starts_with("Battle"), "{}", tank.group);
        assert!(tank.file.ends_with("sounds/battle.ron"));
        assert!(!tank.about.is_empty());
        assert!(
            tank.users.iter().any(|u| u.when.starts_with("fires")),
            "{:?}",
            tank.users
        );
        assert_eq!(
            tank.layer_notes.len(),
            tank.sound.as_ref().unwrap().layers.len()
        );
        // The defaults reach the units that name nothing.
        assert!(c.get("blast").unwrap().users.iter().any(|u| u.fallback));
        // The game's own names are found in its code.
        assert!(c
            .get("thunder_near")
            .unwrap()
            .users
            .iter()
            .any(|u| u.who == "The game"));
        assert!(c.get("hover").unwrap().sfx.is_some());
        let unused: Vec<&str> = c
            .entries
            .iter()
            .filter(|e| e.users.is_empty() && e.bases_of.is_empty())
            .map(|e| e.name.as_str())
            .collect();
        eprintln!("used by nothing: {unused:?}");
    }

    #[test]
    fn notes_come_from_the_comments() {
        let mut out = Notes::new();
        read_notes(
            "// ---- guns ----\n// A bang.\n\"pop\": (\n    layers: [\n        // Crack.\n        Burst(low: 1),\n        Tone(from: 1),\n    ],\n),\n",
            "f",
            &mut out,
        );
        let n = &out["pop"];
        assert_eq!(n.section, "guns");
        assert_eq!(n.about, "A bang.");
        assert_eq!(n.layers, ["Crack.".to_owned(), String::new()]);
    }
}
