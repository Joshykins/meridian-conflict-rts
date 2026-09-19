//! Aster ground vehicles: wheeled, tracked and hover chassis.
//!
//! Shared anatomy: a dark recessed chassis between the running gear, a white
//! faceted shell over it, and (for armed units) a `TURRET` assembly yawing
//! about the model's z axis, because the sim rotates muzzle offsets about the
//! unit origin.

use glam::Vec3;

use super::parts::*;
use crate::models::builder::{chamfered_rect, MeshBuilder, Section};
use crate::models::material::*;
use crate::models::part;

// ---- Mason: engineer -------------------------------------------------------

pub fn engineer(b: &mut MeshBuilder, tech: u8) {
    let deck = tracked_chassis(b, &Chassis { rear: -3.0, front: 3.1, track: (1.4, 2.5, 1.1), split_tracks: false, deck: 1.95 });
    if b.coarse() {
        b.paint(ACCENT);
        b.beam(v3(-0.4, 0.0, 2.0), v3(2.6, 0.0, 3.0), v2(0.7, 0.6), v2(0.9, 0.8));
        team_panel(b, deck.at(0.15, 0.0), v2(1.2, 2.0));
        return;
    }
    // Sensor canopy over the glacis, resource vats on the rear deck.
    b.paint(GLASS);
    b.frustum(deck.at(0.86, 0.0), v2(1.1, 1.5), v2(0.45, 1.0), 0.45, v2(-0.25, 0.0));
    b.mirror_y(|b| {
        b.paint(METAL);
        b.cylinder_between(v3(-2.75, 0.62, 2.2), v3(-1.15, 0.62, 2.2), 0.5, 0.5, b.sides(8));
        if b.fine() {
            b.paint(ACCENT);
            b.cylinder_between(v3(-2.3, 0.62, 2.2), v3(-2.1, 0.62, 2.2), 0.56, 0.56, 8);
            b.cylinder_between(v3(-1.8, 0.62, 2.2), v3(-1.6, 0.62, 2.2), 0.56, 0.56, 8);
        }
    });
    team_panel(b, v3(-0.55, 0.0, 1.95), v2(0.8, 2.2));

    // Construction arms: one at tech 1, a pair from tech 2, plus a mast emitter at tech 3.
    let arm = |b: &mut MeshBuilder, y: f32, reach: f32| {
        let (shoulder, elbow, head) = (v3(0.3, y, 2.0), v3(1.0 + reach * 0.3, y, 3.05), v3(2.3 + reach, y, 2.8));
        b.paint(ACCENT);
        b.prism(v3(0.3, y, 1.9), 6, 0.55, 0.45, 0.3);
        b.beam(shoulder, elbow, v2(0.4, 0.46), v2(0.34, 0.38));
        b.paint(PLATING);
        b.beam(elbow, head, v2(0.46, 0.48), v2(0.56, 0.56));
        b.paint(GLOW);
        b.cylinder_between(head, head + v3(0.5, 0.0, -0.13), 0.34, 0.13, 6);
        if b.fine() {
            b.paint(METAL);
            b.cylinder_between(shoulder + v3(0.4, 0.0, -0.05), elbow + v3(0.3, 0.0, -0.32), 0.07, 0.07, 4);
            b.paint(ACCENT);
            b.cylinder_between(elbow - Vec3::Y * 0.3, elbow + Vec3::Y * 0.3, 0.27, 0.27, 6);
        }
    };
    if tech == 1 {
        arm(b, 0.0, 0.0);
    } else {
        arm(b, 0.8, 0.15);
        arm(b, -0.8, 0.15);
    }
    if tech >= 3 {
        b.paint(PLATING);
        b.prism(v3(-0.55, 0.0, 1.95), 6, 0.45, 0.3, 0.9);
        b.paint(GLOW);
        b.prism(v3(-0.55, 0.0, 2.85), 6, 0.34, 0.1, 0.5);
    }
    if b.fine() {
        antenna(b, v3(-2.6, -1.25, 1.95), 1.4, 0.15);
        if tech >= 2 {
            antenna(b, v3(-2.6, 1.25, 1.95), 1.1, 0.15);
            b.mirror_y(|b| glow_strip(b, deck.at(0.55, 0.88), v2(1.6, 0.1), GLOW));
        }
        if tech >= 3 {
            b.mirror_y(|b| glow_strip(b, v3(-1.95, 0.62, 2.68), v2(1.2, 0.12), GLOW));
        }
    }
}

// ---- Kestrel: scout --------------------------------------------------------

pub fn scout(b: &mut MeshBuilder, _tech: u8) {
    b.set_turret_pivot(v3(0.0, 0.0, 1.25));
    let body = [[-2.1, 0.55], [1.7, 0.42], [2.3, 0.7], [0.7, 1.22], [-1.5, 1.3], [-2.2, 0.95]];
    if b.coarse() {
        b.mirror_y(|b| b.with_part(part::LOCOMOTION, |b| b.paint(TREAD).cuboid_open(v3(0.0, 1.25, 0.5), v3(3.8, 0.5, 1.0))));
        b.paint(PLATING);
        b.frustum_open(v3(0.0, 0.0, 0.45), v2(4.4, 1.9), v2(2.2, 1.3), 0.85, v2(-0.5, 0.0));
        team_panel(b, v3(-1.25, 0.0, 1.3), v2(0.55, 1.2));
        b.with_part(part::TURRET, |b| {
            b.paint(PLATING);
            b.frustum_open(v3(0.0, 0.0, 1.25), v2(1.2, 0.9), v2(0.7, 0.55), 0.53, v2(-0.08, 0.0));
            b.paint(METAL);
            b.beam(v3(0.4, 0.0, 1.6), v3(1.2, 0.0, 1.6), v2(0.2, 0.16), v2(0.16, 0.12));
        });
        return;
    }
    b.mirror_y(|b| {
        wheel(b, v3(1.3, 1.25, 0.55), 0.55, 0.5);
        wheel(b, v3(-1.35, 1.25, 0.55), 0.55, 0.5);
    });
    b.paint(PLATING);
    b.extrude_y_chamfered(&body, 0.98, 0.3);
    b.paint(ACCENT);
    b.block(v3(-1.7, -1.05, 0.35), v3(1.7, 1.05, 0.75));
    team_panel(b, v3(-1.25, 0.0, 1.28), v2(0.55, 1.2));

    b.with_part(part::TURRET, |b| {
        b.paint(PLATING);
        b.loft_z(&turret_plan(1.3, 1.0), &[Section::new(1.25, 0.9), Section::new(1.45, 1.0), Section::scaled(1.78, 0.6, 0.6).shifted(-0.08, 0.0)]);
        b.paint(METAL);
        b.beam(v3(0.4, 0.0, 1.6), v3(1.2, 0.0, 1.6), v2(0.2, 0.16), v2(0.16, 0.12));
        b.paint(GLOW);
        b.cuboid(v3(1.2, 0.0, 1.6), v3(0.06, 0.1, 0.08));
        if b.fine() {
            b.block(v3(0.55, -0.04, 1.68), v3(1.05, 0.04, 1.71));
        }
    });
    if b.fine() {
        on_slope(b, [2.3, 0.7], [0.7, 1.22], 0.45, |b| {
            b.paint(GLASS);
            b.plate(Vec3::ZERO, v2(0.7, 1.1), 0.06, 0.03);
        });
        on_slope(b, [2.3, 0.7], [0.7, 1.22], 0.88, |b| glow_strip(b, Vec3::ZERO, v2(0.12, 1.2), GLOW));
        antenna(b, v3(-1.9, 0.6, 1.2), 1.0, 0.25);
        b.mirror_y(|b| {
            b.paint(ACCENT);
            b.block(v3(-0.5, 0.98, 0.8), v3(0.45, 1.5, 0.95));
        });
    }
}

// ---- Warden: medium tank ---------------------------------------------------

pub fn tank_medium(b: &mut MeshBuilder, _tech: u8) {
    let deck = tracked_chassis(b, &Chassis { rear: -4.2, front: 4.3, track: (1.85, 3.2, 1.4), split_tracks: false, deck: 2.15 });

    b.set_turret_pivot(v3(0.0, 0.0, 2.15));
    b.with_part(part::TURRET, |b| {
        b.paint(PLATING);
        let roof = turret_shell(b, 4.7, 3.4, 2.2, 3.55);
        rail_gun(b, v3(2.0, 0.0, 3.0), v3(5.2, 0.0, 3.0), v2(0.22, 0.36), 0.22, Emitter::Blue);
        team_panel(b, roof.at(0.2, 0.0), v2(roof.length() * 0.36, roof.half_width * 1.7));
        if b.coarse() {
            return;
        }
        b.paint(ACCENT);
        b.prism(v3(0.0, 0.0, 2.05), 8, 1.6, 1.6, 0.2);
        b.block(v3(1.75, -0.75, 2.62), v3(2.3, 0.75, 3.38));
        if b.fine() {
            b.paint(ACCENT);
            b.prism(roof.at(0.68, 0.45), 8, 0.4, 0.33, 0.12);
            b.paint(GLASS);
            b.frustum(roof.at(0.8, -0.5), v2(0.55, 0.45), v2(0.32, 0.32), 0.26, v2(-0.05, 0.0));
            antenna(b, roof.at(-0.12, -0.5), 0.6, 0.3);
        }
    });

    if b.mid() {
        // Dark engine deck with reactor vents.
        b.paint(ACCENT);
        b.plate(deck.at(0.13, 0.0), v2(deck.length() * 0.24, deck.half_width * 1.8), 0.06, 0.03);
    }
    if b.fine() {
        b.mirror_y(|b| vent(b, deck.at(0.13, 0.5) + Vec3::Z * 0.06, v2(1.1, 0.7), 3, GLOW));
    }
}

// ---- Ballista: light artillery ---------------------------------------------

pub fn artillery_light(b: &mut MeshBuilder, _tech: u8) {
    let deck = tracked_chassis(b, &Chassis { rear: -3.8, front: 3.8, track: (1.65, 2.85, 1.2), split_tracks: false, deck: 1.7 });

    // Open gun mount: trunnion cheeks, splinter shield, long elevated tube.
    let (breech, muzzle) = (v3(-1.3, 0.0, 1.97), v3(3.8, 0.0, 3.4));
    b.set_turret_pivot(v3(0.0, 0.0, 1.7));
    b.with_part(part::TURRET, |b| {
        cannon(b, breech, muzzle, 0.22, Emitter::Orange);
        b.paint(PLATING);
        if b.coarse() {
            b.frustum_open(v3(-0.7, 0.0, 1.7), v2(2.6, 2.4), v2(1.6, 1.7), 1.0, v2(-0.2, 0.0));
            team_panel(b, v3(-0.9, 0.0, 2.7), v2(1.6, 0.6));
            return;
        }
        b.mirror_y(|b| b.extrude_y(&[[-1.9, 1.7], [0.7, 1.7], [0.3, 2.75], [-1.2, 2.9], [-1.9, 2.4]], 0.55, 0.85));
        b.pitched(v3(0.35, 0.0, 1.75), -1.15, |b| b.mirror_y(|b| b.block(v3(-1.35, 0.3, 0.0), v3(0.0, 1.25, 0.12))));
        b.paint(ACCENT);
        b.prism(v3(-0.4, 0.0, 1.62), 8, 1.4, 1.4, 0.16);
        b.block(v3(-2.2, -0.5, 1.75), v3(-0.9, 0.5, 2.45));
        b.cylinder_between(v3(-0.75, -0.9, 2.25), v3(-0.75, 0.9, 2.25), 0.22, 0.22, 6);
        team_panel(b, v3(-1.55, 0.0, 2.45), v2(0.9, 0.8));
        if b.fine() {
            // Recoil cylinders along the tube, breech glow behind.
            b.pitched(breech, (muzzle - breech).z.atan2((muzzle - breech).x), |b| {
                b.paint(METAL);
                b.mirror_y(|b| b.cylinder_between(v3(0.3, 0.3, 0.26), v3(2.0, 0.3, 0.26), 0.09, 0.09, 6));
            });
            b.paint(GLOW_ORANGE);
            b.block(v3(-2.23, -0.35, 2.0), v3(-2.2, 0.35, 2.2));
        }
    });

    if b.fine() {
        // Trail spades folded against the tail, deck vents, ammunition lockers.
        b.mirror_y(|b| {
            b.paint(ACCENT);
            b.beam(v3(-3.4, 1.0, 1.3), v3(-4.3, 1.3, 0.7), v2(0.3, 0.3), v2(0.3, 0.3));
            b.block(v3(-4.55, 0.95, 0.3), v3(-4.3, 1.65, 0.95));
            b.plate(deck.at(0.1, 0.62), v2(1.0, 0.8), 0.25, 0.08);
        });
        b.mirror_y(|b| vent(b, deck.at(0.8, 0.55), v2(0.9, 0.6), 3, GLOW));
    }
}

// ---- Bulwark: heavy tank ---------------------------------------------------

pub fn tank_heavy(b: &mut MeshBuilder, _tech: u8) {
    let deck = tracked_chassis(b, &Chassis { rear: -5.8, front: 5.9, track: (2.7, 4.45, 1.75), split_tracks: true, deck: 2.75 });
    if b.mid() {
        // Raised engine deck.
        b.paint(PLATING);
        b.frustum(deck.at(0.1, 0.0), v2(2.2, deck.half_width * 1.8), v2(1.6, deck.half_width * 1.5), 0.4, v2(-0.1, 0.0));
        b.paint(ACCENT);
        b.plate(deck.at(0.1, 0.0) + Vec3::Z * 0.4, v2(1.4, deck.half_width * 1.3), 0.05, 0.02);
    }

    b.set_turret_pivot(v3(0.0, 0.0, 2.75));
    b.with_part(part::TURRET, |b| {
        b.paint(PLATING);
        let roof = turret_shell(b, 6.2, 5.0, 2.8, 4.45);
        team_panel(b, roof.at(0.16, 0.0), v2(roof.length() * 0.3, roof.half_width * 1.7));
        if b.coarse() {
            rail_gun(b, v3(2.5, 0.0, 3.8), v3(6.8, 0.0, 3.8), v2(0.7, 0.45), 0.5, Emitter::Blue);
            return;
        }
        b.mirror_y(|b| rail_gun(b, v3(2.5, 0.6, 3.8), v3(6.8, 0.6, 3.8), v2(0.2, 0.4), 0.2, Emitter::Blue));
        b.paint(ACCENT);
        b.prism(v3(0.0, 0.0, 2.65), 8, 2.3, 2.3, 0.2);
        b.block(v3(2.2, -1.5, 3.3), v3(2.9, 1.5, 4.25));
        // Cheek armour modules.
        b.mirror_y(|b| {
            b.paint(PLATING);
            b.extrude_y(&[[-2.2, 3.05], [1.0, 3.05], [0.6, 4.0], [-1.9, 4.0]], 2.3, 2.65);
        });
        if b.fine() {
            b.mirror_y(|b| glow_strip(b, roof.at(0.66, 0.86), v2(roof.length() * 0.6, 0.12), GLOW));
            b.mirror_y(|b| {
                b.paint(GLOW);
                b.block(v3(-1.6, 2.65, 3.4), v3(0.2, 2.69, 3.6));
            });
            b.paint(ACCENT);
            b.prism(roof.at(0.55, 0.0), 8, 0.5, 0.42, 0.14);
            b.paint(GLASS);
            b.mirror_y(|b| b.frustum(roof.at(0.88, 0.5), v2(0.6, 0.5), v2(0.35, 0.35), 0.3, v2(-0.05, 0.0)));
            antenna(b, roof.at(-0.1, -0.5), 0.9, 0.3);
            antenna(b, roof.at(-0.1, 0.5), 0.6, 0.3);
        }
    });

    if b.fine() {
        b.mirror_y(|b| vent(b, deck.at(0.1, 0.4) + Vec3::Z * 0.45, v2(1.2, 0.8), 4, GLOW));
        b.mirror_y(|b| {
            b.paint(ACCENT);
            b.plate(deck.at(0.93, 0.6), v2(0.8, 0.9), 0.08, 0.03);
        });
    }
}

// ---- Skimmer: hover tank ---------------------------------------------------

pub fn hover_tank(b: &mut MeshBuilder, _tech: u8) {
    let skirt = chamfered_rect(v2(4.9, 3.9), 1.7);
    b.with_part(part::LOCOMOTION, |b| {
        b.paint(TREAD);
        if b.coarse() {
            b.frustum_open(v3(0.0, 0.0, 0.1), v2(8.6, 6.8), v2(9.6, 7.6), 0.8, v2(0.0, 0.0));
            return;
        }
        b.loft_z(&skirt, &[Section::new(0.12, 0.84), Section::new(0.7, 1.0), Section::new(0.85, 0.99)]);
    });
    b.paint(PLATING);
    if b.coarse() {
        b.frustum_open(v3(0.0, 0.0, 0.9), v2(9.0, 6.6), v2(5.6, 3.6), 1.1, v2(-0.5, 0.0));
    } else {
        // Lift-plenum light line, then the shell.
        b.paint(GLOW);
        b.loft_z(&skirt, &[Section::new(0.85, 0.955), Section::new(0.98, 0.955)]);
        b.paint(PLATING);
        let plan = hull_plan(-4.7, 4.9, 3.7, 2.2);
        b.loft_z(&plan, &[Section::new(0.98, 0.97), Section::new(1.3, 0.97), Section::scaled(2.0, 0.66, 0.6).shifted(-0.5, 0.0)]);
        // Lift-fan nacelles on the tail.
        b.mirror_y(|b| {
            b.paint(ACCENT);
            b.prism(v3(-3.2, 2.0, 1.25), b.sides(8), 1.15, 1.0, 0.65);
            b.paint(GLOW);
            if b.fine() {
                b.prism(v3(-3.2, 2.0, 1.9), 8, 0.7, 0.6, 0.06);
            } else {
                b.decal(v3(-3.2, 2.0, 1.92), v2(1.0, 1.0));
            }
        });
    }
    team_panel(b, v3(-2.9, 0.0, 2.0), v2(0.8, 1.8));

    b.set_turret_pivot(v3(0.0, 0.0, 2.0));
    b.with_part(part::TURRET, |b| {
        b.paint(PLATING);
        let roof = turret_shell(b, 3.6, 2.8, 2.02, 3.15);
        // Arc lance: wide-set prongs around a focusing crystal.
        rail_gun(b, v3(1.5, 0.0, 2.8), v3(4.6, 0.0, 2.8), v2(0.15, 0.26), 0.42, Emitter::Blue);
        if b.coarse() {
            return;
        }
        b.paint(GLOW);
        b.spheroid(v3(4.15, 0.0, 2.8), v3(0.34, 0.17, 0.17), 4, 2);
        b.paint(ACCENT);
        b.prism(v3(0.0, 0.0, 1.9), 8, 1.4, 1.4, 0.15);
        team_panel(b, roof.at(0.18, 0.0), v2(roof.length() * 0.32, roof.half_width * 1.7));
        if b.fine() {
            b.mirror_y(|b| glow_strip(b, roof.at(0.68, 0.82), v2(roof.length() * 0.55, 0.1), GLOW));
            b.paint(GLASS);
            b.frustum(roof.at(0.75, 0.0), v2(0.5, 0.7), v2(0.3, 0.5), 0.2, v2(-0.05, 0.0));
            antenna(b, roof.at(-0.1, 0.5), 0.6, 0.3);
        }
    });

    if b.fine() {
        on_slope(b, [4.75, 1.3], [2.73, 2.0], 0.5, |b| {
            glow_strip(b, Vec3::ZERO, v2(0.14, 1.6), GLOW);
            b.paint(ACCENT);
            b.mirror_y(|b| b.plate(v3(-0.1, 1.35, 0.0), v2(1.0, 0.5), 0.07, 0.03));
        });
        // Steering vanes and side intakes.
        b.mirror_y(|b| {
            b.paint(PLATING);
            b.extrude_y(&[[-4.9, 1.0], [-3.9, 1.0], [-4.2, 2.35], [-4.8, 2.35]], 0.75, 0.9);
            b.paint(ACCENT);
            b.block(v3(-0.9, 3.35, 1.0), v3(1.3, 3.62, 1.32));
            glow_strip(b, v3(0.2, 3.49, 1.32), v2(1.8, 0.1), GLOW);
        });
    }
}

// ---- Javelin: missile launcher ---------------------------------------------

pub fn missile_launcher(b: &mut MeshBuilder, _tech: u8) {
    b.mirror_y(|b| {
        if b.coarse() {
            b.with_part(part::LOCOMOTION, |b| b.paint(TREAD).cuboid_open(v3(0.0, 2.35, 0.8), v3(8.2, 0.9, 1.6)));
            return;
        }
        for x in [-3.3, 0.0, 3.3] {
            wheel(b, v3(x, 2.35, 0.9), 0.9, 0.9);
        }
    });
    // The rack is hull-mounted: raked 30 degrees, front face at the muzzle offset.
    let (pivot, rake, length, thickness, half_width) = (v3(-4.0, 0.0, 2.0), std::f32::consts::FRAC_PI_6, 3.8, 1.2, 1.9);
    if b.coarse() {
        b.paint(PLATING);
        b.cuboid_open(v3(0.2, 0.0, 1.3), v3(9.0, 3.8, 1.2));
        b.pitched(pivot, rake, |b| {
            b.cuboid(v3(length * 0.5, 0.0, thickness * 0.5), v3(length, half_width * 2.0, thickness));
            team_panel(b, v3(length * 0.3, 0.0, thickness), v2(0.8, 3.0));
        });
        return;
    }
    b.paint(ACCENT);
    b.block(v3(-4.4, -1.9, 0.7), v3(4.4, 1.9, 1.3));
    b.paint(PLATING);
    b.block(v3(-4.6, -2.2, 1.3), v3(1.7, 2.2, 1.75));
    // Cab.
    let cab = [[1.7, 1.3], [4.6, 1.3], [4.75, 1.9], [3.7, 2.9], [1.9, 2.9], [1.7, 2.5]];
    b.extrude_y_chamfered(&cab, 2.25, 0.5);
    on_slope(b, [4.75, 1.9], [3.7, 2.9], 0.5, |b| {
        b.paint(GLASS);
        b.plate(Vec3::ZERO, v2(0.9, 3.0), 0.06, 0.03);
    });
    team_panel(b, v3(2.75, 0.0, 2.9), v2(1.0, 2.6));

    b.pitched(pivot, rake, |b| {
        b.paint(PLATING);
        b.extrude_y_chamfered(&[[0.0, 0.0], [length, 0.0], [length, thickness], [0.0, thickness]], half_width, 0.18);
        team_panel(b, v3(length * 0.22, 0.0, thickness), v2(0.7, 2.8));
        // Tube mouths: two rows of three, warheads showing.
        for row in 0..2 {
            for col in 0..3 {
                let (y, z) = ((col as f32 - 1.0) * 1.12, 0.32 + row as f32 * 0.56);
                b.paint(ACCENT);
                b.block(v3(length, y - 0.42, z - 0.22), v3(length + 0.04, y + 0.42, z + 0.22));
                b.paint(GLOW_ORANGE);
                b.block(v3(length + 0.04, y - 0.2, z - 0.12), v3(length + 0.07, y + 0.2, z + 0.12));
            }
        }
        if b.fine() {
            // Lid seams over each tube, exhaust ports at the back.
            b.paint(ACCENT);
            for col in 0..2 {
                let y = (col as f32 - 0.5) * 1.12;
                b.block(v3(length * 0.45, y - 0.04, thickness), v3(length - 0.1, y + 0.04, thickness + 0.04));
            }
            b.block(v3(-0.05, -1.4, 0.2), v3(0.0, 1.4, 1.0));
        }
    });
    // Elevation struts and rack cradle.
    b.paint(ACCENT);
    b.extrude_y(&[[-4.2, 1.75], [-1.9, 1.75], [-2.6, 2.75], [-3.8, 2.1]], -1.5, 1.5);
    if b.fine() {
        b.mirror_y(|b| {
            b.paint(METAL);
            b.cylinder_between(v3(0.6, 1.2, 1.75), v3(-1.7, 1.2, 3.2), 0.14, 0.1, 6);
            b.paint(PLATING);
            b.block(v3(-4.2, 2.2, 1.0), v3(-1.0, 2.32, 1.6));
            glow_strip(b, v3(3.0, 1.55, 2.9), v2(1.0, 0.1), GLOW);
        });
        antenna(b, v3(1.95, -1.6, 2.85), 1.5, 0.1);
        b.paint(GLOW);
        b.block(v3(4.75, -1.5, 1.45), v3(4.79, 1.5, 1.6));
    }
}

// ---- Trebuchet: heavy artillery --------------------------------------------

pub fn artillery_heavy(b: &mut MeshBuilder, _tech: u8) {
    let deck = tracked_chassis(b, &Chassis { rear: -6.4, front: 6.4, track: (2.8, 4.65, 1.65), split_tracks: true, deck: 2.4 });

    let (breech, muzzle) = (v3(-3.6, 0.0, 2.9), v3(8.5, 0.0, 5.2));
    let elevation = (muzzle - breech).z.atan2((muzzle - breech).x);
    b.set_turret_pivot(v3(0.0, 0.0, 2.4));
    b.with_part(part::TURRET, |b| {
        rail_gun(b, breech, muzzle, v2(0.4, 0.7), 0.36, Emitter::Blue);
        b.paint(PLATING);
        if b.coarse() {
            b.frustum_open(v3(-1.2, 0.0, 2.4), v2(4.6, 4.2), v2(3.0, 2.8), 1.6, v2(-0.3, 0.0));
            team_panel(b, v3(-1.5, 0.0, 4.0), v2(3.0, 0.8));
            return;
        }
        // Trunnion towers either side of the rails.
        b.mirror_y(|b| b.extrude_y(&[[-3.4, 2.45], [1.6, 2.45], [0.9, 3.9], [-0.8, 4.45], [-2.9, 4.1]], 1.25, 2.1));
        b.paint(ACCENT);
        b.prism(v3(-0.8, 0.0, 2.32), 8, 2.6, 2.6, 0.2);
        b.cylinder_between(v3(-0.9, -2.25, 3.55), v3(-0.9, 2.25, 3.55), 0.42, 0.42, 6);
        b.block(v3(-4.6, -1.1, 2.45), v3(-3.0, 1.1, 3.5));
        on_slope(b, [-0.8, 4.45], [-2.9, 4.1], 0.5, |b| b.mirror_y(|b| team_panel(b, v3(0.0, 1.68, 0.0), v2(1.4, 0.66))));
        if b.fine() {
            // Capacitor banks along the rails: the tech-3 light show.
            b.pitched(breech, elevation, |b| {
                b.mirror_y(|b| {
                    for i in 0..4 {
                        let x = 4.4 + 1.5 * i as f32;
                        b.paint(ACCENT);
                        b.block(v3(x, 0.56, -0.24), v3(x + 0.9, 0.86, 0.24));
                        b.paint(GLOW);
                        b.block(v3(x + 0.15, 0.86, -0.12), v3(x + 0.75, 0.9, 0.12));
                    }
                });
            });
            on_slope(b, [0.9, 3.9], [-0.8, 4.45], 0.5, |b| b.mirror_y(|b| glow_strip(b, v3(0.0, 1.68, 0.0), v2(1.1, 0.16), GLOW)));
            b.paint(GLOW);
            b.block(v3(-4.64, -0.7, 2.75), v3(-4.6, 0.7, 3.2));
            antenna(b, v3(-2.6, -1.7, 4.1), 1.4, 0.15);
        }
    });
    if b.coarse() {
        return;
    }
    // Rear recoil spade and outriggers.
    b.paint(ACCENT);
    b.extrude_y(&[[-6.4, 1.6], [-7.3, 0.9], [-7.3, 0.15], [-6.9, 0.15], [-6.2, 0.9]], -2.2, 2.2);
    if b.fine() {
        b.mirror_y(|b| {
            b.paint(ACCENT);
            b.beam(v3(0.0, 4.65, 1.5), v3(0.0, 5.5, 0.5), v2(0.5, 0.4), v2(0.7, 0.3));
            vent(b, deck.at(0.8, 0.5), v2(1.6, 0.9), 4, GLOW);
            glow_strip(b, deck.at(0.06, 0.7), v2(1.0, 0.14), GLOW);
        });
    }
}
