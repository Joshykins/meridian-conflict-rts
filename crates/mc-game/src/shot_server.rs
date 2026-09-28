//! `--shot-server DIR`: a headless process that keeps one warm renderer and
//! answers `--unit-shot` requests, so a shot costs its views and not the
//! renderer's start-up (shaders, models, terrain, ground cover: ~10 s).
//!
//! `scripts/shot.sh` drives it through files, which both sides of WSL can see:
//! it writes `NAME.req` (one argument per line), the server writes what it said
//! to `NAME.done` (first line `ok` or `error`). The server rewrites `alive` every
//! second and leaves after `IDLE` without a request, or on a `--quit` request.
//! `--reload` re-reads data/ and rebuilds the renderer, after a data edit.

use crate::range::Scenario;
use crate::setup::Options;
use crate::unit_shot::{self, Spec, Studio};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::MapFile;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// A server nobody has asked anything for this long goes away.
const IDLE: Duration = Duration::from_secs(30 * 60);

pub(crate) fn serve(
    dir: &Path,
    base: &Options,
    map: Arc<MapFile>,
    data_dir: &Path,
    pool: Arc<Pool>,
) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut blueprints = load(data_dir)?;
    let mut studio: Option<Studio> = None;
    let mut asked = Instant::now();
    // A beat of its own, so a long renderer build does not look like a dead server.
    let alive = dir.join("alive");
    std::thread::spawn({
        let alive = alive.clone();
        move || loop {
            std::fs::write(&alive, std::process::id().to_string()).ok();
            std::thread::sleep(Duration::from_secs(1));
        }
    });
    log::info!("shot server up in {}", dir.display());
    while asked.elapsed() < IDLE {
        let Some(req) = next_request(dir) else {
            std::thread::sleep(Duration::from_millis(25));
            continue;
        };
        asked = Instant::now();
        let args = std::fs::read_to_string(&req).unwrap_or_default();
        std::fs::remove_file(&req).ok();
        let args: Vec<String> = args.lines().map(str::to_owned).collect();
        if args.iter().any(|a| a == "--quit") {
            answer(&req, Ok("quitting\n".into()));
            break;
        }
        let said = (|| {
            let mut said = String::new();
            if args.iter().any(|a| a == "--reload") {
                blueprints = load(data_dir)?;
                studio = None;
                said += "data/ re-read\n";
            }
            let (opts, ticks, spec) = parse(base, &args)?;
            if blueprints.id_of(&opts.subject).is_none() {
                return Err(format!("{:?} is not a unit", opts.subject));
            }
            let studio = match &mut studio {
                Some(s) => s,
                None => {
                    let t = Instant::now();
                    let s = Studio::new(
                        map.clone(),
                        blueprints.clone(),
                        pool.clone(),
                        spec.width,
                        spec.height,
                    )?;
                    said += &format!("renderer built in {:.1} s\n", t.elapsed().as_secs_f32());
                    studio.insert(s)
                }
            };
            said += &studio.shoot(&opts, ticks, &spec)?;
            Ok(said)
        })();
        answer(&req, said);
    }
    // The beat stops with the process; the file goes first so nobody waits on it.
    std::fs::remove_file(&alive).ok();
    Ok(())
}

fn load(data_dir: &Path) -> Result<Arc<Blueprints>, String> {
    Blueprints::load(data_dir)
        .map(Arc::new)
        .map_err(|e| e.to_string())
}

/// The oldest waiting request (names sort by the time they were made).
fn next_request(dir: &Path) -> Option<PathBuf> {
    let mut reqs: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "req"))
        .collect();
    reqs.sort();
    reqs.into_iter().next()
}

/// Writes `NAME.done` whole: to a temporary name first, so the other side never
/// reads half of it.
fn answer(req: &Path, said: Result<String, String>) {
    let text = match said {
        Ok(s) => format!("ok\n{s}"),
        Err(e) => format!("error\n{e}\n"),
    };
    let tmp = req.with_extension("tmp");
    if std::fs::write(&tmp, text).is_ok() {
        std::fs::rename(&tmp, req.with_extension("done")).ok();
    }
}

/// One request's arguments: the range options and `--unit-shot`'s own.
fn parse(base: &Options, args: &[String]) -> Result<(Options, u32, Spec), String> {
    let mut opts = base.clone();
    opts.scene = crate::setup::Scene::Range;
    let (mut ticks, mut spec) = (0u32, Spec::default());
    let mut it = args.iter().filter(|a| !a.is_empty());
    while let Some(arg) = it.next() {
        let mut value = |name: &str| it.next().cloned().ok_or(format!("{name} needs a value"));
        match arg.as_str() {
            "--reload" => {}
            "--unit-shot" => opts.subject = value(arg)?,
            "--screenshot" => spec.path = value(arg)?,
            "--ticks" => ticks = value(arg)?.parse().map_err(|_| "--ticks takes a number")?,
            "--scenario" => {
                opts.scenario = Some(Scenario::parse(&value(arg)?).ok_or("unknown --scenario")?);
            }
            "--hurt" => {
                opts.hurt = value(arg)?
                    .parse::<i16>()
                    .ok()
                    .filter(|p| (0..100).contains(p))
                    .ok_or("--hurt takes a percentage under 100")?
                    * 10;
            }
            "--size" => {
                let v = value(arg)?;
                let (w, h) = v.split_once('x').ok_or("--size takes WxH")?;
                spec.width = w.parse().map_err(|_| "--size takes WxH")?;
                spec.height = h.parse().map_err(|_| "--size takes WxH")?;
            }
            a if unit_shot::flag(&mut spec, a, &mut value)? => {}
            other => return Err(format!("the shot server does not take {other}")),
        }
    }
    if spec.path.is_empty() {
        return Err("a request needs --screenshot FILE.png".into());
    }
    if spec.views.is_empty() {
        spec.views = unit_shot::parse_views(unit_shot::SHEET)?;
    }
    Ok((opts, ticks, spec))
}
