//! The Plummet, the Regency sonar (`regency_t1_sonar` in
//! `data/factions/regency/units/naval.ron`), on a one-cell lot at sea. It upgrades in place
//! to tech 2 and 3, each tier standing taller with one more listening stage.
//!
//! It stands on a triangular raft (`part::AFLOAT`, as the Canopy does at sea) and hangs
//! its sounding weight, the plummet, under it: a plated double cone with a red waist, held
//! in the water by gravity between three pinch studs, on no line. Over the raft a tripod
//! of plated legs carries a plated bell hung under its hub, a red clapper glowing in its
//! mouth; each tier stacks a smaller bell over the last on a column, and the top one wears
//! a bladed finial with the owner's colour under it.
//!
//! The origin is the waterline.

use glam::{Vec2, Vec3};

use crate::builder::{MeshBuilder, Section};
use crate::material::*;
use crate::part;

use super::kit::{dark_plate, metal, seam, segment, v3};
use super::machine::tier;
use super::machine::*;

/// Each tier's height (the unit file's).
const TOPS: [f32; 3] = [12.0, 15.0, 18.0];
/// The raft's reach and its deck.
const RAFT: f32 = 5.4;
const DECK: f32 = 0.4;
/// The plummet's middle, under the raft.
const PLUMMET: f32 = -5.0;
const THICK: f32 = 0.24;

/// Built at sea: a triangular raft (three lobes round a middle) whose deck stands
/// `DECK` over the water, a graphite rim round it.
fn raft(b: &mut MeshBuilder) {
    let plan: Vec<[f32; 2]> = (0..3)
        .flat_map(|k| {
            let a = (120.0 * k as f32).to_radians();
            [a - 0.32, a + 0.32].map(|t| [t.cos() * RAFT, t.sin() * RAFT])
        })
        .collect();
    b.with_part(part::AFLOAT, |b| {
        dark_plate(b);
        b.loft_z(
            &plan,
            &[
                Section::new(-1.2, 0.84),
                Section::new(-0.4, 0.97),
                Section::new(0.0, 1.0),
            ],
        );
    });
    if b.coarse() {
        return;
    }
    metal(b);
    b.loft_z(
        &plan,
        &[
            Section::new(0.0, 1.0),
            Section::new(DECK - 0.08, 1.0),
            Section::new(DECK, 0.97),
        ],
    );
}

/// The sounding weight under the raft (all `part::AFLOAT`): three plated hangers down
/// from the raft, a red-tipped pinch stud on each, and the plummet floating between them.
fn plummet(b: &mut MeshBuilder) {
    b.with_part(part::AFLOAT, |b| {
        let fine = b.fine();
        let z = PLUMMET;
        for k in 0..3 {
            b.yawed(Vec3::ZERO, (60.0 + 120.0 * k as f32).to_radians(), |b| {
                strut(b, v3(2.6, 0.0, -1.0), v3(2.0, 0.0, z - 1.2), 0.22);
                metal(b);
                b.cylinder_between(v3(1.95, 0.0, z), v3(1.5, 0.0, z), 0.18, 0.13, 5);
                if fine {
                    b.paint(GLOW_LASER);
                    b.cylinder_between(v3(1.5, 0.0, z), v3(1.35, 0.0, z), 0.13, 0.06, 5);
                }
            });
        }
        let sides = b.sides(8);
        let ring = |r: f32, z: f32| -> Vec<Vec3> {
            (0..sides)
                .map(|i| {
                    let a = std::f32::consts::TAU * (i as f32 + 0.5) / sides as f32;
                    v3(a.cos() * r, a.sin() * r, z)
                })
                .collect()
        };
        dark_plate(b);
        b.loft(&[ring(0.12, z + 2.0), ring(0.95, z + 0.3)], true, true);
        b.loft(&[ring(0.95, z - 0.3), ring(0.1, z - 2.4)], true, true);
        b.paint(GLOW_LASER);
        b.loft(&[ring(0.8, z - 0.3), ring(0.8, z + 0.3)], false, false);
    });
}

/// A plated footing on the deck under a mast, `r` across.
fn coarse(b: &mut MeshBuilder, tech: u8) {
    let width = 2.8;
    raft(b);
    let top = TOPS[tech as usize - 1];
    dark_plate(b);
    b.frustum(
        Vec3::Z * DECK,
        Vec2::new(width, 1.6),
        Vec2::new(width * 0.6, 0.8),
        top - DECK,
        Vec2::ZERO,
    );
    b.paint(TEAM);
    b.face(&[
        v3(-0.4, -0.4 * width * 0.6, top + 0.02),
        v3(0.4, -0.4 * width * 0.6, top + 0.02),
        v3(0.4, 0.4 * width * 0.6, top + 0.02),
        v3(-0.4, 0.4 * width * 0.6, top + 0.02),
    ]);
}

/// The owner's colour on a flat top at `z`, `r` across.
fn team_cap(b: &mut MeshBuilder, z: f32, r: f32) {
    b.paint(TEAM);
    b.face(&[v3(r, 0.0, z), v3(0.0, r, z), v3(-r, 0.0, z), v3(0.0, -r, z)]);
}

/// Each tier's hub, and the bell hung under it: its crown, its mouth and its radius.
const HUBS: [f32; 3] = [8.2, 11.4, 14.6];
const BELLS: [(f32, f32, f32); 3] = [(7.8, 2.4, 2.7), (11.0, 8.9, 1.9), (14.2, 12.3, 1.4)];
/// Where the tripod's legs stand on the raft.
const LEG_FOOT: f32 = 4.4;

pub(super) fn sonar(b: &mut MeshBuilder, tech: u8) {
    let tech = tech.clamp(1, 3);
    if b.coarse() {
        coarse(b, tech);
        return;
    }
    raft(b);
    plummet(b);
    for k in 0..3 {
        b.yawed(Vec3::ZERO, (120.0 * k as f32).to_radians(), bell_leg);
    }
    for t in 1..=3u8 {
        let top = t >= tech;
        tier(b, tech, t, 0.3, |b| bell_stage(b, t, top));
    }
}

/// One of the tripod's legs, along +x: a plated limb from a foot block on the raft up to
/// the first hub, a plate lapped down it into a spike.
fn bell_leg(b: &mut MeshBuilder) {
    let foot = v3(LEG_FOOT, 0.0, DECK);
    let hip = v3(0.9, 0.0, HUBS[0] - 0.2);
    dark_plate(b);
    segment(
        b,
        &[(foot + Vec3::Z * 0.6, 0.38, 0.38), (hip, 0.3, 0.3)],
        Vec3::Z,
    );
    b.block(
        v3(LEG_FOOT - 0.7, -0.6, DECK),
        v3(LEG_FOOT + 0.5, 0.6, DECK + 0.9),
    );
    let up = (hip - foot).normalize();
    let out = v3(up.z, 0.0, -up.x);
    dark_plate(b);
    armour(
        b,
        &Frame::new(hip + out * 0.35, -up, out),
        &swept(5.2, 0.5, 0.0, 0.6),
        THICK,
    );
    if b.fine() {
        red_slot(
            b,
            v3(LEG_FOOT + 0.52, 0.0, DECK + 0.55),
            Vec3::X,
            Vec3::Y,
            0.7,
            0.14,
        );
    }
}

/// Tier `t`'s stage: the hub (and the column up to it from the hub below), the bell hung
/// under it, and the finial over it when it is the top stage.
fn bell_stage(b: &mut MeshBuilder, t: u8, top: bool) {
    let fine = b.fine();
    let i = t as usize - 1;
    let hub = HUBS[i];
    if t > 1 {
        shaft(b, Vec3::Z * HUBS[i - 1], Vec3::Z * hub, 0.32);
    }
    collar(b, Vec3::Z * hub, Vec3::Z, 0.75, 0.8);
    let (crown, mouth, r) = BELLS[i];
    bell(b, crown, mouth, r);
    // The clapper: a red glow in the bell's mouth, on a graphite rod.
    shaft(b, Vec3::Z * crown, Vec3::Z * (mouth + 0.9), 0.12);
    b.paint(GLOW_LASER);
    b.spheroid(
        Vec3::Z * (mouth + 0.6),
        Vec3::splat(0.32 * r / 2.7 + 0.12),
        if fine { 8 } else { 5 },
        if fine { 4 } else { 2 },
    );
    if top {
        let foot = hub + 0.4;
        let tip = TOPS[i];
        dark_plate(b);
        b.prism(Vec3::Z * foot, 6, 0.6, 0.3, tip - 1.0 - foot);
        team_cap_ring(b, tip - 1.0);
        dark_plate(b);
        for k in 0..3 {
            let a = (60.0 + 120.0 * k as f32).to_radians();
            let d = v3(a.cos(), a.sin(), 0.0);
            super::kit::blade(
                b,
                d * 0.25 + Vec3::Z * (tip - 1.6),
                Vec3::Z * tip + d * 0.05,
                0.35,
                d,
            );
        }
    }
}

/// The owner's colour on a little plated table at `z`.
fn team_cap_ring(b: &mut MeshBuilder, z: f32) {
    seam(b);
    b.prism(Vec3::Z * (z - 0.15), 6, 0.55, 0.55, 0.15);
    team_cap(b, z + 0.01, 0.4);
}

/// A plated bell from its crown at `crown` down to its mouth at `mouth`, `r` round the
/// mouth: a flared hollow drum, a red-lit band inside its lip, and plates lapped down its
/// flanks past the lip into spikes.
fn bell(b: &mut MeshBuilder, crown: f32, mouth: f32, r: f32) {
    let fine = b.fine();
    let sides = if fine { 12 } else { 6 };
    let h = crown - mouth;
    let ring = |rr: f32, z: f32| -> Vec<Vec3> {
        (0..sides)
            .map(|i| {
                let a = std::f32::consts::TAU * (i as f32 + 0.5) / sides as f32;
                v3(a.cos() * rr, a.sin() * rr, z)
            })
            .collect()
    };
    dark_plate(b);
    b.loft(
        &[
            ring(r * 0.3, crown),
            ring(r * 0.55, crown - h * 0.25),
            ring(r * 0.78, crown - h * 0.7),
            ring(r, mouth),
            ring(r * 0.86, mouth),
            ring(r * 0.66, crown - h * 0.6),
            ring(r * 0.2, crown - 0.15),
        ],
        true,
        true,
    );
    seam(b);
    hoop(b, Vec3::Z * (mouth + 0.1), r * 0.98, 0.18, 0.2, sides);
    if fine {
        b.paint(GLOW_LASER);
        hoop(b, Vec3::Z * (mouth + 0.3), r * 0.84, 0.06, 0.12, sides);
    }
    let plates = if fine { 6 } else { 3 };
    for k in 0..plates {
        let a = (360.0 / plates as f32 * k as f32 + 30.0).to_radians();
        let d = v3(a.cos(), a.sin(), 0.0);
        let at = |s: f32| d * (r * (0.3 + 0.7 * s) + 0.12) + Vec3::Z * (crown - h * s);
        let f = Frame::new(at(0.1), at(1.0) - at(0.1), d * h + Vec3::Z * r * 0.7);
        dark_plate(b);
        armour(
            b,
            &f,
            &swept(at(0.1).distance(at(1.0)) + 0.6, r * 0.32, 0.0, 0.7),
            THICK,
        );
    }
}

#[cfg(test)]
mod tests {
    use crate::{build_model_scaled, rig};

    use super::TOPS;

    #[test]
    fn sonar_fits_at_every_tier() {
        for tech in 1..=3u8 {
            super::super::check_at(
                "regency_sonar",
                tech,
                6.0,
                TOPS[tech as usize - 1],
                Some(1),
                &[],
            );
        }
    }

    /// Each tier carries the next one's stage as refit pieces, and tech 3 is finished.
    #[test]
    fn the_next_tier_is_refit_pieces() {
        for tech in 1..=3u8 {
            let model =
                build_model_scaled("regency_sonar", 6.0, TOPS[tech as usize - 1], tech).unwrap();
            let refit = model.lods[0]
                .vertices
                .iter()
                .any(|v| v.rig & rig::UPGRADE != 0);
            assert_eq!(refit, tech < 3, "tech {tech}");
        }
    }
}
