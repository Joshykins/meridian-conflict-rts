//! Publish all map files together. A failed write leaves the prior folder intact.
use super::document::Document;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
static SERIAL: AtomicU64 = AtomicU64::new(0);

pub(super) fn export(doc: &Document, folder: &Path) -> Result<(), String> {
    doc.placed
        .validate(&doc.map.info, doc.map.props.len())
        .map_err(|e| e.to_string())?;
    let parent = folder.parent().ok_or("Map folder has no parent")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let tag = format!(
        ".mc-editor-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    );
    let staged = parent.join(format!("{tag}.new"));
    let backup = parent.join(format!("{tag}.old"));
    std::fs::create_dir(&staged).map_err(|e| e.to_string())?;
    let result = (|| {
        let source = doc
            .destination
            .as_deref()
            .filter(|p| p.is_dir())
            .unwrap_or_else(|| doc.source.parent().unwrap_or(Path::new(".")));
        if doc.source.file_name().is_some_and(|p| p == "terrain.mcmap") || doc.destination.is_some()
        {
            copy_assets(source, &staged)?;
        } else {
            let config = doc.source.with_extension("ron");
            if config.exists() {
                std::fs::copy(config, staged.join("map.ron")).map_err(|e| e.to_string())?;
            }
        }
        let config = ron::ser::to_string_pretty(&doc.config, ron::ser::PrettyConfig::default())
            .map_err(|e| e.to_string())?;
        std::fs::write(staged.join("map.ron"), config).map_err(|e| e.to_string())?;
        let mut placed = doc.placed.clone();
        let mut order: Vec<usize> = (0..doc.map.props.len()).collect();
        order.sort_by_key(|&i| {
            let (x, y) = doc.map.info.tile_of(doc.map.props[i].pos);
            y * doc.map.info.tiles_w + x
        });
        let mut remap = vec![0; order.len()];
        for (new, &old) in order.iter().enumerate() {
            remap[old] = new as u32;
        }
        for d in &mut placed.doodads {
            d.prop_index = remap[d.prop_index as usize];
        }
        doc.map
            .save(&staged.join("terrain.mcmap"))
            .map_err(|e| e.to_string())?;
        placed
            .write(
                &staged.join("placed.ron"),
                &doc.map.info,
                doc.map.props.len(),
            )
            .map_err(|e| e.to_string())?;
        let existed = folder.exists();
        if existed {
            std::fs::rename(folder, &backup)
                .map_err(|e| format!("Could not preserve previous map: {e}"))?;
        }
        if let Err(error) = std::fs::rename(&staged, folder) {
            if existed {
                std::fs::rename(&backup, folder).map_err(|restore| {
                    format!(
                        "Save failed: {error}; previous map remains at {}: {restore}",
                        backup.display()
                    )
                })?;
            }
            return Err(error.to_string());
        }
        if existed {
            if let Err(e) = std::fs::remove_dir_all(&backup) {
                log::warn!(
                    "Saved map; could not remove backup {}: {e}",
                    backup.display()
                );
            }
        }
        Ok(())
    })();
    if staged.exists() {
        if let Err(e) = std::fs::remove_dir_all(&staged) {
            log::warn!("Save staging cleanup: {e}");
        }
    }
    result
}

fn copy_assets(source: &Path, destination: &Path) -> Result<(), String> {
    for entry in std::fs::read_dir(source).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name();
        if name == "terrain.mcmap" || name == "placed.ron" {
            continue;
        }
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        // A map is a self-contained folder. Never traverse user-created symlinks.
        if kind.is_symlink() {
            return Err(format!(
                "Map assets may not be symlinks: {}",
                entry.path().display()
            ));
        }
        let to = destination.join(name);
        if kind.is_dir() {
            std::fs::create_dir(&to).map_err(|e| e.to_string())?;
            copy_assets(&entry.path(), &to)?;
        } else if kind.is_file() {
            std::fs::copy(entry.path(), to).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
