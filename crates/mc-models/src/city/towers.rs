//! Towers: the high-rise, the skyscraper, the spire and the slab. Each stands on a
//! podium (its first plan part) with its shaft over it (the second), built in tiers
//! that step back, and a crown. Few on the map, seen from everywhere: they get far
//! levels and a budget of their own.

use glam::{Vec2, Vec3};
use mc_map::PropKind;

use super::kit::*;
use crate::builder::MeshBuilder;
use crate::gpu_consts::city::{self as pat, BLANK};

/// What crowns a tower.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Crown {
    /// A louvred screen round the roof plant, `h` tall.
    Plant(f32),
    /// A helipad on the roof, its plant under a deck, a mast beside it rising `h`.
    Helipad(f32),
    /// Stepped crown tiers, and a needle rising `h` over the top.
    Needle(f32),
    /// An open frame over the roof, `h` tall.
    Frame(f32),
}

/// A tower's design.
#[derive(Clone, Copy)]
pub(super) struct Design {
    pub kind: PropKind,
    pub facade: u32,
    pub storey: f32,
    /// Corners cut this far (m).
    pub chamfer: f32,
    /// Set-backs: from this share of the shaft's height up, drawn in this far (m).
    pub tiers: &'static [(f32, f32)],
    /// The shaft tapers in facets from a square to a square turned 45 degrees,
    /// shrinking to this share of its width at the top.
    pub taper: Option<f32>,
    pub crown: Crown,
    /// A fin every this many bays (0: none).
    pub fins: usize,
    /// A balcony slab and a glass front round every floor.
    pub balconies: bool,
    /// A stone cornice at the podium and at every set-back.
    pub cornices: bool,
}

/// An octagon `half` wide each way with its corners cut `c`, as a ring at `z`.
fn octagon(half: Vec2, c: f32, z: f32) -> Vec<Vec3> {
    let c = c.clamp(0.0, half.min_element());
    let (x, y) = (half.x, half.y);
    [
        [x, -y + c],
        [x, y - c],
        [x - c, y],
        [-x + c, y],
        [-x, y - c],
        [-x, -y + c],
        [-x + c, -y],
        [x - c, -y],
    ]
    .iter()
    .map(|p| v3(p[0], p[1], z))
    .collect()
}

/// The ring between two outlines at height `z`, facing up (or down).
fn ring_ledge(b: &mut MeshBuilder, inner: &[Vec3], outer: &[Vec3], up: bool, pattern: u32) {
    paint(b, pattern);
    let n = inner.len();
    let out = if up { Vec3::Z } else { Vec3::NEG_Z };
    for k in 0..n {
        let j = (k + 1) % n;
        let quad = vec![outer[k], outer[j], inner[j], inner[k]];
        let area = (quad[1] - quad[0]).cross(quad[2] - quad[0]).length()
            + (quad[2] - quad[0]).cross(quad[3] - quad[0]).length();
        if area > 1e-3 {
            facing(b, quad, out);
        }
    }
}

/// A band round an octagonal outline: its outer face from `z0` to `z1`, standing `out`
/// proud, with its top and underside.
fn band(b: &mut MeshBuilder, half: Vec2, c: f32, z0: f32, z1: f32, out: f32, pattern: u32) {
    let o = half + Vec2::splat(out);
    paint(b, pattern);
    b.loft(&[octagon(o, c, z0), octagon(o, c, z1)], false, false);
    // Its top and underside only read up close.
    if b.fine() {
        ring_ledge(b, &octagon(half, c, z1), &octagon(o, c, z1), true, pattern);
        ring_ledge(b, &octagon(half, c, z0), &octagon(o, c, z0), false, pattern);
    }
}

pub(super) fn tower(b: &mut MeshBuilder, d: Design) {
    let (podium, podium_top) = part(d.kind, 0);
    let (shaft, top) = part(d.kind, 1);
    let half = shaft.size() * 0.5;
    let crown_h = match d.crown {
        Crown::Plant(h) | Crown::Frame(h) => h,
        Crown::Helipad(_) => 2.0,
        Crown::Needle(_) => top * 0.12,
    };
    let shaft_top = top - crown_h;
    let rise = shaft_top - podium_top;
    if b.far() {
        solid(b, podium, -2.0, podium_top, pat::LOBBY, pat::ROOF_FLAT);
        let r = Rect::centred(0.0, 0.0, half.x * 0.9, half.y * 0.9);
        solid(b, r, podium_top, top, d.facade, pat::ROOF_FLAT);
        return;
    }
    let z0 = if b.mid() { 0.0 } else { -2.0 };
    // The podium: a lobby to the street, shops round the rest.
    if b.mid() {
        plinth(b, podium, 0.05, pat::STONE);
    }
    let lobby_top = 8.0f32.min(podium_top - 2.0);
    walls_on(b, podium, z0, lobby_top, &[Side::Front], pat::LOBBY);
    walls_on(
        b,
        podium,
        z0,
        lobby_top,
        &[Side::Back, Side::East, Side::West],
        pat::SHOP,
    );
    walls(
        b,
        podium,
        lobby_top,
        podium_top,
        if d.cornices { pat::OFFICE } else { pat::RIBBON },
    );
    deck(b, podium, podium_top, pat::ROOF_FLAT);
    // The shaft, tier by tier.
    let mut tiers: Vec<(f32, f32, f32)> = Vec::new();
    let mut from = podium_top;
    let mut inset = 0.0;
    for &(share, next) in d.tiers {
        let z = podium_top + rise * share;
        tiers.push((from, z, inset));
        from = z;
        inset = next;
    }
    tiers.push((from, shaft_top, inset));
    if b.coarse() {
        // Two blocks at most: the shaft to its last set-back, and the last tier.
        if tiers.len() > 2 {
            let last = tiers[tiers.len() - 1];
            tiers = vec![(tiers[0].0, last.0, tiers[0].2), last];
        }
        for &(lo, hi, inset) in &tiers {
            let r = Rect::centred(0.0, 0.0, half.x - inset, half.y - inset);
            match d.taper {
                Some(shrink) => {
                    paint(b, d.facade);
                    let h = half.x - inset;
                    b.loft(
                        &[
                            octagon(Vec2::splat(h), 0.0, lo),
                            octagon(Vec2::splat(h * shrink), h * shrink, hi),
                        ],
                        false,
                        true,
                    );
                }
                None => solid(b, r, lo, hi, d.facade, pat::ROOF_FLAT),
            }
        }
        let last = tiers
            .last()
            .copied()
            .unwrap_or((podium_top, shaft_top, 0.0));
        let r = Rect::centred(0.0, 0.0, (half.x - last.2) * 0.7, (half.y - last.2) * 0.7);
        solid(b, r, shaft_top, top, pat::SHED, pat::ROOF_FLAT);
        return;
    }
    let mut roof_half = half;
    let mut roof_c = d.chamfer;
    for (k, &(lo, hi, inset)) in tiers.iter().enumerate() {
        let h = half - Vec2::splat(inset);
        match d.taper {
            Some(shrink) => {
                // Square at the foot to a square turned 45 degrees at the top, in facets.
                let steps = if b.fine() { 8 } else { 4 };
                let rings: Vec<Vec<Vec3>> = (0..=steps)
                    .map(|i| {
                        let t = i as f32 / steps as f32;
                        let z = lo + (hi - lo) * t;
                        let w = h.x * (1.0 + (shrink - 1.0) * t);
                        octagon(Vec2::splat(w), w * t, z)
                    })
                    .collect();
                paint(b, d.facade);
                b.loft(&rings, false, false);
                roof_half = Vec2::splat(h.x * shrink);
                roof_c = roof_half.x;
            }
            None => {
                paint(b, d.facade);
                b.loft(
                    &[octagon(h, d.chamfer, lo), octagon(h, d.chamfer, hi)],
                    false,
                    false,
                );
                roof_half = h;
                roof_c = d.chamfer;
            }
        }
        // The set-back's terrace over the tier below.
        if k > 0 {
            let below = half - Vec2::splat(tiers[k - 1].2);
            ring_ledge(
                b,
                &octagon(h, d.chamfer, lo),
                &octagon(below, d.chamfer, lo),
                true,
                pat::ROOF_FLAT,
            );
            if d.cornices {
                band(b, below, d.chamfer, lo - 0.6, lo + 0.4, 0.4, pat::STONE);
            }
        }
        if d.fins > 0 && b.fine() && d.taper.is_none() {
            fins(b, h, d.chamfer, lo, hi, d.fins, d.facade);
        }
        if d.balconies {
            let floors = ((hi - lo) / d.storey).round() as usize;
            for f in 1..floors {
                let z = lo + f as f32 * (hi - lo) / floors as f32;
                band(b, h, d.chamfer, z - 0.22, z, 1.3, pat::CONCRETE);
                if b.fine() {
                    paint(b, pat::CURTAIN + BLANK);
                    let o = h + Vec2::splat(1.3);
                    b.loft(
                        &[octagon(o, d.chamfer, z), octagon(o, d.chamfer, z + 1.05)],
                        false,
                        false,
                    );
                }
            }
        }
    }
    if d.cornices {
        band(
            b,
            podium.size() * 0.5,
            0.0,
            podium_top - 0.4,
            podium_top + 0.5,
            0.4,
            pat::STONE,
        );
    }
    // Entrance canopy.
    let (min, max) = Side::Front.block(podium, -6.0, 6.0, 0.0, 3.0, 5.0, 5.4);
    boxed(b, min, max, pat::STEEL);
    door(b, podium, Side::Front, 0.0, 4.0, 3.4, false, pat::STEEL);
    if b.fine() {
        plant(
            b,
            Rect::new(
                podium.min.x + 2.0,
                podium.min.y + 2.0,
                podium.max.x - 2.0,
                -half.y - 2.0,
            ),
            podium_top,
            51,
            6,
            false,
        );
    }
    crown(b, d, roof_half, roof_c, shaft_top, top);
}

/// Vertical fins up the four main faces of an octagonal shaft, one every `every` bays.
fn fins(b: &mut MeshBuilder, h: Vec2, c: f32, lo: f32, hi: f32, every: usize, facade: u32) {
    let bay = if facade == pat::CURTAIN {
        pat::CURTAIN_BAY
    } else {
        pat::OFFICE_BAY
    };
    let r = Rect::centred(0.0, 0.0, h.x, h.y);
    for side in Side::ALL {
        let (a0, a1, _) = side.run(r);
        let (a0, a1) = (a0 + c, a1 - c);
        let (n, step) = grid(a1 - a0, bay);
        for i in (every..n).step_by(every) {
            let a = a0 + i as f32 * step;
            let (min, max) = side.block(r, a - 0.12, a + 0.12, 0.0, 0.7, lo, hi);
            boxed(b, min, max, pat::STONE);
        }
    }
}

fn crown(b: &mut MeshBuilder, d: Design, half: Vec2, c: f32, z: f32, top: f32) {
    match d.crown {
        Crown::Plant(h) => {
            // Louvres round the plant, set in a little.
            let s = half - Vec2::splat(1.2);
            ring_ledge(
                b,
                &octagon(s, c, z),
                &octagon(half, c, z),
                true,
                pat::ROOF_FLAT,
            );
            paint(b, pat::SHED);
            b.loft(&[octagon(s, c, z), octagon(s, c, z + h)], false, false);
            paint(b, pat::ROOF_FLAT);
            b.face(&octagon(s, c, z + h - 0.3));
            if b.mid() {
                mast(b, v3(s.x * 0.5, -s.y * 0.4, z + h), 14.0, 0.5);
                beacon(b, v3(s.x * 0.5, -s.y * 0.4, z + h + 14.0));
            }
        }
        Crown::Frame(h) => {
            paint(b, pat::ROOF_FLAT);
            b.face(&octagon(half, c, z));
            let s = half - Vec2::splat(0.4);
            plant(
                b,
                Rect::centred(0.0, 0.0, s.x * 0.6, s.y * 0.6),
                z,
                61,
                6,
                false,
            );
            // Corner posts and a ring beam: the crown's frame.
            for (sx, sy) in [(1.0f32, 1.0f32), (-1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)] {
                let p = Vec2::new(sx * (s.x - c * 0.5), sy * (s.y - c * 0.5));
                boxed(
                    b,
                    (p - Vec2::splat(0.6)).extend(z),
                    (p + Vec2::splat(0.6)).extend(top),
                    pat::CONCRETE,
                );
            }
            band(
                b,
                s - Vec2::splat(1.0),
                c,
                top - 1.6,
                top,
                1.0,
                pat::CONCRETE,
            );
            if b.fine() {
                for k in 1..4 {
                    let zz = z + h * k as f32 / 4.0;
                    band(b, s - Vec2::splat(0.3), c, zz - 0.15, zz, 0.3, pat::STEEL);
                }
            }
        }
        Crown::Helipad(m) => {
            // A deck over the roof plant, the pad on it, a mast at one corner.
            walls(
                b,
                Rect::centred(0.0, 0.0, half.x * 0.85, half.y * 0.85),
                z,
                top - 0.4,
                pat::SHED,
            );
            ring_ledge(
                b,
                &octagon(half * 0.85, c * 0.85, z),
                &octagon(half, c, z),
                true,
                pat::ROOF_FLAT,
            );
            let pad = Rect::centred(0.0, 0.0, half.x * 0.85, half.y * 0.85);
            solid(b, pad.grow(0.0), top - 0.4, top, pat::STEEL, pat::PAVING);
            if b.mid() {
                paint(b, pat::RENDER);
                b.prism(v3(0.0, 0.0, top), 24, half.x * 0.6, half.x * 0.6, 0.04);
                paint(b, pat::PAVING);
                b.prism(v3(0.0, 0.0, top), 24, half.x * 0.55, half.x * 0.55, 0.06);
                // The H.
                let hh = half.x * 0.22;
                for x in [-hh * 0.6, hh * 0.6] {
                    boxed(
                        b,
                        v3(x - 0.6, -hh, top),
                        v3(x + 0.6, hh, top + 0.08),
                        pat::RENDER,
                    );
                }
                boxed(
                    b,
                    v3(-hh * 0.6, -0.6, top),
                    v3(hh * 0.6, 0.6, top + 0.08),
                    pat::RENDER,
                );
                let at = v3(-half.x * 0.7, -half.y * 0.7, top);
                mast(b, at, m, 1.2);
                beacon(b, at + Vec3::Z * m);
            }
            if b.fine() {
                // Perimeter safety net brackets round the pad.
                band(
                    b,
                    half * 0.85,
                    c * 0.85,
                    top - 0.3,
                    top - 0.1,
                    1.2,
                    pat::STEEL,
                );
            }
        }
        Crown::Needle(h) => {
            // Three stepped tiers, then the needle.
            let steps = 3;
            let mut at = z;
            let mut s = half;
            let mut cc = c;
            for k in 0..steps {
                let next = s * 0.78;
                let nc = cc * 0.78;
                let zz = at + (top - z) / steps as f32;
                ring_ledge(
                    b,
                    &octagon(next, nc, at),
                    &octagon(s, cc, at),
                    true,
                    pat::ROOF_FLAT,
                );
                paint(b, if k + 1 == steps { pat::STONE } else { d.facade });
                b.loft(
                    &[octagon(next, nc, at), octagon(next, nc, zz)],
                    false,
                    false,
                );
                if d.cornices && b.mid() {
                    band(b, next, nc, zz - 0.5, zz, 0.3, pat::STONE);
                }
                at = zz;
                s = next;
                cc = nc;
            }
            paint(b, pat::ROOF_FLAT);
            b.face(&octagon(s, cc, top));
            paint(b, pat::STEEL);
            b.prism(v3(0.0, 0.0, top), b.sides(8), s.x * 0.3, 0.3, h);
            if b.mid() {
                beacon(b, v3(0.0, 0.0, top + h));
            }
        }
    }
}

/// An aircraft warning lamp's housing.
fn beacon(b: &mut MeshBuilder, at: Vec3) {
    if !b.fine() {
        return;
    }
    b.paint(crate::material::GLOW_RED);
    b.cuboid(at + Vec3::Z * 0.3, Vec3::splat(0.6));
}

// ---- the designs --------------------------------------------------------------------

/// A residential tower: rendered flats with a balcony round every floor, a plant
/// screen crown.
pub(super) fn highrise(b: &mut MeshBuilder, _tech: u8) {
    tower(
        b,
        Design {
            kind: PropKind::CityHighrise,
            facade: pat::FLATS,
            storey: pat::FLATS_STOREY,
            chamfer: 3.0,
            tiers: &[],
            taper: None,
            crown: Crown::Plant(4.0),
            fins: 0,
            balconies: true,
            cornices: false,
        },
    );
}

/// An office tower in glass, a set-back near the top, fins every other bay.
pub(super) fn highrise_glass(b: &mut MeshBuilder, _tech: u8) {
    tower(
        b,
        Design {
            kind: PropKind::CityHighrise,
            facade: pat::CURTAIN,
            storey: pat::CURTAIN_STOREY,
            chamfer: 0.0,
            tiers: &[(0.82, 3.0)],
            taper: None,
            crown: Crown::Plant(6.0),
            fins: 4,
            balconies: false,
            cornices: false,
        },
    );
}

/// A stone office tower with piers, cornices at its set-backs, an open frame crown.
pub(super) fn highrise_stone(b: &mut MeshBuilder, _tech: u8) {
    tower(
        b,
        Design {
            kind: PropKind::CityHighrise,
            facade: pat::OFFICE,
            storey: pat::OFFICE_STOREY,
            chamfer: 0.0,
            tiers: &[(0.6, 2.5), (0.85, 5.0)],
            taper: None,
            crown: Crown::Frame(7.0),
            fins: 4,
            balconies: false,
            cornices: true,
        },
    );
}

/// A glass skyscraper with cut corners, two set-backs and a mast over its plant.
pub(super) fn skyscraper(b: &mut MeshBuilder, _tech: u8) {
    tower(
        b,
        Design {
            kind: PropKind::CitySkyscraper,
            facade: pat::CURTAIN,
            storey: pat::CURTAIN_STOREY,
            chamfer: 5.0,
            tiers: &[(0.55, 2.5), (0.8, 5.5)],
            taper: None,
            crown: Crown::Plant(8.0),
            fins: 6,
            balconies: false,
            cornices: false,
        },
    );
}

/// A stone skyscraper in tiers, a cornice at each, its crown stepped to a needle.
pub(super) fn skyscraper_deco(b: &mut MeshBuilder, _tech: u8) {
    tower(
        b,
        Design {
            kind: PropKind::CitySkyscraper,
            facade: pat::OFFICE,
            storey: pat::OFFICE_STOREY,
            chamfer: 2.0,
            tiers: &[(0.45, 3.0), (0.7, 6.0), (0.88, 9.0)],
            taper: None,
            crown: Crown::Needle(26.0),
            fins: 4,
            balconies: false,
            cornices: true,
        },
    );
}

/// A ribbon-windowed skyscraper with stone fins, crowned by an open frame.
pub(super) fn skyscraper_frame(b: &mut MeshBuilder, _tech: u8) {
    tower(
        b,
        Design {
            kind: PropKind::CitySkyscraper,
            facade: pat::RIBBON,
            storey: pat::RIBBON_STOREY,
            chamfer: 0.0,
            tiers: &[],
            taper: None,
            crown: Crown::Frame(12.0),
            fins: 2,
            balconies: false,
            cornices: false,
        },
    );
}

/// The city's tallest: glass, stepped back three times, a helipad on its roof.
pub(super) fn spire(b: &mut MeshBuilder, _tech: u8) {
    tower(
        b,
        Design {
            kind: PropKind::CitySpire,
            facade: pat::CURTAIN,
            storey: pat::CURTAIN_STOREY,
            chamfer: 6.0,
            tiers: &[(0.4, 3.0), (0.65, 6.0), (0.85, 9.5)],
            taper: None,
            crown: Crown::Helipad(30.0),
            fins: 6,
            balconies: false,
            cornices: false,
        },
    );
}

/// A tapering glass obelisk: its square foot turning by facets to a smaller square set
/// at 45 degrees, a needle on top.
pub(super) fn spire_taper(b: &mut MeshBuilder, _tech: u8) {
    tower(
        b,
        Design {
            kind: PropKind::CitySpire,
            facade: pat::CURTAIN,
            storey: pat::CURTAIN_STOREY,
            chamfer: 0.0,
            tiers: &[],
            taper: Some(0.62),
            crown: Crown::Needle(40.0),
            fins: 0,
            balconies: false,
            cornices: false,
        },
    );
}

/// A stone spire in the old style: set-backs with cornices, a stepped crown and a needle.
pub(super) fn spire_deco(b: &mut MeshBuilder, _tech: u8) {
    tower(
        b,
        Design {
            kind: PropKind::CitySpire,
            facade: pat::OFFICE,
            storey: pat::OFFICE_STOREY,
            chamfer: 4.0,
            tiers: &[(0.35, 3.0), (0.6, 6.0), (0.8, 9.0), (0.92, 12.0)],
            taper: None,
            crown: Crown::Needle(36.0),
            fins: 4,
            balconies: false,
            cornices: true,
        },
    );
}

/// A slab of flats on a podium of shops: a balcony along every floor of both long
/// faces, blank ends with a stair's windows up them, lift houses on the roof.
pub(super) fn slab(b: &mut MeshBuilder, _tech: u8) {
    let (podium, podium_top) = part(PropKind::CitySlab, 0);
    let (shaft, top) = part(PropKind::CitySlab, 1);
    let body = shaft.grow_xy(0.0, -1.4);
    let roof = top - 1.2;
    if b.far() {
        solid(b, podium, -2.0, podium_top, pat::SHOP, pat::ROOF_FLAT);
        solid(b, body, podium_top, top, pat::FLATS, pat::ROOF_FLAT);
        return;
    }
    let z0 = if b.mid() { 0.0 } else { -2.0 };
    if b.mid() {
        plinth(b, podium, 0.05, pat::STONE);
    }
    walls(b, podium, z0, pat::SHOP_TOP, pat::SHOP);
    walls(b, podium, pat::SHOP_TOP, podium_top, pat::RIBBON);
    deck(b, podium, podium_top, pat::ROOF_FLAT);
    walls_on(
        b,
        body,
        podium_top,
        roof,
        &[Side::Front, Side::Back],
        pat::FLATS,
    );
    walls_on(
        b,
        body,
        podium_top,
        roof,
        &[Side::East, Side::West],
        pat::FLATS + BLANK,
    );
    if !b.mid() {
        walls(b, body, roof, top, pat::FLATS + BLANK);
        deck(b, body, top, pat::ROOF_FLAT);
        return;
    }
    parapet(b, body, roof, top - roof, 0.3, pat::FLATS + BLANK);
    deck(b, body.grow(-0.3), roof + 0.05, pat::ROOF_FLAT);
    // A stair window strip up each end.
    for side in [Side::East, Side::West] {
        paint(b, pat::RIBBON);
        panel(b, body, side, -1.2, 1.2, podium_top, roof, 0.03);
    }
    let floors = ((roof - podium_top) / pat::FLATS_STOREY).round() as usize;
    for f in 1..floors {
        let z = podium_top + f as f32 * (roof - podium_top) / floors as f32;
        for side in [Side::Front, Side::Back] {
            let (min, max) = side.block(body, body.min.x, body.max.x, 0.0, 1.4, z - 0.22, z);
            boxed(b, min, max, pat::CONCRETE);
            if b.fine() {
                let (min, max) = side.block(body, body.min.x, body.max.x, 1.32, 1.4, z, z + 1.05);
                boxed(b, min, max, pat::CURTAIN + BLANK);
            }
        }
    }
    if b.fine() {
        // Dividers between every other flat's balconies.
        let (_, step) = grid(body.size().x, pat::FLATS_BAY);
        for side in [Side::Front, Side::Back] {
            for (i, x) in cells(body.min.x, body.size().x, pat::FLATS_BAY).enumerate() {
                if i % 2 == 1 {
                    continue;
                }
                let x = x + step * 0.5;
                let (min, max) = side.block(body, x - 0.1, x + 0.1, 0.0, 1.4, podium_top, roof);
                boxed(b, min, max, pat::CONCRETE);
            }
        }
    }
    // The lift houses, and a sculpted vent stack.
    for x in [-body.size().x * 0.3, body.size().x * 0.3] {
        let r = Rect::centred(x, 0.0, 3.5, 4.0);
        solid(b, r, roof, top + 0.9, pat::CONCRETE, pat::ROOF_FLAT);
    }
    paint(b, pat::CONCRETE);
    b.prism(v3(0.0, 0.0, roof), b.sides(12), 2.6, 1.8, 2.1);
    plant_with(
        b,
        Rect::centred(0.0, 0.0, body.size().x * 0.15, body.size().y * 0.3),
        roof,
        71,
        5,
        false,
        false,
    );
    let (min, max) = Side::Front.block(podium, -8.0, 8.0, 0.0, 3.0, 4.6, 5.0);
    boxed(b, min, max, pat::STEEL);
}
