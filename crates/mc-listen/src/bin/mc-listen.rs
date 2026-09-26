//! mc-listen: facts about music for an AI that cannot hear it.
//!
//! mc-listen analyse <file> [--from T --to T] [--json] [--tempo BPM] [--key C:minor] [--beats N] [--no-melody] [--ron-dir DIR]
//! mc-listen leadsheet <file> [--from T --to T] [--tempo BPM] [--key K] [--json]
//! mc-listen leadsheet --song <song.ron> [--section NAME] [--intensity X] [--json]
//! mc-listen transcribe <file> [--tempo BPM] [--key C:minor] [--grid 16] [--align] [--bass] [--ron out.ron] [--from T --to T]
//! mc-listen compare <reference audio|stem> <song.ron | ours.wav> [--mode song|director] [--intensity X] [--seconds S]
//!                   [--section NAME] [--from T --to T] [--tempo BPM] [--json]
//! mc-listen compare --song-ref reference.ron --song ours.ron [--section NAME] [--ref-section NAME]   (chart vs chart)
//! mc-listen compare <reference audio> --song-ref reference.ron --song ours.ron                    (their chart + sound)
//! mc-listen chroma <file> [--from T --to T] [--step S]
//! mc-listen spectrum <file> [--from T --to T]
//!
//! Times are seconds ("92.5") or minutes:seconds ("1:32.5").

use mc_listen::analyse::{hz_label, Options};
use mc_listen::pitch::{self, TranscribeOpts};
use mc_listen::{decode, dsp, features, leadsheet, theory, Audio, Key};
use mc_music::render::{render, render_arrangement};
use mc_music::{Mode, Song, PPQ};
use std::path::Path;

const USAGE: &str = "usage: mc-listen analyse|leadsheet|transcribe|compare|chroma|spectrum <file> ... (see the source header for options)";

struct Args(Vec<String>);

impl Args {
    fn opt(&self, name: &str) -> Option<String> {
        self.0.iter().position(|a| a == name).and_then(|i| self.0.get(i + 1).cloned())
    }
    fn flag(&self, name: &str) -> bool {
        self.0.iter().any(|a| a == name)
    }
    fn f32(&self, name: &str) -> Option<f32> {
        self.opt(name).and_then(|v| v.parse().ok())
    }
    fn time(&self, name: &str) -> Option<f32> {
        self.opt(name).and_then(|v| parse_time(&v))
    }
    /// Positional arguments after the command (not option values).
    fn positional(&self) -> Vec<String> {
        let valued = ["--song-ref", "--ref-section", "--from", "--to", "--tempo", "--key", "--beats", "--ron-dir", "--song", "--section", "--intensity", "--grid", "--ron", "--mode", "--seconds", "--step", "--rate"];
        let mut out = Vec::new();
        let mut i = 1;
        while i < self.0.len() {
            let a = &self.0[i];
            if valued.contains(&a.as_str()) {
                i += 2;
                continue;
            }
            if !a.starts_with("--") {
                out.push(a.clone());
            }
            i += 1;
        }
        out
    }
}

fn parse_time(s: &str) -> Option<f32> {
    if let Some((m, sec)) = s.split_once(':') {
        Some(m.parse::<f32>().ok()? * 60.0 + sec.parse::<f32>().ok()?)
    } else {
        s.parse().ok()
    }
}

fn die(msg: &str) -> ! {
    eprintln!("{msg}");
    std::process::exit(1)
}

fn load(path: &str) -> Audio {
    decode::load(Path::new(path)).unwrap_or_else(|e| die(&e))
}

fn options(args: &Args, source: &str) -> Options {
    Options {
        from: args.time("--from"),
        to: args.time("--to"),
        beats_per_bar: args.opt("--beats").and_then(|v| v.parse().ok()).unwrap_or(4),
        tempo: args.f32("--tempo"),
        key: args.opt("--key").map(|k| Key::parse(&k).unwrap_or_else(|| die(&format!("cannot read key \"{k}\" (try C:minor)")))),
        melody: !args.flag("--no-melody"),
        source: source.to_string(),
    }
}

fn name_of(path: &str) -> String {
    Path::new(path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| path.to_string())
}

fn main() {
    let args = Args(std::env::args().skip(1).collect());
    let Some(cmd) = args.0.first().cloned() else { die(USAGE) };
    let pos = args.positional();
    match cmd.as_str() {
        "analyse" | "analyze" => {
            let file = pos.first().unwrap_or_else(|| die(USAGE));
            let audio = load(file);
            let t0 = std::time::Instant::now();
            let report = mc_listen::reference(&audio, &options(&args, &name_of(file)));
            if args.flag("--json") {
                println!("{}", serde_json::to_string_pretty(&report).unwrap());
            } else {
                print!("{report}");
                println!("\n(analysed {:.1} s of audio in {:.1} s)", report.seconds, t0.elapsed().as_secs_f32());
            }
            if let Some(dir) = args.opt("--ron-dir") {
                std::fs::create_dir_all(&dir).unwrap_or_else(|e| die(&format!("{dir}: {e}")));
                let bass = pitch::to_pattern("bass", &report.bass.notes, report.grid.beats_per_bar);
                let mel = pitch::to_pattern("melody", &report.melody.notes, report.grid.beats_per_bar);
                for (n, p) in [("bass.ron", bass), ("melody.ron", mel)] {
                    let path = Path::new(&dir).join(n);
                    std::fs::write(&path, pitch::pattern_ron(&p)).unwrap_or_else(|e| die(&format!("{}: {e}", path.display())));
                    eprintln!("wrote {}", path.display());
                }
            }
        }
        "leadsheet" => {
            let sheet = if let Some(song_path) = args.opt("--song") {
                let song = Song::load(Path::new(&song_path)).unwrap_or_else(|e| die(&e));
                leadsheet::from_song(&song, args.opt("--section").as_deref(), args.f32("--intensity").unwrap_or(1.0)).unwrap_or_else(|e| die(&e))
            } else {
                let file = pos.first().unwrap_or_else(|| die(USAGE));
                let audio = load(file);
                mc_listen::reference(&audio, &options(&args, &name_of(file))).leadsheet
            };
            if args.flag("--json") {
                println!("{}", serde_json::to_string_pretty(&sheet).unwrap());
            } else {
                print!("{sheet}");
            }
        }
        "transcribe" => {
            let file = pos.first().unwrap_or_else(|| die(USAGE));
            let audio = load(file).span(args.time("--from"), args.time("--to"));
            let tempo = args.f32("--tempo").unwrap_or(120.0);
            let mut o = if args.flag("--bass") { TranscribeOpts::bass(tempo) } else { TranscribeOpts::voice(tempo) };
            if let Some(k) = args.opt("--key") {
                let k = Key::parse(&k).unwrap_or_else(|| die(&format!("cannot read key \"{k}\"")));
                o.key = Some((k.root, k.scale()));
            }
            if let Some(g) = args.opt("--grid").and_then(|g| g.parse::<u32>().ok()) {
                o.grid = (PPQ * 4 / g.max(1)).max(1);
            }
            o.align_first = args.flag("--align");
            let x = audio.mono();
            let x = if args.flag("--bass") { dsp::band(&x, audio.rate, 0.0, 250.0) } else { x };
            let t = pitch::transcribe_with(&x, audio.rate, &o);
            let flats = o.key.map(|(r, s)| Key { root: r, minor: s != mc_music::song::Scale::Major }.flats()).unwrap_or(false);
            println!("{} notes at {tempo} bpm (grid {} ticks; tuning {:+.0} cents taken out; tick 0 = {:.3} s)", t.notes.len(), o.grid, t.tuning_cents, t.origin);
            println!("  {:>7} {:>6} {:>5}  {:<5} {:>5}  {:>5}  {:>9}", "start", "len", "tick", "note", "vel", "conf", "cents off");
            for (n, e) in t.notes.iter().zip(&t.events) {
                let beat = n.0 as f32 / PPQ as f32;
                println!(
                    "  {:>6.3}s {:>5.3}s {:>5}  {:<5} {:>5}  {:>5.2}  {:>+9.0}  bar {} beat {:.2}",
                    e.start,
                    e.end - e.start,
                    n.0,
                    theory::note_name(n.2, flats),
                    n.3,
                    e.confidence,
                    (e.pitch - t.tuning_cents / 100.0 - n.2 as f32) * 100.0,
                    (beat / 4.0).floor() as u32 + 1,
                    beat % 4.0 + 1.0
                );
            }
            if let Some(out) = args.opt("--ron") {
                let p = pitch::to_pattern(&name_of(file).replace('.', "_"), &t.notes, 4);
                std::fs::write(&out, pitch::pattern_ron(&p) + "\n").unwrap_or_else(|e| die(&format!("{out}: {e}")));
                println!("wrote {out}");
            }
        }
        "compare" => {
            // Reference: an audio file (or stem), or a Song (--song-ref, charted exactly), or both
            // (--song-ref for the chart plus an audio file for the sound).
            // Ours: a song file (--song or positional .ron, rendered for the sound) or an audio file.
            let mut pos = pos.clone();
            let song_ref = args.opt("--song-ref");
            let ours_path = args.opt("--song").or_else(|| pos.iter().rposition(|p| p.ends_with(".ron")).map(|i| pos.remove(i))).or_else(|| if pos.len() >= 2 { Some(pos.remove(1)) } else { None });
            let ref_audio = pos.first().cloned();
            let Some(ours_path) = ours_path else { die(USAGE) };
            if song_ref.is_none() && ref_audio.is_none() {
                die(USAGE);
            }
            let intensity = args.f32("--intensity").unwrap_or(1.0);
            let section = args.opt("--section");
            let ref_sheet = song_ref.as_ref().map(|p| {
                let s = Song::load(Path::new(p)).unwrap_or_else(|e| die(&e));
                let sec = args.opt("--ref-section").or_else(|| section.clone().filter(|n| s.section(n).is_some()));
                leadsheet::from_song(&s, sec.as_deref(), intensity).unwrap_or_else(|e| die(&e))
            });
            let ours_is_song = ours_path.ends_with(".ron");
            let ours_song = ours_is_song.then(|| Song::load(Path::new(&ours_path)).unwrap_or_else(|e| die(&e)));
            let ours_sheet = ours_song.as_ref().map(|s| leadsheet::from_song(s, section.as_deref(), intensity).unwrap_or_else(|e| die(&e)));
            let Some(ref_file) = ref_audio else {
                // Chart against chart only.
                let (r, o) = (ref_sheet.unwrap(), ours_sheet.unwrap_or_else(|| die("with --song-ref and no reference audio, ours must be a song .ron")));
                let findings = mc_listen::compare_charts(&o, &r);
                if args.flag("--json") {
                    println!("{}", serde_json::to_string_pretty(&findings).unwrap());
                    return;
                }
                let mut findings = findings;
                findings.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
                println!("== chart vs chart: {} (reference) vs {} (ours): {} differences ==", r.title, o.title, findings.len());
                for f in &findings {
                    println!("  [{:>4.1} {:<7}] {}", f.score, f.topic, f.text);
                }
                println!("\n-- reference --\n{r}\n-- ours --\n{o}");
                return;
            };
            let reference_audio = load(&ref_file);
            let ropts = options(&args, &name_of(&ref_file));
            let mut reference = mc_listen::reference(&reference_audio, &ropts);
            if let Some(sheet) = ref_sheet {
                reference.leadsheet = sheet;
            }
            let mut oo = Options { source: name_of(&ours_path), beats_per_bar: ropts.beats_per_bar, ..Options::default() };
            let ours = if let Some(song) = &ours_song {
                let rate = 48000;
                let seconds = args.f32("--seconds");
                let frames = if let Some(name) = &section {
                    let si = song.section(name).unwrap_or_else(|| die(&format!("no section \"{name}\"")));
                    let len = song.section_ticks(&song.sections[si]) as f64 * song.samples_per_tick(rate as f32) / rate as f64;
                    let reps = (20.0 / len).ceil().max(1.0);
                    render(song, rate, seconds.unwrap_or((len * reps) as f32), Mode::Section(si), (intensity, intensity))
                } else if args.opt("--mode").as_deref() == Some("director") {
                    render(song, rate, seconds.unwrap_or(60.0), Mode::Director, (intensity, intensity))
                } else if let Some(s) = seconds {
                    render(song, rate, s, Mode::Song, (intensity, intensity))
                } else {
                    render_arrangement(song, rate, 1.0)
                };
                oo.tempo = Some(song.tempo);
                let mut r = mc_listen::reference(&Audio::new(rate, frames), &oo);
                // Our chart is exact: from the notes, not from listening.
                r.leadsheet = ours_sheet.unwrap();
                r
            } else {
                mc_listen::reference(&load(&ours_path), &oo)
            };
            let findings = mc_listen::compare_ranked(&ours, &reference);
            if args.flag("--json") {
                println!("{}", serde_json::to_string_pretty(&findings).unwrap());
                return;
            }
            println!("== {} (reference) vs {} (ours): {} differences, most audible first ==", reference.source, ours.source, findings.len());
            for f in &findings {
                println!("  [{:>4.1} {:<7}] {}", f.score, f.topic, f.text);
            }
            println!("\n-- reference --\n{}", reference.leadsheet);
            println!("-- ours --\n{}", ours.leadsheet);
        }
        "chroma" => {
            let file = pos.first().unwrap_or_else(|| die(USAGE));
            let audio = load(file).span(args.time("--from"), args.time("--to"));
            let x = audio.mono();
            let fb = features::pass_b(&x, audio.rate, 0.02);
            let tuning = features::tuning(&fb);
            let ch = features::chroma(&fb, tuning, 80.0, 5000.0);
            let step = args.f32("--step").unwrap_or(0.5).max(0.02);
            let per = (step / fb.hop).round().max(1.0) as usize;
            let tpl = theory::templates(true);
            println!("chroma every {step} s (0-9 per pitch class, 9 = strongest in the slice), tuning {:+.0} cents", tuning * 100.0);
            println!("  {:>7}  C  C# D  D# E  F  F# G  G# A  A# B   best chord", "time");
            let from = args.time("--from").unwrap_or(0.0);
            for (i, c) in ch.chunks(per).enumerate() {
                let mut s = [0.0f32; 12];
                for f in c {
                    for k in 0..12 {
                        s[k] += f[k];
                    }
                }
                let m = s.iter().cloned().fold(0.0f32, f32::max);
                let cells: String = s.iter().map(|v| if m > 0.0 { format!("{:<3}", ((v / m) * 9.0).round() as u32) } else { ".  ".into() }).collect();
                let chord = if m > 0.0 { theory::match_chords(&s, None, &tpl)[0].0.name(false) } else { "-".into() };
                println!("  {:>7}  {} {}", dsp::clock(from + i as f32 * step), cells, chord);
            }
        }
        "spectrum" => {
            let file = pos.first().unwrap_or_else(|| die(USAGE));
            let audio = load(file).span(args.time("--from"), args.time("--to"));
            let x = audio.mono();
            let fa = features::pass_a(&x, audio.rate);
            let fb = features::pass_b(&x, audio.rate, 0.02);
            let s = mc_listen::sound::measure(&audio, &fa, &fb, &[], &[], 0.5);
            println!("third-octave average spectrum, dB (a full-scale sine ~ 0); {:.1} LUFS", s.lufs);
            let top = s.third_octave.iter().map(|t| t.1).fold(f32::MIN, f32::max);
            for (hz, db) in &s.third_octave {
                let n = ((db - top + 60.0).max(0.0) / 1.5) as usize;
                println!("  {:>6} {:>6.1}  {}", hz_label(*hz), db, "#".repeat(n));
            }
            println!("  centroid {:.0} Hz; width sub {:.2} bass {:.2} mids {:.2} highs {:.2}", s.centroid_hz, s.width[0], s.width[1], s.width[2], s.width[3]);
        }
        _ => die(USAGE),
    }
}
