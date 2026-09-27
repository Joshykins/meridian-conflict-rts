//! Every strategic icon kind has a shape in `icons.wgsl`, at its own number, and
//! the shader draws no shape that no kind (and no GPU-side marker) uses.

use mc_data::IconKind;
use std::collections::BTreeSet;

/// Shapes the shader picks by itself, not from a blueprint.
const GPU_ONLY: &[(u32, &str)] = &[(31, "unidentified radar contact")];

/// Every kind. The match has no wildcard, so a new variant does not compile until
/// it is listed here too.
fn every_kind() -> Vec<IconKind> {
    use IconKind::*;
    let all = vec![
        Commander,
        Engineer,
        Bot,
        Tank,
        Artillery,
        AntiAir,
        Scout,
        Factory,
        Extractor,
        Power,
        Storage,
        Defense,
        Intel,
        Wall,
        Shield,
        Fighter,
        Bomber,
        Ship,
        Submarine,
        Gunship,
        Transport,
        Silo,
        AntiNuke,
        Warship,
        Titan,
        Salvage,
        SalvageBoat,
        SalvageCarrier,
        SalvageDrone,
        TorpedoBomber,
    ];
    for kind in &all {
        match kind {
            Commander | Engineer | Bot | Tank | Artillery | AntiAir | Scout | Factory
            | Extractor | Power | Storage | Defense | Intel | Wall | Shield | Fighter | Bomber
            | Ship | Submarine | Gunship | Transport | Silo | AntiNuke | Warship | Titan
            | Salvage | SalvageBoat | SalvageCarrier | SalvageDrone | TorpedoBomber => {}
        }
    }
    all
}

/// The `case Nu` numbers of `icon_shape`'s switch.
fn shader_shapes() -> BTreeSet<u32> {
    let source = include_str!("../shaders/icons.wgsl");
    let start = source
        .find("fn icon_shape(")
        .expect("icon_shape in icons.wgsl");
    let body = &source[start..];
    let end = body.find("\n}\n").expect("end of icon_shape");
    body[..end]
        .lines()
        .filter_map(|l| l.trim().strip_prefix("case "))
        .filter_map(|l| l.split('u').next()?.parse().ok())
        .collect()
}

#[test]
fn every_icon_kind_has_a_shape() {
    let shapes = shader_shapes();
    let mut used = BTreeSet::new();
    for kind in every_kind() {
        let id = kind as u32;
        assert!(
            shapes.contains(&id),
            "{kind:?} ({id}) has no case in icons.wgsl icon_shape"
        );
        assert!(used.insert(id), "two kinds share shape {id}");
    }
    for &(id, what) in GPU_ONLY {
        assert!(
            shapes.contains(&id),
            "the {what} shape ({id}) is gone from icons.wgsl"
        );
        assert!(
            used.insert(id),
            "{what} ({id}) shares a number with an icon kind"
        );
    }
    let unused: Vec<_> = shapes.difference(&used).collect();
    assert!(
        unused.is_empty(),
        "icons.wgsl draws shapes nothing uses: {unused:?}"
    );
}
