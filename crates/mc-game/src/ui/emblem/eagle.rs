//! The Asterian Reach Command's marks: an eagle with its wings raised over a
//! shield, drawn in silver on gunmetal with the ice-blue limb of Asteria
//! rising under a four-point star.
//!
//! Design space: the crest is 1000 units square, x = 500 is its axis, y runs
//! down. Everything else is laid out from the same pieces.

use super::paint::{circle, hex, linear, poly, radial, Canvas, Type};
use super::Words;
use glam::Vec2;
use mc_render::Face;
use tiny_skia::{Color, Path, PathBuilder};

const AXIS: f32 = 500.0;
/// The right wing's pivot, hidden behind the shield's upper corner.
const SHOULDER: Vec2 = Vec2::new(630.0, 480.0);

const SILVER_HI: u32 = 0xF4F7F9;
const SILVER: u32 = 0xC6CDD4;
const SILVER_LO: u32 = 0x7D8893;
const GUNMETAL: u32 = 0x1B222B;
const GUN_DEEP: u32 = 0x0B0F14;
const SLATE: u32 = 0x445264;
const OUTLINE: u32 = 0x0D1117;
const ICE: u32 = 0x6FD0FF;
const ICE_HI: u32 = 0xDDF5FF;

/// Mirrors a right-hand point to `side` (+1 right, -1 left).
fn side_of(p: Vec2, side: f32) -> Vec2 {
    Vec2::new(AXIS + (p.x - AXIS) * side, p.y)
}

// -- the wings -----------------------------------------------------------------

/// One flight feather: a blade from `root` along `angle` (radians, y down),
/// widest past the middle, curving by `bend`, its tip cut on the slant.
fn feather(
    root: Vec2,
    angle: f32,
    len: f32,
    width: f32,
    bend: f32,
    side: f32,
) -> (Option<Path>, [Vec2; 2]) {
    let d = Vec2::from_angle(angle);
    let n = d.perp();
    let at = |along: f32, across: f32| side_of(root + d * (len * along) + n * across, side);
    let bow = |along: f32| bend * 4.0 * along * (1.0 - along);
    let mut pb = PathBuilder::new();
    let start = at(0.0, width * 0.26);
    pb.move_to(start.x, start.y);
    let (c1, c2, tip) = (
        at(0.35, width * 0.52 + bow(0.35)),
        at(0.78, width * 0.56 + bow(0.78)),
        at(1.0, width * 0.08 + bow(1.0)),
    );
    pb.cubic_to(c1.x, c1.y, c2.x, c2.y, tip.x, tip.y);
    let cut = at(0.88, -width * 0.42 + bow(0.88));
    pb.line_to(cut.x, cut.y);
    let (c3, c4, end) = (
        at(0.62, -width * 0.52 + bow(0.62)),
        at(0.25, -width * 0.44 + bow(0.25)),
        at(0.0, -width * 0.26),
    );
    pb.cubic_to(c3.x, c3.y, c4.x, c4.y, end.x, end.y);
    pb.close();
    (pb.finish(), [at(0.0, 0.0), at(1.0, 0.0)])
}

/// A rank of feathers fanned round a shoulder.
struct Rank {
    count: usize,
    /// Angles of the first (innermost, highest) and last feather.
    from: f32,
    to: f32,
    /// Lengths of the first and last feather.
    long: f32,
    short: f32,
    width: f32,
    /// How far out from the shoulder the roots sit.
    reach: f32,
    /// Base brightness, 0..1.
    light: f32,
}

const DEG: f32 = std::f32::consts::PI / 180.0;

/// The crest's wings, back rank first.
fn ranks() -> [Rank; 3] {
    [
        Rank {
            count: 8,
            from: -84.0 * DEG,
            to: -8.0 * DEG,
            long: 390.0,
            short: 250.0,
            width: 80.0,
            reach: 60.0,
            light: 0.5,
        },
        Rank {
            count: 7,
            from: -90.0 * DEG,
            to: -16.0 * DEG,
            long: 260.0,
            short: 180.0,
            width: 74.0,
            reach: 50.0,
            light: 0.78,
        },
        Rank {
            count: 5,
            from: -100.0 * DEG,
            to: -34.0 * DEG,
            long: 140.0,
            short: 105.0,
            width: 68.0,
            reach: 36.0,
            light: 1.0,
        },
    ]
}

fn wings(c: &mut Canvas, shoulder: Vec2, detail: bool) {
    for rank in ranks() {
        for side in [1.0f32, -1.0] {
            // Outermost first, so each feather lies over the one outside it.
            for i in (0..rank.count).rev() {
                let t = i as f32 / (rank.count - 1).max(1) as f32;
                let angle = rank.from + (rank.to - rank.from) * t;
                let len = rank.long + (rank.short - rank.long) * t.powf(1.4);
                let root = shoulder + Vec2::from_angle(angle) * rank.reach;
                let (path, [base, tip]) = feather(root, angle, len, rank.width, 22.0, side);
                let k = rank.light;
                let shade = |rgb: u32| hex(rgb, 1.0).to_color_u8();
                let lo = shade(SILVER_LO);
                let mix = |a: u32| {
                    let c = shade(a);
                    Color::from_rgba8(
                        (lo.red() as f32 + (c.red() as f32 - lo.red() as f32) * k) as u8,
                        (lo.green() as f32 + (c.green() as f32 - lo.green() as f32) * k) as u8,
                        (lo.blue() as f32 + (c.blue() as f32 - lo.blue() as f32) * k) as u8,
                        255,
                    )
                };
                let stops = [
                    (0.0, hex(SILVER_LO, 1.0)),
                    (0.5, mix(SILVER)),
                    (1.0, mix(SILVER_HI)),
                ];
                c.fill_with(&path, linear(base, tip, &stops), None);
                c.stroke(&path, 5.0, 1.0, hex(OUTLINE, 1.0));
                if detail {
                    // The shaft, set toward the leading edge.
                    let d = (tip - base).normalize_or_zero();
                    let n = d.perp() * side;
                    let (a, b) = (
                        base + (tip - base) * 0.18 + n * 6.0,
                        base + (tip - base) * 0.8 + n * 9.0,
                    );
                    c.stroke(&poly(&[a, b]), 2.2, 0.0, hex(OUTLINE, 0.45));
                }
            }
        }
    }
}

// -- the head --------------------------------------------------------------------

/// The head in profile, looking to the viewer's left.
fn head(c: &mut Canvas) {
    let k = 1.0;
    let p = |x: f32, y: f32| Vec2::new(x, y);
    let path = |pts: &[(f32, f32, f32, f32, f32, f32)], start: (f32, f32)| {
        let mut pb = PathBuilder::new();
        let s = p(start.0, start.1);
        pb.move_to(s.x, s.y);
        for &(x1, y1, x2, y2, x, y) in pts {
            let (a, b, e) = (p(x1, y1), p(x2, y2), p(x, y));
            pb.cubic_to(a.x, a.y, b.x, b.y, e.x, e.y);
        }
        pb.close();
        pb.finish()
    };
    // Neck and crown, down to where the shield covers it.
    let skull = path(
        &[
            (590.0, 380.0, 592.0, 300.0, 566.0, 252.0),
            (548.0, 220.0, 500.0, 206.0, 466.0, 222.0),
            (446.0, 230.0, 436.0, 240.0, 428.0, 252.0),
            (420.0, 268.0, 432.0, 296.0, 452.0, 302.0),
            (462.0, 330.0, 450.0, 360.0, 424.0, 392.0),
            (470.0, 400.0, 520.0, 404.0, 575.0, 404.0),
        ],
        (575.0, 404.0),
    );
    // Hackles: the ragged fringe where the neck meets the breast.
    let hackles = poly(
        &[
            (424.0, 392.0),
            (410.0, 432.0),
            (448.0, 410.0),
            (446.0, 446.0),
            (478.0, 414.0),
            (484.0, 450.0),
            (510.0, 416.0),
            (575.0, 404.0),
        ]
        .map(|(x, y)| p(x, y)),
    );
    // The hooked beak.
    let beak = path(
        &[
            (420.0, 238.0, 392.0, 246.0, 374.0, 268.0),
            (364.0, 282.0, 366.0, 300.0, 376.0, 306.0),
            (378.0, 294.0, 386.0, 288.0, 398.0, 290.0),
            (414.0, 292.0, 432.0, 298.0, 452.0, 302.0),
            (440.0, 280.0, 436.0, 258.0, 446.0, 232.0),
        ],
        (446.0, 232.0),
    );
    let shade = linear(
        p(560.0, 220.0),
        p(470.0, 400.0),
        &[(0.0, hex(SILVER_HI, 1.0)), (1.0, hex(SILVER, 1.0))],
    );
    c.fill_with(
        &hackles,
        linear(
            p(480.0, 400.0),
            p(480.0, 450.0),
            &[(0.0, hex(SILVER, 1.0)), (1.0, hex(SILVER_LO, 1.0))],
        ),
        None,
    );
    c.stroke(&hackles, 5.0 * k, 1.0, hex(OUTLINE, 1.0));
    c.fill_with(&skull, shade, None);
    c.stroke(&skull, 5.0 * k, 1.0, hex(OUTLINE, 1.0));
    c.fill_with(
        &beak,
        linear(
            p(450.0, 240.0),
            p(370.0, 300.0),
            &[(0.0, hex(SILVER, 1.0)), (1.0, hex(SILVER_LO, 1.0))],
        ),
        None,
    );
    c.stroke(&beak, 5.0 * k, 1.0, hex(OUTLINE, 1.0));
    // The gape, the eye under a hard brow, and a few strokes of neck feathering.
    c.stroke(
        &open(&[p(452.0, 300.0), p(418.0, 288.0), p(396.0, 286.0)]),
        4.0 * k,
        1.0,
        hex(OUTLINE, 1.0),
    );
    c.fill(
        &poly(&[
            p(464.0, 256.0),
            p(482.0, 247.0),
            p(500.0, 249.0),
            p(490.0, 261.0),
            p(472.0, 262.0),
        ]),
        hex(OUTLINE, 1.0),
    );
    c.fill(&circle(p(487.0, 253.0), 3.2 * k), hex(0xFFFFFF, 1.0));
    c.fill(
        &poly(&[
            p(450.0, 246.0),
            p(478.0, 236.0),
            p(520.0, 232.0),
            p(482.0, 244.0),
            p(458.0, 251.0),
        ]),
        hex(OUTLINE, 1.0),
    );
    for (a, b) in [
        ((540.0, 300.0), (500.0, 350.0)),
        ((566.0, 318.0), (530.0, 372.0)),
        ((520.0, 330.0), (486.0, 376.0)),
    ] {
        c.stroke(
            &open(&[p(a.0, a.1), p(b.0, b.1)]),
            3.0 * k,
            0.0,
            hex(OUTLINE, 0.5),
        );
    }
}

/// An open line through `points`.
fn open(points: &[Vec2]) -> Option<Path> {
    let mut pb = PathBuilder::new();
    let (first, rest) = points.split_first()?;
    pb.move_to(first.x, first.y);
    for q in rest {
        pb.line_to(q.x, q.y);
    }
    pb.finish()
}

// -- the shield --------------------------------------------------------------------

/// The shield, `inset` units in from its rim, its top edge at `top`.
fn shield(inset: f32, top: f32, point: f32) -> Option<Path> {
    shield_at(inset, top, point, 1.0)
}

/// `shield` made `wide` times as broad.
fn shield_at(inset: f32, top: f32, point: f32, wide: f32) -> Option<Path> {
    let i = inset;
    let (x0, x1, t) = (AXIS - 200.0 * wide + i, AXIS + 200.0 * wide - i, top + i);
    let shoulder = top + (point - top) * 0.46;
    let tip = point - i * 1.5;
    let mut pb = PathBuilder::new();
    pb.move_to(x0, t);
    // The chief rises a little to the middle, where the eagle's breast is.
    pb.quad_to(AXIS, t - 18.0, x1, t);
    pb.line_to(x1, shoulder);
    pb.cubic_to(
        x1,
        shoulder + (tip - shoulder) * 0.45,
        AXIS + 110.0 - i * 0.3,
        tip - 60.0,
        AXIS,
        tip,
    );
    pb.cubic_to(
        AXIS - 110.0 + i * 0.3,
        tip - 60.0,
        x0,
        shoulder + (tip - shoulder) * 0.45,
        x0,
        shoulder,
    );
    pb.close();
    pb.finish()
}

/// The letters ARC, blocky with chamfered corners, `cap` tall, centred at `centre`.
pub(super) fn logotype(centre: Vec2, cap: f32) -> Option<Path> {
    const GAP: f32 = 16.0;
    const WIDTHS: [f32; 3] = [100.0, 102.0, 100.0];
    let total = WIDTHS.iter().sum::<f32>() + 2.0 * GAP;
    let k = cap / 100.0;
    let origin = centre - Vec2::new(total * 0.5, 50.0) * k;
    let letters: [&[&[(f32, f32)]]; 3] = [
        &[
            &[
                (0.0, 100.0),
                (32.0, 0.0),
                (68.0, 0.0),
                (100.0, 100.0),
                (76.0, 100.0),
                (67.7, 74.0),
                (32.3, 74.0),
                (24.0, 100.0),
            ],
            &[(49.0, 22.0), (51.0, 22.0), (61.9, 56.0), (38.1, 56.0)],
        ],
        &[
            &[
                (0.0, 0.0),
                (80.0, 0.0),
                (100.0, 20.0),
                (100.0, 40.0),
                (88.0, 54.0),
                (102.0, 100.0),
                (78.0, 100.0),
                (66.0, 62.0),
                (24.0, 62.0),
                (24.0, 100.0),
                (0.0, 100.0),
            ],
            &[
                (24.0, 22.0),
                (72.0, 22.0),
                (76.0, 26.0),
                (76.0, 36.0),
                (72.0, 40.0),
                (24.0, 40.0),
            ],
        ],
        &[&[
            (18.0, 0.0),
            (100.0, 0.0),
            (100.0, 23.0),
            (30.0, 23.0),
            (24.0, 29.0),
            (24.0, 71.0),
            (30.0, 77.0),
            (100.0, 77.0),
            (100.0, 100.0),
            (18.0, 100.0),
            (0.0, 82.0),
            (0.0, 18.0),
        ]],
    ];
    let mut pb = PathBuilder::new();
    let mut x = 0.0;
    for (shapes, w) in letters.iter().zip(WIDTHS) {
        for shape in shapes.iter() {
            for (j, &(px, py)) in shape.iter().enumerate() {
                let q = origin + Vec2::new(x + px, py) * k;
                if j == 0 {
                    pb.move_to(q.x, q.y);
                } else {
                    pb.line_to(q.x, q.y);
                }
            }
            pb.close();
        }
        x += w + GAP;
    }
    pb.finish()
}

/// Where the logotype would be cut for a stencil, in the same frame as `logotype`.
fn stencil_bridges(centre: Vec2, cap: f32) -> Vec<Option<Path>> {
    let k = cap / 100.0;
    let origin = centre - Vec2::new(334.0 * 0.5, 50.0) * k;
    let bar = |x: f32, y0: f32, y1: f32| {
        let (a, b) = (
            origin + Vec2::new(x, y0) * k,
            origin + Vec2::new(x + 7.0, y1) * k,
        );
        poly(&[a, Vec2::new(b.x, a.y), b, Vec2::new(a.x, b.y)])
    };
    vec![
        bar(46.5, -2.0, 24.0),
        bar(116.0 + 24.0, -2.0, 23.0),
        bar(116.0 + 24.0, 39.0, 63.0),
        bar(234.0 + 56.0, -2.0, 25.0),
        bar(234.0 + 56.0, 75.0, 102.0),
    ]
}

/// A four-point star, `r` to its points.
fn star(c: Vec2, r: f32) -> Option<Path> {
    let w = r * 0.2;
    poly(&[
        c + Vec2::new(0.0, -r),
        c + Vec2::new(w, -w),
        c + Vec2::new(r, 0.0),
        c + Vec2::new(w, w),
        c + Vec2::new(0.0, r),
        c + Vec2::new(-w, w),
        c + Vec2::new(-r, 0.0),
        c + Vec2::new(-w, -w),
    ])
}

/// The field: the logotype, the star, and Asteria's limb rising from the foot.
fn field(c: &mut Canvas, top: f32, point: f32, rich: bool) {
    let inner = shield(20.0, top, point);
    c.fill_with(
        &inner,
        linear(
            Vec2::new(AXIS, top),
            Vec2::new(AXIS, point),
            &[(0.0, hex(GUNMETAL, 1.0)), (1.0, hex(GUN_DEEP, 1.0))],
        ),
        None,
    );
    let clip = c.mask(&inner);
    if rich {
        // Asteria: a dark world with a bright rim, and the air above it lit.
        let planet = Vec2::new(AXIS, point + 250.0);
        let r = 400.0;
        c.fill_with(
            &circle(planet, r + 90.0),
            radial(
                planet,
                r + 90.0,
                &[
                    (0.0, hex(ICE, 0.0)),
                    (r / (r + 90.0), hex(ICE, 0.42)),
                    (1.0, hex(ICE, 0.0)),
                ],
            ),
            clip.as_ref(),
        );
        c.fill_with(
            &circle(planet, r),
            radial(
                planet - Vec2::Y * 60.0,
                r,
                &[
                    (0.0, hex(0x05080C, 1.0)),
                    (0.8, hex(0x0D1C2A, 1.0)),
                    (1.0, hex(0x1E4D6E, 1.0)),
                ],
            ),
            clip.as_ref(),
        );
        c.stroke_with(
            &circle(planet, r),
            9.0,
            1.0,
            tiny_skia::Shader::SolidColor(hex(ICE, 1.0)),
            clip.as_ref(),
        );
        c.stroke_with(
            &circle(planet, r - 3.0),
            3.0,
            0.0,
            tiny_skia::Shader::SolidColor(hex(ICE_HI, 1.0)),
            clip.as_ref(),
        );
        // The star, with a halo.
        let s = Vec2::new(AXIS, top + 214.0);
        c.fill_with(
            &circle(s, 80.0),
            radial(s, 80.0, &[(0.0, hex(ICE, 0.55)), (1.0, hex(ICE, 0.0))]),
            clip.as_ref(),
        );
        c.fill(&star(s, 40.0), hex(ICE_HI, 1.0));
        c.fill(&star(s, 22.0), hex(0xFFFFFF, 1.0));
    }
    // The inner rule.
    c.stroke(&shield(34.0, top, point), 3.0, 0.0, hex(SLATE, 1.0));
    let letters = logotype(Vec2::new(AXIS, top + 108.0), 96.0);
    c.stroke(&letters, 12.0, 1.0, hex(OUTLINE, 1.0));
    c.fill_with(
        &letters,
        linear(
            Vec2::new(AXIS, top + 60.0),
            Vec2::new(AXIS, top + 156.0),
            &[(0.0, hex(SILVER_HI, 1.0)), (1.0, hex(SILVER, 1.0))],
        ),
        None,
    );
}

/// The shield's silver rim with its field inside.
fn shield_full(c: &mut Canvas, top: f32, point: f32, rich: bool) {
    let outer = shield(0.0, top, point);
    c.fill_with(
        &outer,
        linear(
            Vec2::new(300.0, top),
            Vec2::new(700.0, point),
            &[
                (0.0, hex(SILVER_HI, 1.0)),
                (0.55, hex(SILVER, 1.0)),
                (1.0, hex(SILVER_LO, 1.0)),
            ],
        ),
        None,
    );
    c.stroke(&outer, 6.0, 1.0, hex(OUTLINE, 1.0));
    field(c, top, point, rich);
}

// -- the banner ----------------------------------------------------------------------

/// The scroll under the shield, bowed like a smile, with the name along it.
fn banner(c: &mut Canvas, words: &Words, y: f32) {
    let centre = Vec2::new(AXIS, y - 900.0);
    let radius = 900.0;
    let half = 50.0 * DEG / 2.0;
    let (r_out, r_in) = (radius + 34.0, radius - 34.0);
    let at = |a: f32, r: f32| centre + Vec2::new(a.cos(), a.sin()) * r;
    let mid = std::f32::consts::FRAC_PI_2;
    // The tails behind: each end tucks back, and a forked tail hangs out and down.
    for side in [1.0f32, -1.0] {
        let a = mid - side * half;
        let (o, i) = (at(a, r_out), at(a, r_in));
        let back = Vec2::new(-side * 18.0, 30.0);
        let (bo, bi) = (o + back, i + back);
        let out = Vec2::new(side * 120.0, 22.0);
        let notch = (bo + bi) * 0.5 + out * 0.7;
        let tail = poly(&[bo, bi, bi + out, notch, bo + out]);
        c.fill_with(
            &tail,
            linear(
                bi,
                bi + out,
                &[(0.0, hex(0x59636E, 1.0)), (1.0, hex(SILVER_LO, 1.0))],
            ),
            None,
        );
        c.stroke(&tail, 5.0, 1.0, hex(OUTLINE, 1.0));
        let fold = poly(&[o, i, bi, bo]);
        c.fill(&fold, hex(0x39424C, 1.0));
        c.stroke(&fold, 5.0, 1.0, hex(OUTLINE, 1.0));
    }
    let mut pb = PathBuilder::new();
    const STEPS: usize = 48;
    for k in 0..=STEPS {
        let a = mid + half - 2.0 * half * k as f32 / STEPS as f32;
        let p = at(a, r_out);
        if k == 0 {
            pb.move_to(p.x, p.y);
        } else {
            pb.line_to(p.x, p.y);
        }
    }
    for k in 0..=STEPS {
        let a = mid - half + 2.0 * half * k as f32 / STEPS as f32;
        let p = at(a, r_in);
        pb.line_to(p.x, p.y);
    }
    pb.close();
    let band = pb.finish();
    c.fill_with(
        &band,
        linear(
            Vec2::new(AXIS, y - 34.0),
            Vec2::new(AXIS, y + 34.0),
            &[(0.0, hex(SILVER_HI, 1.0)), (1.0, hex(SILVER, 1.0))],
        ),
        None,
    );
    c.stroke(&band, 6.0, 1.0, hex(OUTLINE, 1.0));
    c.stroke(
        &arc_line(centre, r_out - 9.0, mid - half * 0.97, mid + half * 0.97),
        2.0,
        0.0,
        hex(SLATE, 0.8),
    );
    c.stroke(
        &arc_line(centre, r_in + 9.0, mid - half * 0.97, mid + half * 0.97),
        2.0,
        0.0,
        hex(SLATE, 0.8),
    );
    if let Some(t) = Type::new(Face::Bold, 30.0, 7.0) {
        let text = words.name.to_uppercase();
        c.fill(
            &t.arc(&text, centre, radius + 14.0, mid, false),
            hex(GUNMETAL, 1.0),
        );
    }
}

fn arc_line(centre: Vec2, r: f32, from: f32, to: f32) -> Option<Path> {
    let pts: Vec<Vec2> = (0..=40)
        .map(|k| from + (to - from) * k as f32 / 40.0)
        .map(|a| centre + Vec2::new(a.cos(), a.sin()) * r)
        .collect();
    open(&pts)
}

// -- the marks ------------------------------------------------------------------------

/// The full crest: wings, head, shield, star and world, and the name on a scroll.
pub fn crest(size: [usize; 2], words: &Words) -> Vec<u8> {
    let mut c = Canvas::new(size, Vec2::new(1000.0, 1000.0));
    let detail = c.scale > 0.25;
    wings(&mut c, SHOULDER, detail);
    head(&mut c);
    shield_full(&mut c, 400.0, 812.0, true);
    banner(&mut c, words, 872.0);
    c.into_rgba()
}

/// The crest without the scroll or the world: for small and flat uses.
pub fn insignia(size: [usize; 2]) -> Vec<u8> {
    let mut c = Canvas::new(size, Vec2::new(1000.0, 840.0));
    let detail = c.scale > 0.3;
    wings(&mut c, SHOULDER, detail);
    head(&mut c);
    shield_full(&mut c, 400.0, 812.0, false);
    c.into_rgba()
}

/// The badge: shield and wings in white, reduced until it reads at 16
/// pixels. Three broad feathers a side and no head: at that size the head
/// only blurs into the wings.
pub fn badge(size: [usize; 2]) -> Vec<u8> {
    let mut c = Canvas::new(size, Vec2::new(1000.0, 1000.0));
    let (top, point) = (300.0, 940.0);
    for side in [1.0f32, -1.0] {
        for (angle, len) in [(-58.0, 470.0), (-33.0, 440.0), (-9.0, 380.0)]
            .into_iter()
            .rev()
        {
            let root = Vec2::new(640.0, 470.0) + Vec2::from_angle(angle * DEG) * 30.0;
            let (path, _) = feather(root, angle * DEG, len, 200.0, 40.0, side);
            c.fill(&path, hex(0xFFFFFF, 1.0));
            c.erase_stroke(&path, 28.0);
        }
    }
    let outer = shield_at(-20.0, top, point, 1.25);
    c.erase_stroke(&outer, 70.0);
    c.fill(&outer, hex(0xFFFFFF, 1.0));
    // The field cut through, the star left standing in it.
    c.erase(&shield_at(52.0, top, point, 1.25));
    c.fill(&star(Vec2::new(AXIS, 570.0), 150.0), hex(0xFFFFFF, 1.0));
    c.into_rgba()
}

/// The name as type: the logotype over a rule and the full name.
pub fn wordmark(size: [usize; 2], words: &Words) -> Vec<u8> {
    let mut c = Canvas::new(size, Vec2::new(1000.0, 420.0));
    let letters = logotype(Vec2::new(AXIS, 150.0), 210.0);
    c.fill_with(
        &letters,
        linear(
            Vec2::new(AXIS, 45.0),
            Vec2::new(AXIS, 255.0),
            &[(0.0, hex(SILVER_HI, 1.0)), (1.0, hex(SILVER, 1.0))],
        ),
        None,
    );
    c.fill(
        &poly(&[
            Vec2::new(150.0, 292.0),
            Vec2::new(462.0, 292.0),
            Vec2::new(462.0, 298.0),
            Vec2::new(150.0, 298.0),
        ]),
        hex(SILVER, 0.8),
    );
    c.fill(
        &poly(&[
            Vec2::new(538.0, 292.0),
            Vec2::new(850.0, 292.0),
            Vec2::new(850.0, 298.0),
            Vec2::new(538.0, 298.0),
        ]),
        hex(SILVER, 0.8),
    );
    c.fill(&star(Vec2::new(AXIS, 295.0), 26.0), hex(ICE, 1.0));
    if let Some(t) = Type::new(Face::Bold, 40.0, 16.0) {
        c.fill(
            &t.line(&words.name.to_uppercase(), Vec2::new(AXIS, 380.0)),
            hex(SILVER, 1.0),
        );
    }
    c.into_rgba()
}

/// The logotype cut for spraying through a stencil, as on hulls.
pub fn stencil(size: [usize; 2]) -> Vec<u8> {
    let mut c = Canvas::new(size, Vec2::new(1000.0, 340.0));
    let (centre, cap) = (Vec2::new(AXIS, 150.0), 220.0);
    c.fill(&logotype(centre, cap), hex(0xE9ECEE, 1.0));
    for bridge in stencil_bridges(centre, cap) {
        c.erase(&bridge);
    }
    // Hazard ticks under it, as a hull marking would carry.
    for k in 0..9 {
        let x = 130.0 + k as f32 * 84.0;
        c.fill(
            &poly(&[
                Vec2::new(x, 292.0),
                Vec2::new(x + 44.0, 292.0),
                Vec2::new(x + 24.0, 322.0),
                Vec2::new(x - 20.0, 322.0),
            ]),
            hex(0xE9ECEE, 1.0),
        );
    }
    c.into_rgba()
}

/// A round seal: the insignia ringed by the name above and the motto below.
pub fn seal(size: [usize; 2], words: &Words) -> Vec<u8> {
    let mut c = Canvas::new(size, Vec2::new(1000.0, 1000.0));
    let centre = Vec2::splat(500.0);
    c.fill_with(
        &circle(centre, 490.0),
        radial(
            centre,
            490.0,
            &[(0.0, hex(GUNMETAL, 1.0)), (1.0, hex(GUN_DEEP, 1.0))],
        ),
        None,
    );
    c.stroke(&circle(centre, 484.0), 12.0, 1.0, hex(SILVER, 1.0));
    c.stroke(&circle(centre, 360.0), 6.0, 1.0, hex(SILVER, 1.0));
    c.stroke(&circle(centre, 468.0), 2.5, 0.0, hex(SLATE, 1.0));
    if let Some(t) = Type::new(Face::Bold, 52.0, 12.0) {
        c.fill(
            &t.arc(
                &words.name.to_uppercase(),
                centre,
                392.0,
                -std::f32::consts::FRAC_PI_2,
                true,
            ),
            hex(SILVER_HI, 1.0),
        );
        c.fill(
            &t.arc(
                &words.motto.to_uppercase(),
                centre,
                440.0,
                std::f32::consts::FRAC_PI_2,
                false,
            ),
            hex(SILVER_HI, 1.0),
        );
    }
    for side in [-1.0f32, 1.0] {
        c.fill(
            &star(centre + Vec2::new(side * 420.0, 0.0), 22.0),
            hex(ICE, 1.0),
        );
    }
    // The insignia inside the inner ring, reduced.
    c.within(Vec2::new(178.0, 222.0), 0.644, |c| {
        wings(c, SHOULDER, false);
        head(c);
        shield_full(c, 400.0, 812.0, false);
    });
    c.into_rgba()
}
