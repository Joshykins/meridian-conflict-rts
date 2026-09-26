//! `mc-bake`: bakes a procedural `.mcmap`.
//!
//! ```text
//! mc-bake --size-km 80 --seed 7 --name "Meridian Basin" -o maps/meridian_basin.mcmap
//! mc-bake --layout islands --size-km 10 --seed 46 --name "Twin Shoals" -o maps/twin_shoals.mcmap
//! mc-bake --layout alpine --size-km 8 --seed 3 --name "Serac Divide" -o maps/serac_divide.mcmap
//! mc-bake --layout alpine-teams --size-km 12 --seed 5 --name "Serac Sound" -o maps/serac_sound.mcmap
//! mc-bake --layout archipelago --size-km 20 --seed 23 --name "The Axis" -o maps/the_axis.mcmap
//! mc-bake --layout twin-bays --size-km 16 --seed 7 --name "Halden's Grip" -o maps/haldens_grip.mcmap
//! mc-bake --layout threshold --size-km 16 --seed 31 --name "The Threshold" -o maps/threshold.mcmap
//! ```
//!
//! Every layout but the survival ones is then stamped with starting wreckage
//! (`mc_map::wreckage`). `--wreckage-only` lays it afresh on a map already baked,
//! terrain untouched: give it the layout and seed the map was baked with.

use mc_map::wreckage::{self, Symmetry};
use mc_map::{bake, BakeParams, Layout, MapFile, Prop};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

const USAGE: &str = "\
usage: mc-bake -o <file.mcmap> [options]
  --size-km <n>    map edge, a multiple of the 2.048 km tile given in whole
                   \"kilometres\" of 1024 m: 4, 8, 16, ... 80 (default 16)
  --seed <n>       terrain seed (default 1)
  --name <text>    map name (default: the output file's stem)
  --layout <kind>  basin: a continent around a central city (default)
                   islands: a 1v1 main island with a central lake and two
                   town islands; wants 6 km or more
                   alpine: \"Serac Divide\", a 1v1 mountain map on a
                   coast, glaciers and a fjord; made for 8 km
                   alpine-teams: \"Serac Sound\", the same country for
                   4v4, sea down the east side; made for 12 km
                   archipelago: \"The Axis\", 4v4 naval: every player on an
                   island, cays between, a jungle island in the middle
                   round the Meridian's blue hole; made for 20 km
                   threshold: \"The Threshold\", the survival map: one road
                   along a coast into a Precursor facility; 3 defender
                   starts and the facility's; made for 16 km
                   twin-bays: \"Halden's Grip\", 4v4 across a land bridge
                   between two bays (after Seton's Clutch); made for 16 km
  --players <n>    start positions, 1-8 (default: 2 up to 8 km, 4 up to 24 km,
                   else 8; islands and alpine: always 2)
  --threads <n>    worker threads (default: all cores; does not change the result)
  --preview <ppm>  also write a shaded overview image with markers
  --verify         re-read the file and check its content id
  --no-wreckage    leave out the starting wreckage
  --wreckage-only  lay the starting wreckage on the map already at -o,
                   replacing any it has; the terrain is kept. Give the
                   --layout and --seed it was baked with";

struct Args {
    out: PathBuf,
    size_km: u32,
    seed: u64,
    name: Option<String>,
    layout: Layout,
    players: Option<u32>,
    threads: usize,
    preview: Option<PathBuf>,
    verify: bool,
    no_wreckage: bool,
    wreckage_only: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        out: PathBuf::new(),
        size_km: 16,
        seed: 1,
        name: None,
        layout: Layout::Basin,
        players: None,
        threads: 0,
        preview: None,
        verify: false,
        no_wreckage: false,
        wreckage_only: false,
    };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        let mut value = || it.next().ok_or(format!("{flag} needs a value"));
        let number = |v: String| {
            v.parse::<u64>()
                .map_err(|_| format!("'{v}' is not a number"))
        };
        match flag.as_str() {
            "-o" | "--out" => args.out = value()?.into(),
            "--size-km" => args.size_km = number(value()?)? as u32,
            "--seed" => args.seed = number(value()?)?,
            "--name" => args.name = Some(value()?),
            "--layout" => {
                args.layout = match value()?.as_str() {
                    "basin" => Layout::Basin,
                    "islands" => Layout::Islands,
                    "alpine" => Layout::Alpine,
                    "alpine-teams" => Layout::AlpineTeams,
                    "archipelago" => Layout::Archipelago,
                    "twin-bays" => Layout::TwinBays,
                    "threshold" => Layout::Threshold,
                    other => {
                        return Err(format!(
                            "unknown layout '{other}' (basin, islands, alpine, alpine-teams, archipelago, twin-bays or threshold)"
                        ))
                    }
                }
            }
            "--players" => args.players = Some(number(value()?)? as u32),
            "--threads" => args.threads = number(value()?)? as usize,
            "--preview" => args.preview = Some(value()?.into()),
            "--verify" => args.verify = true,
            "--no-wreckage" => args.no_wreckage = true,
            "--wreckage-only" => args.wreckage_only = true,
            "-h" | "--help" => return Err(String::new()),
            other => return Err(format!("unknown argument '{other}'")),
        }
    }
    if args.out.as_os_str().is_empty() {
        return Err("no output file given (-o)".into());
    }
    if args.size_km == 0 || !args.size_km.is_multiple_of(2) || args.size_km > 80 {
        return Err("--size-km must be an even number from 2 to 80 (tiles are 2.048 km)".into());
    }
    if args.layout == Layout::Islands && args.players.is_some_and(|n| n != 2) {
        return Err("--layout islands is a two-player layout (--players 2)".into());
    }
    if args.layout == Layout::TwinBays && args.players.is_some_and(|n| n != 8) {
        return Err("--layout twin-bays is an eight-player layout (--players 8)".into());
    }
    if args.layout == Layout::Archipelago && args.players.is_some_and(|n| n != 8) {
        return Err("--layout archipelago is an eight-player layout (--players 8)".into());
    }
    if args.layout == Layout::AlpineTeams && args.players.is_some_and(|n| n != 8) {
        return Err("--layout alpine-teams is an eight-player layout (--players 8)".into());
    }
    if args.layout == Layout::Alpine && args.players.is_some_and(|n| n != 2) {
        return Err("--layout alpine is a two-player layout (--players 2)".into());
    }
    if args.layout == Layout::Threshold && args.players.is_some_and(|n| n != 4) {
        return Err("--layout threshold has exactly 4 starts (--players 4)".into());
    }
    Ok(args)
}


fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(args) => args,
        Err(message) => {
            if !message.is_empty() {
                eprintln!("mc-bake: {message}\n");
            }
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("mc-bake: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    let name = args.name.clone().unwrap_or_else(|| {
        args.out
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
    });
    let mut params = match args.layout {
        Layout::Basin => BakeParams::square(&name, args.size_km / 2, args.seed),
        Layout::Islands => BakeParams::islands(&name, args.size_km / 2, args.seed),
        Layout::Alpine => BakeParams::alpine(&name, args.size_km / 2, args.seed),
        Layout::AlpineTeams => BakeParams::alpine_teams(&name, args.size_km / 2, args.seed),
        Layout::Archipelago => BakeParams::archipelago(&name, args.size_km / 2, args.seed),
        Layout::TwinBays => BakeParams::twin_bays(&name, args.size_km / 2, args.seed),
        Layout::Threshold => BakeParams::threshold(&name, args.size_km / 2, args.seed),
    };
    params.threads = args.threads;
    if let Some(players) = args.players {
        params.players = players;
    }
    if let Some(dir) = args.out.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)?;
    }

    if args.wreckage_only {
        let players = MapFile::open(&args.out)?.start_positions().len() as u32;
        let symmetry = Symmetry::of(args.layout, players)
            .ok_or("this layout takes no starting wreckage")?;
        lay_wreckage(args, symmetry)?;
        return check(args);
    }

    let started = Instant::now();
    let report = bake(&params, &args.out)?;
    let seconds = started.elapsed().as_secs_f64();
    println!("{}: \"{}\"", args.out.display(), name);
    println!(
        "  {} x {} tiles ({:.2} km), {} players, seed {}",
        params.tiles_w,
        params.tiles_h,
        params.tiles_w as f64 * 2.048,
        params.players,
        params.seed
    );
    println!("  content id {:016x}", report.content_id);
    println!(
        "  {}% land, {} ore fields",
        report.land_percent, report.ore_regions
    );
    println!(
        "  {} trees, {} rocks, {} buildings, {} precursor artifacts",
        report.trees, report.rocks, report.buildings, report.precursor
    );
    println!(
        "  {:.1} MB in {seconds:.2} s",
        report.file_bytes as f64 / 1.0e6
    );
    match Symmetry::of(args.layout, params.players) {
        Some(symmetry) if !args.no_wreckage => lay_wreckage(args, symmetry)?,
        _ => {}
    }
    check(args)
}

fn lay_wreckage(args: &Args, symmetry: Symmetry) -> Result<(), Box<dyn std::error::Error>> {
    let started = Instant::now();
    let w = wreckage::stamp(&args.out, symmetry, args.seed)?;
    println!(
        "  wreckage: {} wrecks in {} scrap piles, {} fields and {} sea fields, weight {:.0} a player; {} trees cleared ({:.2} s)",
        w.wrecks,
        w.scrap_piles,
        w.fields,
        w.sea_fields,
        w.weight_per_player,
        w.trees_cleared,
        started.elapsed().as_secs_f64()
    );
    println!("  content id {:016x}", w.content_id);
    Ok(())
}

fn check(args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    if args.verify || args.preview.is_some() {
        let map = MapFile::open(&args.out)?;
        if args.verify {
            let started = Instant::now();
            map.verify()?;
            println!("  verified in {:.2} s", started.elapsed().as_secs_f64());
        }
        if let Some(path) = &args.preview {
            write_preview(&map, path)?;
            println!("  preview: {}", path.display());
        }
    }
    Ok(())
}

/// Overview samples (every 4 cells) to snow layer samples (every 2).
const OVERVIEW_TO_SNOW: u32 = mc_map::OVERVIEW_STRIDE / mc_map::format::SNOW_STRIDE;

/// Shaded relief of the overview as a binary PPM, at most ~1300 px on a side.
fn write_preview(map: &MapFile, path: &Path) -> std::io::Result<()> {
    let (ow, oh) = map.overview_dims();
    let stride = ow.max(oh).div_ceil(1300).max(1);
    let (w, h) = ((ow - 1) / stride + 1, (oh - 1) / stride + 1);
    let info = map.info();
    let z = |x: u32, y: u32| {
        let s = map.overview()[(y.min(oh - 1) * ow + x.min(ow - 1)) as usize];
        info.sample_to_height(s).to_f64()
    };
    let metres_per_px = 32.0 * stride as f64;
    let mut rgb = vec![0u8; (w * h * 3) as usize];
    for py in 0..h {
        for px in 0..w {
            let (x, y) = (px * stride, py * stride);
            let height = z(x, y);
            let base = if height <= 0.0 {
                let depth = (-height / 60.0).clamp(0.0, 1.0);
                [
                    40.0 - 25.0 * depth,
                    95.0 - 45.0 * depth,
                    150.0 - 50.0 * depth,
                ]
            } else {
                let ramp = [
                    (0.0, [196.0, 186.0, 140.0]),
                    (8.0, [120.0, 160.0, 90.0]),
                    (40.0, [96.0, 140.0, 80.0]),
                    (70.0, [150.0, 150.0, 100.0]),
                    (130.0, [140.0, 125.0, 105.0]),
                    (260.0, [170.0, 165.0, 160.0]),
                    (420.0, [245.0, 245.0, 250.0]),
                ];
                // With a snow layer the snow is drawn from it; bare heights are rock.
                let ramp: &[(f64, [f64; 3])] = if map.snow().is_some() {
                    &[
                        (0.0, [196.0, 186.0, 140.0]),
                        (8.0, [120.0, 160.0, 90.0]),
                        (60.0, [96.0, 140.0, 80.0]),
                        (160.0, [120.0, 125.0, 100.0]),
                        (300.0, [125.0, 118.0, 110.0]),
                        (700.0, [150.0, 148.0, 146.0]),
                    ]
                } else {
                    &ramp
                };
                let i = ramp
                    .iter()
                    .rposition(|(at, _)| height >= *at)
                    .unwrap_or(0)
                    .min(ramp.len() - 2);
                let t = ((height - ramp[i].0) / (ramp[i + 1].0 - ramp[i].0)).clamp(0.0, 1.0);
                [0, 1, 2].map(|c| ramp[i].1[c] + (ramp[i + 1].1[c] - ramp[i].1[c]) * t)
            };
            // Light from the north-west; exaggerated so terraces read at this scale.
            let gx = (z(x + stride, y) - z(x.saturating_sub(stride), y)) / (2.0 * metres_per_px);
            let gy = (z(x, y + stride) - z(x, y.saturating_sub(stride))) / (2.0 * metres_per_px);
            // Glacier ice and lying snow from the map's snow layer.
            let base = match map.snow() {
                Some(snow) => {
                    let (sw, _) = map.info().snow_dims();
                    let s = OVERVIEW_TO_SNOW;
                    let at = (((y * s).min(map.info().snow_dims().1 - 1) * sw + (x * s).min(sw - 1)) * 2) as usize;
                    let (ice, lying) = (snow[at] as f64 / 255.0, snow[at + 1] as f64 / 255.0);
                    let base = [0, 1, 2].map(|c| base[c] + ([236.0, 240.0, 246.0][c] - base[c]) * lying);
                    [0, 1, 2].map(|c| base[c] + ([170.0, 215.0, 240.0][c] - base[c]) * ice)
                }
                None => base,
            };
            let shade = if height <= 0.0 {
                1.0
            } else {
                (1.0 + 2.5 * (gy - gx)).clamp(0.45, 1.5)
            };
            // Image rows run top to bottom; the map's +Y is north.
            let at = (((h - 1 - py) * w + px) * 3) as usize;
            for c in 0..3 {
                rgb[at + c] = (base[c] * shade).clamp(0.0, 255.0) as u8;
            }
        }
    }

    let mut dot = |pos: mc_core::FxVec2, radius: i64, colour: [u8; 3]| {
        let cx = (pos.x.to_f64() / metres_per_px) as i64;
        let cy = h as i64 - 1 - (pos.y.to_f64() / metres_per_px) as i64;
        for y in (cy - radius).max(0)..=(cy + radius).min(h as i64 - 1) {
            for x in (cx - radius).max(0)..=(cx + radius).min(w as i64 - 1) {
                let at = ((y * w as i64 + x) * 3) as usize;
                rgb[at..at + 3].copy_from_slice(&colour);
            }
        }
    };
    for Prop { kind, pos, .. } in map.props() {
        if kind.is_tree() {
            dot(*pos, 0, [30, 80, 40]);
        } else if kind.is_building() {
            dot(*pos, 0, [70, 70, 80]);
        }
    }
    // Ore fields: every pixel inside, in the materials red-orange.
    for region in map.ore_regions() {
        let (lo, hi) = region.bounds();
        let (x0, x1) = ((lo.x.to_f64() / metres_per_px) as i64, (hi.x.to_f64() / metres_per_px) as i64);
        let (y0, y1) = ((lo.y.to_f64() / metres_per_px) as i64, (hi.y.to_f64() / metres_per_px) as i64);
        for py in y0..=y1 {
            for px in x0..=x1 {
                let at = mc_core::FxVec2::new(
                    mc_core::Fx((((px as f64 + 0.5) * metres_per_px) * 65536.0) as i64),
                    mc_core::Fx((((py as f64 + 0.5) * metres_per_px) * 65536.0) as i64),
                );
                if region.contains(at) {
                    dot(at, 0, [255, 96, 40]);
                }
            }
        }
    }
    for w in map.wrecks() {
        dot(w.pos, 1, [255, 220, 40]);
    }
    for s in map.start_positions() {
        dot(*s, 3, [230, 30, 30]);
    }

    let mut out = std::io::BufWriter::new(std::fs::File::create(path)?);
    write!(out, "P6\n{w} {h}\n255\n")?;
    out.write_all(&rgb)?;
    out.flush()
}
