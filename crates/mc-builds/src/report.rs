//! Sending a report to us: a player's bug report (the Report a Bug panel) or a
//! crash report (the crash screen's Send report). Posted as a multipart form to
//! the reports endpoint this build was made with (`MERIDIAN_REPORTS`; the Worker
//! in `deploy/reports` takes it in), through the system's `curl`, as the store
//! is read: no TLS stack is built into the game for it.

use std::ffi::OsString;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use crate::{Error, Result};

/// Where this build sends reports, set at compile time (`MERIDIAN_REPORTS`, by
/// scripts/release.sh). A dev build has none, so playing one sends nothing.
pub const REPORTS_URL: Option<&str> = option_env!("MERIDIAN_REPORTS");

/// The most a report's text may be; the endpoint refuses more.
pub const MAX_TEXT: usize = 1 << 20;

/// The most an attachment may be; a bigger one is left out, and the report says so.
pub const MAX_ATTACHMENT: u64 = 8 << 20;

/// The most a player's note may be, in characters.
pub const MAX_NOTE: usize = 2000;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// A player pressed Report a Bug.
    Bug,
    /// The game crashed or had to stop, and the player pressed Send report.
    Crash,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::Bug => "bug",
            Kind::Crash => "crash",
        }
    }
}

/// A file sent with a report.
pub struct Attachment {
    /// The form field: `screenshot`, `dump`.
    pub field: &'static str,
    pub path: PathBuf,
    pub mime: &'static str,
}

pub struct Report {
    pub kind: Kind,
    /// The build's name (`MERIDIAN_BUILD`).
    pub build: String,
    /// The player's callsign, to answer them by.
    pub player: String,
    /// What the player wrote; may be empty.
    pub note: String,
    /// The report itself: what happened, the machine, the game's state, the
    /// settings and the log.
    pub text: String,
    pub attachments: Vec<Attachment>,
}

/// Where this build sends reports, if it was made with somewhere.
pub fn endpoint() -> Result<String> {
    let base = REPORTS_URL
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or(Error::NoReports)?;
    if !base.starts_with("https://") {
        return Err(Error::Malformed(format!(
            "reports endpoint {base:?}: not an https:// URL"
        )));
    }
    Ok(format!("{}/report", base.trim_end_matches('/')))
}

/// Sends `report` to `url`; returns the reference the endpoint gave it, for the
/// player to quote. Blocks until it is sent or has failed: call it off the UI
/// thread. `scratch` is a folder of the caller's to stage the report's text in.
pub fn send(url: &str, report: &Report, scratch: &Path) -> Result<String> {
    std::fs::create_dir_all(scratch)?;
    let text_path = scratch.join("report.txt");
    std::fs::write(&text_path, clip(&report.text, MAX_TEXT))?;
    let mut child = crate::fetch::curl()
        // A report is a few MB at most; a slow line still gets it there.
        .args(["--max-time", "180"])
        .args(form(report, &text_path))
        .arg(url)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(crate::fetch::no_curl)?;
    let mut answer = Vec::new();
    child
        .stdout
        .take()
        .expect("piped")
        .take(4096)
        .read_to_end(&mut answer)?;
    let done = child.wait_with_output()?;
    let _ = std::fs::remove_file(&text_path);
    if !done.status.success() {
        return Err(Error::Fetch(format!(
            "sending the report: {}",
            String::from_utf8_lossy(&done.stderr).trim()
        )));
    }
    reference(&String::from_utf8_lossy(&answer))
}

/// The form's arguments: plain fields as `--form-string` (taken as written, so
/// a note starting `@` or `<` is not read as a file), files as `-F`.
fn form(report: &Report, text: &Path) -> Vec<OsString> {
    let mut args: Vec<OsString> = Vec::new();
    let mut field = |name: &str, value: &str| {
        args.push("--form-string".into());
        args.push(format!("{name}={value}").into());
    };
    field("kind", report.kind.name());
    field("build", &report.build);
    field("platform", crate::platform());
    field("player", &report.player);
    // The note field caps it at MAX_NOTE characters as it is typed.
    field("note", report.note.trim());
    // A file past the limit (a huge dump) stays on the player's disk; the
    // report's `left_out` field says which and why.
    let mut left_out = Vec::new();
    let mut files = vec![format!("report=@\"{}\";type=text/plain", quote(text))];
    for a in &report.attachments {
        match std::fs::metadata(&a.path) {
            Ok(m) if m.len() <= MAX_ATTACHMENT => files.push(format!(
                "{}=@\"{}\";type={}",
                a.field,
                quote(&a.path),
                a.mime
            )),
            Ok(m) => left_out.push(format!("{}: {} bytes", a.path.display(), m.len())),
            Err(e) => left_out.push(format!("{}: {e}", a.path.display())),
        }
    }
    if !left_out.is_empty() {
        field("left_out", &left_out.join("\n"));
    }
    for f in files {
        args.push("-F".into());
        args.push(f.into());
    }
    args
}

/// A path as curl's `-F` reads a quoted file name: backslashes and quotes escaped.
fn quote(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
}

/// The endpoint's answer, `{"id":"..."}`: the report's reference.
fn reference(answer: &str) -> Result<String> {
    let value: serde_json::Value = serde_json::from_str(answer.trim())
        .map_err(|e| Error::Malformed(format!("the reports endpoint answered {answer:?}: {e}")))?;
    value
        .get("id")
        .and_then(|id| id.as_str())
        .filter(|id| !id.is_empty() && id.len() <= 64)
        .map(str::to_owned)
        .ok_or_else(|| Error::Malformed(format!("the reports endpoint answered {answer:?}")))
}

/// At most `max` bytes of `text`, cut on a character, the end kept: a report's
/// last lines (the log's) say the most.
fn clip(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut at = text.len() - max;
    while !text.is_char_boundary(at) {
        at += 1;
    }
    &text[at..]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report() -> Report {
        Report {
            kind: Kind::Bug,
            build: "0.1.0-dev+abc".into(),
            player: "Capn".into(),
            note: "  @etc/passwd <is not a file  ".into(),
            text: "report".into(),
            attachments: vec![Attachment {
                field: "screenshot",
                path: PathBuf::from("/no/such/shot.png"),
                mime: "image/png",
            }],
        }
    }

    #[test]
    fn a_note_is_sent_as_written_and_a_missing_file_is_left_out() {
        let args: Vec<String> = form(&report(), Path::new(r#"C:\a "b"\report.txt"#))
            .into_iter()
            .map(|a| a.into_string().unwrap())
            .collect();
        let at = args
            .iter()
            .position(|a| a.starts_with("note="))
            .expect("a note field");
        assert_eq!(args[at - 1], "--form-string");
        assert_eq!(args[at], "note=@etc/passwd <is not a file");
        assert!(args.contains(&"kind=bug".to_owned()));
        assert!(args.contains(&r#"report=@"C:\\a \"b\"\\report.txt";type=text/plain"#.to_owned()));
        assert!(!args.iter().any(|a| a.starts_with("screenshot=")));
        assert!(args
            .iter()
            .any(|a| a.starts_with("left_out=/no/such/shot.png: ")));
    }

    #[test]
    fn the_answer_names_the_report() {
        assert_eq!(reference(r#" {"id":"r-7f3a"} "#).unwrap(), "r-7f3a");
        assert!(reference("<html>bad gateway</html>").is_err());
        assert!(reference(r#"{"id":""}"#).is_err());
    }

    #[test]
    fn clip_keeps_the_end_on_a_character() {
        assert_eq!(clip("héllo", 4), "llo");
        assert_eq!(clip("hi", 4), "hi");
    }
}
