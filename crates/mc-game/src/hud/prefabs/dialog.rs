//! The box that names a prefab being saved: its plan, what is in it and what it
//! costs, a name to type (the suggestion selected, so typing replaces it), and Save.
//! Enter saves, Escape or Cancel puts it away. While it is up it has the keyboard
//! and the screen.

use super::plan;
use crate::audio::Sfx;
use crate::hud::{whole, Hud, HudAction, Scene, ENERGY, MATERIALS};
use crate::prefabs::{self, Prefab};
use crate::ui::{id, ink, palette, rgb, type_scale, ButtonKind, Key, Rect, Ui};

const W: f32 = 520.0;
const H: f32 = 330.0;
/// Most kinds of structure listed; the rest are counted on one line.
const LISTED: usize = 5;

pub(super) struct Draft {
    prefab: Prefab,
    name: String,
    /// The field has not had the keyboard yet: it takes it, the name selected.
    fresh: bool,
}

impl Draft {
    pub(super) fn new(prefab: Prefab) -> Draft {
        Draft {
            name: prefab.name.clone(),
            prefab,
            fresh: true,
        }
    }
}

pub(super) fn draw(hud: &mut Hud, ui: &mut Ui, s: &Scene) {
    let Some(mut draft) = hud.prefabs.saving.take() else {
        return;
    };
    let (sw, sh) = (ui.size.x, ui.size.y);
    // The whole screen is the box's while it is up: a click off it does nothing.
    let screen = Rect::new(0.0, 0.0, sw, sh);
    ui.fill(screen, ink(0.35));
    let r = Rect::new(((sw - W) * 0.5).round(), ((sh - H) * 0.42).round(), W, H);
    ui.panel(r);
    ui.section(r.x + 18.0, r.y + 22.0, W - 36.0, "Save Prefab");

    // The plan, left.
    let well = Rect::new(r.x + 18.0, r.y + 44.0, 170.0, 170.0);
    ui.fill(well, ink(0.6));
    ui.frame(well, rgb(palette::LINE, 0.14));
    plan(ui, s.blueprints, &draft.prefab, well.inset(6.0), None, 0.0);
    let (lo, hi) = lot_extent(s, &draft.prefab);
    ui.text_centred(
        well.x + well.w * 0.5,
        well.bottom() + 14.0,
        type_scale::MICRO,
        rgb(palette::FAINT, 1.0),
        &format!("{:.0} \u{d7} {:.0} m", hi[0] - lo[0], hi[1] - lo[1]),
    );

    // What is in it, and what it costs, right.
    let x = well.right() + 20.0;
    let cw = r.right() - 18.0 - x;
    let census = draft.prefab.census(s.blueprints);
    let count: usize = census.iter().map(|(_, n)| n).sum();
    ui.text(
        x,
        r.y + 52.0,
        type_scale::MICRO,
        rgb(palette::DIM, 1.0),
        &format!("{count} structures"),
    );
    let mut y = r.y + 72.0;
    for &(b, n) in census.iter().take(LISTED) {
        let bp = s.blueprints.unit(b);
        ui.fill(
            Rect::new(x, y - 5.0, 10.0, 10.0),
            rgb(crate::hud::style::Role::of(bp).tone(), 0.85),
        );
        ui.text_fit_left(
            x + 18.0,
            y,
            cw - 52.0,
            type_scale::VALUE,
            rgb(palette::TEXT, 1.0),
            &bp.name,
        );
        ui.text_right(
            x + cw,
            y,
            type_scale::VALUE,
            rgb(palette::DIM, 1.0),
            &format!("\u{d7}{n}"),
        );
        y += 19.0;
    }
    if census.len() > LISTED {
        let more: usize = census[LISTED..].iter().map(|(_, n)| n).sum();
        ui.text(
            x + 18.0,
            y,
            type_scale::MICRO,
            rgb(palette::FAINT, 1.0),
            &format!("and {more} more"),
        );
    }
    let (materials, energy) = draft.prefab.cost(s.blueprints);
    let cy = r.y + 192.0;
    for (i, (label, value, tone)) in [
        ("Materials", whole(materials), MATERIALS),
        ("Energy", whole(energy), ENERGY),
    ]
    .into_iter()
    .enumerate()
    {
        let cx = x + i as f32 * cw * 0.5;
        ui.fill(Rect::new(cx, cy - 4.0, 2.0, 28.0), rgb(tone, 0.9));
        ui.text(
            cx + 10.0,
            cy + 2.0,
            type_scale::MICRO,
            rgb(palette::FAINT, 1.0),
            label,
        );
        ui.text(
            cx + 10.0,
            cy + 18.0,
            type_scale::VALUE,
            rgb(tone, 1.0),
            &value,
        );
    }

    // The name, and what saving it under that name will do.
    let field_id = id("prefab-name", 0);
    if std::mem::take(&mut draft.fresh) {
        ui.focus_field_selected(field_id);
    }
    let fy = r.bottom() - 62.0;
    ui.text(
        r.x + 18.0,
        fy - 12.0,
        type_scale::MICRO,
        rgb(palette::DIM, 1.0),
        "Name",
    );
    let field = Rect::new(r.x + 18.0, fy, W - 36.0 - 2.0 * 104.0 - 16.0, 36.0);
    ui.text_field(field_id, field, &mut draft.name, prefabs::MAX_NAME);
    // The field keeps the keyboard while the box is up, whatever is clicked.
    if ui.mem.editing.is_none() {
        ui.mem.editing = Some(field_id);
    }
    let name = prefabs::clean_name(&draft.name);
    let replaces = s.prefabs.by_name(&name).is_some();
    let full = !replaces && s.prefabs.len() >= prefabs::MAX_PREFABS;
    let (note, tone) = if full {
        (
            format!(
                "The library is full ({}): delete a prefab first",
                prefabs::MAX_PREFABS
            ),
            palette::BAD,
        )
    } else if replaces {
        (format!("Replaces your prefab {name}"), palette::WARN)
    } else {
        (
            "Kept between matches  \u{b7}  Place it from a builder's Prefabs tab (B, then P)"
                .to_owned(),
            palette::FAINT,
        )
    };
    ui.text_fit_left(
        r.x + 18.0,
        r.bottom() - 13.0,
        W - 36.0,
        type_scale::MICRO,
        rgb(tone, 1.0),
        &note,
    );

    let cancel = Rect::new(r.right() - 18.0 - 2.0 * 104.0 - 8.0, fy, 104.0, 36.0);
    let save = Rect::new(r.right() - 18.0 - 104.0, fy, 104.0, 36.0);
    let cancelled = ui.button(
        id("prefab-cancel", 0),
        cancel,
        "Cancel",
        ButtonKind::Secondary,
        true,
    ) || ui.input.key(Key::Escape);
    let label = if replaces { "Replace" } else { "Save" };
    let saved = ui.button(
        id("prefab-save", 0),
        save,
        label,
        ButtonKind::Primary,
        !full,
    ) || (!full && ui.input.key(Key::Enter));
    hud.claim(ui, screen);
    if saved {
        ui.audio.play(Sfx::Select);
        let mut prefab = draft.prefab.clone();
        prefab.name = name;
        hud.actions.push(HudAction::KeepPrefab(prefab));
    } else if cancelled {
        ui.audio.play(Sfx::Back);
    }
    if saved || cancelled {
        ui.mem.editing = None;
    } else {
        hud.prefabs.saving = Some(draft);
    }
}

/// The corners of the ground a prefab's lots cover, metres from its middle.
fn lot_extent(s: &Scene, prefab: &Prefab) -> ([f32; 2], [f32; 2]) {
    let (members, _) = prefab.resolve(s.blueprints);
    let sites: Vec<_> = members
        .iter()
        .map(|m| (m.blueprint, mc_core::FxVec2::from_ints(m.x, m.y)))
        .collect();
    prefabs::bounds(s.blueprints, &sites).unwrap_or(([0.0; 2], [0.0; 2]))
}
