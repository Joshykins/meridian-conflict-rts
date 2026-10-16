//! A unit built bigger than its model was authored (`scale:` in a unit file): its
//! radius, height and every point the file places on the model (muzzles, pivots, mounts,
//! build and reclaim arms, lamps, drone sockets) are multiplied on load, so the numbers in
//! the file stay the model's own and the renderer, fitting the mesh to the radius and
//! height, draws it to match. A carrier's landing and launch points and a hull
//! launcher's tubes count among those points. Ranges, speeds and everything else that is play, not the
//! model, are left as written.
//!
//! A unit drawn bigger than its body (`draw_scale:`) has its model and the points on it
//! grown and its body left as it is: what it collides with, the room it takes in a hold.

use crate::raw::{RawBuilder, RawReclaimer, RawWeapon, Unit};
use crate::{BlueprintId, DataError, FactionId, UnitBlueprint};

/// The scales a unit file may ask for.
const RANGE: std::ops::RangeInclusive<f64> = 0.25..=4.0;

impl Unit {
    /// Compiles the unit at its `scale` and `draw_scale` (`compile_authored` for the file
    /// as written).
    pub(crate) fn compile(
        &self,
        id: BlueprintId,
        faction: FactionId,
        lookup: &dyn Fn(&str, &str) -> Result<BlueprintId, DataError>,
    ) -> Result<UnitBlueprint, DataError> {
        if self.scale.is_none() && self.draw_scale.is_none() {
            return self.compile_authored(id, faction, lookup);
        }
        let key = &self.key;
        let (k, d) = (self.scale.unwrap_or(1.0), self.draw_scale.unwrap_or(1.0));
        for (what, v) in [("scale", k), ("draw_scale", d)] {
            if !v.is_finite() || !RANGE.contains(&v) {
                return Err(DataError::Invalid(format!(
                    "{key}: {what} {v} is outside {}..={}",
                    RANGE.start(),
                    RANGE.end()
                )));
            }
        }
        // What `scaled` does not reach would be left at the authored size, out of step
        // with the model: refuse it rather than draw it wrong. Refits need nothing here:
        // every kit and loadout is a unit of its own, its modules' points merged in before
        // it comes through this (`refit::expand`), so they are scaled once, with it.
        let unscaled = [
            ("transport", self.transport.is_some()),
            ("gravitic", self.gravitic.is_some()),
            ("stomp", self.stomp.is_some()),
        ];
        if let Some((what, _)) = unscaled.iter().find(|(_, has)| *has) {
            return Err(DataError::Invalid(format!(
                "{key}: scale does not reach its {what} yet"
            )));
        }
        let mut bp = self
            .scaled(k, k * d)
            .compile_authored(id, faction, lookup)?;
        bp.scale = (k * d) as f32;
        bp.visual.draw = d as f32;
        Ok(bp)
    }

    /// The unit file with its body (radius, height, the hull it blocks and is struck on)
    /// multiplied by `body`, and every point on its model by `k`.
    fn scaled(&self, body: f64, k: f64) -> Unit {
        let p3 = |p: &mut (f64, f64, f64)| *p = (p.0 * k, p.1 * k, p.2 * k);
        let mut u = self.clone();
        u.radius *= body;
        u.height *= body;
        if let Some(h) = &mut u.hull {
            *h = (h.0 * body, h.1 * body);
        }
        if let Some(b) = &mut u.body {
            b.fore *= body;
            b.aft *= body;
            b.beam *= body;
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
        // A carrier's deck: where its aircraft come down and leave from.
        if let Some(s) = &mut u.staging {
            p3(&mut s.land);
            for l in &mut s.launch {
                *l = (l.0 * k, l.1 * k, l.2 * k, l.3);
            }
        }
        // A launcher on a hull: its tubes' mouths.
        if let Some(s) = &mut u.strategic {
            s.tubes.iter_mut().for_each(p3);
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
        raw(scale, extra).compile(BlueprintId(0), FactionId(0), &|_, _| Ok(BlueprintId(0)))
    }

    /// [`tank`]'s unit file, not yet compiled.
    fn raw(scale: &str, extra: &str) -> crate::raw::Unit {
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
        ron::Options::default()
            .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
            .from_str(&text)
            .unwrap()
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

    /// Drawn bigger, the model and every point on it grow; the body does not.
    #[test]
    fn a_unit_drawn_bigger_keeps_its_body() {
        let one = tank("", "").unwrap();
        let big = tank("draw_scale: 2,", "").unwrap();
        let both = tank("scale: 2, draw_scale: 1.5,", "").unwrap();
        assert_eq!((big.radius, big.height), (one.radius, one.height));
        assert_eq!(big.hull, one.hull);
        assert_eq!(
            big.weapons[0].muzzle,
            one.weapons[0].muzzle * Fx::from_int(2)
        );
        assert_eq!((big.scale, big.visual.draw), (2.0, 2.0));
        assert_eq!(big.drawn_size(), (8.0, 6.0));
        assert_eq!(both.radius, one.radius * 2);
        assert_eq!(
            both.weapons[0].muzzle,
            one.weapons[0].muzzle * Fx::from_int(3)
        );
        assert_eq!(both.drawn_size(), (12.0, 9.0));
        assert!(tank("draw_scale: 9,", "").is_err());
    }

    /// A refit's points are on the model too: fitted to a unit at twice its size, the
    /// module's gun and the gun it moves over for it are where the model has them, scaled
    /// once, not twice.
    #[test]
    fn a_scaled_units_refits_scale_with_it_once() {
        let refits = "refits: [(key: \"arm\", name: \"Arm\", modules: [(key: \"big\",
            name: \"Big\", icon: Tank, cost: (materials: 1, energy: 1, time: 1),
            remount: [(weapon: \"Gun\", muzzle: (3, 1, 2))],
            weapons: [(name: \"Big\", damage: 10, range: 300, reload: 1, speed: 300,
              trajectory: Direct, muzzle: (6, 0, 2), targets: [\"Land\"], color: Orange)])])],";
        let unit = raw("scale: 2,", refits);
        let lookup = |_: &str, _: &str| Ok(BlueprintId(0));
        let mut units = vec![unit.compile(BlueprintId(0), FactionId(0), &lookup).unwrap()];
        let mut by_key = std::collections::BTreeMap::new();
        let sets = crate::refit::expand(&[(0, unit)], &mut units, &mut by_key, &lookup).unwrap();
        let fitted = &units[sets[0].loadouts[1].index()];
        let at = |x, y, z| FxVec3::new(Fx::from_int(x), Fx::from_int(y), Fx::from_int(z));
        assert_eq!(fitted.radius, Fx::from_int(8));
        assert_eq!(fitted.weapons[0].muzzle, at(6, 2, 4));
        assert_eq!(fitted.weapons[1].muzzle, at(12, 0, 4));
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
