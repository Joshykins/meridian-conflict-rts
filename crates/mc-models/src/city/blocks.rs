//! The city's blocks: tenements, perimeter blocks round a courtyard, mid-rise
//! flats, offices, a car park, a shopping centre, a burnt-out ruin and rubble.

use glam::{Vec2, Vec3};
use mc_map::PropKind;

use super::kit::*;
use crate::builder::{hash_unit, MeshBuilder};
use crate::gpu_consts::city::{self as pat, BLANK};

// ---- a tenement ----------------------------------------------------------------------

/// A five-storey tenement: shops on the ground floor, four storeys of tall sash
/// windows with iron balconies on the first, a heavy cornice and a slate mansard with
/// dormers and chimney stacks; blank party walls at its ends.
pub(super) fn tenement(b: &mut MeshBuilder, _tech: u8) {
    let (plan, top) = part(PropKind::CityTenement, 0);
    let body = plan;
    let eaves = pat::SHOP_TOP + 4.0 * pat::TERRACE_STOREY;
    if b.far() {
        solid(b, body, -2.0, top - 1.0, pat::TERRACE, pat::ROOF_TILE);
        return;
    }
    let z0 = if b.mid() { 0.0 } else { -2.0 };
    if b.mid() {
        plinth(b, body, 0.05, pat::STONE);
    }
    walls_on(b, body, z0, pat::SHOP_TOP, &[Side::Front], pat::SHOP);
    walls_on(
        b,
        body,
        z0,
        pat::SHOP_TOP,
        &[Side::Back],
        pat::TERRACE + BLANK,
    );
    walls_on(
        b,
        body,
        pat::SHOP_TOP,
        eaves,
        &[Side::Front, Side::Back],
        pat::TERRACE,
    );
    walls_on(
        b,
        body,
        z0,
        eaves,
        &[Side::East, Side::West],
        pat::TERRACE + BLANK,
    );
    let roof_h = top - eaves - 0.2;
    if !b.mid() {
        mansard(b, body, eaves, roof_h, 1.6, pat::ROOF_TILE, pat::ROOF_TILE);
        return;
    }
    cornice(b, body, eaves + 0.5, 0.6, 0.45, pat::STONE);
    mansard(
        b,
        body,
        eaves + 0.5,
        roof_h - 0.5,
        1.6,
        pat::ROOF_TILE,
        pat::ROOF_FLAT,
    );
    // Stair doors behind, a band course over the shops.
    for x in cells(body.min.x, body.size().x, 16.0) {
        door(b, body, Side::Back, x, 1.3, 2.6, false, pat::STONE);
    }
    cornice(b, body, pat::SHOP_TOP + 0.2, 0.3, 0.12, pat::STONE);
    // Chimney stacks on the mansard's flat, at the party walls between the plots.
    for x in [-body.size().x / 6.0, body.size().x / 6.0] {
        boxed(
            b,
            v3(x - 0.6, -1.6, top - 1.6),
            v3(x + 0.6, 1.6, top + 0.6),
            pat::BRICK,
        );
    }
    if b.fine() {
        dormers(
            b,
            body,
            Side::Front,
            pat::TERRACE_BAY * 2.0,
            1.4,
            1.8,
            0.7,
            eaves + 0.7,
            pat::TERRACE,
            pat::ROOF_TILE,
        );
        dormers(
            b,
            body,
            Side::Back,
            pat::TERRACE_BAY * 2.0,
            1.4,
            1.8,
            0.7,
            eaves + 0.7,
            pat::TERRACE,
            pat::ROOF_TILE,
        );
        balconies(
            b,
            body,
            Side::Front,
            pat::TERRACE_BAY,
            2,
            pat::SHOP_TOP + 0.2,
            pat::TERRACE_STOREY,
            1,
            0.6,
            0.6,
            pat::STEEL,
        );
        for x in cells(body.min.x, body.size().x, pat::SHOP_BAY) {
            let (_, step) = grid(body.size().x, pat::SHOP_BAY);
            let x = x + step * 0.5;
            if x < body.max.x - 1.0 {
                let (min, max) =
                    Side::Front.block(body, x - 0.3, x + 0.3, 0.0, 0.25, 0.0, pat::SHOP_TOP);
                boxed(b, min, max, pat::STONE);
            }
        }
        for x in [-body.size().x / 6.0, body.size().x / 6.0] {
            paint(b, pat::ROOF_TILE);
            for k in 0..4 {
                b.prism(v3(x, -1.2 + 0.8 * k as f32, top + 0.6), 6, 0.14, 0.12, 0.45);
            }
        }
        downpipe(b, body, Side::Front, body.min.x + 0.3, eaves);
        downpipe(b, body, Side::Front, body.max.x - 0.3, eaves);
    }
}

// ---- a perimeter block ---------------------------------------------------------------

/// A perimeter block round a courtyard: shops along all four streets, four storeys
/// of flats over them, slate mansards, a corner oriel on each corner, a carriage arch
/// through the south wing into the court.
pub(super) fn courtyard(b: &mut MeshBuilder, _tech: u8) {
    let wings: Vec<(Rect, f32)> = (0..4).map(|i| part(PropKind::CityCourtyard, i)).collect();
    let top = wings[0].1;
    let eaves = pat::SHOP_TOP + 4.0 * pat::TERRACE_STOREY - 0.6;
    let outer = wings.iter().fold(wings[0].0, |a, (r, _)| {
        Rect::new(
            a.min.x.min(r.min.x),
            a.min.y.min(r.min.y),
            a.max.x.max(r.max.x),
            a.max.y.max(r.max.y),
        )
    });
    // The court: the hole the wings leave.
    let court = Rect::new(
        wings[3].0.max.x,
        wings[1].0.max.y,
        wings[2].0.min.x,
        wings[0].0.min.y,
    );
    if b.far() {
        for (r, t) in &wings {
            solid(b, *r, -2.0, *t - 1.0, pat::TERRACE, pat::ROOF_TILE);
        }
        return;
    }
    let z0 = if b.mid() { 0.0 } else { -2.0 };
    // The outer walls, shops under flats; the court's walls, flats from the ground.
    if b.mid() {
        plinth(b, outer, 0.05, pat::STONE);
    }
    if b.mid() {
        walls(b, outer, z0, pat::SHOP_TOP, pat::SHOP);
        walls(b, outer, pat::SHOP_TOP, eaves, pat::TERRACE);
    } else {
        walls(b, outer, z0, eaves, pat::TERRACE);
    }
    paint(b, pat::FLATS);
    b.loft(&[court.ring(eaves), court.ring(-0.5)], false, false);
    if !b.mid() {
        for (r, _) in &wings {
            mansard(
                b,
                *r,
                eaves,
                top - eaves,
                1.4,
                pat::ROOF_TILE,
                pat::ROOF_FLAT,
            );
        }
        return;
    }
    cornice(b, outer, eaves + 0.5, 0.6, 0.45, pat::STONE);
    cornice(b, outer, pat::SHOP_TOP + 0.2, 0.3, 0.12, pat::STONE);
    for (r, _) in &wings {
        mansard(
            b,
            *r,
            eaves + 0.5,
            top - eaves - 0.5,
            1.4,
            pat::ROOF_TILE,
            pat::ROOF_FLAT,
        );
    }
    // The carriage arch through the south wing.
    paint(b, pat::SHADOW);
    panel(b, outer, Side::Back, -2.4, 2.4, 0.0, 4.6, 0.04);
    paint(b, pat::SHADOW);
    panel(b, court, Side::Front, -2.4, 2.4, 0.0, 4.6, -0.04);
    // Doors to the stairs from the court.
    for x in cells(court.min.x, court.size().x, 13.0) {
        door(
            b,
            court.grow(0.0),
            Side::Front,
            x,
            1.2,
            2.4,
            false,
            pat::STONE,
        );
    }
    // Chimneys along the mansards.
    for (r, _) in &wings {
        let c = r.centre();
        let along_x = r.size().x > r.size().y;
        for s in [-0.3f32, 0.3] {
            let p = if along_x {
                c + Vec2::new(r.size().x * s, 0.0)
            } else {
                c + Vec2::new(0.0, r.size().y * s)
            };
            boxed(
                b,
                v3(p.x - 0.7, p.y - 0.7, top - 1.4),
                v3(p.x + 0.7, p.y + 0.7, top + 0.6),
                pat::BRICK,
            );
        }
    }
    if b.fine() {
        // Corner oriels: canted bays up the corners from the first floor.
        for (sx, sy) in [(1.0f32, 1.0f32), (-1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)] {
            let c = Vec2::new(outer.max.x * sx, outer.max.y * sy);
            let d = 2.2;
            let ring = |z: f32| {
                let pts = [
                    c + Vec2::new(-d * sx, 0.0),
                    c + Vec2::new(-d * sx * 0.3, 0.9 * sy),
                    c + Vec2::new(0.9 * sx, 0.9 * sy),
                    c + Vec2::new(0.9 * sx, -d * sy * 0.3),
                    c + Vec2::new(0.0, -d * sy),
                ];
                pts.iter().map(|p| p.extend(z)).collect::<Vec<_>>()
            };
            paint(b, pat::STONE);
            b.loft(
                &[ring(pat::SHOP_TOP - 0.6), ring(pat::SHOP_TOP)],
                true,
                false,
            );
            paint(b, pat::TERRACE);
            b.loft(&[ring(pat::SHOP_TOP), ring(eaves)], false, false);
            paint(b, pat::COPPER);
            let cap = ring(eaves);
            let apex = c.extend(eaves + 3.2);
            for k in 0..cap.len() {
                let j = (k + 1) % cap.len();
                let n = (cap[j] - cap[k]).cross(apex - cap[k]);
                if n.length() > 1e-4 {
                    let out = (cap[k] + cap[j]).truncate() * 0.5 - c;
                    facing(
                        b,
                        vec![cap[k], cap[j], apex],
                        (out.extend(0.4)).normalize_or(Vec3::Z),
                    );
                }
            }
        }
        for (r, _) in &wings {
            let along_x = r.size().x > r.size().y;
            if along_x {
                let side = if r.centre().y > 0.0 {
                    Side::Front
                } else {
                    Side::Back
                };
                dormers(
                    b,
                    *r,
                    side,
                    pat::TERRACE_BAY * 3.0,
                    1.4,
                    1.6,
                    0.6,
                    eaves + 0.8,
                    pat::TERRACE,
                    pat::ROOF_TILE,
                );
            }
        }
        balconies(
            b,
            outer,
            Side::Front,
            pat::TERRACE_BAY,
            3,
            pat::SHOP_TOP + 0.2 + pat::TERRACE_STOREY,
            pat::TERRACE_STOREY,
            1,
            0.6,
            0.6,
            pat::STEEL,
        );
        balconies(
            b,
            outer,
            Side::Back,
            pat::TERRACE_BAY,
            3,
            pat::SHOP_TOP + 0.2 + pat::TERRACE_STOREY,
            pat::TERRACE_STOREY,
            1,
            0.6,
            0.6,
            pat::STEEL,
        );
    }
}

// ---- mid-rise flats --------------------------------------------------------------------

/// A slab of flats: shops on the ground floor, a balcony with a glass front along
/// every floor of the street front, stair and lift cores standing out behind, a
/// parapet, solar panels and plant on the flat roof.
pub(super) fn apartments(b: &mut MeshBuilder, _tech: u8) {
    let (plan, top) = part(PropKind::CityApartments, 0);
    let storey = pat::FLATS_STOREY;
    // The street front stands back for the balconies; the cores stand out behind.
    let body = Rect::new(plan.min.x, plan.min.y + 1.4, plan.max.x, plan.max.y - 1.4);
    let ground = 4.6;
    let floors = ((top - 1.1 - ground) / storey).floor() as usize;
    let eaves = ground + floors as f32 * storey;
    if b.far() {
        solid(b, body, -2.0, top, pat::FLATS, pat::ROOF_FLAT);
        return;
    }
    let z0 = if b.mid() { 0.0 } else { -2.0 };
    if b.mid() {
        plinth(b, body, 0.05, pat::STONE);
    }
    walls_on(b, body, z0, ground, &[Side::Front], pat::SHOP);
    let rest = [Side::Back, Side::East, Side::West];
    walls_on(b, body, z0, ground, &rest, pat::FLATS + BLANK);
    walls_on(
        b,
        body,
        ground,
        eaves,
        &[Side::Front, Side::Back],
        pat::FLATS,
    );
    walls_on(
        b,
        body,
        ground,
        eaves,
        &[Side::East, Side::West],
        pat::FLATS + BLANK,
    );
    // The stair and lift cores behind.
    let cores: Vec<Rect> = [-0.28f32, 0.28]
        .iter()
        .map(|s| {
            let x = body.centre().x + body.size().x * s;
            Rect::new(x - 2.6, plan.min.y, x + 2.6, body.min.y + 0.2)
        })
        .collect();
    for c in &cores {
        solid(b, *c, z0, top, pat::FLATS + BLANK, pat::ROOF_FLAT);
    }
    let along = |z0: f32, z1: f32, off0: f32, off1: f32| {
        Side::Front.block(body, body.min.x + 0.4, body.max.x - 0.4, off0, off1, z0, z1)
    };
    if !b.mid() {
        walls(b, body, eaves, top, pat::FLATS + BLANK);
        deck(b, body, top, pat::ROOF_FLAT);
        // The balconies as one sheet in front of the street face.
        let (min, max) = along(ground, eaves, 0.0, 1.4);
        paint(b, pat::FLATS);
        let y = max.y;
        facing(
            b,
            vec![
                v3(min.x, y, min.z),
                v3(max.x, y, min.z),
                v3(max.x, y, max.z),
                v3(min.x, y, max.z),
            ],
            Vec3::Y,
        );
        return;
    }
    parapet(b, body, eaves, top - eaves, 0.3, pat::FLATS + BLANK);
    deck(b, body.grow(-0.3), eaves + 0.05, pat::ROOF_FLAT);
    door(
        b,
        body,
        Side::Back,
        body.centre().x,
        2.0,
        2.8,
        true,
        pat::CONCRETE,
    );
    // A slab and a glass front along each floor, dividers between the flats.
    for f in 0..floors {
        let z = ground + f as f32 * storey;
        let (min, max) = along(z - 0.2, z, 0.0, 1.4);
        boxed(b, min, max, pat::CONCRETE);
        if b.fine() {
            let (min, max) = along(z, z + 1.05, 1.32, 1.4);
            boxed(b, min, max, pat::CURTAIN + BLANK);
        }
    }
    if b.fine() {
        let (_, step) = grid(body.size().x, pat::FLATS_BAY);
        for x in cells(body.min.x, body.size().x, pat::FLATS_BAY * 3.0) {
            let x = x + step * 0.5;
            let (min, max) = Side::Front.block(body, x - 0.1, x + 0.1, 0.0, 1.4, ground, eaves);
            boxed(b, min, max, pat::CONCRETE);
        }
        let (min, max) = along(eaves - 0.2, eaves, 0.0, 1.4);
        boxed(b, min, max, pat::CONCRETE);
        for c in &cores {
            boxed(
                b,
                v3(c.min.x + 0.6, c.min.y + 0.4, top),
                v3(c.max.x - 0.6, c.max.y - 0.2, top + 0.8),
                pat::CONCRETE,
            );
        }
    }
    let lobby = body.centre().x;
    let (min, max) = Side::Front.block(
        body,
        lobby - 4.0,
        lobby + 4.0,
        0.0,
        2.6,
        ground - 1.2,
        ground - 0.9,
    );
    boxed(b, min, max, pat::STEEL);
    let roof = body.grow(-3.0);
    solar(
        b,
        Rect::new(roof.min.x, roof.min.y, roof.centre().x - 4.0, roof.max.y),
        eaves + 0.05,
    );
    // The cores carry the lifts: no lift house among the plant.
    let rest = Rect::new(roof.centre().x + 2.0, roof.min.y, roof.max.x, roof.max.y);
    plant_with(b, rest, eaves + 0.05, 21, 4, false, false);
}

// ---- an office block ---------------------------------------------------------------------

/// A glass office block: a curtain wall with aluminium fins every other bay over a
/// glazed lobby, a louvred plant screen for a crown with a band of light round it.
pub(super) fn office(b: &mut MeshBuilder, _tech: u8) {
    let (plan, top) = part(PropKind::CityOffice, 0);
    let body = plan;
    let storey = pat::CURTAIN_STOREY;
    let ground = 8.0;
    let floors = ((top - 2.6 - ground) / storey).floor() as usize;
    let eaves = ground + floors as f32 * storey;
    if b.far() {
        solid(b, body, -2.0, top, pat::CURTAIN, pat::ROOF_FLAT);
        return;
    }
    let z0 = if b.mid() { 0.0 } else { -2.0 };
    if b.mid() {
        plinth(b, body, 0.05, pat::STONE);
    }
    walls(b, body, z0, ground, pat::LOBBY);
    walls(b, body, ground, eaves, pat::CURTAIN);
    if !b.mid() {
        walls(b, body, eaves, top, pat::CURTAIN + BLANK);
        deck(b, body, top, pat::ROOF_FLAT);
        return;
    }
    // The plant screen: louvres all round a set-back crown, a band of light under it.
    let screen = body.grow(-1.0);
    walls(b, screen, eaves, top, pat::SHED);
    deck(b, screen, top - 0.1, pat::ROOF_FLAT);
    ledge(b, screen, body, eaves, true, pat::ROOF_FLAT);
    cornice(b, body, eaves + 0.05, 0.45, 0.12, pat::LED);
    let (min, max) = Side::Front.block(
        body,
        body.centre().x - 7.0,
        body.centre().x + 7.0,
        0.0,
        3.2,
        4.8,
        5.2,
    );
    boxed(b, min, max, pat::STEEL);
    if b.fine() {
        let (_, step) = grid(body.size().x, pat::CURTAIN_BAY);
        for side in Side::ALL {
            let (a0, a1, _) = side.run(body);
            for (i, a) in cells(a0, a1 - a0, pat::CURTAIN_BAY).enumerate() {
                if i == 0 || i % 2 == 1 {
                    continue;
                }
                let a = a - step * 0.5;
                let (min, max) = side.block(body, a - 0.05, a + 0.05, 0.0, 0.45, ground, eaves);
                boxed(b, min, max, pat::STEEL);
            }
        }
    }
    plant_with(b, screen.grow(-2.0), eaves + 0.05, 31, 5, false, false);
}

// ---- a car park ------------------------------------------------------------------------

/// A multi-storey car park: open decks behind concrete spandrels, a drum of helical
/// ramps at its east end, stair and lift towers on two corners, cars on the roof.
pub(super) fn garage(b: &mut MeshBuilder, _tech: u8) {
    let (plan, top) = part(PropKind::CityGarage, 0);
    let ramp_r = plan.size().y * 0.25;
    let body = Rect::new(
        plan.min.x,
        plan.min.y,
        plan.max.x - 2.0 * ramp_r - 0.6,
        plan.max.y,
    );
    let ramp_at = Vec2::new(plan.max.x - ramp_r - 0.2, 0.0);
    let decks = 4;
    let roof = decks as f32 * pat::DECKS_STOREY;
    let towers = [
        Rect::new(body.min.x, body.min.y, body.min.x + 5.0, body.min.y + 5.0),
        Rect::new(body.max.x - 5.0, body.max.y - 5.0, body.max.x, body.max.y),
    ];
    if b.far() {
        solid(b, body, -2.0, roof + 1.0, pat::DECKS, pat::PAVING);
        return;
    }
    let z0 = if b.mid() { 0.0 } else { -2.0 };
    walls(b, body, z0, roof + 1.1, pat::DECKS);
    deck(b, body, roof, pat::PAVING);
    paint(b, pat::DECKS);
    let sides = b.sides(20);
    b.prism(ramp_at.extend(z0), sides, ramp_r, ramp_r, roof + 1.1 - z0);
    for t in towers {
        solid(b, t, z0, top, pat::CONCRETE, pat::ROOF_FLAT);
    }
    if !b.mid() {
        return;
    }
    // The way in from the street, and the inner face of the roof's parapet.
    paint(b, pat::SHADOW);
    panel(
        b,
        body,
        Side::Front,
        body.centre().x - 4.0,
        body.centre().x + 4.0,
        0.0,
        2.6,
        0.04,
    );
    paint(b, pat::CONCRETE);
    b.loft(
        &[
            body.grow(-0.25).ring(roof + 1.1),
            body.grow(-0.25).ring(roof),
        ],
        false,
        false,
    );
    ledge(b, body.grow(-0.25), body, roof + 1.1, true, pat::CONCRETE);
    if b.fine() {
        // The barrier, the roof's cars and lamp posts, the helix's top ramp.
        let (min, max) = Side::Front.block(
            body,
            body.centre().x - 3.6,
            body.centre().x - 0.4,
            0.4,
            0.5,
            0.9,
            1.0,
        );
        boxed(b, min, max, pat::STEEL);
        for i in 0..14u32 {
            let x = body.min.x + 7.0 + (i % 7) as f32 * 5.6 + hash_unit(5, i) * 0.6;
            let y = if i < 7 {
                body.max.y - 4.0
            } else {
                body.min.y + 4.0
            };
            if hash_unit(7, i) < 0.3 {
                continue;
            }
            let car = Rect::centred(x, y, 1.0, 2.25);
            solid(b, car, roof, roof + 0.75, pat::STEEL, pat::STEEL);
            solid(
                b,
                car.grow_xy(-0.1, -0.7),
                roof + 0.75,
                roof + 1.35,
                pat::STEEL,
                pat::STEEL,
            );
        }
        for x in cells(body.min.x + 6.0, body.size().x - 12.0, 16.0) {
            mast(b, v3(x, 0.0, roof), 6.0, 0.18);
            boxed(
                b,
                v3(x - 0.8, -0.15, roof + 5.9),
                v3(x + 0.8, 0.15, roof + 6.1),
                pat::STEEL,
            );
        }
        paint(b, pat::PAVING);
        let turns = 12;
        let rings: Vec<Vec<Vec3>> = (0..=turns)
            .map(|k| {
                let a = k as f32 / turns as f32 * std::f32::consts::PI;
                let d = Vec2::from_angle(a);
                let z = roof - 1.5 + 1.5 * k as f32 / turns as f32;
                let inner = ramp_at + d * 1.5;
                let outer = ramp_at + d * (ramp_r - 0.4);
                vec![
                    inner.extend(z - 0.3),
                    outer.extend(z - 0.3),
                    outer.extend(z),
                    inner.extend(z),
                ]
            })
            .collect();
        b.loft(&rings, true, true);
        paint(b, pat::CONCRETE);
        b.prism(ramp_at.extend(0.0), 10, 1.4, 1.4, roof + 1.0);
    }
}

// ---- a shopping centre -----------------------------------------------------------------

/// A shopping centre: shopfronts under a canopy along the street, clad walls with
/// big sign panels over them, a glazed entrance atrium with a glass roof, service
/// docks behind, skylights and plant over the roof.
pub(super) fn mall(b: &mut MeshBuilder, _tech: u8) {
    let (plan, top) = part(PropKind::CityMall, 0);
    let body = Rect::new(plan.min.x, plan.min.y, plan.max.x, plan.max.y - 3.0);
    let roof = top - 3.4;
    let atrium = Rect::new(-11.0, body.max.y - 8.0, 11.0, plan.max.y);
    if b.far() {
        solid(b, body, -2.0, roof + 1.0, pat::CONCRETE, pat::ROOF_FLAT);
        return;
    }
    let z0 = if b.mid() { 0.0 } else { -2.0 };
    if b.mid() {
        plinth(b, body, 0.05, pat::CONCRETE);
    }
    walls_on(b, body, z0, pat::SHOP_TOP, &[Side::Front], pat::SHOP);
    walls_on(
        b,
        body,
        z0,
        pat::SHOP_TOP,
        &[Side::Back, Side::East, Side::West],
        pat::CONCRETE,
    );
    walls(b, body, pat::SHOP_TOP, roof + 1.2, pat::RENDER);
    deck(b, body, roof, pat::ROOF_FLAT);
    // The atrium: a glass box with a pitched glass roof.
    walls(b, atrium, z0, roof + 0.4, pat::CURTAIN);
    gable_roof(
        b,
        atrium,
        roof + 0.4,
        top,
        0.3,
        true,
        pat::ROOF_GLASS,
        pat::CURTAIN,
    );
    if !b.mid() {
        return;
    }
    paint(b, pat::RENDER);
    b.loft(
        &[body.grow(-0.3).ring(roof + 1.2), body.grow(-0.3).ring(roof)],
        false,
        false,
    );
    ledge(b, body.grow(-0.3), body, roof + 1.2, true, pat::CONCRETE);
    // The canopy along the shops; sign panels over it.
    for (a0, a1) in [
        (body.min.x + 1.0, atrium.min.x - 0.5),
        (atrium.max.x + 0.5, body.max.x - 1.0),
    ] {
        let (min, max) =
            Side::Front.block(body, a0, a1, 0.0, 2.6, pat::SHOP_HEAD, pat::SHOP_HEAD + 0.3);
        boxed(b, min, max, pat::STEEL);
        let mid = (a0 + a1) * 0.5;
        let (min, max) = Side::Front.block(body, mid - 7.0, mid + 7.0, 0.0, 0.3, 7.5, 11.5);
        boxed(b, min, max, pat::STEEL);
    }
    door(b, atrium, Side::Front, 0.0, 6.0, 3.2, false, pat::STEEL);
    // Service docks behind.
    for x in cells(body.min.x + 10.0, body.size().x - 20.0, 8.0) {
        paint(b, pat::STEEL);
        panel(b, body, Side::Back, x - 1.6, x + 1.6, 1.0, 4.4, 0.04);
    }
    // Skylights over the malls inside, and plant.
    for y in [-14.0f32, 8.0] {
        let strip = Rect::new(body.min.x + 8.0, y - 2.0, body.max.x - 8.0, y + 2.0);
        gable_roof(
            b,
            strip,
            roof,
            roof + 1.6,
            0.1,
            false,
            pat::ROOF_GLASS,
            pat::CONCRETE,
        );
    }
    plant(
        b,
        Rect::new(body.min.x + 6.0, -6.0, body.max.x - 6.0, 2.0),
        roof,
        41,
        12,
        false,
    );
}

// ---- a ruin ------------------------------------------------------------------------------

/// A burnt-out shell: its walls standing to ragged tops, a storey's windows empty
/// holes in each, the roof and floors fallen in to a heap of slabs, beams and rubble.
pub(super) fn ruin(b: &mut MeshBuilder, _tech: u8) {
    let (plan, top) = part(PropKind::CityRuin, 0);
    let shell = plan.grow(-0.4);
    let thick = 0.5;
    if b.far() {
        walls(b, shell, -2.0, top * 0.6, pat::GUTTED);
        return;
    }
    if b.coarse() {
        paint(b, pat::GUTTED);
        walls(b, shell, -2.0, top * 0.66, pat::GUTTED);
        b.loft(
            &[
                shell.grow(-thick).ring(top * 0.66),
                shell.grow(-thick).ring(0.0),
            ],
            false,
            false,
        );
        return;
    }
    let storey = pat::GUTTED_STOREY;
    let most = ((top - 1.0) / storey).floor() as usize;
    // Each wall in strips a bay wide, each standing a few storeys and a ragged top.
    let mut k = 0u32;
    for side in Side::ALL {
        let (a0, a1, _) = side.run(shell);
        let (n, step) = grid(a1 - a0, pat::GUTTED_BAY);
        // Strips a bay wide up close, three bays wide further off.
        let group = if b.fine() { 1 } else { 3 };
        for i in (0..n).step_by(group) {
            k += 1;
            let lo = a0 + i as f32 * step;
            let hi = lo + step * group.min(n - i) as f32;
            // The corners stand to the top; between them the walls are down to a storey
            // or two in places.
            let corner = i == 0 || i + group >= n;
            let h = if corner {
                most
            } else {
                1 + (hash_unit(17, k) * most as f32) as usize
            };
            let h = h.min(most).max(1);
            let z1 = h as f32 * storey;
            let (min, max) = side.block(shell, lo, hi, -thick, 0.0, -1.0, z1);
            walls(
                b,
                Rect::new(min.x, min.y, max.x, max.y),
                -1.0,
                z1,
                pat::GUTTED,
            );
            deck(b, Rect::new(min.x, min.y, max.x, max.y), z1, pat::RUBBLE);
            // A broken top over it: the masonry snapped along a ragged line, higher
            // at one end, a notch where a lintel fell.
            if b.fine() || corner {
                let most_up = (top - z1).max(0.6);
                let up = |salt: u32| {
                    if corner {
                        most_up * (0.6 + 0.4 * hash_unit(salt, k))
                    } else {
                        most_up.min(2.4) * hash_unit(salt, k)
                    }
                };
                let along = |t: f32| lo + (hi - lo) * t;
                let notch = 0.3 + 0.4 * hash_unit(23, k);
                let at = |t: f32, z: f32| side.at(shell, along(t), 0.0, z);
                let mut ring = vec![at(0.0, z1), at(1.0, z1), at(1.0, z1 + up(29))];
                ring.push(at(notch + 0.12, z1 + up(31) * 0.5));
                ring.push(at(notch, z1 + up(37)));
                ring.push(at(0.0, z1 + if corner { most_up } else { up(41) }));
                let inward = -side.out() * thick;
                let back: Vec<_> = ring.iter().map(|p| *p + inward).collect();
                paint(b, pat::GUTTED + BLANK);
                b.loft(&[ring, back], true, true);
            }
        }
    }
    // The heap inside: fallen floors leaning on it, beams, the rubble.
    let c = shell.centre();
    paint(b, pat::RUBBLE);
    b.lumpy_spheroid(
        c.extend(-0.4),
        v3(shell.size().x * 0.4, shell.size().y * 0.38, 2.4),
        b.sides(12),
        5,
        0.2,
        29,
    );
    if b.mid() {
        for (i, (dx, tilt)) in [(-0.25f32, 0.35f32), (0.2, -0.5), (0.05, 0.25)]
            .iter()
            .enumerate()
        {
            let at = v3(
                c.x + shell.size().x * dx,
                c.y + (i as f32 - 1.0) * 3.0,
                2.5 + i as f32,
            );
            b.pitched(at, *tilt, |b| {
                b.yawed(Vec3::ZERO, 0.4 * i as f32, |b| {
                    boxed(b, -v3(5.0, 3.5, 0.15), v3(5.0, 3.5, 0.15), pat::RUBBLE);
                });
            });
        }
    }
    if b.fine() {
        for i in 0..6u32 {
            let a = v3(
                c.x + (hash_unit(31, i) - 0.5) * 30.0,
                c.y + (hash_unit(37, i) - 0.5) * 16.0,
                0.5,
            );
            let d = Vec2::from_angle(hash_unit(41, i) * std::f32::consts::TAU)
                .extend(0.5 + hash_unit(43, i))
                .normalize();
            paint(b, pat::TIMBER);
            b.beam(
                a,
                a + d * (4.0 + hash_unit(47, i) * 5.0),
                Vec2::splat(0.3),
                Vec2::splat(0.25),
            );
        }
    }
}

// ---- rubble ------------------------------------------------------------------------------

/// What is left where a building came down: a low heap of broken concrete and brick,
/// slabs tipped on it, rebar bristling out of them. Nothing to stand behind.
pub(super) fn rubble(b: &mut MeshBuilder, _tech: u8) {
    paint(b, pat::RUBBLE);
    if b.far() {
        b.lumpy_spheroid(v3(0.0, 0.0, -0.5), v3(9.0, 7.0, 2.2), 4, 2, 0.2, 3);
        return;
    }
    let sides = b.sides(10);
    b.lumpy_spheroid(
        v3(0.0, 0.0, -0.5),
        v3(9.0, 7.0, 2.3),
        sides,
        if b.coarse() { 2 } else { 4 },
        0.25,
        3,
    );
    if b.coarse() {
        return;
    }
    for (i, (x, y, r)) in [(-5.0f32, 3.0f32, 3.6f32), (4.5, -3.0, 4.0), (6.0, 4.0, 2.6)]
        .iter()
        .enumerate()
    {
        if !b.fine() && i > 0 {
            continue;
        }
        b.lumpy_spheroid(
            v3(*x, *y, -0.8),
            v3(*r, *r * 0.8, 1.9),
            sides,
            3,
            0.3,
            7 + i as u32,
        );
    }
    // Slabs tipped over the heap.
    for i in 0..4u32 {
        let at = v3(
            (hash_unit(51, i) - 0.5) * 12.0,
            (hash_unit(53, i) - 0.5) * 9.0,
            1.0,
        );
        let tilt = (hash_unit(57, i) - 0.5) * 0.7;
        b.pitched(at, tilt, |b| {
            b.yawed(Vec3::ZERO, hash_unit(59, i) * 3.0, |b| {
                boxed(b, -v3(2.2, 1.6, 0.12), v3(2.2, 1.6, 0.12), pat::RUBBLE);
            });
        });
    }
    if b.fine() {
        for i in 0..10u32 {
            let a = v3(
                (hash_unit(61, i) - 0.5) * 12.0,
                (hash_unit(63, i) - 0.5) * 9.0,
                0.9,
            );
            let d = Vec2::from_angle(hash_unit(67, i) * std::f32::consts::TAU)
                .extend(0.6 + hash_unit(69, i))
                .normalize();
            paint(b, pat::STEEL);
            b.beam(
                a,
                a + d * (0.8 + hash_unit(71, i) * 1.2),
                Vec2::splat(0.05),
                Vec2::splat(0.04),
            );
        }
    }
}
