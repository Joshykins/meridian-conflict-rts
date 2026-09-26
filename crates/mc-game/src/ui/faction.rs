//! The playable factions as the front end sees them, what the race card says
//! about each (its `codex.ron`), and each one's sigil: the
//! small mark that says who a commander fights for wherever a seat is shown
//! (set-up screens, the loading chart).

use super::emblem::{self, Art, Mark};
use super::{Color, Rect, Ui};
use glam::Vec2;
use std::sync::OnceLock;

/// What the race card tells about a faction: `data/factions/<key>/codex.ron`.
#[derive(Clone, Debug, Default, serde::Deserialize)]
#[serde(default)]
pub struct Codex {
    /// The drawings its marks are made from; none draws a ring with its initial.
    pub art: Option<Art>,
    pub motto: String,
    /// What the name means.
    pub meaning: String,
    /// Who they are, in a few sentences.
    pub about: String,
    /// The crest read piece by piece: (piece, what it stands for).
    pub crest: Vec<(String, String)>,
    /// How its army fights.
    pub field: String,
    /// The marks the card shows under the crest.
    pub marks: Vec<Mark>,
}

/// A side a commander can play: a faction in `data/factions/`.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct Race {
    /// The faction key the sim knows it by: "Aster".
    pub key: String,
    /// "Asterian Reach Command", for screens with room for it.
    pub name: String,
    /// "ARC": what the set-up screens show.
    pub abbreviation: String,
    /// Its emitter colour, linear RGB: the sigil is drawn in it.
    #[serde(default = "default_highlight")]
    pub highlight_color: [f32; 3],
    /// The faction whose roster it fields while it has none of its own.
    #[serde(default)]
    pub stand_in: Option<String>,
    /// Read from `codex.ron` beside `faction.ron`.
    #[serde(skip)]
    pub codex: Codex,
}

fn default_highlight() -> [f32; 3] {
    [0.8, 0.8, 0.8]
}

impl Race {
    /// Its sigil colour at `alpha`.
    pub fn tint(&self, alpha: f32) -> Color {
        let [r, g, b] = self.highlight_color;
        [r, g, b, alpha]
    }

    /// The name of the faction whose units it borrows, while it has none of its own.
    pub fn borrowed_roster(&self) -> Option<&str> {
        let key = self.stand_in.as_deref()?;
        Some(races().iter().find(|r| r.key.eq_ignore_ascii_case(key)).map_or(key, |r| r.abbreviation.as_str()))
    }
}

/// Every playable race, read once from the factions' `faction.ron` files.
/// Factions with a roster of their own come first, in key order, so index 0
/// is always one that fields its own army.
pub fn races() -> &'static [Race] {
    static RACES: OnceLock<Vec<Race>> = OnceLock::new();
    RACES.get_or_init(|| {
        let mut out: Vec<Race> = mc_data::Blueprints::locate_data_dir()
            .and_then(|d| std::fs::read_dir(d.join("factions")).ok())
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let dir = e.path();
                let mut race = ron::from_str::<Race>(&std::fs::read_to_string(dir.join("faction.ron")).ok()?).ok()?;
                if let Ok(text) = std::fs::read_to_string(dir.join("codex.ron")) {
                    match ron::from_str::<Codex>(&text) {
                        Ok(codex) => race.codex = codex,
                        Err(e) => log::warn!("{}: {e}", dir.join("codex.ron").display()),
                    }
                }
                Some(race)
            })
            .collect();
        out.sort_by(|a, b| (a.stand_in.is_some(), &a.key).cmp(&(b.stand_in.is_some(), &b.key)));
        if out.is_empty() {
            out.push(Race {
                key: "Aster".into(),
                name: "Asterian Reach Command".into(),
                abbreviation: "ARC".into(),
                highlight_color: [0.55, 0.85, 1.0],
                stand_in: None,
                codex: Codex::default(),
            });
        }
        out
    })
}

/// The race's faction key; the first race for an index out of range.
pub fn race_key(race: u8) -> String {
    race_of(race).key.clone()
}

/// The race at `race`; the first for an index out of range.
pub fn race_of(race: u8) -> &'static Race {
    let all = races();
    all.get(race as usize).unwrap_or(&all[0])
}

/// The race a faction key names (any case), if it is playable.
pub fn race_by_key(key: &str) -> Option<&'static Race> {
    races().iter().find(|r| r.key.eq_ignore_ascii_case(key))
}

/// Draws the faction's sigil (its badge), `r` points in radius, at `alpha`,
/// in the faction's colour.
pub fn sigil(ui: &mut Ui, key: &str, centre: Vec2, r: f32, alpha: f32) {
    match race_by_key(key) {
        Some(race) => {
            let at = Rect::new(centre.x - r, centre.y - r, r * 2.0, r * 2.0);
            emblem::draw(ui, race, Mark::Badge, at, race.tint(alpha));
        }
        None => {
            let tint: Color = super::rgb(super::palette::DIM, alpha);
            ui.arc(centre, r, 0.0, std::f32::consts::TAU, (r * 0.16).clamp(1.2, 2.6), tint);
            let initial: String = key.chars().take(1).collect::<String>().to_uppercase();
            ui.text_centred(centre.x, centre.y, super::type_scale::MICRO, tint, &initial);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_race_on_its_own_roster_comes_first_and_the_naga_are_playable() {
        let all = races();
        assert!(all[0].stand_in.is_none());
        let naga = race_by_key("naga").expect("the Naga are listed");
        assert_eq!(naga.borrowed_roster(), Some("ARC"));
        assert_eq!(race_key(200), all[0].key);
    }

    #[test]
    fn arc_has_a_codex_and_every_mark_it_lists() {
        let arc = race_by_key("aster").expect("ARC is listed");
        let art = arc.codex.art.expect("ARC wears the eagle");
        assert!(!arc.codex.motto.is_empty() && !arc.codex.about.is_empty());
        assert!(arc.codex.marks.iter().all(|m| art.has(*m)));
    }
}
