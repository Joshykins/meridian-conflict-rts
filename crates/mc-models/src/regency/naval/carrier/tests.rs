use super::*;
use crate::{build_model, rig, MeshVertex, Model};

const KEY: &str = "regency_carrier";

/// Where each of a layout's seekers is launched from (the unit file's muzzles), in firing
/// order: under its cell's deck, a round's middle down.
fn muzzles(l: &Layout) -> Vec<Vec3> {
    l.cells
        .iter()
        .flat_map(|&c| {
            let g = cell_grid(c, l.cell_deck);
            let block = crate::CellBlock {
                centre: g.centre.to_array(),
                deck: g.deck,
                pitch: g.pitch,
                half: g.half,
                nx: g.nx,
                ny: g.ny,
                hinge_y: g.hinge_y,
                first: 0,
                order: [0; crate::CellBlock::MAX_CELLS],
            };
            CELL_FIRE.iter().map(move |&(i, j)| {
                Vec2::from(block.grid_centre(i as usize, j as usize))
                    .extend(l.cell_deck - MUZZLE_DROP)
            })
        })
        .collect()
}

fn nearest(model: &Model, lod: usize, at: Vec3, keep: impl Fn(&MeshVertex) -> bool) -> f32 {
    model.lods[lod]
        .vertices
        .iter()
        .filter(|v| keep(v))
        .map(|v| Vec3::from(v.pos).distance(at))
        .fold(f32::MAX, f32::min)
}

/// The ship fits the blueprint (60 m, 22 m) and the library's naval rules: budget,
/// levels of detail, keel under the waterline, owner's colour on top, dark plate, no ARC
/// light.
#[test]
fn it_fits_its_blueprint() {
    let key = KEY;
    let model = build_model(key).unwrap();
    let tris = |lod: usize| model.lods[lod].indices.len() / 3;
    let (full, mid, coarse) = (tris(0), tris(1), tris(2));
    assert!((2000..=14000).contains(&full), "{key}: {full} triangles");
    assert!(
        mid as f32 <= full as f32 * 0.45 + 20.0,
        "{key}: {full}/{mid}"
    );
    assert!(coarse < 60, "{key}: coarse {coarse}");
    for (lod, mesh) in model.lods.iter().enumerate() {
        let name = format!("{key} lod{lod}");
        let top = mesh.vertices.iter().map(|v| v.pos[2]).fold(0.0, f32::max);
        assert!((17.6..=27.5).contains(&top), "{name}: top {top}");
        let low = mesh.vertices.iter().map(|v| v.pos[2]).fold(0.0, f32::min);
        assert!(low >= -4.5, "{name}: keel at {low}");
        let reach = mesh
            .vertices
            .iter()
            .map(|v| v.pos[0].hypot(v.pos[1]))
            .fold(0.0, f32::max);
        assert!((45.0..=78.0).contains(&reach), "{name}: reach {reach}");
        assert!(
            mesh.vertices
                .iter()
                .any(|v| v.material == TEAM && v.normal[2] > 0.5),
            "{name}: no upward team colour"
        );
        assert!(mesh.vertices.iter().any(|v| v.material == PLATING_DARK));
        assert!(
            !mesh
                .vertices
                .iter()
                .any(|v| v.material == GLOW || v.material == GLOW_ORANGE),
            "{name}: ARC's blue or orange light"
        );
    }
}

/// Two blocks of six cells, a round under each muzzle; a AA house on each pivot
/// reaching its muzzle; red on each gravity lens; a turning radar; an interceptor
/// door at each tube.
#[test]
fn it_carries_its_weapons() {
    let key = KEY;
    let l = &LAYOUT;
    let model = build_model(key).unwrap();
    assert_eq!(model.cells.len(), 2, "{key}: two cell blocks");
    for (k, m) in muzzles(l).into_iter().enumerate() {
        let (b, (i, j)) = (k / 6, CELL_FIRE[k % 6]);
        let c = Vec2::from(model.cells[b].grid_centre(i as usize, j as usize));
        assert!(c.distance(m.truncate()) < 1e-3, "{key}: muzzle {k}");
        let round = nearest(&model, 0, m, |v| v.part == part::CELL_ROUND);
        assert!(round < 1.0, "{key}: no round near muzzle {k}");
    }
    assert_eq!(model.houses.len(), 2, "{key}: two gun houses");
    for (slot, (pivot, muzzle)) in l.aa.into_iter().enumerate() {
        let house = model.houses[slot];
        assert_eq!(house.weapon as usize, 1 + slot, "{key}");
        assert!(Vec3::from(house.pivot).distance(pivot) < 1e-3, "{key}");
        let of_house = |v: &MeshVertex| v.rig & rig::LIMB_MASK == rig::HOUSE_FIRST + slot as u32;
        for lod in 0..2 {
            let barrel = nearest(&model, lod, muzzle, |v| {
                of_house(v) && v.rig & rig::RECOIL != 0
            });
            assert!(
                barrel < 0.6,
                "{key} lod{lod}: barrel {barrel} from its muzzle"
            );
            let foot = model.lods[lod]
                .vertices
                .iter()
                .filter(|v| of_house(v))
                .map(|v| v.pos[2])
                .fold(f32::MAX, f32::min);
            assert!(foot <= pivot.z, "{key}: the house floats");
        }
    }
    for lod in 0..2 {
        for at in l.defence {
            let red = nearest(&model, lod, at, |v| v.material == GLOW_LASER);
            assert!(red < 0.9, "{key} lod{lod}: no red at the head {at}");
        }
    }
    assert_eq!(Vec3::from(model.spinner_pivot), l.radar, "{key}");
    assert!(
        model.lods[0]
            .vertices
            .iter()
            .any(|v| v.part == part::SPINNER),
        "{key}: the radar turns"
    );
    for t in l.tubes {
        let door = nearest(&model, 0, t, |v| v.material == METAL);
        assert!(door < 0.6, "{key}: no door at the tube {t}");
    }
}

/// The unit file's numbers are the model's.
#[test]
fn the_unit_files_numbers_are_the_models() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let blueprints = mc_data::Blueprints::load(&dir).unwrap();
    let bp = blueprints.unit(blueprints.id_of("regency_t3_carrier").unwrap());
    assert_eq!(bp.visual.mesh, "regency_carrier");
    let l = &LAYOUT;
    let v = |p: mc_core::FxVec3| Vec3::new(p.x.to_f32(), p.y.to_f32(), p.z.to_f32());
    let cells = muzzles(l);
    let seekers = &bp.weapons[0];
    assert_eq!(seekers.muzzles.len(), cells.len());
    for (m, c) in seekers.muzzles.iter().zip(&cells) {
        assert!(
            v(*m).distance(*c) < 1e-3,
            "seeker muzzle {:?} for {c}",
            v(*m)
        );
    }
    assert!(v(seekers.muzzle).distance(cells[0]) < 1e-3);
    for (k, (pivot, muzzle)) in l.aa.into_iter().enumerate() {
        let w = &bp.weapons[1 + k];
        assert!(v(w.pivot.unwrap()).distance(pivot) < 1e-3, "AA {k} pivot");
        assert!(v(w.muzzle).distance(muzzle) < 1e-3, "AA {k} muzzle");
    }
    let tubes = &bp.weapons[3];
    assert_eq!(tubes.muzzles.len(), 2);
    for (m, t) in tubes.muzzles.iter().zip(l.tubes) {
        assert!(v(*m).distance(t) < 1e-3, "tube {:?} for {t}", v(*m));
    }
    let mounts: Vec<Vec3> = bp.anti_missile_mounts.iter().map(|&m| v(m)).collect();
    assert_eq!(mounts.len(), 4);
    for (m, d) in mounts.iter().zip(l.defence) {
        assert!(m.distance(d) < 1e-3, "gravity lens {m} for {d}");
    }
}

/// Prints the ship's numbers in the unit file's form.
#[test]
#[ignore = "prints numbers: cargo test -p mc-models carrier_numbers -- --ignored --nocapture"]
fn carrier_numbers() {
    let f = |m: Vec3| format!("({:.2}, {:.2}, {:.2})", m.x, m.y, m.z);
    let key = KEY;
    let l = &LAYOUT;
    let cells: Vec<String> = muzzles(l).into_iter().map(f).collect();
    println!("{key} muzzles: [{}]", cells.join(", "));
    for (p, m) in l.aa {
        println!("  AA pivot {} muzzle {}", f(p), f(m));
    }
    let heads: Vec<String> = l.defence.into_iter().map(f).collect();
    println!("  anti_missile_mounts: [{}]", heads.join(", "));
    let tubes: Vec<String> = l.tubes.into_iter().map(f).collect();
    println!("  tubes: [{}], radar {}", tubes.join(", "), f(l.radar));
    let model = build_model(key).unwrap();
    let tris = model.lods.each_ref().map(|l| l.indices.len() / 3);
    println!("  triangles {tris:?}");
}
