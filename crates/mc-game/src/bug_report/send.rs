//! Getting a report to us off the UI thread: the screenshot written as a PNG,
//! then `mc_builds::report::send`. The Report a Bug panel and the crash screen
//! both send through here.

use mc_builds::report::{self, Attachment, Report};
use mc_render::Shot;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;

/// What became of a report: the reference it was given, or why it did not go.
pub(crate) type Outcome = Result<String, String>;

/// The widest a screenshot is sent: a wider frame is halved until it fits. A
/// 4K PNG would be most of a report's size and show nothing more.
const SHOT_MAX_W: u32 = 1920;

/// Whether this build can send reports at all: a dev build has nowhere to.
pub(crate) fn available() -> bool {
    report::endpoint().is_ok()
}

/// Sends `report` (with `shot`, when there is one) on a thread of its own; the
/// receiver hears how it went.
pub(crate) fn spawn(mut report: Report, shot: Option<Arc<Shot>>) -> Receiver<Outcome> {
    let (tx, rx) = channel();
    // A thread that cannot start drops `tx`: the receiver hears it as a failure.
    let _ = std::thread::Builder::new()
        .name("report-send".into())
        .spawn(move || {
            let outcome = run(&mut report, shot.as_deref());
            match &outcome {
                Ok(id) => log::info!("report sent: {id}"),
                Err(e) => log::warn!("the report was not sent: {e}"),
            }
            let _ = tx.send(outcome);
        });
    rx
}

fn run(report: &mut Report, shot: Option<&Shot>) -> Outcome {
    let url = report::endpoint().map_err(|e| e.to_string())?;
    let scratch = scratch_dir();
    std::fs::create_dir_all(&scratch).map_err(|e| format!("{}: {e}", scratch.display()))?;
    if let Some(shot) = shot {
        let path = scratch.join("screenshot.png");
        let (w, h, rgba) = fit(shot);
        crate::headless::write_png(&path, w, h, &rgba)?;
        report.attachments.push(Attachment {
            field: "screenshot",
            path,
            mime: "image/png",
        });
    }
    let outcome = report::send(&url, report, &scratch).map_err(|e| e.to_string());
    let _ = std::fs::remove_dir_all(&scratch);
    outcome
}

/// A folder of this send's own: two reports sent at once never share one.
fn scratch_dir() -> PathBuf {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    std::env::temp_dir().join(format!(
        "meridian-report-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

/// The frame at most [`SHOT_MAX_W`] wide: halved, each pixel the mean of four,
/// as often as it takes.
fn fit(shot: &Shot) -> (u32, u32, std::borrow::Cow<'_, [u8]>) {
    let (mut w, mut h) = (shot.width, shot.height);
    let mut rgba = std::borrow::Cow::Borrowed(shot.rgba.as_slice());
    while w > SHOT_MAX_W && h >= 2 {
        let (hw, hh) = (w / 2, h / 2);
        let mut out = vec![0u8; (hw * hh * 4) as usize];
        for y in 0..hh as usize {
            for x in 0..hw as usize {
                for c in 0..4 {
                    let at = |dx: usize, dy: usize| {
                        u32::from(rgba[((2 * y + dy) * w as usize + 2 * x + dx) * 4 + c])
                    };
                    out[(y * hw as usize + x) * 4 + c] =
                        ((at(0, 0) + at(1, 0) + at(0, 1) + at(1, 1) + 2) / 4) as u8;
                }
            }
        }
        (w, h, rgba) = (hw, hh, std::borrow::Cow::Owned(out));
    }
    (w, h, rgba)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wide_frame_is_halved_to_fit() {
        let shot = Shot {
            width: 3840,
            height: 2160,
            rgba: [10u8, 20, 30, 255].repeat(3840 * 2160),
        };
        let (w, h, rgba) = fit(&shot);
        assert_eq!((w, h), (1920, 1080));
        assert_eq!(rgba.len(), 1920 * 1080 * 4);
        assert_eq!(&rgba[..4], &[10, 20, 30, 255]);
        let small = Shot {
            width: 1280,
            height: 720,
            rgba: vec![0; 1280 * 720 * 4],
        };
        assert!(matches!(fit(&small).2, std::borrow::Cow::Borrowed(_)));
    }
}
