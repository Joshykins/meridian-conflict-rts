//! The whole analysis of a recording: `reference(audio, opts) -> Report`.

use crate::decode::Audio;
use crate::drums::{self, DrumReport};
use crate::dsp::{self, clock};
use crate::features;
use crate::harmony::{self, BarChords, Grid, KeyReport};
use crate::leadsheet::{self, LeadSheet};
use crate::melody::{self, Melody};
use crate::pitch::{self, NoteEvent, TranscribeOpts};
use crate::sound::{self, Sound};
use crate::structure::{self, SectionInfo};
use crate::tempo::{self, Tempo};
use crate::theory::{self, Key};
use mc_music::{Note, PPQ};
use serde::{Deserialize, Serialize};
use std::fmt;

/// What to analyse and what is already known.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Options {
    /// Only this span, seconds.
    pub from: Option<f32>,
    pub to: Option<f32>,
    pub beats_per_bar: u32,
    /// A known tempo: the search stays within 8% of it.
    pub tempo: Option<f32>,
    /// A known key: used for numerals and degrees instead of the detected one.
    pub key: Option<Key>,
    /// Guess a melody (slower, low confidence).
    pub melody: bool,
    /// A name for the report.
    pub source: String,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            from: None,
            to: None,
            beats_per_bar: 4,
            tempo: None,
            key: None,
            melody: true,
            source: String::new(),
        }
    }
}

/// One bar of the bass line: (step 0..16, length in sixteenths, MIDI key).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BassBar {
    pub bar: usize,
    pub notes: Vec<(usize, u32, u8)>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Bass {
    /// At the detected tempo, tick 0 = the first downbeat.
    pub notes: Vec<Note>,
    pub bars: Vec<BassBar>,
    pub events: Vec<NoteEvent>,
    /// Share of the audio with a bass pitch.
    pub voiced: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Report {
    pub source: String,
    pub rate: u32,
    /// Where the analysed span starts in the file, seconds.
    pub from: f32,
    pub seconds: f32,
    pub tempo: Tempo,
    pub grid: Grid,
    pub key: KeyReport,
    pub chords: Vec<BarChords>,
    pub bass: Bass,
    pub drums: DrumReport,
    pub melody: Melody,
    pub sections: Vec<SectionInfo>,
    pub sound: Sound,
    pub leadsheet: LeadSheet,
}

/// Analyses a recording (or the `from..to` part of it).
pub fn reference(audio: &Audio, opts: &Options) -> Report {
    let part = audio.span(opts.from, opts.to);
    let rate = part.rate;
    let x = part.mono();
    let dur = part.seconds();

    let fa = features::pass_a(&x, rate);
    let fb = features::pass_b(&x, rate, 0.02);
    let tuning = features::tuning(&fb);
    // Above the bass register: a bass saw's strong root and fifth partials would
    // otherwise drown the chord's third (the bass has its own chroma).
    let chroma = features::chroma(&fb, tuning, 150.0, 5000.0);
    let bass_chroma = features::chroma(&fb, tuning, 30.0, 250.0);

    let env = tempo::prepare(&fa.flux, fa.hop);
    let env_low = tempo::prepare(&fa.flux_low, fa.hop);
    let mut t = tempo::estimate(&env, Some(&env_low), fa.hop, opts.tempo);
    // Calibration: the flux peaks a little before the true onset with these windows.
    t.beat_phase += ONSET_LAG;
    let bpb = opts.beats_per_bar.max(1);
    let db = tempo::downbeat(
        &t,
        bpb,
        fa.hop,
        &fa.flux_low,
        &fa.flux_mid,
        &chroma,
        fb.hop,
        dur,
    );
    let mut grid = Grid {
        bpm: t.bpm,
        beats_per_bar: bpb,
        downbeat: db,
        bars: 0,
    };
    grid.bars = ((dur - db) / grid.bar_len()).floor().max(0.0) as usize;

    let drums = drums::analyse(&x, rate, &grid, &fa.flux_low, fa.hop);
    // The drum grid's alignment is finer than the flux's: move the grid onto it.
    grid.downbeat += drums.offset_ms / 1000.0;
    if grid.downbeat < -0.05 {
        grid.downbeat += grid.bar_len();
    }
    grid.bars = ((dur - grid.downbeat) / grid.bar_len()).floor().max(0.0) as usize;
    let drums = if drums.offset_ms.abs() > 0.5 {
        drums::analyse(&x, rate, &grid, &fa.flux_low, fa.hop)
    } else {
        drums
    };

    let mut chords = harmony::chords(&chroma, &bass_chroma, fb.hop, &grid);
    let key = harmony::key(&chroma, &bass_chroma, &chords, tuning);
    let use_key = opts.key.unwrap_or(key.key);
    harmony::respell(&mut chords, &use_key);

    // Bass: below 250 Hz, transcribed like a hummed line.
    // The kick's tail sits in the same band: subtract its average waveform at
    // every hit first (the bass, varying from hit to hit, averages out of it).
    // Above 55 Hz: what is left of a kick's tail sits at 40-50 Hz and would pull
    // the tracker an octave down; a bass keeps its period in its harmonics.
    let low = dsp::band(&x, rate, 55.0, 300.0);
    let low = remove_repeated_hits(&low, rate, &drums.kick_times, 1.5);
    let low = remove_repeated_hits(&low, rate, &drums.snare_times, 0.6);
    let mut bo = TranscribeOpts::bass(grid.bpm);
    bo.origin = grid.downbeat;
    let tr = pitch::transcribe_with(&low, rate, &bo);
    let bars_ticks = bpb * PPQ;
    let mut bass_bars: Vec<BassBar> = (0..grid.bars)
        .map(|b| BassBar {
            bar: b + 1,
            notes: Vec::new(),
        })
        .collect();
    for n in &tr.notes {
        let b = (n.0 / bars_ticks) as usize;
        if b < bass_bars.len() {
            bass_bars[b].notes.push((
                ((n.0 % bars_ticks) / (PPQ / 4)) as usize,
                (n.1 / (PPQ / 4)).max(1),
                n.2,
            ));
        }
    }
    let voiced =
        tr.track.f0.iter().filter(|&&f| f > 0.0).count() as f32 / tr.track.f0.len().max(1) as f32;
    let bass = Bass {
        notes: tr.notes,
        bars: bass_bars,
        events: tr.events,
        voiced,
    };

    let melody = if opts.melody {
        melody::guess(&fb, tuning, &grid)
    } else {
        Melody::default()
    };

    // Arrangement from per-bar features.
    let (kp, khop) = sound::k_power(&part);
    let mut feats = Vec::new();
    for b in 0..grid.bars {
        let (s, e) = (grid.bar_start(b), grid.bar_start(b + 1));
        let ia = ((s / fa.hop).max(0.0) as usize).min(fa.mfcc.len());
        let ib = ((e / fa.hop) as usize).clamp(ia, fa.mfcc.len());
        let mut f = Vec::with_capacity(28);
        let ja = ((s / fb.hop).max(0.0) as usize).min(chroma.len());
        let jb = ((e / fb.hop) as usize).clamp(ja, chroma.len());
        let mut c = [0.0f32; 12];
        for ch in &chroma[ja..jb] {
            for k in 0..12 {
                c[k] += ch[k];
            }
        }
        let cs: f32 = c.iter().sum::<f32>().max(1e-9);
        f.extend(c.iter().map(|v| v / cs));
        for q in 1..features::MFCC {
            f.push(dsp::mean(&fa.mfcc[ia..ib].iter().map(|m| m[q]).collect::<Vec<_>>()) * 0.5);
        }
        let loud = sound::loudness_between(&kp, khop, s, e);
        f.push(loud * 0.3);
        f.push(loud * 0.3);
        feats.push(f);
    }
    let bounds = structure::boundaries(&feats);
    let labels = structure::labels(&feats, &bounds);
    let mut sections = Vec::new();
    for (i, &a) in bounds.iter().enumerate() {
        let b = bounds.get(i + 1).copied().unwrap_or(grid.bars);
        let (s, e) = (grid.bar_start(a), grid.bar_start(b));
        let ia = ((s / fa.hop) as usize).min(fa.centroid.len());
        let ib = ((e / fa.hop) as usize).clamp(ia, fa.centroid.len());
        sections.push(SectionInfo {
            label: labels[i].clone(),
            start_bar: a + 1,
            bars: b - a,
            start: s,
            end: e,
            loudness: (sound::loudness_between(&kp, khop, s, e) * 10.0).round() / 10.0,
            brightness: dsp::median(&fa.centroid[ia..ib]).round(),
        });
    }

    let sound = sound::measure(
        &part,
        &fa,
        &fb,
        &drums.kick_times,
        &drums.snare_times,
        grid.beat(),
    );
    let mut report = Report {
        source: opts.source.clone(),
        rate,
        from: opts.from.unwrap_or(0.0),
        seconds: dur,
        tempo: t,
        grid,
        key,
        chords,
        bass,
        drums,
        melody,
        sections,
        sound,
        leadsheet: LeadSheet::default(),
    };
    report.leadsheet = leadsheet::from_report(&report, use_key);
    report
}

/// Subtracts a drum that repeats the same waveform (a sampled or synthesised
/// kick) from `x`. Each hit lasts until the next one (a drum pad retriggered
/// cuts its own tail). The drum is the mean of the segments starting at the
/// hits, aligned to a quarter sample by cross-correlation; anything that does
/// not repeat with the hits (the bass) averages out of it. Each hit gets its
/// own gain. Does nothing unless the hits really repeat and the drum decays.
pub fn remove_repeated_hits(x: &[f32], rate: u32, hits: &[f32], seconds: f32) -> Vec<f32> {
    if hits.len() < 4 || x.is_empty() {
        return x.to_vec();
    }
    let r = rate as f32;
    let len = ((seconds * r) as usize).min(x.len());
    let head = ((0.06 * r) as usize).min(len);
    let at = |v: &[f32], t: f32| -> f32 { dsp::lerp_at(v, t) };
    let mut times: Vec<f32> = hits.iter().map(|&t| t * r - 0.004 * r).collect();
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let lens = |times: &[f32]| -> Vec<usize> {
        (0..times.len())
            .map(|i| {
                times
                    .get(i + 1)
                    .map(|n| ((n - times[i]).max(0.0) as usize).min(len))
                    .unwrap_or(len)
            })
            .collect()
    };
    let average = |times: &[f32], ls: &[usize]| -> Vec<f32> {
        let mut t = vec![0.0f32; len];
        let mut c = vec![0u32; len];
        for (k, &s) in times.iter().enumerate() {
            for i in 0..ls[k] {
                t[i] += at(x, s + i as f32);
                c[i] += 1;
            }
        }
        t.iter()
            .zip(&c)
            .map(|(v, n)| if *n >= 3 { v / *n as f32 } else { 0.0 })
            .collect()
    };
    let mut tpl = average(&times, &lens(&times));
    for _ in 0..2 {
        // Each hit slides onto the template on its own: a search over ±3 ms in
        // quarter samples, by far the slowest part of the analysis, so the hits
        // are shared out over every core. The result does not depend on the split.
        let template = &tpl;
        let shift = |t: f32| {
            let mut best = (f32::MIN, 0.0f32);
            let mut d = -0.003 * r;
            while d <= 0.003 * r {
                let c: f32 = (0..head)
                    .map(|i| at(x, t + d + i as f32) * template[i])
                    .sum();
                if c > best.0 {
                    best = (c, d);
                }
                d += 0.25;
            }
            best.1
        };
        let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
        let per = times.len().div_ceil(threads).max(1);
        std::thread::scope(|s| {
            for chunk in times.chunks_mut(per) {
                let shift = &shift;
                s.spawn(move || {
                    for t in chunk {
                        *t += shift(*t);
                    }
                });
            }
        });
        tpl = average(&times, &lens(&times));
    }
    // Only a drum that really repeats: its hits must look like the template...
    let eh: f32 = tpl[..head].iter().map(|v| v * v).sum::<f32>().max(1e-12);
    let corr: Vec<f32> = times
        .iter()
        .map(|&t| {
            let c: f32 = (0..head).map(|i| at(x, t + i as f32) * tpl[i]).sum();
            let es: f32 = (0..head)
                .map(|i| at(x, t + i as f32).powi(2))
                .sum::<f32>()
                .max(1e-12);
            c / (es * eh).sqrt()
        })
        .collect();
    // ...and it must be percussive: most of its energy in the first 100 ms.
    let cut = |a: f32, b: f32| -> f32 {
        let (i, j) = (((a * r) as usize).min(len), ((b * r) as usize).min(len));
        tpl[i..j].iter().map(|v| v * v).sum::<f32>() / (j - i).max(1) as f32
    };
    if dsp::median(&corr) < 0.6 || cut(0.0, 0.1) < 2.5 * cut(0.15, 0.3) {
        return x.to_vec();
    }
    let ls = lens(&times);
    let mut out = x.to_vec();
    for (k, &t) in times.iter().enumerate() {
        let c: f32 = (0..head).map(|i| at(x, t + i as f32) * tpl[i]).sum();
        let g = (c / eh).clamp(0.5, 1.5);
        let s = t.ceil() as isize;
        // out[s + i] -= g * tpl(s + i - t), up to the next hit.
        for i in 0..ls[k] as isize {
            let j = s + i;
            if j < 0 || j as usize >= out.len() {
                continue;
            }
            out[j as usize] -= g * at(&tpl, j as f32 - t);
        }
    }
    out
}

/// Seconds the spectral-flux peak sits before the onset it marks (measured on
/// rendered drums; see tests/synthetic.rs).
pub const ONSET_LAG: f32 = 0.01;

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let flats = self.key.key.flats();
        let g = &self.grid;
        writeln!(
            f,
            "== {}  {}-{} ({:.1} s), {} Hz ==",
            if self.source.is_empty() {
                "audio"
            } else {
                &self.source
            },
            clock(self.from),
            clock(self.from + self.seconds),
            self.seconds,
            self.rate
        )?;
        writeln!(
            f,
            "\nTEMPO  {:.1} bpm  (confidence {:.2})",
            self.tempo.bpm, self.tempo.confidence
        )?;
        let cands: Vec<String> = self
            .tempo
            .candidates
            .iter()
            .map(|(b, s)| format!("{b:.1} ({s:.2})"))
            .collect();
        writeln!(f, "  other readings: {}", cands.join(", "))?;
        writeln!(
            f,
            "  grid: first downbeat {:.3} s, beat {:.3} s, bar {:.3} s, {}/4, {} whole bars",
            g.downbeat + self.from,
            g.beat(),
            g.bar_len(),
            g.beats_per_bar,
            g.bars
        )?;
        writeln!(
            f,
            "\nKEY  {}  (confidence {:.2}; runner-up {})",
            self.key.name, self.key.confidence, self.key.runner_up
        )?;
        writeln!(f, "  tuning {:+.0} cents from A440", self.key.tuning_cents)?;
        let pcs: Vec<String> = (0..12)
            .map(|i| {
                let pc = (self.key.key.root as usize + i) % 12;
                format!(
                    "{}={:.2}",
                    theory::pc_name(pc as u8, flats),
                    self.key.pitch_classes[pc]
                )
            })
            .collect();
        writeln!(f, "  pitch classes from the tonic: {}", pcs.join(" "))?;

        writeln!(f, "\nCHORDS  (per bar; [a b] = two chords in the bar)")?;
        for row in self.chords.chunks(8) {
            let cells: Vec<String> = row
                .iter()
                .map(|b| {
                    if b.chords.len() == 1 {
                        b.chords[0].name.clone()
                    } else {
                        format!(
                            "[{}]",
                            b.chords
                                .iter()
                                .map(|c| c.name.as_str())
                                .collect::<Vec<_>>()
                                .join(" ")
                        )
                    }
                })
                .collect();
            writeln!(
                f,
                "  {:>3}: {}",
                row[0].bar,
                cells
                    .iter()
                    .map(|c| format!("{c:<7}"))
                    .collect::<String>()
                    .trim_end()
            )?;
        }
        let mean_score = dsp::mean(
            &self
                .chords
                .iter()
                .flat_map(|b| b.chords.iter().map(|c| c.score))
                .collect::<Vec<_>>(),
        );
        writeln!(f, "  mean template fit {mean_score:.2}")?;

        writeln!(
            f,
            "\nBASS  ({} notes, pitched {:.0}% of the time; step/length in 16ths)",
            self.bass.notes.len(),
            self.bass.voiced * 100.0
        )?;
        for row in self.bass.bars.chunks(4) {
            let cells: Vec<String> = row
                .iter()
                .map(|b| {
                    if b.notes.is_empty() {
                        "-".into()
                    } else {
                        b.notes
                            .iter()
                            .map(|(s, l, k)| {
                                format!(
                                    "{}@{}/{}",
                                    theory::note_name(*k, flats),
                                    theory::step_name(*s),
                                    l
                                )
                            })
                            .collect::<Vec<_>>()
                            .join(" ")
                    }
                })
                .collect();
            writeln!(f, "  {:>3}: {}", row[0].bar, cells.join(" | "))?;
        }

        let d = &self.drums;
        writeln!(
            f,
            "\nDRUMS  (16ths; X strong x normal o soft g ghost; grid offset {:+.0} ms)",
            d.offset_ms
        )?;
        writeln!(
            f,
            "  most common bar ({} of {} bars):",
            d.common_count,
            d.bars.len()
        )?;
        for (k, name) in drums::LANES.iter().enumerate() {
            writeln!(f, "    {:<5} {}", name, d.common[k])?;
        }
        if d.common2_count >= 2 && d.common2[0].len() > d.common[0].len() {
            writeln!(f, "  most common 2-bar phrase ({} times):", d.common2_count)?;
            for (k, name) in drums::LANES.iter().enumerate() {
                writeln!(f, "    {:<5} {}", name, d.common2[k])?;
            }
        }
        let odd: Vec<&drums::DrumBar> = d.bars.iter().filter(|b| b.consistency < 0.999).collect();
        if !odd.is_empty() {
            writeln!(f, "  bars that differ (consistency with the common bar):")?;
            for b in odd.iter().take(24) {
                writeln!(
                    f,
                    "    {:>3} ({:.2}): k {}  s {}  h {}",
                    b.bar, b.consistency, b.lanes[0], b.lanes[1], b.lanes[2]
                )?;
            }
            if odd.len() > 24 {
                writeln!(f, "    ... {} more", odd.len() - 24)?;
            }
        }

        if !self.melody.notes.is_empty() {
            writeln!(
                f,
                "\nMELODY GUESS  (confidence {:.2}{}; {} notes)",
                self.melody.confidence,
                if self.melody.confidence < 0.5 {
                    ", LOW: a sketch"
                } else {
                    ""
                },
                self.melody.notes.len()
            )?;
            let keys: Vec<u8> = self.melody.notes.iter().map(|n| n.2).collect();
            writeln!(f, "  {}", melody::contour(&keys))?;
            let first: Vec<Note> = self.melody.notes.iter().take(24).copied().collect();
            writeln!(f, "  first notes: {}", melody::names(&first, flats))?;
        }

        writeln!(f, "\nARRANGEMENT")?;
        for s in &self.sections {
            writeln!(
                f,
                "  {}  bars {:>3}-{:<3} {:>6}-{:<6} {:>5.1} LUFS  centroid {:>5.0} Hz",
                s.label,
                s.start_bar,
                s.start_bar + s.bars - 1,
                clock(s.start + self.from),
                clock(s.end + self.from),
                s.loudness,
                s.brightness
            )?;
        }
        write!(f, "\n{}", self.sound)?;
        write!(f, "\n{}", self.leadsheet)
    }
}

impl fmt::Display for Sound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "SOUND")?;
        writeln!(
            f,
            "  loudness {:.1} LUFS integrated, loudest 3 s {:.1} LUFS, range {:.1} LU",
            self.lufs, self.lufs_loudest_3s, self.lra
        )?;
        writeln!(
            f,
            "  peak {:.1} dBFS, rms {:.1} dBFS, crest {:.1} dB{}",
            self.peak_db,
            self.rms_db,
            self.crest_db,
            if self.clipped > 0 {
                format!(", {} clipped samples", self.clipped)
            } else {
                String::new()
            }
        )?;
        writeln!(
            f,
            "  stereo width (side/mid): sub {:.2}  bass {:.2}  mids {:.2}  highs {:.2}; L/R correlation {:.2}",
            self.width[0], self.width[1], self.width[2], self.width[3], self.correlation
        )?;
        if self.mono_below_hz > 0.0 {
            writeln!(
                f,
                "  low end mono up to {:.0} Hz (width below 120 Hz {:.2})",
                self.mono_below_hz, self.low_width
            )?;
        } else {
            writeln!(
                f,
                "  low end is not mono (width below 120 Hz {:.2})",
                self.low_width
            )?;
        }
        writeln!(
            f,
            "  brightness: median spectral centroid {:.0} Hz; over time: {}",
            self.centroid_hz,
            self.centroid_over_time
                .iter()
                .map(|c| format!("{:.0}", c.1))
                .collect::<Vec<_>>()
                .join(" ")
        )?;
        writeln!(f, "  transients {:.1} onsets/s", self.onsets_per_s)?;
        match self.decay_rt60 {
            Some(rt) => writeln!(f, "  decay after hits ~{:.2} s RT60 (rough, from {} hits; reverb and release together)", rt, self.decay_events)?,
            None => writeln!(f, "  decay after hits: too few isolated hits to measure")?,
        }
        let p = &self.pump;
        if p.detected {
            writeln!(
                f,
                "  sidechain pump: YES, {:.1} dB dip {:.0} ms after the kick, back in {:.0} ms ({:.0}% of {} kicks)",
                p.depth_db,
                p.dip_ms,
                p.recovery_ms,
                p.consistency * 100.0,
                p.kicks
            )?;
        } else {
            writeln!(
                f,
                "  sidechain pump: no ({:.1} dB after kicks, {} kicks)",
                p.depth_db, p.kicks
            )?;
        }
        writeln!(f, "  third-octave spectrum (dB):")?;
        for row in self.third_octave.chunks(10) {
            let cells: Vec<String> = row
                .iter()
                .map(|(hz, db)| format!("{}:{:.0}", hz_label(*hz), db))
                .collect();
            writeln!(f, "    {}", cells.join(" "))?;
        }
        Ok(())
    }
}

pub fn hz_label(hz: f32) -> String {
    if hz >= 1000.0 {
        let k = hz / 1000.0;
        if (k - k.round()).abs() < 0.01 {
            format!("{}k", k.round())
        } else {
            format!("{k:.2}k").replace(".50k", ".5k").replace("0k", "k")
        }
    } else {
        format!("{}", hz)
    }
}
