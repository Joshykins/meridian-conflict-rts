use super::*;
impl Editor {
    pub(super) fn world_command(
        &mut self,
        at: [f32; 2],
        cursor: Vec2,
        renderer: &mc_render::Renderer,
    ) -> Option<Command> {
        match self.tool {
            1..=4 => Some(Command::Terrain {
                at,
                radius: self.radius,
                strength: self.strength,
                brush: [Brush::Raise, Brush::Lower, Brush::Flatten, Brush::Smooth][self.tool - 1],
            }),
            13 | 14 => Some(Command::Terrain {
                at,
                radius: self.radius,
                strength: self.strength,
                brush: if self.tool == 13 {
                    Brush::Cliff
                } else {
                    Brush::Noise
                },
            }),
            15 => match self.region_start.take() {
                None => {
                    self.region_start = Some(at);
                    self.message =
                        "Click the other endpoint to connect the two terrain heights.".into();
                    None
                }
                Some(from) => Some(Command::Ramp {
                    from,
                    to: at,
                    width: self.radius,
                }),
            },
            16 => Some(Command::Stamp {
                at,
                radius: self.radius,
                strength: self.strength,
                asset: self.stamp_asset.clone(),
            }),
            17..=20 => Some(Command::Paint {
                at,
                radius: self.radius,
                paint: [
                    document::Paint::Ground,
                    document::Paint::Snow,
                    document::Paint::Ice,
                    document::Paint::Trail,
                ][self.tool - 17],
                value: if self.tool == 17 {
                    self.paint.ground
                } else {
                    self.paint.value
                },
                heading: self.heading,
            }),
            21 => match self.region_start.take() {
                None => {
                    self.region_start = Some(at);
                    self.message = "Click the road endpoint.".into();
                    None
                }
                Some(from) => Some(Command::Road {
                    from,
                    to: at,
                    width: self.paint.width,
                    kind: self.paint.road,
                }),
            },
            5 => Some(Command::Unit {
                name: self.name.clone(),
                group: self.group.clone(),
                blueprint: self.blueprint.clone(),
                owner: self.owner,
                at,
                heading: self.heading,
            }),
            6 => match self.region_start.take() {
                None => {
                    self.region_start = Some(at);
                    self.message = "Click the opposite corner of the region.".into();
                    None
                }
                Some(a) => Some(Command::Region {
                    name: self.name.clone(),
                    min: [a[0].min(at[0]), a[1].min(at[1])],
                    max: [a[0].max(at[0]), a[1].max(at[1])],
                }),
            },
            7 => Some(Command::Point {
                name: self.name.clone(),
                at,
            }),
            8 => Some(Command::Camera {
                name: self.name.clone(),
                at,
                distance: self.camera.distance.round().clamp(25.0, 300_000.0) as u32,
                yaw: self.camera.yaw.to_degrees(),
            }),
            9 => self.selected.map(|id| Command::Transform {
                id,
                at,
                heading: self.heading,
            }),
            _ => {
                self.pick(cursor, renderer);
                None
            }
        }
    }
}
