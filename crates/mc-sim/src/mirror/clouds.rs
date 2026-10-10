//! The storm seeds in the air (`RenderFrame::clouds`, renderer `storm_cloud_fx`): each
//! cloud a Storm Seeder let go, where it is, how it drifts and how far through its life it
//! is (`cloud_seed.rs`). Its bolts come as `SimEvent::CloudBolt`.

use super::World;

/// One storm seed as the renderer draws it.
#[derive(Clone, Copy, Debug)]
pub struct CloudView {
    /// The shot's serial: never zero, never reused in a match.
    pub serial: u32,
    pub owner: u8,
    /// Its middle this tick, and its drift, metres a tick.
    pub pos: [f32; 3],
    pub vel: [f32; 3],
    /// Ticks since it was let go; ticks it grows for; ticks it lives in all.
    pub age: u16,
    pub grow: u16,
    pub life: u16,
    /// Metres round its middle its bolts fall.
    pub radius: f32,
}

impl World {
    /// Every storm seed in the air.
    pub(super) fn write_clouds(&self, out: &mut Vec<CloudView>) {
        out.clear();
        let p = &self.state.projectiles;
        for i in (0..p.len()).filter(|&i| p.sub[i] == 0) {
            let Some(cloud) =
                self.blueprints.unit(p.blueprint[i]).weapons[p.weapon[i] as usize].cloud
            else {
                continue;
            };
            out.push(CloudView {
                serial: p.serial[i],
                owner: p.owner[i],
                pos: p.pos[i].to_f32(),
                vel: p.vel[i].to_f32(),
                age: p.age[i],
                grow: cloud.grow_ticks,
                life: cloud.life_ticks(),
                radius: cloud.radius.to_f32(),
            });
        }
    }
}
