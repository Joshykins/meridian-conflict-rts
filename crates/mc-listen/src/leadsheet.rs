//! The lead sheet: a chart a musician would write, per section — chords as
//! names and as Roman numerals, harmonic rhythm, the bass rhythm and its scale
//! degrees, the melody as degrees with durations, the drum grid.
//!
//! Built the same way from a recording (`from_report`) and from one of our
//! songs' notes (`from_song`, exact), so the two can be compared chart to
//! chart in numerals and degrees, independent of transposition.

use crate::analyse::Report;
use crate::harmony::{self, split_bar};
use crate::melody;
use crate::theory::{self, Chord, Key};
use mc_music::patch::{Drum, FilterMode, Instrument};
use mc_music::{Note, Song, PPQ};
use serde::{Deserialize, Serialize};
use std::fmt;

const STEP: u32 = PPQ / 4;

/// What was in one bar, whatever it came from. Note times are ticks from the bar start.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BarFacts {
    /// (beat, chord)
    pub chords: Vec<(f32, Option<Chord>)>,
    pub bass: Vec<Note>,
    pub melody: Vec<Note>,
    /// kick, snare, hat, 16 steps per 4/4 bar.
    pub drums: [String; 3],
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SheetMelody {
    /// "5:4 4:2 b3:2 | 1:8 r:8" — degree:sixteenths, ' = an octave up, , = down, r = rest.
    pub degrees: String,
    pub contour: String,
    pub low: String,
    pub high: String,
    /// Bar.beat of the highest note.
    pub peak_at: String,
    pub confident: bool,
    /// Where the line came from (a track name, or "audio").
    pub from: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SheetSection {
    pub label: String,
    /// The song's section name, or the label for audio.
    pub name: String,
    /// 1-based.
    pub start_bar: usize,
    pub bars: usize,
    /// Chord names per bar.
    pub chords: Vec<Vec<String>>,
    /// Roman numerals per bar.
    pub numerals: Vec<Vec<String>>,
    /// Chord changes per bar.
    pub harmonic_rhythm: f32,
    /// The section's most common bass bar: 'x' onset, '-' held, '.' rest.
    pub bass_rhythm: String,
    /// That bar's degrees, one token per step ("1 . . 1 . 1 . . b7 . . . 5 . . .").
    pub bass_degrees: String,
    /// Onset degrees for each bar, "1 1 1 b7 5".
    pub bass_bars: Vec<String>,
    pub bass_notes_per_bar: f32,
    /// The most common drum bar (kick, snare, hat) and how many bars match it.
    pub drums: [String; 3],
    pub drums_count: usize,
    pub melody: Option<SheetMelody>,
    /// The section's own key (a piece that modulates); numerals and degrees are in it.
    pub key: Option<Key>,
    pub key_name: String,
    /// Bars in which the bass / the drums play at all.
    pub bass_bars_playing: usize,
    pub drum_bars_playing: usize,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LeadSheet {
    pub title: String,
    /// "audio" or "song".
    pub source: String,
    pub tempo: f32,
    pub beats_per_bar: u32,
    pub key: Option<Key>,
    pub key_name: String,
    pub sections: Vec<SheetSection>,
    /// The section labels in order: "A A B A".
    pub form: String,
}

fn mask(s: &str) -> String {
    s.chars().map(|c| if c == '.' { '.' } else { 'x' }).collect()
}

fn most_common<T: Clone + PartialEq>(items: &[T]) -> Option<(T, usize)> {
    let mut best: Option<(T, usize)> = None;
    for it in items {
        let n = items.iter().filter(|x| *x == it).count();
        if best.as_ref().map(|b| n > b.1).unwrap_or(true) {
            best = Some((it.clone(), n));
        }
    }
    best
}

fn degree_oct(key: u8, tonic_base: i32, k: &Key) -> String {
    let d = k.degree(key);
    let oct = (key as i32 - tonic_base).div_euclid(12);
    let marks = if oct > 0 { "'".repeat(oct as usize) } else { ",".repeat((-oct) as usize) };
    format!("{d}{marks}")
}

/// Builds a lead sheet from per-bar facts and sections (label, name, first bar 0-based, bars).
pub fn build(
    title: &str,
    source: &str,
    tempo: f32,
    beats_per_bar: u32,
    key: Key,
    sections: &[(String, String, usize, usize)],
    bars: &[BarFacts],
    melody_confident: bool,
    melody_from: &str,
) -> LeadSheet {
    let steps = (beats_per_bar * 4) as usize;
    let song_key = key;
    let mut out = LeadSheet {
        title: title.to_string(),
        source: source.to_string(),
        tempo,
        beats_per_bar,
        key: Some(key),
        key_name: key.name(),
        ..Default::default()
    };
    for (label, name, a, n) in sections {
        let a = (*a).min(bars.len());
        let b = (a + n).min(bars.len());
        let sb = &bars[a..b];
        if sb.is_empty() {
            continue;
        }
        let key = section_key(sb, song_key);
        let flats = key.flats();
        // The melody's degrees are marked against the octave of its lowest tonic.
        let tonic_base = sb
            .iter()
            .flat_map(|b| b.melody.iter().map(|n| n.2))
            .min()
            .map(|lo| lo as i32 - (lo as i32 - key.root as i32).rem_euclid(12))
            .unwrap_or(60);
        let mut sec = SheetSection {
            label: label.clone(),
            name: name.clone(),
            start_bar: a + 1,
            bars: sb.len(),
            key: Some(key),
            key_name: key.name(),
            ..Default::default()
        };
        let mut changes = 0usize;
        let mut prev: Option<Option<Chord>> = None;
        for bar in sb {
            let mut names = Vec::new();
            let mut nums = Vec::new();
            for (_, c) in &bar.chords {
                names.push(c.map(|c| c.name(flats)).unwrap_or_else(|| "N".into()));
                nums.push(c.map(|c| c.numeral(&key)).unwrap_or_else(|| "N".into()));
                if prev != Some(*c) {
                    changes += 1;
                }
                prev = Some(*c);
            }
            sec.chords.push(names);
            sec.numerals.push(nums);
        }
        sec.harmonic_rhythm = changes as f32 / sb.len() as f32;
        // Bass: rhythm grid and degrees per bar.
        let mut grids = Vec::new();
        let mut degs = Vec::new();
        let mut count = 0usize;
        for bar in sb {
            let mut g = vec!['.'; steps];
            let mut d = vec![".".to_string(); steps];
            for n in &bar.bass {
                let s = ((n.0 + STEP / 2) / STEP) as usize;
                if s >= steps {
                    continue;
                }
                let len = (n.1 as f32 / STEP as f32).round().max(1.0) as usize;
                g[s] = 'x';
                for c in g.iter_mut().take((s + len).min(steps)).skip(s + 1) {
                    if *c == '.' {
                        *c = '-';
                    }
                }
                d[s] = key.degree(n.2).to_string();
                count += 1;
            }
            let gs: String = g.iter().collect();
            sec.bass_bars.push(d.iter().filter(|t| *t != ".").cloned().collect::<Vec<_>>().join(" "));
            grids.push(gs);
            degs.push(d.join(" "));
        }
        sec.bass_notes_per_bar = count as f32 / sb.len() as f32;
        let onset_masks: Vec<String> = grids.iter().map(|g| g.replace('-', ".")).collect();
        // The most common bar among those where the bass plays at all.
        let playing: Vec<String> = onset_masks.iter().filter(|m| m.contains('x')).cloned().collect();
        sec.bass_bars_playing = playing.len();
        if let Some((m, _)) = most_common(&playing) {
            let i = onset_masks.iter().position(|x| *x == m).unwrap();
            sec.bass_rhythm = grids[i].clone();
            let pairs: Vec<(String, String)> = grids.iter().cloned().zip(degs.iter().cloned()).filter(|(g, _)| g.replace('-', ".") == m).collect();
            let ds: Vec<String> = pairs.iter().map(|p| p.1.clone()).collect();
            sec.bass_degrees = most_common(&ds).map(|x| x.0).unwrap_or_default();
        }
        // Drums.
        let masks: Vec<String> = sb.iter().map(|b| format!("{}|{}|{}", mask(&b.drums[0]), mask(&b.drums[1]), mask(&b.drums[2]))).collect();
        let playing: Vec<String> = masks.iter().filter(|m| m.contains('x')).cloned().collect();
        sec.drum_bars_playing = playing.len();
        if let Some((m, c)) = most_common(&playing) {
            let i = masks.iter().position(|x| *x == m).unwrap();
            sec.drums = sb[i].drums.clone();
            sec.drums_count = c;
        }
        // Melody.
        let mel: Vec<(usize, Note)> = sb.iter().enumerate().flat_map(|(i, b)| b.melody.iter().map(move |n| (i, *n))).collect();
        if mel.len() >= 2 {
            let bar_ticks = beats_per_bar * PPQ;
            let mut toks = Vec::new();
            let mut cursor = 0u32; // ticks from the section start
            let mut last_bar = 0usize;
            for (i, n) in &mel {
                let at = *i as u32 * bar_ticks + n.0;
                if *i != last_bar && !toks.is_empty() {
                    toks.push("|".to_string());
                }
                last_bar = *i;
                if at > cursor + STEP / 2 {
                    toks.push(format!("r:{}", ((at - cursor) as f32 / STEP as f32).round() as u32));
                }
                let len = (n.1 as f32 / STEP as f32).round().max(1.0) as u32;
                toks.push(format!("{}:{}", degree_oct(n.2, tonic_base, &key), len));
                cursor = at + n.1;
            }
            let keys: Vec<u8> = mel.iter().map(|m| m.1 .2).collect();
            let lo = *keys.iter().min().unwrap();
            let hi = *keys.iter().max().unwrap();
            let pk = mel.iter().find(|m| m.1 .2 == hi).unwrap();
            let mut degrees = toks.join(" ");
            if degrees.len() > 400 {
                let cut = degrees[..400].rfind(' ').unwrap_or(400);
                degrees = format!("{} ...", &degrees[..cut]);
            }
            sec.melody = Some(SheetMelody {
                degrees,
                contour: melody::contour(&keys),
                low: format!("{} ({})", degree_oct(lo, tonic_base, &key), theory::note_name(lo, flats)),
                high: format!("{} ({})", degree_oct(hi, tonic_base, &key), theory::note_name(hi, flats)),
                peak_at: format!("{}.{}", sec.start_bar + pk.0, pk.1 .0 / PPQ + 1),
                confident: melody_confident,
                from: melody_from.to_string(),
            });
        }
        out.sections.push(sec);
    }
    out.form = out.sections.iter().map(|s| s.label.clone()).collect::<Vec<_>>().join(" ");
    out
}

/// A section's own key from its chords (and the bass under them), when it
/// has enough bars to tell; otherwise the song's.
fn section_key(sb: &[BarFacts], song_key: Key) -> Key {
    let chords: Vec<Chord> = sb.iter().flat_map(|b| b.chords.iter().filter_map(|c| c.1)).collect();
    if sb.len() < 4 || chords.len() < 3 {
        return song_key;
    }
    let mut hist = [0.0f32; 12];
    for c in &chords {
        for (j, iv) in c.quality.intervals().iter().enumerate() {
            hist[((c.root + iv) % 12) as usize] += if j == 0 { 1.3 } else { 1.0 };
        }
    }
    for b in sb {
        for n in &b.bass {
            hist[(n.2 % 12) as usize] += 0.3 * (n.1 as f32 / PPQ as f32).min(4.0);
        }
    }
    let starts: Vec<Chord> = sb.iter().step_by(4).filter_map(|b| b.chords.first().and_then(|c| c.1)).collect();
    let ((k, s), _) = harmony::choose_key(&hist, &chords, &starts);
    // Stay in the song's key unless the section clearly sits elsewhere.
    let song_score = harmony::key_fit(&hist, &chords, &starts, song_key);
    if k != song_key && s > song_score + 0.05 {
        k
    } else {
        song_key
    }
}

/// The lead sheet of an analysed recording.
pub fn from_report(r: &Report, key: Key) -> LeadSheet {
    let bpb = r.grid.beats_per_bar;
    let bar_ticks = bpb * PPQ;
    let mut bars: Vec<BarFacts> = (0..r.grid.bars).map(|_| BarFacts::default()).collect();
    for (i, b) in r.chords.iter().enumerate().take(bars.len()) {
        bars[i].chords = b.chords.iter().map(|c| (c.beat, c.chord)).collect();
    }
    for n in &r.bass.notes {
        let b = (n.0 / bar_ticks) as usize;
        if b < bars.len() {
            bars[b].bass.push(Note(n.0 % bar_ticks, n.1, n.2, n.3));
        }
    }
    for n in &r.melody.notes {
        let b = (n.0 / bar_ticks) as usize;
        if b < bars.len() {
            bars[b].melody.push(Note(n.0 % bar_ticks, n.1, n.2, n.3));
        }
    }
    for (i, d) in r.drums.bars.iter().enumerate().take(bars.len()) {
        bars[i].drums = d.lanes.clone();
    }
    let secs: Vec<(String, String, usize, usize)> =
        r.sections.iter().map(|s| (s.label.clone(), s.label.clone(), s.start_bar - 1, s.bars)).collect();
    build(&r.source, "audio", r.tempo.bpm, bpb, key, &secs, &bars, r.melody.confidence >= 0.5, "audio")
}

/// General MIDI drum keys to lanes (35-59): kicks, snares/claps/rims, hats and cymbals.
pub fn gm_lane(key: u8) -> Option<usize> {
    match key {
        35 | 36 => Some(0),
        37..=40 => Some(1),
        42 | 44 | 46 | 49 | 51 | 52 | 53 | 55 | 57 | 59 => Some(2),
        _ => None,
    }
}

/// Which drum lane a kit pad plays: 0 kick, 1 snare, 2 hat, None = other percussion.
pub fn drum_lane(d: &Drum) -> Option<usize> {
    let n = d.name.to_lowercase();
    let has = |w: &[&str]| w.iter().any(|x| n.contains(x));
    if has(&["kick", "bd", "boom", "bass drum"]) {
        return Some(0);
    }
    if has(&["snare", "clap", "rim", "sd"]) {
        return Some(1);
    }
    if has(&["hat", "hh", "shaker", "ride", "cym", "spark", "tick"]) {
        return Some(2);
    }
    match d.key {
        35 | 36 => return Some(0),
        37..=40 => return Some(1),
        42 | 44 | 46 | 51 => return Some(2),
        _ => {}
    }
    // By sound: a low sweep is a kick; bright noise or ring is a hat; mid noise a snare.
    if let Some(b) = &d.body {
        if b.to < 90.0 && d.hiss.as_ref().map(|h| h.mode == FilterMode::LowPass).unwrap_or(true) {
            return Some(0);
        }
    }
    let bright = d.hiss.as_ref().map(|h| h.cutoff >= 6000.0).unwrap_or(false) || d.ring.as_ref().map(|r| r.highpass >= 6000.0 && d.body.is_none()).unwrap_or(false);
    if bright {
        return Some(2);
    }
    if d.hiss.as_ref().map(|h| h.cutoff >= 1000.0).unwrap_or(false) {
        return Some(1);
    }
    None
}

/// A flattened note: (track index, absolute tick, len, key, vel).
type FlatNote = (usize, u32, u32, u8, u8);

/// Every note the song plays, flattened, and each section's start tick.
/// Sections in `order` are laid end to end.
fn flatten(song: &Song, order: &[usize], intensity: f32) -> (Vec<FlatNote>, Vec<u32>) {
    let mut out = Vec::new();
    let mut starts = Vec::new();
    let mut at = 0u32;
    for &si in order {
        let sec = &song.sections[si];
        starts.push(at);
        let st = song.section_ticks(sec);
        for c in &sec.clips {
            let (Some(ti), Some(pi)) = (song.track(&c.track), song.pattern(&c.pattern)) else { continue };
            let t = &song.tracks[ti];
            if t.mute || t.layer.gain(intensity) < 0.05 {
                continue;
            }
            let p = &song.patterns[pi];
            let pt = p.ticks().max(1);
            let span = c.span(pt, st);
            let base = c.at * PPQ;
            let mut r = 0;
            while r * pt < span {
                for n in &p.notes {
                    if n.0 >= pt {
                        continue;
                    }
                    let rel = r * pt + n.0;
                    if rel >= span {
                        continue;
                    }
                    let key = (n.2 as i32 + c.transpose as i32).clamp(0, 127) as u8;
                    out.push((ti, at + base + rel, n.1.min(span - rel), key, n.3));
                }
                r += 1;
            }
        }
        at += st;
    }
    (out, starts)
}

/// The lead sheet of one of our songs, straight from its notes: the whole
/// arrangement, or one section. `intensity` decides which layered tracks play.
pub fn from_song(song: &Song, section: Option<&str>, intensity: f32) -> Result<LeadSheet, String> {
    let order: Vec<usize> = match section {
        Some(name) => vec![song.section(name).ok_or_else(|| format!("no section \"{name}\" in {}", song.name))?],
        None => {
            let v: Vec<usize> = song.arrangement.iter().filter_map(|n| song.section(n)).collect();
            if v.is_empty() {
                (0..song.sections.len()).collect()
            } else {
                v
            }
        }
    };
    let (notes, starts) = flatten(song, &order, intensity);
    let bpb = song.beats_per_bar.max(1);
    let bar_ticks = bpb * PPQ;
    let total_ticks: u32 = order.iter().map(|&i| song.section_ticks(&song.sections[i])).sum();
    let n_bars = (total_ticks / bar_ticks) as usize;
    let is_kit = |ti: usize| matches!(song.tracks[ti].instrument, Instrument::Kit(_));

    // Track roles.
    let mut by_track: std::collections::BTreeMap<usize, Vec<(u32, u32, u8, u8)>> = Default::default();
    for &(ti, t, l, k, v) in &notes {
        by_track.entry(ti).or_default().push((t, l, k, v));
    }
    let median_key = |v: &Vec<(u32, u32, u8, u8)>| crate::dsp::median(&v.iter().map(|n| n.2 as f32).collect::<Vec<_>>());
    let mono_share = |v: &Vec<(u32, u32, u8, u8)>| -> f32 {
        let over = v.iter().filter(|a| v.iter().any(|b| b.0 < a.0 + a.1 && a.0 < b.0 + b.1 && b != *a)).count();
        1.0 - over as f32 / v.len().max(1) as f32
    };
    let named = |ti: usize, words: &[&str]| {
        let n = song.tracks[ti].name.to_lowercase();
        words.iter().any(|w| n.contains(w))
    };
    let synths: Vec<usize> = by_track.keys().copied().filter(|&t| !is_kit(t)).collect();
    let bass_track = synths
        .iter()
        .copied()
        .filter(|&t| named(t, &["bass", "sub"]) && median_key(&by_track[&t]) < 60.0)
        .min_by(|a, b| median_key(&by_track[a]).partial_cmp(&median_key(&by_track[b])).unwrap())
        .or_else(|| {
            synths
                .iter()
                .copied()
                .filter(|&t| median_key(&by_track[&t]) < 53.0)
                .min_by(|a, b| median_key(&by_track[a]).partial_cmp(&median_key(&by_track[b])).unwrap())
        });
    let melody_track = synths
        .iter()
        .copied()
        .find(|&t| Some(t) != bass_track && named(t, &["lead", "melody", "vocal", "hook", "theme"]))
        .or_else(|| {
            synths
                .iter()
                .copied()
                .filter(|&t| Some(t) != bass_track && by_track[&t].len() >= 8 && mono_share(&by_track[&t]) > 0.8 && median_key(&by_track[&t]) >= 60.0)
                .max_by(|a, b| median_key(&by_track[a]).partial_cmp(&median_key(&by_track[b])).unwrap())
        });

    let mut bars: Vec<BarFacts> = (0..n_bars).map(|_| BarFacts { drums: [".".repeat(bpb as usize * 4), ".".repeat(bpb as usize * 4), ".".repeat(bpb as usize * 4)], ..Default::default() }).collect();
    // Drums.
    for &(ti, t, _, k, v) in &notes {
        if let Instrument::Kit(kit) = &song.tracks[ti].instrument {
            let Some(lane) = kit.drum_for(k).and_then(drum_lane).or_else(|| gm_lane(k)) else { continue };
            if t % STEP > STEP / 3 && STEP - t % STEP > STEP / 3 {
                continue; // off the sixteenth grid (a flam or a triplet)
            }
            let s = (t + STEP / 2) / STEP;
            let b = (s / (bpb * 4)) as usize;
            let step = (s % (bpb * 4)) as usize;
            if b < bars.len() {
                let sym = if v >= 115 {
                    'X'
                } else if v >= 88 {
                    'x'
                } else if v >= 60 {
                    'o'
                } else {
                    'g'
                };
                let mut chars: Vec<char> = bars[b].drums[lane].chars().collect();
                let rank = |c: char| "gox X".find(c).unwrap_or(0);
                if chars[step] == '.' || rank(sym) > rank(chars[step]) {
                    chars[step] = sym;
                }
                bars[b].drums[lane] = chars.into_iter().collect();
            }
        }
    }
    // Bass and melody lines.
    for (line, track) in [(0, bass_track), (1, melody_track)] {
        let Some(tr) = track else { continue };
        let mut v = by_track[&tr].clone();
        v.sort_by_key(|n| (n.0, std::cmp::Reverse(n.2)));
        // One note at a time: the lowest for bass, the highest for melody.
        v.dedup_by(|b, a| {
            if a.0 == b.0 {
                if (line == 0 && b.2 < a.2) || (line == 1 && b.2 > a.2) {
                    *a = *b;
                }
                true
            } else {
                false
            }
        });
        for &(t, l, k, vel) in &v {
            let b = (t / bar_ticks) as usize;
            if b < bars.len() {
                let n = Note(t % bar_ticks, l, k, vel);
                if line == 0 {
                    bars[b].bass.push(n);
                } else {
                    bars[b].melody.push(n);
                }
            }
        }
    }
    // Harmony: pitch classes sounding, weighted by overlap and velocity; the bass line as the bass.
    let tpl = theory::templates(false);
    let weights = |a: u32, b: u32| -> ([f32; 12], [f32; 12]) {
        let mut c = [0.0f32; 12];
        let mut bs = [0.0f32; 12];
        for &(ti, t, l, k, v) in &notes {
            if is_kit(ti) || Some(ti) == melody_track {
                continue;
            }
            let ov = (t + l).min(b).saturating_sub(t.max(a));
            if ov == 0 {
                continue;
            }
            let w = ov as f32 * v as f32;
            c[k as usize % 12] += w;
            if Some(ti) == bass_track {
                bs[k as usize % 12] += w;
            }
        }
        (c, bs)
    };
    let mut all_chords = Vec::new();
    let mut hist = [0.0f32; 12];
    for (bi, bar) in bars.iter_mut().enumerate() {
        let s = bi as u32 * bar_ticks;
        let best = |x: f32, y: f32| -> (Option<Chord>, f32) {
            let (c, bs) = weights(s + (x * PPQ as f32) as u32, s + (y * PPQ as f32) as u32);
            if c.iter().sum::<f32>() <= 0.0 {
                return (None, 0.0);
            }
            let m = theory::match_chords(&c, Some(&bs), &tpl);
            (Some(m[0].0), m[0].1)
        };
        let parts = split_bar(bpb as usize, &best);
        bar.chords = parts.iter().map(|p| (p.0, p.1)).collect();
        all_chords.extend(parts.iter().filter_map(|p| p.1));
        let (c, _) = weights(s, s + bar_ticks);
        for k in 0..12 {
            hist[k] += c[k];
        }
    }
    let phrase: Vec<Chord> = bars.iter().step_by(4).filter_map(|b| b.chords.first().and_then(|c| c.1)).collect();
    let ((key, _), _) = harmony::choose_key(&hist, &all_chords, &phrase);

    // Sections: labels by section name, in order of first appearance.
    let mut names: Vec<String> = Vec::new();
    let mut secs = Vec::new();
    for (j, &si) in order.iter().enumerate() {
        let sec = &song.sections[si];
        let idx = names.iter().position(|n| *n == sec.name).unwrap_or_else(|| {
            names.push(sec.name.clone());
            names.len() - 1
        });
        let label = ((b'A' + idx.min(25) as u8) as char).to_string();
        secs.push((label, sec.name.clone(), (starts[j] / bar_ticks) as usize, sec.bars as usize));
    }
    let mel_name = melody_track.map(|t| song.tracks[t].name.clone()).unwrap_or_default();
    let mut sheet = build(&song.name, "song", song.tempo, bpb, key, &secs, &bars, true, &mel_name);
    let declared = Key { root: song.root % 12, minor: !matches!(song.scale, mc_music::song::Scale::Major | mc_music::song::Scale::Lydian | mc_music::song::Scale::Mixolydian) };
    if declared != key {
        sheet.key_name = format!("{} (the song declares {})", key.name(), declared.name());
    }
    Ok(sheet)
}

/// "Cm | Ab | Eb | Bb" with a repeated cycle folded: "(Cm | Ab | Eb | Bb) x2".
pub fn progression(bars: &[Vec<String>]) -> String {
    let cell = |b: &Vec<String>| if b.len() == 1 { b[0].clone() } else { format!("[{}]", b.join(" ")) };
    let cells: Vec<String> = bars.iter().map(cell).collect();
    let n = cells.len();
    for p in [1usize, 2, 4, 8] {
        if p < n && n.is_multiple_of(p) && (0..n).all(|i| cells[i] == cells[i % p]) {
            return format!("({}) x{}", cells[..p].join(" | "), n / p);
        }
    }
    if n > 16 {
        return format!("{} | ... ({} bars)", cells[..16].join(" | "), n);
    }
    cells.join(" | ")
}

impl fmt::Display for LeadSheet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "LEAD SHEET  {} ({})", self.title, self.source)?;
        writeln!(f, "  {:.1} bpm, {}/4, key {}", self.tempo, self.beats_per_bar, self.key_name)?;
        writeln!(f, "  form: {}", self.form)?;
        let mut seen: Vec<&str> = Vec::new();
        for s in &self.sections {
            let repeat = seen.contains(&s.label.as_str());
            seen.push(&s.label);
            let title = if s.name != s.label { format!("{} \"{}\"", s.label, s.name) } else { s.label.clone() };
            let in_key = if s.key.is_some() && s.key != self.key { format!(", key {}", s.key_name) } else { String::new() };
            writeln!(f, "\n  [{}] bars {}-{} ({} bars){}{}", title, s.start_bar, s.start_bar + s.bars - 1, s.bars, in_key, if repeat { ", repeat" } else { "" })?;
            if repeat {
                // A repeat only shows its chords, unless they changed.
                writeln!(f, "    numerals  {}", progression(&s.numerals))?;
                continue;
            }
            writeln!(f, "    chords    {}", progression(&s.chords))?;
            writeln!(f, "    numerals  {}", progression(&s.numerals))?;
            writeln!(f, "    harmonic rhythm {:.2} chords/bar", s.harmonic_rhythm)?;
            if !s.bass_rhythm.is_empty() && s.bass_rhythm.contains('x') {
                writeln!(f, "    bass      {}   ({:.1} notes/bar; plays in {} of {} bars)", s.bass_rhythm, s.bass_notes_per_bar, s.bass_bars_playing, s.bars)?;
                writeln!(f, "    degrees   {}", s.bass_degrees)?;
                let shown = &s.bass_bars[..s.bass_bars.len().min(16)];
                let mut per = fold_bars(shown);
                if s.bass_bars.len() > 16 {
                    per.push_str(&format!(" | ... ({} more bars)", s.bass_bars.len() - 16));
                }
                writeln!(f, "    per bar   {}", per)?;
            } else {
                writeln!(f, "    bass      none heard")?;
            }
            if s.drum_bars_playing == 0 {
                writeln!(f, "    drums     none")?;
            } else {
                let loose = if s.drums_count * 4 < s.drum_bars_playing { ", no repeating pattern" } else { "" };
                writeln!(f, "    drums (this bar {} times; drums play in {} of {} bars{})", s.drums_count, s.drum_bars_playing, s.bars, loose)?;
                for (k, n) in crate::drums::LANES.iter().enumerate() {
                    writeln!(f, "      {:<5} {}", n, s.drums[k])?;
                }
            }
            if let Some(m) = &s.melody {
                writeln!(
                    f,
                    "    melody{} ({}): {}",
                    if m.confident { "" } else { " [low confidence]" },
                    m.from,
                    m.contour
                )?;
                writeln!(f, "      range {} .. {}, peak at {}", m.low, m.high, m.peak_at)?;
                writeln!(f, "      {}", m.degrees)?;
            }
        }
        Ok(())
    }
}

/// "1 1 1 b7 5 | x2 | 1 1 b6" style: repeated bars folded.
fn fold_bars(bars: &[String]) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < bars.len() {
        let mut j = i + 1;
        while j < bars.len() && bars[j] == bars[i] {
            j += 1;
        }
        let cell = if bars[i].is_empty() { "-".to_string() } else { bars[i].clone() };
        out.push(if j - i > 1 { format!("{cell} (x{})", j - i) } else { cell });
        i = j;
    }
    out.join(" | ")
}
