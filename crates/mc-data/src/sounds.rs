//! The sound library: named sounds that unit files refer to.
//!
//! A sound is written once and used by whatever fits it; most weapons share a
//! handful. General sounds live in `data/sounds/*.ron`, a faction's own in
//! `data/factions/<faction>/sounds.ron`; names are global, and a faction file
//! may not redefine a general name. A unit file names sounds in its `sounds`
//! blocks; what it leaves out comes from the library's `defaults`. Not all of
//! it is battle: the sound a unit answers a selection with is here too.
//!
//! There are no audio files. A sound is a recipe, a list of layers the game
//! synthesises at start-up (see `mc-game`'s `audio`), so the library can be
//! tuned in a text editor and reloaded without a rebuild. Nothing here reaches
//! the simulation, and none of it is in the blueprint content hash.

use crate::{sorted_entries, DataError, IconKind};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;

/// One layer of a recipe. Times in seconds, frequencies in hertz, `pan` from
/// -1 (left) to 1. Every layer sounds from `at` and dies away on its own:
/// `attack` is the rise to full level, `decay` the time to fall to about a third.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub enum Layer {
    /// A sine gliding from `from` to `to` over `glide` seconds: thumps, booms, whines.
    Tone {
        #[serde(default)]
        at: f32,
        from: f32,
        to: f32,
        glide: f32,
        attack: f32,
        decay: f32,
        gain: f32,
        #[serde(default)]
        pan: f32,
    },
    /// Several sines gliding together, `partials` as (multiple of the base, gain), the two
    /// ears `detune` apart and `width` to either side: the buzz of an energy bolt.
    Stack {
        #[serde(default)]
        at: f32,
        from: f32,
        to: f32,
        glide: f32,
        attack: f32,
        decay: f32,
        gain: f32,
        detune: f32,
        width: f32,
        partials: Vec<(f32, f32)>,
    },
    /// A sine whose pitch is shaken by a second one at `ratio` times its frequency, `index` deep,
    /// the shaking dying away over `fade` (as `decay` does): it starts bright and mellows, which is
    /// what a struck or driven thing does. A whole `ratio` (1, 2, 3) is reedy, a servo or a horn;
    /// a half is a growl under the note; anything else (1.41, 3.5) is metal. Keep `index` under
    /// about 3, and lower the higher the note, or it turns to fizz.
    Fm {
        #[serde(default)]
        at: f32,
        from: f32,
        to: f32,
        glide: f32,
        ratio: f32,
        index: f32,
        fade: f32,
        attack: f32,
        decay: f32,
        gain: f32,
        #[serde(default)]
        pan: f32,
    },
    /// A short burst of wide noise between `low` and `high`, steep above `high`:
    /// the crack of a gun. Keep it short and `high` low, or it is heard as static.
    Burst {
        #[serde(default)]
        at: f32,
        low: f32,
        high: f32,
        attack: f32,
        decay: f32,
        gain: f32,
        #[serde(default)]
        pan: f32,
        #[serde(default)]
        seed: Option<u32>,
    },
    /// Soft band-passed noise around `freq`: air, dirt, debris.
    Hiss {
        #[serde(default)]
        at: f32,
        freq: f32,
        q: f32,
        attack: f32,
        decay: f32,
        gain: f32,
        #[serde(default)]
        pan: f32,
        #[serde(default)]
        seed: Option<u32>,
    },
    /// Noise through a band gliding from `from` to `to`: a report rolling away, a fire coming up.
    Sweep {
        #[serde(default)]
        at: f32,
        from: f32,
        to: f32,
        glide: f32,
        q: f32,
        attack: f32,
        decay: f32,
        gain: f32,
        #[serde(default)]
        pan: f32,
        #[serde(default)]
        seed: Option<u32>,
    },
    /// Noise whose level is steady: the bed of a loop (an engine, tracks on the ground).
    /// `wobble` hertz of slow level change by `depth` (zero to one).
    Rumble {
        freq: f32,
        q: f32,
        gain: f32,
        #[serde(default)]
        wobble: f32,
        #[serde(default)]
        depth: f32,
        #[serde(default)]
        pan: f32,
        #[serde(default)]
        seed: Option<u32>,
    },
    /// A steady tone with the same slow wobble, for loops.
    Drone {
        freq: f32,
        gain: f32,
        #[serde(default)]
        wobble: f32,
        #[serde(default)]
        depth: f32,
        #[serde(default)]
        pan: f32,
    },
    /// Soft saturation of everything laid down so far, around 1 to 2. Put it
    /// before the noise tails: over them it turns into static.
    Drive(f32),
}

/// A sound as written in a library file: either its own layers, or `like`
/// another sound at a different `size` (bigger is lower and longer).
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSound {
    #[serde(default)]
    like: Option<String>,
    #[serde(default = "one")]
    size: f32,
    #[serde(default)]
    length: Option<f32>,
    #[serde(default)]
    peak: Option<f32>,
    #[serde(default)]
    room: Option<f32>,
    #[serde(default)]
    looped: Option<bool>,
    #[serde(default)]
    layers: Vec<Layer>,
}

fn one() -> f32 {
    1.0
}

/// What plays when a unit file names nothing.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct Defaults {
    /// A weapon firing. Anything that should not sound like a plain gun names its own.
    pub fire: Option<String>,
    /// A shot striking a unit, and striking the ground.
    pub impact: Option<String>,
    pub ground: Option<String>,
    pub death: Option<String>,
    /// A unit being selected, by the kind of unit it is. A file read later adds
    /// to this and replaces kind by kind.
    pub select: BTreeMap<IconKind, String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLibrary {
    #[serde(default)]
    defaults: Defaults,
    sounds: BTreeMap<String, RawSound>,
}

/// A sound ready to synthesise: `like` and `size` already applied.
#[derive(Clone, Debug, PartialEq)]
pub struct Sound {
    pub name: String,
    pub length: f32,
    /// Level of the loudest sample after synthesis, zero to one.
    pub peak: f32,
    /// How much of the shared short room is added.
    pub room: f32,
    /// Plays round and round while something goes on (movement), instead of once.
    pub looped: bool,
    pub layers: Vec<Layer>,
}

/// Index of a sound in [`SoundLibrary::sounds`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct SoundId(pub u16);

#[derive(Clone, Debug, Default)]
pub struct SoundLibrary {
    /// In name order.
    pub sounds: Vec<Sound>,
    pub defaults: Defaults,
}

impl SoundLibrary {
    /// Loads `<data_dir>/sounds/*.ron` and every `<data_dir>/factions/*/sounds.ron`.
    pub fn load(data_dir: &Path) -> Result<SoundLibrary, DataError> {
        let mut files = Vec::new();
        let general = data_dir.join("sounds");
        if general.is_dir() {
            files.extend(
                sorted_entries(&general)?
                    .into_iter()
                    .filter(|f| f.extension().is_some_and(|e| e == "ron")),
            );
        }
        for dir in sorted_entries(&data_dir.join("factions"))? {
            let file = dir.join("sounds.ron");
            if file.is_file() {
                files.push(file);
            }
        }
        let mut raw: BTreeMap<String, RawSound> = BTreeMap::new();
        let mut defaults = Defaults::default();
        for file in files {
            let library: RawLibrary = crate::parse_file(&file)?;
            for (name, sound) in library.sounds {
                if raw.insert(name.clone(), sound).is_some() {
                    return Err(DataError::Invalid(format!(
                        "{}: the sound {name} is already defined in another file",
                        file.display()
                    )));
                }
            }
            let d = library.defaults;
            for (slot, given) in [
                (&mut defaults.fire, d.fire),
                (&mut defaults.impact, d.impact),
                (&mut defaults.ground, d.ground),
                (&mut defaults.death, d.death),
            ] {
                if given.is_some() {
                    *slot = given;
                }
            }
            defaults.select.extend(d.select);
        }
        Self::compile(raw, defaults)
    }

    fn compile(
        raw: BTreeMap<String, RawSound>,
        defaults: Defaults,
    ) -> Result<SoundLibrary, DataError> {
        let mut sounds = Vec::with_capacity(raw.len());
        for name in raw.keys() {
            sounds.push(resolve(name, &raw, 0)?);
        }
        let library = SoundLibrary { sounds, defaults };
        let d = &library.defaults;
        for name in [&d.fire, &d.impact, &d.ground, &d.death]
            .into_iter()
            .flatten()
            .chain(d.select.values())
        {
            library.require(name, "sound defaults")?;
        }
        Ok(library)
    }

    pub fn id_of(&self, name: &str) -> Option<SoundId> {
        self.sounds
            .binary_search_by(|s| s.name.as_str().cmp(name))
            .ok()
            .map(|i| SoundId(i as u16))
    }

    pub fn sound(&self, id: SoundId) -> &Sound {
        &self.sounds[id.0 as usize]
    }

    /// `name` as an id, or an error that says who asked for it.
    pub fn require(&self, name: &str, asked_by: &str) -> Result<SoundId, DataError> {
        self.id_of(name).ok_or_else(|| {
            DataError::Invalid(format!("{asked_by}: there is no sound called {name}"))
        })
    }

    /// Checks every sound the blueprints name, so a typo is an error at start-up and not silence in a battle.
    pub fn check(&self, blueprints: &crate::Blueprints) -> Result<(), DataError> {
        for u in &blueprints.units {
            for name in [
                &u.sounds.death,
                &u.sounds.moving,
                &u.sounds.step,
                &u.sounds.select,
            ]
            .into_iter()
            .flatten()
            {
                self.require(name, &u.key)?;
            }
            for w in &u.weapons {
                let s = &w.sounds;
                for name in [&s.fire, &s.charge, &s.impact, &s.ground]
                    .into_iter()
                    .flatten()
                {
                    self.require(name, &format!("{}/{}", u.key, w.name))?;
                }
            }
        }
        Ok(())
    }
}

/// `name` with its `like` chain followed and every `size` on the way applied.
fn resolve(name: &str, raw: &BTreeMap<String, RawSound>, depth: usize) -> Result<Sound, DataError> {
    let r = raw
        .get(name)
        .ok_or_else(|| DataError::Invalid(format!("there is no sound called {name}")))?;
    if depth > 8 {
        return Err(DataError::Invalid(format!(
            "the sound {name} is `like` itself, through others"
        )));
    }
    let mut sound = match &r.like {
        Some(base) => {
            if !r.layers.is_empty() {
                return Err(DataError::Invalid(format!(
                    "the sound {name} has both `like` and layers of its own"
                )));
            }
            resolve(base, raw, depth + 1).map_err(|e| DataError::Invalid(format!("{name}: {e}")))?
        }
        None => {
            if r.layers.is_empty() {
                return Err(DataError::Invalid(format!(
                    "the sound {name} has no layers"
                )));
            }
            Sound {
                name: String::new(),
                length: r.length.unwrap_or(1.0),
                peak: 0.6,
                room: 0.5,
                looped: false,
                layers: r.layers.clone(),
            }
        }
    };
    sound.name = name.to_owned();
    if r.like.is_some() {
        if r.size <= 0.05 {
            return Err(DataError::Invalid(format!(
                "the sound {name} has a size of {}",
                r.size
            )));
        }
        sound.length = r.length.unwrap_or(sound.length * r.size);
        for layer in &mut sound.layers {
            layer.resize(r.size);
        }
    }
    sound.peak = r.peak.unwrap_or(sound.peak).clamp(0.05, 0.9);
    sound.room = r.room.unwrap_or(sound.room);
    sound.looped = r.looped.unwrap_or(sound.looped);
    if !(0.05..=12.0).contains(&sound.length) {
        return Err(DataError::Invalid(format!(
            "the sound {name} is {} seconds long",
            sound.length
        )));
    }
    Ok(sound)
}

impl Layer {
    /// The same layer for something `size` times bigger: lower by that much, and that much longer.
    fn resize(&mut self, size: f32) {
        let k = 1.0 / size;
        match self {
            Layer::Tone {
                at,
                from,
                to,
                glide,
                decay,
                ..
            }
            | Layer::Stack {
                at,
                from,
                to,
                glide,
                decay,
                ..
            }
            | Layer::Sweep {
                at,
                from,
                to,
                glide,
                decay,
                ..
            } => {
                (*at, *from, *to, *glide, *decay) =
                    (*at * size, *from * k, *to * k, *glide * size, *decay * size);
            }
            Layer::Fm {
                at,
                from,
                to,
                glide,
                fade,
                decay,
                ..
            } => {
                (*at, *from, *to, *glide, *fade, *decay) = (
                    *at * size,
                    *from * k,
                    *to * k,
                    *glide * size,
                    *fade * size,
                    *decay * size,
                )
            }
            Layer::Burst {
                at,
                low,
                high,
                decay,
                ..
            } => (*at, *low, *high, *decay) = (*at * size, *low * k, *high * k, *decay * size),
            Layer::Hiss {
                at, freq, decay, ..
            } => (*at, *freq, *decay) = (*at * size, *freq * k, *decay * size),
            Layer::Rumble { freq, .. } | Layer::Drone { freq, .. } => *freq *= k,
            Layer::Drive(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data_dir() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")
    }

    #[test]
    fn the_shipped_library_loads_and_covers_the_blueprints() {
        let library = SoundLibrary::load(&data_dir()).unwrap();
        let blueprints = crate::Blueprints::load(&data_dir()).unwrap();
        library.check(&blueprints).unwrap();
        assert!(
            library.sounds.windows(2).all(|w| w[0].name < w[1].name),
            "sounds are in name order"
        );
        let d = &library.defaults;
        assert!(
            d.fire.is_some() && d.impact.is_some() && d.ground.is_some() && d.death.is_some(),
            "every default is set"
        );
        // Every unit answers a selection, and not every kind answers alike.
        let select = |u: &crate::UnitBlueprint| {
            u.sounds
                .select
                .as_ref()
                .or(d.select.get(&u.visual.icon))
                .cloned()
        };
        assert!(
            blueprints.units.iter().all(|u| select(u).is_some()),
            "every unit has a selection sound"
        );
        let answers: std::collections::BTreeSet<_> =
            blueprints.units.iter().filter_map(select).collect();
        assert!(answers.len() >= 6, "{} selection sounds", answers.len());
        // Sounds are shared: there are far fewer of them than weapons that make a noise.
        let weapons: usize = blueprints.units.iter().map(|u| u.weapons.len()).sum();
        let fired: std::collections::BTreeSet<_> = blueprints
            .units
            .iter()
            .flat_map(|u| &u.weapons)
            .filter_map(|w| w.sounds.fire.as_ref())
            .collect();
        assert!(
            fired.len() < weapons,
            "{} firing sounds for {weapons} weapons",
            fired.len()
        );
    }

    #[test]
    fn like_makes_a_bigger_sound_lower_and_longer() {
        let text = r#"(sounds: {
            "pop": (length: 0.5, peak: 0.5, layers: [Tone(from: 200, to: 100, glide: 0.1, attack: 0.001, decay: 0.05, gain: 1), Drive(1.2)]),
            "boom": (like: "pop", size: 2),
        })"#;
        let raw: RawLibrary = ron::Options::default()
            .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
            .from_str(text)
            .unwrap();
        let library = SoundLibrary::compile(raw.sounds, raw.defaults).unwrap();
        let boom = library.sound(library.id_of("boom").unwrap());
        assert_eq!(boom.length, 1.0);
        assert_eq!(
            boom.layers[0],
            Layer::Tone {
                at: 0.0,
                from: 100.0,
                to: 50.0,
                glide: 0.2,
                attack: 0.001,
                decay: 0.1,
                gain: 1.0,
                pan: 0.0
            }
        );
        assert_eq!(boom.layers[1], Layer::Drive(1.2));
        assert!(library.id_of("bang").is_none());
    }

    #[test]
    fn mistakes_are_named() {
        let compile = |text: &str| {
            let raw: RawLibrary = ron::Options::default()
                .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
                .from_str(text)
                .unwrap();
            SoundLibrary::compile(raw.sounds, raw.defaults)
                .map(|_| ())
                .map_err(|e| e.to_string())
        };
        assert!(compile(r#"(sounds: {"a": (like: "b"), "b": (like: "a")})"#)
            .unwrap_err()
            .contains("like"));
        assert!(compile(r#"(sounds: {"a": (like: "nothing")})"#)
            .unwrap_err()
            .contains("nothing"));
        assert!(compile(r#"(sounds: {"a": ()})"#)
            .unwrap_err()
            .contains("no layers"));
        assert!(
            compile(r#"(defaults: (fire: "gone"), sounds: {"a": (layers: [Drive(1)])})"#)
                .unwrap_err()
                .contains("gone")
        );
        assert!(compile(
            r#"(defaults: (select: {Tank: "lost"}), sounds: {"a": (layers: [Drive(1)])})"#
        )
        .unwrap_err()
        .contains("lost"));
    }
}
