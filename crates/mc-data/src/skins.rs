//! Skins (`data/skins/<key>.ron`, docs/STANDARD.md): what each surface slot of a
//! model looks like. A model paints slots (plate, trim, emitter, ...), never colours;
//! a skin says what each slot is made of, so a faction's look, a variant of it or a
//! reward can change without touching a mesh or a shader.
//!
//! Presentation only: nothing here reaches the simulation, so skins are not in the
//! content hash.

use crate::{parse_file, sorted_entries, DataError};
use serde::Deserialize;
use std::path::Path;

/// Index into [`crate::Blueprints::skins`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SkinId(pub u8);

/// Skins a game holds at most (`gpu_consts::skin::MAX`).
pub const MAX_SKINS: usize = 16;

/// The brightest a colour may be, linear: emitters are HDR, a few times white.
const MOST_LIGHT: f32 = 64.0;

/// A colour, in linear RGB. It may be given outright or taken from the skin's own
/// colours, so a slot that follows another (the dark plate is the plate darkened,
/// glass carries a little of the emitter's light) follows it when a skin changes it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize)]
pub enum Paint {
    /// This colour.
    Rgb([f32; 3]),
    /// This colour at this intensity: a light's hue and how bright it burns.
    Light([f32; 3], f32),
    /// The skin's `highlight` at this intensity.
    Highlight(f32),
    /// The plate's colour times this tint, channel by channel.
    Plate([f32; 3]),
    /// No colour (an unlit slot's emission).
    #[default]
    None,
}

/// One surface slot: its colour, how metallic and rough it is, and what light it gives.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Surface {
    pub color: Paint,
    #[serde(default)]
    pub metal: f32,
    pub rough: f32,
    #[serde(default)]
    pub glow: Paint,
}

/// Every slot a model can paint (`mc_models::material`, `gpu_consts::skin`).
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Slots {
    pub plate: Surface,
    pub plate_dark: Surface,
    pub trim: Surface,
    pub emitter: Surface,
    pub metal: Surface,
    pub glass: Surface,
    pub rubber: Surface,
    pub gun_glow: Surface,
    pub build_glow: Surface,
    pub beacon: Surface,
    pub nanite: Surface,
    pub laser: Surface,
    pub visor: Surface,
}

/// The Regency finish (regency.wgsl): steel under an iridescent film over graphite
/// machinery. Its colours, linear RGB.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Film {
    /// The plate seen square on.
    pub black: [f32; 3],
    /// The film's sheen as a face turns away: crimson, deep red, copper.
    pub red: [f32; 3],
    pub deep: [f32; 3],
    pub copper: [f32; 3],
    /// The paler second plate.
    pub pale: [f32; 3],
    /// Plate worn through its film to bright steel.
    pub steel_lit: [f32; 3],
    /// The machinery, darker in its grooves, polished where it is worked.
    pub works: [f32; 3],
    pub works_deep: [f32; 3],
    pub works_polish: [f32; 3],
}

/// How plated faces are finished: the grain, wear and sheen under the slots' colours.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
pub enum Finish {
    /// Paint over scanned brushed steel, scratched through at the edges (metal.wgsl).
    #[default]
    Arc,
    /// Steel under an iridescent film over graphite machinery (regency.wgsl, `film`).
    Regency,
}

/// The lights let into dark plating, one to a plate where it has room, in this colour
/// (linear, burning brighter by tier; paint on a tech 1 line unit, where nothing is lit).
#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize)]
pub enum Lights {
    /// None.
    #[default]
    Off,
    /// A short level line.
    Dashes([f32; 3]),
    /// A row of dots, like lit ports.
    Dots([f32; 3]),
}

/// The finish over a skin's plated slots.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Plates {
    pub kind: Finish,
    /// Plate size, times the standard (the courses a face is cut into).
    pub plate: f32,
    /// How much paint the edges lose, times the standard.
    pub wear: f32,
    pub lights: Lights,
    /// Texture detail inside each plate, 0 (none) to 1 (dense): panels, grilles,
    /// louvres, ribs, access panels, bolt rings, vents and hazard stripes.
    pub detail: f32,
    /// How deep and dark seams and recesses are, times the standard.
    pub seams: f32,
    /// How much field dirt and grime a unit gathers, times its tier's.
    pub grime: f32,
}

impl Default for Plates {
    fn default() -> Plates {
        Plates {
            kind: Finish::Arc,
            plate: 1.0,
            wear: 1.0,
            lights: Lights::Off,
            detail: 0.0,
            seams: 1.0,
            grime: 1.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Skin {
    /// Stable: saved with a match and named by a faction. The file's name.
    #[serde(skip)]
    pub key: String,
    /// The skin's light: its emitters' hue (ghosts and charge bands take it too).
    pub highlight: [f32; 3],
    pub slots: Slots,
    /// How its plated faces are finished.
    #[serde(default)]
    pub finish: Plates,
    /// The Regency finish's colours: needed by a skin with that finish.
    #[serde(default)]
    pub film: Option<Film>,
}

impl Skin {
    /// A paint's colour in this skin.
    pub fn paint(&self, p: Paint) -> [f32; 3] {
        match p {
            Paint::Rgb(c) => c,
            Paint::Light(c, k) => c.map(|v| v * k),
            Paint::Highlight(k) => self.highlight.map(|v| v * k),
            Paint::Plate(t) => {
                let plate = self.paint_plain(self.slots.plate.color);
                [plate[0] * t[0], plate[1] * t[1], plate[2] * t[2]]
            }
            Paint::None => [0.0; 3],
        }
    }

    /// The plate's own colour: it may not follow itself.
    fn paint_plain(&self, p: Paint) -> [f32; 3] {
        match p {
            Paint::Plate(_) => [0.0; 3],
            p => self.paint(p),
        }
    }

    /// The slots and their names, in `gpu_consts::skin` order.
    pub fn slot_list(&self) -> [(&'static str, &Surface); 13] {
        let s = &self.slots;
        [
            ("plate", &s.plate),
            ("trim", &s.trim),
            ("emitter", &s.emitter),
            ("metal", &s.metal),
            ("glass", &s.glass),
            ("rubber", &s.rubber),
            ("gun_glow", &s.gun_glow),
            ("build_glow", &s.build_glow),
            ("plate_dark", &s.plate_dark),
            ("beacon", &s.beacon),
            ("nanite", &s.nanite),
            ("laser", &s.laser),
            ("visor", &s.visor),
        ]
    }

    fn check(&self) -> Result<(), String> {
        let colour = |what: &str, c: [f32; 3]| {
            if c.iter()
                .all(|v| v.is_finite() && (0.0..=MOST_LIGHT).contains(v))
            {
                Ok(())
            } else {
                Err(format!("{what} {c:?}: each channel 0 to {MOST_LIGHT}"))
            }
        };
        let unit = |what: &str, v: f32| {
            if v.is_finite() && (0.0..=1.0).contains(&v) {
                Ok(())
            } else {
                Err(format!("{what} {v}: 0 to 1"))
            }
        };
        colour("highlight", self.highlight)?;
        if matches!(self.slots.plate.color, Paint::Plate(_)) {
            return Err("plate: its colour cannot follow the plate".into());
        }
        for (what, s) in self.slot_list() {
            colour(what, self.paint(s.color))?;
            colour(what, self.paint(s.glow))?;
            for p in [s.color, s.glow] {
                if let Paint::Light(_, k) | Paint::Highlight(k) = p {
                    if !(k.is_finite() && (0.0..=MOST_LIGHT).contains(&k)) {
                        return Err(format!("{what}: intensity {k}"));
                    }
                }
            }
            unit(what, s.metal)?;
            unit(what, s.rough)?;
        }
        if self.finish.kind == Finish::Regency && self.film.is_none() {
            return Err("finish Regency needs its film's colours (`film`)".into());
        }
        let p = &self.finish;
        for (what, v) in [
            ("finish plate", p.plate),
            ("finish wear", p.wear),
            ("finish seams", p.seams),
        ] {
            if !(v.is_finite() && (0.25..=4.0).contains(&v)) {
                return Err(format!("{what} {v}: 0.25 to 4"));
            }
        }
        if !(p.detail.is_finite() && (0.0..=1.0).contains(&p.detail)) {
            return Err(format!("finish detail {}: 0 to 1", p.detail));
        }
        if !(p.grime.is_finite() && (0.0..=4.0).contains(&p.grime)) {
            return Err(format!("finish grime {}: 0 to 4", p.grime));
        }
        if let Lights::Dashes(c) | Lights::Dots(c) = p.lights {
            colour("finish lights", c)?;
        }
        if let Some(f) = &self.film {
            for c in [
                f.black,
                f.red,
                f.deep,
                f.copper,
                f.pale,
                f.steel_lit,
                f.works,
                f.works_deep,
                f.works_polish,
            ] {
                colour("film", c)?;
            }
        }
        Ok(())
    }
}

/// Every skin under `<data_dir>/skins`, in sorted key order.
pub(crate) fn load(data_dir: &Path) -> Result<Vec<Skin>, DataError> {
    let dir = data_dir.join("skins");
    let mut skins = Vec::new();
    for path in sorted_entries(&dir)? {
        if path.extension().is_none_or(|e| e != "ron") {
            continue;
        }
        let mut skin: Skin = parse_file(&path)?;
        skin.key = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_owned();
        skin.check()
            .map_err(|e| DataError::Invalid(format!("{}: {e}", path.display())))?;
        skins.push(skin);
    }
    if skins.len() > MAX_SKINS {
        return Err(DataError::Invalid(format!(
            "{}: {} skins, at most {MAX_SKINS}",
            dir.display(),
            skins.len()
        )));
    }
    Ok(skins)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data_dir() -> std::path::PathBuf {
        Path::new(&std::env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../../data")
    }

    #[test]
    fn every_skin_loads_and_every_faction_names_one() {
        let bps = crate::Blueprints::load(&data_dir()).unwrap();
        assert!(!bps.skins.is_empty());
        for f in &bps.factions {
            assert!((f.skin.0 as usize) < bps.skins.len(), "{}", f.key);
        }
    }

    #[test]
    fn a_paint_follows_the_skin() {
        let bps = crate::Blueprints::load(&data_dir()).unwrap();
        let mut skin = bps.skins[0].clone();
        skin.highlight = [1.0, 0.5, 0.25];
        assert_eq!(skin.paint(Paint::Highlight(2.0)), [2.0, 1.0, 0.5]);
        skin.slots.plate.color = Paint::Rgb([0.5, 0.5, 0.5]);
        assert_eq!(skin.paint(Paint::Plate([0.5, 1.0, 2.0])), [0.25, 0.5, 1.0]);
    }

    #[test]
    fn out_of_range_values_are_refused() {
        let bps = crate::Blueprints::load(&data_dir()).unwrap();
        let mut skin = bps.skins[0].clone();
        skin.slots.trim.rough = 2.0;
        assert!(skin.check().is_err());
        let mut skin = bps.skins[0].clone();
        skin.slots.emitter.glow = Paint::Light([1.0, 1.0, 1.0], f32::NAN);
        assert!(skin.check().is_err());
        let mut skin = bps.skins[0].clone();
        skin.slots.plate.color = Paint::Plate([1.0; 3]);
        assert!(skin.check().is_err());
        let mut skin = bps.skins[0].clone();
        skin.finish.kind = Finish::Regency;
        skin.film = None;
        assert!(skin.check().is_err(), "a Regency finish with no film");
        let mut skin = bps.skins[0].clone();
        skin.finish.plate = 0.0;
        assert!(skin.check().is_err());
    }
}
