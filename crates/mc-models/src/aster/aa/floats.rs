//! The pontoon under an anti-air emplacement built at sea (`part::AFLOAT`: drawn only
//! where it stands in water). The emplacement's pad sits on the waterline and rides the
//! pontoon like a barge: a dark hull below the water, a white gunwale above it.
use super::*;
use crate::builder::{chamfered_rect, Section};
use crate::pattern;

/// The pontoon under a pad `half` metres across each way from its middle, a little wider
/// than it: a dark flared hull from under the waterline, a white gunwale round its top, a
/// black rubbing strake on it. From far off the pad on the water is enough.
pub(super) fn pontoon(b: &mut MeshBuilder, half: f32) {
    if b.coarse() {
        return;
    }
    b.with_part(part::AFLOAT, |b| {
        let plan = chamfered_rect(v2(half * 1.08, half * 1.08), half * 0.26);
        if !b.fine() {
            b.paint(PLATING).pattern(pattern::HULL);
            b.loft_z(&plan, &[Section::new(-1.3, 0.9), Section::new(0.65, 1.0)]);
            return;
        }
        b.paint(PLATING_DARK);
        b.loft_z(
            &plan,
            &[
                Section::new(-1.3, 0.86),
                Section::new(-0.6, 0.97),
                Section::new(0.1, 1.0),
            ],
        );
        b.paint(PLATING).pattern(pattern::HULL);
        b.loft_z(
            &plan,
            &[
                Section::new(0.1, 1.0),
                Section::new(0.55, 1.0),
                Section::new(0.65, 0.985),
            ],
        );
        b.paint(TREAD);
        b.loft_z(
            &plan,
            &[Section::new(0.2, 1.025), Section::new(0.42, 1.025)],
        );
    });
}
