//! Where songs and instrument presets live, and remembering the last song.
//!
//! The music folder is found the way the game finds `data`: walk up from the
//! working directory and from the executable until a `data/music` appears, so
//! the studio works from the repo root, from `target/release` and from a
//! Windows build directory outside the checkout alike.

use mc_music::Instrument;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub fn find_music_dir() -> Option<PathBuf> {
    let mut starts = Vec::new();
    if let Ok(d) = std::env::current_dir() {
        starts.push(d);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(d) = exe.parent() {
            starts.push(d.to_path_buf());
        }
    }
    for s in starts {
        let mut at: Option<&Path> = Some(&s);
        while let Some(dir) = at {
            let m = dir.join("data").join("music");
            if m.is_dir() {
                return Some(m);
            }
            at = dir.parent();
        }
    }
    None
}

pub fn save_preset(dir: &Path, name: &str, instrument: &Instrument) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let config = ron::ser::PrettyConfig::new()
        .depth_limit(4)
        .indentor("    ".to_string())
        .struct_names(false)
        .extensions(ron::extensions::Extensions::IMPLICIT_SOME);
    let mut text = ron::ser::to_string_pretty(instrument, config).map_err(|e| e.to_string())?;
    text.push('\n');
    let path = dir.join(format!("{name}.ron"));
    std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(path)
}

pub fn mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// A file name made from what someone typed: lower case, spaces to underscores.
pub fn file_stem(name: &str) -> String {
    let s: String = name
        .trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    let s = s.trim_matches('_').to_string();
    if s.is_empty() {
        "untitled".into()
    } else {
        s
    }
}

/// A remembered yes/no (a file in the config folder). Always false in tests.
pub fn flag(name: &str) -> bool {
    !cfg!(test) && config_dir().is_some_and(|d| d.join(name).is_file())
}

pub fn set_flag_to(name: &str, on: bool) {
    if cfg!(test) {
        return;
    }
    if let Some(d) = config_dir() {
        let _ = std::fs::create_dir_all(&d);
        if on {
            let _ = std::fs::write(d.join(name), b"1");
        } else {
            let _ = std::fs::remove_file(d.join(name));
        }
    }
}

/// A small text file in the config folder. Nothing is read or written in tests.
pub fn read_config(name: &str) -> Option<String> {
    if cfg!(test) {
        return None;
    }
    std::fs::read_to_string(config_dir()?.join(name)).ok()
}

pub fn write_config(name: &str, text: &str) {
    if cfg!(test) {
        return;
    }
    if let Some(d) = config_dir() {
        let _ = std::fs::create_dir_all(&d);
        let _ = std::fs::write(d.join(name), text);
    }
}

fn config_dir() -> Option<PathBuf> {
    if cfg!(windows) {
        std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("mc-studio"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .map(|d| d.join("mc-studio"))
    }
}

pub fn remember_song(path: &Path) {
    if cfg!(test) {
        return;
    }
    if let Some(dir) = config_dir() {
        let _ = std::fs::create_dir_all(&dir);
        let abs = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        let _ = std::fs::write(dir.join("last_song.txt"), abs.to_string_lossy().as_bytes());
    }
}

/// Opens a folder in the system's file browser.
pub fn reveal(dir: &Path) {
    let cmd = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    let _ = std::process::Command::new(cmd).arg(dir).spawn();
}

/// Shows a file in the system's file browser, selected where the browser can
/// do that (Explorer, Finder), else opens its folder.
pub fn reveal_file(path: &Path) {
    let mut cmd = if cfg!(windows) {
        let mut c = std::process::Command::new("explorer");
        c.arg(format!("/select,{}", path.display()));
        c
    } else if cfg!(target_os = "macos") {
        let mut c = std::process::Command::new("open");
        c.arg("-R").arg(path);
        c
    } else {
        reveal(path.parent().unwrap_or(path));
        return;
    };
    let _ = cmd.spawn();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stems_are_safe() {
        assert_eq!(file_stem("Reach Command"), "reach_command");
        assert_eq!(file_stem("  a/b\\c  "), "a_b_c");
        assert_eq!(file_stem("???"), "untitled");
    }
}
