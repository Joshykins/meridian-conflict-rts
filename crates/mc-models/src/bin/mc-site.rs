//! Writes what the website shows of each unit: `mc-site REQUEST OUT_DIR [--portrait N]`
//! reads the units asked for from the file REQUEST (`mc_models::site::parse_request`)
//! and, for each, writes its mesh to `OUT_DIR/NAME.mesh` and its portrait, N pixels
//! square (default 512) of raw RGBA8, to `OUT_DIR/NAME.rgba`. A unit whose mesh is
//! already there, byte for byte, keeps the portrait beside it: taking portraits is
//! nearly all of the work, so after a model edit only the units it changed cost
//! anything. Prints `NAME<tab>TRIANGLES<tab>new|kept` per unit. The site's build runs it
//! (`site/scripts/models.mjs`), so its pages follow the models.

use std::path::Path;
use std::process::ExitCode;

use mc_models::site;

fn run() -> Result<(), String> {
    let usage = || "usage: mc-site REQUEST OUT_DIR [--portrait N]".to_string();
    let mut args = std::env::args().skip(1);
    let (Some(request), Some(out_dir)) = (args.next(), args.next()) else {
        return Err(usage());
    };
    let portrait = match (args.next().as_deref(), args.next(), args.next()) {
        (None, None, None) => 512,
        (Some("--portrait"), Some(n), None) => n
            .parse::<usize>()
            .ok()
            .filter(|n| (16..=2048).contains(n))
            .ok_or("--portrait takes a size from 16 to 2048")?,
        _ => return Err(usage()),
    };
    let text = std::fs::read_to_string(&request).map_err(|e| format!("{request}: {e}"))?;
    let asks = site::parse_request(&text).map_err(|e| format!("{request}: {e}"))?;
    let out_dir = Path::new(&out_dir);
    std::fs::create_dir_all(out_dir).map_err(|e| format!("{}: {e}", out_dir.display()))?;

    // A share of the units on every core: a unit's model and portrait are its own work.
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let next = std::sync::atomic::AtomicUsize::new(0);
    let failed = std::thread::scope(|s| {
        let jobs: Vec<_> = (0..threads.min(asks.len().max(1)))
            .map(|_| {
                s.spawn(|| -> Result<(), String> {
                    loop {
                        let at = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some((name, ask)) = asks.get(at) else {
                            return Ok(());
                        };
                        let rest = site::rest(ask).map_err(|e| format!("{name}: {e}"))?;
                        let write = |ext: &str, bytes: &[u8]| {
                            let path = out_dir.join(format!("{name}.{ext}"));
                            std::fs::write(&path, bytes)
                                .map_err(|e| format!("{}: {e}", path.display()))
                        };
                        let kept = std::fs::read(out_dir.join(format!("{name}.mesh")))
                            .is_ok_and(|old| old == rest.file)
                            && std::fs::metadata(out_dir.join(format!("{name}.rgba")))
                                .is_ok_and(|m| m.len() == (portrait * portrait * 4) as u64);
                        if !kept {
                            // The portrait first: a mesh on disk says its portrait is too.
                            write("rgba", &rest.portrait(&ask.paint, portrait))?;
                            write("mesh", &rest.file)?;
                        }
                        let state = if kept { "kept" } else { "new" };
                        println!("{name}\t{}\t{state}", rest.triangles);
                    }
                })
            })
            .collect();
        jobs.into_iter()
            .filter_map(|j| {
                j.join()
                    .unwrap_or_else(|_| Err("a unit's build panicked".to_owned()))
                    .err()
            })
            .next()
    });
    failed.map_or(Ok(()), Err)
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("mc-site: {e}");
            ExitCode::FAILURE
        }
    }
}
