//! The Naga's squeezed plasma guns firing and striking (docs/STYLE.md "The Naga suite"):
//! the Pinched-plasmeric and Pinch-fusion grades (`Weapon::plasma_grade`) of a direct-fire
//! gun. In flight their slugs are sprites.wgsl's (`mirror::plasma_look`); the gun's own
//! red flash, smoke and the shell's blast where it lands are drawn as any gun's. This adds
//! what the grade is:
//!
//! - **Firing: the gun kicks like a cannon.** A white-hot knot at the bore, the pressure
//!   vented out of side ports behind it as two hard red jets and grey smoke, and heat
//!   haze rising off the barrel; the gun's own pressure wave (`shockwave`) is the generic one.
//! - **Pinched-plasmeric strike: the bind breaks at once.** A sharp white flash, over in an
//!   instant, a hard shock ring, molten spatter thrown out low, and a glassed scorch that
//!   glows red and crusts over. It stops in what it hits: no splash to speak of.
//! - **Pinch-fusion strike: the fusion lets go.** A white fusion flash that holds a beat
//!   and a second, wider one after it, a heavier ring, more spatter, a wider glassed pool.
//!
//! Presentation only; the renderer's own clock. Driven by the weapon's data, never a unit.

use glam::Vec3;
use mc_data::{BlueprintId, PlasmaGrade, Weapon};

use super::{rail_fx, Renderer, PUFF_SMOKE, PUFF_SPARK, PUFF_TREE_SMOKE};

/// Seconds a squeezed slug's glassed scorch takes to crust over and cool.
const GLASS_COOL: f32 = 7.0;
/// A red flash (sprites.wgsl effect kind 8).
const RED_FLASH: f32 = 8.0;

/// The grade of a direct-fire squeezed plasma gun: Pinched or Pinch-fusion. None for a
/// Plasmeric gun, a beam, a thrown charge, or anything not plasma.
fn squeezed(weapon: &Weapon) -> Option<PlasmaGrade> {
    match weapon.plasma_grade {
        Some(g @ (PlasmaGrade::Pinched | PlasmaGrade::PinchFusion))
            if !weapon.beam && !weapon.missile && weapon.curve.0 == 0 =>
        {
            Some(g)
        }
        _ => None,
    }
}

impl Renderer {
    /// A squeezed plasma gun fired (`ShotFired`): `at` its muzzle as drawn, `dir` down the bore.
    pub(super) fn pinch_fired(
        &mut self,
        blueprint: BlueprintId,
        weapon: u8,
        at: Vec3,
        dir: Vec3,
        time: f32,
    ) {
        let w = &self.blueprints.unit(blueprint).weapons[weapon as usize];
        let Some(grade) = squeezed(w) else {
            return;
        };
        let fusion = grade == PlasmaGrade::PinchFusion;
        let power = w.damage.to_f32().max(1.0).sqrt();
        let flash = w.flash;
        let right = dir.cross(Vec3::Z).normalize_or(Vec3::Y);
        // The knot of squeezed plasma at the bore, white-hot and gone at once.
        self.push_effect(
            (at + dir * 0.8).to_array(),
            time,
            (0.7 + power * 0.07) * flash * if fusion { 1.4 } else { 1.0 },
            if fusion { 0.1 } else { 0.07 },
            rail_fx::RAIL_FLASH,
            0.35,
        );
        // The side ports vent the pressure behind the muzzle: a hard red jet each side.
        let port = at - dir * (1.2 + power * 0.03);
        for side in [-1.0, 1.0] {
            let out = right * side;
            self.push_effect(
                (port + out * 0.9).to_array(),
                time,
                (0.5 + power * 0.05) * flash,
                0.09,
                RED_FLASH,
                0.0,
            );
            for k in 0..3 {
                let spray = (out * 1.6 + dir * 0.3 + Vec3::Z * 0.25).normalize_or_zero();
                let speed = 12.0 + self.scatter.unit() * 10.0;
                let drift = Vec3::new(self.scatter.signed(), self.scatter.signed(), 0.5);
                let life = 0.55 + 0.2 * self.scatter.unit();
                self.push_puff(
                    PUFF_SMOKE,
                    port + out * 0.8,
                    spray * speed + drift,
                    time + 0.015 * k as f32,
                    life,
                    (0.35 + power * 0.02, 1.4 + power * 0.06),
                );
            }
            for _ in 0..3 {
                let spray = (out
                    + Vec3::new(
                        self.scatter.signed() * 0.3,
                        self.scatter.signed() * 0.3,
                        self.scatter.unit() * 0.4,
                    ))
                .normalize_or_zero();
                let speed = 18.0 + self.scatter.unit() * 16.0;
                self.push_puff(PUFF_SPARK, port, spray * speed, time, 0.18, (0.16, 0.05));
            }
        }
        // Heat off the barrel: a faint haze that rises and thins.
        for k in 0..2 {
            let rise = 2.0 + self.scatter.unit();
            self.push_puff(
                PUFF_TREE_SMOKE,
                at - dir * (0.5 + k as f32 * 1.5) + Vec3::Z * 0.6,
                Vec3::Z * rise,
                time + 0.05,
                1.1,
                (0.5, 1.6),
            );
        }
    }

    /// A squeezed plasma slug struck (`Impact`), after the shell's own blast.
    pub(super) fn pinch_landed(
        &mut self,
        blueprint: BlueprintId,
        weapon: u8,
        at: Vec3,
        on_unit: bool,
        start: f32,
    ) {
        let w = &self.blueprints.unit(blueprint).weapons[weapon as usize];
        let Some(grade) = squeezed(w) else {
            return;
        };
        let fusion = grade == PlasmaGrade::PinchFusion;
        let power = w.damage.to_f32().max(1.0).sqrt();
        let size = (2.0 + power * 0.12 + w.splash.to_f32() * 0.4) * w.impact;
        // The bind breaking: a sharp white flash, over at once.
        self.push_effect(at.to_array(), start, size, 0.1, rail_fx::RAIL_FLASH, 0.9);
        self.push_shockwave(
            at.to_array(),
            start,
            size * if fusion { 5.0 } else { 3.2 },
            if fusion { 0.55 } else { 0.35 },
            if fusion { 1.0 } else { 0.75 },
            1.0,
            Vec3::ZERO,
        );
        if fusion {
            // The fusion letting go: it holds a beat, and a wider second flash follows.
            self.push_effect(
                (at + Vec3::Z * 0.8).to_array(),
                start + 0.05,
                size * 1.7,
                0.3,
                rail_fx::RAIL_FLASH,
                1.0,
            );
            self.push_effect(
                (at + Vec3::Z * 1.2).to_array(),
                start + 0.12,
                size * 2.2,
                0.45,
                RED_FLASH,
                0.6,
            );
        }
        // Molten spatter, thrown out low and hard.
        let spatter = if fusion { 16 } else { 9 };
        for _ in 0..spatter {
            let spray = Vec3::new(
                self.scatter.signed(),
                self.scatter.signed(),
                0.25 + self.scatter.unit() * 0.6,
            )
            .normalize_or_zero();
            let speed = 22.0 + self.scatter.unit() * if fusion { 40.0 } else { 26.0 };
            let life = 0.55 + self.scatter.unit() * 0.4;
            self.push_puff(
                PUFF_SPARK,
                at + Vec3::Z * 0.3,
                spray * speed,
                start,
                life,
                (0.28, 0.1),
            );
        }
        if !on_unit {
            // A glassed scorch that glows red and crusts over.
            self.bore_fx.melt(
                at.truncate(),
                size * if fusion { 1.1 } else { 0.7 },
                start,
                GLASS_COOL * if fusion { 1.5 } else { 1.0 },
            );
        }
    }
}
