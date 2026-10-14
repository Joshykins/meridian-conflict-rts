//! Reading a store over HTTPS (or `file://`, for tests and a local store),
//! through the system's `curl`: Windows 10 and later ship it, as does every
//! Linux. No TLS stack is built into the game for it, and nothing it fetches
//! is trusted before its manifest's signature or its hash says so.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use mc_core::Channel;

use crate::manifest::{Manifest, MAX_MANIFEST};
use crate::{Error, Result};

/// At most this many downloads at once.
const PARALLEL: u32 = 8;

#[derive(Clone, Debug)]
pub struct Store {
    base: String,
}

/// One file to download: the store path and where it goes.
pub struct Job {
    pub path: String,
    pub dest: PathBuf,
    /// The most it may be; a bigger answer is an error.
    pub max: u64,
}

impl Store {
    /// `base` is `https://...` or `file://...`, with or without a trailing `/`.
    pub fn new(base: &str) -> Result<Store> {
        let base = base.trim().trim_end_matches('/');
        if !(base.starts_with("https://") || base.starts_with("file://")) {
            return Err(Error::Malformed(format!(
                "store {base:?}: not an https:// or file:// URL"
            )));
        }
        Ok(Store {
            base: base.to_owned(),
        })
    }

    /// The store this build was made with (`MERIDIAN_STORE`).
    pub fn published() -> Result<Store> {
        Store::new(
            crate::STORE_URL
                .filter(|s| !s.is_empty())
                .ok_or(Error::NoStore)?,
        )
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    fn url(&self, path: &str) -> String {
        format!("{}/{path}", self.base)
    }

    /// A small file, whole, in memory.
    pub fn bytes(&self, path: &str, max: u64) -> Result<Vec<u8>> {
        let mut child = curl()
            .args(["--max-filesize", &max.to_string()])
            .arg(self.url(path))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(no_curl)?;
        let mut out = Vec::new();
        child
            .stdout
            .take()
            .expect("piped")
            .take(max + 1)
            .read_to_end(&mut out)?;
        let done = child.wait_with_output()?;
        if !done.status.success() {
            return Err(Error::Fetch(format!(
                "{path}: {}",
                String::from_utf8_lossy(&done.stderr).trim()
            )));
        }
        if out.len() as u64 > max {
            return Err(Error::Fetch(format!("{path}: larger than {max} bytes")));
        }
        Ok(out)
    }

    /// A manifest and its signature, verified against the release key.
    pub fn manifest_at(&self, path: &str) -> Result<Manifest> {
        let bytes = self.bytes(path, MAX_MANIFEST)?;
        let sig = self.bytes(&format!("{path}.sig"), 1024)?;
        Manifest::verify(&bytes, &sig)
    }

    /// The manifest of the build named `build` (as replays name it).
    pub fn manifest(&self, build: &str) -> Result<Manifest> {
        let m = self.manifest_at(&Manifest::store_path(crate::platform(), build))?;
        if m.build != build {
            return Err(Error::Malformed(format!(
                "asked for {build}, the store gave {}",
                m.build
            )));
        }
        Ok(m)
    }

    /// The newest build published on `channel`.
    pub fn newest(&self, channel: Channel) -> Result<Manifest> {
        let m = self.manifest_at(&Manifest::channel_path(channel, crate::platform()))?;
        if m.channel() != Some(channel) {
            return Err(Error::Malformed(format!(
                "the {channel} channel names a {} build",
                m.channel
            )));
        }
        Ok(m)
    }

    /// Downloads every job, up to [`PARALLEL`] at once. `progress` hears the
    /// bytes on disk so far about every tenth of a second; `cancel` stops it.
    pub fn download(
        &self,
        jobs: &[Job],
        progress: &mut dyn FnMut(u64),
        cancel: &AtomicBool,
    ) -> Result<()> {
        if jobs.is_empty() {
            return Ok(());
        }
        let Some(dir) = jobs[0].dest.parent() else {
            return Err(Error::Malformed("download into no folder".into()));
        };
        let config = dir.join("curl-jobs.txt");
        let mut text = String::new();
        let mut biggest = 0;
        for j in jobs {
            text += &format!(
                "url = \"{}\"\noutput = \"{}\"\n",
                quote(&self.url(&j.path)),
                quote(&j.dest.to_string_lossy())
            );
            biggest = biggest.max(j.max);
        }
        std::fs::write(&config, text)?;
        let mut child = curl()
            .args(["--parallel", "--parallel-max", &PARALLEL.to_string()])
            .args(["--max-filesize", &biggest.to_string()])
            .arg("--config")
            .arg(&config)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(no_curl)?;
        let status = loop {
            if cancel.load(Ordering::Relaxed) {
                let _ = child.kill();
                let _ = child.wait();
                let _ = std::fs::remove_file(&config);
                return Err(Error::Cancelled);
            }
            if let Some(status) = child.try_wait()? {
                break status;
            }
            progress(on_disk(jobs));
            std::thread::sleep(Duration::from_millis(100));
        };
        let _ = std::fs::remove_file(&config);
        if !status.success() {
            let mut err = String::new();
            if let Some(mut e) = child.stderr.take() {
                let _ = e.read_to_string(&mut err);
            }
            return Err(Error::Fetch(err.trim().to_owned()));
        }
        progress(on_disk(jobs));
        for j in jobs {
            let size = std::fs::metadata(&j.dest).map(|m| m.len()).unwrap_or(0);
            if size > j.max {
                return Err(Error::Fetch(format!(
                    "{}: larger than its manifest says",
                    j.path
                )));
            }
        }
        Ok(())
    }
}

fn on_disk(jobs: &[Job]) -> u64 {
    jobs.iter()
        .filter_map(|j| std::fs::metadata(&j.dest).ok())
        .map(|m| m.len())
        .sum()
}

/// A curl config file's quoted string: backslashes and quotes escaped.
fn quote(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn curl() -> Command {
    let mut c = Command::new("curl");
    c.args([
        "--fail",
        "--silent",
        "--show-error",
        "--location",
        "--retry",
        "3",
        "--connect-timeout",
        "20",
    ]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: no console flashes up over the game.
        c.creation_flags(0x0800_0000);
    }
    c
}

fn no_curl(e: std::io::Error) -> Error {
    Error::Fetch(format!("could not run curl: {e}"))
}

/// `file://` for a folder, as [`Store::new`] takes it.
pub fn file_url(dir: &Path) -> String {
    let path = dir.to_string_lossy().replace('\\', "/");
    if path.starts_with('/') {
        format!("file://{path}")
    } else {
        format!("file:///{path}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_https_and_file_stores() {
        assert!(Store::new("https://builds.example.com/").is_ok());
        assert!(Store::new("file:///tmp/store").is_ok());
        assert!(Store::new("http://insecure.example.com").is_err());
        assert!(Store::new("").is_err());
        assert_eq!(
            Store::new("https://a.b/").unwrap().url("blobs/x"),
            "https://a.b/blobs/x"
        );
    }

    #[test]
    fn quoting_survives_windows_paths() {
        assert_eq!(quote(r#"C:\a "b""#), r#"C:\\a \"b\""#);
    }
}
