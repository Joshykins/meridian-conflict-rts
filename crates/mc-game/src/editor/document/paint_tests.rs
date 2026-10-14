use super::tests::fixture;
use super::*;
#[test]
fn paint_batches_preserve_channels_roundtrip_and_undo_optional_layers() {
    let (folder, mut doc, bp) = fixture();
    let old_snow = doc.map.snow.clone();
    let old_ways = doc.map.ways.clone();
    let old_streets = doc.map.streets.clone();
    let actions = [
        Command::Paint {
            at: [512.0, 512.0],
            radius: 128.0,
            paint: Paint::Ground,
            value: 3,
            heading: 0.0,
        },
        Command::Paint {
            at: [512.0, 512.0],
            radius: 128.0,
            paint: Paint::Snow,
            value: 255,
            heading: 0.0,
        },
        Command::Paint {
            at: [512.0, 512.0],
            radius: 64.0,
            paint: Paint::Ice,
            value: 128,
            heading: 0.0,
        },
        Command::Paint {
            at: [512.0, 512.0],
            radius: 128.0,
            paint: Paint::Trail,
            value: 200,
            heading: 90.0,
        },
        Command::Road {
            from: [384.0, 512.0],
            to: [640.0, 512.0],
            width: 16.0,
            kind: 1,
        },
    ];
    let bad = Command::Paint {
        at: [512.0, 512.0],
        radius: 128.0,
        paint: Paint::Ground,
        value: 9,
        heading: 0.0,
    };
    assert!(doc.apply(&[actions[0].clone(), bad], &bp).is_err());
    assert_eq!(doc.map.streets, old_streets);
    let update = doc.apply(&actions, &bp).unwrap();
    assert_eq!(update.cover.len(), 3);
    assert!(update.terrain.is_none());
    let (sw, _) = doc.map.info.streets_dims();
    let street = (64 * sw + 64) as usize * 4;
    assert_eq!(&doc.map.streets[street..street + 4], &[128, 32, 1, 3]);
    let saved = folder.join("painted");
    doc.save(&saved).unwrap();
    let loaded = Document::open(&saved.join("terrain.mcmap")).unwrap();
    assert_eq!(loaded.map.snow, doc.map.snow);
    assert_eq!(loaded.map.ways, doc.map.ways);
    assert_eq!(loaded.map.streets, doc.map.streets);
    doc.undo().unwrap();
    assert_eq!(doc.map.snow, old_snow);
    assert_eq!(doc.map.ways, old_ways);
    assert_eq!(doc.map.streets, old_streets);
    doc.redo().unwrap();
    assert_eq!(doc.map.streets, loaded.map.streets);
    doc.undo().unwrap();
    doc.map.snow.clear();
    let zero = doc
        .apply(
            &[Command::Paint {
                at: [512.0, 512.0],
                radius: 128.0,
                paint: Paint::Snow,
                value: 0,
                heading: 0.0,
            }],
            &bp,
        )
        .unwrap();
    assert_eq!(zero.cover.len(), 1);
    assert!(!doc.map.snow.is_empty());
    doc.undo().unwrap();
    assert!(doc.map.snow.is_empty());
    std::fs::remove_dir_all(folder).unwrap();
}
