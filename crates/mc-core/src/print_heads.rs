//! Where factories print from: the fabricator heads on each factory model, in
//! model space (metres, x forward, y left, z up from the lot origin). The meshes
//! (`mc-render`'s factory models) stand their fabricators on these mounts and the
//! sim draws build beams from their tips, so the two cannot drift apart. Floats:
//! presentation only (the render mirror draws build beams from these tips;
//! nothing here is written into State). Here and not in mc-sim so the model
//! program builds without the sim.

/// One fabricator head: the trunnion it turns about, its size, the tier whose kit
/// fits it (tech 2 heads go on in the upgrade to tech 2, and so on) and the last tier
/// that keeps it (a later refit may take it down and hang its work elsewhere).
#[derive(Clone, Copy, Debug)]
pub struct PrintHead {
    pub tier: u8,
    pub last: u8,
    pub mount: [f32; 3],
    pub scale: f32,
}

impl PrintHead {
    /// Whether a factory of tech `tech` has this head fitted.
    pub fn fitted(&self, tech: u8) -> bool {
        self.tier <= tech && tech <= self.last
    }
}

/// A factory's heads and the point they all aim at: where the hull is printed.
#[derive(Debug)]
pub struct FactoryHeads {
    pub aim: [f32; 3],
    pub heads: &'static [PrintHead],
}

/// A head's emitter tip is this many `scale`s from its trunnion toward the aim.
pub const TUBE: f32 = 3.6;
/// Height of the land and air factories' lift deck, which their heads aim at.
pub const PAD_DECK: f32 = 1.18;

/// A head fitted from tier `tier` on, kept at every tier after it.
const fn head(tier: u8, x: f32, y: f32, z: f32, scale: f32) -> PrintHead {
    head_until(tier, u8::MAX, x, y, z, scale)
}

/// A head fitted from tier `tier` and taken down after tier `last`.
const fn head_until(tier: u8, last: u8, x: f32, y: f32, z: f32, scale: f32) -> PrintHead {
    PrintHead {
        tier,
        last,
        mount: [x, y, z],
        scale,
    }
}

const LAND: FactoryHeads = FactoryHeads {
    aim: [0.0, 0.0, PAD_DECK],
    heads: &[
        head(1, 0.0, 20.0, 7.8, 1.45),
        head(1, 0.0, -20.0, 7.8, 1.45),
        head(2, 10.0, 20.0, 7.7, 1.3),
        head(2, 10.0, -20.0, 7.7, 1.3),
        head(3, -13.1, 8.0, 7.7, 1.2),
        head(3, -13.1, -8.0, 7.7, 1.2),
    ],
};

const AIR: FactoryHeads = FactoryHeads {
    aim: [0.0, 0.0, PAD_DECK],
    heads: &[
        head(1, -18.6, 5.8, 14.8, 1.45),
        head(1, -18.6, -5.8, 14.8, 1.45),
        head(2, 0.0, 25.2, 11.9, 1.3),
        head(2, 0.0, -25.2, 11.9, 1.3),
        head(3, -7.6, 17.0, 20.6, 1.3),
        head(3, -7.6, -17.0, 20.6, 1.3),
    ],
};

/// The naval yard's heads are all on its quay side (-y): the berth is open water.
const NAVAL: FactoryHeads = FactoryHeads {
    aim: [0.0, 0.0, 3.0],
    heads: &[
        head(1, -24.0, -12.0, 17.4, 1.45),
        head(1, 20.0, -12.0, 17.4, 1.45),
        head(2, -8.0, -25.4, 8.8, 1.3),
        head(2, 6.0, -25.4, 8.8, 1.3),
        head(3, -4.0, -7.0, 29.5, 1.3),
        head(3, 4.0, -7.0, 29.5, 1.3),
    ],
};

/// The Naga land factory (`models::naga::brood`). Tech 1 hangs four heads from the fixed
/// race round its fabrication ring, over the corners of the bay; tech 2 adds two on posts
/// off the press block's face. Tech 3 lifts the ring high enough for the battle scorpion
/// to stand under it: the low race and its four heads come down, and six heads hang from
/// the lifted race instead.
const NAGA_LAND: FactoryHeads = FactoryHeads {
    aim: [0.0, 0.0, 1.2],
    heads: &[
        head_until(1, 2, 8.2, 8.2, 13.4, 1.5),
        head_until(1, 2, -8.2, 8.2, 13.4, 1.5),
        head_until(1, 2, -8.2, -8.2, 13.4, 1.5),
        head_until(1, 2, 8.2, -8.2, 13.4, 1.5),
        head(2, -15.6, 6.6, 12.6, 1.3),
        head(2, -15.6, -6.6, 12.6, 1.3),
        head(3, 11.0, 6.35, 30.6, 1.5),
        head(3, 0.0, 12.7, 30.6, 1.5),
        head(3, -11.0, 6.35, 30.6, 1.5),
        head(3, -11.0, -6.35, 30.6, 1.5),
        head(3, 0.0, -12.7, 30.6, 1.5),
        head(3, 11.0, -6.35, 30.6, 1.5),
    ],
};

/// The Naga air factory (`models::naga::hatchery`): four heads hung from the race high
/// over the pad, between the towers; tech 2 adds two on masts off the flank houses, and
/// tech 3 two more hung from the crown it raises over the race.
const NAGA_AIR: FactoryHeads = FactoryHeads {
    aim: [0.0, 0.0, 1.2],
    heads: &[
        head(1, 12.6, 0.0, 24.0, 1.5),
        head(1, 0.0, 12.6, 24.0, 1.5),
        head(1, -12.6, 0.0, 24.0, 1.5),
        head(1, 0.0, -12.6, 24.0, 1.5),
        head(2, 0.0, 22.6, 12.4, 1.3),
        head(2, 0.0, -22.6, 12.4, 1.3),
        head(3, -11.3, 6.5, 35.0, 1.3),
        head(3, -11.3, -6.5, 35.0, 1.3),
    ],
};

/// The Naga naval factory (`models::naga::tidebrood`): four heads in a row under the
/// gantry's bridge across the slip.
const NAGA_NAVAL: FactoryHeads = FactoryHeads {
    aim: [0.0, 0.0, 2.5],
    heads: &[
        head(1, 2.0, 11.0, 13.6, 1.5),
        head(1, 2.0, 4.5, 13.6, 1.5),
        head(1, 2.0, -4.5, 13.6, 1.5),
        head(1, 2.0, -11.0, 13.6, 1.5),
    ],
};

/// The heads of the factory drawn with `mesh`, or None for a mesh that is not a factory.
pub fn factory_heads(mesh: &str) -> Option<&'static FactoryHeads> {
    match mesh {
        "factory_land" => Some(&LAND),
        "factory_air" => Some(&AIR),
        "factory_naval" => Some(&NAVAL),
        "naga_brood" => Some(&NAGA_LAND),
        "naga_hatchery" => Some(&NAGA_AIR),
        "naga_tidebrood" => Some(&NAGA_NAVAL),
        _ => None,
    }
}

/// Where `head`'s beam leaves: its amber tip, `TUBE * scale` from the trunnion toward `aim`.
pub fn nozzle(head: &PrintHead, aim: [f32; 3]) -> [f32; 3] {
    let [mx, my, mz] = head.mount;
    let d = [aim[0] - mx, aim[1] - my, aim[2] - mz];
    let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt().max(1e-3);
    let t = head.scale * TUBE / len;
    [mx + d[0] * t, my + d[1] * t, mz + d[2] * t]
}
