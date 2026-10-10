//! What the wing carries: a row of fences standing on its upper skin, a row of pods on
//! short pylons along its leading edge, the canted winglet at its tip, and the panels laid
//! on its skin (a dark leading edge, elevons behind their hinge lines, the owner's band).

use super::wing::Wing;
use super::*;

pub(super) struct Kit {
    /// Spans of the fences on the upper skin.
    pub(super) fences: &'static [f32],
    /// Spans of the pods along the leading edge.
    pub(super) pods: &'static [f32],
    /// How far up the winglet stands, and how far it leans out.
    pub(super) winglet: (f32, f32),
}

impl Kit {
    /// The left wing's kit.
    pub(super) fn build(&self, b: &mut MeshBuilder, wing: &Wing) {
        for &y in self.fences {
            fence(b, wing, y);
        }
        for &y in self.pods {
            pod(b, wing, y);
        }
        self.winglet(b, wing);
        if b.fine() {
            panels(b, wing);
        }
    }

    fn winglet(&self, b: &mut MeshBuilder, wing: &Wing) {
        let (y, z, lead, trail, _) = wing.stations[wing.stations.len() - 1];
        let (up, out) = self.winglet;
        let airfoil = |x0: f32, c: f32, y: f32, z: f32, t: f32| {
            vec![
                v3(x0, y, z),
                v3(x0 - c * 0.35, y + t, z),
                v3(x0 - c, y, z),
                v3(x0 - c * 0.35, y - t, z),
            ]
        };
        let c = lead - trail;
        b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
        b.with_facets(|b| {
            b.loft(
                &[
                    airfoil(lead, c, y, z - 0.1, 0.3),
                    airfoil(lead - c * 0.7, c * 0.45, y + out, z + up, 0.12),
                ],
                true,
                true,
            );
        });
    }
}

/// A fence at span `y`: a dark blade standing on the skin from just behind the leading
/// edge back over most of the chord, low at its back.
fn fence(b: &mut MeshBuilder, wing: &Wing, y: f32) {
    // Under two metres high on a 220 m wing: full detail only.
    if !b.fine() {
        return;
    }
    let (lead, trail) = wing.edges(y);
    let c = lead - trail;
    let (front, back) = (lead - 0.2, lead - c * 0.7);
    let foot = |x: f32| wing.top(x, y).z - 0.15;
    let plan = [
        [back, foot(back)],
        [front, foot(front)],
        [front - c * 0.12, foot(front - c * 0.12) + 1.9],
        [back, foot(back) + 0.5],
    ];
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.with_facets(|b| b.extrude_y(&plan, y - 0.15, y + 0.15));
}

/// A pod at span `y`: a faceted body hung under the leading edge on a short pylon, its
/// nose run out ahead of the wing.
fn pod(b: &mut MeshBuilder, wing: &Wing, y: f32) {
    if b.coarse() {
        return;
    }
    let (lead, trail) = wing.edges(y);
    let c = lead - trail;
    let z = wing.bottom(lead - c * 0.3, y).z - 1.1;
    let (nose, tail) = (lead + 4.0, lead - c * 0.55);
    let r = 0.75;
    if !b.fine() {
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        b.beam(v3(nose, y, z), v3(tail, y, z), v2(0.4, 0.4), v2(1.3, 1.3));
        return;
    }
    let n = 6;
    b.with_facets(|b| {
        b.paint(PLATING).pattern(pattern::AIRFRAME);
        b.cylinder_between(v3(nose - 1.6, y, z), v3(tail + 0.8, y, z), r, r, n);
        b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
        b.cylinder_between(v3(nose - 1.6, y, z), v3(nose, y, z), r, 0.1, n);
        b.cylinder_between(v3(tail + 0.8, y, z), v3(tail, y, z), r, r * 0.5, n);
        let top = wing.bottom(lead - c * 0.3, y).z + 0.3;
        b.beam(
            v3(lead - c * 0.3, y, z + 0.3),
            v3(lead - c * 0.3, y, top),
            v2(0.25, c * 0.3),
            v2(0.25, c * 0.25),
        );
    });
}

/// The panels on the left wing's upper skin, bay by bay between stations: a dark leading
/// edge, an elevon behind its hinge line on every bay outboard of the root, the owner's
/// band near the tip.
fn panels(b: &mut MeshBuilder, wing: &Wing) {
    let s = wing.stations;
    let hinge = |y: f32| wing.edges(y).1 + (wing.edges(y).0 - wing.edges(y).1) * 0.22;
    for k in 0..s.len() - 1 {
        let (y0, y1) = (s[k].0 + 0.4, s[k + 1].0 - 0.4);
        let (l0, l1) = (wing.edges(y0).0, wing.edges(y1).0);
        b.paint(PLATING_DARK);
        wing.skin(
            b,
            [[l0, y0], [l1, y1]],
            [[l0 - 1.0, y0], [l1 - 0.5, y1]],
            0.03,
        );
        if k == 0 {
            continue;
        }
        b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
        wing.skin(
            b,
            [[hinge(y0) - 0.2, y0], [hinge(y1) - 0.2, y1]],
            [[wing.edges(y0).1 + 0.05, y0], [wing.edges(y1).1 + 0.05, y1]],
            0.03,
        );
        b.paint(ACCENT);
        wing.skin(
            b,
            [[hinge(y0) + 0.1, y0], [hinge(y1) + 0.1, y1]],
            [[hinge(y0) - 0.2, y0], [hinge(y1) - 0.2, y1]],
            0.04,
        );
    }
    let k = s.len() - 2;
    let (y0, y1) = (s[k].0 + 3.0, s[k].0 + 5.0);
    b.paint(TEAM).pattern(pattern::TEAM_BAND);
    wing.skin(
        b,
        [[wing.edges(y0).0 - 0.8, y0], [wing.edges(y1).0 - 0.8, y1]],
        [[hinge(y0) + 0.5, y0], [hinge(y1) + 0.5, y1]],
        0.05,
    );
}
