//! The ARC fusion plants, each filling its lot, four-fold round its middle:
//! - tech 1, the cell: a solid armoured column on a round cradle, two segmented rings
//!   turning round it the opposite ways, arcs jumping from the column to the inner ring
//!   and across to the outer one (`Model::discharge`, renderer `reactor_fx.rs`);
//! - tech 2, the dome: a faceted, squared-off dome over the whole lot, windows in its
//!   facets onto the fusion inside (`pattern::FUSION`), buttresses on the diagonals, a
//!   ring turning round its crown;
//! - tech 3, the star: the heaviest foundation, heat sinks on splayed feet, their fins
//!   lifting in a wave (`part::REACTOR_FIN`), capacitor bastions; the star held over the
//!   body on four panels (`pattern::CORE`), arcs to their electrodes; four short blades
//!   and two tall ones on rings round the body's head, turning the opposite ways.
//!
//! The rings carry their blades (`part::REACTOR_COLLAR_FIRST`); what stands on the
//! plant (the column, the panels) stands on still decks. Power trunks leave along the
//! axes (`pattern::FLUX`). Authored at the blueprint's size, in metres, power leaving
//! toward +x.

use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, TAU};

use glam::{Vec2, Vec3};

use super::parts::*;
use crate::builder::{chamfered_rect, MeshBuilder, Section};
use crate::material::*;
use crate::{part, pattern};

/// Each tier's size: the blueprints' (radius, height), so the model is drawn at the
/// metres it is authored in.
pub(super) const SIZES: [(f32, f32); 3] = [(11.0, 10.0), (23.0, 26.0), (42.5, 47.0)];

/// Tech 3's plan, in metres; (out, up) pairs are measured from the z axis.
struct Plant {
    /// The plinth: half its width, its chamfer, its top; then each step up on it, half
    /// width and rise.
    plinth: (f32, f32, f32),
    steps: &'static [(f32, f32)],
    /// A diagonal foot: from and to (out), width at root and toe, height at root and toe.
    foot: (Vec2, Vec2, Vec2),
    fins: usize,
    /// An axial power trunk: how far out, its width, its height at root and toe.
    trunk: (f32, f32, Vec2),
    /// The body: half width at its foot and its head, its top, its chamfer.
    body: (Vec2, f32, f32),
    /// The still drum on the body's head the panels stand on: its radius and its top.
    drum: (f32, f32),
    /// The star's middle and its radius.
    core: (f32, f32),
    /// A suspension panel's path, (out, up), from the drum to its electrode, and its
    /// section.
    panel: [Vec2; 3],
    panel_size: Vec2,
    /// The blade sets, inside out: each on its own ring round the head.
    sets: [BladeSet; 2],
}

/// Blades on a ring turning round the plant's z axis.
struct BladeSet {
    /// The ring: inner and outer radius, foot and top, and the segments it is made of.
    ring: (f32, f32, f32, f32),
    segments: usize,
    /// A blade's outer face, (out, up), a Bezier from its foot to its tip; its depth;
    /// how many round the ring, and what share of the turn each spans.
    blade: [Vec2; 4],
    depth: f32,
    count: usize,
    span: f32,
}

const PLANT: Plant = Plant {
    plinth: (34.0, 9.0, 3.0),
    steps: &[(29.0, 3.0), (24.0, 3.0)],
    foot: (
        Vec2::new(24.0, 47.0),
        Vec2::new(15.0, 11.0),
        Vec2::new(12.0, 3.0),
    ),
    fins: 10,
    trunk: (40.0, 9.0, Vec2::new(8.0, 3.0)),
    body: (Vec2::new(18.0, 14.0), 23.0, 5.0),
    drum: (12.6, 24.4),
    core: (34.0, 4.6),
    panel: [
        Vec2::new(8.6, 24.4),
        Vec2::new(10.2, 28.8),
        Vec2::new(7.8, 32.2),
    ],
    panel_size: Vec2::new(2.8, 1.4),
    sets: [
        BladeSet {
            ring: (13.0, 14.8, 23.3, 24.7),
            segments: 8,
            blade: [
                Vec2::new(14.6, 24.7),
                Vec2::new(16.4, 30.0),
                Vec2::new(15.4, 35.4),
                Vec2::new(12.4, 37.8),
            ],
            depth: 1.5,
            count: 4,
            span: 0.36,
        },
        BladeSet {
            ring: (15.6, 17.6, 23.3, 24.5),
            segments: 10,
            blade: [
                Vec2::new(17.4, 24.5),
                Vec2::new(20.4, 32.5),
                Vec2::new(19.2, 41.6),
                Vec2::new(14.2, 46.2),
            ],
            depth: 1.8,
            count: 2,
            span: 0.26,
        },
    ],
};

pub(super) fn power(b: &mut MeshBuilder, tech: u8) {
    match tech {
        0 | 1 => cell(b),
        2 => dome(b),
        _ => plant(b, &PLANT),
    }
}

// ---- tech 1 ----------------------------------------------------------------------

/// Tech 1's deck and the cradle on it.
const CELL_DECK: f32 = 0.9;
const CELL_CRADLE: f32 = 1.8;
/// Its rings, inside out: inner and outer radius, foot and top, segments. They step
/// down outward, so the column and the arcs show over them.
const CELL_RINGS: [(f32, f32, f32, f32, usize); 2] =
    [(3.6, 4.3, 2.3, 6.4, 6), (6.6, 7.4, 2.3, 4.2, 8)];
/// Where the arcs leave the column: its middle at the electrodes, and how far out.
const CELL_CORE: Vec3 = Vec3::new(0.0, 0.0, 5.0);
const CELL_CORE_RADIUS: f32 = 2.2;

/// Tech 1, the cell: a solid column on a round cradle filling the lot, two segmented
/// rings round it turning the opposite ways, the charge arcing from the column to the
/// inner ring and across to the outer.
fn cell(b: &mut MeshBuilder) {
    b.set_spinner_pivot(CELL_CORE);
    if b.coarse() {
        b.paint(PLATING);
        b.block(v3(-11.0, -11.0, 0.0), v3(11.0, 11.0, CELL_DECK));
        b.paint(ACCENT);
        b.prism(v3(0.0, 0.0, CELL_DECK), 6, 8.2, 7.6, 6.3);
        b.paint(PLATING);
        b.prism(v3(0.0, 0.0, 7.2), 4, 2.4, 1.6, 2.6);
        team_panel(b, v3(0.0, 10.0, CELL_DECK), v2(3.6, 1.0));
        return;
    }
    // The plinth, out to the lot's edge.
    let plan = chamfered_rect(v2(11.3, 11.3), 2.6);
    b.paint(ACCENT);
    b.loft_z(
        &plan,
        &[Section::new(0.0, 1.02), Section::new(CELL_DECK * 0.4, 1.0)],
    );
    b.paint(PLATING);
    b.loft_z(
        &plan,
        &[
            Section::new(CELL_DECK * 0.4, 0.99),
            Section::new(CELL_DECK, 0.96),
        ],
    );
    // The round cradle the rings ride on, their tracks, and a lit line between them.
    let sides = round(b, 32);
    let circle = ngon_plan(sides, 1.0);
    b.paint(PLATING);
    b.loft_z(
        &circle,
        &[
            Section::new(CELL_DECK, 9.3),
            Section::new(CELL_CRADLE - 0.2, 9.0),
        ],
    );
    b.paint(ACCENT);
    b.loft_z(
        &circle,
        &[
            Section::new(CELL_CRADLE - 0.2, 9.0),
            Section::new(CELL_CRADLE, 8.8),
        ],
    );
    for &(r0, r1, ..) in &CELL_RINGS {
        annulus(
            b,
            sides,
            r0 - 0.2,
            r1 + 0.2,
            CELL_CRADLE,
            CELL_CRADLE + 0.25,
        );
    }
    column(b);
    // The rings, the inner one first: neighbours turn the opposite ways.
    for (k, &(r0, r1, z0, z1, segments)) in CELL_RINGS.iter().enumerate() {
        b.with_part(part::REACTOR_COLLAR_FIRST + k as u32, |b| {
            ring(b, (r0, r1, z0, z1), segments)
        });
    }
    // Arcs from the column's electrodes to the inner ring, and across to the outer one.
    let at = |r: f32, a: f32, z: f32| v3(r * a.cos(), r * a.sin(), z);
    let (inner, outer) = (CELL_RINGS[0], CELL_RINGS[1]);
    let terminals: Vec<Vec3> = (0..12)
        .map(|i| {
            at(
                inner.0,
                TAU / 12.0 * i as f32,
                (inner.2 + inner.3) * 0.5 + 0.6,
            )
        })
        .collect();
    let bridges: Vec<(Vec3, Vec3)> = (0..8)
        .map(|i| {
            let a = TAU / 8.0 * (i as f32 + 0.3);
            (
                at(inner.1, a, inner.3 - 0.8),
                at(outer.0, a + 0.15, outer.3 - 0.3),
            )
        })
        .collect();
    b.set_discharge(CELL_CORE, CELL_CORE_RADIUS, &terminals, &bridges);
    // A trunk out along x each way, the power pulsing out along it.
    b.radial(2, |b| {
        b.paint(ACCENT);
        wedge(b, 9.0, 11.4, v2(3.2, 2.8), v2(1.7, 1.1));
        b.paint(ACCENT).pattern(pattern::FLUX);
        b.beam(
            v3(9.3, 0.0, 1.62),
            v3(11.1, 0.0, 1.12),
            v2(0.5, 0.12),
            v2(0.5, 0.12),
        );
    });
    // Low armoured housings on the corners, team colour along the sides.
    for x in [-8.6, 8.6] {
        for y in [-8.6, 8.6] {
            let at = v3(x, y, CELL_DECK + 0.7);
            b.paint(ACCENT);
            if b.fine() {
                b.chamfered_box(at, v3(3.2, 3.2, 1.4), 0.5);
                glow_strip(b, at + Vec3::Z * 0.7, v2(2.0, 0.3), GLOW);
            } else {
                b.cuboid(at, v3(3.2, 3.2, 1.4));
            }
        }
    }
    b.mirror_y(|b| team_panel(b, v3(0.0, 10.2, CELL_DECK), v2(3.6, 1.0)));
}

/// Tech 1's solid middle: an armoured column off a collar on the cradle, banded with
/// charge, electrodes round its waist where the arcs leave it, a capped head.
fn column(b: &mut MeshBuilder) {
    let sides = round(b, 12);
    let plan = ngon_plan(sides, 1.0);
    b.paint(ACCENT);
    b.loft_z(
        &plan,
        &[
            Section::new(CELL_CRADLE, 2.5),
            Section::new(CELL_CRADLE + 0.8, 2.3),
        ],
    );
    b.paint(PLATING);
    b.loft_z(
        &plan,
        &[Section::new(CELL_CRADLE + 0.8, 1.8), Section::new(8.4, 1.6)],
    );
    b.paint(ACCENT);
    b.loft_z(&plan, &[Section::new(8.4, 1.9), Section::new(9.3, 1.3)]);
    b.paint(METAL);
    b.prism(v3(0.0, 0.0, 9.3), round(b, 8), 0.5, 0.15, 0.7);
    b.paint(GLOW).pattern(pattern::CHARGE);
    for z in [3.7, 6.5] {
        annulus(b, sides, 1.5, 1.86, z, z + 0.25);
    }
    if !b.fine() {
        return;
    }
    b.radial(8, |b| {
        let z = CELL_CORE.z;
        b.paint(METAL);
        b.cylinder_between(v3(1.6, 0.0, z), v3(2.05, 0.0, z), 0.2, 0.12, 6);
        b.paint(GLOW);
        b.spheroid(v3(2.1, 0.0, z), Vec3::splat(0.14), 6, 2);
    });
}

// ---- tech 2 ----------------------------------------------------------------------

const DOME_DECK: f32 = 2.0;
/// The dome's plan (a squared-off octagon, scaled by its sections) and its sections,
/// wall to crown: (up, scale).
const DOME_CHAMFER: f32 = 0.5;
const DOME: [(f32, f32); 5] = [
    (3.0, 19.6),
    (8.0, 19.2),
    (13.5, 16.2),
    (18.5, 11.6),
    (21.5, 7.0),
];
/// The crown: its collar's top, the neck's top, and the ring turning round it.
const CROWN: f32 = 22.3;
const NECK: f32 = 24.4;
const CROWN_RING: (f32, f32, f32, f32) = (5.0, 6.4, 22.6, 23.6);

/// Tech 2, the dome: a faceted, squared-off dome over the whole lot, windows in its
/// sloped facets onto the fusion inside, buttresses on the diagonals, power trunks out
/// along the axes, a ring turning round its crown and a glass oculus on top.
fn dome(b: &mut MeshBuilder) {
    b.set_spinner_pivot(v3(0.0, 0.0, CROWN));
    if b.coarse() {
        b.paint(PLATING);
        b.block(v3(-23.0, -23.0, 0.0), v3(23.0, 23.0, DOME_DECK));
        b.paint(ACCENT);
        b.prism(v3(0.0, 0.0, DOME_DECK), 8, 21.0, 7.4, CROWN - DOME_DECK);
        team_panel(b, v3(0.0, 21.6, DOME_DECK), v2(7.0, 1.6));
        return;
    }
    let plan = chamfered_rect(v2(23.0, 23.0), 5.0);
    b.paint(ACCENT);
    b.loft_z(
        &plan,
        &[Section::new(0.0, 1.02), Section::new(DOME_DECK * 0.4, 1.0)],
    );
    b.paint(PLATING);
    b.loft_z(
        &plan,
        &[
            Section::new(DOME_DECK * 0.4, 0.99),
            Section::new(DOME_DECK, 0.97),
        ],
    );
    // The dome: a dark footing, the wall and the facets, a dark collar at the crown.
    let unit = chamfered_rect(v2(1.0, 1.0), DOME_CHAMFER);
    b.paint(ACCENT);
    b.loft_z(
        &unit,
        &[Section::new(DOME_DECK, 20.0), Section::new(DOME[0].0, 19.8)],
    );
    b.paint(PLATING);
    let sections: Vec<Section> = DOME.iter().map(|&(z, s)| Section::new(z, s)).collect();
    b.loft_z(&unit, &sections);
    b.paint(ACCENT);
    let top = DOME[DOME.len() - 1];
    b.loft_z(
        &unit,
        &[
            Section::new(top.0, top.1 * 1.03),
            Section::new(CROWN, top.1 * 0.97),
        ],
    );
    // Windows in the two sloped bands, every facet.
    for band in [1, 2] {
        for i in 0..unit.len() {
            window(b, &unit, i, DOME[band], DOME[band + 1]);
        }
    }
    // The crown: the neck, the oculus on it, the ring turning round it.
    let sides = round(b, 16);
    let circle = ngon_plan(round(b, 8), 1.0);
    b.paint(ACCENT);
    b.loft_z(
        &circle,
        &[Section::new(CROWN, 4.8), Section::new(NECK, 4.3)],
    );
    b.paint(GLOW).pattern(pattern::FUSION);
    b.loft_z(
        &circle,
        &[Section::new(NECK, 3.4), Section::new(NECK + 0.3, 3.0)],
    );
    b.paint(GLOW).pattern(pattern::CHARGE);
    annulus(b, sides, 4.2, 4.45, NECK - 0.6, NECK - 0.35);
    b.with_part(part::REACTOR_COLLAR_FIRST, |b| ring(b, CROWN_RING, 6));
    // Buttresses on the diagonals, from a footing at the plinth's corner up to the wall.
    b.yawed(Vec3::ZERO, FRAC_PI_4, |b| {
        b.radial(4, |b| {
            b.paint(ACCENT);
            b.chamfered_box(v3(26.0, 0.0, DOME_DECK + 0.9), v3(4.0, 4.6, 1.8), 0.6);
            b.beam(
                v3(25.8, 0.0, DOME_DECK + 1.6),
                v3(20.6, 0.0, DOME[1].0 + 0.4),
                v2(3.2, 1.8),
                v2(2.4, 1.0),
            );
            if b.fine() {
                glow_strip(b, v3(26.0, 0.0, DOME_DECK + 1.8), v2(2.4, 0.4), GLOW);
            }
        })
    });
    // Power trunks out along the axes, team colour beside them.
    b.radial(4, |b| {
        b.paint(ACCENT);
        wedge(b, 19.0, 23.2, v2(5.0, 4.4), v2(3.6, 2.6));
        b.paint(ACCENT).pattern(pattern::FLUX);
        b.beam(
            v3(19.6, 0.0, 3.62),
            v3(22.8, 0.0, 2.68),
            v2(1.6, 0.14),
            v2(1.6, 0.14),
        );
        b.mirror_y(|b| team_panel(b, v3(21.6, 9.0, DOME_DECK), v2(1.6, 6.0)));
    });
}

/// A window in facet `i` of `plan` between two of the dome's sections: a pane onto the
/// fusion, standing just off the facet, split by dark mullions.
fn window(b: &mut MeshBuilder, plan: &[[f32; 2]], i: usize, lo: (f32, f32), hi: (f32, f32)) {
    let corner = |k: usize, (z, s): (f32, f32)| {
        let q = plan[k % plan.len()];
        v3(q[0] * s, q[1] * s, z)
    };
    let (p00, p10) = (corner(i, lo), corner(i + 1, lo));
    let (p01, p11) = (corner(i, hi), corner(i + 1, hi));
    let mut out = (p10 - p00).cross(p01 - p00).normalize();
    let mid = (p00 + p11) * 0.5;
    if out.dot(v3(mid.x, mid.y, 0.0)) < 0.0 {
        out = -out;
    }
    let at = |u: f32, v: f32, off: f32| {
        let lo = p00.lerp(p10, u);
        let hi = p01.lerp(p11, u);
        lo.lerp(hi, v) + out * off
    };
    let (u0, u1, v0, v1) = (0.1, 0.9, 0.18, 0.82);
    // Panes about four metres across, a mullion between each two; one pane further off.
    let width = p00.distance(p10) * (u1 - u0);
    let panes = if b.fine() {
        (width / 4.0).round().max(1.0) as usize
    } else {
        1
    };
    let du = (u1 - u0) / panes as f32;
    let gap = 0.5 / p00.distance(p10);
    b.paint(GLOW).pattern(pattern::FUSION);
    for k in 0..panes {
        let (a, c) = (u0 + du * k as f32 + gap, u0 + du * (k + 1) as f32 - gap);
        let pane = |off: f32| {
            vec![
                at(a, v0, off),
                at(c, v0, off),
                at(c, v1, off),
                at(a, v1, off),
            ]
        };
        b.loft(&[pane(0.02), pane(0.1)], true, true);
    }
    b.paint(ACCENT);
    for k in 1..panes {
        let u = u0 + du * k as f32;
        b.beam(at(u, v0, 0.1), at(u, v1, 0.1), v2(0.5, 0.3), v2(0.5, 0.3));
    }
    if b.fine() {
        b.beam(at(u0, v0, 0.1), at(u1, v0, 0.1), v2(0.6, 0.3), v2(0.6, 0.3));
    }
}

// ---- tech 3 ----------------------------------------------------------------------

fn plant(b: &mut MeshBuilder, p: &Plant) {
    let core = v3(0.0, 0.0, p.core.0);
    b.set_spinner_pivot(core);
    if b.coarse() {
        let (half, _, deck) = p.plinth;
        let outer = &p.sets[1];
        b.paint(PLATING);
        b.prism(Vec3::ZERO, 4, half * 1.3, half * 1.2, deck);
        b.paint(ACCENT);
        b.prism(
            v3(0.0, 0.0, deck),
            4,
            p.body.0.x * 1.3,
            p.body.0.y * 1.2,
            p.body.1 - deck,
        );
        b.paint(PLATING);
        b.prism(
            v3(0.0, 0.0, outer.ring.2),
            4,
            outer.blade[1].x * 1.2,
            outer.blade[3].x * 1.1,
            outer.blade[3].y - outer.ring.2,
        );
        team_panel(b, v3(-half * 0.6, 0.0, deck), v2(half * 0.3, half * 0.5));
        return;
    }
    foundation(b, p);
    body(b, p);
    // The still drum on the head the panels stand on, a lit band round its top.
    let sides = round(b, 24);
    let (r, top) = p.drum;
    b.paint(ACCENT);
    b.loft_z(
        &ngon_plan(sides, 1.0),
        &[Section::new(p.body.1, r), Section::new(top, r * 0.97)],
    );
    b.paint(GLOW).pattern(pattern::CHARGE);
    annulus(b, sides, r * 0.9, r * 0.93, top, top + r * 0.004);
    // The star, and the four panels that hold it up, on the diagonals.
    b.paint(GLOW).pattern(pattern::CORE);
    let (sides, bands) = if b.fine() { (16, 8) } else { (12, 6) };
    b.spheroid(core, Vec3::splat(p.core.1), sides, bands);
    b.yawed(Vec3::ZERO, FRAC_PI_4, |b| {
        b.radial(4, |b| panel(b, p, core))
    });
    let tip = p.panel[2];
    let tips: Vec<Vec3> = (0..4)
        .map(|i| {
            let a = FRAC_PI_4 + FRAC_PI_2 * i as f32;
            v3(tip.x * a.cos(), tip.x * a.sin(), tip.y)
        })
        .collect();
    b.set_discharge(core, p.core.1, &tips, &[]);
    // The blade sets on their rings, the inner one first: they turn the opposite ways.
    for (k, set) in p.sets.iter().enumerate() {
        b.with_part(part::REACTOR_COLLAR_FIRST + k as u32, |b| {
            ring(b, set.ring, set.segments);
            let span = TAU / set.count as f32 * set.span;
            b.radial(set.count, |b| blade(b, set.blade, set.depth, span));
        });
    }
}

/// The plinth, its steps, the four diagonal feet with their radiators, and the four
/// power trunks.
fn foundation(b: &mut MeshBuilder, p: &Plant) {
    let (half, chamfer, top) = p.plinth;
    let plan = chamfered_rect(v2(half, half), chamfer);
    b.paint(ACCENT);
    b.loft_z(
        &plan,
        &[Section::new(0.0, 1.04), Section::new(top * 0.35, 1.0)],
    );
    b.paint(PLATING);
    b.loft_z(
        &plan,
        &[Section::new(top * 0.35, 0.99), Section::new(top, 0.94)],
    );
    let mut floor = top;
    for &(h, up) in p.steps {
        let plan = chamfered_rect(v2(h, h), chamfer * h / half);
        b.paint(ACCENT);
        b.loft_z(
            &plan,
            &[
                Section::new(floor, 1.0),
                Section::new(floor + up * 0.25, 0.99),
            ],
        );
        b.paint(PLATING);
        b.loft_z(
            &plan,
            &[
                Section::new(floor + up * 0.25, 0.98),
                Section::new(floor + up, 0.94),
            ],
        );
        floor += up;
    }
    b.yawed(Vec3::ZERO, FRAC_PI_4, |b| b.radial(4, |b| foot(b, p)));
    b.radial(4, |b| trunk(b, p));
    b.mirror_y(|b| {
        team_panel(
            b,
            v3(-half * 0.42, half * 0.66, top),
            v2(half * 0.3, half * 0.14),
        )
    });
}

/// A foot on +x: a wedge splaying down to its toe with bevelled shoulders, a dark sole
/// at the toe, a radiator bank laid along its back, a capacitor bastion on its toe.
fn foot(b: &mut MeshBuilder, p: &Plant) {
    let (out, width, height) = p.foot;
    b.paint(PLATING);
    wedge(b, out.x, out.y, width, height);
    b.paint(ACCENT);
    let heel = out.y - (out.y - out.x) * 0.16;
    wedge(
        b,
        heel,
        out.y + width.y * 0.06,
        Vec2::splat(width.y * 1.08),
        Vec2::splat(height.y * 1.3),
    );
    // The heat sink, laid along the slope.
    let (x0, x1) = (
        out.x + (out.y - out.x) * 0.2,
        out.y - (out.y - out.x) * 0.24,
    );
    let slope = (height.x - height.y) / (out.y - out.x);
    let z0 = height.x - slope * (x0 - out.x);
    let len = (x1 - x0) * (1.0 + slope * slope).sqrt();
    let w = width.x - (width.x - width.y) * 0.3;
    b.pitched(v3(x0, 0.0, z0), -slope.atan(), |b| {
        heatsink(b, len, w * 0.86, p.fins)
    });
    // A capacitor bastion on the toe.
    let at = v3(heel + (out.y - heel) * 0.35, 0.0, height.y * 1.3);
    b.paint(ACCENT);
    b.chamfered_box(
        at + Vec3::Z * width.y * 0.3,
        v3(width.y * 0.7, width.y * 0.95, width.y * 0.6),
        width.y * 0.08,
    );
    for y in [-0.28, 0.0, 0.28] {
        capacitor(
            b,
            at + v3(0.0, y * width.y, width.y * 0.6),
            width.y * 0.12,
            width.y * 0.36,
        );
    }
}

/// A power trunk on +x: a low dark wedge from under the body out along the axis, a
/// conduit pulsing out along it, a row of capacitors down each side.
fn trunk(b: &mut MeshBuilder, p: &Plant) {
    let (to, width, height) = p.trunk;
    let from = p.plinth.0 * 0.6;
    b.paint(ACCENT);
    wedge(b, from, to, v2(width, width * 0.85), height);
    let slope = (height.x - height.y) / (to - from);
    let (a, z) = (from + width * 0.3, height.x - slope * width * 0.3);
    let (bb, zb) = (to - width * 0.4, height.y + slope * width * 0.4);
    b.paint(ACCENT).pattern(pattern::FLUX);
    b.beam(
        v3(a, 0.0, z + width * 0.04),
        v3(bb, 0.0, zb + width * 0.04),
        v2(width * 0.35, width * 0.08),
        v2(width * 0.35, width * 0.08),
    );
    b.mirror_y(|b| {
        for k in 0..3 {
            let x = from + (to - from) * (0.35 + 0.17 * k as f32);
            capacitor(b, v3(x, width * 0.9, p.plinth.2), width * 0.2, width * 0.42);
        }
    });
}

/// The tapered body off the foundation, a dark cornice, a buttress up each face with a
/// conduit down it.
fn body(b: &mut MeshBuilder, p: &Plant) {
    let (half, top, chamfer) = p.body;
    let floor = p.plinth.2 + p.steps.iter().map(|s| s.1).sum::<f32>();
    let plan = chamfered_rect(v2(half.x, half.x), chamfer);
    let lip = (top - floor) * 0.1;
    let head = half.y / half.x;
    b.paint(PLATING);
    b.loft_z(
        &plan,
        &[
            Section::new(floor, 1.0),
            Section::new(floor + (top - floor) * 0.3, 0.97),
            Section::new(top - lip, head),
        ],
    );
    b.paint(ACCENT);
    b.loft_z(
        &plan,
        &[
            Section::new(top - lip, head * 1.04),
            Section::new(top, head * 0.99),
        ],
    );
    b.radial(4, |b| {
        let (x0, x1) = (half.x, half.y);
        b.paint(ACCENT);
        b.beam(
            v3(x0 + half.x * 0.12, 0.0, floor),
            v3(x1 * 1.02, 0.0, top - lip),
            v2(half.x * 0.42, half.x * 0.24),
            v2(half.x * 0.3, half.x * 0.12),
        );
        if !b.fine() {
            return;
        }
        b.paint(ACCENT).pattern(pattern::FLUX);
        b.beam(
            v3(x1 * 1.02 + half.x * 0.08, 0.0, top - lip * 1.2),
            v3(x0 + half.x * 0.22, 0.0, floor + half.x * 0.05),
            v2(half.x * 0.14, half.x * 0.06),
            v2(half.x * 0.14, half.x * 0.06),
        );
    });
}

/// A suspension panel on +x: a thick plate rising off the still drum, leaning out and
/// back in at the star, a collar round its knee, an electrode at its tip.
fn panel(b: &mut MeshBuilder, p: &Plant, core: Vec3) {
    let path: Vec<Vec3> = p.panel.iter().map(|q| v3(q.x, 0.0, q.y)).collect();
    b.paint(PLATING);
    sweep(b, &path, p.panel_size, false);
    if b.fine() {
        b.paint(ACCENT);
        let knee = path[1];
        b.beam(
            knee - Vec3::Z * p.panel_size.y,
            knee + Vec3::Z * p.panel_size.y,
            p.panel_size * v2(1.15, 1.6),
            p.panel_size * v2(1.15, 1.6),
        );
    }
    let tip = path[2];
    electrode(b, tip, (core - tip).normalize(), p.panel_size.y * 0.9);
}

/// A blade on +x: a curved armoured shell standing on its ring, `span` of the turn
/// across, leaning in over the star; its inner face lit, a dark spine down its back.
fn blade(b: &mut MeshBuilder, curve: [Vec2; 4], depth: f32, span: f32) {
    // Samples up the face: six close to, three at the middle distance.
    let n = if b.fine() { 6 } else { 3 };
    let steps = if b.fine() { 6 } else { 3 };
    // Inward normal of the outer face at sample i (the face runs up, the inside is left).
    let inward = |i: usize| {
        let t = bezier_dir(curve, i as f32 / n as f32);
        v2(-t.y, t.x)
    };
    let outer: Vec<Vec2> = (0..=n)
        .map(|i| bezier(curve, i as f32 / n as f32))
        .collect();
    let inner: Vec<Vec2> = (0..=n).map(|i| outer[i] + inward(i) * depth).collect();
    // Counter-clockwise in (out, up): up the outer face, back down the inner.
    let mut profile = outer.clone();
    profile.extend(inner.iter().rev());
    b.paint(PLATING);
    shell(b, &profile, -span * 0.5, span * 0.5, steps, false);
    // Its ends, a quad at a time: the crescent is not convex, and a fan across it would
    // close it with a sheet.
    for (a, front) in [(-span * 0.5, false), (span * 0.5, true)] {
        let (c, sn) = (a.cos(), a.sin());
        let at = |q: Vec2| v3(q.x * c, q.x * sn, q.y);
        for i in 0..n {
            let mut quad = [
                at(outer[i]),
                at(outer[i + 1]),
                at(inner[i + 1]),
                at(inner[i]),
            ];
            if front {
                quad.reverse();
            }
            b.face(&quad);
        }
    }
    // A seam of light across the inner face, standing just off it.
    b.paint(GLOW);
    let seam = |t: f32, off: f32| {
        let n = bezier_dir(curve, t);
        bezier(curve, t) + v2(-n.y, n.x) * depth * off
    };
    let strip = [
        seam(0.48, 1.03),
        seam(0.52, 1.03),
        seam(0.52, 1.1),
        seam(0.48, 1.1),
    ];
    shell(b, &strip, -span * 0.42, span * 0.42, steps, true);
    // The spine.
    if b.fine() {
        b.paint(ACCENT);
        let proud: Vec<Vec2> = (0..=n)
            .map(|i| outer[i] - inward(i) * depth * 0.3)
            .collect();
        let mut spine = proud.clone();
        spine.extend(outer.iter().rev());
        shell(b, &spine, -span * 0.07, span * 0.07, 1, true);
    }
}

// ---- the kit ---------------------------------------------------------------------

/// A heat sink in its own frame, `len` along x from the origin, `w` across, standing up
/// off z = 0: a dark housing, the hot core glowing between the fin roots with its heat
/// rippling along it (`pattern::HEAT`), coolant manifolds down both sides, fins tall and
/// short in turn under a clamp bar, end caps; hot air shimmering off it and now and then a
/// wisp of steam (its `Exhaust`, renderer `heat_haze.rs`, `reactor_fx.rs`).
fn heatsink(b: &mut MeshBuilder, len: f32, w: f32, fins: usize) {
    let base = w * 0.16;
    let fin = w * 0.46;
    b.paint(ACCENT);
    b.chamfered_box(v3(len * 0.5, 0.0, base * 0.5), v3(len, w, base), base * 0.3);
    // The hot core, up between the fins to half their height: seen through the gaps.
    b.paint(GLOW_ORANGE).pattern(pattern::HEAT);
    b.cuboid(
        v3(len * 0.5, 0.0, base + fin * 0.24),
        v3(len * 0.86, w * 0.6, fin * 0.48),
    );
    b.paint(METAL);
    for y in [-1.0f32, 1.0] {
        if !b.fine() {
            break;
        }
        let (yy, z) = (y * w * 0.44, base + w * 0.06);
        b.cylinder_between(
            v3(len * 0.03, yy, z),
            v3(len * 0.97, yy, z),
            w * 0.055,
            w * 0.055,
            round(b, 6),
        );
    }
    // The fins lift in a wave while the plant runs: they reach down into the housing
    // further than they rise, so no gap opens under them.
    let n = if b.fine() { fins } else { fins.div_ceil(2) };
    let pitch = len * 0.86 / n as f32;
    let root = base * 0.45;
    b.with_part(part::REACTOR_FIN, |b| {
        b.paint(PLATING);
        for k in 0..n {
            let x = len * 0.07 + pitch * (k as f32 + 0.5);
            let tall = if k % 2 == 0 { 1.0 } else { 0.76 };
            b.cuboid(
                v3(x, 0.0, base - root + (fin * tall + root) * 0.5),
                v3(pitch * 0.32, w * 0.8, fin * tall + root),
            );
        }
    });
    if b.fine() {
        b.paint(ACCENT);
        b.cuboid(
            v3(len * 0.5, 0.0, base + fin + w * 0.02),
            v3(len * 0.88, w * 0.12, w * 0.05),
        );
        for x in [len * 0.035, len * 0.965] {
            b.chamfered_box(
                v3(x, 0.0, base + fin * 0.55),
                v3(len * 0.07, w * 0.94, fin * 1.1),
                w * 0.03,
            );
        }
    }
    b.add_exhaust(v3(len * 0.5, 0.0, base + fin), Vec3::Z, w * 0.4);
}

/// A ring round the z axis, `(inner, outer, foot, top)`, in `segments` with narrow gaps
/// between them so its turning shows: a dark clamp on each segment's outer face, a lit
/// band of charge round its inner face.
fn ring(b: &mut MeshBuilder, (r0, r1, z0, z1): (f32, f32, f32, f32), segments: usize) {
    let each = TAU / segments as f32;
    let gap = (0.35 / r1).min(each * 0.2);
    let steps = (round(b, 32) / segments).max(1);
    let section = [v2(r0, z0), v2(r1, z0), v2(r1, z1), v2(r0, z1)];
    let (h, mid) = (z1 - z0, (z0 + z1) * 0.5);
    for k in 0..segments {
        let a = each * k as f32;
        b.paint(PLATING);
        shell(
            b,
            &section,
            a + gap * 0.5,
            a + each - gap * 0.5,
            steps,
            true,
        );
        if b.fine() {
            b.paint(ACCENT);
            b.yawed(Vec3::ZERO, a + each * 0.5, |b| {
                b.cuboid(
                    v3(r1 + 0.12, 0.0, mid),
                    v3(0.3, (r1 * each * 0.22).min(2.4), h * 0.7),
                )
            });
        }
        b.paint(GLOW).pattern(pattern::CHARGE);
        let band = [
            v2(r0 - 0.04, mid - h * 0.12),
            v2(r0, mid - h * 0.12),
            v2(r0, mid + h * 0.12),
            v2(r0 - 0.04, mid + h * 0.12),
        ];
        shell(b, &band, a + gap, a + each - gap, steps, false);
    }
}

/// A wedge on +x from `x0` to `x1` standing on the ground, `width` across and `height`
/// tall at each end, its top shoulders bevelled.
fn wedge(b: &mut MeshBuilder, x0: f32, x1: f32, width: Vec2, height: Vec2) {
    let ring = |x: f32, w: f32, h: f32| -> Vec<Vec3> {
        let (s, e) = (w * 0.5, w * 0.16);
        vec![
            v3(x, -s, 0.0),
            v3(x, s, 0.0),
            v3(x, s, h * 0.78),
            v3(x, s - e, h),
            v3(x, -s + e, h),
            v3(x, -s, h * 0.78),
        ]
    };
    b.loft(
        &[ring(x0, width.x, height.x), ring(x1, width.y, height.y)],
        true,
        true,
    );
}

/// A solid of `profile` (out, up; counter-clockwise) turned about the z axis from angle
/// `a0` to `a1` in `steps`, capped at both ends if `caps` (a fan: convex profiles only).
fn shell(b: &mut MeshBuilder, profile: &[Vec2], a0: f32, a1: f32, steps: usize, caps: bool) {
    let rings: Vec<Vec<Vec3>> = (0..=steps)
        .map(|i| {
            let a = a0 + (a1 - a0) * i as f32 / steps as f32;
            let (c, s) = (a.cos(), a.sin());
            profile.iter().map(|q| v3(q.x * c, q.x * s, q.y)).collect()
        })
        .collect();
    b.loft(&rings, caps, caps);
}

/// An electrode at `at`: a metal point aimed along `toward` with a lit ball at its end.
fn electrode(b: &mut MeshBuilder, at: Vec3, toward: Vec3, s: f32) {
    b.paint(METAL);
    b.cylinder_between(at - toward * s * 1.6, at, s * 0.5, s * 0.22, round(b, 8));
    b.paint(GLOW);
    b.spheroid(
        at,
        Vec3::splat(s * 0.42),
        round(b, 8),
        if b.fine() { 4 } else { 2 },
    );
}

/// Sides for a round shape with `n` at full detail. The plants are big and many: at the
/// middle distance they drop to half, not the usual three quarters.
fn round(b: &MeshBuilder, n: usize) -> usize {
    if b.fine() {
        n
    } else if b.mid() {
        (n / 2).max(6)
    } else {
        4
    }
}

/// Unit n-gon plan, for lofts scaled by their sections.
fn ngon_plan(sides: usize, radius: f32) -> Vec<[f32; 2]> {
    crate::builder::ngon(sides, radius)
}

/// A flat ring round the z axis, `r0` to `r1` out and `z0` to `z1` up.
fn annulus(b: &mut MeshBuilder, sides: usize, r0: f32, r1: f32, z0: f32, z1: f32) {
    let section = [(r0, z0), (r1, z0), (r1, z1), (r0, z1)];
    let rings: Vec<Vec<Vec3>> = (0..=sides)
        .map(|i| {
            let a = i as f32 * TAU / sides as f32;
            let (c, s) = (a.cos(), a.sin());
            section.iter().map(|&(r, z)| v3(c * r, s * r, z)).collect()
        })
        .collect();
    b.loft(&rings, false, false);
}

/// A bar of rectangular section (`size`: across in y, deep in the path's plane) swept
/// along `path`, closed back on itself if `closed`.
fn sweep(b: &mut MeshBuilder, path: &[Vec3], size: Vec2, closed: bool) {
    let n = path.len();
    let at = |i: usize| path[(i + n) % n];
    let rings: Vec<Vec<Vec3>> = (0..n + usize::from(closed))
        .map(|k| {
            let i = k % n;
            let (prev, next) = if closed {
                (at(i + n - 1), at(i + 1))
            } else {
                (path[i.saturating_sub(1)], path[(i + 1).min(n - 1)])
            };
            let t = (next - prev).normalize_or(Vec3::Z);
            let side = Vec3::Y;
            let up = side.cross(t).normalize_or(Vec3::X);
            let (s, u) = (side * size.x * 0.5, up * size.y * 0.5);
            let p = path[i];
            vec![p - s - u, p + s - u, p + s + u, p - s + u]
        })
        .collect();
    b.loft(&rings, !closed, !closed);
}

/// A capacitor can: a dark drum with a lit crown and a terminal.
fn capacitor(b: &mut MeshBuilder, base: Vec3, r: f32, h: f32) {
    b.paint(ACCENT);
    b.prism(base, round(b, 6), r, r, h);
    b.paint(GLOW);
    if b.fine() {
        b.prism(base + Vec3::Z * h, 6, r * 0.85, r * 0.6, r * 0.3);
    } else {
        // High enough off the lid to hold apart at the middle distance.
        b.decal(base + Vec3::Z * (h + 0.08), Vec2::splat(r * 1.2));
    }
    if b.fine() {
        b.paint(METAL);
        b.prism(
            base + Vec3::Z * (h + r * 0.3),
            6,
            r * 0.2,
            r * 0.15,
            r * 0.5,
        );
    }
}

fn bezier(p: [Vec2; 4], t: f32) -> Vec2 {
    let u = 1.0 - t;
    p[0] * (u * u * u) + p[1] * (3.0 * u * u * t) + p[2] * (3.0 * u * t * t) + p[3] * (t * t * t)
}

fn bezier_dir(p: [Vec2; 4], t: f32) -> Vec2 {
    let u = 1.0 - t;
    ((p[1] - p[0]) * (3.0 * u * u) + (p[2] - p[1]) * (6.0 * u * t) + (p[3] - p[2]) * (3.0 * t * t))
        .normalize_or(Vec2::Y)
}

#[cfg(test)]
mod tests {
    use super::SIZES;
    use crate::{build_model_scaled, part};

    #[test]
    fn plants_within_budget() {
        for key in ["power"] {
            for (i, &(r, h)) in SIZES.iter().enumerate() {
                let tech = i as u8 + 1;
                let model = build_model_scaled(key, r, h, tech).unwrap();
                let [full, mid, coarse] = [0, 1, 2].map(|l| model.lods[l].indices.len() / 3);
                let top = model.lods[0]
                    .vertices
                    .iter()
                    .map(|v| v.pos[2])
                    .fold(0.0f32, f32::max);
                println!("{key} T{tech}: {full}/{mid}/{coarse}, {top:.1} m tall");
                let budget = if tech >= 3 {
                    crate::tests::REACTOR_TRIANGLES
                } else {
                    6000
                };
                assert!(
                    full <= budget && coarse < 60,
                    "{key} T{tech}: {full}/{mid}/{coarse}"
                );
                assert!(
                    top <= h + 0.05,
                    "{key} T{tech}: {top:.2} m tall, the blueprint says {h}"
                );
                assert!(
                    model.lods[0]
                        .vertices
                        .iter()
                        .any(|v| v.part == part::REACTOR_COLLAR_FIRST),
                    "{key} T{tech}: a ring turns"
                );
                // The dome holds its fusion behind glass; the cell and the star arc.
                assert_eq!(model.discharge.is_some(), tech != 2, "{key} T{tech}: arcs");
            }
        }
    }
}
