//! The nuclear fireballs lighting the clouds (`Atmosphere::fires`): gathered once a
//! frame from the blasts the renderer hands the shaders (`Globals::nukes`, shapes as
//! nuke.wgsl draws them). Gathered by every ray of the march into arrays of its own,
//! they held its registers even with no fire burning, and every ray ran slower.

use super::*;

/// Fireballs lighting the clouds at most.
pub(super) const SLOTS: usize = crate::gpu_consts::cloud_fires::SLOTS as usize;

/// A fireball's colour as it cools, 1 white-hot to 0 a dull red.
fn fire_tint(heat: f32) -> Vec3 {
    let h = heat.clamp(0.0, 1.0);
    let c = Vec3::new(0.9, 0.16, 0.03).lerp(Vec3::new(1.0, 0.45, 0.08), smoothstep(0.0, 0.4, h));
    let c = c.lerp(Vec3::new(1.0, 0.78, 0.4), smoothstep(0.4, 0.75, h));
    c.lerp(Vec3::new(1.0, 0.96, 0.9), smoothstep(0.75, 1.0, h))
}

/// A Regency nova's light on the clouds (nova.wgsl `plasma_color`): white, rose, red,
/// crimson.
fn plasma_tint(heat: f32) -> Vec3 {
    let h = heat.clamp(0.0, 1.0);
    let c = Vec3::new(0.85, 0.035, 0.05).lerp(Vec3::new(1.0, 0.19, 0.09), smoothstep(0.0, 0.4, h));
    let c = c.lerp(Vec3::new(1.0, 0.6, 0.64), smoothstep(0.4, 0.75, h));
    c.lerp(Vec3::new(1.0, 0.95, 0.97), smoothstep(0.75, 1.0, h))
}

impl Sky {
    /// The fireballs of this frame's blasts, `nukes` as `Globals::nukes` (four vec4 a
    /// blast, `count` in use) at `now`: after `update`, before the clouds are drawn.
    pub fn set_fires(&mut self, nukes: &[[f32; 4]], count: usize, now: f32) {
        let mut fires = [[0.0f32; 4]; SLOTS * 2];
        let mut burning = 0;
        for blast in nukes.as_chunks::<4>().0.iter().take(count) {
            if burning == SLOTS {
                // A deliberate cap: past this many fireballs, more light is lost in the glare.
                break;
            }
            let [a, b, c, d] = *blast;
            let age = (now - a[3]).max(0.0);
            let scale = b[0].max(0.05);
            let k = scale.max(0.3).sqrt();
            // nuke.wgsl head_radius, head_height, heat_left.
            let reach = scale
                * (240.0 * (1.0 - (-age * 3.0).exp()).sqrt() + 430.0 * (1.0 - (-age / 26.0).exp()));
            let lift = scale.min(scale.powf(0.6));
            let high = lift * 1700.0 * (1.0 - (-(age / 30.0).powf(1.35)).exp());
            // A salvo's fire folded in and bursts near it (nuke_fx.rs): `d.x`.
            let heat = ((-age / (2.5 * k)).exp() * 0.5 + (-age / (16.0 * k)).exp() * 0.5).max(d[0]);
            let glare = (-age / (1.6 * k)).exp().max(d[0] * d[0] * d[0] * 0.6);
            let blaze = (-age / (6.0 * k)).exp();
            let power = 30.0 * glare + 7.0 * blaze + 1.2 * heat * heat;
            if power < 0.02 {
                continue;
            }
            let temp = 0.3 * (-age * 2.0).exp() + 0.75 * heat + 0.1;
            let lean = Vec2::new(c[0], c[1]) * (high / (1600.0 * lift)).clamp(0.0, 1.0);
            fires[burning * 2] = [a[0] + lean.x, a[1] + lean.y, a[2] + high, reach];
            // A Regency nova's threads glow on for half a minute (nova.wgsl `nova_glow`).
            let light = if (d[1] + 0.5) as u32 == crate::gpu_consts::nuke_look::PLASMA {
                let threads = 0.4 * (-age / (8.0 * k)).exp() + 0.6 * (-age / (28.0 * k)).exp();
                plasma_tint(temp) * (power + 0.8 * threads)
            } else {
                fire_tint(temp) * power
            };
            fires[burning * 2 + 1] = light.extend(0.0).to_array();
            burning += 1;
        }
        let at = std::mem::offset_of!(Atmosphere, fires) as u64;
        self.atmos.write(at, bytemuck::cast_slice(&fires));
        let at = std::mem::offset_of!(Atmosphere, fire_count) as u64;
        self.atmos
            .write(at, bytemuck::bytes_of(&[burning as f32, 0.0, 0.0, 0.0]));
    }
}
