//! `--shot-server DIR`: a headless process that keeps one warm renderer and
//! answers `--unit-shot` requests, so a shot costs its views and not the
//! renderer's start-up (shaders, models, terrain, ground cover: ~10 s).
//!
//! `scripts/shot.sh` drives it through files, which both sides of WSL can see:
//! it writes `NAME.req` (one argument per line), the server writes what it said
//! to `NAME.done` (first line `ok` or `error`). The server rewrites `alive` every
//! second and leaves after `IDLE` without a request, or on a `--quit` request.
//! `--reload` re-reads data/ and rebuilds the renderer, after a data edit;
//! `--shaders` recompiles the WGSL in `crates/mc-render/shaders` and rebuilds the
//! renderer with it, after a shader edit (`mc_render::shader_reload`). After a
//! model edit, `--calls-for KEY --calls FILE` writes the mesh calls the renderer
//! made for that unit; `scripts/shot.sh` has a freshly built `mc-models` program
//! answer them, and `--models ANSWER --calls FILE` swaps its meshes in
//! (`mc_models::remote`).
//! None of the three needs a build of the game.

use crate::range::Scenario;
use crate::setup::Options;
use crate::unit_shot::{self, Spec, Studio};
use mc_data::Blueprints;
use mc_jobs::Pool;
use mc_map::MapFile;
use mc_render::models::remote;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// A server nobody has asked anything for this long goes away.
const IDLE: Duration = Duration::from_secs(30 * 60);

/// The shaders' WGSL, from the repository root the server runs in.
const SHADERS: &str = "crates/mc-render/shaders";

pub(crate) fn serve(
    dir: &Path,
    base: &Options,
    map: Arc<MapFile>,
    data_dir: &Path,
    pool: Arc<Pool>,
) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    // Rebuilds after a shader, data or model edit take what the last build computed.
    mc_render::keep::keep_between_builds();
    remote::keep_meshes();
    let mut server = Server {
        base: base.clone(),
        map,
        data_dir: data_dir.to_owned(),
        pool,
        blueprints: load(data_dir)?,
        studio: None,
    };
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
        answer(&req, server.handle(&args));
    }
    // The beat stops with the process; the file goes first so nobody waits on it.
    std::fs::remove_file(&alive).ok();
    Ok(())
}

/// What the server holds between requests.
struct Server {
    base: Options,
    map: Arc<MapFile>,
    data_dir: PathBuf,
    pool: Arc<Pool>,
    blueprints: Arc<Blueprints>,
    studio: Option<Studio>,
}

impl Server {
    /// One request; says what it did.
    fn handle(&mut self, args: &[String]) -> Result<String, String> {
        let mut said = String::new();
        let value = |flag: &str| {
            args.iter()
                .position(|a| a == flag)
                .and_then(|i| args.get(i + 1))
                .cloned()
        };
        if args.iter().any(|a| a == "--reload") {
            self.blueprints = load(&self.data_dir)?;
            self.studio = None;
            said += "data/ re-read\n";
        }
        if args.iter().any(|a| a == "--shaders") {
            let t = Instant::now();
            let n = mc_render::shader_reload::reload_from(Path::new(SHADERS))
                .map_err(|e| e.to_string())?;
            self.studio = None;
            said += &format!(
                "{n} shaders rebuilt in {:.1} s\n",
                t.elapsed().as_secs_f32()
            );
        }
        let read = |path: &str| std::fs::read(path).map_err(|e| format!("{path}: {e}"));
        let calls_file = value("--calls").ok_or("--models and --calls-for need --calls FILE");
        // Any number of `--models ANSWER --calls FILE` pairs, one per unit, so a
        // batch of variants swaps in with one renderer rebuild.
        let all = |flag: &str| -> Vec<String> {
            args.windows(2)
                .filter(|w| w[0] == flag)
                .map(|w| w[1].clone())
                .collect()
        };
        let answers = all("--models");
        if !answers.is_empty() {
            let calls = all("--calls");
            if calls.len() != answers.len() {
                return Err("each --models ANSWER needs its own --calls FILE".into());
            }
            let mut n = 0;
            for (answer, calls) in answers.iter().zip(&calls) {
                let calls = remote::decode_request(&read(calls)?).map_err(|e| e.to_string())?;
                let models = remote::decode_response(&read(answer)?).map_err(|e| e.to_string())?;
                n += remote::replace(&calls, models);
            }
            self.studio = None;
            said += &format!("{n} rebuilt meshes swapped in\n");
        }
        if let Some(key) = value("--calls-for") {
            return self
                .write_calls(&key, Path::new(&calls_file?))
                .map(|n| said + &format!("{n} mesh calls\n"));
        }
        let (opts, ticks, spec) = parse(&self.base, args)?;
        if self.blueprints.id_of(&opts.subject).is_none() {
            return Err(format!("{:?} is not a unit", opts.subject));
        }
        said += &self.studio(spec.width, spec.height)?;
        let studio = self.studio.as_mut().ok_or("no renderer")?;
        said += &studio.shoot(&opts, ticks, &spec)?;
        Ok(said)
    }

    /// Builds the renderer if there is none; says how long it took.
    fn studio(&mut self, width: u32, height: u32) -> Result<String, String> {
        if self.studio.is_some() {
            return Ok(String::new());
        }
        let t = Instant::now();
        self.studio = Some(Studio::new(
            self.map.clone(),
            self.blueprints.clone(),
            self.pool.clone(),
            width,
            height,
        )?);
        Ok(format!(
            "renderer built in {:.1} s\n",
            t.elapsed().as_secs_f32()
        ))
    }

    /// Writes to `path` the mesh calls the renderer made for unit `key`'s model,
    /// for `mc-models` to build again with edited model code.
    fn write_calls(&mut self, key: &str, path: &Path) -> Result<usize, String> {
        let id = self
            .blueprints
            .id_of(key)
            .ok_or_else(|| format!("{key:?} is not a unit"))?;
        // The renderer records its calls as it builds.
        self.studio(800, 600)?;
        let mesh = &self
            .blueprints
            .unit(self.blueprints.base_of(id))
            .visual
            .mesh;
        let calls = remote::kept_calls(mesh);
        let bytes = remote::encode_request(&calls).map_err(|e| e.to_string())?;
        std::fs::write(path, bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(calls.len())
    }
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
            "--reload" | "--shaders" => {}
            "--models" | "--calls-for" | "--calls" => {
                value(arg)?;
            }
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
