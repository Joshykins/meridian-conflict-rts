//! The live desk: a tool outside the game (`scripts/live.sh`, an agent working with
//! the player) works the test range while it is being played. It drops a request in
//! the mailbox folder (`live` beside the settings) and the game answers it there:
//! it lays structures and prefabs (finished, as sites, or built by engineers in their
//! build order), says whether each one fits, reads the range back, keeps a prefab in
//! the player's library, moves the camera and takes a screenshot. `game_live.rs` does
//! the work; this is the mailbox and what is said through it.
//!
//! Only the test range listens, and everything it changes goes through the debug
//! commands, which only a match with cheats obeys. Requests come from outside the
//! process: each is read with a size limit and held to the limits below, and one
//! that breaks them is refused with why, never cut short.

use crate::prefabs::Member;
use mc_data::{BlueprintId, Blueprints};
use serde::Deserialize;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Biggest request file read; a bigger one is refused unread.
const MAX_REQUEST_BYTES: u64 = 256 * 1024;
/// Most structures one `lay` puts down (a whole base, not one prefab).
pub const MAX_LAID: usize = 512;
/// Most builders one `lay` spawns to build it: a crew, not an army.
pub const MAX_BUILDERS: u16 = 25;
/// How often the folder is looked in.
const POLL: Duration = Duration::from_millis(100);
/// How often the heartbeat file is rewritten: `live.sh` reads its age.
const BEAT: Duration = Duration::from_secs(1);
const REQUEST: &str = ".req.json";

/// One request. `op` names it; the rest are its fields.
#[derive(Deserialize, Debug, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    /// The map, the camera, the pointer, every structure (and with `units`, every
    /// mobile unit), the selection and the player's prefabs.
    State {
        #[serde(default)]
        units: bool,
    },
    /// Structures put down: a prefab of the library by `name`, or `structures` given
    /// as a prefab's members.
    Lay(Lay),
    /// The structures selected (or these `ids`) as a prefab, as Save Prefab makes it.
    Capture {
        #[serde(default)]
        ids: Vec<u32>,
    },
    /// Puts a prefab in the player's library (in place of one with its name) and saves it.
    Keep { prefab: KeptPrefab },
    /// Takes units off the map, with no wreck: these `ids`, and whatever stands in
    /// `area` (`[x0, y0, x1, y1]` metres).
    Remove {
        #[serde(default)]
        ids: Vec<u32>,
        #[serde(default)]
        area: Option<[f32; 4]>,
    },
    /// Empties the range: units, wrecks, shots and scorch marks.
    Clear,
    /// Looks at `at`, from `zoom` metres, turned `yaw` degrees.
    Camera {
        #[serde(default)]
        at: Option<At>,
        #[serde(default)]
        zoom: Option<f32>,
        #[serde(default)]
        yaw: Option<f32>,
    },
    /// A screenshot of the next frame, saved in the mailbox as `<id>.png`.
    Shot,
}

/// What `lay` puts down, where, and how.
#[derive(Deserialize, Debug, Default, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Lay {
    /// A prefab of the player's library.
    #[serde(default)]
    pub name: Option<String>,
    /// Or these: metres east and north of `at`, quarter turns from build heading.
    #[serde(default)]
    pub structures: Option<Vec<Member>>,
    #[serde(default)]
    pub at: At,
    /// `Q` or `Qm`: quarter turns anticlockwise, mirrored first (`Turn::parse`).
    #[serde(default)]
    pub turn: Option<String>,
    /// Whose they are; the range's blue side when not given.
    #[serde(default)]
    pub owner: u8,
    #[serde(default)]
    pub how: How,
    /// `how: "build"`: the engineer that builds it; the lowest tech one that can
    /// build all of it when not given.
    #[serde(default)]
    pub builder: Option<String>,
    /// `how: "build"`: how many of them.
    #[serde(default = "one")]
    pub builders: u16,
    /// Only say where each would go and whether it fits.
    #[serde(default)]
    pub dry: bool,
    /// Turns free building on or off for the owner first.
    #[serde(default)]
    pub free_build: Option<bool>,
}

fn one() -> u16 {
    1
}

/// A prefab given whole, as `prefabs.ron` holds one.
#[derive(Deserialize, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct KeptPrefab {
    pub name: String,
    pub members: Vec<Member>,
}

/// Where: a point in metres, or a place the game knows.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq)]
#[serde(untagged)]
pub enum At {
    Point([f32; 2]),
    Spot(Spot),
}

impl Default for At {
    fn default() -> At {
        At::Spot(Spot::Camera)
    }
}

#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Spot {
    /// The ground the camera looks at.
    Camera,
    /// The ground under the pointer.
    Cursor,
    /// The range's pad.
    Pad,
}

/// How what is laid comes to stand.
#[derive(Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum How {
    /// Finished.
    #[default]
    Done,
    /// Construction sites, nothing built.
    Sites,
    /// Built by engineers spawned beside it, in the order given, as a player's
    /// engineers would build a prefab placed with them.
    Build,
}

/// The engineer that builds `wanted` when none is named: the mobile builders that can
/// build all of it, lowest tech first, then cheapest. `None` if none can.
pub fn pick_builder(blueprints: &Blueprints, wanted: &[BlueprintId]) -> Option<BlueprintId> {
    let can = |id: BlueprintId| {
        let bp = blueprints.unit(id);
        bp.is_mobile()
            && bp
                .builder
                .as_ref()
                .is_some_and(|b| wanted.iter().all(|w| b.builds.contains(w)))
    };
    (0..blueprints.units.len())
        .map(|i| BlueprintId(i as u16))
        .filter(|&id| can(id))
        .min_by_key(|&id| {
            let bp = blueprints.unit(id);
            (bp.tech, bp.cost_materials, id.0)
        })
}

/// A request's name, from its file's: letters, digits, `-` and `_`, at most 64.
fn request_id(file: &str) -> Option<&str> {
    let id = file.strip_suffix(REQUEST)?;
    let fine = !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_');
    fine.then_some(id)
}

/// Reads a request, refusing a file too big to be one.
fn read_request(path: &Path) -> Result<Request, String> {
    let size = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
    if size > MAX_REQUEST_BYTES {
        return Err(format!(
            "the request is {size} bytes; the most is {MAX_REQUEST_BYTES}"
        ));
    }
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|e| format!("not a request: {e}"))
}

/// The folder requests are dropped in and answered in.
pub struct Mailbox {
    dir: PathBuf,
    polled: Instant,
    beat: Option<Instant>,
}

/// Writes answers in the mailbox; sent to the thread that saves a screenshot.
pub struct Outbox {
    dir: PathBuf,
}

impl Outbox {
    /// Answers request `id`.
    pub fn answer(&self, id: &str, reply: &Value) {
        self.write(&format!("{id}.reply.json"), reply);
    }

    /// Written aside and moved over, so the reader never sees half a file.
    fn write(&self, name: &str, what: &Value) {
        let path = self.dir.join(name);
        let aside = path.with_extension("new");
        let text = serde_json::to_string_pretty(what).unwrap_or_default();
        if let Err(e) = std::fs::write(&aside, text).and_then(|()| std::fs::rename(&aside, &path)) {
            log::warn!("live desk: {}: {e}", path.display());
        }
    }
}

impl Mailbox {
    /// `live` beside the settings, made if it is not there. `None` with no settings
    /// folder, or one that cannot be written (said once in the log).
    pub fn open() -> Option<Mailbox> {
        let dir = crate::settings::config_dir()?.join("live");
        if let Err(e) = std::fs::create_dir_all(&dir) {
            log::warn!("live desk off: {}: {e}", dir.display());
            return None;
        }
        log::info!("live desk: requests in {}", dir.display());
        Some(Mailbox {
            dir,
            polled: Instant::now(),
            beat: None,
        })
    }

    /// The heartbeat, rewritten every second with `what` (the map and the tick).
    pub fn beat(&mut self, now: Instant, what: &Value) {
        if self.beat.is_some_and(|b| now - b < BEAT) {
            return;
        }
        self.beat = Some(now);
        self.outbox().write("alive", what);
    }

    /// The oldest request waiting, taken out of the folder, at most every `POLL`.
    pub fn next(&mut self, now: Instant) -> Option<(String, Result<Request, String>)> {
        if now - self.polled < POLL {
            return None;
        }
        self.polled = now;
        let mut waiting: Vec<String> = std::fs::read_dir(&self.dir)
            .ok()?
            .filter_map(|e| e.ok()?.file_name().into_string().ok())
            .filter(|name| name.ends_with(REQUEST))
            .collect();
        waiting.sort();
        let file = waiting.into_iter().next()?;
        let path = self.dir.join(&file);
        let read = read_request(&path);
        if let Err(e) = std::fs::remove_file(&path) {
            log::warn!("live desk: {}: {e}", path.display());
        }
        match request_id(&file) {
            Some(id) => Some((id.to_owned(), read)),
            None => {
                log::warn!("live desk: {file:?} ignored: a request is named <letters, digits, - and _>.req.json");
                None
            }
        }
    }

    /// Where request `id`'s screenshot goes.
    pub fn shot_path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.png"))
    }

    /// Answers request `id`.
    pub fn answer(&self, id: &str, reply: &Value) {
        self.outbox().answer(id, reply);
    }

    pub fn outbox(&self) -> Outbox {
        Outbox {
            dir: self.dir.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Result<Request, serde_json::Error> {
        serde_json::from_str(text)
    }

    #[test]
    fn requests_read_with_their_defaults() {
        assert_eq!(parse(r#"{"op":"clear"}"#).unwrap(), Request::Clear);
        assert_eq!(
            parse(r#"{"op":"state"}"#).unwrap(),
            Request::State { units: false }
        );
        let Request::Lay(lay) = parse(
            r#"{"op":"lay","structures":[{"key":"a","x":12,"y":-24,"turn":1}],"at":[100.5,200],"how":"build"}"#,
        )
        .unwrap() else {
            panic!("not a lay");
        };
        assert_eq!(lay.at, At::Point([100.5, 200.0]));
        assert_eq!(lay.how, How::Build);
        assert_eq!(lay.builders, 1);
        assert_eq!(lay.structures.unwrap()[0].turn, 1);
        let Request::Lay(lay) = parse(r#"{"op":"lay","name":"Block","at":"pad"}"#).unwrap() else {
            panic!("not a lay");
        };
        assert_eq!(lay.at, At::Spot(Spot::Pad));
        assert_eq!(lay.how, How::Done);
    }

    #[test]
    fn a_misspelt_field_or_op_is_refused_not_ignored() {
        assert!(parse(r#"{"op":"lay","name":"x","hwo":"build"}"#).is_err());
        assert!(parse(r#"{"op":"state","unit":true}"#).is_err());
        assert!(parse(r#"{"op":"explode"}"#).is_err());
        assert!(parse(r#"{"op":"lay","at":"moon"}"#).is_err());
    }

    #[test]
    fn request_names_are_plain() {
        assert_eq!(request_id("1760-abc_d.req.json"), Some("1760-abc_d"));
        assert_eq!(request_id("../x.req.json"), None);
        assert_eq!(request_id(".req.json"), None);
        assert_eq!(request_id("x.reply.json"), None);
    }

    #[test]
    fn the_builder_picked_can_build_everything_asked() {
        let b =
            Blueprints::load(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data"))
                .unwrap();
        let structures: Vec<BlueprintId> = (0..b.units.len())
            .map(|i| BlueprintId(i as u16))
            .filter(|&id| b.unit(id).is_structure())
            .collect();
        for &s in &structures {
            if let Some(e) = pick_builder(&b, &[s]) {
                let bp = b.unit(e);
                assert!(bp.is_mobile());
                assert!(bp.builder.as_ref().unwrap().builds.contains(&s));
            }
        }
        // Something builds the faction's first power plant, and it is no titan.
        let t1 = structures
            .iter()
            .copied()
            .find(|&s| b.unit(s).tech == 1 && pick_builder(&b, &[s]).is_some())
            .expect("some T1 structure is buildable");
        assert!(b.unit(pick_builder(&b, &[t1]).unwrap()).tech <= 1);
    }
}
