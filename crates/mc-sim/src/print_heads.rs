//! Where factories print from: the fabricator heads on each factory model, in
//! model space (metres, x forward, y left, z up from the lot origin). The meshes
//! (`mc-render`'s factory models) stand their fabricators on these mounts and the
//! sim draws build beams from their tips, so the two cannot drift apart.

/// One fabricator head: the trunnion it turns about, its size, and the tier whose
/// kit fits it (tech 2 heads go on in the upgrade to tech 2, and so on).
#[derive(Clone, Copy, Debug)]
pub struct PrintHead {
    pub tier: u8,
    pub mount: [f32; 3],
    pub scale: f32,
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

const fn head(tier: u8, x: f32, y: f32, z: f32, scale: f32) -> PrintHead {
    PrintHead {
        tier,
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

/// The heads of the factory drawn with `mesh`, or None for a mesh that is not a factory.
pub fn factory_heads(mesh: &str) -> Option<&'static FactoryHeads> {
    match mesh {
        "factory_land" => Some(&LAND),
        "factory_air" => Some(&AIR),
        "factory_naval" => Some(&NAVAL),
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
