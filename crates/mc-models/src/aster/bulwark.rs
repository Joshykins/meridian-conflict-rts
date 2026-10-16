//! Bulwark: tech 2 heavy tank. A long split-track hull with a broad turret carrying two
//! rail guns side by side on one cradle. The cradle elevates about a trunnion across the
//! turret's face (the blueprint's `pivot`), so the pair lay up and down onto the mark
//! together and their slugs fly on parallel, the rails' spacing apart. Nothing on it is
//! lit: ARC armour is plain hardware.
use glam::{Vec2, Vec3};

use super::parts::*;
use crate::builder::MeshBuilder;
use crate::material::*;
use crate::{part, pattern, rig, TurretRail};

/// The trunnion the cradle pitches about: the blueprint's `pivot`.
const TRUNNION: Vec3 = Vec3::new(2.6, 0.0, 3.8);
/// The rails' mouths, either side of the bore: the blueprint's `muzzles`.
const MUZZLE_X: f32 = 8.0;
const RAIL_Y: f32 = 0.6;
/// Where each gun's power block starts.
const BREECH_X: f32 = 2.5;
/// One rail's (width, height), and the slot between a gun's pair.
const RAIL: Vec2 = Vec2::new(0.2, 0.4);
const GAP: f32 = 0.2;

/// Where the charge's arcs crawl (renderer heavy_rail_fx.rs), in unit metres (the unit
/// file's `scale` 1.6) along the bore from the trunnion: over the bare rails from the
/// power blocks' fronts (`rail_gun`'s block ends at 0.3 of its length) out to the
/// prongs, on the two guns' tops, jumping the slot between them.
pub(crate) const RAIL_CHARGE: TurretRail = {
    const UNIT: f32 = 1.6;
    let length = MUZZLE_X - BREECH_X;
    let from = BREECH_X + length * 0.33;
    let step = (MUZZLE_X - from) / 6.0;
    let first = (from + step * 0.5 - TRUNNION.x) * UNIT;
    let s = step * UNIT;
    TurretRail {
        breech: (BREECH_X - TRUNNION.x) * UNIT,
        muzzle: (MUZZLE_X - TRUNNION.x) * UNIT,
        rail_y: RAIL_Y * UNIT,
        rail_top: RAIL.y * 0.5 * UNIT,
        arcs: [
            first,
            first + s,
            first + 2.0 * s,
            first + 3.0 * s,
            first + 4.0 * s,
            first + 5.0 * s,
        ],
        arc_half: step * 0.5 * UNIT,
    }
};

/// The hull's armour: plates of uneven size and shade, nothing repeating.
const PLATES: u32 = pattern::HULL_PLATE;

const CHASSIS: Chassis = Chassis {
    rear: -5.8,
    front: 5.9,
    track: (2.7, 4.45, 1.75),
    split_tracks: true,
    deck: 2.75,
    dark: true,
    // No glowing glacis slit or skirt belt: ARC armour is plain hardware.
    lit: false,
};

pub(super) fn build(b: &mut MeshBuilder, _tech: u8) {
    let deck = tracked_chassis_with(b, &CHASSIS, PLATES, false);
    corner_plates(b);
    skirts(b);
    hull_fittings(b, deck);

    b.set_turret_pivot(v3(0.0, 0.0, deck.z));
    b.set_arm_pivot(TRUNNION);
    b.set_recoil(
        v3(BREECH_X, 0.0, TRUNNION.z),
        v3(MUZZLE_X, 0.0, TRUNNION.z),
        1.15,
    );
    b.with_part(part::TURRET, |b| {
        b.paint(PLATING).pattern(PLATES);
        let roof = turret_shell(b, 6.2, 5.0, 2.8, 4.45);
        team_panel(
            b,
            roof.at(0.16, 0.0),
            v2(roof.length() * 0.3, roof.half_width * 1.7),
        );
        b.with_limb(rig::ARM_GUN, cradle);
        b.with_limb(rig::ARM_GUN | rig::RECOIL, guns);
        if b.coarse() {
            return;
        }
        turret_fittings(b, roof);
    });
}

/// The hull's clipped corners, nose and tail, each faced with a bare plate fitted to
/// its facet: the armour's plates are laid out square, and on a slanted facet their
/// seams ran off its edges.
fn corner_plates(b: &mut MeshBuilder) {
    if !b.mid() {
        return;
    }
    b.paint(PLATING_DARK).pattern(pattern::NONE);
    for quad in chassis_corners(&CHASSIS) {
        let centre = quad.iter().copied().sum::<Vec3>() / 4.0;
        let normal = (quad[1] - quad[0])
            .cross(quad[3] - quad[0])
            .normalize_or_zero();
        // Out from the hull, whichever way the facet was wound.
        let out = if normal.truncate().dot(centre.truncate()) < 0.0 {
            -normal
        } else {
            normal
        };
        let mut plate = quad.map(|p| p + (centre - p) * 0.03 + out * 0.03);
        if normal != out {
            plate.reverse();
        }
        b.face(&plate);
    }
}

/// Armoured skirts hung over the outside of each track from a rail along their top:
/// panels of uneven length, each bevelled out at its foot with a lighter armour face
/// bolted on it, over a rubber flap the length of the track. Only the near view has
/// them, as with every tracked hull's.
fn skirts(b: &mut MeshBuilder) {
    if !b.fine() {
        return;
    }
    let (_, outer, height) = CHASSIS.track;
    let (top, foot) = (height + 0.04, height * 0.54);
    let (x0, x1) = (CHASSIS.rear + 0.55, CHASSIS.front - 0.6);
    b.mirror_y(|b| {
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.block(
            v3(x0 - 0.1, outer - 0.05, top - 0.12),
            v3(x1 + 0.1, outer + 0.24, top + 0.06),
        );
        b.paint(TREAD);
        b.block(v3(x0, outer + 0.02, foot - 0.22), v3(x1, outer + 0.1, foot));
        let cuts = [x0, -3.0, -0.7, 1.2, 3.5, x1];
        for pair in cuts.windows(2) {
            let (a, z) = (pair[0] + 0.05, pair[1] - 0.05);
            b.paint(PLATING_DARK).pattern(PLATES);
            b.extrude_x(
                &[
                    [outer, foot],
                    [outer + 0.18, foot + 0.12],
                    [outer + 0.18, top - 0.12],
                    [outer, top - 0.12],
                ],
                a,
                z,
            );
            b.paint(PLATING).pattern(pattern::PLAIN);
            b.extrude_x(
                &[
                    [outer + 0.18, foot + 0.24],
                    [outer + 0.25, foot + 0.3],
                    [outer + 0.25, top - 0.3],
                    [outer + 0.18, top - 0.24],
                ],
                a + 0.22,
                z - 0.22,
            );
        }
    });
}

/// What breaks up the long deck: spare track on the glacis, stowage bins of uneven
/// length down each side, a raised engine deck with grilles, exhaust boxes astern.
fn hull_fittings(b: &mut MeshBuilder, deck: Roof) {
    if !b.mid() {
        return;
    }
    let z = deck.z;
    // Raised engine deck aft of the turret, a dark seam plate across its front edge.
    b.paint(PLATING).pattern(PLATES);
    b.frustum(
        deck.at(0.12, 0.0),
        v2(2.4, deck.half_width * 1.8),
        v2(1.9, deck.half_width * 1.55),
        0.4,
        v2(-0.1, 0.0),
    );
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.plate(
        deck.at(0.24, 0.0),
        v2(0.25, deck.half_width * 1.6),
        0.08,
        0.02,
    );
    // Glacis: a run of spare track links on a dark mounting plate.
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.plate(
        deck.at(0.95, 0.0),
        v2(0.7, deck.half_width * 1.2),
        0.06,
        0.02,
    );
    if b.fine() {
        b.paint(TREAD);
        for k in 0..3 {
            let y = (k as f32 - 1.0) * deck.half_width * 0.38;
            b.block(
                deck.at(0.95, 0.0) + v3(-0.28, y - 0.3, 0.06),
                deck.at(0.95, 0.0) + v3(0.28, y + 0.3, 0.2),
            );
        }
    }
    b.mirror_y(|b| {
        // Stowage bins along the deck edge, a long one and a short one, lids plain.
        let y = deck.half_width * 0.84;
        b.paint(PLATING_DARK);
        b.block(v3(-0.4, y - 0.42, z), v3(1.9, y + 0.12, z + 0.5));
        b.block(v3(2.3, y - 0.36, z), v3(3.3, y + 0.12, z + 0.38));
        b.paint(PLATING).pattern(pattern::PLAIN);
        b.plate(v3(0.75, y - 0.15, z + 0.5), v2(2.1, 0.46), 0.04, 0.015);
        // Exhaust box on the rear corner, its outlet facing aft.
        b.paint(ACCENT);
        b.block(
            v3(deck.rear - 0.2, y - 0.75, z - 0.1),
            v3(deck.rear + 0.6, y - 0.05, z + 0.42),
        );
        if b.fine() {
            b.paint(METAL);
            b.block(
                v3(deck.rear - 0.26, y - 0.62, z + 0.0),
                v3(deck.rear - 0.18, y - 0.18, z + 0.3),
            );
        }
    });
    if b.fine() {
        // Grilles over the engine, one each side of the centre line.
        b.mirror_y(|b| {
            vent(
                b,
                deck.at(0.1, 0.38) + Vec3::Z * 0.4,
                v2(1.3, 0.85),
                5,
                METAL,
            )
        });
    }
}

/// The turret's working kit: cheek armour, smoke launchers, the commander's cupola, a
/// bustle basket full of stowage, sights, aerials.
fn turret_fittings(b: &mut MeshBuilder, roof: Roof) {
    b.paint(ACCENT);
    b.prism(v3(0.0, 0.0, 2.65), 8, 2.3, 2.3, 0.2);
    // Cheek armour modules, bolted on in the hull's dark shade, a darker block at their front.
    b.mirror_y(|b| {
        b.paint(PLATING_DARK);
        b.extrude_y(
            &[[-2.2, 3.05], [1.0, 3.05], [0.6, 4.0], [-1.9, 4.0]],
            2.3,
            2.65,
        );
        b.paint(PLATING_DARK);
        b.extrude_y(
            &[[1.0, 3.05], [2.2, 3.15], [2.0, 3.9], [0.6, 4.0]],
            2.0,
            2.45,
        );
    });
    if !b.fine() {
        return;
    }
    // Dark vision slits along the roof and down the cheeks.
    b.mirror_y(|b| glow_strip(b, roof.at(0.66, 0.86), v2(roof.length() * 0.6, 0.12), TREAD));
    b.mirror_y(|b| {
        b.paint(TREAD);
        b.block(v3(-1.6, 2.65, 3.4), v3(0.2, 2.69, 3.6));
        // Smoke launchers: three cups on a plate, angled out and up.
        b.paint(PLATING_DARK);
        b.block(v3(0.4, 2.0, 4.0), v3(1.2, 2.4, 4.12));
        b.paint(METAL);
        for k in 0..3 {
            let at = v3(0.55 + k as f32 * 0.25, 2.2, 4.12);
            b.cylinder_between(at, at + v3(0.12, 0.18, 0.28), 0.09, 0.09, 4);
        }
    });
    // Commander's cupola with its periscopes, and the loader's flat hatch.
    b.paint(PLATING);
    b.prism(roof.at(0.35, -0.5), 8, 0.62, 0.52, 0.28);
    b.paint(GLASS);
    for k in 0..2 {
        let a = k as f32 * std::f32::consts::FRAC_PI_2 - 0.8;
        let at = roof.at(0.35, -0.5) + v3(a.cos() * 0.48, a.sin() * 0.48, 0.14);
        b.cuboid(at, v3(0.14, 0.14, 0.1));
    }
    b.paint(ACCENT);
    b.prism(roof.at(0.35, -0.5) + Vec3::Z * 0.28, 8, 0.4, 0.36, 0.06);
    b.paint(PLATING).pattern(pattern::PLAIN);
    b.plate(roof.at(0.4, 0.5), v2(0.8, 0.7), 0.06, 0.02);
    // Gunner's sight box on the roof front, its lens dark.
    b.paint(ACCENT);
    b.prism(roof.at(0.75, 0.0), 8, 0.5, 0.42, 0.14);
    b.paint(PLATING_DARK);
    b.block(roof.at(0.86, 0.35), roof.at(0.98, 0.65) + Vec3::Z * 0.35);
    b.paint(GLASS);
    b.cuboid(roof.at(0.98, 0.5) + Vec3::Z * 0.2, v3(0.04, 0.18, 0.14));
    // Bustle basket: a rail round the turret's back holding crates and a rolled tarp.
    let back = roof.rear - 0.55;
    b.paint(METAL);
    let rail = v2(0.07, 0.07);
    let (y0, y1, zt) = (roof.half_width * 1.05, roof.half_width * 0.2, roof.z + 0.3);
    b.mirror_y(|b| {
        b.beam(v3(roof.rear + 0.3, y0, zt), v3(back, y0, zt), rail, rail);
        b.beam(v3(back, y0, zt), v3(back, y1, zt), rail, rail);
        b.beam(v3(back, y0, roof.z - 0.2), v3(back, y0, zt), rail, rail);
    });
    b.paint(PLATING_DARK);
    b.block(
        v3(back + 0.08, -roof.half_width * 0.95, roof.z - 0.15),
        v3(roof.rear + 0.2, -roof.half_width * 0.25, roof.z + 0.22),
    );
    b.paint(ACCENT);
    b.block(
        v3(back + 0.08, roof.half_width * 0.1, roof.z - 0.15),
        v3(roof.rear - 0.1, roof.half_width * 0.6, roof.z + 0.12),
    );
    b.paint(TREAD);
    b.cylinder_between(
        v3(back + 0.3, roof.half_width * 0.62, roof.z + 0.2),
        v3(back + 0.3, roof.half_width * 0.98, roof.z + 0.2),
        0.2,
        0.2,
        6,
    );
    antenna_unlit(b, roof.at(-0.1, -0.7), 0.9, 0.3);
    antenna_unlit(b, roof.at(-0.1, 0.7), 0.6, 0.3);
}

/// The two rail guns, level from breech to muzzle, side by side.
fn guns(b: &mut MeshBuilder) {
    if b.coarse() {
        rail_gun(
            b,
            v3(BREECH_X, 0.0, TRUNNION.z),
            v3(MUZZLE_X, 0.0, TRUNNION.z),
            v2(0.7, 0.45),
            0.5,
            Emitter::Unlit,
        );
        return;
    }
    b.mirror_y(|b| {
        rail_gun(
            b,
            v3(BREECH_X, RAIL_Y, TRUNNION.z),
            v3(MUZZLE_X, RAIL_Y, TRUNNION.z),
            RAIL,
            GAP,
            Emitter::Unlit,
        );
    });
}

/// The cradle the guns ride in, pitching with them but not kicking: a mantlet across the
/// turret's face and the trunnion through it.
fn cradle(b: &mut MeshBuilder) {
    if b.coarse() {
        return;
    }
    b.paint(ACCENT);
    b.block(v3(2.2, -1.5, 3.3), v3(2.9, 1.5, 4.25));
    b.paint(METAL);
    b.cylinder_between(
        TRUNNION - Vec3::Y * 1.65,
        TRUNNION + Vec3::Y * 1.65,
        0.22,
        0.22,
        b.sides(8),
    );
}
