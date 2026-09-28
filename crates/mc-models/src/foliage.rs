//! Where each leaf atlas keeps its pictures, `[u0, v0, u1, v1]` (u right, v down,
//! as the image is stored). The tree models pick a region with their card UVs;
//! the renderer's `foliage` module holds the atlases themselves.

/// Broadleaf atlas quadrants `[u0, v0, u1, v1]`: two round clusters (twigs
/// radiating from the middle), a branch end growing up from the bottom edge, and
/// a dense lobed clump for distant crowns.
pub const BROADLEAF_REGIONS: [[f32; 4]; 4] = [
    [0.0, 0.0, 0.5, 0.5],
    [0.5, 0.0, 1.0, 0.5],
    [0.0, 0.5, 0.5, 1.0],
    [0.5, 0.5, 1.0, 1.0],
];
/// Conifer atlas: a flat fir branch seen from above (trunk end at u0, 2:1), a
/// pine needle tuft seen from above, and a whole young fir from the side.
pub const CONIFER_REGIONS: [[f32; 4]; 3] = [
    [0.0, 0.0, 1.0, 0.5],
    [0.0, 0.5, 0.5, 1.0],
    [0.5, 0.5, 1.0, 1.0],
];

/// Tropical atlas (`scripts/make-tropical-foliage.py`): a coconut frond seen from
/// above (rachis along the middle, stalk end at u0, 2:1), a cluster of big glossy
/// rainforest leaves, and a dense lobed clump for distant crowns.
pub const TROPICAL_REGIONS: [[f32; 4]; 3] = [
    [0.0, 0.0, 1.0, 0.5],
    [0.0, 0.5, 0.5, 1.0],
    [0.5, 0.5, 1.0, 1.0],
];

/// Desert atlas (`scripts/make-desert-foliage.py`): a Utah juniper's blue-grey
/// scale sprays, a pinyon's dark needle brushes seen from above, a cluster of
/// bright cottonwood leaves, and a dense cottonwood clump for distant crowns.
pub const DESERT_REGIONS: [[f32; 4]; 4] = [
    [0.0, 0.0, 0.5, 0.5],
    [0.5, 0.0, 1.0, 0.5],
    [0.0, 0.5, 0.5, 1.0],
    [0.5, 0.5, 1.0, 1.0],
];
