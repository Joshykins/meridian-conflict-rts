//! The range's "At Work" scenario: the subject at its own work. A builder in the field
//! raises the first thing it builds beside the pad; a factory makes its first product (a
//! yard raises its hull in the sky over itself, `Builder::aloft`).

use mc_core::FxVec2;
use mc_data::{BlueprintId, UnitBlueprint};

use super::{PendingOrder, Staged};

pub(super) fn at_work(
    bp: &UnitBlueprint,
    pad: FxVec2,
    subject: BlueprintId,
) -> Result<Staged, &'static str> {
    let builder = bp.builder.as_ref().ok_or("This Unit Builds Nothing")?;
    let first = *builder.builds.first().ok_or("This Unit Builds Nothing")?;
    if !bp.works_in_field() {
        // A factory: one of its first product, sent off east when it is done.
        return Ok((
            Vec::new(),
            Some((
                subject,
                PendingOrder::Produce {
                    blueprint: first,
                    count: 1,
                    rally: pad + FxVec2::new(bp.radius * 4, mc_core::Fx::ZERO),
                },
            )),
        ));
    }
    // An engineering station builds well off its own lot, to show its reach.
    let off = if bp.is_station() {
        FxVec2::from_ints(120, 80)
    } else {
        FxVec2::from_ints(36, 44)
    };
    Ok((
        Vec::new(),
        Some((
            subject,
            PendingOrder::Build {
                blueprint: first,
                pos: pad + off,
            },
        )),
    ))
}
