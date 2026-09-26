//! `PropKind::ALL` is how a map file's prop numbers are read back
//! (`PropKind::from_raw`): a kind missing from it is silently dropped from every
//! map. This keeps it complete and every number distinct.

use mc_map::PropKind;
use std::collections::BTreeSet;

/// Every kind. The match has no wildcard, so a new variant does not compile until
/// it is listed here too.
fn every_kind() -> Vec<PropKind> {
    use PropKind::*;
    let all = vec![
        TreeBroadleaf,
        TreeConifer,
        TreePine,
        TreeDead,
        TreePalm,
        TreeJungle,
        RockSmall,
        RockLarge,
        BuildingSmall,
        BuildingMedium,
        BuildingLarge,
        BuildingTower,
        PrecursorSpire,
        PrecursorPylon,
        PrecursorArch,
        PrecursorRing,
        PrecursorShard,
        PrecursorWall,
        PrecursorBeacon,
        PrecursorConduit,
        PrecursorFragment,
        PrecursorBastion,
        PrecursorBoom,
        PrecursorTower,
        PrecursorSpan,
        PrecursorViaduct,
        PrecursorPier,
        PrecursorVault,
        PrecursorAxis,
        PrecursorTerrace,
        PrecursorLining,
        PrecursorForge,
        PrecursorCradle,
        PrecursorHeart,
        PrecursorHalo,
        PrecursorMonolith,
        PrecursorSeaGate,
        PrecursorPlatform,
        PrecursorGate,
        PrecursorNeedle,
        PrecursorRampart,
        PrecursorFloor,
        PrecursorSeaway,
        PrecursorCitadel,
    ];
    for kind in &all {
        match kind {
            TreeBroadleaf | TreeConifer | TreePine | TreeDead | TreePalm | TreeJungle
            | RockSmall | RockLarge | BuildingSmall | BuildingMedium | BuildingLarge
            | BuildingTower | PrecursorSpire | PrecursorPylon | PrecursorArch | PrecursorRing
            | PrecursorShard | PrecursorWall | PrecursorBeacon | PrecursorConduit
            | PrecursorFragment | PrecursorBastion | PrecursorBoom | PrecursorTower
            | PrecursorSpan | PrecursorViaduct | PrecursorPier | PrecursorVault | PrecursorAxis
            | PrecursorTerrace | PrecursorLining | PrecursorForge | PrecursorCradle
            | PrecursorHeart | PrecursorHalo | PrecursorMonolith | PrecursorSeaGate
            | PrecursorPlatform | PrecursorGate | PrecursorNeedle | PrecursorRampart
            | PrecursorFloor | PrecursorSeaway | PrecursorCitadel => {}
        }
    }
    all
}

#[test]
fn every_prop_kind_reads_back_from_its_number() {
    let mut numbers = BTreeSet::new();
    for kind in every_kind() {
        assert!(
            PropKind::ALL.contains(&kind),
            "{kind:?} is missing from PropKind::ALL"
        );
        assert!(
            numbers.insert(kind.raw()),
            "{kind:?} reuses number {}",
            kind.raw()
        );
        assert_eq!(PropKind::from_raw(kind.raw()), Some(kind));
    }
    assert_eq!(
        PropKind::ALL.len(),
        every_kind().len(),
        "PropKind::ALL lists a kind twice"
    );
}
