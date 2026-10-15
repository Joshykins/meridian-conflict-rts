//! A unit built bigger than its model was authored (`scale:` in a unit file): its
//! radius, height and every point the file places on the model (muzzles, pivots, mounts,
//! build and reclaim arms, lamps, drone sockets) are multiplied on load, so the numbers in
//! the file stay the model's own and the renderer, fitting the mesh to the radius and
//! height, draws it to match. Ranges, speeds and everything else that is play, not the
//! model, are left as written.

use crate::raw::{RawBuilder, RawReclaimer, RawWeapon, Unit};
use crate::{BlueprintId, DataError, FactionId, UnitBlueprint};

/// The scales a unit file may ask for.
const RANGE: std::ops::RangeInclusive<f64> = 0.25..=4.0;

impl Unit {
    /// Compiles the unit at its `scale` (`compile_authored` for the file as written).
    pub(crate) fn compile(
        &self,
        id: BlueprintId,
        faction: FactionId,
        lookup: &dyn Fn(&str, &str) -> Result<BlueprintId, DataError>,
    ) -> Result<UnitBlueprint, DataError> {
        let Some(k) = self.scale else {
            return self.compile_authored(id, faction, lookup);
        };
        let key = &self.key;
        if !k.is_finite() || !RANGE.contains(&k) {
            return Err(DataError::Invalid(format!(
                "{key}: scale {k} is outside {}..={}",
                RANGE.start(),
                RANGE.end()
            )));
        }
        // What `scaled` does not reach would be left at the authored size, out of step
        // with the model: refuse it rather than draw it wrong.
        let unscaled = [
            ("refits", !self.refits.is_empty()),
            ("transport", self.transport.is_some()),
            ("staging", self.staging.is_some()),
            ("gravitic", self.gravitic.is_some()),
            ("strategic", self.strategic.is_some()),
            ("stomp", self.stomp.is_some()),
            ("edict", self.edict.is_some()),
        ];
        if let Some((what, _)) = unscaled.iter().find(|(_, has)| *has) {
            return Err(DataError::Invalid(format!(
                "{key}: scale does not reach its {what} yet"
            )));
        }
        let mut bp = self.scaled(k).compile_authored(id, faction, lookup)?;
        bp.scale = k as f32;
        Ok(bp)
    }

    /// The unit file with every model-space length multiplied by `k`.
    fn scaled(&self, k: f64) -> Unit {
        let p3 = |p: &mut (f64, f64, f64)| *p = (p.0 * k, p.1 * k, p.2 * k);
        let mut u = self.clone();
        u.radius *= k;
        u.height *= k;
        if let Some(h) = &mut u.hull {
            *h = (h.0 * k, h.1 * k);
        }
        if let Some(b) = &mut u.body {
            b.fore *= k;
            b.aft *= k;
            b.beam *= k;
        }
        if let Some(t) = &mut u.turret_at {
            *t = (t.0 * k, t.1 * k);
        }
        u.anti_missile_mounts.iter_mut().for_each(p3);
        u.drone_sockets.iter_mut().for_each(p3);
        u.drone_approach *= k;
        for l in u.lights.iter_mut().flatten() {
            l.at = l.at.map(|v| v * k as f32);
        }
        u.weapons.iter_mut().for_each(|w| scale_weapon(w, k));
        if let Some(b) = &mut u.builder {
            scale_builder(b, k);
        }
        if let Some(r) = &mut u.reclaimer {
            scale_reclaimer(r, k);
        }
        u
    }
}

fn scale_weapon(w: &mut RawWeapon, k: f64) {
    let p3 = |p: &mut (f64, f64, f64)| *p = (p.0 * k, p.1 * k, p.2 * k);
    p3(&mut w.muzzle);
    w.muzzles.iter_mut().for_each(p3);
    w.pivot.iter_mut().for_each(p3);
    w.mounts.iter_mut().for_each(|m| p3(&mut m.pivot));
    if let Some(h) = &mut w.howitzer {
        h.length *= k;
        h.radius *= k;
    }
    if let Some(s) = &mut w.sabot {
        p3(&mut s.port);
    }
}

fn scale_builder(b: &mut RawBuilder, k: f64) {
    let p3 = |p: &mut (f64, f64, f64)| *p = (p.0 * k, p.1 * k, p.2 * k);
    b.emitters.iter_mut().for_each(p3);
    b.hinge.iter_mut().for_each(p3);
    if let Some(a) = &mut b.arm {
        p3(&mut a.emitter);
        a.pivot.iter_mut().for_each(p3);
        a.shoulder.iter_mut().for_each(p3);
    }
}

fn scale_reclaimer(r: &mut RawReclaimer, k: f64) {
    let p3 = |p: &mut (f64, f64, f64)| *p = (p.0 * k, p.1 * k, p.2 * k);
    for h in &mut r.heads {
        p3(&mut h.emitter);
        h.pivot.iter_mut().for_each(p3);
    }
}

#[cfg(test)]
mod tests {
    use mc_core::{Fx, FxVec3};

    use crate::{BlueprintId, DataError, FactionId, UnitBlueprint};

    /// A tank with a turret gun, a lamp and a build arm, at `scale` (RON, or empty).
    fn tank(scale: &str, extra: &str) -> Result<UnitBlueprint, DataError> {
        let text = format!(
            "(key: \"tank\", name: \"Tank\", role: \"Test\", tech: 1, categories: [\"Land\"],
              health: 100, cost: (materials: 1, energy: 1, time: 1), radius: 4, height: 3,
              vision: 300, {scale} {extra}
              motion: (layer: Land, size: 0, speed: 30, accel: 30, turn: 100),
              builder: (power: 5, range: 50, builds: [], arm: (turn: 90, emitter: (1, 0, 2),
                pivot: (0, 0, 2))),
              lights: [(kind: Point, at: (2, 1, 1))],
              weapons: [(name: \"Gun\", damage: 10, range: 300, reload: 1, speed: 300,
                trajectory: Direct, muzzle: (5, 0, 2), muzzles: [(5, 1, 2)], pivot: (0, 0, 2),
                targets: [\"Land\"], color: Orange)],
              mesh: \"tank\", icon: Tank)"
        );
        let unit: crate::raw::Unit = ron::Options::default()
            .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
            .from_str(&text)
            .unwrap();
        unit.compile(BlueprintId(0), FactionId(0), &|_, _| Ok(BlueprintId(0)))
    }

    /// The model's size and every point the file places on it grow by the scale; what is
    /// play (range, vision, speed) stays as written.
    #[test]
    fn a_scaled_unit_is_its_model_times_the_scale() {
        let one = tank("", "").unwrap();
        let two = tank("scale: 2,", "").unwrap();
        assert_eq!(one.scale, 1.0);
        assert_eq!(two.scale, 2.0);
        assert_eq!((two.radius, two.height), (one.radius * 2, one.height * 2));
        assert_eq!(two.hull, (Fx::from_int(8), Fx::from_int(8)));
        let (a, b) = (&one.weapons[0], &two.weapons[0]);
        assert_eq!(b.muzzle, a.muzzle * Fx::from_int(2));
        assert_eq!(
            b.muzzles,
            vec![FxVec3::new(
                Fx::from_int(10),
                Fx::from_int(2),
                Fx::from_int(4)
            )]
        );
        assert_eq!(b.pivot, a.pivot.map(|p| p * Fx::from_int(2)));
        assert_eq!(b.range_max, a.range_max);
        assert_eq!(two.vision, one.vision);
        assert_eq!(two.motion.unwrap().speed, one.motion.unwrap().speed);
        let arm = |u: &UnitBlueprint| u.builder.as_ref().unwrap().arm.unwrap();
        assert_eq!(arm(&two).emitter, arm(&one).emitter * Fx::from_int(2));
        assert_eq!(
            two.builder.as_ref().unwrap().range,
            one.builder.as_ref().unwrap().range
        );
        let lamp = |u: &UnitBlueprint| u.visual.lights.as_ref().unwrap()[0].at;
        assert_eq!(lamp(&two), [4.0, 2.0, 2.0]);
    }

    #[test]
    fn a_scale_out_of_reason_or_past_what_it_reaches_is_refused() {
        assert!(tank("scale: 0,", "").is_err());
        assert!(tank("scale: 9,", "").is_err());
        assert!(tank(
            "scale: 1.5,",
            "drone_sockets: [], stomp: (pace: 5, reach: 5, gauge: 2, radius: 5, damage: 5),"
        )
        .is_err());
    }
}
