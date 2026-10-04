//! The Regency's missile defence as it is drawn, the gravity crush (`SimEvent::MissileLased`
//! from a defender whose faction's `anti_missile_look` is `Gravitic`; ARC's laser is
//! `laser_fx`). The mount's gravity lens takes hold of the missile and crushes it. Each
//! tick of burn is one tick of grip:
//!
//! - **The tether** (`fade_beam::GRAVITY_TETHER`, sprites.wgsl): a thin hard red line from
//!   the mount to the missile, lensing ripples running out along it, a faint prism shimmer
//!   at its edges; struck bright and gone fast, as the laser.
//! - **The grip** (`puff::CRUSH_LENS`, plasma_puffs.wgsl): a ring of bent light round the
//!   missile that tightens over its short life, laid each tick, so a burn of several ticks
//!   reads as a ring closing in.
//! - **The mount:** a small, soft red-white flare.
//! - **The kill:** the ring snaps shut, then the missile pops: a hard white heart in a red
//!   burst, filaments torn outward, sparks, a short red light. No smoke, no shockwave.
//!
//! A burn the sim lets go of without a kill leaves only the ring fading. Only the look:
//! what dies, and when, is the sim's. Stateless; presentation only, the renderer's clock.

use super::lens_flare::Flare;
use super::{FadeBeam, Renderer};
use crate::gpu_consts::{fade_beam, puff};
use glam::Vec3;
use mc_core::FxVec3;
use mc_data::{AntiMissileLook, BlueprintId};

const LENS: f32 = puff::CRUSH_LENS as f32;
const BURST: f32 = puff::PLASMA_BURST as f32;
const GLOW: f32 = puff::WARP_GLOW as f32;
const STREAK: f32 = puff::WARP_STREAK as f32;
const MOTE: f32 = puff::WARP_MOTE as f32;

/// Missile-defence red, its pink-white heart, and white: brightness in their size.
const RED: Vec3 = Vec3::new(1.0, 0.07, 0.04);
const HOT: Vec3 = Vec3::new(1.0, 0.55, 0.5);
const WHITE: Vec3 = Vec3::new(1.0, 0.96, 1.0);

/// The tether's thickness in the world, metres, and how long it takes to fade, seconds.
const TETHER_WIDTH: f32 = 0.7;
const TETHER_LIFE: f32 = 0.2;
/// The grip's ring: its radius in metres as it is laid and as it has closed (the rim sits
/// at 0.74 of it), and how long it lives, seconds.
const GRIP: (f32, f32) = (9.0, 3.2);
const GRIP_LIFE: f32 = 0.15;
/// The kill: the ring snapping shut, and the pop after it.
const SNAP: (f32, f32) = (6.5, 0.4);
const SNAP_LIFE: f32 = 0.07;
/// The pop's size, metres.
const POP: f32 = 3.0;

/// The glint at the mount each tick, and the pop's.
const MOUNT_GLINT: Flare = Flare {
    color: Vec3::new(6.0, 2.6, 2.2),
    size: 18.0,
};
const POP_GLINT: Flare = Flare {
    color: Vec3::new(4.0, 1.0, 0.7),
    size: 10.0,
};

impl Renderer {
    /// A tick of missile defence (`MissileLased`) from `defender`, drawn as a gravity crush
    /// when its faction's look is `Gravitic`: the mount at `from`, the round at `to`. True
    /// when this drew it (and the laser should not).
    pub(super) fn missile_crushed(
        &mut self,
        defender: BlueprintId,
        from: &FxVec3,
        to: &FxVec3,
        killed: bool,
        time: f32,
    ) -> bool {
        let faction = self.blueprints.unit(defender).faction;
        let look = self
            .blueprints
            .factions
            .get(faction.0 as usize)
            .map_or(AntiMissileLook::Laser, |f| f.anti_missile_look);
        if look != AntiMissileLook::Gravitic {
            return false;
        }
        let mount = Vec3::from(from.to_f32());
        let at = Vec3::from(to.to_f32());
        self.fade_beams.push(FadeBeam {
            from: mount,
            to: at,
            start: time,
            life: TETHER_LIFE,
            width: TETHER_WIDTH,
            kind: fade_beam::GRAVITY_TETHER,
        });
        // The mount's lens flaring as it takes hold, softer than the laser's head.
        self.push_effect(mount.to_array(), time, 2.4, 0.16, 8.0, 0.0);
        self.lens_flares.flash(mount, MOUNT_GLINT, time, 0.16);
        if killed {
            self.push_lit(
                LENS,
                at,
                Vec3::ZERO,
                time,
                SNAP_LIFE,
                SNAP,
                RED.lerp(HOT, 0.3) * 4.5,
                0.0,
            );
            self.crush_pop(at, time + SNAP_LIFE);
        } else {
            self.push_lit(LENS, at, Vec3::ZERO, time, GRIP_LIFE, GRIP, RED * 3.2, 0.0);
        }
        true
    }

    /// The crushed round letting go at `at`: a hard white heart in a red burst, filaments
    /// torn outward, sparks thrown off, a short red light. Small and gone fast.
    fn crush_pop(&mut self, at: Vec3, start: f32) {
        let s = POP;
        self.lens_flares.flash(at, POP_GLINT, start, 0.14);
        self.push_lit(
            GLOW,
            at,
            Vec3::ZERO,
            start,
            0.07,
            (s * 0.3, s * 0.55),
            WHITE * 4.0,
            0.0,
        );
        self.push_lit(
            BURST,
            at,
            Vec3::ZERO,
            start,
            0.24,
            (s * 0.35, s * 1.7),
            RED * 5.0,
            0.0,
        );
        for _ in 0..6 {
            let out = self.crush_scatter(0.0);
            let reach = s * (0.5 + 0.4 * self.scatter.unit());
            self.push_lit(
                STREAK,
                at,
                out * reach,
                start,
                0.14,
                (0.08, 0.04),
                RED.lerp(HOT, 0.4) * 4.5,
                0.0,
            );
        }
        for _ in 0..10 {
            let out = self.crush_scatter(0.1);
            let speed = 16.0 + 24.0 * self.scatter.unit();
            self.push_lit(
                MOTE,
                at,
                out * speed,
                start,
                0.35,
                (0.2, 0.07),
                RED.lerp(HOT, 0.3) * 4.0,
                0.0,
            );
        }
        self.gravitic_glow(at, RED * 220.0, 16.0, start, 0.2);
    }

    /// A random direction, `lift` added upward before it is made unit length.
    fn crush_scatter(&mut self, lift: f32) -> Vec3 {
        Vec3::new(
            self.scatter.signed(),
            self.scatter.signed(),
            self.scatter.signed() + lift,
        )
        .normalize_or(Vec3::Z)
    }
}
