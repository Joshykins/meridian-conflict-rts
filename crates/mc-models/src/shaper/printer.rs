//! The printing Shaper (`replication_node`), the braced blades: a stepped foot; a sheer
//! dark core telescoping up in one section per tier, a pale lip where each comes out of
//! the one below; slab blades leaning round it with sky between them and the core,
//! braced across at every lip. The spine astern, two leaves with a slot of sky between,
//! stands through a gap in the Lens's great ring to the full height; the two flanks
//! forward stop at about half of it, their tops cut on a slant; from tier 3 two
//! buttresses stand astern of them. The Lens (the Lance's focus) rides on the core's
//! head, the anti-nuke's halo round it (`halo.rs`); the shield's ward is three plates
//! between the blades over the foot.
//!
//! Everything is laid out from the Shaper's height and the reach of its foot
//! (`super::reach`), so it grows broader and heavier as it grows taller: the blades'
//! feet stand out at the reach.

use std::f32::consts::PI;

use glam::Affine3A;

use super::super::builder::{MeshBuilder, Section};
use super::super::library::ModelDef;
use super::super::precursor::{cut_rect, pale, v3};
use super::halo::{self, Mount};
use super::kit::{course, lens, ring, shaft, Ring};
use super::Growth;

stage_set! {
    0, KEYS, MODELS, draw;
    p1 "replication_node" 1,
    p2 "replication_node@2" 2,
    p3 "replication_node@3" 3,
    p4 "replication_node@4" 4,
    p5 "replication_node@5" 5,
    p6 "replication_node@6" 6,
    p7 "replication_node@7" 7,
    p8 "replication_node@8" 8,
    p10 "replication_node@10" 10,
    p12 "replication_node@12" 12,
    p14 "replication_node@14" 14,
    p16 "replication_node@16" 16,
    p20 "replication_node@20" 20,
    p25 "replication_node@25" 25,
    p30 "replication_node@30" 30,
    p40 "replication_node@40" 40,
}

/// A blade round the core: its heading, top, and how far its outer edge stands out of
/// the core's sky at its top (`crest`) and, more, at its foot (`crest + swell`), as
/// shares of the reach.
struct Blade {
    heading: f32,
    top: f32,
    crest: f32,
    swell: f32,
    /// The spine: two leaves with a slot of sky between them.
    split: bool,
    /// Sections it is built of (it telescopes with the core).
    sections: usize,
}

/// Where the Lens and its halo stand on the printing Shaper `g` (also `shaper::lens_height`
/// and `shaper::halo_slot`, which put the hard light there).
pub(super) fn mount(g: Growth) -> Mount {
    let (h, r) = (g.height, g.reach);
    let lens_h = 0.08 * h + 0.1 * r + 6.0;
    Mount {
        g,
        lens_mid: h * 0.86 - lens_h * 0.2,
        lens_half: lens_h * 0.5,
        lens_r: 0.06 * h + 0.22 * r + 3.0,
        len: 0.12 * h + 6.0,
    }
}

pub(super) fn draw(b: &mut MeshBuilder, g: Growth) {
    let (h, r) = (g.height, g.reach);
    let sections = g.tier.min(10);

    // The foot, a course more every other tier up to three.
    let foot = 0.05 * h + 0.03 * r + 3.0;
    let courses = 1 + sections.min(4) / 2;
    let step = foot / courses as f32;
    for i in 0..courses {
        let z0 = step * i as f32;
        let hb = r * 0.62 * (1.0 - 0.1 * i as f32);
        course(b, z0, z0 + step, hb, hb * 0.94);
    }

    // The core, telescoping: a section a tier, each a little narrower.
    let m = mount(g);
    let (lens_h, lens_mid, lens_r) = (m.lens_half * 2.0, m.lens_mid, m.lens_r);
    let lens_floor = lens_mid - lens_h * 0.5;
    let c0 = r * 0.24;
    let lip = 0.008 * h + 0.8;
    let run = (lens_floor - foot) / sections as f32;
    let mut joints = Vec::new();
    let (mut z, mut c) = (foot, c0);
    for k in 0..sections {
        let z1 = z + run;
        let next = c * 0.93;
        shaft(b, z, z1, c, next.max(c * 0.97), (c * 2.4).max(6.0));
        if k + 1 < sections {
            pale(b);
            let plan = cut_rect(b, c * 1.02, c * 1.02, c * 0.3);
            b.loft_z(
                &plan,
                &[Section::new(z1 - lip, 1.0), Section::new(z1 + lip, 0.96)],
            );
            joints.push(z1);
        }
        c = next;
        z = z1;
    }
    let head = c;

    // The blades. Each one's foot stands out at the reach.
    let sky = r * 0.1 + 1.0;
    let inner = c0 + sky;
    let thick = r * 0.045 + 0.5;
    let mut blades = vec![
        Blade {
            heading: PI,
            top: h,
            crest: 0.26,
            swell: 0.4,
            split: true,
            sections: sections + 1,
        },
        Blade {
            heading: PI / 3.0,
            top: 0.56 * h,
            crest: 0.2,
            swell: 0.46,
            split: false,
            sections,
        },
        Blade {
            heading: -PI / 3.0,
            top: 0.56 * h,
            crest: 0.2,
            swell: 0.46,
            split: false,
            sections,
        },
    ];
    if g.tier >= 3 {
        for side in [1.0f32, -1.0] {
            blades.push(Blade {
                heading: side * PI * 2.0 / 3.0,
                top: 0.34 * h,
                crest: 0.16,
                swell: 0.5,
                split: false,
                sections: sections.div_ceil(2),
            });
        }
    }
    for blade in &blades {
        b.with(Affine3A::from_rotation_z(blade.heading), |b| {
            draw_blade(b, blade, inner, r, thick, 0.006 * h + 0.6);
            // Braces across the sky to the core at every lip.
            for &zj in &joints {
                if !b.coarse() && zj < blade.top - blade.top / blade.sections as f32 * 0.5 {
                    pale(b);
                    b.chamfered_box(
                        v3((c0 * 0.8 + inner) * 0.5, 0.0, zj),
                        v3(inner - c0 * 0.8 + 1.0, thick * 1.4, lip * 2.6),
                        lip * 0.4,
                    );
                }
            }
        });
    }

    // The ward: three plates between the blades, over the foot.
    ring(
        b,
        Ring {
            r0: c0 * 1.35,
            r1: c0 * 1.75,
            z: foot + run * 0.18,
            h: run * 0.03 + 0.4,
            pieces: 3,
            gap: c0 * 2.6,
            turn: -PI / 3.0 + PI,
        },
    );

    halo::draw(b, &m);

    // The Lens on the core's head, the spine standing through a gap in its great ring.
    pale(b);
    let cap = cut_rect(b, head, head, head * 0.28);
    b.loft_z(
        &cap,
        &[
            Section::new(lens_floor - 1.0, 1.0),
            Section::new(lens_floor + head * 0.15, 0.82),
        ],
    );
    lens(
        b,
        v3(0.0, 0.0, lens_mid),
        lens_h * 0.5,
        lens_r,
        0.0,
        // The halo hangs where the Lens's keys would.
        g.tier.min(3),
        thick * 6.4 + 0.01 * h,
        PI - PI / 12.0,
    );
}

/// A blade in its own frame (x out along its heading): sections stepping in as they
/// climb, the last cut on a slant, the spine's in two leaves.
fn draw_blade(b: &mut MeshBuilder, blade: &Blade, inner: f32, r: f32, thick: f32, gap: f32) {
    let n = blade.sections;
    let len = blade.top / n as f32;
    let out = |f: f32| inner + r * (blade.crest + blade.swell * (1.0 - f).powf(1.2));
    for k in 0..n {
        let z0 = k as f32 * len;
        let z1 = z0 + len - gap;
        let f0 = k as f32 / n as f32;
        let f1 = (k as f32 + 0.85) / n as f32;
        let last = k + 1 == n;
        let profile = if last {
            vec![
                [inner, z0],
                [out(f0), z0],
                [out(1.0) + r * 0.05, z1 - (z1 - z0) * 0.2],
                [inner, z1],
            ]
        } else {
            vec![[inner, z0], [out(f0), z0], [out(f1), z1], [inner, z1]]
        };
        pale(b);
        if blade.split {
            for (y, drop) in [(thick * 1.7, 0.0), (-thick * 1.7, len * 0.45)] {
                let mut leaf = profile.clone();
                if last {
                    leaf[2][1] -= drop;
                    leaf[3][1] -= drop;
                }
                b.at(v3(0.0, y, 0.0), |b| {
                    b.extrude_y_chamfered(&leaf, thick, thick * 0.2);
                });
            }
        } else {
            b.extrude_y_chamfered(&profile, thick, thick * 0.2);
        }
    }
}
