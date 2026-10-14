use super::*;
#[test]
fn live_paint_matches_saved_map_including_sparse_updates_and_layer_removal() {
    let dir = std::env::temp_dir().join(format!("mc-cover-edit-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("base.mcmap");
    mc_map::bake::bake(&mc_map::bake::BakeParams::square("Paint", 2, 81), &source).unwrap();
    let file = MapFile::open(&source).unwrap();
    let mut map = EditMap::load(&file).unwrap();
    let mut live = ground_cover_made(&file);
    let check = |map: &EditMap, live: &GroundCover| {
        let path = dir.join("paint.mcmap");
        map.save(&path).unwrap();
        let loaded = MapFile::open(path).unwrap();
        let disk = ground_cover_made(&loaded);
        assert_eq!(live.texels, disk.texels, "cover");
        assert_eq!(live.ways, disk.ways, "ways");
        assert_eq!(live.streets, disk.streets, "streets");
        assert_eq!(live.mips, disk.mips, "ice mips");
    };
    for layer in [Layer::Snow, Layer::Ways, Layer::Streets] {
        layer.initialize(&mut map);
        let (w, h) = layer.dims(&map.info);
        let i = (16 * w + 16) as usize * layer.channels();
        for c in 0..layer.channels() {
            layer.write(&mut map)[i + c] = (c * 73 + 29) as u8;
        }
        live.paint(&map, layer, [0, 0, w - 1, h - 1]);
        check(&map, &live);
        layer.write(&mut map)[i + 1] = 200;
        let bounds = live.paint(&map, layer, [16, 16, 16, 16]);
        assert!(bounds[2] - bounds[0] < live.width / 2);
        check(&map, &live);
    }
    map.snow.clear();
    let (w, h) = Layer::Snow.dims(&map.info);
    live.paint(&map, Layer::Snow, [0, 0, w - 1, h - 1]);
    check(&map, &live);
    drop(file);
    std::fs::remove_dir_all(dir).unwrap();
}
