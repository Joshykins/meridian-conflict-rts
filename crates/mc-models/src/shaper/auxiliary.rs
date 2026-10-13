//! The auxiliary Shapers (`progenitor_seedcaster`, `progenitor_unmaker`,
//! `progenitor_gatewright`), the stacked
//! spire: stepped courses (a course every two tiers), four blades out of the foot's
//! corners leaning in to the shaft, the shaft in one stage per tier, each hovering over
//! the one below on a dark neck with a collar of segments floating round it, the Lens
//! (the Lance's focus) near the top of the stack, and over it the crown that says which
//! it is: the artillery's bore, two rails leaning forward with a slot between them, or
//! the nuke's silo, a squat block round a dark well with its hatches thrown open. The
//! anti-nuke is the halo round the Lens (`halo.rs`), as on every Shaper; the shield's
//! ward is four plates over the foot.

use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI};

use glam::Affine3A;

use super::super::builder::{MeshBuilder, Section};
use super::super::library::ModelDef;
use super::super::precursor::{cut_rect, dark, pale, v3};
use super::halo::{self, Mount};
use super::kit::{course, joint, ring, shaft, Ring};
use super::Growth;

stage_set! {
    1, ARTILLERY_KEYS, ARTILLERY_MODELS, draw_artillery;
    a1 "progenitor_seedcaster" 1,
    a2 "progenitor_seedcaster@2" 2,
    a3 "progenitor_seedcaster@3" 3,
    a4 "progenitor_seedcaster@4" 4,
    a5 "progenitor_seedcaster@5" 5,
    a6 "progenitor_seedcaster@6" 6,
    a7 "progenitor_seedcaster@7" 7,
    a8 "progenitor_seedcaster@8" 8,
    a10 "progenitor_seedcaster@10" 10,
    a12 "progenitor_seedcaster@12" 12,
    a14 "progenitor_seedcaster@14" 14,
    a16 "progenitor_seedcaster@16" 16,
    a20 "progenitor_seedcaster@20" 20,
    a25 "progenitor_seedcaster@25" 25,
    a30 "progenitor_seedcaster@30" 30,
    a40 "progenitor_seedcaster@40" 40,
}

stage_set! {
    2, NUKE_KEYS, NUKE_MODELS, draw_nuke;
    n1 "progenitor_unmaker" 1,
    n2 "progenitor_unmaker@2" 2,
    n3 "progenitor_unmaker@3" 3,
    n4 "progenitor_unmaker@4" 4,
    n5 "progenitor_unmaker@5" 5,
    n6 "progenitor_unmaker@6" 6,
    n7 "progenitor_unmaker@7" 7,
    n8 "progenitor_unmaker@8" 8,
    n10 "progenitor_unmaker@10" 10,
    n12 "progenitor_unmaker@12" 12,
    n14 "progenitor_unmaker@14" 14,
    n16 "progenitor_unmaker@16" 16,
    n20 "progenitor_unmaker@20" 20,
    n25 "progenitor_unmaker@25" 25,
    n30 "progenitor_unmaker@30" 30,
    n40 "progenitor_unmaker@40" 40,
}

stage_set! {
    3, FLEET_KEYS, FLEET_MODELS, draw_fleet;
    g1 "progenitor_gatewright" 1,
    g2 "progenitor_gatewright@2" 2,
    g3 "progenitor_gatewright@3" 3,
    g4 "progenitor_gatewright@4" 4,
    g5 "progenitor_gatewright@5" 5,
    g6 "progenitor_gatewright@6" 6,
    g7 "progenitor_gatewright@7" 7,
    g8 "progenitor_gatewright@8" 8,
    g10 "progenitor_gatewright@10" 10,
    g12 "progenitor_gatewright@12" 12,
    g14 "progenitor_gatewright@14" 14,
    g16 "progenitor_gatewright@16" 16,
    g20 "progenitor_gatewright@20" 20,
    g25 "progenitor_gatewright@25" 25,
    g30 "progenitor_gatewright@30" 30,
    g40 "progenitor_gatewright@40" 40,
}

/// An Unmaker's silo (`progenitor_silo`), a small piece of its spire: a battered
/// course, a short stage with a collar floating round its neck, and the silo crown
/// thrown open over it, where the warhead leaves.
pub(super) const SILO: ModelDef = ModelDef::new("progenitor_silo", 18.0, 24.0, silo_piece);

fn silo_piece(b: &mut MeshBuilder, _tech: u8) {
    course(b, 0.0, 5.0, 15.0, 13.5);
    shaft(b, 5.0, 14.0, 8.0, 7.0, 0.0);
    if b.coarse() {
        // Far off, a block where the crown stands.
        pale(b);
        b.chamfered_box(v3(0.0, 0.0, 19.0), v3(16.0, 16.0, 10.0), 0.0);
        return;
    }
    joint(b, 14.0, 15.0, 4.5);
    collar(b, 14.5, 7.0, 1.6, false);
    silo(b, 15.0, 24.0, 6.0);
}

/// What stands over an auxiliary Shaper's Lens.
#[derive(Clone, Copy, PartialEq)]
enum Crown {
    /// The replication artillery's bore.
    Bore,
    /// The nuke's silo.
    Silo,
    /// The fleet's gate: a great ring standing upright, facing forward.
    Gate,
}

pub(super) fn draw_artillery(b: &mut MeshBuilder, g: Growth) {
    spire(b, g, Crown::Bore);
}

pub(super) fn draw_nuke(b: &mut MeshBuilder, g: Growth) {
    spire(b, g, Crown::Silo);
}

pub(super) fn draw_fleet(b: &mut MeshBuilder, g: Growth) {
    spire(b, g, Crown::Gate);
}

/// Where the Lens and its halo stand on the auxiliary Shaper `g` of `kind` (1 artillery,
/// else nuke; also `shaper::lens_height` and `shaper::halo_slot`).
pub(super) fn mount(g: Growth, kind: u8) -> Mount {
    let crown = match kind {
        1 => Crown::Bore,
        3 => Crown::Gate,
        _ => Crown::Silo,
    };
    let h = g.height;
    let (lens_h, lens_floor) = lens(h, crown);
    let lens_r = 0.1 * h + 6.0;
    Mount {
        g,
        lens_mid: lens_floor + lens_h * 0.5,
        lens_half: lens_h * 0.5,
        lens_r,
        len: lens_r * 1.15,
    }
}

/// The Lens's height and its floor on a spire `h` tall under `crown`.
fn lens(h: f32, crown: Crown) -> (f32, f32) {
    let lens_h = 0.1 * h + 8.0;
    let crown_h = match crown {
        Crown::Bore => 0.12 * h + 6.0,
        Crown::Silo => 0.07 * h + 4.0,
        Crown::Gate => 0.34 * h + 4.0,
    };
    (lens_h, h - crown_h - lens_h)
}

fn spire(b: &mut MeshBuilder, g: Growth, crown: Crown) {
    let (h, t) = (g.height, g.tier.min(10));
    let bh = g.reach * 0.7;
    // The foot.
    let courses = 1 + t.min(6) / 2;
    let foot = 0.1 * h + 3.0;
    let top_half = bh * 0.62;
    let step = foot / courses as f32;
    for i in 0..courses {
        let z0 = step * i as f32;
        let z1 = z0 + step * 0.84;
        let hb = bh - (bh - top_half) * i as f32 / courses as f32;
        let ht = hb - (bh - top_half) / courses as f32 * 0.5;
        course(b, z0, z1, hb, ht);
        if i + 1 < courses {
            joint(b, z1, z0 + step, ht * 0.94);
        }
    }

    // The shaft, one stage per tier, shorter as it climbs.
    let (lens_h, lens_floor) = lens(h, crown);
    let (w0, w1) = (bh * 0.6, bh * 0.34);
    let width = |z: f32| w0 + (w1 - w0) * (z - foot) / (lens_floor - foot);
    let neck = 0.014 * h + 1.5;
    let weights: Vec<f32> = (0..t).map(|k| 0.84f32.powi(k as i32)).collect();
    let sum: f32 = weights.iter().sum();
    let run = lens_floor - foot - neck * (t - 1) as f32;
    let mut z = foot;
    let mut first = 0.0;
    let mut top = w1;
    for (k, wgt) in weights.iter().enumerate() {
        let z1 = z + run * wgt / sum;
        let (h0, h1) = (width(z), width(z1) * 0.9);
        if k == 0 {
            first = z1 - z;
        }
        shaft(b, z, z1, h0, h1, (h0 * 2.2).max(6.0));
        top = h1;
        if k + 1 < t {
            joint(b, z1, z1 + neck, h1 * 0.55);
            collar(b, z1 + neck * 0.5, h1, neck, k % 2 == 0);
        }
        z = z1 + neck;
    }

    // Blades out of the foot's corners, leaning in to the first stage.
    let corner = g.reach;
    let rise = foot + first * 0.85;
    let blade = [
        [w0 * 0.9, 0.0],
        [corner, 0.0],
        [corner, foot * 0.3],
        [w0 * 1.4, rise],
        [w0 * 0.9, rise],
    ];
    b.radial(4, |b| {
        b.with(Affine3A::from_rotation_z(FRAC_PI_4), |b| {
            pale(b);
            b.extrude_y_chamfered(&blade, bh * 0.06, bh * 0.015);
        });
    });

    // The ward: four plates hovering over the foot, between the blades.
    ring(
        b,
        Ring {
            r0: w0 * 1.25,
            r1: w0 * 1.75,
            z: foot + first * 0.12,
            h: first * 0.03 + 0.4,
            pieces: 4,
            gap: w0 * 1.1,
            turn: FRAC_PI_4,
        },
    );

    // The pillars of the Lens, the Lens and its roof drum.
    let lens_mid = lens_floor + lens_h * 0.5;
    let pillar = [
        [top * 0.95, lens_floor - neck],
        [top * 1.35, lens_floor - neck],
        [top * 1.5, lens_mid],
        [top * 1.35, lens_floor + lens_h + neck],
        [top * 0.95, lens_floor + lens_h + neck],
    ];
    b.radial(4, |b| {
        b.with(Affine3A::from_rotation_z(FRAC_PI_4), |b| {
            pale(b);
            b.extrude_y_chamfered(&pillar, top * 0.16, top * 0.04);
        });
    });
    let kind = match crown {
        Crown::Bore => 1,
        Crown::Silo => 2,
        Crown::Gate => 3,
    };
    let m = mount(g, kind);
    let lens_r = m.lens_r;
    super::kit::lens(
        b,
        v3(0.0, 0.0, lens_mid),
        lens_h * 0.5,
        lens_r,
        top,
        // The halo hangs where the Lens's keys would.
        g.tier.min(3),
        0.0,
        0.0,
    );
    halo::draw(b, &m);
    let roof = lens_floor + lens_h;
    pale(b);
    let drum = cut_rect(b, top, top, top * 0.28);
    b.loft_z(
        &drum,
        &[
            Section::new(roof - top * 0.2, 0.6),
            Section::new(roof - top * 0.12, 0.84),
            Section::new(roof + neck, 1.0),
        ],
    );
    match crown {
        Crown::Bore => bore(b, roof + neck, h, top),
        Crown::Silo => silo(b, roof + neck, h, top),
        Crown::Gate => gate(b, roof + neck, h, top),
    }
}

/// A gate from `z0` to `z1`: the top of its stem, the ring's middle and its radius.
fn gate_ring(z0: f32, z1: f32) -> (f32, f32, f32) {
    let base = z0 + (z1 - z0) * 0.12;
    let r = (z1 - base) * 0.5;
    (base, base + r * 1.02, r)
}

/// Where a Gatewright of growth `g` casts its fleet's print beams from: the middles of
/// every other segment of its gate ring (odd multiples of 36 degrees round from the
/// bottom, so none by the stem), on the ring's inner face at its front, in model space
/// (`shaper::gate_emitters`). The ring's ten segments are centred on multiples of 36
/// degrees from the bottom (`gate`: `turn` PI/10).
pub(super) fn gate_emitters(g: Growth) -> Vec<[f32; 3]> {
    let h = g.height;
    let (lens_h, lens_floor) = lens(h, Crown::Gate);
    let neck = 0.014 * h + 1.5;
    let (_, zc, r) = gate_ring(lens_floor + lens_h + neck, h);
    (0..GATE_EMITTERS)
        .map(|k| {
            let a = (36.0 + 72.0 * k as f32).to_radians();
            [r * 0.13, r * 0.8 * a.sin(), zc - r * 0.8 * a.cos()]
        })
        .collect()
}

/// Print beam emitters round a Gatewright's gate (mc-sim `survival::GATE_EMITTERS`).
pub(super) const GATE_EMITTERS: usize = 5;

/// The Gatewright's gate from `z0` to `z1` over a roof `half` across: a great ring of
/// segments standing upright on a stem over a plinth, clear of the halo, facing forward
/// (x), the way its fleets come out, held between two braced arms leaning out from the
/// plinth to its flanks.
fn gate(b: &mut MeshBuilder, z0: f32, z1: f32, half: f32) {
    let len = z1 - z0;
    let deck = z0 + len * 0.05;
    pale(b);
    let plinth = cut_rect(b, half * 1.1, half * 1.3, half * 0.3);
    b.loft_z(&plinth, &[Section::new(z0, 0.85), Section::new(deck, 1.0)]);
    // A short dark stem lifting the ring clear of the roof.
    let (base, zc, r) = gate_ring(z0, z1);
    dark(b);
    let stem = cut_rect(b, half * 0.4, half * 0.55, half * 0.12);
    b.loft_z(
        &stem,
        &[
            Section::new(deck - 0.3, 1.0),
            Section::new(base + len * 0.02, 0.8),
        ],
    );
    // The ring, its foot just clear of the plinth.
    b.with(
        Affine3A::from_translation(v3(0.0, 0.0, zc)) * Affine3A::from_rotation_y(FRAC_PI_2),
        |b| {
            ring(
                b,
                Ring {
                    r0: r * 0.8,
                    r1: r,
                    z: 0.0,
                    h: r * 0.13,
                    pieces: 10,
                    gap: r * 0.07,
                    turn: PI / 10.0,
                },
            );
            if !b.coarse() {
                // A thin inner course of segments, set back, so the ring reads deep.
                ring(
                    b,
                    Ring {
                        r0: r * 0.68,
                        r1: r * 0.74,
                        z: -r * 0.06,
                        h: r * 0.05,
                        pieces: 20,
                        gap: r * 0.05,
                        turn: 0.0,
                    },
                );
            }
        },
    );
    // The arms: braced slabs from the plinth's sides out and up to the ring's flanks.
    let arm = [
        [half * 0.9, deck],
        [half * 1.45, deck],
        [r * 1.03, zc + r * 0.12],
        [r * 0.84, zc + r * 0.02],
    ];
    for turn in [FRAC_PI_2, -FRAC_PI_2] {
        b.with(Affine3A::from_rotation_z(turn), |b| {
            pale(b);
            b.extrude_y_chamfered(&arm, r * 0.09, r * 0.025);
        });
    }
}

/// A collar of segments round a neck at `z`, under a stage `h1` half across: a great
/// one with a thin outer ring, or a lesser one.
fn collar(b: &mut MeshBuilder, z: f32, h1: f32, neck: f32, great: bool) {
    if !great {
        ring(
            b,
            Ring {
                r0: h1 * 1.3,
                r1: h1 * 1.85,
                z,
                h: neck * 0.3,
                pieces: 8,
                gap: h1 * 0.16,
                turn: 0.0,
            },
        );
        return;
    }
    ring(
        b,
        Ring {
            r0: h1 * 1.55,
            r1: h1 * 2.45,
            z,
            h: neck * 0.45,
            pieces: 8,
            gap: h1 * 0.2,
            turn: PI / 8.0,
        },
    );
    if b.coarse() {
        return;
    }
    ring(
        b,
        Ring {
            r0: h1 * 2.7,
            r1: h1 * 3.1,
            z: z - neck * 0.05,
            h: neck * 0.2,
            pieces: 16,
            gap: h1 * 0.16,
            turn: PI / 16.0,
        },
    );
}

/// The artillery's bore from `z0` to `z1`: a breech block, and over it two rails
/// leaning forward with a slot of sky between them, their tops cut on a slant.
fn bore(b: &mut MeshBuilder, z0: f32, z1: f32, half: f32) {
    let len = z1 - z0;
    pale(b);
    b.chamfered_box(
        v3(0.0, 0.0, z0 + len * 0.1),
        v3(half * 2.0, half * 2.0, len * 0.2),
        half * 0.2,
    );
    let lean = len * 0.22;
    let rail = [
        [-half * 0.75, z0 + len * 0.15],
        [half * 0.55, z0 + len * 0.15],
        [half * 0.55 + lean, z1 - len * 0.16],
        [-half * 0.35 + lean, z1],
    ];
    for y in [-half * 0.62, half * 0.62] {
        b.at(v3(0.0, y, 0.0), |b| {
            b.extrude_y_chamfered(&rail, half * 0.3, half * 0.06);
        });
    }
}

/// The nuke's silo from `z0` to `z1`: a squat battered block round a dark well, four
/// hatch slabs thrown open round its mouth.
fn silo(b: &mut MeshBuilder, z0: f32, z1: f32, half: f32) {
    let len = z1 - z0;
    let w = half * 1.5;
    let rim = z0 + len * 0.55;
    pale(b);
    let plan = cut_rect(b, w, w, w * 0.3);
    b.loft_z(&plan, &[Section::new(z0, 0.8), Section::new(rim, 1.0)]);
    dark(b);
    let well = cut_rect(b, w * 0.5, w * 0.5, w * 0.15);
    b.loft_z(
        &well,
        &[
            Section::new(rim - len * 0.2, 1.0),
            Section::new(rim + 0.3, 1.0),
        ],
    );
    // The hatches, hinged at the well's edge and leaning out.
    let open = 0.6f32;
    b.radial(4, |b| {
        b.with(
            Affine3A::from_translation(v3(w * 0.52, 0.0, rim)) * Affine3A::from_rotation_y(open),
            |b| {
                pale(b);
                let slab = [
                    [-w * 0.06, 0.0],
                    [w * 0.06, 0.0],
                    [w * 0.06, (z1 - rim) * 1.05],
                    [-w * 0.06, (z1 - rim) * 0.9],
                ];
                b.extrude_y_chamfered(&slab, w * 0.42, w * 0.03);
            },
        );
    });
}
