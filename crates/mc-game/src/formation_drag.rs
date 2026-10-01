//! A move order held on the right button: while it is held the formation the
//! selection would take shows on the ground, and dragging turns it to face the
//! way the drag goes. Letting go gives the order; a click without a drag gives
//! it facing the way it goes, as before. The formation shows only once the
//! button has been held for `SHOW_AFTER` or dragged, so a quick click gives its
//! order without the block flashing up.
//!
//! Each member shows where it will stand as a hologram of itself (a placement
//! ghost), and the wheel, while the button is held, makes the block wider or
//! longer: it steps the same shape setting the formation panel shows.
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
use mc_sim::formations::{plan, Member, SHAPES};
use mc_sim::mirror::{UnitInstance, KIND_GHOST};
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
    /// The wheel was turned while it is held: the formation shows at once.
    shaped: bool,
    /// Wheel turned short of a whole notch (touchpads), carried to the next.
    wheel: f32,
}

/// Where one member will stand.
struct Slot {
    at: Vec2,
    facing: f32,
    radius: f32,
    blueprint: u32,
    /// Its cruise height over the ground, aircraft; 0 for the rest.
    altitude: f32,
}

/// How a block of `shape` stands, for the panel and the wheel's caption.
pub(crate) fn shape_label(shape: i8) -> String {
    const RATIO: [&str; 7] = ["", "1.4", "2", "2.8", "4", "5.7", "8"];
    let n = RATIO[shape.unsigned_abs().min(SHAPES as u8) as usize];
    match shape {
        0 => "Square".into(),
        s if s > 0 => format!("Wide \u{d7}{n}"),
        _ => format!("Long \u{d7}{n}"),
    }
}

/// `shape` stepped `notches` wider (or, negative, longer), within what the sim takes.
pub(crate) fn step_shape(shape: i8, notches: i8) -> i8 {
    shape.saturating_add(notches).clamp(-SHAPES, SHAPES)
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
            shaped: false,
            wheel: 0.0,
        })
    }

    /// Whether the formation shows yet: held past `SHOW_AFTER`, dragged `threshold`
    /// pixels or shaped with the wheel.
    pub fn showing(&self, cursor: Vec2, threshold: f32) -> bool {
        self.shaped
            || self.pressed.elapsed() >= SHOW_AFTER
            || cursor.distance(self.pressed_at) >= threshold
    }

    /// The wheel turned `lines` notches while the button is held: up makes the
    /// block wider, down longer. Steps `view`'s shape, which the order takes.
    pub fn wheel(&mut self, lines: f32, view: &mut View) {
        self.shaped = true;
        self.wheel += lines;
        let notches = self.wheel.trunc();
        self.wheel -= notches;
        view.formation_shape = step_shape(view.formation_shape, notches as i8);
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
            shape: view.formation_shape,
        }
    }

    /// The units the order moves, and whether it is an attack-move.
    fn units(&self) -> Option<(&[mc_sim::Handle], bool)> {
        match &self.command {
            Command::Move { units, .. } => Some((units, false)),
            Command::AttackMove { units, .. } => Some((units, true)),
            Command::FormationMove {
                units, attack_move, ..
            } => Some((units, *attack_move)),
            _ => None,
        }
    }

    /// Where each member would stand, laid out as the sim lays out the order.
    fn slots(&self, field: &Field, facing: Option<Angle>) -> Vec<Slot> {
        let view = field.view;
        let Some((units, _)) = self.units() else {
            return Vec::new();
        };
        let mut ids: Vec<u32> = units.iter().map(|h| h.0).collect();
        ids.sort_unstable();
        ids.dedup();
        let members: Vec<(Member, &UnitInstance, f32)> = ids
            .iter()
            .filter_map(|id| view.index_of.get(id))
            .filter_map(|&i| {
                let u = &view.frame.units[i];
                let bp = field.blueprints.unit(BlueprintId(u.blueprint as u16));
                let at = Vec2::new(u.pos[0], u.pos[1]);
                let heading =
                    Angle::atan2(Fx::from_f32(u.heading.sin()), Fx::from_f32(u.heading.cos()));
                let altitude = bp
                    .motion
                    .filter(|m| m.layer == mc_data::MoveLayer::Air)
                    .map_or(0.0, |m| m.altitude.to_f32());
                Some((Member::new(bp, to_fx(at), heading)?, u, altitude))
            })
            .collect();
        let only: Vec<Member> = members.iter().map(|&(m, _, _)| m).collect();
        let mut out = Vec::with_capacity(members.len());
        let laid = plan(
            &only,
            to_fx(self.anchor),
            facing,
            view.formation_spacing,
            view.formation_shape,
        );
        for laid in laid {
            let turned = laid.facing.to_radians_f32();
            for (&i, offset) in laid.members.iter().zip(&laid.offsets) {
                let (_, u, altitude) = members[i];
                out.push(Slot {
                    at: self.anchor + Vec2::from(offset.to_f32()),
                    facing: turned,
                    radius: u.radius,
                    blueprint: u.blueprint,
                    altitude,
                });
            }
        }
        out
    }

    /// A hologram of each member where it would stand, for the renderer's ghosts.
    pub fn ghosts(&self, field: &Field, facing: Option<Angle>, out: &mut Vec<UnitInstance>) {
        let local = field.view.local as u32;
        for slot in self.slots(field, facing) {
            let pos = slot
                .at
                .extend(surface(field, slot.at) + slot.altitude)
                .into();
            out.push(UnitInstance {
                prev_pos: pos,
                pos,
                prev_heading: slot.facing,
                heading: slot.facing,
                blueprint: slot.blueprint,
                owner_flags: local | KIND_GHOST,
                // A ghost's health is whether it fits: these always do.
                health: 1.0,
                build: 1.0,
                radius: slot.radius,
                unit_id: u32::MAX,
                ..bytemuck::Zeroable::zeroed()
            });
        }
    }

    /// The block as it would stand: a faint ring under every member's hologram,
    /// the arrow the drag is turning, and past its tip how the wheel has shaped it.
    pub fn draw(&self, ui: &mut Ui, field: &Field, facing: Option<Angle>) {
        let Some((_, attack)) = self.units() else {
            return;
        };
        let tone = if attack {
            palette::BAD
        } else {
            hud::style::Family::Movement.tone()
        };
        let slots = self.slots(field, facing);
        let Some(first) = slots.first() else {
            return;
        };
        let mut reach = 0.0f32;
        for slot in &slots {
            let radius = slot.radius.max(2.0);
            reach = reach.max(slot.at.distance(self.anchor) + radius);
            ground_ring(
                ui,
                field,
                slot.at,
                radius,
                1.2,
                ui::rgb(tone, 0.5),
                false,
                0.0,
            );
        }
        // The arrow the drag turns: from the middle out past the front rank.
        let forward = Vec2::from_angle(first.facing);
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
            // What the wheel has done and does, past the arrow's tip, clear of the block.
            let away = (t - a).normalize_or_zero();
            let at = t + away * 14.0 + Vec2::new(if away.x < 0.0 { -150.0 } else { 0.0 }, -8.0);
            ui.text(
                at.x,
                at.y,
                ui::type_scale::MICRO,
                ui::rgb(palette::TEXT, 0.95),
                &shape_label(field.view.formation_shape).to_uppercase(),
            );
            ui.text(
                at.x,
                at.y + 13.0,
                ui::type_scale::MICRO,
                ui::rgb(palette::DIM, 0.9),
                "WHEEL: WIDER / LONGER",
            );
        }
    }
}
