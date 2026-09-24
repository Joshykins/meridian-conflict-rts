//! Searchable catalog shared by the range subject and placement controls.
use super::{icons, Hud, HudAction, Scene};
use crate::range::RangeAction;
use crate::ui::{id, ink, palette, rgb, type_scale, ButtonKind, Key, Rect, Ui};
use glam::Vec2;
use mc_data::{cat, UnitBlueprint};

#[derive(Clone, Copy)]
pub(super) enum Target {
    Subject,
    Spawn,
}

pub(super) struct Picker {
    target: Target,
    query: String,
    category: usize,
    tech: u8,
    active: usize,
}

const CATEGORIES: [(&str, u32); 7] = [
    ("All", 0),
    ("Land", cat::LAND),
    ("Air", cat::AIR),
    ("Naval", cat::NAVAL),
    ("Space", cat::SPACE),
    ("Structures", cat::STRUCTURE),
    ("Builders", cat::ENGINEER | cat::COMMANDER | cat::FACTORY),
];

impl Picker {
    pub(super) fn new(target: Target) -> Self {
        Self {
            target,
            query: String::new(),
            category: 0,
            tech: 0,
            active: 0,
        }
    }

    fn matches(&self, bp: &UnitBlueprint) -> bool {
        let mask = CATEGORIES[self.category].1;
        if mask != 0 && bp.categories & mask == 0 {
            return false;
        }
        // LAND is the mobile roster; structures have their own tab.
        if self.category == 1 && !bp.is_mobile() {
            return false;
        }
        if self.tech != 0 && self.tech != bp.tech {
            return false;
        }
        let text = format!(
            "{} {} {} t{} {}",
            bp.name,
            bp.role,
            bp.key,
            bp.tech,
            CATEGORIES
                .iter()
                .filter(|(_, m)| *m != 0 && bp.categories & m != 0)
                .map(|(name, _)| *name)
                .collect::<Vec<_>>()
                .join(" ")
        )
        .to_lowercase();
        self.query
            .to_lowercase()
            .split_whitespace()
            .all(|word| text.contains(word))
    }
}

/// Ellipsize against the actual font metrics so long roles never cross cards.
pub(super) fn fitted(ui: &mut Ui, text: &str, width: f32, style: crate::ui::Style) -> String {
    if ui.text_width(style, text) <= width {
        return text.to_owned();
    }
    let mut out = text.to_owned();
    while !out.is_empty() && ui.text_width(style, &format!("{out}...")) > width {
        out.pop();
    }
    format!("{out}...")
}

pub(super) fn draw(hud: &mut Hud, ui: &mut Ui, s: &Scene) {
    let Some(mut picker) = hud.unit_picker.take() else {
        return;
    };
    let Some(range) = s.view.range.as_ref() else {
        return;
    };
    let full = Rect::new(0.0, 0.0, ui.size.x, ui.size.y);
    hud.claim(ui, full);
    ui.fill(full, ink(0.72));
    let w = (ui.size.x - 48.0).clamp(320.0, 1220.0);
    let h = (ui.size.y - 64.0).min(920.0);
    let panel = Rect::new((ui.size.x - w) * 0.5, (ui.size.y - h) * 0.5, w, h);
    hud.glass(ui, panel);
    let x = panel.x + 24.0;
    let cw = panel.w - 48.0;
    let selected = match picker.target {
        Target::Subject => range.subject,
        Target::Spawn => range.spawn,
    };
    let title = match picker.target {
        Target::Subject => "Choose Test Unit",
        Target::Spawn => "Choose Spawn Unit",
    };
    ui.text(
        x,
        panel.y + 34.0,
        type_scale::OVERLINE,
        rgb(palette::ACCENT, 1.0),
        title,
    );
    if ui.button(
        id("unit-close", 0),
        Rect::new(panel.right() - 168.0, panel.y + 18.0, 144.0, 32.0),
        "Close  ESC",
        ButtonKind::Secondary,
        true,
    ) || ui.input.key(Key::Escape)
    {
        ui.mem.editing = None;
        return;
    }
    ui.text(
        x,
        panel.y + 64.0,
        type_scale::BODY,
        rgb(palette::DIM, 1.0),
        match picker.target {
            Target::Subject => "Pick a unit to test. Changing the subject resets the range.",
            Target::Spawn => "Pick a unit to place. Your current range stays as it is.",
        },
    );
    let search = Rect::new(x, panel.y + 88.0, cw - 202.0, 42.0);
    let mut changed = ui.text_field(id("unit-search", 0), search, &mut picker.query, 48);
    if picker.query.is_empty() {
        ui.text(
            search.x + 16.0,
            search.mid_y(),
            type_scale::BODY,
            rgb(palette::DIM, 0.75),
            "Type a name or role...",
        );
    }
    if ui.button(
        id("unit-clear", 0),
        Rect::new(search.right() + 10.0, search.y, 192.0, search.h),
        "Clear Filters",
        ButtonKind::Secondary,
        true,
    ) {
        picker.query.clear();
        picker.category = 0;
        picker.tech = 0;
        changed = true;
    }
    let cat_columns = ((cw + 6.0) / 126.0).floor().max(1.0) as usize;
    let cat_columns = cat_columns.min(CATEGORIES.len());
    let cat_w = (cw - (cat_columns - 1) as f32 * 6.0) / cat_columns as f32;
    for (i, (label, _)) in CATEGORIES.iter().enumerate() {
        let r = Rect::new(
            x + (i % cat_columns) as f32 * (cat_w + 6.0),
            panel.y + 146.0 + (i / cat_columns) as f32 * 42.0,
            cat_w,
            32.0,
        );
        if super::range::word_tile(
            hud,
            ui,
            id("unit-category", i),
            r,
            label,
            picker.category == i,
            true,
            palette::ACCENT,
        ) {
            picker.category = i;
            changed = true;
        }
    }
    let tech_y = panel.y + 146.0 + CATEGORIES.len().div_ceil(cat_columns) as f32 * 42.0;
    let tech_columns = ((cw + 6.0) / 86.0).floor().max(1.0) as usize;
    for tier in 0..=5 {
        let label = if tier == 0 {
            "All Tech".to_owned()
        } else {
            format!("T{tier}")
        };
        let r = Rect::new(
            x + (tier as usize % tech_columns) as f32 * 86.0,
            tech_y + (tier as usize / tech_columns) as f32 * 40.0,
            80.0,
            30.0,
        );
        if super::range::word_tile(
            hud,
            ui,
            id("unit-tech", tier as usize),
            r,
            &label,
            picker.tech == tier,
            true,
            palette::ACCENT,
        ) {
            picker.tech = tier;
            changed = true;
        }
    }
    let mut results: Vec<_> = s
        .blueprints
        .units
        .iter()
        .filter(|bp| s.blueprints.is_listed(bp.id) && picker.matches(bp))
        .collect();
    results.sort_by(|a, b| (a.tech, &a.name, &a.key).cmp(&(b.tech, &b.name, &b.key)));
    let results_y = tech_y + 6_usize.div_ceil(tech_columns) as f32 * 40.0 + 8.0;
    ui.text_right(
        panel.right() - 24.0,
        results_y,
        type_scale::CAPTION,
        rgb(palette::DIM, 1.0),
        &format!(
            "{} Of {} Units",
            results.len(),
            s.blueprints.units.iter().filter(|b| s.blueprints.is_listed(b.id)).count()
        ),
    );

    let columns = ((cw + 10.0) / 280.0).floor().max(1.0) as usize;
    let cards_y = results_y + 24.0;
    let rows = ((panel.bottom() - 74.0 - cards_y) / 88.0).floor().max(1.0) as usize;
    let per_page = rows * columns;
    let last = results.len().saturating_sub(1);
    if changed {
        picker.active = 0;
    }
    picker.active = picker.active.min(last);
    for (key, delta) in [
        (Key::Left, -1),
        (Key::Right, 1),
        (Key::Up, -(columns as isize)),
        (Key::Down, columns as isize),
    ] {
        if ui.input.key(key) {
            picker.active = picker.active.saturating_add_signed(delta).min(last);
        }
    }
    if ui.input.scroll != 0.0 {
        let delta = if ui.input.scroll < 0.0 {
            per_page as isize
        } else {
            -(per_page as isize)
        };
        picker.active = picker.active.saturating_add_signed(delta).min(last);
    }
    let mut page = picker.active / per_page;
    let pages = results.len().div_ceil(per_page).max(1);
    let footer_y = panel.bottom() - 62.0;
    if ui.button(
        id("unit-prev", 0),
        Rect::new(x, footer_y, 120.0, 32.0),
        "< Prev",
        ButtonKind::Secondary,
        page > 0,
    ) {
        page -= 1;
        picker.active = page * per_page;
    }
    if ui.button(
        id("unit-next", 0),
        Rect::new(panel.right() - 144.0, footer_y, 120.0, 32.0),
        "Next >",
        ButtonKind::Secondary,
        page + 1 < pages,
    ) {
        page += 1;
        picker.active = page * per_page;
    }
    ui.text_centred(
        panel.x + w * 0.5,
        footer_y + 16.0,
        type_scale::CAPTION,
        rgb(palette::DIM, 1.0),
        &format!(
            "Page {} / {}  ·  arrows to browse  ·  enter to pick",
            page + 1,
            pages
        ),
    );
    let card_w = (cw - (columns - 1) as f32 * 10.0) / columns as f32;
    let mut picked = None;
    for (i, bp) in results
        .iter()
        .enumerate()
        .skip(page * per_page)
        .take(per_page)
    {
        let cell = i - page * per_page;
        let r = Rect::new(
            x + (cell % columns) as f32 * (card_w + 10.0),
            cards_y + (cell / columns) as f32 * 88.0,
            card_w,
            78.0,
        );
        let tile = hud.tile(
            ui,
            id("unit-card", bp.id.0 as usize),
            r,
            bp.id == selected,
            true,
        );
        if i == picker.active {
            ui.frame(r.inset(2.0), rgb(palette::ACCENT, 0.8));
        }
        icons::strategic(
            ui,
            bp.visual.icon,
            bp.tech,
            Vec2::new(r.x + 28.0, r.mid_y()),
            16.0,
            rgb(palette::TEXT, 1.0),
            ink(0.95),
        );
        let name = fitted(ui, &bp.name, card_w - 76.0, type_scale::BODY);
        ui.text(
            r.x + 58.0,
            r.y + 20.0,
            type_scale::BODY,
            rgb(palette::TEXT, 1.0),
            &name,
        );
        let role = fitted(ui, &bp.role, card_w - 76.0, type_scale::CAPTION);
        ui.text(
            r.x + 58.0,
            r.y + 41.0,
            type_scale::CAPTION,
            rgb(palette::DIM, 1.0),
            &role,
        );
        ui.text(
            r.x + 58.0,
            r.y + 62.0,
            type_scale::MICRO,
            rgb(palette::ACCENT, 0.9),
            &format!(
                "T{}{}",
                bp.tech,
                if bp.id == selected {
                    "  ·  Current"
                } else {
                    ""
                }
            ),
        );
        if tile.clicked {
            picked = Some(bp.id);
        }
    }
    if results.is_empty() {
        ui.text_centred(
            panel.x + w * 0.5,
            cards_y + 74.0,
            type_scale::ITEM,
            rgb(palette::TEXT, 1.0),
            "No Matching Units",
        );
        ui.text_centred(
            panel.x + w * 0.5,
            cards_y + 110.0,
            type_scale::BODY,
            rgb(palette::DIM, 1.0),
            "Try a shorter name or clear the filters.",
        );
    }
    if ui.input.key(Key::Enter) {
        picked = results.get(picker.active).map(|bp| bp.id);
    }
    if let Some(bp) = picked {
        hud.actions.push(HudAction::Range(match picker.target {
            Target::Subject => RangeAction::PickSubject(bp),
            Target::Spawn => RangeAction::PickSpawn(bp),
        }));
        ui.audio.play(crate::audio::Sfx::Select);
        ui.mem.editing = None;
    } else {
        // Typing always searches, even after using a filter or paging button.
        ui.mem.editing = Some(id("unit-search", 0));
        hud.unit_picker = Some(picker);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn search_intersects_role_category_and_tech() {
        let bp = mc_data::Blueprints::load(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"),
        )
        .unwrap();
        let mut picker = Picker::new(Target::Subject);
        assert!(bp.units.iter().all(|b| picker.matches(b)));
        picker.query = "  TANK   t1 ".into();
        let found: Vec<_> = bp.units.iter().filter(|b| picker.matches(b)).collect();
        assert!(!found.is_empty());
        assert!(found
            .iter()
            .all(|b| b.tech == 1 && b.role.to_lowercase().contains("tank")));
        picker.tech = 2;
        assert!(!bp.units.iter().any(|b| picker.matches(b)));
        picker.query.clear();
        picker.category = 2;
        let found: Vec<_> = bp.units.iter().filter(|b| picker.matches(b)).collect();
        assert!(!found.is_empty());
        assert!(found.iter().all(|b| b.tech == 2 && b.has(cat::AIR)));
        picker.query = "no such unit".into();
        assert!(!bp.units.iter().any(|b| picker.matches(b)));
    }
}
