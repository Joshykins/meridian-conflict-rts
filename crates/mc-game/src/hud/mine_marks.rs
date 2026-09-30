//! Core mines on the battlefield while the survey is up (placing a mine,
//! a mine selected, or reclaim shown): each mine's territory, its sonar
//! rings and workings underground, the cards on it and its ore fields, and
//! what a mine would make at the site under the pointer.

use super::mine_coast::{at_sea, Survey};
use super::{Scene, MASS};
use crate::game::Mode;
use crate::ui::{palette, rgb, type_scale, Rect, Ui};
use glam::{Vec2, Vec3};
use mc_data::{BlueprintId, Blueprints, UnitBlueprint};
use mc_map::MapFile;
use mc_sim::mirror::{UnitInstance, KIND_GHOST, KIND_WRECK};
use mc_sim::tables::OrderKind;
use std::f32::consts::TAU;

/// The middle of every ore field (the mean of its corners), in map order.
fn ore_centres(map: &MapFile) -> Vec<Vec2> {
    map.ore_regions()
        .iter()
        .map(|r| {
            let sum: Vec2 = r.points.iter().map(|p| Vec2::from(p.to_f32())).sum();
            sum / r.points.len().max(1) as f32
        })
        .collect()
}

/// A core mine in sight: where, its reach, its id, and whether it stands in the sea.
pub(super) struct Sighted {
    pub(super) at: Vec2,
    pub(super) reach: f32,
    pub(super) id: u32,
    pub(super) sea: bool,
}

/// Core mines in sight (or remembered). Anyone's; a mine's territory does not
/// care whose the next one is.
pub(super) fn mines_in_sight(
    map: &MapFile,
    blueprints: &Blueprints,
    units: &[UnitInstance],
) -> Vec<Sighted> {
    units
        .iter()
        .filter(|u| u.owner_flags & (KIND_WRECK | KIND_GHOST) == 0)
        .filter_map(|u| {
            let m = blueprints.unit(BlueprintId(u.blueprint as u16)).mine?;
            let at = Vec2::new(u.pos[0], u.pos[1]);
            let sea = at_sea(map, at);
            Some(Sighted {
                at,
                reach: m.reach_on(sea).to_f32(),
                id: u.unit_id,
                sea,
            })
        })
        .collect()
}

/// Which ore fields, in map order, a mine the viewer has seen is working:
/// its middle lies in some mine's reach.
pub fn ore_tapped(map: &MapFile, blueprints: &Blueprints, units: &[UnitInstance]) -> Vec<bool> {
    let mines = mines_in_sight(map, blueprints, units);
    ore_centres(map)
        .into_iter()
        .map(|c| {
            let sea = at_sea(map, c);
            mines
                .iter()
                .any(|m| m.sea == sea && m.at.distance(c) < m.reach)
        })
        .collect()
}

/// Whether `at` falls to the mine at `centre` with `reach` among `all`
/// (itself included): the least `distance^2 - reach^2`, as the sim divides ground.
fn owns(at: Vec2, centre: Vec2, reach: f32, all: &[(Vec2, f32)]) -> bool {
    let mine = at.distance_squared(centre) - reach * reach;
    mine <= 0.0
        && all
            .iter()
            .all(|(p, r)| at.distance_squared(*p) - r * r >= mine - 1e-3)
}

pub(super) const TERRITORY_SEGMENTS: usize = 240;

/// Overlay vertices the mine survey may use; the rest are the panels'.
const SURVEY_BUDGET: usize = mc_render::overlay::MAX_OVERLAY_VERTICES / 2;

/// A mine's territory: its circle, cut by a straight line toward every
/// neighbour it overlaps (the line through the two points where the circles
/// cross). Points round it on the ground, closed.
pub(super) fn territory(centre: Vec2, reach: f32, others: &[(Vec2, f32)]) -> Vec<Vec2> {
    (0..=TERRITORY_SEGMENTS)
        .map(|i| {
            let a = i as f32 / TERRITORY_SEGMENTS as f32 * TAU;
            let u = Vec2::new(a.cos(), a.sin());
            let mut t = reach;
            for &(c, r) in others {
                let d = c - centre;
                let k = u.dot(d);
                if k <= 1e-4 || d.length_squared() < 1e-3 {
                    continue;
                }
                let limit = (d.length_squared() - r * r + reach * reach) / (2.0 * k);
                t = t.min(limit.max(0.0));
            }
            centre + u * t
        })
        .collect()
}

/// How a mine's territory reads at its tier, so a glance tells them apart:
/// short sparse dashes at T1, long ones at T2, a solid double edge at T3 and
/// the same heavier at T4, the fill deepening and the orange running hotter
/// as they climb.
struct TierLook {
    width: f32,
    /// Dash length and period, points; `None` is a solid line.
    dash: Option<(f32, f32)>,
    /// A second, thinner edge just inside the first.
    inner: bool,
    /// Times the base fill.
    fill: f32,
    /// Sonar rings sweeping out at once.
    rings: usize,
    /// How far the orange runs toward white.
    heat: f32,
}

fn tier_look(tier: u8) -> TierLook {
    match tier {
        0 | 1 => TierLook {
            width: 1.1,
            dash: Some((5.0, 13.0)),
            inner: false,
            fill: 0.5,
            rings: 1,
            heat: 0.0,
        },
        2 => TierLook {
            width: 1.7,
            dash: Some((16.0, 21.0)),
            inner: false,
            fill: 1.1,
            rings: 2,
            heat: 0.18,
        },
        3 => TierLook {
            width: 2.3,
            dash: None,
            inner: true,
            fill: 1.7,
            rings: 2,
            heat: 0.36,
        },
        _ => TierLook {
            width: 3.0,
            dash: None,
            inner: true,
            fill: 2.4,
            rings: 3,
            heat: 0.52,
        },
    }
}

/// `tone` run `k` of the way toward white.
fn heat(tone: u32, k: f32) -> u32 {
    let ch = |shift: u32| {
        let c = ((tone >> shift) & 0xFF) as f32;
        ((c + (255.0 - c) * k).round() as u32) << shift
    };
    ch(16) | ch(8) | ch(0)
}

/// Where a point `depth` metres under the ground at `xy` sits in the world.
fn underground(s: &Scene, xy: Vec2, depth: f32) -> Vec3 {
    xy.extend(overview_height(s.map, xy) - depth)
}

/// Pixels per metre at a point of the world, in interface points.
fn px_per_metre(s: &Scene, scale: f32, world: Vec3) -> f32 {
    s.camera.projection_scale() / s.camera.eye().distance(world).max(1.0) / scale
}

/// A shaded tube through the world from `a` to `b`, radius in metres at each
/// end: a dark translucent sheath, a bright core and a thin glint, so it reads
/// as something round and solid. Never thinner than `min` points.
fn tube(
    ui: &mut Ui,
    s: &Scene,
    (a, ra): (Vec3, f32),
    (b, rb): (Vec3, f32),
    min: f32,
    tone: u32,
    alpha: f32,
) {
    let scale = ui.s;
    let (Some(pa), Some(pb)) = (s.camera.project(a), s.camera.project(b)) else {
        return;
    };
    let (pa, pb) = (pa / scale, pb / scale);
    let d = (pb - pa).normalize_or_zero();
    if d == Vec2::ZERO {
        return;
    }
    let wa = (ra * px_per_metre(s, scale, a)).max(min * 0.5);
    let wb = (rb * px_per_metre(s, scale, b)).max(min * 0.5);
    // Running off the screen: only the part on it, as thick as it is there,
    // and no rounded end where it was cut.
    let (lo, hi) = screen_box(ui, s);
    let Some((ca, cb)) = clip(pa, pb, lo, hi) else {
        return;
    };
    let length = pa.distance(pb);
    let (cut_a, cut_b) = (ca != pa, cb != pb);
    let width_at = |p: Vec2| wa + (wb - wa) * (pa.distance(p) / length).clamp(0.0, 1.0);
    let (wa, wb, pa, pb) = (width_at(ca), width_at(cb), ca, cb);
    // Shaded across like a lit cylinder: a bright core easing out through
    // the sheath to a soft rim a point wide, never a hard step.
    let c = |a: f32| rgb(tone, alpha * a);
    let clear = rgb(tone, 0.0);
    let body = [
        (0.0, 0.0, c(0.9)),
        (0.4, 0.0, c(0.82)),
        (0.62, 0.0, c(0.38)),
        (1.0, -0.5, c(0.22)),
        (1.0, 0.5, clear),
    ];
    let mut across: Vec<(f32, f32, crate::ui::Color)> = body
        .iter()
        .rev()
        .map(|&(k, px, col)| (-k, -px, col))
        .collect();
    across.extend_from_slice(&body[1..]);
    ui.ribbon(pa, pb, wa, wb, &across);
    // Round ends, so a shaft meets its drifts in a knuckle rather than a corner.
    if !cut_a {
        ui.ribbon_cap(pa, -d, wa, &body);
    }
    if !cut_b {
        ui.ribbon_cap(pb, d, wb, &body);
    }
    // The glint, soft on both sides.
    let glint = rgb(0xFFFFFF, alpha * 0.35);
    let edge = rgb(0xFFFFFF, 0.0);
    ui.ribbon(
        pa,
        pb,
        wa,
        wb,
        &[
            (0.18, -0.5, edge),
            (0.26, 0.0, glint),
            (0.38, 0.0, glint),
            (0.46, 0.5, edge),
        ],
    );
}

/// A dashed line through the world, the dashes marching along with time.
/// The part of the segment `a`-`b` inside the rectangle `lo`..`hi`, if any
/// (Liang-Barsky). World marks near the camera project far off screen; drawn
/// whole they would cost the overlay its vertex budget and the frame its time.
fn clip(a: Vec2, b: Vec2, lo: Vec2, hi: Vec2) -> Option<(Vec2, Vec2)> {
    let d = b - a;
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for (p, q) in [
        (-d.x, a.x - lo.x),
        (d.x, hi.x - a.x),
        (-d.y, a.y - lo.y),
        (d.y, hi.y - a.y),
    ] {
        if p.abs() < 1e-6 {
            if q < 0.0 {
                return None;
            }
        } else {
            let r = q / p;
            if p < 0.0 {
                t0 = t0.max(r);
            } else {
                t1 = t1.min(r);
            }
            if t0 > t1 {
                return None;
            }
        }
    }
    Some((a + d * t0, a + d * t1))
}

/// The screen, a little larger, in interface points.
fn screen_box(ui: &Ui, s: &Scene) -> (Vec2, Vec2) {
    let size = s.camera.viewport / ui.s;
    (Vec2::splat(-40.0), size + 40.0)
}

/// A line through interface points, cut to the screen, with mitred joins:
/// a gap (`None`) or a cut at the screen's edge starts a new run.
fn smooth(ui: &mut Ui, s: &Scene, points: &[Option<Vec2>], width: f32, color: crate::ui::Color) {
    let (lo, hi) = screen_box(ui, s);
    let mut run: Vec<Vec2> = Vec::new();
    let closed = points.len() > 2
        && matches!((points[0], points[points.len() - 1]), (Some(a), Some(b)) if a.distance(b) < 0.01);
    let mut whole = true;
    for pair in points.windows(2) {
        let cut = match (pair[0], pair[1]) {
            (Some(a), Some(b)) => clip(a, b, lo, hi).map(|(p, q)| (p, q, p != a, q != b)),
            _ => None,
        };
        match cut {
            Some((p, q, cut_start, cut_end)) => {
                if cut_start || run.is_empty() {
                    if run.len() > 1 {
                        ui.polyline(&run, width, color, false);
                    }
                    run.clear();
                    run.push(p);
                }
                run.push(q);
                if cut_end {
                    whole = false;
                    ui.polyline(&run, width, color, false);
                    run.clear();
                }
                whole &= !cut_start;
            }
            None => {
                whole = false;
                if run.len() > 1 {
                    ui.polyline(&run, width, color, false);
                }
                run.clear();
            }
        }
    }
    if run.len() > 1 {
        ui.polyline(&run, width, color, closed && whole);
    }
}

/// What a mine has still to build: light grey.
const PLANNED: u32 = 0xC8C8C4;

/// A mine's unbuilt workings: grey dashes marching, every fourth one in the
/// materials red-orange, so the plan reads as a plan and not as the real thing.
fn planned(ui: &mut Ui, s: &Scene, points: &[Vec3], width: f32, alpha: f32, phase: f32) {
    dashed(
        ui,
        s,
        points,
        width,
        rgb(PLANNED, alpha),
        phase,
        Some(rgb(MASS, alpha.max(0.6))),
    );
}

fn dashed(
    ui: &mut Ui,
    s: &Scene,
    points: &[Vec3],
    width: f32,
    color: crate::ui::Color,
    phase: f32,
    accent: Option<crate::ui::Color>,
) {
    dashed_by(ui, s, points, width, color, phase, accent, 8.0, 14.0);
}

/// `dashed` with dashes `on` points long every `period` points.
fn dashed_by(
    ui: &mut Ui,
    s: &Scene,
    points: &[Vec3],
    width: f32,
    color: crate::ui::Color,
    phase: f32,
    accent: Option<crate::ui::Color>,
    on: f32,
    period: f32,
) {
    let scale = ui.s;
    let mut run = 0.0f32;
    let mut last = points
        .first()
        .and_then(|&p| s.camera.project(p))
        .map(|p| p / scale);
    for &p in &points[1..] {
        let next = s.camera.project(p).map(|q| q / scale);
        if let (Some(a0), Some(b0)) = (last, next) {
            let full = a0.distance(b0);
            let (lo, hi) = screen_box(ui, s);
            let Some((a, b)) = clip(a0, b0, lo, hi) else {
                run += full;
                last = next;
                continue;
            };
            let skipped = a0.distance(a);
            let len = a.distance(b);
            // Dashes `on` points of every `period`, measured from the unclipped start.
            let mut t = 0.0;
            let run = run + skipped;
            let mut dashes = 0;
            let dash_on = on;
            while t < len && dashes < 400 {
                dashes += 1;
                let at = (run + t + phase) % period;
                let on = (dash_on - at).max(0.0).min(len - t);
                if at < dash_on && on > 0.0 {
                    let dash = ((run + t + phase) / period).floor() as i64;
                    let tone = match accent {
                        Some(c) if dash.rem_euclid(4) == 0 => c,
                        _ => color,
                    };
                    ui.stroke(a.lerp(b, t / len), a.lerp(b, (t + on) / len), width, tone);
                }
                t += if at < dash_on {
                    on.max(0.5)
                } else {
                    (period - at).max(0.5)
                };
            }
        }
        if let (Some(a), Some(b)) = (last, next) {
            run += a.distance(b);
        }
        last = next;
    }
}

/// The mine survey: while a core mine is placed or selected, or Ctrl is held.
/// The ore shows as veins deep underground; every mine in sight shows its
/// territory (a dim fill with sonar pulses and a marching edge, cut straight
/// where it meets a neighbour) and its workings: a main shaft straight down,
/// thicker with the tier, and a drift out to each field at that field's
/// depth, dug over time, the ore flowing back once it arrives. A mine being
/// placed shows the workings it would dig, and what it would make.
pub(super) fn mine_marks(ui: &mut Ui, s: &Scene, survey: &mut Survey) {
    let placing = match s.view.mode {
        Mode::Place(bp) => s.blueprints.unit(bp).mine.map(|m| (bp, m)),
        _ => None,
    };
    let selected: Vec<u32> = s
        .view
        .frame
        .units
        .iter()
        .filter(|u| s.view.selection.contains(&u.unit_id) && s.bp(u).mine.is_some())
        .map(|u| u.unit_id)
        .collect();
    // A builder with a mine still to start in its queue brings the survey up
    // too: the grid it is laying out reads only against the others' reach.
    let ordering = s
        .view
        .status
        .queues
        .iter()
        .filter(|q| s.view.selection.contains(&q.unit_id))
        .flat_map(|q| &q.orders)
        .any(|o| o.kind == OrderKind::Build && s.blueprints.unit(o.blueprint).mine.is_some());
    if placing.is_none() && selected.is_empty() && !ordering && !s.show_reclaim {
        return;
    }
    let time = ui.time;
    let far = ((s.camera.distance - 1200.0) / 5000.0).clamp(0.0, 1.0);
    let fields: Vec<(Vec2, f32)> = s
        .map
        .ore_regions()
        .iter()
        .map(|r| (Vec2::from(r.centre().to_f32()), r.depth().to_f32()))
        .collect();

    let mut mines = built_sites(s);
    let planned = planned_sites(s, &mines);
    mines.extend(planned);
    let ghost = placing.and_then(|(bp, m)| {
        let at = Vec2::from(s.placing?.to_f32());
        let sea = at_sea(s.map, at);
        Some(Site {
            at,
            reach: m.reach_on(sea).to_f32(),
            sea,
            id: u32::MAX,
            tier: s.blueprints.unit(bp).tech,
            age: None,
            spread: 0.0,
            kind: SiteKind::Ghost,
        })
    });
    let ghost_at = ghost.as_ref().map(|g| g.at);
    mines.extend(ghost);
    for plan in plan_surveys(ui, s, survey, &mines, &selected) {
        // Only if the estimates were far out: the panels drawn next need room.
        if ui.o.vertices.len() > mc_render::overlay::MAX_OVERLAY_VERTICES * 3 / 4 {
            break;
        }
        let site = &mines[plan.i];
        let strength = match site.kind {
            SiteKind::Ghost => 1.0,
            SiteKind::Planned => 0.8,
            SiteKind::Built if plan.lit => 1.0,
            SiteKind::Built => 0.6,
        };
        draw_territory(ui, s, site, &plan, strength, far, time);
        if plan.full {
            // Mines of the other kind work other ground: they take none of its ore.
            let all: Vec<(Vec2, f32)> = mines
                .iter()
                .filter(|m| m.sea == site.sea)
                .map(|m| (m.at, m.reach))
                .collect();
            draw_workings(ui, s, site, &fields, &all, strength, far, time);
        }
    }

    // The deposits themselves are real geometry the renderer draws through
    // the ground (`ore_vein_mesh`, `fs_vein`) while the survey is up.
    mine_cards(ui, s, &selected, &fields, time);

    // What the mine under the pointer would make there.
    let (Some((bp, mine)), Some(fx), Some(site)) = (placing, s.placing, ghost_at) else {
        return;
    };
    // Its neighbours, the planned ones too: the land it would have once they stand.
    let others: Vec<(Vec2, f32)> = mines
        .iter()
        .filter(|m| m.kind != SiteKind::Ghost)
        .map(|m| (m.at, m.reach))
        .collect();
    // The sim leaves out neighbours of the other kind itself.
    ghost_readout(ui, s, survey, (bp, mine), (fx, site), &others);
}

/// A core mine the survey draws.
struct Site {
    at: Vec2,
    /// Its sea reach if it stands in the sea.
    reach: f32,
    /// It stands in the sea and works the sea, sharing only with mines at sea.
    sea: bool,
    id: u32,
    tier: u8,
    /// Seconds it has been digging; unknown (anyone else's) counts as long done.
    age: Option<f32>,
    /// Metres out the land (or sea) it works reaches so far.
    spread: f32,
    kind: SiteKind,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SiteKind {
    /// Standing, finished or still being built.
    Built,
    /// Queued with a builder of the viewer's side and not started.
    Planned,
    /// The one being placed, under the pointer.
    Ghost,
}

/// Mines in sight, anyone's.
fn built_sites(s: &Scene) -> Vec<Site> {
    s.view
        .frame
        .units
        .iter()
        .filter(|u| u.owner_flags & (KIND_WRECK | KIND_GHOST) == 0)
        .filter_map(|u| {
            let bp = s.bp(u);
            let m = bp.mine?;
            let view = s.queue_of(u).and_then(|q| q.mine);
            let at = Vec2::new(u.pos[0], u.pos[1]);
            let sea = at_sea(s.map, at);
            Some(Site {
                at,
                reach: m.reach_on(sea).to_f32(),
                sea,
                id: u.unit_id,
                tier: bp.tech,
                age: Some(view.map_or(1.0e9, |v| v.age)),
                spread: view.map_or(1.0e9, |v| v.spread),
                kind: SiteKind::Built,
            })
        })
        .collect()
}

/// Mines the viewer's side has queued and not started: each once, though a
/// group's builders all carry it, and none where a site already stands.
fn planned_sites(s: &Scene, built: &[Site]) -> Vec<Site> {
    let mut planned: Vec<Site> = Vec::new();
    for o in s.view.status.queues.iter().flat_map(|q| &q.orders) {
        if o.kind != OrderKind::Build {
            continue;
        }
        let bp = s.blueprints.unit(o.blueprint);
        let Some(m) = bp.mine else {
            continue;
        };
        let at = Vec2::from(o.at.to_f32());
        if built
            .iter()
            .chain(&planned)
            .any(|b| b.at.distance(at) < 2.0)
        {
            continue;
        }
        let sea = at_sea(s.map, at);
        planned.push(Site {
            at,
            reach: m.reach_on(sea).to_f32(),
            sea,
            // Below the ghost's, one each, so the draw order stays put.
            id: u32::MAX - 1 - planned.len() as u32,
            tier: bp.tech,
            age: None,
            spread: 0.0,
            kind: SiteKind::Planned,
        });
    }
    planned
}

/// One mine's survey as settled for this frame.
struct Plan {
    i: usize,
    lit: bool,
    /// The territory, every few points of screen.
    outline: Vec<Vec2>,
    /// The same, coarser, for the rings and the inner edge.
    coarse: Vec<Vec2>,
    /// Sonar rings and workings, not only the edge and fill.
    full: bool,
}

/// Which mines get drawn, and which of them the full survey.
///
/// Late in a match there are dozens of mines, each with thousands of
/// vertices of survey, more than the overlay can spare. Which ones get
/// the full survey (sonar rings, workings) is settled up front from what
/// each would cost, which only the camera changes: the lit ones first,
/// then outward from the middle of the screen. The rest keep their fill
/// and tier edge, and only past even that do the farthest drop out. A
/// cut decided by the vertices actually drawn moved every frame with the
/// rings and dashes, and the mines at the end of the list flickered.
/// Planned mines never get the full survey: their edge is the plan.
fn plan_surveys(
    ui: &Ui,
    s: &Scene,
    survey: &mut Survey,
    mines: &[Site],
    selected: &[u32],
) -> Vec<Plan> {
    let scale = ui.s;
    let viewport = s.camera.viewport / scale;
    let middle = viewport * 0.5;
    let mut plans: Vec<(Plan, f32, usize, usize)> = Vec::new();
    for (i, site) in mines.iter().enumerate() {
        // Off screen: nothing of it shows (its cards are placed separately).
        let centre = site.at.extend(overview_height(s.map, site.at));
        let reach_px = site.reach * px_per_metre(s, scale, centre);
        let c = match s.camera.project(centre).map(|p| p / scale) {
            Some(c) if on_screen(viewport, c, reach_px * 1.5 + 200.0) => c,
            _ => continue,
        };
        // Only mines of its own kind: land mines and sea mines work different ground.
        let others: Vec<(Vec2, f32)> = mines
            .iter()
            .enumerate()
            .filter(|&(j, m)| {
                j != i && m.sea == site.sea && m.at.distance(site.at) < site.reach + m.reach
            })
            .map(|(_, m)| (m.at, m.reach))
            .collect();
        // Zoomed out a territory is small on screen: keep its segments a few
        // points long rather than drawing 240 of them. Every stride divides
        // 240, so the stepped outline still closes.
        let per_segment = TAU * reach_px / TERRITORY_SEGMENTS as f32;
        let strides = [24, 20, 16, 15, 12, 10, 8, 6, 5, 4, 3, 2, 1];
        let stride = strides
            .into_iter()
            .find(|&k| per_segment * k as f32 <= 6.0)
            .unwrap_or(1);
        let coarse_stride = strides
            .into_iter()
            .rev()
            .find(|&k| k % stride == 0 && per_segment * k as f32 >= 14.0)
            .unwrap_or(24);
        // Each edge a few points inside its own side, so where two meet both
        // lines show side by side, each in its own tier's look.
        let inset = 3.0 * site.reach / reach_px.max(1.0);
        let whole: Vec<Vec2> = survey
            .on_own_ground(
                s,
                site.at,
                site.reach,
                territory(site.at, site.reach, &others),
            )
            .into_iter()
            .map(|p| {
                let d = p - site.at;
                site.at + d.normalize_or_zero() * (d.length() - inset).max(0.0)
            })
            .collect();
        let outline: Vec<Vec2> = whole.iter().copied().step_by(stride).collect();
        let coarse: Vec<Vec2> = whole.into_iter().step_by(coarse_stride).collect();
        let look = tier_look(site.tier);
        let edge = outline.len() * 24 + if look.inner { coarse.len() * 18 } else { 0 };
        let extra = look.rings * coarse.len() * 18
            + if site.spread < site.reach {
                outline.len() * 18
            } else {
                0
            }
            + 3000;
        let lit = site.kind == SiteKind::Ghost || selected.contains(&site.id);
        let off_middle = if lit { -1.0 } else { c.distance(middle) };
        plans.push((
            Plan {
                i,
                lit,
                outline,
                coarse,
                full: false,
            },
            off_middle,
            edge,
            extra,
        ));
    }
    plans.sort_by(|a, b| {
        a.1.total_cmp(&b.1)
            .then(mines[a.0.i].id.cmp(&mines[b.0.i].id))
    });
    // Every mine's edge first, then as many full surveys as still fit. A
    // fixed allowance, not what the marks drawn before happen to leave: those
    // move with the units, and the cut would move with them.
    let mut left = SURVEY_BUDGET;
    let mut kept = 0;
    for (_, _, edge, _) in &plans {
        if *edge > left {
            break;
        }
        left -= edge;
        kept += 1;
    }
    plans.truncate(kept);
    for (plan, _, _, extra) in &mut plans {
        if mines[plan.i].kind != SiteKind::Planned && *extra <= left {
            left = left.saturating_sub(*extra);
            plan.full = true;
        }
    }
    plans.into_iter().map(|(plan, ..)| plan).collect()
}

/// The territory: a dim fill, then sonar rings sweeping out from the mine
/// to its edge, then the edge itself in its tier's line. A mine not yet
/// working (placed, planned, or still spreading) edges it in marching dashes.
fn draw_territory(
    ui: &mut Ui,
    s: &Scene,
    site: &Site,
    plan: &Plan,
    strength: f32,
    far: f32,
    time: f32,
) {
    let scale = ui.s;
    let viewport = s.camera.viewport / scale;
    let (outline, coarse) = (&plan.outline, &plan.coarse);
    let look = tier_look(site.tier);
    let tone = if site.kind == SiteKind::Built {
        heat(MASS, look.heat)
    } else {
        PLANNED
    };
    // Close in the territory is bigger than the screen: only the edge.
    if far > 0.15 {
        if let Some(c) = ground(s, scale, site.at) {
            let pts: Vec<Option<Vec2>> = outline.iter().map(|&p| ground(s, scale, p)).collect();
            let shade = rgb(tone, 0.035 * look.fill * strength * (1.0 + far));
            let near = |p: Vec2| p.x.abs() < viewport.x * 3.0 && p.y.abs() < viewport.y * 3.0;
            if near(c) && pts.iter().all(|p| p.is_some_and(near)) {
                for pair in pts.windows(2) {
                    if let (Some(a), Some(b)) = (pair[0], pair[1]) {
                        ui.triangle(c, a, b, shade);
                    }
                }
            }
        }
    }
    // The land it works so far: its territory cut to the spread, a solid line.
    let worked_r = site.spread.min(site.reach);
    if plan.full && worked_r < site.reach {
        let worked: Vec<Vec3> = outline
            .iter()
            .map(|&edge| {
                let d = edge - site.at;
                let p = site.at + d.normalize_or_zero() * d.length().min(worked_r);
                p.extend(overview_height(s.map, p) + 1.5)
            })
            .collect();
        let pts: Vec<Option<Vec2>> = worked
            .iter()
            .map(|&p| s.camera.project(p).map(|q| q / scale))
            .collect();
        smooth(ui, s, &pts, 1.8 + far * 1.6, rgb(tone, 0.8 * strength));
    }
    for k in 0..if plan.full { look.rings } else { 0 } {
        let t = (time / 5.0 + k as f32 / look.rings as f32 + (site.id % 97) as f32 * 0.17).fract();
        let r = worked_r * t;
        let ring: Vec<Option<Vec2>> = coarse
            .iter()
            .map(|&edge| {
                let d = edge - site.at;
                (d.length() > r)
                    .then(|| ground(s, scale, site.at + d.normalize_or_zero() * r))
                    .flatten()
            })
            .collect();
        let a = 0.35 * strength * (1.0 - t) * t * 4.0;
        smooth(ui, s, &ring, 1.2 + far * 1.5, rgb(tone, a));
    }
    let lift = |p: Vec2| p.extend(overview_height(s.map, p) + 1.5);
    let edge: Vec<Vec3> = outline.iter().map(|&p| lift(p)).collect();
    let width = look.width + far * 1.6;
    if site.spread < site.reach {
        planned(ui, s, &edge, width, 0.6 * strength, time * 18.0);
    } else if let Some((on, period)) = look.dash {
        dashed_by(
            ui,
            s,
            &edge,
            width,
            rgb(tone, 0.55 * strength),
            time * 18.0,
            None,
            on,
            period,
        );
    } else {
        let pts: Vec<Option<Vec2>> = edge
            .iter()
            .map(|&p| s.camera.project(p).map(|q| q / scale))
            .collect();
        smooth(ui, s, &pts, width, rgb(tone, 0.6 * strength));
    }
    // The top tiers' second, inner edge: a double border reads as rank.
    if look.inner {
        let inset: Vec<Option<Vec2>> = coarse
            .iter()
            .map(|&p| {
                s.camera
                    .project(lift(site.at + (p - site.at) * 0.94))
                    .map(|q| q / scale)
            })
            .collect();
        smooth(ui, s, &inset, 1.0 + far, rgb(tone, 0.35 * strength));
    }
}

/// The workings: a main shaft down, drifts out at each field's depth.
#[expect(
    clippy::too_many_arguments,
    reason = "one site's drawing, pulled out of mine_marks with the frame's values it reads"
)]
fn draw_workings(
    ui: &mut Ui,
    s: &Scene,
    site: &Site,
    fields: &[(Vec2, f32)],
    all: &[(Vec2, f32)],
    strength: f32,
    far: f32,
    time: f32,
) {
    let scale = ui.s;
    let owned: Vec<usize> = fields
        .iter()
        .enumerate()
        .filter(|&(_, &(c, _))| owns(c, site.at, site.reach, all))
        .map(|(f, _)| f)
        .collect();
    let bottom = owned.iter().map(|&f| fields[f].1).fold(0.0f32, f32::max);
    if bottom <= 0.0 {
        return;
    }
    let shaft_r = [5.0, 8.0, 12.0, 16.0][(site.tier.clamp(1, 4) - 1) as usize];
    let top = underground(s, site.at, 0.0);
    let shaft_speed = mc_sim::mines::SHAFT_SPEED as f32;
    let drift_speed = mc_sim::mines::DRIFT_SPEED as f32;
    let dug = site.age.map_or(0.0, |age| (age * shaft_speed).min(bottom));
    let alpha = 0.9 * strength;
    // What is still to dig: a dashed plan.
    if dug < bottom {
        planned(
            ui,
            s,
            &[
                underground(s, site.at, dug),
                underground(s, site.at, bottom),
            ],
            1.4,
            0.6 * strength,
            time * 12.0,
        );
    }
    if dug > 0.0 {
        let foot = underground(s, site.at, dug);
        tube(
            ui,
            s,
            (top, shaft_r),
            (foot, shaft_r),
            2.5 + site.tier as f32,
            MASS,
            alpha,
        );
        if dug < bottom {
            let pulse = 0.5 + 0.5 * (time * 6.0).sin();
            if let Some(p) = s.camera.project(foot) {
                ui.disc(p / scale, 3.0 + 3.0 * pulse, rgb(MASS, 0.9));
                ui.arc(
                    p / scale,
                    8.0 + 6.0 * pulse,
                    0.0,
                    TAU,
                    1.2,
                    rgb(MASS, 0.6 * (1.0 - pulse)),
                );
            }
        }
    }
    for &f in &owned {
        let (c, depth) = fields[f];
        let from = underground(s, site.at, depth);
        let to = underground(s, c, depth);
        let length = site.at.distance(c);
        // Seconds since the shaft reached this depth, as far as we know.
        let driven = site.age.map_or(0.0, |age| {
            ((age - depth / shaft_speed) * drift_speed).clamp(0.0, length)
        });
        let drift_r = shaft_r * 0.55;
        if driven < length {
            let head = from.lerp(to, if length > 0.0 { driven / length } else { 1.0 });
            planned(ui, s, &[head, to], 1.2, 0.55 * strength, time * 12.0);
            if driven > 0.0 {
                tube(ui, s, (from, drift_r), (head, drift_r), 2.0, MASS, alpha);
                let pulse = 0.5 + 0.5 * (time * 6.0 + f as f32).sin();
                if let Some(p) = s.camera.project(head) {
                    ui.disc(p / scale, 2.5 + 2.5 * pulse, rgb(MASS, 0.9));
                }
            }
        } else {
            tube(ui, s, (from, drift_r), (to, drift_r), 2.0, MASS, alpha);
            // Reached: ore running back along the drift and up the shaft.
            let path = [to, from, top];
            let legs = [length.max(1.0), depth.max(1.0)];
            let total = legs[0] + legs[1];
            for k in 0..6 {
                let t = ((time * 60.0 / total) + k as f32 / 6.0 + f as f32 * 0.13).fract() * total;
                let (a, b, u) = if t < legs[0] {
                    (path[0], path[1], t / legs[0])
                } else {
                    (path[1], path[2], (t - legs[0]) / legs[1])
                };
                if let Some(p) = s.camera.project(a.lerp(b, u)) {
                    ui.disc(p / scale, 2.2 + far, rgb(0xFFFFFF, 0.8));
                }
            }
        }
    }
}

/// The viewer's mines: a selected one gets a card of its own and one on
/// every ore field it is going for; the rest a small readout. Cards make
/// room for each other, the mines' first.
fn mine_cards(ui: &mut Ui, s: &Scene, selected: &[u32], fields: &[(Vec2, f32)], time: f32) {
    let scale = ui.s;
    let viewport = s.camera.viewport / scale;
    let mut taken: Vec<Rect> = Vec::new();
    let mut deposits: Vec<(Vec2, Option<Vec2>, mc_sim::mirror::MineVein)> = Vec::new();
    for u in &s.view.frame.units {
        let Some(q) = s.queue_of(u) else {
            continue;
        };
        let Some(view) = q.mine else {
            continue;
        };
        let at = Vec2::new(u.pos[0], u.pos[1]);
        let Some(p) = ground(s, scale, at) else {
            continue;
        };
        let bp = s.bp(u);
        if !selected.contains(&u.unit_id) {
            if on_screen(viewport, p, 60.0) {
                mine_readout(ui, p, &view);
            }
            continue;
        }
        for vein in &q.mine_veins {
            let Some(&(c, depth)) = fields.get(vein.field as usize) else {
                continue;
            };
            let Some(anchor) = ground(s, scale, c) else {
                continue;
            };
            if !on_screen(viewport, anchor, 80.0) {
                continue;
            }
            let heart = s
                .camera
                .project(underground(s, c, depth))
                .map(|p| p / scale);
            deposits.push((anchor, heart, *vein));
        }
        if on_screen(viewport, p, 120.0) {
            mine_card(ui, p, bp, &view, &q.mine_veins, time, &mut taken);
        }
    }
    let soonest = deposits
        .iter()
        .map(|d| d.2.eta)
        .filter(|&eta| eta > 0.0)
        .fold(f32::INFINITY, f32::min);
    for (anchor, heart, vein) in deposits {
        let next = vein.eta == soonest;
        deposit_card(ui, anchor, heart, &vein, next, time, &mut taken);
    }
}

/// What the mine under the pointer would make at `site` among `others`.
fn ghost_readout(
    ui: &mut Ui,
    s: &Scene,
    survey: &mut Survey,
    (bp, mine): (BlueprintId, mc_data::Mine),
    (fx, site): (mc_core::FxVec2, Vec2),
    others: &[(Vec2, f32)],
) {
    let grid = survey.grid(s);
    let sea = grid.at_sea(fx);
    let reach = mine.reach_on(sea);
    let others: Vec<(mc_core::FxVec2, mc_core::Fx)> = others
        .iter()
        .copied()
        .filter(|&(p, r)| p.distance(site) < reach.to_f32() + r)
        .map(|(p, r)| {
            (
                mc_core::FxVec2::new(mc_core::Fx::from_f32(p.x), mc_core::Fx::from_f32(p.y)),
                mc_core::Fx::from_f32(r),
            )
        })
        .collect();
    let share = grid.share(fx, reach, &others);
    let rate = share.rate(&mine).to_f32();
    let efficiency = share.efficiency(&mine).to_f32();
    let cost = s.blueprints.unit(bp).cost_mass.to_f32();
    let z = overview_height(s.map, site);
    let Some(p) = s.camera.project(site.extend(z)) else {
        return;
    };
    let c = p / ui.s;
    let ground = if sea { "sea" } else { "land" };
    let land = if share.ore > mc_core::Fx::ZERO {
        format!(
            "{:.0} ha of {ground}  \u{b7}  {:.1} ha of ore",
            share.ground.to_f32(),
            share.ore.to_f32()
        )
    } else {
        format!(
            "{:.0} ha of {ground}  \u{b7}  no ore",
            share.ground.to_f32()
        )
    };
    let tone = if efficiency >= 0.9 {
        palette::TEXT
    } else if efficiency >= 0.6 {
        palette::WARN
    } else {
        palette::BAD
    };
    let payback = if rate > 0.0 {
        format!("Pays back in {}", super::mine::duration(cost / rate))
    } else {
        "Never pays back".to_owned()
    };
    let lines = [
        (format!("{rate:.1} materials/s"), MASS),
        (
            format!("Efficiency {:.0}%  \u{b7}  {payback}", efficiency * 100.0),
            tone,
        ),
        (land, palette::DIM),
    ];
    let w = 236.0;
    let r = Rect::new(c.x + 40.0, c.y - 28.0, w, 56.0);
    ui.frost(r, 0.9);
    ui.fill(Rect::new(r.x, r.y, 2.0, r.h), rgb(MASS, 1.0));
    for (i, (text, tone)) in lines.iter().enumerate() {
        ui.text(
            r.x + 10.0,
            r.y + 5.0 + i as f32 * 16.0,
            type_scale::CAPTION,
            rgb(*tone, 1.0),
            text,
        );
    }
}

/// Where the ground at `xy` is on screen, points.
fn ground(s: &Scene, scale: f32, xy: Vec2) -> Option<Vec2> {
    s.camera
        .project(xy.extend(overview_height(s.map, xy) + 1.5))
        .map(|p| p / scale)
}

/// Whether `p`, points, is on screen or within `margin` of it.
fn on_screen(viewport: Vec2, p: Vec2, margin: f32) -> bool {
    p.x > -margin && p.y > -margin && p.x < viewport.x + margin && p.y < viewport.y + margin
}

/// A mine's small readout over it: what it makes, and how much of its reach it has.
fn mine_readout(ui: &mut Ui, p: Vec2, view: &mc_sim::mirror::MineView) {
    let tone = share_tone(view.share);
    let rate = format!("{:.1}/s", view.rate);
    let share = if view.rate + 0.05 < view.full {
        format!("{:.0}%  \u{b7}  growing", view.share * 100.0)
    } else {
        format!("{:.0}%", view.share * 100.0)
    };
    let w =
        26.0 + ui.text_width(type_scale::VALUE, &rate) + ui.text_width(type_scale::MICRO, &share);
    let r = Rect::new(p.x - w * 0.5, p.y - 44.0, w, 22.0);
    ui.frost(r, 0.9);
    ui.fill(Rect::new(r.x, r.y, 2.0, r.h), rgb(MASS, 1.0));
    let after = r.x + 8.0 + ui.text_width(type_scale::VALUE, &rate) + 6.0;
    ui.text(
        r.x + 8.0,
        r.y + 4.0,
        type_scale::VALUE,
        rgb(MASS, 1.0),
        &rate,
    );
    ui.text(after, r.y + 6.0, type_scale::MICRO, rgb(tone, 1.0), &share);
    ui.stroke(Vec2::new(p.x, r.bottom()), p, 1.4, rgb(MASS, 0.8));
}

fn share_tone(share: f32) -> u32 {
    if share >= 0.9 {
        palette::TEXT
    } else if share >= 0.6 {
        palette::WARN
    } else {
        palette::BAD
    }
}

/// `r` moved up, then down, until it overlaps none of `taken`; then taken.
fn make_room(mut r: Rect, taken: &mut Vec<Rect>) -> Rect {
    let hits = |r: &Rect, taken: &[Rect]| {
        taken
            .iter()
            .find(|t| {
                r.x < t.right() + 4.0
                    && t.x < r.right() + 4.0
                    && r.y < t.bottom() + 4.0
                    && t.y < r.bottom() + 4.0
            })
            .copied()
    };
    let start = r;
    for step in 0..12 {
        match hits(&r, taken) {
            None => break,
            Some(t) => {
                r = if step < 6 {
                    Rect::new(r.x, t.y - r.h - 6.0, r.w, r.h)
                } else {
                    Rect::new(start.x, t.bottom() + 6.0, r.w, r.h)
                };
            }
        }
    }
    taken.push(r);
    r
}

/// A thin bar: `k` of it filled in `fill`, the rest in the planned grey.
fn progress(ui: &mut Ui, r: Rect, k: f32, fill: u32) {
    ui.fill(r, rgb(PLANNED, 0.22));
    ui.fill(
        Rect::new(r.x, r.y, r.w * k.clamp(0.0, 1.0), r.h),
        rgb(fill, 0.95),
    );
}

/// A selected mine's card, just over it: what it makes now and, while it is
/// still growing (its land spreading, drifts on their way to the ore), what
/// it will make and when, with a bar for how far along it is. Efficiency only
/// shows when a neighbour is taking some of its reach.
fn mine_card(
    ui: &mut Ui,
    p: Vec2,
    bp: &UnitBlueprint,
    view: &mc_sim::mirror::MineView,
    veins: &[mc_sim::mirror::MineVein],
    time: f32,
    taken: &mut Vec<Rect>,
) {
    let reach = bp.mine.map_or(1.0, |m| m.reach_on(view.sea).to_f32());
    let spread = view.spread.min(reach);
    // Grown once the land is all worked and the last drift is in.
    let land_left = (reach - spread) / mc_sim::mines::spread_speed(view.sea) as f32;
    let left = veins
        .iter()
        .map(|v| v.eta)
        .fold(land_left, f32::max)
        .max(0.0);
    let growing = left > 0.5 && view.rate + 0.05 < view.full;
    let shared = view.share < 0.9;
    let h = 38.0 + if growing { 8.0 } else { 0.0 } + if shared { 14.0 } else { 0.0 };
    let w = 200.0;
    // The reticle keeps its own room, so no field's chip lands on the mine.
    taken.push(Rect::new(p.x - 24.0, p.y - 24.0, 48.0, 48.0));
    let r = make_room(Rect::new(p.x - w * 0.5, p.y - 34.0 - h, w, h), taken);
    // A reticle on the mine, turning slowly.
    for k in 0..4 {
        let a = time * 0.6 + k as f32 * std::f32::consts::FRAC_PI_2;
        let d = Vec2::new(a.cos(), a.sin());
        ui.stroke(p + d * 14.0, p + d * 22.0, 2.0, rgb(MASS, 0.9));
    }
    ui.arc(p, 17.0, 0.0, TAU, 1.2, rgb(MASS, 0.5));
    ui.stroke(
        Vec2::new(r.x + r.w * 0.5, r.bottom()),
        p - Vec2::Y * 22.0,
        1.2,
        rgb(MASS, 0.6),
    );

    ui.frost(r, 0.92);
    ui.fill(Rect::new(r.x, r.y, 3.0, r.h), rgb(MASS, 1.0));
    let x = r.x + 12.0;
    let right = r.right() - 10.0;
    ui.text(
        x,
        r.y + 8.0,
        type_scale::CAPTION,
        rgb(palette::TEXT, 1.0),
        &bp.name,
    );
    ui.text_right(
        right,
        r.y + 7.0,
        type_scale::VALUE,
        rgb(MASS, 1.0),
        &format!("{:.1}/s", view.rate),
    );
    let mut y = r.y + 26.0;
    if growing {
        let after = ui.text(x, y, type_scale::MICRO, rgb(palette::DIM, 1.0), "Grows to ");
        ui.text(
            after,
            y,
            type_scale::MICRO,
            rgb(MASS, 1.0),
            &format!("{:.1}/s", view.full),
        );
        ui.text_right(
            right,
            y,
            type_scale::MICRO,
            rgb(palette::TEXT, 0.9),
            &super::mine::duration(left),
        );
        let done = view.age / (view.age + left).max(1.0);
        progress(ui, Rect::new(x, y + 10.0, right - x, 3.0), done, MASS);
        y += 22.0;
    } else {
        ui.text(
            x,
            y,
            type_scale::MICRO,
            rgb(palette::DIM, 1.0),
            "Fully grown",
        );
        y += 14.0;
    }
    if shared {
        ui.text(
            x,
            y,
            type_scale::MICRO,
            rgb(share_tone(view.share), 1.0),
            &format!(
                "Shares its {}  \u{b7}  {:.0}% efficient",
                if view.sea { "sea" } else { "land" },
                view.share * 100.0
            ),
        );
    }
}

/// An ore field a selected mine is going for: a ring on the field that fills
/// as the drift comes, and beside it a chip of what the field adds and when
/// it starts; the soonest one brightest. Once reached, the ring is solid and
/// the chip says so.
fn deposit_card(
    ui: &mut Ui,
    p: Vec2,
    heart: Option<Vec2>,
    vein: &mc_sim::mirror::MineVein,
    next: bool,
    time: f32,
    taken: &mut Vec<Rect>,
) {
    let reached = vein.eta <= 0.0;
    let tone = if reached { MASS } else { PLANNED };
    if let Some(h) = heart {
        ui.stroke(p, h, 1.0, rgb(tone, 0.35));
    }
    let start = -std::f32::consts::FRAC_PI_2;
    if reached {
        let pulse = 0.5 + 0.5 * (time * 3.0).sin();
        ui.disc(p, 3.0, rgb(MASS, 1.0));
        ui.arc(p, 7.0, 0.0, TAU, 2.0, rgb(MASS, 1.0));
        ui.arc(
            p,
            10.0 + 4.0 * pulse,
            0.0,
            TAU,
            1.0,
            rgb(MASS, 0.5 * (1.0 - pulse)),
        );
    } else {
        let done = if vein.dig > 0.0 {
            (1.0 - vein.eta / vein.dig).clamp(0.0, 1.0)
        } else {
            1.0
        };
        ui.arc(p, 7.0, 0.0, TAU, 2.0, rgb(PLANNED, 0.45));
        if done > 0.0 {
            ui.arc(p, 7.0, start, start + TAU * done, 2.0, rgb(MASS, 1.0));
        }
    }

    let rate = format!("+{:.1}/s", vein.rate);
    let note = if reached {
        "mining".to_owned()
    } else {
        super::mine::duration(vein.eta)
    };
    let w =
        22.0 + ui.text_width(type_scale::VALUE, &rate) + ui.text_width(type_scale::MICRO, &note);
    let h = 20.0;
    // Right of the ring, else left of it, else wherever there is room.
    let beside = [
        Rect::new(p.x + 13.0, p.y - h * 0.5, w, h),
        Rect::new(p.x - 13.0 - w, p.y - h * 0.5, w, h),
    ];
    let clear = |r: &Rect| {
        !taken.iter().any(|t| {
            r.x < t.right() + 4.0
                && t.x < r.right() + 4.0
                && r.y < t.bottom() + 4.0
                && t.y < r.bottom() + 4.0
        })
    };
    let r = match beside.iter().find(|r| clear(r)) {
        Some(&r) => {
            taken.push(r);
            r
        }
        None => {
            let r = make_room(beside[0], taken);
            let end = Vec2::new(r.x, r.y + r.h * 0.5);
            ui.stroke(
                p + (end - p).normalize_or_zero() * 8.0,
                end,
                1.0,
                rgb(tone, 0.6),
            );
            r
        }
    };
    let strong = reached || next;
    ui.frost(r, if strong { 0.9 } else { 0.75 });
    ui.fill(Rect::new(r.x, r.y, 2.0, r.h), rgb(tone, 1.0));
    let after = ui.text(
        r.x + 8.0,
        r.y + 10.0,
        type_scale::VALUE,
        rgb(MASS, if strong { 1.0 } else { 0.7 }),
        &rate,
    );
    let note_tone = if reached {
        rgb(MASS, 1.0)
    } else if next {
        rgb(palette::TEXT, 0.95)
    } else {
        rgb(palette::DIM, 1.0)
    };
    ui.text(after + 6.0, r.y + 10.0, type_scale::MICRO, note_tone, &note);
}

pub(super) fn overview_height(map: &MapFile, xy: Vec2) -> f32 {
    let info = map.info();
    let (ow, oh) = map.overview_dims();
    let size = info.size_metres().to_f32();
    let u = (xy.x / size[0] * (ow - 1) as f32).clamp(0.0, (ow - 1) as f32);
    let v = (xy.y / size[1] * (oh - 1) as f32).clamp(0.0, (oh - 1) as f32);
    let (x0, y0) = (u.floor() as u32, v.floor() as u32);
    let (x1, y1) = ((x0 + 1).min(ow - 1), (y0 + 1).min(oh - 1));
    let (fx, fy) = (u - x0 as f32, v - y0 as f32);
    let o = map.overview();
    let w = ow as usize;
    let z = |x: u32, y: u32| {
        info.sample_to_height(o[y as usize * w + x as usize])
            .to_f32()
    };
    let a = z(x0, y0) * (1.0 - fx) + z(x1, y0) * fx;
    let b = z(x0, y1) * (1.0 - fx) + z(x1, y1) * fx;
    a * (1.0 - fy) + b * fy
}
