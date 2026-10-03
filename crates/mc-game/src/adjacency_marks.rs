//! Adjacency on the ground (`mc_sim::adjacency`, `hud::adjacency`).
//!
//! - The links of the buildings selected or under the pointer: a tag on each shared
//!   edge with what it saves, and an arrow across the seam from the provider into the
//!   building it saves; a bound pair's tag says so. The neighbour lit from the unit
//!   panel's band stands out.
//! - Placing a building: every link it would make with the side's finished buildings,
//!   tagged on its seam (the renderer draws its planned conduit), and a card at the
//!   site with what it would gain, what it would give, and whether it would be bound
//!   to a neighbour.

use crate::hud::adjacency::{self as adj, Tie, BOUND};
use crate::nuke_marks::{project, surface, tag};
use crate::orders::Field;
use crate::ui::{palette, rgb, type_scale, Rect, Ui};
use glam::Vec2;
use mc_core::{Fx, FxVec2};
use mc_data::BlueprintId;

/// Selected buildings whose links are tagged: past this many the tags would bury the
/// base (a cosmetic cap; the band still lists each one's own).
const MAX_TAGGED: usize = 24;

/// The tags for the selection and the pointer's building, and the placing site's links.
pub fn draw(
    ui: &mut Ui,
    field: &Field,
    hover: Option<u32>,
    lit: Option<u32>,
    placing: Option<(BlueprintId, Vec2)>,
) {
    let _t = mc_core::perf_span!("ui.adjacency_marks");
    let view = field.view;
    let focus: Vec<u32> = view
        .selection
        .iter()
        .copied()
        .take(MAX_TAGGED)
        .chain(hover)
        .collect();
    if !focus.is_empty() {
        for l in &view.frame.links {
            let mine = focus.contains(&l.consumer) || focus.contains(&l.provider);
            if !mine {
                continue;
            }
            let hot = lit.is_some_and(|p| p == l.provider || p == l.consumer);
            let [a, b] = l.edge.map(Vec2::from);
            let from = Vec2::from(l.from);
            let text = format!("{} {}", adj::percent(l.share), adj::word(l.resource));
            seam(
                ui,
                field,
                a,
                b,
                from,
                adj::tone(l.resource),
                hot,
                false,
                &text,
                l.bound,
            );
        }
    }
    if let Some((bp, site)) = placing {
        placing_marks(ui, field, bp, site);
    }
}

/// One link's seam from `a` to `b`: tagged at its middle, with an arrow across it away
/// from `from` (the provider's centre). `planned`: a site's link, dashed.
#[expect(
    clippy::too_many_arguments,
    reason = "one seam: its ends, its provider, its look and its words"
)]
fn seam(
    ui: &mut Ui,
    field: &Field,
    a: Vec2,
    b: Vec2,
    from: Vec2,
    tone: u32,
    hot: bool,
    planned: bool,
    text: &str,
    bound: bool,
) {
    let mid = (a + b) * 0.5;
    let lift = |p: Vec2| p.extend(surface(field, p) + 1.5);
    let (Some(pa), Some(pb), Some(pm)) = (
        project(ui, field, lift(a)),
        project(ui, field, lift(b)),
        project(ui, field, lift(mid)),
    ) else {
        return;
    };
    // A site's links: the renderer draws the planned conduit; the seam is underlined.
    let alpha = if hot { 1.0 } else { 0.8 };
    if planned {
        ui.stroke(pa, pb, 1.0, rgb(tone, 0.5));
    }
    // The arrow: from the provider's side of the seam into the other.
    let along = (b - a).normalize_or_zero();
    let mut across = Vec2::new(-along.y, along.x);
    if across.dot(mid - from) < 0.0 {
        across = -across;
    }
    let reach = 6.0 + if hot { 2.0 } else { 0.0 };
    if let (Some(t0), Some(t1)) = (
        project(ui, field, lift(mid - across * reach)),
        project(ui, field, lift(mid + across * reach)),
    ) {
        let c = rgb(tone, alpha);
        let w = if hot { 2.4 } else { 1.6 };
        ui.stroke(t0, t1, w, c);
        let d = (t1 - t0).normalize_or_zero();
        let n = Vec2::new(-d.y, d.x);
        ui.stroke(t1, t1 - d * 6.0 + n * 4.0, w, c);
        ui.stroke(t1, t1 - d * 6.0 - n * 4.0, w, c);
    }
    tag(ui, pm - Vec2::new(0.0, 16.0), text, tone);
    if bound {
        tag(ui, pm - Vec2::new(0.0, 36.0), "Bound", BOUND);
    }
}

/// The links a building placed at `site` would make, and the card that sums them up.
fn placing_marks(ui: &mut Ui, field: &Field, bp: BlueprintId, site: Vec2) {
    let view = field.view;
    let bp = field.blueprints.unit(bp);
    if !bp.is_structure() {
        return;
    }
    let at = FxVec2::new(Fx::from_f32(site.x), Fx::from_f32(site.y));
    let ties = adj::prospective(field.blueprints, &view.frame.units, view.local, bp, at);
    for t in &ties {
        let [a, b] = t.edge.map(Vec2::from);
        let partner = view
            .index_of
            .get(&t.partner)
            .map(|&i| Vec2::new(view.frame.units[i].pos[0], view.frame.units[i].pos[1]));
        // The provider's centre: the partner's when it saves the site, else the site's.
        let from = if t.incoming {
            partner.unwrap_or(site)
        } else {
            site
        };
        let text = if t.incoming {
            format!("{} {}", adj::percent(t.share), adj::word(t.resource))
        } else {
            let name = &field.blueprints.unit(t.partner_blueprint).name;
            format!(
                "{} {} for {name}",
                adj::percent(t.share),
                adj::word(t.resource)
            )
        };
        seam(
            ui,
            field,
            a,
            b,
            from,
            adj::tone(t.resource),
            true,
            true,
            &text,
            t.bound,
        );
    }
    let Some(g) = project(ui, field, site.extend(surface(field, site) + 2.0)) else {
        return;
    };
    card(ui, field, bp, &ties, g);
}

/// The placing card beside `anchor`: what the site gains and gives, and any bond.
fn card(ui: &mut Ui, field: &Field, bp: &mc_data::UnitBlueprint, ties: &[Tie], anchor: Vec2) {
    let gains = adj::totals(ties);
    let mut lines: Vec<(String, u32)> = Vec::new();
    for r in [
        mc_sim::adjacency::Resource::Energy,
        mc_sim::adjacency::Resource::Mass,
    ] {
        let share = gains[r as usize];
        if share > 0.0 {
            let n = ties
                .iter()
                .filter(|t| t.incoming && t.resource == r)
                .count();
            lines.push((
                format!(
                    "Gets {} {} from {n} neighbour{}",
                    adj::percent(share),
                    adj::word(r).to_lowercase(),
                    if n == 1 { "" } else { "s" }
                ),
                adj::tone(r),
            ));
        }
    }
    for t in ties.iter().filter(|t| !t.incoming) {
        lines.push((
            format!(
                "Gives {} {} to {}",
                adj::percent(t.share),
                adj::word(t.resource).to_lowercase(),
                field.blueprints.unit(t.partner_blueprint).name
            ),
            adj::tone(t.resource),
        ));
    }
    if let Some(b) = ties.iter().find(|t| t.bound) {
        lines.push((
            format!(
                "Bound to {}: either dying takes both",
                field.blueprints.unit(b.partner_blueprint).name
            ),
            BOUND,
        ));
    }
    if lines.is_empty() {
        // Nothing alongside: say what would help, if anything would.
        let Some(h) = adj::hint(field.blueprints, bp, ties) else {
            return;
        };
        lines.push((h, palette::FAINT));
    }
    let w = lines
        .iter()
        .map(|(l, _)| ui.text_width(type_scale::MICRO, l))
        .fold(160.0, f32::max)
        + 24.0;
    let h = 26.0 + lines.len() as f32 * 16.0;
    // Below the cover card's place, beside the pointer.
    let r = Rect::new(anchor.x + 26.0, anchor.y + 18.0, w, h);
    ui.frost(r, 0.72);
    let lead = if gains[1] > 0.0 {
        adj::tone(mc_sim::adjacency::Resource::Energy)
    } else if gains[0] > 0.0 {
        adj::tone(mc_sim::adjacency::Resource::Mass)
    } else {
        palette::DIM
    };
    ui.fill(Rect::new(r.x, r.y, 2.0, r.h), rgb(lead, 1.0));
    let x = r.x + 12.0;
    ui.text(
        x,
        r.y + 13.0,
        type_scale::MICRO,
        rgb(palette::TEXT, 0.95),
        "Adjacency",
    );
    let mut y = r.y + 30.0;
    for (line, tone) in &lines {
        ui.text(x, y, type_scale::MICRO, rgb(*tone, 1.0), line);
        y += 16.0;
    }
}
