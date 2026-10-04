//! The outskirts: houses, terraces, a parade of shops, a farmstead, a warehouse,
//! a factory and a tank farm, south of the wall.

use glam::{Vec2, Vec3};
use mc_map::PropKind;

use super::kit::*;
use crate::builder::MeshBuilder;
use crate::gpu_consts::city::{self as pat, BLANK};

// ---- a detached house --------------------------------------------------------------

/// A two-storey detached house: rendered or brick walls on a stone base, a tiled
/// gable roof along its length, a chimney on the west gable, a porch over the front
/// door, and a flat-roofed garage to the east.
pub(super) fn house(b: &mut MeshBuilder, _tech: u8) {
    let (plan, top) = part(PropKind::CityHouse, 0);
    let main = Rect::new(
        plan.min.x + 0.6,
        plan.min.y + 1.0,
        plan.min.x + 11.0,
        plan.max.y - 1.4,
    );
    let wing = Rect::new(main.max.x, main.min.y, plan.max.x - 0.4, main.min.y + 7.8);
    let base = 0.45;
    let eaves = base + 2.0 * pat::HOUSE_STOREY;
    if b.far() {
        solid(b, main, -2.0, eaves + 1.2, pat::HOUSE, pat::ROOF_TILE);
        return;
    }
    if b.mid() {
        plinth(b, main, base, pat::STONE);
        plinth(b, wing, base, pat::STONE);
    }
    let z0 = if b.mid() { base } else { -2.0 };
    // The front: the door in the middle bay on the ground floor, windows either side.
    let (_, cw) = grid(main.size().x, pat::HOUSE_BAY);
    let door_at = main.centre().x;
    let ground = base + pat::HOUSE_STOREY;
    if b.mid() {
        paint(b, pat::HOUSE);
        panel(
            b,
            main,
            Side::Front,
            main.min.x,
            door_at - cw * 0.5,
            z0,
            ground,
            0.0,
        );
        panel(
            b,
            main,
            Side::Front,
            door_at + cw * 0.5,
            main.max.x,
            z0,
            ground,
            0.0,
        );
        paint(b, pat::HOUSE + BLANK);
        panel(
            b,
            main,
            Side::Front,
            door_at - cw * 0.5,
            door_at + cw * 0.5,
            z0,
            ground,
            0.0,
        );
        walls_on(b, main, ground, eaves, &[Side::Front], pat::HOUSE);
        walls_on(b, main, z0, eaves, &[Side::Back, Side::East], pat::HOUSE);
        // The west gable carries the chimney stack: no windows.
        walls_on(b, main, z0, eaves, &[Side::West], pat::HOUSE + BLANK);
        door(b, main, Side::Front, door_at, 1.1, 2.3, true, pat::STONE);
    } else {
        walls(b, main, z0, eaves, pat::HOUSE);
    }
    gable_roof(
        b,
        main,
        eaves,
        top,
        0.45,
        false,
        pat::ROOF_TILE,
        pat::HOUSE + BLANK,
    );
    // The garage: a flat roof behind a low parapet, its door to the street.
    let wing_top = 3.3;
    walls(b, wing, z0, wing_top, pat::HOUSE + BLANK);
    deck(b, wing, wing_top, pat::ROOF_FLAT);
    if b.mid() {
        paint(b, pat::STEEL);
        let c = wing.centre().x;
        panel(b, wing, Side::Front, c - 1.5, c + 1.5, base, 2.7, 0.04);
        // The chimney on the west gable.
        boxed(
            b,
            v3(main.min.x - 0.7, -0.75, 0.0),
            v3(main.min.x + 0.75, 0.55, top + 0.7),
            pat::BRICK,
        );
    }
    if b.fine() {
        // Pots on the chimney, a parapet on the garage, gutters and downpipes.
        paint(b, pat::ROOF_TILE);
        for y in [-0.25, 0.25] {
            b.prism(v3(main.min.x + 0.27, y, top + 0.7), 6, 0.14, 0.12, 0.45);
        }
        parapet(b, wing, wing_top, 0.35, 0.2, pat::HOUSE + BLANK);
        for side in [Side::Front, Side::Back] {
            let (min, max) = side.block(
                main,
                main.min.x - 0.36,
                main.max.x + 0.36,
                0.38,
                0.6,
                eaves - 0.42,
                eaves - 0.25,
            );
            boxed(b, min, max, pat::STEEL);
            downpipe(b, main.grow(0.0), side, main.min.x + 0.3, eaves - 0.3);
        }
        // The front path's step and a meter box.
        boxed(
            b,
            v3(door_at - 0.9, main.max.y, 0.0),
            v3(door_at + 0.9, main.max.y + 0.9, base - 0.05),
            pat::STONE,
        );
        boxed(
            b,
            v3(main.max.x - 1.2, main.max.y, 0.6),
            v3(main.max.x - 0.6, main.max.y + 0.25, 1.4),
            pat::STEEL,
        );
        // A satellite dish under the eaves.
        paint(b, pat::STEEL);
        b.prism(
            v3(main.max.x - 0.8, main.min.y - 0.3, eaves - 1.6),
            8,
            0.35,
            0.35,
            0.06,
        );
    }
}

// ---- a terrace -----------------------------------------------------------------------

/// A row of six two-storey terraced houses in pairs, each with its door beside a
/// canted bay window, a slate roof along the row with chimney stacks on the party
/// walls, front steps.
pub(super) fn rowhouses(b: &mut MeshBuilder, _tech: u8) {
    let (plan, top) = part(PropKind::CityRowhouses, 0);
    let body = Rect::new(plan.min.x, plan.min.y + 0.8, plan.max.x, plan.max.y - 1.2);
    let base = 0.6;
    let eaves = base + 2.0 * pat::TERRACE_STOREY;
    if b.far() {
        solid(b, body, -2.0, eaves + 2.0, pat::TERRACE, pat::ROOF_TILE);
        return;
    }
    let houses = 6;
    let w = body.size().x / houses as f32;
    let z0 = if b.mid() { base } else { -2.0 };
    if b.mid() {
        plinth(b, body, base, pat::STONE);
        walls_on(b, body, z0, eaves, &[Side::Back], pat::TERRACE);
        walls_on(
            b,
            body,
            z0,
            eaves,
            &[Side::East, Side::West],
            pat::TERRACE + BLANK,
        );
        let ground = base + pat::TERRACE_STOREY;
        walls_on(b, body, ground, eaves, &[Side::Front], pat::TERRACE);
        for i in 0..houses {
            let x0 = body.min.x + i as f32 * w;
            // Handed in pairs: the doors of a pair side by side.
            let left = i % 2 == 1;
            let (door_lo, door_hi) = if left {
                (x0, x0 + 2.0)
            } else {
                (x0 + w - 2.0, x0 + w)
            };
            let (win_lo, win_hi) = if left {
                (x0 + 2.0, x0 + w)
            } else {
                (x0, x0 + w - 2.0)
            };
            paint(b, pat::TERRACE + BLANK);
            panel(b, body, Side::Front, door_lo, door_hi, z0, ground, 0.0);
            paint(b, pat::TERRACE);
            panel(b, body, Side::Front, win_lo, win_hi, z0, ground, 0.0);
            door(
                b,
                body,
                Side::Front,
                (door_lo + door_hi) * 0.5,
                1.05,
                2.5,
                false,
                pat::STONE,
            );
            // The bay window: canted sides, its own sash windows, a lead roof.
            let c = (win_lo + win_hi) * 0.5;
            let bay = 2.9;
            let out = 0.75;
            let y = body.max.y;
            let pts = [
                v3(c - bay * 0.5, y, 0.0),
                v3(c - bay * 0.5 + out, y + out, 0.0),
                v3(c + bay * 0.5 - out, y + out, 0.0),
                v3(c + bay * 0.5, y, 0.0),
            ];
            let ring = |z: f32| pts.iter().map(|p| v3(p.x, p.y, z)).collect::<Vec<_>>();
            paint(b, pat::STONE);
            b.loft(&[ring(-1.0), ring(base)], false, true);
            paint(b, pat::TERRACE);
            b.loft(&[ring(base), ring(ground - 0.3)], false, false);
            paint(b, pat::ROOF_TILE);
            b.loft(&[ring(ground - 0.3), ring(ground)], false, true);
            if b.fine() {
                // Steps up to the door.
                let d = (door_lo + door_hi) * 0.5;
                for (k, z) in [(0.0, base), (0.3, base * 0.5)] {
                    boxed(
                        b,
                        v3(d - 0.7, y, 0.0),
                        v3(d + 0.7, y + 0.35 + k, z),
                        pat::STONE,
                    );
                }
            }
        }
    } else {
        walls(b, body, z0, eaves, pat::TERRACE);
    }
    gable_roof(
        b,
        body,
        eaves,
        top - 0.2,
        0.3,
        false,
        pat::ROOF_TILE,
        pat::TERRACE + BLANK,
    );
    if b.mid() {
        // Chimney stacks on the party walls and the end gables.
        for i in 0..=houses {
            let x = (body.min.x + i as f32 * w).clamp(body.min.x + 0.5, body.max.x - 0.5);
            boxed(
                b,
                v3(x - 0.5, -1.3, top - 1.6),
                v3(x + 0.5, 1.3, top + 0.6),
                pat::BRICK,
            );
            if b.fine() {
                paint(b, pat::ROOF_TILE);
                for k in 0..4 {
                    b.prism(v3(x, -0.9 + 0.6 * k as f32, top + 0.6), 6, 0.13, 0.11, 0.4);
                }
            }
        }
    }
    if b.fine() {
        for side in [Side::Front, Side::Back] {
            let (min, max) = side.block(
                body,
                body.min.x,
                body.max.x,
                0.25,
                0.42,
                eaves - 0.36,
                eaves - 0.2,
            );
            boxed(b, min, max, pat::STEEL);
        }
        for i in 0..houses {
            downpipe(
                b,
                body,
                Side::Front,
                body.min.x + (i as f32 + 0.02) * w + 0.15,
                eaves - 0.3,
            );
        }
    }
}

// ---- a parade of shops ---------------------------------------------------------------

/// A parade of shops with a storey of flats over them: shopfronts under a canopy
/// along the street, a cornice and parapet, plant on the flat roof, a service yard
/// side behind.
pub(super) fn shops(b: &mut MeshBuilder, _tech: u8) {
    let (plan, top) = part(PropKind::CityShops, 0);
    let body = Rect::new(plan.min.x, plan.min.y, plan.max.x, plan.max.y - 1.4);
    let roof = pat::SHOP_TOP + pat::FLATS_STOREY;
    if b.far() {
        solid(b, body, -2.0, top, pat::FLATS, pat::ROOF_FLAT);
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
        &[Side::Back, Side::East, Side::West],
        pat::FLATS + BLANK,
    );
    walls_on(
        b,
        body,
        pat::SHOP_TOP,
        roof,
        &[Side::Front, Side::Back],
        pat::FLATS,
    );
    walls_on(
        b,
        body,
        pat::SHOP_TOP,
        roof,
        &[Side::East, Side::West],
        pat::FLATS + BLANK,
    );
    if !b.mid() {
        walls(b, body, roof, top, pat::FLATS + BLANK);
        deck(b, body, top, pat::ROOF_FLAT);
        return;
    }
    cornice(b, body, roof + 0.35, 0.35, 0.25, pat::STONE);
    parapet(
        b,
        body,
        roof + 0.35,
        top - roof - 0.35,
        0.25,
        pat::FLATS + BLANK,
    );
    deck(b, body.grow(-0.25), roof + 0.1, pat::ROOF_FLAT);
    // The canopy along the shopfronts, on brackets.
    let (min, max) = Side::Front.block(
        body,
        body.min.x + 0.3,
        body.max.x - 0.3,
        0.0,
        1.35,
        pat::SHOP_HEAD - 0.05,
        pat::SHOP_HEAD + 0.12,
    );
    boxed(b, min, max, pat::STEEL);
    // Service doors behind.
    for x in cells(body.min.x, body.size().x, 11.0) {
        door(b, body, Side::Back, x, 1.2, 2.3, false, pat::CONCRETE);
    }
    plant_with(b, body.grow(-1.5), roof + 0.1, 11, 7, false, false);
    if b.fine() {
        // Pilasters between the shops, and the canopy's brackets.
        for x in cells(body.min.x, body.size().x, pat::SHOP_BAY)
            .map(|c| c + grid(body.size().x, pat::SHOP_BAY).1 * 0.5)
            .take_while(|x| *x < body.max.x - 1.0)
        {
            let (min, max) =
                Side::Front.block(body, x - 0.25, x + 0.25, 0.0, 0.22, 0.0, pat::SHOP_TOP);
            boxed(b, min, max, pat::STONE);
            let (min, max) = Side::Front.block(
                body,
                x - 0.05,
                x + 0.05,
                0.0,
                1.2,
                pat::SHOP_HEAD + 0.12,
                pat::SHOP_HEAD + 0.6,
            );
            boxed(b, min, max, pat::STEEL);
        }
        // Bins and a delivery bay's bollards behind.
        for i in 0..4 {
            let x = body.min.x + 4.0 + i as f32 * 10.5;
            boxed(
                b,
                v3(x, body.min.y - 1.3, 0.0),
                v3(x + 1.2, body.min.y - 0.3, 1.3),
                pat::STEEL,
            );
        }
    }
}

// ---- a farmstead ---------------------------------------------------------------------

/// A timber barn under a gambrel roof, its big doors open to the yard (+y), a lean-to
/// along its back, and a steel grain silo with a cage ladder and a domed cap.
pub(super) fn farmstead(b: &mut MeshBuilder, _tech: u8) {
    let (plan, top) = part(PropKind::CityFarmstead, 0);
    let (silo_plan, silo_top) = part(PropKind::CityFarmstead, 1);
    let barn = plan.grow_xy(-0.4, -0.6);
    let eaves = 5.8;
    let silo_at = silo_plan.centre();
    let silo_r = silo_plan.size().x * 0.5 - 0.2;
    if b.far() {
        solid(b, barn, -2.0, eaves + 3.0, pat::TIMBER, pat::ROOF_METAL);
        paint(b, pat::STEEL);
        b.prism(silo_at.extend(0.0), 4, silo_r, silo_r, silo_top);
        return;
    }
    if b.mid() {
        plinth(b, barn, 0.5, pat::CONCRETE);
    }
    let z0 = if b.mid() { 0.5 } else { -2.0 };
    walls(b, barn, z0, eaves, pat::TIMBER);
    if b.coarse() {
        gable_roof(
            b,
            barn,
            eaves,
            top,
            0.0,
            false,
            pat::ROOF_METAL,
            pat::TIMBER,
        );
        paint(b, pat::STEEL);
        drum(b, silo_at, silo_r, -1.0, silo_top, 6);
        return;
    }
    // The gambrel: steep lower slopes to a knee, shallow upper ones to the ridge.
    let hw = barn.size().y * 0.5 + 0.4;
    let mid = barn.centre().y;
    let knee = (hw * 0.7, eaves + (top - eaves) * 0.62);
    let t = 0.25;
    let profile = [
        [mid - hw, eaves - 0.3 - t],
        [mid - hw, eaves - 0.3],
        [mid - knee.0, knee.1],
        [mid, top],
        [mid + knee.0, knee.1],
        [mid + hw, eaves - 0.3],
        [mid + hw, eaves - 0.3 - t],
        [mid + knee.0, knee.1 - t],
        [mid, top - t],
        [mid - knee.0, knee.1 - t],
    ];
    paint(b, pat::ROOF_METAL);
    b.extrude_x(&profile, barn.min.x - 0.4, barn.max.x + 0.4);
    paint(b, pat::TIMBER);
    for (x, out) in [(barn.min.x, -1.0f32), (barn.max.x, 1.0)] {
        let pts = vec![
            v3(x, mid - barn.size().y * 0.5, eaves),
            v3(x, mid + barn.size().y * 0.5, eaves),
            v3(x, mid + knee.0 - 0.3, knee.1 - t),
            v3(x, mid, top - t),
            v3(x, mid - knee.0 + 0.3, knee.1 - t),
        ];
        facing(b, pts, Vec3::X * out);
    }
    // The silo.
    let sides = b.sides(20);
    paint(b, pat::STEEL);
    b.prism(silo_at.extend(-1.0), sides, silo_r, silo_r, silo_top - 2.0);
    b.prism(silo_at.extend(silo_top - 1.0), sides, silo_r, 0.6, 1.0);
    if !b.mid() {
        return;
    }
    // The yard doors standing open, slid along their rail, and the hay door.
    let c = barn.centre().x;
    paint(b, pat::SHADOW);
    panel(b, barn, Side::Front, c - 3.0, c + 3.0, 0.5, 4.8, 0.03);
    panel(
        b,
        barn,
        Side::West,
        mid - 1.4,
        mid + 1.4,
        eaves + 0.3,
        eaves + 2.3,
        0.03,
    );
    for s in [-1.0f32, 1.0] {
        let (min, max) = Side::Front.block(barn, c + s * 3.0, c + s * 6.2, 0.1, 0.3, 0.5, 4.9);
        boxed(b, min, max, pat::TIMBER);
    }
    // The lean-to along the back.
    let lean = Rect::new(barn.min.x + 3.0, plan.min.y, barn.max.x - 6.0, barn.min.y);
    walls_on(
        b,
        lean,
        0.0,
        3.0,
        &[Side::Back, Side::East, Side::West],
        pat::TIMBER,
    );
    paint(b, pat::ROOF_METAL);
    b.extrude_x(
        &[
            [lean.min.y - 0.3, 2.7],
            [lean.min.y - 0.3, 2.9],
            [lean.max.y, 4.4],
            [lean.max.y, 4.2],
        ],
        lean.min.x - 0.2,
        lean.max.x + 0.2,
    );
    // The silo's base ring, bands, ladder cage and the auger to the barn.
    boxed(
        b,
        v3(silo_at.x - silo_r - 0.4, silo_at.y - silo_r - 0.4, -0.5),
        v3(silo_at.x + silo_r + 0.4, silo_at.y + silo_r + 0.4, 0.4),
        pat::CONCRETE,
    );
    if b.fine() {
        paint(b, pat::STEEL);
        for k in 1..6 {
            let z = k as f32 * (silo_top - 2.0) / 6.0;
            b.prism(silo_at.extend(z), sides, silo_r + 0.08, silo_r + 0.08, 0.12);
        }
        let lx = silo_at.x - silo_r - 0.35;
        for y in [-0.3, 0.3] {
            boxed(
                b,
                v3(lx - 0.05, silo_at.y + y - 0.05, 0.4),
                v3(lx + 0.05, silo_at.y + y + 0.05, silo_top - 1.0),
                pat::STEEL,
            );
        }
        for k in 0..8 {
            let z = 3.0 + k as f32 * 2.2;
            boxed(
                b,
                v3(lx - 0.6, silo_at.y - 0.5, z),
                v3(lx - 0.5, silo_at.y + 0.5, z + 0.08),
                pat::STEEL,
            );
        }
        paint(b, pat::STEEL);
        b.cylinder_between(
            v3(silo_at.x - silo_r * 0.5, silo_at.y, silo_top - 3.0),
            v3(barn.max.x + 0.2, silo_at.y - 1.0, eaves + 1.0),
            0.25,
            0.25,
            6,
        );
        // Hay bales by the yard doors.
        paint(b, pat::TIMBER);
        for k in 0..3 {
            let x = barn.min.x + 2.0 + k as f32 * 1.4;
            b.cylinder_between(
                v3(x, barn.max.y + 1.5, 0.6),
                v3(x + 1.2, barn.max.y + 1.5, 0.6),
                0.6,
                0.6,
                8,
            );
        }
    }
}

// ---- a warehouse ---------------------------------------------------------------------

/// A logistics shed: profiled steel walls on a concrete base, a sawtooth roof with
/// north lights, loading docks along the yard side (+y) under a canopy, a two-storey
/// office in its east bay.
pub(super) fn warehouse(b: &mut MeshBuilder, _tech: u8) {
    let (plan, top) = part(PropKind::CityWarehouse, 0);
    let shed = Rect::new(plan.min.x, plan.min.y, plan.max.x, plan.max.y - 2.6);
    let eaves = top - 2.6;
    if b.far() {
        solid(b, shed, -2.0, top - 1.0, pat::SHED, pat::ROOF_METAL);
        return;
    }
    let office = Rect::new(shed.max.x - 12.0, shed.min.y, shed.max.x, shed.max.y);
    let base = 1.0;
    if b.mid() {
        plinth(b, shed, base, pat::CONCRETE);
    }
    let z0 = if b.mid() { base } else { -2.0 };
    let office_top = base + 2.0 * pat::RIBBON_STOREY;
    if b.mid() {
        let main = Rect::new(shed.min.x, shed.min.y, office.min.x, shed.max.y);
        walls_on(b, main, z0, eaves, &[Side::Front, Side::Back], pat::SHED);
        walls_on(b, shed, z0, eaves, &[Side::West], pat::SHED);
        paint(b, pat::RIBBON);
        panel(
            b,
            office,
            Side::Front,
            office.min.x,
            office.max.x,
            z0,
            office_top,
            0.0,
        );
        panel(
            b,
            office,
            Side::East,
            office.max.y - 14.0,
            office.max.y,
            z0,
            office_top,
            0.0,
        );
        paint(b, pat::SHED + BLANK);
        panel(
            b,
            office,
            Side::Front,
            office.min.x,
            office.max.x,
            office_top,
            eaves,
            0.0,
        );
        panel(
            b,
            office,
            Side::East,
            office.max.y - 14.0,
            office.max.y,
            office_top,
            eaves,
            0.0,
        );
        paint(b, pat::SHED);
        panel(
            b,
            office,
            Side::East,
            office.min.y,
            office.max.y - 14.0,
            z0,
            eaves,
            0.0,
        );
        panel(
            b,
            office,
            Side::Back,
            office.min.x,
            office.max.x,
            z0,
            eaves,
            0.0,
        );
    } else {
        walls(b, shed, z0, eaves, pat::SHED);
    }
    if b.coarse() {
        paint(b, pat::ROOF_METAL);
        b.loft(
            &[shed.ring(eaves), shed.grow(-0.5).ring(top - 1.0)],
            false,
            false,
        );
        deck(b, shed.grow(-0.5), top - 1.0, pat::ROOF_METAL);
        return;
    }
    // The sawtooth: each tooth a slope rising to the east and a glazed north light
    // facing west down from its crest.
    let teeth = 8;
    let tw = shed.size().x / teeth as f32;
    let (y0, y1) = (shed.min.y, shed.max.y);
    for k in 0..teeth {
        let x0 = shed.min.x + k as f32 * tw;
        let x1 = x0 + tw;
        paint(b, pat::ROOF_METAL);
        b.face(&[
            v3(x0, y0, eaves),
            v3(x1, y0, top),
            v3(x1, y1, top),
            v3(x0, y1, eaves),
        ]);
        paint(b, pat::ROOF_GLASS);
        facing(
            b,
            vec![
                v3(x0, y0, eaves),
                v3(x0, y1, eaves),
                v3(x0, y1, top),
                v3(x0, y0, top),
            ],
            Vec3::NEG_X,
        );
        paint(b, pat::SHED + BLANK);
        facing(
            b,
            vec![v3(x0, y0, eaves), v3(x1, y0, eaves), v3(x1, y0, top)],
            Vec3::NEG_Y,
        );
        facing(
            b,
            vec![v3(x0, y1, eaves), v3(x1, y1, eaves), v3(x1, y1, top)],
            Vec3::Y,
        );
    }
    paint(b, pat::SHED + BLANK);
    facing(
        b,
        vec![
            v3(shed.max.x, y0, eaves),
            v3(shed.max.x, y1, eaves),
            v3(shed.max.x, y1, top),
            v3(shed.max.x, y0, top),
        ],
        Vec3::X,
    );
    // The docks: a door per bay, bumpers and a canopy over them.
    let docks = Rect::new(shed.min.x, shed.min.y, office.min.x, shed.max.y);
    for x in cells(docks.min.x + 2.0, docks.size().x - 4.0, 6.5) {
        paint(b, pat::STEEL);
        panel(
            b,
            docks,
            Side::Front,
            x - 1.5,
            x + 1.5,
            base + 0.1,
            base + 3.4,
            0.04,
        );
        paint(b, pat::SHADOW);
        panel(
            b,
            docks,
            Side::Front,
            x - 1.7,
            x + 1.7,
            base + 3.4,
            base + 3.6,
            0.04,
        );
        if b.fine() {
            for s in [-1.0, 1.0] {
                let (min, max) = Side::Front.block(
                    docks,
                    x + s * 1.75 - 0.15,
                    x + s * 1.75 + 0.15,
                    0.0,
                    0.35,
                    base - 0.4,
                    base + 0.2,
                );
                boxed(b, min, max, pat::SHADOW);
            }
            let (min, max) = Side::Front.block(docks, x - 1.6, x + 1.6, 0.0, 1.0, base - 0.2, base);
            boxed(b, min, max, pat::STEEL);
        }
    }
    let (min, max) = Side::Front.block(
        docks,
        docks.min.x + 1.0,
        docks.max.x - 1.0,
        0.0,
        2.4,
        base + 4.6,
        base + 4.9,
    );
    boxed(b, min, max, pat::STEEL);
    door(
        b,
        office,
        Side::Front,
        office.centre().x,
        1.8,
        2.4,
        true,
        pat::CONCRETE,
    );
    for y in cells(shed.min.y + 4.0, shed.size().y - 8.0, 12.0) {
        door(b, shed, Side::West, y, 1.0, 2.2, false, pat::CONCRETE);
    }
    if b.fine() {
        // Canopy hangers, roof vents along the crests, downpipes.
        for x in cells(docks.min.x + 1.0, docks.size().x - 2.0, 9.0) {
            paint(b, pat::STEEL);
            b.beam(
                Side::Front.at(docks, x, 2.3, base + 4.9),
                Side::Front.at(docks, x, 0.0, base + 7.6),
                Vec2::splat(0.08),
                Vec2::splat(0.08),
            );
        }
        for k in 0..teeth {
            let x = shed.min.x + (k as f32 + 1.0) * tw - 0.8;
            for y in [-10.0, 0.0, 10.0] {
                paint(b, pat::STEEL);
                b.prism(v3(x - 1.0, y, top - 0.6), 8, 0.4, 0.35, 1.2);
            }
        }
        for x in cells(shed.min.x, shed.size().x, 16.0) {
            downpipe(b, shed, Side::Back, x, eaves);
        }
    }
}

// ---- a factory -----------------------------------------------------------------------

/// A works: a brick office block on the street end, a twin-gabled production hall
/// behind it with ridge vents and roof lights, ducts and a pipe bridge across to the
/// brick chimney stack at the east end.
pub(super) fn factory(b: &mut MeshBuilder, _tech: u8) {
    let (plan, top) = part(PropKind::CityFactory, 0);
    let (stack_plan, stack_top) = part(PropKind::CityFactory, 1);
    let office = Rect::new(plan.min.x, plan.min.y, plan.min.x + 9.0, plan.max.y);
    let hall = Rect::new(office.max.x, plan.min.y, plan.max.x, plan.max.y);
    let eaves = top - 5.0;
    let base = 0.6;
    let office_top = base + 3.0 * pat::OFFICE_STOREY + 1.3;
    let stack_at = stack_plan.centre();
    let stack_r = stack_plan.size().x * 0.5;
    if b.far() {
        solid(b, hall, -2.0, eaves + 2.5, pat::SHED, pat::ROOF_METAL);
        solid(b, office, -2.0, office_top, pat::OFFICE, pat::ROOF_FLAT);
        paint(b, pat::BRICK);
        b.prism(
            stack_at.extend(0.0),
            4,
            stack_r * 0.8,
            stack_r * 0.5,
            stack_top,
        );
        return;
    }
    if b.coarse() {
        solid(b, hall, -2.0, eaves + 2.5, pat::SHED, pat::ROOF_METAL);
        solid(b, office, -2.0, office_top, pat::OFFICE, pat::ROOF_FLAT);
        boxed(
            b,
            v3(stack_plan.min.x, stack_plan.min.y, -1.0),
            v3(stack_plan.max.x, stack_plan.max.y, 7.0),
            pat::BRICK,
        );
        paint(b, pat::BRICK);
        let ring = |z: f32, r: f32| {
            (0..5)
                .map(|i| (stack_at + Vec2::from_angle(i as f32 * 1.2566) * r).extend(z))
                .collect::<Vec<_>>()
        };
        b.loft(
            &[ring(7.0, stack_r * 0.82), ring(stack_top, stack_r * 0.55)],
            false,
            true,
        );
        return;
    }
    let z0 = if b.mid() { base } else { -2.0 };
    if b.mid() {
        plinth(b, office, base, pat::STONE);
        plinth(b, hall, base, pat::CONCRETE);
    }
    // The office block.
    walls(b, office, z0, office_top - 1.3, pat::OFFICE);
    if b.mid() {
        cornice(b, office, office_top - 0.9, 0.4, 0.3, pat::STONE);
        parapet(b, office, office_top - 0.9, 0.9, 0.3, pat::OFFICE + BLANK);
        deck(b, office.grow(-0.3), office_top - 1.0, pat::ROOF_FLAT);
        door(
            b,
            office,
            Side::West,
            office.centre().y,
            2.4,
            3.0,
            true,
            pat::STONE,
        );
    } else {
        walls(b, office, office_top - 1.3, office_top, pat::OFFICE + BLANK);
        deck(b, office, office_top, pat::ROOF_FLAT);
    }
    // The hall: two gables side by side, a valley between them.
    walls(b, hall, z0, eaves, pat::SHED);
    let halves = [
        Rect::new(hall.min.x, hall.min.y, hall.max.x, hall.centre().y),
        Rect::new(hall.min.x, hall.centre().y, hall.max.x, hall.max.y),
    ];
    for h in halves {
        gable_roof(
            b,
            h,
            eaves,
            top,
            if b.mid() { 0.4 } else { 0.0 },
            false,
            pat::ROOF_METAL,
            pat::SHED + BLANK,
        );
    }
    // The stack.
    let sides = b.sides(16);
    boxed(
        b,
        v3(stack_plan.min.x, stack_plan.min.y, -1.0),
        v3(stack_plan.max.x, stack_plan.max.y, 7.0),
        pat::BRICK,
    );
    paint(b, pat::BRICK);
    b.prism(
        stack_at.extend(7.0),
        sides,
        stack_r * 0.82,
        stack_r * 0.5,
        stack_top - 8.0,
    );
    paint(b, pat::CONCRETE);
    b.prism(
        stack_at.extend(stack_top - 1.0),
        sides,
        stack_r * 0.6,
        stack_r * 0.56,
        1.0,
    );
    if !b.mid() {
        return;
    }
    // Big doors to the yard, and the ducts and pipe bridge to the stack.
    for x in [hall.min.x + 14.0, hall.min.x + 36.0] {
        paint(b, pat::STEEL);
        panel(
            b,
            hall,
            Side::Front,
            x - 3.0,
            x + 3.0,
            base,
            base + 6.0,
            0.04,
        );
    }
    paint(b, pat::STEEL);
    b.cylinder_between(
        v3(hall.max.x - 0.5, stack_at.y + 2.0, eaves - 3.0),
        v3(stack_at.x - stack_r * 0.6, stack_at.y + 0.6, eaves - 3.0),
        0.9,
        0.9,
        b.sides(10),
    );
    b.cylinder_between(
        v3(hall.max.x - 0.5, stack_at.y - 2.5, eaves - 5.5),
        v3(stack_at.x - stack_r * 0.7, stack_at.y - 1.2, eaves - 5.5),
        0.6,
        0.6,
        b.sides(10),
    );
    if b.fine() {
        // The pipe bridge's trestles, the stack's steel bands and ladder, ridge vents,
        // roof lights.
        for x in [hall.max.x + 1.5, hall.max.x + 4.5] {
            boxed(
                b,
                v3(x - 0.15, stack_at.y - 3.0, 0.0),
                v3(x + 0.15, stack_at.y + 3.0, 0.3),
                pat::CONCRETE,
            );
            for y in [stack_at.y - 2.8, stack_at.y + 2.8] {
                boxed(
                    b,
                    v3(x - 0.12, y - 0.12, 0.0),
                    v3(x + 0.12, y + 0.12, eaves - 2.0),
                    pat::STEEL,
                );
            }
            boxed(
                b,
                v3(x - 0.15, stack_at.y - 3.0, eaves - 6.3),
                v3(x + 0.15, stack_at.y + 3.0, eaves - 6.0),
                pat::STEEL,
            );
        }
        paint(b, pat::STEEL);
        for k in 0..5 {
            let z = 12.0 + k as f32 * 7.0;
            let t = (z - 7.0) / (stack_top - 8.0);
            let r = stack_r * (0.82 + (0.5 - 0.82) * t) + 0.06;
            b.prism(stack_at.extend(z), sides, r, r, 0.3);
        }
        for h in halves {
            for x in cells(h.min.x + 3.0, h.size().x - 6.0, 6.0) {
                paint(b, pat::STEEL);
                b.prism(v3(x, h.centre().y, top - 0.2), 6, 0.6, 0.45, 1.1);
            }
            for side in [-1.0f32, 1.0] {
                let y = h.centre().y + side * h.size().y * 0.25;
                let z = eaves + (top - eaves) * 0.5 + 0.06;
                let slope = (top - eaves) / (h.size().y * 0.5);
                let half = 2.0;
                for x in cells(h.min.x + 4.0, h.size().x - 8.0, 10.0) {
                    let dz = half * slope;
                    let p = |dx: f32, dy: f32| {
                        v3(x + dx, y + dy, z - dy * side * slope.signum() * dz / half)
                    };
                    paint(b, pat::ROOF_GLASS);
                    facing(
                        b,
                        vec![p(-2.5, -half), p(2.5, -half), p(2.5, half), p(-2.5, half)],
                        Vec3::Z,
                    );
                }
            }
        }
        for x in cells(hall.min.x, hall.size().x, 12.0) {
            downpipe(b, hall, Side::Back, x, eaves);
            downpipe(b, hall, Side::Front, x, eaves);
        }
    }
}

// ---- a tank farm ---------------------------------------------------------------------

/// Four fixed-roof storage tanks in a concrete bund, a stair round each to its roof,
/// wind girders and a rail round the roof edge, and the pipework to a manifold in the
/// middle.
pub(super) fn tank_farm(b: &mut MeshBuilder, _tech: u8) {
    let tanks: Vec<(Vec2, f32, f32)> = (0..4)
        .map(|i| {
            let (r, top) = part(PropKind::CityTankFarm, i);
            (r.centre(), r.size().x * 0.5 - 0.5, top)
        })
        .collect();
    let sides = b.sides(28);
    for &(at, r, top) in &tanks {
        paint(b, pat::STEEL);
        if b.coarse() {
            // A few facets and no floor: four tanks in a few dozen triangles.
            let n = if b.far() { 4 } else { 5 };
            drum(b, at, r, -1.0, top, n);
            continue;
        }
        let shell = top - 1.2;
        b.prism(at.extend(-1.0), sides, r, r, shell + 1.0);
        b.prism(at.extend(shell), sides, r, r * 0.08, top - shell);
    }
    if b.coarse() {
        return;
    }
    // The bund: a low wall round the lot, and the manifold.
    let lot = tanks
        .iter()
        .fold(Rect::centred(0.0, 0.0, 0.0, 0.0), |acc, &(at, r, _)| {
            Rect::new(
                acc.min.x.min(at.x - r),
                acc.min.y.min(at.y - r),
                acc.max.x.max(at.x + r),
                acc.max.y.max(at.y + r),
            )
        })
        .grow(1.2);
    let inner = lot.grow(-0.4);
    walls(b, lot, -1.0, 1.4, pat::CONCRETE);
    paint(b, pat::CONCRETE);
    b.loft(&[inner.ring(1.4), inner.ring(0.0)], false, false);
    ledge(b, inner, lot, 1.4, true, pat::CONCRETE);
    boxed(b, v3(-2.0, -1.5, 0.0), v3(2.0, 1.5, 2.6), pat::STEEL);
    for &(at, r, top) in &tanks {
        let toward = (Vec2::ZERO - at).normalize();
        let from = at + toward * r;
        paint(b, pat::STEEL);
        b.cylinder_between(from.extend(0.9), (toward * -2.0).extend(0.9), 0.3, 0.3, 6);
        if !b.fine() {
            continue;
        }
        let shell = top - 1.2;
        // Wind girder and roof rail.
        paint(b, pat::STEEL);
        b.prism(at.extend(shell - 2.0), sides, r + 0.5, r + 0.5, 0.25);
        // The stair: flights up round the shell on its outer side.
        let flights = 8;
        let start = at.y.atan2(at.x) - 1.3;
        for k in 0..flights {
            let a0 = start + k as f32 * 0.33;
            let a1 = a0 + 0.33;
            let z0 = k as f32 * shell / flights as f32;
            let z1 = z0 + shell / flights as f32;
            let p0 = at + Vec2::from_angle(a0) * (r + 0.7);
            let p1 = at + Vec2::from_angle(a1) * (r + 0.7);
            paint(b, pat::STEEL);
            b.beam(
                p0.extend(z0),
                p1.extend(z1),
                Vec2::new(0.9, 0.15),
                Vec2::new(0.9, 0.15),
            );
            b.beam(
                (p0 + (p0 - at).normalize() * 0.45).extend(z0 + 1.0),
                (p1 + (p1 - at).normalize() * 0.45).extend(z1 + 1.0),
                Vec2::splat(0.06),
                Vec2::splat(0.06),
            );
        }
        // A vent and a gauge hatch on the roof.
        b.prism(at.extend(top - 0.6), 8, 0.5, 0.5, 1.0);
        boxed(
            b,
            (at + Vec2::new(r * 0.5, 0.0)).extend(top - 0.9),
            (at + Vec2::new(r * 0.5 + 1.0, 1.0)).extend(top - 0.3),
            pat::STEEL,
        );
    }
    if b.fine() {
        // A pipe bridge over the bund to the road side.
        paint(b, pat::STEEL);
        b.cylinder_between(
            v3(0.0, 1.0, 1.9),
            v3(0.0, lot.max.y + 1.5, 1.9),
            0.3,
            0.3,
            6,
        );
        b.cylinder_between(
            v3(0.6, 1.0, 1.9),
            v3(0.6, lot.max.y + 1.5, 1.9),
            0.25,
            0.25,
            6,
        );
    }
}

/// An upright drum of `n` flat sides with a lid and no floor.
fn drum(b: &mut MeshBuilder, at: Vec2, r: f32, z0: f32, z1: f32, n: usize) {
    let ring = |z: f32| {
        (0..n)
            .map(|i| {
                let a = (i as f32 + 0.5) * std::f32::consts::TAU / n as f32;
                (at + Vec2::from_angle(a) * r).extend(z)
            })
            .collect::<Vec<_>>()
    };
    b.loft(&[ring(z0), ring(z1)], false, true);
}
