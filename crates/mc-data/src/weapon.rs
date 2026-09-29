//! A compiled weapon: what a unit's gun does in the sim, and how it is drawn and heard.

use mc_core::{Angle, Fx, FxVec3};

use crate::{Bore, PlasmaGrade, Sabot, Trajectory, WeaponColor, WeaponSounds};

/// The widest circle, metres, a `Bombard` order may spread a gun's shots over unless
/// its data gives it more (`RawWeapon::bombard`).
pub const BOMBARD_RADIUS: f64 = 250.0;

#[derive(Clone, Debug)]
pub struct Weapon {
    pub name: String,
    pub damage: Fx,
    /// Splash radius; zero for a single-target hit.
    pub splash: Fx,
    pub range_min: Fx,
    pub range_max: Fx,
    pub reload_ticks: u16,
    /// Shots per reload cycle and the gap between them. A delay of zero dumps the volley together.
    pub salvo: u8,
    /// Number released simultaneously at each salvo step.
    pub salvo_batch: u8,
    pub salvo_delay_ticks: u8,
    /// Metres per second.
    pub projectile_speed: Fx,
    pub trajectory: Trajectory,
    /// Crosses its range in the tick it is fired (`projectile_speed` is set to do so),
    /// drawn as a beam from the muzzle to what it hit rather than a traveling slug.
    pub hitscan: bool,
    /// A held beam (`RawWeapon::beam`): one steady stream while it fires. Cosmetic.
    pub beam: bool,
    /// An ARC rail gun's slug: flies at `projectile_speed` like any shell, but is drawn
    /// white-hot with a vapour trail, and flashes and lands like a rail.
    pub rail: bool,
    /// A flak shell (`RawWeapon::flak`): bursts at its aim point on a timed fuse if its
    /// proximity fuse has not gone off first.
    pub flak: bool,
    /// Extra ticks a ballistic shell stays up. Zero: it flies at `projectile_speed`.
    pub loft_ticks: u16,
    /// Angle steps per tick. Zero means the weapon is fixed to the hull.
    pub turret_turn: u16,
    /// Half-width of the firing arc around the hull's forward axis; 0x8000 is all-round.
    pub half_arc: u16,
    /// Muzzle position in unit space (x forward, y left, z up).
    pub muzzle: FxVec3,
    /// Tube mouths a volley fires from, in the same space as `muzzle`. Empty: `muzzle` only.
    pub muzzles: Vec<FxVec3>,
    /// The elbow (or trunnion) the weapon pitches about to point up or down at its target,
    /// carrying the muzzle with it. `None`: the weapon only turns.
    pub pivot: Option<FxVec3>,
    /// Random aim error, angle steps.
    pub spread: u16,
    /// Half-angle of the cone ahead of the gun: it fires while anything it may shoot is
    /// in it, angle steps (`RawWeapon::sweep`). Zero: it fires only on target.
    pub sweep: u16,
    pub target_mask: u32,
    /// Kinds it takes first when choosing for itself (`RawWeapon::prefer`). Zero: the nearest.
    pub prefer_mask: u32,
    pub color: WeaponColor,
    pub missile: bool,
    /// Hit points an intercept laser must burn through. Zero on a missile is a
    /// light casing. Heavier missiles take a longer burst.
    pub intercept_hp: Fx,
    pub guided: bool,
    pub vertical_launch: bool,
    /// How far a vertical-launch cell leans toward the bow off the vertical.
    pub cant: Angle,
    /// Unpowered ejection, mid-air aim, and hang before a guided missile ignites.
    pub cold_launch_ticks: u16,
    /// Of `cold_launch_ticks`, the booster's burn straight up out of the cell; the rest is
    /// a coast while thrusters turn the nose over. Zero: a pneumatic toss.
    pub boost_ticks: u16,
    /// Ticks the cell hatches take to open before a salvo (`Units::deploy` counts them).
    pub hatch_ticks: u16,
    /// A salvo's missiles spread over the targets in range (`combat::split_target`).
    pub split: bool,
    pub proximity: Fx,
    pub burn_ticks: u16,
    pub rear: bool,
    /// Holds for the unit's other `volley` weapons and fires with them as one broadside.
    pub volley: bool,
    /// Which way the weapon rests and its arc is centred, off the nose (180 for `rear`).
    /// A limited arc off the nose also limits what it takes as a target (`RawWeapon::facing`).
    pub facing: Angle,
    /// Multiplies the muzzle flash. 1 is the size the damage implies.
    pub flash: f32,
    /// Multiplies the impact flash. 1 is the size the damage implies.
    pub impact: f32,
    /// Multiplies the muzzle and impact shockwave. 0 is none; 1 is the size the damage implies.
    pub shockwave: f32,
    /// Multiplies the projectile tracer. 1 is the size the damage implies.
    pub tracer: f32,
    /// A projectile wake: blue energy for Blue shots, white smoke for Orange shots.
    pub trail: bool,
    /// Seconds the wake hangs. Zero: the usual hang, when the shot has a trail.
    pub wake: f32,
    /// Multiplies a blue plasma sheath around the traveling slug. 0 is none;
    /// 1 is the size the damage implies. Does not change the tracer, flash, or impact.
    pub plasma: f32,
    /// Blue-white bolts thrown at the muzzle and the impact. Zero: none.
    pub bolts: u8,
    /// Multiplies how long a plasma slug's tail is drawn (`RawWeapon::streak`). Zero: one.
    pub streak: f32,
    /// Metres of a charged shell's last flight the lightning strikes down when it lands. Zero: none.
    pub discharge: f32,
    /// Where a shell lands on open ground it melts a pool this many times its splash
    /// across, glowing then crusting over (`RawWeapon::melt`). Zero: none.
    pub melt: f32,
    /// A capital rail gun: its shot is drawn and heard at this scale over an ordinary rail. Zero: none.
    pub heavy_rail: f32,
    /// A bolt rifle's charge and firing sequence, drawn on a gun this many metres long
    /// (`RawWeapon::arc_charge`). Zero: none.
    pub arc_charge: f32,
    /// An Arc Howitzer's charge and firing sequence, drawn on a tube of this size
    /// (`RawWeapon::howitzer`). None: none.
    pub howitzer: Option<HowitzerLook>,
    /// A ballistic gun laid flat (`RawWeapon::flat_fire`): its range reads as direct fire.
    pub flat_fire: bool,
    /// A great gun: its firing, trail and hit are drawn at this scale (`RawWeapon::great_gun`).
    /// Zero: none.
    pub great_gun: f32,
    /// Stays laid where it last aimed while it has nothing to shoot (`RawWeapon::keeps_aim`).
    pub keeps_aim: bool,
    /// The widest circle a `Bombard` order may spread this gun's shots over, metres
    /// (`RawWeapon::bombard`); never under `BOMBARD_RADIUS`.
    pub bombard_radius: Fx,
    /// A missile's body across, in metres, as drawn (`RawWeapon::caliber`). Zero: from its damage.
    pub caliber: f32,
    /// A Naga plasma weapon's grade (`RawWeapon::plasma_grade`). Cosmetic. None: not plasma.
    pub plasma_grade: Option<PlasmaGrade>,
    /// How far a gun house on a capital hull may dip below its deck; zero: no limit.
    pub depression: Angle,
    /// How far a torso gun may swing off the torso, pitching on its own (`RawWeapon::sway`).
    pub sway: Angle,
    /// A rocket rack's launch angle above level; zero: the rack's rake (`RawWeapon::rake`).
    pub rake: Angle,
    /// Its own turret on the unit's turret: aims about `pivot` by itself and fires while the unit works.
    pub mount: bool,
    /// Reaches what is on the ground or the water along the line of sight, so an
    /// aircraft high up cannot reach it until it comes down. Aircraft: across the map.
    pub slant: bool,
    /// Ticks a rotary gun spins up before it fires. Zero: it fires at once.
    pub spin_ticks: u16,
    /// Hundredths: a rotary gun that fires while it spins up, its reload this much longer
    /// at a third of its spin and down to `reload` at full (`RawWeapon::spin_ramp`). Zero:
    /// it waits for full spin.
    pub spin_ramp: u16,
    /// Barrels round a rotary gun's cluster (`RawWeapon::barrels`): it fires as one of
    /// them reaches the top. Zero: whenever it is ready.
    pub barrels: u8,
    /// Rounds each shot is drawn as, spread over the time to the next shot. Cosmetic:
    /// the sim flies one projectile; the mirror draws the rest behind it. One: just the shot.
    pub rounds: u8,
    /// Metres behind the muzzle where spent casings are thrown out, one per round.
    /// Zero: none. Cosmetic: not in the content hash.
    pub casings: f32,
    /// A giant gun's spent sabot (`RawSabot`).
    pub sabot: Option<Sabot>,
    /// How far a stream gun's tracers lean from deep orange to red, zero to one.
    /// Cosmetic: not in the content hash.
    pub red: f32,
    /// Runs under the water, homing, and can hit a submerged hull (nothing else can).
    pub torpedo: bool,
    /// A guided missile's cruise height over ground and water (a sea skimmer); zero for none.
    pub skim: Fx,
    /// A guided missile's climb before it comes down on its mark (a high arc); zero for none.
    pub apogee: Fx,
    /// A thrown charge that curves onto its mark (`RawWeapon::curve`): how far off the line
    /// to the mark a salvo's shots leave. Zero: it flies the usual way.
    pub curve: Angle,
    /// Only fires with the hull on the surface (a submarine's deck gun).
    pub surfaced: bool,
    /// Interceptor torpedo tubes: fired at enemy torpedoes in range, never at units.
    pub intercepts: bool,
    /// An Argon Electric Bore: the charge follows the tracer's channel when it lands.
    pub bore: Option<Bore>,
    /// Names from the sound library; what is `None` falls back to the library's defaults.
    pub sounds: WeaponSounds,
    /// Ticks before a salvo at which the weapon is heard charging. Zero: it does not charge.
    pub charge_ticks: u16,
    /// Background text for the interface, from the faction's `lore.ron`. Empty when it has none.
    /// Cosmetic: not in the content hash.
    pub lore: String,
}

impl Weapon {
    /// Whether a missile `age` ticks out flies with no motor burning: tossed out of its
    /// cell, or coasting after its booster while it turns over, before the motor lights.
    pub fn motor_out(&self, age: u16) -> bool {
        self.cold_launch_ticks > 0 && age <= self.cold_launch_ticks && age > self.boost_ticks
    }

    /// Casing hit points an intercept laser has to burn through. A non-missile
    /// has none. A missile that does not say otherwise fails in one tick.
    pub fn casing_hp(&self) -> Fx {
        if !self.missile {
            Fx::ZERO
        } else if self.intercept_hp > Fx::ZERO {
            self.intercept_hp
        } else {
            Fx::from_int(10)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use mc_core::Fx;

    use crate::Blueprints;

    /// The Naga fire plasma (docs/STYLE.md, "The Naga suite"): every gun of theirs names
    /// its grade, and nothing of ARC's does.
    #[test]
    fn only_the_naga_fire_plasma() {
        let bp =
            Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap();
        let naga = bp.faction_by_key("naga").unwrap().id;
        let mut naga_guns = 0;
        for unit in &bp.units {
            for w in &unit.weapons {
                let naga_gun = unit.faction == naga;
                naga_guns += usize::from(naga_gun);
                assert_eq!(
                    w.plasma_grade.is_some(),
                    naga_gun,
                    "{}: {}",
                    unit.key,
                    w.name
                );
            }
        }
        assert!(naga_guns > 0);
    }

    /// Flak (docs/STYLE.md "Flak"): a shell slow enough to follow up, on a fuse, that
    /// bursts wide among aircraft.
    #[test]
    fn flak_is_slow_and_bursts_wide() {
        let bp =
            Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap();
        let mut guns = 0;
        for unit in &bp.units {
            for w in unit.weapons.iter().filter(|w| w.flak) {
                guns += 1;
                let what = format!("{}: {}", unit.key, w.name);
                assert!(!w.hitscan && !w.rail && !w.missile, "{what}");
                assert!(w.projectile_speed <= Fx::from_int(400), "{what}: too fast");
                assert!(w.splash >= Fx::from_int(25), "{what}: burst too small");
                assert!(w.proximity > Fx::ZERO, "{what}: no proximity fuse");
                assert_ne!(w.target_mask & crate::cat::AIR, 0, "{what}");
            }
        }
        assert!(guns >= 5, "flak guns: {guns}");
    }

    /// The Onager sits just out of a Trebuchet's reach, and the commander's Shoulder
    /// Howitzer matches the Onager.
    #[test]
    fn the_onager_outranges_the_trebuchet_and_the_commander_matches_it() {
        let bp =
            Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap();
        let reach = |key: &str| bp.unit(bp.id_of(key).unwrap()).weapons[0].range_max;
        let onager = reach("aster_t2_artillery");
        let trebuchet = reach("aster_t3_artillery");
        assert!(onager > trebuchet, "{onager:?} vs {trebuchet:?}");
        assert!(
            onager <= trebuchet * Fx::ratio(6, 5),
            "only slightly further"
        );
        let shoulder: Vec<Fx> = bp
            .units
            .iter()
            .flat_map(|u| &u.weapons)
            .filter(|w| w.name == "Shoulder Howitzer")
            .map(|w| w.range_max)
            .collect();
        assert!(!shoulder.is_empty());
        assert!(shoulder.iter().all(|&r| r == onager), "{shoulder:?}");
    }
}

/// An Arc Howitzer's tube (`RawWeapon::howitzer`): metres from breech to muzzle, and half
/// the breech housing's height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HowitzerLook {
    pub length: f32,
    pub radius: f32,
}
