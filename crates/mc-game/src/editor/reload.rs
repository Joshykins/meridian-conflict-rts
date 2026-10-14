//! Build a candidate renderer while the last working editor remains visible.
use super::*;
use crate::loading::{Job, MapSource, Order, Ready};
use serde::Deserialize;
use std::io::Read;
#[derive(Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct ReloadRequest {
    pub data: bool,
    pub shaders: bool,
    pub calls: Option<PathBuf>,
    pub models: Option<PathBuf>,
}
#[derive(Default)]
pub(super) struct Reload {
    requested: Option<ReloadRequest>,
    pending: Option<Pending>,
}
#[derive(Default)]
struct Caches {
    shaders: Option<mc_render::shader_reload::Checkpoint>,
    models: Option<mc_render::models::remote::Checkpoint>,
}
impl Caches {
    fn commit(self) {
        if let Some(shaders) = self.shaders {
            shaders.commit();
        }
        if let Some(models) = self.models {
            models.commit();
        }
    }
}
struct Pending {
    caches: Caches,
    sounds: Option<mc_data::SoundLibrary>,
    job: Job,
    blueprints: Arc<Blueprints>,
    frame: RenderFrame,
}
pub(crate) struct Reloaded {
    caches: Caches,
    pub sounds: Option<mc_data::SoundLibrary>,
    pub ready: Ready,
    blueprints: Arc<Blueprints>,
    frame: RenderFrame,
}
fn bounded(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut bytes = vec![];
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > limit {
        return Err("Reload file exceeds its size limit".into());
    }
    Ok(bytes)
}
fn resources(
    request: &ReloadRequest,
    previous: &Arc<Blueprints>,
) -> Result<(Arc<Blueprints>, Caches), String> {
    let mut caches = Caches::default();
    let dir = Blueprints::locate_data_dir().ok_or("Cannot locate data/")?;
    let blueprints = if request.data {
        Arc::new(Blueprints::load(&dir).map_err(|e| e.to_string())?)
    } else {
        previous.clone()
    };
    if request.shaders {
        caches.shaders = Some(mc_render::shader_reload::checkpoint());
        let shaders = dir
            .parent()
            .ok_or("Data directory has no parent")?
            .join("crates/mc-render/shaders");
        mc_render::shader_reload::reload_from(&shaders).map_err(|e| e.to_string())?;
    }
    match (&request.calls, &request.models) {
        (Some(calls), Some(models)) => {
            use mc_render::models::remote;
            let calls = remote::decode_request(&bounded(calls, remote::MAX_REQUEST_BYTES)?)
                .map_err(|e| e.to_string())?;
            let models = remote::decode_response(&bounded(models, remote::MAX_RESPONSE_BYTES)?)
                .map_err(|e| e.to_string())?;
            if calls.len() != models.len() || models.iter().any(Option::is_none) {
                return Err("Model reload must resolve every requested mesh".into());
            }
            caches.models = Some(remote::checkpoint(&calls));
            remote::replace(&calls, models);
        }
        (None, None) => {}
        _ => return Err("Model reload needs both calls and models files".into()),
    }
    Ok((blueprints, caches))
}
impl Editor {
    pub(crate) fn reloading(&self) -> bool {
        self.reload.requested.is_some() || self.reload.pending.is_some()
    }
    pub(crate) fn reload_requested(&self) -> bool {
        self.reload.requested.is_some()
    }
    pub(crate) fn request_reload(&mut self, request: ReloadRequest) -> Result<(), String> {
        if self.reloading() {
            return Err("A reload is already running".into());
        }
        if !request.data && !request.shaders && request.models.is_none() {
            return Err("Choose data, shaders, or model files to reload".into());
        }
        self.reload.requested = Some(request);
        self.message = "Preparing reload; the current view remains available.".into();
        Ok(())
    }
    pub(crate) fn begin_reload(&mut self, target: mc_render::Target) {
        let Some(request) = self.reload.requested.take() else {
            return;
        };
        let result = (|| {
            let (blueprints, caches) = resources(&request, &self.blueprints)?;
            let sounds = if request.data {
                let dir = Blueprints::locate_data_dir().ok_or("Cannot locate data/")?;
                let sounds = mc_data::SoundLibrary::load(&dir).map_err(|e| e.to_string())?;
                sounds.check(&blueprints).map_err(|e| e.to_string())?;
                Some(sounds)
            } else {
                None
            };
            let frame = preview::build(
                &self.doc,
                self.map.content_id(),
                blueprints.clone(),
                self.pool.clone(),
            )?;
            let map = super::snapshot::map(&self.doc, "reload")?;
            let job = Job::start(Order {
                target,
                map: MapSource::Loaded(map),
                blueprints: blueprints.clone(),
                pool: self.pool.clone(),
                colors: crate::setup::TEAM_COLORS,
                pictures: None,
            });
            Ok::<_, String>(Pending {
                caches,
                sounds,
                job,
                blueprints,
                frame,
            })
        })();
        match result {
            Ok(p) => self.reload.pending = Some(p),
            Err(e) => self.complain_reload(e),
        }
    }
    pub(crate) fn poll_reload(&mut self) -> Option<Reloaded> {
        let pending = self.reload.pending.as_ref()?;
        let Some(result) = pending.job.finished() else {
            let (step, done) = pending.job.progress();
            self.message = format!("Reloading: {step} ({:.0}%)", done * 100.0);
            return None;
        };
        let pending = self.reload.pending.take()?;
        match result {
            Ok(ready) => Some(Reloaded {
                caches: pending.caches,
                sounds: pending.sounds,
                ready,
                blueprints: pending.blueprints,
                frame: pending.frame,
            }),
            Err(e) => {
                self.complain_reload(e);
                None
            }
        }
    }
    pub(crate) fn install_reload(
        &mut self,
        mut reloaded: Reloaded,
        old: &mut mc_render::Renderer,
        settings: &mut crate::settings::Settings,
        audio: &crate::audio::Audio,
        window: bool,
    ) -> Result<(), String> {
        if window {
            old.release_window();
        }
        let result = reloaded
            .ready
            .renderer
            .attach()
            .map_err(|e| e.to_string())
            .and_then(|()| {
                settings
                    .apply_graphics(&mut reloaded.ready.renderer)
                    .map_err(|e| e.to_string())
            });
        if let Err(error) = result {
            reloaded.ready.renderer.release_window();
            if window {
                old.attach()
                    .map_err(|e| format!("Could not restore renderer after reload failure: {e}"))?;
            }
            self.complain_reload(error);
            return Ok(());
        }
        reloaded
            .ready
            .renderer
            .set_map_look(&crate::setup::map_config(&reloaded.ready.map).look());
        if let Some(sounds) = reloaded.sounds.take() {
            audio.set_library(sounds);
        }
        let replacement = self.publish_reload(reloaded);
        crate::loading::retire(std::mem::replace(old, replacement));
        Ok(())
    }
    fn publish_reload(&mut self, reloaded: Reloaded) -> mc_render::Renderer {
        reloaded.caches.commit();
        self.map = reloaded.ready.map;
        self.blueprints = reloaded.blueprints;
        self.frame = reloaded.frame;
        self.fresh = true;
        self.message = "Reload complete. Document, camera and undo history preserved.".into();
        reloaded.ready.renderer
    }
    pub(crate) fn current_blueprints(&self) -> Arc<Blueprints> {
        self.blueprints.clone()
    }
    pub(crate) fn complain_reload(&mut self, error: String) {
        self.message = format!("Reload failed: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_models_leave_document_camera_and_undo_untouched() {
        let folder =
            std::env::temp_dir().join(format!("editor-reload-test-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let path = folder.join("terrain.mcmap");
        mc_map::bake::bake(
            &mc_map::bake::BakeParams::square("Reload test", 2, 17),
            &path,
        )
        .unwrap();
        let bp = Arc::new(
            Blueprints::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")).unwrap(),
        );
        let mut editor = Editor::open(
            &path,
            bp.clone(),
            Arc::new(Pool::new(0)),
            Vec2::new(1280.0, 720.0),
        )
        .unwrap();
        editor.api.enabled = false;
        editor
            .doc
            .apply(
                &[Command::Point {
                    name: "unsaved".into(),
                    at: [500.0, 500.0],
                }],
                &bp,
            )
            .unwrap();
        editor.camera.distance = 1234.0;
        let before = editor.doc.placed.clone();
        let bad = folder.join("invalid.bin");
        std::fs::write(&bad, [255, 1, 9]).unwrap();
        editor
            .request_reload(ReloadRequest {
                calls: Some(bad.clone()),
                models: Some(bad),
                ..Default::default()
            })
            .unwrap();
        editor.begin_reload(mc_render::Target::Headless {
            width: 1280,
            height: 720,
        });
        assert!(!editor.reloading());
        assert!(editor.message.starts_with("Reload failed:"));
        assert_eq!(editor.doc.placed, before);
        assert!(editor.doc.dirty);
        assert!(editor.doc.can_undo());
        assert_eq!(editor.camera.distance, 1234.0);
        editor.doc.undo().unwrap();
        assert!(editor.doc.placed.points.is_empty());
        drop(editor);
        std::fs::remove_dir_all(folder).unwrap();
    }
}
