//! Faction marks: each faction's crest and its variations, drawn as vector
//! art and rasterised at exactly the size they are shown (see
//! `Overlay::sprite`), so a crest is as sharp in a 16-point list row as it is
//! filling the race card.
//!
//! Which art a faction wears, and which of its marks exist, is data: the
//! `art` and `marks` of its `codex.ron`.

mod eagle;
pub(crate) mod monogram;
mod paint;
mod serpent;

use super::faction::Race;
use super::{palette, rgb, type_scale, Color, Rect, Ui};
use glam::Vec2;
use std::hash::{Hash, Hasher};

/// A set of drawings a faction's marks are made from.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, serde::Deserialize)]
pub enum Art {
    /// The eagle over a shield: the Asterian Reach Command.
    Eagle,
    /// A hooded serpent with a burning eye: the Naga, as ARC draws them.
    Serpent,
}

/// One of a faction's marks.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, serde::Deserialize)]
pub enum Mark {
    /// The full achievement: everything, in colour, with the name.
    Crest,
    /// The crest reduced to its core, for flat and small uses.
    Insignia,
    /// The smallest form, white for tinting: list rows and seat markers.
    Badge,
    /// The name as type.
    Wordmark,
    /// Type cut for a spray stencil, as on hulls and vehicles.
    Stencil,
    /// A round seal with the name and motto.
    Seal,
}

impl Mark {
    pub fn label(self) -> &'static str {
        match self {
            Mark::Crest => "Crest",
            Mark::Insignia => "Insignia",
            Mark::Badge => "Badge",
            Mark::Wordmark => "Wordmark",
            Mark::Stencil => "Stencil",
            Mark::Seal => "Seal",
        }
    }

    /// Width over height of the design.
    fn aspect(self) -> f32 {
        match self {
            Mark::Insignia => 1000.0 / 840.0,
            Mark::Wordmark => 1000.0 / 420.0,
            Mark::Stencil => 1000.0 / 340.0,
            Mark::Crest | Mark::Badge | Mark::Seal => 1.0,
        }
    }

    /// Drawn in white, to be tinted (the others carry their own colour).
    fn mono(self) -> bool {
        matches!(self, Mark::Badge | Mark::Stencil)
    }
}

/// The words a mark is lettered with.
pub struct Words {
    pub name: String,
    pub motto: String,
}

impl Art {
    /// Whether this art has `mark`.
    pub fn has(self, mark: Mark) -> bool {
        match self {
            Art::Eagle => true,
            Art::Serpent => matches!(mark, Mark::Crest | Mark::Badge),
        }
    }

    fn render(self, mark: Mark, size: [usize; 2], words: &Words) -> Vec<u8> {
        match (self, mark) {
            (Art::Eagle, Mark::Crest) => eagle::crest(size, words),
            (Art::Eagle, Mark::Insignia) => eagle::insignia(size),
            (Art::Eagle, Mark::Badge) => eagle::badge(size),
            (Art::Eagle, Mark::Wordmark) => eagle::wordmark(size, words),
            (Art::Eagle, Mark::Stencil) => eagle::stencil(size),
            (Art::Eagle, Mark::Seal) => eagle::seal(size, words),
            (Art::Serpent, Mark::Badge) => serpent::badge(size),
            (Art::Serpent, _) => serpent::crest(size),
        }
    }
}

/// The part of `r` a mark of `aspect` fills, centred.
fn fit(r: Rect, aspect: f32) -> Rect {
    let (w, h) = if r.w / r.h.max(1e-3) > aspect {
        (r.h * aspect, r.h)
    } else {
        (r.w, r.w / aspect)
    };
    Rect::new(r.x + (r.w - w) * 0.5, r.y + (r.h - h) * 0.5, w, h)
}

/// Draws `race`'s `mark` fitted into `r`. Coloured marks keep their colours
/// (`tint`'s alpha fades them); white ones take `tint`. A race with no art,
/// or without that mark, gets a ring with its initial.
pub fn draw(ui: &mut Ui, race: &Race, mark: Mark, r: Rect, tint: Color) {
    let Some(art) = race.codex.art.filter(|a| a.has(mark)) else {
        let c = Vec2::new(r.x + r.w * 0.5, r.mid_y());
        let radius = r.w.min(r.h) * 0.45;
        ui.arc(
            c,
            radius,
            0.0,
            std::f32::consts::TAU,
            (radius * 0.16).clamp(1.2, 2.6),
            tint,
        );
        let initial: String = race.name.chars().take(1).collect::<String>().to_uppercase();
        ui.text_centred(c.x, c.y, type_scale::MICRO, tint, &initial);
        return;
    };
    let at = fit(r, mark.aspect());
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (race.key.as_str(), art, mark).hash(&mut h);
    let words = Words {
        name: race.name.clone(),
        motto: race.codex.motto.clone(),
    };
    let tint = if mark.mono() {
        tint
    } else {
        [1.0, 1.0, 1.0, tint[3]]
    };
    ui.sprite(h.finish(), at, tint, |size| art.render(mark, size, &words));
}

/// A mark for a choice that is not a race yet: an outlined shield with a
/// question in it (the random pick).
pub fn unknown(ui: &mut Ui, r: Rect, alpha: f32) {
    let s = r.w.min(r.h);
    let c = Vec2::new(r.x + r.w * 0.5, r.mid_y());
    let p = |x: f32, y: f32| c + Vec2::new(x, y) * s;
    let line = rgb(palette::DIM, alpha);
    let weight = (s * 0.035).clamp(1.2, 3.0);
    ui.polyline(
        &[
            p(-0.3, -0.36),
            p(0.3, -0.36),
            p(0.3, 0.02),
            p(0.2, 0.24),
            p(0.0, 0.4),
            p(-0.2, 0.24),
            p(-0.3, 0.02),
        ],
        weight,
        line,
        true,
    );
    let st = super::style(mc_render::Face::Light, (s * 0.42).max(9.0), 0.0);
    ui.text_centred(c.x, c.y - s * 0.02, st, rgb(palette::TEXT, alpha), "?");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Writes every mark of every art as PNGs, to look at while drawing them:
    /// `cargo test -p mc-game zz_emblem_sheet -- --ignored`, then open
    /// `<temp dir>/meridian-emblems/*.png`.
    #[test]
    #[ignore]
    fn zz_emblem_sheet() {
        let dir = std::env::temp_dir().join("meridian-emblems");
        std::fs::create_dir_all(&dir).unwrap();
        let words = Words {
            name: "Asterian Reach Command".into(),
            motto: "Hold the Reach".into(),
        };
        for art in [Art::Eagle, Art::Serpent] {
            for mark in [
                Mark::Crest,
                Mark::Insignia,
                Mark::Badge,
                Mark::Wordmark,
                Mark::Stencil,
                Mark::Seal,
            ] {
                if !art.has(mark) {
                    continue;
                }
                for px in [640usize, 96, 18] {
                    let size = [px, (px as f32 / mark.aspect()).round() as usize];
                    let rgba = art.render(mark, size, &words);
                    let path = dir.join(format!("{art:?}-{mark:?}-{px}.png").to_lowercase());
                    let file = std::fs::File::create(&path).unwrap();
                    let mut enc = png::Encoder::new(
                        std::io::BufWriter::new(file),
                        size[0] as u32,
                        size[1] as u32,
                    );
                    enc.set_color(png::ColorType::Rgba);
                    enc.set_depth(png::BitDepth::Eight);
                    enc.write_header().unwrap().write_image_data(&rgba).unwrap();
                }
            }
        }
        println!("wrote {}", dir.display());
    }

    /// Redraws the program icon, `assets/meridian.ico` (built into the
    /// Windows exe by `build.rs`), from `monogram.rs`, and writes each size as
    /// a PNG to look at: `cargo test -p mc-game zz_write_app_icon -- --ignored`,
    /// then open `<temp dir>/meridian-icon/*.png`.
    #[test]
    #[ignore]
    fn zz_write_app_icon() {
        let dir = std::env::temp_dir().join("meridian-icon");
        std::fs::create_dir_all(&dir).unwrap();
        for px in monogram::SIZES {
            let file = std::fs::File::create(dir.join(format!("icon-{px}.png"))).unwrap();
            let mut enc = png::Encoder::new(std::io::BufWriter::new(file), px as u32, px as u32);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            let rgba = monogram::icon(px);
            enc.write_header().unwrap().write_image_data(&rgba).unwrap();
        }
        let ico = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/meridian.ico");
        std::fs::write(&ico, monogram::ico()).unwrap();
        println!("wrote {} and {}", ico.display(), dir.display());
    }

    /// The checked-in icon holds every size Windows asks for.
    #[test]
    fn the_program_icon_has_every_size() {
        let ico = include_bytes!("../../../assets/meridian.ico");
        assert_eq!(&ico[..4], &[0, 0, 1, 0], "an icon file");
        let count = u16::from_le_bytes([ico[4], ico[5]]) as usize;
        let sides: Vec<usize> = (0..count)
            .map(|i| match ico[6 + 16 * i] {
                0 => 256,
                side => side as usize,
            })
            .collect();
        assert_eq!(sides, monogram::SIZES);
    }

    #[test]
    fn every_mark_fills_the_size_it_was_asked_for() {
        let words = Words {
            name: "Asterian Reach Command".into(),
            motto: "Hold the Reach".into(),
        };
        for art in [Art::Eagle, Art::Serpent] {
            for mark in [
                Mark::Crest,
                Mark::Insignia,
                Mark::Badge,
                Mark::Wordmark,
                Mark::Stencil,
                Mark::Seal,
            ] {
                if art.has(mark) {
                    let rgba = art.render(mark, [40, 30], &words);
                    assert_eq!(rgba.len(), 40 * 30 * 4, "{art:?} {mark:?}");
                    assert!(
                        rgba.as_chunks::<4>().0.iter().any(|p| p[3] > 0),
                        "{art:?} {mark:?} drew something"
                    );
                }
            }
        }
    }
}
