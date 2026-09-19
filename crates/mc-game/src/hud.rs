//! Heads-up display: economy bar, selection panel, build menu, profiler.
//! Immediate mode: rebuilt into the overlay every frame; buttons are remembered
//! for hit-testing until the next frame.

use crate::app::{Mode, View};
use glam::Vec2;
use mc_data::{cat, BlueprintId, Blueprints};
use mc_render::{FrameStats, Overlay};
use mc_sim::tables::flag;

const PANEL: [f32; 4] = [0.02, 0.03, 0.045, 0.82];
const EDGE: [f32; 4] = [0.45, 0.75, 1.0, 0.55];
const TEXT: [f32; 4] = [0.9, 0.95, 1.0, 1.0];
const DIM: [f32; 4] = [0.55, 0.65, 0.75, 1.0];
const MASS: [f32; 4] = [0.35, 0.9, 0.4, 1.0];
const ENERGY: [f32; 4] = [1.0, 0.8, 0.2, 1.0];
const BAD: [f32; 4] = [1.0, 0.3, 0.2, 1.0];

#[derive(Clone, Copy, Debug)]
pub enum Action {
    Build(BlueprintId),
    Upgrade,
    Stop,
    Repeat(bool),
}

struct Button {
    min: Vec2,
    max: Vec2,
    action: Action,
}

#[derive(Default)]
pub struct Hud {
    buttons: Vec<Button>,
    /// Panels that swallow clicks even where there is no button.
    panels: Vec<(Vec2, Vec2)>,
}

fn panel(o: &mut Overlay, x: f32, y: f32, w: f32, h: f32) {
    o.rect(x, y, w, h, PANEL);
    o.frame(x, y, w, h, 1.0, EDGE);
}

impl Hud {
    pub fn hit(&self, p: Vec2) -> Option<Action> {
        self.buttons.iter().find(|b| p.cmpge(b.min).all() && p.cmple(b.max).all()).map(|b| b.action)
    }

    pub fn covers(&self, p: Vec2) -> bool {
        self.panels.iter().any(|(min, max)| p.cmpge(*min).all() && p.cmple(*max).all())
    }

    fn button(&mut self, o: &mut Overlay, x: f32, y: f32, w: f32, h: f32, label: &str, sub: &str, action: Action, lit: bool) {
        o.rect(x, y, w, h, if lit { [0.12, 0.3, 0.45, 0.95] } else { [0.06, 0.09, 0.13, 0.95] });
        o.frame(x, y, w, h, 1.0, EDGE);
        o.text(x + 6.0, y + 5.0, 1.5, TEXT, label);
        o.text(x + 6.0, y + 22.0, 1.0, DIM, sub);
        self.buttons.push(Button { min: Vec2::new(x, y), max: Vec2::new(x + w, y + h), action });
    }

    pub fn draw(&mut self, o: &mut Overlay, view: &View, blueprints: &Blueprints, viewport: Vec2, gpu: &FrameStats) {
        self.buttons.clear();
        self.panels.clear();
        let status = &view.status;

        // Economy bar.
        if let Some(p) = status.players.get(view.local as usize) {
            let (w, h) = (560.0, 46.0);
            let x = (viewport.x - w) * 0.5;
            panel(o, x, 8.0, w, h);
            self.panels.push((Vec2::new(x, 8.0), Vec2::new(x + w, 8.0 + h)));
            for (i, (name, have, cap, income, demand, color)) in
                [("MASS", p.mass, p.mass_capacity, p.mass_income, p.mass_demand, MASS), ("ENERGY", p.energy, p.energy_capacity, p.energy_income, p.energy_demand, ENERGY)].into_iter().enumerate()
            {
                let bx = x + 12.0 + i as f32 * 274.0;
                o.text(bx, 14.0, 1.5, color, name);
                o.text(bx + 70.0, 14.0, 1.5, TEXT, &format!("{:.0}/{:.0}", have, cap));
                let net = income - demand * p.efficiency;
                o.text(bx + 190.0, 14.0, 1.5, if net < 0.0 { BAD } else { color }, &format!("{net:+.1}"));
                o.rect(bx, 34.0, 260.0, 10.0, [0.0, 0.0, 0.0, 0.6]);
                o.rect(bx, 34.0, 260.0 * (have / cap.max(1.0)).clamp(0.0, 1.0), 10.0, color);
                o.text(bx + 4.0, 35.0, 1.0, [0.0, 0.0, 0.0, 1.0], &format!("+{income:.1} -{demand:.1}"));
            }
            if p.efficiency < 0.999 {
                o.label(x + w + 10.0, 22.0, 1.5, BAD, &format!("STALLING {:.0}%", p.efficiency * 100.0));
            }
        }

        self.selection_panel(o, view, blueprints, viewport);
        if view.show_profiler {
            profiler(o, view, viewport, gpu);
        }

        // Match state.
        let centre = |o: &mut Overlay, y: f32, scale: f32, color: [f32; 4], text: &str| {
            o.label((viewport.x - Overlay::text_width(scale, text)) * 0.5, y, scale, color, text);
        };
        if let Some(e) = &status.error {
            centre(o, viewport.y * 0.4, 2.0, BAD, e);
        } else if let Some(team) = status.winner {
            let won = status.players.get(view.local as usize).is_some_and(|p| p.team == team);
            centre(o, viewport.y * 0.35, 5.0, if won { MASS } else { BAD }, if won { "VICTORY" } else { "DEFEAT" });
        } else if status.tick == 0 {
            centre(o, viewport.y * 0.45, 2.5, TEXT, "LOADING MAP...");
        }
        o.text(8.0, viewport.y - 12.0, 1.0, DIM, "LMB select  RMB order  F attack-move  X stop  U upgrade  Home commander  WASD/QE/wheel camera  F1 profiler");
    }

    fn selection_panel(&mut self, o: &mut Overlay, view: &View, blueprints: &Blueprints, viewport: Vec2) {
        let units: Vec<_> = view.selection.iter().filter_map(|id| view.index_of.get(id)).map(|&i| &view.frame.units[i]).collect();
        if units.is_empty() {
            return;
        }
        let (w, h) = (300.0, 118.0);
        let (x, y) = (8.0, viewport.y - h - 22.0);
        panel(o, x, y, w, h);
        self.panels.push((Vec2::new(x, y), Vec2::new(x + w, y + h)));
        let first = blueprints.unit(BlueprintId(units[0].blueprint as u16));
        if units.len() == 1 {
            let u = units[0];
            o.text(x + 10.0, y + 10.0, 2.0, TEXT, &first.name);
            o.text(x + 10.0, y + 32.0, 1.5, DIM, &format!("T{} {}", first.tech, first.role));
            let hp = first.health.to_f32();
            o.text(x + 10.0, y + 54.0, 1.5, MASS, &format!("HP {:.0}/{:.0}", u.health * hp, hp));
            if u.owner_flags & (flag::UNDER_CONSTRUCTION as u32) << 8 != 0 {
                o.text(x + 10.0, y + 74.0, 1.5, ENERGY, &format!("BUILDING {:.0}%", u.build * 100.0));
            }
            for (i, wpn) in first.weapons.iter().take(2).enumerate() {
                o.text(x + 10.0, y + 92.0 + i as f32 * 11.0, 1.0, DIM, &format!("{}: {:.0} dmg, {:.0} m", wpn.name, wpn.damage.to_f32(), wpn.range_max.to_f32()));
            }
        } else {
            o.text(x + 10.0, y + 10.0, 2.0, TEXT, &format!("{} UNITS", units.len()));
            let mut counts: std::collections::BTreeMap<u32, usize> = Default::default();
            for u in &units {
                *counts.entry(u.blueprint).or_default() += 1;
            }
            for (i, (bp, n)) in counts.iter().take(7).enumerate() {
                o.text(x + 10.0, y + 34.0 + i as f32 * 12.0, 1.0, DIM, &format!("{n:>4} x {}", blueprints.unit(BlueprintId(*bp as u16)).name));
            }
        }

        // Build menu: what the first builder in the selection can make.
        let builder = units.iter().map(|u| blueprints.unit(BlueprintId(u.blueprint as u16))).find(|bp| bp.builder.is_some());
        let mut bx = x + w + 10.0;
        let by = viewport.y - 62.0;
        let placing = match view.mode {
            Mode::Place(b) => Some(b),
            _ => None,
        };
        if let Some(bp) = builder {
            let builds = &bp.builder.as_ref().expect("filtered above").builds;
            let per_row = (((viewport.x - bx - 10.0) / 176.0) as usize).max(1);
            let rows = builds.len().div_ceil(per_row);
            let top = by - (rows as f32 - 1.0) * 44.0;
            self.panels.push((Vec2::new(bx - 4.0, top - 4.0), Vec2::new(viewport.x, viewport.y)));
            for (i, id) in builds.iter().enumerate() {
                let item = blueprints.unit(*id);
                let (col, row) = (i % per_row, i / per_row);
                let cost = format!("M{:.0} E{:.0}", item.cost_mass.to_f32(), item.cost_energy.to_f32());
                self.button(o, bx + col as f32 * 176.0, top + row as f32 * 44.0, 170.0, 38.0, &item.name, &cost, Action::Build(*id), placing == Some(*id));
            }
            bx += (builds.len().min(per_row)) as f32 * 176.0;
        }
        let _ = bx;
        // Unit actions above the selection panel.
        let mut ax = x;
        let ay = y - 44.0;
        self.button(o, ax, ay, 70.0, 38.0, "STOP", "X", Action::Stop, false);
        ax += 76.0;
        if first.upgrades_to.is_some() {
            self.button(o, ax, ay, 100.0, 38.0, "UPGRADE", "U", Action::Upgrade, false);
            ax += 106.0;
        }
        if first.has(cat::FACTORY) {
            let on = units[0].owner_flags & (flag::REPEAT as u32) << 8 != 0;
            self.button(o, ax, ay, 100.0, 38.0, "REPEAT", if on { "on" } else { "off" }, Action::Repeat(!on), on);
        }
        self.panels.push((Vec2::new(x, ay), Vec2::new(x + 300.0, ay + 38.0)));
    }
}

/// Timing for every sim phase and GPU pass, as the spec asks, plus table sizes.
fn profiler(o: &mut Overlay, view: &View, viewport: Vec2, gpu: &FrameStats) {
    let s = &view.status;
    let lines = 10 + s.phases.len() + gpu.gpu_passes.len();
    let (w, h) = (250.0, lines as f32 * 11.0 + 14.0);
    let x = viewport.x - w - 8.0;
    panel(o, x, 8.0, w, h);
    let mut y = 14.0;
    let mut row = |o: &mut Overlay, color: [f32; 4], text: String| {
        o.text(x + 8.0, y, 1.0, color, &text);
        y += 11.0;
    };
    row(o, TEXT, format!("{:>5.0} fps   cpu frame {:>5.2} ms", view.fps, view.cpu_ms));
    let gpu_total: f32 = gpu.gpu_passes.iter().map(|p| p.1).sum();
    row(o, TEXT, format!("gpu frame {gpu_total:>6.2} ms"));
    for (name, ms) in &gpu.gpu_passes {
        row(o, DIM, format!("  {name:<12}{ms:>8.2} ms"));
    }
    let budget = if s.tick_ns > 25_000_000 { BAD } else { TEXT };
    row(o, budget, format!("sim tick {:>6.2} ms  worst {:>5.2}", s.tick_ns as f32 / 1e6, s.worst_tick_ns as f32 / 1e6));
    for (name, ns) in &s.phases {
        row(o, DIM, format!("  {name:<12}{:>8.3} ms", *ns as f32 / 1e6));
    }
    row(o, TEXT, format!("tick {}  hash {:08x}", s.tick, s.hash as u32));
    row(o, DIM, format!("units {}  projectiles {}", s.units, s.projectiles));
    row(o, DIM, format!("wrecks {}  stains {}", s.wrecks, s.stains));
    row(o, DIM, format!("orders {}  fields {}  late {}", s.orders, s.flow_fields, s.late_paths));
    row(o, DIM, format!("drawn: {} entities, {} props", gpu.dynamic_entities, gpu.static_entities));
    row(o, DIM, format!("terrain nodes {}", gpu.terrain_nodes));
}

pub fn cursor_hint(o: &mut Overlay, view: &View, blueprints: &Blueprints, cursor: Vec2) {
    let text = match view.mode {
        Mode::Normal => return,
        Mode::AttackMove => "ATTACK-MOVE".to_owned(),
        Mode::Place(b) => format!("PLACE {}  (shift: keep placing, RMB: cancel)", blueprints.unit(b).name),
    };
    o.label(cursor.x + 18.0, cursor.y + 18.0, 1.5, ENERGY, &text);
}
