//! The ARC fusion plants, four-fold, built round the core:
//! - a foundation building up to the middle: a chamfered square plinth, heat sinks on
//!   its corners or on heavy feet splayed out on the diagonals, power trunks out along
//!   the axes (their conduits pulsing, `pattern::FLUX`), a tapered buttressed body;
//! - the core held up over the body on four suspension panels, churning
//!   (`pattern::CORE`), arcs crackling to it from the panels' electrodes
//!   (`Model::discharge`, renderer `reactor_fx.rs`);
//! - armoured blades on a turntable turning round it (`part::REACTOR_COLLAR_FIRST`).
//!
//! Tech 1, the cell, is small: two broad blades, heat sinks on the plinth's corners.
//! Tech 2 has four great blades on a stepped foundation with feet, capacitors and pumps.
//! Tech 3 holds a star: the heaviest foundation, capacitor bastions on the feet's toes
//! and rows of them down the trunks, taller blades. The heat sinks breathe: their cores
//! ripple with heat (`pattern::HEAT`), hot air shimmers off them and steam wisps rise.
//! Authored at the blueprint's size, in metres, power leaving toward +x.

use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, TAU};

use glam::{Vec2, Vec3};

use super::parts::*;
use crate::builder::{chamfered_rect, MeshBuilder, Section};
use crate::material::*;
use crate::{part, pattern};

/// Each tier's size: the blueprints' (radius, height), so the model is drawn at the
/// metres it is authored in.
pub(super) const SIZES: [(f32, f32); 3] = [(6.9, 10.0), (18.75, 26.0), (42.5, 52.0)];

/// One tier's plan, in metres; (out, up) pairs are measured from the z axis.
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
    /// The core's middle and its radii (across, up).
    core: (f32, Vec2),
    /// A suspension panel's path, (out, up), from the body's head to its electrode, and
    /// its section.
    panel: [Vec2; 3],
    panel_size: Vec2,
    /// A blade's outer face, (out, up), a Bezier from its foot to its tip, and its depth.
    blade: [Vec2; 4],
    blade_depth: f32,
    /// The turntable the blades stand on: its radius and its top.
    table: (f32, f32),
    capacitors: bool,
    /// Tech 3's capacitor bastions on the feet's toes and rows down the trunks.
    bastions: bool,
    /// Pumps by the trunks, their size (0: none).
    pumps: f32,
}

const PLANTS: [Plant; 2] = [
    Plant {
        plinth: (15.0, 4.2, 1.6),
        steps: &[(11.5, 1.8)],
        foot: (
            Vec2::new(11.0, 24.0),
            Vec2::new(6.8, 5.0),
            Vec2::new(6.0, 1.3),
        ),
        fins: 7,
        trunk: (18.4, 4.4, Vec2::new(3.6, 1.4)),
        body: (Vec2::new(8.4, 6.6), 11.4, 2.4),
        core: (18.6, Vec2::new(3.0, 3.8)),
        panel: [
            Vec2::new(5.6, 11.4),
            Vec2::new(7.4, 15.6),
            Vec2::new(5.6, 18.6),
        ],
        panel_size: Vec2::new(2.2, 1.1),
        blade: [
            Vec2::new(9.6, 11.8),
            Vec2::new(10.8, 17.0),
            Vec2::new(9.2, 22.4),
            Vec2::new(7.6, 24.6),
        ],
        blade_depth: 1.1,
        table: (10.4, 11.9),
        capacitors: true,
        bastions: false,
        pumps: 0.95,
    },
    Plant {
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
        core: (37.0, Vec2::new(8.0, 9.5)),
        panel: [
            Vec2::new(10.0, 23.0),
            Vec2::new(12.6, 28.0),
            Vec2::new(10.2, 32.0),
        ],
        panel_size: Vec2::new(3.6, 1.8),
        blade: [
            Vec2::new(18.5, 25.5),
            Vec2::new(22.5, 33.5),
            Vec2::new(21.5, 44.0),
            Vec2::new(16.5, 50.0),
        ],
        blade_depth: 2.0,
        table: (19.5, 25.5),
        capacitors: true,
        bastions: true,
        pumps: 1.7,
    },
];

pub(super) fn power(b: &mut MeshBuilder, tech: u8) {
    match tech {
        0 | 1 => cell(b),
        2 => plant(b, &PLANTS[0]),
        _ => plant(b, &PLANTS[1]),
    }
}

fn plant(b: &mut MeshBuilder, p: &Plant) {
    let core = v3(0.0, 0.0, p.core.0);
    b.set_spinner_pivot(core);
    if b.coarse() {
        let (half, _, deck) = p.plinth;
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
            v3(0.0, 0.0, p.blade[0].y),
            4,
            p.blade[1].x * 1.2,
            p.blade[3].x * 1.1,
            p.blade[3].y - p.blade[0].y,
        );
        team_panel(b, v3(-half * 0.6, 0.0, deck), v2(half * 0.3, half * 0.5));
        return;
    }
    foundation(b, p);
    body(b, p);
    // The core, and the four panels that hold it up, on the diagonals.
    b.paint(GLOW).pattern(pattern::CORE);
    let (sides, bands) = if b.fine() { (20, 10) } else { (10, 5) };
    b.spheroid(core, v3(p.core.1.x, p.core.1.x, p.core.1.y), sides, bands);
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
    b.set_discharge(core, p.core.1.x, &tips);
    // The turntable, and the blades on it turning round the core.
    b.with_part(part::REACTOR_COLLAR_FIRST, |b| {
        let (r, top) = p.table;
        let deck = p.body.1;
        let sides = round(b, 24);
        b.paint(ACCENT);
        b.loft_z(
            &ngon_plan(sides, 1.0),
            &[Section::new(deck, r * 0.9), Section::new(top, r)],
        );
        b.paint(GLOW).pattern(pattern::CHARGE);
        annulus(b, sides, r * 0.8, r * 0.86, top, top + r * 0.012);
        b.radial(4, |b| blade(b, p.blade, p.blade_depth, TAU / 4.0 * 0.44));
    });
}

/// Tech 1's head height: the top of its body.
const CELL_TOP: f32 = 3.8;

/// Tech 1's foundation and body: a compact square plinth with a radiator block on each
/// corner and two trunks out along x, and a squat tapered body with a buttress on each
/// corner. Drawn whole at the far level (with the head's mass), where it returns false.
fn cell_base(b: &mut MeshBuilder) -> bool {
    const DECK: f32 = 0.7;
    const BODY: f32 = CELL_TOP;
    if b.coarse() {
        b.paint(PLATING);
        b.prism(Vec3::ZERO, 4, 9.4, 9.0, DECK);
        b.paint(ACCENT);
        b.prism(v3(0.0, 0.0, DECK), 4, 4.6, 3.8, BODY - DECK);
        b.paint(PLATING);
        b.prism(v3(0.0, 0.0, BODY), 4, 3.4, 4.6, 5.4);
        team_panel(b, v3(-3.8, 0.0, DECK), v2(1.4, 2.4));
        return false;
    }
    // The plinth.
    let plan = chamfered_rect(v2(6.5, 6.5), 2.0);
    b.paint(ACCENT);
    b.loft_z(
        &plan,
        &[Section::new(0.0, 1.03), Section::new(DECK * 0.4, 1.0)],
    );
    b.paint(PLATING);
    b.loft_z(
        &plan,
        &[Section::new(DECK * 0.4, 0.99), Section::new(DECK, 0.95)],
    );
    // A heat sink on each corner, out along the diagonal.
    b.yawed(Vec3::ZERO, FRAC_PI_4, |b| {
        b.radial(4, |b| {
            b.at(v3(4.4, 0.0, DECK), |b| heatsink(b, 2.7, 2.3, 7))
        })
    });
    // Trunks out along x, the power pulsing out along them.
    b.radial(2, |b| {
        b.paint(ACCENT);
        wedge(b, 3.0, 7.4, v2(1.6, 1.4), v2(1.4, 0.45));
        b.paint(ACCENT).pattern(pattern::FLUX);
        b.beam(
            v3(3.6, 0.0, 1.3),
            v3(7.0, 0.0, 0.6),
            v2(0.55, 0.12),
            v2(0.55, 0.12),
        );
    });
    b.mirror_y(|b| team_panel(b, v3(0.0, 5.4, DECK), v2(2.2, 0.8)));
    // The body: tapered, its cornice, a buttress up each face.
    let plan = chamfered_rect(v2(3.6, 3.6), 1.2);
    b.paint(PLATING);
    b.loft_z(
        &plan,
        &[Section::new(DECK, 1.0), Section::new(BODY - 0.4, 0.82)],
    );
    b.paint(ACCENT);
    b.loft_z(
        &plan,
        &[Section::new(BODY - 0.4, 0.85), Section::new(BODY, 0.8)],
    );
    b.yawed(Vec3::ZERO, FRAC_PI_4, |b| {
        b.radial(4, |b| {
            b.paint(ACCENT);
            b.beam(
                v3(4.3, 0.0, DECK),
                v3(3.3, 0.0, BODY - 0.4),
                v2(1.1, 0.7),
                v2(0.8, 0.3),
            );
        })
    });
    true
}

/// Tech 1, the cell: the core held up on four panels on the diagonals, two broad blades turning
/// round it on a turntable.
fn cell(b: &mut MeshBuilder) {
    let top = CELL_TOP;
    let core = v3(0.0, 0.0, top + 2.5);
    b.set_spinner_pivot(core);
    if !cell_base(b) {
        return;
    }
    b.paint(GLOW).pattern(pattern::CORE);
    let (sides, bands) = if b.fine() { (18, 9) } else { (10, 5) };
    b.spheroid(core, v3(1.2, 1.2, 1.5), sides, bands);
    let path = [
        v3(2.0, 0.0, top),
        v3(2.5, 0.0, top + 1.2),
        v3(1.95, 0.0, top + 2.4),
    ];
    b.yawed(Vec3::ZERO, FRAC_PI_4, |b| {
        b.radial(4, |b| {
            b.paint(PLATING);
            sweep(b, &path, v2(0.8, 0.4), false);
            electrode(b, path[2], (core - path[2]).normalize(), 0.35);
        })
    });
    let tips: Vec<Vec3> = (0..4)
        .map(|i| {
            let a = FRAC_PI_4 + FRAC_PI_2 * i as f32;
            v3(path[2].x * a.cos(), path[2].x * a.sin(), path[2].z)
        })
        .collect();
    b.set_discharge(core, 1.2, &tips);
    let blade_face = [
        v2(3.1, top + 0.4),
        v2(3.8, top + 2.2),
        v2(3.3, top + 4.4),
        v2(2.2, top + 5.4),
    ];
    b.with_part(part::REACTOR_COLLAR_FIRST, |b| {
        let sides = round(b, 20);
        b.paint(ACCENT);
        annulus(b, sides, 2.5, 3.6, top, top + 0.4);
        b.paint(GLOW).pattern(pattern::CHARGE);
        annulus(b, sides, 2.8, 3.1, top + 0.4, top + 0.43);
        b.radial(2, |b| blade(b, blade_face, 0.5, TAU / 2.0 * 0.42));
    });
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
/// at the toe, a radiator bank laid along its back, and on the bigger plants a capacitor
/// at its toe.
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
    if p.bastions {
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
    } else if p.capacitors {
        capacitor(
            b,
            v3(heel + (out.y - heel) * 0.35, 0.0, height.y * 1.3),
            width.y * 0.26,
            width.y * 0.55,
        );
    }
}

/// A power trunk on +x: a low dark wedge from under the body out along the axis, a
/// conduit pulsing out along it, pumps either side of its toe on the bigger plants.
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
    if p.pumps > 0.0 && b.fine() {
        b.mirror_y(|b| pump(b, v3(to - width * 0.9, width * 1.0, p.plinth.2), p.pumps));
    }
    if p.bastions {
        // A row of capacitors down each side of the trunk.
        b.mirror_y(|b| {
            for k in 0..3 {
                let x = from + (to - from) * (0.35 + 0.17 * k as f32);
                capacitor(b, v3(x, width * 0.9, p.plinth.2), width * 0.2, width * 0.42);
            }
        });
    }
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

/// A suspension panel on +x: a thick plate rising off the body's head, leaning out and
/// back in at the core, a collar round its knee, an electrode at its tip.
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

/// A blade on +x: a curved armoured shell on the turntable, a quarter turn less its gap,
/// leaning in over the core; its inner face lit, a dark spine down its back.
fn blade(b: &mut MeshBuilder, curve: [Vec2; 4], depth: f32, span: f32) {
    // Samples up the face: six close to, three at the middle distance.
    let n = if b.fine() { 6 } else { 3 };
    let steps = if b.fine() { 8 } else { 3 };
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
    let n = if b.fine() { fins } else { fins.div_ceil(2) };
    let pitch = len * 0.86 / n as f32;
    b.paint(PLATING);
    for k in 0..n {
        let x = len * 0.07 + pitch * (k as f32 + 0.5);
        let tall = if k % 2 == 0 { 1.0 } else { 0.76 };
        b.cuboid(
            v3(x, 0.0, base + fin * tall * 0.5),
            v3(pitch * 0.32, w * 0.8, fin * tall),
        );
    }
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

/// A circulating pump: a housing on the deck and its piston working above it (`part::PUMP`).
fn pump(b: &mut MeshBuilder, base: Vec3, s: f32) {
    b.paint(ACCENT);
    b.chamfered_box(
        base + Vec3::Z * 0.8 * s,
        v3(2.2 * s, 2.2 * s, 1.6 * s),
        0.4 * s,
    );
    b.paint(METAL);
    b.prism(
        base + Vec3::Z * 1.6 * s,
        round(b, 8),
        0.7 * s,
        0.7 * s,
        0.25 * s,
    );
    b.with_part(part::PUMP, |b| {
        b.paint(METAL);
        b.prism(
            base + Vec3::Z * 1.6 * s,
            round(b, 8),
            0.35 * s,
            0.35 * s,
            1.9 * s,
        );
        b.paint(PLATING);
        b.chamfered_box(
            base + Vec3::Z * 3.7 * s,
            v3(1.1 * s, 1.1 * s, 0.6 * s),
            0.2 * s,
        );
    });
    if b.fine() {
        glow_strip(
            b,
            base + v3(1.12 * s, 0.0, 0.4 * s),
            v2(0.08, 1.2 * s),
            GLOW,
        );
    }
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
                    model.lods[0]
                        .vertices
                        .iter()
                        .any(|v| v.part == part::REACTOR_COLLAR_FIRST),
                    "{key} T{tech}: the blades turn"
                );
                assert!(model.discharge.is_some(), "{key} T{tech}: arcs");
            }
        }
    }
}
