//! Local campaign persistence. A checkpoint is the full deterministic world at
//! a beat boundary, including pending dialogue and one-shot trigger history.
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize)]
pub(crate) struct Checkpoint {
    version: u32,
    map: u64,
    blueprints: u64,
    program: u64,
    pub(crate) next_tick: u32,
    pub(crate) beat: u16,
    pub(crate) blob: Vec<u8>,
}

fn program_hash(definition: &mc_sim::mission::Definition) -> Result<u64, String> {
    let mut h = mc_core::StateHasher::new();
    h.write_u8s(&bincode::serialize(definition).map_err(|e| e.to_string())?);
    Ok(h.finish())
}
fn folder(map: u64, key: &str) -> Result<PathBuf, String> {
    let mut h = mc_core::StateHasher::new();
    h.write_u8s(key.as_bytes());
    let mission = h.finish();
    crate::settings::config_dir()
        .map(|p| {
            p.join("campaign")
                .join(format!("{mission:016x}-{map:016x}"))
        })
        .ok_or("Cannot locate campaign save directory".into())
}

impl Checkpoint {
    pub(crate) fn load(
        map: &mc_map::MapFile,
        blueprints: &mc_data::Blueprints,
    ) -> Result<Self, String> {
        use std::io::Read;
        let definition =
            crate::campaign::definition(map)?.ok_or("This map has no campaign mission")?;
        let path = folder(map.content_id(), &definition.key)?.join("latest.bin");
        let file = std::fs::File::open(path).map_err(|e| format!("No readable checkpoint: {e}"))?;
        let limit = mc_sim::MAX_SNAPSHOT_BYTES + 1024;
        let mut bytes = Vec::new();
        file.take(limit + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() as u64 > limit {
            return Err("Checkpoint is too large".into());
        }
        let saved: Self = mc_sim::decode_untrusted(&bytes, limit)?;
        if saved.version != 3
            || saved.map != map.content_id()
            || saved.blueprints != blueprints.content_hash()
            || saved.program != program_hash(&definition)?
            || saved.next_tick == 0
        {
            return Err(
                "Checkpoint uses a different mission, map, data version or format. Restart this mission."
                    .into(),
            );
        }
        Ok(saved)
    }
}

#[derive(Default)]
pub(crate) struct Recorder {
    boundary: Option<(u16, u32)>,
    finished: bool,
}
impl Recorder {
    pub(crate) fn record(&mut self, world: &mut mc_sim::World) -> Result<(), String> {
        // Editor/range tests never replace the player's campaign checkpoints.
        if world.state.cheats {
            return Ok(());
        }
        let Some(status) = world.mission_status() else {
            return Ok(());
        };
        if status.beat > 0 && self.boundary != Some((status.beat, status.checkpoint)) {
            let saved = Checkpoint {
                version: 3,
                map: world.map.content_id,
                blueprints: world.blueprints.content_hash(),
                program: program_hash(
                    &world
                        .state
                        .mission
                        .as_ref()
                        .ok_or("Mission vanished")?
                        .definition,
                )?,
                next_tick: world.tick_count(),
                beat: status.beat,
                blob: world.snapshot(),
            };
            if saved.next_tick > 0 {
                let dir = folder(saved.map, &status.key)?;
                std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                let bytes = bincode::serialize(&saved).map_err(|e| e.to_string())?;
                atomic_write(&dir.join(format!("beat-{}.bin", saved.beat)), &bytes)?;
                atomic_write(&dir.join("latest.bin"), &bytes)?;
                self.boundary = Some((status.beat, status.checkpoint));
            }
        }
        if status.outcome == Some(true) && !self.finished {
            let dir = folder(world.map.content_id, &status.key)?;
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            // Versioned campaign progress survives quitting, separate from live checkpoint.
            let text = format!("(version: 1, mission: {:?}, completed: true)\n", status.key);
            atomic_write(&dir.join("progress.ron"), text.as_bytes())?;
            self.finished = true;
        }
        Ok(())
    }
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let temp = path.with_extension(format!("{}.tmp", std::process::id()));
    std::fs::write(&temp, bytes).map_err(|e| e.to_string())?;
    std::fs::rename(temp, path).map_err(|e| e.to_string())
}
