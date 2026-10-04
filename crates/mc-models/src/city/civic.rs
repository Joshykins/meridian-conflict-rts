//! The city's public buildings: the civic hall with its dome and portico, the
//! railway station with its arched glass train shed, and the stone church.

use std::f32::consts::{FRAC_PI_2, PI};

use glam::{Vec2, Vec3};
use mc_map::PropKind;

use super::kit::*;
use crate::builder::MeshBuilder;
use crate::gpu_consts::city::{self as pat, BLANK};

// ---- the civic hall ----------------------------------------------------------------------

/// A domed civic hall: two tall storeys of arched windows over a rusticated base, a
/// cornice and a balustraded attic, a columned portico with a pediment to the square
/// (+y), and a copper dome on a windowed drum with a lantern.
pub(super) fn civic(b: &mut MeshBuilder, _tech: u8) {
    let (plan, top) = part(PropKind::CityCivic, 0);
    let portico = Rect::new(-14.0, plan.max.y - 8.0, 14.0, plan.max.y);
    let body = Rect::new(plan.min.x, plan.min.y, plan.max.x, portico.min.y);
    let base = 1.4;
    let eaves = base + 2.0 * pat::ARCHED_STOREY;
    let attic = eaves + 4.0;
    let drum_r = 10.0;
    let drum_top = top - 7.5;
    if b.far() {
        solid(b, body, -2.0, attic, pat::ARCHED, pat::ROOF_FLAT);
        paint(b, pat::COPPER);
        b.prism(v3(0.0, -4.0, attic), 6, drum_r, drum_r * 0.4, top - attic);
        return;
    }
    let z0 = if b.mid() { base } else { -2.0 };
    if b.mid() {
        plinth(b, body, base, pat::STONE);
    }
    walls(b, body, z0, eaves, pat::ARCHED);
    let centre = v3(0.0, body.centre().y, 0.0);
    if !b.mid() {
        walls(b, body, eaves, attic, pat::ARCHED + BLANK);
        deck(b, body, attic, pat::ROOF_FLAT);
        paint(b, pat::COPPER);
        b.prism(
            centre + Vec3::Z * attic,
            6,
            drum_r,
            drum_r * 0.35,
            top - attic,
        );
        solid(b, portico, -2.0, eaves, pat::STONE, pat::STONE);
        return;
    }
    cornice(b, body, eaves + 0.6, 1.0, 0.8, pat::STONE);
    walls(b, body, eaves + 0.6, attic, pat::ARCHED + BLANK);
    cornice(b, body, attic, 0.5, 0.4, pat::STONE);
    deck(b, body, attic - 0.5, pat::ROOF_FLAT);
    cornice(b, body, base + 4.0, 0.3, 0.12, pat::STONE);
    // The drum and the dome.
    let sides = b.sides(24);
    paint(b, pat::ARCHED);
    b.prism(
        centre + Vec3::Z * (attic - 0.5),
        sides,
        drum_r,
        drum_r,
        drum_top - attic + 0.5,
    );
    let rings = if b.fine() { 7 } else { 4 };
    let dome: Vec<Vec<Vec3>> = (0..=rings)
        .map(|k| {
            let a = k as f32 / rings as f32 * FRAC_PI_2 * 0.98;
            let r = (drum_r + 0.3) * a.cos();
            let z = drum_top + (top - drum_top - 0.4) * a.sin();
            (0..sides)
                .map(|i| {
                    let t = (i as f32 + 0.5) * std::f32::consts::TAU / sides as f32;
                    centre + v3(t.cos() * r, t.sin() * r, z)
                })
                .collect()
        })
        .collect();
    paint(b, pat::COPPER);
    b.loft(&dome, true, true);
    if b.fine() {
        // The lantern and its cupola, copper-clad.
        paint(b, pat::COPPER);
        b.prism(centre + Vec3::Z * (top - 0.6), 8, 1.6, 1.6, 2.6);
        paint(b, pat::COPPER);
        b.prism(centre + Vec3::Z * (top + 2.0), 8, 1.9, 0.1, 2.2);
        paint(b, pat::STONE);
        b.prism(
            centre + Vec3::Z * (drum_top - 0.2),
            sides,
            drum_r + 0.5,
            drum_r + 0.5,
            0.6,
        );
        // A balustrade along the attic's edge.
        for side in Side::ALL {
            let (a0, a1, _) = side.run(body);
            for a in cells(a0 + 1.0, a1 - a0 - 2.0, 1.2) {
                let (min, max) =
                    side.block(body, a - 0.18, a + 0.18, -0.6, -0.25, attic, attic + 0.9);
                boxed(b, min, max, pat::STONE);
            }
            let (min, max) = side.block(
                body,
                a0 + 0.5,
                a1 - 0.5,
                -0.65,
                -0.2,
                attic + 0.9,
                attic + 1.15,
            );
            boxed(b, min, max, pat::STONE);
        }
    }
    // The portico: steps, eight columns, the entablature and the pediment.
    for (k, inset) in [0.0f32, 1.0, 2.0].iter().enumerate() {
        let r = Rect::new(
            portico.min.x - 2.0 + inset,
            portico.min.y,
            portico.max.x + 2.0 - inset,
            portico.max.y - inset,
        );
        boxed(
            b,
            r.ring(0.0)[0].with_z(-1.0),
            r.ring(0.0)[2].with_z((k + 1) as f32 * base / 3.0),
            pat::STONE,
        );
    }
    let floor = base;
    let entab = eaves - 2.0;
    paint(b, pat::STONE);
    let cols = 8;
    let col_r = 0.85;
    for i in 0..cols {
        let x = portico.min.x + 1.2 + (portico.size().x - 2.4) * i as f32 / (cols - 1) as f32;
        let y = portico.max.y - 3.2;
        paint(b, pat::STONE);
        b.prism(
            v3(x, y, floor),
            b.sides(12),
            col_r,
            col_r * 0.85,
            entab - floor,
        );
        if b.fine() {
            boxed(
                b,
                v3(x - 1.0, y - 1.0, entab - 0.5),
                v3(x + 1.0, y + 1.0, entab),
                pat::STONE,
            );
            boxed(
                b,
                v3(x - 1.0, y - 1.0, floor),
                v3(x + 1.0, y + 1.0, floor + 0.5),
                pat::STONE,
            );
        }
    }
    let porch = Rect::new(
        portico.min.x,
        portico.min.y,
        portico.max.x,
        portico.max.y - 1.8,
    );
    boxed(
        b,
        porch.ring(0.0)[0].with_z(entab),
        porch.ring(0.0)[2].with_z(eaves + 0.6),
        pat::STONE,
    );
    paint(b, pat::STONE);
    let peak = eaves + 0.6 + porch.size().x * 0.2;
    b.extrude_y(
        &[
            [porch.min.x - 0.4, eaves + 0.6],
            [porch.max.x + 0.4, eaves + 0.6],
            [0.0, peak],
        ],
        porch.min.y,
        porch.max.y + 0.4,
    );
    // The doors under the portico.
    for x in [-7.0, 0.0, 7.0] {
        paint(b, pat::SHADOW);
        panel(
            b,
            body,
            Side::Front,
            x - 1.8,
            x + 1.8,
            base,
            base + 6.0,
            0.04,
        );
    }
}

// ---- the station -------------------------------------------------------------------------

/// A railway terminus: a stone head house to the street (+y), three great arched
/// entrances under a canopy and a clock; behind it the train shed, a glass barrel
/// vault on steel ribs over brick side walls, its ends glazed down to the trains'
/// openings.
pub(super) fn station(b: &mut MeshBuilder, _tech: u8) {
    let (plan, top) = part(PropKind::CityStation, 0);
    let head = Rect::new(
        plan.min.x + 6.0,
        plan.max.y - 13.0,
        plan.max.x - 6.0,
        plan.max.y,
    );
    let shed = Rect::new(plan.min.x, plan.min.y, plan.max.x, head.min.y + 1.0);
    let spring = 9.0;
    let head_top = 17.0;
    if b.far() {
        solid(b, head, -2.0, head_top, pat::ARCHED, pat::ROOF_FLAT);
        solid(b, shed, -2.0, top - 4.0, pat::BRICK, pat::ROOF_GLASS);
        return;
    }
    let z0 = if b.mid() { 0.0 } else { -2.0 };
    // The shed's side wall and its ends below the springing.
    walls_on(b, shed, z0, spring, &[Side::Back], pat::ARCHED);
    // The vault: a thick arch across y, along x.
    let mid = shed.centre().y;
    let half = shed.size().y * 0.5;
    let n = if b.fine() {
        14
    } else if b.mid() {
        8
    } else {
        4
    };
    let arc = |r: f32, rise: f32| -> Vec<[f32; 2]> {
        (0..=n)
            .map(|i| {
                let a = PI * i as f32 / n as f32;
                [mid - r * a.cos(), spring + rise * a.sin()]
            })
            .collect()
    };
    let rise = top - spring;
    let mut profile = arc(half, rise);
    let mut inner = arc(half - 0.4, rise - 0.4);
    inner.reverse();
    profile.extend(inner);
    paint(b, pat::ROOF_GLASS);
    b.extrude_x(&profile, shed.min.x, shed.max.x);
    if !b.mid() {
        solid(b, head, z0, head_top, pat::ARCHED, pat::ROOF_FLAT);
        return;
    }
    // Glazed end screens down to the openings the trains run out of.
    for (x, out) in [(shed.min.x + 0.2, -1.0f32), (shed.max.x - 0.2, 1.0)] {
        let pts: Vec<Vec3> = arc(half - 0.4, rise - 0.4)
            .iter()
            .map(|p| v3(x, p[0], p[1]))
            .collect();
        paint(b, pat::ROOF_GLASS);
        // The arch's ends meet the springing; the screen runs down to 6 m.
        let mut ring = vec![v3(x, mid - half + 0.4, 6.0)];
        ring.extend(pts.iter().copied());
        ring.push(v3(x, mid + half - 0.4, 6.0));
        facing(b, ring, Vec3::X * out);
        paint(b, pat::SHADOW);
        facing(
            b,
            vec![
                v3(x, mid - half + 0.4, 0.0),
                v3(x, mid + half - 0.4, 0.0),
                v3(x, mid + half - 0.4, 6.0),
                v3(x, mid - half + 0.4, 6.0),
            ],
            Vec3::X * out,
        );
        let (min, max) = (v3(x - 0.3, mid - half, 5.6), v3(x + 0.3, mid + half, 6.2));
        boxed(b, min, max, pat::STEEL);
        // Brick piers at the ends of the side wall.
        boxed(
            b,
            v3(x - 1.0, shed.min.y - 0.3, -1.0),
            v3(x + 1.0, shed.min.y + 1.2, spring + 1.0),
            pat::BRICK,
        );
    }
    // The steel ribs over the glass.
    let ribs = if b.fine() { 15 } else { 7 };
    for k in 0..=ribs {
        let x = shed.min.x + 1.0 + (shed.size().x - 2.0) * k as f32 / ribs as f32;
        let mut rib = arc(half + 0.35, rise + 0.35);
        let mut under = arc(half - 0.05, rise - 0.05);
        under.reverse();
        rib.extend(under);
        paint(b, pat::STEEL);
        b.extrude_x(&rib, x - 0.2, x + 0.2);
    }
    // The head house.
    plinth(b, head, 0.6, pat::STONE);
    walls(b, head, 0.6, head_top - 1.0, pat::ARCHED);
    cornice(b, head, head_top - 0.6, 0.8, 0.6, pat::STONE);
    parapet(b, head, head_top - 0.6, 1.6, 0.3, pat::ARCHED + BLANK);
    deck(b, head.grow(-0.3), head_top - 0.5, pat::ROOF_FLAT);
    // The central pavilion with its clock and the three entrance arches.
    let pav = Rect::new(-12.0, head.min.y, 12.0, head.max.y);
    walls(b, pav, 0.6, head_top + 5.0, pat::ARCHED + BLANK);
    gable_roof(
        b,
        pav,
        head_top + 5.0,
        top - 1.0,
        0.4,
        true,
        pat::ROOF_TILE,
        pat::ARCHED + BLANK,
    );
    for x in [-7.0, 0.0, 7.0] {
        let (lo, hi) = (x - 2.6, x + 2.6);
        paint(b, pat::SHADOW);
        let y = pav.max.y + 0.04;
        let mut arch = vec![v3(lo, y, 0.6), v3(hi, y, 0.6)];
        for i in 0..=8 {
            let a = PI * i as f32 / 8.0;
            arch.push(v3(x + 2.6 * a.cos(), y, 8.0 + 2.6 * a.sin()));
        }
        facing(b, arch, Vec3::Y);
    }
    let clock = v3(0.0, pav.max.y + 0.1, head_top + 1.8);
    paint(b, pat::STONE);
    b.cylinder_between(clock, clock + Vec3::Y * 0.3, 2.4, 2.4, b.sides(16));
    paint(b, pat::RENDER);
    b.cylinder_between(
        clock + Vec3::Y * 0.3,
        clock + Vec3::Y * 0.4,
        2.0,
        2.0,
        b.sides(16),
    );
    if b.fine() {
        boxed(
            b,
            clock + v3(-0.08, 0.4, 0.0),
            clock + v3(0.08, 0.5, 1.5),
            pat::SHADOW,
        );
        boxed(
            b,
            clock + v3(0.0, 0.4, -0.08),
            clock + v3(1.1, 0.5, 0.08),
            pat::SHADOW,
        );
    }
    let (min, max) = Side::Front.block(pav, pav.min.x - 4.0, pav.max.x + 4.0, 0.0, 2.8, 11.6, 12.0);
    boxed(b, min, max, pat::STEEL);
    if b.fine() {
        for x in [pav.min.x - 3.6, pav.max.x + 3.6] {
            boxed(
                b,
                v3(x - 0.15, pav.max.y + 2.3, 0.0),
                v3(x + 0.15, pav.max.y + 2.6, 11.6),
                pat::STEEL,
            );
        }
        // Brick pilasters between the side wall's arches.
        for x in cells(shed.min.x, shed.size().x, pat::ARCHED_BAY * 2.0) {
            let (min, max) = Side::Back.block(shed, x - 0.5, x + 0.5, 0.0, 0.5, 0.0, spring + 0.6);
            boxed(b, min, max, pat::BRICK);
        }
    }
}

// ---- the church --------------------------------------------------------------------------

/// A stone church: a nave with a clerestory over two aisles under lean-to roofs, a
/// steep slate roof, an apse at its east end, buttresses; a west tower with the main
/// door, a belfry, corner pinnacles and an octagonal slate spire.
pub(super) fn church(b: &mut MeshBuilder, _tech: u8) {
    let (plan, top) = part(PropKind::CityChurch, 0);
    let (tower, spire_top) = part(PropKind::CityChurch, 1);
    let nave = Rect::new(tower.max.x, -7.0, plan.max.x - 7.0, 7.0);
    let aisle_h = 7.5;
    let eaves = 15.5;
    let belfry = (tower.size().x * 0.5, 34.0, 44.0);
    if b.far() {
        solid(
            b,
            Rect::new(nave.min.x, plan.min.y, nave.max.x, plan.max.y),
            -2.0,
            eaves + 3.0,
            pat::ARCHED,
            pat::ROOF_TILE,
        );
        solid(b, tower, -2.0, belfry.2, pat::STONE, pat::ROOF_FLAT);
        paint(b, pat::ROOF_TILE);
        b.prism(
            tower.centre().extend(belfry.2),
            4,
            belfry.0,
            0.2,
            spire_top - belfry.2,
        );
        return;
    }
    if b.coarse() {
        let body = Rect::new(nave.min.x, plan.min.y, nave.max.x + 4.0, plan.max.y);
        walls(b, body, -2.0, aisle_h, pat::ARCHED);
        gable_roof(
            b,
            body,
            aisle_h,
            top,
            0.0,
            false,
            pat::ROOF_TILE,
            pat::ARCHED + BLANK,
        );
        solid(b, tower, -2.0, belfry.2, pat::STONE, pat::ROOF_FLAT);
        paint(b, pat::ROOF_TILE);
        b.prism(
            tower.centre().extend(belfry.2),
            4,
            belfry.0,
            0.2,
            spire_top - belfry.2,
        );
        return;
    }
    let z0 = if b.mid() { 0.0 } else { -2.0 };
    let aisles = [
        Rect::new(nave.min.x, plan.min.y, nave.max.x, nave.min.y),
        Rect::new(nave.min.x, nave.max.y, nave.max.x, plan.max.y),
    ];
    // Nave: clerestory over the aisles.
    walls_on(
        b,
        nave,
        aisle_h,
        eaves,
        &[Side::Front, Side::Back],
        pat::ARCHED,
    );
    walls_on(b, nave, z0, eaves, &[Side::East], pat::ARCHED + BLANK);
    gable_roof(
        b,
        nave,
        eaves,
        top,
        if b.mid() { 0.5 } else { 0.0 },
        false,
        pat::ROOF_TILE,
        pat::ARCHED + BLANK,
    );
    for (k, a) in aisles.iter().enumerate() {
        let side = if k == 0 { Side::Back } else { Side::Front };
        walls_on(b, *a, z0, aisle_h, &[side], pat::ARCHED);
        walls_on(
            b,
            *a,
            z0,
            aisle_h,
            &[Side::East, Side::West],
            pat::ARCHED + BLANK,
        );
        // The lean-to roof up against the clerestory.
        let (outer, inner) = if k == 0 {
            (a.min.y - 0.4, a.max.y)
        } else {
            (a.max.y + 0.4, a.min.y)
        };
        paint(b, pat::ROOF_TILE);
        b.extrude_x(
            &[
                [outer, aisle_h - 0.25],
                [outer, aisle_h],
                [inner, aisle_h + 3.0],
                [inner, aisle_h + 2.75],
            ],
            a.min.x - 0.3,
            a.max.x + 0.3,
        );
    }
    // The apse: half an octagon on the east end.
    let apse_r = nave.size().y * 0.5;
    let apse_at = Vec2::new(nave.max.x, 0.0);
    let half_ring = |r: f32, z: f32| -> Vec<Vec3> {
        (0..=4)
            .map(|i| {
                let a = -FRAC_PI_2 + PI * i as f32 / 4.0;
                (apse_at + Vec2::from_angle(a) * r).extend(z)
            })
            .collect()
    };
    paint(b, pat::ARCHED);
    b.loft(
        &[half_ring(apse_r, z0), half_ring(apse_r, eaves)],
        false,
        false,
    );
    paint(b, pat::ROOF_TILE);
    let apex = apse_at.extend(top - 3.0);
    let rim = half_ring(apse_r + 0.4, eaves - 0.3);
    for k in 0..4 {
        let out = ((rim[k] + rim[k + 1]).truncate() * 0.5 - apse_at).extend(0.8);
        facing(b, vec![rim[k], rim[k + 1], apex], out);
    }
    // The tower.
    let tz = belfry.1;
    walls(b, tower, z0, tz, pat::ARCHED + BLANK);
    walls(b, tower, tz, belfry.2, pat::ARCHED);
    deck(b, tower, belfry.2, pat::ROOF_FLAT);
    paint(b, pat::ROOF_TILE);
    let spire_base = belfry.2 + 0.6;
    let oct: Vec<[f32; 2]> = crate::builder::ngon(8, tower.size().x * 0.44);
    b.at(tower.centre().extend(0.0), |b| {
        b.loft_z(
            &oct,
            &[
                crate::builder::Section::new(spire_base, 1.0),
                crate::builder::Section::new(spire_top, 0.02),
            ],
        )
    });
    if !b.mid() {
        return;
    }
    if b.mid() {
        plinth(
            b,
            Rect::new(nave.min.x, plan.min.y, nave.max.x, plan.max.y),
            0.5,
            pat::STONE,
        );
        plinth(b, tower, 0.5, pat::STONE);
    }
    // The west door: a pointed arch in the tower's foot, a window over it.
    let y0 = tower.centre().y;
    let x = tower.min.x - 0.05;
    let mut arch = vec![v3(x, y0 - 2.0, 0.5), v3(x, y0 + 2.0, 0.5)];
    for i in 0..=6 {
        let a = i as f32 / 6.0;
        let y = y0 + 2.0 - 4.0 * a;
        let z = 5.5 + 2.8 * (1.0 - (2.0 * a - 1.0).powi(2)).sqrt();
        arch.push(v3(x, y, z));
    }
    paint(b, pat::SHADOW);
    facing(b, arch, Vec3::NEG_X);
    let (min, max) = Side::West.block(tower, y0 - 3.0, y0 + 3.0, 0.0, 0.6, 0.0, 9.5);
    boxed(b, min, max, pat::STONE);
    cornice(b, tower, tz, 0.5, 0.3, pat::STONE);
    cornice(b, tower, belfry.2 + 0.6, 0.6, 0.4, pat::STONE);
    // Pinnacles at the tower's corners.
    for (sx, sy) in [(1.0f32, 1.0f32), (-1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)] {
        if !b.fine() {
            break;
        }
        let c = tower.centre() + Vec2::new(sx, sy) * (tower.size().x * 0.5 - 0.6);
        boxed(
            b,
            (c - Vec2::splat(0.6)).extend(belfry.2),
            (c + Vec2::splat(0.6)).extend(belfry.2 + 2.0),
            pat::STONE,
        );
        paint(b, pat::STONE);
        b.prism(c.extend(belfry.2 + 2.0), 4, 0.8, 0.05, 4.0);
    }
    // Buttresses between the aisles' windows, stepped.
    for (k, a) in aisles.iter().enumerate() {
        let side = if k == 0 { Side::Back } else { Side::Front };
        let (_, step) = grid(a.size().x, pat::ARCHED_BAY);
        for x in cells(a.min.x, a.size().x, pat::ARCHED_BAY).skip(1) {
            let x = x - step * 0.5;
            let (min, max) = side.block(*a, x - 0.5, x + 0.5, 0.0, 1.2, -0.5, 4.5);
            boxed(b, min, max, pat::STONE);
            if b.fine() {
                let (min, max) = side.block(*a, x - 0.45, x + 0.45, 0.0, 0.7, 4.5, aisle_h + 0.3);
                boxed(b, min, max, pat::STONE);
            }
        }
    }
    if b.fine() {
        // A cross on the spire and on the nave's east gable; the clock on the tower.
        let tip = tower.centre().extend(spire_top);
        boxed(
            b,
            tip + v3(-0.12, -0.12, 0.0),
            tip + v3(0.12, 0.12, 3.0),
            pat::STEEL,
        );
        boxed(
            b,
            tip + v3(-0.12, -0.9, 1.9),
            tip + v3(0.12, 0.9, 2.15),
            pat::STEEL,
        );
        let gable = v3(nave.min.x + 0.5, 0.0, top);
        boxed(
            b,
            gable + v3(-0.15, -0.15, 0.0),
            gable + v3(0.15, 0.15, 2.0),
            pat::STONE,
        );
        boxed(
            b,
            gable + v3(-0.15, -0.7, 1.2),
            gable + v3(0.15, 0.7, 1.5),
            pat::STONE,
        );
        for side in [Side::Front, Side::Back, Side::West] {
            let along = if side == Side::West {
                y0
            } else {
                tower.centre().x
            };
            let c = side.at(tower, along, 0.1, tz - 4.0);
            paint(b, pat::RENDER);
            b.cylinder_between(c, c + side.out() * 0.2, 1.6, 1.6, 12);
        }
    }
}
