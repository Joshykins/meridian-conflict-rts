//! A capital ship's hull as the entity shader draws it: placed, turned, pitched by its
//! spinal lay and rolled by its bank, so effects on its guns are laid where the guns are
//! drawn. Presentation only.

use glam::Vec3;
use mc_data::Weapon;
use mc_sim::mirror::{HousePose, UnitInstance, UNIT_HOUSE_SHIFT};

/// The hull's pitch as the sim lays it (a spinal gun's slot 0, `hull_pitched`), for a
/// capital ship; zero for anything else.
pub(super) fn hull_pitch(u: &UnitInstance, mesh: &str) -> f32 {
    if crate::models::capital_rig(mesh).is_some() {
        u.arm_pitch[1]
    } else {
        0.0
    }
}

/// A hull as entity.wgsl `vs_main` draws it `f` of the way through the tick: at its
/// place, turned by its heading, pitched by a capital ship's spinal lay (`hull_pitch`)
/// and rolled by its bank. Effects on its gun houses are laid in it: laid level, a
/// pitched ship's guns charge and fire well off their barrels.
#[derive(Clone, Copy)]
pub(super) struct DrawnHull {
    pos: Vec3,
    heading: f32,
    pitch: f32,
    bank: f32,
}

impl DrawnHull {
    pub(super) fn of(u: &UnitInstance, mesh: &str, f: f32) -> Self {
        let turn = (u.heading - u.prev_heading + std::f32::consts::PI)
            .rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        let pitch = if crate::models::capital_rig(mesh).is_some() {
            u.arm_pitch[0] + (u.arm_pitch[1] - u.arm_pitch[0]) * f
        } else {
            0.0
        };
        Self {
            pos: Vec3::from(u.prev_pos).lerp(Vec3::from(u.pos), f),
            heading: u.prev_heading + turn * f,
            pitch,
            bank: u._pad2[0] + (u._pad2[1] - u._pad2[0]) * f,
        }
    }

    /// A direction in the hull's frame (+X forward) in the world.
    pub(super) fn turn(&self, v: Vec3) -> Vec3 {
        let (s, c) = self.bank.sin_cos();
        let rolled = Vec3::new(v.x, v.y * c - v.z * s, v.y * s + v.z * c);
        rot_z(rot_xz(rolled, self.pitch), self.heading)
    }

    /// A point on the hull's model (metres, +X forward) in the world.
    pub(super) fn place(&self, local: Vec3) -> Vec3 {
        self.pos + self.turn(local)
    }
}

/// A gun as entity.wgsl draws it `f` of the way through the tick: on a house of its own
/// (a warship's turret, posed by its weapon's `HousePose`), on the unit's turret (turned
/// with it, pitched about its trunnion by the arm), or fixed in the hull; and the hull
/// itself as drawn (`DrawnHull`). Effects on a gun's bores are laid through it.
#[derive(Clone, Copy)]
pub(super) struct GunFrame {
    hull: DrawnHull,
    pivot: Option<Vec3>,
    house: bool,
    turret: bool,
    yaw: f32,
    pitch: f32,
}

impl GunFrame {
    /// Gun `weapon` (`w`) of `u`, whose mesh is `mesh`; `houses` is this tick's
    /// `RenderFrame::houses`. `turret` says a gun without a house of its own turns with
    /// the unit's turret and pitches with its arm (else it lies fixed in the hull).
    pub(super) fn of(
        u: &UnitInstance,
        w: &Weapon,
        weapon: u8,
        houses: &[HousePose],
        mesh: &str,
        turret: bool,
        f: f32,
    ) -> Self {
        let pose = (u.status[1] >> UNIT_HOUSE_SHIFT)
            .checked_sub(1)
            .and_then(|i| houses.get(i as usize))
            .filter(|_| w.mount)
            .and_then(|h| h.pose.get(weapon as usize));
        let pivot = w.pivot.map(|p| Vec3::from(p.to_f32()));
        let turret = pose.is_none() && turret && pivot.is_some();
        let (yaw, pitch) = match pose {
            Some(p) => (lerp_angle(p[0], p[1], f), p[2] + (p[3] - p[2]) * f),
            None if turret => (
                lerp_angle(u.prev_turret_yaw, u.turret_yaw, f),
                u.arm_pitch[0] + (u.arm_pitch[1] - u.arm_pitch[0]) * f,
            ),
            None => (0.0, 0.0),
        };
        Self {
            hull: DrawnHull::of(u, mesh, f),
            pivot,
            house: pose.is_some(),
            turret,
            yaw,
            pitch,
        }
    }

    /// A point on the gun (model metres, authored at rest) in the world.
    pub(super) fn place(&self, local: Vec3) -> Vec3 {
        let on_hull = match self.pivot {
            // A house turns about its pivot; what is in it pitches there too.
            Some(p) if self.house => p + rot_z(rot_xz(local - p, self.pitch), self.yaw),
            // The turret turns about the unit's middle, the gun about its trunnion.
            Some(p) if self.turret => rot_z(p + rot_xz(local - p, self.pitch), self.yaw),
            _ => local,
        };
        self.hull.place(on_hull)
    }

    /// Each of `w`'s muzzles (its `muzzles`, else its one `muzzle`) in the world, an aft
    /// house's mirrored as the sim mirrors them, and the way its bore points there: from
    /// the trunnion out to the muzzle, pitched with the gun.
    pub(super) fn mouths(&self, w: &Weapon) -> Vec<(Vec3, Vec3)> {
        let mouths: Vec<Vec3> = if w.muzzles.is_empty() {
            vec![Vec3::from(w.muzzle.to_f32())]
        } else {
            w.muzzles.iter().map(|m| Vec3::from(m.to_f32())).collect()
        };
        mouths.into_iter().map(|m| self.mouth(w, m)).collect()
    }

    /// The muzzle `m` of `w` (as the unit file gives it) in the world, and its bore there.
    pub(super) fn mouth(&self, w: &Weapon, m: Vec3) -> (Vec3, Vec3) {
        let local = if w.rear {
            Vec3::new(-m.x, -m.y, m.z)
        } else {
            m
        };
        let bore = self.pivot.map_or(Vec3::X, |p| {
            let d = local - p;
            Vec3::new(d.x, 0.0, d.z).normalize_or(Vec3::X)
        });
        let muzzle = self.place(local);
        (
            muzzle,
            (self.place(local + bore) - muzzle).normalize_or(Vec3::X),
        )
    }
}

fn lerp_angle(a: f32, b: f32, f: f32) -> f32 {
    let d = (b - a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
    a + d * f
}

fn rot_z(v: Vec3, a: f32) -> Vec3 {
    let (s, c) = a.sin_cos();
    Vec3::new(v.x * c - v.y * s, v.x * s + v.y * c, v.z)
}

fn rot_xz(v: Vec3, a: f32) -> Vec3 {
    let (s, c) = a.sin_cos();
    Vec3::new(v.x * c - v.z * s, v.y, v.x * s + v.z * c)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A gun on a house of its own charges and fires where its house holds it: the
    /// Flamberge's fore house at rest, and turned to port.
    #[test]
    fn a_house_guns_mouths_follow_its_pose() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let blueprints = mc_data::Blueprints::load(&dir).unwrap();
        let id = blueprints.id_of("regency_t3_battleship").unwrap();
        let bp = blueprints.unit(id);
        let mut u: UnitInstance = bytemuck::Zeroable::zeroed();
        u.pos = [100.0, 50.0, 0.0];
        u.prev_pos = u.pos;
        u.status[1] = 1 << UNIT_HOUSE_SHIFT;
        let mut pose = HousePose::default();
        let mouths = |pose: &HousePose, weapon: u8| {
            let w = &bp.weapons[weapon as usize];
            GunFrame::of(
                &u,
                w,
                weapon,
                std::slice::from_ref(pose),
                &bp.visual.mesh,
                true,
                1.0,
            )
            .mouths(w)
        };
        let near = |a: Vec3, b: Vec3| a.distance(b) < 0.05;
        let m = mouths(&pose, 0);
        assert!(near(m[0].0, Vec3::new(153.0, 48.4, 8.4)), "{:?}", m);
        assert!(near(m[1].0, Vec3::new(153.0, 51.6, 8.4)), "{:?}", m);
        // Turned a quarter to port, the house's muzzles stand out to port of its pivot.
        pose.pose[0] = [std::f32::consts::FRAC_PI_2; 4];
        pose.pose[0][2] = 0.0;
        pose.pose[0][3] = 0.0;
        let m = mouths(&pose, 0);
        assert!(near(m[0].0, Vec3::new(139.6, 65.0, 8.4)), "{:?}", m);
        // The aft house rests turned round: its muzzles are aft of its pivot.
        pose.pose[2] = [std::f32::consts::PI, std::f32::consts::PI, 0.0, 0.0];
        let m = mouths(&pose, 2);
        assert!(
            near(m[0].0 * Vec3::new(1.0, 0.0, 1.0), Vec3::new(49.0, 0.0, 8.4)),
            "{:?}",
            m
        );
    }
}
