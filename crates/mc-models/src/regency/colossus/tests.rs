//! The Colossus model against the unit file and the library's checks.

use glam::Vec2;

use super::*;
use crate::gpu_consts::charge_gear;
use crate::material::*;
use crate::{build_model, build_model_scaled, rig, Model};

fn model() -> Model {
    build_model("regency_colossus").unwrap()
}

fn v(p: mc_core::FxVec3) -> Vec3 {
    Vec3::new(p.x.to_f32(), p.y.to_f32(), p.z.to_f32())
}

#[test]
fn fits_the_librarys_checks() {
    super::super::check_charge(
        "regency_colossus",
        RADIUS,
        HEIGHT,
        None,
        &[LANCE_MUZZLE.to_array()],
        HOLD,
    );
}

/// The unit file builds the model `SCALE` times its authored size: its size, the lance's
/// trunnion and charge, the guns' trunnions and mouths and the pack's hinge, face and eight
/// tube mouths are this file's times `SCALE`.
#[test]
fn the_unit_files_weapons_are_the_models() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let blueprints = mc_data::Blueprints::load(&dir).unwrap();
    let bp = blueprints.unit(blueprints.id_of("regency_t5_colossus").unwrap());
    assert_eq!(bp.visual.mesh, "regency_colossus");
    assert!((bp.radius.to_f32() - RADIUS * SCALE).abs() < 1e-3);
    assert!((bp.height.to_f32() - HEIGHT * SCALE).abs() < 1e-3);
    assert_eq!(bp.weapons.len(), 4);
    let near = |a: Vec3, b: Vec3, what: &str| {
        assert!(
            a.distance(b * SCALE) < 1e-2,
            "{what}: {a} against {b} x {SCALE}"
        )
    };
    let [lance, left, right, rockets] = &bp.weapons[..] else {
        panic!("four weapons");
    };
    near(v(lance.muzzle), LANCE_MUZZLE, "lance muzzle");
    near(v(lance.pivot.unwrap()), LANCE_PIVOT, "lance pivot");
    assert!(lance.beam && lance.star_beam && !lance.mount);
    for (w, side) in [(left, 1.0f32), (right, -1.0)] {
        let mirror = Vec3::new(1.0, side, 1.0);
        near(v(w.muzzle), GUN_MUZZLE * mirror, "gun muzzle");
        near(v(w.pivot.unwrap()), GUN_PIVOT * mirror, "gun pivot");
        assert!(w.mount, "{}: a house of its own", w.name);
    }
    near(v(rockets.muzzle), ROCKET_MUZZLE, "rocket face");
    near(v(rockets.pivot.unwrap()), ROCKET_PIVOT, "rocket hinge");
    assert!(rockets.mount && rockets.missile);
    assert_eq!(rockets.muzzles.len(), rockets::CELLS.len());
    for (m, at) in rockets.muzzles.iter().zip(rockets::mouths()) {
        near(v(*m), at, "cell mouth");
    }
    assert!(bp.turret_at.is_none(), "the head turns about the middle");
}

/// The rig and what the renderer reads off the model: the head's pivot, the lance's arm
/// pivot, one house for each of weapons 1 to 3, the charge gear, the shield mast, no violet.
#[test]
fn the_rig_is_the_unit_files() {
    let model = model();
    assert_eq!(Vec3::from(model.turret_pivot), v3(0.0, 0.0, RACE));
    assert_eq!(Vec3::from(model.arm_pivot.unwrap()), LANCE_PIVOT);
    assert_eq!(Vec3::from(model.shield_emitter.unwrap()), body::EMITTER);
    let houses: Vec<(u8, Vec3)> = model
        .houses
        .iter()
        .map(|h| (h.weapon, Vec3::from(h.pivot)))
        .collect();
    assert_eq!(
        houses,
        vec![
            (1, GUN_PIVOT),
            (2, GUN_PIVOT * Vec3::new(1.0, -1.0, 1.0)),
            (3, ROCKET_PIVOT)
        ]
    );
    let gear = model.charge_gear.expect("charge gear");
    assert_eq!(gear[3], lance::GEAR, "gear travels at the model's size");
    for lod in &model.lods {
        assert!(lod.vertices.iter().all(|v| v.material != GLOW_VIOLET));
    }
    // Irregular armour faces retain their polygon outlines for panel seams;
    // falling back to unframed triangles makes the finish look like wireframe.
    assert!(model.lods[0]
        .vertices
        .iter()
        .any(|v| { v.surface & 0xff == crate::pattern::EMBER_MACHINED && v.face[3] < 0.0 }));
    // The charge coils the renderer feeds the charge to (`renderer.rs`, `titan_charge`).
    assert!(model.lods[0].vertices.iter().any(|v| {
        (crate::pattern::COIL..=crate::pattern::COIL_TURN_BACK).contains(&(v.surface & 0xFF))
    }));
}

/// Four legs, two mirrored pairs, that reach the ground where their rest pose stands them,
/// and the head's sweep never meets a thigh or knee.
#[test]
fn stands_on_four_legs() {
    let model = model();
    let crawl = model.legs.and_then(|l| l.crawl).expect("a crawler");
    assert_eq!(crawl.pairs, 2);
    assert_eq!(crawl.lone, None);
    for lod in &model.lods[..2] {
        for pair in 0..2u32 {
            let bones = |limb: u32| {
                lod.vertices.iter().filter(move |v| {
                    v.part == part::LOCOMOTION
                        && v.rig & rig::LIMB_MASK == limb
                        && (v.rig & rig::PAIR_MASK) >> rig::PAIR_SHIFT == pair
                })
            };
            for limb in [rig::THIGH, rig::SHIN] {
                assert!(bones(limb).count() > 0, "pair {pair} bone {limb}");
            }
            let [_, _, foot] = crawl.joints[pair as usize];
            let foot = Vec2::new(foot[0], foot[1]);
            let low = bones(rig::SHIN).map(|v| v.pos[2]).fold(f32::MAX, f32::min);
            assert!(low < 0.08, "pair {pair} foot at {low}");
            let reach = bones(rig::SHIN)
                .map(|v| Vec2::new(v.pos[0], v.pos[1]).dot(foot))
                .fold(f32::MIN, f32::max)
                / foot.length();
            assert!(reach >= foot.length() - 0.1, "pair {pair} reaches {reach}");
        }
    }
}

/// Whatever way the head turns within its arc (`aim_arc` 80: 40 degrees either side), nothing
/// it carries meets a leg: turned in steps across the arc, every vertex of the turret stands
/// above the highest leg vertex under it.
#[test]
fn the_head_sweeps_clear_of_the_legs() {
    let model = model();
    for lod in &model.lods[..2] {
        let cell = |x: f32, y: f32| ((x.floor() as i32), (y.floor() as i32));
        let mut top: std::collections::BTreeMap<(i32, i32), f32> =
            std::collections::BTreeMap::new();
        for v in lod.vertices.iter().filter(|v| v.part == part::LOCOMOTION) {
            let e = top.entry(cell(v.pos[0], v.pos[1])).or_insert(f32::MIN);
            *e = e.max(v.pos[2]);
        }
        for deg in [-40.0f32, -20.0, 0.0, 20.0, 40.0] {
            let (s, c) = deg.to_radians().sin_cos();
            for v in lod.vertices.iter().filter(|v| v.part == part::TURRET) {
                let (x, y) = (v.pos[0] * c - v.pos[1] * s, v.pos[0] * s + v.pos[1] * c);
                for (dx, dy) in [(0.0, 0.0), (0.4, 0.0), (-0.4, 0.0), (0.0, 0.4), (0.0, -0.4)] {
                    if let Some(&t) = top.get(&cell(x + dx, y + dy)) {
                        assert!(
                            v.pos[2] > t + 0.2,
                            "turret vertex at {:?} turned {deg} degrees stands at {} over a leg to {t}",
                            v.pos,
                            v.pos[2]
                        );
                    }
                }
            }
        }
    }
}

/// Built at the unit file's size the legs take strides to match: the stride grows with the
/// build, or the long legs patter through tiny steps.
#[test]
fn strides_grow_with_the_build() {
    let authored = model().legs.expect("the colossus walks");
    assert_eq!(authored.stride, legs::STRIDE);
    let built = build_model_scaled("regency_colossus", RADIUS * SCALE, HEIGHT * SCALE, 4)
        .unwrap()
        .legs
        .unwrap();
    assert!(
        (built.stride - MeshBuilder::fitted_stride(legs::STRIDE * SCALE)).abs() < 0.01,
        "stride {} at {SCALE}x",
        built.stride
    );
}

/// The four feet swing in turn, rear left, front right, rear right, front left, each
/// for a quarter of the cycle: at most one is ever up, so three always stand.
#[test]
fn the_feet_swing_in_turn() {
    let crawl = model().legs.unwrap().crawl.unwrap();
    assert_eq!(crawl.stance[0], legs::STANCE);
    assert_eq!(crawl.stance[1], legs::STANCE);
    let up = |p: f32, s: f32| move |t: f32| (t - p).rem_euclid(1.0) >= s;
    let (front, rear) = (crawl.phase[0], crawl.phase[1]);
    let feet = [
        up(rear, legs::STANCE),
        up(front + 0.5, legs::STANCE),
        up(rear + 0.5, legs::STANCE),
        up(front, legs::STANCE),
    ];
    for i in 0..1000 {
        let t = (i as f32 + 0.5) / 1000.0;
        let lifted: Vec<bool> = feet.iter().map(|f| f(t)).collect();
        let n = lifted.iter().filter(|&&l| l).count();
        assert_eq!(n, 1, "{n} feet up at {t}");
        // They come in order: the foot up is the one in this quarter.
        assert!(lifted[(t * 4.0) as usize], "{lifted:?} at {t}");
    }
}

/// The recessed jaws frame the exposed bore: every jaw vertex keeps its side of
/// the seam (the shader moves them by the sign of y), at rest they meet no nearer than the
/// gap, open the gap is wider than the bore's coils and the prism comes out short of the
/// charge.
#[test]
fn the_jaws_part_round_the_bore() {
    let model = model();
    let gear = |v: &crate::MeshVertex| (v.rig >> rig::CHARGE_GEAR_SHIFT) & charge_gear::MASK;
    let local = |v: &crate::MeshVertex| Vec3::from(v.pos) - LANCE_PIVOT;
    let jaws: Vec<Vec3> = model.lods[0]
        .vertices
        .iter()
        .filter(|v| gear(v) == charge_gear::SPREAD)
        .map(local)
        .collect();
    assert!(jaws.len() > 100);
    assert!(
        jaws.iter().all(|p| p.y.abs() >= 0.05),
        "a jaw straddles the seam"
    );
    assert!(jaws.iter().any(|p| p.y > 0.0) && jaws.iter().any(|p| p.y < 0.0));
    let tip = jaws.iter().map(|p| p.x).fold(f32::MIN, f32::max);
    assert!(
        LANCE_MUZZLE.x - LANCE_PIVOT.x - tip > HOLD * 0.4 - 0.05,
        "jaw tips at {tip}"
    );
    let opened = charge_gear::SPREAD_M * lance::GEAR;
    assert!(
        (2.0 * (opened + 0.12) - 0.24 - 5.0).abs() < 1.0,
        "the jaws part {opened} m a side"
    );
    // The coils, lit rings of the core, fit in the open gap.
    let coil_r = model.lods[0]
        .vertices
        .iter()
        .filter(|v| gear(v) == 0 && v.material == GLOW_LASER)
        .map(|v| {
            let p = local(v);
            Vec2::new(p.y, p.z).length()
        })
        .filter(|r| *r < 3.0)
        .fold(0.0f32, f32::max);
    assert!(
        coil_r < opened + 0.1,
        "coils to {coil_r} m in a gap of {opened}"
    );
    let prism_tip = model.lods[0]
        .vertices
        .iter()
        .filter(|v| gear(v) == charge_gear::EXTEND)
        .map(|v| local(v).x)
        .fold(f32::MIN, f32::max);
    assert!(
        prism_tip + charge_gear::EXTEND_M * lance::GEAR < lance::MOUTH,
        "the prism comes out to {prism_tip}"
    );
}

/// Rockets leave their cells at the unit file's `rake`: along each cell's axis a bore as wide
/// as the cell is clear of every vertex that is not the battery's own, at every quarter turn
/// of the battery on its turntable.
#[test]
fn the_cells_are_clear() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let blueprints = mc_data::Blueprints::load(&dir).unwrap();
    let bp = blueprints.unit(blueprints.id_of("regency_t5_colossus").unwrap());
    let rake = bp.weapons[3].rake.to_radians_f32();
    assert!(
        (rake.to_degrees() - 65.0).abs() < 0.5,
        "rake {}",
        rake.to_degrees()
    );
    let (theta, _) = rockets::axis();
    assert!(
        (theta - rake).to_degrees().abs() < 8.0,
        "the cells stand at {} degrees, rake {}",
        theta.to_degrees(),
        rake.to_degrees()
    );
    let model = model();
    let own = |v: &crate::MeshVertex| {
        v.rig & rig::LIMB_MASK == rig::HOUSE_FIRST + 2 && v.rig & rig::HOUSE_HIGH == 0
    };
    let others: Vec<Vec3> = model.lods[0]
        .vertices
        .iter()
        .filter(|v| !own(v))
        .map(|v| Vec3::from(v.pos))
        .collect();
    let up = Vec3::new(rake.cos(), 0.0, rake.sin());
    let pivot = ROCKET_PIVOT;
    for quarter in 0..4 {
        let yaw = glam::Mat3::from_rotation_z(quarter as f32 * std::f32::consts::FRAC_PI_2);
        for mouth in rockets::mouths() {
            let mouth = pivot + yaw * (mouth - pivot);
            let dir = yaw * up;
            for p in &others {
                let d = *p - mouth;
                let along = d.dot(dir);
                if (0.3..60.0).contains(&along) {
                    let off = (d - dir * along).length();
                    assert!(
                        off > rockets::CELL_R * 0.9,
                        "cell at {mouth} turned {quarter} quarters: {p} stands {off} m off its bore {along} m out"
                    );
                }
            }
        }
    }
}
