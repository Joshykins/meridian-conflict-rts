//! Area reclaim on the battlefield: the circle a Reclaim drag would clear, drawn in
//! the mass colour the salvage brings in. Queued ones are drawn with the guard rings.

use super::guard_rings::guard_ring;
use super::Field;
use crate::game::{Mode, Targeting};
use crate::hud;
use crate::ui::{self, Ui};
use glam::Vec2;
use mc_sim::command::MAX_RECLAIM_RADIUS;

/// Smallest circle a Reclaim drag sets, metres of radius.
const RECLAIM_MIN: f32 = 20.0;
/// How far the pointer has to have gone from the press, metres, before the circle shows.
const SHOWN_FROM: f32 = 10.0;

/// The circle a Reclaim drag of `drag` metres clears.
pub(crate) fn reclaim_radius(drag: f32) -> f32 {
    drag.clamp(RECLAIM_MIN, MAX_RECLAIM_RADIUS.to_f32())
}

/// The circle a Reclaim being dragged out would clear, and its centre.
pub(super) fn draw_reclaim_drag(
    ui: &mut Ui,
    field: &Field,
    ground: Option<Vec2>,
    project: impl Fn(Vec2) -> Option<Vec2>,
) {
    let view = field.view;
    if view.mode != Mode::Target(Targeting::Reclaim) {
        return;
    }
    let Some((centre, g)) = view.circle_from.zip(ground) else {
        return;
    };
    if centre.distance(g) < SHOWN_FROM {
        return;
    }
    guard_ring(
        ui,
        field,
        centre,
        reclaim_radius(centre.distance(g)),
        1.0,
        hud::MASS,
    );
    if let Some(c) = project(centre) {
        ui.disc(c, 3.0, ui::rgb(hud::MASS, 1.0));
    }
}
