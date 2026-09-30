//! Placing a radar post: the side's radar cover on the map.
//!
//! The renderer draws the cover as one merged outline (`rings::radar_network` puts every
//! standing post's ring in the radar group, beside the new site's). Over it this draws
//! each post's own reach, faint and dashed, so what overlaps shows; posts still being
//! built, dashed amber; and a card at the site saying how much ground it adds.

use crate::nuke_marks::{ground_ring, new_ground, project, surface, tag};
use crate::orders::Field;
use crate::rings::{radar_post, Reach};
use crate::ui::{palette, rgb, type_scale, Rect, Ui};
use glam::Vec2;
use mc_data::BlueprintId;
use mc_sim::mirror::{KIND_GHOST, KIND_PROP, KIND_WRECK};

/// Construction amber, for posts still being built.
const BUILDING: u32 = 0xFFA928;

/// The side's radar posts and the card, if a radar post is being placed at `placing`.
pub fn draw(ui: &mut Ui, field: &Field, placing: Option<(BlueprintId, Vec2)>) {
    let Some((reach, site)) =
        placing.and_then(|(bp, at)| Some((radar_post(field.blueprints.unit(bp))?, at)))
    else {
        return;
    };
    let view = field.view;
    let tone = Reach::Radar.tone();
    let t = ui.time;
    let (mut standing, mut building) = (Vec::new(), 0);
    for u in &view.frame.units {
        if u.owner_flags & (KIND_WRECK | KIND_PROP | KIND_GHOST) != 0
            || (u.owner_flags & 0xFF) as u8 != view.local
        {
            continue;
        }
        let Some(r) = radar_post(field.blueprints.unit(BlueprintId(u.blueprint as u16))) else {
            continue;
        };
        let at = Vec2::new(u.pos[0], u.pos[1]);
        if u.build >= 1.0 {
            ground_ring(ui, field, at, r, 1.6, rgb(0x9DB6FF, 0.6), true, 0.0);
            standing.push((at, r));
        } else {
            // Its cover to come, apart from the network until it stands.
            ground_ring(ui, field, at, r, 1.2, rgb(BUILDING, 0.45), true, t * 0.02);
            if let Some(g) = project(ui, field, at.extend(surface(field, at) + 20.0)) {
                tag(
                    ui,
                    g - Vec2::new(0.0, 14.0),
                    &format!("Building  \u{b7}  {:.0}%", u.build * 100.0),
                    BUILDING,
                );
            }
            building += 1;
        }
    }
    let Some(g) = project(ui, field, site.extend(surface(field, site) + 2.0)) else {
        return;
    };
    card(
        ui,
        g,
        reach,
        &standing,
        building,
        new_ground(site, reach, &standing),
    );
}

/// The network's card beside `anchor`: posts standing, and what the new one adds.
fn card(
    ui: &mut Ui,
    anchor: Vec2,
    reach: f32,
    standing: &[(Vec2, f32)],
    building: usize,
    fresh: f32,
) {
    let tone = Reach::Radar.tone();
    let r = Rect::new(anchor.x + 26.0, anchor.y - 34.0, 236.0, 72.0);
    ui.frost(r, 0.72);
    ui.fill(Rect::new(r.x, r.y, 2.0, r.h), rgb(tone, 1.0));
    let x = r.x + 12.0;
    ui.text(
        x,
        r.y + 14.0,
        type_scale::MICRO,
        rgb(palette::TEXT, 0.95),
        "Radar network",
    );
    let posts = match standing.len() {
        0 => "No radar standing".to_owned(),
        1 => "1 radar".to_owned(),
        n => format!("{n} radars"),
    };
    ui.text_right(
        r.right() - 10.0,
        r.y + 14.0,
        type_scale::MICRO,
        rgb(palette::TEXT, 0.7),
        &posts,
    );
    let (text, color) = if standing.is_empty() {
        ("First radar cover".to_owned(), palette::TEXT)
    } else if fresh < 0.02 {
        ("No new ground".to_owned(), palette::WARN)
    } else {
        (format!("+{:.0}% new ground", fresh * 100.0), palette::TEXT)
    };
    ui.text(x, r.y + 38.0, type_scale::VALUE, rgb(color, 1.0), &text);
    let mut detail = format!("Reach {reach:.0} m");
    if building > 0 {
        detail += &format!("  \u{b7}  {building} building");
    }
    ui.text(
        x,
        r.y + 60.0,
        type_scale::MICRO,
        rgb(palette::TEXT, 0.7),
        &detail,
    );
}
