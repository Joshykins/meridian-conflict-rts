//! Adjacency on the interface (`mc_sim::adjacency`): where a building's savings come
//! from and what they come to, wherever a building is shown.
//!
//! - the unit panel's Adjacency band: the total saved of each resource and what that
//!   is a second, each provider it comes from, and each building it saves in turn; a
//!   building that could be saved and is not says what to build against it;
//! - the build card: what it gets from neighbours and what it gives them;
//! - the placing card and ground tags (`crate::adjacency_marks`).
//!
//! Hovering a row of the band lights that neighbour's link on the ground.

use super::selection::wrap_text;
use super::{Scene, ENERGY, MASS};
use crate::ui::{id, palette, rgb, type_scale, Rect, Ui};
use glam::Vec2;
use mc_core::{Fx, FxVec2};
use mc_data::{BlueprintId, Blueprints, UnitBlueprint};
use mc_sim::adjacency::{self, Resource};
use mc_sim::mirror::{LinkView, UnitInstance, KIND_GHOST, KIND_PROP, KIND_WRECK};

/// Rows of neighbours the band lists before it sums up the rest.
const ROWS: usize = 3;
const ROW_H: f32 = 17.0;

/// The neighbour whose link is lit, from the band's row under the pointer.
#[derive(Default)]
pub struct Focus {
    pub partner: Option<u32>,
}

/// The resource's tone and its word.
pub fn tone(r: Resource) -> u32 {
    match r {
        Resource::Mass => MASS,
        Resource::Energy => ENERGY,
    }
}

/// The tone a link from `provider` is drawn in on the ground: its faction's power line
/// for energy (`Blueprints::power_line`, linear light, here as sRGB for the interface),
/// the materials tone for materials. The panels keep the economy's own tones (`tone`).
pub fn line_tone(blueprints: &Blueprints, provider: BlueprintId, r: Resource) -> u32 {
    if r == Resource::Mass {
        return MASS;
    }
    let srgb = |c: f32| (c.clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0).round() as u32;
    let [cr, cg, cb] = blueprints.power_line(provider).color;
    srgb(cr) << 16 | srgb(cg) << 8 | srgb(cb)
}

pub fn word(r: Resource) -> &'static str {
    match r {
        Resource::Mass => "Materials",
        Resource::Energy => "Energy",
    }
}

/// One of a building's links, from its own side.
#[derive(Clone, Copy, Debug)]
pub struct Tie {
    /// The other building's unit id (0 for a site being placed).
    pub partner: u32,
    pub partner_blueprint: BlueprintId,
    pub resource: Resource,
    pub share: f32,
    /// The other building saves this one (else this one saves it).
    pub incoming: bool,
    pub edge: [[f32; 2]; 2],
}

/// The links of unit `id`, in and out.
pub fn ties_of(links: &[LinkView], id: u32) -> Vec<Tie> {
    links
        .iter()
        .filter(|l| l.consumer == id || l.provider == id)
        .map(|l| {
            let incoming = l.consumer == id;
            Tie {
                partner: if incoming { l.provider } else { l.consumer },
                partner_blueprint: if incoming {
                    l.provider_blueprint
                } else {
                    l.consumer_blueprint
                },
                resource: l.resource,
                share: l.share,
                incoming,
                edge: l.edge,
            }
        })
        .collect()
}

/// The links a building of `bp` would make at `at` with `local`'s finished buildings.
pub fn prospective(
    blueprints: &Blueprints,
    units: &[UnitInstance],
    local: u8,
    bp: &UnitBlueprint,
    at: FxVec2,
) -> Vec<Tie> {
    let own = adjacency::lot(bp, at);
    let mut out = Vec::new();
    for u in units {
        if u.owner_flags & (KIND_WRECK | KIND_PROP | KIND_GHOST) != 0
            || (u.owner_flags & 0xFF) as u8 != local
            || u.build < 1.0
        {
            continue;
        }
        let other = blueprints.unit(BlueprintId(u.blueprint as u16));
        if !other.is_structure() {
            continue;
        }
        let pos = FxVec2::new(Fx::from_f32(u.pos[0]), Fx::from_f32(u.pos[1]));
        let theirs = adjacency::lot(other, pos);
        let Some(shared) = adjacency::shared_edge(own, theirs) else {
            continue;
        };
        let edge = [shared.0.to_f32(), shared.1.to_f32()];
        for (incoming, offered, consumer) in [
            (true, adjacency::offers(other, bp), own),
            (false, adjacency::offers(bp, other), theirs),
        ] {
            for (resource, full) in offered.into_iter().flatten() {
                out.push(Tie {
                    partner: u.unit_id,
                    partner_blueprint: other.id,
                    resource,
                    share: adjacency::edge_share(full, shared, consumer).to_f32(),
                    incoming,
                    edge,
                });
            }
        }
    }
    out
}

/// The links a building of `bp` placed at `at` would make, for the renderer's
/// planned conduits (`Renderer::set_link_focus`): the site is unit 0.
pub fn planned_links(
    blueprints: &Blueprints,
    units: &[UnitInstance],
    local: u8,
    bp: &UnitBlueprint,
    at: FxVec2,
) -> Vec<LinkView> {
    let site = at.to_f32();
    prospective(blueprints, units, local, bp, at)
        .into_iter()
        .map(|t| {
            let partner = units
                .iter()
                .find(|u| u.unit_id == t.partner)
                .map_or(site, |u| [u.pos[0], u.pos[1]]);
            let (provider, consumer, from, to) = if t.incoming {
                (t.partner, 0, partner, site)
            } else {
                (0, t.partner, site, partner)
            };
            LinkView {
                provider,
                consumer,
                provider_blueprint: if t.incoming {
                    t.partner_blueprint
                } else {
                    bp.id
                },
                consumer_blueprint: if t.incoming {
                    bp.id
                } else {
                    t.partner_blueprint
                },
                owner: local,
                resource: t.resource,
                share: t.share,
                edge: t.edge,
                from,
                to,
            }
        })
        .collect()
}

/// What the incoming ties save: `[mass, energy]`.
pub fn totals(ties: &[Tie]) -> [f32; 2] {
    let mut t = [0.0f32; 2];
    for tie in ties.iter().filter(|t| t.incoming) {
        t[tie.resource as usize] += tie.share;
    }
    t
}

/// How much of the perimeter of a building of `bp` its providers cover, zero to one:
/// its savings grow until this is one.
pub fn ringed(bp: &UnitBlueprint, ties: &[Tie]) -> f32 {
    let mut partners: Vec<(u32, f32)> = ties
        .iter()
        .filter(|t| t.incoming)
        .map(|t| {
            let [a, b] = t.edge;
            (t.partner, (b[0] - a[0]).abs() + (b[1] - a[1]).abs())
        })
        .collect();
    partners.sort_by_key(|p| p.0);
    partners.dedup_by_key(|p| p.0);
    let around = adjacency::perimeter(adjacency::lot(bp, FxVec2::ZERO)) as f32;
    (partners.iter().map(|p| p.1).sum::<f32>() / around).min(1.0)
}

/// The share as "-20%".
pub fn percent(share: f32) -> String {
    format!("\u{2212}{:.0}%", share * 100.0)
}

/// The faction's providers of `resource`: (lowest share, highest share, a name).
fn providers(
    blueprints: &Blueprints,
    bp: &UnitBlueprint,
    resource: Resource,
) -> Option<(f32, f32, &'static str)> {
    let shares: Vec<f32> = blueprints
        .units
        .iter()
        .filter(|p| p.faction == bp.faction)
        .filter_map(|p| p.adjacency)
        .map(|a| match resource {
            Resource::Mass => a.mass.to_f32(),
            Resource::Energy => a.energy.to_f32(),
        })
        .filter(|&s| s > 0.0)
        .collect();
    let lo = shares.iter().copied().fold(f32::MAX, f32::min);
    let hi = shares.iter().copied().fold(0.0, f32::max);
    let name = match resource {
        Resource::Mass => "Fabricators",
        Resource::Energy => "Power Plants",
    };
    (hi > 0.0).then_some((lo, hi, name))
}

/// The share range as "-5 to -20%", or "-20%" when there is one.
fn range(lo: f32, hi: f32) -> String {
    if (hi - lo).abs() < 1e-4 {
        percent(hi)
    } else {
        format!("\u{2212}{:.0} to \u{2212}{:.0}%", lo * 100.0, hi * 100.0)
    }
}

/// What a building of `bp` would gain from neighbours and give them, for the build card:
/// (label, value, tone) rows, none when it takes no part.
pub fn card_rows(blueprints: &Blueprints, bp: &UnitBlueprint) -> Vec<(String, String, u32)> {
    let mut rows = Vec::new();
    if !bp.is_structure() {
        return rows;
    }
    for r in [Resource::Energy, Resource::Mass] {
        if !adjacency::uses(bp, r) {
            continue;
        }
        if let Some((lo, hi, name)) = providers(blueprints, bp, r) {
            let what = match (r, bp.builder.is_some()) {
                (Resource::Energy, false) => "upkeep",
                (Resource::Energy, true) => "energy per build",
                (Resource::Mass, _) => "materials per build",
            };
            rows.push((
                format!("Ringed by {name}"),
                format!("{} {what}", range(lo, hi)),
                tone(r),
            ));
        }
    }
    if let Some(a) = bp.adjacency {
        for (r, share) in [(Resource::Energy, a.energy), (Resource::Mass, a.mass)] {
            if share > Fx::ZERO {
                let whom = match r {
                    Resource::Energy => "upkeep and factory builds",
                    Resource::Mass => "a factory's builds",
                };
                rows.push((
                    "Saves a Ringed Neighbour".into(),
                    format!("{} {} on {whom}", percent(share.to_f32()), word(r)),
                    tone(r),
                ));
            }
        }
    }
    rows
}

/// What to build against a building of `bp` that is saved nothing: one line, or none.
pub fn hint(blueprints: &Blueprints, bp: &UnitBlueprint, ties: &[Tie]) -> Option<String> {
    if !bp.is_structure() {
        return None;
    }
    let has = |r: Resource| ties.iter().any(|t| t.incoming && t.resource == r);
    let wants: Vec<_> = [Resource::Energy, Resource::Mass]
        .into_iter()
        .filter(|&r| adjacency::uses(bp, r) && !has(r))
        .filter_map(|r| providers(blueprints, bp, r).map(|(_, hi, name)| (r, hi, name)))
        .collect();
    match wants.as_slice() {
        [] => None,
        [(r, hi, name)] => Some(format!(
            "Ring it with {}: up to {} {}",
            name.to_lowercase(),
            percent(*hi),
            word(*r).to_lowercase()
        )),
        _ => Some("Build power plants or fabricators against it to cut its costs".into()),
    }
}

/// What the incoming savings come to a second: `[mass, energy]`. The full upkeep is the
/// blueprint's; a factory's build draw is what it asks for now, already less the saving.
fn per_second(s: &Scene, u: &UnitInstance, bp: &UnitBlueprint, saved: [f32; 2]) -> [f32; 2] {
    let (mass, energy) = super::economy::flows(s, u, bp);
    let upkeep = if u.paused() {
        0.0
    } else {
        bp.economy.energy_upkeep.to_f32()
    };
    let build = |wanted: f32, share: f32| {
        if share >= 1.0 {
            0.0
        } else {
            wanted * share / (1.0 - share)
        }
    };
    if bp.builder.is_some() {
        [build(mass.wanted, saved[0]), build(energy.wanted, saved[1])]
    } else {
        [0.0, upkeep * saved[1]]
    }
}

/// Height [`band`] takes for this building.
pub fn band_h(s: &Scene, u: &UnitInstance, bp: &UnitBlueprint) -> f32 {
    let ties = ties_of(&s.view.frame.links, u.unit_id);
    if ties.is_empty() {
        return if hint(s.blueprints, bp, &ties).is_some() && u.build >= 1.0 {
            ROW_H * 2.0 + 6.0
        } else {
            0.0
        };
    }
    let rows = ties.len().min(ROWS) + usize::from(ties.len() > ROWS);
    ROW_H * (1 + rows) as f32 + 6.0
}

/// The unit panel's Adjacency band at `y`; the y below it. `focus` gets the neighbour
/// whose row is under the pointer.
#[expect(
    clippy::too_many_arguments,
    reason = "one band of the card: its unit, place and focus"
)]
pub fn band(
    ui: &mut Ui,
    s: &Scene,
    u: &UnitInstance,
    bp: &UnitBlueprint,
    x: f32,
    y: f32,
    cw: f32,
    focus: &mut Focus,
) -> f32 {
    if band_h(s, u, bp) <= 0.0 {
        return y;
    }
    let ties = ties_of(&s.view.frame.links, u.unit_id);
    let mut y = y + 4.0;
    ui.text(x, y, type_scale::MICRO, rgb(palette::DIM, 1.0), "Adjacency");
    if ties.is_empty() {
        if let Some(h) = hint(s.blueprints, bp, &ties) {
            ui.text_right(
                x + cw,
                y,
                type_scale::MICRO,
                rgb(palette::FAINT, 1.0),
                "None",
            );
            y += ROW_H;
            let line = wrap_text(ui, type_scale::MICRO, &h, cw).remove(0);
            ui.text(x, y, type_scale::MICRO, rgb(palette::FAINT, 1.0), &line);
        }
        return y + ROW_H + 2.0;
    }
    // The totals on the header line, right to left: energy, then materials.
    let saved = totals(&ties);
    let rate = per_second(s, u, bp, saved);
    let mut right = x + cw;
    for r in [Resource::Energy, Resource::Mass] {
        let share = saved[r as usize];
        if share <= 0.0 {
            continue;
        }
        let per = rate[r as usize];
        let text = if per >= 0.05 {
            format!("{} {}  {:.0}/s", percent(share), word(r), per)
        } else {
            format!("{} {}", percent(share), word(r))
        };
        ui.text_right(right, y, type_scale::VALUE, rgb(tone(r), 1.0), &text);
        right -= ui.text_width(type_scale::VALUE, &text) + 14.0;
    }
    if saved != [0.0, 0.0] {
        let ring = ringed(bp, &ties);
        let text = if ring >= 0.999 {
            "Ringed".to_string()
        } else {
            format!("{:.0}% ringed", ring * 100.0)
        };
        ui.text_right(right, y, type_scale::MICRO, rgb(palette::FAINT, 1.0), &text);
    } else {
        let n = ties.iter().filter(|t| !t.incoming).count();
        let text = format!("Saves {n} neighbour{}", if n == 1 { "" } else { "s" });
        ui.text_right(x + cw, y, type_scale::MICRO, rgb(palette::TEXT, 1.0), &text);
    }
    y += ROW_H;
    // Each neighbour: an arrow in (it saves this one) or out (this one saves it).
    let mut hovered = None;
    for (i, t) in ties.iter().take(ROWS).enumerate() {
        let row = Rect::new(x - 4.0, y - ROW_H * 0.5, cw + 8.0, ROW_H);
        let res = ui.interact(id("adjacency-row", i), row, true);
        if res.hovered {
            hovered = Some(t.partner);
            ui.fill(row, rgb(tone(t.resource), 0.10));
        }
        let c = rgb(tone(t.resource), 1.0);
        arrow(ui, Vec2::new(x + 5.0, y), t.incoming, c);
        let name = &s.blueprints.unit(t.partner_blueprint).name;
        let lead = if t.incoming { "from" } else { "to" };
        let end = ui.text(x + 16.0, y, type_scale::MICRO, rgb(palette::DIM, 1.0), lead);
        ui.text(
            end + 5.0,
            y,
            type_scale::MICRO,
            rgb(palette::TEXT, 1.0),
            name,
        );
        ui.text_right(
            x + cw,
            y,
            type_scale::MICRO,
            c,
            &format!("{} {}", percent(t.share), word(t.resource)),
        );
        y += ROW_H;
    }
    if ties.len() > ROWS {
        let more = format!("+{} more, lit on the ground", ties.len() - ROWS);
        ui.text(
            x + 16.0,
            y,
            type_scale::MICRO,
            rgb(palette::FAINT, 1.0),
            &more,
        );
        y += ROW_H;
    }
    focus.partner = hovered;
    y + 2.0
}

/// A small arrow at `c`: pointing right (in) or left (out) of the building.
pub fn arrow(ui: &mut Ui, c: Vec2, incoming: bool, color: crate::ui::Color) {
    let d = if incoming { 1.0 } else { -1.0 };
    ui.stroke(
        c - Vec2::new(4.0 * d, 0.0),
        c + Vec2::new(4.0 * d, 0.0),
        1.3,
        color,
    );
    ui.stroke(
        c + Vec2::new(4.0 * d, 0.0),
        c + Vec2::new(1.0 * d, -3.0),
        1.3,
        color,
    );
    ui.stroke(
        c + Vec2::new(4.0 * d, 0.0),
        c + Vec2::new(1.0 * d, 3.0),
        1.3,
        color,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn blueprints() -> Blueprints {
        Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap()
    }

    fn unit(b: &Blueprints, key: &str, id: u32, x: f32, y: f32) -> UnitInstance {
        UnitInstance {
            unit_id: id,
            blueprint: b.id_of(key).unwrap().0 as u32,
            pos: [x, y, 0.0],
            build: 1.0,
            ..bytemuck::Zeroable::zeroed()
        }
    }

    #[test]
    fn a_site_against_a_reactor_and_a_factory_shows_both_ways() {
        let b = blueprints();
        // Lots: the site 780..804; the Reactor III east (804..900), the factory west (684..780).
        let units = [
            unit(&b, "aster_t3_power", 1, 852.0, 792.0),
            unit(&b, "aster_t3_air_factory", 2, 732.0, 792.0),
        ];
        let fab = b.unit(b.id_of("aster_t3_fabricator").unwrap());
        let ties = prospective(&b, &units, 0, fab, FxVec2::from_ints(792, 792));
        // The reactor covers the site's east side: a quarter of a full ring's 60%.
        let gets = totals(&ties);
        assert!((gets[Resource::Energy as usize] - 0.15).abs() < 1e-3);
        assert!((ringed(fab, &ties) - 0.25).abs() < 1e-3);
        assert!(ties
            .iter()
            .any(|t| !t.incoming && t.partner == 2 && t.resource == Resource::Mass));
    }

    #[test]
    fn the_build_card_says_what_a_fabricator_gets_and_gives() {
        let b = blueprints();
        let fab = b.unit(b.id_of("aster_t2_fabricator").unwrap());
        let rows = card_rows(&b, fab);
        let labels: Vec<&str> = rows.iter().map(|r| r.0.as_str()).collect();
        assert!(labels.contains(&"Ringed by Power Plants"));
        assert!(labels.contains(&"Saves a Ringed Neighbour"));
        assert!(!rows.iter().any(|r| r.0 == "Bound"));
        assert!(hint(&b, fab, &[]).is_some_and(|h| h.contains("power plants")));
    }
}
