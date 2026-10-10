//! The bridged tail: two swept fins canted out, each on a short boom run aft off the top
//! of its engine sponson, and a long swept wing laid across their tips, running on past
//! them to sharp points. The dispenser's mouth stays clear between and behind them.

use super::dispenser::face_toward;
use super::*;

pub(super) struct Tail {
    /// How far the fins cant out from upright (radians).
    pub(super) cant: f32,
    /// The fins' root leading edge (x), and the height of their tips, where the wing runs.
    pub(super) front: f32,
    pub(super) top: f32,
    /// The wing across the tips: its leading edge on the centre line, its sweep (x back per
    /// metre out), its chord, and how far it runs on past the fins' tips.
    pub(super) lead: f32,
    pub(super) sweep: f32,
    pub(super) chord: f32,
    pub(super) overhang: f32,
}

/// The fins' root trailing edge.
const BACK: f32 = -41.0;
/// The tail booms: off the top of each sponson at `BOOM_Y`, from `BOOM_FRONT` to `BACK`.
const BOOM_Y: f32 = 8.6;
const BOOM_Z: f32 = 6.6;
const BOOM_R: f32 = 1.1;
const BOOM_FRONT: f32 = -23.5;
/// Where a fin's root stands on its boom.
const FOOT: f32 = BOOM_Z + BOOM_R * 0.6;

impl Tail {
    /// The left fin's tip: its span, and its leading and trailing x under the wing.
    fn tip(&self) -> (f32, f32, f32) {
        let y = BOOM_Y + (self.top - FOOT) * self.cant.tan();
        let lead = self.lead - y * self.sweep;
        (y, lead, lead - self.chord)
    }

    /// The fin's outline in its own plane: x along, up the fin from its foot.
    fn plan(&self) -> [[f32; 2]; 4] {
        let (_, lead, trail) = self.tip();
        let len = (self.top - FOOT) / self.cant.cos();
        [[BACK, 0.0], [self.front, 0.0], [lead, len], [trail, len]]
    }

    /// The left fin's frame: on its boom, canted out.
    fn place(&self) -> Affine3A {
        Affine3A::from_translation(v3(0.0, BOOM_Y, FOOT)) * Affine3A::from_rotation_x(-self.cant)
    }

    pub(super) fn build(&self, b: &mut MeshBuilder) {
        let plan = self.plan();
        b.mirror_y(|b| {
            boom(b);
            b.with(self.place(), |b| {
                b.paint(PLATING).pattern(pattern::AIRFRAME);
                b.with_facets(|b| b.extrude_y(&plan, -0.5, 0.5));
                if b.fine() {
                    // A graphite face let into each side.
                    let [_, _, [lead, len], [trail, _]] = plan;
                    let face = [
                        [BACK + 2.0, 1.2],
                        [self.front - 2.5, 1.2],
                        [lead - 1.0, len - 1.6],
                        [trail + 1.2, len - 1.6],
                    ];
                    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
                    b.extrude_y(&face, -0.55, 0.55);
                }
            });
        });
        // The wing across the tips: full chord out to the fins, then swept to a point.
        let outer = self.tip().0;
        let airfoil = |y: f32, c: f32, t: f32, back: f32| {
            let lead = self.lead - y * self.sweep - back;
            let z = self.top + y * 0.04;
            vec![
                v3(lead, y, z),
                v3(lead - c * 0.3, y, z + t),
                v3(lead - c, y, z),
                v3(lead - c * 0.35, y, z - t * 0.6),
            ]
        };
        b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
        b.with_facets(|b| {
            b.mirror_y(|b| {
                b.loft(
                    &[
                        airfoil(0.0, self.chord, 0.6, 0.0),
                        airfoil(outer, self.chord, 0.5, 0.0),
                        airfoil(
                            outer + self.overhang,
                            self.chord * 0.25,
                            0.12,
                            self.chord * 0.5,
                        ),
                    ],
                    true,
                    true,
                );
            });
        });
    }

    /// Far away: each fin a plate seen from outboard, the wing a plate seen from above.
    pub(super) fn coarse(&self, b: &mut MeshBuilder) {
        let fin = self.plan().map(|[x, z]| v3(x, 0.0, z));
        b.paint(PLATING);
        b.mirror_y(|b| b.with(self.place(), |b| face_toward(b, &fin, Vec3::Y)));
        let span = self.tip().0 + self.overhang;
        let lead = |y: f32| self.lead - y * self.sweep;
        let wing = [
            v3(lead(0.0), 0.0, self.top),
            v3(lead(span) - self.chord * 0.5, span, self.top),
            v3(lead(0.0) - self.chord, 0.0, self.top),
            v3(lead(span) - self.chord * 0.5, -span, self.top),
        ];
        b.paint(PLATING_DARK);
        face_toward(b, &wing, Vec3::Z);
    }
}

/// The left tail boom: a slim faceted spar off the top of the sponson, carrying its fin.
fn boom(b: &mut MeshBuilder) {
    let n = b.sides(6);
    let axis = |x: f32| v3(x, BOOM_Y, BOOM_Z);
    b.paint(PLATING_DARK).pattern(pattern::AIRFRAME);
    b.with_facets(|b| {
        b.cylinder_between(axis(BOOM_FRONT), axis(BOOM_FRONT - 3.0), 0.4, BOOM_R, n);
        b.cylinder_between(axis(BOOM_FRONT - 3.0), axis(BACK + 1.0), BOOM_R, BOOM_R, n);
        b.cylinder_between(axis(BACK + 1.0), axis(BACK - 1.5), BOOM_R, 0.3, n);
    });
}
