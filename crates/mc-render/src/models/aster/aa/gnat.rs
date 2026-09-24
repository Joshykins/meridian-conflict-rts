//! Gnat: a light tracked flak carrier. One long single-barrel autocannon rides
//! high in an open cradle on a squat welded turret, a box magazine on its left
//! flank and the sight on its right; a search radar stands on a post behind it.
//! Tech 1: steel, rubber and field stowage, nothing lit, a dark bore.
use super::*;
use glam::Vec3;
use crate::models::builder::{chamfered_rect, Section};

/// Trunnion height: the blueprint's gun pivot.
const TRUNNION: f32 = 3.5;
/// Where the barrel ends: the blueprint's muzzle.
const MUZZLE: f32 = 5.5;

pub(super) fn build(b: &mut MeshBuilder) {
    let deck = tracked_chassis(
        b,
        &Chassis {
            rear: -3.2,
            front: 3.0,
            track: (1.25, 2.3, 1.1),
            split_tracks: false,
            deck: 1.62,
            dark: false,
            lit: false,
        },
    );
    // Dust climbs the running gear and lower hull to the deck, as on the Warden, not
    // 62% of the way up a model whose mast makes it tall.
    b.set_dust_line(deck.z);
    let z = TRUNNION;
    b.set_turret_pivot(v3(0.0, 0.0, z));
    b.set_arm_pivot(v3(0.0, 0.0, z));
    b.set_recoil(v3(0.9, 0.0, z), v3(MUZZLE, 0.0, z), 0.35);
    b.with_part(part::TURRET, |b| turret(b, z));
    hull_fittings(b, deck);
}

fn turret(b: &mut MeshBuilder, z: f32) {
    // A low welded tub, wider than it is tall, that the cradle stands on.
    let roof = 2.62;
    b.paint(PLATING);
    if b.coarse() {
        b.frustum_open(v3(-0.1, 0.0, 1.6), v2(2.9, 2.7), v2(2.3, 2.3), roof - 1.6, v2(-0.1, 0.0));
    } else {
        let plan = chamfered_rect(v2(1.45, 1.36), 0.55);
        b.loft_z(
            &plan,
            &[
                Section::new(1.62, 0.9).shifted(-0.05, 0.0),
                Section::new(2.0, 1.0).shifted(-0.05, 0.0),
                Section::scaled(roof, 0.84, 0.92).shifted(-0.16, 0.0),
            ],
        );
    }
    team_panel(b, v3(-0.62, 0.0, roof), v2(0.55, 1.1));

    // Cradle cheeks: two raked plates carrying the trunnion pins.
    b.mirror_y(|b| {
        b.paint(PLATING);
        let cheek = [[-0.72, roof], [0.62, roof], [0.36, z + 0.36], [-0.42, z + 0.42]];
        if !b.coarse() {
            b.extrude_y(&cheek, 0.9, 1.08);
        }
        if b.mid() {
            b.paint(ACCENT);
            b.cylinder_between(v3(0.0, 0.84, z), v3(0.0, 1.16, z), 0.24, 0.24, b.sides(8));
        }
    });
    if b.fine() {
        // Traverse ring skirt where the tub meets the deck.
        b.paint(ACCENT);
        b.prism(v3(0.0, 0.0, 1.56), 10, 1.3, 1.3, 0.1);
    }

    b.with_limb(rig::ARM_GUN, |b| gun(b, z));

    if b.coarse() {
        return;
    }
    radar(b, roof);
    if !b.fine() {
        return;
    }
    // A ready-round bin strapped across the tub's back, a whip at the corner.
    b.paint(PLATING_DARK);
    b.block(v3(-1.82, -0.7, 1.75), v3(-1.42, 0.7, 2.3));
    b.paint(METAL);
    for y in [-0.4, 0.4] {
        b.block(v3(-1.85, y - 0.05, 1.73), v3(-1.42, y + 0.05, 2.33));
    }
    whip(b, v3(-1.0, -1.0, roof), 1.3, 0.18);
    // Hatch on the roof beside the gun.
    b.paint(ACCENT);
    b.plate(v3(0.25, -0.5, roof), v2(0.44, 0.38), 0.05, 0.02);
}

/// The gun on its cradle: receiver, gun shield, magazine, sight, and a long
/// barrel with a recoil spring behind and a slotted muzzle brake at the end.
fn gun(b: &mut MeshBuilder, z: f32) {
    let barrel = 0.13;
    if b.coarse() {
        b.with_recoil(|b| {
            b.paint(METAL);
            b.beam(v3(-0.6, 0.0, z), v3(MUZZLE, 0.0, z), v2(0.5, 0.45), v2(0.3, 0.3));
        });
        return;
    }
    // Receiver: a black box through the cradle, its rear kept short so the gun
    // can go near vertical without burying it in the roof.
    b.paint(ACCENT);
    b.chamfered_box(v3(-0.08, 0.0, z), v3(1.3, 0.86, 0.72), 0.14);
    // Gun shield: a sloped white plate the barrel passes through.
    b.paint(PLATING);
    b.frustum(
        v3(0.66, 0.0, z - 0.46),
        v2(0.42, 1.34),
        v2(0.2, 1.16),
        0.92,
        v2(0.05, 0.0),
    );
    // Magazine on the left flank, the sight on the right.
    b.paint(PLATING_DARK);
    b.chamfered_box(v3(-0.2, 0.63, z + 0.1), v3(1.1, 0.4, 0.9), 0.1);
    b.paint(ACCENT);
    b.chamfered_box(v3(0.12, -0.6, z + 0.2), v3(0.62, 0.3, 0.38), 0.08);
    if b.fine() {
        b.paint(GLASS);
        b.block(v3(0.43, -0.7, z + 0.1), v3(0.46, -0.5, z + 0.3));
        // The feed lip from the magazine, and the casing chute out of the right side.
        b.paint(METAL);
        b.block(v3(-0.5, 0.42, z + 0.3), v3(0.1, 0.45, z + 0.4));
        b.beam(v3(-0.1, -0.44, z - 0.12), v3(-0.3, -0.72, z - 0.42), v2(0.18, 0.16), v2(0.2, 0.18));
        // Latches down the magazine's side.
        b.paint(ACCENT);
        for x in [-0.55, 0.15] {
            b.block(v3(x, 0.8, z - 0.05), v3(x + 0.12, 0.83, z + 0.25));
        }
    }
    b.with_recoil(|b| {
        b.paint(METAL);
        let sides = b.sides(8);
        b.cylinder_between(v3(0.62, 0.0, z), v3(MUZZLE - 0.3, 0.0, z), barrel * 1.12, barrel, sides);
        // White cooling jacket over the breech half, so the gun reads from afar.
        b.paint(PLATING);
        b.cylinder_between(v3(0.7, 0.0, z), v3(2.7, 0.0, z), barrel * 1.9, barrel * 1.6, sides);
        b.paint(METAL);
        b.paint(ACCENT);
        // Muzzle brake: a slotted block, the bore left dark (tech 1 is unlit).
        b.chamfered_box(v3(MUZZLE - 0.18, 0.0, z), v3(0.36, 0.34, 0.3), 0.06);
        if b.fine() {
            b.paint(METAL);
            b.paint(ACCENT);
            b.cylinder_between(v3(1.6, 0.0, z), v3(1.72, 0.0, z), barrel * 2.0, barrel * 2.0, 8);
            b.cylinder_between(v3(2.62, 0.0, z), v3(2.74, 0.0, z), barrel * 1.8, barrel * 1.8, 8);
            b.paint(METAL);
            b.cylinder_between(v3(3.9, 0.0, z), v3(4.0, 0.0, z), barrel * 1.4, barrel * 1.4, 8);
            b.paint(ACCENT);
            b.mirror_y(|b| {
                b.block(v3(MUZZLE - 0.3, 0.17, z - 0.09), v3(MUZZLE - 0.22, 0.19, z + 0.09));
                b.block(v3(MUZZLE - 0.14, 0.17, z - 0.09), v3(MUZZLE - 0.06, 0.19, z + 0.09));
            });
        }
    });
}

/// Search radar: a flat array on a post at the back of the turret, leaned
/// back to scan the sky. It turns with the turret; it does not pitch.
fn radar(b: &mut MeshBuilder, roof: f32) {
    let post = v3(-1.02, 0.0, roof);
    let head = post + v3(-0.1, 0.0, 1.2);
    b.paint(METAL);
    b.cylinder_between(post, head, 0.11, 0.09, b.sides(6));
    // Leaned back: the array's face (+x here) looks forward and up.
    b.pitched(head, 0.3, |b| {
        b.paint(PLATING);
        b.cuboid(v3(-0.07, 0.0, 0.42), v3(0.16, 2.1, 0.96));
        if b.fine() {
            b.paint(ACCENT);
            b.cuboid(v3(0.02, 0.0, 0.42), v3(0.04, 1.94, 0.82));
            b.paint(METAL);
            for y in [-0.65, 0.0, 0.65] {
                b.cuboid(v3(0.05, y, 0.42), v3(0.03, 0.05, 0.78));
            }
            // Drive housing where the array meets its post.
            b.paint(PLATING_DARK);
            b.cuboid(v3(-0.22, 0.0, 0.1), v3(0.3, 0.46, 0.3));
        }
    });
}

fn hull_fittings(b: &mut MeshBuilder, deck: Roof) {
    if b.mid() {
        // Engine deck behind the turret: a dark louvred plate.
        b.paint(ACCENT);
        b.plate(deck.at(0.12, 0.0), v2(deck.length() * 0.2, deck.half_width * 1.6), 0.06, 0.03);
    }
    if !b.fine() {
        return;
    }
    b.mirror_y(|b| vent(b, deck.at(0.12, 0.45) + Vec3::Z * 0.06, v2(0.8, 0.5), 4, METAL));
    // Driver's vision block on the glacis, headlights and tow hooks below it.
    b.paint(ACCENT);
    b.block(deck.at(1.0, -0.26) + v3(-0.42, 0.0, 0.0), deck.at(1.0, 0.26) + v3(-0.08, 0.0, 0.14));
    b.paint(GLASS);
    b.block(deck.at(1.0, -0.2) + v3(-0.08, 0.0, 0.03), deck.at(1.0, 0.2) + v3(-0.05, 0.0, 0.11));
    b.mirror_y(|b| {
        b.paint(ACCENT);
        b.cuboid(v3(2.5, 0.95, 1.42), v3(0.3, 0.3, 0.26));
        b.paint(GLASS);
        b.cuboid(v3(2.66, 0.95, 1.42), v3(0.04, 0.2, 0.17));
        b.paint(METAL);
        b.block(v3(2.92, 0.62, 0.75), v3(3.22, 0.78, 0.95));
    });
    // Spare track links bolted across the glacis.
    on_slope(b, [3.0, 1.15], [1.5, 1.62], 0.5, |b| {
        b.paint(TREAD);
        for i in -1..=1 {
            let y = i as f32 * 0.55;
            b.block(v3(-0.22, y - 0.24, 0.0), v3(0.22, y + 0.24, 0.1));
        }
    });
    // Fender lockers on the left, a spare road wheel and a tool rack on the right.
    b.paint(PLATING);
    b.block(v3(-2.4, 1.62, 1.08), v3(-0.9, 2.24, 1.44));
    b.paint(ACCENT);
    b.block(v3(-1.72, 1.6, 1.3), v3(-1.58, 2.26, 1.46));
    b.paint(TREAD);
    b.cylinder_between(v3(-1.8, -1.78, 1.08), v3(-1.8, -1.78, 1.3), 0.34, 0.34, 8);
    b.paint(METAL);
    b.cylinder_between(v3(-1.2, -2.05, 1.14), v3(0.3, -2.05, 1.14), 0.045, 0.045, 4);
    b.block(v3(0.3, -2.2, 1.08), v3(0.65, -1.9, 1.14));
    // Exhausts out of the tail under heat shields, a jerrycan rack between them.
    b.mirror_y(|b| {
        b.paint(METAL);
        b.cylinder_between(v3(-2.55, 0.95, 1.38), v3(-3.3, 0.95, 1.25), 0.13, 0.15, 6);
        b.paint(ACCENT);
        b.block(v3(-3.1, 0.76, 1.42), v3(-2.6, 1.14, 1.47));
    });
    for y in [-0.45, 0.05] {
        b.paint(PLATING_DARK);
        b.block(v3(-3.42, y, 0.7), v3(-3.14, y + 0.4, 1.25));
    }
}
