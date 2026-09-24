//! Sunder: a low armored carrier with a compact, recoiling shatter projector.
use super::*;

pub(super) fn build(b: &mut MeshBuilder) {
    b.set_treads(4.7, 2.4, -6.5);
    b.set_turret_pivot(v3(0.0, 0.0, 5.5));
    b.set_arm_pivot(v3(0.0, 0.0, 5.5));
    b.set_recoil(v3(0.0, 0.0, 5.5), v3(8.0, 0.0, 5.5), 0.85);
    if b.coarse() {
        b.paint(PLATING);
        b.cuboid_open(v3(-0.2, 0.0, 2.4), v3(11.6, 7.6, 2.6));
        b.mirror_y(|b| track(b, -6.5, 6.3, 3.5, 5.9, 2.4));
        b.with_part(part::TURRET, |b| {
            b.paint(PLATING_DARK);
            b.cuboid_open(v3(-0.4, 0.0, 4.5), v3(3.8, 3.8, 2.0));
            b.with_limb(rig::ARM_GUN | rig::RECOIL, |b| {
                b.paint(ACCENT);
                b.cuboid_open(v3(2.75, 0.0, 5.5), v3(10.5, 1.7, 1.5));
            });
        });
        team_panel(b, v3(-4.6, 0.0, 3.72), v2(1.4, 2.2));
        return;
    }
    b.paint(ACCENT);
    b.chamfered_box(v3(-0.2, 0.0, 1.9), v3(12.4, 8.3, 2.3), 0.65);
    b.paint(PLATING);
    b.frustum(
        v3(-0.2, 0.0, 2.3),
        v2(12.0, 8.0),
        v2(9.4, 6.7),
        1.4,
        v2(-0.65, 0.0),
    );
    b.mirror_y(|b| {
        track(b, -6.5, 6.3, 3.5, 5.9, 2.4);
        b.paint(PLATING_DARK);
        b.cuboid_open(v3(-0.3, 4.7, 2.65), v3(12.6, 2.6, 0.45));
        if b.fine() {
            // Separate skirt plates reveal the black rail and drive wheels.
            for (x, len) in [(-4.1, 3.0), (-0.5, 3.4), (3.4, 3.0)] {
                b.paint(PLATING);
                b.chamfered_box(v3(x, 6.0, 2.35), v3(len, 0.45, 1.55), 0.22);
                b.paint(ACCENT);
                b.cuboid(v3(x, 6.24, 2.6), v3(len * 0.55, 0.06, 0.25));
                b.paint(METAL);
                b.cuboid(v3(x - len * 0.3, 6.28, 2.0), v3(0.19, 0.08, 0.22));
            }
        }
        // Two compact power packs flank the rear service deck.
        b.paint(PLATING);
        b.chamfered_box(v3(-3.75, 2.55, 3.65), v3(3.7, 1.55, 1.2), 0.3);
        b.paint(ACCENT);
        b.cuboid(v3(-3.8, 2.55, 4.27), v3(2.7, 1.0, 0.12));
        b.paint(GLOW);
        b.cuboid(v3(-2.25, 2.55, 4.3), v3(0.17, 0.8, 0.08));
        if b.fine() {
            b.paint(METAL);
            for x in [-4.6, -4.0, -3.4, -2.8] {
                b.cuboid(v3(x, 2.55, 4.38), v3(0.16, 0.85, 0.12));
            }
            b.paint(ACCENT);
            b.chamfered_box(v3(4.5, 2.65, 3.25), v3(1.6, 0.95, 0.6), 0.18);
            b.paint(GLASS);
            b.cuboid(v3(5.32, 2.65, 3.28), v3(0.06, 0.62, 0.22));
            b.paint(METAL);
            b.cylinder_between(v3(-5.7, 2.6, 2.8), v3(-6.6, 2.6, 3.1), 0.26, 0.24, 8);
        }
    });
    b.paint(ACCENT);
    b.chamfered_box(v3(-4.3, 0.0, 3.72), v3(2.7, 2.6, 0.18), 0.25);
    team_panel(b, v3(-4.3, 0.0, 3.83), v2(1.6, 1.8));
    if b.fine() {
        b.paint(METAL);
        b.cuboid(v3(-4.9, 0.0, 3.89), v3(0.15, 0.9, 0.12));
        b.paint(ACCENT);
        b.chamfered_box(v3(3.65, 0.0, 3.57), v3(2.3, 2.8, 0.22), 0.2);
        b.paint(PLATING_DARK);
        b.cuboid(v3(3.6, 0.0, 3.7), v3(1.6, 1.5, 0.08));
    }
    // Continuous deck bearing and a compact two-sided elevation saddle.
    b.paint(ACCENT);
    b.prism(v3(0.0, 0.0, 3.6), b.sides(12), 2.1, 2.1, 0.55);
    b.with_part(part::TURRET, |b| {
        b.paint(PLATING_DARK);
        b.prism(v3(0.0, 0.0, 4.05), b.sides(12), 1.9, 1.65, 0.6);
        b.mirror_y(|b| {
            b.paint(PLATING);
            b.chamfered_box(v3(-0.45, 1.5, 5.2), v3(2.9, 0.7, 2.1), 0.28);
            b.paint(ACCENT);
            b.cylinder_between(
                v3(0.0, 0.95, 5.5),
                v3(0.0, 1.92, 5.5),
                0.58,
                0.58,
                b.sides(8),
            );
            if b.fine() {
                b.paint(METAL);
                b.cylinder_between(v3(0.0, 1.93, 5.5), v3(0.0, 2.02, 5.5), 0.26, 0.26, 6);
            }
        });
        if b.fine() {
            b.paint(PLATING_DARK);
            b.chamfered_box(v3(-1.65, -2.05, 5.55), v3(1.45, 0.65, 0.95), 0.15);
            b.paint(GLASS);
            b.cuboid(v3(-0.91, -2.05, 5.55), v3(0.05, 0.42, 0.48));
        }
        b.with_limb(rig::ARM_GUN | rig::RECOIL, |b| {
            b.paint(ACCENT);
            b.chamfered_box(v3(-0.35, 0.0, 5.5), v3(4.3, 2.15, 1.65), 0.35);
            b.chamfered_box(v3(4.45, 0.0, 5.5), v3(6.3, 1.05, 0.85), 0.18);
            b.mirror_y(|b| {
                b.paint(PLATING);
                b.chamfered_box(v3(2.7, 0.68, 5.65), v3(3.7, 0.38, 1.15), 0.13);
                b.paint(GLOW);
                b.cuboid(v3(5.2, 0.54, 5.5), v3(2.3, 0.06, 0.12));
                if b.fine() {
                    b.paint(METAL);
                    b.cylinder_between(v3(-1.65, 0.8, 6.35), v3(1.3, 0.8, 6.35), 0.13, 0.13, 6);
                    b.paint(PLATING_DARK);
                    for x in [-1.4, -0.8, -0.2] {
                        b.cuboid(v3(x, 1.08, 5.45), v3(0.16, 0.08, 0.6));
                    }
                }
            });
            b.paint(METAL);
            b.chamfered_box(v3(7.65, 0.0, 5.5), v3(0.7, 1.4, 1.15), 0.15);
            b.paint(ACCENT);
            b.cuboid(v3(8.01, 0.0, 5.5), v3(0.04, 1.02, 0.79));
            b.paint(GLOW);
            b.cuboid(v3(8.04, 0.0, 5.5), v3(0.02, 0.56, 0.25));
            team_panel(b, v3(-1.0, 0.0, 6.34), v2(1.15, 1.15));
        });
    });
}
