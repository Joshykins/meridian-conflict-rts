//! The ground stakes a siege unit fires into the ground to plant (`gpu_consts::stake`):
//! where they are, the order they fire in and when each strikes, for the model that
//! builds them, the renderer and the sound that mark each strike, and the tests. The
//! vertex shader (`entity.wgsl` `stake_pose`) poses them from the same constants.

use crate::gpu_consts::stake;
use crate::{rig, Model};
use glam::Vec3;

/// One corner's stake.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stake {
    /// At the front of the carriage (+x), and on its left (+y).
    pub front: bool,
    pub left: bool,
}

/// Every corner, in firing order.
pub const STAKES: [Stake; 4] = [
    Stake {
        front: true,
        left: true,
    },
    Stake {
        front: false,
        left: false,
    },
    Stake {
        front: true,
        left: false,
    },
    Stake {
        front: false,
        left: true,
    },
];

impl Stake {
    /// Its place in the firing order: one diagonal and then the other, front first.
    pub fn order(self) -> usize {
        let diagonal = self.front == self.left;
        (if diagonal { 0 } else { 2 }) + usize::from(!self.front)
    }

    /// The share of the deploy at which its tube starts to swing down.
    pub fn start(self) -> f32 {
        stake::START + self.order() as f32 * stake::STEP
    }

    /// The share of the deploy at which its spike strikes the ground.
    pub fn strikes(self) -> f32 {
        self.start() + stake::FIRE + stake::FIRE_TIME
    }

    /// Its tube's hinge, at the authored 1.88 m deck.
    pub fn hinge(self) -> Vec3 {
        let x = if self.front {
            stake::FRONT_X
        } else {
            stake::REAR_X
        };
        let y = if self.left { stake::Y } else { -stake::Y };
        Vec3::new(x, y, stake::Z)
    }

    /// The way its planted tube points, down its line.
    pub fn down(self) -> Vec3 {
        let sx = if self.front { 1.0 } else { -1.0 };
        let sy = if self.left { 1.0 } else { -1.0 };
        Vec3::new(sx * stake::OUT_X, sy * stake::OUT_Y, -stake::DOWN).normalize()
    }

    /// Where its line meets flat ground, at the authored deck.
    pub fn strike_point(self) -> Vec3 {
        let down = self.down();
        self.hinge() + down * (stake::Z / -down.z)
    }
}

/// Whether a model plants ground stakes, and so how much it is scaled from the authored
/// deck the stakes are placed for (its turret pivot's height over 1.88 m, as the shader
/// scales them).
pub fn stake_scale(model: &Model) -> Option<f32> {
    model.lods[0]
        .vertices
        .iter()
        .any(|v| v.rig & rig::DEPLOY != 0 && v.rig & rig::STAKE != 0)
        .then(|| {
            if model.turret_pivot[2] > 0.5 {
                model.turret_pivot[2] / 1.88
            } else {
                1.0
            }
        })
}
