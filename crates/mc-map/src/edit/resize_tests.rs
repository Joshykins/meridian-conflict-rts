use super::*;
use crate::{test_util, PropKind, TILE_SIZE_M};
use mc_core::{Angle, Fx};
fn fixture() -> EditMap {
    let mut map = EditMap::load(&MapFile::open(test_util::baked_4km()).unwrap()).unwrap();
    for layer in [Layer::Snow, Layer::Ways, Layer::Streets] {
        layer.initialize(&mut map);
    }
    map.snow[3] = 173;
    map.ways[6] = 245;
    map.streets[11] = 3;
    map.props = vec![
        Prop {
            kind: PropKind::RockSmall,
            pos: FxVec2::from_ints(500, 500),
            heading: Angle::ZERO,
            scale_milli: 1000,
            wear_milli: 0,
        },
        Prop {
            kind: PropKind::RockLarge,
            pos: FxVec2::from_ints(3000, 3000),
            heading: Angle::ZERO,
            scale_milli: 1200,
            wear_milli: 0,
        },
    ];
    map.tints = vec![[255, 10, 0], [20, 255, 30]];
    map
}
#[test]
fn extension_then_crop_preserves_every_original_sample_and_layer() {
    let original = fixture();
    let tiles = [original.info.tiles_w, original.info.tiles_h];
    let (extended, indices) = original
        .resized([-1, -1], [tiles[0] + 2, tiles[1] + 2], Fx::from_int(-20))
        .unwrap();
    assert_eq!(indices, vec![Some(0), Some(1)]);
    assert_eq!(
        extended.samples()[0],
        original.info.height_to_sample(Fx::from_int(-20))
    );
    let (back, _) = extended.resized([1, 1], tiles, Fx::ZERO).unwrap();
    assert_eq!(back.samples(), original.samples());
    assert_eq!(back.snow, original.snow);
    assert_eq!(back.ways, original.ways);
    assert_eq!(back.streets, original.streets);
    assert_eq!(back.props, original.props);
    assert_eq!(back.tints, original.tints);
    assert_eq!(back.starts, original.starts);
    assert_eq!(back.ore, original.ore);
    assert_eq!(back.decks, original.decks);
    assert_eq!(back.wrecks, original.wrecks);
    let a = test_util::temp_path("resize-original");
    let b = test_util::temp_path("resize-back");
    assert_eq!(original.save(&a).unwrap(), back.save(&b).unwrap());
    std::fs::remove_file(a).unwrap();
    std::fs::remove_file(b).unwrap();
}
#[test]
fn cropping_remaps_tints_and_clips_ore_and_bridges() {
    let mut original = fixture();
    original.ore = vec![OreRegion {
        points: vec![
            FxVec2::from_ints(1800, 1800),
            FxVec2::from_ints(2400, 1800),
            FxVec2::from_ints(2400, 2400),
            FxVec2::from_ints(1800, 2400),
        ],
    }];
    original.decks = vec![Deck {
        a: FxVec2::from_ints(1800, 2200),
        b: FxVec2::from_ints(2500, 2200),
        half: Fx::from_int(12),
        top: Fx::from_int(50),
    }];
    let (cropped, indices) = original.resized([1, 1], [1, 1], Fx::ZERO).unwrap();
    assert_eq!(indices, vec![None, Some(0)]);
    assert_eq!(
        cropped.props[0].pos,
        FxVec2::from_ints(3000 - TILE_SIZE_M, 3000 - TILE_SIZE_M)
    );
    assert_eq!(cropped.tints, vec![[20, 255, 30]]);
    assert_eq!(cropped.ore.len(), 1);
    assert!(cropped.ore[0].contains(FxVec2::from_ints(100, 100)));
    assert_eq!(cropped.decks[0].a, FxVec2::from_ints(0, 2200 - TILE_SIZE_M));
    assert!(cropped.decks[0].valid(cropped.info.size_metres()));
    let file = test_util::temp_path("resize-cropped");
    cropped.save(&file).unwrap();
    MapFile::open(&file).unwrap().verify().unwrap();
    std::fs::remove_file(file).unwrap();
    assert!(original.resized([0, 0], [0, 1], Fx::ZERO).is_err());
    assert!(original.resized([i32::MAX, 0], [1, 1], Fx::ZERO).is_err());
}
