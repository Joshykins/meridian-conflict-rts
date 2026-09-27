//! The music folder a song belongs to: its instrument library
//! (`instruments/<name>.ron`) and its recorded sample sets (`samples/<set>/`).
//!
//! A song names library instruments with `Use("violins")` and sets with
//! `Sampler((set: "violins", ..))`; loading the song links it to its folder, and
//! the engine turns every track's instrument into one it can play with
//! [`Library::voice`].

use crate::patch::{Bank, Instrument};
use crate::samples::{self, SampleSet};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// A song's link to its folder. Not part of what the song says: songs compare
/// equal whatever their libraries.
#[derive(Clone, Default)]
pub struct Library(Option<Arc<Linked>>);

struct Linked {
    dir: PathBuf,
    instruments: HashMap<String, Instrument>,
    sets: HashMap<String, Arc<SampleSet>>,
    /// What could not be loaded, for `mc-music check` and the studio.
    problems: Vec<String>,
}

impl PartialEq for Library {
    fn eq(&self, _: &Library) -> bool {
        true
    }
}

impl std::fmt::Debug for Library {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            Some(l) => write!(f, "Library({})", l.dir.display()),
            None => f.write_str("Library(none)"),
        }
    }
}

/// The music folder for a song file: the nearest folder at or above it holding
/// `instruments/` or `samples/` (so moments and the studio's saved revisions find it too).
pub fn music_dir(song_path: &Path) -> Option<PathBuf> {
    song_path
        .ancestors()
        .skip(1)
        .find(|d| d.join("instruments").is_dir() || d.join("samples").is_dir())
        .map(Path::to_path_buf)
}

pub fn instruments_dir(music_dir: &Path) -> PathBuf {
    music_dir.join("instruments")
}

pub fn samples_dir(music_dir: &Path) -> PathBuf {
    music_dir.join("samples")
}

/// The sample sets in the folder, by name, sorted.
pub fn set_names(music_dir: &Path) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(samples_dir(music_dir))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().join("set.ron").is_file())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    out.sort();
    out
}

/// A library instrument as a menu shows it.
#[derive(Clone, Debug)]
pub struct Listing {
    /// Its file name, what `Use` names.
    pub name: String,
    /// What it is: a recording's title (`set.ron`), else the name.
    pub title: String,
    pub recorded: bool,
}

/// Every library instrument, recorded ones first, each group by title.
pub fn catalogue(music_dir: &Path) -> Vec<Listing> {
    #[derive(serde::Deserialize)]
    struct Title {
        title: String,
    }
    let mut out: Vec<Listing> = instrument_names(music_dir)
        .into_iter()
        .map(|name| {
            let set = match load_instrument(music_dir, &name) {
                Ok(Instrument::Sampler(s)) => Some(s.set),
                _ => None,
            };
            let title = set.as_ref().and_then(|set| {
                let text =
                    std::fs::read_to_string(samples_dir(music_dir).join(set).join("set.ron"))
                        .ok()?;
                ron::from_str::<Title>(&text).ok().map(|t| t.title)
            });
            Listing {
                title: title.unwrap_or_else(|| name.replace('_', " ")),
                recorded: set.is_some(),
                name,
            }
        })
        .collect();
    out.sort_by(|a, b| (!a.recorded, &a.title).cmp(&(!b.recorded, &b.title)));
    out
}

/// The library instruments in the folder, by name, sorted.
pub fn instrument_names(music_dir: &Path) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(instruments_dir(music_dir))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            if p.extension()? != "ron" {
                return None;
            }
            Some(p.file_stem()?.to_string_lossy().into_owned())
        })
        .collect();
    out.sort();
    out
}

/// A library instrument as written (a `Use` inside it is not followed).
pub fn load_instrument(dir: &Path, name: &str) -> Result<Instrument, String> {
    let path = instruments_dir(dir).join(format!("{name}.ron"));
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    ron::Options::default()
        .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
        .from_str(&text)
        .map_err(|e| format!("{}: {e}", path.display()))
}

impl Library {
    /// Links `instruments` (a song's tracks) to the folder `dir`: reads every library
    /// instrument they name and decodes every sample set they play (once per process;
    /// `samples::open` keeps them).
    pub fn link<'a>(dir: &Path, instruments: impl Iterator<Item = &'a Instrument>) -> Library {
        let mut l = Linked {
            dir: dir.to_path_buf(),
            instruments: HashMap::new(),
            sets: HashMap::new(),
            problems: Vec::new(),
        };
        for inst in instruments {
            let inst = match inst {
                Instrument::Use(name) => {
                    if !l.instruments.contains_key(name) {
                        match load_instrument(dir, name) {
                            Ok(Instrument::Use(_)) => {
                                l.problems
                                    .push(format!("instrument {name}: names another instrument"));
                                continue;
                            }
                            Ok(i) => {
                                l.instruments.insert(name.clone(), i);
                            }
                            Err(e) => {
                                l.problems.push(e);
                                continue;
                            }
                        }
                    }
                    &l.instruments[name]
                }
                other => other,
            };
            if let Instrument::Sampler(s) = inst {
                if !l.sets.contains_key(&s.set) {
                    let set = s.set.clone();
                    match samples::open(&samples_dir(dir).join(&set)) {
                        Ok(bank) => {
                            l.sets.insert(set, bank);
                        }
                        Err(e) => l.problems.push(e.to_string()),
                    }
                }
            }
        }
        Library(Some(Arc::new(l)))
    }

    /// The folder this song was linked to.
    pub fn dir(&self) -> Option<&Path> {
        self.0.as_ref().map(|l| l.dir.as_path())
    }

    pub fn problems(&self) -> &[String] {
        self.0.as_ref().map_or(&[], |l| &l.problems)
    }

    /// The instrument to play for `inst`: a library instrument in place of its name,
    /// and every sampler holding its decoded set. Anything missing plays silence.
    pub fn voice(&self, inst: &Instrument) -> Instrument {
        let inst = match (inst, &self.0) {
            (Instrument::Use(name), Some(l)) => match l.instruments.get(name) {
                Some(i) => i,
                None => return Instrument::Use(name.clone()),
            },
            _ => inst,
        };
        match inst {
            Instrument::Sampler(s) => {
                let mut s = s.clone();
                s.bank = Bank(self.0.as_ref().and_then(|l| l.sets.get(&s.set).cloned()));
                Instrument::Sampler(s)
            }
            other => other.clone(),
        }
    }
}
