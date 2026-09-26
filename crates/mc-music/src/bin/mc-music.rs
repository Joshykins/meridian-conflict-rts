//! mc-music: render and measure songs without the studio.
//!
//! mc-music check <song.ron>...                 parse, list problems
//! mc-music json <song.ron> [out.json]         the song as JSON, for scripts to edit
//! mc-music ron <in.json> <out.ron>             back to a song file, checked
//! mc-music render <song.ron> <out.wav> [opts]  write a WAV
//! mc-music analyse <song.ron> [opts]           print numbers about a render
//!
//! The desk (data/music/.studio, see mc_music::history), for working from a chat:
//! mc-music log <song>                          revisions and proposals
//! mc-music pending <song>                      proposals waiting for the user
//! mc-music propose <song> <file.ron> -m MSG [--group G --label A --question Q]
//! mc-music accept|reject <song> <id> [--comment C]
//! mc-music revert <song> <rev>
//! mc-music commit <song> -m MSG                snapshot the working copy
//! mc-music session                             what the studio shows now
//! mc-music inbox [--read]                      unread "Tell Claude" messages (--read marks them)
//! mc-music wait-inbox [--timeout S]            blocks until a message arrives, prints it
//! mc-music taste                               every verdict so far
//! mc-music reply "text" [--to m<time>]         answer in the studio's conversation
//! mc-music conversation                        both sides, oldest first
//!
//! opts: --mode song|director|section:NAME|pattern:NAME:TRACK  (default song)
//!       --seconds S   --intensity A[:B] (a ramp from A to B)  --rate R
//!       --tracks      also measure each track soloed
//!       --layers      also analyse each intensity step 0, .25, .5, .75, 1 in director mode

use mc_music::render::{analyse, render, render_arrangement, write_wav};
use mc_music::{Mode, Song};
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: mc-music check|render|analyse <song.ron> ...");
        std::process::exit(2);
    }
    let opt = |name: &str| -> Option<String> {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1).cloned())
    };
    let rate: u32 = opt("--rate").and_then(|v| v.parse().ok()).unwrap_or(48000);
    let seconds: Option<f32> = opt("--seconds").and_then(|v| v.parse().ok());
    let intensity = opt("--intensity")
        .map(|v| {
            let mut it = v.split(':').filter_map(|x| x.parse::<f32>().ok());
            let a = it.next().unwrap_or(1.0);
            (a, it.next().unwrap_or(a))
        })
        .unwrap_or((1.0, 1.0));
    match args[0].as_str() {
        "check" => {
            let mut bad = false;
            for p in &args[1..] {
                match Song::load(Path::new(p)) {
                    Ok(s) => {
                        let problems = s.problems();
                        println!(
                            "{p}: {} tracks, {} patterns, {} sections, {:.0} s arrangement",
                            s.tracks.len(),
                            s.patterns.len(),
                            s.sections.len(),
                            s.arrangement_ticks() as f64 * s.samples_per_tick(48000.0) / 48000.0
                        );
                        for pr in &problems {
                            println!("  problem: {pr}");
                        }
                        bad |= !problems.is_empty();
                    }
                    Err(e) => {
                        println!("{e}");
                        bad = true;
                    }
                }
            }
            if bad {
                std::process::exit(1);
            }
        }
        "render" | "analyse" => {
            let song = Song::load(Path::new(&args[1])).unwrap_or_else(|e| {
                eprintln!("{e}");
                std::process::exit(1)
            });
            let mode = parse_mode(&song, opt("--mode").as_deref());
            let started = std::time::Instant::now();
            let frames = match (&mode, seconds) {
                (Mode::Song, None) => render_arrangement(&song, rate, 3.0),
                (m, s) => render(&song, rate, s.unwrap_or(60.0), m.clone(), intensity),
            };
            let took = started.elapsed().as_secs_f32();
            let secs = frames.len() as f32 / rate as f32;
            if args[0] == "render" {
                let out = args
                    .get(2)
                    .filter(|a| !a.starts_with("--"))
                    .cloned()
                    .unwrap_or_else(|| "out.wav".into());
                write_wav(Path::new(&out), &frames, rate).expect("write wav");
                println!("wrote {out}");
            }
            println!("{}", analyse(&frames, rate));
            println!(
                "  rendered {secs:.1} s in {took:.2} s ({:.1}x real time)",
                secs / took.max(1e-6)
            );
            if args.iter().any(|a| a == "--tracks") {
                // Each track soloed, same render: where the loudness of the mix comes from.
                for (i, t) in song.tracks.iter().enumerate() {
                    let mut solo = song.clone();
                    for (j, u) in solo.tracks.iter_mut().enumerate() {
                        u.solo = j == i;
                    }
                    let f = match (&mode, seconds) {
                        (Mode::Song, None) => render_arrangement(&solo, rate, 3.0),
                        (m, s) => render(&solo, rate, s.unwrap_or(60.0), m.clone(), intensity),
                    };
                    let a = analyse(&f, rate);
                    println!(
                        "  {:<14} LUFS {:>6.1} peak {:>6.1}  bands {:?}  width {:.2}",
                        t.name,
                        a.lufs,
                        a.peak_db,
                        a.bands.map(|b| b.round()),
                        a.width
                    );
                }
            }
            if args.iter().any(|a| a == "--layers") {
                for step in [0.0, 0.25, 0.5, 0.75, 1.0] {
                    let f = render(
                        &song,
                        rate,
                        seconds.unwrap_or(30.0),
                        Mode::Director,
                        (step, step),
                    );
                    let a = analyse(&f, rate);
                    println!(
                        "intensity {step:.2}: LUFS {:.1} peak {:.1} bands {:?}",
                        a.lufs,
                        a.peak_db,
                        a.bands.map(|b| b.round())
                    );
                }
            }
        }
        "bench" => {
            // Render only, no analysis: what the engine costs. Run under `time` for CPU seconds.
            let song = Song::load(Path::new(&args[1])).unwrap_or_else(|e| {
                eprintln!("{e}");
                std::process::exit(1)
            });
            let mode = parse_mode(&song, opt("--mode").as_deref());
            let secs = seconds.unwrap_or(60.0);
            let started = std::time::Instant::now();
            let (frames, profile) = mc_music::render::render_profiled(
                &song,
                rate,
                secs,
                mode,
                intensity,
                args.iter().any(|a| a == "--profile"),
            );
            let took = started.elapsed().as_secs_f32();
            let sum: f32 = frames.iter().map(|f| f[0].abs()).sum();
            println!(
                "{secs:.0} s rendered in {took:.2} s wall = {:.2}% of a core (checksum {sum:.1})",
                took / secs * 100.0
            );
            if let Some(p) = profile {
                let pct = |t: f64| t / secs as f64 * 100.0;
                for (i, t) in song.tracks.iter().enumerate() {
                    println!(
                        "  {:<14} synth {:5.2}%  effects {:5.2}%",
                        t.name,
                        pct(p.synth.get(i).copied().unwrap_or(0.0)),
                        pct(p.chain.get(i).copied().unwrap_or(0.0))
                    );
                }
                for (i, b) in song.buses.iter().enumerate() {
                    println!(
                        "  bus {:<10} {:5.2}%",
                        b.name,
                        pct(p.buses.get(i).copied().unwrap_or(0.0))
                    );
                }
                println!(
                    "  master         {:5.2}%   sequencing {:5.2}%",
                    pct(p.master),
                    pct(p.other)
                );
            }
        }
        "json" => {
            // Song RON -> JSON on stdout (or to a file), for scripts to edit.
            let song = Song::load(Path::new(&args[1])).unwrap_or_else(|e| {
                eprintln!("{e}");
                std::process::exit(1)
            });
            let text = serde_json::to_string_pretty(&song).expect("json");
            match args.get(2).filter(|a| !a.starts_with("--")) {
                Some(out) => std::fs::write(out, text).expect("write json"),
                None => println!("{text}"),
            }
        }
        "ron" => {
            // JSON -> song RON, checked.
            let text = std::fs::read_to_string(&args[1]).expect("read json");
            let song: Song = serde_json::from_str(&text).unwrap_or_else(|e| {
                eprintln!("{}: {e}", args[1]);
                std::process::exit(1)
            });
            for p in song.problems() {
                eprintln!("problem: {p}");
            }
            let out = args.get(2).cloned().unwrap_or_else(|| {
                eprintln!("ron <in.json> <out.ron>");
                std::process::exit(2)
            });
            song.save(Path::new(&out)).expect("write ron");
            println!("wrote {out}");
        }
        "log" | "pending" | "propose" | "accept" | "reject" | "revert" | "session" | "inbox"
        | "wait-inbox" | "taste" | "commit" | "reply" | "conversation" => {
            desk_command(&args, &opt);
        }
        other => {
            eprintln!("unknown command {other}");
            std::process::exit(2);
        }
    }
}

fn parse_mode(song: &Song, m: Option<&str>) -> Mode {
    match m {
        None | Some("song") => Mode::Song,
        Some("director") => Mode::Director,
        Some(s) if s.starts_with("section:") => {
            let name = &s[8..];
            Mode::Section(song.section(name).unwrap_or_else(|| {
                eprintln!("no section {name}");
                std::process::exit(1)
            }))
        }
        Some(s) if s.starts_with("pattern:") => {
            let mut it = s[8..].split(':');
            let p = it.next().and_then(|n| song.pattern(n)).expect("pattern");
            let t = it.next().and_then(|n| song.track(n)).expect("track");
            Mode::Pattern {
                pattern: p,
                track: t,
            }
        }
        Some(s) => {
            eprintln!("unknown mode {s}");
            std::process::exit(1)
        }
    }
}

/// `data/music`, found by walking up from the working directory.
fn music_dir() -> std::path::PathBuf {
    let mut dir = std::env::current_dir().expect("cwd");
    loop {
        let m = dir.join("data").join("music");
        if m.is_dir() {
            return m;
        }
        if !dir.pop() {
            eprintln!("no data/music above the working directory");
            std::process::exit(1);
        }
    }
}

fn when(t: u64) -> String {
    let ago = mc_music::history::now().saturating_sub(t);
    match ago {
        0..=59 => format!("{ago}s ago"),
        60..=3599 => format!("{}m ago", ago / 60),
        3600..=86399 => format!("{}h ago", ago / 3600),
        _ => format!("{}d ago", ago / 86400),
    }
}

fn print_entry(e: &mc_music::history::Entry) {
    let tag = match (&e.group, &e.label) {
        (Some(g), Some(l)) => format!(" [{g}/{l}]"),
        (Some(g), None) => format!(" [{g}]"),
        _ => String::new(),
    };
    println!(
        "{} {:?}{} by {} {}: {}",
        e.id,
        e.status,
        tag,
        e.author,
        when(e.time),
        e.message
    );
    for c in &e.changes {
        println!("    {c}");
    }
    if !e.reactions.is_empty() || !e.comment.is_empty() {
        let r: Vec<&str> = e.reactions.iter().map(|r| r.name()).collect();
        println!("    > {} {}", r.join(", "), e.comment);
    }
    if !e.question.is_empty() {
        println!("    ? {}", e.question);
    }
}

fn print_message(m: &mc_music::history::Message) {
    println!("[{}] {}", when(m.time), m.text);
    let s = &m.session;
    println!(
        "  song {} ({}), {} mode, view {}, {}, section {}, at {}, intensity {:.2}",
        s.song,
        s.head.as_deref().unwrap_or("-"),
        s.mode,
        s.view,
        if s.playing { "playing" } else { "stopped" },
        s.section.as_deref().unwrap_or("-"),
        s.position,
        s.intensity
    );
    if let Some(t) = &s.track {
        println!(
            "  track {t}, pattern {}",
            s.pattern.as_deref().unwrap_or("-")
        );
    }
    if let Some((a, b)) = s.loop_bars {
        println!("  loop bars {a}-{b}");
    }
    if !s.notes.is_empty() {
        println!("  selected notes {:?}", s.notes);
    }
    if let Some(r) = &s.reference {
        println!("  reference {r} {:?}", s.reference_span);
    }
    if let Some(p) = &m.sketch {
        println!("  sketch {} beats: {:?}", p.beats, p.notes);
    }
}

fn desk_command(args: &[String], opt: &dyn Fn(&str) -> Option<String>) {
    use mc_music::history::{Desk, Reaction};
    let desk = Desk::open(&music_dir());
    let song = || {
        args.get(1).cloned().unwrap_or_else(|| {
            eprintln!("which song?");
            std::process::exit(2)
        })
    };
    let fail = |e: String| -> ! {
        eprintln!("{e}");
        std::process::exit(1)
    };
    let message = opt("-m").or_else(|| opt("--message")).unwrap_or_default();
    match args[0].as_str() {
        "log" => {
            for e in desk.log(&song()).entries {
                print_entry(&e);
            }
        }
        "pending" => {
            for e in desk.pending(&song()) {
                print_entry(&e);
            }
        }
        "commit" => match desk.commit(
            &song(),
            &Song::load(&desk.song_path(&song())).unwrap_or_else(|e| fail(e)),
            "claude",
            &message,
        ) {
            Ok(Some(e)) => print_entry(&e),
            Ok(None) => println!("no change since the last revision"),
            Err(e) => fail(e),
        },
        "propose" => {
            let file = args
                .get(2)
                .unwrap_or_else(|| fail("propose <song> <file.ron> -m MSG".into()));
            let proposed = Song::load(Path::new(file)).unwrap_or_else(|e| fail(e));
            let problems = proposed.problems();
            if !problems.is_empty() {
                fail(format!("the proposal has problems: {problems:?}"));
            }
            let q = opt("--question").unwrap_or_default();
            match desk.propose(
                &song(),
                &proposed,
                &message,
                opt("--group").as_deref(),
                opt("--label").as_deref(),
                &q,
            ) {
                Ok(e) => print_entry(&e),
                Err(e) => fail(e),
            }
        }
        "accept" | "reject" => {
            let id = args
                .get(2)
                .cloned()
                .unwrap_or_else(|| fail("which proposal?".into()));
            let comment = opt("--comment").unwrap_or_default();
            let none: [Reaction; 0] = [];
            let r = if args[0] == "accept" {
                desk.accept(&song(), &id, &none, &comment)
                    .map(|e| print_entry(&e))
            } else {
                desk.reject(&song(), &id, &none, &comment)
                    .map(|_| println!("rejected {id}"))
            };
            r.unwrap_or_else(|e| fail(e));
        }
        "revert" => {
            let id = args
                .get(2)
                .cloned()
                .unwrap_or_else(|| fail("which revision?".into()));
            match desk.revert(&song(), &id, "claude") {
                Ok(Some(e)) => print_entry(&e),
                Ok(None) => println!("already at {id}"),
                Err(e) => fail(e),
            }
        }
        "session" => match desk.session() {
            Some(s) => {
                let m = mc_music::history::Message {
                    time: s.time,
                    text: "(current session)".into(),
                    from: "you".into(),
                    reply_to: None,
                    session: s,
                    sketch: None,
                    read: true,
                };
                print_message(&m);
            }
            None => println!("no studio session"),
        },
        "inbox" => {
            let mark = args.iter().any(|a| a == "--read");
            let unread = desk.unread();
            if unread.is_empty() {
                println!("no unread messages");
            }
            for (path, m) in unread {
                print_message(&m);
                if mark {
                    let _ = desk.mark_read(&path);
                }
            }
        }
        "wait-inbox" => {
            let timeout: u64 = opt("--timeout")
                .and_then(|v| v.parse().ok())
                .unwrap_or(3600);
            let started = std::time::Instant::now();
            loop {
                let unread = desk.unread();
                if !unread.is_empty() {
                    for (path, m) in unread {
                        print_message(&m);
                        let _ = desk.mark_read(&path);
                    }
                    return;
                }
                if started.elapsed().as_secs() >= timeout {
                    println!("no message");
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
        }
        "reply" => {
            // mc-music reply "text" [--to <message time>]: shown in the studio's conversation.
            let text = args
                .get(1)
                .cloned()
                .unwrap_or_else(|| fail("reply \"text\"".into()));
            let to = opt("--to").and_then(|v| v.trim_start_matches('m').parse().ok());
            match desk.reply(&text, to) {
                Ok(p) => println!("replied ({})", p.display()),
                Err(e) => fail(e),
            }
        }
        "conversation" => {
            for m in desk.conversation() {
                println!("[{}] {}: {}", when(m.time), m.from, m.text);
            }
        }
        "taste" => {
            for v in desk.taste().verdicts {
                let r: Vec<&str> = v.reactions.iter().map(|r| r.name()).collect();
                println!(
                    "{} {} {:?} {}: {} | {} {}",
                    when(v.time),
                    v.song,
                    v.status,
                    v.proposal,
                    v.message,
                    r.join(", "),
                    v.comment
                );
            }
        }
        _ => unreachable!(),
    }
}
