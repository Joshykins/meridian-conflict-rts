//! Renderers and playtests read private snapshots, leaving the authored folder
//! free of native streaming handles during a transactional save.
use super::*;
use std::sync::atomic::{AtomicU64, Ordering};
struct Folder(PathBuf);
impl Drop for Folder {
    fn drop(&mut self) {
        if self.0.exists() {
            if let Err(error) = std::fs::remove_dir_all(&self.0) {
                log::warn!(
                    "Could not remove private editor snapshot {}: {error}",
                    self.0.display()
                );
            }
        }
    }
}
static SERIAL: AtomicU64 = AtomicU64::new(0);
pub(super) fn map(doc: &Document, purpose: &str) -> Result<Arc<MapFile>, String> {
    let folder = std::env::temp_dir().join(format!(
        "meridian-editor-{purpose}-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    let owner = Folder(folder.clone());
    doc.export(&folder)?;
    Ok(Arc::new(
        MapFile::open(folder.join("terrain.mcmap"))
            .map_err(|e| e.to_string())?
            .with_lifetime(owner),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshots_are_unique_and_live_until_the_last_reader_finishes() {
        let source =
            std::env::temp_dir().join(format!("editor-snapshot-owner-test-{}", std::process::id()));
        std::fs::create_dir_all(&source).unwrap();
        let path = source.join("terrain.mcmap");
        mc_map::bake::bake(&mc_map::bake::BakeParams::square("Snapshot", 1, 5), &path).unwrap();
        let mut doc = Document::open(&path).unwrap();
        let first = map(&doc, "lifetime-test").unwrap();
        let second = map(&doc, "lifetime-test").unwrap();
        let first_folder = first.path().parent().unwrap().to_owned();
        let second_folder = second.path().parent().unwrap().to_owned();
        assert_ne!(first_folder, second_folder);
        assert_ne!(first_folder, source);
        // Authoring saves can replace their folder while snapshots keep streaming.
        doc.save(&source).unwrap();
        doc.save(&source).unwrap();
        let reader = first.clone();
        drop(first);
        assert!(first_folder.exists());
        reader.read_tile(0, 0).unwrap();
        drop(reader);
        assert!(!first_folder.exists());
        drop(second);
        assert!(!second_folder.exists());
        assert!(source.join("terrain.mcmap").exists());
        std::fs::remove_dir_all(source).unwrap();
    }
}
