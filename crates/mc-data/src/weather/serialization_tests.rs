use super::*;
#[test]
fn every_checked_in_map_configuration_roundtrips_without_losing_metadata() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../maps");
    let mut checked = 0;
    for entry in std::fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        let config = if path.is_dir() {
            path.join("map.ron")
        } else {
            path
        };
        if !config.is_file() || config.extension().is_none_or(|e| e != "ron") {
            continue;
        }
        let text = std::fs::read_to_string(&config).unwrap();
        let before =
            MapConfig::parse(&text).unwrap_or_else(|e| panic!("{}: {e}", config.display()));
        let serialized =
            ron::ser::to_string_pretty(&before, ron::ser::PrettyConfig::default()).unwrap();
        assert_eq!(
            MapConfig::parse(&serialized).unwrap(),
            before,
            "{}",
            config.display()
        );
        checked += 1;
    }
    assert!(checked > 5);
}
