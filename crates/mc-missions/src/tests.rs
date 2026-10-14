use mc_core::{Fx, FxVec2};
use mc_sim::mission::Action;
use std::{collections::BTreeMap, path::Path};
#[test]
fn the_contract_has_resolved_references_and_all_dialogue() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let placed = ron::from_str(include_str!("../../../maps/vela_m1/placed.ron")).unwrap();
    let program = crate::definition(placed);
    let bp = mc_data::Blueprints::load(&root.join("data")).unwrap();
    program
        .validate(&bp, 6, FxVec2::from_ints(12288, 12288), 0)
        .unwrap();
    let strings: BTreeMap<String, Text> =
        ron::from_str(include_str!("../../../maps/vela_m1/strings.en.ron")).unwrap();
    let mut checkpoints = vec![];
    for event in &program.events {
        for action in &event.actions {
            match action {
                Action::Say { line, .. } | Action::Objective { line, .. } => {
                    let text = strings
                        .get(line)
                        .unwrap_or_else(|| panic!("Missing text {line}"));
                    assert!(!text.text.is_empty());
                }
                Action::Checkpoint(beat) => checkpoints.push(*beat),
                _ => {}
            }
        }
    }
    assert_eq!(checkpoints, vec![1, 2, 3, 4, 5, 6]);
    let merrick = bp.unit_by_key("company_merrick").unwrap();
    assert!(merrick.mission_only);
    assert!(!bp.is_listed(merrick.id));
    assert_eq!(merrick.visual.skin.as_deref(), Some("company"));
    assert!(merrick.builder.is_none());
    assert_eq!(merrick.economy.materials_income, Fx::ZERO);
    assert_eq!(merrick.economy.energy_income, Fx::ZERO);
    assert!(merrick.refit.is_none());
}
#[derive(serde::Deserialize)]
struct Text {
    text: String,
}
