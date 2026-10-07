//! Skyhook (`aster_t5_space_elevator`, mesh "space_elevator"): ARC's tech 5 space
//! elevator, authored at blueprint scale (metres; a 26x26 lot of 12 m cells, 312 m
//! square, 300 m high).
//!
//! A clamp with three sides: three heavy clamp towers at 0, 120 and 240 degrees round a
//! central receiving well, the ways out between them (60, 180 and 300 degrees) kept
//! open, each with a marked apron to the lot edge. A cable comes down out of orbit into
//! the well, the three jaws swing in and grip it, and every half minute three cargo
//! crates fall down it and land in the ways out, open their doors and roofs and let their
//! units drive out, then sink into the ground. The crates only ever come down.
//!
//! What moves (`gpu_consts::elevator`, entity.wgsl `ELEVATOR_PART_*`):
//! - The jaws (`part::ELEVATOR_ARM`), authored shut on the cable: each swings out
//!   about its own tower's hinge ([`HINGE`]) while the clamps are open.
//! - The cable (`part::ELEVATOR_CABLE`), from its coupling seated in the well up to just
//!   over `CABLE_SPLIT`; what is above the split is drawn up into orbit.
//! - The crates: a shell (`part::ELEVATOR_CRATE`: walls, frame, braking thruster bells)
//!   authored landed, two door leaves (`part::ELEVATOR_DOOR`) shut on its outer end and
//!   two roof halves (`part::ELEVATOR_ROOF`) shut over it. Each stands in a bay, a box
//!   on the bearing of a way out (`BAY_*`, the same numbers the sim seats units by).
//!
//! Everything else stands still (`part::HULL`): the foundation, the well, the landing
//! pad round the cable, the aprons and the towers. The towers keep out of the jaws'
//! swing (the gap between a tower's cheeks, and everything over and behind its hinge out
//! of reach), and nothing stands in the bays above the pad.
//!
//! The towers and jaws are the `pylon` design: armoured A-frames with straight
//! ram-driven jaws over a fortified well (picked over a knuckle-boom gantry and a
//! terraced buttress design, in history at a03495cf).

mod pylon;

use std::f32::consts::TAU;

use glam::{Vec2, Vec3};

use super::parts::*;
use crate::builder::{chamfered_rect, ngon, MeshBuilder, Section};
use crate::gpu_consts::elevator::*;
use crate::material::*;
use crate::{part, pattern};

/// The jaws' hinge, in a tower's frame (x out along its bearing, z up).
const HINGE: Vec3 = Vec3::new(HINGE_R, 0.0, HINGE_Z);

// ---- the foundation -----------------------------------------------------------------

/// The slab's half width, its corner cut and its top.
const SLAB: f32 = 154.0;
const SLAB_CUT: f32 = 26.0;
const SLAB_TOP: f32 = 0.45;
/// The well: its open radius (the deck docks inside it) and the rim round it.
const WELL_R: f32 = DECK_R + 2.5;
const RIM_R: f32 = WELL_R + 10.0;
const RIM_TOP: f32 = 2.6;
/// Half the angle each exit takes out of the rim, about its bearing.
const EXIT_HALF: f32 = 0.62;
/// The aprons' half width at the well and at the lot edge, and how far out they run.
const APRON_IN: f32 = 34.0;
const APRON_OUT: f32 = 46.0;
const APRON_END: f32 = 168.0;
const APRON_TOP: f32 = 0.6;

// ---- the landing pad ----------------------------------------------------------------

/// The landing pad plate's underside.
const DECK_FOOT: f32 = SLAB_TOP - 0.1;
/// The guide sleeve round the cable: bore, outer radius, top and foot.
const SLEEVE_IN: f32 = 14.5;
const SLEEVE_OUT: f32 = 19.5;
const SLEEVE_TOP: f32 = 32.0;

// ---- the cable ----------------------------------------------------------------------

/// The cable's core, and the grip band the jaws close on (radius `CABLE_R`).
const CORE_R: f32 = 8.0;
const BAND_HALF: f32 = 14.0;
/// The coupling at its foot: radius (it passes through the sleeve), and top.
const COUPLING_R: f32 = 13.5;
const COUPLING_TOP: f32 = 24.0;
/// The authored top of the cable, just over the split, and so drawn up into orbit.
const CABLE_TOP: f32 = CABLE_SPLIT + 1.5;
const _: () = assert!(COUPLING_R < SLEEVE_IN && COUPLING_TOP < SLEEVE_TOP);
const _: () = assert!(CABLE_TOP <= 300.0 && GRIP_Z + BAND_HALF < CABLE_SPLIT);

/// Three armoured A-frame pylons with straight ram-driven jaws over a fortified well.
pub(super) fn space_elevator(b: &mut MeshBuilder, _tech: u8) {
    build(b, pylon::tower, pylon::jaw);
    if !b.coarse() {
        b.radial(3, pylon::bastion);
    }
}

fn build(b: &mut MeshBuilder, tower: fn(&mut MeshBuilder), jaw: fn(&mut MeshBuilder)) {
    if b.coarse() {
        coarse(b);
        return;
    }
    foundation(b);
    b.radial(3, |b| {
        b.yawed(Vec3::ZERO, TAU / 6.0, apron);
        tower(b);
        beacon(b);
        b.with_part(part::ELEVATOR_ARM, jaw);
    });
    cable(b);
    landing_pad(b);
    crates(b);
}

/// Far off: the slab, three tower blocks, three jaws as spikes, the cable and the
/// crates, and the owner's colour on a crown. Under 60 triangles.
fn coarse(b: &mut MeshBuilder) {
    b.paint(PLATING_DARK);
    b.decal(v3(0.0, 0.0, SLAB_TOP), v2(2.0 * SLAB, 2.0 * SLAB));
    b.radial(3, |b| {
        // A tower: a three-sided block, open underneath.
        b.paint(PLATING);
        let foot = [
            v3(76.0, -40.0, 0.0),
            v3(150.0, 0.0, 0.0),
            v3(76.0, 40.0, 0.0),
        ];
        let top = [
            v3(96.0, -30.0, 158.0),
            v3(150.0, 0.0, 158.0),
            v3(96.0, 30.0, 158.0),
        ];
        b.loft(&[foot.to_vec(), top.to_vec()], false, true);
        // A jaw: a three-sided spike from its hinge to the cable.
        b.with_part(part::ELEVATOR_ARM, |b| {
            b.paint(PLATING_DARK);
            let root = vec![
                v3(HINGE_R + 6.0, -10.0, HINGE_Z),
                v3(HINGE_R + 6.0, 10.0, HINGE_Z),
                v3(HINGE_R - 10.0, 0.0, HINGE_Z + 8.0),
            ];
            b.loft(&[root, vec![v3(CABLE_R, 0.0, GRIP_Z); 3]], false, false);
        });
    });
    b.paint(TEAM);
    b.decal(v3(BEACON_R, 0.0, BEACON_Z - 3.8), v2(24.0, 24.0));
    b.with_part(part::ELEVATOR_CABLE, |b| {
        b.paint(METAL).pattern(pattern::PLAIN);
        let ring = |z: f32| disc(3, CABLE_R, z);
        b.loft(&[ring(CABLE_FOOT), ring(CABLE_TOP)], false, false);
    });
    b.yawed(Vec3::ZERO, BAY_BEARING_DEG.to_radians(), |b| {
        b.radial(3, |b| {
            b.with_part(part::ELEVATOR_CRATE, |b| {
                b.paint(PLATING_DARK);
                let (x0, x1, hw) = (BAY_FROM, BAY_FROM + BAY_LENGTH, BAY_WIDTH / 2.0);
                // A wedge standing for the box: three corners, a flat top.
                let foot = [v3(x0, -hw, 0.3), v3(x1, 0.0, 0.3), v3(x0, hw, 0.3)];
                let top = foot.map(|p| v3(p.x, p.y, BAY_HEIGHT));
                b.loft(&[foot.to_vec(), top.to_vec()], false, false);
            });
        });
    });
}

// ---- the foundation ---------------------------------------------------------------

/// The slab over the whole lot, the well's floor and its rim, and the power yards on
/// the lot's corners.
fn foundation(b: &mut MeshBuilder) {
    let plan = chamfered_rect(v2(SLAB, SLAB), SLAB_CUT);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.loft_z(&plan, &[Section::new(0.0, 1.0), Section::new(0.35, 1.0)]);
    b.paint(PLATING_DARK);
    b.loft_z(
        &plan,
        &[Section::new(0.3, 0.995), Section::new(SLAB_TOP, 0.985)],
    );
    // The well's floor: dark, ringed in hazard paint, the docking rails across it. All
    // of it lies inside the docked deck, so it shows only while the platform is away.
    let sides = b.sides(36);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.face(&disc(sides, WELL_R, SLAB_TOP + 0.04));
    b.paint(PLATING).pattern(pattern::HAZARD);
    annulus(
        b,
        sides,
        WELL_R - 5.0,
        WELL_R - 0.2,
        SLAB_TOP,
        SLAB_TOP + 0.1,
        0.0,
        TAU,
    );
    // The rim: an armoured kerb round the well, broken at the three ways out.
    let arcs = b.sides(10);
    b.radial(3, |b| {
        let (a0, a1) = (-TAU / 6.0 + EXIT_HALF, TAU / 6.0 - EXIT_HALF);
        b.paint(PLATING);
        annulus(b, arcs, WELL_R, RIM_R, SLAB_TOP - 0.1, RIM_TOP, a0, a1);
        b.paint(PLATING).pattern(pattern::HAZARD);
        annulus(
            b,
            arcs,
            WELL_R - 0.3,
            WELL_R + 1.6,
            RIM_TOP - 0.1,
            RIM_TOP + 0.15,
            a0,
            a1,
        );
    });
    if b.mid() {
        for (x, y) in [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
            let at = v3(x * 128.0, y * 128.0, SLAB_TOP);
            power_yard(b, at, (-y).atan2(-x));
        }
    }
}

/// A power yard on a lot corner, facing the middle: a transformer house on a plinth,
/// a bank of insulator posts and the cable run toward the well.
fn power_yard(b: &mut MeshBuilder, at: Vec3, yaw: f32) {
    let fine = b.fine();
    b.yawed(at, yaw, |b| {
        b.paint(ACCENT).pattern(pattern::PLAIN);
        b.block(v3(-14.0, -12.0, 0.0), v3(12.0, 12.0, 1.0));
        b.paint(PLATING);
        b.chamfered_box(v3(-5.0, 0.0, 6.0), v3(14.0, 18.0, 10.0), 2.0);
        team_panel(b, v3(-5.0, 0.0, 11.0), v2(8.0, 10.0));
        b.paint(PLATING_DARK);
        for y in [-7.5f32, 7.5] {
            b.chamfered_box(v3(6.5, y, 4.5), v3(7.0, 5.0, 7.0), 1.0);
        }
        if !fine {
            return;
        }
        let sides = b.sides(8);
        for y in [-7.5f32, 0.0, 7.5] {
            b.paint(ACCENT).pattern(pattern::PLAIN);
            b.prism(v3(1.0, y, 1.0), sides, 0.9, 0.7, 5.0);
            b.paint(METAL).pattern(pattern::PLAIN);
            for k in 0..4 {
                b.prism(v3(1.0, y, 2.4 + 0.9 * k as f32), sides, 1.4, 1.4, 0.35);
            }
        }
        b.paint(METAL).pattern(pattern::PLAIN);
        for y in [-1.2f32, 1.2] {
            b.cylinder_between(v3(12.0, y, 1.3), v3(26.0, y, 1.3), 0.7, 0.7, 6);
        }
        b.paint(GLOW_RED);
        b.cuboid(v3(-5.0, 0.0, 11.6), v3(1.0, 1.0, 1.0));
    });
}

/// One way out, along +x of its frame: a marked apron from the well to the lot edge,
/// kerbed, lamps along both kerbs, a hazard sill at the well.
fn apron(b: &mut MeshBuilder) {
    let fine = b.fine();
    let plan = [
        [WELL_R - 1.0, -APRON_IN],
        [APRON_END, -APRON_OUT],
        [APRON_END, APRON_OUT],
        [WELL_R - 1.0, APRON_IN],
    ];
    b.paint(PLATING).pattern(pattern::ROADWAY);
    b.loft_z(
        &plan,
        &[
            Section::new(SLAB_TOP - 0.1, 1.0),
            Section::new(APRON_TOP, 1.0),
        ],
    );
    b.paint(PLATING).pattern(pattern::HAZARD);
    b.block(
        v3(WELL_R - 1.0, -APRON_IN, APRON_TOP - 0.2),
        v3(WELL_R + 3.0, APRON_IN, APRON_TOP + 0.08),
    );
    b.mirror_y(|b| {
        b.paint(ACCENT);
        b.beam(
            v3(WELL_R + 4.0, APRON_IN + 1.5, SLAB_TOP + 1.25),
            v3(APRON_END - 2.0, APRON_OUT + 1.5, SLAB_TOP + 1.25),
            v2(2.2, 2.4),
            v2(2.2, 2.4),
        );
        if !fine {
            return;
        }
        // Lamp posts along the kerb, lit amber toward the road.
        for k in 0..4 {
            let t = (k as f32 + 0.5) / 4.0;
            let x = WELL_R + 8.0 + t * (APRON_END - WELL_R - 14.0);
            let y = APRON_IN + 1.5 + (APRON_OUT - APRON_IN) * (x - WELL_R) / (APRON_END - WELL_R);
            b.paint(PLATING_DARK);
            b.block(v3(x - 0.6, y - 0.6, SLAB_TOP), v3(x + 0.6, y + 0.6, 7.0));
            b.block(v3(x - 1.0, y - 2.4, 7.0), v3(x + 1.0, y + 0.6, 7.8));
            b.paint(GLOW_LAMP);
            b.block(v3(x - 0.7, y - 2.2, 6.85), v3(x + 0.7, y - 0.8, 7.0));
        }
    });
}

/// The warning beacon on a tower's crown: a short mast and a red lamp at
/// (`BEACON_R`, `BEACON_Z`).
fn beacon(b: &mut MeshBuilder) {
    let sides = b.sides(8);
    b.paint(PLATING_DARK);
    b.prism(v3(BEACON_R, 0.0, BEACON_Z - 6.0), sides, 2.2, 1.6, 4.6);
    b.paint(GLOW_RED);
    b.prism(v3(BEACON_R, 0.0, BEACON_Z - 1.4), sides, 1.4, 1.4, 2.8);
    b.paint(ACCENT).pattern(pattern::PLAIN);
    b.prism(v3(BEACON_R, 0.0, BEACON_Z + 1.4), sides, 1.8, 0.6, 1.0);
}

// ---- the cable --------------------------------------------------------------------

/// The cable (`part::ELEVATOR_CABLE`): the coupling at its foot that seats in the well,
/// the core, the three guide rails the platform's rollers run on (on the bearings of the
/// ways out, between the jaws), and the grip band the jaws close on.
fn cable(b: &mut MeshBuilder) {
    b.with_part(part::ELEVATOR_CABLE, |b| {
        let fine = b.fine();
        let sides = b.sides(16);
        // The coupling: a flared foot plate, a tapering armoured body, latch lugs.
        b.paint(PLATING_DARK);
        b.prism(v3(0.0, 0.0, CABLE_FOOT), sides, COUPLING_R, COUPLING_R, 2.4);
        b.paint(PLATING);
        b.prism(
            v3(0.0, 0.0, CABLE_FOOT + 2.4),
            sides,
            COUPLING_R - 0.6,
            CORE_R + 1.8,
            COUPLING_TOP - CABLE_FOOT - 2.4,
        );
        b.paint(PLATING).pattern(pattern::HAZARD);
        b.prism(
            v3(0.0, 0.0, CABLE_FOOT + 2.4),
            sides,
            COUPLING_R - 0.3,
            COUPLING_R - 0.4,
            1.4,
        );
        b.radial(3, |b| {
            b.paint(ACCENT);
            b.block(
                v3(COUPLING_R - 3.0, -1.4, CABLE_FOOT + 1.0),
                v3(COUPLING_R - 0.2, 1.4, CABLE_FOOT + 9.0),
            );
        });
        // The core, up into orbit, and its guide rails.
        b.paint(METAL).pattern(pattern::PLAIN);
        b.prism(
            v3(0.0, 0.0, COUPLING_TOP - 1.0),
            sides,
            CORE_R,
            CORE_R,
            CABLE_TOP - COUPLING_TOP + 1.0,
        );
        b.yawed(Vec3::ZERO, TAU / 6.0, |b| {
            b.radial(3, |b| {
                b.paint(PLATING_DARK);
                b.block(
                    v3(CORE_R - 0.6, -1.1, COUPLING_TOP - 2.0),
                    v3(CORE_R + 1.6, 1.1, CABLE_TOP),
                );
            });
        });
        // The grip band: a heavy sleeve on the cable where the jaws close, collared.
        b.paint(PLATING_DARK);
        b.prism(
            v3(0.0, 0.0, GRIP_Z - BAND_HALF),
            sides,
            CABLE_R,
            CABLE_R,
            2.0 * BAND_HALF,
        );
        b.paint(PLATING);
        for z in [GRIP_Z - BAND_HALF - 2.0, GRIP_Z + BAND_HALF] {
            b.prism(v3(0.0, 0.0, z), sides, CABLE_R + 0.9, CABLE_R + 0.9, 2.0);
        }
        if fine {
            // Splice collars up the cable, and between them the band's ribs.
            b.paint(PLATING).pattern(pattern::PLAIN);
            for z in [60.0f32, 110.0, 160.0, 262.0] {
                b.prism(v3(0.0, 0.0, z), sides, CORE_R + 1.9, CORE_R + 1.9, 3.0);
                b.paint(ACCENT).pattern(pattern::PLAIN);
                b.prism(
                    v3(0.0, 0.0, z + 3.0),
                    sides,
                    CORE_R + 1.9,
                    CORE_R + 1.0,
                    1.0,
                );
                b.paint(PLATING).pattern(pattern::PLAIN);
            }
        }
    });
}

// ---- the platform -----------------------------------------------------------------

/// The landing pad (`part::HULL`): a heavy armoured plate round the cable's guide sleeve
/// with a hazard edge, flush with the well's rim, and the owner's colour and lane lines
/// on it. The crates land over it.
fn landing_pad(b: &mut MeshBuilder) {
    deck(b);
    sleeve(b);
}

fn deck(b: &mut MeshBuilder) {
    let fine = b.fine();
    let sides = b.sides(24);
    // The deck plate: a ring round the sleeve.
    b.paint(ACCENT).pattern(pattern::DECK);
    annulus(
        b,
        sides,
        SLEEVE_OUT - 0.5,
        DECK_R - 3.0,
        DECK_FOOT,
        DECK_Z,
        0.0,
        TAU,
    );
    b.paint(PLATING).pattern(pattern::HAZARD);
    annulus(
        b,
        sides,
        DECK_R - 3.2,
        DECK_R,
        DECK_FOOT,
        DECK_Z + 0.05,
        0.0,
        TAU,
    );
    // The owner's colour in three bands across the deck, by the towers.
    b.radial(3, |b| {
        b.paint(TEAM).pattern(pattern::PLAIN);
        b.decal(v3(38.0, 0.0, DECK_Z + 0.03), v2(10.0, 22.0));
        if !fine {
            return;
        }
        // Lane lines toward the ways out.
        b.yawed(Vec3::ZERO, TAU / 6.0, |b| {
            b.mirror_y(|b| {
                b.paint(PLATING).pattern(pattern::HAZARD);
                b.decal(v3(40.0, 18.0, DECK_Z + 0.03), v2(30.0, 1.4));
            });
        });
    });
    if !fine {
        return;
    }
    // Flush edge lamps round the rim.
    b.radial(12, |b| {
        b.paint(GLOW_AMBER);
        b.decal(v3(DECK_R - 1.6, 0.0, DECK_Z + 0.08), v2(1.2, 3.0));
    });
}

/// The guide sleeve round the cable: a heavy collar rising from the deck, three
/// buttresses toward the towers, roller housings on the rails' bearings at its head.
fn sleeve(b: &mut MeshBuilder) {
    let fine = b.fine();
    let sides = b.sides(20);
    b.paint(PLATING);
    sweep_ring(
        b,
        sides,
        &[
            (SLEEVE_IN, DECK_FOOT),
            (SLEEVE_OUT + 4.0, DECK_FOOT),
            (SLEEVE_OUT + 4.0, DECK_Z + 3.0),
            (SLEEVE_OUT, DECK_Z + 8.0),
            (SLEEVE_OUT, SLEEVE_TOP - 3.0),
            (SLEEVE_OUT - 1.5, SLEEVE_TOP),
            (SLEEVE_IN, SLEEVE_TOP),
        ],
    );
    b.paint(PLATING).pattern(pattern::HAZARD);
    annulus(
        b,
        sides,
        SLEEVE_OUT + 3.6,
        SLEEVE_OUT + 4.3,
        DECK_Z + 2.4,
        DECK_Z + 3.2,
        0.0,
        TAU,
    );
    b.radial(3, |b| {
        b.paint(PLATING_DARK);
        b.extrude_y(
            &[
                [SLEEVE_OUT - 0.5, DECK_Z],
                [SLEEVE_OUT + 10.0, DECK_Z],
                [SLEEVE_OUT + 10.0, DECK_Z + 2.0],
                [SLEEVE_OUT - 0.5, SLEEVE_TOP - 6.0],
            ],
            -2.0,
            2.0,
        );
    });
    if !fine {
        return;
    }
    b.yawed(Vec3::ZERO, TAU / 6.0, |b| {
        b.radial(3, |b| {
            b.paint(PLATING_DARK);
            b.chamfered_box(
                v3(SLEEVE_OUT + 1.6, 0.0, SLEEVE_TOP - 4.0),
                v3(5.0, 7.0, 7.0),
                1.4,
            );
            b.paint(METAL).pattern(pattern::PLAIN);
            b.cylinder_between(
                v3(SLEEVE_IN - 0.3, -2.4, SLEEVE_TOP - 3.0),
                v3(SLEEVE_IN - 0.3, 2.4, SLEEVE_TOP - 3.0),
                1.6,
                1.6,
                10,
            );
            b.paint(GLOW_LAMP);
            b.block(
                v3(SLEEVE_OUT + 4.05, -1.6, SLEEVE_TOP - 5.0),
                v3(SLEEVE_OUT + 4.2, 1.6, SLEEVE_TOP - 3.4),
            );
        });
    });
}

// ---- the crates -------------------------------------------------------------------

/// A crate's frame: x out along its bay's bearing, y to the left, z up from the foot.
const WALL_FOOT: f32 = 0.3;
/// The door leaves' and roof halves' thickness.
const LEAF: f32 = 3.0;
const ROOF_T: f32 = 2.5;
/// A crate's ribs: how far they stand off the walls, and their pitch.
const RIB: f32 = 2.0;
const RIB_PITCH: f32 = 20.0;

/// The three cargo crates, one in each way out, landed: each its own shell, door leaves
/// and roof halves (the shader swings and sinks them by the crates' height, see
/// `gpu_consts::elevator`).
fn crates(b: &mut MeshBuilder) {
    b.yawed(Vec3::ZERO, BAY_BEARING_DEG.to_radians(), |b| {
        b.radial(3, |b| {
            b.with_part(part::ELEVATOR_CRATE, shell);
            b.with_part(part::ELEVATOR_DOOR, doors);
            b.with_part(part::ELEVATOR_ROOF, roof);
        });
    });
}

/// The bay's outer end (where the doors are) and half its width.
fn bay_end() -> f32 {
    BAY_FROM + BAY_LENGTH
}

/// The shell: an end wall at the cable side, two side walls with ribs and belts, door
/// posts, a hazard sill, and a braking thruster bell under each corner.
fn shell(b: &mut MeshBuilder) {
    let fine = b.fine();
    let (x0, x1) = (BAY_FROM, bay_end());
    let (hw, t, top) = (BAY_WIDTH / 2.0, BAY_WALL, BAY_HEIGHT - ROOF_T);
    b.paint(PLATING);
    b.block(v3(x0, -hw, WALL_FOOT), v3(x0 + t, hw, top));
    b.mirror_y(|b| {
        b.paint(PLATING);
        b.block(v3(x0, hw - t, WALL_FOOT), v3(x1, hw, top));
        // Belts round the wall, a dark foot rail and a hazard-marked sill.
        b.paint(ACCENT).pattern(pattern::PLAIN);
        for z in [22.0f32, 46.0] {
            b.block(v3(x0, hw, z), v3(x1, hw + 1.0, z + 3.0));
        }
        b.paint(PLATING_DARK);
        b.block(v3(x0, hw, WALL_FOOT), v3(x1, hw + 2.5, WALL_FOOT + 5.0));
        b.block(v3(x0, hw, top - 4.0), v3(x1, hw + 2.5, top));
        // Door post: a heavy dark column the leaf hangs from.
        b.block(
            v3(x1 - 7.0, hw - 4.0, WALL_FOOT),
            v3(x1 + 1.5, hw + 3.0, top),
        );
        if fine {
            // Vertical ribs between the belts.
            b.paint(PLATING_DARK);
            let mut x = x0 + RIB_PITCH * 0.5;
            while x < x1 - 12.0 {
                b.block(v3(x, hw, WALL_FOOT + 5.0), v3(x + 4.0, hw + RIB, top - 4.0));
                x += RIB_PITCH;
            }
        }
    });
    // The inner end wall's ribs and a cargo-number block on it.
    if fine {
        b.paint(PLATING_DARK);
        let mut y = -hw + 12.0;
        while y < hw - 8.0 {
            b.block(v3(x0 - RIB, y, WALL_FOOT + 5.0), v3(x0, y + 4.0, top - 4.0));
            y += RIB_PITCH;
        }
    }
    // The braking thrusters under the corners: a housing in the wall's foot, a bell
    // opening down at `CAR_UNDER_Z`.
    let bells = b.sides(12);
    for x in [x0 + THRUSTER_INSET, x1 - THRUSTER_INSET] {
        b.mirror_y(|b| {
            let y = hw - THRUSTER_INSET;
            b.paint(PLATING);
            b.chamfered_box(v3(x, y, 3.0), v3(12.0, 12.0, 8.0), 2.0);
            b.paint(METAL).pattern(pattern::PLAIN);
            b.prism(v3(x, y, CAR_UNDER_Z), bells, 5.2, 3.2, 3.0 - CAR_UNDER_Z);
            if fine {
                b.paint(ACCENT).pattern(pattern::PLAIN);
                b.prism(v3(x, y, CAR_UNDER_Z + 0.01), bells, 4.0, 4.0, 0.05);
            }
        });
    }
}

/// The door leaves, shut across the outer end: two heavy plates hung from the posts,
/// meeting in the middle, braced across their outer faces, hazard bands at the foot.
fn doors(b: &mut MeshBuilder) {
    let fine = b.fine();
    let x1 = bay_end();
    let (hw, top) = (BAY_WIDTH / 2.0, BAY_HEIGHT - ROOF_T);
    b.mirror_y(|b| {
        b.paint(PLATING);
        b.block(v3(x1 - LEAF, 1.0, WALL_FOOT), v3(x1, hw - 3.0, top));
        b.paint(PLATING).pattern(pattern::HAZARD);
        b.block(
            v3(x1 - LEAF - 0.1, 1.0, WALL_FOOT),
            v3(x1 + 0.1, hw - 3.0, WALL_FOOT + 6.0),
        );
        b.paint(ACCENT).pattern(pattern::PLAIN);
        for z in [22.0f32, 46.0] {
            b.block(v3(x1, 1.0, z), v3(x1 + 1.0, hw - 3.0, z + 3.0));
        }
        if fine {
            b.paint(PLATING_DARK);
            for y in [10.0f32, 24.0] {
                b.block(v3(x1, y, WALL_FOOT + 6.0), v3(x1 + RIB, y + 4.0, top - 4.0));
            }
            // The lock bars where the leaves meet.
            b.paint(METAL).pattern(pattern::PLAIN);
            for z in [14.0f32, 36.0, 58.0] {
                b.block(v3(x1 + 0.2, 0.0, z), v3(x1 + 1.6, 6.0, z + 2.0));
            }
        }
    });
}

/// The roof halves, shut: a plate over each side, hinged at the side wall's top, the
/// owner's colour on each, ribs across.
fn roof(b: &mut MeshBuilder) {
    let fine = b.fine();
    let (x0, x1) = (BAY_FROM, bay_end());
    let hw = BAY_WIDTH / 2.0;
    let z = BAY_HEIGHT - ROOF_T;
    b.mirror_y(|b| {
        b.paint(PLATING);
        b.block(v3(x0, 0.4, z), v3(x1, hw, BAY_HEIGHT));
        b.paint(TEAM).pattern(pattern::PLAIN);
        b.decal(
            v3((x0 + x1) * 0.5, hw * 0.5, BAY_HEIGHT + 0.04),
            v2(34.0, 22.0),
        );
        if fine {
            b.paint(PLATING_DARK);
            let mut x = x0 + 8.0;
            while x < x1 - 8.0 {
                b.block(v3(x, 0.4, BAY_HEIGHT), v3(x + 3.0, hw, BAY_HEIGHT + 1.2));
                x += RIB_PITCH * 1.5;
            }
        }
    });
}

// ---- kit --------------------------------------------------------------------------

/// An n-gon disc at height `z`, wound to face up.
fn disc(sides: usize, r: f32, z: f32) -> Vec<Vec3> {
    ngon(sides, r).iter().map(|p| v3(p[0], p[1], z)).collect()
}

/// A flat ring round the z axis, `r0` to `r1` out, `z0` to `z1` up, from angle `a0` to
/// `a1` in `sides` steps a whole turn (an arc short of a turn is closed at its ends).
#[expect(
    clippy::too_many_arguments,
    reason = "a ring's plan and section, and its arc"
)]
fn annulus(
    b: &mut MeshBuilder,
    sides: usize,
    r0: f32,
    r1: f32,
    z0: f32,
    z1: f32,
    a0: f32,
    a1: f32,
) {
    let whole = (a1 - a0 - TAU).abs() < 1e-4;
    let steps = if whole {
        sides
    } else {
        ((sides as f32 * (a1 - a0) / TAU).ceil() as usize).max(1)
    };
    let section = [(r0, z0), (r1, z0), (r1, z1), (r0, z1)];
    let rings: Vec<Vec<Vec3>> = (0..=steps)
        .map(|i| {
            let a = a0 + (a1 - a0) * i as f32 / steps as f32;
            let (s, c) = a.sin_cos();
            section.iter().map(|&(r, z)| v3(c * r, s * r, z)).collect()
        })
        .collect();
    b.loft(&rings, !whole, !whole);
}

/// A closed section (radius, height) swept a whole turn round the z axis.
fn sweep_ring(b: &mut MeshBuilder, sides: usize, section: &[(f32, f32)]) {
    let rings: Vec<Vec<Vec3>> = (0..=sides)
        .map(|i| {
            let (s, c) = (TAU * i as f32 / sides as f32).sin_cos();
            section.iter().map(|&(r, z)| v3(c * r, s * r, z)).collect()
        })
        .collect();
    b.loft(&rings, false, false);
}

/// A box girder from `a` to `b` in a plane square to `side`: its section a chamfered
/// rectangle, `half_a` and `half_b` (across along `side`, deep in the plane) at its ends.
fn girder(b: &mut MeshBuilder, a: Vec3, z: Vec3, half_a: Vec2, half_b: Vec2, cut: f32) {
    let axis = (z - a).normalize();
    let side = Vec3::Y;
    let up = side.cross(axis).normalize();
    let ring = |c: Vec3, half: Vec2| -> Vec<Vec3> {
        chamfered_rect(half, cut)
            .iter()
            .map(|p| c + side * p[0] + up * p[1])
            .collect()
    };
    b.loft(&[ring(a, half_a), ring(z, half_b)], true, true);
}

/// A hydraulic ram from `foot` to `head`: the barrel over the first `share` of it, the
/// polished rod out of it, and an eye at each end across y.
fn ram(b: &mut MeshBuilder, foot: Vec3, head: Vec3, barrel: f32, share: f32) {
    let sides = b.sides(12);
    let split = foot + (head - foot) * share;
    b.paint(PLATING_DARK);
    b.cylinder_between(foot, split, barrel, barrel, sides);
    b.paint(PLATING);
    b.cylinder_between(
        split - (head - foot).normalize() * 1.2,
        split,
        barrel * 1.15,
        barrel * 1.15,
        sides,
    );
    b.paint(METAL).pattern(pattern::PLAIN);
    b.cylinder_between(split, head, barrel * 0.55, barrel * 0.55, sides);
    if b.fine() {
        b.paint(ACCENT);
        for p in [foot, head] {
            b.cylinder_between(
                p - Vec3::Y * barrel * 0.8,
                p + Vec3::Y * barrel * 0.8,
                barrel * 0.9,
                barrel * 0.9,
                sides,
            );
        }
    }
}

/// A floodlight: a dark housing at `at`, its lit face toward `toward`.
fn floodlight(b: &mut MeshBuilder, at: Vec3, toward: Vec3, size: f32) {
    let d = (toward - at).normalize();
    b.paint(PLATING_DARK);
    b.beam(
        at - d * size,
        at,
        v2(size * 1.6, size),
        v2(size * 1.8, size * 1.2),
    );
    b.paint(GLOW_LAMP);
    b.beam(
        at,
        at + d * 0.15,
        v2(size * 1.5, size * 0.95),
        v2(size * 1.5, size * 0.95),
    );
}

/// The grip pads on a jaw's head: a concave shoe that closes on the cable's band, its
/// face at `CABLE_R` across ±`half_angle` of the jaw's bearing, `half_z` up and down
/// from `GRIP_Z`, backed to `back` out.
fn grip_pad(b: &mut MeshBuilder, back: f32, half_angle: f32, half_z: f32) {
    let steps = if b.fine() { 6 } else { 3 };
    let r = CABLE_R + 0.05;
    let mut plan: Vec<[f32; 2]> = vec![
        [back, -r * half_angle.sin() - 1.0],
        [back, r * half_angle.sin() + 1.0],
    ];
    for i in 0..=steps {
        let a = half_angle - 2.0 * half_angle * i as f32 / steps as f32;
        plan.push([r * a.cos(), r * a.sin()]);
    }
    b.paint(TREAD).pattern(pattern::NONE);
    b.loft_z(
        &plan,
        &[
            Section::new(GRIP_Z - half_z, 1.0),
            Section::new(GRIP_Z + half_z, 1.0),
        ],
    );
}

#[cfg(test)]
mod tests;
