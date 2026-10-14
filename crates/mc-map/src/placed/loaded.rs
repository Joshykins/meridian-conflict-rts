//! Placement snapshots participate in map identity without altering the baked format.
use super::*;
pub(crate) struct Loaded {
    pub placed: Placed,
    pub content_id: u64,
    pub by_prop: std::collections::BTreeMap<u32, usize>,
}
impl Loaded {
    pub(crate) fn read(
        path: &Path,
        info: &MapInfo,
        prop_count: usize,
        terrain_id: u64,
    ) -> Result<Self, MapError> {
        let placed = if path.file_name().is_some_and(|n| n == "terrain.mcmap") {
            let path = path.with_file_name("placed.ron");
            match std::fs::metadata(&path) {
                Ok(_) => Placed::read(&path, info, prop_count)?,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Placed::default(),
                Err(e) => return Err(e.into()),
            }
        } else {
            Placed::default()
        };
        let content_id = if placed == Placed::default() {
            terrain_id
        } else {
            // Canonical serialization hashes values, never source whitespace,
            // padding or filesystem timestamps. All placement numbers are integers.
            let text =
                ron::ser::to_string(&placed).map_err(|e| MapError::Invalid(e.to_string()))?;
            let mut hash = mc_core::StateHasher::new();
            hash.write_u8s(b"meridian-authored-map-v1");
            hash.write_u64(terrain_id);
            hash.write_u8s(text.as_bytes());
            hash.finish()
        };
        let by_prop = placed
            .doodads
            .iter()
            .enumerate()
            .map(|(i, d)| (d.prop_index, i))
            .collect();
        Ok(Self {
            placed,
            content_id,
            by_prop,
        })
    }
}
