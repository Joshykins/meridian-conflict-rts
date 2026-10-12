//! Mistral: tech 3 tracked rail AA. One long rail cannon that throws a slug at one
//! aircraft, on a T3 tracked hull in the Arbalest's language: hard facets, nothing lit
//! but dark lenses, the rails bare metal. The gun lies level here; the renderer raises
//! it to the sky at rest, as on every AA-only pivoted weapon. The rail rides between
//! two tall cheeks of a low turret (a yoke), a flat search array leaned back on the
//! bustle.
use super::*;
use glam::{Vec2, Vec3};

/// The trunnion the rail pitches about: the blueprint's `pivot`.
const TRUNNION: Vec3 = Vec3::new(-0.4, 0.0, 3.6);
/// The rails' mouth: the blueprint's `muzzle`.
const MUZZLE: Vec3 = Vec3::new(9.8, 0.0, 3.6);
/// Where the rails' power block starts, a short way behind the trunnion so the breech
/// clears the turret when the gun stands up at the sky.
const BREECH: Vec3 = Vec3::new(-0.9, 0.0, 3.6);
/// One rail's (width, height), and the slot between the pair.
const RAIL: Vec2 = Vec2::new(0.26, 0.56);
const GAP: f32 = 0.4;

pub(super) fn build(b: &mut MeshBuilder) {
    let deck = tracked_chassis(
        b,
        &Chassis {
            rear: -5.4,
            front: 5.6,
            track: (2.35, 3.85, 1.3),
            split_tracks: false,
            deck: 2.0,
            dark: false,
            lit: false,
        },
    );
    b.set_dust_line(1.5);
    hull_fittings(b, deck);
    b.set_turret_pivot(v3(0.0, 0.0, deck.z));
    b.set_arm_pivot(TRUNNION);
    b.set_recoil(BREECH, MUZZLE, 0.4);
    b.with_part(part::TURRET, |b| {
        // A low faceted race, the cheeks standing up out of it either side of the rail.
        b.paint(PLATING);
        let roof = turret_shell(b, 5.4, 4.2, deck.z, 2.7);
        b.with_limb(rig::ARM_GUN | rig::RECOIL, rail);
        if b.coarse() {
            team_panel(b, roof.at(0.3, 0.0), v2(1.4, 1.2));
            return;
        }
        b.paint(PLATING);
        b.mirror_y(|b| cheek(b, 0.62, 0.3));
        team_panel(b, roof.at(0.25, -0.62), v2(1.0, 0.6));
        team_panel(b, roof.at(0.25, 0.62), v2(1.0, 0.6));
        search_array(b, v3(-2.15, 0.0, 2.7), 0.45, v2(1.1, 2.6));
        if b.fine() {
            // A dark sight in the left cheek's face, looking down the rail.
            b.paint(GLASS);
            b.cuboid(v3(1.32, 0.9, 3.9), v3(0.05, 0.26, 0.2));
            antenna_unlit(b, roof.at(0.1, -0.85), 1.1, 0.15);
        }
    });
}

/// The rail cannon, level from breech to muzzle.
fn rail(b: &mut MeshBuilder) {
    rail_gun(b, BREECH, MUZZLE, RAIL, GAP, Emitter::Unlit);
}

/// One cheek of the yoke on the +y side: a sloped plate from the turret roof up past the
/// trunnion, `y` its inner face, `t` thick.
fn cheek(b: &mut MeshBuilder, y: f32, t: f32) {
    b.at(v3(0.0, y + t * 0.5, 0.0), |b| {
        b.extrude_y_chamfered(
            &[
                [-1.9, 2.6],
                [1.1, 2.6],
                [1.45, 3.1],
                [1.0, 4.3],
                [-0.9, 4.45],
                [-1.9, 3.5],
            ],
            t * 0.5,
            0.08,
        );
    });
}

/// A flat search array leaned back to scan the sky: a post from `base` and a dark panel
/// `size` (height, width) pitched back by `lean` radians, a steel strip along its foot.
fn search_array(b: &mut MeshBuilder, base: Vec3, lean: f32, size: Vec2) {
    if b.coarse() {
        return;
    }
    let head = base + v3(0.0, 0.0, 0.3);
    b.paint(METAL);
    b.cylinder_between(base, head, 0.16, 0.14, b.sides(6));
    b.pitched(head, lean, |b| {
        b.paint(PLATING);
        b.cuboid(v3(-0.1, 0.0, size.x * 0.5), v3(0.2, size.y, size.x));
        b.paint(PLATING_DARK);
        b.cuboid(
            v3(0.02, 0.0, size.x * 0.5),
            v3(0.06, size.y - 0.2, size.x - 0.2),
        );
        if b.fine() {
            b.paint(METAL);
            b.cuboid(v3(0.06, 0.0, 0.12), v3(0.04, size.y - 0.2, 0.06));
        }
    });
}

/// Driver's block, grilles, exhausts and lockers on the hull deck.
fn hull_fittings(b: &mut MeshBuilder, deck: Roof) {
    if !b.fine() {
        return;
    }
    b.mirror_y(|b| {
        vent(
            b,
            deck.at(0.08, 0.5) + Vec3::Z * 0.04,
            v2(1.1, 0.8),
            5,
            METAL,
        )
    });
    b.paint(ACCENT);
    b.block(
        deck.at(1.0, -0.3) + v3(-0.5, 0.0, 0.0),
        deck.at(1.0, 0.3) + v3(-0.1, 0.0, 0.16),
    );
    b.paint(GLASS);
    b.block(
        deck.at(1.0, -0.24) + v3(-0.1, 0.0, 0.03),
        deck.at(1.0, 0.24) + v3(-0.07, 0.0, 0.13),
    );
}
