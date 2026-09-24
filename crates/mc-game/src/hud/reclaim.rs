//! Reclaim survey: hold Control and every settled wreck shows the mass left in
//! it. Nearby marks share one label, and the join distance grows with the
//! camera so a zoomed-out view collapses a field into a few sums. Up close
//! each wreck draws a leader to its label; zoomed out a group folds into one
//! bracketed field around its wrecks with a single leader and a wreck count.

use super::{Scene, MASS};
use crate::audio::Sfx;
use crate::ui::{ink, rgb, type_scale, Rect, Ui};
use glam::Vec2;
use mc_sim::mirror::{UnitInstance, KIND_GHOST, KIND_PROP, KIND_WRECK, STATE_RADAR, STATE_UNIDENTIFIED, WRECK_FALLING};
use std::f32::consts::TAU;

struct Mark {
    at: Vec2,
    mass: f32,
}

struct Cluster {
    marks: Vec<usize>,
    centre: Vec2,
    mass: f32,
}

fn merge(a: &mut Cluster, b: Cluster) {
    let mass = a.mass + b.mass;
    a.centre = if mass > 0.0 {
        (a.centre * a.mass + b.centre * b.mass) / mass
    } else {
        (a.centre + b.centre) * 0.5
    };
    a.mass = mass;
    a.marks.extend(b.marks);
}

/// Join screen-space marks whose centres sit within `radius` points.
fn cluster_marks(marks: &[Mark], radius: f32) -> Vec<Cluster> {
    let single = |(i, m): (usize, &Mark)| Cluster {
        marks: vec![i],
        centre: m.at,
        mass: m.mass,
    };
    // A big field is first binned into half-radius cells, so the pairwise
    // joining below only ever sees a screenful of cells, never every wreck.
    let mut groups: Vec<Cluster> = if marks.len() > 48 {
        let cell = (radius * 0.5).max(1.0);
        let mut bins: std::collections::HashMap<(i32, i32), Cluster> = Default::default();
        for (i, m) in marks.iter().enumerate() {
            let key = ((m.at.x / cell).floor() as i32, (m.at.y / cell).floor() as i32);
            let one = single((i, m));
            match bins.get_mut(&key) {
                Some(c) => merge(c, one),
                None => {
                    bins.insert(key, one);
                }
            }
        }
        let mut v: Vec<Cluster> = bins.into_values().collect();
        // Keep the result independent of hash order.
        v.sort_by(|a, b| a.marks[0].cmp(&b.marks[0]));
        v
    } else {
        marks.iter().enumerate().map(single).collect()
    };
    loop {
        let mut best: Option<(usize, usize, f32)> = None;
        for i in 0..groups.len() {
            for j in (i + 1)..groups.len() {
                let d = groups[i].centre.distance(groups[j].centre);
                if best.is_none_or(|(_, _, best_d)| d < best_d) {
                    best = Some((i, j, d));
                }
            }
        }
        let Some((i, j, d)) = best else {
            break;
        };
        if d > radius {
            break;
        }
        let b = groups.remove(j);
        merge(&mut groups[i], b);
    }
    groups
}

fn reclaim_mass(s: &Scene, u: &UnitInstance) -> f32 {
    let flags = u.owner_flags;
    if flags & (KIND_GHOST | KIND_PROP | STATE_RADAR | STATE_UNIDENTIFIED) != 0 {
        return 0.0;
    }
    let bp = s.bp(u);
    let cost = bp.cost_mass.to_f32();
    if flags & KIND_WRECK == 0 || u._pad == WRECK_FALLING {
        return 0.0;
    }
    (cost * bp.wreck_fraction.to_f32() * u.health).max(0.0)
}

/// Where a leader from `from` meets the label pill.
fn label_join(rect: Rect, from: Vec2) -> Vec2 {
    let centre = Vec2::new(rect.x + rect.w * 0.5, rect.mid_y());
    let delta = from - centre;
    let hx = rect.w * 0.5;
    let hy = rect.h * 0.5;
    if delta.x.abs() < 1e-3 && delta.y.abs() < 1e-3 {
        return Vec2::new(centre.x, rect.bottom());
    }
    let tx = if delta.x.abs() < 1e-4 {
        f32::INFINITY
    } else {
        hx / delta.x.abs()
    };
    let ty = if delta.y.abs() < 1e-4 {
        f32::INFINITY
    } else {
        hy / delta.y.abs()
    };
    centre + delta * tx.min(ty)
}

/// How far apart two marks may be, in points, and still share a label.
fn join_radius(camera_distance: f32) -> f32 {
    36.0 + ((camera_distance - 180.0) / 1000.0).clamp(0.0, 1.0) * 190.0
}

/// 1 up close, where every wreck gets its own leader; 0 once zoomed out far
/// enough that a group reads as one field.
fn leader_detail(camera_distance: f32) -> f32 {
    let t = ((camera_distance - 260.0) / 380.0).clamp(0.0, 1.0);
    1.0 - t * t * (3.0 - 2.0 * t)
}

pub(super) fn draw(ui: &mut Ui, s: &Scene, open: f32) {
    if open < 0.02 {
        return;
    }
    let viewport = s.camera.viewport;
    let mut marks = Vec::new();
    for u in &s.view.frame.units {
        let mass = reclaim_mass(s, u);
        if mass < 0.5 {
            continue;
        }
        let xy = Vec2::new(u.pos[0], u.pos[1]);
        let Some(p) = s.camera.project(xy.extend(u.pos[2] + 1.4)) else {
            continue;
        };
        if p.x < -80.0 || p.y < -80.0 || p.x > viewport.x + 80.0 || p.y > viewport.y + 80.0 {
            continue;
        }
        marks.push(Mark {
            at: p / ui.s,
            mass,
        });
    }
    let groups = cluster_marks(&marks, join_radius(s.camera.distance));
    let detail = leader_detail(s.camera.distance);
    let rise = 1.0 - (1.0 - open).powi(3);
    let pulse = 0.65 + 0.35 * (ui.time * 2.4).sin();
    for g in &groups {
        let n = g.marks.len();
        // A lone wreck always keeps its leader; a group trades its leaders for brackets.
        let own = if n == 1 { 1.0 } else { detail };
        let field = 1.0 - own;
        let spread = g
            .marks
            .iter()
            .map(|&i| marks[i].at.distance(g.centre))
            .fold(0.0f32, f32::max);
        let ring = spread.min(90.0) + 8.0;
        let lift_near = 18.0 + (n as f32).sqrt() * 6.0;
        let lift_far = ring + 16.0;
        let lift = lift_near + (lift_far - lift_near) * field;
        let label = g.centre + Vec2::new(0.0, -lift);
        let text = format!("+{}", super::whole(g.mass));
        let count = (n > 1).then(|| format!("{n}"));
        let tw = ui.text_width(type_scale::CAPTION, &text);
        let cw = count.as_ref().map_or(0.0, |c| ui.text_width(type_scale::CAPTION, c) + 10.0);
        let w = tw + 18.0 + cw * field;
        let r = Rect::new(label.x - w * 0.5, label.y - 9.0, w, 18.0);
        let a = rise * if g.mass >= 1.0 { 1.0 } else { 0.55 };
        for &i in &g.marks {
            let at = marks[i].at;
            if own > 0.01 {
                // Grow from the mass out to the label, and stay joined while Control is held.
                let la = a * own;
                let anchor = label_join(r, at);
                let tip = at.lerp(anchor, rise);
                let mid = tip.lerp(at, 0.55);
                ui.stroke(tip, mid, 1.15, rgb(MASS, 0.55 * la));
                ui.stroke(mid, at, 1.35, rgb(MASS, 0.9 * la));
                let tick = (ui.time * 0.55 + marks[i].mass * 0.01).fract();
                let glint = at.lerp(tip, tick);
                ui.arc(glint, 1.6, 0.0, TAU, 1.4, rgb(0xFFFFFF, 0.55 * la * pulse));
                ui.arc(at, 3.2 + pulse, 0.0, TAU, 1.3, rgb(MASS, 0.9 * la));
            }
            if field > 0.01 {
                // Zoomed out, a wreck is only a pip inside its group's brackets.
                ui.disc(at, 1.6, rgb(MASS, 0.75 * a * field));
            }
        }
        if field > 0.01 {
            let fa = a * field;
            let top = g.centre + Vec2::new(0.0, -ring);
            // Four corner brackets, not a closed ring, so a field never reads as a
            // territory or range outline; each spans a fixed length on screen.
            let radius = ring * (0.6 + 0.4 * rise);
            let half = (7.0 / radius.max(7.0)).min(0.6);
            for k in 0..4 {
                let at = TAU * (k as f32 + 0.5) / 4.0;
                ui.arc(g.centre, radius, at - half, at + half, 1.3, rgb(MASS, 0.75 * fa));
            }
            let tip = top.lerp(Vec2::new(label.x, r.bottom()), rise);
            ui.stroke(top, tip, 1.2, rgb(MASS, 0.8 * fa));
            let tick = (ui.time * 0.55 + g.mass * 0.01).fract();
            ui.arc(top.lerp(tip, tick), 1.6, 0.0, TAU, 1.4, rgb(0xFFFFFF, 0.55 * fa * pulse));
        }
        ui.fill(r, ink(0.72 * a));
        ui.fill(Rect::new(r.x, r.y, r.w, 1.0), rgb(MASS, 0.9 * a));
        ui.text(
            r.x + 9.0,
            r.mid_y(),
            type_scale::CAPTION,
            rgb(MASS, a),
            &text,
        );
        if let Some(c) = count.filter(|_| field > 0.01) {
            // How many wrecks the sum stands for, dimmer, on the right of the pill.
            ui.text_right(r.right() - 9.0, r.mid_y(), type_scale::CAPTION, rgb(0xFFFFFF, 0.5 * a * field), &c);
        }
    }
}

pub(super) fn cue(ui: &Ui, on: bool) {
    ui.audio.play_at(
        if on { Sfx::ToggleOn } else { Sfx::ToggleOff },
        0.22,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mark(x: f32, y: f32, mass: f32) -> Mark {
        Mark {
            at: Vec2::new(x, y),
            mass,
        }
    }

    #[test]
    fn nearby_mass_becomes_one_label() {
        let marks = vec![mark(0.0, 0.0, 10.0), mark(20.0, 0.0, 30.0), mark(400.0, 0.0, 5.0)];
        let groups = cluster_marks(&marks, 40.0);
        assert_eq!(groups.len(), 2);
        let joined = groups.iter().find(|g| g.marks.len() == 2).unwrap();
        assert!((joined.mass - 40.0).abs() < 0.01);
        assert!(joined.centre.x > 10.0 && joined.centre.x < 20.0);
    }

    #[test]
    fn a_leader_meets_the_label_edge() {
        let pill = Rect::new(40.0, 10.0, 80.0, 18.0);
        let below = label_join(pill, Vec2::new(80.0, 90.0));
        assert!((below.x - 80.0).abs() < 0.01);
        assert!((below.y - pill.bottom()).abs() < 0.01);
        let left = label_join(pill, Vec2::new(0.0, pill.mid_y()));
        assert!((left.x - pill.x).abs() < 0.01);
        assert!((left.y - pill.mid_y()).abs() < 0.01);
    }

    #[test]
    fn a_wider_join_collapses_a_zoomed_out_field() {
        let marks = vec![mark(0.0, 0.0, 1.0), mark(80.0, 0.0, 1.0), mark(40.0, 60.0, 1.0)];
        assert_eq!(cluster_marks(&marks, 30.0).len(), 3);
        assert_eq!(cluster_marks(&marks, 120.0).len(), 1);
    }

    #[test]
    fn a_big_field_joins_without_losing_mass() {
        let marks: Vec<Mark> = (0..400)
            .map(|i| mark((i % 20) as f32 * 9.0, (i / 20) as f32 * 9.0, 2.0))
            .collect();
        let groups = cluster_marks(&marks, 200.0);
        assert!(groups.len() <= 2, "{} groups", groups.len());
        let total: f32 = groups.iter().map(|g| g.mass).sum();
        assert!((total - 800.0).abs() < 0.1);
        assert_eq!(groups.iter().map(|g| g.marks.len()).sum::<usize>(), 400);
    }

    #[test]
    fn leaders_fade_as_the_camera_pulls_back() {
        assert_eq!(leader_detail(150.0), 1.0);
        assert_eq!(leader_detail(2000.0), 0.0);
        assert!(join_radius(2000.0) > join_radius(300.0));
    }
}
