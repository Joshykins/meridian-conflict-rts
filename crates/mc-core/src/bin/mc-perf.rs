//! Reads `mc_core::perf` reports.
//!
//! ```text
//! mc-perf show REPORT.json            the table again
//! mc-perf diff BEFORE.json AFTER.json [TOP]   what got dearer or cheaper
//! ```

use mc_core::perf::{diff, Saved};
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("diff") if args.len() >= 3 => (|| {
            let a = Saved::load(Path::new(&args[1]))?;
            let b = Saved::load(Path::new(&args[2]))?;
            let top = args.get(3).and_then(|t| t.parse().ok()).unwrap_or(25);
            print!("{}", diff(&a, &b, top));
            Ok::<(), String>(())
        })(),
        Some("show") if args.len() >= 2 => (|| {
            let a = Saved::load(Path::new(&args[1]))?;
            println!("== {} ({} frames)", a.title, a.frames);
            for (k, v) in &a.notes {
                println!("   {k}: {v}");
            }
            let mut rows: Vec<_> = a.rows.iter().collect();
            rows.sort_by(|x, y| y.1.mean_ms.total_cmp(&x.1.mean_ms).then(x.0.cmp(y.0)));
            for (name, r) in rows {
                if r.span {
                    println!("{name:<40} {:>9.3} ms  max {:>8.3}", r.mean_ms, r.max_ms);
                } else {
                    println!("{name:<40} {:>12.1}  max {:>10}", r.mean_n, r.max_n);
                }
            }
            Ok(())
        })(),
        _ => Err(
            "usage: mc-perf show REPORT.json | mc-perf diff BEFORE.json AFTER.json [TOP]".into(),
        ),
    };
    if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
