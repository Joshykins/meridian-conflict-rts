//! Aureole: tech 2 mobile missile defence, the Corona taken off its plinth. Two of the
//! navy's laser heads (`pd_laser`) ride the Testudo's tracked hull, a little shorter, to
//! burn down what is fired at the column it drives with. Unarmed. The heads stand where
//! the unit file's `anti_missile_mounts` say the beams leave
//! (`aster_t2_mobile_missile_defense` in `data/factions/aster/units/land.ron`): keep
//! [`HEADS`] and the data in step. Nothing that carries a head turns, since the sim fires
//! from the mounts as laid (turned with the hull); only the tracking radar spins.

use super::missile_defense::radar;
use super::naval::pd_laser;
use super::parts::*;
use crate::builder::MeshBuilder;
use crate::material::*;
use glam::Vec3;

/// The laser heads' centres, either side across y.
const HEADS: [Vec3; 2] = [Vec3::new(-0.6, 1.5, 5.0), Vec3::new(-0.6, -1.5, 5.0)];
/// Laser head size (the Corona's are 0.72).
const HEAD: f32 = 0.6;

/// The Testudo's tracked chassis, a little shorter.
const TRACKED: Chassis = Chassis {
    rear: -4.3,
    front: 4.3,
    track: (1.95, 3.25, 1.32),
    split_tracks: false,
    deck: 1.85,
    dark: false,
    lit: false,
};

/// Two tall raked posts up from pods over the tracks, leaning in to carry the heads
/// high, a dark capacitor box between them on the deck and the tracking radar on it.
pub(super) fn missile_carrier(b: &mut MeshBuilder, _tech: u8) {
    let deck = hull(b);
    if b.coarse() {
        coarse_heads(b);
        return;
    }
    b.mirror_y(|b| {
        let head = HEADS[0];
        let foot = v3(head.x - 0.6, 2.35, deck - 0.15);
        b.paint(PLATING);
        b.block(v3(-3.4, 2.0, 1.35), v3(0.8, 2.6, deck + 0.3));
        b.paint(ACCENT);
        b.beam(
            foot,
            v3(head.x, head.y, head.z - 0.8),
            v2(0.6, 0.55),
            v2(0.4, 0.4),
        );
        pd_laser(b, head, HEAD, Some(head.z - 0.8));
        if b.fine() {
            vent(b, v3(-2.2, 2.3, deck + 0.3), v2(1.2, 0.5), 3, METAL);
        }
    });
    b.paint(PLATING_DARK);
    b.chamfered_box(v3(-1.6, 0.0, deck + 0.6), v3(2.6, 1.8, 1.2), 0.15);
    radar(b, deck + 1.2, 1.1);
}

/// The tracked hull, the owner's colour on its glacis; the deck's height.
fn hull(b: &mut MeshBuilder) -> f32 {
    let deck = tracked_chassis(b, &TRACKED);
    team_panel(b, deck.at(0.9, 0.0), v2(0.8, 2.0));
    deck.z
}

/// Far off: a red point where each head is.
fn coarse_heads(b: &mut MeshBuilder) {
    b.paint(GLOW_LASER);
    for at in HEADS {
        b.decal(at, v2(1.1, 1.1));
    }
}

#[cfg(test)]
mod tests {
    use super::HEADS;
    use crate::{build_model_fitted, part};
    use glam::Vec3;

    /// The heads are the unit file's anti-missile mounts.
    #[test]
    fn the_unit_files_mounts_are_the_heads() {
        let dir = std::path::Path::new(&std::env::var_os("CARGO_MANIFEST_DIR").unwrap())
            .join("../../data");
        let bps = mc_data::Blueprints::load(&dir).unwrap();
        let unit = bps.unit_by_key("aster_t2_mobile_missile_defense").unwrap();
        assert_eq!(unit.visual.mesh, "missile_carrier");
        let mounts: Vec<Vec3> = unit
            .anti_missile_mounts
            .iter()
            .map(|&m| crate::authored(unit, m))
            .collect();
        assert_eq!(mounts.len(), HEADS.len());
        for (m, h) in mounts.iter().zip(&HEADS) {
            assert!(m.distance(*h) < 1e-3, "mount {m} vs head {h}");
        }
    }

    #[test]
    fn its_radar_turns() {
        let model = build_model_fitted("missile_carrier", 5.0, 6.0, 2, &[]).unwrap();
        assert!(model.lods[0]
            .vertices
            .iter()
            .any(|v| v.part == part::SPINNER));
    }
}
