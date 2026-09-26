//! Synthetic humming with the faults of a real voice — vibrato, pitch drift,
//! scoops into notes, breath noise, uneven timing and level, a singer who is
//! sharp overall — transcribed back.

use mc_listen::pitch::{transcribe, transcribe_with, TranscribeOpts};
use mc_music::{Note, PPQ};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
    fn bi(&mut self) -> f32 {
        self.next() * 2.0 - 1.0
    }
}

/// (beat, length in beats, MIDI key); `None` key = rest.
type Line = Vec<(f32, f32, u8)>;

fn melody() -> Line {
    // D minor, 4 bars at 4/4, with a repeated note, an octave leap and steps.
    let mut v = Vec::new();
    let mut b = 0.0;
    for &(len, key) in &[
        (1.0, 62), (0.5, 65), (0.5, 67), (1.0, 69), (0.5, 69), (0.5, 72),
        (1.0, 70), (1.0, 69), (0.5, 67), (0.5, 65), (1.0, 64),
        (1.0, 62), (1.0, 74), (0.5, 72), (0.5, 70), (1.0, 69),
        (0.5, 65), (0.5, 64), (2.0, 62),
    ] {
        v.push((b, len, key));
        b += len;
    }
    v
}

struct Voice {
    vibrato_cents: f32,
    drift_cents: f32,
    jitter_s: f32,
    breath: f32,
    sharp_cents: f32,
    transpose: i32,
}

/// Renders a hummed line at `tempo`; returns the samples and the quantised truth.
fn hum(line: &Line, tempo: f32, rate: u32, v: &Voice, seed: u64) -> (Vec<f32>, Vec<Note>) {
    let mut rng = Rng(seed);
    let beat = 60.0 / tempo;
    let lead_in = 0.5;
    let total = lead_in + line.iter().map(|n| n.0 + n.1).fold(0.0, f32::max) * beat + 0.5;
    let n = (total * rate as f32) as usize;
    let mut out = vec![0.0f32; n];
    let mut truth = Vec::new();
    let mut phase = 0.0f32;
    let mut drift = 0.0f32;
    let mut prev_key: Option<f32> = None;
    for (i, &(b, len, key)) in line.iter().enumerate() {
        let key = key as i32 + v.transpose;
        truth.push(Note((b * PPQ as f32).round() as u32, (len * PPQ as f32) as u32, key as u8, 100));
        let start = lead_in + b * beat + v.jitter_s * rng.bi();
        // Hummers breathe between notes: a gap before repeated pitches and at phrase ends.
        let next_same = line.get(i + 1).map(|nx| nx.2 as i32 + v.transpose == key).unwrap_or(true);
        let gap = if next_same || len >= 2.0 { 0.09 } else { 0.015 };
        let end = lead_in + (b + len) * beat - gap + v.jitter_s * 0.5 * rng.bi();
        let level = 0.3 * 10f32.powf(3.0 * rng.bi() / 20.0);
        let s0 = (start * rate as f32) as usize;
        let s1 = ((end * rate as f32) as usize).min(n);
        let from = prev_key.unwrap_or(key as f32 - 0.6);
        for s in s0..s1 {
            let t = (s - s0) as f32 / rate as f32;
            let left = (s1 - s) as f32 / rate as f32;
            // Scoop or glide into the note over 50 ms.
            let glide = (t / 0.05).min(1.0);
            let mut m = from + (key as f32 - from) * (1.0 - (1.0 - glide).powi(2));
            // Slow drift, bounded.
            drift = (drift + rng.bi() * 0.02).clamp(-v.drift_cents, v.drift_cents);
            m += drift / 100.0 + v.sharp_cents / 100.0;
            // Vibrato fades in after 150 ms.
            let vib = ((t - 0.15) / 0.2).clamp(0.0, 1.0);
            m += vib * v.vibrato_cents / 100.0 * (2.0 * std::f32::consts::PI * 5.5 * t).sin();
            let f = 440.0 * 2f32.powf((m - 69.0) / 12.0);
            phase = (phase + f / rate as f32).fract();
            let env = (t / 0.03).min(1.0) * (left / 0.05).min(1.0);
            let mut y = 0.0;
            for (h, a) in [1.0f32, 0.55, 0.35, 0.22, 0.12, 0.08, 0.05].iter().enumerate() {
                y += a * (2.0 * std::f32::consts::PI * phase * (h + 1) as f32).sin();
            }
            out[s] += y * env * level;
        }
        prev_key = Some(key as f32);
    }
    // Breath: low-passed noise under everything.
    let mut lp = 0.0f32;
    for s in out.iter_mut() {
        lp += 0.2 * (rng.bi() - lp);
        *s += lp * v.breath;
    }
    (out, truth)
}

fn score(truth: &[Note], got: &[Note], origin_ticks: u32) -> (f32, usize) {
    let got: Vec<Note> = got.iter().map(|n| Note(n.0.saturating_sub(origin_ticks), n.1, n.2, n.3)).collect();
    let hit = truth
        .iter()
        .filter(|t| got.iter().any(|g| g.2 == t.2 && (g.0 as i64 - t.0 as i64).abs() <= (PPQ / 4) as i64))
        .count();
    let octave = got
        .iter()
        .filter(|g| truth.iter().any(|t| (t.0 as i64 - g.0 as i64).abs() <= (PPQ / 4) as i64 && (t.2 as i32 - g.2 as i32).abs() == 12))
        .count();
    (hit as f32 / truth.len() as f32, octave)
}

fn run(name: &str, v: Voice, tempo: f32, seed: u64) -> (f32, usize) {
    let rate = 44100;
    let (x, truth) = hum(&melody(), tempo, rate, &v, seed);
    let mut o = TranscribeOpts::voice(tempo);
    o.origin = 0.5; // the lead-in
    let t = transcribe_with(&x, rate, &o);
    let (acc, oct) = score(&truth, &t.notes, 0);

    let names: Vec<String> = t.notes.iter().map(|n| mc_listen::theory::note_name(n.2, true)).collect();
    println!(
        "{name:<22} {:.0}% notes right, {oct} octave errors, {} notes heard ({} true), tuning {:+.0} cents: {}",
        acc * 100.0,
        t.notes.len(),
        truth.len(),
        t.tuning_cents,
        names.join(" ")
    );
    (acc, oct)
}

#[test]
fn hummed_lines_come_back() {
    let plain = Voice { vibrato_cents: 0.0, drift_cents: 0.0, jitter_s: 0.0, breath: 0.0, sharp_cents: 0.0, transpose: 0 };
    let real = Voice { vibrato_cents: 40.0, drift_cents: 30.0, jitter_s: 0.03, breath: 0.08, sharp_cents: 20.0, transpose: 0 };
    let runs = vec![
        ("clean", run("clean", plain, 100.0, 1), 0),
        ("vibrato+drift+breath", run("vibrato+drift+breath", Voice { ..real }, 100.0, 2), 0),
        ("low male voice", run("low male voice", Voice { transpose: -12, ..real }, 90.0, 3), 0),
        // Breath at -14 dB under a high voice: not clear material, one slip allowed.
        ("high, very breathy", run("high, very breathy", Voice { transpose: 7, breath: 0.2, ..real }, 120.0, 4), 1),
        ("wide vibrato", run("wide vibrato", Voice { vibrato_cents: 80.0, ..real }, 100.0, 5), 0),
        ("flat singer", run("flat singer", Voice { sharp_cents: -35.0, ..real }, 100.0, 6), 0),
    ];
    let mean = runs.iter().map(|r| r.1 .0).sum::<f32>() / runs.len() as f32;
    println!("mean {:.1}%", mean * 100.0);
    for (name, (acc, oct), allowed) in &runs {
        assert!(*acc >= 0.9, "{name}: {:.0}% under 90%", acc * 100.0);
        assert!(*oct <= *allowed, "{name}: {oct} octave errors");
    }
}

#[test]
fn steady_tone_is_one_note() {
    let rate = 48000;
    let x: Vec<f32> = (0..rate).map(|i| 0.3 * (2.0 * std::f32::consts::PI * 440.0 * i as f32 / rate as f32).sin()).collect();
    let notes = transcribe(&x, rate, 120.0);
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert_eq!(notes[0].2, 69);
    // A second at 120 bpm is two beats.
    assert!((notes[0].1 as i32 - 2 * PPQ as i32).abs() <= (PPQ / 4) as i32, "{notes:?}");
}

#[test]
fn key_snapping() {
    let rate = 44100;
    // 45 cents above F#4: rounds to F#/G boundary side G? no: to F#; in C major it must snap to G or F.
    let m = 66.45f32;
    let f = 440.0 * 2f32.powf((m - 69.0) / 12.0);
    let x: Vec<f32> = (0..rate / 2).map(|i| 0.3 * (2.0 * std::f32::consts::PI * f * i as f32 / rate as f32).sin()).collect();
    let mut o = TranscribeOpts::voice(120.0);
    o.adapt_tuning = false;
    let free = transcribe_with(&x, rate, &o);
    assert_eq!(free.notes[0].2, 66);
    o.key = Some((0, mc_music::song::Scale::Major));
    let snapped = transcribe_with(&x, rate, &o);
    assert_eq!(snapped.notes[0].2, 67, "F# is not in C major; 66.45 is nearer G");
}
