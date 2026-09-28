//! Shaders rebuilt from their WGSL while the process runs, so a warm headless
//! renderer (`meridian --shot-server`) shows a shader edit in a second or two
//! instead of after a build of the game. [`reload_from`] compiles every shader
//! in a directory exactly as the build does (`shader_prelude`); from then on
//! each `spirv!` in the crate hands out the rebuilt SPIR-V, so the next renderer
//! built in this process uses it. The game itself never calls this.

use crate::shader_prelude::{compile, Preludes, PRELUDES};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, RwLock};

/// SPIR-V by shader name.
type Spirv = BTreeMap<String, Vec<u8>>;

/// The rebuilt shaders, once `reload_from` has run.
static REBUILT: RwLock<Option<Arc<Spirv>>> = RwLock::new(None);

/// Why the shaders could not be rebuilt; the ones built before stay in use.
#[derive(Debug)]
pub enum ShaderReloadError {
    Read(std::io::Error),
    /// Shader name and naga's message, with source lines.
    Compile(String, String),
}

impl std::fmt::Display for ShaderReloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShaderReloadError::Read(e) => write!(f, "reading shaders: {e}"),
            ShaderReloadError::Compile(name, e) => write!(f, "{name}.wgsl: {e}"),
        }
    }
}

impl std::error::Error for ShaderReloadError {}

/// Compiles every shader in `dir` (the crate's `shaders/`) and, if all of them
/// compile, makes them the ones renderers built from now on use. Returns how
/// many were compiled.
pub fn reload_from(dir: &Path) -> Result<usize, ShaderReloadError> {
    let read_file = |name: &str| std::fs::read_to_string(dir.join(format!("{name}.wgsl")));
    for name in PRELUDES {
        read_file(name).map_err(ShaderReloadError::Read)?;
    }
    let preludes = Preludes::new(&crate::gpu_consts::wgsl(), &|name| {
        read_file(name).unwrap_or_default()
    });
    let mut rebuilt = BTreeMap::new();
    for entry in std::fs::read_dir(dir).map_err(ShaderReloadError::Read)? {
        let path = entry.map_err(ShaderReloadError::Read)?.path();
        let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if path.extension().is_none_or(|e| e != "wgsl") || PRELUDES.contains(&name) {
            continue;
        }
        let body = std::fs::read_to_string(&path).map_err(ShaderReloadError::Read)?;
        let (source, offset) = preludes.assemble(&body);
        let (_, words) = compile(&source).map_err(|e| {
            ShaderReloadError::Compile(
                name.to_owned(),
                format!("(its line 1 is line {offset} below)\n{e}"),
            )
        })?;
        rebuilt.insert(
            name.to_owned(),
            words.iter().flat_map(|w| w.to_le_bytes()).collect(),
        );
    }
    let count = rebuilt.len();
    *REBUILT.write().unwrap_or_else(|p| p.into_inner()) = Some(Arc::new(rebuilt));
    Ok(count)
}

/// Shader `name`'s SPIR-V: rebuilt by `reload_from`, else `built` into the game.
pub(crate) fn spirv_of(name: &str, built: &'static [u8]) -> Cow<'static, [u8]> {
    let rebuilt = REBUILT.read().unwrap_or_else(|p| p.into_inner()).clone();
    match rebuilt.as_ref().and_then(|r| r.get(name)) {
        Some(words) => Cow::Owned(words.clone()),
        None => Cow::Borrowed(built),
    }
}

/// A shader's SPIR-V by file stem, as `spirv` hands it out.
macro_rules! spirv {
    ($name:literal) => {
        &*crate::shader_reload::spirv_of(
            $name,
            include_bytes!(concat!(env!("OUT_DIR"), "/", $name, ".spv")),
        )
    };
}
pub(crate) use spirv;
