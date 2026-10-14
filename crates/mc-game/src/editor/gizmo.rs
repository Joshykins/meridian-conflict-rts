//! Screen handles drive shared document commands; a release is one undo step.
use super::*;
#[derive(Clone, Copy, Default, PartialEq)]
enum Mode {
    #[default]
    Move,
    Rotate,
    Scale,
}
#[derive(Clone, Copy)]
struct Pose {
    id: u64,
    at: Vec2,
    heading: f32,
    scalable: bool,
    rotatable: bool,
}
struct Drag {
    pose: Pose,
    handle: usize,
    start: Vec2,
    centre: Vec2,
    end: Vec2,
    at: Vec2,
    heading: f32,
    factor: f32,
}
#[derive(Default)]
pub(super) struct Gizmo {
    mode: Mode,
    grid: f32,
    drag: Option<Drag>,
}
impl Gizmo {
    pub(super) fn snap(&self, at: [f32; 2]) -> [f32; 2] {
        if self.grid > 0.0 {
            at.map(|v| (v / self.grid).round() * self.grid)
        } else {
            at
        }
    }
}
impl Editor {
    pub(super) fn configure_gizmo(&mut self, mode: &str, grid: f32) -> Result<(), String> {
        let mode = match mode {
            "move" => Mode::Move,
            "rotate" => Mode::Rotate,
            "scale" => Mode::Scale,
            _ => return Err("Gizmo mode must be move, rotate or scale".into()),
        };
        if !grid.is_finite() || !(0.0..=1024.0).contains(&grid) {
            return Err("Grid spacing must be 0 to 1024 metres; 0 disables snapping".into());
        }
        self.gizmo = Gizmo {
            mode,
            grid,
            drag: None,
        };
        Ok(())
    }
    fn gizmo_pose(&self) -> Option<Pose> {
        let id = self.selected?;
        let placed = &self.doc.placed;
        let (at, heading, scalable, rotatable) =
            if let Some(d) = placed.doodads.iter().find(|d| d.id == id) {
                let p = &self.doc.map.props[d.prop_index as usize];
                (p.pos, p.heading.to_radians_f32().to_degrees(), true, true)
            } else if let Some(u) = placed.units.iter().find(|u| u.id == id) {
                (u.at, u.heading.to_radians_f32().to_degrees(), false, true)
            } else if let Some(p) = placed.points.iter().find(|p| p.id == id) {
                (p.at, 0.0, false, false)
            } else if let Some(c) = placed.cameras.iter().find(|c| c.id == id) {
                (c.at, c.yaw.to_radians_f32().to_degrees(), false, true)
            } else {
                let r = placed.regions.iter().find(|r| r.id == id)?;
                ((r.min + r.max) * mc_core::Fx::HALF, 0.0, true, false)
            };
        Some(Pose {
            id,
            at: at.to_f32().into(),
            heading,
            scalable,
            rotatable,
        })
    }
    fn gizmo_handles(&self, pose: Pose, renderer: &mc_render::Renderer) -> Option<[Vec2; 3]> {
        let ground = renderer.ground_height(pose.at) + 3.0;
        let centre = self.camera.project(pose.at.extend(ground))?;
        let axis = |v: Vec2| {
            self.camera
                .project((pose.at + v * 30.0).extend(ground))
                .map(|p| centre + (p - centre).normalize_or_zero() * 64.0)
        };
        Some([centre, axis(Vec2::X)?, axis(Vec2::Y)?])
    }
    pub(super) fn gizmo_controls(&mut self, ui: &mut Ui, y: f32) {
        for (i, (label, mode)) in [
            ("Move", Mode::Move),
            ("Rotate", Mode::Rotate),
            ("Scale", Mode::Scale),
        ]
        .into_iter()
        .enumerate()
        {
            if ui.button(
                id("gizmo_mode", i),
                Rect::new(34.0 + i as f32 * 110.0, y, 102.0, 30.0),
                label,
                if self.gizmo.mode == mode {
                    ButtonKind::Primary
                } else {
                    ButtonKind::Secondary
                },
                true,
            ) {
                self.gizmo.mode = mode;
            }
        }
        let label = if self.gizmo.grid == 0.0 {
            "Grid: off".into()
        } else {
            format!("Grid: {} m", self.gizmo.grid)
        };
        if ui.button(
            id("gizmo_grid", 0),
            Rect::new(374.0, y, 140.0, 30.0),
            &label,
            ButtonKind::Secondary,
            true,
        ) {
            self.gizmo.grid = match self.gizmo.grid as u32 {
                0 => 1.0,
                1 => 4.0,
                4 => 16.0,
                _ => 0.0,
            };
        }
        ui.text_fit_left(
            530.0,
            y + 22.0,
            (ui.size.x - 564.0).max(1.0),
            type_scale::CAPTION,
            rgb(palette::DIM, 1.0),
            "Ground snapped · drag a handle · right click cancels",
        );
    }
    pub(super) fn draw_gizmo(&self, ui: &mut Ui, renderer: &mc_render::Renderer) {
        if !matches!(self.tool, 0 | 9 | 27) {
            return;
        }
        let Some(mut pose) = self.gizmo_pose() else {
            return;
        };
        if let Some(d) = &self.gizmo.drag {
            pose.at = d.at;
            pose.heading = d.heading;
        }
        let Some([c, x, y]) = self.gizmo_handles(pose, renderer) else {
            return;
        };
        let line = |ui: &mut Ui, a: Vec2, b: Vec2, color| {
            ui.o.line(a.to_array(), b.to_array(), 3.0, rgb(color, 1.0))
        };
        let square = |ui: &mut Ui, p: Vec2, color| {
            ui.fill(
                Rect::new(p.x / ui.s - 6.0, p.y / ui.s - 6.0, 12.0, 12.0),
                rgb(color, 1.0),
            )
        };
        match self.gizmo.mode {
            Mode::Move => {
                line(ui, c, x, 0xFF7777);
                line(ui, c, y, 0x77DD99);
                square(ui, x, 0xFF7777);
                square(ui, y, 0x77DD99);
                square(ui, c, 0xFFD077);
            }
            Mode::Rotate if pose.rotatable => {
                for i in 0..48 {
                    let a = i as f32 * std::f32::consts::TAU / 48.0;
                    let b = (i + 1) as f32 * std::f32::consts::TAU / 48.0;
                    line(
                        ui,
                        c + Vec2::from_angle(a) * 52.0,
                        c + Vec2::from_angle(b) * 52.0,
                        0xFFD077,
                    );
                }
                line(
                    ui,
                    c,
                    c + Vec2::from_angle(-pose.heading.to_radians()) * 52.0,
                    0xFFFFFF,
                );
            }
            Mode::Scale if pose.scalable => {
                let end = c + Vec2::new(48.0, -48.0);
                line(ui, c, end, 0xAABBFF);
                square(ui, end, 0xAABBFF);
            }
            _ => {}
        }
        if let Some(d) = &self.gizmo.drag {
            ui.text_fit_left(
                c.x / ui.s + 16.0,
                c.y / ui.s + 30.0,
                340.0,
                type_scale::CAPTION,
                rgb(0xFFFFFF, 1.0),
                &format!(
                    "{:.1}, {:.1} m · {:.0}° · ×{:.2}",
                    d.at.x, d.at.y, d.heading, d.factor
                ),
            );
        }
    }
    pub(super) fn gizmo_input(
        &mut self,
        ui: &Ui,
        renderer: &mut mc_render::Renderer,
        viewport: bool,
    ) -> bool {
        if ui.input.right_pressed && self.gizmo.drag.take().is_some() {
            self.message = "Transform cancelled.".into();
            return true;
        }
        if let Some(mut d) = self.gizmo.drag.take() {
            if ui.input.down {
                d.end = ui.input.cursor;
                update_drag(
                    &mut d,
                    self.gizmo.mode,
                    self.gizmo.grid,
                    &self.camera,
                    renderer,
                );
                self.gizmo.drag = Some(d);
            } else if d.end.distance(d.start) > 2.0 {
                let command = if self.gizmo.mode == Mode::Scale {
                    Command::Scale {
                        id: d.pose.id,
                        factor: d.factor,
                    }
                } else {
                    Command::Transform {
                        id: d.pose.id,
                        at: d.at.to_array(),
                        heading: d.heading,
                    }
                };
                match self.apply(command, renderer) {
                    Ok(()) => {
                        let _ = self.select_object(d.pose.id);
                        self.message = "Transform applied. Undo restores the whole drag.".into();
                    }
                    Err(e) => self.message = e,
                }
            }
            return true;
        }
        if !viewport || !ui.input.pressed || !matches!(self.tool, 0 | 9 | 27) {
            return false;
        }
        let Some(pose) = self.gizmo_pose() else {
            return false;
        };
        let Some([centre, x, y]) = self.gizmo_handles(pose, renderer) else {
            return false;
        };
        let cursor = ui.input.cursor;
        let handle = match self.gizmo.mode {
            Mode::Move => [centre, x, y]
                .iter()
                .position(|p| p.distance(cursor) < 13.0),
            Mode::Rotate if pose.rotatable => {
                ((cursor.distance(centre) - 52.0).abs() < 10.0).then_some(0)
            }
            Mode::Scale if pose.scalable => {
                ((centre + Vec2::new(48.0, -48.0)).distance(cursor) < 15.0).then_some(0)
            }
            _ => None,
        };
        let Some(handle) = handle else {
            return false;
        };
        self.gizmo.drag = Some(Drag {
            pose,
            handle,
            start: cursor,
            centre,
            end: cursor,
            at: pose.at,
            heading: pose.heading,
            factor: 1.0,
        });
        true
    }
}
fn update_drag(
    d: &mut Drag,
    mode: Mode,
    grid: f32,
    camera: &Camera,
    renderer: &mc_render::Renderer,
) {
    match mode {
        Mode::Move => {
            let ground = |cursor| {
                let (o, v) = camera.ray(cursor);
                renderer.pick_ground(o, v).map(|p| p.truncate())
            };
            if let (Some(start), Some(end)) = (ground(d.start), ground(d.end)) {
                let mut delta = end - start;
                if d.handle == 1 {
                    delta.y = 0.0;
                }
                if d.handle == 2 {
                    delta.x = 0.0;
                }
                d.at = d.pose.at + delta;
                if grid > 0.0 {
                    d.at = (d.at / grid).round() * grid;
                }
                if d.handle == 1 {
                    d.at.y = d.pose.at.y;
                }
                if d.handle == 2 {
                    d.at.x = d.pose.at.x;
                }
            }
        }
        Mode::Rotate => {
            let a = d.start - d.centre;
            let b = d.end - d.centre;
            d.heading = d.pose.heading - (b.y.atan2(b.x) - a.y.atan2(a.x)).to_degrees();
            if grid > 0.0 {
                d.heading = (d.heading / 15.0).round() * 15.0;
            }
            d.heading = d.heading.rem_euclid(360.0);
        }
        Mode::Scale => {
            d.factor = ((d.end - d.centre).length() / (d.start - d.centre).length().max(1.0))
                .clamp(0.05, 16.0);
            if grid > 0.0 {
                d.factor = (d.factor * 4.0).round().max(1.0) / 4.0;
            }
        }
    }
}
