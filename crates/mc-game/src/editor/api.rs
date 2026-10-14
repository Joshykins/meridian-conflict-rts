//! Bounded file-mailbox protocol. Requests are published by rename; responses
//! are published the same way. Only this editor process services its directory.
use super::{Command, Editor, Event};
use mc_render::Renderer;
use serde::Deserialize;
use serde_json::{json, Value};
use std::io::Read;
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
enum Request {
    State,
    Edit { actions: Vec<Command> },
    Undo,
    Redo,
    Save { name: Option<String> },
    Camera { at: [f32; 2], zoom: f32, yaw: f32 },
    Shot,
    Test,
}

pub(super) struct Api {
    dir: PathBuf,
    pub(super) enabled: bool,
    polled: Instant,
    shot: Option<String>,
}

impl Api {
    pub fn new() -> Self {
        let dir = crate::settings::config_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("editor")
            .join(std::process::id().to_string());
        Self {
            dir,
            enabled: true,
            polled: Instant::now() - Duration::from_secs(1),
            shot: None,
        }
    }
    fn answer(&self, id: &str, value: Value) {
        let result = (|| -> std::io::Result<()> {
            let tmp = self.dir.join(format!("{id}.reply.tmp"));
            std::fs::write(&tmp, value.to_string())?;
            std::fs::rename(tmp, self.dir.join(format!("{id}.reply.json")))
        })();
        if let Err(e) = result {
            log::error!("editor reply {id}: {e}");
        }
    }
    fn poll(&mut self) -> Option<(String, Result<Request, String>)> {
        if !self.enabled
            || self.polled.elapsed() < Duration::from_millis(100)
            || self.shot.is_some()
        {
            return None;
        }
        self.polled = Instant::now();
        if let Err(e) = std::fs::create_dir_all(&self.dir) {
            log::error!("editor mailbox: {e}");
            return None;
        }
        if let Err(e) = std::fs::write(self.dir.join("alive"), std::process::id().to_string()) {
            log::error!("editor heartbeat: {e}");
        }
        let mut requests: Vec<_> = std::fs::read_dir(&self.dir)
            .ok()?
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .and_then(|n| n.strip_suffix(".req.json"))
                    .is_some_and(|id| {
                        !id.is_empty()
                            && id.len() <= 128
                            && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
                    })
            })
            // Bound per-frame tool work; remaining requests are serviced on later frames.
            .take(512)
            .collect();
        requests.sort();
        let path = requests.first()?;
        let id = path
            .file_name()?
            .to_str()?
            .strip_suffix(".req.json")?
            .to_owned();
        if !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
            return None;
        }
        let result = (|| {
            let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
            let mut bytes = Vec::new();
            file.take(262_145)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            if bytes.len() > 262_144 {
                return Err("Request exceeds 256 KiB".into());
            }
            serde_json::from_slice(&bytes).map_err(|e| e.to_string())
        })();
        if let Err(e) = std::fs::remove_file(path) {
            self.answer(&id, json!({"ok":false,"error":e.to_string()}));
            return None;
        }
        Some((id, result))
    }
}
impl Drop for Api {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(self.dir.join("alive"));
    }
}

impl Editor {
    pub(super) fn api_frame(&mut self, renderer: &mut Renderer) -> Option<Event> {
        if let Some(id) = self.api.shot.clone() {
            if let Some(result) = renderer.take_capture() {
                self.api.shot = None;
                let response = result.map_err(str::to_owned).and_then(|shot| {
                    let path = self.api.dir.join(format!("{id}.png"));
                    crate::headless::write_png(&path, shot.width, shot.height, &shot.rgba)?;
                    Ok(json!({"ok":true,"shot":path,"size":[shot.width,shot.height]}))
                });
                self.api.answer(
                    &id,
                    response.unwrap_or_else(|e| json!({"ok":false,"error":e})),
                );
            }
        }
        let (id, request) = self.api.poll()?;
        let mut event = None;
        let result=request.and_then(|request| {
            match request {
                Request::State => return Ok(json!({"ok":true,"map":self.doc.map.info.name,"source":self.doc.source,
                    "dirty":self.doc.dirty,"placed":self.doc.placed,"camera":{"at":self.camera.focus.to_array(),"zoom":self.camera.distance,"yaw":self.camera.yaw.to_degrees()}})),
                Request::Edit {actions} => {
                    if let Some(rect)=self.doc.apply(&actions,&self.blueprints)? {renderer.edit_terrain(&self.doc.map,rect).map_err(|e|e.to_string())?;}
                    if actions.iter().any(|a|!matches!(a,Command::Terrain{..})){self.refresh_units()?;}
                }
                Request::Undo|Request::Redo => {
                    let rect=if matches!(request,Request::Undo){self.doc.undo()?}else{self.doc.redo()?};
                    if let Some(rect)=rect {renderer.edit_terrain(&self.doc.map,rect).map_err(|e|e.to_string())?;}
                    self.refresh_units()?;
                }
                Request::Save {name}=> {if let Some(name)=name {self.save_as(name)?;}else{self.save()?;}}
                Request::Camera {at,zoom,yaw}=>{
                    if at.iter().chain([&zoom,&yaw]).any(|v|!v.is_finite()) || zoom<25.0 || zoom>300_000.0 {
                        return Err("Invalid camera coordinates".into());
                    }
                    self.camera.focus=glam::Vec2::from(at).extend(renderer.ground_height(at.into()));
                    self.camera.distance=zoom;self.camera.yaw=yaw.to_radians();self.camera.clamp_focus();
                }
                Request::Shot=>{renderer.capture_next_frame();self.api.shot=Some(id.clone());}
                Request::Test=>{event=Some(Event::Test(Box::new(self.test()?)));}
            }
            Ok(json!({"ok":true}))
        });
        if self.api.shot.as_deref() != Some(&id) {
            self.api.answer(
                &id,
                result.unwrap_or_else(|e| json!({"ok":false,"error":e})),
            );
        }
        event
    }
}
