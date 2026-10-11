//! The box that names a prefab being saved, or edits one of the library: its plan,
//! what is in it and what it costs, the shelf it goes on and its icon (each lit on
//! the one its structures make until the player picks another), a name to type (the
//! suggestion selected, so typing replaces it), and Save. Enter saves, Escape or
//! Cancel puts it away. While it is up it has the keyboard and the screen.

use super::{plan, shelf};
use crate::audio::Sfx;
use crate::hud::icons;
use crate::hud::{whole, Hud, HudAction, Scene, ENERGY, MATERIALS};
use crate::prefabs::{self, Badge, Category, Prefab};
use crate::ui::{id, ink, palette, rgb, type_scale, ButtonKind, Key, Rect, Ui};
use glam::Vec2;

const W: f32 = 520.0;
const H: f32 = 410.0;
/// Most kinds of structure listed; the rest are counted on one line.
const LISTED: usize = 5;
/// The pickers' rows: where their labels end and their chips begin.
const PICK_X: f32 = 56.0;

pub(super) struct Draft {
    prefab: Prefab,
    name: String,
    /// It is in the library already: saving puts it back in its place.
    editing: bool,
    /// The field has not had the keyboard yet: it takes it, the name selected.
    fresh: bool,
}

impl Draft {
    pub(super) fn new(prefab: Prefab, editing: bool) -> Draft {
        Draft {
            name: prefab.name.clone(),
            prefab,
            editing,
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
    ui.section(
        r.x + 18.0,
        r.y + 22.0,
        W - 36.0,
        if draft.editing {
            "Edit Prefab"
        } else {
            "Save Prefab"
        },
    );

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

    pickers(hud, ui, s, &mut draft.prefab, r);

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
    let namesake = s.prefabs.by_name(&name).filter(|p| p.id != draft.prefab.id);
    let replaces = !draft.editing && namesake.is_some();
    let clash = draft.editing && namesake.is_some();
    let full = !draft.editing && !replaces && s.prefabs.len() >= prefabs::MAX_PREFABS;
    let (note, tone) = if clash {
        (
            format!("Another prefab is called {name}: name this one otherwise"),
            palette::BAD,
        )
    } else if draft.editing {
        (
            "Its structures stay as saved  \u{b7}  save a new layout to change them".to_owned(),
            palette::FAINT,
        )
    } else if full {
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
    let can = !full && !clash;
    let saved = ui.button(id("prefab-save", 0), save, label, ButtonKind::Primary, can)
        || (can && ui.input.key(Key::Enter));
    hud.claim(ui, screen);
    if saved {
        ui.audio.play(Sfx::Select);
        let mut prefab = draft.prefab.clone();
        prefab.name = name;
        hud.actions.push(if draft.editing {
            HudAction::EditPrefab(prefab)
        } else {
            HudAction::KeepPrefab(prefab)
        });
    } else if cancelled {
        ui.audio.play(Sfx::Back);
    }
    if saved || cancelled {
        ui.mem.editing = None;
    } else {
        hud.prefabs.saving = Some(draft);
    }
}

/// The shelf and icon rows: a chip for each, the one the prefab has lit. A pick is
/// kept only when it differs from what its structures make, so `None` stays "its own".
fn pickers(hud: &mut Hud, ui: &mut Ui, s: &Scene, prefab: &mut Prefab, r: Rect) {
    let (x, y) = (r.x + 18.0, r.y + 246.0);
    let row_w = W - 36.0 - PICK_X;
    let own = Prefab {
        category: None,
        icon: None,
        ..prefab.clone()
    };
    for (i, label) in ["Shelf", "Icon"].into_iter().enumerate() {
        ui.text(
            x,
            y + 13.0 + i as f32 * 44.0,
            type_scale::MICRO,
            rgb(palette::DIM, 1.0),
            label,
        );
    }
    let now = prefab.category(s.blueprints);
    let n = Category::ALL.len() as f32;
    let cw = (row_w - (n - 1.0) * 4.0) / n;
    for (i, c) in Category::ALL.into_iter().enumerate() {
        let cr = Rect::new(x + PICK_X + i as f32 * (cw + 4.0), y, cw, 26.0);
        let t = hud.tile(ui, id("prefab-pick-shelf", i), cr, c == now, true);
        ui.text_centred(
            cr.x + cr.w * 0.5,
            cr.mid_y(),
            type_scale::MICRO,
            rgb(
                if c == now { 0xFFFFFF } else { palette::TEXT },
                if c == now { 1.0 } else { 0.65 + 0.35 * t.glow },
            ),
            shelf(c).label(true),
        );
        if t.clicked {
            ui.audio.play(Sfx::Tick);
            prefab.category = (c != own.category(s.blueprints)).then_some(c);
        }
    }
    let now = prefab.badge(s.blueprints);
    let n = Badge::ALL.len() as f32;
    let bw = ((row_w - (n - 1.0) * 4.0) / n).min(30.0);
    for (i, b) in Badge::ALL.into_iter().enumerate() {
        let br = Rect::new(x + PICK_X + i as f32 * (bw + 4.0), y + 44.0, bw, 28.0);
        let t = hud.tile(ui, id("prefab-pick-icon", i), br, b == now, true);
        icons::strategic(
            ui,
            b.icon(),
            0,
            Vec2::new(br.x + br.w * 0.5, br.mid_y()),
            8.0,
            rgb(
                if b == now { 0xFFFFFF } else { palette::TEXT },
                if b == now { 1.0 } else { 0.6 + 0.4 * t.glow },
            ),
            ink(0.62),
        );
        if t.clicked {
            ui.audio.play(Sfx::Tick);
            prefab.icon = (b != own.badge(s.blueprints)).then_some(b);
        }
    }
}

/// The corners of the ground a prefab's lots cover, metres from its middle.
fn lot_extent(s: &Scene, prefab: &Prefab) -> ([f32; 2], [f32; 2]) {
    let (members, _) = prefab.resolve(s.blueprints);
    let sites: Vec<_> = members
        .iter()
        .map(|m| prefabs::Laid {
            blueprint: m.blueprint,
            at: mc_core::FxVec2::from_ints(m.x, m.y),
            heading: m.heading(s.blueprints),
        })
        .collect();
    prefabs::bounds(s.blueprints, &sites).unwrap_or(([0.0; 2], [0.0; 2]))
}
