//! Teams as the set-up screens and the observer panel show them: the
//! matchup ("2 v 2"), a numbered team badge, and sides drawn from where the
//! landing zones lie on the map.

use super::{ink, palette, rgb, type_scale, Rect, Ui};
use glam::Vec2;

/// Every seated commander on a team of their own (with more than two of them).
pub fn free_for_all(teams: &[u8]) -> bool {
    teams.len() > 2 && sizes(teams).iter().all(|&(_, n)| n == 1)
}

/// Teams in play and how many commanders each has, in team order.
pub fn sizes(teams: &[u8]) -> Vec<(u8, usize)> {
    let mut out: Vec<(u8, usize)> = Vec::new();
    for &t in teams {
        match out.iter_mut().find(|(k, _)| *k == t) {
            Some((_, n)) => *n += 1,
            None => out.push((t, 1)),
        }
    }
    out.sort_by_key(|&(t, _)| t);
    out
}

/// "2 v 2", "1 v 1 v 1", or "Free for All" when nobody is allied; past four
/// even teams, "8 Teams of 4".
pub fn matchup(teams: &[u8]) -> String {
    if free_for_all(teams) {
        return format!("Free for All \u{b7} {}", teams.len());
    }
    let s = sizes(teams);
    if s.len() > 4 && !uneven(teams) {
        return format!("{} Teams of {}", s.len(), s[0].1);
    }
    s.iter()
        .map(|(_, n)| n.to_string())
        .collect::<Vec<_>>()
        .join(" v ")
}

/// The teams differ in size (a 3 v 1, not a 2 v 2).
pub fn uneven(teams: &[u8]) -> bool {
    let s = sizes(teams);
    s.iter().any(|&(_, n)| n != s[0].1)
}

pub const BADGE_H: f32 = 18.0;

/// A small cut-cornered tag with the team's number, centred on `y`. Returns
/// its right edge. `lit` draws it filled, for the team being pointed at.
pub fn badge(ui: &mut Ui, x: f32, y: f32, team: u8, alpha: f32, lit: bool) -> f32 {
    let label = format!("T{}", team + 1);
    let w = ui.text_width(type_scale::CAPTION, &label) + 12.0;
    let r = Rect::new(x, y - BADGE_H * 0.5, w, BADGE_H);
    if lit {
        ui.fill_cut(r, 4.0, rgb(palette::TEXT, 0.92 * alpha));
    } else {
        ui.fill_cut(r, 4.0, rgb(palette::LINE, 0.9 * alpha));
        ui.fill_cut(r.inset(1.0), 3.5, ink(0.92 * alpha));
    }
    let tone = if lit {
        rgb(palette::INK, alpha)
    } else {
        rgb(palette::TEXT, alpha)
    };
    ui.text_centred(r.x + w * 0.5, y, type_scale::CAPTION, tone, &label);
    r.right()
}

/// The pairs of points to join so each team's landing zones read as one
/// side: a shortest tree over each team's points. `teams[i]` is point i's team.
pub fn links(points: &[Vec2], teams: &[u8]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    for (team, _) in sizes(teams) {
        let members: Vec<usize> = (0..points.len()).filter(|&i| teams[i] == team).collect();
        if members.len() < 2 {
            continue;
        }
        // Prim's: grow from the first member, always adding the nearest outsider.
        let mut inside = vec![members[0]];
        while inside.len() < members.len() {
            let best = members
                .iter()
                .filter(|m| !inside.contains(m))
                .flat_map(|&m| inside.iter().map(move |&k| (k, m)))
                .min_by(|a, b| {
                    let da = points[a.0].distance_squared(points[a.1]);
                    let db = points[b.0].distance_squared(points[b.1]);
                    da.total_cmp(&db)
                })
                .unwrap();
            out.push(best);
            inside.push(best.1);
        }
    }
    out
}

/// Splits `points` into `groups` teams of as-equal-as-possible size so that
/// allies land near each other: the split with the least distance between
/// team-mates. Returns each point's team, numbered in order of first point.
pub fn by_ground(points: &[Vec2], groups: usize) -> Vec<u8> {
    let n = points.len();
    let groups = groups.clamp(1, n.max(1));
    let mut sizes: Vec<usize> = (0..groups)
        .map(|g| n / groups + usize::from(g < n % groups))
        .collect();
    if n > EXACT_MAX {
        return by_arcs(points, &sizes);
    }
    let mut best = (
        f32::INFINITY,
        (0..n).map(|i| (i % groups) as u8).collect::<Vec<u8>>(),
    );
    let mut team = vec![u8::MAX; n];
    fn search(
        at: usize,
        points: &[Vec2],
        sizes: &mut [usize],
        team: &mut [u8],
        cost: f32,
        best: &mut (f32, Vec<u8>),
    ) {
        if cost >= best.0 {
            return;
        }
        if at == points.len() {
            *best = (cost, team.to_vec());
            return;
        }
        // Teams are interchangeable: the next point only opens one new team.
        let opened = team[..at]
            .iter()
            .copied()
            .filter(|&t| t != u8::MAX)
            .max()
            .map_or(0, |t| t as usize + 1);
        for g in 0..sizes.len().min(opened + 1) {
            if sizes[g] == 0 {
                continue;
            }
            let add: f32 = (0..at)
                .filter(|&k| team[k] == g as u8)
                .map(|k| points[k].distance(points[at]))
                .sum();
            sizes[g] -= 1;
            team[at] = g as u8;
            search(at + 1, points, sizes, team, cost + add, best);
            team[at] = u8::MAX;
            sizes[g] += 1;
        }
    }
    search(0, points, &mut sizes, &mut team, 0.0, &mut best);
    best.1
}

/// Past this many points the exact search in [`by_ground`] takes too long: it
/// grows like the number of ways to split them.
const EXACT_MAX: usize = 12;

/// [`by_ground`] for many points: each team takes a run of neighbours around
/// the middle (on a ring of starts, an arc of the ring), from whichever first
/// point gives the least distance between team-mates.
fn by_arcs(points: &[Vec2], sizes: &[usize]) -> Vec<u8> {
    let n = points.len();
    let centre = points.iter().copied().sum::<Vec2>() / n as f32;
    let angle = |i: usize| {
        let d = points[i] - centre;
        d.y.atan2(d.x)
    };
    let mut around: Vec<usize> = (0..n).collect();
    around.sort_by(|&a, &b| angle(a).total_cmp(&angle(b)));
    let mut best = (f32::INFINITY, vec![0u8; n]);
    for shift in 0..n {
        let mut team = vec![0u8; n];
        let mut k = shift;
        for (g, &size) in sizes.iter().enumerate() {
            for _ in 0..size {
                team[around[k % n]] = g as u8;
                k += 1;
            }
        }
        let cost: f32 = (0..n)
            .flat_map(|a| (a + 1..n).map(move |b| (a, b)))
            .filter(|&(a, b)| team[a] == team[b])
            .map(|(a, b)| points[a].distance(points[b]))
            .sum();
        if cost < best.0 {
            best = (cost, team);
        }
    }
    // Teams numbered in order of first point, as the exact search numbers them.
    let mut names = vec![u8::MAX; sizes.len()];
    let mut next = 0;
    best.1
        .iter()
        .map(|&t| {
            if names[t as usize] == u8::MAX {
                names[t as usize] = next;
                next += 1;
            }
            names[t as usize]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matchups_read_like_a_lobby() {
        assert_eq!(matchup(&[0, 1]), "1 v 1");
        assert_eq!(matchup(&[1, 0, 1, 0]), "2 v 2");
        assert_eq!(matchup(&[0, 0, 0, 1]), "3 v 1");
        assert!(uneven(&[0, 0, 0, 1]));
        assert!(!uneven(&[0, 1, 0, 1]));
        assert_eq!(matchup(&[0, 1, 2, 3]), "Free for All \u{b7} 4");
        assert_eq!(matchup(&[0, 0, 1, 1, 2, 2]), "2 v 2 v 2");
        let eights: Vec<u8> = (0..32).map(|i| i / 4).collect();
        assert_eq!(matchup(&eights), "8 Teams of 4");
    }

    #[test]
    fn sides_follow_the_ground_not_the_slot_order() {
        // Two zones west, two east, listed alternately.
        let p = [
            Vec2::new(0.0, 0.0),
            Vec2::new(100.0, 0.0),
            Vec2::new(0.0, 10.0),
            Vec2::new(100.0, 10.0),
        ];
        assert_eq!(by_ground(&p, 2), vec![0, 1, 0, 1]);
        // Eight zones in four corner pairs make four pairs.
        let corners = [(0.0, 0.0), (100.0, 0.0), (0.0, 100.0), (100.0, 100.0)];
        let p: Vec<Vec2> = corners
            .iter()
            .flat_map(|&(x, y)| [Vec2::new(x, y), Vec2::new(x + 5.0, y)])
            .collect();
        let t = by_ground(&p, 4);
        for k in 0..4 {
            assert_eq!(t[2 * k], t[2 * k + 1]);
        }
        assert_eq!(links(&p, &t).len(), 4);
    }

    #[test]
    fn a_ring_of_thirty_two_splits_into_arcs_at_once() {
        let p: Vec<Vec2> = (0..32)
            .map(|k| Vec2::from_angle(k as f32 * std::f32::consts::TAU / 32.0) * 1000.0)
            .collect();
        for groups in [2, 4, 8] {
            let t = by_ground(&p, groups);
            assert_eq!(t[0], 0, "numbered from the first point");
            for g in 0..groups as u8 {
                assert_eq!(t.iter().filter(|&&x| x == g).count(), 32 / groups);
            }
            // Each team is one run around the ring: it changes team `groups` times.
            let changes = (0..32).filter(|&k| t[k] != t[(k + 1) % 32]).count();
            assert_eq!(changes, groups);
        }
    }
}
