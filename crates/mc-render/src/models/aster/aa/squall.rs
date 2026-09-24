//! Squall: a tracked flak carrier, the Gnat grown up. A faceted enclosed turret
//! carries twin flak guns that kick back together; a tracking radar sits on the
//! roof beside them and a broad search array stands on a post behind.
//! Tech 2: faceted armour, split running gear, a few blue sensor lights.
use super::*;
use glam::Vec3;

/// Trunnion height: the blueprint's gun pivot.
const TRUNNION: f32 = 4.5;
/// Where the barrel ends: the blueprint's muzzle.
const MUZZLE: f32 = 6.0;

pub(super) fn build(b: &mut MeshBuilder) {
    let deck = tracked_chassis(
        b,
        &Chassis {
            rear: -4.6,
            front: 4.4,
            track: (1.85, 3.25, 1.45),
            split_tracks: true,
            deck: 2.2,
            dark: false,
            lit: true,
        },
    );
    // Dust stops at the deck, as on the Gnat and the Warden.
    b.set_dust_line(deck.z);
    let z = TRUNNION;
    b.set_turret_pivot(v3(0.0, 0.0, z));
    b.set_arm_pivot(v3(0.0, 0.0, z));
    b.set_recoil(v3(1.4, 0.0, z), v3(MUZZLE, 0.0, z), 0.6);
    b.with_part(part::TURRET, |b| turret(b, z));
    hull_fittings(b, deck);
}

fn turret(b: &mut MeshBuilder, z: f32) {
    b.paint(PLATING);
    let roof = turret_shell(b, 4.8, 3.8, 2.25, 4.95);
    b.with_limb(rig::ARM_GUN, |b| gun(b, z));
    if b.coarse() {
        return;
    }
    team_panel(b, roof.at(0.42, -0.4), v2(roof.length() * 0.26, roof.half_width * 0.7));
    search_radar(b, roof);
    // Turret ring.
    b.paint(ACCENT);
    b.prism(v3(0.0, 0.0, 2.16), b.sides(12), 1.75, 1.75, 0.12);
    // Tracking radar: a dark pod on the roof's front left, its lens lit.
    let pod = roof.at(0.82, 0.62);
    b.paint(PLATING_DARK);
    b.chamfered_box(pod + v3(0.0, 0.0, 0.26), v3(0.8, 0.62, 0.52), 0.1);
    if b.mid() {
        b.paint(GLOW);
        b.cuboid(pod + v3(0.41, 0.0, 0.28), v3(0.04, 0.34, 0.2));
    }
    if !b.fine() {
        return;
    }
    b.paint(GLASS);
    b.cuboid(pod + v3(0.41, 0.0, 0.28), v3(0.03, 0.5, 0.36));
    b.paint(GLOW);
    b.cuboid(pod + v3(0.43, 0.0, 0.28), v3(0.03, 0.34, 0.2));
    // Sensor strips along both roof edges: the tier's blue, kept thin.
    b.mirror_y(|b| {
        glow_strip(b, roof.at(0.5, 0.93), v2(roof.length() * 0.7, 0.08), GLOW);
        // Smoke dischargers on the cheeks.
        b.paint(ACCENT);
        b.block(v3(0.2, 1.72, 3.55), v3(1.1, 1.9, 3.72));
        b.paint(METAL);
        for i in 0..3 {
            let base = v3(0.32 + 0.3 * i as f32, 1.84, 3.7);
            b.cylinder_between(base, base + v3(0.25, 0.14, 0.3), 0.1, 0.1, 5);
        }
    });
    // Hatch, periscopes, a whip at the rear corner.
    let hatch = roof.at(0.45, 0.5);
    b.paint(ACCENT);
    b.plate(hatch, v2(0.7, 0.62), 0.06, 0.03);
    b.paint(METAL);
    b.block(hatch + v3(-0.42, -0.1, 0.0), hatch + v3(-0.35, 0.1, 0.12));
    b.paint(GLASS);
    for v in [-0.1, 0.25] {
        b.block(roof.at(0.95, v) + v3(-0.1, -0.1, 0.0), roof.at(0.95, v) + v3(0.06, 0.1, 0.14));
    }
    antenna(b, roof.at(0.05, -0.8), 1.3, 0.2);
    // Ready-round lockers on the bustle.
    b.paint(PLATING_DARK);
    b.block(v3(-2.75, -1.1, 3.0), v3(-2.3, 1.1, 3.9));
    b.paint(METAL);
    for y in [-0.6, 0.6] {
        b.block(v3(-2.8, y - 0.06, 2.96), v3(-2.3, y + 0.06, 3.94));
    }
}

/// Twin flak guns in one cradle: a wide mantlet, two white-jacketed barrels
/// that recoil together, a yoke between them near the muzzles, and long
/// perforated brakes. Conventional guns: the bores stay dark.
fn gun(b: &mut MeshBuilder, z: f32) {
    let r = 0.13;
    let gap = 0.3;
    if b.coarse() {
        b.with_recoil(|b| {
            b.paint(METAL);
            b.beam(v3(1.3, 0.0, z), v3(MUZZLE, 0.0, z), v2(1.0, 0.5), v2(0.9, 0.4));
        });
        return;
    }
    // Cast mantlet in the turret's front face.
    b.paint(ACCENT);
    b.chamfered_box(v3(1.45, 0.0, z), v3(0.8, 1.6, 1.0), 0.25);
    if b.fine() {
        // Recuperator under each barrel; they stay while the barrels slide.
        b.paint(METAL);
        b.mirror_y(|b| b.cylinder_between(v3(1.8, gap, z - 0.3), v3(2.8, gap, z - 0.3), 0.09, 0.09, 6));
        b.paint(PLATING_DARK);
        b.block(v3(2.75, -0.5, z - 0.42), v3(2.95, 0.5, z - 0.18));
    }
    b.with_recoil(|b| {
        let sides = b.sides(8);
        b.mirror_y(|b| {
            b.paint(METAL);
            b.cylinder_between(v3(1.7, gap, z), v3(MUZZLE - 0.7, gap, z), r * 1.1, r, sides);
            b.paint(PLATING);
            b.cylinder_between(v3(1.8, gap, z), v3(3.5, gap, z), r * 1.9, r * 1.7, sides);
            // Perforated brake: a dark sleeve, the bore left dark.
            b.paint(ACCENT);
            b.cylinder_between(v3(MUZZLE - 0.75, gap, z), v3(MUZZLE, gap, z), r * 1.55, r * 1.55, sides);
            if b.fine() {
                b.cylinder_between(v3(2.4, gap, z), v3(2.5, gap, z), r * 2.05, r * 2.05, 8);
                b.paint(PLATING_DARK);
                for k in 0..4 {
                    let x = MUZZLE - 0.66 + 0.16 * k as f32;
                    b.block(v3(x, gap + r * 1.5, z - 0.08), v3(x + 0.08, gap + r * 1.6, z + 0.08));
                }
            }
        });
        // The yoke that keeps the pair in line.
        b.paint(ACCENT);
        b.chamfered_box(v3(4.3, 0.0, z), v3(0.3, 2.0 * gap + 0.3, 0.3), 0.06);
    });
}

/// Search radar: a broad array on a post at the back of the turret roof, leaned
/// back to scan the sky, with a lit strip along its foot.
fn search_radar(b: &mut MeshBuilder, roof: Roof) {
    let post = roof.at(0.06, 0.0);
    let head = post + v3(-0.05, 0.0, 0.6);
    b.paint(PLATING_DARK);
    b.chamfered_box(post + v3(0.0, 0.0, 0.15), v3(0.7, 0.8, 0.3), 0.1);
    b.paint(METAL);
    b.cylinder_between(post, head, 0.14, 0.12, b.sides(6));
    b.pitched(head, 0.32, |b| {
        b.paint(PLATING);
        b.cuboid(v3(-0.08, 0.0, 0.48), v3(0.18, 2.6, 1.0));
        if b.mid() {
            b.paint(ACCENT);
            b.cuboid(v3(0.02, 0.0, 0.5), v3(0.04, 2.42, 0.84));
        }
        if b.fine() {
            b.paint(METAL);
            for y in [-0.8, -0.27, 0.27, 0.8] {
                b.cuboid(v3(0.05, y, 0.5), v3(0.03, 0.05, 0.8));
            }
            b.paint(GLOW);
            b.cuboid(v3(0.05, 0.0, 0.1), v3(0.03, 1.8, 0.04));
            b.paint(PLATING_DARK);
            b.cuboid(v3(-0.25, 0.0, 0.12), v3(0.34, 0.5, 0.3));
        }
    });
}

fn hull_fittings(b: &mut MeshBuilder, deck: Roof) {
    if b.mid() {
        b.paint(ACCENT);
        b.plate(deck.at(0.18, 0.0) + v3(0.4, 0.0, 0.0), v2(0.9, deck.half_width * 1.5), 0.06, 0.03);
    }
    if !b.fine() {
        return;
    }
    // Engine grilles on the rear deck, their slats lit low.
    b.mirror_y(|b| vent(b, deck.at(0.1, 0.45) + Vec3::Z * 0.06, v2(1.0, 0.8), 5, GLOW));
    // Driver's block, headlights, tow hooks.
    b.paint(ACCENT);
    b.block(deck.at(1.0, -0.3) + v3(-0.5, 0.0, 0.0), deck.at(1.0, 0.3) + v3(-0.1, 0.0, 0.16));
    b.paint(GLASS);
    b.block(deck.at(1.0, -0.24) + v3(-0.1, 0.0, 0.03), deck.at(1.0, 0.24) + v3(-0.07, 0.0, 0.13));
    b.mirror_y(|b| {
        b.paint(ACCENT);
        b.cuboid(v3(3.55, 1.45, 1.95), v3(0.36, 0.36, 0.3));
        b.paint(GLASS);
        b.cuboid(v3(3.74, 1.45, 1.95), v3(0.04, 0.26, 0.2));
        b.paint(METAL);
        b.block(v3(4.1, 1.0, 1.0), v3(4.5, 1.2, 1.25));
        // Exhausts out of the tail under heat shields.
        b.cylinder_between(v3(-3.9, 1.35, 1.9), v3(-4.8, 1.35, 1.75), 0.16, 0.18, 6);
        b.paint(ACCENT);
        b.block(v3(-4.6, 1.12, 1.95), v3(-3.95, 1.58, 2.01));
        // Stowage lockers along the fenders.
        b.paint(PLATING);
        b.block(v3(-3.4, 2.35, 1.45), v3(-1.6, 3.1, 1.85));
        b.paint(ACCENT);
        b.block(v3(-2.55, 2.33, 1.7), v3(-2.4, 3.12, 1.87));
    });
}
