use super::*;
use mc_data::{
    regions::{Region as ClimateRegion, Wall},
    weather::{Climate, MapConfig, WeatherPreset},
};
#[test]
fn environment_batch_roundtrips_preserves_other_settings_and_undoes() {
    let (folder, mut doc, bp) = tests::fixture();
    doc.config = MapConfig::parse(
        r#"(
        climate: Tropical, weather: Cloudy, time: Dawn,
        biome: Coastal, style: Teams, strata_lift: 123, blend: 14,
        survival: (engine: (500, 500), ray_height: 720, engine_start: 1,
            guards: [(key: "aster_t2_point_defense", at: (300, 400), facing: 90)],
            fronts: [(name: "North", domain: Land, path: [(500, 500), (100, 100)])]),
    )"#,
    )
    .unwrap();
    let original = doc.config.clone();
    let water = doc.map.info.water_level;
    let wall = Wall {
        line: vec![(500.0, 0.0), (500.0, 1024.0)],
        left: 0,
        right: 1,
    };
    let actions = [
        Command::ClimateRegions {
            regions: vec![
                ClimateRegion {
                    name: "West".into(),
                    climate: Climate::Desert,
                    ..Default::default()
                },
                ClimateRegion {
                    name: "East".into(),
                    climate: Climate::Polar,
                    ..Default::default()
                },
            ],
            walls: vec![wall],
            blend: 100.0,
        },
        Command::Weather {
            region: 1,
            preset: WeatherPreset::Stormy,
        },
        Command::Water { level: 133.25 },
        Command::Time { hour: 17.5 },
    ];
    let update = doc.apply(&actions, &bp).unwrap();
    assert!(update.environment);
    assert_eq!(doc.config.survival, original.survival);
    assert_eq!(doc.config.biome, original.biome);
    assert_eq!(doc.config.style, original.style);
    assert_eq!(doc.config.strata_lift, original.strata_lift);
    let edited = doc.config.clone();
    assert_eq!(edited.climate_at(100.0, 500.0), Climate::Desert);
    assert_eq!(edited.climate_at(900.0, 500.0), Climate::Polar);
    let saved = folder.join("environment");
    doc.save(&saved).unwrap();
    let opened = Document::open(&saved.join("terrain.mcmap")).unwrap();
    assert_eq!(opened.config, edited);
    assert_eq!(opened.map.info.water_level, Fx::from_f32(133.25));
    doc.undo().unwrap();
    assert_eq!(doc.config, original);
    assert_eq!(doc.map.info.water_level, water);
    doc.redo().unwrap();
    assert_eq!(doc.config, edited);
    std::fs::remove_dir_all(folder).unwrap();
}

#[test]
fn invalid_environment_does_not_publish_other_actions() {
    let (folder, mut doc, bp) = tests::fixture();
    let original = doc.config.clone();
    let samples = doc.map.samples().to_vec();
    let water = doc.map.info.water_level;
    for bad in [
        Command::Time { hour: f32::NAN },
        Command::Water {
            level: f32::INFINITY,
        },
        Command::Climate {
            region: 12,
            value: Climate::Polar,
        },
        Command::ClimateRegions {
            regions: vec![],
            walls: vec![],
            blend: 0.0,
        },
        Command::ClimateRegions {
            regions: vec![ClimateRegion::default(); 2],
            walls: vec![Wall {
                line: vec![(1.0, 1.0), (1.0, 1.0)],
                left: 0,
                right: 1,
            }],
            blend: 0.0,
        },
    ] {
        assert!(doc
            .apply(
                &[
                    Command::Terrain {
                        at: [500.0, 500.0],
                        radius: 64.0,
                        strength: 10.0,
                        brush: Brush::Raise
                    },
                    Command::Time { hour: 3.0 },
                    Command::Water { level: 200.0 },
                    bad,
                ],
                &bp
            )
            .is_err());
        assert_eq!(doc.config, original);
        assert_eq!(doc.map.samples(), samples);
        assert_eq!(doc.map.info.water_level, water);
        assert!(!doc.can_undo());
    }
    std::fs::remove_dir_all(folder).unwrap();
}

#[test]
fn configuration_read_errors_do_not_silently_replace_settings_with_defaults() {
    let (folder, doc, _) = tests::fixture();
    let path = doc.source.with_extension("ron");
    std::fs::write(&path, "(unknown: 7)").unwrap();
    assert!(Document::open(&doc.source).is_err());
    std::fs::write(&path, " ".repeat(1_048_577)).unwrap();
    assert!(Document::open(&doc.source).err().unwrap().contains("1 MiB"));
    std::fs::remove_dir_all(folder).unwrap();
}
