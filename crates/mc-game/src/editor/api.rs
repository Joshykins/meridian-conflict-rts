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
    Catalogue {
        query: String,
    },
    ChooseUnit {
        blueprint: String,
    },
    Edit {
        actions: Vec<Command>,
    },
    Undo,
    Redo,
    Save {
        name: Option<String>,
    },
    Camera {
        at: [f32; 2],
        zoom: f32,
        yaw: f32,
        height: Option<f32>,
        pitch: Option<f32>,
    },
    Shot,
    Test,
    Close {
        #[serde(default)]
        discard: bool,
    },
    Reload {
        resources: super::ReloadRequest,
    },
    ModelCalls {
        key: String,
    },
}

pub(super) struct Api {
    dir: PathBuf,
    pub(super) enabled: bool,
    pub(super) headless: bool,
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
            headless: false,
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
            let capture = if self.api.headless {
                renderer.read_pixels().map(|rgba| {
                    Ok(mc_render::Shot {
                        width: self.camera.viewport.x as u32,
                        height: self.camera.viewport.y as u32,
                        rgba,
                    })
                })
            } else {
                renderer.take_capture()
            };
            if let Some(result) = capture {
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
            if self.reloading() && !matches!(request,Request::State|Request::Shot|Request::Camera{..}) {return Err("Reload is running; wait for state to report completion".into());}
            match request {
                Request::Catalogue{query}=>{
                    if query.len()>128 {return Err("Search is limited to 128 bytes".into());}
                    let units=self.catalogue_entries(&query);
                    self.catalogue=Some(super::catalogue::Catalogue::with_query(query));
                    return Ok(json!({"ok":true,"units":units}));
                }
                Request::ChooseUnit{blueprint}=>self.choose_unit(&blueprint)?,
                Request::Reload{resources}=>self.request_reload(resources)?,
                Request::ModelCalls{key}=>{
                    let calls=mc_render::models::remote::kept_calls(&key);
                    if calls.is_empty(){return Err("No rendered model calls match that key".into());}
                    let path=self.api.dir.join(format!("{id}.calls.bin"));
                    let bytes=mc_render::models::remote::encode_request(&calls).map_err(|e|e.to_string())?;
                    std::fs::write(&path,bytes).map_err(|e|e.to_string())?;
                    return Ok(json!({"ok":true,"calls":path,"count":calls.len()}));
                }
                Request::State => return Ok(json!({"ok":true,"map":self.doc.map.info.name,"source":self.doc.source,
                    "dirty":self.doc.dirty,"reloading":self.reloading(),"message":self.message,"destination":self.doc.destination,"placed":self.doc.placed,"camera":{"at":self.camera.focus.to_array(),"zoom":self.camera.distance,"yaw":self.camera.yaw.to_degrees(),"pitch":self.camera.pitch_free.map(f32::to_degrees)}})),
                Request::Edit {actions} => self.apply_batch(&actions,renderer)?,
                Request::Undo|Request::Redo => self.history(matches!(request,Request::Redo),renderer)?,
                Request::Save {name}=> {if let Some(name)=name {self.save_as(name)?;}else{self.save()?;}}
                Request::Camera {at,zoom,yaw,height,pitch}=>{
                    if at.iter().chain([&zoom,&yaw]).any(|v|!v.is_finite()) || zoom<25.0 || zoom>300_000.0 {
                        return Err("Invalid camera coordinates".into());
                    }
                    match (height,pitch) {
                        (Some(z),Some(p)) if z.is_finite() && (-10_000.0..=300_000.0).contains(&z) && p.is_finite() && (-89.0..=89.0).contains(&p)=>{
                            self.navigation.enter(&mut self.camera);
                            self.camera.pitch_free=Some(p.to_radians());
                        },
                        (None,None)=>self.navigation.exit(&mut self.camera),
                        _=>return Err("Free camera requires finite height and pitch (-89 to 89 degrees)".into())
                    }
                    self.camera.focus=glam::Vec2::from(at).extend(height.unwrap_or_else(||renderer.ground_height(at.into())));
                    self.camera.distance=zoom;self.camera.yaw=yaw.to_radians();self.camera.clamp_focus();
                }
                Request::Shot=>{if !self.api.headless {renderer.capture_next_frame();} self.api.shot=Some(id.clone());}
                Request::Test=>{
                    if self.api.headless{return Err("Playtests require a windowed editor; --mission-pilot checks the route offscreen".into());}
                    event=Some(Event::Test(Box::new(self.test()?)));
                }
                Request::Close{discard}=>{
                    if self.doc.dirty && !discard {return Err("Save edits before closing, or explicitly set discard to true".into());}
                    event=Some(Event::Leave);
                }
            }
            Ok(json!({"ok":true}))
        });
        if self.api.shot.as_deref() != Some(&id) {
            let response = match result {
                Ok(value) => value,
                Err(error) => {
                    self.message = error.clone();
                    json!({"ok":false,"error":error})
                }
            };
            self.api.answer(&id, response);
        }
        event
    }
}
