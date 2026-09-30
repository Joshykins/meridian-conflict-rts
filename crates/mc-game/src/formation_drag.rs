//! A move order held on the right button: while it is held the formation the
//! selection would take shows on the ground, and dragging turns it to face the
//! way the drag goes. Letting go gives the order; a click without a drag gives
//! it facing the way it goes, as before. The formation shows only once the
//! button has been held for `SHOW_AFTER` or dragged, so a quick click gives its
//! order without the block flashing up.
//!
//! The slots come from `mc_sim::formations::plan`, the layout the sim itself
//! gives the order, so what shows is where the units go (short of the sim moving
//! the whole block clear of a map edge or a parked hull).

use crate::game::View;
use crate::hud;
use crate::nuke_marks::{ground_ring, project, surface};
use crate::orders::Field;
use crate::ui::{self, palette, Ui};
use glam::Vec2;
use mc_core::{Angle, Fx, FxVec2};
use mc_data::BlueprintId;
use mc_sim::formations::{plan, Member};
use mc_sim::Command;
use std::time::{Duration, Instant};

/// Metres the pointer must be from where it was pressed, on the ground, to turn the block.
const TURN_REACH: f32 = 4.0;

/// How long the button must be held before the formation shows, if it is not dragged.
const SHOW_AFTER: Duration = Duration::from_millis(250);

/// A right-press that would give a move, not yet let go.
pub struct FormationDrag {
    /// Where it was pressed, pixels.
    pressed_at: Vec2,
    /// When it was pressed.
    pressed: Instant,
    /// The ground it was pressed on: the middle of the block.
    anchor: Vec2,
    /// The order a click gives: a `Move`, `AttackMove` or `FormationMove`.
    command: Command,
}

fn to_fx(p: Vec2) -> FxVec2 {
    FxVec2::new(Fx::from_f32(p.x), Fx::from_f32(p.y))
}

impl FormationDrag {
    /// Holds `command` if it is a move of some kind; gives any other back.
    pub fn new(command: Command, pressed_at: Vec2) -> Result<FormationDrag, Command> {
        let target = match &command {
            Command::Move { target, .. }
            | Command::AttackMove { target, .. }
            | Command::FormationMove { target, .. } => *target,
            _ => return Err(command),
        };
        Ok(FormationDrag {
            pressed_at,
            pressed: Instant::now(),
            anchor: Vec2::from(target.to_f32()),
            command,
        })
    }

    /// Whether the formation shows yet: held past `SHOW_AFTER`, or dragged `threshold` pixels.
    pub fn showing(&self, cursor: Vec2, threshold: f32) -> bool {
        self.pressed.elapsed() >= SHOW_AFTER || cursor.distance(self.pressed_at) >= threshold
    }

    /// Which way the drag has turned the block: `None` until the pointer has left the press.
    pub fn facing(&self, cursor: Vec2, ground: Option<Vec2>, threshold: f32) -> Option<Angle> {
        let ground = ground?;
        if cursor.distance(self.pressed_at) < threshold || ground.distance(self.anchor) < TURN_REACH
        {
            return None;
        }
        let d = to_fx(ground) - to_fx(self.anchor);
        Some(Angle::atan2(d.y, d.x))
    }

    /// The order to give on letting go, with the formation settings in `view`.
    pub fn command(self, facing: Option<Angle>, view: &View) -> Command {
        let (units, target, queue, attack_move) = match self.command {
            Command::Move {
                units,
                target,
                queue,
            } => (units, target, queue, false),
            Command::AttackMove {
                units,
                target,
                queue,
            } => (units, target, queue, true),
            Command::FormationMove {
                units,
                target,
                queue,
                attack_move,
                ..
            } => (units, target, queue, attack_move),
            other => return other,
        };
        Command::FormationMove {
            units,
            target,
            queue,
            attack_move,
            facing,
            together: view.formation_together,
            spacing: view.formation_spacing,
        }
    }

    /// The block as it would stand: a ring for every member's place, a tick on each
    /// pointing the way it faces, and the arrow the drag is turning.
    pub fn draw(&self, ui: &mut Ui, field: &Field, facing: Option<Angle>) {
        let view = field.view;
        let (units, attack) = match &self.command {
            Command::Move { units, .. } => (units, false),
            Command::AttackMove { units, .. } => (units, true),
            Command::FormationMove {
                units, attack_move, ..
            } => (units, *attack_move),
            _ => return,
        };
        let tone = if attack {
            palette::BAD
        } else {
            hud::style::Family::Movement.tone()
        };
        let mut ids: Vec<u32> = units.iter().map(|h| h.0).collect();
        ids.sort_unstable();
        ids.dedup();
        let members: Vec<(Member, f32)> = ids
            .iter()
            .filter_map(|id| view.index_of.get(id))
            .filter_map(|&i| {
                let u = &view.frame.units[i];
                let bp = field.blueprints.unit(BlueprintId(u.blueprint as u16));
                let at = Vec2::new(u.pos[0], u.pos[1]);
                let heading =
                    Angle::atan2(Fx::from_f32(u.heading.sin()), Fx::from_f32(u.heading.cos()));
                Some((Member::new(bp, to_fx(at), heading)?, bp.radius.to_f32()))
            })
            .collect();
        if members.is_empty() {
            return;
        }
        let only: Vec<Member> = members.iter().map(|&(m, _)| m).collect();
        let target = to_fx(self.anchor);
        let spacing = view.formation_spacing;
        let mut reach = 0.0f32;
        let mut turned = 0.0f32;
        for laid in plan(&only, target, facing, spacing) {
            turned = laid.facing.to_radians_f32();
            let forward = Vec2::from_angle(turned);
            for (&i, offset) in laid.members.iter().zip(&laid.offsets) {
                let slot = self.anchor + Vec2::from(offset.to_f32());
                let radius = members[i].1.max(2.0);
                reach = reach.max(slot.distance(self.anchor) + radius);
                ground_ring(
                    ui,
                    field,
                    slot,
                    radius,
                    1.4,
                    ui::rgb(tone, 0.85),
                    false,
                    0.0,
                );
                let lift = |p: Vec2| p.extend(surface(field, p) + 1.0);
                if let (Some(a), Some(b)) = (
                    project(ui, field, lift(slot)),
                    project(ui, field, lift(slot + forward * radius * 1.4)),
                ) {
                    ui.stroke(a, b, 1.6, ui::rgb(tone, 0.95));
                }
            }
        }
        // The arrow the drag turns: from the middle out past the front rank.
        let forward = Vec2::from_angle(turned);
        let lift = |p: Vec2| p.extend(surface(field, p) + 1.5);
        let tip = self.anchor + forward * (reach + 12.0).max(30.0);
        let side = forward.perp() * 7.0;
        let points: Vec<Option<Vec2>> = [
            self.anchor,
            tip,
            tip - forward * 10.0 + side,
            tip - forward * 10.0 - side,
        ]
        .into_iter()
        .map(|p| project(ui, field, lift(p)))
        .collect();
        if let [Some(a), Some(t), Some(l), Some(r)] = points[..] {
            let color = ui::rgb(tone, if facing.is_some() { 1.0 } else { 0.55 });
            ui.stroke(a, t, 2.0, color);
            ui.stroke(t, l, 2.0, color);
            ui.stroke(t, r, 2.0, color);
            ui.disc(a, 3.0, color);
        }
    }
}
