//! Note editing as plain functions over a pattern's note list, so the piano
//! roll's gestures (move, resize, quantize, duplicate, paste) are testable
//! without a window. Selections are indices into the list; anything that
//! reorders the list returns the new indices.

use mc_music::{Note, PPQ};

/// Snap values offered by the editors: (name, ticks). 0 = off.
pub const GRIDS: [(&str, u32); 11] = [
    ("Bar", 0xFFFF_FFFF),
    ("1/4", PPQ),
    ("1/8", PPQ / 2),
    ("1/16", PPQ / 4),
    ("1/32", PPQ / 8),
    ("1/4 triplet", PPQ * 2 / 3),
    ("1/8 triplet", PPQ / 3),
    ("1/16 triplet", PPQ / 6),
    ("1/32 triplet", PPQ / 12),
    ("1/64", PPQ / 16),
    ("Off", 1),
];

/// A grid step in ticks; "Bar" resolves against the song's bar.
pub fn grid_ticks(index: usize, bar: u32) -> u32 {
    let g = GRIDS.get(index).map(|g| g.1).unwrap_or(PPQ / 4);
    if g == 0xFFFF_FFFF {
        bar.max(1)
    } else {
        g.max(1)
    }
}

/// Nearest grid line.
pub fn snap_round(t: i64, grid: u32) -> i64 {
    let g = grid.max(1) as i64;
    (t + g / 2).div_euclid(g) * g
}

/// Grid line at or before.
pub fn snap_floor(t: i64, grid: u32) -> i64 {
    let g = grid.max(1) as i64;
    t.div_euclid(g) * g
}

/// Grid line at or after.
pub fn snap_ceil(t: i64, grid: u32) -> i64 {
    let g = grid.max(1) as i64;
    (t + g - 1).div_euclid(g) * g
}

/// Sorts notes by start then key and maps the selection to the new order.
pub fn normalise(notes: &mut Vec<Note>, sel: &[usize]) -> Vec<usize> {
    let mut tagged: Vec<(Note, bool)> = notes
        .iter()
        .enumerate()
        .map(|(i, n)| (*n, sel.contains(&i)))
        .collect();
    tagged.sort_by_key(|(n, _)| (n.0, n.2, n.1));
    notes.clear();
    let mut out = Vec::new();
    for (i, (n, s)) in tagged.into_iter().enumerate() {
        notes.push(n);
        if s {
            out.push(i);
        }
    }
    out
}

/// Moves the selected notes by `dt` ticks and `dk` semitones, clamped so the
/// group stays inside 0..`max_tick` and the MIDI range.
pub fn move_notes(notes: &mut [Note], sel: &[usize], dt: i64, dk: i32, max_tick: u32) {
    if sel.is_empty() {
        return;
    }
    let min_at = sel.iter().map(|&i| notes[i].0 as i64).min().unwrap_or(0);
    let max_at = sel.iter().map(|&i| notes[i].0 as i64).max().unwrap_or(0);
    let min_k = sel.iter().map(|&i| notes[i].2 as i32).min().unwrap_or(0);
    let max_k = sel.iter().map(|&i| notes[i].2 as i32).max().unwrap_or(0);
    let dt = dt
        .max(-min_at)
        .min((max_tick as i64 - 1 - max_at).max(-min_at));
    let dk = dk.clamp(-min_k, 127 - max_k);
    for &i in sel {
        let n = &mut notes[i];
        n.0 = (n.0 as i64 + dt) as u32;
        n.2 = (n.2 as i32 + dk) as u8;
    }
}

/// Lengthens (or shortens) the selected notes by `dlen`, never below `min_len`.
pub fn resize_notes(notes: &mut [Note], sel: &[usize], dlen: i64, min_len: u32) {
    for &i in sel {
        let n = &mut notes[i];
        n.1 = (n.1 as i64 + dlen).max(min_len.max(1) as i64) as u32;
    }
}

/// Moves starts to the nearest grid line; the whole list when `sel` is empty.
/// A note that would land past the pattern's end stays on the last line.
pub fn quantize(notes: &mut [Note], sel: &[usize], grid: u32, max_tick: u32) {
    let all: Vec<usize> = (0..notes.len()).collect();
    let which = if sel.is_empty() { &all[..] } else { sel };
    for &i in which {
        let n = &mut notes[i];
        let mut t = snap_round(n.0 as i64, grid);
        if t >= max_tick as i64 {
            t = snap_floor(max_tick as i64 - 1, grid);
        }
        n.0 = t.max(0) as u32;
    }
}

/// Copies the selection to just after itself (rounded up to the grid) and
/// returns the copies' indices. Copies that would start past `max_tick` are dropped.
pub fn duplicate(notes: &mut Vec<Note>, sel: &[usize], grid: u32, max_tick: u32) -> Vec<usize> {
    if sel.is_empty() {
        return Vec::new();
    }
    let start = sel.iter().map(|&i| notes[i].0).min().unwrap_or(0);
    let end = sel.iter().map(|&i| notes[i].end()).max().unwrap_or(0);
    let span = snap_ceil((end - start) as i64, grid).max(grid as i64) as u32;
    let copies: Vec<Note> = sel.iter().map(|&i| notes[i]).collect();
    let mut out = Vec::new();
    for mut n in copies {
        n.0 += span;
        if n.0 < max_tick {
            out.push(notes.len());
            notes.push(n);
        }
    }
    out
}

/// The selection as a clipboard: times relative to its first note.
pub fn copy(notes: &[Note], sel: &[usize]) -> Vec<Note> {
    let start = sel.iter().map(|&i| notes[i].0).min().unwrap_or(0);
    sel.iter()
        .map(|&i| Note(notes[i].0 - start, notes[i].1, notes[i].2, notes[i].3))
        .collect()
}

/// Puts a clipboard at `at` and returns the new notes' indices.
pub fn paste(notes: &mut Vec<Note>, clip: &[Note], at: u32, max_tick: u32) -> Vec<usize> {
    let mut out = Vec::new();
    for n in clip {
        let t = n.0 + at;
        if t < max_tick {
            out.push(notes.len());
            notes.push(Note(t, n.1, n.2, n.3));
        }
    }
    out
}

pub fn delete(notes: &mut Vec<Note>, sel: &[usize]) {
    let mut i = 0;
    notes.retain(|_| {
        let k = !sel.contains(&i);
        i += 1;
        k
    });
}

/// Notes that overlap the time range and key range (inclusive keys).
pub fn in_rect(notes: &[Note], t0: u32, t1: u32, k0: u8, k1: u8) -> Vec<usize> {
    notes
        .iter()
        .enumerate()
        .filter(|(_, n)| n.2 >= k0 && n.2 <= k1 && n.end() > t0 && n.0 < t1)
        .map(|(i, _)| i)
        .collect()
}

/// "bar.beat.tick" from ticks, 1-based like every DAW.
pub fn position_text(tick: u32, beats_per_bar: u32) -> String {
    let beat = tick / PPQ;
    let bpb = beats_per_bar.max(1);
    format!("{}.{}.{:02}", beat / bpb + 1, beat % bpb + 1, tick % PPQ)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapping() {
        assert_eq!(snap_round(11, 24), 0);
        assert_eq!(snap_round(12, 24), 24);
        assert_eq!(snap_round(-13, 24), -24);
        assert_eq!(snap_floor(47, 24), 24);
        assert_eq!(snap_floor(-1, 24), -24);
        assert_eq!(snap_ceil(25, 24), 48);
        assert_eq!(snap_ceil(24, 24), 24);
        assert_eq!(grid_ticks(0, 384), 384);
        assert_eq!(grid_ticks(3, 384), 24);
        assert_eq!(grid_ticks(6, 384), 32);
    }

    #[test]
    fn move_clamps_the_group() {
        let mut n = vec![
            Note(0, 24, 60, 100),
            Note(96, 24, 64, 100),
            Note(200, 24, 10, 100),
        ];
        move_notes(&mut n, &[0, 1], -48, 2, 384);
        // The group cannot move before 0: the first note pins it.
        assert_eq!(n[0], Note(0, 24, 62, 100));
        assert_eq!(n[1], Note(96, 24, 66, 100));
        move_notes(&mut n, &[0, 1], 48, 100, 384);
        assert_eq!(n[1].2, 127);
        assert_eq!(n[0].0, 48);
        assert_eq!(n[2], Note(200, 24, 10, 100));
        move_notes(&mut n, &[1], 1000, 0, 384);
        assert_eq!(n[1].0, 383);
    }

    #[test]
    fn resize_keeps_a_minimum() {
        let mut n = vec![Note(0, 24, 60, 100)];
        resize_notes(&mut n, &[0], -100, 6);
        assert_eq!(n[0].1, 6);
        resize_notes(&mut n, &[0], 42, 6);
        assert_eq!(n[0].1, 48);
    }

    #[test]
    fn quantize_moves_starts_only() {
        let mut n = vec![
            Note(5, 30, 60, 100),
            Note(40, 10, 62, 100),
            Note(380, 10, 62, 100),
        ];
        quantize(&mut n, &[], 24, 384);
        assert_eq!(n[0], Note(0, 30, 60, 100));
        assert_eq!(n[1], Note(48, 10, 62, 100));
        // 380 rounds to 384, which is the end: it stays on the last line.
        assert_eq!(n[2].0, 360);
        let mut m = vec![Note(5, 30, 60, 100), Note(40, 10, 62, 100)];
        quantize(&mut m, &[1], 24, 384);
        assert_eq!(m[0].0, 5);
        assert_eq!(m[1].0, 48);
    }

    #[test]
    fn duplicate_places_after_the_selection() {
        let mut n = vec![Note(0, 24, 60, 100), Note(24, 20, 62, 90)];
        let sel = duplicate(&mut n, &[0, 1], 24, 384);
        assert_eq!(sel, vec![2, 3]);
        assert_eq!(n[2], Note(48, 24, 60, 100));
        assert_eq!(n[3], Note(72, 20, 62, 90));
        // Past the end, copies are dropped.
        let mut m = vec![Note(300, 80, 60, 100)];
        assert!(duplicate(&mut m, &[0], 24, 384).is_empty());
    }

    #[test]
    fn copy_paste_round_trip() {
        let n = vec![Note(96, 24, 60, 100), Note(120, 24, 64, 80)];
        let clip = copy(&n, &[0, 1]);
        assert_eq!(clip[0].0, 0);
        assert_eq!(clip[1].0, 24);
        let mut m = Vec::new();
        let sel = paste(&mut m, &clip, 192, 384);
        assert_eq!(sel, vec![0, 1]);
        assert_eq!(m[1], Note(216, 24, 64, 80));
    }

    #[test]
    fn delete_and_normalise() {
        let mut n = vec![
            Note(96, 24, 60, 100),
            Note(0, 24, 64, 80),
            Note(48, 24, 62, 80),
        ];
        let sel = normalise(&mut n, &[0]);
        assert_eq!(n[0].0, 0);
        assert_eq!(sel, vec![2]);
        delete(&mut n, &[0, 2]);
        assert_eq!(n, vec![Note(48, 24, 62, 80)]);
    }

    #[test]
    fn hits_and_rects() {
        let n = vec![
            Note(0, 48, 60, 100),
            Note(24, 48, 60, 100),
            Note(0, 96, 67, 100),
        ];
        assert_eq!(in_rect(&n, 50, 60, 60, 66), vec![1]);
        assert_eq!(in_rect(&n, 0, 10, 0, 127), vec![0, 2]);
    }

    #[test]
    fn positions() {
        assert_eq!(position_text(0, 4), "1.1.00");
        assert_eq!(position_text(96 * 5 + 12, 4), "2.2.12");
    }
}
